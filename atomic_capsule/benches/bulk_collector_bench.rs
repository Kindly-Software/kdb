//! B32 Benchmarks for BulkCollectorCapsule
//!
//! Fair baselines:
//! - Compare to Mutex<Vec<T>>
//! - Same hardware, same workload
//! - 1000+ iterations, 95% CI

#![cfg(feature = "bulk-collector")]

use atomic_capsule::collections::BulkCollectorCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::{Arc, Mutex};
use std::thread;

// ============================================================================
// Append Benchmarks
// ============================================================================

fn bench_append_single_thread(c: &mut Criterion) {
    let mut group = c.benchmark_group("append_single_thread");

    // BulkCollectorCapsule (lockfree)
    group.bench_function("bulk_collector", |b| {
        let collector = BulkCollectorCapsule::<u64>::new(10_000);
        let mut counter = 0u64;

        b.iter(|| {
            collector.record(black_box(counter)).unwrap();
            counter += 1;
            if counter >= 10_000 {
                counter = 0;
                collector.reset();
            }
        });
    });

    // Mutex<Vec<T>> (baseline)
    group.bench_function("mutex_vec", |b| {
        let vec = Mutex::new(Vec::with_capacity(10_000));
        let mut counter = 0u64;

        b.iter(|| {
            vec.lock().unwrap().push(black_box(counter));
            counter += 1;
            if counter >= 10_000 {
                vec.lock().unwrap().clear();
                counter = 0;
            }
        });
    });

    group.finish();
}

fn bench_append_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("append_concurrent");

    for num_threads in [2, 4, 8] {
        // BulkCollectorCapsule (lockfree)
        group.bench_with_input(
            BenchmarkId::new("bulk_collector", num_threads),
            &num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let collector = Arc::new(BulkCollectorCapsule::<u64>::new(10_000));

                    let handles: Vec<_> = (0..num_threads)
                        .map(|_| {
                            let collector = Arc::clone(&collector);
                            thread::spawn(move || {
                                for i in 0..1_000 {
                                    collector.record(i).unwrap();
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    black_box(collector.len());
                });
            },
        );

        // Mutex<Vec<T>> (baseline)
        group.bench_with_input(
            BenchmarkId::new("mutex_vec", num_threads),
            &num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let vec = Arc::new(Mutex::new(Vec::with_capacity(10_000)));

                    let handles: Vec<_> = (0..num_threads)
                        .map(|_| {
                            let vec = Arc::clone(&vec);
                            thread::spawn(move || {
                                for i in 0..1_000 {
                                    vec.lock().unwrap().push(i);
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    black_box(vec.lock().unwrap().len());
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Export Benchmarks
// ============================================================================

fn bench_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("export");

    // BulkCollectorCapsule::export_arc (Arc clone)
    group.bench_function("bulk_collector_arc", |b| {
        let collector = BulkCollectorCapsule::<u64>::new(10_000);
        for i in 0..10_000 {
            collector.record(i).unwrap();
        }

        b.iter(|| {
            let arc = collector.export_arc();
            black_box(arc.len());
        });
    });

    // BulkCollectorCapsule::view (borrow)
    group.bench_function("bulk_collector_view", |b| {
        let collector = BulkCollectorCapsule::<u64>::new(10_000);
        for i in 0..10_000 {
            collector.record(i).unwrap();
        }

        b.iter(|| {
            let view = collector.view();
            black_box(view.len());
        });
    });

    // Mutex<Vec<T>>::extend(iter().copied()) (baseline)
    group.bench_function("mutex_vec_extend", |b| {
        let source = Mutex::new(Vec::with_capacity(10_000));
        for i in 0..10_000 {
            source.lock().unwrap().push(i);
        }

        b.iter(|| {
            let mut dest = Vec::with_capacity(10_000);
            dest.extend(source.lock().unwrap().iter().copied());
            black_box(dest.len());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_append_single_thread,
    bench_append_concurrent,
    bench_export
);
criterion_main!(benches);
