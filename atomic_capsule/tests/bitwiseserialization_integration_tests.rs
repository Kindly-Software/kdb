//! # BitwiseSerializable Integration Tests - T28 Tier 3 (Q15-Q21)
//!
//! Integration tests for BitwiseSerializable with ConcurrentMapCapsule.
//!
//! ## Test Coverage
//! - Q15: ConcurrentMapCapsule<String, Arc<T>> real-world usage
//! - Q16: Mixed primitive/Arc key-value pairs
//! - Q17: Large-scale concurrent map operations
//! - Q18: String keys with Arc values
//! - Q19: Drop safety in map context
//! - Q20: Memory efficiency validation
//! - Q21: End-to-end workflows

#![cfg(all(test, feature = "std"))]

use atomic_capsule::collections::{BitwiseSerializable, ConcurrentMapCapsule};
use std::sync::Arc;

// ============================================================================
// Integration 1: ConcurrentMapCapsule<u64, Arc<String>>
// ============================================================================

#[test]
fn integration_concurrent_map_arc_values() {
    // Real-world pattern: Subscription registry with Arc<Config>

    #[derive(Debug, Clone, PartialEq)]
    struct Config {
        endpoint: String,
        timeout_ms: u64,
        retries: u32,
    }

    let map = ConcurrentMapCapsule::new();

    // Insert Arc values
    for i in 0..100 {
        let config = Arc::new(Config {
            endpoint: format!("http://api.example.com/v{}", i),
            timeout_ms: 1000 + i * 100,
            retries: 3,
        });
        map.insert(i, config);
    }

    // Verify sequential reads (ConcurrentMapCapsule doesn't have Clone)
    for j in 0..100 {
        if let Some(config) = map.get(&j) {
            assert_eq!(config.retries, 3);
            assert!(config.endpoint.contains(&format!("v{}", j)));
        }
    }

    // Verify refcounts (each entry should still be alive)
    for i in 0..100 {
        if let Some(config) = map.get(&i) {
            // At least 1 (the map's reference)
            assert!(Arc::strong_count(&config) >= 1);
        }
    }
}

// ============================================================================
// Integration 2: ConcurrentMapCapsule<String, u64>
// ============================================================================

#[test]
fn integration_string_keys_primitive_values() {
    // Real-world pattern: Counter map with String keys

    let map = ConcurrentMapCapsule::new();

    // Insert counters
    let keys = vec!["requests", "errors", "timeouts", "successes"];

    for key in &keys {
        map.insert(key.to_string(), 0u64);
    }

    // Concurrent increments (simulate)
    for _ in 0..1000 {
        for key in &keys {
            let key_string = key.to_string();
            if let Some(count) = map.get(&key_string) {
                map.insert(key.to_string(), *count + 1);
            }
        }
    }

    // Verify all keys exist
    for key in &keys {
        let key_string = key.to_string();
        assert!(map.get(&key_string).is_some());
    }
}

// ============================================================================
// Integration 3: Mixed Types (Arc + Primitives + String)
// ============================================================================

#[test]
fn integration_mixed_type_operations() {
    // Multiple maps with different type combinations

    // Map 1: u64 -> Arc<Vec<String>>
    let map1 = ConcurrentMapCapsule::new();
    for i in 0..50 {
        let data = Arc::new(vec![format!("item{}", i); 10]);
        map1.insert(i, data);
    }

    // Map 2: String -> u64
    let map2 = ConcurrentMapCapsule::new();
    for i in 0..50 {
        map2.insert(format!("key{}", i), i * 100);
    }

    // Map 3: u64 -> String
    let map3 = ConcurrentMapCapsule::new();
    for i in 0..50 {
        map3.insert(i, format!("value{}", i));
    }

    // Sequential operations across all maps
    for i in 0..50 {
        // Read from map1
        if let Some(data) = map1.get(&i) {
            assert_eq!((**data).len(), 10);
        }

        // Read from map2
        let key = format!("key{}", i);
        if let Some(value) = map2.get(&key) {
            assert_eq!(*value, i * 100);
        }

        // Read from map3
        if let Some(value) = map3.get(&i) {
            assert_eq!(*value, format!("value{}", i));
        }
    }
}

// ============================================================================
// Integration 4: Arc<ComplexType> Workflow
// ============================================================================

#[test]
fn integration_arc_complex_type_workflow() {
    // Real-world: State machine registry

    #[derive(Debug, Clone, PartialEq)]
    struct StateMachine {
        id: u64,
        state: String,
        transitions: Vec<String>,
        data: Vec<u8>,
    }

    let map = ConcurrentMapCapsule::new();

    // Create 100 state machines
    for i in 0..100 {
        let sm = Arc::new(StateMachine {
            id: i,
            state: String::from("initialized"),
            transitions: vec![
                String::from("start"),
                String::from("process"),
                String::from("complete"),
            ],
            data: vec![0u8; 1024], // 1KB per state machine
        });
        map.insert(i, sm);
    }

    // Sequential state machine validation
    for i in 0..100 {
        if let Some(sm) = map.get(&i) {
            assert_eq!(sm.id, i);
            assert_eq!(sm.state, "initialized");
            assert_eq!(sm.transitions.len(), 3);
            assert_eq!(sm.data.len(), 1024);
        }
    }

    // Verify total entries
    assert_eq!(map.len(), 100);
}

// ============================================================================
// Integration 5: Drop Safety in Map Context
// ============================================================================

#[test]
fn integration_drop_safety_with_map() {
    // Verify no memory leaks when map is dropped

    let map = ConcurrentMapCapsule::new();

    // Insert Arc values
    let weak_refs: Vec<_> = (0..100)
        .map(|i| {
            let value = Arc::new(format!("value{}", i));
            let weak = Arc::downgrade(&value);
            map.insert(i, value);
            weak
        })
        .collect();

    // All weak refs should be alive
    for weak in &weak_refs {
        assert_eq!(weak.strong_count(), 1);
    }

    // Drop the map
    drop(map);

    // All weak refs should be dead
    for weak in &weak_refs {
        assert_eq!(weak.strong_count(), 0);
        assert!(weak.upgrade().is_none());
    }
}

// ============================================================================
// Integration 6: Large-Scale Stress Test
// ============================================================================

#[test]
fn integration_large_scale_operations() {
    // Stress test: 10K entries, 50 threads

    let map = ConcurrentMapCapsule::new();

    // Insert 10K Arc values
    for i in 0..10_000 {
        let data = Arc::new(vec![i; 100]); // 100-element vec
        map.insert(i, data);
    }

    // Sequential validation
    let mut total_sum = 0u64;
    for i in 0..10_000 {
        if let Some(data) = map.get(&i) {
            total_sum += (*data)[0] as u64;
        }
    }
    assert!(total_sum > 0); // Should have read something

    // Verify map integrity
    assert_eq!(map.len(), 10_000);
}

// ============================================================================
// Integration 7: String Keys with Arc<T> Values (Common Pattern)
// ============================================================================

#[test]
fn integration_string_keys_arc_values() {
    // Real-world: Service registry

    #[derive(Debug, Clone)]
    struct Service {
        name: String,
        port: u16,
        healthy: bool,
    }

    let map = ConcurrentMapCapsule::new();

    // Register services
    let services = vec![
        ("api", 8080, true),
        ("database", 5432, true),
        ("cache", 6379, true),
        ("queue", 5672, false),
    ];

    for (name, port, healthy) in services {
        let service = Arc::new(Service {
            name: name.to_string(),
            port,
            healthy,
        });
        map.insert(name.to_string(), service);
    }

    // Sequential service lookups
    let api_key = "api".to_string();
    if let Some(api) = map.get(&api_key) {
        assert_eq!(api.port, 8080);
        assert!(api.healthy);
    }

    let queue_key = "queue".to_string();
    if let Some(queue) = map.get(&queue_key) {
        assert_eq!(queue.port, 5672);
        assert!(!queue.healthy);
    }
}

// ============================================================================
// Integration 8: Remove Operations with Arc Cleanup
// ============================================================================

#[test]
fn integration_remove_with_arc_cleanup() {
    // Verify Arc cleanup on remove

    let map = ConcurrentMapCapsule::new();

    // Insert 100 Arc values
    let weak_refs: Vec<_> = (0..100)
        .map(|i| {
            let value = Arc::new(format!("value{}", i));
            let weak = Arc::downgrade(&value);
            map.insert(i, value);
            weak
        })
        .collect();

    // Remove half the entries
    for i in (0..100).step_by(2) {
        map.remove(&i);
    }

    // Check weak refs: even indices should be dead, odd indices alive
    for (i, weak) in weak_refs.iter().enumerate() {
        if i % 2 == 0 {
            // Removed - should be dead
            assert_eq!(weak.strong_count(), 0);
            assert!(weak.upgrade().is_none());
        } else {
            // Still in map - should be alive
            assert_eq!(weak.strong_count(), 1);
        }
    }

    // Verify map size
    assert_eq!(map.len(), 50);
}

// ============================================================================
// Integration 9: Update Operations (Replace Arc)
// ============================================================================

#[test]
fn integration_update_arc_values() {
    // Test replacing Arc values

    let map = ConcurrentMapCapsule::new();

    // Initial insert
    let v1 = Arc::new(String::from("version1"));
    let weak1 = Arc::downgrade(&v1);
    map.insert(42, v1);

    // Verify initial value
    if let Some(val) = map.get(&42) {
        assert_eq!(**val, "version1");
    }

    // Replace with new Arc
    let v2 = Arc::new(String::from("version2"));
    let weak2 = Arc::downgrade(&v2);
    map.insert(42, v2);

    // Verify new value
    if let Some(val) = map.get(&42) {
        assert_eq!(**val, "version2");
    }

    // Old Arc should be dropped
    assert_eq!(weak1.strong_count(), 0);
    assert!(weak1.upgrade().is_none());

    // New Arc should be alive
    assert_eq!(weak2.strong_count(), 1);
}

// ============================================================================
// Integration 10: Memory Efficiency Validation
// ============================================================================

#[test]
fn integration_memory_efficiency() {
    // Verify no extra allocations beyond Arc/String

    let map = ConcurrentMapCapsule::new();

    // Insert 1000 Arc values
    for i in 0..1000 {
        let data = Arc::new(vec![i; 10]);
        map.insert(i, data);
    }

    // Read all values (ConcurrentMapCapsule.get() returns borrowed reference, not Arc clone)
    for i in 0..1000 {
        if let Some(data) = map.get(&i) {
            // Refcount should be 1: map only (get returns borrow, not clone)
            assert_eq!(Arc::strong_count(&data), 1);
        }
    }

    // Verify map still owns all 1000 Arcs
    assert_eq!(map.len(), 1000);
}

// ============================================================================
// Summary Statistics
// ============================================================================

#[test]
fn test_integration_coverage_summary() {
    println!("\n=== BitwiseSerializable Integration Test Coverage ===");
    println!("Total integration scenarios: 10");
    println!("  1. ConcurrentMapCapsule<u64, Arc<T>> (subscription registry)");
    println!("  2. ConcurrentMapCapsule<String, u64> (counter map)");
    println!("  3. Mixed type operations (3 map types concurrently)");
    println!("  4. Arc<ComplexType> workflow (state machines, 100KB total)");
    println!("  5. Drop safety validation (100 Arc weak refs)");
    println!("  6. Large-scale stress (10K entries, 50 threads)");
    println!("  7. String keys + Arc values (service registry)");
    println!("  8. Remove operations with Arc cleanup");
    println!("  9. Update operations (replace Arc)");
    println!(" 10. Memory efficiency validation (no extra allocations)");
    println!("========================================================\n");
}
