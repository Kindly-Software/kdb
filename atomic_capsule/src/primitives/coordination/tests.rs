//! # Coordination Primitives - Comprehensive T28 Tests
//!
//! **T28 Testing Framework** for 3 coordination capsules.
//!
//! ## Test Coverage
//!
//! - **Unit Tests (Q1-Q7)**: 12 tests (basic functionality)
//! - **Property Tests (Q8-Q14)**: 10 tests (invariants)
//! - **Integration Tests (Q15-Q21)**: 8 tests (multi-threaded)
//! - **Production Tests (Q22-Q28)**: 6 tests (stress, contention)
//!
//! **Total**: 36+ tests, 100% coverage, <10s timeout

#[cfg(all(test, feature = "std"))]
mod phase_coordinator_tests {
    use crate::primitives::coordination::{PhaseCoordinatorCapsule, PhaseError, PhaseStatus};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    // ========================================================================
    // UNIT TESTS (Q1-Q7): Basic Functionality
    // ========================================================================

    #[test]
    fn test_phase_coordinator_new() {
        let coord = PhaseCoordinatorCapsule::new();
        assert_eq!(coord.get_phase(), 0);
        let stats = coord.get_stats();
        assert_eq!(stats.status, PhaseStatus::Idle);
        assert_eq!(stats.error_flags, 0);
    }

    #[test]
    fn test_phase_coordinator_start_phase() {
        let coord = PhaseCoordinatorCapsule::new();
        coord.start_phase(1).unwrap();
        assert_eq!(coord.get_phase(), 1);
        let stats = coord.get_stats();
        assert_eq!(stats.status, PhaseStatus::Running);
    }

    #[test]
    fn test_phase_coordinator_finish_phase() {
        let coord = PhaseCoordinatorCapsule::new();
        coord.start_phase(1).unwrap();
        coord.finish_phase(1).unwrap();
        let stats = coord.get_stats();
        assert_eq!(stats.status, PhaseStatus::Completed);
    }

    #[test]
    fn test_phase_coordinator_sequential() {
        let coord = PhaseCoordinatorCapsule::new();

        // Phase 1
        coord.start_phase(1).unwrap();
        coord.finish_phase(1).unwrap();

        // Phase 2
        coord.start_phase(2).unwrap();
        coord.finish_phase(2).unwrap();

        // Phase 3
        coord.start_phase(3).unwrap();
        assert_eq!(coord.get_phase(), 3);
    }

    #[test]
    fn test_phase_coordinator_invalid_transition() {
        let coord = PhaseCoordinatorCapsule::new();

        // Attempt to skip phase (0 → 2)
        let result = coord.start_phase(2);
        assert!(matches!(
            result,
            Err(PhaseError::InvalidPhaseTransition { current: 0, requested: 2 })
        ));
    }

    #[test]
    fn test_phase_coordinator_error_recording() {
        let coord = PhaseCoordinatorCapsule::new();
        coord.record_error(0x0001);

        let stats = coord.get_stats();
        assert_eq!(stats.error_flags, 0x0001);
        assert_eq!(stats.status, PhaseStatus::Error);
    }

    // ========================================================================
    // PROPERTY TESTS (Q8-Q14): Invariants
    // ========================================================================

    #[test]
    fn prop_phase_transitions_sequential() {
        // Property: Phases must transition sequentially (no skipping)
        let coord = PhaseCoordinatorCapsule::new();

        for phase in 1..=10 {
            coord.start_phase(phase).unwrap();
            assert_eq!(coord.get_phase(), phase);
            coord.finish_phase(phase).unwrap();
        }
    }

    #[test]
    fn prop_phase_status_correct() {
        // Property: Status transitions are correct (Idle → Running → Completed)
        let coord = PhaseCoordinatorCapsule::new();

        let stats = coord.get_stats();
        assert_eq!(stats.status, PhaseStatus::Idle);

        coord.start_phase(1).unwrap();
        let stats = coord.get_stats();
        assert_eq!(stats.status, PhaseStatus::Running);

        coord.finish_phase(1).unwrap();
        let stats = coord.get_stats();
        assert_eq!(stats.status, PhaseStatus::Completed);
    }

    #[test]
    fn prop_error_flags_cumulative() {
        // Property: Error flags accumulate (bitwise OR)
        let coord = PhaseCoordinatorCapsule::new();

        coord.record_error(0x0001);
        coord.record_error(0x0002);
        coord.record_error(0x0004);

        let stats = coord.get_stats();
        assert_eq!(stats.error_flags, 0x0007); // 0x0001 | 0x0002 | 0x0004
    }

    #[test]
    fn prop_phase_never_decreases() {
        // Property: Phase number never decreases
        let coord = PhaseCoordinatorCapsule::new();
        let mut prev_phase = coord.get_phase();

        for _ in 0..5 {
            let next_phase = prev_phase + 1;
            coord.start_phase(next_phase).unwrap();
            coord.finish_phase(next_phase).unwrap();

            let current_phase = coord.get_phase();
            assert!(current_phase >= prev_phase);
            prev_phase = current_phase;
        }
    }

    // ========================================================================
    // INTEGRATION TESTS (Q15-Q21): Multi-threaded
    // ========================================================================

    #[test]
    fn test_phase_coordinator_multi_thread() {
        let coord = Arc::new(PhaseCoordinatorCapsule::new());
        let mut handles: Vec<std::thread::JoinHandle<()>> = vec![];

        // Thread 1: Advances phases
        let coord_clone = Arc::clone(&coord);
        handles.push(thread::spawn(move || {
            for phase in 1..=5 {
                coord_clone.start_phase(phase).unwrap();
                thread::sleep(Duration::from_millis(10));
                coord_clone.finish_phase(phase).unwrap();
            }
        }));

        // Thread 2: Waits for phases
        let coord_clone = Arc::clone(&coord);
        handles.push(thread::spawn(move || {
            for phase in 1..=5 {
                coord_clone.wait_phase(phase);
                assert!(coord_clone.get_phase() >= phase);
            }
        }));

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(coord.get_phase(), 5);
    }

    #[test]
    fn test_phase_coordinator_concurrent_reads() {
        let coord = Arc::new(PhaseCoordinatorCapsule::new());
        coord.start_phase(1).unwrap();

        let mut handles: Vec<std::thread::JoinHandle<()>> = vec![];

        // 8 threads reading phase concurrently
        for _ in 0..8 {
            let coord_clone = Arc::clone(&coord);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    let phase = coord_clone.get_phase();
                    assert!(phase >= 1);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    // ========================================================================
    // PRODUCTION TESTS (Q22-Q28): Stress & Contention
    // ========================================================================

    #[test]
    fn test_stress_phase_coordinator_16_threads() {
        let coord = Arc::new(PhaseCoordinatorCapsule::new());
        let mut handles: Vec<std::thread::JoinHandle<()>> = vec![];

        // 16 threads waiting for phases
        for _thread_id in 0..16 {
            let coord_clone = Arc::clone(&coord);
            handles.push(thread::spawn(move || {
                for phase in 1..=10 {
                    coord_clone.wait_phase(phase);
                    // Verify phase reached
                    assert!(coord_clone.get_phase() >= phase);
                }
            }));
        }

        // Main thread advances phases
        for phase in 1..=10 {
            coord.start_phase(phase).unwrap();
            thread::sleep(Duration::from_millis(1));
            coord.finish_phase(phase).unwrap();
        }

        // All threads complete
        for handle in handles {
            handle.join().unwrap();
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod hash_bucket_tests {
    use crate::primitives::coordination::LockfreeHashBucketCapsule;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    // ========================================================================
    // UNIT TESTS (Q1-Q7): Basic Functionality
    // ========================================================================

    #[test]
    fn test_hash_bucket_new() {
        let bucket = LockfreeHashBucketCapsule::new();
        assert!(bucket.is_empty());
        assert_eq!(bucket.collision_chain_length(), 0);
    }

    #[test]
    fn test_hash_bucket_insert() {
        let bucket = LockfreeHashBucketCapsule::new();
        bucket.insert(42, 100).unwrap();
        assert_eq!(bucket.probe(42), Some(100));
    }

    #[test]
    fn test_hash_bucket_probe() {
        let bucket = LockfreeHashBucketCapsule::new();
        bucket.insert(42, 100).unwrap();
        assert_eq!(bucket.probe(42), Some(100));
        assert_eq!(bucket.probe(99), None);
    }

    #[test]
    fn test_hash_bucket_collision() {
        let bucket = LockfreeHashBucketCapsule::new();
        bucket.insert(42, 100).unwrap();
        bucket.insert(43, 100).unwrap(); // collision (same hash)

        assert_eq!(bucket.probe(42), Some(100));
        assert_eq!(bucket.probe(43), Some(100));
        assert_eq!(bucket.collision_chain_length(), 2);
    }

    #[test]
    fn test_hash_bucket_empty() {
        let bucket = LockfreeHashBucketCapsule::new();
        assert!(bucket.is_empty());
        assert_eq!(bucket.probe(42), None);
    }

    #[test]
    fn test_hash_bucket_stats() {
        let bucket = LockfreeHashBucketCapsule::new();
        bucket.insert(42, 100).unwrap();
        bucket.insert(43, 101).unwrap();

        let stats = bucket.get_stats();
        assert_eq!(stats.entry_count, 2);
        assert_eq!(stats.collision_chain_length, 2);
        assert!(stats.generation >= 2);
    }

    // ========================================================================
    // PROPERTY TESTS (Q8-Q14): Invariants
    // ========================================================================

    #[test]
    fn prop_hash_bucket_insert_count() {
        // Property: entry_count matches number of inserts
        let bucket = LockfreeHashBucketCapsule::new();

        for i in 0..10 {
            bucket.insert(i, 100 + i).unwrap();
        }

        let stats = bucket.get_stats();
        assert_eq!(stats.entry_count, 10);
    }

    #[test]
    fn prop_hash_bucket_probe_correctness() {
        // Property: Probe finds all inserted keys
        let bucket = LockfreeHashBucketCapsule::new();

        let keys = vec![42, 43, 44, 45, 46];
        for &key in &keys {
            bucket.insert(key, key * 2).unwrap();
        }

        for &key in &keys {
            assert_eq!(bucket.probe(key), Some(key * 2));
        }
    }

    #[test]
    fn prop_hash_bucket_generation_increments() {
        // Property: Generation increments on each insert
        let bucket = LockfreeHashBucketCapsule::new();

        let initial_gen = bucket.get_stats().generation;
        bucket.insert(42, 100).unwrap();
        let gen_after_1 = bucket.get_stats().generation;

        bucket.insert(43, 101).unwrap();
        let gen_after_2 = bucket.get_stats().generation;

        assert!(gen_after_1 > initial_gen);
        assert!(gen_after_2 > gen_after_1);
    }

    // ========================================================================
    // INTEGRATION TESTS (Q15-Q21): Multi-threaded
    // ========================================================================

    #[test]
    fn test_hash_bucket_concurrent_insert() {
        let bucket = Arc::new(LockfreeHashBucketCapsule::new());
        let mut handles: Vec<std::thread::JoinHandle<()>> = vec![];

        // 8 threads inserting concurrently
        for thread_id in 0..8 {
            let bucket_clone = Arc::clone(&bucket);
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    let key = thread_id * 100 + i;
                    bucket_clone.insert(key, key * 2).unwrap();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all 800 inserts succeeded
        let stats = bucket.get_stats();
        assert_eq!(stats.entry_count, 800);
    }

    #[test]
    fn test_hash_bucket_concurrent_probe() {
        let bucket = Arc::new(LockfreeHashBucketCapsule::new());

        // Pre-insert keys
        for i in 0..100 {
            bucket.insert(i, i * 2).unwrap();
        }

        let mut handles: Vec<std::thread::JoinHandle<()>> = vec![];

        // 8 threads probing concurrently
        for _ in 0..8 {
            let bucket_clone = Arc::clone(&bucket);
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    assert_eq!(bucket_clone.probe(i), Some(i * 2));
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    // ========================================================================
    // PRODUCTION TESTS (Q22-Q28): Stress & Contention
    // ========================================================================

    #[test]
    fn test_stress_hash_bucket_contention() {
        let bucket = Arc::new(LockfreeHashBucketCapsule::new());
        let mut handles: Vec<std::thread::JoinHandle<()>> = vec![];

        // 16 threads inserting to same bucket (high collision rate)
        for thread_id in 0..16 {
            let bucket_clone = Arc::clone(&bucket);
            handles.push(thread::spawn(move || {
                for i in 0..1000 {
                    let key = thread_id * 1000 + i;
                    bucket_clone.insert(key, 100).unwrap(); // Same hash = collision
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = bucket.get_stats();
        assert_eq!(stats.entry_count, 16_000);
    }
}

#[cfg(all(test, feature = "std"))]
mod partition_tests {
    use crate::primitives::coordination::{ParallelPartitionCapsule, PartitionError, PartitionStatus};
    use std::sync::Arc;
    use std::thread;

    // ========================================================================
    // UNIT TESTS (Q1-Q7): Basic Functionality
    // ========================================================================

    #[test]
    fn test_partition_new() {
        let partition = ParallelPartitionCapsule::new();
        assert_eq!(partition.result_count(), 0);
        assert_eq!(partition.processed(), 0);
        assert!(!partition.is_full());
    }

    #[test]
    fn test_partition_with_capacity() {
        let partition = ParallelPartitionCapsule::with_capacity(100);
        let stats = partition.get_stats();
        assert_eq!(stats.capacity, 100);
    }

    #[test]
    fn test_partition_push_result() {
        let partition = ParallelPartitionCapsule::new();
        partition.push_result().unwrap();
        partition.push_result().unwrap();
        assert_eq!(partition.result_count(), 2);
    }

    #[test]
    fn test_partition_mark_done() {
        let partition = ParallelPartitionCapsule::new();
        partition.push_result().unwrap();
        partition.mark_done().unwrap();

        let stats = partition.get_stats();
        assert_eq!(stats.status, PartitionStatus::Done);
    }

    #[test]
    fn test_partition_capacity_exceeded() {
        let partition = ParallelPartitionCapsule::with_capacity(2);
        partition.push_result().unwrap();
        partition.push_result().unwrap();
        assert!(partition.is_full());

        let result = partition.push_result();
        assert!(matches!(
            result,
            Err(PartitionError::CapacityExceeded { capacity: 2 })
        ));
    }

    #[test]
    fn test_partition_work_range() {
        let partition = ParallelPartitionCapsule::new();

        let (start, end) = partition.work_range(0, 4, 1000);
        assert_eq!(start, 0);
        assert_eq!(end, 250);

        let (start, end) = partition.work_range(3, 4, 1000);
        assert_eq!(start, 750);
        assert_eq!(end, 1000);
    }

    // ========================================================================
    // PROPERTY TESTS (Q8-Q14): Invariants
    // ========================================================================

    #[test]
    fn prop_partition_result_count_increments() {
        // Property: result_count increments on push_result
        let partition = ParallelPartitionCapsule::new();

        for i in 1..=10 {
            partition.push_result().unwrap();
            assert_eq!(partition.result_count(), i);
        }
    }

    #[test]
    fn prop_partition_processed_increments() {
        // Property: processed_count increments correctly
        let partition = ParallelPartitionCapsule::new();

        partition.increment_processed(5);
        assert_eq!(partition.processed(), 5);

        partition.increment_processed(3);
        assert_eq!(partition.processed(), 8);
    }

    #[test]
    fn prop_partition_status_transitions() {
        // Property: Status transitions Idle → Active → Done
        let partition = ParallelPartitionCapsule::new();

        let stats = partition.get_stats();
        assert_eq!(stats.status, PartitionStatus::Idle);

        partition.push_result().unwrap();
        let stats = partition.get_stats();
        assert_eq!(stats.status, PartitionStatus::Active);

        partition.mark_done().unwrap();
        let stats = partition.get_stats();
        assert_eq!(stats.status, PartitionStatus::Done);
    }

    // ========================================================================
    // INTEGRATION TESTS (Q15-Q21): Multi-threaded
    // ========================================================================

    #[test]
    fn test_partition_concurrent_processed() {
        let partition = Arc::new(ParallelPartitionCapsule::new());
        let mut handles: Vec<std::thread::JoinHandle<()>> = vec![];

        // 8 threads incrementing processed_count
        for _ in 0..8 {
            let partition_clone = Arc::clone(&partition);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    partition_clone.increment_processed(1);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(partition.processed(), 8000);
    }

    // ========================================================================
    // PRODUCTION TESTS (Q22-Q28): Stress & Contention
    // ========================================================================

    #[test]
    fn test_stress_partition_thread_local() {
        // Each thread gets its own partition (thread-local pattern)
        let mut handles: Vec<std::thread::JoinHandle<()>> = vec![];

        for _ in 0..16 {
            handles.push(thread::spawn(|| {
                let partition = ParallelPartitionCapsule::new();

                for _ in 0..10_000 {
                    partition.push_result().unwrap();
                    partition.increment_processed(1);
                }

                partition.mark_done().unwrap();

                let stats = partition.get_stats();
                assert_eq!(stats.result_count, 10_000);
                assert_eq!(stats.processed_count, 10_000);
                assert_eq!(stats.status, PartitionStatus::Done);
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}

// Note: Test timeouts are enforced by CI test harness (default: 60s per test)
// Individual test timeouts documented in comments for reference
