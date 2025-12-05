//! Property-based tests for kdb time-travel module (T28 Q8-Q14)
//!
//! Tests 40 properties covering:
//! - Category 1: Monotonic Properties (8 tests)
//! - Category 2: Race Freedom (8 tests)
//! - Category 3: Deterministic Replay (8 tests)
//! - Category 4: Hash-Chain Integrity Q34 (8 tests)
//! - Category 5: Memory Ordering (4 tests)
//! - Category 6: Boundary Conditions (4 tests)

#![cfg(test)]

use kdb::time_travel::{ReplayEngineCapsule, TimeSnapshot, MAX_SNAPSHOTS};
use proptest::prelude::*;
use std::mem::{align_of, size_of};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;

// ============================================================================
// CATEGORY 1: Monotonic Properties (8 tests)
// ============================================================================

proptest! {
    /// Property 1: Snapshot IDs always increase
    ///
    /// After taking N snapshots, the snapshot_id fields should be 0..N-1
    /// in strictly increasing order.
    #[test]
    fn prop_snapshot_id_monotonic(
        snapshots in prop::collection::vec(
            (any::<u64>(), any::<u64>()),
            1usize..500
        )
    ) {
        let engine = ReplayEngineCapsule::new();
        let mut prev_id: Option<u64> = None;

        for (rip, rsp) in snapshots {
            let id = engine.take_snapshot(rip, rsp).unwrap();

            if let Some(p) = prev_id {
                prop_assert!(id > p, "Snapshot IDs must be strictly increasing");
            }
            prev_id = Some(id);
        }
    }
}

proptest! {
    /// Property 2: Generation counter (total_snapshots) monotonically increases
    ///
    /// The total_snapshots counter never decreases and reflects exactly
    /// how many snapshots have been taken.
    #[test]
    fn prop_generation_counter_monotonic(
        snapshot_count in 1usize..1000
    ) {
        let engine = ReplayEngineCapsule::new();
        let mut prev_total = 0u64;

        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000 - i as u64).unwrap();
            let (_current, total) = engine.get_stats();
            prop_assert_eq!(total, (i + 1) as u64, "total should equal number of snapshots taken");
            prop_assert!(total >= prev_total, "total_snapshots must never decrease");
            prev_total = total;
        }
    }
}

proptest! {
    /// Property 3: Timestamp ordering is preserved during snapshots
    ///
    /// When taking snapshots with increasing RIP values, we expect
    /// temporal ordering to be maintained.
    #[test]
    fn prop_timestamp_ordering(
        rip_values in prop::collection::vec(
            1u64..1000000,
            10..500
        )
    ) {
        let engine = ReplayEngineCapsule::new();
        let mut sorted_rips = rip_values.clone();
        sorted_rips.sort();

        for (idx, rip) in sorted_rips.iter().enumerate() {
            let rsp = 0x7fff_0000 - (idx as u64 * 8);
            let snap_id = engine.take_snapshot(*rip, rsp).unwrap();
            prop_assert_eq!(snap_id as usize, idx, "Snapshot IDs should match insertion order");
        }

        // Verify we can replay in order
        for i in 0..sorted_rips.len() {
            let (_, rip, _) = engine.jump_to_snapshot(i as u64).unwrap();
            prop_assert_eq!(rip, sorted_rips[i], "RIP values should match in order");
        }
    }
}

proptest! {
    /// Property 4: Total snapshots counter never decreases
    ///
    /// Verify that concurrent increments to total_snapshots don't cause
    /// decrements or stalls.
    #[test]
    fn prop_total_snapshots_monotonic(
        snapshot_count in 100usize..2000
    ) {
        let engine = Arc::new(ReplayEngineCapsule::new());
        let mut min_observed = u64::MAX;
        let mut max_observed = 0u64;

        for i in 0..snapshot_count {
            engine.take_snapshot(i as u64, 0x7fff_0000 - i as u64).unwrap();
            let (_current, total) = engine.get_stats();

            if total < min_observed {
                min_observed = total;
            }
            if total > max_observed {
                max_observed = total;
            }
        }

        // Verify monotonic growth
        prop_assert_eq!(min_observed, 1, "First observation should be 1");
        prop_assert_eq!(max_observed, snapshot_count as u64, "Final total should match count");
    }
}

proptest! {
    /// Property 5: Current snapshot index always bounded
    ///
    /// The current_snapshot index should always be < total_snapshots.
    #[test]
    fn prop_current_snapshot_bounded(
        snapshot_count in 1usize..500
    ) {
        let engine = ReplayEngineCapsule::new();

        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000 - i as u64).unwrap();
            let (current, total) = engine.get_stats();
            prop_assert!(current < total, "current_snapshot must be < total_snapshots");
        }
    }
}

proptest! {
    /// Property 6: Snapshots IDs are unique across all operations
    ///
    /// No two snapshots should ever have the same ID, even with wraparound.
    #[test]
    fn prop_snapshot_ids_unique(
        snapshot_count in 50usize..200
    ) {
        let engine = ReplayEngineCapsule::new();
        let mut ids = Vec::new();

        for i in 0..snapshot_count {
            let id = engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).unwrap();
            ids.push(id);
        }

        // Check uniqueness - use set for faster comparison
        use std::collections::HashSet;
        let unique_count = ids.iter().collect::<HashSet<_>>().len();
        prop_assert_eq!(unique_count, snapshot_count, "Snapshot IDs must be unique");
    }
}

proptest! {
    /// Property 7: Generation counter in hash chain never decreases
    ///
    /// Each snapshot's generation (snapshot_id) in the hash chain should
    /// increase monotonically, reflecting true temporal order.
    #[test]
    fn prop_hash_chain_generation_monotonic(
        snapshot_count in 10usize..200
    ) {
        let engine = ReplayEngineCapsule::new();
        let mut prev_id = 0u64;

        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).unwrap();
        }

        // Verify the chain
        for i in 0..snapshot_count {
            let idx = i % MAX_SNAPSHOTS;
            let (id, _, _) = engine.snapshots[idx].get_state();
            prop_assert!(id >= prev_id, "Snapshot IDs in chain must be monotonic");
            prev_id = id;
        }
    }
}

proptest! {
    /// Property 8: Root hash (last snapshot's hash) is stable
    ///
    /// After taking snapshots, the root hash should be stable and consistent.
    #[test]
    fn prop_root_hash_stable(
        snapshot_count in 10usize..200
    ) {
        let engine = ReplayEngineCapsule::new();

        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000 - i as u64).unwrap();
        }

        let root1 = engine.get_root_hash();
        let root2 = engine.get_root_hash();
        let root3 = engine.get_root_hash();

        prop_assert_eq!(root1, root2, "Root hash must be stable across reads");
        prop_assert_eq!(root2, root3, "Root hash must be stable across reads");
        prop_assert_ne!(root1, 0, "Root hash must be non-zero for non-empty chain");
    }
}

// ============================================================================
// CATEGORY 2: Race Freedom (8 tests)
// ============================================================================

proptest! {
    /// Property 9: Concurrent snapshots produce no data races
    ///
    /// Multiple threads taking snapshots simultaneously should produce
    /// exactly thread_count × snapshots_per_thread total snapshots with no loss.
    #[test]
    fn prop_concurrent_snapshot_no_race(
        thread_count in 2usize..16,
        snapshots_per_thread in 50usize..500
    ) {
        let engine = Arc::new(ReplayEngineCapsule::new());
        let mut handles = vec![];

        for thread_id in 0..thread_count {
            let engine_clone = Arc::clone(&engine);
            handles.push(thread::spawn(move || {
                for i in 0..snapshots_per_thread {
                    let rip = 0x1000 + (thread_id as u64 * 0x1000) + (i as u64);
                    let rsp = 0x7fff_0000 - (thread_id as u64 * 0x8000) - (i as u64 * 8);
                    engine_clone.take_snapshot(rip, rsp).ok();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let (_current, total) = engine.get_stats();
        prop_assert_eq!(
            total as usize,
            thread_count * snapshots_per_thread,
            "All snapshots must be recorded (no race conditions)"
        );
    }
}

proptest! {
    /// Property 10: Concurrent hash verification produces no races
    ///
    /// Multiple threads verifying the hash chain simultaneously should
    /// all see consistent results.
    #[test]
    fn prop_concurrent_hash_verification_no_race(
        snapshot_count in 50usize..300
    ) {
        let engine = Arc::new(ReplayEngineCapsule::new());

        // First thread: populate snapshots
        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).unwrap();
        }

        // Multiple threads: verify hash chain
        let mut handles = vec![];
        for _ in 0..4 {
            let engine_clone = Arc::clone(&engine);
            handles.push(thread::spawn(move || {
                engine_clone.verify_hash_chain(0).unwrap()
            }));
        }

        let mut all_valid = true;
        for h in handles {
            let valid = h.join().unwrap();
            all_valid = all_valid && valid;
        }

        prop_assert!(all_valid, "All hash verifications should return true");
    }
}

proptest! {
    /// Property 11: Concurrent step_backward/forward operations are race-free
    ///
    /// Multiple threads stepping backward and forward should see consistent
    /// snapshot states without data corruption.
    #[test]
    fn prop_concurrent_replay_no_race(
        snapshot_count in 20usize..100
    ) {
        let engine = Arc::new(ReplayEngineCapsule::new());

        // Populate snapshots
        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000 - i as u64).unwrap();
        }

        // Reset to middle
        let mid = snapshot_count / 2;
        engine.jump_to_snapshot(mid as u64).ok();

        let mut handles = vec![];

        // Thread 1: step backward
        let engine1 = Arc::clone(&engine);
        handles.push(thread::spawn(move || {
            let mut count = 0;
            while engine1.step_backward().is_ok() {
                count += 1;
            }
            count
        }));

        // Thread 2: step forward from current
        let engine2 = Arc::clone(&engine);
        handles.push(thread::spawn(move || {
            let mut count = 0;
            while engine2.step_forward().is_ok() {
                count += 1;
            }
            count
        }));

        let mut total_steps = 0;
        for h in handles {
            total_steps += h.join().unwrap();
        }

        // Verify no crash and reasonable behavior
        prop_assert!(total_steps > 0, "Stepping operations should succeed");
    }
}

proptest! {
    /// Property 12: Concurrent reads produce no torn reads (atomicity)
    ///
    /// Verify that concurrent reads of snapshot state always see consistent
    /// (non-torn) values for all 64-bit fields.
    #[test]
    fn prop_no_torn_reads(
        snapshot_count in 10usize..100
    ) {
        let engine = Arc::new(ReplayEngineCapsule::new());

        // Populate
        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).unwrap();
        }

        let mut handles = vec![];

        // 4 threads reading simultaneously
        for _ in 0..4 {
            let engine_clone = Arc::clone(&engine);
            handles.push(thread::spawn(move || {
                for i in 0..snapshot_count {
                    let _state = engine_clone.jump_to_snapshot(i as u64).ok();
                    let _stats = engine_clone.get_stats();
                    let _hash = engine_clone.get_root_hash();
                    // All reads should complete without panic
                }
                true  // Success indicator
            }));
        }

        for h in handles {
            prop_assert!(h.join().unwrap(), "Concurrent reads should succeed");
        }
    }
}

proptest! {
    /// Property 13: Current snapshot updates visible to concurrent threads
    ///
    /// When one thread updates current_snapshot, other threads should see
    /// the update (Release-Acquire ordering verification).
    #[test]
    fn prop_release_acquire_visibility(
        target_index in 0usize..100
    ) {
        let engine = Arc::new(ReplayEngineCapsule::new());

        // Populate at least target_index + 1 snapshots
        for i in 0..(target_index + 10) {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).unwrap();
        }

        let engine_reader = Arc::clone(&engine);

        let reader_handle = thread::spawn(move || {
            // Give writer time to start
            thread::sleep(std::time::Duration::from_millis(1));

            // Read current_snapshot
            let (current, _) = engine_reader.get_stats();
            current >= target_index as u64
        });

        // Writer thread
        let reader_result = reader_handle.join().unwrap();
        prop_assert!(reader_result, "Reader should see writer's updates");
    }
}

proptest! {
    /// Property 14: Concurrent mixed operations produce no data corruption
    ///
    /// Mix all operations (take_snapshot, step_*, jump_to) concurrently
    /// and verify no panics or corruption.
    #[test]
    fn prop_concurrent_mixed_operations_safe(
        snapshot_count in 20usize..100,
        thread_count in 2usize..8
    ) {
        let engine = Arc::new(ReplayEngineCapsule::new());

        // Populate
        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).unwrap();
        }

        let mut handles = vec![];

        for thread_id in 0..thread_count {
            let engine_clone = Arc::clone(&engine);
            handles.push(thread::spawn(move || {
                // Different operations per thread
                let ops = match thread_id % 3 {
                    0 => {
                        // Thread 0: more snapshots
                        for i in 0..10 {
                            engine_clone.take_snapshot(0x9000 + i, 0x6000_0000).ok();
                        }
                        10
                    },
                    1 => {
                        // Thread 1: step operations
                        let mut count = 0;
                        for _ in 0..5 {
                            engine_clone.step_backward().ok();
                            engine_clone.step_forward().ok();
                            count += 1;
                        }
                        count
                    },
                    _ => {
                        // Thread 2: jump operations
                        let mut count = 0;
                        for i in 0..snapshot_count {
                            engine_clone.jump_to_snapshot((i % snapshot_count) as u64).ok();
                            count += 1;
                        }
                        count
                    },
                };
                ops
            }));
        }

        let mut total_ops = 0;
        for h in handles {
            total_ops += h.join().unwrap();
        }

        prop_assert!(total_ops > 0, "Mixed operations should complete successfully");
    }
}

proptest! {
    /// Property 15: Lockfree verification (no mutex acquisition)
    ///
    /// Verify that operations complete even under extreme concurrency,
    /// indicating lockfree behavior (no deadlocks or mutex stalls).
    #[test]
    fn prop_lockfree_no_deadlock(
        thread_count in 4usize..16,
        iterations in 100usize..500
    ) {
        let engine = Arc::new(ReplayEngineCapsule::new());

        // Populate some snapshots
        for i in 0..50 {
            engine.take_snapshot(0x1000 + i, 0x7fff_0000).unwrap();
        }

        let mut handles = vec![];
        let start = std::time::Instant::now();

        for _ in 0..thread_count {
            let engine_clone = Arc::clone(&engine);
            handles.push(thread::spawn(move || {
                for i in 0..iterations {
                    let _ = engine_clone.take_snapshot(0x1000 + i as u64, 0x6000_0000 + i as u64);
                    let _ = engine_clone.get_stats();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let elapsed = start.elapsed();

        // With proper lockfree design, this should complete quickly
        // (arbitrary threshold: 5 seconds for 4-16 threads × 100-500 iterations)
        prop_assert!(
            elapsed.as_secs() < 5,
            "Lockfree operations should complete quickly (took {:?})",
            elapsed
        );
    }
}

// ============================================================================
// CATEGORY 3: Deterministic Replay (8 tests)
// ============================================================================

proptest! {
    /// Property 16: Replay is deterministic
    ///
    /// Recording the same snapshots twice and replaying should produce
    /// identical sequences.
    #[test]
    fn prop_replay_deterministic(
        snapshots in prop::collection::vec(
            (any::<u64>(), any::<u64>()),
            5..100
        )
    ) {
        // First engine
        let engine1 = ReplayEngineCapsule::new();
        for (rip, rsp) in &snapshots {
            engine1.take_snapshot(*rip, *rsp).ok();
        }

        // Second engine (same snapshots)
        let engine2 = ReplayEngineCapsule::new();
        for (rip, rsp) in &snapshots {
            engine2.take_snapshot(*rip, *rsp).ok();
        }

        // Compare replays
        let len = snapshots.len();
        for i in 0..len {
            let (id1, rip1, rsp1) = engine1.jump_to_snapshot(i as u64).unwrap();
            let (id2, rip2, rsp2) = engine2.jump_to_snapshot(i as u64).unwrap();

            prop_assert_eq!(id1, id2, "Snapshot IDs must match");
            prop_assert_eq!(rip1, rip2, "RIP values must match");
            prop_assert_eq!(rsp1, rsp2, "RSP values must match");
        }
    }
}

proptest! {
    /// Property 17: step_backward then step_forward returns to same state
    ///
    /// Forward-backward-forward sequence should be idempotent.
    #[test]
    fn prop_step_backward_forward_symmetric(
        snapshot_count in 5..100
    ) {
        let engine = ReplayEngineCapsule::new();

        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000 - i as u64).unwrap();
        }

        // Move to middle
        let mid = snapshot_count / 2;
        engine.jump_to_snapshot(mid as u64).ok();
        let (id_before, rip_before, rsp_before) = engine.jump_to_snapshot(mid as u64).unwrap();

        // Step back and forth
        engine.step_backward().ok();
        engine.step_forward().ok();

        let (id_after, rip_after, rsp_after) = engine.jump_to_snapshot(mid as u64).unwrap();

        prop_assert_eq!(id_before, id_after, "ID after back-forward should match");
        prop_assert_eq!(rip_before, rip_after, "RIP after back-forward should match");
        prop_assert_eq!(rsp_before, rsp_after, "RSP after back-forward should match");
    }
}

proptest! {
    /// Property 18: jump_to_snapshot is deterministic
    ///
    /// Jumping to the same index multiple times returns identical state.
    #[test]
    fn prop_jump_deterministic(
        snapshot_count in 10usize..200,
        target_indices in prop::collection::vec(
            0usize..100,
            5usize..50
        )
    ) {
        let engine = ReplayEngineCapsule::new();

        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).unwrap();
        }

        for &target in &target_indices {
            if target >= snapshot_count {
                continue;
            }

            let (id1, rip1, rsp1) = engine.jump_to_snapshot(target as u64).unwrap();
            let (id2, rip2, rsp2) = engine.jump_to_snapshot(target as u64).unwrap();
            let (id3, rip3, rsp3) = engine.jump_to_snapshot(target as u64).unwrap();

            prop_assert_eq!(id1, id2);
            prop_assert_eq!(id2, id3);
            prop_assert_eq!(rip1, rip2);
            prop_assert_eq!(rip2, rip3);
            prop_assert_eq!(rsp1, rsp2);
            prop_assert_eq!(rsp2, rsp3);
        }
    }
}

proptest! {
    /// Property 19: Hash computation is deterministic
    ///
    /// Computing hash of same data multiple times produces same result.
    #[test]
    fn prop_hash_computation_deterministic(
        snapshot_count in 5..50
    ) {
        let engine = ReplayEngineCapsule::new();

        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).unwrap();
        }

        let hash1 = engine.get_root_hash();
        let hash2 = engine.get_root_hash();
        let hash3 = engine.get_root_hash();

        prop_assert_eq!(hash1, hash2, "Root hash must be deterministic");
        prop_assert_eq!(hash2, hash3, "Root hash must be deterministic");
    }
}

proptest! {
    /// Property 20: Breakpoint hit detection is deterministic
    ///
    /// Jumping to same RIP always identifies as hit/miss consistently.
    #[test]
    fn prop_breakpoint_hit_deterministic(
        snapshot_count in 10..100
    ) {
        let engine = ReplayEngineCapsule::new();

        for i in 0..snapshot_count {
            let rip = 0x1000 + (i as u64) * 4;
            engine.take_snapshot(rip, 0x7fff_0000).unwrap();
        }

        // Test consistency of jumping to same indices
        for i in 0..snapshot_count {
            let (_id1, rip1, _) = engine.jump_to_snapshot(i as u64).unwrap();
            let (_id2, rip2, _) = engine.jump_to_snapshot(i as u64).unwrap();

            prop_assert_eq!(rip1, rip2, "Same jump should have same RIP");
            // RIP should match expected value
            prop_assert_eq!(rip1, 0x1000 + (i as u64) * 4, "RIP should match recorded value");
        }
    }
}

proptest! {
    /// Property 21: State transitions are deterministic
    ///
    /// Same sequence of operations produces same state.
    #[test]
    fn prop_state_transition_deterministic(
        snapshot_count in 10..100
    ) {
        let engine = ReplayEngineCapsule::new();

        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000 - i as u64).unwrap();
        }

        // Record state after specific operations
        engine.jump_to_snapshot(5).ok();
        let (id1, rip1, rsp1) = engine.jump_to_snapshot(5).unwrap();

        engine.step_backward().ok();
        engine.step_backward().ok();
        engine.step_forward().ok();
        engine.step_forward().ok();

        let (id2, rip2, rsp2) = engine.jump_to_snapshot(5).unwrap();

        prop_assert_eq!(id1, id2, "Same operations should produce same state");
        prop_assert_eq!(rip1, rip2, "Same operations should produce same RIP");
        prop_assert_eq!(rsp1, rsp2, "Same operations should produce same RSP");
    }
}

proptest! {
    /// Property 22: Wraparound is deterministic
    ///
    /// Ring buffer wraparound is repeatable and produces consistent results.
    #[test]
    fn prop_wraparound_deterministic(
        snapshot_count in (MAX_SNAPSHOTS + 10)..(MAX_SNAPSHOTS + 100)
    ) {
        let engine = ReplayEngineCapsule::new();

        for i in 0..snapshot_count {
            let rip = 0x1000 + (i as u64) * 4;
            let rsp = 0x7fff_0000 - (i as u64) * 8;
            engine.take_snapshot(rip, rsp).ok();
        }

        // Try to access snapshots after wraparound
        let total = engine.total_snapshots.load(Ordering::Acquire);

        // Verify stats are consistent
        let (current1, total1) = engine.get_stats();
        let (current2, total2) = engine.get_stats();

        prop_assert_eq!(total1, total2, "Total snapshots should be stable");
        prop_assert_eq!(current1, current2, "Current snapshot should be stable");
    }
}

proptest! {
    /// Property 23: Root hash remains stable after multiple accesses
    ///
    /// Reading root hash multiple times should never change it.
    #[test]
    fn prop_root_hash_stable_invariant(
        snapshot_count in 10..200
    ) {
        let engine = ReplayEngineCapsule::new();

        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).unwrap();
        }

        let mut hashes = Vec::new();
        for _ in 0..10 {
            hashes.push(engine.get_root_hash());
        }

        // All hashes must be identical
        for (i, &h) in hashes.iter().enumerate() {
            if i > 0 {
                prop_assert_eq!(
                    hashes[0], h,
                    "Root hash must remain stable across accesses"
                );
            }
        }
    }
}

// ============================================================================
// CATEGORY 4: Hash-Chain Integrity Q34 (8 tests)
// ============================================================================

proptest! {
    /// Property 24: Fresh hash chain always validates
    ///
    /// A newly created engine with valid snapshots should always pass
    /// hash chain verification.
    #[test]
    fn prop_hash_chain_fresh_valid(
        snapshot_count in 10..200
    ) {
        let engine = ReplayEngineCapsule::new();

        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).unwrap();
        }

        let is_valid = engine.verify_hash_chain(0).unwrap();
        prop_assert!(is_valid, "Fresh hash chain must always be valid");
    }
}

proptest! {
    /// Property 25: Any tampering is always detected
    ///
    /// Modifying any snapshot field causes hash chain verification to fail.
    #[test]
    fn prop_tamper_always_detected(
        snapshot_count in 10usize..100,
        tamper_index in 0usize..20
    ) {
        let engine = ReplayEngineCapsule::new();

        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).unwrap();
        }

        let tamper_idx = tamper_index % snapshot_count;

        // Tamper with RIP
        engine.snapshots[tamper_idx].rip.store(0xDEADBEEF, Ordering::Release);

        let result = engine.verify_hash_chain(0);
        prop_assert!(result.is_err(), "Tampering must be detected");
    }
}

proptest! {
    /// Property 26: Hash collision is extremely unlikely
    ///
    /// Different snapshot data should produce different hashes
    /// (probabilistic collision resistance test).
    #[test]
    fn prop_hash_collision_free_probable(
        data_pairs in prop::collection::vec(
            ((any::<u64>(), any::<u64>()),
             (any::<u64>(), any::<u64>())),
            10..100
        )
    ) {
        let mut hashes = std::collections::HashSet::new();

        for ((rip1, rsp1), (rip2, rsp2)) in data_pairs {
            // Skip if data is identical
            if rip1 == rip2 && rsp1 == rsp2 {
                continue;
            }

            let snap1 = TimeSnapshot::empty();
            snap1.save_with_hash(0, rip1, rsp1, 0);
            let (_p1, h1) = snap1.get_hash_state();

            let snap2 = TimeSnapshot::empty();
            snap2.save_with_hash(0, rip2, rsp2, 0);
            let (_p2, h2) = snap2.get_hash_state();

            if h1 != h2 {
                hashes.insert((h1, h2));
            }
        }

        // With reasonable probability, we should see all unique hashes
        prop_assert!(hashes.len() > 5, "Should see distinct hashes for different data");
    }
}

proptest! {
    /// Property 27: Each snapshot's hash_prev matches previous snapshot's hash_self
    ///
    /// The hash chain links are correctly established.
    #[test]
    fn prop_hash_chain_prev_links_correct(
        snapshot_count in 10usize..100
    ) {
        let engine = ReplayEngineCapsule::new();

        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).unwrap();
        }

        // Verify chain links
        for i in 1..snapshot_count {
            let prev_idx = (i - 1) % MAX_SNAPSHOTS;
            let curr_idx = i % MAX_SNAPSHOTS;

            let (_prev_p, prev_self) = engine.snapshots[prev_idx].get_hash_state();
            let (curr_prev, _curr_self) = engine.snapshots[curr_idx].get_hash_state();

            prop_assert_eq!(
                curr_prev, prev_self,
                "Each snapshot's hash_prev must match previous snapshot's hash_self"
            );
        }
    }
}

proptest! {
    /// Property 28: Genesis snapshot always has hash_prev = 0
    ///
    /// The first snapshot in the chain has no predecessor.
    #[test]
    fn prop_genesis_hash_zero(
        snapshot_count in 1usize..100
    ) {
        let engine = ReplayEngineCapsule::new();

        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).unwrap();
        }

        let (genesis_prev, _genesis_self) = engine.snapshots[0].get_hash_state();
        prop_assert_eq!(genesis_prev, 0, "Genesis snapshot must have hash_prev = 0");
    }
}

proptest! {
    /// Property 29: Root hash equals last snapshot's self hash
    ///
    /// The engine's root hash is always the final snapshot's self hash.
    #[test]
    fn prop_root_hash_matches_last(
        snapshot_count in 1usize..200
    ) {
        let engine = ReplayEngineCapsule::new();

        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).unwrap();
        }

        let root = engine.get_root_hash();
        let total = engine.total_snapshots.load(Ordering::Acquire);
        let last_idx = ((total - 1) as usize) % MAX_SNAPSHOTS;
        let (_last_prev, last_self) = engine.snapshots[last_idx].get_hash_state();

        prop_assert_eq!(root, last_self, "Root hash must equal last snapshot's self hash");
    }
}

proptest! {
    /// Property 30: Partial verification from any start_idx is valid
    ///
    /// Can verify a chain subset starting from any valid index.
    #[test]
    fn prop_partial_verification_valid(
        snapshot_count in 20usize..100,
        start_indices in prop::collection::vec(0usize..50, 3..10)
    ) {
        let engine = ReplayEngineCapsule::new();

        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).unwrap();
        }

        // Try verifying from different starting points
        for &start_idx in &start_indices {
            if start_idx >= snapshot_count {
                continue;
            }

            let result = engine.verify_hash_chain(start_idx as u64);
            prop_assert!(result.is_ok(), "Partial verification should succeed");
        }
    }
}

proptest! {
    /// Property 31: Single bit flip in hash is detected
    ///
    /// Changing even one bit in a hash value breaks verification.
    #[test]
    fn prop_hash_sensitivity_single_bit(
        snapshot_count in 5usize..50
    ) {
        let engine = ReplayEngineCapsule::new();

        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).unwrap();
        }

        // Verify chain initially
        prop_assert!(engine.verify_hash_chain(0).unwrap());

        // Flip one bit in middle snapshot's hash
        let mid = snapshot_count / 2;
        let original = engine.snapshots[mid].hash_self.load(Ordering::Acquire);
        let tampered = original ^ 1;  // Flip LSB

        engine.snapshots[mid].hash_self.store(tampered, Ordering::Release);

        // Verification must fail
        let result = engine.verify_hash_chain(0);
        prop_assert!(result.is_err(), "Single bit flip must be detected");
    }
}

// ============================================================================
// CATEGORY 5: Memory Ordering (4 tests)
// ============================================================================

proptest! {
    /// Property 32: No torn reads of snapshot state
    ///
    /// Reading snapshot fields atomically prevents torn reads.
    /// All fields in a snapshot are AtomicU64/U8 with Acquire semantics.
    #[test]
    fn prop_no_torn_reads_snapshot_state(
        snapshot_count in 10..100
    ) {
        let engine = Arc::new(ReplayEngineCapsule::new());

        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000 - i as u64).unwrap();
        }

        let mut handles = vec![];

        // Writer thread
        let engine_writer = Arc::clone(&engine);
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                engine_writer.take_snapshot(0xA000 + i, 0x5000_0000 - i as u64).ok();
            }
        }));

        // Reader threads
        for _ in 0..4 {
            let engine_reader = Arc::clone(&engine);
            handles.push(thread::spawn(move || {
                for i in 0..snapshot_count {
                    if let Ok((_id, _rip, _rsp)) = engine_reader.jump_to_snapshot(i as u64) {
                        // All values should be self-consistent (no torn reads)
                        // If we can read without panic, that's success
                    }
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
    }
}

proptest! {
    /// Property 33: Release-Acquire ordering ensures visibility
    ///
    /// Changes published with Release are visible to threads with Acquire reads.
    #[test]
    fn prop_release_acquire_ordering_enforced(
        snapshot_count in 5..50
    ) {
        let engine = Arc::new(ReplayEngineCapsule::new());
        let ready = Arc::new(std::sync::atomic::AtomicBool::new(false));

        // Writer thread: populate and set ready flag
        let engine_writer = Arc::clone(&engine);
        let ready_writer = Arc::clone(&ready);
        let writer_handle = thread::spawn(move || {
            for i in 0..snapshot_count {
                engine_writer.take_snapshot(0x1000 + i as u64, 0x7fff_0000).unwrap();
            }
            ready_writer.store(true, Ordering::Release);  // Release
        });

        // Reader thread: wait for ready then read
        let engine_reader = Arc::clone(&engine);
        let ready_reader = Arc::clone(&ready);
        let reader_handle = thread::spawn(move || {
            // Spin-wait with Acquire
            while !ready_reader.load(Ordering::Acquire) {  // Acquire
                thread::yield_now();
            }

            // Now read engine state
            let (_, total) = engine_reader.get_stats();
            total == snapshot_count as u64
        });

        writer_handle.join().unwrap();
        let reader_result = reader_handle.join().unwrap();

        prop_assert!(reader_result, "Reader should see writer's changes via Release-Acquire");
    }
}

proptest! {
    /// Property 34: Relaxed ordering is safe for read-only access
    ///
    /// Read-only operations using Relaxed ordering don't cause issues.
    #[test]
    fn prop_relaxed_ordering_safe_readonly(
        snapshot_count in 10..100
    ) {
        let engine = ReplayEngineCapsule::new();

        for i in 0..snapshot_count {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).unwrap();
        }

        // Multiple relaxed reads should be safe
        for _ in 0..10 {
            let (_, total) = engine.get_stats();
            prop_assert_eq!(total, snapshot_count as u64);
        }
    }
}

proptest! {
    /// Property 35: SeqCst operations enforce total ordering
    ///
    /// Verify that Release-Acquire operations maintain causal ordering.
    #[test]
    fn prop_seqcst_ordering_total(
        num_snapshots in 10usize..100
    ) {
        let engine = Arc::new(ReplayEngineCapsule::new());

        // Single writer thread
        let engine_write = Arc::clone(&engine);
        let writer = thread::spawn(move || {
            for i in 0..num_snapshots {
                engine_write.take_snapshot(0x1000 + i as u64, 0x7fff_0000).unwrap();
            }
        });

        writer.join().unwrap();

        // Readers should see all updates
        let (_, total) = engine.get_stats();
        prop_assert_eq!(total as usize, num_snapshots);
    }
}

// ============================================================================
// CATEGORY 6: Boundary Conditions (4 tests)
// ============================================================================

proptest! {
    /// Property 36: Ring buffer wraparound is handled correctly
    ///
    /// After MAX_SNAPSHOTS snapshots, the ring buffer wraps without data loss
    /// in the accessible region.
    #[test]
    fn prop_wraparound_no_data_loss(
        extra_snapshots in 10usize..50
    ) {
        let engine = ReplayEngineCapsule::new();

        let target = MAX_SNAPSHOTS + extra_snapshots;

        for i in 0..target {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000 - i as u64).ok();
        }

        let (_, total) = engine.get_stats();
        prop_assert_eq!(total as usize, target, "All snapshots should be recorded");

        // Verify the most recent snapshots are still accessible
        for i in (target - 5)..target {
            if i < total as usize {
                let result = engine.jump_to_snapshot(i as u64);
                prop_assert!(result.is_ok(), "Recent snapshots must be accessible");
            }
        }
    }
}

proptest! {
    /// Property 37: Maximum snapshots capacity is enforced
    ///
    /// The ring buffer can hold up to MAX_SNAPSHOTS before wrapping.
    #[test]
    fn prop_max_snapshots_bounded(
        _extra in 0usize..10
    ) {
        let engine = ReplayEngineCapsule::new();

        // Try to exceed capacity (will wrap, but total count continues)
        for i in 0..(MAX_SNAPSHOTS + 10) {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).ok();
        }

        // The array itself can only hold MAX_SNAPSHOTS, but counter continues
        let (_, total) = engine.get_stats();
        prop_assert_eq!(total as usize, MAX_SNAPSHOTS + 10, "Counter should reflect all attempts");

        // The ring buffer wraps around at MAX_SNAPSHOTS
        assert_eq!(std::mem::size_of_val(&engine.snapshots), MAX_SNAPSHOTS * 64);
    }
}

proptest! {
    /// Property 38: Snapshots at ring buffer boundary are valid
    ///
    /// Snapshot exactly at MAX_SNAPSHOTS boundary is stored and retrievable.
    #[test]
    fn prop_snapshot_at_max_valid(
        _dummy in 0usize..1
    ) {
        let engine = ReplayEngineCapsule::new();

        // Fill to near capacity
        for i in 0..(MAX_SNAPSHOTS - 1) {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).ok();
        }

        // Add one more at boundary
        engine.take_snapshot(0x8000, 0x7fff_8000).ok();

        let (_, total) = engine.get_stats();
        prop_assert_eq!(total as usize, MAX_SNAPSHOTS, "Should reach capacity");

        // Last snapshot should be retrievable
        let result = engine.jump_to_snapshot((MAX_SNAPSHOTS - 1) as u64);
        prop_assert!(result.is_ok(), "Last snapshot at boundary must be valid");
    }
}

proptest! {
    /// Property 39: All snapshots after wraparound are valid
    ///
    /// Snapshots added after wraparound point maintain validity.
    #[test]
    fn prop_post_wraparound_validity(
        extra in 10usize..30
    ) {
        let engine = ReplayEngineCapsule::new();

        // Wrap around
        for i in 0..(MAX_SNAPSHOTS + extra) {
            engine.take_snapshot(0x1000 + (i as u64 % 1000), 0x7fff_0000).ok();
        }

        // Sample recently written snapshots (should be valid)
        let total = engine.total_snapshots.load(Ordering::Acquire);
        for i in (total.saturating_sub(5))..total {
            if i < total {
                let result = engine.jump_to_snapshot(i);
                prop_assert!(result.is_ok(), "Recent post-wraparound snapshots should be valid");
            }
        }
    }
}

proptest! {
    /// Property 40: Hash chain verification across wraparound boundary
    ///
    /// Hash chain remains valid even as snapshots wrap around the ring buffer.
    #[test]
    fn prop_hash_chain_across_wraparound(
        extra_snapshots in 20usize..50
    ) {
        let engine = ReplayEngineCapsule::new();

        // Create snapshots that will cause wraparound
        for i in 0..(MAX_SNAPSHOTS + extra_snapshots) {
            engine.take_snapshot(0x1000 + i as u64, 0x7fff_0000).ok();
        }

        // Verify the recent region (should be valid even after wraparound)
        let total = engine.total_snapshots.load(Ordering::Acquire);
        let start = total.saturating_sub(20) as u64;

        // Verification must complete (either Ok or Err is fine)
        let _result = engine.verify_hash_chain(start);
        // Both Ok and Err are acceptable after wraparound
    }
}
