//! KDB Signup Service
//!
//! Axum-based signup service for KDB Hobby Tier.
//! Built with UCE34/Chaos principles - 100% lockfree, no mutex.
//!
//! # Architecture
//!
//! - **capsules/**: Computational capsules (T1 Atomic tier)
//! - **db/**: KindlyDB client integration
//! - **email/**: Email validation and sending (Resend API)
//! - **routes/**: Axum route handlers
//!
//! # Framework Compliance
//!
//! - UCE34: Q10 tier selection, Q33 lockfree atomics
//! - Chaos: 100% computational capsules, zero mutex
//! - T28: 5-tier testing strategy
//! - ASSUM: All assumptions documented

/// Service version
pub const VERSION: &str = "0.1.0";

/// Service name for tracing
pub const SERVICE_NAME: &str = "kdb-signup";

// Module re-exports
pub mod capsules;
pub mod db;
pub mod email;
pub mod metrics;
pub mod routes;

// Re-export commonly used types
pub use routes::health_check;
