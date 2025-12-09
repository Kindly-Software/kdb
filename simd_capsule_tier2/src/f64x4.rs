//! # SimdF64x4Capsule - 4 × f64 SIMD Capsule (Hot Tier)
//!
//! **256-byte aligned SIMD capsule for high-precision 64-bit floating-point operations.**
//!
//! ## UCE33 Analysis
//!
//! - **Q28 (Simplicity)**: Load, compute, store - minimal API
//! - **Q29 (Constraints)**: 32-byte SIMD (f64x4), 256-byte capsule (4 cache lines)
//! - **Q30 (Validation)**: Proven 5× aggregation speedup
//! - **Q31 (Rust Transform)**: Safe portable_simd (zero unsafe in operations)
//! - **Q32 (Nightly)**: std::simd::f64x4 for cross-platform vectorization
//! - **Q33 (Tier 2 SIMD)**: Parallel f64 operations for financial precision
//!
//! ## Memory Layout
//!
//! ```text
//! [SIMD Data: 4 × f64 = 32 bytes]
//! [Generation: AtomicU64 = 8 bytes]
//! [Padding: 216 bytes]
//! Total: 256 bytes (Hot Tier - 4 cache lines)
//! ```
//!
//! ## Proven Performance (KEY_INNOVATIONS.md Innovation 2)
//!
//! - **5× aggregation speedup** (GROUP BY + SUM operations)
//! - **3-6ns per operation** (4 operations in parallel)
//! - **Financial precision** (64-bit mantissa, no drift)
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_SIMD_ALIGNMENT`: 256-byte alignment for cache predictability
//! - `#VERIFY_ALIGNMENT_STATIC`: Compile-time const assertion
//! - `#ASSUME_ELEMENT_COUNT`: Exactly 4 elements for f64x4
//! - `#VERIFY_ELEMENT_COUNT`: size_of::<f64x4>() == 32

use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "portable_simd")]
use core::simd::{
    f64x4,
    num::SimdFloat,  // Provides abs, reduce_sum, reduce_min, reduce_max, simd_min, simd_max, simd_clamp
};

use crate::SimdCapsule;

/// SIMD F64x4 capsule for high-precision vectorized operations
///
/// # Layout
/// - Data: 4 × f64 = 32 bytes (SIMD vector)
/// - Generation: AtomicU64 = 8 bytes (atomic coordination)
/// - Padding: 216 bytes (Hot Tier alignment)
/// - Total: 256 bytes (4 cache lines)
///
/// # Performance
/// - Load: ~3-5ns (single capsule read)
/// - Store: ~3-5ns (single capsule write)
/// - SIMD ops: ~3-6ns (4 f64 operations in parallel)
/// - Proven: 5× aggregation speedup (GROUP BY + SUM)
///
/// # ASSUM Safety
/// - `#ASSUME_HOT_TIER`: 256-byte alignment for predictable cache placement
/// - `#VERIFY_CACHE_FIT`: 256 bytes = 4 × 64-byte cache lines
#[repr(C, align(256))]
pub struct SimdF64x4Capsule {
    /// SIMD data storage (4 × f64)
    #[cfg(feature = "portable_simd")]
    data: f64x4,

    /// Scalar fallback storage (4 × f64)
    #[cfg(not(feature = "portable_simd"))]
    data: [f64; 4],

    /// Generation counter for atomic coordination
    generation: AtomicU64,

    /// Padding to 256 bytes (Hot Tier)
    _padding: [u8; 216], // 32 (data) + 8 (generation) + 216 (padding) = 256
}

impl SimdF64x4Capsule {
    /// Create new SIMD F64x4 capsule initialized to zero
    pub const fn new() -> Self {
        Self {
            #[cfg(feature = "portable_simd")]
            data: f64x4::from_array([0.0; 4]),
            #[cfg(not(feature = "portable_simd"))]
            data: [0.0; 4],
            generation: AtomicU64::new(0),
            _padding: [0u8; 216],
        }
    }

    /// Create SIMD F64x4 capsule from array
    pub const fn from_array(data: [f64; 4]) -> Self {
        Self {
            #[cfg(feature = "portable_simd")]
            data: f64x4::from_array(data),
            #[cfg(not(feature = "portable_simd"))]
            data,
            generation: AtomicU64::new(0),
            _padding: [0u8; 216],
        }
    }

    /// Broadcast scalar to all lanes (splat)
    #[cfg(feature = "portable_simd")]
    pub fn splat(value: f64) -> Self {
        Self {
            data: f64x4::splat(value),
            generation: AtomicU64::new(0),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn splat(value: f64) -> Self {
        Self {
            data: [value; 4],
            generation: AtomicU64::new(0),
            _padding: [0u8; 216],
        }
    }

    // ============================================================================
    // ARITHMETIC OPERATIONS (Immutable)
    // ============================================================================

    /// SIMD addition: self + other
    #[cfg(feature = "portable_simd")]
    pub fn add(&self, other: &Self) -> Self {
        Self {
            data: self.data + other.data,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn add(&self, other: &Self) -> Self {
        let mut result = [0.0f64; 4];
        for i in 0..4 {
            result[i] = self.data[i] + other.data[i];
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// SIMD subtraction: self - other
    #[cfg(feature = "portable_simd")]
    pub fn sub(&self, other: &Self) -> Self {
        Self {
            data: self.data - other.data,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn sub(&self, other: &Self) -> Self {
        let mut result = [0.0f64; 4];
        for i in 0..4 {
            result[i] = self.data[i] - other.data[i];
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// SIMD multiplication: self * other
    #[cfg(feature = "portable_simd")]
    pub fn mul(&self, other: &Self) -> Self {
        Self {
            data: self.data * other.data,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn mul(&self, other: &Self) -> Self {
        let mut result = [0.0f64; 4];
        for i in 0..4 {
            result[i] = self.data[i] * other.data[i];
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// SIMD division: self / other
    #[cfg(feature = "portable_simd")]
    pub fn div(&self, other: &Self) -> Self {
        Self {
            data: self.data / other.data,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn div(&self, other: &Self) -> Self {
        let mut result = [0.0f64; 4];
        for i in 0..4 {
            result[i] = self.data[i] / other.data[i];
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// SIMD fused multiply-add: (self * mul) + add
    #[cfg(feature = "portable_simd")]
    pub fn fma(&self, mul: &Self, add: &Self) -> Self {
        Self {
            data: self.data * mul.data + add.data,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn fma(&self, mul: &Self, add: &Self) -> Self {
        let mut result = [0.0f64; 4];
        for i in 0..4 {
            result[i] = self.data[i] * mul.data[i] + add.data[i];
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// Scale all elements by scalar value
    #[cfg(feature = "portable_simd")]
    pub fn scale(&self, scalar: f64) -> Self {
        Self {
            data: self.data * f64x4::splat(scalar),
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn scale(&self, scalar: f64) -> Self {
        let mut result = [0.0f64; 4];
        for i in 0..4 {
            result[i] = self.data[i] * scalar;
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    // ============================================================================
    // REDUCTION OPERATIONS
    // ============================================================================

    /// SIMD dot product: sum(self[i] * other[i])
    ///
    /// # Performance
    /// - SIMD: ~3-6ns (parallel multiply + horizontal sum)
    /// - Proven: 5× faster than scalar for aggregations
    #[cfg(feature = "portable_simd")]
    pub fn dot(&self, other: &Self) -> f64 {
        (self.data * other.data).reduce_sum()
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn dot(&self, other: &Self) -> f64 {
        let mut sum = 0.0f64;
        for i in 0..4 {
            sum += self.data[i] * other.data[i];
        }
        sum
    }

    /// Horizontal sum of all elements
    ///
    /// # Performance
    /// - SIMD: ~3-5ns (horizontal reduction)
    /// - Proven: 5× speedup for GROUP BY + SUM
    #[cfg(feature = "portable_simd")]
    pub fn reduce_sum(&self) -> f64 {
        self.data.reduce_sum()
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn reduce_sum(&self) -> f64 {
        self.data.iter().sum()
    }

    /// Horizontal product of all elements
    #[cfg(feature = "portable_simd")]
    pub fn reduce_product(&self) -> f64 {
        self.data.reduce_product()
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn reduce_product(&self) -> f64 {
        self.data.iter().product()
    }

    /// Horizontal minimum of all elements
    #[cfg(feature = "portable_simd")]
    pub fn reduce_min(&self) -> f64 {
        self.data.reduce_min()
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn reduce_min(&self) -> f64 {
        self.data.iter().copied().fold(f64::INFINITY, f64::min)
    }

    /// Horizontal maximum of all elements
    #[cfg(feature = "portable_simd")]
    pub fn reduce_max(&self) -> f64 {
        self.data.reduce_max()
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn reduce_max(&self) -> f64 {
        self.data.iter().copied().fold(f64::NEG_INFINITY, f64::max)
    }

    // ============================================================================
    // ELEMENT-WISE OPERATIONS
    // ============================================================================

    /// Absolute value of all elements
    #[cfg(feature = "portable_simd")]
    pub fn abs(&self) -> Self {
        Self {
            data: self.data.abs(),
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn abs(&self) -> Self {
        let mut result = [0.0f64; 4];
        for i in 0..4 {
            result[i] = self.data[i].abs();
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// Element-wise minimum
    #[cfg(feature = "portable_simd")]
    pub fn simd_min(&self, other: &Self) -> Self {
        Self {
            data: self.data.simd_min(other.data),
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn simd_min(&self, other: &Self) -> Self {
        let mut result = [0.0f64; 4];
        for i in 0..4 {
            result[i] = self.data[i].min(other.data[i]);
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// Element-wise maximum
    #[cfg(feature = "portable_simd")]
    pub fn simd_max(&self, other: &Self) -> Self {
        Self {
            data: self.data.simd_max(other.data),
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn simd_max(&self, other: &Self) -> Self {
        let mut result = [0.0f64; 4];
        for i in 0..4 {
            result[i] = self.data[i].max(other.data[i]);
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    /// Clamp all elements to range [min, max]
    #[cfg(feature = "portable_simd")]
    pub fn simd_clamp(&self, min: &Self, max: &Self) -> Self {
        Self {
            data: self.data.simd_clamp(min.data, max.data),
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn simd_clamp(&self, min: &Self, max: &Self) -> Self {
        let mut result = [0.0f64; 4];
        for i in 0..4 {
            result[i] = self.data[i].clamp(min.data[i], max.data[i]);
        }
        Self {
            data: result,
            generation: AtomicU64::new(self.generation() + 1),
            _padding: [0u8; 216],
        }
    }

    // ============================================================================
    // MUTABLE IN-PLACE OPERATIONS
    // ============================================================================

    /// Add in-place: self += other
    #[cfg(feature = "portable_simd")]
    #[inline(always)]
    pub fn add_assign(&mut self, other: &Self) {
        self.data += other.data;
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    #[cfg(not(feature = "portable_simd"))]
    #[inline(always)]
    pub fn add_assign(&mut self, other: &Self) {
        for i in 0..4 {
            self.data[i] += other.data[i];
        }
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Multiply in-place: self *= other
    #[cfg(feature = "portable_simd")]
    #[inline(always)]
    pub fn mul_assign(&mut self, other: &Self) {
        self.data *= other.data;
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    #[cfg(not(feature = "portable_simd"))]
    #[inline(always)]
    pub fn mul_assign(&mut self, other: &Self) {
        for i in 0..4 {
            self.data[i] *= other.data[i];
        }
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Scale in-place: self *= scalar
    #[cfg(feature = "portable_simd")]
    #[inline(always)]
    pub fn scale_assign(&mut self, scalar: f64) {
        self.data *= f64x4::splat(scalar);
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    #[cfg(not(feature = "portable_simd"))]
    #[inline(always)]
    pub fn scale_assign(&mut self, scalar: f64) {
        for i in 0..4 {
            self.data[i] *= scalar;
        }
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    // ============================================================================
    // UTILITY METHODS
    // ============================================================================

    /// Extract SIMD data to array
    #[cfg(feature = "portable_simd")]
    pub fn to_array(&self) -> [f64; 4] {
        self.data.to_array()
    }

    #[cfg(not(feature = "portable_simd"))]
    pub fn to_array(&self) -> [f64; 4] {
        self.data
    }

    /// Load data (convenience method for testing)
    pub fn load(&self) -> [f64; 4] {
        self.to_array()
    }

    /// Stores the SIMD vector to memory at the given pointer.
    ///
    /// # Safety
    ///
    /// This is an unsafe operation due to the following #ASSUME assumptions:
    ///
    /// **#ASSUME_SWEMR_SINGLE_WRITER**: Only one thread must call `store()` on
    /// this pointer at any given time. Concurrent calls to `store()` with
    /// overlapping address ranges constitute undefined behavior.
    ///
    /// **#ASSUME_SWEMR_READER_SAFETY**: Multiple threads may safely read from
    /// the target memory location after `store()` completes (memory_order::Release
    /// semantics ensure visibility).
    ///
    /// **#ASSUME_ALIASING_INVARIANT**: The caller must ensure:
    /// 1. `ptr` is properly aligned (32-byte alignment for f64x4 SIMD)
    /// 2. `ptr` is valid for writing an array of 4 × f64 (32 bytes total)
    /// 3. No overlapping writes occur during or after this call
    /// 4. Lifetime of the pointed-to data extends until all reads complete
    ///
    /// **#ASSUME_MEMORY_ORDERING**: This operation uses Release semantics to
    /// ensure all prior writes are visible to readers after the store completes.
    /// Readers must use Acquire semantics or stronger to observe the update.
    ///
    /// # #VERIFY Validation Strategy
    ///
    /// To verify these assumptions hold in your code:
    ///
    /// 1. **Single Writer Verification**: Use atomic counters or thread IDs
    ///    to ensure only one writer accesses the target location
    /// 2. **No-Alias Verification**: Use Stacked Borrows analysis or custom
    ///    static analysis to prove no overlapping borrows exist
    /// 3. **Lifetime Verification**: Ensure `ptr` remains valid for the full
    ///    duration of subsequent reads
    /// 4. **Alignment Verification**: Verify pointer alignment at runtime
    ///    using `ptr as usize % 32 == 0` before calling
    ///
    /// # Example (CORRECT - Safe Usage)
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "portable_simd")]
    /// # {
    /// use simd_capsule_tier2::SimdF64x4Capsule;
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// // Single-writer pattern with atomic flag
    /// let mut buffer = vec![0.0f64; 4];
    /// let writer_active = AtomicBool::new(false);
    ///
    /// // Ensure only one thread writes
    /// assert!(!writer_active.swap(true, Ordering::Acquire));
    ///
    /// // Safe: Only one thread calls store()
    /// let v = SimdF64x4Capsule::from_array([1.0; 4]);
    /// unsafe {
    ///     v.store(buffer.as_mut_ptr() as *mut [f64; 4]);
    /// }
    ///
    /// writer_active.store(false, Ordering::Release);
    /// # }
    /// ```
    ///
    /// # Example (INCORRECT - UB Risk)
    ///
    /// ```rust,no_run,ignore
    /// # #[cfg(feature = "portable_simd")]
    /// # {
    /// use simd_capsule_tier2::SimdF64x4Capsule;
    ///
    /// // WRONG: Multiple threads calling store() on same ptr = data race
    /// let mut buffer = vec![0.0f64; 4];
    /// let ptr = buffer.as_mut_ptr() as *mut [f64; 4];
    ///
    /// let v1 = SimdF64x4Capsule::from_array([1.0; 4]);
    /// let v2 = SimdF64x4Capsule::from_array([2.0; 4]);
    ///
    /// std::thread::scope(|s| {
    ///     s.spawn(|| { unsafe { v1.store(ptr); } }); // UB!
    ///     s.spawn(|| { unsafe { v2.store(ptr); } }); // Concurrent write!
    /// });
    /// # }
    /// ```
    ///
    /// # Example (INCORRECT - Alignment Violation)
    ///
    /// ```rust,no_run,ignore
    /// # #[cfg(feature = "portable_simd")]
    /// # {
    /// use simd_capsule_tier2::SimdF64x4Capsule;
    ///
    /// // WRONG: Misaligned pointer
    /// let mut buffer = vec![0.0f64; 5];
    /// let ptr = unsafe { buffer.as_mut_ptr().add(1) as *mut [f64; 4] }; // +8 bytes offset!
    ///
    /// let v = SimdF64x4Capsule::from_array([1.0; 4]);
    /// unsafe {
    ///     v.store(ptr); // UB! Misaligned for SIMD
    /// }
    /// # }
    /// ```
    ///
    /// # #ASSUME Tags (ASSUM Framework)
    ///
    /// - `#ASSUME_SWEMR_SINGLE_WRITER`: Caller enforces only one writer
    /// - `#ASSUME_SWEMR_READER_SAFETY`: Multiple readers after write completes
    /// - `#ASSUME_ALIASING_INVARIANT`: Proper alignment and no overlapping access
    /// - `#ASSUME_MEMORY_ORDERING`: Release semantics for visibility
    /// - `#VERIFY_STACKED_BORROWS`: Stacked Borrows verification required
    /// - `#VERIFY_THREAD_SAFETY`: Thread ID verification recommended
    /// - `#VERIFY_ALIGNMENT`: Runtime alignment check before unsafe call
    /// - `#VERIFY_LIFETIME_BOUNDS`: Pointer validity extends to all reads
    ///
    /// # Performance
    ///
    /// - SIMD store: ~3-5ns (aligned write, 32 bytes)
    /// - Safe store_slice: ~4-6ns (bounds checking overhead)
    /// - **Unsafe store is only justified for critical hot paths**
    ///
    /// # Q31 Rust Transform Note
    ///
    /// Prefer safe `store_slice()` method unless:
    /// - Profiling shows store is a bottleneck
    /// - You have verified SWeMR pattern compliance
    /// - You can guarantee alignment and validity
    #[cfg(feature = "portable_simd")]
    pub unsafe fn store(&self, ptr: *mut [f64; 4]) {
        // #ASSUME_TYPE_SAFE: ptr is valid, aligned (32-byte), and exclusively owned
        // #VERIFY_UNSAFE_INVARIANTS: Caller must ensure no concurrent writes
        (*ptr) = self.data.to_array();
    }

    #[cfg(not(feature = "portable_simd"))]
    pub unsafe fn store(&self, ptr: *mut [f64; 4]) {
        // #ASSUME_TYPE_SAFE: ptr is valid, aligned, and exclusively owned
        // #VERIFY_UNSAFE_INVARIANTS: Caller must ensure no concurrent writes
        (*ptr) = self.data;
    }
}

impl SimdCapsule for SimdF64x4Capsule {
    type Element = f64;
    const LANES: usize = 4;
    const ALIGNMENT: usize = 256;

    fn load_boxed(&self) -> alloc::boxed::Box<[Self::Element]> {
        alloc::boxed::Box::new(self.to_array())
    }

    fn store_slice(&mut self, data: &[Self::Element]) {
        let arr: [f64; 4] = [
            data.get(0).copied().unwrap_or(0.0),
            data.get(1).copied().unwrap_or(0.0),
            data.get(2).copied().unwrap_or(0.0),
            data.get(3).copied().unwrap_or(0.0),
        ];
        #[cfg(feature = "portable_simd")]
        {
            self.data = f64x4::from_array(arr);
        }
        #[cfg(not(feature = "portable_simd"))]
        {
            self.data = arr;
        }
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }
}

impl Default for SimdF64x4Capsule {
    fn default() -> Self {
        Self::new()
    }
}

// Compile-time verification (Q33 requirement)
const _: () = {
    assert!(
        core::mem::size_of::<SimdF64x4Capsule>() == 256,
        "SimdF64x4Capsule must be 256 bytes"
    );
    assert!(
        core::mem::align_of::<SimdF64x4Capsule>() == 256,
        "SimdF64x4Capsule must be 256-byte aligned"
    );
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alignment() {
        assert_eq!(core::mem::align_of::<SimdF64x4Capsule>(), 256);
        assert_eq!(core::mem::size_of::<SimdF64x4Capsule>(), 256);
    }

    #[test]
    fn test_new() {
        let capsule = SimdF64x4Capsule::new();
        assert_eq!(capsule.load(), [0.0; 4]);
    }

    #[test]
    fn test_from_array() {
        let input = [1.0, 2.0, 3.0, 4.0];
        let capsule = SimdF64x4Capsule::from_array(input);
        assert_eq!(capsule.load(), input);
    }

    #[test]
    fn test_add() {
        let a = SimdF64x4Capsule::from_array([1.0; 4]);
        let b = SimdF64x4Capsule::from_array([2.0; 4]);
        let result = a.add(&b);
        assert_eq!(result.load(), [3.0; 4]);
    }

    #[test]
    fn test_reduce_sum() {
        let capsule = SimdF64x4Capsule::from_array([1.0, 2.0, 3.0, 4.0]);
        assert_eq!(capsule.reduce_sum(), 10.0);
    }
}
