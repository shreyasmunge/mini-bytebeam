//! App settings loaded from environment variables.
//!
//! Why env vars? So Docker Compose and local runs can change ports/passwords
//! without editing code.

use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    /// Postgres connection string, e.g. postgres://user:pass@localhost:5432/db
    pub database_url: String,
    /// Host to bind, usually 0.0.0.0 inside Docker
    pub host: String,
    /// HTTP port, default 3000
    pub port: u16,
    /// If no telemetry for this many seconds → device becomes offline
    pub offline_after_secs: i64,
    /// How often the background job checks for offline devices
    pub sweep_interval_secs: u64,
    /// Engine temp above this → create an alert
    pub engine_temp_alert_c: f64,
    /// Battery below this → create an alert
    pub battery_low_pct: f64,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                // Default uses host port 5433 (see docker-compose.yml)
                "postgres://fleetpulse:fleetpulse@localhost:5433/fleetpulse".to_string()
            }),
            host: env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("API_PORT")
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
        }
    }
}
