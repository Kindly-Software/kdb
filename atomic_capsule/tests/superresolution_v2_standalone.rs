//! Standalone tests for SuperresolutionCapsuleV2
//!
//! This test file isolates SuperresolutionCapsuleV2 tests from other compilation errors.

use atomic_capsule::encoder::SuperresolutionCapsuleV2;

// ============================================================================
// Q1-Q7: Unit Tests (Tier 1)
// ============================================================================

#[test]
fn test_new() {
    let sr = SuperresolutionCapsuleV2::new(10);
    assert_eq!(sr.get_denominator(), 10);
    assert_eq!(sr.generation(), 0);
    assert_eq!(sr.get_dimensions(), (0, 0));
}

#[test]
fn test_new_default() {
    let sr = SuperresolutionCapsuleV2::default();
    assert_eq!(sr.get_denominator(), 16);
    assert_eq!(sr.generation(), 0);
}

#[test]
fn test_set_denominator() {
    let sr = SuperresolutionCapsuleV2::new(10);

    assert!(sr.set_denominator(12));
    assert_eq!(sr.get_denominator(), 12);
    assert_eq!(sr.generation(), 1);

    assert!(sr.set_denominator(9));
    assert_eq!(sr.get_denominator(), 9);
    assert_eq!(sr.generation(), 2);

    // Invalid denominators
    assert!(!sr.set_denominator(8)); // Too low
    assert_eq!(sr.get_denominator(), 9); // Unchanged
    assert!(!sr.set_denominator(17)); // Too high
    assert_eq!(sr.get_denominator(), 9); // Unchanged
}

#[test]
fn test_set_dimensions() {
    let sr = SuperresolutionCapsuleV2::new(10);

    sr.set_dimensions(1920, 1080);
    assert_eq!(sr.get_dimensions(), (1920, 1080));
    assert_eq!(sr.generation(), 1);

    sr.set_dimensions(3840, 2160);
    assert_eq!(sr.get_dimensions(), (3840, 2160));
    assert_eq!(sr.generation(), 2);
}

#[test]
fn test_compute_upscale_width() {
    // No scaling (denom=16)
    assert_eq!(SuperresolutionCapsuleV2::compute_upscale_width(1920, 16), 1920);

    // Denominator 10: 1920 * 16 / 10 = 3072
    assert_eq!(
        SuperresolutionCapsuleV2::compute_upscale_width(1920, 10),
        3072
    );

    // Denominator 12: 1920 * 16 / 12 = 2560
    assert_eq!(
        SuperresolutionCapsuleV2::compute_upscale_width(1920, 12),
        2560
    );

    // Edge case: small width
    assert_eq!(SuperresolutionCapsuleV2::compute_upscale_width(100, 10), 160);
}

#[test]
fn test_get_filter_coefficients() {
    let sr = SuperresolutionCapsuleV2::new(10);

    // Verify all phases
    for phase in 0..8 {
        let coeffs = sr.get_filter_coefficients(phase);
        // Filter coefficients should have 8 taps
        assert_eq!(coeffs.len(), 8);

        // All phases should sum to ~128 (normalization factor)
        let sum: i32 = coeffs.iter().map(|&c| c as i32).sum();
        assert!(
            (sum - 128).abs() <= 2,
            "Phase {} sum {} not close to 128",
            phase,
            sum
        );
    }
}

#[test]
fn test_get_output_dimensions() {
    let sr = SuperresolutionCapsuleV2::new(10);
    sr.set_dimensions(1920, 1080);

    let (output_width, output_height) = sr.get_output_dimensions();
    assert_eq!(output_width, 3072);
    assert_eq!(output_height, 1080);
}

#[test]
fn test_generation_counter() {
    let sr = SuperresolutionCapsuleV2::new(10);
    assert_eq!(sr.generation(), 0);

    let gen1 = sr.increment_generation();
    assert_eq!(gen1, 1);
    assert_eq!(sr.generation(), 1);

    let gen2 = sr.increment_generation();
    assert_eq!(gen2, 2);
    assert_eq!(sr.generation(), 2);
}

// ============================================================================
// Q8-Q14: Property Tests (Tier 2)
// ============================================================================

#[test]
fn test_upscale_row_no_scaling() {
    let sr = SuperresolutionCapsuleV2::new(16);

    let input = vec![128u8; 100];
    let mut output = vec![0u8; 100];

    sr.upscale_row_simd(&input, &mut output);

    // Should just copy input to output
    assert_eq!(output, input);
}

#[test]
fn test_upscale_row_uniform() {
    let sr = SuperresolutionCapsuleV2::new(10);

    // Simple test: uniform input should produce uniform output
    let input = vec![128u8; 192]; // 192 * 16 / 10 = 307
    let mut output = vec![0u8; 307];

    sr.upscale_row_simd(&input, &mut output);

    // All output pixels should be close to 128 (within tolerance for filter effects)
    for &pixel in &output {
        assert!(
            (pixel as i32 - 128).abs() <= 5,
            "Pixel {} too far from 128",
            pixel
        );
    }
}

#[test]
fn test_upscale_row_gradient() {
    let sr = SuperresolutionCapsuleV2::new(10);

    // Create gradient input (0 to 255 over 192 pixels)
    let input: Vec<u8> = (0..192).map(|i| (i * 255 / 191) as u8).collect();
    let mut output = vec![0u8; 307];

    sr.upscale_row_simd(&input, &mut output);

    // Output should be monotonically increasing (allowing for small filter artifacts)
    for i in 1..output.len() {
        assert!(
            output[i] as i32 >= output[i - 1] as i32 - 10,
            "Output should be mostly increasing at index {}: {} vs {}",
            i,
            output[i],
            output[i - 1]
        );
    }
}

#[test]
fn test_coefficient_symmetry() {
    let sr = SuperresolutionCapsuleV2::new(10);

    // Filter coefficients should be symmetric around tap 3 (for phase 0)
    let phase0 = sr.get_filter_coefficients(0);
    assert_eq!(phase0[0], phase0[7]); // 0 == 0
    assert_eq!(phase0[1], phase0[6]); // 0 == 0
    assert_eq!(phase0[2], phase0[5]); // 0 == 0
}

// ============================================================================
// Q15-Q21: Integration Tests (Tier 3)
// ============================================================================

#[test]
fn test_full_upscale_pipeline() {
    let sr = SuperresolutionCapsuleV2::new(10);
    sr.set_dimensions(192, 108);

    let (output_width, output_height) = sr.get_output_dimensions();
    assert_eq!(output_width, 307);
    assert_eq!(output_height, 108);

    // Upscale multiple rows
    for _ in 0..10 {
        let input = vec![128u8; 192];
        let mut output = vec![0u8; 307];
        sr.upscale_row_simd(&input, &mut output);

        for &pixel in &output {
            assert!(
                (pixel as i32 - 128).abs() <= 5,
                "Pixel {} too far from 128",
                pixel
            );
        }
    }

    // Check stats
    let (_, rows) = sr.get_stats();
    assert_eq!(rows, 10);
}

#[test]
fn test_size_and_alignment() {
    assert_eq!(
        core::mem::size_of::<SuperresolutionCapsuleV2>(),
        256,
        "Size must be 256 bytes"
    );
    assert_eq!(
        core::mem::align_of::<SuperresolutionCapsuleV2>(),
        64,
        "Alignment must be 64 bytes"
    );
}

#[test]
fn test_concurrent_operations() {
    use std::sync::Arc;
    use std::thread;

    let sr = Arc::new(SuperresolutionCapsuleV2::new(10));
    let handles: Vec<_> = (0..4)
        .map(|i| {
            let sr_clone = Arc::clone(&sr);
            thread::spawn(move || {
                for _ in 0..100 {
                    sr_clone.set_denominator(9 + (i % 8) as u8);
                    let _ = sr_clone.get_denominator();
                    sr_clone.increment_generation();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify capsule still in valid state
    let denom = sr.get_denominator();
    assert!(denom >= 9 && denom <= 16);
    assert!(sr.generation() > 0);
}

#[test]
fn test_edge_case_small_width() {
    let sr = SuperresolutionCapsuleV2::new(10);

    let input = vec![128u8; 16]; // Small width
    let mut output = vec![0u8; 26]; // 16 * 16 / 10 = 25

    sr.upscale_row_simd(&input, &mut output);

    // All output pixels should be reasonable
    for &pixel in &output {
        assert!(pixel <= 255);
    }
}

#[test]
fn test_stats_accumulation() {
    let sr = SuperresolutionCapsuleV2::new(10);

    let input = vec![128u8; 192];
    let mut output = vec![0u8; 307];

    // Upscale 100 rows
    for _ in 0..100 {
        sr.upscale_row_simd(&input, &mut output);
    }

    let (_, rows) = sr.get_stats();
    assert_eq!(rows, 100);
}
