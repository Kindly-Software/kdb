//! Handlers - KindlyDB Integration for Computational Capsules
//!
//! **Purpose**: Bridge between computational capsules and KindlyDB storage
//! **Architecture**: Zero-overhead handlers wrapping Tier 1-6 capsules
//!
//! # Handlers
//! - **MetricsHandler**: MetricsStreamCapsule (T5) → KindlyDB metrics_stream table
//! - **OAuthHandler**: OAuthSessionCapsule (T1) → KindlyDB oauth_sessions table
//! - **PaymentHandler**: PaymentCapsule256 (T1+T3) → Stripe + KindlyDB payments table
//!
//! # Design Principles
//! - Lockfree: All handlers maintain capsule performance characteristics
//! - Async: Non-blocking KindlyDB/Stripe integration via async/await
//! - Zero-copy: Minimize allocations in hot paths
//! - Prometheus: Export metrics to Prometheus text format
//! - Idempotent: Webhook handling with hash-based deduplication

pub mod metrics_handler;
pub mod metrics_endpoint;

#[cfg(feature = "oauth")]
pub mod oauth_handler;

#[cfg(feature = "payments")]
pub mod payment_handler;

pub use metrics_handler::{MetricsHandler, MetricType, MetricEntry};
pub use metrics_endpoint::{MetricsEndpointState, MetricsRateLimiter};

#[cfg(feature = "oauth")]
pub use oauth_handler::{OAuthHandler, OAuthError, OAuthResult, OAuthMetrics};

#[cfg(feature = "payments")]
pub use payment_handler::{PaymentHandler, StripeConfig, PaymentRequest, PaymentResponse};

// P3-E7: Health check handler (Kubernetes liveness/readiness probes)
pub mod health_handler;
pub use health_handler::{health_routes, HealthQuery, HealthResponse, DeepHealthResponse};
