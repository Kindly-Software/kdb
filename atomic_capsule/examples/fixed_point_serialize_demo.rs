//! Fixed-Point Serialization Demo
//!
//! Demonstrates FixedPointSerialize trait usage for Q16.16, Q8.8, and Q32.32 formats.

use atomic_capsule::serialize::fixed_point_serialize::*;

fn main() {
    println!("=== Fixed-Point Serialization Demo ===\n");

    // ========================================================================
    // Q16.16 Financial Standard (16 integer bits, 16 fractional bits)
    // ========================================================================
    println!("--- Q16.16 (Financial Standard) ---");

    let payment = FixedQ16_16::from_decimal(1234, 5678); // 1234.5678
    println!("Original value: {}", payment);

    // Binary serialization (exact i64)
    let raw = payment.serialize_raw();
    println!("Raw i64: 0x{:016X} ({})", raw, raw);

    // Decimal serialization (human-readable)
    let decimal = payment.serialize_decimal();
    println!("Decimal string: {}", decimal);

    // Roundtrip verification
    let restored = FixedQ16_16::deserialize_from_raw(raw);
    println!("Roundtrip: {} (matches: {})", restored, payment == restored);
    assert!(payment.verify_roundtrip());
    assert!(payment.verify_decimal_determinism());

    // Binary format with checksum
    #[cfg(feature = "std")]
    {
        let bytes = serialize_to_binary(&payment);
        println!("Binary format: {} bytes", bytes.len());

        let restored_binary: FixedQ16_16 = deserialize_from_binary(&bytes).unwrap();
        println!(
            "Binary roundtrip: {} (matches: {})\n",
            restored_binary,
            payment == restored_binary
        );
    }

    // ========================================================================
    // Q8.8 Fast Arithmetic (8 integer bits, 8 fractional bits)
    // ========================================================================
    println!("--- Q8.8 (Fast Arithmetic) ---");

    let fast_value = FixedQ8_8::from_decimal(12, 34); // 12.34
    println!("Original value: {}", fast_value);
    println!("Raw i64: 0x{:016X}", fast_value.serialize_raw());
    println!("Decimal string: {}", fast_value.serialize_decimal());
    assert!(fast_value.verify_roundtrip());
    println!();

    // ========================================================================
    // Q32.32 High Precision (32 integer bits, 32 fractional bits)
    // ========================================================================
    println!("--- Q32.32 (High Precision) ---");

    let precise = FixedQ32_32::from_decimal(1234, 567890123); // 1234.567890123
    println!("Original value: {}", precise);
    println!("Raw i64: 0x{:016X}", precise.serialize_raw());
    println!("Decimal string: {}", precise.serialize_decimal());
    assert!(precise.verify_roundtrip());
    println!();

    // ========================================================================
    // Negative Values
    // ========================================================================
    println!("--- Negative Values ---");

    let negative = FixedQ16_16::from_decimal(-999, 9999);
    println!("Negative Q16.16: {}", negative);
    println!("Raw i64: 0x{:016X}", negative.serialize_raw());
    println!("Decimal: {}", negative.serialize_decimal());
    assert!(negative.verify_roundtrip());
    println!();

    // ========================================================================
    // Edge Cases
    // ========================================================================
    println!("--- Edge Cases ---");

    let zero = FixedQ16_16::from_decimal(0, 0);
    println!("Zero: {} (raw: 0x{:016X})", zero, zero.serialize_raw());

    let max_fractional = FixedQ16_16::from_decimal(0, 9999);
    println!(
        "Max fractional: {} (raw: 0x{:016X})",
        max_fractional,
        max_fractional.serialize_raw()
    );

    println!("\n=== All Tests Passed ===");
}
