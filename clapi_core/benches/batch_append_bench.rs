//! Batch Append Benchmarks (B32 Framework)
//!
//! Performance targets:
//! - Single append: ~78ns per event
//! - Batch append: ~15ns per event (5.2× faster)
//! - Batch 1000 events: 15μs total (vs 78μs single)

use clapi_core::capsules::batch_append_capsule::BatchAppendRequest;
use clapi_core::capsules::timeline_aggregation_capsule::TimelineAggregationCapsuleWrapper;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::{Duration, SystemTime};

// ============================================================================
// Baseline: Single append
// ============================================================================

fn bench_single_append(c: &mut Criterion) {
    let timeline = TimelineAggregationCapsuleWrapper::default();

    c.bench_function("single_append/1_event", |b| {
        let mut i = 0u64;
        b.iter(|| {
            let time = SystemTime::UNIX_EPOCH + Duration::from_secs(1000 + i);
            i += 1;
            let result = timeline.append_system_time(black_box(time), "test");
            black_box(result);
        })
    });
}

fn bench_single_append_1000(c: &mut Criterion) {
    c.bench_function("single_append/1000_events", |b| {
        b.iter(|| {
            let timeline = TimelineAggregationCapsuleWrapper::default();
            for i in 0..1000 {
                let time = SystemTime::UNIX_EPOCH + Duration::from_secs(1000 + i);
                let _ = timeline.append_system_time(time, "test");
            }
            black_box(timeline.total_events());
        })
    });
}

// ============================================================================
// Batch append: Various batch sizes
// ============================================================================

fn bench_batch_append_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_append");

    for size in [10, 100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let timeline = TimelineAggregationCapsuleWrapper::default();
            let timestamps: Vec<u64> = (1000..1000 + size).collect();
            let request = BatchAppendRequest::new(timestamps);

            b.iter(|| {
                let result = timeline.append_batch(black_box(request.clone()));
                black_box(result);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Batch vs Single comparison
// ============================================================================

fn bench_batch_vs_single_1000(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_vs_single_1000");

    // Single append baseline
    group.bench_function("single", |b| {
        b.iter(|| {
            let timeline = TimelineAggregationCapsuleWrapper::default();
            for i in 0..1000 {
                let time = SystemTime::UNIX_EPOCH + Duration::from_secs(1000 + i);
                let _ = timeline.append_system_time(time, "test");
            }
            black_box(timeline.total_events());
        })
    });

    // Batch append
    group.bench_function("batch", |b| {
        b.iter(|| {
            let timeline = TimelineAggregationCapsuleWrapper::default();
            let timestamps: Vec<u64> = (1000..2000).collect();
            let request = BatchAppendRequest::new(timestamps);
            let result = timeline.append_batch(request);
            black_box(result);
        })
    });

    group.finish();
}

// ============================================================================
// Batch with pre-computed hints
// ============================================================================

fn bench_batch_with_hints(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_hints");

    // Without hints
    group.bench_function("without_hints", |b| {
        let timeline = TimelineAggregationCapsuleWrapper::default();
        let timestamps: Vec<u64> = (1000..2000).collect();
        let request = BatchAppendRequest::new(timestamps);

        b.iter(|| {
            let result = timeline.append_batch(black_box(request.clone()));
            black_box(result);
        })
    });

    // With hints (pre-computed bucket IDs)
    group.bench_function("with_hints", |b| {
        let timeline = TimelineAggregationCapsuleWrapper::default();
        let timestamps: Vec<u64> = (1000..2000).collect();

        // Pre-compute bucket hints (all timestamps map to same bucket for simplicity)
        let hints: Vec<u32> = (0..1000).map(|i| (i / 60) as u32).collect();
        let request = BatchAppendRequest::with_hints(timestamps, hints).unwrap();

        b.iter(|| {
            let result = timeline.append_batch(black_box(request.clone()));
            black_box(result);
        })
    });

    group.finish();
}

// ============================================================================
// Throughput: Events per second
// ============================================================================

fn bench_throughput_single(c: &mut Criterion) {
    c.bench_function("throughput/single_eps", |b| {
        let timeline = TimelineAggregationCapsuleWrapper::default();
        let mut i = 0u64;

        b.iter(|| {
            // Append 10K events
            for _ in 0..10000 {
                let time = SystemTime::UNIX_EPOCH + Duration::from_secs(1000 + i);
                i += 1;
                let _ = timeline.append_system_time(time, "test");
            }
            black_box(timeline.total_events());
        })
    });
}

fn bench_throughput_batch(c: &mut Criterion) {
    c.bench_function("throughput/batch_eps", |b| {
        let timeline = TimelineAggregationCapsuleWrapper::default();
        let timestamps: Vec<u64> = (1000..11000).collect();
        let request = BatchAppendRequest::new(timestamps);

        b.iter(|| {
            let result = timeline.append_batch(black_box(request.clone()));
            black_box(result);
        })
    });
}

// ============================================================================
// Latency per item
// ============================================================================

fn bench_latency_per_item(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_per_item");

    // Single append latency
    group.bench_function("single", |b| {
        let timeline = TimelineAggregationCapsuleWrapper::default();
        let mut i = 0u64;

        b.iter(|| {
            let time = SystemTime::UNIX_EPOCH + Duration::from_secs(1000 + i);
            i += 1;
            let start = std::time::Instant::now();
            let _ = timeline.append_system_time(time, "test");
            let latency = start.elapsed().as_nanos();
            black_box(latency);
        })
    });

    // Batch append latency (amortized)
    group.bench_function("batch_amortized", |b| {
        let timeline = TimelineAggregationCapsuleWrapper::default();
        let timestamps: Vec<u64> = (1000..2000).collect();
        let request = BatchAppendRequest::new(timestamps);

        b.iter(|| {
            let start = std::time::Instant::now();
            let stats = timeline.append_batch(request.clone()).unwrap();
            let total_latency = start.elapsed().as_nanos();
            let per_item = total_latency / stats.appended as u128;
            black_box(per_item);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_single_append,
    bench_single_append_1000,
    bench_batch_append_sizes,
    bench_batch_vs_single_1000,
    bench_batch_with_hints,
    bench_throughput_single,
    bench_throughput_batch,
    bench_latency_per_item,
);
criterion_main!(benches);
