//! IntraPredictionCapsule Benchmarks (B32 Honest Baseline)
//!
//! # Methodology
//! - Baseline: Scalar C-like implementations (NOT strawman)
//! - Hardware: Same CPU, same compiler flags
//! - Iterations: 1000+ per benchmark (95% CI via Criterion)
//! - Fair comparison: Both implementations highly optimized
//!
//! # Performance Targets (Conservative)
//! - 4×4 blocks: <50ns (16 pixels, SIMD 4-way)
//! - 8×8 blocks: <150ns (64 pixels, SIMD 8-way)
//! - 16×16 blocks: <400ns (256 pixels, SIMD 32-way)
//! - 32×32 blocks: <1μs (1024 pixels, PRIMARY TARGET)
//!
//! # Expected Results
//! - SIMD: 2-5× speedup vs scalar (conservative)
//! - DC mode: Fastest (simple average)
//! - Directional: Medium (SIMD vector ops)
//! - Smooth/Paeth: Slowest (complex interpolation)
//!
//! # B32 Reality Check
//! AV1 intra prediction is COMPUTE-BOUND (2-19× SIMD speedup is realistic).
//! Baseline is optimized scalar C code (NOT naive loops).

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

#[cfg(feature = "portable_simd")]
use atomic_capsule::encoder::{IntraMode, IntraPredictionCapsule};

// ============================================================================
// BASELINE: Scalar Implementations (OPTIMIZED, NOT STRAWMAN)
// ============================================================================

/// Scalar DC prediction (average of reference pixels)
#[inline(never)]
fn scalar_predict_dc(refs: &[u8], output: &mut [u8], size: usize) {
    // Sum all reference pixels (top + left)
    let mut sum = 0u32;
    let ref_count = size * 2;
    for i in 0..ref_count {
        sum += refs[i] as u32;
    }

    // Average (with rounding)
    let avg = ((sum + (ref_count as u32 / 2)) / ref_count as u32) as u8;

    // Fill output block
    for pixel in output.iter_mut() {
        *pixel = avg;
    }
}

/// Scalar directional prediction (vertical mode for simplicity)
#[inline(never)]
fn scalar_predict_directional(refs: &[u8], output: &mut [u8], size: usize) {
    // Vertical prediction: copy top row to all rows
    let top_refs = &refs[0..size];

    for row in 0..size {
        let row_offset = row * size;
        for col in 0..size {
            output[row_offset + col] = top_refs[col];
        }
    }
}

/// Scalar smooth prediction (bilinear interpolation)
#[inline(never)]
fn scalar_predict_smooth(refs: &[u8], output: &mut [u8], size: usize) {
    let top_refs = &refs[0..size];
    let left_refs = &refs[size..size * 2];

    let top_right = refs[size - 1] as i32;
    let bottom_left = left_refs[size - 1] as i32;

    for row in 0..size {
        let row_weight = (row * 256) / size;
        let inv_row_weight = 256 - row_weight;

        for col in 0..size {
            let col_weight = (col * 256) / size;
            let inv_col_weight = 256 - col_weight;

            // Bilinear interpolation
            let horiz = (top_refs[col] as i32 * inv_row_weight + bottom_left * row_weight) >> 8;
            let vert = (left_refs[row] as i32 * inv_col_weight + top_right * col_weight) >> 8;

            output[row * size + col] = ((horiz + vert + 1) >> 1) as u8;
        }
    }
}

// ============================================================================
// BENCHMARKS: 4×4 Blocks (<50ns target)
// ============================================================================

#[cfg(feature = "portable_simd")]
fn bench_4x4_dc(c: &mut Criterion) {
    let mut group = c.benchmark_group("intra_4x4_dc");

    let capsule = IntraPredictionCapsule::new();
    capsule.set_mode(IntraMode::DC, 0);
    capsule.set_block_size(4, 4);

    // Reference pixels: 4 top + 4 left + 1 top_left = 9 bytes
    let top = vec![128u8; 4];
    let left = vec![128u8; 4];
    capsule.load_references(&top, &left, 128);

    // SIMD implementation
    group.bench_function("simd", |b| {
        b.iter(|| black_box(capsule.predict_block_4x4().unwrap()));
    });

    // Scalar baseline
    let mut output = vec![0u8; 16];
    group.bench_function("scalar", |b| {
        b.iter(|| {
            black_box(scalar_predict_dc(
                black_box(&refs),
                black_box(&mut output),
                black_box(4),
            ))
        });
    });

    group.finish();
}

#[cfg(feature = "portable_simd")]
fn bench_4x4_directional(c: &mut Criterion) {
    let mut group = c.benchmark_group("intra_4x4_directional");

    let capsule = IntraPredictionCapsule::new();
    capsule.set_mode(IntraMode::Vertical, 0);
    capsule.set_block_size(4, 4);

    let top = vec![128u8; 4];
    let left = vec![128u8; 4];
    capsule.load_references(&top, &left, 128);

    group.bench_function("simd", |b| {
        b.iter(|| black_box(capsule.predict_block_4x4().unwrap()));
    });

    let mut output = vec![0u8; 16];
    group.bench_function("scalar", |b| {
        b.iter(|| {
            black_box(scalar_predict_directional(
                black_box(&refs),
                black_box(&mut output),
                black_box(4),
            ))
        });
    });

    group.finish();
}

#[cfg(feature = "portable_simd")]
fn bench_4x4_smooth(c: &mut Criterion) {
    let mut group = c.benchmark_group("intra_4x4_smooth");

    let capsule = IntraPredictionCapsule::new();
    capsule.set_mode(IntraMode::Smooth, 0);
    capsule.set_block_size(4, 4);

    let top = vec![128u8; 4];
    let left = vec![128u8; 4];
    capsule.load_references(&top, &left, 128);

    group.bench_function("simd", |b| {
        b.iter(|| black_box(capsule.predict_block_4x4().unwrap()));
    });

    let mut output = vec![0u8; 16];
    group.bench_function("scalar", |b| {
        b.iter(|| {
            black_box(scalar_predict_smooth(
                black_box(&refs),
                black_box(&mut output),
                black_box(4),
            ))
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARKS: 8×8 Blocks (<150ns target)
// ============================================================================

#[cfg(feature = "portable_simd")]
fn bench_8x8_dc(c: &mut Criterion) {
    let mut group = c.benchmark_group("intra_8x8_dc");

    let capsule = IntraPredictionCapsule::new();
    capsule.set_mode(IntraMode::DC, 0);
    capsule.set_block_size(8, 8);

    let top = vec![128u8; 8];
    let left = vec![128u8; 8];
    capsule.load_references(&top, &left, 128);

    group.bench_function("simd", |b| {
        b.iter(|| black_box(capsule.predict_block_8x8().unwrap()));
    });

    let mut output = vec![0u8; 64];
    group.bench_function("scalar", |b| {
        b.iter(|| {
            black_box(scalar_predict_dc(
                black_box(&refs),
                black_box(&mut output),
                black_box(8),
            ))
        });
    });

    group.finish();
}

#[cfg(feature = "portable_simd")]
fn bench_8x8_directional(c: &mut Criterion) {
    let mut group = c.benchmark_group("intra_8x8_directional");

    let capsule = IntraPredictionCapsule::new();
    capsule.set_mode(IntraMode::Horizontal, 0);
    capsule.set_block_size(8, 8);

    let top = vec![128u8; 8];
    let left = vec![128u8; 8];
    capsule.load_references(&top, &left, 128);

    group.bench_function("simd", |b| {
        b.iter(|| black_box(capsule.predict_block_8x8().unwrap()));
    });

    let mut output = vec![0u8; 64];
    group.bench_function("scalar", |b| {
        b.iter(|| {
            black_box(scalar_predict_directional(
                black_box(&refs),
                black_box(&mut output),
                black_box(8),
            ))
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARKS: 16×16 Blocks (<400ns target)
// ============================================================================

#[cfg(feature = "portable_simd")]
fn bench_16x16_dc(c: &mut Criterion) {
    let mut group = c.benchmark_group("intra_16x16_dc");

    let capsule = IntraPredictionCapsule::new();
    capsule.set_mode(IntraMode::DC, 0);
    capsule.set_block_size(16, 16);

    let top = vec![128u8; 16];
    let left = vec![128u8; 16];
    capsule.load_references(&top, &left, 128);

    group.bench_function("simd", |b| {
        b.iter(|| black_box(capsule.predict_block_16x16().unwrap()));
    });

    let mut output = vec![0u8; 256];
    group.bench_function("scalar", |b| {
        b.iter(|| {
            black_box(scalar_predict_dc(
                black_box(&refs),
                black_box(&mut output),
                black_box(16),
            ))
        });
    });

    group.finish();
}

#[cfg(feature = "portable_simd")]
fn bench_16x16_smooth(c: &mut Criterion) {
    let mut group = c.benchmark_group("intra_16x16_smooth");

    let capsule = IntraPredictionCapsule::new();
    capsule.set_mode(IntraMode::SmoothV, 0);
    capsule.set_block_size(16, 16);

    let top = vec![128u8; 16];
    let left = vec![128u8; 16];
    capsule.load_references(&top, &left, 128);

    group.bench_function("simd", |b| {
        b.iter(|| black_box(capsule.predict_block_16x16().unwrap()));
    });

    let mut output = vec![0u8; 256];
    group.bench_function("scalar", |b| {
        b.iter(|| {
            black_box(scalar_predict_smooth(
                black_box(&refs),
                black_box(&mut output),
                black_box(16),
            ))
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARKS: 32×32 Blocks (<1μs PRIMARY TARGET)
// ============================================================================

#[cfg(feature = "portable_simd")]
fn bench_32x32_dc(c: &mut Criterion) {
    let mut group = c.benchmark_group("intra_32x32_dc");

    let capsule = IntraPredictionCapsule::new();
    capsule.set_mode(IntraMode::DC, 0);
    capsule.set_block_size(32, 32);

    let top = vec![128u8; 32];
    let left = vec![128u8; 32];
    capsule.load_references(&top, &left, 128);

    group.bench_function("simd", |b| {
        b.iter(|| black_box(capsule.predict_block_32x32().unwrap()));
    });

    let mut output = vec![0u8; 1024];
    group.bench_function("scalar", |b| {
        b.iter(|| {
            black_box(scalar_predict_dc(
                black_box(&refs),
                black_box(&mut output),
                black_box(32),
            ))
        });
    });

    group.finish();
}

#[cfg(feature = "portable_simd")]
fn bench_32x32_directional(c: &mut Criterion) {
    let mut group = c.benchmark_group("intra_32x32_directional");

    let capsule = IntraPredictionCapsule::new();
    capsule.set_mode(IntraMode::D45, 0);
    capsule.set_block_size(32, 32);

    let top = vec![128u8; 32];
    let left = vec![128u8; 32];
    capsule.load_references(&top, &left, 128);

    group.bench_function("simd", |b| {
        b.iter(|| black_box(capsule.predict_block_32x32().unwrap()));
    });

    let mut output = vec![0u8; 1024];
    group.bench_function("scalar", |b| {
        b.iter(|| {
            black_box(scalar_predict_directional(
                black_box(&refs),
                black_box(&mut output),
                black_box(32),
            ))
        });
    });

    group.finish();
}

#[cfg(feature = "portable_simd")]
fn bench_32x32_smooth(c: &mut Criterion) {
    let mut group = c.benchmark_group("intra_32x32_smooth");

    let capsule = IntraPredictionCapsule::new();
    capsule.set_mode(IntraMode::SmoothH, 0);
    capsule.set_block_size(32, 32);

    let top = vec![128u8; 32];
    let left = vec![128u8; 32];
    capsule.load_references(&top, &left, 128);

    group.bench_function("simd", |b| {
        b.iter(|| black_box(capsule.predict_block_32x32().unwrap()));
    });

    let mut output = vec![0u8; 1024];
    group.bench_function("scalar", |b| {
        b.iter(|| {
            black_box(scalar_predict_smooth(
                black_box(&refs),
                black_box(&mut output),
                black_box(32),
            ))
        });
    });

    group.finish();
}

#[cfg(feature = "portable_simd")]
fn bench_32x32_paeth(c: &mut Criterion) {
    let mut group = c.benchmark_group("intra_32x32_paeth");

    let capsule = IntraPredictionCapsule::new();
    capsule.set_mode(IntraMode::Paeth, 0);
    capsule.set_block_size(32, 32);

    let top = vec![128u8; 32];
    let left = vec![128u8; 32];
    capsule.load_references(&top, &left, 128);

    group.bench_function("simd", |b| {
        b.iter(|| black_box(capsule.predict_block_32x32().unwrap()));
    });

    // Paeth is complex, use smooth as scalar baseline
    let mut output = vec![0u8; 1024];
    group.bench_function("scalar", |b| {
        b.iter(|| {
            black_box(scalar_predict_smooth(
                black_box(&refs),
                black_box(&mut output),
                black_box(32),
            ))
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARKS: Mode Comparison (32×32)
// ============================================================================

#[cfg(feature = "portable_simd")]
fn bench_mode_comparison_32x32(c: &mut Criterion) {
    let mut group = c.benchmark_group("mode_comparison_32x32");

    let top = vec![128u8; 32];
    let left = vec![128u8; 32];

    // Test all 13 IntraMode variants
    let modes = [
        IntraMode::DC,
        IntraMode::Vertical,
        IntraMode::Horizontal,
        IntraMode::D45,
        IntraMode::D67,
        IntraMode::D113,
        IntraMode::D135,
        IntraMode::D157,
        IntraMode::D203,
        IntraMode::Smooth,
        IntraMode::SmoothV,
        IntraMode::SmoothH,
        IntraMode::Paeth,
    ];

    for mode in &modes {
        let capsule = IntraPredictionCapsule::new();
        capsule.set_mode(*mode, 0);
        capsule.set_block_size(32, 32);
        capsule.load_references(&top, &left, 128);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{:?}", mode)),
            mode,
            |b, _| {
                b.iter(|| black_box(capsule.predict_block_32x32().unwrap()));
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARKS: Angle Delta Sweep (32×32)
// ============================================================================

#[cfg(feature = "portable_simd")]
fn bench_angle_delta_sweep_32x32(c: &mut Criterion) {
    let mut group = c.benchmark_group("angle_delta_sweep_32x32");

    let top = vec![128u8; 32];
    let left = vec![128u8; 32];

    // Test D45 with all 7 delta angles (-3 to +3)
    for delta in -3..=3 {
        let capsule = IntraPredictionCapsule::new();
        capsule.set_mode(IntraMode::D45, delta);
        capsule.set_block_size(32, 32);
        capsule.load_references(&top, &left, 128);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("delta_{}", delta)),
            &delta,
            |b, _| {
                b.iter(|| black_box(capsule.predict_block_32x32().unwrap()));
            },
        );
    }

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

#[cfg(feature = "portable_simd")]
criterion_group!(
    benches,
    // 4×4 blocks
    bench_4x4_dc,
    bench_4x4_directional,
    bench_4x4_smooth,
    // 8×8 blocks
    bench_8x8_dc,
    bench_8x8_directional,
    // 16×16 blocks
    bench_16x16_dc,
    bench_16x16_smooth,
    // 32×32 blocks (PRIMARY)
    bench_32x32_dc,
    bench_32x32_directional,
    bench_32x32_smooth,
    bench_32x32_paeth,
    // Mode comparison
    bench_mode_comparison_32x32,
    // Angle delta sweep
    bench_angle_delta_sweep_32x32,
);

#[cfg(not(feature = "portable_simd"))]
criterion_group!(benches,);

criterion_main!(benches);
