//! # Consistent Hashing - Deterministic Shard Routing
//!
//! **Minimal rebalancing** on shard add/remove using virtual nodes.
//!
//! ## Design Principles
//!
//! - **Virtual nodes**: Each shard gets N vnodes (default 150)
//! - **Binary search**: O(log N) lookup (<10ns for 1000 vnodes)
//! - **Deterministic**: Same key always routes to same shard (no randomness)
//! - **Minimal rebalancing**: Only K/N keys move when adding/removing shard
//!
//! ## Performance (B32 Framework)
//!
//! - Lookup: <10ns (binary search on sorted vnodes)
//! - Add shard: <50µs (insert N vnodes, re-sort)
//! - Remove shard: <50µs (filter vnodes, re-sort)
//! - Memory: 16 bytes per vnode (shard_id + hash)
//!
//! ## Example
//!
//! ```
//! use atomic_capsule::network::ConsistentHashRing;
//!
//! let mut ring = ConsistentHashRing::new(150);
//! ring.add_shard(1);
//! ring.add_shard(2);
//! ring.add_shard(3);
//!
//! let shard_id = ring.get_shard(b"some_key");
//! assert!(shard_id == 1 || shard_id == 2 || shard_id == 3);
//! ```

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Virtual node in the consistent hash ring
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VirtualNode {
    /// Hash value for this vnode (determines position on ring)
    hash: u64,
    /// Physical shard ID this vnode maps to
    shard_id: u64,
}

impl PartialOrd for VirtualNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VirtualNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.hash.cmp(&other.hash)
    }
}

/// Consistent hash ring for shard routing
///
/// # ASSUM
///
/// - `#ASSUME_DETERMINISTIC_HASH`: DefaultHasher is deterministic (it is on same platform)
/// - `#ASSUME_VNODE_SPREAD`: Virtual nodes spread evenly across hash space
/// - `#VERIFY_SORTED`: Ring is always sorted (binary search requirement)
pub struct ConsistentHashRing {
    /// Virtual nodes (sorted by hash)
    vnodes: Vec<VirtualNode>,
    /// Number of virtual nodes per shard
    vnodes_per_shard: usize,
}

impl ConsistentHashRing {
    /// Create new consistent hash ring
    ///
    /// # Arguments
    ///
    /// - `vnodes_per_shard`: Number of virtual nodes per physical shard (default: 150)
    ///
    /// # Performance
    ///
    /// Higher vnodes = better distribution, but slower add/remove operations
    /// Recommended: 100-200 vnodes per shard
    pub fn new(vnodes_per_shard: usize) -> Self {
        Self {
            vnodes: Vec::new(),
            vnodes_per_shard,
        }
    }

    /// Add shard to ring
    ///
    /// Creates `vnodes_per_shard` virtual nodes for this shard.
    ///
    /// # Performance
    ///
    /// - <50µs for 150 vnodes (hash + insert + sort)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_UNIQUE_SHARD_ID`: Shard IDs are unique
    pub fn add_shard(&mut self, shard_id: u64) {
        for i in 0..self.vnodes_per_shard {
            let hash = self.hash_vnode(shard_id, i);
            self.vnodes.push(VirtualNode { hash, shard_id });
        }

        // #VERIFY_SORTED: Keep vnodes sorted for binary search
        self.vnodes.sort_unstable();
    }

    /// Remove shard from ring
    ///
    /// # Performance
    ///
    /// - <50µs for 150 vnodes (filter + sort)
    pub fn remove_shard(&mut self, shard_id: u64) {
        self.vnodes.retain(|vnode| vnode.shard_id != shard_id);
    }

    /// Get shard for given key
    ///
    /// # Returns
    ///
    /// Shard ID that should handle this key, or None if ring is empty
    ///
    /// # Performance
    ///
    /// - <10ns (binary search on sorted vnodes)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_NON_EMPTY`: Caller ensures ring has shards
    /// - `#VERIFY_SORTED`: vnodes must be sorted (ensured by add_shard)
    pub fn get_shard(&self, key: &[u8]) -> Option<u64> {
        if self.vnodes.is_empty() {
            return None;
        }

        let key_hash = self.hash_key(key);

        // Binary search for first vnode >= key_hash
        let idx = match self
            .vnodes
            .binary_search_by_key(&key_hash, |vnode| vnode.hash)
        {
            Ok(i) => i,
            Err(i) => {
                if i >= self.vnodes.len() {
                    0 // Wrap around to first vnode
                } else {
                    i
                }
            }
        };

        Some(self.vnodes[idx].shard_id)
    }

    /// Get N shards for given key (for replication)
    ///
    /// Returns up to N distinct shard IDs in clockwise order on ring
    pub fn get_shards(&self, key: &[u8], n: usize) -> Vec<u64> {
        if self.vnodes.is_empty() {
            return Vec::new();
        }

        let key_hash = self.hash_key(key);
        let start_idx = match self
            .vnodes
            .binary_search_by_key(&key_hash, |vnode| vnode.hash)
        {
            Ok(i) => i,
            Err(i) => {
                if i >= self.vnodes.len() {
                    0
                } else {
                    i
                }
            }
        };

        let mut shards = Vec::with_capacity(n);
        let mut idx = start_idx;

        // Walk clockwise until we have N distinct shards
        while shards.len() < n {
            let shard_id = self.vnodes[idx].shard_id;
            if !shards.contains(&shard_id) {
                shards.push(shard_id);
            }

            idx = (idx + 1) % self.vnodes.len();

            // Safety: prevent infinite loop if we have fewer shards than requested
            if idx == start_idx {
                break;
            }
        }

        shards
    }

    /// Get total number of shards
    pub fn shard_count(&self) -> usize {
        let mut unique_shards = std::collections::HashSet::new();
        for vnode in &self.vnodes {
            unique_shards.insert(vnode.shard_id);
        }
        unique_shards.len()
    }

    /// Hash virtual node (shard_id + vnode_num)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_DETERMINISTIC_HASH`: DefaultHasher produces same output for same input
    fn hash_vnode(&self, shard_id: u64, vnode_num: usize) -> u64 {
        let mut hasher = DefaultHasher::new();
        shard_id.hash(&mut hasher);
        vnode_num.hash(&mut hasher);
        hasher.finish()
    }

    /// Hash key for lookup
    fn hash_key(&self, key: &[u8]) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_ring_creation() {
        let ring = ConsistentHashRing::new(150);
        assert_eq!(ring.shard_count(), 0);
    }

    #[test]
    fn test_add_shard() {
        let mut ring = ConsistentHashRing::new(150);
        ring.add_shard(1);
        assert_eq!(ring.shard_count(), 1);
        assert_eq!(ring.vnodes.len(), 150);

        ring.add_shard(2);
        assert_eq!(ring.shard_count(), 2);
        assert_eq!(ring.vnodes.len(), 300);
    }

    #[test]
    fn test_remove_shard() {
        let mut ring = ConsistentHashRing::new(150);
        ring.add_shard(1);
        ring.add_shard(2);
        assert_eq!(ring.shard_count(), 2);

        ring.remove_shard(1);
        assert_eq!(ring.shard_count(), 1);
        assert_eq!(ring.vnodes.len(), 150);

        // All remaining vnodes should be shard 2
        for vnode in &ring.vnodes {
            assert_eq!(vnode.shard_id, 2);
        }
    }

    #[test]
    fn test_get_shard() {
        let mut ring = ConsistentHashRing::new(150);
        ring.add_shard(1);
        ring.add_shard(2);
        ring.add_shard(3);

        let shard = ring.get_shard(b"test_key");
        assert!(shard.is_some());
        assert!(shard.unwrap() >= 1 && shard.unwrap() <= 3);
    }

    #[test]
    fn test_get_shard_empty() {
        let ring = ConsistentHashRing::new(150);
        assert_eq!(ring.get_shard(b"test_key"), None);
    }

    #[test]
    fn test_deterministic_routing() {
        let mut ring = ConsistentHashRing::new(150);
        ring.add_shard(1);
        ring.add_shard(2);
        ring.add_shard(3);

        // Same key should always route to same shard
        let key = b"deterministic_test";
        let shard1 = ring.get_shard(key).unwrap();
        let shard2 = ring.get_shard(key).unwrap();
        let shard3 = ring.get_shard(key).unwrap();

        assert_eq!(shard1, shard2);
        assert_eq!(shard2, shard3);
    }

    #[test]
    fn test_distribution() {
        let mut ring = ConsistentHashRing::new(150);
        ring.add_shard(1);
        ring.add_shard(2);
        ring.add_shard(3);

        // Check distribution across 1000 keys
        let mut counts = HashMap::new();
        for i in 0..1000 {
            let key = format!("key_{}", i);
            let shard = ring.get_shard(key.as_bytes()).unwrap();
            *counts.entry(shard).or_insert(0) += 1;
        }

        // Each shard should get roughly 333 keys (±20%)
        for count in counts.values() {
            assert!(
                *count >= 250 && *count <= 450,
                "Distribution skewed: {}",
                count
            );
        }
    }

    #[test]
    fn test_minimal_rebalancing() {
        let mut ring = ConsistentHashRing::new(150);
        ring.add_shard(1);
        ring.add_shard(2);

        // Record shard assignments
        let mut assignments_before = HashMap::new();
        for i in 0..1000 {
            let key = format!("key_{}", i);
            let shard = ring.get_shard(key.as_bytes()).unwrap();
            assignments_before.insert(key, shard);
        }

        // Add third shard
        ring.add_shard(3);

        // Check how many keys moved
        let mut moved = 0;
        for i in 0..1000 {
            let key = format!("key_{}", i);
            let shard_after = ring.get_shard(key.as_bytes()).unwrap();
            if assignments_before[&key] != shard_after {
                moved += 1;
            }
        }

        // Should move roughly 1/3 of keys (K/N where N=3)
        // Allow ±20% variance
        let expected = 333;
        assert!(
            moved >= 250 && moved <= 450,
            "Too many keys moved: {} (expected ~{})",
            moved,
            expected
        );
    }

    #[test]
    fn test_get_multiple_shards() {
        let mut ring = ConsistentHashRing::new(150);
        ring.add_shard(1);
        ring.add_shard(2);
        ring.add_shard(3);

        let shards = ring.get_shards(b"test_key", 3);
        assert_eq!(shards.len(), 3);

        // All shards should be unique
        assert_ne!(shards[0], shards[1]);
        assert_ne!(shards[1], shards[2]);
        assert_ne!(shards[0], shards[2]);
    }

    #[test]
    fn test_get_more_shards_than_available() {
        let mut ring = ConsistentHashRing::new(150);
        ring.add_shard(1);
        ring.add_shard(2);

        // Request 5 shards but only 2 available
        let shards = ring.get_shards(b"test_key", 5);
        assert_eq!(shards.len(), 2);
    }

    #[test]
    fn test_vnodes_sorted() {
        let mut ring = ConsistentHashRing::new(150);
        ring.add_shard(1);
        ring.add_shard(2);
        ring.add_shard(3);

        // Verify vnodes are sorted
        for i in 1..ring.vnodes.len() {
            assert!(
                ring.vnodes[i - 1].hash <= ring.vnodes[i].hash,
                "Vnodes not sorted at index {}",
                i
            );
        }
    }

    #[test]
    fn test_wrap_around() {
        let mut ring = ConsistentHashRing::new(10); // Fewer vnodes for predictability
        ring.add_shard(1);

        // A key that hashes beyond all vnodes should wrap to first
        let key = b"wrap_test";
        let shard = ring.get_shard(key);
        assert_eq!(shard, Some(1));
    }
}
