//! B32 Benchmark: StreamingStatsCapsule vs HistogramCapsule
//!
//! # Comparison
//! - **Baseline**: HistogramCapsule (exact percentiles, O(N) memory)
//! - **Optimized**: StreamingStatsCapsule (±1% error, O(1) memory)
//!
//! # Performance Targets
//! - insert(): <50ns (vs <10ns baseline = 5× slower acceptable)
//! - query_percentile(): <100ns (vs <5ns cached = 20× slower acceptable)
//! - Memory: 512B vs 8KB = 16× reduction
//!
//! # Trade-offs
//! - Memory: 16× reduction (512B vs 8KB)
//! - Accuracy: ±1% error (vs ±0% exact)
//! - Insert latency: ~5× slower (acceptable for O(1) memory)
//!
//! # Hardware Reality (K1-K70)
//! - K20: Debug mode 10-50× slower than release
//! - K45: Multi-core contention reduces throughput
//! - K55: Cache effects dominate <100ns operations

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;
use std::thread;

#[cfg(feature = "streaming-stats")]
use atomic_capsule::collections::StreamingStatsCapsule;

#[cfg(feature = "histogram")]
use atomic_capsule::collections::HistogramCapsule;

// ============================================================================
// Baseline: HistogramCapsule (Exact percentiles, O(N) memory)
// ============================================================================

#[cfg(feature = "histogram")]
fn bench_histogram_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("histogram_insert");
    group.throughput(Throughput::Elements(1));

    group.bench_function("baseline_histogram_insert", |b| {
        let histogram = HistogramCapsule::new();
        let mut value = 1_000_000u64;
        b.iter(|| {
            histogram.record(black_box(value));
            value = value.wrapping_add(1000);
        });
    });

    group.finish();
}

#[cfg(feature = "histogram")]
fn bench_histogram_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("histogram_query");
    group.throughput(Throughput::Elements(1));

    // Populate histogram with 1000 values
    let histogram = HistogramCapsule::new();
    for i in 0..1000 {
        histogram.record(i * 1000);
    }

    group.bench_function("baseline_histogram_p50", |b| {
        b.iter(|| {
            black_box(histogram.p50());
        });
    });

    group.bench_function("baseline_histogram_p99", |b| {
        b.iter(|| {
            black_box(histogram.p99());
        });
    });

    group.finish();
}

// ============================================================================
// Optimized: StreamingStatsCapsule (±1% error, O(1) memory)
// ============================================================================

#[cfg(feature = "streaming-stats")]
fn bench_streaming_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("streaming_insert");
    group.throughput(Throughput::Elements(1));

    group.bench_function("optimized_streaming_insert", |b| {
        let stats = StreamingStatsCapsule::new();
        let mut value = 1_000_000u64;
        b.iter(|| {
            stats.insert(black_box(value));
            value = value.wrapping_add(1000);
        });
    });

    group.finish();
}

#[cfg(feature = "streaming-stats")]
fn bench_streaming_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("streaming_query");
    group.throughput(Throughput::Elements(1));

    // Populate stats with 1000 values
    let stats = StreamingStatsCapsule::new();
    for i in 0..1000 {
        stats.insert(i * 1000);
    }

    group.bench_function("optimized_streaming_p50", |b| {
        b.iter(|| {
            black_box(stats.p50());
        });
    });

    group.bench_function("optimized_streaming_p99", |b| {
        b.iter(|| {
            black_box(stats.p99());
        });
    });

    group.finish();
}

// ============================================================================
// Comparison: Insert throughput vs data size
// ============================================================================

#[cfg(all(feature = "streaming-stats", feature = "histogram"))]
fn bench_insert_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_scaling");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        group.throughput(Throughput::Elements(*size));

        group.bench_with_input(
            BenchmarkId::new("baseline_histogram", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let histogram = HistogramCapsule::new();
                    for i in 0..size {
                        histogram.record(i * 1000);
                    }
                    black_box(histogram.total_count());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("optimized_streaming", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let stats = StreamingStatsCapsule::new();
                    for i in 0..size {
                        stats.insert(i * 1000);
                    }
                    black_box(stats.total_count());
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Comparison: Multi-threaded concurrent inserts
// ============================================================================

#[cfg(all(feature = "streaming-stats", feature = "histogram"))]
fn bench_concurrent_inserts(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_inserts");
    let thread_counts = [1, 2, 4, 8];

    for &threads in thread_counts.iter() {
        let per_thread = 10_000 / threads;
        group.throughput(Throughput::Elements(10_000));

        group.bench_with_input(
            BenchmarkId::new("baseline_histogram", threads),
            &threads,
            |b, &threads| {
                b.iter(|| {
                    let histogram = Arc::new(HistogramCapsule::new());
                    let handles: Vec<_> = (0..threads)
                        .map(|tid| {
                            let h = Arc::clone(&histogram);
                            thread::spawn(move || {
                                for i in 0..per_thread {
                                    h.record((tid * per_thread + i) * 1000);
                                }
                            })
                        })
                        .collect();
                    for h in handles {
                        h.join().unwrap();
                    }
                    black_box(histogram.total_count());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("optimized_streaming", threads),
            &threads,
            |b, &threads| {
                b.iter(|| {
                    let stats = Arc::new(StreamingStatsCapsule::new());
                    let handles: Vec<_> = (0..threads)
                        .map(|tid| {
                            let s = Arc::clone(&stats);
                            thread::spawn(move || {
                                for i in 0..per_thread {
                                    s.insert((tid * per_thread + i) * 1000);
                                }
                            })
                        })
                        .collect();
                    for h in handles {
                        h.join().unwrap();
                    }
                    black_box(stats.total_count());
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Memory Footprint Comparison
// ============================================================================

#[cfg(all(feature = "streaming-stats", feature = "histogram"))]
fn bench_memory_footprint(c: &mut Criterion) {
    use std::mem::size_of;

    let mut group = c.benchmark_group("memory_footprint");

    group.bench_function("baseline_histogram_size", |b| {
        b.iter(|| {
            black_box(size_of::<HistogramCapsule>());
        });
    });

    group.bench_function("optimized_streaming_size", |b| {
        b.iter(|| {
            black_box(size_of::<StreamingStatsCapsule>());
        });
    });

    println!("\n=== Memory Footprint ===");
    println!(
        "HistogramCapsule:       {} bytes",
        size_of::<HistogramCapsule>()
    );
    println!(
        "StreamingStatsCapsule:  {} bytes",
        size_of::<StreamingStatsCapsule>()
    );
    println!(
        "Reduction:              {:.1}×",
        size_of::<HistogramCapsule>() as f64 / size_of::<StreamingStatsCapsule>() as f64
    );

    group.finish();
}

// ============================================================================
// Accuracy Validation (not a benchmark, just verification)
// ============================================================================

#[cfg(all(feature = "streaming-stats", feature = "histogram"))]
fn validate_accuracy(c: &mut Criterion) {
    let mut group = c.benchmark_group("accuracy_validation");

    group.bench_function("validate_uniform_distribution", |b| {
        b.iter(|| {
            let histogram = HistogramCapsule::new();
            let stats = StreamingStatsCapsule::new();

            // Insert 1000 uniform values
            for i in 1..=1000 {
                let value = i * 1_000_000;
                histogram.record(value);
                stats.insert(value);
            }

            let hist_p50 = histogram.p50().unwrap();
            let stream_p50 = stats.p50().unwrap();

            let error = (stream_p50 as i64 - hist_p50 as i64).abs();
            let error_pct = (error as f64 / hist_p50 as f64) * 100.0;

            black_box((error_pct, hist_p50, stream_p50));
        });
    });

    group.finish();

    // Print accuracy analysis
    let histogram = HistogramCapsule::new();
    let stats = StreamingStatsCapsule::new();

    for i in 1..=1000 {
        let value = i * 1_000_000;
        histogram.record(value);
        stats.insert(value);
    }

    let hist_p50 = histogram.p50().unwrap();
    let stream_p50 = stats.p50().unwrap();
    let p50_error = ((stream_p50 as i64 - hist_p50 as i64).abs() as f64 / hist_p50 as f64) * 100.0;

    let hist_p99 = histogram.p99().unwrap();
    let stream_p99 = stats.p99().unwrap();
    let p99_error = ((stream_p99 as i64 - hist_p99 as i64).abs() as f64 / hist_p99 as f64) * 100.0;

    println!("\n=== Accuracy Analysis (1000 uniform samples) ===");
    println!("P50:");
    println!("  Histogram:       {} μs", hist_p50 / 1000);
    println!("  Streaming:       {} μs", stream_p50 / 1000);
    println!("  Error:           {:.2}% (target: <1%)", p50_error);
    println!("P99:");
    println!("  Histogram:       {} μs", hist_p99 / 1000);
    println!("  Streaming:       {} μs", stream_p99 / 1000);
    println!("  Error:           {:.2}% (target: <1%)", p99_error);
    println!("Centroids: {}", stats.centroid_count());
}

// ============================================================================
// Criterion Groups
// ============================================================================

#[cfg(feature = "histogram")]
criterion_group!(
    histogram_baseline,
    bench_histogram_insert,
    bench_histogram_query
);

#[cfg(feature = "streaming-stats")]
criterion_group!(
    streaming_optimized,
    bench_streaming_insert,
    bench_streaming_query
);

#[cfg(all(feature = "streaming-stats", feature = "histogram"))]
criterion_group!(
    comparison,
    bench_insert_scaling,
    bench_concurrent_inserts,
    bench_memory_footprint,
    validate_accuracy
);

// Conditional main based on feature flags
#[cfg(all(feature = "streaming-stats", feature = "histogram"))]
criterion_main!(histogram_baseline, streaming_optimized, comparison);

#[cfg(all(feature = "streaming-stats", not(feature = "histogram")))]
criterion_main!(streaming_optimized);

#[cfg(all(not(feature = "streaming-stats"), feature = "histogram"))]
criterion_main!(histogram_baseline);

#[cfg(not(any(feature = "streaming-stats", feature = "histogram")))]
fn main() {
    println!("Enable 'streaming-stats' and/or 'histogram' features to run benchmarks");
}
