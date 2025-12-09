//! Q16.16 fixed-point AVX2 operations
//!
//! 8-wide SIMD operations for Q16.16 fixed-point arithmetic.
//!
//! # Fixed-Point Format
//!
//! Q16.16 = 16 integer bits + 16 fractional bits (i32 storage)
//! - 1.0 = 0x00010000 (65536)
//! - 0.5 = 0x00008000 (32768)
//! - Range: [-32768.0, 32767.99998]
//!
//! # Safety
//!
//! All functions marked with `#[target_feature(enable = "avx2")]` and `unsafe`.
//! Caller must ensure:
//! - AVX2 available via `is_x86_feature_detected!("avx2")`
//! - Aligned pointers for `*_aligned` functions (32-byte alignment)
//! - Valid memory ranges for load/store operations
//!
//! # ASSUM Tags
//!
//! - #ASSUME_AVX2_AVAILABLE: Caller verified CPU support
//! - #ASSUME_Q16_SATURATION: Results saturated to i32 range
//! - #ASSUME_ALIGNED_PTR: Pointers 32-byte aligned for load_aligned/store_aligned
//! - #ASSUME_VALID_MEMORY: Pointers reference valid memory (8× i32 elements)

#![allow(clippy::missing_safety_doc)] // Safety documented at module level

use core::arch::x86_64::*;

// ============================================================================
// Load/Store Operations
// ============================================================================

/// Load 8× Q16.16 values from 32-byte aligned memory
///
/// # Safety
///
/// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
/// - #ASSUME_ALIGNED_PTR: `ptr` must be 32-byte aligned (256-bit boundary)
/// - #ASSUME_VALID_MEMORY: `ptr[0..8]` must be valid i32 elements
///
/// # Performance
///
/// - Latency: 1 cycle (aligned load)
/// - Throughput: 2 loads/cycle (dual-port cache)
#[target_feature(enable = "avx2")]
#[inline]
pub unsafe fn load_q16_aligned(ptr: *const i32) -> __m256i {
    // #VERIFY: Debug assertion for alignment
    debug_assert_eq!(ptr as usize % 32, 0, "load_q16_aligned: ptr not 32-byte aligned");
    _mm256_load_si256(ptr as *const __m256i)
}

/// Load 8× Q16.16 values from unaligned memory
///
/// # Safety
///
/// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
/// - #ASSUME_VALID_MEMORY: `ptr[0..8]` must be valid i32 elements
///
/// # Performance
///
/// - Latency: 1 cycle (unaligned load, may split cache line)
/// - Throughput: 1 load/cycle
/// - Use aligned variant when possible for 2× throughput
#[target_feature(enable = "avx2")]
#[inline]
pub unsafe fn load_q16_unaligned(ptr: *const i32) -> __m256i {
    _mm256_loadu_si256(ptr as *const __m256i)
}

/// Store 8× Q16.16 values to 32-byte aligned memory
///
/// # Safety
///
/// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
/// - #ASSUME_ALIGNED_PTR: `ptr` must be 32-byte aligned
/// - #ASSUME_VALID_MEMORY: `ptr[0..8]` must be valid writable i32 storage
///
/// # Performance
///
/// - Latency: 1 cycle
/// - Throughput: 1 store/cycle
#[target_feature(enable = "avx2")]
#[inline]
pub unsafe fn store_q16_aligned(ptr: *mut i32, v: __m256i) {
    debug_assert_eq!(ptr as usize % 32, 0, "store_q16_aligned: ptr not 32-byte aligned");
    _mm256_store_si256(ptr as *mut __m256i, v)
}

/// Store 8× Q16.16 values to unaligned memory
///
/// # Safety
///
/// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
/// - #ASSUME_VALID_MEMORY: `ptr[0..8]` must be valid writable i32 storage
#[target_feature(enable = "avx2")]
#[inline]
pub unsafe fn store_q16_unaligned(ptr: *mut i32, v: __m256i) {
    _mm256_storeu_si256(ptr as *mut __m256i, v)
}

// ============================================================================
// Arithmetic Operations
// ============================================================================

/// Multiply two Q16.16 vectors: (a * b) >> 16
///
/// Performs full 32×32→64 bit multiply, then shifts right by 16 to maintain
/// Q16.16 format.
///
/// # Safety
///
/// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
/// - #ASSUME_Q16_SATURATION: Results saturated to i32 range (clamp externally if needed)
///
/// # Algorithm
///
/// For each lane i:
/// 1. Multiply a[i] * b[i] → 64-bit result
/// 2. Shift right by 16 (Q16.16 scaling)
/// 3. Truncate to 32-bit (overflow possible if inputs out of range)
///
/// # Performance
///
/// - Latency: 5 cycles (mul_epi32 + blend)
/// - Throughput: 0.5 ops/cycle
#[target_feature(enable = "avx2")]
#[inline]
pub unsafe fn mul_q16_avx2(a: __m256i, b: __m256i) -> __m256i {
    // Split into even/odd 32-bit elements for 32×32→64 multiply
    // Even elements: lanes 0,2,4,6
    let prod_even = _mm256_mul_epi32(a, b);

    // Odd elements: lanes 1,3,5,7 (shift right by 32 to get upper halves)
    let a_odd = _mm256_srli_epi64(a, 32);
    let b_odd = _mm256_srli_epi64(b, 32);
    let prod_odd = _mm256_mul_epi32(a_odd, b_odd);

    // Shift products right by 16 (Q16.16 scaling)
    let even_shifted = _mm256_srli_epi64(prod_even, 16);
    let odd_shifted = _mm256_srli_epi64(prod_odd, 16);

    // Repack: even elements in lower 32 bits, odd in upper 32 bits
    // Use shuffle to interleave even/odd results
    let even_packed = _mm256_shuffle_epi32(even_shifted, 0b00_00_10_00); // [e0,e0,e2,e0, e4,e4,e6,e4]
    let odd_packed = _mm256_shuffle_epi32(odd_shifted, 0b00_00_10_00);   // [o1,o1,o3,o1, o5,o5,o7,o5]

    // Blend: take even from even_packed, odd from odd_packed
    _mm256_blend_epi32(even_packed, odd_packed, 0b10101010)
}

/// Add two Q16.16 vectors
///
/// # Safety
///
/// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
/// - #ASSUME_Q16_SATURATION: Overflow wraps (use clamp_unit_avx2 for [0,1] clamping)
///
/// # Performance
///
/// - Latency: 1 cycle
/// - Throughput: 3 ops/cycle
#[target_feature(enable = "avx2")]
#[inline]
pub unsafe fn add_q16_avx2(a: __m256i, b: __m256i) -> __m256i {
    _mm256_add_epi32(a, b)
}

/// Subtract two Q16.16 vectors
///
/// # Safety
///
/// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
/// - #ASSUME_Q16_SATURATION: Underflow wraps (use clamp_unit_avx2 for [0,1] clamping)
#[target_feature(enable = "avx2")]
#[inline]
pub unsafe fn sub_q16_avx2(a: __m256i, b: __m256i) -> __m256i {
    _mm256_sub_epi32(a, b)
}

/// Subtract two Q16.16 vectors with saturation to [0.0, 1.0] range
///
/// Equivalent to clamp_unit_avx2(sub_q16_avx2(a, b)).
///
/// # Safety
///
/// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
#[target_feature(enable = "avx2")]
#[inline]
pub unsafe fn sub_q16_sat_avx2(a: __m256i, b: __m256i) -> __m256i {
    clamp_unit_avx2(sub_q16_avx2(a, b))
}

// ============================================================================
// Comparison Operations
// ============================================================================

/// Branchless threshold comparison: returns mask where v > threshold
///
/// Returns all-ones (0xFFFFFFFF) for lanes where v > threshold, else all-zeros.
///
/// # Safety
///
/// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
///
/// # Performance
///
/// - Latency: 1 cycle
/// - Throughput: 2 ops/cycle
/// - Branchless (no misprediction penalty)
#[target_feature(enable = "avx2")]
#[inline]
pub unsafe fn threshold_gt_avx2(v: __m256i, threshold: __m256i) -> __m256i {
    _mm256_cmpgt_epi32(v, threshold)
}

/// Branchless select: if mask bit set, take a, else b
///
/// For each byte: if mask[i] MSB set, result[i] = a[i], else result[i] = b[i]
///
/// # Safety
///
/// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
///
/// # Performance
///
/// - Latency: 1 cycle
/// - Throughput: 2 ops/cycle
#[target_feature(enable = "avx2")]
#[inline]
pub unsafe fn select_avx2(mask: __m256i, a: __m256i, b: __m256i) -> __m256i {
    _mm256_blendv_epi8(b, a, mask)
}

/// Minimum of two Q16.16 vectors (element-wise)
///
/// # Safety
///
/// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
#[target_feature(enable = "avx2")]
#[inline]
pub unsafe fn min_q16_avx2(a: __m256i, b: __m256i) -> __m256i {
    _mm256_min_epi32(a, b)
}

/// Maximum of two Q16.16 vectors (element-wise)
///
/// # Safety
///
/// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
#[target_feature(enable = "avx2")]
#[inline]
pub unsafe fn max_q16_avx2(a: __m256i, b: __m256i) -> __m256i {
    _mm256_max_epi32(a, b)
}

/// Clamp Q16.16 to [0.0, 1.0] range (0x00000000 to 0x00010000)
///
/// Saturates values below 0.0 to 0.0 and above 1.0 to 1.0.
///
/// # Safety
///
/// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
///
/// # Performance
///
/// - Latency: 2 cycles (2× min/max)
/// - Throughput: 1 ops/cycle
#[target_feature(enable = "avx2")]
#[inline]
pub unsafe fn clamp_unit_avx2(v: __m256i) -> __m256i {
    let zero = _mm256_setzero_si256();
    let one = _mm256_set1_epi32(0x0001_0000); // Q16.16 1.0
    max_q16_avx2(zero, min_q16_avx2(v, one))
}

// ============================================================================
// Horizontal Reductions
// ============================================================================

/// Horizontal sum of 8× i32 elements
///
/// Returns sum of all 8 lanes as scalar i32.
///
/// # Safety
///
/// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
/// - #ASSUME_Q16_SATURATION: Overflow wraps (ensure inputs won't overflow)
///
/// # Performance
///
/// - Latency: 4-5 cycles
/// - Throughput: 1 ops/cycle
#[target_feature(enable = "avx2")]
#[inline]
pub unsafe fn hsum_avx2(v: __m256i) -> i32 {
    // Add high and low 128-bit lanes
    let hi = _mm256_extracti128_si256(v, 1);
    let lo = _mm256_castsi256_si128(v);
    let sum128 = _mm_add_epi32(hi, lo);

    // Horizontal add within 128-bit lane
    let sum64 = _mm_hadd_epi32(sum128, sum128);
    let sum32 = _mm_hadd_epi32(sum64, sum64);

    _mm_cvtsi128_si32(sum32)
}

/// Horizontal max of 8× i32 elements
///
/// Returns maximum of all 8 lanes as scalar i32.
///
/// # Safety
///
/// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
///
/// # Performance
///
/// - Latency: 5-6 cycles
/// - Throughput: 1 ops/cycle
#[target_feature(enable = "avx2")]
#[inline]
pub unsafe fn hmax_avx2(v: __m256i) -> i32 {
    let hi = _mm256_extracti128_si256(v, 1);
    let lo = _mm256_castsi256_si128(v);
    let max128 = _mm_max_epi32(hi, lo);

    // Shuffle and max to reduce to single element
    let max64 = _mm_max_epi32(max128, _mm_shuffle_epi32(max128, 0b00_00_11_10));
    let max32 = _mm_max_epi32(max64, _mm_shuffle_epi32(max64, 0b00_00_00_01));

    _mm_cvtsi128_si32(max32)
}

/// Horizontal min of 8× i32 elements
///
/// Returns minimum of all 8 lanes as scalar i32.
///
/// # Safety
///
/// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
#[target_feature(enable = "avx2")]
#[inline]
pub unsafe fn hmin_avx2(v: __m256i) -> i32 {
    let hi = _mm256_extracti128_si256(v, 1);
    let lo = _mm256_castsi256_si128(v);
    let min128 = _mm_min_epi32(hi, lo);

    let min64 = _mm_min_epi32(min128, _mm_shuffle_epi32(min128, 0b00_00_11_10));
    let min32 = _mm_min_epi32(min64, _mm_shuffle_epi32(min64, 0b00_00_00_01));

    _mm_cvtsi128_si32(min32)
}

// ============================================================================
// Broadcast Operations
// ============================================================================

/// Set all 8 lanes to the same Q16.16 value
///
/// # Safety
///
/// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
///
/// # Performance
///
/// - Latency: 1 cycle
/// - Throughput: 1 ops/cycle
#[target_feature(enable = "avx2")]
#[inline]
pub unsafe fn broadcast_q16_avx2(val: i32) -> __m256i {
    _mm256_set1_epi32(val)
}

// ============================================================================
// Constants
// ============================================================================

/// Q16.16 constant vectors
pub mod constants {
    use core::arch::x86_64::*;

    /// Q16.16 zero vector (all 0.0 = 0x00000000)
    ///
    /// # Safety
    ///
    /// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
    #[target_feature(enable = "avx2")]
    #[inline]
    pub unsafe fn zero() -> __m256i {
        _mm256_setzero_si256()
    }

    /// Q16.16 one vector (all 1.0 = 0x00010000)
    ///
    /// # Safety
    ///
    /// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
    #[target_feature(enable = "avx2")]
    #[inline]
    pub unsafe fn one() -> __m256i {
        _mm256_set1_epi32(0x0001_0000)
    }

    /// Q16.16 half vector (all 0.5 = 0x00008000)
    ///
    /// # Safety
    ///
    /// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
    #[target_feature(enable = "avx2")]
    #[inline]
    pub unsafe fn half() -> __m256i {
        _mm256_set1_epi32(0x0000_8000)
    }

    /// Q16.16 epsilon vector (all ~0.0000153 = 0x00000001)
    ///
    /// Smallest representable positive value in Q16.16.
    ///
    /// # Safety
    ///
    /// - #ASSUME_AVX2_AVAILABLE: Caller verified AVX2 support
    #[target_feature(enable = "avx2")]
    #[inline]
    pub unsafe fn epsilon() -> __m256i {
        _mm256_set1_epi32(0x0000_0001)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to run AVX2 tests only on supported hardware
    macro_rules! test_avx2 {
        ($name:ident, $body:expr) => {
            #[test]
            fn $name() {
                if is_x86_feature_detected!("avx2") {
                    unsafe { $body }
                } else {
                    println!("Skipping {} - AVX2 not available", stringify!($name));
                }
            }
        };
    }

    test_avx2!(test_load_store_aligned, {
        // 32-byte aligned storage
        #[repr(align(32))]
        struct Aligned([i32; 8]);

        let mut data = Aligned([1, 2, 3, 4, 5, 6, 7, 8]);
        let ptr = data.0.as_ptr();

        // Load
        let v = load_q16_aligned(ptr);

        // Store back
        let mut result = Aligned([0; 8]);
        store_q16_aligned(result.0.as_mut_ptr(), v);

        assert_eq!(result.0, [1, 2, 3, 4, 5, 6, 7, 8]);
    });

    test_avx2!(test_load_store_unaligned, {
        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let v = load_q16_unaligned(data.as_ptr());

        let mut result = [0; 8];
        store_q16_unaligned(result.as_mut_ptr(), v);

        assert_eq!(result, [1, 2, 3, 4, 5, 6, 7, 8]);
    });

    test_avx2!(test_add_q16, {
        let a = broadcast_q16_avx2(0x0001_0000); // 1.0
        let b = broadcast_q16_avx2(0x0000_8000); // 0.5
        let sum = add_q16_avx2(a, b);

        let mut result = [0; 8];
        store_q16_unaligned(result.as_mut_ptr(), sum);

        // All lanes should be 1.5 = 0x00018000
        for &val in &result {
            assert_eq!(val, 0x0001_8000);
        }
    });

    test_avx2!(test_sub_q16, {
        let a = broadcast_q16_avx2(0x0001_0000); // 1.0
        let b = broadcast_q16_avx2(0x0000_8000); // 0.5
        let diff = sub_q16_avx2(a, b);

        let mut result = [0; 8];
        store_q16_unaligned(result.as_mut_ptr(), diff);

        // All lanes should be 0.5 = 0x00008000
        for &val in &result {
            assert_eq!(val, 0x0000_8000);
        }
    });

    test_avx2!(test_mul_q16, {
        let a = broadcast_q16_avx2(0x0002_0000); // 2.0
        let b = broadcast_q16_avx2(0x0000_8000); // 0.5
        let prod = mul_q16_avx2(a, b);

        let mut result = [0; 8];
        store_q16_unaligned(result.as_mut_ptr(), prod);

        // All lanes should be 1.0 = 0x00010000
        for &val in &result {
            assert_eq!(val, 0x0001_0000, "Expected 1.0, got 0x{:08x}", val);
        }
    });

    test_avx2!(test_threshold_gt, {
        let data = [
            0x0000_8000, // 0.5
            0x0001_0000, // 1.0
            0x0000_4000, // 0.25
            0x0001_8000, // 1.5
            0x0000_0000, // 0.0
            0x0002_0000, // 2.0
            0x0000_C000, // 0.75
            0x0001_4000, // 1.25
        ];
        let v = load_q16_unaligned(data.as_ptr());
        let threshold = broadcast_q16_avx2(0x0001_0000); // 1.0

        let mask = threshold_gt_avx2(v, threshold);
        let mut result = [0; 8];
        store_q16_unaligned(result.as_mut_ptr(), mask);

        // Values > 1.0 should have mask = 0xFFFFFFFF
        assert_eq!(result[0], 0x0000_0000); // 0.5 <= 1.0
        assert_eq!(result[1], 0x0000_0000); // 1.0 <= 1.0
        assert_eq!(result[2], 0x0000_0000); // 0.25 <= 1.0
        assert_eq!(result[3] as u32, 0xFFFF_FFFF); // 1.5 > 1.0
        assert_eq!(result[4], 0x0000_0000); // 0.0 <= 1.0
        assert_eq!(result[5] as u32, 0xFFFF_FFFF); // 2.0 > 1.0
        assert_eq!(result[6], 0x0000_0000); // 0.75 <= 1.0
        assert_eq!(result[7] as u32, 0xFFFF_FFFF); // 1.25 > 1.0
    });

    test_avx2!(test_select, {
        let a = broadcast_q16_avx2(0x0001_0000); // 1.0
        let b = broadcast_q16_avx2(0x0000_0000); // 0.0
        let mask = broadcast_q16_avx2(0xFFFF_FFFF_u32 as i32); // All ones

        let result = select_avx2(mask, a, b);
        let mut out = [0; 8];
        store_q16_unaligned(out.as_mut_ptr(), result);

        // All lanes should select 'a' (1.0)
        for &val in &out {
            assert_eq!(val, 0x0001_0000);
        }
    });

    test_avx2!(test_min_max, {
        let a = broadcast_q16_avx2(0x0001_0000); // 1.0
        let b = broadcast_q16_avx2(0x0000_8000); // 0.5

        let min = min_q16_avx2(a, b);
        let max = max_q16_avx2(a, b);

        let mut min_result = [0; 8];
        let mut max_result = [0; 8];
        store_q16_unaligned(min_result.as_mut_ptr(), min);
        store_q16_unaligned(max_result.as_mut_ptr(), max);

        for &val in &min_result {
            assert_eq!(val, 0x0000_8000); // 0.5
        }
        for &val in &max_result {
            assert_eq!(val, 0x0001_0000); // 1.0
        }
    });

    test_avx2!(test_clamp_unit, {
        let data = [
            -0x0001_0000, // -1.0 (underflow)
            0x0000_8000,  // 0.5 (valid)
            0x0002_0000,  // 2.0 (overflow)
            0x0001_0000,  // 1.0 (valid)
            0x0000_0000,  // 0.0 (valid)
            0x0003_0000,  // 3.0 (overflow)
            0x0000_4000,  // 0.25 (valid)
            -0x0000_8000, // -0.5 (underflow)
        ];
        let v = load_q16_unaligned(data.as_ptr());
        let clamped = clamp_unit_avx2(v);

        let mut result = [0; 8];
        store_q16_unaligned(result.as_mut_ptr(), clamped);

        assert_eq!(result[0], 0x0000_0000); // -1.0 → 0.0
        assert_eq!(result[1], 0x0000_8000); // 0.5 → 0.5
        assert_eq!(result[2], 0x0001_0000); // 2.0 → 1.0
        assert_eq!(result[3], 0x0001_0000); // 1.0 → 1.0
        assert_eq!(result[4], 0x0000_0000); // 0.0 → 0.0
        assert_eq!(result[5], 0x0001_0000); // 3.0 → 1.0
        assert_eq!(result[6], 0x0000_4000); // 0.25 → 0.25
        assert_eq!(result[7], 0x0000_0000); // -0.5 → 0.0
    });

    test_avx2!(test_hsum, {
        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let v = load_q16_unaligned(data.as_ptr());
        let sum = hsum_avx2(v);

        assert_eq!(sum, 36); // 1+2+3+4+5+6+7+8 = 36
    });

    test_avx2!(test_hmax, {
        let data = [1, 20, 3, 4, 5, 6, 7, 8];
        let v = load_q16_unaligned(data.as_ptr());
        let max = hmax_avx2(v);

        assert_eq!(max, 20);
    });

    test_avx2!(test_hmin, {
        let data = [10, 2, 30, 4, 5, 6, 7, 8];
        let v = load_q16_unaligned(data.as_ptr());
        let min = hmin_avx2(v);

        assert_eq!(min, 2);
    });

    test_avx2!(test_broadcast, {
        let v = broadcast_q16_avx2(0x1234_5678);
        let mut result = [0; 8];
        store_q16_unaligned(result.as_mut_ptr(), v);

        for &val in &result {
            assert_eq!(val, 0x1234_5678);
        }
    });

    test_avx2!(test_constants, {
        let zero = constants::zero();
        let one = constants::one();
        let half = constants::half();
        let epsilon = constants::epsilon();

        let mut z = [0; 8];
        let mut o = [0; 8];
        let mut h = [0; 8];
        let mut e = [0; 8];

        store_q16_unaligned(z.as_mut_ptr(), zero);
        store_q16_unaligned(o.as_mut_ptr(), one);
        store_q16_unaligned(h.as_mut_ptr(), half);
        store_q16_unaligned(e.as_mut_ptr(), epsilon);

        for &val in &z {
            assert_eq!(val, 0x0000_0000);
        }
        for &val in &o {
            assert_eq!(val, 0x0001_0000);
        }
        for &val in &h {
            assert_eq!(val, 0x0000_8000);
        }
        for &val in &e {
            assert_eq!(val, 0x0000_0001);
        }
    });

    test_avx2!(test_mul_precision, {
        // Test Q16.16 multiplication precision
        // 0.75 * 0.5 = 0.375
        let a = broadcast_q16_avx2(0x0000_C000); // 0.75
        let b = broadcast_q16_avx2(0x0000_8000); // 0.5
        let prod = mul_q16_avx2(a, b);

        let mut result = [0; 8];
        store_q16_unaligned(result.as_mut_ptr(), prod);

        // Expected: 0.375 = 0x00006000
        for &val in &result {
            assert_eq!(val, 0x0000_6000, "Expected 0.375, got 0x{:08x}", val);
        }
    });
}
