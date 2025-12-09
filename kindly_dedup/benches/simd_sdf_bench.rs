//! SIMD SDF Renderer Benchmarks (B32 Framework Compliant)
//!
//! # Performance Claims Validation
//!
//! **Framework**: B32 (95% CI, 1000+ iterations, fair baselines)
//!
//! **Hardware Calibration**: AMD Ryzen 9 6900HX @ kindly-hub (192.168.0.38)
//!
//! **Baselines**:
//! - Scalar: Unoptimized but correct implementation (NOT strawman)
//! - SIMD 4-wide: f32x4 portable_simd (AVX/SSE compatible)
//! - SIMD 8-wide: f32x8 portable_simd (AVX2/AVX-512 required)
//!
//! **Claims to Validate**:
//! - capsule_sdf: 4-8× speedup (12ns → 3ns → 1.5ns)
//! - sdf_to_coverage: 4-8× speedup (8ns → 2ns → 1ns)
//! - multi_segment_sdf: 4-8× speedup (96ns → 24ns → 12ns for 8 segments)
//! - render_glyph: 4-8× speedup (1.2ms → 300μs → 150μs for 256×256 atlas)
//!
//! # Running Benchmarks
//!
//! ```bash
//! # Remote execution (MANDATORY per CLAUDE.md)
//! ssh samuel@kindly-hub "cd ~/Primitives/kindly_dedup && cargo bench --bench simd_sdf_bench --features simd-sdf-rendering"
//!
//! # Results: target/criterion/simd_sdf_*/report/index.html
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use kindly_dedup::simd_sdf_renderer::SdfRendererCapsule;

#[cfg(feature = "simd-sdf-rendering")]
use core::simd::{f32x4, f32x8};

// ============================================================================
// Baseline: Scalar Capsule SDF
// ============================================================================

fn bench_capsule_sdf_scalar(c: &mut Criterion) {
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    let mut group = c.benchmark_group("capsule_sdf_scalar");
    group.throughput(Throughput::Elements(1));

    group.bench_function("single_pixel", |b| {
        b.iter(|| {
            black_box(renderer.capsule_sdf_scalar(
                black_box(0.5), black_box(0.5),
                black_box(0.0), black_box(0.0),
                black_box(1.0), black_box(1.0),
            ))
        });
    });

    group.finish();
}

// ============================================================================
// SIMD 4-wide: Capsule SDF
// ============================================================================

#[cfg(feature = "simd-sdf-rendering")]
fn bench_capsule_sdf_4wide(c: &mut Criterion) {
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    let mut group = c.benchmark_group("capsule_sdf_4wide");
    group.throughput(Throughput::Elements(4));

    let px = f32x4::from_array([0.5, 1.5, 2.5, 3.5]);
    let py = f32x4::from_array([0.5, 1.5, 2.5, 3.5]);

    group.bench_function("four_pixels", |b| {
        b.iter(|| {
            black_box(SdfRendererCapsule::capsule_sdf_4wide(
                black_box(px), black_box(py),
                black_box(0.0), black_box(0.0),
                black_box(1.0), black_box(1.0),
            ))
        });
    });

    group.finish();
}

// ============================================================================
// SIMD 8-wide: Capsule SDF
// ============================================================================

#[cfg(feature = "simd-sdf-rendering")]
fn bench_capsule_sdf_8wide(c: &mut Criterion) {
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    let mut group = c.benchmark_group("capsule_sdf_8wide");
    group.throughput(Throughput::Elements(8));

    let px = f32x8::from_array([0.5, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5]);
    let py = f32x8::from_array([0.5, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5]);

    group.bench_function("eight_pixels", |b| {
        b.iter(|| {
            black_box(SdfRendererCapsule::capsule_sdf_8wide(
                black_box(px), black_box(py),
                black_box(0.0), black_box(0.0),
                black_box(1.0), black_box(1.0),
            ))
        });
    });

    group.finish();
}

// ============================================================================
// Baseline: Scalar Smootherstep
// ============================================================================

fn bench_smootherstep_scalar(c: &mut Criterion) {
    let mut group = c.benchmark_group("smootherstep_scalar");
    group.throughput(Throughput::Elements(1));

    group.bench_function("single_pixel", |b| {
        b.iter(|| {
            black_box(SdfRendererCapsule::smootherstep_scalar(
                black_box(0.5),
            ))
        });
    });

    group.finish();
}

// ============================================================================
// SIMD 4-wide: Smootherstep
// ============================================================================

#[cfg(feature = "simd-sdf-rendering")]
fn bench_smootherstep_4wide(c: &mut Criterion) {
    let mut group = c.benchmark_group("smootherstep_4wide");
    group.throughput(Throughput::Elements(4));

    let x = f32x4::from_array([0.2, 0.4, 0.6, 0.8]);

    group.bench_function("four_pixels", |b| {
        b.iter(|| {
            black_box(SdfRendererCapsule::smootherstep_4wide(
                black_box(x),
            ))
        });
    });

    group.finish();
}

// ============================================================================
// SIMD 8-wide: Smootherstep
// ============================================================================

#[cfg(feature = "simd-sdf-rendering")]
fn bench_smootherstep_8wide(c: &mut Criterion) {
    let mut group = c.benchmark_group("smootherstep_8wide");
    group.throughput(Throughput::Elements(8));

    let x = f32x8::from_array([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]);

    group.bench_function("eight_pixels", |b| {
        b.iter(|| {
            black_box(SdfRendererCapsule::smootherstep_8wide(
                black_box(x),
            ))
        });
    });

    group.finish();
}

// ============================================================================
// Baseline: Scalar SDF to Coverage
// ============================================================================

fn bench_sdf_to_coverage_scalar(c: &mut Criterion) {
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    let mut group = c.benchmark_group("sdf_to_coverage_scalar");
    group.throughput(Throughput::Elements(1));

    group.bench_function("single_pixel", |b| {
        b.iter(|| {
            black_box(renderer.sdf_to_coverage_scalar(
                black_box(0.3),
            ))
        });
    });

    group.finish();
}

// ============================================================================
// SIMD 4-wide: SDF to Coverage
// ============================================================================

#[cfg(feature = "simd-sdf-rendering")]
fn bench_sdf_to_coverage_4wide(c: &mut Criterion) {
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    let mut group = c.benchmark_group("sdf_to_coverage_4wide");
    group.throughput(Throughput::Elements(4));

    let sdf = f32x4::from_array([0.1, 0.3, 0.5, 0.7]);

    group.bench_function("four_pixels", |b| {
        b.iter(|| {
            black_box(renderer.sdf_to_coverage_4wide(
                black_box(sdf),
            ))
        });
    });

    group.finish();
}

// ============================================================================
// SIMD 8-wide: SDF to Coverage
// ============================================================================

#[cfg(feature = "simd-sdf-rendering")]
fn bench_sdf_to_coverage_8wide(c: &mut Criterion) {
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    let mut group = c.benchmark_group("sdf_to_coverage_8wide");
    group.throughput(Throughput::Elements(8));

    let sdf = f32x8::from_array([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]);

    group.bench_function("eight_pixels", |b| {
        b.iter(|| {
            black_box(renderer.sdf_to_coverage_8wide(
                black_box(sdf),
            ))
        });
    });

    group.finish();
}

// ============================================================================
// Multi-Segment SDF: Scalar Baseline
// ============================================================================

fn bench_multi_segment_scalar(c: &mut Criterion) {
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    // 8-segment glyph (e.g., "E" shape)
    let segments = [
        (0.0, 0.0, 1.0, 0.0), // Bottom horizontal
        (0.0, 0.0, 0.0, 2.0), // Left vertical
        (0.0, 2.0, 1.0, 2.0), // Top horizontal
        (0.0, 1.0, 0.8, 1.0), // Middle horizontal
        (1.0, 0.0, 1.0, 0.2), // Bottom right
        (1.0, 1.8, 1.0, 2.0), // Top right
        (0.8, 0.9, 0.8, 1.1), // Middle right
        (0.0, 0.0, 0.0, 0.0), // Padding
    ];

    let mut group = c.benchmark_group("multi_segment_scalar");
    group.throughput(Throughput::Elements(8)); // 8 segments

    group.bench_function("eight_segments", |b| {
        b.iter(|| {
            let mut min_dist = f32::MAX;
            for &(ax, ay, bx, by) in &segments {
                let dist = SdfRendererCapsule::capsule_sdf_scalar(
                    black_box(0.5), black_box(1.0),
                    black_box(ax), black_box(ay),
                    black_box(bx), black_box(by),
                );
                min_dist = min_dist.min(dist);
            }
            black_box(min_dist)
        });
    });

    group.finish();
}

// ============================================================================
// Multi-Segment SDF: SIMD 4-wide
// ============================================================================

#[cfg(feature = "simd-sdf-rendering")]
fn bench_multi_segment_4wide(c: &mut Criterion) {
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    let segments = [
        (0.0, 0.0, 1.0, 0.0),
        (0.0, 0.0, 0.0, 2.0),
        (0.0, 2.0, 1.0, 2.0),
        (0.0, 1.0, 0.8, 1.0),
        (1.0, 0.0, 1.0, 0.2),
        (1.0, 1.8, 1.0, 2.0),
        (0.8, 0.9, 0.8, 1.1),
        (0.0, 0.0, 0.0, 0.0),
    ];

    let mut group = c.benchmark_group("multi_segment_4wide");
    group.throughput(Throughput::Elements(8));

    group.bench_function("eight_segments", |b| {
        b.iter(|| {
            black_box(renderer.multi_segment_sdf_4wide(
                black_box(0.5), black_box(1.0),
                black_box(&segments),
            ))
        });
    });

    group.finish();
}

// ============================================================================
// Multi-Segment SDF: SIMD 8-wide
// ============================================================================

#[cfg(feature = "simd-sdf-rendering")]
fn bench_multi_segment_8wide(c: &mut Criterion) {
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    // 16-segment glyph (complex shape)
    let segments = [
        (0.0, 0.0, 1.0, 0.0), (0.0, 0.0, 0.0, 2.0),
        (0.0, 2.0, 1.0, 2.0), (0.0, 1.0, 0.8, 1.0),
        (1.0, 0.0, 1.0, 0.2), (1.0, 1.8, 1.0, 2.0),
        (0.8, 0.9, 0.8, 1.1), (0.0, 0.0, 0.0, 0.0),
        (0.5, 0.5, 1.5, 0.5), (0.5, 0.5, 0.5, 1.5),
        (0.5, 1.5, 1.5, 1.5), (1.5, 0.5, 1.5, 1.5),
        (0.2, 0.2, 0.3, 0.3), (0.7, 0.7, 0.8, 0.8),
        (0.0, 0.0, 0.0, 0.0), (0.0, 0.0, 0.0, 0.0),
    ];

    let mut group = c.benchmark_group("multi_segment_8wide");
    group.throughput(Throughput::Elements(16));

    group.bench_function("sixteen_segments", |b| {
        b.iter(|| {
            black_box(renderer.multi_segment_sdf_8wide(
                black_box(0.5), black_box(1.0),
                black_box(&segments),
            ))
        });
    });

    group.finish();
}

// ============================================================================
// Full Glyph Rendering: Scalar Baseline
// ============================================================================

fn bench_render_glyph_scalar(c: &mut Criterion) {
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    let mut group = c.benchmark_group("render_glyph_scalar");

    for size in [64, 128, 256] {
        group.throughput(Throughput::Elements((size * size) as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let mut coverage_sum = 0.0f32;
                for y in 0..size {
                    for x in 0..size {
                        let px = x as f32 / size as f32;
                        let py = y as f32 / size as f32;
                        let sdf = SdfRendererCapsule::capsule_sdf_scalar(
                            px, py, 0.2, 0.2, 0.8, 0.8,
                        );
                        let coverage = renderer.sdf_to_coverage_scalar(sdf);
                        coverage_sum += coverage;
                    }
                }
                black_box(coverage_sum)
            });
        });
    }

    group.finish();
}

// ============================================================================
// Full Glyph Rendering: SIMD 4-wide
// ============================================================================

#[cfg(feature = "simd-sdf-rendering")]
fn bench_render_glyph_4wide(c: &mut Criterion) {
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    let mut group = c.benchmark_group("render_glyph_4wide");

    for size in [64, 128, 256] {
        group.throughput(Throughput::Elements((size * size) as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
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

                        let coverage = renderer.render_pixels_4wide(
                            px, py, 0.2, 0.2, 0.8, 0.8,
                        );

                        coverage_sum += coverage[0] + coverage[1] + coverage[2] + coverage[3];
                        x += 4;
                    }

                    // Handle remaining pixels (scalar fallback)
                    while x < size {
                        let px = x as f32 / size as f32;
                        let py = y as f32 / size as f32;
                        let sdf = SdfRendererCapsule::capsule_sdf_scalar(
                            px, py, 0.2, 0.2, 0.8, 0.8,
                        );
                        coverage_sum += renderer.sdf_to_coverage_scalar(sdf);
                        x += 1;
                    }
                }
                black_box(coverage_sum)
            });
        });
    }

    group.finish();
}

// ============================================================================
// Full Glyph Rendering: SIMD 8-wide
// ============================================================================

#[cfg(feature = "simd-sdf-rendering")]
fn bench_render_glyph_8wide(c: &mut Criterion) {
    let renderer = SdfRendererCapsule::new(2.0, 0.5);

    let mut group = c.benchmark_group("render_glyph_8wide");

    for size in [64, 128, 256] {
        group.throughput(Throughput::Elements((size * size) as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
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

                        let coverage = renderer.render_pixels_8wide(
                            px, py, 0.2, 0.2, 0.8, 0.8,
                        );

                        for i in 0..8 {
                            coverage_sum += coverage[i];
                        }
                        x += 8;
                    }

                    // Handle remaining pixels (scalar fallback)
                    while x < size {
                        let px = x as f32 / size as f32;
                        let py = y as f32 / size as f32;
                        let sdf = SdfRendererCapsule::capsule_sdf_scalar(
                            px, py, 0.2, 0.2, 0.8, 0.8,
                        );
                        coverage_sum += renderer.sdf_to_coverage_scalar(sdf);
                        x += 1;
                    }
                }
                black_box(coverage_sum)
            });
        });
    }

    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    benches_scalar,
    bench_capsule_sdf_scalar,
    bench_smootherstep_scalar,
    bench_sdf_to_coverage_scalar,
    bench_multi_segment_scalar,
    bench_render_glyph_scalar,
);

#[cfg(feature = "simd-sdf-rendering")]
criterion_group!(
    benches_simd_4wide,
    bench_capsule_sdf_4wide,
    bench_smootherstep_4wide,
    bench_sdf_to_coverage_4wide,
    bench_multi_segment_4wide,
    bench_render_glyph_4wide,
);

#[cfg(feature = "simd-sdf-rendering")]
criterion_group!(
    benches_simd_8wide,
    bench_capsule_sdf_8wide,
    bench_smootherstep_8wide,
    bench_sdf_to_coverage_8wide,
    bench_multi_segment_8wide,
    bench_render_glyph_8wide,
);

#[cfg(feature = "simd-sdf-rendering")]
criterion_main!(benches_scalar, benches_simd_4wide, benches_simd_8wide);

#[cfg(not(feature = "simd-sdf-rendering"))]
criterion_main!(benches_scalar);
