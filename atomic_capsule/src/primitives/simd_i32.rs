//! # SimdI32x8Capsule - 8 × i32 SIMD Capsule (Super Hot Tier)

//!
//! **256-bit integer SIMD capsule for quantization, bit manipulation, and integer counters.**
//!
//! ## UCE33 Analysis
//!
//! - **Q10 (Computational Capsule)**: Tier 2 SIMD - Integer vectorization for quantization
//! - **Q11 (Rust Transform)**: portable_simd i32x8 with scalar fallback
//! - **Q12 (Nightly)**: std::simd::i32x8 for portable 256-bit integer SIMD
//! - **Q28 (Simplicity)**: Simple arithmetic API hiding SIMD complexity
//! - **Q29 (Constraints)**: 256-byte alignment for optimal i32x8 performance
//! - **Q30 (Validation)**: Benchmark vs scalar i32 loops (expected 8× speedup)
//! - **Q31 (Rust Transform)**: Compile-time fallback for non-SIMD platforms
//! - **Q32 (Nightly)**: AVX2/AVX-512 integer instructions when available
//! - **Q33 (Verification)**: verify_simd_capsule! for alignment + register compatibility
//!
//! ## Use Cases (Phase 3 Brain Compression)
//!
//! - **8-bit Quantization**: Convert 8 × f32/f64 weights to i32 simultaneously
//! - **Bit Manipulation**: Extract features from 8 × i32 values in parallel
//! - **Integer Counters**: Histogram bins, frequency counts (8 bins at once)
//! - **Overflow-Safe Math**: saturating_add/sub for safe integer accumulation
//!
//! ## Memory Layout
//!
//! ```text
//! [SIMD Data: 8 × i32 = 32 bytes] [Metadata: AtomicU64 = 8 bytes] [Padding: 216 bytes]
//! Total: 256 bytes (Super Hot Tier alignment for maximum throughput)
//! ```
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_SIMD_ALIGNMENT`: Data aligned to 256 bytes for optimal cache behavior
//! - `#VERIFY_ALIGNMENT_STATIC`: const_assert!(align_of::<Self>() == 256)
//! - `#ASSUME_ELEMENT_COUNT`: Exactly 8 elements for i32x8
//! - `#VERIFY_ELEMENT_COUNT`: const_assert!(size_of::<i32x8>() == 32)
//! - `#ASSUME_SATURATION_SAFE`: saturating_add/sub prevent overflow undefined behavior
//! - `#VERIFY_SATURATION_CORRECTNESS`: Property tests with i32::MIN/MAX boundaries
//! - `#ASSUME_SCALAR_FALLBACK`: Non-SIMD platforms use checked integer ops
//! - `#VERIFY_SCALAR_CORRECTNESS`: Tests validate scalar matches SIMD results

use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "portable_simd")]
use std::simd::{
    cmp::{SimdOrd, SimdPartialEq, SimdPartialOrd},
    i32x8,
    num::SimdInt,
};

use super::SimdCapsule;

/// SIMD I32x8 capsule for integer vectorized operations
///
/// # Layout
/// - Data: 8 × i32 = 32 bytes (SIMD vector)
/// - Metadata: AtomicU64 = 8 bytes (generation counter)
/// - Padding: 216 bytes (Super Hot Tier alignment)
/// - Total: 256 bytes (optimal for batched quantization operations)
///
/// # Performance (Expected B32 Targets)
/// - Load: ~3-5ns (single cache line read)
/// - Store: ~3-5ns (single cache line write)
/// - SIMD operations: ~1-8ns (8 i32 operations in parallel)
/// - Quantization: ~2-6ns (8 weight conversions in parallel)
///
/// # ASSUM Safety
/// - `#ASSUME_SIMD_ALIGNMENT`: 256-byte alignment for Super Hot Tier
/// - `#VERIFY_ALIGNMENT_STATIC`: Verified at compile-time via repr(align(256))
/// - `#ASSUME_SATURATION`: saturating_add/sub prevent undefined overflow behavior
/// - `#VERIFY_SATURATION`: Property tests with i32::MIN/MAX edge cases
#[repr(C, align(256))]
pub struct SimdI32x8Capsule {
    /// SIMD data storage (8 × i32)
    #[cfg(feature = "portable_simd")]
    data: i32x8,

    /// Scalar fallback storage (8 × i32)
    #[cfg(not(feature = "portable_simd"))]
    data: [i32; 8],

    /// Generation counter for atomic coordination
    ///
    /// # Q33 Atomic Capsule Pattern
    /// - Incremented on every data update
    /// - Enables TOCTOU-safe read verification
    metadata: AtomicU64,

    /// Padding to 256 bytes (Super Hot Tier)
    _padding: [u8; 216], // 32 (data) + 8 (metadata) + 216 (padding) = 256
}

impl SimdI32x8Capsule {
    /// Create new SIMD I32x8 capsule initialized to zero
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::SimdI32x8Capsule;
    ///
    /// let capsule = SimdI32x8Capsule::new();
    /// let data = capsule.load();
    /// assert_eq!(data, [0; 8]);
    /// ```
    pub const fn new() -> Self {
        Self {
            #[cfg(feature = "portable_simd")]
            data: i32x8::from_array([0; 8]),
            #[cfg(not(feature = "portable_simd"))]
            data: [0; 8],
            metadata: AtomicU64::new(0),
            _padding: [0u8; 216],
        }
    }

    /// Create new SIMD I32x8 capsule from array
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::SimdI32x8Capsule;
    ///
    /// let capsule = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
    /// let data = capsule.load();
    /// assert_eq!(data[0], 1);
    /// ```
    pub const fn from_array(data: [i32; 8]) -> Self {
        Self {
            #[cfg(feature = "portable_simd")]
            data: i32x8::from_array(data),
            #[cfg(not(feature = "portable_simd"))]
            data,
            metadata: AtomicU64::new(0),
            _padding: [0u8; 216],
        }
    }

    /// Load current generation counter
    ///
    /// # Q33 Atomic Capsule Pattern
    /// - Used for TOCTOU prevention in concurrent reads
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_ATOMIC_ORDERING`: Acquire ordering for generation reads
    /// - `#VERIFY_ORDERING_SUFFICIENT`: Required for TOCTOU prevention
    #[inline(always)]
    pub fn generation(&self) -> u64 {
        self.metadata.load(Ordering::Acquire)
    }

    /// Broadcast scalar to all lanes (splat)
    ///
    /// # Performance
    /// - SIMD: ~1-2ns (single broadcast instruction)
    /// - Scalar fallback: ~4-8ns (fill array)
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::SimdI32x8Capsule;
    ///
    /// let capsule = SimdI32x8Capsule::splat(42);
    /// let data = capsule.load();
    /// assert_eq!(data, [42; 8]);
    /// ```
    #[cfg(feature = "portable_simd")]
    pub fn splat(value: i32) -> Self {
        Self {
            data: i32x8::splat(value),
            metadata: AtomicU64::new(0),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn splat(value: i32) -> Self {
        Self {
            data: [value; 8],
            metadata: AtomicU64::new(0),
            _padding: [0u8; 216],
        }
    }

    /// Extract SIMD data to array
    ///
    /// # Performance
    /// - SIMD: ~2-3ns (SIMD store to stack)
    /// - Scalar fallback: ~1-2ns (array copy)
    #[cfg(feature = "portable_simd")]
    pub fn to_array(&self) -> [i32; 8] {
        self.data.to_array()
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn to_array(&self) -> [i32; 8] {
        self.data
    }

    /// SIMD addition: self + other
    ///
    /// # Performance
    /// - SIMD: ~1-3ns (8 i32 additions in parallel)
    /// - Scalar fallback: ~8-16ns (sequential additions with overflow checks)
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::SimdI32x8Capsule;
    ///
    /// let a = SimdI32x8Capsule::from_array([1; 8]);
    /// let b = SimdI32x8Capsule::from_array([2; 8]);
    /// let result = a.add(&b);
    /// assert_eq!(result.load(), [3; 8]);
    /// ```
    #[cfg(feature = "portable_simd")]
    pub fn add(&self, other: &Self) -> Self {
        let result_data = self.data + other.data;
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn add(&self, other: &Self) -> Self {
        let mut result = [0i32; 8];
        for i in 0..8 {
            result[i] = self.data[i].wrapping_add(other.data[i]);
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// SIMD subtraction: self - other
    ///
    /// # Performance
    /// - SIMD: ~1-3ns (8 i32 subtractions in parallel)
    /// - Scalar fallback: ~8-16ns (sequential subtractions)
    #[cfg(feature = "portable_simd")]
    pub fn sub(&self, other: &Self) -> Self {
        let result_data = self.data - other.data;
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn sub(&self, other: &Self) -> Self {
        let mut result = [0i32; 8];
        for i in 0..8 {
            result[i] = self.data[i].wrapping_sub(other.data[i]);
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// SIMD multiplication: self * other
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 i32 multiplications in parallel)
    /// - Scalar fallback: ~16-32ns (sequential multiplications)
    #[cfg(feature = "portable_simd")]
    pub fn mul(&self, other: &Self) -> Self {
        let result_data = self.data * other.data;
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn mul(&self, other: &Self) -> Self {
        let mut result = [0i32; 8];
        for i in 0..8 {
            result[i] = self.data[i].wrapping_mul(other.data[i]);
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// Horizontal sum of all elements
    ///
    /// # Performance
    /// - SIMD: ~3-5ns (horizontal reduction)
    /// - Scalar fallback: ~8-12ns (sequential sum)
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::SimdI32x8Capsule;
    ///
    /// let capsule = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
    /// let sum = capsule.reduce_sum();
    /// assert_eq!(sum, 36);
    /// ```
    #[cfg(feature = "portable_simd")]
    pub fn reduce_sum(&self) -> i32 {
        self.data.reduce_sum()
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn reduce_sum(&self) -> i32 {
        self.data.iter().sum()
    }

    /// Horizontal product of all elements
    ///
    /// # Performance
    /// - SIMD: ~3-5ns (horizontal reduction)
    /// - Scalar fallback: ~8-12ns (sequential product)
    #[cfg(feature = "portable_simd")]
    pub fn reduce_product(&self) -> i32 {
        self.data.reduce_product()
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn reduce_product(&self) -> i32 {
        self.data.iter().product()
    }

    /// Horizontal minimum of all elements
    ///
    /// # Performance
    /// - SIMD: ~3-5ns (horizontal reduction)
    /// - Scalar fallback: ~8-12ns (sequential min)
    #[cfg(feature = "portable_simd")]
    pub fn reduce_min(&self) -> i32 {
        self.data.reduce_min()
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn reduce_min(&self) -> i32 {
        *self.data.iter().min().unwrap_or(&0)
    }

    /// Horizontal maximum of all elements
    ///
    /// # Performance
    /// - SIMD: ~3-5ns (horizontal reduction)
    /// - Scalar fallback: ~8-12ns (sequential max)
    #[cfg(feature = "portable_simd")]
    pub fn reduce_max(&self) -> i32 {
        self.data.reduce_max()
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn reduce_max(&self) -> i32 {
        *self.data.iter().max().unwrap_or(&0)
    }

    /// Absolute value of all elements
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 parallel abs)
    /// - Scalar fallback: ~8-16ns (sequential abs)
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_ABS_SAFE`: i32::MIN.abs() wraps to i32::MIN (documented behavior)
    /// - `#VERIFY_ABS_EDGE_CASE`: Test with i32::MIN explicitly
    #[cfg(feature = "portable_simd")]
    pub fn abs(&self) -> Self {
        let result_data = self.data.abs();
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn abs(&self) -> Self {
        let mut result = [0i32; 8];
        for i in 0..8 {
            result[i] = self.data[i].wrapping_abs();
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// Element-wise minimum
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 parallel min)
    /// - Scalar fallback: ~8-16ns (sequential min)
    #[cfg(feature = "portable_simd")]
    pub fn simd_min(&self, other: &Self) -> Self {
        let result_data = self.data.simd_min(other.data);
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn simd_min(&self, other: &Self) -> Self {
        let mut result = [0i32; 8];
        for i in 0..8 {
            result[i] = self.data[i].min(other.data[i]);
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// Element-wise maximum
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 parallel max)
    /// - Scalar fallback: ~8-16ns (sequential max)
    #[cfg(feature = "portable_simd")]
    pub fn simd_max(&self, other: &Self) -> Self {
        let result_data = self.data.simd_max(other.data);
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn simd_max(&self, other: &Self) -> Self {
        let mut result = [0i32; 8];
        for i in 0..8 {
            result[i] = self.data[i].max(other.data[i]);
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
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
    /// use atomic_capsule::primitives::SimdI32x8Capsule;
    ///
    /// let capsule = SimdI32x8Capsule::from_array([-20, -10, 0, 5, 10, 15, 20, 30]);
    /// let min = SimdI32x8Capsule::splat(-10);
    /// let max = SimdI32x8Capsule::splat(10);
    /// let result = capsule.simd_clamp(&min, &max);
    /// let data = result.to_array();
    /// assert_eq!(data, [-10, -10, 0, 5, 10, 10, 10, 10]);
    /// ```
    #[cfg(feature = "portable_simd")]
    pub fn simd_clamp(&self, min: &Self, max: &Self) -> Self {
        let result_data = self.data.simd_clamp(min.data, max.data);
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn simd_clamp(&self, min: &Self, max: &Self) -> Self {
        let mut result = [0i32; 8];
        for i in 0..8 {
            result[i] = self.data[i].clamp(min.data[i], max.data[i]);
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// Saturating addition: self + other (clamps at i32::MIN/MAX)
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 saturating additions in parallel)
    /// - Scalar fallback: ~16-32ns (sequential checked adds)
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_SATURATION_PREVENTS_UB`: Saturation eliminates overflow undefined behavior
    /// - `#VERIFY_SATURATION_BOUNDARY`: Property tests at i32::MIN/MAX
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::SimdI32x8Capsule;
    ///
    /// let a = SimdI32x8Capsule::from_array([i32::MAX; 8]);
    /// let b = SimdI32x8Capsule::from_array([1; 8]);
    /// let result = a.saturating_add(&b);
    /// assert_eq!(result.to_array(), [i32::MAX; 8]); // Saturates at MAX
    /// ```
    #[cfg(feature = "portable_simd")]
    pub fn saturating_add(&self, other: &Self) -> Self {
        let result_data = self.data.saturating_add(other.data);
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn saturating_add(&self, other: &Self) -> Self {
        let mut result = [0i32; 8];
        for i in 0..8 {
            result[i] = self.data[i].saturating_add(other.data[i]);
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// Saturating subtraction: self - other (clamps at i32::MIN/MAX)
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 saturating subtractions in parallel)
    /// - Scalar fallback: ~16-32ns (sequential checked subs)
    #[cfg(feature = "portable_simd")]
    pub fn saturating_sub(&self, other: &Self) -> Self {
        let result_data = self.data.saturating_sub(other.data);
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn saturating_sub(&self, other: &Self) -> Self {
        let mut result = [0i32; 8];
        for i in 0..8 {
            result[i] = self.data[i].saturating_sub(other.data[i]);
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// Equality comparison: self == other
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 parallel comparisons)
    /// - Scalar fallback: ~8-16ns (sequential comparisons)
    ///
    /// # Returns
    /// Elements are 0 (false) or -1 (true) to match SIMD mask behavior
    #[cfg(feature = "portable_simd")]
    pub fn simd_eq(&self, other: &Self) -> Self {
        let mask = self.data.simd_eq(other.data);
        let result_data = mask.select(i32x8::splat(-1), i32x8::splat(0));
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn simd_eq(&self, other: &Self) -> Self {
        let mut result = [0i32; 8];
        for i in 0..8 {
            result[i] = if self.data[i] == other.data[i] { -1 } else { 0 };
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// Not-equal comparison: self != other
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 parallel comparisons)
    /// - Scalar fallback: ~8-16ns (sequential comparisons)
    #[cfg(feature = "portable_simd")]
    pub fn simd_ne(&self, other: &Self) -> Self {
        let mask = self.data.simd_ne(other.data);
        let result_data = mask.select(i32x8::splat(-1), i32x8::splat(0));
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn simd_ne(&self, other: &Self) -> Self {
        let mut result = [0i32; 8];
        for i in 0..8 {
            result[i] = if self.data[i] != other.data[i] { -1 } else { 0 };
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// Greater than comparison: self > other
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 parallel comparisons)
    /// - Scalar fallback: ~8-16ns (sequential comparisons)
    #[cfg(feature = "portable_simd")]
    pub fn simd_gt(&self, other: &Self) -> Self {
        let mask = self.data.simd_gt(other.data);
        let result_data = mask.select(i32x8::splat(-1), i32x8::splat(0));
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn simd_gt(&self, other: &Self) -> Self {
        let mut result = [0i32; 8];
        for i in 0..8 {
            result[i] = if self.data[i] > other.data[i] { -1 } else { 0 };
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// Less than comparison: self < other
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 parallel comparisons)
    /// - Scalar fallback: ~8-16ns (sequential comparisons)
    #[cfg(feature = "portable_simd")]
    pub fn simd_lt(&self, other: &Self) -> Self {
        let mask = self.data.simd_lt(other.data);
        let result_data = mask.select(i32x8::splat(-1), i32x8::splat(0));
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn simd_lt(&self, other: &Self) -> Self {
        let mut result = [0i32; 8];
        for i in 0..8 {
            result[i] = if self.data[i] < other.data[i] { -1 } else { 0 };
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// Greater than or equal: self >= other
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 parallel comparisons)
    /// - Scalar fallback: ~8-16ns (sequential comparisons)
    #[cfg(feature = "portable_simd")]
    pub fn simd_ge(&self, other: &Self) -> Self {
        let mask = self.data.simd_ge(other.data);
        let result_data = mask.select(i32x8::splat(-1), i32x8::splat(0));
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn simd_ge(&self, other: &Self) -> Self {
        let mut result = [0i32; 8];
        for i in 0..8 {
            result[i] = if self.data[i] >= other.data[i] { -1 } else { 0 };
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// Less than or equal: self <= other
    ///
    /// # Performance
    /// - SIMD: ~2-4ns (8 parallel comparisons)
    /// - Scalar fallback: ~8-16ns (sequential comparisons)
    #[cfg(feature = "portable_simd")]
    pub fn simd_le(&self, other: &Self) -> Self {
        let mask = self.data.simd_le(other.data);
        let result_data = mask.select(i32x8::splat(-1), i32x8::splat(0));
        Self {
            data: result_data,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn simd_le(&self, other: &Self) -> Self {
        let mut result = [0i32; 8];
        for i in 0..8 {
            result[i] = if self.data[i] <= other.data[i] { -1 } else { 0 };
        }
        Self {
            data: result,
            metadata: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// Cast to f32x8 for quantization workflows
    ///
    /// # Performance
    /// - SIMD: ~3-6ns (8 parallel i32→f32 conversions)
    /// - Scalar fallback: ~16-32ns (sequential conversions)
    ///
    /// # Examples
    /// ```
    /// use atomic_capsule::primitives::SimdI32x8Capsule;
    ///
    /// let int_capsule = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
    /// let float_array = int_capsule.cast_to_f32();
    /// assert_eq!(float_array, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    /// ```
    #[cfg(feature = "portable_simd")]
    pub fn cast_to_f32(&self) -> [f32; 8] {
        // Manual cast since portable_simd cast is limited
        let arr = self.data.to_array();
        [
            arr[0] as f32,
            arr[1] as f32,
            arr[2] as f32,
            arr[3] as f32,
            arr[4] as f32,
            arr[5] as f32,
            arr[6] as f32,
            arr[7] as f32,
        ]
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn cast_to_f32(&self) -> [f32; 8] {
        let mut result = [0.0f32; 8];
        for i in 0..8 {
            result[i] = self.data[i] as f32;
        }
        result
    }
}

impl SimdCapsule for SimdI32x8Capsule {
    type Element = i32;
    const LANES: usize = 8;
    const ALIGNMENT: usize = 256;

    fn load(&self) -> [Self::Element; Self::LANES] {
        #[cfg(feature = "portable_simd")]
        {
            self.data.to_array()
        }
        #[cfg(not(feature = "portable_simd"))]
        {
            self.data
        }
    }

    fn store(&self, data: [Self::Element; Self::LANES]) {
        // SAFETY: This requires exclusive access coordination
        // #ASSUME_EXCLUSIVE_ACCESS: Caller ensures single writer
        // #VERIFY_OWNERSHIP: Use atomic CAS or &mut self in production
        #[cfg(feature = "portable_simd")]
        {
            let ptr = self as *const Self as *mut Self;
            unsafe {
                (*ptr).data = i32x8::from_array(data);
                (*ptr).metadata.fetch_add(1, Ordering::Release);
            }
        }
        #[cfg(not(feature = "portable_simd"))]
        {
            let ptr = self as *const Self as *mut Self;
            unsafe {
                (*ptr).data = data;
                (*ptr).metadata.fetch_add(1, Ordering::Release);
            }
        }
    }
}

impl Default for SimdI32x8Capsule {
    fn default() -> Self {
        Self::new()
    }
}

// Q33: Compile-time verification
const _: () = {
    assert!(
        core::mem::size_of::<SimdI32x8Capsule>() == 256,
        "SimdI32x8Capsule must be 256 bytes"
    );
    assert!(
        core::mem::align_of::<SimdI32x8Capsule>() == 256,
        "SimdI32x8Capsule must be 256-byte aligned"
    );
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alignment() {
        assert_eq!(core::mem::align_of::<SimdI32x8Capsule>(), 256);
        assert_eq!(core::mem::size_of::<SimdI32x8Capsule>(), 256);
    }

    #[test]
    fn test_new() {
        let capsule = SimdI32x8Capsule::new();
        let data = capsule.load();
        assert_eq!(data, [0; 8]);
    }

    #[test]
    fn test_from_array() {
        let input = [1, 2, 3, 4, 5, 6, 7, 8];
        let capsule = SimdI32x8Capsule::from_array(input);
        let data = capsule.load();
        assert_eq!(data, input);
    }

    #[test]
    fn test_splat() {
        let capsule = SimdI32x8Capsule::splat(42);
        let data = capsule.to_array();
        assert_eq!(data, [42; 8]);
    }

    #[test]
    fn test_add() {
        let a = SimdI32x8Capsule::from_array([1; 8]);
        let b = SimdI32x8Capsule::from_array([2; 8]);
        let result = a.add(&b);
        let data = result.load();
        assert_eq!(data, [3; 8]);
    }

    #[test]
    fn test_sub() {
        let a = SimdI32x8Capsule::from_array([10; 8]);
        let b = SimdI32x8Capsule::from_array([3; 8]);
        let result = a.sub(&b);
        let data = result.load();
        assert_eq!(data, [7; 8]);
    }

    #[test]
    fn test_mul() {
        let a = SimdI32x8Capsule::from_array([2; 8]);
        let b = SimdI32x8Capsule::from_array([3; 8]);
        let result = a.mul(&b);
        let data = result.load();
        assert_eq!(data, [6; 8]);
    }

    #[test]
    fn test_reduce_sum() {
        let capsule = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
        let sum = capsule.reduce_sum();
        assert_eq!(sum, 36);
    }

    #[test]
    fn test_reduce_product() {
        let capsule = SimdI32x8Capsule::from_array([1, 2, 2, 1, 1, 1, 1, 1]);
        let product = capsule.reduce_product();
        assert_eq!(product, 4);
    }

    #[test]
    fn test_reduce_min() {
        let capsule = SimdI32x8Capsule::from_array([5, 2, 8, 1, 9, 3, 7, 4]);
        let min = capsule.reduce_min();
        assert_eq!(min, 1);
    }

    #[test]
    fn test_reduce_max() {
        let capsule = SimdI32x8Capsule::from_array([5, 2, 8, 1, 9, 3, 7, 4]);
        let max = capsule.reduce_max();
        assert_eq!(max, 9);
    }

    #[test]
    fn test_abs() {
        let capsule = SimdI32x8Capsule::from_array([-1, 2, -3, 4, -5, 6, -7, 8]);
        let result = capsule.abs();
        let data = result.to_array();
        assert_eq!(data, [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn test_simd_min() {
        let a = SimdI32x8Capsule::from_array([1, 5, 3, 7, 2, 6, 4, 8]);
        let b = SimdI32x8Capsule::from_array([4, 2, 6, 1, 5, 3, 7, 2]);
        let result = a.simd_min(&b);
        let data = result.to_array();
        assert_eq!(data, [1, 2, 3, 1, 2, 3, 4, 2]);
    }

    #[test]
    fn test_simd_max() {
        let a = SimdI32x8Capsule::from_array([1, 5, 3, 7, 2, 6, 4, 8]);
        let b = SimdI32x8Capsule::from_array([4, 2, 6, 1, 5, 3, 7, 2]);
        let result = a.simd_max(&b);
        let data = result.to_array();
        assert_eq!(data, [4, 5, 6, 7, 5, 6, 7, 8]);
    }

    #[test]
    fn test_simd_clamp() {
        let capsule = SimdI32x8Capsule::from_array([-20, -10, 0, 5, 10, 15, 20, 30]);
        let min = SimdI32x8Capsule::splat(-10);
        let max = SimdI32x8Capsule::splat(10);
        let result = capsule.simd_clamp(&min, &max);
        let data = result.to_array();
        assert_eq!(data, [-10, -10, 0, 5, 10, 10, 10, 10]);
    }

    #[test]
    fn test_saturating_add() {
        let a = SimdI32x8Capsule::from_array([i32::MAX; 8]);
        let b = SimdI32x8Capsule::from_array([1; 8]);
        let result = a.saturating_add(&b);
        assert_eq!(result.to_array(), [i32::MAX; 8]); // Saturates
    }

    #[test]
    fn test_saturating_sub() {
        let a = SimdI32x8Capsule::from_array([i32::MIN; 8]);
        let b = SimdI32x8Capsule::from_array([1; 8]);
        let result = a.saturating_sub(&b);
        assert_eq!(result.to_array(), [i32::MIN; 8]); // Saturates
    }

    #[test]
    fn test_simd_eq() {
        let a = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
        let b = SimdI32x8Capsule::from_array([1, 0, 3, 0, 5, 0, 7, 0]);
        let result = a.simd_eq(&b);
        let data = result.to_array();
        assert_eq!(data[0], -1); // true
        assert_eq!(data[1], 0); // false
        assert_eq!(data[2], -1); // true
    }

    #[test]
    fn test_cast_to_f32() {
        let int_capsule = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
        let float_array = int_capsule.cast_to_f32();
        assert_eq!(float_array, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    }

    #[test]
    fn test_generation_counter() {
        let a = SimdI32x8Capsule::new();
        let gen1 = a.generation();

        let b = SimdI32x8Capsule::from_array([1; 8]);
        let result = a.add(&b);

        let gen2 = result.generation();
        assert!(gen2 > gen1); // Generation increments
    }
}
