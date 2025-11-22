//! # Unified Computational Capsule Trait Hierarchy
//!
//! Hierarchical trait system enabling 95% COCA compliance across all 10 tiers.
//!
//! ## UCE33 Framework Applied
//!
//! - **Q10 (Capsule Tier)**: Foundation for all 10 tiers (Atomic/SIMD/Fixed-Point/Batch/Streaming/Mixed/GPU/Network/Persistent/Probabilistic)
//! - **Q28 (Simplicity)**: Base trait + tier-specific extensions
//! - **Q29 (Constraints)**: Rust trait system, no GATs needed
//! - **Q33 (Verification)**: Integrated verification in base trait
//!
//! ## Architecture
//!
//! ```text
//! Capsule (base trait)
//!   ├── Tier 1: AtomicCapsule (<100ns lockfree coordination)
//!   ├── Tier 2: SimdCapsule (2-19× vectorized computation)
//!   ├── Tier 3: FixedPointCapsule (2-10× deterministic arithmetic)
//!   ├── Tier 4: BatchCapsule (10-100× high-throughput)
//!   ├── Tier 5: StreamingCapsule (O(1) continuous computation)
//!   └── Tier 6: MixedCapsule<T1, T2> (12-2000× compound speedups)
//! ```
//!
//! ## Feature Flags
//!
//! - `unified-traits`: Enable new hierarchical trait system (opt-in, backward compatible)
//! - `portable_simd`: Enable Tier 2 SIMD capsules (nightly)
//!
//! ## Migration Path
//!
//! See `docs/TRAIT_HIERARCHY_MIGRATION_GUIDE.md` for detailed migration instructions.
//!
//! ## Backward Compatibility (I20 Framework)
//!
//! **Q6 (Architectural Compatibility)**: Existing traits remain unchanged without `unified-traits` feature
//! **Q7 (Performance Compatibility)**: Zero runtime overhead (all trait methods inline)
//! **Q9 (Concurrency)**: Send + Sync enforced via trait bounds

// Legacy traits (backward compatible)
pub mod atomic;
pub mod computational;

#[cfg(feature = "portable_simd")]
pub mod simd;

pub mod fixed_point;

// New Tier 4-6 traits (always available)
pub mod batch;
pub mod mixed;
pub mod streaming;

// New Tier 7-8 traits (foundation)
pub mod gpu;
#[cfg(feature = "std")] // NetworkCapsule requires Vec/String
pub mod network;

// Tier 0: Auditable Capsule Foundation (meta-tier below all others, requires std for Vec)
#[cfg(feature = "std")]
pub mod auditable;

// Re-export legacy traits (always available)
// Note: These remain as the default imports (backward compatible)
pub use atomic::AtomicCapsule;
pub use computational::ComputationalCapsule;

#[cfg(feature = "portable_simd")]
pub use simd::SimdCapsule;

pub use fixed_point::FixedPointCapsule;

// Re-export new tier traits (always available)
pub use batch::{BatchCapsule, BatchError};
pub use mixed::{AtomicFixedPointMixed, MixedCapsule};
pub use streaming::{StreamError, StreamingCapsule};

// Re-export Tier 7-8 traits (foundation)
pub use gpu::{GpuCapsule, GpuError, GpuProperties};
#[cfg(feature = "std")] // NetworkCapsule requires Vec/String
pub use network::{NetworkCapsule, NetworkError, NetworkStats};

// Re-export Tier 0: Auditable Capsule Foundation (requires std for Vec)
#[cfg(feature = "std")]
pub use auditable::{AuditableCapsule, CapsuleAuditTrail, CapsuleSnapshot};

#[cfg(feature = "portable_simd")]
pub use mixed::{AtomicSimdMixed, SimdFixedPointMixed, TripleMixed};

// Unified trait hierarchy (opt-in via feature flag)
#[cfg(feature = "unified-traits")]
pub mod unified;

#[cfg(feature = "unified-traits")]
pub use unified::*;

// Export everything under `unified::` namespace when feature enabled
#[cfg(feature = "unified-traits")]
pub mod prelude {
    //! Convenience prelude for unified trait hierarchy
    pub use super::unified::{
        // Tier-specific traits
        AtomicCapsule,
        BatchCapsule,
        Capsule,
        FixedPointCapsule,
        MixedCapsule,
        StreamingCapsule,
        Tier,
        VerificationError,
    };

    #[cfg(feature = "portable_simd")]
    pub use super::unified::SimdCapsule;
}
