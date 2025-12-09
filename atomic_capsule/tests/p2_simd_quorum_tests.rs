//! P2: SIMD Hash + Quorum Read Comprehensive Tests (T28 Framework)
//!
//! **Test Coverage**: 4-tier testing pyramid
//! - Unit: Individual function correctness (8 tests)
//! - Property: Invariants and edge cases (6 tests)
//! - Integration: End-to-end workflows (4 tests)
//! - Production: Stress and concurrency (2 tests)
//!
//! **Total**: 20+ comprehensive tests

// ============================================================================
// Unit Tests (8 tests) - Individual function correctness
// ============================================================================

#[cfg(feature = "simd-hashing")]
#[test]
fn test_simd_hash_8_keys_basic() {
    use atomic_capsule::hash::simd_hash_capsule::simd_hash_8_keys;

    let keys = [1u64, 2, 3, 4, 5, 6, 7, 8];
    let hashes = simd_hash_8_keys(&keys);

    // All hashes non-zero
    assert!(hashes.iter().all(|&h| h != 0));

    // All hashes unique (for simple sequential keys)
    let unique: std::collections::HashSet<_> = hashes.iter().collect();
    assert_eq!(unique.len(), 8);
}

#[cfg(feature = "simd-hashing")]
#[test]
fn test_simd_hash_deterministic() {
    use atomic_capsule::hash::simd_hash_capsule::SimdHashCapsule;

    let capsule = SimdHashCapsule::new();
    let keys = [42u64, 123, 456, 789, 1011, 1213, 1415, 1617];

    let hashes1 = capsule.hash_batch_8(&keys);
    let hashes2 = capsule.hash_batch_8(&keys);

    assert_eq!(hashes1, hashes2, "SIMD hashing must be deterministic");
}

#[cfg(feature = "simd-hashing")]
#[test]
fn test_scalar_hash_single() {
    use atomic_capsule::hash::simd_hash_capsule::scalar_hash_single;

    let key = 12345u64;
    let hash = scalar_hash_single(key);

    assert_ne!(hash, 0, "Hash should not be zero");
    assert_eq!(
        hash,
        scalar_hash_single(key),
        "Scalar hash must be deterministic"
    );
}

#[cfg(feature = "simd-hashing")]
#[test]
fn test_adaptive_batch_small() {
    use atomic_capsule::hash::simd_hash_capsule::SimdHashCapsule;

    let capsule = SimdHashCapsule::new();
    let keys = vec![1u64, 2, 3]; // <8 keys: scalar fallback

    let hashes = capsule.hash_batch_adaptive(&keys);

    assert_eq!(hashes.len(), 3);
    assert!(hashes.iter().all(|&h| h != 0));
}

#[test]
fn test_quorum_capsule_basic() {
    use atomic_capsule::network::quorum_read::QuorumReadCapsule;

    let capsule: QuorumReadCapsule<u64> = QuorumReadCapsule::new();

    // Setup generations
    capsule.set_generation(0, 10);
    capsule.set_generation(1, 20);
    capsule.set_generation(2, 15);

    // Select winner (highest generation)
    let (winner_idx, winner_gen) = capsule.select_winner();
    assert_eq!(winner_idx, 1, "Replica 1 has highest generation (20)");
    assert_eq!(winner_gen, 20);
}

#[test]
fn test_quorum_threshold() {
    use atomic_capsule::network::quorum_read::QuorumReadCapsule;

    let capsule: QuorumReadCapsule<u64> = QuorumReadCapsule::new();

    assert!(!capsule.has_quorum(), "0/3 replicas completed");

    capsule.mark_completed(0);
    assert!(!capsule.has_quorum(), "1/3 replicas completed");

    capsule.mark_completed(1);
    assert!(capsule.has_quorum(), "2/3 replicas = quorum!");

    capsule.mark_completed(2);
    assert!(capsule.has_quorum(), "3/3 replicas completed");
}

#[test]
fn test_quorum_failure_tracking() {
    use atomic_capsule::network::quorum_read::QuorumReadCapsule;

    let capsule: QuorumReadCapsule<u64> = QuorumReadCapsule::new();

    capsule.mark_failed(1);
    assert_eq!(capsule.count_failed(), 1);

    capsule.mark_failed(2);
    assert_eq!(capsule.count_failed(), 2);

    capsule.mark_completed(0);
    assert_eq!(capsule.count_completed(), 1);
}

#[test]
fn test_quorum_reset() {
    use atomic_capsule::network::quorum_read::QuorumReadCapsule;

    let capsule: QuorumReadCapsule<u64> = QuorumReadCapsule::new();

    capsule.mark_completed(0);
    capsule.mark_completed(1);
    capsule.mark_failed(2);
    capsule.set_generation(0, 42);

    capsule.reset();

    assert_eq!(capsule.count_completed(), 0, "Completed count reset");
    assert_eq!(capsule.count_failed(), 0, "Failed count reset");
    assert_eq!(capsule.get_winner(), (0, 0), "Winner reset");
}

// ============================================================================
// Property Tests (6 tests) - Invariants and edge cases
// ============================================================================

#[cfg(feature = "simd-hashing")]
#[test]
fn test_simd_hash_collision_resistance() {
    use atomic_capsule::hash::simd_hash_capsule::SimdHashCapsule;

    let capsule = SimdHashCapsule::new();
    let keys1 = [1u64, 2, 3, 4, 5, 6, 7, 8];
    let keys2 = [9u64, 10, 11, 12, 13, 14, 15, 16];

    let hashes1 = capsule.hash_batch_8(&keys1);
    let hashes2 = capsule.hash_batch_8(&keys2);

    // No collisions between different key sets
    let set1: std::collections::HashSet<_> = hashes1.iter().collect();
    let set2: std::collections::HashSet<_> = hashes2.iter().collect();

    assert_eq!(
        set1.intersection(&set2).count(),
        0,
        "Different keys should produce different hashes"
    );
}

#[cfg(feature = "simd-hashing")]
#[test]
fn test_adaptive_batch_threshold() {
    use atomic_capsule::hash::simd_hash_capsule::SimdHashCapsule;

    let capsule = SimdHashCapsule::new();

    // Test threshold: 7 keys (scalar), 8 keys (SIMD)
    let keys_7: Vec<u64> = (0..7).collect();
    let keys_8: Vec<u64> = (0..8).collect();

    let hashes_7 = capsule.hash_batch_adaptive(&keys_7);
    let hashes_8 = capsule.hash_batch_adaptive(&keys_8);

    assert_eq!(hashes_7.len(), 7);
    assert_eq!(hashes_8.len(), 8);
}

#[cfg(feature = "simd-hashing")]
#[test]
fn test_adaptive_batch_large() {
    use atomic_capsule::hash::simd_hash_capsule::SimdHashCapsule;

    let capsule = SimdHashCapsule::new();
    let keys: Vec<u64> = (0..100).collect(); // 100 keys: 12 SIMD batches + 4 scalar

    let hashes = capsule.hash_batch_adaptive(&keys);

    assert_eq!(hashes.len(), 100);
    assert!(hashes.iter().all(|&h| h != 0));

    // All unique
    let unique: std::collections::HashSet<_> = hashes.iter().collect();
    assert_eq!(unique.len(), 100);
}

#[test]
fn test_quorum_concurrent_updates() {
    use atomic_capsule::network::quorum_read::QuorumReadCapsule;

    let capsule: QuorumReadCapsule<u64> = QuorumReadCapsule::new();

    // Simulate concurrent updates from 3 replicas
    capsule.set_generation(0, 5);
    capsule.set_generation(1, 10);
    capsule.set_generation(2, 8);

    capsule.mark_completed(0);
    capsule.mark_completed(1);

    assert!(capsule.has_quorum());

    let (winner_idx, winner_gen) = capsule.select_winner();
    assert_eq!(winner_idx, 1);
    assert_eq!(winner_gen, 10);
}

#[test]
fn test_quorum_partial_failure() {
    use atomic_capsule::network::quorum_read::QuorumReadCapsule;

    let capsule: QuorumReadCapsule<u64> = QuorumReadCapsule::new();

    // Replica 0: success (gen 100)
    capsule.set_generation(0, 100);
    capsule.mark_completed(0);

    // Replica 1: failed
    capsule.mark_failed(1);

    // Replica 2: success (gen 200)
    capsule.set_generation(2, 200);
    capsule.mark_completed(2);

    // Still have quorum (2/3 completed)
    assert!(capsule.has_quorum());
    assert_eq!(capsule.count_failed(), 1);

    // Winner is replica 2 (highest gen)
    let (winner_idx, winner_gen) = capsule.select_winner();
    assert_eq!(winner_idx, 2);
    assert_eq!(winner_gen, 200);
}

#[test]
fn test_quorum_all_failed() {
    use atomic_capsule::network::quorum_read::QuorumReadCapsule;

    let capsule: QuorumReadCapsule<u64> = QuorumReadCapsule::new();

    capsule.mark_failed(0);
    capsule.mark_failed(1);
    capsule.mark_failed(2);

    assert!(!capsule.has_quorum());
    assert_eq!(capsule.count_failed(), 3);
    assert_eq!(capsule.count_completed(), 0);
}

// ============================================================================
// Integration Tests (4 tests) - End-to-end workflows
// ============================================================================

#[cfg(feature = "simd-hashing")]
#[test]
fn test_simd_hash_workflow_distributed_cache() {
    use atomic_capsule::hash::simd_hash_capsule::SimdHashCapsule;

    // Simulate distributed cache scenario: hash 64 cache keys
    let capsule = SimdHashCapsule::new();
    let cache_keys: Vec<u64> = (1000..1064).collect();

    let hashes = capsule.hash_batch_adaptive(&cache_keys);

    // Verify all keys hashed
    assert_eq!(hashes.len(), 64);

    // Verify uniqueness (no collisions)
    let unique: std::collections::HashSet<_> = hashes.iter().collect();
    assert_eq!(unique.len(), 64);

    // Verify determinism (re-hash same keys)
    let hashes2 = capsule.hash_batch_adaptive(&cache_keys);
    assert_eq!(hashes, hashes2);
}

#[test]
fn test_quorum_read_workflow_full() {
    use atomic_capsule::network::quorum_read::QuorumReadCapsule;

    let capsule: QuorumReadCapsule<String> = QuorumReadCapsule::new();

    // Phase 1: Setup replicas
    capsule.set_generation(0, 100);
    capsule.set_generation(1, 200);
    capsule.set_generation(2, 150);

    // Phase 2: Simulate parallel reads
    capsule.mark_completed(0);
    capsule.mark_completed(1);

    // Phase 3: Check quorum
    assert!(capsule.has_quorum());

    // Phase 4: Select winner
    let (winner_idx, winner_gen) = capsule.select_winner();
    assert_eq!(winner_idx, 1);
    assert_eq!(winner_gen, 200);

    // Phase 5: Reset for next read
    capsule.reset();
    assert_eq!(capsule.count_completed(), 0);
}

#[test]
fn test_quorum_read_workflow_with_retry() {
    use atomic_capsule::network::quorum_read::QuorumReadCapsule;

    let capsule: QuorumReadCapsule<u64> = QuorumReadCapsule::new();

    // Attempt 1: Only 1 replica responds
    capsule.set_generation(0, 100);
    capsule.mark_completed(0);
    assert!(!capsule.has_quorum());

    // Attempt 2: Second replica responds (quorum reached)
    capsule.set_generation(1, 200);
    capsule.mark_completed(1);
    assert!(capsule.has_quorum());

    let (winner_idx, winner_gen) = capsule.select_winner();
    assert_eq!(winner_idx, 1);
    assert_eq!(winner_gen, 200);
}

#[test]
fn test_quorum_read_workflow_stale_replica() {
    use atomic_capsule::network::quorum_read::QuorumReadCapsule;

    let capsule: QuorumReadCapsule<u64> = QuorumReadCapsule::new();

    // Replica 0: fresh (gen 300)
    capsule.set_generation(0, 300);
    capsule.mark_completed(0);

    // Replica 1: stale (gen 100)
    capsule.set_generation(1, 100);
    capsule.mark_completed(1);

    // Replica 2: fresh (gen 290)
    capsule.set_generation(2, 290);
    capsule.mark_completed(2);

    // Quorum reached
    assert!(capsule.has_quorum());

    // Winner is freshest replica
    let (winner_idx, winner_gen) = capsule.select_winner();
    assert_eq!(winner_idx, 0);
    assert_eq!(winner_gen, 300);
}

// ============================================================================
// Production Tests (2 tests) - Stress and concurrency
// ============================================================================

#[cfg(feature = "simd-hashing")]
#[test]
fn test_simd_hash_stress_large_batch() {
    use atomic_capsule::hash::simd_hash_capsule::SimdHashCapsule;

    let capsule = SimdHashCapsule::new();
    let large_keys: Vec<u64> = (0..10000).collect();

    let hashes = capsule.hash_batch_adaptive(&large_keys);

    assert_eq!(hashes.len(), 10000);
    assert!(hashes.iter().all(|&h| h != 0));

    // Verify collision rate is low
    let unique: std::collections::HashSet<_> = hashes.iter().collect();
    let collision_rate = 1.0 - (unique.len() as f64 / hashes.len() as f64);
    assert!(collision_rate < 0.01, "Collision rate should be <1%");
}

#[test]
fn test_quorum_read_stress_many_rounds() {
    use atomic_capsule::network::quorum_read::QuorumReadCapsule;

    let capsule: QuorumReadCapsule<u64> = QuorumReadCapsule::new();

    // Stress test: 1000 rounds of quorum reads
    for round in 0..1000 {
        capsule.reset();

        // Simulate 3 replica reads
        capsule.set_generation(0, round * 3);
        capsule.set_generation(1, round * 3 + 1);
        capsule.set_generation(2, round * 3 + 2);

        capsule.mark_completed(0);
        capsule.mark_completed(1);
        capsule.mark_completed(2);

        assert!(capsule.has_quorum());

        let (winner_idx, winner_gen) = capsule.select_winner();
        assert_eq!(winner_idx, 2); // Highest generation
        assert_eq!(winner_gen, round * 3 + 2);
    }
}
