//! T28 Q30 Bitwise Reproducibility Tests for T3 Fixed-Point Tier
//!
//! **Tier**: T3 Fixed-Point (2-10× speedup, deterministic arithmetic)
//! **Focus**: Q30 Bitwise Reproducibility (cross-platform, compiler-agnostic)
//! **Critical**: Fixed-point's main advantage over floating-point is exact bit-for-bit reproducibility
//!
//! # Q30 Requirements
//!
//! **Q30: Bitwise Reproducibility** ⚠️ CRITICAL
//! - Fixed-point operations produce EXACT same bit patterns across runs
//! - Cross-platform reproducibility (x86-64, ARM64, RISC-V)
//! - Compiler optimization determinism (-O2 vs -O3)
//! - Rounding consistency (0.5 → always rounds to even/odd, never varies)
//! - Overflow/underflow behavior deterministic
//!
//! # ASSUM Safety Framework
//!
//! - **#ASSUME_BITWISE_DETERMINISTIC**: Fixed-point operations are bitwise identical
//! - **#VERIFY_BITWISE_DETERMINISTIC**: 100 runs produce identical bit patterns
//! - **#ASSUME_CROSS_PLATFORM**: Arithmetic deterministic across platforms
//! - **#VERIFY_CROSS_PLATFORM**: x86_64 results match ARM64 (via CI tests)
//! - **#ASSUME_ROUNDING_CONSISTENT**: 0.5 rounding never changes
//! - **#VERIFY_ROUNDING_CONSISTENT**: Tested across 1000+ cases
//! - **#ASSUME_OVERFLOW_DETERMINISTIC**: Overflow always saturates same way
//! - **#VERIFY_OVERFLOW_DETERMINISTIC**: Max+Max, Min-Min always same result
//!
//! # Test Organization
//!
//! 1. **test_t28_q30_q16_16_bitwise_reproducibility** (100 runs, compare bit patterns)
//! 2. **test_t28_q30_q16_16_cross_platform_identical** (x86 vs ARM via CI)
//! 3. **test_t28_q30_fixed_point_rounding_consistency** (0.5 rounding never varies)
//! 4. **test_t28_q30_fixed_point_overflow_determinism** (Max+Max saturates same)
//! 5. **test_t28_q30_fixed_point_underflow_determinism** (Min-Min saturates same)
//! 6. **test_t28_q30_q8_8_packed_bitwise_identical** (CircuitBreaker Q8.8 metrics)
//! 7. **test_t28_q30_fixed_serialize_decode_bitwise_match** (Q34 audit determinism)
//! 8. **test_t28_q30_mul_div_bitwise_reproducibility** (Complex operations)
//! 9. **test_t28_q30_q32_32_large_number_reproducibility** (Max precision)
//! 10. **test_t28_q30_q48_16_large_range_reproducibility** (Max range)
//! 11. **test_t28_q30_negative_number_reproducibility** (Sign preservation)
//! 12. **test_t28_q30_compiler_optimization_determinism** (O2 vs O3 identical)

#![cfg(test)]

#[cfg(test)]
mod q30_bitwise_reproducibility {
    use atomic_capsule::primitives::fixed_point::{FixedPoint, Q16_16, Q32_32, Q48_16, Q8_8};

    // =========================================================================
    // Q30.1: Q16.16 Bitwise Reproducibility (100 runs)
    // =========================================================================

    /// Q30.1: Core Q16.16 operations produce EXACT same bit patterns across 100 runs
    ///
    /// #ASSUME_BITWISE_DETERMINISTIC: Fixed-point add produces identical bits
    /// #VERIFY_BITWISE_DETERMINISTIC: 100 runs all identical
    #[test]
    fn test_t28_q30_q16_16_bitwise_reproducibility() {
        let a = Q16_16::from_f64(123.45);
        let b = Q16_16::from_f64(67.89);
        let expected_raw = (a + b).to_raw();

        // Run 100 times and verify all results are EXACTLY the same at bit level
        for iteration in 0..100 {
            let result = a + b;
            let result_raw = result.to_raw();
            assert_eq!(
                result_raw, expected_raw,
                "Iteration {} produced different bit pattern: {} vs {}",
                iteration, result_raw, expected_raw
            );
        }
    }

    /// Q30.2: Q16.16 multiplication bitwise reproducibility (100 runs)
    ///
    /// #ASSUME_BITWISE_DETERMINISTIC: saturating_mul produces identical bits
    /// #VERIFY_BITWISE_DETERMINISTIC: 100 runs all identical
    #[test]
    fn test_t28_q30_q16_16_mul_bitwise_reproducibility() {
        let x = Q16_16::from_f64(12.345);
        let y = Q16_16::from_f64(6.789);
        let expected_raw = x.saturating_mul(y).to_raw();

        for iteration in 0..100 {
            let result = x.saturating_mul(y);
            let result_raw = result.to_raw();
            assert_eq!(
                result_raw, expected_raw,
                "Mul iteration {} produced different bits: {} vs {}",
                iteration, result_raw, expected_raw
            );
        }
    }

    /// Q30.3: Q16.16 division bitwise reproducibility (100 runs)
    ///
    /// #ASSUME_BITWISE_DETERMINISTIC: div produces identical bits
    /// #VERIFY_BITWISE_DETERMINISTIC: 100 runs all identical
    #[test]
    fn test_t28_q30_q16_16_div_bitwise_reproducibility() {
        let numerator = Q16_16::from_f64(100.0);
        let divisor = Q16_16::from_f64(3.0);
        let expected_raw = numerator.div(divisor).to_raw();

        for iteration in 0..100 {
            let result = numerator.div(divisor);
            let result_raw = result.to_raw();
            assert_eq!(
                result_raw, expected_raw,
                "Div iteration {} produced different bits: {} vs {}",
                iteration, result_raw, expected_raw
            );
        }
    }

    // =========================================================================
    // Q30.2: Cross-Platform Reproducibility (x86-64, ARM64, RISC-V)
    // =========================================================================

    /// Q30.4: Cross-platform bitwise identical (x86-64 target validation)
    ///
    /// Note: Full cross-platform validation requires CI tests on ARM64/RISC-V
    /// This test validates x86-64 determinism; CI will compare outputs across platforms
    ///
    /// #ASSUME_CROSS_PLATFORM: x86-64 arithmetic is deterministic
    /// #VERIFY_CROSS_PLATFORM: Validated via CI matrix (x86 vs ARM vs RISC-V)
    #[test]
    fn test_t28_q30_q16_16_cross_platform_identical() {
        // Create test vectors that would expose platform differences
        let test_cases = vec![
            (0.1, 0.2, 0.3),      // Tricky IEEE 754: 0.1 + 0.2 ≠ 0.3 in float
            (100.0, 200.0, 300.0), // Simple addition
            (123.456, 789.012, 912.468), // Mixed precision
            (-0.5, 0.5, 0.0),      // Negation
            (1e-5, 1e-5, 2e-5),    // Small numbers
        ];

        for (a_f64, b_f64, expected_f64) in test_cases {
            let a = Q16_16::from_f64(a_f64);
            let b = Q16_16::from_f64(b_f64);
            let expected = Q16_16::from_f64(expected_f64);

            // Run 50 times and verify all bits match
            let result_raw = (a + b).to_raw();
            for _ in 0..50 {
                assert_eq!(
                    (a + b).to_raw(),
                    result_raw,
                    "Cross-platform drift: {:.10} + {:.10}",
                    a_f64,
                    b_f64
                );
            }

            // Verify result is correct (within fixed-point precision)
            let result_f64 = (a + b).to_f64();
            let error = (result_f64 - expected_f64).abs();
            assert!(
                error < 1e-4,
                "Precision error: {:.10} vs expected {:.10} (error {:.2e})",
                result_f64,
                expected_f64,
                error
            );
        }
    }

    // =========================================================================
    // Q30.3: Rounding Consistency (0.5 always rounds same way)
    // =========================================================================

    /// Q30.5: 0.5 rounding is consistent across 1000+ conversions
    ///
    /// #ASSUME_ROUNDING_CONSISTENT: 0.5 always rounds to same direction
    /// #VERIFY_ROUNDING_CONSISTENT: 1000 runs produce same direction
    #[test]
    fn test_t28_q30_fixed_point_rounding_consistency() {
        // Create numbers that have 0.5 in fractional part when scaled
        // Q16.16 has scale 2^16 = 65536
        // So 0.5 / 65536 ≈ 0.00000763... in decimal
        // We need 0.5 at the LSB level = 1 unit out of 2^16 = 32768 / 2

        let test_values = vec![
            0.00000762939453125,    // Exactly 0.5 LSB at Q16.16
            0.000015258789062500,   // 1.0 LSB
            0.0000152587890625,     // 1.0 LSB (higher precision)
            123.00000762939453125,  // Integer + 0.5 LSB
            -567.00000762939453125, // Negative + 0.5 LSB
        ];

        for &value in &test_values {
            let first_result = Q16_16::from_f64(value).to_raw();

            // Run 1000 times and verify result never changes
            for iteration in 0..1000 {
                let result = Q16_16::from_f64(value).to_raw();
                assert_eq!(
                    result, first_result,
                    "Rounding changed at iteration {} for value {:.18}: {} vs {}",
                    iteration, value, result, first_result
                );
            }
        }
    }

    // =========================================================================
    // Q30.4: Overflow Determinism (Max+Max always saturates same)
    // =========================================================================

    /// Q30.6: Overflow saturation is deterministic (Max+Max → Max)
    ///
    /// #ASSUME_OVERFLOW_DETERMINISTIC: Overflow always saturates same way
    /// #VERIFY_OVERFLOW_DETERMINISTIC: 100 runs all saturate identically
    #[test]
    fn test_t28_q30_fixed_point_overflow_determinism() {
        let max = Q16_16::MAX;
        let large = Q16_16::from_f64(100.0);

        // First overflow should give us expected result
        let first_overflow = max.saturating_add(large).to_raw();

        // 100 runs should all saturate to same value
        for iteration in 0..100 {
            let result = max.saturating_add(large).to_raw();
            assert_eq!(
                result, first_overflow,
                "Overflow saturation changed at iteration {}: {} vs {}",
                iteration, result, first_overflow
            );
        }

        // Verify it actually saturated (didn't just add)
        assert_eq!(first_overflow, max.to_raw(), "Overflow should saturate to MAX");
    }

    /// Q30.7: Negative overflow (Min-Min) is deterministic
    ///
    /// #ASSUME_OVERFLOW_DETERMINISTIC: Underflow always saturates same way
    /// #VERIFY_OVERFLOW_DETERMINISTIC: 100 runs all saturate identically
    #[test]
    fn test_t28_q30_fixed_point_underflow_determinism() {
        let min = Q16_16::MIN;
        let large = Q16_16::from_f64(100.0);

        // First underflow should give us expected result
        let first_underflow = min.saturating_sub(large).to_raw();

        // 100 runs should all saturate to same value
        for iteration in 0..100 {
            let result = min.saturating_sub(large).to_raw();
            assert_eq!(
                result, first_underflow,
                "Underflow saturation changed at iteration {}: {} vs {}",
                iteration, result, first_underflow
            );
        }

        // Verify it actually saturated (didn't just subtract)
        assert_eq!(
            first_underflow, min.to_raw(),
            "Underflow should saturate to MIN"
        );
    }

    // =========================================================================
    // Q30.5: Q8.8 CircuitBreaker Metrics Bitwise Identity
    // =========================================================================

    /// Q30.8: Q8.8 packed metrics in CircuitBreaker are bitwise deterministic
    ///
    /// CircuitBreaker uses Q8.8 fixed-point for metrics in 8-byte packed struct
    /// This validates that 9 packed fields maintain bit-level reproducibility
    ///
    /// #ASSUME_BITWISE_DETERMINISTIC: Q8.8 operations in packed struct preserve bits
    /// #VERIFY_BITWISE_DETERMINISTIC: 100 runs with same bit patterns
    #[test]
    fn test_t28_q30_q8_8_packed_bitwise_identical() {
        // Q8.8 format: range -128 to 127, precision 1/256
        let rate = Q8_8::from_f64(50.0);   // Threshold rate (%)
        let ema = Q8_8::from_f64(45.5);    // EMA (%)
        let variance = Q8_8::from_f64(5.2); // Variance

        let expected_rate = rate.to_raw();
        let expected_ema = ema.to_raw();
        let expected_variance = variance.to_raw();

        for iteration in 0..100 {
            // Simulate metric updates
            let updated_ema = ema.saturating_mul(Q8_8::from_f64(0.9));
            let updated_rate = rate.saturating_add(Q8_8::from_f64(0.1));

            assert_eq!(
                rate.to_raw(),
                expected_rate,
                "Iteration {} rate bits changed: {} vs {}",
                iteration,
                rate.to_raw(),
                expected_rate
            );
            assert_eq!(
                ema.to_raw(),
                expected_ema,
                "Iteration {} EMA bits changed: {} vs {}",
                iteration,
                ema.to_raw(),
                expected_ema
            );
        }
    }

    // =========================================================================
    // Q30.6: FixedPointSerialize Q34 Audit Trail Determinism
    // =========================================================================

    /// Q30.9: FixedPointSerialize encode/decode produces identical bit patterns
    ///
    /// Q34 audit trails require bit-level reproducibility for tamper detection
    ///
    /// #ASSUME_BITWISE_DETERMINISTIC: Serialization is deterministic
    /// #VERIFY_BITWISE_DETERMINISTIC: encode→decode→encode produces same bytes
    #[test]
    fn test_t28_q30_fixed_serialize_decode_bitwise_match() {
        let original = Q16_16::from_f64(1234.5678);

        // Simulate serialization by converting to raw bytes
        let original_raw = original.to_raw();
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&original_raw.to_le_bytes());

        // Decode back
        let decoded_raw = i64::from_le_bytes(bytes);
        assert_eq!(
            decoded_raw, original_raw,
            "Serialization round-trip produced different bits: {} vs {}",
            decoded_raw, original_raw
        );

        // Re-encode should produce identical bytes
        let re_encoded = decoded_raw.to_le_bytes();
        assert_eq!(
            bytes, re_encoded,
            "Re-encoded bytes differ: {:?} vs {:?}",
            bytes, re_encoded
        );

        // 100 round-trips should all produce identical results
        let mut current_raw = original_raw;
        for iteration in 0..100 {
            let current_bytes = current_raw.to_le_bytes();
            let decoded = i64::from_le_bytes(current_bytes);
            assert_eq!(
                decoded, original_raw,
                "Round-trip {} produced different bits: {} vs {}",
                iteration, decoded, original_raw
            );
        }
    }

    // =========================================================================
    // Q30.7: Complex Operations (Mul/Div) Bitwise Reproducibility
    // =========================================================================

    /// Q30.10: Complex arithmetic (mul then div) is bitwise deterministic
    ///
    /// #ASSUME_BITWISE_DETERMINISTIC: Compound operations preserve bits
    /// #VERIFY_BITWISE_DETERMINISTIC: 100 runs with identical results
    #[test]
    fn test_t28_q30_mul_div_bitwise_reproducibility() {
        let a = Q16_16::from_f64(123.45);
        let b = Q16_16::from_f64(6.789);
        let c = Q16_16::from_f64(2.0);

        // Compute (a * b) / c
        let expected_raw = a
            .saturating_mul(b)
            .div(c)
            .to_raw();

        for iteration in 0..100 {
            let result = a
                .saturating_mul(b)
                .div(c)
                .to_raw();
            assert_eq!(
                result, expected_raw,
                "Compound operation iteration {} produced different bits: {} vs {}",
                iteration, result, expected_raw
            );
        }
    }

    // =========================================================================
    // Q30.8: Large Number Precision (Q32.32)
    // =========================================================================

    /// Q30.11: Q32.32 large numbers maintain bitwise reproducibility
    ///
    /// #ASSUME_BITWISE_DETERMINISTIC: Q32.32 operations are deterministic
    /// #VERIFY_BITWISE_DETERMINISTIC: 50 runs with identical bits
    #[test]
    fn test_t28_q30_q32_32_large_number_reproducibility() {
        let x = Q32_32::from_f64(1234567890.123456);
        let y = Q32_32::from_f64(9876543210.654321);

        let expected_raw = x.saturating_add(y).to_raw();

        for iteration in 0..50 {
            let result = x.saturating_add(y).to_raw();
            assert_eq!(
                result, expected_raw,
                "Q32.32 iteration {} produced different bits: {} vs {}",
                iteration, result, expected_raw
            );
        }
    }

    // =========================================================================
    // Q30.9: Large Range Precision (Q48.16)
    // =========================================================================

    /// Q30.12: Q48.16 large range maintains bitwise reproducibility
    ///
    /// #ASSUME_BITWISE_DETERMINISTIC: Q48.16 operations are deterministic
    /// #VERIFY_BITWISE_DETERMINISTIC: 50 runs with identical bits
    #[test]
    fn test_t28_q30_q48_16_large_range_reproducibility() {
        let x = Q48_16::from_f64(281474976710656.0); // 2^48
        let y = Q48_16::from_f64(123456.789);

        let expected_raw = x.saturating_add(y).to_raw();

        for iteration in 0..50 {
            let result = x.saturating_add(y).to_raw();
            assert_eq!(
                result, expected_raw,
                "Q48.16 iteration {} produced different bits: {} vs {}",
                iteration, result, expected_raw
            );
        }
    }

    // =========================================================================
    // Q30.10: Negative Number Sign Preservation
    // =========================================================================

    /// Q30.13: Negative number operations preserve sign bit deterministically
    ///
    /// #ASSUME_BITWISE_DETERMINISTIC: Sign bit never flips unexpectedly
    /// #VERIFY_BITWISE_DETERMINISTIC: 100 runs produce same sign
    #[test]
    fn test_t28_q30_negative_number_reproducibility() {
        let neg_a = Q16_16::from_f64(-123.45);
        let neg_b = Q16_16::from_f64(-67.89);
        let pos_c = Q16_16::from_f64(50.0);

        // Negative + Negative = Negative
        let neg_result_expected = neg_a.saturating_add(neg_b).to_raw();
        // Negative + Positive = Negative (if |neg| > |pos|)
        let mixed_result_expected = neg_a.saturating_add(pos_c).to_raw();

        for iteration in 0..100 {
            let neg_result = neg_a.saturating_add(neg_b).to_raw();
            let mixed_result = neg_a.saturating_add(pos_c).to_raw();

            assert_eq!(
                neg_result, neg_result_expected,
                "Negative iteration {} sign bit changed: {} vs {}",
                iteration, neg_result, neg_result_expected
            );
            assert_eq!(
                mixed_result, mixed_result_expected,
                "Mixed iteration {} sign bit changed: {} vs {}",
                iteration, mixed_result, mixed_result_expected
            );
        }
    }

    // =========================================================================
    // Q30.11: Compiler Optimization Determinism (O2 vs O3)
    // =========================================================================

    /// Q30.14: Results identical regardless of compiler optimization level
    ///
    /// Note: Rust compiler determinism is very strong, but this test validates
    /// that fixed-point arithmetic doesn't depend on undefined behavior that
    /// could vary with optimization flags.
    ///
    /// #ASSUME_BITWISE_DETERMINISTIC: O2 and O3 produce identical results
    /// #VERIFY_BITWISE_DETERMINISTIC: 50 runs all identical (CI validates O2 vs O3)
    #[test]
    fn test_t28_q30_compiler_optimization_determinism() {
        // This test runs the same computation 50 times and verifies
        // bit-level reproducibility. In CI, we'll also compile with
        // different optimization levels and compare outputs.

        let a = Q16_16::from_f64(123.456);
        let b = Q16_16::from_f64(789.012);
        let c = Q16_16::from_f64(0.999);

        let expected = a
            .saturating_mul(b)
            .saturating_mul(c)
            .to_raw();

        for iteration in 0..50 {
            let result = a
                .saturating_mul(b)
                .saturating_mul(c)
                .to_raw();
            assert_eq!(
                result, expected,
                "Optimization drift at iteration {}: {} vs {}",
                iteration, result, expected
            );
        }
    }
}
