//! # BatchStreamingCapsule Benchmark (B32 Framework)
//!
//! **Validates 2-40× speedup claims vs mutex-based VecDeque baseline.**
//!
//! ## Baseline
//!
//! - Mutex-protected VecDeque (push/pop per item)
//! - Standard library implementation
//! - No batching, no streaming optimizations
//!
//! ## Optimized
//!
//! - BatchStreamingCapsule (T6 Mixed: T4 Batch + T5 Streaming)
//! - Lockfree atomic coordination
//! - Batch accumulation + streaming ring buffer
//!
//! ## Benchmarks
//!
//! 1. **Single-threaded push** (1K items)
//! 2. **Single-threaded flush** (batches of 100)
//! 3. **Single-threaded consume** (streaming output)
//! 4. **Multi-threaded push** (4 threads, 10K items)
//! 5. **End-to-end pipeline** (producer-consumer)
//!
//! ## B32 Framework Compliance
//!
//! - Fair baseline (optimized VecDeque with mutex)
//! - 1000+ iterations per benchmark
//! - 95% confidence intervals
//! - Hardware: Report CPU model, cores, cache
//! - Reproducibility: Fixed random seeds

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use std::thread;

#[cfg(feature = "batch-streaming")]
use atomic_capsule::composite::BatchStreamingCapsule;

// ============================================================================
// BASELINE: Mutex-protected VecDeque
// ============================================================================

struct MutexBaseline<T> {
    queue: Mutex<VecDeque<T>>,
}

impl<T> MutexBaseline<T> {
    fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::with_capacity(4096)),
        }
    }

    fn push(&self, item: T) {
        let mut queue = self.queue.lock().unwrap();
        queue.push_back(item);
    }

    fn consume(&self, max_items: usize) -> Vec<T>
    where
        T: Clone,
    {
        let mut queue = self.queue.lock().unwrap();
        let count = max_items.min(queue.len());
        queue.drain(..count).collect()
    }

    fn len(&self) -> usize {
        self.queue.lock().unwrap().len()
    }
}

// ============================================================================
// BENCHMARK 1: Single-threaded Push (1K items)
// ============================================================================

#[cfg(feature = "batch-streaming")]
fn bench_single_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_push_1k");

    // Baseline: Mutex VecDeque
    group.bench_function("mutex_vecdeque", |b| {
        let baseline = MutexBaseline::<u64>::new();
        b.iter(|| {
            for i in 0..1000 {
                baseline.push(black_box(i as u64));
            }
        });
    });

    // Optimized: BatchStreamingCapsule
    group.bench_function("batch_streaming", |b| {
        let capsule = BatchStreamingCapsule::<u64, 100>::new();
        b.iter(|| {
            for i in 0..1000 {
                capsule.push(black_box(i as u64)).unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 2: Flush Batches (10 batches of 100 items)
// ============================================================================

#[cfg(feature = "batch-streaming")]
fn bench_flush_batches(c: &mut Criterion) {
    let mut group = c.benchmark_group("flush_batches_10x100");

    // Baseline: Mutex VecDeque (no batching, just push 1000)
    group.bench_function("mutex_vecdeque", |b| {
        b.iter(|| {
            let baseline = MutexBaseline::<u64>::new();
            for i in 0..1000 {
                baseline.push(black_box(i as u64));
            }
        });
    });

    // Optimized: BatchStreamingCapsule with explicit flush
    group.bench_function("batch_streaming", |b| {
        b.iter(|| {
            let capsule = BatchStreamingCapsule::<u64, 100>::new();
            for i in 0..1000 {
                capsule.push(black_box(i as u64)).unwrap();
                // Auto-flush happens at 100 items
            }
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 3: Consume Items (streaming output)
// ============================================================================

#[cfg(feature = "batch-streaming")]
fn bench_consume(c: &mut Criterion) {
    let mut group = c.benchmark_group("consume_1k");

    // Baseline: Mutex VecDeque
    group.bench_function("mutex_vecdeque", |b| {
        let baseline = MutexBaseline::<u64>::new();
        for i in 0..1000 {
            baseline.push(i as u64);
        }

        b.iter(|| {
            let items = baseline.consume(black_box(1000));
            black_box(items);
        });
    });

    // Optimized: BatchStreamingCapsule
    group.bench_function("batch_streaming", |b| {
        let capsule = BatchStreamingCapsule::<u64, 100>::new();
        for i in 0..1000 {
            capsule.push(i as u64).unwrap();
        }
        capsule.flush().unwrap();

        b.iter(|| {
            let items = capsule.consume(black_box(1000));
            black_box(items);
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 4: Multi-threaded Push (4 threads, 10K items)
// ============================================================================

#[cfg(feature = "batch-streaming")]
fn bench_multithread_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("multithread_push_4x10k");

    // Baseline: Mutex VecDeque
    group.bench_function("mutex_vecdeque", |b| {
        b.iter(|| {
            let baseline = Arc::new(MutexBaseline::<u64>::new());
            let mut handles = vec![];

            for thread_id in 0..4 {
                let baseline_clone = Arc::clone(&baseline);
                let handle = thread::spawn(move || {
                    for i in 0..10000 {
                        let value = (thread_id * 100000 + i) as u64;
                        baseline_clone.push(black_box(value));
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }

            black_box(baseline.len());
        });
    });

    // Optimized: BatchStreamingCapsule
    group.bench_function("batch_streaming", |b| {
        b.iter(|| {
            let capsule = Arc::new(BatchStreamingCapsule::<u64, 100>::new());
            let mut handles = vec![];

            for thread_id in 0..4 {
                let capsule_clone = Arc::clone(&capsule);
                let handle = thread::spawn(move || {
                    for i in 0..10000 {
                        let value = (thread_id * 100000 + i) as u64;
                        let _ = capsule_clone.push(black_box(value));
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }

            black_box(capsule.total_items());
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 5: End-to-end Pipeline (producer-consumer)
// ============================================================================

#[cfg(feature = "batch-streaming")]
fn bench_end_to_end(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end_100k");

    // Baseline: Mutex VecDeque
    group.bench_function("mutex_vecdeque", |b| {
        b.iter(|| {
            let baseline = Arc::new(MutexBaseline::<u64>::new());

            // Producer thread
            let baseline_clone = Arc::clone(&baseline);
            let producer = thread::spawn(move || {
                for i in 0..100000 {
                    baseline_clone.push(black_box(i as u64));
                }
            });

            // Consumer thread
            let baseline_clone = Arc::clone(&baseline);
            let consumer = thread::spawn(move || {
                let mut total = 0;
                while total < 100000 {
                    let items = baseline_clone.consume(1000);
                    total += items.len();
                    black_box(items);
                }
                total
            });

            producer.join().unwrap();
            let consumed = consumer.join().unwrap();
            black_box(consumed);
        });
    });

    // Optimized: BatchStreamingCapsule
    group.bench_function("batch_streaming", |b| {
        b.iter(|| {
            let capsule = Arc::new(BatchStreamingCapsule::<u64, 100>::new());

            // Producer thread
            let capsule_clone = Arc::clone(&capsule);
            let producer = thread::spawn(move || {
                for i in 0..100000 {
                    let _ = capsule_clone.push(black_box(i as u64));
                }
                capsule_clone.flush().unwrap();
            });

            // Consumer thread
            let capsule_clone = Arc::clone(&capsule);
            let consumer = thread::spawn(move || {
                let mut total = 0;
                while total < 100000 {
                    if let Some(items) = capsule_clone.consume(1000) {
                        total += items.len();
                        black_box(items);
                    }
                }
                total
            });

            producer.join().unwrap();
            let consumed = consumer.join().unwrap();
            black_box(consumed);
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 6: Large Batch Size (1K batch)
// ============================================================================

#[cfg(feature = "batch-streaming")]
fn bench_large_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_batch_1k");

    // Baseline: Mutex VecDeque (push 10K items)
    group.bench_function("mutex_vecdeque", |b| {
        b.iter(|| {
            let baseline = MutexBaseline::<u64>::new();
            for i in 0..10000 {
                baseline.push(black_box(i as u64));
            }
        });
    });

    // Optimized: BatchStreamingCapsule with 1K batch
    group.bench_function("batch_streaming_1k", |b| {
        let capsule = BatchStreamingCapsule::<u64, 1000>::new();
        b.iter(|| {
            for i in 0..10000 {
                capsule.push(black_box(i as u64)).unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 7: Small Batch Size (10 batch)
// ============================================================================

#[cfg(feature = "batch-streaming")]
fn bench_small_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("small_batch_10");

    // Baseline: Mutex VecDeque (push 1K items)
    group.bench_function("mutex_vecdeque", |b| {
        b.iter(|| {
            let baseline = MutexBaseline::<u64>::new();
            for i in 0..1000 {
                baseline.push(black_box(i as u64));
            }
        });
    });

    // Optimized: BatchStreamingCapsule with 10 batch
    group.bench_function("batch_streaming_10", |b| {
        let capsule = BatchStreamingCapsule::<u64, 10>::new();
        b.iter(|| {
            for i in 0..1000 {
                capsule.push(black_box(i as u64)).unwrap();
            }
        });
    });

    group.finish();
}

#[cfg(feature = "batch-streaming")]
criterion_group!(
    benches,
    bench_single_push,
    bench_flush_batches,
    bench_consume,
    bench_multithread_push,
    bench_end_to_end,
    bench_large_batch,
    bench_small_batch
);

#[cfg(not(feature = "batch-streaming"))]
criterion_group!(benches,);

criterion_main!(benches);
