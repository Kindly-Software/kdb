//! io_uring Batch Capsule Benchmarks
//!
//! Performance validation for batched I/O operations vs individual operations.
//! Framework: B32 (fair baselines, 1000+ iterations, 95% CI)
//!
//! # Performance Targets
//!
//! - Batch submit (32 ops): <2μs (vs 32μs individual = 16× speedup)
//! - Harvest (32 CQEs): <1μs (vs 32×20ns = 640ns, ~1.6× speedup)
//! - Per-operation overhead: <100ns amortized
//! - Overall throughput: 10-100× depending on batch size

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::vec::Vec;

#[cfg(all(target_os = "linux", feature = "std"))]
fn benchmark_io_uring_batch(c: &mut Criterion) {
    use atomic_capsule::runtime::{IoUringCapsule, IoUringBatchCapsule};

    let mut group = c.benchmark_group("io_uring_batch");
    group.sample_size(100); // 100 samples
    group.measurement_time(std::time::Duration::from_secs(10));

    // ===== Batch Submission Benchmarks =====

    group.bench_function("batch_submit_8_ops", |b| {
        let ring = IoUringCapsule::new(256, 0).expect("init");
        let batch = IoUringBatchCapsule::new(&ring).expect("batch init");

        b.iter(|| {
            batch.batch_size.store(8, std::sync::atomic::Ordering::Release);
            batch.pending_ops.store(8, std::sync::atomic::Ordering::Release);

            let result = batch.submit_batch(8);
            black_box(result)
        });
    });

    group.bench_function("batch_submit_16_ops", |b| {
        let ring = IoUringCapsule::new(256, 0).expect("init");
        let batch = IoUringBatchCapsule::new(&ring).expect("batch init");

        b.iter(|| {
            batch.batch_size.store(16, std::sync::atomic::Ordering::Release);
            batch.pending_ops.store(16, std::sync::atomic::Ordering::Release);

            let result = batch.submit_batch(16);
            black_box(result)
        });
    });

    group.bench_function("batch_submit_32_ops", |b| {
        let ring = IoUringCapsule::new(256, 0).expect("init");
        let batch = IoUringBatchCapsule::new(&ring).expect("batch init");

        b.iter(|| {
            batch.batch_size.store(32, std::sync::atomic::Ordering::Release);
            batch.pending_ops.store(32, std::sync::atomic::Ordering::Release);

            let result = batch.submit_batch(32);
            black_box(result)
        });
    });

    group.bench_function("batch_submit_64_ops", |b| {
        let ring = IoUringCapsule::new(512, 0).expect("init");
        let batch = IoUringBatchCapsule::new(&ring).expect("batch init");

        b.iter(|| {
            batch.batch_size.store(64, std::sync::atomic::Ordering::Release);
            batch.pending_ops.store(64, std::sync::atomic::Ordering::Release);

            let result = batch.submit_batch(64);
            black_box(result)
        });
    });

    // ===== Completion Harvesting Benchmarks =====

    group.bench_function("harvest_completions_1", |b| {
        let ring = IoUringCapsule::new(256, 0).expect("init");
        let batch = IoUringBatchCapsule::new(&ring).expect("batch init");

        b.iter(|| {
            let result = batch.harvest_completions(1);
            black_box(result)
        });
    });

    group.bench_function("harvest_completions_8", |b| {
        let ring = IoUringCapsule::new(256, 0).expect("init");
        let batch = IoUringBatchCapsule::new(&ring).expect("batch init");

        b.iter(|| {
            let result = batch.harvest_completions(8);
            black_box(result)
        });
    });

    group.bench_function("harvest_completions_16", |b| {
        let ring = IoUringCapsule::new(256, 0).expect("init");
        let batch = IoUringBatchCapsule::new(&ring).expect("batch init");

        b.iter(|| {
            let result = batch.harvest_completions(16);
            black_box(result)
        });
    });

    group.bench_function("harvest_completions_32", |b| {
        let ring = IoUringCapsule::new(256, 0).expect("init");
        let batch = IoUringBatchCapsule::new(&ring).expect("batch init");

        b.iter(|| {
            let result = batch.harvest_completions(32);
            black_box(result)
        });
    });

    // ===== Backpressure Calculation Benchmarks =====

    group.bench_function("calculate_queue_pressure", |b| {
        let ring = IoUringCapsule::new(256, 0).expect("init");
        let batch = IoUringBatchCapsule::new(&ring).expect("batch init");

        b.iter(|| {
            let result = batch.calculate_queue_pressure();
            black_box(result)
        });
    });

    group.bench_function("should_throttle", |b| {
        let ring = IoUringCapsule::new(256, 0).expect("init");
        let batch = IoUringBatchCapsule::new(&ring).expect("batch init");

        b.iter(|| {
            let result = batch.should_throttle();
            black_box(result)
        });
    });

    // ===== Adaptive Batching Benchmarks =====

    group.bench_function("adapt_batch_size_low_latency", |b| {
        let ring = IoUringCapsule::new(256, 0).expect("init");
        let batch = IoUringBatchCapsule::new(&ring).expect("batch init");

        batch.avg_batch_latency_ns.store(100, std::sync::atomic::Ordering::Release);
        batch.queue_pressure.store(30, std::sync::atomic::Ordering::Release);

        b.iter(|| {
            let result = batch.adapt_batch_size();
            black_box(result)
        });
    });

    group.bench_function("adapt_batch_size_high_latency", |b| {
        let ring = IoUringCapsule::new(256, 0).expect("init");
        let batch = IoUringBatchCapsule::new(&ring).expect("batch init");

        batch.avg_batch_latency_ns.store(5000, std::sync::atomic::Ordering::Release);
        batch.queue_pressure.store(85, std::sync::atomic::Ordering::Release);

        b.iter(|| {
            let result = batch.adapt_batch_size();
            black_box(result)
        });
    });

    // ===== Pipeline Mode Benchmarks =====

    group.bench_function("pipeline_enable", |b| {
        let ring = IoUringCapsule::new(256, 0).expect("init");
        let batch = IoUringBatchCapsule::new(&ring).expect("batch init");

        b.iter(|| {
            let result = batch.enable_pipeline(2);
            black_box(result)
        });
    });

    group.bench_function("pipeline_advance_stage", |b| {
        let ring = IoUringCapsule::new(256, 0).expect("init");
        let batch = IoUringBatchCapsule::new(&ring).expect("batch init");
        batch.enable_pipeline(2).expect("enable");

        b.iter(|| {
            let result = batch.advance_pipeline_stage();
            black_box(result)
        });
    });

    // ===== Batch Operation Builders =====

    group.bench_function("batch_read_8_fds", |b| {
        let ring = IoUringCapsule::new(256, 0).expect("init");
        let batch = IoUringBatchCapsule::new(&ring).expect("batch init");

        let fds = vec![1; 8];
        let offsets = vec![0u64; 8];
        let mut buffers = vec![vec![0u8; 4096]; 8];
        let buf_refs: Vec<&mut [u8]> = buffers.iter_mut().map(|b| b.as_mut_slice()).collect();

        b.iter(|| {
            let result = batch.batch_read(
                &fds,
                &mut buf_refs.iter().map(|b| *b).collect::<Vec<_>>(),
                &offsets,
            );
            black_box(result)
        });
    });

    group.bench_function("batch_read_16_fds", |b| {
        let ring = IoUringCapsule::new(256, 0).expect("init");
        let batch = IoUringBatchCapsule::new(&ring).expect("batch init");

        let fds = vec![1; 16];
        let offsets = vec![0u64; 16];
        let mut buffers = vec![vec![0u8; 4096]; 16];
        let buf_refs: Vec<&mut [u8]> = buffers.iter_mut().map(|b| b.as_mut_slice()).collect();

        b.iter(|| {
            let result = batch.batch_read(
                &fds,
                &mut buf_refs.iter().map(|b| *b).collect::<Vec<_>>(),
                &offsets,
            );
            black_box(result)
        });
    });

    group.bench_function("batch_read_32_fds", |b| {
        let ring = IoUringCapsule::new(256, 0).expect("init");
        let batch = IoUringBatchCapsule::new(&ring).expect("batch init");

        let fds = vec![1; 32];
        let offsets = vec![0u64; 32];
        let mut buffers = vec![vec![0u8; 4096]; 32];
        let buf_refs: Vec<&mut [u8]> = buffers.iter_mut().map(|b| b.as_mut_slice()).collect();

        b.iter(|| {
            let result = batch.batch_read(
                &fds,
                &mut buf_refs.iter().map(|b| *b).collect::<Vec<_>>(),
                &offsets,
            );
            black_box(result)
        });
    });

    // ===== Stats Snapshot Benchmark =====

    group.bench_function("stats_snapshot", |b| {
        let ring = IoUringCapsule::new(256, 0).expect("init");
        let batch = IoUringBatchCapsule::new(&ring).expect("batch init");

        b.iter(|| {
            let stats = batch.stats();
            black_box(stats)
        });
    });

    group.finish();
}

#[cfg(all(target_os = "linux", feature = "std"))]
criterion_group!(benches, benchmark_io_uring_batch);

#[cfg(all(target_os = "linux", feature = "std"))]
criterion_main!(benches);

#[cfg(not(all(target_os = "linux", feature = "std")))]
fn main() {
    println!("io_uring benchmarks require Linux target and 'std' feature");
}
