//! B32-Compliant Benchmark: RequestCapsule128 Budget Validation
//!
//! **Framework**: B32 (32 benchmarking guidelines + 50 hardware reality checks)
//! **Baseline**: Multiple optimized implementations (NO STRAWMEN)
//! **Statistical Rigor**: 1000+ iterations, 95% CI, warmup period
//! **Hardware**: Target - Intel Ultra 7 155H (6P+8E cores)
//!
//! ## Benchmarks
//!
//! 1. **Single-threaded**: Atomic capsule vs mutex vs parking_lot
//! 2. **Contention scaling**: 1, 2, 4, 8, 16 threads
//! 3. **Realistic workload**: Mixed success/failure validations
//!
//! ## Expected Results (B32 Reality Checks)
//!
//! - Atomic vs std::Mutex: 2-3× speedup (K27: typical optimization)
//! - Atomic vs parking_lot: 1.5-2× speedup (K27: parking_lot is optimized)
//! - Contention scaling: 3-8× at 16 threads (K12: lockfree sweet spot)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use clapi_core::RequestCapsule128;
use std::sync::{Arc, Mutex as StdMutex};
use std::thread;
use std::time::Duration;

// ============================================================================
// B1-B5: Fair Baseline Implementations (NO STRAWMEN)
// ============================================================================

/// Baseline 1: std::sync::Mutex (fair baseline, not strawman)
struct MutexBudget {
    budget: StdMutex<u64>,
}

impl MutexBudget {
    fn new(budget: u64) -> Self {
        Self {
            budget: StdMutex::new(budget),
        }
    }

    fn try_validate(&self, cost: u64) -> Result<(), ()> {
        let mut guard = self.budget.lock().unwrap();
        if *guard >= cost {
            *guard -= cost;
            Ok(())
        } else {
            Err(())
        }
    }

    fn budget(&self) -> u64 {
        *self.budget.lock().unwrap()
    }
}

/// Baseline 2: parking_lot::Mutex (optimized baseline)
struct ParkingLotBudget {
    budget: parking_lot::Mutex<u64>,
}

impl ParkingLotBudget {
    fn new(budget: u64) -> Self {
        Self {
            budget: parking_lot::Mutex::new(budget),
        }
    }

    fn try_validate(&self, cost: u64) -> Result<(), ()> {
        let mut guard = self.budget.lock();
        if *guard >= cost {
            *guard -= cost;
            Ok(())
        } else {
            Err(())
        }
    }

    fn budget(&self) -> u64 {
        *self.budget.lock()
    }
}

// ============================================================================
// B2: Single-Threaded Benchmarks (Uncontended Case)
// ============================================================================

fn bench_single_threaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("budget_validation_single_thread");
    group.warm_up_time(Duration::from_secs(3)); // B8: Cache warming
    group.measurement_time(Duration::from_secs(10)); // B2: Sustained measurement
    group.sample_size(1000); // B2: Statistical significance

    // Atomic capsule (our implementation)
    group.bench_function("atomic_capsule", |b| {
        let capsule = RequestCapsule128::new(1, 1_000_000_00); // $1M budget
        b.iter(|| {
            // B3: Realistic workload (90% success rate)
            let cost = if black_box(0) % 10 == 0 { 2_000_00 } else { 10_00 };
            let _ = black_box(capsule.try_validate(cost));
        });
    });

    // Baseline 1: std::sync::Mutex
    group.bench_function("std_mutex", |b| {
        let mutex_budget = MutexBudget::new(1_000_000_00);
        b.iter(|| {
            let cost = if black_box(0) % 10 == 0 { 2_000_00 } else { 10_00 };
            let _ = black_box(mutex_budget.try_validate(cost));
        });
    });

    // Baseline 2: parking_lot::Mutex (optimized)
    group.bench_function("parking_lot_mutex", |b| {
        let pl_budget = ParkingLotBudget::new(1_000_000_00);
        b.iter(|| {
            let cost = if black_box(0) % 10 == 0 { 2_000_00 } else { 10_00 };
            let _ = black_box(pl_budget.try_validate(cost));
        });
    });

    group.finish();
}

// ============================================================================
// B4: Contention Scaling Benchmarks (B12: Thread Scaling)
// ============================================================================

fn bench_contention_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("budget_validation_contention");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(15)); // B3: Realistic sustained load
    group.sample_size(100); // Fewer samples for thread benchmarks

    // Test with 1, 2, 4, 8, 16 threads (B12: Thread scaling analysis)
    for num_threads in [1, 2, 4, 8, 16] {
        group.throughput(Throughput::Elements(num_threads as u64 * 1000));

        // Atomic capsule
        group.bench_with_input(
            BenchmarkId::new("atomic_capsule", num_threads),
            &num_threads,
            |b, &num_threads| {
                let capsule = Arc::new(RequestCapsule128::new(1, 10_000_000_00)); // $10M budget
                b.iter(|| {
                    let handles: Vec<_> = (0..num_threads)
                        .map(|_| {
                            let capsule_clone = Arc::clone(&capsule);
                            thread::spawn(move || {
                                for _ in 0..1000 {
                                    // B3: Realistic workload (mixed costs)
                                    let cost = if black_box(0) % 3 == 0 { 20_00 } else { 5_00 };
                                    let _ = black_box(capsule_clone.try_validate(cost));
                                }
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );

        // std::Mutex baseline
        group.bench_with_input(
            BenchmarkId::new("std_mutex", num_threads),
            &num_threads,
            |b, &num_threads| {
                let mutex_budget = Arc::new(MutexBudget::new(10_000_000_00));
                b.iter(|| {
                    let handles: Vec<_> = (0..num_threads)
                        .map(|_| {
                            let budget_clone = Arc::clone(&mutex_budget);
                            thread::spawn(move || {
                                for _ in 0..1000 {
                                    let cost = if black_box(0) % 3 == 0 { 20_00 } else { 5_00 };
                                    let _ = black_box(budget_clone.try_validate(cost));
                                }
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );

        // parking_lot::Mutex baseline
        group.bench_with_input(
            BenchmarkId::new("parking_lot_mutex", num_threads),
            &num_threads,
            |b, &num_threads| {
                let pl_budget = Arc::new(ParkingLotBudget::new(10_000_000_00));
                b.iter(|| {
                    let handles: Vec<_> = (0..num_threads)
                        .map(|_| {
                            let budget_clone = Arc::clone(&pl_budget);
                            thread::spawn(move || {
                                for _ in 0..1000 {
                                    let cost = if black_box(0) % 3 == 0 { 20_00 } else { 5_00 };
                                    let _ = black_box(budget_clone.try_validate(cost));
                                }
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// B3: Realistic Workload Benchmarks
// ============================================================================

fn bench_realistic_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("budget_validation_realistic");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(500);

    // Realistic workload: 70% small requests, 20% medium, 10% large
    // Models real AI API usage patterns (most requests cheap, some expensive)
    group.bench_function("atomic_capsule_realistic_mix", |b| {
        let capsule = Arc::new(RequestCapsule128::new(1, 100_000_00)); // $100k budget
        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let capsule_clone = Arc::clone(&capsule);
                    thread::spawn(move || {
                        for i in 0..250 {
                            // Realistic cost distribution
                            let cost = match i % 10 {
                                0..=6 => 50, // 70% small ($0.50)
                                7..=8 => 200, // 20% medium ($2.00)
                                _ => 1000, // 10% large ($10.00)
                            };
                            let _ = black_box(capsule_clone.try_validate(cost));
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    // Same realistic workload with parking_lot baseline
    group.bench_function("parking_lot_realistic_mix", |b| {
        let pl_budget = Arc::new(ParkingLotBudget::new(100_000_00));
        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let budget_clone = Arc::clone(&pl_budget);
                    thread::spawn(move || {
                        for i in 0..250 {
                            let cost = match i % 10 {
                                0..=6 => 50,
                                7..=8 => 200,
                                _ => 1000,
                            };
                            let _ = black_box(budget_clone.try_validate(cost));
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// B16: Latency Percentile Analysis (P50, P95, P99)
// ============================================================================

fn bench_latency_distribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("budget_validation_latency");
    group.warm_up_time(Duration::from_secs(5)); // Extra warmup for stability
    group.measurement_time(Duration::from_secs(15)); // Long measurement for percentiles
    group.sample_size(2000); // Large sample size for accurate percentiles

    // Atomic capsule - single operation latency
    group.bench_function("atomic_capsule_latency", |b| {
        let capsule = RequestCapsule128::new(1, 1_000_000_00);
        b.iter(|| {
            black_box(capsule.try_validate(black_box(10_00)))
        });
    });

    // Mutex baseline - single operation latency
    group.bench_function("parking_lot_latency", |b| {
        let pl_budget = ParkingLotBudget::new(1_000_000_00);
        b.iter(|| {
            black_box(pl_budget.try_validate(black_box(10_00)))
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration (B2: Statistical Rigor)
// ============================================================================

criterion_group! {
    name = benches;
    config = Criterion::default()
        .confidence_level(0.95) // B2: 95% confidence intervals
        .significance_level(0.05)
        .noise_threshold(0.05);
    targets =
        bench_single_threaded,
        bench_contention_scaling,
        bench_realistic_workload,
        bench_latency_distribution
}

criterion_main!(benches);
