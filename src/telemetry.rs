//! Telemetry ingestion — the core of FleetPulse.
//!
//! Idempotency: client sends event `id`. That id is the PRIMARY KEY.
//! Sending the same id twice does NOT create two rows.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthDevice;
use crate::error::AppError;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/telemetry", post(ingest_telemetry))
}

#[derive(Debug, Deserialize)]
pub struct TelemetryRequest {
    /// Client-chosen UUID. Re-use the same id on retry = safe.
    pub id: Uuid,
    pub recorded_at: DateTime<Utc>,
    pub latitude: f64,
    pub longitude: f64,
    pub speed_kmh: f64,
    pub battery_pct: f64,
    pub engine_temp_c: f64,
}

#[derive(Debug, Serialize)]
pub struct TelemetryResponse {
    pub status: String,
    pub alerts_created: Vec<String>,
}

/// POST /telemetry
async fn ingest_telemetry(
    State(state): State<AppState>,
    auth: AuthDevice,
    Json(body): Json<TelemetryRequest>,
) -> Result<(StatusCode, Json<TelemetryResponse>), AppError> {
    validate(&body)?;

    // Try insert. If PK already exists → duplicate (idempotent success).
    let insert = sqlx::query(
        r#"
        INSERT INTO telemetry_events (
            id, device_id, recorded_at, latitude, longitude,
            speed_kmh, battery_pct, engine_temp_c
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(body.id)
    .bind(auth.id)
    .bind(body.recorded_at)
    .bind(body.latitude)
    .bind(body.longitude)
    .bind(body.speed_kmh)
    .bind(body.battery_pct)
    .bind(body.engine_temp_c)
    .execute(&state.pool)
    .await;

    match insert {
        Ok(_) => {
            // Mark device online + update last_seen
            sqlx::query(
                r#"
                UPDATE devices
                SET status = 'online', last_seen_at = now()
                WHERE id = $1
                "#,
            )
            .bind(auth.id)
            .execute(&state.pool)
            .await?;

            let alerts = maybe_create_alerts(&state, auth.id, body.id, &body).await?;

            Ok((
                StatusCode::CREATED,
                Json(TelemetryResponse {
                    status: "created".into(),
                    alerts_created: alerts,
                }),
            ))
        }
        Err(sqlx::Error::Database(db_err)) if db_err.constraint() == Some("telemetry_events_pkey") => {
            // Same event id again → treat as success (idempotent).
            Ok((
                StatusCode::OK,
                Json(TelemetryResponse {
                    status: "duplicate_ignored".into(),
                    alerts_created: vec![],
                }),
            ))
        }
        Err(err) => Err(AppError::from(err)),
    }
}

fn validate(body: &TelemetryRequest) -> Result<(), AppError> {
    if !(-90.0..=90.0).contains(&body.latitude) {
        return Err(AppError::BadRequest("latitude must be between -90 and 90".into()));
    }
    if !(-180.0..=180.0).contains(&body.longitude) {
        return Err(AppError::BadRequest(
            "longitude must be between -180 and 180".into(),
        ));
    }
    if body.speed_kmh < 0.0 {
        return Err(AppError::BadRequest("speed_kmh must be >= 0".into()));
    }
    if !(0.0..=100.0).contains(&body.battery_pct) {
        return Err(AppError::BadRequest(
            "battery_pct must be between 0 and 100".into(),
        ));
    }
    Ok(())
}

async fn maybe_create_alerts(
    state: &AppState,
    device_id: Uuid,
    telemetry_id: Uuid,
    body: &TelemetryRequest,
) -> Result<Vec<String>, AppError> {
    // Thresholds come from DB so the UI "red zone" form can change them live.
    let thresholds = crate::settings::load_thresholds(&state.pool).await?;
    let mut created = Vec::new();

    if body.engine_temp_c > thresholds.engine_temp_alert_c {
        let kind = "high_engine_temp";
        let message = format!(
            "engine_temp_c={} above threshold {}",
            body.engine_temp_c, thresholds.engine_temp_alert_c
        );
        insert_alert(state, device_id, telemetry_id, kind, &message).await?;
        created.push(kind.to_string());
    }

    if body.battery_pct < thresholds.battery_low_pct {
        let kind = "low_battery";
        let message = format!(
            "battery_pct={} below threshold {}",
            body.battery_pct, thresholds.battery_low_pct
        );
        insert_alert(state, device_id, telemetry_id, kind, &message).await?;
        created.push(kind.to_string());
    }

    Ok(created)
}

async fn insert_alert(
    state: &AppState,
    device_id: Uuid,
    telemetry_id: Uuid,
    kind: &str,
    message: &str,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO alerts (id, device_id, kind, message, telemetry_id)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(device_id)
    .bind(kind)
    .bind(message)
    .bind(telemetry_id)
    .execute(&state.pool)
    .await?;
    Ok(())
}
