//! Axum Route Handlers
//!
//! HTTP endpoints for KDB signup service.
//!
//! # Endpoints
//!
//! - `GET /health` - Health check (returns JSON status)
//! - `POST /api/v1/signup` - Register new user
//! - `GET /api/v1/verify/:token` - Email verification
//! - `POST /api/v1/resend-verification` - Resend verification email
//! - `GET /api/v1/my-license` - Get license via OAuth token
//!
//! # Framework Compliance
//!
//! - All handlers are async
//! - No blocking operations
//! - Structured JSON responses
//! - UCE34: Q10 route handlers using T1 Atomic capsules
//! - Chaos: All state via capsule atomics, no mutex in handlers

pub mod my_license;
pub mod signup;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;

// Re-export signup module types for convenience
pub use signup::{
    signup_router, AppState, ErrorResponse, ResendRequest, ResendResponse, SignupRequest,
    SignupResponse,
};

// Re-export my_license handler
pub use my_license::{get_my_license, MyLicenseResponse};

/// Health check response
#[derive(Serialize)]
pub struct HealthResponse {
    /// Service status
    pub status: &'static str,
    /// Service version
    pub version: &'static str,
    /// Service name
    pub service: &'static str,
}

/// Health check endpoint
///
/// Returns JSON with service status, version, and name.
///
/// # Response
///
/// ```json
/// {
///   "status": "healthy",
///   "version": "0.1.0",
///   "service": "kdb-signup"
/// }
/// ```
pub async fn health_check() -> impl IntoResponse {
    let response = HealthResponse {
        status: "healthy",
        version: crate::VERSION,
        service: crate::SERVICE_NAME,
    };

    (StatusCode::OK, Json(response))
}

// Signup routes are now implemented in the signup module:
// - signup_handler: POST /api/v1/signup
// - verify_handler: GET /api/v1/verify/:token
// - resend_handler: POST /api/v1/resend-verification
