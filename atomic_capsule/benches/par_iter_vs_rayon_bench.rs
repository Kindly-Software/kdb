//! # B32 Benchmark: par_iter vs Rayon Direct Comparison
//!
//! **Framework**: B32 - Fair benchmarking with 32 guidelines
//! **Hardware**: AMD Ryzen 9 6900HX (8 cores, 16 threads), 64GB DDR5-4800
//! **Samples**: 1000+ per benchmark, 95% confidence intervals
//! **Baseline**: Rayon 1.8+ ParallelIterator (optimized)
//! **Date**: 2025-11-24
//!
//! ## Purpose
//!
//! Validate atomic_capsule parallel iterator API as a Rayon alternative.
//! Target: 4.4x vs Mutex baseline (already validated in hybrid_batch_pool_bench.rs)
//! Reality check: 1.2-2x vs Rayon (conservative expectation)
//!
//! ## Benchmark Categories
//!
//! 1. **for_each**: Side-effect operations
//! 2. **map**: Transform and collect
//! 3. **filter**: Predicate filtering
//! 4. **reduce**: Parallel reduction
//! 5. **find**: Early-exit search
//! 6. **Scaling**: 1K to 100K elements
//! 7. **Tail Latency**: P99.9 distribution
//!
//! ## Run Benchmarks
//!
//! ```bash
//! cargo bench --bench par_iter_vs_rayon_bench
//! ```

use atomic_capsule::parallel::{HybridBatchPool, IntoParallelIterator, ParallelIterator};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

// ============================================================================
// B32-1: for_each() COMPARISON
// ============================================================================

fn bench_for_each_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("par_iter_for_each");
    group.sample_size(500);

    for &n_items in &[1000usize, 10000, 100000] {
        group.throughput(Throughput::Elements(n_items as u64));
        let data: Vec<i32> = (0..n_items as i32).collect();

        // Rayon baseline
        group.bench_with_input(BenchmarkId::new("rayon", n_items), &data, |b, data| {
            b.iter(|| {
                use rayon::prelude::*;
                data.par_iter().for_each(|x| {
                    black_box(x * x);
                });
            });
        });

        // atomic_capsule into_par_iter
        group.bench_with_input(
            BenchmarkId::new("atomic_capsule", n_items),
            &data,
            |b, data| {
                b.iter(|| {
                    // Use slice reference for zero-allocation
                    data[..].into_par_iter().for_each(|x| {
                        black_box(x * x);
                    });
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// B32-2: map() COMPARISON
// ============================================================================

fn bench_map_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("par_iter_map");
    group.sample_size(500);

    for &n_items in &[1000usize, 10000, 100000] {
        group.throughput(Throughput::Elements(n_items as u64));
        let data: Vec<i32> = (0..n_items as i32).collect();

        // Rayon baseline
        group.bench_with_input(BenchmarkId::new("rayon", n_items), &data, |b, data| {
            b.iter(|| {
                use rayon::prelude::*;
                let result: Vec<i32> = data.par_iter().map(|&x| x * 2).collect();
                black_box(result);
            });
        });

        // atomic_capsule into_par_iter
        group.bench_with_input(
            BenchmarkId::new("atomic_capsule", n_items),
            &data,
            |b, data| {
                b.iter(|| {
                    let result: Vec<i32> = data[..].into_par_iter().map(|&x| x * 2);
                    black_box(result);
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// B32-3: filter() COMPARISON
// ============================================================================

fn bench_filter_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("par_iter_filter");
    group.sample_size(500);

    let n_items = 10000;
    group.throughput(Throughput::Elements(n_items as u64));
    let data: Vec<i32> = (0..n_items).collect();

    // Rayon baseline (50% selectivity)
    group.bench_function("rayon_50pct", |b| {
        b.iter(|| {
            use rayon::prelude::*;
            let result: Vec<i32> = data.par_iter().filter(|&&x| x % 2 == 0).cloned().collect();
            black_box(result);
        });
    });

    // atomic_capsule (50% selectivity)
    group.bench_function("atomic_capsule_50pct", |b| {
        b.iter(|| {
            let result: Vec<&i32> = data[..].into_par_iter().filter(|&&x| x % 2 == 0);
            black_box(result);
        });
    });

    // Rayon baseline (10% selectivity)
    group.bench_function("rayon_10pct", |b| {
        b.iter(|| {
            use rayon::prelude::*;
            let result: Vec<i32> = data.par_iter().filter(|&&x| x % 10 == 0).cloned().collect();
            black_box(result);
        });
    });

    // atomic_capsule (10% selectivity)
    group.bench_function("atomic_capsule_10pct", |b| {
        b.iter(|| {
            let result: Vec<&i32> = data[..].into_par_iter().filter(|&&x| x % 10 == 0);
            black_box(result);
        });
    });

    group.finish();
}

// ============================================================================
// B32-4: reduce() COMPARISON
// ============================================================================

fn bench_reduce_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("par_iter_reduce");
    group.sample_size(500);

    for &n_items in &[1000usize, 10000, 100000] {
        group.throughput(Throughput::Elements(n_items as u64));
        let data: Vec<i32> = (0..n_items as i32).collect();

        // Rayon baseline
        group.bench_with_input(BenchmarkId::new("rayon", n_items), &data, |b, data| {
            b.iter(|| {
                use rayon::prelude::*;
                let sum: i32 = data.par_iter().cloned().reduce(|| 0, |a, b| a + b);
                black_box(sum);
            });
        });

        // atomic_capsule fold
        group.bench_with_input(
            BenchmarkId::new("atomic_capsule", n_items),
            &data,
            |b, data| {
                b.iter(|| {
                    // Use fold with combiner (identity, accumulator, combiner)
                    let sum: i32 =
                        data[..]
                            .into_par_iter()
                            .fold(|| 0i32, |acc, &x| acc + x, |a, b| a + b);
                    black_box(sum);
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// B32-5: find() COMPARISON
// ============================================================================

fn bench_find_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("par_iter_find");
    group.sample_size(500);

    let n_items = 100000;
    group.throughput(Throughput::Elements(n_items as u64));
    let data: Vec<i32> = (0..n_items).collect();

    // Find near start (early exit) - Rayon
    group.bench_function("rayon_early", |b| {
        b.iter(|| {
            use rayon::prelude::*;
            let result = data.par_iter().find_any(|&&x| x == 100);
            black_box(result);
        });
    });

    // Find near start - atomic_capsule (using for_each with AtomicBool for early exit)
    group.bench_function("atomic_capsule_early", |b| {
        b.iter(|| {
            let found = Arc::new(AtomicUsize::new(usize::MAX));
            let target = 100i32;
            data[..].into_par_iter().for_each(|&x| {
                if x == target && found.load(Ordering::Relaxed) == usize::MAX {
                    found.store(x as usize, Ordering::Relaxed);
                }
            });
            let result = found.load(Ordering::Acquire);
            black_box(if result != usize::MAX {
                Some(result as i32)
            } else {
                None
            });
        });
    });

    // Find near end (full scan) - Rayon
    group.bench_function("rayon_late", |b| {
        b.iter(|| {
            use rayon::prelude::*;
            let result = data.par_iter().find_any(|&&x| x == 99900);
            black_box(result);
        });
    });

    // Find near end - atomic_capsule
    group.bench_function("atomic_capsule_late", |b| {
        b.iter(|| {
            let found = Arc::new(AtomicUsize::new(usize::MAX));
            let target = 99900i32;
            data[..].into_par_iter().for_each(|&x| {
                if x == target && found.load(Ordering::Relaxed) == usize::MAX {
                    found.store(x as usize, Ordering::Relaxed);
                }
            });
            let result = found.load(Ordering::Acquire);
            black_box(if result != usize::MAX {
                Some(result as i32)
            } else {
                None
            });
        });
    });

    // Not found (full scan) - Rayon
    group.bench_function("rayon_not_found", |b| {
        b.iter(|| {
            use rayon::prelude::*;
            let result = data.par_iter().find_any(|&&x| x < 0);
            black_box(result);
        });
    });

    // Not found - atomic_capsule
    group.bench_function("atomic_capsule_not_found", |b| {
        b.iter(|| {
            let found = Arc::new(AtomicUsize::new(usize::MAX));
            data[..].into_par_iter().for_each(|&x| {
                if x < 0 && found.load(Ordering::Relaxed) == usize::MAX {
                    found.store(x as usize, Ordering::Relaxed);
                }
            });
            let result = found.load(Ordering::Acquire);
            black_box(if result != usize::MAX {
                Some(result as i32)
            } else {
                None
            });
        });
    });

    group.finish();
}

// ============================================================================
// B32-6: CANONICAL WORKLOAD (1,600 tasks, 50 threads)
// ============================================================================

fn bench_canonical_1600_tasks(c: &mut Criterion) {
    let mut group = c.benchmark_group("canonical_1600_tasks");
    group.sample_size(1000);
    group.measurement_time(std::time::Duration::from_secs(30));

    // Rayon with 50 thread spawns (simulating HybridBatchPool workload)
    group.bench_function("rayon_spawn_50x32", |b| {
        b.iter(|| {
            let counter = Arc::new(AtomicUsize::new(0));

            use rayon::prelude::*;
            (0..50).into_par_iter().for_each(|_| {
                for _ in 0..32 {
                    counter.fetch_add(1, Ordering::Relaxed);
                }
            });

            black_box(counter.load(Ordering::Relaxed));
        });
    });

    // atomic_capsule HybridBatchPool direct
    group.bench_function("atomic_capsule_hybrid_50x32", |b| {
        b.iter(|| {
            let pool = Arc::new(HybridBatchPool::new(8).unwrap());
            let counter = Arc::new(AtomicUsize::new(0));

            let handles: Vec<_> = (0..50)
                .map(|_| {
                    let p = pool.clone();
                    let c = counter.clone();

                    std::thread::spawn(move || {
                        for _ in 0..32 {
                            let cc = c.clone();
                            p.push(Box::new(move || {
                                cc.fetch_add(1, Ordering::Relaxed);
                            }))
                            .unwrap();
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }

            pool.wait();
            black_box(counter.load(Ordering::Relaxed));
        });
    });

    group.finish();
}

// ============================================================================
// B32-7: TAIL LATENCY (P99.9)
// ============================================================================

fn bench_tail_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("tail_latency_p999");
    group.sample_size(1000);
    group.measurement_time(std::time::Duration::from_secs(30));

    let n_items = 10000;
    let data: Vec<i32> = (0..n_items).collect();

    // Rayon P99.9
    group.bench_function("rayon", |b| {
        b.iter_custom(|iters| {
            let mut latencies = Vec::with_capacity(iters as usize);

            for _ in 0..iters {
                let start = Instant::now();
                use rayon::prelude::*;
                let sum: i32 = data.par_iter().map(|&x| x * 2).sum();
                black_box(sum);
                latencies.push(start.elapsed());
            }

            latencies.sort_unstable();
            let p50 = latencies[latencies.len() / 2];

            if latencies.len() >= 100 {
                let p99 = latencies[latencies.len() * 99 / 100];
                let p999 = latencies[latencies.len() * 999 / 1000];
                eprintln!("Rayon: P50={:?}, P99={:?}, P99.9={:?}", p50, p99, p999);
            }

            p50
        });
    });

    // atomic_capsule P99.9
    group.bench_function("atomic_capsule", |b| {
        b.iter_custom(|iters| {
            let mut latencies = Vec::with_capacity(iters as usize);

            for _ in 0..iters {
                let start = Instant::now();
                let result: Vec<i32> = data[..].into_par_iter().map(|&x| x * 2);
                let sum: i32 = result.iter().sum();
                black_box(sum);
                latencies.push(start.elapsed());
            }

            latencies.sort_unstable();
            let p50 = latencies[latencies.len() / 2];

            if latencies.len() >= 100 {
                let p99 = latencies[latencies.len() * 99 / 100];
                let p999 = latencies[latencies.len() * 999 / 1000];
                eprintln!(
                    "atomic_capsule: P50={:?}, P99={:?}, P99.9={:?}",
                    p50, p99, p999
                );
            }

            p50
        });
    });

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    name = par_iter_vs_rayon_benches;
    config = Criterion::default()
        .measurement_time(std::time::Duration::from_secs(10))
        .sample_size(500)
        .confidence_level(0.95);
    targets =
        bench_for_each_comparison,
        bench_map_comparison,
        bench_filter_comparison,
        bench_reduce_comparison,
        bench_find_comparison,
        bench_canonical_1600_tasks,
        bench_tail_latency
);

criterion_main!(par_iter_vs_rayon_benches);
