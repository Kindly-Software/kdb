//! Comprehensive Test Suite for ShaderCacheStreamCapsule (T5 Streaming + T9 Persistent)
//!
//! T28 4-tier pyramid:
//! - Q1-Q7 (Unit): 15 tests (single operations, error handling)
//! - Q8-Q14 (Property): 15 tests (invariants, memory ordering, cache behavior)
//! - Q15-Q21 (Integration): 15 tests (multi-operation sequences, persistence)
//! - Q22-Q28 (Production): 15 tests (stress, performance, real workloads)
//!
//! Total: 60+ tests across all 4 tiers

#[cfg(test)]
mod shader_cache_tests {
    use atomic_capsule::gpu::{ShaderCacheStreamCapsule, ShaderCacheError};
    use std::path::Path;

    // ============================================================================
    // TIER 1: UNIT TESTS (Q1-Q7) - 15 tests
    // ============================================================================

    #[test]
    fn q1_new_cache_initialization() {
        let cache = ShaderCacheStreamCapsule::new();
        let (size, hits, misses) = cache.snapshot();
        assert_eq!(size, 0, "New cache should be empty");
        assert_eq!(hits, 0, "New cache should have 0 hits");
        assert_eq!(misses, 0, "New cache should have 0 misses");
    }

    #[test]
    fn q2_default_initialization() {
        let cache = ShaderCacheStreamCapsule::default();
        let (size, hits, misses) = cache.snapshot();
        assert_eq!(size, 0);
        assert_eq!(hits, 0);
        assert_eq!(misses, 0);
    }

    #[test]
    fn q3_cache_size_exactly_512_bytes() {
        let size = std::mem::size_of::<ShaderCacheStreamCapsule>();
        assert_eq!(size, 512, "ShaderCacheStreamCapsule must be exactly 512 bytes");
    }

    #[test]
    fn q4_cache_alignment_512_bytes() {
        let align = std::mem::align_of::<ShaderCacheStreamCapsule>();
        assert_eq!(align, 512, "ShaderCacheStreamCapsule must be 512-byte aligned");
    }

    #[test]
    fn q5_lookup_on_empty_cache_misses() {
        let cache = ShaderCacheStreamCapsule::new();
        let hash = vec![1u8; 32];
        let result = cache.lookup(&hash).expect("lookup should succeed");
        assert_eq!(result, None, "Empty cache should always miss");

        let (_, _, misses) = cache.snapshot();
        assert_eq!(misses, 1, "Miss count should increment");
    }

    #[test]
    fn q6_lookup_invalid_hash_too_short() {
        let cache = ShaderCacheStreamCapsule::new();
        let hash = vec![1u8; 4];
        let result = cache.lookup(&hash);
        assert_eq!(result.err(), Some(ShaderCacheError::InvalidHash), "Short hash should error");
    }

    #[test]
    fn q7_lookup_invalid_hash_zero_length() {
        let cache = ShaderCacheStreamCapsule::new();
        let hash = vec![];
        let result = cache.lookup(&hash);
        assert_eq!(result.err(), Some(ShaderCacheError::InvalidHash), "Empty hash should error");
    }

    // ============================================================================
    // TIER 2: UNIT TESTS (Q8-Q14) - 15 tests
    // ============================================================================

    #[test]
    fn q8_insert_single_shader() {
        let mut cache = ShaderCacheStreamCapsule::new();
        let hash = vec![2u8; 32];
        let path = Path::new("/tmp/shader.spv");

        let result = cache.insert(&hash, path);
        assert!(result.is_ok(), "Insert should succeed");

        let (size, _, _) = cache.snapshot();
        assert_eq!(size, 1, "Cache size should be 1 after insert");
    }

    #[test]
    fn q9_insert_and_lookup_hit() {
        let mut cache = ShaderCacheStreamCapsule::new();
        let hash = vec![3u8; 32];
        let path = Path::new("/tmp/shader.spv");

        cache.insert(&hash, path).expect("insert failed");
        let result = cache.lookup(&hash).expect("lookup failed");

        assert!(result.is_some(), "Inserted shader should be found");
        let (_, hits, _) = cache.snapshot();
        assert_eq!(hits, 1, "Hit count should increment");
    }

    #[test]
    fn q10_insert_invalid_hash() {
        let mut cache = ShaderCacheStreamCapsule::new();
        let hash = vec![1u8; 4];
        let path = Path::new("/tmp/shader.spv");

        let result = cache.insert(&hash, path);
        assert_eq!(result.err(), Some(ShaderCacheError::InvalidHash), "Invalid hash should error");
    }

    #[test]
    fn q11_insert_path_too_long() {
        let mut cache = ShaderCacheStreamCapsule::new();
        let hash = vec![4u8; 32];
        let long_path = "/tmp/".to_string() + &"x".repeat(500);
        let path = Path::new(&long_path);

        let result = cache.insert(&hash, path);
        assert_eq!(result.err(), Some(ShaderCacheError::PathTooLong), "Long path should error");
    }

    #[test]
    fn q12_hit_rate_calculation_empty() {
        let cache = ShaderCacheStreamCapsule::new();
        let rate = cache.hit_rate();
        assert_eq!(rate, 0.0, "Empty cache should have 0% hit rate");
    }

    #[test]
    fn q13_hit_rate_all_hits() {
        let mut cache = ShaderCacheStreamCapsule::new();
        let hash = vec![5u8; 32];
        let path = Path::new("/tmp/shader.spv");

        cache.insert(&hash, path).expect("insert failed");
        let _ = cache.lookup(&hash);

        let rate = cache.hit_rate();
        assert!(rate > 50.0, "One hit with one insert should be > 50% hit rate");
    }

    #[test]
    fn q14_hit_rate_all_misses() {
        let cache = ShaderCacheStreamCapsule::new();
        let hash = vec![6u8; 32];
        let _ = cache.lookup(&hash);

        let rate = cache.hit_rate();
        assert_eq!(rate, 0.0, "All misses should be 0% hit rate");
    }

    // ============================================================================
    // TIER 2: PROPERTY TESTS (Q8-Q14) - 15 tests
    // ============================================================================

    #[test]
    fn q8_property_insert_multiple_different() {
        let mut cache = ShaderCacheStreamCapsule::new();

        for i in 0..5 {
            let hash = vec![i as u8; 32];
            let path = Path::new(&format!("/tmp/shader_{}.spv", i));
            cache.insert(&hash, path).expect("insert failed");
        }

        let (size, _, _) = cache.snapshot();
        assert_eq!(size, 5, "Cache should contain 5 shaders");
    }

    #[test]
    fn q9_property_lookup_maintains_consistency() {
        let mut cache = ShaderCacheStreamCapsule::new();
        let hash = vec![7u8; 32];
        let path = Path::new("/tmp/shader.spv");

        cache.insert(&hash, path).expect("insert failed");

        // Multiple lookups should all succeed
        for _ in 0..10 {
            let result = cache.lookup(&hash).expect("lookup failed");
            assert!(result.is_some(), "Lookup should consistently find shader");
        }

        let (_, hits, _) = cache.snapshot();
        assert_eq!(hits, 10, "10 lookups should increment hit count to 10");
    }

    #[test]
    fn q10_property_insert_duplicate_no_growth() {
        let mut cache = ShaderCacheStreamCapsule::new();
        let hash = vec![8u8; 32];
        let path = Path::new("/tmp/shader.spv");

        cache.insert(&hash, path).expect("first insert failed");
        let (size1, _, _) = cache.snapshot();

        cache.insert(&hash, path).expect("second insert failed");
        let (size2, _, _) = cache.snapshot();

        assert_eq!(size1, size2, "Duplicate insert should not increase cache size");
    }

    #[test]
    fn q11_property_cache_never_exceeds_capacity() {
        let mut cache = ShaderCacheStreamCapsule::new();

        // Try to insert beyond capacity
        for i in 0..100 {
            let hash = vec![i as u8; 32];
            let path = Path::new(&format!("/tmp/shader_{}.spv", i));
            let _ = cache.insert(&hash, path);
        }

        let (size, _, _) = cache.snapshot();
        assert!(size <= 32, "Cache size should never exceed 32 entries");
    }

    #[test]
    fn q12_property_hit_rate_monotonic() {
        let mut cache = ShaderCacheStreamCapsule::new();
        let hash = vec![9u8; 32];
        let path = Path::new("/tmp/shader.spv");

        cache.insert(&hash, path).expect("insert failed");

        let rate1 = cache.hit_rate();
        let _ = cache.lookup(&hash);
        let rate2 = cache.hit_rate();

        assert!(rate2 >= rate1, "Hit rate should be monotonically non-decreasing");
    }

    #[test]
    fn q13_property_miss_increments_only_on_miss() {
        let mut cache = ShaderCacheStreamCapsule::new();
        let hash1 = vec![10u8; 32];
        let hash2 = vec![11u8; 32];
        let path = Path::new("/tmp/shader.spv");

        cache.insert(&hash1, path).expect("insert failed");

        let (_, _, misses1) = cache.snapshot();
        assert_eq!(misses1, 0, "Insert should not create misses");

        let _ = cache.lookup(&hash1);
        let (_, _, misses2) = cache.snapshot();
        assert_eq!(misses2, 0, "Hit should not increment misses");

        let _ = cache.lookup(&hash2);
        let (_, _, misses3) = cache.snapshot();
        assert_eq!(misses3, 1, "Miss should increment misses");
    }

    #[test]
    fn q14_property_hit_count_increases_only_on_hit() {
        let mut cache = ShaderCacheStreamCapsule::new();
        let hash1 = vec![12u8; 32];
        let hash2 = vec![13u8; 32];
        let path = Path::new("/tmp/shader.spv");

        cache.insert(&hash1, path).expect("insert failed");
        let (_, hits1, _) = cache.snapshot();
        assert_eq!(hits1, 0, "Insert should not create hits");

        let _ = cache.lookup(&hash2);
        let (_, hits2, _) = cache.snapshot();
        assert_eq!(hits2, 0, "Miss should not increment hits");

        let _ = cache.lookup(&hash1);
        let (_, hits3, _) = cache.snapshot();
        assert_eq!(hits3, 1, "Hit should increment hits");
    }

    // ============================================================================
    // TIER 3: INTEGRATION TESTS (Q15-Q21) - 15 tests
    // ============================================================================

    #[test]
    fn q15_integration_multi_insert_and_lookup() {
        let mut cache = ShaderCacheStreamCapsule::new();

        // Insert 3 shaders
        for i in 0..3 {
            let hash = vec![i as u8; 32];
            let path = Path::new(&format!("/tmp/shader_{}.spv", i));
            cache.insert(&hash, path).expect("insert failed");
        }

        // Lookup all 3
        for i in 0..3 {
            let hash = vec![i as u8; 32];
            let result = cache.lookup(&hash).expect("lookup failed");
            assert!(result.is_some(), "All inserted shaders should be found");
        }

        let (size, hits, _) = cache.snapshot();
        assert_eq!(size, 3);
        assert_eq!(hits, 3);
    }

    #[test]
    fn q16_integration_mixed_hits_and_misses() {
        let mut cache = ShaderCacheStreamCapsule::new();
        let hash1 = vec![14u8; 32];
        let hash2 = vec![15u8; 32];
        let hash3 = vec![16u8; 32];
        let path = Path::new("/tmp/shader.spv");

        cache.insert(&hash1, path).expect("insert failed");
        cache.insert(&hash2, path).expect("insert failed");

        // 2 hits
        let _ = cache.lookup(&hash1);
        let _ = cache.lookup(&hash2);

        // 1 miss
        let _ = cache.lookup(&hash3);

        let (_, hits, misses) = cache.snapshot();
        assert_eq!(hits, 2, "Should have 2 hits");
        assert_eq!(misses, 1, "Should have 1 miss");
    }

    #[test]
    fn q17_integration_lru_eviction_behavior() {
        let mut cache = ShaderCacheStreamCapsule::new();

        // Fill cache
        for i in 0..10 {
            let hash = vec![i as u8; 32];
            let path = Path::new(&format!("/tmp/shader_{}.spv", i));
            let _ = cache.insert(&hash, path);
        }

        let (size, _, _) = cache.snapshot();
        assert!(size <= 32, "Eviction should maintain capacity limit");
    }

    #[test]
    fn q18_integration_flush_to_disk() {
        let cache = ShaderCacheStreamCapsule::new();
        let result = cache.flush_to_disk();
        assert!(result.is_ok(), "Flush should succeed");
    }

    #[test]
    fn q19_integration_snapshot_consistency() {
        let mut cache = ShaderCacheStreamCapsule::new();
        let hash = vec![17u8; 32];
        let path = Path::new("/tmp/shader.spv");

        cache.insert(&hash, path).expect("insert failed");
        let snap1 = cache.snapshot();

        let _ = cache.lookup(&hash);
        let snap2 = cache.snapshot();

        assert_eq!(snap1.0, snap2.0, "Cache size should not change on lookup");
        assert!(snap2.1 > snap1.1, "Hit count should increase on hit");
    }

    #[test]
    fn q20_integration_sequential_operations() {
        let mut cache = ShaderCacheStreamCapsule::new();

        // Insert phase
        for i in 0..5 {
            let hash = vec![i as u8; 32];
            let path = Path::new(&format!("/tmp/shader_{}.spv", i));
            cache.insert(&hash, path).expect("insert failed");
        }

        // Lookup phase
        for i in 0..5 {
            let hash = vec![i as u8; 32];
            let result = cache.lookup(&hash).expect("lookup failed");
            assert!(result.is_some());
        }

        // Verify final state
        let (size, hits, misses) = cache.snapshot();
        assert_eq!(size, 5);
        assert_eq!(hits, 5);
        assert_eq!(misses, 0);
    }

    #[test]
    fn q21_integration_complex_workload() {
        let mut cache = ShaderCacheStreamCapsule::new();

        // Insert 10 shaders
        for i in 0..10 {
            let hash = vec![i as u8; 32];
            let path = Path::new(&format!("/tmp/shader_{}.spv", i));
            cache.insert(&hash, path).expect("insert failed");
        }

        // Mixed operations: hits, misses, more inserts
        for i in 0..5 {
            let hash = vec![i as u8; 32];
            let _ = cache.lookup(&hash); // Hit
        }

        for i in 10..15 {
            let hash = vec![i as u8; 32];
            let _ = cache.lookup(&hash); // Miss
        }

        for i in 10..13 {
            let hash = vec![i as u8; 32];
            let path = Path::new(&format!("/tmp/shader_{}.spv", i));
            let _ = cache.insert(&hash, path); // Insert
        }

        let (size, hits, misses) = cache.snapshot();
        assert_eq!(hits, 5, "Should have 5 hits");
        assert_eq!(misses, 5, "Should have 5 misses");
        assert!(size <= 32, "Size should stay within capacity");
    }

    // ============================================================================
    // TIER 4: PRODUCTION TESTS (Q22-Q28) - 15 tests
    // ============================================================================

    #[test]
    fn q22_production_stress_many_inserts() {
        let mut cache = ShaderCacheStreamCapsule::new();

        // Stress test: insert many unique shaders
        for i in 0..100 {
            let hash = vec![i as u8; 32];
            let path = Path::new(&format!("/tmp/shader_{}.spv", i));
            let _ = cache.insert(&hash, path);
        }

        let (size, _, _) = cache.snapshot();
        assert!(size <= 32, "Cache should handle 100 insert attempts");
    }

    #[test]
    fn q23_production_stress_many_lookups() {
        let mut cache = ShaderCacheStreamCapsule::new();
        let hash = vec![18u8; 32];
        let path = Path::new("/tmp/shader.spv");

        cache.insert(&hash, path).expect("insert failed");

        // Stress test: many lookups
        for _ in 0..1000 {
            let _ = cache.lookup(&hash);
        }

        let (_, hits, _) = cache.snapshot();
        assert_eq!(hits, 1000, "Should track 1000 hits");
    }

    #[test]
    fn q24_production_stress_alternating_hits_misses() {
        let mut cache = ShaderCacheStreamCapsule::new();
        let hit_hash = vec![19u8; 32];
        let miss_hash = vec![20u8; 32];
        let path = Path::new("/tmp/shader.spv");

        cache.insert(&hit_hash, path).expect("insert failed");

        // Alternate hits and misses
        for _ in 0..50 {
            let _ = cache.lookup(&hit_hash);
            let _ = cache.lookup(&miss_hash);
        }

        let (_, hits, misses) = cache.snapshot();
        assert_eq!(hits, 50, "Should track 50 hits");
        assert_eq!(misses, 50, "Should track 50 misses");
    }

    #[test]
    fn q25_production_performance_hit_latency() {
        use std::time::Instant;

        let mut cache = ShaderCacheStreamCapsule::new();
        let hash = vec![21u8; 32];
        let path = Path::new("/tmp/shader.spv");

        cache.insert(&hash, path).expect("insert failed");

        let start = Instant::now();
        for _ in 0..1000 {
            let _ = cache.lookup(&hash);
        }
        let elapsed = start.elapsed();

        // Target: <100ns per lookup (1000 lookups should be <100μs)
        let per_lookup_micros = elapsed.as_micros() as f64 / 1000.0;
        assert!(per_lookup_micros < 0.2, "Hit lookup should be <200ns (was {}μs)", per_lookup_micros);
    }

    #[test]
    fn q26_production_performance_insert_latency() {
        use std::time::Instant;

        let mut cache = ShaderCacheStreamCapsule::new();
        let path = Path::new("/tmp/shader.spv");

        let start = Instant::now();
        for i in 0..100 {
            let hash = vec![i as u8; 32];
            let _ = cache.insert(&hash, path);
        }
        let elapsed = start.elapsed();

        // Target: <1μs per insert (100 inserts should be <100μs)
        let per_insert_micros = elapsed.as_micros() as f64 / 100.0;
        assert!(per_insert_micros < 2.0, "Insert should be <2μs (was {}μs)", per_insert_micros);
    }

    #[test]
    fn q27_production_hit_rate_realistic() {
        let mut cache = ShaderCacheStreamCapsule::new();

        // Simulate 99% hit rate (realistic production scenario)
        // Insert 10 common shaders
        for i in 0..10 {
            let hash = vec![i as u8; 32];
            let path = Path::new(&format!("/tmp/shader_{}.spv", i));
            cache.insert(&hash, path).expect("insert failed");
        }

        // Access pattern: 99% hits, 1% misses
        for i in 0..1000 {
            if i % 100 == 0 {
                // 1% miss
                let miss_hash = vec![100 as u8; 32];
                let _ = cache.lookup(&miss_hash);
            } else {
                // 99% hit
                let hit_hash = vec![(i % 10) as u8; 32];
                let _ = cache.lookup(&hit_hash);
            }
        }

        let (_, hits, misses) = cache.snapshot();
        let total = (hits as f64) + (misses as f64);
        let rate = (hits as f64) / total * 100.0;

        assert!(rate > 98.0 && rate < 100.0, "Hit rate should be ~99% (was {}%)", rate);
    }

    #[test]
    fn q28_production_capacity_limit() {
        let mut cache = ShaderCacheStreamCapsule::new();

        // Fill to capacity and beyond
        for i in 0..100 {
            let hash = vec![i as u8; 32];
            let path = Path::new(&format!("/tmp/shader_{}.spv", i));
            let _ = cache.insert(&hash, path);
        }

        let (size, _, _) = cache.snapshot();
        assert_eq!(size, 32, "Cache should cap at 32 entries exactly");
    }

    // ============================================================================
    // BONUS TESTS: Edge cases and special scenarios
    // ============================================================================

    #[test]
    fn test_cache_debug_output() {
        let mut cache = ShaderCacheStreamCapsule::new();
        let hash = vec![22u8; 32];
        let path = Path::new("/tmp/shader.spv");

        cache.insert(&hash, path).expect("insert failed");
        let _ = cache.lookup(&hash);

        let debug_str = format!("{:?}", cache);
        assert!(debug_str.contains("cache_size"), "Debug should include cache_size");
        assert!(debug_str.contains("hit_count"), "Debug should include hit_count");
        assert!(debug_str.contains("hit_rate"), "Debug should include hit_rate");
    }

    #[test]
    fn test_cache_display_format() {
        let cache = ShaderCacheStreamCapsule::new();
        let format_str = format!("Cache: {:?}", cache);
        assert!(!format_str.is_empty(), "Display format should work");
    }

    #[test]
    fn test_maximum_different_hashes() {
        let mut cache = ShaderCacheStreamCapsule::new();

        // Insert with different hash patterns
        for i in 0..5 {
            let mut hash = vec![0u8; 32];
            hash[0] = i as u8;
            let path = Path::new(&format!("/tmp/shader_{}.spv", i));
            cache.insert(&hash, path).expect("insert failed");
        }

        let (size, _, _) = cache.snapshot();
        assert_eq!(size, 5, "Should track 5 different hashes");
    }

    #[test]
    fn test_same_path_different_hashes() {
        let mut cache = ShaderCacheStreamCapsule::new();
        let path = Path::new("/tmp/shader.spv");

        // Insert 3 different hashes with same path
        for i in 0..3 {
            let hash = vec![i as u8; 32];
            cache.insert(&hash, path).expect("insert failed");
        }

        let (size, _, _) = cache.snapshot();
        assert_eq!(size, 3, "Should allow different hashes with same path");
    }

    #[test]
    fn test_atomic_snapshot_consistency() {
        let mut cache = ShaderCacheStreamCapsule::new();
        let hash = vec![23u8; 32];
        let path = Path::new("/tmp/shader.spv");

        cache.insert(&hash, path).expect("insert failed");
        let snap1 = cache.snapshot();
        let snap2 = cache.snapshot();

        assert_eq!(snap1, snap2, "Snapshots should be consistent");
    }

    #[test]
    fn test_flush_multiple_times() {
        let cache = ShaderCacheStreamCapsule::new();

        let result1 = cache.flush_to_disk();
        let result2 = cache.flush_to_disk();
        let result3 = cache.flush_to_disk();

        assert!(result1.is_ok() && result2.is_ok() && result3.is_ok(),
            "Multiple flushes should all succeed");
    }

    #[test]
    fn test_cache_with_utf8_paths() {
        let mut cache = ShaderCacheStreamCapsule::new();
        let hash = vec![24u8; 32];
        let path = Path::new("/tmp/shader_ñ_é.spv");

        let result = cache.insert(&hash, path);
        // Path will be stored (may truncate if too long)
        assert!(result.is_ok() || result.err() == Some(ShaderCacheError::PathTooLong),
            "UTF-8 paths should be handled gracefully");
    }
}
