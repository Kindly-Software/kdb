//! Fixed-Point Arithmetic Guide
//!
//! Demonstrates canonical usage of fixed-point utilities to replace
//! scattered implementations across the codebase.
//!
//! **UCE33 Framework**:
//! - Q10: Tier 3 (Fixed-Point Computational Capsule)
//! - Q28: Simplicity (generic FixedPoint<INT, FRAC>)
//! - Q33: Validation (property tests verify accuracy)

use atomic_capsule::primitives::fixed_point::{q16_16, FixedPoint, Q16_16, Q32_32, Q48_16, Q8_8};

fn main() {
    println!("Fixed-Point Arithmetic Guide");
    println!("============================\n");

    example_1_basic_usage();
    example_2_financial_calculations();
    example_3_helper_modules();
    example_4_arithmetic_operations();
    example_5_precision_comparison();
    example_6_atomic_integration();
}

/// Example 1: Basic usage
fn example_1_basic_usage() {
    println!("Example 1: Basic Usage");
    println!("----------------------");

    // Create from f64
    let price = Q16_16::from_f64(123.45);
    println!("Price (Q16.16): {}", price);

    // Convert back to f64
    let price_f64 = price.to_f64();
    println!("Price (f64): {}", price_f64);

    // Create from integer
    let quantity = Q16_16::from_int(10);
    println!("Quantity: {}", quantity);

    println!();
}

/// Example 2: Financial calculations (P&L tracking)
fn example_2_financial_calculations() {
    println!("Example 2: Financial Calculations (P&L)");
    println!("----------------------------------------");

    // Use Q16.16 for dollar amounts with good precision
    let entry_price = Q16_16::from_f64(100.50);
    let exit_price = Q16_16::from_f64(105.75);
    let quantity = Q16_16::from_f64(100.0);

    // Calculate P&L: (exit - entry) * quantity
    let price_diff = exit_price.saturating_sub(entry_price);
    let pnl = price_diff.saturating_mul(quantity);

    println!("Entry Price: ${}", entry_price);
    println!("Exit Price: ${}", exit_price);
    println!("Quantity: {}", quantity);
    println!("P&L: ${:.2}", pnl.to_f64());
    println!();
}

/// Example 3: Using helper modules
fn example_3_helper_modules() {
    println!("Example 3: Helper Modules");
    println!("-------------------------");

    // q16_16 helper module for convenient conversion
    let price = q16_16::from_f64(50.25);
    println!("Price (via helper): {}", q16_16::to_f64(price));

    // Helper provides scale factor constant
    println!("Q16.16 scale factor: {}", q16_16::SCALE);
    println!();
}

/// Example 4: Arithmetic operations
fn example_4_arithmetic_operations() {
    println!("Example 4: Arithmetic Operations");
    println!("---------------------------------");

    let a = Q16_16::from_f64(100.0);
    let b = Q16_16::from_f64(25.0);

    // Addition
    let sum = a + b;
    println!("{} + {} = {}", a, b, sum);

    // Subtraction
    let diff = a - b;
    println!("{} - {} = {}", a, b, diff);

    // Multiplication
    let product = a * b;
    println!("{} * {} = {}", a, b, product);

    // Division
    let quotient = a / b;
    println!("{} / {} = {}", a, b, quotient);

    // Saturating operations (prevent overflow)
    let max = Q16_16::MAX;
    let one = Q16_16::ONE;
    let sum_saturating = max.saturating_add(one);
    println!("MAX + 1 (saturating) = {}", sum_saturating.to_f64());

    println!();
}

/// Example 5: Precision comparison
fn example_5_precision_comparison() {
    println!("Example 5: Precision Comparison");
    println!("--------------------------------");

    let value = 0.123456789;

    // Q8.8: Low precision, small range
    let q8 = Q8_8::from_f64(value);
    let error_q8 = (q8.to_f64() - value).abs();
    println!("Q8.8:   {} (error: {:.9})", q8.to_f64(), error_q8);

    // Q16.16: Good precision, moderate range
    let q16 = Q16_16::from_f64(value);
    let error_q16 = (q16.to_f64() - value).abs();
    println!("Q16.16: {} (error: {:.9})", q16.to_f64(), error_q16);

    // Q32.32: High precision, large range
    let q32 = Q32_32::from_f64(value);
    let error_q32 = (q32.to_f64() - value).abs();
    println!("Q32.32: {} (error: {:.9})", q32.to_f64(), error_q32);

    println!();
}

/// Example 6: Integration with atomic operations
fn example_6_atomic_integration() {
    use std::sync::atomic::{AtomicI64, Ordering};

    println!("Example 6: Atomic Integration");
    println!("------------------------------");

    // Store fixed-point value in atomic
    let pnl_atomic = AtomicI64::new(0);

    // Convert trade P&L to fixed-point and update atomically
    let trade_pnl = Q16_16::from_f64(123.45);
    pnl_atomic.fetch_add(trade_pnl.to_raw(), Ordering::Relaxed);

    // Add another trade
    let trade2_pnl = Q16_16::from_f64(67.89);
    pnl_atomic.fetch_add(trade2_pnl.to_raw(), Ordering::Relaxed);

    // Read back total P&L
    let total_raw = pnl_atomic.load(Ordering::Relaxed);
    let total_pnl = Q16_16::from_raw(total_raw);

    println!("Trade 1 P&L: ${:.2}", trade_pnl.to_f64());
    println!("Trade 2 P&L: ${:.2}", trade2_pnl.to_f64());
    println!("Total P&L: ${:.2}", total_pnl.to_f64());
    println!();
}

/// Bonus: Choosing the right precision
#[allow(dead_code)]
fn choosing_precision_guide() {
    println!("Choosing the Right Precision");
    println!("============================\n");

    println!("Q8.8 (8 integer bits, 8 fractional bits)");
    println!("  Range: -128.0 to 127.996");
    println!("  Precision: 1/256 ≈ 0.0039 (~0.4 basis points)");
    println!("  Use case: Basis points, small percentages, ratios");
    println!("  Example: Risk adjustment factors (0.5× to 2.0×)");
    println!();

    println!("Q16.16 (16 integer bits, 16 fractional bits)");
    println!("  Range: -32768.0 to 32767.999");
    println!("  Precision: 1/65536 ≈ 0.000015 (~0.15 basis points)");
    println!("  Use case: Prices, P&L, VWAP, most financial calculations");
    println!("  Example: Stock/futures prices ($10 - $10,000)");
    println!();

    println!("Q32.32 (32 integer bits, 32 fractional bits)");
    println!("  Range: ±2.1 billion");
    println!("  Precision: 1/4.3 billion ≈ 2.3e-10");
    println!("  Use case: High-precision scientific calculations");
    println!("  Example: Mathematical constants, physics simulations");
    println!();

    println!("Q48.16 (48 integer bits, 16 fractional bits)");
    println!("  Range: ±140 trillion");
    println!("  Precision: 1/65536 ≈ 0.000015");
    println!("  Use case: Large dollar amounts with decent precision");
    println!("  Example: Portfolio value ($1M - $1T)");
    println!();
}

/// Migration example: Before/After
#[allow(dead_code)]
fn migration_example() {
    println!("Migration Example");
    println!("=================\n");

    println!("BEFORE (manual fixed-point):");
    println!("----------------------------");
    println!("```rust");
    println!("const FIXED_POINT_SCALE: i64 = 256;");
    println!();
    println!("fn f64_to_fixed(value: f64) -> u64 {{");
    println!("    let scaled = value * (1u64 << 32) as f64;");
    println!("    scaled as i64 as u64");
    println!("}}");
    println!();
    println!("fn fixed_to_f64(fixed: u64) -> f64 {{");
    println!("    (fixed as i64) as f64 / (1u64 << 32) as f64");
    println!("}}");
    println!("```");
    println!();

    println!("AFTER (canonical fixed-point):");
    println!("------------------------------");
    println!("```rust");
    println!("use atomic_capsule::primitives::fixed_point::Q32_32;");
    println!();
    println!("let fixed = Q32_32::from_f64(value);");
    println!("let recovered = fixed.to_f64();");
    println!("```");
    println!();

    println!("Benefits:");
    println!("  - Type-safe: Q8.8 vs Q16.16 vs Q32.32 enforced at compile-time");
    println!("  - Tested: Property tests validate conversion accuracy");
    println!("  - Reusable: No duplication across capsules");
    println!("  - Arithmetic: Built-in operations (add, sub, mul, div)");
    println!("  - Safe: Saturating semantics prevent overflow panics");
}
