//! Health endpoints, alert inbox + acknowledge, offline sweeper.

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use std::sync::atomic::Ordering;
use uuid::Uuid;

use crate::auth::AuthDevice;
use crate::config::Config;
use crate::error::AppError;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(live))
        .route("/ready", get(ready))
        .route("/alerts", get(list_fleet_alerts))
        .route("/alerts/{id}/ack", post(ack_alert))
        .route("/devices/{id}/alerts", get(list_alerts))
}

async fn live() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

async fn ready(State(state): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    sqlx::query("SELECT 1").execute(&state.pool).await?;
    Ok(Json(json!({ "status": "ready" })))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AlertRow {
    pub id: Uuid,
    pub device_id: Uuid,
    pub kind: String,
    pub message: String,
    pub created_at: DateTime<Utc>,
    pub telemetry_id: Option<Uuid>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub device_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AlertListQuery {
    /// open | all (default open)
    pub status: Option<String>,
    pub limit: Option<i64>,
}

/// GET /alerts?status=open — fleet alert inbox (dashboard).
async fn list_fleet_alerts(
    State(state): State<AppState>,
    Query(query): Query<AlertListQuery>,
) -> Result<Json<Vec<AlertRow>>, AppError> {
    let status = query.status.unwrap_or_else(|| "open".into());
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let open_only = status != "all";

    let rows = sqlx::query_as::<_, AlertRow>(
        r#"
        SELECT
            a.id, a.device_id, a.kind, a.message, a.created_at,
            a.telemetry_id, a.acknowledged_at, d.name AS device_name
        FROM alerts a
        JOIN devices d ON d.id = a.device_id
        WHERE ($1 = false OR a.acknowledged_at IS NULL)
        ORDER BY a.created_at DESC
        LIMIT $2
        "#,
    )
    .bind(open_only)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows))
}

/// POST /alerts/:id/ack
async fn ack_alert(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let res = sqlx::query(
        r#"
        UPDATE alerts
        SET acknowledged_at = now()
        WHERE id = $1 AND acknowledged_at IS NULL
        "#,
    )
    .bind(id)
    .execute(&state.pool)
    .await?;

    if res.rows_affected() == 0 {
        // Already acked or missing — check existence.
        let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM alerts WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?;
        if exists.is_none() {
            return Err(AppError::NotFound("alert not found".into()));
        }
    }

    Ok(Json(json!({ "status": "acknowledged", "id": id })))
}

/// GET /devices/:id/alerts
async fn list_alerts(
    State(state): State<AppState>,
    auth: AuthDevice,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<AlertRow>>, AppError> {
    if auth.id != id {
        return Err(AppError::Unauthorized(
            "you can only view your own alerts".into(),
        ));
    }

    let rows = sqlx::query_as::<_, AlertRow>(
        r#"
        SELECT
            a.id, a.device_id, a.kind, a.message, a.created_at,
            a.telemetry_id, a.acknowledged_at, d.name AS device_name
        FROM alerts a
        JOIN devices d ON d.id = a.device_id
        WHERE a.device_id = $1
        ORDER BY a.created_at DESC
        LIMIT 50
        "#,
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows))
}

/// Background loop: mark devices offline + raise offline alerts once.
pub async fn offline_sweeper(pool: PgPool, config: Config, metrics: std::sync::Arc<crate::metrics::PipelineMetrics>) {
    let mut ticker =
        tokio::time::interval(std::time::Duration::from_secs(config.sweep_interval_secs));

    loop {
        ticker.tick().await;

        let offline_after = match crate::settings::load_thresholds(&pool).await {
            Ok(t) => t.offline_after_secs as f64,
            Err(_) => config.offline_after_secs as f64,
        };

        // Devices going offline now.
        let newly_offline: Result<Vec<(Uuid,)>, sqlx::Error> = sqlx::query_as(
            r#"
            UPDATE devices
            SET status = 'offline'
            WHERE status = 'online'
              AND last_seen_at < now() - make_interval(secs => $1)
            RETURNING id
            "#,
        )
        .bind(offline_after)
        .fetch_all(&pool)
        .await;

        match newly_offline {
            Ok(ids) if !ids.is_empty() => {
                tracing::info!("marked {} device(s) offline", ids.len());
                for (device_id,) in ids {
                    // Avoid alert spam: only if no open offline alert.
                    let open: Option<(Uuid,)> = sqlx::query_as(
                        r#"
                        SELECT id FROM alerts
                        WHERE device_id = $1 AND kind = 'offline' AND acknowledged_at IS NULL
                        LIMIT 1
                        "#,
                    )
                    .bind(device_id)
                    .fetch_optional(&pool)
                    .await
                    .unwrap_or(None);

                    if open.is_none() {
                        let _ = sqlx::query(
                            r#"
                            INSERT INTO alerts (id, device_id, kind, message, telemetry_id)
                            VALUES ($1, $2, 'offline', 'device stopped sending telemetry', NULL)
                            "#,
                        )
                        .bind(Uuid::new_v4())
                        .bind(device_id)
                        .execute(&pool)
                        .await;
                        metrics.alerts_created.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            Ok(_) => {}
            Err(err) => tracing::error!("offline sweeper failed: {}", err),
        }
    }
}

/// Retention / rollup job — keep recent raw data, roll older into 1m buckets.
pub async fn retention_worker(pool: PgPool, retain_days: i64) {
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(3600));
    loop {
        ticker.tick().await;
        if let Err(err) = run_retention(&pool, retain_days).await {
            tracing::error!("retention job failed: {}", err);
        }
    }
}

async fn run_retention(pool: &PgPool, retain_days: i64) -> Result<(), sqlx::Error> {
    // Roll up hour-old data into plain 1m aggregates (idempotent upsert).
    sqlx::query(
        r#"
        INSERT INTO telemetry_1m_plain (
            bucket, device_id, avg_speed_kmh, avg_battery_pct, avg_engine_temp_c, samples
        )
        SELECT
            date_trunc('minute', recorded_at) AS bucket,
            device_id,
            avg(speed_kmh),
            avg(battery_pct),
            avg(engine_temp_c),
            count(*)
        FROM telemetry_events
        WHERE recorded_at < now() - interval '1 hour'
          AND recorded_at >= now() - ($1::int * interval '1 day')
        GROUP BY 1, 2
        ON CONFLICT (bucket, device_id) DO UPDATE SET
            avg_speed_kmh = EXCLUDED.avg_speed_kmh,
            avg_battery_pct = EXCLUDED.avg_battery_pct,
            avg_engine_temp_c = EXCLUDED.avg_engine_temp_c,
            samples = EXCLUDED.samples
        "#,
    )
    .bind(retain_days as i32)
    .execute(pool)
    .await?;

    let deleted = sqlx::query(
        r#"
        DELETE FROM telemetry_events
        WHERE recorded_at < now() - ($1::int * interval '1 day')
        "#,
    )
    .bind(retain_days as i32)
    .execute(pool)
    .await?;

    if deleted.rows_affected() > 0 {
        tracing::info!(
            "retention deleted {} old telemetry rows (keep {} days)",
            deleted.rows_affected(),
            retain_days
        );
    }
    Ok(())
}

