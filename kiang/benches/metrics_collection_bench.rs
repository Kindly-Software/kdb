//! Metrics Collection Performance Benchmarks
//!
//! Benchmarks measuring overhead of performance metrics collection.
//! Follows B32 framework for honest overhead measurement.
//!
//! # Performance Targets (B32 K2, K25)
//!
//! - Counter increment: <20ns (atomic fetch_add)
//! - Metrics snapshot: <100ns (4 atomic loads)
//! - Export overhead: <5% of total runtime
//! - Concurrent updates: Contention-aware scaling
//!
//! # B32 Compliance
//!
//! - B1: Fair baseline (with vs without metrics)
//! - B2: Statistical rigor (Criterion 95% CI)
//! - K25: Integration overhead (monitoring cost)

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use kiang::GpuMetrics;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Baseline GPU work simulation (no metrics)
fn simulate_gpu_work_no_metrics() {
    // Simulate command submission
    let _ = black_box(42u64);
    let _ = black_box(4096u64);
}

/// GPU work with metrics collection
fn simulate_gpu_work_with_metrics(metrics: &GpuMetrics) {
    metrics.inc_commands(1);
    let _ = black_box(42u64);
    let _ = black_box(4096u64);
}

/// Benchmark: Counter increment overhead
///
/// # Expected Results (B32 K2)
/// - AtomicU64 fetch_add: ~20ns
/// - Relaxed ordering: <10ns (no sync overhead)
fn bench_counter_increment(c: &mut Criterion) {
    let mut group = c.benchmark_group("counter_increment");

    let metrics = GpuMetrics::new();

    // Single counter increment
    group.bench_function("single_inc", |b| {
        b.iter(|| {
            black_box(&metrics).inc_commands(black_box(1));
        });
    });

    // Batch increment (amortized cost)
    group.bench_function("batch_inc", |b| {
        b.iter(|| {
            black_box(&metrics).inc_commands(black_box(10));
        });
    });

    // Frame counter increment
    group.bench_function("inc_frames", |b| {
        b.iter(|| {
            black_box(&metrics).inc_frames();
        });
    });

    // Error counter increment
    group.bench_function("inc_errors", |b| {
        b.iter(|| {
            black_box(&metrics).inc_errors();
        });
    });

    group.finish();
}

/// Benchmark: Metrics snapshot overhead
///
/// # Expected Results
/// - Snapshot: <100ns (4 atomic loads with Relaxed)
fn bench_metrics_snapshot(c: &mut Criterion) {
    let metrics = GpuMetrics::new();

    // Populate metrics
    for i in 0..1000 {
        metrics.inc_commands(10);
        if i % 20 == 0 {
            metrics.inc_errors();
        }
        metrics.inc_frames();
    }

    c.bench_function("snapshot", |b| {
        b.iter(|| {
            let snapshot = black_box(&metrics).snapshot();
            black_box(snapshot);
        });
    });
}

/// Benchmark: Derived metrics calculation
///
/// Tests overhead of computing derived metrics (error rate, memory usage).
fn bench_derived_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("derived_metrics");

    let metrics = GpuMetrics::new();

    // Populate with realistic data
    metrics.inc_commands(10000);
    metrics.inc_errors();
    metrics.set_allocated(5_000_000_000); // 5GB

    // Error rate calculation
    group.bench_function("error_rate", |b| {
        b.iter(|| {
            let rate = black_box(&metrics).error_rate();
            black_box(rate);
        });
    });

    // Memory usage percentage
    group.bench_function("memory_usage_pct", |b| {
        b.iter(|| {
            let pct = black_box(&metrics).memory_usage_pct();
            black_box(pct);
        });
    });

    group.finish();
}

/// Benchmark: Overhead of metrics in hot path
///
/// Measures actual cost of metrics collection during GPU operations.
///
/// # B32 Validation
/// - K25: Integration overhead (target: <5% total runtime)
fn bench_hot_path_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("hot_path_overhead");

    let metrics = GpuMetrics::new();

    // Baseline: No metrics
    group.bench_function("no_metrics", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                simulate_gpu_work_no_metrics();
            }
        });
    });

    // With metrics: Measure overhead
    group.bench_function("with_metrics", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                simulate_gpu_work_with_metrics(black_box(&metrics));
            }
        });
    });

    group.finish();
}

/// Benchmark: Concurrent metrics updates
///
/// Tests contention under concurrent access.
///
/// # B32 Compliance
/// - B4: Contention scenarios (1, 2, 4, 8 threads)
/// - K12: Lockfree scaling (<12 threads optimal)
fn bench_concurrent_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_updates");

    for num_threads in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            num_threads,
            |b, &num_threads| {
                let metrics = Arc::new(GpuMetrics::new());

                b.iter(|| {
                    let mut handles = vec![];

                    for _ in 0..num_threads {
                        let metrics_clone = Arc::clone(&metrics);
                        handles.push(std::thread::spawn(move || {
                            for _ in 0..100 {
                                metrics_clone.inc_commands(1);
                                metrics_clone.inc_frames();
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

/// Benchmark: Export format generation
///
/// Tests overhead of exporting metrics to monitoring systems.
fn bench_export_formats(c: &mut Criterion) {
    let mut group = c.benchmark_group("export_formats");

    let metrics = GpuMetrics::new();

    // Populate with realistic data
    for i in 0..1000 {
        metrics.inc_commands(10);
        if i % 20 == 0 {
            metrics.inc_errors();
        }
        metrics.inc_frames();
    }
    metrics.set_allocated(4_000_000_000);

    // JSON export (expensive)
    group.bench_function("json_export", |b| {
        b.iter(|| {
            let snapshot = metrics.snapshot();
            let json = format!(
                r#"{{"frames":{}, "commands":{}, "errors":{}, "allocated":{}}}"#,
                snapshot.frames_rendered,
                snapshot.commands_submitted,
                snapshot.errors,
                snapshot.bytes_allocated
            );
            black_box(json);
        });
    });

    // Prometheus format (text-based)
    group.bench_function("prometheus_export", |b| {
        b.iter(|| {
            let snapshot = metrics.snapshot();
            let prom = format!(
                "gpu_frames_rendered {}\ngpu_commands_submitted {}\ngpu_errors_total {}\ngpu_allocated_bytes {}\n",
                snapshot.frames_rendered,
                snapshot.commands_submitted,
                snapshot.errors,
                snapshot.bytes_allocated
            );
            black_box(prom);
        });
    });

    // Binary export (most efficient)
    group.bench_function("binary_export", |b| {
        b.iter(|| {
            let snapshot = metrics.snapshot();
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    &snapshot as *const _ as *const u8,
                    std::mem::size_of_val(&snapshot),
                )
            };
            black_box(bytes);
        });
    });

    group.finish();
}

/// Benchmark: Periodic metrics collection
///
/// Simulates background metrics collection task.
///
/// # Expected Results
/// - Collection frequency: 10Hz (100ms interval)
/// - CPU overhead: <1% single core
fn bench_periodic_collection(c: &mut Criterion) {
    let metrics = Arc::new(GpuMetrics::new());

    c.bench_function("periodic_collection", |b| {
        let metrics_clone = Arc::clone(&metrics);

        // Simulate 1 second of collection
        b.iter(|| {
            for _ in 0..10 {
                // 10Hz collection
                let _snapshot = metrics_clone.snapshot();
                std::thread::sleep(Duration::from_millis(10));
            }
        });
    });
}

/// Benchmark: Metrics reset overhead
///
/// Tests cost of resetting counters (for per-frame metrics).
fn bench_metrics_reset(c: &mut Criterion) {
    let metrics = GpuMetrics::new();

    // Populate metrics
    for _ in 0..1000 {
        metrics.inc_commands(1);
        metrics.inc_frames();
    }

    c.bench_function("reset", |b| {
        b.iter(|| {
            // Reset by creating new metrics
            let new_metrics = GpuMetrics::new();
            black_box(new_metrics);
        });
    });
}

/// Benchmark: Contention hotspot detection
///
/// Identifies which metric updates cause most contention.
fn bench_contention_hotspots(c: &mut Criterion) {
    let mut group = c.benchmark_group("contention_hotspots");

    let metrics = Arc::new(GpuMetrics::new());

    // Test commands counter under contention
    group.bench_function("commands", |b| {
        let metrics_clone = Arc::clone(&metrics);

        b.iter(|| {
            let mut handles = vec![];

            for _ in 0..4 {
                let m = Arc::clone(&metrics_clone);
                handles.push(std::thread::spawn(move || {
                    for _ in 0..100 {
                        m.inc_commands(1);
                    }
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    // Test frames counter under contention
    group.bench_function("frames", |b| {
        let metrics_clone = Arc::clone(&metrics);

        b.iter(|| {
            let mut handles = vec![];

            for _ in 0..4 {
                let m = Arc::clone(&metrics_clone);
                handles.push(std::thread::spawn(move || {
                    for _ in 0..100 {
                        m.inc_frames();
                    }
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    // Test errors counter under contention
    group.bench_function("errors", |b| {
        let metrics_clone = Arc::clone(&metrics);

        b.iter(|| {
            let mut handles = vec![];

            for _ in 0..4 {
                let m = Arc::clone(&metrics_clone);
                handles.push(std::thread::spawn(move || {
                    for _ in 0..100 {
                        m.inc_errors();
                    }
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    group.finish();
}

/// Benchmark: Memory usage tracking accuracy
///
/// Tests overhead and accuracy of memory usage tracking.
fn bench_memory_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_tracking");

    let metrics = GpuMetrics::new();

    // Simulate memory allocations
    let sizes = [4096, 65536, 1_048_576, 16_777_216]; // 4KB to 16MB

    for size in sizes.iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let current = metrics.snapshot().bytes_allocated;
                black_box(&metrics).set_allocated(current + size);
            });
        });
    }

    group.finish();
}

/// Benchmark: Metrics aggregation
///
/// Tests cost of aggregating metrics from multiple sources.
fn bench_metrics_aggregation(c: &mut Criterion) {
    // Create multiple metric sources (per-GPU or per-context)
    let sources: Vec<GpuMetrics> = (0..4)
        .map(|_| {
            let m = GpuMetrics::new();
            for _ in 0..100 {
                m.inc_commands(10);
                m.inc_frames();
            }
            m
        })
        .collect();

    c.bench_function("aggregate_metrics", |b| {
        b.iter(|| {
            let mut total_commands = 0u64;
            let mut total_frames = 0u64;
            let mut total_errors = 0u64;

            for source in &sources {
                let snapshot = source.snapshot();
                total_commands += snapshot.commands_submitted;
                total_frames += snapshot.frames_rendered;
                total_errors += snapshot.errors;
            }

            black_box((total_commands, total_frames, total_errors));
        });
    });
}

criterion_group!(
    benches,
    bench_counter_increment,
    bench_metrics_snapshot,
    bench_derived_metrics,
    bench_hot_path_overhead,
    bench_concurrent_updates,
    bench_export_formats,
    bench_periodic_collection,
    bench_metrics_reset,
    bench_contention_hotspots,
    bench_memory_tracking,
    bench_metrics_aggregation,
);

criterion_main!(benches);
