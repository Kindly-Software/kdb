//! # Kindly Consensus - Atomic Byzantine Fault Tolerance (A-BFT)
//!
//! Lockfree consensus engine using atomic capsules for instant finality detection.
//!
//! ## Design Principles
//!
//! - **100% Lockfree**: No mutex/RwLock, atomic capsules only
//! - **Instant Finality**: 2/3 vote detection in <50ns
//! - **Phi-Based Selection**: Golden ratio validator weighting
//! - **Generation Counters**: Fork detection and ABA prevention
//! - **Circuit Breaker**: Attack detection and graceful degradation
//!
//! ## Capsules
//!
//! - `ValidatorCapsule` (AVC-512): Phi-based validator state, <100ns updates
//! - `FinalityCapsule` (AFC-128): Instant finality detection, <50ns checks
//! - `VoteAggregator` (VAC-256): Lockfree vote counting
//!
//! ## Performance Targets
//!
//! - Vote processing: <100ns per vote
//! - Finality check: <50ns (single atomic read)
//! - Consensus latency: <10ms full round
//! - Fork detection: <5ns (generation counter check)
//!
//! ## UCE33 Analysis (Internal)
//!
//! **Q33: How do atomic capsules transform consensus?**
//! - Lockfree voting eliminates coordinator bottleneck
//! - Instant finality detection (single atomic read vs merkle tree traversal)
//! - No message passing overhead (atomic state coordination)
//! - Generation counters prevent fork attacks
//! - Circuit breaker enables graceful degradation under attack

#![warn(missing_docs, rust_2018_idioms)]
#![cfg_attr(feature = "nightly", feature(portable_simd))]

pub mod validator_capsule;
pub mod abft_engine;
pub mod finality_capsule;
pub mod vote_aggregator;

// Re-export core types
pub use validator_capsule::{
    ValidatorCapsule, ValidatorId, ValidatorState, ValidatorError, ValidatorSelection,
};
pub use abft_engine::{AbftEngine, AbftConfig, ConsensusRound, ConsensusError};
pub use finality_capsule::{FinalityCapsule, FinalityStatus, FinalityError};
pub use vote_aggregator::{VoteAggregator, VoteCount, VoteError, VoteWeight};

/// Consensus protocol version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Golden ratio for phi-based validator selection
pub const PHI: f64 = 1.6180339887498948;

/// Finality threshold (2/3 of total voting weight)
pub const FINALITY_THRESHOLD: f64 = 2.0 / 3.0;

/// Maximum validators supported (power of 2 for efficient indexing)
pub const MAX_VALIDATORS: usize = 128;

/// Maximum concurrent consensus rounds
pub const MAX_CONSENSUS_ROUNDS: usize = 32;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_exists() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_phi_constant() {
        assert!((PHI - 1.618).abs() < 0.001);
    }

    #[test]
    fn test_finality_threshold() {
        assert!((FINALITY_THRESHOLD - 0.6666666666).abs() < 0.001);
    }
}
