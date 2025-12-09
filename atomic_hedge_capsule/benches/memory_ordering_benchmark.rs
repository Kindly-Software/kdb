//! B32-Compliant Memory Ordering Benchmark
//!
//! Following B32 Framework guidelines for fair performance validation:
//! - Multiple optimized baselines (not strawmen)
//! - Statistical rigor with 95% confidence intervals
//! - Real workloads representative of production
//! - Contention testing across thread counts
//! - Hardware-specific validation on Intel Ultra 7 155H

use criterion::{
    black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput,
};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// Mock atomic operations for baseline comparison
struct SeqCstBaseline {
    flag: AtomicBool,
    counter: AtomicU64,
}

impl SeqCstBaseline {
    fn new() -> Self {
        Self {
            flag: AtomicBool::new(false),
            counter: AtomicU64::new(0),
        }
    }

    fn set_emergency(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    fn is_emergency(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    fn increment_counter(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::SeqCst)
    }
}

struct AcquireReleaseOptimized {
    flag: AtomicBool,
    counter: AtomicU64,
}

impl AcquireReleaseOptimized {
    fn new() -> Self {
        Self {
            flag: AtomicBool::new(false),
            counter: AtomicU64::new(0),
        }
    }

    fn set_emergency(&self) {
        self.flag.store(true, Ordering::Release);
    }

    fn is_emergency(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }

    fn increment_counter(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::Relaxed)
    }
}

struct ParkingLotBaseline {
    flag: parking_lot::Mutex<bool>,
    counter: parking_lot::Mutex<u64>,
}

impl ParkingLotBaseline {
    fn new() -> Self {
        Self {
            flag: parking_lot::Mutex::new(false),
            counter: parking_lot::Mutex::new(0),
        }
    }

    fn set_emergency(&self) {
        *self.flag.lock() = true;
    }

    fn is_emergency(&self) -> bool {
        *self.flag.lock()
    }

    fn increment_counter(&self) -> u64 {
        let mut counter = self.counter.lock();
        *counter += 1;
        *counter
    }
}

/// B32 Guideline B1: Fair Baseline Selection
/// Testing against multiple optimized implementations, not strawmen
fn bench_emergency_coordination(c: &mut Criterion) {
    let mut group = c.benchmark_group("emergency_coordination");

    // B32 Guideline B2: Statistical rigor with 95% CI
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(2));

    // Test uncontended case (best case for all implementations)
    group.bench_function("seqcst_baseline", |b| {
        let coordinator = SeqCstBaseline::new();
        b.iter(|| {
            coordinator.set_emergency();
            black_box(coordinator.is_emergency());
        });
    });

    group.bench_function("acquire_release_optimized", |b| {
        let coordinator = AcquireReleaseOptimized::new();
        b.iter(|| {
            coordinator.set_emergency();
            black_box(coordinator.is_emergency());
        });
    });

    group.bench_function("parking_lot_baseline", |b| {
        let coordinator = ParkingLotBaseline::new();
        b.iter(|| {
            coordinator.set_emergency();
            black_box(coordinator.is_emergency());
        });
    });

    group.finish();
}

/// B32 Guideline B4: Contention Scenarios
/// Test both uncontended and contended cases
fn bench_contention_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("contention_scaling");
    group.throughput(Throughput::Elements(1));

    for num_threads in [1, 2, 4, 8, 16].iter() {
        // SeqCst baseline under contention
        group.bench_with_input(
            BenchmarkId::new("seqcst_contended", num_threads),
            num_threads,
            |b, &num_threads| {
                let coordinator = Arc::new(SeqCstBaseline::new());

                b.iter_custom(|iters| {
                    let start = std::time::Instant::now();

                    let handles: Vec<_> = (0..num_threads)
                        .map(|_| {
                            let coord = Arc::clone(&coordinator);
                            thread::spawn(move || {
                                for _ in 0..iters / num_threads as u64 {
                                    coord.increment_counter();
                                    black_box(coord.is_emergency());
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    start.elapsed()
                });
            },
        );

        // Acquire/Release optimized under contention
        group.bench_with_input(
            BenchmarkId::new("acquire_release_contended", num_threads),
            num_threads,
            |b, &num_threads| {
                let coordinator = Arc::new(AcquireReleaseOptimized::new());

                b.iter_custom(|iters| {
                    let start = std::time::Instant::now();

                    let handles: Vec<_> = (0..num_threads)
                        .map(|_| {
                            let coord = Arc::clone(&coordinator);
                            thread::spawn(move || {
                                for _ in 0..iters / num_threads as u64 {
                                    coord.increment_counter();
                                    black_box(coord.is_emergency());
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    start.elapsed()
                });
            },
        );
    }

    group.finish();
}

/// B32 Guideline B3: Realistic Workloads
/// Test with production-like access patterns
fn bench_realistic_hedge_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_hedge_workload");

    // Simulate realistic hedge operation mix:
    // 70% emergency checks, 20% counter updates, 10% emergency sets
    group.bench_function("seqcst_realistic", |b| {
        let coordinator = SeqCstBaseline::new();
        b.iter_batched(
            || 0u64,
            |mut operation_count| {
                for _ in 0..100 {
                    match operation_count % 10 {
                        0 => coordinator.set_emergency(), // 10% emergency sets
                        1..=2 => {
                            coordinator.increment_counter();
                        } // 20% updates
                        _ => {
                            black_box(coordinator.is_emergency());
                        } // 70% checks
                    }
                    operation_count += 1;
                }
                operation_count
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("acquire_release_realistic", |b| {
        let coordinator = AcquireReleaseOptimized::new();
        b.iter_batched(
            || 0u64,
            |mut operation_count| {
                for _ in 0..100 {
                    match operation_count % 10 {
                        0 => coordinator.set_emergency(), // 10% emergency sets
                        1..=2 => {
                            coordinator.increment_counter();
                        } // 20% updates
                        _ => {
                            black_box(coordinator.is_emergency());
                        } // 70% checks
                    }
                    operation_count += 1;
                }
                operation_count
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// B32 Guideline B8: Cache Warming Strategy
/// Separate micro and integration benchmarks
fn bench_isolated_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("isolated_operations");

    // Emergency flag store only
    group.bench_function("emergency_store_seqcst", |b| {
        let flag = AtomicBool::new(false);
        b.iter(|| {
            flag.store(true, Ordering::SeqCst);
            flag.store(false, Ordering::SeqCst);
        });
    });

    group.bench_function("emergency_store_release", |b| {
        let flag = AtomicBool::new(false);
        b.iter(|| {
            flag.store(true, Ordering::Release);
            flag.store(false, Ordering::Release);
        });
    });

    // Emergency flag load only
    group.bench_function("emergency_load_seqcst", |b| {
        let flag = AtomicBool::new(false);
        b.iter(|| {
            black_box(flag.load(Ordering::SeqCst));
        });
    });

    group.bench_function("emergency_load_acquire", |b| {
        let flag = AtomicBool::new(false);
        b.iter(|| {
            black_box(flag.load(Ordering::Acquire));
        });
    });

    // Counter operations
    group.bench_function("counter_acqrel", |b| {
        let counter = AtomicU64::new(0);
        b.iter(|| {
            counter.fetch_add(1, Ordering::AcqRel);
        });
    });

    group.bench_function("counter_relaxed", |b| {
        let counter = AtomicU64::new(0);
        b.iter(|| {
            counter.fetch_add(1, Ordering::Relaxed);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_emergency_coordination,
    bench_contention_scaling,
    bench_realistic_hedge_workload,
    bench_isolated_operations
);
criterion_main!(benches);
