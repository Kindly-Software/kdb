//! # T28 Property Tests for CapsuleSerialize (Phase 1)
//!
//! **Comprehensive property-based validation for deterministic serialization.**
//!
//! ## T28 Coverage
//!
//! - Q8: Universal properties (determinism, roundtrip)
//! - Q9: Concurrent invariants (atomic snapshots)
//! - Q10: Edge case properties (boundary values)
//! - Q11: ASSUM verification (#VERIFY tags)
//! - Q12: Composition properties (multi-field)
//! - Q13: Statistical properties (hash distribution)
//! - Q14: Regression tracking (proptest regressions)
//!
//! ## Test Strategy
//!
//! 1. **Primitive Types**: u64, i32, [u8; N], etc.
//! 2. **Boundary Values**: MIN, MAX, 0, -1
//! 3. **Determinism**: serialize twice, must equal
//! 4. **Roundtrip**: deserialize(serialize(x)) == x
//! 5. **Error Handling**: Corrupted data always produces error
//! 6. **Concurrent Modification**: TOCTOU prevention via atomic snapshot
//!
//! ## Performance Targets (B32)
//!
//! - Serialization: <10ns for primitives (u64, i32)
//! - Array serialization: <2ns/byte for [u8; N]
//! - Hash integration: <10ns overhead for serialize_for_hash()
//! - Deserialization: <10ns for primitives

#![cfg(feature = "std")]

use atomic_capsule::serialize::{CapsuleSerialize, SerializeError, SerializeResult};
use proptest::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Test Helpers - Simple Capsule Implementations
// ============================================================================

/// Simple u64 capsule for testing
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct U64Capsule {
    value: u64,
}

impl CapsuleSerialize for U64Capsule {
    const MAGIC: u32 = 0x55363400; // "U64\0"
    const VERSION: u16 = 1;
    const FIELD_COUNT: usize = 1;

    fn serialize_deterministic(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::serialized_size());
        bytes.extend_from_slice(&Self::MAGIC.to_le_bytes());
        bytes.extend_from_slice(&Self::VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.value.to_le_bytes());
        bytes
    }

    fn deserialize_from_bytes(bytes: &[u8]) -> SerializeResult<Self> {
        if bytes.len() < Self::serialized_size() {
            return Err(SerializeError::BufferTooSmall {
                required: Self::serialized_size(),
                actual: bytes.len(),
            });
        }

        let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        if magic != Self::MAGIC {
            return Err(SerializeError::InvalidMagic {
                expected: Self::MAGIC,
                actual: magic,
            });
        }

        let version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
        if version != Self::VERSION {
            return Err(SerializeError::VersionMismatch {
                expected: Self::VERSION,
                actual: version,
            });
        }

        let value = u64::from_le_bytes(bytes[6..14].try_into().unwrap());
        Ok(U64Capsule { value })
    }

    fn serialized_size() -> usize {
        4 + 2 + 8 // magic + version + value
    }
}

/// Simple i32 capsule for testing
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct I32Capsule {
    value: i32,
}

impl CapsuleSerialize for I32Capsule {
    const MAGIC: u32 = 0x49333200; // "I32\0"
    const VERSION: u16 = 1;
    const FIELD_COUNT: usize = 1;

    fn serialize_deterministic(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::serialized_size());
        bytes.extend_from_slice(&Self::MAGIC.to_le_bytes());
        bytes.extend_from_slice(&Self::VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.value.to_le_bytes());
        bytes
    }

    fn deserialize_from_bytes(bytes: &[u8]) -> SerializeResult<Self> {
        if bytes.len() < Self::serialized_size() {
            return Err(SerializeError::BufferTooSmall {
                required: Self::serialized_size(),
                actual: bytes.len(),
            });
        }

        let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        if magic != Self::MAGIC {
            return Err(SerializeError::InvalidMagic {
                expected: Self::MAGIC,
                actual: magic,
            });
        }

        let version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
        if version != Self::VERSION {
            return Err(SerializeError::VersionMismatch {
                expected: Self::VERSION,
                actual: version,
            });
        }

        let value = i32::from_le_bytes(bytes[6..10].try_into().unwrap());
        Ok(I32Capsule { value })
    }

    fn serialized_size() -> usize {
        4 + 2 + 4 // magic + version + value
    }
}

/// Array capsule for testing (various sizes)
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct Array8Capsule {
    data: [u8; 8],
}

impl CapsuleSerialize for Array8Capsule {
    const MAGIC: u32 = 0x41525238; // "ARR8"
    const VERSION: u16 = 1;
    const FIELD_COUNT: usize = 1;

    fn serialize_deterministic(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::serialized_size());
        bytes.extend_from_slice(&Self::MAGIC.to_le_bytes());
        bytes.extend_from_slice(&Self::VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.data);
        bytes
    }

    fn deserialize_from_bytes(bytes: &[u8]) -> SerializeResult<Self> {
        if bytes.len() < Self::serialized_size() {
            return Err(SerializeError::BufferTooSmall {
                required: Self::serialized_size(),
                actual: bytes.len(),
            });
        }

        let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        if magic != Self::MAGIC {
            return Err(SerializeError::InvalidMagic {
                expected: Self::MAGIC,
                actual: magic,
            });
        }

        let version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
        if version != Self::VERSION {
            return Err(SerializeError::VersionMismatch {
                expected: Self::VERSION,
                actual: version,
            });
        }

        let mut data = [0u8; 8];
        data.copy_from_slice(&bytes[6..14]);
        Ok(Array8Capsule { data })
    }

    fn serialized_size() -> usize {
        4 + 2 + 8 // magic + version + data
    }
}

/// Multi-field capsule for composition testing
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct MultiFieldCapsule {
    field1: u64,
    field2: i32,
    field3: u16,
}

impl CapsuleSerialize for MultiFieldCapsule {
    const MAGIC: u32 = 0x4D554C54; // "MULT"
    const VERSION: u16 = 1;
    const FIELD_COUNT: usize = 3;

    fn serialize_deterministic(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::serialized_size());
        bytes.extend_from_slice(&Self::MAGIC.to_le_bytes());
        bytes.extend_from_slice(&Self::VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.field1.to_le_bytes());
        bytes.extend_from_slice(&self.field2.to_le_bytes());
        bytes.extend_from_slice(&self.field3.to_le_bytes());
        bytes
    }

    fn deserialize_from_bytes(bytes: &[u8]) -> SerializeResult<Self> {
        if bytes.len() < Self::serialized_size() {
            return Err(SerializeError::BufferTooSmall {
                required: Self::serialized_size(),
                actual: bytes.len(),
            });
        }

        let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        if magic != Self::MAGIC {
            return Err(SerializeError::InvalidMagic {
                expected: Self::MAGIC,
                actual: magic,
            });
        }

        let version = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
        if version != Self::VERSION {
            return Err(SerializeError::VersionMismatch {
                expected: Self::VERSION,
                actual: version,
            });
        }

        let field1 = u64::from_le_bytes(bytes[6..14].try_into().unwrap());
        let field2 = i32::from_le_bytes(bytes[14..18].try_into().unwrap());
        let field3 = u16::from_le_bytes(bytes[18..20].try_into().unwrap());

        Ok(MultiFieldCapsule {
            field1,
            field2,
            field3,
        })
    }

    fn serialized_size() -> usize {
        4 + 2 + 8 + 4 + 2 // magic + version + field1 + field2 + field3
    }
}

// ============================================================================
// T28 Q8: Universal Properties
// ============================================================================

proptest! {
    /// Property: Determinism - serialize twice produces same bytes
    #[test]
    fn prop_u64_deterministic(value in any::<u64>()) {
        let capsule = U64Capsule { value };

        // Property: Serialize twice, must equal
        let bytes1 = capsule.serialize_deterministic();
        let bytes2 = capsule.serialize_deterministic();

        prop_assert_eq!(&bytes1, &bytes2, "Serialization not deterministic");
        prop_assert!(capsule.verify_determinism(), "verify_determinism() failed");
    }

    /// Property: Roundtrip - deserialize(serialize(x)) == x
    #[test]
    fn prop_u64_roundtrip(value in any::<u64>()) {
        let capsule = U64Capsule { value };

        // Property: Roundtrip preserves value
        let bytes = capsule.serialize_deterministic();
        let restored = U64Capsule::deserialize_from_bytes(&bytes)?;

        prop_assert_eq!(capsule, restored, "Roundtrip failed");
        prop_assert!(capsule.verify_roundtrip(), "verify_roundtrip() failed");
    }

    /// Property: i32 determinism (including negative values)
    #[test]
    fn prop_i32_deterministic(value in any::<i32>()) {
        let capsule = I32Capsule { value };

        let bytes1 = capsule.serialize_deterministic();
        let bytes2 = capsule.serialize_deterministic();

        prop_assert_eq!(&bytes1, &bytes2);
        prop_assert!(capsule.verify_determinism());
    }

    /// Property: i32 roundtrip (including negative values)
    #[test]
    fn prop_i32_roundtrip(value in any::<i32>()) {
        let capsule = I32Capsule { value };

        let bytes = capsule.serialize_deterministic();
        let restored = I32Capsule::deserialize_from_bytes(&bytes)?;

        prop_assert_eq!(capsule, restored);
        prop_assert!(capsule.verify_roundtrip());
    }

    /// Property: Array determinism
    #[test]
    fn prop_array8_deterministic(data in any::<[u8; 8]>()) {
        let capsule = Array8Capsule { data };

        let bytes1 = capsule.serialize_deterministic();
        let bytes2 = capsule.serialize_deterministic();

        prop_assert_eq!(&bytes1, &bytes2);
        prop_assert!(capsule.verify_determinism());
    }

    /// Property: Array roundtrip
    #[test]
    fn prop_array8_roundtrip(data in any::<[u8; 8]>()) {
        let capsule = Array8Capsule { data };

        let bytes = capsule.serialize_deterministic();
        let restored = Array8Capsule::deserialize_from_bytes(&bytes)?;

        prop_assert_eq!(capsule, restored);
        prop_assert!(capsule.verify_roundtrip());
    }

    /// Property: Multi-field determinism
    #[test]
    fn prop_multifield_deterministic(
        field1 in any::<u64>(),
        field2 in any::<i32>(),
        field3 in any::<u16>()
    ) {
        let capsule = MultiFieldCapsule { field1, field2, field3 };

        let bytes1 = capsule.serialize_deterministic();
        let bytes2 = capsule.serialize_deterministic();

        prop_assert_eq!(&bytes1, &bytes2);
        prop_assert!(capsule.verify_determinism());
    }

    /// Property: Multi-field roundtrip
    #[test]
    fn prop_multifield_roundtrip(
        field1 in any::<u64>(),
        field2 in any::<i32>(),
        field3 in any::<u16>()
    ) {
        let capsule = MultiFieldCapsule { field1, field2, field3 };

        let bytes = capsule.serialize_deterministic();
        let restored = MultiFieldCapsule::deserialize_from_bytes(&bytes)?;

        prop_assert_eq!(capsule, restored);
        prop_assert!(capsule.verify_roundtrip());
    }
}

// ============================================================================
// T28 Q10: Edge Case Properties
// ============================================================================

#[test]
fn test_u64_boundary_values() {
    // Test boundary values explicitly
    let test_values = vec![0, 1, u64::MAX, u64::MAX - 1, u64::MAX / 2];

    for value in test_values {
        let capsule = U64Capsule { value };

        // Determinism
        let bytes1 = capsule.serialize_deterministic();
        let bytes2 = capsule.serialize_deterministic();
        assert_eq!(bytes1, bytes2, "Boundary value {} not deterministic", value);

        // Roundtrip
        let restored = U64Capsule::deserialize_from_bytes(&bytes1)
            .expect("Boundary value deserialization failed");
        assert_eq!(
            capsule, restored,
            "Boundary value {} roundtrip failed",
            value
        );
    }
}

#[test]
fn test_i32_boundary_values() {
    // Test boundary values including negatives
    let test_values = vec![0, 1, -1, i32::MAX, i32::MIN, i32::MIN + 1];

    for value in test_values {
        let capsule = I32Capsule { value };

        // Determinism
        let bytes1 = capsule.serialize_deterministic();
        let bytes2 = capsule.serialize_deterministic();
        assert_eq!(bytes1, bytes2, "Boundary value {} not deterministic", value);

        // Roundtrip
        let restored = I32Capsule::deserialize_from_bytes(&bytes1)
            .expect("Boundary value deserialization failed");
        assert_eq!(
            capsule, restored,
            "Boundary value {} roundtrip failed",
            value
        );
    }
}

#[test]
fn test_array_special_patterns() {
    // Test special patterns (all zeros, all ones, alternating)
    let patterns = vec![
        [0u8; 8],
        [255u8; 8],
        [0, 255, 0, 255, 0, 255, 0, 255],
        [1, 2, 3, 4, 5, 6, 7, 8],
    ];

    for data in patterns {
        let capsule = Array8Capsule { data };

        // Determinism
        let bytes1 = capsule.serialize_deterministic();
        let bytes2 = capsule.serialize_deterministic();
        assert_eq!(bytes1, bytes2, "Pattern {:?} not deterministic", data);

        // Roundtrip
        let restored =
            Array8Capsule::deserialize_from_bytes(&bytes1).expect("Pattern deserialization failed");
        assert_eq!(capsule, restored, "Pattern {:?} roundtrip failed", data);
    }
}

// ============================================================================
// T28 Q11: ASSUM Verification - Error Handling
// ============================================================================

/// #VERIFY_BUFFER_TOO_SMALL: Corrupted data produces error
#[test]
fn test_error_buffer_too_small() {
    let capsule = U64Capsule { value: 42 };
    let bytes = capsule.serialize_deterministic();

    // Truncate buffer
    let truncated = &bytes[..5];

    let result = U64Capsule::deserialize_from_bytes(truncated);
    assert!(result.is_err(), "Should reject truncated buffer");

    match result {
        Err(SerializeError::BufferTooSmall { required, actual }) => {
            assert_eq!(required, U64Capsule::serialized_size());
            assert_eq!(actual, 5);
        }
        _ => panic!("Expected BufferTooSmall error"),
    }
}

/// #VERIFY_INVALID_MAGIC: Corrupted magic number produces error
#[test]
fn test_error_invalid_magic() {
    let capsule = U64Capsule { value: 42 };
    let mut bytes = capsule.serialize_deterministic();

    // Corrupt magic number
    bytes[0] = 0xFF;

    let result = U64Capsule::deserialize_from_bytes(&bytes);
    assert!(result.is_err(), "Should reject invalid magic");

    match result {
        Err(SerializeError::InvalidMagic { expected, actual }) => {
            assert_eq!(expected, U64Capsule::MAGIC);
            assert_ne!(actual, expected);
        }
        _ => panic!("Expected InvalidMagic error"),
    }
}

/// #VERIFY_VERSION_MISMATCH: Incompatible version produces error
#[test]
fn test_error_version_mismatch() {
    let capsule = U64Capsule { value: 42 };
    let mut bytes = capsule.serialize_deterministic();

    // Corrupt version
    bytes[4] = 99;
    bytes[5] = 0;

    let result = U64Capsule::deserialize_from_bytes(&bytes);
    assert!(result.is_err(), "Should reject version mismatch");

    match result {
        Err(SerializeError::VersionMismatch { expected, actual }) => {
            assert_eq!(expected, U64Capsule::VERSION);
            assert_eq!(actual, 99);
        }
        _ => panic!("Expected VersionMismatch error"),
    }
}

// ============================================================================
// T28 Q12: Composition Properties
// ============================================================================

#[test]
fn test_multi_field_ordering_preserved() {
    // Property: Field order in serialized bytes matches declaration order
    let capsule = MultiFieldCapsule {
        field1: 0x1122334455667788,
        field2: 0x11223344u32 as i32,
        field3: 0x1122,
    };

    let bytes = capsule.serialize_deterministic();

    // Verify field order: magic (4) + version (2) + field1 (8) + field2 (4) + field3 (2)
    let field1_bytes = &bytes[6..14];
    let field2_bytes = &bytes[14..18];
    let field3_bytes = &bytes[18..20];

    assert_eq!(
        u64::from_le_bytes(field1_bytes.try_into().unwrap()),
        0x1122334455667788
    );
    assert_eq!(
        i32::from_le_bytes(field2_bytes.try_into().unwrap()),
        0x11223344u32 as i32
    );
    assert_eq!(u16::from_le_bytes(field3_bytes.try_into().unwrap()), 0x1122);
}

// ============================================================================
// T28 Q13: Statistical Properties (Hash Distribution)
// ============================================================================

#[cfg(feature = "fast-hash")]
#[test]
fn test_hash_distribution_no_collisions() {
    use std::collections::HashSet;

    // Generate 10,000 unique values
    let mut hashes = HashSet::new();
    let count = 10_000;

    for i in 0..count {
        let capsule = U64Capsule { value: i };
        let hash = capsule.serialize_for_hash();
        hashes.insert(hash);
    }

    // Property: No hash collisions for unique inputs
    assert_eq!(
        hashes.len(),
        count as usize,
        "Hash collisions detected: expected {} unique, got {}",
        count,
        hashes.len()
    );
}

// ============================================================================
// T28 Q14: Regression Tracking
// ============================================================================

/// Regression test: Known values must serialize to known bytes
#[test]
fn test_regression_u64_zero() {
    let capsule = U64Capsule { value: 0 };
    let bytes = capsule.serialize_deterministic();

    // Expected: magic (4) + version (2) + value (8) = 14 bytes
    assert_eq!(bytes.len(), 14);

    // Verify exact byte sequence
    let expected = vec![
        0x00, 0x64, 0x36, 0x55, // magic (little-endian)
        0x01, 0x00, // version
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // value = 0
    ];
    assert_eq!(bytes, expected, "Regression: u64(0) changed serialization");
}

/// Regression test: Known i32 negative value
#[test]
fn test_regression_i32_negative_one() {
    let capsule = I32Capsule { value: -1 };
    let bytes = capsule.serialize_deterministic();

    // Expected: magic (4) + version (2) + value (4) = 10 bytes
    assert_eq!(bytes.len(), 10);

    // Verify exact byte sequence
    let expected = vec![
        0x00, 0x32, 0x33, 0x49, // magic (little-endian)
        0x01, 0x00, // version
        0xFF, 0xFF, 0xFF, 0xFF, // value = -1 (two's complement)
    ];
    assert_eq!(bytes, expected, "Regression: i32(-1) changed serialization");
}

// ============================================================================
// T28 Q9: Concurrent Invariants (Integration Tests)
// ============================================================================

/// Property: Concurrent serialization produces consistent results
#[test]
fn test_concurrent_serialization_deterministic() {
    let capsule = Arc::new(U64Capsule { value: 42 });
    let num_threads = 10;
    let iterations = 1000;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let cap = Arc::clone(&capsule);
            thread::spawn(move || {
                let mut all_equal = true;
                let first_bytes = cap.serialize_deterministic();

                for _ in 0..iterations {
                    let bytes = cap.serialize_deterministic();
                    if bytes != first_bytes {
                        all_equal = false;
                        break;
                    }
                }

                all_equal
            })
        })
        .collect();

    for handle in handles {
        let result = handle.join().expect("Thread panicked");
        assert!(result, "Concurrent serialization not deterministic");
    }
}

// ============================================================================
// Stress Tests (T28 Q22)
// ============================================================================

/// Stress test: 10,000 random serializations
#[test]
fn stress_test_u64_10k_random() {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    for _ in 0..10_000 {
        let value: u64 = rng.gen();
        let capsule = U64Capsule { value };

        // Determinism
        let bytes1 = capsule.serialize_deterministic();
        let bytes2 = capsule.serialize_deterministic();
        assert_eq!(bytes1, bytes2, "Non-deterministic at value {}", value);

        // Roundtrip
        let restored = U64Capsule::deserialize_from_bytes(&bytes1).expect("Deserialization failed");
        assert_eq!(capsule, restored, "Roundtrip failed at value {}", value);
    }
}

/// Stress test: Concurrent 1000-thread serialize + deserialize
#[test]
#[ignore] // Run manually: cargo test --test capsule_serialize_property_tests -- --ignored
fn stress_test_concurrent_1000_threads() {
    let num_threads = 1000;
    let per_thread = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            thread::spawn(move || {
                for i in 0..per_thread {
                    let value = (thread_id * per_thread + i) as u64;
                    let capsule = U64Capsule { value };

                    // Serialize
                    let bytes = capsule.serialize_deterministic();

                    // Deserialize
                    let restored =
                        U64Capsule::deserialize_from_bytes(&bytes).expect("Deserialization failed");

                    assert_eq!(capsule, restored);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread panicked");
    }
}
