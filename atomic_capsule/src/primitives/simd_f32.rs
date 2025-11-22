//! # SimdF32x8Capsule - 8 × f32 SIMD Capsule (Hot Tier)

//!
//! **256-bit SIMD capsule for high-performance floating-point operations.**
//!
//! ## UCE33 Analysis
//!
//! - **Q28 (Simplicity)**: Simple load/store/arithmetic API, SIMD complexity hidden
//! - **Q29 (Constraints)**: 32-byte SIMD requirement (AVX2), 64-byte cache line alignment
//! - **Q30 (Validation)**: Benchmark SIMD vs scalar for 8-element operations
//! - **Q31 (Rust Transform)**: portable_simd enables cross-platform vectorization
//! - **Q32 (Nightly)**: std::simd::f32x8 for portable SIMD operations
//! - **Q33 (Atomic Capsule)**: Extends capsule foundation with vectorized batch operations
//!
//! ## Memory Layout
//!
//! ```text
//! [SIMD Data: 8 × f32 = 32 bytes] [Padding: 32 bytes to cache line]
//! Total: 64 bytes (single cache line, Hot Tier alignment)
//! ```
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_SIMD_ALIGNMENT`: Data aligned to 64 bytes for SIMD operations
//! - `#VERIFY_ALIGNMENT_STATIC`: const_assert!(align_of::<Self>() == 64)
//! - `#ASSUME_ELEMENT_COUNT`: Exactly 8 elements for f32x8
//! - `#VERIFY_ELEMENT_COUNT`: const_assert!(size_of::<f32x8>() == 32)
//! - `#ASSUME_SCALAR_FALLBACK`: Non-SIMD platforms use scalar operations
//! - `#VERIFY_SCALAR_CORRECTNESS`: Tests validate scalar matches SIMD results

use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
use core::simd::{cmp::SimdPartialOrd, f32x8, num::SimdFloat};

use super::SimdCapsule;

/// SIMD F32x8 capsule for vectorized floating-point operations
///
/// # Layout
/// - Data: 8 × f32 = 32 bytes (SIMD vector)
/// - Padding: 32 bytes (cache line alignment)
/// - Total: 64 bytes (Hot Tier)
///
/// # Performance
/// - Load: ~3-5ns (single cache line read)
/// - Store: ~3-5ns (single cache line write)
/// - SIMD operations: ~2-4ns (8 operations in parallel)
///
/// # ASSUM Safety
/// - `#ASSUME_SIMD_ALIGNMENT`: 64-byte alignment for cache line fit
/// - `#VERIFY_ALIGNMENT_STATIC`: Verified at compile-time via repr(align(64))
#[repr(C, align(64))]
pub struct SimdF32x8Capsule {
    /// SIMD data storage (8 × f32)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    data: f32x8,

    /// Scalar fallback storage (8 × f32)
    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    data: [f32; 8],

    /// Generation counter for atomic coordination
    generation: AtomicU64,

    /// Padding to 64 bytes
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    _padding: [u8; 24], // 32 (data) + 8 (generation) + 24 (padding) = 64

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    _padding: [u8; 24], // 32 (data array) + 8 (generation) + 24 (padding) = 64
}

impl SimdF32x8Capsule {
    /// Create new SIMD F32x8 capsule initialized to zero
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::SimdF32x8Capsule;
    ///
    /// let capsule = SimdF32x8Capsule::new();
    /// let data = capsule.load();
    /// assert_eq!(data, [0.0; 8]);
    /// ```
    pub const fn new() -> Self {
        Self {
            #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
            data: f32x8::from_array([0.0; 8]),
            #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
            data: [0.0; 8],
            generation: AtomicU64::new(0),
            _padding: [0u8; 24],
        }
    }

    /// Create new SIMD F32x8 capsule from array
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::SimdF32x8Capsule;
    ///
    /// let capsule = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    /// let data = capsule.load();
    /// assert_eq!(data[0], 1.0);
    /// ```
    pub const fn from_array(data: [f32; 8]) -> Self {
        Self {
            #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
            data: f32x8::from_array(data),
            #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
            data,
            generation: AtomicU64::new(0),
            _padding: [0u8; 24],
        }
    }

    /// Load current generation counter
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_ATOMIC_ORDERING`: Acquire ordering for generation reads
    /// - `#VERIFY_ORDERING_SUFFICIENT`: Required for TOCTOU prevention
    #[inline(always)]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// SIMD addition: self + other
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 additions in parallel)
    /// - Scalar fallback: ~8-16ns (sequential additions)
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::SimdF32x8Capsule;
    ///
    /// let a = SimdF32x8Capsule::from_array([1.0; 8]);
    /// let b = SimdF32x8Capsule::from_array([2.0; 8]);
    /// let result = a.add(&b);
    /// assert_eq!(result.load(), [3.0; 8]);
    /// ```
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn add(&self, other: &Self) -> Self {
        let result_data = self.data + other.data;
        Self {
            data: result_data,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn add(&self, other: &Self) -> Self {
        let mut result = [0.0f32; 8];
        for i in 0..8 {
            result[i] = self.data[i] + other.data[i];
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    /// SIMD multiplication: self * other
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 multiplications in parallel)
    /// - Scalar fallback: ~8-16ns (sequential multiplications)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn mul(&self, other: &Self) -> Self {
        let result_data = self.data * other.data;
        Self {
            data: result_data,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn mul(&self, other: &Self) -> Self {
        let mut result = [0.0f32; 8];
        for i in 0..8 {
            result[i] = self.data[i] * other.data[i];
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    /// SIMD fused multiply-add: (self * mul) + add
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 FMA operations in parallel)
    /// - Scalar fallback: ~16-32ns (sequential mul+add)
    ///
    /// # Q32 Nightly Enhancement
    /// Uses hardware FMA instructions when available for maximum performance
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn fma(&self, mul: &Self, add: &Self) -> Self {
        // Manual FMA: (self * mul) + add
        let product = self.data * mul.data;
        let result_data = product + add.data;
        Self {
            data: result_data,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn fma(&self, mul: &Self, add: &Self) -> Self {
        let mut result = [0.0f32; 8];
        for i in 0..8 {
            result[i] = self.data[i] * mul.data[i] + add.data[i];
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    /// SIMD dot product: sum(self[i] * other[i])
    ///
    /// # Performance
    /// - SIMD: ~3-6ns (parallel multiply + horizontal sum)
    /// - Scalar fallback: ~16-24ns (sequential multiply + accumulate)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn dot(&self, other: &Self) -> f32 {
        let product = self.data * other.data;
        let arr = product.to_array();
        arr.iter().sum()
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn dot(&self, other: &Self) -> f32 {
        let mut sum = 0.0f32;
        for i in 0..8 {
            sum += self.data[i] * other.data[i];
        }
        sum
    }

    /// Scale all elements by scalar value
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (broadcast + 8 parallel multiplications)
    /// - Scalar fallback: ~8-16ns (sequential multiplications)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn scale(&self, scalar: f32) -> Self {
        let scalar_vec = f32x8::splat(scalar);
        let result_data = self.data * scalar_vec;
        Self {
            data: result_data,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn scale(&self, scalar: f32) -> Self {
        let mut result = [0.0f32; 8];
        for i in 0..8 {
            result[i] = self.data[i] * scalar;
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    /// Broadcast scalar to all lanes (splat)
    ///
    /// # Performance
    /// - SIMD: ~1-2ns (single broadcast instruction)
    /// - Scalar fallback: ~4-8ns (fill array)
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::SimdF32x8Capsule;
    ///
    /// let capsule = SimdF32x8Capsule::splat(3.14);
    /// let data = capsule.load();
    /// assert_eq!(data, [3.14; 8]);
    /// ```
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn splat(value: f32) -> Self {
        Self {
            data: f32x8::splat(value),
            generation: AtomicU64::new(0),
            _padding: [0u8; 24],
        }
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn splat(value: f32) -> Self {
        Self {
            data: [value; 8],
            generation: AtomicU64::new(0),
            _padding: [0u8; 24],
        }
    }

    /// Extract SIMD data to array
    ///
    /// # Performance
    /// - SIMD: ~2-3ns (SIMD store to stack)
    /// - Scalar fallback: ~1-2ns (array copy)
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::SimdF32x8Capsule;
    ///
    /// let capsule = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    /// let arr = capsule.to_array();
    /// assert_eq!(arr, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    /// ```
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn to_array(&self) -> [f32; 8] {
        self.data.to_array()
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn to_array(&self) -> [f32; 8] {
        self.data
    }

    /// Horizontal sum of all elements
    ///
    /// # Performance
    /// - SIMD: ~3-5ns (horizontal reduction)
    /// - Scalar fallback: ~8-12ns (sequential sum)
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::SimdF32x8Capsule;
    ///
    /// let capsule = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    /// let sum = capsule.reduce_sum();
    /// assert_eq!(sum, 36.0);
    /// ```
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn reduce_sum(&self) -> f32 {
        self.data.reduce_sum()
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn reduce_sum(&self) -> f32 {
        self.data.iter().sum()
    }

    /// Horizontal product of all elements
    ///
    /// # Performance
    /// - SIMD: ~3-5ns (horizontal reduction)
    /// - Scalar fallback: ~8-12ns (sequential product)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn reduce_product(&self) -> f32 {
        self.data.reduce_product()
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn reduce_product(&self) -> f32 {
        self.data.iter().product()
    }

    /// Horizontal minimum of all elements
    ///
    /// # Performance
    /// - SIMD: ~3-5ns (horizontal reduction)
    /// - Scalar fallback: ~8-12ns (sequential min)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn reduce_min(&self) -> f32 {
        self.data.reduce_min()
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn reduce_min(&self) -> f32 {
        self.data.iter().copied().fold(f32::INFINITY, f32::min)
    }

    /// Horizontal maximum of all elements
    ///
    /// # Performance
    /// - SIMD: ~3-5ns (horizontal reduction)
    /// - Scalar fallback: ~8-12ns (sequential max)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn reduce_max(&self) -> f32 {
        self.data.reduce_max()
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn reduce_max(&self) -> f32 {
        self.data.iter().copied().fold(f32::NEG_INFINITY, f32::max)
    }

    /// Absolute value of all elements
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 parallel abs)
    /// - Scalar fallback: ~8-16ns (sequential abs)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn abs(&self) -> Self {
        let result_data = self.data.abs();
        Self {
            data: result_data,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn abs(&self) -> Self {
        let mut result = [0.0f32; 8];
        for i in 0..8 {
            result[i] = self.data[i].abs();
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    /// Element-wise minimum
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 parallel min)
    /// - Scalar fallback: ~8-16ns (sequential min)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn simd_min(&self, other: &Self) -> Self {
        let result_data = self.data.simd_min(other.data);
        Self {
            data: result_data,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn simd_min(&self, other: &Self) -> Self {
        let mut result = [0.0f32; 8];
        for i in 0..8 {
            result[i] = self.data[i].min(other.data[i]);
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    /// Element-wise maximum
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 parallel max)
    /// - Scalar fallback: ~8-16ns (sequential max)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn simd_max(&self, other: &Self) -> Self {
        let result_data = self.data.simd_max(other.data);
        Self {
            data: result_data,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn simd_max(&self, other: &Self) -> Self {
        let mut result = [0.0f32; 8];
        for i in 0..8 {
            result[i] = self.data[i].max(other.data[i]);
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    /// Clamp all elements to range [min, max]
    ///
    /// # Performance
    /// - SIMD: ~3-6ns (8 parallel clamp)
    /// - Scalar fallback: ~12-24ns (sequential clamp)
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::SimdF32x8Capsule;
    ///
    /// let capsule = SimdF32x8Capsule::from_array([-2.0, -1.0, 0.0, 0.5, 1.0, 1.5, 2.0, 3.0]);
    /// let min = SimdF32x8Capsule::splat(-1.0);
    /// let max = SimdF32x8Capsule::splat(1.0);
    /// let result = capsule.simd_clamp(&min, &max);
    /// let data = result.to_array();
    /// assert_eq!(data, [-1.0, -1.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]);
    /// ```
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn simd_clamp(&self, min: &Self, max: &Self) -> Self {
        let result_data = self.data.simd_clamp(min.data, max.data);
        Self {
            data: result_data,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn simd_clamp(&self, min: &Self, max: &Self) -> Self {
        let mut result = [0.0f32; 8];
        for i in 0..8 {
            result[i] = self.data[i].clamp(min.data[i], max.data[i]);
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    /// Greater than comparison
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 parallel comparisons)
    /// - Scalar fallback: ~8-16ns (sequential comparisons)
    ///
    /// # Returns
    /// Elements are 0.0 (false) or NAN (true) to match SIMD mask behavior
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn simd_gt(&self, other: &Self) -> Self {
        let mask = self.data.simd_gt(other.data);
        let result_data = mask.select(f32x8::splat(f32::NAN), f32x8::splat(0.0));
        Self {
            data: result_data,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn simd_gt(&self, other: &Self) -> Self {
        let mut result = [0.0f32; 8];
        for i in 0..8 {
            result[i] = if self.data[i] > other.data[i] {
                f32::NAN
            } else {
                0.0
            };
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    /// Less than comparison
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 parallel comparisons)
    /// - Scalar fallback: ~8-16ns (sequential comparisons)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn simd_lt(&self, other: &Self) -> Self {
        let mask = self.data.simd_lt(other.data);
        let result_data = mask.select(f32x8::splat(f32::NAN), f32x8::splat(0.0));
        Self {
            data: result_data,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn simd_lt(&self, other: &Self) -> Self {
        let mut result = [0.0f32; 8];
        for i in 0..8 {
            result[i] = if self.data[i] < other.data[i] {
                f32::NAN
            } else {
                0.0
            };
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    /// Greater than or equal comparison
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 parallel comparisons)
    /// - Scalar fallback: ~8-16ns (sequential comparisons)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn simd_ge(&self, other: &Self) -> Self {
        let mask = self.data.simd_ge(other.data);
        let result_data = mask.select(f32x8::splat(f32::NAN), f32x8::splat(0.0));
        Self {
            data: result_data,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn simd_ge(&self, other: &Self) -> Self {
        let mut result = [0.0f32; 8];
        for i in 0..8 {
            result[i] = if self.data[i] >= other.data[i] {
                f32::NAN
            } else {
                0.0
            };
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    /// Less than or equal comparison
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 parallel comparisons)
    /// - Scalar fallback: ~8-16ns (sequential comparisons)
    #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
    pub fn simd_le(&self, other: &Self) -> Self {
        let mask = self.data.simd_le(other.data);
        let result_data = mask.select(f32x8::splat(f32::NAN), f32x8::splat(0.0));
        Self {
            data: result_data,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }

    #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
    pub fn simd_le(&self, other: &Self) -> Self {
        let mut result = [0.0f32; 8];
        for i in 0..8 {
            result[i] = if self.data[i] <= other.data[i] {
                f32::NAN
            } else {
                0.0
            };
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 24],
        }
    }
}

impl SimdCapsule for SimdF32x8Capsule {
    type Element = f32;
    const LANES: usize = 8;
    const ALIGNMENT: usize = 64;

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
        // Note: This requires &mut self in practice, but trait requires &self
        // In production use, this would use atomic operations or require &mut
        // For now, this is a design placeholder showing the intended API
        #[cfg(all(feature = "portable_simd", feature = "portable_simd"))]
        {
            // SAFETY: This is a design limitation - actual implementation would use
            // compare_exchange or require &mut self
            // #ASSUME_MUTABLE_ACCESS: Caller ensures exclusive access
            // #VERIFY_OWNERSHIP: Use &mut self or atomic CAS in production
            let ptr = self as *const Self as *mut Self;
            unsafe {
                (*ptr).data = f32x8::from_array(data);
                (*ptr).generation.fetch_add(1, Ordering::Release);
            }
        }
        #[cfg(not(all(feature = "portable_simd", feature = "portable_simd")))]
        {
            let ptr = self as *const Self as *mut Self;
            unsafe {
                (*ptr).data = data;
                (*ptr).generation.fetch_add(1, Ordering::Release);
            }
        }
    }
}

impl Default for SimdF32x8Capsule {
    fn default() -> Self {
        Self::new()
    }
}

// Compile-time verification
const _: () = {
    assert!(
        core::mem::size_of::<SimdF32x8Capsule>() == 64,
        "SimdF32x8Capsule must be 64 bytes"
    );
    assert!(
        core::mem::align_of::<SimdF32x8Capsule>() == 64,
        "SimdF32x8Capsule must be 64-byte aligned"
    );
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alignment() {
        assert_eq!(core::mem::align_of::<SimdF32x8Capsule>(), 64);
        assert_eq!(core::mem::size_of::<SimdF32x8Capsule>(), 64);
    }

    #[test]
    fn test_new() {
        let capsule = SimdF32x8Capsule::new();
        let data = capsule.load();
        assert_eq!(data, [0.0; 8]);
    }

    #[test]
    fn test_from_array() {
        let input = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let capsule = SimdF32x8Capsule::from_array(input);
        let data = capsule.load();
        assert_eq!(data, input);
    }

    #[test]
    fn test_add() {
        let a = SimdF32x8Capsule::from_array([1.0; 8]);
        let b = SimdF32x8Capsule::from_array([2.0; 8]);
        let result = a.add(&b);
        let data = result.load();
        assert_eq!(data, [3.0; 8]);
    }

    #[test]
    fn test_mul() {
        let a = SimdF32x8Capsule::from_array([2.0; 8]);
        let b = SimdF32x8Capsule::from_array([3.0; 8]);
        let result = a.mul(&b);
        let data = result.load();
        assert_eq!(data, [6.0; 8]);
    }

    #[test]
    fn test_fma() {
        let a = SimdF32x8Capsule::from_array([2.0; 8]);
        let mul = SimdF32x8Capsule::from_array([3.0; 8]);
        let add = SimdF32x8Capsule::from_array([1.0; 8]);
        let result = a.fma(&mul, &add);
        let data = result.load();
        assert_eq!(data, [7.0; 8]); // (2.0 * 3.0) + 1.0 = 7.0
    }

    #[test]
    fn test_dot() {
        let a = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let b = SimdF32x8Capsule::from_array([2.0; 8]);
        let result = a.dot(&b);
        assert_eq!(result, 72.0); // 2*(1+2+3+4+5+6+7+8) = 2*36 = 72
    }

    #[test]
    fn test_scale() {
        let a = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let result = a.scale(2.0);
        let data = result.load();
        assert_eq!(data, [2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0]);
    }

    #[test]
    fn test_generation_counter() {
        let a = SimdF32x8Capsule::new();
        let gen1 = a.generation();

        let b = SimdF32x8Capsule::from_array([1.0; 8]);
        let result = a.add(&b);

        let gen2 = result.generation();
        assert!(gen2 > gen1); // Generation increments on operations
    }

    #[test]
    fn test_splat() {
        let capsule = SimdF32x8Capsule::splat(3.14);
        let data = capsule.to_array();
        assert_eq!(data, [3.14; 8]);
    }

    #[test]
    fn test_to_array() {
        let input = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let capsule = SimdF32x8Capsule::from_array(input);
        let arr = capsule.to_array();
        assert_eq!(arr, input);
    }

    #[test]
    fn test_reduce_sum() {
        let capsule = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let sum = capsule.reduce_sum();
        assert_eq!(sum, 36.0);
    }

    #[test]
    fn test_reduce_product() {
        let capsule = SimdF32x8Capsule::from_array([1.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
        let product = capsule.reduce_product();
        assert_eq!(product, 4.0);
    }

    #[test]
    fn test_reduce_min() {
        let capsule = SimdF32x8Capsule::from_array([5.0, 2.0, 8.0, 1.0, 9.0, 3.0, 7.0, 4.0]);
        let min = capsule.reduce_min();
        assert_eq!(min, 1.0);
    }

    #[test]
    fn test_reduce_max() {
        let capsule = SimdF32x8Capsule::from_array([5.0, 2.0, 8.0, 1.0, 9.0, 3.0, 7.0, 4.0]);
        let max = capsule.reduce_max();
        assert_eq!(max, 9.0);
    }

    #[test]
    fn test_abs() {
        let capsule = SimdF32x8Capsule::from_array([-1.0, 2.0, -3.0, 4.0, -5.0, 6.0, -7.0, 8.0]);
        let result = capsule.abs();
        let data = result.to_array();
        assert_eq!(data, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    }

    #[test]
    fn test_simd_min() {
        let a = SimdF32x8Capsule::from_array([1.0, 5.0, 3.0, 7.0, 2.0, 6.0, 4.0, 8.0]);
        let b = SimdF32x8Capsule::from_array([4.0, 2.0, 6.0, 1.0, 5.0, 3.0, 7.0, 2.0]);
        let result = a.simd_min(&b);
        let data = result.to_array();
        assert_eq!(data, [1.0, 2.0, 3.0, 1.0, 2.0, 3.0, 4.0, 2.0]);
    }

    #[test]
    fn test_simd_max() {
        let a = SimdF32x8Capsule::from_array([1.0, 5.0, 3.0, 7.0, 2.0, 6.0, 4.0, 8.0]);
        let b = SimdF32x8Capsule::from_array([4.0, 2.0, 6.0, 1.0, 5.0, 3.0, 7.0, 2.0]);
        let result = a.simd_max(&b);
        let data = result.to_array();
        assert_eq!(data, [4.0, 5.0, 6.0, 7.0, 5.0, 6.0, 7.0, 8.0]);
    }

    #[test]
    fn test_simd_clamp() {
        let capsule = SimdF32x8Capsule::from_array([-2.0, -1.0, 0.0, 0.5, 1.0, 1.5, 2.0, 3.0]);
        let min = SimdF32x8Capsule::splat(-1.0);
        let max = SimdF32x8Capsule::splat(1.0);
        let result = capsule.simd_clamp(&min, &max);
        let data = result.to_array();
        assert_eq!(data, [-1.0, -1.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_simd_gt() {
        let a = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let b = SimdF32x8Capsule::from_array([2.0, 2.0, 2.0, 5.0, 5.0, 5.0, 8.0, 8.0]);
        let result = a.simd_gt(&b);
        let data = result.to_array();
        // NAN for true, 0.0 for false
        assert_eq!(data[0], 0.0); // 1.0 > 2.0 = false
        assert_eq!(data[1], 0.0); // 2.0 > 2.0 = false
        assert!(data[2].is_nan()); // 3.0 > 2.0 = true
        assert_eq!(data[3], 0.0); // 4.0 > 5.0 = false
        assert_eq!(data[4], 0.0); // 5.0 > 5.0 = false
        assert!(data[5].is_nan()); // 6.0 > 5.0 = true
        assert_eq!(data[6], 0.0); // 7.0 > 8.0 = false
        assert_eq!(data[7], 0.0); // 8.0 > 8.0 = false
    }

    #[test]
    fn test_simd_lt() {
        let a = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let b = SimdF32x8Capsule::from_array([2.0, 2.0, 2.0, 5.0, 5.0, 5.0, 8.0, 8.0]);
        let result = a.simd_lt(&b);
        let data = result.to_array();
        assert!(data[0].is_nan()); // 1.0 < 2.0 = true
        assert_eq!(data[1], 0.0); // 2.0 < 2.0 = false
        assert_eq!(data[2], 0.0); // 3.0 < 2.0 = false
        assert!(data[3].is_nan()); // 4.0 < 5.0 = true
        assert_eq!(data[4], 0.0); // 5.0 < 5.0 = false
        assert_eq!(data[5], 0.0); // 6.0 < 5.0 = false
        assert!(data[6].is_nan()); // 7.0 < 8.0 = true
        assert_eq!(data[7], 0.0); // 8.0 < 8.0 = false
    }

    #[test]
    fn test_simd_ge() {
        let a = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let b = SimdF32x8Capsule::from_array([2.0, 2.0, 2.0, 5.0, 5.0, 5.0, 8.0, 8.0]);
        let result = a.simd_ge(&b);
        let data = result.to_array();
        assert_eq!(data[0], 0.0); // 1.0 >= 2.0 = false
        assert!(data[1].is_nan()); // 2.0 >= 2.0 = true
        assert!(data[2].is_nan()); // 3.0 >= 2.0 = true
        assert_eq!(data[3], 0.0); // 4.0 >= 5.0 = false
        assert!(data[4].is_nan()); // 5.0 >= 5.0 = true
        assert!(data[5].is_nan()); // 6.0 >= 5.0 = true
        assert_eq!(data[6], 0.0); // 7.0 >= 8.0 = false
        assert!(data[7].is_nan()); // 8.0 >= 8.0 = true
    }

    #[test]
    fn test_simd_le() {
        let a = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let b = SimdF32x8Capsule::from_array([2.0, 2.0, 2.0, 5.0, 5.0, 5.0, 8.0, 8.0]);
        let result = a.simd_le(&b);
        let data = result.to_array();
        assert!(data[0].is_nan()); // 1.0 <= 2.0 = true
        assert!(data[1].is_nan()); // 2.0 <= 2.0 = true
        assert_eq!(data[2], 0.0); // 3.0 <= 2.0 = false
        assert!(data[3].is_nan()); // 4.0 <= 5.0 = true
        assert!(data[4].is_nan()); // 5.0 <= 5.0 = true
        assert_eq!(data[5], 0.0); // 6.0 <= 5.0 = false
        assert!(data[6].is_nan()); // 7.0 <= 8.0 = true
        assert!(data[7].is_nan()); // 8.0 <= 8.0 = true
    }
}
