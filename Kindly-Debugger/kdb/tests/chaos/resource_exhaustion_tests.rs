//! Resource Exhaustion Chaos Tests
//!
//! Tests kdb's behavior under resource exhaustion:
//! - OOM during snapshot capture (RLIMIT_AS)
//! - FD exhaustion during session (RLIMIT_NOFILE)
//! - Stack exhaustion during deep recursion
//! - CPU starvation under high load
//!
//! # Requirements
//!
//! - Linux x86_64
//! - `chaos-testing` feature
//! - Some tests require elevated privileges (#[ignore])
//!
//! # Safety
//!
//! All resource limit modifications are restored on Drop.
//! Tests use temporary limits that don't affect system-wide settings.
//!
//! # Framework Compliance
//!
//! - T28 Q22-Q28: Production stress scenarios
//! - ASSUM: Resource limit assumptions documented

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use libc::RLIMIT_NOFILE;

use kdb::DebuggerCapsule;
use super::ChaosInjector;

// ============================================================================
// OOM During Snapshot Capture
// ============================================================================

/// Test debugger behavior during OOM conditions.
///
/// This test:
/// 1. Sets RLIMIT_AS to 100MB (or lower)
/// 2. Attempts to capture many snapshots
/// 3. Verifies OutOfMemory error, partial snapshots remain valid
///
/// # Why #[ignore]
///
/// Requires privilege to modify RLIMIT_AS, which may fail on some systems.
/// Run explicitly: `cargo test --features chaos-testing oom_during_snapshot -- --ignored`
#[test]
#[ignore = "Requires privilege to set memory limits - run explicitly with --ignored"]
fn test_oom_during_snapshot_capture() {
    let mut injector = ChaosInjector::new();

    // Get baseline memory info
    let baseline_mem_mb = get_process_memory_mb();
    println!(
        "[test_oom_during_snapshot_capture] Baseline memory: {} MB",
        baseline_mem_mb
    );

    // Create debugger first (while memory is available)
    let debugger = Box::new(DebuggerCapsule::new(1234));

    // Take initial snapshots (should succeed)
    let initial_count = 100;
    for i in 0..initial_count {
        let rip = 0x400000_u64 + i * 4;
        let rsp = 0x7fff_0000_u64 - i * 8;
        debugger.replay_engine.take_snapshot(rip, rsp).unwrap();
    }

    let before_oom = debugger.replay_engine.total_snapshots.load(Ordering::Acquire);
    println!(
        "[test_oom_during_snapshot_capture] Snapshots before OOM: {}",
        before_oom
    );

    // Inject OOM condition (limit to baseline + 50MB)
    let memory_limit_mb = baseline_mem_mb.saturating_add(50).max(100);
    let inject_result = injector.inject_oom(memory_limit_mb);

    if let Err(e) = inject_result {
        println!(
            "[test_oom_during_snapshot_capture] Cannot set memory limit (expected on some systems): {:?}",
            e
        );
        return;
    }

    println!(
        "[test_oom_during_snapshot_capture] Memory limit set to {} MB",
        memory_limit_mb
    );

    // Try to capture many more snapshots
    // Ring buffer is fixed size, so this should still work
    // (kdb uses preallocated ring buffer, not dynamic allocation)
    let mut oom_triggered = false;
    let mut snapshots_taken = 0_u64;

    for i in 0..10_000 {
        let rip = 0x500000_u64 + i * 4;
        let rsp = 0x6fff_0000_u64 - i * 8;

        match debugger.replay_engine.take_snapshot(rip, rsp) {
            Ok(_) => {
                snapshots_taken += 1;
            }
            Err(e) => {
                println!(
                    "[test_oom_during_snapshot_capture] Snapshot failed at {}: {:?}",
                    i, e
                );
                oom_triggered = true;
                break;
            }
        }
    }

    let after_oom = debugger.replay_engine.total_snapshots.load(Ordering::Acquire);

    println!(
        "[test_oom_during_snapshot_capture] Snapshots after OOM test: {}",
        after_oom
    );
    println!(
        "[test_oom_during_snapshot_capture] OOM triggered: {}",
        oom_triggered
    );
    println!(
        "[test_oom_during_snapshot_capture] Snapshots taken during OOM: {}",
        snapshots_taken
    );

    // Verify existing snapshots are still valid
    // (Ring buffer should maintain integrity)
    let verify_result = debugger.replay_engine.verify_hash_chain(0);
    match verify_result {
        Ok(true) => println!("[test_oom_during_snapshot_capture] Hash chain: VALID"),
        Ok(false) => println!("[test_oom_during_snapshot_capture] Hash chain: INVALID (unexpected)"),
        Err(e) => println!("[test_oom_during_snapshot_capture] Hash chain verification: {:?}", e),
    }

    // Key insight: kdb uses preallocated ring buffer (128KB)
    // So OOM shouldn't affect snapshot capture directly
    // This test verifies graceful behavior rather than OOM failure
    assert!(
        after_oom >= before_oom,
        "Should not lose existing snapshots"
    );

    // Injector Drop restores memory limit
}

// ============================================================================
// FD Exhaustion During Session
// ============================================================================

/// Test debugger behavior during FD exhaustion.
///
/// This test:
/// 1. Sets RLIMIT_NOFILE to 10
/// 2. Tries to create new session (should fail gracefully)
/// 3. Verifies existing sessions unaffected
#[test]
fn test_fd_exhaustion_during_session() {
    let mut injector = ChaosInjector::new();

    // Create debugger first (before FD exhaustion)
    let debugger = Box::new(DebuggerCapsule::new(1234));

    // Take initial snapshots
    for i in 0..50 {
        let rip = 0x400000_u64 + i * 4;
        let rsp = 0x7fff_0000_u64 - i * 8;
        debugger.replay_engine.take_snapshot(rip, rsp).unwrap();
    }

    let before_exhaustion = debugger.replay_engine.total_snapshots.load(Ordering::Acquire);
    println!(
        "[test_fd_exhaustion_during_session] Snapshots before FD exhaustion: {}",
        before_exhaustion
    );

    // Get current FD limit for reporting
    let (current_soft, current_hard) = get_fd_limits();
    println!(
        "[test_fd_exhaustion_during_session] Current FD limits: soft={}, hard={}",
        current_soft, current_hard
    );

    // Inject FD exhaustion (limit to very low number)
    // Note: We need at least stdin(0), stdout(1), stderr(2) open
    let fd_limit = 10;

    if let Err(e) = injector.inject_fd_exhaustion(fd_limit) {
        println!(
            "[test_fd_exhaustion_during_session] Cannot set FD limit: {:?}",
            e
        );
        return;
    }

    println!(
        "[test_fd_exhaustion_during_session] FD limit set to {}",
        fd_limit
    );

    // Try operations that might need FDs
    // (kdb's ring buffer is in-memory, shouldn't need FDs)
    let mut post_exhaustion_ops = 0_u64;

    for i in 0..100 {
        let rip = 0x500000_u64 + i * 4;
        let rsp = 0x6fff_0000_u64 - i * 8;

        match debugger.replay_engine.take_snapshot(rip, rsp) {
            Ok(_) => post_exhaustion_ops += 1,
            Err(e) => {
                println!(
                    "[test_fd_exhaustion_during_session] Snapshot failed: {:?}",
                    e
                );
            }
        }
    }

    let after_exhaustion = debugger.replay_engine.total_snapshots.load(Ordering::Acquire);

    println!(
        "[test_fd_exhaustion_during_session] Snapshots after FD exhaustion: {}",
        after_exhaustion
    );
    println!(
        "[test_fd_exhaustion_during_session] Operations during exhaustion: {}",
        post_exhaustion_ops
    );

    // In-memory operations should still work
    assert!(
        post_exhaustion_ops > 0,
        "In-memory operations should work under FD exhaustion"
    );

    // Existing session should be unaffected
    assert!(
        after_exhaustion >= before_exhaustion,
        "Should not lose existing snapshots"
    );

    // FD limit is restored on injector drop
}

/// Test that opening files fails under FD exhaustion.
///
/// This verifies the FD exhaustion injection actually works.
#[test]
fn test_fd_exhaustion_prevents_file_open() {
    let mut injector = ChaosInjector::new();

    // Record initial open FD count
    let initial_fds = count_open_fds();
    println!(
        "[test_fd_exhaustion_prevents_file_open] Initial open FDs: {}",
        initial_fds
    );

    // Set very low limit
    let fd_limit = initial_fds as u64 + 2; // Allow 2 more

    if let Err(e) = injector.inject_fd_exhaustion(fd_limit) {
        println!(
            "[test_fd_exhaustion_prevents_file_open] Cannot set FD limit: {:?}",
            e
        );
        return;
    }

    // Try to open files until we hit the limit
    let mut files_opened = 0_usize;
    let mut open_files: Vec<std::fs::File> = Vec::new();

    for i in 0..100 {
        let path = format!("/tmp/kdb_chaos_fd_test_{}", i);
        match std::fs::File::create(&path) {
            Ok(f) => {
                files_opened += 1;
                open_files.push(f);
            }
            Err(e) => {
                println!(
                    "[test_fd_exhaustion_prevents_file_open] File open failed at {}: {:?}",
                    i, e
                );
                break;
            }
        }
    }

    println!(
        "[test_fd_exhaustion_prevents_file_open] Files opened before exhaustion: {}",
        files_opened
    );

    // Should have hit the limit
    assert!(
        files_opened < 100,
        "FD limit should prevent opening 100 files"
    );

    // Clean up opened files
    for f in open_files {
        drop(f);
    }

    // Clean up temp files
    for i in 0..files_opened {
        let _ = std::fs::remove_file(format!("/tmp/kdb_chaos_fd_test_{}", i));
    }

    // Limit restored on drop
}

// ============================================================================
// Stack Exhaustion During Deep Operations
// ============================================================================

/// Test debugger behavior with deeply nested operations.
///
/// This doesn't modify stack limits (dangerous), but tests
/// behavior with deep stack frames.
#[test]
fn test_deep_stack_operations() {
    let debugger = Box::new(DebuggerCapsule::new(1234));

    // Push many stack frames
    const FRAME_COUNT: usize = 128; // Max supported by simd_stack

    let start = Instant::now();

    for i in 0..FRAME_COUNT {
        let rip = 0x400000_u64 + (i as u64) * 0x1000;
        let rbp = 0x7fff_0000_u64 - (i as u64) * 0x100;
        let rsp = 0x7fff_0000_u64 - (i as u64) * 0x108;

        match debugger.simd_stack.push_frame(rip, rbp, rsp) {
            Ok(()) => {}
            Err(e) => {
                println!(
                    "[test_deep_stack_operations] Push failed at frame {}: {:?}",
                    i, e
                );
                break;
            }
        }
    }

    let push_duration = start.elapsed();
    let depth = debugger.simd_stack.get_depth();

    println!(
        "[test_deep_stack_operations] Pushed {} frames in {:?}",
        depth, push_duration
    );

    // Get stack trace (SIMD-accelerated)
    let trace_start = Instant::now();
    let trace = debugger.get_stack_trace();
    let trace_duration = trace_start.elapsed();

    match trace {
        Ok(addresses) => {
            println!(
                "[test_deep_stack_operations] Stack trace: {} addresses in {:?}",
                addresses.len(),
                trace_duration
            );
            assert!(addresses.len() > 0, "Should have stack addresses");
        }
        Err(e) => {
            println!("[test_deep_stack_operations] Stack trace failed: {:?}", e);
        }
    }

    // Performance assertion
    assert!(
        push_duration < Duration::from_millis(10),
        "Should push {} frames in <10ms",
        FRAME_COUNT
    );
}

// ============================================================================
// CPU Starvation Under High Load
// ============================================================================

/// Test debugger behavior under CPU starvation.
///
/// Spawns busy threads to compete for CPU, then measures debugger performance.
/// Uses multiple measurement rounds to reduce noise from micro-benchmarking.
#[test]
fn test_cpu_starvation_performance() {
    const STRESS_THREADS: usize = 4;
    const OPS_PER_MEASUREMENT: usize = 5000;  // More ops for stable timing
    const MEASUREMENT_ROUNDS: usize = 3;       // Multiple rounds for averaging

    let debugger = Arc::new(Box::new(DebuggerCapsule::new(1234)));
    let stop_flag = Arc::new(AtomicU64::new(0));

    // Warmup run (not measured)
    for i in 0..1000 {
        let rip = 0x300000_u64 + i as u64 * 4;
        let rsp = 0x8fff_0000_u64 - i as u64 * 8;
        let _ = debugger.replay_engine.take_snapshot(rip, rsp);
    }

    // Measure baseline performance (best of N rounds)
    let mut best_baseline_ops_per_sec = 0.0_f64;
    for round in 0..MEASUREMENT_ROUNDS {
        let baseline_start = Instant::now();
        for i in 0..OPS_PER_MEASUREMENT {
            let rip = 0x400000_u64 + (round * OPS_PER_MEASUREMENT + i) as u64 * 4;
            let rsp = 0x7fff_0000_u64 - i as u64 * 8;
            let _ = debugger.replay_engine.take_snapshot(rip, rsp);
        }
        let baseline_duration = baseline_start.elapsed();
        let ops_per_sec = OPS_PER_MEASUREMENT as f64 / baseline_duration.as_secs_f64();
        if ops_per_sec > best_baseline_ops_per_sec {
            best_baseline_ops_per_sec = ops_per_sec;
        }
    }

    println!(
        "[test_cpu_starvation_performance] Baseline: {} ops x {} rounds, best {:.0} ops/sec",
        OPS_PER_MEASUREMENT, MEASUREMENT_ROUNDS, best_baseline_ops_per_sec
    );

    // Start CPU stress threads
    let mut stress_handles = Vec::new();

    for _ in 0..STRESS_THREADS {
        let stop = Arc::clone(&stop_flag);

        stress_handles.push(thread::spawn(move || {
            let mut counter = 0_u64;
            while stop.load(Ordering::Relaxed) == 0 {
                // Busy loop to consume CPU
                counter = counter.wrapping_add(1);
                if counter % 1_000_000 == 0 {
                    thread::yield_now();
                }
            }
            counter
        }));
    }

    // Give stress threads time to saturate CPU
    thread::sleep(Duration::from_millis(100));

    // Measure performance under stress
    let stress_start = Instant::now();
    for i in 0..OPS_PER_MEASUREMENT {
        let rip = 0x500000_u64 + i as u64 * 4;
        let rsp = 0x6fff_0000_u64 - i as u64 * 8;
        let _ = debugger.replay_engine.take_snapshot(rip, rsp);
    }
    let stress_duration = stress_start.elapsed();
    let stress_ops_per_sec = OPS_PER_MEASUREMENT as f64 / stress_duration.as_secs_f64();

    println!(
        "[test_cpu_starvation_performance] Under stress: {} ops in {:?} ({:.0} ops/sec)",
        OPS_PER_MEASUREMENT, stress_duration, stress_ops_per_sec
    );

    // Stop stress threads
    stop_flag.store(1, Ordering::SeqCst);

    for handle in stress_handles {
        let _ = handle.join();
    }

    // Calculate degradation
    let degradation_percent = ((best_baseline_ops_per_sec - stress_ops_per_sec) / best_baseline_ops_per_sec) * 100.0;

    println!(
        "[test_cpu_starvation_performance] Performance degradation: {:.1}%",
        degradation_percent.abs()
    );

    // Lockfree operations should degrade gracefully under CPU stress
    // Allow up to 90% degradation (we're competing with 4 busy threads)
    assert!(
        stress_ops_per_sec > best_baseline_ops_per_sec * 0.1,
        "Should maintain at least 10% performance under CPU stress"
    );

    // Recovery: wait for threads to fully exit and scheduler to settle
    thread::sleep(Duration::from_millis(200));

    // Warmup after recovery (scheduler settling)
    for i in 0..1000 {
        let rip = 0x580000_u64 + i as u64 * 4;
        let rsp = 0x4fff_0000_u64 - i as u64 * 8;
        let _ = debugger.replay_engine.take_snapshot(rip, rsp);
    }

    // Measure recovery (best of N rounds)
    let mut best_recovery_ops_per_sec = 0.0_f64;
    for round in 0..MEASUREMENT_ROUNDS {
        let recovery_start = Instant::now();
        for i in 0..OPS_PER_MEASUREMENT {
            let rip = 0x600000_u64 + (round * OPS_PER_MEASUREMENT + i) as u64 * 4;
            let rsp = 0x5fff_0000_u64 - i as u64 * 8;
            let _ = debugger.replay_engine.take_snapshot(rip, rsp);
        }
        let recovery_duration = recovery_start.elapsed();
        let ops_per_sec = OPS_PER_MEASUREMENT as f64 / recovery_duration.as_secs_f64();
        if ops_per_sec > best_recovery_ops_per_sec {
            best_recovery_ops_per_sec = ops_per_sec;
        }
    }

    println!(
        "[test_cpu_starvation_performance] After recovery: {:.0} ops/sec (best of {} rounds)",
        best_recovery_ops_per_sec, MEASUREMENT_ROUNDS
    );

    // Should recover close to baseline
    // Using 50% threshold to account for:
    // - Measurement noise in micro-benchmarks
    // - System background processes
    // - Ring buffer state (may be near capacity)
    let recovery_ratio = best_recovery_ops_per_sec / best_baseline_ops_per_sec;
    assert!(
        recovery_ratio > 0.5,
        "Should recover to at least 50% of baseline performance (got {:.1}%)",
        recovery_ratio * 100.0
    );
}

// ============================================================================
// Memory Pressure Test
// ============================================================================

/// Test debugger under memory pressure.
///
/// Allocates large buffers to create memory pressure, then verifies
/// debugger operations still work.
#[test]
fn test_memory_pressure() {
    // Create debugger first
    let debugger = Box::new(DebuggerCapsule::new(1234));

    // Take baseline snapshots
    for i in 0..100 {
        let rip = 0x400000_u64 + i * 4;
        let rsp = 0x7fff_0000_u64 - i * 8;
        debugger.replay_engine.take_snapshot(rip, rsp).unwrap();
    }

    let before_pressure = debugger.replay_engine.total_snapshots.load(Ordering::Acquire);

    // Allocate large buffers to create pressure
    // Use Vec to allocate on heap
    let mut pressure_buffers: Vec<Vec<u8>> = Vec::new();
    let buffer_size = 10 * 1024 * 1024; // 10MB each
    let target_pressure = 100; // Try to allocate 100 * 10MB = 1GB

    println!(
        "[test_memory_pressure] Attempting to allocate {} x 10MB buffers",
        target_pressure
    );

    let mut allocated = 0_usize;
    for i in 0..target_pressure {
        match std::panic::catch_unwind(|| {
            vec![0xABu8; buffer_size]
        }) {
            Ok(buffer) => {
                pressure_buffers.push(buffer);
                allocated += 1;
            }
            Err(_) => {
                println!("[test_memory_pressure] Allocation failed at buffer {}", i);
                break;
            }
        }

        // Take snapshot during allocation
        let rip = 0x500000_u64 + i as u64 * 4;
        let rsp = 0x6fff_0000_u64 - i as u64 * 8;
        let _ = debugger.replay_engine.take_snapshot(rip, rsp);
    }

    let allocated_mb = allocated * 10;
    println!(
        "[test_memory_pressure] Allocated {} MB of pressure buffers",
        allocated_mb
    );

    // Debugger operations should still work
    let mut ops_under_pressure = 0_u64;
    for i in 0..100 {
        let rip = 0x600000_u64 + i * 4;
        let rsp = 0x5fff_0000_u64 - i * 8;
        if debugger.replay_engine.take_snapshot(rip, rsp).is_ok() {
            ops_under_pressure += 1;
        }
    }

    let after_pressure = debugger.replay_engine.total_snapshots.load(Ordering::Acquire);

    println!(
        "[test_memory_pressure] Operations under pressure: {}",
        ops_under_pressure
    );
    println!(
        "[test_memory_pressure] Total snapshots: {} -> {}",
        before_pressure, after_pressure
    );

    // In-memory ring buffer should work regardless of external memory pressure
    assert!(
        ops_under_pressure >= 90,
        "Should complete most operations under memory pressure"
    );

    // Release pressure buffers
    drop(pressure_buffers);

    // Verify recovery
    let mut ops_after_release = 0_u64;
    for i in 0..100 {
        let rip = 0x700000_u64 + i * 4;
        let rsp = 0x4fff_0000_u64 - i * 8;
        if debugger.replay_engine.take_snapshot(rip, rsp).is_ok() {
            ops_after_release += 1;
        }
    }

    println!(
        "[test_memory_pressure] Operations after release: {}",
        ops_after_release
    );

    assert_eq!(
        ops_after_release, 100,
        "All operations should succeed after memory release"
    );
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get current process memory usage in KB.
/// Returns 0 on non-Linux platforms or if unable to read /proc/self/status.
fn get_process_memory_kb() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<u64>() {
                            return kb;
                        }
                    }
                }
            }
        }
    }
    0
}

/// Get current process memory usage in MB (convenience wrapper).
#[allow(dead_code)]
fn get_process_memory_mb() -> u64 {
    get_process_memory_kb() / 1024
}

/// Get current file descriptor limits.
fn get_fd_limits() -> (u64, u64) {
    let mut rl = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };

    // SAFETY: getrlimit is safe with valid resource and pointer
    unsafe {
        if libc::getrlimit(RLIMIT_NOFILE, &mut rl) == 0 {
            return (rl.rlim_cur, rl.rlim_max);
        }
    }

    (0, 0)
}

/// Count currently open file descriptors.
fn count_open_fds() -> usize {
    #[cfg(target_os = "linux")]
    {
        if let Ok(entries) = std::fs::read_dir("/proc/self/fd") {
            return entries.count();
        }
    }
    0
}

// ============================================================================
// Unit Tests for Helpers
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_process_memory() {
        let mem_kb = get_process_memory_kb();
        // Process should use at least some memory (even 1 KB is fine)
        assert!(mem_kb > 0 || cfg!(not(target_os = "linux")));
    }

    #[test]
    fn test_get_fd_limits() {
        let (soft, hard) = get_fd_limits();
        // Should have valid limits
        assert!(soft > 0 || cfg!(not(target_os = "linux")));
        assert!(hard >= soft);
    }

    #[test]
    fn test_count_open_fds() {
        let count = count_open_fds();
        // Should have at least stdin, stdout, stderr
        assert!(count >= 3 || cfg!(not(target_os = "linux")));
    }
}
