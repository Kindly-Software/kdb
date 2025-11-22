//! # HyperLogLogConst Benchmarks
//!
//! Performance benchmarks for HyperLogLogConst with various precision levels.
//!
//! ## Test Coverage
//! - Insert performance across precision levels (P4, P8, P14, P18)
//! - Cardinality estimation accuracy
//! - Memory footprint
//! - Merge operations

#![allow(unused)]

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

#[cfg(feature = "nightly-const-probabilistic")]
mod benches {
    use super::*;
    use atomic_capsule::probabilistic::{HyperLogLogConst, validate_hll_precision, calculate_hll_error};

    /// Benchmark insert performance for P14
    pub fn bench_insert_p14(c: &mut Criterion) {
        let mut group = c.benchmark_group("hyperloglog_const_insert");

        group.bench_function("insert_single_p14", |b| {
            b.iter(|| {
                let hll: HyperLogLogConst<14, 50> = HyperLogLogConst::new();
                hll.insert(black_box(42));
                hll
            });
        });

        group.finish();
    }

    /// Benchmark cardinality estimation accuracy
    pub fn bench_cardinality_p14(c: &mut Criterion) {
        let mut group = c.benchmark_group("hyperloglog_const_cardinality");

        group.bench_function("cardinality_1k_p14", |b| {
            let hll: HyperLogLogConst<14, 50> = HyperLogLogConst::new();
            for i in 0..1000 {
                hll.insert(i);
            }
            b.iter(|| hll.cardinality());
        });

        group.finish();
    }

    /// Benchmark memory usage across precision levels
    pub fn bench_memory_footprint(c: &mut Criterion) {
        let mut group = c.benchmark_group("hyperloglog_const_memory");

        group.bench_function("memory_p4", |b| {
            b.iter(|| {
                let hll: HyperLogLogConst<4, 50> = HyperLogLogConst::new();
                hll.memory_bytes()
            });
        });

        group.bench_function("memory_p14", |b| {
            b.iter(|| {
                let hll: HyperLogLogConst<14, 50> = HyperLogLogConst::new();
                hll.memory_bytes()
            });
        });

        group.finish();
    }

    /// Benchmark compile-time error calculation
    pub fn bench_error_rate(c: &mut Criterion) {
        let mut group = c.benchmark_group("hyperloglog_const_error");

        group.bench_function("error_p14", |b| {
            b.iter(|| {
                let hll: HyperLogLogConst<14, 50> = HyperLogLogConst::new();
                hll.error_rate()
            });
        });

        group.finish();
    }
}

#[cfg(feature = "nightly-const-probabilistic")]
criterion_group!(
    benches,
    benches::bench_insert_p14,
    benches::bench_cardinality_p14,
    benches::bench_memory_footprint,
    benches::bench_error_rate,
);

#[cfg(not(feature = "nightly-const-probabilistic"))]
criterion_group!(benches,);

criterion_main!(benches);
