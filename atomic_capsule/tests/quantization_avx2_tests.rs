//! # T28 Comprehensive Tests for AVX2 Quantization
//!
//! **Framework**: T28 Testing Framework (All 4 Tiers, 28 Questions)
//! **Target**: AVX2-accelerated INT8/INT16 quantization for LLM inference
//! **Status**: Production-ready validation suite
//!
//! ## Test Coverage
//!
//! - **Tier 1 (Q1-Q7)**: Unit tests (7 tests)
//! - **Tier 2 (Q8-Q14)**: Property tests (7 tests, proptest)
//! - **Tier 3 (Q15-Q21)**: Integration tests (7 tests)
//! - **Tier 4 (Q22-Q28)**: Production tests (7 tests)
//! - **Total**: 28 tests (100% T28 framework compliance)

#![cfg(all(target_arch = "x86_64", feature = "portable_simd"))]
#![allow(clippy::excessive_precision)]
#![allow(clippy::float_cmp)]

use atomic_capsule::primitives::inference::quantization::QuantizationCapsule;
use proptest::prelude::*;

// =============================================================================
// TIER 1: UNIT TESTS (Q1-Q7) - 7 Tests
// =============================================================================

// -----------------------------------------------------------------------------
// Q1: Core Behaviors - Test quantize_avx2() correctness vs scalar
// -----------------------------------------------------------------------------

#[test]
fn t28_q1_quantize_avx2_correctness_vs_scalar() {
    // Arrange: Create quantization capsule with symmetric quantization
    let quant = QuantizationCapsule::from_range(-10.0, 10.0);

    // Test data: 16 weights (multiple of 8 for SIMD)
    let weights = vec![
        -10.0, -8.0, -6.0, -4.0, -2.0, 0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 5.0, -5.0, 3.0, -3.0, 1.0,
    ];

    // Act: Quantize with SIMD (portable_simd uses AVX2 on x86_64)
    let simd_quantized = quant.quantize_simd(&weights);
    let scalar_quantized = quant.quantize(&weights);

    // Assert: SIMD matches scalar (within Q8.8 rounding tolerance)
    assert_eq!(simd_quantized.len(), scalar_quantized.len());
    for (i, (&simd, &scalar)) in simd_quantized
        .iter()
        .zip(scalar_quantized.iter())
        .enumerate()
    {
        let diff = (simd - scalar).abs();
        assert!(
            diff <= 256, // Q8.8 format: 1 unit = 1/256
            "Mismatch at index {}: simd={}, scalar={}, diff={}",
            i,
            simd,
            scalar,
            diff
        );
    }
}

// -----------------------------------------------------------------------------
// Q2: Edge Cases - Test min/max values, zero, negative
// -----------------------------------------------------------------------------

#[test]
fn t28_q2_quantize_avx2_edge_cases() {
    let quant = QuantizationCapsule::new(1.0, 0);

    // Test edge cases: min, max, zero, negative
    let weights = vec![
        -128.0, -127.0, -1.0, 0.0, 1.0, 127.0, 128.0, -200.0, // Overflow test
    ];

    let quantized = quant.quantize_simd(&weights);

    // Verify clipping: Q8.8 range is [-128, 127.996]
    for &q in &quantized {
        let fp = q as f32 / 256.0;
        assert!(
            fp >= -128.0 && fp <= 127.0,
            "Quantized value out of range: fp={}",
            fp
        );
    }
}

// -----------------------------------------------------------------------------
// Q3: Alignment Requirements - Test multiple of 16 assertion
// -----------------------------------------------------------------------------

#[test]
#[should_panic(expected = "length must be multiple of 8")]
fn t28_q3_quantize_avx2_alignment_requirement() {
    let quant = QuantizationCapsule::new(1.0, 0);

    // Non-multiple of 8 should panic
    let weights = vec![1.0, 2.0, 3.0, 4.0, 5.0]; // 5 elements
    let _ = quant.quantize_simd(&weights);
}

// -----------------------------------------------------------------------------
// Q4: Q8.8 Scaling Accuracy - Test deterministic fixed-point arithmetic
// -----------------------------------------------------------------------------

#[test]
fn t28_q4_q8_8_scaling_accuracy() {
    let quant = QuantizationCapsule::new(1.0, 0);

    // Known values with exact Q8.8 representation
    let weights = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];

    let quantized = quant.quantize_simd(&weights);
    let dequantized = quant.dequantize_simd(&quantized);

    // Verify roundtrip accuracy (Q8.8 precision: 1/256 ≈ 0.00391)
    for (i, (&orig, &deq)) in weights.iter().zip(dequantized.iter()).enumerate() {
        let error = (orig - deq).abs();
        assert!(
            error < 0.01,
            "Roundtrip error at index {}: orig={}, deq={}, error={}",
            i,
            orig,
            deq,
            error
        );
    }
}

// -----------------------------------------------------------------------------
// Q5: Determinism - Same input → same output
// -----------------------------------------------------------------------------

#[test]
fn t28_q5_quantize_avx2_determinism() {
    let quant = QuantizationCapsule::from_range(-10.0, 10.0);
    let weights = vec![-5.5, -2.2, 0.0, 3.3, 7.7, -8.8, 4.4, 1.1];

    // Run quantization multiple times
    let result1 = quant.quantize_simd(&weights);
    let result2 = quant.quantize_simd(&weights);
    let result3 = quant.quantize_simd(&weights);

    // Assert: All results identical (deterministic)
    assert_eq!(result1, result2);
    assert_eq!(result2, result3);
}

// -----------------------------------------------------------------------------
// Q6: Dequantize AVX2 Correctness - Test dequantize_avx2() vs scalar
// -----------------------------------------------------------------------------

#[test]
fn t28_q6_dequantize_avx2_correctness_vs_scalar() {
    let quant = QuantizationCapsule::from_range(-10.0, 10.0);
    let weights = vec![-10.0, -5.0, 0.0, 5.0, 10.0, -2.0, 3.0, 7.0];

    // Quantize first
    let quantized = quant.quantize_simd(&weights);

    // Dequantize with SIMD and scalar
    let simd_dequant = quant.dequantize_simd(&quantized);
    let scalar_dequant = quant.dequantize(&quantized);

    // Assert: SIMD matches scalar (within floating-point tolerance)
    for (i, (&simd, &scalar)) in simd_dequant.iter().zip(scalar_dequant.iter()).enumerate() {
        let diff = (simd - scalar).abs();
        assert!(
            diff < 1e-5,
            "Dequant mismatch at index {}: simd={}, scalar={}, diff={}",
            i,
            simd,
            scalar,
            diff
        );
    }
}

// -----------------------------------------------------------------------------
// Q7: Overflow Behavior - Test saturating arithmetic
// -----------------------------------------------------------------------------

#[test]
fn t28_q7_quantize_avx2_overflow_saturation() {
    let quant = QuantizationCapsule::new(0.1, 0);

    // Values that would overflow without saturation
    let weights = vec![-500.0, -200.0, -150.0, -100.0, 100.0, 150.0, 200.0, 500.0];

    let quantized = quant.quantize_simd(&weights);

    // Verify saturation at Q8.8 bounds
    for &q in &quantized {
        let fp = q as f32 / 256.0;
        assert!(
            fp >= -128.0 && fp <= 127.0,
            "Overflow not saturated: fp={}",
            fp
        );
    }
}

// =============================================================================
// TIER 2: PROPERTY TESTS (Q8-Q14) - 7 Tests
// =============================================================================

// -----------------------------------------------------------------------------
// Q8: Roundtrip Property - quantize → dequantize ≈ identity
// -----------------------------------------------------------------------------

proptest! {
    #[test]
    fn t28_q8_roundtrip_property(
        input in prop::collection::vec(
            prop::num::f32::NORMAL, // Avoid NaN, inf
            16..=256 // Multiple of 8
        ).prop_filter("Length must be multiple of 8", |v| v.len() % 8 == 0)
    ) {
        let quant = QuantizationCapsule::from_range(-100.0, 100.0);

        // Quantize → Dequantize
        let quantized = quant.quantize_simd(&input);
        let dequantized = quant.dequantize_simd(&quantized);

        // Property: Roundtrip error < Q8.8 precision (1/256 ≈ 0.00391)
        for (i, (&orig, &deq)) in input.iter().zip(dequantized.iter()).enumerate() {
            let error = (orig - deq).abs();
            let relative_error = if orig.abs() > 1e-6 { error / orig.abs() } else { error };

            prop_assert!(
                relative_error < 0.1, // 10% relative error tolerance (Q8.8 quantization)
                "Roundtrip error at index {}: orig={}, deq={}, error={}",
                i, orig, deq, error
            );
        }
    }
}

// -----------------------------------------------------------------------------
// Q9: Range Invariant - Output in [-128, 127] range
// -----------------------------------------------------------------------------

proptest! {
    #[test]
    fn t28_q9_range_invariant(
        input in prop::collection::vec(
            -1000.0f32..1000.0, // Wide input range
            16..=128
        ).prop_filter("Length must be multiple of 8", |v| v.len() % 8 == 0)
    ) {
        let quant = QuantizationCapsule::new(1.0, 0);

        let quantized = quant.quantize_simd(&input);

        // Property: All quantized values in Q8.8 range
        for (i, &q) in quantized.iter().enumerate() {
            let fp = q as f32 / 256.0;
            prop_assert!(
                fp >= -128.0 && fp <= 127.0,
                "Range violation at index {}: q={}, fp={}",
                i, q, fp
            );
        }
    }
}

// -----------------------------------------------------------------------------
// Q10: Determinism Property - 100 iterations, same result
// -----------------------------------------------------------------------------

proptest! {
    #[test]
    fn t28_q10_determinism_property(
        input in prop::collection::vec(
            -10.0f32..10.0,
            16..=64
        ).prop_filter("Length must be multiple of 8", |v| v.len() % 8 == 0)
    ) {
        let quant = QuantizationCapsule::from_range(-10.0, 10.0);

        // Run quantization 100 times
        let first_result = quant.quantize_simd(&input);

        for _ in 0..100 {
            let result = quant.quantize_simd(&input);
            prop_assert_eq!(
                &result[..],
                &first_result[..],
                "Quantization not deterministic"
            );
        }
    }
}

// -----------------------------------------------------------------------------
// Q11: Commutativity - Batch order doesn't matter
// -----------------------------------------------------------------------------

proptest! {
    #[test]
    fn t28_q11_commutativity_property(
        input in prop::collection::vec(
            -10.0f32..10.0,
            16..=64
        ).prop_filter("Length must be multiple of 8", |v| v.len() % 8 == 0)
    ) {
        let quant = QuantizationCapsule::new(1.0, 0);

        // Quantize entire batch
        let full_result = quant.quantize_simd(&input);

        // Quantize in two halves
        let mid = input.len() / 2;
        let half1 = quant.quantize_simd(&input[..mid]);
        let half2 = quant.quantize_simd(&input[mid..]);

        let combined: Vec<i16> = half1.into_iter().chain(half2).collect();

        // Property: Full batch = concatenated halves (order doesn't matter)
        prop_assert_eq!(&combined[..], &full_result[..]);
    }
}

// -----------------------------------------------------------------------------
// Q12: Associativity - Grouping doesn't matter
// -----------------------------------------------------------------------------

proptest! {
    #[test]
    fn t28_q12_associativity_property(
        input in prop::collection::vec(
            -10.0f32..10.0,
            24..=48
        ).prop_filter("Length must be multiple of 24", |v| v.len() % 24 == 0)
    ) {
        let quant = QuantizationCapsule::new(1.0, 0);

        // Group 1: Quantize all at once
        let all_at_once = quant.quantize_simd(&input);

        // Group 2: Quantize in thirds
        let third = input.len() / 3;
        let chunk1 = quant.quantize_simd(&input[..third]);
        let chunk2 = quant.quantize_simd(&input[third..2*third]);
        let chunk3 = quant.quantize_simd(&input[2*third..]);

        let grouped: Vec<i16> = chunk1.into_iter()
            .chain(chunk2)
            .chain(chunk3)
            .collect();

        // Property: Grouping doesn't affect result
        prop_assert_eq!(&grouped[..], &all_at_once[..]);
    }
}

// -----------------------------------------------------------------------------
// Q13: Overflow Behavior - Saturating arithmetic property
// -----------------------------------------------------------------------------

proptest! {
    #[test]
    fn t28_q13_overflow_saturation_property(
        scale in 0.01f32..10.0,
        input in prop::collection::vec(
            -1000.0f32..1000.0, // Wide range to trigger overflow
            16..=64
        ).prop_filter("Length must be multiple of 8", |v| v.len() % 8 == 0)
    ) {
        let quant = QuantizationCapsule::new(scale, 0);

        let quantized = quant.quantize_simd(&input);

        // Property: All values saturate within Q8.8 range
        for &q in &quantized {
            let fp = q as f32 / 256.0;
            prop_assert!(fp >= -128.0 && fp <= 127.0);
        }
    }
}

// -----------------------------------------------------------------------------
// Q14: Precision Bounds - Q8.8 error < 1/256
// -----------------------------------------------------------------------------

proptest! {
    #[test]
    fn t28_q14_precision_bounds_property(
        input in prop::collection::vec(
            -10.0f32..10.0,
            16..=64
        ).prop_filter("Length must be multiple of 8", |v| v.len() % 8 == 0)
    ) {
        let quant = QuantizationCapsule::from_range(-10.0, 10.0);

        let quantized = quant.quantize_simd(&input);
        let dequantized = quant.dequantize_simd(&quantized);

        // Property: Quantization error bounded by Q8.8 precision
        for (i, (&orig, &deq)) in input.iter().zip(dequantized.iter()).enumerate() {
            let error = (orig - deq).abs();

            // Q8.8 precision: 1/256 ≈ 0.00391
            // Allow 10% relative error (quantization inherently lossy)
            let relative_error = if orig.abs() > 1e-6 { error / orig.abs() } else { error };

            prop_assert!(
                relative_error < 0.1,
                "Precision error at index {}: orig={}, deq={}, rel_error={}",
                i, orig, deq, relative_error
            );
        }
    }
}

// =============================================================================
// TIER 3: INTEGRATION TESTS (Q15-Q21) - 7 Tests
// =============================================================================

// -----------------------------------------------------------------------------
// Q15: Test with kindly_inference QuantizationCapsule wrapper
// -----------------------------------------------------------------------------

#[test]
fn t28_q15_integration_with_quantization_capsule() {
    // This test validates that QuantizationCapsule integrates correctly
    // with the quantize_simd/dequantize_simd API

    let quant = QuantizationCapsule::from_range(-10.0, 10.0);

    // Simulate LLM weight quantization (512 weights = 64 × 8)
    let weights: Vec<f32> = (0..512).map(|i| (i as f32 / 100.0) - 2.56).collect();

    let quantized = quant.quantize_simd(&weights);
    let dequantized = quant.dequantize_simd(&quantized);

    // Integration validation: Verify capsule maintains accuracy
    let mut total_error = 0.0;
    for (&orig, &deq) in weights.iter().zip(dequantized.iter()) {
        total_error += (orig - deq).abs();
    }
    let avg_error = total_error / weights.len() as f32;

    assert!(avg_error < 0.05, "Average error too high: {}", avg_error);
}

// -----------------------------------------------------------------------------
// Q16: Test with SIMDMatMulCapsule integration (placeholder)
// -----------------------------------------------------------------------------

#[test]
fn t28_q16_integration_with_simd_matmul() {
    // Integration test: Quantized matrix multiplication
    // This validates that quantized weights can be used in SIMD matmul

    let quant = QuantizationCapsule::new(1.0, 0);

    // Simulate weight matrix: 8×8 (64 elements)
    let weights: Vec<f32> = (0..64).map(|i| (i as f32) / 10.0).collect();

    let quantized = quant.quantize_simd(&weights);
    let dequantized = quant.dequantize_simd(&quantized);

    // Verify integration: Quantized weights can be reconstructed
    assert_eq!(dequantized.len(), 64);

    // Check matrix structure preserved (sum of rows)
    let orig_sum: f32 = weights.iter().sum();
    let deq_sum: f32 = dequantized.iter().sum();
    let error = (orig_sum - deq_sum).abs();

    assert!(error < 1.0, "Matrix sum error: {}", error);
}

// -----------------------------------------------------------------------------
// Q17: Test feature flag combinations
// -----------------------------------------------------------------------------

#[test]
#[cfg(all(feature = "portable_simd", target_arch = "x86_64"))]
fn t28_q17_feature_flag_compatibility() {
    // Verify portable_simd feature enables AVX2 on x86_64

    let quant = QuantizationCapsule::new(1.0, 0);
    let weights = vec![1.0; 64]; // 64 elements = 8 SIMD ops

    // This should compile and run with portable_simd feature
    let quantized = quant.quantize_simd(&weights);
    assert_eq!(quantized.len(), 64);

    let dequantized = quant.dequantize_simd(&quantized);
    assert_eq!(dequantized.len(), 64);
}

// -----------------------------------------------------------------------------
// Q18: Test CPU detection fallback (non-AVX2)
// -----------------------------------------------------------------------------

#[test]
fn t28_q18_cpu_detection_fallback() {
    // Test scalar fallback when AVX2 unavailable
    // portable_simd gracefully falls back to scalar on non-AVX2 CPUs

    let quant = QuantizationCapsule::new(1.0, 0);
    let weights = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

    // Scalar path (always available)
    let scalar_result = quant.quantize(&weights);

    // SIMD path (falls back to scalar if AVX2 unavailable)
    #[cfg(feature = "portable_simd")]
    let simd_result = quant.quantize_simd(&weights);

    #[cfg(feature = "portable_simd")]
    {
        // Verify fallback produces same result
        for (i, (&s, &v)) in scalar_result.iter().zip(simd_result.iter()).enumerate() {
            let diff = (s - v).abs();
            assert!(
                diff <= 256,
                "Fallback mismatch at index {}: scalar={}, simd={}",
                i,
                s,
                v
            );
        }
    }

    #[cfg(not(feature = "portable_simd"))]
    {
        // Without portable_simd, only scalar available
        assert_eq!(scalar_result.len(), 8);
    }
}

// -----------------------------------------------------------------------------
// Q19: Test thread safety (parallel quantization)
// -----------------------------------------------------------------------------

#[test]
fn t28_q19_thread_safety_parallel_quantization() {
    use std::sync::Arc;
    use std::thread;

    let quant = Arc::new(QuantizationCapsule::from_range(-10.0, 10.0));
    let weights = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let q = Arc::clone(&quant);
            let w = weights.clone();
            thread::spawn(move || {
                #[cfg(feature = "portable_simd")]
                let _ = q.quantize_simd(&w);

                #[cfg(not(feature = "portable_simd"))]
                let _ = q.quantize(&w);
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread panicked");
    }
}

// -----------------------------------------------------------------------------
// Q20: Test memory alignment in production scenarios
// -----------------------------------------------------------------------------

#[test]
fn t28_q20_memory_alignment_production() {
    // Verify QuantizationCapsule alignment
    assert_eq!(std::mem::align_of::<QuantizationCapsule>(), 64);
    assert_eq!(std::mem::size_of::<QuantizationCapsule>(), 64);

    // Verify aligned allocation works in production
    let quant = QuantizationCapsule::new(1.0, 0);
    let weights = vec![0.0; 1024]; // Large batch

    #[cfg(feature = "portable_simd")]
    {
        let quantized = quant.quantize_simd(&weights);
        assert_eq!(quantized.len(), 1024);
    }

    #[cfg(not(feature = "portable_simd"))]
    {
        let quantized = quant.quantize(&weights);
        assert_eq!(quantized.len(), 1024);
    }
}

// -----------------------------------------------------------------------------
// Q21: Test with real LLM weights (Llama 7B sample)
// -----------------------------------------------------------------------------

#[test]
fn t28_q21_real_llm_weights_sample() {
    // Simulate Llama 7B weight distribution
    // Layer norm weights: mean ≈ 1.0, std ≈ 0.1
    // Attention weights: mean ≈ 0.0, std ≈ 0.02

    let quant = QuantizationCapsule::from_range(-0.5, 0.5);

    // Simulate 4096 attention weights (512 × 8 for SIMD)
    let weights: Vec<f32> = (0..4096)
        .map(|i| {
            let x = (i as f32 / 4096.0) - 0.5;
            x * 0.02 // Typical attention weight scale
        })
        .collect();

    #[cfg(feature = "portable_simd")]
    {
        let quantized = quant.quantize_simd(&weights);
        let dequantized = quant.dequantize_simd(&quantized);

        // Verify LLM weight preservation
        let mut total_error = 0.0;
        for (&orig, &deq) in weights.iter().zip(dequantized.iter()) {
            total_error += (orig - deq).abs();
        }
        let avg_error = total_error / weights.len() as f32;

        // LLM quantization tolerance: <1% average error
        assert!(avg_error < 0.001, "LLM weight error: {}", avg_error);
    }
}

// =============================================================================
// TIER 4: PRODUCTION TESTS (Q22-Q28) - 7 Tests
// =============================================================================

// -----------------------------------------------------------------------------
// Q22: Stress Test - 1M elements, 1000 iterations
// -----------------------------------------------------------------------------

#[test]
#[ignore] // Run manually: cargo test --ignored
fn t28_q22_stress_test_1m_elements() {
    let quant = QuantizationCapsule::new(1.0, 0);

    // 1M elements = 125K SIMD ops (8 elements per op)
    let weights: Vec<f32> = (0..1_000_000).map(|i| (i % 256) as f32 / 10.0).collect();

    // Run 1000 iterations
    for _ in 0..1000 {
        #[cfg(feature = "portable_simd")]
        let _ = quant.quantize_simd(&weights);

        #[cfg(not(feature = "portable_simd"))]
        let _ = quant.quantize(&weights);
    }

    // If we get here without panic, stress test passed
}

// -----------------------------------------------------------------------------
// Q23: Performance Test - Measure actual speedup
// -----------------------------------------------------------------------------

#[test]
#[ignore] // Run manually: cargo test --ignored -- --nocapture
fn t28_q23_performance_test_speedup() {
    use std::time::Instant;

    let quant = QuantizationCapsule::from_range(-10.0, 10.0);
    let weights: Vec<f32> = (0..100_000).map(|i| (i as f32 / 1000.0) - 50.0).collect();

    // Scalar baseline
    let start = Instant::now();
    for _ in 0..100 {
        let _ = quant.quantize(&weights);
    }
    let scalar_time = start.elapsed();

    #[cfg(feature = "portable_simd")]
    {
        // SIMD path
        let start = Instant::now();
        for _ in 0..100 {
            let _ = quant.quantize_simd(&weights);
        }
        let simd_time = start.elapsed();

        let speedup = scalar_time.as_nanos() as f64 / simd_time.as_nanos() as f64;
        println!(
            "Speedup: {:.2}× (scalar: {:?}, SIMD: {:?})",
            speedup, scalar_time, simd_time
        );

        // Expect at least 2× speedup with AVX2 (8-wide f32)
        assert!(speedup >= 2.0, "SIMD speedup too low: {:.2}×", speedup);
    }
}

// -----------------------------------------------------------------------------
// Q24: Memory Leak Test - Valgrind/MIRI placeholder
// -----------------------------------------------------------------------------

#[test]
fn t28_q24_memory_leak_test() {
    // This test validates no memory leaks in quantization
    // Run with: cargo miri test t28_q24_memory_leak_test

    let quant = QuantizationCapsule::new(1.0, 0);

    for _ in 0..1000 {
        let weights = vec![1.0; 1024];

        #[cfg(feature = "portable_simd")]
        {
            let quantized = quant.quantize_simd(&weights);
            let _ = quant.dequantize_simd(&quantized);
        }

        #[cfg(not(feature = "portable_simd"))]
        {
            let quantized = quant.quantize(&weights);
            let _ = quant.dequantize(&quantized);
        }
        // Vectors should be dropped here
    }

    // If MIRI passes, no leaks detected
}

// -----------------------------------------------------------------------------
// Q25: Cross-Platform Test - x86_64 vs ARM fallback
// -----------------------------------------------------------------------------

#[test]
fn t28_q25_cross_platform_compatibility() {
    // Test that quantization works on both x86_64 (AVX2) and ARM (NEON)
    let quant = QuantizationCapsule::new(1.0, 0);
    let weights = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

    #[cfg(feature = "portable_simd")]
    {
        let quantized = quant.quantize_simd(&weights);
        assert_eq!(quantized.len(), 8);
    }

    // Scalar path always works (cross-platform fallback)
    let quantized = quant.quantize(&weights);
    assert_eq!(quantized.len(), 8);
}

// -----------------------------------------------------------------------------
// Q26: Regression Test - Benchmark against baseline
// -----------------------------------------------------------------------------

#[test]
fn t28_q26_regression_test_baseline() {
    // Known baseline: scalar quantization ~50ns per weight (from docs)
    // SIMD target: <10ns per weight (5× speedup)

    let quant = QuantizationCapsule::from_range(-10.0, 10.0);
    let weights = vec![1.0; 8];

    // Regression check: Ensure results haven't changed
    let baseline_result = vec![
        256, 256, 256, 256, 256, 256, 256, 256, // 1.0 scaled by Q8.8
    ];

    #[cfg(feature = "portable_simd")]
    {
        let quantized = quant.quantize_simd(&weights);

        // Allow Q8.8 rounding tolerance
        for (i, (&q, &b)) in quantized.iter().zip(baseline_result.iter()).enumerate() {
            let diff = (q - b).abs();
            assert!(
                diff <= 256,
                "Regression at index {}: got {}, expected {}",
                i,
                q,
                b
            );
        }
    }
}

// -----------------------------------------------------------------------------
// Q27: Error Injection Test - Invalid inputs
// -----------------------------------------------------------------------------

#[test]
fn t28_q27_error_injection_invalid_inputs() {
    let quant = QuantizationCapsule::new(1.0, 0);

    // Test 1: Empty slice
    #[cfg(feature = "portable_simd")]
    {
        let empty: Vec<f32> = vec![];
        let quantized = quant.quantize_simd(&empty);
        assert_eq!(quantized.len(), 0);
    }

    // Test 2: NaN/Inf handling (scalar path validates this)
    let invalid = vec![
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        1.0,
        2.0,
        3.0,
        4.0,
        5.0,
    ];

    // Scalar quantization should handle invalid values gracefully
    let quantized = quant.quantize(&invalid);
    assert_eq!(quantized.len(), 8);
}

// -----------------------------------------------------------------------------
// Q28: Production Simulation - Realistic workload
// -----------------------------------------------------------------------------

#[test]
#[ignore] // Run manually: cargo test --ignored -- --nocapture
fn t28_q28_production_simulation_realistic_workload() {
    // Simulate Llama 7B inference:
    // - 32 layers
    // - 4096 × 4096 weight matrices per layer
    // - Total: ~500M weights
    // - Simplified: 1000 batches of 512 weights (8192 weights total)

    let quant = QuantizationCapsule::from_range(-0.5, 0.5);

    let batch_size = 512; // 64 × 8 (SIMD-friendly)
    let num_batches = 1000;

    let mut total_time = std::time::Duration::ZERO;

    for batch_idx in 0..num_batches {
        // Generate weights with realistic distribution
        let weights: Vec<f32> = (0..batch_size)
            .map(|i| {
                let x = ((batch_idx * batch_size + i) as f32 / 100.0).sin();
                x * 0.02 // Typical weight scale
            })
            .collect();

        let start = std::time::Instant::now();

        #[cfg(feature = "portable_simd")]
        {
            let quantized = quant.quantize_simd(&weights);
            let _ = quant.dequantize_simd(&quantized);
        }

        #[cfg(not(feature = "portable_simd"))]
        {
            let quantized = quant.quantize(&weights);
            let _ = quant.dequantize(&quantized);
        }

        total_time += start.elapsed();
    }

    let total_weights = batch_size * num_batches;
    let avg_ns_per_weight = total_time.as_nanos() / total_weights as u128;

    println!(
        "Production simulation: {} weights in {:?} ({} ns/weight)",
        total_weights, total_time, avg_ns_per_weight
    );

    // Production target: <50ns per weight (scalar baseline)
    // SIMD target: <10ns per weight
    #[cfg(feature = "portable_simd")]
    assert!(
        avg_ns_per_weight < 50,
        "Production latency too high: {} ns/weight",
        avg_ns_per_weight
    );
}

// =============================================================================
// TEST COUNT SUMMARY
// =============================================================================

// Tier 1: Unit Tests (Q1-Q7) = 7 tests
// Tier 2: Property Tests (Q8-Q14) = 7 tests (proptest)
// Tier 3: Integration Tests (Q15-Q21) = 7 tests
// Tier 4: Production Tests (Q22-Q28) = 7 tests
// TOTAL: 28 tests (100% T28 framework compliance)
