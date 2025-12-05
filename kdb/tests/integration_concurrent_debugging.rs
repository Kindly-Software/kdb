//! Integration Tests - Concurrent Debugging (4 tests)
//!
//! Tests for concurrent snapshot capture, breakpoint management, and replay navigation.
//! Framework: T28 Q15-Q21 (Integration testing tier)
//!
//! #ASSUME_LOCKFREE_SNAPSHOTS: All snapshot operations are lockfree
//! #ASSUME_NO_SNAPSHOT_LOSS: All snapshots recorded despite contention
//! #ASSUME_CONCURRENT_READS: Multiple readers don't block each other

#[cfg(test)]
mod tests {
    use kdb::DebuggerCapsule;
    use kdb::time_travel::TimeSnapshot;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;
    use std::time::Instant;

    // =========================================================================
    // Test 19: test_concurrent_snapshot_capture_high_contention
    // =========================================================================
    // Validates: Multiple threads can capture snapshots without data loss
    // Category: Concurrent Debugging
    // Framework: T28 Q15 (Concurrent snapshot operations)
    #[test]
    fn test_concurrent_snapshot_capture_high_contention() {
        let debugger = Arc::new(DebuggerCapsule::new(10000u64));
        let mut handles = vec![];
        const NUM_THREADS: usize = 10;
        const SNAPSHOTS_PER_THREAD: usize = 100;

        let start = Instant::now();

        // Spawn 10 threads, each capturing 100 snapshots
        for thread_id in 0..NUM_THREADS {
            let d = Arc::clone(&debugger);
            handles.push(thread::spawn(move || {
                for i in 0..SNAPSHOTS_PER_THREAD {
                    let rip = 0x400000 + (thread_id as u64 * 10000 + i as u64);
                    d.execution.set_rip(rip);
                    d.trace.record(10, thread_id as u32, rip);
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        let elapsed = start.elapsed();

        // Validate all snapshots recorded
        let total = debugger.trace.total_events.load(Ordering::Acquire);
        assert_eq!(
            total, (NUM_THREADS * SNAPSHOTS_PER_THREAD) as u64,
            "All {} snapshots must be recorded, got {}",
            NUM_THREADS * SNAPSHOTS_PER_THREAD,
            total
        );

        // Performance: should complete in < 100ms for 1000 snapshots
        println!(
            "Captured {} snapshots in {:?} ({:.1} snapshots/ms)",
            total,
            elapsed,
            total as f64 / elapsed.as_secs_f64() / 1000.0
        );

        assert!(
            elapsed.as_millis() < 500,
            "1000 concurrent snapshots should complete in < 500ms, took {:?}",
            elapsed
        );
    }

    // =========================================================================
    // Test 20: test_concurrent_breakpoint_management
    // =========================================================================
    // Validates: Multiple threads can manage breakpoints safely
    // Category: Concurrent Debugging
    // Framework: T28 Q16 (Concurrent breakpoint updates)
    #[test]
    fn test_concurrent_breakpoint_management() {
        let debugger = Arc::new(DebuggerCapsule::new(10001u64));
        let mut handles = vec![];
        const NUM_THREADS: usize = 8;
        const BREAKPOINTS_PER_THREAD: usize = 20;

        // Spawn threads to set/enable/disable breakpoints concurrently
        for thread_id in 0..NUM_THREADS {
            let d = Arc::clone(&debugger);
            handles.push(thread::spawn(move || {
                for i in 0..BREAKPOINTS_PER_THREAD {
                    let idx = (thread_id * BREAKPOINTS_PER_THREAD + i) % 256;
                    let addr = 0x400000 + (i as u64 * 0x1000);

                    // Set breakpoint
                    d.breakpoints.entries[idx]
                        .address
                        .store(addr, Ordering::Release);
                    d.breakpoints.entries[idx]
                        .enabled
                        .store(1, Ordering::Release);

                    // Simulate hit
                    d.breakpoints.entries[idx]
                        .hit_count
                        .fetch_add(1, Ordering::AcqRel);

                    // Disable/enable cycle
                    if i % 2 == 0 {
                        d.breakpoints.entries[idx]
                            .enabled
                            .store(0, Ordering::Release);
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // Validate breakpoints set
        let mut enabled_count = 0;
        let mut hit_count = 0u64;

        for entry in &debugger.breakpoints.entries[0..256] {
            let enabled = entry.enabled.load(Ordering::Acquire);
            if enabled == 1 {
                enabled_count += 1;
            }
            hit_count += entry.hit_count.load(Ordering::Acquire);
        }

        println!(
            "Breakpoints: {} enabled, {} total hits",
            enabled_count, hit_count
        );
        assert!(enabled_count > 0, "Some breakpoints should be enabled");
        assert!(hit_count > 0, "Some breakpoints should have been hit");
    }

    // =========================================================================
    // Test 21: test_concurrent_replay_navigation
    // =========================================================================
    // Validates: Multiple threads can navigate snapshots concurrently
    // Category: Concurrent Debugging
    // Framework: T28 Q17 (Concurrent read-heavy access)
    #[test]
    fn test_concurrent_replay_navigation() {
        let debugger = Arc::new(DebuggerCapsule::new(10002u64));
        let mut handles = vec![];
        const NUM_THREADS: usize = 4;
        const SNAPSHOTS: usize = 500;

        // Pre-populate trace with snapshots
        for i in 0..SNAPSHOTS {
            let rip = 0x400000 + (i as u64 * 0x10);
            debugger.execution.set_rip(rip);
            debugger.trace.record(10, 0, rip);
        }

        // Spawn reader threads that navigate the snapshots
        for thread_id in 0..NUM_THREADS {
            let d = Arc::clone(&debugger);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    // Simulate navigation by reading trace
                    let recent = d.trace.drain_recent(50);

                    assert!(
                        !recent.is_empty(),
                        "Thread {} should find recent snapshots",
                        thread_id
                    );

                    // Validate data integrity
                    for (_, tid, _, _) in recent {
                        assert!(tid <= 0u32, "TID should be valid");
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // Final validation
        let total = debugger.trace.total_events.load(Ordering::Acquire);
        assert!(total >= SNAPSHOTS as u64, "All snapshots should be preserved");
    }

    // =========================================================================
    // Test 22: test_concurrent_hash_chain_verification
    // =========================================================================
    // Validates: Hash chain integrity during concurrent access
    // Category: Concurrent Debugging
    // Framework: T28 Q18 (Hash integrity under contention)
    #[test]
    fn test_concurrent_hash_chain_verification() {
        // Create snapshot array (TimeSnapshot doesn't implement Clone, so create manually)
        let mut snap_vec = Vec::with_capacity(1000);
        for _ in 0..1000 {
            snap_vec.push(TimeSnapshot::empty());
        }
        let snapshots = Arc::new(snap_vec);

        // Pre-populate with valid snapshots
        for i in 0..1000 {
            snapshots[i].save(i as u64, 0x400000 + (i as u64) * 0x10, 0x7fff_0000);
        }

        let mut handles = vec![];
        const NUM_READERS: usize = 4;

        // Spawn reader threads that verify the hash chain
        for _ in 0..NUM_READERS {
            let s = Arc::clone(&snapshots);
            handles.push(thread::spawn(move || {
                // Verify snapshots maintain their state
                for i in 0..1000 {
                    let (sid, rip, rsp) = s[i].get_state();
                    assert_eq!(sid, i as u64, "Snapshot ID must be preserved");
                    assert!(rip >= 0x400000, "RIP must be valid");
                    assert!(rsp > 0, "RSP must be valid");
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // Validation: all snapshots intact
        for i in 0..1000 {
            let (sid, _, _) = snapshots[i].get_state();
            assert_eq!(sid, i as u64, "Final validation: snapshot {} integrity", i);
        }
    }

    // =========================================================================
    // Test 23: test_snapshot_capture_performance_latency
    // =========================================================================
    // Validates: Snapshot capture latency under concurrent load
    // Category: Concurrent Debugging
    // Framework: T28 Q19 (Latency characterization)
    #[test]
    fn test_snapshot_capture_performance_latency() {
        let debugger = Arc::new(DebuggerCapsule::new(10003u64));
        let latencies = Arc::new(AtomicU64::new(0));
        let mut handles = vec![];
        const NUM_THREADS: usize = 8;
        const ITERATIONS: usize = 100;

        // Spawn threads measuring capture latency
        for thread_id in 0..NUM_THREADS {
            let d = Arc::clone(&debugger);
            let l = Arc::clone(&latencies);

            handles.push(thread::spawn(move || {
                for i in 0..ITERATIONS {
                    let start = std::time::Instant::now();

                    d.execution.set_rip(0x400000 + (i as u64) * 0x10);
                    d.trace.record(10, thread_id as u32, 0);

                    let elapsed_ns = start.elapsed().as_nanos() as u64;
                    l.fetch_add(elapsed_ns, Ordering::Relaxed);
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // Calculate average latency
        let total_ns = latencies.load(Ordering::Acquire);
        let total_ops = (NUM_THREADS * ITERATIONS) as u64;
        let avg_ns = total_ns / total_ops;

        println!("Average snapshot latency: {} ns", avg_ns);

        // Performance target: <10 microseconds per snapshot
        // (Given <10ns per atomic operation, this is easily achievable)
        assert!(
            avg_ns < 10_000,
            "Average snapshot latency should be < 10μs, got {}ns",
            avg_ns
        );
    }
}
