//! # SimdI32x8Capsule Comprehensive Tests
//!
//! **T28 Testing Framework Applied**: Unit, Property, Integration tests for i32x8 SIMD operations
//!
//! ## Test Coverage
//!
//! - Unit tests (18+): All operations (add, sub, mul, reduce, clamp, saturating, comparisons)
//! - Property tests: Overflow behavior, saturation boundaries, commutativity
//! - Integration tests: Quantization roundtrip, histogram calculation
//! - Edge cases: i32::MIN, i32::MAX, zero, negative values

use atomic_capsule::primitives::{SimdCapsule, SimdI32x8Capsule};

// =============================================================================
// UNIT TESTS (T28 Q1-Q7): Basic functionality
// =============================================================================

#[test]
fn test_alignment_and_size() {
    // Q33: Verify compile-time guarantees
    assert_eq!(core::mem::align_of::<SimdI32x8Capsule>(), 256);
    assert_eq!(core::mem::size_of::<SimdI32x8Capsule>(), 256);
}

#[test]
fn test_new_zero_initialized() {
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
fn test_to_array() {
    let input = [10, 20, 30, 40, 50, 60, 70, 80];
    let capsule = SimdI32x8Capsule::from_array(input);
    let arr = capsule.to_array();
    assert_eq!(arr, input);
}

// =============================================================================
// ARITHMETIC OPERATIONS (T28 Q1-Q7)
// =============================================================================

#[test]
fn test_add_positive() {
    let a = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
    let b = SimdI32x8Capsule::from_array([10, 20, 30, 40, 50, 60, 70, 80]);
    let result = a.add(&b);
    let data = result.load();
    assert_eq!(data, [11, 22, 33, 44, 55, 66, 77, 88]);
}

#[test]
fn test_add_negative() {
    let a = SimdI32x8Capsule::from_array([-1, -2, -3, -4, -5, -6, -7, -8]);
    let b = SimdI32x8Capsule::from_array([10, 20, 30, 40, 50, 60, 70, 80]);
    let result = a.add(&b);
    let data = result.load();
    assert_eq!(data, [9, 18, 27, 36, 45, 54, 63, 72]);
}

#[test]
fn test_sub() {
    let a = SimdI32x8Capsule::from_array([10, 20, 30, 40, 50, 60, 70, 80]);
    let b = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
    let result = a.sub(&b);
    let data = result.load();
    assert_eq!(data, [9, 18, 27, 36, 45, 54, 63, 72]);
}

#[test]
fn test_mul() {
    let a = SimdI32x8Capsule::from_array([2, 3, 4, 5, 6, 7, 8, 9]);
    let b = SimdI32x8Capsule::from_array([10, 10, 10, 10, 10, 10, 10, 10]);
    let result = a.mul(&b);
    let data = result.load();
    assert_eq!(data, [20, 30, 40, 50, 60, 70, 80, 90]);
}

// =============================================================================
// REDUCTION OPERATIONS (T28 Q1-Q7)
// =============================================================================

#[test]
fn test_reduce_sum() {
    let capsule = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
    let sum = capsule.reduce_sum();
    assert_eq!(sum, 36); // 1+2+3+4+5+6+7+8 = 36
}

#[test]
fn test_reduce_sum_negative() {
    let capsule = SimdI32x8Capsule::from_array([-1, 2, -3, 4, -5, 6, -7, 8]);
    let sum = capsule.reduce_sum();
    assert_eq!(sum, 4); // -1+2-3+4-5+6-7+8 = 4
}

#[test]
fn test_reduce_product() {
    let capsule = SimdI32x8Capsule::from_array([1, 2, 2, 1, 1, 1, 1, 1]);
    let product = capsule.reduce_product();
    assert_eq!(product, 4); // 1*2*2*1*1*1*1*1 = 4
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

// =============================================================================
// UNARY OPERATIONS (T28 Q1-Q7)
// =============================================================================

#[test]
fn test_abs_mixed_signs() {
    let capsule = SimdI32x8Capsule::from_array([-1, 2, -3, 4, -5, 6, -7, 8]);
    let result = capsule.abs();
    let data = result.to_array();
    assert_eq!(data, [1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn test_abs_all_negative() {
    let capsule = SimdI32x8Capsule::from_array([-1, -2, -3, -4, -5, -6, -7, -8]);
    let result = capsule.abs();
    let data = result.to_array();
    assert_eq!(data, [1, 2, 3, 4, 5, 6, 7, 8]);
}

// =============================================================================
// ELEMENT-WISE OPERATIONS (T28 Q1-Q7)
// =============================================================================

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

// =============================================================================
// SATURATING ARITHMETIC (T28 Q1-Q7 + ASSUM Verification)
// =============================================================================

#[test]
fn test_saturating_add_no_overflow() {
    let a = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
    let b = SimdI32x8Capsule::from_array([10, 20, 30, 40, 50, 60, 70, 80]);
    let result = a.saturating_add(&b);
    assert_eq!(result.to_array(), [11, 22, 33, 44, 55, 66, 77, 88]);
}

#[test]
fn test_saturating_add_positive_overflow() {
    // ASSUM: #VERIFY_SATURATION_BOUNDARY - Test at i32::MAX
    let a = SimdI32x8Capsule::from_array([i32::MAX; 8]);
    let b = SimdI32x8Capsule::from_array([1; 8]);
    let result = a.saturating_add(&b);
    assert_eq!(result.to_array(), [i32::MAX; 8]); // Saturates at MAX
}

#[test]
fn test_saturating_add_large_values() {
    let a = SimdI32x8Capsule::from_array([i32::MAX - 5; 8]);
    let b = SimdI32x8Capsule::from_array([10; 8]);
    let result = a.saturating_add(&b);
    assert_eq!(result.to_array(), [i32::MAX; 8]); // Saturates
}

#[test]
fn test_saturating_sub_no_underflow() {
    let a = SimdI32x8Capsule::from_array([10, 20, 30, 40, 50, 60, 70, 80]);
    let b = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
    let result = a.saturating_sub(&b);
    assert_eq!(result.to_array(), [9, 18, 27, 36, 45, 54, 63, 72]);
}

#[test]
fn test_saturating_sub_negative_underflow() {
    // ASSUM: #VERIFY_SATURATION_BOUNDARY - Test at i32::MIN
    let a = SimdI32x8Capsule::from_array([i32::MIN; 8]);
    let b = SimdI32x8Capsule::from_array([1; 8]);
    let result = a.saturating_sub(&b);
    assert_eq!(result.to_array(), [i32::MIN; 8]); // Saturates at MIN
}

// =============================================================================
// COMPARISON OPERATIONS (T28 Q1-Q7)
// =============================================================================

#[test]
fn test_simd_eq() {
    let a = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
    let b = SimdI32x8Capsule::from_array([1, 0, 3, 0, 5, 0, 7, 0]);
    let result = a.simd_eq(&b);
    let data = result.to_array();
    assert_eq!(data[0], -1); // true
    assert_eq!(data[1], 0); // false
    assert_eq!(data[2], -1); // true
    assert_eq!(data[3], 0); // false
}

#[test]
fn test_simd_ne() {
    let a = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
    let b = SimdI32x8Capsule::from_array([1, 0, 3, 0, 5, 0, 7, 0]);
    let result = a.simd_ne(&b);
    let data = result.to_array();
    assert_eq!(data[0], 0); // false (equal)
    assert_eq!(data[1], -1); // true (not equal)
}

#[test]
fn test_simd_gt() {
    let a = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
    let b = SimdI32x8Capsule::from_array([2, 2, 2, 5, 5, 5, 8, 8]);
    let result = a.simd_gt(&b);
    let data = result.to_array();
    assert_eq!(data[0], 0); // 1 > 2 = false
    assert_eq!(data[1], 0); // 2 > 2 = false
    assert_eq!(data[2], -1); // 3 > 2 = true
}

#[test]
fn test_simd_lt() {
    let a = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
    let b = SimdI32x8Capsule::from_array([2, 2, 2, 5, 5, 5, 8, 8]);
    let result = a.simd_lt(&b);
    let data = result.to_array();
    assert_eq!(data[0], -1); // 1 < 2 = true
    assert_eq!(data[1], 0); // 2 < 2 = false
    assert_eq!(data[2], 0); // 3 < 2 = false
}

#[test]
fn test_simd_ge() {
    let a = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
    let b = SimdI32x8Capsule::from_array([2, 2, 2, 5, 5, 5, 8, 8]);
    let result = a.simd_ge(&b);
    let data = result.to_array();
    assert_eq!(data[0], 0); // 1 >= 2 = false
    assert_eq!(data[1], -1); // 2 >= 2 = true
    assert_eq!(data[2], -1); // 3 >= 2 = true
}

#[test]
fn test_simd_le() {
    let a = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
    let b = SimdI32x8Capsule::from_array([2, 2, 2, 5, 5, 5, 8, 8]);
    let result = a.simd_le(&b);
    let data = result.to_array();
    assert_eq!(data[0], -1); // 1 <= 2 = true
    assert_eq!(data[1], -1); // 2 <= 2 = true
    assert_eq!(data[2], 0); // 3 <= 2 = false
}

// =============================================================================
// TYPE CONVERSION (T28 Q1-Q7)
// =============================================================================

#[test]
fn test_cast_to_f32_positive() {
    let int_capsule = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
    let float_array = int_capsule.cast_to_f32();
    assert_eq!(float_array, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
}

#[test]
fn test_cast_to_f32_negative() {
    let int_capsule = SimdI32x8Capsule::from_array([-1, -2, -3, -4, -5, -6, -7, -8]);
    let float_array = int_capsule.cast_to_f32();
    assert_eq!(
        float_array,
        [-1.0, -2.0, -3.0, -4.0, -5.0, -6.0, -7.0, -8.0]
    );
}

#[test]
fn test_cast_to_f32_mixed() {
    let int_capsule = SimdI32x8Capsule::from_array([-100, -50, -10, 0, 10, 50, 100, 200]);
    let float_array = int_capsule.cast_to_f32();
    assert_eq!(
        float_array,
        [-100.0, -50.0, -10.0, 0.0, 10.0, 50.0, 100.0, 200.0]
    );
}

// =============================================================================
// GENERATION COUNTER (T28 Q1-Q7 + Q33 Atomic Pattern)
// =============================================================================

#[test]
fn test_generation_counter_increments() {
    let a = SimdI32x8Capsule::new();
    let gen1 = a.generation();

    let b = SimdI32x8Capsule::from_array([1; 8]);
    let result = a.add(&b);

    let gen2 = result.generation();
    assert!(gen2 > gen1); // Generation increments on operations
}

#[test]
fn test_generation_initial_zero() {
    let capsule = SimdI32x8Capsule::new();
    assert_eq!(capsule.generation(), 0);
}

// =============================================================================
// PROPERTY TESTS (T28 Q8-Q14)
// =============================================================================

#[test]
fn property_add_commutative() {
    // Property: a + b == b + a
    let a = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
    let b = SimdI32x8Capsule::from_array([10, 20, 30, 40, 50, 60, 70, 80]);

    let ab = a.add(&b);
    let ba = b.add(&a);

    assert_eq!(ab.to_array(), ba.to_array());
}

#[test]
fn property_add_associative() {
    // Property: (a + b) + c == a + (b + c)
    let a = SimdI32x8Capsule::from_array([1, 2, 3, 4, 5, 6, 7, 8]);
    let b = SimdI32x8Capsule::from_array([10, 20, 30, 40, 50, 60, 70, 80]);
    let c = SimdI32x8Capsule::from_array([5, 5, 5, 5, 5, 5, 5, 5]);

    let ab_c = a.add(&b).add(&c);
    let a_bc = a.add(&b.add(&c));

    assert_eq!(ab_c.to_array(), a_bc.to_array());
}

#[test]
fn property_mul_commutative() {
    // Property: a * b == b * a
    let a = SimdI32x8Capsule::from_array([2, 3, 4, 5, 6, 7, 8, 9]);
    let b = SimdI32x8Capsule::from_array([5, 5, 5, 5, 5, 5, 5, 5]);

    let ab = a.mul(&b);
    let ba = b.mul(&a);

    assert_eq!(ab.to_array(), ba.to_array());
}

#[test]
fn property_saturating_add_idempotent_at_max() {
    // Property: MAX + x saturates to MAX for all x > 0
    let max_capsule = SimdI32x8Capsule::from_array([i32::MAX; 8]);
    let positive =
        SimdI32x8Capsule::from_array([1, 10, 100, 1000, 10000, 100000, 1000000, 10000000]);

    let result = max_capsule.saturating_add(&positive);
    assert_eq!(result.to_array(), [i32::MAX; 8]);
}

#[test]
fn property_saturating_sub_idempotent_at_min() {
    // Property: MIN - x saturates to MIN for all x > 0
    let min_capsule = SimdI32x8Capsule::from_array([i32::MIN; 8]);
    let positive =
        SimdI32x8Capsule::from_array([1, 10, 100, 1000, 10000, 100000, 1000000, 10000000]);

    let result = min_capsule.saturating_sub(&positive);
    assert_eq!(result.to_array(), [i32::MIN; 8]);
}

#[test]
fn property_min_max_duality() {
    // Property: min(a, b) + max(a, b) == a + b
    let a = SimdI32x8Capsule::from_array([1, 5, 3, 7, 2, 6, 4, 8]);
    let b = SimdI32x8Capsule::from_array([4, 2, 6, 1, 5, 3, 7, 2]);

    let min_result = a.simd_min(&b);
    let max_result = a.simd_max(&b);
    let sum_min_max = min_result.add(&max_result);
    let sum_ab = a.add(&b);

    assert_eq!(sum_min_max.to_array(), sum_ab.to_array());
}

// =============================================================================
// INTEGRATION TESTS (T28 Q15-Q21)
// =============================================================================

#[test]
fn integration_quantization_roundtrip() {
    // Simulate 8-bit quantization workflow (Phase 3 brain compression use case)
    // Input: f32 weights in range [-1.0, 1.0]
    let weights_f32 = [-1.0, -0.5, -0.25, 0.0, 0.25, 0.5, 0.75, 1.0];

    // Quantize to i32 (scale by 127 for 8-bit range)
    let scale = 127.0;
    let weights_i32: [i32; 8] = [
        (weights_f32[0] * scale) as i32,
        (weights_f32[1] * scale) as i32,
        (weights_f32[2] * scale) as i32,
        (weights_f32[3] * scale) as i32,
        (weights_f32[4] * scale) as i32,
        (weights_f32[5] * scale) as i32,
        (weights_f32[6] * scale) as i32,
        (weights_f32[7] * scale) as i32,
    ];

    let quantized = SimdI32x8Capsule::from_array(weights_i32);

    // Dequantize back to f32
    let dequantized_f32 = quantized.cast_to_f32();
    let recovered: [f32; 8] = [
        dequantized_f32[0] / scale,
        dequantized_f32[1] / scale,
        dequantized_f32[2] / scale,
        dequantized_f32[3] / scale,
        dequantized_f32[4] / scale,
        dequantized_f32[5] / scale,
        dequantized_f32[6] / scale,
        dequantized_f32[7] / scale,
    ];

    // Verify roundtrip accuracy within quantization error
    for i in 0..8 {
        let error = (recovered[i] - weights_f32[i]).abs();
        assert!(error < 0.01, "Quantization error too large: {}", error);
    }
}

#[test]
fn integration_histogram_calculation() {
    // Simulate histogram bin counting (8 bins)
    let bin_counts = SimdI32x8Capsule::from_array([10, 20, 15, 30, 25, 5, 12, 18]);
    let new_samples = SimdI32x8Capsule::from_array([2, 3, 1, 5, 4, 0, 2, 3]);

    let updated_histogram = bin_counts.add(&new_samples);
    assert_eq!(
        updated_histogram.to_array(),
        [12, 23, 16, 35, 29, 5, 14, 21]
    );

    let total_count = updated_histogram.reduce_sum();
    assert_eq!(total_count, 155);
}

#[test]
fn integration_feature_extraction() {
    // Simulate extracting 8 features from raw data
    let raw_features = SimdI32x8Capsule::from_array([100, 200, 150, 300, 250, 50, 120, 180]);

    // Normalize to range [0, 255] (8-bit)
    let _max_feature = SimdI32x8Capsule::splat(raw_features.reduce_max());
    let min_feature = SimdI32x8Capsule::splat(raw_features.reduce_min());
    let max_val = SimdI32x8Capsule::splat(255);

    // Clamp to valid range
    let clamped = raw_features.simd_clamp(&min_feature, &max_val);

    // All values should be within [50, 255]
    let data = clamped.to_array();
    for &val in &data {
        assert!(val >= 50 && val <= 255);
    }
}

// =============================================================================
// EDGE CASE TESTS (T28 Q15-Q21)
// =============================================================================

#[test]
fn edge_case_i32_max() {
    let capsule = SimdI32x8Capsule::from_array([i32::MAX; 8]);
    let data = capsule.load();
    assert_eq!(data, [i32::MAX; 8]);
}

#[test]
fn edge_case_i32_min() {
    let capsule = SimdI32x8Capsule::from_array([i32::MIN; 8]);
    let data = capsule.load();
    assert_eq!(data, [i32::MIN; 8]);
}

#[test]
fn edge_case_zero() {
    let capsule = SimdI32x8Capsule::from_array([0; 8]);

    // Adding zero should be identity
    let result = capsule.add(&capsule);
    assert_eq!(result.to_array(), [0; 8]);

    // Multiplying by zero should give zero
    let one = SimdI32x8Capsule::from_array([1; 8]);
    let result = one.mul(&capsule);
    assert_eq!(result.to_array(), [0; 8]);
}

#[test]
fn edge_case_abs_i32_min() {
    // ASSUM: #VERIFY_ABS_EDGE_CASE - i32::MIN.abs() wraps to i32::MIN
    let capsule = SimdI32x8Capsule::from_array([i32::MIN; 8]);
    let result = capsule.abs();
    assert_eq!(result.to_array(), [i32::MIN; 8]); // Documented wrapping behavior
}

#[test]
fn edge_case_mixed_extremes() {
    let capsule = SimdI32x8Capsule::from_array([i32::MIN, i32::MAX, 0, 1, -1, 100, -100, 42]);
    let data = capsule.load();
    assert_eq!(data[0], i32::MIN);
    assert_eq!(data[1], i32::MAX);
    assert_eq!(data[2], 0);
}
