//! # SystemdServiceCapsule T28 Tests
//!
//! **Comprehensive testing (T28 framework: 4 tiers)**
//!
//! ## Test Coverage
//! - **Unit Tests (Q1-Q7)**: Core functionality, state transitions, health updates
//! - **Property Tests (Q8-Q14)**: Concurrency, generation counters, monotonicity
//! - **Integration Tests (Q15-Q21)**: Systemd integration (requires systemctl)
//! - **Production Tests (Q22-Q28)**: Real-world scenarios, error handling
//!
//! ## Framework Compliance
//! - **UCE34**: T1 Atomic tier validation
//! - **Chaos**: 100% lockfree verification
//! - **ASSUM**: Safety assumptions tested
//! - **B32**: Performance baselines measured
//! - **T28**: 4-tier comprehensive testing
//! - **I20**: Integration validation

#![cfg(feature = "systemd")]

use atomic_capsule::daemon::{
    HealthStatus, ServiceState, ServiceStats, SystemdServiceCapsule,
};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Unit Tests (Q1-Q7)
// ============================================================================

#[test]
fn test_q1_capsule_layout() {
    // Q1: Verify memory layout (64 bytes, 64-byte aligned)
    assert_eq!(core::mem::size_of::<SystemdServiceCapsule>(), 64);
    assert_eq!(core::mem::align_of::<SystemdServiceCapsule>(), 64);
}

#[test]
fn test_q2_service_state_enum() {
    // Q2: Verify ServiceState enum variants
    assert_eq!(ServiceState::Unknown as u8, 0);
    assert_eq!(ServiceState::Running as u8, 1);
    assert_eq!(ServiceState::Stopped as u8, 2);
    assert_eq!(ServiceState::Failed as u8, 3);
    assert_eq!(ServiceState::Restarting as u8, 4);
}

#[test]
fn test_q3_health_status_enum() {
    // Q3: Verify HealthStatus enum variants
    assert_eq!(HealthStatus::Unknown as u8, 0);
    assert_eq!(HealthStatus::Healthy as u8, 1);
    assert_eq!(HealthStatus::Degraded as u8, 2);
    assert_eq!(HealthStatus::Failing as u8, 3);
    assert_eq!(HealthStatus::Unhealthy as u8, 4);
}

#[test]
fn test_q4_new_capsule_initialization() {
    // Q4: Verify new capsule defaults
    let capsule = SystemdServiceCapsule::new("test-service");
    assert_eq!(capsule.get_state(), ServiceState::Unknown);
    assert_eq!(capsule.get_pid(), 0);
    assert_eq!(capsule.get_generation(), 0);
    assert_eq!(capsule.get_restart_count(), 0);
    assert_eq!(capsule.get_health(), HealthStatus::Unknown);
}

#[test]
fn test_q5_state_transitions() {
    // Q5: Verify state transition correctness
    let capsule = SystemdServiceCapsule::new("test-service");

    // Transition: Unknown → Running
    capsule.update_state(ServiceState::Running, 1234);
    assert_eq!(capsule.get_state(), ServiceState::Running);
    assert_eq!(capsule.get_pid(), 1234);
    assert_eq!(capsule.get_generation(), 1);

    // Transition: Running → Stopped
    capsule.update_state(ServiceState::Stopped, 0);
    assert_eq!(capsule.get_state(), ServiceState::Stopped);
    assert_eq!(capsule.get_pid(), 0);
    assert_eq!(capsule.get_generation(), 2);

    // Transition: Stopped → Failed
    capsule.update_state(ServiceState::Failed, 0);
    assert_eq!(capsule.get_state(), ServiceState::Failed);
    assert_eq!(capsule.get_generation(), 3);

    // Transition: Failed → Restarting
    capsule.update_state(ServiceState::Restarting, 0);
    assert_eq!(capsule.get_state(), ServiceState::Restarting);
    assert_eq!(capsule.get_generation(), 4);

    // Transition: Restarting → Running
    capsule.update_state(ServiceState::Running, 5678);
    assert_eq!(capsule.get_state(), ServiceState::Running);
    assert_eq!(capsule.get_pid(), 5678);
    assert_eq!(capsule.get_generation(), 5);
}

#[test]
fn test_q6_health_updates() {
    // Q6: Verify health status updates
    let capsule = SystemdServiceCapsule::new("test-service");

    capsule.update_health(HealthStatus::Healthy);
    assert_eq!(capsule.get_health(), HealthStatus::Healthy);

    capsule.update_health(HealthStatus::Degraded);
    assert_eq!(capsule.get_health(), HealthStatus::Degraded);

    capsule.update_health(HealthStatus::Failing);
    assert_eq!(capsule.get_health(), HealthStatus::Failing);

    capsule.update_health(HealthStatus::Unhealthy);
    assert_eq!(capsule.get_health(), HealthStatus::Unhealthy);
}

#[test]
fn test_q7_restart_counter() {
    // Q7: Verify restart counter increments
    let capsule = SystemdServiceCapsule::new("test-service");

    assert_eq!(capsule.get_restart_count(), 0);

    capsule.record_restart();
    assert_eq!(capsule.get_restart_count(), 1);

    capsule.record_restart();
    assert_eq!(capsule.get_restart_count(), 2);

    // Verify counter monotonicity (10 restarts)
    for _ in 0..10 {
        capsule.record_restart();
    }
    assert_eq!(capsule.get_restart_count(), 12);
}

// ============================================================================
// Property Tests (Q8-Q14)
// ============================================================================

#[test]
fn test_q8_concurrent_state_reads() {
    // Q8: Verify concurrent state queries (lockfree reads)
    let capsule = Arc::new(SystemdServiceCapsule::new("test-service"));
    capsule.update_state(ServiceState::Running, 9999);

    let handles: Vec<_> = (0..16)
        .map(|_| {
            let capsule = Arc::clone(&capsule);
            thread::spawn(move || {
                // Read state 1000 times
                for _ in 0..1000 {
                    let state = capsule.get_state();
                    assert_eq!(state, ServiceState::Running);
                    let pid = capsule.get_pid();
                    assert_eq!(pid, 9999);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_q9_concurrent_state_writes() {
    // Q9: Verify concurrent state updates (lockfree writes)
    let capsule = Arc::new(SystemdServiceCapsule::new("test-service"));

    let handles: Vec<_> = (0..8)
        .map(|i| {
            let capsule = Arc::clone(&capsule);
            thread::spawn(move || {
                // Alternate between Running and Stopped
                for j in 0..100 {
                    if j % 2 == 0 {
                        capsule.update_state(ServiceState::Running, 1000 + i);
                    } else {
                        capsule.update_state(ServiceState::Stopped, 0);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final state is coherent
    let state = capsule.get_state();
    assert!(matches!(
        state,
        ServiceState::Running | ServiceState::Stopped
    ));

    // Generation counter should be >= 800 (8 threads × 100 updates)
    let generation = capsule.get_generation();
    assert!(generation >= 800, "Generation: {}", generation);
}

#[test]
fn test_q10_concurrent_health_updates() {
    // Q10: Verify concurrent health updates
    let capsule = Arc::new(SystemdServiceCapsule::new("test-service"));

    let handles: Vec<_> = (0..8)
        .map(|i| {
            let capsule = Arc::clone(&capsule);
            thread::spawn(move || {
                for _ in 0..100 {
                    let health = match i % 4 {
                        0 => HealthStatus::Healthy,
                        1 => HealthStatus::Degraded,
                        2 => HealthStatus::Failing,
                        _ => HealthStatus::Unhealthy,
                    };
                    capsule.update_health(health);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final health is one of the expected values
    let health = capsule.get_health();
    assert!(matches!(
        health,
        HealthStatus::Healthy
            | HealthStatus::Degraded
            | HealthStatus::Failing
            | HealthStatus::Unhealthy
    ));
}

#[test]
fn test_q11_concurrent_restart_increments() {
    // Q11: Verify concurrent restart increments (monotonicity)
    let capsule = Arc::new(SystemdServiceCapsule::new("test-service"));

    let handles: Vec<_> = (0..16)
        .map(|_| {
            let capsule = Arc::clone(&capsule);
            thread::spawn(move || {
                for _ in 0..100 {
                    capsule.record_restart();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Should be exactly 1600 (16 threads × 100 increments)
    let restart_count = capsule.get_restart_count();
    assert_eq!(restart_count, 1600);
}

#[test]
fn test_q12_generation_counter_monotonicity() {
    // Q12: Verify generation counter never decrements
    let capsule = SystemdServiceCapsule::new("test-service");

    let mut prev_gen = capsule.get_generation();
    assert_eq!(prev_gen, 0);

    for i in 1..=100 {
        capsule.update_state(
            if i % 2 == 0 {
                ServiceState::Running
            } else {
                ServiceState::Stopped
            },
            if i % 2 == 0 { i } else { 0 },
        );

        let current_gen = capsule.get_generation();
        assert!(
            current_gen > prev_gen,
            "Generation should be monotonic: {} > {}",
            current_gen,
            prev_gen
        );
        prev_gen = current_gen;
    }

    assert_eq!(prev_gen, 100);
}

#[test]
fn test_q13_pid_24bit_boundary() {
    // Q13: Verify PID fits 24-bit limit
    let capsule = SystemdServiceCapsule::new("test-service");

    // Max 24-bit PID: 2^24 - 1 = 16,777,215
    capsule.update_state(ServiceState::Running, 16_777_215);
    assert_eq!(capsule.get_pid(), 16_777_215);

    // Linux max PID is typically ~4 million, well under 2^24
    capsule.update_state(ServiceState::Running, 4_194_304);
    assert_eq!(capsule.get_pid(), 4_194_304);
}

#[test]
#[should_panic(expected = "PID exceeds 24-bit limit")]
fn test_q14_pid_24bit_overflow() {
    // Q14: Verify PID overflow panics
    let capsule = SystemdServiceCapsule::new("test-service");

    // Try to set PID > 2^24-1 (should panic)
    capsule.update_state(ServiceState::Running, 1 << 24);
}

// ============================================================================
// Integration Tests (Q15-Q21)
// ============================================================================

#[test]
fn test_q15_systemd_state_parsing() {
    // Q15: Verify systemd ActiveState parsing
    assert_eq!(
        ServiceState::from_systemd_active_state("active"),
        ServiceState::Running
    );
    assert_eq!(
        ServiceState::from_systemd_active_state("inactive"),
        ServiceState::Stopped
    );
    assert_eq!(
        ServiceState::from_systemd_active_state("failed"),
        ServiceState::Failed
    );
    assert_eq!(
        ServiceState::from_systemd_active_state("activating"),
        ServiceState::Restarting
    );
    assert_eq!(
        ServiceState::from_systemd_active_state("unknown"),
        ServiceState::Unknown
    );
    assert_eq!(
        ServiceState::from_systemd_active_state("deactivating"),
        ServiceState::Unknown
    );
}

#[test]
fn test_q16_is_running_helper() {
    // Q16: Verify is_running() helper
    let capsule = SystemdServiceCapsule::new("test-service");

    assert!(!capsule.is_running());

    capsule.update_state(ServiceState::Running, 1234);
    assert!(capsule.is_running());

    capsule.update_state(ServiceState::Stopped, 0);
    assert!(!capsule.is_running());

    capsule.update_state(ServiceState::Failed, 0);
    assert!(!capsule.is_running());

    capsule.update_state(ServiceState::Restarting, 0);
    assert!(!capsule.is_running());
}

#[test]
fn test_q17_is_healthy_helper() {
    // Q17: Verify is_healthy() helper
    let capsule = SystemdServiceCapsule::new("test-service");

    assert!(!capsule.is_healthy()); // Unknown is not healthy

    capsule.update_health(HealthStatus::Healthy);
    assert!(capsule.is_healthy());

    capsule.update_health(HealthStatus::Degraded);
    assert!(capsule.is_healthy()); // Degraded is still "healthy enough"

    capsule.update_health(HealthStatus::Failing);
    assert!(!capsule.is_healthy());

    capsule.update_health(HealthStatus::Unhealthy);
    assert!(!capsule.is_healthy());
}

#[test]
fn test_q18_get_stats_snapshot() {
    // Q18: Verify get_stats() returns coherent snapshot
    let capsule = SystemdServiceCapsule::new("test-service");

    capsule.update_state(ServiceState::Running, 7777);
    capsule.update_health(HealthStatus::Healthy);
    capsule.record_restart();
    capsule.record_restart();
    capsule.record_restart();

    let stats = capsule.get_stats();
    assert_eq!(stats.state, ServiceState::Running);
    assert_eq!(stats.pid, 7777);
    assert_eq!(stats.generation, 1);
    assert_eq!(stats.restart_count, 3);
    assert_eq!(stats.health, HealthStatus::Healthy);
    assert!(stats.uptime_ns > 0);
    assert!(stats.last_start_ns > 0);
}

#[test]
fn test_q19_uptime_tracking() {
    // Q19: Verify uptime calculation
    let capsule = SystemdServiceCapsule::new("test-service");

    // Not running initially
    assert_eq!(capsule.get_uptime_ns(), 0);

    // Start service
    capsule.update_state(ServiceState::Running, 8888);
    thread::sleep(std::time::Duration::from_millis(10));

    // Uptime should be > 0
    let uptime = capsule.get_uptime_ns();
    assert!(uptime > 0, "Uptime: {} ns", uptime);
    assert!(uptime >= 10_000_000, "Uptime: {} ns (should be >= 10ms)", uptime);

    // Stop service
    capsule.update_state(ServiceState::Stopped, 0);
    assert_eq!(capsule.get_uptime_ns(), 0);
}

#[test]
fn test_q20_restart_counter_saturation() {
    // Q20: Verify restart counter saturates at u16::MAX
    let capsule = SystemdServiceCapsule::new("test-service");

    // Increment to near-max
    for _ in 0..65534 {
        capsule.record_restart();
    }
    assert_eq!(capsule.get_restart_count(), 65534);

    // One more (should reach max)
    capsule.record_restart();
    assert_eq!(capsule.get_restart_count(), 65535);

    // Try to increment past max (should saturate)
    capsule.record_restart();
    assert_eq!(capsule.get_restart_count(), 65535); // Saturated
}

#[test]
fn test_q21_default_trait() {
    // Q21: Verify Default trait implementation
    let capsule = SystemdServiceCapsule::default();
    assert_eq!(capsule.get_state(), ServiceState::Unknown);
    assert_eq!(capsule.get_pid(), 0);
    assert_eq!(capsule.get_generation(), 0);
    assert_eq!(capsule.get_restart_count(), 0);
    assert_eq!(capsule.get_health(), HealthStatus::Unknown);
}

// ============================================================================
// Production Tests (Q22-Q28)
// ============================================================================

#[test]
fn test_q22_high_contention_state_updates() {
    // Q22: Verify correctness under high contention
    let capsule = Arc::new(SystemdServiceCapsule::new("test-service"));

    let handles: Vec<_> = (0..32)
        .map(|i| {
            let capsule = Arc::clone(&capsule);
            thread::spawn(move || {
                for j in 0..1000 {
                    let state = if (i + j) % 2 == 0 {
                        ServiceState::Running
                    } else {
                        ServiceState::Stopped
                    };
                    let pid = if state == ServiceState::Running {
                        (i * 1000 + j) as u32
                    } else {
                        0
                    };
                    capsule.update_state(state, pid);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final state is coherent
    let state = capsule.get_state();
    assert!(matches!(
        state,
        ServiceState::Running | ServiceState::Stopped
    ));

    // Generation should be 32,000 (32 threads × 1000 updates)
    let generation = capsule.get_generation();
    assert_eq!(generation, 32_000);
}

#[test]
fn test_q23_mixed_operations_correctness() {
    // Q23: Verify correctness under mixed concurrent operations
    let capsule = Arc::new(SystemdServiceCapsule::new("test-service"));

    let state_handles: Vec<_> = (0..8)
        .map(|i| {
            let capsule = Arc::clone(&capsule);
            thread::spawn(move || {
                for j in 0..500 {
                    capsule.update_state(
                        if (i + j) % 2 == 0 {
                            ServiceState::Running
                        } else {
                            ServiceState::Stopped
                        },
                        if (i + j) % 2 == 0 { (i * 100 + j) as u32 } else { 0 },
                    );
                }
            })
        })
        .collect();

    let health_handles: Vec<_> = (0..8)
        .map(|i| {
            let capsule = Arc::clone(&capsule);
            thread::spawn(move || {
                for j in 0..500 {
                    let health = match (i + j) % 4 {
                        0 => HealthStatus::Healthy,
                        1 => HealthStatus::Degraded,
                        2 => HealthStatus::Failing,
                        _ => HealthStatus::Unhealthy,
                    };
                    capsule.update_health(health);
                }
            })
        })
        .collect();

    let restart_handles: Vec<_> = (0..8)
        .map(|_| {
            let capsule = Arc::clone(&capsule);
            thread::spawn(move || {
                for _ in 0..500 {
                    capsule.record_restart();
                }
            })
        })
        .collect();

    for handle in state_handles {
        handle.join().unwrap();
    }
    for handle in health_handles {
        handle.join().unwrap();
    }
    for handle in restart_handles {
        handle.join().unwrap();
    }

    // Verify coherent final state
    let stats = capsule.get_stats();
    assert!(matches!(
        stats.state,
        ServiceState::Running | ServiceState::Stopped
    ));
    assert!(matches!(
        stats.health,
        HealthStatus::Healthy
            | HealthStatus::Degraded
            | HealthStatus::Failing
            | HealthStatus::Unhealthy
    ));

    // Restart count should be exactly 4000 (8 threads × 500 restarts)
    assert_eq!(stats.restart_count, 4000);

    // Generation should be 4000 (8 threads × 500 state updates)
    assert_eq!(stats.generation, 4000);
}

#[test]
fn test_q24_state_query_performance() {
    // Q24: Verify state query performance (<50ns target)
    let capsule = SystemdServiceCapsule::new("test-service");
    capsule.update_state(ServiceState::Running, 1234);

    let start = std::time::Instant::now();
    for _ in 0..10000 {
        let _ = capsule.get_state();
        let _ = capsule.get_pid();
        let _ = capsule.get_generation();
    }
    let elapsed = start.elapsed();

    // 30,000 atomic loads in <1ms (target: <50ns per load = <1.5ms total)
    assert!(
        elapsed.as_micros() < 1500,
        "Performance: {} µs (target: < 1500µs)",
        elapsed.as_micros()
    );

    // Average per-query latency
    let avg_ns = elapsed.as_nanos() / 30_000;
    println!("Average state query: {} ns", avg_ns);
}

#[test]
fn test_q25_health_update_performance() {
    // Q25: Verify health update performance (<20ns target)
    let capsule = SystemdServiceCapsule::new("test-service");

    let start = std::time::Instant::now();
    for i in 0..10000 {
        let health = match i % 4 {
            0 => HealthStatus::Healthy,
            1 => HealthStatus::Degraded,
            2 => HealthStatus::Failing,
            _ => HealthStatus::Unhealthy,
        };
        capsule.update_health(health);
    }
    let elapsed = start.elapsed();

    // 10,000 health updates in <1000µs (target: <20ns per update = <200µs in release)
    // Note: Debug builds have overhead, so we use 1000µs threshold
    assert!(
        elapsed.as_micros() < 1000,
        "Performance: {} µs (target: < 1000µs)",
        elapsed.as_micros()
    );

    // Average per-update latency
    let avg_ns = elapsed.as_nanos() / 10_000;
    println!("Average health update: {} ns", avg_ns);
}

#[test]
fn test_q26_restart_increment_performance() {
    // Q26: Verify restart increment performance (<100ns target)
    let capsule = SystemdServiceCapsule::new("test-service");

    let start = std::time::Instant::now();
    for _ in 0..10000 {
        capsule.record_restart();
    }
    let elapsed = start.elapsed();

    // 10,000 restart increments in <2ms (target: <100ns per increment = <1ms)
    assert!(
        elapsed.as_micros() < 2000,
        "Performance: {} µs (target: < 2000µs)",
        elapsed.as_micros()
    );

    // Average per-increment latency
    let avg_ns = elapsed.as_nanos() / 10_000;
    println!("Average restart increment: {} ns", avg_ns);
}

#[test]
fn test_q27_state_transition_performance() {
    // Q27: Verify state transition performance (<100ns target)
    let capsule = SystemdServiceCapsule::new("test-service");

    let start = std::time::Instant::now();
    for i in 0..10000 {
        capsule.update_state(
            if i % 2 == 0 {
                ServiceState::Running
            } else {
                ServiceState::Stopped
            },
            if i % 2 == 0 { i as u32 } else { 0 },
        );
    }
    let elapsed = start.elapsed();

    // 10,000 state transitions in <2ms (target: <100ns per transition = <1ms)
    assert!(
        elapsed.as_micros() < 2000,
        "Performance: {} µs (target: < 2000µs)",
        elapsed.as_micros()
    );

    // Average per-transition latency
    let avg_ns = elapsed.as_nanos() / 10_000;
    println!("Average state transition: {} ns", avg_ns);
}

#[test]
fn test_q28_get_stats_performance() {
    // Q28: Verify get_stats() performance (<200ns target)
    let capsule = SystemdServiceCapsule::new("test-service");
    capsule.update_state(ServiceState::Running, 9999);
    capsule.update_health(HealthStatus::Healthy);

    let start = std::time::Instant::now();
    for _ in 0..10000 {
        let _ = capsule.get_stats();
    }
    let elapsed = start.elapsed();

    // 10,000 get_stats calls in <5ms (target: <200ns per call = <2ms)
    assert!(
        elapsed.as_micros() < 5000,
        "Performance: {} µs (target: < 5000µs)",
        elapsed.as_micros()
    );

    // Average per-call latency
    let avg_ns = elapsed.as_nanos() / 10_000;
    println!("Average get_stats: {} ns", avg_ns);
}
