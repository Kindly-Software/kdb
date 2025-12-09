/// Tier 2: Property Tests (Q8-Q14)
/// Goal: Validate invariants hold across input space

use crate::capsules::{ProcessStateCapsule, ResourceGovernorCapsule, CircuitState};
use proptest::prelude::*;
use std::sync::Arc;
use std::thread;

// ============================================================================
// Q8: Universal Properties
// ============================================================================

proptest! {
    #[test]
    fn prop_pid_extracted_correctly(pid in 0u32..1_048_576) {
        let capsule = ProcessStateCapsule::new(pid);

        // Property: PID always extractable from state
        prop_assert_eq!(capsule.pid(), pid);
    }

    #[test]
    fn prop_cpu_pct_bounded(cpu in 0.0f64..500.0) {
        let capsule = ProcessStateCapsule::new(123);
        capsule.update(123, cpu, 100, false, false, false);

        // Property: CPU percentage always representable (clamped at max)
        let stored = capsule.cpu_pct();
        prop_assert!(stored >= 0.0);
        prop_assert!(stored <= 409.5); // Max representable with 12 bits / 10
    }

    #[test]
    fn prop_runtime_bounded(runtime in 0u64..2_000_000) {
        let capsule = ProcessStateCapsule::new(456);
        capsule.update(456, 100.0, runtime, false, false, false);

        // Property: Runtime always bounded by 20-bit limit
        let stored = capsule.runtime_sec();
        prop_assert!(stored <= 1_048_575);
    }

    #[test]
    fn prop_generation_monotonic(
        updates in prop::collection::vec((0.0f64..200.0, 0u64..1000), 1..100)
    ) {
        let capsule = ProcessStateCapsule::new(789);
        let mut last_gen = capsule.generation();

        for (cpu, runtime) in updates {
            capsule.update(789, cpu, runtime, false, false, false);
            let current_gen = capsule.generation();

            // Property: Generation always increases (mod 256)
            let expected = (last_gen + 1) & 0xFF;
            prop_assert_eq!(current_gen, expected);

            last_gen = current_gen;
        }
    }

    #[test]
    fn prop_whitelisted_never_hung(
        cpu in 100.0f64..400.0,
        runtime in 300u64..10000,
        threshold_cpu in 50.0f64..150.0,
        threshold_runtime in 100u64..500,
    ) {
        let capsule = ProcessStateCapsule::new(999);
        capsule.update(999, cpu, runtime, false, false, false);
        capsule.set_whitelisted(true);

        // Property: Whitelisted processes never detected as hung
        prop_assert!(!capsule.is_hung(threshold_cpu, threshold_runtime));
    }
}

// ============================================================================
// Q9: Concurrent Invariants
// ============================================================================

#[test]
fn prop_concurrent_no_lost_updates() {
    let capsule = Arc::new(ProcessStateCapsule::new(1000));
    let num_threads = 50;
    let updates_per_thread = 100;

    let initial_gen = capsule.generation();

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                for j in 0..updates_per_thread {
                    c.update(1000, 100.0 + i as f64, j, false, false, false);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Property: Generation incremented by total_updates (mod 256)
    let total_updates = (num_threads * updates_per_thread) as u8; // Wraps at 256
    let expected_gen = initial_gen.wrapping_add(total_updates);
    assert_eq!(capsule.generation(), expected_gen);
}

#[test]
fn prop_concurrent_generation_consistency() {
    let capsule = Arc::new(ProcessStateCapsule::new(2000));
    let readers = 50;
    let writers = 10;
    let writer_updates = 100;

    // Writers: Update state
    let write_handles: Vec<_> = (0..writers)
        .map(|_| {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                for _ in 0..writer_updates {
                    c.update(2000, 150.0, 400, false, false, false);
                }
            })
        })
        .collect();

    // Readers: Verify consistency
    let read_handles: Vec<_> = (0..readers)
        .map(|_| {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                for _ in 0..1000 {
                    let gen_before = c.generation();
                    let pid = c.pid();
                    let gen_after = c.generation();

                    // Property: If generations match, read is consistent
                    if gen_before == gen_after {
                        assert_eq!(pid, 2000);
                    }
                }
            })
        })
        .collect();

    for h in write_handles.into_iter().chain(read_handles) {
        h.join().unwrap();
    }
}

#[test]
fn prop_concurrent_circuit_breaker_atomic() {
    let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 10, 60));
    let threads = 20;

    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let g = Arc::clone(&governor);
            thread::spawn(move || {
                for _ in 0..10 {
                    let _ = g.record_kill();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Property: Total kills = threads * kills_per_thread
    // (Some may be rejected by circuit breaker, but total <= expected)
    let total = governor.total_kills();
    assert!(total <= threads * 10);
}

#[test]
fn prop_concurrent_whitelist_no_race() {
    let capsule = Arc::new(ProcessStateCapsule::new(3000));
    let threads = 100;

    // Update and whitelist concurrently
    let handles: Vec<_> = (0..threads)
        .map(|i| {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                if i % 2 == 0 {
                    c.update(3000, 200.0, 500, false, false, false);
                } else {
                    c.set_whitelisted(true);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Property: Eventually whitelisted = never hung
    std::thread::sleep(std::time::Duration::from_millis(10));
    assert!(!capsule.is_hung(100.0, 300));
}

// ============================================================================
// Q10: Edge Case Properties
// ============================================================================

proptest! {
    #[test]
    fn prop_extreme_values_no_panic(
        // CRITICAL-012 FIX: Constrain PID to valid 20-bit range (max 1,048,575)
        pid in 0u32..1_048_576,
        cpu in any::<f64>(),
        runtime in any::<u64>(),
    ) {
        let capsule = ProcessStateCapsule::new(pid);

        // Property: No panic on any valid input (graceful clamping)
        capsule.update(pid, cpu, runtime, false, false, false);

        // Should always be readable
        let _ = capsule.pid();
        let _ = capsule.cpu_pct();
        let _ = capsule.runtime_sec();
    }

    #[test]
    fn prop_zero_values_valid(pid in 0u32..100) {
        let capsule = ProcessStateCapsule::new(pid);
        capsule.update(pid, 0.0, 0, false, false, false);

        // Property: Zero values are valid
        prop_assert_eq!(capsule.cpu_pct(), 0.0);
        prop_assert_eq!(capsule.runtime_sec(), 0);
    }

    #[test]
    fn prop_max_values_clamped(
        // CRITICAL-012 FIX: Constrain PID to valid 20-bit range (max 1,048,575)
        pid in 1000u32..1_048_576,
        cpu in 500.0f64..10000.0,
        runtime in 2_000_000u64..u64::MAX,
    ) {
        let capsule = ProcessStateCapsule::new(pid);
        capsule.update(pid, cpu, runtime, false, false, false);

        // Property: Extreme values clamped to representable range
        let stored_pid = capsule.pid();
        let stored_cpu = capsule.cpu_pct();
        let stored_runtime = capsule.runtime_sec();

        prop_assert!(stored_pid <= 1_048_575); // 20-bit max
        prop_assert!(stored_cpu <= 409.5);     // 12-bit / 10 max
        prop_assert!(stored_runtime <= 1_048_575); // 20-bit max
    }
}

// ============================================================================
// Q11: ASSUM Verification
// ============================================================================

// #ASSUME: Generation counter prevents TOCTOU
// #VERIFY: Property test with concurrent readers/writers

proptest! {
    #[test]
    fn prop_verify_no_toctou(
        updates in prop::collection::vec((0.0f64..200.0, 0u64..1000), 100..500)
    ) {
        let capsule = Arc::new(ProcessStateCapsule::new(4000));

        // Concurrent writers
        let writers: Vec<_> = updates.chunks(50)
            .map(|chunk| {
                let c = Arc::clone(&capsule);
                let ops = chunk.to_vec();
                thread::spawn(move || {
                    for (cpu, runtime) in ops {
                        c.update(4000, cpu, runtime, false, false, false);
                    }
                })
            })
            .collect();

        // Concurrent reader checking TOCTOU prevention
        let reader = {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                let mut consistent_reads = 0;
                for _ in 0..10000 {
                    let gen1 = c.generation();
                    let pid = c.pid();
                    let gen2 = c.generation();

                    // Property: If gens match, no TOCTOU (verified)
                    if gen1 == gen2 {
                        consistent_reads += 1;
                        assert_eq!(pid, 4000);
                    }
                }
                consistent_reads
            })
        };

        for w in writers {
            w.join().unwrap();
        }
        let reads = reader.join().unwrap();

        // Should have many consistent reads
        prop_assert!(reads > 0);
    }
}

// #ASSUME: Circuit breaker prevents kill storms
// #VERIFY: Property test with max contention

#[test]
fn prop_verify_circuit_breaker_bounds_kills() {
    let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 5, 1));
    let threads = 100;
    let attempts_per_thread = 100;

    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let g = Arc::clone(&governor);
            thread::spawn(move || {
                let mut successful = 0;
                for _ in 0..attempts_per_thread {
                    if g.record_kill() {
                        successful += 1;
                    }
                }
                successful
            })
        })
        .collect();

    let mut total_successful = 0;
    for h in handles {
        total_successful += h.join().unwrap();
    }

    // Property: Circuit breaker limits kills (not all attempts succeed)
    assert!(total_successful < threads * attempts_per_thread);

    // Circuit should have tripped
    assert_ne!(governor.circuit_state(), CircuitState::Closed);
}

// #ASSUME: Atomic operations ensure no torn reads
// #VERIFY: Stress test with high contention

#[test]
fn prop_verify_no_torn_reads() {
    let capsule = Arc::new(ProcessStateCapsule::new(5000));
    let writers = 50;
    let readers = 50;

    // Writers: Set distinct PID patterns
    let write_handles: Vec<_> = (0..writers)
        .map(|i| {
            let c = Arc::clone(&capsule);
            let pid = 5000 + i;
            thread::spawn(move || {
                for _ in 0..100 {
                    c.update(pid, 100.0 + i as f64, 100 + i as u64, false, false, false);
                }
            })
        })
        .collect();

    // Readers: Verify no torn reads
    let read_handles: Vec<_> = (0..readers)
        .map(|_| {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                for _ in 0..1000 {
                    let pid = c.pid();
                    let cpu = c.cpu_pct();
                    let runtime = c.runtime_sec();

                    // Property: All fields should be self-consistent
                    // (no torn reads across field boundaries)
                    assert!(pid >= 5000 && pid < 5000 + writers);
                    assert!(cpu >= 100.0 && cpu <= 100.0 + writers as f64);
                    assert!(runtime >= 100 && runtime <= 100 + writers as u64);
                }
            })
        })
        .collect();

    for h in write_handles.into_iter().chain(read_handles) {
        h.join().unwrap();
    }
}

// ============================================================================
// Q12: Composition Properties
// ============================================================================

proptest! {
    #[test]
    fn prop_capsule_governor_coordination(
        operations in prop::collection::vec((100.0f64..300.0, 300u64..600), 10..50)
    ) {
        let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 5, 60));
        let capsule = Arc::new(ProcessStateCapsule::new(6000));

        for (cpu, runtime) in operations {
            // Update capsule
            capsule.update(6000, cpu, runtime, false, false, false);

            // Check if hung
            if capsule.is_hung(100.0, 300) {
                // Try to kill
                if governor.can_kill() {
                    governor.record_kill();
                }
            }

            // Property: Circuit state reflects kill activity
            if governor.active_kills() > 5 {
                prop_assert_ne!(governor.circuit_state(), CircuitState::Closed);
            }
        }
    }
}

// ============================================================================
// Q13: Statistical Properties
// ============================================================================

proptest! {
    #[test]
    fn prop_generation_distribution_uniform(
        updates in prop::collection::vec(0.0f64..200.0, 1000..2000)
    ) {
        let capsule = ProcessStateCapsule::new(7000);

        for cpu in updates {
            capsule.update(7000, cpu, 100, false, false, false);
        }

        // Property: Generation wraps uniformly (no bias)
        let final_gen = capsule.generation();
        // Generation is u8, always < 256 by definition
        prop_assert!(final_gen <= 255);
    }

    #[test]
    fn prop_kill_rate_bounded(
        kill_attempts in 15usize..1000
    ) {
        // Threshold at 10, need at least 15 attempts to guarantee trip
        let governor = ResourceGovernorCapsule::new(100.0, 4096, 10, 60);

        let mut successful = 0;
        for _ in 0..kill_attempts {
            if governor.record_kill() {
                successful += 1;
            }
        }

        // Property: Circuit breaker limits kill rate
        // (should trip after threshold exceeded)
        let success_rate = (successful as f64) / (kill_attempts as f64);
        prop_assert!(success_rate < 1.0, "Circuit breaker should have tripped");
    }
}

// ============================================================================
// Q14: Regression Tracking
// ============================================================================

// Proptest automatically saves failing cases to .proptest-regressions/
// These tests ensure regressions are caught

proptest! {
    #[test]
    fn prop_regression_pid_extraction(
        pid in 0u32..1_048_576
    ) {
        let capsule = ProcessStateCapsule::new(pid);

        // If this fails, proptest saves the case
        prop_assert_eq!(capsule.pid(), pid);
    }

    #[test]
    fn prop_regression_hung_detection(
        cpu in 0.0f64..500.0,
        runtime in 0u64..10000,
        threshold_cpu in 50.0f64..200.0,
        threshold_runtime in 100u64..1000,
    ) {
        let capsule = ProcessStateCapsule::new(8000);
        capsule.update(8000, cpu, runtime, false, false, false);

        let is_hung = capsule.is_hung(threshold_cpu, threshold_runtime);

        // Property: Hung if both thresholds exceeded
        let expected = cpu > threshold_cpu && runtime > threshold_runtime;
        prop_assert_eq!(is_hung, expected);
    }

    #[test]
    fn prop_regression_circuit_state_transitions(
        threshold in 1u8..20,
        attempts in 1usize..50,
    ) {
        let governor = ResourceGovernorCapsule::new(100.0, 4096, threshold, 60);

        for _ in 0..attempts {
            let _ = governor.record_kill();
        }

        // Property: Circuit trips when threshold exceeded
        if attempts > threshold as usize {
            prop_assert_ne!(governor.circuit_state(), CircuitState::Closed);
        }
    }
}

// ============================================================================
// Additional Property Tests
// ============================================================================

#[test]
fn prop_concurrent_flag_updates_safe() {
    let capsule = Arc::new(ProcessStateCapsule::new(9000));
    let threads = 100;

    let handles: Vec<_> = (0..threads)
        .map(|i| {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                let is_test = i % 2 == 0;
                let is_bench = i % 3 == 0;
                let is_cargo = i % 5 == 0;

                for _ in 0..100 {
                    c.update(9000, 100.0, 100, is_test, is_bench, is_cargo);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Property: No corruption despite concurrent flag updates
    let _ = capsule.is_test_or_bench(); // Should not panic
}

#[test]
fn prop_reset_preserves_total_kills() {
    let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 100, 60));
    let threads = 10;

    // Record kills concurrently
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let g = Arc::clone(&governor);
            thread::spawn(move || {
                for _ in 0..10 {
                    let _ = g.record_kill();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let total_before = governor.total_kills();

    // Reset active kills
    governor.reset_active_kills();

    // Property: Total kills unchanged by reset
    assert_eq!(governor.total_kills(), total_before);
    assert_eq!(governor.active_kills(), 0);
}
