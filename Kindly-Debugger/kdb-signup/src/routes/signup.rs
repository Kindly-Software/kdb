//! Signup Routes - Axum handlers for user signup flow
//!
//! Implements the following endpoints:
//! - `POST /api/v1/signup` - Register new user
//! - `GET /api/v1/verify/{token}` - Verify email token
//! - `POST /api/v1/resend-verification` - Resend verification email
//!
//! # Framework Compliance
//!
//! - UCE34: Q10 route handlers using T1 Atomic capsules
//! - Chaos: All state via capsule atomics, no mutex in handlers
//! - T28: Comprehensive error handling and status codes
//! - ASSUM: All assumptions documented

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::capsules::{
    EmailVerificationCapsule, LicenseGeneratorCapsule, SignupError, SubscriptionTier,
    UserRegistrationCapsule, VerificationError,
};
use crate::db::{compute_audit_hash, KindlyDbClient, SignupAuditEntry, User};
use crate::email::{validate_email, EmailError, ResendClient};

// ============================================================================
// AppState
// ============================================================================

/// Application state shared across all handlers
///
/// Contains all capsules and services needed for signup flow.
/// Wrapped in `Arc` for cheap cloning across handlers.
pub struct AppState {
    /// User registration capsule (T1 Atomic, 256B)
    pub registration: UserRegistrationCapsule,
    /// Email verification capsule (T1 Atomic, 256B)
    pub verification: EmailVerificationCapsule,
    /// License generator capsule (T1 Atomic, 512B)
    pub license_gen: LicenseGeneratorCapsule,
    /// Email sender (Resend API client)
    pub email_sender: Option<ResendClient>,
    /// KindlyDB client for persistent storage (optional for graceful degradation)
    pub db_client: Option<KindlyDbClient>,
    /// Ed25519 signing key (32 bytes)
    pub signing_key: [u8; 32],
    /// Base URL for verification links (e.g., "https://api.kindly.software")
    pub base_url: String,
    /// From email address for sending emails
    pub from_email: String,
}

impl AppState {
    /// Create new AppState with all capsules initialized
    ///
    /// # Arguments
    /// * `signing_key` - Ed25519 private key for license signing
    /// * `base_url` - Base URL for verification links
    /// * `from_email` - From email address
    pub fn new(signing_key: [u8; 32], base_url: String, from_email: String) -> Self {
        Self {
            registration: UserRegistrationCapsule::new(),
            verification: EmailVerificationCapsule::new(),
            license_gen: LicenseGeneratorCapsule::new(),
            email_sender: ResendClient::new().ok(),
            db_client: KindlyDbClient::from_env().ok(),
            signing_key,
            base_url,
            from_email,
        }
    }

    /// Create AppState with custom email sender and DB client (for testing)
    #[cfg(test)]
    pub fn new_with_sender(
        signing_key: [u8; 32],
        base_url: String,
        from_email: String,
        email_sender: Option<ResendClient>,
    ) -> Self {
        Self {
            registration: UserRegistrationCapsule::new(),
            verification: EmailVerificationCapsule::new(),
            license_gen: LicenseGeneratorCapsule::new(),
            email_sender,
            db_client: None, // No DB in tests by default
            signing_key,
            base_url,
            from_email,
        }
    }
}

// ============================================================================
// Request/Response Types
// ============================================================================

/// Signup request body
#[derive(Debug, Deserialize)]
pub struct SignupRequest {
    /// User's email address
    pub email: String,
    /// Organization name
    pub org_name: String,
}

/// Successful signup response
#[derive(Debug, Serialize)]
pub struct SignupResponse {
    /// Status code: "verification_sent"
    pub status: String,
    /// Human-readable message
    pub message: String,
}

/// Resend verification request body
#[derive(Debug, Deserialize)]
pub struct ResendRequest {
    /// User's email address
    pub email: String,
}

/// Resend verification response
#[derive(Debug, Serialize)]
pub struct ResendResponse {
    /// Status code: "sent" or "rate_limited"
    pub status: String,
    /// Human-readable message
    pub message: String,
}

/// Error response body
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Human-readable error message
    pub error: String,
    /// Error code for programmatic handling
    pub code: String,
}

impl ErrorResponse {
    /// Create a new error response
    pub fn new(error: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            code: code.into(),
        }
    }
}

// ============================================================================
// Router
// ============================================================================

/// Create the signup router with all endpoints
///
/// # Endpoints
/// - `POST /api/v1/signup` - Register new user
/// - `GET /api/v1/verify/:token` - Verify email token
/// - `POST /api/v1/resend-verification` - Resend verification email
/// - `GET /api/v1/my-license` - Get user's license via OAuth token
pub fn signup_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/signup", post(signup_handler))
        .route("/api/v1/verify/:token", get(verify_handler))
        .route("/api/v1/resend-verification", post(resend_handler))
        .route("/api/v1/my-license", get(super::my_license::get_my_license))
}

// ============================================================================
// Handlers
// ============================================================================

/// POST /api/v1/signup - Register new user
///
/// # Flow
/// 1. Parse JSON body
/// 2. Check disposable email -> 400 BadRequest
/// 3. Check rate limit (UserRegistrationCapsule) -> 429 TooManyRequests
/// 4. Register user -> get PendingUser with email_hash
/// 5. Generate verification token (EmailVerificationCapsule)
/// 6. Send verification email (ResendClient)
/// 7. Return 201 Created with SignupResponse
///
/// # Errors
/// - 400 BadRequest: Invalid email format or disposable email
/// - 429 TooManyRequests: Rate limit exceeded
/// - 500 InternalServerError: Email sending failed
pub async fn signup_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<SignupRequest>,
) -> Response {
    // Step 1 & 2: Validate email format and check disposable
    if let Err(e) = validate_email(&payload.email) {
        let (error_msg, code) = match e {
            EmailError::InvalidFormat(email) => {
                (format!("Invalid email format: {}", email), "INVALID_EMAIL")
            }
            EmailError::DisposableEmail(email) => {
                (format!("Disposable emails not allowed: {}", email), "DISPOSABLE_EMAIL")
            }
            _ => ("Email validation failed".to_string(), "EMAIL_ERROR"),
        };

        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(error_msg, code)),
        )
            .into_response();
    }

    // Extract client IP for rate limiting from X-Forwarded-For header
    // Fly.io/Cloudflare adds real client IP to this header
    let client_ip = extract_client_ip(&headers);

    // Step 3: Check rate limit
    if !state.registration.check_rate_limit(&client_ip) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorResponse::new(
                "Rate limit exceeded. Please try again later.",
                "RATE_LIMITED",
            )),
        )
            .into_response();
    }

    // Step 3b: Check if email has been seen recently (duplicate prevention)
    let email_hash = hash_email(&payload.email);
    if state.registration.is_email_seen(email_hash) {
        tracing::info!(email_hash = email_hash, "Duplicate signup attempt detected in capsule");
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse::new(
                "This email is already registered. Please check your inbox for the verification email or use the resend option.",
                "EMAIL_ALREADY_REGISTERED",
            )),
        )
            .into_response();
    }

    // Step 4: Register user
    let pending_user = match state.registration.register(&payload.email, &payload.org_name, &client_ip) {
        Ok(user) => user,
        Err(e) => {
            let (status, error_msg, code) = match e {
                SignupError::RateLimitExceeded => (
                    StatusCode::TOO_MANY_REQUESTS,
                    "Rate limit exceeded. Please try again later.".to_string(),
                    "RATE_LIMITED",
                ),
                SignupError::InvalidEmail => (
                    StatusCode::BAD_REQUEST,
                    "Invalid email format".to_string(),
                    "INVALID_EMAIL",
                ),
                SignupError::DisposableEmail => (
                    StatusCode::BAD_REQUEST,
                    "Disposable emails not allowed".to_string(),
                    "DISPOSABLE_EMAIL",
                ),
                SignupError::EmailAlreadyRegistered => (
                    StatusCode::CONFLICT,
                    "This email is already registered".to_string(),
                    "EMAIL_ALREADY_REGISTERED",
                ),
            };
            return (status, Json(ErrorResponse::new(error_msg, code))).into_response();
        }
    };

    // Record email as seen for future duplicate detection
    state.registration.record_email_seen(pending_user.email_hash);

    // Step 5: Generate verification token
    let verification_token = match state.verification.generate_token(pending_user.email_hash) {
        Ok(token) => token,
        Err(e) => {
            tracing::error!("Failed to generate verification token: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "Failed to generate verification token",
                    "TOKEN_GENERATION_FAILED",
                )),
            )
                .into_response();
        }
    };

    // Step 5b: Persist user to KindlyDB (optional - graceful degradation)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Token expires in 24 hours
    let token_expires_at = now + 86400;

    if let Some(ref db_client) = state.db_client {
        let user = User {
            id: 0, // Auto-assigned by server
            email_hash: pending_user.email_hash,
            email_encrypted: payload.email.clone(), // TODO: Encrypt in production
            email_verified: false,
            verification_token: Some(verification_token.token.clone()),
            verification_expires_at: Some(token_expires_at),
            tier: 0, // Hobby tier
            license_key: None,
            org_name: payload.org_name.clone(),
            is_promo: false, // Will be set during verification
            created_at: now,
            updated_at: now,
        };

        match db_client.create_user(&user).await {
            Ok(user_id) => {
                tracing::info!(user_id = user_id, email_hash = pending_user.email_hash, "User created in KindlyDB");

                // Q34: Log SIGNUP audit entry
                let audit_entry = SignupAuditEntry {
                    id: 0, // Auto-assigned
                    user_id,
                    event_type: "SIGNUP".to_string(),
                    ip_address: client_ip.to_string(),
                    prev_hash: 0, // First entry in chain
                    timestamp: now,
                };
                if let Err(e) = db_client.log_audit(&audit_entry).await {
                    tracing::warn!("Failed to log SIGNUP audit: {}", e);
                }
            }
            Err(crate::db::DbError::AlreadyExists) => {
                tracing::info!(email_hash = pending_user.email_hash, "User already exists in DB - rejecting duplicate signup");

                // Record in capsule for fast rejection of future duplicates
                state.registration.record_email_seen(pending_user.email_hash);

                // Return 409 Conflict with helpful message
                return (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse::new(
                        "This email is already registered. Please check your inbox for the verification email, or use the resend option.",
                        "EMAIL_ALREADY_REGISTERED",
                    )),
                )
                    .into_response();
            }
            Err(e) => {
                tracing::warn!("Failed to persist user to KindlyDB (graceful degradation): {}", e);
                // Continue without DB persistence - capsule state is authoritative during degradation
            }
        }
    } else {
        tracing::warn!("No KindlyDB client configured - user not persisted");
    }

    // Step 6: Send verification email
    if let Some(ref email_sender) = state.email_sender {
        // Log the verification URL for debugging
        tracing::info!(
            "Sending verification email to {} with token {}",
            payload.email,
            verification_token.token
        );

        // Send email using ResendClient (async)
        if let Err(e) = email_sender
            .send_verification_email(&payload.email, &verification_token.token, &payload.org_name)
            .await
        {
            tracing::error!("Failed to send verification email: {}", e);
            // Don't fail the request - user can resend
        }
    } else {
        // No email sender configured - log token for testing
        tracing::warn!(
            "No email sender configured. Verification token: {}",
            verification_token.token
        );
    }

    // Step 7: Return success
    (
        StatusCode::CREATED,
        Json(SignupResponse {
            status: "verification_sent".to_string(),
            message: format!(
                "Verification email sent to {}. Please check your inbox.",
                payload.email
            ),
        }),
    )
        .into_response()
}

/// GET /api/v1/verify/:token - Verify email token
///
/// # Flow
/// 1. Extract token from path
/// 2. Look up user by token in KindlyDB
/// 3. Verify token validity (EmailVerificationCapsule)
/// 4. If expired -> Redirect to /expired
/// 5. If invalid -> 400 BadRequest
/// 6. Generate license (LicenseGeneratorCapsule)
/// 7. Persist verified status and license to KindlyDB
/// 8. Log VERIFIED audit entry (Q34)
/// 9. Send license email (ResendClient)
/// 10. Redirect to https://www.kindly.software/#verified?license={license_key}
///
/// # Errors
/// - 400 BadRequest: Invalid token format
/// - Redirect to /expired: Token expired
pub async fn verify_handler(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> Response {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Step 2: Look up user by verification token in KindlyDB
    // We need user data for email, org_name, etc.
    let (user_id, user_email, org_name, email_hash) = if let Some(ref db_client) = state.db_client {
        // Find user by token - we need to search since token is stored with user
        // For now, hash token to get email_hash (existing behavior)
        // In production, we'd have a token->user_id index
        let email_hash = hash_token_to_email(&token);

        match db_client.get_user_by_email_hash(email_hash).await {
            Ok(Some(user)) => {
                // Verify token matches
                if user.verification_token.as_deref() != Some(&token) {
                    tracing::warn!(email_hash = email_hash, "Token mismatch in DB lookup");
                    // Fall through to capsule verification which will also fail
                }

                // Check if user already has a license (prevents duplicate license generation)
                // This handles both: verified users AND race conditions where license was issued but not marked verified
                if let Some(ref license_key) = user.license_key {
                    tracing::info!(
                        user_id = user.id,
                        email_verified = user.email_verified,
                        "User already has license, redirecting to verified page"
                    );
                    let redirect_url = format!(
                        "https://www.kindly.software/#verified?license={}",
                        license_key
                    );
                    return Redirect::temporary(&redirect_url).into_response();
                }

                // Check token expiration
                if let Some(expires_at) = user.verification_expires_at {
                    if now > expires_at {
                        tracing::info!(user_id = user.id, "Verification token expired");
                        return Redirect::temporary("/expired").into_response();
                    }
                }

                (Some(user.id), user.email_encrypted.clone(), user.org_name.clone(), user.email_hash)
            }
            Ok(None) => {
                tracing::warn!(email_hash = email_hash, "User not found in DB for token");
                // Fall back to capsule-only verification
                (None, String::new(), "Verified User".to_string(), email_hash)
            }
            Err(e) => {
                tracing::warn!("DB lookup failed (graceful degradation): {}", e);
                let email_hash = hash_token_to_email(&token);
                (None, String::new(), "Verified User".to_string(), email_hash)
            }
        }
    } else {
        // No DB client - use token hash as email_hash (legacy behavior)
        let email_hash = hash_token_to_email(&token);
        (None, String::new(), "Verified User".to_string(), email_hash)
    };

    // Step 3: Verify token with capsule
    match state.verification.verify_token(&token, email_hash) {
        Ok(()) => {
            // Token is valid format - proceed with license generation
        }
        Err(e) => {
            return match e {
                VerificationError::TokenExpired => {
                    // Redirect to expired page
                    Redirect::temporary("/expired").into_response()
                }
                VerificationError::TooManyAttempts => {
                    (
                        StatusCode::TOO_MANY_REQUESTS,
                        Json(ErrorResponse::new(
                            "Too many verification attempts",
                            "TOO_MANY_ATTEMPTS",
                        )),
                    )
                        .into_response()
                }
                VerificationError::InvalidToken | VerificationError::TokenMismatch => {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse::new("Invalid verification token", "INVALID_TOKEN")),
                    )
                        .into_response()
                }
                VerificationError::RandomError => {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse::new(
                            "Internal error during verification",
                            "INTERNAL_ERROR",
                        )),
                    )
                        .into_response()
                }
            };
        }
    }

    // Step 6: Generate license key
    let license = match state.license_gen.generate_license(
        SubscriptionTier::Hobby,
        &org_name,
        &state.signing_key,
    ) {
        Ok(license) => license,
        Err(e) => {
            tracing::error!("Failed to generate license: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "Failed to generate license",
                    "LICENSE_GENERATION_FAILED",
                )),
            )
                .into_response();
        }
    };

    // Step 7 & 8: Persist verified status and log audit to KindlyDB
    let is_promo = state.license_gen.is_promo_active();
    if let Some(ref db_client) = state.db_client {
        if let Some(uid) = user_id {
            // Set user as verified with license
            if let Err(e) = db_client.set_verified(uid, &license.key, is_promo).await {
                tracing::warn!("Failed to persist verified status: {}", e);
                // Continue - license was generated successfully
            } else {
                tracing::info!(user_id = uid, license_key = %license.key, "User verified in KindlyDB");

                // Q34: Log VERIFIED audit entry with hash chain
                // Get previous audit hash for chain integrity
                let prev_hash = match db_client.get_audit_trail(uid).await {
                    Ok(entries) if !entries.is_empty() => {
                        let last = entries.last().unwrap();
                        compute_audit_hash(last, last.prev_hash)
                    }
                    _ => 0,
                };

                let audit_entry = SignupAuditEntry {
                    id: 0, // Auto-assigned
                    user_id: uid,
                    event_type: "VERIFIED".to_string(),
                    ip_address: "0.0.0.0".to_string(), // TODO: Extract from request
                    prev_hash,
                    timestamp: now,
                };
                if let Err(e) = db_client.log_audit(&audit_entry).await {
                    tracing::warn!("Failed to log VERIFIED audit: {}", e);
                }
            }
        }
    }

    // Step 9: Send license email
    if let Some(ref email_sender) = state.email_sender {
        if !user_email.is_empty() {
            let tier_name = "Hobby";
            let sessions_per_month = if is_promo { u64::MAX } else { 5 };

            if let Err(e) = email_sender
                .send_license_email(&user_email, &license.key, tier_name, sessions_per_month, is_promo)
                .await
            {
                tracing::error!("Failed to send license email: {}", e);
                // Don't fail - user gets license via redirect
            } else {
                tracing::info!(to = %user_email, "License email sent successfully");
            }
        } else {
            tracing::warn!("No email address available - skipping license email");
        }
    }

    // Step 10: Redirect to verified page on kindly.software with license
    let redirect_url = format!("https://www.kindly.software/#verified?license={}", license.key);
    Redirect::temporary(&redirect_url).into_response()
}

/// POST /api/v1/resend-verification - Resend verification email
///
/// # Flow
/// 1. Parse JSON body
/// 2. Check rate limit for resend (1 per minute)
/// 3. Look up existing user by email_hash in KindlyDB
/// 4. Generate new verification token
/// 5. Update token in KindlyDB
/// 6. Log RESEND audit entry (Q34)
/// 7. Send verification email using ResendClient
/// 8. Return ResendResponse
///
/// # Errors
/// - 429 TooManyRequests: Rate limited (too many resend attempts)
/// - 404 NotFound: No pending signup for this email
pub async fn resend_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ResendRequest>,
) -> Response {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Step 1: Validate email format
    if let Err(e) = validate_email(&payload.email) {
        let error_msg = match e {
            EmailError::InvalidFormat(_) => "Invalid email format",
            EmailError::DisposableEmail(_) => "Disposable emails not allowed",
            _ => "Email validation failed",
        };
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(error_msg, "INVALID_EMAIL")),
        )
            .into_response();
    }

    // Step 2: Check rate limit using the registration capsule
    let client_ip = "0.0.0.0"; // TODO: Extract from request headers
    if !state.registration.check_rate_limit(client_ip) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ResendResponse {
                status: "rate_limited".to_string(),
                message: "Too many requests. Please wait before trying again.".to_string(),
            }),
        )
            .into_response();
    }

    // Hash the email to get email_hash
    let email_hash = hash_email(&payload.email);

    // Step 3: Look up existing user by email_hash in KindlyDB
    let (user_id, org_name) = if let Some(ref db_client) = state.db_client {
        match db_client.get_user_by_email_hash(email_hash).await {
            Ok(Some(user)) => {
                // Check if already verified
                if user.email_verified {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse::new(
                            "Email already verified",
                            "ALREADY_VERIFIED",
                        )),
                    )
                        .into_response();
                }
                (Some(user.id), user.org_name)
            }
            Ok(None) => {
                tracing::info!(email_hash = email_hash, "User not found for resend");
                // User not found - still allow resend in case capsule has state
                (None, "User".to_string())
            }
            Err(e) => {
                tracing::warn!("DB lookup failed (graceful degradation): {}", e);
                (None, "User".to_string())
            }
        }
    } else {
        (None, "User".to_string())
    };

    // Step 4: Generate new verification token
    let verification_token = match state.verification.generate_token(email_hash) {
        Ok(token) => token,
        Err(e) => {
            tracing::error!("Failed to generate verification token: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "Failed to generate verification token",
                    "TOKEN_GENERATION_FAILED",
                )),
            )
                .into_response();
        }
    };

    // Token expires in 24 hours
    let token_expires_at = now + 86400;

    // Step 5: Update token in KindlyDB (if user exists)
    if let Some(ref db_client) = state.db_client {
        if let Some(uid) = user_id {
            // Get current user and update token
            if let Ok(Some(mut user)) = db_client.get_user_by_id(uid).await {
                user.verification_token = Some(verification_token.token.clone());
                user.verification_expires_at = Some(token_expires_at);
                user.updated_at = now;

                if let Err(e) = db_client.update_user(&user).await {
                    tracing::warn!("Failed to update user token in DB: {}", e);
                    // Continue - capsule state is authoritative
                } else {
                    tracing::info!(user_id = uid, "Verification token updated in KindlyDB");

                    // Step 6: Log RESEND audit entry (Q34)
                    let prev_hash = match db_client.get_audit_trail(uid).await {
                        Ok(entries) if !entries.is_empty() => {
                            let last = entries.last().unwrap();
                            compute_audit_hash(last, last.prev_hash)
                        }
                        _ => 0,
                    };

                    let audit_entry = SignupAuditEntry {
                        id: 0, // Auto-assigned
                        user_id: uid,
                        event_type: "RESEND".to_string(),
                        ip_address: client_ip.to_string(),
                        prev_hash,
                        timestamp: now,
                    };
                    if let Err(e) = db_client.log_audit(&audit_entry).await {
                        tracing::warn!("Failed to log RESEND audit: {}", e);
                    }
                }
            }
        }
    }

    // Step 7: Send verification email using ResendClient
    if let Some(ref email_sender) = state.email_sender {
        tracing::info!(
            "Resending verification email to {} with token {}",
            payload.email,
            verification_token.token
        );

        if let Err(e) = email_sender
            .send_verification_email(&payload.email, &verification_token.token, &org_name)
            .await
        {
            tracing::error!("Failed to send verification email: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "Failed to send verification email",
                    "EMAIL_SEND_FAILED",
                )),
            )
                .into_response();
        }
    } else {
        tracing::warn!(
            "No email sender configured. Verification token: {}",
            verification_token.token
        );
    }

    // Step 8: Return success
    (
        StatusCode::OK,
        Json(ResendResponse {
            status: "sent".to_string(),
            message: format!(
                "Verification email resent to {}. Please check your inbox.",
                payload.email
            ),
        }),
    )
        .into_response()
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Hash email address using BLAKE3 (returns first 8 bytes as u64)
#[inline]
fn hash_email(email: &str) -> u64 {
    let normalized = email.to_lowercase();
    let hash = blake3::hash(normalized.as_bytes());
    let bytes = hash.as_bytes();
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

/// Hash token to derive an email hash (for verification lookup)
/// In production, this would be replaced by a proper token->email mapping
#[inline]
fn hash_token_to_email(token: &str) -> u64 {
    let hash = blake3::hash(token.as_bytes());
    let bytes = hash.as_bytes();
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

/// Extract client IP from X-Forwarded-For header (Fly.io/Cloudflare) or fallback to "unknown"
///
/// X-Forwarded-For format: "client_ip, proxy1_ip, proxy2_ip"
/// We want the first (leftmost) IP which is the original client
fn extract_client_ip(headers: &HeaderMap) -> String {
    // Try X-Forwarded-For first (standard proxy header)
    if let Some(xff) = headers.get("x-forwarded-for") {
        if let Ok(xff_str) = xff.to_str() {
            // Get first IP in comma-separated list
            if let Some(client_ip) = xff_str.split(',').next() {
                let trimmed = client_ip.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }

    // Try CF-Connecting-IP (Cloudflare specific)
    if let Some(cf_ip) = headers.get("cf-connecting-ip") {
        if let Ok(ip_str) = cf_ip.to_str() {
            let trimmed = ip_str.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    // Try Fly-Client-IP (Fly.io specific)
    if let Some(fly_ip) = headers.get("fly-client-ip") {
        if let Ok(ip_str) = fly_ip.to_str() {
            let trimmed = ip_str.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    // Fallback
    "unknown".to_string()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
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
        signup_router().with_state(state)
    }

    #[tokio::test]
    async fn test_signup_success() {
        let app = create_test_app();

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/signup")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"email": "test@example.com", "org_name": "Test Org"}"#,
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn test_signup_invalid_email() {
        let app = create_test_app();

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/signup")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"email": "not-an-email", "org_name": "Test Org"}"#,
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_signup_disposable_email() {
        let app = create_test_app();

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/signup")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"email": "test@mailinator.com", "org_name": "Test Org"}"#,
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_verify_invalid_token() {
        let app = create_test_app();

        let request = Request::builder()
            .method("GET")
            .uri("/api/v1/verify/invalid-token!!!")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_resend_verification() {
        let app = create_test_app();

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/resend-verification")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email": "test@example.com"}"#))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_resend_invalid_email() {
        let app = create_test_app();

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/resend-verification")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"email": "not-an-email"}"#))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_hash_email() {
        let hash1 = hash_email("Test@Example.com");
        let hash2 = hash_email("test@example.com");

        // Should be case-insensitive
        assert_eq!(hash1, hash2);

        // Different emails should have different hashes
        let hash3 = hash_email("other@example.com");
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_app_state_creation() {
        let state = AppState::new(
            TEST_SIGNING_KEY,
            "http://localhost".to_string(),
            "test@test.com".to_string(),
        );

        assert_eq!(state.registration.generation(), 0);
        assert_eq!(state.verification.generation(), 0);
        assert_eq!(state.license_gen.generation(), 0);
    }

    #[test]
    fn test_error_response_creation() {
        let error = ErrorResponse::new("Test error", "TEST_CODE");
        assert_eq!(error.error, "Test error");
        assert_eq!(error.code, "TEST_CODE");
    }

    #[test]
    fn test_signup_request_deserialization() {
        let json = r#"{"email": "test@example.com", "org_name": "Acme Corp"}"#;
        let request: SignupRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.email, "test@example.com");
        assert_eq!(request.org_name, "Acme Corp");
    }

    #[test]
    fn test_signup_response_serialization() {
        let response = SignupResponse {
            status: "verification_sent".to_string(),
            message: "Check your email".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("verification_sent"));
        assert!(json.contains("Check your email"));
    }

    #[test]
    fn test_resend_request_deserialization() {
        let json = r#"{"email": "resend@example.com"}"#;
        let request: ResendRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.email, "resend@example.com");
    }

    #[test]
    fn test_resend_response_serialization() {
        let response = ResendResponse {
            status: "sent".to_string(),
            message: "Email sent".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("sent"));
        assert!(json.contains("Email sent"));
    }
}
