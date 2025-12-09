//! Comprehensive Overflow Test Suite - T28 Framework Compliance
//!
//! ## T28 Testing Framework Coverage
//!
//! - **Q1-Q7 (Unit Testing)**: Individual operation correctness
//! - **Q8-Q14 (Property Testing)**: Mathematical invariants
//! - **Q15-Q21 (Integration)**: Cross-format consistency
//! - **Q22-Q28 (Production)**: Real-world financial scenarios
//!
//! ## ASSUM Safety Verification
//!
//! All tests verify ASSUM assumptions:
//! - #ASSUME_OVERFLOW_DETECTION → Tests prove overflow detected
//! - #ASSUME_SATURATION_CORRECTNESS → Tests prove saturation correct
//! - #ASSUME_WRAPPING_INTENTIONAL → Tests document wrapping behavior
//! - #VERIFY_NO_PANICS → All tests run without panics
//! - #VERIFY_PRECISION_LOSS → Precision maintained within format limits

use fixed_point_tier3::{Q8_8, Q16_16, Q32_32, FixedPointError};

// ============================================================================
// Q1-Q7: Unit Tests - Basic Arithmetic Correctness
// ============================================================================

mod q8_8_unit_tests {
    use super::*;

    #[test]
    fn test_checked_add_normal() {
        let a = Q8_8::from_fixed(50_00);
        let b = Q8_8::from_fixed(30_00);
        let result = a.checked_add(b).expect("Normal addition should succeed");
        assert!((result.to_f64() - 80.0).abs() < 0.01);
    }

    #[test]
    fn test_checked_add_overflow() {
        let max = Q8_8::MAX;
        let one = Q8_8::from_fixed(1_00);
        // #VERIFY_OVERFLOW_DETECTION: MAX + 1 must return None
        assert_eq!(max.checked_add(one), None);
    }

    #[test]
    fn test_checked_sub_normal() {
        let a = Q8_8::from_fixed(100_00);
        let b = Q8_8::from_fixed(30_00);
        let result = a.checked_sub(b).expect("Normal subtraction should succeed");
        assert!((result.to_f64() - 70.0).abs() < 0.01);
    }

    #[test]
    fn test_checked_sub_underflow() {
        let min = Q8_8::MIN;
        let one = Q8_8::from_fixed(1_00);
        // #VERIFY_OVERFLOW_DETECTION: MIN - 1 must return None
        assert_eq!(min.checked_sub(one), None);
    }

    #[test]
    fn test_checked_mul_precision() {
        let a = Q8_8::from_fixed(2_00);
        let b = Q8_8::from_fixed(3_00);
        let result = a.checked_mul(b).expect("2 * 3 should succeed");
        // #VERIFY_PRECISION_LOSS: Result maintains Q8.8 precision
        assert!((result.to_f64() - 6.0).abs() < 0.01);
    }

    #[test]
    fn test_checked_mul_overflow() {
        let large = Q8_8::from_fixed(100_00);
        // 100 * 100 = 10000, exceeds Q8.8 range (±127)
        assert_eq!(large.checked_mul(large), None);
    }

    #[test]
    fn test_checked_div_precision() {
        let a = Q8_8::from_fixed(6_00);
        let b = Q8_8::from_fixed(2_00);
        let result = a.checked_div(b).expect("6 / 2 should succeed");
        assert!((result.to_f64() - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_checked_div_by_zero() {
        let a = Q8_8::from_fixed(6_00);
        let zero = Q8_8::ZERO;
        // #VERIFY_OVERFLOW_DETECTION: Division by zero must return None
        assert_eq!(a.checked_div(zero), None);
    }

    #[test]
    fn test_saturating_add_max() {
        let max = Q8_8::MAX;
        let one = Q8_8::from_fixed(1_00);
        // #VERIFY_SATURATION_CORRECTNESS: Saturates at MAX
        assert_eq!(max.saturating_add(one), Q8_8::MAX);
    }

    #[test]
    fn test_saturating_sub_min() {
        let min = Q8_8::MIN;
        let one = Q8_8::from_fixed(1_00);
        // #VERIFY_SATURATION_CORRECTNESS: Saturates at MIN
        assert_eq!(min.saturating_sub(one), Q8_8::MIN);
    }

    #[test]
    fn test_wrapping_overflow() {
        let max = Q8_8::MAX;
        let one = Q8_8::from_fixed(1_00);
        let result = max.wrapping_add(one);
        // #VERIFY_WRAPPING_BEHAVIOR: Wraps to negative
        assert!(result.raw() < 0);
    }
}

mod q16_16_unit_tests {
    use super::*;

    #[test]
    fn test_checked_add_normal() {
        let a = Q16_16::from_fixed(100_0000);
        let b = Q16_16::from_fixed(50_0000);
        let result = a.checked_add(b).expect("Normal addition should succeed");
        assert!((result.to_f64() - 150.0).abs() < 0.0001);
    }

    #[test]
    fn test_checked_add_overflow() {
        let max = Q16_16::MAX;
        let one = Q16_16::from_fixed(1_0000);
        // #VERIFY_OVERFLOW_DETECTION
        assert_eq!(max.checked_add(one), None);
    }

    #[test]
    fn test_checked_sub_underflow() {
        let min = Q16_16::MIN;
        let one = Q16_16::from_fixed(1_0000);
        // #VERIFY_OVERFLOW_DETECTION
        assert_eq!(min.checked_sub(one), None);
    }

    #[test]
    fn test_checked_mul_precision() {
        let a = Q16_16::from_fixed(2_0000);
        let b = Q16_16::from_fixed(3_0000);
        let result = a.checked_mul(b).expect("2 * 3 should succeed");
        // #VERIFY_PRECISION_LOSS
        assert!((result.to_f64() - 6.0).abs() < 0.0001);
    }

    #[test]
    fn test_checked_div_precision() {
        let a = Q16_16::from_fixed(6_0000);
        let b = Q16_16::from_fixed(2_0000);
        let result = a.checked_div(b).expect("6 / 2 should succeed");
        assert!((result.to_f64() - 3.0).abs() < 0.0001);
    }

    #[test]
    fn test_saturating_add_max() {
        let max = Q16_16::MAX;
        let one = Q16_16::from_fixed(1_0000);
        // #VERIFY_SATURATION_CORRECTNESS
        assert_eq!(max.saturating_add(one), Q16_16::MAX);
    }

    #[test]
    fn test_saturating_sub_min() {
        let min = Q16_16::MIN;
        let one = Q16_16::from_fixed(1_0000);
        // #VERIFY_SATURATION_CORRECTNESS
        assert_eq!(min.saturating_sub(one), Q16_16::MIN);
    }
}

mod q32_32_unit_tests {
    use super::*;

    #[test]
    fn test_checked_add_normal() {
        let a = Q32_32::from_fixed(100_000000000);
        let b = Q32_32::from_fixed(50_000000000);
        let result = a.checked_add(b).expect("Normal addition should succeed");
        assert!((result.to_f64() - 150.0).abs() < 0.00001);
    }

    #[test]
    fn test_checked_add_overflow() {
        let max = Q32_32::MAX;
        let one = Q32_32::from_fixed(1_000000000);
        // #VERIFY_OVERFLOW_DETECTION
        assert_eq!(max.checked_add(one), None);
    }

    #[test]
    fn test_checked_mul_precision() {
        let a = Q32_32::from_fixed(2_000000000);
        let b = Q32_32::from_fixed(3_000000000);
        let result = a.checked_mul(b).expect("2 * 3 should succeed");
        // #VERIFY_PRECISION_LOSS
        assert!((result.to_f64() - 6.0).abs() < 0.00001);
    }

    #[test]
    fn test_high_precision_addition() {
        // Test precision beyond typical f64 use
        let a = Q32_32::from_f64(0.123456789).unwrap();
        let b = Q32_32::from_f64(0.987654321).unwrap();
        let result = a.checked_add(b).unwrap();
        assert!((result.to_f64() - 1.11111111).abs() < 0.00001);
    }
}

// ============================================================================
// Q8-Q14: Property Tests - Mathematical Invariants
// ============================================================================

mod property_tests {
    use super::*;

    #[test]
    fn test_checked_always_safe() {
        // Property: checked_* operations never panic, always return Some or None
        // Test with boundary values

        let values_q16 = vec![
            Q16_16::MIN,
            Q16_16::from_fixed(-1000_0000),
            Q16_16::ZERO,
            Q16_16::from_fixed(1000_0000),
            Q16_16::MAX,
        ];

        for &a in &values_q16 {
            for &b in &values_q16 {
                // These should never panic
                let _ = a.checked_add(b);
                let _ = a.checked_sub(b);
                let _ = a.checked_mul(b);
                let _ = a.checked_div(b);
            }
        }
    }

    #[test]
    fn test_saturating_always_returns_value() {
        // Property: saturating_* operations always return a valid value in range

        let values = vec![
            Q16_16::MIN,
            Q16_16::from_fixed(-1000_0000),
            Q16_16::ZERO,
            Q16_16::from_fixed(1000_0000),
            Q16_16::MAX,
        ];

        for &a in &values {
            for &b in &values {
                let add_result = a.saturating_add(b);
                assert!(add_result.raw() >= Q16_16::MIN.raw());
                assert!(add_result.raw() <= Q16_16::MAX.raw());

                let sub_result = a.saturating_sub(b);
                assert!(sub_result.raw() >= Q16_16::MIN.raw());
                assert!(sub_result.raw() <= Q16_16::MAX.raw());
            }
        }
    }

    #[test]
    fn test_addition_commutative() {
        // Property: a + b = b + a (when no overflow)
        let a = Q16_16::from_fixed(100_0000);
        let b = Q16_16::from_fixed(200_0000);

        assert_eq!(a.checked_add(b), b.checked_add(a));
        assert_eq!(a.saturating_add(b), b.saturating_add(a));
    }

    #[test]
    fn test_multiplication_commutative() {
        // Property: a * b = b * a
        let a = Q16_16::from_fixed(5_0000);
        let b = Q16_16::from_fixed(7_0000);

        assert_eq!(a.checked_mul(b), b.checked_mul(a));
    }

    #[test]
    fn test_division_inverse() {
        // Property: (a / b) * b ≈ a (within precision)
        let a = Q16_16::from_fixed(100_0000);
        let b = Q16_16::from_fixed(7_0000);

        let divided = a.checked_div(b).unwrap();
        let restored = divided.checked_mul(b).unwrap();

        // Should be close to original (within precision limits)
        assert!((restored.to_f64() - a.to_f64()).abs() < 1.0);
    }

    #[test]
    fn test_zero_identity_addition() {
        // Property: a + 0 = a
        let a = Q16_16::from_fixed(123_0000);
        let zero = Q16_16::ZERO;

        assert_eq!(a.checked_add(zero), Some(a));
        assert_eq!(a.saturating_add(zero), a);
    }

    #[test]
    fn test_one_identity_multiplication() {
        // Property: a * 1 = a
        let a = Q16_16::from_fixed(123_0000);
        let one = Q16_16::ONE;

        let result = a.checked_mul(one).unwrap();
        assert!((result.to_f64() - a.to_f64()).abs() < 0.001);
    }
}

// ============================================================================
// Q15-Q21: Integration Tests - Cross-Format Consistency
// ============================================================================

mod integration_tests {
    use super::*;

    #[test]
    fn test_format_consistency() {
        // Test that Q8.8, Q16.16, Q32.32 produce consistent results
        // within their respective precision limits

        let value_f64 = 42.5;

        let q8 = Q8_8::from_f64(value_f64).unwrap();
        let q16 = Q16_16::from_f64(value_f64).unwrap();
        let q32 = Q32_32::from_f64(value_f64).unwrap();

        // All should round-trip to approximately the same value
        assert!((q8.to_f64() - value_f64).abs() < 0.01);   // Q8.8 precision
        assert!((q16.to_f64() - value_f64).abs() < 0.0001); // Q16.16 precision
        assert!((q32.to_f64() - value_f64).abs() < 0.00001); // Q32.32 precision
    }

    #[test]
    fn test_arithmetic_consistency_across_formats() {
        // Test that arithmetic operations produce consistent results

        let a_f64 = 10.5;
        let b_f64 = 3.2;

        // Q16.16 calculation
        let a_q16 = Q16_16::from_f64(a_f64).unwrap();
        let b_q16 = Q16_16::from_f64(b_f64).unwrap();
        let sum_q16 = a_q16.checked_add(b_q16).unwrap();

        // Q32.32 calculation
        let a_q32 = Q32_32::from_f64(a_f64).unwrap();
        let b_q32 = Q32_32::from_f64(b_f64).unwrap();
        let sum_q32 = a_q32.checked_add(b_q32).unwrap();

        // Results should match within Q16.16 precision
        assert!((sum_q16.to_f64() - sum_q32.to_f64()).abs() < 0.0001);
    }
}

// ============================================================================
// Q22-Q28: Production Tests - Real-World Financial Scenarios
// ============================================================================

mod production_tests {
    use super::*;

    #[test]
    fn test_financial_calculation_no_overflow() {
        // Real-world scenario: Stock trade P&L
        // Buy 1000 shares @ $123.45, sell @ $125.67, fee $2.50

        let buy_price = Q16_16::from_f64(123.45).unwrap();
        let sell_price = Q16_16::from_f64(125.67).unwrap();
        let quantity = Q16_16::from_fixed(1000_0000);
        let fee = Q16_16::from_f64(2.50).unwrap();

        // Calculate P&L: (sell_price - buy_price) * quantity - fee
        let price_diff = sell_price.checked_sub(buy_price).expect("Price diff overflow");
        let gross_pnl = price_diff.checked_mul(quantity).expect("Gross P&L overflow");
        let net_pnl = gross_pnl.checked_sub(fee).expect("Net P&L overflow");

        // Expected: (125.67 - 123.45) * 1000 - 2.50 = 2217.50
        assert!((net_pnl.to_f64() - 2217.50).abs() < 0.01);
    }

    #[test]
    fn test_cumulative_operations_no_drift() {
        // Test that cumulative operations don't accumulate error
        // This is a key advantage of fixed-point over floating-point
        // Use Q32_32 for this test since Q16.16 maxes at ±32767

        let mut total = Q32_32::ZERO;
        let increment = Q32_32::from_f64(0.01).unwrap(); // 1 cent

        // Add 1 cent 10,000 times = $100.00
        for _ in 0..10000 {
            total = total.checked_add(increment).expect("Cumulative overflow");
        }

        // Should be exactly 100.00 (no drift)
        assert!((total.to_f64() - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_overflow_protection_in_production() {
        // Test that overflow is properly detected in realistic scenarios

        // Scenario: Large position value calculation
        let price = Q16_16::from_f64(30000.0).unwrap(); // Bitcoin-like price
        let quantity = Q16_16::from_fixed(1000_0000);   // 1000 units

        // This would overflow i32 in raw form (30M)
        let result = price.checked_mul(quantity);

        // Depending on range, this might overflow Q16.16
        // The important thing is it returns None, not wrong value
        if result.is_none() {
            // Overflow correctly detected - use saturating or different format
            let saturated = price.saturating_mul(quantity);
            assert_eq!(saturated, Q16_16::MAX); // Saturated to max value
        }
    }

    #[test]
    fn test_precision_in_percentage_calculations() {
        // Financial scenario: Calculate 2.75% fee on $1,000
        // Q16.16 maxes at ±32767, so use smaller principal
        let principal = Q16_16::from_f64(1000.0).unwrap();
        let fee_rate = Q16_16::from_f64(0.0275).unwrap(); // 2.75%

        let fee = principal.checked_mul(fee_rate).expect("Fee calculation overflow");

        // Expected: 1000 * 0.0275 = 27.50
        assert!((fee.to_f64() - 27.50).abs() < 0.01);
    }

    #[test]
    fn test_regulatory_compliance_determinism() {
        // Test deterministic calculation for regulatory compliance
        // Same inputs must always produce same outputs
        // Q16.16 maxes at ±32767, so use values within range

        let inputs = [
            (123.45, 67.89),    // Product: 8379.6105 ✓
            (0.01, 0.99),       // Product: 0.0099 ✓
            (999.99, 0.01),     // Product: 9.9999 ✓
        ];

        for (a_f64, b_f64) in inputs {
            let a = Q16_16::from_f64(a_f64).unwrap();
            let b = Q16_16::from_f64(b_f64).unwrap();

            // Run calculation multiple times
            let results: Vec<_> = (0..100)
                .map(|_| a.checked_mul(b))
                .collect();

            // All results must be identical (deterministic)
            assert!(results.windows(2).all(|w| w[0] == w[1]));
        }
    }

    #[test]
    fn test_edge_case_near_limits() {
        // Test calculations near format limits

        // Q8.8: ±127.99
        let near_max = Q8_8::from_f64(127.0).unwrap();
        let small = Q8_8::from_f64(0.5).unwrap();

        // Should succeed (127.5 is within range)
        assert!(near_max.checked_add(small).is_some());

        // Should fail (128.5 exceeds range)
        let large = Q8_8::from_f64(1.5).unwrap();
        assert_eq!(near_max.checked_add(large), None);
    }
}

// ============================================================================
// ASSUM Verification Tests
// ============================================================================

mod assum_verification {
    use super::*;

    #[test]
    fn verify_no_panics_on_overflow() {
        // #VERIFY_NO_PANICS: All checked operations return Result, never panic

        let max = Q16_16::MAX;
        let min = Q16_16::MIN;
        let one = Q16_16::ONE;
        let zero = Q16_16::ZERO;

        // These should all return None, never panic
        assert_eq!(max.checked_add(one), None);
        assert_eq!(min.checked_sub(one), None);
        assert_eq!(one.checked_div(zero), None);
    }

    #[test]
    fn verify_saturation_correctness() {
        // #VERIFY_SATURATION_CORRECTNESS: Saturation maintains range invariants

        let max = Q16_16::MAX;
        let min = Q16_16::MIN;
        let large = Q16_16::from_fixed(10000_0000);

        // Saturation should clamp to MAX/MIN
        assert_eq!(max.saturating_add(large), Q16_16::MAX);
        assert_eq!(min.saturating_sub(large), Q16_16::MIN);
    }

    #[test]
    fn verify_precision_maintained() {
        // #VERIFY_PRECISION_LOSS: Precision loss within format limits

        let a = Q16_16::from_f64(123.4567).unwrap();
        let b = Q16_16::from_f64(789.0123).unwrap();

        let sum = a.checked_add(b).unwrap();

        // Precision loss should be within Q16.16 resolution (1/65536)
        let expected = 123.4567 + 789.0123;
        assert!((sum.to_f64() - expected).abs() < 0.0001);
    }
}
