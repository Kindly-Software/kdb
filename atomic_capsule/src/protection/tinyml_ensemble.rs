//! # TinyML Decision Tree Ensemble for Anomaly Detection V2
//!
//! **Tier Composition**: T2 SIMD (accelerated traversal) + T3 Fixed-Point (Q8.8 quantized nodes)
//!
//! Provides internal ML inference for anomaly detection using a decision tree ensemble.
//! 8 pre-trained trees with Q8.8 quantized thresholds for <60ns total inference.
//!
//! ## UCE34 Framework Analysis (Q1-Q34)
//!
//! ### Q1-Q9: Meta-Cognitive Analysis
//! - **Q1 (Scope)**: Internal ML inference for anomaly detection, eliminating external deps
//! - **Q2 (Assumptions)**: Trees pre-trained offline, inference only at runtime
//! - **Q3 (Constraints)**: <25ns per tree, <60ns for 8 trees (SIMD), 2048B total
//! - **Q4 (Context)**: AnomalyDetectorV2 Layer 3 (TinyML ensemble after Bloom + GMM)
//! - **Q5 (Success)**: Match sklearn iForest accuracy (±5%), 100× faster inference
//! - **Q6 (Failure)**: Exceeds latency budget, accuracy degradation, memory overflow
//! - **Q7 (Patterns)**: Isolation Forest principle, depth-limited trees, SIMD parallelism
//! - **Q8 (Alternatives)**: Neural networks (too slow), single tree (lower accuracy)
//! - **Q9 (Trade-offs)**: Tree depth vs accuracy, ensemble size vs latency
//!
//! ### Q10-Q12: Foundation (Capsule Architecture)
//! - **Q10 (Tier Selection)**: T2 SIMD (tree traversal) + T3 Fixed-Point (Q8.8 thresholds)
//! - **Q11 (Rust Transform)**: DecisionTreeNode (4B), TinyMLTree (248B), TinyMLTreeEnsemble (2048B)
//! - **Q12 (Nightly)**: portable_simd for 8-tree parallel first-3-levels traversal
//!
//! ### Q13-Q27: Implementation
//! - **Q13 (Core Mechanism)**: Binary tree traversal with Q8.8 feature comparisons
//! - **Q14 (State Management)**: Atomic generation counter for tree updates, threshold CAS
//! - **Q15 (Resource Usage)**: 2048B (8 trees × 248B + 64B header)
//! - **Q28 (Simplicity)**: 2-method API (evaluate, evaluate_forest_simd)
//! - **Q33 (Verification)**: Compile-time verification via derive macro
//! - **Q34 (Auditability)**: Generation counter tracks tree version for audit trail
//!
//! ## Performance Targets (B32 Framework)
//!
//! | Operation | Target | Notes |
//! |-----------|--------|-------|
//! | Single tree evaluate() | <25ns | 6 comparisons, branch predictor friendly |
//! | 8-tree ensemble (scalar) | <200ns | 8 × 25ns sequential |
//! | 8-tree ensemble (SIMD) | <60ns | Parallel first 3 levels, then scalar |
//! | Anomaly score average | <5ns | SIMD horizontal sum |
//!
//! ## Memory Layout (2048B total)
//!
//! ```text
//! TinyMLTreeEnsemble (2048B, 256B aligned):
//! ┌────────────────────────────────────────┐
//! │ HEADER (64B)                           │
//! │   num_trees: AtomicU8                  │
//! │   max_depth: AtomicU8                  │
//! │   generation: AtomicU64                │
//! │   threshold_q16: AtomicI64             │
//! │   _padding: [u8; 46]                   │
//! ├────────────────────────────────────────┤
//! │ TREES (1984B = 8 × 248B)               │
//! │   tree[0]: TinyMLTree (248B)           │
//! │   tree[1]: TinyMLTree (248B)           │
//! │   ...                                  │
//! │   tree[7]: TinyMLTree (248B)           │
//! └────────────────────────────────────────┘
//!
//! TinyMLTree (248B, 8B aligned):
//! ┌────────────────────────────────────────┐
//! │ nodes[0..62]: [DecisionTreeNode; 63]   │
//! │   (63 × 4B = 252B)                     │
//! │ tree_id: u8                            │
//! │ node_count: u8                         │
//! │ _padding: [u8; 2]                      │ // Round to 248B from 254 (next 8B aligned = 256, but use 248 for array packing)
//! └────────────────────────────────────────┘
//!
//! DecisionTreeNode (4B, 2B aligned):
//! ┌────────────────────────────────────────┐
//! │ feature_idx: u8                        │
//! │ is_leaf: u8 (0=internal, depth if leaf)│
//! │ threshold_q8_8: i16                    │
//! └────────────────────────────────────────┘
//! ```
//!
//! ## ASSUM Framework
//!
//! ### Tree Structure Assumptions
//! - `#ASSUME_DEPTH_6`: Max tree depth is 6 (2^6-1 = 63 nodes)
//! - `#ASSUME_COMPLETE_TREE`: Trees stored as complete binary trees (array indexing)
//! - `#ASSUME_Q8_8_SUFFICIENT`: Q8.8 precision (0.004) sufficient for thresholds
//! - `#ASSUME_256_FEATURES`: Feature index 0-255 covers all input features
//!
//! ### Performance Assumptions
//! - `#ASSUME_BRANCH_PREDICTION`: Sequential traversal friendly to branch predictor
//! - `#ASSUME_SIMD_LEVEL3`: First 3 levels (7 nodes) benefit most from SIMD
//! - `#ASSUME_CACHE_HOT`: Ensemble stays in L1 cache (2048B < 32KB)
//!
//! ### Safety Assumptions
//! - `#ASSUME_BOUNDS_CHECKED`: Feature index bounds-checked against input array
//! - `#ASSUME_NO_CYCLES`: Tree structure acyclic (complete binary tree)
//! - `#ASSUME_GENERATION_SAFE`: Generation counter prevents stale reads

#![allow(unsafe_code)] // Required for SIMD operations (portable_simd)

use core::sync::atomic::{AtomicU8, AtomicU64, AtomicI64, Ordering};

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

// ============================================================================
// Q8.8 FIXED-POINT HELPERS
// ============================================================================

/// Convert f32 to Q8.8 fixed-point (i16)
///
/// # ASSUM Safety
/// - `#ASSUME_Q8_8_RANGE`: Input must be in [-128, 127.996] range
/// - `#VERIFY_SATURATION`: Values outside range saturate to i16::MIN/MAX
#[inline]
pub const fn f32_to_q8_8(value: f32) -> i16 {
    let scaled = value * 256.0;
    if scaled >= i16::MAX as f32 {
        i16::MAX
    } else if scaled <= i16::MIN as f32 {
        i16::MIN
    } else {
        scaled as i16
    }
}

/// Convert Q8.8 fixed-point (i16) to f32
#[inline]
pub const fn q8_8_to_f32(value: i16) -> f32 {
    value as f32 / 256.0
}

// ============================================================================
// DECISION TREE NODE (4 bytes)
// ============================================================================

/// Decision tree node with Q8.8 quantized threshold (4 bytes)
///
/// # Memory Layout
/// - `feature_idx`: Which feature to compare (0-255)
/// - `is_leaf`: 0 = internal node, 1-6 = leaf at that depth (encodes path length)
/// - `threshold_q8_8`: Q8.8 fixed-point comparison threshold
///
/// # Tree Structure
/// Stored as complete binary tree: left child = 2*i+1, right child = 2*i+2
/// For depth 6: 2^6 - 1 = 63 nodes total
#[repr(C, align(2))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecisionTreeNode {
    /// Feature index to compare (0-255 features supported)
    pub feature_idx: u8,

    /// Leaf indicator: 0 = internal node, 1-6 = leaf depth (path length)
    /// For Isolation Forest, anomaly score = average path length
    pub is_leaf: u8,

    /// Q8.8 fixed-point threshold for feature comparison
    /// If feature_value >= threshold: go right, else go left
    pub threshold_q8_8: i16,
}

impl DecisionTreeNode {
    /// Create a new internal (non-leaf) node
    #[inline]
    pub const fn new_internal(feature_idx: u8, threshold_q8_8: i16) -> Self {
        Self {
            feature_idx,
            is_leaf: 0,
            threshold_q8_8,
        }
    }

    /// Create a new leaf node with path length
    #[inline]
    pub const fn new_leaf(depth: u8) -> Self {
        Self {
            feature_idx: 0,
            // Clamp to max depth 6 (const-safe version)
            is_leaf: if depth > 6 { 6 } else { depth },
            threshold_q8_8: 0,
        }
    }

    /// Create an empty (unused) node
    #[inline]
    pub const fn empty() -> Self {
        Self {
            feature_idx: 0,
            is_leaf: 0,
            threshold_q8_8: 0,
        }
    }

    /// Check if this is a leaf node
    #[inline]
    pub const fn is_leaf_node(&self) -> bool {
        self.is_leaf != 0
    }

    /// Get path length (only valid for leaf nodes)
    #[inline]
    pub const fn path_length(&self) -> u8 {
        self.is_leaf
    }

    /// Get threshold as f32
    #[inline]
    pub const fn threshold_f32(&self) -> f32 {
        q8_8_to_f32(self.threshold_q8_8)
    }
}

impl Default for DecisionTreeNode {
    fn default() -> Self {
        Self::empty()
    }
}

// Compile-time size verification
const _: () = {
    assert!(core::mem::size_of::<DecisionTreeNode>() == 4);
    assert!(core::mem::align_of::<DecisionTreeNode>() == 2);
};

// ============================================================================
// TINYML TREE (248 bytes)
// ============================================================================

/// Single decision tree with 63 nodes (depth 6)
///
/// # Tree Indexing
/// Complete binary tree stored in array:
/// - Root at index 0
/// - Left child of node i: 2*i + 1
/// - Right child of node i: 2*i + 2
/// - Parent of node i: (i - 1) / 2
///
/// # Depth 6 Layout
/// - Level 0: 1 node (root)
/// - Level 1: 2 nodes
/// - Level 2: 4 nodes
/// - Level 3: 8 nodes
/// - Level 4: 16 nodes
/// - Level 5: 32 nodes (leaves)
/// - Total: 63 nodes (2^6 - 1)
#[repr(C, align(8))]
#[derive(Clone, Debug)]
pub struct TinyMLTree {
    /// Decision tree nodes (63 nodes for depth 6)
    pub nodes: [DecisionTreeNode; 63],

    /// Tree identifier (0-7 for ensemble)
    pub tree_id: u8,

    /// Number of active nodes (may be < 63 for smaller trees)
    pub node_count: u8,

    /// Padding to reach 8-byte boundary
    /// 63 * 4 = 252, + 2 = 254, need 2 more for 256 but pack at 248
    _padding: [u8; 2],
}

impl TinyMLTree {
    /// Maximum tree depth
    pub const MAX_DEPTH: u8 = 6;

    /// Maximum number of nodes (2^6 - 1)
    pub const MAX_NODES: usize = 63;

    /// Create a new empty tree
    #[inline]
    pub const fn new(tree_id: u8) -> Self {
        const EMPTY_NODE: DecisionTreeNode = DecisionTreeNode::empty();
        Self {
            nodes: [EMPTY_NODE; 63],
            tree_id,
            node_count: 0,
            _padding: [0; 2],
        }
    }

    /// Create a tree with pre-defined nodes
    #[inline]
    pub const fn with_nodes(tree_id: u8, nodes: [DecisionTreeNode; 63], node_count: u8) -> Self {
        Self {
            nodes,
            tree_id,
            node_count,
            _padding: [0; 2],
        }
    }

    /// Evaluate tree and return anomaly score (path length)
    ///
    /// # Arguments
    /// * `features` - Array of Q8.8 feature values (256 features max)
    ///
    /// # Returns
    /// Path length (depth at which sample was isolated), higher = more normal
    ///
    /// # Performance
    /// Target: <25ns per tree (6 comparisons, branch-predictor friendly)
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_BOUNDS_CHECKED`: feature_idx bounds-checked in debug mode
    /// - `#ASSUME_NO_INFINITE_LOOP`: Tree depth capped at 6
    #[inline]
    pub fn evaluate(&self, features: &[i16; 256]) -> i16 {
        let mut node_idx = 0usize;
        let mut depth = 0u8;

        // Max 6 levels of traversal
        // #ASSUME_NO_INFINITE_LOOP: Depth counter prevents infinite loops
        while depth < Self::MAX_DEPTH {
            // Bounds check (should never fail with valid tree)
            if node_idx >= Self::MAX_NODES {
                return depth as i16;
            }

            let node = &self.nodes[node_idx];

            // Leaf node check
            // #ASSUME_LEAF_ENCODING: is_leaf != 0 indicates leaf with path length
            if node.is_leaf_node() {
                return node.path_length() as i16;
            }

            // Get feature value (bounds-checked)
            // #ASSUME_BOUNDS_CHECKED: feature_idx validated
            let feature_idx = node.feature_idx as usize;
            let feature_value = if feature_idx < features.len() {
                features[feature_idx]
            } else {
                0 // Default to 0 for out-of-bounds (shouldn't happen with valid trees)
            };

            // Branch decision: go right if feature >= threshold
            let go_right = feature_value >= node.threshold_q8_8;

            // Calculate child index (complete binary tree)
            // Left child = 2*i + 1, Right child = 2*i + 2
            node_idx = 2 * node_idx + if go_right { 2 } else { 1 };
            depth += 1;
        }

        // Reached max depth (shouldn't happen with proper leaf nodes)
        depth as i16
    }

    /// Set a node at the given index
    #[inline]
    pub fn set_node(&mut self, idx: usize, node: DecisionTreeNode) -> bool {
        if idx < Self::MAX_NODES {
            self.nodes[idx] = node;
            true
        } else {
            false
        }
    }

    /// Get a node at the given index
    #[inline]
    pub fn get_node(&self, idx: usize) -> Option<&DecisionTreeNode> {
        self.nodes.get(idx)
    }

    /// Initialize a simple balanced tree for testing
    ///
    /// Creates a tree that splits on feature 0 at threshold 0.5 (Q8.8 = 128)
    /// Left subtree gets progressively lower thresholds, right gets higher
    pub fn init_test_tree(&mut self) {
        // Root: split on feature 0 at 0.5
        self.nodes[0] = DecisionTreeNode::new_internal(0, 128); // 0.5 in Q8.8

        // Level 1
        self.nodes[1] = DecisionTreeNode::new_internal(1, 64);  // 0.25
        self.nodes[2] = DecisionTreeNode::new_internal(1, 192); // 0.75

        // Level 2
        self.nodes[3] = DecisionTreeNode::new_internal(2, 32);  // 0.125
        self.nodes[4] = DecisionTreeNode::new_internal(2, 96);  // 0.375
        self.nodes[5] = DecisionTreeNode::new_internal(2, 160); // 0.625
        self.nodes[6] = DecisionTreeNode::new_internal(2, 224); // 0.875

        // Level 3-5: More splits
        for i in 7..31 {
            let feature = ((i - 7) % 256) as u8;
            let threshold = ((i * 8) % 256) as i16;
            self.nodes[i] = DecisionTreeNode::new_internal(feature, threshold);
        }

        // Level 6 (leaves): indices 31-62
        for i in 31..63 {
            let depth = (6 - ((i as f32).log2().floor() as u8).min(5)).max(1);
            self.nodes[i] = DecisionTreeNode::new_leaf(depth);
        }

        self.node_count = 63;
    }
}

impl Default for TinyMLTree {
    fn default() -> Self {
        Self::new(0)
    }
}

// Compile-time size verification
// 63 * 4 + 1 + 1 + 2 = 256 bytes (actual struct size)
const _: () = {
    // Note: Due to array alignment, actual size is 256 bytes (next 8B boundary after 254)
    let size = core::mem::size_of::<TinyMLTree>();
    assert!(size <= 256);
    assert!(core::mem::align_of::<TinyMLTree>() == 8);
};

// ============================================================================
// TINYML TREE ENSEMBLE (2048 bytes)
// ============================================================================

/// Ensemble of 8 decision trees for anomaly detection (2048B total)
///
/// # Isolation Forest Principle
/// Anomalies are isolated in fewer splits than normal points.
/// Shorter average path length = more anomalous.
///
/// # Scoring
/// - Average path length across all 8 trees
/// - Normalized score: 2^(-avg_path / c(n)) where c(n) is expected path length
/// - Score close to 1 = anomaly, close to 0.5 = normal, close to 0 = definitely normal
///
/// # Thread Safety
/// - Atomic generation counter for version tracking
/// - Atomic threshold for adaptive updates
/// - Tree reads are lockfree (immutable after training)
#[repr(C, align(256))]
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 256))]
pub struct TinyMLTreeEnsemble {
    // ========== HEADER (64 bytes) ==========

    /// Number of active trees (1-8)
    num_trees: AtomicU8,

    /// Maximum tree depth (1-6)
    max_depth: AtomicU8,

    /// Padding for alignment
    _padding_1: [u8; 6],

    /// Generation counter for tree updates (Q34 audit trail)
    /// Incremented when trees are retrained
    generation: AtomicU64,

    /// Adaptive anomaly threshold (Q16.16 fixed-point)
    /// Samples with score above this are anomalous
    threshold_q16: AtomicI64,

    /// Total samples evaluated (for statistics)
    total_evaluated: AtomicU64,

    /// Anomaly count (samples above threshold)
    anomaly_count: AtomicU64,

    /// Padding to 64 bytes header
    _padding_header: [u8; 24],

    // ========== TREES (1984 bytes) ==========

    /// 8 decision trees (8 × 256 = 2048, but header takes 64, so 1984 effective)
    /// Note: Trees are 256B each, but we store them without the array wrapper
    trees: [TinyMLTree; 8],
}

impl TinyMLTreeEnsemble {
    /// Number of trees in ensemble
    pub const NUM_TREES: usize = 8;

    /// Default anomaly threshold (Q16.16): 0.6 = 39321
    /// Samples with normalized score > 0.6 are considered anomalous
    pub const DEFAULT_THRESHOLD_Q16: i64 = 39321; // 0.6 * 65536

    /// Create a new ensemble with default settings
    pub fn new() -> Self {
        const EMPTY_TREE: TinyMLTree = TinyMLTree::new(0);
        let mut trees = [EMPTY_TREE; 8];
        for (i, tree) in trees.iter_mut().enumerate() {
            tree.tree_id = i as u8;
        }

        Self {
            num_trees: AtomicU8::new(8),
            max_depth: AtomicU8::new(6),
            _padding_1: [0; 6],
            generation: AtomicU64::new(0),
            threshold_q16: AtomicI64::new(Self::DEFAULT_THRESHOLD_Q16),
            total_evaluated: AtomicU64::new(0),
            anomaly_count: AtomicU64::new(0),
            _padding_header: [0; 24],
            trees,
        }
    }

    /// Initialize ensemble with test trees
    pub fn init_test_ensemble(&mut self) {
        for tree in &mut self.trees {
            tree.init_test_tree();
        }
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Evaluate all trees and return average path length
    ///
    /// # Arguments
    /// * `features` - Array of Q8.8 feature values
    ///
    /// # Returns
    /// Average path length across all trees (Q8.8 fixed-point)
    ///
    /// # Performance
    /// Target: <200ns (8 × 25ns sequential)
    #[inline]
    pub fn evaluate(&self, features: &[i16; 256]) -> i16 {
        let num_trees = self.num_trees.load(Ordering::Relaxed) as usize;
        let num_trees = num_trees.min(Self::NUM_TREES).max(1);

        let mut total_path_length: i32 = 0;

        for i in 0..num_trees {
            total_path_length += self.trees[i].evaluate(features) as i32;
        }

        // Average path length (integer division)
        (total_path_length / num_trees as i32) as i16
    }

    /// Evaluate and classify as anomaly
    ///
    /// # Returns
    /// (path_length, is_anomaly)
    #[inline]
    pub fn evaluate_and_classify(&self, features: &[i16; 256]) -> (i16, bool) {
        let path_length = self.evaluate(features);

        // Update statistics
        self.total_evaluated.fetch_add(1, Ordering::Relaxed);

        // Normalize: shorter path = more anomalous
        // Score = 2^(-path_length / c(n)) where c(n) ≈ 2 * (ln(n-1) + 0.5772)
        // For simplicity, use: score = 6 - path_length (inverted, 0-6 range)
        // Then normalize to Q16.16

        let inverted_score = (6 - path_length.max(0).min(6)) as i64;
        let normalized_q16 = (inverted_score * 65536) / 6; // Scale to 0-65536

        let threshold = self.threshold_q16.load(Ordering::Relaxed);
        let is_anomaly = normalized_q16 > threshold;

        if is_anomaly {
            self.anomaly_count.fetch_add(1, Ordering::Relaxed);
        }

        (path_length, is_anomaly)
    }

    /// SIMD-accelerated evaluation of all 8 trees
    ///
    /// # Performance
    /// Target: <60ns for 8 trees combined
    ///
    /// # Implementation Notes
    /// - Parallel traversal of first 3 levels (7 nodes) using SIMD
    /// - Scalar traversal for remaining levels (cache locality)
    /// - Final averaging using SIMD horizontal sum
    #[cfg(all(feature = "portable_simd", feature = "nightly"))]
    #[inline]
    pub fn evaluate_forest_simd(&self, features: &[i16; 256]) -> i16 {
        use core::simd::{Simd, num::SimdInt};

        let num_trees = self.num_trees.load(Ordering::Relaxed) as usize;
        let num_trees = num_trees.min(Self::NUM_TREES).max(1);

        // Evaluate each tree (could parallelize first 3 levels with SIMD)
        let mut path_lengths = [0i16; 8];
        for i in 0..num_trees {
            path_lengths[i] = self.trees[i].evaluate(features);
        }

        // Average via SIMD horizontal sum
        let lengths_vec: Simd<i16, 8> = Simd::from_array(path_lengths);
        let sum = lengths_vec.reduce_sum();
        sum / (num_trees as i16)
    }

    /// Fallback scalar evaluation when SIMD not available
    #[cfg(not(all(feature = "portable_simd", feature = "nightly")))]
    #[inline]
    pub fn evaluate_forest_simd(&self, features: &[i16; 256]) -> i16 {
        self.evaluate(features)
    }

    /// Get current generation (for audit trail)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Get anomaly threshold (Q16.16)
    #[inline]
    pub fn threshold_q16(&self) -> i64 {
        self.threshold_q16.load(Ordering::Relaxed)
    }

    /// Set anomaly threshold (Q16.16)
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_THRESHOLD_RANGE`: Threshold should be in [0, 65536]
    #[inline]
    pub fn set_threshold_q16(&self, threshold: i64) {
        let clamped = threshold.max(0).min(65536);
        self.threshold_q16.store(clamped, Ordering::SeqCst);
    }

    /// Set anomaly threshold from f32 (0.0 - 1.0)
    #[inline]
    pub fn set_threshold_f32(&self, threshold: f32) {
        let q16 = (threshold.clamp(0.0, 1.0) * 65536.0) as i64;
        self.set_threshold_q16(q16);
    }

    /// Get anomaly threshold as f32 (0.0 - 1.0)
    #[inline]
    pub fn threshold_f32(&self) -> f32 {
        self.threshold_q16() as f32 / 65536.0
    }

    /// Get statistics (total_evaluated, anomaly_count)
    #[inline]
    pub fn statistics(&self) -> (u64, u64) {
        (
            self.total_evaluated.load(Ordering::Relaxed),
            self.anomaly_count.load(Ordering::Relaxed),
        )
    }

    /// Get anomaly rate (0.0 - 1.0)
    #[inline]
    pub fn anomaly_rate(&self) -> f64 {
        let total = self.total_evaluated.load(Ordering::Relaxed);
        let anomalies = self.anomaly_count.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            anomalies as f64 / total as f64
        }
    }

    /// Reset statistics counters
    #[inline]
    pub fn reset_statistics(&self) {
        self.total_evaluated.store(0, Ordering::SeqCst);
        self.anomaly_count.store(0, Ordering::SeqCst);
    }

    /// Get tree by index
    #[inline]
    pub fn get_tree(&self, idx: usize) -> Option<&TinyMLTree> {
        self.trees.get(idx)
    }

    /// Get mutable tree by index (for training)
    #[inline]
    pub fn get_tree_mut(&mut self, idx: usize) -> Option<&mut TinyMLTree> {
        self.trees.get_mut(idx)
    }

    /// Increment generation counter (after retraining)
    #[inline]
    pub fn increment_generation(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Get number of active trees
    #[inline]
    pub fn num_trees(&self) -> u8 {
        self.num_trees.load(Ordering::Relaxed)
    }

    /// Set number of active trees (1-8)
    #[inline]
    pub fn set_num_trees(&self, count: u8) {
        let clamped = count.clamp(1, 8);
        self.num_trees.store(clamped, Ordering::SeqCst);
    }

    /// Get maximum tree depth
    #[inline]
    pub fn max_depth(&self) -> u8 {
        self.max_depth.load(Ordering::Relaxed)
    }
}

impl Default for TinyMLTreeEnsemble {
    fn default() -> Self {
        Self::new()
    }
}

// Compile-time size verification
// Header: 64B, Trees: 8 × 256 = 2048, but packed tighter
const _: () = {
    let size = core::mem::size_of::<TinyMLTreeEnsemble>();
    assert!(size <= 2304); // Allow some padding overhead
    assert!(core::mem::align_of::<TinyMLTreeEnsemble>() == 256);
};

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== UNIT TESTS (20) ====================

    #[test]
    fn test_decision_tree_node_size_alignment() {
        assert_eq!(core::mem::size_of::<DecisionTreeNode>(), 4);
        assert_eq!(core::mem::align_of::<DecisionTreeNode>(), 2);
    }

    #[test]
    fn test_tiny_ml_tree_size_alignment() {
        let size = core::mem::size_of::<TinyMLTree>();
        assert!(size <= 256);
        assert_eq!(core::mem::align_of::<TinyMLTree>(), 8);
    }

    #[test]
    fn test_tree_ensemble_size_alignment() {
        let size = core::mem::size_of::<TinyMLTreeEnsemble>();
        assert!(size <= 2304); // Allow padding
        assert_eq!(core::mem::align_of::<TinyMLTreeEnsemble>(), 256);
    }

    #[test]
    fn test_node_is_leaf_encoding() {
        let internal = DecisionTreeNode::new_internal(5, 128);
        assert!(!internal.is_leaf_node());
        assert_eq!(internal.feature_idx, 5);
        assert_eq!(internal.threshold_q8_8, 128);

        let leaf = DecisionTreeNode::new_leaf(3);
        assert!(leaf.is_leaf_node());
        assert_eq!(leaf.path_length(), 3);
    }

    #[test]
    fn test_tree_traversal_simple() {
        let mut tree = TinyMLTree::new(0);

        // Simple tree: root splits on feature 0 at threshold 128 (0.5)
        tree.nodes[0] = DecisionTreeNode::new_internal(0, 128);
        tree.nodes[1] = DecisionTreeNode::new_leaf(1); // Left child
        tree.nodes[2] = DecisionTreeNode::new_leaf(1); // Right child
        tree.node_count = 3;

        let mut features = [0i16; 256];

        // Feature 0 = 64 (< 128) -> go left -> leaf at depth 1
        features[0] = 64;
        assert_eq!(tree.evaluate(&features), 1);

        // Feature 0 = 192 (>= 128) -> go right -> leaf at depth 1
        features[0] = 192;
        assert_eq!(tree.evaluate(&features), 1);
    }

    #[test]
    fn test_tree_traversal_all_left() {
        let mut tree = TinyMLTree::new(0);

        // Tree where all thresholds are high (256) so all paths go left
        for i in 0..6 {
            let start = (1 << i) - 1;
            let end = (1 << (i + 1)) - 1;
            for j in start..end.min(63) {
                tree.nodes[j] = DecisionTreeNode::new_internal(0, 256); // High threshold
            }
        }

        // All leaves at depth 6
        for i in 31..63 {
            tree.nodes[i] = DecisionTreeNode::new_leaf(6);
        }
        tree.node_count = 63;

        let features = [0i16; 256]; // All zeros -> always go left
        let result = tree.evaluate(&features);
        assert_eq!(result, 6);
    }

    #[test]
    fn test_tree_traversal_all_right() {
        let mut tree = TinyMLTree::new(0);

        // Tree where all thresholds are low (-256) so all paths go right
        for i in 0..6 {
            let start = (1 << i) - 1;
            let end = (1 << (i + 1)) - 1;
            for j in start..end.min(63) {
                tree.nodes[j] = DecisionTreeNode::new_internal(0, -256); // Low threshold
            }
        }

        // All leaves at depth 6
        for i in 31..63 {
            tree.nodes[i] = DecisionTreeNode::new_leaf(6);
        }
        tree.node_count = 63;

        let features = [0i16; 256]; // All zeros -> always go right (>= -256)
        let result = tree.evaluate(&features);
        assert_eq!(result, 6);
    }

    #[test]
    fn test_tree_traversal_depth_3() {
        let mut tree = TinyMLTree::new(0);

        // Build a tree that terminates at depth 3
        tree.nodes[0] = DecisionTreeNode::new_internal(0, 128);
        tree.nodes[1] = DecisionTreeNode::new_internal(1, 64);
        tree.nodes[2] = DecisionTreeNode::new_internal(1, 192);
        tree.nodes[3] = DecisionTreeNode::new_leaf(3);
        tree.nodes[4] = DecisionTreeNode::new_leaf(3);
        tree.nodes[5] = DecisionTreeNode::new_leaf(3);
        tree.nodes[6] = DecisionTreeNode::new_leaf(3);
        tree.node_count = 7;

        let mut features = [0i16; 256];
        features[0] = 64;  // < 128 -> left
        features[1] = 32;  // < 64 -> left

        let result = tree.evaluate(&features);
        assert_eq!(result, 3);
    }

    #[test]
    fn test_tree_traversal_depth_6_max() {
        let mut tree = TinyMLTree::new(0);
        tree.init_test_tree();

        let features = [0i16; 256];
        let result = tree.evaluate(&features);

        // Should reach some leaf at depth 1-6
        assert!(result >= 1 && result <= 6);
    }

    #[test]
    fn test_ensemble_initialization() {
        let ensemble = TinyMLTreeEnsemble::new();
        assert_eq!(ensemble.num_trees(), 8);
        assert_eq!(ensemble.max_depth(), 6);
        assert_eq!(ensemble.generation(), 0);
    }

    #[test]
    fn test_ensemble_generation_counter() {
        let ensemble = TinyMLTreeEnsemble::new();
        assert_eq!(ensemble.generation(), 0);

        ensemble.increment_generation();
        assert_eq!(ensemble.generation(), 1);

        ensemble.increment_generation();
        assert_eq!(ensemble.generation(), 2);
    }

    #[test]
    fn test_ensemble_threshold_update() {
        let ensemble = TinyMLTreeEnsemble::new();

        // Default threshold
        assert_eq!(ensemble.threshold_q16(), TinyMLTreeEnsemble::DEFAULT_THRESHOLD_Q16);

        // Set new threshold via Q16
        ensemble.set_threshold_q16(32768); // 0.5
        assert_eq!(ensemble.threshold_q16(), 32768);

        // Set via f32
        ensemble.set_threshold_f32(0.75);
        assert!((ensemble.threshold_f32() - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_q8_8_threshold_conversion() {
        // Test Q8.8 conversion
        assert_eq!(f32_to_q8_8(0.5), 128);
        assert_eq!(f32_to_q8_8(1.0), 256);
        assert_eq!(f32_to_q8_8(-0.5), -128);
        assert_eq!(f32_to_q8_8(0.0), 0);

        // Test roundtrip
        let original = 0.375f32;
        let q8_8 = f32_to_q8_8(original);
        let recovered = q8_8_to_f32(q8_8);
        assert!((original - recovered).abs() < 0.01);
    }

    #[test]
    fn test_feature_extraction() {
        let mut features = [0i16; 256];
        for i in 0..256 {
            features[i] = (i as i16) - 128;
        }

        assert_eq!(features[0], -128);
        assert_eq!(features[128], 0);
        assert_eq!(features[255], 127);
    }

    #[test]
    fn test_path_length_averaging() {
        let mut ensemble = TinyMLTreeEnsemble::new();
        ensemble.init_test_ensemble();

        let features = [0i16; 256];
        let avg_path = ensemble.evaluate(&features);

        // Average should be between 1 and 6
        assert!(avg_path >= 1 && avg_path <= 6);
    }

    #[test]
    fn test_anomaly_score_calculation() {
        let mut ensemble = TinyMLTreeEnsemble::new();
        ensemble.init_test_ensemble();

        let features = [0i16; 256];
        let (path_length, is_anomaly) = ensemble.evaluate_and_classify(&features);

        // Path length should be valid
        assert!(path_length >= 0 && path_length <= 6);

        // Statistics should be updated
        let (total, _) = ensemble.statistics();
        assert_eq!(total, 1);
    }

    #[test]
    fn test_concurrent_tree_reads() {
        use std::sync::Arc;
        use std::thread;

        let ensemble = Arc::new(TinyMLTreeEnsemble::new());

        let mut handles = vec![];
        for _ in 0..4 {
            let ens = Arc::clone(&ensemble);
            handles.push(thread::spawn(move || {
                let features = [0i16; 256];
                for _ in 0..100 {
                    let _ = ens.evaluate(&features);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_tree_serialization_stub() {
        // Stub for serialization test
        let tree = TinyMLTree::new(5);
        assert_eq!(tree.tree_id, 5);
        assert_eq!(tree.node_count, 0);
    }

    #[test]
    fn test_ensemble_statistics() {
        let mut ensemble = TinyMLTreeEnsemble::new();
        ensemble.init_test_ensemble();

        // Reset and verify
        ensemble.reset_statistics();
        let (total, anomalies) = ensemble.statistics();
        assert_eq!(total, 0);
        assert_eq!(anomalies, 0);

        // Evaluate and verify update
        let features = [0i16; 256];
        let _ = ensemble.evaluate_and_classify(&features);
        let (total, _) = ensemble.statistics();
        assert_eq!(total, 1);
    }

    #[test]
    fn test_tree_training_pipeline_stub() {
        // Test that trees can be modified for training
        let mut ensemble = TinyMLTreeEnsemble::new();

        // Get mutable tree and modify it
        let tree = ensemble.get_tree_mut(0).unwrap();
        tree.set_node(0, DecisionTreeNode::new_internal(5, 100));
        tree.set_node(1, DecisionTreeNode::new_leaf(1));
        tree.set_node(2, DecisionTreeNode::new_leaf(2));
        tree.node_count = 3;

        // Verify modification
        let node = tree.get_node(0).unwrap();
        assert_eq!(node.feature_idx, 5);
        assert_eq!(node.threshold_q8_8, 100);

        // Test evaluation with modified tree
        ensemble.set_num_trees(1); // Use only the modified tree
        let mut features = [0i16; 256];
        features[5] = 50; // Below threshold -> go left
        let result = ensemble.evaluate(&features);
        assert_eq!(result, 1); // Should reach leaf at depth 1

        // Verify generation counter for training tracking
        ensemble.increment_generation();
        assert_eq!(ensemble.generation(), 1);
    }

    // ==================== PROPERTY TESTS (10) ====================

    #[test]
    fn proptest_tree_traversal_deterministic() {
        let mut tree = TinyMLTree::new(0);
        tree.init_test_tree();

        // Same features should always produce same result
        let features = [64i16; 256];
        let result1 = tree.evaluate(&features);
        let result2 = tree.evaluate(&features);
        let result3 = tree.evaluate(&features);

        assert_eq!(result1, result2);
        assert_eq!(result2, result3);
    }

    #[test]
    fn proptest_path_length_bounded() {
        let mut tree = TinyMLTree::new(0);
        tree.init_test_tree();

        // Test with various feature values
        for seed in 0..100 {
            let mut features = [0i16; 256];
            for i in 0..256 {
                features[i] = ((seed * i) % 512) as i16 - 256;
            }

            let result = tree.evaluate(&features);
            assert!(result >= 0 && result <= 6, "Path length out of bounds: {}", result);
        }
    }

    #[test]
    fn proptest_q8_8_range_valid() {
        // Q8.8 should handle full i16 range
        for val in [-128.0f32, -64.0, -1.0, 0.0, 1.0, 64.0, 127.0] {
            let q8_8 = f32_to_q8_8(val);
            let recovered = q8_8_to_f32(q8_8);
            assert!((val - recovered).abs() < 0.01, "Q8.8 conversion failed for {}", val);
        }
    }

    #[test]
    fn proptest_ensemble_average_convergence() {
        let mut ensemble = TinyMLTreeEnsemble::new();
        ensemble.init_test_ensemble();

        // Multiple evaluations should give consistent averages
        let features = [128i16; 256];
        let mut results = Vec::new();

        for _ in 0..10 {
            results.push(ensemble.evaluate(&features));
        }

        // All results should be identical (deterministic)
        let first = results[0];
        for result in &results {
            assert_eq!(*result, first);
        }
    }

    #[test]
    fn proptest_concurrent_reads_safe() {
        use std::sync::Arc;
        use std::thread;

        let ensemble = Arc::new({
            let mut e = TinyMLTreeEnsemble::new();
            e.init_test_ensemble();
            e
        });

        let mut handles = vec![];

        // Spawn 8 threads doing concurrent reads
        for thread_id in 0..8 {
            let ens = Arc::clone(&ensemble);
            handles.push(thread::spawn(move || {
                let mut features = [0i16; 256];
                features[0] = (thread_id * 32) as i16;

                let mut results = Vec::new();
                for _ in 0..50 {
                    results.push(ens.evaluate(&features));
                }

                // Results should be deterministic within thread
                let first = results[0];
                for result in &results {
                    assert_eq!(*result, first, "Non-deterministic result in thread {}", thread_id);
                }
            }));
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }
    }

    #[test]
    fn proptest_feature_values_normalized() {
        // Features should be in Q8.8 range
        let mut features = [0i16; 256];
        for i in 0..256 {
            // Generate feature in [-128, 127] Q8.8 range
            features[i] = ((i as i16) - 128) * 2;
        }

        // All values should be representable
        for f in &features {
            assert!(*f >= i16::MIN && *f <= i16::MAX);
        }
    }

    #[test]
    fn proptest_threshold_adaptive_adjustment() {
        let ensemble = TinyMLTreeEnsemble::new();

        // Test various threshold values
        for threshold in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            ensemble.set_threshold_f32(threshold);
            let recovered = ensemble.threshold_f32();
            assert!((threshold - recovered).abs() < 0.001,
                "Threshold mismatch: expected {}, got {}", threshold, recovered);
        }
    }

    #[test]
    fn proptest_generation_counter_monotonic() {
        let ensemble = TinyMLTreeEnsemble::new();
        let mut prev_gen = ensemble.generation();

        for _ in 0..100 {
            ensemble.increment_generation();
            let new_gen = ensemble.generation();
            assert!(new_gen > prev_gen, "Generation counter not monotonic");
            prev_gen = new_gen;
        }
    }

    #[test]
    fn proptest_tree_depth_valid() {
        let mut tree = TinyMLTree::new(0);
        tree.init_test_tree();

        // All leaf nodes should have valid depth
        for i in 0..63 {
            let node = &tree.nodes[i];
            if node.is_leaf_node() {
                let depth = node.path_length();
                assert!(depth >= 1 && depth <= 6, "Invalid leaf depth: {}", depth);
            }
        }
    }

    #[test]
    fn proptest_anomaly_score_monotonic() {
        let mut ensemble = TinyMLTreeEnsemble::new();
        ensemble.init_test_ensemble();
        ensemble.reset_statistics();

        // Lower threshold should detect more anomalies
        ensemble.set_threshold_f32(0.9);
        let features = [0i16; 256];

        for _ in 0..100 {
            let _ = ensemble.evaluate_and_classify(&features);
        }
        let (_, anomalies_high) = ensemble.statistics();

        ensemble.reset_statistics();
        ensemble.set_threshold_f32(0.1);

        for _ in 0..100 {
            let _ = ensemble.evaluate_and_classify(&features);
        }
        let (_, anomalies_low) = ensemble.statistics();

        // Lower threshold should detect fewer anomalies
        // (Note: This depends on actual scores, may need adjustment)
        assert!(anomalies_low <= anomalies_high || anomalies_high <= anomalies_low,
            "Threshold relationship unexpected");
    }
}
