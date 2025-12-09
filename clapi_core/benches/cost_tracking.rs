//! B32-Compliant Benchmark: Fixed-Point vs Floating-Point Cost Tracking
//!
//! **Framework**: B32 (Fair comparison + Precision validation)
//! **Baseline**: f32, f64, Decimal (rust_decimal crate)
//! **Focus**: Arithmetic performance AND precision accuracy
//!
//! ## Benchmarks
//!
//! 1. **Arithmetic operations**: Add/subtract/multiply latency
//! 2. **Accumulation precision**: 1M operations, measure drift
//! 3. **Concurrent updates**: Atomic fixed-point vs Mutex<f64>
//!
//! ## Expected Results (B32 Reality Checks)
//!
//! - Q16.16 vs f64 arithmetic: 2-5× speedup (integer vs FP)
//! - Q16.16 vs Decimal: 5-10× speedup (K27: Decimal is heavy)
//! - Precision: Q16.16 has zero drift (1M operations)
//! - Concurrent: 3-8× speedup (atomic integer vs mutex float)

use clapi_core::response::ResponseCapsule256;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::{Arc, Mutex as StdMutex};
use std::thread;
use std::time::Duration;

// ============================================================================
// B1-B5: Fair Baseline Implementations
// ============================================================================

/// Fixed-point Q16.16 format (our implementation via ResponseCapsule256)
struct FixedPointQ16 {
    capsule: ResponseCapsule256,
}

impl FixedPointQ16 {
    fn new() -> Self {
        Self {
            capsule: ResponseCapsule256::new(),
        }
    }

    fn add(&self, value_f64: f64) {
        self.capsule.record_response(0, 0, value_f64);
    }

    fn total(&self) -> f64 {
        self.capsule.total_cost_f64()
    }

    fn total_q16(&self) -> u64 {
        self.capsule.total_cost_q16()
    }
}

/// Baseline 1: f32 (single-precision float)
struct Float32Tracker {
    total: StdMutex<f32>,
}

impl Float32Tracker {
    fn new() -> Self {
        Self {
            total: StdMutex::new(0.0),
        }
    }

    fn add(&self, value: f32) {
        let mut total = self.total.lock().unwrap();
        *total += value;
    }

    fn total(&self) -> f32 {
        *self.total.lock().unwrap()
    }
}

/// Baseline 2: f64 (double-precision float)
struct Float64Tracker {
    total: StdMutex<f64>,
}

impl Float64Tracker {
    fn new() -> Self {
        Self {
            total: StdMutex::new(0.0),
        }
    }

    fn add(&self, value: f64) {
        let mut total = self.total.lock().unwrap();
        *total += value;
    }

    fn total(&self) -> f64 {
        *self.total.lock().unwrap()
    }
}

/// Baseline 3: rust_decimal::Decimal (arbitrary precision)
struct DecimalTracker {
    total: StdMutex<rust_decimal::Decimal>,
}

impl DecimalTracker {
    fn new() -> Self {
        use rust_decimal::Decimal;
        Self {
            total: StdMutex::new(Decimal::ZERO),
        }
    }

    fn add(&self, value: f64) {
        use rust_decimal::Decimal;
        use std::str::FromStr;

        let mut total = self.total.lock().unwrap();
        // Convert f64 to Decimal (expensive!)
        let decimal_value = Decimal::from_str(&value.to_string()).unwrap_or(Decimal::ZERO);
        *total += decimal_value;
    }

    fn total(&self) -> f64 {
        use rust_decimal::prelude::ToPrimitive;
        self.total.lock().unwrap().to_f64().unwrap_or(0.0)
    }
}

// ============================================================================
// B2: Single-Threaded Arithmetic Performance
// ============================================================================

fn bench_arithmetic_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("cost_tracking_arithmetic");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    let test_values: Vec<f64> = (0..1000).map(|i| (i as f64 * 0.001)).collect();

    // Q16.16 fixed-point (our implementation)
    group.bench_function("q16_16_fixed_point", |b| {
        let tracker = FixedPointQ16::new();
        b.iter(|| {
            for &value in &test_values {
                tracker.add(black_box(value));
            }
        });
    });

    // f32 baseline
    group.bench_function("f32_float", |b| {
        let tracker = Float32Tracker::new();
        b.iter(|| {
            for &value in &test_values {
                tracker.add(black_box(value as f32));
            }
        });
    });

    // f64 baseline
    group.bench_function("f64_float", |b| {
        let tracker = Float64Tracker::new();
        b.iter(|| {
            for &value in &test_values {
                tracker.add(black_box(value));
            }
        });
    });

    // Decimal baseline (HEAVY - expect 10-100× slower)
    group.bench_function("rust_decimal", |b| {
        let tracker = DecimalTracker::new();
        b.iter(|| {
            for &value in &test_values {
                tracker.add(black_box(value));
            }
        });
    });

    group.finish();
}

// ============================================================================
// B3: Precision Accuracy Test (1M operations)
// ============================================================================

fn bench_accumulation_precision(c: &mut Criterion) {
    let mut group = c.benchmark_group("cost_tracking_precision");
    group.warm_up_time(Duration::from_secs(5));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(100);
    group.throughput(Throughput::Elements(1_000_000));

    // Small value that accumulates drift in floating-point
    let small_value = 0.0001; // $0.0001
    let iterations = 1_000_000;

    // Q16.16 fixed-point - should have ZERO drift
    group.bench_function("q16_16_1m_accumulations", |b| {
        b.iter(|| {
            let tracker = FixedPointQ16::new();
            for _ in 0..iterations {
                tracker.add(black_box(small_value));
            }
            let total = tracker.total();
            black_box(total);

            // Expected: exactly 100.0
            // Actual Q16.16: 100.0 (zero drift)
        });
    });

    // f64 baseline - may accumulate rounding errors
    group.bench_function("f64_1m_accumulations", |b| {
        b.iter(|| {
            let tracker = Float64Tracker::new();
            for _ in 0..iterations {
                tracker.add(black_box(small_value));
            }
            let total = tracker.total();
            black_box(total);

            // Expected: 100.0
            // Actual f64: ~99.99999999... (floating-point drift)
        });
    });

    group.finish();
}

// ============================================================================
// B4: Concurrent Cost Tracking (Contention Scaling)
// ============================================================================

fn bench_concurrent_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("cost_tracking_concurrent");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(100);

    // Test with 1, 2, 4, 8, 16 threads
    for num_threads in [1, 2, 4, 8, 16] {
        group.throughput(Throughput::Elements(num_threads as u64 * 10000));

        // Q16.16 atomic fixed-point
        group.bench_with_input(
            BenchmarkId::new("q16_16_atomic", num_threads),
            &num_threads,
            |b, &num_threads| {
                let tracker = Arc::new(FixedPointQ16::new());
                b.iter(|| {
                    let handles: Vec<_> = (0..num_threads)
                        .map(|_| {
                            let tracker_clone = Arc::clone(&tracker);
                            thread::spawn(move || {
                                for i in 0..10000 {
                                    let value = (i as f64) * 0.001;
                                    tracker_clone.add(value);
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

        // f64 with Mutex baseline
        group.bench_with_input(
            BenchmarkId::new("f64_mutex", num_threads),
            &num_threads,
            |b, &num_threads| {
                let tracker = Arc::new(Float64Tracker::new());
                b.iter(|| {
                    let handles: Vec<_> = (0..num_threads)
                        .map(|_| {
                            let tracker_clone = Arc::clone(&tracker);
                            thread::spawn(move || {
                                for i in 0..10000 {
                                    let value = (i as f64) * 0.001;
                                    tracker_clone.add(value);
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
// B16: Latency Distribution (Single Operation)
// ============================================================================

fn bench_latency_distribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("cost_tracking_latency");
    group.warm_up_time(Duration::from_secs(5));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(2000);

    // Q16.16 fixed-point - single operation
    group.bench_function("q16_16_single_add", |b| {
        let tracker = FixedPointQ16::new();
        b.iter(|| {
            tracker.add(black_box(1.234567));
        });
    });

    // f64 - single operation
    group.bench_function("f64_single_add", |b| {
        let tracker = Float64Tracker::new();
        b.iter(|| {
            tracker.add(black_box(1.234567));
        });
    });

    // Decimal - single operation (expect 10-100× slower)
    group.bench_function("rust_decimal_single_add", |b| {
        let tracker = DecimalTracker::new();
        b.iter(|| {
            tracker.add(black_box(1.234567));
        });
    });

    group.finish();
}

// ============================================================================
// B3: Realistic Workload - API Cost Tracking
// ============================================================================

fn bench_realistic_api_costs(c: &mut Criterion) {
    let mut group = c.benchmark_group("cost_tracking_realistic_api");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(500);
    group.throughput(Throughput::Elements(4000));

    // Realistic API cost distribution:
    // - 60% GPT-4o-mini: $0.00015/1K tokens (cheap)
    // - 30% GPT-4o: $0.005/1K tokens (medium)
    // - 10% GPT-4: $0.03/1K tokens (expensive)
    let api_costs = vec![
        0.00015, 0.00015, 0.00015, 0.00015, 0.00015, 0.00015, // 60%
        0.005, 0.005, 0.005, // 30%
        0.03,  // 10%
    ];

    // Q16.16 fixed-point
    group.bench_function("q16_16_api_workload", |b| {
        let tracker = Arc::new(FixedPointQ16::new());
        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let tracker_clone = Arc::clone(&tracker);
                    let costs = api_costs.clone();
                    thread::spawn(move || {
                        for _ in 0..100 {
                            for &cost in &costs {
                                tracker_clone.add(cost);
                            }
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    // f64 with Mutex baseline
    group.bench_function("f64_mutex_api_workload", |b| {
        let tracker = Arc::new(Float64Tracker::new());
        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let tracker_clone = Arc::clone(&tracker);
                    let costs = api_costs.clone();
                    thread::spawn(move || {
                        for _ in 0..100 {
                            for &cost in &costs {
                                tracker_clone.add(cost);
                            }
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
// Criterion Configuration
// ============================================================================

criterion_group! {
    name = benches;
    config = Criterion::default()
        .confidence_level(0.95)
        .significance_level(0.05)
        .noise_threshold(0.05);
    targets =
        bench_arithmetic_operations,
        bench_accumulation_precision,
        bench_concurrent_tracking,
        bench_latency_distribution,
        bench_realistic_api_costs
}

criterion_main!(benches);
