//! LRU Cache Implementation - 100% Lockfree with Generation Counters
//!
//! # UCE34 Q10: Tier 6 Mixed Capsule (Atomic + Batch)
//!
//! **Tier 1 (Atomic)**: Lockfree CacheKeyCapsule array
//! **Tier 4 (Batch)**: Batch eviction for 10K+ entries
//! **Compound Speedup**: 30-1000× potential (3-10× atomic × 10-100× batch)
//!
//! # UCE34 Q13-Q21: Domain Analysis
//!
//! **Q13 (Resources)**: 1.28MB for 10K entries (128B × 10K)
//! **Q14 (Dependencies)**: atomic_capsule crate only
//! **Q15 (Scale)**: Linear with entry count (O(1) lookup, O(n) eviction)
//! **Q16 (Security)**: Hash-based addressing (no direct pointers exposed)
//! **Q17 (Interfaces)**: get(), insert(), evict_lru() - simple API
//! **Q18 (Testing)**: Unit + property + integration + stress tests
//! **Q19 (Monitoring)**: Atomic hit/miss/eviction counters
//! **Q20 (Error Handling)**: Result<T> for all fallible operations
//! **Q21 (Lifecycle)**: new() for allocation, Drop for cleanup

use super::capsule::CacheKeyCapsule;
use super::{CacheEntry, CacheError, Result};
use std::sync::atomic::{AtomicU64, Ordering};

use atomic_capsule::collections::ConcurrentMapCapsule;

#[cfg(test)]
use atomic_capsule::hash::const_fast_hash;

/// Cache configuration
///
/// # UCE34 Q32: Practical Constraints
///
/// **Default**: 10,000 entries × 128B = 1.28MB
/// **Max**: 100,000 entries × 128B = 12.8MB
pub struct CacheConfig {
    /// Maximum number of cache entries
    pub max_entries: usize,
    /// Default TTL (nanoseconds, 0 = no expiration)
    pub default_ttl_ns: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 10_000,
            default_ttl_ns: 3_600_000_000_000, // 1 hour in nanoseconds
        }
    }
}

/// Cache statistics for monitoring
///
/// # UCE34 Q19: Monitoring - Atomic Metrics
///
/// **Pattern**: Lockfree atomic counters (no external dependencies)
pub struct CacheStats {
    /// Total cache hits
    pub hits: AtomicU64,
    /// Total cache misses
    pub misses: AtomicU64,
    /// Total insertions
    pub inserts: AtomicU64,
    /// Total evictions
    pub evictions: AtomicU64,
    /// Total TTL expirations
    pub expirations: AtomicU64,
}

impl Default for CacheStats {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheStats {
    pub const fn new() -> Self {
        Self {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            inserts: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            expirations: AtomicU64::new(0),
        }
    }

    /// Get hit rate (0.0 to 1.0)
    ///
    /// #ASSUME: Relaxed ordering safe for statistics
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed) as f64;
        let misses = self.misses.load(Ordering::Relaxed) as f64;
        let total = hits + misses;

        if total == 0.0 {
            0.0
        } else {
            hits / total
        }
    }

    /// Get total requests
    pub fn total_requests(&self) -> u64 {
        self.hits.load(Ordering::Relaxed) + self.misses.load(Ordering::Relaxed)
    }

    /// Get eviction rate
    pub fn eviction_rate(&self) -> f64 {
        let evictions = self.evictions.load(Ordering::Relaxed) as f64;
        let inserts = self.inserts.load(Ordering::Relaxed) as f64;

        if inserts == 0.0 {
            0.0
        } else {
            evictions / inserts
        }
    }
}

/// LRU Cache - 100% Lockfree with Atomic Capsules
///
/// # UCE34 Q22: State Management
///
/// **State**: Fixed-size array of CacheKeyCapsule (preallocated)
/// **Concurrency**: 100% lockfree (no mutex/RwLock anywhere)
/// **Eviction**: Batch scan for LRU entry (Tier 4)
pub struct LruCache {
    /// Fixed-size cache entry array
    ///
    /// #ASSUME: Preallocated, never resizes
    /// #VERIFY: size == config.max_entries
    entries: Box<[CacheKeyCapsule]>,

    /// Separate response storage (hash → response JSON)
    ///
    /// #ASSUME: ConcurrentMapCapsule provides lockfree concurrent access
    /// #VERIFY: ConcurrentMapCapsule is lockfree (atomic_capsule guarantee, T4 tier)
    responses: ConcurrentMapCapsule<u64, String>,

    /// Cache configuration
    config: CacheConfig,

    /// Cache statistics (atomic counters)
    ///
    /// #ASSUME: Relaxed ordering safe for statistics
    stats: CacheStats,

    /// Global generation counter for LRU tracking
    ///
    /// #ASSUME: Incremented on every cache access (get/insert/touch)
    /// #VERIFY: Used to calculate generation distance for LRU eviction
    global_generation: AtomicU64,
}

impl LruCache {
    /// Create a new LRU cache with the given configuration
    ///
    /// # UCE34 Q21: Lifecycle - Initialization
    ///
    /// **Pattern**: Preallocate fixed-size array of capsules
    ///
    /// #ASSUME: max_entries > 0
    /// #VERIFY: Validated in constructor
    pub fn new(config: CacheConfig) -> Self {
        assert!(config.max_entries > 0, "max_entries must be > 0");

        // Preallocate cache entries
        let entries = (0..config.max_entries)
            .map(|_| CacheKeyCapsule::new())
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Self {
            entries,
            responses: ConcurrentMapCapsule::new(),
            config,
            stats: CacheStats::new(),
            global_generation: AtomicU64::new(0),
        }
    }

    /// Create a new LRU cache with default configuration
    pub fn with_default_config() -> Self {
        Self::new(CacheConfig::default())
    }

    /// Get a cached response by request hash
    ///
    /// # UCE34 Q23: Concurrency - Lockfree Lookup
    ///
    /// **Performance**: <100ns cache hit (target)
    /// **Pattern**: Hash-based index + atomic loads
    ///
    /// # Returns
    ///
    /// - `Ok(CacheEntry)`: Cache hit
    /// - `Err(CacheError::CacheMiss)`: Entry not found
    /// - `Err(CacheError::TtlExpired)`: Entry expired
    ///
    /// #ASSUME: Acquire ordering ensures response data visibility
    /// #VERIFY: Capsule uses Acquire for hash load
    pub fn get(&self, request_hash: u64) -> Result<CacheEntry> {
        if request_hash == 0 {
            return Err(CacheError::InvalidHash);
        }

        // Hash-based index with linear probing for lookup
        // #ASSUME: Linear probing matches insert() strategy
        // #VERIFY: Empty slot (hash == 0) indicates end of probe chain
        let base_index = (request_hash as usize) % self.entries.len();
        const LINEAR_PROBE_LIMIT: usize = 256;

        for probe in 0..LINEAR_PROBE_LIMIT {
            let index = (base_index + probe) % self.entries.len();
            let entry = &self.entries[index];
            let stored_hash = entry.hash();

            if stored_hash == 0 {
                // Empty slot - end of chain
                self.stats.misses.fetch_add(1, Ordering::Relaxed);
                return Err(CacheError::CacheMiss(request_hash));
            }

            if stored_hash != request_hash {
                // Different entry - continue probing
                continue;
            }

            // Found matching entry - proceed to access it
            // Acquire reference to prevent eviction during access
            // #ASSUME: Acquire ordering prevents eviction race
            // #VERIFY: ref_count > 0 blocks eviction in evict()
            entry.acquire_ref();

            // Check TTL
            if entry.is_expired() {
                self.stats.expirations.fetch_add(1, Ordering::Relaxed);
                self.stats.misses.fetch_add(1, Ordering::Relaxed);

                // Release reference before eviction
                entry.release_ref();

                // Evict expired entry
                entry.evict();
                self.responses.remove(&stored_hash);

                return Err(CacheError::TtlExpired {
                    hash: request_hash,
                    expired_ns: entry.last_access_ns(),
                });
            }

            // Cache hit - update LRU timestamp and frequency
            let gen = self.next_generation();
            entry.touch(gen);
            entry.increment_freq();
            self.stats.hits.fetch_add(1, Ordering::Relaxed);

            // Retrieve response from separate storage
            let response = self.responses.get(&stored_hash).cloned().ok_or_else(|| {
                // Release reference on error
                entry.release_ref();
                CacheError::CacheMiss(request_hash)
            })?;

            // Release reference after successful read
            // #ASSUME: Release ordering allows next eviction
            entry.release_ref();

            return Ok(CacheEntry {
                hash: stored_hash,
                response,
                timestamp_ns: entry.last_access_ns(),
            });
        }

        // No matching entry found after probing all slots
        self.stats.misses.fetch_add(1, Ordering::Relaxed);
        Err(CacheError::CacheMiss(request_hash))
    }

    /// Insert a new cache entry
    ///
    /// # UCE34 Q23: Concurrency - Lockfree Insertion
    ///
    /// **Performance**: <200ns insertion (target)
    /// **Pattern**: CAS-based slot allocation + eviction if full
    ///
    /// # Returns
    ///
    /// - `Ok(())`: Entry inserted successfully
    /// - `Err(CacheError::CacheFull)`: All slots occupied (eviction triggered)
    ///
    /// #ASSUME: request_hash != 0
    /// #VERIFY: Validated in method
    pub fn insert(&self, request_hash: u64, response: String) -> Result<()> {
        if request_hash == 0 {
            return Err(CacheError::InvalidHash);
        }

        // Hash-based index with linear probing for collision resolution
        // #ASSUME: Linear probing finds empty or matching slot within array bounds
        // #VERIFY: Stride of 1 ensures all slots are reachable (O(n) worst case)
        let base_index = (request_hash as usize) % self.entries.len();
        let ttl_ns = self.config.default_ttl_ns;
        const MAX_EVICTIONS: usize = 10;
        const LINEAR_PROBE_LIMIT: usize = 256; // Probe up to 256 slots for collision resolution

        for eviction_attempt in 0..MAX_EVICTIONS {
            // Step 1: Linear probing to find slot
            for probe in 0..LINEAR_PROBE_LIMIT {
                let index = (base_index + probe) % self.entries.len();
                let entry = &self.entries[index];
                let gen = self.next_generation();

                match entry.try_insert(request_hash, index as u64, ttl_ns, gen) {
                    Ok(()) => {
                        // Success - store response
                        let _ = self.responses.insert(request_hash, response);
                        self.stats.inserts.fetch_add(1, Ordering::Relaxed);
                        return Ok(());
                    }
                    Err(current_hash) if current_hash == request_hash => {
                        // Same entry exists - update response
                        let _ = self.responses.insert(request_hash, response);
                        let gen = self.next_generation();
                        entry.touch(gen);
                        return Ok(());
                    }
                    Err(_) => {
                        // Collision with different entry - continue probing
                        continue;
                    }
                }
            }

            // Step 2: All probe slots occupied or MAX_PROBE reached - trigger eviction
            if eviction_attempt < MAX_EVICTIONS - 1 {
                self.evict_lru()?;
                // Loop will retry with linear probing from start
            } else {
                // Max eviction attempts exceeded
                return Err(CacheError::CacheFull {
                    current: self.len(),
                    max: self.entries.len(),
                });
            }
        }

        // Should never reach here
        Err(CacheError::CacheFull {
            current: self.len(),
            max: self.entries.len(),
        })
    }

    /// Evict the least recently used entry (frequency-weighted LRU)
    ///
    /// # UCE34 Q23: Concurrency - Batch Eviction (Tier 4)
    ///
    /// **Performance**: O(n) scan for LRU entry
    /// **Pattern**: Frequency-weighted LRU scoring (hot entries survive longer)
    ///
    /// # Algorithm
    ///
    /// **Score Calculation**: `score = generation_distance / (freq_count + 1)`
    /// - `generation_distance`: How long since last access (higher = older)
    /// - `freq_count`: Number of cache hits (higher = hotter)
    /// - Hot entries (high freq) have LOW score (survive eviction)
    /// - Cold entries (low freq) have HIGH score (evicted first)
    ///
    /// **Example**:
    /// - Entry A: distance=1000, freq=10 → score = 100.0 (hot, keep)
    /// - Entry B: distance=1000, freq=1 → score = 500.0 (cold, evict)
    ///
    /// # Returns
    ///
    /// - `Ok(())`: Entry evicted successfully
    /// - `Err(CacheError::CacheFull)`: No evictable entries found
    ///
    /// #ASSUME: At least one non-empty slot exists
    /// #VERIFY: Returns error if all slots empty or in-use
    pub fn evict_lru(&self) -> Result<()> {
        const MAX_RETRIES: usize = 100; // Retry limit to prevent infinite loops

        for _ in 0..MAX_RETRIES {
            // Get current global generation for distance calculation
            let current_gen = self.current_generation();

            // Tier 4: Batch scan for LRU entry with frequency-weighted scoring
            let mut victim_index = None;
            let mut max_score = f64::MIN;

            for (index, entry) in self.entries.iter().enumerate() {
                if entry.is_empty() {
                    continue;
                }

                // Skip entries with in-flight references
                // #ASSUME: ref_count > 0 means entry is actively being used
                // #VERIFY: Skip eviction for in-use entries
                if entry.ref_count() > 0 {
                    continue;
                }

                // Calculate generation distance (current - last_access_generation)
                // #ASSUME: Generation is monotonically increasing from global counter
                // #VERIFY: Distance represents "age" since last access (higher = older)
                let last_gen = entry.generation();
                let distance = current_gen.saturating_sub(last_gen);

                // Calculate frequency-weighted score
                // #ASSUME: freq_count + 1 prevents division by zero
                // #VERIFY: Higher freq → lower score → survives eviction
                // #VERIFY: Higher distance → higher score → evicted first
                let freq = entry.freq_count();
                let score = distance as f64 / (freq + 1) as f64;

                // Select entry with MAXIMUM score (oldest + coldest)
                // #ASSUME: max_score initially set to f64::MIN
                // #VERIFY: First valid entry will have score > MIN
                if score > max_score {
                    max_score = score;
                    victim_index = Some(index);
                }
            }

            match victim_index {
                Some(index) => {
                    let entry = &self.entries[index];
                    let hash = entry.hash();

                    // Evict entry and remove response
                    // #ASSUME: evict() checks ref_count again (double-check)
                    // #VERIFY: Returns false if ref_count > 0 (race condition)
                    if entry.evict() {
                        self.responses.remove(&hash);
                        self.stats.evictions.fetch_add(1, Ordering::Relaxed);
                        return Ok(());
                    }
                    // Entry became in-use between check and evict, loop will retry with different victim
                }
                None => {
                    // No evictable entries (all slots empty or in-use)
                    return Err(CacheError::CacheFull {
                        current: self.entries.len(),
                        max: self.entries.len(),
                    });
                }
            }
        }

        // Max retries exceeded (extremely unlikely in practice)
        Err(CacheError::CacheFull {
            current: self.entries.len(),
            max: self.entries.len(),
        })
    }

    /// Get and increment the global generation counter
    ///
    /// # UCE34 Q22: State Management - Generation Counter
    ///
    /// **Pattern**: Monotonic global generation for LRU distance tracking
    ///
    /// #ASSUME: Called on every cache access (get, insert, touch)
    /// #VERIFY: Returns unique generation number for each access
    fn next_generation(&self) -> u64 {
        // #ASSUME: Relaxed ordering safe (monotonic counter, no data dependency)
        // #VERIFY: fetch_add ensures unique generation per access
        self.global_generation.fetch_add(1, Ordering::Relaxed)
    }

    /// Get current generation (without incrementing)
    fn current_generation(&self) -> u64 {
        self.global_generation.load(Ordering::Relaxed)
    }

    /// Get cache statistics
    ///
    /// # UCE34 Q19: Monitoring
    ///
    /// #ASSUME: Relaxed ordering safe for statistics
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Get cache configuration
    pub fn config(&self) -> &CacheConfig {
        &self.config
    }

    /// Clear all cache entries
    ///
    /// # UCE34 Q21: Lifecycle - Cleanup
    ///
    /// **Pattern**: Batch eviction for all entries
    pub fn clear(&self) {
        for entry in self.entries.iter() {
            if !entry.is_empty() {
                // Ignore result - clear() is best-effort
                let _ = entry.evict();
            }
        }
        self.responses.clear();
    }

    /// Get current entry count (expensive - O(n) scan)
    ///
    /// #ASSUME: Used for debugging/monitoring only (not hot path)
    pub fn len(&self) -> usize {
        self.entries.iter().filter(|e| !e.is_empty()).count()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// Implement Default trait for API compatibility with tests
impl Default for LruCache {
    fn default() -> Self {
        Self::with_default_config()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lru_cache_insert_and_get() {
        let cache = LruCache::default();

        let hash = const_fast_hash(b"test_request");
        let response = "test_response".to_string();

        // Insert
        cache.insert(hash, response.clone()).unwrap();

        // Get
        let entry = cache.get(hash).unwrap();
        assert_eq!(entry.hash, hash);
        assert_eq!(entry.response, response);
    }

    #[test]
    fn test_lru_cache_miss() {
        let cache = LruCache::default();

        let hash = const_fast_hash(b"nonexistent");
        let result = cache.get(hash);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CacheError::CacheMiss(_)));
    }

    #[test]
    fn test_lru_cache_update() {
        let cache = LruCache::default();

        let hash = const_fast_hash(b"test_request");
        cache.insert(hash, "response1".to_string()).unwrap();
        cache.insert(hash, "response2".to_string()).unwrap();

        let entry = cache.get(hash).unwrap();
        assert_eq!(entry.response, "response2");
    }

    #[test]
    fn test_lru_cache_eviction() {
        let mut config = CacheConfig::default();
        config.max_entries = 10; // Small cache for testing

        let cache = LruCache::new(config);

        // Fill cache
        for i in 0..10 {
            let hash = const_fast_hash(format!("request_{}", i).as_bytes());
            cache.insert(hash, format!("response_{}", i)).unwrap();
        }

        // Record evictions before explicit evict (may have occurred during filling due to hash collisions)
        let evictions_before = cache.stats().evictions.load(Ordering::Relaxed);

        // Evict LRU
        cache.evict_lru().unwrap();

        // Verify eviction counter increased by exactly 1
        let evictions_after = cache.stats().evictions.load(Ordering::Relaxed);
        assert_eq!(
            evictions_after - evictions_before,
            1,
            "Expected exactly 1 eviction, got {}",
            evictions_after - evictions_before
        );
    }

    #[test]
    fn test_lru_cache_hit_rate() {
        let cache = LruCache::default();

        let hash1 = const_fast_hash(b"request1");
        let hash2 = const_fast_hash(b"request2");

        cache.insert(hash1, "response1".to_string()).unwrap();

        // 1 hit
        cache.get(hash1).unwrap();

        // 1 miss
        let _ = cache.get(hash2);

        let hit_rate = cache.stats().hit_rate();
        assert!((hit_rate - 0.5).abs() < 0.01); // 50% hit rate
    }

    #[test]
    fn test_lru_cache_clear() {
        let cache = LruCache::default();

        let hash = const_fast_hash(b"test");
        cache.insert(hash, "response".to_string()).unwrap();

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_lru_cache_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let cache = Arc::new(LruCache::default());
        let mut handles = vec![];

        // Spawn 10 threads inserting and reading
        for i in 0..10 {
            let cache_clone = Arc::clone(&cache);
            let handle = thread::spawn(move || {
                for j in 0..100 {
                    let hash = const_fast_hash(format!("request_{}_{}", i, j).as_bytes());
                    let response = format!("response_{}_{}", i, j);

                    cache_clone.insert(hash, response.clone()).unwrap();
                    let _ = cache_clone.get(hash);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify cache has entries
        assert!(cache.len() > 0);

        // Verify hit rate is reasonable
        let hit_rate = cache.stats().hit_rate();
        assert!(hit_rate > 0.0);
    }
}
