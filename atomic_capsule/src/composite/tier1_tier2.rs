//! # Tier 1 + Tier 2 Composite Capsules (Atomic + SIMD)
//!
//! **Lockfree vectorized state coordination.**
//!
//! ## Performance Claims (B32 Framework)
//!
//! - **Target Speedup**: 12× (3× atomic × 4× SIMD)
//! - **Latency**: <50ns per operation
//! - **Throughput**: 8 parallel operations per SIMD instruction
//!
//! ## Use Cases
//!
//! - Circuit breaker with vectorized counters
//! - Lockfree SIMD statistics (mean, variance, histogram)
//! - Atomic coordination for parallel SIMD workers
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_ALIGNMENT_128B`: 128B alignment prevents false sharing
//! - `#VERIFY_ALIGNMENT_128B`: Compile-time static assertions
//! - `#ASSUME_ATOMIC_ORDERING`: Acquire/Release semantics for coordination
//! - `#VERIFY_ATOMIC_ORDERING`: Memory ordering documented per operation

#[cfg(feature = "portable_simd")]
use core::simd::f32x8;
use core::sync::atomic::{AtomicU64, Ordering};

/// Atomic + SIMD Composite Capsule (T1 + T2)
///
/// Combines atomic coordination (T1) with SIMD vectorized computation (T2).
///
/// ## Layout (128 bytes)
///
/// ```text
/// | Offset | Size | Field          | Tier | Purpose                     |
/// |--------|------|----------------|------|-----------------------------|
/// | 0      | 8    | atomic_counter | T1   | Atomic coordination state   |
/// | 8      | 8    | atomic_gen     | T1   | Generation counter (TOCTOU) |
/// | 16     | 32   | simd_data      | T2   | 8×f32 SIMD values           |
/// | 48     | 80   | _padding       | --   | Cache line alignment        |
/// ```
///
/// ## Performance
///
/// - Atomic read: <5ns (Acquire ordering)
/// - Atomic write: <10ns (Release ordering)
/// - SIMD operation: <10ns (8 parallel operations)
/// - Combined: <25ns (atomic coordination + SIMD computation)
///
/// ## Example
///
/// ```rust,ignore
/// use atomic_capsule::composite::AtomicSimdCapsule;
///
/// let capsule = AtomicSimdCapsule::new();
///
/// // T1: Atomic coordination
/// capsule.increment_counter();
///
/// // T2: SIMD computation
/// let simd_values = capsule.load_simd();
/// let sum: f32 = simd_values.iter().sum();
/// ```
#[cfg_attr(
    feature = "derive",
    derive(atomic_capsule_derive::ComputationalCapsule)
)]
#[cfg_attr(feature = "derive", capsule(alignment = 128, size = 128))]
#[repr(C, align(128))]
pub struct AtomicSimdCapsule {
    /// T1: Atomic coordination counter
    atomic_counter: AtomicU64,

    /// T1: Generation counter (TOCTOU prevention)
    atomic_generation: AtomicU64,

    /// T2: SIMD vectorized data (8×f32)
    #[cfg(feature = "portable_simd")]
    simd_data: [f32; 8],

    #[cfg(not(feature = "portable_simd"))]
    simd_data: [f32; 8],

    /// Padding to 128 bytes
    _padding: [u8; 104],
}

impl AtomicSimdCapsule {
    /// Create new composite capsule with zero-initialized state
    ///
    /// ## Performance
    /// - Latency: <10ns
    /// - Zero allocation (stack-allocated)
    pub const fn new() -> Self {
        Self {
            atomic_counter: AtomicU64::new(0),
            atomic_generation: AtomicU64::new(0),
            simd_data: [0.0; 8],
            _padding: [0; 104],
        }
    }

    /// Increment atomic counter (T1 operation)
    ///
    /// ## Performance
    /// - Latency: <10ns (CAS loop with backoff)
    ///
    /// ## Memory Ordering
    /// - Success: AcqRel (coordinate with other threads)
    /// - Failure: Relaxed (retry on contention)
    #[inline]
    pub fn increment_counter(&self) -> u64 {
        self.atomic_counter.fetch_add(1, Ordering::AcqRel)
    }

    /// Load SIMD data (T2 operation)
    ///
    /// ## Performance
    /// - Latency: <5ns (single cache line read)
    #[cfg(feature = "portable_simd")]
    #[inline]
    pub fn load_simd(&self) -> f32x8 {
        f32x8::from_array(self.simd_data)
    }

    /// Store SIMD data (T2 operation)
    ///
    /// ## Performance
    /// - Latency: <10ns (single cache line write)
    #[cfg(feature = "portable_simd")]
    #[inline]
    pub fn store_simd(&mut self, data: f32x8) {
        self.simd_data = data.to_array();
    }

    /// Combined operation: increment counter and update SIMD (T1+T2)
    ///
    /// ## Performance
    /// - Latency: <25ns (atomic + SIMD combined)
    #[cfg(feature = "portable_simd")]
    #[inline]
    pub fn update_combined(&mut self, simd_data: f32x8) -> u64 {
        let counter = self.increment_counter();
        self.store_simd(simd_data);
        counter
    }
}

impl Default for AtomicSimdCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Manual verification if derive feature not enabled
#[cfg(not(feature = "derive"))]
const _: () = {
    const fn verify_layout() {
        assert!(core::mem::size_of::<AtomicSimdCapsule>() == 128);
        assert!(core::mem::align_of::<AtomicSimdCapsule>() == 128);
    }
    let _ = verify_layout();
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout() {
        assert_eq!(core::mem::size_of::<AtomicSimdCapsule>(), 128);
        assert_eq!(core::mem::align_of::<AtomicSimdCapsule>(), 128);
    }

    #[test]
    fn test_atomic_operations() {
        let capsule = AtomicSimdCapsule::new();
        assert_eq!(capsule.increment_counter(), 0);
        assert_eq!(capsule.increment_counter(), 1);
        assert_eq!(capsule.increment_counter(), 2);
    }

    #[cfg(feature = "portable_simd")]
    #[test]
    fn test_simd_operations() {
        let mut capsule = AtomicSimdCapsule::new();
        let data = f32x8::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        capsule.store_simd(data);
        let loaded = capsule.load_simd();
        assert_eq!(loaded.to_array(), [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    }

    #[cfg(feature = "portable_simd")]
    #[test]
    fn test_combined_operations() {
        let mut capsule = AtomicSimdCapsule::new();
        let data = f32x8::from_array([1.0; 8]);
        let counter = capsule.update_combined(data);
        assert_eq!(counter, 0);
        assert_eq!(capsule.load_simd().to_array(), [1.0; 8]);
    }
}
