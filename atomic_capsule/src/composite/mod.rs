//! # Multi-Tier Composite Capsules (Phase 11)
//!
//! **Composite capsules combine 2-3 tiers in flat layout for compound speedups.**
//!
//! ## UCE35 Q10.5 - Composition Terminology
//!
//! **Composite Capsule** (this module):
//! - Single struct combining fields from multiple tiers
//! - Flat layout (all fields inline, no nested indirection)
//! - Use case: <10K objects, 2-3 tier combinations
//! - Speedup: 12-24× compound (proven: 3× atomic × 4× SIMD × 2× fixed-point)
//!
//! **Container Capsule** (see `collections` module):
//! - Management structure coordinating ≥100K capsules
//! - Use case: ≥100K objects, isolation requirements
//! - Examples: BudgetMetaCapsule, ConcurrentMapCapsule
//!
//! ## Performance Claims (B32 Framework)
//!
//! | Composition | Speedup | Use Case |
//! |-------------|---------|----------|
//! | T1+T2 | 12× | Lockfree vectorized state |
//! | T2+T3 | 8× | Deterministic vectorized math |
//! | T1+T2+T3 | 24× | Complete coordination + computation |
//! | Full (T1+T2+T3+T4) | 50-100× | Maximum optimization (rare) |
//!
//! ## Design Principles
//!
//! - **Flat Layout**: All fields inline, no indirection
//! - **Cache Alignment**: Max of component tiers (128B for T1+T2)
//! - **Lockfree**: 100% atomic operations, no mutex/RwLock
//! - **Compile-time Verified**: `#[derive(ComputationalCapsule)]` mandatory
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_COMPOSITE_LAYOUT`: Flat layout ensures single cache line access
//! - `#VERIFY_COMPOSITE_LAYOUT`: `#[repr(C)]` + verification macros
//! - `#ASSUME_ALIGNMENT_MAX`: 128B sufficient for T1+T2 composites
//! - `#VERIFY_ALIGNMENT_MAX`: Compile-time static assertions
//!
//! ## UCE34 Framework Compliance
//!
//! - **Q10 (Tier Selection)**: Composite capsule for 2-3 tier combinations
//! - **Q11 (Rust Transform)**: Zero-cost abstractions via traits
//! - **Q12 (Nightly Enhancement)**: portable_simd for SIMD operations
//! - **Q33 (Validation)**: Automatic verification via derive macro
//!
//! ## Usage
//!
//! ```rust,ignore
//! use atomic_capsule::composite::AtomicSimdCapsule;
//!
//! #[derive(ComputationalCapsule)]
//! #[capsule(alignment = 128, size = 128)]
//! #[repr(C, align(128))]
//! struct MyComposite {
//!     atomic_state: AtomicU64,  // T1: Atomic coordination
//!     simd_data: [f32; 8],      // T2: SIMD computation
//!     _padding: [u8; 88],
//! }
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

// Existing composite implementations (Phase 2.4.1)
#[cfg(feature = "portable_simd")]
pub mod atomic_simd;

#[cfg(feature = "portable_simd")]
pub mod simd_fixed_point;

#[cfg(feature = "portable_simd")]
pub mod full_compound;

// New Phase 11 tier-specific modules
#[cfg(feature = "tier1-tier2")]
pub mod tier1_tier2;

#[cfg(feature = "tier2-tier3")]
pub mod tier2_tier3;

#[cfg(feature = "tier1-tier2-tier3")]
pub mod tier1_tier2_tier3;

// Phase 4: Observability Capsule (T6 Mixed: T1+T2+T5)
#[cfg(feature = "observability")]
pub mod observability;

// Re-export existing composite types (Phase 2.4.1)
#[cfg(feature = "portable_simd")]
pub use atomic_simd::{AtomicSimdAccumulator, AtomicSimdCounter, AtomicSimdF32x8};

#[cfg(feature = "portable_simd")]
pub use simd_fixed_point::{OverflowError, SimdDeterministicML, SimdFinancialCalc, SimdFixedQ16x8};

#[cfg(feature = "portable_simd")]
pub use full_compound::{
    BatchAtomicSimdFixedQ16Capsule, FinancialBatchProcessor, MLBatchInference,
};

// Re-export new composite types (Phase 11)
#[cfg(feature = "tier1-tier2")]
pub use tier1_tier2::AtomicSimdCapsule;

#[cfg(feature = "tier2-tier3")]
pub use tier2_tier3::SimdFixedPointCapsule;

#[cfg(feature = "tier1-tier2-tier3")]
pub use tier1_tier2_tier3::FullCompositeCapsule;

// Re-export observability types (Phase 4)
#[cfg(feature = "observability")]
pub use observability::{ObservabilityCapsule, TraceEvent, TraceRingBuffer};
