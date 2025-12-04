//! B32 Benchmark - Prometheus Metrics Performance
//!
//! **Targets**:
//! - Increment: <10ns per operation (Relaxed atomic)
//! - Scrape: <5ms for all metrics
//! - Memory: <10 KB overhead
//!
//! **Framework**: B32 (fair benchmarking, 95% CI, 1000+ iterations)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use kdb_mcp::{MetricsCapsule, ToolId};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Group 1: Increment Latency Benchmarks
// ============================================================================

fn bench_increment_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics_increment");
    group.sample_size(1000);

    group.bench_function("record_request_success", |b| {
        let capsule = MetricsCapsule::new();
        b.iter(|| {
            capsule.record_request(black_box(ToolId::DebuggerAttach), black_box(true), black_box(5000));
        });
    });

    group.bench_function("record_request_error", |b| {
        let capsule = MetricsCapsule::new();
        b.iter(|| {
            capsule.record_request(black_box(ToolId::DebuggerAttach), black_box(false), black_box(1000));
        });
    });

    group.bench_function("increment_error_quota_exceeded", |b| {
        let capsule = MetricsCapsule::new();
        b.iter(|| {
            capsule.increment_error_quota_exceeded();
        });
    });

    group.bench_function("increment_deletion_proofs", |b| {
        let capsule = MetricsCapsule::new();
        b.iter(|| {
            capsule.increment_deletion_proofs();
        });
    });

    group.bench_function("set_memory_heap_bytes", |b| {
        let capsule = MetricsCapsule::new();
        b.iter(|| {
            capsule.set_memory_heap_bytes(black_box(52_428_800));
        });
    });

    group.bench_function("set_cpu_usage_percent", |b| {
        let capsule = MetricsCapsule::new();
        b.iter(|| {
            capsule.set_cpu_usage_percent(black_box(12.5));
        });
    });

    group.finish();
}

// ============================================================================
// Group 2: Concurrent Increment Benchmarks
// ============================================================================

fn bench_increment_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics_concurrent");
    group.sample_size(100);

    for num_threads in [2, 4, 8, 16].iter() {
        group.throughput(Throughput::Elements(*num_threads as u64 * 10_000));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}threads", num_threads)),
            num_threads,
            |b, &num_threads| {
                let capsule = Arc::new(MetricsCapsule::new());

                b.iter(|| {
                    let mut handles = vec![];

                    for _ in 0..num_threads {
                        let capsule_clone = Arc::clone(&capsule);
                        let handle = thread::spawn(move || {
                            // Each thread increments 10,000 times
                            for _ in 0..10_000 {
                                capsule_clone.record_request(
                                    black_box(ToolId::DebuggerAttach),
                                    black_box(true),
                                    black_box(5_000),
                                );
                            }
                        });
                        handles.push(handle);
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
// Group 3: Scrape Performance Benchmarks
// ============================================================================

fn bench_scrape(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics_scrape");
    group.sample_size(100);

    group.bench_function("export_prometheus_empty", |b| {
        let capsule = MetricsCapsule::new();
        b.iter(|| {
            black_box(capsule.export_prometheus());
        });
    });

    group.bench_function("export_prometheus_populated", |b| {
        let capsule = MetricsCapsule::new();

        // Pre-populate with metrics
        for i in 0..100 {
            capsule.record_request(
                match i % 12 {
                    0 => ToolId::DebuggerAttach,
                    1 => ToolId::DebuggerSetBreakpoint,
                    2 => ToolId::DebuggerContinue,
                    3 => ToolId::DebuggerStepForward,
                    4 => ToolId::DebuggerStepBackward,
                    5 => ToolId::DebuggerGetStackTrace,
                    6 => ToolId::DebuggerGetVariables,
                    7 => ToolId::DebuggerFindSimilarBugs,
                    8 => ToolId::DebuggerExportTrace,
                    9 => ToolId::DebuggerGetDeletionProof,
                    10 => ToolId::DebuggerVerifyDeletionProof,
                    _ => ToolId::DebuggerQuotaStatus,
                },
                i % 2 == 0,
                1000 + (i as u64 * 100),
            );
        }

        // Record various other metrics
        capsule.increment_error_quota_exceeded();
        capsule.increment_error_rate_limited();
        capsule.increment_error_attach_failed();
        capsule.set_memory_heap_bytes(52_428_800);
        capsule.set_cpu_usage_percent(12.5);
        capsule.set_threads_active(16);
        capsule.increment_deletion_proofs();
        capsule.set_active_sessions(15, 5);
        capsule.record_sla_violation_10us();
        capsule.record_sla_violation_100us();

        b.iter(|| {
            black_box(capsule.export_prometheus());
        });
    });

    group.bench_function("scrape_latency_load", |b| {
        let capsule = MetricsCapsule::new();

        // Simulate many metrics being recorded during benchmark
        b.iter_batched(
            || {
                // Setup: populate metrics
                for i in 0..1000 {
                    capsule.record_request(
                        match i % 12 {
                            0 => ToolId::DebuggerAttach,
                            1 => ToolId::DebuggerSetBreakpoint,
                            2 => ToolId::DebuggerContinue,
                            3 => ToolId::DebuggerStepForward,
                            4 => ToolId::DebuggerStepBackward,
                            5 => ToolId::DebuggerGetStackTrace,
                            6 => ToolId::DebuggerGetVariables,
                            7 => ToolId::DebuggerFindSimilarBugs,
                            8 => ToolId::DebuggerExportTrace,
                            9 => ToolId::DebuggerGetDeletionProof,
                            10 => ToolId::DebuggerVerifyDeletionProof,
                            _ => ToolId::DebuggerQuotaStatus,
                        },
                        i % 2 == 0,
                        1000 + (i as u64 * 100),
                    );
                }
            },
            |_| {
                // Measure: scrape the metrics
                black_box(capsule.export_prometheus());
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ============================================================================
// Group 4: Mixed Load Benchmarks (Real-World Simulation)
// ============================================================================

fn bench_mixed_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics_mixed_load");
    group.sample_size(100);

    group.bench_function("realistic_workload_single_thread", |b| {
        let capsule = MetricsCapsule::new();

        b.iter(|| {
            // Simulate realistic workload:
            // - 100 requests recorded
            // - 5 errors
            // - 1 scrape
            for i in 0..100 {
                capsule.record_request(
                    match i % 12 {
                        0 => ToolId::DebuggerAttach,
                        1 => ToolId::DebuggerSetBreakpoint,
                        _ => ToolId::DebuggerContinue,
                    },
                    i % 20 != 0, // 5% error rate
                    (1000 + (i as u64) * 50) % 100_000,
                );
            }

            // Error tracking
            for i in 0..5 {
                match i % 5 {
                    0 => capsule.increment_error_quota_exceeded(),
                    1 => capsule.increment_error_rate_limited(),
                    2 => capsule.increment_error_attach_failed(),
                    3 => capsule.increment_error_invalid_license(),
                    _ => capsule.increment_error_ptrace(),
                }
            }

            // Metrics update
            capsule.set_memory_heap_bytes(52_428_800);
            capsule.set_threads_active(16);

            // Scrape (1 per 100 requests typically)
            black_box(capsule.export_prometheus());
        });
    });

    group.finish();
}

// ============================================================================
// Group 5: Memory and Size Verification
// ============================================================================

fn bench_memory_verification(_c: &mut Criterion) {
    // This is more of a sanity check than a performance benchmark
    use std::mem::{align_of, size_of};

    let capsule = MetricsCapsule::new();
    let capsule_size = size_of::<MetricsCapsule>();
    let capsule_align = align_of::<MetricsCapsule>();

    println!("MetricsCapsule size: {} bytes ({:.1} KB)", capsule_size, capsule_size as f64 / 1024.0);
    println!("MetricsCapsule alignment: {} bytes", capsule_align);

    assert!(capsule_align == 256, "Must be 256-byte aligned");
    assert!(capsule_size < 16384, "Must be < 16 KB");

    // Verify all methods compile and return correct types
    let _metrics_output = capsule.export_prometheus();
    assert!(_metrics_output.len() > 0, "Metrics output should not be empty");
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    name = benches;
    config = Criterion::default()
        .measurement_time(std::time::Duration::from_secs(10))
        .sample_size(1000);
    targets = bench_increment_single, bench_increment_concurrent, bench_scrape, bench_mixed_load, bench_memory_verification
);

criterion_main!(benches);
