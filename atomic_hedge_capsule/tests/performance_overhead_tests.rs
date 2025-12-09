//! Performance Overhead Validation Tests
//!
//! UCE-32 Q29 (Practical Constraints): Observability must not impact production performance
//! UCE-32 Q30 (Empirical Validation): Zero overhead claims validated through rigorous measurement
//! UCE-32 Q31 (Rust Transform): Zero-cost abstractions maintain performance guarantees
//! UCE-32 Q32 (Nightly Enhancement): Advanced measurement techniques for overhead detection
//!
//! ## Test Coverage
//! 1. Zero overhead verification - observability features don't impact core performance
//! 2. Metrics collection overhead - measurement precision validation
//! 3. Logging performance impact - structured logging overhead assessment
//! 4. Diagnostic system overhead - health checks and diagnostics cost analysis
//! 5. Feature flag overhead - conditional compilation effectiveness

use atomic_hedge_capsule::{
    AtomicHedgeCapsule, BracketOrder, EntryOrder, HedgeError, HedgeStateSnapshot, HedgeStatus,
    OrderState,
};

#[cfg(feature = "presets")]
use atomic_hedge_capsule::{
    AtomicHedgeCapsulePresets, MemoryOrderingLevel, MonitoringConfig, PresetConfig, ValidationLevel,
};

#[cfg(feature = "logging")]
use atomic_hedge_capsule::{
    current_log_level, init_logging, is_logging_enabled, set_logging_enabled, LogConfig, LogLevel,
};

#[cfg(feature = "diagnostics")]
use atomic_hedge_capsule::{DiagnosticsExt, HealthStatus, PerformanceStatus};

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Performance test configuration
const WARMUP_ITERATIONS: usize = 1000;
const MEASUREMENT_ITERATIONS: usize = 10000;
const STATISTICAL_CONFIDENCE: f64 = 0.95; // 95% confidence intervals
const ZERO_OVERHEAD_THRESHOLD_PERCENT: f64 = 1.0; // 1% maximum acceptable overhead
const MEASUREMENT_PRECISION_NS: f64 = 0.5; // 0.5ns measurement precision target

/// Performance measurement result
#[derive(Debug, Clone)]
struct PerformanceMeasurement {
    operation_name: String,
    baseline_ns: f64,
    with_observability_ns: f64,
    overhead_percent: f64,
    standard_deviation: f64,
    confidence_interval: (f64, f64),
    passes_threshold: bool,
}

impl PerformanceMeasurement {
    fn new(operation_name: String, baseline_times: &[f64], observability_times: &[f64]) -> Self {
        let baseline_ns = statistical_mean(baseline_times);
        let with_observability_ns = statistical_mean(observability_times);
        let overhead_percent = ((with_observability_ns - baseline_ns) / baseline_ns) * 100.0;

        let baseline_std = standard_deviation(baseline_times);
        let observability_std = standard_deviation(observability_times);
        let combined_std = (baseline_std + observability_std) / 2.0;

        // 95% confidence interval for overhead measurement
        let margin_of_error = 1.96 * combined_std / (baseline_times.len() as f64).sqrt();
        let confidence_interval = (
            overhead_percent - margin_of_error,
            overhead_percent + margin_of_error,
        );

        let passes_threshold = overhead_percent <= ZERO_OVERHEAD_THRESHOLD_PERCENT;

        Self {
            operation_name,
            baseline_ns,
            with_observability_ns,
            overhead_percent,
            standard_deviation: combined_std,
            confidence_interval,
            passes_threshold,
        }
    }

    fn report(&self) -> String {
        format!(
            "{}: {:.2}ns baseline, {:.2}ns with observability, {:.3}% overhead (±{:.3}%), {}",
            self.operation_name,
            self.baseline_ns,
            self.with_observability_ns,
            self.overhead_percent,
            (self.confidence_interval.1 - self.confidence_interval.0) / 2.0,
            if self.passes_threshold {
                "✓ PASS"
            } else {
                "✗ FAIL"
            }
        )
    }
}

// ============================================================================
// CORE ZERO OVERHEAD VERIFICATION TESTS
// ============================================================================

#[test]
fn test_basic_operations_zero_overhead() {
    // UCE-32 Q30: Empirical validation that basic operations have zero overhead
    let baseline_hedge = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
        .expect("Failed to create baseline hedge");

    let observability_hedge =
        AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
            .expect("Failed to create observability hedge");

    // Test submit_order operation
    let submit_measurement = measure_operation_overhead(
        "submit_order",
        || {
            let _ = baseline_hedge.submit_order();
        },
        || {
            let _ = observability_hedge.submit_order();
            let _ = observability_hedge.snapshot();
            let _ = observability_hedge.status();
        },
    );

    println!("{}", submit_measurement.report());
    assert!(
        submit_measurement.passes_threshold,
        "submit_order overhead ({:.3}%) exceeds threshold ({:.1}%)",
        submit_measurement.overhead_percent, ZERO_OVERHEAD_THRESHOLD_PERCENT
    );

    // Test snapshot operation
    let snapshot_measurement = measure_operation_overhead(
        "snapshot",
        || {
            let _ = baseline_hedge.submit_order();
        },
        || {
            let _ = baseline_hedge.submit_order();
            let _ = baseline_hedge.snapshot();
        },
    );

    println!("{}", snapshot_measurement.report());
    assert!(
        snapshot_measurement.passes_threshold,
        "snapshot overhead ({:.3}%) exceeds threshold ({:.1}%)",
        snapshot_measurement.overhead_percent, ZERO_OVERHEAD_THRESHOLD_PERCENT
    );

    // Test status operation
    let status_measurement = measure_operation_overhead(
        "status",
        || {
            let _ = baseline_hedge.submit_order();
        },
        || {
            let _ = baseline_hedge.submit_order();
            let _ = baseline_hedge.status();
        },
    );

    println!("{}", status_measurement.report());
    assert!(
        status_measurement.passes_threshold,
        "status overhead ({:.3}%) exceeds threshold ({:.1}%)",
        status_measurement.overhead_percent, ZERO_OVERHEAD_THRESHOLD_PERCENT
    );
}

#[test]
fn test_emergency_operations_zero_overhead() {
    // UCE-32 Q31: Emergency operations must maintain zero overhead guarantee
    let baseline_hedge = AtomicHedgeCapsule::create_hedge("ETHUSD", "NDAX", 1.0, 3000.0, 4000.0)
        .expect("Failed to create baseline hedge");

    let observability_hedge =
        AtomicHedgeCapsule::create_hedge("ETHUSD", "NDAX", 1.0, 3000.0, 4000.0)
            .expect("Failed to create observability hedge");

    // Test emergency_stop operation
    let emergency_measurement = measure_operation_overhead(
        "emergency_stop",
        || {
            baseline_hedge.emergency_stop();
        },
        || {
            observability_hedge.emergency_stop();
            let _ = observability_hedge.snapshot();
            let _ = observability_hedge.status();
        },
    );

    println!("{}", emergency_measurement.report());
    assert!(
        emergency_measurement.passes_threshold,
        "emergency_stop overhead ({:.3}%) exceeds threshold ({:.1}%)",
        emergency_measurement.overhead_percent, ZERO_OVERHEAD_THRESHOLD_PERCENT
    );
}

#[test]
fn test_concurrent_operations_zero_overhead() {
    // UCE-32 Q31: Zero overhead must be maintained under concurrent access
    let baseline_hedge = Arc::new(
        AtomicHedgeCapsule::create_hedge("ADAUSD", "NDAX", 1000.0, 1.0, 2.0)
            .expect("Failed to create baseline hedge"),
    );

    let observability_hedge = Arc::new(
        AtomicHedgeCapsule::create_hedge("ADAUSD", "NDAX", 1000.0, 1.0, 2.0)
            .expect("Failed to create observability hedge"),
    );

    // Measure concurrent baseline performance
    let baseline_times = measure_concurrent_operations(
        Arc::clone(&baseline_hedge),
        |hedge| {
            let _ = hedge.submit_order();
        },
        4,   // 4 threads
        100, // 100 operations per thread
    );

    // Measure concurrent performance with observability
    let observability_times = measure_concurrent_operations(
        Arc::clone(&observability_hedge),
        |hedge| {
            let _ = hedge.submit_order();
            let _ = hedge.snapshot();
            let _ = hedge.status();
        },
        4,   // 4 threads
        100, // 100 operations per thread
    );

    let concurrent_measurement = PerformanceMeasurement::new(
        "concurrent_operations".to_string(),
        &baseline_times,
        &observability_times,
    );

    println!("{}", concurrent_measurement.report());
    assert!(
        concurrent_measurement.passes_threshold,
        "Concurrent operations overhead ({:.3}%) exceeds threshold ({:.1}%)",
        concurrent_measurement.overhead_percent, ZERO_OVERHEAD_THRESHOLD_PERCENT
    );
}

// ============================================================================
// METRICS COLLECTION OVERHEAD TESTS
// ============================================================================

#[test]
fn test_metrics_collection_precision() {
    // UCE-32 Q32: Advanced measurement techniques for metrics overhead
    let hedge = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
        .expect("Failed to create hedge");

    // Measure just the metrics collection overhead
    let metrics_times: Vec<f64> = (0..MEASUREMENT_ITERATIONS)
        .map(|_| {
            let start = Instant::now();
            let _ = hedge.snapshot();
            start.elapsed().as_nanos() as f64
        })
        .collect();

    let avg_metrics_time = statistical_mean(&metrics_times);
    let std_metrics_time = standard_deviation(&metrics_times);

    println!(
        "Metrics collection timing: {:.2}ns avg, {:.2}ns std",
        avg_metrics_time, std_metrics_time
    );

    // Metrics collection should be extremely fast and consistent
    assert!(
        avg_metrics_time < 50.0, // < 50ns average
        "Metrics collection too slow: {:.2}ns average",
        avg_metrics_time
    );

    assert!(
        std_metrics_time < avg_metrics_time * 0.5, // Std dev < 50% of mean
        "Metrics collection too variable: {:.2}ns std vs {:.2}ns avg",
        std_metrics_time,
        avg_metrics_time
    );

    // Test status collection timing
    let status_times: Vec<f64> = (0..MEASUREMENT_ITERATIONS)
        .map(|_| {
            let start = Instant::now();
            let _ = hedge.status();
            start.elapsed().as_nanos() as f64
        })
        .collect();

    let avg_status_time = statistical_mean(&status_times);
    let std_status_time = standard_deviation(&status_times);

    println!(
        "Status collection timing: {:.2}ns avg, {:.2}ns std",
        avg_status_time, std_status_time
    );

    assert!(
        avg_status_time < 50.0, // < 50ns average
        "Status collection too slow: {:.2}ns average",
        avg_status_time
    );

    assert!(
        std_status_time < avg_status_time * 0.5,
        "Status collection too variable: {:.2}ns std vs {:.2}ns avg",
        std_status_time,
        avg_status_time
    );
}

#[test]
fn test_metrics_calculation_overhead() {
    // UCE-32 Q30: Metrics calculations must be zero-cost
    let hedge = AtomicHedgeCapsule::create_hedge("ETHUSD", "NDAX", 1.0, 3000.0, 4000.0)
        .expect("Failed to create hedge");

    // Measure time to retrieve raw data vs calculated metrics
    let raw_data_times: Vec<f64> = (0..MEASUREMENT_ITERATIONS)
        .map(|_| {
            let start = Instant::now();
            let snapshot = hedge.snapshot();
            let _ = snapshot.entry_state;
            let _ = snapshot.filled_size;
            let _ = snapshot.operation_count;
            start.elapsed().as_nanos() as f64
        })
        .collect();

    let calculated_metrics_times: Vec<f64> = (0..MEASUREMENT_ITERATIONS)
        .map(|_| {
            let start = Instant::now();
            let snapshot = hedge.snapshot();
            let _ = snapshot.completion_percentage();
            let _ = snapshot.risk_status();
            let _ = snapshot.is_terminal();
            let _ = snapshot.is_processing();
            start.elapsed().as_nanos() as f64
        })
        .collect();

    let calculation_measurement = PerformanceMeasurement::new(
        "metrics_calculations".to_string(),
        &raw_data_times,
        &calculated_metrics_times,
    );

    println!("{}", calculation_measurement.report());

    // Calculated metrics should have minimal overhead over raw data access
    assert!(
        calculation_measurement.overhead_percent < 10.0, // < 10% overhead for calculations
        "Metrics calculations overhead ({:.3}%) too high",
        calculation_measurement.overhead_percent
    );
}

// ============================================================================
// LOGGING PERFORMANCE IMPACT TESTS
// ============================================================================

#[cfg(feature = "logging")]
#[test]
fn test_logging_disabled_overhead() {
    // UCE-32 Q29: Disabled logging must have zero runtime cost

    // Ensure logging is disabled for baseline
    set_logging_enabled(false);
    assert!(!is_logging_enabled());

    let hedge = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
        .expect("Failed to create hedge");

    // Measure operations with logging disabled
    let disabled_times: Vec<f64> = (0..MEASUREMENT_ITERATIONS)
        .map(|_| {
            let start = Instant::now();
            let _ = hedge.submit_order();
            // Simulate logging calls that should be no-ops
            start.elapsed().as_nanos() as f64
        })
        .collect();

    // Enable logging for comparison
    set_logging_enabled(true);
    assert!(is_logging_enabled());

    // Measure operations with logging enabled
    let enabled_times: Vec<f64> = (0..MEASUREMENT_ITERATIONS)
        .map(|_| {
            let start = Instant::now();
            let _ = hedge.submit_order();
            start.elapsed().as_nanos() as f64
        })
        .collect();

    let logging_measurement = PerformanceMeasurement::new(
        "logging_overhead".to_string(),
        &disabled_times,
        &enabled_times,
    );

    println!("{}", logging_measurement.report());

    // Logging should add minimal overhead when enabled
    assert!(
        logging_measurement.overhead_percent < 5.0, // < 5% overhead when enabled
        "Logging overhead ({:.3}%) exceeds threshold (5%)",
        logging_measurement.overhead_percent
    );

    // Reset to disabled state
    set_logging_enabled(false);
}

#[cfg(feature = "logging")]
#[test]
fn test_log_level_impact() {
    // UCE-32 Q31: Different log levels should have predictable performance impact

    let hedge = AtomicHedgeCapsule::create_hedge("ETHUSD", "NDAX", 1.0, 3000.0, 4000.0)
        .expect("Failed to create hedge");

    let log_levels = vec![
        LogLevel::Error,
        LogLevel::Warn,
        LogLevel::Info,
        LogLevel::Debug,
        LogLevel::Trace,
    ];

    let mut level_measurements = HashMap::new();

    for level in log_levels {
        init_logging(LogConfig {
            level,
            enabled: true,
            console_output: false,
            file_output: None,
            structured: true,
        })
        .expect("Failed to initialize logging");

        let level_times: Vec<f64> = (0..1000)
            .map(|_| {
                let start = Instant::now();
                let _ = hedge.submit_order();
                start.elapsed().as_nanos() as f64
            })
            .collect();

        let avg_time = statistical_mean(&level_times);
        level_measurements.insert(level, avg_time);

        println!("Log level {:?}: {:.2}ns average", level, avg_time);
    }

    // Error level should be fastest (least logging)
    // Trace level should be slowest (most logging)
    assert!(
        level_measurements[&LogLevel::Error] <= level_measurements[&LogLevel::Trace],
        "Error level ({:.2}ns) should be faster than Trace level ({:.2}ns)",
        level_measurements[&LogLevel::Error],
        level_measurements[&LogLevel::Trace]
    );

    // Performance degradation should be reasonable
    let overhead_percent = ((level_measurements[&LogLevel::Trace]
        - level_measurements[&LogLevel::Error])
        / level_measurements[&LogLevel::Error])
        * 100.0;

    assert!(
        overhead_percent < 20.0, // < 20% overhead from Error to Trace
        "Log level overhead ({:.3}%) too high",
        overhead_percent
    );
}

// ============================================================================
// DIAGNOSTIC SYSTEM OVERHEAD TESTS
// ============================================================================

#[cfg(feature = "diagnostics")]
#[test]
fn test_diagnostics_collection_overhead() {
    // UCE-32 Q29: Diagnostic collection must not impact core performance
    let hedge = AtomicHedgeCapsule::create_hedge("ADAUSD", "NDAX", 1000.0, 1.0, 2.0)
        .expect("Failed to create hedge");

    // Measure operations without diagnostics
    let baseline_times: Vec<f64> = (0..MEASUREMENT_ITERATIONS)
        .map(|_| {
            let start = Instant::now();
            let _ = hedge.submit_order();
            start.elapsed().as_nanos() as f64
        })
        .collect();

    // Measure operations with diagnostic collection
    let diagnostics_times: Vec<f64> = (0..MEASUREMENT_ITERATIONS)
        .map(|_| {
            let start = Instant::now();
            let _ = hedge.submit_order();
            let _ = hedge.diagnostics();
            start.elapsed().as_nanos() as f64
        })
        .collect();

    let diagnostics_measurement = PerformanceMeasurement::new(
        "diagnostics_collection".to_string(),
        &baseline_times,
        &diagnostics_times,
    );

    println!("{}", diagnostics_measurement.report());
    assert!(
        diagnostics_measurement.overhead_percent < 3.0, // < 3% overhead for diagnostics
        "Diagnostics collection overhead ({:.3}%) exceeds threshold (3%)",
        diagnostics_measurement.overhead_percent
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn test_health_check_frequency_impact() {
    // UCE-32 Q30: Frequent health checks should have minimal cumulative impact
    let hedge = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
        .expect("Failed to create hedge");

    // Test different health check frequencies
    let frequencies = vec![1, 10, 100, 1000]; // Health check every N operations

    let mut frequency_results = HashMap::new();

    for frequency in frequencies {
        let times: Vec<f64> = (0..1000)
            .map(|i| {
                let start = Instant::now();
                let _ = hedge.submit_order();

                // Perform health check based on frequency
                if i % frequency == 0 {
                    let _ = hedge.health_check();
                }

                start.elapsed().as_nanos() as f64
            })
            .collect();

        let avg_time = statistical_mean(&times);
        frequency_results.insert(frequency, avg_time);

        println!(
            "Health check frequency 1/{}: {:.2}ns average",
            frequency, avg_time
        );
    }

    // Higher frequency should have slightly higher average time
    assert!(
        frequency_results[&1] >= frequency_results[&1000],
        "Frequency impact not as expected: 1/{} = {:.2}ns, 1/{} = {:.2}ns",
        1,
        frequency_results[&1],
        1000,
        frequency_results[&1000]
    );

    // But impact should be minimal
    let max_overhead =
        ((frequency_results[&1] - frequency_results[&1000]) / frequency_results[&1000]) * 100.0;

    assert!(
        max_overhead < 5.0, // < 5% overhead for continuous health checks
        "Health check frequency overhead ({:.3}%) too high",
        max_overhead
    );
}

// ============================================================================
// FEATURE FLAG OVERHEAD TESTS
// ============================================================================

#[test]
fn test_feature_flag_efficiency() {
    // UCE-32 Q31: Feature flags should compile away completely
    let hedge = AtomicHedgeCapsule::create_hedge("ETHUSD", "NDAX", 1.0, 3000.0, 4000.0)
        .expect("Failed to create hedge");

    // Test that feature detection is zero-cost
    let feature_check_times: Vec<f64> = (0..MEASUREMENT_ITERATIONS)
        .map(|_| {
            let start = Instant::now();

            // These should all compile away to constants
            let _ = atomic_hedge_capsule::features::has_nightly_features();
            let _ = atomic_hedge_capsule::features::has_simd();
            let _ = atomic_hedge_capsule::features::has_async();

            #[cfg(feature = "diagnostics")]
            let _ = atomic_hedge_capsule::features::has_diagnostics();

            #[cfg(feature = "logging")]
            let _ = atomic_hedge_capsule::features::has_logging();

            start.elapsed().as_nanos() as f64
        })
        .collect();

    let avg_feature_time = statistical_mean(&feature_check_times);
    println!(
        "Feature detection timing: {:.2}ns average",
        avg_feature_time
    );

    // Feature detection should be effectively zero-cost (< 1ns)
    assert!(
        avg_feature_time < 1.0,
        "Feature detection overhead ({:.3}ns) should be zero-cost",
        avg_feature_time
    );
}

#[cfg(feature = "presets")]
#[test]
fn test_preset_configuration_overhead() {
    // UCE-32 Q30: Preset configurations should not impact runtime performance

    // Test different preset configurations
    let presets = vec![
        ("hft", PresetConfig::high_frequency_trading()),
        ("risk_management", PresetConfig::risk_management()),
        ("development", PresetConfig::development()),
        ("production", PresetConfig::production()),
    ];

    let mut preset_results = HashMap::new();

    for (preset_name, config) in presets {
        let hedge = AtomicHedgeCapsulePresets::create_with_config(
            "BTCUSD", "NDAX", 1.0, 45000.0, 55000.0, config,
        )
        .expect(&format!("Failed to create {} preset hedge", preset_name));

        let times: Vec<f64> = (0..1000)
            .map(|_| {
                let start = Instant::now();
                let _ = hedge.submit_order();
                let _ = hedge.snapshot();
                start.elapsed().as_nanos() as f64
            })
            .collect();

        let avg_time = statistical_mean(&times);
        preset_results.insert(preset_name, avg_time);

        println!("Preset '{}': {:.2}ns average", preset_name, avg_time);
    }

    // All presets should have similar performance (within 10% of each other)
    let min_time = preset_results
        .values()
        .min_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();
    let max_time = preset_results
        .values()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();
    let variation_percent = ((max_time - min_time) / min_time) * 100.0;

    assert!(
        variation_percent < 10.0, // < 10% variation between presets
        "Preset performance variation ({:.3}%) too high",
        variation_percent
    );
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn measure_operation_overhead<F1, F2>(
    operation_name: &str,
    baseline_op: F1,
    observability_op: F2,
) -> PerformanceMeasurement
where
    F1: Fn() + Clone,
    F2: Fn() + Clone,
{
    // Warmup
    for _ in 0..WARMUP_ITERATIONS {
        baseline_op();
        observability_op();
    }

    // Measure baseline
    let baseline_times: Vec<f64> = (0..MEASUREMENT_ITERATIONS)
        .map(|_| {
            let start = Instant::now();
            baseline_op();
            start.elapsed().as_nanos() as f64
        })
        .collect();

    // Measure with observability
    let observability_times: Vec<f64> = (0..MEASUREMENT_ITERATIONS)
        .map(|_| {
            let start = Instant::now();
            observability_op();
            start.elapsed().as_nanos() as f64
        })
        .collect();

    PerformanceMeasurement::new(
        operation_name.to_string(),
        &baseline_times,
        &observability_times,
    )
}

fn measure_concurrent_operations<F>(
    hedge: Arc<AtomicHedgeCapsule>,
    operation: F,
    num_threads: usize,
    operations_per_thread: usize,
) -> Vec<f64>
where
    F: Fn(&AtomicHedgeCapsule) + Send + Sync + Clone + 'static,
{
    let mut handles = vec![];
    let times = Arc::new(std::sync::Mutex::new(Vec::new()));

    for _ in 0..num_threads {
        let hedge_clone = Arc::clone(&hedge);
        let times_clone = Arc::clone(&times);
        let op_clone = operation.clone();

        let handle = thread::spawn(move || {
            let mut local_times = Vec::new();

            for _ in 0..operations_per_thread {
                let start = Instant::now();
                op_clone(&hedge_clone);
                let elapsed = start.elapsed().as_nanos() as f64;
                local_times.push(elapsed);
            }

            {
                let mut shared_times = times_clone.lock().unwrap();
                shared_times.extend(local_times);
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    let result = times.lock().unwrap().clone();
    result
}

fn statistical_mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn standard_deviation(values: &[f64]) -> f64 {
    let mean = statistical_mean(values);
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / values.len() as f64;
    variance.sqrt()
}

// ============================================================================
// INTEGRATION OVERHEAD TESTS
// ============================================================================

#[test]
fn test_full_observability_stack_overhead() {
    // UCE-32 Q29: Full observability stack should maintain acceptable overhead
    let baseline_hedge = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
        .expect("Failed to create baseline hedge");

    let observability_hedge =
        AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
            .expect("Failed to create observability hedge");

    // Enable all observability features
    #[cfg(feature = "logging")]
    {
        set_logging_enabled(true);
        init_logging(LogConfig {
            level: LogLevel::Info,
            enabled: true,
            console_output: false,
            file_output: None,
            structured: true,
        })
        .expect("Failed to initialize logging");
    }

    // Measure full observability stack
    let full_stack_measurement = measure_operation_overhead(
        "full_observability_stack",
        || {
            let _ = baseline_hedge.submit_order();
        },
        || {
            let _ = observability_hedge.submit_order();
            let _ = observability_hedge.snapshot();
            let _ = observability_hedge.status();

            #[cfg(feature = "diagnostics")]
            let _ = observability_hedge.diagnostics();

            #[cfg(feature = "diagnostics")]
            let _ = observability_hedge.health_check();
        },
    );

    println!("{}", full_stack_measurement.report());

    // Full observability stack should still maintain reasonable overhead
    assert!(
        full_stack_measurement.overhead_percent < 10.0, // < 10% for full stack
        "Full observability stack overhead ({:.3}%) exceeds threshold (10%)",
        full_stack_measurement.overhead_percent
    );

    // Reset logging state
    #[cfg(feature = "logging")]
    set_logging_enabled(false);
}

#[test]
fn test_overhead_accumulation() {
    // UCE-32 Q31: Multiple observability features should not have exponential overhead
    let hedge = AtomicHedgeCapsule::create_hedge("ETHUSD", "NDAX", 1.0, 3000.0, 4000.0)
        .expect("Failed to create hedge");

    // Test individual feature overhead
    let individual_measurements = vec![
        measure_individual_feature_overhead(&hedge, "snapshot", || {
            let _ = hedge.snapshot();
        }),
        measure_individual_feature_overhead(&hedge, "status", || {
            let _ = hedge.status();
        }),
        #[cfg(feature = "diagnostics")]
        measure_individual_feature_overhead(&hedge, "diagnostics", || {
            let _ = hedge.diagnostics();
        }),
        #[cfg(feature = "diagnostics")]
        measure_individual_feature_overhead(&hedge, "health_check", || {
            let _ = hedge.health_check();
        }),
    ];

    // Measure combined overhead
    let combined_measurement = measure_individual_feature_overhead(&hedge, "combined", || {
        let _ = hedge.snapshot();
        let _ = hedge.status();

        #[cfg(feature = "diagnostics")]
        let _ = hedge.diagnostics();

        #[cfg(feature = "diagnostics")]
        let _ = hedge.health_check();
    });

    // Calculate expected additive overhead
    let expected_combined_time: f64 = individual_measurements
        .iter()
        .map(|m| m.with_observability_ns - m.baseline_ns)
        .sum::<f64>()
        + individual_measurements[0].baseline_ns;

    let actual_combined_time = combined_measurement.with_observability_ns;
    let accumulation_ratio = actual_combined_time / expected_combined_time;

    println!(
        "Overhead accumulation ratio: {:.3} (1.0 = perfect additive)",
        accumulation_ratio
    );

    // Overhead should be roughly additive (ratio close to 1.0)
    assert!(
        accumulation_ratio < 1.5, // < 50% overhead accumulation
        "Overhead accumulation ratio ({:.3}) indicates exponential overhead growth",
        accumulation_ratio
    );

    assert!(
        accumulation_ratio > 0.8, // > 80% efficiency
        "Overhead accumulation ratio ({:.3}) indicates unexpected efficiency loss",
        accumulation_ratio
    );
}

fn measure_individual_feature_overhead<F>(
    hedge: &AtomicHedgeCapsule,
    feature_name: &str,
    feature_op: F,
) -> PerformanceMeasurement
where
    F: Fn() + Clone,
{
    let baseline_times: Vec<f64> = (0..1000)
        .map(|_| {
            let start = Instant::now();
            let _ = hedge.submit_order();
            start.elapsed().as_nanos() as f64
        })
        .collect();

    let feature_times: Vec<f64> = (0..1000)
        .map(|_| {
            let start = Instant::now();
            let _ = hedge.submit_order();
            feature_op();
            start.elapsed().as_nanos() as f64
        })
        .collect();

    PerformanceMeasurement::new(feature_name.to_string(), &baseline_times, &feature_times)
}
