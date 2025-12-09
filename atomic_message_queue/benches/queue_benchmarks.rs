use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use atomic_message_queue::{SPSCQueue, MessageBatch};
use std::sync::Arc;
use std::thread;
use std::sync::atomic::{AtomicU64, Ordering};

fn bench_single_threaded_ops(c: &mut Criterion) {
    let queue = SPSCQueue::<u64, 1024>::new();

    c.bench_function("push_single", |b| {
        b.iter(|| {
            for i in 0..100 {
                let _ = queue.push(black_box(i));
            }
            // Clear queue for next iteration
            while queue.pop().is_ok() {}
        });
    });

    // Fill queue first for pop benchmark
    for i in 0..512 {
        let _ = queue.push(i);
    }

    c.bench_function("pop_single", |b| {
        b.iter(|| {
            for _ in 0..100 {
                if queue.pop().is_err() {
                    // Refill when empty
                    for i in 0..512 {
                        let _ = queue.push(i);
                    }
                }
            }
        });
    });
}

fn bench_batch_operations(c: &mut Criterion) {
    let queue = SPSCQueue::<u64, 2048>::new();

    let batch_sizes = [1, 4, 16, 64, 256];

    for &batch_size in &batch_sizes {
        c.bench_with_input(
            BenchmarkId::new("batch_push", batch_size),
            &batch_size,
            |b, &size| {
                b.iter(|| {
                    let mut batch = MessageBatch::new(size);

                    // Fill batch
                    for i in 0..size {
                        batch.add(black_box(i as u64));
                    }

                    // Push to queue
                    let _pushed = batch.push_to_queue(&queue);

                    // Clear queue for next iteration
                    while queue.pop().is_ok() {}
                });
            },
        );
    }

    // Prepare queue with data for pop benchmarks
    for i in 0..1024 {
        let _ = queue.push(i);
    }

    for &batch_size in &batch_sizes {
        c.bench_with_input(
            BenchmarkId::new("batch_pop", batch_size),
            &batch_size,
            |b, &size| {
                b.iter(|| {
                    let mut batch = MessageBatch::new(size);
                    let _popped = batch.pop_from_queue(&queue);

                    // Refill queue when low
                    if queue.len() < 100 {
                        for i in 0..1024 {
                            let _ = queue.push(i);
                        }
                    }
                });
            },
        );
    }
}

fn bench_concurrent_throughput(c: &mut Criterion) {
    let capacities = [256, 1024, 4096];

    for &capacity in &capacities {
        c.bench_with_input(
            BenchmarkId::new("concurrent_throughput", capacity),
            &capacity,
            |b, &cap| {
                b.iter_custom(|iters| {
                    let start = std::time::Instant::now();

                    for _ in 0..iters {
                        run_concurrent_test(cap, 10000);
                    }

                    start.elapsed()
                });
            },
        );
    }
}

fn run_concurrent_test(capacity: usize, num_items: u64) {
    match capacity {
        256 => run_concurrent_test_typed::<256>(num_items),
        1024 => run_concurrent_test_typed::<1024>(num_items),
        4096 => run_concurrent_test_typed::<4096>(num_items),
        _ => panic!("Unsupported capacity"),
    }
}

fn run_concurrent_test_typed<const CAPACITY: usize>(num_items: u64) {
    let queue = Arc::new(SPSCQueue::<u64, CAPACITY>::new());
    let producer_queue = Arc::clone(&queue);
    let consumer_queue = Arc::clone(&queue);

    let producer = thread::spawn(move || {
        for i in 0..num_items {
            loop {
                match producer_queue.push(i) {
                    Ok(()) => break,
                    Err(_) => {
                        thread::yield_now();
                        continue;
                    }
                }
            }
        }
    });

    let consumer = thread::spawn(move || {
        let mut received = 0;

        while received < num_items {
            match consumer_queue.pop() {
                Ok(_) => received += 1,
                Err(_) => {
                    thread::yield_now();
                    continue;
                }
            }
        }

        received
    });

    producer.join().unwrap();
    let received = consumer.join().unwrap();
    assert_eq!(received, num_items);
}

fn bench_memory_ordering_comparison(c: &mut Criterion) {
    // This benchmark demonstrates the performance characteristics
    // of different memory orderings (for educational purposes)

    let counter = AtomicU64::new(0);

    c.bench_function("atomic_relaxed", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                counter.fetch_add(1, Ordering::Relaxed);
            }
        });
    });

    c.bench_function("atomic_acquire_release", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                counter.store(counter.load(Ordering::Acquire) + 1, Ordering::Release);
            }
        });
    });

    c.bench_function("atomic_seq_cst", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });
    });
}

fn bench_cache_effects(c: &mut Criterion) {
    // Benchmark to demonstrate cache line effects

    #[repr(align(64))]
    struct CacheAligned {
        value: AtomicU64,
    }

    #[repr(C)]
    struct NotAligned {
        value: AtomicU64,
    }

    let aligned = CacheAligned {
        value: AtomicU64::new(0),
    };

    let not_aligned = NotAligned {
        value: AtomicU64::new(0),
    };

    c.bench_function("cache_aligned_increment", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                aligned.value.fetch_add(1, black_box(Ordering::Relaxed));
            }
        });
    });

    c.bench_function("not_aligned_increment", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                not_aligned.value.fetch_add(1, black_box(Ordering::Relaxed));
            }
        });
    });
}

fn bench_queue_sizes(c: &mut Criterion) {
    // Compare performance across different queue sizes
    let sizes = [64, 256, 1024, 4096];

    for &size in &sizes {
        c.bench_with_input(
            BenchmarkId::new("queue_push_pop_cycle", size),
            &size,
            |b, &s| {
                b.iter_custom(|iters| {
                    let start = std::time::Instant::now();

                    for _ in 0..iters {
                        match s {
                            64 => run_push_pop_cycle::<64>(),
                            256 => run_push_pop_cycle::<256>(),
                            1024 => run_push_pop_cycle::<1024>(),
                            4096 => run_push_pop_cycle::<4096>(),
                            _ => panic!("Unsupported size"),
                        }
                    }

                    start.elapsed()
                });
            },
        );
    }
}

fn run_push_pop_cycle<const CAPACITY: usize>() {
    let queue = SPSCQueue::<u64, CAPACITY>::new();

    // Fill to half capacity
    let half_cap = CAPACITY / 2;
    for i in 0..half_cap {
        let _ = queue.push(i as u64);
    }

    // Push and pop alternately
    for i in 0..1000 {
        let _ = queue.push(black_box(i));
        let _ = queue.pop();
    }
}

criterion_group!(
    benches,
    bench_single_threaded_ops,
    bench_batch_operations,
    bench_concurrent_throughput,
    bench_memory_ordering_comparison,
    bench_cache_effects,
    bench_queue_sizes
);

criterion_main!(benches);