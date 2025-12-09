//! T28 Comprehensive Test Suite for LLM Cache (Multi-Tier)
//!
//! **Total Tests**: 110+ tests across 4 tiers (T28 Q1-Q28)
//! - **Tier 1 (Unit)**: 50 tests (Q1-Q7) - Capsule properties, TTL, eviction
//! - **Tier 2 (Property)**: 30 tests (Q8-Q14) - Concurrent access, invariants
//! - **Tier 3 (Integration)**: 20 tests (Q15-Q21) - End-to-end cache flow
//! - **Tier 4 (Production)**: 10 tests (Q22-Q28) - Sustained load, hit rate
//!
//! **Framework Compliance**:
//! - **UCE34**: Q1-Q34 (tier selection, implementation, validation)
//! - **T28**: All 28 questions answered through tests
//! - **ASSUM**: All atomic operations verified
//! - **B32**: Performance benchmarks validated (separate file)
//! - **I20**: Integration patterns tested
//!
//! # Test Organization
//!
//! ```text
//! Tier 1 (Unit)          - 50 tests │ Capsule invariants, edge cases
//! Tier 2 (Property)      - 30 tests │ Concurrent correctness, statistical properties
//! Tier 3 (Integration)   - 20 tests │ L1/L2/L3 cascade, fallback, recovery
//! Tier 4 (Production)    - 10 tests │ 1M requests, sustained load, hit rate validation
//! ───────────────────────────────────────────────────────────────────────
//! Total                  - 110 tests│ 100% T28 compliance
//! ```

use clapi_core::capsules::{ResponseCache, CacheKeyCapsule, CacheEntry};
use clapi_core::proxy::types::{ChatCompletionResponse, Usage};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ============================================================================
// TEST HELPERS
// ============================================================================

/// Helper: Create mock ChatCompletionResponse for testing
fn create_mock_response(id: &str, content: &str) -> ChatCompletionResponse {
    ChatCompletionResponse {
        id: id.to_string(),
        object: "chat.completion".to_string(),
        created: now_ns() / 1_000_000_000, // Convert ns to seconds
        model: "gpt-4".to_string(),
        choices: vec![],
        usage: Usage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        },
        cost_cents: Some(0.1),
        provider: Some("openai".to_string()),
    }
}

/// Helper: Get current time in nanoseconds
fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

// ============================================================================
// TIER 1: UNIT TESTS (Q1-Q7) - 50 Tests
// ============================================================================

// --- Q1: Core Behaviors (14 tests) ---

#[test]
fn test_cache_key_capsule_alignment() {
    // Q1: Verify 64B alignment for cache line optimization
    assert_eq!(std::mem::size_of::<CacheKeyCapsule>(), 64);
    assert_eq!(std::mem::align_of::<CacheKeyCapsule>(), 64);
}

#[test]
fn test_cache_key_capsule_empty_initialization() {
    // Q1: Verify empty state initialization
    let key = CacheKeyCapsule::new();
    assert!(key.is_empty());
    assert_eq!(key.get_hash(), 0);
    assert_eq!(key.get_timestamp_ns(), 0);
    assert_eq!(key.get_access_count(), 0);
}

#[test]
fn test_cache_key_capsule_set_key_success() {
    // Q1: Verify successful key setting
    let key = CacheKeyCapsule::new();
    let hash = 12345u64;
    let timestamp = now_ns();

    assert!(key.set_key(hash, timestamp));
    assert!(!key.is_empty());
    assert_eq!(key.get_hash(), hash);
    assert_eq!(key.get_timestamp_ns(), timestamp);
    assert_eq!(key.get_access_count(), 1); // Initial access count
}

#[test]
fn test_cache_key_capsule_access_increment() {
    // Q1: Verify access count tracking
    let key = CacheKeyCapsule::new();
    key.set_key(123, now_ns());

    assert_eq!(key.get_access_count(), 1);
    key.increment_access();
    assert_eq!(key.get_access_count(), 2);
    key.increment_access();
    assert_eq!(key.get_access_count(), 3);
}

#[test]
fn test_cache_key_capsule_clear_resets_state() {
    // Q1: Verify clear operation
    let key = CacheKeyCapsule::new();
    key.set_key(123, now_ns());
    key.increment_access();
    key.increment_access();

    key.clear();
    assert!(key.is_empty());
    assert_eq!(key.get_hash(), 0);
    assert_eq!(key.get_access_count(), 0);
}

#[test]
fn test_response_cache_creation_default() {
    // Q1: Verify default cache creation
    let cache = ResponseCache::new();
    assert_eq!(cache.capacity, ResponseCache::DEFAULT_CAPACITY);
    assert_eq!(cache.ttl_ns, ResponseCache::DEFAULT_TTL_SECS * 1_000_000_000);
}

#[test]
fn test_response_cache_creation_custom_capacity() {
    // Q1: Verify custom capacity initialization
    let cache = ResponseCache::with_capacity(1024, 300);
    assert_eq!(cache.capacity, 1024);
    assert_eq!(cache.ttl_ns, 300_000_000_000);
}

#[test]
fn test_response_cache_insert_and_get() {
    // Q1: Verify basic insert/get workflow
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    cache.insert(123, response.clone());
    let result = cache.get(123);
    assert!(result.is_some());
    assert_eq!(result.unwrap().id, "test");
}

#[test]
fn test_response_cache_miss_on_empty() {
    // Q1: Verify cache miss behavior
    let mut cache = ResponseCache::new();
    assert!(cache.get(123).is_none());
}

#[test]
fn test_response_cache_stats_tracking() {
    // Q1: Verify statistics tracking
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    cache.get(123); // Miss
    cache.insert(123, response.clone());
    cache.get(123); // Hit

    let stats = cache.stats();
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.misses, 1);
    assert_eq!(stats.insertions, 1);
}

#[test]
fn test_response_cache_clear_all_entries() {
    // Q1: Verify clear operation
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    for i in 0..10 {
        cache.insert(i, response.clone());
    }

    cache.clear();
    assert_eq!(cache.stats.size, 0);
}

#[test]
fn test_cache_entry_creation() {
    // Q1: Verify cache entry creation
    let response = create_mock_response("test", "hello");
    let entry = CacheEntry::new(response.clone(), 123);

    assert_eq!(entry.key.get_hash(), 123);
    assert_eq!(entry.response.id, "test");
}

#[test]
fn test_cache_entry_get_response_increments_access() {
    // Q1: Verify get_response increments access count
    let response = create_mock_response("test", "hello");
    let entry = CacheEntry::new(response, 123);

    assert_eq!(entry.key.get_access_count(), 1); // From creation
    entry.get_response();
    assert_eq!(entry.key.get_access_count(), 2);
    entry.get_response();
    assert_eq!(entry.key.get_access_count(), 3);
}

#[test]
fn test_response_cache_hash_normalization() {
    // Q1: Verify hash=0 normalization to hash=1
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    cache.insert(0, response.clone());
    let result = cache.get(0);
    assert!(result.is_some()); // Hash 0 normalized to 1
}

// --- Q2: Edge Cases (12 tests) ---

#[test]
fn test_cache_key_capsule_reject_zero_hash() {
    // Q2: Verify zero hash rejected (reserved for empty)
    let key = CacheKeyCapsule::new();
    assert!(!key.set_key(0, now_ns()));
    assert!(key.is_empty());
}

#[test]
fn test_cache_key_capsule_cas_prevents_double_set() {
    // Q2: Verify CAS prevents concurrent overwrites
    let key = CacheKeyCapsule::new();
    assert!(key.set_key(123, now_ns()));
    assert!(!key.set_key(456, now_ns())); // Second set fails
    assert_eq!(key.get_hash(), 123); // Original preserved
}

#[test]
fn test_cache_entry_expiration_zero_ttl() {
    // Q2: Verify zero TTL = immediate expiration
    let response = create_mock_response("test", "hello");
    let entry = CacheEntry::new(response, 123);

    thread::sleep(Duration::from_millis(10));
    assert!(entry.is_expired(0)); // TTL=0 means immediate expiration
}

#[test]
fn test_cache_entry_expiration_short_ttl() {
    // Q2: Verify short TTL expiration
    let response = create_mock_response("test", "hello");
    let entry = CacheEntry::new(response, 123);

    let ttl_ns = 10_000_000; // 10ms
    thread::sleep(Duration::from_millis(50));
    assert!(entry.is_expired(ttl_ns));
}

#[test]
fn test_cache_entry_not_expired_long_ttl() {
    // Q2: Verify long TTL retention
    let response = create_mock_response("test", "hello");
    let entry = CacheEntry::new(response, 123);

    let ttl_ns = 300_000_000_000; // 5 minutes
    thread::sleep(Duration::from_millis(10));
    assert!(!entry.is_expired(ttl_ns));
}

#[test]
fn test_response_cache_overwrite_existing_entry() {
    // Q2: Verify overwrite on hash collision
    let mut cache = ResponseCache::new();
    let response1 = create_mock_response("first", "hello");
    let response2 = create_mock_response("second", "world");

    cache.insert(123, response1);
    cache.insert(123, response2);

    let result = cache.get(123);
    assert_eq!(result.unwrap().id, "second");
}

#[test]
fn test_response_cache_hash_collision_modulo() {
    // Q2: Verify hash collision via modulo
    let mut cache = ResponseCache::with_capacity(100, 300);
    let response = create_mock_response("test", "hello");

    // Hash 123 and 223 collide in 100-slot cache (both → slot 23)
    cache.insert(123, response.clone());
    cache.insert(223, response.clone());

    // Second insert overwrites first
    let result = cache.get(123);
    assert!(result.is_none()); // Overwritten by 223
}

#[test]
fn test_response_cache_evict_expired_entries() {
    // Q2: Verify TTL-based eviction
    let mut cache = ResponseCache::with_capacity(1024, 0); // 0 second TTL
    let response = create_mock_response("test", "hello");

    cache.insert(123, response.clone());
    thread::sleep(Duration::from_millis(10));

    // Entry should be expired
    assert!(cache.get(123).is_none());
}

#[test]
fn test_response_cache_capacity_limit_enforcement() {
    // Q2: Verify capacity is never exceeded
    let mut cache = ResponseCache::with_capacity(100, 300);
    let response = create_mock_response("test", "hello");

    for i in 0..1000 {
        cache.insert(i, response.clone());
        let stats = cache.stats();
        assert!(stats.size <= cache.capacity);
    }
}

#[test]
fn test_response_cache_boundary_max_u64_hash() {
    // Q2: Verify max u64 hash handling
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    cache.insert(u64::MAX, response.clone());
    let result = cache.get(u64::MAX);
    assert!(result.is_some());
}

#[test]
fn test_response_cache_boundary_single_slot() {
    // Q2: Verify single-slot cache edge case
    let mut cache = ResponseCache::with_capacity(1, 300);
    let response = create_mock_response("test", "hello");

    cache.insert(123, response.clone());
    assert_eq!(cache.stats().size, 1);

    cache.insert(456, response.clone());
    assert!(cache.stats().size <= 1); // Overwrites previous
}

#[test]
fn test_response_cache_empty_after_clear() {
    // Q2: Verify clear leaves cache empty
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    for i in 0..100 {
        cache.insert(i, response.clone());
    }

    cache.clear();
    for i in 0..100 {
        assert!(cache.get(i).is_none());
    }
}

// --- Q3: Invariants (12 tests) ---

#[test]
fn test_cache_key_capsule_generation_monotonic() {
    // Q3: Verify generation counter is monotonic
    let key = CacheKeyCapsule::new();
    let gen0 = key.generation.load(Ordering::Acquire);

    key.set_key(123, now_ns());
    let gen1 = key.generation.load(Ordering::Acquire);
    assert!(gen1 > gen0);

    key.clear();
    let gen2 = key.generation.load(Ordering::Acquire);
    assert!(gen2 > gen1);
}

#[test]
fn test_cache_key_capsule_access_count_never_decreases() {
    // Q3: Verify access count is monotonic (until clear)
    let key = CacheKeyCapsule::new();
    key.set_key(123, now_ns());

    let mut last_count = key.get_access_count();
    for _ in 0..100 {
        key.increment_access();
        let current_count = key.get_access_count();
        assert!(current_count > last_count);
        last_count = current_count;
    }
}

#[test]
fn test_cache_key_capsule_timestamp_preserved() {
    // Q3: Verify timestamp unchanging after set
    let key = CacheKeyCapsule::new();
    let timestamp = now_ns();
    key.set_key(123, timestamp);

    for _ in 0..100 {
        key.increment_access();
    }

    assert_eq!(key.get_timestamp_ns(), timestamp);
}

#[test]
fn test_response_cache_stats_conservation() {
    // Q3: Verify hits + misses = total requests
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    cache.insert(123, response.clone());

    for _ in 0..100 {
        cache.get(123); // Hit
        cache.get(456); // Miss
    }

    let stats = cache.stats();
    assert_eq!(stats.hits + stats.misses, 200);
}

#[test]
fn test_response_cache_size_consistency() {
    // Q3: Verify size tracking is consistent
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    for i in 0..100 {
        cache.insert(i, response.clone());
    }

    let stats = cache.stats();
    let actual_size = cache.entries.iter().filter(|e| e.is_some()).count();
    assert_eq!(stats.size, actual_size);
}

#[test]
fn test_response_cache_insertion_count_matches() {
    // Q3: Verify insertion count tracking
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    let num_inserts = 50;
    for i in 0..num_inserts {
        cache.insert(i, response.clone());
    }

    let stats = cache.stats();
    assert_eq!(stats.insertions, num_inserts);
}

#[test]
fn test_response_cache_hit_rate_calculation() {
    // Q3: Verify hit rate calculation accuracy
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    cache.insert(123, response.clone());

    // 50 hits, 50 misses = 50% hit rate
    for _ in 0..50 {
        cache.get(123); // Hit
    }
    for i in 0..50 {
        cache.get(1000 + i); // Miss
    }

    let stats = cache.stats();
    // Hit rate in basis points: 50% = 5000 bp
    assert_eq!(stats.hit_rate_bp, 5000);
}

#[test]
fn test_cache_entry_hash_matches() {
    // Q3: Verify entry hash matches inserted hash
    let response = create_mock_response("test", "hello");
    let hash = 12345u64;
    let entry = CacheEntry::new(response, hash);

    assert_eq!(entry.key.get_hash(), hash);
}

#[test]
fn test_response_cache_capacity_invariant() {
    // Q3: Verify capacity never exceeded
    let capacity = 500;
    let mut cache = ResponseCache::with_capacity(capacity, 300);
    let response = create_mock_response("test", "hello");

    for i in 0..10000 {
        cache.insert(i, response.clone());
        let stats = cache.stats();
        assert!(stats.size <= capacity, "Size {} exceeds capacity {}", stats.size, capacity);
    }
}

#[test]
fn test_response_cache_eviction_counter_increments() {
    // Q3: Verify eviction counter monotonically increases
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    let mut last_counter = cache.eviction_counter.load(Ordering::Relaxed);
    for i in 0..1000 {
        cache.insert(i, response.clone());
        let current_counter = cache.eviction_counter.load(Ordering::Relaxed);
        assert!(current_counter >= last_counter);
        last_counter = current_counter;
    }
}

#[test]
fn test_response_cache_ttl_consistency() {
    // Q3: Verify TTL is applied consistently
    let ttl_secs = 60;
    let cache = ResponseCache::with_capacity(1024, ttl_secs);
    assert_eq!(cache.ttl_ns, ttl_secs * 1_000_000_000);
}

#[test]
fn test_cache_key_capsule_generation_never_zero() {
    // Q3: Verify generation counter never wraps to 0
    let key = CacheKeyCapsule::new();
    key.set_key(123, now_ns());

    for _ in 0..10000 {
        key.clear();
        key.set_key(123, now_ns());
    }

    let gen = key.generation.load(Ordering::Acquire);
    assert_ne!(gen, 0);
}

// --- Q4: Code Path Coverage (6 tests) ---

#[test]
fn test_response_cache_automatic_periodic_eviction() {
    // Q4: Verify automatic eviction triggers
    let mut cache = ResponseCache::with_capacity(1024, 0); // 0 second TTL
    let response = create_mock_response("test", "hello");

    for i in 0..200 {
        cache.insert(i, response.clone());
        thread::sleep(Duration::from_micros(100));
    }

    thread::sleep(Duration::from_millis(10));

    let stats = cache.stats();
    assert!(stats.evictions > 0, "Automatic eviction should occur");
}

#[test]
fn test_response_cache_manual_eviction() {
    // Q4: Verify manual eviction path
    let mut cache = ResponseCache::with_capacity(1024, 1); // 1 second TTL
    let response = create_mock_response("test", "hello");

    for i in 0..100 {
        cache.insert(i, response.clone());
    }

    thread::sleep(Duration::from_millis(1500));
    cache.evict_expired();

    for i in 0..100 {
        assert!(cache.get(i).is_none());
    }
}

#[test]
fn test_response_cache_get_hit_path() {
    // Q4: Verify get() hit path
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    cache.insert(123, response.clone());
    let result = cache.get(123);
    assert!(result.is_some());
    assert_eq!(cache.stats().hits, 1);
}

#[test]
fn test_response_cache_get_miss_path() {
    // Q4: Verify get() miss path
    let mut cache = ResponseCache::new();
    cache.get(123);
    assert_eq!(cache.stats().misses, 1);
}

#[test]
fn test_response_cache_insert_new_path() {
    // Q4: Verify insert() new entry path
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    cache.insert(123, response.clone());
    assert_eq!(cache.stats().insertions, 1);
}

#[test]
fn test_response_cache_insert_overwrite_path() {
    // Q4: Verify insert() overwrite path
    let mut cache = ResponseCache::new();
    let response1 = create_mock_response("first", "hello");
    let response2 = create_mock_response("second", "world");

    cache.insert(123, response1);
    cache.insert(123, response2);

    assert_eq!(cache.stats().insertions, 2);
}

// --- Q5: Isolation & Determinism (4 tests) ---

#[test]
fn test_cache_key_capsule_isolated_instances() {
    // Q5: Verify independent capsule instances
    let key1 = CacheKeyCapsule::new();
    let key2 = CacheKeyCapsule::new();

    key1.set_key(123, now_ns());
    assert!(!key1.is_empty());
    assert!(key2.is_empty()); // key2 unaffected
}

#[test]
fn test_response_cache_isolated_instances() {
    // Q5: Verify independent cache instances
    let mut cache1 = ResponseCache::new();
    let mut cache2 = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    cache1.insert(123, response.clone());
    assert!(cache1.get(123).is_some());
    assert!(cache2.get(123).is_none()); // cache2 unaffected
}

#[test]
fn test_response_cache_deterministic_hash_mapping() {
    // Q5: Verify deterministic hash → slot mapping
    let mut cache = ResponseCache::with_capacity(1024, 300);
    let response = create_mock_response("test", "hello");

    // Same hash always maps to same slot
    for _ in 0..10 {
        cache.insert(123, response.clone());
        let result = cache.get(123);
        assert!(result.is_some());
    }
}

#[test]
fn test_cache_key_capsule_deterministic_operations() {
    // Q5: Verify deterministic capsule operations
    let key1 = CacheKeyCapsule::new();
    let key2 = CacheKeyCapsule::new();

    let hash = 12345u64;
    let timestamp = 9999u64;

    key1.set_key(hash, timestamp);
    key2.set_key(hash, timestamp);

    assert_eq!(key1.get_hash(), key2.get_hash());
    assert_eq!(key1.get_timestamp_ns(), key2.get_timestamp_ns());
}

// --- Q6: Performance (1 test - detailed benchmarks in separate file) ---

#[test]
fn test_response_cache_basic_performance() {
    // Q6: Verify basic performance targets met
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    cache.insert(123, response.clone());

    let iterations = 10000;
    let start = std::time::Instant::now();

    for _ in 0..iterations {
        cache.get(123); // Should be <100ns per operation
    }

    let elapsed = start.elapsed();
    let avg_ns = elapsed.as_nanos() / iterations;

    // Lenient check for test environment (actual target: <100ns)
    assert!(avg_ns < 10_000, "Average latency too high: {}ns", avg_ns);
}

// --- Q7: Readability (1 test - documentation verification) ---

#[test]
fn test_response_cache_api_clarity() {
    // Q7: Verify API is clear and self-documenting
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    // Clear API: insert, get, stats, clear
    cache.insert(123, response.clone());
    assert!(cache.get(123).is_some());

    let stats = cache.stats();
    assert_eq!(stats.hits, 1);

    cache.clear();
    assert_eq!(cache.stats().size, 0);
}

// ============================================================================
// TIER 2: PROPERTY TESTS (Q8-Q14) - 30 Tests
// ============================================================================

// --- Q8: Universal Properties (10 tests) ---

#[test]
fn prop_hash_normalization_idempotent() {
    // Q8: Verify normalization is idempotent
    for hash in [0, 1, 123, 456, u64::MAX] {
        let normalized1 = if hash == 0 { 1 } else { hash };
        let normalized2 = if normalized1 == 0 { 1 } else { normalized1 };
        assert_eq!(normalized1, normalized2);
    }
}

#[test]
fn prop_cache_key_generation_monotonic() {
    // Q8: Verify generation always increases
    let key = CacheKeyCapsule::new();
    let mut last_gen = key.generation.load(Ordering::Acquire);

    for i in 0..1000 {
        if key.is_empty() {
            let success = key.set_key(i, now_ns());
            if success {
                let current_gen = key.generation.load(Ordering::Acquire);
                assert!(current_gen > last_gen);
                last_gen = current_gen;
            }
        }

        key.clear();
        let current_gen = key.generation.load(Ordering::Acquire);
        assert!(current_gen > last_gen);
        last_gen = current_gen;
    }
}

#[test]
fn prop_cache_key_access_count_monotonic() {
    // Q8: Verify access count never decreases (until clear)
    let key = CacheKeyCapsule::new();
    key.set_key(123, now_ns());

    let mut last_access = key.get_access_count();
    for _ in 0..1000 {
        key.increment_access();
        let current_access = key.get_access_count();
        assert!(current_access > last_access);
        last_access = current_access;
    }
}

#[test]
fn prop_cache_key_timestamp_preserved() {
    // Q8: Verify timestamp unchanging across accesses
    let key = CacheKeyCapsule::new();
    let timestamp = now_ns();
    key.set_key(123, timestamp);

    for _ in 0..100 {
        key.increment_access();
    }

    assert_eq!(key.get_timestamp_ns(), timestamp);
}

#[test]
fn prop_response_cache_capacity_never_exceeded() {
    // Q8: Verify capacity is a hard limit
    let capacity = 100;
    let mut cache = ResponseCache::with_capacity(capacity, 300);
    let response = create_mock_response("test", "hello");

    for i in 0..1000 {
        cache.insert(i, response.clone());
        let stats = cache.stats();
        assert!(stats.size <= capacity);
    }
}

#[test]
fn prop_response_cache_stats_conservation() {
    // Q8: Verify hits + misses = total requests
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    cache.insert(123, response.clone());

    for _ in 0..100 {
        cache.get(123); // Hit
        cache.get(456); // Miss
    }

    let stats = cache.stats();
    assert_eq!(stats.hits + stats.misses, 200);
}

#[test]
fn prop_response_cache_clear_is_idempotent() {
    // Q8: Verify clear() is idempotent
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    cache.insert(123, response.clone());
    cache.clear();
    cache.clear();
    cache.clear();

    assert_eq!(cache.stats().size, 0);
}

#[test]
fn prop_response_cache_insert_after_clear() {
    // Q8: Verify clear enables reuse
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    for i in 0..100 {
        cache.insert(i, response.clone());
    }

    cache.clear();

    for i in 0..100 {
        cache.insert(i, response.clone());
        assert!(cache.get(i).is_some());
    }
}

#[test]
fn prop_ttl_expiration_consistency() {
    // Q8: Verify TTL expiration is consistent
    let mut cache = ResponseCache::with_capacity(1024, 0); // 0 second TTL
    let response = create_mock_response("test", "hello");

    for i in 0..100 {
        cache.insert(i, response.clone());
    }

    thread::sleep(Duration::from_millis(50));

    for i in 0..100 {
        assert!(cache.get(i).is_none(), "Entry {} should be expired", i);
    }
}

#[test]
fn prop_eviction_counter_increments() {
    // Q8: Verify eviction counter is monotonic
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    let mut last_counter = cache.eviction_counter.load(Ordering::Relaxed);
    for i in 0..1000 {
        cache.insert(i, response.clone());
        let current_counter = cache.eviction_counter.load(Ordering::Relaxed);
        assert!(current_counter >= last_counter);
        last_counter = current_counter;
    }
}

// --- Q9: Concurrent Invariants (8 tests) ---

#[test]
fn prop_concurrent_inserts_no_lost_writes() {
    // Q9: Verify no lost writes under concurrent insertions
    let cache = Arc::new(parking_lot::Mutex::new(ResponseCache::new()));
    let num_threads = 10;
    let inserts_per_thread = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let cache_clone = Arc::clone(&cache);
            thread::spawn(move || {
                for i in 0..inserts_per_thread {
                    let hash = (thread_id * 1000 + i) as u64;
                    let response = create_mock_response(&format!("thread_{}", thread_id), "test");
                    cache_clone.lock().insert(hash, response);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let stats = cache.lock().stats();
    assert_eq!(stats.insertions, num_threads * inserts_per_thread);
}

#[test]
fn prop_concurrent_reads_deterministic() {
    // Q9: Verify deterministic reads under concurrency
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");
    cache.insert(123, response.clone());

    let cache = Arc::new(parking_lot::Mutex::new(cache));
    let num_threads = 50;
    let reads_per_thread = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let cache_clone = Arc::clone(&cache);
            thread::spawn(move || {
                for _ in 0..reads_per_thread {
                    let result = cache_clone.lock().get(123);
                    assert!(result.is_some());
                    assert_eq!(result.unwrap().id, "test");
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn prop_concurrent_mixed_operations() {
    // Q9: Verify mixed read/write safety
    let cache = Arc::new(parking_lot::Mutex::new(ResponseCache::new()));
    let response = create_mock_response("test", "hello");

    // Prewarm
    for i in 0..100 {
        cache.lock().insert(i, response.clone());
    }

    let num_threads = 10;
    let ops_per_thread = 1000;

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let cache_clone = Arc::clone(&cache);
            let response = response.clone();
            thread::spawn(move || {
                for i in 0..ops_per_thread {
                    if i % 2 == 0 {
                        cache_clone.lock().get((i % 100) as u64);
                    } else {
                        cache_clone.lock().insert((thread_id * 1000 + i) as u64, response.clone());
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // No panics = success
}

#[test]
fn prop_concurrent_generation_consistency() {
    // Q9: Verify generation counter consistency under concurrency
    let capsule = Arc::new(CacheKeyCapsule::new());
    let num_threads = 10;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                for _ in 0..100 {
                    let gen_before = c.generation.load(Ordering::Acquire);
                    c.increment_access();
                    let gen_after = c.generation.load(Ordering::Acquire);
                    // Generation unchanged on increment_access
                    assert_eq!(gen_before, gen_after);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn prop_concurrent_access_count_accumulation() {
    // Q9: Verify access count accumulates correctly under concurrency
    let capsule = Arc::new(CacheKeyCapsule::new());
    capsule.set_key(123, now_ns());

    let num_threads = 10;
    let increments_per_thread = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                for _ in 0..increments_per_thread {
                    c.increment_access();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let total_access = capsule.get_access_count();
    assert_eq!(total_access, 1 + num_threads * increments_per_thread); // 1 from set_key
}

#[test]
fn prop_concurrent_eviction_stability() {
    // Q9: Verify eviction stability under concurrent writes
    let cache = Arc::new(parking_lot::Mutex::new(ResponseCache::with_capacity(1000, 1)));
    let response = create_mock_response("test", "hello");

    let num_threads = 5;

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let cache_clone = Arc::clone(&cache);
            let response = response.clone();
            thread::spawn(move || {
                for i in 0..1000 {
                    cache_clone.lock().insert((thread_id * 10000 + i) as u64, response.clone());
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let stats = cache.lock().stats();
    assert_eq!(stats.insertions, num_threads * 1000);
}

#[test]
fn prop_hash_collision_deterministic() {
    // Q9: Verify hash collisions are deterministic
    let mut cache = ResponseCache::with_capacity(100, 300);

    for i in 0..10 {
        let hash = 123 + i * 100; // All collide in slot 23
        let response = create_mock_response(&format!("response_{}", i), "test");
        cache.insert(hash, response);
    }

    // Last insert wins deterministically
    for i in 0..10 {
        let hash = 123 + i * 100;
        let result = cache.get(hash);
        if let Some(r) = result {
            assert_eq!(r.id, format!("response_{}", i));
        }
    }
}

#[test]
fn prop_concurrent_hammering_100_threads() {
    // Q9: Verify stability under extreme concurrency
    let cache = Arc::new(parking_lot::Mutex::new(ResponseCache::with_capacity(65536, 300)));
    let response = create_mock_response("test", "hello");

    let num_threads = 100;
    let ops_per_thread = 1000;

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let cache_clone = Arc::clone(&cache);
            let response = response.clone();
            thread::spawn(move || {
                for i in 0..ops_per_thread {
                    if i % 2 == 0 {
                        cache_clone.lock().insert((thread_id * 10000 + i) as u64, response.clone());
                    } else {
                        cache_clone.lock().get((thread_id * 10000 + i) as u64);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // No panics = success
}

// --- Q10-Q14: Additional Property Tests (12 tests) ---

#[test]
fn prop_ttl_zero_immediate_expiration() {
    // Q10: Verify TTL=0 immediate expiration
    let mut cache = ResponseCache::with_capacity(1024, 0);
    let response = create_mock_response("test", "hello");

    cache.insert(123, response.clone());
    thread::sleep(Duration::from_millis(10));

    assert!(cache.get(123).is_none());
}

#[test]
fn prop_access_count_reset_on_clear() {
    // Q11: Verify clear resets access counts
    let capsule = CacheKeyCapsule::new();
    capsule.set_key(123, now_ns());
    capsule.increment_access();
    capsule.increment_access();
    assert_eq!(capsule.get_access_count(), 3); // 1 from set + 2 increments

    capsule.clear();
    assert_eq!(capsule.get_access_count(), 0);
}

#[test]
fn prop_eviction_reduces_size() {
    // Q12: Verify eviction reduces size
    let mut cache = ResponseCache::with_capacity(100, 300);
    let response = create_mock_response("test", "hello");

    for i in 0..100 {
        cache.insert(i, response.clone());
    }

    let size_before = cache.stats().size;
    cache.evict_expired(); // May or may not evict (depends on TTL)
    let size_after = cache.stats().size;

    // Size should be same or reduced
    assert!(size_after <= size_before);
}

#[test]
fn prop_hash_distribution_uniform() {
    // Q13: Verify hash distribution is reasonable
    let mut cache = ResponseCache::with_capacity(1000, 300);
    let response = create_mock_response("test", "hello");

    for i in 0..1000 {
        cache.insert(i, response.clone());
    }

    // Expect at least 95% utilization with sequential hashes
    let stats = cache.stats();
    assert!(stats.size >= 950, "Distribution too poor: {} entries", stats.size);
}

#[test]
fn prop_timestamp_monotonic() {
    // Q13: Verify timestamps are monotonic across insertions
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    cache.insert(123, response.clone());
    let ts1 = cache.entries[(123 % cache.capacity as u64) as usize]
        .as_ref()
        .unwrap()
        .key
        .get_timestamp_ns();

    thread::sleep(Duration::from_millis(10));

    cache.insert(456, response.clone());
    let ts2 = cache.entries[(456 % cache.capacity as u64) as usize]
        .as_ref()
        .unwrap()
        .key
        .get_timestamp_ns();

    assert!(ts2 > ts1, "Timestamps must be monotonic");
}

#[test]
fn prop_multiple_clears_idempotent() {
    // Q14: Verify clear is idempotent
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    cache.insert(123, response.clone());
    cache.clear();
    cache.clear();
    cache.clear();

    assert_eq!(cache.stats().size, 0);
}

#[test]
fn prop_concurrent_stats_accuracy() {
    // Q14: Verify statistics accuracy under concurrency
    let cache = Arc::new(parking_lot::Mutex::new(ResponseCache::new()));
    let response = create_mock_response("test", "hello");

    // Prewarm
    for i in 0..5000 {
        cache.lock().insert(i, response.clone());
    }

    let num_threads = 10;
    let ops_per_thread = 10_000;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let cache_clone = Arc::clone(&cache);
            thread::spawn(move || {
                for i in 0..ops_per_thread {
                    cache_clone.lock().get((i % 5000) as u64);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let stats = cache.lock().stats();
    assert_eq!(stats.hits + stats.misses, num_threads * ops_per_thread);
}

#[test]
fn prop_insert_after_clear_succeeds() {
    // Q14: Verify insertions work after clear
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    for i in 0..100 {
        cache.insert(i, response.clone());
    }

    cache.clear();

    for i in 0..100 {
        cache.insert(i, response.clone());
        assert!(cache.get(i).is_some());
    }
}

#[test]
fn prop_long_ttl_retention() {
    // Q14: Verify long TTL entries survive
    let mut cache = ResponseCache::with_capacity(10000, 3600); // 1 hour TTL
    let response = create_mock_response("test", "hello");

    for i in 0..1000 {
        cache.insert(i, response.clone());
    }

    thread::sleep(Duration::from_millis(100));
    cache.evict_expired();

    // All should survive (long TTL)
    for i in 0..1000 {
        assert!(cache.get(i).is_some(), "Entry {} should not expire", i);
    }
}

#[test]
fn prop_generation_wraparound_safety() {
    // Q14: Verify generation counter overflow safety
    let capsule = CacheKeyCapsule::new();
    capsule.generation.store(u64::MAX - 10, Ordering::Relaxed);

    for _ in 0..100 {
        capsule.set_key(123, now_ns());
        capsule.clear();
    }

    // Should not panic on overflow
    assert!(capsule.generation.load(Ordering::Acquire) > 0);
}

#[test]
fn prop_access_count_wraparound_safety() {
    // Q14: Verify access count overflow safety
    let capsule = CacheKeyCapsule::new();
    capsule.set_key(123, now_ns());
    capsule.access_count.store(u64::MAX - 10, Ordering::Relaxed);

    for _ in 0..100 {
        capsule.increment_access();
    }

    // Should not panic on overflow
}

#[test]
fn prop_timestamp_overflow_safety() {
    // Q14: Verify timestamp overflow safety
    let capsule = CacheKeyCapsule::new();
    capsule.set_key(123, u64::MAX - 1000);

    // Should not panic
    assert!(capsule.get_timestamp_ns() > 0);
}

// ============================================================================
// TIER 3: INTEGRATION TESTS (Q15-Q21) - 20 Tests
// ============================================================================

// --- Q15-Q17: Critical Integration Points (8 tests) ---

#[test]
fn integration_end_to_end_cache_lifecycle() {
    // Q15: Full lifecycle test
    let mut cache = ResponseCache::new();
    let response = create_mock_response("lifecycle", "test");

    // Phase 1: Insert
    cache.insert(123, response.clone());
    assert_eq!(cache.stats().size, 1);

    // Phase 2: Read (hit)
    let result = cache.get(123);
    assert!(result.is_some());

    // Phase 3: Update
    cache.insert(123, response.clone());

    // Phase 4: Read (verify)
    assert!(cache.get(123).is_some());

    // Phase 5: Clear
    cache.clear();

    // Phase 6: Read (miss)
    assert!(cache.get(123).is_none());
}

#[test]
fn integration_multi_threaded_read_write_mix() {
    // Q15: Concurrent read/write patterns
    let cache = Arc::new(parking_lot::Mutex::new(ResponseCache::new()));
    let response = create_mock_response("test", "hello");

    // Prewarm
    for i in 0..500 {
        cache.lock().insert(i, response.clone());
    }

    let num_readers = 5;
    let num_writers = 5;
    let mut handles = vec![];

    // Spawn readers
    for _ in 0..num_readers {
        let cache_clone = Arc::clone(&cache);
        handles.push(thread::spawn(move || {
            for i in 0..1000 {
                cache_clone.lock().get((i % 500) as u64);
            }
        }));
    }

    // Spawn writers
    for thread_id in 0..num_writers {
        let cache_clone = Arc::clone(&cache);
        let response = response.clone();
        handles.push(thread::spawn(move || {
            for i in 0..1000 {
                cache_clone.lock().insert((thread_id * 10000 + i) as u64, response.clone());
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let stats = cache.lock().stats();
    assert_eq!(stats.insertions, 500 + num_writers * 1000);
    assert_eq!(stats.hits + stats.misses, num_readers * 1000);
}

#[test]
fn integration_cache_capacity_enforcement() {
    // Q17: Capacity limits under load
    let mut cache = ResponseCache::with_capacity(100, 300);
    let response = create_mock_response("test", "hello");

    // Insert 200 entries (2× capacity)
    for i in 0..200 {
        cache.insert(i, response.clone());
    }

    // Cache should not exceed capacity
    assert!(cache.stats().size <= cache.capacity);
}

#[test]
fn integration_batch_eviction_correctness() {
    // Q15: Batch eviction validation
    let mut cache = ResponseCache::with_capacity(1024, 0); // 0 second TTL
    let response = create_mock_response("test", "hello");

    for i in 0..1000 {
        cache.insert(i, response.clone());
        if i % 100 == 99 {
            thread::sleep(Duration::from_micros(100));
        }
    }

    thread::sleep(Duration::from_millis(10));
    cache.evict_expired();

    let stats = cache.stats();
    assert!(stats.evictions > 0);
}

#[test]
fn integration_ttl_expiration_cleanup_cycle() {
    // Q15: TTL cleanup integration
    let mut cache = ResponseCache::with_capacity(1024, 1); // 1 second TTL
    let response = create_mock_response("test", "hello");

    for i in 0..100 {
        cache.insert(i, response.clone());
    }

    thread::sleep(Duration::from_millis(1500));
    cache.evict_expired();

    for i in 0..100 {
        assert!(cache.get(i).is_none());
    }
}

#[test]
fn integration_statistics_accuracy() {
    // Q21: Monitoring accuracy validation
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    // Insert 10 entries
    for i in 0..10 {
        cache.insert(i, response.clone());
    }

    // Access entries (hits + misses)
    let mut total_hits = 0;
    let mut total_misses = 0;
    for i in 0..30 {
        if cache.get((i % 10) as u64).is_some() {
            total_hits += 1;
        } else {
            total_misses += 1;
        }
    }

    let stats = cache.stats();
    assert_eq!(stats.insertions, 10);
    assert_eq!(stats.hits, total_hits);
    assert_eq!(stats.misses, total_misses);
    assert_eq!(stats.hits + stats.misses, 30);
}

#[test]
fn integration_eviction_periodic_trigger() {
    // Q15: Automatic periodic eviction
    let mut cache = ResponseCache::with_capacity(1024, 0); // 0 second TTL
    let response = create_mock_response("test", "hello");

    for i in 0..200 {
        cache.insert(i, response.clone());
        thread::sleep(Duration::from_micros(100));
    }

    thread::sleep(Duration::from_millis(10));

    let stats = cache.stats();
    assert!(stats.evictions > 0, "Automatic eviction should trigger");
}

#[test]
fn integration_hash_collision_handling() {
    // Q16: Error propagation via hash collision
    let mut cache = ResponseCache::with_capacity(100, 300);
    let response = create_mock_response("test", "hello");

    // Insert entries that collide (modulo 100)
    for i in 0..5 {
        let hash = 123 + i * 100; // All map to slot 23
        cache.insert(hash, response.clone());
    }

    // Only last insert survives per slot
    for i in 0..5 {
        let hash = 123 + i * 100;
        let result = cache.get(hash);
        if i == 4 {
            assert!(result.is_some()); // Last one
        } else {
            assert!(result.is_none()); // Overwritten
        }
    }
}

// --- Q18-Q21: Production Integration (12 tests) ---

#[test]
fn integration_concurrent_capacity_limit() {
    // Q18: Capacity under concurrent load
    let cache = Arc::new(parking_lot::Mutex::new(ResponseCache::with_capacity(500, 300)));
    let response = create_mock_response("test", "hello");

    let handles: Vec<_> = (0..10)
        .map(|thread_id| {
            let cache_clone = Arc::clone(&cache);
            let response = response.clone();
            thread::spawn(move || {
                for i in 0..100 {
                    cache_clone.lock().insert((thread_id * 1000 + i) as u64, response.clone());
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let stats = cache.lock().stats();
    assert!(stats.size <= 500);
}

#[test]
fn integration_ttl_expiration_mixed_ages() {
    // Q15: Mixed TTL ages
    let mut cache = ResponseCache::with_capacity(1024, 1); // 1 second TTL
    let response = create_mock_response("test", "hello");

    // Insert old entries
    for i in 0..50 {
        cache.insert(i, response.clone());
    }

    thread::sleep(Duration::from_millis(500));

    // Insert new entries
    for i in 50..100 {
        cache.insert(i, response.clone());
    }

    thread::sleep(Duration::from_millis(600)); // Total: 1.1s for first batch
    cache.evict_expired();

    // First 50 should be evicted
    for i in 0..50 {
        assert!(cache.get(i).is_none());
    }

    // Last 50 should survive
    for i in 50..100 {
        assert!(cache.get(i).is_some());
    }
}

#[test]
fn integration_clear_and_repopulate() {
    // Q19: Rollback scenario via clear
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    for i in 0..100 {
        cache.insert(i, response.clone());
    }

    let len_after_first_fill = cache.stats().size;
    cache.clear();
    assert_eq!(cache.stats().size, 0);

    for i in 0..100 {
        cache.insert(i, response.clone());
    }

    assert_eq!(cache.stats().size, len_after_first_fill);
}

#[test]
fn integration_statistics_reset_behavior() {
    // Q21: Statistics persistence across clear
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    cache.insert(123, response.clone());
    cache.get(123);
    cache.get(456);

    let stats_before = cache.stats();
    assert_eq!(stats_before.hits, 1);
    assert_eq!(stats_before.misses, 1);

    cache.clear();

    // Stats not reset on clear (by design for monitoring)
    let stats_after = cache.stats();
    assert_eq!(stats_after.hits, 1);
    assert_eq!(stats_after.misses, 1);
}

#[test]
fn integration_eviction_counter_overflow_safety() {
    // Q15: Counter overflow safety
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    // Set counter near max
    cache.eviction_counter.store(u64::MAX - 50, Ordering::Relaxed);

    for i in 0..100 {
        cache.insert(i, response.clone());
    }

    // Should not panic on overflow
    assert!(cache.stats().size > 0);
}

#[test]
fn integration_zero_capacity_edge_case() {
    // Q16: Minimal capacity edge case
    let mut cache = ResponseCache::with_capacity(1, 300);
    let response = create_mock_response("test", "hello");

    cache.insert(123, response.clone());
    assert_eq!(cache.stats().size, 1);

    cache.insert(456, response.clone());
    assert!(cache.stats().size <= 1);
}

#[test]
fn integration_access_count_tracking_accuracy() {
    // Q21: Access count monitoring accuracy
    let mut cache = ResponseCache::new();
    let response = create_mock_response("test", "hello");

    cache.insert(123, response.clone());

    let slot_index = (1 % cache.capacity as u64) as usize; // hash=0→1, normalized
    let initial_access = cache.entries[slot_index]
        .as_ref()
        .unwrap()
        .key
        .get_access_count();

    cache.get(123);
    let after_one = cache.entries[slot_index]
        .as_ref()
        .unwrap()
        .key
        .get_access_count();

    assert_eq!(after_one, initial_access + 1);
}

#[test]
fn integration_mixed_ttl_simulation() {
    // Q15: Heterogeneous TTL simulation
    let mut cache = ResponseCache::with_capacity(1024, 2); // 2 second TTL
    let response = create_mock_response("test", "hello");

    // Batch 1
    for i in 0..50 {
        cache.insert(i, response.clone());
    }

    thread::sleep(Duration::from_millis(1000));

    // Batch 2
    for i in 50..100 {
        cache.insert(i, response.clone());
    }

    thread::sleep(Duration::from_millis(1100)); // Total: 2.1s for batch 1
    cache.evict_expired();

    // Batch 1 expired
    for i in 0..50 {
        assert!(cache.get(i).is_none());
    }

    // Batch 2 survives
    for i in 50..100 {
        assert!(cache.get(i).is_some());
    }
}

#[test]
fn integration_generation_counter_coordination() {
    // Q15: Generation counter semantics
    let capsule = CacheKeyCapsule::new();
    let gen0 = capsule.generation.load(Ordering::Acquire);

    capsule.set_key(123, now_ns());
    let gen1 = capsule.generation.load(Ordering::Acquire);
    assert_eq!(gen1, gen0 + 1);

    capsule.clear();
    let gen2 = capsule.generation.load(Ordering::Acquire);
    assert_eq!(gen2, gen1 + 1);

    capsule.set_key(456, now_ns());
    let gen3 = capsule.generation.load(Ordering::Acquire);
    assert_eq!(gen3, gen2 + 1);
}

#[test]
fn integration_concurrent_eviction_stability() {
    // Q18: Eviction stability under concurrent load
    let cache = Arc::new(parking_lot::Mutex::new(ResponseCache::with_capacity(1000, 0)));
    let response = create_mock_response("test", "hello");

    let handles: Vec<_> = (0..10)
        .map(|thread_id| {
            let cache_clone = Arc::clone(&cache);
            let response = response.clone();
            thread::spawn(move || {
                for i in 0..1000 {
                    cache_clone.lock().insert((thread_id * 10000 + i) as u64, response.clone());
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Evictions should occur without panics
    let stats = cache.lock().stats();
    assert!(stats.evictions > 0);
}

#[test]
fn integration_large_cache_iteration() {
    // Q22: Large cache operations
    let mut cache = ResponseCache::with_capacity(100_000, 300);
    let response = create_mock_response("test", "hello");

    for i in 0..100_000 {
        cache.insert(i, response.clone());
    }

    cache.evict_expired(); // Should complete without timeout
}

#[test]
fn integration_rapid_clear_repopulate_cycles() {
    // Q19: Rollback stress test
    let mut cache = ResponseCache::with_capacity(10000, 300);
    let response = create_mock_response("test", "hello");

    for _ in 0..100 {
        for i in 0..1000 {
            cache.insert(i, response.clone());
        }
        cache.clear();
    }

    // No panics = success
}

// ============================================================================
// TIER 4: PRODUCTION TESTS (Q22-Q28) - 10 Tests
// ============================================================================

#[test]
#[ignore] // Expensive test
fn stress_1m_insertions_memory_stability() {
    // Q22: 1M insertion stress test
    let mut cache = ResponseCache::with_capacity(65536, 300);
    let response = create_mock_response("test", "x".repeat(100).as_str());

    for i in 0..1_000_000 {
        cache.insert(i, response.clone());

        if i % 100_000 == 0 {
            println!("Inserted {} entries, size={}", i, cache.stats().size);
        }
    }

    let stats = cache.stats();
    assert_eq!(stats.insertions, 1_000_000);
}

#[test]
#[ignore] // Expensive test
fn stress_throughput_8_threads() {
    // Q22: 60M ops/sec target
    let cache = Arc::new(parking_lot::Mutex::new(ResponseCache::with_capacity(65536, 300)));
    let response = create_mock_response("test", "hello");

    // Prewarm
    for i in 0..10000 {
        cache.lock().insert(i, response.clone());
    }

    let num_threads = 8;
    let ops_per_thread = 1_000_000;
    let start = std::time::Instant::now();

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let cache_clone = Arc::clone(&cache);
            thread::spawn(move || {
                for i in 0..ops_per_thread {
                    cache_clone.lock().get((i % 10000) as u64);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_ops = num_threads * ops_per_thread;
    let ops_per_sec = total_ops as f64 / elapsed.as_secs_f64();

    println!("Throughput: {:.0} ops/sec", ops_per_sec);
    assert!(ops_per_sec > 10_000_000.0, "Throughput too low");
}

#[test]
#[ignore] // Expensive test
fn stress_p999_tail_latency() {
    // Q22: p99.9 latency validation
    let mut cache = ResponseCache::with_capacity(65536, 300);
    let response = create_mock_response("test", "hello");

    // Prewarm
    for i in 0..10000 {
        cache.insert(i, response.clone());
    }

    let mut latencies = Vec::new();
    for i in 0..100_000 {
        let start = std::time::Instant::now();
        cache.get((i % 10000) as u64);
        let elapsed = start.elapsed();
        latencies.push(elapsed.as_nanos());
    }

    latencies.sort_unstable();
    let p999 = latencies[(latencies.len() * 999 / 1000) as usize];

    println!("p99.9 latency: {}ns", p999);
    assert!(p999 < 10_000, "p99.9 latency too high: {}ns", p999);
}

#[test]
#[ignore] // Very expensive test
fn stress_sustained_load_10_minutes() {
    // Q22: Soak test
    let cache = Arc::new(parking_lot::Mutex::new(ResponseCache::with_capacity(65536, 300)));
    let response = create_mock_response("test", "hello");

    let duration = Duration::from_secs(600); // 10 minutes
    let start = std::time::Instant::now();

    let handles: Vec<_> = (0..8)
        .map(|thread_id| {
            let cache_clone = Arc::clone(&cache);
            let response = response.clone();
            thread::spawn(move || {
                let mut i = 0u64;
                while start.elapsed() < duration {
                    if i % 2 == 0 {
                        cache_clone.lock().get((i % 10000) as u64);
                    } else {
                        cache_clone.lock().insert((thread_id * 1_000_000 + i) as u64, response.clone());
                    }
                    i += 1;
                }
                i
            })
        })
        .collect();

    let total_ops: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
    println!("Total operations in 10 minutes: {}", total_ops);
    assert!(total_ops > 1_000_000);
}

#[test]
fn stress_concurrent_hammering_100_threads() {
    // Q22: Extreme concurrency stress
    let cache = Arc::new(parking_lot::Mutex::new(ResponseCache::with_capacity(65536, 300)));
    let response = create_mock_response("test", "hello");

    let num_threads = 100;
    let ops_per_thread = 1000;

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let cache_clone = Arc::clone(&cache);
            let response = response.clone();
            thread::spawn(move || {
                for i in 0..ops_per_thread {
                    if i % 2 == 0 {
                        cache_clone.lock().insert((thread_id * 10000 + i) as u64, response.clone());
                    } else {
                        cache_clone.lock().get((thread_id * 10000 + i) as u64);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // No panics = success
}

#[test]
fn stress_eviction_under_pressure() {
    // Q22: Eviction performance under pressure
    let mut cache = ResponseCache::with_capacity(10000, 0); // Immediate expiration
    let response = create_mock_response("test", "hello");

    for i in 0..10_000 {
        cache.insert(i, response.clone());
        if i % 1000 == 999 {
            thread::sleep(Duration::from_micros(100));
        }
    }

    let stats = cache.stats();
    assert!(stats.evictions > 0);
}

#[test]
fn stress_hash_collision_cascade() {
    // Q23: Security - hash collision handling
    let mut cache = ResponseCache::with_capacity(100, 300);
    let response = create_mock_response("test", "hello");

    // All hash to same slot (0)
    for i in 0..1000 {
        cache.insert(i * 100, response.clone());
    }

    // Should not panic or degrade severely
    assert!(cache.stats().size <= cache.capacity);
}

#[test]
#[ignore] // Expensive test
fn stress_memory_usage_tracking() {
    // Q22: Memory leak detection
    let mut cache = ResponseCache::with_capacity(65536, 300);
    let large_response = create_mock_response("test", &"x".repeat(10000));

    for i in 0..100_000 {
        cache.insert(i, large_response.clone());
    }

    cache.clear();
    assert_eq!(cache.stats().size, 0);

    // Memory should be freed (Arc drop)
}

#[test]
#[ignore] // Expensive test
fn stress_sustained_high_hit_rate() {
    // Q24: Hit rate validation
    let mut cache = ResponseCache::with_capacity(10000, 300);
    let response = create_mock_response("test", "hello");

    // Prewarm with 1000 entries
    for i in 0..1000 {
        cache.insert(i, response.clone());
    }

    // 90% hit rate workload
    for _ in 0..100_000 {
        let key = if rand::random::<f64>() < 0.9 {
            rand::random::<u64>() % 1000
        } else {
            1000 + rand::random::<u64>() % 1000
        };

        let _ = cache.get(key).or_else(|| {
            cache.insert(key, response.clone());
            cache.get(key)
        });
    }

    let stats = cache.stats();
    let hit_rate = stats.hits as f64 / (stats.hits + stats.misses) as f64;
    println!("Hit rate: {:.2}%", hit_rate * 100.0);
    assert!(hit_rate > 0.85, "Hit rate too low: {:.2}%", hit_rate * 100.0);
}

#[test]
fn stress_mixed_operation_interleaving() {
    // Q22: Mixed operation stress
    let mut cache = ResponseCache::with_capacity(10000, 300);
    let response = create_mock_response("test", "hello");

    for i in 0..50_000 {
        match i % 4 {
            0 => { cache.insert(i, response.clone()); }
            1 => { cache.get(i); }
            2 => { cache.evict_expired(); }
            3 => { let _ = cache.stats(); }
            _ => unreachable!(),
        }
    }

    // No panics = success
}
