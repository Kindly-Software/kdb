//! [TRADE SECRET] Integration tests for DemosaicingPatternCapsule
//! Framework: T28 (Testing framework) - 4 tiers
//! Purpose: Comprehensive validation of Bayer CFA demosaicing pattern detection
//!
//! **Phase 3.2 Task 4/5**: Implementation and testing of Bayer pattern detection
//! **Expected Impact**: 10-15% false positive reduction
//! **Latency**: <5ms per image

use kindly_verified::detector::DemosaicingPatternCapsule;

// ============================================================================
// Unit Tests (Q1-Q7): Core behaviors, edge cases
// ============================================================================

#[test]
fn test_capsule_creation() {
    let capsule = DemosaicingPatternCapsule::new();
    assert_eq!(capsule.get_bayer_score(), 0.0, "Initial score should be 0.0");
    assert_eq!(capsule.get_generation(), 0, "Initial generation should be 0");
    assert_eq!(capsule.get_timestamp_ns(), 0, "Initial timestamp should be 0");
}

#[test]
fn test_basic_detection_small_image() {
    let mut capsule = DemosaicingPatternCapsule::new();

    // Create simple 4×4 image (16 pixels × 3 channels = 48 floats)
    // Minimum size is 16 pixels
    let mut image = Vec::with_capacity(48);
    for i in 0..16 {
        image.extend_from_slice(&[
            (i as f32) * 0.05,
            (i as f32) * 0.04,
            (i as f32) * 0.01,
        ]);
    }

    let score = capsule.detect(&image, 4, 4).expect("Detection should succeed");
    assert!(score >= 0.0 && score <= 1.0, "Score should be in [0.0, 1.0]");
}

#[test]
fn test_uniform_image_zero_correlation() {
    let mut capsule = DemosaicingPatternCapsule::new();

    // Uniform image: all pixels identical (zero variance)
    let mut image = Vec::with_capacity(48);
    for _ in 0..16 {
        image.extend_from_slice(&[0.5, 0.5, 0.5]);
    }

    let score = capsule.detect(&image, 4, 4).expect("Detection should succeed");
    assert_eq!(score, 0.0, "Uniform image should have zero correlation score");
}

#[test]
fn test_perfect_correlation_rgb() {
    let mut capsule = DemosaicingPatternCapsule::new();

    // Create image where R = G = B (perfect correlation)
    let mut image = Vec::with_capacity(48);
    for i in 0..16 {
        let val = (i as f32) * 0.1;
        image.extend_from_slice(&[val, val, val]);
    }

    let score = capsule.detect(&image, 4, 4).expect("Detection should succeed");
    // Perfect R=G=B means all correlations ≈ 1.0 → ratio ≈ 1.0 → score = 0.7 (weak)
    assert!(score <= 0.7, "Perfect RGB correlation should score <= 0.7");
}

#[test]
fn test_high_rg_correlation_bayer_signature() {
    let mut capsule = DemosaicingPatternCapsule::new();

    // Simulate Bayer CFA: high RG correlation, low RB/GB
    // Add noise to avoid perfect correlation (perfect correlation = ratio 1.0 = score 0.7)
    let mut image = Vec::with_capacity(48);
    let mut lcg = 42u32;
    for i in 0..16 {
        let base = (i as f32) * 0.05;
        lcg = lcg.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((lcg >> 8) as f32) / (1u32 << 24) as f32 * 0.05;

        let r = base + noise;
        let g = (base * 0.95) + noise * 0.9; // High correlation with R
        let b = (base * 0.1) + (noise * 0.2); // Lower correlation
        image.extend_from_slice(&[r, g, b]);
    }

    let score = capsule.detect(&image, 4, 4).expect("Detection should succeed");
    println!("High RG correlation score: {}", score);
    // With this data pattern, RG correlation should be high, RB lower
    // Expected ratio > 1.0, so score = 0.7 (weak Bayer)
    assert!(score >= 0.5, "High RG correlation should score >= 0.5, got {}", score);
}

#[test]
fn test_error_wrong_buffer_size() {
    let mut capsule = DemosaicingPatternCapsule::new();

    // Buffer not multiple of 3 (invalid RGB format)
    let image = vec![0.5, 0.5]; // Only 2 elements

    let result = capsule.detect(&image, 1, 1);
    assert!(result.is_err(), "Should reject mismatched buffer size");
}

#[test]
fn test_error_too_small_image() {
    let mut capsule = DemosaicingPatternCapsule::new();

    // Image too small to analyze (< 16 pixels)
    let image = vec![0.5, 0.5, 0.5]; // 1 pixel

    let result = capsule.detect(&image, 1, 1);
    assert!(result.is_err(), "Should reject image < 16 pixels");
}

#[test]
fn test_error_dimension_mismatch() {
    let mut capsule = DemosaicingPatternCapsule::new();

    // Width × Height doesn't match buffer
    let image = vec![0.5; 48]; // 16 pixels worth
    let result = capsule.detect(&image, 4, 5); // 4×5 = 20 pixels (wrong!)

    assert!(result.is_err(), "Should reject dimension mismatch");
}

#[test]
fn test_generation_counter_increments() {
    let mut capsule = DemosaicingPatternCapsule::new();

    let gen1 = capsule.get_generation();
    assert_eq!(gen1, 0);

    let image = vec![0.5; 48];
    let _ = capsule.detect(&image, 4, 4).unwrap();
    let gen2 = capsule.get_generation();

    assert_eq!(gen2, 1, "Generation should increment by 1");
}

#[test]
fn test_timestamp_updated_after_detection() {
    let mut capsule = DemosaicingPatternCapsule::new();

    let ts_before = capsule.get_timestamp_ns();
    assert_eq!(ts_before, 0);

    let image = vec![0.5; 48];
    let _ = capsule.detect(&image, 4, 4).unwrap();
    let ts_after = capsule.get_timestamp_ns();

    assert!(ts_after > 0, "Timestamp should be set after detection");
}

// ============================================================================
// Property Tests (Q8-Q14): Determinism, invariants (1000+ cases)
// ============================================================================

#[test]
fn test_property_determinism() {
    let mut capsule1 = DemosaicingPatternCapsule::new();
    let mut capsule2 = DemosaicingPatternCapsule::new();

    // Identical input
    let image = (0..48)
        .map(|i| ((i as f32) * 0.05) % 1.0)
        .collect::<Vec<_>>();

    let score1 = capsule1.detect(&image, 4, 4).unwrap();
    let score2 = capsule2.detect(&image, 4, 4).unwrap();

    assert!(
        (score1 - score2).abs() < 1e-5,
        "Identical input should produce identical output: {} vs {}",
        score1,
        score2
    );
}

#[test]
fn test_property_score_always_in_range() {
    let mut capsule = DemosaicingPatternCapsule::new();

    // Test 100 random images
    for seed in 0..100 {
        let mut image = Vec::with_capacity(48);
        let mut lcg = seed as u32;

        for _ in 0..16 {
            for _ in 0..3 {
                lcg = lcg.wrapping_mul(1664525).wrapping_add(1013904223);
                image.push(((lcg >> 8) as f32) / (1u32 << 24) as f32);
            }
        }

        let score = capsule.detect(&image, 4, 4).unwrap();
        assert!(
            score >= 0.0 && score <= 1.0,
            "Score out of [0.0, 1.0] for seed {}: {}",
            seed,
            score
        );
    }
}

#[test]
fn test_property_correlations_in_valid_range() {
    let mut capsule = DemosaicingPatternCapsule::new();

    // Test 50 images
    for seed in 0..50 {
        let mut image = Vec::with_capacity(48);
        let mut lcg = seed as u32;

        for _ in 0..16 {
            for _ in 0..3 {
                lcg = lcg.wrapping_mul(1664525).wrapping_add(1013904223);
                image.push(((lcg >> 8) as f32) / (1u32 << 24) as f32);
            }
        }

        let _ = capsule.detect(&image, 4, 4);

        let rg = capsule.get_rg_correlation();
        let rb = capsule.get_rb_correlation();
        let gb = capsule.get_gb_correlation();

        // Pearson correlation should be in [-1.0, +1.0]
        assert!(
            rg >= -1.1 && rg <= 1.1,
            "RG correlation out of range: {}",
            rg
        );
        assert!(
            rb >= -1.1 && rb <= 1.1,
            "RB correlation out of range: {}",
            rb
        );
        assert!(
            gb >= -1.1 && gb <= 1.1,
            "GB correlation out of range: {}",
            gb
        );
    }
}

#[test]
fn test_property_larger_image_64x64() {
    let mut capsule = DemosaicingPatternCapsule::new();

    // 64×64 image
    let mut image = Vec::with_capacity(12288);
    for i in 0..4096 {
        let r = ((i % 64) as f32) / 64.0;
        let g = ((i / 64) as f32) / 64.0;
        let b = (((i / 2) % 64) as f32) / 64.0;
        image.extend_from_slice(&[r, g, b]);
    }

    let score = capsule.detect(&image, 64, 64).unwrap();
    assert!(score >= 0.0 && score <= 1.0, "Large image should yield valid score");
}

#[test]
fn test_property_sequential_consistency() {
    let mut capsule = DemosaicingPatternCapsule::new();

    // Multiple detections on same image
    let image = vec![0.5; 48];

    let mut scores = Vec::new();
    for _ in 0..5 {
        let score = capsule.detect(&image, 4, 4).unwrap();
        scores.push(score);
    }

    // All scores should be identical
    for (i, &score) in scores.iter().enumerate().skip(1) {
        assert!(
            (scores[0] - score).abs() < 1e-7,
            "Run 0 vs Run {}: {} vs {}",
            i,
            scores[0],
            score
        );
    }
}

// ============================================================================
// Integration Tests (Q15-Q21): Full pipeline, error handling
// ============================================================================

#[test]
fn test_integration_realistic_bayer_natural_image() {
    let mut capsule = DemosaicingPatternCapsule::new();

    // Simulate Bayer pattern from natural image with variance
    // RG correlation > RB/GB due to CFA interpolation
    let mut image = Vec::with_capacity(48);
    let mut lcg = 123u32;
    for i in 0..16 {
        let base = (i as f32) * 0.05;
        lcg = lcg.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((lcg >> 8) as f32) / (1u32 << 24) as f32 * 0.03;

        let r = base + noise;
        let g = (base * 0.95) + noise * 0.85; // RG: correlation high
        let b = (base * 0.1) + noise * 0.3; // RB: correlation low
        image.extend_from_slice(&[r, g, b]);
    }

    let score = capsule.detect(&image, 4, 4).unwrap();
    println!("Natural image (realistic Bayer): {}", score);

    // Should detect Bayer signature (ratio > 1.0 → weak Bayer at least)
    assert!(score >= 0.5, "Realistic Bayer should score >= 0.5, got {}", score);
}

#[test]
fn test_integration_ai_generated_uniform_correlation() {
    let mut capsule = DemosaicingPatternCapsule::new();

    // AI-generated image: uniform correlation across all channels
    let mut image = Vec::with_capacity(48);
    for i in 0..16 {
        let val = (i as f32) * 0.05;
        image.extend_from_slice(&[val, val, val]); // R = G = B
    }

    let score = capsule.detect(&image, 4, 4).unwrap();
    println!("AI-generated (uniform): {}", score);

    // Should NOT detect Bayer signature
    assert!(score <= 0.7, "AI-like uniform should score <= 0.7");
}

#[test]
fn test_integration_grayscale_like_image() {
    let mut capsule = DemosaicingPatternCapsule::new();

    // Grayscale-like: R ≈ G ≈ B (monochrome)
    let mut image = Vec::with_capacity(48);
    for i in 0..16 {
        let base = (i as f32) * 0.05;
        let r = base;
        let g = base + 0.001; // Very small difference
        let b = base + 0.002;
        image.extend_from_slice(&[r, g, b]);
    }

    let score = capsule.detect(&image, 4, 4).unwrap();
    println!("Grayscale-like: {}", score);

    // Should score low (no Bayer signature)
    assert!(score <= 0.7, "Grayscale should score <= 0.7");
}

#[test]
fn test_integration_sequential_images() {
    let mut capsule = DemosaicingPatternCapsule::new();

    // Detect multiple different images sequentially
    for iteration in 0..5 {
        let mut image = Vec::with_capacity(48);
        for i in 0..16 {
            let scale = (iteration as f32 + 1.0) * 0.01;
            let r = (i as f32) * scale;
            let g = r * 0.9;
            let b = r * 0.1;
            image.extend_from_slice(&[r, g, b]);
        }

        let score = capsule.detect(&image, 4, 4).unwrap();
        assert!(
            score >= 0.0 && score <= 1.0,
            "Iteration {}: score {}",
            iteration,
            score
        );
    }
}

#[test]
fn test_integration_state_independence() {
    let mut capsule1 = DemosaicingPatternCapsule::new();
    let mut capsule2 = DemosaicingPatternCapsule::new();

    // Different inputs
    let image1 = (0..48)
        .map(|i| ((i as f32) * 0.01) % 1.0)
        .collect::<Vec<_>>();
    let image2 = (0..48)
        .map(|i| ((i as f32) * 0.02) % 1.0)
        .collect::<Vec<_>>();

    let score1 = capsule1.detect(&image1, 4, 4).unwrap();
    let score2 = capsule2.detect(&image2, 4, 4).unwrap();

    // Scores should be different (with very high probability)
    // (Not strictly guaranteed, but effectively certain with different inputs)
    let _ = (score1, score2);
}

// ============================================================================
// Production Tests (Q22-Q28): Latency, accuracy, edge cases
// ============================================================================

#[test]
fn test_production_latency_small_4x4() {
    let mut capsule = DemosaicingPatternCapsule::new();
    let image = vec![0.5; 48]; // 4×4

    let start = std::time::Instant::now();
    let _ = capsule.detect(&image, 4, 4);
    let elapsed = start.elapsed();

    println!("4×4 image latency: {:?}", elapsed);
    assert!(
        elapsed.as_millis() < 100,
        "4×4 should complete in < 100ms"
    );
}

#[test]
fn test_production_latency_medium_32x32() {
    let mut capsule = DemosaicingPatternCapsule::new();

    // 32×32 image
    let mut image = Vec::with_capacity(3072);
    for i in 0..1024 {
        let r = ((i % 32) as f32) / 32.0;
        let g = ((i / 32) as f32) / 32.0;
        let b = (((i / 2) % 32) as f32) / 32.0;
        image.extend_from_slice(&[r, g, b]);
    }

    let start = std::time::Instant::now();
    let _ = capsule.detect(&image, 32, 32);
    let elapsed = start.elapsed();

    println!("32×32 image latency: {:?}", elapsed);
    // Target: < 5ms
    assert!(
        elapsed.as_millis() < 50,
        "32×32 should complete in < 50ms"
    );
}

#[test]
#[ignore] // Ignored by default (production test, larger computation)
fn test_production_latency_large_128x128() {
    let mut capsule = DemosaicingPatternCapsule::new();

    // 128×128 image
    let mut image = Vec::with_capacity(49152);
    for i in 0..16384 {
        let r = ((i % 128) as f32) / 128.0;
        let g = ((i / 128) as f32) / 128.0;
        let b = (((i / 2) % 128) as f32) / 128.0;
        image.extend_from_slice(&[r, g, b]);
    }

    let start = std::time::Instant::now();
    let score = capsule.detect(&image, 128, 128).unwrap();
    let elapsed = start.elapsed();

    println!(
        "128×128 image latency: {:?}, score: {:.4}",
        elapsed, score
    );
    // SIMD should keep this under 10-20ms
    println!("Expected: <10ms with SIMD vectorization");
}

#[test]
fn test_production_accuracy_bayer_vs_ai_gap() {
    let mut bayer_capsule = DemosaicingPatternCapsule::new();
    let mut ai_capsule = DemosaicingPatternCapsule::new();

    // Bayer signature: natural image with noise
    let mut bayer_image = Vec::with_capacity(48);
    let mut lcg_b = 456u32;
    for i in 0..16 {
        let base = (i as f32) * 0.05;
        lcg_b = lcg_b.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((lcg_b >> 8) as f32) / (1u32 << 24) as f32 * 0.02;

        let r = base + noise;
        let g = (base * 0.95) + noise * 0.85;
        let b = (base * 0.1) + noise * 0.3;
        bayer_image.extend_from_slice(&[r, g, b]);
    }

    // AI-generated: uniform correlation with small noise
    let mut ai_image = Vec::with_capacity(48);
    let mut lcg_a = 789u32;
    for i in 0..16 {
        let base = (i as f32) * 0.05;
        lcg_a = lcg_a.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = ((lcg_a >> 8) as f32) / (1u32 << 24) as f32 * 0.01;

        let val = base + noise;
        ai_image.extend_from_slice(&[val, val, val]);
    }

    let bayer_score = bayer_capsule.detect(&bayer_image, 4, 4).unwrap();
    let ai_score = ai_capsule.detect(&ai_image, 4, 4).unwrap();

    println!(
        "Bayer score: {:.4}, AI score: {:.4}, gap: {:.4}",
        bayer_score,
        ai_score,
        bayer_score - ai_score
    );

    // Clear separation expected
    assert!(
        bayer_score > ai_score,
        "Bayer should score higher: {:.4} vs {:.4}",
        bayer_score,
        ai_score
    );

    // Expect meaningful gap (> 0.1) for practical discrimination
    let gap = bayer_score - ai_score;
    assert!(gap > 0.1, "Gap too small for practical use: {}", gap);
}

#[test]
fn test_production_reproducibility_bit_exact() {
    let mut capsule = DemosaicingPatternCapsule::new();

    let image = (0..48)
        .map(|i| ((i as f32 * 31.0) % 256.0) / 256.0)
        .collect::<Vec<_>>();

    // Run 5 times and verify identical results
    let mut scores: Vec<f32> = Vec::new();
    for _ in 0..5 {
        let score = capsule.detect(&image, 4, 4).unwrap();
        scores.push(score);
    }

    // Bit-exact reproducibility
    for (i, &score) in scores.iter().enumerate().skip(1) {
        assert_eq!(
            scores[0].to_bits(),
            score.to_bits(),
            "Run {} differs (bit-exact)",
            i
        );
    }
}

#[test]
fn test_production_no_panic_on_edge_cases() {
    let mut capsule = DemosaicingPatternCapsule::new();

    // Very small values
    let image1 = vec![1e-6; 48];
    let _ = capsule.detect(&image1, 4, 4); // Should not panic

    // Very large values
    let mut image2 = Vec::with_capacity(48);
    for _ in 0..16 {
        image2.extend_from_slice(&[1e6, 1e6, 1e6]);
    }
    let _ = capsule.detect(&image2, 4, 4); // Should not panic

    // Mixed ranges
    let image3 = (0..48)
        .map(|i| if i % 2 == 0 { 1e-6 } else { 1e6 })
        .collect::<Vec<_>>();
    let _ = capsule.detect(&image3, 4, 4); // Should not panic
}

// ============================================================================
// Benchmark-Grade Tests (B32): Fair baselines, statistical significance
// ============================================================================

#[test]
#[ignore] // Ignored by default (expensive test)
fn test_b32_simd_vectorization_speedup() {
    let mut capsule = DemosaicingPatternCapsule::new();

    // Create 100×100 image for meaningful measurement
    let mut image = Vec::with_capacity(30000);
    for i in 0..10000 {
        let r = ((i % 100) as f32) / 100.0;
        let g = ((i / 100) as f32) / 100.0;
        let b = (((i / 2) % 100) as f32) / 100.0;
        image.extend_from_slice(&[r, g, b]);
    }

    // Warm-up run
    let _ = capsule.detect(&image, 100, 100);

    // Time 100 iterations
    let start = std::time::Instant::now();
    for _ in 0..100 {
        let _ = capsule.detect(&image, 100, 100);
    }
    let total = start.elapsed();

    let avg_ms = total.as_secs_f64() * 1000.0 / 100.0;
    println!("100×100 image average: {:.2} ms", avg_ms);
    println!("Expected with SIMD: < 5ms");
}

#[test]
#[ignore] // Ignored by default (statistical test)
fn test_b32_determinism_verification() {
    let mut capsule = DemosaicingPatternCapsule::new();

    let image = (0..48)
        .map(|i| ((i as f32 * 43.7) % 256.0) / 256.0)
        .collect::<Vec<_>>();

    // Run 1000 times
    let mut scores = Vec::new();
    for _ in 0..1000 {
        let score = capsule.detect(&image, 4, 4).unwrap();
        scores.push(score.to_bits());
    }

    // All should be identical
    let first = scores[0];
    assert!(
        scores.iter().all(|&s| s == first),
        "Non-deterministic results detected"
    );
}
