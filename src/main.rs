//! FleetPulse API entry point.
//!
//! Steps on startup:
//! 1. Load config from env
//! 2. Connect to Postgres + run migrations
//! 3. Start offline sweeper in the background
//! 4. Listen for HTTP on host:port

use mini_bytebeam::config::Config;
use mini_bytebeam::health::offline_sweeper;
use mini_bytebeam::{app_router, build_state};

#[tokio::main]
async fn main() {
    // Logs: RUST_LOG=info cargo run
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .init();

    let config = Config::from_env();
    tracing::info!("database_url host parsed from env; binding {}:{}", config.host, config.port);

    let state = build_state(config.clone())
        .await
        .expect("failed to connect/migrate database");

    // Clone pool/config for the background task
    tokio::spawn(offline_sweeper(state.pool.clone(), state.config.clone()));

    let app = app_router(state);
    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind address");

    tracing::info!("FleetPulse listening on http://{}", addr);
    axum::serve(listener, app).await.expect("server error");
}
