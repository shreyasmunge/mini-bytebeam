//! Turn app problems into clear HTTP JSON responses.
//!
//! Interview tip: always map errors to status codes on purpose
//! (401 vs 400 vs 503) instead of returning 500 for everything.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    /// Missing or wrong device API key
    Unauthorized(String),
    /// Bad JSON / out-of-range numbers / missing fields
    BadRequest(String),
    /// Resource does not exist
    NotFound(String),
    /// Database is down or query failed in an unexpected way
    Database(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Database(msg) => {
                tracing::error!("database error: {}", msg);
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "database error".to_string(),
                )
            }
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

/// Helper so `?` works on sqlx errors inside handlers.
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Database(err.to_string())
    }
}
