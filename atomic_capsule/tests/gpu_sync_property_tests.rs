//! GPU Sync Capsule Property Tests (T28 Q8-Q14)
//!
//! Property-based testing for GpuSyncCapsule using proptest.

#![cfg(test)]

use atomic_capsule::gpu::{GpuSyncCapsule, MemoryBarrier};
use proptest::prelude::*;
use std::sync::Arc;
use std::thread;

// === Q8: Concurrent fence allocation (property test) ===

proptest! {
    #[test]
    fn property_concurrent_fence_allocation(
        num_threads in 2usize..8,
        allocations_per_thread in 10usize..50,
    ) {
        let capsule = Arc::new(GpuSyncCapsule::new(2));
        let mut handles = vec![];

        for _ in 0..num_threads {
            let capsule_clone = Arc::clone(&capsule);
            let handle = thread::spawn(move || {
                let mut allocated = Vec::new();
                for _ in 0..allocations_per_thread {
                    if let Some(fence) = capsule_clone.allocate_fence() {
                        allocated.push(fence);
                    }
                }
                // Free all allocated fences
                for fence in allocated {
                    capsule_clone.free_fence(fence);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // All fences should be freed
        prop_assert_eq!(capsule.get_fence_utilization(), 0);
    }
}

proptest! {
    #[test]
    fn property_fence_pool_exhaustion(seed in 0u64..1000) {
        let capsule = GpuSyncCapsule::new(2);
        let mut allocated = Vec::new();

        // Use seed for deterministic behavior
        let _ = seed;

        // Allocate all 8 fences
        for _ in 0..8 {
            if let Some(fence) = capsule.allocate_fence() {
                allocated.push(fence);
            }
        }

        // Pool should be full
        prop_assert_eq!(allocated.len(), 8);
        prop_assert_eq!(capsule.get_fence_utilization(), 8);

        // Next allocation must fail
        prop_assert!(capsule.allocate_fence().is_none());

        // Free one fence
        if let Some(fence) = allocated.pop() {
            capsule.free_fence(fence);
        }

        // Should be able to allocate again
        prop_assert!(capsule.allocate_fence().is_some());
    }
}

// === Q9: Timeline semaphore monotonicity (property test) ===

proptest! {
    #[test]
    fn property_timeline_monotonic(num_signals in 1usize..100) {
        let capsule = GpuSyncCapsule::new(2);
        let mut last_value = 0u64;

        for _ in 0..num_signals {
            let value = capsule.signal_timeline();
            prop_assert!(value > last_value, "Timeline must be monotonic");
            last_value = value;
        }

        prop_assert_eq!(capsule.get_timeline_value(), num_signals as u64);
    }
}

proptest! {
    #[test]
    fn property_timeline_wait_correctness(
        signals_before in 1usize..50,
        wait_value in 1usize..50,
    ) {
        let capsule = GpuSyncCapsule::new(2);

        // Signal N times
        for _ in 0..signals_before {
            capsule.signal_timeline();
        }

        let current_value = capsule.get_timeline_value();

        // Wait should succeed if wait_value <= current_value
        let should_succeed = (wait_value as u64) <= current_value;
        let result = capsule.wait_timeline(wait_value as u64);

        prop_assert_eq!(result, should_succeed);
    }
}

// === Q10: Concurrent timeline operations (property test) ===

proptest! {
    #[test]
    fn property_concurrent_timeline_signals(
        num_threads in 2usize..8,
        signals_per_thread in 10usize..50,
    ) {
        let capsule = Arc::new(GpuSyncCapsule::new(2));
        let mut handles = vec![];

        for _ in 0..num_threads {
            let capsule_clone = Arc::clone(&capsule);
            let handle = thread::spawn(move || {
                for _ in 0..signals_per_thread {
                    capsule_clone.signal_timeline();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let expected = (num_threads * signals_per_thread) as u64;
        prop_assert_eq!(capsule.get_timeline_value(), expected);
    }
}

// === Q11: Frame synchronization wraparound (property test) ===

proptest! {
    #[test]
    fn property_frame_wraparound(
        frames_in_flight in 2u32..8,
        num_advances in 1usize..100,
    ) {
        let capsule = GpuSyncCapsule::new(frames_in_flight);

        for _ in 0..num_advances {
            let frame = capsule.advance_frame();
            prop_assert!(frame < frames_in_flight as u64, "Frame must be within bounds");
        }

        // Check wraparound occurred
        let final_frame = capsule.get_current_frame();
        let expected = (num_advances as u64) % (frames_in_flight as u64);
        prop_assert_eq!(final_frame, expected);
    }
}

// === Q12: Barrier recording correctness (property test) ===

proptest! {
    #[test]
    fn property_barrier_recording(num_barriers in 1usize..1000) {
        let capsule = GpuSyncCapsule::new(2);

        for _ in 0..num_barriers {
            capsule.record_barrier(&MemoryBarrier::render_to_sample());
        }

        prop_assert_eq!(capsule.get_total_barriers(), num_barriers as u64);
    }
}

// === Q13: Statistics consistency (property test) ===

proptest! {
    #[test]
    fn property_statistics_consistency(
        num_signals in 1usize..100,
        num_waits in 1usize..100,
        num_barriers in 1usize..100,
        num_frames in 1usize..100,
    ) {
        let capsule = GpuSyncCapsule::new(3);

        for _ in 0..num_signals {
            capsule.signal_timeline();
        }

        for _ in 0..num_waits {
            capsule.wait_timeline(0);
        }

        for _ in 0..num_barriers {
            capsule.record_barrier(&MemoryBarrier::compute_to_compute());
        }

        for _ in 0..num_frames {
            capsule.advance_frame();
        }

        prop_assert_eq!(capsule.get_total_signals(), num_signals as u64);
        prop_assert_eq!(capsule.get_total_waits(), num_waits as u64);
        prop_assert_eq!(capsule.get_total_barriers(), num_barriers as u64);
        prop_assert_eq!(capsule.get_total_syncs(), num_frames as u64);
    }
}

// === Q14: Fence handle persistence (property test) ===

proptest! {
    #[test]
    fn property_fence_handle_persistence(handles in prop::collection::vec(any::<u64>(), 8)) {
        let capsule = GpuSyncCapsule::new(2);
        let mut allocated = Vec::new();

        // Allocate all fences
        for _ in 0..8 {
            if let Some(fence) = capsule.allocate_fence() {
                allocated.push(fence);
            }
        }

        // Set handles
        for (i, &handle) in handles.iter().enumerate() {
            capsule.set_fence_handle(allocated[i], handle);
        }

        // Verify handles persist
        for (i, &handle) in handles.iter().enumerate() {
            let retrieved = capsule.get_fence_handle(allocated[i]);
            prop_assert_eq!(retrieved, handle);
        }
    }
}
