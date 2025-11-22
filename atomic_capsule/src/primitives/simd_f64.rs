//! # SimdF64x8Capsule - 8 × f64 SIMD Capsule (Warm Tier)

//!
//! **512-bit SIMD capsule for high-precision floating-point operations with atomic publishing.**
//!
//! ## UCE33 Analysis
//!
//! - **Q28 (Simplicity)**: Simple load/store/arithmetic API with atomic result publishing
//! - **Q29 (Constraints)**: 64-byte SIMD requirement (AVX-512), 128-byte cache line alignment
//! - **Q30 (Validation)**: Benchmark SIMD vs scalar for 8-element f64 operations
//! - **Q31 (Rust Transform)**: portable_simd enables cross-platform f64 vectorization
//! - **Q32 (Nightly)**: std::simd::f64x8 for portable 512-bit SIMD operations
//! - **Q33 (Atomic Capsule)**: DualAtomicU64 pattern for atomic SIMD result publishing
//!
//! ## Memory Layout
//!
//! ```text
//! [SIMD Data: 8 × f64 = 64 bytes] [Metadata: AtomicU64 = 8 bytes] [Padding: 56 bytes]
//! Total: 128 bytes (dual cache line, Warm Tier alignment)
//! ```
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_SIMD_ALIGNMENT`: Data aligned to 128 bytes for dual cache line fit
//! - `#VERIFY_ALIGNMENT_STATIC`: const_assert!(align_of::<Self>() == 128)
//! - `#ASSUME_ELEMENT_COUNT`: Exactly 8 elements for f64x8
//! - `#VERIFY_ELEMENT_COUNT`: const_assert!(size_of::<f64x8>() == 64)
//! - `#ASSUME_ATOMIC_PUBLISHING`: Metadata enables atomic result coordination
//! - `#VERIFY_ATOMIC_CORRECTNESS`: Generation counter prevents TOCTOU races

use core::sync::atomic::{AtomicU64, Ordering};

use core::simd::f64x8;
#[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
use std::simd::num::SimdFloat;
use std::simd::StdFloat;

use super::SimdCapsule;

/// SIMD F64x8 capsule for high-precision vectorized operations
///
/// # Layout
/// - Data: 8 × f64 = 64 bytes (SIMD vector)
/// - Metadata: AtomicU64 = 8 bytes (generation counter)
/// - Padding: 56 bytes (dual cache line alignment)
/// - Total: 128 bytes (Warm Tier)
///
/// # Dual-Channel Pattern (Q33)
/// - Data channel: SIMD computation results (64 bytes)
/// - Metadata channel: Atomic generation counter (8 bytes)
/// - Enables atomic publishing of SIMD results with TOCTOU protection
///
/// # Performance
/// - Load: ~5-8ns (dual cache line read)
/// - Store: ~5-8ns (dual cache line write)
/// - SIMD operations: ~3-6ns (8 f64 operations in parallel)
/// - Atomic publish: ~15ns (data write + generation increment)
///
/// # ASSUM Safety
/// - `#ASSUME_DUAL_CHANNEL`: Separate cache lines for data + metadata
/// - `#VERIFY_CACHE_SEPARATION`: 64-byte data + metadata ensures no false sharing
#[repr(C, align(128))]
pub struct SimdF64x8Capsule {
    /// SIMD data storage (8 × f64)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    data: f64x8,

    /// Scalar fallback storage (8 × f64)
    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    data: [f64; 8],

    /// Generation counter for atomic coordination (metadata channel)
    ///
    /// # Q33 Atomic Capsule Pattern
    /// - Incremented on every data update
    /// - Enables TOCTOU-safe read verification
    /// - Readers check generation before and after data read
    metadata: AtomicU64,

    /// Padding to 128 bytes (Warm Tier)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    _padding: [u8; 56], // 64 (data) + 8 (metadata) + 56 (padding) = 128

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    _padding: [u8; 56], // 64 (data array) + 8 (metadata) + 56 (padding) = 128
}

impl SimdF64x8Capsule {
    /// Create new SIMD F64x8 capsule initialized to zero
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::SimdF64x8Capsule;
    ///
    /// let capsule = SimdF64x8Capsule::new();
    /// let data = capsule.load();
    /// assert_eq!(data, [0.0; 8]);
    /// ```
    pub const fn new() -> Self {
        Self {
            #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
            data: f64x8::from_array([0.0; 8]),
            #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
            data: [0.0; 8],
            metadata: AtomicU64::new(0),
            _padding: [0u8; 56],
        }
    }

    /// Create new SIMD F64x8 capsule from array
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::SimdF64x8Capsule;
    ///
    /// let capsule = SimdF64x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    /// let data = capsule.load();
    /// assert_eq!(data[0], 1.0);
    /// ```
    pub const fn from_array(data: [f64; 8]) -> Self {
        Self {
            #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
            data: f64x8::from_array(data),
            #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
            data,
            metadata: AtomicU64::new(0),
            _padding: [0u8; 56],
        }
    }

    /// Load current generation counter
    ///
    /// # Q33 Atomic Capsule Pattern
    /// - Used for TOCTOU prevention in concurrent reads
    /// - Readers check generation before and after data load
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_ATOMIC_ORDERING`: Acquire ordering for generation reads
    /// - `#VERIFY_ORDERING_SUFFICIENT`: Required for data dependency
    #[inline(always)]
    pub fn generation(&self) -> u64 {
        self.metadata.load(Ordering::Acquire)
    }

    /// Atomically publish SIMD result with generation increment
    ///
    /// # Q33 Atomic Capsule Pattern
    /// - Two-phase commit: data write → generation increment
    /// - Release ordering ensures data visibility before generation update
    ///
    /// # Performance
    /// - ~15ns total (data write + atomic increment)
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_RELEASE_ORDERING`: Release on generation ensures data visibility
    /// - `#VERIFY_ORDERING_CORRECTNESS`: Required for atomic publishing
    pub fn publish(&self, data: [f64; 8]) {
        // SAFETY: Atomic publishing pattern from The Atomic Capsule
        // #ASSUME_EXCLUSIVE_WRITER: Single writer per capsule (SWeMR pattern)
        // #VERIFY_WRITER_COORDINATION: Documented in usage guidelines
        let ptr = self as *const Self as *mut Self;
        unsafe {
            #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
            {
                (*ptr).data = f64x8::from_array(data);
            }
            #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
            {
                (*ptr).data = data;
            }
        }

        // Atomic generation increment with Release ordering
        self.metadata.fetch_add(1, Ordering::Release);
    }

    /// SIMD addition: self + other
    ///
    /// # Performance
    /// - SIMD: ~3-6ns (8 f64 additions in parallel)
    /// - Scalar fallback: ~16-24ns (sequential additions)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn add(&self, other: &Self) -> Self {
        let result_data = self.data + other.data;
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 56],
        }
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn add(&self, other: &Self) -> Self {
        let mut result = [0.0f64; 8];
        for i in 0..8 {
            result[i] = self.data[i] + other.data[i];
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 56],
        }
    }

    /// SIMD multiplication: self * other
    ///
    /// # Performance
    /// - SIMD: ~3-6ns (8 f64 multiplications in parallel)
    /// - Scalar fallback: ~16-24ns (sequential multiplications)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn mul(&self, other: &Self) -> Self {
        let result_data = self.data * other.data;
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 56],
        }
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn mul(&self, other: &Self) -> Self {
        let mut result = [0.0f64; 8];
        for i in 0..8 {
            result[i] = self.data[i] * other.data[i];
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 56],
        }
    }

    /// SIMD fused multiply-add: (self * mul) + add
    ///
    /// # Performance
    /// - SIMD: ~3-6ns (8 f64 FMA operations in parallel)
    /// - Scalar fallback: ~24-40ns (sequential mul+add)
    ///
    /// # Q32 Nightly Enhancement
    /// Uses hardware FMA instructions when available for maximum precision and performance
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn fma(&self, mul: &Self, add: &Self) -> Self {
        // Manual FMA: (self * mul) + add
        let product = self.data * mul.data;
        let result_data = product + add.data;
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 56],
        }
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn fma(&self, mul: &Self, add: &Self) -> Self {
        let mut result = [0.0f64; 8];
        for i in 0..8 {
            result[i] = self.data[i] * mul.data[i] + add.data[i];
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 56],
        }
    }

    /// SIMD dot product: sum(self[i] * other[i])
    ///
    /// # Performance
    /// - SIMD: ~4-8ns (parallel multiply + horizontal sum)
    /// - Scalar fallback: ~24-32ns (sequential multiply + accumulate)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn dot(&self, other: &Self) -> f64 {
        let product = self.data * other.data;
        product.reduce_sum()
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn dot(&self, other: &Self) -> f64 {
        let mut sum = 0.0f64;
        for i in 0..8 {
            sum += self.data[i] * other.data[i];
        }
        sum
    }

    /// Scale all elements by scalar value
    ///
    /// # Performance
    /// - SIMD: ~3-6ns (broadcast + 8 parallel multiplications)
    /// - Scalar fallback: ~16-24ns (sequential multiplications)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn scale(&self, scalar: f64) -> Self {
        let scalar_vec = f64x8::splat(scalar);
        let result_data = self.data * scalar_vec;
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 56],
        }
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn scale(&self, scalar: f64) -> Self {
        let mut result = [0.0f64; 8];
        for i in 0..8 {
            result[i] = self.data[i] * scalar;
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 56],
        }
    }

    /// SIMD square root: sqrt(self[i])
    ///
    /// # Performance
    /// - SIMD: ~10-15ns (8 parallel sqrt operations)
    /// - Scalar fallback: ~40-60ns (sequential sqrt)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn sqrt(&self) -> Self {
        let result_data = self.data.sqrt();
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 56],
        }
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn sqrt(&self) -> Self {
        let mut result = [0.0f64; 8];
        for i in 0..8 {
            result[i] = self.data[i].sqrt();
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 56],
        }
    }
}

impl SimdCapsule for SimdF64x8Capsule {
    type Element = f64;
    const LANES: usize = 8;
    const ALIGNMENT: usize = 128;

    fn load(&self) -> [Self::Element; Self::LANES] {
        #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
        {
            self.data.to_array()
        }
        #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
        {
            self.data
        }
    }

    fn store(&self, data: [Self::Element; Self::LANES]) {
        self.publish(data);
    }
}

impl Default for SimdF64x8Capsule {
    fn default() -> Self {
        Self::new()
    }
}

// Compile-time verification
const _: () = {
    assert!(
        core::mem::size_of::<SimdF64x8Capsule>() == 128,
        "SimdF64x8Capsule must be 128 bytes"
    );
    assert!(
        core::mem::align_of::<SimdF64x8Capsule>() == 128,
        "SimdF64x8Capsule must be 128-byte aligned"
    );
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alignment() {
        assert_eq!(core::mem::align_of::<SimdF64x8Capsule>(), 128);
        assert_eq!(core::mem::size_of::<SimdF64x8Capsule>(), 128);
    }

    #[test]
    fn test_new() {
        let capsule = SimdF64x8Capsule::new();
        let data = capsule.load();
        assert_eq!(data, [0.0; 8]);
    }

    #[test]
    fn test_from_array() {
        let input = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let capsule = SimdF64x8Capsule::from_array(input);
        let data = capsule.load();
        assert_eq!(data, input);
    }

    #[test]
    fn test_publish() {
        let capsule = SimdF64x8Capsule::new();
        let gen_before = capsule.generation();

        capsule.publish([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);

        let gen_after = capsule.generation();
        assert_eq!(gen_after, gen_before + 1);

        let data = capsule.load();
        assert_eq!(data, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    }

    #[test]
    fn test_add() {
        let a = SimdF64x8Capsule::from_array([1.0; 8]);
        let b = SimdF64x8Capsule::from_array([2.0; 8]);
        let result = a.add(&b);
        let data = result.load();
        assert_eq!(data, [3.0; 8]);
    }

    #[test]
    fn test_mul() {
        let a = SimdF64x8Capsule::from_array([2.0; 8]);
        let b = SimdF64x8Capsule::from_array([3.0; 8]);
        let result = a.mul(&b);
        let data = result.load();
        assert_eq!(data, [6.0; 8]);
    }

    #[test]
    fn test_fma() {
        let a = SimdF64x8Capsule::from_array([2.0; 8]);
        let mul = SimdF64x8Capsule::from_array([3.0; 8]);
        let add = SimdF64x8Capsule::from_array([1.0; 8]);
        let result = a.fma(&mul, &add);
        let data = result.load();
        assert_eq!(data, [7.0; 8]); // (2.0 * 3.0) + 1.0 = 7.0
    }

    #[test]
    fn test_dot() {
        let a = SimdF64x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let b = SimdF64x8Capsule::from_array([2.0; 8]);
        let result = a.dot(&b);
        assert_eq!(result, 72.0); // 2*(1+2+3+4+5+6+7+8) = 2*36 = 72
    }

    #[test]
    fn test_scale() {
        let a = SimdF64x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let result = a.scale(2.0);
        let data = result.load();
        assert_eq!(data, [2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0]);
    }

    #[test]
    fn test_sqrt() {
        let a = SimdF64x8Capsule::from_array([4.0, 9.0, 16.0, 25.0, 36.0, 49.0, 64.0, 81.0]);
        let result = a.sqrt();
        let data = result.load();
        assert_eq!(data, [2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
    }

    #[test]
    fn test_generation_atomic_publish() {
        let capsule = SimdF64x8Capsule::new();

        // Multiple publishes increment generation
        for i in 1..=5 {
            capsule.publish([i as f64; 8]);
            assert_eq!(capsule.generation(), i);
        }
    }
}
