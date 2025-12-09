//! # LockfreeResultAggregatorV2 - Comprehensive Test Suite (T28 Framework)
//!
//! **30+ tests across 4 tiers: Unit / Property / Integration / Production**

use atomic_capsule::parallel::LockfreeResultAggregatorV2;
use std::sync::Arc;
use std::thread;

// ============================================================================
// TIER 1: UNIT TESTS (Basic functionality)
// ============================================================================

#[test]
fn unit_new() {
    let agg: LockfreeResultAggregatorV2<u64, u64> = LockfreeResultAggregatorV2::new();
    assert!(agg.is_empty());
    assert_eq!(agg.len(), 0);
    assert_eq!(agg.capacity(), 16384); // DEFAULT_CAPACITY
}

#[test]
fn unit_with_capacity() {
    let agg: LockfreeResultAggregatorV2<u64, u64> = LockfreeResultAggregatorV2::with_capacity(1000);
    assert!(agg.is_empty());
    assert_eq!(agg.len(), 0);
    assert_eq!(agg.capacity(), 1000);
}

#[test]
fn unit_insert_single() {
    let agg = LockfreeResultAggregatorV2::new();
    let result = agg.insert(42u64, 100u64);
    assert!(result.is_ok());
    assert_eq!(agg.len(), 1);
    assert!(!agg.is_empty());
}

#[test]
fn unit_insert_multiple_same_key() {
    let agg = LockfreeResultAggregatorV2::new();
    agg.insert(42u64, 100u64).unwrap();
    agg.insert(42u64, 200u64).unwrap();
    agg.insert(42u64, 300u64).unwrap();

    // Verify len is still 1 (same key, multiple values)
    assert_eq!(agg.len(), 1);

    let results = agg.merge();
    assert_eq!(results.len(), 1);
    assert!(results.contains_key(&42));
    let values = &results[&42];
    assert_eq!(values.len(), 3);
    assert!(values.contains(&100));
    assert!(values.contains(&200));
    assert!(values.contains(&300));
}

#[test]
fn unit_insert_multiple_keys() {
    let agg = LockfreeResultAggregatorV2::new();
    agg.insert(1u64, 100u64).unwrap();
    agg.insert(2u64, 200u64).unwrap();
    agg.insert(3u64, 300u64).unwrap();

    assert_eq!(agg.len(), 3);

    let results = agg.merge();
    assert_eq!(results.len(), 3);
    assert_eq!(results[&1], vec![100]);
    assert_eq!(results[&2], vec![200]);
    assert_eq!(results[&3], vec![300]);
}

#[test]
fn unit_merge_empty() {
    let agg = LockfreeResultAggregatorV2::<u64, u64>::new();
    let results = agg.merge();
    assert_eq!(results.len(), 0);
}

#[test]
fn unit_capacity_exhaustion() {
    // Create small capacity to force exhaustion
    let agg = LockfreeResultAggregatorV2::with_capacity(4);

    // Fill capacity (should succeed)
    for i in 0..4 {
        agg.insert(i, i).unwrap();
    }

    // Insert beyond capacity (should fail after MAX_PROBE_DISTANCE)
    // Use a key that doesn't collide with existing keys
    let result = agg.insert(1000u64, 1000u64);
    assert!(result.is_err());
}

#[test]
fn unit_default() {
    let agg: LockfreeResultAggregatorV2<u64, u64> = LockfreeResultAggregatorV2::default();
    assert!(agg.is_empty());
    assert_eq!(agg.capacity(), 16384);
}

// ============================================================================
// TIER 2: PROPERTY TESTS (Correctness invariants)
// ============================================================================

#[test]
fn property_deterministic_hashing() {
    let agg = LockfreeResultAggregatorV2::new();

    // Same key should always produce same hash (deterministic)
    agg.insert(12345u64, 1u64).unwrap();
    agg.insert(12345u64, 2u64).unwrap();
    agg.insert(12345u64, 3u64).unwrap();

    let results = agg.merge();
    assert_eq!(results.len(), 1); // Same key, one entry
    assert_eq!(results[&12345].len(), 3); // Three values
}

#[test]
fn property_no_lost_updates() {
    let agg = LockfreeResultAggregatorV2::new();

    // Insert 1000 values for same key
    for i in 0..1000 {
        agg.insert(42u64, i).unwrap();
    }

    let results = agg.merge();
    assert_eq!(results.len(), 1);
    assert_eq!(results[&42].len(), 1000); // All values preserved
}

#[test]
fn property_key_uniqueness() {
    let agg = LockfreeResultAggregatorV2::new();

    // Insert 100 unique keys
    for i in 0..100 {
        agg.insert(i, i * 10).unwrap();
    }

    let results = agg.merge();
    assert_eq!(results.len(), 100); // All keys unique

    // Verify all keys present
    for i in 0..100 {
        assert!(results.contains_key(&i));
        assert_eq!(results[&i], vec![i * 10]);
    }
}

#[test]
fn property_value_ordering() {
    let agg = LockfreeResultAggregatorV2::new();

    // Insert values in order
    for i in 0..10 {
        agg.insert(42u64, i).unwrap();
    }

    let results = agg.merge();
    let values = &results[&42];

    // Verify values are in insertion order
    assert_eq!(values.len(), 10);
    for (idx, &val) in values.iter().enumerate() {
        assert_eq!(val, idx as u64);
    }
}

#[test]
fn property_len_approximate() {
    let agg = LockfreeResultAggregatorV2::new();

    // Insert 100 unique keys
    for i in 0..100 {
        agg.insert(i, i).unwrap();
    }

    // len() is approximate (Relaxed ordering), but should be close
    let len = agg.len();
    assert!(len > 0 && len <= 100);

    // merge().len() is exact
    let results = agg.merge();
    assert_eq!(results.len(), 100);
}

// ============================================================================
// TIER 3: INTEGRATION TESTS (Concurrent access)
// ============================================================================

#[test]
fn integration_concurrent_insert_unique_keys() {
    let agg = Arc::new(LockfreeResultAggregatorV2::new());
    let mut handles = vec![];

    // Spawn 16 threads, each inserting 1000 unique keys
    for thread_id in 0..16 {
        let agg_clone = Arc::clone(&agg);
        let handle = thread::spawn(move || {
            for i in 0..1000 {
                let key = (thread_id * 1000 + i) as u64;
                agg_clone.insert(key, thread_id as u64).unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify: 16 threads × 1000 inserts = 16K unique keys
    let results = agg.merge();
    assert_eq!(results.len(), 16000);

    // Verify each key has exactly one value (thread_id)
    for (key, values) in results.iter() {
        assert_eq!(values.len(), 1);
        let thread_id = (key / 1000) as u64;
        assert_eq!(values[0], thread_id);
    }
}

#[test]
fn integration_concurrent_insert_same_keys() {
    let agg = Arc::new(LockfreeResultAggregatorV2::new());
    let mut handles = vec![];

    // Spawn 16 threads, all inserting to same 100 keys
    // This tests contention on same keys
    for thread_id in 0..16 {
        let agg_clone = Arc::clone(&agg);
        let handle = thread::spawn(move || {
            for key in 0..100 {
                agg_clone.insert(key, thread_id as u64).unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify: 100 keys, each should have 16 values (one per thread)
    let results = agg.merge();
    assert_eq!(results.len(), 100);

    // All 16 values should be present (no lost updates)
    for (_key, values) in results.iter() {
        assert_eq!(values.len(), 16);
    }

    // Count total values (should be exactly 16 × 100 = 1600)
    let total_values: usize = results.values().map(|v| v.len()).sum();
    assert_eq!(total_values, 1600);
}

#[test]
fn integration_concurrent_mixed_pattern() {
    let agg = Arc::new(LockfreeResultAggregatorV2::new());
    let mut handles = vec![];

    // Thread 0-7: Unique keys (low contention)
    for thread_id in 0..8 {
        let agg_clone = Arc::clone(&agg);
        let handle = thread::spawn(move || {
            for i in 0..500 {
                let key = (thread_id * 1000 + i) as u64;
                agg_clone.insert(key, thread_id as u64).unwrap();
            }
        });
        handles.push(handle);
    }

    // Thread 8-15: Shared keys (high contention)
    for thread_id in 8..16 {
        let agg_clone = Arc::clone(&agg);
        let handle = thread::spawn(move || {
            for key in 0..100 {
                agg_clone.insert(key, thread_id as u64).unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    let results = agg.merge();

    // Verify unique keys (thread 0-7): 8 × 500 = 4000 keys
    let unique_count = results.iter().filter(|(_, v)| v.len() == 1).count();
    assert_eq!(unique_count, 4000);

    // Verify shared keys (thread 8-15): 100 keys with 8 values each
    let shared_count = results.iter().filter(|(_, v)| v.len() == 8).count();
    assert_eq!(shared_count, 100);
}

#[test]
fn integration_thread_safety() {
    // Verify Send + Sync bounds (compile-time check)
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<LockfreeResultAggregatorV2<u64, u64>>();
}

// ============================================================================
// TIER 4: PRODUCTION TESTS (Real-world scenarios)
// ============================================================================

#[test]
fn production_stress_test_100k() {
    let agg = Arc::new(LockfreeResultAggregatorV2::new());
    let mut handles = vec![];

    // Spawn 16 threads, each inserting 6250 results (100K total)
    for thread_id in 0..16 {
        let agg_clone = Arc::clone(&agg);
        let handle = thread::spawn(move || {
            for i in 0..6250 {
                let key = (thread_id * 10000 + i) as u64;
                agg_clone.insert(key, thread_id as u64).unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify: 100K unique keys
    let results = agg.merge();
    assert_eq!(results.len(), 100000);

    // Verify all values present
    let total_values: usize = results.values().map(|v| v.len()).sum();
    assert_eq!(total_values, 100000);
}

#[test]
fn production_duplicate_heavy_workload() {
    let agg = Arc::new(LockfreeResultAggregatorV2::new());
    let mut handles = vec![];

    // Spawn 16 threads, all inserting to same 10 keys (high duplication)
    for thread_id in 0..16 {
        let agg_clone = Arc::clone(&agg);
        let handle = thread::spawn(move || {
            for _ in 0..1000 {
                for key in 0..10 {
                    agg_clone.insert(key, thread_id as u64).unwrap();
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify: 10 keys, each with 16K values (16 threads × 1000 iterations)
    let results = agg.merge();
    assert_eq!(results.len(), 10);

    for (_key, values) in results.iter() {
        assert_eq!(values.len(), 16000);
    }

    // Total values: 10 × 16K = 160K
    let total_values: usize = results.values().map(|v| v.len()).sum();
    assert_eq!(total_values, 160000);
}

#[test]
fn production_realistic_dedup_workload() {
    // Simulate kindly_dedup workload:
    // - 10K documents
    // - 80% duplicates (2K unique documents)
    // - Each duplicate has 5 candidates on average

    let agg = Arc::new(LockfreeResultAggregatorV2::new());
    let mut handles = vec![];

    // Spawn 16 threads
    for thread_id in 0..16 {
        let agg_clone = Arc::clone(&agg);
        let handle = thread::spawn(move || {
            // Each thread processes 625 documents
            for i in 0..625 {
                let doc_id = (thread_id * 625 + i) as u64;

                // 80% duplicates map to 2K unique buckets
                let bucket_id = doc_id % 2000;

                // Insert 5 candidates per document
                for candidate_id in 0..5 {
                    agg_clone
                        .insert(bucket_id, doc_id * 5 + candidate_id)
                        .unwrap();
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify: ~2K unique buckets
    let results = agg.merge();
    assert!(results.len() >= 1900 && results.len() <= 2100); // Approximate due to hash distribution

    // Total candidates: 10K documents × 5 candidates = 50K
    let total_candidates: usize = results.values().map(|v| v.len()).sum();
    assert_eq!(total_candidates, 50000);
}

#[test]
fn production_collision_resistance() {
    // Test collision handling with keys that may hash to same bucket
    let agg = LockfreeResultAggregatorV2::with_capacity(256); // Small capacity to force collisions

    // Insert 100 sequential keys (likely to collide)
    for i in 0..100 {
        agg.insert(i, i).unwrap();
    }

    let results = agg.merge();
    assert_eq!(results.len(), 100);

    // Verify all keys present (no lost keys due to collisions)
    for i in 0..100 {
        assert!(results.contains_key(&i));
        assert_eq!(results[&i], vec![i]);
    }
}

#[test]
fn production_string_keys() {
    // Test with String keys (common in real-world usage)
    let agg = LockfreeResultAggregatorV2::new();

    for i in 0..1000 {
        let key = format!("doc_{}", i);
        agg.insert(key, i as u64).unwrap();
    }

    let results = agg.merge();
    assert_eq!(results.len(), 1000);

    // Verify specific keys
    assert!(results.contains_key("doc_0"));
    assert!(results.contains_key("doc_500"));
    assert!(results.contains_key("doc_999"));
}

#[test]
fn production_large_values() {
    // Test with large Vec<V> (many values per key)
    let agg = LockfreeResultAggregatorV2::new();

    // Insert 10K values to same key
    for i in 0..10000 {
        agg.insert(42u64, i).unwrap();
    }

    let results = agg.merge();
    assert_eq!(results.len(), 1);
    assert_eq!(results[&42].len(), 10000);
}

#[test]
fn production_merge_idempotency() {
    // Verify merge can be called multiple times
    let agg = LockfreeResultAggregatorV2::new();

    for i in 0..100 {
        agg.insert(i, i).unwrap();
    }

    // First merge
    let results1 = agg.merge();
    assert_eq!(results1.len(), 100);

    // Second merge (should produce same result)
    let results2 = agg.merge();
    assert_eq!(results2.len(), 100);

    // Verify contents match
    for i in 0..100 {
        assert_eq!(results1[&i], results2[&i]);
    }
}

// ============================================================================
// EDGE CASES
// ============================================================================

#[test]
fn edge_case_zero_capacity() {
    // Capacity of 0 should still work (empty aggregator)
    let agg = LockfreeResultAggregatorV2::<u64, u64>::with_capacity(0);
    assert_eq!(agg.capacity(), 0);
    let results = agg.merge();
    assert_eq!(results.len(), 0);
}

#[test]
fn edge_case_single_capacity() {
    // Capacity of 1 (minimal)
    let agg = LockfreeResultAggregatorV2::with_capacity(1);
    agg.insert(42u64, 100u64).unwrap();

    // Second insert should fail (capacity exhausted)
    let result = agg.insert(43u64, 200u64);
    assert!(result.is_err());

    // But appending to same key should work
    agg.insert(42u64, 300u64).unwrap();

    let results = agg.merge();
    assert_eq!(results.len(), 1);
    assert_eq!(results[&42], vec![100, 300]);
}

#[test]
fn edge_case_max_key_value() {
    // Test with maximum u64 values
    let agg = LockfreeResultAggregatorV2::new();
    agg.insert(u64::MAX, u64::MAX).unwrap();

    let results = agg.merge();
    assert_eq!(results.len(), 1);
    assert_eq!(results[&u64::MAX], vec![u64::MAX]);
}
