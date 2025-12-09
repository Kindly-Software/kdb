//! MetricsStreamCapsule Benchmark - B32 Framework Validation
//!
//! **Purpose**: Validate Tier 5 streaming capsule performance characteristics
//! **Framework**: B32 (honest benchmarking with statistical rigor)
//!
//! # Performance Targets
//! - record_metric(): <10ns (single atomic increment + store)
//! - snapshot(): <50ns (capture head/tail, return slice)
//! - get_p50/p90/p95/p99/p999(): <500ns (in-place sort of 64 values)
//!
//! # Benchmarks
//! 1. record_metric() - Single-threaded append performance
//! 2. snapshot() - Ring buffer capture performance
//! 3. percentile_calculations() - Statistical query performance
//! 4. concurrent_writes() - Multi-threaded contention testing
//! 5. full_workflow() - End-to-end metrics collection pipeline

use clapi_core::capsules::MetricsStreamCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::Arc;
use std::thread;

/// Benchmark: record_metric() - Single atomic increment + store
///
/// **Target**: <10ns per operation
/// **Baseline**: Vec::push (requires mutex) = ~30-50ns
/// **Expected Speedup**: 3-5× vs mutex-based approach
fn bench_record_metric(c: &mut Criterion) {
    let capsule = MetricsStreamCapsule::new();

    c.bench_function("record_metric", |b| {
        b.iter(|| {
            capsule.record_metric(black_box(1_000_000));
        });
    });
}

/// Benchmark: snapshot() - Capture ring buffer state
///
/// **Target**: <50ns for pointer capture
/// **Baseline**: Vec::clone() = ~100-200ns
/// **Expected Speedup**: 2-4× vs clone-based approach
fn bench_snapshot(c: &mut Criterion) {
    let capsule = MetricsStreamCapsule::new();

    // Pre-fill buffer with 64 metrics
    for i in 0..64 {
        capsule.record_metric(i * 1000);
    }

    c.bench_function("snapshot", |b| {
        b.iter(|| {
            black_box(capsule.snapshot());
        });
    });
}

/// Benchmark: percentile_calculations() - Statistical queries
///
/// **Target**: <500ns for sort + percentile extraction
/// **Baseline**: Full histogram = ~1-2μs
/// **Expected Speedup**: 2-4× vs histogram-based approach
fn bench_percentile_calculations(c: &mut Criterion) {
    let capsule = MetricsStreamCapsule::new();

    // Fill buffer with 64 random-ish metrics
    for i in 0..64 {
        capsule.record_metric((i * 17) % 1000 * 1000);
    }

    let mut group = c.benchmark_group("percentile_calculations");

    group.bench_function("p50", |b| {
        b.iter(|| black_box(capsule.get_p50()));
    });

    group.bench_function("p90", |b| {
        b.iter(|| black_box(capsule.get_p90()));
    });

    group.bench_function("p99", |b| {
        b.iter(|| black_box(capsule.get_p99()));
    });

    group.bench_function("p999", |b| {
        b.iter(|| black_box(capsule.get_p999()));
    });

    group.bench_function("get_statistics", |b| {
        b.iter(|| black_box(capsule.get_statistics()));
    });

    group.finish();
}

/// Benchmark: concurrent_writes() - Multi-threaded contention
///
/// **Target**: Linear scaling up to 8 threads
/// **Baseline**: Mutex-protected Vec = sublinear scaling
/// **Expected Speedup**: 8× throughput at 8 threads (vs mutex bottleneck)
fn bench_concurrent_writes(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_writes");

    for thread_count in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let capsule = Arc::new(MetricsStreamCapsule::new());
                    let mut handles = vec![];

                    // Each thread records 100 metrics
                    for _ in 0..thread_count {
                        let c = Arc::clone(&capsule);
                        handles.push(thread::spawn(move || {
                            for i in 0..100 {
                                c.record_metric(i);
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }

                    black_box(capsule.size());
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: full_workflow() - End-to-end metrics pipeline
///
/// **Target**: <20ns per metric (record + amortized snapshot)
/// **Baseline**: Mutex Vec + clone + sort = ~100ns per metric
/// **Expected Speedup**: 5× end-to-end
fn bench_full_workflow(c: &mut Criterion) {
    c.bench_function("full_workflow", |b| {
        b.iter(|| {
            let capsule = MetricsStreamCapsule::new();

            // Record 100 latency measurements
            for i in 0..100 {
                capsule.record_metric(i * 10_000); // 0-1ms latencies
            }

            // Snapshot and calculate p99
            let _ = black_box(capsule.get_p99());

            // Export to Prometheus (simulation)
            let stats = black_box(capsule.get_statistics());
            let _ = black_box(stats.count);
        });
    });
}

criterion_group!(
    benches,
    bench_record_metric,
    bench_snapshot,
    bench_percentile_calculations,
    bench_concurrent_writes,
    bench_full_workflow,
);
criterion_main!(benches);
