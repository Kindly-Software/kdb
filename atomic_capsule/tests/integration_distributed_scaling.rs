//! # Integration Test 4: Dynamic Scaling
//!
//! **T28 Tier 3**: End-to-end dynamic scaling with consistent hashing rebalance
//! **I20 Framework**: Capsule-to-capsule integration (simplified)
//!
//! ## Test Objective
//!
//! Validate dynamic cluster scaling:
//! 1. Add 4th shard to 3-shard cluster
//! 2. Consistent hashing rebalances keys
//! 3. Only K/N keys migrate (<30% redistribution)
//! 4. Rebalancing completes in <2 seconds
//!
//! ## I20 Integration Analysis (I20-Capsule Simplified)
//!
//! **Phase 1 (Q1-Q5): Scope**
//! - Q1: Components = ConsistentHashRing + NetworkShardCapsule (4 shards)
//! - Q2: Problem = Validate minimal key redistribution during scale-up
//! - Q3: Why integrate = Test horizontal scaling with minimal disruption
//! - Q4: Success = <30% keys migrate, rebalancing <2s, deterministic routing
//! - Q5: Fallback = Static cluster size (no scaling)
//!
//! **Phase 2 (Q6-Q10): Compatibility** (SIMPLIFIED - All capsules)
//! - Q6: Architecture = All T8 capsules → Automatically compatible ✅
//! - Q7: Performance = Hash lookup <10ns, shard add <50µs → Compatible ✅
//! - Q8: Error handling = Both use Result<T,E> → Compatible ✅
//! - Q9: Concurrency = Lockfree atomic → Compatible ✅
//! - Q10: Boundaries = Ring modification boundary (atomic vnodes)
//!
//! **Phase 3 (Q11-Q15): Failure Modes** (SIMPLIFIED - Lockfree)
//! - Q11: What breaks = Concurrent add/remove, ring inconsistency
//! - Q12: Inputs = Add during routing → Consistent hashing handles
//! - Q13: States = 3-shard → 4-shard → stable (monotonic growth)
//! - Q14: Race/Deadlock = SKIP (lockfree capsules) ✅
//! - Q15: Escape Hatches = Git revert (deterministic capsules)
//!
//! **Phase 4 (Q16-Q20): Validation** (DEPLOY 100% if tests pass)
//! - Q16: Test strategy = Add shard, measure redistribution, verify determinism
//! - Q17: Properties = Minimal migration (K/N), deterministic routing
//! - Q18: Failure injection = Add during load, concurrent add/remove
//! - Q19: Deployment = 100% immediate (deterministic capsules) ✅
//! - Q20: Rollback = Git revert <5 minutes (unlikely needed) ✅
//!
//! ## B32 Performance Budget
//!
//! - Add shard: <50µs (150 vnodes insert + sort)
//! - Key redistribution: <30% of total keys
//! - Rebalancing time: <2 seconds (for 1000 keys)
//! - Hash lookup after add: <10ns (same as before)
//!
//! ## ASSUM Safety
//!
//! - `#ASSUME_CONSISTENT_HASH_MINIMAL_MIGRATION`: Only K/N keys migrate
//! - `#ASSUME_VNODE_DETERMINISM`: Same vnode count = consistent distribution
//! - `#VERIFY_LOCKFREE`: All operations use atomics (no mutex)
//! - `#VERIFY_REDISTRIBUTION_BOUND`: Migration <30% of total keys

use atomic_capsule::network::{ConsistentHashRing, NetworkShardCapsule};
use std::collections::HashMap;
use std::time::Instant;

/// Test: Dynamic scaling with consistent hashing rebalance
///
/// Setup: 3-shard cluster with 1000 keys
/// Action: Add new shard (4th shard)
/// Verify: Consistent hashing rebalances keys
/// Assert: Only K/N keys migrate (<30% redistribution)
/// Latency: <2 seconds rebalancing
#[test]
fn test_dynamic_scaling() {
    let start = Instant::now();

    // ========================================================================
    // Setup: 3-shard cluster with 1000 keys
    // ========================================================================

    let mut ring = ConsistentHashRing::new(150); // 150 vnodes per shard

    // Add initial 3 shards
    ring.add_shard(1);
    ring.add_shard(2);
    ring.add_shard(3);

    let shard1 = NetworkShardCapsule::new(1);
    let shard2 = NetworkShardCapsule::new(2);
    let shard3 = NetworkShardCapsule::new(3);

    shard1.update_heartbeat();
    shard2.update_heartbeat();
    shard3.update_heartbeat();

    // Distribute 1000 keys
    let total_keys = 1000;
    let mut original_routing: HashMap<String, u64> = HashMap::new();

    for i in 0..total_keys {
        let key = format!("key_{}", i);
        let shard_id = ring.get_shard(key.as_bytes()).unwrap_or(0);
        original_routing.insert(key, shard_id);
    }

    println!("\n=== Initial 3-Shard Distribution ===");
    let mut initial_counts: HashMap<u64, usize> = HashMap::new();
    initial_counts.insert(1, 0);
    initial_counts.insert(2, 0);
    initial_counts.insert(3, 0);

    for shard_id in original_routing.values() {
        *initial_counts.get_mut(shard_id).unwrap() += 1;
    }

    for (shard_id, count) in &initial_counts {
        println!(
            "Shard {}: {} keys ({:.1}%)",
            shard_id,
            count,
            (*count as f64 / total_keys as f64) * 100.0
        );
    }

    // ========================================================================
    // Action: Add 4th shard
    // ========================================================================

    println!("\n=== Adding 4th Shard ===");

    let add_start = Instant::now();

    ring.add_shard(4); // Add new shard

    let add_elapsed = add_start.elapsed();
    println!("Shard added in {:?}", add_elapsed);

    assert!(
        add_elapsed.as_micros() < 100,
        "Shard add too slow: {:?} (expected <100µs)",
        add_elapsed
    );

    let shard4 = NetworkShardCapsule::new(4);
    shard4.update_heartbeat();

    // ========================================================================
    // Verify: Consistent hashing rebalances keys
    // ========================================================================

    let rebalance_start = Instant::now();

    let mut new_routing: HashMap<String, u64> = HashMap::new();
    let mut migrated_keys = 0;

    for i in 0..total_keys {
        let key = format!("key_{}", i);
        let new_shard_id = ring.get_shard(key.as_bytes()).unwrap_or(0);
        new_routing.insert(key.clone(), new_shard_id);

        // Check if key migrated
        let original_shard_id = original_routing.get(&key).unwrap();
        if *original_shard_id != new_shard_id {
            migrated_keys += 1;
        }
    }

    let rebalance_elapsed = rebalance_start.elapsed();

    println!("\n=== Rebalancing Results ===");
    println!("Rebalancing completed in {:?}", rebalance_elapsed);
    println!(
        "Keys migrated: {} / {} ({:.1}%)",
        migrated_keys,
        total_keys,
        (migrated_keys as f64 / total_keys as f64) * 100.0
    );

    // ========================================================================
    // Assert: Only K/N keys migrate (<30% redistribution)
    // ========================================================================

    let migration_percent = (migrated_keys as f64 / total_keys as f64) * 100.0;

    // Theoretical migration: K/N = 1000/4 = 250 keys (25%)
    // Allow up to 30% due to hash distribution variance
    assert!(
        migration_percent <= 30.0,
        "Too many keys migrated: {:.1}% (expected ≤30%)",
        migration_percent
    );

    println!(
        "Migration within tolerance: {:.1}% ≤ 30%",
        migration_percent
    );

    // ========================================================================
    // Verify: New distribution is balanced
    // ========================================================================

    let mut new_counts: HashMap<u64, usize> = HashMap::new();
    new_counts.insert(1, 0);
    new_counts.insert(2, 0);
    new_counts.insert(3, 0);
    new_counts.insert(4, 0);

    for shard_id in new_routing.values() {
        *new_counts.get_mut(shard_id).unwrap() += 1;
    }

    println!("\n=== New 4-Shard Distribution ===");
    for (shard_id, count) in &new_counts {
        println!(
            "Shard {}: {} keys ({:.1}%)",
            shard_id,
            count,
            (*count as f64 / total_keys as f64) * 100.0
        );
    }

    // Expected: ~250 keys per shard (1000/4)
    let expected_per_shard = total_keys / 4;
    let tolerance = (expected_per_shard as f64 * 0.30) as usize; // ±30%

    for (shard_id, count) in &new_counts {
        assert!(*count > 0, "Shard {} should have some keys", shard_id);
        assert!(
            *count >= expected_per_shard.saturating_sub(tolerance),
            "Shard {} underloaded: {} (expected ≥{})",
            shard_id,
            count,
            expected_per_shard.saturating_sub(tolerance)
        );
        assert!(
            *count <= expected_per_shard + tolerance,
            "Shard {} overloaded: {} (expected ≤{})",
            shard_id,
            count,
            expected_per_shard + tolerance
        );
    }

    // ========================================================================
    // Performance: Total rebalancing <2 seconds
    // ========================================================================

    let total_elapsed = start.elapsed();
    println!("\nTotal scaling time: {:?}", total_elapsed);
    println!("  - Add shard: {:?}", add_elapsed);
    println!("  - Rebalance: {:?}", rebalance_elapsed);

    assert!(
        total_elapsed.as_secs() < 2,
        "Scaling too slow: {:?} (expected <2s)",
        total_elapsed
    );

    println!("\n✅ Dynamic scaling test PASSED");
    println!("   - Shard added in {:?}", add_elapsed);
    println!("   - {:.1}% keys migrated (≤30%)", migration_percent);
    println!("   - New distribution balanced (±30%)");
    println!("   - Total time {:?} (<2s)", total_elapsed);
}

/// Test: Deterministic routing after scaling
#[test]
fn test_deterministic_routing_after_scaling() {
    let mut ring = ConsistentHashRing::new(100);

    ring.add_shard(1);
    ring.add_shard(2);

    // Route 10 keys before scaling
    let mut before_routing = vec![];
    for i in 0..10 {
        let key = format!("key_{}", i);
        before_routing.push(ring.get_shard(key.as_bytes()).unwrap_or(0));
    }

    // Add 3rd shard
    ring.add_shard(3);

    // Route same 10 keys after scaling
    let mut after_routing = vec![];
    for i in 0..10 {
        let key = format!("key_{}", i);
        after_routing.push(ring.get_shard(key.as_bytes()).unwrap_or(0));
    }

    // Verify routing is still deterministic (same key → same shard multiple times)
    for i in 0..10 {
        let key = format!("key_{}", i);
        let shard1 = ring.get_shard(key.as_bytes()).unwrap_or(0);
        let shard2 = ring.get_shard(key.as_bytes()).unwrap_or(0);
        assert_eq!(
            shard1, shard2,
            "Routing should be deterministic after scaling"
        );
    }
}

/// Test: Scale down (remove shard)
#[test]
fn test_scale_down() {
    let mut ring = ConsistentHashRing::new(100);

    ring.add_shard(1);
    ring.add_shard(2);
    ring.add_shard(3);

    let total_keys = 1000;
    let mut original_routing: HashMap<String, u64> = HashMap::new();

    for i in 0..total_keys {
        let key = format!("key_{}", i);
        let shard_id = ring.get_shard(key.as_bytes()).unwrap_or(0);
        original_routing.insert(key, shard_id);
    }

    // Remove shard 2
    ring.remove_shard(2);

    // Verify keys no longer route to shard 2
    for i in 0..total_keys {
        let key = format!("key_{}", i);
        let shard_id = ring.get_shard(key.as_bytes()).unwrap_or(0);
        assert!(
            shard_id == 1 || shard_id == 3,
            "Keys should not route to removed shard 2"
        );
    }

    // Calculate migration
    let mut migrated = 0;
    for i in 0..total_keys {
        let key = format!("key_{}", i);
        let original_shard = original_routing.get(&key).unwrap();
        let new_shard = ring.get_shard(key.as_bytes()).unwrap_or(0);

        if *original_shard != new_shard {
            migrated += 1;
        }
    }

    let migration_percent = (migrated as f64 / total_keys as f64) * 100.0;

    println!("\n=== Scale Down Results ===");
    println!(
        "Keys migrated: {} / {} ({:.1}%)",
        migrated, total_keys, migration_percent
    );

    // Migration should be ~K/N = 1000/3 = 33.3%
    // Allow up to 45% due to hash distribution
    assert!(
        migration_percent <= 45.0,
        "Too many keys migrated during scale down: {:.1}%",
        migration_percent
    );
}

/// Test: Multiple sequential scale operations
#[test]
fn test_multiple_scale_operations() {
    let mut ring = ConsistentHashRing::new(100);

    // Start with 2 shards
    ring.add_shard(1);
    ring.add_shard(2);

    let total_keys = 500;

    // Scale to 3 shards
    ring.add_shard(3);

    // Verify keys distributed across 3 shards
    let mut counts_3: HashMap<u64, usize> = HashMap::new();
    for i in 0..total_keys {
        let key = format!("key_{}", i);
        let shard_id = ring.get_shard(key.as_bytes()).unwrap_or(0);
        *counts_3.entry(shard_id).or_insert(0) += 1;
    }

    assert_eq!(counts_3.len(), 3, "Should have 3 shards");

    // Scale to 4 shards
    ring.add_shard(4);

    let mut counts_4: HashMap<u64, usize> = HashMap::new();
    for i in 0..total_keys {
        let key = format!("key_{}", i);
        let shard_id = ring.get_shard(key.as_bytes()).unwrap_or(0);
        *counts_4.entry(shard_id).or_insert(0) += 1;
    }

    assert_eq!(counts_4.len(), 4, "Should have 4 shards");

    // Scale down to 3 shards
    ring.remove_shard(1);

    let mut counts_final: HashMap<u64, usize> = HashMap::new();
    for i in 0..total_keys {
        let key = format!("key_{}", i);
        let shard_id = ring.get_shard(key.as_bytes()).unwrap_or(0);
        *counts_final.entry(shard_id).or_insert(0) += 1;
    }

    assert_eq!(
        counts_final.len(),
        3,
        "Should have 3 shards after scale down"
    );
    assert!(!counts_final.contains_key(&1), "Shard 1 should be removed");
}

/// Test: Vnode distribution uniformity
#[test]
fn test_vnode_distribution_uniformity() {
    let mut ring = ConsistentHashRing::new(150);

    ring.add_shard(1);
    ring.add_shard(2);
    ring.add_shard(3);

    // Sample 10000 keys to test distribution
    let total_keys = 10000;
    let mut counts: HashMap<u64, usize> = HashMap::new();
    counts.insert(1, 0);
    counts.insert(2, 0);
    counts.insert(3, 0);

    for i in 0..total_keys {
        let key = format!("key_{}", i);
        let shard_id = ring.get_shard(key.as_bytes()).unwrap_or(0);
        *counts.get_mut(&shard_id).unwrap() += 1;
    }

    let expected_per_shard = total_keys / 3;
    let tolerance = (expected_per_shard as f64 * 0.15) as usize; // ±15%

    println!("\n=== Vnode Distribution Uniformity ===");
    println!("Total keys: {}", total_keys);
    println!(
        "Expected per shard: {} (±{})",
        expected_per_shard, tolerance
    );

    for (shard_id, count) in &counts {
        let deviation = (*count as i64 - expected_per_shard as i64).abs();
        let deviation_percent = (deviation as f64 / expected_per_shard as f64) * 100.0;

        println!(
            "Shard {}: {} keys ({:.1}% deviation)",
            shard_id, count, deviation_percent
        );

        assert!(
            *count >= expected_per_shard.saturating_sub(tolerance),
            "Shard {} underloaded: {}",
            shard_id,
            count
        );
        assert!(
            *count <= expected_per_shard + tolerance,
            "Shard {} overloaded: {}",
            shard_id,
            count
        );
    }
}
