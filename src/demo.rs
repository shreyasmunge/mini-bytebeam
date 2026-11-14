//! Demo helpers — reset old demo devices so the map stays clean.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::delete;
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::error::AppError;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/demo/reset", delete(reset_demo))
}

/// DELETE /demo/reset — remove devices whose name starts with `demo-`
/// (and their telemetry/alerts). Safe for local demos.
async fn reset_demo(State(state): State<AppState>) -> Result<(StatusCode, Json<Value>), AppError> {
    // Order matters because of foreign keys.
    sqlx::query(
        r#"
        DELETE FROM alerts
        WHERE device_id IN (SELECT id FROM devices WHERE name LIKE 'demo-%')
        "#,
    )
    .execute(&state.pool)
    .await?;

    sqlx::query(
        r#"
        DELETE FROM telemetry_events
        WHERE device_id IN (SELECT id FROM devices WHERE name LIKE 'demo-%')
        "#,
    )
    .execute(&state.pool)
    .await?;

    let deleted = sqlx::query(
        r#"
        DELETE FROM devices WHERE name LIKE 'demo-%'
        "#,
    )
    .execute(&state.pool)
    .await?
    .rows_affected();

    Ok((
        StatusCode::OK,
        Json(json!({ "deleted_demo_devices": deleted })),
    ))
}
