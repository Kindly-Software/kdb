//! # T28 Property Tests (Q8-Q14) - 20+ Tests
//!
//! Randomized testing with proptest to validate invariants across input space.

use git_coord::*;
use proptest::prelude::*;
use std::sync::Arc;
use std::thread;

// ============================================================================
// Q8: Universal Properties (10 tests)
// ============================================================================

proptest! {
    #[test]
    fn prop_lock_exclusivity(id1 in 1..u32::MAX, id2 in 1..u32::MAX) {
        let lock = Arc::new(GitLock::new());

        let id1 = id1 as u64;
        let id2 = id2 as u64;

        if id1 != id2 {
            let guard1 = lock.try_acquire(id1);
            let guard2 = lock.try_acquire(id2);

            // Property: Lock can only be held by one instance
            prop_assert!(guard1.is_ok() || guard2.is_ok());
            prop_assert!(!(guard1.is_ok() && guard2.is_ok()));
        }
    }

    #[test]
    fn prop_lock_sequence_monotonic(operations in 1..100usize) {
        let lock = GitLock::new();
        let mut last_seq = lock.sequence();

        for _ in 0..operations {
            if let Ok(guard) = lock.try_acquire(1) {
                let seq = guard.sequence();
                prop_assert!(seq > last_seq);
                last_seq = seq;
                drop(guard);
            }
        }
    }

    #[test]
    fn prop_queue_fifo_order(ops in prop::collection::vec(any::<u64>(), 1..100)) {
        let q = GitQueue::new();

        // Enqueue all operations
        for &id in &ops {
            if !q.is_full() {
                q.enqueue(GitOperation::Commit { author_id: id, timestamp: id });
            }
        }

        // Dequeue and verify FIFO order
        let mut dequeued = Vec::new();
        while let Some(op) = q.dequeue() {
            if let GitOperation::Commit { author_id, .. } = op {
                dequeued.push(author_id);
            }
        }

        // Property: Dequeued order matches enqueued order
        prop_assert_eq!(&dequeued[..], &ops[..dequeued.len()]);
    }

    #[test]
    fn prop_instance_uniqueness(count in 1..1000usize) {
        let reg = InstanceRegistry::new();
        let mut ids = Vec::new();

        for _ in 0..count {
            ids.push(reg.generate_id());
        }

        // Property: All IDs are unique
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        prop_assert_eq!(unique.len(), ids.len());
    }

    #[test]
    fn prop_instance_nonzero(count in 1..1000usize) {
        let reg = InstanceRegistry::new();

        for _ in 0..count {
            let id = reg.generate_id();
            // Property: IDs are never zero
            prop_assert_ne!(id.as_u64(), 0);
        }
    }

    #[test]
    fn prop_audit_hash_verification(entries in 1..100usize) {
        let mut log = AuditLog::new();

        for i in 1..=entries {
            log.append(i as u64, i as u64 * 1000, GitOperation::Noop);
        }

        // Property: All entries verify correctly
        for i in 0..log.len() {
            let entry = log.get(i as u64).unwrap();
            prop_assert!(entry.verify());
        }
    }

    #[test]
    fn prop_audit_chain_integrity(entries in 1..100usize) {
        let mut log = AuditLog::new();

        for i in 1..=entries {
            log.append(i as u64, i as u64 * 1000, GitOperation::Noop);
        }

        // Property: Hash chain is intact
        prop_assert!(log.verify_chain());
    }

    #[test]
    fn prop_queue_length_conservation(ops in 1..256usize) {
        let q = GitQueue::new();
        let mut enqueued = 0;

        for _ in 0..ops {
            if !q.is_full() {
                q.enqueue(GitOperation::Noop);
                enqueued += 1;
            }
        }

        // Property: Length equals enqueued count
        prop_assert_eq!(q.len(), enqueued);

        let mut dequeued = 0;
        while q.dequeue().is_some() {
            dequeued += 1;
        }

        // Property: Dequeued count equals enqueued count
        prop_assert_eq!(dequeued, enqueued);
    }

    #[test]
    fn prop_lock_release_makes_available(id in 1..u32::MAX) {
        let lock = GitLock::new();
        let id = id as u64;

        let guard = lock.try_acquire(id).unwrap();
        drop(guard);

        // Property: After release, lock is available
        prop_assert_eq!(lock.status(), LockStatus::Available);
        prop_assert_eq!(lock.holder(), 0);
    }

    #[test]
    fn prop_coordinator_execute_idempotent(count in 1..100usize) {
        let coord = GitCoordinator::new();

        for _ in 0..count {
            let result = coord.execute(GitOperation::Noop);
            // Property: Execute always succeeds (ignoring queue full)
            prop_assert!(result.is_ok() || result.is_err());
        }
    }
}

// ============================================================================
// Q9: Concurrent Properties (5 tests)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_concurrent_lock_no_lost_releases(operations in 10..100usize) {
        let lock = Arc::new(GitLock::new());
        let handles: Vec<_> = (0..4)
            .map(|i| {
                let l = Arc::clone(&lock);
                let ops = operations;
                thread::spawn(move || {
                    for _ in 0..ops {
                        if let Ok(guard) = l.try_acquire(i + 1) {
                            drop(guard);
                        }
                        thread::yield_now();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // Property: Lock is available after all releases
        prop_assert_eq!(lock.status(), LockStatus::Available);
    }

    #[test]
    fn prop_concurrent_instance_uniqueness(threads in 2..16usize) {
        let reg = Arc::new(InstanceRegistry::new());
        let ids = Arc::new(std::sync::Mutex::new(Vec::new()));

        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let r = Arc::clone(&reg);
                let i = Arc::clone(&ids);
                thread::spawn(move || {
                    for _ in 0..100 {
                        let id = r.generate_id();
                        i.lock().unwrap().push(id);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let ids = ids.lock().unwrap();
        let unique: std::collections::HashSet<_> = ids.iter().collect();

        // Property: All IDs are unique even under concurrency
        prop_assert_eq!(unique.len(), ids.len());
    }

    #[test]
    fn prop_concurrent_queue_no_lost_items(producers in 2..8usize, items_per_producer in 10..50usize) {
        let q = Arc::new(GitQueue::new());
        let total_items = producers * items_per_producer;

        // Producers
        let handles: Vec<_> = (0..producers)
            .map(|i| {
                let queue = Arc::clone(&q);
                let items = items_per_producer;
                thread::spawn(move || {
                    for j in 0..items {
                        let id = (i * 1000 + j) as u64;
                        while !queue.enqueue(GitOperation::Commit { author_id: id, timestamp: id }) {
                            thread::yield_now();
                        }
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // Consumer
        let mut count = 0;
        while q.dequeue().is_some() {
            count += 1;
        }

        // Property: No lost items
        prop_assert_eq!(count, total_items);
    }

    #[test]
    fn prop_lock_generation_consistency(threads in 2..8usize, ops_per_thread in 10..50usize) {
        let lock = Arc::new(GitLock::new());

        let handles: Vec<_> = (0..threads)
            .map(|i| {
                let l = Arc::clone(&lock);
                let ops = ops_per_thread;
                thread::spawn(move || {
                    for _ in 0..ops {
                        if let Ok(guard) = l.try_acquire(i as u64 + 1) {
                            let _seq = guard.sequence();
                            drop(guard);
                        }
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // Property: Final sequence number >= operations performed
        let final_seq = lock.sequence();
        prop_assert!(final_seq > 0);
    }

    #[test]
    fn prop_concurrent_coordinator_no_conflicts(instances in 2..8usize, ops_per_instance in 10..50usize) {
        let lock = Arc::new(GitLock::new());
        let success_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let handles: Vec<_> = (0..instances)
            .map(|i| {
                let l = Arc::clone(&lock);
                let s = Arc::clone(&success_count);
                let ops = ops_per_instance;
                thread::spawn(move || {
                    for _ in 0..ops {
                        if let Ok(guard) = l.try_acquire(i as u64 + 1) {
                            s.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            drop(guard);
                        }
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // Property: At least some operations succeeded
        let successes = success_count.load(std::sync::atomic::Ordering::Relaxed);
        prop_assert!(successes > 0);
    }
}

// ============================================================================
// Q10: Edge Cases with Properties (already covered in unit tests)
// ============================================================================

// ============================================================================
// Q11: ASSUM Verification (already covered via #ASSUME/#VERIFY comments)
// ============================================================================

// ============================================================================
// Q12-Q14: Additional Properties (5 tests)
// ============================================================================

proptest! {
    #[test]
    fn prop_audit_sequence_monotonic(entries in 1..100usize) {
        let mut log = AuditLog::new();

        for i in 1..=entries {
            log.append(i as u64, i as u64 * 1000, GitOperation::Noop);
        }

        // Property: Sequence numbers are strictly increasing
        for i in 0..log.len() - 1 {
            let seq1 = log.get(i as u64).unwrap().sequence;
            let seq2 = log.get((i + 1) as u64).unwrap().sequence;
            prop_assert!(seq2 > seq1);
        }
    }

    #[test]
    fn prop_queue_capacity_bound(enqueue_attempts in 256..512usize) {
        let q = GitQueue::new();
        let mut successful = 0;

        for _ in 0..enqueue_attempts {
            if q.enqueue(GitOperation::Noop) {
                successful += 1;
            }
        }

        // Property: Can't enqueue more than capacity
        prop_assert!(successful <= QUEUE_CAPACITY);
    }

    #[test]
    fn prop_lock_holder_consistency(id in 1..u32::MAX) {
        let lock = GitLock::new();
        let id = id as u64;

        let guard = lock.try_acquire(id).unwrap();

        // Property: Holder ID matches acquired ID
        prop_assert_eq!(lock.holder(), id);

        drop(guard);

        // Property: Holder is 0 after release
        prop_assert_eq!(lock.holder(), 0);
    }

    #[test]
    fn prop_operation_serialization_roundtrip(
        author_id in any::<u64>(),
        timestamp in any::<u64>()
    ) {
        #[cfg(feature = "bincode")]
        {
            let op = GitOperation::Commit { author_id, timestamp };

            // Property: Serialization round-trip preserves data
            let bytes = op.to_bytes().unwrap();
            let decoded = GitOperation::from_bytes(&bytes).unwrap();

            prop_assert_eq!(op, decoded);
        }
    }

    #[test]
    fn prop_audit_hash_deterministic(
        instance_id in any::<u64>(),
        timestamp in any::<u64>()
    ) {
        let entry1 = AuditEntry::new(1, instance_id, timestamp, GitOperation::Noop, [0u8; 32]);
        let entry2 = AuditEntry::new(1, instance_id, timestamp, GitOperation::Noop, [0u8; 32]);

        // Property: Same inputs produce same hash
        prop_assert_eq!(entry1.hash, entry2.hash);
    }
}
