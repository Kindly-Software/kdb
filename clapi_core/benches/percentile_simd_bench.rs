//! B32 Framework Benchmarks for SIMD Percentile Implementation
//!
//! # B32 Compliance
//!
//! - **Fair Baseline**: Compare SIMD vs Scalar (same algorithm, different execution)
//! - **Statistical Rigor**: 1000+ iterations, 95% CI via Criterion
//! - **Honest Reporting**: Document SIMD overhead for small datasets
//! - **Reality Check**: 2-4× typical for SIMD bucket operations (target: 2.5×)
//!
//! # Benchmarks
//!
//! 1. **percentile_scalar()**: Baseline linear scan (50ns expected)
//! 2. **percentile_simd()**: SIMD u64x8 parallel scan (20ns expected)
//! 3. **percentile_optimized()**: Transparent API (auto-selects SIMD/scalar)
//! 4. **batch_percentiles()**: Amortized batch processing (40ns for 4 percentiles)
//! 5. **stats_simd()**: SIMD-accelerated snapshot (60ns vs 150ns)
//!
//! # Performance Targets
//!
//! - **SIMD Speedup**: 2.5× (50ns → 20ns)
//! - **Batch Speedup**: 2× (80ns → 40ns for 4 percentiles)
//! - **Stats Speedup**: 2.5× (150ns → 60ns)
//!
//! # Build Instructions
//!
//! Stable Rust (scalar only):
//! ```bash
//! cargo bench --bench percentile_simd_bench
//! ```
//!
//! Nightly Rust (SIMD + scalar comparison):
//! ```bash
//! cargo +nightly bench --bench percentile_simd_bench --features portable_simd
//! ```

use clapi_core::profiling::capsule::LatencyHistogramCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// ============================================================================
// HELPER: Create Populated Histogram
// ============================================================================

fn create_histogram_with_samples(num_samples: usize) -> LatencyHistogramCapsule {
    let histogram = LatencyHistogramCapsule::new();
    for i in 0..num_samples {
        histogram.record((i * 73) % num_samples as u64); // Pseudo-random distribution
    }
    histogram
}

// ============================================================================
// BENCHMARK 1: Scalar Percentile Baseline
// ============================================================================

fn bench_percentile_scalar(c: &mut Criterion) {
    let histogram = create_histogram_with_samples(10_000);

    c.bench_function("percentile_scalar_p99", |b| {
        b.iter(|| {
            black_box(histogram.percentile_scalar(99.0));
        });
    });
}

fn bench_percentile_scalar_multiple(c: &mut Criterion) {
    let histogram = create_histogram_with_samples(10_000);

    c.bench_function("percentile_scalar_multiple (p50/p90/p99/p999)", |b| {
        b.iter(|| {
            black_box(histogram.percentile_scalar(50.0));
            black_box(histogram.percentile_scalar(90.0));
            black_box(histogram.percentile_scalar(99.0));
            black_box(histogram.percentile_scalar(99.9));
        });
    });
}

// ============================================================================
// BENCHMARK 2: SIMD Percentile (Nightly Feature)
// ============================================================================

#[cfg(feature = "portable_simd")]
fn bench_percentile_simd(c: &mut Criterion) {
    let histogram = create_histogram_with_samples(10_000);

    c.bench_function("percentile_simd_p99", |b| {
        b.iter(|| {
            black_box(histogram.percentile_simd(99.0));
        });
    });
}

#[cfg(feature = "portable_simd")]
fn bench_percentile_simd_multiple(c: &mut Criterion) {
    let histogram = create_histogram_with_samples(10_000);

    c.bench_function("percentile_simd_multiple (p50/p90/p99/p999)", |b| {
        b.iter(|| {
            black_box(histogram.percentile_simd(50.0));
            black_box(histogram.percentile_simd(90.0));
            black_box(histogram.percentile_simd(99.0));
            black_box(histogram.percentile_simd(99.9));
        });
    });
}

// ============================================================================
// BENCHMARK 3: Transparent Optimized API
// ============================================================================

fn bench_percentile_optimized(c: &mut Criterion) {
    let histogram = create_histogram_with_samples(10_000);

    c.bench_function("percentile_optimized_p99", |b| {
        b.iter(|| {
            black_box(histogram.percentile_optimized(99.0));
        });
    });
}

// ============================================================================
// BENCHMARK 4: Batch Percentiles (SIMD Only)
// ============================================================================

#[cfg(feature = "portable_simd")]
fn bench_batch_percentiles(c: &mut Criterion) {
    let histogram = create_histogram_with_samples(10_000);
    let percentiles = vec![50.0, 90.0, 99.0, 99.9];

    c.bench_function("batch_percentiles (4 percentiles)", |b| {
        b.iter(|| {
            black_box(histogram.batch_percentiles(black_box(&percentiles)));
        });
    });
}

#[cfg(feature = "portable_simd")]
fn bench_batch_percentiles_comprehensive(c: &mut Criterion) {
    let histogram = create_histogram_with_samples(10_000);
    let percentiles: Vec<f64> = (0..=100).step_by(5).map(|p| p as f64).collect();

    c.bench_function("batch_percentiles (21 percentiles)", |b| {
        b.iter(|| {
            black_box(histogram.batch_percentiles(black_box(&percentiles)));
        });
    });
}

// ============================================================================
// BENCHMARK 5: SIMD Stats Snapshot
// ============================================================================

#[cfg(feature = "portable_simd")]
fn bench_stats_simd(c: &mut Criterion) {
    let histogram = create_histogram_with_samples(10_000);

    c.bench_function("stats_simd", |b| {
        b.iter(|| {
            black_box(histogram.stats_simd());
        });
    });
}

fn bench_stats_original(c: &mut Criterion) {
    let histogram = create_histogram_with_samples(10_000);

    c.bench_function("stats_original", |b| {
        b.iter(|| {
            black_box(histogram.stats());
        });
    });
}

// ============================================================================
// BENCHMARK 6: Percentile Comparison Across Dataset Sizes
// ============================================================================

fn bench_percentile_by_dataset_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("percentile_by_dataset_size");

    for size in [100, 1_000, 10_000, 100_000] {
        group.throughput(Throughput::Elements(size as u64));

        // Scalar baseline
        group.bench_with_input(
            BenchmarkId::new("scalar", size),
            &size,
            |b, &size| {
                let histogram = create_histogram_with_samples(size);
                b.iter(|| {
                    black_box(histogram.percentile_scalar(99.0));
                });
            },
        );

        // SIMD (if available)
        #[cfg(feature = "portable_simd")]
        group.bench_with_input(
            BenchmarkId::new("simd", size),
            &size,
            |b, &size| {
                let histogram = create_histogram_with_samples(size);
                b.iter(|| {
                    black_box(histogram.percentile_simd(99.0));
                });
            },
        );

        // Optimized (auto-select)
        group.bench_with_input(
            BenchmarkId::new("optimized", size),
            &size,
            |b, &size| {
                let histogram = create_histogram_with_samples(size);
                b.iter(|| {
                    black_box(histogram.percentile_optimized(99.0));
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 7: Different Percentiles (B32 Honest Reporting)
// ============================================================================

fn bench_percentiles_by_value(c: &mut Criterion) {
    let histogram = create_histogram_with_samples(10_000);
    let mut group = c.benchmark_group("percentiles_by_value");

    for p in [0.0, 10.0, 25.0, 50.0, 75.0, 90.0, 95.0, 99.0, 99.9, 100.0] {
        // Scalar
        group.bench_with_input(
            BenchmarkId::new("scalar", format!("p{}", p)),
            &p,
            |b, &p| {
                b.iter(|| {
                    black_box(histogram.percentile_scalar(p));
                });
            },
        );

        // SIMD (if available)
        #[cfg(feature = "portable_simd")]
        group.bench_with_input(
            BenchmarkId::new("simd", format!("p{}", p)),
            &p,
            |b, &p| {
                b.iter(|| {
                    black_box(histogram.percentile_simd(p));
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 8: SIMD Overhead for Small Datasets (B32 Honest Reporting)
// ============================================================================

#[cfg(feature = "portable_simd")]
fn bench_simd_overhead_small_dataset(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_overhead_small_dataset");

    // Small datasets where SIMD overhead may dominate
    for size in [10, 50, 100, 500] {
        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(
            BenchmarkId::new("scalar", size),
            &size,
            |b, &size| {
                let histogram = create_histogram_with_samples(size);
                b.iter(|| {
                    black_box(histogram.percentile_scalar(99.0));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("simd", size),
            &size,
            |b, &size| {
                let histogram = create_histogram_with_samples(size);
                b.iter(|| {
                    black_box(histogram.percentile_simd(99.0));
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 9: Concurrent Percentile Queries (Scalability)
// ============================================================================

fn bench_concurrent_percentile_queries(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;

    let mut group = c.benchmark_group("concurrent_percentile_queries");

    let histogram = Arc::new(create_histogram_with_samples(10_000));

    for num_threads in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::new("scalar", num_threads),
            &num_threads,
            |b, &threads| {
                b.iter(|| {
                    let mut handles = vec![];
                    for _ in 0..threads {
                        let hist = Arc::clone(&histogram);
                        handles.push(thread::spawn(move || {
                            for _ in 0..100 {
                                black_box(hist.percentile_scalar(99.0));
                            }
                        }));
                    }
                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );

        #[cfg(feature = "portable_simd")]
        group.bench_with_input(
            BenchmarkId::new("simd", num_threads),
            &num_threads,
            |b, &threads| {
                b.iter(|| {
                    let mut handles = vec![];
                    for _ in 0..threads {
                        let hist = Arc::clone(&histogram);
                        handles.push(thread::spawn(move || {
                            for _ in 0..100 {
                                black_box(hist.percentile_simd(99.0));
                            }
                        }));
                    }
                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 10: Extreme Percentiles (p0, p100)
// ============================================================================

fn bench_extreme_percentiles(c: &mut Criterion) {
    let histogram = create_histogram_with_samples(10_000);
    let mut group = c.benchmark_group("extreme_percentiles");

    // p0 (minimum)
    group.bench_function("scalar_p0", |b| {
        b.iter(|| {
            black_box(histogram.percentile_scalar(0.0));
        });
    });

    #[cfg(feature = "portable_simd")]
    group.bench_function("simd_p0", |b| {
        b.iter(|| {
            black_box(histogram.percentile_simd(0.0));
        });
    });

    // p100 (maximum)
    group.bench_function("scalar_p100", |b| {
        b.iter(|| {
            black_box(histogram.percentile_scalar(100.0));
        });
    });

    #[cfg(feature = "portable_simd")]
    group.bench_function("simd_p100", |b| {
        b.iter(|| {
            black_box(histogram.percentile_simd(100.0));
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    simd_percentile_benches,
    // Baseline
    bench_percentile_scalar,
    bench_percentile_scalar_multiple,
    bench_stats_original,

    // SIMD (nightly-only)
    #[cfg(feature = "portable_simd")]
    bench_percentile_simd,
    #[cfg(feature = "portable_simd")]
    bench_percentile_simd_multiple,
    #[cfg(feature = "portable_simd")]
    bench_batch_percentiles,
    #[cfg(feature = "portable_simd")]
    bench_batch_percentiles_comprehensive,
    #[cfg(feature = "portable_simd")]
    bench_stats_simd,
    #[cfg(feature = "portable_simd")]
    bench_simd_overhead_small_dataset,

    // Transparent API
    bench_percentile_optimized,

    // Comparative
    bench_percentile_by_dataset_size,
    bench_percentiles_by_value,
    bench_concurrent_percentile_queries,
    bench_extreme_percentiles,
);

criterion_main!(simd_percentile_benches);
