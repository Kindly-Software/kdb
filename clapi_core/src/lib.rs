//! Clapi Core - AI Call Protection Proxy with Computational Capsule Architecture
//!
//! ## Architecture
//!
//! Built on 14 computational capsules (Week 1-4):
//! - **RequestCapsule128** (T1 Atomic): Budget validation, 3-5× speedup
//! - **RoutingCapsule128** (T1 Atomic): Provider selection, 3-8× speedup
//! - **ResponseCapsule256** (T2+T3 SIMD+Fixed-Point): Metrics tracking, 4-12× speedup
//! - **AuditLogEntry128** (T5 Streaming): Audit trail, 10-100× speedup
//! - **EpochTile1024** (T4+T3 Batch+Fixed-Point): Cost aggregation, 10-20× speedup
//! - **BudgetMetaCapsule** (T4+T1 Batch+Atomic): 1M budget management, <20ns operations
//! - **OAuthSessionCapsule** (T1 Atomic): Session management, <50ns verification
//! - **PaymentCapsule256/128** (T3 Fixed-Point): Payment tracking, <150ns creation
//! - **RateLimitCapsule** (T1 Atomic): Token bucket rate limiting, <40ns acquisition
//! - **CompressionStateCapsule** (T4 Batch): Streaming compression state, O(1) latency
//!
//! ## Week 4 Option A: Capsule Optimizations
//! Three independent deterministic optimizations (I20-Capsule integration):
//! - `payment-128`: PaymentCapsule128 (50% memory reduction: 256B → 128B)
//! - `portable_simd`: SIMD percentile queries (2-4× speedup for p50/p95/p99)
//! - `oauth-hash-chain`: OAuth session auditability (hash chain integrity)
//!
//! Build with: `cargo +nightly build --release --features week4-option-a`
//!
//! ## Phase 2.3: Nightly Optimizations (UCE34 Q30-Q32)
//! Enable cutting-edge nightly features for maximum performance:
//! - `nightly-const-fp`: Compile-time fee calculations (0ns runtime)
//! - `nightly-simd`: Vectorized hash operations (2-4× speedup)
//! - `nightly-atomic-from-mut`: Zero-copy atomic initialization (10-50% faster)
//!
//! Build with: `cargo +nightly build --release --features nightly-phase23`

// Phase 2.3 Nightly Features (UCE34 Q30-Q32)
// Note: const_fn_floating_point_arithmetic is stable since Rust 1.82 - no feature gate needed
#![cfg_attr(feature = "nightly-atomic-from-mut", feature(atomic_from_mut))]

// SIMD feature (both nightly-simd and legacy simd use portable_simd)
#![cfg_attr(any(feature = "nightly-simd", feature = "simd"), feature(portable_simd))]
// Note: clippy::missing_capsule_verification requires custom clippy lint (not yet installed)
// #![warn(clippy::missing_capsule_verification)]

pub mod cache;  // LRU cache for request/response deduplication (Week 3: Feature 1, Tier 6 Mixed)
pub mod capsules;
pub mod cli;  // CLI framework (banner, error formatter, commands)
// pub mod compression;  // TEMPORARY: Disabled due to compilation errors
pub mod error;
pub mod licensing;  // Subscription tiers, trials, and retention policies
// pub mod load_balancer;  // TEMPORARY: Disabled (not yet implemented)
pub mod logging;  // E21: Structured logging with tracing integration (P1 enhancements)
pub mod observability;  // Alert system (PagerDuty + Slack) and monitoring
pub mod profiling;  // Latency profiling with SIMD percentile optimization (Week 4)
pub mod proxy;
pub mod client;  // Client SDK utilities (const hash lookups)
pub mod test_mode;  // Mock AI provider for zero-config testing
pub mod test_utils;  // Test utilities (ConcurrentTestBuilder, TimelineFixture) - P1 E7-E8
// pub mod replay_log;  // TEMPORARY: Disabled (not yet implemented)

// Feature-gated modules
#[cfg(feature = "compliance")]
pub mod compliance;  // Compliance export infrastructure (SOX, SOC2, GDPR)

#[cfg(feature = "kindlydb")]
pub mod db;  // KindlyDB integration layer

#[cfg(any(feature = "kindlydb", feature = "oauth", feature = "payments"))]
pub mod handlers;  // KindlyDB handlers

#[cfg(feature = "oauth")]
pub mod auth;  // OAuth2 PKCE authentication

// Week 4 Option A: OAuth hash chain auditability (integrated into OAuthSessionCapsule)
// No separate module needed - hash chain is part of capsule structure

pub mod monitoring;  // Rollout monitoring and alerting
pub mod infrastructure;  // P3-E6/E11: Prometheus metrics, Kubernetes integration
pub mod tui;  // Terminal User Interface with command palette (T1 Atomic)

// Dashboard integration (feature-gated)
#[cfg(feature = "dashboard")]
pub mod dashboard;  // MetricsSource implementation for kindly_dash

// Re-export capsules for convenience
pub use capsules::{
    RequestCapsule128,
    RoutingCapsule128,
    ResponseCapsule256,
    AuditLogEntry128,
    EpochTile1024,
    BudgetMetaCapsule,
    BudgetMetaCapsuleHeader,
    MetaCapsuleStats,
    MAX_BUDGET_SLOTS,
    ProviderState,
    AuditEntry,
    EventType,
    EpochSnapshot,
    ProviderSnapshot,
};

// Re-export error types
pub use error::{ClapiError, ClapiResult};

// Re-export licensing types
pub use licensing::{SubscriptionTier, TierCache, TrialCapsule, RetentionPolicy};

// Re-export observability types
pub use observability::{Alert, AlertLevel, AlertSystem};

#[cfg(feature = "kindlydb")]
pub use licensing::{CleanupCoordinator, run_cleanup_task};

// Re-export proxy types
pub use proxy::{
    ProxyServer,
    ProxyConfig,
    ProviderConfig,
    ChatCompletionRequest,
    ChatCompletionResponse,
    Message,
    Choice,
    Usage,
};
