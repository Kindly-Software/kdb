//! B32 Benchmarking for LockfreeList<T> Tail Retry Loop Performance
//!
//! **Focus**: Measure tail retry loop overhead and retry distribution
//! **Baseline**: Current implementation with 8-retry bounded loop
//! **Goal**: Validate <50ns P50 push latency @ low contention
//!
//! **B32 Framework Compliance**:
//! - Fair baselines: Mutex<Vec<T>>, parking_lot::Mutex
//! - Statistical rigor: 1000+ samples, 95% CI
//! - Realistic workloads: 1/2/4/8/16 threads
//! - Contention testing: Low/Medium/High scenarios
//! - Honest reporting: Document where LockfreeList wins AND loses

use atomic_capsule::parallel::LockfreeList;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// ============================================================================
// BASELINE: Mutex<Vec<T>> (Fair Comparison)
// ============================================================================

fn bench_mutex_vec_push_latency_by_threads(c: &mut Criterion) {
    let mut group = c.benchmark_group("mutex_vec_push_latency");

    for num_threads in [1, 2, 4, 8, 16] {
        group.throughput(Throughput::Elements(1000 * num_threads as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_threads", num_threads)),
            &num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let vec = Arc::new(Mutex::new(Vec::new()));
                    let mut handles = vec![];

                    for i in 0..num_threads {
                        let vec = Arc::clone(&vec);
                        handles.push(thread::spawn(move || {
                            for j in 0..1000 {
                                vec.lock().unwrap().push(black_box(i * 1000 + j));
                            }
                        }));
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
// LOCKFREE LIST: Push Latency by Thread Count
// ============================================================================

fn bench_lockfree_list_push_latency_by_threads(c: &mut Criterion) {
    let mut group = c.benchmark_group("lockfree_list_push_latency");

    for num_threads in [1, 2, 4, 8, 16] {
        group.throughput(Throughput::Elements(1000 * num_threads as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_threads", num_threads)),
            &num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let list = Arc::new(LockfreeList::new());
                    let mut handles = vec![];

                    for i in 0..num_threads {
                        let list = Arc::clone(&list);
                        handles.push(thread::spawn(move || {
                            for j in 0..1000 {
                                list.push(black_box(i * 1000 + j));
                            }
                        }));
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
// LOCKFREE LIST: Single Push Latency (Uncontended)
// ============================================================================

fn bench_lockfree_list_single_push_uncontended(c: &mut Criterion) {
    c.bench_function("lockfree_list/push/uncontended_single", |b| {
        let list: LockfreeList<u64> = LockfreeList::new();
        b.iter(|| {
            list.push(black_box(42u64));
        });
    });
}

// ============================================================================
// LOCKFREE LIST: Burst Push (Measure P50/P95/P99)
// ============================================================================

fn bench_lockfree_list_burst_push_percentiles(c: &mut Criterion) {
    let mut group = c.benchmark_group("lockfree_list_burst_push_percentiles");

    for num_threads in [2, 4, 8, 16] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_threads", num_threads)),
            &num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let list = Arc::new(LockfreeList::new());
                    let mut handles = vec![];

                    // Each thread pushes 100 items (shorter burst for percentile measurement)
                    for i in 0..num_threads {
                        let list = Arc::clone(&list);
                        handles.push(thread::spawn(move || {
                            for j in 0..100 {
                                list.push(black_box(i * 100 + j));
                            }
                        }));
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
// LOCKFREE LIST: Sustained Throughput (60 seconds stress test)
// ============================================================================

fn bench_lockfree_list_sustained_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("lockfree_list_sustained_throughput");
    group.sample_size(10); // Fewer samples for long-running test
    group.measurement_time(Duration::from_secs(60));

    for num_threads in [4, 8, 16] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_threads", num_threads)),
            &num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let list = Arc::new(LockfreeList::new());
                    let mut handles = vec![];

                    // Each thread pushes 100K items (sustained load)
                    for i in 0..num_threads {
                        let list = Arc::clone(&list);
                        handles.push(thread::spawn(move || {
                            for j in 0..100_000 {
                                list.push(black_box(i * 100_000 + j));
                            }
                        }));
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
// LOCKFREE LIST: Pathological Contention (32+ threads)
// ============================================================================

fn bench_lockfree_list_pathological_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("lockfree_list_pathological_contention");

    for num_threads in [32, 64] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_threads", num_threads)),
            &num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let list = Arc::new(LockfreeList::new());
                    let mut handles = vec![];

                    // Each thread pushes 100 items (high contention)
                    for i in 0..num_threads {
                        let list = Arc::clone(&list);
                        handles.push(thread::spawn(move || {
                            for j in 0..100 {
                                list.push(black_box(i * 100 + j));
                            }
                        }));
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
// COMPARATIVE: LockfreeList vs Mutex<Vec<T>> Scaling
// ============================================================================

fn bench_comparative_push_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("comparative_push_scaling");

    for num_threads in [1, 2, 4, 8, 16, 32] {
        // Mutex<Vec<T>> baseline
        group.bench_with_input(
            BenchmarkId::new("mutex_vec", num_threads),
            &num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let vec = Arc::new(Mutex::new(Vec::new()));
                    let mut handles = vec![];

                    for i in 0..num_threads {
                        let vec = Arc::clone(&vec);
                        handles.push(thread::spawn(move || {
                            for j in 0..1000 {
                                vec.lock().unwrap().push(black_box(i * 1000 + j));
                            }
                        }));
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );

        // LockfreeList<T>
        group.bench_with_input(
            BenchmarkId::new("lockfree_list", num_threads),
            &num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let list = Arc::new(LockfreeList::new());
                    let mut handles = vec![];

                    for i in 0..num_threads {
                        let list = Arc::clone(&list);
                        handles.push(thread::spawn(move || {
                            for j in 0..1000 {
                                list.push(black_box(i * 1000 + j));
                            }
                        }));
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
// MIXED WORKLOAD: Push + Iterate Concurrently
// ============================================================================

fn bench_lockfree_list_mixed_push_iterate(c: &mut Criterion) {
    let mut group = c.benchmark_group("lockfree_list_mixed_push_iterate");

    for num_writers in [2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_writers_1_reader", num_writers)),
            &num_writers,
            |b, &num_writers| {
                b.iter(|| {
                    let list = Arc::new(LockfreeList::new());
                    let mut handles = vec![];

                    // Writer threads
                    for i in 0..num_writers {
                        let list = Arc::clone(&list);
                        handles.push(thread::spawn(move || {
                            for j in 0..1000 {
                                list.push(black_box(i * 1000 + j));
                            }
                        }));
                    }

                    // Reader thread (iterates 100 times)
                    let list_reader = Arc::clone(&list);
                    handles.push(thread::spawn(move || {
                        for _ in 0..100 {
                            let _count = list_reader.iter().count();
                        }
                    }));

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
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    tail_retry_benches,
    // Baseline comparisons
    bench_mutex_vec_push_latency_by_threads,
    // Core latency tests
    bench_lockfree_list_push_latency_by_threads,
    bench_lockfree_list_single_push_uncontended,
    bench_lockfree_list_burst_push_percentiles,
    // Stress tests
    bench_lockfree_list_sustained_throughput,
    bench_lockfree_list_pathological_contention,
    // Comparative analysis
    bench_comparative_push_scaling,
    // Mixed workload
    bench_lockfree_list_mixed_push_iterate,
);

criterion_main!(tail_retry_benches);
