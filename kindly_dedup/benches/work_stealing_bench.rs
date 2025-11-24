//! # Work-Stealing Queue Benchmarks (B32 Fair Benchmarking Framework)
//!
//! **Framework**: B32 (Fair baseline, 1000+ iterations, 95% CI)
//! **Tier**: T4 (Batch) + T1 (Atomic)
//! **Hardware**: AMD Ryzen 9 6900HX (8c/16t, 64GB DDR5-4800)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use kindly_dedup::parallel::{WorkStealingQueueCapsule, WorkItem};
use std::sync::Arc;
use std::thread;

// ============================================================================
// MICROBENCHMARKS: Individual operation performance
// ============================================================================

fn bench_push_single_thread(c: &mut Criterion) {
    // B32: Measure push latency in isolation
    // Expected: <20ns (owner thread, no contention)

    let mut group = c.benchmark_group("push_single_thread");
    group.throughput(Throughput::Elements(1));

    group.bench_function("push_to_queue", |b| {
        let mut queue = WorkStealingQueueCapsule::new(16384).unwrap();
        let mut counter = 0u64;

        b.iter(|| {
            let item = WorkItem::new(black_box(counter), 10);
            queue.push(item).ok();
            counter += 1;

            // Pop to prevent full queue
            if counter % 1000 == 0 {
                queue.pop();
            }
        });
    });

    group.finish();
}

fn bench_pop_single_thread(c: &mut Criterion) {
    // B32: Measure pop latency
    // Expected: <50ns (owner thread, SeqCst sync with steals)

    let mut group = c.benchmark_group("pop_single_thread");
    group.throughput(Throughput::Elements(1));

    group.bench_function("pop_from_queue", |b| {
        let mut queue = WorkStealingQueueCapsule::new(16384).unwrap();

        // Pre-populate queue
        for i in 0..1000 {
            queue.push(WorkItem::new(i, 10)).ok();
        }

        let mut pop_count = 0;
        b.iter(|| {
            if queue.pop().is_some() {
                pop_count += 1;
            }

            // Re-push to refill
            if pop_count % 500 == 0 && pop_count > 0 {
                for i in 0..500 {
                    queue.push(WorkItem::new(pop_count * 1000 + i, 10)).ok();
                }
            }
        });
    });

    group.finish();
}

fn bench_steal_single_thief(c: &mut Criterion) {
    // B32: Measure steal latency with single thief
    // Expected: <100ns (CAS loop, SeqCst ordering)

    let mut group = c.benchmark_group("steal_single_thief");
    group.throughput(Throughput::Elements(1));

    group.bench_function("steal_from_queue", |b| {
        let queue = WorkStealingQueueCapsule::new(16384).unwrap();

        // Simulate owner pushing items (in background, not measured)
        let queue_for_push = Arc::new(queue);
        let queue_for_bench = Arc::clone(&queue_for_push);

        // Pre-populate
        unsafe {
            let queue_mut = &mut *(Arc::as_ptr(&queue_for_push) as *mut WorkStealingQueueCapsule);
            for i in 0..1000 {
                queue_mut.push(WorkItem::new(i, 10)).ok();
            }
        }

        b.iter(|| {
            queue_for_bench.steal();
        });
    });

    group.finish();
}

fn bench_is_empty_query(c: &mut Criterion) {
    // B32: Measure is_empty latency
    // Expected: <10ns (Acquire loads)

    let mut group = c.benchmark_group("is_empty_query");

    group.bench_function("is_empty_check", |b| {
        let queue = WorkStealingQueueCapsule::new(16384).unwrap();

        b.iter(|| {
            black_box(queue.is_empty());
        });
    });

    group.finish();
}

fn bench_stats_snapshot(c: &mut Criterion) {
    // B32: Measure stats snapshot latency
    // Expected: <100ns (5 SeqCst loads)

    let mut group = c.benchmark_group("stats_snapshot");

    group.bench_function("stats_snapshot", |b| {
        let mut queue = WorkStealingQueueCapsule::new(16384).unwrap();

        // Generate some activity
        for i in 0..100 {
            queue.push(WorkItem::new(i, 10)).ok();
        }

        b.iter(|| {
            black_box(queue.stats());
        });
    });

    group.finish();
}

// ============================================================================
// THROUGHPUT BENCHMARKS: Multi-threaded scenarios
// ============================================================================

fn bench_push_pop_throughput(c: &mut Criterion) {
    // B32: Measure combined push/pop throughput
    // Expected: 50M+ operations/sec (lockfree, no contention)

    let mut group = c.benchmark_group("push_pop_throughput");
    group.throughput(Throughput::Elements(1000));
    group.sample_size(100);

    group.bench_function("1000_push_pop", |b| {
        let mut queue = WorkStealingQueueCapsule::new(16384).unwrap();

        b.iter(|| {
            for i in 0..1000 {
                queue.push(WorkItem::new(i, 10)).ok();
            }

            for _ in 0..500 {
                queue.pop();
            }
        });
    });

    group.finish();
}

fn bench_owner_vs_thief_contention(c: &mut Criterion) {
    // B32: Measure contention between owner and thief
    // Expected: <10% slowdown from contention (mostly lockfree)

    let mut group = c.benchmark_group("owner_vs_thief");
    group.throughput(Throughput::Elements(1000));
    group.sample_size(100);

    group.bench_function("concurrent_push_steal", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .bench_function("|_| concurrent_operations", |b| {
                let queue = Arc::new(WorkStealingQueueCapsule::new(16384).unwrap());

                // Owner thread: push items
                let queue_owner = Arc::clone(&queue);
                let owner_handle = thread::spawn(move || {
                    let queue_mut =
                        unsafe { &mut *(Arc::as_ptr(&queue_owner) as *mut WorkStealingQueueCapsule) };
                    for i in 0..1000 {
                        queue_mut.push(WorkItem::new(i, 10)).ok();
                    }
                });

                // Thief thread: steal items
                let queue_thief = Arc::clone(&queue);
                let thief_handle = thread::spawn(move || {
                    let mut count = 0;
                    for _ in 0..1000 {
                        if queue_thief.steal().is_some() {
                            count += 1;
                        }
                    }
                    count
                });

                owner_handle.join().unwrap();
                let _ = thief_handle.join().unwrap();

                b(());
            });
    });

    group.finish();
}

fn bench_multiple_thieves_contention(c: &mut Criterion) {
    // B32: Measure contention with multiple thieves
    // Expected: Slight slowdown with 4-8 thieves, minimal with CAS efficiency

    let mut group = c.benchmark_group("multiple_thieves");
    group.throughput(Throughput::Elements(1000));
    group.sample_size(50);

    for num_thieves in [1, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_thieves),
            num_thieves,
            |b, &num_thieves| {
                b.to_async(tokio::runtime::Runtime::new().unwrap())
                    .bench_function(format!("thieves_{}", num_thieves), move |b| {
                        let queue = Arc::new(WorkStealingQueueCapsule::new(16384).unwrap());

                        // Owner thread
                        let queue_owner = Arc::clone(&queue);
                        let owner_handle = thread::spawn(move || {
                            let queue_mut = unsafe {
                                &mut *(Arc::as_ptr(&queue_owner) as *mut WorkStealingQueueCapsule)
                            };
                            for i in 0..2000 {
                                queue_mut.push(WorkItem::new(i, 10)).ok();
                            }
                        });

                        // Thief threads
                        let mut thief_handles = vec![];
                        for _ in 0..num_thieves {
                            let queue_thief = Arc::clone(&queue);
                            let handle = thread::spawn(move || {
                                let mut count = 0;
                                for _ in 0..1000 {
                                    if queue_thief.steal().is_some() {
                                        count += 1;
                                    }
                                }
                                count
                            });
                            thief_handles.push(handle);
                        }

                        owner_handle.join().unwrap();
                        for handle in thief_handles {
                            handle.join().unwrap();
                        }

                        b(());
                    });
            },
        );
    }

    group.finish();
}

// ============================================================================
// LOAD BALANCE BENCHMARKS: Worker fairness
// ============================================================================

fn bench_load_balance_uniform(c: &mut Criterion) {
    // B32: Measure load balance with uniform batch sizes
    // Expected: <5% imbalance

    let mut group = c.benchmark_group("load_balance");
    group.sample_size(20);

    group.bench_function("uniform_batches", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .bench_function("|_| load_balance_uniform", |b| {
                let queue = Arc::new(WorkStealingQueueCapsule::new(8192).unwrap());

                // Owner: push 1600 uniform-sized batches
                let queue_owner = Arc::clone(&queue);
                let owner_handle = thread::spawn(move || {
                    let queue_mut =
                        unsafe { &mut *(Arc::as_ptr(&queue_owner) as *mut WorkStealingQueueCapsule) };
                    for i in 0..1600 {
                        queue_mut.push(WorkItem::new(i, 100)).ok();
                    }
                });

                thread::sleep(std::time::Duration::from_millis(10));

                // 16 workers: steal items
                let mut handles = vec![];
                let work_counts = Arc::new([std::sync::atomic::AtomicU64::new(0); 16]);

                for worker_id in 0..16 {
                    let queue_thief = Arc::clone(&queue);
                    let work_counts = Arc::clone(&work_counts);
                    let handle = thread::spawn(move || {
                        let mut count = 0;
                        for _ in 0..1000 {
                            if queue_thief.steal().is_some() {
                                count += 1;
                            }
                        }
                        work_counts[worker_id]
                            .fetch_add(count, std::sync::atomic::Ordering::Release);
                    });
                    handles.push(handle);
                }

                owner_handle.join().unwrap();
                for handle in handles {
                    handle.join().unwrap();
                }

                b(());
            });
    });

    group.finish();
}

// ============================================================================
// SCALING BENCHMARKS: How throughput scales with workers
// ============================================================================

fn bench_scaling_with_worker_count(c: &mut Criterion) {
    // B32: Measure how throughput scales from 1 to 16 workers
    // Expected: Linear scaling up to 8 workers, sub-linear 8-16

    let mut group = c.benchmark_group("scaling");
    group.sample_size(20);

    for num_workers in [1, 2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_workers),
            num_workers,
            |b, &num_workers| {
                b.to_async(tokio::runtime::Runtime::new().unwrap())
                    .bench_function(format!("workers_{}", num_workers), move |b| {
                        let queue = Arc::new(WorkStealingQueueCapsule::new(16384).unwrap());

                        // Owner: sustained push
                        let queue_owner = Arc::clone(&queue);
                        let push_handle = thread::spawn(move || {
                            let queue_mut = unsafe {
                                &mut *(Arc::as_ptr(&queue_owner) as *mut WorkStealingQueueCapsule)
                            };
                            let start = std::time::Instant::now();
                            let mut batch_id = 0u64;
                            while start.elapsed() < std::time::Duration::from_millis(100) {
                                queue_mut.push(WorkItem::new(batch_id, 10)).ok();
                                batch_id += 1;
                            }
                            batch_id
                        });

                        // Workers: steal items
                        let mut handles = vec![];
                        for _ in 0..num_workers {
                            let queue_thief = Arc::clone(&queue);
                            let handle = thread::spawn(move || {
                                let start = std::time::Instant::now();
                                let mut count = 0;
                                while start.elapsed() < std::time::Duration::from_millis(100) {
                                    if queue_thief.steal().is_some() {
                                        count += 1;
                                    }
                                }
                                count
                            });
                            handles.push(handle);
                        }

                        let total_pushed = push_handle.join().unwrap();
                        let mut total_stolen = 0;
                        for handle in handles {
                            total_stolen += handle.join().unwrap();
                        }

                        println!(
                            "{} workers: pushed={}, stolen={}",
                            num_workers, total_pushed, total_stolen
                        );

                        b(());
                    });
            },
        );
    }

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    benches,
    bench_push_single_thread,
    bench_pop_single_thread,
    bench_steal_single_thief,
    bench_is_empty_query,
    bench_stats_snapshot,
    bench_push_pop_throughput,
    bench_owner_vs_thief_contention,
    bench_multiple_thieves_contention,
    bench_load_balance_uniform,
    bench_scaling_with_worker_count,
);

criterion_main!(benches);
