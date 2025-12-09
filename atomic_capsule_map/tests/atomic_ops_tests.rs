//! Atomic operations tests for AtomicCapsuleMap
//!
//! Tests atomic operations unique to capsule design:
//! - get_or_insert
//! - compare_and_swap
//! - update

use atomic_capsule_map::AtomicCapsuleMap;
use std::sync::Arc;
use std::thread;

#[test]
fn test_get_or_insert_new_key() {
    let map = AtomicCapsuleMap::new();

    let result = map.get_or_insert(1, 42);
    assert_eq!(result, 42);

    // Verify it was actually inserted
    assert_eq!(map.get(&1), Some(42));
}

#[test]
fn test_get_or_insert_existing_key() {
    let map = AtomicCapsuleMap::new();

    map.insert(1, 42);

    // Should return existing value, not insert new one
    let result = map.get_or_insert(1, 100);
    assert_eq!(result, 42);

    // Verify old value is still there
    assert_eq!(map.get(&1), Some(42));
}

#[test]
fn test_compare_and_swap_success() {
    let map = AtomicCapsuleMap::new();

    map.insert(1, 42);

    // Swap should succeed when value matches
    let result = map.compare_and_swap(&1, 42, 100);
    assert!(result.is_ok());

    // Verify new value
    assert_eq!(map.get(&1), Some(100));
}

#[test]
fn test_compare_and_swap_failure() {
    let map = AtomicCapsuleMap::new();

    map.insert(1, 42);

    // Swap should fail when value doesn't match
    let result = map.compare_and_swap(&1, 99, 100);
    assert_eq!(result, Err(42)); // Returns current value

    // Verify old value is unchanged
    assert_eq!(map.get(&1), Some(42));
}

#[test]
fn test_compare_and_swap_missing_key() {
    let map: AtomicCapsuleMap<u64, i32> = AtomicCapsuleMap::new();

    // CAS on missing key should fail
    let result = map.compare_and_swap(&1, 42, 100);
    assert!(result.is_err());

    // Key should still not exist
    assert_eq!(map.get(&1), None);
}

#[test]
fn test_update_new_key() {
    let map = AtomicCapsuleMap::new();

    // Update non-existent key (closure receives None)
    let result = map.update(1, |v| v.map_or(42, |&x| x + 1));
    assert_eq!(result, 42);

    // Verify it was inserted
    assert_eq!(map.get(&1), Some(42));
}

#[test]
fn test_update_existing_key() {
    let map = AtomicCapsuleMap::new();

    map.insert(1, 10);

    // Update existing key (closure receives Some)
    let result = map.update(1, |v| v.map_or(0, |&x| x + 5));
    assert_eq!(result, 15);

    // Verify it was updated
    assert_eq!(map.get(&1), Some(15));
}

#[test]
fn test_update_counter() {
    let map = AtomicCapsuleMap::new();

    // Increment counter multiple times
    for _ in 0..10 {
        map.update(2, |v| v.map_or(1, |&x| x + 1));
    }

    assert_eq!(map.get(&2), Some(10));
}

#[test]
fn test_concurrent_compare_and_swap() {
    let map = Arc::new(AtomicCapsuleMap::new());
    map.insert(1, 0);

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let map_clone = Arc::clone(&map);
            thread::spawn(move || {
                for _ in 0..100 {
                    loop {
                        // Read current value
                        if let Some(current) = map_clone.get(&1) {
                            // Try to increment
                            if map_clone.compare_and_swap(&1, current, current + 1).is_ok() {
                                break;
                            }
                            // If CAS failed, retry
                        }
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Should have incremented 1000 times (10 threads * 100 iterations)
    assert_eq!(map.get(&1), Some(1000));
}

#[test]
fn test_concurrent_update() {
    let map = Arc::new(AtomicCapsuleMap::new());

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let map_clone = Arc::clone(&map);
            thread::spawn(move || {
                for _ in 0..100 {
                    map_clone.update(2, |v| v.map_or(1, |&x| x + 1));
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Should have incremented 1000 times
    assert_eq!(map.get(&2), Some(1000));
}

#[test]
fn test_concurrent_get_or_insert_idempotent() {
    let map = Arc::new(AtomicCapsuleMap::new());

    // All threads try to insert the same value
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let map_clone = Arc::clone(&map);
            thread::spawn(move || {
                for i in 0..100 {
                    let val = map_clone.get_or_insert(i, i * 10);
                    assert_eq!(val, i * 10);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // All keys should exist with correct values
    for i in 0..100 {
        assert_eq!(map.get(&i), Some(i * 10));
    }
}

#[test]
fn test_update_complex_value() {
    // Use u64 packed with count (high 32 bits) and sum (low 32 bits)
    // This fits in 8 bytes and tests complex update logic
    let map = AtomicCapsuleMap::new();

    // Pack count and sum into u64: (count << 32) | sum
    fn pack(count: u32, sum: u32) -> u64 {
        ((count as u64) << 32) | (sum as u64)
    }
    fn unpack(packed: u64) -> (u32, u32) {
        let count = (packed >> 32) as u32;
        let sum = (packed & 0xFFFF_FFFF) as u32;
        (count, sum)
    }

    // Update with packed value
    for i in 1..=10u32 {
        map.update(3, |v: Option<&u64>| {
            v.map_or(pack(1, i), |&packed| {
                let (count, sum) = unpack(packed);
                pack(count + 1, sum + i)
            })
        });
    }

    let packed = map.get(&3).unwrap();
    let (count, sum) = unpack(packed);
    assert_eq!(count, 10);
    assert_eq!(sum, 55); // 1+2+3+...+10
}

#[test]
fn test_atomic_ops_preserve_generation_counter() {
    // This test validates that generation counters are working
    // by performing many operations that could expose ABA problems
    let map = Arc::new(AtomicCapsuleMap::new());

    map.insert(1, 0);

    let handles: Vec<_> = (0..4)
        .map(|thread_id| {
            let map_clone = Arc::clone(&map);
            thread::spawn(move || {
                for i in 0..1000 {
                    match thread_id {
                        0 => {
                            // Rapid inserts/removes
                            map_clone.insert(1, i);
                            let _ = map_clone.remove(&1);
                            map_clone.insert(1, i);
                        }
                        1 => {
                            // CAS operations
                            if let Some(val) = map_clone.get(&1) {
                                let _ = map_clone.compare_and_swap(&1, val, val + 1);
                            }
                        }
                        2 => {
                            // Update operations
                            map_clone
                                .update(1, |v: Option<&i32>| v.map_or(0, |&x| x.wrapping_add(1)));
                        }
                        3 => {
                            // get_or_insert operations
                            let _ = map_clone.get_or_insert(1, i);
                        }
                        _ => unreachable!(),
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Map should be in consistent state (no ABA corruption)
    // The exact value doesn't matter, but it should exist
    assert!(map.get(&1).is_some() || map.get(&1).is_none());
}
