//! Public load-test playground — blast a capped number of events (no API key).
//!
//! POST /playground/ingest
//!   { "count": 100 }
//!   { "count": 200, "vehicles": 20 }
//!   { "events": [ ... ] }
//!
//! Cap comes from PLAYGROUND_MAX_EVENTS (default 500; free hosts should use 100–200).

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use uuid::Uuid;

use crate::auth::{generate_api_key, hash_api_key};
use crate::error::AppError;
use crate::telemetry::{validate, TelemetryRequest};
use crate::twin::{mark_device_online, upsert_twin, TwinUpdate};
use crate::AppState;

const DEFAULT_VEHICLES: usize = 20;
const MAX_VEHICLES: usize = 100;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/playground/ingest",
            post(playground_ingest).get(playground_help),
        )
        .route("/playground", get(playground_help))
}

#[derive(Debug, Deserialize)]
pub struct PlaygroundIngestRequest {
    pub count: Option<usize>,
    pub vehicles: Option<usize>,
    pub events: Option<Vec<TelemetryRequest>>,
}

#[derive(Debug, Serialize)]
pub struct PlaygroundIngestResponse {
    pub accepted: usize,
    pub duplicates: usize,
    pub vehicles_used: usize,
    pub endpoint: String,
    pub max_events: usize,
    pub hint: String,
}

async fn playground_help(State(state): State<AppState>) -> Json<Value> {
    let base = state.config.public_base_url.trim_end_matches('/');
    let max = state.config.playground_max_events;
    Json(json!({
        "endpoint": "POST /playground/ingest",
        "base_url": base,
        "auth": "none — public demo playground",
        "max_events_per_request": max,
        "local_benchmark_note": "Laptop load_fleet: 1000 vehicles, ~28962 events, 0 errors, ~445 msg/s. Free hosted API is capped much lower.",
        "examples": {
            "small": { "count": 100 },
            "medium": { "count": max.min(200), "vehicles": 20 }
        },
        "curl": format!(
            "curl -s -X POST {base}/playground/ingest -H 'content-type: application/json' -d '{{\"count\":100}}'"
        ),
        "watch": format!("{base}/metrics/pipeline")
    }))
}

async fn playground_ingest(
    State(state): State<AppState>,
    Json(body): Json<PlaygroundIngestRequest>,
) -> Result<(StatusCode, Json<PlaygroundIngestResponse>), AppError> {
    let max_events = state.config.playground_max_events.max(1);
    let vehicle_n = body
        .vehicles
        .unwrap_or(DEFAULT_VEHICLES)
        .clamp(1, MAX_VEHICLES);

    let events: Vec<TelemetryRequest> = if let Some(ev) = body.events {
        if ev.is_empty() {
            return Err(AppError::BadRequest("events must not be empty".into()));
        }
        if ev.len() > max_events {
            return Err(AppError::BadRequest(format!(
                "hosted demo max is {max_events} events per request (local laptop tested far higher)"
            )));
        }
        for e in &ev {
            validate(e)?;
        }
        ev
    } else {
        let count = body.count.unwrap_or(0);
        if count == 0 {
            return Err(AppError::BadRequest(
                "provide count (e.g. {\"count\":100}) or events: [...]".into(),
            ));
        }
        if count > max_events {
            return Err(AppError::BadRequest(format!(
                "hosted demo max is {max_events} events per request (local laptop tested ~1000 vehicles / ~445 msg/s)"
            )));
        }
        generate_events(count, vehicle_n)
    };

    let device_ids = ensure_playground_devices(&state, vehicle_n).await?;

    let mut accepted = 0usize;
    let mut duplicates = 0usize;

    for (chunk_idx, chunk) in events.chunks(200).enumerate() {
        let mut ids = Vec::with_capacity(chunk.len());
        let mut devs = Vec::with_capacity(chunk.len());
        let mut ats = Vec::with_capacity(chunk.len());
        let mut lats = Vec::with_capacity(chunk.len());
        let mut lons = Vec::with_capacity(chunk.len());
        let mut speeds = Vec::with_capacity(chunk.len());
        let mut batts = Vec::with_capacity(chunk.len());
        let mut temps = Vec::with_capacity(chunk.len());

        for (i, e) in chunk.iter().enumerate() {
            let global_i = chunk_idx * 200 + i;
            let device_id = device_ids[global_i % device_ids.len()];
            ids.push(e.id);
            devs.push(device_id);
            ats.push(e.recorded_at);
            lats.push(e.latitude);
            lons.push(e.longitude);
            speeds.push(e.speed_kmh);
            batts.push(e.battery_pct);
            temps.push(e.engine_temp_c);
        }

        let res = sqlx::query(
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
        .bind(&devs)
        .bind(&ats)
        .bind(&lats)
        .bind(&lons)
        .bind(&speeds)
        .bind(&batts)
        .bind(&temps)
        .execute(&state.pool)
        .await;

        match res {
            Ok(r) => {
                let n = r.rows_affected() as usize;
                accepted += n;
                duplicates += chunk.len().saturating_sub(n);
            }
            Err(err) => {
                state.metrics.ingest_errors.fetch_add(1, Ordering::Relaxed);
                return Err(AppError::from(err));
            }
        }

        let mut seen = std::collections::HashSet::new();
        for (i, e) in chunk.iter().enumerate().rev() {
            let global_i = chunk_idx * 200 + i;
            let device_id = device_ids[global_i % device_ids.len()];
            if !seen.insert(device_id) {
                continue;
            }
            let _ = mark_device_online(&state.pool, device_id).await;
            let _ = upsert_twin(
                &state.pool,
                &TwinUpdate {
                    device_id,
                    recorded_at: e.recorded_at,
                    latitude: e.latitude,
                    longitude: e.longitude,
                    speed_kmh: e.speed_kmh,
                    battery_pct: e.battery_pct,
                    engine_temp_c: e.engine_temp_c,
                },
            )
            .await;
        }
    }

    state
        .metrics
        .ingested
        .fetch_add(accepted as u64, Ordering::Relaxed);
    state
        .metrics
        .duplicates
        .fetch_add(duplicates as u64, Ordering::Relaxed);
    state.metrics.batch_flushes.fetch_add(1, Ordering::Relaxed);
    state
        .metrics
        .batch_events
        .fetch_add(events.len() as u64, Ordering::Relaxed);

    let playground_id = device_ids.first().copied().unwrap_or_else(Uuid::nil);
    state
        .metrics
        .record(
            playground_id,
            "playground",
            "/playground/ingest",
            if accepted > 0 {
                "created"
            } else {
                "duplicate_ignored"
            },
            events.len() as u32,
            vec![],
            format!(
                "accepted={accepted} duplicates={duplicates} vehicles={}",
                device_ids.len()
            ),
        )
        .await;

    let base = state.config.public_base_url.trim_end_matches('/');
    Ok((
        StatusCode::CREATED,
        Json(PlaygroundIngestResponse {
            accepted,
            duplicates,
            vehicles_used: device_ids.len(),
            endpoint: format!("POST {base}/playground/ingest"),
            max_events,
            hint: format!(
                "Hosted demo capped at {max_events}/request. Local laptop: ~1000 vehicles, ~445 msg/s."
            ),
        }),
    ))
}

fn generate_events(count: usize, vehicles: usize) -> Vec<TelemetryRequest> {
    let now = Utc::now();
    (0..count)
        .map(|i| {
            let v = i % vehicles.max(1);
            TelemetryRequest {
                id: Uuid::new_v4(),
                recorded_at: now,
                latitude: 12.90 + (v as f64) * 0.002,
                longitude: 77.50 + ((v % 20) as f64) * 0.01,
                speed_kmh: 20.0 + ((i % 40) as f64),
                // Stay under red-zone thresholds (battery≥15, temp≤110 by default).
                battery_pct: 55.0 + ((v % 35) as f64),
                engine_temp_c: 78.0 + ((v % 18) as f64),
            }
        })
        .collect()
}

/// Keep hosted playground vehicles online with healthy twin ticks (demo map).
pub async fn playground_keepalive(pool: sqlx::PgPool) {
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(12));
    loop {
        ticker.tick().await;
        if let Err(err) = tick_playground_keepalive(&pool).await {
            tracing::warn!("playground keepalive: {err}");
        }
    }
}

async fn tick_playground_keepalive(pool: &sqlx::PgPool) -> Result<(), String> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        latitude: Option<f64>,
        longitude: Option<f64>,
        battery_pct: Option<f64>,
    }

    let rows: Vec<Row> = sqlx::query_as(
        r#"
        SELECT d.id, s.latitude, s.longitude, s.battery_pct
        FROM devices d
        INNER JOIN device_state s ON s.device_id = d.id
        WHERE d.name LIKE 'playground-%'
        ORDER BY d.name
        LIMIT 100
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    if rows.is_empty() {
        return Ok(());
    }

    let now = Utc::now();
    for (i, row) in rows.iter().enumerate() {
        let base_lat = row.latitude.unwrap_or(12.97);
        let base_lon = row.longitude.unwrap_or(77.59);
        let drift = ((now.timestamp() as usize / 12 + i) % 20) as f64 * 0.00008;
        let battery = (row.battery_pct.unwrap_or(70.0) + 0.2).clamp(55.0, 95.0);
        let temp = 82.0 + ((i % 12) as f64);
        let speed = 25.0 + ((i % 30) as f64);

        let twin = TwinUpdate {
            device_id: row.id,
            recorded_at: now,
            latitude: base_lat + drift,
            longitude: base_lon + drift * 0.6,
            speed_kmh: speed,
            battery_pct: battery,
            engine_temp_c: temp,
        };
        upsert_twin(pool, &twin)
            .await
            .map_err(|e| e.to_string())?;
        mark_device_online(pool, row.id)
            .await
            .map_err(|e| e.to_string())?;

        // Clear stale offline alerts when the demo fleet is breathing again.
        let _ = sqlx::query(
            r#"
            UPDATE alerts
            SET acknowledged_at = now()
            WHERE device_id = $1
              AND kind = 'offline'
              AND acknowledged_at IS NULL
            "#,
        )
        .bind(row.id)
        .execute(pool)
        .await;
    }

    Ok(())
}

async fn ensure_playground_devices(
    state: &AppState,
    n: usize,
) -> Result<Vec<Uuid>, AppError> {
    let existing: Vec<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT id FROM devices
        WHERE name LIKE 'playground-%'
        ORDER BY name
        LIMIT $1
        "#,
    )
    .bind(n as i64)
    .fetch_all(&state.pool)
    .await?;

    let mut ids: Vec<Uuid> = existing.into_iter().map(|r| r.0).collect();

    while ids.len() < n {
        let i = ids.len() + 1;
        let id = Uuid::new_v4();
        let name = format!("playground-{i:03}");
        let key = generate_api_key();
        let hash = hash_api_key(&key);
        sqlx::query(
            r#"
            INSERT INTO devices (id, name, api_key_hash, status)
            VALUES ($1, $2, $3, 'offline')
            "#,
        )
        .bind(id)
        .bind(&name)
        .bind(&hash)
        .execute(&state.pool)
        .await?;
        ids.push(id);
    }

    Ok(ids)
}
