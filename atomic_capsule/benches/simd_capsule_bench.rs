//! B32-compliant SIMD capsule benchmarks
//!
//! This benchmark suite validates computational capsule performance following
//! the B32 framework for fair, reproducible, and statistically valid measurements.

#![feature(portable_simd)]

use atomic_capsule::primitives::SimdCapsule;
use atomic_capsule::SimdF32x8Capsule;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::time::Duration;

/// B32 B1: Fair baseline - optimized scalar implementation
fn baseline_scalar_dot_product(a: &[f32; 8], b: &[f32; 8]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// B32 B3: Realistic dot product benchmark with multiple baselines
fn bench_dot_product(c: &mut Criterion) {
    let mut group = c.benchmark_group("dot_product");

    // B32 B2: Statistical rigor
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(3));

    let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let b = [8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];

    // B32 B1: Baseline - Optimized scalar with iterator fusion
    group.bench_function("scalar", |bencher| {
        bencher.iter(|| black_box(baseline_scalar_dot_product(&a, &b)));
    });

    // B32: SIMD capsule (aligned, optimized)
    group.bench_function("simd_capsule", |bencher| {
        let cap_a = SimdF32x8Capsule::from_array(a);
        let cap_b = SimdF32x8Capsule::from_array(b);

        bencher.iter(|| black_box(cap_a.dot(&cap_b)));
    });

    group.finish();
}

/// B32 B3: Realistic element-wise operations
fn bench_element_wise_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("element_wise");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let b = [8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];

    // Scalar baseline - element-wise multiply
    group.bench_function("multiply_scalar", |bencher| {
        bencher.iter(|| {
            let mut result = [0.0f32; 8];
            for i in 0..8 {
                result[i] = a[i] * b[i];
            }
            black_box(result)
        });
    });

    // SIMD capsule - element-wise multiply
    group.bench_function("multiply_simd", |bencher| {
        let cap_a = SimdF32x8Capsule::from_array(a);
        let cap_b = SimdF32x8Capsule::from_array(b);

        bencher.iter(|| {
            let result = cap_a.mul(&cap_b);
            black_box(result.load())
        });
    });

    // Scalar baseline - element-wise add
    group.bench_function("add_scalar", |bencher| {
        bencher.iter(|| {
            let mut result = [0.0f32; 8];
            for i in 0..8 {
                result[i] = a[i] + b[i];
            }
            black_box(result)
        });
    });

    // SIMD capsule - element-wise add
    group.bench_function("add_simd", |bencher| {
        let cap_a = SimdF32x8Capsule::from_array(a);
        let cap_b = SimdF32x8Capsule::from_array(b);

        bencher.iter(|| {
            let result = cap_a.add(&cap_b);
            black_box(result.load())
        });
    });

    group.finish();
}

/// B32 B8: Cache behavior validation
fn bench_cache_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_efficiency");

    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Cold cache - create new capsule each iteration
    group.bench_function("simd_cold_cache", |bencher| {
        bencher.iter_batched(
            || SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]),
            |cap| black_box(cap.load()),
            criterion::BatchSize::PerIteration,
        );
    });

    // Warm cache - reuse same capsule
    group.bench_function("simd_warm_cache", |bencher| {
        let cap = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);

        bencher.iter(|| black_box(cap.load()));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_dot_product,
    bench_element_wise_operations,
    bench_cache_efficiency,
);

criterion_main!(benches);
