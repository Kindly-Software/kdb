// [TRADE SECRET] API error types and handling

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// API Result type
pub type ApiResult<T> = Result<T, ApiError>;

/// API errors
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    #[error("Missing header: {0}")]
    MissingHeader(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("JSON error: {0}")]
    JsonError(String),

    #[error("Early adopter licenses sold out")]
    EarlyAdopterSoldOut,

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Internal server error")]
    InternalError,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ApiError::InvalidSignature(msg) => (StatusCode::UNAUTHORIZED, msg),
            ApiError::MissingHeader(msg) => (StatusCode::BAD_REQUEST, format!("Missing header: {}", msg)),
            ApiError::InvalidRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::JsonError(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::EarlyAdopterSoldOut => (
                StatusCode::CONFLICT,
                "Early adopter licenses sold out. Please use regular pricing.".to_string(),
            ),
            ApiError::ConfigError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ApiError::DatabaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ApiError::InternalError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
        };

        let body = Json(json!({
            "error": error_message,
            "status": status.as_u16()
        }));

        (status, body).into_response()
    }
}

// Convert common error types
impl From<serde_json::Error> for ApiError {
    fn from(err: serde_json::Error) -> Self {
        ApiError::JsonError(err.to_string())
    }
}

#[cfg(feature = "sqlite")]
impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        ApiError::DatabaseError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_messages() {
        let err = ApiError::InvalidSignature("test".to_string());
        assert_eq!(err.to_string(), "Invalid signature: test");
    }
}
