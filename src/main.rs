//! FleetPulse API entry point.

use mini_bytebeam::config::Config;
use mini_bytebeam::health::{offline_sweeper, retention_worker};
use mini_bytebeam::{app_router, build_state};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .init();

    let config = Config::from_env();
    tracing::info!(
        "binding {}:{} | db_pool={} | auth_cache_ttl={}s | retain_days={}",
        config.host,
        config.port,
        config.db_max_connections,
        config.auth_cache_ttl_secs,
        config.telemetry_retain_days
    );

    let state = build_state(config.clone())
        .await
        .expect("failed to connect/migrate database");

    tokio::spawn(offline_sweeper(
        state.pool.clone(),
        state.config.clone(),
        state.metrics.clone(),
    ));
    tokio::spawn(retention_worker(
        state.pool.clone(),
        state.config.telemetry_retain_days,
    ));

    let app = app_router(state);
    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind address");

    tracing::info!("FleetPulse listening on http://{}", addr);
    axum::serve(listener, app).await.expect("server error");
}
