//! Async Flush Pipeline Benchmarks (B32 Framework)
//!
//! Performance targets:
//! - Append: <78ns (unchanged)
//! - Schedule flush: <200ns (RingBufferBroadcast send)
//! - P99.9 latency: <100ns (vs 1-10μs sync flush) = 10-128× improvement

use clapi_core::capsules::async_flush_capsule::{AsyncFlushPipeline, FlushTask};
use clapi_core::capsules::timeline_aggregation_capsule::TimelineAggregationCapsuleWrapper;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

// ============================================================================
// Baseline: Sync flush (for comparison)
// ============================================================================

fn bench_sync_flush_compute_hash(c: &mut Criterion) {
    let task = FlushTask::new(0, 1000, 1060, 42, 0);

    c.bench_function("sync_flush/compute_hash", |b| {
        b.iter(|| {
            let hash = black_box(task.compute_hash());
            black_box(hash);
        })
    });
}

// ============================================================================
// Async flush: Scheduling overhead
// ============================================================================

fn bench_async_flush_schedule(c: &mut Criterion) {
    let pipeline = AsyncFlushPipeline::new(|_result| {});

    let task = FlushTask::new(0, 1000, 1060, 42, 0);

    c.bench_function("async_flush/schedule", |b| {
        b.iter(|| {
            let result = pipeline.schedule_flush(black_box(task.clone()));
            black_box(result);
        })
    });
}

fn bench_async_flush_schedule_batch(c: &mut Criterion) {
    let pipeline = AsyncFlushPipeline::new(|_result| {});

    c.bench_function("async_flush/schedule_batch_1000", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let task = FlushTask::new(i as u32, 1000, 1060, 42, 0);
                let _ = pipeline.schedule_flush(task);
            }
        })
    });
}

// ============================================================================
// Timeline integration: Append with async flush
// ============================================================================

fn bench_timeline_append_baseline(c: &mut Criterion) {
    let timeline = TimelineAggregationCapsuleWrapper::default();

    c.bench_function("timeline/append_baseline", |b| {
        let mut i = 0u64;
        b.iter(|| {
            let time = SystemTime::UNIX_EPOCH + Duration::from_secs(1000 + i);
            i += 1;
            let result = timeline.append_system_time(black_box(time), "test");
            black_box(result);
        })
    });
}

fn bench_timeline_append_with_async_flush(c: &mut Criterion) {
    let pipeline = AsyncFlushPipeline::new(|_result| {});
    let timeline = TimelineAggregationCapsuleWrapper::default();

    c.bench_function("timeline/append_with_async_flush", |b| {
        let mut i = 0u64;
        b.iter(|| {
            let time = SystemTime::UNIX_EPOCH + Duration::from_secs(1000 + i);
            i += 1;
            let result = timeline.append_with_async_flush(black_box(time), Some(&pipeline));
            black_box(result);
        })
    });
}

// ============================================================================
// Throughput: Concurrent scheduling
// ============================================================================

fn bench_async_flush_throughput(c: &mut Criterion) {
    let completed = Arc::new(AtomicU64::new(0));
    let completed_clone = Arc::clone(&completed);

    let pipeline = Arc::new(AsyncFlushPipeline::new(move |_result| {
        completed_clone.fetch_add(1, Ordering::Relaxed);
    }));

    c.bench_function("async_flush/throughput_concurrent_10k", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..10)
                .map(|tid| {
                    let pipeline = Arc::clone(&pipeline);
                    std::thread::spawn(move || {
                        for i in 0..1000 {
                            let task = FlushTask::new((tid * 1000 + i) as u32, 1000, 1060, 42, 0);
                            let _ = pipeline.schedule_flush(task);
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        })
    });
}

// ============================================================================
// Latency: P99.9 measurement
// ============================================================================

fn bench_async_flush_p999_latency(c: &mut Criterion) {
    let pipeline = AsyncFlushPipeline::new(|_result| {});

    c.bench_function("async_flush/p999_latency", |b| {
        b.iter(|| {
            let mut latencies = Vec::with_capacity(1000);

            for i in 0..1000 {
                let task = FlushTask::new(i as u32, 1000, 1060, 42, 0);
                let start = std::time::Instant::now();
                let _ = pipeline.schedule_flush(task);
                latencies.push(start.elapsed().as_nanos() as u64);
            }

            // Calculate p99.9
            latencies.sort();
            let p999_idx = (latencies.len() as f64 * 0.999) as usize;
            let p999 = latencies[p999_idx];

            black_box(p999);
        })
    });
}

criterion_group!(
    benches,
    bench_sync_flush_compute_hash,
    bench_async_flush_schedule,
    bench_async_flush_schedule_batch,
    bench_timeline_append_baseline,
    bench_timeline_append_with_async_flush,
    bench_async_flush_throughput,
    bench_async_flush_p999_latency,
);
criterion_main!(benches);
