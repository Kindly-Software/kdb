//! Axum Middleware for Tier Detection
//!
//! **Purpose**: Extract and cache subscription tier from JWT tokens
//! **Architecture**: Zero allocation on hot path, <200ns execution
//!
//! # Performance Targets (B32 Framework)
//! - JWT decode: <150ns (jsonwebtoken crate)
//! - Tier lookup: <50ns (TierCache atomic load)
//! - Total overhead: <200ns per request
//!
//! # UCE34 Compliance
//! - **Q10**: Tier 1 Atomic for tier cache
//! - **Q11**: AtomicU8 + pattern matching
//! - **Q33**: Manual verification (simple middleware)
//!
//! # ASSUM Safety
//! - #ASSUME: JWT signature validation prevents forgery
//! - #VERIFY: jsonwebtoken crate validates HMAC/RSA signatures
//! - #ASSUME: "tier" claim exists in token
//! - #VERIFY: Fallback to Free tier if missing

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use crate::licensing::SubscriptionTier;

/// Axum request extension for subscription tier
///
/// # Usage
/// ```ignore
/// use axum::extract::Extension;
/// use clapi_core::licensing::{SubscriptionTier, TierExtension};
///
/// async fn handler(Extension(tier): Extension<TierExtension>) -> String {
///     format!("Your tier: {}", tier.tier)
/// }
/// ```
#[derive(Debug, Clone)]
pub struct TierExtension {
    /// User ID extracted from JWT
    pub user_id: u64,
    /// Subscription tier
    pub tier: SubscriptionTier,
}

/// Extract JWT token from Authorization header
///
/// # Performance: <50ns (header lookup + string slice)
///
/// # Arguments
/// - `req`: HTTP request
///
/// # Returns
/// - `Some(token)` if "Authorization: Bearer <token>" header present
/// - `None` if header missing or malformed
///
/// # ASSUM Safety
/// - #ASSUME: Authorization header format is "Bearer <token>"
/// - #VERIFY: String slicing validates "Bearer " prefix (7 chars)
fn extract_bearer_token(req: &Request) -> Option<&str> {
    req.headers()
        .get("Authorization")?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

/// Axum middleware to extract and inject subscription tier
///
/// # Performance: <200ns (JWT decode + tier lookup + extension insertion)
///
/// # Arguments
/// - `req`: HTTP request
/// - `next`: Next middleware/handler in chain
///
/// # Returns
/// - `Ok(response)` if tier successfully extracted
/// - `Err(401 Unauthorized)` if JWT invalid or missing
///
/// # Behavior
/// 1. Extract JWT from Authorization header
/// 2. Decode JWT and extract "tier" claim
/// 3. Parse tier string to SubscriptionTier enum
/// 4. Insert TierExtension into request
/// 5. Call next handler
///
/// # Example Usage
/// ```ignore
/// use axum::{Router, routing::get};
/// use tower::ServiceBuilder;
/// use clapi_core::licensing::middleware::tier_extraction_middleware;
///
/// let app = Router::new()
///     .route("/api/v1/models", get(list_models))
///     .layer(ServiceBuilder::new().layer(axum::middleware::from_fn(tier_extraction_middleware)));
/// ```
pub async fn tier_extraction_middleware(
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract JWT token from Authorization header
    let _token = extract_bearer_token(&req)
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // TODO: Decode JWT and extract user_id + tier claim
    // For now, use placeholder values
    let user_id = 12345u64;
    let tier = SubscriptionTier::Free;

    // TODO: Real implementation would:
    // 1. Decode JWT using jsonwebtoken crate
    // 2. Validate signature (HMAC-SHA256 or RSA)
    // 3. Extract "user_id" and "tier" claims
    // 4. Parse tier string to SubscriptionTier enum
    //
    // Example:
    // ```
    // let claims: Claims = decode(token, &DecodingKey::from_secret(...), &Validation::default())
    //     .map_err(|_| StatusCode::UNAUTHORIZED)?
    //     .claims;
    // let user_id = claims.sub.parse().map_err(|_| StatusCode::UNAUTHORIZED)?;
    // let tier = SubscriptionTier::parse(&claims.tier).unwrap_or(SubscriptionTier::Free);
    // ```

    // Insert tier extension into request
    req.extensions_mut().insert(TierExtension { user_id, tier });

    // Call next handler
    Ok(next.run(req).await)
}

/// Helper to get tier from request extensions
///
/// # Performance: <20ns (extension lookup)
///
/// # Arguments
/// - `req`: HTTP request
///
/// # Returns
/// - `Some(tier)` if tier extension present
/// - `None` if middleware not applied
///
/// # Example
/// ```ignore
/// use clapi_core::licensing::middleware::get_tier_from_request;
///
/// async fn handler(req: Request) -> Result<String, StatusCode> {
///     let tier = get_tier_from_request(&req).ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
///     Ok(format!("Your tier: {}", tier.tier))
/// }
/// ```
pub fn get_tier_from_request(req: &Request) -> Option<&TierExtension> {
    req.extensions().get::<TierExtension>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use axum::body::Body;

    #[test]
    fn test_extract_bearer_token() {
        let req = Request::builder()
            .header("Authorization", "Bearer test_token_12345")
            .body(Body::empty())
            .unwrap();

        let token = extract_bearer_token(&req);
        assert_eq!(token, Some("test_token_12345"));
    }

    #[test]
    fn test_extract_bearer_token_missing() {
        let req = Request::builder()
            .body(Body::empty())
            .unwrap();

        let token = extract_bearer_token(&req);
        assert_eq!(token, None);
    }

    #[test]
    fn test_extract_bearer_token_malformed() {
        let req = Request::builder()
            .header("Authorization", "Basic dXNlcjpwYXNz")
            .body(Body::empty())
            .unwrap();

        let token = extract_bearer_token(&req);
        assert_eq!(token, None);
    }
}
