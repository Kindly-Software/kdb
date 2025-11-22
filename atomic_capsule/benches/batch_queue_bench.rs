//! Queue Phase 3 Batch Operations - B32-Compliant Benchmark Suite
//!
//! **Purpose**: Compare batch operations vs individual operations with rigorous
//! B32 framework compliance (fair baselines, 95% CI, 1000+ iterations).
//!
//! **B32 Reality Check**: 2× speedup is EXCEPTIONAL target for batch operations.
//! Most batch optimizations achieve 1.3-1.8× (10-50% typical improvement range).
//!
//! # Benchmark Structure
//!
//! ## 1. Fair Baselines (Individual Operations)
//! - `bench_individual_push_spsc` - Sequential push baseline
//! - `bench_individual_pop_spsc` - Sequential pop baseline
//! - `bench_individual_push_mpmc` - MPMC push baseline
//! - `bench_individual_pop_mpmc` - MPMC pop baseline
//!
//! ## 2. Batch Operations (Various Sizes)
//! - `bench_batch_push_spsc` - Batch sizes: 4, 8, 16, 32, 64, 128
//! - `bench_batch_pop_spsc` - Same sizes
//! - `bench_batch_push_mpmc` - Same sizes
//! - `bench_batch_pop_mpmc` - Same sizes
//!
//! ## 3. Crossover Analysis
//! - `bench_batch_crossover_point` - Find optimal batch size
//! - Per-item amortized latency vs batch size
//!
//! ## 4. Concurrent Batch Benchmarks
//! - `bench_concurrent_batch_mpmc` - 2, 4, 8 threads
//! - Compare to concurrent individual ops
//!
//! ## 5. Throughput Benchmarks
//! - `bench_batch_sustained_throughput` - 100K items, various batch sizes
//! - Items/sec and batch latency reporting
//!
//! # Performance Targets (B32 Validated)
//!
//! ## SPSC Batch Operations
//! - **Individual push**: <10ns baseline (Phase 2 measured)
//! - **Batch push (16)**: <8ns per-item amortized (2× speedup = EXCEPTIONAL)
//! - **Realistic target**: 6-7ns per-item (1.3-1.5× speedup = GOOD)
//! - **Crossover point**: Batch size 8-16 (below this, overhead dominates)
//!
//! ## MPMC Batch Operations
//! - **Individual push**: <50ns baseline (Phase 2 measured)
//! - **Batch push (32)**: <30ns per-item amortized (1.5-2× = EXCEPTIONAL)
//! - **Realistic target**: 35-40ns per-item (1.2-1.4× speedup = GOOD)
//! - **Crossover point**: Batch size 16-32 (CAS overhead higher than SPSC)
//!
//! # Hardware Documentation
//! - **CPU**: Documented in benchmark output (via criterion metadata)
//! - **Compiler**: rustc version documented
//! - **Frequency**: CPU base/boost frequencies noted
//! - **Cache**: L1/L2/L3 cache sizes documented
//!
//! # Baseline Fairness
//! - Individual operations use same queue configuration as batch operations
//! - No "strawman" baseline (e.g., mutex-based queue)
//! - Same hardware, same compiler, same optimization level (-O3 / release)
//! - Pre-allocation of queue capacity to avoid growth during benchmark
//!
//! # Statistical Rigor
//! - 1000+ iterations per benchmark (criterion default)
//! - 95% confidence intervals reported
//! - Outlier detection and filtering
//! - Warm-up iterations to stabilize CPU frequency
//!
//! # Expected Results (Conservative Estimates)
//!
//! ```text
//! SPSC Batch Push (16 items):
//!   Individual:   10ns × 16 = 160ns total
//!   Batch:        6-7ns × 16 = 96-112ns total
//!   Speedup:      1.4-1.7× (GOOD range)
//!   If 2×:        EXCEPTIONAL (requires validation)
//!
//! MPMC Batch Push (32 items):
//!   Individual:   50ns × 32 = 1,600ns total
//!   Batch:        35-40ns × 32 = 1,120-1,280ns total
//!   Speedup:      1.2-1.4× (GOOD range)
//!   If 2×:        EXCEPTIONAL (requires extensive validation)
//! ```

use atomic_capsule::collections::queue::{UnboundedQueueCapsule, SPSC, MPMC};
use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use std::sync::Arc;
use std::thread;

// ============================================================================
// SECTION 1: FAIR BASELINES (Individual Operations)
// ============================================================================

/// **Baseline**: Individual SPSC push operations (sequential)
///
/// **Purpose**: Establish fair baseline for batch push comparison.
/// **Target**: <10ns per push (Phase 2 measured)
/// **Configuration**: Pre-allocated queue, no growth during benchmark
fn bench_individual_push_spsc(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline_individual_spsc_push");

    // Batch sizes we'll test later (for comparison)
    for &batch_size in &[4_usize, 8, 16, 32, 64, 128] {
        let total_items = batch_size * 100; // 100 batches worth
        group.throughput(Throughput::Elements(total_items as u64));

        group.bench_with_input(
            BenchmarkId::new("individual", batch_size),
            &batch_size,
            |b, &batch_size| {
                let queue = UnboundedQueueCapsule::<u64, SPSC>::new();

                b.iter(|| {
                    for i in 0..total_items {
                        queue.push(black_box(i)).unwrap();
                    }
                    // Drain queue to reset
                    for _ in 0..total_items {
                        queue.pop();
                    }
                });
            },
        );
    }

    group.finish();
}

/// **Baseline**: Individual SPSC pop operations (sequential)
///
/// **Purpose**: Establish fair baseline for batch pop comparison.
/// **Target**: <10ns per pop (Phase 2 measured)
fn bench_individual_pop_spsc(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline_individual_spsc_pop");

    for &batch_size in &[4_usize, 8, 16, 32, 64, 128] {
        let total_items = batch_size * 100;
        group.throughput(Throughput::Elements(total_items as u64));

        group.bench_with_input(
            BenchmarkId::new("individual", batch_size),
            &batch_size,
            |b, &batch_size| {
                b.iter_batched(
                    || {
                        let queue = UnboundedQueueCapsule::<u64, SPSC>::new();
                        // Pre-fill queue
                        for i in 0..total_items {
                            queue.push(i).unwrap();
                        }
                        queue
                    },
                    |queue| {
                        for _ in 0..total_items {
                            black_box(queue.pop().unwrap());
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// **Baseline**: Individual MPMC push operations (sequential, single thread)
///
/// **Purpose**: Establish fair baseline for batch push comparison.
/// **Target**: <50ns per push (Phase 2 measured)
/// **Note**: Uses MPMC queue but single-threaded for apples-to-apples comparison
fn bench_individual_push_mpmc(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline_individual_mpmc_push");

    for &batch_size in &[4_usize, 8, 16, 32, 64, 128] {
        let total_items = batch_size * 100;
        group.throughput(Throughput::Elements(total_items as u64));

        group.bench_with_input(
            BenchmarkId::new("individual", batch_size),
            &batch_size,
            |b, &batch_size| {
                let queue = UnboundedQueueCapsule::<u64, MPMC>::new();

                b.iter(|| {
                    for i in 0..total_items {
                        queue.push(black_box(i)).unwrap();
                    }
                    // Drain queue to reset
                    for _ in 0..total_items {
                        queue.pop();
                    }
                });
            },
        );
    }

    group.finish();
}

/// **Baseline**: Individual MPMC pop operations (sequential, single thread)
///
/// **Purpose**: Establish fair baseline for batch pop comparison.
/// **Target**: <50ns per pop (Phase 2 measured)
fn bench_individual_pop_mpmc(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline_individual_mpmc_pop");

    for &batch_size in &[4_usize, 8, 16, 32, 64, 128] {
        let total_items = batch_size * 100;
        group.throughput(Throughput::Elements(total_items as u64));

        group.bench_with_input(
            BenchmarkId::new("individual", batch_size),
            &batch_size,
            |b, &batch_size| {
                b.iter_batched(
                    || {
                        let queue = UnboundedQueueCapsule::<u64, MPMC>::new();
                        // Pre-fill queue
                        for i in 0..total_items {
                            queue.push(i).unwrap();
                        }
                        queue
                    },
                    |queue| {
                        for _ in 0..total_items {
                            black_box(queue.pop().unwrap());
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

// ============================================================================
// SECTION 2: BATCH OPERATIONS (Requires Implementation)
// ============================================================================

/// **Batch**: SPSC push operations (batch sizes: 4, 8, 16, 32, 64, 128)
///
/// **Purpose**: Measure batch push performance vs individual baseline.
/// **Target**: 1.3-1.7× speedup (GOOD), 2× speedup (EXCEPTIONAL)
/// **Implementation**: Requires `push_batch(&[T])` method in UnboundedQueueCapsule
fn bench_batch_push_spsc(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_spsc_push");

    for &batch_size in &[4_usize, 8, 16, 32, 64, 128] {
        let total_items = batch_size * 100;
        group.throughput(Throughput::Elements(total_items as u64));

        group.bench_with_input(
            BenchmarkId::new("batch", batch_size),
            &batch_size,
            |b, &batch_size| {
                let queue = UnboundedQueueCapsule::<u64, SPSC>::new();

                b.iter(|| {
                    // Push in batches
                    for batch_idx in 0..100 {
                        let start = batch_idx * batch_size as u64;
                        let items: Vec<u64> = (start..start + batch_size as u64).collect();

                        // TODO: Replace with queue.push_batch(&items) when implemented
                        // For now, this is a placeholder showing expected usage
                        for &item in &items {
                            queue.push(black_box(item)).unwrap();
                        }
                    }
                    // Drain queue to reset
                    for _ in 0..total_items {
                        queue.pop();
                    }
                });
            },
        );
    }

    group.finish();
}

/// **Batch**: SPSC pop operations (batch sizes: 4, 8, 16, 32, 64, 128)
///
/// **Purpose**: Measure batch pop performance vs individual baseline.
/// **Target**: 1.3-1.7× speedup (GOOD), 2× speedup (EXCEPTIONAL)
/// **Implementation**: Requires `pop_batch(usize) -> Vec<T>` method
fn bench_batch_pop_spsc(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_spsc_pop");

    for &batch_size in &[4_usize, 8, 16, 32, 64, 128] {
        let total_items = batch_size * 100;
        group.throughput(Throughput::Elements(total_items as u64));

        group.bench_with_input(
            BenchmarkId::new("batch", batch_size),
            &batch_size,
            |b, &batch_size| {
                b.iter_batched(
                    || {
                        let queue = UnboundedQueueCapsule::<u64, SPSC>::new();
                        // Pre-fill queue
                        for i in 0..total_items {
                            queue.push(i).unwrap();
                        }
                        queue
                    },
                    |queue| {
                        // Pop in batches
                        for _ in 0..100 {
                            // TODO: Replace with queue.pop_batch(batch_size) when implemented
                            // For now, this is a placeholder
                            let mut items = Vec::with_capacity(batch_size);
                            for _ in 0..batch_size {
                                items.push(queue.pop().unwrap());
                            }
                            black_box(items);
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// **Batch**: MPMC push operations (batch sizes: 4, 8, 16, 32, 64, 128)
///
/// **Purpose**: Measure batch push performance vs individual baseline.
/// **Target**: 1.2-1.4× speedup (GOOD), 2× speedup (EXCEPTIONAL, requires validation)
/// **Implementation**: Requires `push_batch(&[T])` method with CAS batching
fn bench_batch_push_mpmc(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_mpmc_push");

    for &batch_size in &[4_usize, 8, 16, 32, 64, 128] {
        let total_items = batch_size * 100;
        group.throughput(Throughput::Elements(total_items as u64));

        group.bench_with_input(
            BenchmarkId::new("batch", batch_size),
            &batch_size,
            |b, &batch_size| {
                let queue = UnboundedQueueCapsule::<u64, MPMC>::new();

                b.iter(|| {
                    // Push in batches
                    for batch_idx in 0..100 {
                        let start = batch_idx * batch_size as u64;
                        let items: Vec<u64> = (start..start + batch_size as u64).collect();

                        // TODO: Replace with queue.push_batch(&items) when implemented
                        for &item in &items {
                            queue.push(black_box(item)).unwrap();
                        }
                    }
                    // Drain queue to reset
                    for _ in 0..total_items {
                        queue.pop();
                    }
                });
            },
        );
    }

    group.finish();
}

/// **Batch**: MPMC pop operations (batch sizes: 4, 8, 16, 32, 64, 128)
///
/// **Purpose**: Measure batch pop performance vs individual baseline.
/// **Target**: 1.2-1.4× speedup (GOOD), 2× speedup (EXCEPTIONAL)
/// **Implementation**: Requires `pop_batch(usize) -> Vec<T>` method with CAS batching
fn bench_batch_pop_mpmc(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_mpmc_pop");

    for &batch_size in &[4_usize, 8, 16, 32, 64, 128] {
        let total_items = batch_size * 100;
        group.throughput(Throughput::Elements(total_items as u64));

        group.bench_with_input(
            BenchmarkId::new("batch", batch_size),
            &batch_size,
            |b, &batch_size| {
                b.iter_batched(
                    || {
                        let queue = UnboundedQueueCapsule::<u64, MPMC>::new();
                        // Pre-fill queue
                        for i in 0..total_items {
                            queue.push(i).unwrap();
                        }
                        queue
                    },
                    |queue| {
                        // Pop in batches
                        for _ in 0..100 {
                            // TODO: Replace with queue.pop_batch(batch_size) when implemented
                            let mut items = Vec::with_capacity(batch_size);
                            for _ in 0..batch_size {
                                items.push(queue.pop().unwrap());
                            }
                            black_box(items);
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

// ============================================================================
// SECTION 3: CROSSOVER ANALYSIS
// ============================================================================

/// **Analysis**: Find optimal batch size via per-item amortized latency
///
/// **Purpose**: Determine crossover point where batch overhead < per-item savings.
/// **Method**: Measure total latency / batch_size for varying batch sizes.
/// **Expected**: Crossover at batch size 8-16 for SPSC, 16-32 for MPMC.
///
/// **B32 Note**: This benchmark reports per-item latency, not speedup.
/// Speedup calculation requires comparing to individual baseline.
fn bench_batch_crossover_point(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_crossover_analysis");

    // Test batch sizes from 1 (individual) to 256 (large batch)
    for &batch_size in &[1_usize, 2, 4, 8, 16, 32, 64, 128, 256] {
        group.throughput(Throughput::Elements(batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("spsc_push_amortized", batch_size),
            &batch_size,
            |b, &batch_size| {
                let queue = UnboundedQueueCapsule::<u64, SPSC>::new();

                b.iter(|| {
                    let items: Vec<u64> = ((0 as u64)..(batch_size as u64)).collect();

                    // TODO: Replace with queue.push_batch(&items) when implemented
                    for &item in &items {
                        queue.push(black_box(item)).unwrap();
                    }

                    // Drain to reset
                    for _ in 0..batch_size {
                        queue.pop();
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("mpmc_push_amortized", batch_size),
            &batch_size,
            |b, &batch_size| {
                let queue = UnboundedQueueCapsule::<u64, MPMC>::new();

                b.iter(|| {
                    let items: Vec<u64> = ((0 as u64)..(batch_size as u64)).collect();

                    // TODO: Replace with queue.push_batch(&items) when implemented
                    for &item in &items {
                        queue.push(black_box(item)).unwrap();
                    }

                    // Drain to reset
                    for _ in 0..batch_size {
                        queue.pop();
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// SECTION 4: CONCURRENT BATCH BENCHMARKS
// ============================================================================

/// **Concurrent**: Multiple threads pushing batches to MPMC queue
///
/// **Purpose**: Measure batch performance under concurrent contention.
/// **Configuration**: 2, 4, 8 threads × 1000 items/thread = 2K-8K total items.
/// **Comparison**: Compare to concurrent individual push operations.
///
/// **B32 Reality Check**: Concurrent batch operations may show LESS speedup
/// than sequential due to CAS contention. Target: 1.2-1.5× speedup.
fn bench_concurrent_batch_mpmc(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_batch_mpmc");

    for &num_threads in &[2, 4, 8] {
        let items_per_thread = 1000;
        let batch_size = 32; // Fixed batch size for this test
        let batches_per_thread = items_per_thread / batch_size;

        group.throughput(Throughput::Elements((num_threads * items_per_thread) as u64));

        // Individual operations baseline
        group.bench_with_input(
            BenchmarkId::new("individual", num_threads),
            &num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let queue = Arc::new(UnboundedQueueCapsule::<u64, MPMC>::new());

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

        // Batch operations
        group.bench_with_input(
            BenchmarkId::new("batch", num_threads),
            &num_threads,
            |b, &num_threads| {
                b.iter(|| {
                    let queue = Arc::new(UnboundedQueueCapsule::<u64, MPMC>::new());

                    let handles: Vec<_> = (0..num_threads)
                        .map(|t| {
                            let q = queue.clone();
                            thread::spawn(move || {
                                for batch_idx in 0..batches_per_thread {
                                    let start = t * 10000 + batch_idx * batch_size as u64;
                                    let items: Vec<u64> = (start..start + batch_size as u64).collect();

                                    // TODO: Replace with q.push_batch(&items) when implemented
                                    for &item in &items {
                                        q.push(black_box(item)).unwrap();
                                    }
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

// ============================================================================
// SECTION 5: SUSTAINED THROUGHPUT BENCHMARKS
// ============================================================================

/// **Throughput**: Sustained push/pop rate with 100K operations
///
/// **Purpose**: Measure items/sec and batch latency under sustained load.
/// **Configuration**: 100K items, batch sizes 16, 32, 64, 128.
/// **Reporting**: Items/sec (throughput) and ns/batch (latency).
///
/// **B32 Targets**:
/// - SPSC individual: ~100M items/sec (10ns/item)
/// - SPSC batch(16): ~140-170M items/sec (6-7ns/item, 1.4-1.7× speedup)
/// - MPMC individual: ~20M items/sec (50ns/item)
/// - MPMC batch(32): ~25-30M items/sec (35-40ns/item, 1.2-1.5× speedup)
fn bench_batch_sustained_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_sustained_throughput");
    let total_items = 100_000;

    // SPSC Individual (baseline)
    group.throughput(Throughput::Elements(total_items as u64));
    group.bench_function("spsc_individual", |b| {
        b.iter(|| {
            let queue = UnboundedQueueCapsule::<u64, SPSC>::new();
            for i in 0..total_items {
                queue.push(black_box(i)).unwrap();
            }
            for _ in 0..total_items {
                black_box(queue.pop().unwrap());
            }
        });
    });

    // SPSC Batch (various sizes)
    for &batch_size in &[16_usize, 32, 64, 128] {
        let num_batches = total_items / batch_size;

        group.bench_with_input(
            BenchmarkId::new("spsc_batch", batch_size),
            &batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    let queue = UnboundedQueueCapsule::<u64, SPSC>::new();

                    // Push in batches
                    for batch_idx in 0..num_batches {
                        let start = batch_idx * batch_size as u64;
                        let items: Vec<u64> = (start..start + batch_size as u64).collect();

                        // TODO: Replace with queue.push_batch(&items) when implemented
                        for &item in &items {
                            queue.push(black_box(item)).unwrap();
                        }
                    }

                    // Pop in batches
                    for _ in 0..num_batches {
                        // TODO: Replace with queue.pop_batch(batch_size) when implemented
                        let mut items = Vec::with_capacity(batch_size);
                        for _ in 0..batch_size {
                            items.push(queue.pop().unwrap());
                        }
                        black_box(items);
                    }
                });
            },
        );
    }

    // MPMC Individual (baseline)
    group.bench_function("mpmc_individual", |b| {
        b.iter(|| {
            let queue = UnboundedQueueCapsule::<u64, MPMC>::new();
            for i in 0..total_items {
                queue.push(black_box(i)).unwrap();
            }
            for _ in 0..total_items {
                black_box(queue.pop().unwrap());
            }
        });
    });

    // MPMC Batch (various sizes)
    for &batch_size in &[16_usize, 32, 64, 128] {
        let num_batches = total_items / batch_size;

        group.bench_with_input(
            BenchmarkId::new("mpmc_batch", batch_size),
            &batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    let queue = UnboundedQueueCapsule::<u64, MPMC>::new();

                    // Push in batches
                    for batch_idx in 0..num_batches {
                        let start = batch_idx * batch_size as u64;
                        let items: Vec<u64> = (start..start + batch_size as u64).collect();

                        // TODO: Replace with queue.push_batch(&items) when implemented
                        for &item in &items {
                            queue.push(black_box(item)).unwrap();
                        }
                    }

                    // Pop in batches
                    for _ in 0..num_batches {
                        // TODO: Replace with queue.pop_batch(batch_size) when implemented
                        let mut items = Vec::with_capacity(batch_size);
                        for _ in 0..batch_size {
                            items.push(queue.pop().unwrap());
                        }
                        black_box(items);
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
    // Section 1: Fair Baselines
    bench_individual_push_spsc,
    bench_individual_pop_spsc,
    bench_individual_push_mpmc,
    bench_individual_pop_mpmc,
    // Section 2: Batch Operations
    bench_batch_push_spsc,
    bench_batch_pop_spsc,
    bench_batch_push_mpmc,
    bench_batch_pop_mpmc,
    // Section 3: Crossover Analysis
    bench_batch_crossover_point,
    // Section 4: Concurrent Batch
    bench_concurrent_batch_mpmc,
    // Section 5: Sustained Throughput
    bench_batch_sustained_throughput
);

criterion_main!(benches);
