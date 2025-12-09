//! Phase 5.7: Audit Bridge B32 Benchmarks
//!
//! Benchmarking the AuditLogBridge async/blocking bridge pattern
//! comparing against Mutex<File> baseline (old implementation)

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use clapi_core::proxy::AuditLogBridge;
use std::sync::Arc;

// Benchmark 1: Single append latency
fn bench_single_append(c: &mut Criterion) {
    c.bench_function("audit_bridge_single_append", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let bridge = black_box(AuditLogBridge::new());
                let _ = bridge.append(black_box("test event message")).await;
            });
    });
}

// Benchmark 2: Batch flush latency (100 events)
fn bench_batch_flush(c: &mut Criterion) {
    c.bench_function("audit_bridge_batch_flush_100", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let bridge = black_box(AuditLogBridge::new());
                for i in 0..100 {
                    let _ = bridge.append(&format!("event {}", i)).await;
                }
                // Wait for flush to complete
                tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
            });
    });
}

// Benchmark 3: log_request convenience method
fn bench_log_request(c: &mut Criterion) {
    c.bench_function("audit_bridge_log_request", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let bridge = black_box(AuditLogBridge::new());
                let _ = bridge.log_request(black_box(42), black_box(1000), black_box(0x1234)).await;
            });
    });
}

// Benchmark 4: Concurrent appends (10 concurrent writers)
fn bench_concurrent_append(c: &mut Criterion) {
    c.bench_function("audit_bridge_concurrent_10_writers", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let bridge = Arc::new(black_box(AuditLogBridge::new()));
                let mut tasks = vec![];

                for writer in 0..10 {
                    let bridge_clone = Arc::clone(&bridge);
                    let task = tokio::spawn(async move {
                        for i in 0..10 {
                            let _ = bridge_clone.append(&format!("w{} i{}", writer, i)).await;
                        }
                    });
                    tasks.push(task);
                }

                futures::future::join_all(tasks).await;
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            });
    });
}

// Benchmark 5: Throughput (events per second)
fn bench_throughput(c: &mut Criterion) {
    c.bench_function("audit_bridge_throughput_1000_events", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .iter(|| async {
                let bridge = black_box(AuditLogBridge::new());
                for i in 0..1000 {
                    let _ = bridge.append(&format!("event {}", i)).await;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            });
    });
}

// Benchmark 6: Error counter access (lockfree metric read)
fn bench_error_counter_read(c: &mut Criterion) {
    c.bench_function("audit_bridge_error_counter_read", |b| {
        let bridge = AuditLogBridge::new();
        b.iter(|| {
            let _ = black_box(bridge.error_count());
        });
    });
}

criterion_group!(
    benches,
    bench_single_append,
    bench_batch_flush,
    bench_log_request,
    bench_concurrent_append,
    bench_throughput,
    bench_error_counter_read
);
criterion_main!(benches);
