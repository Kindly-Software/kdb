//! Concurrent access tests for AtomicCapsuleMap
//!
//! Tests lockfree concurrent operations and validates atomic capsule behavior

use atomic_capsule_map::AtomicCapsuleMap;
use std::sync::Arc;
use std::thread;

#[test]
fn test_concurrent_reads() {
    let map = Arc::new(AtomicCapsuleMap::new());

    // Pre-populate
    for i in 0..100 {
        map.insert(i, i * 10);
    }

    // Spawn multiple readers
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let map_clone = Arc::clone(&map);
            thread::spawn(move || {
                for _ in 0..1000 {
                    for i in 0..100 {
                        let val = map_clone.get(&i);
                        if let Some(v) = val {
                            assert_eq!(v, i * 10);
                        }
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
fn test_concurrent_writes() {
    let map = Arc::new(AtomicCapsuleMap::new());

    // Spawn multiple writers, each writing to different keys
    let handles: Vec<_> = (0..8)
        .map(|thread_id| {
            let map_clone = Arc::clone(&map);
            thread::spawn(move || {
                for i in 0..100 {
                    let key = thread_id * 1000 + i;
                    map_clone.insert(key, key * 2);
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
            let key = thread_id * 1000 + i;
            assert_eq!(map.get(&key), Some(key * 2));
        }
    }

    assert_eq!(map.len(), 800);
}

#[test]
fn test_concurrent_mixed_operations() {
    let map = Arc::new(AtomicCapsuleMap::new());

    // Pre-populate some keys
    for i in 0..50 {
        map.insert(i, i * 10);
    }

    let handles: Vec<_> = (0..4)
        .map(|thread_id| {
            let map_clone = Arc::clone(&map);
            thread::spawn(move || {
                for i in 0..200 {
                    match thread_id {
                        0 => {
                            // Reader
                            let _ = map_clone.get(&(i % 100));
                        }
                        1 => {
                            // Writer
                            map_clone.insert(i % 100, i);
                        }
                        2 => {
                            // Remover
                            let _ = map_clone.remove(&(i % 100));
                        }
                        3 => {
                            // Mixed
                            if i % 2 == 0 {
                                map_clone.insert(i % 100, i);
                            } else {
                                let _ = map_clone.get(&(i % 100));
                            }
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

    // Map should still be valid (no corruption)
    // We can't assert exact contents due to race conditions, but operations should succeed
    assert!(map.len() <= 100);
}

#[test]
fn test_concurrent_insert_same_key() {
    let map = Arc::new(AtomicCapsuleMap::new());
    let key = 42u64;

    // Multiple threads trying to insert the same key
    let handles: Vec<_> = (0..10)
        .map(|thread_id| {
            let map_clone = Arc::clone(&map);
            thread::spawn(move || {
                for _ in 0..100 {
                    map_clone.insert(key, thread_id);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Key should have some value (last writer wins)
    assert!(map.get(&key).is_some());
    assert_eq!(map.len(), 1);
}

#[test]
fn test_concurrent_get_or_insert() {
    let map = Arc::new(AtomicCapsuleMap::new());

    let handles: Vec<_> = (0..8)
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

    // All keys should be present exactly once
    assert_eq!(map.len(), 100);
    for i in 0..100 {
        assert_eq!(map.get(&i), Some(i * 10));
    }
}

#[test]
fn test_concurrent_remove_same_key() {
    let map = Arc::new(AtomicCapsuleMap::new());
    let key = 99u64;

    map.insert(key, 42);

    // Multiple threads trying to remove the same key
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let map_clone = Arc::clone(&map);
            thread::spawn(move || map_clone.remove(&key))
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Exactly one thread should have gotten the value
    let successful_removes = results.iter().filter(|r| r.is_some()).count();
    assert!(successful_removes <= 1); // At most one thread should succeed

    // Key should be gone
    assert_eq!(map.get(&key), None);
}

#[test]
fn test_concurrent_iteration() {
    let map = Arc::new(AtomicCapsuleMap::new());

    // Populate map
    for i in 0..100 {
        map.insert(i, i * 2);
    }

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let map_clone = Arc::clone(&map);
            thread::spawn(move || {
                // Iterate while other threads might be modifying
                let _count: usize = map_clone.iter().count();
            })
        })
        .collect();

    // Concurrent modifications
    for i in 100..150 {
        map.insert(i, i * 2);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_stress_concurrent_operations() {
    let map = Arc::new(AtomicCapsuleMap::new());
    let iterations = 10000;

    let handles: Vec<_> = (0..8)
        .map(|thread_id| {
            let map_clone = Arc::clone(&map);
            thread::spawn(move || {
                for i in 0..iterations {
                    let key = (thread_id * iterations + i) % 1000;

                    match i % 5 {
                        0 => {
                            map_clone.insert(key, i);
                        }
                        1 => {
                            let _ = map_clone.get(&key);
                        }
                        2 => {
                            let _ = map_clone.remove(&key);
                        }
                        3 => {
                            let _ = map_clone.get_or_insert(key, i);
                        }
                        4 => {
                            let _ = map_clone.contains_key(&key);
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

    // Map should be in valid state (no corruption)
    assert!(map.len() <= 1000);
}
