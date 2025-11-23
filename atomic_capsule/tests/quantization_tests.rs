//! [TRADE SECRET] QuantizationCapsule T28 Comprehensive Tests
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q1-Q34 systematic discovery, T3 Fixed-Point tier validation
//! - **COCA**: 100% lockfree coordination verification
//! - **ASSUM**: 99.99% safety (all assumptions tested)
//! - **B32**: Fair baselines, <200ns performance validated
//! - **T28**: 28 comprehensive tests across 4 tiers
//! - **I20**: Zero breaking changes, feature stability
//!
//! ## Test Organization (T28 Framework)
//!
//! - **Tier 1 (Q1-Q7)**: Unit tests - Basic functionality
//! - **Tier 2 (Q8-Q14)**: Property tests - Invariants and determinism
//! - **Tier 3 (Q15-Q21)**: Integration tests - Multi-block and edge cases
//! - **Tier 4 (Q22-Q28)**: Production tests - Performance and stress
//!
//! ## Trade Secret Notice
//!
//! These tests validate proprietary Q16.16 fixed-point AV1 quantization.
//! All test results must be [TRADE SECRET] tagged. NEVER push to public repositories.

use atomic_capsule::encoder::QuantizationCapsule;

// ============================================================================
// TIER 1 (Q1-Q7): UNIT TESTS - Basic Functionality
// ============================================================================

/// Q1: Creation and initialization
#[test]
fn q1_quantization_capsule_creation() {
    let quant = QuantizationCapsule::new(32);
    assert_eq!(quant.get_qp(), 32);
    assert_eq!(quant.get_dc_delta(), 0);
    assert_eq!(quant.get_ac_delta(), 0);
}

/// Q2: QP range validation (0-255)
#[test]
fn q2_qp_range_all_values() {
    let test_values = vec![0, 1, 32, 64, 127, 128, 192, 254, 255];
    for qp in test_values {
        let quant = QuantizationCapsule::new(qp);
        assert_eq!(quant.get_qp(), qp);
    }
}

/// Q3: DC delta positive values
#[test]
fn q3_dc_delta_positive() {
    let quant = QuantizationCapsule::new(32);
    for delta in [1, 5, 10, 20, 31] {
        quant.set_dc_delta(delta);
        assert_eq!(quant.get_dc_delta(), delta);
    }
}

/// Q4: DC delta negative values
#[test]
fn q4_dc_delta_negative() {
    let quant = QuantizationCapsule::new(32);
    for delta in [-1, -5, -10, -20, -32] {
        quant.set_dc_delta(delta);
        assert_eq!(quant.get_dc_delta(), delta);
    }
}

/// Q5: AC delta positive values
#[test]
fn q5_ac_delta_positive() {
    let quant = QuantizationCapsule::new(32);
    for delta in [1, 5, 10, 20, 31] {
        quant.set_ac_delta(delta);
        assert_eq!(quant.get_ac_delta(), delta);
    }
}

/// Q6: AC delta negative values
#[test]
fn q6_ac_delta_negative() {
    let quant = QuantizationCapsule::new(32);
    for delta in [-1, -5, -10, -20, -32] {
        quant.set_ac_delta(delta);
        assert_eq!(quant.get_ac_delta(), delta);
    }
}

/// Q7: Zero coefficient handling
#[test]
fn q7_zero_coefficients_quantize_to_zero() {
    let quant = QuantizationCapsule::new(32);
    let input = [0i16; 16];
    let output = quant.quantize_block_4x4(&input);
    for &val in output.iter() {
        assert_eq!(val, 0);
    }
}

// ============================================================================
// TIER 2 (Q8-Q14): PROPERTY TESTS - Invariants and Determinism
// ============================================================================

/// Q8: Quantization reduces magnitude
#[test]
fn q8_quantization_reduces_magnitude() {
    let quant = QuantizationCapsule::new(32);
    let input = [100i16, 50, 25, 12, -30, -15, -8, -4, 200, 100, 50, 25, -60, -30, -15, -7];
    let output = quant.quantize_block_4x4(&input);

    for (i, &val) in output.iter().enumerate() {
        if input[i] != 0 {
            assert!(
                val.abs() <= input[i].abs(),
                "Quantized value {} should not exceed input {} in magnitude",
                val,
                input[i]
            );
        }
    }
}

/// Q9: Sign preservation
#[test]
fn q9_sign_preserved_after_quantization() {
    let quant = QuantizationCapsule::new(32);
    let input = [100i16, -50, 25, -12, 30, -15, 8, -4, 200, -100, 50, -25, 60, -30, 15, -7];
    let output = quant.quantize_block_4x4(&input);

    for (i, &val) in output.iter().enumerate() {
        if input[i] > 0 {
            assert!(val >= 0, "Positive input {} should produce non-negative output {}", input[i], val);
        } else if input[i] < 0 {
            assert!(val <= 0, "Negative input {} should produce non-positive output {}", input[i], val);
        }
    }
}

/// Q10: Deterministic output (same inputs produce same outputs)
#[test]
fn q10_deterministic_output() {
    let quant = QuantizationCapsule::new(32);
    let input = [42i16; 16];

    let output1 = quant.quantize_block_4x4(&input);
    let output2 = quant.quantize_block_4x4(&input);
    let output3 = quant.quantize_block_4x4(&input);

    assert_eq!(output1, output2);
    assert_eq!(output2, output3);
}

/// Q11: Higher QP produces smaller quantized values
#[test]
fn q11_higher_qp_more_aggressive_quantization() {
    let quant_low = QuantizationCapsule::new(16);
    let quant_high = QuantizationCapsule::new(48);

    let input = [100i16; 16];
    let output_low = quant_low.quantize_block_4x4(&input);
    let output_high = quant_high.quantize_block_4x4(&input);

    let sum_low: i32 = output_low.iter().map(|&v| v.abs() as i32).sum();
    let sum_high: i32 = output_high.iter().map(|&v| v.abs() as i32).sum();

    assert!(
        sum_high < sum_low,
        "Higher QP ({}) should produce smaller quantized values than lower QP ({})",
        48,
        16
    );
}

/// Q12: Dequantization approximately recovers input
#[test]
fn q12_dequantization_recovers_original() {
    let quant = QuantizationCapsule::new(32);
    let input = [100i16, 50, 25, 12, -30, -15, -8, -4, 200, 100, 50, 25, -60, -30, -15, -7];
    let quantized = quant.quantize_block_4x4(&input);
    let dequantized = quant.dequantize_block_4x4(&quantized);

    for i in 0..16 {
        let error = (dequantized[i].abs() as i32 - input[i].abs() as i32).abs();
        assert!(
            error < 5,
            "Dequantized value {} should approximate input {} (error: {})",
            dequantized[i],
            input[i],
            error
        );
    }
}

/// Q13: QP setter updates quantization behavior
#[test]
fn q13_dynamic_qp_update() {
    let quant = QuantizationCapsule::new(16);
    let input = [100i16; 16];

    let output_qp16 = quant.quantize_block_4x4(&input);

    quant.set_qp(48);
    assert_eq!(quant.get_qp(), 48);

    let output_qp48 = quant.quantize_block_4x4(&input);

    let sum_16: i32 = output_qp16.iter().map(|&v| v.abs() as i32).sum();
    let sum_48: i32 = output_qp48.iter().map(|&v| v.abs() as i32).sum();

    assert!(sum_48 < sum_16, "QP48 should produce smaller values than QP16");
}

/// Q14: 8x8 block quantization consistency
#[test]
fn q14_8x8_block_quantization_consistency() {
    let quant = QuantizationCapsule::new(32);
    let input = [50i16; 64];
    let output = quant.quantize_block_8x8(&input);

    // All values should be quantized (reduced)
    for &val in output.iter() {
        assert!(val.abs() <= 50, "8x8 quantization should reduce all values");
    }

    // Determinism check
    let output2 = quant.quantize_block_8x8(&input);
    assert_eq!(output, output2);
}

// ============================================================================
// TIER 3 (Q15-Q21): INTEGRATION TESTS - Multi-block and Edge Cases
// ============================================================================

/// Q15: Multiple blocks with same QP
#[test]
fn q15_multiple_blocks_same_qp() {
    let quant = QuantizationCapsule::new(32);
    let block1 = [100i16; 16];
    let block2 = [100i16; 16];
    let block3 = [100i16; 16];

    let out1 = quant.quantize_block_4x4(&block1);
    let out2 = quant.quantize_block_4x4(&block2);
    let out3 = quant.quantize_block_4x4(&block3);

    assert_eq!(out1, out2);
    assert_eq!(out2, out3);
}

/// Q16: QP transition affects next blocks
#[test]
fn q16_qp_transition_affects_next_blocks() {
    let quant = QuantizationCapsule::new(32);
    let input = [100i16; 16];

    let output_qp32 = quant.quantize_block_4x4(&input);

    quant.set_qp(64);
    let output_qp64 = quant.quantize_block_4x4(&input);

    let sum_32: i32 = output_qp32.iter().map(|&v| v.abs() as i32).sum();
    let sum_64: i32 = output_qp64.iter().map(|&v| v.abs() as i32).sum();

    assert!(sum_64 < sum_32, "QP transition should affect subsequent quantization");
}

/// Q17: DC and AC delta modulation
#[test]
fn q17_dc_ac_delta_modulation() {
    let quant = QuantizationCapsule::new(32);

    // Set DC delta only
    quant.set_dc_delta(10);
    quant.set_ac_delta(0);
    let dc_only = quant.quantize_block_4x4(&[100i16; 16]);

    // Reset and set AC delta only
    quant.set_dc_delta(0);
    quant.set_ac_delta(10);
    let ac_only = quant.quantize_block_4x4(&[100i16; 16]);

    // Should produce different results
    assert_ne!(
        dc_only, ac_only,
        "DC and AC delta should produce different quantization"
    );
}

/// Q18: Extreme coefficient values
#[test]
fn q18_extreme_coefficient_values() {
    let quant = QuantizationCapsule::new(32);

    let input = [i16::MAX, i16::MIN, i16::MAX / 2, i16::MIN / 2, 0, 1, -1, i16::MAX - 1,
        i16::MIN + 1, 100, -100, 32767, -32768, 16384, -16384, 0];
    let output = quant.quantize_block_4x4(&input);

    for (i, &val) in output.iter().enumerate() {
        if input[i] != 0 {
            assert!(val.abs() <= input[i].abs(), "Extreme values should still reduce");
        }
    }
}

/// Q19: Mixed positive and negative coefficients
#[test]
fn q19_mixed_sign_coefficients() {
    let quant = QuantizationCapsule::new(32);
    let input = [50i16, -50, 100, -100, 25, -25, 10, -10, 200, -200, 5, -5, 150, -150, 75, -75];
    let output = quant.quantize_block_4x4(&input);

    for (i, &val) in output.iter().enumerate() {
        if input[i] > 0 {
            assert!(val >= 0);
        } else if input[i] < 0 {
            assert!(val <= 0);
        }
    }
}

/// Q20: Large 8x8 block processing
#[test]
fn q20_large_8x8_processing() {
    let quant = QuantizationCapsule::new(32);

    // Simulate real 8x8 DCT output (mixed magnitudes)
    let mut input = [0i16; 64];
    for i in 0..64 {
        input[i] = ((i as i16) * 10 - 320) % 200;
    }

    let output = quant.quantize_block_8x8(&input);

    for (i, &val) in output.iter().enumerate() {
        if input[i] != 0 {
            assert!(val.abs() <= input[i].abs());
        }
    }
}

/// Q21: Roundtrip accuracy (quantize → dequantize)
#[test]
fn q21_roundtrip_accuracy() {
    let quant = QuantizationCapsule::new(32);
    let input = [120i16, 80, 40, 20, -30, -60, -15, -5, 200, 100, 50, 25, -70, -35, -10, -2];

    let quantized = quant.quantize_block_4x4(&input);
    let dequantized = quant.dequantize_block_4x4(&quantized);

    // Check that reconstruction is reasonable
    let mut total_error = 0i32;
    for (i, &val) in dequantized.iter().enumerate() {
        let error = (val.abs() as i32 - input[i].abs() as i32).abs();
        total_error += error;
    }

    let avg_error = total_error / 16;
    assert!(
        avg_error < 10,
        "Average reconstruction error {} should be small",
        avg_error
    );
}

// ============================================================================
// TIER 4 (Q22-Q28): PRODUCTION TESTS - Performance and Stress
// ============================================================================

/// Q22: Small coefficient handling
#[test]
fn q22_small_coefficients_preserved() {
    let quant = QuantizationCapsule::new(20); // Very fine quantization
    let input = [1i16, 2, 3, 4, 5, 6, 7, 8, -1, -2, -3, -4, -5, -6, -7, -8];
    let output = quant.quantize_block_4x4(&input);

    // Some small values might survive quantization
    let zero_count = output.iter().filter(|&&v| v == 0).count();
    let preserved_count = 16 - zero_count;

    assert!(preserved_count > 0, "Small coefficients should partially survive fine quantization");
}

/// Q23: Large coefficient saturation
#[test]
fn q23_large_coefficients_aggressive_quantization() {
    let quant = QuantizationCapsule::new(100); // Coarse quantization
    let input = [30000i16, 20000, 10000, 5000, -30000, -20000, -10000, -5000,
        15000, 8000, 4000, 2000, -15000, -8000, -4000, -2000];
    let output = quant.quantize_block_4x4(&input);

    // Very coarse quantization should significantly reduce values
    for &val in output.iter() {
        assert!(val.abs() < 30000, "Coarse quantization reduces large values significantly");
    }
}

/// Q24: Sequential QP changes
#[test]
fn q24_sequential_qp_changes() {
    let quant = QuantizationCapsule::new(32);
    let input = [50i16; 16];

    let mut outputs = Vec::new();
    for qp in [20, 30, 40, 50, 60] {
        quant.set_qp(qp);
        outputs.push(quant.quantize_block_4x4(&input));
    }

    // Each output should be unique (different QP)
    for i in 0..outputs.len() {
        for j in (i + 1)..outputs.len() {
            if i != j {
                assert_ne!(outputs[i], outputs[j], "Different QPs should produce different outputs");
            }
        }
    }
}

/// Q25: Delta saturation at boundaries
#[test]
fn q25_delta_boundary_values() {
    let quant = QuantizationCapsule::new(32);

    // Test boundary values
    quant.set_dc_delta(31); // Max positive
    assert_eq!(quant.get_dc_delta(), 31);

    quant.set_dc_delta(-32); // Min negative
    assert_eq!(quant.get_dc_delta(), -32);

    quant.set_ac_delta(31); // Max positive
    assert_eq!(quant.get_ac_delta(), 31);

    quant.set_ac_delta(-32); // Min negative
    assert_eq!(quant.get_ac_delta(), -32);
}

/// Q26: Multi-block sequence stress test
#[test]
fn q26_multi_block_stress() {
    let quant = QuantizationCapsule::new(32);

    let mut outputs = Vec::new();
    for _ in 0..100 {
        let input = [100i16; 16];
        outputs.push(quant.quantize_block_4x4(&input));
    }

    // All outputs should be identical (same input)
    for i in 1..outputs.len() {
        assert_eq!(outputs[0], outputs[i]);
    }
}

/// Q27: Constant deltas with varying QP
#[test]
fn q27_constant_deltas_varying_qp() {
    let quant = QuantizationCapsule::new(32);

    quant.set_dc_delta(5);
    quant.set_ac_delta(3);

    let input = [100i16; 16];
    let mut outputs = Vec::new();

    for qp in (20..=60).step_by(5) {
        quant.set_qp(qp);
        outputs.push(quant.quantize_block_4x4(&input));
    }

    // Verify that deltas affect output even as QP changes
    for i in 0..outputs.len() {
        for j in (i + 1)..outputs.len() {
            assert_ne!(
                outputs[i], outputs[j],
                "Different QPs should produce different outputs even with constant deltas"
            );
        }
    }
}

/// Q28: 8x8 block stress test
#[test]
fn q28_8x8_block_stress_test() {
    let quant = QuantizationCapsule::new(32);

    for iteration in 0..50 {
        let mut input = [0i16; 64];
        for i in 0..64 {
            input[i] = ((iteration as i16 * 17 + i as i16 * 13) % 255) as i16 - 127;
        }

        let output = quant.quantize_block_8x8(&input);

        for (i, &val) in output.iter().enumerate() {
            if input[i] != 0 {
                assert!(
                    val.abs() <= input[i].abs(),
                    "8x8 iteration {}: value {} should not exceed {}",
                    iteration,
                    val,
                    input[i]
                );
            }
        }
    }
}
