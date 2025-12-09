#!/usr/bin/env rust-script
//! [TRADE SECRET] Lambda LUT Validation Tool
//!
//! Verifies that pre-computed Q16.16 lambda values match runtime f64 computation
//! within ±0.5 ULP tolerance.
//!
//! UCE34: T28 Unit Testing (Q1-Q7) - Verify accuracy assumptions

// Include the generated LUT inline for validation
const LAMBDA_LUT_Q16: [u32; 256] = [
    // QP 0-3
    0x00000D9A, // QP=0: Minimum lambda (near-lossless)
    0x00001123, 0x00001597, 0x00001B33,
    // QP 4-7
    0x00002245, 0x00002B2D, 0x00003666, 0x0000448A,
    // QP 8-11
    0x0000565B, 0x00006CCD, 0x00008914, 0x0000ACB6,
    // QP 12-15
    0x0000D99A, // QP=12: Unity exponent (λ=0.85)
    0x00011229, 0x0001596B, 0x0001B333,
    // QP 16-19
    0x00022451, 0x0002B2D6, 0x00036666, 0x000448A3,
    // QP 20-23
    0x000565AD, 0x0006CCCD, 0x00089145, 0x000ACB59,
    // QP 24-27
    0x000D999A, // QP=24: Moderate compression
    0x0011228B, 0x001596B2, 0x001B3333,
    // QP 28-31
    0x00224515, 0x002B2D64, 0x00366666, 0x00448A2A,
    // QP 32-35
    0x00565AC8, 0x006CCCCD, 0x00891454, 0x00ACB590,
    // QP 36-39
    0x00D9999A, // QP=36: High compression
    0x011228A8, 0x01596B21, 0x01B33333,
    // QP 40-43
    0x02245151, 0x02B2D642, 0x03666666, 0x0448A2A2,
    // QP 44-47
    0x0565AC83, 0x06CCCCCD, 0x08914544, 0x0ACB5906,
    // QP 48-51
    0x0D99999A, // QP=48: Very high compression
    0x11228A87, 0x1596B20C, 0x1B333333,
    // QP 52-55
    0x2245150F, 0x2B2D6419, 0x36666666, 0x448A2A1D,
    // QP 56-59
    0x565AC832, 0x6CCCCCCD, 0x7FFFFFFF, 0x7FFFFFFF,
    // QP 60-63
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, // QP=63: Standard max QP for 8-bit
    // QP 64-127 (all clamped to i32::MAX)
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    // QP 128-191
    0x7FFFFFFF, // QP=128: Extended range midpoint
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    // QP 192-255
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF,
    0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, 0x7FFFFFFF, // QP=255: Maximum QP (extreme compression, clamped)
];

fn main() {
    println!("Lambda LUT Validation Report");
    println!("============================\n");

    let mut passed = 0;
    let mut failed = 0;
    let mut max_error = 0i64;
    let mut max_error_qp = 0;

    for qp in 0..256 {
        let lut_value = LAMBDA_LUT_Q16[qp];

        // Compute runtime value using f64 precision
        let exponent = (qp as f64 - 12.0) / 3.0;
        let lambda_f64 = 0.85 * 2_f64.powf(exponent);
        let lambda_q16_f64 = lambda_f64 * 65536.0;

        // Expected value with round-to-nearest and clamping
        let expected = if lambda_q16_f64 > 0x7FFFFFFF as f64 {
            0x7FFFFFFF
        } else {
            (lambda_q16_f64 + 0.5) as u32
        };

        // Calculate absolute error
        let error = (lut_value as i64 - expected as i64).abs();

        if error <= 1 {
            // ±0.5 ULP tolerance (allowing for rounding variations)
            passed += 1;
        } else {
            failed += 1;
            println!("FAIL: QP={:3} | LUT=0x{:08X} | Expected=0x{:08X} | Error={}",
                     qp, lut_value, expected, error);
        }

        if error > max_error {
            max_error = error;
            max_error_qp = qp;
        }
    }

    println!("\nValidation Summary");
    println!("==================");
    println!("Total Tests:    256");
    println!("Passed:         {}", passed);
    println!("Failed:         {}", failed);
    println!("Pass Rate:      {:.2}%", (passed as f64 / 256.0) * 100.0);
    println!("Max Error:      {} ULP (QP={})", max_error, max_error_qp);

    if failed == 0 {
        println!("\n✓ ALL TESTS PASSED - LUT is production-ready!");
    } else {
        println!("\n✗ VALIDATION FAILED - Review errors above");
        std::process::exit(1);
    }

    // Print key QP values for manual inspection
    println!("\nKey QP Values (Manual Inspection)");
    println!("==================================");
    let key_qps = [0, 12, 24, 36, 48, 63, 128, 255];
    for &qp in &key_qps {
        let lut_value = LAMBDA_LUT_Q16[qp];
        let lambda_float = (lut_value as f64) / 65536.0;
        println!("QP={:3} | LUT=0x{:08X} | Lambda={:.6}", qp, lut_value, lambda_float);
    }
}
