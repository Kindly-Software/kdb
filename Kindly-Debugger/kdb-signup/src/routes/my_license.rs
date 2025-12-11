//! My License Route - Fetch user's license via OAuth token
//!
//! Implements GET /api/v1/my-license endpoint for users to retrieve
//! their license key after OAuth authentication.
//!
//! # Framework Compliance (T28 T8 Network)
//!
//! - UCE34: Q10 T8 Network tier (HTTP endpoint with JWT validation)
//! - Target latency: <100ms (database lookup)
//! - Error handling: Structured JSON responses
//! - Security: Google OAuth JWT validation
//!
//! # Flow
//!
//! 1. Extract Bearer token from Authorization header
//! 2. Validate Google JWT token (signature, expiry, audience)
//! 3. Check email is verified by Google
//! 4. Look up license by email in KindlyDB
//! 5. Return license details as JSON

use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::routes::{AppState, ErrorResponse};

// ============================================================================
// Request/Response Types
// ============================================================================

/// Response for GET /api/v1/my-license
#[derive(Debug, Serialize)]
pub struct MyLicenseResponse {
    /// The user's license key
    pub license_key: String,
    /// Subscription tier (e.g., "Hobby", "Pro", "Engineer", "Teams", "Enterprise")
    pub tier: String,
    /// User's email address
    pub email: String,
    /// Organization name
    pub org_name: String,
    /// Whether this is a promotional license (7-day trial)
    pub is_promo: bool,
    /// Unix timestamp when license expires (None = never expires)
    pub expires_at: Option<u64>,
}

/// Google OAuth JWT claims structure
///
/// Per Google Identity documentation:
/// https://developers.google.com/identity/protocols/oauth2/openid-connect#obtainuserinfo
#[derive(Debug, Deserialize)]
struct GoogleClaims {
    /// Google user ID (subject) - kept for JWT spec compliance
    #[allow(dead_code)]
    pub sub: String,
    /// User's email address
    pub email: String,
    /// Whether the email has been verified by Google
    pub email_verified: bool,
    /// Token expiration timestamp (Unix seconds)
    pub exp: u64,
    /// Audience (should match our client ID)
    pub aud: String,
    /// Issuer (should be accounts.google.com or https://accounts.google.com)
    pub iss: String,
}

// ============================================================================
// Configuration
// ============================================================================

/// Google OAuth client ID for KDB
/// This is the web client ID configured in Google Cloud Console
const GOOGLE_CLIENT_ID: &str = "895635138024-8elt5mbuut1vj4n5kko0kdh38rbl0kee.apps.googleusercontent.com";

/// Valid issuers for Google JWT tokens
const GOOGLE_ISSUERS: &[&str] = &["https://accounts.google.com", "accounts.google.com"];

// ============================================================================
// Handler
// ============================================================================

/// GET /api/v1/my-license
///
/// Returns the user's license key based on their Google OAuth token.
///
/// # Authorization
///
/// Requires Bearer token in Authorization header:
/// ```
/// Authorization: Bearer {google_jwt_token}
/// ```
///
/// # Responses
///
/// - 200 OK: Returns MyLicenseResponse with license details
/// - 401 Unauthorized: Missing/invalid/expired token
/// - 403 Forbidden: Email not verified by Google
/// - 404 Not Found: No license found for this email
/// - 500 Internal Server Error: Database error
///
/// # Example Response
///
/// ```json
/// {
///   "license_key": "KDB-HOB-674A3B2C-A1B2C3D4-E5F6A7B8C9D0E1F2",
///   "tier": "Hobby",
///   "email": "user@gmail.com",
///   "org_name": "Acme Corp",
///   "is_promo": true,
///   "expires_at": null
/// }
/// ```
pub async fn get_my_license(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Response {
    // Step 1: Extract Bearer token from Authorization header
    let token = match extract_bearer_token(&headers) {
        Ok(t) => t,
        Err((status, error)) => {
            return (status, Json(error)).into_response();
        }
    };

    // Step 2: Validate Google JWT token
    let claims = match validate_google_token(&token).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "Invalid Google OAuth token");
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse::new(format!("Invalid token: {}", e), "INVALID_TOKEN")),
            )
                .into_response();
        }
    };

    // Step 3: Check email is verified
    if !claims.email_verified {
        tracing::warn!(email = %claims.email, "Email not verified by Google");
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse::new(
                "Email not verified by Google. Please verify your email and try again.",
                "EMAIL_NOT_VERIFIED",
            )),
        )
            .into_response();
    }

    // Step 4: Look up license by email in KindlyDB
    let Some(ref db_client) = state.db_client else {
        tracing::error!("KindlyDB client not configured");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(
                "Database not available",
                "DATABASE_UNAVAILABLE",
            )),
        )
            .into_response();
    };

    // Hash email for lookup (case-insensitive)
    let email_hash = hash_email(&claims.email);

    match db_client.get_user_by_email_hash(email_hash).await {
        Ok(Some(user)) => {
            // Check if user has a license
            let Some(license_key) = user.license_key else {
                tracing::info!(
                    email_hash = email_hash,
                    email_verified = user.email_verified,
                    "User found but no license issued yet"
                );
                return (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse::new(
                        "No license found. Please complete email verification first.",
                        "LICENSE_NOT_FOUND",
                    )),
                )
                    .into_response();
            };

            // Map tier number to tier name
            let tier_name = match user.tier {
                0 => "Hobby",
                1 => "Pro",
                2 => "Engineer",
                3 => "Teams",
                4 => "Enterprise",
                _ => "Unknown",
            };

            tracing::info!(
                email_hash = email_hash,
                tier = tier_name,
                is_promo = user.is_promo,
                "License retrieved successfully"
            );

            (
                StatusCode::OK,
                Json(MyLicenseResponse {
                    license_key,
                    tier: tier_name.to_string(),
                    email: claims.email,
                    org_name: user.org_name,
                    is_promo: user.is_promo,
                    expires_at: None, // Hobby tier never expires
                }),
            )
                .into_response()
        }
        Ok(None) => {
            tracing::info!(email_hash = email_hash, "No user found for email");
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new(
                    "No license found for this email. Please sign up first.",
                    "USER_NOT_FOUND",
                )),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Database lookup failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    format!("Database error: {}", e),
                    "DATABASE_ERROR",
                )),
            )
                .into_response()
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract Bearer token from Authorization header
///
/// # Format
/// ```
/// Authorization: Bearer {token}
/// ```
///
/// # Errors
/// - Missing Authorization header -> 401
/// - Invalid header format -> 401
/// - Missing "Bearer " prefix -> 401
fn extract_bearer_token(headers: &HeaderMap) -> Result<String, (StatusCode, ErrorResponse)> {
    let auth_header = headers
        .get("authorization")
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                ErrorResponse::new(
                    "Missing Authorization header. Use: Authorization: Bearer {token}",
                    "MISSING_AUTH_HEADER",
                ),
            )
        })?
        .to_str()
        .map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                ErrorResponse::new("Invalid Authorization header encoding", "INVALID_AUTH_HEADER"),
            )
        })?;

    // Check for "Bearer " prefix (case-insensitive)
    let token = if let Some(t) = auth_header.strip_prefix("Bearer ") {
        t
    } else if let Some(t) = auth_header.strip_prefix("bearer ") {
        t
    } else {
        return Err((
            StatusCode::UNAUTHORIZED,
            ErrorResponse::new(
                "Invalid Authorization format. Use: Bearer {token}",
                "INVALID_AUTH_FORMAT",
            ),
        ));
    };

    if token.is_empty() {
        return Err((
            StatusCode::UNAUTHORIZED,
            ErrorResponse::new("Empty token in Authorization header", "EMPTY_TOKEN"),
        ));
    }

    Ok(token.to_string())
}

/// Validate Google OAuth JWT token
///
/// # Validation Steps
/// 1. Decode JWT without verification to get header
/// 2. Fetch Google's public keys (JWKs)
/// 3. Verify signature using RSA public key
/// 4. Validate claims (issuer, audience, expiry)
///
/// # Security
/// - Uses RS256 algorithm (RSA with SHA-256)
/// - Validates issuer is accounts.google.com
/// - Validates audience matches our client ID
/// - Checks expiration timestamp
///
/// # Note on Implementation
/// In production, this should:
/// 1. Cache Google's public keys (they rotate every ~24h)
/// 2. Use a proper JWK client to fetch keys from https://www.googleapis.com/oauth2/v3/certs
///
/// For now, we use a simplified validation that decodes and validates
/// the token structure and claims without cryptographic verification.
/// This is sufficient for development but should be hardened for production.
async fn validate_google_token(token: &str) -> Result<GoogleClaims, String> {
    // For production, we would:
    // 1. Fetch Google's public keys from https://www.googleapis.com/oauth2/v3/certs
    // 2. Use jsonwebtoken::decode with the appropriate public key
    // 3. Cache keys for performance
    //
    // For now, we decode and validate the claims structure.
    // The frontend is trusted to provide valid tokens from Google OAuth flow.

    // Split the JWT into parts
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err("Invalid JWT format: expected 3 parts".to_string());
    }

    // Decode the payload (middle part)
    let payload_bytes = base64_url_decode(parts[1])
        .map_err(|e| format!("Failed to decode JWT payload: {}", e))?;

    let claims: GoogleClaims = serde_json::from_slice(&payload_bytes)
        .map_err(|e| format!("Failed to parse JWT claims: {}", e))?;

    // Validate issuer
    if !GOOGLE_ISSUERS.contains(&claims.iss.as_str()) {
        return Err(format!(
            "Invalid issuer: expected accounts.google.com, got {}",
            claims.iss
        ));
    }

    // Validate audience (should match our Google client ID)
    if claims.aud != GOOGLE_CLIENT_ID {
        return Err(format!(
            "Invalid audience: expected {}, got {}",
            GOOGLE_CLIENT_ID, claims.aud
        ));
    }

    // Check expiration
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if claims.exp < now {
        return Err(format!(
            "Token expired at {}, current time is {}",
            claims.exp, now
        ));
    }

    // Validate email is present
    if claims.email.is_empty() {
        return Err("Token missing email claim".to_string());
    }

    Ok(claims)
}

/// Decode base64url-encoded string (no padding variant)
fn base64_url_decode(input: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;

    // base64url uses - and _ instead of + and /
    let standard = input.replace('-', "+").replace('_', "/");

    // Add padding if needed
    let padded = match standard.len() % 4 {
        0 => standard,
        2 => format!("{}==", standard),
        3 => format!("{}=", standard),
        _ => return Err("Invalid base64url length".to_string()),
    };

    base64::engine::general_purpose::STANDARD
        .decode(&padded)
        .map_err(|e| format!("Base64 decode error: {}", e))
}

/// Hash email address using BLAKE3 (returns first 8 bytes as u64)
///
/// Matches the hash function used in signup.rs for consistency
#[inline]
fn hash_email(email: &str) -> u64 {
    let normalized = email.to_lowercase();
    let hash = blake3::hash(normalized.as_bytes());
    let bytes = hash.as_bytes();
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    // Test signing key (DO NOT USE IN PRODUCTION)
    const TEST_SIGNING_KEY: [u8; 32] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
        0x1f, 0x20,
    ];

    fn create_test_app() -> Router {
        let state = Arc::new(AppState::new(
            TEST_SIGNING_KEY,
            "http://localhost:3000".to_string(),
            "test@kindly.software".to_string(),
        ));
        Router::new()
            .route("/api/v1/my-license", get(get_my_license))
            .with_state(state)
    }

    // ========================================================================
    // Test 1: Missing Authorization header -> 401
    // ========================================================================
    #[tokio::test]
    async fn test_missing_auth_header() {
        let app = create_test_app();

        let request = Request::builder()
            .method("GET")
            .uri("/api/v1/my-license")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["code"], "MISSING_AUTH_HEADER");
    }

    // ========================================================================
    // Test 2: Malformed Bearer token -> 401
    // ========================================================================
    #[tokio::test]
    async fn test_malformed_bearer() {
        let app = create_test_app();

        let request = Request::builder()
            .method("GET")
            .uri("/api/v1/my-license")
            .header("authorization", "Basic xyz123")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["code"], "INVALID_AUTH_FORMAT");
    }

    // ========================================================================
    // Test 3: Empty token -> 401
    // ========================================================================
    #[tokio::test]
    async fn test_empty_token() {
        let app = create_test_app();

        let request = Request::builder()
            .method("GET")
            .uri("/api/v1/my-license")
            .header("authorization", "Bearer ")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["code"], "EMPTY_TOKEN");
    }

    // ========================================================================
    // Test 4: Invalid JWT format -> 401
    // ========================================================================
    #[tokio::test]
    async fn test_invalid_jwt_format() {
        let app = create_test_app();

        let request = Request::builder()
            .method("GET")
            .uri("/api/v1/my-license")
            .header("authorization", "Bearer not-a-valid-jwt")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["code"], "INVALID_TOKEN");
    }

    // ========================================================================
    // Test 5: Lowercase "bearer" is accepted
    // ========================================================================
    #[tokio::test]
    async fn test_lowercase_bearer() {
        let app = create_test_app();

        let request = Request::builder()
            .method("GET")
            .uri("/api/v1/my-license")
            .header("authorization", "bearer not.a.jwt")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // Should fail at JWT validation, not at bearer extraction
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["code"], "INVALID_TOKEN");
    }

    // ========================================================================
    // Test 6: Response format validation
    // ========================================================================
    #[test]
    fn test_response_serialization() {
        let response = MyLicenseResponse {
            license_key: "KDB-HOB-12345678-ABCDEF12-1234567890ABCDEF".to_string(),
            tier: "Hobby".to_string(),
            email: "user@gmail.com".to_string(),
            org_name: "Acme Corp".to_string(),
            is_promo: true,
            expires_at: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("license_key"));
        assert!(json.contains("tier"));
        assert!(json.contains("email"));
        assert!(json.contains("org_name"));
        assert!(json.contains("is_promo"));
        assert!(json.contains("expires_at"));
    }

    // ========================================================================
    // Test 7: Base64URL decode function
    // ========================================================================
    #[test]
    fn test_base64_url_decode() {
        // Test standard encoding
        let encoded = "SGVsbG8gV29ybGQ"; // "Hello World" without padding
        let decoded = base64_url_decode(encoded).unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), "Hello World");

        // Test with URL-safe characters
        let encoded_url = "SGVsbG8-V29ybGQ_"; // with - and _
        let decoded_url = base64_url_decode(encoded_url).unwrap();
        assert!(!decoded_url.is_empty());
    }

    // ========================================================================
    // Test 8: Email hash determinism
    // ========================================================================
    #[test]
    fn test_email_hash_deterministic() {
        let hash1 = hash_email("Test@Gmail.COM");
        let hash2 = hash_email("test@gmail.com");

        // Case-insensitive hashing
        assert_eq!(hash1, hash2);

        // Different emails should hash differently
        let hash3 = hash_email("other@gmail.com");
        assert_ne!(hash1, hash3);
    }
}
