//! Realistic SIMD Validation Benchmark - Find the Crossover Point
//!
//! **Purpose**: Validate SIMD capsule performance with realistic array sizes (100-1000 elements)
//! to determine the break-even point where SIMD becomes faster than scalar.
//!
//! **Phase 5 Finding**: SIMD is SLOWER for 8-element operations due to capsule overhead.
//! **Question**: At what array size does SIMD overtake scalar performance?
//!
//! **Framework Compliance**:
//! - UCE33 Q10: Tier 2 SIMD Capsule
//! - UCE33 Q28: Simplicity - find actual crossover point, not toy benchmarks
//! - UCE33 Q29: Constraints - capsule overhead dominates for small arrays
//! - UCE33 Q30: Validation - find ACTUAL crossover point where SIMD wins
//! - UCE33 Q33: Honest reporting - report both successes and failures (B32)
//! - B32 K15: SIMD 2-8× typical (but requires sufficient data)
//! - B32 B1: Fair baselines (optimized scalar vs SIMD)
//! - B32 B2: Statistical rigor (95% CI, 1000+ iterations)
//! - B32 B27: Honest reporting (document when SIMD loses)

#![feature(portable_simd)]

use atomic_capsule::primitives::SimdCapsule;
use atomic_capsule::SimdF32x8Capsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

// ============================================================================
// ARRAY SIZE CONFIGURATIONS
// ============================================================================

/// Array sizes to test (powers of 2 for clean SIMD chunking)
const ARRAY_SIZES: &[usize] = &[
    8,    // Phase 5 baseline (known SLOWER)
    16,   // 2× baseline
    32,   // 4× baseline
    64,   // 8× baseline (potential crossover?)
    128,  // 16× baseline
    256,  // 32× baseline (expected SIMD win)
    512,  // 64× baseline
    1024, // 128× baseline (realistic workload)
    2048, // 256× baseline (large workload)
    4096, // 512× baseline (very large)
];

// ============================================================================
// ARITHMETIC OPERATIONS - F32
// ============================================================================

/// B32 B1: Fair baseline - optimized scalar addition
fn scalar_add_f32(a: &[f32], b: &[f32]) -> Vec<f32> {
    a.iter().zip(b.iter()).map(|(x, y)| x + y).collect()
}

/// SIMD addition with F32x8 capsules (chunked processing)
fn simd_add_f32(a: &[f32], b: &[f32]) -> Vec<f32> {
    let mut result = Vec::with_capacity(a.len());

    // Process 8 elements at a time
    for chunk in a.chunks_exact(8).zip(b.chunks_exact(8)) {
        let (a_chunk, b_chunk) = chunk;
        let cap_a = SimdF32x8Capsule::from_array([
            a_chunk[0], a_chunk[1], a_chunk[2], a_chunk[3], a_chunk[4], a_chunk[5], a_chunk[6],
            a_chunk[7],
        ]);
        let cap_b = SimdF32x8Capsule::from_array([
            b_chunk[0], b_chunk[1], b_chunk[2], b_chunk[3], b_chunk[4], b_chunk[5], b_chunk[6],
            b_chunk[7],
        ]);

        let sum = cap_a.add(&cap_b);
        result.extend_from_slice(&sum.load());
    }

    // Handle remainder (if array size not multiple of 8)
    let remainder_start = (a.len() / 8) * 8;
    for i in remainder_start..a.len() {
        result.push(a[i] + b[i]);
    }

    result
}

/// Benchmark: Addition across array sizes
fn bench_add_realistic(c: &mut Criterion) {
    let mut group = c.benchmark_group("f32_add_realistic");

    // B32 B2: Statistical rigor
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    for &size in ARRAY_SIZES {
        let a: Vec<f32> = (0..size).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..size).map(|i| (size - i) as f32).collect();

        group.throughput(Throughput::Elements(size as u64));

        // Scalar baseline
        group.bench_with_input(BenchmarkId::new("scalar", size), &size, |bencher, _| {
            bencher.iter(|| black_box(scalar_add_f32(&a, &b)));
        });

        // SIMD capsule
        group.bench_with_input(BenchmarkId::new("simd", size), &size, |bencher, _| {
            bencher.iter(|| black_box(simd_add_f32(&a, &b)));
        });
    }

    group.finish();
}

// ============================================================================
// MULTIPLICATION - F32
// ============================================================================

fn scalar_mul_f32(a: &[f32], b: &[f32]) -> Vec<f32> {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).collect()
}

fn simd_mul_f32(a: &[f32], b: &[f32]) -> Vec<f32> {
    let mut result = Vec::with_capacity(a.len());

    for chunk in a.chunks_exact(8).zip(b.chunks_exact(8)) {
        let (a_chunk, b_chunk) = chunk;
        let cap_a = SimdF32x8Capsule::from_array([
            a_chunk[0], a_chunk[1], a_chunk[2], a_chunk[3], a_chunk[4], a_chunk[5], a_chunk[6],
            a_chunk[7],
        ]);
        let cap_b = SimdF32x8Capsule::from_array([
            b_chunk[0], b_chunk[1], b_chunk[2], b_chunk[3], b_chunk[4], b_chunk[5], b_chunk[6],
            b_chunk[7],
        ]);

        let product = cap_a.mul(&cap_b);
        result.extend_from_slice(&product.load());
    }

    let remainder_start = (a.len() / 8) * 8;
    for i in remainder_start..a.len() {
        result.push(a[i] * b[i]);
    }

    result
}

fn bench_mul_realistic(c: &mut Criterion) {
    let mut group = c.benchmark_group("f32_mul_realistic");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    for &size in ARRAY_SIZES {
        let a: Vec<f32> = (0..size).map(|i| i as f32 + 1.0).collect();
        let b: Vec<f32> = (0..size).map(|i| 2.0 / (i as f32 + 1.0)).collect();

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::new("scalar", size), &size, |bencher, _| {
            bencher.iter(|| black_box(scalar_mul_f32(&a, &b)));
        });

        group.bench_with_input(BenchmarkId::new("simd", size), &size, |bencher, _| {
            bencher.iter(|| black_box(simd_mul_f32(&a, &b)));
        });
    }

    group.finish();
}

// ============================================================================
// FMA (FUSED MULTIPLY-ADD) - F32
// ============================================================================

fn scalar_fma_f32(a: &[f32], b: &[f32], c: &[f32]) -> Vec<f32> {
    a.iter()
        .zip(b.iter())
        .zip(c.iter())
        .map(|((x, y), z)| x.mul_add(*y, *z))
        .collect()
}

fn simd_fma_f32(a: &[f32], b: &[f32], c: &[f32]) -> Vec<f32> {
    let mut result = Vec::with_capacity(a.len());

    for (chunk_a, (chunk_b, chunk_c)) in a
        .chunks_exact(8)
        .zip(b.chunks_exact(8).zip(c.chunks_exact(8)))
    {
        let cap_a = SimdF32x8Capsule::from_array([
            chunk_a[0], chunk_a[1], chunk_a[2], chunk_a[3], chunk_a[4], chunk_a[5], chunk_a[6],
            chunk_a[7],
        ]);
        let cap_b = SimdF32x8Capsule::from_array([
            chunk_b[0], chunk_b[1], chunk_b[2], chunk_b[3], chunk_b[4], chunk_b[5], chunk_b[6],
            chunk_b[7],
        ]);
        let cap_c = SimdF32x8Capsule::from_array([
            chunk_c[0], chunk_c[1], chunk_c[2], chunk_c[3], chunk_c[4], chunk_c[5], chunk_c[6],
            chunk_c[7],
        ]);

        let fma_result = cap_a.fma(&cap_b, &cap_c);
        result.extend_from_slice(&fma_result.load());
    }

    let remainder_start = (a.len() / 8) * 8;
    for i in remainder_start..a.len() {
        result.push(a[i].mul_add(b[i], c[i]));
    }

    result
}

fn bench_fma_realistic(c: &mut Criterion) {
    let mut group = c.benchmark_group("f32_fma_realistic");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    for &size in ARRAY_SIZES {
        let a: Vec<f32> = (0..size).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..size).map(|_| 2.0).collect();
        let c: Vec<f32> = (0..size).map(|_| 1.0).collect();

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::new("scalar", size), &size, |bencher, _| {
            bencher.iter(|| black_box(scalar_fma_f32(&a, &b, &c)));
        });

        group.bench_with_input(BenchmarkId::new("simd", size), &size, |bencher, _| {
            bencher.iter(|| black_box(simd_fma_f32(&a, &b, &c)));
        });
    }

    group.finish();
}

// ============================================================================
// DOT PRODUCT - F32
// ============================================================================

fn scalar_dot_f32(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn simd_dot_f32(a: &[f32], b: &[f32]) -> f32 {
    let mut sum = 0.0;

    for chunk in a.chunks_exact(8).zip(b.chunks_exact(8)) {
        let (a_chunk, b_chunk) = chunk;
        let cap_a = SimdF32x8Capsule::from_array([
            a_chunk[0], a_chunk[1], a_chunk[2], a_chunk[3], a_chunk[4], a_chunk[5], a_chunk[6],
            a_chunk[7],
        ]);
        let cap_b = SimdF32x8Capsule::from_array([
            b_chunk[0], b_chunk[1], b_chunk[2], b_chunk[3], b_chunk[4], b_chunk[5], b_chunk[6],
            b_chunk[7],
        ]);

        sum += cap_a.dot(&cap_b);
    }

    // Handle remainder
    let remainder_start = (a.len() / 8) * 8;
    for i in remainder_start..a.len() {
        sum += a[i] * b[i];
    }

    sum
}

fn bench_dot_product_realistic(c: &mut Criterion) {
    let mut group = c.benchmark_group("f32_dot_product_realistic");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    for &size in ARRAY_SIZES {
        let a: Vec<f32> = (0..size).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..size).map(|i| (size - i) as f32).collect();

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::new("scalar", size), &size, |bencher, _| {
            bencher.iter(|| black_box(scalar_dot_f32(&a, &b)));
        });

        group.bench_with_input(BenchmarkId::new("simd", size), &size, |bencher, _| {
            bencher.iter(|| black_box(simd_dot_f32(&a, &b)));
        });
    }

    group.finish();
}

// ============================================================================
// ELEMENT-WISE ABS - F32
// ============================================================================

fn scalar_abs_f32(a: &[f32]) -> Vec<f32> {
    a.iter().map(|x| x.abs()).collect()
}

fn simd_abs_f32(a: &[f32]) -> Vec<f32> {
    let mut result = Vec::with_capacity(a.len());

    for chunk in a.chunks_exact(8) {
        let cap = SimdF32x8Capsule::from_array([
            chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
        ]);

        let abs_result = cap.abs();
        result.extend_from_slice(&abs_result.load());
    }

    let remainder_start = (a.len() / 8) * 8;
    for i in remainder_start..a.len() {
        result.push(a[i].abs());
    }

    result
}

fn bench_abs_realistic(c: &mut Criterion) {
    let mut group = c.benchmark_group("f32_abs_realistic");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    for &size in ARRAY_SIZES {
        let a: Vec<f32> = (0..size)
            .map(|i| (i as f32) - (size as f32 / 2.0))
            .collect();

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::new("scalar", size), &size, |bencher, _| {
            bencher.iter(|| black_box(scalar_abs_f32(&a)));
        });

        group.bench_with_input(BenchmarkId::new("simd", size), &size, |bencher, _| {
            bencher.iter(|| black_box(simd_abs_f32(&a)));
        });
    }

    group.finish();
}

// ============================================================================
// ELEMENT-WISE MIN/MAX - F32
// ============================================================================

fn scalar_min_f32(a: &[f32], b: &[f32]) -> Vec<f32> {
    a.iter().zip(b.iter()).map(|(x, y)| x.min(*y)).collect()
}

fn simd_min_f32(a: &[f32], b: &[f32]) -> Vec<f32> {
    let mut result = Vec::with_capacity(a.len());

    for chunk in a.chunks_exact(8).zip(b.chunks_exact(8)) {
        let (a_chunk, b_chunk) = chunk;
        let cap_a = SimdF32x8Capsule::from_array([
            a_chunk[0], a_chunk[1], a_chunk[2], a_chunk[3], a_chunk[4], a_chunk[5], a_chunk[6],
            a_chunk[7],
        ]);
        let cap_b = SimdF32x8Capsule::from_array([
            b_chunk[0], b_chunk[1], b_chunk[2], b_chunk[3], b_chunk[4], b_chunk[5], b_chunk[6],
            b_chunk[7],
        ]);

        let min_result = cap_a.simd_min(&cap_b);
        result.extend_from_slice(&min_result.load());
    }

    let remainder_start = (a.len() / 8) * 8;
    for i in remainder_start..a.len() {
        result.push(a[i].min(b[i]));
    }

    result
}

fn bench_min_realistic(c: &mut Criterion) {
    let mut group = c.benchmark_group("f32_min_realistic");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    for &size in ARRAY_SIZES {
        let a: Vec<f32> = (0..size).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..size).map(|i| (size - i) as f32).collect();

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::new("scalar", size), &size, |bencher, _| {
            bencher.iter(|| black_box(scalar_min_f32(&a, &b)));
        });

        group.bench_with_input(BenchmarkId::new("simd", size), &size, |bencher, _| {
            bencher.iter(|| black_box(simd_min_f32(&a, &b)));
        });
    }

    group.finish();
}

// ============================================================================
// REALISTIC TRADING WORKLOAD: RISK AGGREGATION (512 positions)
// ============================================================================

/// Realistic kindly_hft scenario: aggregate risk across 512 positions
fn scalar_risk_aggregation(positions: &[f32], weights: &[f32]) -> f32 {
    positions
        .iter()
        .zip(weights.iter())
        .map(|(p, w)| p * w)
        .sum()
}

fn simd_risk_aggregation(positions: &[f32], weights: &[f32]) -> f32 {
    let mut sum = 0.0;

    for chunk in positions.chunks_exact(8).zip(weights.chunks_exact(8)) {
        let (pos_chunk, wt_chunk) = chunk;
        let cap_pos = SimdF32x8Capsule::from_array([
            pos_chunk[0],
            pos_chunk[1],
            pos_chunk[2],
            pos_chunk[3],
            pos_chunk[4],
            pos_chunk[5],
            pos_chunk[6],
            pos_chunk[7],
        ]);
        let cap_wt = SimdF32x8Capsule::from_array([
            wt_chunk[0],
            wt_chunk[1],
            wt_chunk[2],
            wt_chunk[3],
            wt_chunk[4],
            wt_chunk[5],
            wt_chunk[6],
            wt_chunk[7],
        ]);

        sum += cap_pos.dot(&cap_wt);
    }

    let remainder_start = (positions.len() / 8) * 8;
    for i in remainder_start..positions.len() {
        sum += positions[i] * weights[i];
    }

    sum
}

fn bench_trading_risk_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("trading_risk_aggregation");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Realistic sizes for trading systems
    let sizes = [256, 512, 1024];

    for &size in &sizes {
        let positions: Vec<f32> = (0..size).map(|i| (i as f32) * 100.0).collect();
        let weights: Vec<f32> = (0..size).map(|_| 1.0 / (size as f32)).collect();

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::new("scalar", size), &size, |bencher, _| {
            bencher.iter(|| black_box(scalar_risk_aggregation(&positions, &weights)));
        });

        group.bench_with_input(BenchmarkId::new("simd", size), &size, |bencher, _| {
            bencher.iter(|| black_box(simd_risk_aggregation(&positions, &weights)));
        });
    }

    group.finish();
}

// ============================================================================
// REALISTIC BRAIN WORKLOAD: HEBBIAN LEARNING (5000 connections)
// ============================================================================

/// Hebbian update: weights[i] += lr * pre[i] * post
fn scalar_hebbian_update(weights: &mut [f32], pre: &[f32], post: f32, lr: f32) {
    for i in 0..weights.len() {
        weights[i] += lr * pre[i] * post;
    }
}

fn simd_hebbian_update(weights: &mut [f32], pre: &[f32], post: f32, lr: f32) {
    let lr_post = lr * post;
    let splat_lr_post = SimdF32x8Capsule::splat(lr_post);

    for (i, chunk) in pre.chunks_exact(8).enumerate() {
        let cap_pre = SimdF32x8Capsule::from_array([
            chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
        ]);
        let cap_wt = SimdF32x8Capsule::from_array([
            weights[i * 8],
            weights[i * 8 + 1],
            weights[i * 8 + 2],
            weights[i * 8 + 3],
            weights[i * 8 + 4],
            weights[i * 8 + 5],
            weights[i * 8 + 6],
            weights[i * 8 + 7],
        ]);

        // weights += lr_post * pre
        let delta = cap_pre.mul(&splat_lr_post);
        let new_wt = cap_wt.add(&delta);
        let new_wt_data = new_wt.load();

        weights[i * 8..i * 8 + 8].copy_from_slice(&new_wt_data);
    }

    // Handle remainder
    let remainder_start = (pre.len() / 8) * 8;
    for i in remainder_start..pre.len() {
        weights[i] += lr * pre[i] * post;
    }
}

fn bench_brain_hebbian_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("brain_hebbian_update");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Realistic sizes for brain training
    let sizes = [512, 1024, 5000];

    for &size in &sizes {
        let pre: Vec<f32> = (0..size).map(|i| (i as f32) / (size as f32)).collect();
        let post = 0.5f32;
        let lr = 0.01f32;

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::new("scalar", size), &size, |bencher, _| {
            bencher.iter_batched(
                || vec![0.0f32; size],
                |mut weights| {
                    scalar_hebbian_update(&mut weights, &pre, post, lr);
                    black_box(weights)
                },
                criterion::BatchSize::SmallInput,
            );
        });

        group.bench_with_input(BenchmarkId::new("simd", size), &size, |bencher, _| {
            bencher.iter_batched(
                || vec![0.0f32; size],
                |mut weights| {
                    simd_hebbian_update(&mut weights, &pre, post, lr);
                    black_box(weights)
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

// ============================================================================
// CRITERION GROUPS
// ============================================================================

criterion_group!(
    arithmetic_benches,
    bench_add_realistic,
    bench_mul_realistic,
    bench_fma_realistic,
);

criterion_group!(reduction_benches, bench_dot_product_realistic,);

criterion_group!(
    element_wise_benches,
    bench_abs_realistic,
    bench_min_realistic,
);

criterion_group!(
    realistic_workloads,
    bench_trading_risk_aggregation,
    bench_brain_hebbian_update,
);

criterion_main!(
    arithmetic_benches,
    reduction_benches,
    element_wise_benches,
    realistic_workloads,
);
