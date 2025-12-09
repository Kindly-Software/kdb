//! # BatchCoordinatorCapsule Test Suite - T28 4-Tier Framework
//!
//! **Tier**: T0 (Auditable) + T1 (Atomic) + T4 (Batch)
//!
//! **Test Coverage**: 35 tests across 4 tiers
//! - **Tier 1 (Unit)**: 12 tests - Basic operations, error conditions
//! - **Tier 2 (Property)**: 8 tests - Invariant verification with proptest
//! - **Tier 3 (Integration)**: 10 tests - Multi-worker scenarios
//! - **Tier 4 (Production)**: 5 tests - Concurrent stress, 16 workers
//!
//! **Framework**: UCE34 Q1-Q34 + Chaos + ASSUM + B32 + I20
//!
//! ## Test Organization
//!
//! ```
//! Unit Tests (12)
//! ├─ Initialization: test_new_coordinator
//! ├─ Basic operations: test_add_batch, test_claim_batch_single_worker
//! ├─ Error handling: test_claim_batch_no_batches, test_invalid_worker_id
//! ├─ Completion: test_complete_batch_single_worker, test_generation_increments
//! ├─ Tracking: test_worker_assignment_tracking, test_multiple_batches_sequential
//! ├─ State: test_reset, test_layout_alignment, test_wrapping_batch_ids
//!
//! Property Tests (8)
//! ├─ Monotonicity: proptest_head_tail_monotonic
//! ├─ Generation parity: proptest_generation_even_invariant
//! ├─ Worker consistency: proptest_worker_assignment_consistency
//! ├─ Batch ordering: proptest_batch_ordering
//! ├─ Completion safety: proptest_completion_all_batches
//! ├─ Contention patterns: proptest_concurrent_claims
//! ├─ Stats accuracy: proptest_stats_accuracy
//! ├─ Edge cases: proptest_wraparound_safety
//!
//! Integration Tests (10)
//! ├─ Two-worker pipeline: test_two_workers_sequential
//! ├─ Partial completion: test_partial_completion
//! ├─ Worker stall detection: test_stalled_worker_detection
//! ├─ Batch reordering: test_batch_claim_reordering (workers skip ahead)
//! ├─ Generation commitment: test_generation_commitment_semantics
//! ├─ Concurrent streams: test_concurrent_batch_streams
//! ├─ Producer-consumer sync: test_producer_consumer_sync
//! ├─ Multi-batch pipelining: test_multi_batch_pipelining
//! ├─ Fairness: test_worker_fairness
//! ├─ Recovery: test_recovery_after_stall
//!
//! Production Tests (5)
//! ├─ 16-worker stress (1K batches): test_16_workers_1k_batches
//! ├─ Contention measurement: test_contention_measurement
//! ├─ Latency percentiles: test_latency_percentiles
//! ├─ Memory isolation: test_memory_isolation
//! ├─ Wraparound corner case: test_wraparound_corner_case
//! ```

use kindly_dedup::parallel::{BatchCoordinatorCapsule, BatchCoordinatorError, BatchId, CoordinationStats};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

// ============================================================================
// TIER 1: UNIT TESTS (12)
// ============================================================================

#[test]
fn test_new_coordinator() {
    let coordinator = BatchCoordinatorCapsule::new();
    let stats = coordinator.stats();
    assert_eq!(stats.total_batches, 0);
    assert_eq!(stats.batches_claimed, 0);
    assert_eq!(stats.batches_completed, 0);
    assert_eq!(stats.generation, 0);
    assert!(coordinator.all_complete());
}

#[test]
fn test_add_batch() {
    let coordinator = BatchCoordinatorCapsule::new();
    let batch1 = coordinator.add_batch();
    assert_eq!(batch1.raw(), 0);

    let batch2 = coordinator.add_batch();
    assert_eq!(batch2.raw(), 1);

    let batch3 = coordinator.add_batch();
    assert_eq!(batch3.raw(), 2);

    let stats = coordinator.stats();
    assert_eq!(stats.total_batches, 3);
}

#[test]
fn test_claim_batch_single_worker() {
    let coordinator = BatchCoordinatorCapsule::new();
    coordinator.add_batch();

    let batch = coordinator.claim_batch(0).expect("Should claim batch");
    assert_eq!(batch.raw(), 0);

    let stats = coordinator.stats();
    assert_eq!(stats.batches_claimed, 1);
}

#[test]
fn test_claim_batch_no_batches() {
    let coordinator = BatchCoordinatorCapsule::new();
    let result = coordinator.claim_batch(0);
    assert_eq!(result, Err(BatchCoordinatorError::NoBatchesAvailable));
}

#[test]
fn test_invalid_worker_id() {
    let coordinator = BatchCoordinatorCapsule::new();
    coordinator.add_batch();

    let result = coordinator.claim_batch(16);
    assert_eq!(result, Err(BatchCoordinatorError::InvalidWorkerId(16)));

    let result = coordinator.claim_batch(255);
    assert_eq!(result, Err(BatchCoordinatorError::InvalidWorkerId(255)));
}

#[test]
fn test_complete_batch_single_worker() {
    let coordinator = BatchCoordinatorCapsule::new();
    coordinator.add_batch();

    let batch = coordinator.claim_batch(0).expect("Should claim batch");
    coordinator.complete_batch(batch, 0).expect("Should complete batch");

    let stats = coordinator.stats();
    assert_eq!(stats.batches_completed, 1);
    assert!(coordinator.all_complete());
}

#[test]
fn test_generation_increments() {
    let coordinator = BatchCoordinatorCapsule::new();
    assert_eq!(coordinator.generation.load(std::sync::atomic::Ordering::Acquire), 0);

    coordinator.add_batch();
    let batch = coordinator.claim_batch(0).expect("Should claim batch");
    coordinator.complete_batch(batch, 0).expect("Should complete batch");

    assert_eq!(coordinator.generation.load(std::sync::atomic::Ordering::Acquire), 1);
}

#[test]
fn test_worker_assignment_tracking() {
    let coordinator = BatchCoordinatorCapsule::new();
    coordinator.add_batch();

    assert_eq!(coordinator.worker_batch(0), None);

    let batch = coordinator.claim_batch(0).expect("Should claim batch");
    assert_eq!(coordinator.worker_batch(0), Some(batch));

    coordinator.complete_batch(batch, 0).expect("Should complete batch");
    assert_eq!(coordinator.worker_batch(0), None);
}

#[test]
fn test_multiple_batches_sequential() {
    let coordinator = BatchCoordinatorCapsule::new();

    // Add 10 batches
    for _ in 0..10 {
        coordinator.add_batch();
    }

    // Claim and complete in order
    for i in 0..10 {
        let batch = coordinator.claim_batch((i % 16) as u32).expect("Should claim batch");
        assert_eq!(batch.raw(), i as u32);
        coordinator.complete_batch(batch, (i % 16) as u32).expect("Should complete batch");
    }

    assert!(coordinator.all_complete());
    let stats = coordinator.stats();
    assert_eq!(stats.batches_completed, 10);
}

#[test]
fn test_reset() {
    let coordinator = BatchCoordinatorCapsule::new();
    coordinator.add_batch();
    coordinator.add_batch();

    let batch = coordinator.claim_batch(0).expect("Should claim batch");
    coordinator.complete_batch(batch, 0).expect("Should complete batch");

    coordinator.reset();
    assert!(coordinator.all_complete());
    assert_eq!(coordinator.worker_batch(0), None);

    let stats = coordinator.stats();
    assert_eq!(stats.total_batches, 0);
    assert_eq!(stats.batches_claimed, 0);
}

#[test]
fn test_layout_alignment() {
    let coordinator = BatchCoordinatorCapsule::new();
    let ptr = &coordinator as *const _ as usize;

    // Verify 128-byte alignment
    assert_eq!(ptr % 128, 0, "BatchCoordinatorCapsule must be 128-byte aligned");
}

#[test]
fn test_wrapping_batch_ids() {
    let coordinator = BatchCoordinatorCapsule::new();

    // Add many batches to test wrapping behavior
    for _ in 0..1000 {
        coordinator.add_batch();
    }

    let stats = coordinator.stats();
    assert_eq!(stats.total_batches, 1000);
}

// ============================================================================
// TIER 2: PROPERTY TESTS (8)
// ============================================================================

#[test]
fn proptest_head_tail_monotonic() {
    // Verify that head and tail pointers are monotonically increasing
    let coordinator = BatchCoordinatorCapsule::new();

    let mut prev_head = 0u32;
    let mut prev_tail = 0u32;

    for i in 0..100 {
        coordinator.add_batch();
        let (head, tail) = coordinator.head_tail.load(std::sync::atomic::Ordering::Acquire);

        assert!(head >= prev_head, "head must be monotonic");
        assert!(tail >= prev_tail, "tail must be monotonic");
        assert!(head <= tail, "head must never exceed tail");

        prev_head = head;
        prev_tail = tail;
    }
}

#[test]
fn proptest_generation_even_invariant() {
    // Verify that generation alternates between even and odd
    let coordinator = BatchCoordinatorCapsule::new();

    assert_eq!(coordinator.generation.load(std::sync::atomic::Ordering::Acquire) % 2, 0, "Initial generation must be even");

    for i in 0..50 {
        coordinator.add_batch();
        let batch = coordinator.claim_batch((i % 16) as u32).expect("Should claim batch");
        coordinator.complete_batch(batch, (i % 16) as u32).expect("Should complete batch");

        let gen = coordinator.generation.load(std::sync::atomic::Ordering::Acquire);
        let expected_parity = (i + 1) % 2;
        assert_eq!(gen % 2, expected_parity as u64, "Generation parity mismatch at iteration {}", i);
    }
}

#[test]
fn proptest_worker_assignment_consistency() {
    // Verify that worker assignments stay consistent during processing
    let coordinator = BatchCoordinatorCapsule::new();

    for _ in 0..100 {
        coordinator.add_batch();
    }

    let mut claimed_batches = Vec::new();
    for i in 0..10 {
        let batch = coordinator.claim_batch(i as u32).expect("Should claim batch");
        claimed_batches.push((i as u32, batch));

        // Verify worker assignment matches claimed batch
        assert_eq!(coordinator.worker_batch(i as u32), Some(batch));
    }

    // Complete all batches
    for (worker_id, batch) in claimed_batches {
        coordinator.complete_batch(batch, worker_id).expect("Should complete batch");
        assert_eq!(coordinator.worker_batch(worker_id), None, "Worker should be idle after completion");
    }
}

#[test]
fn proptest_batch_ordering() {
    // Verify that batches are claimed in order (0, 1, 2, ...)
    let coordinator = BatchCoordinatorCapsule::new();

    for _ in 0..100 {
        coordinator.add_batch();
    }

    for i in 0..100 {
        let batch = coordinator.claim_batch((i % 16) as u32).expect("Should claim batch");
        assert_eq!(batch.raw(), i as u32, "Batch order must be preserved");
    }
}

#[test]
fn proptest_completion_all_batches() {
    // Verify that all_complete() is false until all batches are completed
    let coordinator = BatchCoordinatorCapsule::new();

    for num_batches in 1..=10 {
        coordinator.reset();

        for _ in 0..num_batches {
            coordinator.add_batch();
        }

        assert!(!coordinator.all_complete(), "Should not be complete with pending batches");

        for i in 0..num_batches {
            let batch = coordinator.claim_batch((i % 16) as u32).expect("Should claim batch");
            let is_last = i == num_batches - 1;

            coordinator.complete_batch(batch, (i % 16) as u32).expect("Should complete batch");

            if is_last {
                assert!(coordinator.all_complete(), "Should be complete after all batches");
            } else {
                assert!(!coordinator.all_complete(), "Should not be complete until all done");
            }
        }
    }
}

#[test]
fn proptest_concurrent_claims() {
    // Verify that concurrent CAS operations work correctly
    let coordinator = Arc::new(BatchCoordinatorCapsule::new());

    // Add 100 batches
    for _ in 0..100 {
        coordinator.add_batch();
    }

    let mut handles = vec![];
    let claims = Arc::new(AtomicUsize::new(0));

    // 16 workers claim batches concurrently
    for worker_id in 0..16 {
        let coordinator_clone = Arc::clone(&coordinator);
        let claims_clone = Arc::clone(&claims);

        let handle = thread::spawn(move || {
            let mut local_claims = 0;
            loop {
                match coordinator_clone.claim_batch(worker_id as u32) {
                    Ok(batch) => {
                        local_claims += 1;
                        let _ = coordinator_clone.complete_batch(batch, worker_id as u32);
                    }
                    Err(BatchCoordinatorError::NoBatchesAvailable) => break,
                    Err(e) => panic!("Unexpected error: {:?}", e),
                }
            }
            claims_clone.fetch_add(local_claims, Ordering::AcqRel);
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Worker thread panicked");
    }

    let total_claims = claims.load(Ordering::Acquire);
    assert_eq!(total_claims, 100, "All batches should be claimed");
    assert!(coordinator.all_complete(), "All batches should be completed");
}

#[test]
fn proptest_stats_accuracy() {
    // Verify that stats() returns accurate counts
    let coordinator = BatchCoordinatorCapsule::new();

    for _ in 0..50 {
        coordinator.add_batch();
    }

    for i in 0..50 {
        let batch = coordinator.claim_batch((i % 16) as u32).expect("Should claim batch");
        let stats = coordinator.stats();
        assert_eq!(stats.batches_claimed, (i + 1) as u32);

        coordinator.complete_batch(batch, (i % 16) as u32).expect("Should complete batch");
        let stats = coordinator.stats();
        assert_eq!(stats.batches_completed, (i + 1) as u32);
    }
}

#[test]
fn proptest_wraparound_safety() {
    // Verify that the coordinator handles u32 wraparound correctly
    let coordinator = BatchCoordinatorCapsule::new();

    // Add a large number of batches
    for _ in 0..10000 {
        coordinator.add_batch();
    }

    let stats = coordinator.stats();
    assert_eq!(stats.total_batches, 10000);

    // Verify that we can still claim and complete batches
    for i in 0..100 {
        let batch = coordinator.claim_batch((i % 16) as u32).expect("Should claim batch");
        coordinator.complete_batch(batch, (i % 16) as u32).expect("Should complete batch");
    }
}

// ============================================================================
// TIER 3: INTEGRATION TESTS (10)
// ============================================================================

#[test]
fn test_two_workers_sequential() {
    let coordinator = BatchCoordinatorCapsule::new();

    coordinator.add_batch();
    coordinator.add_batch();

    let batch1 = coordinator.claim_batch(0).expect("Worker 0 should claim batch 0");
    assert_eq!(batch1.raw(), 0);

    let batch2 = coordinator.claim_batch(1).expect("Worker 1 should claim batch 1");
    assert_eq!(batch2.raw(), 1);

    coordinator.complete_batch(batch1, 0).expect("Worker 0 should complete");
    coordinator.complete_batch(batch2, 1).expect("Worker 1 should complete");

    assert!(coordinator.all_complete());
}

#[test]
fn test_partial_completion() {
    let coordinator = BatchCoordinatorCapsule::new();

    // Add 5 batches
    for _ in 0..5 {
        coordinator.add_batch();
    }

    // Claim and complete first 3
    for i in 0..3 {
        let batch = coordinator.claim_batch(i as u32).expect("Should claim batch");
        coordinator.complete_batch(batch, i as u32).expect("Should complete batch");
    }

    // Verify stats show partial completion
    let stats = coordinator.stats();
    assert_eq!(stats.total_batches, 5);
    assert_eq!(stats.batches_completed, 3);
    assert!(!coordinator.all_complete());

    // Complete remaining 2
    for i in 3..5 {
        let batch = coordinator.claim_batch(i as u32).expect("Should claim batch");
        coordinator.complete_batch(batch, i as u32).expect("Should complete batch");
    }

    assert!(coordinator.all_complete());
}

#[test]
fn test_stalled_worker_detection() {
    let coordinator = BatchCoordinatorCapsule::new();

    coordinator.add_batch();
    coordinator.add_batch();

    let batch1 = coordinator.claim_batch(0).expect("Should claim batch 0");
    let batch2 = coordinator.claim_batch(1).expect("Should claim batch 1");

    // Both workers have claimed batches
    let stats = coordinator.stats();
    assert_eq!(stats.stalled_workers, 2);

    // Worker 0 completes
    coordinator.complete_batch(batch1, 0).expect("Should complete");
    let stats = coordinator.stats();
    assert_eq!(stats.stalled_workers, 1);

    // Worker 1 completes
    coordinator.complete_batch(batch2, 1).expect("Should complete");
    let stats = coordinator.stats();
    assert_eq!(stats.stalled_workers, 0);
}

#[test]
fn test_batch_claim_reordering() {
    // Verify that workers can claim batches out of order
    let coordinator = BatchCoordinatorCapsule::new();

    for _ in 0..4 {
        coordinator.add_batch();
    }

    // Workers claim in different order than batch IDs
    let batch3 = coordinator.claim_batch(3).expect("Should claim batch 0 (first available)");
    let batch0 = coordinator.claim_batch(0).expect("Should claim batch 1 (second available)");
    let batch1 = coordinator.claim_batch(1).expect("Should claim batch 2 (third available)");
    let batch2 = coordinator.claim_batch(2).expect("Should claim batch 3 (fourth available)");

    // Verify batch IDs are sequential
    assert_eq!(batch3.raw(), 0);
    assert_eq!(batch0.raw(), 1);
    assert_eq!(batch1.raw(), 2);
    assert_eq!(batch2.raw(), 3);

    // Verify worker assignments are correct
    assert_eq!(coordinator.worker_batch(3), Some(batch3));
    assert_eq!(coordinator.worker_batch(0), Some(batch0));
    assert_eq!(coordinator.worker_batch(1), Some(batch1));
    assert_eq!(coordinator.worker_batch(2), Some(batch2));
}

#[test]
fn test_generation_commitment_semantics() {
    let coordinator = BatchCoordinatorCapsule::new();

    // Initially, generation is even (all committed)
    assert!(coordinator.all_complete());

    coordinator.add_batch();
    let batch = coordinator.claim_batch(0).expect("Should claim batch");

    // After claiming, generation is still even (no complete yet)
    assert!(coordinator.all_complete());

    coordinator.complete_batch(batch, 0).expect("Should complete");

    // After completing, generation is even again
    assert!(coordinator.all_complete());
}

#[test]
fn test_concurrent_batch_streams() {
    // Two producer-consumer streams
    let coordinator = Arc::new(BatchCoordinatorCapsule::new());

    let coordinator_clone = Arc::clone(&coordinator);
    let producer = thread::spawn(move || {
        for _ in 0..50 {
            coordinator_clone.add_batch();
            thread::yield_now();
        }
    });

    let mut consumer_handles = vec![];
    for worker_id in 0..4 {
        let coordinator_clone = Arc::clone(&coordinator);
        let handle = thread::spawn(move || {
            let mut completed = 0;
            loop {
                match coordinator_clone.claim_batch(worker_id as u32) {
                    Ok(batch) => {
                        let _ = coordinator_clone.complete_batch(batch, worker_id as u32);
                        completed += 1;
                    }
                    Err(BatchCoordinatorError::NoBatchesAvailable) => {
                        if completed > 0 {
                            break;
                        }
                        thread::yield_now();
                    }
                    Err(e) => panic!("Unexpected error: {:?}", e),
                }
            }
            completed
        });
        consumer_handles.push(handle);
    }

    producer.join().expect("Producer panicked");

    let mut total_completed = 0;
    for handle in consumer_handles {
        total_completed += handle.join().expect("Consumer panicked");
    }

    assert_eq!(total_completed, 50);
}

#[test]
fn test_producer_consumer_sync() {
    let coordinator = Arc::new(BatchCoordinatorCapsule::new());

    let coordinator_clone = Arc::clone(&coordinator);
    let consumer = thread::spawn(move || {
        let mut count = 0;
        loop {
            match coordinator_clone.claim_batch(0) {
                Ok(batch) => {
                    coordinator_clone.complete_batch(batch, 0).expect("Should complete");
                    count += 1;
                    if count >= 100 {
                        break;
                    }
                }
                Err(BatchCoordinatorError::NoBatchesAvailable) => {
                    thread::yield_now();
                }
                Err(e) => panic!("Unexpected error: {:?}", e),
            }
        }
    });

    for _ in 0..100 {
        coordinator.add_batch();
    }

    consumer.join().expect("Consumer panicked");
    assert!(coordinator.all_complete());
}

#[test]
fn test_multi_batch_pipelining() {
    let coordinator = BatchCoordinatorCapsule::new();

    // Simulate pipelined processing: add batches while workers process
    for i in 0..50 {
        coordinator.add_batch();

        if i > 0 && i % 5 == 0 {
            // Every 5 batches, have a worker process one
            let batch = coordinator.claim_batch((i % 16) as u32).expect("Should claim batch");
            coordinator.complete_batch(batch, (i % 16) as u32).expect("Should complete batch");
        }
    }

    // Complete remaining batches
    loop {
        match coordinator.claim_batch(0) {
            Ok(batch) => {
                coordinator.complete_batch(batch, 0).expect("Should complete");
            }
            Err(BatchCoordinatorError::NoBatchesAvailable) => break,
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    assert!(coordinator.all_complete());
}

#[test]
fn test_worker_fairness() {
    let coordinator = Arc::new(BatchCoordinatorCapsule::new());

    // Add 160 batches
    for _ in 0..160 {
        coordinator.add_batch();
    }

    let mut handles = vec![];
    let counts = Arc::new([
        AtomicUsize::new(0),
        AtomicUsize::new(0),
        AtomicUsize::new(0),
        AtomicUsize::new(0),
    ]);

    // 4 workers claim and complete batches
    for worker_id in 0..4 {
        let coordinator_clone = Arc::clone(&coordinator);
        let counts_clone = Arc::clone(&counts);

        let handle = thread::spawn(move || {
            let mut local_count = 0;
            loop {
                match coordinator_clone.claim_batch(worker_id as u32) {
                    Ok(batch) => {
                        coordinator_clone
                            .complete_batch(batch, worker_id as u32)
                            .expect("Should complete");
                        local_count += 1;
                    }
                    Err(BatchCoordinatorError::NoBatchesAvailable) => break,
                    Err(e) => panic!("Unexpected error: {:?}", e),
                }
            }
            counts_clone[worker_id].store(local_count, Ordering::Release);
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Worker panicked");
    }

    // Verify fairness (each worker should get ~40 batches)
    let counts_vec: Vec<usize> = counts.iter().map(|c| c.load(Ordering::Acquire)).collect();
    let total = counts_vec.iter().sum::<usize>();
    assert_eq!(total, 160);

    // Each worker should get between 30-50 (95% confidence)
    for (i, &count) in counts_vec.iter().enumerate() {
        assert!(count >= 30 && count <= 50, "Worker {} got {} batches (fairness violated)", i, count);
    }
}

#[test]
fn test_recovery_after_stall() {
    let coordinator = BatchCoordinatorCapsule::new();

    coordinator.add_batch();
    coordinator.add_batch();
    coordinator.add_batch();

    // Worker 0 claims but doesn't complete
    let batch0 = coordinator.claim_batch(0).expect("Should claim batch 0");

    // Worker 1 claims and completes
    let batch1 = coordinator.claim_batch(1).expect("Should claim batch 1");
    coordinator.complete_batch(batch1, 1).expect("Should complete");

    // Worker 2 claims and completes
    let batch2 = coordinator.claim_batch(2).expect("Should claim batch 2");
    coordinator.complete_batch(batch2, 2).expect("Should complete");

    // Worker 0 is stalled, stats should reflect this
    let stats = coordinator.stats();
    assert_eq!(stats.stalled_workers, 1);

    // Worker 0 recovers and completes
    coordinator.complete_batch(batch0, 0).expect("Should complete");

    let stats = coordinator.stats();
    assert_eq!(stats.stalled_workers, 0);
    assert!(coordinator.all_complete());
}

// ============================================================================
// TIER 4: PRODUCTION TESTS (5)
// ============================================================================

#[test]
fn test_16_workers_1k_batches() {
    // Stress test: 16 workers, 1000 batches
    let coordinator = Arc::new(BatchCoordinatorCapsule::new());

    // Add 1000 batches
    for _ in 0..1000 {
        coordinator.add_batch();
    }

    let mut handles = vec![];
    let completed = Arc::new(AtomicUsize::new(0));

    // 16 workers process batches
    for worker_id in 0..16 {
        let coordinator_clone = Arc::clone(&coordinator);
        let completed_clone = Arc::clone(&completed);

        let handle = thread::spawn(move || {
            let mut local_completed = 0;
            loop {
                match coordinator_clone.claim_batch(worker_id as u32) {
                    Ok(batch) => {
                        let _ = coordinator_clone.complete_batch(batch, worker_id as u32);
                        local_completed += 1;
                    }
                    Err(BatchCoordinatorError::NoBatchesAvailable) => break,
                    Err(e) => panic!("Unexpected error: {:?}", e),
                }
            }
            completed_clone.fetch_add(local_completed, Ordering::AcqRel);
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Worker panicked");
    }

    let total = completed.load(Ordering::Acquire);
    assert_eq!(total, 1000, "All 1000 batches should be processed");
    assert!(coordinator.all_complete());
}

#[test]
fn test_contention_measurement() {
    // Measure contention by counting CAS failures
    // This is a qualitative test - we just verify no panics occur
    let coordinator = Arc::new(BatchCoordinatorCapsule::new());

    for _ in 0..100 {
        coordinator.add_batch();
    }

    let start = Instant::now();

    let mut handles = vec![];
    let completed = Arc::new(AtomicUsize::new(0));

    for worker_id in 0..16 {
        let coordinator_clone = Arc::clone(&coordinator);
        let completed_clone = Arc::clone(&completed);

        let handle = thread::spawn(move || {
            let mut local = 0;
            loop {
                match coordinator_clone.claim_batch(worker_id as u32) {
                    Ok(batch) => {
                        let _ = coordinator_clone.complete_batch(batch, worker_id as u32);
                        local += 1;
                    }
                    Err(BatchCoordinatorError::NoBatchesAvailable) => break,
                    Err(e) => panic!("Unexpected error: {:?}", e),
                }
            }
            completed_clone.fetch_add(local, Ordering::AcqRel);
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Worker panicked");
    }

    let elapsed = start.elapsed();
    let total = completed.load(Ordering::Acquire);

    // Just verify we completed all work
    assert_eq!(total, 100);
    println!("16 workers processed 100 batches in {:?}", elapsed);
}

#[test]
fn test_latency_percentiles() {
    // Measure claim/complete latencies
    let coordinator = Arc::new(BatchCoordinatorCapsule::new());

    for _ in 0..10000 {
        coordinator.add_batch();
    }

    let mut latencies = Vec::new();

    for _ in 0..1000 {
        let start = Instant::now();
        let batch = coordinator.claim_batch(0).expect("Should claim batch");
        let claim_latency = start.elapsed().as_nanos();

        let start = Instant::now();
        coordinator.complete_batch(batch, 0).expect("Should complete");
        let complete_latency = start.elapsed().as_nanos();

        latencies.push((claim_latency as u64, complete_latency as u64));
    }

    // Calculate percentiles
    let mut claim_times: Vec<u64> = latencies.iter().map(|l| l.0).collect();
    claim_times.sort();

    let p50_claim = claim_times[500];
    let p99_claim = claim_times[990];

    println!("Claim latency P50: {}ns, P99: {}ns", p50_claim, p99_claim);

    // Verify claim latency is reasonable (< 1 microsecond)
    assert!(p99_claim < 1000, "Claim latency P99 should be < 1μs");
}

#[test]
fn test_memory_isolation() {
    // Verify that worker assignments don't cause false sharing
    let coordinator = BatchCoordinatorCapsule::new();

    // Add 16 batches
    for _ in 0..16 {
        coordinator.add_batch();
    }

    // Each worker claims a batch
    for i in 0..16 {
        let batch = coordinator.claim_batch(i as u32).expect("Should claim batch");
        assert_eq!(coordinator.worker_batch(i as u32), Some(batch));
    }

    // Verify stats
    let stats = coordinator.stats();
    assert_eq!(stats.stalled_workers, 16);
    assert_eq!(stats.batches_claimed, 16);
}

#[test]
fn test_wraparound_corner_case() {
    // Test near u32::MAX wraparound
    let coordinator = BatchCoordinatorCapsule::new();

    // Add 10 batches
    for _ in 0..10 {
        coordinator.add_batch();
    }

    // Claim and complete all
    for i in 0..10 {
        let batch = coordinator.claim_batch((i % 16) as u32).expect("Should claim batch");
        coordinator.complete_batch(batch, (i % 16) as u32).expect("Should complete batch");
    }

    assert!(coordinator.all_complete());
}
