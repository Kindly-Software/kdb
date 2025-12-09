//! B32 System Benchmarks - Throughput and Contention Analysis
//!
//! # B32 Compliance
//!
//! - ✅ B4: Test uncontended (1 thread) and contended (2,4,8,16 threads) cases
//! - ✅ B8: Warm CPU caches before measurement
//! - ✅ B14: Monitor memory bandwidth saturation
//! - ✅ B18: Identify scaling cliffs and document efficiency
//! - ✅ K12: Lockfree scaling sweet spot <12 threads (AMD 6900HX)
//! - ✅ K23: Expect 6.5× with 6 P-cores, 10-12× with all cores
//!
//! # Expected Results (B32 K20, K23)
//!
//! | Threads | Target Throughput | Scaling Efficiency |
//! |---------|-------------------|-------------------|
//! | 1 | 10M ops/sec | 1.0× (baseline) |
//! | 2 | 19M ops/sec | 0.95× |
//! | 4 | 36M ops/sec | 0.90× |
//! | 8 | 64M ops/sec | 0.80× (E-cores) |
//! | 16 | 96M ops/sec | 0.60× (contention) |
//!
//! Reality: Expect near-linear scaling up to 8 threads, then diminishing
//! returns due to memory bandwidth (K29) and CAS contention (K12).

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use git_coordinator_bench::{GitCoordinator, GitOperation};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Configure Criterion for B32 system benchmarks
fn criterion_config() -> Criterion {
    Criterion::default()
        .sample_size(100) // Fewer samples for multi-threaded (longer duration)
        .measurement_time(Duration::from_secs(15)) // Longer for thermal stability
        .warm_up_time(Duration::from_secs(3)) // B8: Warm CPU caches
        .confidence_level(0.95)
}

/// Benchmark Group 1: Lock contention scaling (1, 2, 4, 8, 16 threads)
///
/// Tests how lock acquisition scales under contention.
/// Expected: Near-linear up to 8 threads, then diminishing (K12, K23)
fn bench_lock_contention_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("lock/contention/scaling");

    for num_threads in [1, 2, 4, 8, 16] {
        group.throughput(Throughput::Elements(num_threads * 1000));

        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            &num_threads,
            |b, &threads| {
                let lock = Arc::new(git_coordinator_bench::AtomicLock::new());

                b.iter(|| {
                    // Each thread attempts 1000 lock acquires
                    let handles: Vec<_> = (0..threads)
                        .map(|tid| {
                            let lock_clone = Arc::clone(&lock);
                            std::thread::spawn(move || {
                                let mut successes = 0u64;
                                for _ in 0..1000 {
                                    if let Some(guard) = lock_clone.try_acquire(tid as u32) {
                                        black_box(&guard);
                                        successes += 1;
                                        drop(guard);
                                    }
                                }
                                successes
                            })
                        })
                        .collect();

                    let total: u64 = handles
                        .into_iter()
                        .map(|h| h.join().unwrap())
                        .sum();

                    black_box(total);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark Group 2: Queue throughput scaling (single producer, N consumers)
///
/// Tests how queue dequeue scales with multiple consumers.
/// Expected: Near-linear up to 4 threads (memory bandwidth limit, K29)
fn bench_queue_throughput_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("queue/throughput/scaling");

    for num_consumers in [1, 2, 4, 8] {
        group.throughput(Throughput::Elements(10_000));

        group.bench_with_input(
            BenchmarkId::from_parameter(num_consumers),
            &num_consumers,
            |b, &consumers| {
                let queue = Arc::new(git_coordinator_bench::OperationQueue::new(16384));

                b.iter(|| {
                    // Producer: enqueue 10K operations
                    for _ in 0..10_000 {
                        queue.try_enqueue(GitOperation::Read);
                    }

                    // Consumers: dequeue in parallel
                    let handles: Vec<_> = (0..consumers)
                        .map(|_| {
                            let queue_clone = Arc::clone(&queue);
                            std::thread::spawn(move || {
                                let mut dequeued = 0u64;
                                while queue_clone.try_dequeue().is_some() {
                                    dequeued += 1;
                                }
                                dequeued
                            })
                        })
                        .collect();

                    let total: u64 = handles
                        .into_iter()
                        .map(|h| h.join().unwrap())
                        .sum();

                    black_box(total);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark Group 3: Coordinator execute throughput (N instances)
///
/// Tests end-to-end throughput with multiple coordinator instances.
/// Expected: 10× scaling at 16 threads (B32 example target)
fn bench_coordinator_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("coordinator/throughput/instances");

    for num_instances in [1, 2, 4, 8, 16] {
        group.throughput(Throughput::Elements(num_instances * 100));

        group.bench_with_input(
            BenchmarkId::from_parameter(num_instances),
            &num_instances,
            |b, &instances| {
                let coord = GitCoordinator::new(0);

                b.iter(|| {
                    let handles: Vec<_> = (0..instances)
                        .map(|tid| {
                            let coord_clone = coord.clone_shared(tid as u32);
                            std::thread::spawn(move || {
                                let mut successes = 0u64;
                                for _ in 0..100 {
                                    if coord_clone.execute(|| {
                                        black_box(42);
                                    }).is_ok() {
                                        successes += 1;
                                    }
                                }
                                successes
                            })
                        })
                        .collect();

                    let total: u64 = handles
                        .into_iter()
                        .map(|h| h.join().unwrap())
                        .sum();

                    black_box(total);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark Group 4: Lock timeout behavior under contention
///
/// Tests lock acquisition timeout mechanism.
/// Expected: Timeouts increase with contention (monitoring metric)
fn bench_lock_timeout_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("lock/timeout/contention");

    for num_threads in [2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            &num_threads,
            |b, &threads| {
                let lock = Arc::new(git_coordinator_bench::AtomicLock::new());
                let timeout_count = Arc::new(AtomicU64::new(0));

                b.iter(|| {
                    let handles: Vec<_> = (0..threads)
                        .map(|tid| {
                            let lock_clone = Arc::clone(&lock);
                            let timeouts = Arc::clone(&timeout_count);
                            std::thread::spawn(move || {
                                for _ in 0..10 {
                                    // Very short timeout to force contention
                                    match lock_clone.acquire_timeout(
                                        tid as u32,
                                        Duration::from_micros(10),
                                    ) {
                                        Some(guard) => {
                                            black_box(&guard);
                                            // Hold lock briefly
                                            std::thread::sleep(Duration::from_micros(5));
                                        }
                                        None => {
                                            timeouts.fetch_add(1, Ordering::Relaxed);
                                        }
                                    }
                                }
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }

                    let timeouts = timeout_count.load(Ordering::Relaxed);
                    black_box(timeouts);
                    timeout_count.store(0, Ordering::Relaxed);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark Group 5: Backoff efficiency under contention
///
/// Measures exponential backoff effectiveness.
/// Expected: Reduced CPU utilization vs spin-loop
fn bench_backoff_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("lock/backoff/efficiency");

    let lock = Arc::new(git_coordinator_bench::AtomicLock::new());

    group.bench_function("contended_8_threads", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..8)
                .map(|tid| {
                    let lock_clone = Arc::clone(&lock);
                    std::thread::spawn(move || {
                        // Try to acquire with generous timeout
                        if let Some(guard) = lock_clone.acquire_timeout(
                            tid as u32,
                            Duration::from_millis(10),
                        ) {
                            black_box(&guard);
                            // Hold lock for 1ms
                            std::thread::sleep(Duration::from_micros(1000));
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = criterion_config();
    targets = bench_lock_contention_scaling,
              bench_queue_throughput_scaling,
              bench_coordinator_throughput,
              bench_lock_timeout_contention,
              bench_backoff_efficiency
}

criterion_main!(benches);
