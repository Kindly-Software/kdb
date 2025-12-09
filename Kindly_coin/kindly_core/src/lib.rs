//! # Kindly Core - Atomic Capsule Cryptocurrency Primitives
//!
//! Core atomic capsule primitives for Kindly Coin cryptocurrency.
//!
//! ## Design Principles
//!
//! - **100% Lockfree**: No mutex/RwLock in any code path
//! - **Atomic Capsules**: Single-read decisions, two-phase commits
//! - **Generation Counters**: ABA prevention, fork detection
//! - **Circuit Breaker Integration**: Instant security response
//!
//! ## Capsules
//!
//! - `AtomicTransactionCapsule` (ATC-512): <500ns transaction validation
//! - `AtomicBlockCapsule` (ABC-1024): <1μs block validation
//! - `AccountStateCapsule` (ASC-256): <100ns account updates
//!
//! ## Performance Targets
//!
//! Based on The Atomic Capsule architecture:
//! - Transaction validation: <500ns
//! - Block validation: <1μs
//! - Account updates: <100ns
//! - Zero allocation in hot paths

#![warn(missing_docs, rust_2018_idioms)]
#![cfg_attr(feature = "nightly", feature(portable_simd))]

pub mod transaction_capsule;
pub mod block_capsule;
pub mod account_state_capsule;
pub mod capsule_primitives;

// Re-export core types
pub use transaction_capsule::{
    AtomicTransactionCapsule, TransactionData, TransactionStatus, TransactionError,
};
pub use block_capsule::{AtomicBlockCapsule, BlockHeader, BlockData, BlockError};
pub use account_state_capsule::{AccountStateCapsule, AccountState, AccountError};
pub use capsule_primitives::{CapsuleHeader, CapsuleStatus, ProtectionLevel};

/// Kindly Core version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Golden ratio constant (φ) for phi-based scaling
///
/// Used throughout Kindly Coin for:
/// - Validator selection (φ × voting weight)
/// - Risk scaling (L1 = 1/φ, L2 = 1/φ²)
/// - Fee prioritization
pub const PHI: f64 = 1.6180339887498948;

/// Phi conjugate (1/φ) for scaling down
pub const PHI_CONJUGATE: f64 = 0.6180339887498948;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_exists() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_phi_constants() {
        // Verify golden ratio properties
        assert!((PHI - 1.618).abs() < 0.001);
        assert!((PHI_CONJUGATE - 0.618).abs() < 0.001);
        assert!((PHI * PHI_CONJUGATE - 1.0).abs() < 0.001);
    }
}
