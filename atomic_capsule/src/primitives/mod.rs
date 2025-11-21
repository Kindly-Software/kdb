//! # Computational Capsule Primitives
//!
//! **Production-ready computational capsule implementations.**
//!
//! This module provides concrete capsule patterns optimized for different use cases:
//! - `SimdF32x8Capsule`: 8 × f32 SIMD operations (Hot Tier, 256 bits)
//! - `SimdF64x8Capsule`: 8 × f64 SIMD operations (Warm Tier, 512 bits)
//! - `FixedQ16_16Capsule`: Q16.16 fixed-point arithmetic (Hot Tier, 64 bits)
//! - `fixed_point`: Generic fixed-point arithmetic module (NEW)
//!
//! ## UCE33 Analysis Applied
//!
//! - **Q28 (Simplicity)**: Simple SIMD trait interface hides platform-specific complexity
//! - **Q29 (Constraints)**: SIMD alignment (32B AVX2, 64B AVX-512), cache line awareness
//! - **Q30 (Validation)**: Benchmarked against scalar baselines, statistical validation required
//! - **Q31 (Rust Transform)**: portable_simd for cross-platform SIMD, compile-time verification
//! - **Q32 (Nightly)**: portable_simd (nightly feature), const_fn_floating_point
//! - **Q33 (Atomic Capsule)**: SIMD extends capsule foundation for batch operations
//!
//! ## Design Principles
//!
//! All computational capsules follow atomic capsule principles:
//! - Cache-aligned structures (64B or 128B)
//! - Lockfree coordination via atomic generation counters
//! - Scalar fallback for non-SIMD platforms
//! - Zero-cost abstractions via const generics
//!
//! ## ASSUM Framework
//!
//! Safety assumptions documented per capsule:
//! - `#ASSUME_SIMD_ALIGNMENT`: Data aligned to SIMD requirements
//! - `#VERIFY_ALIGNMENT_STATIC`: Compile-time alignment verification
//! - `#ASSUME_ELEMENT_COUNT`: Fixed element count per capsule
//! - `#VERIFY_ELEMENT_COUNT`: Compile-time size verification

pub mod fixed_point;
pub mod fixed_q16_16;

#[cfg(feature = "portable_simd")]
pub mod simd_f32;

#[cfg(feature = "portable_simd")]
pub mod simd_f64;

#[cfg(feature = "portable_simd")]
pub mod simd_i32;

// Phase 2.3: atomic_from_mut Foundation (T0 tier - nightly feature)
#[cfg(feature = "nightly-atomic")]
pub mod atomic_from_mut;

// CPU Capability Detection (T1 Atomic)
#[cfg(feature = "cpu-capabilities")]
pub mod cpu_capabilities;

// Phase 2.1: SIMD + Fixed-Point Vectorization Layer (portable_simd feature)
#[cfg(feature = "portable_simd")]
pub mod simd_vectorization;

// Phase 2.4.1: T1+T2+T3 Composite Capsules (Atomic + SIMD + Fixed-Point)
#[cfg(feature = "portable_simd")]
pub mod atomic_simd_fixed;

// Phase 2.4.1: Inference Primitives (T2+T4, T2+T5, T3)
#[cfg(feature = "portable_simd")]
pub mod inference;

// Phase 4.2: Complex Number Primitives (T2 SIMD + T3 Fixed-Point)
#[cfg(any(feature = "complex-simd", feature = "complex-fixed"))]
pub mod complex;

// Phase 4: SIMD Cryptography (T2 SIMD AES-256-GCM, SHA3-256, PBKDF2)
#[cfg(feature = "simd-crypto")]
pub mod simd_crypto;

// Progress Tracker (Lockfree Atomic)
pub mod progress_tracker;

// Phase 11: Lockfree Coordination Primitives
pub mod coordination;

// Phase 4.1: Financial Greeks Calculator (T3 Fixed-Point)
#[cfg(feature = "financial-greeks")]
pub mod greeks;

// Re-export capsule types (conditionally based on features)
pub use fixed_q16_16::FixedQ16_16Capsule;

#[cfg(feature = "portable_simd")]
pub use simd_f32::SimdF32x8Capsule;

#[cfg(feature = "portable_simd")]
pub use simd_f64::SimdF64x8Capsule;

#[cfg(feature = "portable_simd")]
pub use simd_i32::SimdI32x8Capsule;

// Phase 2.1 exports (portable_simd feature)
#[cfg(feature = "portable_simd")]
pub use simd_vectorization::{
    BatchSimdFixedPoint, SimdF32x8Capsule as SimdF32x8CapsuleNew, SimdFixedPointQ16x8Capsule,
    SimdI32x8Capsule as SimdI32x8CapsuleNew,
};

// Re-export generic fixed-point types
pub use fixed_point::{q16_16, q32_32, q48_16, q8_8, FixedPoint, Q16_16, Q32_32, Q48_16, Q8_8};

// Phase 2.3 exports (nightly-atomic feature)
#[cfg(feature = "nightly-atomic")]
pub use atomic_from_mut::{from_mut_pair, AtomicFromMut, AtomicFromMutError};

// Phase 2.4.1 exports (portable_simd feature)
#[cfg(feature = "portable_simd")]
pub use atomic_simd_fixed::{
    AtomicSimdFixedQ16x8Capsule, DeterministicMLInference, LockfreeFinancialAggregator,
};

// Phase 2.4.1 inference exports (portable_simd feature)
#[cfg(feature = "portable_simd")]
pub use inference::{FlashAttentionCapsule, QuantizationCapsule, SIMDMatMulCapsule};

// Phase 4.2 complex number exports
#[cfg(feature = "complex-simd")]
pub use complex::ComplexF32x4;

#[cfg(feature = "complex-fixed")]
pub use complex::{from_q16_48, to_q16_48, ComplexCell};

// Phase P3 progress tracker exports
pub use progress_tracker::ProgressTrackerCapsule;

// Phase 4: SIMD Crypto exports (simd-crypto feature)
#[cfg(feature = "simd-crypto")]
pub use simd_crypto::{CryptoError, SimdCryptoCapsule};

// Phase 11 Baseline coordination exports
pub use coordination::{
    PhaseCoordinatorCapsule, PhaseError, PhaseStats, PhaseStatus,
    LockfreeHashBucketCapsule, InsertError, BucketStats,
    ParallelPartitionCapsule, PartitionError, PartitionStats, PartitionStatus,
};

// Phase 4.1 Financial Greeks exports
#[cfg(feature = "financial-greeks")]
pub use greeks::GreeksCapsule;

/// Common trait for SIMD capsule operations
///
/// # UCE33 Integration
/// - **Q33**: SIMD capsules extend atomic capsule foundation for vectorized operations
/// - **Q31**: Trait enables zero-cost abstraction across different SIMD widths
///
/// Note: This trait requires `generic_const_exprs` feature for const array sizes.
#[cfg(feature = "nightly")]
pub trait SimdCapsule {
    /// SIMD element type (f32, f64, i32, etc.)
    type Element: Copy;

    /// Number of SIMD lanes
    const LANES: usize;

    /// Cache alignment requirement
    const ALIGNMENT: usize;

    /// Load data from capsule
    fn load(&self) -> [Self::Element; Self::LANES];

    /// Store data to capsule
    fn store(&self, data: [Self::Element; Self::LANES]);
}

/// Fixed-point capsule trait for deterministic arithmetic
///
/// # UCE33 Integration
/// - **Q33**: Fixed-point enables deterministic atomic operations
/// - **Q29**: Constraint - fixed-point avoids FP non-determinism
pub trait FixedPointCapsule {
    /// Fixed-point scale factor (e.g., 2^16 for Q16.16)
    const SCALE: i32;

    /// Convert from floating-point to fixed-point
    fn from_f64(value: f64) -> Self;

    /// Convert from fixed-point to floating-point
    fn to_f64(&self) -> f64;

    /// Fixed-point multiplication with proper scaling
    fn mul(&self, other: &Self) -> Self;

    /// Fixed-point division with proper scaling
    fn div(&self, other: &Self) -> Self;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "portable_simd")]
    #[test]
    fn test_module_structure() {
        // Verify module exports are accessible
        use crate::primitives::SimdF32x8Capsule;

        // Type existence check
        assert_eq!(core::mem::size_of::<SimdF32x8Capsule>(), 64);
        assert_eq!(core::mem::align_of::<SimdF32x8Capsule>(), 64);
    }
}
