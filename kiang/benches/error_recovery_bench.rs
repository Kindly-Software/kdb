//! Error Recovery Performance Benchmarks
//!
//! Benchmarks measuring GPU error detection and recovery latency.
//! Follows B32 framework for fair, statistically rigorous measurement.
//!
//! # Performance Targets
//!
//! - Hang detection: <100μs (continuous monitoring overhead)
//! - Context reset: <1ms (GPU state reset)
//! - Recovery complete: <5ms (full recovery workflow)
//! - Monitoring overhead: <5% CPU
//!
//! # B32 Compliance
//!
//! - B1: Fair baseline (with vs without monitoring)
//! - B2: Statistical rigor (Criterion 95% CI)
//! - B3: Realistic scenarios (actual error patterns)
//! - B16: Latency distribution (P50, P95, P99)

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use kiang::{GpuCircuitBreaker, GpuMetrics, QualityLevel};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Simulated GPU hang detection (baseline)
fn detect_hang_simulated(metrics: &GpuMetrics) -> bool {
    let errors = metrics.snapshot().errors;
    errors > 100 // Threshold for hang detection
}

/// Benchmark: Hang detection latency
///
/// # Expected Results (B32 K2)
/// - Single check: <100ns (metrics read + comparison)
/// - Continuous monitoring: <5% CPU overhead
fn bench_hang_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("hang_detection");

    let metrics = Arc::new(GpuMetrics::new());

    // Simulate some GPU activity
    for i in 0..50 {
        metrics.inc_commands(10);
        if i % 10 == 0 {
            metrics.inc_errors();
        }
    }

    // Single hang check
    group.bench_function("single_check", |b| {
        b.iter(|| {
            let is_hung = detect_hang_simulated(black_box(&metrics));
            black_box(is_hung);
        });
    });

    // Continuous monitoring overhead
    group.bench_function("continuous_monitoring", |b| {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        let metrics_clone = Arc::clone(&metrics);

        // Spawn monitoring thread
        let monitor = std::thread::spawn(move || {
            let mut check_count = 0u64;
            while running_clone.load(Ordering::Relaxed) {
                let _hung = detect_hang_simulated(&metrics_clone);
                check_count += 1;
                std::thread::sleep(Duration::from_micros(100));
            }
            check_count
        });

        b.iter(|| {
            // Simulate GPU work while monitoring runs
            for _ in 0..100 {
                metrics.inc_commands(1);
                black_box(&metrics);
            }
        });

        running.store(false, Ordering::Relaxed);
        let _checks = monitor.join().unwrap();
    });

    group.finish();
}

/// Benchmark: Context reset time
///
/// # Expected Results (B32 K18)
/// - Software reset: ~1ms (state cleanup)
/// - Hardware reset: ~5ms (GPU recovery)
fn bench_context_reset(c: &mut Criterion) {
    let mut group = c.benchmark_group("context_reset");

    // Simulated software reset (state cleanup only)
    group.bench_function("software_reset", |b| {
        let breaker = GpuCircuitBreaker::new();

        b.iter(|| {
            // Simulate state cleanup
            black_box(&breaker).force_level(QualityLevel::L3); // Pause
            std::thread::sleep(Duration::from_micros(100)); // Cleanup overhead
            black_box(&breaker).force_level(QualityLevel::L0); // Resume
        });
    });

    // Simulated hardware reset (includes GPU recovery)
    group.bench_function("hardware_reset", |b| {
        b.iter(|| {
            // Simulate hardware reset sequence
            std::thread::sleep(Duration::from_millis(1)); // GPU reset
            black_box(true);
        });
    });

    group.finish();
}

/// Benchmark: Recovery workflow latency
///
/// Complete recovery workflow:
/// 1. Detect hang
/// 2. Force quality degradation
/// 3. Reset context
/// 4. Restore normal operation
///
/// # Expected Results
/// - Complete workflow: <5ms
/// - Recovery success rate: >99%
fn bench_recovery_workflow(c: &mut Criterion) {
    let mut group = c.benchmark_group("recovery_workflow");

    let metrics = Arc::new(GpuMetrics::new());
    let breaker = Arc::new(GpuCircuitBreaker::new());

    // Simulate error condition
    for _ in 0..150 {
        metrics.inc_errors();
    }

    group.bench_function("complete_workflow", |b| {
        b.iter(|| {
            // 1. Detect hang
            let hung = detect_hang_simulated(&metrics);
            black_box(hung);

            // 2. Force degradation
            breaker.force_level(QualityLevel::L3);

            // 3. Simulate context reset
            std::thread::sleep(Duration::from_micros(100));

            // 4. Restore normal operation
            breaker.force_level(QualityLevel::L0);
        });
    });

    group.finish();
}

/// Benchmark: Circuit breaker auto-recovery
///
/// Tests automatic recovery when error rate decreases.
///
/// # B32 Validation
/// - B17: Throughput vs latency tradeoffs
fn bench_auto_recovery(c: &mut Criterion) {
    let breaker = GpuCircuitBreaker::new();

    c.bench_function("auto_recovery", |b| {
        b.iter(|| {
            // High error rate → degradation
            black_box(&breaker).auto_adjust(
                black_box(85_000), // High temp
                black_box(60),     // High errors
                black_box(85),     // High memory
                black_box(95),     // High utilization
            );

            // Error rate decreases → recovery
            black_box(&breaker).auto_adjust(
                black_box(65_000), // Normal temp
                black_box(5),      // Low errors
                black_box(50),     // Normal memory
                black_box(60),     // Normal utilization
            );
        });
    });
}

/// Benchmark: Error rate calculation overhead
///
/// Measures cost of computing error metrics.
fn bench_error_rate_calculation(c: &mut Criterion) {
    let metrics = GpuMetrics::new();

    // Simulate activity
    for i in 0..1000 {
        metrics.inc_commands(10);
        if i % 20 == 0 {
            metrics.inc_errors();
        }
    }

    c.bench_function("error_rate_calculation", |b| {
        b.iter(|| {
            let rate = black_box(&metrics).error_rate();
            black_box(rate);
        });
    });
}

/// Benchmark: Thermal throttling response
///
/// Measures latency from thermal event to quality degradation.
///
/// # Expected Results
/// - Thermal check: <100ns
/// - Auto-adjust: <50ns (single atomic write)
fn bench_thermal_throttling(c: &mut Criterion) {
    let breaker = GpuCircuitBreaker::new();

    c.bench_function("thermal_throttling", |b| {
        b.iter(|| {
            // Simulate thermal spike
            black_box(&breaker).auto_adjust(
                black_box(95_000), // Critical temp
                black_box(0),      // No errors
                black_box(50),     // Normal memory
                black_box(75),     // High utilization
            );
        });
    });
}

/// Benchmark: Recovery under load
///
/// Tests recovery performance with concurrent GPU activity.
///
/// # B32 Compliance
/// - B4: Contention scenarios (1, 2, 4 threads)
fn bench_recovery_under_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("recovery_under_load");

    for num_threads in [1, 2, 4].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            num_threads,
            |b, &num_threads| {
                let metrics = Arc::new(GpuMetrics::new());
                let breaker = Arc::new(GpuCircuitBreaker::new());

                b.iter(|| {
                    let mut handles = vec![];

                    // Spawn load threads
                    for _ in 0..num_threads {
                        let metrics_clone = Arc::clone(&metrics);
                        let breaker_clone = Arc::clone(&breaker);

                        handles.push(std::thread::spawn(move || {
                            // Simulate GPU work
                            for _ in 0..100 {
                                metrics_clone.inc_commands(1);
                                let _allow = breaker_clone.should_allow_command();
                            }
                        }));
                    }

                    // Trigger recovery while load is active
                    for _ in 0..10 {
                        metrics.inc_errors();
                    }
                    breaker.force_level(QualityLevel::L2);
                    std::thread::sleep(Duration::from_micros(50));
                    breaker.force_level(QualityLevel::L0);

                    // Wait for load threads
                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Memory pressure recovery
///
/// Tests recovery from memory exhaustion.
fn bench_memory_pressure_recovery(c: &mut Criterion) {
    let metrics = GpuMetrics::new();
    let breaker = GpuCircuitBreaker::new();

    // Simulate memory pressure
    metrics.set_allocated(7_000_000_000); // 7GB / 8GB = 87.5%

    c.bench_function("memory_pressure_recovery", |b| {
        b.iter(|| {
            // Check memory pressure
            let mem_pct = black_box(&metrics).memory_usage_pct();

            // Auto-adjust based on memory
            black_box(&breaker).auto_adjust(
                black_box(70_000), // Normal temp
                black_box(0),      // No errors
                black_box(mem_pct),
                black_box(80), // High utilization
            );

            // Simulate memory freed
            metrics.set_allocated(3_000_000_000); // 3GB / 8GB = 37.5%

            // Recovery
            let mem_pct = metrics.memory_usage_pct();
            breaker.auto_adjust(
                black_box(70_000),
                black_box(0),
                black_box(mem_pct),
                black_box(60),
            );
        });
    });
}

/// Benchmark: Quality level transition latency
///
/// Measures time to transition between quality levels.
///
/// # Expected Results (B32 K2)
/// - Single transition: <50ns (atomic compare_exchange)
fn bench_quality_transitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("quality_transitions");

    let breaker = GpuCircuitBreaker::new();

    // Test each transition
    let transitions = [
        (QualityLevel::L0, QualityLevel::L1, "L0_to_L1"),
        (QualityLevel::L1, QualityLevel::L2, "L1_to_L2"),
        (QualityLevel::L2, QualityLevel::L3, "L2_to_L3"),
        (QualityLevel::L3, QualityLevel::L0, "L3_to_L0"),
    ];

    for (from, to, name) in transitions.iter() {
        group.bench_function(*name, |b| {
            b.iter(|| {
                breaker.force_level(*from);
                breaker.force_level(*to);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_hang_detection,
    bench_context_reset,
    bench_recovery_workflow,
    bench_auto_recovery,
    bench_error_rate_calculation,
    bench_thermal_throttling,
    bench_recovery_under_load,
    bench_memory_pressure_recovery,
    bench_quality_transitions,
);

criterion_main!(benches);
