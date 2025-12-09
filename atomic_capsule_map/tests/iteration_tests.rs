//! Iteration tests for AtomicCapsuleMap

use atomic_capsule_map::AtomicCapsuleMap;

#[test]
fn test_empty_iteration() {
    let map: AtomicCapsuleMap<u64, i32> = AtomicCapsuleMap::new();

    let count = map.iter().count();
    assert_eq!(count, 0);
}

#[test]
fn test_iteration_basic() {
    let map = AtomicCapsuleMap::new();

    for i in 0..10 {
        map.insert(i, i * 10);
    }

    let items: Vec<_> = map.iter().collect();
    assert_eq!(items.len(), 10);

    // Verify all items are present
    for (key, value) in items {
        assert_eq!(value, key * 10);
    }
}

#[test]
fn test_iteration_preserves_values() {
    let map = AtomicCapsuleMap::new();

    map.insert(1u64, 1);
    map.insert(2u64, 2);
    map.insert(3u64, 3);

    let mut items: Vec<_> = map.iter().collect();
    items.sort_by_key(|(k, _)| *k);

    assert_eq!(items.len(), 3);
    assert_eq!(items[0], (1u64, 1));
    assert_eq!(items[1], (2u64, 2));
    assert_eq!(items[2], (3u64, 3));
}

#[test]
fn test_iteration_after_remove() {
    let map = AtomicCapsuleMap::new();

    for i in 0..10 {
        map.insert(i, i * 10);
    }

    // Remove some items
    map.remove(&2);
    map.remove(&5);
    map.remove(&8);

    let count = map.iter().count();
    assert_eq!(count, 7);

    // Verify removed items are not in iteration
    let keys: Vec<_> = map.iter().map(|(k, _)| k).collect();
    assert!(!keys.contains(&2));
    assert!(!keys.contains(&5));
    assert!(!keys.contains(&8));
}

#[test]
fn test_iteration_snapshot_consistency() {
    let map = AtomicCapsuleMap::new();

    for i in 0..100 {
        map.insert(i, i);
    }

    // Iterate and collect
    let snapshot1: Vec<_> = map.iter().collect();
    let snapshot2: Vec<_> = map.iter().collect();

    // Without concurrent modifications, snapshots should be identical
    assert_eq!(snapshot1.len(), snapshot2.len());
}

#[test]
fn test_multiple_iterations() {
    let map = AtomicCapsuleMap::new();

    for i in 0..10 {
        map.insert(i, i);
    }

    // Multiple iterations should work
    for _ in 0..5 {
        let count = map.iter().count();
        assert_eq!(count, 10);
    }
}

#[test]
fn test_iteration_clone_values() {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    struct Value {
        x: i32,
        y: i32,
    }

    // SAFETY: Value contains only i32 fields which are BitwiseSerializable.
    // All bit patterns are valid for i32.
    unsafe impl atomic_capsule_map::BitwiseSerializable for Value {
        #[inline(always)]
        fn to_storage(self) -> u64 {
            let mut bytes = [0u8; 8];
            unsafe {
                core::ptr::write(bytes.as_mut_ptr() as *mut Value, self);
            }
            u64::from_ne_bytes(bytes)
        }

        #[inline(always)]
        fn from_storage(data: u64) -> Self {
            let bytes = data.to_ne_bytes();
            unsafe { core::ptr::read(bytes.as_ptr() as *const Value) }
        }

        #[inline(always)]
        unsafe fn drop_storage(_data: u64) {
            // No-op for Copy types
        }
    }

    let map = AtomicCapsuleMap::new();

    map.insert(1, Value { x: 10, y: 100 });
    map.insert(2, Value { x: 20, y: 200 });

    let mut items: Vec<_> = map.iter().collect();
    items.sort_by_key(|(k, _)| *k);

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].1.x, 10);
    assert_eq!(items[0].1.y, 100);
    assert_eq!(items[1].1.x, 20);
    assert_eq!(items[1].1.y, 200);
}

#[test]
fn test_iteration_with_clear() {
    let map = AtomicCapsuleMap::new();

    for i in 0..10 {
        map.insert(i, i);
    }

    assert_eq!(map.iter().count(), 10);

    map.clear();

    assert_eq!(map.iter().count(), 0);
}

#[test]
fn test_iteration_functional_operations() {
    let map = AtomicCapsuleMap::new();

    for i in 0..10 {
        map.insert(i, i * 2);
    }

    // Test various iterator methods
    let sum: i32 = map.iter().map(|(_, v)| v).sum();
    assert_eq!(sum, 90); // 0+2+4+6+8+10+12+14+16+18

    let count = map.iter().filter(|(k, _)| k % 2 == 0).count();
    assert_eq!(count, 5);

    let exists = map.iter().any(|(k, v)| k == 5 && v == 10);
    assert!(exists);

    let all_positive = map.iter().all(|(_, v)| v >= 0);
    assert!(all_positive);
}

#[test]
fn test_debug_formatting() {
    let map = AtomicCapsuleMap::new();

    map.insert(1u64, 1);
    map.insert(2u64, 2);

    // Should be able to debug print
    let debug_str = format!("{:?}", map);
    assert!(debug_str.contains("1") || debug_str.contains("2"));
}

#[test]
fn test_clone_map() {
    let map = AtomicCapsuleMap::new();

    for i in 0..10 {
        map.insert(i, i * 10);
    }

    let cloned = map.clone();

    // Both maps should have same content
    assert_eq!(map.len(), cloned.len());

    for i in 0..10 {
        assert_eq!(map.get(&i), cloned.get(&i));
    }

    // Modifications to clone shouldn't affect original
    cloned.insert(100, 1000);
    assert_eq!(map.get(&100), None);
    assert_eq!(cloned.get(&100), Some(1000));
}
