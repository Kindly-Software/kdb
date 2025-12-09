//! Arc<T> runtime support tests
//!
//! Tests that Arc<T> reference counting works correctly with AtomicCapsuleMap.
//! This test ensures no memory leaks, no double-free, and concurrent access works.
//!
//! NOTE: Requires `arc_support` feature - AtomicCapsuleMap currently requires V: Copy
//! To enable: `cargo test --features arc_support`

#![cfg(all(test, feature = "arc_support"))]

use atomic_capsule_map::AtomicCapsuleMap;
use std::sync::Arc;

#[test]
fn test_arc_string_roundtrip() {
    let map = AtomicCapsuleMap::<u64, Arc<String>>::with_capacity(16);

    // Insert Arc<String>
    let data = Arc::new(String::from("Hello, Arc!"));
    let refcount_before = Arc::strong_count(&data);

    map.insert(42, data.clone()).unwrap();

    // Get should return cloned Arc with incremented refcount
    let retrieved = map.get(&42).expect("Key should exist");
    assert_eq!(*retrieved, "Hello, Arc!");

    // Verify refcount increased (original + clone in map + retrieved)
    let refcount_after = Arc::strong_count(&data);
    assert!(
        refcount_after > refcount_before,
        "Arc refcount should increase"
    );
}

#[test]
fn test_arc_no_leak() {
    let map = AtomicCapsuleMap::<u64, Arc<String>>::with_capacity(16);

    let data = Arc::new(String::from("No leak test"));
    let weak = Arc::downgrade(&data);

    map.insert(1, data.clone()).unwrap();
    drop(data); // Drop original reference

    // Weak pointer should still be valid (Arc in map keeps it alive)
    assert!(weak.upgrade().is_some(), "Arc should be alive in map");

    // Remove from map
    map.remove(&1).unwrap();

    // Now weak pointer should be invalid
    assert!(
        weak.upgrade().is_none(),
        "Arc should be dropped after removal"
    );
}

#[test]
fn test_arc_concurrent_access() {
    use std::thread;

    let map = Arc::new(AtomicCapsuleMap::<u64, Arc<String>>::with_capacity(256));

    // Insert initial Arc data
    for i in 0..10u64 {
        let data = Arc::new(format!("Value {}", i));
        map.insert(i, data).unwrap();
    }

    // Spawn multiple readers
    let mut handles = vec![];
    for _ in 0..4 {
        let map_clone = Arc::clone(&map);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                for i in 0..10u64 {
                    let value = map_clone.get(&i).expect("Key should exist");
                    assert_eq!(*value, format!("Value {}", i));
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_arc_update_refcount() {
    let map = AtomicCapsuleMap::<u64, Arc<String>>::with_capacity(16);

    let data1 = Arc::new(String::from("First"));
    let data2 = Arc::new(String::from("Second"));

    map.insert(1, data1.clone()).unwrap();
    let refcount1 = Arc::strong_count(&data1);

    // Update with new Arc
    map.insert(1, data2.clone()).unwrap();

    // Old Arc refcount should decrease (only original reference remains)
    assert_eq!(
        Arc::strong_count(&data1),
        refcount1 - 1,
        "Old Arc should lose reference"
    );

    // New Arc refcount should increase
    assert_eq!(
        Arc::strong_count(&data2),
        2,
        "New Arc should have 2 refs (original + map)"
    );
}
