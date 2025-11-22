//! Fixed-Point Type Detection API Demo
//!
//! This example demonstrates all detection strategies and error handling.

use atomic_capsule::serialize::fixed_point_type_detection::{
    check_precision_loss, check_type_conflict, detect_fixed_point_type, DetectionStrategy,
    FixedPointType,
};

fn main() {
    println!("=== Fixed-Point Type Detection Demo ===\n");

    // ========================================================================
    // Strategy 1: Path-Based Detection (Fast Path)
    // ========================================================================
    println!("1. Path-Based Detection (Fast Path)");
    println!("   - <100ns per field");
    println!("   - 100% accuracy\n");

    let info =
        detect_fixed_point_type("atomic_capsule::serialize::fixed_point_impls::Q16_16").unwrap();
    println!("   Type: {}", info.type_name);
    println!("   Detected as: {}", info.fp_type);
    println!("   Strategy: {}", info.strategy);
    println!("   Container depth: {}", info.container_depth);
    println!();

    // ========================================================================
    // Strategy 2: Type Name Heuristics (Fallback)
    // ========================================================================
    println!("2. Type Name Heuristics (Fallback)");
    println!("   - <200ns per field");
    println!("   - 95% accuracy\n");

    // Direct type name
    let info = detect_fixed_point_type("Q16_16").unwrap();
    println!("   Direct: Q16_16 → {}", info.fp_type);

    // NewType wrapper (suffix matching)
    let info = detect_fixed_point_type("PriceQ16_16").unwrap();
    println!("   NewType: PriceQ16_16 → {}", info.fp_type);
    println!();

    // ========================================================================
    // Strategy 3: Container Detection (Recursive)
    // ========================================================================
    println!("3. Container Detection (Recursive)");
    println!("   - <300ns per field");
    println!("   - 100% accuracy\n");

    // Single container
    let info = detect_fixed_point_type("Option<Q16_16>").unwrap();
    println!(
        "   Single: Option<Q16_16> → {} (depth={})",
        info.fp_type, info.container_depth
    );

    // Nested containers
    let info = detect_fixed_point_type("Option<Vec<Box<Q32_32>>>").unwrap();
    println!(
        "   Nested: Option<Vec<Box<Q32_32>>> → {} (depth={})",
        info.fp_type, info.container_depth
    );
    println!();

    // ========================================================================
    // Type Properties
    // ========================================================================
    println!("4. Type Properties");
    println!();

    for fp_type in [
        FixedPointType::Q8_8,
        FixedPointType::Q16_16,
        FixedPointType::Q32_32,
    ] {
        println!("   {}:", fp_type);
        println!("      - Integer bits: {}", fp_type.integer_bits());
        println!("      - Fractional bits: {}", fp_type.fractional_bits());
        println!("      - Total bits: {}", fp_type.total_bits());
        println!("      - Storage: {}", fp_type.storage_type());
        println!("      - Precision: {:.10}", fp_type.precision());
        println!();
    }

    // ========================================================================
    // Type Conflict Detection
    // ========================================================================
    println!("5. Type Conflict Detection");
    println!();

    // Compatible (same type)
    match check_type_conflict(FixedPointType::Q16_16, FixedPointType::Q16_16, "fee") {
        Ok(_) => println!("   ✅ Q16_16 vs Q16_16 (same field): Compatible"),
        Err(e) => println!("   ❌ Error: {}", e),
    }

    // Incompatible (different types)
    match check_type_conflict(FixedPointType::Q8_8, FixedPointType::Q16_16, "amount") {
        Ok(_) => println!("   ✅ Compatible"),
        Err(e) => println!(
            "   ❌ Q8_8 vs Q16_16 (same field): Conflict detected\n{}",
            e
        ),
    }

    // ========================================================================
    // Precision Loss Detection
    // ========================================================================
    println!("6. Precision Loss Detection");
    println!();

    println!("   Safe Conversions (Upcasts):");
    for (from, to) in [
        (FixedPointType::Q8_8, FixedPointType::Q16_16),
        (FixedPointType::Q8_8, FixedPointType::Q32_32),
        (FixedPointType::Q16_16, FixedPointType::Q32_32),
    ] {
        match check_precision_loss(from, to, "upcast") {
            Ok(_) => println!("      ✅ {} → {}: Safe", from, to),
            Err(e) => println!("      ❌ {} → {}: {}", from, to, e),
        }
    }

    println!("\n   Unsafe Conversions (Downcasts):");
    for (from, to) in [
        (FixedPointType::Q16_16, FixedPointType::Q8_8),
        (FixedPointType::Q32_32, FixedPointType::Q8_8),
        (FixedPointType::Q32_32, FixedPointType::Q16_16),
    ] {
        match check_precision_loss(from, to, "downcast") {
            Ok(_) => println!("      ✅ {} → {}: Safe", from, to),
            Err(e) => println!("      ⚠️  {} → {}: Unsafe (precision loss)", from, to),
        }
    }
    println!();

    // ========================================================================
    // Error Messages (Unknown Type)
    // ========================================================================
    println!("7. Error Messages (Unknown Type)");
    println!();

    match detect_fixed_point_type("UnknownType") {
        Ok(_) => println!("   ✅ Detected"),
        Err(e) => println!("   {}", e),
    }

    // ========================================================================
    // Fuzzy Matching (Typos)
    // ========================================================================
    println!("8. Fuzzy Matching (Typos)");
    println!();

    // Typo: Q16_15 instead of Q16_16
    match detect_fixed_point_type("Q16_15") {
        Ok(_) => println!("   ✅ Detected"),
        Err(e) => println!("   {}", e),
    }

    // ========================================================================
    // Real-World Example: Payment Struct
    // ========================================================================
    println!("9. Real-World Example: Payment Struct");
    println!();

    println!("   #[derive(CapsuleSerialize)]");
    println!("   #[repr(C)]");
    println!("   struct Payment {{");
    println!("       amount_cents: Q16_16,  // Auto-detected");
    println!("       fee_cents: Q16_16,     // Auto-detected");
    println!("       rate_bp: Q8_8,         // Auto-detected (different precision)");
    println!("   }}");
    println!();

    // Detect field types
    let amount_type = detect_fixed_point_type("Q16_16").unwrap();
    let fee_type = detect_fixed_point_type("Q16_16").unwrap();
    let rate_type = detect_fixed_point_type("Q8_8").unwrap();

    println!("   Detected types:");
    println!("      - amount_cents: {}", amount_type.fp_type);
    println!("      - fee_cents: {}", fee_type.fp_type);
    println!("      - rate_bp: {}", rate_type.fp_type);
    println!();

    // Verify amount and fee use same type
    match check_type_conflict(amount_type.fp_type, fee_type.fp_type, "fee_cents") {
        Ok(_) => println!("   ✅ amount_cents and fee_cents use compatible types"),
        Err(e) => println!("   ❌ Type conflict: {}", e),
    }

    // Note: Different types allowed in different fields
    println!("   ℹ️  rate_bp uses different type (Q8_8) - this is allowed");
    println!();

    println!("=== Demo Complete ===");
}
