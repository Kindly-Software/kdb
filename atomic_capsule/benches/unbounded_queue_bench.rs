//! Unbounded Queue benchmarks - B32 framework validation
//!
//! Measures UnboundedQueueCapsule (SPSC/MPMC) performance:
//! - Sequential push/pop with automatic growth
//! - Per-operation latency (with and without growth)
//! - Growth overhead (segment allocation)
//! - Concurrent MPMC operations
//!
//! # Performance Targets (Phase 2)
//! - SPSC push (no growth): <10ns
//! - SPSC push (with growth): <1µs
//! - MPMC push (no growth): <50ns
//! - MPMC push (with growth): <2µs

use atomic_capsule::collections::queue::{UnboundedQueueCapsule, SPSC, MPMC};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;
use std::thread;

/// Sequential SPSC: Push/pop without growth (fits in initial segment)
fn bench_spsc_no_growth(c: &mut Criterion) {
    let mut group = c.benchmark_group("unbounded_spsc_no_growth");

    // Initial segment: 256 elements, 90% threshold = 230 items
    let size = 200; // Stays within initial segment
    group.throughput(Throughput::Elements(size as u64));

    group.bench_function("push_pop", |b| {
        let queue = UnboundedQueueCapsule::<u64, SPSC>::new();
        b.iter(|| {
            for i in 0..size {
                queue.push(black_box(i)).unwrap();
            }
            for _ in 0..size {
                black_box(queue.pop().unwrap());
            }
        });
    });

    group.finish();
}

/// Sequential SPSC: Push/pop with automatic growth (crosses segment boundaries)
fn bench_spsc_with_growth(c: &mut Criterion) {
    let mut group = c.benchmark_group("unbounded_spsc_with_growth");

    for size in [500, 2000, 10000] {
        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let queue = UnboundedQueueCapsule::<u64, SPSC>::new();
                for i in 0..size {
                    queue.push(black_box(i)).unwrap();
                }
                for _ in 0..size {
                    black_box(queue.pop().unwrap());
                }
            });
        });
    }

    group.finish();
}

/// SPSC latency: Single push/pop operation (no growth)
fn bench_spsc_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("unbounded_spsc_latency");
    group.throughput(Throughput::Elements(1));

    let queue = UnboundedQueueCapsule::<u64, SPSC>::new();

    // Pre-fill to avoid growth during benchmark
    for i in 0..100 {
        queue.push(i).unwrap();
    }
    for _ in 0..100 {
        queue.pop();
    }

    group.bench_function("push_pop_pair", |b| {
        b.iter(|| {
            queue.push(black_box(42)).unwrap();
            black_box(queue.pop().unwrap());
        });
    });

    group.finish();
}

/// Growth overhead: Measure segment allocation cost
fn bench_growth_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("unbounded_growth_overhead");

    // Push exactly to growth threshold, then measure next push (triggers allocation)
    group.bench_function("segment_allocation", |b| {
        b.iter_batched(
            || {
                let queue = UnboundedQueueCapsule::<u64, SPSC>::new();
                // Fill to 90% of 256 = 230 items (just before growth)
                for i in 0..230 {
                    queue.push(i).unwrap();
                }
                queue
            },
            |queue| {
                // This push triggers segment allocation
                queue.push(black_box(999)).unwrap();
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Sequential MPMC: Push/pop without growth
fn bench_mpmc_no_growth(c: &mut Criterion) {
    let mut group = c.benchmark_group("unbounded_mpmc_no_growth");

    let size = 200;
    group.throughput(Throughput::Elements(size as u64));

    group.bench_function("push_pop", |b| {
        let queue = UnboundedQueueCapsule::<u64, MPMC>::new();
        b.iter(|| {
            for i in 0..size {
                queue.push(black_box(i)).unwrap();
            }
            for _ in 0..size {
                black_box(queue.pop().unwrap());
            }
        });
    });

    group.finish();
}

/// Sequential MPMC: Push/pop with automatic growth
fn bench_mpmc_with_growth(c: &mut Criterion) {
    let mut group = c.benchmark_group("unbounded_mpmc_with_growth");

    for size in [500, 2000, 10000] {
        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let queue = UnboundedQueueCapsule::<u64, MPMC>::new();
                for i in 0..size {
                    queue.push(black_box(i)).unwrap();
                }
                for _ in 0..size {
                    black_box(queue.pop().unwrap());
                }
            });
        });
    }

    group.finish();
}

/// MPMC latency: Single push/pop operation
fn bench_mpmc_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("unbounded_mpmc_latency");
    group.throughput(Throughput::Elements(1));

    let queue = UnboundedQueueCapsule::<u64, MPMC>::new();

    // Pre-fill to avoid growth
    for i in 0..100 {
        queue.push(i).unwrap();
    }
    for _ in 0..100 {
        queue.pop();
    }

    group.bench_function("push_pop_pair", |b| {
        b.iter(|| {
            queue.push(black_box(42)).unwrap();
            black_box(queue.pop().unwrap());
        });
    });

    group.finish();
}

/// Concurrent MPMC: Multiple producers pushing to unbounded queue
fn bench_mpmc_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("unbounded_mpmc_concurrent");

    for num_threads in [2, 4, 8] {
        let items_per_thread = 1000;
        group.throughput(Throughput::Elements((num_threads * items_per_thread) as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            &num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let queue = Arc::new(UnboundedQueueCapsule::<u64, MPMC>::new());

                    // Producer threads
                    let handles: Vec<_> = (0..num_threads)
                        .map(|t| {
                            let q = queue.clone();
                            thread::spawn(move || {
                                for i in 0..items_per_thread {
                                    q.push(black_box(t * 10000 + i)).unwrap();
                                }
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }

                    // Drain queue
                    let mut count = 0;
                    while queue.pop().is_some() {
                        count += 1;
                    }
                    assert_eq!(count, num_threads * items_per_thread);
                });
            },
        );
    }

    group.finish();
}

/// Throughput test: Sustained push/pop rate (100K operations)
fn bench_sustained_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("unbounded_sustained_throughput");
    let size = 100_000;
    group.throughput(Throughput::Elements(size as u64));

    group.bench_function("spsc", |b| {
        b.iter(|| {
            let queue = UnboundedQueueCapsule::<u64, SPSC>::new();
            for i in 0..size {
                queue.push(black_box(i)).unwrap();
            }
            for _ in 0..size {
                black_box(queue.pop().unwrap());
            }
        });
    });

    group.bench_function("mpmc", |b| {
        b.iter(|| {
            let queue = UnboundedQueueCapsule::<u64, MPMC>::new();
            for i in 0..size {
                queue.push(black_box(i)).unwrap();
            }
            for _ in 0..size {
                black_box(queue.pop().unwrap());
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_spsc_no_growth,
    bench_spsc_with_growth,
    bench_spsc_latency,
    bench_growth_overhead,
    bench_mpmc_no_growth,
    bench_mpmc_with_growth,
    bench_mpmc_latency,
    bench_mpmc_concurrent,
    bench_sustained_throughput
);
criterion_main!(benches);
