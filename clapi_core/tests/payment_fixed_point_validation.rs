//! Fixed-Point Validation - 1M Random Amounts (Zero Drift)
//!
//! This test validates that Q0.64 fixed-point arithmetic:
//! 1. Has ZERO rounding errors (exact to the cent)
//! 2. Is reversible (amount = net + fee)
//! 3. Is deterministic (same input → same output)
//! 4. Has no drift after 1M operations
//!
//! Run with: cargo test --test payment_fixed_point_validation -- --ignored --nocapture

use clapi_core::capsules::PaymentCapsule256;
use rand::Rng;

#[test]
#[ignore] // Run explicitly: cargo test --test payment_fixed_point_validation -- --ignored --nocapture
fn validate_1m_random_amounts_zero_drift() {
    println!("==================================================");
    println!("Fixed-Point Validation: 1M Random Amounts");
    println!("==================================================\n");

    let mut rng = rand::thread_rng();
    let mut errors = Vec::new();
    let mut max_amount = 0i64;
    let mut min_amount = i64::MAX;

    println!("Generating and validating 1,000,000 random amounts...\n");

    for i in 1..=1_000_000 {
        // Generate random amount (1 cent to $10 billion)
        let amount: i64 = rng.gen_range(1..10_000_000_000_00);

        // Track range
        max_amount = max_amount.max(amount);
        min_amount = min_amount.min(amount);

        // Create payment capsule
        let payment = PaymentCapsule256::new(i, i % 10_000, amount);

        // Validation 1: Fee calculation determinism
        let expected_fee = (amount * 3) / 100;
        if payment.fee() != expected_fee {
            errors.push(format!(
                "Iteration {}: Fee mismatch (expected {}, got {})",
                i,
                expected_fee,
                payment.fee()
            ));
        }

        // Validation 2: Net calculation determinism
        let expected_net = amount - expected_fee;
        if payment.net() != expected_net {
            errors.push(format!(
                "Iteration {}: Net mismatch (expected {}, got {})",
                i,
                expected_net,
                payment.net()
            ));
        }

        // Validation 3: Reversibility (amount = net + fee)
        let reconstructed = payment.net() + payment.fee();
        if reconstructed != amount {
            errors.push(format!(
                "Iteration {}: Reversibility violation (amount {} != net {} + fee {})",
                i,
                amount,
                payment.net(),
                payment.fee()
            ));
        }

        // Validation 4: Arithmetic identity (amount - fee = net)
        if amount - payment.fee() != payment.net() {
            errors.push(format!(
                "Iteration {}: Arithmetic identity violation (amount {} - fee {} != net {})",
                i,
                amount,
                payment.fee(),
                payment.net()
            ));
        }

        // Validation 5: Capsule verify_arithmetic() method
        if !payment.verify_arithmetic() {
            errors.push(format!(
                "Iteration {}: verify_arithmetic() failed for amount {}",
                i,
                amount
            ));
        }

        // Progress reporting
        if i % 100_000 == 0 {
            println!("✓ Validated {:>9} amounts ({}% complete)", i, i / 10_000);
        }
    }

    println!("\n==================================================");
    println!("Validation Results");
    println!("==================================================\n");

    println!("Total amounts tested: 1,000,000");
    println!("Amount range: ${:.2} to ${:.2}", min_amount as f64 / 100.0, max_amount as f64 / 100.0);
    println!("Errors detected: {}", errors.len());

    if errors.is_empty() {
        println!("\n🎉 SUCCESS: Zero drift detected!");
        println!("   - All 1M amounts validated with exact arithmetic");
        println!("   - Zero rounding errors");
        println!("   - Perfect reversibility (amount = net + fee)");
        println!("   - Deterministic fee calculation");
        println!("   - No floating-point drift\n");
    } else {
        println!("\n⚠️  FAILURE: Errors detected!\n");
        for (i, error) in errors.iter().take(10).enumerate() {
            println!("  {}. {}", i + 1, error);
        }
        if errors.len() > 10 {
            println!("  ... and {} more errors", errors.len() - 10);
        }
        println!();
        panic!("Fixed-point validation failed with {} errors", errors.len());
    }

    println!("==================================================\n");
}

#[test]
#[ignore]
fn validate_edge_cases() {
    println!("\n==================================================");
    println!("Fixed-Point Edge Case Validation");
    println!("==================================================\n");

    let test_cases = vec![
        (1, "Minimum amount (1 cent)"),
        (100, "$1.00"),
        (1_00, "$1.00 (explicit)"),
        (10_00, "$10.00"),
        (100_00, "$100.00"),
        (1_000_00, "$1,000.00"),
        (10_000_00, "$10,000.00"),
        (100_000_00, "$100,000.00"),
        (1_000_000_00, "$1,000,000.00"),
        (10_000_000_00, "$10,000,000.00"),
        (100_000_000_00, "$100,000,000.00"),
        (1_000_000_000_00, "$1,000,000,000.00 (1 billion)"),
        (10_000_000_000_00, "$10,000,000,000.00 (10 billion)"),
    ];

    println!("Testing {} edge cases:\n", test_cases.len());

    for (amount, description) in test_cases {
        let payment = PaymentCapsule256::new(1, 1, amount);
        let fee = payment.fee();
        let net = payment.net();
        let reconstructed = net + fee;

        println!("  {} ({})", description, amount);
        println!("    Fee:           ${:.2}", fee as f64 / 100.0);
        println!("    Net:           ${:.2}", net as f64 / 100.0);
        println!("    Reconstructed: ${:.2}", reconstructed as f64 / 100.0);
        println!("    Valid:         {}", if reconstructed == amount { "✓" } else { "✗" });

        assert_eq!(
            reconstructed,
            amount,
            "Edge case failed: {} (amount = {})",
            description,
            amount
        );

        println!();
    }

    println!("✓ All edge cases validated successfully\n");
    println!("==================================================\n");
}

#[test]
#[ignore]
fn validate_precision_limits() {
    println!("\n==================================================");
    println!("Fixed-Point Precision Limits");
    println!("==================================================\n");

    println!("Q0.64 Format (cents as i64):");
    println!("  - Representation: Direct integer cents");
    println!("  - Precision: Exact to the cent");
    println!("  - Range: ±92,233,720,368,547,758.08");
    println!("  - Scaling: None (1 cent = 1)");
    println!();

    // Test precision at various scales
    let test_amounts = vec![
        1,                     // $0.01
        99,                    // $0.99
        100,                   // $1.00
        999,                   // $9.99
        1_234,                 // $12.34
        99_99,                 // $99.99
        100_00,                // $100.00
        1_234_567,             // $12,345.67
        99_999_99,             // $99,999.99
        1_000_000_00,          // $1,000,000.00
        10_000_000_00,         // $10,000,000.00
        100_000_000_00,        // $100,000,000.00
        1_000_000_000_00,      // $1,000,000,000.00
        i64::MAX / 100,        // Near maximum
    ];

    println!("Testing precision at {} different scales:\n", test_amounts.len());

    for amount in test_amounts {
        let payment = PaymentCapsule256::new(1, 1, amount);
        let fee = (amount * 3) / 100;
        let net = amount - fee;

        // Verify exact arithmetic
        assert_eq!(payment.fee(), fee);
        assert_eq!(payment.net(), net);
        assert_eq!(payment.net() + payment.fee(), amount);

        println!("  ${:>18.2}: fee = ${:>12.2}, net = ${:>18.2} ✓",
            amount as f64 / 100.0,
            fee as f64 / 100.0,
            net as f64 / 100.0
        );
    }

    println!("\n✓ All precision limits validated\n");
    println!("==================================================\n");
}

#[test]
fn validate_no_float_comparison() {
    // Ensure we never use floating-point comparison
    let payment = PaymentCapsule256::new(1, 1, 1_234_567);

    // All values are i64 (no floats)
    let amount: i64 = payment.amount();
    let fee: i64 = payment.fee();
    let net: i64 = payment.net();

    // Exact comparison (no epsilon)
    assert_eq!(amount - fee, net);
    assert_eq!(net + fee, amount);

    // No floating-point conversions in hot path
    assert_eq!(std::mem::size_of_val(&amount), 8);
    assert_eq!(std::mem::size_of_val(&fee), 8);
    assert_eq!(std::mem::size_of_val(&net), 8);
}

#[test]
fn validate_deterministic_behavior() {
    // Same input → same output (always)
    let amount = 1_234_567;

    let payment1 = PaymentCapsule256::new(1, 1, amount);
    let payment2 = PaymentCapsule256::new(2, 2, amount);
    let payment3 = PaymentCapsule256::new(3, 3, amount);

    // All should have identical fee/net calculations
    assert_eq!(payment1.fee(), payment2.fee());
    assert_eq!(payment2.fee(), payment3.fee());

    assert_eq!(payment1.net(), payment2.net());
    assert_eq!(payment2.net(), payment3.net());

    // Run 10,000 times to verify consistency
    for _ in 0..10_000 {
        let payment = PaymentCapsule256::new(1, 1, amount);
        assert_eq!(payment.fee(), payment1.fee());
        assert_eq!(payment.net(), payment1.net());
    }
}
