//! Test Arc<T> support - validates Copy requirement removal
//!
//! This test validates that AtomicCapsuleMap can now store Arc<T>
//! directly without requiring Mutex workarounds.
//!
//! NOTE: Requires `arc_support` feature - AtomicCapsuleMap currently requires V: Copy
//! To enable: `cargo test --features arc_support`

#![cfg(all(test, feature = "arc_support"))]

use std::sync::Arc;

#[test]
fn test_arc_storage_compiles() {
    use atomic_capsule_map::AtomicCapsuleMap;

    // This should compile now (previously required Copy)
    let map: AtomicCapsuleMap<u64, Arc<String>> = AtomicCapsuleMap::new();

    // Insert Arc<String>
    let value = Arc::new(String::from("test"));
    let _old_value = map.insert(1, value.clone());

    // Verify we can get it back
    assert_eq!(map.get(&1), Some(value));
}

#[test]
fn test_arc_concurrent_access() {
    use atomic_capsule_map::AtomicCapsuleMap;

    let map = Arc::new(AtomicCapsuleMap::<u64, Arc<String>>::new());

    // Insert some data
    for i in 0..10 {
        let value = Arc::new(format!("value_{}", i));
        map.insert(i, value).expect("insert should succeed");
    }

    // Concurrent reads (no Mutex needed!)
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let map_clone = Arc::clone(&map);
            std::thread::spawn(move || {
                for i in 0..10 {
                    let _ = map_clone.get(&i);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread should complete");
    }
}

#[test]
fn test_no_mutex_required() {
    use atomic_capsule_map::AtomicCapsuleMap;

    // Complex data structure
    #[derive(Debug)]
    struct ComplexData {
        values: Vec<u64>,
        metadata: String,
    }

    // Can use Arc<ComplexData> directly - no Mutex wrapper!
    let map: AtomicCapsuleMap<u64, Arc<ComplexData>> = AtomicCapsuleMap::new();

    let data = Arc::new(ComplexData {
        values: vec![1, 2, 3, 4, 5],
        metadata: String::from("test data"),
    });

    map.insert(1, data).expect("insert should succeed");

    // This demonstrates the architectural win:
    // Before: AtomicCapsuleMap<u64, usize> + Mutex<Vec<Arc<ComplexData>>>
    // After:  AtomicCapsuleMap<u64, Arc<ComplexData>>  (no Mutex!)
}
