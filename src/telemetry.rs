//! Telemetry ingestion — single + batch, idempotent, updates device twin.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use uuid::Uuid;

use crate::auth::AuthDevice;
use crate::error::AppError;
use crate::twin::{mark_device_online, upsert_twin, TwinUpdate};
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/telemetry", post(ingest_telemetry))
        .route("/telemetry/batch", post(ingest_batch))
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub struct BatchRequest {
    pub events: Vec<TelemetryRequest>,
}

#[derive(Debug, Serialize)]
pub struct BatchResponse {
    pub accepted: usize,
    pub duplicates: usize,
    pub alerts_created: Vec<String>,
}

/// POST /telemetry
async fn ingest_telemetry(
    State(state): State<AppState>,
    auth: AuthDevice,
    Json(body): Json<TelemetryRequest>,
) -> Result<(StatusCode, Json<TelemetryResponse>), AppError> {
    validate(&body)?;

    let insert = sqlx::query(
        r#"
        INSERT INTO telemetry_events (
            id, device_id, recorded_at, latitude, longitude,
            speed_kmh, battery_pct, engine_temp_c
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (id) DO NOTHING
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
        Ok(res) if res.rows_affected() == 0 => {
            state.metrics.duplicates.fetch_add(1, Ordering::Relaxed);
            state
                .metrics
                .record(
                    auth.id,
                    &auth.name,
                    "/telemetry",
                    "duplicate_ignored",
                    1,
                    vec![],
                    format!("event_id={}", body.id),
                )
                .await;
            Ok((
                StatusCode::OK,
                Json(TelemetryResponse {
                    status: "duplicate_ignored".into(),
                    alerts_created: vec![],
                }),
            ))
        }
        Ok(_) => {
            mark_device_online(&state.pool, auth.id).await?;
            upsert_twin(
                &state.pool,
                &TwinUpdate {
                    device_id: auth.id,
                    recorded_at: body.recorded_at,
                    latitude: body.latitude,
                    longitude: body.longitude,
                    speed_kmh: body.speed_kmh,
                    battery_pct: body.battery_pct,
                    engine_temp_c: body.engine_temp_c,
                },
            )
            .await?;

            let alerts = maybe_create_alerts(&state, auth.id, body.id, &body).await?;
            state.metrics.ingested.fetch_add(1, Ordering::Relaxed);
            state
                .metrics
                .alerts_created
                .fetch_add(alerts.len() as u64, Ordering::Relaxed);
            state
                .metrics
                .record(
                    auth.id,
                    &auth.name,
                    "/telemetry",
                    "created",
                    1,
                    alerts.clone(),
                    format!(
                        "lat={:.4} lon={:.4} batt={:.0} temp={:.0}",
                        body.latitude, body.longitude, body.battery_pct, body.engine_temp_c
                    ),
                )
                .await;

            Ok((
                StatusCode::CREATED,
                Json(TelemetryResponse {
                    status: "created".into(),
                    alerts_created: alerts,
                }),
            ))
        }
        Err(err) => {
            state.metrics.ingest_errors.fetch_add(1, Ordering::Relaxed);
            state
                .metrics
                .record(
                    auth.id,
                    &auth.name,
                    "/telemetry",
                    "error",
                    1,
                    vec![],
                    err.to_string(),
                )
                .await;
            Err(AppError::from(err))
        }
    }
}

/// POST /telemetry/batch — many events, one auth device (or same device fleet tick).
/// For multi-device scale tests, prefer many concurrent single/batch calls with per-device keys.
async fn ingest_batch(
    State(state): State<AppState>,
    auth: AuthDevice,
    Json(body): Json<BatchRequest>,
) -> Result<(StatusCode, Json<BatchResponse>), AppError> {
    if body.events.is_empty() {
        return Err(AppError::BadRequest("events must not be empty".into()));
    }
    if body.events.len() > 2000 {
        return Err(AppError::BadRequest("max 2000 events per batch".into()));
    }

    for e in &body.events {
        validate(e)?;
    }

    let mut ids = Vec::with_capacity(body.events.len());
    let mut device_ids = Vec::with_capacity(body.events.len());
    let mut recorded_ats = Vec::with_capacity(body.events.len());
    let mut lats = Vec::with_capacity(body.events.len());
    let mut lons = Vec::with_capacity(body.events.len());
    let mut speeds = Vec::with_capacity(body.events.len());
    let mut batteries = Vec::with_capacity(body.events.len());
    let mut temps = Vec::with_capacity(body.events.len());

    for e in &body.events {
        ids.push(e.id);
        device_ids.push(auth.id);
        recorded_ats.push(e.recorded_at);
        lats.push(e.latitude);
        lons.push(e.longitude);
        speeds.push(e.speed_kmh);
        batteries.push(e.battery_pct);
        temps.push(e.engine_temp_c);
    }

    let inserted = sqlx::query(
        r#"
        INSERT INTO telemetry_events (
            id, device_id, recorded_at, latitude, longitude,
            speed_kmh, battery_pct, engine_temp_c
        )
        SELECT * FROM UNNEST(
            $1::uuid[], $2::uuid[], $3::timestamptz[],
            $4::float8[], $5::float8[], $6::float8[], $7::float8[], $8::float8[]
        )
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(&ids)
    .bind(&device_ids)
    .bind(&recorded_ats)
    .bind(&lats)
    .bind(&lons)
    .bind(&speeds)
    .bind(&batteries)
    .bind(&temps)
    .execute(&state.pool)
    .await;

    let inserted = match inserted {
        Ok(res) => res.rows_affected() as usize,
        Err(err) => {
            state.metrics.ingest_errors.fetch_add(1, Ordering::Relaxed);
            state
                .metrics
                .record(
                    auth.id,
                    &auth.name,
                    "/telemetry/batch",
                    "error",
                    body.events.len() as u32,
                    vec![],
                    err.to_string(),
                )
                .await;
            return Err(AppError::from(err));
        }
    };

    let duplicates = body.events.len().saturating_sub(inserted);
    state
        .metrics
        .ingested
        .fetch_add(inserted as u64, Ordering::Relaxed);
    state
        .metrics
        .duplicates
        .fetch_add(duplicates as u64, Ordering::Relaxed);
    state.metrics.batch_flushes.fetch_add(1, Ordering::Relaxed);
    state
        .metrics
        .batch_events
        .fetch_add(body.events.len() as u64, Ordering::Relaxed);

    // Twin = latest by recorded_at in this batch.
    if let Some(latest) = body.events.iter().max_by_key(|e| e.recorded_at) {
        mark_device_online(&state.pool, auth.id).await?;
        upsert_twin(
            &state.pool,
            &TwinUpdate {
                device_id: auth.id,
                recorded_at: latest.recorded_at,
                latitude: latest.latitude,
                longitude: latest.longitude,
                speed_kmh: latest.speed_kmh,
                battery_pct: latest.battery_pct,
                engine_temp_c: latest.engine_temp_c,
            },
        )
        .await?;
    }

    let mut alerts_created = Vec::new();
    // Alert only on newest event in batch (keeps alert volume sane under load).
    if let Some(latest) = body.events.iter().max_by_key(|e| e.recorded_at) {
        if inserted > 0 {
            alerts_created = maybe_create_alerts(&state, auth.id, latest.id, latest).await?;
            state
                .metrics
                .alerts_created
                .fetch_add(alerts_created.len() as u64, Ordering::Relaxed);
        }
    }

    let outcome = if inserted == 0 {
        "duplicate_ignored"
    } else if duplicates > 0 {
        "partial"
    } else {
        "created"
    };
    state
        .metrics
        .record(
            auth.id,
            &auth.name,
            "/telemetry/batch",
            outcome,
            body.events.len() as u32,
            alerts_created.clone(),
            format!("accepted={inserted} duplicates={duplicates}"),
        )
        .await;

    let status = if inserted > 0 {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };

    Ok((
        status,
        Json(BatchResponse {
            accepted: inserted,
            duplicates,
            alerts_created,
        }),
    ))
}

pub fn validate(body: &TelemetryRequest) -> Result<(), AppError> {
    if !(-90.0..=90.0).contains(&body.latitude) {
        return Err(AppError::BadRequest(
            "latitude must be between -90 and 90".into(),
        ));
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
    let thresholds = crate::settings::load_thresholds(&state.pool).await?;
    let mut created = Vec::new();

    if body.engine_temp_c > thresholds.engine_temp_alert_c {
        let kind = "high_engine_temp";
        let message = format!(
            "engine_temp_c={} above threshold {}",
            body.engine_temp_c, thresholds.engine_temp_alert_c
        );
        insert_alert(state, device_id, Some(telemetry_id), kind, &message).await?;
        created.push(kind.to_string());
    }

    if body.battery_pct < thresholds.battery_low_pct {
        let kind = "low_battery";
        let message = format!(
            "battery_pct={} below threshold {}",
            body.battery_pct, thresholds.battery_low_pct
        );
        insert_alert(state, device_id, Some(telemetry_id), kind, &message).await?;
        created.push(kind.to_string());
    }

    Ok(created)
}

pub async fn insert_alert(
    state: &AppState,
    device_id: Uuid,
    telemetry_id: Option<Uuid>,
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
