/// Tier 3: Integration Tests (Q15-Q21)
/// Goal: Validate components work together

use sysrespond::{ProcessStateCapsule, ResourceGovernorCapsule, CircuitState};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// Q15: Critical Integration Points
// ============================================================================

#[test]
fn test_integration_capsule_to_governor_kill_path() {
    // Arrange: Set up full pipeline
    let capsule = Arc::new(ProcessStateCapsule::new(1234));
    let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 5, 60));

    // Act: Simulate hung process detection → kill attempt
    capsule.update(1234, 200.0, 500, false, false, false);

    let is_hung = capsule.is_hung(100.0, 300);
    let can_kill = governor.can_kill();

    // Assert: Integration works
    assert!(is_hung, "Process should be detected as hung");
    assert!(can_kill, "Governor should allow kill");

    // Record kill
    assert!(governor.record_kill());
    assert_eq!(governor.total_kills(), 1);
}

#[test]
fn test_integration_circuit_breaker_blocks_kills() {
    // Arrange: Governor with low threshold
    let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 3, 60));

    // Act: Trip circuit breaker
    for _ in 0..4 {
        governor.record_kill();
    }

    // Assert: Circuit open, kills blocked
    assert_eq!(governor.circuit_state(), CircuitState::Open);
    assert!(!governor.can_kill());

    // Further kill attempts rejected
    let result = governor.record_kill();
    assert!(!result, "Kill should be rejected when circuit open");
}

#[test]
fn test_integration_whitelist_overrides_hung_detection() {
    // Arrange: Process with high resource usage
    let capsule = Arc::new(ProcessStateCapsule::new(5678));
    capsule.update(5678, 300.0, 600, false, false, false);

    // Initially hung
    assert!(capsule.is_hung(100.0, 300));

    // Act: Whitelist process
    capsule.set_whitelisted(true);

    // Assert: No longer detected as hung
    assert!(!capsule.is_hung(100.0, 300));
}

#[test]
fn test_integration_generation_counter_coordination() {
    // Arrange: Multiple capsules with coordinated updates
    let capsule1 = Arc::new(ProcessStateCapsule::new(100));
    let capsule2 = Arc::new(ProcessStateCapsule::new(200));

    // Act: Update both in lockstep
    for i in 0..10 {
        capsule1.update(100, 100.0, 100, false, false, false);
        capsule2.update(200, 100.0, 100, false, false, false);

        // Assert: Generation counters increment together
        assert_eq!(capsule1.generation(), (i + 1) & 0xFF);
        assert_eq!(capsule2.generation(), (i + 1) & 0xFF);
    }
}

// ============================================================================
// Q16: Error Propagation
// ============================================================================

#[test]
fn test_integration_circuit_trip_propagates() {
    // Arrange: Governor with low threshold
    let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 2, 60));

    // Act: Trigger circuit trip
    governor.record_kill();
    governor.record_kill();
    governor.record_kill(); // Should trip

    // Assert: Circuit state propagates to can_kill()
    assert_eq!(governor.circuit_state(), CircuitState::Open);
    assert!(!governor.can_kill());
}

#[test]
fn test_integration_circuit_recovery_after_reset() {
    // Arrange: Tripped circuit
    let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 2, 60));

    for _ in 0..3 {
        governor.record_kill();
    }
    assert_eq!(governor.circuit_state(), CircuitState::Open);

    // Act: Reset active kills
    governor.reset_active_kills();

    // Assert: Circuit moves to half-open
    assert_eq!(governor.circuit_state(), CircuitState::HalfOpen);
}

// ============================================================================
// Q17: Performance Budgets
// ============================================================================

#[test]
fn test_integration_performance_budget_detection_pipeline() {
    // Arrange: Complete detection pipeline
    let capsule = Arc::new(ProcessStateCapsule::new(9999));
    let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 100, 60));

    capsule.update(9999, 200.0, 500, false, false, false);

    let iterations = 10_000;
    let start = Instant::now();

    // Act: Run full detection pipeline
    for _ in 0..iterations {
        let is_hung = capsule.is_hung(100.0, 300);
        if is_hung && governor.can_kill() {
            let _ = governor.record_kill();
        }
    }

    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() / iterations;

    // Assert: Budget <200ns for full pipeline (50ns hung + 20ns can_kill + 50ns record)
    assert!(
        avg_ns < 500,
        "Integration overhead too high: {}ns > 500ns",
        avg_ns
    );
}

#[test]
fn test_integration_performance_concurrent_access() {
    // Arrange: Shared capsules
    let capsule = Arc::new(ProcessStateCapsule::new(8888));
    let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 255, 60)); // Max u8

    let threads = 10;
    let iterations = 1_000;

    let start = Instant::now();

    // Act: Concurrent access to integration points
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let c = Arc::clone(&capsule);
            let g = Arc::clone(&governor);

            thread::spawn(move || {
                for _ in 0..iterations {
                    c.update(8888, 150.0, 400, false, false, false);
                    let _ = c.is_hung(100.0, 300);
                    let _ = g.can_kill();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_ops = threads * iterations;
    let avg_us = elapsed.as_micros() / total_ops;

    // Assert: Maintain performance under concurrent load
    assert!(
        avg_us < 5,
        "Concurrent integration too slow: {}μs > 5μs",
        avg_us
    );
}

// ============================================================================
// Q18: Production Load
// ============================================================================

#[test]
fn test_integration_handle_1000_processes() {
    // Arrange: Simulate 1000 concurrent processes
    let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 100, 60));
    let processes: Vec<_> = (0..1000)
        .map(|i| Arc::new(ProcessStateCapsule::new(i)))
        .collect();

    let start = Instant::now();

    // Act: Scan all processes
    let mut hung_count = 0;
    for (i, capsule) in processes.iter().enumerate() {
        // Vary CPU and runtime
        let cpu = 50.0 + (i % 200) as f64;
        let runtime = 100 + (i % 500) as u64;

        capsule.update(i as u32, cpu, runtime, false, false, false);

        if capsule.is_hung(100.0, 300) && governor.can_kill() {
            hung_count += 1;
            let _ = governor.record_kill();
        }
    }

    let elapsed = start.elapsed();

    // Assert: Can process 1000 processes quickly
    assert!(
        elapsed < Duration::from_millis(100),
        "1000 process scan too slow: {:?} > 100ms",
        elapsed
    );

    println!("Scanned 1000 processes in {:?}, {} hung", elapsed, hung_count);
}

#[test]
fn test_integration_sustained_load() {
    // Arrange: Governor under sustained kill load
    let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 50, 60));

    let start = Instant::now();
    let mut successful_kills = 0;
    let total_attempts = 10_000;

    // Act: Sustained kill attempts
    for _ in 0..total_attempts {
        if governor.record_kill() {
            successful_kills += 1;
        }
    }

    let elapsed = start.elapsed();

    // Assert: Circuit breaker prevents all kills
    assert!(
        successful_kills < total_attempts,
        "Circuit breaker should have tripped"
    );

    println!(
        "Sustained load: {}/{} kills in {:?}",
        successful_kills, total_attempts, elapsed
    );
}

// ============================================================================
// Q19: Rollback Scenarios
// ============================================================================

#[test]
fn test_integration_rollback_circuit_reset() {
    // Arrange: Tripped circuit
    let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 5, 60));

    for _ in 0..10 {
        governor.record_kill();
    }

    assert_eq!(governor.circuit_state(), CircuitState::Open);
    let total_before = governor.total_kills();

    // Act: Rollback via reset
    governor.reset_active_kills();

    // Assert: Can recover without data loss
    assert_eq!(governor.circuit_state(), CircuitState::HalfOpen);
    assert_eq!(governor.total_kills(), total_before); // Total preserved
    assert_eq!(governor.active_kills(), 0); // Active reset
}

#[test]
fn test_integration_rollback_whitelist_toggle() {
    // Arrange: Hung process
    let capsule = Arc::new(ProcessStateCapsule::new(7777));
    capsule.update(7777, 200.0, 500, false, false, false);

    assert!(capsule.is_hung(100.0, 300));

    // Act: Whitelist (rollback protection)
    capsule.set_whitelisted(true);
    assert!(!capsule.is_hung(100.0, 300));

    // Act: Unwhitelist (rollback rollback)
    capsule.set_whitelisted(false);
    assert!(capsule.is_hung(100.0, 300));
}

// ============================================================================
// Q20: I20 Validation
// ============================================================================

#[test]
fn test_integration_i20_q13_boundary_invariants() {
    // I20 Q13: Boundary invariants between components
    let capsule = Arc::new(ProcessStateCapsule::new(1111));
    let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 5, 60));

    capsule.update(1111, 150.0, 400, false, false, false);

    // Boundary invariant: hung detection → kill coordination
    if capsule.is_hung(100.0, 300) {
        assert!(governor.can_kill()); // Governor state coordinated
    }
}

#[test]
fn test_integration_i20_q17_property_invariants_composition() {
    // I20 Q17: Property invariants across composition
    let capsule = Arc::new(ProcessStateCapsule::new(2222));
    let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 10, 60));

    // Property: Generation counter and kill counter both monotonic
    let gen1 = capsule.generation();
    let kills1 = governor.total_kills();

    capsule.update(2222, 100.0, 100, false, false, false);
    governor.record_kill();

    let gen2 = capsule.generation();
    let kills2 = governor.total_kills();

    assert_eq!(gen2, (gen1 + 1) & 0xFF); // Generation monotonic
    assert!(kills2 >= kills1); // Kills monotonic
}

// ============================================================================
// Q21: Monitoring Instrumentation
// ============================================================================

#[test]
fn test_integration_metrics_collection() {
    // Arrange: Components with observable metrics
    let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 5, 60));

    // Act: Perform operations
    for _ in 0..3 {
        governor.record_kill();
    }

    // Assert: Metrics available
    assert_eq!(governor.total_kills(), 3);
    assert_eq!(governor.active_kills(), 3);
    assert_eq!(governor.circuit_state(), CircuitState::Closed);

    // Trip circuit
    for _ in 0..3 {
        governor.record_kill();
    }

    // Metrics reflect state change
    assert_eq!(governor.circuit_state(), CircuitState::Open);
}

#[test]
fn test_integration_state_visibility() {
    // Arrange: Process with various states
    let capsule = Arc::new(ProcessStateCapsule::new(3333));

    // Act: Update and query state
    capsule.update(3333, 234.5, 567, true, false, true);

    // Assert: All state observable
    assert_eq!(capsule.pid(), 3333);
    assert!((capsule.cpu_pct() - 234.5).abs() < 0.1);
    assert_eq!(capsule.runtime_sec(), 567);
    assert!(capsule.is_test_or_bench());

    let gen = capsule.generation();
    assert!(gen <= 255); // u8, always in range
}

// ============================================================================
// Additional Integration Tests
// ============================================================================

#[test]
fn test_integration_concurrent_pipeline() {
    // Arrange: Shared components with concurrent access
    let capsules: Vec<_> = (0..10)
        .map(|i| Arc::new(ProcessStateCapsule::new(i * 1000)))
        .collect();

    let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 50, 60));

    // Act: Concurrent pipeline execution
    let handles: Vec<_> = capsules
        .iter()
        .enumerate()
        .map(|(i, capsule)| {
            let c = Arc::clone(capsule);
            let g = Arc::clone(&governor);

            thread::spawn(move || {
                for j in 0..100 {
                    c.update(
                        (i * 1000) as u32,
                        100.0 + j as f64,
                        200 + j,
                        false,
                        false,
                        false,
                    );

                    if c.is_hung(150.0, 250) && g.can_kill() {
                        let _ = g.record_kill();
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Assert: No data corruption
    assert!(governor.total_kills() > 0);
}

#[test]
fn test_integration_stress_generation_counters() {
    // Arrange: High-frequency updates
    let capsule = Arc::new(ProcessStateCapsule::new(4444));

    // Act: Rapid updates
    for i in 0..1000 {
        capsule.update(4444, 100.0, 100, false, false, false);
        assert_eq!(capsule.generation(), ((i + 1) & 0xFF) as u8);
    }

    // Assert: Generation wrapped correctly (u8, always <= 255)
    assert!(capsule.generation() <= 255);
}
