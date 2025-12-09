//! Benchmarks for CompressionStateCapsule (Tier 6 Mixed SIMD+Fixed-Point+Atomic)
//!
//! ## B32 Framework Compliance
//! - Fair baseline: Compare against scalar implementation (not strawman)
//! - Statistical rigor: Criterion provides 1000+ samples, 95% CI
//! - Honest reporting: Document where optimizations help and where they don't
//! - Reality checks: Expect 2-4× SIMD speedup, <50ns operations
//!
//! ## Expected Results
//! - record(): <50ns (target), <100ns typical
//! - compression_ratio_bp(): <5ns (single atomic load)
//! - histogram_scalar: ~100ns for 256 bytes
//! - histogram_simd: ~30ns for 256 bytes (3-4× speedup)
//!
//! ## Hardware Reality (B32 K2, K9)
//! - Atomic CAS: 10-20ns (hardware reality)
//! - SIMD throughput: 2-4× typical (3-8× theoretical)
//! - L1 cache hit: <1ns (128-byte alignment)
//! - Memory bandwidth: 32GB/s typical (DDR4-3200)

use clapi_core::capsules::{compute_histogram, compute_histogram_scalar, CompressionStateCapsule};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

#[cfg(feature = "simd")]
use clapi_core::capsules::compute_histogram_simd;

/// Benchmark: record() operation
///
/// ## Target
/// - <50ns per operation
///
/// ## Reality Check (B32 K2)
/// - Atomic fetch_add: ~5ns (hardware limit)
/// - 3× fetch_add + division + generation = ~25ns minimum
/// - Realistic: 30-50ns
fn bench_record(c: &mut Criterion) {
    let capsule = CompressionStateCapsule::new();

    c.bench_function("compression_state_record", |b| {
        b.iter(|| {
            capsule.record(black_box(1000), black_box(650));
        });
    });
}

/// Benchmark: compression_ratio_bp() query
///
/// ## Target
/// - <5ns per operation (single atomic load)
///
/// ## Reality Check (B32 K2)
/// - Atomic load: ~1ns (L1 cache hit guaranteed)
/// - Realistic: 2-5ns
fn bench_compression_ratio(c: &mut Criterion) {
    let capsule = CompressionStateCapsule::new();
    capsule.record(1000, 650); // Prime with data

    c.bench_function("compression_state_ratio_bp", |b| {
        b.iter(|| {
            black_box(capsule.compression_ratio_bp());
        });
    });
}

/// Benchmark: snapshot() operation
///
/// ## Target
/// - <20ns (4× atomic loads)
///
/// ## Reality Check (B32 K2)
/// - 4× atomic load: ~4ns minimum
/// - Realistic: 10-20ns
fn bench_snapshot(c: &mut Criterion) {
    let capsule = CompressionStateCapsule::new();
    capsule.record(1000, 650);

    c.bench_function("compression_state_snapshot", |b| {
        b.iter(|| {
            black_box(capsule.snapshot());
        });
    });
}

/// Benchmark: Histogram computation (scalar)
///
/// ## Expected
/// - ~100ns for 256 bytes
/// - Throughput: ~2.5 GB/s
///
/// ## Reality Check (B32 K9)
/// - 256 byte loads + 256 increments = ~100ns realistic
fn bench_histogram_scalar(c: &mut Criterion) {
    let mut group = c.benchmark_group("histogram_scalar");

    for size in [64, 256, 1024, 4096].iter() {
        let data = vec![0u8; *size];

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                black_box(compute_histogram_scalar(black_box(&data)));
            });
        });
    }

    group.finish();
}

/// Benchmark: Histogram computation (SIMD vs Scalar)
///
/// ## Expected Speedup
/// - 3-4× for large inputs (>256 bytes)
/// - 1-2× for small inputs (<64 bytes, SIMD overhead)
///
/// ## Reality Check (B32 K9)
/// - SIMD setup overhead: ~10ns
/// - Amortized over 256 bytes: 3-4× speedup realistic
#[cfg(feature = "simd")]
fn bench_histogram_simd_vs_scalar(c: &mut Criterion) {
    let mut group = c.benchmark_group("histogram_simd_vs_scalar");

    for size in [64, 256, 1024, 4096].iter() {
        let data = vec![0u8; *size];

        // Scalar baseline
        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            b.iter(|| {
                black_box(compute_histogram_scalar(black_box(&data)));
            });
        });

        // SIMD optimized
        group.bench_with_input(BenchmarkId::new("simd", size), size, |b, _| {
            b.iter(|| {
                black_box(compute_histogram_simd(black_box(&data)));
            });
        });
    }

    group.finish();
}

/// Benchmark: Histogram computation (auto-select)
///
/// Tests the `compute_histogram()` function which automatically selects
/// SIMD or scalar based on feature flags.
fn bench_histogram_auto(c: &mut Criterion) {
    let mut group = c.benchmark_group("histogram_auto");

    for size in [64, 256, 1024, 4096].iter() {
        let data = vec![0u8; *size];

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                black_box(compute_histogram(black_box(&data)));
            });
        });
    }

    group.finish();
}

/// Benchmark: Concurrent record operations
///
/// ## Expected
/// - Linear scaling up to 4 threads
/// - Contention at 8+ threads (CAS retries)
///
/// ## Reality Check (B32 K2)
/// - Atomic CAS contention: Expect slowdown at 8+ threads
/// - Relaxed ordering: Minimal overhead (<5ns)
fn bench_concurrent_record(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;

    let mut group = c.benchmark_group("concurrent_record");

    for thread_count in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            thread_count,
            |b, &threads| {
                b.iter(|| {
                    let capsule = Arc::new(CompressionStateCapsule::new());
                    let mut handles = vec![];

                    for _ in 0..threads {
                        let c = Arc::clone(&capsule);
                        handles.push(thread::spawn(move || {
                            for _ in 0..100 {
                                c.record(black_box(1000), black_box(650));
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Compression ratio computation overhead
///
/// ## Expected
/// - Division overhead: ~5ns
/// - Total with atomics: ~10-15ns
///
/// ## Reality Check (B32 K2)
/// - Integer division: ~3-5ns (hardware)
/// - Realistic: 10-15ns with atomics
fn bench_ratio_computation(c: &mut Criterion) {
    let capsule = CompressionStateCapsule::new();

    c.bench_function("compression_state_ratio_computation", |b| {
        b.iter(|| {
            // Simulate record() without accumulation (isolate ratio computation)
            capsule.record(black_box(1000), black_box(650));
            black_box(capsule.compression_ratio_bp());
        });
    });
}

criterion_group!(
    benches,
    bench_record,
    bench_compression_ratio,
    bench_snapshot,
    bench_histogram_scalar,
    bench_histogram_auto,
    bench_concurrent_record,
    bench_ratio_computation,
);

#[cfg(feature = "simd")]
criterion_group!(simd_benches, bench_histogram_simd_vs_scalar,);

#[cfg(feature = "simd")]
criterion_main!(benches, simd_benches);

#[cfg(not(feature = "simd"))]
criterion_main!(benches);
