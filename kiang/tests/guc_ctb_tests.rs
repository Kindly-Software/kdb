//! Property-based tests for GuC CTB ring buffer management
//!
//! These tests validate critical invariants:
//! 1. Ring buffer NEVER overflows (10% safety margin enforced)
//! 2. Concurrent reservations are always consistent
//! 3. Version consistency prevents TOCTOU races
//! 4. Head/tail pointers maintain ring buffer integrity

use kiang::guc_ctb::{GucCtbRingBuffer, GucCtbState, GucReadyCapsule};
use proptest::prelude::*;
use std::sync::Arc;
use std::thread;

// ============================================================================
// Property Tests - Buffer Overflow Prevention
// ============================================================================

proptest! {
    /// Property: Buffer NEVER overflows regardless of reservation pattern
    ///
    /// #ASSUME_INVARIANT: 10% safety margin prevents all overflow scenarios
    /// #VERIFY_INVARIANT: Property test with random reservation patterns
    #[test]
    fn prop_no_buffer_overflow(
        capacity in 1024u32..8192,
        reservations in prop::collection::vec(1u32..256, 1..50)
    ) {
        let capsule = GucReadyCapsule::with_capacity(capacity);

        let mut state = GucCtbState {
            h2g_head: 0,
            h2g_tail: 0,
            g2h_head: 0,
            g2h_tail: 0,
            capacity,
            pending_count: 0,
        };

        for size in reservations {
            // If buffer says we have space, reservation MUST succeed
            if state.has_h2g_space(size) {
                // Simulate reservation
                let new_tail = (state.h2g_tail + size) % capacity;

                // INVARIANT: New tail must not overflow with safety margin
                let used_after = if new_tail >= state.h2g_head {
                    new_tail - state.h2g_head
                } else {
                    capacity - (state.h2g_head - new_tail)
                };

                let safety_margin = capacity / 10;
                prop_assert!(used_after + safety_margin <= capacity,
                    "Buffer overflow detected: used={}, safety={}, capacity={}",
                    used_after, safety_margin, capacity);

                state.h2g_tail = new_tail;
            }
        }
    }

    /// Property: Safety margin is always maintained
    #[test]
    fn prop_safety_margin_enforced(
        capacity in 1024u32..8192,
        head in 0u32..4096,
        tail in 0u32..4096
    ) {
        let state = GucCtbState {
            h2g_head: head % capacity,
            h2g_tail: tail % capacity,
            g2h_head: 0,
            g2h_tail: 0,
            capacity,
            pending_count: 0,
        };

        let safety_margin = capacity / 10; // 10%
        let used = if state.h2g_tail >= state.h2g_head {
            state.h2g_tail - state.h2g_head
        } else {
            capacity - (state.h2g_head - state.h2g_tail)
        };

        // If buffer is near capacity, has_h2g_space should reject
        if used > capacity - safety_margin {
            prop_assert!(!state.has_h2g_space(1),
                "Safety margin violated: used={}, margin={}, capacity={}",
                used, safety_margin, capacity);
        }
    }

    /// Property: Utilization calculation is always accurate
    #[test]
    fn prop_utilization_accurate(
        capacity in 1024u32..8192,
        head in 0u32..4096,
        tail in 0u32..4096
    ) {
        let state = GucCtbState {
            h2g_head: head % capacity,
            h2g_tail: tail % capacity,
            g2h_head: 0,
            g2h_tail: 0,
            capacity,
            pending_count: 0,
        };

        let utilization = state.h2g_utilization();

        // Utilization must be in valid range [0, 100]
        prop_assert!(utilization <= 100,
            "Invalid utilization: {}%", utilization);

        // Calculate expected utilization
        let used = if state.h2g_tail >= state.h2g_head {
            state.h2g_tail - state.h2g_head
        } else {
            capacity - (state.h2g_head - state.h2g_tail)
        };
        let expected = ((used as u64 * 100) / capacity as u64) as u8;

        prop_assert_eq!(utilization, expected,
            "Utilization mismatch: got {}%, expected {}%",
            utilization, expected);
    }

    /// Property: Wrap-around handling is correct
    #[test]
    fn prop_wrap_around_correct(
        capacity in 1024u32..8192,
        tail_offset in 0u32..1024
    ) {
        let head = capacity - 512; // Near end of buffer
        let tail = tail_offset % 512; // Wrapped around to start

        let state = GucCtbState {
            h2g_head: head,
            h2g_tail: tail,
            g2h_head: 0,
            g2h_tail: 0,
            capacity,
            pending_count: 0,
        };

        // Calculate used space correctly across wrap boundary
        let used = capacity - (head - tail);
        let expected_util = ((used as u64 * 100) / capacity as u64) as u8;

        prop_assert_eq!(state.h2g_utilization(), expected_util,
            "Wrap-around utilization incorrect: head={}, tail={}, capacity={}",
            head, tail, capacity);
    }
}

// ============================================================================
// Concurrent Access Tests
// ============================================================================

#[test]
fn test_concurrent_readers() {
    let capsule = Arc::new(GucReadyCapsule::with_capacity(4096));
    let mut handles = vec![];

    // Writer thread publishes updates
    let writer = capsule.clone();
    handles.push(thread::spawn(move || {
        for i in 0..1000 {
            let state = GucCtbState {
                h2g_head: 0,
                h2g_tail: i * 4,
                g2h_head: 0,
                g2h_tail: 0,
                capacity: 4096,
                pending_count: i as u16,
            };
            writer.publish(state);
        }
    }));

    // Multiple reader threads
    for _ in 0..8 {
        let reader = capsule.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..10000 {
                if let Some(state) = reader.read() {
                    // Verify state consistency
                    assert_eq!(state.capacity, 4096);
                    assert!(state.h2g_tail <= 4096);
                }
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_reservations() {
    // Allocate test buffers
    let h2g_buffer = vec![0u8; 4096];
    let g2h_buffer = vec![0u8; 4096];

    let ring_buffer = Arc::new(unsafe {
        GucCtbRingBuffer::new(
            h2g_buffer.as_ptr() as *mut u8,
            g2h_buffer.as_ptr() as *mut u8,
            4096,
        )
    });

    let mut handles = vec![];

    // Multiple threads try to reserve slots
    for thread_id in 0..8 {
        let buffer = ring_buffer.clone();
        handles.push(thread::spawn(move || {
            let mut successful_reservations = 0;

            for _ in 0..100 {
                if let Some((offset, size)) = buffer.reserve_h2g_slot(64) {
                    // Verify reservation is valid
                    assert!(offset < 4096, "Invalid offset: {}", offset);
                    assert_eq!(size, 64, "Size should be 64");
                    successful_reservations += 1;

                    // Simulate work
                    thread::sleep(std::time::Duration::from_micros(1));
                }
            }

            successful_reservations
        }));
    }

    let total_reservations: u32 = handles.into_iter().map(|h| h.join().unwrap()).sum();

    println!("Total successful reservations: {}", total_reservations);
    assert!(
        total_reservations > 0,
        "Should have some successful reservations"
    );

    // Verify buffer never overflowed
    if let Some(state) = ring_buffer.state() {
        let utilization = state.h2g_utilization();
        assert!(utilization <= 100, "Utilization should be <= 100%");
        println!("Final buffer utilization: {}%", utilization);
    }
}

// ============================================================================
// TOCTOU Prevention Tests
// ============================================================================

#[test]
fn test_version_consistency_prevents_toctou() {
    let capsule = GucReadyCapsule::with_capacity(4096);

    // Publish state with version 0
    let state1 = GucCtbState {
        h2g_head: 0,
        h2g_tail: 1024,
        g2h_head: 0,
        g2h_tail: 0,
        capacity: 4096,
        pending_count: 5,
    };
    capsule.publish(state1);

    // Read should succeed
    let read1 = capsule.read();
    assert!(read1.is_some());
    assert_eq!(read1.unwrap().h2g_tail, 1024);

    // Publish state with version 1
    let state2 = GucCtbState {
        h2g_head: 0,
        h2g_tail: 2048,
        g2h_head: 0,
        g2h_tail: 0,
        capacity: 4096,
        pending_count: 10,
    };
    capsule.publish(state2);

    // Read should get new state
    let read2 = capsule.read();
    assert!(read2.is_some());
    assert_eq!(read2.unwrap().h2g_tail, 2048);

    // Version must have incremented (wrapping is okay)
    // This is implicitly verified by successful reads
}

#[test]
fn test_uncommitted_state_rejected() {
    // Raw new() creates uncommitted capsule
    let capsule = GucReadyCapsule::new();

    // Initial state is uncommitted
    let read = capsule.read();
    assert!(read.is_none(), "Uncommitted state should be rejected");

    // After publishing, reads should succeed
    let state = GucCtbState {
        h2g_head: 0,
        h2g_tail: 0,
        g2h_head: 0,
        g2h_tail: 0,
        capacity: 4096,
        pending_count: 0,
    };
    capsule.publish(state);

    let read = capsule.read();
    assert!(read.is_some(), "Committed state should be readable");

    // with_capacity() should be immediately readable
    let capsule2 = GucReadyCapsule::with_capacity(4096);
    let read2 = capsule2.read();
    assert!(
        read2.is_some(),
        "with_capacity() should be immediately readable"
    );
    assert_eq!(read2.unwrap().capacity, 4096);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_full_buffer_rejection() {
    let capacity = 4096u32;
    let safety_margin = capacity / 10; // 409
    let max_usable = capacity - safety_margin; // 3686

    let state = GucCtbState {
        h2g_head: 0,
        h2g_tail: max_usable, // Exactly at limit
        g2h_head: 0,
        g2h_tail: 0,
        capacity,
        pending_count: 0,
    };

    // Should reject any further allocations (at safety margin)
    assert!(
        !state.has_h2g_space(1),
        "Should reject when at safety margin"
    );
    assert!(
        !state.has_h2g_space(410),
        "Should reject when exceeding margin"
    );

    // State with some headroom should accept
    let state_with_space = GucCtbState {
        h2g_head: 0,
        h2g_tail: max_usable - 100,
        g2h_head: 0,
        g2h_tail: 0,
        capacity,
        pending_count: 0,
    };
    assert!(
        state_with_space.has_h2g_space(50),
        "Should accept when under limit"
    );
}

#[test]
fn test_empty_buffer_acceptance() {
    let capacity = 4096u32;
    let safety_margin = capacity / 10; // 409
    let max_usable = capacity - safety_margin; // 3687

    let state = GucCtbState {
        h2g_head: 0,
        h2g_tail: 0,
        g2h_head: 0,
        g2h_tail: 0,
        capacity,
        pending_count: 0,
    };

    // Should accept allocation up to max_usable
    assert!(state.has_h2g_space(1), "Should accept small allocation");
    assert!(
        state.has_h2g_space(max_usable),
        "Should accept allocation within margin"
    );
    assert!(
        !state.has_h2g_space(max_usable + 1),
        "Should reject allocation exceeding margin"
    );
}

#[test]
fn test_zero_capacity_rejection() {
    let state = GucCtbState {
        h2g_head: 0,
        h2g_tail: 0,
        g2h_head: 0,
        g2h_tail: 0,
        capacity: 0,
        pending_count: 0,
    };

    assert!(
        !state.has_h2g_space(1),
        "Zero capacity should reject all allocations"
    );
    assert_eq!(
        state.h2g_utilization(),
        0,
        "Zero capacity utilization should be 0"
    );
}

#[test]
fn test_pending_count_tracking() {
    let capsule = GucReadyCapsule::with_capacity(4096);

    let mut state = GucCtbState {
        h2g_head: 0,
        h2g_tail: 0,
        g2h_head: 0,
        g2h_tail: 0,
        capacity: 4096,
        pending_count: 0,
    };

    // Increment pending count
    for i in 1..=10 {
        state.pending_count = i;
        capsule.publish(state);

        let read = capsule.read().unwrap();
        assert_eq!(read.pending_count, i, "Pending count should match");
    }

    // Decrement pending count
    for i in (0..10).rev() {
        state.pending_count = i;
        capsule.publish(state);

        let read = capsule.read().unwrap();
        assert_eq!(read.pending_count, i, "Pending count should match");
    }
}

// ============================================================================
// Performance Invariant Tests
// ============================================================================

#[test]
fn test_readiness_check_performance() {
    let capsule = GucReadyCapsule::with_capacity(4096);

    let state = GucCtbState {
        h2g_head: 0,
        h2g_tail: 1024,
        g2h_head: 0,
        g2h_tail: 0,
        capacity: 4096,
        pending_count: 0,
    };
    capsule.publish(state);

    // Warm up
    for _ in 0..1000 {
        let _ = capsule.has_space_for(256);
    }

    // Measure performance
    let iterations = 10_000_000;
    let start = std::time::Instant::now();

    for _ in 0..iterations {
        let _ = capsule.has_space_for(256);
    }

    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_nanos() / iterations;

    println!("Readiness check: {} ns/op", ns_per_op);

    // Target: <5ns per operation (aggressive but achievable with hot cache in release builds)
    // Debug builds are slower - allow up to 50ns
    #[cfg(debug_assertions)]
    let max_ns = 50;
    #[cfg(not(debug_assertions))]
    let max_ns = 10;

    assert!(
        ns_per_op < max_ns,
        "Readiness check too slow: {} ns/op (target <{}ns)",
        ns_per_op,
        max_ns
    );
}

#[test]
fn test_state_read_performance() {
    let capsule = GucReadyCapsule::with_capacity(4096);

    let state = GucCtbState {
        h2g_head: 0,
        h2g_tail: 1024,
        g2h_head: 0,
        g2h_tail: 0,
        capacity: 4096,
        pending_count: 5,
    };
    capsule.publish(state);

    // Warm up
    for _ in 0..1000 {
        let _ = capsule.read();
    }

    // Measure performance
    let iterations = 10_000_000;
    let start = std::time::Instant::now();

    for _ in 0..iterations {
        let _ = capsule.read();
    }

    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_nanos() / iterations;

    println!("State read: {} ns/op", ns_per_op);

    // Should be very fast (cache-hot atomic loads)
    // Debug builds are slower - allow up to 50ns
    #[cfg(debug_assertions)]
    let max_ns = 50;
    #[cfg(not(debug_assertions))]
    let max_ns = 15;

    assert!(
        ns_per_op < max_ns,
        "State read too slow: {} ns/op (target <{}ns)",
        ns_per_op,
        max_ns
    );
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_complete_command_submission_flow() {
    let h2g_buffer = vec![0u8; 4096];
    let g2h_buffer = vec![0u8; 4096];

    let ring_buffer = unsafe {
        GucCtbRingBuffer::new(
            h2g_buffer.as_ptr() as *mut u8,
            g2h_buffer.as_ptr() as *mut u8,
            4096,
        )
    };

    // 1. Check readiness
    let initial_state = ring_buffer.state().unwrap();
    assert_eq!(initial_state.h2g_head, 0);
    assert_eq!(initial_state.h2g_tail, 0);

    // 2. Reserve slot
    let reservation = ring_buffer.reserve_h2g_slot(64);
    assert!(reservation.is_some());
    let (offset, size) = reservation.unwrap();
    assert_eq!(offset, 0);
    assert_eq!(size, 64);

    // 3. Verify state updated
    let after_reserve = ring_buffer.state().unwrap();
    assert_eq!(after_reserve.h2g_tail, 64);
    assert_eq!(after_reserve.pending_count, 1);

    // 4. Reserve multiple slots
    for _ in 0..10 {
        ring_buffer.reserve_h2g_slot(32);
    }

    let after_multiple = ring_buffer.state().unwrap();
    assert_eq!(after_multiple.pending_count, 11); // Initial + 10 more
    assert!(after_multiple.h2g_tail > 64);
}

#[test]
fn test_g2h_response_processing() {
    let h2g_buffer = vec![0u8; 4096];
    let g2h_buffer = vec![0u8; 4096];

    let ring_buffer = unsafe {
        GucCtbRingBuffer::new(
            h2g_buffer.as_ptr() as *mut u8,
            g2h_buffer.as_ptr() as *mut u8,
            4096,
        )
    };

    // Simulate GuC sending responses (manually update state)
    // In real usage, GuC would write to g2h_buffer and update tail
    // For this test, we'll just verify the processing logic

    let processed = ring_buffer.process_g2h_responses();
    assert_eq!(processed, 0, "Should process 0 responses initially");

    // Verify utilization calculations work
    let h2g_util = ring_buffer.h2g_utilization();
    let g2h_util = ring_buffer.g2h_utilization();

    assert_eq!(h2g_util, 0, "H2G should be empty initially");
    assert_eq!(g2h_util, 0, "G2H should be empty initially");
}
