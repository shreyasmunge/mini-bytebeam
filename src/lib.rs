//! FleetPulse library crate — shared by the API binary and tests.

pub mod auth;
pub mod auth_cache;
pub mod config;
pub mod db;
pub mod demo;
pub mod devices;
pub mod error;
pub mod fleet;
pub mod health;
pub mod metrics;
pub mod playground;
pub mod settings;
pub mod telemetry;
pub mod twin;

use axum::http::{header, HeaderValue, Method};
use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::auth_cache::AuthCache;
use crate::config::Config;
use crate::metrics::PipelineMetrics;

/// Shared state passed into every Axum handler.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub auth_cache: AuthCache,
    pub metrics: Arc<PipelineMetrics>,
}

fn cors_origin_ok(origin: &HeaderValue) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    origin.starts_with("http://127.0.0.1:")
        || origin.starts_with("http://localhost:")
        || origin.ends_with(".vercel.app")
        || origin == "https://vercel.app"
}

/// Build the HTTP router (all routes in one place).
pub fn app_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
            cors_origin_ok(origin)
        }))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    Router::new()
        .merge(health::router())
        .merge(devices::router())
        .merge(telemetry::router())
        .merge(fleet::router())
        .merge(settings::router())
        .merge(demo::router())
        .merge(playground::router())
        .merge(metrics::router())
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Connect DB, migrate, return ready state.
pub async fn build_state(config: Config) -> Result<AppState, String> {
    let pool = db::connect(&config.database_url, config.db_max_connections).await?;
    db::migrate(&pool).await?;
    let metrics = PipelineMetrics::new();
    metrics.mark_started();
    Ok(AppState {
        pool,
        auth_cache: AuthCache::new(config.auth_cache_ttl_secs),
        metrics,
        config,
    })
}
