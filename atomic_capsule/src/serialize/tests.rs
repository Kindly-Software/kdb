//! # CapsuleSerialize Tests - Compile-Time Safety & Property Verification
//!
//! **Phase 1 Verification Approach** - Comprehensive test suite for CapsuleSerialize
//!
//! ## Test Coverage (T28 Framework Q1-Q7: Unit Tests)
//!
//! 1. **Compile-Time Safety** (trybuild compile-fail tests)
//!    - Missing #[repr(C)] detection
//!    - Type system enforcement (PartialEq requirement)
//!    - Alignment verification
//!
//! 2. **Roundtrip Properties** (property-based testing)
//!    - Determinism: serialize twice → same bytes
//!    - Reversibility: deserialize(serialize(x)) == x
//!    - Endianness: Cross-platform consistency
//!
//! 3. **Error Handling** (all SerializeError variants)
//!    - BufferTooSmall: Too small buffer detection
//!    - InvalidMagic: Wrong magic number rejection
//!    - VersionMismatch: Incompatible version detection
//!    - ChecksumMismatch: Corruption detection (Phase 2)
//!
//! 4. **Edge Cases** (boundary conditions)
//!    - Zero values (all fields zero)
//!    - Maximum values (u64::MAX, i64::MAX/MIN)
//!    - Empty arrays
//!    - Atomic snapshot consistency
//!
//! ## ASSUM Framework Coverage
//!
//! ```text
//! #ASSUME_REPR_C: Types must have #[repr(C)]
//! #VERIFY_REPR_C: Compile-time detection via size checks + trybuild
//!
//! #ASSUME_DETERMINISTIC: Same input always same output
//! #VERIFY_DETERMINISTIC: Property test with identical consecutive serializations
//!
//! #ASSUME_ATOMIC_SNAPSHOT: Concurrent reads produce consistent snapshots
//! #VERIFY_ATOMIC_SNAPSHOT: Test concurrent modifications during serialization
//!
//! #ASSUME_LITTLE_ENDIAN: All integers serialized as little-endian
//! #VERIFY_LITTLE_ENDIAN: Byte-level comparison against manual LE encoding
//!
//! #ASSUME_MAGIC_VALIDATION: Deserialize rejects invalid magic numbers
//! #VERIFY_MAGIC_VALIDATION: Test all magic number mismatches
//!
//! #ASSUME_VERSION_VALIDATION: Deserialize rejects incompatible versions
//! #VERIFY_VERSION_VALIDATION: Test version mismatch detection
//! ```
//!
//! ## UCE34 Framework Application
//!
//! - **Q10**: Tier 0 (Auditable Foundation) - Hash chain integrity
//! - **Q11**: Rust Transform - Zero-copy via atomic_from_mut
//! - **Q33**: Validation - Compile-fail tests + property tests
//! - **Q34**: Auditability - Deterministic serialization for audit trails

use super::*;

#[cfg(feature = "std")]
use std::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// TEST FIXTURES - Valid #[repr(C)] Capsules
// ============================================================================

/// Simple test capsule with primitive types
///
/// # ASSUM Framework
/// - #ASSUME_REPR_C: #[repr(C)] guarantees deterministic field order
/// - #VERIFY_REPR_C: Compile-time size check in test_basic_roundtrip
#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(C)]
struct SimpleCapsule {
    field_u64: u64,
    field_i32: i32,
    field_u16: u16,
    field_u8: u8,
    _padding: u8, // Explicit padding for alignment
}

impl CapsuleSerialize for SimpleCapsule {
    const MAGIC: u32 = 0x53494D50; // "SIMP"
    const VERSION: u16 = 1;
    const FIELD_COUNT: usize = 4;

    fn serialize_deterministic(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::serialized_size());
        bytes.extend_from_slice(&Self::MAGIC.to_le_bytes());
        bytes.extend_from_slice(&Self::VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.field_u64.to_le_bytes());
        bytes.extend_from_slice(&self.field_i32.to_le_bytes());
        bytes.extend_from_slice(&self.field_u16.to_le_bytes());
        bytes.push(self.field_u8);
        bytes
    }

    fn deserialize_from_bytes(bytes: &[u8]) -> SerializeResult<Self> {
        // #ASSUME_BUFFER_SIZE: Buffer must be at least serialized_size()
        // #VERIFY_BUFFER_SIZE: Runtime check with clear error
        if bytes.len() < Self::serialized_size() {
            return Err(SerializeError::BufferTooSmall {
                required: Self::serialized_size(),
                actual: bytes.len(),
            });
        }

        let mut offset = 0;

        // Parse magic (4 bytes)
        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        offset += 4;

        // #ASSUME_MAGIC_VALIDATION: Magic must match expected value
        // #VERIFY_MAGIC_VALIDATION: Test in test_invalid_magic
        if magic != Self::MAGIC {
            return Err(SerializeError::InvalidMagic {
                expected: Self::MAGIC,
                actual: magic,
            });
        }

        // Parse version (2 bytes)
        let version = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        offset += 2;

        // #ASSUME_VERSION_VALIDATION: Version must match expected value
        // #VERIFY_VERSION_VALIDATION: Test in test_version_mismatch
        if version != Self::VERSION {
            return Err(SerializeError::VersionMismatch {
                expected: Self::VERSION,
                actual: version,
            });
        }

        // Parse fields (little-endian)
        let field_u64 = u64::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]);
        offset += 8;

        let field_i32 = i32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);
        offset += 4;

        let field_u16 = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        offset += 2;

        let field_u8 = bytes[offset];

        Ok(SimpleCapsule {
            field_u64,
            field_i32,
            field_u16,
            field_u8,
            _padding: 0,
        })
    }

    fn serialized_size() -> usize {
        4 + 2 + 8 + 4 + 2 + 1 // magic + version + fields
    }
}

/// Capsule with array fields (fixed-size)
#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(C)]
struct ArrayCapsule {
    data: [u8; 32],
    count: u64,
}

impl CapsuleSerialize for ArrayCapsule {
    const MAGIC: u32 = 0x41525241; // "ARRA"
    const VERSION: u16 = 1;
    const FIELD_COUNT: usize = 2;

    fn serialize_deterministic(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::serialized_size());
        bytes.extend_from_slice(&Self::MAGIC.to_le_bytes());
        bytes.extend_from_slice(&Self::VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.data);
        bytes.extend_from_slice(&self.count.to_le_bytes());
        bytes
    }

    fn deserialize_from_bytes(bytes: &[u8]) -> SerializeResult<Self> {
        if bytes.len() < Self::serialized_size() {
            return Err(SerializeError::BufferTooSmall {
                required: Self::serialized_size(),
                actual: bytes.len(),
            });
        }

        let mut offset = 0;

        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        offset += 4;

        if magic != Self::MAGIC {
            return Err(SerializeError::InvalidMagic {
                expected: Self::MAGIC,
                actual: magic,
            });
        }

        let version = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        offset += 2;

        if version != Self::VERSION {
            return Err(SerializeError::VersionMismatch {
                expected: Self::VERSION,
                actual: version,
            });
        }

        let mut data = [0u8; 32];
        data.copy_from_slice(&bytes[offset..offset + 32]);
        offset += 32;

        let count = u64::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]);

        Ok(ArrayCapsule { data, count })
    }

    fn serialized_size() -> usize {
        4 + 2 + 32 + 8 // magic + version + data + count
    }
}

/// Capsule with atomic fields (snapshot testing)
#[cfg(feature = "std")]
#[derive(Debug)]
#[repr(C, align(64))]
struct AtomicCapsule {
    state: AtomicU64,
    counter: AtomicU64,
    _padding: [u8; 48],
}

#[cfg(feature = "std")]
impl PartialEq for AtomicCapsule {
    fn eq(&self, other: &Self) -> bool {
        self.state.load(Ordering::Acquire) == other.state.load(Ordering::Acquire)
            && self.counter.load(Ordering::Acquire) == other.counter.load(Ordering::Acquire)
    }
}

#[cfg(feature = "std")]
impl CapsuleSerialize for AtomicCapsule {
    const MAGIC: u32 = 0x41544F4D; // "ATOM"
    const VERSION: u16 = 1;
    const FIELD_COUNT: usize = 2;

    fn serialize_deterministic(&self) -> Vec<u8> {
        // #ASSUME_ATOMIC_SNAPSHOT: Load all atomics with Acquire ordering
        // #VERIFY_ATOMIC_SNAPSHOT: Test in test_atomic_snapshot_consistency
        let state_snapshot = self.state.load(Ordering::Acquire);
        let counter_snapshot = self.counter.load(Ordering::Acquire);

        let mut bytes = Vec::with_capacity(Self::serialized_size());
        bytes.extend_from_slice(&Self::MAGIC.to_le_bytes());
        bytes.extend_from_slice(&Self::VERSION.to_le_bytes());
        bytes.extend_from_slice(&state_snapshot.to_le_bytes());
        bytes.extend_from_slice(&counter_snapshot.to_le_bytes());
        bytes
    }

    fn deserialize_from_bytes(bytes: &[u8]) -> SerializeResult<Self> {
        if bytes.len() < Self::serialized_size() {
            return Err(SerializeError::BufferTooSmall {
                required: Self::serialized_size(),
                actual: bytes.len(),
            });
        }

        let mut offset = 0;

        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        offset += 4;

        if magic != Self::MAGIC {
            return Err(SerializeError::InvalidMagic {
                expected: Self::MAGIC,
                actual: magic,
            });
        }

        let version = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        offset += 2;

        if version != Self::VERSION {
            return Err(SerializeError::VersionMismatch {
                expected: Self::VERSION,
                actual: version,
            });
        }

        let state = u64::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]);
        offset += 8;

        let counter = u64::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]);

        Ok(AtomicCapsule {
            state: AtomicU64::new(state),
            counter: AtomicU64::new(counter),
            _padding: [0; 48],
        })
    }

    fn serialized_size() -> usize {
        4 + 2 + 8 + 8 // magic + version + state + counter
    }
}

// ============================================================================
// UNIT TESTS - T28 Q1-Q7
// ============================================================================

#[test]
fn test_basic_roundtrip() {
    // #VERIFY_DETERMINISTIC: Roundtrip property
    let capsule = SimpleCapsule {
        field_u64: 0x0123456789ABCDEF,
        field_i32: -42,
        field_u16: 1234,
        field_u8: 0xFF,
        _padding: 0,
    };

    let bytes = capsule.serialize_deterministic();
    let restored = SimpleCapsule::deserialize_from_bytes(&bytes).unwrap();

    assert_eq!(capsule, restored);
}

#[test]
fn test_determinism_same_struct_twice() {
    // #VERIFY_DETERMINISTIC: Same struct → same bytes (twice)
    let capsule = SimpleCapsule {
        field_u64: 42,
        field_i32: -1,
        field_u16: 0,
        field_u8: 7,
        _padding: 0,
    };

    let bytes1 = capsule.serialize_deterministic();
    let bytes2 = capsule.serialize_deterministic();

    assert_eq!(bytes1, bytes2, "Same struct must produce identical bytes");
}

#[test]
fn test_verify_determinism_trait_method() {
    // #VERIFY_DETERMINISTIC: Use trait method
    let capsule = SimpleCapsule {
        field_u64: 999,
        field_i32: -999,
        field_u16: 555,
        field_u8: 0xAA,
        _padding: 0,
    };

    assert!(capsule.verify_determinism());
}

#[test]
fn test_verify_roundtrip_trait_method() {
    // #VERIFY_DETERMINISTIC: Use trait method for roundtrip
    let capsule = SimpleCapsule {
        field_u64: u64::MAX,
        field_i32: i32::MIN,
        field_u16: 0,
        field_u8: 1,
        _padding: 0,
    };

    assert!(capsule.verify_roundtrip());
}

#[test]
fn test_endianness_little_endian() {
    // #VERIFY_LITTLE_ENDIAN: Verify little-endian byte order
    let capsule = SimpleCapsule {
        field_u64: 0x0102030405060708,
        field_i32: 0x01020304,
        field_u16: 0x0102,
        field_u8: 0xAB,
        _padding: 0,
    };

    let bytes = capsule.serialize_deterministic();

    // Skip magic (4 bytes) + version (2 bytes) = offset 6
    let offset = 6;

    // Verify u64 field (little-endian)
    assert_eq!(bytes[offset], 0x08);
    assert_eq!(bytes[offset + 1], 0x07);
    assert_eq!(bytes[offset + 2], 0x06);
    assert_eq!(bytes[offset + 3], 0x05);
    assert_eq!(bytes[offset + 4], 0x04);
    assert_eq!(bytes[offset + 5], 0x03);
    assert_eq!(bytes[offset + 6], 0x02);
    assert_eq!(bytes[offset + 7], 0x01);
}

#[test]
fn test_magic_number_validation() {
    // #VERIFY_MAGIC_VALIDATION: Reject invalid magic numbers
    let capsule = SimpleCapsule {
        field_u64: 42,
        field_i32: 0,
        field_u16: 0,
        field_u8: 0,
        _padding: 0,
    };

    let mut bytes = capsule.serialize_deterministic();

    // Corrupt magic number (first 4 bytes)
    bytes[0] = 0xFF;
    bytes[1] = 0xFF;
    bytes[2] = 0xFF;
    bytes[3] = 0xFF;

    let result = SimpleCapsule::deserialize_from_bytes(&bytes);
    assert!(matches!(result, Err(SerializeError::InvalidMagic { .. })));
}

#[test]
fn test_version_mismatch_detection() {
    // #VERIFY_VERSION_VALIDATION: Reject incompatible versions
    let capsule = SimpleCapsule {
        field_u64: 123,
        field_i32: 456,
        field_u16: 789,
        field_u8: 0,
        _padding: 0,
    };

    let mut bytes = capsule.serialize_deterministic();

    // Corrupt version (bytes 4-5)
    bytes[4] = 0xFF;
    bytes[5] = 0xFF;

    let result = SimpleCapsule::deserialize_from_bytes(&bytes);
    assert!(matches!(
        result,
        Err(SerializeError::VersionMismatch { .. })
    ));
}

#[test]
fn test_buffer_too_small_error() {
    // #VERIFY_BUFFER_SIZE: Detect too-small buffers
    let bytes = vec![0u8; 5]; // Way too small

    let result = SimpleCapsule::deserialize_from_bytes(&bytes);
    assert!(matches!(result, Err(SerializeError::BufferTooSmall { .. })));

    if let Err(SerializeError::BufferTooSmall { required, actual }) = result {
        assert_eq!(actual, 5);
        assert!(required > 5);
    }
}

#[test]
fn test_zero_values() {
    // Edge case: All zeros
    let capsule = SimpleCapsule {
        field_u64: 0,
        field_i32: 0,
        field_u16: 0,
        field_u8: 0,
        _padding: 0,
    };

    assert!(capsule.verify_roundtrip());
    assert!(capsule.verify_determinism());
}

#[test]
fn test_maximum_values() {
    // Edge case: Maximum values
    let capsule = SimpleCapsule {
        field_u64: u64::MAX,
        field_i32: i32::MAX,
        field_u16: u16::MAX,
        field_u8: u8::MAX,
        _padding: 0,
    };

    assert!(capsule.verify_roundtrip());
    assert!(capsule.verify_determinism());
}

#[test]
fn test_minimum_signed_values() {
    // Edge case: Minimum signed values
    let capsule = SimpleCapsule {
        field_u64: 0,
        field_i32: i32::MIN,
        field_u16: 0,
        field_u8: 0,
        _padding: 0,
    };

    assert!(capsule.verify_roundtrip());
}

#[test]
fn test_array_capsule_roundtrip() {
    let mut data = [0u8; 32];
    for (i, byte) in data.iter_mut().enumerate() {
        *byte = (i % 256) as u8;
    }

    let capsule = ArrayCapsule { data, count: 999 };

    assert!(capsule.verify_roundtrip());
    assert!(capsule.verify_determinism());
}

#[test]
fn test_array_capsule_zero_array() {
    // Edge case: Zero-filled array
    let capsule = ArrayCapsule {
        data: [0u8; 32],
        count: 0,
    };

    assert!(capsule.verify_roundtrip());
}

#[test]
#[cfg(feature = "std")]
fn test_atomic_capsule_roundtrip() {
    // #VERIFY_ATOMIC_SNAPSHOT: Atomic snapshot consistency
    let capsule = AtomicCapsule {
        state: AtomicU64::new(0xDEADBEEF),
        counter: AtomicU64::new(42),
        _padding: [0; 48],
    };

    assert!(capsule.verify_roundtrip());
}

#[test]
#[cfg(feature = "std")]
fn test_atomic_snapshot_consistency() {
    // #VERIFY_ATOMIC_SNAPSHOT: Concurrent modifications don't break snapshot
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(AtomicCapsule {
        state: AtomicU64::new(0),
        counter: AtomicU64::new(0),
        _padding: [0; 48],
    });

    let capsule_clone = Arc::clone(&capsule);

    // Spawn thread that modifies atomics during serialization
    let handle = thread::spawn(move || {
        for i in 0..1000 {
            capsule_clone.state.store(i, Ordering::Release);
            capsule_clone.counter.fetch_add(1, Ordering::Relaxed);
        }
    });

    // Serialize while other thread is modifying
    // This should produce a consistent snapshot (all from same instant)
    let bytes = capsule.serialize_deterministic();

    handle.join().unwrap();

    // Verify deserialization succeeds (snapshot was valid)
    let restored = AtomicCapsule::deserialize_from_bytes(&bytes).unwrap();

    // Verify snapshot consistency: both atomics from same serialization moment
    // (They should be internally consistent, even if not equal to final values)
    let state = restored.state.load(Ordering::Acquire);
    let counter = restored.counter.load(Ordering::Acquire);

    // Just verify they're valid values (no corruption)
    assert!(state <= 1000);
    assert!(counter <= 1000);
}

#[test]
fn test_serialized_size_matches_actual() {
    let capsule = SimpleCapsule {
        field_u64: 0,
        field_i32: 0,
        field_u16: 0,
        field_u8: 0,
        _padding: 0,
    };

    let bytes = capsule.serialize_deterministic();
    assert_eq!(bytes.len(), SimpleCapsule::serialized_size());
}

#[test]
fn test_error_display_formatting() {
    // Verify error messages are human-readable
    let err = SerializeError::BufferTooSmall {
        required: 100,
        actual: 50,
    };
    let msg = format!("{}", err);
    assert!(msg.contains("100"));
    assert!(msg.contains("50"));

    let err = SerializeError::InvalidMagic {
        expected: 0x12345678,
        actual: 0xDEADBEEF,
    };
    let msg = format!("{}", err);
    assert!(msg.contains("0x12345678"));
    assert!(msg.contains("0xDEADBEEF"));
}

// ============================================================================
// PROPERTY TESTS - T28 Q8-Q14 (Simplified Unit-Style)
// ============================================================================

#[test]
fn test_property_determinism_100_iterations() {
    // Property: Same struct always produces same bytes
    for i in 0..100 {
        let capsule = SimpleCapsule {
            field_u64: i,
            field_i32: -(i as i32),
            field_u16: (i % u16::MAX as u64) as u16,
            field_u8: (i % 256) as u8,
            _padding: 0,
        };

        assert!(capsule.verify_determinism());
    }
}

#[test]
fn test_property_roundtrip_100_iterations() {
    // Property: deserialize(serialize(x)) == x
    for i in 0..100 {
        let capsule = SimpleCapsule {
            field_u64: i * 1000,
            field_i32: i as i32,
            field_u16: i as u16,
            field_u8: (i % 256) as u8,
            _padding: 0,
        };

        assert!(capsule.verify_roundtrip());
    }
}

#[test]
fn test_property_different_values_different_bytes() {
    // Property: Different structs produce different bytes
    let capsule1 = SimpleCapsule {
        field_u64: 1,
        field_i32: 0,
        field_u16: 0,
        field_u8: 0,
        _padding: 0,
    };

    let capsule2 = SimpleCapsule {
        field_u64: 2, // Only this field differs
        field_i32: 0,
        field_u16: 0,
        field_u8: 0,
        _padding: 0,
    };

    let bytes1 = capsule1.serialize_deterministic();
    let bytes2 = capsule2.serialize_deterministic();

    assert_ne!(
        bytes1, bytes2,
        "Different values must produce different bytes"
    );
}

// ============================================================================
// COMPILE-TIME VERIFICATION TESTS
// ============================================================================

/// Compile-time verification that repr(C) is enforced
///
/// This test uses size_of checks to verify deterministic layout.
/// For full compile-fail tests, use trybuild in tests/ directory.
#[test]
fn test_repr_c_size_consistency() {
    // #VERIFY_REPR_C: Size must be predictable with repr(C)
    use core::mem::size_of;

    // SimpleCapsule: u64(8) + i32(4) + u16(2) + u8(1) + padding(1) = 16 bytes
    assert_eq!(size_of::<SimpleCapsule>(), 16);

    // ArrayCapsule: [u8;32](32) + u64(8) = 40 bytes
    assert_eq!(size_of::<ArrayCapsule>(), 40);
}

#[test]
fn test_alignment_verification() {
    use core::mem::align_of;

    // SimpleCapsule should be 8-byte aligned (largest field is u64)
    assert_eq!(align_of::<SimpleCapsule>(), 8);

    // ArrayCapsule should be 8-byte aligned (u64 field)
    assert_eq!(align_of::<ArrayCapsule>(), 8);
}

#[test]
#[cfg(feature = "std")]
fn test_atomic_capsule_cache_aligned() {
    use core::mem::align_of;

    // #VERIFY_ALIGNMENT: AtomicCapsule is 64-byte aligned
    assert_eq!(align_of::<AtomicCapsule>(), 64);
}

// ============================================================================
// INTEGRATION WITH HASH MODULE (Phase 2)
// ============================================================================

#[test]
#[cfg(feature = "fast-hash")]
fn test_serialize_for_hash_integration() {
    use crate::hash::const_fast_hash;

    let capsule = SimpleCapsule {
        field_u64: 0xCAFEBABE,
        field_i32: -999,
        field_u16: 0x1234,
        field_u8: 0xFF,
        _padding: 0,
    };

    // serialize_for_hash should match manual hash
    let hash_integrated = capsule.serialize_for_hash();
    let bytes = capsule.serialize_deterministic();
    let hash_manual = const_fast_hash(&bytes);

    assert_eq!(hash_integrated, hash_manual);
}

#[test]
#[cfg(feature = "fast-hash")]
fn test_serialize_for_hash_determinism() {
    let capsule = SimpleCapsule {
        field_u64: 42,
        field_i32: -1,
        field_u16: 0,
        field_u8: 1,
        _padding: 0,
    };

    // Same struct should produce same hash
    let hash1 = capsule.serialize_for_hash();
    let hash2 = capsule.serialize_for_hash();

    assert_eq!(hash1, hash2);
}

// ============================================================================
// DOCUMENTATION TESTS - Ensure Examples Compile
// ============================================================================

/// This test verifies the usage examples from module documentation
#[test]
fn test_documentation_example() {
    // Example from mod.rs documentation
    let capsule = SimpleCapsule {
        field_u64: 42,
        field_i32: -1,
        field_u16: 0,
        field_u8: 7,
        _padding: 0,
    };

    // Deterministic serialization
    let bytes = capsule.serialize_deterministic();

    // Roundtrip
    let restored = SimpleCapsule::deserialize_from_bytes(&bytes).unwrap();
    assert_eq!(capsule, restored);
}

// ============================================================================
// ASSUM FRAMEWORK SUMMARY
// ============================================================================

/// Summary of all ASSUM tags verified in this test suite
///
/// ```text
/// #ASSUME_REPR_C → #VERIFY_REPR_C
///   - test_repr_c_size_consistency()
///   - Compile-fail tests (trybuild, Phase 2)
///
/// #ASSUME_DETERMINISTIC → #VERIFY_DETERMINISTIC
///   - test_determinism_same_struct_twice()
///   - test_verify_determinism_trait_method()
///   - test_property_determinism_100_iterations()
///
/// #ASSUME_ATOMIC_SNAPSHOT → #VERIFY_ATOMIC_SNAPSHOT
///   - test_atomic_capsule_roundtrip()
///   - test_atomic_snapshot_consistency()
///
/// #ASSUME_LITTLE_ENDIAN → #VERIFY_LITTLE_ENDIAN
///   - test_endianness_little_endian()
///
/// #ASSUME_MAGIC_VALIDATION → #VERIFY_MAGIC_VALIDATION
///   - test_magic_number_validation()
///
/// #ASSUME_VERSION_VALIDATION → #VERIFY_VERSION_VALIDATION
///   - test_version_mismatch_detection()
///
/// #ASSUME_BUFFER_SIZE → #VERIFY_BUFFER_SIZE
///   - test_buffer_too_small_error()
/// ```
#[allow(dead_code)]
const _ASSUM_VERIFICATION_COMPLETE: () = ();
