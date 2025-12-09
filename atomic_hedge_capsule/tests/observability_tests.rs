//! Comprehensive Observability Tests for AtomicHedgeCapsule
//!
//! UCE-32 Q28 (Simplicity): Simple, accurate metrics validation
//! UCE-32 Q29 (Practical Constraints): Real-world observability requirements
//! UCE-32 Q30 (Empirical Validation): Measurable, accurate observability data
//! UCE-32 Q31 (Rust Transform): Zero-cost observability through type system
//! UCE-32 Q32 (Nightly Enhancement): Advanced metrics collection with nightly features
//!
//! ## Test Coverage
//! 1. Metrics accuracy validation - ensuring all metrics reflect true system state
//! 2. Zero overhead verification - observability doesn't impact performance
//! 3. Health check correctness - status reporting matches internal state
//! 4. Diagnostic usefulness - diagnostics provide actionable insights
//! 5. Monitoring integration - proper metrics collection and reporting

use atomic_hedge_capsule::{
    types::HedgeRiskStatus, AtomicHedgeCapsule, BracketOrder, EntryOrder, HedgeError,
    HedgeStateSnapshot, HedgeStatus, OrderState,
};

#[cfg(feature = "presets")]
use atomic_hedge_capsule::{
    AtomicHedgeCapsulePresets, CacheOptimization, MemoryOrderingLevel, MonitoringConfig,
    PerformanceFeatures, PresetConfig, ValidationLevel,
};

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Test observability configuration
const TEST_ITERATIONS: usize = 1000;
const MEASUREMENT_THRESHOLD_NS: f64 = 5.0; // 5ns measurement precision threshold
const ZERO_OVERHEAD_THRESHOLD_PERCENT: f64 = 2.0; // 2% overhead threshold

// ============================================================================
// CORE METRICS ACCURACY TESTS
// ============================================================================

#[test]
fn test_basic_metrics_accuracy() {
    // UCE-32 Q30: Empirical validation that metrics reflect true system state
    let hedge = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
        .expect("Failed to create hedge");

    // Initial state verification
    let initial_snapshot = hedge.get_hedge_state();
    assert_eq!(initial_snapshot.entry_state, OrderState::PendingValidation);
    assert_eq!(initial_snapshot.stop_state, OrderState::PendingValidation);
    assert_eq!(initial_snapshot.target_state, OrderState::PendingValidation);
    assert_eq!(initial_snapshot.filled_size, 0.0);
    assert_eq!(initial_snapshot.operation_count, 0);
    assert!(!initial_snapshot.emergency_stopped);
    assert!(!initial_snapshot.is_emergency);

    // Test status consistency
    let status = hedge.status();
    assert!(!status.is_active);
    assert!(!status.is_emergency);
    assert_eq!(status.completion, 0.0);
    assert_eq!(status.filled_size, 0.0);
    assert_eq!(status.risk_level, HedgeRiskStatus::LowRisk);
}

#[test]
fn test_state_transition_metrics() {
    // UCE-32 Q31: Type-safe state tracking with metrics accuracy
    let hedge = AtomicHedgeCapsule::create_hedge("ETHUSD", "NDAX", 2.5, 3000.0, 4000.0)
        .expect("Failed to create hedge");

    // Submit order and verify metrics update
    hedge.submit_order().expect("Failed to submit order");

    let snapshot_after_submit = hedge.get_hedge_state();
    assert_eq!(snapshot_after_submit.entry_state, OrderState::Submitted);
    assert!(snapshot_after_submit.operation_count > 0);

    // Verify status reflects state change
    let status_after_submit = hedge.status();
    assert!(status_after_submit.is_active);
    assert!(status_after_submit.completion > 0.0);
}

#[test]
fn test_emergency_metrics_accuracy() {
    // UCE-32 Q28: Simple emergency detection with accurate metrics
    let hedge = AtomicHedgeCapsule::create_hedge("ADAUSD", "NDAX", 1000.0, 1.0, 2.0)
        .expect("Failed to create hedge");

    // Trigger emergency and verify metrics
    hedge
        .emergency_stop("Test emergency")
        .expect("Failed to trigger emergency");

    let emergency_snapshot = hedge.get_hedge_state();
    assert!(emergency_snapshot.emergency_stopped);
    assert!(emergency_snapshot.is_emergency);
    assert!(emergency_snapshot.emergency_count > 0);

    let emergency_status = hedge.status();
    assert!(emergency_status.is_emergency);
    assert!(emergency_status.needs_attention());
    assert!(!emergency_status.is_safe());
    assert_eq!(emergency_status.risk_level, HedgeRiskStatus::Emergency);
}

#[test]
fn test_generation_counter_accuracy() {
    // UCE-32 Q31: Generation counters prevent TOCTOU in metrics
    let hedge = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 40000.0, 60000.0)
        .expect("Failed to create hedge");

    let initial_generation = hedge.get_hedge_state().generation;

    // Perform operations and verify generation increments
    hedge.submit_order().expect("Failed to submit order");
    let after_submit_generation = hedge.get_hedge_state().generation;
    assert!(after_submit_generation > initial_generation);

    // Emergency operations should also increment generation
    hedge
        .emergency_stop("Test emergency")
        .expect("Failed to trigger emergency");
    let after_emergency_generation = hedge.get_hedge_state().generation;
    assert!(after_emergency_generation > after_submit_generation);
}

#[test]
fn test_concurrent_metrics_accuracy() {
    // UCE-32 Q31: Lockfree metrics under concurrent access
    let hedge = Arc::new(
        AtomicHedgeCapsule::create_hedge("ETHUSD", "NDAX", 1.0, 2500.0, 3500.0)
            .expect("Failed to create hedge"),
    );

    let mut handles = vec![];
    let metrics_consistency = Arc::new(AtomicU64::new(0));

    // Spawn multiple threads accessing metrics
    for i in 0..8 {
        let hedge_clone = Arc::clone(&hedge);
        let consistency_counter = Arc::clone(&metrics_consistency);

        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let snapshot = hedge_clone.get_hedge_state();
                let status = hedge_clone.status();

                // Verify metrics consistency
                assert_eq!(snapshot.emergency_stopped, status.is_emergency);
                assert_eq!(snapshot.filled_size, status.filled_size);

                if i % 2 == 0 {
                    // Half the threads perform operations
                    let _ = hedge_clone.submit_order();
                } else {
                    // Other half just read metrics
                    consistency_counter.fetch_add(1, Ordering::Relaxed);
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // Verify final metrics are still consistent
    let final_snapshot = hedge.get_hedge_state();
    let final_status = hedge.status();
    assert_eq!(final_snapshot.emergency_stopped, final_status.is_emergency);
    assert_eq!(final_snapshot.filled_size, final_status.filled_size);
}

// ============================================================================
// ZERO OVERHEAD VERIFICATION TESTS
// ============================================================================

#[test]
fn test_metrics_collection_zero_overhead() {
    // UCE-32 Q30: Empirical validation that metrics don't impact performance
    let hedge = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
        .expect("Failed to create hedge");

    // Measure operations with metrics enabled
    let start_with_metrics = Instant::now();
    for _ in 0..TEST_ITERATIONS {
        let _ = hedge.get_hedge_state();
        let _ = hedge.status();
    }
    let with_metrics_duration = start_with_metrics.elapsed();

    // Compare with baseline (just the hedge operations)
    let start_baseline = Instant::now();
    for _ in 0..TEST_ITERATIONS {
        let _ = hedge.submit_order();
    }
    let baseline_duration = start_baseline.elapsed();

    // Calculate overhead
    let metrics_overhead_percent = ((with_metrics_duration.as_nanos() as f64
        - baseline_duration.as_nanos() as f64)
        / baseline_duration.as_nanos() as f64)
        * 100.0;

    println!("Metrics overhead: {:.2}%", metrics_overhead_percent);

    // UCE-32 Q29: Real-world constraint - metrics should add < 2% overhead
    assert!(
        metrics_overhead_percent < ZERO_OVERHEAD_THRESHOLD_PERCENT,
        "Metrics collection overhead ({:.2}%) exceeds threshold ({}%)",
        metrics_overhead_percent,
        ZERO_OVERHEAD_THRESHOLD_PERCENT
    );
}

#[test]
fn test_status_reporting_zero_overhead() {
    // UCE-32 Q32: Nightly features for zero-cost status reporting
    let hedge = AtomicHedgeCapsule::create_hedge("ETHUSD", "NDAX", 2.0, 3000.0, 4000.0)
        .expect("Failed to create hedge");

    let measurements = (0..100)
        .map(|_| {
            let start = Instant::now();
            let _ = hedge.status();
            start.elapsed().as_nanos() as f64
        })
        .collect::<Vec<_>>();

    let avg_time = measurements.iter().sum::<f64>() / measurements.len() as f64;

    // Status reporting should be extremely fast (< 50ns average)
    assert!(
        avg_time < 50.0,
        "Status reporting too slow: {:.1}ns average (threshold: 50ns)",
        avg_time
    );

    println!("Status reporting: {:.1}ns average", avg_time);
}

#[cfg(feature = "presets")]
#[test]
fn test_monitoring_config_overhead() {
    // UCE-32 Q29: Monitoring configuration should not impact core performance
    let config_high_monitoring = PresetConfig {
        monitoring: MonitoringConfig {
            detailed_tracking: true,
            performance_metrics: true,
            memory_monitoring: true,
            debug_assertions: true,
            cache_analysis: true,
        },
        ..PresetConfig::development()
    };

    let config_minimal_monitoring = PresetConfig {
        monitoring: MonitoringConfig {
            detailed_tracking: false,
            performance_metrics: false,
            memory_monitoring: false,
            debug_assertions: false,
            cache_analysis: false,
        },
        ..PresetConfig::high_frequency_trading()
    };

    // Create hedges with different monitoring levels
    let hedge_high =
        AtomicHedgeCapsule::with_development_preset("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
            .expect("Failed to create high monitoring hedge");

    let hedge_minimal =
        AtomicHedgeCapsule::with_hft_preset("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
            .expect("Failed to create minimal monitoring hedge");

    // Benchmark both configurations
    let high_monitoring_time = benchmark_hedge_operations(&hedge_high, 100);
    let minimal_monitoring_time = benchmark_hedge_operations(&hedge_minimal, 100);

    let overhead_percent =
        ((high_monitoring_time - minimal_monitoring_time) / minimal_monitoring_time) * 100.0;

    println!("Monitoring overhead: {:.2}%", overhead_percent);

    // Even detailed monitoring should add < 10% overhead
    assert!(
        overhead_percent < 10.0,
        "Detailed monitoring overhead ({:.2}%) exceeds threshold (10%)",
        overhead_percent
    );
}

// ============================================================================
// HEALTH CHECK CORRECTNESS TESTS
// ============================================================================

#[test]
fn test_health_check_accuracy() {
    // UCE-32 Q28: Simple health checks that accurately reflect system state
    let hedge = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
        .expect("Failed to create hedge");

    // Initial health check
    let initial_status = hedge.status();
    assert!(initial_status.is_safe());
    assert!(!initial_status.needs_attention());
    assert_eq!(initial_status.risk_level, HedgeRiskStatus::LowRisk);

    // Submit order and check health
    hedge.submit_order().expect("Failed to submit order");
    let active_status = hedge.status();
    assert!(active_status.is_active);
    assert!(active_status.is_safe()); // Should still be safe in normal operation

    // Trigger emergency and verify health check detects it
    hedge
        .emergency_stop("Test emergency")
        .expect("Failed to trigger emergency");
    let emergency_status = hedge.status();
    assert!(!emergency_status.is_safe());
    assert!(emergency_status.needs_attention());
    assert_eq!(emergency_status.risk_level, HedgeRiskStatus::Emergency);
}

#[test]
fn test_risk_level_accuracy() {
    // UCE-32 Q31: Type-safe risk level assessment
    let hedge = AtomicHedgeCapsule::create_hedge("ETHUSD", "NDAX", 1.0, 3000.0, 4000.0)
        .expect("Failed to create hedge");

    // Test progression through risk levels
    let initial_snapshot = hedge.get_hedge_state();
    assert_eq!(initial_snapshot.risk_status(), HedgeRiskStatus::LowRisk);

    // Simulate multiple emergency events to increase risk
    for _ in 0..2 {
        hedge
            .emergency_stop("Test emergency")
            .expect("Failed to trigger emergency");
        // Reset for next iteration (in real implementation)
    }

    let elevated_snapshot = hedge.get_hedge_state();
    // After multiple emergencies, risk should be elevated
    assert!(matches!(
        elevated_snapshot.risk_status(),
        HedgeRiskStatus::MediumRisk | HedgeRiskStatus::HighRisk | HedgeRiskStatus::Emergency
    ));
}

#[test]
fn test_completion_percentage_accuracy() {
    // UCE-32 Q28: Simple completion tracking
    let hedge = AtomicHedgeCapsule::create_hedge("ADAUSD", "NDAX", 1000.0, 1.0, 2.0)
        .expect("Failed to create hedge");

    // Initial completion should be 0%
    let initial_snapshot = hedge.get_hedge_state();
    assert_eq!(initial_snapshot.completion_percentage(), 0.0);

    // After submission, should show some progress
    hedge.submit_order().expect("Failed to submit order");
    let submitted_snapshot = hedge.get_hedge_state();
    assert!(submitted_snapshot.completion_percentage() > 0.0);
    assert!(submitted_snapshot.completion_percentage() < 1.0);
}

// ============================================================================
// DIAGNOSTIC USEFULNESS TESTS
// ============================================================================

#[test]
fn test_diagnostic_information_quality() {
    // UCE-32 Q30: Diagnostics provide actionable insights
    let hedge = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
        .expect("Failed to create hedge");

    // Test status descriptions are meaningful
    let status = hedge.status();
    let description = status.description();
    assert!(!description.is_empty());
    assert!(description.len() > 3); // More than just "OK" or similar

    // Test display formatting includes useful information
    let status_display = format!("{}", status);
    assert!(status_display.contains("HedgeStatus"));
    assert!(status_display.contains("complete"));
    assert!(status_display.contains("filled"));
    assert!(status_display.contains("risk"));

    // Test snapshot display is comprehensive
    let snapshot = hedge.get_hedge_state();
    let snapshot_display = format!("{}", snapshot);
    assert!(snapshot_display.contains("HedgeState"));
    assert!(snapshot_display.contains("entry"));
    assert!(snapshot_display.contains("stop"));
    assert!(snapshot_display.contains("target"));
    assert!(snapshot_display.contains("filled"));
}

#[test]
fn test_error_diagnostic_quality() {
    // UCE-32 Q28: Error diagnostics provide clear guidance

    // Test validation error diagnostics
    let validation_error = HedgeError::invalid_value("size", "-1.0", "Size must be positive");
    assert!(validation_error.is_recoverable());
    assert!(!validation_error.is_critical());

    let action = validation_error.suggested_action();
    assert!(!action.is_empty());
    assert!(action.contains("parameters") || action.contains("validation"));

    // Test emergency error diagnostics
    let emergency_error = HedgeError::emergency("Risk threshold exceeded");
    assert!(!emergency_error.is_recoverable());
    assert!(emergency_error.is_critical());

    let emergency_action = emergency_error.suggested_action();
    assert!(emergency_action.contains("emergency"));

    // Test error categorization
    use atomic_hedge_capsule::types::{ErrorCategory, HedgeResultExt};

    let timeout_result: Result<(), HedgeError> = Err(HedgeError::Timeout);
    assert_eq!(
        timeout_result.error_category(),
        Some(ErrorCategory::Transient)
    );
    assert!(timeout_result.is_recoverable());

    let system_result: Result<(), HedgeError> = Err(HedgeError::MemoryOrderingViolation {
        operation: "test".to_string(),
    });
    assert_eq!(system_result.error_category(), Some(ErrorCategory::System));
    assert!(!system_result.is_recoverable());
}

// ============================================================================
// MONITORING INTEGRATION TESTS
// ============================================================================

#[cfg(feature = "presets")]
#[test]
fn test_monitoring_integration() {
    // UCE-32 Q31: Monitoring integrates seamlessly with core operations
    let config = PresetConfig {
        monitoring: MonitoringConfig {
            detailed_tracking: true,
            performance_metrics: true,
            memory_monitoring: true,
            debug_assertions: true,
            cache_analysis: true,
        },
        ..PresetConfig::development()
    };

    let hedge =
        AtomicHedgeCapsule::with_development_preset("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
            .expect("Failed to create monitored hedge");

    // Perform operations and verify monitoring captures them
    hedge.submit_order().expect("Failed to submit order");

    let snapshot = hedge.get_hedge_state();
    assert!(snapshot.operation_count > 0);
    assert!(snapshot.generation > 0);

    // Test that monitoring doesn't interfere with normal operations
    let execution_result = hedge.execute_hedge(0.5);
    match execution_result {
        Ok(result) => {
            assert!(result.entry_filled >= 0.0);
        }
        Err(e) => {
            // Error is acceptable, but should be properly categorized
            assert!(!e.suggested_action().is_empty());
        }
    }
}

#[test]
fn test_metrics_temporal_accuracy() {
    // UCE-32 Q30: Metrics accurately track changes over time
    let hedge = AtomicHedgeCapsule::create_hedge("ETHUSD", "NDAX", 1.0, 3000.0, 4000.0)
        .expect("Failed to create hedge");

    let mut snapshots = vec![];

    // Collect snapshots over time
    snapshots.push(hedge.get_hedge_state());

    hedge.submit_order().expect("Failed to submit order");
    snapshots.push(hedge.get_hedge_state());

    hedge
        .emergency_stop("Test emergency")
        .expect("Failed to trigger emergency");
    snapshots.push(hedge.get_hedge_state());

    // Verify temporal consistency
    assert!(snapshots[0].operation_count <= snapshots[1].operation_count);
    assert!(snapshots[1].operation_count <= snapshots[2].operation_count);

    assert!(snapshots[0].generation <= snapshots[1].generation);
    assert!(snapshots[1].generation <= snapshots[2].generation);

    // Emergency should be reflected in final snapshot
    assert!(!snapshots[0].emergency_stopped);
    assert!(!snapshots[1].emergency_stopped);
    assert!(snapshots[2].emergency_stopped);
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn benchmark_hedge_operations(hedge: &AtomicHedgeCapsule, iterations: usize) -> f64 {
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = hedge.get_hedge_state();
        let _ = hedge.status();
        let _ = hedge.submit_order();
    }
    start.elapsed().as_nanos() as f64 / iterations as f64
}

// ============================================================================
// ADVANCED OBSERVABILITY TESTS
// ============================================================================

#[test]
fn test_observability_under_stress() {
    // UCE-32 Q29: Observability remains accurate under high load
    let hedge = Arc::new(
        AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
            .expect("Failed to create hedge"),
    );

    let stress_duration = Duration::from_millis(100);
    let start_time = Instant::now();
    let mut handles = vec![];

    // High-frequency operations
    for _ in 0..4 {
        let hedge_clone = Arc::clone(&hedge);
        let handle = thread::spawn(move || {
            let mut ops = 0;
            while start_time.elapsed() < stress_duration {
                let _ = hedge_clone.submit_order();
                let _ = hedge_clone.get_hedge_state();
                ops += 1;
            }
            ops
        });
        handles.push(handle);
    }

    // Concurrent metric readers
    for _ in 0..4 {
        let hedge_clone = Arc::clone(&hedge);
        let handle = thread::spawn(move || {
            let mut reads = 0;
            while start_time.elapsed() < stress_duration {
                let snapshot = hedge_clone.get_hedge_state();
                let status = hedge_clone.status();

                // Verify consistency under stress
                assert_eq!(snapshot.emergency_stopped, status.is_emergency);
                reads += 1;
            }
            reads
        });
        handles.push(handle);
    }

    // Wait for stress test completion
    let mut total_ops = 0;
    for handle in handles {
        total_ops += handle.join().expect("Thread panicked");
    }

    println!("Stress test completed: {} total operations", total_ops);

    // Final consistency check
    let final_snapshot = hedge.get_hedge_state();
    let final_status = hedge.status();
    assert_eq!(final_snapshot.emergency_stopped, final_status.is_emergency);
    assert_eq!(final_snapshot.filled_size, final_status.filled_size);
}

#[test]
fn test_observer_effect_minimal() {
    // UCE-32 Q29: Observing metrics should not significantly affect system behavior
    let hedge1 = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
        .expect("Failed to create hedge1");

    let hedge2 = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
        .expect("Failed to create hedge2");

    // Measure operations without observation
    let start_unobserved = Instant::now();
    for _ in 0..TEST_ITERATIONS {
        let _ = hedge1.submit_order();
    }
    let unobserved_duration = start_unobserved.elapsed();

    // Measure operations with heavy observation
    let start_observed = Instant::now();
    for _ in 0..TEST_ITERATIONS {
        let _ = hedge2.submit_order();
        let _ = hedge2.snapshot();
        let _ = hedge2.status();
        let _ = hedge2.snapshot();
    }
    let observed_duration = start_observed.elapsed();

    let observer_effect = ((observed_duration.as_nanos() as f64
        - unobserved_duration.as_nanos() as f64)
        / unobserved_duration.as_nanos() as f64)
        * 100.0;

    println!("Observer effect: {:.2}%", observer_effect);

    // Observer effect should be minimal (< 5%)
    assert!(
        observer_effect < 5.0,
        "Observer effect ({:.2}%) exceeds threshold (5%)",
        observer_effect
    );
}
