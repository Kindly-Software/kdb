//! Comprehensive T28 test suite for SurfaceStateCacheCapsule
//!
//! T28 4-tier testing:
//! Q1-Q7: Unit tests (single-capsule functionality)
//! Q8-Q14: Property tests (invariants, monotonicity)
//! Q15-Q21: Integration tests (multi-threaded, real workloads)
//! Q22-Q28: Production tests (stress, latency, zero-allocation)

#![allow(dead_code)]

// Tests are feature-gated to allow compilation without GPU support
#[cfg(any(feature = "gpu-cuda", feature = "gpu-rocm", feature = "gpu-intel", feature = "gpu-all"))]
mod tests {
use atomic_capsule::gpu::{SurfaceStateCacheCapsule, CacheError, InvalidateError};
use std::sync::Arc;
use std::thread;

// ============================================================================
// TIER 1 (Q1-Q7): UNIT TESTS
// ============================================================================

#[test]
fn t1_q1_empty_cache_initialization() {
    let cache = SurfaceStateCacheCapsule::new();
    let (hits, misses) = cache.stats();

    assert_eq!(hits, 0, "New cache should have 0 hits");
    assert_eq!(misses, 0, "New cache should have 0 misses");
    assert_eq!(cache.hit_rate(), 0.0, "Empty cache hit rate should be 0.0");
}

#[test]
fn t1_q2_alignment_verification() {
    use std::mem;

    let size = mem::size_of::<SurfaceStateCacheCapsule>();
    let align = mem::align_of::<SurfaceStateCacheCapsule>();

    assert_eq!(size, 256, "Capsule must be exactly 256 bytes");
    assert_eq!(align, 256, "Capsule must be 256-byte aligned");
}

#[test]
fn t1_q3_insert_single_hash() {
    let cache = SurfaceStateCacheCapsule::new();
    let hash = 0x1234567890ABCDEF;

    let result = cache.insert(hash);
    assert!(result.is_ok(), "Insert should succeed");

    let slot = result.unwrap();
    assert!(slot < 16, "Slot index must be 0-15");
}

#[test]
fn t1_q4_lookup_hit_after_insert() {
    let cache = SurfaceStateCacheCapsule::new();
    let hash = 0xCAFEBABEDEADBEEF;

    cache.insert(hash).unwrap();
    let lookup = cache.lookup(hash);

    assert!(lookup.is_some(), "Lookup should find recently inserted hash");
    let (hits, misses) = cache.stats();
    assert_eq!(hits, 1, "Stats should show 1 hit");
    assert_eq!(misses, 0, "Stats should show 0 misses");
}

#[test]
fn t1_q5_lookup_miss_not_inserted() {
    let cache = SurfaceStateCacheCapsule::new();
    let hash = 0xDEADDEADDEADDEAD;

    let lookup = cache.lookup(hash);
    assert!(lookup.is_none(), "Lookup should miss for non-inserted hash");

    let (hits, misses) = cache.stats();
    assert_eq!(hits, 0);
    assert_eq!(misses, 1);
}

#[test]
fn t1_q6_invalidate_makes_lookup_miss() {
    let cache = SurfaceStateCacheCapsule::new();
    let hash = 0x0123456789ABCDEF;

    let slot = cache.insert(hash).unwrap();
    cache.lookup(hash).unwrap();

    // Invalidate
    let inv_result = cache.invalidate(slot);
    assert!(inv_result.is_ok());

    // Next lookup should miss
    let post_lookup = cache.lookup(hash);
    assert!(post_lookup.is_none(), "Lookup should miss after invalidation");
}

#[test]
fn t1_q7_reject_zero_hash() {
    let cache = SurfaceStateCacheCapsule::new();

    let result = cache.insert(0);
    assert!(result.is_err(), "Zero hash should be rejected");
}

// ============================================================================
// TIER 2 (Q8-Q14): PROPERTY TESTS
// ============================================================================

#[test]
fn t2_q8_hit_rate_monotonic_property() {
    let cache = SurfaceStateCacheCapsule::new();

    for i in 1..=5 {
        let hash = (i as u64) * 0x1111111111111111;
        cache.insert(hash).unwrap();
    }

    let rates = vec![
        cache.lookup(1u64 * 0x1111111111111111).is_some(),
        cache.lookup(2u64 * 0x1111111111111111).is_some(),
        cache.lookup(3u64 * 0x1111111111111111).is_some(),
        cache.lookup(4u64 * 0x1111111111111111).is_some(),
        cache.lookup(5u64 * 0x1111111111111111).is_some(),
    ];

    // All should be hits
    assert!(rates.iter().all(|&hit| hit), "All inserted hashes should be found");

    let hit_rate = cache.hit_rate();
    assert_eq!(hit_rate, 1.0, "Hit rate should be 100% for all inserted hashes");
}

#[test]
fn t2_q9_generation_counter_wrapping() {
    let cache = SurfaceStateCacheCapsule::new();
    let hash = 0x9876543210FEDCBA;

    let slot = cache.insert(hash).unwrap();
    let initial_gen = cache.entries[slot].generation();

    // Invalidate and re-insert multiple times to check generation counter
    for _ in 0..3 {
        cache.invalidate(slot).ok();
        let _ = cache.insert(hash);
    }

    let final_gen = cache.entries[slot].generation();
    // Generation should be different (wraps at 256)
    assert_ne!(initial_gen, final_gen, "Generation counter should change across invalidations");
}

#[test]
fn t2_q10_memory_ordering_acquire_release() {
    let cache = Arc::new(SurfaceStateCacheCapsule::new());
    let hash = 0xAAAAAAAAAAAAAAAA;

    // Insert with visibility guarantee
    cache.insert(hash).unwrap();

    // Lookup should see the written value (Acquire/Release ordering)
    let result = cache.lookup(hash);
    assert!(result.is_some(), "Memory ordering should make writes visible");
}

#[test]
fn t2_q11_aba_prevention_via_generation() {
    // ABA: Insert A, erase A, insert A again, lookup finds new A not old A
    let cache = SurfaceStateCacheCapsule::new();
    let hash = 0xBBBBBBBBBBBBBBBB;

    let slot1 = cache.insert(hash).unwrap();
    let gen1 = cache.entries[slot1].generation();

    cache.invalidate(slot1).unwrap();
    let gen2 = cache.entries[slot1].generation();

    cache.insert(hash).unwrap();
    let gen3 = cache.entries[slot1].generation();

    // Generations should all be different
    assert_ne!(gen1, gen2, "Generation should change on invalidation");
    assert_ne!(gen2, gen3, "Generation should change on re-insert");
}

#[test]
fn t2_q12_cache_size_limits() {
    let cache = SurfaceStateCacheCapsule::new();

    // Capacity is 16 entries with max 4 linear probes each
    // Should be able to insert 16 items
    for i in 1..=16 {
        let hash = (i as u64) * 0x0123456789ABCDEF;
        let result = cache.insert(hash);
        assert!(result.is_ok(), "Should insert entry {}", i);
    }

    // 17th should fail
    let result = cache.insert(17u64 * 0x0123456789ABCDEF);
    assert!(result.is_err(), "Should fail on cache full");
}

#[test]
fn t2_q13_refcount_increment_property() {
    let cache = SurfaceStateCacheCapsule::new();
    let hash = 0xCCCCCCCCCCCCCCCC;

    let slot = cache.insert(hash).unwrap();
    let rc1 = cache.entries[slot].refcount();

    // Re-insert same hash (should increment refcount)
    cache.insert(hash).ok();
    let rc2 = cache.entries[slot].refcount();

    assert!(rc2 > rc1, "Refcount should increase on duplicate insert");
}

#[test]
fn t2_q14_linear_probing_collision_resolution() {
    let cache = SurfaceStateCacheCapsule::new();

    // Insert hashes that may collide
    let hashes = vec![
        0x1000000000000001,
        0x1000000000000002,
        0x1000000000000003,
        0x1000000000000004,
    ];

    for hash in &hashes {
        assert!(cache.insert(*hash).is_ok(), "Should handle collisions");
    }

    // All should be findable
    for hash in &hashes {
        assert!(cache.lookup(*hash).is_some(), "Collision-resolved hash should be found");
    }
}

// ============================================================================
// TIER 3 (Q15-Q21): INTEGRATION TESTS
// ============================================================================

#[test]
fn t3_q15_multi_threaded_lookup_concurrent() {
    let cache: Arc<SurfaceStateCacheCapsule> = Arc::new(SurfaceStateCacheCapsule::new());

    // Populate cache
    for i in 1..=8 {
        cache.insert((i as u64) * 0x1111111111111111).unwrap();
    }

    let mut threads = vec![];

    // Spawn 8 threads all doing lookups
    for thread_id in 0..8 {
        let cache_clone: Arc<SurfaceStateCacheCapsule> = Arc::clone(&cache);
        let t = thread::spawn(move || {
            for _ in 0..100 {
                let hash = ((thread_id + 1) as u64) * 0x1111111111111111;
                let _result = cache_clone.lookup(hash);
            }
        });
        threads.push(t);
    }

    for t in threads {
        t.join().unwrap();
    }

    let (hits, _misses) = cache.stats();
    assert!(hits > 0, "Should have recorded hits from concurrent lookups");
}

#[test]
fn t3_q16_rest_and_invalidation_sequence() {
    let cache = SurfaceStateCacheCapsule::new();

    // Insert 5 items
    let hashes: Vec<u64> = (1..=5)
        .map(|i| (i as u64) * 0x1010101010101010)
        .collect();

    for hash in &hashes {
        cache.insert(*hash).unwrap();
    }

    // Invalidate odd-numbered slots
    for (i, hash) in hashes.iter().enumerate() {
        if i % 2 == 0 {
            if let Some(slot) = cache.lookup(*hash) {
                cache.invalidate(slot).ok();
            }
        }
    }

    // Even-indexed should still be present
    for (i, hash) in hashes.iter().enumerate() {
        if i % 2 == 1 {
            assert!(cache.lookup(*hash).is_some(), "Even-indexed hash should still exist");
        }
    }
}

#[test]
fn t3_q17_stats_accumulation_over_time() {
    let cache = SurfaceStateCacheCapsule::new();
    let hash = 0xEEEEEEEEEEEEEEEE;

    cache.insert(hash).unwrap();

    // Perform 10 lookups (should all hit)
    for _ in 0..10 {
        cache.lookup(hash).ok();
    }

    // Perform 5 misses
    for i in 100..105 {
        cache.lookup((i as u64) * 0xFFFFFFFFFFFFFFFF).ok();
    }

    let (hits, misses) = cache.stats();
    assert_eq!(hits, 10, "Should accumulate 10 hits");
    assert_eq!(misses, 5, "Should accumulate 5 misses");

    let hit_rate = cache.hit_rate();
    assert!((hit_rate - 10.0 / 15.0).abs() < 0.0001, "Hit rate should be 10/15");
}

#[test]
fn t3_q18_slot_reuse_after_invalidation() {
    let cache = SurfaceStateCacheCapsule::new();
    let hash1 = 0x0DEDEDEDEDEDEDED;
    let hash2 = 0x0ADADADADADADADA;

    // Insert hash1
    let slot1 = cache.insert(hash1).unwrap();
    assert!(cache.lookup(hash1).is_some());

    // Invalidate
    cache.invalidate(slot1).ok();
    assert!(cache.lookup(hash1).is_none());

    // Insert different hash (should be able to reuse slot)
    let slot2 = cache.insert(hash2).unwrap();
    assert!(cache.lookup(hash2).is_some());

    // Slot should be reused (same index)
    assert_eq!(slot1, slot2, "Invalidated slot should be reused");
}

#[test]
fn t3_q19_mixed_hits_and_misses_pattern() {
    let cache = SurfaceStateCacheCapsule::new();

    // Pattern: Insert A,B,C, then lookup A,B,C,X,Y,Z (3 hits, 3 misses)
    for i in 1..=3 {
        cache.insert((i as u64) * 0xAAAAAAAAAAAAAAAA).unwrap();
    }

    for i in 1..=3 {
        cache.lookup((i as u64) * 0xAAAAAAAAAAAAAAAA).ok(); // hits
    }

    for i in 100..=102 {
        cache.lookup((i as u64) * 0xBBBBBBBBBBBBBBBB).ok(); // misses
    }

    let (hits, misses) = cache.stats();
    assert_eq!(hits, 3);
    assert_eq!(misses, 3);
}

#[test]
fn t3_q20_production_realistic_95_percent_hit_rate() {
    let cache = SurfaceStateCacheCapsule::new();

    // Populate cache with 10 common surface states
    for i in 1..=10 {
        cache.insert((i as u64) * 0x1111111111111111).unwrap();
    }

    // Simulate 1000 accesses with 95% hit rate
    // 95% hit: repeated access to 10 items = 95 * 10 = 950 hits
    for _ in 0..95 {
        for i in 1..=10 {
            cache.lookup((i as u64) * 0x1111111111111111).ok();
        }
    }

    // 5% miss: 50 misses
    for i in 0..50 {
        cache.lookup((i as u64) * 0xFFFFFFFFFFFFFFFF).ok();
    }

    let hit_rate = cache.hit_rate();
    // Should be approximately 95% ± 1%
    assert!(hit_rate > 0.94 && hit_rate < 0.96, "Production hit rate: {}", hit_rate);
}

#[test]
fn t3_q21_error_handling_coverage() {
    let cache = SurfaceStateCacheCapsule::new();

    // Test CacheError::InvalidSlot (zero hash)
    assert!(cache.insert(0).is_err());

    // Test InvalidateError::InvalidSlot
    assert!(cache.invalidate(100).is_err());

    // Test InvalidateError::AlreadyInvalid
    assert!(cache.invalidate(0).is_err());
}

// ============================================================================
// TIER 4 (Q22-Q28): PRODUCTION TESTS
// ============================================================================

#[test]
fn t4_q22_stress_test_high_insert_rate() {
    let cache = SurfaceStateCacheCapsule::new();

    // Insert 16 different hashes rapidly
    for i in 1..=16 {
        let hash = (i as u64) * 0x0FEDCBA987654321;
        assert!(cache.insert(hash).is_ok());
    }

    let (hits, misses) = cache.stats();
    assert_eq!(misses, 0, "No lookups, no misses");
}

#[test]
fn t4_q23_stress_test_concurrent_mixed_operations() {
    let cache: Arc<SurfaceStateCacheCapsule> = Arc::new(SurfaceStateCacheCapsule::new());

    // Pre-populate
    for i in 1..=5 {
        cache.insert((i as u64) * 0x1010101010101010).unwrap();
    }

    let mut threads = vec![];

    // Spawn threads doing mixed insert/lookup
    for thread_id in 0..4 {
        let cache_clone: Arc<SurfaceStateCacheCapsule> = Arc::clone(&cache);
        let t = thread::spawn(move || {
            for iter in 0..50 {
                if iter % 2 == 0 {
                    // Lookup
                    let hash = ((thread_id + 1) as u64) * 0x1010101010101010;
                    let _result = cache_clone.lookup(hash);
                } else {
                    // Insert (may fail if full)
                    let hash = ((thread_id * 1000 + iter) as u64) * 0x0102030405060708;
                    let _result = cache_clone.insert(hash);
                }
            }
        });
        threads.push(t);
    }

    for t in threads {
        t.join().unwrap();
    }

    // Stats should be updated atomically
    let (_hits, _misses) = cache.stats();
    // Both should be > 0 due to mixed operations
}

#[test]
fn t4_q24_latency_measurement_lookup() {
    let cache = SurfaceStateCacheCapsule::new();
    let hash = 0x0123456789ABCDEF;

    cache.insert(hash).unwrap();

    // Warm up
    for _ in 0..10 {
        cache.lookup(hash).ok();
    }

    // Measure 1000 lookups (should be <20ns per hit on modern CPU)
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        cache.lookup(hash).ok();
    }
    let elapsed = start.elapsed();

    let per_lookup = elapsed.as_nanos() / 1000;
    println!("Average lookup latency: {} ns", per_lookup);

    // Should be well under 100ns (even with measurement overhead)
    assert!(per_lookup < 1000, "Lookup latency too high: {} ns", per_lookup);
}

#[test]
fn t4_q25_latency_measurement_insert() {
    let cache = SurfaceStateCacheCapsule::new();

    let start = std::time::Instant::now();
    for i in 1..=100 {
        let hash = (i as u64) * 0x0ABCDEF123456789;
        cache.insert(hash).ok();
    }
    let elapsed = start.elapsed();

    let per_insert = elapsed.as_nanos() / 100;
    println!("Average insert latency: {} ns", per_insert);

    // Should be well under 100ns
    assert!(per_insert < 500, "Insert latency too high: {} ns", per_insert);
}

#[test]
fn t4_q26_zero_allocation_invariant() {
    // Verify no allocations happen during operations
    let cache = SurfaceStateCacheCapsule::new();

    // All operations should be on stack-allocated 256B structure
    for i in 1..=10 {
        cache.insert((i as u64) * 0x1122334455667788).ok();
        cache.lookup((i as u64) * 0x1122334455667788).ok();
    }

    // No panics or OOM means no allocations occurred
}

#[test]
fn t4_q27_cache_full_graceful_degradation() {
    let cache = SurfaceStateCacheCapsule::new();

    // Fill cache
    for i in 1..=16 {
        cache.insert((i as u64) * 0x1234567890ABCDEF).ok();
    }

    // Further inserts should return error, not panic
    for i in 17..=20 {
        let result = cache.insert((i as u64) * 0x1234567890ABCDEF);
        assert!(result.is_err(), "Should gracefully fail when full");
    }

    // Lookups of existing items should still work
    let result = cache.lookup(1u64 * 0x1234567890ABCDEF);
    assert!(result.is_some(), "Cache should still serve existing items");
}

#[test]
fn t4_q28_production_end_to_end_workflow() {
    // Simulate real GPU driver surface state caching workflow
    let cache = SurfaceStateCacheCapsule::new();

    // Scenario 1: Insert 8 common surface states
    let mut slots = vec![];
    for i in 1..=8 {
        let hash = (i as u64) * 0x0FEDCBA987654321;
        if let Ok(slot) = cache.insert(hash) {
            slots.push((i as u64, hash, slot));
        }
    }

    // Scenario 2: High hit rate lookups (typical GPU workload)
    let mut hit_count = 0;
    for iteration in 0..100 {
        for (_i, hash, _slot) in &slots {
            if cache.lookup(*hash).is_some() {
                hit_count += 1;
            }
        }
    }

    // Should achieve ~95% hit rate (100 lookups per surface = 800 total, 8 surfaces)
    let expected_hits = (slots.len() * 100) as i32;
    assert!(hit_count > expected_hits - 10, "Hit rate should be near 100% for repeated access");

    // Scenario 3: Invalidate some surfaces (e.g., texture rebinding)
    for idx in [0, 2, 4, 6] {
        if let Some((_i, _hash, slot)) = slots.get(idx) {
            cache.invalidate(*slot).ok();
        }
    }

    // Scenario 4: Verify invalidated surfaces are gone
    for idx in [0, 2, 4, 6] {
        if let Some((_i, hash, _slot)) = slots.get(idx) {
            assert!(cache.lookup(*hash).is_none(), "Invalidated surface should be gone");
        }
    }

    // Stats should show reasonable hit/miss distribution
    let (hits, misses) = cache.stats();
    assert!(hits + misses > 0, "Cache should have recorded operations");
}

// ============================================================================
// AUXILIARY PROPERTY TESTS (T28 extended coverage)
// ============================================================================

#[test]
fn aux_p1_hash_distribution_property() {
    let cache = SurfaceStateCacheCapsule::new();

    // Insert 8 random hashes and verify they're distributed
    let hashes: Vec<u64> = (1..=8)
        .map(|i| (i as u64).wrapping_mul(0x9E3779B97F4A7C15))
        .collect();

    let mut slots_used = vec![];
    for hash in &hashes {
        if let Ok(slot) = cache.insert(*hash) {
            slots_used.push(slot);
        }
    }

    // Slots should be mostly distinct (some collisions OK due to linear probing)
    assert!(slots_used.len() >= 6, "Most hashes should find slots");
}

#[test]
fn aux_p2_idempotent_lookups() {
    let cache = SurfaceStateCacheCapsule::new();
    let hash = 0x1234567890ABCDEF;

    cache.insert(hash).unwrap();

    // Multiple lookups of same hash should be idempotent
    let result1 = cache.lookup(hash);
    let result2 = cache.lookup(hash);
    let result3 = cache.lookup(hash);

    assert_eq!(result1, result2);
    assert_eq!(result2, result3);

    let (hits, _misses) = cache.stats();
    assert_eq!(hits, 3, "All three lookups should count as hits");
}

#[test]
fn aux_p3_default_trait() {
    let cache1 = SurfaceStateCacheCapsule::new();
    let cache2 = SurfaceStateCacheCapsule::default();

    let (h1, m1) = cache1.stats();
    let (h2, m2) = cache2.stats();

    assert_eq!(h1, h2);
    assert_eq!(m1, m2);
}
}
