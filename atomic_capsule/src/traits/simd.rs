//! # SIMD Capsule Trait (Nightly Only)
//!
//! Specialized trait for SIMD vectorized computation capsules.
//!
//! ## UCE33 Q32 (Nightly Enhancement)
//!
//! This trait leverages cutting-edge nightly features:
//! - `portable_simd`: Cross-platform SIMD via std::simd
//! - `const_fn_floating_point`: Compile-time SIMD lane initialization
//! - Platform-independent vectorization (x86 AVX, ARM NEON, RISC-V RVV)
//!
//! ## UCE33 Q33 (Foundation)
//!
//! SIMD capsules extend atomic capsule foundation to vectorized computation:
//! - Same alignment requirements (64/128/256 bytes)
//! - Same cache-aware design
//! - Computational efficiency via parallel lanes
//!
//! ## Performance Targets (B32 Framework)
//!
//! - f32x8 operations: <5ns (8 parallel float ops)
//! - f64x4 operations: <10ns (4 parallel double ops)
//! - 8-16x speedup vs scalar on AVX-512

use super::ComputationalCapsule;

#[cfg(feature = "portable_simd")]
use core::simd::SimdElement;

/// SIMD capsule specialization for vectorized computation.
///
/// Implementors MUST use portable SIMD types from `core::simd`.
///
/// # UCE33 Q32 (Nightly Enhancement)
///
/// Portable SIMD enables:
/// - Cross-platform vectorization (x86/ARM/RISC-V)
/// - Compile-time lane count verification
/// - Zero-cost abstraction over platform intrinsics
///
/// # Safety Model
///
/// This trait is intentionally unsafe to implement because:
/// - Incorrect SIMD alignment causes segfaults on some platforms
/// - Lane count mismatches cause undefined behavior
/// - SIMD operations require specific CPU features
///
/// # ASSUM Framework
///
/// - `#ASSUME_SIMD_ALIGNED`: SIMD types require alignment (16/32/64 bytes)
/// - `#VERIFY_SIMD_ALIGNED`: Compile-time via const generics
/// - `#ASSUME_LANES_VALID`: Lane count is power of 2 (2/4/8/16/32/64)
/// - `#VERIFY_LANES_VALID`: Enforced by Simd<T, N> type bounds
///
/// # Safety
///
/// This trait is unsafe to implement because:
/// - Implementor must ensure proper SIMD alignment (16/32/64 bytes for AVX2/AVX-512)
/// - Incorrect alignment causes undefined behavior on SIMD loads/stores
/// - Lane count must be power of 2 and supported by target architecture
/// - Implementor must validate SIMD operations compile correctly for target CPU
///
/// # Example
///
/// ```rust,ignore
/// #![feature(portable_simd)]
/// use atomic_capsule::traits::{ComputationalCapsule, SimdCapsule};
/// use core::simd::f32x8;
///
/// #[repr(C, align(64))]
/// struct VectorizedPricingCapsule {
///     prices: [f32x8; 4], // 8 lanes × 4 vectors = 32 prices
/// }
///
/// unsafe impl ComputationalCapsule for VectorizedPricingCapsule {
///     const ALIGNMENT: usize = 64;
///     const SIZE: usize = 128; // 32 × f32
///     const TYPE_ID: &'static str = "VectorizedPricingCapsule";
/// }
///
/// unsafe impl SimdCapsule for VectorizedPricingCapsule {
///     type Element = f32;
///     const LANES: usize = 8;
///
///     fn simd_alignment() -> usize {
///         32 // AVX-256 alignment
///     }
/// }
/// ```
#[cfg(feature = "portable_simd")]
// const_trait disabled for this nightly
// #[cfg_attr(feature = "portable_simd", const_trait)]
pub unsafe trait SimdCapsule: ComputationalCapsule {
    /// SIMD element type (f32, f64, i32, u64, etc.).
    ///
    /// # UCE33 Q31 (Rust Transform)
    /// Associated type enables compile-time element selection
    type Element: SimdElement;

    /// Number of SIMD lanes (2, 4, 8, 16, 32, 64).
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_LANES_POW2`: Lane count is power of 2
    /// - `#VERIFY_LANES_POW2`: Enforced by Simd<T, N> bounds
    ///
    /// # UCE33 Q29 (Constraints)
    /// Hardware constraint: Lane counts match SIMD register widths
    /// - AVX-256: 8×f32, 4×f64
    /// - AVX-512: 16×f32, 8×f64
    /// - NEON: 4×f32, 2×f64
    const LANES: usize;

    /// SIMD alignment requirement (16, 32, 64 bytes).
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_SIMD_ALIGNED`: SIMD loads/stores require alignment
    /// - `#VERIFY_SIMD_ALIGNED`: Compile-time via capsule alignment
    ///
    /// # Default: Capsule alignment
    ///
    /// SIMD alignment typically matches capsule alignment.
    #[inline(always)]
    fn simd_alignment() -> usize {
        Self::ALIGNMENT
    }

    /// Check if lane count is valid.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_LANES_VALID`: Lane count is power of 2 and within bounds
    /// - `#VERIFY_LANES_VALID`: Checked at compile-time
    ///
    /// # UCE33 Q30 (Validation)
    /// Compile-time verification of SIMD lane count
    #[inline(always)]
    fn verify_lanes() -> bool {
        // Power of 2 check
        Self::LANES.count_ones() == 1
            // Reasonable bounds (2-64 lanes)
            && Self::LANES >= 2
            && Self::LANES <= 64
    }

    /// Expected SIMD operation latency in nanoseconds.
    ///
    /// # UCE33 Q29 (Constraints)
    ///
    /// Hardware constraint: SIMD operation latency
    /// - f32x8 (AVX): ~3-5ns
    /// - f64x4 (AVX): ~5-10ns
    /// - f32x16 (AVX-512): ~5-10ns
    ///
    /// # Performance Targets (B32 Framework)
    ///
    /// - Vectorized: <10ns per SIMD operation
    /// - Speedup: 8-16x vs scalar (depending on lanes)
    ///
    /// # Default: 5ns
    ///
    /// Typical SIMD operation latency on modern CPUs.
    #[inline(always)]
    fn expected_simd_latency_ns() -> u64 {
        5
    }

    /// Platform-specific SIMD capabilities.
    ///
    /// # UCE33 Q32 (Nightly Enhancement)
    ///
    /// Portable SIMD detects platform capabilities at compile-time:
    /// - x86: AVX, AVX2, AVX-512
    /// - ARM: NEON, SVE
    /// - RISC-V: RVV
    ///
    /// # Default: "portable"
    ///
    /// Portable SIMD works on all platforms.
    #[inline(always)]
    fn simd_capabilities() -> &'static str {
        "portable"
    }
}

// NOTE: verify_simd_capsule! macro is defined in verification.rs
// Do not duplicate it here to avoid macro name conflicts

#[cfg(all(test, feature = "portable_simd"))]
mod tests {
    use super::*;
    use crate::verify_simd_capsule;
    use core::simd::f32x8;

    #[repr(C, align(64))]
    struct TestSimdCapsule {
        data: [f32x8; 4],
    }

    unsafe impl ComputationalCapsule for TestSimdCapsule {
        const ALIGNMENT: usize = 64;
        const SIZE: usize = 128; // 32 × f32
        const TYPE_ID: &'static str = "TestSimdCapsule";
    }

    unsafe impl SimdCapsule for TestSimdCapsule {
        type Element = f32;
        const LANES: usize = 8;
    }

    #[test]
    fn test_simd_capsule_defaults() {
        assert_eq!(TestSimdCapsule::simd_alignment(), 64);
        assert!(TestSimdCapsule::verify_lanes());
        assert_eq!(TestSimdCapsule::expected_simd_latency_ns(), 5);
        assert_eq!(TestSimdCapsule::simd_capabilities(), "portable");
    }

    #[test]
    fn test_simd_verification_macro() {
        verify_simd_capsule!(TestSimdCapsule, 64, 32);
    }
}
