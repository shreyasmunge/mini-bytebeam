//! Fleet snapshot APIs — map reads device_state (twin), not full history.

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/fleet/snapshot", get(fleet_snapshot))
        .route("/devices/{id}/telemetry", get(device_telemetry))
}

#[derive(Debug, Serialize)]
pub struct FleetVehicle {
    pub id: Uuid,
    pub name: String,
    pub status: String,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub speed_kmh: Option<f64>,
    pub battery_pct: Option<f64>,
    pub engine_temp_c: Option<f64>,
    pub recorded_at: Option<DateTime<Utc>>,
    pub in_red_zone: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct FleetVehicleRow {
    id: Uuid,
    name: String,
    status: String,
    last_seen_at: Option<DateTime<Utc>>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    speed_kmh: Option<f64>,
    battery_pct: Option<f64>,
    engine_temp_c: Option<f64>,
    recorded_at: Option<DateTime<Utc>>,
}

/// GET /fleet/snapshot — O(devices) via twin table.
async fn fleet_snapshot(State(state): State<AppState>) -> Result<Json<Vec<FleetVehicle>>, AppError> {
    let thresholds = crate::settings::load_thresholds(&state.pool).await?;

    let rows = sqlx::query_as::<_, FleetVehicleRow>(
        r#"
        SELECT
            d.id,
            d.name,
            d.status,
            COALESCE(s.last_seen_at, d.last_seen_at) AS last_seen_at,
            s.latitude,
            s.longitude,
            s.speed_kmh,
            s.battery_pct,
            s.engine_temp_c,
            s.recorded_at
        FROM devices d
        LEFT JOIN device_state s ON s.device_id = d.id
        ORDER BY d.name
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let vehicles = rows
        .into_iter()
        .map(|r| {
            let reading_red = match (r.battery_pct, r.engine_temp_c) {
                (Some(b), Some(t)) => {
                    b < thresholds.battery_low_pct || t > thresholds.engine_temp_alert_c
                }
                (Some(b), None) => b < thresholds.battery_low_pct,
                (None, Some(t)) => t > thresholds.engine_temp_alert_c,
                _ => false,
            };
            let in_red_zone = reading_red || r.status == "offline";

            FleetVehicle {
                id: r.id,
                name: r.name,
                status: r.status,
                last_seen_at: r.last_seen_at,
                latitude: r.latitude,
                longitude: r.longitude,
                speed_kmh: r.speed_kmh,
                battery_pct: r.battery_pct,
                engine_temp_c: r.engine_temp_c,
                recorded_at: r.recorded_at,
                in_red_zone,
            }
        })
        .collect();

    Ok(Json(vehicles))
}

#[derive(Debug, Deserialize)]
pub struct TelemetryQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TelemetryRow {
    pub id: Uuid,
    pub recorded_at: DateTime<Utc>,
    pub latitude: f64,
    pub longitude: f64,
    pub speed_kmh: f64,
    pub battery_pct: f64,
    pub engine_temp_c: f64,
}

/// GET /devices/:id/telemetry?limit=50 — history path (not used by live map).
async fn device_telemetry(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<TelemetryQuery>,
) -> Result<Json<Vec<TelemetryRow>>, AppError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let rows = sqlx::query_as::<_, TelemetryRow>(
        r#"
        SELECT id, recorded_at, latitude, longitude, speed_kmh, battery_pct, engine_temp_c
        FROM telemetry_events
        WHERE device_id = $1
        ORDER BY recorded_at DESC
        LIMIT $2
        "#,
    )
    .bind(id)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}
