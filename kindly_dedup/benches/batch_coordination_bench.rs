//! # BatchCoordinatorCapsule Benchmark Suite - B32 Framework
//!
//! **Tier**: T1 (Atomic) + T4 (Batch)
//!
//! **Purpose**: Validate contention reduction from 50% → 5% (10× improvement)
//! by measuring claim/complete latencies under concurrent load.
//!
//! **Framework**: B32 (95% CI, 1000+ iterations, fair baselines)
//!
//! ## Benchmarks (3 suites)
//!
//! 1. **Basic Operations**
//!    - claim_batch_single_worker: Baseline CAS latency
//!    - complete_batch_single_worker: Generation increment overhead
//!    - stats_snapshot: Health check performance
//!
//! 2. **Contention Scaling**
//!    - claim_batch_2_workers: 2-way CAS contention
//!    - claim_batch_4_workers: 4-way CAS contention
//!    - claim_batch_8_workers: 8-way CAS contention
//!    - claim_batch_16_workers: 16-way CAS contention
//!
//! 3. **End-to-End Pipeline**
//!    - pipeline_single_worker: Sequential claim → complete
//!    - pipeline_16_workers: Full 16-worker concurrent pipeline

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use kindly_dedup::parallel::BatchCoordinatorCapsule;
use std::sync::Arc;
use std::thread;

// ============================================================================
// SUITE 1: BASIC OPERATIONS
// ============================================================================

fn bench_claim_batch_single_worker(c: &mut Criterion) {
    c.bench_function("claim_batch_single_worker", |b| {
        b.iter_batched(
            || {
                let coordinator = BatchCoordinatorCapsule::new();
                coordinator.add_batch();
                coordinator
            },
            |coordinator| {
                black_box(coordinator.claim_batch(0).expect("Should claim batch"))
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_complete_batch_single_worker(c: &mut Criterion) {
    c.bench_function("complete_batch_single_worker", |b| {
        b.iter_batched(
            || {
                let coordinator = BatchCoordinatorCapsule::new();
                coordinator.add_batch();
                let batch = coordinator.claim_batch(0).expect("Should claim batch");
                (coordinator, batch)
            },
            |(coordinator, batch)| {
                coordinator
                    .complete_batch(batch, 0)
                    .expect("Should complete batch")
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_stats_snapshot(c: &mut Criterion) {
    c.bench_function("stats_snapshot", |b| {
        let coordinator = BatchCoordinatorCapsule::new();
        for _ in 0..100 {
            coordinator.add_batch();
        }
        for i in 0..100 {
            let batch = coordinator.claim_batch((i % 16) as u32).expect("Should claim batch");
            coordinator.complete_batch(batch, (i % 16) as u32).expect("Should complete");
        }

        b.iter(|| black_box(coordinator.stats()));
    });
}

// ============================================================================
// SUITE 2: CONTENTION SCALING
// ============================================================================

fn bench_claim_batch_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("claim_batch_concurrent");

    for num_workers in [2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_workers", num_workers)),
            num_workers,
            |b, &num_workers| {
                b.iter_batched(
                    || {
                        let coordinator = Arc::new(BatchCoordinatorCapsule::new());
                        // Add batches for all workers
                        for _ in 0..(num_workers * 10) {
                            coordinator.add_batch();
                        }
                        coordinator
                    },
                    |coordinator| {
                        // Simulate concurrent claims
                        let mut handles = vec![];
                        let completed = Arc::new(std::sync::atomic::AtomicUsize::new(0));

                        for worker_id in 0..num_workers {
                            let coordinator_clone = Arc::clone(&coordinator);
                            let completed_clone = Arc::clone(&completed);

                            let handle = thread::spawn(move || {
                                let mut count = 0;
                                loop {
                                    match coordinator_clone.claim_batch(worker_id as u32) {
                                        Ok(_batch) => {
                                            count += 1;
                                        }
                                        Err(_) => break,
                                    }
                                }
                                completed_clone.fetch_add(count, std::sync::atomic::Ordering::Release);
                            });

                            handles.push(handle);
                        }

                        for handle in handles {
                            handle.join().expect("Worker panicked");
                        }

                        black_box(completed.load(std::sync::atomic::Ordering::Acquire))
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

// ============================================================================
// SUITE 3: END-TO-END PIPELINE
// ============================================================================

fn bench_pipeline_single_worker(c: &mut Criterion) {
    c.bench_function("pipeline_single_worker_1000_batches", |b| {
        b.iter_batched(
            || {
                let coordinator = BatchCoordinatorCapsule::new();
                for _ in 0..1000 {
                    coordinator.add_batch();
                }
                coordinator
            },
            |coordinator| {
                for i in 0..1000 {
                    let batch = coordinator.claim_batch(0).expect("Should claim batch");
                    let _ = coordinator.complete_batch(batch, 0);
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_pipeline_16_workers(c: &mut Criterion) {
    c.bench_function("pipeline_16_workers_1000_batches", |b| {
        b.iter_batched(
            || {
                let coordinator = Arc::new(BatchCoordinatorCapsule::new());
                for _ in 0..1000 {
                    coordinator.add_batch();
                }
                coordinator
            },
            |coordinator| {
                let mut handles = vec![];
                let completed = Arc::new(std::sync::atomic::AtomicUsize::new(0));

                for worker_id in 0..16 {
                    let coordinator_clone = Arc::clone(&coordinator);
                    let completed_clone = Arc::clone(&completed);

                    let handle = thread::spawn(move || {
                        let mut count = 0;
                        loop {
                            match coordinator_clone.claim_batch(worker_id as u32) {
                                Ok(batch) => {
                                    let _ = coordinator_clone.complete_batch(batch, worker_id as u32);
                                    count += 1;
                                }
                                Err(_) => break,
                            }
                        }
                        completed_clone.fetch_add(count, std::sync::atomic::Ordering::Release);
                    });

                    handles.push(handle);
                }

                for handle in handles {
                    handle.join().expect("Worker panicked");
                }

                black_box(completed.load(std::sync::atomic::Ordering::Acquire))
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

criterion_group!(
    benches,
    bench_claim_batch_single_worker,
    bench_complete_batch_single_worker,
    bench_stats_snapshot,
    bench_claim_batch_concurrent,
    bench_pipeline_single_worker,
    bench_pipeline_16_workers,
);
criterion_main!(benches);
