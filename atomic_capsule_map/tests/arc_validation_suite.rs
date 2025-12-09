//! Comprehensive Arc<T> validation test suite
//!
//! Tests Arc<T> support with emphasis on safety and correctness.
//! Following ASSUM framework and UCE32 Q30 empirical validation.
//!
//! NOTE: These tests require `arc_support` feature because AtomicCapsuleMap
//! currently requires `V: Copy`, but Arc<T> is Clone-only.
//! To enable: `cargo test --features arc_support`

#![cfg(all(test, feature = "arc_support"))]

use atomic_capsule_map::AtomicCapsuleMap;
use std::sync::Arc;

// =============================================================================
// Test 1: Basic Arc Roundtrip
// =============================================================================

#[test]
fn test_arc_basic_roundtrip() {
    let map: AtomicCapsuleMap<u64, Arc<String>> = AtomicCapsuleMap::new();
    let value = Arc::new(String::from("test"));

    // #ASSUME_ARC_REFCOUNT: Arc::clone increases strong count
    // #VERIFY_NO_LEAK: Refcount validation proves correctness
    assert_eq!(Arc::strong_count(&value), 1);

    map.insert(1, value.clone());

    // After insert, we have 3 references: (original + map storage + snapshot)
    // The snapshot is maintained for iteration support
    assert_eq!(
        Arc::strong_count(&value),
        3,
        "Map should hold references in storage and snapshot"
    );

    let retrieved = map.get(&1).expect("Failed to retrieve value");

    assert_eq!(*retrieved, "test");
    assert_eq!(
        Arc::strong_count(&value),
        4,
        "Retrieved Arc should be fourth reference (original + storage + snapshot + retrieved)"
    );
}

// =============================================================================
// Test 2: Arc Reference Counting Correctness
// =============================================================================

#[test]
fn test_arc_refcount_lifecycle() {
    let map: AtomicCapsuleMap<u64, Arc<Vec<u8>>> = AtomicCapsuleMap::new();
    let data = Arc::new(vec![1, 2, 3, 4, 5]);

    // Initial: 1 reference
    assert_eq!(Arc::strong_count(&data), 1);

    // After insert: 3 references (original + map storage + snapshot)
    map.insert(1, data.clone());
    assert_eq!(Arc::strong_count(&data), 3);

    // After get: 4 references (original + map storage + snapshot + retrieved)
    let retrieved = map.get(&1).unwrap();
    assert_eq!(Arc::strong_count(&data), 4);

    // After drop retrieved: back to 3
    drop(retrieved);
    assert_eq!(Arc::strong_count(&data), 3);

    // After remove: back to 1 (both storage and snapshot cleaned up)
    map.remove(&1).unwrap();
    assert_eq!(Arc::strong_count(&data), 1);

    // Final: only original reference remains
    drop(data);
    // If we reach here without crash, Arc was properly managed
}

// =============================================================================
// Test 3: No Double-Free
// =============================================================================

#[test]
fn test_arc_no_double_free() {
    let map: AtomicCapsuleMap<u64, Arc<String>> = AtomicCapsuleMap::new();

    for i in 0..10 {
        let value = Arc::new(format!("value_{}", i));
        map.insert(i, value);
    }

    // Clear map - each Arc should be dropped exactly once
    map.clear();

    // If we reach here without crash/ASAN error, no double-free occurred
}

// =============================================================================
// Test 4: Concurrent Arc Access (No Mutex!)
// =============================================================================

#[test]
fn test_arc_concurrent_reads() {
    let map = Arc::new(AtomicCapsuleMap::<u64, Arc<String>>::new());
    map.insert(1, Arc::new(String::from("shared")));

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let map_clone = Arc::clone(&map);
            std::thread::spawn(move || {
                for _ in 0..1000 {
                    let val = map_clone.get(&1).unwrap();
                    assert_eq!(*val, "shared");
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}

// =============================================================================
// Test 5: Arc Drop Safety
// =============================================================================

#[test]
fn test_arc_no_leaks() {
    let map: AtomicCapsuleMap<u64, Arc<Vec<u8>>> = AtomicCapsuleMap::new();

    // Insert many Arc values
    for i in 0..100 {
        let data = Arc::new(vec![i as u8; 1024]);
        map.insert(i, data);
    }

    // Clear map - all Arcs should be dropped
    map.clear();

    // Run with:
    // RUSTFLAGS="-Z sanitizer=leak" cargo +nightly test test_arc_no_leaks
    // Should show 0 leaks
}

// =============================================================================
// Test 6: Arc Replacement
// =============================================================================

#[test]
fn test_arc_replacement() {
    let map: AtomicCapsuleMap<u64, Arc<String>> = AtomicCapsuleMap::new();

    let v1 = Arc::new(String::from("v1"));
    let v2 = Arc::new(String::from("v2"));

    // Insert first value (storage + snapshot)
    map.insert(1, v1.clone());
    assert_eq!(Arc::strong_count(&v1), 3);

    // Replace with second value
    map.insert(1, v2.clone());

    // v1 should have been dropped by map (both storage and snapshot)
    assert_eq!(Arc::strong_count(&v1), 1);
    assert_eq!(Arc::strong_count(&v2), 3);

    // Retrieved value should be v2
    let retrieved = map.get(&1).unwrap();
    assert_eq!(*retrieved, "v2");
}

// =============================================================================
// Test 7: Primitive Types Still Work
// =============================================================================

#[test]
fn test_primitives_after_arc_support() {
    let map = AtomicCapsuleMap::<u64, u64>::new();

    for i in 0..100 {
        map.insert(i, i * 2);
    }

    for i in 0..100 {
        assert_eq!(map.get(&i).unwrap(), i * 2);
    }
}

// =============================================================================
// Test 8: Arc<T> with Complex Types
// =============================================================================

#[test]
fn test_arc_complex_types() {
    #[derive(Debug, Clone, PartialEq)]
    struct ComplexData {
        id: u64,
        values: Vec<u64>,
        metadata: String,
    }

    let map: AtomicCapsuleMap<u64, Arc<ComplexData>> = AtomicCapsuleMap::new();

    let data = Arc::new(ComplexData {
        id: 42,
        values: vec![1, 2, 3, 4, 5],
        metadata: String::from("test data"),
    });

    map.insert(1, data.clone());

    let retrieved = map.get(&1).unwrap();
    assert_eq!(retrieved.id, 42);
    assert_eq!(retrieved.values, vec![1, 2, 3, 4, 5]);
    assert_eq!(retrieved.metadata, "test data");
}

// =============================================================================
// Test 9: Arc Strong Count Validation
// =============================================================================

#[test]
fn test_arc_strong_count_correct() {
    let map: AtomicCapsuleMap<u64, Arc<String>> = AtomicCapsuleMap::new();
    let value = Arc::new(String::from("test"));

    // Before insert: 1
    assert_eq!(Arc::strong_count(&value), 1);

    // After insert: 3 (original + storage + snapshot)
    map.insert(1, value.clone());
    assert_eq!(Arc::strong_count(&value), 3);

    // Multiple gets shouldn't permanently increase count
    {
        let _r1 = map.get(&1).unwrap();
        assert_eq!(Arc::strong_count(&value), 4);
        let _r2 = map.get(&1).unwrap();
        assert_eq!(Arc::strong_count(&value), 5);
    }

    // After drops: back to 3 (original + storage + snapshot)
    assert_eq!(Arc::strong_count(&value), 3);
}

// =============================================================================
// Test 10: Arc Map Drop Cleanup
// =============================================================================

#[test]
fn test_arc_map_drop_cleanup() {
    let outer_value = Arc::new(String::from("test"));

    {
        let map: AtomicCapsuleMap<u64, Arc<String>> = AtomicCapsuleMap::new();
        map.insert(1, outer_value.clone());
        assert_eq!(Arc::strong_count(&outer_value), 3); // original + storage + snapshot

        // Map goes out of scope here
    }

    // After map drop, only original reference should remain
    assert_eq!(Arc::strong_count(&outer_value), 1);
}
