//! Integration Tests - Performance Under Load (3 tests)
//!
//! Tests for high-volume snapshot capture, many breakpoints, and sustained operation.
//! Framework: T28 Q15-Q21 (Integration testing tier)
//!
//! #ASSUME_MEMORY_BOUNDED: Memory usage stays within expected bounds
//! #ASSUME_NO_MEMORY_LEAK: Memory not leaked under sustained load
//! #ASSUME_LATENCY_PREDICTABLE: Latency remains consistent under load

#[cfg(test)]
mod tests {
    use kdb::DebuggerCapsule;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;
    use std::thread;
    use std::time::Instant;

    // ============================================================================
    // Helper: Estimate RSS (Resident Set Size) in bytes
    // ============================================================================

    #[cfg(target_os = "linux")]
    fn get_rss() -> usize {
        use std::fs;

        // Read /proc/self/status and extract VmRSS
        match fs::read_to_string("/proc/self/status") {
            Ok(content) => {
                for line in content.lines() {
                    if line.starts_with("VmRSS:") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            if let Ok(kb) = parts[1].parse::<usize>() {
                                return kb * 1024;  // Convert KB to bytes
                            }
                        }
                    }
                }
            }
            Err(_) => {}
        }

        0  // Fallback
    }

    #[cfg(not(target_os = "linux"))]
    fn get_rss() -> usize {
        // On non-Linux, return a dummy value
        0
    }

    // =========================================================================
    // Test 24: test_large_snapshot_volume_memory_bounded
    // =========================================================================
    // Validates: Memory usage stays bounded with many snapshots
    // Category: Performance Under Load
    // Framework: T28 Q15 (Memory pressure testing)
    #[test]
    fn test_large_snapshot_volume_memory_bounded() {
        let debugger = Arc::new(DebuggerCapsule::new(20000u64));

        // Ring buffer capacity is 3072, so overflow should be handled
        const OVERFLOW_SNAPSHOTS: usize = 10000;

        let start_mem = get_rss();

        // Capture many snapshots (will overflow ring buffer)
        {
            let d = Arc::clone(&debugger);
            thread::spawn(move || {
                for i in 0..OVERFLOW_SNAPSHOTS {
                    let rip = 0x400000 + (i as u64);
                    d.execution.set_rip(rip);
                    d.trace.record(1, 0, rip);

                    // Periodically check memory
                    if i % 1000 == 0 {
                        let current_mem = get_rss();
                        let growth = current_mem as i64 - start_mem as i64;
                        println!(
                            "After {} snapshots: {}MB (growth: {}MB)",
                            i,
                            current_mem / 1024 / 1024,
                            growth / 1024 / 1024
                        );
                    }
                }
            })
            .join()
            .expect("Thread panicked");
        }

        let end_mem = get_rss();
        let memory_growth_mb = (end_mem as i64 - start_mem as i64) / 1024 / 1024;

        println!(
            "Memory growth for {} snapshots: {} MB",
            OVERFLOW_SNAPSHOTS, memory_growth_mb
        );

        // Should not grow unbounded (bounded ring buffer)
        assert!(
            memory_growth_mb < 100,
            "Memory growth should be bounded, got {}MB",
            memory_growth_mb
        );

        // Validate snapshots captured
        let total = debugger.trace.total_events.load(Ordering::Acquire);
        assert_eq!(
            total, OVERFLOW_SNAPSHOTS as u64,
            "All snapshots should be counted"
        );
    }

    // =========================================================================
    // Test 25: test_many_concurrent_breakpoints
    // =========================================================================
    // Validates: Debugger handles 100+ concurrent breakpoints
    // Category: Performance Under Load
    // Framework: T28 Q16 (Breakpoint table stress)
    #[test]
    fn test_many_concurrent_breakpoints() {
        let debugger = Arc::new(DebuggerCapsule::new(20001u64));
        let mut handles = vec![];
        const NUM_THREADS: usize = 4;
        const BREAKPOINTS_PER_THREAD: usize = 64;
        const HITS_PER_BP: usize = 100;

        let start = Instant::now();

        // Spawn threads setting many breakpoints and simulating hits
        for thread_id in 0..NUM_THREADS {
            let d = Arc::clone(&debugger);
            handles.push(thread::spawn(move || {
                for bp_idx in 0..BREAKPOINTS_PER_THREAD {
                    let entry_idx = (thread_id * BREAKPOINTS_PER_THREAD + bp_idx) % 256;
                    let addr = 0x400000 + (entry_idx as u64 * 0x1000);

                    // Set breakpoint
                    d.breakpoints.entries[entry_idx]
                        .address
                        .store(addr, Ordering::Release);
                    d.breakpoints.entries[entry_idx]
                        .enabled
                        .store(1, Ordering::Release);

                    // Simulate many hits
                    for _ in 0..HITS_PER_BP {
                        d.breakpoints.entries[entry_idx]
                            .hit_count
                            .fetch_add(1, Ordering::AcqRel);
                        d.execution.breakpoint_hits.fetch_add(1, Ordering::Relaxed);
                        d.trace.record(2, thread_id as u32, addr);
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        let elapsed = start.elapsed();

        // Validate breakpoint activity
        let total_bp_hits = debugger.execution.breakpoint_hits.load(Ordering::Acquire);
        let expected_hits = (NUM_THREADS * BREAKPOINTS_PER_THREAD * HITS_PER_BP) as u64;

        assert_eq!(
            total_bp_hits, expected_hits,
            "All {} breakpoint hits should be recorded, got {}",
            expected_hits, total_bp_hits
        );

        println!(
            "Processed {} breakpoint hits in {:?}",
            total_bp_hits, elapsed
        );

        // Performance: should handle in < 500ms
        assert!(
            elapsed.as_millis() < 500,
            "Should handle 25,600 breakpoint hits in < 500ms, took {:?}",
            elapsed
        );
    }

    // =========================================================================
    // Test 26: test_long_running_continuous_operation
    // =========================================================================
    // Validates: Debugger maintains correctness under sustained operation
    // Category: Performance Under Load
    // Framework: T28 Q17 (Sustained load testing)
    //
    // This test runs for 10 seconds of continuous operation
    // Use --ignored flag to skip in CI if needed
    #[test]
    #[ignore]  // Optional: takes 10+ seconds
    fn test_long_running_continuous_operation() {
        let debugger = Arc::new(DebuggerCapsule::new(20002u64));
        let mut handles = vec![];
        const NUM_THREADS: usize = 4;
        const DURATION_SECS: u64 = 10;

        let start_global = Instant::now();

        // Spawn threads running continuous operations
        for thread_id in 0..NUM_THREADS {
            let d = Arc::clone(&debugger);
            handles.push(thread::spawn(move || {
                let start = Instant::now();

                let mut iteration = 0u64;
                while start.elapsed().as_secs() < DURATION_SECS {
                    iteration += 1;

                    // Continuous operations
                    let rip = 0x400000 + iteration;
                    d.execution.set_rip(rip);
                    d.execution.instruction_count.fetch_add(1, Ordering::Relaxed);

                    // Record trace events
                    d.trace.record(3, thread_id as u32, rip);

                    // Set/unset breakpoints cyclically
                    let bp_idx = (iteration as usize) % 256;
                    if iteration % 2 == 0 {
                        d.breakpoints.entries[bp_idx]
                            .address
                            .store(rip, Ordering::Release);
                    } else {
                        d.breakpoints.entries[bp_idx]
                            .address
                            .store(0, Ordering::Release);
                    }

                    // Every 100K iterations, check time
                    if iteration % 100_000 == 0 {
                        println!(
                            "Thread {}: {} iterations in {:?}",
                            thread_id,
                            iteration,
                            start.elapsed()
                        );
                    }
                }

                println!(
                    "Thread {} completed {} iterations",
                    thread_id, iteration
                );
                iteration
            }));
        }

        // Wait for all threads
        let mut total_iterations = 0u64;
        for handle in handles {
            total_iterations += handle.join().expect("Thread panicked");
        }

        let total_elapsed = start_global.elapsed();

        println!(
            "Completed {} total iterations in {:?}",
            total_iterations, total_elapsed
        );
        println!(
            "Throughput: {:.0} ops/sec",
            total_iterations as f64 / total_elapsed.as_secs_f64()
        );

        // Validate final state
        let final_instruction_count = debugger.execution.instruction_count.load(Ordering::Acquire);
        assert!(final_instruction_count > 0, "Operations should have been recorded");
    }
}
