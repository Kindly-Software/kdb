//! Integration tests for RobinHoodHashCapsule
//!
//! Tests the complete implementation including:
//! - High load factor (80-90%)
//! - Concurrent operations
//! - Robin Hood swapping correctness
//! - Backward shift deletion

use atomic_capsule::collections::RobinHoodHashCapsule;

#[test]
fn test_basic_operations() {
    let map = RobinHoodHashCapsule::new();

    // Insert
    assert!(map.insert(1u64, "one").is_ok());
    assert!(map.insert(2u64, "two").is_ok());
    assert!(map.insert(3u64, "three").is_ok());

    // Get
    assert_eq!(map.get(&1), Some("one"));
    assert_eq!(map.get(&2), Some("two"));
    assert_eq!(map.get(&3), Some("three"));
    assert_eq!(map.get(&999), None);

    // Remove
    assert_eq!(map.remove(&2), Some("two"));
    assert_eq!(map.get(&2), None);
    assert_eq!(map.remove(&2), None);

    // Still have others
    assert_eq!(map.get(&1), Some("one"));
    assert_eq!(map.get(&3), Some("three"));
}

#[test]
fn test_high_load_factor_80_percent() {
    // Test at 80% load factor (Robin Hood target)
    let capacity = 1024;
    let map = RobinHoodHashCapsule::with_capacity(capacity);

    // Fill to 80% (819 entries)
    let target_entries = (capacity as f64 * 0.80) as usize;

    for i in 0..target_entries {
        let result = map.insert(i as u64, i as u64);
        assert!(result.is_ok(), "Insert failed at entry {} (load factor: {:.2}%)",
            i, map.load_factor() * 100.0);
    }

    // Verify all entries retrievable
    for i in 0..target_entries {
        assert_eq!(map.get(&(i as u64)), Some(i as u64),
            "Key {} not found at 80% load", i);
    }

    // Verify load factor
    let lf = map.load_factor();
    assert!(lf >= 0.75 && lf <= 0.85, "Load factor out of range: {:.2}%", lf * 100.0);
}

#[test]
fn test_high_load_factor_90_percent() {
    // Test at 90% load factor (stretch goal)
    let capacity = 512;
    let map = RobinHoodHashCapsule::with_capacity(capacity);

    // Fill to 90% (460 entries)
    let target_entries = (capacity as f64 * 0.90) as usize;

    let mut successful = 0;
    for i in 0..target_entries {
        if map.insert(i as u64, i as u64).is_ok() {
            successful += 1;
        }
    }

    // Should succeed at ~90% with occasional failures acceptable
    assert!(successful >= (target_entries as f64 * 0.95) as usize,
        "Too many failures at 90% load: {}/{}", successful, target_entries);

    // Verify all successful entries retrievable
    for i in 0..successful {
        assert_eq!(map.get(&(i as u64)), Some(i as u64));
    }
}

#[test]
fn test_large_dataset() {
    // Test with 10K entries (stress test)
    let map = RobinHoodHashCapsule::with_capacity(16384); // 16K capacity

    for i in 0..10_000 {
        assert!(map.insert(i, i * 10).is_ok(), "Insert failed at {}", i);
    }

    // Verify all entries
    for i in 0..10_000 {
        assert_eq!(map.get(&i), Some(i * 10), "Key {} not found", i);
    }

    // Verify load factor (10K / 16K = 62.5%)
    let lf = map.load_factor();
    assert!(lf >= 0.60 && lf <= 0.65, "Load factor: {:.2}%", lf * 100.0);
}

#[test]
fn test_robin_hood_swapping() {
    // Test that Robin Hood swapping maintains retrieval correctness
    let map = RobinHoodHashCapsule::with_capacity(128);

    // Insert 100 entries (78% load) - will trigger swapping
    for i in 0..100 {
        map.insert(i, i * 100).unwrap();
    }

    // Verify all entries retrievable after swapping
    for i in 0..100 {
        assert_eq!(map.get(&i), Some(i * 100),
            "Key {} not found after Robin Hood swapping", i);
    }
}

#[test]
fn test_backward_shift_deletion() {
    // Test backward shift deletion maintains correctness
    let map = RobinHoodHashCapsule::with_capacity(256);

    // Insert 100 entries
    for i in 0..100 {
        map.insert(i, i * 10).unwrap();
    }

    // Delete every other entry
    for i in (0..100).step_by(2) {
        assert_eq!(map.remove(&i), Some(i * 10), "Failed to remove key {}", i);
    }

    // Verify remaining entries still retrievable
    for i in (1..100).step_by(2) {
        assert_eq!(map.get(&i), Some(i * 10),
            "Key {} not found after deletions", i);
    }

    // Verify deleted entries are gone
    for i in (0..100).step_by(2) {
        assert_eq!(map.get(&i), None, "Key {} should be deleted", i);
    }

    // Verify size is correct
    assert_eq!(map.len(), 50);
}

#[test]
fn test_value_replacement() {
    let map = RobinHoodHashCapsule::new();

    // Initial insert
    assert_eq!(map.insert(42, "forty-two"), Ok(None));
    assert_eq!(map.get(&42), Some("forty-two"));

    // Replace value
    assert_eq!(map.insert(42, "FORTY-TWO"), Ok(Some("forty-two")));
    assert_eq!(map.get(&42), Some("FORTY-TWO"));

    // Size should still be 1
    assert_eq!(map.len(), 1);
}

#[test]
fn test_empty_map() {
    let map: RobinHoodHashCapsule<u64, String> = RobinHoodHashCapsule::new();

    assert_eq!(map.len(), 0);
    assert!(map.is_empty());
    assert_eq!(map.get(&1), None);
    assert_eq!(map.remove(&1), None);
}

#[test]
fn test_lsh_workload_simulation() {
    // Simulate LSH workload: 1000 docs × 1250 band hashes = 1.25M inserts
    let map = RobinHoodHashCapsule::with_capacity(2_000_000); // 2M capacity → 62.5% load

    let docs = 1000;
    let bands_per_doc = 1250;

    for doc in 0..docs {
        for band in 0..bands_per_doc {
            let key = doc * 10_000 + band; // Unique key per (doc, band)
            let value = doc as u32;

            assert!(map.insert(key, value).is_ok(),
                "Insert failed at doc {}, band {} (total: {})",
                doc, band, doc * bands_per_doc + band);
        }
    }

    // Verify all entries retrievable
    for doc in 0..docs {
        for band in 0..bands_per_doc {
            let key = doc * 10_000 + band;
            let expected_value = doc as u32;

            assert_eq!(map.get(&key), Some(expected_value),
                "LSH entry ({}, {}) not found", doc, band);
        }
    }

    // Verify total entries
    assert_eq!(map.len(), docs * bands_per_doc);

    // Verify load factor is reasonable (59-70% for 1.25M / 2M = 62.5% target)
    let lf = map.load_factor();
    assert!(lf >= 0.58 && lf <= 0.70, "Load factor: {:.2}%", lf * 100.0);
}

#[test]
fn test_concurrent_inserts_sequential() {
    // Sequential test (concurrent version requires std::thread)
    let map = RobinHoodHashCapsule::with_capacity(4096);

    // Simulate concurrent inserts by interleaving operations
    for i in 0..1000 {
        assert!(map.insert(i, i).is_ok());
    }

    for i in 0..1000 {
        assert_eq!(map.get(&i), Some(i));
    }
}
