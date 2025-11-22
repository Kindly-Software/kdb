//! # LockfreeBTree Types
//!
//! Core type definitions for lockfree B-tree implementation.

use core::fmt;

/// Node type in B+ tree
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    /// Internal node (keys + children, no values)
    Internal,
    /// Leaf node (keys + values, no children except right sibling pointer)
    Leaf,
}

/// Search result from binary search
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchResult {
    /// Key found at index
    Found(usize),
    /// Key not found, insertion point at index
    NotFound(usize),
}

/// Error types for B-tree operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BTreeError {
    /// Key not found in tree
    KeyNotFound,
    /// Concurrent modification detected (CAS failed)
    ConcurrentModification,
    /// Tree is full (cannot insert more nodes)
    TreeFull,
    /// Invalid node state (corrupted metadata)
    InvalidNodeState,
    /// Maximum retry limit exceeded
    MaxRetriesExceeded,
}

impl fmt::Display for BTreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BTreeError::KeyNotFound => write!(f, "Key not found in tree"),
            BTreeError::ConcurrentModification => {
                write!(f, "Concurrent modification detected (CAS failed)")
            }
            BTreeError::TreeFull => write!(f, "Tree is full (cannot insert more nodes)"),
            BTreeError::InvalidNodeState => write!(f, "Invalid node state (corrupted metadata)"),
            BTreeError::MaxRetriesExceeded => write!(f, "Maximum retry limit exceeded"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BTreeError {}

/// Result type for B-tree operations
pub type BTreeResult<T> = Result<T, BTreeError>;

/// Metadata packing constants
pub const NODE_TYPE_MASK: u64 = 1u64 << 63;
pub const NUM_KEYS_MASK: u64 = 0x7FFF << 48; // 15 bits
pub const GENERATION_MASK: u64 = (1u64 << 48) - 1; // 48 bits

/// Unpack metadata into (node_type, num_keys, generation)
#[inline(always)]
pub fn unpack_metadata(meta: u64) -> (NodeType, usize, u64) {
    let node_type = if (meta & NODE_TYPE_MASK) != 0 {
        NodeType::Leaf
    } else {
        NodeType::Internal
    };
    let num_keys = ((meta & NUM_KEYS_MASK) >> 48) as usize;
    let generation = meta & GENERATION_MASK;
    (node_type, num_keys, generation)
}

/// Pack metadata from (node_type, num_keys, generation)
#[inline(always)]
pub fn pack_metadata(node_type: NodeType, num_keys: usize, generation: u64) -> u64 {
    let type_bit = match node_type {
        NodeType::Internal => 0,
        NodeType::Leaf => NODE_TYPE_MASK,
    };
    let keys_bits = ((num_keys as u64) & 0x7FFF) << 48;
    let gen_bits = generation & GENERATION_MASK;
    type_bit | keys_bits | gen_bits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_packing_internal() {
        let packed = pack_metadata(NodeType::Internal, 5, 1000);
        let (node_type, num_keys, generation) = unpack_metadata(packed);

        assert_eq!(node_type, NodeType::Internal);
        assert_eq!(num_keys, 5);
        assert_eq!(generation, 1000);
    }

    #[test]
    fn test_metadata_packing_leaf() {
        let packed = pack_metadata(NodeType::Leaf, 7, 0xFFFF_FFFF_FFFF);
        let (node_type, num_keys, generation) = unpack_metadata(packed);

        assert_eq!(node_type, NodeType::Leaf);
        assert_eq!(num_keys, 7);
        assert_eq!(generation, 0xFFFF_FFFF_FFFF);
    }

    #[test]
    fn test_generation_wrap() {
        // Test 48-bit generation counter wraps correctly
        let max_gen = (1u64 << 48) - 1;
        let packed = pack_metadata(NodeType::Leaf, 3, max_gen);
        let (_, _, generation) = unpack_metadata(packed);

        assert_eq!(generation, max_gen);

        // Wrap around
        let wrapped_gen = (max_gen + 1) & GENERATION_MASK;
        let packed = pack_metadata(NodeType::Leaf, 3, wrapped_gen);
        let (_, _, generation) = unpack_metadata(packed);

        assert_eq!(generation, 0); // Wraps to 0
    }

    #[test]
    fn test_max_keys() {
        // Test 15-bit num_keys (max 32767)
        let max_keys = 0x7FFF; // 32767
        let packed = pack_metadata(NodeType::Internal, max_keys, 100);
        let (_, num_keys, _) = unpack_metadata(packed);

        assert_eq!(num_keys, max_keys);
    }
}
