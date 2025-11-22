//! # CapsuleSerialize Basic Usage Example
//!
//! Demonstrates Phase 1 implementations for primitives, arrays, and tuples.

use atomic_capsule::serialize::CapsuleSerialize;

fn main() {
    println!("=== CapsuleSerialize Phase 1 Examples ===\n");

    // ========================================================================
    // Example 1: Primitive Types
    // ========================================================================

    println!("1. Primitive Types");
    println!("------------------");

    // u64 serialization
    let value = 0x0123456789ABCDEF_u64;
    let bytes = value.serialize_deterministic();
    let restored = u64::deserialize_from_bytes(&bytes).unwrap();

    println!("u64 value: 0x{:016X}", value);
    println!("Serialized: {} bytes", bytes.len());
    println!("Restored:  0x{:016X}", restored);
    println!("Match: {}", value == restored);
    println!();

    // i64 serialization
    let value = -9223372036854775808_i64; // i64::MIN
    let bytes = value.serialize_deterministic();
    let restored = i64::deserialize_from_bytes(&bytes).unwrap();

    println!("i64 value: {}", value);
    println!("Serialized: {} bytes", bytes.len());
    println!("Restored:  {}", restored);
    println!("Match: {}", value == restored);
    println!();

    // bool serialization
    let value = true;
    let bytes = value.serialize_deterministic();
    let restored = bool::deserialize_from_bytes(&bytes).unwrap();

    println!("bool value: {}", value);
    println!("Serialized: {} bytes", bytes.len());
    println!("Restored:  {}", restored);
    println!("Match: {}", value == restored);
    println!();

    // ========================================================================
    // Example 2: Fixed Arrays
    // ========================================================================

    println!("2. Fixed Arrays");
    println!("---------------");

    // [u8; 8] serialization
    let value = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let bytes = value.serialize_deterministic();
    let restored = <[u8; 8]>::deserialize_from_bytes(&bytes).unwrap();

    println!("Array [u8; 8]: {:?}", value);
    println!("Serialized: {} bytes", bytes.len());
    println!("Restored:  {:?}", restored);
    println!("Match: {}", value == restored);
    println!();

    // [u8; 64] serialization
    let mut value = [0u8; 64];
    for (i, byte) in value.iter_mut().enumerate() {
        *byte = ((i * 7) % 256) as u8;
    }
    let bytes = value.serialize_deterministic();
    let restored = <[u8; 64]>::deserialize_from_bytes(&bytes).unwrap();

    println!("Array [u8; 64]: {} bytes", value.len());
    println!("Serialized: {} bytes", bytes.len());
    println!("Match: {}", value == restored);
    println!();

    // ========================================================================
    // Example 3: Tuples
    // ========================================================================

    println!("3. Tuples");
    println!("---------");

    // 2-tuple
    let value = (42_u64, 0xDEADBEEF_u32);
    let bytes = value.serialize_deterministic();
    let restored = <(u64, u32)>::deserialize_from_bytes(&bytes).unwrap();

    println!("Tuple (u64, u32): ({}, 0x{:08X})", value.0, value.1);
    println!("Serialized: {} bytes", bytes.len());
    println!("Restored:  ({}, 0x{:08X})", restored.0, restored.1);
    println!("Match: {}", value == restored);
    println!();

    // 3-tuple with mixed types
    let value = (42_u64, -123_i32, true);
    let bytes = value.serialize_deterministic();
    let restored = <(u64, i32, bool)>::deserialize_from_bytes(&bytes).unwrap();

    println!(
        "Tuple (u64, i32, bool): ({}, {}, {})",
        value.0, value.1, value.2
    );
    println!("Serialized: {} bytes", bytes.len());
    println!(
        "Restored:  ({}, {}, {})",
        restored.0, restored.1, restored.2
    );
    println!("Match: {}", value == restored);
    println!();

    // ========================================================================
    // Example 4: Determinism Verification
    // ========================================================================

    println!("4. Determinism Verification");
    println!("---------------------------");

    let value = 0xFEDCBA9876543210_u64;
    let bytes1 = value.serialize_deterministic();
    let bytes2 = value.serialize_deterministic();

    println!("Value: 0x{:016X}", value);
    println!("Serialize twice:");
    println!("  Bytes 1: {} bytes", bytes1.len());
    println!("  Bytes 2: {} bytes", bytes2.len());
    println!("  Match: {}", bytes1 == bytes2);
    println!("  verify_determinism(): {}", value.verify_determinism());
    println!();

    // ========================================================================
    // Example 5: Roundtrip Verification
    // ========================================================================

    println!("5. Roundtrip Verification");
    println!("-------------------------");

    let value = [0xFF_u8; 16];
    let bytes = value.serialize_deterministic();
    let restored = <[u8; 16]>::deserialize_from_bytes(&bytes).unwrap();

    println!("Original:  {:?}", &value[..4]); // Show first 4 bytes
    println!("Restored:  {:?}", &restored[..4]);
    println!("Full match: {}", value == restored);
    println!("verify_roundtrip(): {}", value.verify_roundtrip());
    println!();

    // ========================================================================
    // Example 6: Error Handling
    // ========================================================================

    println!("6. Error Handling");
    println!("-----------------");

    // Buffer too small
    let bytes = vec![0u8; 5]; // Too small for u64 (needs 14 bytes)
    match u64::deserialize_from_bytes(&bytes) {
        Ok(_) => println!("ERROR: Should have failed!"),
        Err(e) => println!("Buffer too small: {}", e),
    }

    // Invalid magic
    let mut bytes = 42_u64.serialize_deterministic();
    bytes[0] = 0xFF; // Corrupt magic
    match u64::deserialize_from_bytes(&bytes) {
        Ok(_) => println!("ERROR: Should have failed!"),
        Err(e) => println!("Invalid magic: {}", e),
    }

    // Version mismatch
    let mut bytes = 42_u64.serialize_deterministic();
    bytes[4] = 99; // Invalid version
    match u64::deserialize_from_bytes(&bytes) {
        Ok(_) => println!("ERROR: Should have failed!"),
        Err(e) => println!("Version mismatch: {}", e),
    }
    println!();

    // ========================================================================
    // Example 7: Serialized Sizes
    // ========================================================================

    println!("7. Serialized Sizes");
    println!("-------------------");

    println!(
        "u64:       {} bytes (4 magic + 2 version + 8 data)",
        u64::serialized_size()
    );
    println!(
        "u32:       {} bytes (4 magic + 2 version + 4 data)",
        u32::serialized_size()
    );
    println!(
        "bool:      {} bytes (4 magic + 2 version + 1 data)",
        bool::serialized_size()
    );
    println!(
        "[u8; 8]:   {} bytes (4 magic + 2 version + 8 data)",
        <[u8; 8]>::serialized_size()
    );
    println!(
        "[u8; 64]:  {} bytes (4 magic + 2 version + 64 data)",
        <[u8; 64]>::serialized_size()
    );
    println!();

    println!("=== All Examples Complete ===");
}
