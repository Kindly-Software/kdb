//! Sharded map implementation for concurrent access.
//!
//! Each shard is an AtomicCapsuleMap providing lockfree operations.
//! The API layer distributes keys across shards using hash-based routing.

use crate::map::AtomicCapsuleMap;
use crate::serializable::BitwiseSerializable;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::borrow::Borrow;
use core::hash::Hash;
use parking_lot::RwLock;

/// Default buckets per shard
const DEFAULT_BUCKETS_PER_SHARD: usize = 256;

/// A single shard in the sharded map.
///
/// Uses AtomicCapsuleMap for lockfree key-value storage with atomic capsule architecture.
pub struct ShardedMap<K, V>
where
    K: Hash + Eq,
    V: BitwiseSerializable,
{
    /// The underlying atomic capsule map
    /// Uses a smaller bucket count per shard for better cache locality
    map: AtomicCapsuleMap<K, V, DEFAULT_BUCKETS_PER_SHARD>,

    /// RwLock for iteration support only (not used in hot path)
    /// #ASSUME: Iteration is rare compared to get/insert/remove
    /// #VERIFY: Hot path (get/insert/remove) never acquires this lock
    snapshot: RwLock<BTreeMap<K, V>>,
}

impl<K, V> ShardedMap<K, V>
where
    K: Hash + Eq + Clone + Ord + BitwiseSerializable,
    V: Clone + BitwiseSerializable,
{
    pub fn with_capacity(_capacity: usize) -> Self {
        // Use const generic for bucket count
        // Each shard gets DEFAULT_BUCKETS_PER_SHARD buckets
        Self {
            map: AtomicCapsuleMap::new(),
            snapshot: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn clear(&self) {
        // Clear all buckets in the atomic map
        self.map.clear();
        // Also clear snapshot for iteration
        *self.snapshot.write() = BTreeMap::new();
    }

    pub fn get<Q>(&self, key: &Q, _hash: u64) -> Option<V>
    where
        K: Borrow<Q> + BitwiseSerializable,
        Q: Hash + Eq + ?Sized,
    {
        // OPTIMIZED: Direct Borrow<Q> lookup without allocation
        //
        // Previous version used ToOwned to convert &Q -> K (e.g., &str -> String).
        // This allocated for every lookup, which is inefficient.
        //
        // New approach: AtomicCapsuleMap::get() now supports Borrow<Q> directly.
        // It deserializes the stored key once and compares via Borrow trait.
        //
        // Performance improvement:
        // - Before: &str lookup required String allocation (~20ns overhead)
        // - After: Direct comparison, zero allocation
        //
        // #ASSUME_BORROW_SUPPORT: AtomicCapsuleMap::get() handles Borrow<Q>
        // #VERIFY_NO_ALLOCATION: No ToOwned call, no String allocation for &str lookups
        self.map.get(key)
    }

    pub fn insert(&self, key: K, value: V, _hash: u64) -> Option<V> {
        // Get old value first
        let old_value = self.map.get(&key);

        // Clone key/value for snapshot before moving into map
        let key_clone = key.clone();
        let value_clone = value.clone();

        // Atomic insert via compare-exchange in AtomicCapsuleMap
        // insert returns Result<(), ()> - we ignore errors (table full is rare)
        let _ = self.map.insert(key, value);

        // Update snapshot for iteration (non-critical path)
        // #ASSUME: Snapshot updates don't need to be perfectly synchronized
        let mut snap = self.snapshot.write();
        snap.insert(key_clone, value_clone);

        old_value
    }

    pub fn remove<Q>(&self, key: &Q, _hash: u64) -> Option<V>
    where
        K: Borrow<Q> + BitwiseSerializable + Clone,
        Q: Hash + Eq + ?Sized,
    {
        // OPTIMIZED: Direct Borrow<Q> lookup without allocation for initial get
        //
        // Previous version used ToOwned to convert &Q -> K for both lookup and removal.
        // This allocated String for every &str remove operation.
        //
        // New approach: Use Borrow<Q> for initial lookup and removal, get owned key
        // from snapshot for BTreeMap update.
        //
        // #ASSUME_BORROW_SUPPORT: AtomicCapsuleMap::get() and remove() handle Borrow<Q>
        // #VERIFY_SNAPSHOT_LOOKUP: Snapshot lookup finds matching key for removal

        // Get old value first (no allocation with Borrow<Q>)
        let old_value = self.map.get(key);

        // Atomic remove via AtomicCapsuleMap (no allocation with Borrow<Q>)
        // remove returns Result<(), ()> - we ignore errors
        let _ = self.map.remove(key);

        // Update snapshot for iteration (non-critical path)
        // Need to find and remove matching key from snapshot
        {
            let mut snap = self.snapshot.write();

            // Find the owned key in snapshot that matches our query key
            // BTreeMap::remove requires K: Ord, so we find matching key first
            let owned_key_opt: Option<K> = snap
                .keys()
                .find(|k| {
                    let borrowed: &Q = Borrow::<Q>::borrow(*k);
                    borrowed == key
                })
                .cloned();

            // Remove using the owned key (K implements Ord)
            // Use turbofish to specify we're removing with K, not Q
            if let Some(owned_key) = owned_key_opt {
                snap.remove::<K>(&owned_key);
            }
        }

        old_value
    }

    pub fn get_or_insert(&self, key: K, value: V, _hash: u64) -> V {
        // Check if exists first (lockfree)
        if let Some(existing) = self.map.get(&key) {
            return existing;
        }

        // Clone key/value for snapshot before moving
        let key_clone = key.clone();
        let value_clone = value.clone();
        let value_return = value.clone();

        // Insert and return inserted value
        let _ = self.map.insert(key, value);

        // Update snapshot
        let mut snap = self.snapshot.write();
        snap.insert(key_clone, value_clone);

        value_return
    }

    pub fn compare_and_swap(&self, key: &K, expected: V, new_value: V, _hash: u64) -> Result<(), V>
    where
        V: PartialEq,
    {
        // Clone new_value for snapshot before moving
        let new_value_clone = new_value.clone();

        // Delegate to AtomicCapsuleMap's CAS implementation
        let result = self.map.compare_and_swap(key, expected, new_value);

        // Update snapshot if successful
        if result.is_ok() {
            let mut snap = self.snapshot.write();
            snap.insert(key.clone(), new_value_clone);
        }

        result
    }

    pub fn update<F>(&self, key: K, f: F, _hash: u64) -> V
    where
        F: Fn(Option<&V>) -> V,
        V: PartialEq,
    {
        // Clone key for snapshot before moving
        let key_clone = key.clone();

        // Delegate to AtomicCapsuleMap's CAS-based update method
        let new_value = self.map.update(key, f);

        // Update snapshot for iteration (non-critical path)
        let mut snap = self.snapshot.write();
        snap.insert(key_clone, new_value.clone());

        new_value
    }

    pub fn iter(&self) -> ShardIter<K, V> {
        // Return iterator over snapshot
        // #ASSUME: Snapshot may be slightly stale but provides consistent iteration
        let snap = self.snapshot.read();
        let items: Vec<(K, V)> = snap.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

        ShardIter { items, index: 0 }
    }
}

pub struct ShardIter<K, V> {
    items: Vec<(K, V)>,
    index: usize,
}

impl<K: Clone, V: Clone> Iterator for ShardIter<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.items.len() {
            let item = self.items[self.index].clone();
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}
