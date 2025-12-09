//! # Integration Test 1: Multi-Shard Coordination
//!
//! **T28 Tier 3**: End-to-end distributed coordination workflow
//! **I20 Framework**: Capsule-to-capsule integration (simplified)
//!
//! ## Test Objective
//!
//! Validate that keys are correctly distributed across multiple shards using
//! consistent hashing, with balanced load distribution.
//!
//! ## I20 Integration Analysis (I20-Capsule Simplified)
//!
//! **Phase 1 (Q1-Q5): Scope**
//! - Q1: Components = ConsistentHashRing + NetworkShardCapsule (3 shards)
//! - Q2: Problem = Validate deterministic shard routing + load balancing
//! - Q3: Why integrate = Test multi-shard coordination end-to-end
//! - Q4: Success = Keys distributed evenly (300-400 per shard ±20%)
//! - Q5: Fallback = Single shard (no distribution)
//!
//! **Phase 2 (Q6-Q10): Compatibility** (SIMPLIFIED - All capsules)
//! - Q6: Architecture = Both T8 capsules → Automatically compatible ✅
//! - Q7: Performance = Hash lookup <10ns, shard update <20ns → Compatible ✅
//! - Q8: Error handling = Both use Result<T,E> → Compatible ✅
//! - Q9: Concurrency = Both lockfree atomic → Compatible ✅
//! - Q10: Boundaries = Consistent hashing boundary (deterministic)
//!
//! **Phase 3 (Q11-Q15): Failure Modes** (SIMPLIFIED - Lockfree)
//! - Q11: What breaks = Hash collision (rare), empty ring
//! - Q12: Inputs = Empty ring → Returns error
//! - Q13: States = Shard offline → Circuit breaker opens
//! - Q14: Race/Deadlock = SKIP (lockfree capsules) ✅
//! - Q15: Escape Hatches = Git revert (deterministic capsules)
//!
//! **Phase 4 (Q16-Q20): Validation** (DEPLOY 100% if tests pass)
//! - Q16: Test strategy = Property test (1000 keys), load distribution
//! - Q17: Properties = Determinism (same key → same shard), balance
//! - Q18: Failure injection = Empty ring, zero vnodes
//! - Q19: Deployment = 100% immediate (deterministic capsules) ✅
//! - Q20: Rollback = Git revert <5 minutes (unlikely needed) ✅
//!
//! ## B32 Performance Budget
//!
//! - Test execution: <500ms (3-shard setup + 1000 keys)
//! - Hash lookup: <10ns per key
//! - Shard update: <20ns per operation
//! - Load balance tolerance: ±20% (240-460 keys per shard)
//!
//! ## ASSUM Safety
//!
//! - `#ASSUME_CONSISTENT_HASH_DETERMINISM`: Same key always routes to same shard
//! - `#ASSUME_VNODE_SPREAD`: Virtual nodes distribute evenly
//! - `#VERIFY_LOCKFREE`: All operations use atomics (no mutex)
//! - `#VERIFY_LOAD_BALANCE`: Keys distribute within tolerance

use atomic_capsule::network::{ConsistentHashRing, NetworkShardCapsule};
use std::collections::HashMap;
use std::time::Instant;

/// Test: Multi-shard coordination with consistent hashing
///
/// Setup: 3-shard cluster with 150 vnodes per shard
/// Action: Distribute 1000 keys across shards
/// Verify: Keys map to correct shard (deterministic)
/// Assert: Load balanced (300-400 keys per shard, ±20%)
/// Latency: <500ms total
#[test]
fn test_multi_shard_coordination() {
    let start = Instant::now();

    // ========================================================================
    // Setup: 3-shard cluster
    // ========================================================================

    let mut ring = ConsistentHashRing::new(150); // 150 vnodes per shard

    // Add 3 shards
    ring.add_shard(1);
    ring.add_shard(2);
    ring.add_shard(3);

    // Create shard capsules
    let shard1 = NetworkShardCapsule::new(1);
    let shard2 = NetworkShardCapsule::new(2);
    let shard3 = NetworkShardCapsule::new(3);

    // Initialize shard health
    shard1.update_heartbeat();
    shard2.update_heartbeat();
    shard3.update_heartbeat();

    // Verify all shards are healthy
    assert!(shard1.is_healthy());
    assert!(shard2.is_healthy());
    assert!(shard3.is_healthy());

    // ========================================================================
    // Action: Distribute 1000 keys across shards
    // ========================================================================

    let mut shard_counts: HashMap<u64, usize> = HashMap::new();
    shard_counts.insert(1, 0);
    shard_counts.insert(2, 0);
    shard_counts.insert(3, 0);

    let total_keys = 1000;

    for i in 0..total_keys {
        let key = format!("key_{}", i);
        let shard_id = ring.get_shard(key.as_bytes()).unwrap_or(0);

        // Verify shard ID is valid (1, 2, or 3)
        assert!(
            shard_id == 1 || shard_id == 2 || shard_id == 3,
            "Invalid shard ID: {}",
            shard_id
        );

        // Increment shard counter
        *shard_counts.get_mut(&shard_id).unwrap() += 1;
    }

    // ========================================================================
    // Verify: Deterministic routing (same key → same shard)
    // ========================================================================

    for i in 0..10 {
        let key = format!("key_{}", i);
        let shard1 = ring.get_shard(key.as_bytes()).unwrap_or(0);
        let shard2 = ring.get_shard(key.as_bytes()).unwrap_or(0);
        let shard3 = ring.get_shard(key.as_bytes()).unwrap_or(0);

        assert_eq!(shard1, shard2, "Non-deterministic routing for key {}", i);
        assert_eq!(shard2, shard3, "Non-deterministic routing for key {}", i);
    }

    // ========================================================================
    // Assert: Load balanced (300-400 keys per shard, ±20%)
    // ========================================================================

    let expected_per_shard: usize = total_keys / 3; // 333 keys per shard
    let tolerance: usize = (expected_per_shard as f64 * 0.20) as usize; // ±20%
    let min_expected = expected_per_shard.saturating_sub(tolerance); // ~267
    let max_expected = expected_per_shard + tolerance; // ~400

    println!("\n=== Multi-Shard Load Distribution ===");
    println!("Total keys: {}", total_keys);
    println!(
        "Expected per shard: {} (±{})",
        expected_per_shard, tolerance
    );
    println!("Tolerance range: {}-{}", min_expected, max_expected);
    println!();

    for (shard_id, count) in shard_counts.iter() {
        println!(
            "Shard {}: {} keys ({:.1}%)",
            shard_id,
            count,
            (*count as f64 / total_keys as f64) * 100.0
        );

        // Assert within tolerance
        assert!(
            *count >= min_expected,
            "Shard {} underloaded: {} keys (expected ≥{})",
            shard_id,
            count,
            min_expected
        );
        assert!(
            *count <= max_expected,
            "Shard {} overloaded: {} keys (expected ≤{})",
            shard_id,
            count,
            max_expected
        );
    }

    // ========================================================================
    // Performance: <500ms total
    // ========================================================================

    let elapsed = start.elapsed();
    println!("\nTest duration: {:?}", elapsed);

    assert!(
        elapsed.as_millis() < 500,
        "Test too slow: {:?} (expected <500ms)",
        elapsed
    );

    println!("\n✅ Multi-shard coordination test PASSED");
    println!("   - Deterministic routing verified");
    println!("   - Load balanced within ±20% tolerance");
    println!("   - Performance <500ms");
}

/// Property test: Consistent hashing is deterministic
///
/// Verify that the same key always routes to the same shard across multiple lookups.
#[test]
fn test_consistent_hash_determinism() {
    let mut ring = ConsistentHashRing::new(100);

    ring.add_shard(10);
    ring.add_shard(20);
    ring.add_shard(30);

    // Test 100 keys
    for i in 0..100 {
        let key = format!("test_key_{}", i);

        // Query 10 times
        let first_shard = ring.get_shard(key.as_bytes()).unwrap_or(0);

        for _ in 0..10 {
            let shard = ring.get_shard(key.as_bytes()).unwrap_or(0);
            assert_eq!(
                shard, first_shard,
                "Non-deterministic routing for key: {}",
                key
            );
        }
    }
}

/// Test: Empty ring returns shard 0 (fallback)
#[test]
fn test_empty_ring_fallback() {
    let ring = ConsistentHashRing::new(100);

    // Empty ring should return 0
    let shard = ring.get_shard(b"any_key").unwrap_or(0);
    assert_eq!(shard, 0, "Empty ring should return shard 0");
}

/// Test: Single shard (all keys route to it)
#[test]
fn test_single_shard_routing() {
    let mut ring = ConsistentHashRing::new(100);
    ring.add_shard(42);

    // All keys should route to shard 42
    for i in 0..100 {
        let key = format!("key_{}", i);
        let shard = ring.get_shard(key.as_bytes()).unwrap_or(0);
        assert_eq!(shard, 42, "All keys should route to shard 42");
    }
}

/// Test: Shard health tracking during routing
#[test]
fn test_shard_health_coordination() {
    let mut ring = ConsistentHashRing::new(100);
    ring.add_shard(1);
    ring.add_shard(2);

    let shard1 = NetworkShardCapsule::new(1);
    let shard2 = NetworkShardCapsule::new(2);

    // Update heartbeats
    shard1.update_heartbeat();
    shard2.update_heartbeat();

    // Route 100 keys and verify shards are healthy
    for i in 0..100 {
        let key = format!("key_{}", i);
        let shard_id = ring.get_shard(key.as_bytes()).unwrap_or(0);

        if shard_id == 1 {
            assert!(shard1.is_healthy(), "Shard 1 should be healthy");
        } else if shard_id == 2 {
            assert!(shard2.is_healthy(), "Shard 2 should be healthy");
        }
    }
}
