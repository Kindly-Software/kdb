//! Test example for fixed_point_impls module
//!
//! Verifies that Q8_8, Q16_16, and Q32_32 types work correctly.

#[cfg(feature = "capsule-serialize")]
use atomic_capsule::serialize::fixed_point_impls::{Q16_16, Q32_32, Q8_8};

#[cfg(not(feature = "capsule-serialize"))]
fn main() {
    eprintln!("This example requires the 'capsule-serialize' feature");
    eprintln!("Run with: cargo run --example test_fixed_point_impls --features capsule-serialize");
}

#[cfg(feature = "capsule-serialize")]

fn main() {
    println!("Testing Fixed-Point Implementations\n");

    // Q8.8 Tests
    println!("=== Q8.8 Tests ===");
    let a = Q8_8::from_f64(12.5);
    let b = Q8_8::from_f64(3.25);
    println!("a = {}", a);
    println!("b = {}", b);
    println!("a + b = {}", a.saturating_add(b));
    println!("a - b = {}", a.saturating_sub(b));
    println!("a * b = {}", a.saturating_mul(b));
    println!("a / b = {}", a.div(b));
    println!("Range: {} to {}", Q8_8::MIN, Q8_8::MAX);
    println!();

    // Q16.16 Tests
    println!("=== Q16.16 Tests ===");
    let price = Q16_16::from_f64(123.45);
    let qty = Q16_16::from_f64(10.0);
    println!("price = {}", price);
    println!("qty = {}", qty);
    println!("total = price * qty = {}", price.saturating_mul(qty));
    println!("avg = total / qty = {}", price.saturating_mul(qty).div(qty));
    println!("Range: {} to {}", Q16_16::MIN, Q16_16::MAX);
    println!();

    // Q32.32 Tests
    println!("=== Q32.32 Tests ===");
    let large = Q32_32::from_f64(1_000_000.123456);
    let small = Q32_32::from_f64(0.000001);
    println!("large = {}", large);
    println!("small = {}", small);
    println!("large + small = {}", large.saturating_add(small));
    println!("Range: {} to {}", Q32_32::MIN, Q32_32::MAX);
    println!();

    // Saturation Tests
    println!("=== Saturation Tests ===");
    let max_q16 = Q16_16::MAX;
    let one_q16 = Q16_16::ONE;
    println!("Q16.16 MAX = {}", max_q16);
    println!("Q16.16 MAX + 1 = {}", max_q16.saturating_add(one_q16));
    println!("(Should equal MAX due to saturation)");
    println!();

    // Rounding Modes
    println!("=== Rounding Modes ===");
    let value = 12.7;
    println!("Value: {}", value);
    println!("Truncate: {}", Q16_16::from_f64(value));
    println!("Round: {}", Q16_16::from_f64_round(value));
    println!("Ceil: {}", Q16_16::from_f64_ceil(value));
    println!("Floor: {}", Q16_16::from_f64_floor(value));
    println!();

    // Helper Methods
    println!("=== Helper Methods ===");
    let pos = Q16_16::from_f64(10.0);
    let neg = Q16_16::from_f64(-5.0);
    println!("pos.is_positive() = {}", pos.is_positive());
    println!("neg.is_negative() = {}", neg.is_negative());
    println!("pos.abs() = {}", pos.abs());
    println!("neg.abs() = {}", neg.abs());
    println!("pos.min(neg) = {}", pos.min(neg));
    println!("pos.max(neg) = {}", pos.max(neg));
    println!();

    println!("All tests passed!");
}
