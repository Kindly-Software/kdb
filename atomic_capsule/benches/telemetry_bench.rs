// Criterion Benchmarks for Telemetry Aggregation
//
// Performance Targets:
// - record_request: <50ns
// - get_snapshot: <100ns
// - export_metrics: <1ms for 100 metrics
//
// Framework: B32 (fair baselines, 95% CI, 1000+ iterations)

#![cfg(feature = "telemetry-prometheus")]

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use atomic_capsule::meta::{
    TelemetryAggregatorCapsule,
    PrometheusExporterCapsule,
    ProtocolType,
};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Benchmark 1: record_request performance (<50ns target)
// ============================================================================

fn bench_record_request(c: &mut Criterion) {
    let mut group = c.benchmark_group("record_request");
    group.throughput(Throughput::Elements(1));

    let telemetry = TelemetryAggregatorCapsule::new();

    group.bench_function("single_thread", |b| {
        b.iter(|| {
            telemetry.record_request(ProtocolType::REST, 500_000, 1024);
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark 2: Concurrent record_request
// ============================================================================

fn bench_concurrent_record_request(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_record_request");

    for num_threads in [1, 2, 4, 8, 16] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            &num_threads,
            |b, &num_threads| {
                let telemetry = Arc::new(TelemetryAggregatorCapsule::new());

                b.iter(|| {
                    let mut handles = vec![];
                    for _ in 0..num_threads {
                        let t = Arc::clone(&telemetry);
                        let handle = thread::spawn(move || {
                            for _ in 0..100 {
                                t.record_request(ProtocolType::REST, 500_000, 1024);
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
// Benchmark 3: get_snapshot performance (<100ns target)
// ============================================================================

fn bench_get_snapshot(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_snapshot");
    group.throughput(Throughput::Elements(1));

    let telemetry = TelemetryAggregatorCapsule::new();

    // Pre-populate with data
    for _ in 0..1000 {
        telemetry.record_request(ProtocolType::REST, 500_000, 1024);
    }

    group.bench_function("populated", |b| {
        b.iter(|| {
            let _ = telemetry.get_snapshot();
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark 4: Histogram bucket assignment
// ============================================================================

fn bench_histogram_bucket_assignment(c: &mut Criterion) {
    let mut group = c.benchmark_group("histogram_bucket_assignment");
    group.throughput(Throughput::Elements(1));

    let telemetry = TelemetryAggregatorCapsule::new();

    let latencies = [
        500,          // <1μs
        5_000,        // <10μs
        50_000,       // <100μs
        500_000,      // <1ms
        5_000_000,    // <10ms
        50_000_000,   // <100ms
        500_000_000,  // <1s
        5_000_000_000, // >1s
    ];

    for (idx, &latency) in latencies.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("latency", idx),
            &latency,
            |b, &latency| {
                b.iter(|| {
                    telemetry.record_request(ProtocolType::REST, latency, 1024);
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark 5: Active requests gauge
// ============================================================================

fn bench_active_requests_gauge(c: &mut Criterion) {
    let mut group = c.benchmark_group("active_requests_gauge");
    group.throughput(Throughput::Elements(1));

    let telemetry = TelemetryAggregatorCapsule::new();

    group.bench_function("increment", |b| {
        b.iter(|| {
            telemetry.increment_active();
        });
    });

    group.bench_function("decrement", |b| {
        b.iter(|| {
            telemetry.decrement_active();
        });
    });

    group.bench_function("increment_decrement", |b| {
        b.iter(|| {
            telemetry.increment_active();
            telemetry.decrement_active();
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark 6: Cache hit rate update
// ============================================================================

fn bench_cache_hit_rate_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_hit_rate_update");
    group.throughput(Throughput::Elements(1));

    let telemetry = TelemetryAggregatorCapsule::new();

    group.bench_function("update", |b| {
        b.iter(|| {
            telemetry.update_cache_hit_rate(0.75);
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark 7: Prometheus export_metrics (<1ms target)
// ============================================================================

fn bench_prometheus_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("prometheus_export");
    group.throughput(Throughput::Elements(1));

    let telemetry = TelemetryAggregatorCapsule::new();

    // Pre-populate with data
    for i in 0..1000 {
        let protocol = match i % 6 {
            0 => ProtocolType::REST,
            1 => ProtocolType::GraphQL,
            2 => ProtocolType::Grpc,
            3 => ProtocolType::WebSocket,
            4 => ProtocolType::JsonRPC,
            _ => ProtocolType::SSE,
        };
        telemetry.record_request(protocol, (i % 8) * 100_000, 1024);
    }

    let snapshot = telemetry.get_snapshot();

    group.bench_function("export_metrics", |b| {
        b.iter(|| {
            let _ = PrometheusExporterCapsule::export_metrics(&snapshot);
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark 8: Full request lifecycle (<100ns target)
// ============================================================================

fn bench_full_request_lifecycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_request_lifecycle");
    group.throughput(Throughput::Elements(1));

    let telemetry = TelemetryAggregatorCapsule::new();

    group.bench_function("lifecycle", |b| {
        b.iter(|| {
            telemetry.increment_active();
            telemetry.record_request(ProtocolType::REST, 500_000, 1024);
            telemetry.decrement_active();
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark 9: Mixed operations (realistic workload)
// ============================================================================

fn bench_mixed_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_operations");
    group.throughput(Throughput::Elements(100));

    let telemetry = TelemetryAggregatorCapsule::new();

    group.bench_function("realistic_workload", |b| {
        b.iter(|| {
            // 100 operations: 80% requests, 10% errors, 5% timeouts, 5% snapshots
            for i in 0..100 {
                if i < 80 {
                    telemetry.record_request(ProtocolType::REST, 500_000, 1024);
                } else if i < 90 {
                    telemetry.record_error(ProtocolType::REST);
                } else if i < 95 {
                    telemetry.record_timeout(ProtocolType::REST);
                } else {
                    let _ = telemetry.get_snapshot();
                }
            }
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark 10: Scalability under load
// ============================================================================

fn bench_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalability");

    for num_requests in [100, 1_000, 10_000, 100_000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_requests),
            &num_requests,
            |b, &num_requests| {
                let telemetry = TelemetryAggregatorCapsule::new();

                b.iter(|| {
                    for i in 0..num_requests {
                        telemetry.record_request(ProtocolType::REST, (i % 10) * 100_000, 1024);
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    bench_record_request,
    bench_concurrent_record_request,
    bench_get_snapshot,
    bench_histogram_bucket_assignment,
    bench_active_requests_gauge,
    bench_cache_hit_rate_update,
    bench_prometheus_export,
    bench_full_request_lifecycle,
    bench_mixed_operations,
    bench_scalability,
);

criterion_main!(benches);
