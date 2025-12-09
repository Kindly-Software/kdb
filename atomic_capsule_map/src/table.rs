//! Hash Table Core Structure
//!
//! Manages the array of BucketCapsules with atomic resize coordination.
//! Uses open addressing with linear probing for collision resolution.
//!
//! # Design Principles
//!
//! 1. **Fixed-Size Buckets**: No dynamic allocation in hot path
//! 2. **Linear Probing**: Simple, cache-friendly collision resolution
//! 3. **Power-of-Two Sizing**: Fast modulo via bitwise AND
//! 4. **Lockfree Reads**: All reads are lockfree with retry
//!
//! # Safety Assumptions
//!
//! #ASSUME: Power-of-two bucket count enables fast hash->index mapping
//! #VERIFY: Compile-time assertions validate bucket count
//! #ASSUME: Linear probing terminates within bucket array size
//! #VERIFY: Property tests validate probe termination

use crate::bucket::{BucketCapsule, BucketSnapshot};
use ahash::AHasher;
use alloc::alloc::{alloc_zeroed, handle_alloc_error, Layout};
use alloc::boxed::Box;
use core::hash::{Hash, Hasher};
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Default bucket count (power of 2)
pub const DEFAULT_BUCKET_COUNT: usize = 1024;

/// Maximum probe distance before giving up
/// #ASSUME: Table is sized such that linear probing finds slot within this distance
/// #VERIFY: Benchmarks validate probe distance distribution
/// Increased to 128 to handle high load factors (>80%) gracefully
const MAX_PROBE_DISTANCE: usize = 128;

/// Hash table containing array of bucket capsules
///
/// Provides lockfree reads and atomic writes using bucket-level
/// atomic capsules with two-phase commit.
pub struct AtomicTable<const N: usize = DEFAULT_BUCKET_COUNT> {
    /// Array of bucket capsules
    /// #ASSUME: Const generic N is power of 2
    /// #VERIFY: Const assertion validates power-of-two
    pub(crate) buckets: Box<[BucketCapsule; N]>,

    /// Bitmask for fast modulo (N - 1)
    /// Precomputed for zero-cost hash->index mapping
    mask: usize,

    /// Count of entries currently in table
    /// Used for load factor calculation and resize decisions
    /// #ASSUME: Relaxed ordering sufficient for approximate count
    /// #VERIFY: Load factor metrics validate count accuracy
    count: AtomicUsize,

    /// Total number of insertions (for metrics)
    total_insertions: AtomicU64,

    /// Total number of deletions (for metrics)
    total_deletions: AtomicU64,

    /// Total probe distance (for metrics)
    total_probe_distance: AtomicU64,
}

impl<const N: usize> AtomicTable<N> {
    /// Create new hash table
    ///
    /// # Panics
    ///
    /// Panics if N is not a power of 2
    pub fn new() -> Self {
        assert!(N.is_power_of_two(), "Bucket count must be power of 2");
        assert!(N > 0, "Bucket count must be > 0");

        // Create array of buckets
        // #ASSUME: Box allocation succeeds (OOM is unrecoverable)
        let buckets = unsafe {
            let layout = Layout::new::<[BucketCapsule; N]>();
            let ptr = alloc_zeroed(layout) as *mut [BucketCapsule; N];
            if ptr.is_null() {
                handle_alloc_error(layout);
            }
            Box::from_raw(ptr)
        };

        Self {
            buckets,
            mask: N - 1,
            count: AtomicUsize::new(0),
            total_insertions: AtomicU64::new(0),
            total_deletions: AtomicU64::new(0),
            total_probe_distance: AtomicU64::new(0),
        }
    }

    /// Get bucket count
    /// TODO(Phase 3): Used in metrics and debugging
    #[allow(dead_code)]
    #[inline(always)]
    pub const fn bucket_count(&self) -> usize {
        N
    }

    /// Get current entry count (approximate)
    #[inline(always)]
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }

    /// Get current load factor (approximate)
    /// TODO(Phase 3): Used in metrics and load monitoring
    #[allow(dead_code)]
    #[inline(always)]
    pub fn load_factor(&self) -> f64 {
        self.count() as f64 / N as f64
    }

    /// Hash key to bucket index
    ///
    /// Uses high-quality hash function and power-of-two masking
    /// for fast, well-distributed bucket selection.
    ///
    /// Supports borrowing unsized types (e.g., &str for String keys).
    ///
    /// # Performance
    ///
    /// Target: <5ns (single hash + bitwise AND)
    #[inline(always)]
    fn hash_to_index<Q: Hash + ?Sized>(&self, key: &Q) -> usize {
        let mut hasher = AHasher::default();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        // Use high bits for better distribution
        ((hash >> 32) as usize) & self.mask
    }

    /// Extract 24-bit key hash for bucket storage
    ///
    /// Supports borrowing unsized types (e.g., &str for String keys).
    #[inline(always)]
    fn key_hash<Q: Hash + ?Sized>(key: &Q) -> u32 {
        let mut hasher = AHasher::default();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        // Use low 24 bits
        (hash & 0x00FF_FFFF) as u32
    }

    /// Linear probe to find bucket for key
    ///
    /// Returns (bucket_index, probe_distance, snapshot)
    ///
    /// Supports borrowing unsized types (e.g., &str for String keys).
    ///
    /// # Lockfree Guarantee
    ///
    /// Reads are lockfree - never blocks on writes.
    /// May return None if concurrent modifications prevent consistent read.
    ///
    /// # Performance
    ///
    /// Target: <50ns for average case (1-2 probes)
    /// Target: <500ns for worst case (MAX_PROBE_DISTANCE probes)
    fn find_bucket<Q: Hash + Eq + ?Sized>(
        &self,
        key: &Q,
        key_hash: u32,
    ) -> Option<(usize, usize, BucketSnapshot)> {
        let start_idx = self.hash_to_index(key);

        for probe_dist in 0..MAX_PROBE_DISTANCE {
            let idx = (start_idx + probe_dist) & self.mask;
            let snapshot = self.buckets[idx].read()?;

            // Empty bucket - key not found
            if snapshot.is_empty() {
                return Some((idx, probe_dist, snapshot));
            }

            // Check if this bucket matches our key
            if snapshot.key_hash == key_hash {
                // Hash matches - this is likely our key
                // (Full key comparison would happen in map layer)
                return Some((idx, probe_dist, snapshot));
            }

            // Continue probing
        }

        // Exceeded max probe distance - table may be too full
        None
    }

    /// Find empty bucket for insertion
    ///
    /// Linear probe starting from hash index to find empty slot.
    /// TODO(Phase 3): Used in advanced insertion strategies
    #[allow(dead_code)]
    fn find_empty_bucket(&self, key_hash: u32) -> Option<usize> {
        let start_idx = (key_hash as usize) & self.mask;

        for probe_dist in 0..MAX_PROBE_DISTANCE {
            let idx = (start_idx + probe_dist) & self.mask;
            let snapshot = self.buckets[idx].read()?;

            if snapshot.is_empty() {
                return Some(idx);
            }
        }

        None
    }

    /// Insert entry into table
    ///
    /// Returns Ok(bucket_index) on success, Err if table is too full.
    ///
    /// # Performance
    ///
    /// Target: <100ns for average case (1-2 probes + publish)
    pub fn insert<K: Hash + Eq>(
        &self,
        key: &K,
        key_data: u64,
        value_data: u64,
    ) -> Result<usize, ()> {
        let key_hash = Self::key_hash(key);
        let start_idx = self.hash_to_index(key);

        // Retry loop for concurrent insert conflicts (IMPL-2: simple retry)
        for _attempt in 0..10 {
            // Scan probe chain: find existing key OR first empty slot
            let mut first_empty_idx = None;
            let mut first_empty_dist = 0;

            for probe_dist in 0..MAX_PROBE_DISTANCE {
                let idx = (start_idx + probe_dist) & self.mask;
                let snapshot = self.buckets[idx].read().ok_or(())?;

                // If we find the key already exists, update it
                if !snapshot.is_empty() && snapshot.key_hash == key_hash {
                    self.buckets[idx].publish(key_hash, key_data, value_data);
                    self.total_probe_distance
                        .fetch_add(probe_dist as u64, Ordering::Relaxed);
                    // NOTE: Count is NOT incremented on update - key already exists
                    return Ok(idx);
                }

                // Remember first empty slot
                if snapshot.is_empty() && first_empty_idx.is_none() {
                    first_empty_idx = Some(idx);
                    first_empty_dist = probe_dist;
                }
            }

            // If we found an empty slot, try to publish there
            if let Some(idx) = first_empty_idx {
                // Re-check the slot is still empty before publishing
                let recheck = self.buckets[idx].read().ok_or(())?;
                if recheck.is_empty() {
                    self.buckets[idx].publish(key_hash, key_data, value_data);
                    self.count.fetch_add(1, Ordering::Relaxed);
                    self.total_insertions.fetch_add(1, Ordering::Relaxed);
                    self.total_probe_distance
                        .fetch_add(first_empty_dist as u64, Ordering::Relaxed);
                    return Ok(idx);
                }
                // Slot filled by another thread, retry
                continue;
            }

            // Table too full
            return Err(());
        }

        // Max retries exceeded (extremely rare)
        Err(())
    }

    /// Remove entry from table
    ///
    /// Returns Ok(()) if entry found and removed, Err(()) if not found.
    ///
    /// Supports borrowing unsized types (e.g., &str for String keys).
    pub fn remove<Q: Hash + Eq + ?Sized>(&self, key: &Q) -> Result<(), ()> {
        let key_hash = Self::key_hash(key);

        if let Some((idx, probe_dist, snapshot)) = self.find_bucket(key, key_hash) {
            if !snapshot.is_empty() && snapshot.key_hash == key_hash {
                self.buckets[idx].remove();
                self.count.fetch_sub(1, Ordering::Relaxed);
                self.total_deletions.fetch_add(1, Ordering::Relaxed);
                self.total_probe_distance
                    .fetch_add(probe_dist as u64, Ordering::Relaxed);
                return Ok(());
            }
        }

        Err(())
    }

    /// Get entry from table (lockfree read)
    ///
    /// Returns snapshot of bucket if found.
    ///
    /// Supports borrowing unsized types (e.g., &str for String keys).
    ///
    /// # Performance
    ///
    /// Target: <50ns for average case (1-2 probes, lockfree reads)
    pub fn get<Q: Hash + Eq + ?Sized>(&self, key: &Q) -> Option<BucketSnapshot> {
        let key_hash = Self::key_hash(key);

        let (_idx, probe_dist, snapshot) = self.find_bucket(key, key_hash)?;

        if !snapshot.is_empty() && snapshot.key_hash == key_hash {
            self.total_probe_distance
                .fetch_add(probe_dist as u64, Ordering::Relaxed);
            Some(snapshot)
        } else {
            None
        }
    }

    /// CAS update for a key - atomically update value using generation counter
    ///
    /// Returns Ok(()) if CAS succeeded, Err(()) if generation changed or key not found.
    ///
    /// # Performance
    ///
    /// Target: <100ns (find bucket + CAS attempt)
    pub fn cas_update_key<K: Hash + Eq>(
        &self,
        key: &K,
        expected_generation: u32,
        key_data: u64,
        value_data: u64,
    ) -> Result<(), ()> {
        let key_hash = Self::key_hash(key);
        let start_idx = self.hash_to_index(key);

        // Linear probe to find bucket
        for probe_dist in 0..MAX_PROBE_DISTANCE {
            let idx = (start_idx + probe_dist) & self.mask;
            let snapshot = self.buckets[idx].read().ok_or(())?;

            if !snapshot.is_empty() && snapshot.key_hash == key_hash {
                // Found matching bucket - attempt CAS
                return self.buckets[idx]
                    .cas_update(expected_generation, key_hash, key_data, value_data)
                    .map(|_| ())
                    .map_err(|_| ());
            }

            if snapshot.is_empty() {
                // Key not found
                return Err(());
            }
        }

        Err(())
    }

    /// Clear all entries from table (atomic deletion of all buckets)
    ///
    /// # Performance
    ///
    /// Target: O(N) where N is bucket count
    pub fn clear(&self) {
        // Remove all buckets atomically
        for bucket in self.buckets.iter() {
            bucket.remove();
        }
        // Reset count to 0
        self.count.store(0, Ordering::Relaxed);
    }

    /// Get metrics snapshot
    /// TODO(Phase 3): Used in monitoring and debugging
    #[allow(dead_code)]
    pub fn metrics(&self) -> TableMetrics {
        let insertions = self.total_insertions.load(Ordering::Relaxed);
        let deletions = self.total_deletions.load(Ordering::Relaxed);
        let total_ops = insertions + deletions;
        let total_probe_dist = self.total_probe_distance.load(Ordering::Relaxed);

        let avg_probe_dist = if total_ops > 0 {
            total_probe_dist as f64 / total_ops as f64
        } else {
            0.0
        };

        TableMetrics {
            bucket_count: N,
            entry_count: self.count(),
            load_factor: self.load_factor(),
            total_insertions: insertions,
            total_deletions: deletions,
            average_probe_distance: avg_probe_dist,
        }
    }
}

impl<const N: usize> Default for AtomicTable<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Table performance metrics
/// TODO(Phase 3): Used in monitoring and debugging
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct TableMetrics {
    pub bucket_count: usize,
    pub entry_count: usize,
    pub load_factor: f64,
    pub total_insertions: u64,
    pub total_deletions: u64,
    pub average_probe_distance: f64,
}

// Compile-time validation that default bucket count is power of 2
const _: () = {
    assert!(DEFAULT_BUCKET_COUNT.is_power_of_two());
    assert!(DEFAULT_BUCKET_COUNT > 0);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_new() {
        let table = AtomicTable::<16>::new();
        assert_eq!(table.bucket_count(), 16);
        assert_eq!(table.count(), 0);
        assert_eq!(table.load_factor(), 0.0);
    }

    #[test]
    fn table_insert_get() {
        let table = AtomicTable::<16>::new();

        let key = 42u64;
        let result = table.insert(&key, 100, 200);
        assert!(result.is_ok());

        let snapshot = table.get(&key).unwrap();
        assert_eq!(snapshot.key_data, 100);
        assert_eq!(snapshot.value_data, 200);
    }

    #[test]
    fn table_insert_remove() {
        let table = AtomicTable::<16>::new();

        let key = 42u64;
        table.insert(&key, 100, 200).unwrap();
        assert_eq!(table.count(), 1);

        table.remove(&key).unwrap();
        assert_eq!(table.count(), 0);

        assert!(table.get(&key).is_none());
    }

    #[test]
    fn table_multiple_entries() {
        let table = AtomicTable::<16>::new();

        for i in 0..10u64 {
            table.insert(&i, i * 10, i * 20).unwrap();
        }

        assert_eq!(table.count(), 10);

        for i in 0..10u64 {
            let snapshot = table.get(&i).unwrap();
            assert_eq!(snapshot.key_data, i * 10);
            assert_eq!(snapshot.value_data, i * 20);
        }
    }

    #[test]
    fn table_update_existing() {
        let table = AtomicTable::<16>::new();

        let key = 42u64;
        table.insert(&key, 100, 200).unwrap();

        // Update with new value
        table.insert(&key, 300, 400).unwrap();

        let snapshot = table.get(&key).unwrap();
        assert_eq!(snapshot.key_data, 300);
        assert_eq!(snapshot.value_data, 400);

        // Count should still be 1 (update, not new entry)
        assert_eq!(table.count(), 1);
    }

    #[test]
    fn table_metrics() {
        let table = AtomicTable::<16>::new();

        for i in 0..5u64 {
            table.insert(&i, i, i).unwrap();
        }

        let metrics = table.metrics();
        assert_eq!(metrics.bucket_count, 16);
        assert_eq!(metrics.entry_count, 5);
        assert_eq!(metrics.total_insertions, 5);
        assert_eq!(metrics.total_deletions, 0);
    }
}
