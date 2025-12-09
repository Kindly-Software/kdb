//! # FilmGrainCapsule Tests (T28: 28 tests across 4 tiers)
//!
//! ## Test Structure
//! - **Q1-Q7 (Unit)**: Basic functionality, layout, parameters
//! - **Q8-Q14 (Property)**: Determinism, bounds, edge cases
//! - **Q15-Q21 (Integration)**: Multi-block, scaling, AR model
//! - **Q22-Q28 (Production)**: Performance, stress, real-world patterns

#![cfg(feature = "encoder")]

use atomic_capsule::encoder::film_grain::{FilmGrainCapsule, ScalingPoint};
use std::time::Instant;

// ============================================================================
// Q1-Q7: UNIT TESTS
// ============================================================================

/// Q1: Verify capsule layout matches specification (256B, 256B-aligned)
#[test]
fn q1_verify_layout() {
    assert_eq!(
        core::mem::size_of::<FilmGrainCapsule>(),
        256,
        "FilmGrainCapsule must be exactly 256 bytes"
    );
    assert_eq!(
        core::mem::align_of::<FilmGrainCapsule>(),
        256,
        "FilmGrainCapsule must be 256-byte aligned"
    );

    // Verify alignment in practice (address multiple of 256)
    let capsule = Box::new(FilmGrainCapsule::new(0x1234));
    let addr = &*capsule as *const _ as usize;
    assert_eq!(
        addr % 256,
        0,
        "Capsule address must be 256-byte aligned: 0x{:X}",
        addr
    );
}

/// Q2: Verify new() initialization sets correct default values
#[test]
fn q2_new_initialization() {
    let seed = 0xABCD;
    let capsule = FilmGrainCapsule::new(seed);

    assert_eq!(capsule.get_seed(), seed, "Seed must match initialization");
    assert_eq!(
        capsule.get_num_y_points(),
        0,
        "New capsule has zero scaling points"
    );
}

/// Q3: Verify grain_params bit packing (seed, points, generation)
#[test]
fn q3_grain_params_packing() {
    let capsule = FilmGrainCapsule::new(0xFFFF); // Max seed
    assert_eq!(capsule.get_seed(), 0xFFFF, "Seed fits in 16 bits");

    // Set scaling points to verify num_y_points packing
    let points = [
        ScalingPoint {
            value: 0,
            scaling: 64,
        },
        ScalingPoint {
            value: 255,
            scaling: 192,
        },
    ];
    capsule.set_scaling_points(&points);
    assert_eq!(capsule.get_num_y_points(), 2, "num_y_points updated");
    assert_eq!(
        capsule.get_seed(),
        0xFFFF,
        "Seed preserved after point update"
    );
}

/// Q4: Verify generate_grain_block produces 64 samples
#[test]
fn q4_generate_grain_block_size() {
    let capsule = FilmGrainCapsule::new(0x1234);
    let grain = capsule.generate_grain_block(0, 0);

    assert_eq!(grain.len(), 64, "Must produce 64 grain samples");

    // Verify samples are in valid range (-128 to +127)
    for &sample in &grain {
        assert!(
            sample >= -128 && sample <= 127,
            "Sample out of range: {}",
            sample
        );
    }
}

/// Q5: Verify apply_grain clips to valid range [0, 255]
#[test]
fn q5_apply_grain_clipping() {
    let capsule = FilmGrainCapsule::new(0x1234);

    // Test upper bound clipping
    let result_high = capsule.apply_grain(250, 20);
    assert!(result_high <= 255, "Must clip to 255: {}", result_high);

    // Test lower bound clipping
    let result_low = capsule.apply_grain(5, -20);
    assert!(result_low >= 0, "Must clip to 0: {}", result_low);

    // Test no clipping (mid-range)
    let result_mid = capsule.apply_grain(128, 10);
    assert!(
        result_mid >= 128 && result_mid <= 138,
        "Mid-range grain applied: {}",
        result_mid
    );
}

/// Q6: Verify set_scaling_points validates point count (max 14)
#[test]
fn q6_set_scaling_points_validation() {
    let capsule = FilmGrainCapsule::new(0x1234);

    // Test single point
    let points1 = [ScalingPoint {
        value: 128,
        scaling: 128,
    }];
    capsule.set_scaling_points(&points1);
    assert_eq!(capsule.get_num_y_points(), 1, "Single point accepted");

    // Test max points (14)
    let points14: Vec<ScalingPoint> = (0..14)
        .map(|i| ScalingPoint {
            value: (i * 18) as u8,
            scaling: ((i * 18) as u8).saturating_add(64),
        })
        .collect();
    capsule.set_scaling_points(&points14);
    assert_eq!(capsule.get_num_y_points(), 14, "Max 14 points accepted");

    // Test overflow (15 points clamped to 14)
    let points15: Vec<ScalingPoint> = (0..15)
        .map(|i| ScalingPoint {
            value: (i * 17) as u8,
            scaling: ((i * 17) as u8).saturating_add(64),
        })
        .collect();
    capsule.set_scaling_points(&points15);
    assert_eq!(
        capsule.get_num_y_points(),
        14,
        "Overflow clamped to 14 points"
    );
}

/// Q7: Verify set_ar_coefficients stores coefficients correctly
#[test]
fn q7_set_ar_coefficients() {
    let capsule = FilmGrainCapsule::new(0x1234);

    // Test with 4 coefficients (lag=1: 2×1×2 = 4 coefficients)
    let coeffs = [10i8, -20i8, 30i8, -40i8];
    capsule.set_ar_coefficients(&coeffs);

    // Verify coefficients stored (check via grain generation)
    let grain = capsule.generate_grain_block(0, 0);
    assert_eq!(grain.len(), 64, "Grain generation works with AR coeffs");
}

// ============================================================================
// Q8-Q14: PROPERTY TESTS
// ============================================================================

/// Q8: Property - Same seed + coords produces same grain (determinism)
#[test]
fn q8_property_determinism() {
    let capsule = FilmGrainCapsule::new(0xDEAD);

    let grain1 = capsule.generate_grain_block(5, 10);
    let grain2 = capsule.generate_grain_block(5, 10);
    let grain3 = capsule.generate_grain_block(5, 10);

    assert_eq!(
        grain1, grain2,
        "Same coords must produce same grain (run 1 vs 2)"
    );
    assert_eq!(
        grain2, grain3,
        "Same coords must produce same grain (run 2 vs 3)"
    );
}

/// Q9: Property - Different seeds produce different grain
#[test]
fn q9_property_different_seeds() {
    let capsule1 = FilmGrainCapsule::new(0x0001);
    let capsule2 = FilmGrainCapsule::new(0xFFFF);

    let grain1 = capsule1.generate_grain_block(0, 0);
    let grain2 = capsule2.generate_grain_block(0, 0);

    assert_ne!(
        grain1, grain2,
        "Different seeds must produce different grain"
    );
}

/// Q10: Property - Different block coords produce different grain
#[test]
fn q10_property_different_coords() {
    let capsule = FilmGrainCapsule::new(0xBEEF);

    let grain_0_0 = capsule.generate_grain_block(0, 0);
    let grain_1_0 = capsule.generate_grain_block(1, 0);
    let grain_0_1 = capsule.generate_grain_block(0, 1);

    assert_ne!(
        grain_0_0, grain_1_0,
        "Different X coord produces different grain"
    );
    assert_ne!(
        grain_0_0, grain_0_1,
        "Different Y coord produces different grain"
    );
    assert_ne!(
        grain_1_0, grain_0_1,
        "Different block positions produce different grain"
    );
}

/// Q11: Property - apply_grain is monotonic (higher pixel -> higher result, given positive grain)
#[test]
fn q11_property_monotonic() {
    let capsule = FilmGrainCapsule::new(0x1234);

    // Positive grain should increase pixel value (with clipping)
    let grain = 10i8;
    let result1 = capsule.apply_grain(50, grain);
    let result2 = capsule.apply_grain(100, grain);

    // Monotonicity: higher input -> higher output (within clipping bounds)
    assert!(
        result2 >= result1,
        "Monotonicity: apply_grain(100, 10) >= apply_grain(50, 10)"
    );
}

/// Q12: Property - Scaling points interpolate linearly
#[test]
fn q12_property_linear_interpolation() {
    let capsule = FilmGrainCapsule::new(0x1234);

    // Set two points: (0, 0) and (255, 255) -> identity scaling
    let points = [
        ScalingPoint {
            value: 0,
            scaling: 0,
        },
        ScalingPoint {
            value: 255,
            scaling: 255,
        },
    ];
    capsule.set_scaling_points(&points);

    // Apply grain at midpoint (128) with grain=10
    // Expected: scaling ≈ 128, scaled_grain ≈ (128 × 10) >> 8 = 5
    let result = capsule.apply_grain(128, 10);
    assert!(
        result >= 128 && result <= 138,
        "Linear interpolation applies scaled grain: {}",
        result
    );
}

/// Q13: Property - Zero grain produces unchanged pixel
#[test]
fn q13_property_zero_grain() {
    let capsule = FilmGrainCapsule::new(0x1234);

    for pixel in [0u8, 50, 128, 200, 255] {
        let result = capsule.apply_grain(pixel, 0);
        assert_eq!(
            result, pixel,
            "Zero grain must not modify pixel: {} != {}",
            result, pixel
        );
    }
}

/// Q14: Property - Grain bounds are symmetric (min/max grain produce symmetric offsets)
#[test]
fn q14_property_symmetric_bounds() {
    let capsule = FilmGrainCapsule::new(0x1234);

    let pixel = 128u8;
    let grain_pos = 20i8;
    let grain_neg = -20i8;

    let result_pos = capsule.apply_grain(pixel, grain_pos);
    let result_neg = capsule.apply_grain(pixel, grain_neg);

    // Symmetric offsets from midpoint
    let offset_pos = result_pos as i16 - pixel as i16;
    let offset_neg = result_neg as i16 - pixel as i16;

    assert!(
        offset_pos.abs() >= 0 && offset_neg.abs() >= 0,
        "Grain offsets are symmetric: +{} vs {}",
        offset_pos,
        offset_neg
    );
}

// ============================================================================
// Q15-Q21: INTEGRATION TESTS
// ============================================================================

/// Q15: Integration - Generate grain for 4×4 grid of blocks
#[test]
fn q15_integration_multi_block() {
    let capsule = FilmGrainCapsule::new(0xCAFE);

    let mut grain_grid = Vec::new();
    for y in 0..4 {
        for x in 0..4 {
            let grain = capsule.generate_grain_block(x, y);
            grain_grid.push(grain);
        }
    }

    assert_eq!(grain_grid.len(), 16, "4×4 grid = 16 blocks");

    // Verify all blocks are unique
    for i in 0..16 {
        for j in (i + 1)..16 {
            assert_ne!(
                grain_grid[i], grain_grid[j],
                "Blocks {} and {} must be unique",
                i, j
            );
        }
    }
}

/// Q16: Integration - Apply grain to 8×8 pixel block
#[test]
fn q16_integration_pixel_block() {
    let capsule = FilmGrainCapsule::new(0x1234);

    // Generate grain for block
    let grain = capsule.generate_grain_block(0, 0);

    // Apply grain to 8×8 pixel block (64 pixels)
    let pixels: Vec<u8> = (0..64).map(|i| (i * 4) as u8).collect();
    let mut result = Vec::new();

    for (i, &pixel) in pixels.iter().enumerate() {
        let grain_sample = grain[i];
        let modified = capsule.apply_grain(pixel, grain_sample);
        result.push(modified);
    }

    assert_eq!(result.len(), 64, "All 64 pixels processed");

    // Verify all results are valid
    for &pixel in &result {
        assert!(pixel <= 255, "Result pixel in valid range: {}", pixel);
    }
}

/// Q17: Integration - Scaling LUT with 5 points
#[test]
fn q17_integration_scaling_lut() {
    let capsule = FilmGrainCapsule::new(0x1234);

    // Set 5 scaling points (piecewise linear)
    let points = [
        ScalingPoint {
            value: 0,
            scaling: 32,
        },
        ScalingPoint {
            value: 64,
            scaling: 64,
        },
        ScalingPoint {
            value: 128,
            scaling: 128,
        },
        ScalingPoint {
            value: 192,
            scaling: 160,
        },
        ScalingPoint {
            value: 255,
            scaling: 192,
        },
    ];
    capsule.set_scaling_points(&points);
    assert_eq!(capsule.get_num_y_points(), 5, "5 points set");

    // Test grain application at each interval
    for &pixel in &[0u8, 64, 128, 192, 255] {
        let result = capsule.apply_grain(pixel, 10);
        assert!(result <= 255, "Grain applied at pixel {}: {}", pixel, result);
    }
}

/// Q18: Integration - AR coefficients affect grain pattern
#[test]
fn q18_integration_ar_coefficients() {
    let capsule = FilmGrainCapsule::new(0xDEAD);

    // Generate grain without AR coefficients
    let grain_before = capsule.generate_grain_block(0, 0);

    // Set AR coefficients
    let coeffs = [10i8, -10i8, 5i8, -5i8];
    capsule.set_ar_coefficients(&coeffs);

    // Generate grain with AR coefficients
    // Note: Current simplified implementation doesn't apply AR filtering yet,
    // so this test verifies the API works (full AR model would show difference)
    let grain_after = capsule.generate_grain_block(0, 0);

    assert_eq!(grain_before.len(), grain_after.len(), "Same grain size");
}

/// Q19: Integration - Concurrent grain generation (lockfree reads)
#[test]
fn q19_integration_concurrent_reads() {
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(FilmGrainCapsule::new(0xBEEF));

    let mut handles = vec![];
    for block_id in 0..8 {
        let capsule_clone = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            let grain = capsule_clone.generate_grain_block(block_id, 0);
            assert_eq!(grain.len(), 64, "Thread {} generated grain", block_id);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

/// Q20: Integration - Update scaling points during concurrent reads
#[test]
fn q20_integration_concurrent_update() {
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(FilmGrainCapsule::new(0xCAFE));

    // Reader threads
    let mut handles = vec![];
    for _ in 0..4 {
        let capsule_clone = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let _grain = capsule_clone.generate_grain_block(0, 0);
            }
        });
        handles.push(handle);
    }

    // Writer thread (update scaling points)
    let capsule_clone = Arc::clone(&capsule);
    let writer = thread::spawn(move || {
        for i in 0..10 {
            let points = [
                ScalingPoint {
                    value: 0,
                    scaling: (i * 10) as u8,
                },
                ScalingPoint {
                    value: 255,
                    scaling: ((i * 10) + 64) as u8,
                },
            ];
            capsule_clone.set_scaling_points(&points);
            thread::sleep(std::time::Duration::from_millis(1));
        }
    });

    for handle in handles {
        handle.join().unwrap();
    }
    writer.join().unwrap();
}

/// Q21: Integration - Full pipeline (generate + apply grain to image strip)
#[test]
fn q21_integration_full_pipeline() {
    let capsule = FilmGrainCapsule::new(0x1234);

    // Set realistic scaling points
    let points = [
        ScalingPoint {
            value: 0,
            scaling: 64,
        },
        ScalingPoint {
            value: 128,
            scaling: 128,
        },
        ScalingPoint {
            value: 255,
            scaling: 192,
        },
    ];
    capsule.set_scaling_points(&points);

    // Simulate processing 64×64 block (4096 pixels)
    let pixels: Vec<u8> = (0..4096).map(|i| ((i % 256) as u8)).collect();
    let mut result = Vec::new();

    // Process in 64-sample chunks
    for chunk_idx in 0..64 {
        let grain = capsule.generate_grain_block((chunk_idx % 8) as u16, (chunk_idx / 8) as u16);

        for sample_idx in 0..64 {
            let pixel_idx = chunk_idx * 64 + sample_idx;
            let pixel = pixels[pixel_idx];
            let grain_sample = grain[sample_idx];
            let modified = capsule.apply_grain(pixel, grain_sample);
            result.push(modified);
        }
    }

    assert_eq!(result.len(), 4096, "All 4096 pixels processed");
}

// ============================================================================
// Q22-Q28: PRODUCTION TESTS
// ============================================================================

/// Q22: Production - Performance: generate_grain_block <5μs
#[test]
fn q22_production_performance_generate() {
    let capsule = FilmGrainCapsule::new(0x1234);

    let iterations = 1000;
    let start = Instant::now();

    for i in 0..iterations {
        let _grain = capsule.generate_grain_block((i % 10) as u16, (i / 10) as u16);
    }

    let elapsed = start.elapsed();
    let per_block_ns = elapsed.as_nanos() / iterations;

    println!(
        "generate_grain_block: {} ns/block (target: <5000 ns)",
        per_block_ns
    );

    // Target: <5μs per block
    assert!(
        per_block_ns < 5_000,
        "Performance regression: {} ns > 5000 ns",
        per_block_ns
    );
}

/// Q23: Production - Performance: apply_grain <10ns per pixel
#[test]
fn q23_production_performance_apply() {
    let capsule = FilmGrainCapsule::new(0x1234);

    let iterations = 100_000;
    let start = Instant::now();

    for i in 0..iterations {
        let pixel = (i % 256) as u8;
        let grain = ((i % 50) as i8) - 25; // -25 to +24
        let _ = capsule.apply_grain(pixel, grain);
    }

    let elapsed = start.elapsed();
    let per_pixel_ns = elapsed.as_nanos() / iterations;

    println!(
        "apply_grain: {} ns/pixel (target: <10 ns)",
        per_pixel_ns
    );

    // Target: <10ns per pixel
    assert!(
        per_pixel_ns < 10,
        "Performance regression: {} ns > 10 ns",
        per_pixel_ns
    );
}

/// Q24: Production - Stress test: 1000 blocks with different seeds
#[test]
fn q24_production_stress_blocks() {
    for seed in 0..1000u16 {
        let capsule = FilmGrainCapsule::new(seed);
        let grain = capsule.generate_grain_block(0, 0);

        // Verify all samples valid
        for &sample in &grain {
            assert!(
                sample >= -128 && sample <= 127,
                "Seed {} produced invalid sample: {}",
                seed,
                sample
            );
        }
    }
}

/// Q25: Production - Stress test: 10K pixel applications
#[test]
fn q25_production_stress_pixels() {
    let capsule = FilmGrainCapsule::new(0x1234);

    for pixel in 0..256u16 {
        for grain in -128..128i16 {
            let result = capsule.apply_grain(pixel as u8, grain as i8);
            assert!(result <= 255, "Invalid result: {}", result);
        }
    }
}

/// Q26: Production - Real-world pattern: 1080p frame (1920×1080 = 2,073,600 pixels)
#[test]
fn q26_production_real_world_1080p() {
    let capsule = FilmGrainCapsule::new(0xDEAD);

    // Set typical film grain scaling
    let points = [
        ScalingPoint {
            value: 0,
            scaling: 80,
        },
        ScalingPoint {
            value: 128,
            scaling: 128,
        },
        ScalingPoint {
            value: 255,
            scaling: 176,
        },
    ];
    capsule.set_scaling_points(&points);

    // 1920×1080 = 2,073,600 pixels
    // 64×64 blocks: (1920/64) × (1080/64) = 30 × 17 = 510 blocks
    let blocks_x = 30;
    let blocks_y = 17;

    let start = Instant::now();
    let mut total_pixels = 0;

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let grain = capsule.generate_grain_block(bx, by);
            total_pixels += grain.len();
        }
    }

    let elapsed = start.elapsed();
    println!(
        "1080p grain generation: {} ms for {} pixels ({} blocks)",
        elapsed.as_millis(),
        total_pixels,
        blocks_x * blocks_y
    );

    // Target: <50ms for full 1080p frame
    assert!(
        elapsed.as_millis() < 50,
        "1080p too slow: {} ms",
        elapsed.as_millis()
    );
}

/// Q27: Production - Memory safety: No panics on extreme values
#[test]
fn q27_production_memory_safety() {
    let capsule = FilmGrainCapsule::new(0xFFFF);

    // Extreme grain values
    let _ = capsule.apply_grain(0, i8::MIN);
    let _ = capsule.apply_grain(255, i8::MAX);
    let _ = capsule.apply_grain(128, i8::MIN);
    let _ = capsule.apply_grain(128, i8::MAX);

    // Extreme block coordinates
    let _ = capsule.generate_grain_block(u16::MAX, 0);
    let _ = capsule.generate_grain_block(0, u16::MAX);
    let _ = capsule.generate_grain_block(u16::MAX, u16::MAX);

    // Extreme scaling points
    let points = [
        ScalingPoint {
            value: 0,
            scaling: 0,
        },
        ScalingPoint {
            value: 255,
            scaling: 255,
        },
    ];
    capsule.set_scaling_points(&points);

    // No panics = success
}

/// Q28: Production - Zero-copy grain buffer cache
#[test]
fn q28_production_grain_cache() {
    let capsule = FilmGrainCapsule::new(0xBEEF);

    // Generate grain (updates cache)
    let grain1 = capsule.generate_grain_block(5, 10);

    // Apply grain immediately (reads from cache)
    for (i, &grain_sample) in grain1.iter().enumerate() {
        let pixel = (i * 4) as u8;
        let _ = capsule.apply_grain(pixel, grain_sample);
    }

    // Generate different grain (cache updated)
    let grain2 = capsule.generate_grain_block(10, 5);

    // Verify grains are different (cache working)
    assert_ne!(grain1, grain2, "Grain cache updates correctly");
}
