//! B32 Benchmarking for LockfreeList<T>
//!
//! **Fair Baseline**: Compare against Mutex<Vec<T>> (not strawman)
//! **Statistical Rigor**: 1000+ samples, 95% CI, Criterion
//! **Honest Reporting**: Document where LockfreeList wins AND loses

use atomic_capsule::parallel::LockfreeList;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::{Arc, Mutex};
use std::thread;

// ============================================================================
// BASELINE: Mutex<Vec<T>>
// ============================================================================

fn bench_mutex_vec_push_single_thread(c: &mut Criterion) {
    c.bench_function("mutex_vec/push/single_thread", |b| {
        let vec = Mutex::new(Vec::new());
        b.iter(|| {
            vec.lock().unwrap().push(black_box(42u64));
        });
    });
}

fn bench_mutex_vec_push_1000(c: &mut Criterion) {
    c.bench_function("mutex_vec/push/1000_items", |b| {
        b.iter(|| {
            let vec = Mutex::new(Vec::new());
            for i in 0..1000 {
                vec.lock().unwrap().push(black_box(i));
            }
        });
    });
}

fn bench_mutex_vec_concurrent_push_2_threads(c: &mut Criterion) {
    c.bench_function("mutex_vec/push/concurrent_2_threads", |b| {
        b.iter(|| {
            let vec = Arc::new(Mutex::new(Vec::new()));
            let mut handles = vec![];

            for i in 0..2 {
                let vec = Arc::clone(&vec);
                handles.push(thread::spawn(move || {
                    for j in 0..500 {
                        vec.lock().unwrap().push(black_box(i * 500 + j));
                    }
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });
}

fn bench_mutex_vec_concurrent_push_16_threads(c: &mut Criterion) {
    c.bench_function("mutex_vec/push/concurrent_16_threads", |b| {
        b.iter(|| {
            let vec = Arc::new(Mutex::new(Vec::new()));
            let mut handles = vec![];

            for i in 0..16 {
                let vec = Arc::clone(&vec);
                handles.push(thread::spawn(move || {
                    for j in 0..100 {
                        vec.lock().unwrap().push(black_box(i * 100 + j));
                    }
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });
}

fn bench_mutex_vec_iterate_1000(c: &mut Criterion) {
    let vec = Mutex::new((0..1000).collect::<Vec<u64>>());
    c.bench_function("mutex_vec/iterate/1000_items", |b| {
        b.iter(|| {
            let guard = vec.lock().unwrap();
            let mut sum = 0u64;
            for &val in guard.iter() {
                sum += black_box(val);
            }
            black_box(sum);
        });
    });
}

// ============================================================================
// LOCKFREE LIST BENCHMARKS
// ============================================================================

fn bench_lockfree_list_push_single_thread(c: &mut Criterion) {
    c.bench_function("lockfree_list/push/single_thread", |b| {
        let list: LockfreeList<u64> = LockfreeList::new();
        b.iter(|| {
            list.push(black_box(42u64));
        });
    });
}

fn bench_lockfree_list_push_1000(c: &mut Criterion) {
    c.bench_function("lockfree_list/push/1000_items", |b| {
        b.iter(|| {
            let list: LockfreeList<u64> = LockfreeList::new();
            for i in 0..1000 {
                list.push(black_box(i));
            }
        });
    });
}

fn bench_lockfree_list_concurrent_push_2_threads(c: &mut Criterion) {
    c.bench_function("lockfree_list/push/concurrent_2_threads", |b| {
        b.iter(|| {
            let list = Arc::new(LockfreeList::new());
            let mut handles = vec![];

            for i in 0..2 {
                let list = Arc::clone(&list);
                handles.push(thread::spawn(move || {
                    for j in 0..500 {
                        list.push(black_box(i * 500 + j));
                    }
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });
}

fn bench_lockfree_list_concurrent_push_16_threads(c: &mut Criterion) {
    c.bench_function("lockfree_list/push/concurrent_16_threads", |b| {
        b.iter(|| {
            let list = Arc::new(LockfreeList::new());
            let mut handles = vec![];

            for i in 0..16 {
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
    });
}

fn bench_lockfree_list_iterate_1000(c: &mut Criterion) {
    let list: LockfreeList<u64> = LockfreeList::new();
    for i in 0..1000 {
        list.push(i);
    }

    c.bench_function("lockfree_list/iterate/1000_items", |b| {
        b.iter(|| {
            let mut sum = 0u64;
            for &val in list.iter() {
                sum += black_box(val);
            }
            black_box(sum);
        });
    });
}

// ============================================================================
// SCALING BENCHMARKS
// ============================================================================

fn bench_push_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("push_scaling");

    for size in [100, 1_000, 10_000, 100_000] {
        group.bench_with_input(
            BenchmarkId::new("lockfree_list", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let list: LockfreeList<u64> = LockfreeList::new();
                    for i in 0..size {
                        list.push(black_box(i as u64));
                    }
                });
            },
        );

        group.bench_with_input(BenchmarkId::new("mutex_vec", size), &size, |b, &size| {
            b.iter(|| {
                let vec = Mutex::new(Vec::new());
                for i in 0..size {
                    vec.lock().unwrap().push(black_box(i as u64));
                }
            });
        });
    }

    group.finish();
}

fn bench_concurrent_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_scaling");

    for threads in [2, 4, 8, 16] {
        group.bench_with_input(
            BenchmarkId::new("lockfree_list", threads),
            &threads,
            |b, &threads| {
                b.iter(|| {
                    let list = Arc::new(LockfreeList::new());
                    let mut handles = vec![];

                    for i in 0..threads {
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

        group.bench_with_input(
            BenchmarkId::new("mutex_vec", threads),
            &threads,
            |b, &threads| {
                b.iter(|| {
                    let vec = Arc::new(Mutex::new(Vec::new()));
                    let mut handles = vec![];

                    for i in 0..threads {
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
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    benches,
    // Mutex baseline
    bench_mutex_vec_push_single_thread,
    bench_mutex_vec_push_1000,
    bench_mutex_vec_concurrent_push_2_threads,
    bench_mutex_vec_concurrent_push_16_threads,
    bench_mutex_vec_iterate_1000,
    // LockfreeList
    bench_lockfree_list_push_single_thread,
    bench_lockfree_list_push_1000,
    bench_lockfree_list_concurrent_push_2_threads,
    bench_lockfree_list_concurrent_push_16_threads,
    bench_lockfree_list_iterate_1000,
    // Scaling
    bench_push_scaling,
    bench_concurrent_scaling,
);

criterion_main!(benches);
