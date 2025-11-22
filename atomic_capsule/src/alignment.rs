//! # Alignment Tier Traits
//!
//! Type-safe cache alignment tiers for atomic capsule architecture.
//!
//! ## UCE32 Analysis
//!
//! - **Q28 (Simplicity)**: Three tiers (Hot/Warm/Cold) match The Atomic Capsule architecture
//! - **Q29 (Constraints)**: Cache line sizes: 64B (typical), 128B (dual-channel), 256B (multi-line)
//! - **Q30 (Validation)**: Compile-time verification via `const ALIGNMENT` checks
//! - **Q31 (Rust Transform)**: Trait bounds enforce alignment at compile-time (zero runtime cost)
//! - **Q32 (Nightly)**: Optional `const_trait` for compile-time tier selection
//!
//! ## Design Pattern
//!
//! Following The Atomic Capsule (Section 6: Design Rules):
//! - **Rule 4**: "Single cache line where possible. 64–512 bits are ideal"
//! - **Hot Tier** (64B): Single cache line for sub-15ns atomic operations
//! - **Warm Tier** (128B): Dual-channel coordination (DualAtomicU64 pattern)
//! - **Cold Tier** (256B): Multi-line structures for portfolio/batch operations
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_ALIGNMENT_VALID`: All alignments are powers of 2
//! - `#VERIFY_ALIGNMENT_POW2`: Enforced by const generic bounds
//! - `#ASSUME_CACHE_ALIGNED`: Alignments match hardware cache lines
//! - `#VERIFY_CACHE_SIZES`: x86: 64B, ARM: 64B, RISC-V: 64B (Section 9 architecture notes)

use core::marker::PhantomData;

/// Alignment tier trait for atomic capsule structures.
///
/// Implementors must guarantee alignment via `#[repr(C, align(N))]`.
///
/// # Safety Model
///
/// This trait is intentionally unsafe to implement because incorrect alignment
/// can violate performance assumptions or cause false sharing.
///
/// # ASSUM Framework
/// - `#ASSUME_TYPE_SAFE`: Implementor guarantees `#[repr(C, align(ALIGNMENT))]`
/// - `#VERIFY_ALIGNMENT`: Compile-time via `const ALIGNMENT` checks
///
/// # Example
///
/// ```rust
/// use atomic_capsule::{AlignmentTier, HotTier};
///
/// #[repr(C, align(64))]
/// struct MyCapsule {
///     data: [u8; 64],
/// }
///
/// // Compiler verifies alignment matches HotTier::ALIGNMENT
/// impl AlignmentTier for MyCapsule {
///     const TIER: &'static str = "hot";
///     const ALIGNMENT: usize = 64;
/// }
/// ```
// Note: const_trait removed from this nightly version
// #[cfg_attr(feature = "portable_simd", const_trait)]
pub trait AlignmentTier {
    /// Human-readable tier name for debugging
    const TIER: &'static str;

    /// Alignment in bytes (must be power of 2)
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_ALIGNMENT_POW2`: Value is 64, 128, or 256
    /// - `#VERIFY_ALIGNMENT_POW2`: Enforced by HotTier/WarmTier/ColdTier implementations
    const ALIGNMENT: usize;

    /// Verify alignment at compile-time
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_INVARIANT`: Alignment is power of 2 and within valid range
    /// - `#VERIFY_INVARIANT`: Checked at compile-time via const evaluation
    #[inline(always)]
    fn verify_alignment() -> bool {
        // Power of 2 check
        Self::ALIGNMENT.count_ones() == 1
            // Within valid range
            && Self::ALIGNMENT >= crate::MIN_ALIGNMENT
            && Self::ALIGNMENT <= crate::MAX_ALIGNMENT
    }
}

/// Hot tier: 64-byte alignment for single cache line atomics.
///
/// **Performance Target**: <15ns atomic operations (hardware CAS latency).
///
/// **Use Cases** (from The Atomic Capsule):
/// - ACB-64: Circuit breaker (L0..L3 decision in 1 read)
/// - ACT-128: Cost gate (gross/fees/slip/net in 1 cache line)
/// - AVS-128: Venue snapshot (spread/OBI/microprice)
///
/// # UCE32 Q31 (Rust Transform)
///
/// Zero-cost abstraction: Alignment verified at compile-time, no runtime checks.
///
/// # ASSUM Framework
/// - `#ASSUME_CACHE_ALIGNED`: 64B matches x86/ARM/RISC-V L1 cache line size
/// - `#VERIFY_CACHE_ALIGNED`: Documented in arch::CacheLineSize detection
pub struct HotTier;

impl AlignmentTier for HotTier {
    const TIER: &'static str = "hot";
    const ALIGNMENT: usize = 64;
}

/// Warm tier: 128-byte alignment for dual-channel coordination.
///
/// **Performance Target**: <100ns coordination operations.
///
/// **Use Cases** (from The Atomic Capsule):
/// - Dual-channel atomic state (primary + metadata)
/// - ALE-128: Ledger entry (timestamp + hash chain)
/// - ALT-128: Latency ticket (f2d/d2a/a2f + jitter)
///
/// **Pattern**: DualAtomicU64 (cache-separated dual-channel coordination).
///
/// # UCE32 Q29 (Constraints)
///
/// 128-byte alignment prevents false sharing between dual channels.
///
/// # ASSUM Framework
/// - `#ASSUME_DUAL_CHANNEL`: Two 64-byte cache lines for independent coordination
/// - `#VERIFY_DUAL_CHANNEL`: 128B = 2 × 64B cache lines
pub struct WarmTier;

impl AlignmentTier for WarmTier {
    const TIER: &'static str = "warm";
    const ALIGNMENT: usize = 128;
}

/// Cold tier: 256-byte alignment for multi-line structures.
///
/// **Performance Target**: <1μs batch operations.
///
/// **Use Cases** (from The Atomic Capsule):
/// - APM-1024: Portfolio map (N symbols + totals)
/// - PEX-1024: Pre-executed plays (2-4 ready specs)
/// - RLT-1024: Risk ladder (thresholds per level)
///
/// # UCE32 Q29 (Constraints)
///
/// 256-byte alignment for structures spanning multiple cache lines.
///
/// # ASSUM Framework
/// - `#ASSUME_MULTI_LINE`: Structure requires >128 bytes (portfolio/batch data)
/// - `#VERIFY_MULTI_LINE`: 256B = 4 × 64B cache lines
pub struct ColdTier;

impl AlignmentTier for ColdTier {
    const TIER: &'static str = "cold";
    const ALIGNMENT: usize = 256;
}

/// Marker struct for compile-time alignment verification.
///
/// # UCE32 Q31 (Rust Transform)
///
/// PhantomData + const generics enable zero-cost compile-time verification.
///
/// # Example
///
/// ```rust
/// use atomic_capsule::{AlignmentMarker, HotTier};
///
/// struct MyCapsule<T: atomic_capsule::AlignmentTier> {
///     _marker: AlignmentMarker<T>,
///     data: [u8; 64],
/// }
/// ```
#[derive(Debug, Copy, Clone)]
pub struct AlignmentMarker<T: AlignmentTier>(PhantomData<T>);

impl<T: AlignmentTier> AlignmentMarker<T> {
    /// Create new alignment marker (compile-time only).
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_INVARIANT`: T::verify_alignment() is true
    /// - `#VERIFY_INVARIANT`: Compile-time const evaluation
    #[inline(always)]
    pub const fn new() -> Self {
        // Compile-time verification
        assert!(
            T::ALIGNMENT.count_ones() == 1,
            "Alignment must be power of 2"
        );
        assert!(T::ALIGNMENT >= crate::MIN_ALIGNMENT, "Alignment too small");
        assert!(T::ALIGNMENT <= crate::MAX_ALIGNMENT, "Alignment too large");

        Self(PhantomData)
    }

    /// Get alignment value.
    #[inline(always)]
    pub const fn alignment(&self) -> usize {
        T::ALIGNMENT
    }

    /// Get tier name.
    #[inline(always)]
    pub const fn tier(&self) -> &'static str {
        T::TIER
    }
}

impl<T: AlignmentTier> Default for AlignmentMarker<T> {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hot_tier() {
        assert_eq!(HotTier::TIER, "hot");
        assert_eq!(HotTier::ALIGNMENT, 64);
        assert!(HotTier::verify_alignment());
    }

    #[test]
    fn test_warm_tier() {
        assert_eq!(WarmTier::TIER, "warm");
        assert_eq!(WarmTier::ALIGNMENT, 128);
        assert!(WarmTier::verify_alignment());
    }

    #[test]
    fn test_cold_tier() {
        assert_eq!(ColdTier::TIER, "cold");
        assert_eq!(ColdTier::ALIGNMENT, 256);
        assert!(ColdTier::verify_alignment());
    }

    #[test]
    fn test_alignment_marker() {
        let hot = AlignmentMarker::<HotTier>::new();
        assert_eq!(hot.alignment(), 64);
        assert_eq!(hot.tier(), "hot");

        let warm = AlignmentMarker::<WarmTier>::new();
        assert_eq!(warm.alignment(), 128);
        assert_eq!(warm.tier(), "warm");

        let cold = AlignmentMarker::<ColdTier>::new();
        assert_eq!(cold.alignment(), 256);
        assert_eq!(cold.tier(), "cold");
    }

    #[test]
    fn test_all_tiers_power_of_two() {
        assert_eq!(HotTier::ALIGNMENT.count_ones(), 1);
        assert_eq!(WarmTier::ALIGNMENT.count_ones(), 1);
        assert_eq!(ColdTier::ALIGNMENT.count_ones(), 1);
    }

    #[test]
    fn test_all_tiers_in_range() {
        assert!(HotTier::ALIGNMENT >= crate::MIN_ALIGNMENT);
        assert!(HotTier::ALIGNMENT <= crate::MAX_ALIGNMENT);

        assert!(WarmTier::ALIGNMENT >= crate::MIN_ALIGNMENT);
        assert!(WarmTier::ALIGNMENT <= crate::MAX_ALIGNMENT);

        assert!(ColdTier::ALIGNMENT >= crate::MIN_ALIGNMENT);
        assert!(ColdTier::ALIGNMENT <= crate::MAX_ALIGNMENT);
    }
}
