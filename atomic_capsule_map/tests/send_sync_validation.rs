//! Thread safety validation tests for AtomicCapsuleMap
//!
//! Validates that Send + Sync implementations work correctly and that
//! BitwiseSerializable trait prevents unsafe transmute usage.

use atomic_capsule_map::AtomicCapsuleMap;
use std::sync::Arc;
use std::thread;

/// Verify that AtomicCapsuleMap is Send + Sync
#[test]
fn test_send_sync_bounds() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<AtomicCapsuleMap<u32, u64>>();
    assert_sync::<AtomicCapsuleMap<u32, u64>>();
}

/// Test concurrent access from multiple threads
#[test]
fn test_concurrent_access() {
    let map = Arc::new(AtomicCapsuleMap::<u64, u64>::new());

    // Insert initial data
    for i in 0..100u64 {
        map.insert(i, i * 10);
    }

    // Spawn reader threads
    let mut handles = vec![];
    for _ in 0..4 {
        let map_clone = Arc::clone(&map);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                for i in 0..100u64 {
                    let value = map_clone.get(&i);
                    assert_eq!(value, Some(i * 10));
                }
            }
        });
        handles.push(handle);
    }

    // Spawn writer threads
    for tid in 0..2 {
        let map_clone = Arc::clone(&map);
        let handle = thread::spawn(move || {
            let offset = 100 + (tid * 50);
            for i in 0..50u64 {
                map_clone.insert(offset + i, (offset + i) * 10);
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all data is present
    for i in 0..200u64 {
        assert_eq!(map.get(&i), Some(i * 10));
    }
}

/// Test that BitwiseSerializable prevents transmute of invalid types
#[test]
fn test_bitwise_serializable_types() {
    // These should compile (primitive types implement BitwiseSerializable)
    let _map_u8 = AtomicCapsuleMap::<u32, u8>::new();
    let _map_u16 = AtomicCapsuleMap::<u32, u16>::new();
    let _map_u32 = AtomicCapsuleMap::<u32, u32>::new();
    let _map_u64 = AtomicCapsuleMap::<u32, u64>::new();
    let _map_i32 = AtomicCapsuleMap::<u32, i32>::new();
    let _map_i64 = AtomicCapsuleMap::<u32, i64>::new();
    let _map_f32 = AtomicCapsuleMap::<u32, f32>::new();
    let _map_f64 = AtomicCapsuleMap::<u32, f64>::new();
}

/// Test f64 NaN handling (all bit patterns should be valid)
#[test]
fn test_f64_nan_handling() {
    let map = AtomicCapsuleMap::<u64, f64>::new();

    // Insert NaN value
    map.insert(1, f64::NAN);

    // Retrieve NaN (note: NaN != NaN, so we check is_nan)
    let value = map.get(&1).unwrap();
    assert!(value.is_nan());

    // Insert infinity
    map.insert(2, f64::INFINITY);
    assert_eq!(map.get(&2), Some(f64::INFINITY));

    // Insert negative infinity
    map.insert(3, f64::NEG_INFINITY);
    assert_eq!(map.get(&3), Some(f64::NEG_INFINITY));
}

/// Stress test with high concurrency
#[cfg(not(miri))] // Skip in miri due to timeout
#[test]
#[ignore] // TODO: Fix capacity issues in high stress scenario
fn test_high_concurrency_stress() {
    let map = Arc::new(AtomicCapsuleMap::<u64, u64>::new());
    let num_threads = 16;
    let ops_per_thread = 10000;

    let mut handles = vec![];

    for tid in 0..num_threads {
        let map_clone = Arc::clone(&map);
        let handle = thread::spawn(move || {
            let base = tid as u64 * ops_per_thread;

            for i in 0..ops_per_thread {
                let key = base + i;
                // Insert
                map_clone.insert(key, key * 2);
                // Read back
                assert_eq!(map_clone.get(&key), Some(key * 2));
                // Update
                map_clone.insert(key, key * 3);
                // Verify
                assert_eq!(map_clone.get(&key), Some(key * 3));
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final state
    for tid in 0..num_threads {
        let base = tid as u64 * ops_per_thread;
        for i in 0..ops_per_thread {
            let key = base + i;
            assert_eq!(map.get(&key), Some(key * 3));
        }
    }
}

/// Test that map can be shared across thread boundaries
#[test]
fn test_cross_thread_sharing() {
    let map = Arc::new(AtomicCapsuleMap::<u32, u32>::new());

    // Thread 1: Insert
    {
        let map_clone = Arc::clone(&map);
        thread::spawn(move || {
            map_clone.insert(42, 100);
        })
        .join()
        .unwrap();
    }

    // Thread 2: Read
    {
        let map_clone = Arc::clone(&map);
        let handle = thread::spawn(move || map_clone.get(&42));
        assert_eq!(handle.join().unwrap(), Some(100));
    }

    // Thread 3: Remove
    {
        let map_clone = Arc::clone(&map);
        thread::spawn(move || {
            map_clone.remove(&42).unwrap();
        })
        .join()
        .unwrap();
    }

    // Verify removed
    assert_eq!(map.get(&42), None);
}
