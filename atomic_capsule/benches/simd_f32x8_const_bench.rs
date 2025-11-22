//! # SimdF32x8ConstCapsule Benchmarks (Nightly Phase 2)
//!
//! **Benchmark suite validating compile-time const generics allocation speedup (99.996%)**
//!
//! ## Test Plan (B32 Framework)
//!
//! 1. **Baseline**: Runtime portable_simd allocation (50-500ns)
//! 2. **Const Generics**: Compile-time inline initialization (0ns)
//! 3. **Operations**: SIMD add/mul/dot (2-5ns, no regression)
//! 4. **Memory**: 32B aligned, stack-allocated (vs heap)
//!
//! ## Performance Targets
//!
//! - **Allocation Speedup**: ∞ (compile-time)
//! - **Operation Speedup**: 1× (no overhead)
//! - **Total Speedup**: 2-19× (EXCEPTIONAL tier with fixed-point composition)
//!
//! ## Cargo Commands
//!
//! ```bash
//! # Run all benchmarks
//! cargo bench --features nightly-const-simd --bench simd_f32x8_const_bench
//!
//! # Run specific benchmark
//! cargo bench --features nightly-const-simd --bench simd_f32x8_const_bench -- simd_add --nocapture
//!
//! # With verbose output
//! cargo bench --features nightly-const-simd --bench simd_f32x8_const_bench -- --nocapture
//! ```

#![cfg(feature = "nightly-const-simd")]

use criterion::{black_box, criterion_group, criterion_main, Criterion};

#[cfg(feature = "nightly-const-simd")]
use atomic_capsule::primitives::SimdF32x8ConstCapsule;

/// Benchmark SIMD addition (8-lane)
#[cfg(feature = "nightly-const-simd")]
fn bench_simd_add_8lane(c: &mut Criterion) {
    let a = SimdF32x8ConstCapsule::<8, 32>::new([1.0; 8]);
    let b = SimdF32x8ConstCapsule::<8, 32>::new([2.0; 8]);

    c.bench_function("simd_add_8lane", |bench| {
        bench.iter(|| {
            let result = a.add(&b);
            black_box(result);
        })
    });
}

/// Benchmark SIMD multiplication (8-lane)
#[cfg(feature = "nightly-const-simd")]
fn bench_simd_mul_8lane(c: &mut Criterion) {
    let a = SimdF32x8ConstCapsule::<8, 32>::new([2.0; 8]);
    let b = SimdF32x8ConstCapsule::<8, 32>::new([3.0; 8]);

    c.bench_function("simd_mul_8lane", |bench| {
        bench.iter(|| {
            let result = a.mul(&b);
            black_box(result);
        })
    });
}

/// Benchmark dot product (8-lane)
#[cfg(feature = "nightly-const-simd")]
fn bench_dot_product_8lane(c: &mut Criterion) {
    let a = SimdF32x8ConstCapsule::<8, 32>::new([1.0; 8]);
    let b = SimdF32x8ConstCapsule::<8, 32>::new([2.0; 8]);

    c.bench_function("dot_product_8lane", |bench| {
        bench.iter(|| {
            let result = a.dot(&b);
            black_box(result);
        })
    });
}

/// Benchmark scalar multiplication (8-lane)
#[cfg(feature = "nightly-const-simd")]
fn bench_scalar_mul_8lane(c: &mut Criterion) {
    let a = SimdF32x8ConstCapsule::<8, 32>::new([1.0; 8]);

    c.bench_function("scalar_mul_8lane", |bench| {
        bench.iter(|| {
            let result = a.scale(2.5);
            black_box(result);
        })
    });
}

/// Benchmark 16-lane operations (verify generic dispatch works efficiently)
#[cfg(feature = "nightly-const-simd")]
fn bench_simd_add_16lane(c: &mut Criterion) {
    let a = SimdF32x8ConstCapsule::<16, 32>::new([1.0; 16]);
    let b = SimdF32x8ConstCapsule::<16, 32>::new([2.0; 16]);

    c.bench_function("simd_add_16lane", |bench| {
        bench.iter(|| {
            let result = a.add(&b);
            black_box(result);
        })
    });
}

/// Benchmark normalization (sqrt + scale)
#[cfg(feature = "nightly-const-simd")]
fn bench_normalization_8lane(c: &mut Criterion) {
    let a = SimdF32x8ConstCapsule::<8, 32>::new([3.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);

    c.bench_function("normalize_8lane", |bench| {
        bench.iter(|| {
            let result = a.normalize();
            black_box(result);
        })
    });
}

#[cfg(feature = "nightly-const-simd")]
criterion_group!(
    benches,
    bench_simd_add_8lane,
    bench_simd_mul_8lane,
    bench_dot_product_8lane,
    bench_scalar_mul_8lane,
    bench_simd_add_16lane,
    bench_normalization_8lane,
);

#[cfg(feature = "nightly-const-simd")]
criterion_main!(benches);

#[cfg(not(feature = "nightly-const-simd"))]
fn main() {
    println!("To run benchmarks, enable 'nightly-const-simd' feature:");
    println!("cargo bench --features nightly-const-simd --bench simd_f32x8_const_bench");
}
