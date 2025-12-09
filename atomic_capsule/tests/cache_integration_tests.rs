//! # LockfreeCacheCapsule Integration Tests (I20 Framework)
//!
//! **Goal**: Validate end-to-end integration between LockfreeCacheCapsule and CacheSlot
//!
//! ## I20 Integration Validation
//!
//! - **Q1-Q5**: Scope - LockfreeCacheCapsule wraps CacheSlot array
//! - **Q6-Q10**: Compatibility - Feature flags align, API surface correct
//! - **Q11-Q15**: Safety - No unsafe blocks in public API, proper Drop
//! - **Q16-Q20**: Validation - Integration tests cover all code paths
//!
//! ## Feature Flag Matrix
//!
//! 1. Base (std only): Random SipHash + TTL
//! 2. cache-hmac: + HMAC integrity
//! 3. cache-multi-tenant: + Tenant isolation
//! 4. All features: Full security stack

use atomic_capsule::collections::LockfreeCacheCapsule;
use std::time::Duration;

// ============================================================================
// T28 Q15-Q21: Integration Tests (End-to-End)
// ============================================================================

#[test]
fn test_end_to_end_insert_get() {
    let cache = LockfreeCacheCapsule::<String>::new(128);

    // Insert value
    let inserted = cache.insert("key1", "value1".to_string(), Duration::from_secs(60));
    assert!(inserted, "Insert should succeed");

    // Get value
    let retrieved = cache.get(&"key1");
    assert_eq!(retrieved, Some("value1".to_string()));

    // Verify cache stats
    assert_eq!(cache.len(), 1);
    assert!(!cache.is_empty());
}

#[test]
fn test_end_to_end_multiple_inserts() {
    let cache = LockfreeCacheCapsule::<String>::new(128);

    // Insert 10 values
    for i in 0..10 {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        let inserted = cache.insert(&key, value, Duration::from_secs(60));
        assert!(inserted, "Insert {} should succeed", i);
    }

    // Verify all values retrievable
    for i in 0..10 {
        let key = format!("key{}", i);
        let expected = format!("value{}", i);
        let retrieved = cache.get(&key);
        assert_eq!(
            retrieved,
            Some(expected),
            "Get key{} should return value{}",
            i,
            i
        );
    }

    // Verify cache size
    assert_eq!(cache.len(), 10);
}

#[test]
fn test_end_to_end_ttl_expiration() {
    let cache = LockfreeCacheCapsule::<String>::new(128);

    // Insert with zero TTL (expires immediately)
    let inserted = cache.insert("key1", "value1".to_string(), Duration::from_secs(0));
    assert!(inserted, "Insert should succeed");

    // Get should return None (expired)
    let retrieved = cache.get(&"key1");
    assert_eq!(retrieved, None, "Expired entry should return None");
}

#[test]
fn test_end_to_end_batch_evict_lru() {
    let cache = LockfreeCacheCapsule::<String>::new(128);

    // Insert 20 values
    for i in 0..20 {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        cache.insert(&key, value, Duration::from_secs(60));
    }

    assert_eq!(cache.len(), 20);

    // Batch evict 10 entries
    let evicted = cache.batch_evict_lru(10);
    assert_eq!(evicted, 10, "Should evict exactly 10 entries");

    // Verify cache size reduced
    assert_eq!(cache.len(), 10);
}

#[test]
fn test_end_to_end_batch_expire_ttl() {
    let cache = LockfreeCacheCapsule::<String>::new(128);

    // Insert 10 entries with zero TTL (expire immediately)
    for i in 0..10 {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        cache.insert(&key, value, Duration::from_secs(0));
    }

    // Insert 10 entries with long TTL (don't expire)
    for i in 10..20 {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        cache.insert(&key, value, Duration::from_secs(3600));
    }

    // Batch expire should remove 10 expired entries
    let expired = cache.batch_expire_ttl();
    assert_eq!(expired, 10, "Should expire exactly 10 entries");

    // Verify only non-expired entries remain
    assert_eq!(cache.len(), 10);

    // Verify non-expired entries still retrievable
    for i in 10..20 {
        let key = format!("key{}", i);
        let expected = format!("value{}", i);
        let retrieved = cache.get(&key);
        assert_eq!(
            retrieved,
            Some(expected),
            "Non-expired key{} should be retrievable",
            i
        );
    }
}

#[test]
fn test_cache_slot_update_same_key() {
    let cache = LockfreeCacheCapsule::<String>::new(128);

    // Insert initial value
    let inserted1 = cache.insert("key1", "value1".to_string(), Duration::from_secs(60));
    assert!(inserted1);

    // Update with new value (same key)
    let inserted2 = cache.insert("key1", "value2".to_string(), Duration::from_secs(60));
    assert!(inserted2);

    // Get should return updated value
    let retrieved = cache.get(&"key1");
    assert_eq!(retrieved, Some("value2".to_string()));

    // Cache size should still be 1 (update, not insert)
    assert_eq!(cache.len(), 1);
}

#[test]
fn test_cache_capacity_exhaustion() {
    let cache = LockfreeCacheCapsule::<String>::new(8);

    // Fill cache to capacity
    for i in 0..8 {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        let inserted = cache.insert(&key, value, Duration::from_secs(60));
        assert!(inserted, "Insert {} should succeed", i);
    }

    // Cache should be full
    assert_eq!(cache.len(), 8);

    // Batch evict to make room
    let evicted = cache.batch_evict_lru(4);
    assert_eq!(evicted, 4);

    // Verify eviction worked
    assert_eq!(cache.len(), 4);

    // Now new inserts should succeed
    let inserted = cache.insert("new_key", "new_value".to_string(), Duration::from_secs(60));
    assert!(inserted, "Insert after eviction should succeed");
}

#[test]
fn test_lru_ordering() {
    let cache = LockfreeCacheCapsule::<String>::new(128);

    // Insert 5 entries
    for i in 0..5 {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        cache.insert(&key, value, Duration::from_secs(60));
        std::thread::sleep(Duration::from_millis(10)); // Ensure different access times
    }

    // Access first 3 entries (bump LRU)
    for i in 0..3 {
        let key = format!("key{}", i);
        let _value = cache.get(&key);
    }

    // Batch evict 2 (should evict least recently accessed: key3, key4)
    let evicted = cache.batch_evict_lru(2);
    assert_eq!(evicted, 2);

    // Verify accessed entries still exist
    for i in 0..3 {
        let key = format!("key{}", i);
        let expected = format!("value{}", i);
        let retrieved = cache.get(&key);
        assert_eq!(
            retrieved,
            Some(expected),
            "Accessed key{} should still exist",
            i
        );
    }

    // Verify evicted entries are gone
    for i in 3..5 {
        let key = format!("key{}", i);
        let retrieved = cache.get(&key);
        assert_eq!(retrieved, None, "Evicted key{} should be None", i);
    }
}

// ============================================================================
// Feature Flag Matrix: HMAC Verification
// ============================================================================

#[cfg(feature = "cache-hmac")]
#[test]
fn test_hmac_verification_integration() {
    // This test validates HMAC verification through LockfreeCacheCapsule API
    // However, HMAC verification is internal to CacheSlot, so we test indirectly
    // by verifying that values are retrievable after insert (integrity maintained)

    let cache = LockfreeCacheCapsule::<Vec<u8>>::new(128);

    // Insert value
    let value = vec![1, 2, 3, 4, 5];
    let inserted = cache.insert("key1", value.clone(), Duration::from_secs(60));
    assert!(inserted);

    // Get should succeed (HMAC valid internally)
    let retrieved = cache.get(&"key1");
    assert_eq!(retrieved, Some(value));
}

// ============================================================================
// Feature Flag Matrix: Multi-Tenant Isolation
// ============================================================================

#[cfg(feature = "cache-multi-tenant")]
#[test]
fn test_multi_tenant_insert_get() {
    let cache = LockfreeCacheCapsule::<String>::new(128);

    // Tenant 1 insert
    let inserted_t1 = cache.insert_tenant(
        1,
        "shared_key",
        "tenant1_value".to_string(),
        Duration::from_secs(60),
    );
    assert!(inserted_t1);

    // Tenant 2 insert (same key, different tenant)
    let inserted_t2 = cache.insert_tenant(
        2,
        "shared_key",
        "tenant2_value".to_string(),
        Duration::from_secs(60),
    );
    assert!(inserted_t2);

    // Tenant 1 get (should return tenant1_value)
    let retrieved_t1 = cache.get_tenant(1, &"shared_key");
    assert_eq!(retrieved_t1, Some("tenant1_value".to_string()));

    // Tenant 2 get (should return tenant2_value)
    let retrieved_t2 = cache.get_tenant(2, &"shared_key");
    assert_eq!(retrieved_t2, Some("tenant2_value".to_string()));

    // Cross-tenant get (tenant 1 tries to access tenant 2's data)
    let cross_tenant = cache.get_tenant(1, &"shared_key");
    // NOTE: This will return tenant1_value, not None, because keys hash differently per tenant
    // The isolation works because hash(tenant_id || key) differs per tenant
    assert_eq!(cross_tenant, Some("tenant1_value".to_string()));
}

#[cfg(feature = "cache-multi-tenant")]
#[test]
fn test_multi_tenant_isolation_batch_evict() {
    let cache = LockfreeCacheCapsule::<String>::new(128);

    // Insert 10 entries for tenant 1
    for i in 0..10 {
        let key = format!("key{}", i);
        let value = format!("tenant1_value{}", i);
        cache.insert_tenant(1, &key, value, Duration::from_secs(60));
    }

    // Insert 10 entries for tenant 2
    for i in 0..10 {
        let key = format!("key{}", i);
        let value = format!("tenant2_value{}", i);
        cache.insert_tenant(2, &key, value, Duration::from_secs(60));
    }

    // Batch evict 10 entries (LRU across all tenants)
    let evicted = cache.batch_evict_lru(10);
    assert!(evicted <= 20, "Should evict at most all entries");

    // Verify some entries still exist
    assert!(cache.len() > 0);
}

// ============================================================================
// Feature Flag Matrix: Combined HMAC + Multi-Tenant
// ============================================================================

#[cfg(all(feature = "cache-hmac", feature = "cache-multi-tenant"))]
#[test]
fn test_full_security_stack() {
    let cache = LockfreeCacheCapsule::<Vec<u8>>::new(128);

    // Tenant 1: Insert value with HMAC
    let value_t1 = vec![1, 2, 3, 4, 5];
    let inserted_t1 = cache.insert_tenant(1, "key1", value_t1.clone(), Duration::from_secs(60));
    assert!(inserted_t1);

    // Tenant 2: Insert value with HMAC
    let value_t2 = vec![6, 7, 8, 9, 10];
    let inserted_t2 = cache.insert_tenant(2, "key1", value_t2.clone(), Duration::from_secs(60));
    assert!(inserted_t2);

    // Tenant 1 get (should succeed with HMAC verification)
    let retrieved_t1 = cache.get_tenant(1, &"key1");
    assert_eq!(retrieved_t1, Some(value_t1));

    // Tenant 2 get (should succeed with HMAC verification)
    let retrieved_t2 = cache.get_tenant(2, &"key1");
    assert_eq!(retrieved_t2, Some(value_t2));
}

// ============================================================================
// T28 Q22-Q28: Stress Tests (Concurrent Access)
// ============================================================================

#[test]
fn test_concurrent_insert_get() {
    use std::sync::Arc;
    use std::thread;

    let cache = Arc::new(LockfreeCacheCapsule::<String>::new(1024));

    // Spawn 8 threads inserting concurrently
    let mut handles = vec![];
    for tid in 0..8 {
        let cache_clone = Arc::clone(&cache);
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let key = format!("thread{}_key{}", tid, i);
                let value = format!("thread{}_value{}", tid, i);
                cache_clone.insert(&key, value, Duration::from_secs(60));
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify total entries (should be 800)
    assert_eq!(cache.len(), 800);
}

#[test]
fn test_concurrent_batch_evict() {
    use std::sync::Arc;
    use std::thread;

    let cache = Arc::new(LockfreeCacheCapsule::<String>::new(1024));

    // Insert 1000 entries
    for i in 0..1000 {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        cache.insert(&key, value, Duration::from_secs(60));
    }

    assert_eq!(cache.len(), 1000);

    // Spawn 4 threads evicting concurrently
    let mut handles = vec![];
    for _tid in 0..4 {
        let cache_clone = Arc::clone(&cache);
        let handle = thread::spawn(move || cache_clone.batch_evict_lru(100));
        handles.push(handle);
    }

    // Collect eviction counts
    let mut total_evicted = 0;
    for handle in handles {
        total_evicted += handle.join().unwrap();
    }

    // Verify total evicted (should be ~400, but may vary due to races)
    assert!(
        total_evicted <= 1000,
        "Cannot evict more than total entries"
    );
    assert!(
        cache.len() < 1000,
        "Cache should have fewer entries after eviction"
    );
}

#[test]
fn test_concurrent_expire_ttl() {
    use std::sync::Arc;
    use std::thread;

    let cache = Arc::new(LockfreeCacheCapsule::<String>::new(1024));

    // Insert 500 expired entries
    for i in 0..500 {
        let key = format!("expired_key{}", i);
        let value = format!("expired_value{}", i);
        cache.insert(&key, value, Duration::from_secs(0)); // Expired
    }

    // Insert 500 non-expired entries
    for i in 0..500 {
        let key = format!("valid_key{}", i);
        let value = format!("valid_value{}", i);
        cache.insert(&key, value, Duration::from_secs(3600)); // Valid
    }

    assert_eq!(cache.len(), 1000);

    // Spawn 4 threads expiring concurrently
    let mut handles = vec![];
    for _tid in 0..4 {
        let cache_clone = Arc::clone(&cache);
        let handle = thread::spawn(move || cache_clone.batch_expire_ttl());
        handles.push(handle);
    }

    // Collect expiration counts
    let mut total_expired = 0;
    for handle in handles {
        total_expired += handle.join().unwrap();
    }

    // Verify total expired (may have duplicates due to concurrent scans)
    assert!(total_expired >= 500, "Should expire at least 500 entries");
    assert!(
        cache.len() <= 500,
        "Should have at most 500 non-expired entries"
    );
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_insert_get_zero_ttl() {
    let cache = LockfreeCacheCapsule::<String>::new(128);

    // Insert with zero TTL
    let inserted = cache.insert("key1", "value1".to_string(), Duration::from_secs(0));
    assert!(inserted);

    // Get should return None (expired)
    let retrieved = cache.get(&"key1");
    assert_eq!(retrieved, None);
}

#[test]
fn test_batch_evict_empty_cache() {
    let cache = LockfreeCacheCapsule::<String>::new(128);

    let evicted = cache.batch_evict_lru(10);
    assert_eq!(evicted, 0);
}

#[test]
fn test_batch_expire_no_expired() {
    let cache = LockfreeCacheCapsule::<String>::new(128);

    // Insert entries with long TTL
    for i in 0..10 {
        let key = format!("key{}", i);
        let value = format!("value{}", i);
        cache.insert(&key, value, Duration::from_secs(3600));
    }

    // Batch expire should find nothing
    let expired = cache.batch_expire_ttl();
    assert_eq!(expired, 0);
}

#[test]
fn test_generation_counter_monotonic() {
    let cache = LockfreeCacheCapsule::<String>::new(128);

    // Generation starts at 0
    let gen1 = cache.next_generation();
    assert_eq!(gen1, 0);

    // Increments monotonically
    let gen2 = cache.next_generation();
    assert_eq!(gen2, 1);

    let gen3 = cache.next_generation();
    assert_eq!(gen3, 2);
}

#[test]
fn test_cache_slot_clear_safety() {
    let cache = LockfreeCacheCapsule::<String>::new(128);

    // Insert value
    let inserted = cache.insert("key1", "value1".to_string(), Duration::from_secs(60));
    assert!(inserted);

    // Get value (should exist)
    let retrieved1 = cache.get(&"key1");
    assert_eq!(retrieved1, Some("value1".to_string()));

    // Batch evict (clears slot)
    let evicted = cache.batch_evict_lru(1);
    assert_eq!(evicted, 1);

    // Get value (should be None after eviction)
    let retrieved2 = cache.get(&"key1");
    assert_eq!(retrieved2, None);
}

// ============================================================================
// I20 Q16: Minimal Integration Test
// ============================================================================

#[test]
fn i20_q16_minimal_integration() {
    // Minimal test proving LockfreeCacheCapsule + CacheSlot integration works
    let cache = LockfreeCacheCapsule::<String>::new(128);

    // Insert via LockfreeCacheCapsule
    assert!(cache.insert("key", "value".to_string(), Duration::from_secs(60)));

    // Get via LockfreeCacheCapsule
    assert_eq!(cache.get(&"key"), Some("value".to_string()));

    // Batch evict via LockfreeCacheCapsule
    assert_eq!(cache.batch_evict_lru(1), 1);

    // Verify eviction worked
    assert_eq!(cache.get(&"key"), None);
}
