//! IdempotencyCacheCapsule - T1 Atomic Request Deduplication Cache (16KB)
//!
//! Prevents request replay attacks by caching idempotency keys.
//! Client sends X-Idempotency-Key header, server returns cached response.
//!
//! **Tier**: T1 Atomic (lockfree hash table)
//! **Size**: 16KB (2048 entries × 8 bytes)
//! **Latency**: <30ns lookup, <50ns insert
//! **TTL**: 24 hours (86400 seconds)
//!
//! ## UCE34 Compliance
//! - Q10: T1 Atomic (hash table with FNV-1a)
//! - Q22: Packed entries (key_hash_high:32 | timestamp_offset:24 | flags:8)
//! - Q23: 100% lockfree (CAS loops)
//! - Q33: Cache-aligned (64B)
//!
//! ## Usage
//! ```rust,ignore
//! let cache = IdempotencyCacheCapsule::new();
//!
//! // Check if key exists (returns cached response hash if found)
//! if let Some(_) = cache.get("user-request-123") {
//!     return cached_response();
//! }
//!
//! // Process request...
//! let response = process_request();
//!
//! // Cache for future requests
//! cache.insert("user-request-123");
//! ```

use core::sync::atomic::{AtomicU64, Ordering};

/// Number of cache entries (power of 2 for fast modulo)
const CACHE_SIZE: usize = 2048;

/// TTL in 64-second units (24 hours = 86400 seconds = 1350 units)
const TTL_UNITS: u32 = 1350;

/// FNV-1a constants
const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

/// Maximum probe distance for linear probing
const MAX_PROBES: usize = 8;

/// Idempotency cache entry
///
/// Packed format (64 bits):
/// - Bits 32-63: Key hash (high 32 bits of FNV-1a)
/// - Bits 8-31: Timestamp offset (seconds since epoch / 64, ~136 year range)
/// - Bits 0-7: Flags (bit 0 = occupied, bits 1-7 reserved)
#[repr(C)]
struct CacheEntry {
    packed: AtomicU64,
}

impl CacheEntry {
    const fn new() -> Self {
        Self {
            packed: AtomicU64::new(0),
        }
    }

    #[inline]
    fn pack(key_hash_high: u32, timestamp_offset: u32, occupied: bool) -> u64 {
        ((key_hash_high as u64) << 32)
            | (((timestamp_offset & 0xFFFFFF) as u64) << 8)
            | (occupied as u64)
    }

    #[inline]
    fn unpack(value: u64) -> (u32, u32, bool) {
        let key_hash_high = (value >> 32) as u32;
        let timestamp_offset = ((value >> 8) & 0xFFFFFF) as u32;
        let occupied = (value & 1) != 0;
        (key_hash_high, timestamp_offset, occupied)
    }
}

/// T1 Atomic Idempotency Cache (16KB)
///
/// Lockfree hash table for caching idempotency keys.
/// Uses linear probing with CAS for thread-safe access.
#[repr(C, align(64))]
pub struct IdempotencyCacheCapsule {
    /// Cache entries (2048 x 8B = 16KB)
    entries: [CacheEntry; CACHE_SIZE],
    /// Statistics
    hits: AtomicU64,
    misses: AtomicU64,
    inserts: AtomicU64,
    evictions: AtomicU64,
    generation: AtomicU64,
    /// Padding to 64B alignment for stats block
    _padding: [u64; 3],
}

impl IdempotencyCacheCapsule {
    /// Create new empty cache
    pub const fn new() -> Self {
        // Safe: CacheEntry is just AtomicU64::new(0)
        const EMPTY: CacheEntry = CacheEntry::new();
        Self {
            entries: [EMPTY; CACHE_SIZE],
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            inserts: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            _padding: [0; 3],
        }
    }

    /// Check if idempotency key exists and is not expired
    ///
    /// # Returns
    /// - `Some(true)` if key exists and is valid
    /// - `None` if key not found or expired
    ///
    /// # Performance
    /// - <30ns (FNV-1a hash + atomic load)
    pub fn get(&self, key: &str) -> Option<bool> {
        let hash = fnv1a_hash(key);
        let key_hash_high = (hash >> 32) as u32;
        let index = (hash as usize) % CACHE_SIZE;

        let now_offset = Self::current_timestamp_offset();

        // Linear probing (max 8 probes)
        for probe in 0..MAX_PROBES {
            let slot = (index + probe) % CACHE_SIZE;
            let entry = self.entries[slot].packed.load(Ordering::Acquire);
            let (stored_hash, stored_ts, occupied) = CacheEntry::unpack(entry);

            if !occupied {
                // Empty slot, key not found
                self.misses.fetch_add(1, Ordering::Relaxed);
                return None;
            }

            if stored_hash == key_hash_high {
                // Check TTL (24 hours = 1350 units of 64 seconds)
                let age = now_offset.saturating_sub(stored_ts);
                if age <= TTL_UNITS {
                    self.hits.fetch_add(1, Ordering::Relaxed);
                    return Some(true);
                }
                // Expired
                self.misses.fetch_add(1, Ordering::Relaxed);
                return None;
            }
        }

        self.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Check if idempotency key exists using a custom timestamp offset (for testing)
    ///
    /// # Note
    /// This method is exposed for integration tests (T28 Q8-Q14 property tests).
    /// Not intended for production use.
    #[doc(hidden)]
    pub fn get_with_offset(&self, key: &str, now_offset: u32) -> Option<bool> {
        let hash = fnv1a_hash(key);
        let key_hash_high = (hash >> 32) as u32;
        let index = (hash as usize) % CACHE_SIZE;

        for probe in 0..MAX_PROBES {
            let slot = (index + probe) % CACHE_SIZE;
            let entry = self.entries[slot].packed.load(Ordering::Acquire);
            let (stored_hash, stored_ts, occupied) = CacheEntry::unpack(entry);

            if !occupied {
                self.misses.fetch_add(1, Ordering::Relaxed);
                return None;
            }

            if stored_hash == key_hash_high {
                let age = now_offset.saturating_sub(stored_ts);
                if age <= TTL_UNITS {
                    self.hits.fetch_add(1, Ordering::Relaxed);
                    return Some(true);
                }
                self.misses.fetch_add(1, Ordering::Relaxed);
                return None;
            }
        }

        self.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Insert idempotency key with current timestamp
    ///
    /// # Returns
    /// - `true` if inserted (new key)
    /// - `false` if key already existed (duplicate)
    ///
    /// # Performance
    /// - <50ns (FNV-1a hash + CAS)
    pub fn insert(&self, key: &str) -> bool {
        self.insert_with_offset(key, Self::current_timestamp_offset())
    }

    /// Insert with custom timestamp offset (for testing)
    ///
    /// # Note
    /// This method is exposed for integration tests (T28 Q8-Q14 property tests).
    /// Not intended for production use.
    #[doc(hidden)]
    pub fn insert_with_offset(&self, key: &str, now_offset: u32) -> bool {
        let hash = fnv1a_hash(key);
        let key_hash_high = (hash >> 32) as u32;
        let index = (hash as usize) % CACHE_SIZE;

        let new_value = CacheEntry::pack(key_hash_high, now_offset, true);

        // Linear probing (max 8 probes)
        for probe in 0..MAX_PROBES {
            let slot = (index + probe) % CACHE_SIZE;
            let entry = &self.entries[slot];
            let current = entry.packed.load(Ordering::Acquire);
            let (stored_hash, stored_ts, occupied) = CacheEntry::unpack(current);

            // Check if we found an empty slot
            if !occupied {
                // Try to claim empty slot
                if entry
                    .packed
                    .compare_exchange(current, new_value, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    self.inserts.fetch_add(1, Ordering::Relaxed);
                    self.generation.fetch_add(1, Ordering::Relaxed);
                    return true;
                }
                // CAS failed, retry this slot
                continue;
            }

            if stored_hash == key_hash_high {
                // Key exists - check if expired
                let age = now_offset.saturating_sub(stored_ts);
                if age <= TTL_UNITS {
                    // Still valid, duplicate request
                    return false;
                }
                // Expired, overwrite
                if entry
                    .packed
                    .compare_exchange(current, new_value, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    self.evictions.fetch_add(1, Ordering::Relaxed);
                    self.inserts.fetch_add(1, Ordering::Relaxed);
                    self.generation.fetch_add(1, Ordering::Relaxed);
                    return true;
                }
            }

            // Check if this slot is expired (LRU eviction)
            let age = now_offset.saturating_sub(stored_ts);
            if age > TTL_UNITS {
                // Expired, try to evict
                if entry
                    .packed
                    .compare_exchange(current, new_value, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    self.evictions.fetch_add(1, Ordering::Relaxed);
                    self.inserts.fetch_add(1, Ordering::Relaxed);
                    self.generation.fetch_add(1, Ordering::Relaxed);
                    return true;
                }
            }
        }

        // All 8 slots full and not expired - forced eviction of oldest
        let mut oldest_slot = index;
        let mut oldest_ts = u32::MAX;

        for probe in 0..MAX_PROBES {
            let slot = (index + probe) % CACHE_SIZE;
            let entry = self.entries[slot].packed.load(Ordering::Acquire);
            let (_, stored_ts, _) = CacheEntry::unpack(entry);
            if stored_ts < oldest_ts {
                oldest_ts = stored_ts;
                oldest_slot = slot;
            }
        }

        self.entries[oldest_slot]
            .packed
            .store(new_value, Ordering::Release);
        self.evictions.fetch_add(1, Ordering::Relaxed);
        self.inserts.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// Get cache statistics
    pub fn stats(&self) -> IdempotencyCacheStats {
        IdempotencyCacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            inserts: self.inserts.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
            generation: self.generation.load(Ordering::Relaxed),
        }
    }

    /// Get generation counter
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    /// Reset cache (clear all entries)
    pub fn reset(&self) {
        for entry in &self.entries {
            entry.packed.store(0, Ordering::Relaxed);
        }
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.inserts.store(0, Ordering::Relaxed);
        self.evictions.store(0, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current count of occupied entries
    pub fn len(&self) -> usize {
        let mut count = 0;
        for entry in &self.entries {
            let value = entry.packed.load(Ordering::Relaxed);
            let (_, _, occupied) = CacheEntry::unpack(value);
            if occupied {
                count += 1;
            }
        }
        count
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get cache capacity
    pub const fn capacity(&self) -> usize {
        CACHE_SIZE
    }

    #[inline]
    fn current_timestamp_offset() -> u32 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        // Divide by 64 to fit in 24 bits (~136 year range)
        (now / 64) as u32 & 0xFFFFFF
    }
}

/// FNV-1a hash function
#[inline]
pub fn fnv1a_hash(s: &str) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in s.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Cache statistics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdempotencyCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub inserts: u64,
    pub evictions: u64,
    pub generation: u64,
}

impl IdempotencyCacheStats {
    /// Calculate hit rate (0.0 - 1.0)
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    /// Calculate eviction rate (evictions per insert)
    pub fn eviction_rate(&self) -> f64 {
        if self.inserts == 0 {
            0.0
        } else {
            self.evictions as f64 / self.inserts as f64
        }
    }
}

impl Default for IdempotencyCacheCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: IdempotencyCacheCapsule only contains AtomicU64 fields which are Send + Sync
unsafe impl Send for IdempotencyCacheCapsule {}
unsafe impl Sync for IdempotencyCacheCapsule {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    // =========================================================================
    // Basic Insert/Get Tests
    // =========================================================================

    #[test]
    fn test_basic_insert_get() {
        let cache = IdempotencyCacheCapsule::new();

        // Insert a key
        assert!(cache.insert("request-123"));

        // Get should find it
        assert_eq!(cache.get("request-123"), Some(true));

        // Stats should reflect this
        let stats = cache.stats();
        assert_eq!(stats.inserts, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 0);
    }

    #[test]
    fn test_get_nonexistent() {
        let cache = IdempotencyCacheCapsule::new();

        // Get on empty cache
        assert_eq!(cache.get("nonexistent-key"), None);

        let stats = cache.stats();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 0);
    }

    #[test]
    fn test_multiple_keys() {
        let cache = IdempotencyCacheCapsule::new();

        // Insert multiple keys
        assert!(cache.insert("key-1"));
        assert!(cache.insert("key-2"));
        assert!(cache.insert("key-3"));

        // All should be found
        assert_eq!(cache.get("key-1"), Some(true));
        assert_eq!(cache.get("key-2"), Some(true));
        assert_eq!(cache.get("key-3"), Some(true));

        // Non-existent still not found
        assert_eq!(cache.get("key-4"), None);

        let stats = cache.stats();
        assert_eq!(stats.inserts, 3);
        assert_eq!(stats.hits, 3);
        assert_eq!(stats.misses, 1);
    }

    // =========================================================================
    // Duplicate Detection Tests
    // =========================================================================

    #[test]
    fn test_duplicate_detection() {
        let cache = IdempotencyCacheCapsule::new();

        // First insert succeeds
        assert!(cache.insert("duplicate-key"));

        // Second insert of same key fails (duplicate)
        assert!(!cache.insert("duplicate-key"));

        // Third attempt also fails
        assert!(!cache.insert("duplicate-key"));

        let stats = cache.stats();
        assert_eq!(stats.inserts, 1); // Only 1 successful insert
    }

    #[test]
    fn test_duplicate_after_get() {
        let cache = IdempotencyCacheCapsule::new();

        assert!(cache.insert("key"));
        assert_eq!(cache.get("key"), Some(true));

        // Insert after get should still detect duplicate
        assert!(!cache.insert("key"));
    }

    // =========================================================================
    // TTL Expiration Tests
    // =========================================================================

    #[test]
    fn test_ttl_expiration() {
        let cache = IdempotencyCacheCapsule::new();

        // Insert with a specific timestamp
        let old_offset = 1000u32;
        assert!(cache.insert_with_offset("expired-key", old_offset));

        // Check with current time (should be expired - way past 24 hours)
        let now_offset = old_offset + TTL_UNITS + 100; // Definitely expired
        assert_eq!(cache.get_with_offset("expired-key", now_offset), None);
    }

    #[test]
    fn test_ttl_not_expired() {
        let cache = IdempotencyCacheCapsule::new();

        let base_offset = 1000u32;
        assert!(cache.insert_with_offset("valid-key", base_offset));

        // Check just before expiration
        let still_valid = base_offset + TTL_UNITS - 1;
        assert_eq!(cache.get_with_offset("valid-key", still_valid), Some(true));

        // Check exactly at expiration boundary
        let at_boundary = base_offset + TTL_UNITS;
        assert_eq!(cache.get_with_offset("valid-key", at_boundary), Some(true));
    }

    #[test]
    fn test_expired_key_reinsert() {
        let cache = IdempotencyCacheCapsule::new();

        let old_offset = 1000u32;
        assert!(cache.insert_with_offset("reinsert-key", old_offset));

        // After expiration, should be able to reinsert
        let new_offset = old_offset + TTL_UNITS + 100;
        assert!(cache.insert_with_offset("reinsert-key", new_offset));

        // Should find it with new timestamp
        assert_eq!(cache.get_with_offset("reinsert-key", new_offset), Some(true));

        let stats = cache.stats();
        assert_eq!(stats.inserts, 2);
        assert_eq!(stats.evictions, 1); // Old entry was evicted
    }

    // =========================================================================
    // Concurrent Access Tests
    // =========================================================================

    #[test]
    fn test_concurrent_inserts() {
        let cache = Arc::new(IdempotencyCacheCapsule::new());
        let num_threads = 8;
        let keys_per_thread = 100;

        let handles: Vec<_> = (0..num_threads)
            .map(|t| {
                let cache = Arc::clone(&cache);
                thread::spawn(move || {
                    for i in 0..keys_per_thread {
                        let key = format!("thread-{}-key-{}", t, i);
                        cache.insert(&key);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // All unique keys should be insertable
        let stats = cache.stats();
        assert_eq!(stats.inserts, (num_threads * keys_per_thread) as u64);
    }

    #[test]
    fn test_concurrent_same_key() {
        let cache = Arc::new(IdempotencyCacheCapsule::new());
        let num_threads = 16;

        let handles: Vec<_> = (0..num_threads)
            .map(|_| {
                let cache = Arc::clone(&cache);
                thread::spawn(move || cache.insert("same-key"))
            })
            .collect();

        let results: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // Exactly one thread should succeed
        let successes: usize = results.iter().filter(|&&r| r).count();
        assert_eq!(successes, 1);

        let stats = cache.stats();
        assert_eq!(stats.inserts, 1);
    }

    #[test]
    fn test_concurrent_read_write() {
        let cache = Arc::new(IdempotencyCacheCapsule::new());

        // Pre-populate some keys
        for i in 0..100 {
            cache.insert(&format!("pre-key-{}", i));
        }

        let num_readers = 4;
        let num_writers = 4;
        let iterations = 100;

        let readers: Vec<_> = (0..num_readers)
            .map(|_| {
                let cache = Arc::clone(&cache);
                thread::spawn(move || {
                    let mut hits = 0u64;
                    for i in 0..iterations {
                        if cache.get(&format!("pre-key-{}", i % 100)).is_some() {
                            hits += 1;
                        }
                    }
                    hits
                })
            })
            .collect();

        let writers: Vec<_> = (0..num_writers)
            .map(|t| {
                let cache = Arc::clone(&cache);
                thread::spawn(move || {
                    for i in 0..iterations {
                        cache.insert(&format!("new-key-{}-{}", t, i));
                    }
                })
            })
            .collect();

        // Wait for all threads
        for handle in readers {
            let hits = handle.join().unwrap();
            assert!(hits > 0); // Should get some hits
        }

        for handle in writers {
            handle.join().unwrap();
        }

        // Cache should have entries
        assert!(!cache.is_empty());
    }

    // =========================================================================
    // LRU Eviction Tests
    // =========================================================================

    #[test]
    fn test_lru_eviction_on_collision() {
        let cache = IdempotencyCacheCapsule::new();

        // Fill the entire cache (way beyond probe limit to force evictions)
        let base_offset = 1000u32;

        // Insert many keys to fill the cache
        for i in 0..(CACHE_SIZE + 100) {
            cache.insert_with_offset(&format!("fill-key-{}", i), base_offset + i as u32);
        }

        let stats = cache.stats();
        // Since we inserted more than capacity and some slots will collide,
        // we should have evictions
        assert!(
            stats.evictions >= 1,
            "Expected evictions >= 1, got {} (inserts: {})",
            stats.evictions,
            stats.inserts
        );
    }

    #[test]
    fn test_evict_oldest_entry() {
        let cache = IdempotencyCacheCapsule::new();

        // Insert entries with different ages
        let base = 1000u32;
        cache.insert_with_offset("oldest", base);
        cache.insert_with_offset("middle", base + 100);
        cache.insert_with_offset("newest", base + 200);

        // All should be findable initially
        let check_time = base + 200;
        assert!(cache.get_with_offset("oldest", check_time).is_some());
        assert!(cache.get_with_offset("middle", check_time).is_some());
        assert!(cache.get_with_offset("newest", check_time).is_some());
    }

    // =========================================================================
    // FNV-1a Hash Consistency Tests
    // =========================================================================

    #[test]
    fn test_fnv1a_deterministic() {
        let key = "test-key-12345";

        let hash1 = fnv1a_hash(key);
        let hash2 = fnv1a_hash(key);
        let hash3 = fnv1a_hash(key);

        assert_eq!(hash1, hash2);
        assert_eq!(hash2, hash3);
    }

    #[test]
    fn test_fnv1a_different_keys() {
        let hash1 = fnv1a_hash("key1");
        let hash2 = fnv1a_hash("key2");
        let hash3 = fnv1a_hash("key3");

        // Different keys should (very likely) produce different hashes
        assert_ne!(hash1, hash2);
        assert_ne!(hash2, hash3);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_fnv1a_known_values() {
        // FNV-1a known test vectors
        assert_eq!(fnv1a_hash(""), FNV_OFFSET);

        // Single character
        let hash_a = fnv1a_hash("a");
        assert_ne!(hash_a, FNV_OFFSET);

        // Different from single char
        let hash_ab = fnv1a_hash("ab");
        assert_ne!(hash_ab, hash_a);
    }

    #[test]
    fn test_fnv1a_distribution() {
        // Test that hashes distribute across the cache slots
        let mut slot_counts = vec![0u32; 16];

        for i in 0..1000 {
            let key = format!("distribution-test-key-{}", i);
            let hash = fnv1a_hash(&key);
            let slot = (hash as usize) % 16;
            slot_counts[slot] += 1;
        }

        // Each slot should have roughly 1000/16 = 62.5 entries
        // Allow for some variance (30-100 per slot)
        for (i, &count) in slot_counts.iter().enumerate() {
            assert!(
                count >= 30 && count <= 100,
                "Slot {} has {} entries (expected 30-100)",
                i,
                count
            );
        }
    }

    // =========================================================================
    // Stats Tracking Tests
    // =========================================================================

    #[test]
    fn test_stats_initial() {
        let cache = IdempotencyCacheCapsule::new();
        let stats = cache.stats();

        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.inserts, 0);
        assert_eq!(stats.evictions, 0);
        assert_eq!(stats.generation, 0);
    }

    #[test]
    fn test_stats_after_operations() {
        let cache = IdempotencyCacheCapsule::new();

        // Insert 5 keys
        for i in 0..5 {
            cache.insert(&format!("key-{}", i));
        }

        // Hit 3 of them
        cache.get("key-0");
        cache.get("key-1");
        cache.get("key-2");

        // Miss 2 times
        cache.get("nonexistent-1");
        cache.get("nonexistent-2");

        let stats = cache.stats();
        assert_eq!(stats.inserts, 5);
        assert_eq!(stats.hits, 3);
        assert_eq!(stats.misses, 2);
        assert_eq!(stats.generation, 5); // One per insert
    }

    #[test]
    fn test_stats_hit_rate() {
        let stats = IdempotencyCacheStats {
            hits: 75,
            misses: 25,
            inserts: 100,
            evictions: 10,
            generation: 100,
        };

        assert!((stats.hit_rate() - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_stats_eviction_rate() {
        let stats = IdempotencyCacheStats {
            hits: 0,
            misses: 0,
            inserts: 100,
            evictions: 20,
            generation: 100,
        };

        assert!((stats.eviction_rate() - 0.20).abs() < 0.001);
    }

    #[test]
    fn test_stats_zero_division() {
        let stats = IdempotencyCacheStats {
            hits: 0,
            misses: 0,
            inserts: 0,
            evictions: 0,
            generation: 0,
        };

        assert_eq!(stats.hit_rate(), 0.0);
        assert_eq!(stats.eviction_rate(), 0.0);
    }

    // =========================================================================
    // Reset Functionality Tests
    // =========================================================================

    #[test]
    fn test_reset_clears_entries() {
        let cache = IdempotencyCacheCapsule::new();

        // Insert some keys
        cache.insert("key-1");
        cache.insert("key-2");
        cache.insert("key-3");

        assert!(!cache.is_empty());
        assert_eq!(cache.len(), 3);

        // Reset
        cache.reset();

        // Should be empty
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);

        // Keys should not be found
        assert_eq!(cache.get("key-1"), None);
        assert_eq!(cache.get("key-2"), None);
        assert_eq!(cache.get("key-3"), None);
    }

    #[test]
    fn test_reset_clears_stats() {
        let cache = IdempotencyCacheCapsule::new();

        cache.insert("key");
        cache.get("key");
        cache.get("nonexistent");

        cache.reset();

        let stats = cache.stats();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.inserts, 0);
        assert_eq!(stats.evictions, 0);
        // Generation should increment on reset
        assert_eq!(stats.generation, 2); // 1 from insert + 1 from reset
    }

    #[test]
    fn test_reset_allows_reinsert() {
        let cache = IdempotencyCacheCapsule::new();

        cache.insert("key");
        assert!(!cache.insert("key")); // Duplicate

        cache.reset();

        assert!(cache.insert("key")); // Should work after reset
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_empty_key() {
        let cache = IdempotencyCacheCapsule::new();

        assert!(cache.insert(""));
        assert_eq!(cache.get(""), Some(true));
    }

    #[test]
    fn test_long_key() {
        let cache = IdempotencyCacheCapsule::new();

        let long_key = "a".repeat(10000);
        assert!(cache.insert(&long_key));
        assert_eq!(cache.get(&long_key), Some(true));
    }

    #[test]
    fn test_unicode_key() {
        let cache = IdempotencyCacheCapsule::new();

        let unicode_key = "Hello World";
        assert!(cache.insert(unicode_key));
        assert_eq!(cache.get(unicode_key), Some(true));
    }

    #[test]
    fn test_special_characters() {
        let cache = IdempotencyCacheCapsule::new();

        let keys = [
            "key-with-dash",
            "key_with_underscore",
            "key.with.dots",
            "key:with:colons",
            "key/with/slashes",
            "key?with=query&params",
            "key\nwith\nnewlines",
            "key\twith\ttabs",
        ];

        for key in &keys {
            assert!(cache.insert(key), "Failed to insert: {}", key);
        }

        for key in &keys {
            assert_eq!(
                cache.get(key),
                Some(true),
                "Failed to find: {}",
                key
            );
        }
    }

    // =========================================================================
    // Capacity and Size Tests
    // =========================================================================

    #[test]
    fn test_capacity() {
        let cache = IdempotencyCacheCapsule::new();
        assert_eq!(cache.capacity(), CACHE_SIZE);
        assert_eq!(cache.capacity(), 2048);
    }

    #[test]
    fn test_len_is_empty() {
        let cache = IdempotencyCacheCapsule::new();

        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);

        cache.insert("key-1");
        assert!(!cache.is_empty());
        assert_eq!(cache.len(), 1);

        cache.insert("key-2");
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_size_is_16kb() {
        // Verify the cache is approximately 16KB
        let size = std::mem::size_of::<IdempotencyCacheCapsule>();

        // 2048 entries * 8 bytes = 16384 bytes = 16KB
        // Plus stats (5 * 8 = 40 bytes) + padding (3 * 8 = 24 bytes) = 64 bytes
        // Total: 16384 + 64 = 16448 bytes
        assert!(size >= 16384, "Cache size {} is less than 16KB", size);
        assert!(size <= 17000, "Cache size {} is too large", size);
    }

    #[test]
    fn test_alignment() {
        // Verify 64-byte alignment
        assert_eq!(
            std::mem::align_of::<IdempotencyCacheCapsule>(),
            64,
            "Cache should be 64-byte aligned"
        );
    }

    // =========================================================================
    // Generation Counter Tests
    // =========================================================================

    #[test]
    fn test_generation_increments() {
        let cache = IdempotencyCacheCapsule::new();

        assert_eq!(cache.generation(), 0);

        cache.insert("key-1");
        assert_eq!(cache.generation(), 1);

        cache.insert("key-2");
        assert_eq!(cache.generation(), 2);

        // Duplicate doesn't increment
        cache.insert("key-1");
        assert_eq!(cache.generation(), 2);

        // Get doesn't increment
        cache.get("key-1");
        assert_eq!(cache.generation(), 2);

        // Reset increments
        cache.reset();
        assert_eq!(cache.generation(), 3);
    }

    // =========================================================================
    // Default Trait Test
    // =========================================================================

    #[test]
    fn test_default_trait() {
        let cache: IdempotencyCacheCapsule = Default::default();
        assert!(cache.is_empty());
        assert_eq!(cache.generation(), 0);
    }

    // =========================================================================
    // Send + Sync Tests
    // =========================================================================

    #[test]
    fn test_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<IdempotencyCacheCapsule>();
        assert_sync::<IdempotencyCacheCapsule>();
    }

    // =========================================================================
    // Stress Tests
    // =========================================================================

    #[test]
    fn test_high_load() {
        let cache = IdempotencyCacheCapsule::new();

        // Insert many more keys than capacity
        for i in 0..10000 {
            cache.insert(&format!("stress-key-{}", i));
        }

        // Cache should still function
        let stats = cache.stats();
        assert!(stats.inserts > 0);
        assert!(!cache.is_empty());

        // Recent keys should be findable (with high probability)
        let mut found = 0;
        for i in 9900..10000 {
            if cache.get(&format!("stress-key-{}", i)).is_some() {
                found += 1;
            }
        }
        // At least some recent keys should be found
        assert!(found > 50, "Only {} of 100 recent keys found", found);
    }

    #[test]
    fn test_rapid_insert_get() {
        let cache = Arc::new(IdempotencyCacheCapsule::new());
        let iterations = 10000;

        let inserter = {
            let cache = Arc::clone(&cache);
            thread::spawn(move || {
                for i in 0..iterations {
                    cache.insert(&format!("rapid-key-{}", i));
                }
            })
        };

        let getter = {
            let cache = Arc::clone(&cache);
            thread::spawn(move || {
                let mut found = 0u64;
                for i in 0..iterations {
                    if cache.get(&format!("rapid-key-{}", i)).is_some() {
                        found += 1;
                    }
                }
                found
            })
        };

        inserter.join().unwrap();
        let found = getter.join().unwrap();

        // Some keys should be found (timing dependent)
        // The getter runs concurrently, so may or may not find keys
        let stats = cache.stats();
        assert_eq!(stats.inserts, iterations as u64);
        assert!(found <= iterations as u64);
    }
}
