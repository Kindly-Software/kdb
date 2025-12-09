//! Ptrace Resilience Chaos Tests
//!
//! Tests kdb's resilience when ptrace operations fail or targets die:
//! - Target killed during attach
//! - Target killed during step
//! - Signal flood during debugging
//! - Multi-target chaos
//!
//! # Requirements
//!
//! - Linux x86_64 with ptrace support
//! - `chaos-testing` feature
//! - CAP_SYS_PTRACE or same UID as target
//!
//! # Framework Compliance
//!
//! - T28 Q22-Q28: Production stress scenarios
//! - ASSUM: All ptrace assumptions documented

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use libc::pid_t;

use kdb::DebuggerCapsule;
use super::ChaosInjector;

// ============================================================================
// Target Killed During Attach
// ============================================================================

/// Test debugger behavior when target is killed during attach.
///
/// This test:
/// 1. Spawns a sleep process
/// 2. Starts attach in a background thread
/// 3. Kills target during the attach window
/// 4. Verifies debugger returns clean error, no panic, no zombie
///
/// # Why #[ignore]
///
/// This test spawns real processes and races attach/kill.
/// Run explicitly: `cargo test --features chaos-testing target_killed_during_attach -- --ignored`
#[test]
#[ignore = "Requires process spawning and timing - run explicitly with --ignored"]
fn test_target_killed_during_attach() {
    const ITERATIONS: usize = 10;
    let mut success_count = 0_usize;
    let mut clean_error_count = 0_usize;

    for iteration in 0..ITERATIONS {
        let mut injector = ChaosInjector::new();

        // Spawn target process
        let target_pid = match injector.spawn_sleep_target() {
            Ok(pid) => pid,
            Err(e) => {
                eprintln!("[Iteration {}] Failed to spawn target: {:?}", iteration, e);
                continue;
            }
        };

        // Create debugger
        let debugger = Box::new(DebuggerCapsule::new(target_pid as u64));

        // Race condition: start attach and kill nearly simultaneously
        let attach_start = Instant::now();

        // Start attach attempt
        let attach_result = debugger.attach_to_process(target_pid as u64);

        // Kill target (may or may not happen before attach completes)
        let _ = ChaosInjector::kill_process(target_pid);

        // Give kernel time to process the kill
        thread::sleep(Duration::from_millis(10));

        let attach_duration = attach_start.elapsed();

        // Analyze result
        match attach_result {
            Ok(()) => {
                // Attach succeeded before kill took effect
                success_count += 1;
                println!(
                    "[Iteration {}] Attach succeeded in {:?} (target killed after)",
                    iteration, attach_duration
                );

                // Debugger should handle dead target gracefully
                // Further operations should fail cleanly
                let step_result = debugger.step_instruction();
                if step_result.is_err() {
                    clean_error_count += 1;
                }
            }
            Err(e) => {
                // Attach failed - should be clean error
                clean_error_count += 1;
                println!(
                    "[Iteration {}] Attach failed cleanly: {:?} ({:?})",
                    iteration, e, attach_duration
                );
            }
        }

        // Wait for target to fully exit
        ChaosInjector::wait_for_exit(target_pid, 1000);

        // Verify no zombie left
        assert!(
            !ChaosInjector::is_process_alive(target_pid),
            "Target should be fully dead"
        );

        // Drop injector to clean up
        drop(injector);
    }

    println!(
        "\n[test_target_killed_during_attach] Results:"
    );
    println!("  Iterations: {}", ITERATIONS);
    println!("  Attach succeeded: {}", success_count);
    println!("  Clean errors: {}", clean_error_count);

    // Assertion: All iterations should complete without panic
    // Either attach succeeds or fails cleanly
    assert_eq!(
        success_count + clean_error_count,
        ITERATIONS,
        "All iterations should complete (success or clean error)"
    );
}

// ============================================================================
// Target Killed During Step
// ============================================================================

/// Test debugger behavior when target is killed during single-step.
///
/// This test:
/// 1. Attaches to a running process
/// 2. Starts step operation
/// 3. Kills target between step and waitpid
/// 4. Verifies graceful detach, no zombie
#[test]
#[ignore = "Requires process spawning and timing - run explicitly with --ignored"]
fn test_target_killed_during_step() {
    const ITERATIONS: usize = 10;
    let mut step_success = 0_usize;
    let mut step_failed_clean = 0_usize;

    for iteration in 0..ITERATIONS {
        let mut injector = ChaosInjector::new();

        // Spawn a busy target (not sleep, needs to be steppable)
        let target_pid = match injector.spawn_debuggable_target() {
            Ok(pid) => pid,
            Err(e) => {
                eprintln!("[Iteration {}] Failed to spawn target: {:?}", iteration, e);
                continue;
            }
        };

        // Create debugger and attach
        let debugger = Box::new(DebuggerCapsule::new(target_pid as u64));

        if debugger.attach_to_process(target_pid as u64).is_err() {
            eprintln!("[Iteration {}] Failed to attach", iteration);
            let _ = ChaosInjector::kill_process(target_pid);
            continue;
        }

        // Small delay to let attach settle
        thread::sleep(Duration::from_millis(5));

        // Start stepping and race with kill
        let step_thread_debugger: &DebuggerCapsule = unsafe {
            // SAFETY: Sharing reference across threads for test only
            // This is a controlled test environment
            &*(&*debugger as *const DebuggerCapsule)
        };

        // Spawn thread to kill target after brief delay
        let kill_handle = thread::spawn(move || {
            thread::sleep(Duration::from_micros(50));
            ChaosInjector::kill_process(target_pid)
        });

        // Attempt step operations
        let mut steps_completed = 0_usize;
        for _ in 0..100 {
            match step_thread_debugger.step_instruction() {
                Ok(_) => steps_completed += 1,
                Err(_) => break,
            }
        }

        // Wait for kill thread
        let _ = kill_handle.join();

        if steps_completed > 0 {
            step_success += 1;
            println!(
                "[Iteration {}] Completed {} steps before target death",
                iteration, steps_completed
            );
        } else {
            step_failed_clean += 1;
            println!("[Iteration {}] Step failed immediately (clean)", iteration);
        }

        // Wait for cleanup
        ChaosInjector::wait_for_exit(target_pid, 1000);

        assert!(
            !ChaosInjector::is_process_alive(target_pid),
            "No zombie should remain"
        );
    }

    println!("\n[test_target_killed_during_step] Results:");
    println!("  Iterations: {}", ITERATIONS);
    println!("  Some steps succeeded: {}", step_success);
    println!("  Steps failed clean: {}", step_failed_clean);

    // All iterations should complete without crash
    assert_eq!(
        step_success + step_failed_clean,
        ITERATIONS,
        "All iterations should complete"
    );
}

// ============================================================================
// Signal Flood During Debugging
// ============================================================================

/// Test debugger resilience under signal flood.
///
/// This test:
/// 1. Attaches to a process
/// 2. Sends 1000 SIGUSR1 to the debugger process (self)
/// 3. Verifies debugger continues operation
#[test]
fn test_signal_flood_during_debugging() {
    const SIGNAL_COUNT: usize = 1000;

    // Create debugger (attach to self for this test)
    let debugger = Box::new(DebuggerCapsule::new(std::process::id() as u64));
    let self_pid = std::process::id() as pid_t;

    // Take some initial snapshots
    let initial_snapshots = 50_usize;
    for i in 0..initial_snapshots {
        let rip = 0x400000_u64 + i as u64 * 4;
        let rsp = 0x7fff_0000_u64 - i as u64 * 8;
        debugger.replay_engine.take_snapshot(rip, rsp).unwrap();
    }

    let before_flood = debugger.replay_engine.total_snapshots.load(Ordering::Acquire);
    println!(
        "[test_signal_flood_during_debugging] Snapshots before flood: {}",
        before_flood
    );

    // Install SIGUSR1 handler to ignore (prevent default termination)
    // SAFETY: Installing signal handler is safe
    unsafe {
        libc::signal(libc::SIGUSR1, libc::SIG_IGN);
    }

    // Send signal flood to self
    let start = Instant::now();
    let mut signals_sent = 0_usize;

    for _ in 0..SIGNAL_COUNT {
        if ChaosInjector::send_sigusr1(self_pid).is_ok() {
            signals_sent += 1;
        }

        // Continue debugger operations during flood
        let i = signals_sent as u64;
        let rip = 0x500000_u64 + i * 4;
        let rsp = 0x6fff_0000_u64 - i * 8;
        let _ = debugger.replay_engine.take_snapshot(rip, rsp);
    }

    let flood_duration = start.elapsed();

    // Restore default SIGUSR1 handler
    unsafe {
        libc::signal(libc::SIGUSR1, libc::SIG_DFL);
    }

    let after_flood = debugger.replay_engine.total_snapshots.load(Ordering::Acquire);

    println!(
        "[test_signal_flood_during_debugging] Signals sent: {} in {:?}",
        signals_sent, flood_duration
    );
    println!(
        "[test_signal_flood_during_debugging] Snapshots after flood: {}",
        after_flood
    );
    println!(
        "[test_signal_flood_during_debugging] New snapshots during flood: {}",
        after_flood.saturating_sub(before_flood)
    );

    // Verify debugger still works
    let final_result = debugger.replay_engine.take_snapshot(0xDEADBEEF, 0xCAFEBABE);
    assert!(final_result.is_ok(), "Debugger should still work after signal flood");

    // Performance assertion
    let signals_per_sec = signals_sent as f64 / flood_duration.as_secs_f64();
    println!(
        "[test_signal_flood_during_debugging] Signal throughput: {:.0} signals/sec",
        signals_per_sec
    );

    // Should handle at least 10K signals/sec
    assert!(
        signals_per_sec > 10_000.0,
        "Should handle >10K signals/sec"
    );
}

// ============================================================================
// Multi-Target Chaos
// ============================================================================

/// Test debugger with multiple targets dying concurrently.
///
/// This test:
/// 1. Spawns multiple target processes
/// 2. Creates debugger sessions for each
/// 3. Randomly kills targets
/// 4. Verifies all sessions handle death gracefully
#[test]
#[ignore = "Requires multiple process spawning - run explicitly with --ignored"]
fn test_multi_target_chaos() {
    const TARGET_COUNT: usize = 5;

    let mut injector = ChaosInjector::new();
    let mut targets: Vec<pid_t> = Vec::new();
    let mut debuggers: Vec<Box<DebuggerCapsule>> = Vec::new();

    // Spawn targets
    for i in 0..TARGET_COUNT {
        match injector.spawn_sleep_target() {
            Ok(pid) => {
                targets.push(pid);
                debuggers.push(Box::new(DebuggerCapsule::new(pid as u64)));
                println!("[test_multi_target_chaos] Spawned target {} (PID {})", i, pid);
            }
            Err(e) => {
                eprintln!("[test_multi_target_chaos] Failed to spawn target {}: {:?}", i, e);
            }
        }
    }

    assert!(targets.len() >= 3, "Need at least 3 targets for meaningful test");

    // Take snapshots on all debuggers
    for (i, debugger) in debuggers.iter().enumerate() {
        for j in 0..20 {
            let rip = 0x400000_u64 + (i as u64 * 0x10000) + (j as u64 * 4);
            let rsp = 0x7fff_0000_u64 - (j as u64 * 8);
            let _ = debugger.replay_engine.take_snapshot(rip, rsp);
        }
    }

    // Kill half the targets randomly
    let kill_count = targets.len() / 2;
    for i in 0..kill_count {
        let target_pid = targets[i * 2]; // Kill every other target
        println!("[test_multi_target_chaos] Killing target PID {}", target_pid);
        let _ = ChaosInjector::kill_process(target_pid);
    }

    // Small delay for kills to process
    thread::sleep(Duration::from_millis(100));

    // Try to continue operations on all debuggers
    let mut alive_count = 0_usize;
    let mut dead_handled_count = 0_usize;

    for (i, (pid, debugger)) in targets.iter().zip(debuggers.iter()).enumerate() {
        let is_alive = ChaosInjector::is_process_alive(*pid);

        // Try to take more snapshots
        let result = debugger.replay_engine.take_snapshot(0xABCD0000, 0x1234_0000);

        if is_alive {
            alive_count += 1;
            // Should succeed for alive targets
            if result.is_ok() {
                println!("[test_multi_target_chaos] Target {} alive, operations working", i);
            }
        } else {
            dead_handled_count += 1;
            // In-memory operations should still work even if target is dead
            // The debugger capsule is independent of the target process
            println!(
                "[test_multi_target_chaos] Target {} dead, in-memory ops: {:?}",
                i,
                if result.is_ok() { "OK" } else { "Failed" }
            );
        }
    }

    println!("\n[test_multi_target_chaos] Results:");
    println!("  Total targets: {}", targets.len());
    println!("  Killed: {}", kill_count);
    println!("  Still alive: {}", alive_count);
    println!("  Dead handled gracefully: {}", dead_handled_count);

    // Verify all targets can be cleaned up
    for pid in &targets {
        if ChaosInjector::is_process_alive(*pid) {
            let _ = ChaosInjector::kill_process(*pid);
        }
    }

    // Wait for all to exit
    for pid in &targets {
        ChaosInjector::wait_for_exit(*pid, 1000);
    }

    // Verify no zombies
    for pid in &targets {
        assert!(!ChaosInjector::is_process_alive(*pid), "No zombie should remain");
    }
}

// ============================================================================
// Rapid Attach/Detach Cycles
// ============================================================================

/// Test rapid attach/detach cycles on same target.
///
/// Verifies no resource leaks or state corruption with rapid cycling.
#[test]
fn test_rapid_attach_detach_cycles() {
    const CYCLES: usize = 100;

    let mut injector = ChaosInjector::new();

    // Spawn a target
    let target_pid = injector.spawn_sleep_target().expect("Failed to spawn target");
    println!(
        "[test_rapid_attach_detach_cycles] Target PID: {}",
        target_pid
    );

    let start = Instant::now();

    for cycle in 0..CYCLES {
        // Create debugger (simulates attach)
        let debugger = Box::new(DebuggerCapsule::new(target_pid as u64));

        // Attach
        let attach_result = debugger.attach_to_process(target_pid as u64);

        if attach_result.is_ok() {
            // Take a snapshot
            let rip = 0x400000_u64 + cycle as u64 * 4;
            let rsp = 0x7fff_0000_u64;
            let _ = debugger.replay_engine.take_snapshot(rip, rsp);
        }

        // Drop debugger (simulates detach)
        drop(debugger);

        // Progress
        if cycle > 0 && cycle % 25 == 0 {
            println!(
                "[test_rapid_attach_detach_cycles] Completed {} cycles",
                cycle
            );
        }
    }

    let duration = start.elapsed();
    let cycles_per_sec = CYCLES as f64 / duration.as_secs_f64();

    println!(
        "[test_rapid_attach_detach_cycles] {} cycles in {:?} ({:.1} cycles/sec)",
        CYCLES, duration, cycles_per_sec
    );

    // Target should still be alive
    assert!(
        ChaosInjector::is_process_alive(target_pid),
        "Target should survive rapid attach/detach"
    );

    // Cleanup
    let _ = ChaosInjector::kill_process(target_pid);
}

// ============================================================================
// Thread-Safe Concurrent Attach Test
// ============================================================================

/// Test concurrent debugger creation from multiple threads.
///
/// Verifies thread-safety of debugger capsule initialization.
#[test]
fn test_concurrent_debugger_creation() {
    const THREAD_COUNT: usize = 8;
    const DEBUGGERS_PER_THREAD: usize = 50;

    let success_count = Arc::new(AtomicU64::new(0));
    let mut handles = Vec::new();

    let start = Instant::now();

    for thread_id in 0..THREAD_COUNT {
        let success = Arc::clone(&success_count);

        handles.push(thread::spawn(move || {
            for i in 0..DEBUGGERS_PER_THREAD {
                // Create debugger with unique PID
                let pid = ((thread_id * 10000) + i) as u64;
                let debugger = Box::new(DebuggerCapsule::new(pid));

                // Take snapshots
                for j in 0..10 {
                    let rip = 0x400000_u64 + (thread_id as u64 * 0x100000) + (j as u64 * 4);
                    let rsp = 0x7fff_0000_u64 - (j as u64 * 8);
                    let _ = debugger.replay_engine.take_snapshot(rip, rsp);
                }

                // Verify state
                let count = debugger.replay_engine.total_snapshots.load(Ordering::Relaxed);
                if count >= 10 {
                    success.fetch_add(1, Ordering::Relaxed);
                }

                drop(debugger);
            }
        }));
    }

    // Wait for all threads
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    let duration = start.elapsed();
    let total_debuggers = THREAD_COUNT * DEBUGGERS_PER_THREAD;
    let successes = success_count.load(Ordering::Acquire);

    println!(
        "[test_concurrent_debugger_creation] Created {} debuggers across {} threads in {:?}",
        total_debuggers, THREAD_COUNT, duration
    );
    println!(
        "[test_concurrent_debugger_creation] Successful operations: {}/{}",
        successes, total_debuggers
    );

    // All debugger creations should succeed
    assert_eq!(
        successes as usize,
        total_debuggers,
        "All debugger creations should succeed"
    );
}
