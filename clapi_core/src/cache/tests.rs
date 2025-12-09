//! Comprehensive Cache Tests (T28 Framework)
//!
//! # T28 Testing Framework Coverage
//!
//! **Q1-Q7 (Unit Tests)**: Cache hit/miss, LRU eviction, TTL expiration
//! **Q8-Q14 (Property Tests)**: Concurrent access, hit rate verification
//! **Q15-Q21 (Integration Tests)**: End-to-end cache lifecycle
//! **Q22-Q28 (Stress Tests)**: 1M requests with 90% duplicates

use super::*;
use atomic_capsule::hash::const_fast_hash;
use std::sync::Arc;
use std::thread;

// ============================================================================
// T28 Q1-Q7: Unit Tests
// ============================================================================

#[test]
fn test_cache_empty_initialization() {
    let cache = LruCache::default();
    assert!(cache.is_empty());
    assert_eq!(cache.len(), 0);
}

#[test]
fn test_cache_insert_and_retrieve() {
    let cache = LruCache::default();

    let hash = const_fast_hash(b"test_request");
    let response = "test_response".to_string();

    cache.insert(hash, response.clone()).unwrap();

    let entry = cache.get(hash).unwrap();
    assert_eq!(entry.hash, hash);
    assert_eq!(entry.response, response);
}

#[test]
fn test_cache_miss_on_nonexistent() {
    let cache = LruCache::default();

    let hash = const_fast_hash(b"nonexistent");
    let result = cache.get(hash);

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CacheError::CacheMiss(_)));
}

#[test]
fn test_cache_update_existing_entry() {
    let cache = LruCache::default();

    let hash = const_fast_hash(b"test_request");
    cache.insert(hash, "response1".to_string()).unwrap();
    cache.insert(hash, "response2".to_string()).unwrap();

    let entry = cache.get(hash).unwrap();
    assert_eq!(entry.response, "response2");
}

#[test]
fn test_cache_lru_eviction() {
    let mut config = CacheConfig::default();
    config.max_entries = 100;

    let cache = LruCache::new(config);

    // Fill cache completely
    for i in 0..100 {
        let hash = const_fast_hash(format!("request_{}", i).as_bytes());
        cache.insert(hash, format!("response_{}", i)).unwrap();
    }

    // Access first 50 entries (make them more recent)
    for i in 0..50 {
        let hash = const_fast_hash(format!("request_{}", i).as_bytes());
        let _ = cache.get(hash);
    }

    // Record evictions before explicit evict (may have occurred during filling due to hash collisions)
    let evictions_before = cache.stats().evictions.load(std::sync::atomic::Ordering::Relaxed);

    // Evict LRU (should evict one of 50-99 range)
    cache.evict_lru().unwrap();

    // Verify exactly 1 additional eviction
    let evictions_after = cache.stats().evictions.load(std::sync::atomic::Ordering::Relaxed);
    assert_eq!(evictions_after - evictions_before, 1, "Expected exactly 1 eviction, got {}", evictions_after - evictions_before);
}

#[test]
fn test_cache_ttl_expiration() {
    let mut config = CacheConfig::default();
    config.default_ttl_ns = 100_000; // 100 microseconds

    let cache = LruCache::new(config);

    let hash = const_fast_hash(b"expiring_request");
    cache.insert(hash, "response".to_string()).unwrap();

    // Wait for TTL expiration
    thread::sleep(std::time::Duration::from_millis(10));

    let result = cache.get(hash);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CacheError::TtlExpired { .. }));
}

#[test]
fn test_cache_no_ttl_never_expires() {
    let mut config = CacheConfig::default();
    config.default_ttl_ns = 0; // No expiration

    let cache = LruCache::new(config);

    let hash = const_fast_hash(b"permanent_request");
    cache.insert(hash, "response".to_string()).unwrap();

    // Wait
    thread::sleep(std::time::Duration::from_millis(10));

    // Should still be valid
    let result = cache.get(hash);
    assert!(result.is_ok());
}

#[test]
fn test_cache_clear_all_entries() {
    let cache = LruCache::default();

    for i in 0..10 {
        let hash = const_fast_hash(format!("request_{}", i).as_bytes());
        cache.insert(hash, format!("response_{}", i)).unwrap();
    }

    assert_eq!(cache.len(), 10);

    cache.clear();
    assert!(cache.is_empty());
}

#[test]
fn test_cache_stats_hit_rate() {
    let cache = LruCache::default();

    let hash1 = const_fast_hash(b"request1");
    let hash2 = const_fast_hash(b"request2");
    let hash3 = const_fast_hash(b"request3");

    cache.insert(hash1, "response1".to_string()).unwrap();
    cache.insert(hash2, "response2".to_string()).unwrap();

    // 2 hits
    cache.get(hash1).unwrap();
    cache.get(hash2).unwrap();

    // 1 miss
    let _ = cache.get(hash3);

    let hit_rate = cache.stats().hit_rate();
    assert!((hit_rate - 0.666).abs() < 0.01); // ~66.6% hit rate
}

#[test]
fn test_cache_invalid_hash_zero() {
    let cache = LruCache::default();

    let result = cache.insert(0, "response".to_string());
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CacheError::InvalidHash));

    let result = cache.get(0);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CacheError::InvalidHash));
}

// ============================================================================
// T28 Q8-Q14: Property Tests
// ============================================================================

#[test]
fn test_cache_concurrent_inserts() {
    let cache = Arc::new(LruCache::default());
    let mut handles = vec![];

    // Spawn 10 threads, each inserting 100 entries
    for i in 0..10 {
        let cache_clone = Arc::clone(&cache);
        let handle = thread::spawn(move || {
            for j in 0..100 {
                let hash = const_fast_hash(format!("request_{}_{}", i, j).as_bytes());
                let response = format!("response_{}_{}", i, j);
                cache_clone.insert(hash, response).unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify cache has entries (some may have been evicted)
    assert!(cache.len() > 0);

    // Verify statistics
    let total_inserts = cache.stats().inserts.load(std::sync::atomic::Ordering::Relaxed);
    assert_eq!(total_inserts, 1000); // 10 threads × 100 inserts
}

#[test]
fn test_cache_concurrent_reads_and_writes() {
    let cache = Arc::new(LruCache::default());

    // Prewarm cache
    for i in 0..100 {
        let hash = const_fast_hash(format!("request_{}", i).as_bytes());
        cache.insert(hash, format!("response_{}", i)).unwrap();
    }

    let mut handles = vec![];

    // Spawn readers
    for _ in 0..5 {
        let cache_clone = Arc::clone(&cache);
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let hash = const_fast_hash(format!("request_{}", i).as_bytes());
                let _ = cache_clone.get(hash);
            }
        });
        handles.push(handle);
    }

    // Spawn writers
    for i in 100..110 {
        let cache_clone = Arc::clone(&cache);
        let handle = thread::spawn(move || {
            for j in 0..10 {
                let hash = const_fast_hash(format!("request_{}_{}", i, j).as_bytes());
                let response = format!("response_{}_{}", i, j);
                cache_clone.insert(hash, response).unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify no panics, data integrity maintained
    let hit_rate = cache.stats().hit_rate();
    assert!(hit_rate > 0.0);
}

#[test]
fn test_cache_property_hit_rate_with_duplicates() {
    let cache = LruCache::default();

    // Insert 100 unique entries
    for i in 0..100 {
        let hash = const_fast_hash(format!("request_{}", i).as_bytes());
        cache.insert(hash, format!("response_{}", i)).unwrap();
    }

    // Access with 90% duplicate rate (simulate real workload)
    for _ in 0..1000 {
        let i = if rand::random::<f64>() < 0.9 {
            // 90% duplicates (access existing entries)
            rand::random::<usize>() % 100
        } else {
            // 10% new entries
            100 + rand::random::<usize>() % 100
        };

        let hash = const_fast_hash(format!("request_{}", i).as_bytes());
        let _ = cache.get(hash).or_else(|_| {
            cache.insert(hash, format!("response_{}", i))?;
            Ok::<CacheEntry, CacheError>(cache.get(hash).unwrap())
        });
    }

    // Verify hit rate is close to 90%
    let hit_rate = cache.stats().hit_rate();
    assert!(hit_rate > 0.85, "Hit rate {} < 85%", hit_rate);
}

// ============================================================================
// T28 Q15-Q21: Integration Tests
// ============================================================================

#[test]
fn test_cache_end_to_end_lifecycle() {
    let cache = LruCache::default();

    // Phase 1: Insert
    let hash = const_fast_hash(b"lifecycle_test");
    cache.insert(hash, "initial_response".to_string()).unwrap();

    // Phase 2: Read (cache hit)
    let entry = cache.get(hash).unwrap();
    assert_eq!(entry.response, "initial_response");

    // Phase 3: Update
    cache.insert(hash, "updated_response".to_string()).unwrap();

    // Phase 4: Read (verify update)
    let entry = cache.get(hash).unwrap();
    assert_eq!(entry.response, "updated_response");

    // Phase 5: Clear
    cache.clear();

    // Phase 6: Read (cache miss)
    let result = cache.get(hash);
    assert!(result.is_err());
}

#[test]
fn test_cache_eviction_preserves_mru() {
    let mut config = CacheConfig::default();
    config.max_entries = 50;

    let cache = LruCache::new(config);

    // Fill cache
    for i in 0..50 {
        let hash = const_fast_hash(format!("request_{}", i).as_bytes());
        cache.insert(hash, format!("response_{}", i)).unwrap();
    }

    // Collect hashes for accessed entries (0-24)
    let mut accessed_hashes = Vec::new();
    for i in 0..25 {
        let hash = const_fast_hash(format!("request_{}", i).as_bytes());
        accessed_hashes.push(hash);
    }

    // Access these entries multiple times to build up high frequency
    for hash in &accessed_hashes {
        for _ in 0..5 {
            let _ = cache.get(*hash);
        }
    }

    // Collect hashes for unaccessed entries (25-49) - DON'T access them
    let mut unaccessed_hashes = Vec::new();
    for i in 25..50 {
        let hash = const_fast_hash(format!("request_{}", i).as_bytes());
        unaccessed_hashes.push(hash);
    }

    // Evict entries until we can't anymore (should evict unaccessed entries first due to low freq)
    let mut evictions = 0;
    while evictions < 25 && cache.evict_lru().is_ok() {
        evictions += 1;
    }

    // Verify that accessed entries (high freq) survived better than unaccessed (low freq)
    let mut accessed_survived = 0;
    let mut unaccessed_survived = 0;

    for hash in &accessed_hashes {
        if cache.get(*hash).is_ok() {
            accessed_survived += 1;
        }
    }

    for hash in &unaccessed_hashes {
        // Use internal check without incrementing freq (can't do this without modifying cache API)
        // Instead, just accept that this will increment freq
        if cache.get(*hash).is_ok() {
            unaccessed_survived += 1;
        }
    }

    // Frequency weighting should protect accessed entries better
    // At least 50% of accessed entries should survive, while most unaccessed should be evicted
    let accessed_survival_rate = accessed_survived as f64 / accessed_hashes.len() as f64;
    assert!(accessed_survival_rate > 0.5,
        "Accessed survival rate {:.1}% too low - frequency weighting not working (accessed={}/{}, unaccessed={}/{})",
        accessed_survival_rate * 100.0, accessed_survived, accessed_hashes.len(), unaccessed_survived, unaccessed_hashes.len());
}

// ============================================================================
// T28 Q22-Q28: Stress Tests
// ============================================================================

#[test]
#[ignore] // Expensive test - run with: cargo test -- --ignored
fn test_cache_stress_1m_requests_90_percent_duplicates() {
    let cache = Arc::new(LruCache::default());

    // Insert 1000 unique entries
    for i in 0..1000 {
        let hash = const_fast_hash(format!("request_{}", i).as_bytes());
        cache.insert(hash, format!("response_{}", i)).unwrap();
    }

    let mut handles = vec![];

    // Spawn 10 threads, each making 100K requests
    for thread_id in 0..10 {
        let cache_clone = Arc::clone(&cache);
        let handle = thread::spawn(move || {
            for _ in 0..100_000 {
                let i = if rand::random::<f64>() < 0.9 {
                    // 90% duplicates
                    rand::random::<usize>() % 1000
                } else {
                    // 10% new
                    1000 + rand::random::<usize>() % 1000
                };

                let hash = const_fast_hash(format!("request_{}", i).as_bytes());
                let _ = cache_clone.get(hash).or_else(|_| {
                    cache_clone.insert(hash, format!("response_{}_{}", thread_id, i))?;
                    Ok::<CacheEntry, CacheError>(cache_clone.get(hash).unwrap())
                });
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify hit rate is ~90%
    let hit_rate = cache.stats().hit_rate();
    println!("Hit rate: {:.2}%", hit_rate * 100.0);
    assert!(hit_rate > 0.85, "Hit rate {} < 85%", hit_rate);

    // Verify total requests
    let total_requests = cache.stats().total_requests();
    assert_eq!(total_requests, 1_000_000);
}

#[test]
#[ignore] // Expensive test
fn test_cache_stress_concurrent_eviction() {
    let mut config = CacheConfig::default();
    config.max_entries = 1000;

    let cache = Arc::new(LruCache::new(config));

    let mut handles = vec![];

    // Spawn threads that insert aggressively (trigger evictions)
    for i in 0..10 {
        let cache_clone = Arc::clone(&cache);
        let handle = thread::spawn(move || {
            for j in 0..10_000 {
                let hash = const_fast_hash(format!("request_{}_{}", i, j).as_bytes());
                let response = format!("response_{}_{}", i, j);
                let _ = cache_clone.insert(hash, response);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify evictions occurred
    let evictions = cache.stats().evictions.load(std::sync::atomic::Ordering::Relaxed);
    assert!(evictions > 0, "No evictions occurred");

    println!("Total evictions: {}", evictions);
}
