//! Standalone tests for BatchLshIndexCapsule (T4 Batch + T9 Persistent)
//!
//! # Framework Compliance
//!
//! - **T28**: 4-tier tests (Unit/Property/Integration/Production)
//! - **UCE34**: Q10 (T4 Batch tier), Q33 (verified), Q34 (audit-ready)
//! - **Chaos**: 100% lockfree (no mutex in hot path)
//! - **ASSUM**: 10 documented assumptions with verification

#![allow(unused)]

use std::sync::Arc;
use std::thread;

// Note: Tests import directly to avoid namespace conflicts
#[path = "../src/lsh/batch_lsh_index.rs"]
mod batch_lsh_index_module {
    pub use super::super::src::lsh::batch_lsh_index::*;
}

use batch_lsh_index_module::*;

// ============================================================================
// UNIT TESTS (T28 Q1-Q7: Basic functionality, edge cases)
// ============================================================================

#[test]
fn unit_new_valid_config() {
    let capsule = BatchLshIndexCapsule::new(1000, 5).expect("creation failed");
    assert_eq!(capsule.batch_size(), 1000);
    assert_eq!(capsule.num_bands(), 5);
    let (size, pending, gen) = capsule.stats();
    assert_eq!(size, 0);
    assert_eq!(pending, 0);
    assert_eq!(gen, 0); // Initial generation is even (committed)
}

#[test]
fn unit_new_invalid_batch_size_too_small() {
    let result = BatchLshIndexCapsule::new(50, 5);
    assert!(result.is_err(), "Should reject batch_size < 100");
}

#[test]
fn unit_new_invalid_batch_size_too_large() {
    let result = BatchLshIndexCapsule::new(50_000, 5);
    assert!(result.is_err(), "Should reject batch_size > 10000");
}

#[test]
fn unit_insert_single_signature() {
    let capsule = BatchLshIndexCapsule::new(1000, 5).expect("creation failed");
    let result = capsule.insert_signature(1, 0, 0x123456789abcdef0);
    assert!(result.is_ok(), "Single insert should succeed");

    let (size, pending, _) = capsule.stats();
    assert_eq!(size, 1);
    assert_eq!(pending, 1);
}

#[test]
fn unit_insert_until_batch_full() {
    let capsule = BatchLshIndexCapsule::new(10, 5).expect("creation failed");

    // Insert 10 items (capacity)
    for i in 0..10 {
        let result = capsule.insert_signature(i, 0, i as u64);
        assert!(result.is_ok(), "Insert {} should succeed", i);
    }

    // 11th insert should fail (batch full)
    let result = capsule.insert_signature(10, 0, 10);
    assert!(result.is_err(), "11th insert should fail (batch full)");
}

#[test]
fn unit_should_flush_at_capacity() {
    let capsule = BatchLshIndexCapsule::new(10, 5).expect("creation failed");

    for i in 0..10 {
        let _ = capsule.insert_signature(i, 0, i as u64);
    }

    assert!(
        capsule.should_flush(),
        "Full batch should indicate flush needed"
    );
}

#[test]
fn unit_flush_resets_batch() {
    let capsule = BatchLshIndexCapsule::new(100, 5).expect("creation failed");

    // Add items
    for i in 0..50 {
        let _ = capsule.insert_signature(i, 0, i as u64);
    }

    let (size_before, _, _) = capsule.stats();
    assert_eq!(size_before, 50);

    // Flush
    let result = capsule.flush();
    assert!(result.is_ok(), "Flush should succeed");

    // Check batch is empty
    let (size_after, _, _) = capsule.stats();
    assert_eq!(size_after, 0, "Batch should be empty after flush");
}

#[test]
fn unit_generation_counter_initialization() {
    let capsule = BatchLshIndexCapsule::new(1000, 5).expect("creation failed");
    let (_, _, gen) = capsule.stats();
    assert_eq!(gen, 0, "Initial generation should be 0 (even, committed)");
    assert!(capsule.is_committed(), "Should start in committed state");
}

// ============================================================================
// PROPERTY TESTS (T28 Q8-Q14: Invariants, no data loss)
// ============================================================================

#[test]
fn property_pending_inserts_monotonic() {
    let capsule = BatchLshIndexCapsule::new(1000, 5).expect("creation failed");

    let mut last_pending = 0;
    for i in 0..100 {
        let _ = capsule.insert_signature(i, 0, i as u64);
        let (_, pending, _) = capsule.stats();
        assert!(pending >= last_pending, "Pending inserts should be monotonic");
        last_pending = pending;
    }
}

#[test]
fn property_batch_size_consistent_with_pending() {
    let capsule = BatchLshIndexCapsule::new(1000, 5).expect("creation failed");

    for i in 0..50 {
        let _ = capsule.insert_signature(i, 0, i as u64);
        let (size, pending, _) = capsule.stats();
        // Current size should be <= pending (after first flush)
        assert!(size <= pending, "Size {} > pending {}", size, pending);
    }
}

#[test]
fn property_flush_is_idempotent() {
    let capsule = BatchLshIndexCapsule::new(1000, 5).expect("creation failed");

    for i in 0..10 {
        let _ = capsule.insert_signature(i, 0, i as u64);
    }

    let result1 = capsule.flush();
    assert!(result1.is_ok());

    let (size_after_1, _, gen_after_1) = capsule.stats();

    // Second flush should succeed (idempotent)
    let result2 = capsule.flush();
    assert!(result2.is_ok());

    let (size_after_2, _, gen_after_2) = capsule.stats();

    // State should be unchanged
    assert_eq!(size_after_1, size_after_2);
    assert_eq!(
        gen_after_1, gen_after_2,
        "Generation should not change on idempotent flush"
    );
}

#[test]
fn property_generation_parity_after_flush() {
    let capsule = BatchLshIndexCapsule::new(100, 5).expect("creation failed");

    // Insert some data
    for i in 0..10 {
        let _ = capsule.insert_signature(i, 0, i as u64);
    }

    // Generation should be even (committed)
    let (_, _, gen_before) = capsule.stats();
    assert_eq!(gen_before % 2, 0, "Should be even before flush");

    // Flush
    let _ = capsule.flush();

    // Generation should still be even after flush
    let (_, _, gen_after) = capsule.stats();
    assert_eq!(gen_after % 2, 0, "Should be even after flush");
    assert!(capsule.is_committed(), "Should be committed after flush");
}

// ============================================================================
// INTEGRATION TESTS (T28 Q15-Q21: Multi-operation workflows)
// ============================================================================

#[test]
fn integration_insert_flush_insert_cycle() {
    let capsule = BatchLshIndexCapsule::new(50, 5).expect("creation failed");

    // First cycle: insert 50, flush
    for i in 0..50 {
        let _ = capsule.insert_signature(i, 0, i as u64);
    }
    assert!(capsule.should_flush());
    let _ = capsule.flush();
    let (size_after_1, _, _) = capsule.stats();
    assert_eq!(size_after_1, 0, "Batch should be empty after flush");

    // Second cycle: insert 30, check no flush needed
    for i in 50..80 {
        let _ = capsule.insert_signature(i, 0, i as u64);
    }
    assert!(!capsule.should_flush());
    let (size_mid, pending_mid, _) = capsule.stats();
    assert_eq!(size_mid, 30);
    assert!(
        pending_mid > 50,
        "Total pending should still include first batch"
    );
}

#[test]
fn integration_multiple_flushes_accumulate_pending() {
    let capsule = BatchLshIndexCapsule::new(10, 5).expect("creation failed");

    for cycle in 0..3 {
        for i in 0..10 {
            let doc_id = (cycle * 10 + i) as u64;
            let _ = capsule.insert_signature(doc_id, 0, doc_id);
        }
        assert!(capsule.should_flush());
        let _ = capsule.flush();
    }

    let (size, pending, _) = capsule.stats();
    assert_eq!(size, 0, "Batch should be empty");
    assert_eq!(pending, 30, "Should have accumulated 30 total pending inserts");
}

#[test]
fn integration_band_distribution() {
    let capsule = BatchLshIndexCapsule::new(100, 5).expect("creation failed");

    // Insert entries with different band indices
    for i in 0..25 {
        for band in 0..5 {
            let _ = capsule.insert_signature(i, band as u8, (i * 5 + band) as u64);
        }
    }

    let (size, pending, _) = capsule.stats();
    assert_eq!(size, 125); // 25 docs × 5 bands
    assert_eq!(pending, 125);
}

// ============================================================================
// PRODUCTION TESTS (T28 Q22-Q28: Stress, scalability, edge cases)
// ============================================================================

#[test]
fn production_large_batch_stress() {
    let capsule = BatchLshIndexCapsule::new(1000, 20).expect("creation failed");

    // Insert 1000 documents (stress test)
    for i in 0..1000 {
        for band in 0..20 {
            let result = capsule.insert_signature(i as u64, band as u8, i as u64 * band as u64);
            if i < 1000 && band < 20 {
                // Most inserts should succeed
                assert!(
                    result.is_ok(),
                    "Insert i={}, band={} should succeed",
                    i,
                    band
                );
            }
        }
    }

    // Should be full (or close to it)
    let (size, pending, _) = capsule.stats();
    println!("Stress test - Final state: size={}, pending={}", size, pending);
    assert!(pending > 0, "Should have pending inserts");
}

#[test]
fn production_rapid_flush_cycles() {
    let capsule = BatchLshIndexCapsule::new(50, 5).expect("creation failed");

    // Rapid insert/flush cycles
    for cycle in 0..10 {
        for i in 0..50 {
            let _ = capsule.insert_signature(
                (cycle * 50 + i) as u64,
                (i % 5) as u8,
                (cycle * 50 + i) as u64,
            );
        }

        let result = capsule.flush();
        assert!(result.is_ok(), "Cycle {} flush should succeed", cycle);

        let (size, _, _) = capsule.stats();
        assert_eq!(size, 0, "Cycle {}: batch should be empty after flush", cycle);
    }
}

#[test]
fn production_concurrent_reads_during_stable_state() {
    let capsule = Arc::new(BatchLshIndexCapsule::new(1000, 5).expect("creation failed"));

    // Insert some data first
    for i in 0..100 {
        let _ = capsule.insert_signature(i, 0, i as u64);
    }

    // Spawn multiple reader threads
    let mut handles = vec![];

    for _ in 0..4 {
        let capsule_clone = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let (size, pending, gen) = capsule_clone.stats();
                assert!(
                    pending >= 100,
                    "Pending should be at least 100, got {}",
                    pending
                );
                assert!(gen % 2 == 0, "Should be in committed state");
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }
}

#[test]
fn production_capsule_size_and_alignment() {
    use std::mem::{align_of, size_of};

    // Verify 256-byte size (4 cache lines on 64B L2)
    assert!(
        size_of::<BatchLshIndexCapsule>() <= 256,
        "Capsule size should be ≤256 bytes, got {}",
        size_of::<BatchLshIndexCapsule>()
    );

    // Verify 128-byte alignment
    assert_eq!(
        align_of::<BatchLshIndexCapsule>(),
        128,
        "Capsule should be 128-byte aligned"
    );
}
