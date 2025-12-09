//! Health monitoring and circuit breaker tests

use atomic_capsule_map::{AtomicCapsuleMap, BreakerLevel};

#[test]
fn test_health_status_initial() {
    let map: AtomicCapsuleMap<u64, i32> = AtomicCapsuleMap::new();

    let health = map.health_status();

    // Should start at L0 (normal)
    assert_eq!(health.breaker_level, BreakerLevel::L0);
}

#[test]
fn test_set_breaker_level() {
    let map: AtomicCapsuleMap<u64, i32> = AtomicCapsuleMap::new();

    // Set to different levels
    map.set_breaker_level(BreakerLevel::L1);
    assert_eq!(map.health_status().breaker_level, BreakerLevel::L1);

    map.set_breaker_level(BreakerLevel::L2);
    assert_eq!(map.health_status().breaker_level, BreakerLevel::L2);

    map.set_breaker_level(BreakerLevel::L3);
    assert_eq!(map.health_status().breaker_level, BreakerLevel::L3);

    // Back to normal
    map.set_breaker_level(BreakerLevel::L0);
    assert_eq!(map.health_status().breaker_level, BreakerLevel::L0);
}

#[test]
fn test_operations_work_at_all_levels() {
    let map = AtomicCapsuleMap::new();

    for level in [
        BreakerLevel::L0,
        BreakerLevel::L1,
        BreakerLevel::L2,
        BreakerLevel::L3,
    ] {
        map.set_breaker_level(level);

        // All operations should work (degradation is handled internally)
        map.insert(1u64, 42);
        assert_eq!(map.get(&1u64), Some(42));
        assert_eq!(map.remove(&1u64), Some(42));
    }
}

#[test]
fn test_health_check_overhead() {
    let map = AtomicCapsuleMap::new();

    // Insert many items to ensure health checks are happening
    for i in 0..1000 {
        map.insert(i, i * 2);
    }

    // Verify health status is still accessible
    let health = map.health_status();
    assert_eq!(health.breaker_level, BreakerLevel::L0);
}

#[test]
fn test_concurrent_breaker_changes() {
    use std::sync::Arc;
    use std::thread;

    let map = Arc::new(AtomicCapsuleMap::new());

    // Pre-populate
    for i in 0..100 {
        map.insert(i, i);
    }

    let handles: Vec<_> = (0..4)
        .map(|thread_id| {
            let map_clone = Arc::clone(&map);
            thread::spawn(move || {
                for i in 0..100 {
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
                            // Breaker controller
                            let level = match i % 4 {
                                0 => BreakerLevel::L0,
                                1 => BreakerLevel::L1,
                                2 => BreakerLevel::L2,
                                _ => BreakerLevel::L3,
                            };
                            map_clone.set_breaker_level(level);
                        }
                        3 => {
                            // Health checker
                            let _ = map_clone.health_status();
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

    // Map should still be valid
    assert!(map.len() <= 100);
}

#[test]
fn test_breaker_level_ordering() {
    // Verify that breaker levels can be compared
    assert!(BreakerLevel::L0 < BreakerLevel::L1);
    assert!(BreakerLevel::L1 < BreakerLevel::L2);
    assert!(BreakerLevel::L2 < BreakerLevel::L3);

    assert_eq!(BreakerLevel::L0, BreakerLevel::L0);
    assert_ne!(BreakerLevel::L0, BreakerLevel::L1);
}
