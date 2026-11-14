//! In-process pipeline metrics + recent ingest activity feed.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::AppState;

const RECENT_CAP: usize = 80;

#[derive(Debug, Clone, Serialize)]
pub struct IngestActivity {
    pub at: DateTime<Utc>,
    pub device_id: Uuid,
    pub device_name: String,
    pub path: String,
    pub outcome: String,
    pub events: u32,
    pub alerts: Vec<String>,
    pub detail: String,
}

#[derive(Default)]
pub struct PipelineMetrics {
    pub ingested: AtomicU64,
    pub duplicates: AtomicU64,
    pub batch_flushes: AtomicU64,
    pub batch_events: AtomicU64,
    pub alerts_created: AtomicU64,
    pub auth_cache_hits: AtomicU64,
    pub auth_cache_misses: AtomicU64,
    pub ingest_errors: AtomicU64,
    started_at: std::sync::OnceLock<Instant>,
    recent: RwLock<VecDeque<IngestActivity>>,
}

impl PipelineMetrics {
    pub fn new() -> Arc<Self> {
        let m = Arc::new(Self::default());
        let _ = m.started_at.set(Instant::now());
        m
    }

    pub fn mark_started(&self) {
        let _ = self.started_at.set(Instant::now());
    }

    fn uptime_secs(&self) -> u64 {
        self.started_at
            .get()
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0)
            .max(1)
    }

    pub async fn record(
        &self,
        device_id: Uuid,
        device_name: &str,
        path: &str,
        outcome: &str,
        events: u32,
        alerts: Vec<String>,
        detail: impl Into<String>,
    ) {
        let entry = IngestActivity {
            at: Utc::now(),
            device_id,
            device_name: device_name.to_string(),
            path: path.to_string(),
            outcome: outcome.to_string(),
            events,
            alerts,
            detail: detail.into(),
        };
        let mut q = self.recent.write().await;
        if q.len() >= RECENT_CAP {
            q.pop_front();
        }
        q.push_back(entry);
    }

    pub async fn recent_snapshot(&self) -> Vec<IngestActivity> {
        let q = self.recent.read().await;
        q.iter().rev().cloned().collect()
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let ingested = self.ingested.load(Ordering::Relaxed);
        let uptime = self.uptime_secs();
        MetricsSnapshot {
            ingested_total: ingested,
            duplicates_total: self.duplicates.load(Ordering::Relaxed),
            batch_flushes_total: self.batch_flushes.load(Ordering::Relaxed),
            batch_events_total: self.batch_events.load(Ordering::Relaxed),
            alerts_created_total: self.alerts_created.load(Ordering::Relaxed),
            auth_cache_hits_total: self.auth_cache_hits.load(Ordering::Relaxed),
            auth_cache_misses_total: self.auth_cache_misses.load(Ordering::Relaxed),
            ingest_errors_total: self.ingest_errors.load(Ordering::Relaxed),
            uptime_secs: uptime,
            approx_ingest_per_sec: ingested as f64 / uptime as f64,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MetricsSnapshot {
    pub ingested_total: u64,
    pub duplicates_total: u64,
    pub batch_flushes_total: u64,
    pub batch_events_total: u64,
    pub alerts_created_total: u64,
    pub auth_cache_hits_total: u64,
    pub auth_cache_misses_total: u64,
    pub ingest_errors_total: u64,
    pub uptime_secs: u64,
    pub approx_ingest_per_sec: f64,
}

#[derive(Debug, Serialize)]
pub struct TryApiGuide {
    pub base_url: String,
    pub register: EndpointExample,
    pub telemetry: EndpointExample,
    pub batch: EndpointExample,
    pub pipeline: EndpointExample,
    pub last_local_benchmark: String,
}

#[derive(Debug, Serialize)]
pub struct EndpointExample {
    pub method: String,
    pub path: String,
    pub headers: Vec<String>,
    pub body_example: serde_json::Value,
    pub notes: String,
}

#[derive(Debug, Serialize)]
pub struct PipelineStatus {
    pub metrics: MetricsSnapshot,
    pub devices_online: i64,
    pub devices_offline: i64,
    pub open_alerts: i64,
    pub twin_rows: i64,
    pub recent: Vec<IngestActivity>,
    pub try_api: TryApiGuide,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/metrics", get(get_metrics))
        .route("/metrics/pipeline", get(get_pipeline))
        .route("/metrics/recent", get(get_recent))
}

async fn get_metrics(State(state): State<AppState>) -> Json<MetricsSnapshot> {
    Json(state.metrics.snapshot())
}

async fn get_recent(State(state): State<AppState>) -> Json<Vec<IngestActivity>> {
    Json(state.metrics.recent_snapshot().await)
}

fn try_api_guide(base_url: &str, playground_max: usize) -> TryApiGuide {
    let base = base_url.trim_end_matches('/').to_string();
    TryApiGuide {
        base_url: base.clone(),
        register: EndpointExample {
            method: "POST".into(),
            path: "/playground/ingest".into(),
            headers: vec!["Content-Type: application/json".into()],
            body_example: serde_json::json!({ "count": playground_max.min(100) }),
            notes: format!(
                "PUBLIC demo endpoint — no API key. Hosted max {playground_max} events/request. Local laptop tested much higher (~445 msg/s)."
            ),
        },
        telemetry: EndpointExample {
            method: "POST".into(),
            path: "/telemetry".into(),
            headers: vec![
                "Content-Type: application/json".into(),
                "Authorization: Bearer <api_key>".into(),
            ],
            body_example: serde_json::json!({
                "id": "00000000-0000-4000-8000-000000000001",
                "recorded_at": "2026-11-14T10:00:00Z",
                "latitude": 12.97,
                "longitude": 77.59,
                "speed_kmh": 42.0,
                "battery_pct": 78.0,
                "engine_temp_c": 91.0
            }),
            notes: "Per-device Bearer key path.".into(),
        },
        batch: EndpointExample {
            method: "POST".into(),
            path: "/telemetry/batch".into(),
            headers: vec![
                "Content-Type: application/json".into(),
                "Authorization: Bearer <api_key>".into(),
            ],
            body_example: serde_json::json!({
                "events": [{
                    "id": "00000000-0000-4000-8000-000000000002",
                    "recorded_at": "2026-11-14T10:00:01Z",
                    "latitude": 12.97,
                    "longitude": 77.59,
                    "speed_kmh": 40.0,
                    "battery_pct": 70.0,
                    "engine_temp_c": 88.0
                }]
            }),
            notes: "Authenticated batch.".into(),
        },
        pipeline: EndpointExample {
            method: "GET".into(),
            path: "/metrics/pipeline".into(),
            headers: vec![],
            body_example: serde_json::json!({}),
            notes: "Live counters + recent ingest feed.".into(),
        },
        last_local_benchmark: format!(
            "Local laptop: 1000 vehicles × 1 Hz → 28,962 events, 0 errors, ~445 msg/s. Hosted free API demo capped at {playground_max} events/request — not the same scale."
        ),
    }
}

async fn get_pipeline(
    State(state): State<AppState>,
) -> Result<Json<PipelineStatus>, crate::error::AppError> {
    let devices_online: (i64,) =
        sqlx::query_as("SELECT count(*) FROM devices WHERE status = 'online'")
            .fetch_one(&state.pool)
            .await?;
    let devices_offline: (i64,) =
        sqlx::query_as("SELECT count(*) FROM devices WHERE status = 'offline'")
            .fetch_one(&state.pool)
            .await?;
    let open_alerts: (i64,) =
        sqlx::query_as("SELECT count(*) FROM alerts WHERE acknowledged_at IS NULL")
            .fetch_one(&state.pool)
            .await?;
    let twin_rows: (i64,) = sqlx::query_as("SELECT count(*) FROM device_state")
        .fetch_one(&state.pool)
        .await?;

    Ok(Json(PipelineStatus {
        metrics: state.metrics.snapshot(),
        devices_online: devices_online.0,
        devices_offline: devices_offline.0,
        open_alerts: open_alerts.0,
        twin_rows: twin_rows.0,
        recent: state.metrics.recent_snapshot().await,
        try_api: try_api_guide(
            &state.config.public_base_url,
            state.config.playground_max_events,
        ),
    }))
}
