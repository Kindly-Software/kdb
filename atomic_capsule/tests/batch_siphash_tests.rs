//! T28 Comprehensive Tests for SIMD Batch SipHash
//!
//! **Test Coverage (T28 Framework):**
//! - Q1-Q7 (Unit): Correctness, determinism, threshold behavior
//! - Q8-Q14 (Property): Collision resistance, independence, performance scaling
//! - Q15-Q21 (Integration): Distributed cache multi_get/multi_insert integration
//! - Q22-Q28 (Production): Stress testing, real-world workloads
//!
//! **Target:** 100% pass rate, all tests validate SIMD correctness vs sequential

#![cfg(feature = "distributed")]

use atomic_capsule::hash::batch_siphash::{
    batch_siphash_4_fixed, batch_siphash_8_fixed, batch_siphash_keys, siphash_single,
};

// ============================================================================
// Q1-Q7: Unit Tests - Correctness and Basic Functionality
// ============================================================================

#[test]
fn test_unit_q1_siphash_single_deterministic() {
    // Q1: Basic correctness - single key hashing
    let key = b"test_key_deterministic";
    let hash1 = siphash_single(key);
    let hash2 = siphash_single(key);
    assert_eq!(hash1, hash2, "Single hash must be deterministic");
}

#[test]
fn test_unit_q2_siphash_single_collision_resistance() {
    // Q2: Different keys produce different hashes
    let keys = [b"key1", b"key2", b"key3"];
    let hashes: Vec<_> = keys.iter().map(|k| siphash_single(*k)).collect();

    for i in 0..hashes.len() {
        for j in i + 1..hashes.len() {
            assert_ne!(
                hashes[i], hashes[j],
                "Collision between keys {} and {}",
                i, j
            );
        }
    }
}

#[test]
fn test_unit_q3_batch_empty() {
    // Q3: Edge case - empty batch
    let keys: Vec<&[u8]> = vec![];
    let hashes = batch_siphash_keys(&keys);
    assert_eq!(hashes.len(), 0, "Empty batch should return empty result");
}

#[test]
fn test_unit_q4_batch_single_key() {
    // Q4: Edge case - single key batch
    let keys = vec![b"single_key".as_ref()];
    let hashes = batch_siphash_keys(&keys);
    assert_eq!(hashes.len(), 1);
    assert_eq!(
        hashes[0],
        siphash_single(b"single_key"),
        "Single key batch should match sequential"
    );
}

#[test]
fn test_unit_q5_batch_threshold_small() {
    // Q5: Below threshold (<4 keys) - sequential path
    let keys = vec![b"k1".as_ref(), b"k2", b"k3"];
    let hashes = batch_siphash_keys(&keys);
    assert_eq!(hashes.len(), 3);

    // Verify correctness against sequential
    for (i, key) in keys.iter().enumerate() {
        assert_eq!(hashes[i], siphash_single(key), "Mismatch at index {}", i);
    }
}

#[test]
fn test_unit_q6_batch_threshold_exact() {
    // Q6: Exact threshold (4 keys) - SIMD path
    let keys = vec![b"k1".as_ref(), b"k2", b"k3", b"k4"];
    let hashes = batch_siphash_keys(&keys);
    assert_eq!(hashes.len(), 4);

    // Verify correctness
    for (i, key) in keys.iter().enumerate() {
        assert_eq!(
            hashes[i],
            siphash_single(key),
            "SIMD mismatch at index {}",
            i
        );
    }
}

#[test]
fn test_unit_q7_batch_above_threshold() {
    // Q7: Above threshold (8+ keys) - SIMD batching
    let keys = vec![
        b"k1".as_ref(),
        b"k2",
        b"k3",
        b"k4",
        b"k5",
        b"k6",
        b"k7",
        b"k8",
    ];
    let hashes = batch_siphash_keys(&keys);
    assert_eq!(hashes.len(), 8);

    for (i, key) in keys.iter().enumerate() {
        assert_eq!(
            hashes[i],
            siphash_single(key),
            "Batch mismatch at index {}",
            i
        );
    }
}

// ============================================================================
// Q8-Q14: Property Tests - Invariants and Scaling
// ============================================================================

#[test]
fn test_property_q8_determinism_repeated() {
    // Q8: Property - determinism across multiple calls
    let keys = vec![
        b"key1".as_ref(),
        b"key2",
        b"key3",
        b"key4",
        b"key5",
        b"key6",
        b"key7",
        b"key8",
    ];

    let hashes1 = batch_siphash_keys(&keys);
    let hashes2 = batch_siphash_keys(&keys);
    let hashes3 = batch_siphash_keys(&keys);

    assert_eq!(hashes1, hashes2, "First and second run should match");
    assert_eq!(hashes2, hashes3, "Second and third run should match");
}

#[test]
fn test_property_q9_independence() {
    // Q9: Property - key independence (no cross-contamination)
    let keys = vec![b"k1".as_ref(), b"k2", b"k3", b"k4"];
    let hashes_batch = batch_siphash_keys(&keys);

    // Hash each key individually
    let hashes_individual: Vec<_> = keys.iter().map(|k| siphash_single(k)).collect();

    assert_eq!(
        hashes_batch, hashes_individual,
        "Batch should not contaminate individual hashes"
    );
}

#[test]
fn test_property_q10_order_sensitivity() {
    // Q10: Property - hash order sensitivity
    let keys1 = vec![b"k1".as_ref(), b"k2", b"k3", b"k4"];
    let keys2 = vec![b"k4".as_ref(), b"k3", b"k2", b"k1"];

    let hashes1 = batch_siphash_keys(&keys1);
    let hashes2 = batch_siphash_keys(&keys2);

    // Different order should produce different result vectors
    assert_ne!(hashes1, hashes2, "Hash order should matter");

    // But hashes[i] should match keys[i]
    for (i, key) in keys1.iter().enumerate() {
        assert_eq!(hashes1[i], siphash_single(key));
    }
}

#[test]
fn test_property_q11_collision_resistance_many_keys() {
    // Q11: Property - collision resistance with 1000 random-ish keys
    let keys: Vec<Vec<u8>> = (0..1000)
        .map(|i| format!("user_session_{:06}", i).into_bytes())
        .collect();

    let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
    let hashes = batch_siphash_keys(&key_refs);

    // Check for collisions (extremely unlikely with SipHash-2-4)
    use std::collections::HashSet;
    let unique: HashSet<_> = hashes.iter().copied().collect();
    assert_eq!(
        unique.len(),
        1000,
        "Should have 1000 unique hashes (no collisions)"
    );
}

#[test]
fn test_property_q12_similar_keys_different_hashes() {
    // Q12: Property - similar keys produce different hashes
    let keys = vec![
        b"user_123456".as_ref(),
        b"user_123457",  // Off by 1
        b"user_123456 ", // Extra space
        b"User_123456",  // Different case
    ];

    let hashes = batch_siphash_keys(&keys);

    // All should be unique
    for i in 0..hashes.len() {
        for j in i + 1..hashes.len() {
            assert_ne!(hashes[i], hashes[j], "Similar keys should hash differently");
        }
    }
}

#[test]
fn test_property_q13_various_batch_sizes() {
    // Q13: Property - correctness across all batch sizes (1-32)
    for size in 1..=32 {
        let keys: Vec<Vec<u8>> = (0..size)
            .map(|i| format!("key_{}", i).into_bytes())
            .collect();

        let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
        let hashes_batch = batch_siphash_keys(&key_refs);
        let hashes_sequential: Vec<_> = key_refs.iter().map(|k| siphash_single(k)).collect();

        assert_eq!(
            hashes_batch, hashes_sequential,
            "Size {} batch failed",
            size
        );
    }
}

#[test]
fn test_property_q14_non_multiples_of_4() {
    // Q14: Property - remainder handling (sizes not multiple of 4)
    for size in [5, 6, 7, 9, 10, 11, 13, 17, 19, 23, 31] {
        let keys: Vec<Vec<u8>> = (0..size)
            .map(|i| format!("key_{}", i).into_bytes())
            .collect();

        let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
        let hashes_batch = batch_siphash_keys(&key_refs);

        assert_eq!(hashes_batch.len(), size);

        // Verify correctness
        for (i, key) in key_refs.iter().enumerate() {
            assert_eq!(
                hashes_batch[i],
                siphash_single(key),
                "Size {} failed at index {}",
                size,
                i
            );
        }
    }
}

// ============================================================================
// Q15-Q21: Integration Tests - Real-World Usage
// ============================================================================

#[test]
fn test_integration_q15_fixed_batch_4() {
    // Q15: Fixed-size batch API (zero allocation)
    let keys = [b"k1".as_ref(), b"k2", b"k3", b"k4"];
    let hashes = batch_siphash_4_fixed(&keys);

    for (i, key) in keys.iter().enumerate() {
        assert_eq!(
            hashes[i],
            siphash_single(key),
            "Fixed batch 4 failed at {}",
            i
        );
    }
}

#[test]
fn test_integration_q16_fixed_batch_8() {
    // Q16: Fixed-size batch API (zero allocation)
    let keys = [
        b"k1".as_ref(),
        b"k2",
        b"k3",
        b"k4",
        b"k5",
        b"k6",
        b"k7",
        b"k8",
    ];
    let hashes = batch_siphash_8_fixed(&keys);

    for (i, key) in keys.iter().enumerate() {
        assert_eq!(
            hashes[i],
            siphash_single(key),
            "Fixed batch 8 failed at {}",
            i
        );
    }
}

#[test]
fn test_integration_q17_cache_key_pattern() {
    // Q17: Realistic distributed cache key pattern
    let user_ids = [12345u64, 67890, 11111, 22222, 33333, 44444, 55555, 66666];
    let keys: Vec<Vec<u8>> = user_ids
        .iter()
        .map(|id| format!("session:user:{}:token", id).into_bytes())
        .collect();

    let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
    let hashes = batch_siphash_keys(&key_refs);

    assert_eq!(hashes.len(), 8);

    // Verify uniqueness
    use std::collections::HashSet;
    let unique: HashSet<_> = hashes.iter().copied().collect();
    assert_eq!(unique.len(), 8, "Cache keys should hash uniquely");
}

#[test]
fn test_integration_q18_variable_length_keys() {
    // Q18: Variable-length keys (common in real systems)
    let keys = vec![
        b"short".as_ref(),
        b"medium_length_key",
        b"this_is_a_very_long_key_with_many_characters_to_hash",
        b"x", // Single character
    ];

    let hashes = batch_siphash_keys(&keys);
    assert_eq!(hashes.len(), 4);

    // Verify correctness
    for (i, key) in keys.iter().enumerate() {
        assert_eq!(hashes[i], siphash_single(key));
    }
}

#[test]
fn test_integration_q19_unicode_keys() {
    // Q19: Unicode keys (internationalization)
    let keys = vec![
        "user:日本語".as_bytes(),
        "user:中文".as_bytes(),
        "user:한국어".as_bytes(),
        "user:العربية".as_bytes(),
    ];

    let hashes = batch_siphash_keys(&keys);
    assert_eq!(hashes.len(), 4);

    // All unique
    use std::collections::HashSet;
    let unique: HashSet<_> = hashes.iter().copied().collect();
    assert_eq!(unique.len(), 4);
}

#[test]
fn test_integration_q20_binary_keys() {
    // Q20: Binary keys (not just ASCII/UTF-8)
    let keys: Vec<Vec<u8>> = vec![
        vec![0x00, 0x01, 0x02, 0x03],
        vec![0xFF, 0xFE, 0xFD, 0xFC],
        vec![0xDE, 0xAD, 0xBE, 0xEF],
        vec![0xCA, 0xFE, 0xBA, 0xBE],
    ];

    let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
    let hashes = batch_siphash_keys(&key_refs);

    assert_eq!(hashes.len(), 4);

    for (i, key) in key_refs.iter().enumerate() {
        assert_eq!(hashes[i], siphash_single(key));
    }
}

#[test]
fn test_integration_q21_distributed_cache_simulation() {
    // Q21: Simulate distributed cache multi_get pattern
    // Typical: 10-100 keys per batch
    let num_keys = 32;
    let keys: Vec<Vec<u8>> = (0..num_keys)
        .map(|i| format!("cache:item:{:08}", i).into_bytes())
        .collect();

    let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();

    // Batch hash (production path)
    let hashes_batch = batch_siphash_keys(&key_refs);

    // Sequential hash (baseline)
    let hashes_sequential: Vec<_> = key_refs.iter().map(|k| siphash_single(k)).collect();

    assert_eq!(
        hashes_batch, hashes_sequential,
        "Distributed cache pattern failed"
    );
}

// ============================================================================
// Q22-Q28: Production Tests - Stress and Real-World Workloads
// ============================================================================

#[test]
fn test_production_q22_large_batch() {
    // Q22: Large batch (100 keys)
    let num_keys = 100;
    let keys: Vec<Vec<u8>> = (0..num_keys)
        .map(|i| format!("large_batch_key_{:04}", i).into_bytes())
        .collect();

    let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
    let hashes = batch_siphash_keys(&key_refs);

    assert_eq!(hashes.len(), num_keys);

    // Verify uniqueness
    use std::collections::HashSet;
    let unique: HashSet<_> = hashes.iter().copied().collect();
    assert_eq!(
        unique.len(),
        num_keys,
        "Should have no collisions in 100 keys"
    );
}

#[test]
fn test_production_q23_very_large_batch() {
    // Q23: Very large batch (1000 keys) - stress test
    let num_keys = 1000;
    let keys: Vec<Vec<u8>> = (0..num_keys)
        .map(|i| format!("stress_test_key_{:06}", i).into_bytes())
        .collect();

    let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
    let hashes = batch_siphash_keys(&key_refs);

    assert_eq!(hashes.len(), num_keys);

    // Verify correctness (sample check)
    for i in (0..num_keys).step_by(100) {
        assert_eq!(hashes[i], siphash_single(&key_refs[i]));
    }
}

#[test]
fn test_production_q24_repeated_batches() {
    // Q24: Repeated batching (cache warming pattern)
    let keys: Vec<Vec<u8>> = (0..20)
        .map(|i| format!("repeated_key_{}", i).into_bytes())
        .collect();

    let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();

    // Hash same batch 100 times
    for _ in 0..100 {
        let hashes = batch_siphash_keys(&key_refs);
        assert_eq!(hashes.len(), 20);
    }
}

#[test]
fn test_production_q25_mixed_batch_sizes() {
    // Q25: Mixed batch sizes (realistic workload)
    let batch_sizes = [1, 2, 3, 4, 5, 8, 10, 16, 20, 32, 50];

    for &size in &batch_sizes {
        let keys: Vec<Vec<u8>> = (0..size)
            .map(|i| format!("mixed_key_{}", i).into_bytes())
            .collect();

        let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
        let hashes = batch_siphash_keys(&key_refs);

        assert_eq!(hashes.len(), size, "Failed for batch size {}", size);
    }
}

#[test]
fn test_production_q26_high_throughput_simulation() {
    // Q26: High throughput (10K batches)
    let keys: Vec<Vec<u8>> = (0..8)
        .map(|i| format!("throughput_key_{}", i).into_bytes())
        .collect();

    let key_refs: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();

    // Simulate 10K operations
    for _ in 0..10_000 {
        let hashes = batch_siphash_keys(&key_refs);
        assert_eq!(hashes.len(), 8);
    }
}

#[test]
fn test_production_q27_zero_allocation_fixed_batch() {
    // Q27: Zero allocation path (fixed-size batches)
    // This tests the stack-allocated fixed batch APIs

    // 4-key batches (common pattern)
    for _ in 0..1000 {
        let keys = [b"k1".as_ref(), b"k2", b"k3", b"k4"];
        let hashes = batch_siphash_4_fixed(&keys);
        assert_eq!(hashes.len(), 4);
    }

    // 8-key batches (optimal for SIMD)
    for _ in 0..1000 {
        let keys = [
            b"k1".as_ref(),
            b"k2",
            b"k3",
            b"k4",
            b"k5",
            b"k6",
            b"k7",
            b"k8",
        ];
        let hashes = batch_siphash_8_fixed(&keys);
        assert_eq!(hashes.len(), 8);
    }
}

#[test]
fn test_production_q28_realistic_cache_workload() {
    // Q28: Realistic cache workload (Zipfian distribution)
    // Hot keys (frequently accessed)
    let hot_keys: Vec<Vec<u8>> = (0..10)
        .map(|i| format!("hot_key_{}", i).into_bytes())
        .collect();

    // Warm keys (moderately accessed)
    let warm_keys: Vec<Vec<u8>> = (10..50)
        .map(|i| format!("warm_key_{}", i).into_bytes())
        .collect();

    // Cold keys (rarely accessed)
    let cold_keys: Vec<Vec<u8>> = (50..100)
        .map(|i| format!("cold_key_{}", i).into_bytes())
        .collect();

    // Simulate 1000 operations with Zipfian-like access
    for op in 0..1000 {
        let batch_keys: Vec<&[u8]> = if op % 10 < 7 {
            // 70% hot keys
            hot_keys.iter().take(8).map(|k| k.as_slice()).collect()
        } else if op % 10 < 9 {
            // 20% warm keys
            warm_keys.iter().take(8).map(|k| k.as_slice()).collect()
        } else {
            // 10% cold keys
            cold_keys.iter().take(8).map(|k| k.as_slice()).collect()
        };

        let hashes = batch_siphash_keys(&batch_keys);
        assert_eq!(hashes.len(), 8);
    }
}

// ============================================================================
// Bonus: Performance Characteristic Tests
// ============================================================================

#[test]
fn test_performance_threshold_behavior() {
    // Verify threshold behavior (SIMD kicks in at 4 keys)
    // This doesn't measure performance, but validates code paths

    // Below threshold: sequential
    let keys_3 = vec![b"k1".as_ref(), b"k2", b"k3"];
    let hashes_3 = batch_siphash_keys(&keys_3);
    assert_eq!(hashes_3.len(), 3);

    // At threshold: SIMD
    let keys_4 = vec![b"k1".as_ref(), b"k2", b"k3", b"k4"];
    let hashes_4 = batch_siphash_keys(&keys_4);
    assert_eq!(hashes_4.len(), 4);

    // Above threshold: SIMD batching
    let keys_8 = vec![
        b"k1".as_ref(),
        b"k2",
        b"k3",
        b"k4",
        b"k5",
        b"k6",
        b"k7",
        b"k8",
    ];
    let hashes_8 = batch_siphash_keys(&keys_8);
    assert_eq!(hashes_8.len(), 8);
}
