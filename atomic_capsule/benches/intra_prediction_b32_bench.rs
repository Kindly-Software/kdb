//! B32 Benchmark: IntraPredictionCapsule (SIMD-accelerated AV1 Intra Prediction)
//!
//! # Performance Targets (B32 Framework)
//! - 4×4:   <50ns  (SIMD-optimized)
//! - 8×8:   <150ns (SIMD-optimized)
//! - 16×16: <400ns (SIMD-optimized)
//! - 32×32: <1μs   (PRIMARY TARGET, SIMD-optimized)
//!
//! # Framework Compliance
//! - **B32**: Fair baselines (scalar implementation, same hardware/compiler)
//! - **UCE34**: Q10 T2 SIMD tier validation
//! - **Chaos**: 100% lockfree, 256B cache-aligned
//! - **T28**: Performance regression tests (Q22-Q28)
//!
//! # Methodology
//! - 1000+ iterations per benchmark
//! - 95% confidence interval via Criterion
//! - Hardware: AMD Ryzen 9 6900HX (kindly-hub)
//! - Compiler: rustc nightly with portable_simd
//!
//! # Expected Speedups (SIMD vs Scalar)
//! - DC prediction: 5-8× (horizontal reduction + splat)
//! - SmoothV: 8-10× (vertical replication)
//! - SmoothH: 6-8× (horizontal splat)
//! - Paeth: 2-3× (branchless selection)
//! - Directional: 2-4× (vectorized interpolation)

#![cfg(all(feature = "portable_simd", feature = "encoder-intra-prediction"))]

use atomic_capsule::encoder::intra_prediction::{IntraMode, IntraPredictionCapsule};
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

// ============================================================================
// DC Prediction Benchmarks
// ============================================================================

fn bench_dc_prediction_4x4(c: &mut Criterion) {
    let capsule = IntraPredictionCapsule::new();

    let top = [100, 110, 120, 130];
    let left = [90, 100, 110, 120];
    let top_left = 100;

    capsule.load_references(&top, &left, top_left);
    capsule.set_block_size(4, 4);
    capsule.set_mode(IntraMode::DC, 0);

    c.bench_function("dc_4x4", |b| {
        b.iter(|| {
            let output = capsule.predict_block_4x4();
            black_box(output);
        });
    });
}

fn bench_dc_prediction_8x8(c: &mut Criterion) {
    let capsule = IntraPredictionCapsule::new();

    let top = [50, 60, 70, 80, 90, 100, 110, 120];
    let left = [40, 50, 60, 70, 80, 90, 100, 110];
    let top_left = 50;

    capsule.load_references(&top, &left, top_left);
    capsule.set_block_size(8, 8);
    capsule.set_mode(IntraMode::DC, 0);

    c.bench_function("dc_8x8", |b| {
        b.iter(|| {
            let output = capsule.predict_block_8x8();
            black_box(output);
        });
    });
}

fn bench_dc_prediction_16x16(c: &mut Criterion) {
    let capsule = IntraPredictionCapsule::new();

    let top = [128u8; 16];
    let left = [64u8; 16];
    let top_left = 96;

    capsule.load_references(&top, &left, top_left);
    capsule.set_block_size(16, 16);
    capsule.set_mode(IntraMode::DC, 0);

    c.bench_function("dc_16x16", |b| {
        b.iter(|| {
            let output = capsule.predict_block_16x16();
            black_box(output);
        });
    });
}

fn bench_dc_prediction_32x32(c: &mut Criterion) {
    let capsule = IntraPredictionCapsule::new();

    let top = [200u8; 32];
    let left = [100u8; 32];
    let top_left = 150;

    capsule.load_references(&top, &left, top_left);
    capsule.set_block_size(32, 32);
    capsule.set_mode(IntraMode::DC, 0);

    c.bench_function("dc_32x32_PRIMARY_TARGET", |b| {
        b.iter(|| {
            let output = capsule.predict_block_32x32();
            black_box(output);
        });
    });
}

// ============================================================================
// Smooth-V Prediction Benchmarks (Vertical Replication)
// ============================================================================

fn bench_smooth_v_8x8(c: &mut Criterion) {
    let capsule = IntraPredictionCapsule::new();

    let top = [10, 20, 30, 40, 50, 60, 70, 80];
    let left = [0u8; 8];
    let top_left = 0;

    capsule.load_references(&top, &left, top_left);
    capsule.set_block_size(8, 8);
    capsule.set_mode(IntraMode::SmoothV, 0);

    c.bench_function("smooth_v_8x8", |b| {
        b.iter(|| {
            let output = capsule.predict_block_8x8();
            black_box(output);
        });
    });
}

fn bench_smooth_v_32x32(c: &mut Criterion) {
    let capsule = IntraPredictionCapsule::new();

    let top: Vec<u8> = (0..32).map(|i| (i * 8) as u8).collect();
    let left = [0u8; 32];
    let top_left = 0;

    capsule.load_references(&top, &left, top_left);
    capsule.set_block_size(32, 32);
    capsule.set_mode(IntraMode::SmoothV, 0);

    c.bench_function("smooth_v_32x32", |b| {
        b.iter(|| {
            let output = capsule.predict_block_32x32();
            black_box(output);
        });
    });
}

// ============================================================================
// Smooth-H Prediction Benchmarks (Horizontal Splat)
// ============================================================================

fn bench_smooth_h_8x8(c: &mut Criterion) {
    let capsule = IntraPredictionCapsule::new();

    let top = [0u8; 8];
    let left = [10, 20, 30, 40, 50, 60, 70, 80];
    let top_left = 0;

    capsule.load_references(&top, &left, top_left);
    capsule.set_block_size(8, 8);
    capsule.set_mode(IntraMode::SmoothH, 0);

    c.bench_function("smooth_h_8x8", |b| {
        b.iter(|| {
            let output = capsule.predict_block_8x8();
            black_box(output);
        });
    });
}

fn bench_smooth_h_32x32(c: &mut Criterion) {
    let capsule = IntraPredictionCapsule::new();

    let top = [0u8; 32];
    let left: Vec<u8> = (0..32).map(|i| (i * 4) as u8).collect();
    let top_left = 0;

    capsule.load_references(&top, &left, top_left);
    capsule.set_block_size(32, 32);
    capsule.set_mode(IntraMode::SmoothH, 0);

    c.bench_function("smooth_h_32x32", |b| {
        b.iter(|| {
            let output = capsule.predict_block_32x32();
            black_box(output);
        });
    });
}

// ============================================================================
// Paeth Prediction Benchmarks (Branchless Selection)
// ============================================================================

fn bench_paeth_4x4(c: &mut Criterion) {
    let capsule = IntraPredictionCapsule::new();

    let top = [100, 110, 120, 130];
    let left = [90, 100, 110, 120];
    let top_left = 100;

    capsule.load_references(&top, &left, top_left);
    capsule.set_block_size(4, 4);
    capsule.set_mode(IntraMode::Paeth, 0);

    c.bench_function("paeth_4x4", |b| {
        b.iter(|| {
            let output = capsule.predict_block_4x4();
            black_box(output);
        });
    });
}

fn bench_paeth_8x8(c: &mut Criterion) {
    let capsule = IntraPredictionCapsule::new();

    let top = [100, 110, 120, 130, 140, 150, 160, 170];
    let left = [90, 100, 110, 120, 130, 140, 150, 160];
    let top_left = 100;

    capsule.load_references(&top, &left, top_left);
    capsule.set_block_size(8, 8);
    capsule.set_mode(IntraMode::Paeth, 0);

    c.bench_function("paeth_8x8", |b| {
        b.iter(|| {
            let output = capsule.predict_block_8x8();
            black_box(output);
        });
    });
}

fn bench_paeth_32x32(c: &mut Criterion) {
    let capsule = IntraPredictionCapsule::new();

    let top: Vec<u8> = (0..32).map(|i| (i * 8) as u8).collect();
    let left: Vec<u8> = (0..32).map(|i| (i * 4) as u8).collect();
    let top_left = 0;

    capsule.load_references(&top, &left, top_left);
    capsule.set_block_size(32, 32);
    capsule.set_mode(IntraMode::Paeth, 0);

    c.bench_function("paeth_32x32", |b| {
        b.iter(|| {
            let output = capsule.predict_block_32x32();
            black_box(output);
        });
    });
}

// ============================================================================
// Directional Prediction Benchmarks (Vectorized Interpolation)
// ============================================================================

fn bench_directional_vertical_8x8(c: &mut Criterion) {
    let capsule = IntraPredictionCapsule::new();

    let top = [50, 60, 70, 80, 90, 100, 110, 120];
    let left = [0u8; 8];
    let top_left = 50;

    capsule.load_references(&top, &left, top_left);
    capsule.set_block_size(8, 8);
    capsule.set_mode(IntraMode::Vertical, 0);

    c.bench_function("directional_vertical_8x8", |b| {
        b.iter(|| {
            let output = capsule.predict_block_8x8();
            black_box(output);
        });
    });
}

fn bench_directional_horizontal_8x8(c: &mut Criterion) {
    let capsule = IntraPredictionCapsule::new();

    let top = [0u8; 8];
    let left = [50, 60, 70, 80, 90, 100, 110, 120];
    let top_left = 50;

    capsule.load_references(&top, &left, top_left);
    capsule.set_block_size(8, 8);
    capsule.set_mode(IntraMode::Horizontal, 0);

    c.bench_function("directional_horizontal_8x8", |b| {
        b.iter(|| {
            let output = capsule.predict_block_8x8();
            black_box(output);
        });
    });
}

fn bench_directional_diagonal_8x8(c: &mut Criterion) {
    let capsule = IntraPredictionCapsule::new();

    let top = [100, 110, 120, 130, 140, 150, 160, 170];
    let left = [90, 100, 110, 120, 130, 140, 150, 160];
    let top_left = 100;

    capsule.load_references(&top, &left, top_left);
    capsule.set_block_size(8, 8);
    capsule.set_mode(IntraMode::D45, 0);

    c.bench_function("directional_diagonal_8x8", |b| {
        b.iter(|| {
            let output = capsule.predict_block_8x8();
            black_box(output);
        });
    });
}

fn bench_directional_diagonal_32x32(c: &mut Criterion) {
    let capsule = IntraPredictionCapsule::new();

    let top: Vec<u8> = (0..32).map(|i| (i * 8) as u8).collect();
    let left: Vec<u8> = (0..32).map(|i| (i * 4) as u8).collect();
    let top_left = 0;

    capsule.load_references(&top, &left, top_left);
    capsule.set_block_size(32, 32);
    capsule.set_mode(IntraMode::D45, 0);

    c.bench_function("directional_diagonal_32x32", |b| {
        b.iter(|| {
            let output = capsule.predict_block_32x32();
            black_box(output);
        });
    });
}

// ============================================================================
// Comparison Benchmark (All Modes at 8×8)
// ============================================================================

fn bench_all_modes_8x8(c: &mut Criterion) {
    let mut group = c.benchmark_group("all_modes_8x8");

    let capsule = IntraPredictionCapsule::new();

    let top = [100, 110, 120, 130, 140, 150, 160, 170];
    let left = [90, 100, 110, 120, 130, 140, 150, 160];
    let top_left = 100;

    capsule.load_references(&top, &left, top_left);
    capsule.set_block_size(8, 8);

    let modes = [
        ("DC", IntraMode::DC),
        ("Smooth", IntraMode::Smooth),
        ("SmoothV", IntraMode::SmoothV),
        ("SmoothH", IntraMode::SmoothH),
        ("Paeth", IntraMode::Paeth),
        ("Vertical", IntraMode::Vertical),
        ("Horizontal", IntraMode::Horizontal),
        ("D45", IntraMode::D45),
        ("D67", IntraMode::D67),
        ("D113", IntraMode::D113),
        ("D135", IntraMode::D135),
        ("D157", IntraMode::D157),
        ("D203", IntraMode::D203),
    ];

    for (name, mode) in modes.iter() {
        group.bench_with_input(BenchmarkId::new("mode", name), mode, |b, &mode| {
            capsule.set_mode(mode, 0);
            b.iter(|| {
                let output = capsule.predict_block_8x8();
                black_box(output);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Mode Setting Overhead Benchmark
// ============================================================================

fn bench_mode_setting_overhead(c: &mut Criterion) {
    let capsule = IntraPredictionCapsule::new();

    c.bench_function("mode_setting_overhead", |b| {
        b.iter(|| {
            capsule.set_mode(black_box(IntraMode::DC), black_box(0));
        });
    });
}

// ============================================================================
// Reference Loading Overhead Benchmark
// ============================================================================

fn bench_reference_loading_overhead(c: &mut Criterion) {
    let capsule = IntraPredictionCapsule::new();

    let top = [128u8; 32];
    let left = [64u8; 32];
    let top_left = 96;

    c.bench_function("reference_loading_overhead", |b| {
        b.iter(|| {
            capsule.load_references(black_box(&top), black_box(&left), black_box(top_left));
        });
    });
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    dc_benches,
    bench_dc_prediction_4x4,
    bench_dc_prediction_8x8,
    bench_dc_prediction_16x16,
    bench_dc_prediction_32x32,
);

criterion_group!(
    smooth_benches,
    bench_smooth_v_8x8,
    bench_smooth_v_32x32,
    bench_smooth_h_8x8,
    bench_smooth_h_32x32,
);

criterion_group!(
    paeth_benches,
    bench_paeth_4x4,
    bench_paeth_8x8,
    bench_paeth_32x32,
);

criterion_group!(
    directional_benches,
    bench_directional_vertical_8x8,
    bench_directional_horizontal_8x8,
    bench_directional_diagonal_8x8,
    bench_directional_diagonal_32x32,
);

criterion_group!(
    comparison_benches,
    bench_all_modes_8x8,
    bench_mode_setting_overhead,
    bench_reference_loading_overhead,
);

criterion_main!(
    dc_benches,
    smooth_benches,
    paeth_benches,
    directional_benches,
    comparison_benches,
);
