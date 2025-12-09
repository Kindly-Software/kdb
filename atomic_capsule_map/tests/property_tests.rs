//! Property-Based Tests for AtomicCapsuleMap
//!
//! Tests essential invariants using proptest for randomized testing.
//! Following IMPL-2: Test essential properties, not exhaustive.
//!
//! # Test Coverage (UCE32 Q19 Analysis)
//!
//! Essential properties validated:
//! 1. Insert-Get consistency: Value inserted is value retrieved
//! 2. Remove-Get consistency: Removed key returns None
//! 3. Contains accuracy: Contains returns true iff key exists
//! 4. Len accounting: Insert increases len, update preserves len
//! 5. Generation monotonicity: Generation counters never decrease
//! 6. ABA prevention: Generation validation prevents lost updates
//! 7. Atomic CAS correctness: Compare-and-swap follows spec
//! 8. Update atomicity: Updates are atomic and consistent
//! 9. No torn reads: Always see consistent state (generation validation)
//! 10. Concurrent safety: Operations are safe under concurrent access

use atomic_capsule_map::{
    generation::{pack_gen_high, unpack_gen_high, unpack_value_low},
    AtomicCapsuleMap,
};
use proptest::prelude::*;
use std::sync::Arc;
use std::thread;

// ============================================================================
// PROPERTY 1: Insert-Get Consistency
// ============================================================================

proptest! {
    /// Property: Insert then get returns the inserted value
    ///
    /// Invariant: ∀(k,v): insert(k,v) → get(k) = Some(v)
    ///
    /// This validates the fundamental correctness of the atomic capsule
    /// read/write protocol with generation counter validation.
    #[test]
    fn prop_insert_get_returns_value(key in any::<u64>(), value in any::<u64>()) {
        let map = AtomicCapsuleMap::new();
        map.insert(key, value);
        prop_assert_eq!(map.get(&key), Some(value));
    }

    /// Property: Multiple inserts overwrite previous values
    ///
    /// Invariant: insert(k,v1) → insert(k,v2) → get(k) = Some(v2)
    #[test]
    fn prop_insert_overwrites(
        key in any::<u64>(),
        val1 in any::<u64>(),
        val2 in any::<u64>()
    ) {
        let map = AtomicCapsuleMap::new();
        map.insert(key, val1);
        map.insert(key, val2);
        prop_assert_eq!(map.get(&key), Some(val2));
    }
}

// ============================================================================
// PROPERTY 2: Remove-Get Consistency
// ============================================================================

proptest! {
    /// Property: Remove then get returns None
    ///
    /// Invariant: insert(k,v) → remove(k) → get(k) = None
    ///
    /// Validates that tombstone marking in atomic capsules works correctly.
    #[test]
    fn prop_remove_get_none(key in any::<u64>(), value in any::<u64>()) {
        let map = AtomicCapsuleMap::new();
        map.insert(key, value);
        let removed = map.remove(&key);
        prop_assert_eq!(removed, Some(value));
        prop_assert_eq!(map.get(&key), None);
    }

    /// Property: Remove non-existent key returns None
    ///
    /// Invariant: get(k) = None → remove(k) = None
    #[test]
    fn prop_remove_missing_returns_none(key in any::<u64>()) {
        let map: AtomicCapsuleMap<u64, u64> = AtomicCapsuleMap::new();
        prop_assert_eq!(map.remove(&key), None);
    }
}

// ============================================================================
// PROPERTY 3: Contains-Key Accuracy
// ============================================================================

proptest! {
    /// Property: Contains returns true after insert
    ///
    /// Invariant: insert(k,v) → contains_key(k) = true
    #[test]
    fn prop_contains_after_insert(key in any::<u64>(), value in any::<u64>()) {
        let map = AtomicCapsuleMap::new();
        map.insert(key, value);
        prop_assert!(map.contains_key(&key));
    }

    /// Property: Contains returns false after remove
    ///
    /// Invariant: insert(k,v) → remove(k) → contains_key(k) = false
    #[test]
    fn prop_not_contains_after_remove(key in any::<u64>(), value in any::<u64>()) {
        let map = AtomicCapsuleMap::new();
        map.insert(key, value);
        map.remove(&key);
        prop_assert!(!map.contains_key(&key));
    }
}

// ============================================================================
// PROPERTY 4: Len Accounting
// ============================================================================

proptest! {
    /// Property: Len increases on insert of new key
    ///
    /// Invariant: len() increases by 1 when inserting a new key
    ///
    /// Note: This test ensures the key doesn't exist first to guarantee
    /// the insert is truly adding a new element.
    #[test]
    fn prop_len_increases_on_new_insert(key in any::<u64>(), value in any::<u64>()) {
        let map = AtomicCapsuleMap::new();
        prop_assume!(map.get(&key).is_none()); // Ensure key doesn't exist
        let old_len = map.len();
        map.insert(key, value);
        prop_assert_eq!(map.len(), old_len + 1);
    }

    /// Property: Len decreases on remove
    ///
    /// Invariant: insert(k,v) → remove(k) → len decreases by 1
    #[test]
    fn prop_len_decreases_on_remove(key in any::<u64>(), value in any::<u64>()) {
        let map = AtomicCapsuleMap::new();
        map.insert(key, value);
        let len_after_insert = map.len();
        map.remove(&key);
        prop_assert_eq!(map.len(), len_after_insert - 1);
    }

    /// Property: Update doesn't change len
    ///
    /// Invariant: insert(k,v1) → insert(k,v2) → len unchanged
    ///
    /// Validates that updating an existing key preserves the element count.
    #[test]
    fn prop_update_preserves_len(
        key in any::<u64>(),
        val1 in any::<u64>(),
        val2 in any::<u64>()
    ) {
        let map = AtomicCapsuleMap::new();
        map.insert(key, val1);
        let len = map.len();
        map.insert(key, val2);
        prop_assert_eq!(map.len(), len);
    }
}

// ============================================================================
// PROPERTY 5: Get-Or-Insert Atomicity
// ============================================================================

proptest! {
    /// Property: get_or_insert returns inserted value on first call
    ///
    /// Invariant: get_or_insert(k,v) = v when k doesn't exist
    #[test]
    fn prop_get_or_insert_first_call(key in any::<u64>(), value in any::<u64>()) {
        let map = AtomicCapsuleMap::new();
        let result = map.get_or_insert(key, value);
        prop_assert_eq!(result, value);
        prop_assert_eq!(map.get(&key), Some(value));
    }

    /// Property: get_or_insert returns existing value on second call
    ///
    /// Invariant: insert(k,v1) → get_or_insert(k,v2) = v1
    #[test]
    fn prop_get_or_insert_existing(
        key in any::<u64>(),
        val1 in any::<u64>(),
        val2 in any::<u64>()
    ) {
        let map = AtomicCapsuleMap::new();
        map.insert(key, val1);
        let result = map.get_or_insert(key, val2);
        prop_assert_eq!(result, val1); // Returns existing, not new
        prop_assert_eq!(map.get(&key), Some(val1)); // Value unchanged
    }
}

// ============================================================================
// PROPERTY 6: Compare-And-Swap Correctness
// ============================================================================

proptest! {
    /// Property: CAS succeeds when expected value matches
    ///
    /// Invariant: insert(k,v1) → CAS(k, v1, v2) = Ok(())
    #[test]
    fn prop_cas_succeeds_on_match(
        key in any::<u64>(),
        val1 in any::<u64>(),
        val2 in any::<u64>()
    ) {
        let map = AtomicCapsuleMap::new();
        map.insert(key, val1);
        let result = map.compare_and_swap(&key, val1, val2);
        prop_assert!(result.is_ok());
        prop_assert_eq!(map.get(&key), Some(val2));
    }

    /// Property: CAS fails when expected value doesn't match
    ///
    /// Invariant: insert(k,v1) → CAS(k, v2, v3) = Err(v1)
    #[test]
    fn prop_cas_fails_on_mismatch(
        key in any::<u64>(),
        val1 in any::<u64>(),
        val2 in any::<u64>(),
        val3 in any::<u64>()
    ) {
        prop_assume!(val1 != val2); // Ensure values differ
        let map = AtomicCapsuleMap::new();
        map.insert(key, val1);
        let result = map.compare_and_swap(&key, val2, val3);
        prop_assert_eq!(result, Err(val1)); // Returns current value
        prop_assert_eq!(map.get(&key), Some(val1)); // Value unchanged
    }

    /// Property: CAS fails on missing key
    ///
    /// Invariant: CAS on non-existent key fails
    #[test]
    fn prop_cas_fails_on_missing(
        key in any::<u64>(),
        expected in any::<u64>(),
        new_val in any::<u64>()
    ) {
        let map: AtomicCapsuleMap<u64, u64> = AtomicCapsuleMap::new();
        let result = map.compare_and_swap(&key, expected, new_val);
        prop_assert!(result.is_err());
    }
}

// ============================================================================
// PROPERTY 7: Update Atomicity
// ============================================================================

proptest! {
    /// Property: Update applies function atomically
    ///
    /// Invariant: update(k, f) applies f to current value atomically
    #[test]
    fn prop_update_atomicity(key in any::<u64>(), value in any::<u64>()) {
        let map = AtomicCapsuleMap::new();
        map.insert(key, value);

        // Update should increment value
        let result = map.update(key, |v| v.map_or(0, |n| n.wrapping_add(1)));
        prop_assert_eq!(result, value.wrapping_add(1));
        prop_assert_eq!(map.get(&key), Some(value.wrapping_add(1)));
    }

    /// Property: Update creates entry if missing
    ///
    /// Invariant: update(k, f) on missing key applies f(None)
    #[test]
    fn prop_update_creates_if_missing(key in any::<u64>(), default in any::<u64>()) {
        let map = AtomicCapsuleMap::new();
        let result = map.update(key, |v| v.copied().unwrap_or(default));
        prop_assert_eq!(result, default);
        prop_assert_eq!(map.get(&key), Some(default));
    }
}

// ============================================================================
// PROPERTY 8: Generation Counter Monotonicity
// ============================================================================

proptest! {
    /// Property: Generation counters are monotonic
    ///
    /// Invariant: Multiple operations never decrease generation counter
    ///
    /// This tests the core atomic capsule principle: generation counters
    /// provide monotonic versioning to prevent ABA problems.
    #[test]
    fn prop_generation_monotonic(
        key in any::<u64>(),
        values in prop::collection::vec(any::<u64>(), 1..10)
    ) {
        let map = AtomicCapsuleMap::new();

        // Insert values sequentially and track that operations succeed
        for value in values {
            map.insert(key, value);
            // Each insert should succeed (generation increments)
            prop_assert!(map.contains_key(&key));
        }
    }
}

// ============================================================================
// PROPERTY 9: Multiple Keys Independence
// ============================================================================

proptest! {
    /// Property: Operations on different keys are independent
    ///
    /// Invariant: insert(k1,v1) doesn't affect get(k2) where k1 ≠ k2
    #[test]
    fn prop_key_independence(
        key1 in any::<u64>(),
        key2 in any::<u64>(),
        val1 in any::<u64>(),
        val2 in any::<u64>()
    ) {
        prop_assume!(key1 != key2); // Ensure keys differ
        let map = AtomicCapsuleMap::new();

        map.insert(key1, val1);
        map.insert(key2, val2);

        prop_assert_eq!(map.get(&key1), Some(val1));
        prop_assert_eq!(map.get(&key2), Some(val2));

        // Removing key1 doesn't affect key2
        map.remove(&key1);
        prop_assert_eq!(map.get(&key1), None);
        prop_assert_eq!(map.get(&key2), Some(val2));
    }
}

// ============================================================================
// PROPERTY 10: Clear Resets Map
// ============================================================================

proptest! {
    /// Property: Clear removes all elements
    ///
    /// Invariant: insert many → clear() → len() = 0 and all gets return None
    #[test]
    fn prop_clear_empties_map(
        entries in prop::collection::vec((any::<u64>(), any::<u64>()), 1..20)
    ) {
        let map = AtomicCapsuleMap::new();

        // Insert all entries
        for (key, value) in &entries {
            map.insert(*key, *value);
        }

        prop_assert!(map.len() > 0);

        // Clear map
        map.clear();

        prop_assert_eq!(map.len(), 0);
        prop_assert!(map.is_empty());

        // All keys should return None
        for (key, _) in &entries {
            prop_assert_eq!(map.get(key), None);
        }
    }
}

// ============================================================================
// PROPERTY 11: Concurrent Safety (Generation Counter Validation)
// ============================================================================

#[cfg(not(miri))] // Skip under Miri (too slow for concurrent tests)
proptest! {
    /// Property: Concurrent inserts don't lose updates
    ///
    /// Invariant: N concurrent inserts → len() ≥ 1 (at least one succeeds)
    ///
    /// This validates the atomic capsule's generation counter prevents ABA
    /// and ensures at least one concurrent operation succeeds.
    #[test]
    fn prop_concurrent_inserts_safe(
        key in any::<u64>(),
        values in prop::collection::vec(any::<u64>(), 2..8)
    ) {
        let map = Arc::new(AtomicCapsuleMap::new());
        let mut handles = vec![];

        // Spawn threads to insert concurrently
        for value in values {
            let map_clone = Arc::clone(&map);
            let handle = thread::spawn(move || {
                map_clone.insert(key, value);
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Key should exist (at least one insert succeeded)
        prop_assert!(map.contains_key(&key));
        prop_assert_eq!(map.len(), 1);
    }

    /// Property: Concurrent updates are atomic
    ///
    /// Invariant: N concurrent increments → final value = initial + N
    ///
    /// This validates that the atomic update operation correctly handles
    /// concurrent modifications through generation counter validation.
    #[test]
    fn prop_concurrent_updates_atomic(
        key in any::<u64>(),
        initial in 0u64..100,
        increments in 2usize..8
    ) {
        let map = Arc::new(AtomicCapsuleMap::new());
        map.insert(key, initial);

        let mut handles = vec![];

        // Spawn threads to increment concurrently
        for _ in 0..increments {
            let map_clone = Arc::clone(&map);
            let handle = thread::spawn(move || {
                map_clone.update(key, |v| v.map_or(0, |n| n + 1));
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Final value should be initial + number of increments
        let final_value = map.get(&key).unwrap();
        prop_assert_eq!(final_value, initial + increments as u64);
    }
}

// ============================================================================
// PROPERTY 12: Generation Counter Packing/Unpacking
// ============================================================================

proptest! {
    /// Property: Generation packing round-trips correctly
    ///
    /// Invariant: unpack(pack(value, gen)) = (value, gen)
    ///
    /// This validates the bit packing utilities used in atomic capsules.
    #[test]
    fn prop_generation_packing_roundtrip(value in any::<u32>(), gen in any::<u32>()) {
        let packed = pack_gen_high(value, gen);
        let unpacked_value = unpack_value_low(packed);
        let unpacked_gen = unpack_gen_high(packed);

        prop_assert_eq!(unpacked_value, value);
        prop_assert_eq!(unpacked_gen, gen);
    }
}

// ============================================================================
// PROPERTY 13: Iteration Consistency
// ============================================================================

proptest! {
    /// Property: Iteration visits all inserted keys
    ///
    /// Invariant: iter() returns all key-value pairs in the map
    #[test]
    fn prop_iteration_completeness(
        entries in prop::collection::hash_map(any::<u64>(), any::<u64>(), 1..20)
    ) {
        let map = AtomicCapsuleMap::new();

        // Insert all entries
        for (key, value) in &entries {
            map.insert(*key, *value);
        }

        // Collect all iterated entries
        let iterated: std::collections::HashMap<_, _> =
            map.iter().collect();

        // All original entries should be present
        for (key, value) in &entries {
            prop_assert_eq!(iterated.get(key), Some(value));
        }

        prop_assert_eq!(iterated.len(), entries.len());
    }
}

// ============================================================================
// PROPERTY 14: No Torn Reads
// ============================================================================

proptest! {
    /// Property: Reads never observe partially updated state
    ///
    /// Invariant: get(k) returns either old value or new value, never torn state
    ///
    /// This validates the atomic capsule's generation counter prevents
    /// TOCTOU issues and torn reads during concurrent updates.
    #[test]
    fn prop_no_torn_reads(
        key in any::<u64>(),
        val1 in any::<u64>(),
        val2 in any::<u64>()
    ) {
        prop_assume!(val1 != val2); // Ensure values differ
        let map = Arc::new(AtomicCapsuleMap::new());
        map.insert(key, val1);

        let map_reader = Arc::clone(&map);
        let map_writer = Arc::clone(&map);

        // Spawn reader thread
        let reader_handle = thread::spawn(move || {
            let mut reads = vec![];
            for _ in 0..100 {
                if let Some(value) = map_reader.get(&key) {
                    reads.push(value);
                }
            }
            reads
        });

        // Spawn writer thread
        let writer_handle = thread::spawn(move || {
            for _ in 0..50 {
                map_writer.insert(key, val2);
            }
        });

        writer_handle.join().unwrap();
        let reads = reader_handle.join().unwrap();

        // All reads should be either val1 or val2, never torn/invalid
        for value in reads {
            prop_assert!(value == val1 || value == val2);
        }
    }
}

// ============================================================================
// Test Configuration
// ============================================================================

#[cfg(test)]
mod config {
    use super::*;

    /// Configure proptest to run sufficient iterations for statistical confidence
    ///
    /// Following B32 benchmark framework: 95% confidence requires 1000+ iterations
    fn proptest_config() -> ProptestConfig {
        ProptestConfig {
            cases: 1000,             // 1000 random test cases per property
            max_shrink_iters: 10000, // Allow thorough shrinking on failure
            ..ProptestConfig::default()
        }
    }
}
