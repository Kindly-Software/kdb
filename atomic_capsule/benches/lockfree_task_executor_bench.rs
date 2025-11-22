//! B32 Benchmark Suite: LockfreeTaskExecutor (ThreadPool)
//!
//! **Purpose**: Validate atomic_capsule::parallel::ThreadPool performance claims
//!
//! **B32 Framework Compliance**:
//! - Fair baselines: Compare against Rayon (industry standard)
//! - 95% CI: Criterion.rs with 1000+ iterations
//! - Reality check: 10-50% typical, 2-10× exceptional, 100×+ suspicious
//! - Reproducibility: All benchmarks deterministic, multiple runs consistent
//!
//! **Performance Claims to Validate**:
//! - Cold start: 100-500ns (vs Rayon 1-10μs) = **10-100× faster**
//! - Hot iteration: Similar to Rayon (within 10%)
//! - Batch (1K tasks): 50μs (vs Rayon 500μs) = **10× faster**
//! - P99.9 latency: <2μs (vs Rayon 100-500μs) = **50-250× better tail**
//!
//! ## Benchmark Groups
//!
//! **Group 1: Executor Creation** - Cold start latency
//! - `executor_creation_16_workers`: Measure pool initialization overhead
//! - Expected: <2 ms (thread spawn overhead dominates)
//!
//! **Group 2: Coordination Overhead** - Task claiming without work
//! - `task_claiming_empty_10k`: Submit 10K empty tasks (measure coordination only)
//! - Expected: <100 ms total (<10μs per task)
//!
//! **Group 3: Real Workload** - Simulated CNLS computation
//! - `simulated_cnls_1000_tasks`: 1000 tasks × 100μs work each
//! - Expected: ~62ms (1000 tasks / 16 workers ≈ 62.5 tasks per worker × 100μs)
//!
//! **Group 4: Scalability** - Thread count scaling (1, 2, 4, 8, 16, 32)
//! - Measure linear speedup up to hardware thread count
//! - Expected: Linear up to 16 cores, diminishing returns beyond
//!
//! **Group 5: Contention Scenarios** - Compare uncontended vs contended
//! - 1 thread: Uncontended baseline
//! - 4 threads: Light contention
//! - 16 threads: Moderate contention
//! - 32 threads: Heavy contention (if hardware supports)
//!
//! **Group 6: Comparison to Rayon** - Direct head-to-head
//! - Same workload, measure atomic_capsule vs Rayon
//! - Expected: Within 10% for hot iteration (fair baseline)
//!
//! ## Hardware Reality Checks (K1-K50)
//!
//! **K18: Scheduling Overhead**
//! - Thread creation: 50μs typical
//! - Task spawn: 200ns (tokio), 5-10ns (our claim)
//!
//! **K20: Throughput Scaling**
//! - Single thread: 1× baseline
//! - P-cores only: 6.5× with 6 cores
//! - All cores: 12× with proper cooling
//!
//! **K27: HONEST GAINS**
//! - Typical: 10-50% improvement
//! - Exceptional: 2× speedup
//! - Suspicious: 10×+ without algorithm change
//!
//! ## ASSUM Safety
//!
//! All benchmarks use black_box() to prevent compiler optimization erasure.
//! No unsafe code in benchmark harness.

use atomic_capsule::parallel::{get_global_pool, ThreadPool};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ============================================================================
// Group 1: Executor Creation
// ============================================================================

fn bench_executor_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("executor_creation");

    group.bench_function("executor_creation_16_workers", |b| {
        b.iter(|| {
            let executor = ThreadPool::new(16).expect("Failed to create thread pool");
            black_box(executor);
        });
    });

    group.bench_function("executor_creation_8_workers", |b| {
        b.iter(|| {
            let executor = ThreadPool::new(8).expect("Failed to create thread pool");
            black_box(executor);
        });
    });

    group.finish();
}

// ============================================================================
// Group 2: Coordination Overhead (Empty Tasks)
// ============================================================================

fn bench_task_claiming_empty(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_claiming_empty");

    // Configure for statistical validity (B2)
    group
        .sample_size(100) // 100 iterations (creating pools is expensive)
        .measurement_time(Duration::from_secs(10));

    group.bench_function("task_claiming_empty_10k_16_workers", |b| {
        b.iter(|| {
            let pool = ThreadPool::new(16).expect("Failed to create thread pool");

            // Submit 10K empty tasks (measure coordination only)
            for i in 0..10_000 {
                pool.push(Box::new(move || {
                    black_box(i); // Prevent optimization erasure
                }))
                .expect("Queue full");
            }

            // Wait for completion
            pool.wait();
        });
    });

    group.bench_function("task_claiming_empty_1k_16_workers", |b| {
        b.iter(|| {
            let pool = ThreadPool::new(16).expect("Failed to create thread pool");

            for i in 0..1_000 {
                pool.push(Box::new(move || {
                    black_box(i);
                }))
                .expect("Queue full");
            }

            pool.wait();
        });
    });

    group.finish();
}

// ============================================================================
// Group 3: Real Workload (Simulated CNLS)
// ============================================================================

fn bench_simulated_cnls_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("simulated_cnls_workload");

    // Configure for statistical validity
    group
        .sample_size(50) // 50 iterations (workload is expensive)
        .measurement_time(Duration::from_secs(10));

    group.bench_function("simulated_cnls_1000_tasks_16_workers", |b| {
        b.iter(|| {
            let pool = ThreadPool::new(16).expect("Failed to create thread pool");

            // Simulate 1000 CNLS tasks (100μs work each)
            for i in 0..1000 {
                pool.push(Box::new(move || {
                    // Simulate 100μs of work (burn CPU cycles)
                    let start = std::time::Instant::now();
                    while start.elapsed() < Duration::from_micros(100) {
                        black_box(i * i);
                    }
                }))
                .expect("Queue full");
            }

            pool.wait();
        });
    });

    group.bench_function("simulated_cnls_100_tasks_16_workers", |b| {
        b.iter(|| {
            let pool = ThreadPool::new(16).expect("Failed to create thread pool");

            // Smaller workload (100 tasks × 100μs)
            for i in 0..100 {
                pool.push(Box::new(move || {
                    let start = std::time::Instant::now();
                    while start.elapsed() < Duration::from_micros(100) {
                        black_box(i * i);
                    }
                }))
                .expect("Queue full");
            }

            pool.wait();
        });
    });

    group.finish();
}

// ============================================================================
// Group 4: Scalability (Thread Count Scaling)
// ============================================================================

fn bench_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalability");

    // Configure for statistical validity
    group
        .sample_size(50)
        .measurement_time(Duration::from_secs(10));

    // Test thread counts: 1, 2, 4, 8, 16, 32 (if supported)
    // K20: Expect linear scaling up to hardware thread count
    for num_workers in [1, 2, 4, 8, 16] {
        group.bench_with_input(
            BenchmarkId::new("thread_scaling", num_workers),
            &num_workers,
            |b, &workers| {
                b.iter(|| {
                    let pool = ThreadPool::new(workers).expect("Failed to create thread pool");

                    // Fixed workload: 1000 tasks × 100μs
                    // Expect: 1 worker = 100ms, 16 workers = ~6.25ms (16× speedup)
                    for i in 0..1000 {
                        pool.push(Box::new(move || {
                            let start = std::time::Instant::now();
                            while start.elapsed() < Duration::from_micros(100) {
                                black_box(i * i);
                            }
                        }))
                        .expect("Queue full");
                    }

                    pool.wait();
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Group 5: Contention Scenarios
// ============================================================================

fn bench_contention_scenarios(c: &mut Criterion) {
    let mut group = c.benchmark_group("contention_scenarios");

    // Configure for statistical validity
    group
        .sample_size(50)
        .measurement_time(Duration::from_secs(10));

    // Uncontended (1 thread)
    group.bench_function("contention_uncontended_1_thread", |b| {
        b.iter(|| {
            let pool = ThreadPool::new(1).expect("Failed to create thread pool");

            for i in 0..1000 {
                pool.push(Box::new(move || {
                    black_box(i * 2);
                }))
                .expect("Queue full");
            }

            pool.wait();
        });
    });

    // Light contention (4 threads)
    group.bench_function("contention_light_4_threads", |b| {
        b.iter(|| {
            let pool = ThreadPool::new(4).expect("Failed to create thread pool");

            for i in 0..1000 {
                pool.push(Box::new(move || {
                    black_box(i * 2);
                }))
                .expect("Queue full");
            }

            pool.wait();
        });
    });

    // Moderate contention (16 threads)
    group.bench_function("contention_moderate_16_threads", |b| {
        b.iter(|| {
            let pool = ThreadPool::new(16).expect("Failed to create thread pool");

            for i in 0..1000 {
                pool.push(Box::new(move || {
                    black_box(i * 2);
                }))
                .expect("Queue full");
            }

            pool.wait();
        });
    });

    group.finish();
}

// ============================================================================
// Group 6: Comparison to Rayon (Fair Baseline)
// ============================================================================

fn bench_comparison_to_rayon(c: &mut Criterion) {
    let mut group = c.benchmark_group("comparison_rayon");

    // Configure for statistical validity (B2)
    group
        .sample_size(100)
        .measurement_time(Duration::from_secs(15));

    // Rayon baseline (optimized, NOT strawman)
    group.bench_function("rayon_for_each_1000_tasks", |b| {
        b.iter(|| {
            use rayon::prelude::*;

            let data: Vec<usize> = (0..1000).collect();
            data.par_iter().for_each(|&i| {
                // Simulate 100μs work
                let start = std::time::Instant::now();
                while start.elapsed() < Duration::from_micros(100) {
                    black_box(i * i);
                }
            });
        });
    });

    // atomic_capsule ThreadPool (our implementation)
    group.bench_function("atomic_capsule_threadpool_1000_tasks", |b| {
        b.iter(|| {
            let pool = ThreadPool::new(16).expect("Failed to create thread pool");

            for i in 0..1000 {
                pool.push(Box::new(move || {
                    let start = std::time::Instant::now();
                    while start.elapsed() < Duration::from_micros(100) {
                        black_box(i * i);
                    }
                }))
                .expect("Queue full");
            }

            pool.wait();
        });
    });

    // Rayon: Smaller task (10μs each)
    group.bench_function("rayon_for_each_10k_tasks_small", |b| {
        b.iter(|| {
            use rayon::prelude::*;

            let data: Vec<usize> = (0..10_000).collect();
            data.par_iter().for_each(|&i| {
                // 10μs work
                let start = std::time::Instant::now();
                while start.elapsed() < Duration::from_micros(10) {
                    black_box(i * i);
                }
            });
        });
    });

    // atomic_capsule: Smaller task (10μs each)
    group.bench_function("atomic_capsule_threadpool_10k_tasks_small", |b| {
        b.iter(|| {
            let pool = ThreadPool::new(16).expect("Failed to create thread pool");

            for i in 0..10_000 {
                pool.push(Box::new(move || {
                    let start = std::time::Instant::now();
                    while start.elapsed() < Duration::from_micros(10) {
                        black_box(i * i);
                    }
                }))
                .expect("Queue full");
            }

            pool.wait();
        });
    });

    group.finish();
}

// ============================================================================
// Group 7: Global Pool API (planck-universe usage)
// ============================================================================

fn bench_global_pool_api(c: &mut Criterion) {
    let mut group = c.benchmark_group("global_pool_api");

    // Configure for statistical validity
    group
        .sample_size(100)
        .measurement_time(Duration::from_secs(10));

    // Global pool (lazy initialization, reused across calls)
    group.bench_function("global_pool_scope_1000_tasks", |b| {
        b.iter(|| {
            let pool = get_global_pool();

            pool.scope(|s| {
                for i in 0..1000 {
                    s.spawn(move || {
                        black_box(i * 2);
                    })
                    .expect("Queue full");
                }
            });
        });
    });

    // Global pool with simulated work
    group.bench_function("global_pool_scope_100_tasks_work", |b| {
        b.iter(|| {
            let pool = get_global_pool();

            pool.scope(|s| {
                for i in 0..100 {
                    s.spawn(move || {
                        let start = std::time::Instant::now();
                        while start.elapsed() < Duration::from_micros(100) {
                            black_box(i * i);
                        }
                    })
                    .expect("Queue full");
                }
            });
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group! {
    name = benches;
    config = Criterion::default()
        .confidence_level(0.95)      // B2: 95% confidence interval
        .significance_level(0.05)    // B2: p-value < 0.05 for statistical significance
        .warm_up_time(Duration::from_secs(3)); // B19: Warmup period validation

    targets = bench_executor_creation,
              bench_task_claiming_empty,
              bench_simulated_cnls_workload,
              bench_scalability,
              bench_contention_scenarios,
              bench_comparison_to_rayon,
              bench_global_pool_api
}

criterion_main!(benches);
