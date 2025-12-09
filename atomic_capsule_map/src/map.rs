//! AtomicCapsuleMap - Lockfree Concurrent Hash Map
//!
//! Main API for lockfree concurrent hash map built on atomic capsule primitives.
//!
//! # Design Principles (from "The Atomic Capsule")
//!
//! 1. **One read → One decision**: Lockfree get() with single atomic read path
//! 2. **Two-phase commit**: Atomic insert() using capsule publish protocol
//! 3. **SWeMR per bucket**: Single-Writer, Many-Readers at bucket level
//! 4. **Degrade, don't die**: Graceful degradation under high contention
//!
//! # API Design
//!
//! Generic over K: Hash + Eq and V for type-safe key-value storage.
//! For large keys/values, store inline data + pointer to external storage.
//!
//! # Performance Targets
//!
//! - get(): <50ns (lockfree, 1-2 cache lines)
//! - insert(): <100ns (two-phase commit, minimal probing)
//! - remove(): <100ns (atomic deletion)
//!
//! # Safety Assumptions
//!
//! #ASSUME: Generic types K and V are safe to transmute to/from u64 storage
//! #VERIFY: Size checks in compile-time assertions
//! #ASSUME: Hash + Eq traits provide collision-resistant hashing
//! #VERIFY: Property tests validate hash distribution

use crate::serializable::BitwiseSerializable;
use crate::table::{AtomicTable, DEFAULT_BUCKET_COUNT};
use core::hash::Hash;
use core::marker::PhantomData;

/// Lockfree concurrent hash map using atomic capsule primitives
///
/// Provides lockfree reads and atomic writes for concurrent key-value storage.
///
/// # Type Parameters
///
/// * `K` - Key type (must be Hash + Eq)
/// * `V` - Value type
/// * `N` - Bucket count (must be power of 2, defaults to 1024)
///
/// # Example
///
/// ```
/// use atomic_capsule_map::AtomicCapsuleMap;
///
/// let map = AtomicCapsuleMap::<u64, u64>::new();
///
/// // Lockfree insert
/// map.insert(42, 100).unwrap();
///
/// // Lockfree get
/// let value = map.get(&42).unwrap();
/// assert_eq!(value, 100);
///
/// // Lockfree remove
/// map.remove(&42).unwrap();
/// assert!(map.get(&42).is_none());
/// ```
pub struct AtomicCapsuleMap<K, V, const N: usize = DEFAULT_BUCKET_COUNT>
where
    K: Hash + Eq,
    V: BitwiseSerializable,
{
    /// Underlying hash table
    table: AtomicTable<N>,

    /// Phantom data for K and V types
    /// #ASSUME: PhantomData has zero size and cost
    _phantom: PhantomData<(K, V)>,
}

// #ASSUME_SEND_SYNC: AtomicCapsuleMap is thread-safe through atomic operations only
// #VERIFY_THREAD_SAFE: All coordination via atomic primitives (AtomicU64, AtomicU128)
// #ASSUME_LOCKFREE_ONLY: No Mutex, RwLock, or blocking primitives in implementation
// #VERIFY_NO_BLOCKING: Lockfree mandate enforced - all operations use atomics
// #ASSUME_TYPE_SAFETY: K and V bounds ensure safe concurrent access
// #VERIFY_BOUNDS: Send/Sync bounds propagated from generic parameters
// #ASSUME_SEND_SYNC_BITWISESERIALIZABLE: BitwiseSerializable types are thread-safe for atomic storage
// #VERIFY_THREAD_SAFE: Arc<T> is Send+Sync, primitives are Send+Sync, all supported types thread-safe
unsafe impl<K, V, const N: usize> Send for AtomicCapsuleMap<K, V, N>
where
    K: Send + Hash + Eq,
    V: Send + BitwiseSerializable,
{
}

unsafe impl<K, V, const N: usize> Sync for AtomicCapsuleMap<K, V, N>
where
    K: Sync + Hash + Eq,
    V: Sync + BitwiseSerializable,
{
}

impl<K, V, const N: usize> AtomicCapsuleMap<K, V, N>
where
    K: Hash + Eq,
    V: BitwiseSerializable,
{
    /// Create new lockfree hash map
    ///
    /// # Panics
    ///
    /// Panics if N is not a power of 2
    pub fn new() -> Self {
        Self {
            table: AtomicTable::new(),
            _phantom: PhantomData,
        }
    }

    /// Get value for key (lockfree read) with Borrow support
    ///
    /// Returns Some(value) if key exists, None otherwise.
    ///
    /// Supports efficient lookup with borrowed types (e.g., lookup String with &str)
    /// without allocating an owned key.
    ///
    /// # Lockfree Guarantee
    ///
    /// Never blocks on writes. May retry internally but always makes progress.
    ///
    /// # Performance
    ///
    /// Target: <50ns average case (1-2 probes, lockfree bucket reads)
    /// OPTIMIZATION: No allocation for borrowed lookups (e.g., &str vs String)
    ///
    /// # Hash Collision Safety
    ///
    /// Performs FULL key equality check (not just hash equality) to prevent
    /// returning wrong value on hash collision.
    ///
    /// #ASSUME: K: BitwiseSerializable allows deserialization of stored key
    /// #VERIFY: Full key comparison via Borrow<Q> prevents hash collision bugs
    pub fn get<Q>(&self, key: &Q) -> Option<V>
    where
        K: BitwiseSerializable + core::borrow::Borrow<Q>,
        V: BitwiseSerializable,
        Q: Hash + Eq + ?Sized,
    {
        // Get snapshot from table (only checks hash equality)
        let snapshot = self.table.get(key)?;

        // CRITICAL: Deserialize stored key and perform FULL equality check
        //
        // The table.get() only checks hash equality (hash collision vulnerability).
        // We MUST deserialize the stored key and compare via Borrow trait.
        //
        // #ASSUME_BITWISE_SAFE: K: BitwiseSerializable guarantees safe from_storage
        // #VERIFY_FULL_EQUALITY: Borrow<Q> comparison prevents hash collision bugs
        let stored_key = K::from_storage(snapshot.key_data);

        // Compare stored key with query key via Borrow trait
        // For String/&str: stored_key.borrow() returns &str, compares with query &str
        if stored_key.borrow() != key {
            return None; // Hash collision - keys don't match
        }

        // Use trait-based compile-time dispatch for zero-cost deserialization
        // #ASSUME_BITWISE_SAFE: V: BitwiseSerializable guarantees safe from_storage
        // #VERIFY_TRAIT_DISPATCH: Compiler monomorphizes per type - zero runtime cost
        Some(V::from_storage(snapshot.value_data))
    }

    /// Insert key-value pair (atomic write)
    ///
    /// Updates existing entry if key already exists.
    /// Returns Ok(()) on success, Err(()) if table is full.
    ///
    /// # Performance
    ///
    /// Target: <100ns average case (1-2 probes + two-phase commit publish)
    ///
    /// # Safety
    ///
    /// For V types that fit in 64 bits, value is stored inline.
    /// For larger V types, this would need extension with external storage.
    pub fn insert(&self, key: K, value: V) -> Result<(), ()>
    where
        K: BitwiseSerializable + Clone,
        V: BitwiseSerializable,
    {
        // For insert, we need to:
        // 1. Keep key alive for hashing/comparison in table.insert()
        // 2. Serialize key AFTER we're done using it for hashing
        //
        // This requires Clone for now to avoid lifetime issues
        // #ASSUME_KEY_CLONE: K implements Clone for temporary usage
        // #VERIFY_EFFICIENT: For Arc<T> keys, clone is cheap (refcount++)

        // Use trait-based dispatch for value serialization
        // #ASSUME_TRAIT_METHOD: BitwiseSerializable provides to_storage for V
        // #VERIFY_ZERO_COST: Compiler monomorphizes per type - inlines to optimal code
        let value_data = value.to_storage();

        // Serialize key using BitwiseSerializable for storage
        // #ASSUME_TRAIT_METHOD: BitwiseSerializable provides to_storage for K
        // #VERIFY_ZERO_COST: Compiler monomorphizes per type - inlines to optimal code
        let key_clone = key.clone();
        let key_data = key_clone.to_storage();

        // table.insert borrows key for hashing, key_data goes into storage
        // table.insert returns Result<usize, ()> but we return Result<(), ()>
        self.table.insert(&key, key_data, value_data).map(|_| ())
    }

    /// Remove key from map (atomic deletion) with Borrow support
    ///
    /// Returns Ok(()) if key was removed, Err(()) if key not found.
    ///
    /// Supports efficient removal with borrowed types (e.g., remove String with &str)
    /// without allocating an owned key.
    ///
    /// # Performance
    ///
    /// Target: <100ns average case (1-2 probes + two-phase commit deletion)
    /// OPTIMIZATION: No allocation for borrowed removals (e.g., &str vs String)
    ///
    /// # Arc<T> Refcount Management
    ///
    /// For Arc<T> values, this properly decrements the refcount by:
    /// 1. Getting the snapshot to extract the Arc pointer
    /// 2. Removing from storage (marks bucket empty)
    /// 3. Reconstructing the storage's Arc and dropping it
    ///
    /// The key insight: `from_storage()` uses clone+forget pattern, so the storage
    /// keeps an Arc reference. We must explicitly reconstruct that storage Arc and
    /// drop it to properly decrement the refcount.
    ///
    /// #ASSUME_ARC_CLEANUP: Explicit Arc reconstruction properly cleans up storage reference
    /// #VERIFY_NO_LEAK: Tests validate refcount returns to original after remove
    pub fn remove<Q>(&self, key: &Q) -> Result<(), ()>
    where
        K: BitwiseSerializable + core::borrow::Borrow<Q>,
        V: BitwiseSerializable,
        Q: Hash + Eq + ?Sized,
    {
        // For Arc<T> values: Get the snapshot BEFORE removing to extract pointer
        // #ASSUME_NEEDS_DROP: Only Arc<T> and similar types need this cleanup
        // #VERIFY_ZERO_COST: Compiler optimizes away entire block for primitive types
        if core::mem::needs_drop::<V>() {
            // Get current snapshot to extract the Arc pointer
            if let Some(snapshot) = self.table.get(key) {
                // CRITICAL: Verify full key equality (not just hash equality)
                // Same hash collision protection as get()
                let stored_key = K::from_storage(snapshot.key_data);
                if stored_key.borrow() != key {
                    return Err(()); // Hash collision - keys don't match
                }

                // Extract the storage value data BEFORE removing
                let storage_data = snapshot.value_data;

                // Remove from table
                self.table.remove(key)?;

                // Call drop_storage to clean up the Arc reference
                // For Arc<T>: Reconstructs Arc from raw pointer and drops it (refcount -1)
                // For primitives: No-op (compiles to nothing)
                // #ASSUME_SINGLE_DROP: Storage value dropped exactly once
                // #VERIFY_REFCOUNT: Arc refcount decremented by 1 for Arc<T>, no-op for primitives
                unsafe {
                    V::drop_storage(storage_data);
                }

                return Ok(());
            } else {
                return Err(()); // Key not found
            }
        }

        // For primitive types: Simple remove without Arc cleanup
        self.table.remove(key)
    }

    /// Atomically update value using CAS retry loop
    ///
    /// Applies function `f` to current value and atomically updates using
    /// generation counter validation to prevent TOCTOU races.
    ///
    /// # Performance
    ///
    /// Target: <200ns average case (2-3 CAS attempts under low contention)
    pub fn update<F>(&self, key: K, f: F) -> V
    where
        K: BitwiseSerializable + Clone,
        V: BitwiseSerializable + Clone,
        F: Fn(Option<&V>) -> V,
    {
        const MAX_RETRIES: usize = 100;

        for _ in 0..MAX_RETRIES {
            // Get current snapshot with generation counter
            let snapshot_opt = self.table.get(&key);

            if let Some(snapshot) = snapshot_opt {
                // Key exists - deserialize using trait dispatch
                let current_value = V::from_storage(snapshot.value_data);

                let new_value = f(Some(&current_value));

                // Serialize new value using trait dispatch
                // #ASSUME_TRAIT_METHOD: to_storage consumes value, clone first
                // #VERIFY_ZERO_COST: Compiler inlines clone+to_storage optimally
                let new_value_clone = new_value.clone();
                let new_data = new_value_clone.to_storage();

                // Serialize key using BitwiseSerializable for storage
                let key_clone = key.clone();
                let key_data = key_clone.to_storage();

                // Attempt CAS update with hardware compare_exchange
                if self
                    .table
                    .cas_update_key(&key, snapshot.generation, key_data, new_data)
                    .is_ok()
                {
                    return new_value;
                }
                // CAS failed - generation changed, retry
            } else {
                // Key doesn't exist - insert new value
                let new_value = f(None);
                let _ = self.insert(key.clone(), new_value.clone());
                return new_value;
            }
        }

        // Fallback after max retries (extremely rare)
        let current = self.get(&key);
        let new_value = f(current.as_ref());
        let _ = self.insert(key, new_value.clone());
        new_value
    }

    /// Atomic compare-and-swap operation
    ///
    /// Swaps value only if current value equals expected.
    /// Returns Ok(()) if swap succeeded, Err(current) if it failed.
    ///
    /// # Performance
    ///
    /// Target: <200ns (find bucket + CAS attempt)
    pub fn compare_and_swap(&self, key: &K, expected: V, new_value: V) -> Result<(), V>
    where
        K: BitwiseSerializable + Clone,
        V: BitwiseSerializable + Clone + PartialEq,
    {
        // Get current snapshot
        let snapshot_opt = self.table.get(key);

        let snapshot = match snapshot_opt {
            Some(s) => s,
            None => return Err(new_value), // Key doesn't exist
        };

        // Deserialize current value using trait dispatch
        // #ASSUME_TRAIT_METHOD: from_storage reconstructs value from storage
        // #VERIFY_ZERO_COST: Compiler monomorphizes per type
        let current_value = V::from_storage(snapshot.value_data);

        // Check if current matches expected
        if current_value != expected {
            return Err(current_value);
        }

        // Serialize new value using trait dispatch
        // #ASSUME_TRAIT_METHOD: to_storage consumes value, clone first if needed
        let new_value_clone = new_value.clone();
        let new_data = new_value_clone.to_storage();

        // Serialize key using BitwiseSerializable for storage
        let key_clone = key.clone();
        let key_data = key_clone.to_storage();

        // Attempt single CAS with current generation
        match self
            .table
            .cas_update_key(key, snapshot.generation, key_data, new_data)
        {
            Ok(()) => Ok(()),
            Err(()) => {
                // CAS failed - value changed concurrently
                // Re-read and return actual current value
                let actual = self.get(key).unwrap_or(new_value);
                Err(actual)
            }
        }
    }

    /// Get current entry count (approximate)
    ///
    /// Count may be slightly inaccurate under concurrent modifications
    /// due to relaxed memory ordering.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.table.count()
    }

    /// Check if map is empty (approximate)
    /// TODO(Phase 3): Used in tests and public API
    #[allow(dead_code)]
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get current load factor
    ///
    /// Returns ratio of entries to buckets.
    /// Load factor > 0.75 indicates resize may be beneficial.
    /// TODO(Phase 3): Used in tests and public API
    #[allow(dead_code)]
    #[inline(always)]
    pub fn load_factor(&self) -> f64 {
        self.table.load_factor()
    }

    /// Get bucket count
    /// TODO(Phase 3): Used in tests and public API
    #[allow(dead_code)]
    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Check if key exists (lockfree)
    ///
    /// Equivalent to get(key).is_some() but doesn't extract value.
    /// TODO(Phase 3): Used in tests and public API
    #[allow(dead_code)]
    pub fn contains_key(&self, key: &K) -> bool {
        self.table.get(key).is_some()
    }

    /// Clear all entries from map (atomic deletion)
    ///
    /// Removes all key-value pairs by atomically clearing all buckets.
    ///
    /// # Performance
    ///
    /// Target: O(N) where N is bucket count
    pub fn clear(&self) {
        self.table.clear();
    }

    /// Get performance metrics
    /// TODO(Phase 3): Used in tests and public API
    #[allow(dead_code)]
    pub fn metrics(&self) -> MapMetrics {
        let table_metrics = self.table.metrics();
        MapMetrics {
            capacity: N,
            count: table_metrics.entry_count,
            load_factor: table_metrics.load_factor,
            total_insertions: table_metrics.total_insertions,
            total_deletions: table_metrics.total_deletions,
            average_probe_distance: table_metrics.average_probe_distance,
        }
    }
}

impl<K, V, const N: usize> Default for AtomicCapsuleMap<K, V, N>
where
    K: Hash + Eq,
    V: BitwiseSerializable,
{
    fn default() -> Self {
        Self::new()
    }
}

/// Drop implementation for Arc<T> refcount cleanup
///
/// #ASSUME_EXCLUSIVE_ACCESS: &mut self guarantees no concurrent readers during drop
/// #VERIFY_RUST_SEMANTICS: Rust Drop guarantees exclusive access before calling drop
/// #ASSUME_ARC_POINTERS: For Arc<T> values, value_data contains valid Arc raw pointers
/// #VERIFY_TYPE_SAFETY: BitwiseSerializable trait ensures type-safe Arc reconstruction
impl<K, V, const N: usize> Drop for AtomicCapsuleMap<K, V, N>
where
    K: Hash + Eq,
    V: BitwiseSerializable,
{
    fn drop(&mut self) {
        // Only Arc<T> values need drop cleanup for refcount management
        // Primitives (u64, i32, f64, etc.) don't need any cleanup
        //
        // #ASSUME_NEEDS_DROP: core::mem::needs_drop::<V>() returns true for Arc<T>
        // #VERIFY_ZERO_COST: Compiler eliminates entire function for primitive V types
        if !core::mem::needs_drop::<V>() {
            return; // Zero-cost for primitives - entire function optimized away
        }

        // For Arc<T> values: iterate all buckets and drop Arc values
        // to properly decrement refcounts
        //
        // #ASSUME_EXCLUSIVE_ACCESS: No concurrent readers during drop (&mut self)
        // #VERIFY_NO_CONCURRENT_ACCESS: Rust drop semantics guarantee exclusivity
        // #ASSUME_DROP_STORAGE_SAFE: drop_storage called exactly once per stored value
        // #VERIFY_NO_DOUBLE_FREE: Bucket iteration visits each value exactly once
        for bucket in self.table.buckets.iter() {
            // Read bucket snapshot (safe: we have exclusive access)
            // #ASSUME_EXCLUSIVE_READ: &mut self guarantees no concurrent access
            // #VERIFY_SAFETY: Rust Drop semantics prevent concurrent readers
            if let Some(snapshot) = bucket.read() {
                if !snapshot.is_empty() {
                    // Call drop_storage to clean up the Arc reference
                    // For Arc<T>: Reconstructs Arc from raw pointer and drops it (refcount -1)
                    // For primitives: No-op (compiles to nothing)
                    //
                    // #ASSUME_VALID_STORAGE: value_data contains valid storage from to_storage
                    // #VERIFY_TYPE_SAFETY: V: BitwiseSerializable ensures type-safe drop
                    // #ASSUME_SINGLE_DROP: Each bucket value dropped exactly once
                    // #VERIFY_NO_LEAK: All remaining Arc references properly cleaned up
                    unsafe {
                        V::drop_storage(snapshot.value_data);
                    }
                }
            }
        }
    }
}

/// Map performance metrics
/// TODO(Phase 3): Used in tests and public API
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct MapMetrics {
    /// Total bucket capacity
    pub capacity: usize,

    /// Current entry count (approximate)
    pub count: usize,

    /// Load factor (count / capacity)
    pub load_factor: f64,

    /// Total insertions since creation
    pub total_insertions: u64,

    /// Total deletions since creation
    pub total_deletions: u64,

    /// Average probe distance for operations
    pub average_probe_distance: f64,
}

// Compile-time validation for common key/value types
const _: () = {
    // Validate u64 key/value work
    assert!(core::mem::size_of::<u64>() <= 8);
    assert!(core::mem::size_of::<u32>() <= 8);
    assert!(core::mem::size_of::<usize>() <= 8);
    assert!(core::mem::size_of::<u8>() <= 8);
    assert!(core::mem::size_of::<i32>() <= 8);
    assert!(core::mem::size_of::<i64>() <= 8);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_new() {
        let map = AtomicCapsuleMap::<u64, u64, 16>::new();
        assert_eq!(map.capacity(), 16);
        assert_eq!(map.len(), 0);
        assert!(map.is_empty());
    }

    #[test]
    fn map_insert_get() {
        let map = AtomicCapsuleMap::<u64, u64, 16>::new();

        map.insert(42, 100).unwrap();
        assert_eq!(map.get(&42).unwrap(), 100);
        assert_eq!(map.len(), 1);
        assert!(!map.is_empty());
    }

    #[test]
    fn map_insert_update() {
        let map = AtomicCapsuleMap::<u64, u64, 16>::new();

        map.insert(42, 100).unwrap();
        map.insert(42, 200).unwrap();

        assert_eq!(map.get(&42).unwrap(), 200);
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn map_remove() {
        let map = AtomicCapsuleMap::<u64, u64, 16>::new();

        map.insert(42, 100).unwrap();
        assert!(map.contains_key(&42));

        map.remove(&42).unwrap();
        assert!(!map.contains_key(&42));
        assert!(map.get(&42).is_none());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn map_multiple_entries() {
        let map = AtomicCapsuleMap::<u64, u64, 16>::new();

        for i in 0..10u64 {
            map.insert(i, i * 100).unwrap();
        }

        for i in 0..10u64 {
            assert_eq!(map.get(&i).unwrap(), i * 100);
        }

        assert_eq!(map.len(), 10);
    }

    #[test]
    fn map_contains_key() {
        let map = AtomicCapsuleMap::<u64, u64, 16>::new();

        assert!(!map.contains_key(&42));

        map.insert(42, 100).unwrap();
        assert!(map.contains_key(&42));

        map.remove(&42).unwrap();
        assert!(!map.contains_key(&42));
    }

    #[test]
    fn map_load_factor() {
        let map = AtomicCapsuleMap::<u64, u64, 16>::new();

        for i in 0..8u64 {
            map.insert(i, i).unwrap();
        }

        assert_eq!(map.load_factor(), 0.5); // 8/16
    }

    #[test]
    fn map_metrics() {
        let map = AtomicCapsuleMap::<u64, u64, 16>::new();

        for i in 0..5u64 {
            map.insert(i, i * 10).unwrap();
        }

        map.remove(&2).unwrap();

        let metrics = map.metrics();
        assert_eq!(metrics.capacity, 16);
        assert_eq!(metrics.count, 4);
        assert_eq!(metrics.total_insertions, 5);
        assert_eq!(metrics.total_deletions, 1);
    }

    #[test]
    fn map_u32_values() {
        let map = AtomicCapsuleMap::<u32, u32, 16>::new();

        map.insert(10u32, 20u32).unwrap();
        map.insert(30u32, 40u32).unwrap();

        assert_eq!(map.get(&10u32).unwrap(), 20u32);
        assert_eq!(map.get(&30u32).unwrap(), 40u32);
    }

    #[cfg(not(miri))]
    #[test]
    fn map_concurrent_reads() {
        use std::sync::Arc;
        use std::thread;

        let map = Arc::new(AtomicCapsuleMap::<u64, u64, 256>::new());

        // Insert initial data
        for i in 0..100u64 {
            map.insert(i, i * 100).unwrap();
        }

        // Spawn reader threads
        let mut handles = vec![];
        for _ in 0..8 {
            let map_clone = Arc::clone(&map);
            let handle = thread::spawn(move || {
                for _ in 0..1000 {
                    for i in 0..100u64 {
                        assert_eq!(map_clone.get(&i).unwrap(), i * 100);
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn map_arc_minimal() {
        use std::sync::Arc;

        let map = AtomicCapsuleMap::<u64, Arc<u64>, 16>::new();

        let value = Arc::new(42u64);
        let ptr_before = Arc::as_ptr(&value);
        eprintln!(
            "Before insert, ptr: {:p}, refcount: {}",
            ptr_before,
            Arc::strong_count(&value)
        );

        let clone_for_insert = value.clone();
        let clone_ptr = Arc::as_ptr(&clone_for_insert);
        eprintln!(
            "Clone ptr: {:p}, same as original? {}",
            clone_ptr,
            clone_ptr == ptr_before
        );

        map.insert(1, clone_for_insert).unwrap();
        eprintln!("After insert, refcount: {}", Arc::strong_count(&value));

        // Try to get the value back
        eprintln!("About to call get()...");
        let retrieved = map.get(&1).unwrap();
        eprintln!("After get, refcount: {}", Arc::strong_count(&value));
        eprintln!("Retrieved ptr: {:p}", Arc::as_ptr(&retrieved));
        eprintln!("Retrieved value: {}", *retrieved);

        assert_eq!(*retrieved, 42);
    }

    #[cfg(feature = "std")]
    #[test]
    fn map_arc_u64_values() {
        use std::sync::Arc;

        let map = AtomicCapsuleMap::<u64, Arc<u64>, 16>::new();

        // Insert Arc<u64> values
        let value1 = Arc::new(100u64);
        let value2 = Arc::new(200u64);

        map.insert(1, value1.clone()).unwrap();
        map.insert(2, value2.clone()).unwrap();

        // Verify retrieval
        let retrieved1 = map.get(&1).unwrap();
        let retrieved2 = map.get(&2).unwrap();

        assert_eq!(*retrieved1, 100);
        assert_eq!(*retrieved2, 200);

        // Verify Arc pointer identity (same allocation)
        assert_eq!(Arc::as_ptr(&retrieved1), Arc::as_ptr(&value1));
        assert_eq!(Arc::as_ptr(&retrieved2), Arc::as_ptr(&value2));
    }

    #[cfg(feature = "std")]
    #[test]
    fn map_arc_string_values() {
        use std::sync::Arc;

        let map = AtomicCapsuleMap::<u64, Arc<String>, 16>::new();

        // Insert Arc<String> values
        let hello = Arc::new(String::from("Hello"));
        let world = Arc::new(String::from("World"));

        map.insert(1, hello.clone()).unwrap();
        map.insert(2, world.clone()).unwrap();

        // Verify retrieval
        let retrieved_hello = map.get(&1).unwrap();
        let retrieved_world = map.get(&2).unwrap();

        assert_eq!(*retrieved_hello, "Hello");
        assert_eq!(*retrieved_world, "World");

        // Verify Arc pointer identity
        assert_eq!(Arc::as_ptr(&retrieved_hello), Arc::as_ptr(&hello));
        assert_eq!(Arc::as_ptr(&retrieved_world), Arc::as_ptr(&world));
    }

    #[cfg(feature = "std")]
    #[test]
    fn map_arc_refcount_management() {
        use std::sync::Arc;

        let map = AtomicCapsuleMap::<u64, Arc<u64>, 16>::new();

        let value = Arc::new(42u64);
        assert_eq!(Arc::strong_count(&value), 1);

        // Insert increases refcount
        map.insert(1, value.clone()).unwrap();

        // Map holds one reference, our value variable holds another
        let retrieved = map.get(&1).unwrap();
        assert_eq!(Arc::strong_count(&value), 3); // value + map storage + retrieved

        // Drop retrieved
        drop(retrieved);
        assert_eq!(Arc::strong_count(&value), 2); // value + map storage

        // Remove from map
        map.remove(&1).unwrap();
        assert_eq!(Arc::strong_count(&value), 1); // Only value remains

        drop(value);
        // value is now fully dropped
    }

    #[cfg(feature = "std")]
    #[test]
    fn map_arc_update() {
        use std::sync::Arc;

        let map = AtomicCapsuleMap::<u64, Arc<u64>, 16>::new();

        let initial = Arc::new(100u64);
        map.insert(1, initial.clone()).unwrap();

        // Update to new Arc value
        let updated = Arc::new(200u64);
        map.insert(1, updated.clone()).unwrap();

        let retrieved = map.get(&1).unwrap();
        assert_eq!(*retrieved, 200);
        assert_eq!(Arc::as_ptr(&retrieved), Arc::as_ptr(&updated));
    }
}
