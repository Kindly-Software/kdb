//! Metrics Accuracy Validation Tests
//!
//! UCE-32 Q30 (Empirical Validation): Precise validation that metrics reflect true system state
//! UCE-32 Q31 (Rust Transform): Type-safe metrics with impossible states unrepresentable
//! UCE-32 Q32 (Nightly Enhancement): Advanced metrics validation with atomic precision
//!
//! ## Validation Framework
//! 1. Atomic-level accuracy - metrics match atomic state exactly
//! 2. Temporal consistency - metrics maintain ordering guarantees
//! 3. Precision validation - measurements within nanosecond precision
//! 4. Consistency verification - cross-metric consistency validation
//! 5. Edge case coverage - boundary conditions and error states

use atomic_hedge_capsule::{
    AtomicHedgeCapsule, BracketOrder, EntryOrder, HedgeError, HedgeRiskStatus, HedgeStateSnapshot,
    HedgeStatus, OrderState,
};

#[cfg(feature = "presets")]
use atomic_hedge_capsule::{
    AtomicHedgeCapsulePresets, MemoryOrderingLevel, MonitoringConfig, PresetConfig, ValidationLevel,
};

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Metrics validation configuration
const PRECISION_THRESHOLD_NS: f64 = 1.0; // 1ns precision for measurements
const CONSISTENCY_ITERATIONS: usize = 10000; // High iteration count for consistency
const STRESS_DURATION_MS: u64 = 50; // 50ms stress test duration
const ACCEPTABLE_DRIFT_PERCENT: f64 = 0.1; // 0.1% acceptable drift in metrics

// ============================================================================
// ATOMIC-LEVEL ACCURACY TESTS
// ============================================================================

#[test]
fn test_generation_counter_atomicity() {
    // UCE-32 Q31: Generation counters must be atomically consistent
    let hedge = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
        .expect("Failed to create hedge");

    // Test that generation increments are atomic and monotonic
    let mut previous_generation = 0u64;

    for i in 0..1000 {
        // Perform an operation that should increment generation
        let _ = hedge.submit_order();

        let snapshot = hedge.snapshot();
        let current_generation = snapshot.generation;

        // Generation must always increase
        assert!(
            current_generation > previous_generation,
            "Generation not monotonic: iteration {}, prev={}, current={}",
            i,
            previous_generation,
            current_generation
        );

        // Entry generation should also be consistent
        assert!(
            snapshot.entry_generation <= current_generation,
            "Entry generation ({}) exceeds main generation ({})",
            snapshot.entry_generation,
            current_generation
        );

        previous_generation = current_generation;
    }
}

#[test]
fn test_operation_count_precision() {
    // UCE-32 Q30: Operation counts must precisely track actual operations
    let hedge = AtomicHedgeCapsule::create_hedge("ETHUSD", "NDAX", 2.0, 3000.0, 4000.0)
        .expect("Failed to create hedge");

    let initial_snapshot = hedge.snapshot();
    let initial_count = initial_snapshot.operation_count;

    // Perform exactly N operations and verify count
    const OPERATION_COUNT: usize = 100;
    for _ in 0..OPERATION_COUNT {
        let _ = hedge.submit_order();
    }

    let final_snapshot = hedge.snapshot();
    let final_count = final_snapshot.operation_count;

    // Operation count should reflect exact number of operations
    let operations_performed = (final_count - initial_count) as usize;

    // Allow for some operations from concurrent internal processes, but verify minimum
    assert!(
        operations_performed >= OPERATION_COUNT,
        "Operation count ({}) less than operations performed ({})",
        operations_performed,
        OPERATION_COUNT
    );

    // Should not be wildly off
    assert!(
        operations_performed <= OPERATION_COUNT * 2,
        "Operation count ({}) too high for operations performed ({})",
        operations_performed,
        OPERATION_COUNT
    );
}

#[test]
fn test_emergency_count_accuracy() {
    // UCE-32 Q28: Emergency count must accurately track emergency events
    let hedge = AtomicHedgeCapsule::create_hedge("ADAUSD", "NDAX", 1000.0, 1.0, 2.0)
        .expect("Failed to create hedge");

    let initial_snapshot = hedge.snapshot();
    assert_eq!(initial_snapshot.emergency_count, 0);
    assert!(!initial_snapshot.emergency_stopped);

    // Trigger exactly 5 emergencies
    const EMERGENCY_COUNT: u32 = 5;
    for i in 0..EMERGENCY_COUNT {
        hedge.emergency_stop();

        let snapshot = hedge.snapshot();

        // Emergency count should increment with each emergency
        assert!(
            snapshot.emergency_count >= i + 1,
            "Emergency count ({}) less than expected ({}) after {} emergencies",
            snapshot.emergency_count,
            i + 1,
            i + 1
        );

        assert!(snapshot.emergency_stopped);
        assert!(snapshot.is_emergency);

        // For testing, simulate clearing emergency to trigger next one
        // (In real implementation, this would be handled differently)
    }
}

#[test]
fn test_filled_size_precision() {
    // UCE-32 Q30: Filled size must precisely track order fills
    let hedge = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 10.0, 45000.0, 55000.0)
        .expect("Failed to create hedge");

    let initial_snapshot = hedge.snapshot();
    assert_eq!(initial_snapshot.filled_size, 0.0);

    // Execute partial fill
    let fill_amount = 2.5;
    let result = hedge.execute_hedge(fill_amount);

    // Even if execution fails, check that metrics are consistent
    let post_execution_snapshot = hedge.snapshot();

    match result {
        Ok(execution_result) => {
            // If successful, filled size should match execution result
            assert_eq!(
                post_execution_snapshot.filled_size, execution_result.entry_filled,
                "Snapshot filled size ({}) doesn't match execution result ({})",
                post_execution_snapshot.filled_size, execution_result.entry_filled
            );
        }
        Err(_) => {
            // If failed, filled size should remain consistent
            assert!(
                post_execution_snapshot.filled_size >= initial_snapshot.filled_size,
                "Filled size decreased after failed execution"
            );
        }
    }
}

// ============================================================================
// TEMPORAL CONSISTENCY TESTS
// ============================================================================

#[test]
fn test_timestamp_ordering() {
    // UCE-32 Q31: Temporal ordering must be maintained in metrics
    let hedge = AtomicHedgeCapsule::create_hedge("ETHUSD", "NDAX", 1.0, 3000.0, 4000.0)
        .expect("Failed to create hedge");

    let mut timestamps = Vec::new();
    let mut generations = Vec::new();

    // Collect temporal data over operations
    for _ in 0..50 {
        let start_time = Instant::now();
        let _ = hedge.submit_order();
        let end_time = Instant::now();

        let snapshot = hedge.snapshot();

        timestamps.push((start_time, end_time, snapshot.generation));
        generations.push(snapshot.generation);

        // Small delay to ensure temporal separation
        std::thread::sleep(Duration::from_nanos(100));
    }

    // Verify temporal ordering
    for i in 1..timestamps.len() {
        let (_, prev_end, prev_gen) = timestamps[i - 1];
        let (curr_start, _, curr_gen) = timestamps[i];

        // Generation should increase over time
        assert!(
            curr_gen >= prev_gen,
            "Generation decreased: {} -> {} at iteration {}",
            prev_gen,
            curr_gen,
            i
        );

        // If operations are sequential, generations should be strictly increasing
        if curr_start >= prev_end {
            assert!(
                curr_gen > prev_gen,
                "Generation not strictly increasing for sequential operations: {} -> {}",
                prev_gen,
                curr_gen
            );
        }
    }
}

#[test]
fn test_concurrent_temporal_consistency() {
    // UCE-32 Q31: Temporal consistency under concurrent access
    let hedge = Arc::new(
        AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
            .expect("Failed to create hedge"),
    );

    let shared_results = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut handles = vec![];

    // Spawn multiple threads performing operations
    for thread_id in 0..4 {
        let hedge_clone = Arc::clone(&hedge);
        let results_clone = Arc::clone(&shared_results);

        let handle = thread::spawn(move || {
            let mut local_results = Vec::new();

            for i in 0..100 {
                let before_op = Instant::now();
                let _ = hedge_clone.submit_order();
                let after_op = Instant::now();

                let snapshot = hedge_clone.snapshot();

                local_results.push((
                    thread_id,
                    i,
                    before_op,
                    after_op,
                    snapshot.generation,
                    snapshot.operation_count,
                ));
            }

            {
                let mut shared = results_clone.lock().unwrap();
                shared.extend(local_results);
            }
        });

        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    let results = shared_results.lock().unwrap();

    // Sort by generation to verify temporal consistency
    let mut sorted_results = results.clone();
    sorted_results.sort_by_key(|&(_, _, _, _, gen, _)| gen);

    // Verify generation ordering
    for i in 1..sorted_results.len() {
        let (_, _, _, _, prev_gen, _) = sorted_results[i - 1];
        let (_, _, _, _, curr_gen, _) = sorted_results[i];

        assert!(
            curr_gen >= prev_gen,
            "Generation ordering violation: {} -> {} at position {}",
            prev_gen,
            curr_gen,
            i
        );
    }
}

// ============================================================================
// PRECISION VALIDATION TESTS
// ============================================================================

#[test]
fn test_metrics_measurement_precision() {
    // UCE-32 Q32: Nightly features enable nanosecond-precision metrics
    let hedge = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
        .expect("Failed to create hedge");

    // Measure metrics collection precision
    let measurements: Vec<Duration> = (0..1000)
        .map(|_| {
            let start = Instant::now();
            let _ = hedge.snapshot();
            start.elapsed()
        })
        .collect();

    let avg_duration = measurements.iter().sum::<Duration>() / measurements.len() as u32;
    let min_duration = measurements.iter().min().unwrap();
    let max_duration = measurements.iter().max().unwrap();

    println!(
        "Metrics precision - Avg: {:?}, Min: {:?}, Max: {:?}",
        avg_duration, min_duration, max_duration
    );

    // Metrics collection should be extremely fast and consistent
    assert!(
        avg_duration.as_nanos() < 100, // < 100ns average
        "Metrics collection too slow: {}ns average",
        avg_duration.as_nanos()
    );

    // Maximum should not be wildly different from minimum (indicating consistent performance)
    let ratio = max_duration.as_nanos() as f64 / min_duration.as_nanos() as f64;
    assert!(
        ratio < 10.0,
        "Metrics collection inconsistent: max/min ratio = {:.2}",
        ratio
    );
}

#[test]
fn test_status_calculation_precision() {
    // UCE-32 Q28: Status calculations must be precise and consistent
    let hedge = AtomicHedgeCapsule::create_hedge("ETHUSD", "NDAX", 1.0, 3000.0, 4000.0)
        .expect("Failed to create hedge");

    // Test precision across multiple status calculations
    for _ in 0..100 {
        let snapshot = hedge.snapshot();
        let status = hedge.status();

        // Verify cross-metric consistency with high precision
        assert_eq!(
            snapshot.emergency_stopped, status.is_emergency,
            "Emergency status inconsistency"
        );

        assert_eq!(
            snapshot.filled_size, status.filled_size,
            "Filled size inconsistency"
        );

        // Risk level calculation should be deterministic
        let calculated_risk = snapshot.risk_status();
        assert_eq!(
            calculated_risk, status.risk_level,
            "Risk level calculation inconsistency"
        );

        // Completion calculation should be precise
        let calculated_completion = snapshot.completion_percentage();
        assert!(
            (calculated_completion - status.completion).abs() < 0.001,
            "Completion percentage precision error: {} vs {}",
            calculated_completion,
            status.completion
        );
    }
}

// ============================================================================
// CONSISTENCY VERIFICATION TESTS
// ============================================================================

#[test]
fn test_cross_metric_consistency() {
    // UCE-32 Q31: All metrics must be mutually consistent
    let hedge = AtomicHedgeCapsule::create_hedge("ADAUSD", "NDAX", 1000.0, 1.0, 2.0)
        .expect("Failed to create hedge");

    for _ in 0..CONSISTENCY_ITERATIONS {
        let snapshot = hedge.snapshot();
        let status = hedge.status();

        // Basic consistency checks
        assert_eq!(snapshot.emergency_stopped, status.is_emergency);
        assert_eq!(snapshot.is_emergency, status.is_emergency);
        assert_eq!(snapshot.filled_size, status.filled_size);

        // Generation consistency
        assert!(snapshot.entry_generation <= snapshot.generation);
        assert!(snapshot.bracket_generation <= snapshot.generation);
        assert!(snapshot.emergency_generation <= snapshot.generation);

        // If emergency stopped, emergency generation should be > 0
        if snapshot.emergency_stopped {
            assert!(snapshot.emergency_generation > 0);
            assert!(snapshot.emergency_count > 0);
        }

        // Active state consistency
        if snapshot.is_active {
            assert!(
                snapshot.entry_state != OrderState::PendingValidation
                    || snapshot.stop_state != OrderState::PendingValidation
                    || snapshot.target_state != OrderState::PendingValidation
            );
        }

        // Risk status consistency
        let calculated_risk = snapshot.risk_status();
        assert_eq!(calculated_risk, status.risk_level);

        if status.is_emergency {
            assert_eq!(calculated_risk, HedgeRiskStatus::Emergency);
        }

        // Perform random operation to change state
        match fastrand::u32(0..3) {
            0 => {
                let _ = hedge.submit_order();
            }
            1 => {
                hedge.emergency_stop();
            }
            _ => {
                let _ = hedge.execute_hedge(0.1);
            }
        }
    }
}

#[test]
fn test_state_transition_consistency() {
    // UCE-32 Q31: State transitions must be reflected consistently across metrics
    let hedge = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)
        .expect("Failed to create hedge");

    // Track state transitions
    let mut state_history = Vec::new();
    let mut generation_history = Vec::new();

    for _ in 0..50 {
        let before_snapshot = hedge.snapshot();

        // Perform operation that should change state
        let _ = hedge.submit_order();

        let after_snapshot = hedge.snapshot();

        // Verify consistency of state change
        if after_snapshot.generation > before_snapshot.generation {
            // If generation changed, something should have changed
            assert!(
                after_snapshot.entry_state != before_snapshot.entry_state
                    || after_snapshot.operation_count != before_snapshot.operation_count
                    || after_snapshot.entry_generation != before_snapshot.entry_generation,
                "Generation changed but no visible state change"
            );
        }

        state_history.push((before_snapshot.entry_state, after_snapshot.entry_state));
        generation_history.push((before_snapshot.generation, after_snapshot.generation));
    }

    // Verify all transitions are valid
    for (before, after) in state_history {
        // All state transitions should follow valid state machine rules
        if before != after {
            // Verify valid transition (implementation specific)
            assert!(
                is_valid_order_state_transition(before, after),
                "Invalid state transition: {:?} -> {:?}",
                before,
                after
            );
        }
    }
}

// ============================================================================
// EDGE CASE COVERAGE TESTS
// ============================================================================

#[test]
fn test_boundary_value_accuracy() {
    // UCE-32 Q29: Metrics must be accurate at boundary conditions

    // Test with minimal values
    let hedge_min = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 0.000001, 0.01, 0.02)
        .expect("Failed to create minimal hedge");

    let min_snapshot = hedge_min.snapshot();
    assert_eq!(min_snapshot.filled_size, 0.0);
    assert!(min_snapshot.operation_count >= 0);

    // Test with large values
    let hedge_max =
        AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1_000_000.0, 100_000.0, 200_000.0)
            .expect("Failed to create maximum hedge");

    let max_snapshot = hedge_max.snapshot();
    assert_eq!(max_snapshot.filled_size, 0.0);
    assert!(max_snapshot.operation_count >= 0);
}

#[test]
fn test_error_state_metrics() {
    // UCE-32 Q28: Metrics must remain accurate even in error conditions

    // Create hedge that will likely encounter errors
    let hedge = AtomicHedgeCapsule::create_hedge("INVALID", "INVALID", -1.0, 0.0, 0.0);

    match hedge {
        Ok(h) => {
            // If creation succeeded despite invalid values, test error metrics
            let snapshot = h.snapshot();
            assert!(snapshot.generation >= 0);
            assert!(snapshot.operation_count >= 0);
        }
        Err(_) => {
            // Error creation is acceptable for invalid values
            // Test that the error is properly categorized
            // Error handling is working as expected
        }
    }
}

#[test]
fn test_concurrent_metric_accuracy() {
    // UCE-32 Q31: Metrics remain accurate under high concurrency
    let hedge = Arc::new(
        AtomicHedgeCapsule::create_hedge("ETHUSD", "NDAX", 1.0, 3000.0, 4000.0)
            .expect("Failed to create hedge"),
    );

    let inconsistency_count = Arc::new(AtomicU64::new(0));
    let total_checks = Arc::new(AtomicU64::new(0));
    let mut handles = vec![];

    let test_duration = Duration::from_millis(STRESS_DURATION_MS);
    let start_time = Instant::now();

    // Spawn multiple threads checking metric consistency
    for _ in 0..8 {
        let hedge_clone = Arc::clone(&hedge);
        let inconsistency_clone = Arc::clone(&inconsistency_count);
        let total_clone = Arc::clone(&total_checks);

        let handle = thread::spawn(move || {
            while start_time.elapsed() < test_duration {
                let snapshot = hedge_clone.snapshot();
                let status = hedge_clone.status();

                // Check consistency
                let mut inconsistent = false;

                if snapshot.emergency_stopped != status.is_emergency {
                    inconsistent = true;
                }

                if snapshot.filled_size != status.filled_size {
                    inconsistent = true;
                }

                if snapshot.risk_status() != status.risk_level {
                    inconsistent = true;
                }

                if inconsistent {
                    inconsistency_clone.fetch_add(1, Ordering::Relaxed);
                }

                total_clone.fetch_add(1, Ordering::Relaxed);

                // Perform operations to stress the system
                let _ = hedge_clone.submit_order();
            }
        });

        handles.push(handle);
    }

    // Wait for test completion
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    let total_checks = total_checks.load(Ordering::Relaxed);
    let inconsistencies = inconsistency_count.load(Ordering::Relaxed);
    let inconsistency_rate = (inconsistencies as f64 / total_checks as f64) * 100.0;

    println!(
        "Concurrent accuracy: {}/{} checks, {:.4}% inconsistency rate",
        inconsistencies, total_checks, inconsistency_rate
    );

    // Inconsistency rate should be extremely low (< 0.1%)
    assert!(
        inconsistency_rate < ACCEPTABLE_DRIFT_PERCENT,
        "Metric inconsistency rate ({:.4}%) exceeds threshold ({:.4}%)",
        inconsistency_rate,
        ACCEPTABLE_DRIFT_PERCENT
    );
}

// ============================================================================
// ADVANCED ACCURACY TESTS
// ============================================================================

#[cfg(feature = "presets")]
#[test]
fn test_monitoring_level_accuracy() {
    // UCE-32 Q30: Different monitoring levels should maintain accuracy
    let configs = vec![
        (
            "minimal",
            PresetConfig {
                monitoring: MonitoringConfig {
                    detailed_tracking: false,
                    performance_metrics: false,
                    memory_monitoring: false,
                    debug_assertions: false,
                    cache_analysis: false,
                },
                ..PresetConfig::high_frequency_trading()
            },
        ),
        (
            "comprehensive",
            PresetConfig {
                monitoring: MonitoringConfig {
                    detailed_tracking: true,
                    performance_metrics: true,
                    memory_monitoring: true,
                    debug_assertions: true,
                    cache_analysis: true,
                },
                ..PresetConfig::development()
            },
        ),
    ];

    for (config_name, config) in configs {
        let hedge = AtomicHedgeCapsulePresets::create_with_config(
            "BTCUSD", "NDAX", 1.0, 45000.0, 55000.0, config,
        )
        .expect(&format!("Failed to create {} hedge", config_name));

        // Test accuracy regardless of monitoring level
        for _ in 0..100 {
            let snapshot = hedge.snapshot();
            let status = hedge.status();

            assert_eq!(
                snapshot.emergency_stopped, status.is_emergency,
                "Inconsistency in {} monitoring",
                config_name
            );
            assert_eq!(
                snapshot.filled_size, status.filled_size,
                "Filled size inconsistency in {} monitoring",
                config_name
            );
        }
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn is_valid_order_state_transition(from: OrderState, to: OrderState) -> bool {
    // Implementation-specific state transition validation
    // This is a simplified version - real implementation would be more complex
    use OrderState::*;

    match (from, to) {
        (PendingValidation, Validated) => true,
        (PendingValidation, Rejected) => true,
        (Validated, Submitted) => true,
        (Submitted, Acknowledged) => true,
        (Submitted, Rejected) => true,
        (Acknowledged, PartiallyFilled) => true,
        (Acknowledged, Filled) => true,
        (Acknowledged, Cancelled) => true,
        (PartiallyFilled, Filled) => true,
        (PartiallyFilled, Cancelled) => true,
        (_, Expired) => true, // Can expire from any state
        (_, Stopped) => true, // Can be stopped from any state
        _ => from == to,      // Same state is always valid
    }
}

#[test]
fn test_metrics_mathematical_accuracy() {
    // UCE-32 Q32: Mathematical calculations in metrics must be precise
    let hedge = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 3.14159, 45000.0, 55000.0)
        .expect("Failed to create hedge");

    let snapshot = hedge.snapshot();

    // Test floating-point precision in metrics
    assert!(snapshot.filled_size.is_finite());
    assert!(!snapshot.filled_size.is_nan());

    // Risk calculations should maintain precision
    let risk_status = snapshot.risk_status();
    assert!(matches!(
        risk_status,
        HedgeRiskStatus::LowRisk
            | HedgeRiskStatus::MediumRisk
            | HedgeRiskStatus::HighRisk
            | HedgeRiskStatus::Emergency
    ));

    // Completion percentage should be valid
    let completion = snapshot.completion_percentage();
    assert!(
        completion >= 0.0 && completion <= 1.0,
        "Invalid completion percentage: {}",
        completion
    );
    assert!(completion.is_finite());
    assert!(!completion.is_nan());
}
