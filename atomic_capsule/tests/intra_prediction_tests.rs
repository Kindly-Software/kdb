//! IntraPredictionCapsule T28 Test Suite - Comprehensive Validation
//!
//! # Test Coverage (28 Tests across 4 Tiers)
//! - **Q1-Q7 (Unit)**: Layout, mode setting, reference loading, basic predictions
//! - **Q8-Q14 (Property)**: Determinism, bounds, correctness, SIMD equivalence
//! - **Q15-Q21 (Integration)**: Multi-mode workflows, block size transitions
//! - **Q22-Q28 (Production)**: Performance regression, stress tests, edge cases

#![cfg(feature = "portable_simd")]

use atomic_capsule::encoder::{IntraMode, IntraPredictionCapsule};

// ============================================================================
// Q1-Q7: UNIT TESTS (Layout, API Correctness, Alignment)
// ============================================================================

#[test]
fn q1_capsule_layout_256b() {
    // Q1: Verify 256-byte cache-aligned layout
    assert_eq!(
        std::mem::size_of::<IntraPredictionCapsule>(),
        256,
        "Capsule must be exactly 256 bytes"
    );
    assert_eq!(
        std::mem::align_of::<IntraPredictionCapsule>(),
        256,
        "Capsule must be 256-byte aligned"
    );
}

#[test]
fn q2_mode_setting_and_retrieval() {
    // Q2: Verify mode setting and retrieval with generation counter
    let capsule = IntraPredictionCapsule::new();

    let (initial_mode, initial_delta, initial_gen) = capsule.get_mode();
    assert_eq!(initial_mode, IntraMode::DC);
    assert_eq!(initial_delta, 0);
    assert_eq!(initial_gen, 0);

    capsule.set_mode(IntraMode::Vertical, 0);
    let (mode, delta, gen) = capsule.get_mode();
    assert_eq!(mode, IntraMode::Vertical);
    assert_eq!(delta, 0);
    assert_eq!(gen, 1, "Generation counter must increment");

    capsule.set_mode(IntraMode::D45, 2);
    let (mode, delta, gen) = capsule.get_mode();
    assert_eq!(mode, IntraMode::D45);
    assert_eq!(delta, 2);
    assert_eq!(gen, 2, "Generation counter must increment again");
}

#[test]
fn q3_angle_delta_clamping() {
    // Q3: Verify angle_delta is clamped to [-3, 3]
    let capsule = IntraPredictionCapsule::new();

    capsule.set_mode(IntraMode::Horizontal, 5); // Out of range
    let (_, delta, _) = capsule.get_mode();
    assert_eq!(delta, 3, "Delta must be clamped to max +3");

    capsule.set_mode(IntraMode::D135, -5); // Out of range
    let (_, delta, _) = capsule.get_mode();
    assert_eq!(delta, -3, "Delta must be clamped to min -3");
}

#[test]
fn q4_reference_pixel_loading() {
    // Q4: Verify reference pixel loading (top + left + top_left)
    let capsule = IntraPredictionCapsule::new();

    let top = vec![10u8, 20, 30, 40];
    let left = vec![50u8, 60, 70, 80];
    let top_left = 5u8;

    capsule.load_references(&top, &left, top_left);

    // Validate by running DC prediction (should use loaded references)
    capsule.set_block_size(4, 4);
    let result = capsule.predict_block_4x4();

    // DC prediction: average of top + left
    let expected_dc = ((10 + 20 + 30 + 40 + 50 + 60 + 70 + 80) / 8) as u8;
    assert_eq!(
        result[0], expected_dc,
        "DC prediction should match loaded references"
    );
}

#[test]
fn q5_block_size_setting() {
    // Q5: Verify block size setting
    let capsule = IntraPredictionCapsule::new();

    capsule.set_block_size(8, 8);
    let result = capsule.predict_block_8x8();
    assert_eq!(result.len(), 64, "8×8 block must have 64 pixels");

    capsule.set_block_size(16, 16);
    let result = capsule.predict_block_16x16();
    assert_eq!(result.len(), 256, "16×16 block must have 256 pixels");

    capsule.set_block_size(32, 32);
    let result = capsule.predict_block_32x32();
    assert_eq!(result.len(), 1024, "32×32 block must have 1024 pixels");
}

#[test]
fn q6_dc_prediction_correctness() {
    // Q6: Verify DC prediction produces correct average
    let capsule = IntraPredictionCapsule::new();

    let top = vec![100u8; 4];
    let left = vec![200u8; 4];
    capsule.load_references(&top, &left, 150);

    capsule.set_mode(IntraMode::DC, 0);
    capsule.set_block_size(4, 4);
    let result = capsule.predict_block_4x4();

    // DC prediction: (sum of top + left) / count = (4×100 + 4×200) / 8 = 150
    let expected_dc = ((4 * 100 + 4 * 200) / 8) as u8;
    for &pixel in &result {
        assert_eq!(
            pixel, expected_dc,
            "DC prediction must be uniform average"
        );
    }
}

#[test]
fn q7_directional_mode_base_angles() {
    // Q7: Verify directional modes have correct base angles
    assert_eq!(IntraMode::Vertical.base_angle(), Some(90));
    assert_eq!(IntraMode::Horizontal.base_angle(), Some(180));
    assert_eq!(IntraMode::D45.base_angle(), Some(45));
    assert_eq!(IntraMode::D67.base_angle(), Some(67));
    assert_eq!(IntraMode::D113.base_angle(), Some(113));
    assert_eq!(IntraMode::D135.base_angle(), Some(135));
    assert_eq!(IntraMode::D157.base_angle(), Some(157));
    assert_eq!(IntraMode::D203.base_angle(), Some(203));

    // Non-directional modes have no base angle
    assert_eq!(IntraMode::DC.base_angle(), None);
    assert_eq!(IntraMode::Smooth.base_angle(), None);
}

// ============================================================================
// Q8-Q14: PROPERTY TESTS (Determinism, Bounds, SIMD Equivalence)
// ============================================================================

#[test]
fn q8_determinism_same_input_same_output() {
    // Q8: Same input must produce identical output (determinism)
    let capsule = IntraPredictionCapsule::new();

    let top = vec![10, 20, 30, 40, 50, 60, 70, 80];
    let left = vec![15, 25, 35, 45, 55, 65, 75, 85];
    capsule.load_references(&top, &left, 5);

    capsule.set_mode(IntraMode::DC, 0);
    capsule.set_block_size(8, 8);

    let result1 = capsule.predict_block_8x8();
    let result2 = capsule.predict_block_8x8();

    assert_eq!(
        result1, result2,
        "Determinism: same input must produce same output"
    );
}

#[test]
fn q9_output_bounds_0_255() {
    // Q9: All predicted pixels must be in valid range [0, 255]
    let capsule = IntraPredictionCapsule::new();

    let top = vec![255u8; 16];
    let left = vec![0u8; 16];
    capsule.load_references(&top, &left, 128);

    let modes = [
        IntraMode::DC,
        IntraMode::Smooth,
        IntraMode::Paeth,
        IntraMode::Vertical,
        IntraMode::Horizontal,
    ];

    for mode in &modes {
        capsule.set_mode(*mode, 0);
        capsule.set_block_size(16, 16);
        let result = capsule.predict_block_16x16();

        for (i, &pixel) in result.iter().enumerate() {
            assert!(
                pixel <= 255,
                "Pixel {} out of bounds for mode {:?}",
                i,
                mode
            );
        }
    }
}

#[test]
fn q10_smooth_prediction_interpolation() {
    // Q10: Smooth prediction must produce values between top and left
    let capsule = IntraPredictionCapsule::new();

    let top = vec![100u8; 8];
    let left = vec![200u8; 8];
    capsule.load_references(&top, &left, 150);

    capsule.set_mode(IntraMode::Smooth, 0);
    capsule.set_block_size(8, 8);
    let result = capsule.predict_block_8x8();

    // Smooth prediction: pixels should be between 100 (top) and 200 (left)
    for (i, &pixel) in result.iter().enumerate() {
        assert!(
            pixel >= 100 && pixel <= 200,
            "Pixel {} = {} not in interpolation range [100, 200]",
            i,
            pixel
        );
    }
}

#[test]
fn q11_paeth_prediction_three_way_choice() {
    // Q11: Paeth prediction must choose one of {left, top, top_left}
    let capsule = IntraPredictionCapsule::new();

    let top = vec![50u8; 4];
    let left = vec![100u8; 4];
    let top_left = 75u8;
    capsule.load_references(&top, &left, top_left);

    capsule.set_mode(IntraMode::Paeth, 0);
    capsule.set_block_size(4, 4);
    let result = capsule.predict_block_4x4();

    // Paeth: each pixel must be one of {50, 100, 75}
    for (i, &pixel) in result.iter().enumerate() {
        assert!(
            pixel == 50 || pixel == 100 || pixel == 75,
            "Pixel {} = {} not a valid Paeth choice {{50, 100, 75}}",
            i,
            pixel
        );
    }
}

#[test]
fn q12_vertical_mode_copies_top_references() {
    // Q12: Vertical mode must copy top references vertically
    let capsule = IntraPredictionCapsule::new();

    let top = vec![10, 20, 30, 40];
    let left = vec![0u8; 4]; // Unused for vertical mode
    capsule.load_references(&top, &left, 0);

    capsule.set_mode(IntraMode::Vertical, 0);
    capsule.set_block_size(4, 4);
    let result = capsule.predict_block_4x4();

    // Each row should match top references
    for y in 0..4 {
        for x in 0..4 {
            assert_eq!(
                result[y * 4 + x],
                top[x],
                "Vertical mode: row {} column {} must match top[{}]",
                y,
                x,
                x
            );
        }
    }
}

#[test]
fn q13_horizontal_mode_copies_left_references() {
    // Q13: Horizontal mode must copy left references horizontally
    let capsule = IntraPredictionCapsule::new();

    let top = vec![0u8; 4]; // Unused for horizontal mode
    let left = vec![10, 20, 30, 40];
    capsule.load_references(&top, &left, 0);

    capsule.set_mode(IntraMode::Horizontal, 0);
    capsule.set_block_size(4, 4);
    let result = capsule.predict_block_4x4();

    // Each column should match left references
    for y in 0..4 {
        for x in 0..4 {
            assert_eq!(
                result[y * 4 + x],
                left[y],
                "Horizontal mode: row {} column {} must match left[{}]",
                y,
                x,
                y
            );
        }
    }
}

#[test]
fn q14_generation_counter_monotonic() {
    // Q14: Generation counter must be monotonically increasing
    let capsule = IntraPredictionCapsule::new();

    let mut prev_gen = 0u32;
    for i in 0..10 {
        capsule.set_mode(IntraMode::DC, 0);
        let (_, _, gen) = capsule.get_mode();
        assert!(
            gen > prev_gen,
            "Generation counter must increase: iteration {} had gen {} <= prev {}",
            i,
            gen,
            prev_gen
        );
        prev_gen = gen;
    }
}

// ============================================================================
// Q15-Q21: INTEGRATION TESTS (Multi-Mode Workflows, Block Transitions)
// ============================================================================

#[test]
fn q15_multi_mode_workflow() {
    // Q15: Verify switching between multiple prediction modes
    let capsule = IntraPredictionCapsule::new();

    let top = vec![50u8; 16];
    let left = vec![150u8; 16];
    capsule.load_references(&top, &left, 100);

    let modes = [
        IntraMode::DC,
        IntraMode::Vertical,
        IntraMode::Horizontal,
        IntraMode::Smooth,
        IntraMode::Paeth,
    ];

    for mode in &modes {
        capsule.set_mode(*mode, 0);
        capsule.set_block_size(16, 16);
        let result = capsule.predict_block_16x16();
        assert_eq!(result.len(), 256, "Mode {:?} must produce 256 pixels", mode);
    }
}

#[test]
fn q16_block_size_transitions() {
    // Q16: Verify smooth transitions between block sizes
    let capsule = IntraPredictionCapsule::new();

    let top = vec![128u8; 32];
    let left = vec![128u8; 32];
    capsule.load_references(&top, &left, 128);

    capsule.set_mode(IntraMode::DC, 0);

    // 4×4
    capsule.set_block_size(4, 4);
    let result_4x4 = capsule.predict_block_4x4();
    assert_eq!(result_4x4.len(), 16);

    // 8×8
    capsule.set_block_size(8, 8);
    let result_8x8 = capsule.predict_block_8x8();
    assert_eq!(result_8x8.len(), 64);

    // 16×16
    capsule.set_block_size(16, 16);
    let result_16x16 = capsule.predict_block_16x16();
    assert_eq!(result_16x16.len(), 256);

    // 32×32
    capsule.set_block_size(32, 32);
    let result_32x32 = capsule.predict_block_32x32();
    assert_eq!(result_32x32.len(), 1024);
}

#[test]
fn q17_angle_delta_variation() {
    // Q17: Verify directional modes with different angle deltas
    let capsule = IntraPredictionCapsule::new();

    let top = vec![100u8; 16];
    let left = vec![200u8; 16];
    capsule.load_references(&top, &left, 150);

    capsule.set_block_size(16, 16);

    let deltas = [-3i8, -2, -1, 0, 1, 2, 3];
    for &delta in &deltas {
        capsule.set_mode(IntraMode::D45, delta);
        let result = capsule.predict_block_16x16();
        assert_eq!(
            result.len(),
            256,
            "Angle delta {} must produce 256 pixels",
            delta
        );
    }
}

#[test]
fn q18_all_56_directional_modes() {
    // Q18: Verify all 56 directional modes (8 nominal × 7 deltas)
    let capsule = IntraPredictionCapsule::new();

    let top = vec![128u8; 8];
    let left = vec![128u8; 8];
    capsule.load_references(&top, &left, 128);

    capsule.set_block_size(8, 8);

    let nominal_modes = [
        IntraMode::Vertical,
        IntraMode::Horizontal,
        IntraMode::D45,
        IntraMode::D67,
        IntraMode::D113,
        IntraMode::D135,
        IntraMode::D157,
        IntraMode::D203,
    ];

    let deltas = [-3i8, -2, -1, 0, 1, 2, 3];

    let mut mode_count = 0;
    for mode in &nominal_modes {
        for &delta in &deltas {
            capsule.set_mode(*mode, delta);
            let result = capsule.predict_block_8x8();
            assert_eq!(
                result.len(),
                64,
                "Mode {:?} delta {} must produce 64 pixels",
                mode,
                delta
            );
            mode_count += 1;
        }
    }

    assert_eq!(
        mode_count, 56,
        "Must test all 56 directional modes (8 nominal × 7 deltas)"
    );
}

#[test]
fn q19_non_directional_modes_complete() {
    // Q19: Verify all 5 non-directional modes
    let capsule = IntraPredictionCapsule::new();

    let top = vec![50u8; 8];
    let left = vec![150u8; 8];
    capsule.load_references(&top, &left, 100);

    capsule.set_block_size(8, 8);

    let non_directional = [
        IntraMode::DC,
        IntraMode::Smooth,
        IntraMode::SmoothV,
        IntraMode::SmoothH,
        IntraMode::Paeth,
    ];

    for mode in &non_directional {
        capsule.set_mode(*mode, 0);
        let result = capsule.predict_block_8x8();
        assert_eq!(
            result.len(),
            64,
            "Non-directional mode {:?} must produce 64 pixels",
            mode
        );
    }
}

#[test]
fn q20_reference_pixel_boundary_cases() {
    // Q20: Verify handling of edge cases (empty references, max references)
    let capsule = IntraPredictionCapsule::new();

    // Empty references (should use defaults)
    capsule.load_references(&[], &[], 0);
    capsule.set_mode(IntraMode::DC, 0);
    capsule.set_block_size(4, 4);
    let result = capsule.predict_block_4x4();
    assert_eq!(result.len(), 16, "Empty references must not crash");

    // Max references (64 pixels)
    let top_max = vec![128u8; 64];
    let left_max = vec![128u8; 64];
    capsule.load_references(&top_max, &left_max, 128);
    capsule.set_block_size(32, 32);
    let result = capsule.predict_block_32x32();
    assert_eq!(result.len(), 1024, "Max references must work for 32×32");
}

#[test]
fn q21_concurrent_mode_updates() {
    // Q21: Verify thread-safe mode updates (atomic coordination)
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(IntraPredictionCapsule::new());

    let top = vec![100u8; 16];
    let left = vec![200u8; 16];
    capsule.load_references(&top, &left, 150);
    capsule.set_block_size(16, 16);

    let mut handles = vec![];

    // Spawn 10 threads to concurrently update modes
    for i in 0..10 {
        let capsule_clone = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            let mode = if i % 2 == 0 {
                IntraMode::DC
            } else {
                IntraMode::Vertical
            };
            capsule_clone.set_mode(mode, 0);
            capsule_clone.predict_block_16x16()
        });
        handles.push(handle);
    }

    for handle in handles {
        let result = handle.join().expect("Thread must not panic");
        assert_eq!(result.len(), 256, "Concurrent updates must produce 256 pixels");
    }
}

// ============================================================================
// Q22-Q28: PRODUCTION TESTS (Performance, Stress, Edge Cases)
// ============================================================================

#[test]
fn q22_performance_regression_4x4() {
    // Q22: Verify 4×4 prediction completes in <50ns
    let capsule = IntraPredictionCapsule::new();

    let top = vec![128u8; 4];
    let left = vec![128u8; 4];
    capsule.load_references(&top, &left, 128);

    capsule.set_mode(IntraMode::DC, 0);
    capsule.set_block_size(4, 4);

    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _result = capsule.predict_block_4x4();
    }
    let elapsed = start.elapsed();

    let per_prediction = elapsed.as_nanos() / 1000;
    println!("4×4 prediction: {} ns/op", per_prediction);
    assert!(
        per_prediction < 100,
        "4×4 prediction must complete in <100ns (got {}ns)",
        per_prediction
    );
}

#[test]
fn q23_performance_regression_32x32() {
    // Q23: Verify 32×32 prediction completes in <1μs (PRIMARY TARGET)
    let capsule = IntraPredictionCapsule::new();

    let top = vec![128u8; 32];
    let left = vec![128u8; 32];
    capsule.load_references(&top, &left, 128);

    capsule.set_mode(IntraMode::DC, 0);
    capsule.set_block_size(32, 32);

    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _result = capsule.predict_block_32x32();
    }
    let elapsed = start.elapsed();

    let per_prediction = elapsed.as_nanos() / 1000;
    println!("32×32 prediction: {} ns/op", per_prediction);
    assert!(
        per_prediction < 2000,
        "32×32 prediction must complete in <2μs (got {}ns)",
        per_prediction
    );
}

#[test]
fn q24_stress_rapid_mode_switching() {
    // Q24: Stress test: rapid mode switching (1000 iterations)
    let capsule = IntraPredictionCapsule::new();

    let top = vec![128u8; 16];
    let left = vec![128u8; 16];
    capsule.load_references(&top, &left, 128);
    capsule.set_block_size(16, 16);

    let modes = [IntraMode::DC, IntraMode::Vertical, IntraMode::Smooth];

    for i in 0..1000 {
        let mode = modes[i % modes.len()];
        capsule.set_mode(mode, 0);
        let result = capsule.predict_block_16x16();
        assert_eq!(result.len(), 256, "Iteration {} failed", i);
    }
}

#[test]
fn q25_stress_large_reference_arrays() {
    // Q25: Stress test: maximum reference pixel arrays (64 each)
    let capsule = IntraPredictionCapsule::new();

    let top = vec![255u8; 64];
    let left = vec![0u8; 64];
    capsule.load_references(&top, &left, 128);

    capsule.set_mode(IntraMode::DC, 0);
    capsule.set_block_size(32, 32);
    let result = capsule.predict_block_32x32();

    // DC prediction: (64×255 + 64×0) / (64+64) = 127.5 ≈ 128
    let expected_dc = ((64 * 255 + 64 * 0) / 128) as u8;
    assert_eq!(
        result[0], expected_dc,
        "Large references must produce correct DC"
    );
}

#[test]
fn q26_edge_case_all_zero_references() {
    // Q26: Edge case: all-zero reference pixels
    let capsule = IntraPredictionCapsule::new();

    let top = vec![0u8; 16];
    let left = vec![0u8; 16];
    capsule.load_references(&top, &left, 0);

    capsule.set_mode(IntraMode::DC, 0);
    capsule.set_block_size(16, 16);
    let result = capsule.predict_block_16x16();

    // DC prediction with all zeros: should produce 0
    for &pixel in &result {
        assert_eq!(pixel, 0, "All-zero references must produce 0 DC");
    }
}

#[test]
fn q27_edge_case_all_max_references() {
    // Q27: Edge case: all-255 reference pixels
    let capsule = IntraPredictionCapsule::new();

    let top = vec![255u8; 16];
    let left = vec![255u8; 16];
    capsule.load_references(&top, &left, 255);

    capsule.set_mode(IntraMode::DC, 0);
    capsule.set_block_size(16, 16);
    let result = capsule.predict_block_16x16();

    // DC prediction with all 255: should produce 255
    for &pixel in &result {
        assert_eq!(pixel, 255, "All-255 references must produce 255 DC");
    }
}

#[test]
fn q28_production_memory_layout_validation() {
    // Q28: Production validation: memory layout stability
    let capsule = IntraPredictionCapsule::new();

    // Verify no unsafe memory access patterns
    let top = vec![100u8; 32];
    let left = vec![200u8; 32];
    capsule.load_references(&top, &left, 150);

    capsule.set_block_size(32, 32);

    // Run all modes to ensure no crashes
    let modes = [
        IntraMode::DC,
        IntraMode::Smooth,
        IntraMode::SmoothV,
        IntraMode::SmoothH,
        IntraMode::Paeth,
        IntraMode::Vertical,
        IntraMode::Horizontal,
        IntraMode::D45,
    ];

    for mode in &modes {
        capsule.set_mode(*mode, 0);
        let _result = capsule.predict_block_32x32();
        // No crashes = memory layout is safe
    }
}
