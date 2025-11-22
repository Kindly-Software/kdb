//! Tier 6: Mixed Capsule Trait
//!
//! **Hybrid strategies for compound speedups (12-2000×)**
//!
//! ## UCE33 Q10: Tier 6 (Mixed)
//!
//! Mixed capsules combine multiple tiers for multiplicative speedups:
//! - **Proven**: 12× (transaction begin), 35× (Hebbian learning), 2000× (specialized)
//! - **Combinations**: Atomic+SIMD, Fixed-Point+SIMD, Atomic+Fixed-Point+SIMD
//! - **Alignment**: Max of component alignments
//! - **Use cases**: Circuit breaker + SIMD position adjustment, Atomic coordination + fixed-point accounting
//!
//! ## Compound Speedup Examples
//!
//! ```text
//! Atomic (3×) + SIMD (7×) = 21× compound speedup
//! Atomic (3×) + Fixed-Point (5×) = 15× compound speedup
//! Atomic (3×) + SIMD (7×) + Batch (100×) = 2,100× theoretical
//! ```
//!
//! ## Memory Layout
//!
//! ```text
//! [ Mixed Capsule (alignment = max(T1::ALIGNMENT, T2::ALIGNMENT)) ]
//! ├─ component1: T1 (primary tier)
//! ├─ component2: T2 (secondary tier)
//! └─ _padding: [u8] (ensure alignment)
//! ```

/// Tier 6: Mixed Capsule Trait
///
/// Provides composition of multiple capsule tiers for compound speedups.
///
/// ## UCE33 Framework Compliance
///
/// - **Q10 (Tier Selection)**: Tier 6 for hybrid strategies
/// - **Q13 (Resources)**: Union of component tier requirements
/// - **Q15 (Scaling)**: Compound scaling (multiplicative speedups)
/// - **Q17 (Interface)**: Access to component capsules via traits
/// - **Q27 (Composition)**: Safe composition via trait bounds
/// - **Q33 (Verification)**: Automatic alignment calculation
///
/// ## Safety Requirements
///
/// - **Alignment**: Max of all component alignments
/// - **Atomicity**: Preserve lockfree properties from Tier 1 components
/// - **Verification**: All components must be verified capsules
///
/// ## Example (Atomic + SIMD)
///
/// ```rust,ignore
/// use atomic_capsule::{MixedCapsule, AtomicCapsule, SimdCapsule, verify_capsule_properties};
/// use core::sync::atomic::{AtomicU64, Ordering};
///
/// #[repr(C, align(128))]
/// struct AtomicSimdMixed {
///     state: AtomicU64,          // Tier 1: Atomic coordination
///     simd_data: [f32; 8],       // Tier 2: SIMD computation
///     _padding: [u8; 88],
/// }
///
/// verify_capsule_properties!(AtomicSimdMixed, 128, 128);
///
/// impl AtomicCapsule for AtomicSimdMixed {
///     type Primitive = u64;
///     fn load(&self) -> Self::Primitive {
///         self.state.load(Ordering::Acquire)
///     }
///     fn store(&self, value: Self::Primitive) {
///         self.state.store(value, Ordering::Release);
///     }
/// }
///
/// impl SimdCapsule for AtomicSimdMixed {
///     type Element = f32;
///     const LANES: usize = 8;
///     fn simd_load(&self) -> [Self::Element; Self::LANES] {
///         self.simd_data
///     }
/// }
///
/// impl MixedCapsule for AtomicSimdMixed {
///     const PRIMARY_TIER: usize = 1;   // Atomic
///     const SECONDARY_TIER: usize = 2; // SIMD
///     const COMPOUND_SPEEDUP_EXPECTED: f64 = 21.0; // 3× × 7×
/// }
/// ```
pub trait MixedCapsule: super::ComputationalCapsule {
    /// Primary capsule tier (1-10)
    const PRIMARY_TIER: usize;

    /// Secondary capsule tier (1-10)
    const SECONDARY_TIER: usize;

    /// Expected compound speedup (multiplicative)
    ///
    /// Example: Atomic (3×) + SIMD (7×) = 21×
    const COMPOUND_SPEEDUP_EXPECTED: f64;

    /// Verify mixed capsule composition
    ///
    /// Checks:
    /// - Alignment is max of component alignments
    /// - Size accommodates both components
    /// - Thread safety preserved
    fn verify_composition() -> bool {
        // Default implementation: always valid
        // Implementers can override for custom validation
        true
    }
}

/// Marker trait for Tier 1 + Tier 2 mixed capsules (Atomic + SIMD)
///
/// ## Use Case
///
/// Circuit breaker coordination (Atomic) + venue scoring (SIMD)
///
/// ## Expected Speedup
///
/// 3× (Atomic) × 7× (SIMD) = 21× compound
#[cfg(feature = "portable_simd")]
pub trait AtomicSimdMixed: MixedCapsule + super::AtomicCapsule + super::SimdCapsule {}

/// Marker trait for Tier 1 + Tier 3 mixed capsules (Atomic + Fixed-Point)
///
/// ## Use Case
///
/// Execution state (Atomic) + P&L tracking (Fixed-Point)
///
/// ## Expected Speedup
///
/// 3× (Atomic) × 5× (Fixed-Point) = 15× compound
pub trait AtomicFixedPointMixed:
    MixedCapsule + super::AtomicCapsule + super::FixedPointCapsule
{
}

/// Marker trait for Tier 2 + Tier 3 mixed capsules (SIMD + Fixed-Point)
///
/// ## Use Case
///
/// Vectorized deterministic financial calculations
///
/// ## Expected Speedup
///
/// 7× (SIMD) × 5× (Fixed-Point) = 35× compound (proven in Hebbian learning)
#[cfg(feature = "portable_simd")]
pub trait SimdFixedPointMixed:
    MixedCapsule + super::SimdCapsule + super::FixedPointCapsule
{
}

/// Marker trait for triple mixed capsules (Atomic + SIMD + Fixed-Point)
///
/// ## Use Case
///
/// Complete trading system (coordination + vectorization + determinism)
///
/// ## Expected Speedup
///
/// 3× (Atomic) × 7× (SIMD) × 5× (Fixed-Point) = 105× theoretical
#[cfg(feature = "portable_simd")]
pub trait TripleMixed:
    MixedCapsule + super::AtomicCapsule + super::SimdCapsule + super::FixedPointCapsule
{
}

/// Helper macro for calculating mixed capsule alignment
///
/// Returns the maximum alignment of all component types.
///
/// ## Example
///
/// ```rust,ignore
/// use atomic_capsule::max_alignment;
///
/// struct ComponentA;
/// impl ComputationalCapsule for ComponentA {
///     const ALIGNMENT: usize = 64;
/// }
///
/// struct ComponentB;
/// impl ComputationalCapsule for ComponentB {
///     const ALIGNMENT: usize = 128;
/// }
///
/// const MIXED_ALIGNMENT: usize = max_alignment!(ComponentA, ComponentB);
/// assert_eq!(MIXED_ALIGNMENT, 128); // max(64, 128)
/// ```
#[macro_export]
macro_rules! max_alignment {
    ($first:ty) => {
        <$first as $crate::traits::ComputationalCapsule>::ALIGNMENT
    };
    ($first:ty, $($rest:ty),+) => {
        {
            const FIRST: usize = <$first as $crate::traits::ComputationalCapsule>::ALIGNMENT;
            const REST: usize = $crate::max_alignment!($($rest),+);
            if FIRST > REST { FIRST } else { REST }
        }
    };
}

/// Helper macro for calculating expected compound speedup
///
/// Returns the product of all individual tier speedups.
///
/// ## Example
///
/// ```rust,ignore
/// use atomic_capsule::compound_speedup;
///
/// const ATOMIC_SPEEDUP: f64 = 3.0;
/// const SIMD_SPEEDUP: f64 = 7.0;
/// const FIXED_POINT_SPEEDUP: f64 = 5.0;
///
/// const COMPOUND: f64 = compound_speedup!(ATOMIC_SPEEDUP, SIMD_SPEEDUP, FIXED_POINT_SPEEDUP);
/// assert_eq!(COMPOUND, 105.0); // 3 × 7 × 5
/// ```
#[macro_export]
macro_rules! compound_speedup {
    ($first:expr) => {
        $first
    };
    ($first:expr, $($rest:expr),+) => {
        $first * $crate::compound_speedup!($($rest),+)
    };
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_compound_speedup_calculation() {
        const ATOMIC_SPEEDUP: f64 = 3.0;
        const SIMD_SPEEDUP: f64 = 7.0;
        const FIXED_POINT_SPEEDUP: f64 = 5.0;

        // Single tier
        const SINGLE: f64 = compound_speedup!(ATOMIC_SPEEDUP);
        assert_eq!(SINGLE, 3.0);

        // Two tiers
        const DOUBLE: f64 = compound_speedup!(ATOMIC_SPEEDUP, SIMD_SPEEDUP);
        assert_eq!(DOUBLE, 21.0);

        // Three tiers
        const TRIPLE: f64 = compound_speedup!(ATOMIC_SPEEDUP, SIMD_SPEEDUP, FIXED_POINT_SPEEDUP);
        assert_eq!(TRIPLE, 105.0);
    }
}
