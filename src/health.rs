//! Health endpoints + offline device sweeper + alert listing.

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::AuthDevice;
use crate::config::Config;
use crate::error::AppError;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(live))
        .route("/ready", get(ready))
        .route("/devices/{id}/alerts", get(list_alerts))
}

/// GET /health — process is up (does not check DB).
async fn live() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

/// GET /ready — can talk to Postgres.
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
        SELECT id, device_id, kind, message, created_at, telemetry_id
        FROM alerts
        WHERE device_id = $1
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows))
}

/// Background loop: mark devices offline if they stopped sending data.
///
/// Why a loop? HTTP handlers only run when a request arrives.
/// A silent device never hits an API — so we need a timer.
pub async fn offline_sweeper(pool: PgPool, config: Config) {
    let mut ticker =
        tokio::time::interval(std::time::Duration::from_secs(config.sweep_interval_secs));

    loop {
        ticker.tick().await;

        // Prefer DB setting (UI can change it); fall back to env config.
        let offline_after = match crate::settings::load_thresholds(&pool).await {
            Ok(t) => t.offline_after_secs as f64,
            Err(_) => config.offline_after_secs as f64,
        };

        let result = sqlx::query(
            r#"
            UPDATE devices
            SET status = 'offline'
            WHERE status = 'online'
              AND last_seen_at < now() - make_interval(secs => $1)
            "#,
        )
        .bind(offline_after)
        .execute(&pool)
        .await;

        match result {
            Ok(res) if res.rows_affected() > 0 => {
                tracing::info!("marked {} device(s) offline", res.rows_affected());
            }
            Ok(_) => {}
            Err(err) => tracing::error!("offline sweeper failed: {}", err),
        }
    }
}
