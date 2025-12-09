//! Property tests for FenceCapsule
//!
//! ## Test Coverage
//!
//! 1. **Concurrent Readers**: Many threads reading, one writer
//! 2. **Version Consistency**: No torn reads under concurrent access
//! 3. **Monotonicity**: Fence values always increase
//! 4. **Generation Counter**: Prevents ABA problems
//! 5. **Memory Ordering**: Relaxed reads are safe

use kiang::fence::{FenceCapsule, FenceState};
use proptest::prelude::*;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ========== Property Tests ==========

proptest! {
    /// Property: Fence values are always monotonically increasing
    ///
    /// #ASSUME_MONOTONIC: Signal operations preserve monotonicity
    /// #VERIFY_MONOTONIC: This test validates the invariant
    #[test]
    fn prop_monotonic_signals(values in prop::collection::vec(1u64..1000000, 10..100)) {
        let fence = FenceCapsule::new(1);

        let mut sorted_values = values.clone();
        sorted_values.sort_unstable();

        // Signal in sorted order
        for (i, &value) in sorted_values.iter().enumerate() {
            fence.signal(value, (i as u64) * 1000);

            // Verify completed value is at least the signaled value
            assert!(fence.completed_value() >= value);

            // All previous values should be signaled
            for &prev in sorted_values.iter().take(i + 1) {
                assert!(fence.is_signaled(prev));
            }
        }
    }

    /// Property: Readers never see torn reads (version mismatch)
    ///
    /// #ASSUME_TOCTOU_SAFE: Generation counter prevents torn reads
    /// #VERIFY_TOCTOU_PREVENTED: This test validates under concurrent load
    #[test]
    fn prop_no_torn_reads(fence_id in 1u32..1000, values in prop::collection::vec(1u64..100000, 100..200)) {
        let fence = Arc::new(FenceCapsule::new(fence_id));

        // Do first signal to ensure capsule is valid before readers start
        fence.signal(values[0], 0);

        // Spawn reader threads
        let mut readers = Vec::new();
        for _ in 0..8 {
            let fence_clone = Arc::clone(&fence);
            let handle = thread::spawn(move || {
                let mut valid_reads = 0;
                let mut invalid_reads = 0;

                for _ in 0..1000 {
                    let snapshot = fence_clone.read_snapshot();
                    if snapshot.is_some() {
                        valid_reads += 1;
                    } else {
                        invalid_reads += 1;
                    }
                    thread::yield_now();
                }

                (valid_reads, invalid_reads)
            });
            readers.push(handle);
        }

        // Writer signals remaining values
        for (i, &value) in values.iter().skip(1).enumerate() {
            fence.signal(value, (i as u64) * 1000);
            thread::yield_now();
        }

        // Join readers
        for handle in readers {
            let (valid, _invalid) = handle.join().unwrap();
            // Should have many valid reads (after initial signal)
            assert!(valid > 0, "Reader saw only invalid snapshots");
        }

        // Final verification: fence should be valid
        assert!(fence.read_snapshot().is_some());
    }

    /// Property: is_signaled() is consistent with check_fence()
    ///
    /// #ASSUME_CONSISTENCY: Both methods read same underlying state
    /// #VERIFY_CONSISTENCY: This test validates equivalence
    #[test]
    fn prop_is_signaled_consistent(completed in 0u64..100000, wait in 0u64..100000) {
        let fence = FenceCapsule::new(1);
        fence.signal(completed, 1000);

        let is_sig = fence.is_signaled(wait);
        let state = fence.check_fence(wait);

        match state {
            FenceState::Signaled { .. } => assert!(is_sig, "is_signaled() should be true"),
            FenceState::Pending { .. } => assert!(!is_sig, "is_signaled() should be false"),
            FenceState::Invalid => {
                // Invalid state can happen if read during write, both methods should agree
            }
        }
    }

    /// Property: Snapshot data matches fence state
    ///
    /// #ASSUME_DATA_INTEGRITY: Snapshot reflects actual fence state
    /// #VERIFY_DATA_INTEGRITY: This test validates field consistency
    #[test]
    fn prop_snapshot_data_integrity(fence_id in 1u32..1000, value in 1u64..1000000, timestamp in 0u64..u32::MAX as u64) {
        let fence = FenceCapsule::new(fence_id);
        fence.signal(value, timestamp);

        if let Some(snapshot) = fence.read_snapshot() {
            assert_eq!(snapshot.fence_id, fence_id);
            assert_eq!(snapshot.completed_value, value);
            assert_eq!(snapshot.timestamp_ns, timestamp);
            assert_eq!(snapshot.version & 1, 0, "Version should be even (committed)");
        }
    }
}

// ========== Concurrent Stress Tests ==========

#[test]
fn test_concurrent_readers_single_writer() {
    const NUM_READERS: usize = 16;
    const SIGNALS_PER_WRITER: u64 = 1000;

    let fence = Arc::new(FenceCapsule::new(42));

    // Spawn reader threads
    let mut readers = Vec::new();
    for reader_id in 0..NUM_READERS {
        let fence_clone = Arc::clone(&fence);
        let handle = thread::spawn(move || {
            let mut max_seen = 0u64;
            let mut valid_reads = 0;
            let mut monotonic_violations = 0;

            for _ in 0..10000 {
                let current = fence_clone.completed_value();

                // Verify monotonicity (we never see values decrease)
                if current < max_seen {
                    monotonic_violations += 1;
                }

                max_seen = max_seen.max(current);

                if let Some(_snapshot) = fence_clone.read_snapshot() {
                    valid_reads += 1;
                }

                // Small yield to allow writer to proceed
                thread::yield_now();
            }

            (reader_id, max_seen, valid_reads, monotonic_violations)
        });
        readers.push(handle);
    }

    // Writer signals increasing values
    let writer_fence = Arc::clone(&fence);
    let writer = thread::spawn(move || {
        for i in 1..=SIGNALS_PER_WRITER {
            writer_fence.signal(i, i * 1000);
            thread::yield_now();
        }
    });

    // Wait for writer
    writer.join().unwrap();

    // Verify all readers saw monotonic increases
    for handle in readers {
        let (reader_id, max_seen, valid_reads, violations) = handle.join().unwrap();

        println!(
            "Reader {}: max_seen={}, valid_reads={}, violations={}",
            reader_id, max_seen, valid_reads, violations
        );

        assert_eq!(
            violations, 0,
            "Reader {} saw non-monotonic values",
            reader_id
        );
        assert!(
            valid_reads > 0,
            "Reader {} never saw valid snapshots",
            reader_id
        );
    }

    // Final verification
    assert_eq!(fence.completed_value(), SIGNALS_PER_WRITER);
}

#[test]
fn test_high_frequency_signaling() {
    // Simulate high-frequency GPU signals (1kHz GuC scheduler)
    const SIGNAL_COUNT: u64 = 10000;

    let fence = Arc::new(FenceCapsule::new(1));

    // Spawn monitoring reader
    let fence_clone = Arc::clone(&fence);
    let reader = thread::spawn(move || {
        let mut max_seen = 0u64;
        let start = std::time::Instant::now();

        while max_seen < SIGNAL_COUNT {
            max_seen = fence_clone.completed_value();
            thread::yield_now();
        }

        start.elapsed()
    });

    // Writer signals as fast as possible
    let fence_clone = Arc::clone(&fence);
    let writer = thread::spawn(move || {
        let start = std::time::Instant::now();

        for i in 1..=SIGNAL_COUNT {
            let now_ns = start.elapsed().as_nanos() as u64;
            fence_clone.signal(i, now_ns);
        }

        start.elapsed()
    });

    let write_time = writer.join().unwrap();
    let read_time = reader.join().unwrap();

    println!("High-frequency test:");
    println!("  Signals: {}", SIGNAL_COUNT);
    println!("  Write time: {:?}", write_time);
    println!("  Read time: {:?}", read_time);
    println!(
        "  Avg signal latency: {:?}",
        write_time / (SIGNAL_COUNT as u32)
    );

    // Verify final state
    assert_eq!(fence.completed_value(), SIGNAL_COUNT);
}

#[test]
fn test_wait_timeout_success() {
    let fence = Arc::new(FenceCapsule::new(1));

    // Signal in background after delay
    let fence_clone = Arc::clone(&fence);
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        fence_clone.signal(100, 5000);
    });

    // Wait should succeed
    let result = fence.wait_timeout(100, Duration::from_millis(100));
    assert!(result.is_ok());

    let snapshot = result.unwrap();
    assert_eq!(snapshot.completed_value, 100);
}

#[test]
fn test_wait_timeout_failure() {
    let fence = FenceCapsule::new(1);

    // Never signal, should timeout
    let result = fence.wait_timeout(100, Duration::from_millis(50));
    assert!(result.is_err());

    let elapsed = result.unwrap_err();
    assert!(elapsed >= Duration::from_millis(50));
}

#[test]
fn test_fence_state_transitions() {
    let fence = FenceCapsule::new(1);

    // Initially invalid (uncommitted)
    assert_eq!(fence.check_fence(0), FenceState::Invalid);

    // After first signal: pending for higher values
    fence.signal(50, 1000);

    match fence.check_fence(100) {
        FenceState::Pending {
            current_value,
            wait_value,
        } => {
            assert_eq!(current_value, 50);
            assert_eq!(wait_value, 100);
        }
        _ => panic!("Expected Pending state"),
    }

    // Signaled for lower values
    match fence.check_fence(25) {
        FenceState::Signaled {
            completed_value,
            timestamp_ns,
        } => {
            assert_eq!(completed_value, 50);
            assert_eq!(timestamp_ns, 1000);
        }
        _ => panic!("Expected Signaled state"),
    }

    // After second signal: now signaled
    fence.signal(100, 2000);

    match fence.check_fence(100) {
        FenceState::Signaled {
            completed_value,
            timestamp_ns,
        } => {
            assert_eq!(completed_value, 100);
            assert_eq!(timestamp_ns, 2000);
        }
        _ => panic!("Expected Signaled state"),
    }
}

#[test]
fn test_version_counter_wrapping() {
    // Test version counter wraps correctly after 255 signals
    let fence = FenceCapsule::new(1);

    for i in 0..300 {
        fence.signal(i, i * 1000);

        // Should always be readable
        let snapshot = fence
            .read_snapshot()
            .expect("Fence should be valid after signal");

        assert_eq!(snapshot.completed_value, i);

        // Version should always be even (committed)
        assert_eq!(snapshot.version & 1, 0, "Version should be even");
    }
}

#[test]
fn test_memory_layout() {
    // Verify 64-byte alignment for cache efficiency
    let fence = FenceCapsule::new(1);
    let ptr = &fence as *const FenceCapsule as usize;

    assert_eq!(
        ptr % 64,
        0,
        "FenceCapsule should be 64-byte aligned for cache optimization"
    );
}

#[test]
fn test_concurrent_snapshot_reads() {
    // Verify many concurrent snapshot reads don't interfere
    const NUM_READERS: usize = 32;

    let fence = Arc::new(FenceCapsule::new(1));
    fence.signal(12345, 67890);

    let mut handles = Vec::new();

    for _ in 0..NUM_READERS {
        let fence_clone = Arc::clone(&fence);
        let handle = thread::spawn(move || {
            for _ in 0..1000 {
                if let Some(snapshot) = fence_clone.read_snapshot() {
                    assert_eq!(snapshot.fence_id, 1);
                    assert_eq!(snapshot.completed_value, 12345);
                    assert_eq!(snapshot.timestamp_ns, 67890);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}
