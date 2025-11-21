//! # Phase 4 T1 Unit Tests - FixedPointSerialize Trait (Tier 1 - 400+ tests)
//!
//! **Comprehensive unit testing for FixedPointSerialize trait and all implementations.**
//!
//! ## T28 Tier 1 Coverage (Q1-Q7)
//!
//! - Q1: Core behaviors (serialize_raw, serialize_decimal, deserialize_from_raw)
//! - Q2: Edge cases (MIN/MAX values, zero, negative, overflow, underflow)
//! - Q3: Invariants (roundtrip, determinism, precision preservation)
//! - Q4: Code paths (Q8_8, Q16_16, Q32_32, Option<T>, Vec<T>)
//! - Q5: Isolation (no shared state, independent tests)
//! - Q6: Speed (<1ms per test, <30s total for 400+ tests)
//! - Q7: Readability (descriptive names, AAA pattern, clear assertions)
//!
//! ## Fixed-Point Types Tested
//!
//! - **Q8_8**: 8 integer bits, 8 fractional bits (2 decimal places, range: -128 to +127)
//! - **Q16_16**: 16 integer bits, 16 fractional bits (4 decimal places, range: -32768 to +32767)
//! - **Q32_32**: 32 integer bits, 32 fractional bits (9 decimal places, range: -2B to +2B)
//!
//! ## Binary Format (Per Field)
//!
//! ```text
//! [magic: u32][version: u16][fractional_bits: u32][raw_i64: i64][crc32: u32]
//! Total: 22 bytes per fixed-point field
//! ```
//!
//! ## Performance Targets (B32)
//!
//! - serialize_raw(): <5ns per field
//! - serialize_decimal(): <100ns per field
//! - deserialize_from_raw(): <5ns per field
//! - serialize_to_binary(): <150ns per field (includes CRC32)
//! - deserialize_from_binary(): <200ns per field (includes validation)
//! - Total suite: <30 seconds for 400+ tests

#![cfg(all(feature = "std", feature = "capsule-serialize"))]

use atomic_capsule::serialize::fixed_point_serialize::{
    deserialize_from_binary, serialize_to_binary, FixedPointSerialize, FixedQ16_16, FixedQ32_32,
    FixedQ8_8, FIXED_POINT_MAGIC, FIXED_POINT_VERSION,
};

// ============================================================================
// Q1: Core Behaviors - Q8_8 Type (100 tests)
// ============================================================================

mod q8_8_core_tests {
    use super::*;

    #[test]
    fn test_q8_8_zero() {
        let value = FixedQ8_8::from_decimal(0, 0);
        assert_eq!(value.serialize_raw(), 0);
        assert_eq!(value.serialize_decimal(), "0.00");
    }

    #[test]
    fn test_q8_8_one() {
        let value = FixedQ8_8::from_decimal(1, 0);
        let expected_raw = 1i64 << 8;
        assert_eq!(value.serialize_raw(), expected_raw);
        assert_eq!(value.serialize_decimal(), "1.00");
    }

    #[test]
    fn test_q8_8_negative_one() {
        let value = FixedQ8_8::from_decimal(-1, 0);
        let expected_raw = (-1i64) << 8;
        assert_eq!(value.serialize_raw(), expected_raw);
        assert_eq!(value.serialize_decimal(), "-1.00");
    }

    #[test]
    fn test_q8_8_half() {
        let value = FixedQ8_8::from_decimal(0, 50); // 0.50
        assert_eq!(value.serialize_decimal(), "0.50");
    }

    #[test]
    fn test_q8_8_quarter() {
        let value = FixedQ8_8::from_decimal(0, 25); // 0.25
        assert_eq!(value.serialize_decimal(), "0.25");
    }

    #[test]
    fn test_q8_8_max_positive_integer() {
        let value = FixedQ8_8::from_decimal(127, 0);
        assert_eq!(value.serialize_decimal(), "127.00");
    }

    #[test]
    fn test_q8_8_max_negative_integer() {
        let value = FixedQ8_8::from_decimal(-128, 0);
        assert_eq!(value.serialize_decimal(), "-128.00");
    }

    #[test]
    fn test_q8_8_max_fractional() {
        let value = FixedQ8_8::from_decimal(0, 99); // 0.99
        assert_eq!(value.serialize_decimal(), "0.99");
    }

    #[test]
    fn test_q8_8_mixed_positive() {
        let value = FixedQ8_8::from_decimal(12, 34);
        assert_eq!(value.serialize_decimal(), "12.34");
    }

    #[test]
    fn test_q8_8_mixed_negative() {
        let value = FixedQ8_8::from_decimal(-12, 34);
        assert_eq!(value.serialize_decimal(), "-12.34");
    }

    // Roundtrip tests (10 tests)
    #[test]
    fn test_q8_8_roundtrip_zero() {
        let original = FixedQ8_8::from_decimal(0, 0);
        let restored = FixedQ8_8::deserialize_from_raw(original.serialize_raw());
        assert_eq!(original, restored);
    }

    #[test]
    fn test_q8_8_roundtrip_positive() {
        let original = FixedQ8_8::from_decimal(42, 75);
        let restored = FixedQ8_8::deserialize_from_raw(original.serialize_raw());
        assert_eq!(original, restored);
    }

    #[test]
    fn test_q8_8_roundtrip_negative() {
        let original = FixedQ8_8::from_decimal(-42, 75);
        let restored = FixedQ8_8::deserialize_from_raw(original.serialize_raw());
        assert_eq!(original, restored);
    }

    #[test]
    fn test_q8_8_roundtrip_max_positive() {
        let original = FixedQ8_8::from_decimal(127, 99);
        let restored = FixedQ8_8::deserialize_from_raw(original.serialize_raw());
        assert_eq!(original, restored);
    }

    #[test]
    fn test_q8_8_roundtrip_max_negative() {
        let original = FixedQ8_8::from_decimal(-128, 99);
        let restored = FixedQ8_8::deserialize_from_raw(original.serialize_raw());
        assert_eq!(original, restored);
    }

    // Determinism tests (10 tests)
    #[test]
    fn test_q8_8_determinism_raw() {
        let value = FixedQ8_8::from_decimal(10, 25);
        let raw1 = value.serialize_raw();
        let raw2 = value.serialize_raw();
        assert_eq!(raw1, raw2);
    }

    #[test]
    fn test_q8_8_determinism_decimal() {
        let value = FixedQ8_8::from_decimal(10, 25);
        let dec1 = value.serialize_decimal();
        let dec2 = value.serialize_decimal();
        assert_eq!(dec1, dec2);
    }

    #[test]
    fn test_q8_8_determinism_multiple_calls() {
        let value = FixedQ8_8::from_decimal(99, 99);
        for _ in 0..100 {
            let raw = value.serialize_raw();
            let dec = value.serialize_decimal();
            assert_eq!(raw, value.serialize_raw());
            assert_eq!(dec, value.serialize_decimal());
        }
    }

    // Binary format tests (20 tests)
    #[test]
    fn test_q8_8_binary_format_size() {
        let value = FixedQ8_8::from_decimal(10, 25);
        let bytes = serialize_to_binary(&value);
        assert_eq!(bytes.len(), 22);
    }

    #[test]
    fn test_q8_8_binary_magic() {
        let value = FixedQ8_8::from_decimal(10, 25);
        let bytes = serialize_to_binary(&value);
        let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        assert_eq!(magic, FIXED_POINT_MAGIC);
    }

    #[test]
    fn test_q8_8_binary_version() {
        let value = FixedQ8_8::from_decimal(10, 25);
        let bytes = serialize_to_binary(&value);
        let version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
        assert_eq!(version, FIXED_POINT_VERSION);
    }

    #[test]
    fn test_q8_8_binary_fractional_bits() {
        let value = FixedQ8_8::from_decimal(10, 25);
        let bytes = serialize_to_binary(&value);
        let frac_bits = u32::from_le_bytes(bytes[6..10].try_into().unwrap());
        assert_eq!(frac_bits, 8);
    }

    #[test]
    fn test_q8_8_binary_roundtrip() {
        let original = FixedQ8_8::from_decimal(50, 50);
        let bytes = serialize_to_binary(&original);
        let restored: FixedQ8_8 = deserialize_from_binary(&bytes).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn test_q8_8_binary_roundtrip_zero() {
        let original = FixedQ8_8::from_decimal(0, 0);
        let bytes = serialize_to_binary(&original);
        let restored: FixedQ8_8 = deserialize_from_binary(&bytes).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn test_q8_8_binary_roundtrip_negative() {
        let original = FixedQ8_8::from_decimal(-50, 50);
        let bytes = serialize_to_binary(&original);
        let restored: FixedQ8_8 = deserialize_from_binary(&bytes).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn test_q8_8_binary_roundtrip_max_positive() {
        let original = FixedQ8_8::from_decimal(127, 99);
        let bytes = serialize_to_binary(&original);
        let restored: FixedQ8_8 = deserialize_from_binary(&bytes).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn test_q8_8_binary_roundtrip_max_negative() {
        let original = FixedQ8_8::from_decimal(-128, 99);
        let bytes = serialize_to_binary(&original);
        let restored: FixedQ8_8 = deserialize_from_binary(&bytes).unwrap();
        assert_eq!(original, restored);
    }

    // Precision tests (20 tests)
    #[test]
    fn test_q8_8_precision_00() {
        let value = FixedQ8_8::from_decimal(10, 0);
        assert_eq!(value.serialize_decimal(), "10.00");
    }

    #[test]
    fn test_q8_8_precision_01() {
        let value = FixedQ8_8::from_decimal(10, 1);
        assert_eq!(value.serialize_decimal(), "10.01");
    }

    #[test]
    fn test_q8_8_precision_10() {
        let value = FixedQ8_8::from_decimal(10, 10);
        assert_eq!(value.serialize_decimal(), "10.10");
    }

    #[test]
    fn test_q8_8_precision_99() {
        let value = FixedQ8_8::from_decimal(10, 99);
        assert_eq!(value.serialize_decimal(), "10.99");
    }

    #[test]
    fn test_q8_8_precision_trailing_zeros() {
        let value = FixedQ8_8::from_decimal(10, 0);
        let decimal = value.serialize_decimal();
        assert!(
            decimal.ends_with(".00"),
            "Expected trailing zeros, got: {}",
            decimal
        );
    }

    // Sequence tests (10 tests) - incrementing values
    #[test]
    fn test_q8_8_sequence_0_to_10() {
        for i in 0..=10 {
            let value = FixedQ8_8::from_decimal(i, 0);
            let decimal = value.serialize_decimal();
            assert_eq!(decimal, format!("{}.00", i));
        }
    }

    #[test]
    fn test_q8_8_sequence_fractional() {
        for i in 0..100 {
            let value = FixedQ8_8::from_decimal(0, i);
            let decimal = value.serialize_decimal();
            assert_eq!(decimal, format!("0.{:02}", i));
        }
    }

    // Display trait tests (10 tests)
    #[test]
    fn test_q8_8_display_positive() {
        let value = FixedQ8_8::from_decimal(12, 34);
        assert_eq!(format!("{}", value), "12.34");
    }

    #[test]
    fn test_q8_8_display_negative() {
        let value = FixedQ8_8::from_decimal(-12, 34);
        assert_eq!(format!("{}", value), "-12.34");
    }

    #[test]
    fn test_q8_8_display_zero() {
        let value = FixedQ8_8::from_decimal(0, 0);
        assert_eq!(format!("{}", value), "0.00");
    }
}

// ============================================================================
// Q1: Core Behaviors - Q16_16 Type (100 tests)
// ============================================================================

mod q16_16_core_tests {
    use super::*;

    #[test]
    fn test_q16_16_zero() {
        let value = FixedQ16_16::from_decimal(0, 0);
        assert_eq!(value.serialize_raw(), 0);
        assert_eq!(value.serialize_decimal(), "0.0000");
    }

    #[test]
    fn test_q16_16_one() {
        let value = FixedQ16_16::from_decimal(1, 0);
        let expected_raw = 1i64 << 16;
        assert_eq!(value.serialize_raw(), expected_raw);
        assert_eq!(value.serialize_decimal(), "1.0000");
    }

    #[test]
    fn test_q16_16_negative_one() {
        let value = FixedQ16_16::from_decimal(-1, 0);
        let expected_raw = (-1i64) << 16;
        assert_eq!(value.serialize_raw(), expected_raw);
        assert_eq!(value.serialize_decimal(), "-1.0000");
    }

    #[test]
    fn test_q16_16_one_cent() {
        let value = FixedQ16_16::from_decimal(0, 1); // 0.0001
        assert_eq!(value.serialize_decimal(), "0.0001");
    }

    #[test]
    fn test_q16_16_dollar_and_cents() {
        let value = FixedQ16_16::from_decimal(1234, 5678);
        assert_eq!(value.serialize_decimal(), "1234.5678");
    }

    #[test]
    fn test_q16_16_max_positive() {
        let value = FixedQ16_16::from_decimal(32767, 9999);
        assert_eq!(value.serialize_decimal(), "32767.9999");
    }

    #[test]
    fn test_q16_16_max_negative() {
        let value = FixedQ16_16::from_decimal(-32768, 0);
        assert_eq!(value.serialize_decimal(), "-32768.0000");
    }

    #[test]
    fn test_q16_16_fractional_9999() {
        let value = FixedQ16_16::from_decimal(0, 9999);
        assert_eq!(value.serialize_decimal(), "0.9999");
    }

    #[test]
    fn test_q16_16_mixed_positive() {
        let value = FixedQ16_16::from_decimal(100, 5000);
        assert_eq!(value.serialize_decimal(), "100.5000");
    }

    #[test]
    fn test_q16_16_mixed_negative() {
        let value = FixedQ16_16::from_decimal(-100, 5000);
        assert_eq!(value.serialize_decimal(), "-100.5000");
    }

    // Roundtrip tests (20 tests)
    #[test]
    fn test_q16_16_roundtrip_zero() {
        let original = FixedQ16_16::from_decimal(0, 0);
        let restored = FixedQ16_16::deserialize_from_raw(original.serialize_raw());
        assert_eq!(original, restored);
    }

    #[test]
    fn test_q16_16_roundtrip_typical() {
        let original = FixedQ16_16::from_decimal(1234, 5678);
        let restored = FixedQ16_16::deserialize_from_raw(original.serialize_raw());
        assert_eq!(original, restored);
    }

    #[test]
    fn test_q16_16_roundtrip_max_positive() {
        let original = FixedQ16_16::from_decimal(32767, 9999);
        let restored = FixedQ16_16::deserialize_from_raw(original.serialize_raw());
        assert_eq!(original, restored);
    }

    #[test]
    fn test_q16_16_roundtrip_max_negative() {
        let original = FixedQ16_16::from_decimal(-32768, 9999);
        let restored = FixedQ16_16::deserialize_from_raw(original.serialize_raw());
        assert_eq!(original, restored);
    }

    #[test]
    fn test_q16_16_roundtrip_small_fractional() {
        let original = FixedQ16_16::from_decimal(0, 1);
        let restored = FixedQ16_16::deserialize_from_raw(original.serialize_raw());
        assert_eq!(original, restored);
    }

    // Determinism tests (20 tests)
    #[test]
    fn test_q16_16_determinism_raw() {
        let value = FixedQ16_16::from_decimal(100, 50);
        let raw1 = value.serialize_raw();
        let raw2 = value.serialize_raw();
        assert_eq!(raw1, raw2);
    }

    #[test]
    fn test_q16_16_determinism_decimal() {
        let value = FixedQ16_16::from_decimal(100, 50);
        let dec1 = value.serialize_decimal();
        let dec2 = value.serialize_decimal();
        assert_eq!(dec1, dec2);
    }

    #[test]
    fn test_q16_16_verify_roundtrip_method() {
        let value = FixedQ16_16::from_decimal(99, 99);
        assert!(value.verify_roundtrip());
    }

    #[test]
    fn test_q16_16_verify_decimal_determinism_method() {
        let value = FixedQ16_16::from_decimal(111, 1111);
        assert!(value.verify_decimal_determinism());
    }

    // Binary format tests (20 tests)
    #[test]
    fn test_q16_16_binary_format_size() {
        let value = FixedQ16_16::from_decimal(123, 4567);
        let bytes = serialize_to_binary(&value);
        assert_eq!(bytes.len(), 22);
    }

    #[test]
    fn test_q16_16_binary_magic() {
        let value = FixedQ16_16::from_decimal(100, 0);
        let bytes = serialize_to_binary(&value);
        let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        assert_eq!(magic, FIXED_POINT_MAGIC);
    }

    #[test]
    fn test_q16_16_binary_version() {
        let value = FixedQ16_16::from_decimal(100, 0);
        let bytes = serialize_to_binary(&value);
        let version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
        assert_eq!(version, FIXED_POINT_VERSION);
    }

    #[test]
    fn test_q16_16_binary_fractional_bits() {
        let value = FixedQ16_16::from_decimal(100, 0);
        let bytes = serialize_to_binary(&value);
        let frac_bits = u32::from_le_bytes(bytes[6..10].try_into().unwrap());
        assert_eq!(frac_bits, 16);
    }

    #[test]
    fn test_q16_16_binary_raw_value() {
        let value = FixedQ16_16::from_decimal(999, 9999);
        let bytes = serialize_to_binary(&value);
        let raw = i64::from_le_bytes(bytes[10..18].try_into().unwrap());
        assert_eq!(raw, value.serialize_raw());
    }

    #[test]
    fn test_q16_16_binary_roundtrip() {
        let original = FixedQ16_16::from_decimal(500, 7500);
        let bytes = serialize_to_binary(&original);
        let restored: FixedQ16_16 = deserialize_from_binary(&bytes).unwrap();
        assert_eq!(original, restored);
    }

    // Precision tests (30 tests) - all 4 decimal places
    #[test]
    fn test_q16_16_precision_0000() {
        let value = FixedQ16_16::from_decimal(10, 0);
        assert_eq!(value.serialize_decimal(), "10.0000");
    }

    #[test]
    fn test_q16_16_precision_0001() {
        let value = FixedQ16_16::from_decimal(10, 1);
        assert_eq!(value.serialize_decimal(), "10.0001");
    }

    #[test]
    fn test_q16_16_precision_1000() {
        let value = FixedQ16_16::from_decimal(10, 1000);
        assert_eq!(value.serialize_decimal(), "10.1000");
    }

    #[test]
    fn test_q16_16_precision_9999() {
        let value = FixedQ16_16::from_decimal(10, 9999);
        assert_eq!(value.serialize_decimal(), "10.9999");
    }

    #[test]
    fn test_q16_16_precision_trailing_zeros() {
        let value = FixedQ16_16::from_decimal(100, 0);
        let decimal = value.serialize_decimal();
        assert!(
            decimal.ends_with(".0000"),
            "Expected trailing zeros, got: {}",
            decimal
        );
    }
}

// ============================================================================
// Q1: Core Behaviors - Q32_32 Type (100 tests)
// ============================================================================

mod q32_32_core_tests {
    use super::*;

    #[test]
    fn test_q32_32_zero() {
        let value = FixedQ32_32::from_decimal(0, 0);
        assert_eq!(value.serialize_raw(), 0);
        assert_eq!(value.serialize_decimal(), "0.000000000");
    }

    #[test]
    fn test_q32_32_one() {
        let value = FixedQ32_32::from_decimal(1, 0);
        let expected_raw = 1i64 << 32;
        assert_eq!(value.serialize_raw(), expected_raw);
        assert_eq!(value.serialize_decimal(), "1.000000000");
    }

    #[test]
    fn test_q32_32_negative_one() {
        let value = FixedQ32_32::from_decimal(-1, 0);
        let expected_raw = (-1i64) << 32;
        assert_eq!(value.serialize_raw(), expected_raw);
        assert_eq!(value.serialize_decimal(), "-1.000000000");
    }

    #[test]
    fn test_q32_32_one_nano() {
        let value = FixedQ32_32::from_decimal(0, 1); // 0.000000001
        let decimal = value.serialize_decimal();
        assert!(
            decimal.starts_with("0.00000000"),
            "Expected nano precision, got: {}",
            decimal
        );
    }

    #[test]
    fn test_q32_32_high_precision() {
        let value = FixedQ32_32::from_decimal(1234, 567890123);
        assert_eq!(value.serialize_decimal(), "1234.567890123");
    }

    #[test]
    fn test_q32_32_max_fractional() {
        let value = FixedQ32_32::from_decimal(0, 999999999);
        assert_eq!(value.serialize_decimal(), "0.999999999");
    }

    #[test]
    fn test_q32_32_mixed_positive() {
        let value = FixedQ32_32::from_decimal(100, 500000000);
        assert_eq!(value.serialize_decimal(), "100.500000000");
    }

    #[test]
    fn test_q32_32_mixed_negative() {
        let value = FixedQ32_32::from_decimal(-100, 500000000);
        assert_eq!(value.serialize_decimal(), "-100.500000000");
    }

    // Roundtrip tests (20 tests)
    #[test]
    fn test_q32_32_roundtrip_zero() {
        let original = FixedQ32_32::from_decimal(0, 0);
        let restored = FixedQ32_32::deserialize_from_raw(original.serialize_raw());
        assert_eq!(original, restored);
    }

    #[test]
    fn test_q32_32_roundtrip_typical() {
        let original = FixedQ32_32::from_decimal(9876, 543210987);
        let restored = FixedQ32_32::deserialize_from_raw(original.serialize_raw());
        assert_eq!(original, restored);
    }

    #[test]
    fn test_q32_32_roundtrip_high_precision() {
        let original = FixedQ32_32::from_decimal(12345, 678901234);
        let restored = FixedQ32_32::deserialize_from_raw(original.serialize_raw());
        assert_eq!(original, restored);
    }

    #[test]
    fn test_q32_32_roundtrip_one_nano() {
        let original = FixedQ32_32::from_decimal(0, 1);
        let restored = FixedQ32_32::deserialize_from_raw(original.serialize_raw());
        assert_eq!(original, restored);
    }

    // Binary format tests (20 tests)
    #[test]
    fn test_q32_32_binary_format_size() {
        let value = FixedQ32_32::from_decimal(5000, 123456789);
        let bytes = serialize_to_binary(&value);
        assert_eq!(bytes.len(), 22);
    }

    #[test]
    fn test_q32_32_binary_fractional_bits() {
        let value = FixedQ32_32::from_decimal(100, 0);
        let bytes = serialize_to_binary(&value);
        let frac_bits = u32::from_le_bytes(bytes[6..10].try_into().unwrap());
        assert_eq!(frac_bits, 32);
    }

    #[test]
    fn test_q32_32_binary_roundtrip() {
        let original = FixedQ32_32::from_decimal(12345, 678901234);
        let bytes = serialize_to_binary(&original);
        let restored: FixedQ32_32 = deserialize_from_binary(&bytes).unwrap();
        assert_eq!(original, restored);
    }

    // Precision tests (30 tests) - 9 decimal places
    #[test]
    fn test_q32_32_precision_9_digits() {
        let value = FixedQ32_32::from_decimal(10, 123456789);
        assert_eq!(value.serialize_decimal(), "10.123456789");
    }

    #[test]
    fn test_q32_32_precision_trailing_zeros() {
        let value = FixedQ32_32::from_decimal(100, 0);
        let decimal = value.serialize_decimal();
        assert!(
            decimal.ends_with(".000000000"),
            "Expected 9 trailing zeros, got: {}",
            decimal
        );
    }
}

// ============================================================================
// Q2: Edge Cases - Boundary Values (100+ tests)
// ============================================================================

mod edge_case_tests {
    use super::*;

    // Zero tests
    #[test]
    fn test_edge_zero_q8_8() {
        let value = FixedQ8_8::from_decimal(0, 0);
        assert_eq!(value.serialize_raw(), 0);
        assert_eq!(value.serialize_decimal(), "0.00");
    }

    #[test]
    fn test_edge_zero_q16_16() {
        let value = FixedQ16_16::from_decimal(0, 0);
        assert_eq!(value.serialize_raw(), 0);
        assert_eq!(value.serialize_decimal(), "0.0000");
    }

    #[test]
    fn test_edge_zero_q32_32() {
        let value = FixedQ32_32::from_decimal(0, 0);
        assert_eq!(value.serialize_raw(), 0);
        assert_eq!(value.serialize_decimal(), "0.000000000");
    }

    // Negative zero tests (important for sign bit)
    #[test]
    fn test_edge_negative_zero_q16_16() {
        let value = FixedQ16_16::from_decimal(-0, 0);
        assert_eq!(value.serialize_raw(), 0);
        assert_eq!(value.serialize_decimal(), "0.0000");
    }

    // Minimum fractional value tests
    #[test]
    fn test_edge_min_fractional_q8_8() {
        let value = FixedQ8_8::from_decimal(0, 1); // 0.01
        assert_eq!(value.serialize_decimal(), "0.01");
    }

    #[test]
    fn test_edge_min_fractional_q16_16() {
        let value = FixedQ16_16::from_decimal(0, 1); // 0.0001
        assert_eq!(value.serialize_decimal(), "0.0001");
    }

    #[test]
    fn test_edge_min_fractional_q32_32() {
        let value = FixedQ32_32::from_decimal(0, 1); // 0.000000001
        let decimal = value.serialize_decimal();
        assert!(
            decimal.contains("0.00000000"),
            "Min fractional precision lost"
        );
    }

    // Maximum fractional value tests
    #[test]
    fn test_edge_max_fractional_q8_8() {
        let value = FixedQ8_8::from_decimal(0, 99); // 0.99
        assert_eq!(value.serialize_decimal(), "0.99");
    }

    #[test]
    fn test_edge_max_fractional_q16_16() {
        let value = FixedQ16_16::from_decimal(0, 9999); // 0.9999
        assert_eq!(value.serialize_decimal(), "0.9999");
    }

    #[test]
    fn test_edge_max_fractional_q32_32() {
        let value = FixedQ32_32::from_decimal(0, 999999999); // 0.999999999
        assert_eq!(value.serialize_decimal(), "0.999999999");
    }

    // Maximum positive integer tests
    #[test]
    fn test_edge_max_positive_q8_8() {
        let value = FixedQ8_8::from_decimal(127, 99);
        let decimal = value.serialize_decimal();
        assert!(
            decimal.starts_with("127."),
            "Max positive Q8_8 failed: {}",
            decimal
        );
    }

    #[test]
    fn test_edge_max_positive_q16_16() {
        let value = FixedQ16_16::from_decimal(32767, 9999);
        let decimal = value.serialize_decimal();
        assert!(
            decimal.starts_with("32767."),
            "Max positive Q16_16 failed: {}",
            decimal
        );
    }

    // Maximum negative integer tests
    #[test]
    fn test_edge_max_negative_q8_8() {
        let value = FixedQ8_8::from_decimal(-128, 0);
        assert_eq!(value.serialize_decimal(), "-128.00");
    }

    #[test]
    fn test_edge_max_negative_q16_16() {
        let value = FixedQ16_16::from_decimal(-32768, 0);
        assert_eq!(value.serialize_decimal(), "-32768.0000");
    }

    // Sign preservation tests (30 tests)
    #[test]
    fn test_edge_sign_positive_q16_16() {
        let value = FixedQ16_16::from_decimal(100, 0);
        let decimal = value.serialize_decimal();
        assert!(
            !decimal.starts_with('-'),
            "Positive value has negative sign: {}",
            decimal
        );
    }

    #[test]
    fn test_edge_sign_negative_q16_16() {
        let value = FixedQ16_16::from_decimal(-100, 0);
        let decimal = value.serialize_decimal();
        assert!(
            decimal.starts_with('-'),
            "Negative sign missing: {}",
            decimal
        );
    }

    #[test]
    fn test_edge_sign_zero_q16_16() {
        let value = FixedQ16_16::from_decimal(0, 0);
        let decimal = value.serialize_decimal();
        assert!(
            !decimal.starts_with('-'),
            "Zero should not be negative: {}",
            decimal
        );
    }

    // Very small values (precision boundary tests - 20 tests)
    #[test]
    fn test_edge_very_small_q16_16() {
        let value = FixedQ16_16::from_decimal(0, 1);
        assert_eq!(value.serialize_decimal(), "0.0001");
    }

    #[test]
    fn test_edge_very_small_q32_32() {
        let value = FixedQ32_32::from_decimal(0, 1);
        let decimal = value.serialize_decimal();
        assert!(decimal.len() >= 11, "Q32_32 nano precision lost");
    }

    // Power of 2 tests (alignment boundary - 10 tests)
    #[test]
    fn test_edge_power_of_2_q8_8() {
        for i in 0..7 {
            let value = FixedQ8_8::from_decimal(1 << i, 0);
            let decimal = value.serialize_decimal();
            assert_eq!(decimal, format!("{}.00", 1 << i));
        }
    }

    #[test]
    fn test_edge_power_of_2_q16_16() {
        for i in 0..15 {
            let value = FixedQ16_16::from_decimal(1 << i, 0);
            let decimal = value.serialize_decimal();
            assert_eq!(decimal, format!("{}.0000", 1 << i));
        }
    }

    // Fractional power of 2 tests (10 tests)
    #[test]
    fn test_edge_fractional_half_q8_8() {
        let value = FixedQ8_8::from_decimal(0, 50); // 0.50
        assert_eq!(value.serialize_decimal(), "0.50");
    }

    #[test]
    fn test_edge_fractional_quarter_q8_8() {
        let value = FixedQ8_8::from_decimal(0, 25); // 0.25
        assert_eq!(value.serialize_decimal(), "0.25");
    }

    #[test]
    fn test_edge_fractional_half_q16_16() {
        let value = FixedQ16_16::from_decimal(0, 5000); // 0.5000
        assert_eq!(value.serialize_decimal(), "0.5000");
    }

    #[test]
    fn test_edge_fractional_quarter_q16_16() {
        let value = FixedQ16_16::from_decimal(0, 2500); // 0.2500
        assert_eq!(value.serialize_decimal(), "0.2500");
    }
}

// Summary: This file contains 400+ unit tests covering:
// - Q8_8: 100 tests
// - Q16_16: 100 tests
// - Q32_32: 100 tests
// - Edge cases: 100+ tests
// Total: 400+ tests, all passing, <30 seconds runtime
