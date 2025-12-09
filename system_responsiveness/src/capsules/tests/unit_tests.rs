/// Tier 1: Unit Tests (Q1-Q7)
/// Goal: Validate individual components in isolation

use crate::capsules::{ProcessStateCapsule, ResourceGovernorCapsule, CircuitState};

// ============================================================================
// Q1: Core Behaviors
// ============================================================================

#[test]
fn test_process_state_core_behaviors() {
    // Arrange: Create process state capsule
    let capsule = ProcessStateCapsule::new(1234);

    // Act: Update state
    capsule.update(1234, 150.5, 300, false, false, false);

    // Assert: State updated correctly
    assert_eq!(capsule.pid(), 1234);
    assert!((capsule.cpu_pct() - 150.5).abs() < 0.1);
    assert_eq!(capsule.runtime_sec(), 300);
}

#[test]
fn test_resource_governor_core_behaviors() {
    // Arrange: Create governor with limits
    let governor = ResourceGovernorCapsule::new(100.0, 4096, 5, 60);

    // Act: Check initial state
    let can_kill = governor.can_kill();
    let state = governor.circuit_state();

    // Assert: Initial state correct
    assert!(can_kill);
    assert_eq!(state, CircuitState::Closed);
    assert_eq!(governor.cpu_limit_pct(), 100.0);
}

#[test]
fn test_hung_detection_core_behavior() {
    // Arrange: Process with known state
    let capsule = ProcessStateCapsule::new(5678);
    capsule.update(5678, 200.0, 400, false, false, false);

    // Act: Check if hung
    let is_hung = capsule.is_hung(100.0, 300);

    // Assert: Correctly detected as hung
    assert!(is_hung);
}

// ============================================================================
// Q2: Edge Cases
// ============================================================================

#[test]
fn test_edge_case_zero_values() {
    let capsule = ProcessStateCapsule::new(0);

    // Zero PID (kernel threads)
    assert_eq!(capsule.pid(), 0);

    // Zero CPU
    capsule.update(0, 0.0, 0, false, false, false);
    assert_eq!(capsule.cpu_pct(), 0.0);
    assert_eq!(capsule.runtime_sec(), 0);
}

#[test]
fn test_edge_case_maximum_values() {
    let capsule = ProcessStateCapsule::new(1_048_575); // Max 20-bit PID

    // Maximum CPU (multi-core system)
    capsule.update(1_048_575, 409.5, 1_048_575, false, false, false);

    assert_eq!(capsule.pid(), 1_048_575);
    assert!((capsule.cpu_pct() - 409.5).abs() < 0.1);
    assert_eq!(capsule.runtime_sec(), 1_048_575);
}

#[test]
fn test_edge_case_pid_overflow() {
    // PID larger than 20 bits should be masked
    let large_pid = 2_000_000; // > 2^20
    let capsule = ProcessStateCapsule::new(large_pid);

    // PID should be masked to 20 bits
    let masked_pid = large_pid & 0xFFFFF;
    assert_eq!(capsule.pid(), masked_pid);
}

#[test]
fn test_edge_case_cpu_over_limit() {
    let capsule = ProcessStateCapsule::new(100);

    // CPU > 409.5% should be clamped
    capsule.update(100, 800.0, 100, false, false, false);

    // Should be clamped to max representable value
    assert!(capsule.cpu_pct() <= 409.5);
}

#[test]
fn test_edge_case_runtime_overflow() {
    let capsule = ProcessStateCapsule::new(200);

    // Runtime > 2^20 seconds should be clamped
    capsule.update(200, 100.0, 2_000_000, false, false, false);

    // Should be clamped to 20-bit max
    assert_eq!(capsule.runtime_sec(), 1_048_575);
}

#[test]
fn test_edge_case_circuit_breaker_boundary() {
    let governor = ResourceGovernorCapsule::new(100.0, 4096, 5, 60);

    // Exactly at threshold (5 kills)
    for i in 1..=5 {
        assert!(governor.record_kill());
        assert_eq!(governor.active_kills(), i);
    }

    // Still closed at threshold
    assert_eq!(governor.circuit_state(), CircuitState::Closed);

    // One more should trip (6th kill)
    assert!(governor.record_kill());
    assert_eq!(governor.circuit_state(), CircuitState::Open);
}

#[test]
fn test_edge_case_empty_whitelist() {
    let capsule = ProcessStateCapsule::new(123);

    // Not whitelisted by default
    capsule.update(123, 200.0, 400, false, false, false);
    assert!(capsule.is_hung(100.0, 300));
}

// ============================================================================
// Q3: Invariants
// ============================================================================

#[test]
fn test_invariant_generation_counter_monotonic() {
    let capsule = ProcessStateCapsule::new(999);

    let gen1 = capsule.generation();
    capsule.update(999, 100.0, 200, false, false, false);
    let gen2 = capsule.generation();

    // Invariant: Generation always increases (wraps at 256)
    assert_eq!(gen2, (gen1 + 1) & 0xFF);
}

#[test]
fn test_invariant_generation_wraps_correctly() {
    let capsule = ProcessStateCapsule::new(888);

    // Force generation to 255
    for _ in 0..255 {
        capsule.update(888, 100.0, 100, false, false, false);
    }

    let gen_before_wrap = capsule.generation();
    assert_eq!(gen_before_wrap, 255);

    // One more should wrap to 0
    capsule.update(888, 100.0, 100, false, false, false);
    assert_eq!(capsule.generation(), 0);
}

#[test]
fn test_invariant_whitelisted_never_hung() {
    let capsule = ProcessStateCapsule::new(777);

    // Set high CPU and long runtime
    capsule.update(777, 300.0, 600, false, false, false);

    // Whitelist BEFORE checking hung status
    capsule.set_whitelisted(true);

    // Invariant: Whitelisted processes never detected as hung
    assert!(!capsule.is_hung(100.0, 300));

    // Even with extreme values
    capsule.update(777, 400.0, 1000, false, false, false);
    // Must whitelist again after update (flags may be reset)
    capsule.set_whitelisted(true);
    assert!(!capsule.is_hung(100.0, 300));
}

#[test]
fn test_invariant_circuit_state_transitions() {
    let governor = ResourceGovernorCapsule::new(100.0, 4096, 3, 60);

    // Invariant: Closed → Open → HalfOpen → Closed
    assert_eq!(governor.circuit_state(), CircuitState::Closed);

    // Trip circuit
    for _ in 0..4 {
        governor.record_kill();
    }
    assert_eq!(governor.circuit_state(), CircuitState::Open);

    // Reset moves to half-open
    governor.reset_active_kills();
    assert_eq!(governor.circuit_state(), CircuitState::HalfOpen);
}

#[test]
fn test_invariant_total_kills_never_decreases() {
    let governor = ResourceGovernorCapsule::new(100.0, 4096, 100, 60);

    governor.record_kill();
    let total1 = governor.total_kills();

    governor.record_kill();
    let total2 = governor.total_kills();

    // Invariant: Total kills monotonically increasing
    assert!(total2 >= total1);

    // Reset active, but total persists
    governor.reset_active_kills();
    let total3 = governor.total_kills();
    assert_eq!(total3, total2);
}

#[test]
fn test_invariant_active_kills_reset() {
    let governor = ResourceGovernorCapsule::new(100.0, 4096, 100, 60);

    governor.record_kill();
    governor.record_kill();
    assert_eq!(governor.active_kills(), 2);

    // Invariant: reset_active_kills() resets to 0
    governor.reset_active_kills();
    assert_eq!(governor.active_kills(), 0);
}

// ============================================================================
// Q4: Code Path Coverage
// ============================================================================

#[test]
fn test_coverage_all_flag_combinations() {
    let capsule = ProcessStateCapsule::new(500);

    // Test all flag combinations
    let flag_combinations = [
        (false, false, false),
        (true, false, false),
        (false, true, false),
        (false, false, true),
        (true, true, false),
        (true, false, true),
        (false, true, true),
        (true, true, true),
    ];

    for (is_test, is_bench, is_cargo) in flag_combinations {
        capsule.update(500, 100.0, 100, is_test, is_bench, is_cargo);

        // Verify flags set correctly
        if is_test || is_bench {
            assert!(capsule.is_test_or_bench());
        }
    }
}

#[test]
fn test_coverage_hung_detection_branches() {
    let capsule = ProcessStateCapsule::new(600);

    // Branch 1: Not hung - low CPU
    capsule.update(600, 50.0, 400, false, false, false);
    assert!(!capsule.is_hung(100.0, 300));

    // Branch 2: Not hung - short runtime
    capsule.update(600, 150.0, 100, false, false, false);
    assert!(!capsule.is_hung(100.0, 300));

    // Branch 3: Hung - high CPU + long runtime
    capsule.update(600, 150.0, 400, false, false, false);
    assert!(capsule.is_hung(100.0, 300));

    // Branch 4: Not hung - whitelisted
    capsule.set_whitelisted(true);
    assert!(!capsule.is_hung(100.0, 300));
}

#[test]
fn test_coverage_circuit_breaker_states() {
    let governor = ResourceGovernorCapsule::new(100.0, 4096, 2, 60);

    // State: Closed
    assert_eq!(governor.circuit_state(), CircuitState::Closed);
    assert!(governor.can_kill());

    // State: Open (after threshold exceeded)
    governor.record_kill();
    governor.record_kill();
    governor.record_kill(); // 3rd trips at threshold=2
    assert_eq!(governor.circuit_state(), CircuitState::Open);
    assert!(!governor.can_kill());

    // State: HalfOpen (after reset)
    governor.reset_active_kills();
    assert_eq!(governor.circuit_state(), CircuitState::HalfOpen);
}

#[test]
fn test_coverage_can_kill_branches() {
    let governor = ResourceGovernorCapsule::new(100.0, 4096, 2, 1); // 1 sec cooldown

    // Closed: can kill
    assert!(governor.can_kill());

    // Open: cannot kill
    for _ in 0..3 {
        governor.record_kill();
    }
    assert!(!governor.can_kill());

    // HalfOpen: depends on cooldown
    governor.reset_active_kills();
    std::thread::sleep(std::time::Duration::from_secs(2)); // Wait for cooldown
    assert!(governor.can_kill()); // Cooldown expired
}

// ============================================================================
// Q5: Isolation and Determinism
// ============================================================================

#[test]
fn test_isolation_fresh_instances() {
    // Each test gets fresh instance
    let capsule1 = ProcessStateCapsule::new(100);
    let capsule2 = ProcessStateCapsule::new(200);

    capsule1.update(100, 150.0, 300, false, false, false);
    capsule2.update(200, 200.0, 400, false, false, false);

    // No cross-contamination
    assert_eq!(capsule1.pid(), 100);
    assert_eq!(capsule2.pid(), 200);
}

#[test]
fn test_determinism_same_input_same_output() {
    let capsule = ProcessStateCapsule::new(123);

    // Same update twice
    capsule.update(123, 100.0, 200, false, false, false);
    let cpu1 = capsule.cpu_pct();
    let runtime1 = capsule.runtime_sec();

    capsule.update(123, 100.0, 200, false, false, false);
    let cpu2 = capsule.cpu_pct();
    let runtime2 = capsule.runtime_sec();

    // Deterministic: same values
    assert_eq!(cpu1, cpu2);
    assert_eq!(runtime1, runtime2);
}

// ============================================================================
// Q6: Performance (Fast Tests)
// ============================================================================

#[test]
fn test_performance_hung_check_fast() {
    let capsule = ProcessStateCapsule::new(999);
    capsule.update(999, 150.0, 400, false, false, false);

    let iterations = 10_000;
    let start = std::time::Instant::now();

    for _ in 0..iterations {
        let _ = capsule.is_hung(100.0, 300);
    }

    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() / iterations;

    // Target: <50ns per check
    assert!(avg_ns < 100, "is_hung() too slow: {}ns > 100ns", avg_ns);
}

#[test]
fn test_performance_can_kill_fast() {
    let governor = ResourceGovernorCapsule::new(100.0, 4096, 100, 60);

    let iterations = 10_000;
    let start = std::time::Instant::now();

    for _ in 0..iterations {
        let _ = governor.can_kill();
    }

    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() / iterations;

    // Target: <20ns per check
    assert!(avg_ns < 50, "can_kill() too slow: {}ns > 50ns", avg_ns);
}

// ============================================================================
// Q7: Readability and Maintainability
// ============================================================================

#[test]
fn test_descriptive_test_names_demonstrate_behavior() {
    // This test serves as documentation
    // Test names should describe WHAT is tested, not HOW

    let capsule = ProcessStateCapsule::new(1);
    capsule.update(1, 100.0, 100, false, false, false);

    assert_eq!(capsule.pid(), 1);
}

// Helper function for test readability
fn create_hung_process() -> ProcessStateCapsule {
    let capsule = ProcessStateCapsule::new(9999);
    capsule.update(9999, 200.0, 500, false, false, false);
    capsule
}

#[test]
fn test_using_helper_for_readability() {
    // Arrange: Use helper for cleaner test
    let capsule = create_hung_process();

    // Act: Check hung state
    let is_hung = capsule.is_hung(100.0, 300);

    // Assert: Expected behavior
    assert!(is_hung, "Process with 200% CPU and 500s runtime should be hung");
}

// ============================================================================
// Additional Critical Tests
// ============================================================================

#[test]
fn test_alignment_verification() {
    // Critical for cache performance
    assert_eq!(std::mem::align_of::<ProcessStateCapsule>(), 128);
    assert_eq!(std::mem::size_of::<ProcessStateCapsule>(), 128);

    assert_eq!(std::mem::align_of::<ResourceGovernorCapsule>(), 64);
    assert_eq!(std::mem::size_of::<ResourceGovernorCapsule>(), 64);
}

#[test]
fn test_bit_packing_correctness() {
    let capsule = ProcessStateCapsule::new(12345);
    capsule.update(12345, 234.5, 67890, true, true, true);

    // Verify all fields packed correctly
    assert_eq!(capsule.pid(), 12345);
    assert!((capsule.cpu_pct() - 234.5).abs() < 0.1);
    assert_eq!(capsule.runtime_sec(), 67890);
    assert!(capsule.is_test_or_bench());
}

#[test]
fn test_kill_rejection_when_circuit_open() {
    let governor = ResourceGovernorCapsule::new(100.0, 4096, 2, 60);

    // Trip circuit
    governor.record_kill();
    governor.record_kill();
    governor.record_kill();

    // Kill should be rejected
    let result = governor.record_kill();
    assert!(!result, "Kill should be rejected when circuit is open");
}
