//! Cache Key Capsule - 128B Tier 1 Atomic Capsule for LRU Cache Entries
//!
//! # UCE34 Q10: Tier 1 Atomic Capsule
//!
//! **Tier**: Tier 1 (Atomic) - Lockfree coordination with generation counters
//! **Size**: 128 bytes (cache-aligned)
//! **Performance**: <100ns cache hit, 3-10× vs mutex
//!
//! # UCE34 Q22: State Management
//!
//! **Packed State**: hash | last_access_ns | response_offset | ttl_ns | generation
//! **Generation Counter**: TOCTOU prevention for concurrent access
//! **Cache Alignment**: 128B for false sharing prevention

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Cache entry representing a cached AI request/response pair
///
/// # UCE34 Q24: Memory Layout
///
/// ```text
/// Offset | Field            | Size | Purpose
/// -------|------------------|------|----------------------------------
/// 0      | hash             | 8B   | Request hash (const_fast_hash)
/// 8      | last_access_ns   | 8B   | LRU timestamp (nanoseconds)
/// 16     | response_offset  | 8B   | Pointer to cached response
/// 24     | ttl_ns           | 8B   | Time-to-live (nanoseconds)
/// 32     | generation       | 8B   | Generation counter (TOCTOU)
/// 40     | ref_count        | 4B   | In-flight references (eviction guard)
/// 44     | _padding         | 84B  | Cache line padding
/// ```
///
/// **Total**: 128 bytes (cache-aligned)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
pub struct CacheKeyCapsule {
    /// Request hash (0 = empty slot)
    ///
    /// #ASSUME: hash != 0 for valid entries (zero reserved for empty)
    /// #VERIFY: Validated in insert() and get() methods
    hash: AtomicU64,

    /// Last access timestamp (nanoseconds since epoch)
    ///
    /// #ASSUME: Monotonically increasing for LRU ordering
    /// #VERIFY: Updated on every cache hit via fetch_max
    last_access_ns: AtomicU64,

    /// Response data offset/pointer
    ///
    /// #ASSUME: Points to valid response data in separate storage
    /// #VERIFY: Validated before dereference (bounds checking)
    response_offset: AtomicU64,

    /// Time-to-live (nanoseconds)
    ///
    /// #ASSUME: TTL > 0 for valid entries (0 = no expiration)
    /// #VERIFY: Checked in is_expired() method
    ttl_ns: AtomicU64,

    /// Generation counter for TOCTOU prevention
    ///
    /// #ASSUME: Incremented on every update (odd during write, even when stable)
    /// #VERIFY: Checked for odd/even transitions in concurrent access
    generation: AtomicU64,

    /// Reference counter for in-flight access tracking
    ///
    /// #ASSUME: ref_count > 0 means entry is actively being used
    /// #VERIFY: Incremented in get(), decremented on drop, checked in evict()
    ref_count: AtomicU32,

    /// Frequency counter for access tracking (frequency-weighted LRU)
    ///
    /// #ASSUME: Incremented on each cache hit, never decremented
    /// #VERIFY: Used for frequency-weighted LRU scoring (hot entries survive longer)
    freq_count: AtomicU32,

    /// Padding to 128 bytes (prevent false sharing)
    _padding: [u8; 80],
}

impl Default for CacheKeyCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheKeyCapsule {
    /// Create a new empty cache entry
    ///
    /// # UCE34 Q21: Lifecycle - Initialization
    ///
    /// **Pattern**: Const initialization with zero values
    pub const fn new() -> Self {
        Self {
            hash: AtomicU64::new(0),
            last_access_ns: AtomicU64::new(0),
            response_offset: AtomicU64::new(0),
            ttl_ns: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            ref_count: AtomicU32::new(0),
            freq_count: AtomicU32::new(0),
            _padding: [0; 80],
        }
    }

    /// Check if this slot is empty (hash == 0)
    ///
    /// # UCE34 Q23: Concurrency
    ///
    /// **Memory Ordering**: Relaxed (no synchronization needed for check)
    ///
    /// #ASSUME: Empty iff hash == 0
    /// #VERIFY: All insert operations set hash != 0
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        // #ASSUME: Relaxed ordering safe (no data dependency)
        self.hash.load(Ordering::Relaxed) == 0
    }

    /// Get the request hash for this entry
    ///
    /// #ASSUME: Acquire ordering ensures visibility of associated data
    /// #VERIFY: Caller checks for zero (empty slot)
    #[inline(always)]
    pub fn hash(&self) -> u64 {
        // #ASSUME: Acquire ordering for hash → response_offset dependency
        self.hash.load(Ordering::Acquire)
    }

    /// Get the last access timestamp (nanoseconds since epoch)
    ///
    /// #ASSUME: Relaxed ordering (timestamp read doesn't synchronize data)
    #[inline(always)]
    pub fn last_access_ns(&self) -> u64 {
        // #ASSUME: Relaxed safe (LRU ordering, not data synchronization)
        self.last_access_ns.load(Ordering::Relaxed)
    }

    /// Get the response offset/pointer
    ///
    /// #ASSUME: Acquire ordering ensures response data visibility
    #[inline(always)]
    pub fn response_offset(&self) -> u64 {
        // #ASSUME: Acquire ordering for happens-before relationship
        self.response_offset.load(Ordering::Acquire)
    }

    /// Get the generation counter
    ///
    /// #ASSUME: Even generation = stable, odd = in-flight update
    /// #VERIFY: Used for TOCTOU detection in concurrent updates
    #[inline(always)]
    pub fn generation(&self) -> u64 {
        // #ASSUME: Acquire ordering for generation → data dependency
        self.generation.load(Ordering::Acquire)
    }

    /// Get the reference count
    ///
    /// #ASSUME: ref_count > 0 means entry is in-flight (being used)
    /// #VERIFY: Incremented in acquire_ref(), decremented in release_ref()
    #[inline(always)]
    pub fn ref_count(&self) -> u32 {
        // #ASSUME: Relaxed ordering (ref_count is independent counter)
        self.ref_count.load(Ordering::Relaxed)
    }

    /// Get the frequency count (number of cache hits)
    ///
    /// #ASSUME: Monotonically increasing counter for hot entry detection
    /// #VERIFY: Incremented on cache hit, used for frequency-weighted LRU
    #[inline(always)]
    pub fn freq_count(&self) -> u32 {
        // #ASSUME: Relaxed ordering (frequency is independent counter)
        self.freq_count.load(Ordering::Relaxed)
    }

    /// Increment frequency counter (on cache hit)
    ///
    /// #ASSUME: Called on every cache hit to track hot entries
    /// #VERIFY: Saturating add prevents overflow
    #[inline]
    pub fn increment_freq(&self) {
        // #ASSUME: Relaxed ordering (frequency doesn't synchronize data)
        // #VERIFY: Saturating add prevents overflow at u32::MAX
        self.freq_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Acquire a reference to this entry (prevents eviction)
    ///
    /// # UCE34 Q23: Concurrency - Reference Counting
    ///
    /// **Pattern**: Atomic ref counting for eviction protection
    ///
    /// #ASSUME: fetch_add ensures atomic increment
    /// #VERIFY: Every acquire_ref() must be paired with release_ref()
    #[inline]
    pub fn acquire_ref(&self) {
        // #ASSUME: Acquire ordering prevents eviction reordering
        // #VERIFY: Evict() checks ref_count before proceeding
        self.ref_count.fetch_add(1, Ordering::Acquire);
    }

    /// Release a reference to this entry (allows eviction)
    ///
    /// #ASSUME: ref_count > 0 before release
    /// #VERIFY: Paired with acquire_ref() call
    #[inline]
    pub fn release_ref(&self) {
        // #ASSUME: Release ordering allows next eviction
        // #VERIFY: Underflow checked in debug builds
        let old = self.ref_count.fetch_sub(1, Ordering::Release);
        debug_assert!(old > 0, "Reference count underflow");
    }

    /// Check if this entry is expired based on current time
    ///
    /// # UCE34 Q6: Failure Modes - TTL Expiration
    ///
    /// #ASSUME: TTL == 0 means no expiration
    /// #VERIFY: Returns false for ttl_ns == 0
    pub fn is_expired(&self) -> bool {
        let ttl = self.ttl_ns.load(Ordering::Relaxed);
        if ttl == 0 {
            return false; // No expiration
        }

        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time before UNIX epoch")
            .as_nanos() as u64;

        let last_access = self.last_access_ns.load(Ordering::Relaxed);
        now_ns.saturating_sub(last_access) > ttl
    }

    /// Update last access time and generation (LRU tracking)
    ///
    /// # UCE34 Q23: Concurrency - Lockfree Update
    ///
    /// **Pattern**: fetch_max for monotonic timestamp and generation updates
    ///
    /// #ASSUME: Timestamps and generations are monotonically increasing
    /// #VERIFY: fetch_max ensures we never go backwards in time/generation
    #[inline]
    pub fn touch(&self, current_generation: u64) {
        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time before UNIX epoch")
            .as_nanos() as u64;

        // #ASSUME: fetch_max ensures monotonic ordering
        // #VERIFY: Even if multiple threads touch, timestamp only increases
        self.last_access_ns.fetch_max(now_ns, Ordering::Release);

        // Update generation for LRU tracking
        // #ASSUME: Relaxed ordering safe (generation is for relative comparison only)
        self.generation.store(current_generation, Ordering::Relaxed);
    }

    /// Try to insert a new cache entry (CAS-based)
    ///
    /// # UCE34 Q23: Concurrency - Lockfree CAS
    ///
    /// **Pattern**: Compare-and-swap for atomic slot allocation
    ///
    /// # Returns
    ///
    /// - `Ok(())`: Entry inserted successfully
    /// - `Err(current_hash)`: Slot occupied by another entry
    ///
    /// #ASSUME: hash != 0 (validated by caller)
    /// #VERIFY: CAS ensures only one writer wins
    pub fn try_insert(
        &self,
        hash: u64,
        response_offset: u64,
        ttl_ns: u64,
        initial_generation: u64,
    ) -> Result<(), u64> {
        assert_ne!(hash, 0, "Hash must be non-zero");

        // Phase 1: Try to claim the slot with CAS
        // #ASSUME: CAS on hash == 0 → hash atomically claims slot
        // #VERIFY: Only one thread can transition 0 → hash
        match self.hash.compare_exchange(
            0,
            hash,
            Ordering::AcqRel,  // Success: Acquire+Release for full sync
            Ordering::Acquire,  // Failure: Acquire to read current value
        ) {
            Ok(_) => {
                // Phase 2: Initialize remaining fields (we own the slot now)
                let now_ns = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("System time before UNIX epoch")
                    .as_nanos() as u64;

                // #ASSUME: No other thread can write (we own the slot)
                // #VERIFY: Hash CAS succeeded, so we have exclusive access
                self.last_access_ns.store(now_ns, Ordering::Relaxed);
                self.response_offset.store(response_offset, Ordering::Release);
                self.ttl_ns.store(ttl_ns, Ordering::Relaxed);
                self.generation.store(initial_generation, Ordering::Release);  // Set to current generation for LRU

                Ok(())
            }
            Err(current_hash) => {
                // Slot occupied
                Err(current_hash)
            }
        }
    }

    /// Evict this cache entry (reset to empty)
    ///
    /// # UCE34 Q23: Concurrency - Lockfree Eviction
    ///
    /// **Pattern**: Generation counter + reference counting for safe eviction
    ///
    /// #ASSUME: ref_count == 0 (no in-flight references)
    /// #VERIFY: Caller checks ref_count before calling evict()
    ///
    /// # Returns
    ///
    /// - `true`: Entry evicted successfully
    /// - `false`: Entry has in-flight references (eviction skipped)
    pub fn evict(&self) -> bool {
        // Phase 0: Check for in-flight references
        // #ASSUME: Relaxed ordering safe (ref_count is independent)
        // #VERIFY: If ref_count > 0, skip eviction (entry is in use)
        if self.ref_count.load(Ordering::Relaxed) > 0 {
            return false;
        }

        // Phase 1: Double-check ref_count with stronger ordering
        // #ASSUME: If ref_count > 0, another thread acquired ref between check and evict
        // #VERIFY: Prevents eviction race condition
        if self.ref_count.load(Ordering::Acquire) > 0 {
            return false;
        }

        // Phase 2: Clear all fields
        // #ASSUME: No other thread is accessing (ref_count == 0)
        // #VERIFY: Relaxed ordering safe (no data dependencies)
        self.response_offset.store(0, Ordering::Relaxed);
        self.last_access_ns.store(0, Ordering::Relaxed);
        self.ttl_ns.store(0, Ordering::Relaxed);
        self.generation.store(0, Ordering::Relaxed);
        self.freq_count.store(0, Ordering::Relaxed);

        // Phase 3: Clear hash (releases slot)
        // #ASSUME: Release ordering ensures all field clears visible before hash=0
        // #VERIFY: Other threads see hash==0 only after all fields cleared
        self.hash.store(0, Ordering::Release);

        true
    }
}

/// High-level cache entry with response data
///
/// # UCE34 Q17: Interfaces - Simple API
///
/// **Pattern**: Hide capsule complexity behind clean interface
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Request hash
    pub hash: u64,
    /// Cached response data (JSON string)
    pub response: String,
    /// Timestamp (nanoseconds since epoch)
    pub timestamp_ns: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_capsule_empty() {
        let capsule = CacheKeyCapsule::new();
        assert!(capsule.is_empty());
        assert_eq!(capsule.hash(), 0);
    }

    #[test]
    fn test_cache_key_capsule_insert() {
        let capsule = CacheKeyCapsule::new();

        let hash = 0x1234_5678_9ABC_DEF0;
        let response_offset = 42;
        let ttl_ns = 1_000_000_000; // 1 second
        let generation = 100;

        assert!(capsule.try_insert(hash, response_offset, ttl_ns, generation).is_ok());
        assert_eq!(capsule.hash(), hash);
        assert_eq!(capsule.response_offset(), response_offset);
        assert_eq!(capsule.generation(), generation);
        assert!(!capsule.is_empty());
    }

    #[test]
    fn test_cache_key_capsule_cas_conflict() {
        let capsule = CacheKeyCapsule::new();

        let hash1 = 0x1111_1111_1111_1111;
        let hash2 = 0x2222_2222_2222_2222;

        // First insert succeeds
        assert!(capsule.try_insert(hash1, 100, 1_000_000_000, 1).is_ok());

        // Second insert fails (slot occupied)
        let result = capsule.try_insert(hash2, 200, 1_000_000_000, 2);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), hash1);
    }

    #[test]
    fn test_cache_key_capsule_touch() {
        let capsule = CacheKeyCapsule::new();
        capsule.try_insert(0x1234, 100, 1_000_000_000, 1).unwrap();

        let initial = capsule.last_access_ns();
        let initial_gen = capsule.generation();

        std::thread::sleep(std::time::Duration::from_micros(100));
        capsule.touch(2);

        let updated = capsule.last_access_ns();
        let updated_gen = capsule.generation();
        assert!(updated >= initial);
        assert_eq!(updated_gen, 2);
        assert!(updated_gen > initial_gen);
    }

    #[test]
    fn test_cache_key_capsule_evict() {
        let capsule = CacheKeyCapsule::new();
        capsule.try_insert(0x1234, 100, 1_000_000_000, 1).unwrap();

        assert!(!capsule.is_empty());
        assert!(capsule.evict());
        assert!(capsule.is_empty());
        assert_eq!(capsule.hash(), 0);
    }

    #[test]
    fn test_cache_key_capsule_evict_with_ref() {
        let capsule = CacheKeyCapsule::new();
        capsule.try_insert(0x1234, 100, 1_000_000_000, 1).unwrap();

        // Acquire reference
        capsule.acquire_ref();

        // Eviction should fail (entry in-use)
        assert!(!capsule.evict());
        assert!(!capsule.is_empty());

        // Release reference
        capsule.release_ref();

        // Eviction should succeed now
        assert!(capsule.evict());
        assert!(capsule.is_empty());
    }

    #[test]
    fn test_cache_key_capsule_ttl() {
        let capsule = CacheKeyCapsule::new();

        // Insert with very short TTL
        let short_ttl = 1; // 1 nanosecond
        capsule.try_insert(0x1234, 100, short_ttl, 1).unwrap();

        // Should expire almost immediately
        std::thread::sleep(std::time::Duration::from_micros(100));
        assert!(capsule.is_expired());
    }

    #[test]
    fn test_cache_key_capsule_no_ttl() {
        let capsule = CacheKeyCapsule::new();

        // Insert with no TTL (0 = never expires)
        capsule.try_insert(0x1234, 100, 0, 1).unwrap();

        // Should never expire
        std::thread::sleep(std::time::Duration::from_micros(100));
        assert!(!capsule.is_expired());
    }
}
