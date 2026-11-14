//! Device twin (last-known state) — map/fleet reads this, not full history.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct TwinUpdate {
    pub device_id: Uuid,
    pub recorded_at: DateTime<Utc>,
    pub latitude: f64,
    pub longitude: f64,
    pub speed_kmh: f64,
    pub battery_pct: f64,
    pub engine_temp_c: f64,
}

pub async fn upsert_twin(pool: &PgPool, u: &TwinUpdate) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO device_state (
            device_id, latitude, longitude, speed_kmh, battery_pct, engine_temp_c,
            recorded_at, last_seen_at, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, now(), now())
        ON CONFLICT (device_id) DO UPDATE SET
            latitude = EXCLUDED.latitude,
            longitude = EXCLUDED.longitude,
            speed_kmh = EXCLUDED.speed_kmh,
            battery_pct = EXCLUDED.battery_pct,
            engine_temp_c = EXCLUDED.engine_temp_c,
            recorded_at = EXCLUDED.recorded_at,
            last_seen_at = now(),
            updated_at = now()
        WHERE device_state.recorded_at IS NULL
           OR EXCLUDED.recorded_at >= device_state.recorded_at
        "#,
    )
    .bind(u.device_id)
    .bind(u.latitude)
    .bind(u.longitude)
    .bind(u.speed_kmh)
    .bind(u.battery_pct)
    .bind(u.engine_temp_c)
    .bind(u.recorded_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_device_online(pool: &PgPool, device_id: Uuid) -> Result<(), AppError> {
    sqlx::query(
        r#"
        UPDATE devices
        SET status = 'online', last_seen_at = now()
        WHERE id = $1
        "#,
    )
    .bind(device_id)
    .execute(pool)
    .await?;
    Ok(())
}
