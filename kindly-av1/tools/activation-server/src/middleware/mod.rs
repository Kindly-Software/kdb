//! Middleware Module - HTTP Request Processing Pipeline
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! ## Architecture
//!
//! Middleware chain for RapidAPI-compatible request processing:
//! 1. RapidAPI authentication (X-RapidAPI-Key extraction)
//! 2. Rate limiting (AdaptiveRateLimiterCapsule)
//! 3. Quota tracking (QuotaTrackerCapsule)
//! 4. Request routing
//!
//! ## Modules
//!
//! - `rapidapi` - RapidAPI header authentication and tier detection
//! - `rate_limit` - Adaptive rate limiting with token bucket + EWMA
//!
//! ## Framework Compliance
//!
//! - UCE34 Q11: 100% Rust implementation
//! - Chaos: Lockfree state (atomic_capsule primitives)
//! - ASSUM: All security assumptions documented
//! - T28: Comprehensive middleware testing

pub mod rapidapi;
pub mod rate_limit;

pub use rapidapi::{RapidApiAuth, RapidApiMiddleware, SubscriptionTier};
pub use rate_limit::{RateLimitMiddleware, RateLimitResult};
