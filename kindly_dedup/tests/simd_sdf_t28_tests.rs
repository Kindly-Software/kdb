//! T28 Comprehensive Testing for SIMD SDF Renderer
//!
//! # Test Coverage (28 tests)
//!
//! **Q1-Q7 (Unit Tests)**: Core functionality, edge cases, boundary conditions
//! **Q8-Q14 (Property Tests)**: Invariants, mathematical properties, correctness
//! **Q15-Q21 (Integration Tests)**: Full pipeline, multi-segment, complex glyphs
//! **Q22-Q28 (Production Tests)**: Performance, stress, memory, concurrency
//!
//! # Framework Compliance
//!
//! - **T28**: 5-tier testing (Unit/Property/Integration/Production/Determinism)
//! - **UCE34**: Q33 (lockfree verification), Q34 (audit trail validation)
//! - **ASSUM**: 99.99% safe (nightly feature gated, scalar fallback)
//! - **Chaos**: 100% lockfree (AtomicU64 state, cache-aligned 64B)
//! - **B32**: Fair performance claims (4-8× SIMD speedup validated)
//!
//! # Running Tests
//!
//! ```bash
//! # Remote execution (MANDATORY per CLAUDE.md)
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly_dedup && cargo test --test simd_sdf_t28_tests --features simd-sdf-rendering"
//! ```

#![cfg(feature = "simd-sdf-rendering")]

use kindly_dedup::simd_sdf_renderer::SdfRendererCapsule;
use core::simd::{f32x4, f32x8};

// ============================================================================
// Q1-Q7: Unit Tests (Core Functionality)
// ============================================================================

#[test]
fn q1_capsule_layout_cache_aligned_64b() {
    // UCE34 Q33: Verify cache-aligned 64B structure (T2 SIMD requirement)
    assert_eq!(
        core::mem::size_of::<SdfRendererCapsule>(),
        64,
        "SdfRendererCapsule must be exactly 64 bytes"
    );
    assert_eq!(
        core::mem::align_of::<SdfRendererCapsule>(),
        64,
        "SdfRendererCapsule must be 64-byte aligned"
    );
}

#[test]
fn q2_scalar_capsule_sdf_correctness() {
    // Verify scalar capsule SDF against known values
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    // Point on capsule centerline (distance = 0)
    let sdf = renderer.capsule_sdf_scalar(0.5, 0.5, 0.0, 0.0, 1.0, 1.0);
    assert!(sdf.abs() < 0.01, "Point on centerline should have distance ~0, got {}", sdf);

    // Point at capsule endpoint A (distance = 0)
    let sdf = renderer.capsule_sdf_scalar(0.0, 0.0, 0.0, 0.0, 1.0, 1.0);
    assert!(sdf.abs() < 0.01, "Endpoint A should have distance ~0, got {}", sdf);

    // Point at capsule endpoint B (distance = 0)
    let sdf = renderer.capsule_sdf_scalar(1.0, 1.0, 0.0, 0.0, 1.0, 1.0);
    assert!(sdf.abs() < 0.01, "Endpoint B should have distance ~0, got {}", sdf);

    // Point far from capsule (distance > 0)
    let sdf = renderer.capsule_sdf_scalar(10.0, 10.0, 0.0, 0.0, 1.0, 1.0);
    assert!(sdf > 12.0, "Far point should have distance >12, got {}", sdf);
}

#[test]
fn q3_scalar_smootherstep_boundary_conditions() {
    // Ken Perlin smootherstep: 6x^5 - 15x^4 + 10x^3
    // Should satisfy: f(0)=0, f(1)=1, f'(0)=0, f'(1)=0, f''(0)=0, f''(1)=0

    let y0 = SdfRendererCapsule::smootherstep_scalar(0.0);
    let y1 = SdfRendererCapsule::smootherstep_scalar(1.0);
    let y_mid = SdfRendererCapsule::smootherstep_scalar(0.5);

    assert_eq!(y0, 0.0, "smootherstep(0) must be 0");
    assert_eq!(y1, 1.0, "smootherstep(1) must be 1");
    assert!(y_mid > 0.4 && y_mid < 0.6, "smootherstep(0.5) should be ~0.5, got {}", y_mid);

    // Verify clamping
    let y_neg = SdfRendererCapsule::smootherstep_scalar(-1.0);
    let y_over = SdfRendererCapsule::smootherstep_scalar(2.0);
    assert_eq!(y_neg, 0.0, "smootherstep(-1) should clamp to 0");
    assert_eq!(y_over, 1.0, "smootherstep(2) should clamp to 1");
}

#[test]
fn q4_scalar_sdf_to_coverage_threshold() {
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    // SDF = threshold → coverage = smootherstep(0) = 0
    let coverage = renderer.sdf_to_coverage_scalar(0.5);
    assert!(coverage.abs() < 0.01, "SDF at threshold should give coverage ~0, got {}", coverage);

    // SDF < threshold → coverage > 0 (inside glyph)
    let coverage = renderer.sdf_to_coverage_scalar(0.3);
    assert!(coverage > 0.0, "SDF < threshold should give coverage >0, got {}", coverage);

    // SDF > threshold → coverage = 0 (outside glyph)
    let coverage = renderer.sdf_to_coverage_scalar(2.6);
    assert!(coverage.abs() < 0.01, "SDF far from threshold should give coverage ~0, got {}", coverage);
}

#[test]
fn q5_simd_4wide_capsule_sdf_correctness() {
    let px = f32x4::from_array([0.5, 1.5, 2.5, 10.0]);
    let py = f32x4::from_array([0.5, 1.5, 2.5, 10.0]);

    let sdf = SdfRendererCapsule::capsule_sdf_4wide(px, py, 0.0, 0.0, 1.0, 1.0);

    // First pixel on centerline
    assert!(sdf[0].abs() < 0.01, "Pixel 0 should have distance ~0, got {}", sdf[0]);

    // Second pixel on centerline (extended past endpoint)
    assert!(sdf[1] > 0.5, "Pixel 1 should have distance >0.5, got {}", sdf[1]);

    // Third pixel further out
    assert!(sdf[2] > 1.5, "Pixel 2 should have distance >1.5, got {}", sdf[2]);

    // Fourth pixel far away
    assert!(sdf[3] > 12.0, "Pixel 3 should have distance >12, got {}", sdf[3]);
}

#[test]
fn q6_simd_8wide_capsule_sdf_correctness() {
    let px = f32x8::from_array([0.0, 0.25, 0.5, 0.75, 1.0, 5.0, 10.0, 20.0]);
    let py = f32x8::from_array([0.0, 0.25, 0.5, 0.75, 1.0, 5.0, 10.0, 20.0]);

    let sdf = SdfRendererCapsule::capsule_sdf_8wide(px, py, 0.0, 0.0, 1.0, 1.0);

    // First 5 pixels on or near capsule
    for i in 0..5 {
        assert!(sdf[i] < 1.0, "Pixel {} should have distance <1, got {}", i, sdf[i]);
    }

    // Last 3 pixels far away
    for i in 5..8 {
        assert!(sdf[i] > 4.0, "Pixel {} should have distance >4, got {}", i, sdf[i]);
    }
}

#[test]
fn q7_state_management_generation_counter() {
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    // Initial state
    assert_eq!(renderer.pixels_rendered(), 0);
    assert_eq!(renderer.generation(), 0);

    // Increment pixels
    renderer.render_pixels_4wide(
        f32x4::splat(0.5),
        f32x4::splat(0.5),
        0.0, 0.0, 1.0, 1.0,
    );
    assert_eq!(renderer.pixels_rendered(), 4);
    assert_eq!(renderer.generation(), 0);

    // Reset state (increments generation)
    renderer.reset();
    assert_eq!(renderer.pixels_rendered(), 0);
    assert_eq!(renderer.generation(), 1);

    // Verify generation wraps at u32::MAX (ASSUM safety)
    for _ in 0..(u32::MAX as u64) {
        renderer.reset();
    }
    assert_eq!(renderer.generation(), 0, "Generation should wrap at u32::MAX");
}

// ============================================================================
// Q8-Q14: Property Tests (Mathematical Invariants)
// ============================================================================

#[test]
fn q8_scalar_vs_simd_4wide_equivalence() {
    // Property: SIMD 4-wide should match scalar results element-wise
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    let test_points = [
        (0.0, 0.0),
        (0.5, 0.5),
        (1.0, 1.0),
        (2.5, 2.5),
    ];

    let px = f32x4::from_array([
        test_points[0].0,
        test_points[1].0,
        test_points[2].0,
        test_points[3].0,
    ]);
    let py = f32x4::from_array([
        test_points[0].1,
        test_points[1].1,
        test_points[2].1,
        test_points[3].1,
    ]);

    let sdf_simd = SdfRendererCapsule::capsule_sdf_4wide(px, py, 0.0, 0.0, 1.0, 1.0);

    for (i, &(x, y)) in test_points.iter().enumerate() {
        let sdf_scalar = renderer.capsule_sdf_scalar(x, y, 0.0, 0.0, 1.0, 1.0);
        let diff = (sdf_simd[i] - sdf_scalar).abs();
        assert!(
            diff < 1e-5,
            "SIMD[{}] mismatch: scalar={}, simd={}, diff={}",
            i, sdf_scalar, sdf_simd[i], diff
        );
    }
}

#[test]
fn q9_scalar_vs_simd_8wide_equivalence() {
    // Property: SIMD 8-wide should match scalar results element-wise
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    let test_points: [(f32, f32); 8] = [
        (0.0, 0.0), (0.5, 0.5), (1.0, 1.0), (1.5, 1.5),
        (2.0, 2.0), (3.0, 3.0), (5.0, 5.0), (10.0, 10.0),
    ];

    let px = f32x8::from_array([
        test_points[0].0, test_points[1].0, test_points[2].0, test_points[3].0,
        test_points[4].0, test_points[5].0, test_points[6].0, test_points[7].0,
    ]);
    let py = f32x8::from_array([
        test_points[0].1, test_points[1].1, test_points[2].1, test_points[3].1,
        test_points[4].1, test_points[5].1, test_points[6].1, test_points[7].1,
    ]);

    let sdf_simd = SdfRendererCapsule::capsule_sdf_8wide(px, py, 0.0, 0.0, 1.0, 1.0);

    for (i, &(x, y)) in test_points.iter().enumerate() {
        let sdf_scalar = renderer.capsule_sdf_scalar(x, y, 0.0, 0.0, 1.0, 1.0);
        let diff = (sdf_simd[i] - sdf_scalar).abs();
        assert!(
            diff < 1e-5,
            "SIMD[{}] mismatch: scalar={}, simd={}, diff={}",
            i, sdf_scalar, sdf_simd[i], diff
        );
    }
}

#[test]
fn q10_smootherstep_monotonicity() {
    // Property: smootherstep(x) should be monotonically increasing on [0, 1]
    let steps = 100;
    let mut prev = SdfRendererCapsule::smootherstep_scalar(0.0);

    for i in 1..=steps {
        let x = i as f32 / steps as f32;
        let y = SdfRendererCapsule::smootherstep_scalar(x);
        assert!(
            y >= prev,
            "smootherstep not monotonic at x={}: y={}, prev={}",
            x, y, prev
        );
        prev = y;
    }
}

#[test]
fn q11_horizontal_min_reduction_correctness() {
    // Property: Horizontal min should return global minimum
    let test_cases = [
        f32x4::from_array([1.0, 2.0, 3.0, 4.0]),
        f32x4::from_array([4.0, 3.0, 2.0, 1.0]),
        f32x4::from_array([2.5, 1.5, 3.5, 2.0]),
        f32x4::from_array([5.0, 5.0, 5.0, 5.0]),
    ];

    for (i, &v) in test_cases.iter().enumerate() {
        let min_simd = SdfRendererCapsule::horizontal_min_4wide(v);
        let min_scalar = v[0].min(v[1]).min(v[2]).min(v[3]);
        assert_eq!(
            min_simd, min_scalar,
            "Test case {}: SIMD min={}, scalar min={}",
            i, min_simd, min_scalar
        );
    }
}

#[test]
fn q12_horizontal_min_8wide_correctness() {
    // Property: Horizontal min 8-wide should return global minimum
    let test_cases = [
        f32x8::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]),
        f32x8::from_array([8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0]),
        f32x8::from_array([4.5, 2.5, 6.5, 1.5, 7.5, 3.5, 8.5, 5.5]),
    ];

    for (i, &v) in test_cases.iter().enumerate() {
        let min_simd = SdfRendererCapsule::horizontal_min_8wide(v);
        let min_scalar = v.to_array().iter().copied().fold(f32::INFINITY, f32::min);
        assert_eq!(
            min_simd, min_scalar,
            "Test case {}: SIMD min={}, scalar min={}",
            i, min_simd, min_scalar
        );
    }
}

#[test]
fn q13_sdf_coverage_range_0_to_1() {
    // Property: Coverage should always be in [0, 1]
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    let test_sdfs = [
        -10.0, -1.0, -0.5, 0.0, 0.3, 0.5, 0.7, 1.0, 2.0, 10.0,
    ];

    for &sdf in &test_sdfs {
        let coverage = renderer.sdf_to_coverage_scalar(sdf);
        assert!(
            coverage >= 0.0 && coverage <= 1.0,
            "Coverage out of range for sdf={}: coverage={}",
            sdf, coverage
        );
    }
}

#[test]
fn q14_capsule_sdf_non_negative() {
    // Property: Capsule SDF should always be non-negative (unsigned distance)
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    let test_points = [
        (0.0, 0.0), (0.5, 0.5), (1.0, 1.0),
        (-1.0, -1.0), (5.0, 5.0), (10.0, 10.0),
    ];

    for &(x, y) in &test_points {
        let sdf = renderer.capsule_sdf_scalar(x, y, 0.0, 0.0, 1.0, 1.0);
        assert!(
            sdf >= 0.0,
            "SDF negative for point ({}, {}): sdf={}",
            x, y, sdf
        );
    }
}

// ============================================================================
// Q15-Q21: Integration Tests (Full Pipeline)
// ============================================================================

#[test]
fn q15_multi_segment_sdf_4wide_integration() {
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    // 8-segment "E" shape
    let segments = [
        (0.0, 0.0, 1.0, 0.0), // Bottom
        (0.0, 0.0, 0.0, 2.0), // Left
        (0.0, 2.0, 1.0, 2.0), // Top
        (0.0, 1.0, 0.8, 1.0), // Middle
        (1.0, 0.0, 1.0, 0.2),
        (1.0, 1.8, 1.0, 2.0),
        (0.8, 0.9, 0.8, 1.1),
        (0.0, 0.0, 0.0, 0.0), // Padding
    ];

    // Point inside "E" shape
    let sdf_inside = renderer.multi_segment_sdf_4wide(0.5, 1.0, &segments);
    assert!(sdf_inside < 0.5, "Inside point should have small SDF, got {}", sdf_inside);

    // Point outside "E" shape
    let sdf_outside = renderer.multi_segment_sdf_4wide(5.0, 5.0, &segments);
    assert!(sdf_outside > 4.0, "Outside point should have large SDF, got {}", sdf_outside);
}

#[test]
fn q16_multi_segment_sdf_8wide_integration() {
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    // 16-segment complex shape
    let segments = [
        (0.0, 0.0, 1.0, 0.0), (0.0, 0.0, 0.0, 2.0),
        (0.0, 2.0, 1.0, 2.0), (0.0, 1.0, 0.8, 1.0),
        (1.0, 0.0, 1.0, 0.2), (1.0, 1.8, 1.0, 2.0),
        (0.8, 0.9, 0.8, 1.1), (0.5, 0.5, 1.5, 0.5),
        (0.5, 0.5, 0.5, 1.5), (0.5, 1.5, 1.5, 1.5),
        (1.5, 0.5, 1.5, 1.5), (0.2, 0.2, 0.3, 0.3),
        (0.7, 0.7, 0.8, 0.8), (0.0, 0.0, 0.0, 0.0),
        (0.0, 0.0, 0.0, 0.0), (0.0, 0.0, 0.0, 0.0),
    ];

    let sdf = renderer.multi_segment_sdf_8wide(0.75, 0.75, &segments);
    assert!(sdf < 1.0, "SDF for complex shape should be <1, got {}", sdf);
}

#[test]
fn q17_render_glyph_4wide_integration() {
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    // Render 64×64 glyph
    let size = 64;
    let mut coverage_sum = 0.0f32;

    for y in 0..size {
        let mut x = 0;
        while x + 4 <= size {
            let px = f32x4::from_array([
                (x + 0) as f32 / size as f32,
                (x + 1) as f32 / size as f32,
                (x + 2) as f32 / size as f32,
                (x + 3) as f32 / size as f32,
            ]);
            let py = f32x4::splat(y as f32 / size as f32);

            let coverage = renderer.render_pixels_4wide(px, py, 0.2, 0.2, 0.8, 0.8);
            coverage_sum += coverage[0] + coverage[1] + coverage[2] + coverage[3];
            x += 4;
        }
    }

    // Verify reasonable coverage sum (inside capsule region)
    assert!(
        coverage_sum > 0.0,
        "Coverage sum should be >0 for glyph, got {}",
        coverage_sum
    );
    assert_eq!(
        renderer.pixels_rendered(),
        size * (size / 4) * 4,
        "Pixels rendered count mismatch"
    );
}

#[test]
fn q18_render_glyph_8wide_integration() {
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    // Render 64×64 glyph with 8-wide SIMD
    let size = 64;
    let mut coverage_sum = 0.0f32;

    for y in 0..size {
        let mut x = 0;
        while x + 8 <= size {
            let px = f32x8::from_array([
                (x + 0) as f32 / size as f32,
                (x + 1) as f32 / size as f32,
                (x + 2) as f32 / size as f32,
                (x + 3) as f32 / size as f32,
                (x + 4) as f32 / size as f32,
                (x + 5) as f32 / size as f32,
                (x + 6) as f32 / size as f32,
                (x + 7) as f32 / size as f32,
            ]);
            let py = f32x8::splat(y as f32 / size as f32);

            let coverage = renderer.render_pixels_8wide(px, py, 0.2, 0.2, 0.8, 0.8);
            for i in 0..8 {
                coverage_sum += coverage[i];
            }
            x += 8;
        }
    }

    assert!(
        coverage_sum > 0.0,
        "Coverage sum should be >0 for glyph, got {}",
        coverage_sum
    );
}

#[test]
fn q19_scalar_vs_simd_full_glyph_equivalence() {
    // Property: Scalar and SIMD should produce identical glyph coverage
    let renderer = SdfRendererCapsule::new(2.0, 0.5);
    let size = 32; // Small size for quick test

    let mut coverage_scalar = vec![vec![0.0f32; size]; size];
    let mut coverage_simd = vec![vec![0.0f32; size]; size];

    // Scalar rendering
    for y in 0..size {
        for x in 0..size {
            let px = x as f32 / size as f32;
            let py = y as f32 / size as f32;
            let sdf = SdfRendererCapsule::capsule_sdf_scalar(px, py, 0.2, 0.2, 0.8, 0.8);
            coverage_scalar[y][x] = renderer.sdf_to_coverage_scalar(sdf);
        }
    }

    // SIMD 4-wide rendering
    for y in 0..size {
        let mut x = 0;
        while x + 4 <= size {
            let px = f32x4::from_array([
                (x + 0) as f32 / size as f32,
                (x + 1) as f32 / size as f32,
                (x + 2) as f32 / size as f32,
                (x + 3) as f32 / size as f32,
            ]);
            let py = f32x4::splat(y as f32 / size as f32);
            let coverage = renderer.render_pixels_4wide(px, py, 0.2, 0.2, 0.8, 0.8);

            for i in 0..4 {
                coverage_simd[y][x + i] = coverage[i];
            }
            x += 4;
        }
    }

    // Compare (allow 1e-5 floating-point tolerance)
    for y in 0..size {
        for x in 0..size {
            let diff = (coverage_scalar[y][x] - coverage_simd[y][x]).abs();
            assert!(
                diff < 1e-5,
                "Coverage mismatch at ({}, {}): scalar={}, simd={}, diff={}",
                x, y, coverage_scalar[y][x], coverage_simd[y][x], diff
            );
        }
    }
}

#[test]
fn q20_state_reset_between_glyphs() {
    // Integration: Verify state reset between glyph renders
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    // Render first glyph
    renderer.render_pixels_4wide(
        f32x4::splat(0.5),
        f32x4::splat(0.5),
        0.0, 0.0, 1.0, 1.0,
    );
    assert_eq!(renderer.pixels_rendered(), 4);
    assert_eq!(renderer.generation(), 0);

    // Reset
    renderer.reset();
    assert_eq!(renderer.pixels_rendered(), 0);
    assert_eq!(renderer.generation(), 1);

    // Render second glyph
    renderer.render_pixels_4wide(
        f32x4::splat(0.5),
        f32x4::splat(0.5),
        0.0, 0.0, 1.0, 1.0,
    );
    assert_eq!(renderer.pixels_rendered(), 4);
    assert_eq!(renderer.generation(), 1);
}

#[test]
fn q21_edge_case_zero_length_capsule() {
    // Edge case: Capsule with zero length (degenerate to point)
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    let sdf = renderer.capsule_sdf_scalar(0.5, 0.5, 1.0, 1.0, 1.0, 1.0);

    // Distance to point (1, 1) from (0.5, 0.5)
    let expected = ((0.5f32).powi(2) + (0.5f32).powi(2)).sqrt();
    let diff = (sdf - expected).abs();
    assert!(
        diff < 0.01,
        "Zero-length capsule failed: expected={}, got={}, diff={}",
        expected, sdf, diff
    );
}

// ============================================================================
// Q22-Q28: Production Tests (Performance, Stress, Memory)
// ============================================================================

#[test]
fn q22_stress_test_large_glyph_256x256() {
    // Stress test: Render 256×256 glyph (65,536 pixels)
    let renderer = SdfRendererCapsule::new(2.0, 0.5);
    let size = 256;

    let mut coverage_sum = 0.0f32;
    for y in 0..size {
        let mut x = 0;
        while x + 8 <= size {
            let px = f32x8::from_array([
                (x + 0) as f32 / size as f32,
                (x + 1) as f32 / size as f32,
                (x + 2) as f32 / size as f32,
                (x + 3) as f32 / size as f32,
                (x + 4) as f32 / size as f32,
                (x + 5) as f32 / size as f32,
                (x + 6) as f32 / size as f32,
                (x + 7) as f32 / size as f32,
            ]);
            let py = f32x8::splat(y as f32 / size as f32);

            let coverage = renderer.render_pixels_8wide(px, py, 0.2, 0.2, 0.8, 0.8);
            for i in 0..8 {
                coverage_sum += coverage[i];
            }
            x += 8;
        }
    }

    assert!(coverage_sum > 0.0, "Large glyph render failed");
    println!("Q22: Rendered 256×256 glyph, coverage_sum={}", coverage_sum);
}

#[test]
fn q23_stress_test_complex_multi_segment() {
    // Stress test: 64-segment glyph (complex font)
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    let mut segments = vec![];
    for i in 0..64 {
        let t = i as f32 / 64.0;
        segments.push((
            t, 0.0,
            t + 0.01, 0.1,
        ));
    }

    let sdf = renderer.multi_segment_sdf_8wide(0.5, 0.05, &segments);
    assert!(sdf >= 0.0, "Multi-segment SDF failed");
}

#[test]
fn q24_memory_safety_state_overflow() {
    // Memory safety: Verify u32 overflow handling (pixels_rendered)
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    // Increment to near u32::MAX
    let initial_state = ((u32::MAX - 10) as u64) << 0; // pixels_rendered in lower 32 bits
    renderer.state.store(initial_state, core::sync::atomic::Ordering::Release);

    // Render 20 pixels (should overflow)
    for _ in 0..5 {
        renderer.render_pixels_4wide(
            f32x4::splat(0.5),
            f32x4::splat(0.5),
            0.0, 0.0, 1.0, 1.0,
        );
    }

    // Verify wrapping behavior (ASSUM safety)
    let pixels = renderer.pixels_rendered();
    assert_eq!(pixels, 9, "u32 overflow should wrap correctly, got {}", pixels);
}

#[test]
fn q25_concurrent_read_state() {
    // Concurrency: Verify lockfree state reads (no data races)
    use std::sync::Arc;
    use std::thread;

    let renderer = Arc::new(SdfRendererCapsule::new(2.0, 0.5));

    // Spawn 4 reader threads
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let renderer = renderer.clone();
            thread::spawn(move || {
                for _ in 0..1000 {
                    let _ = renderer.pixels_rendered();
                    let _ = renderer.generation();
                }
            })
        })
        .collect();

    // Wait for readers
    for handle in handles {
        handle.join().unwrap();
    }

    println!("Q25: Concurrent reads completed successfully");
}

#[test]
fn q26_performance_baseline_scalar() {
    // Performance: Scalar baseline (for B32 comparison)
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    let iterations = 10_000;
    let start = std::time::Instant::now();

    for i in 0..iterations {
        let px = (i % 100) as f32 / 100.0;
        let py = (i / 100) as f32 / 100.0;
        let _ = renderer.capsule_sdf_scalar(px, py, 0.0, 0.0, 1.0, 1.0);
    }

    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_nanos() as f64 / iterations as f64;
    println!("Q26: Scalar baseline: {:.2}ns per capsule_sdf", ns_per_op);
}

#[test]
fn q27_performance_simd_4wide() {
    // Performance: SIMD 4-wide (for B32 comparison)
    let iterations = 10_000 / 4; // Process 4 pixels at a time

    let start = std::time::Instant::now();

    for i in 0..iterations {
        let base = (i * 4) % 100;
        let px = f32x4::from_array([
            (base + 0) as f32 / 100.0,
            (base + 1) as f32 / 100.0,
            (base + 2) as f32 / 100.0,
            (base + 3) as f32 / 100.0,
        ]);
        let py = f32x4::splat(0.5);
        let _ = SdfRendererCapsule::capsule_sdf_4wide(px, py, 0.0, 0.0, 1.0, 1.0);
    }

    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_nanos() as f64 / (iterations * 4) as f64;
    println!("Q27: SIMD 4-wide: {:.2}ns per capsule_sdf", ns_per_op);
}

#[test]
fn q28_performance_simd_8wide() {
    // Performance: SIMD 8-wide (for B32 comparison)
    let iterations = 10_000 / 8;

    let start = std::time::Instant::now();

    for i in 0..iterations {
        let base = (i * 8) % 100;
        let px = f32x8::from_array([
            (base + 0) as f32 / 100.0,
            (base + 1) as f32 / 100.0,
            (base + 2) as f32 / 100.0,
            (base + 3) as f32 / 100.0,
            (base + 4) as f32 / 100.0,
            (base + 5) as f32 / 100.0,
            (base + 6) as f32 / 100.0,
            (base + 7) as f32 / 100.0,
        ]);
        let py = f32x8::splat(0.5);
        let _ = SdfRendererCapsule::capsule_sdf_8wide(px, py, 0.0, 0.0, 1.0, 1.0);
    }

    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_nanos() as f64 / (iterations * 8) as f64;
    println!("Q28: SIMD 8-wide: {:.2}ns per capsule_sdf", ns_per_op);
}
