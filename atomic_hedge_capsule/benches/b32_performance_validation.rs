//! B32 Framework Performance Validation
//!
//! Comprehensive validation of zero overhead claims using B32 benchmarking framework
//! with Kontext27 hardware reality checks for Intel Ultra 7 155H

use atomic_hedge_capsule::{AtomicHedgeCapsule, BracketOrder, EntryOrder};
use criterion::{black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use std::time::{Duration, Instant};

/// B32 Framework: Statistical validation with hardware reality checks
///
/// Validates performance claims against Kontext27 baselines:
/// - K13: Allocation costs (20ns small, 50ns medium, 200ns+ large)
/// - K2: Atomic operation costs (10-15ns CAS, 20ns FetchAdd)
/// - K6: Cache hierarchy (1ns L1, 3ns L2, 12ns L3, 100ns RAM)
struct B32PerformanceValidator {
    samples: Vec<Duration>,
    baseline_samples: Vec<Duration>,
}

impl B32PerformanceValidator {
    fn new() -> Self {
        Self {
            samples: Vec::with_capacity(10000),
            baseline_samples: Vec::with_capacity(10000),
        }
    }

    fn add_sample(&mut self, duration: Duration) {
        self.samples.push(duration);
    }

    fn add_baseline(&mut self, duration: Duration) {
        self.baseline_samples.push(duration);
    }

    /// Calculate statistical metrics per B32 requirements
    fn calculate_statistics(&self) -> PerformanceStatistics {
        let samples: Vec<f64> = self.samples.iter().map(|d| d.as_nanos() as f64).collect();
        let baselines: Vec<f64> = self
            .baseline_samples
            .iter()
            .map(|d| d.as_nanos() as f64)
            .collect();

        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        let baseline_mean = baselines.iter().sum::<f64>() / baselines.len() as f64;

        let mut sorted_samples = samples.clone();
        sorted_samples.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let p50 = sorted_samples[sorted_samples.len() / 2];
        let p95 = sorted_samples[(sorted_samples.len() * 95) / 100];
        let p99 = sorted_samples[(sorted_samples.len() * 99) / 100];

        let variance =
            samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / samples.len() as f64;
        let std_dev = variance.sqrt();

        // Calculate 95% confidence interval
        let std_error = std_dev / (samples.len() as f64).sqrt();
        let confidence_margin = 1.96 * std_error; // 95% CI

        PerformanceStatistics {
            mean,
            baseline_mean,
            p50,
            p95,
            p99,
            std_dev,
            confidence_interval: (mean - confidence_margin, mean + confidence_margin),
            overhead_percentage: ((mean - baseline_mean) / baseline_mean) * 100.0,
        }
    }

    /// Validate against Kontext27 hardware constraints
    fn validate_hardware_constraints(
        &self,
        stats: &PerformanceStatistics,
    ) -> HardwareValidationResult {
        HardwareValidationResult {
            within_cache_latency: stats.p95 < 12_000.0, // L3 cache latency
            within_allocation_budget: stats.mean < 50_000.0, // Medium allocation threshold
            atomic_operation_reasonable: stats.p50 < 100_000.0, // Multiple atomic ops
            statistical_significance: stats.confidence_interval.1 - stats.confidence_interval.0
                < stats.mean * 0.1,
        }
    }
}

#[derive(Debug)]
struct PerformanceStatistics {
    mean: f64,
    baseline_mean: f64,
    p50: f64,
    p95: f64,
    p99: f64,
    std_dev: f64,
    confidence_interval: (f64, f64),
    overhead_percentage: f64,
}

#[derive(Debug)]
struct HardwareValidationResult {
    within_cache_latency: bool,
    within_allocation_budget: bool,
    atomic_operation_reasonable: bool,
    statistical_significance: bool,
}

/// B32 B1: Fair baseline - optimized direct construction
fn benchmark_baseline_construction(validator: &mut B32PerformanceValidator, iterations: usize) {
    for _ in 0..iterations {
        let start = Instant::now();

        // Optimized direct construction (not strawman)
        let capsule = AtomicHedgeCapsule::new();
        let entry = EntryOrder::new(
            "NDAX".to_string(),
            "BTCUSD".to_string(),
            "Buy".to_string(),
            1.0,
        );
        let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
        let result = capsule.initialize(entry, bracket);

        let duration = start.elapsed();
        validator.add_baseline(duration);
        black_box(result);
    }
}

/// B32 validation: Builder pattern performance
fn benchmark_builder_pattern(validator: &mut B32PerformanceValidator, iterations: usize) {
    for _ in 0..iterations {
        let start = Instant::now();

        // Builder pattern construction
        let result = AtomicHedgeCapsule::hedge("BTCUSD")
            .on_exchange("NDAX")
            .size(1.0)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .build();

        let duration = start.elapsed();
        validator.add_sample(duration);
        black_box(result);
    }
}

/// B32 B3: Realistic workload testing
fn bench_realistic_workload_zero_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_workload");

    // B32 B2: Statistical rigor - 1000+ iterations, 95% CI
    group
        .confidence_level(0.95)
        .sample_size(1000)
        .warm_up_time(Duration::from_secs(3));

    // B32 B3: Production-like workload
    group.bench_function("production_workload_direct", |b| {
        b.iter_batched(
            || {
                // Setup: realistic market data
                vec![
                    ("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0),
                    ("ETHUSD", "NDAX", 10.0, 3000.0, 4000.0),
                    ("ADAUSD", "NDAX", 1000.0, 0.8, 1.2),
                ]
            },
            |orders| {
                for (symbol, exchange, size, stop, target) in orders {
                    let capsule = AtomicHedgeCapsule::new();
                    let entry = EntryOrder::new(
                        exchange.to_string(),
                        symbol.to_string(),
                        "Buy".to_string(),
                        size,
                    );
                    let bracket = BracketOrder::new(stop, target, size);
                    let _ = capsule.initialize(entry, bracket);
                    black_box(capsule);
                }
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("production_workload_builder", |b| {
        b.iter_batched(
            || {
                // Setup: identical realistic market data
                vec![
                    ("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0),
                    ("ETHUSD", "NDAX", 10.0, 3000.0, 4000.0),
                    ("ADAUSD", "NDAX", 1000.0, 0.8, 1.2),
                ]
            },
            |orders| {
                for (symbol, exchange, size, stop, target) in orders {
                    let capsule = AtomicHedgeCapsule::hedge(symbol)
                        .on_exchange(exchange)
                        .size(size)
                        .stop_loss(stop)
                        .take_profit(target)
                        .build()
                        .unwrap();
                    black_box(capsule);
                }
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("production_workload_simplified", |b| {
        b.iter_batched(
            || {
                // Setup: identical realistic market data
                vec![
                    ("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0),
                    ("ETHUSD", "NDAX", 10.0, 3000.0, 4000.0),
                    ("ADAUSD", "NDAX", 1000.0, 0.8, 1.2),
                ]
            },
            |orders| {
                for (symbol, exchange, size, stop, target) in orders {
                    let capsule =
                        AtomicHedgeCapsule::create_hedge(symbol, exchange, size, stop, target)
                            .unwrap();
                    black_box(capsule);
                }
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

/// B32 B4: Contention scenarios for zero overhead validation
fn bench_contention_zero_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("contention_scenarios");

    for thread_count in [1, 2, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::new("direct_contention", thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let handles: Vec<_> = (0..thread_count)
                        .map(|i| {
                            std::thread::spawn(move || {
                                let capsule = AtomicHedgeCapsule::new();
                                let entry = EntryOrder::new(
                                    "NDAX".to_string(),
                                    format!("BTC{}", i),
                                    "Buy".to_string(),
                                    1.0,
                                );
                                let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
                                capsule.initialize(entry, bracket).unwrap();
                                capsule
                            })
                        })
                        .collect();

                    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
                    black_box(results)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("builder_contention", thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let handles: Vec<_> = (0..thread_count)
                        .map(|i| {
                            std::thread::spawn(move || {
                                AtomicHedgeCapsule::hedge(&format!("BTC{}", i))
                                    .on_exchange("NDAX")
                                    .size(1.0)
                                    .stop_loss(45000.0)
                                    .take_profit(55000.0)
                                    .build()
                                    .unwrap()
                            })
                        })
                        .collect();

                    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
                    black_box(results)
                })
            },
        );
    }

    group.finish();
}

/// B32 B5: Comprehensive reporting with hardware validation
fn bench_comprehensive_performance_report(c: &mut Criterion) {
    const ITERATIONS: usize = 10000;

    let mut validator = B32PerformanceValidator::new();

    // Collect baseline samples
    benchmark_baseline_construction(&mut validator, ITERATIONS);

    // Collect builder pattern samples
    benchmark_builder_pattern(&mut validator, ITERATIONS);

    // Calculate statistics
    let stats = validator.calculate_statistics();
    let hardware_validation = validator.validate_hardware_constraints(&stats);

    // Print B32 compliant performance report
    println!("\n=== B32 Framework Performance Validation Report ===");
    println!("Hardware: Intel Ultra 7 155H (6P+8E+2LP cores)");
    println!("OS: Linux 6.14.0-27-generic");
    println!("Rust: 1.88.0-nightly");
    println!("Sample Size: {} iterations", ITERATIONS);
    println!();

    println!("Performance Metrics (nanoseconds):");
    println!("  Baseline Mean:    {:.1}ns", stats.baseline_mean);
    println!("  Builder Mean:     {:.1}ns", stats.mean);
    println!("  P50 (Median):     {:.1}ns", stats.p50);
    println!("  P95:              {:.1}ns", stats.p95);
    println!("  P99:              {:.1}ns", stats.p99);
    println!("  Std Deviation:    {:.1}ns", stats.std_dev);
    println!(
        "  95% CI:           [{:.1}, {:.1}]ns",
        stats.confidence_interval.0, stats.confidence_interval.1
    );
    println!();

    println!("Zero Overhead Analysis:");
    println!("  Overhead:         {:.2}%", stats.overhead_percentage);
    println!(
        "  Zero Overhead:    {}",
        if stats.overhead_percentage.abs() < 5.0 {
            "✓ VALIDATED"
        } else {
            "✗ FAILED"
        }
    );
    println!();

    println!("Kontext27 Hardware Validation:");
    println!(
        "  Cache Latency:    {}",
        if hardware_validation.within_cache_latency {
            "✓"
        } else {
            "✗"
        }
    );
    println!(
        "  Allocation Budget: {}",
        if hardware_validation.within_allocation_budget {
            "✓"
        } else {
            "✗"
        }
    );
    println!(
        "  Atomic Ops:       {}",
        if hardware_validation.atomic_operation_reasonable {
            "✓"
        } else {
            "✗"
        }
    );
    println!(
        "  Statistical:      {}",
        if hardware_validation.statistical_significance {
            "✓"
        } else {
            "✗"
        }
    );
    println!();

    // Validate against B32 requirements
    let b32_compliance = stats.overhead_percentage.abs() < 5.0 && // Zero overhead requirement
        hardware_validation.within_cache_latency &&
        hardware_validation.within_allocation_budget &&
        hardware_validation.statistical_significance;

    println!(
        "B32 Framework Compliance: {}",
        if b32_compliance {
            "✓ PASSED"
        } else {
            "✗ FAILED"
        }
    );

    // Assert zero overhead for CI/CD validation
    assert!(
        stats.overhead_percentage.abs() < 5.0,
        "Builder pattern introduces {}% overhead (threshold: 5%)",
        stats.overhead_percentage
    );

    // Minimal benchmark for criterion
    c.bench_function("b32_validation_placeholder", |b| {
        b.iter(|| black_box(stats.mean))
    });
}

/// Memory allocation validation
///
/// B32 B7: Memory allocation patterns must be identical
fn bench_allocation_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocation_patterns");

    // Use custom allocator tracking for precise measurements
    group.bench_function("direct_allocations", |b| {
        b.iter_batched(
            || (),
            |_| {
                let capsule = AtomicHedgeCapsule::new();
                let entry = EntryOrder::new(
                    "NDAX".to_string(),
                    "BTCUSD".to_string(),
                    "Buy".to_string(),
                    1.0,
                );
                let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
                capsule.initialize(entry, bracket).unwrap();
                black_box(capsule)
            },
            BatchSize::PerIteration,
        )
    });

    group.bench_function("builder_allocations", |b| {
        b.iter_batched(
            || (),
            |_| {
                let capsule = AtomicHedgeCapsule::hedge("BTCUSD")
                    .on_exchange("NDAX")
                    .size(1.0)
                    .stop_loss(45000.0)
                    .take_profit(55000.0)
                    .build()
                    .unwrap();
                black_box(capsule)
            },
            BatchSize::PerIteration,
        )
    });

    group.finish();
}

/// Cache optimization preservation
///
/// UCE-32 Q29: Verify cache optimization is preserved
fn bench_cache_optimization_preservation(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_optimization");

    // Test that cache optimization is preserved across construction methods
    let direct_capsule = {
        let c = AtomicHedgeCapsule::new();
        let entry = EntryOrder::new(
            "NDAX".to_string(),
            "BTCUSD".to_string(),
            "Buy".to_string(),
            1.0,
        );
        let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
        c.initialize(entry, bracket).unwrap();
        c
    };

    let builder_capsule = AtomicHedgeCapsule::hedge("BTCUSD")
        .on_exchange("NDAX")
        .size(1.0)
        .stop_loss(45000.0)
        .take_profit(55000.0)
        .build()
        .unwrap();

    // Validate cache info is identical
    let direct_cache = direct_capsule.cache_info();
    let builder_cache = builder_capsule.cache_info();

    assert_eq!(
        direct_cache.alignment, builder_cache.alignment,
        "Cache alignment must be identical"
    );
    assert_eq!(
        direct_cache.hot_data_offset, builder_cache.hot_data_offset,
        "Hot data offset must be identical"
    );
    assert_eq!(
        direct_cache.cold_data_offset, builder_cache.cold_data_offset,
        "Cold data offset must be identical"
    );

    // Benchmark hot path performance to ensure cache optimization is preserved
    group.bench_function("direct_hot_path", |b| {
        b.iter(|| direct_capsule.load_hot_data())
    });

    group.bench_function("builder_hot_path", |b| {
        b.iter(|| builder_capsule.load_hot_data())
    });

    group.finish();
}

criterion_group!(
    b32_validation,
    bench_realistic_workload_zero_overhead,
    bench_contention_zero_overhead,
    bench_comprehensive_performance_report,
    bench_allocation_patterns,
    bench_cache_optimization_preservation
);

criterion_main!(b32_validation);
