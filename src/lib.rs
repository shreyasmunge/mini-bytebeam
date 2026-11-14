//! FleetPulse library crate — shared by the API binary and tests.

pub mod auth;
pub mod config;
pub mod db;
pub mod demo;
pub mod devices;
pub mod error;
pub mod fleet;
pub mod health;
pub mod settings;
pub mod telemetry;

use axum::http::{header, HeaderValue, Method};
use axum::Router;
use sqlx::PgPool;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::config::Config;

/// Shared state passed into every Axum handler.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
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
    // Local Vite + Vercel frontend origins.
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(
            |origin: &HeaderValue, _| cors_origin_ok(origin),
        ))
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
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Connect DB, migrate, return ready state.
pub async fn build_state(config: Config) -> Result<AppState, String> {
    let pool = db::connect(&config.database_url).await?;
    db::migrate(&pool).await?;
    Ok(AppState { pool, config })
}
