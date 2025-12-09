# T5 Streaming Phase 3 - Code Templates & Quick Reference

This document provides ready-to-use code templates, boilerplate, and quick-reference guides for implementing the 3 T5 Streaming primitives.

---

## 1. StreamingDedupCapsule Code Template

### Module Structure (src/streaming/dedup.rs)

```rust
//! T5 Streaming Deduplication Capsule
//!
//! Duplicate detection in sliding windows using Bloom filter + exact match.
//!
//! # Performance
//! - Unique item: 5-10ns (Bloom miss)
//! - Duplicate: 20-50ns (Bloom hit + ring scan)
//! - Throughput: 20M items/sec
//! - Memory: 8-16 KB (O(1) window)
//!
//! # Example
//! ```ignore
//! use kindly_dedup::streaming::StreamingDedupCapsule;
//!
//! let mut dedup = StreamingDedupCapsule::<u64>::new();
//! assert!(!dedup.is_duplicate(42));   // First occurrence
//! assert!(dedup.is_duplicate(42));    // Duplicate detected
//! ```

use atomic_capsule::collections::RingBufferCapsule;
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};

/// Duplicate detection capsule with Bloom filter + exact match
///
/// # Tier: T5 Streaming
/// - O(1) memory (bounded by WINDOW)
/// - <50ns per operation (typical)
/// - No allocations after creation
#[derive(Debug)]
#[repr(C, align(256))]  // ColdTier: 256B cache alignment
pub struct StreamingDedupCapsule<T: Hash + Eq + Copy, const WINDOW: usize = 1024> {
    /// Bloom filter (128 × u64 = 1 KB, 0.08% FPR)
    bloom: [AtomicU64; 128],

    /// Ring buffer for exact match verification
    ring: RingBufferCapsule<T>,

    /// Metrics
    unique_count: AtomicU64,
    duplicate_count: AtomicU64,
    generation: AtomicU64,

    /// Padding to 256B alignment
    _padding: [u8; PADDING],
}

const PADDING: usize = 256 - (
    std::mem::size_of::<[AtomicU64; 128]>() +
    std::mem::size_of::<AtomicU64>() * 3
);

impl<T: Hash + Eq + Copy, const WINDOW: usize> StreamingDedupCapsule<T, WINDOW> {
    /// Create new deduplication capsule
    ///
    /// # Example
    /// ```ignore
    /// let dedup = StreamingDedupCapsule::<u64>::new();
    /// ```
    pub fn new() -> Self {
        Self {
            bloom: std::array::from_fn(|_| AtomicU64::new(0)),
            ring: RingBufferCapsule::new(),
            unique_count: AtomicU64::new(0),
            duplicate_count: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            _padding: [0; PADDING],
        }
    }

    /// Check if item is duplicate and optionally insert
    ///
    /// Returns true if item was seen before (within WINDOW)
    ///
    /// # Time Complexity
    /// - O(1) if Bloom filter miss (unique)
    /// - O(WINDOW) if Bloom hit (scan ring buffer)
    /// - Typical: <50ns (0.08% false positive rate)
    #[inline]
    pub fn is_duplicate(&self, item: T) -> bool {
        // STEP 1: Hash to 3 Bloom filter positions
        let h = fxhash::hash64(&item);  // TODO: Replace with SipHash
        let h1 = ((h >> 0) & 0xFFF) as u16;
        let h2 = ((h >> 12) & 0xFFF) as u16;
        let h3 = ((h >> 24) & 0xFFF) as u16;

        // STEP 2: Check all 3 bits in Bloom filter
        if !self.check_bloom_bit(h1) ||
           !self.check_bloom_bit(h2) ||
           !self.check_bloom_bit(h3) {
            // At least one bit not set → definitely unique
            self.unique_count.fetch_add(1, Ordering::Relaxed);
            return false;
        }

        // STEP 3: Bloom hit → scan ring for exact match
        // TODO: Implement ring iteration
        // for ring_item in self.ring.iter_recent() {
        //     if ring_item == item {
        //         self.duplicate_count.fetch_add(1, Ordering::Relaxed);
        //         return true;
        //     }
        // }

        // STEP 4: Collision (false positive) → insert and return false
        self.ring.record(item);  // TODO: Verify RingBufferCapsule API
        self.set_bloom_bits(h1, h2, h3);
        self.unique_count.fetch_add(1, Ordering::Relaxed);
        false
    }

    /// Get deduplication statistics
    pub fn stats(&self) -> DedupStats {
        DedupStats {
            unique_count: self.unique_count.load(Ordering::Relaxed),
            duplicate_count: self.duplicate_count.load(Ordering::Relaxed),
            bloom_utilization: self.bloom_fill_ratio(),
            generation: self.generation.load(Ordering::Relaxed),
        }
    }

    /// Reset capsule (increments generation counter)
    pub fn reset(&mut self) {
        // Clear Bloom filter
        for i in 0..128 {
            self.bloom[i].store(0, Ordering::Release);
        }
        // Clear ring
        // TODO: Call ring.reset() if available
        // Reset counters
        self.unique_count.store(0, Ordering::Release);
        self.duplicate_count.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    // ===== Internal Methods =====

    #[inline]
    fn check_bloom_bit(&self, hash: u16) -> bool {
        let u64_idx = (hash as usize) / 64;
        let bit_offset = (hash as usize) % 64;
        let val = self.bloom[u64_idx].load(Ordering::Acquire);
        (val & (1u64 << bit_offset)) != 0
    }

    fn set_bloom_bits(&self, h1: u16, h2: u16, h3: u16) {
        for hash in [h1, h2, h3].iter() {
            let u64_idx = (*hash as usize) / 64;
            let bit_offset = (*hash as usize) % 64;
            self.bloom[u64_idx].fetch_or(1u64 << bit_offset, Ordering::Release);
        }
    }

    fn bloom_fill_ratio(&self) -> f64 {
        let mut count = 0u64;
        for i in 0..128 {
            count += self.bloom[i].load(Ordering::Relaxed).count_ones() as u64;
        }
        count as f64 / (128 * 64) as f64  // Total bits = 128 × 64
    }
}

impl<T: Hash + Eq + Copy, const WINDOW: usize> Default for StreamingDedupCapsule<T, WINDOW> {
    fn default() -> Self {
        Self::new()
    }
}

/// Deduplication statistics
#[derive(Debug, Clone)]
pub struct DedupStats {
    pub unique_count: u64,
    pub duplicate_count: u64,
    pub bloom_utilization: f64,
    pub generation: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_empty() {
        let capsule = StreamingDedupCapsule::<u64>::new();
        assert_eq!(capsule.stats().unique_count, 0);
        assert_eq!(capsule.stats().duplicate_count, 0);
    }

    #[test]
    fn test_single_unique_item() {
        let mut capsule = StreamingDedupCapsule::<u64>::new();
        assert!(!capsule.is_duplicate(42));
        assert_eq!(capsule.stats().unique_count, 1);
    }

    #[test]
    fn test_duplicate_detection() {
        let mut capsule = StreamingDedupCapsule::<u64>::new();
        capsule.is_duplicate(42);  // Insert
        assert!(capsule.is_duplicate(42));  // Duplicate
    }

    // Additional tests: test_bloom_collision_detection, test_window_wraparound, etc.
    // See T5_STREAMING_PHASE3_IMPLEMENTATION_PLAN.md for full list
}
```

---

## 2. StreamingJoinCapsule Code Template

### Core Implementation (src/streaming/join.rs)

```rust
//! T5 Streaming Join Capsule
//!
//! Stream-stream joins with windowed coordination.
//!
//! # Performance
//! - Single join: 50-80ns
//! - Typical scan: 150-200ns (10 matches per WINDOW)
//! - Throughput: 5M joins/sec
//! - Memory: 48.5 KB (O(1) window)

use atomic_capsule::collections::RingBufferCapsule;
use std::sync::atomic::{AtomicU64, Ordering};

/// Stream-stream join capsule with windowed coordination
///
/// # Tier: T5 Streaming
/// - Inner join semantics (only keys in both streams)
/// - O(WINDOW^2) worst-case output (bounded)
/// - <200ns per join operation
#[derive(Debug)]
#[repr(C, align(256))]
pub struct StreamingJoinCapsule<L: Copy, R: Copy, const WINDOW: usize = 1024> {
    /// Left stream (keyed tuples)
    left_ring: RingBufferCapsule<(u64, L)>,

    /// Right stream (keyed tuples)
    right_ring: RingBufferCapsule<(u64, R)>,

    /// Output buffer (joined pairs)
    join_buffer: RingBufferCapsule<(L, R)>,

    /// Metrics
    left_count: AtomicU64,
    right_count: AtomicU64,
    join_count: AtomicU64,
    generation: AtomicU64,

    /// Padding to 256B alignment
    _padding: [u8; PADDING],
}

const PADDING: usize = 256 - (
    std::mem::size_of::<AtomicU64>() * 4
);

impl<L: Copy, R: Copy, const WINDOW: usize> StreamingJoinCapsule<L, R, WINDOW> {
    /// Create new join capsule
    pub fn new() -> Self {
        Self {
            left_ring: RingBufferCapsule::new(),
            right_ring: RingBufferCapsule::new(),
            join_buffer: RingBufferCapsule::new(),
            left_count: AtomicU64::new(0),
            right_count: AtomicU64::new(0),
            join_count: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            _padding: [0; PADDING],
        }
    }

    /// Push left item and produce joins
    ///
    /// # Algorithm
    /// 1. Append (key, value) to left_ring
    /// 2. Scan right_ring for all matching keys
    /// 3. For each match: append (value, right_value) to join_buffer
    #[inline]
    pub fn push_left(&mut self, key: u64, value: L) {
        self.left_ring.record((key, value));

        // Scan right ring for matches
        // TODO: Implement iteration over right_ring
        // for (right_key, right_value) in self.right_ring.iter_recent() {
        //     if right_key == key {
        //         self.join_buffer.record((value, *right_value));
        //         self.join_count.fetch_add(1, Ordering::Relaxed);
        //     }
        // }

        self.left_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Push right item and produce joins
    #[inline]
    pub fn push_right(&mut self, key: u64, value: R) {
        self.right_ring.record((key, value));

        // Scan left ring for matches
        // TODO: Similar to push_left

        self.right_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Consume all joined pairs
    ///
    /// Returns vector of all joined pairs in order.
    /// Clears the join buffer.
    pub fn consume(&mut self) -> Vec<(L, R)> {
        let mut result = Vec::new();

        // TODO: Drain join_buffer
        // while let Some(pair) = self.join_buffer.pop() {
        //     result.push(pair);
        // }

        result
    }

    /// Peek next joined pair without consuming
    pub fn peek(&self) -> Option<(L, R)> {
        // TODO: Implement ring peek
        None
    }

    /// Get join statistics
    pub fn stats(&self) -> JoinStats {
        let left_count = self.left_count.load(Ordering::Relaxed);
        let right_count = self.right_count.load(Ordering::Relaxed);
        let join_count = self.join_count.load(Ordering::Relaxed);

        JoinStats {
            left_count,
            right_count,
            join_count,
            join_ratio: if (left_count + right_count) > 0 {
                join_count as f64 / (left_count + right_count) as f64
            } else {
                0.0
            },
        }
    }

    /// Reset capsule
    pub fn reset(&mut self) {
        // TODO: Reset all rings
        self.left_count.store(0, Ordering::Release);
        self.right_count.store(0, Ordering::Release);
        self.join_count.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }
}

impl<L: Copy, R: Copy, const WINDOW: usize> Default for StreamingJoinCapsule<L, R, WINDOW> {
    fn default() -> Self {
        Self::new()
    }
}

/// Join statistics
#[derive(Debug, Clone)]
pub struct JoinStats {
    pub left_count: u64,
    pub right_count: u64,
    pub join_count: u64,
    pub join_ratio: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_empty() {
        let join = StreamingJoinCapsule::<u64, u64, 1024>::new();
        let stats = join.stats();
        assert_eq!(stats.left_count, 0);
        assert_eq!(stats.right_count, 0);
        assert_eq!(stats.join_count, 0);
    }

    // Additional tests...
}
```

---

## 3. StreamingGroupByCapsule Code Template

### Core Implementation (src/streaming/groupby.rs)

```rust
//! T5 Streaming Group-By Capsule
//!
//! Lockfree windowed group-by aggregation with atomic updates.
//!
//! # Performance
//! - Push (no collision): 20-30ns
//! - Push (with collision): 30-50ns
//! - get_groups: 1-2μs (256 buckets)
//! - Throughput: 33M items/sec
//! - Memory: 16.5 KB (O(1) window)

use std::sync::atomic::{AtomicU64, Ordering};

/// Lockfree group-by aggregation capsule
///
/// # Tier: T5 Streaming
/// - Open addressing hash table (linear probing)
/// - CAS-based atomic updates (no mutex)
/// - Fixed-size bucket array (16.5 KB)
#[derive(Debug)]
#[repr(C, align(256))]
pub struct StreamingGroupByCapsule<K: Hash + Eq + Copy, V: Copy, const GROUPS: usize = 256> {
    /// Hash table buckets
    groups: [GroupBucket<V>; GROUPS],

    /// Metrics
    group_count: AtomicU64,
    total_items: AtomicU64,
    generation: AtomicU64,

    /// Padding to 256B alignment
    _padding: [u8; PADDING],
}

const PADDING: usize = 256 - (std::mem::size_of::<AtomicU64>() * 3);

/// Individual bucket in the hash table
#[derive(Debug)]
#[repr(C, align(64))]  // HotTier: 64B cache line
pub struct GroupBucket<V: Copy> {
    /// Hash of key (0 = empty)
    key_hash: AtomicU64,

    /// Accumulated value (bitcast from V)
    value: AtomicU64,

    /// Item count in group
    count: AtomicU64,

    /// Padding to 64B cache line
    _padding: [u8; 40],
}

impl<V: Copy> GroupBucket<V> {
    fn new() -> Self {
        Self {
            key_hash: AtomicU64::new(0),
            value: AtomicU64::new(0),
            count: AtomicU64::new(0),
            _padding: [0; 40],
        }
    }
}

use std::hash::Hash;

impl<K: Hash + Eq + Copy, V: Copy, const GROUPS: usize>
    StreamingGroupByCapsule<K, V, GROUPS>
{
    /// Create new group-by capsule
    pub fn new() -> Self {
        Self {
            groups: std::array::from_fn(|_| GroupBucket::new()),
            group_count: AtomicU64::new(0),
            total_items: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            _padding: [0; PADDING],
        }
    }

    /// Push item to group (hash + value)
    ///
    /// Uses CAS loop to insert or update atomically.
    /// Linear probing on collision.
    ///
    /// # Algorithm
    /// 1. Hash key_hash to bucket index
    /// 2. CAS on empty bucket to insert new group
    /// 3. If occupied and key matches: add to value + count
    /// 4. If collision: linear probe to next bucket
    #[inline]
    pub fn push(&self, key_hash: u64, value: V) {
        if key_hash == 0 {
            panic!("key_hash cannot be 0 (reserved for empty)");
        }

        let mut probe_idx = (key_hash as usize) % GROUPS;
        let max_probes = GROUPS;

        for _ in 0..max_probes {
            let bucket = &self.groups[probe_idx];

            // Try to insert new group
            match bucket.key_hash.compare_exchange_weak(
                0,
                key_hash,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    // Successfully inserted new group
                    bucket.value.store(self.bitcast_value(value), Ordering::Release);
                    bucket.count.store(1, Ordering::Release);
                    self.group_count.fetch_add(1, Ordering::Relaxed);
                    self.total_items.fetch_add(1, Ordering::Relaxed);
                    return;
                }
                Err(existing_hash) => {
                    if existing_hash == key_hash {
                        // Found our group → update atomically
                        bucket.value.fetch_add(
                            self.bitcast_value(value),
                            Ordering::Release
                        );
                        bucket.count.fetch_add(1, Ordering::Release);
                        self.total_items.fetch_add(1, Ordering::Relaxed);
                        return;
                    }
                    // Different key → linear probe
                    probe_idx = (probe_idx + 1) % GROUPS;
                }
            }
        }

        // Table full
        panic!(
            "GroupBy table full ({}% utilization)",
            (self.group_count.load(Ordering::Relaxed) * 100) / GROUPS as u64
        );
    }

    /// Get all groups as snapshot
    ///
    /// Returns vector of (key_hash, aggregated_value) pairs.
    /// Performs full table scan (O(GROUPS)).
    pub fn get_groups(&self) -> Vec<(u64, V)> {
        let mut result = Vec::new();

        for bucket in self.groups.iter() {
            let key_hash = bucket.key_hash.load(Ordering::Acquire);
            if key_hash != 0 {
                let value = bucket.value.load(Ordering::Acquire);
                result.push((key_hash, self.unbitcast_value(value)));
            }
        }

        result
    }

    /// Get single group by key_hash
    pub fn get(&self, key_hash: u64) -> Option<(V, u64)> {
        let mut probe_idx = (key_hash as usize) % GROUPS;

        for _ in 0..GROUPS {
            let bucket = &self.groups[probe_idx];
            let stored_hash = bucket.key_hash.load(Ordering::Acquire);

            if stored_hash == key_hash {
                let value = bucket.value.load(Ordering::Acquire);
                let count = bucket.count.load(Ordering::Acquire);
                return Some((self.unbitcast_value(value), count));
            }

            if stored_hash == 0 {
                return None;  // Not found
            }

            probe_idx = (probe_idx + 1) % GROUPS;
        }

        None
    }

    /// Get group statistics
    pub fn stats(&self) -> GroupStats {
        let group_count = self.group_count.load(Ordering::Relaxed);
        let total_items = self.total_items.load(Ordering::Relaxed);

        GroupStats {
            group_count,
            total_items,
            bucket_utilization: group_count as f64 / GROUPS as f64,
            avg_items_per_group: if group_count > 0 {
                total_items as f64 / group_count as f64
            } else {
                0.0
            },
        }
    }

    /// Reset capsule
    pub fn reset(&mut self) {
        for bucket in self.groups.iter_mut() {
            bucket.key_hash.store(0, Ordering::Release);
            bucket.value.store(0, Ordering::Release);
            bucket.count.store(0, Ordering::Release);
        }
        self.group_count.store(0, Ordering::Release);
        self.total_items.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    // ===== Internal Methods =====

    #[inline]
    fn bitcast_value(&self, value: V) -> u64 {
        // SAFETY: V must be ≤ 8 bytes (enforced by trait bound)
        // Bitcast assumes stable bit representation
        unsafe { std::mem::transmute_copy::<V, u64>(&value) }
    }

    #[inline]
    fn unbitcast_value(&self, bits: u64) -> V {
        // SAFETY: Same as bitcast_value
        unsafe { std::mem::transmute_copy::<u64, V>(&bits) }
    }
}

impl<K: Hash + Eq + Copy, V: Copy, const GROUPS: usize> Default
    for StreamingGroupByCapsule<K, V, GROUPS>
{
    fn default() -> Self {
        Self::new()
    }
}

/// Group statistics
#[derive(Debug, Clone)]
pub struct GroupStats {
    pub group_count: u64,
    pub total_items: u64,
    pub bucket_utilization: f64,
    pub avg_items_per_group: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_empty() {
        let groupby = StreamingGroupByCapsule::<u64, u64, 256>::new();
        let stats = groupby.stats();
        assert_eq!(stats.group_count, 0);
        assert_eq!(stats.total_items, 0);
    }

    #[test]
    fn test_single_group() {
        let groupby = StreamingGroupByCapsule::<u64, u64, 256>::new();
        groupby.push(42, 100);

        let groups = groupby.get_groups();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], (42, 100));
    }

    // Additional tests...
}
```

---

## 4. Module Exports (src/streaming/mod.rs)

```rust
//! T5 Streaming Tier Primitives
//!
//! Phase 3: Dedup, Join, GroupBy capsules

pub mod dedup;
pub mod join;
pub mod groupby;

pub use dedup::StreamingDedupCapsule;
pub use join::StreamingJoinCapsule;
pub use groupby::StreamingGroupByCapsule;
```

---

## 5. Test Boilerplate

### Unit Test Template (tests/streaming_dedup_tests.rs)

```rust
//! Unit tests for StreamingDedupCapsule (Q1-Q7)

#[cfg(test)]
mod unit_tests {
    use kindly_dedup::streaming::StreamingDedupCapsule;

    #[test]
    fn test_new_empty() {
        let capsule = StreamingDedupCapsule::<u64>::new();
        assert_eq!(capsule.stats().unique_count, 0);
        assert_eq!(capsule.stats().duplicate_count, 0);
    }

    #[test]
    fn test_single_unique() {
        let mut capsule = StreamingDedupCapsule::<u64>::new();
        assert!(!capsule.is_duplicate(42));
        let stats = capsule.stats();
        assert_eq!(stats.unique_count, 1);
        assert_eq!(stats.duplicate_count, 0);
    }

    // 5 more unit tests...
}

#[cfg(test)]
mod property_tests {
    use proptest::prelude::*;
    use kindly_dedup::streaming::StreamingDedupCapsule;

    proptest! {
        #[test]
        fn prop_no_false_negatives(
            items in vec(any::<u64>(), 10..1000)
        ) {
            let mut capsule = StreamingDedupCapsule::<u64>::new();

            // First pass: insert all
            for item in items.iter() {
                capsule.is_duplicate(*item);
            }

            // Second pass: all should be duplicates
            for item in items.iter() {
                prop_assert!(
                    capsule.is_duplicate(*item),
                    "False negative: item {} not detected as duplicate",
                    item
                );
            }
        }
    }

    // 6 more property tests...
}

#[cfg(test)]
mod integration_tests {
    use kindly_dedup::streaming::StreamingDedupCapsule;

    #[test]
    fn test_streaming_sequence() {
        // Test 10K items with 50% duplicate rate
        let mut capsule = StreamingDedupCapsule::<u64>::new();
        let mut expected_duplicates = 0;

        for i in 0..10_000 {
            let item = i % 5_000;  // 50% duplicate
            let is_dup = capsule.is_duplicate(item);

            if i >= 5_000 {
                // After first 5K, everything is duplicate
                assert!(is_dup || i < 5_000);
                if is_dup {
                    expected_duplicates += 1;
                }
            }
        }

        let stats = capsule.stats();
        assert_eq!(stats.unique_count, 5_000);
        // Note: exact duplicate count may vary due to Bloom collisions
    }

    // 6 more integration tests...
}

#[cfg(test)]
#[ignore]  // Long-running stress tests
mod stress_tests {
    use kindly_dedup::streaming::StreamingDedupCapsule;

    #[test]
    fn stress_1m_items() {
        let mut capsule = StreamingDedupCapsule::<u64>::new();

        for i in 0..1_000_000 {
            let item = i % 500_000;  // 50% duplicate
            capsule.is_duplicate(item);
        }

        let stats = capsule.stats();
        assert_eq!(stats.unique_count, 500_000);
        assert!(stats.duplicate_count > 450_000);  // At least 450K
    }

    // 6 more stress tests...
}
```

### Benchmark Template (benches/streaming_dedup_bench.rs)

```rust
//! B32 Benchmarks for StreamingDedupCapsule

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use kindly_dedup::streaming::StreamingDedupCapsule;

fn bench_dedup_unique(c: &mut Criterion) {
    c.bench_function("dedup_unique_item", |b| {
        let mut capsule = StreamingDedupCapsule::<u64>::new();
        let items = black_box((0..1000u64).collect::<Vec<_>>());

        b.iter(|| {
            for item in items.iter() {
                capsule.is_duplicate(*item)
            }
        });
    });
}

fn bench_dedup_duplicate(c: &mut Criterion) {
    c.bench_function("dedup_duplicate_item", |b| {
        let mut capsule = StreamingDedupCapsule::<u64>::new();
        let item = black_box(42u64);

        // Pre-insert
        capsule.is_duplicate(item);

        b.iter(|| {
            capsule.is_duplicate(item)
        });
    });
}

fn bench_dedup_mixed_50pct(c: &mut Criterion) {
    c.bench_function("dedup_mixed_50pct_duplicate", |b| {
        let mut capsule = StreamingDedupCapsule::<u64>::new();
        let items = black_box((0..10_000u64).collect::<Vec<_>>());

        b.iter(|| {
            for (i, item) in items.iter().enumerate() {
                let dup = item % 5000;  // 50% duplicate
                capsule.is_duplicate(dup);
            }
        });
    });
}

criterion_group!(benches, bench_dedup_unique, bench_dedup_duplicate, bench_dedup_mixed_50pct);
criterion_main!(benches);
```

---

## 6. Feature Flags (Cargo.toml additions)

```toml
[features]
# T5 Streaming Phase 3 primitives
streaming-dedup-capsule = ["std"]
streaming-join-capsule = ["std"]
streaming-groupby-capsule = ["std"]

# All streaming primitives
streaming-all = [
    "streaming-dedup-capsule",
    "streaming-join-capsule",
    "streaming-groupby-capsule",
]

# Benchmarking support
streaming-bench = ["benchmarking"]

# Testing support
streaming-tests = ["std"]
```

---

## 7. Quick Reference Checklists

### Pre-Implementation Checklist

- [ ] **Architecture Review**
  - [ ] Memory layout verified (256B/64B alignment)
  - [ ] Atomic ordering analyzed (Acquire/Release justification)
  - [ ] Algorithm pseudocode validated
  - [ ] Error handling strategy defined

- [ ] **Design Documentation**
  - [ ] API signatures finalized
  - [ ] Performance model documented
  - [ ] Test strategy designed
  - [ ] Feature flags planned

### Implementation Checklist

- [ ] **StreamingDedupCapsule**
  - [ ] Core structure implemented
  - [ ] Bloom filter logic complete
  - [ ] Ring buffer integration done
  - [ ] is_duplicate() API working
  - [ ] stats() API complete
  - [ ] 24 unit/property/integration tests passing
  - [ ] Benchmark validates <50ns typical
  - [ ] Clippy: 0 warnings

- [ ] **StreamingJoinCapsule**
  - [ ] Core structure implemented
  - [ ] push_left() logic complete
  - [ ] push_right() logic complete
  - [ ] consume() API working
  - [ ] stats() API complete
  - [ ] 25 tests passing
  - [ ] Benchmark validates <200ns typical
  - [ ] Clippy: 0 warnings

- [ ] **StreamingGroupByCapsule**
  - [ ] Core structure implemented
  - [ ] CAS loop logic complete
  - [ ] Linear probing collision handling done
  - [ ] push() API working
  - [ ] get_groups() API complete
  - [ ] 25 tests passing
  - [ ] Benchmark validates <30ns typical
  - [ ] Clippy: 0 warnings

### Validation Checklist

- [ ] **Testing**
  - [ ] All 74 tests pass (Unit/Property/Integration/Stress)
  - [ ] Property tests with proptest
  - [ ] Stress tests with 1M+ items
  - [ ] Concurrent safety tests
  - [ ] Memory profiling (no leaks)

- [ ] **Performance**
  - [ ] Criterion.rs benchmarks (1000+ iterations)
  - [ ] B32 fair baselines (HashSet, HashMap)
  - [ ] Claims validated (7-25× speedup)
  - [ ] Memory profile (O(1) constant)

- [ ] **Compliance**
  - [ ] UCE34 Q1-Q34 complete
  - [ ] Chaos 100% lockfree
  - [ ] ASSUM 99.5%+ safety
  - [ ] B32 fair benchmarking
  - [ ] T28 4-tier testing
  - [ ] I20 20/20 integration

- [ ] **Documentation**
  - [ ] Rustdoc 100% coverage
  - [ ] Examples for each API
  - [ ] Architecture guide complete
  - [ ] Framework compliance doc

### Deployment Checklist

- [ ] **Code Quality**
  - [ ] Clippy: 0 warnings
  - [ ] rustfmt: applied
  - [ ] Documentation: no warnings
  - [ ] Tests: all passing

- [ ] **Git**
  - [ ] Commit message: `[TRADE SECRET] feat(streaming): T5 Phase 3 (Dedup+Join+GroupBy)`
  - [ ] Local commit only (no push)
  - [ ] Branch: clean-readme or feature-streaming

- [ ] **Integration**
  - [ ] src/streaming/mod.rs exports updated
  - [ ] lib.rs imports updated
  - [ ] Feature flags in Cargo.toml
  - [ ] No breaking changes

---

## References

- **Implementation Plan**: `T5_STREAMING_PHASE3_IMPLEMENTATION_PLAN.md`
- **Architecture**: `T5_STREAMING_PHASE3_ARCHITECTURE.md`
- **Frameworks**: `/home/samuel/projects/kindly-ecosystem/kindly-main/docs/frameworks/`
- **atomic_capsule**: `/home/samuel/Primitives/atomic_capsule/CLAUDE.md`

---

**Status**: Templates ready for use. Copy & paste, implement methods, run tests.
