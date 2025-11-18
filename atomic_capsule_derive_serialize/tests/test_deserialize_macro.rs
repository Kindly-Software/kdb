//! Integration test for CapsuleDeserialize macro
//!
//! Tests that the macro generates valid code that compiles and implements CapsuleDeserialize trait.

use atomic_capsule_derive_serialize::CapsuleDeserialize;
use atomic_capsule::serialize::CapsuleDeserialize as CapsuleDeserializeTrait;

// Test struct with named fields
#[derive(CapsuleDeserialize)]
#[repr(C, align(128))]
struct PaymentCapsule {
    amount: i64,
    fee: i64,
}

// Test tuple struct
#[derive(CapsuleDeserialize)]
#[repr(C, align(64))]
struct CoordinatePair(i64, i64);

#[test]
fn test_deserialize_macro_generates_trait() {
    // This test verifies that the macro generates code that implements CapsuleDeserialize
    // The actual deserialization requires proper binary format, which is tested separately

    // Just verify that the trait is implemented (compile-time check)
    // If this code compiles, the macro worked correctly
    let _: fn(&[u8]) -> Result<PaymentCapsule, _> = PaymentCapsule::deserialize;
    let _: fn(&[u8]) -> Result<CoordinatePair, _> = CoordinatePair::deserialize;
}

#[test]
fn test_deserialize_insufficient_data_error() {
    // Binary data that's too small (< 22 bytes for header)
    let small_data = vec![0x46, 0x49, 0x58, 0x50]; // Just 4 bytes

    let result = PaymentCapsule::deserialize(&small_data);
    assert!(result.is_err(), "Should fail with insufficient data");
}

#[test]
fn test_deserialize_invalid_magic_error() {
    // Create buffer with valid size but wrong magic
    let mut data = vec![0u8; 22];
    // First 4 bytes are magic (should be 0x43505346)
    data[0] = 0xFF;
    data[1] = 0xFF;
    data[2] = 0xFF;
    data[3] = 0xFF;

    let result = PaymentCapsule::deserialize(&data);
    assert!(result.is_err(), "Should fail with invalid format");
}

#[test]
fn test_deserialize_valid_minimal_structure() {
    // Create valid binary structure (minimal header + 2 fields)
    let mut data = vec![0u8; 38]; // 22 byte header + 16 bytes payload (2 i64 fields)

    // Magic number: 0x43505346 ("CPSF")
    data[0..4].copy_from_slice(&0x43505346u32.to_le_bytes());

    // Version: 0x0001
    data[4..6].copy_from_slice(&0x0001u16.to_le_bytes());

    // Payload size: 16 bytes (2 x i64)
    data[6..14].copy_from_slice(&16u64.to_le_bytes());

    // Hash (placeholder)
    data[14..22].copy_from_slice(&0u64.to_le_bytes());

    // Field 1: 1000 (as i64)
    data[22..30].copy_from_slice(&1000i64.to_le_bytes());

    // Field 2: 50 (as i64)
    data[30..38].copy_from_slice(&50i64.to_le_bytes());

    let result = PaymentCapsule::deserialize(&data);

    // Should deserialize successfully if the generated code is correct
    match result {
        Ok(capsule) => {
            assert_eq!(capsule.amount, 1000);
            assert_eq!(capsule.fee, 50);
        }
        Err(e) => {
            panic!("Deserialization failed: {:?}", e);
        }
    }
}
