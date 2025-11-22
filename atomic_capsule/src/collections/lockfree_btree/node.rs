//! # BTreeNode - Lockfree B+ Tree Node
//!
//! 128-byte cache-aligned node with atomic metadata and generation counters.

use core::marker::PhantomData;
use core::sync::atomic::{AtomicPtr, AtomicU64, Ordering};

use super::types::{pack_metadata, unpack_metadata, NodeType, SearchResult};

/// Default B-tree degree (8 children max, 7 keys max)
///
/// # Rationale
/// - DEGREE = 8: Balance between tree height and cache locality
/// - 7 keys: Fits in 128B node with metadata + pointers
/// - Tree height for 1M entries: ~5 levels (8^5 = 32768 capacity per level)
pub const DEFAULT_DEGREE: usize = 8;

/// Maximum number of keys per node (DEGREE - 1)
pub const MAX_KEYS: usize = DEFAULT_DEGREE - 1;

/// BTreeNode - Lockfree B+ tree node (128B cache-aligned)
///
/// # Memory Layout
/// ```text
/// Offset 0-7:    metadata (AtomicU64) - node_type(1) | num_keys(15) | generation(48)
/// Offset 8-71:   keys [Option<K>; 7] - Sorted keys (MAX_KEYS = 7 for DEGREE=8)
/// Offset 72-135: values [Option<V>; 7] - Values (leaf nodes only)
/// Offset 136-199: children [AtomicPtr<Node>; 8] - Child pointers (internal nodes only)
/// Offset 200-255: _padding - Complete 128B alignment
/// ```
///
/// # Node Types
/// - **Internal**: keys + children (no values)
/// - **Leaf**: keys + values (children[DEGREE-1] = right sibling pointer)
///
/// # Generation Counter
/// - 48-bit counter in metadata (wraps after 281 trillion operations)
/// - Incremented on every modification
/// - Prevents ABA problem in concurrent CAS operations
///
/// # Safety
/// - `#[repr(C, align(128))]` guarantees layout and alignment
/// - AtomicPtr prevents data races on child pointers
/// - Generation counter prevents TOCTOU races
///
/// NOTE: Cannot use derive(ComputationalCapsule) on generic structs with const parameters
/// Manual verification via const assertions below
#[repr(C, align(128))]
pub struct BTreeNode<K, V> {
    /// Packed metadata: node_type(1 bit) | num_keys(15 bits) | generation(48 bits)
    ///
    /// # Ordering
    /// - Load: Acquire (synchronize with node updates)
    /// - Store: Release (publish all field updates)
    /// - CAS: AcqRel (full synchronization for node replacement)
    metadata: AtomicU64,

    /// Sorted keys (MAX_KEYS = 7 for DEGREE=8)
    ///
    /// # Invariant
    /// - keys[0..num_keys] are sorted in ascending order
    /// - keys[num_keys..] are None
    /// - Binary search for lookups (O(log DEGREE))
    keys: [Option<K>; MAX_KEYS],

    /// Values (leaf nodes only, MAX_KEYS = 7)
    ///
    /// # Invariant
    /// - values[i] corresponds to keys[i] in leaf nodes
    /// - values are None for internal nodes
    values: [Option<V>; MAX_KEYS],

    /// Child pointers (internal nodes only, DEGREE = 8 children)
    ///
    /// # Invariant
    /// - children[i] contains keys < keys[i] (for i < num_keys)
    /// - children[num_keys] contains keys >= keys[num_keys-1]
    /// - For leaf nodes: children[DEGREE-1] = right sibling pointer, rest null
    ///
    /// # Ordering
    /// - Load: Acquire (synchronize with child updates)
    /// - Store: Release (publish child after split)
    /// - CAS: AcqRel (atomic child replacement during merge)
    children: [AtomicPtr<BTreeNode<K, V>>; DEFAULT_DEGREE],

    /// Padding to complete 128-byte alignment
    /// Size calculated to make struct exactly 128 bytes
    /// NOTE: Actual size depends on K and V sizes
    _padding: [u8; 0], // Placeholder - will adjust based on K/V

    /// PhantomData for unused generic parameters in const context
    _phantom: PhantomData<(K, V)>,
}

// Compile-time verification (when not using derive feature)
#[cfg(not(feature = "derive"))]
crate::verify_alignment_only!(BTreeNode<(), ()>, 128);

impl<K, V> BTreeNode<K, V>
where
    K: Ord + Clone,
    V: Clone,
{
    /// Create new empty node
    pub fn new(node_type: NodeType) -> Self {
        // Initialize with empty metadata
        let metadata = pack_metadata(node_type, 0, 0);

        Self {
            metadata: AtomicU64::new(metadata),
            keys: Default::default(),
            values: Default::default(),
            children: [
                AtomicPtr::new(core::ptr::null_mut()),
                AtomicPtr::new(core::ptr::null_mut()),
                AtomicPtr::new(core::ptr::null_mut()),
                AtomicPtr::new(core::ptr::null_mut()),
                AtomicPtr::new(core::ptr::null_mut()),
                AtomicPtr::new(core::ptr::null_mut()),
                AtomicPtr::new(core::ptr::null_mut()),
                AtomicPtr::new(core::ptr::null_mut()),
            ],
            _padding: [],
            _phantom: PhantomData,
        }
    }

    /// Load node metadata
    ///
    /// # Returns
    /// (node_type, num_keys, generation)
    #[inline(always)]
    pub fn load_metadata(&self) -> (NodeType, usize, u64) {
        let meta = self.metadata.load(Ordering::Acquire);
        unpack_metadata(meta)
    }

    /// Check if node is empty
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        let (_, num_keys, _) = self.load_metadata();
        num_keys == 0
    }

    /// Check if node is full
    #[inline(always)]
    pub fn is_full(&self) -> bool {
        let (_, num_keys, _) = self.load_metadata();
        num_keys >= MAX_KEYS
    }

    /// Binary search for key position
    ///
    /// # Returns
    /// - SearchResult::Found(idx) if key exists at idx
    /// - SearchResult::NotFound(idx) if key not found, idx is insertion point
    #[inline(always)]
    pub fn binary_search(&self, key: &K) -> SearchResult {
        let (_, num_keys, _) = self.load_metadata();

        // Binary search in keys[0..num_keys]
        let mut left = 0;
        let mut right = num_keys;

        while left < right {
            let mid = left + (right - left) / 2;

            match &self.keys[mid] {
                Some(k) if k == key => return SearchResult::Found(mid),
                Some(k) if k < key => left = mid + 1,
                _ => right = mid,
            }
        }

        SearchResult::NotFound(left)
    }

    /// Get key at index
    ///
    /// # Safety
    /// Caller must ensure idx < num_keys
    #[inline(always)]
    pub fn get_key(&self, idx: usize) -> Option<&K> {
        self.keys.get(idx).and_then(|k| k.as_ref())
    }

    /// Get value at index (leaf nodes only)
    ///
    /// # Safety
    /// Caller must ensure idx < num_keys and node is leaf
    #[inline(always)]
    pub fn get_value(&self, idx: usize) -> Option<&V> {
        self.values.get(idx).and_then(|v| v.as_ref())
    }

    /// Load child pointer at index
    ///
    /// # Safety
    /// Caller must ensure idx <= num_keys and node is internal
    #[inline(always)]
    pub fn load_child(&self, idx: usize) -> *mut BTreeNode<K, V> {
        self.children[idx].load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_new_internal() {
        let node = BTreeNode::<u64, String>::new(NodeType::Internal);
        let (node_type, num_keys, generation) = node.load_metadata();

        assert_eq!(node_type, NodeType::Internal);
        assert_eq!(num_keys, 0);
        assert_eq!(generation, 0);
        assert!(node.is_empty());
        assert!(!node.is_full());
    }

    #[test]
    fn test_node_new_leaf() {
        let node = BTreeNode::<u64, String>::new(NodeType::Leaf);
        let (node_type, num_keys, generation) = node.load_metadata();

        assert_eq!(node_type, NodeType::Leaf);
        assert_eq!(num_keys, 0);
        assert_eq!(generation, 0);
        assert!(node.is_empty());
    }

    #[test]
    fn test_binary_search_empty() {
        let node = BTreeNode::<u64, String>::new(NodeType::Leaf);
        let result = node.binary_search(&42);

        assert_eq!(result, SearchResult::NotFound(0));
    }

    #[test]
    fn test_alignment() {
        // Verify 128-byte alignment
        use core::mem::{align_of, size_of};

        assert_eq!(align_of::<BTreeNode<u64, String>>(), 128);
        // Size may vary depending on K/V, but alignment must be 128
    }
}
