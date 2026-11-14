//! Device authentication using Bearer API keys + in-memory cache.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::atomic::Ordering;
use uuid::Uuid;

use crate::error::AppError;

/// Hash a plaintext API key with SHA-256 → hex string.
pub fn hash_api_key(api_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(api_key.as_bytes());
    hex::encode(hasher.finalize())
}

/// Create a random API key string the device will store.
pub fn generate_api_key() -> String {
    format!("fp_live_{}", Uuid::new_v4().simple())
}

/// Attached to handlers that need a logged-in device.
#[derive(Debug, Clone)]
pub struct AuthDevice {
    pub id: Uuid,
    pub name: String,
}

impl FromRequestParts<crate::AppState> for AuthDevice {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &crate::AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("missing Authorization header".into()))?;

        let api_key = header
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::Unauthorized("expected Bearer token".into()))?
            .trim();

        if api_key.is_empty() {
            return Err(AppError::Unauthorized("empty API key".into()));
        }

        let key_hash = hash_api_key(api_key);

        if let Some(device) = state.auth_cache.get(&key_hash).await {
            state
                .metrics
                .auth_cache_hits
                .fetch_add(1, Ordering::Relaxed);
            return Ok(device);
        }

        state
            .metrics
            .auth_cache_misses
            .fetch_add(1, Ordering::Relaxed);

        let row = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM devices WHERE api_key_hash = $1",
        )
        .bind(&key_hash)
        .fetch_optional(&state.pool)
        .await?;

        match row {
            Some((id, name)) => {
                let device = AuthDevice { id, name };
                state.auth_cache.put(key_hash, device.clone()).await;
                Ok(device)
            }
            None => Err(AppError::Unauthorized("invalid API key".into())),
        }
    }
}

#[allow(dead_code)]
pub async fn find_device_by_key(pool: &PgPool, api_key: &str) -> Result<Option<Uuid>, sqlx::Error> {
    let key_hash = hash_api_key(api_key);
    let row = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM devices WHERE api_key_hash = $1")
        .bind(key_hash)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.0))
}
