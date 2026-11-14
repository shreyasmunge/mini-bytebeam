//! Red-zone threshold settings (editable from the frontend form).

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/settings/thresholds",
        get(get_thresholds).put(put_thresholds),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Thresholds {
    pub engine_temp_alert_c: f64,
    pub battery_low_pct: f64,
    pub offline_after_secs: i64,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct ThresholdsUpdate {
    pub engine_temp_alert_c: f64,
    pub battery_low_pct: f64,
    pub offline_after_secs: i64,
}

pub async fn load_thresholds(pool: &sqlx::PgPool) -> Result<Thresholds, AppError> {
    let row = sqlx::query_as::<_, Thresholds>(
        r#"
        SELECT engine_temp_alert_c, battery_low_pct, offline_after_secs, updated_at
        FROM settings
        WHERE id = 1
        "#,
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.unwrap_or(Thresholds {
        engine_temp_alert_c: 110.0,
        battery_low_pct: 15.0,
        offline_after_secs: 60,
        updated_at: None,
    }))
}

async fn get_thresholds(State(state): State<AppState>) -> Result<Json<Thresholds>, AppError> {
    Ok(Json(load_thresholds(&state.pool).await?))
}

async fn put_thresholds(
    State(state): State<AppState>,
    Json(body): Json<ThresholdsUpdate>,
) -> Result<Json<Thresholds>, AppError> {
    if body.battery_low_pct < 0.0 || body.battery_low_pct > 100.0 {
        return Err(AppError::BadRequest(
            "battery_low_pct must be between 0 and 100".into(),
        ));
    }
    if body.engine_temp_alert_c < 0.0 || body.engine_temp_alert_c > 300.0 {
        return Err(AppError::BadRequest(
            "engine_temp_alert_c looks invalid".into(),
        ));
    }
    if body.offline_after_secs < 5 {
        return Err(AppError::BadRequest(
            "offline_after_secs must be at least 5".into(),
        ));
    }

    sqlx::query(
        r#"
        INSERT INTO settings (id, engine_temp_alert_c, battery_low_pct, offline_after_secs, updated_at)
        VALUES (1, $1, $2, $3, now())
        ON CONFLICT (id) DO UPDATE SET
            engine_temp_alert_c = EXCLUDED.engine_temp_alert_c,
            battery_low_pct = EXCLUDED.battery_low_pct,
            offline_after_secs = EXCLUDED.offline_after_secs,
            updated_at = now()
        "#,
    )
    .bind(body.engine_temp_alert_c)
    .bind(body.battery_low_pct)
    .bind(body.offline_after_secs)
    .execute(&state.pool)
    .await?;

    Ok(Json(load_thresholds(&state.pool).await?))
}
