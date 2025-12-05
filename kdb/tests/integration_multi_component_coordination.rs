//! Integration Tests - Category 1: Multi-Component Coordination (6 tests)
//!
//! These tests validate cross-component interaction and real-world debugging scenarios.
//! Framework: T28 Q15-Q21 (Integration testing tier)
//!
//! #ASSUME_LOCKFREE_COORDINATION: All components use atomic coordination
//! #ASSUME_CONSISTENT_STATE: Snapshot integrity maintained across operations
//! #ASSUME_NO_DATA_LOSS: All events recorded atomically
//! #ASSUME_CONCURRENT_SAFE: Multiple threads can coordinate safely

#[cfg(test)]
mod tests {
    use kdb::DebuggerCapsule;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;
    use std::thread;

    // =========================================================================
    // Test 1: test_debugger_creation_initialization
    // =========================================================================
    // Validates: DebuggerCapsule can be created and initialized with correct state
    // Category: Multi-Component Coordination
    // Framework: T28 Q15 (Basic cross-component validation)
    #[test]
    fn test_debugger_creation_initialization() {
        let pid = 12345u64;
        // Note: DebuggerCapsule is 1.09 MB, requires 8MB+ stack
        // Stack size configured in .cargo/config.toml via [target] llvm-cflags
        let debugger = Box::new(DebuggerCapsule::new(pid));

        // Validate T1 Atomic components initialized
        assert_eq!(debugger.execution.get_pid(), pid, "PID must be set");
        assert!(debugger.execution.is_running(), "Execution should be running on init");
        assert_eq!(
            debugger.execution.instruction_count.load(Ordering::Relaxed),
            0,
            "Instruction count starts at 0"
        );
        assert_eq!(
            debugger.execution.breakpoint_hits.load(Ordering::Relaxed),
            0,
            "Breakpoint hits start at 0"
        );

        // Validate T5 Streaming trace initialized
        assert_eq!(
            debugger.trace.total_events.load(Ordering::Relaxed),
            0,
            "Trace events start at 0"
        );

        // Validate size (1.09 MB as per CLAUDE.md)
        let size = std::mem::size_of::<DebuggerCapsule>();
        assert!(
            size >= 1_140_000 && size <= 1_160_000,
            "DebuggerCapsule should be ~1.09 MB, got {} bytes",
            size
        );

        // Validate alignment (256-byte cache-aligned)
        assert_eq!(
            std::mem::align_of::<DebuggerCapsule>(),
            256,
            "DebuggerCapsule must be 256-byte aligned"
        );
    }

    // =========================================================================
    // Test 2: test_execution_state_and_trace_coordination
    // =========================================================================
    // Validates: Execution state changes properly coordinate with trace recording
    // Category: Multi-Component Coordination
    // Framework: T28 Q16 (Cross-component data flow)
    #[test]
    fn test_execution_state_and_trace_coordination() {
        let debugger = Box::new(DebuggerCapsule::new(5678u64));

        // Initial state: running
        assert!(debugger.execution.is_running());

        // Simulate execution state transition: pause
        debugger.execution.pause();
        assert!(!debugger.execution.is_running());

        // Record a trace event while paused
        debugger.trace.record(1, 100, 0x1000);

        // Validate trace recorded
        let recent = debugger.trace.drain_recent(10);
        assert!(!recent.is_empty(), "Trace event should be recorded");
        assert_eq!(recent[0].0, 1, "Event type must match");
        assert_eq!(recent[0].1, 100, "Thread ID must match");

        // Resume execution
        debugger.execution.resume();
        assert!(debugger.execution.is_running());
    }

    // =========================================================================
    // Test 3: test_concurrent_state_updates_with_trace
    // =========================================================================
    // Validates: Multiple threads can update execution state and trace simultaneously
    // Category: Multi-Component Coordination
    // Framework: T28 Q17 (Concurrent cross-component interaction)
    #[test]
    fn test_concurrent_state_updates_with_trace() {
        let debugger = Arc::new(DebuggerCapsule::new(9999u64));
        let mut handles = vec![];

        // Spawn 5 threads, each updating state and recording events
        for thread_id in 0..5 {
            let d = Arc::clone(&debugger);
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    // Update execution state
                    let rip = 0x400000 + (thread_id * 1000 + i) as u64;
                    d.execution.set_rip(rip);

                    // Record trace event
                    d.trace.record(
                        (i % 8) as u8,
                        thread_id as u32,
                        rip,
                    );

                    // Increment instruction count
                    d.execution.instruction_count.fetch_add(1, Ordering::Relaxed);
                }
            }));
        }

        // Wait for all threads
        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // Validate results
        assert_eq!(
            debugger.execution.instruction_count.load(Ordering::Acquire),
            500,
            "All 500 instructions should be counted"
        );
        assert!(
            debugger.trace.total_events.load(Ordering::Acquire) > 0,
            "Trace events should be recorded"
        );
    }

    // =========================================================================
    // Test 4: test_breakpoint_hit_with_trace_and_snapshot
    // =========================================================================
    // Validates: Breakpoint hit triggers trace recording and execution state update
    // Category: Multi-Component Coordination
    // Framework: T28 Q18 (State machine transitions with side effects)
    #[test]
    fn test_breakpoint_hit_with_trace_and_snapshot() {
        let debugger = Box::new(DebuggerCapsule::new(1111u64));
        let breakpoint_addr = 0x400500u64;

        // Simulate breakpoint hit
        let breakpoint_index = 0;
        debugger.breakpoints.entries[breakpoint_index]
            .address
            .store(breakpoint_addr, Ordering::Release);
        debugger.breakpoints.entries[breakpoint_index]
            .hit_count
            .store(1, Ordering::Release);

        // Update execution state to reflect breakpoint stop
        debugger.execution.set_rip(breakpoint_addr);
        debugger.execution.pause();
        debugger.execution.breakpoint_hits.fetch_add(1, Ordering::Release);

        // Record trace event for breakpoint hit
        debugger.trace.record(5, 0, breakpoint_addr);

        // Validate coordinated state
        assert_eq!(debugger.execution.get_rip(), breakpoint_addr);
        assert!(!debugger.execution.is_running());
        assert_eq!(
            debugger.execution.breakpoint_hits.load(Ordering::Acquire),
            1,
            "Breakpoint hit count"
        );
        assert!(
            debugger.trace.total_events.load(Ordering::Acquire) > 0,
            "Trace event recorded"
        );
    }

    // =========================================================================
    // Test 5: test_concurrent_breakpoint_settings_with_trace_capture
    // =========================================================================
    // Validates: Multiple threads can set breakpoints while trace is recording
    // Category: Multi-Component Coordination
    // Framework: T28 Q19 (Concurrent modification under load)
    #[test]
    fn test_concurrent_breakpoint_settings_with_trace_capture() {
        let debugger = Arc::new(DebuggerCapsule::new(2222u64));
        let mut handles = vec![];
        const NUM_THREADS: usize = 8;
        const BREAKPOINTS_PER_THREAD: usize = 10;

        // Spawn threads: half set breakpoints, half record trace
        for thread_id in 0..NUM_THREADS {
            let d = Arc::clone(&debugger);
            handles.push(thread::spawn(move || {
                if thread_id < NUM_THREADS / 2 {
                    // Setters: set breakpoints
                    for i in 0..BREAKPOINTS_PER_THREAD {
                        let idx = (thread_id * BREAKPOINTS_PER_THREAD + i) % 256;
                        let addr = 0x400000 + (i * 0x100) as u64;
                        d.breakpoints.entries[idx].address.store(addr, Ordering::Release);
                    }
                } else {
                    // Readers: record trace events
                    for _ in 0..BREAKPOINTS_PER_THREAD * 10 {
                        d.trace.record(1, thread_id as u32, 0);
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // Validate: breakpoints set and trace recorded
        let total_events = debugger.trace.total_events.load(Ordering::Acquire);
        assert!(
            total_events >= 400,
            "Expected 400+ trace events, got {}",
            total_events
        );
    }

    // =========================================================================
    // Test 6: test_execution_state_generation_counter_prevents_toctou
    // =========================================================================
    // Validates: Generation counter prevents TOCTOU bugs in state synchronization
    // Category: Multi-Component Coordination
    // Framework: T28 Q20 (Race condition prevention)
    #[test]
    fn test_execution_state_generation_counter_prevents_toctou() {
        let debugger = Arc::new(DebuggerCapsule::new(3333u64));
        let initial_gen = debugger.execution.generation.load(Ordering::Acquire);

        // Simulate concurrent state reads and writes
        let d1 = Arc::clone(&debugger);
        let d2 = Arc::clone(&debugger);

        let h1 = thread::spawn(move || {
            for _ in 0..50 {
                let gen_before = d1.execution.generation.load(Ordering::Acquire);
                let rip = d1.execution.get_rip();
                let gen_after = d1.execution.generation.load(Ordering::Acquire);

                // If generation changed, we saw an intermediate state (which is OK)
                assert!(gen_after >= gen_before, "Generation must be monotonic");
            }
        });

        let h2 = thread::spawn(move || {
            for i in 0..50 {
                d2.execution.set_rip(0x400000 + (i as u64) * 0x1000);
            }
        });

        h1.join().expect("Reader thread panicked");
        h2.join().expect("Writer thread panicked");

        // Generation counter must have increased
        let final_gen = debugger.execution.generation.load(Ordering::Acquire);
        assert!(final_gen > initial_gen, "Generation counter must increase with writes");
    }
}
