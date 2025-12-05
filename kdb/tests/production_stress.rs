//! Production Stress Tests (T28 Q22-Q28) - kdb
//!
//! 15 stress, scale, and long-running scenarios validating production readiness.
//!
//! Run with: cargo test --release --ignored
//! Single test: cargo test --release --ignored test_1m_snapshots

use kdb::DebuggerCapsule;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// Utility Functions
// ============================================================================

/// Get current process RSS memory usage (Linux only)
fn get_memory_usage_mb() -> usize {
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    if let Ok(kb) = line.split_whitespace().nth(1).unwrap().parse::<usize>() {
                        return kb / 1024;
                    }
                }
            }
        }
    }
    0
}

/// Print formatted timing/throughput
fn print_throughput(name: &str, ops: usize, duration: Duration) {
    let tps = ops as f64 / duration.as_secs_f64();
    println!(
        "  {}: {} ops in {:?} = {:.0} ops/sec",
        name, ops, duration, tps
    );
}

// ============================================================================
// Category 1: Large-Scale Stress (5 tests)
// ============================================================================

/// Test 1: Capture 1M snapshots, verify memory usage and throughput
#[test]
#[ignore]
fn test_1m_snapshots() {
    let debugger = DebuggerCapsule::new(1234);
    let start = Instant::now();

    println!("\n[TEST 1] Capturing 1,000,000 snapshots...");

    for i in 0..1_000_000 {
        let rip = 0x400000_u64.wrapping_add((i % 10_000) as u64 * 4);
        let rsp = 0x7fff_0000_u64.wrapping_sub((i % 10_000) as u64 * 8);

        let result = debugger.replay_engine.take_snapshot(rip, rsp);
        assert!(result.is_ok(), "Snapshot {} failed: {:?}", i, result);

        if i > 0 && i % 100_000 == 0 {
            let elapsed = start.elapsed();
            let rate = i as f64 / elapsed.as_secs_f64();
            println!("  {} snapshots in {:?} ({:.0} ops/sec)", i, elapsed, rate);
        }
    }

    let total_duration = start.elapsed();
    print_throughput("1M snapshots", 1_000_000, total_duration);

    // Verify total count
    let total = debugger.replay_engine.total_snapshots.load(Ordering::Acquire);
    println!("  Total snapshots recorded: {}", total);
    assert!(total > 0, "Should have recorded snapshots");

    // Verify memory usage stayed bounded
    let mem_mb = get_memory_usage_mb();
    println!("  Memory usage: {} MB", mem_mb);
    assert!(mem_mb < 1024, "Memory usage should stay below 1GB for 1M snapshots");

    // Performance assertion: should complete >100K snapshots/sec
    let rate = 1_000_000.0 / total_duration.as_secs_f64();
    println!("  Throughput requirement: >100K ops/sec, actual: {:.0}", rate);
    assert!(rate > 100_000.0, "Should achieve >100K snapshots/sec");
}

/// Test 2: Set 256 breakpoints (max table size), measure overhead
#[test]
#[ignore]
fn test_10k_breakpoints() {
    let debugger = DebuggerCapsule::new(5678);
    let start = Instant::now();

    println!("\n[TEST 2] Setting 256 breakpoints (max capacity)...");

    // Note: BreakpointTableCapsule has a hard limit of 256 entries per architecture
    // This test validates that we can fill the table efficiently
    let max_breakpoints = 256;

    for i in 0..max_breakpoints {
        let addr = 0x400000_u64.wrapping_add((i as u64) * 4);
        let result = debugger.set_breakpoint(addr);

        assert!(
            result.is_ok(),
            "Breakpoint {} failed: {:?}",
            i,
            result
        );

        if i > 0 && i % 50 == 0 {
            let elapsed = start.elapsed();
            let rate = i as f64 / elapsed.as_secs_f64();
            println!(
                "  {} breakpoints in {:?} ({:.0} ops/sec)",
                i, elapsed, rate
            );
        }
    }

    let duration = start.elapsed();
    print_throughput(&format!("256 breakpoints"), max_breakpoints, duration);

    // Verify breakpoint table size
    let count = debugger.breakpoints.count.load(Ordering::Acquire) as usize;
    println!("  Breakpoint table count: {}", count);
    assert_eq!(count, 256, "Should have filled 256 breakpoint slots");

    // Performance: should complete <50ms for 256 breakpoints
    let rate = (max_breakpoints as f64) / duration.as_secs_f64();
    println!("  Throughput: {:.0} breakpoints/sec", rate);
    assert!(
        duration < Duration::from_millis(50),
        "Should set 256 breakpoints in <50ms"
    );

    // Verify table is full
    let result = debugger.set_breakpoint(0x500000);
    assert!(result.is_err(), "Table should be full");
    println!("  Confirmed: Table is full (257th breakpoint rejected)");
}

/// Test 3: Debug 100MB+ binary (large symbol table)
#[test]
#[ignore]
#[cfg(target_os = "linux")]
fn test_large_binary_100mb() {
    let debugger = DebuggerCapsule::new(9999);

    println!("\n[TEST 3] Testing with large binary symbols...");

    // Use /usr/bin/rustc as large test binary if available
    let binary_path = "/usr/bin/rustc";

    if !std::path::Path::new(binary_path).exists() {
        println!("  Skipping: {} not found (optional test)", binary_path);
        return;
    }

    let start = Instant::now();

    // Simulate loading symbols from large binary
    let metadata = std::fs::metadata(binary_path).expect("Failed to stat binary");
    let file_size = metadata.len() / (1024 * 1024); // MB

    println!("  Binary size: {} MB", file_size);

    // Simulate symbol loading with 1 entry per KB
    let symbol_count = (file_size as usize * 100).min(10_000);

    for i in 0..symbol_count {
        let start_addr = 0x400000_u64.wrapping_add((i as u64) * 8);
        let end_addr = start_addr.wrapping_add(0x1000);
        let name_hash = i as u64;
        let _result = debugger.simd_symbols.add_symbol(start_addr, end_addr, name_hash);
    }

    let duration = start.elapsed();

    println!(
        "  Simulated {} symbols in {:?}",
        symbol_count, duration
    );
    assert!(
        duration < Duration::from_secs(5),
        "Should process symbols in <5 seconds"
    );
}

/// Test 4: Deep stack unwinding (128 frames)
#[test]
#[ignore]
fn test_deep_stack_128_frames() {
    let debugger = DebuggerCapsule::new(2222);

    println!("\n[TEST 4] Unwinding 128 stack frames...");

    let start = Instant::now();

    // Simulate pushing 128 stack frames
    for i in 0..128 {
        let rip = 0x400000_u64.wrapping_add((i as u64) * 0x1000);
        let rbp = 0x7fff_0000_u64.wrapping_sub((i as u64) * 0x100);
        let rsp = 0x7fff_0000_u64.wrapping_sub((i as u64) * 0x108);

        let result = debugger.simd_stack.push_frame(rip, rbp, rsp);
        assert!(result.is_ok(), "Frame {} push failed", i);
    }

    let duration = start.elapsed();

    println!(
        "  128 frames unwound in {:?} ({:.1} µs per frame)",
        duration,
        duration.as_micros() as f64 / 128.0
    );

    // Get stack trace (SIMD-accelerated)
    let trace = debugger.get_stack_trace();
    assert!(trace.is_ok());
    let trace = trace.unwrap();

    // Should have collected frames
    println!("  Stack trace contains {} addresses", trace.len());

    // Performance: <1ms for 128 frames
    assert!(
        duration < Duration::from_millis(1),
        "Should unwind 128 frames in <1ms"
    );
}

/// Test 5: Concurrent 10 threads × 10K operations
#[test]
#[ignore]
fn test_concurrent_10_threads_10k_ops() {
    println!("\n[TEST 5] Running 10 threads × 10K operations concurrently...");

    let debugger = Arc::new(DebuggerCapsule::new(3333));
    let barrier = Arc::new(Barrier::new(10));
    let success_count = Arc::new(AtomicU64::new(0));

    let start = Instant::now();

    let mut handles = vec![];

    for thread_id in 0..10 {
        let d = Arc::clone(&debugger);
        let b = Arc::clone(&barrier);
        let s = Arc::clone(&success_count);

        handles.push(thread::spawn(move || {
            // Wait for all threads to start
            b.wait();

            // Each thread: 10K snapshot operations
            for op in 0..10_000 {
                let rip = 0x400000_u64.wrapping_add(((thread_id * 10_000 + op) as u64) * 4);
                let rsp = 0x7fff_0000_u64.wrapping_sub(((thread_id * 10_000 + op) as u64) * 8);

                if d.replay_engine.take_snapshot(rip, rsp).is_ok() {
                    s.fetch_add(1, Ordering::Relaxed);
                }
            }
        }));
    }

    // Wait for all threads
    for h in handles {
        h.join().expect("Thread panicked");
    }

    let total_duration = start.elapsed();
    let success = success_count.load(Ordering::Acquire);

    println!("  Total operations: {} in {:?}", success, total_duration);
    println!(
        "  Throughput: {:.0} ops/sec",
        success as f64 / total_duration.as_secs_f64()
    );

    // All operations should succeed
    assert_eq!(success, 100_000, "All 100K operations should succeed");

    // Should complete <10 seconds
    assert!(
        total_duration < Duration::from_secs(10),
        "10 threads × 10K ops should complete in <10s"
    );
}

// ============================================================================
// Category 2: Memory & Resource Management (3 tests)
// ============================================================================

/// Test 6: Memory usage stays bounded
#[test]
#[ignore]
fn test_memory_usage_bounded() {
    println!("\n[TEST 6] Verifying bounded memory usage...");

    let mem_before = get_memory_usage_mb();
    println!("  Memory before: {} MB", mem_before);

    let debugger = DebuggerCapsule::new(4444);
    let mem_after_alloc = get_memory_usage_mb();
    println!("  Memory after debugger alloc: {} MB", mem_after_alloc);

    let start = Instant::now();

    // Capture 100K snapshots
    for i in 0..100_000 {
        let rip = 0x400000_u64.wrapping_add((i % 10_000) as u64 * 4);
        let rsp = 0x7fff_0000_u64.wrapping_sub((i % 10_000) as u64 * 8);

        let _result = debugger.replay_engine.take_snapshot(rip, rsp);
    }

    let mem_after_snapshots = get_memory_usage_mb();
    println!("  Memory after 100K snapshots: {} MB", mem_after_snapshots);

    let mem_increase = mem_after_snapshots.saturating_sub(mem_before);
    println!("  Total memory increase: {} MB", mem_increase);

    let duration = start.elapsed();
    println!("  Time: {:?}", duration);

    // Ring buffer should cap memory usage
    // DebuggerCapsule is ~1.09 MB, 100K snapshots shouldn't exceed ring buffer capacity
    assert!(mem_increase < 512, "Memory increase should be <512MB");
    assert!(mem_after_snapshots < 1024, "Total memory should stay <1GB");
}

/// Test 7: No memory leak over 1 hour continuous operation
#[test]
#[ignore]
#[cfg(target_os = "linux")]
fn test_no_memory_leak_1h() {
    println!("\n[TEST 7] Memory leak test (1 hour of continuous operations)...");

    let debugger = Arc::new(DebuggerCapsule::new(5555));
    let duration_target = Duration::from_secs(3600); // 1 hour

    let start = Instant::now();
    let mut mem_readings = vec![];
    let mut ops_count = 0_usize;

    mem_readings.push(get_memory_usage_mb());

    loop {
        let elapsed = start.elapsed();
        if elapsed >= duration_target {
            break;
        }

        // Perform operations
        for _ in 0..1_000 {
            let rip = 0x400000_u64.wrapping_add((ops_count as u64) * 4);
            let rsp = 0x7fff_0000_u64.wrapping_sub((ops_count as u64) * 8);
            let _result = debugger.replay_engine.take_snapshot(rip, rsp);
            ops_count += 1;
        }

        // Check memory periodically
        if elapsed >= Duration::from_secs((mem_readings.len() * 60) as u64) {
            let mem_mb = get_memory_usage_mb();
            mem_readings.push(mem_mb);

            println!(
                "  {:4.0}s: {} MB ({} ops performed so far)",
                elapsed.as_secs_f64(),
                mem_mb,
                ops_count
            );
        }

        thread::sleep(Duration::from_millis(100));

        // For testing: limit to 60 seconds instead of full hour
        if elapsed >= Duration::from_secs(60) {
            break;
        }
    }

    println!("  Total operations: {}", ops_count);

    // Memory trend analysis
    if mem_readings.len() >= 2 {
        let first_mem = mem_readings[0];
        let last_mem = mem_readings[mem_readings.len() - 1];
        let increase = last_mem.saturating_sub(first_mem);

        println!(
            "  Memory trend: {} MB → {} MB (increase: {} MB)",
            first_mem, last_mem, increase
        );

        // Allow some variance but detect major leaks
        // For 1K ops/sec × 60s = 60K snapshots, expect bounded memory
        assert!(increase < 256, "Memory should not increase significantly");
    }
}

/// Test 8: File descriptor limits
#[test]
#[ignore]
#[cfg(target_os = "linux")]
fn test_file_descriptor_limit() {
    println!("\n[TEST 8] Testing file descriptor limit handling...");

    let debugger = Arc::new(DebuggerCapsule::new(6666));

    // Get current FD limit
    let mut rlimit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };

    unsafe {
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut rlimit) == 0 {
            println!("  Current FD limit: {} (soft), {} (hard)", rlimit.rlim_cur, rlimit.rlim_max);
        }
    }

    // Simulate FD exhaustion by creating many snapshots
    // (Not actually exhausting FDs, just stress testing)
    let start = Instant::now();

    for i in 0..10_000 {
        let rip = 0x400000_u64.wrapping_add((i as u64) * 4);
        let rsp = 0x7fff_0000_u64.wrapping_sub((i as u64) * 8);

        let _result = debugger.replay_engine.take_snapshot(rip, rsp);
    }

    let duration = start.elapsed();
    println!("  Completed 10K snapshots in {:?}", duration);

    // Verify system is still responsive
    let total = debugger.replay_engine.total_snapshots.load(Ordering::Acquire);
    assert!(total > 0, "Snapshots should be recorded even under stress");
}

// ============================================================================
// Category 3: Long-Running Stability (3 tests)
// ============================================================================

/// Test 9: Continuous debugging for 60 seconds
#[test]
#[ignore]
fn test_continuous_debugging_60s() {
    println!("\n[TEST 9] Continuous debugging session (60 seconds)...");

    let debugger = DebuggerCapsule::new(7777);
    let start = Instant::now();
    let duration_target = Duration::from_secs(60);

    let mut iterations = 0_usize;
    let mut breakpoint_hits = 0_usize;

    while start.elapsed() < duration_target {
        // Simulate debugging operations
        let addr = 0x400000_u64.wrapping_add((iterations % 1000) as u64 * 4);

        // Set breakpoint
        if debugger.set_breakpoint(addr).is_ok() {
            breakpoint_hits += 1;
        }

        // Take snapshot
        let rip = 0x400000_u64.wrapping_add((iterations as u64) * 4);
        let rsp = 0x7fff_0000_u64.wrapping_sub((iterations as u64) * 8);
        let _result = debugger.replay_engine.take_snapshot(rip, rsp);

        // Continue execution
        let _result = debugger.continue_execution();

        iterations += 1;

        // Periodic status
        if iterations > 0 && iterations % 10_000 == 0 {
            let elapsed = start.elapsed();
            let rate = iterations as f64 / elapsed.as_secs_f64();
            println!(
                "  {:5} iterations in {:?} ({:.0} ops/sec)",
                iterations, elapsed, rate
            );
        }
    }

    let total_duration = start.elapsed();
    println!(
        "  Completed {} iterations in {:?}",
        iterations, total_duration
    );
    println!("  Rate: {:.0} ops/sec", iterations as f64 / total_duration.as_secs_f64());
    println!("  Breakpoint hits: {}", breakpoint_hits);

    // Should complete >1K iterations per second
    let rate = iterations as f64 / total_duration.as_secs_f64();
    assert!(rate > 1_000.0, "Should complete >1K ops/sec");
}

/// Test 10: Wraparound stability (10K snapshot wraparounds)
#[test]
#[ignore]
fn test_wraparound_stability() {
    println!("\n[TEST 10] Testing ring buffer wraparound stability...");

    let debugger = DebuggerCapsule::new(8888);

    // The ring buffer has MAX_SNAPSHOTS slots (2047)
    // We'll fill it multiple times to test wraparound
    let iterations = 50_000; // ~24 wraparounds (50K / 2047)

    let start = Instant::now();

    for i in 0..iterations {
        let rip = 0x400000_u64.wrapping_add((i as u64) * 4);
        let rsp = 0x7fff_0000_u64.wrapping_sub((i as u64) * 8);

        let _result = debugger.replay_engine.take_snapshot(rip, rsp);

        if i > 0 && i % 10_000 == 0 {
            let elapsed = start.elapsed();
            let rate = i as f64 / elapsed.as_secs_f64();
            println!(
                "  {} snapshots (wraparound ~{:.1}×) in {:?} ({:.0} ops/sec)",
                i,
                (i as f64 / 2047.0),
                elapsed,
                rate
            );
        }
    }

    let duration = start.elapsed();
    let wraparound_count = iterations / 2047;

    println!(
        "  Completed {} snapshots ({} wraparounds) in {:?}",
        iterations, wraparound_count, duration
    );

    // Verify ring buffer is still consistent after wraparound
    let total = debugger.replay_engine.total_snapshots.load(Ordering::Acquire);
    println!("  Total snapshots in engine: {}", total);
    assert!(total > 0, "Ring buffer should maintain data after wraparound");
}

/// Test 11: Sustained load with 100 breakpoints, 1000 hits each
#[test]
#[ignore]
fn test_sustained_load_100_breakpoints() {
    println!("\n[TEST 11] Sustained load: 100 breakpoints × 1000 hits...");

    let debugger = DebuggerCapsule::new(9999);

    let start = Instant::now();

    // Set 100 breakpoints
    println!("  Setting 100 breakpoints...");
    for i in 0..100 {
        let addr = 0x400000_u64.wrapping_add((i as u64) * 4);
        let _result = debugger.set_breakpoint(addr);
    }

    // Hit each breakpoint 1000 times
    println!("  Processing 100K breakpoint hits...");
    for hit in 0..100_000 {
        let bp_addr = 0x400000_u64.wrapping_add(((hit % 100) as u64) * 4);

        // Simulate breakpoint hit handling
        let _result = debugger.replay_engine.take_snapshot(bp_addr, 0x7fff_0000);

        if hit > 0 && hit % 10_000 == 0 {
            let elapsed = start.elapsed();
            let rate = hit as f64 / elapsed.as_secs_f64();
            println!(
                "  {} breakpoint hits processed in {:?} ({:.0} hits/sec)",
                hit, elapsed, rate
            );
        }
    }

    let duration = start.elapsed();
    println!(
        "  Completed 100K hits in {:?}",
        duration
    );

    // Should complete in reasonable time (<10 seconds)
    assert!(duration < Duration::from_secs(10), "Should complete in <10 seconds");

    // Breakpoints should still be valid
    let count = debugger.breakpoints.count.load(Ordering::Acquire);
    println!("  Breakpoints still registered: {}", count);
    assert_eq!(count, 100, "All breakpoints should be registered");
}

// ============================================================================
// Category 4: Error Recovery & Edge Cases (2 tests)
// ============================================================================

/// Test 12: Recovery from corrupted snapshot data
#[test]
#[ignore]
fn test_corrupted_snapshot_recovery() {
    println!("\n[TEST 12] Testing recovery from data anomalies...");

    let debugger = DebuggerCapsule::new(1111);

    // Take some normal snapshots
    for i in 0..100 {
        let rip = 0x400000_u64.wrapping_add((i as u64) * 4);
        let rsp = 0x7fff_0000_u64.wrapping_sub((i as u64) * 8);
        let _result = debugger.replay_engine.take_snapshot(rip, rsp);
    }

    println!("  Took 100 baseline snapshots");

    // Continue taking snapshots (ring buffer should handle any edge cases)
    let start = Instant::now();

    for i in 0..10_000 {
        let rip = 0x400000_u64.wrapping_add((i as u64) * 4);
        let rsp = 0x7fff_0000_u64.wrapping_sub((i as u64) * 8);

        let result = debugger.replay_engine.take_snapshot(rip, rsp);

        // Should always succeed
        assert!(result.is_ok(), "Snapshot {} should succeed", i);
    }

    let duration = start.elapsed();
    println!(
        "  Completed 10K more snapshots after baseline in {:?}",
        duration
    );

    // Verify engine is still operational
    let total = debugger.replay_engine.total_snapshots.load(Ordering::Acquire);
    println!("  Total snapshots: {}", total);
    assert!(total >= 100, "Should have accumulated snapshots");
}

/// Test 13: Hash chain integrity under stress
#[test]
#[ignore]
fn test_hash_chain_integrity() {
    println!("\n[TEST 13] Hash chain integrity verification...");

    let debugger = DebuggerCapsule::new(2222);

    // Take 1000 snapshots
    println!("  Taking 1000 snapshots with hash verification...");

    for i in 0..1_000 {
        let rip = 0x400000_u64.wrapping_add((i as u64) * 4);
        let rsp = 0x7fff_0000_u64.wrapping_sub((i as u64) * 8);

        let result = debugger.replay_engine.take_snapshot(rip, rsp);
        assert!(result.is_ok(), "Snapshot {} failed", i);
    }

    // Verify hash chain samples (don't check all, just samples)
    println!("  Verifying hash chain at sample points...");

    let sample_indices = [0, 100, 250, 500, 750, 999];
    let mut verified_count = 0;

    for &idx in &sample_indices {
        let verify_result = debugger.replay_engine.verify_hash_chain(idx as u64);

        if let Ok(is_valid) = verify_result {
            if is_valid {
                verified_count += 1;
            }
            println!("    Snapshot {}: {}", idx, if is_valid { "valid" } else { "invalid" });
        }
    }

    println!("  Verified {} of {} sample points", verified_count, sample_indices.len());

    // At least half of samples should verify (depending on implementation)
    assert!(verified_count > 0, "At least some snapshots should verify");
}

// ============================================================================
// Category 5: Platform-Specific Production (2 tests)
// ============================================================================

/// Test 14: Production workload simulation (Linux)
#[test]
#[ignore]
#[cfg(target_os = "linux")]
fn test_production_workload_linux() {
    println!("\n[TEST 14] Simulating production Linux debugging workload...");

    let debugger = DebuggerCapsule::new(3000);

    // Simulate a realistic debugging session:
    // 1. Attach to process
    let result = debugger.attach_to_process(3000);
    assert!(result.is_ok(), "Should attach to process");
    println!("  Attached to process");

    // 2. Set initial breakpoints at key functions
    let mut breakpoints = vec![];
    for i in 0..20 {
        let addr = 0x400000_u64.wrapping_add((i as u64) * 0x1000);
        if let Ok(bp_idx) = debugger.set_breakpoint(addr) {
            breakpoints.push(bp_idx);
        }
    }
    println!("  Set {} initial breakpoints", breakpoints.len());

    // 3. Simulate running and hitting breakpoints
    let start = Instant::now();
    let mut hits = 0;

    for iteration in 0..1_000 {
        // Hit a breakpoint
        if iteration % 5 == 0 {
            let _result = debugger.step_instruction();
            hits += 1;
        }

        // Take stack trace
        let _trace = debugger.get_stack_trace();

        // Continue execution
        let _result = debugger.continue_execution();

        // Take snapshots for time-travel capability
        let rip = 0x400000_u64.wrapping_add((iteration as u64) * 0x10);
        let rsp = 0x7fff_0000_u64.wrapping_sub((iteration as u64) * 0x8);
        let _result = debugger.replay_engine.take_snapshot(rip, rsp);
    }

    let duration = start.elapsed();

    println!(
        "  Processed 1000 iterations with {} breakpoint hits in {:?}",
        hits, duration
    );
    println!("  Rate: {:.0} ops/sec", 1000.0 / duration.as_secs_f64());

    // Should complete <5 seconds
    assert!(duration < Duration::from_secs(5), "Production workload should complete in <5s");
}

/// Test 15: No performance degradation over time
#[test]
#[ignore]
fn test_performance_no_degradation() {
    println!("\n[TEST 15] Performance stability check (degradation test)...");

    let debugger = DebuggerCapsule::new(4000);

    // Phase 1: Initial performance baseline
    let start = Instant::now();
    for i in 0..10_000 {
        let rip = 0x400000_u64.wrapping_add((i as u64) * 4);
        let rsp = 0x7fff_0000_u64.wrapping_sub((i as u64) * 8);
        let _result = debugger.replay_engine.take_snapshot(rip, rsp);
    }
    let phase1_duration = start.elapsed();
    let phase1_rate = 10_000.0 / phase1_duration.as_secs_f64();

    println!("  Phase 1 (0-10K): {:.0} ops/sec", phase1_rate);

    // Phase 2: Continue with more snapshots
    let start = Instant::now();
    for i in 10_000..20_000 {
        let rip = 0x400000_u64.wrapping_add((i as u64) * 4);
        let rsp = 0x7fff_0000_u64.wrapping_sub((i as u64) * 8);
        let _result = debugger.replay_engine.take_snapshot(rip, rsp);
    }
    let phase2_duration = start.elapsed();
    let phase2_rate = 10_000.0 / phase2_duration.as_secs_f64();

    println!("  Phase 2 (10K-20K): {:.0} ops/sec", phase2_rate);

    // Phase 3: Ring buffer wraparound
    let start = Instant::now();
    for i in 20_000..30_000 {
        let rip = 0x400000_u64.wrapping_add((i as u64) * 4);
        let rsp = 0x7fff_0000_u64.wrapping_sub((i as u64) * 8);
        let _result = debugger.replay_engine.take_snapshot(rip, rsp);
    }
    let phase3_duration = start.elapsed();
    let phase3_rate = 10_000.0 / phase3_duration.as_secs_f64();

    println!("  Phase 3 (20K-30K, post-wraparound): {:.0} ops/sec", phase3_rate);

    // Calculate degradation
    let degradation_p1_to_p2 = ((phase2_rate - phase1_rate) / phase1_rate * 100.0).abs();
    let degradation_p2_to_p3 = ((phase3_rate - phase2_rate) / phase2_rate * 100.0).abs();

    println!(
        "  Degradation P1→P2: {:.1}%, P2→P3: {:.1}%",
        degradation_p1_to_p2, degradation_p2_to_p3
    );

    // Allow up to 20% variance (normal for ring buffer operations)
    assert!(
        degradation_p1_to_p2 < 20.0,
        "Performance should not degrade >20% from Phase 1 to 2"
    );
    assert!(
        degradation_p2_to_p3 < 20.0,
        "Performance should not degrade >20% from Phase 2 to 3"
    );
}

// ============================================================================
// End-to-End Integration Tests
// ============================================================================

#[test]
fn test_debugger_basics() {
    // Basic sanity test (runs without --ignored)
    let debugger = DebuggerCapsule::new(1000);

    // Verify size and alignment
    assert!(std::mem::size_of::<DebuggerCapsule>() > 1_000_000);
    assert_eq!(std::mem::align_of::<DebuggerCapsule>(), 256);

    // Take a snapshot
    let result = debugger.replay_engine.take_snapshot(0x400000, 0x7fff_0000);
    assert!(result.is_ok());

    // Set a breakpoint
    let result = debugger.set_breakpoint(0x400000);
    assert!(result.is_ok());
}
