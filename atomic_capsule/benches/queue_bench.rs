//! Queue benchmarks - B32 framework validation
//!
//! Compares QueueCapsule (SPSC/MPMC) against crossbeam-queue baselines.

use atomic_capsule::collections::queue::{QueueCapsule, SPSC, MPMC};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;
use std::thread;

fn bench_spsc_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("spsc_sequential");

    for size in [256, 1024, 4096] {
        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::new("QueueCapsule", size), &size, |b, &size| {
            let queue = QueueCapsule::<u64, SPSC>::new(size).unwrap();
            b.iter(|| {
                for i in 0..size {
                    queue.push(black_box(i as u64)).unwrap();
                }
                for _ in 0..size {
                    black_box(queue.pop().unwrap());
                }
            });
        });

        #[cfg(feature = "crossbeam")]
        group.bench_with_input(BenchmarkId::new("crossbeam", size), &size, |b, &size| {
            use crossbeam_queue::ArrayQueue;
            let queue = ArrayQueue::<u64>::new(size);
            b.iter(|| {
                for i in 0..size {
                    queue.push(black_box(i as u64)).unwrap();
                }
                for _ in 0..size {
                    black_box(queue.pop().unwrap());
                }
            });
        });
    }

    group.finish();
}

fn bench_mpmc_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("mpmc_sequential");

    for size in [256, 1024, 4096] {
        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::new("QueueCapsule", size), &size, |b, &size| {
            let queue = QueueCapsule::<u64, MPMC>::new(size).unwrap();
            b.iter(|| {
                for i in 0..size {
                    queue.push(black_box(i as u64)).unwrap();
                }
                for _ in 0..size {
                    black_box(queue.pop().unwrap());
                }
            });
        });

        #[cfg(feature = "crossbeam")]
        group.bench_with_input(BenchmarkId::new("crossbeam", size), &size, |b, &size| {
            use crossbeam_queue::ArrayQueue;
            let queue = ArrayQueue::<u64>::new(size);
            b.iter(|| {
                for i in 0..size {
                    queue.push(black_box(i as u64)).unwrap();
                }
                for _ in 0..size {
                    black_box(queue.pop().unwrap());
                }
            });
        });
    }

    group.finish();
}

fn bench_spsc_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("spsc_latency");
    group.throughput(Throughput::Elements(1));

    let queue = QueueCapsule::<u64, SPSC>::new(1024).unwrap();
    group.bench_function("push", |b| {
        b.iter(|| {
            let _ = queue.push(black_box(42));
            black_box(queue.pop());
        });
    });

    group.finish();
}

fn bench_mpmc_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("mpmc_latency");
    group.throughput(Throughput::Elements(1));

    let queue = QueueCapsule::<u64, MPMC>::new(1024).unwrap();
    group.bench_function("push", |b| {
        b.iter(|| {
            let _ = queue.push(black_box(42));
            black_box(queue.pop());
        });
    });

    group.finish();
}

fn bench_mpmc_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("mpmc_concurrent");

    for num_threads in [2, 4, 8] {
        group.throughput(Throughput::Elements((num_threads * 1000) as u64));

        group.bench_with_input(
            BenchmarkId::new("QueueCapsule", num_threads),
            &num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let queue = Arc::new(QueueCapsule::<u64, MPMC>::new(4096).unwrap());

                    let handles: Vec<_> = (0..num_threads)
                        .map(|t| {
                            let q = queue.clone();
                            thread::spawn(move || {
                                for i in 0..1000 {
                                    while q.push(black_box(t * 10000 + i)).is_err() {
                                        thread::yield_now();
                                    }
                                }
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }

                    // Drain queue
                    while queue.pop().is_some() {}
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_spsc_sequential,
    bench_mpmc_sequential,
    bench_spsc_latency,
    bench_mpmc_latency,
    bench_mpmc_concurrent
);
criterion_main!(benches);
