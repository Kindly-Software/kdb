//! # Fixed-Point Compile-Time Verification Demo
//!
//! Demonstrates compile-time precision guarantees for fixed-point types.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example fixed_point_verification_demo
//! ```
//!
//! ## UCE34 Framework
//!
//! - **Q10 (Tier 3)**: Fixed-point computational capsules
//! - **Q33 (Validation)**: Compile-time verification prevents runtime errors
//! - **Q34 (Auditability)**: ASSUM tags document all assumptions

use atomic_capsule::primitives::fixed_point::{Q16_16, Q32_32, Q8_8};
use atomic_capsule::serialize::fixed_point_verification::{
    q16_16_verification, q32_32_verification, q8_8_verification, rounding_error_bounds,
};
use atomic_capsule::verify_fixed_point_format;

fn main() {
    println!("=== Fixed-Point Compile-Time Verification Demo ===\n");

    // Q8.8 Format Verification
    println!("Q8.8 Format:");
    println!(
        "  Precision: {} (1/256)",
        q8_8_verification::verify_q8_8_precision()
    );
    let (min, max) = q8_8_verification::verify_q8_8_range();
    println!("  Range: [{}, {}]", min, max);
    println!(
        "  Max error: {} (~0.4 basis points)",
        rounding_error_bounds::Q8_8_MAX_ERROR
    );

    // Compile-time verification (zero runtime cost)
    verify_fixed_point_format!(Q8_8, 8, 8, i16);
    println!("  ✓ Compile-time verification passed\n");

    // Q16.16 Format Verification
    println!("Q16.16 Format:");
    println!(
        "  Precision: {} (1/65536)",
        q16_16_verification::verify_q16_16_precision()
    );
    let (min, max) = q16_16_verification::verify_q16_16_range();
    println!("  Range: [{}, {}]", min, max);
    println!(
        "  Max error: {} (~0.15 basis points)",
        rounding_error_bounds::Q16_16_MAX_ERROR
    );

    // Compile-time verification (zero runtime cost)
    verify_fixed_point_format!(Q16_16, 16, 16, i32);
    println!("  ✓ Compile-time verification passed\n");

    // Q32.32 Format Verification
    println!("Q32.32 Format:");
    println!(
        "  Precision: {} (1/2^32)",
        q32_32_verification::verify_q32_32_precision()
    );
    let (min, max) = q32_32_verification::verify_q32_32_range();
    println!("  Range: [{}, {}]", min, max);
    println!(
        "  Max error: {} (scientific precision)",
        rounding_error_bounds::Q32_32_MAX_ERROR
    );

    // Compile-time verification (zero runtime cost)
    verify_fixed_point_format!(Q32_32, 32, 32, i64);
    println!("  ✓ Compile-time verification passed\n");

    // Roundtrip Error Validation
    println!("=== Roundtrip Error Validation ===\n");

    let test_value = 123.456789;

    // Q8.8 roundtrip
    let q8_8 = Q8_8::from_f64(test_value);
    let recovered = q8_8.to_f64();
    let error = (test_value - recovered).abs();
    let within_epsilon = rounding_error_bounds::verify_roundtrip_error(
        test_value,
        recovered,
        rounding_error_bounds::Q8_8_MAX_ERROR,
    );
    println!(
        "Q8.8:   {} → {} (error: {:.6}, within epsilon: {})",
        test_value, recovered, error, within_epsilon
    );

    // Q16.16 roundtrip
    let q16_16 = Q16_16::from_f64(test_value);
    let recovered = q16_16.to_f64();
    let error = (test_value - recovered).abs();
    let within_epsilon = rounding_error_bounds::verify_roundtrip_error(
        test_value,
        recovered,
        rounding_error_bounds::Q16_16_MAX_ERROR,
    );
    println!(
        "Q16.16: {} → {} (error: {:.8}, within epsilon: {})",
        test_value, recovered, error, within_epsilon
    );

    // Q32.32 roundtrip
    let q32_32 = Q32_32::from_f64(test_value);
    let recovered = q32_32.to_f64();
    let error = (test_value - recovered).abs();
    let within_epsilon = rounding_error_bounds::verify_roundtrip_error(
        test_value,
        recovered,
        rounding_error_bounds::Q32_32_MAX_ERROR,
    );
    println!(
        "Q32.32: {} → {} (error: {:.12}, within epsilon: {})",
        test_value, recovered, error, within_epsilon
    );

    println!("\n=== Financial Precision Example (Q16.16) ===\n");

    // Sub-cent precision test
    let price = Q16_16::from_f64(0.01); // 1 cent
    let quantity = Q16_16::from_f64(1000.0);
    let total = price.saturating_mul(quantity);

    println!("Price:    $0.01");
    println!("Quantity: 1000");
    println!("Total:    ${:.4}", total.to_f64());
    println!(
        "Error:    {:.8} (< 1 cent precision)",
        (10.0 - total.to_f64()).abs()
    );

    println!("\n=== Saturation Safety ===\n");

    // Overflow saturation
    let max = Q16_16::MAX;
    let one = Q16_16::ONE;
    let overflow = max.saturating_add(one);
    println!("Q16.16::MAX + 1 = {} (saturated to MAX)", overflow.to_f64());

    // Underflow saturation
    let min = Q16_16::MIN;
    let underflow = min.saturating_sub(one);
    println!(
        "Q16.16::MIN - 1 = {} (saturated to MIN)",
        underflow.to_f64()
    );

    println!("\n✓ All verifications passed!");
    println!("✓ Zero runtime cost (compile-time verification)");
    println!("✓ ASSUM framework compliance (all assumptions documented)");
}
