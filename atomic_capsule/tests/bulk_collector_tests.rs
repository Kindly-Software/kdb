//! T28 Tests for BulkCollectorCapsule
//!
//! Test tiers:
//! - Unit (Q1-Q7): Basic operations, bounds, capacity
//! - Property (Q8-Q14): Concurrent append, monotonic position
//! - Integration (Q15-Q21): Export Arc/slice, reset reuse
//! - Production (Q22-Q28): Benchmarks (see benches/)

#![cfg(feature = "bulk-collector")]

use atomic_capsule::collections::{BulkCollectorCapsule, BulkCollectorError};
use std::sync::Arc;
use std::thread;

// ============================================================================
// T28 Unit Tests (Q1-Q7)
// ============================================================================

#[test]
fn test_new() {
    let collector = BulkCollectorCapsule::<u64>::new(100);
    assert_eq!(collector.len(), 0);
    assert_eq!(collector.capacity(), 100);
    assert!(collector.is_empty());
    assert!(!collector.is_full());
}

#[test]
#[should_panic(expected = "capacity must be > 0")]
fn test_new_zero_capacity_panics() {
    let _collector = BulkCollectorCapsule::<u64>::new(0);
}

#[test]
fn test_record_basic() {
    let collector = BulkCollectorCapsule::<u64>::new(10);

    for i in 0..10 {
        collector.record(i as u64).unwrap();
    }

    assert_eq!(collector.len(), 10);
    assert!(collector.is_full());
}

#[test]
fn test_record_overflow() {
    let collector = BulkCollectorCapsule::<u64>::new(5);

    // Fill to capacity
    for i in 0..5 {
        collector.record(i as u64).unwrap();
    }

    // Overflow
    let result = collector.record(999);
    assert!(matches!(
        result,
        Err(BulkCollectorError::CapacityExceeded {
            capacity: 5,
            index: 5
        })
    ));

    // Length should remain at capacity (not exceed)
    assert_eq!(collector.len(), 5);
}

#[test]
fn test_view() {
    let collector = BulkCollectorCapsule::<u64>::new(100);

    for i in 0..10 {
        collector.record(i as u64).unwrap();
    }

    let view = collector.view();
    assert_eq!(view.len(), 10);
    for i in 0..10 {
        assert_eq!(view[i], i as u64);
    }
}

#[test]
fn test_export_arc() {
    let collector = BulkCollectorCapsule::<u64>::new(100);

    for i in 0..10 {
        collector.record(i as u64).unwrap();
    }

    let arc = collector.export_arc();
    assert_eq!(arc.len(), 10);
    for i in 0..10 {
        assert_eq!(arc[i], i as u64);
    }
}

#[test]
fn test_reset() {
    let collector = BulkCollectorCapsule::<u64>::new(100);

    collector.record(42u64).unwrap();
    assert_eq!(collector.len(), 1);

    let gen1 = collector.generation();
    collector.reset();

    assert_eq!(collector.len(), 0);
    assert!(collector.is_empty());
    assert_eq!(collector.generation(), gen1 + 1);

    // Reuse after reset
    collector.record(999u64).unwrap();
    assert_eq!(collector.len(), 1);
    assert_eq!(collector.view()[0], 999u64);
}

// ============================================================================
// T28 Property Tests (Q8-Q14)
// ============================================================================

#[test]
fn test_concurrent_append_safety() {
    let collector = Arc::new(BulkCollectorCapsule::<u64>::new(10_000));
    let num_threads = 8;
    let items_per_thread = 1_000;

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let collector = Arc::clone(&collector);
            thread::spawn(move || {
                for i in 0..items_per_thread {
                    let value = (thread_id * 10_000) + i;
                    collector.record(value as u64).unwrap();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(collector.len(), num_threads * items_per_thread);

    // Verify all values present (order not guaranteed, but uniqueness is)
    let data = collector.export_arc();
    let mut sorted: Vec<u64> = data.iter().copied().collect();
    sorted.sort_unstable();

    let mut expected = Vec::new();
    for thread_id in 0..num_threads {
        for i in 0..items_per_thread {
            expected.push((thread_id * 10_000 + i) as u64);
        }
    }
    expected.sort_unstable();

    assert_eq!(sorted, expected);
}

#[test]
fn test_monotonic_position() {
    let collector = BulkCollectorCapsule::<u64>::new(1000);

    let mut prev_len = 0;
    for i in 0..1000 {
        collector.record(i).unwrap();
        let current_len = collector.len();
        assert!(
            current_len > prev_len,
            "Position must be monotonically increasing"
        );
        prev_len = current_len;
    }
}

#[test]
fn test_concurrent_read_during_write() {
    let collector = Arc::new(BulkCollectorCapsule::<u64>::new(10_000));

    // Writer thread
    let writer_collector = Arc::clone(&collector);
    let writer = thread::spawn(move || {
        for i in 0..5_000 {
            writer_collector.record(i).unwrap();
        }
    });

    // Reader thread (reads length during writes)
    let reader_collector = Arc::clone(&collector);
    let reader = thread::spawn(move || {
        let mut prev_len = 0;
        for _ in 0..100 {
            let len = reader_collector.len();
            assert!(
                len >= prev_len,
                "Length must be monotonic even during concurrent writes"
            );
            prev_len = len;
            std::thread::sleep(std::time::Duration::from_micros(10));
        }
    });

    writer.join().unwrap();
    reader.join().unwrap();
}

// ============================================================================
// T28 Integration Tests (Q15-Q21)
// ============================================================================

#[test]
fn test_export_arc_zero_copy() {
    let collector = BulkCollectorCapsule::<u64>::new(100);

    for i in 0..50 {
        collector.record(i).unwrap();
    }

    let arc1 = collector.export_arc();
    let arc2 = collector.export_arc();

    // Both Arcs should have same data
    assert_eq!(arc1.len(), 50);
    assert_eq!(arc2.len(), 50);
    assert_eq!(arc1[0], arc2[0]);
}

#[test]
fn test_view_lifetime() {
    let collector = BulkCollectorCapsule::<u64>::new(100);
    collector.record(42).unwrap();

    {
        let view = collector.view();
        assert_eq!(view[0], 42);
        // view dropped here
    }

    // Collector still usable after view dropped
    collector.record(999).unwrap();
    assert_eq!(collector.len(), 2);
}

#[test]
fn test_reset_multi_phase() {
    let collector = BulkCollectorCapsule::<u64>::new(100);

    // Phase 1
    for i in 0..10 {
        collector.record(i).unwrap();
    }
    let phase1_data = collector.export_arc();
    assert_eq!(phase1_data.len(), 10);

    collector.reset();

    // Phase 2
    for i in 100..110 {
        collector.record(i).unwrap();
    }
    let phase2_data = collector.export_arc();
    assert_eq!(phase2_data.len(), 10);
    assert_eq!(phase2_data[0], 100); // Not 0 from phase 1
}

#[test]
fn test_large_capacity() {
    let collector = BulkCollectorCapsule::<u64>::new(100_000);

    for i in 0..100_000 {
        collector.record(i).unwrap();
    }

    assert_eq!(collector.len(), 100_000);
    assert!(collector.is_full());

    let data = collector.export_arc();
    assert_eq!(data[0], 0);
    assert_eq!(data[99_999], 99_999);
}

// ============================================================================
// Type Tests (Generic support)
// ============================================================================

#[test]
fn test_generic_types() {
    // u8
    let collector_u8 = BulkCollectorCapsule::<u8>::new(10);
    collector_u8.record(255).unwrap();
    assert_eq!(collector_u8.view()[0], 255);

    // u128
    let collector_u128 = BulkCollectorCapsule::<u128>::new(10);
    collector_u128.record(u128::MAX).unwrap();
    assert_eq!(collector_u128.view()[0], u128::MAX);

    // [u16; 128] (MinHashSig use case)
    let collector_array = BulkCollectorCapsule::<[u16; 128]>::new(10);
    let sig = [42u16; 128];
    collector_array.record(sig).unwrap();
    assert_eq!(collector_array.view()[0], sig);
}

#[test]
fn test_debug_impl() {
    let collector = BulkCollectorCapsule::<u64>::new(100);
    collector.record(42).unwrap();

    let debug_str = format!("{:?}", collector);
    assert!(debug_str.contains("position"));
    assert!(debug_str.contains("capacity"));
}
