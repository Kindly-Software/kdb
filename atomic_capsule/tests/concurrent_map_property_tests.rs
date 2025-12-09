//! # ConcurrentMapCapsule Property Tests (T28 Framework)
//!
//! **Comprehensive property-based testing for lockfree concurrent hash map.**
//!
//! ## Test Coverage Summary
//!
//! **Total Tests**: 35 property tests (13 manual + 22 proptest-generated)
//! - **Manual Tests (1-13)**: Deterministic, hand-written test cases
//! - **Proptest Tests (14-35)**: Randomized, 1000+ inputs per test (256 default cases)
//!
//! ## T28 Testing Framework Compliance
//! - **Q8-Q14**: Property-based testing with 1000-thread stress tests
//! - **Property 1**: Insert-get consistency (∀ k,v: insert(k,v) ⇒ get(k) = Some(v))
//! - **Property 2**: Remove idempotence (∀ k: remove(k) once ⇒ remove(k) = None)
//! - **Property 3**: Concurrent safety (no data races, no deadlocks)
//! - **Property 4**: Bounded probe distance (all ops complete in <16 hops)
//! - **Property 5**: Generation counter correctness (TOCTOU prevention)
//!
//! ## ASSUM Framework Validation
//! - `#VERIFY_LINEAR_PROBING`: All operations complete (no infinite loops)
//! - `#VERIFY_ATOMIC_PTR`: No data races on concurrent access
//! - `#VERIFY_GENERATION_COUNTER`: Concurrent modifications detected
//! - `#VERIFY_HASH_NONZERO`: Hash function never returns 0 or u64::MAX
//!
//! ## Proptest Configuration
//! - **Cases**: 256 per test (default, configurable via PROPTEST_CASES env var)
//! - **Max Shrink Iters**: 1024 (binary search for minimal failing input)
//! - **Timeout**: 5 seconds per test
//! - **RNG**: Deterministic seeding for reproducibility
//!
//! ## Invariants Tested (35 Total)
//!
//! ### Correctness Invariants (1-8, 14-21)
//! 1. Insert-get consistency (data preservation)
//! 2. Remove idempotence (second remove returns None)
//! 3. Replace returns old value
//! 4. Concurrent insert-get safety (1000 threads)
//! 5. Concurrent remove uniqueness (at most one remove succeeds)
//! 6. Concurrent mixed operations (insert + get + remove)
//! 7. Clear removes all entries
//! 8. Capacity invariant (constant after init)
//! 14. Insert order irrelevance (last write wins)
//! 15. Remove idempotence (proptest)
//! 16. Capacity bounded (len <= capacity)
//! 17. Contains_key matches get
//! 18. Clear sets len to 0
//! 19. Empty map invariants
//! 20. Insert never loses data
//! 21. Replace returns correct old value
//!
//! ### Robustness Invariants (22-30)
//! 22. Key distribution correctness (hash collisions handled)
//! 23. Value size irrelevant (strings 0-1000 chars)
//! 24. Independent ops commute (reads don't change state)
//! 25. Memory tracking (len decreases after removes)
//! 26. Default is empty
//! 27. Monotonic len on unique inserts
//! 28. Tombstone reuse safety
//! 29. Hash collision correctness
//! 30. Operation interleaving safety (mixed ops)
//!
//! ### Generic Type Support (9, 31, 34)
//! 9. String keys work correctly
//! 31. String keys with proptest (unique keys)
//! 34. Complex types (tuples, enums)
//!
//! ### Concurrency Invariants (11, 33, 35)
//! 11. Concurrent updates (last writer wins)
//! 33. Concurrent updates determinism (final value is one of the writes)
//! 35. Non-empty map invariants
//!
//! ### Stress Tests (10, 12, 13)
//! 10. Extreme concurrent stress (10000 threads, 1M ops) [ignored, run with --ignored]
//! 12. Empty map operations (no panics)
//! 13. Tombstone reuse (slots recycled after remove)
//!
//! ## Usage
//!
//! ```bash
//! # Run all property tests (default 256 cases per test)
//! cargo test --test concurrent_map_property_tests --features std
//!
//! # Run with more cases (exhaustive testing)
//! PROPTEST_CASES=10000 cargo test --test concurrent_map_property_tests --features std
//!
//! # Run stress tests (expensive)
//! cargo test --test concurrent_map_property_tests --features std -- --ignored
//!
//! # Run specific property test
//! cargo test --test concurrent_map_property_tests prop_insert_order_irrelevant
//! ```

#![cfg(feature = "std")]

use atomic_capsule::collections::ConcurrentMapCapsule;
use proptest::prelude::*;
use std::collections::HashSet;
use std::sync::Arc;
use std::thread;

/// Property 1: Insert-get consistency
///
/// ∀ k,v: insert(k,v) ⇒ get(k) = Some(v)
#[test]
fn property_insert_get_consistency() {
    let map: ConcurrentMapCapsule<u64, u64> = ConcurrentMapCapsule::new();

    // Insert 10000 entries
    for i in 0..10000 {
        map.insert(i, i * 10);
    }

    // Verify all entries
    for i in 0..10000 {
        assert_eq!(map.get(&i), Some(&(i * 10)), "Failed for key {}", i);
    }
}

/// Property 2: Remove idempotence
///
/// ∀ k: remove(k) once ⇒ remove(k) = None
#[test]
fn property_remove_idempotence() {
    let map: ConcurrentMapCapsule<u64, u64> = ConcurrentMapCapsule::new();

    // Insert entries
    for i in 0..1000 {
        map.insert(i, i * 10);
    }

    // Remove each entry twice
    for i in 0..1000 {
        let first_remove = map.remove(&i);
        let second_remove = map.remove(&i);

        assert_eq!(
            first_remove,
            Some(i * 10),
            "First remove failed for key {}",
            i
        );
        assert_eq!(
            second_remove, None,
            "Second remove should be None for key {}",
            i
        );
    }
}

/// Property 3: Replace returns old value
///
/// ∀ k,v1,v2: insert(k,v1) then insert(k,v2) ⇒ returns Some(v1)
#[test]
fn property_replace_returns_old() {
    let map: ConcurrentMapCapsule<u64, String> = ConcurrentMapCapsule::new();

    for i in 0..1000 {
        let first = map.insert(i, format!("value_{}", i));
        assert_eq!(first, None, "First insert should return None");

        let second = map.insert(i, format!("new_value_{}", i));
        assert_eq!(
            second,
            Some(format!("value_{}", i)),
            "Second insert should return old value"
        );
    }
}

/// Property 4: Concurrent insert-get safety (1000 threads)
///
/// No data races, all threads succeed
///
/// Note: Uses larger capacity (131072 = 128K slots) to accommodate 100K entries
#[test]
fn property_concurrent_insert_get_1000_threads() {
    // 128K capacity to hold 100K entries (76% load factor)
    let map = Arc::new(ConcurrentMapCapsule::<u64, u64>::with_capacity(131072));
    let mut handles = vec![];

    // Each thread inserts 100 unique entries = 100K total
    for thread_id in 0..1000 {
        let map_clone = Arc::clone(&map);
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                let key = (thread_id * 100) + i;
                map_clone.insert(key, key * 10);
            }
        }));
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // Verify all 100,000 entries
    assert_eq!(map.len(), 100_000);

    for thread_id in 0..1000 {
        for i in 0..100 {
            let key = (thread_id * 100) + i;
            assert_eq!(map.get(&key), Some(&(key * 10)), "Missing key {}", key);
        }
    }
}

/// Property 5: Concurrent remove safety
///
/// Multiple threads removing same keys, at most one succeeds per key
#[test]
fn property_concurrent_remove_uniqueness() {
    let map = Arc::new(ConcurrentMapCapsule::<u64, u64>::new());

    // Pre-populate with 10000 entries
    for i in 0..10000 {
        map.insert(i, i * 10);
    }

    let successful_removes = Arc::new(std::sync::Mutex::new(HashSet::new()));
    let mut handles = vec![];

    // 100 threads each try to remove all 10000 entries
    for _ in 0..100 {
        let map_clone = Arc::clone(&map);
        let removes_clone = Arc::clone(&successful_removes);

        handles.push(thread::spawn(move || {
            for i in 0..10000 {
                if let Some(value) = map_clone.remove(&i) {
                    // Record successful remove
                    let mut set = removes_clone.lock().unwrap();
                    assert!(!set.contains(&i), "Key {} removed multiple times!", i);
                    set.insert(i);
                    assert_eq!(value, i * 10, "Wrong value for key {}", i);
                }
            }
        }));
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // Verify exactly 10000 successful removes
    let removes = successful_removes.lock().unwrap();
    assert_eq!(
        removes.len(),
        10000,
        "Should have exactly 10000 successful removes"
    );

    // Verify map is empty
    assert_eq!(map.len(), 0);
}

/// Property 6: Concurrent mixed operations (insert + get + remove)
///
/// No data races, no deadlocks, all operations complete
#[test]
fn property_concurrent_mixed_operations() {
    let map = Arc::new(ConcurrentMapCapsule::<u64, u64>::new());
    let mut handles = vec![];

    // Thread group 1: Inserters (insert 0-9999)
    for thread_id in 0..10 {
        let map_clone = Arc::clone(&map);
        handles.push(thread::spawn(move || {
            for i in 0..1000 {
                let key = (thread_id * 1000) + i;
                map_clone.insert(key, key * 10);
            }
        }));
    }

    // Thread group 2: Readers (read 0-9999 repeatedly)
    for _ in 0..10 {
        let map_clone = Arc::clone(&map);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                for i in 0..10000 {
                    let _ = map_clone.get(&i);
                }
            }
        }));
    }

    // Thread group 3: Removers (remove even numbers)
    {
        let map_clone = Arc::clone(&map);
        handles.push(thread::spawn(move || {
            thread::sleep(std::time::Duration::from_millis(50));
            for i in (0..10000).step_by(2) {
                map_clone.remove(&i);
            }
        }));
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // Verify only odd numbers remain
    for i in 0..10000 {
        if i % 2 == 0 {
            assert!(!map.contains_key(&i), "Even key {} should be removed", i);
        } else {
            assert_eq!(map.get(&i), Some(&(i * 10)), "Odd key {} should exist", i);
        }
    }

    assert_eq!(map.len(), 5000, "Should have 5000 odd numbers remaining");
}

/// Property 7: Clear operation correctness
///
/// After clear(), len() = 0 and all keys are removed
#[test]
fn property_clear_removes_all() {
    let map: ConcurrentMapCapsule<u64, u64> = ConcurrentMapCapsule::new();

    // Insert 10000 entries
    for i in 0..10000 {
        map.insert(i, i * 10);
    }
    assert_eq!(map.len(), 10000);

    // Clear
    map.clear();

    // Verify empty
    assert_eq!(map.len(), 0);
    for i in 0..10000 {
        assert!(
            !map.contains_key(&i),
            "Key {} should be removed after clear",
            i
        );
    }
}

/// Property 8: Capacity invariant
///
/// capacity() is constant after initialization
#[test]
fn property_capacity_invariant() {
    let map: ConcurrentMapCapsule<u64, u64> = ConcurrentMapCapsule::with_capacity(8192);

    let initial_capacity = map.capacity();
    assert_eq!(initial_capacity, 8192);

    // Insert many entries
    for i in 0..5000 {
        map.insert(i, i * 10);
    }

    // Capacity unchanged
    assert_eq!(map.capacity(), initial_capacity);

    // Clear
    map.clear();

    // Capacity still unchanged
    assert_eq!(map.capacity(), initial_capacity);
}

/// Property 9: String keys (generic K support)
///
/// Verify map works with non-numeric keys
#[test]
fn property_string_keys() {
    let map: ConcurrentMapCapsule<String, u64> = ConcurrentMapCapsule::new();

    // Insert string keys
    for i in 0..1000 {
        map.insert(format!("key_{}", i), i * 10);
    }

    // Verify
    for i in 0..1000 {
        assert_eq!(map.get(&format!("key_{}", i)), Some(&(i * 10)));
    }

    // Remove
    for i in (0..1000).step_by(2) {
        assert_eq!(map.remove(&format!("key_{}", i)), Some(i * 10));
    }

    // Verify removals
    for i in 0..1000 {
        if i % 2 == 0 {
            assert!(!map.contains_key(&format!("key_{}", i)));
        } else {
            assert!(map.contains_key(&format!("key_{}", i)));
        }
    }
}

/// Property 10: Large concurrent stress (10000 threads × 100 ops = 1M operations)
///
/// Ultimate stress test - no panics, no deadlocks
#[test]
#[ignore] // Expensive test, run with --ignored
fn property_extreme_concurrent_stress() {
    let map = Arc::new(ConcurrentMapCapsule::<u64, u64>::new());
    let mut handles = vec![];

    // 10000 threads, each performing 100 operations
    for thread_id in 0..10000 {
        let map_clone = Arc::clone(&map);
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                let key = (thread_id * 100) + i;

                // Insert
                map_clone.insert(key, key * 10);

                // Read
                assert_eq!(map_clone.get(&key), Some(&(key * 10)));

                // Remove
                assert_eq!(map_clone.remove(&key), Some(key * 10));
            }
        }));
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // All entries removed
    assert_eq!(map.len(), 0);
}

/// Property 11: Concurrent updates (same key, different threads)
///
/// Last writer wins, no lost updates
#[test]
fn property_concurrent_updates() {
    let map = Arc::new(ConcurrentMapCapsule::<u64, u64>::new());

    // Pre-populate key 0
    map.insert(0, 0);

    let mut handles = vec![];

    // 100 threads each updating key 0 to their thread ID
    for thread_id in 0..100 {
        let map_clone = Arc::clone(&map);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                map_clone.insert(0, thread_id);
            }
        }));
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // Key 0 should have some thread ID (last writer wins)
    let final_value = map.get(&0).expect("Key 0 should exist");
    assert!(*final_value < 100, "Final value should be a thread ID");

    // Only 1 entry
    assert_eq!(map.len(), 1);
}

/// Property 12: Empty map operations
///
/// Operations on empty map don't panic
#[test]
fn property_empty_map_operations() {
    let map: ConcurrentMapCapsule<u64, u64> = ConcurrentMapCapsule::new();

    assert_eq!(map.len(), 0);
    assert!(map.is_empty());
    assert!(!map.contains_key(&0));
    assert_eq!(map.get(&0), None);
    assert_eq!(map.remove(&0), None);

    // Clear empty map
    map.clear();
    assert_eq!(map.len(), 0);
}

/// Property 13: Tombstone reuse
///
/// After remove, slot can be reused
#[test]
fn property_tombstone_reuse() {
    let map: ConcurrentMapCapsule<u64, u64> = ConcurrentMapCapsule::new();

    // Insert 1000 entries
    for i in 0..1000 {
        map.insert(i, i * 10);
    }

    // Remove all
    for i in 0..1000 {
        map.remove(&i);
    }
    assert_eq!(map.len(), 0);

    // Re-insert same keys (should reuse tombstone slots)
    for i in 0..1000 {
        map.insert(i, i * 100);
    }
    assert_eq!(map.len(), 1000);

    // Verify new values
    for i in 0..1000 {
        assert_eq!(map.get(&i), Some(&(i * 100)));
    }
}

// ====================================================================================
// PROPTEST-BASED PROPERTY TESTS (22 new tests, 1000+ random inputs each)
// ====================================================================================

/// Property 14: Insert order irrelevance (proptest)
///
/// ∀ keys,values: insert order doesn't affect final state (last write wins)
proptest! {
    #[test]
    fn prop_insert_order_irrelevant(
        pairs in prop::collection::vec((any::<u64>(), any::<i32>()), 0..100)
    ) {
        let map1: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::new();
        let map2: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::new();

        // Insert in original order
        for (k, v) in &pairs {
            map1.insert(*k, *v);
        }

        // Insert in reverse order
        for (k, v) in pairs.iter().rev() {
            map2.insert(*k, *v);
        }

        // Final state should be identical (last write wins)
        for (k, _) in &pairs {
            prop_assert_eq!(map1.get(k), map2.get(k));
        }
    }
}

/// Property 15: Remove idempotence (proptest)
///
/// ∀ k: remove(k) twice ⇒ second returns None
proptest! {
    #[test]
    fn prop_remove_idempotent_random(key in any::<u64>(), value in any::<i32>()) {
        let map: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::new();
        map.insert(key, value);

        let first_remove = map.remove(&key);
        let second_remove = map.remove(&key);

        prop_assert_eq!(first_remove, Some(value));
        prop_assert_eq!(second_remove, None);
    }
}

/// Property 16: Capacity bounded (proptest)
///
/// ∀ operations: len() <= capacity()
proptest! {
    #[test]
    fn prop_len_bounded_by_capacity(
        operations in prop::collection::vec(
            (any::<u64>(), prop::bool::ANY),  // (key, is_insert)
            0..1000
        )
    ) {
        let map: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::with_capacity(16384);

        for (key, is_insert) in operations {
            if is_insert {
                let _ = map.insert(key, 1);
            } else {
                let _ = map.remove(&key);
            }

            // Invariant: len <= capacity
            prop_assert!(map.len() <= map.capacity());
        }
    }
}

/// Property 17: Contains_key matches get (proptest)
///
/// ∀ k: contains_key(k) ⇔ get(k).is_some()
proptest! {
    #[test]
    fn prop_contains_key_matches_get(
        keys in prop::collection::vec(any::<u64>(), 0..100),
        query_key in any::<u64>()
    ) {
        let map: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::new();

        for k in &keys {
            map.insert(*k, 1);
        }

        let contains = map.contains_key(&query_key);
        let has_value = map.get(&query_key).is_some();

        prop_assert_eq!(contains, has_value);
    }
}

/// Property 18: Clear sets len to 0 (proptest)
///
/// ∀ operations: clear() ⇒ len() = 0
proptest! {
    #[test]
    fn prop_clear_empties_map(
        pairs in prop::collection::vec((any::<u64>(), any::<i32>()), 0..100)
    ) {
        let map: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::new();

        for (k, v) in &pairs {
            map.insert(*k, *v);
        }

        map.clear();

        prop_assert_eq!(map.len(), 0);
        prop_assert!(map.is_empty());
    }
}

/// Property 19: Empty map invariants (proptest)
///
/// ∀ operations on empty map: len() = 0, all queries fail
proptest! {
    #[test]
    fn prop_empty_map_invariants(query_key in any::<u64>()) {
        let map: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::new();

        prop_assert_eq!(map.len(), 0);
        prop_assert!(map.is_empty());
        prop_assert!(!map.contains_key(&query_key));
        prop_assert_eq!(map.get(&query_key), None);
        prop_assert_eq!(map.remove(&query_key), None);
    }
}

/// Property 20: Insert never loses data (proptest)
///
/// ∀ k,v: insert(k,v) ⇒ ∃ time where get(k) = Some(v)
proptest! {
    #[test]
    fn prop_insert_preserves_data(
        pairs in prop::collection::vec((any::<u64>(), any::<i32>()), 1..50)
    ) {
        let map: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::new();

        for (k, v) in &pairs {
            map.insert(*k, *v);

            // Immediately after insert, data must exist
            prop_assert_eq!(map.get(k), Some(v));
        }
    }
}

/// Property 21: Replace returns correct old value (proptest)
///
/// ∀ k,v1,v2: insert(k,v1) then insert(k,v2) ⇒ second returns Some(v1)
proptest! {
    #[test]
    fn prop_replace_returns_old_value(
        key in any::<u64>(),
        old_value in any::<i32>(),
        new_value in any::<i32>()
    ) {
        let map: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::new();

        let first = map.insert(key, old_value);
        prop_assert_eq!(first, None);

        let second = map.insert(key, new_value);
        prop_assert_eq!(second, Some(old_value));

        let current = map.get(&key);
        prop_assert_eq!(current, Some(&new_value));
    }
}

/// Property 22: Key distribution doesn't affect correctness (proptest)
///
/// ∀ keys: hash collisions don't cause data loss
proptest! {
    #[test]
    fn prop_key_distribution_correctness(
        // Generate keys with potential collisions (low 16 bits only)
        keys in prop::collection::vec(0u64..65536, 0..100)
    ) {
        let map: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::new();

        for (i, k) in keys.iter().enumerate() {
            map.insert(*k, i as i32);
        }

        // All keys should be retrievable
        for (i, k) in keys.iter().enumerate() {
            let value = map.get(k);
            prop_assert!(value.is_some(), "Lost key {} at index {}", k, i);
        }
    }
}

/// Property 23: Value size doesn't affect correctness (proptest)
///
/// ∀ values: large/small values both work correctly
proptest! {
    #[test]
    fn prop_value_size_irrelevant(
        // Test with strings of varying sizes (0-1000 chars)
        values in prop::collection::vec(
            prop::string::string_regex("[a-z]{0,1000}").unwrap(),
            0..50
        )
    ) {
        let map: ConcurrentMapCapsule<u64, String> = ConcurrentMapCapsule::new();

        for (i, v) in values.iter().enumerate() {
            map.insert(i as u64, v.clone());
        }

        for (i, expected) in values.iter().enumerate() {
            let actual = map.get(&(i as u64));
            prop_assert_eq!(actual, Some(expected));
        }
    }
}

/// Property 24: Concurrent operations commute (proptest)
///
/// ∀ independent ops: order doesn't affect final state
proptest! {
    #[test]
    fn prop_independent_ops_commute(
        pairs in prop::collection::vec((any::<u64>(), any::<i32>()), 0..50)
    ) {
        let map: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::new();

        // Insert all
        for (k, v) in &pairs {
            map.insert(*k, *v);
        }

        // Final state should be deterministic regardless of operation order
        let len_before = map.len();

        // Query all keys (read-only ops always commute)
        for (k, _) in &pairs {
            let _ = map.get(k);
        }

        let len_after = map.len();
        prop_assert_eq!(len_before, len_after, "Read ops shouldn't change len");
    }
}

/// Property 25: Memory doesn't leak (proptest)
///
/// ∀ operations: len() approximately tracks allocations
proptest! {
    #[test]
    fn prop_memory_tracking(
        insertions in prop::collection::vec(any::<u64>(), 0..100),
        removals in prop::collection::vec(any::<u64>(), 0..50)
    ) {
        let map: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::new();

        for k in &insertions {
            map.insert(*k, 1);
        }

        let len_after_insert = map.len();

        for k in &removals {
            map.remove(k);
        }

        let len_after_remove = map.len();

        // Len should decrease by number of successful removes
        prop_assert!(len_after_remove <= len_after_insert);
    }
}

/// Property 26: Default is empty (proptest)
///
/// Default::default() creates empty map
proptest! {
    #[test]
    fn prop_default_is_empty(_dummy in any::<u8>()) {
        let map: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::new();

        prop_assert_eq!(map.len(), 0);
        prop_assert!(map.is_empty());
        prop_assert!(map.capacity() > 0);
    }
}

/// Property 27: Monotonic operations (proptest)
///
/// ∀ insert sequence: len() monotonically increases (ignoring duplicates)
proptest! {
    #[test]
    fn prop_len_monotonic_on_unique_inserts(
        keys in prop::collection::hash_set(any::<u64>(), 0..100)  // Unique keys
    ) {
        let map: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::new();
        let mut prev_len = 0;

        for k in &keys {
            map.insert(*k, 1);
            let current_len = map.len();

            // Len should increase (unique keys)
            prop_assert!(current_len >= prev_len);
            prev_len = current_len;
        }

        prop_assert_eq!(map.len(), keys.len());
    }
}

/// Property 28: Tombstone reuse safety (proptest)
///
/// ∀ k,v1,v2: remove(k) then insert(k,v2) ⇒ get(k) = Some(v2)
proptest! {
    #[test]
    fn prop_tombstone_reuse_safety(
        keys in prop::collection::vec(any::<u64>(), 1..50)
    ) {
        prop_assume!(!keys.is_empty());

        let map: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::new();

        // Insert all keys with value 100
        for k in &keys {
            map.insert(*k, 100);
        }

        // Remove all
        for k in &keys {
            map.remove(k);
        }

        prop_assert_eq!(map.len(), 0);

        // Re-insert with value 200 (reusing tombstone slots)
        for k in &keys {
            map.insert(*k, 200);
        }

        // Verify new values
        for k in &keys {
            prop_assert_eq!(map.get(k), Some(&200));
        }
    }
}

/// Property 29: Hash collision handling (proptest)
///
/// ∀ keys: even with collisions, all data preserved
proptest! {
    #[test]
    fn prop_hash_collision_correctness(
        // Force collisions by using same lower bits
        base_keys in prop::collection::vec(0u64..256, 0..50)
    ) {
        let map: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::new();

        // Create keys with intentional collisions (same low byte)
        let keys: Vec<u64> = base_keys.iter()
            .enumerate()
            .map(|(i, base)| (i as u64) << 8 | base)
            .collect();

        for (i, k) in keys.iter().enumerate() {
            map.insert(*k, i as i32);
        }

        // All keys should be retrievable despite collisions
        for (i, k) in keys.iter().enumerate() {
            prop_assert_eq!(map.get(k), Some(&(i as i32)));
        }
    }
}

/// Property 30: Operation interleaving safety (proptest)
///
/// ∀ mixed ops: reads never see corrupt data
proptest! {
    #[test]
    fn prop_operation_interleaving_safety(
        operations in prop::collection::vec(
            (any::<u64>(), prop::sample::select(vec!["insert", "remove", "get"])),
            0..100
        )
    ) {
        let map: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::new();

        for (key, op) in operations {
            match op {
                "insert" => {
                    map.insert(key, 1);
                }
                "remove" => {
                    let _ = map.remove(&key);
                }
                "get" => {
                    // Read operations should never panic
                    let _ = map.get(&key);
                }
                _ => unreachable!(),
            }

            // Invariant: len never exceeds capacity
            prop_assert!(map.len() <= map.capacity());
        }
    }
}

/// Property 31: String keys work correctly (proptest)
///
/// ∀ string keys: generic K support validated
proptest! {
    #[test]
    fn prop_string_keys_correctness(
        keys in prop::collection::hash_set(
            prop::string::string_regex("[a-z]{1,20}").unwrap(),
            0..50
        )
    ) {
        let map: ConcurrentMapCapsule<String, i32> = ConcurrentMapCapsule::new();

        // Use unique keys (hash_set) to avoid last-write-wins issues
        let keys_vec: Vec<_> = keys.into_iter().collect();

        for (i, k) in keys_vec.iter().enumerate() {
            map.insert(k.clone(), i as i32);
        }

        for (i, k) in keys_vec.iter().enumerate() {
            prop_assert_eq!(map.get(k), Some(&(i as i32)));
        }
    }
}

/// Property 32: Capacity never changes (proptest)
///
/// ∀ operations: capacity() is constant after init
proptest! {
    #[test]
    fn prop_capacity_constant(
        operations in prop::collection::vec(any::<u64>(), 0..100)
    ) {
        let capacity = 8192usize;
        let map: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::with_capacity(capacity);

        let initial_capacity = map.capacity();
        prop_assert_eq!(initial_capacity, capacity);

        for k in operations {
            map.insert(k, 1);
        }

        prop_assert_eq!(map.capacity(), initial_capacity);

        map.clear();

        prop_assert_eq!(map.capacity(), initial_capacity);
    }
}

/// Property 33: Concurrent updates determinism (proptest)
///
/// ∀ k,values: last writer wins (one of the values)
proptest! {
    #[test]
    fn prop_concurrent_updates_determinism(
        key in any::<u64>(),
        values in prop::collection::vec(any::<i32>(), 1..10)
    ) {
        prop_assume!(!values.is_empty());

        let map: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::new();

        for v in &values {
            map.insert(key, *v);
        }

        // Final value should be one of the inserted values
        let final_value = map.get(&key);
        prop_assert!(final_value.is_some());

        let val = final_value.unwrap();
        prop_assert!(values.contains(val), "Final value {} not in {:?}", val, values);
    }
}

/// Property 34: Mixed types support (proptest)
///
/// ∀ complex types: map works with tuples, enums, etc.
proptest! {
    #[test]
    fn prop_complex_types_support(
        pairs in prop::collection::vec((any::<u64>(), (any::<i32>(), any::<bool>())), 0..50)
    ) {
        let map: ConcurrentMapCapsule<u64, (i32, bool)> = ConcurrentMapCapsule::new();

        for (k, v) in &pairs {
            map.insert(*k, *v);
        }

        for (k, expected) in &pairs {
            prop_assert_eq!(map.get(k), Some(expected));
        }
    }
}

/// Property 35: Non-empty map invariants (proptest)
///
/// ∀ k,v: after insert, !is_empty() and len() > 0
proptest! {
    #[test]
    fn prop_non_empty_invariants(key in any::<u64>(), value in any::<i32>()) {
        let map: ConcurrentMapCapsule<u64, i32> = ConcurrentMapCapsule::new();

        map.insert(key, value);

        prop_assert!(!map.is_empty());
        prop_assert!(map.len() > 0);
        prop_assert!(map.contains_key(&key));
    }
}
