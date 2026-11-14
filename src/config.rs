//! App settings loaded from environment variables.

use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub offline_after_secs: i64,
    pub sweep_interval_secs: u64,
    pub engine_temp_alert_c: f64,
    pub battery_low_pct: f64,
    /// Postgres pool size (raise for load tests).
    pub db_max_connections: u32,
    /// Auth cache TTL seconds.
    pub auth_cache_ttl_secs: u64,
    /// Delete raw telemetry older than this many days (after rollup).
    pub telemetry_retain_days: i64,
    /// Public base URL shown in Pipeline docs (e.g. https://api.onrender.com).
    pub public_base_url: String,
    /// Cap for POST /playground/ingest (free hosting should stay small).
    pub playground_max_events: usize,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://fleetpulse:fleetpulse@localhost:5433/fleetpulse".to_string()
            }),
            host: env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            // Render/Heroku set PORT; local uses API_PORT.
            port: env::var("PORT")
                .or_else(|_| env::var("API_PORT"))
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3000),
            offline_after_secs: env::var("OFFLINE_AFTER_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(60),
            sweep_interval_secs: env::var("SWEEP_INTERVAL_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10),
            engine_temp_alert_c: env::var("ENGINE_TEMP_ALERT_C")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(110.0),
            battery_low_pct: env::var("BATTERY_LOW_PCT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(15.0),
            db_max_connections: env::var("DB_MAX_CONNECTIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(40),
            auth_cache_ttl_secs: env::var("AUTH_CACHE_TTL_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(300),
            telemetry_retain_days: env::var("TELEMETRY_RETAIN_DAYS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(14),
            public_base_url: env::var("PUBLIC_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string()),
            playground_max_events: env::var("PLAYGROUND_MAX_EVENTS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(500),
        }
    }
}
