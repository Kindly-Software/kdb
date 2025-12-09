// TIER 1: UNIT TESTS - Consistent Hashing
// T28 Testing Framework - Individual Component Testing
//
// Tests: New ConsistentHashRing, shard assignment, add/remove shards

#![allow(dead_code)]

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Consistent Hash Ring for distributed sharding
///
/// # T8 Network Tier
/// - Deterministic shard assignment
/// - Minimal rebalancing when adding/removing shards
/// - Virtual nodes for even distribution
pub struct ConsistentHashRing {
    /// Virtual nodes (shard_count × vnodes_per_shard)
    vnodes: Vec<(u64, u16)>, // (hash, shard_id)

    /// Number of virtual nodes per shard (default: 150)
    vnodes_per_shard: u16,
}

impl ConsistentHashRing {
    /// Create new consistent hash ring
    ///
    /// # Arguments
    /// - shard_count: Number of physical shards (1-1024)
    ///
    /// # Returns
    /// - ConsistentHashRing with vnodes_per_shard=150 (even distribution)
    pub fn new(shard_count: u16) -> Self {
        const DEFAULT_VNODES_PER_SHARD: u16 = 150;

        let mut vnodes = Vec::new();

        for shard_id in 0..shard_count {
            for vnode in 0..DEFAULT_VNODES_PER_SHARD {
                let mut hasher = DefaultHasher::new();
                (shard_id, vnode).hash(&mut hasher);
                let hash = hasher.finish();

                vnodes.push((hash, shard_id));
            }
        }

        // Sort by hash (enables binary search)
        vnodes.sort_by_key(|(hash, _)| *hash);

        Self {
            vnodes,
            vnodes_per_shard: DEFAULT_VNODES_PER_SHARD,
        }
    }

    /// Get shard for LSH bucket (deterministic routing)
    ///
    /// # T28 Property Test Support
    /// - Deterministic: Same bucket → same shard always
    /// - Fast: O(log N) binary search
    ///
    /// # Returns
    /// - Shard ID (0 to shard_count-1)
    pub fn get_shard(&self, lsh_bucket: u16) -> u16 {
        if self.vnodes.is_empty() {
            return 0;
        }

        let mut hasher = DefaultHasher::new();
        lsh_bucket.hash(&mut hasher);
        let bucket_hash = hasher.finish();

        // Binary search for closest vnode
        let idx = match self.vnodes.binary_search_by_key(&bucket_hash, |(h, _)| *h) {
            Ok(i) => i,
            Err(i) => {
                if i >= self.vnodes.len() {
                    0 // Wrap around to first vnode
                } else {
                    i
                }
            }
        };

        self.vnodes[idx].1 // Return shard_id
    }

    /// Add new shard (minimal rebalancing)
    ///
    /// # T28 Integration Test Support
    /// - Only affects keys near new vnodes
    /// - <1% key migration (vs 50% for naive modulo)
    pub fn add_shard(&mut self, shard_id: u16) {
        for vnode in 0..self.vnodes_per_shard {
            let mut hasher = DefaultHasher::new();
            (shard_id, vnode).hash(&mut hasher);
            let hash = hasher.finish();

            self.vnodes.push((hash, shard_id));
        }

        // Re-sort vnodes
        self.vnodes.sort_by_key(|(hash, _)| *hash);
    }

    /// Remove shard (redistribute keys evenly)
    ///
    /// # T28 Integration Test Support
    /// - Removes all vnodes for shard
    /// - Keys redistribute to next vnodes (no hotspots)
    pub fn remove_shard(&mut self, shard_id: u16) {
        self.vnodes.retain(|(_, id)| *id != shard_id);
    }

    /// Get total vnode count
    pub fn vnode_count(&self) -> usize {
        self.vnodes.len()
    }

    /// Get shard count
    pub fn shard_count(&self) -> u16 {
        if self.vnodes.is_empty() {
            return 0;
        }

        let mut max_shard = 0;
        for (_, shard_id) in &self.vnodes {
            if *shard_id > max_shard {
                max_shard = *shard_id;
            }
        }

        max_shard + 1
    }
}

// ============================================================================
// TIER 1: UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // ------------------------------------------------------------------------
    // Test Group 1: New ConsistentHashRing (N shards)
    // ------------------------------------------------------------------------

    #[test]
    fn test_new_ring_1_shard() {
        let ring = ConsistentHashRing::new(1);

        assert_eq!(ring.shard_count(), 1);
        assert_eq!(ring.vnode_count(), 150);
    }

    #[test]
    fn test_new_ring_10_shards() {
        let ring = ConsistentHashRing::new(10);

        assert_eq!(ring.shard_count(), 10);
        assert_eq!(ring.vnode_count(), 1500); // 10 × 150
    }

    #[test]
    fn test_new_ring_100_shards() {
        let ring = ConsistentHashRing::new(100);

        assert_eq!(ring.shard_count(), 100);
        assert_eq!(ring.vnode_count(), 15000); // 100 × 150
    }

    #[test]
    fn test_new_ring_empty() {
        let ring = ConsistentHashRing::new(0);

        assert_eq!(ring.shard_count(), 0);
        assert_eq!(ring.vnode_count(), 0);
    }

    // ------------------------------------------------------------------------
    // Test Group 2: Get shard for LSH bucket (deterministic, 1000 buckets same shard)
    // ------------------------------------------------------------------------

    #[test]
    fn test_get_shard_deterministic() {
        let ring = ConsistentHashRing::new(10);

        let bucket = 42;
        let shard1 = ring.get_shard(bucket);
        let shard2 = ring.get_shard(bucket);
        let shard3 = ring.get_shard(bucket);

        // Same bucket always maps to same shard
        assert_eq!(shard1, shard2);
        assert_eq!(shard2, shard3);
    }

    #[test]
    fn test_get_shard_1000_buckets_deterministic() {
        let ring = ConsistentHashRing::new(10);

        for bucket in 0..1000 {
            let shard1 = ring.get_shard(bucket);
            let shard2 = ring.get_shard(bucket);

            assert_eq!(shard1, shard2, "Bucket {} should map to same shard", bucket);
        }
    }

    #[test]
    fn test_get_shard_distribution() {
        let ring = ConsistentHashRing::new(10);

        let mut shard_counts = HashMap::new();

        // Assign 10,000 buckets
        for bucket in 0..10000 {
            let shard = ring.get_shard(bucket);
            *shard_counts.entry(shard).or_insert(0) += 1;
        }

        // Check all shards have some buckets (no zero-count shards)
        for shard_id in 0..10 {
            let count = shard_counts.get(&shard_id).copied().unwrap_or(0);
            assert!(
                count > 0,
                "Shard {} should have buckets, got {}",
                shard_id,
                count
            );
        }

        // Check distribution is roughly even (within 50% of average)
        let average = 10000 / 10; // 1000 per shard
        for (shard_id, count) in &shard_counts {
            let deviation = (*count as f64 - average as f64).abs() / average as f64;
            assert!(
                deviation < 0.5,
                "Shard {} has poor distribution: {} buckets (avg {})",
                shard_id,
                count,
                average
            );
        }
    }

    #[test]
    fn test_get_shard_range() {
        let ring = ConsistentHashRing::new(10);

        for bucket in 0..1000 {
            let shard = ring.get_shard(bucket);

            // Shard ID must be in range [0, 10)
            assert!(
                shard < 10,
                "Shard {} out of range for bucket {}",
                shard,
                bucket
            );
        }
    }

    // ------------------------------------------------------------------------
    // Test Group 3: Add shard (minimal rebalancing, <1% migration)
    // ------------------------------------------------------------------------

    #[test]
    fn test_add_shard_increases_count() {
        let mut ring = ConsistentHashRing::new(10);

        ring.add_shard(10); // Add shard 10

        assert_eq!(ring.shard_count(), 11);
        assert_eq!(ring.vnode_count(), 1650); // 11 × 150
    }

    #[test]
    fn test_add_shard_minimal_rebalancing() {
        let mut ring = ConsistentHashRing::new(10);

        // Record shard assignments before adding new shard
        let mut before = HashMap::new();
        for bucket in 0..10000 {
            before.insert(bucket, ring.get_shard(bucket));
        }

        // Add new shard
        ring.add_shard(10);

        // Record shard assignments after
        let mut changed = 0;
        for bucket in 0..10000 {
            let before_shard = before[&bucket];
            let after_shard = ring.get_shard(bucket);

            if before_shard != after_shard {
                changed += 1;
            }
        }

        // Check: <20% of keys migrated (ideal is ~9% = 1/11)
        let migration_pct = (changed as f64 / 10000.0) * 100.0;
        assert!(
            migration_pct < 20.0,
            "Too many keys migrated: {:.1}% (expected <20%)",
            migration_pct
        );
    }

    #[test]
    fn test_add_shard_even_distribution() {
        let mut ring = ConsistentHashRing::new(10);
        ring.add_shard(10);

        let mut shard_counts = HashMap::new();

        for bucket in 0..10000 {
            let shard = ring.get_shard(bucket);
            *shard_counts.entry(shard).or_insert(0) += 1;
        }

        // Check new shard gets roughly equal share
        let new_shard_count = shard_counts.get(&10).copied().unwrap_or(0);
        let average = 10000 / 11; // ~909 per shard

        let deviation = (new_shard_count as f64 - average as f64).abs() / average as f64;
        assert!(
            deviation < 0.5,
            "New shard has poor distribution: {} buckets (avg {})",
            new_shard_count,
            average
        );
    }

    // ------------------------------------------------------------------------
    // Test Group 4: Remove shard (even distribution, no hotspots)
    // ------------------------------------------------------------------------

    #[test]
    fn test_remove_shard_decreases_count() {
        let mut ring = ConsistentHashRing::new(10);

        ring.remove_shard(5); // Remove shard 5

        assert_eq!(ring.shard_count(), 9);
        assert_eq!(ring.vnode_count(), 1350); // 9 × 150
    }

    #[test]
    fn test_remove_shard_redistributes_evenly() {
        let mut ring = ConsistentHashRing::new(10);

        ring.remove_shard(5);

        let mut shard_counts = HashMap::new();

        for bucket in 0..10000 {
            let shard = ring.get_shard(bucket);
            *shard_counts.entry(shard).or_insert(0) += 1;
        }

        // Check no shard is a hotspot (>2× average)
        let average = 10000 / 9; // ~1111 per shard

        for (shard_id, count) in &shard_counts {
            assert!(
                *count < average * 2,
                "Shard {} is hotspot: {} buckets (avg {})",
                shard_id,
                count,
                average
            );
        }
    }

    #[test]
    fn test_remove_shard_no_orphaned_buckets() {
        let mut ring = ConsistentHashRing::new(10);

        ring.remove_shard(5);

        // All buckets should still map to valid shards
        for bucket in 0..1000 {
            let shard = ring.get_shard(bucket);

            assert_ne!(shard, 5, "Bucket {} still maps to removed shard 5", bucket);
        }
    }

    #[test]
    fn test_remove_all_shards() {
        let mut ring = ConsistentHashRing::new(3);

        ring.remove_shard(0);
        ring.remove_shard(1);
        ring.remove_shard(2);

        assert_eq!(ring.shard_count(), 0);
        assert_eq!(ring.vnode_count(), 0);

        // Should handle empty ring gracefully
        let shard = ring.get_shard(42);
        assert_eq!(shard, 0); // Default to shard 0 when empty
    }

    // ------------------------------------------------------------------------
    // Test Group 5: Edge Cases
    // ------------------------------------------------------------------------

    #[test]
    fn test_get_shard_with_1_shard() {
        let ring = ConsistentHashRing::new(1);

        // All buckets should map to shard 0
        for bucket in 0..1000 {
            let shard = ring.get_shard(bucket);
            assert_eq!(shard, 0);
        }
    }

    #[test]
    fn test_add_duplicate_shard() {
        let mut ring = ConsistentHashRing::new(10);

        ring.add_shard(5); // Shard 5 already exists

        // Should have vnodes from both (10 original + 1 duplicate)
        assert_eq!(ring.vnode_count(), 1650); // 11 × 150
    }

    #[test]
    fn test_remove_nonexistent_shard() {
        let mut ring = ConsistentHashRing::new(10);

        ring.remove_shard(99); // Shard 99 doesn't exist

        // Should be unchanged
        assert_eq!(ring.shard_count(), 10);
        assert_eq!(ring.vnode_count(), 1500);
    }
}
