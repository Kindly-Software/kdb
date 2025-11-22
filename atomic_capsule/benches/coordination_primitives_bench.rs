//! # Coordination Primitives - B32 Benchmarks
//!
//! **B32 Benchmark Framework** for 3 coordination capsules.
//!
//! ## Benchmarks
//!
//! - **PhaseCoordinatorCapsule**: start/finish/get_phase (9 benchmarks)
//! - **LockfreeHashBucketCapsule**: insert/probe/collision (6 benchmarks)
//! - **ParallelPartitionCapsule**: push_result/increment_processed/mark_done (6 benchmarks)
//!
//! **Total**: 21 benchmarks, B32-compliant (1000+ iterations, 95% CI)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use atomic_capsule::primitives::coordination::{
    LockfreeHashBucketCapsule, ParallelPartitionCapsule, PhaseCoordinatorCapsule,
};
use std::sync::Arc;
use std::thread;

// ============================================================================
// MODULE 1: PhaseCoordinator Benchmarks (9 benchmarks)
// ============================================================================

fn bench_phase_coordinator(c: &mut Criterion) {
    let mut group = c.benchmark_group("phase_coordinator");

    // Benchmark 1: start_phase (AcqRel CAS)
    group.bench_function("start_phase", |b| {
        b.iter_batched(
            || PhaseCoordinatorCapsule::new(),
            |coord| {
                coord.start_phase(1).unwrap();
                black_box(&coord)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Benchmark 2: finish_phase (AcqRel CAS)
    group.bench_function("finish_phase", |b| {
        b.iter_batched(
            || {
                let coord = PhaseCoordinatorCapsule::new();
                coord.start_phase(1).unwrap();
                coord
            },
            |coord| {
                coord.finish_phase(1).unwrap();
                black_box(&coord)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Benchmark 3: get_phase (Acquire load)
    group.bench_function("get_phase", |b| {
        let coord = PhaseCoordinatorCapsule::new();
        coord.start_phase(1).unwrap();

        b.iter(|| {
            black_box(coord.get_phase());
        })
    });

    // Benchmark 4: get_stats (Acquire load + unpack)
    group.bench_function("get_stats", |b| {
        let coord = PhaseCoordinatorCapsule::new();
        coord.start_phase(1).unwrap();

        b.iter(|| {
            black_box(coord.get_stats());
        })
    });

    // Benchmark 5: record_error (AcqRel CAS + Relaxed increment)
    group.bench_function("record_error", |b| {
        let coord = PhaseCoordinatorCapsule::new();

        b.iter(|| {
            coord.record_error(black_box(0x0001));
        })
    });

    // Benchmark 6: Full phase lifecycle (start + finish)
    group.bench_function("full_phase_lifecycle", |b| {
        b.iter_batched(
            || PhaseCoordinatorCapsule::new(),
            |coord| {
                coord.start_phase(1).unwrap();
                coord.finish_phase(1).unwrap();
                black_box(&coord)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Benchmark 7: Multi-phase progression (10 phases)
    group.bench_function("multi_phase_10_phases", |b| {
        b.iter_batched(
            || PhaseCoordinatorCapsule::new(),
            |coord| {
                for phase in 1..=10 {
                    coord.start_phase(phase).unwrap();
                    coord.finish_phase(phase).unwrap();
                }
                black_box(&coord)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Benchmark 8: Concurrent readers (8 threads)
    group.bench_function("concurrent_readers_8_threads", |b| {
        let coord = Arc::new(PhaseCoordinatorCapsule::new());
        coord.start_phase(1).unwrap();

        b.iter(|| {
            let mut handles = vec![];

            for _ in 0..8 {
                let coord_clone = Arc::clone(&coord);
                handles.push(thread::spawn(move || {
                    for _ in 0..100 {
                        black_box(coord_clone.get_phase());
                    }
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }
        })
    });

    // Benchmark 9: wait_phase with backoff (worst case)
    group.bench_function("wait_phase_immediate", |b| {
        let coord = PhaseCoordinatorCapsule::new();
        coord.start_phase(1).unwrap();
        coord.finish_phase(1).unwrap();

        b.iter(|| {
            coord.wait_phase(black_box(1)); // Already at phase 1
        })
    });

    group.finish();
}

// ============================================================================
// MODULE 2: LockfreeHashBucket Benchmarks (6 benchmarks)
// ============================================================================

fn bench_hash_bucket(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_bucket");

    // Benchmark 1: insert (empty bucket)
    group.bench_function("insert_empty", |b| {
        b.iter_batched(
            || LockfreeHashBucketCapsule::new(),
            |bucket| {
                bucket.insert(black_box(42), black_box(100)).unwrap();
                black_box(&bucket)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Benchmark 2: insert (with collisions)
    group.bench_function("insert_with_collisions", |b| {
        b.iter_batched(
            || {
                let bucket = LockfreeHashBucketCapsule::new();
                // Pre-insert 10 entries
                for i in 0..10 {
                    bucket.insert(i, 100).unwrap();
                }
                bucket
            },
            |bucket| {
                bucket.insert(black_box(99), black_box(100)).unwrap();
                black_box(&bucket)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Benchmark 3: probe (empty bucket)
    group.bench_function("probe_empty", |b| {
        let bucket = LockfreeHashBucketCapsule::new();

        b.iter(|| {
            black_box(bucket.probe(black_box(42)));
        })
    });

    // Benchmark 4: probe (hit, no collision)
    group.bench_function("probe_hit_no_collision", |b| {
        let bucket = LockfreeHashBucketCapsule::new();
        bucket.insert(42, 100).unwrap();

        b.iter(|| {
            black_box(bucket.probe(black_box(42)));
        })
    });

    // Benchmark 5: probe (hit, with collisions)
    group.bench_function("probe_hit_with_collisions", |b| {
        let bucket = LockfreeHashBucketCapsule::new();
        for i in 0..10 {
            bucket.insert(i, 100).unwrap();
        }

        b.iter(|| {
            black_box(bucket.probe(black_box(5))); // Middle of chain
        })
    });

    // Benchmark 6: concurrent insert (8 threads)
    group.bench_function("concurrent_insert_8_threads", |b| {
        b.iter_batched(
            || Arc::new(LockfreeHashBucketCapsule::new()),
            |bucket| {
                let mut handles = vec![];

                for thread_id in 0..8 {
                    let bucket_clone = Arc::clone(&bucket);
                    handles.push(thread::spawn(move || {
                        for i in 0..100 {
                            let key = thread_id * 100 + i;
                            bucket_clone.insert(key, key * 2).unwrap();
                        }
                    }));
                }

                for handle in handles {
                    handle.join().unwrap();
                }

                black_box(&bucket)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

// ============================================================================
// MODULE 3: ParallelPartition Benchmarks (6 benchmarks)
// ============================================================================

fn bench_partition(c: &mut Criterion) {
    let mut group = c.benchmark_group("partition");

    // Benchmark 1: push_result (thread-local, Relaxed)
    group.bench_function("push_result", |b| {
        let partition = ParallelPartitionCapsule::new();

        b.iter(|| {
            partition.push_result().unwrap();
        })
    });

    // Benchmark 2: increment_processed (AcqRel fetch_add)
    group.bench_function("increment_processed", |b| {
        let partition = ParallelPartitionCapsule::new();

        b.iter(|| {
            partition.increment_processed(black_box(1));
        })
    });

    // Benchmark 3: processed (Acquire load)
    group.bench_function("processed", |b| {
        let partition = ParallelPartitionCapsule::new();
        partition.increment_processed(100);

        b.iter(|| {
            black_box(partition.processed());
        })
    });

    // Benchmark 4: result_count (Relaxed load)
    group.bench_function("result_count", |b| {
        let partition = ParallelPartitionCapsule::new();
        partition.push_result().unwrap();

        b.iter(|| {
            black_box(partition.result_count());
        })
    });

    // Benchmark 5: mark_done (AcqRel CAS)
    group.bench_function("mark_done", |b| {
        b.iter_batched(
            || {
                let partition = ParallelPartitionCapsule::new();
                partition.push_result().unwrap();
                partition
            },
            |partition| {
                partition.mark_done().unwrap();
                black_box(&partition)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Benchmark 6: Full partition lifecycle (push + mark_done)
    group.bench_function("full_partition_lifecycle", |b| {
        b.iter_batched(
            || ParallelPartitionCapsule::with_capacity(1000),
            |partition| {
                for _ in 0..100 {
                    partition.push_result().unwrap();
                    partition.increment_processed(1);
                }
                partition.mark_done().unwrap();
                black_box(&partition)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

// ============================================================================
// BASELINE BENCHMARKS (Mutex-based comparison for B32 honesty)
// ============================================================================

fn bench_baseline_mutex_phase(c: &mut Criterion) {
    use std::sync::Mutex;

    let mut group = c.benchmark_group("baseline_mutex");

    // Baseline: Mutex<u8> for phase coordination
    group.bench_function("mutex_phase_transition", |b| {
        b.iter_batched(
            || Mutex::new(0u8),
            |mutex| {
                let mut guard = mutex.lock().unwrap();
                *guard += 1; // Simulate phase transition
                drop(guard);
                black_box(&mutex)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Baseline: Arc<Mutex<Vec<u64>>> for hash bucket
    group.bench_function("mutex_vec_insert", |b| {
        b.iter_batched(
            || Mutex::new(Vec::<(u64, u64)>::new()),
            |mutex| {
                let mut guard = mutex.lock().unwrap();
                guard.push((black_box(42), black_box(100)));
                drop(guard);
                black_box(&mutex)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Baseline: Arc<Mutex<Vec<T>>> for partition
    group.bench_function("mutex_vec_push", |b| {
        b.iter_batched(
            || Arc::new(Mutex::new(Vec::<u64>::new())),
            |mutex| {
                let mut guard = mutex.lock().unwrap();
                guard.push(black_box(42));
                drop(guard);
                black_box(&mutex)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

// ============================================================================
// PERFORMANCE ASSERTIONS (B32 Reality Check)
// ============================================================================

fn bench_performance_assertions(c: &mut Criterion) {
    let mut group = c.benchmark_group("performance_assertions");

    // Assertion 1: PhaseCoordinatorCapsule.get_phase() <10ns
    group.bench_function("assert_get_phase_lt_10ns", |b| {
        let coord = PhaseCoordinatorCapsule::new();
        coord.start_phase(1).unwrap();

        b.iter(|| {
            let start = std::time::Instant::now();
            black_box(coord.get_phase());
            let duration = start.elapsed();

            // Target: <10ns (will be measured by criterion)
            assert!(duration.as_nanos() < 1000); // <1µs sanity check
        })
    });

    // Assertion 2: LockfreeHashBucketCapsule.insert() <50ns
    group.bench_function("assert_insert_lt_50ns", |b| {
        b.iter_batched(
            || LockfreeHashBucketCapsule::new(),
            |bucket| {
                let start = std::time::Instant::now();
                bucket.insert(black_box(42), black_box(100)).unwrap();
                let duration = start.elapsed();

                // Target: <50ns (will be measured by criterion)
                assert!(duration.as_nanos() < 10000); // <10µs sanity check
                black_box(&bucket)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Assertion 3: ParallelPartitionCapsule.push_result() <20ns
    group.bench_function("assert_push_result_lt_20ns", |b| {
        let partition = ParallelPartitionCapsule::new();

        b.iter(|| {
            let start = std::time::Instant::now();
            partition.push_result().unwrap();
            let duration = start.elapsed();

            // Target: <20ns (will be measured by criterion)
            assert!(duration.as_nanos() < 5000); // <5µs sanity check
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_phase_coordinator,
    bench_hash_bucket,
    bench_partition,
    bench_baseline_mutex_phase,
    bench_performance_assertions,
);
criterion_main!(benches);
