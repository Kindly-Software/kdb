//! Basic functionality tests for AtomicCapsuleMap
//!
//! Tests fundamental operations: insert, get, remove, contains_key, len, is_empty, clear

use atomic_capsule_map::AtomicCapsuleMap;

#[test]
fn test_new_map_is_empty() {
    let map: AtomicCapsuleMap<u64, i32> = AtomicCapsuleMap::new();
    assert!(map.is_empty());
    assert_eq!(map.len(), 0);
}

#[test]
fn test_with_capacity() {
    let map: AtomicCapsuleMap<u64, i32> = AtomicCapsuleMap::with_capacity(100);
    assert!(map.is_empty());
    assert_eq!(map.len(), 0);
}

#[test]
fn test_insert_and_get() {
    let map = AtomicCapsuleMap::new();

    // Insert new key
    assert_eq!(map.insert(1, 100), None);

    // Get should return the value
    assert_eq!(map.get(&1), Some(100));

    // Get non-existent key
    assert_eq!(map.get(&2), None);
}

#[test]
fn test_insert_replaces_value() {
    let map = AtomicCapsuleMap::new();

    // First insert
    assert_eq!(map.insert(1, 42), None);

    // Second insert should return old value
    assert_eq!(map.insert(1, 100), Some(42));

    // Get should return new value
    assert_eq!(map.get(&1), Some(100));
}

#[test]
fn test_remove() {
    let map = AtomicCapsuleMap::new();

    map.insert(1, 100);

    // Remove should return the value
    assert_eq!(map.remove(&1), Some(100));

    // Get should now return None
    assert_eq!(map.get(&1), None);

    // Remove again should return None
    assert_eq!(map.remove(&1), None);
}

#[test]
fn test_contains_key() {
    let map = AtomicCapsuleMap::new();

    assert!(!map.contains_key(&1));

    map.insert(1, 42);
    assert!(map.contains_key(&1));

    map.remove(&1);
    assert!(!map.contains_key(&1));
}

#[test]
fn test_len() {
    let map = AtomicCapsuleMap::new();

    assert_eq!(map.len(), 0);

    map.insert(1, 10);
    assert_eq!(map.len(), 1);

    map.insert(2, 20);
    assert_eq!(map.len(), 2);

    map.insert(1, 100); // Replace
    assert_eq!(map.len(), 2);

    map.remove(&1);
    assert_eq!(map.len(), 1);
}

#[test]
fn test_clear() {
    let map = AtomicCapsuleMap::new();

    for i in 0..10 {
        map.insert(i, i * 10);
    }

    assert_eq!(map.len(), 10);

    map.clear();

    assert!(map.is_empty());
    assert_eq!(map.len(), 0);

    // All keys should be gone
    for i in 0..10 {
        assert_eq!(map.get(&i), None);
    }
}

#[test]
fn test_multiple_types() {
    // u32 keys
    let map1: AtomicCapsuleMap<u32, u32> = AtomicCapsuleMap::new();
    map1.insert(1, 100);
    assert_eq!(map1.get(&1), Some(100));

    // i64 keys
    let map2: AtomicCapsuleMap<i64, i64> = AtomicCapsuleMap::new();
    map2.insert(42, 42);
    assert_eq!(map2.get(&42), Some(42));

    // usize keys
    let map3: AtomicCapsuleMap<usize, usize> = AtomicCapsuleMap::new();
    map3.insert(1, 100);
    assert_eq!(map3.get(&1), Some(100));
}

#[test]
fn test_clone_values() {
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Data {
        x: i32,
        y: i32,
    }

    // SAFETY: Data is repr(C) and contains only i32 fields which are BitwiseSerializable.
    // All bit patterns are valid for i32.
    unsafe impl atomic_capsule_map::BitwiseSerializable for Data {
        #[inline(always)]
        fn to_storage(self) -> u64 {
            let mut bytes = [0u8; 8];
            unsafe {
                core::ptr::write(bytes.as_mut_ptr() as *mut Data, self);
            }
            u64::from_ne_bytes(bytes)
        }

        #[inline(always)]
        fn from_storage(data: u64) -> Self {
            let bytes = data.to_ne_bytes();
            unsafe { core::ptr::read(bytes.as_ptr() as *const Data) }
        }

        #[inline(always)]
        unsafe fn drop_storage(_data: u64) {
            // No-op for Copy types
        }
    }

    let map = AtomicCapsuleMap::new();
    let data = Data { x: 42, y: 999 };

    map.insert(1, data);

    let retrieved = map.get(&1).unwrap();
    assert_eq!(retrieved.x, 42);
    assert_eq!(retrieved.y, 999);
}
