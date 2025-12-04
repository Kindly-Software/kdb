//! Integration Tests for AccessControlCapsule
//!
//! **Framework**: T28 (Q1-Q28 Testing Framework)
//! - Q1-Q7: Unit tests (embedded in access_control.rs)
//! - Q8-Q14: Property-based tests (this file)
//! - Q15-Q21: Integration tests (this file)
//! - Q22-Q28: Production tests (load/stress)
//!
//! **Compliance**: COCA (100% computational capsule), ASSUM (99.99% safety),
//! B32 (fair baseline), UCE34 (Q10-Q12 tier selection + Q33/Q34)

use kdb_mcp::access_control::{AccessControlCapsule, Command, AccessError};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// Q8-Q14: Property-Based Tests
// ============================================================================

#[test]
fn property_pid_whitelist_idempotent() {
    /// Property: allow_pid(N) twice should be equivalent to once (OR is idempotent)
    let ac = AccessControlCapsule::new();

    for pid in 0..64 {
        ac.allow_pid(pid).unwrap();
        ac.allow_pid(pid).unwrap(); // Call twice
        assert!(ac.is_pid_allowed(pid), "PID {} should be allowed", pid);
    }
}

#[test]
fn property_command_allow_deny_symmetry() {
    /// Property: allow(cmd) then deny(cmd) should result in denied state
    let ac = AccessControlCapsule::new();

    for cmd_num in 0..8 {
        let cmd = Command::from_u8(cmd_num).unwrap();

        ac.allow_command(cmd).unwrap();
        assert!(ac.is_command_allowed(cmd), "Command {} should be allowed after allow", cmd_num);

        ac.deny_command(cmd);
        assert!(!ac.is_command_allowed(cmd), "Command {} should be denied after deny", cmd_num);
    }
}

#[test]
fn property_clear_all_denies_everything() {
    /// Property: After clear_all(), all PIDs and commands should be denied
    let ac = AccessControlCapsule::new();

    // Allow many PIDs and commands
    for pid in 0..64 {
        ac.allow_pid(pid).unwrap();
    }
    for cmd_num in 0..8 {
        ac.allow_command(Command::from_u8(cmd_num).unwrap()).unwrap();
    }

    // All should be allowed
    for pid in 0..64 {
        assert!(ac.is_pid_allowed(pid));
    }
    for cmd_num in 0..8 {
        assert!(ac.is_command_allowed(Command::from_u8(cmd_num).unwrap()));
    }

    // Clear all
    ac.clear_all();

    // All should be denied
    for pid in 0..64 {
        assert!(!ac.is_pid_allowed(pid));
    }
    for cmd_num in 0..8 {
        assert!(!ac.is_command_allowed(Command::from_u8(cmd_num).unwrap()));
    }
}

#[test]
fn property_denial_audit_monotonic() {
    /// Property: access_denied_count should never decrease
    let ac = Arc::new(AccessControlCapsule::new());

    let mut denied_counts = vec![];

    for _ in 0..100 {
        let _ = ac.is_pid_allowed(1); // Try denied access
        let count = ac.get_stats().access_denied_count;
        denied_counts.push(count);
    }

    // Should be monotonically non-decreasing
    for i in 1..denied_counts.len() {
        assert!(
            denied_counts[i] >= denied_counts[i - 1],
            "Denial count should be monotonic"
        );
    }
}

// ============================================================================
// Q15-Q21: Integration Tests
// ============================================================================

#[test]
fn integration_multi_client_scenario() {
    /// Simulate 10 concurrent MCP clients with different access levels
    let ac = Arc::new(AccessControlCapsule::new());

    // Client 0: Debugger admin (all PIDs + all commands)
    ac.allow_pid(0).unwrap();
    for i in 0..8 {
        ac.allow_command(Command::from_u8(i).unwrap()).unwrap();
    }

    // Client 1-9: Limited (specific PIDs + read-only)
    for pid in 1..10 {
        ac.allow_pid(pid as u32).unwrap();
    }
    ac.allow_command(Command::Read).unwrap();
    ac.allow_command(Command::StackTrace).unwrap();

    let mut handles = vec![];

    // Admin client: should have full access
    let ac_clone = Arc::clone(&ac);
    handles.push(thread::spawn(move || {
        for _ in 0..1000 {
            assert!(ac_clone.check_access(0, Command::Write).is_ok());
            assert!(ac_clone.check_access(0, Command::Breakpoint).is_ok());
        }
    }));

    // Regular clients: should have limited access
    for client_id in 1..10 {
        let ac_clone = Arc::clone(&ac);
        let pid = client_id as u32;

        handles.push(thread::spawn(move || {
            // Should succeed: Read allowed
            for _ in 0..1000 {
                assert!(ac_clone.check_access(pid, Command::Read).is_ok());
            }

            // Should fail: Write denied
            for _ in 0..1000 {
                assert_eq!(
                    ac_clone.check_access(pid, Command::Write),
                    Err(AccessError::CommandNotAllowed { cmd: 1 })
                );
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify audit trail
    let stats = ac.get_stats();
    assert!(stats.access_denied_count > 0, "Should have recorded denials");
}

#[test]
fn integration_dynamic_whitelist_changes() {
    /// Test dynamic whitelist updates during access checks
    let ac = Arc::new(AccessControlCapsule::new());

    let barrier_start = Arc::new(std::sync::Barrier::new(2));
    let barrier_update = Arc::new(std::sync::Barrier::new(2));

    let barrier_start_clone = Arc::clone(&barrier_start);
    let barrier_update_clone = Arc::clone(&barrier_update);
    let ac_clone = Arc::clone(&ac);

    let reader = thread::spawn(move || {
        // Wait for writer to start
        barrier_start_clone.wait();

        let mut denied_before_update = 0;
        let mut allowed_after_update = 0;

        // Phase 1: Check before whitelist update
        for _ in 0..1000 {
            if !ac_clone.is_pid_allowed(1) {
                denied_before_update += 1;
            }
        }

        // Wait for writer to update whitelist
        barrier_update_clone.wait();

        // Phase 2: Check after whitelist update
        for _ in 0..1000 {
            if ac_clone.is_pid_allowed(1) {
                allowed_after_update += 1;
            }
        }

        (denied_before_update, allowed_after_update)
    });

    let barrier_start_clone = Arc::clone(&barrier_start);
    let barrier_update_clone = Arc::clone(&barrier_update);
    let ac_clone = Arc::clone(&ac);

    let writer = thread::spawn(move || {
        // Wait for reader to start
        barrier_start_clone.wait();

        // Let reader check a few times before update
        thread::sleep(std::time::Duration::from_millis(10));

        // Update whitelist
        ac_clone.allow_pid(1).unwrap();

        // Signal reader that update is done
        barrier_update_clone.wait();
    });

    let (denied_before, allowed_after) = reader.join().unwrap();
    writer.join().unwrap();

    assert!(denied_before > 0, "PID should be denied before whitelist update");
    assert!(allowed_after > 0, "PID should be allowed after whitelist update");
}

#[test]
fn integration_access_control_cascade() {
    /// Test cascading access control: PID must pass, then command must pass
    let ac = Arc::new(AccessControlCapsule::new());

    // Whitelist PID 5 with Read+Write
    ac.allow_pid(5).unwrap();
    ac.allow_command(Command::Read).unwrap();
    ac.allow_command(Command::Write).unwrap();

    let mut handles = vec![];

    // 100 concurrent requests to PID 5
    for _ in 0..100 {
        let ac_clone = Arc::clone(&ac);
        handles.push(thread::spawn(move || {
            let mut success_count = 0;
            for _ in 0..100 {
                if ac_clone.check_access(5, Command::Read).is_ok() {
                    success_count += 1;
                }
            }
            success_count
        }));
    }

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // All requests should succeed
    for success_count in results {
        assert_eq!(success_count, 100);
    }
}

// ============================================================================
// Q22-Q28: Production/Load Tests
// ============================================================================

#[test]
fn load_test_1m_checks_latency() {
    /// Load test: 1M access checks with latency tracking (B32 validation)
    let ac = Arc::new(AccessControlCapsule::new());
    ac.allow_pid(1).unwrap();

    let iterations = 1_000_000;
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = ac.is_pid_allowed(1);
    }

    let elapsed = start.elapsed();
    let latency_ns = elapsed.as_nanos() as f64 / iterations as f64;

    println!("1M checks latency: {:.2} ns/op", latency_ns);
    assert!(latency_ns < 20.0, "Latency should be <20ns (got {:.2}ns)", latency_ns);
}

#[test]
fn load_test_concurrent_stress() {
    /// Stress test: 16 threads, 100K operations each
    let ac = Arc::new(AccessControlCapsule::new());

    // Whitelist some PIDs
    for pid in 0..32 {
        ac.allow_pid(pid).unwrap();
    }

    let num_threads = 16;
    let ops_per_thread = 100_000;
    let mut handles = vec![];

    let start = Instant::now();

    for thread_id in 0..num_threads {
        let ac_clone = Arc::clone(&ac);
        handles.push(thread::spawn(move || {
            for i in 0..ops_per_thread {
                let pid = (thread_id + i) % 32;
                let _ = ac_clone.is_pid_allowed(pid as u32);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_ops = (num_threads * ops_per_thread) as u64;
    let throughput = (total_ops as f64 / elapsed.as_secs_f64()) as u64;

    println!("Concurrent stress: {} ops/sec", throughput);
    assert!(
        throughput > 100_000_000,
        "Should achieve >100M ops/sec (got {})", throughput
    );
}

#[test]
fn load_test_contentious_pid() {
    /// Contentious access: 16 threads all checking same PID concurrently
    let ac = Arc::new(AccessControlCapsule::new());
    ac.allow_pid(1).unwrap();

    let num_threads = 16;
    let iterations_per_thread = 100_000;
    let mut handles = vec![];

    let start = Instant::now();

    for _ in 0..num_threads {
        let ac_clone = Arc::clone(&ac);
        handles.push(thread::spawn(move || {
            for _ in 0..iterations_per_thread {
                let _ = ac_clone.is_pid_allowed(1);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_ops = (num_threads * iterations_per_thread) as u64;
    let throughput = (total_ops as f64 / elapsed.as_secs_f64()) as u64;

    println!("Contentious PID check: {} ops/sec", throughput);
    assert!(
        throughput > 50_000_000,
        "Should achieve >50M ops/sec even with contention (got {})", throughput
    );
}

#[test]
fn load_test_allow_deny_cycles() {
    /// Load test: Rapidly allow and deny PIDs
    let ac = Arc::new(AccessControlCapsule::new());

    let iterations = 100_000;
    let start = Instant::now();

    for i in 0..iterations {
        let pid = (i % 64) as u32;
        let _ = ac.allow_pid(pid);
        let _ = ac.is_pid_allowed(pid);
        ac.deny_pid(pid);
    }

    let elapsed = start.elapsed();
    let latency_ns = elapsed.as_nanos() as f64 / (iterations * 3) as f64; // 3 ops per iteration

    println!("Allow/deny/check cycle: {:.2} ns/op", latency_ns);
    assert!(latency_ns < 50.0, "Cycle latency should be <50ns");
}

#[test]
fn load_test_command_whitelist_contention() {
    /// Load test: Multiple threads updating command whitelist
    let ac = Arc::new(AccessControlCapsule::new());

    let num_threads = 8;
    let iterations = 10_000;
    let mut handles = vec![];

    let start = Instant::now();

    for _ in 0..num_threads {
        let ac_clone = Arc::clone(&ac);
        handles.push(thread::spawn(move || {
            for i in 0..iterations {
                let cmd = Command::from_u8((i % 8) as u8).unwrap();
                let _ = ac_clone.allow_command(cmd);
                let _ = ac_clone.is_command_allowed(cmd);
                ac_clone.deny_command(cmd);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_ops = (num_threads * iterations * 3) as u64; // 3 ops per iteration
    let throughput = (total_ops as f64 / elapsed.as_secs_f64()) as u64;

    println!("Command whitelist contention: {} ops/sec", throughput);
    assert!(
        throughput > 10_000_000,
        "Should achieve >10M ops/sec (got {})", throughput
    );
}

#[test]
fn load_test_audit_trail_contention() {
    /// Load test: Many threads generating denials (audit trail contention)
    let ac = Arc::new(AccessControlCapsule::new());
    let num_threads = 32;
    let iterations = 1_000;
    let mut handles = vec![];

    let start = Instant::now();

    for thread_id in 0..num_threads {
        let ac_clone = Arc::clone(&ac);
        handles.push(thread::spawn(move || {
            for i in 0..iterations {
                // Generate denied access (contends on access_denied_count)
                let pid = ((thread_id * iterations + i) % 1000) as u32;
                let _ = ac_clone.is_pid_allowed(pid);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_ops = (num_threads * iterations) as u64;
    let throughput = (total_ops as f64 / elapsed.as_secs_f64()) as u64;

    let stats = ac.get_stats();
    println!("Audit trail denials: {} ops/sec | Total denials: {}", throughput, stats.access_denied_count);

    assert!(stats.access_denied_count == (total_ops as u64));
    assert!(throughput > 10_000_000, "Should achieve >10M ops/sec");
}

#[test]
fn load_test_full_whitelist_scenario() {
    /// Production scenario: Allow all 64 PIDs + all 8 commands, 100K concurrent checks
    let ac = Arc::new(AccessControlCapsule::new());

    // Whitelist all PIDs and commands
    for pid in 0..64 {
        ac.allow_pid(pid).unwrap();
    }
    for i in 0..8 {
        ac.allow_command(Command::from_u8(i).unwrap()).unwrap();
    }

    let num_threads = 32;
    let iterations = 100_000;
    let ops_completed = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    let start = Instant::now();

    for _ in 0..num_threads {
        let ac_clone = Arc::clone(&ac);
        let ops_completed_clone = Arc::clone(&ops_completed);

        handles.push(thread::spawn(move || {
            for _ in 0..iterations {
                for pid in 0..64 {
                    if ac_clone.check_access(pid, Command::Read).is_ok() {
                        ops_completed_clone.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_ops = ops_completed.load(Ordering::Relaxed) as u64;
    let throughput = (total_ops as f64 / elapsed.as_secs_f64()) as u64;

    println!("Full whitelist scenario: {} ops/sec", throughput);
    assert!(throughput > 500_000_000, "Should achieve >500M ops/sec");
}

// ============================================================================
// Verification Tests (ASSUM)
// ============================================================================

#[test]
fn verify_assume_no_mutex_contention() {
    /// Verify there's no mutex-like contention (lockfree atomic operations only)
    /// Run with high thread count to detect lock contention
    let ac = Arc::new(AccessControlCapsule::new());

    for pid in 0..64 {
        ac.allow_pid(pid).unwrap();
    }

    let num_threads = 256; // High contention
    let iterations = 10_000;
    let mut handles = vec![];

    let start = Instant::now();

    for _ in 0..num_threads {
        let ac_clone = Arc::clone(&ac);
        handles.push(thread::spawn(move || {
            for i in 0..iterations {
                let pid = (i % 64) as u32;
                let _ = ac_clone.is_pid_allowed(pid);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_ops = (num_threads * iterations) as u64;
    let avg_latency_ns = (elapsed.as_nanos() as f64 / total_ops as f64);

    println!("256-thread contention latency: {:.2} ns/op", avg_latency_ns);

    // Even with 256 threads, should stay <100ns (no mutex queueing)
    assert!(
        avg_latency_ns < 100.0,
        "Lockfree should scale to 256 threads (got {:.2}ns)", avg_latency_ns
    );
}
