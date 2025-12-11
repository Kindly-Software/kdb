//! OAuth 2.0 Authentication Module
//!
//! Provides lockfree, T1 Atomic capsules for OAuth 2.0 authentication flows:
//! - CSRF protection via state parameter storage
//! - RFC 7636 PKCE (Proof Key for Code Exchange) support
//! - Google OAuth 2.0 client for token exchange and user info (feature-gated)
//!
//! ## Capsules
//!
//! - [`OAuthStateCapsule`]: T1 Atomic state storage for CSRF and PKCE (4KB, 256 slots)
//! - [`GoogleOAuthClientCapsule`]: T1 Atomic Google OAuth client (512B, 64B-aligned) [google-oauth feature]
//! - [`OAuthUserMappingCapsule`]: T1 Atomic OAuth-to-user-ID mapping (16KB, 1024 slots) [google-oauth feature]
//!
//! ## UCE35 Compliance
//!
//! - Q10: T1 Atomic tier (lockfree hash tables)
//! - Q23: 100% lockfree (no mutex/RwLock)
//! - Q33: 64B-aligned capsules with generation counters
//! - Q34: Audit trail for state creation/validation/expiration
//!
//! ## Security Features
//!
//! - **CSRF Protection**: State parameter prevents cross-site request forgery
//! - **PKCE S256**: SHA-256 code challenge prevents authorization code interception
//! - **TTL Enforcement**: States automatically expire (default 10 minutes)
//! - **Constant-Time Comparison**: Prevents timing attacks on hash comparison
//! - **ID Token Validation**: JWT validation for Google ID tokens [google-oauth feature]

pub mod state_capsule;

#[cfg(feature = "google-oauth")]
pub mod google_client;

// User mapping and authorization codes available with oauth feature
#[cfg(feature = "oauth")]
pub mod user_mapping;

#[cfg(feature = "oauth")]
pub mod authorization_codes;

// Re-exports for state capsule (always available)
pub use state_capsule::{
    fnv1a_hash,
    fnv1a_hash_bytes,
    CodeChallengeMethod,
    OAuthStateCapsule,
    OAuthStateError,
    OAuthStateSlot,
    OAuthStateStats,
    StoredStateData,
};

// Google OAuth re-exports (feature-gated)
#[cfg(feature = "google-oauth")]
pub use google_client::{
    GoogleOAuthClientCapsule,
    GoogleTokenResponse,
    GoogleUserInfo,
    IdTokenClaims,
    GoogleTokenError,
    GoogleOAuthError,
    OAuthMetrics,
    GOOGLE_AUTH_URL,
    GOOGLE_TOKEN_URL,
    GOOGLE_USERINFO_URL,
    GOOGLE_JWKS_URL,
    GOOGLE_SCOPES,
    GOOGLE_ISSUER,
    GOOGLE_ISSUER_ALT,
};

// User mapping re-exports (oauth feature)
#[cfg(feature = "oauth")]
pub use user_mapping::{
    OAuthUserCapsule,
    OAuthUserError,
    OAuthUserStats,
    fnv1a_hash_oauth,
    USER_TABLE_SLOTS,
};

// Authorization code re-exports (oauth feature)
#[cfg(feature = "oauth")]
pub use authorization_codes::{
    AuthorizationCodeCapsule,
    AuthCodeError,
    AuthCodeStats,
    fnv1a_hash_code,
    sha256_to_fnv,
    generate_secure_code,
    CODE_TABLE_SLOTS,
    CODE_TTL_SECS,
};
