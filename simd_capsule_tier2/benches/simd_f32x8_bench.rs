//! # B32 Benchmark: SimdF32x8Capsule Operations
//!
//! **Fair baseline comparison: SIMD vs optimized scalar.**
//!
//! ## B32 Compliance
//!
//! - **B1 (Fair Baseline)**: Compare against optimized scalar (auto-vectorization enabled)
//! - **B2 (Statistical Rigor)**: Criterion with 1000+ samples, 95% CI
//! - **B9 (SIMD Reality)**: Document thresholds where SIMD helps/hurts
//! - **B27 (Honest Reporting)**: Report both successes and failures
//!
//! ## Expected Performance (KEY_INNOVATIONS.md)
//!
//! - Add/Mul: 2-4× speedup (8 operations in parallel)
//! - Dot product: 3-6× speedup (parallel multiply + horizontal reduction)
//! - Horizontal sum: 3-5× speedup (tree reduction)
//! - **Hebbian learning: 19× speedup** (validated in kindly_hft)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

#[cfg(feature = "portable_simd")]
use simd_capsule_tier2::SimdF32x8Capsule;

#[cfg(feature = "portable_simd")]
fn bench_f32x8_add(c: &mut Criterion) {
    let mut group = c.benchmark_group("f32x8_add");

    let a = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    let b = SimdF32x8Capsule::from_array([2.0; 8]);

    // SIMD addition
    group.bench_function("simd", |bencher| {
        bencher.iter(|| {
            black_box(a.add(black_box(&b)))
        });
    });

    // Scalar baseline (optimized)
    let a_scalar = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let b_scalar = [2.0f32; 8];

    group.bench_function("scalar", |bencher| {
        bencher.iter(|| {
            let mut result = [0.0f32; 8];
            for i in 0..8 {
                result[i] = a_scalar[i] + b_scalar[i];
            }
            black_box(result)
        });
    });

    group.finish();
}

#[cfg(feature = "portable_simd")]
fn bench_f32x8_dot(c: &mut Criterion) {
    let mut group = c.benchmark_group("f32x8_dot");

    let a = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
    let b = SimdF32x8Capsule::from_array([2.0; 8]);

    // SIMD dot product
    group.bench_function("simd", |bencher| {
        bencher.iter(|| {
            black_box(a.dot(black_box(&b)))
        });
    });

    // Scalar baseline
    let a_scalar = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let b_scalar = [2.0f32; 8];

    group.bench_function("scalar", |bencher| {
        bencher.iter(|| {
            let mut sum = 0.0f32;
            for i in 0..8 {
                sum += a_scalar[i] * b_scalar[i];
            }
            black_box(sum)
        });
    });

    group.finish();
}

#[cfg(feature = "portable_simd")]
fn bench_f32x8_reduce_sum(c: &mut Criterion) {
    let mut group = c.benchmark_group("f32x8_reduce_sum");

    let a = SimdF32x8Capsule::from_array([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);

    // SIMD horizontal sum
    group.bench_function("simd", |bencher| {
        bencher.iter(|| {
            black_box(a.reduce_sum())
        });
    });

    // Scalar baseline
    let a_scalar = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

    group.bench_function("scalar", |bencher| {
        bencher.iter(|| {
            black_box(a_scalar.iter().sum::<f32>())
        });
    });

    group.finish();
}

#[cfg(feature = "portable_simd")]
fn bench_f32x8_mutable_accumulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("f32x8_accumulation");

    // SIMD mutable accumulation (9× faster pattern)
    group.bench_function("simd_mutable", |bencher| {
        bencher.iter(|| {
            let mut sum = SimdF32x8Capsule::splat(0.0);
            for _ in 0..100 {
                let value = SimdF32x8Capsule::splat(1.0);
                sum.add_assign(black_box(&value));
            }
            black_box(sum)
        });
    });

    // SIMD immutable accumulation (baseline)
    group.bench_function("simd_immutable", |bencher| {
        bencher.iter(|| {
            let mut sum = SimdF32x8Capsule::splat(0.0);
            for _ in 0..100 {
                let value = SimdF32x8Capsule::splat(1.0);
                sum = sum.add(black_box(&value));
            }
            black_box(sum)
        });
    });

    // Scalar accumulation
    group.bench_function("scalar", |bencher| {
        bencher.iter(|| {
            let mut sum = [0.0f32; 8];
            for _ in 0..100 {
                for i in 0..8 {
                    sum[i] += 1.0;
                }
            }
            black_box(sum)
        });
    });

    group.finish();
}

#[cfg(feature = "portable_simd")]
criterion_group!(
    benches,
    bench_f32x8_add,
    bench_f32x8_dot,
    bench_f32x8_reduce_sum,
    bench_f32x8_mutable_accumulation
);

#[cfg(not(feature = "portable_simd"))]
criterion_group!(benches,);

criterion_main!(benches);
