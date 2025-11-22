//! Standalone test to verify fixed-point implementation
//! This bypasses the existing compile errors in the codebase

// Directly include the fixed_point module code
#[path = "../src/primitives/fixed_point.rs"]
mod fixed_point;

use fixed_point::{q16_16, FixedPoint, Q16_16, Q32_32, Q48_16, Q8_8};

fn main() {
    println!("Fixed-Point Arithmetic Standalone Test");
    println!("=======================================\n");

    test_basic_conversion();
    test_arithmetic();
    test_financial_pattern();
    test_precision_comparison();

    println!("\n✓ All tests passed!");
}

fn test_basic_conversion() {
    println!("Test 1: Basic Conversion");
    println!("-----------------------");

    let value = Q16_16::from_f64(123.45);
    let recovered = value.to_f64();
    let error = (recovered - 123.45).abs();

    println!("  Original: 123.45");
    println!("  Recovered: {}", recovered);
    println!("  Error: {:.9}", error);

    assert!(error < 0.001, "Conversion error too large");
    println!("  ✓ Passed\n");
}

fn test_arithmetic() {
    println!("Test 2: Arithmetic Operations");
    println!("-----------------------------");

    let a = Q16_16::from_f64(100.0);
    let b = Q16_16::from_f64(25.0);

    let sum = a + b;
    let diff = a - b;
    let product = a * b;
    let quotient = a / b;

    println!("  {} + {} = {}", a, b, sum);
    println!("  {} - {} = {}", a, b, diff);
    println!("  {} * {} = {}", a, b, product);
    println!("  {} / {} = {}", a, b, quotient);

    assert!((sum.to_f64() - 125.0).abs() < 0.001);
    assert!((diff.to_f64() - 75.0).abs() < 0.001);
    assert!((product.to_f64() - 2500.0).abs() < 0.1);
    assert!((quotient.to_f64() - 4.0).abs() < 0.001);

    println!("  ✓ Passed\n");
}

fn test_financial_pattern() {
    println!("Test 3: Financial Pattern (P&L)");
    println!("--------------------------------");

    let entry_price = Q16_16::from_f64(100.50);
    let exit_price = Q16_16::from_f64(105.75);
    let quantity = Q16_16::from_f64(100.0);

    let price_diff = exit_price.saturating_sub(entry_price);
    let pnl = price_diff.saturating_mul(quantity);

    println!("  Entry: ${}", entry_price);
    println!("  Exit: ${}", exit_price);
    println!("  Quantity: {}", quantity);
    println!("  P&L: ${:.2}", pnl.to_f64());

    let expected_pnl = (105.75 - 100.50) * 100.0;
    assert!((pnl.to_f64() - expected_pnl).abs() < 0.01);

    println!("  ✓ Passed\n");
}

fn test_precision_comparison() {
    println!("Test 4: Precision Comparison");
    println!("----------------------------");

    let value = 0.123456789;

    let q8 = Q8_8::from_f64(value);
    let error_q8 = (q8.to_f64() - value).abs();

    let q16 = Q16_16::from_f64(value);
    let error_q16 = (q16.to_f64() - value).abs();

    let q32 = Q32_32::from_f64(value);
    let error_q32 = (q32.to_f64() - value).abs();

    println!("  Original: {}", value);
    println!("  Q8.8:   {} (error: {:.9})", q8.to_f64(), error_q8);
    println!("  Q16.16: {} (error: {:.9})", q16.to_f64(), error_q16);
    println!("  Q32.32: {} (error: {:.9})", q32.to_f64(), error_q32);

    assert!(error_q32 < error_q16);
    assert!(error_q16 < error_q8);

    println!("  ✓ Passed (Q32.32 < Q16.16 < Q8.8)\n");
}
