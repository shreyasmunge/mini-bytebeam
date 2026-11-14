//! Device authentication using Bearer API keys.
//!
//! Flow:
//! 1. Device sends: Authorization: Bearer <api_key>
//! 2. We hash the key the same way as registration
//! 3. Look up device by api_key_hash
//! 4. If found → request may continue; else → 401

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

/// Hash a plaintext API key with SHA-256 → hex string.
/// Same input always gives the same hash (needed for lookup).
pub fn hash_api_key(api_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(api_key.as_bytes());
    hex::encode(hasher.finalize())
}

/// Create a random API key string the device will store.
pub fn generate_api_key() -> String {
    // Prefix makes keys easy to spot in logs / configs.
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
        // 1) Read header
        let header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("missing Authorization header".into()))?;

        // 2) Expect "Bearer <token>"
        let api_key = header
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::Unauthorized("expected Bearer token".into()))?
            .trim();

        if api_key.is_empty() {
            return Err(AppError::Unauthorized("empty API key".into()));
        }

        // 3) Hash + lookup
        let key_hash = hash_api_key(api_key);
        let row = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM devices WHERE api_key_hash = $1",
        )
        .bind(&key_hash)
        .fetch_optional(&state.pool)
        .await?;

        match row {
            Some((id, name)) => Ok(AuthDevice { id, name }),
            None => Err(AppError::Unauthorized("invalid API key".into())),
        }
    }
}

/// Small helper used by tests / simulator docs.
#[allow(dead_code)]
pub async fn find_device_by_key(pool: &PgPool, api_key: &str) -> Result<Option<Uuid>, sqlx::Error> {
    let key_hash = hash_api_key(api_key);
    let row = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM devices WHERE api_key_hash = $1")
        .bind(key_hash)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.0))
}
