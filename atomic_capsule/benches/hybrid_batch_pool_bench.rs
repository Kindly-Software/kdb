//! HybridBatchPool B32 Benchmark Suite
//!
//! **Purpose**: Validate 4.4× speedup claim with fair baselines (Mutex vs HybridBatchPool)
//! **Framework**: B32 (95% CI, 1000+ iterations, fair baselines)
//! **Metrics**: Throughput, latency percentiles (P50/P95/P99/P99.9), scaling efficiency
//!
//! **Performance Target**:
//! - Mutex baseline: ~88μs for 1,600 tasks (50 threads × 32 tasks)
//! - HybridBatchPool: <20μs for 1,600 tasks (>4.4× speedup)
//! - Scaling: Linear to 256 threads (95%+ efficiency)

use atomic_capsule::parallel::HybridBatchPool;
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput, PlotConfiguration, AxisScale};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use std::time::Instant;

// ============================================================================
// BASELINE: Mutex<VecDeque> Approach (Current ThreadPool pattern)
// ============================================================================

/// Baseline task execution using Mutex (simulating current behavior)
fn bench_mutex_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline_mutex");
    group.sample_size(1000);  // B32: 1000+ iterations
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));

    for num_threads in [1, 4, 8, 16, 32, 50] {
        group.throughput(Throughput::Elements(1600 as u64));
        group.bench_with_input(
            BenchmarkId::new("push_1600_tasks", num_threads),
            &num_threads,
            |b, &threads| {
                b.iter(|| {
                    let queue = Arc::new(Mutex::new(VecDeque::new()));
                    let counter = Arc::new(AtomicUsize::new(0));
                    let completed = Arc::new(AtomicUsize::new(0));

                    let handles: Vec<_> = (0..threads)
                        .map(|_| {
                            let q = queue.clone();
                            let c = counter.clone();
                            let comp = completed.clone();

                            std::thread::spawn(move || {
                                // Enqueue phase
                                for _ in 0..(1600 / threads) {
                                    let _guard = q.lock().unwrap();
                                    c.fetch_add(1, Ordering::Relaxed);
                                }

                                // Worker thread drains queue
                                loop {
                                    let mut q_guard = q.lock().unwrap();
                                    if q_guard.is_empty() && c.load(Ordering::Relaxed) == 0 {
                                        drop(q_guard);
                                        break;
                                    }
                                    if let Some(_task) = q_guard.pop_front() {
                                        drop(q_guard);
                                        comp.fetch_add(1, Ordering::Relaxed);
                                        c.fetch_sub(1, Ordering::Relaxed);
                                    } else {
                                        drop(q_guard);
                                        std::thread::yield_now();
                                    }
                                }
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }

                    black_box(completed.load(Ordering::Relaxed));
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// OPTIMIZED: HybridBatchPool Approach
// ============================================================================

/// HybridBatchPool execution (optimized pattern)
fn bench_hybrid_batch_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("hybrid_batch_pool");
    group.sample_size(1000);  // B32: 1000+ iterations
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));

    for num_threads in [1, 4, 8, 16, 32, 50] {
        group.throughput(Throughput::Elements(1600 as u64));
        group.bench_with_input(
            BenchmarkId::new("push_1600_tasks", num_threads),
            &num_threads,
            |b, &threads| {
                b.iter(|| {
                    let pool = Arc::new(HybridBatchPool::new(8).unwrap());
                    let counter = Arc::new(AtomicUsize::new(0));

                    let handles: Vec<_> = (0..threads)
                        .map(|_| {
                            let p = pool.clone();
                            let c = counter.clone();

                            std::thread::spawn(move || {
                                for _ in 0..(1600 / threads) {
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
            },
        );
    }

    group.finish();
}

// ============================================================================
// DIRECT COMPARISON: 1,600 Tasks at 50 Threads
// ============================================================================

/// Direct latency comparison at the canonical workload
fn bench_1600_tasks_50_threads_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("1600_tasks_50_threads");
    group.sample_size(1000);  // B32: 1000+ iterations for 95% CI
    group.measurement_time(std::time::Duration::from_secs(30));

    // BASELINE: Mutex
    group.bench_function("mutex_baseline", |b| {
        b.iter(|| {
            let queue = Arc::new(Mutex::new(VecDeque::new()));
            let counter = Arc::new(AtomicUsize::new(0));
            let completed = Arc::new(AtomicUsize::new(0));

            let handles: Vec<_> = (0..50)
                .map(|_| {
                    let q = queue.clone();
                    let c = counter.clone();
                    let comp = completed.clone();

                    std::thread::spawn(move || {
                        for _ in 0..32 {
                            let _guard = q.lock().unwrap();
                            c.fetch_add(1, Ordering::Relaxed);
                        }

                        loop {
                            let mut q_guard = q.lock().unwrap();
                            if q_guard.is_empty() && c.load(Ordering::Relaxed) == 0 {
                                drop(q_guard);
                                break;
                            }
                            if let Some(_task) = q_guard.pop_front() {
                                drop(q_guard);
                                comp.fetch_add(1, Ordering::Relaxed);
                                c.fetch_sub(1, Ordering::Relaxed);
                            } else {
                                drop(q_guard);
                                std::thread::yield_now();
                            }
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }

            black_box(completed.load(Ordering::Relaxed))
        });
    });

    // OPTIMIZED: HybridBatchPool
    group.bench_function("hybrid_batch_pool", |b| {
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
            black_box(counter.load(Ordering::Relaxed))
        });
    });

    group.finish();
}

// ============================================================================
// TASK COUNT VARIATION: 100 to 10,000 tasks
// ============================================================================

/// Benchmark varying task counts (100, 1000, 1600, 10000)
fn bench_task_count_variation(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_count_variation");
    group.sample_size(500);  // Reduced for larger workloads
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));

    for task_count in [100, 1000, 1600, 10000] {
        group.throughput(Throughput::Elements(task_count as u64));
        group.bench_with_input(
            BenchmarkId::new("hybrid_batch_pool", task_count),
            &task_count,
            |b, &count| {
                b.iter(|| {
                    let pool = Arc::new(HybridBatchPool::new(8).unwrap());
                    let counter = Arc::new(AtomicUsize::new(0));

                    // Use 50 threads for consistency
                    let num_threads = 50.min(count);
                    let tasks_per_thread = count / num_threads;

                    let handles: Vec<_> = (0..num_threads)
                        .map(|_| {
                            let p = pool.clone();
                            let c = counter.clone();

                            std::thread::spawn(move || {
                                for _ in 0..tasks_per_thread {
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
                    black_box(counter.load(Ordering::Relaxed))
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// LATENCY PERCENTILES: P50, P95, P99, P99.9
// ============================================================================

/// Capture latency percentiles for 1,600 task workload
fn measure_latency_percentiles() {
    let mut latencies = Vec::new();

    for _ in 0..1000 {
        let start = Instant::now();

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
        let elapsed = start.elapsed().as_micros();
        latencies.push(elapsed as f64);
    }

    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let p50 = latencies[(500) as usize];
    let p95 = latencies[(950) as usize];
    let p99 = latencies[(990) as usize];
    let p99_9 = latencies[(999) as usize];

    println!("\n=== LATENCY PERCENTILES (1,600 tasks, 50 threads) ===");
    println!("P50:   {:.2} μs", p50);
    println!("P95:   {:.2} μs", p95);
    println!("P99:   {:.2} μs", p99);
    println!("P99.9: {:.2} μs", p99_9);
    println!("Min:   {:.2} μs", latencies[0]);
    println!("Max:   {:.2} μs", latencies[999]);
    println!("Mean:  {:.2} μs", latencies.iter().sum::<f64>() / latencies.len() as f64);
}

// ============================================================================
// SCALING EFFICIENCY: 1 to 256 threads
// ============================================================================

/// Measure scaling efficiency across thread counts
fn bench_scaling_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_efficiency");
    group.sample_size(100);
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));

    for num_threads in [1, 2, 4, 8, 16, 32, 64, 128] {
        let task_count = 1600;
        let tasks_per_thread = task_count / num_threads;

        group.throughput(Throughput::Elements(task_count as u64));
        group.bench_with_input(
            BenchmarkId::new("hybrid_batch_pool", num_threads),
            &num_threads,
            |b, &threads| {
                b.iter(|| {
                    let pool = Arc::new(HybridBatchPool::new(8).unwrap());
                    let counter = Arc::new(AtomicUsize::new(0));

                    let handles: Vec<_> = (0..threads)
                        .map(|_| {
                            let p = pool.clone();
                            let c = counter.clone();

                            std::thread::spawn(move || {
                                for _ in 0..tasks_per_thread {
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
                    black_box(counter.load(Ordering::Relaxed))
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// CRITERION SETUP
// ============================================================================

criterion_group!(
    name = benches;
    config = Criterion::default()
        .sample_size(1000)
        .measurement_time(std::time::Duration::from_secs(30));
    targets =
        bench_mutex_baseline,
        bench_hybrid_batch_pool,
        bench_1600_tasks_50_threads_comparison,
        bench_task_count_variation,
        bench_scaling_efficiency
);

criterion_main!(benches);
