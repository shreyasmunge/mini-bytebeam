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

/// Scattered Bangalore-area homes (not a straight line).
const PLAYGROUND_HOMES: &[(f64, f64)] = &[
    (13.1989, 77.7068), // airport
    (12.9698, 77.7500), // whitefield
    (12.8452, 77.6602), // electronic city
    (12.9352, 77.6245), // koramangala
    (13.1007, 77.5963), // yelahanka
    (12.9510, 77.5200), // mysore road
    (12.9784, 77.6408), // indiranagar
    (13.0358, 77.5970), // hebbal
    (12.9308, 77.5838), // jayanagar
    (12.9591, 77.6974), // marathahalli
];

fn playground_home(index: usize) -> (f64, f64) {
    PLAYGROUND_HOMES[index % PLAYGROUND_HOMES.len()]
}

#[derive(Clone, Copy, PartialEq)]
enum PlayRole {
    Healthy,
    HeatCycle,
    DropCycle,
}

fn playground_role(index: usize) -> PlayRole {
    // Real ops console feel: most green, ~1 heat, ~1 drop.
    match index % 10 {
        2 => PlayRole::HeatCycle,
        5 => PlayRole::DropCycle,
        _ => PlayRole::Healthy,
    }
}

fn generate_events(count: usize, vehicles: usize) -> Vec<TelemetryRequest> {
    let now = Utc::now();
    (0..count)
        .map(|i| {
            let v = i % vehicles.max(1);
            let (base_lat, base_lon) = playground_home(v);
            // Small per-event jitter so blasts don't stack on one pixel.
            let j = ((i * 17 + v * 31) % 40) as f64;
            TelemetryRequest {
                id: Uuid::new_v4(),
                recorded_at: now,
                latitude: base_lat + (j - 20.0) * 0.00035,
                longitude: base_lon + ((j * 0.7) - 14.0) * 0.00035,
                speed_kmh: 20.0 + ((i % 40) as f64),
                battery_pct: 55.0 + ((v % 35) as f64),
                engine_temp_c: 78.0 + ((v % 18) as f64),
            }
        })
        .collect()
}

/// Live hosted demo: mostly healthy, 1 heat-cycle + 1 drop-cycle, scattered on the map.
pub async fn playground_keepalive(pool: sqlx::PgPool) {
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(5));
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
    }

    let rows: Vec<Row> = sqlx::query_as(
        r#"
        SELECT d.id
        FROM devices d
        INNER JOIN device_state s ON s.device_id = d.id
        WHERE d.name LIKE 'playground-%'
        ORDER BY d.name
        LIMIT 10
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    if rows.is_empty() {
        return Ok(());
    }

    let thresholds = crate::settings::load_thresholds(pool)
        .await
        .map_err(|e| format!("{e:?}"))?;
    let offline_after = thresholds.offline_after_secs.max(15) as u64;
    // Silent long enough for sweeper to mark offline, then stay offline briefly.
    let drop_silent_secs = offline_after + 25;
    let drop_cycle_secs = drop_silent_secs + 80;
    let heat_on_secs: u64 = 35;
    let heat_cycle_secs: u64 = 105;

    let now = Utc::now();
    let t = now.timestamp().max(0) as u64;

    for (i, row) in rows.iter().enumerate() {
        let role = playground_role(i);
        let (home_lat, home_lon) = playground_home(i);

        // Drop cycle: stop ticking so sweeper flips to offline (real case).
        if role == PlayRole::DropCycle {
            let phase = t % drop_cycle_secs;
            if phase >= 80 {
                continue;
            }
        }

        let heating = role == PlayRole::HeatCycle && (t % heat_cycle_secs) < heat_on_secs;

        // Wander around the home area (not a line).
        let tick = (t / 5) + (i as u64) * 13;
        let ang = (tick as f64) * 0.17 + (i as f64) * 1.7;
        let radius = 0.004 + ((i % 5) as f64) * 0.0012;
        let lat = home_lat + ang.sin() * radius;
        let lon = home_lon + ang.cos() * radius * 1.15;

        let speed = 18.0 + ((tick + i as u64) % 32) as f64;
        let mut battery = 62.0 + (((tick / 3) + i as u64) % 28) as f64;
        if battery > 92.0 {
            battery = 68.0;
        }
        let mut temp = 84.0 + (((tick / 2) + i as u64) % 14) as f64 * 0.8;
        if heating {
            temp = 116.0 + ((tick % 4) as f64);
        }

        let twin = TwinUpdate {
            device_id: row.id,
            recorded_at: now,
            latitude: lat,
            longitude: lon,
            speed_kmh: speed,
            battery_pct: battery,
            engine_temp_c: temp,
        };
        upsert_twin(pool, &twin)
            .await
            .map_err(|e| format!("{e:?}"))?;
        mark_device_online(pool, row.id)
            .await
            .map_err(|e| format!("{e:?}"))?;

        if heating {
            let open: Option<(Uuid,)> = sqlx::query_as(
                r#"
                SELECT id FROM alerts
                WHERE device_id = $1 AND kind = 'high_engine_temp' AND acknowledged_at IS NULL
                LIMIT 1
                "#,
            )
            .bind(row.id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
            if open.is_none() {
                let _ = sqlx::query(
                    r#"
                    INSERT INTO alerts (id, device_id, kind, message, telemetry_id)
                    VALUES ($1, $2, 'high_engine_temp', $3, NULL)
                    "#,
                )
                .bind(Uuid::new_v4())
                .bind(row.id)
                .bind(format!(
                    "engine_temp_c={temp} above threshold {}",
                    thresholds.engine_temp_alert_c
                ))
                .execute(pool)
                .await;
            }
        }

        // Back online after a drop — clear offline alerts.
        if role == PlayRole::DropCycle || role == PlayRole::Healthy {
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
