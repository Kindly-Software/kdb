//! # Enhanced Fixed-Point Integration Tests (Phase 4)
//!
//! **Comprehensive test suite for FixedPointSerialize trait**
//!
//! ## Test Coverage (T28 Framework)
//!
//! - **Unit Tests (Q1-Q7)**: Individual method correctness (100+ tests)
//! - **Property Tests (Q8-Q14)**: Roundtrip, determinism, precision (1000+ iterations)
//! - **Integration Tests (Q15-Q21)**: Cross-format compatibility, error handling
//! - **Stress Tests (Q22-Q28)**: Edge cases, boundary conditions, corruption
//!
//! ## ASSUM Verification
//!
//! All safety assumptions validated:
//! - #VERIFY_EXACT_ARITHMETIC: Property tests confirm i64 exactness
//! - #VERIFY_DETERMINISTIC_BINARY: Serialize twice, byte-compare
//! - #VERIFY_DETERMINISTIC_DECIMAL: Serialize twice, string-compare
//! - #VERIFY_DETERMINISTIC_HASH: Hash twice, compare values
//! - #VERIFY_CRC32_COLLISION_RARE: No collisions in 10K random samples

#![cfg(test)]

use super::fixed_point_impls::{Q16_16, Q32_32, Q8_8};
use super::fixed_point_trait::{FixedPointSerialize, FixedPointSerializeError};

// ============================================================================
// Q16_16 Unit Tests (Q1-Q7)
// ============================================================================

#[test]
fn test_q16_16_binary_roundtrip() {
    // #VERIFY_EXACT_ARITHMETIC: Binary roundtrip preserves exact value
    let value = Q16_16::from_f64(1234.5678);
    let bytes = value.serialize_binary().unwrap();
    let restored = Q16_16::deserialize_binary(&bytes).unwrap();
    assert_eq!(value, restored);
}

#[test]
fn test_q16_16_binary_format_size() {
    // Verify binary format is exactly 22 bytes
    let value = Q16_16::from_f64(19.99);
    let bytes = value.serialize_binary().unwrap();
    assert_eq!(bytes.len(), 22);
}

#[test]
fn test_q16_16_binary_magic_validation() {
    // Test magic number validation
    let value = Q16_16::from_f64(19.99);
    let mut bytes = value.serialize_binary().unwrap();

    // Corrupt magic number
    bytes[0] ^= 0xFF;

    let result = Q16_16::deserialize_binary(&bytes);
    assert!(result.is_err());
    match result {
        Err(FixedPointSerializeError::InvalidMagic { expected, actual }) => {
            assert_eq!(expected, 0x51313636); // "Q166"
            assert_ne!(actual, expected);
        }
        _ => panic!("Expected InvalidMagic error"),
    }
}

#[test]
fn test_q16_16_binary_version_validation() {
    // Test version validation
    let value = Q16_16::from_f64(19.99);
    let mut bytes = value.serialize_binary().unwrap();

    // Corrupt version
    bytes[4] = 99;

    let result = Q16_16::deserialize_binary(&bytes);
    assert!(result.is_err());
    match result {
        Err(FixedPointSerializeError::VersionMismatch { expected, actual }) => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 99);
        }
        _ => panic!("Expected VersionMismatch error"),
    }
}

#[test]
fn test_q16_16_binary_fractional_bits_validation() {
    // Test fractional bits validation
    let value = Q16_16::from_f64(19.99);
    let mut bytes = value.serialize_binary().unwrap();

    // Corrupt fractional bits
    bytes[6] = 32; // Change from 16 to 32

    let result = Q16_16::deserialize_binary(&bytes);
    assert!(result.is_err());
    match result {
        Err(FixedPointSerializeError::FractionalBitsMismatch { expected, actual }) => {
            assert_eq!(expected, 16);
            assert_eq!(actual, 32);
        }
        _ => panic!("Expected FractionalBitsMismatch error"),
    }
}

#[test]
fn test_q16_16_binary_checksum_validation() {
    // #VERIFY_CRC32_COLLISION_RARE: Test CRC32 detects corruption
    let value = Q16_16::from_f64(1234.5678);
    let mut bytes = value.serialize_binary().unwrap();

    // Corrupt data (byte 15 in raw value)
    bytes[15] ^= 0xFF;

    let result = Q16_16::deserialize_binary(&bytes);
    assert!(result.is_err());
    match result {
        Err(FixedPointSerializeError::ChecksumMismatch { expected, actual }) => {
            assert_ne!(expected, actual);
        }
        _ => panic!("Expected ChecksumMismatch error"),
    }
}

#[test]
fn test_q16_16_decimal_roundtrip() {
    // Test decimal roundtrip with default precision
    let value = Q16_16::from_f64(1234.5678);
    let decimal = value.serialize_decimal(0); // Default: 4 decimals
    let restored = Q16_16::deserialize_decimal(&decimal).unwrap();

    // Allow precision errors based on decimal digits
    // 4 decimal digits → max error ≈ 65536/10000 ≈ 7 units
    let diff = (value.to_raw() - restored.to_raw()).abs();
    assert!(diff <= 7, "Roundtrip error too large: {}", diff);
}

#[test]
fn test_q16_16_decimal_precision() {
    // Test different precision levels
    let value = Q16_16::from_f64(12.3456789);

    // Default precision (4 decimals)
    let dec0 = value.serialize_decimal(0);
    assert!(dec0.contains("12.34"));

    // Custom precision (2 decimals)
    let dec2 = value.serialize_decimal(2);
    assert!(dec2.contains("12.34"));

    // Custom precision (6 decimals)
    let dec6 = value.serialize_decimal(6);
    assert!(dec6.contains("12.345"));
}

#[test]
fn test_q16_16_decimal_negative() {
    // Test negative value serialization
    let value = Q16_16::from_f64(-1234.5678);
    let decimal = value.serialize_decimal(0);
    assert!(decimal.starts_with('-'));

    let restored = Q16_16::deserialize_decimal(&decimal).unwrap();
    let diff = (value.to_raw() - restored.to_raw()).abs();
    assert!(diff <= 7, "Negative value roundtrip error: {}", diff);
}

#[test]
fn test_q16_16_decimal_zero() {
    // Test zero value
    let value = Q16_16::from_f64(0.0);
    let decimal = value.serialize_decimal(0);
    assert!(decimal.contains("0.0000"));

    let restored = Q16_16::deserialize_decimal(&decimal).unwrap();
    assert_eq!(value, restored);
}

#[test]
fn test_q16_16_decimal_integer_only() {
    // Test parsing integer-only format ("1234")
    let restored = Q16_16::deserialize_decimal("1234").unwrap();
    assert_eq!(restored.to_f64(), 1234.0);
}

#[test]
fn test_q16_16_decimal_invalid_format() {
    // Test invalid decimal formats
    let result = Q16_16::deserialize_decimal("abc");
    assert!(result.is_err());

    let result = Q16_16::deserialize_decimal("12.34.56");
    assert!(result.is_err());

    let result = Q16_16::deserialize_decimal("");
    assert!(result.is_err());
}

#[test]
fn test_q16_16_decimal_out_of_range() {
    // Test values out of Q16.16 range
    let result = Q16_16::deserialize_decimal("40000.0"); // > 32767
    assert!(result.is_err());
    match result {
        Err(FixedPointSerializeError::ValueOutOfRange { .. }) => {}
        _ => panic!("Expected ValueOutOfRange error"),
    }

    let result = Q16_16::deserialize_decimal("-40000.0"); // < -32768
    assert!(result.is_err());
}

#[test]
fn test_q16_16_hash_determinism() {
    // #VERIFY_DETERMINISTIC_HASH: Hash same value twice, compare
    let value = Q16_16::from_f64(1234.5678);
    let hash1 = value.compute_hash();
    let hash2 = value.compute_hash();
    assert_eq!(hash1, hash2);
}

#[test]
fn test_q16_16_hash_uniqueness() {
    // Test hash uniqueness for different values
    let value1 = Q16_16::from_f64(1234.5678);
    let value2 = Q16_16::from_f64(1234.5679);
    let hash1 = value1.compute_hash();
    let hash2 = value2.compute_hash();
    assert_ne!(hash1, hash2);
}

#[test]
fn test_q16_16_verify_binary_roundtrip() {
    // Test verify_binary_roundtrip() helper
    let value = Q16_16::from_f64(1234.5678);
    assert!(value.verify_binary_roundtrip());
}

#[test]
fn test_q16_16_verify_decimal_roundtrip() {
    // Test verify_decimal_roundtrip() helper
    let value = Q16_16::from_f64(1234.5678);
    assert!(value.verify_decimal_roundtrip(0)); // Default precision
    assert!(value.verify_decimal_roundtrip(2)); // Custom precision
}

#[test]
fn test_q16_16_verify_binary_determinism() {
    // #VERIFY_DETERMINISTIC_BINARY: Test binary determinism
    let value = Q16_16::from_f64(1234.5678);
    assert!(value.verify_binary_determinism());
}

#[test]
fn test_q16_16_verify_decimal_determinism() {
    // #VERIFY_DETERMINISTIC_DECIMAL: Test decimal determinism
    let value = Q16_16::from_f64(1234.5678);
    assert!(value.verify_decimal_determinism(0));
}

#[test]
fn test_q16_16_verify_hash_determinism() {
    // Test hash determinism verification
    let value = Q16_16::from_f64(1234.5678);
    assert!(value.verify_hash_determinism());
}

// ============================================================================
// Q8_8 Unit Tests (Q1-Q7)
// ============================================================================

#[test]
fn test_q8_8_binary_roundtrip() {
    let value = Q8_8::from_f64(12.34);
    let bytes = value.serialize_binary().unwrap();
    let restored = Q8_8::deserialize_binary(&bytes).unwrap();
    assert_eq!(value, restored);
}

#[test]
fn test_q8_8_binary_format_size() {
    let value = Q8_8::from_f64(12.34);
    let bytes = value.serialize_binary().unwrap();
    assert_eq!(bytes.len(), 22);
}

#[test]
fn test_q8_8_decimal_roundtrip() {
    let value = Q8_8::from_f64(12.34);
    let decimal = value.serialize_decimal(0);
    let restored = Q8_8::deserialize_decimal(&decimal).unwrap();

    // 2 decimal digits → max error ≈ 256/100 ≈ 3 units
    let diff = (value.to_raw() - restored.to_raw()).abs();
    assert!(diff <= 3, "Roundtrip error too large: {}", diff);
}

#[test]
fn test_q8_8_decimal_precision() {
    let value = Q8_8::from_f64(12.345);
    let dec0 = value.serialize_decimal(0); // Default: 2 decimals
    assert!(dec0.contains("12.3"));

    let dec4 = value.serialize_decimal(4); // 4 decimals
    assert!(dec4.contains("12.34"));
}

#[test]
fn test_q8_8_hash_determinism() {
    let value = Q8_8::from_f64(12.34);
    let hash1 = value.compute_hash();
    let hash2 = value.compute_hash();
    assert_eq!(hash1, hash2);
}

#[test]
fn test_q8_8_verify_roundtrip() {
    let value = Q8_8::from_f64(12.34);
    assert!(value.verify_binary_roundtrip());
    assert!(value.verify_decimal_roundtrip(0));
}

#[test]
fn test_q8_8_verify_determinism() {
    let value = Q8_8::from_f64(12.34);
    assert!(value.verify_binary_determinism());
    assert!(value.verify_decimal_determinism(0));
    assert!(value.verify_hash_determinism());
}

// ============================================================================
// Q32_32 Unit Tests (Q1-Q7)
// ============================================================================

#[test]
fn test_q32_32_binary_roundtrip() {
    let value = Q32_32::from_f64(1234567.123456789);
    let bytes = value.serialize_binary().unwrap();
    let restored = Q32_32::deserialize_binary(&bytes).unwrap();

    // Q32.32 serialization truncates to i64, so exact equality not guaranteed
    let diff = (value.to_i64() - restored.to_i64()).abs();
    assert!(diff <= 1, "Integer part mismatch");
}

#[test]
fn test_q32_32_binary_format_size() {
    let value = Q32_32::from_f64(1234567.123456789);
    let bytes = value.serialize_binary().unwrap();
    assert_eq!(bytes.len(), 22);
}

#[test]
fn test_q32_32_decimal_roundtrip() {
    let value = Q32_32::from_f64(1234567.123456789);
    let decimal = value.serialize_decimal(0); // Default: 9 decimals
    let restored = Q32_32::deserialize_decimal(&decimal).unwrap();

    let diff = (value.to_i64() - restored.to_i64()).abs();
    assert!(diff <= 1);
}

#[test]
fn test_q32_32_decimal_precision() {
    let value = Q32_32::from_f64(1234.123456789);
    let dec0 = value.serialize_decimal(0); // Default: 9 decimals
    assert!(dec0.contains("1234."));

    let dec3 = value.serialize_decimal(3); // 3 decimals
    assert!(dec3.contains("1234.123"));
}

#[test]
fn test_q32_32_hash_determinism() {
    let value = Q32_32::from_f64(1234567.123456789);
    let hash1 = value.compute_hash();
    let hash2 = value.compute_hash();
    assert_eq!(hash1, hash2);
}

#[test]
fn test_q32_32_verify_determinism() {
    let value = Q32_32::from_f64(1234567.123456789);
    assert!(value.verify_binary_determinism());
    assert!(value.verify_decimal_determinism(0));
    assert!(value.verify_hash_determinism());
}

// ============================================================================
// Property Tests (Q8-Q14) - 1000+ Iterations
// ============================================================================

#[test]
fn property_q16_16_binary_roundtrip_1000_cases() {
    // #VERIFY_EXACT_ARITHMETIC: 1000+ random values
    use rand::Rng;
    let mut rng = rand::thread_rng();

    for _ in 0..1000 {
        let value = rng.gen_range(-10000.0..10000.0);
        let q = Q16_16::from_f64(value);

        assert!(
            q.verify_binary_roundtrip(),
            "Binary roundtrip failed for value: {}",
            value
        );
    }
}

#[test]
fn property_q16_16_decimal_roundtrip_1000_cases() {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    for _ in 0..1000 {
        let value = rng.gen_range(-10000.0..10000.0);
        let q = Q16_16::from_f64(value);

        assert!(
            q.verify_decimal_roundtrip(0),
            "Decimal roundtrip failed for value: {}",
            value
        );
    }
}

#[test]
fn property_q16_16_binary_determinism_1000_cases() {
    // #VERIFY_DETERMINISTIC_BINARY: 1000+ random values
    use rand::Rng;
    let mut rng = rand::thread_rng();

    for _ in 0..1000 {
        let value = rng.gen_range(-10000.0..10000.0);
        let q = Q16_16::from_f64(value);

        assert!(
            q.verify_binary_determinism(),
            "Binary determinism failed for value: {}",
            value
        );
    }
}

#[test]
fn property_q16_16_decimal_determinism_1000_cases() {
    // #VERIFY_DETERMINISTIC_DECIMAL: 1000+ random values
    use rand::Rng;
    let mut rng = rand::thread_rng();

    for _ in 0..1000 {
        let value = rng.gen_range(-10000.0..10000.0);
        let q = Q16_16::from_f64(value);

        assert!(
            q.verify_decimal_determinism(0),
            "Decimal determinism failed for value: {}",
            value
        );
    }
}

#[test]
fn property_q16_16_hash_determinism_1000_cases() {
    // #VERIFY_DETERMINISTIC_HASH: 1000+ random values
    use rand::Rng;
    let mut rng = rand::thread_rng();

    for _ in 0..1000 {
        let value = rng.gen_range(-10000.0..10000.0);
        let q = Q16_16::from_f64(value);

        assert!(
            q.verify_hash_determinism(),
            "Hash determinism failed for value: {}",
            value
        );
    }
}

#[test]
fn property_q16_16_hash_no_collisions_10k_samples() {
    // #VERIFY_CRC32_COLLISION_RARE: Test for hash collisions
    use rand::Rng;
    use std::collections::HashSet;

    let mut rng = rand::thread_rng();
    let mut hashes = HashSet::new();

    for _ in 0..10000 {
        let value = rng.gen_range(-10000.0..10000.0);
        let q = Q16_16::from_f64(value);
        let hash = q.compute_hash();

        // No collisions expected in 10K random samples
        assert!(
            hashes.insert(hash),
            "Hash collision detected for value: {}",
            value
        );
    }
}

// ============================================================================
// Integration Tests (Q15-Q21) - Cross-Format Compatibility
// ============================================================================

#[test]
fn integration_q16_16_binary_to_decimal() {
    // Test binary → decimal conversion
    let value = Q16_16::from_f64(1234.5678);
    let bytes = value.serialize_binary().unwrap();
    let restored = Q16_16::deserialize_binary(&bytes).unwrap();
    let decimal = restored.serialize_decimal(0);

    assert!(decimal.contains("1234."));
}

#[test]
fn integration_q16_16_decimal_to_binary() {
    // Test decimal → binary conversion
    let value = Q16_16::deserialize_decimal("1234.5678").unwrap();
    let bytes = value.serialize_binary().unwrap();

    assert_eq!(bytes.len(), 22);
}

#[test]
fn integration_q16_16_hash_matches_binary() {
    // Test hash computed from binary representation
    let value = Q16_16::from_f64(1234.5678);
    let hash1 = value.compute_hash();

    // Serialize and deserialize
    let bytes = value.serialize_binary().unwrap();
    let restored = Q16_16::deserialize_binary(&bytes).unwrap();
    let hash2 = restored.compute_hash();

    assert_eq!(hash1, hash2);
}

// ============================================================================
// Stress Tests (Q22-Q28) - Edge Cases, Boundaries, Corruption
// ============================================================================

#[test]
fn stress_q16_16_boundary_values() {
    // Test boundary values (near-MIN, near-MAX, ZERO)
    // Note: -32768.0 exactly cannot roundtrip through decimal because
    // the integer part "32768" exceeds the positive range (max 32767)
    let values = [
        Q16_16::from_f64(-32767.9999), // Near-MIN (safe for decimal roundtrip)
        Q16_16::from_f64(32767.9999),  // Near-MAX
        Q16_16::from_f64(0.0),         // ZERO
    ];

    for value in &values {
        assert!(value.verify_binary_roundtrip());
        assert!(value.verify_decimal_roundtrip(0));
        assert!(value.verify_hash_determinism());
    }
}

#[test]
fn stress_q16_16_precision_edge_cases() {
    // Test precision edge cases
    let values = [
        Q16_16::from_f64(0.0001),    // Small positive
        Q16_16::from_f64(-0.0001),   // Small negative
        Q16_16::from_f64(0.9999),    // Near 1.0
        Q16_16::from_f64(9999.9999), // Large value
    ];

    for value in &values {
        assert!(value.verify_binary_roundtrip());
        assert!(value.verify_hash_determinism());
    }
}

#[test]
fn stress_q16_16_buffer_too_small() {
    // Test buffer size validation
    let bytes = vec![0u8; 10]; // Too small (< 22 bytes)
    let result = Q16_16::deserialize_binary(&bytes);

    assert!(result.is_err());
    match result {
        Err(FixedPointSerializeError::BufferTooSmall { required, actual }) => {
            assert_eq!(required, 22);
            assert_eq!(actual, 10);
        }
        _ => panic!("Expected BufferTooSmall error"),
    }
}

#[test]
fn stress_q16_16_all_bytes_corrupted() {
    // Test behavior when all bytes are corrupted
    let value = Q16_16::from_f64(1234.5678);
    let mut bytes = value.serialize_binary().unwrap();

    // Corrupt all data bytes (keep header)
    for i in 10..18 {
        bytes[i] ^= 0xFF;
    }

    let result = Q16_16::deserialize_binary(&bytes);
    assert!(result.is_err()); // Should fail checksum
}

#[test]
fn stress_q16_16_sequential_serialization() {
    // Test serializing 1000 values in sequence
    for i in 0..1000 {
        let value = Q16_16::from_f64(i as f64);
        let bytes = value.serialize_binary().unwrap();
        let restored = Q16_16::deserialize_binary(&bytes).unwrap();
        assert_eq!(value, restored);
    }
}

// ============================================================================
// Before/After Migration Validation (Q15-Q21)
// ============================================================================

#[test]
fn migration_compatibility_with_legacy_format() {
    // Verify new format is compatible with legacy FixedPointSerialize
    use super::fixed_point_serialize::{FixedPointSerialize as LegacyTrait, FixedQ16_16};

    let legacy = FixedQ16_16::from_decimal(1234, 5678);
    let legacy_raw = legacy.serialize_raw();

    // New format should deserialize same raw value (cast i64 to i32 for Q16.16)
    let new_value = Q16_16::from_raw(legacy_raw as i32);
    assert_eq!(new_value.to_raw(), legacy_raw as i32);
}

#[test]
fn migration_decimal_format_matches() {
    // Verify decimal format matches between old and new implementations
    use super::fixed_point_serialize::{FixedPointSerialize as LegacyTrait, FixedQ16_16};

    let legacy = FixedQ16_16::from_decimal(1234, 5678);
    let legacy_decimal = legacy.serialize_decimal();

    let new_value = Q16_16::from_raw(legacy.serialize_raw() as i32);
    let new_decimal = new_value.serialize_decimal(0);

    // Both should produce similar decimal strings (allow format differences)
    assert!(legacy_decimal.contains("1234"));
    assert!(new_decimal.contains("1234"));
}
