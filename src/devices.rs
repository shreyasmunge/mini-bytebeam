//! Device registration, lookup, and API key rotation.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{generate_api_key, hash_api_key, AuthDevice};
use crate::error::AppError;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/devices", get(list_devices).post(register_device))
        .route("/devices/{id}", get(get_device))
        .route("/devices/{id}/rotate-key", post(rotate_key))
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub id: Uuid,
    pub name: String,
    /// Shown ONLY once at registration. Store it on the device.
    pub api_key: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct RotateKeyResponse {
    pub id: Uuid,
    pub api_key: String,
    pub rotated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct DevicePublic {
    pub id: Uuid,
    pub name: String,
    pub status: String,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub status: Option<String>,
    pub q: Option<String>,
}

async fn list_devices(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<DevicePublic>>, AppError> {
    let status = query.status.unwrap_or_else(|| "all".into());
    let q = query.q.unwrap_or_default();
    let pattern = if q.trim().is_empty() {
        "%%".to_string()
    } else {
        format!("%{}%", q.trim())
    };

    let rows = sqlx::query_as::<_, DevicePublic>(
        r#"
        SELECT id, name, status, last_seen_at, created_at
        FROM devices
        WHERE ($1 = 'all' OR status = $1)
          AND name ILIKE $2
        ORDER BY name
        "#,
    )
    .bind(&status)
    .bind(&pattern)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows))
}

async fn register_device(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<RegisterResponse>), AppError> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("name must not be empty".into()));
    }
    if name.len() > 100 {
        return Err(AppError::BadRequest("name too long (max 100)".into()));
    }

    let id = Uuid::new_v4();
    let api_key = generate_api_key();
    let api_key_hash = hash_api_key(&api_key);

    sqlx::query(
        r#"
        INSERT INTO devices (id, name, api_key_hash, status)
        VALUES ($1, $2, $3, 'offline')
        "#,
    )
    .bind(id)
    .bind(name)
    .bind(&api_key_hash)
    .execute(&state.pool)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            id,
            name: name.to_string(),
            api_key,
            status: "offline".to_string(),
        }),
    ))
}

async fn get_device(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<DevicePublic>, AppError> {
    let device = sqlx::query_as::<_, DevicePublic>(
        r#"
        SELECT id, name, status, last_seen_at, created_at
        FROM devices
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("device not found".into()))?;

    Ok(Json(device))
}

/// POST /devices/:id/rotate-key — requires current Bearer key; returns new key once.
async fn rotate_key(
    State(state): State<AppState>,
    auth: AuthDevice,
    Path(id): Path<Uuid>,
) -> Result<Json<RotateKeyResponse>, AppError> {
    if auth.id != id {
        return Err(AppError::Unauthorized(
            "you can only rotate your own API key".into(),
        ));
    }

    let api_key = generate_api_key();
    let api_key_hash = hash_api_key(&api_key);
    let rotated_at = Utc::now();

    sqlx::query(
        r#"
        UPDATE devices
        SET api_key_hash = $2, api_key_rotated_at = $3
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(&api_key_hash)
    .bind(rotated_at)
    .execute(&state.pool)
    .await?;

    state.auth_cache.invalidate_device(id).await;

    Ok(Json(RotateKeyResponse {
        id,
        api_key,
        rotated_at,
    }))
}
