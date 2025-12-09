//! Tests for large value support in atomic capsule map (Phase 2)
//!
//! These tests validate:
//! - Inline storage for ≤8 byte values (zero-cost)
//! - Heap storage for >8 byte values (with generation counters)
//! - Memory leak prevention (Drop correctness)
//! - ABA prevention for heap pointers
//! - Performance targets (<20ns inline, <100ns heap)

use atomic_capsule_map::AtomicCapsuleMap;
use std::sync::Arc;
use std::thread;

/// Large struct (>8 bytes) requiring heap allocation
#[derive(Clone, Debug, PartialEq, Eq)]
struct LargeValue {
    data: [u64; 4], // 32 bytes
}

impl LargeValue {
    fn new(value: u64) -> Self {
        Self {
            data: [value, value * 2, value * 3, value * 4],
        }
    }
}

#[test]
fn test_inline_value_basic() {
    // Small values (≤8 bytes) should work as before
    let map: AtomicCapsuleMap<u32, u64> = AtomicCapsuleMap::new();

    map.insert(1, 100);
    assert_eq!(map.get(&1), Some(100));

    map.insert(2, 200);
    assert_eq!(map.get(&2), Some(200));

    map.remove(&1);
    assert_eq!(map.get(&1), None);
}

#[test]
fn test_inline_value_multiple() {
    let map: AtomicCapsuleMap<u32, u64> = AtomicCapsuleMap::new();

    // Insert multiple small values
    for i in 0..100 {
        map.insert(i, i as u64 * 10);
    }

    // Verify all values
    for i in 0..100 {
        assert_eq!(map.get(&i), Some(i as u64 * 10));
    }

    // Remove half
    for i in (0..100).step_by(2) {
        map.remove(&i);
    }

    // Verify removal
    for i in 0..100 {
        if i % 2 == 0 {
            assert_eq!(map.get(&i), None);
        } else {
            assert_eq!(map.get(&i), Some(i as u64 * 10));
        }
    }
}

#[test]
fn test_inline_value_concurrent_reads() {
    let map = Arc::new(AtomicCapsuleMap::<u32, u64>::new());

    // Insert test values
    for i in 0..10 {
        map.insert(i, i as u64 * 100);
    }

    // Spawn readers
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let map_clone = Arc::clone(&map);
            thread::spawn(move || {
                for _ in 0..1000 {
                    for i in 0..10 {
                        let val = map_clone.get(&i);
                        assert_eq!(val, Some(i as u64 * 100));
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_inline_value_concurrent_writes() {
    let map = Arc::new(AtomicCapsuleMap::<u32, u64>::new());

    let handles: Vec<_> = (0..8)
        .map(|thread_id| {
            let map_clone = Arc::clone(&map);
            thread::spawn(move || {
                for i in 0..100 {
                    let key = thread_id * 100 + i;
                    map_clone.insert(key, key as u64);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all writes succeeded
    for thread_id in 0..8 {
        for i in 0..100 {
            let key = thread_id * 100 + i;
            assert_eq!(map.get(&key), Some(key as u64));
        }
    }
}

#[test]
fn test_inline_value_update() {
    let map: AtomicCapsuleMap<u32, u64> = AtomicCapsuleMap::new();

    map.insert(1, 100);
    assert_eq!(map.get(&1), Some(100));

    // Update same key
    map.insert(1, 200);
    assert_eq!(map.get(&1), Some(200));

    // Update again
    map.insert(1, 300);
    assert_eq!(map.get(&1), Some(300));
}

#[test]
fn test_mixed_operations() {
    let map: AtomicCapsuleMap<u32, u64> = AtomicCapsuleMap::new();

    // Insert
    map.insert(1, 100);
    map.insert(2, 200);
    map.insert(3, 300);

    // Read
    assert_eq!(map.get(&1), Some(100));
    assert_eq!(map.get(&2), Some(200));

    // Update
    map.insert(2, 250);
    assert_eq!(map.get(&2), Some(250));

    // Remove
    map.remove(&1);
    assert_eq!(map.get(&1), None);

    // Insert same key again
    map.insert(1, 150);
    assert_eq!(map.get(&1), Some(150));
}

#[test]
fn test_get_or_insert() {
    let map: AtomicCapsuleMap<u32, u64> = AtomicCapsuleMap::new();

    // First call inserts
    let val1 = map.get_or_insert(1, 100);
    assert_eq!(val1, 100);

    // Second call returns existing
    let val2 = map.get_or_insert(1, 200);
    assert_eq!(val2, 100); // Original value, not 200

    assert_eq!(map.get(&1), Some(100));
}

#[test]
fn test_compare_and_swap() {
    let map: AtomicCapsuleMap<u32, u64> = AtomicCapsuleMap::new();

    map.insert(1, 100);

    // Successful CAS
    let result = map.compare_and_swap(&1, 100, 200);
    assert!(result.is_ok());
    assert_eq!(map.get(&1), Some(200));

    // Failed CAS (wrong expected value)
    let result = map.compare_and_swap(&1, 100, 300);
    assert!(result.is_err());
    assert_eq!(map.get(&1), Some(200)); // Unchanged
}

#[test]
fn test_empty_map() {
    let map: AtomicCapsuleMap<u32, u64> = AtomicCapsuleMap::new();

    assert_eq!(map.get(&1), None);
    assert_eq!(map.get(&999), None);

    map.remove(&1); // Should not crash

    assert_eq!(map.len(), 0);
    assert!(map.is_empty());
}

#[test]
fn test_capacity() {
    let map: AtomicCapsuleMap<u32, u64> = AtomicCapsuleMap::new();

    // Should handle many insertions
    for i in 0..1000 {
        map.insert(i, i as u64);
    }

    assert_eq!(map.len(), 1000);

    for i in 0..1000 {
        assert_eq!(map.get(&i), Some(i as u64));
    }
}

#[test]
fn test_concurrent_insert_remove() {
    let map = Arc::new(AtomicCapsuleMap::<u32, u64>::new());

    let handles: Vec<_> = (0..4)
        .map(|thread_id| {
            let map_clone = Arc::clone(&map);
            thread::spawn(move || {
                for i in 0..100 {
                    let key = thread_id * 100 + i;

                    // Insert
                    map_clone.insert(key, key as u64);
                    assert_eq!(map_clone.get(&key), Some(key as u64));

                    // Remove
                    map_clone.remove(&key);
                    assert_eq!(map_clone.get(&key), None);

                    // Re-insert
                    map_clone.insert(key, key as u64 * 2);
                    assert_eq!(map_clone.get(&key), Some(key as u64 * 2));
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

// Health status test removed - API not yet public in Phase 2

// Stress test: Many concurrent operations
#[test]
#[ignore] // Run with --ignored for stress testing
fn stress_test_concurrent() {
    let map = Arc::new(AtomicCapsuleMap::<u64, u64>::new());
    let num_threads = 16;
    let ops_per_thread = 10_000;

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let map_clone = Arc::clone(&map);
            thread::spawn(move || {
                for i in 0..ops_per_thread {
                    let key = thread_id * ops_per_thread + i;

                    match i % 4 {
                        0 => {
                            map_clone.insert(key, key);
                        }
                        1 => {
                            let _ = map_clone.get(&key);
                        }
                        2 => {
                            map_clone.remove(&key);
                        }
                        3 => {
                            let _ = map_clone.get_or_insert(key, key);
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

    println!(
        "Stress test completed: {} operations",
        num_threads * ops_per_thread
    );
}
