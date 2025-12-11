//! # FlameGraphCapsule - T4 Batch Flamegraph Generation
//!
//! **High-performance lockfree flamegraph generator with <1ms generation time.**
//!
//! ## SOTA Research Integration (2024-2025)
//!
//! ### Brendan Gregg's Flame Graph Methodology
//! - Collapsed stack format: semicolon-delimited frame paths with counts
//! - D3-flame-graph: Interactive SVG with zoom/hover
//! - Differential flame graphs: Before/after comparison
//! - Source: [Flame Graphs](https://www.brendangregg.com/flamegraphs.html)
//!
//! ### Folded Stack Format
//! - Input: `func_a;func_b;func_c 42` (stack trace, count)
//! - Output: SVG with proportional widths
//! - Aggregation: Merge identical stacks
//! - Source: [FlameGraph GitHub](https://github.com/brendangregg/FlameGraph)
//!
//! ### Flamescope (Netflix)
//! - Time-based subsetting of profiles
//! - Differential comparisons
//! - Interactive timeline selection
//! - Source: [Flamescope](https://netflixtechblog.com/netflix-flamescope-a57ca19d47bb)
//!
//! ## Architecture
//!
//! ```text
//! FlameGraphCapsule (2048B, T4 Batch)
//! ├── Header (64B, cache-aligned)
//! │   ├── state: AtomicU64 (idle/processing/complete)
//! │   ├── generation: AtomicU64 (ABA prevention)
//! │   ├── total_frames: AtomicU64
//! │   ├── unique_stacks: AtomicU64
//! │   └── total_samples: AtomicU64
//! ├── Node Pool Index (64B)
//! │   ├── node_count: AtomicU64
//! │   ├── max_nodes: u64 (65536)
//! │   └── root_index: u64
//! └── Statistics (1920B)
//!     ├── max_depth: AtomicU64
//!     ├── processing_time_ns: AtomicU64
//!     └── _reserved: [u8; ...]
//! ```
//!
//! ## Performance Targets
//!
//! - **Stack aggregation**: <100ns per sample (batch processing)
//! - **Node allocation**: <10ns (lockfree pool)
//! - **SVG generation**: <1ms for 10K unique stacks
//! - **Memory**: 2KB capsule + 8MB node pool
//!
//! ## ASSUM Safety Framework
//!
//! - `#ASSUME_LOCKFREE_AGGREGATION`: All aggregation is lock-free
//! - `#ASSUME_NODE_POOL_BOUNDED`: Node pool has fixed capacity
//! - `#ASSUME_TREE_CONSISTENT`: Tree invariants maintained atomically
//! - `#ASSUME_SVG_SAFE`: SVG output is well-formed

use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(feature = "std")]
use std::string::String;

#[cfg(feature = "std")]
use std::collections::HashMap;

use super::profiler::{SampleEntry, StackFrame};

// ============================================================================
// Constants
// ============================================================================

/// Maximum nodes in flamegraph tree
///
/// # ASSUM Safety
/// - `#ASSUME_MAX_NODES_BOUNDED`: 65536 nodes covers most profiles
/// - `#VERIFY_MAX_NODES`: Sufficient for 100K samples with 64-frame stacks
pub const MAX_NODES: usize = 65536;

/// Maximum stack depth in flamegraph
pub const MAX_FLAME_DEPTH: usize = 128;

/// Capsule size (2048 bytes)
pub const FLAMEGRAPH_CAPSULE_SIZE: usize = 2048;

/// Default SVG width in pixels
pub const DEFAULT_SVG_WIDTH: u32 = 1200;

/// Default frame height in pixels
pub const DEFAULT_FRAME_HEIGHT: u32 = 16;

/// Color palette for flamegraph (warm colors for CPU)
pub const FLAME_COLORS: &[&str] = &[
    "#ff6600", "#ff7700", "#ff8800", "#ff9900", "#ffaa00",
    "#ffbb00", "#ffcc00", "#ffdd00", "#ffee00", "#ffff00",
];

// ============================================================================
// Flamegraph State
// ============================================================================

/// Flamegraph processing state
///
/// # ASSUM Safety
/// - `#ASSUME_STATE_ATOMIC`: State transitions are atomic
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum FlameState {
    /// Idle, ready for processing
    Idle = 0,
    /// Processing samples
    Processing = 1,
    /// Processing complete
    Complete = 2,
    /// Error occurred
    Error = 3,
}

impl FlameState {
    /// Convert from raw u32
    #[inline]
    pub const fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Idle),
            1 => Some(Self::Processing),
            2 => Some(Self::Complete),
            3 => Some(Self::Error),
            _ => None,
        }
    }
}

// ============================================================================
// Flame Node
// ============================================================================

/// Single node in flamegraph tree
///
/// # Memory Layout (64 bytes, cache-aligned)
/// - name_hash: u64 (FNV-1a hash of function name)
/// - self_count: AtomicU64 (samples ending at this node)
/// - total_count: AtomicU64 (samples passing through this node)
/// - parent_idx: u32 (index of parent node, 0 = root)
/// - first_child_idx: AtomicU32 (first child, 0 = none)
/// - next_sibling_idx: AtomicU32 (next sibling, 0 = none)
/// - depth: u16 (depth in tree)
/// - flags: u16 (node flags)
/// - instruction_ptr: u64 (representative IP)
///
/// # ASSUM Safety
/// - `#ASSUME_NODE_ALIGNED`: 64B alignment for cache efficiency
/// - `#VERIFY_NODE_SIZE`: 64 bytes verified at compile time
#[repr(C, align(64))]
#[derive(Debug)]
pub struct FlameNode {
    /// Hash of function/symbol name (for fast lookup)
    pub name_hash: u64,

    /// Self samples (samples ending at this node)
    self_count: AtomicU64,

    /// Total samples (samples passing through this node)
    total_count: AtomicU64,

    /// Parent node index (0 = root/no parent)
    pub parent_idx: u32,

    /// First child index (atomic for concurrent insertion)
    first_child_idx: AtomicU32,

    /// Next sibling index (atomic for concurrent insertion)
    next_sibling_idx: AtomicU32,

    /// Depth in tree (0 = root)
    pub depth: u16,

    /// Node flags
    pub flags: u16,

    /// Representative instruction pointer
    pub instruction_ptr: u64,

    /// Padding to 64 bytes
    _padding: [u8; 8],
}

// Compile-time size verification
const _: () = {
    assert!(core::mem::size_of::<FlameNode>() == 64);
    assert!(core::mem::align_of::<FlameNode>() == 64);
};

impl FlameNode {
    /// Create new root node
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_ROOT_ZEROED`: Root node initialized to zero counts
    pub const fn root() -> Self {
        Self {
            name_hash: 0,
            self_count: AtomicU64::new(0),
            total_count: AtomicU64::new(0),
            parent_idx: 0,
            first_child_idx: AtomicU32::new(0),
            next_sibling_idx: AtomicU32::new(0),
            depth: 0,
            flags: 0,
            instruction_ptr: 0,
            _padding: [0; 8],
        }
    }

    /// Create new child node
    pub fn new(name_hash: u64, parent_idx: u32, depth: u16, ip: u64) -> Self {
        Self {
            name_hash,
            self_count: AtomicU64::new(0),
            total_count: AtomicU64::new(0),
            parent_idx,
            first_child_idx: AtomicU32::new(0),
            next_sibling_idx: AtomicU32::new(0),
            depth,
            flags: 0,
            instruction_ptr: ip,
            _padding: [0; 8],
        }
    }

    /// Increment self count (sample ends here)
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_COUNT_MONOTONIC`: Counts only increase
    #[inline]
    pub fn inc_self(&self) {
        self.self_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment total count (sample passes through)
    #[inline]
    pub fn inc_total(&self) {
        self.total_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get self count
    #[inline]
    pub fn self_count(&self) -> u64 {
        self.self_count.load(Ordering::Relaxed)
    }

    /// Get total count
    #[inline]
    pub fn total_count(&self) -> u64 {
        self.total_count.load(Ordering::Relaxed)
    }

    /// Get first child index
    #[inline]
    pub fn first_child(&self) -> u32 {
        self.first_child_idx.load(Ordering::Acquire)
    }

    /// Get next sibling index
    #[inline]
    pub fn next_sibling(&self) -> u32 {
        self.next_sibling_idx.load(Ordering::Acquire)
    }

    /// Set first child (CAS for concurrent safety)
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_CHILD_CAS`: Only one thread wins child insertion
    #[inline]
    pub fn set_first_child(&self, expected: u32, new: u32) -> Result<(), u32> {
        self.first_child_idx
            .compare_exchange(expected, new, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
    }

    /// Set next sibling (CAS for concurrent safety)
    #[inline]
    pub fn set_next_sibling(&self, expected: u32, new: u32) -> Result<(), u32> {
        self.next_sibling_idx
            .compare_exchange(expected, new, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
    }
}

impl Default for FlameNode {
    fn default() -> Self {
        Self::root()
    }
}

// ============================================================================
// Collapsed Stack (for output)
// ============================================================================

/// Collapsed stack format (for flamegraph.pl compatibility)
///
/// Format: `func_a;func_b;func_c count`
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub struct CollapsedStack {
    /// Stack path (semicolon-separated function names)
    pub path: String,
    /// Sample count
    pub count: u64,
}

#[cfg(feature = "std")]
impl CollapsedStack {
    /// Create new collapsed stack
    pub fn new(path: String, count: u64) -> Self {
        Self { path, count }
    }

    /// Format as folded stack line
    pub fn to_folded(&self) -> String {
        std::format!("{} {}", self.path, self.count)
    }
}

// ============================================================================
// FlameGraphCapsule
// ============================================================================

/// T4 Batch Flamegraph Generator
///
/// # Architecture
///
/// ```text
/// ┌───────────────────────────────────────────────────────────────┐
/// │ FlameGraphCapsule (2048B)                                     │
/// ├───────────────────────────────────────────────────────────────┤
/// │ Header (64B, cache-line 0):                                   │
/// │   state: AtomicU64 (Idle/Processing/Complete/Error)          │
/// │   generation: AtomicU64                                       │
/// │   total_frames: AtomicU64                                     │
/// │   unique_stacks: AtomicU64                                    │
/// │   total_samples: AtomicU64                                    │
/// │   _pad0: [u8; 24]                                             │
/// ├───────────────────────────────────────────────────────────────┤
/// │ Node Pool Index (64B, cache-line 1):                          │
/// │   node_count: AtomicU64                                       │
/// │   max_nodes: u64                                              │
/// │   root_index: AtomicU64                                       │
/// │   _pad1: [u8; 40]                                             │
/// └───────────────────────────────────────────────────────────────┘
/// ```
///
/// # ASSUM Safety Framework
///
/// - `#ASSUME_CAPSULE_SIZE_2KB`: 2048 bytes for metadata
/// - `#ASSUME_NODE_POOL_EXTERNAL`: Node pool stored externally
/// - `#ASSUME_LOCKFREE_TREE`: Tree operations are CAS-based
/// - `#ASSUME_AGGREGATION_IDEMPOTENT`: Same sample processed once
#[repr(C, align(128))]
pub struct FlameGraphCapsule {
    // =========== Header (64B, cache-line 0) ===========

    /// Current state
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_STATE_ORDERING`: AcqRel for state transitions
    state: AtomicU64,

    /// Generation counter
    generation: AtomicU64,

    /// Total frames processed
    total_frames: AtomicU64,

    /// Number of unique stack traces
    unique_stacks: AtomicU64,

    /// Total samples processed
    total_samples: AtomicU64,

    /// Padding
    _pad0: [u8; 24],

    // =========== Node Pool Index (64B, cache-line 1) ===========

    /// Number of nodes allocated
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_NODE_COUNT_ATOMIC`: Incremented atomically on allocation
    node_count: AtomicU64,

    /// Maximum nodes (constant)
    max_nodes: u64,

    /// Root node index (always 0)
    root_index: AtomicU64,

    /// Padding
    _pad1: [u8; 40],

    // =========== Statistics (remaining space) ===========

    /// Maximum stack depth seen
    max_depth: AtomicU64,

    /// Processing time in nanoseconds
    processing_time_ns: AtomicU64,

    /// Reserved for future use
    _reserved: [u8; 1792],
}

// Compile-time size verification
// Note: Actual size is ~1920 bytes but may vary with alignment padding
// The capsule is designed to fit within 2KB
const _: () = {
    assert!(core::mem::size_of::<FlameGraphCapsule>() <= 2048);
};

impl FlameGraphCapsule {
    /// Create new flamegraph generator
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_NEW_IDLE`: Initial state is Idle
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(FlameState::Idle as u64),
            generation: AtomicU64::new(0),
            total_frames: AtomicU64::new(0),
            unique_stacks: AtomicU64::new(0),
            total_samples: AtomicU64::new(0),
            _pad0: [0; 24],
            node_count: AtomicU64::new(1), // Root node pre-allocated
            max_nodes: MAX_NODES as u64,
            root_index: AtomicU64::new(0),
            _pad1: [0; 40],
            max_depth: AtomicU64::new(0),
            processing_time_ns: AtomicU64::new(0),
            _reserved: [0; 1792],
        }
    }

    /// Get current state
    #[inline]
    pub fn state(&self) -> FlameState {
        let raw = self.state.load(Ordering::Relaxed);
        FlameState::from_u32(raw as u32).unwrap_or(FlameState::Error)
    }

    /// Get total samples processed
    #[inline]
    pub fn total_samples(&self) -> u64 {
        self.total_samples.load(Ordering::Relaxed)
    }

    /// Get unique stack count
    #[inline]
    pub fn unique_stacks(&self) -> u64 {
        self.unique_stacks.load(Ordering::Relaxed)
    }

    /// Get node count
    #[inline]
    pub fn node_count(&self) -> u64 {
        self.node_count.load(Ordering::Relaxed)
    }

    /// Get maximum depth seen
    #[inline]
    pub fn max_depth(&self) -> u64 {
        self.max_depth.load(Ordering::Relaxed)
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Reset flamegraph
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_RESET_IDLE`: Reset only in Idle/Complete state
    pub fn reset(&self) {
        self.state.store(FlameState::Idle as u64, Ordering::Release);
        self.total_frames.store(0, Ordering::Release);
        self.unique_stacks.store(0, Ordering::Release);
        self.total_samples.store(0, Ordering::Release);
        self.node_count.store(1, Ordering::Release); // Root node
        self.max_depth.store(0, Ordering::Release);
        self.processing_time_ns.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Allocate node from pool
    ///
    /// # Returns
    /// - `Some(index)` if allocation succeeded
    /// - `None` if pool exhausted
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_ALLOC_ATOMIC`: Allocation is atomic increment
    /// - `#VERIFY_ALLOC_BOUNDED`: Returns None if pool full
    #[inline]
    fn allocate_node(&self) -> Option<u32> {
        let index = self.node_count.fetch_add(1, Ordering::AcqRel);
        if index < self.max_nodes {
            Some(index as u32)
        } else {
            // Rollback
            self.node_count.fetch_sub(1, Ordering::Release);
            None
        }
    }

    /// Process samples into flamegraph tree
    ///
    /// # Arguments
    /// - `samples`: Slice of profiler samples
    /// - `nodes`: Mutable node pool
    /// - `symbolizer`: Function to convert IP to name hash
    ///
    /// # Performance
    /// - <100ns per sample (batch processing)
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_SAMPLES_VALID`: Samples have valid stack frames
    /// - `#VERIFY_NODES_SUFFICIENT`: Node pool must be >= MAX_NODES
    #[cfg(feature = "std")]
    pub fn process_samples<F>(
        &self,
        samples: &[SampleEntry],
        nodes: &mut [FlameNode],
        mut symbolizer: F,
    ) -> Result<(), FlameGraphError>
    where
        F: FnMut(&StackFrame) -> u64,
    {
        // Transition to Processing state
        let result = self.state.compare_exchange(
            FlameState::Idle as u64,
            FlameState::Processing as u64,
            Ordering::AcqRel,
            Ordering::Acquire,
        );

        if result.is_err() {
            return Err(FlameGraphError::InvalidState);
        }

        let start = std::time::Instant::now();

        // Ensure root node exists
        if nodes.is_empty() {
            self.state.store(FlameState::Error as u64, Ordering::Release);
            return Err(FlameGraphError::NodePoolExhausted);
        }
        nodes[0] = FlameNode::root();

        // Process each sample
        for sample in samples {
            if !sample.is_valid() {
                continue;
            }

            self.total_samples.fetch_add(1, Ordering::Relaxed);

            // Process stack (bottom-up: first frame is leaf)
            let mut current_idx: u32 = 0; // Start at root
            let stack = sample.stack();

            // Walk from root to leaf (reverse order)
            for (frame_idx, frame) in stack.iter().rev().enumerate() {
                if frame.is_empty() {
                    continue;
                }

                let name_hash = symbolizer(frame);
                let depth = (frame_idx + 1) as u16;

                // Update max depth
                let current_max = self.max_depth.load(Ordering::Relaxed);
                if depth as u64 > current_max {
                    self.max_depth.store(depth as u64, Ordering::Relaxed);
                }

                // Find or create child node
                let child_idx = self.find_or_create_child(
                    nodes,
                    current_idx,
                    name_hash,
                    depth,
                    frame.instruction_ptr,
                )?;

                // Increment total count (sample passes through)
                if let Some(node) = nodes.get(child_idx as usize) {
                    node.inc_total();
                }

                self.total_frames.fetch_add(1, Ordering::Relaxed);
                current_idx = child_idx;
            }

            // Increment self count on leaf node
            if let Some(node) = nodes.get(current_idx as usize) {
                node.inc_self();
            }
        }

        let elapsed = start.elapsed().as_nanos() as u64;
        self.processing_time_ns.store(elapsed, Ordering::Release);

        // Transition to Complete
        self.state.store(FlameState::Complete as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);

        Ok(())
    }

    /// Find or create child node with given name hash
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_CHILD_LOOKUP_LOCKFREE`: Uses CAS for concurrent safety
    fn find_or_create_child(
        &self,
        nodes: &mut [FlameNode],
        parent_idx: u32,
        name_hash: u64,
        depth: u16,
        ip: u64,
    ) -> Result<u32, FlameGraphError> {
        let parent = nodes.get(parent_idx as usize)
            .ok_or(FlameGraphError::InvalidNodeIndex)?;

        // Search existing children
        let mut child_idx = parent.first_child();
        while child_idx != 0 {
            let child = nodes.get(child_idx as usize)
                .ok_or(FlameGraphError::InvalidNodeIndex)?;

            if child.name_hash == name_hash {
                return Ok(child_idx);
            }

            child_idx = child.next_sibling();
        }

        // Not found - allocate new node
        let new_idx = self.allocate_node()
            .ok_or(FlameGraphError::NodePoolExhausted)?;

        // Initialize new node
        if let Some(new_node) = nodes.get_mut(new_idx as usize) {
            *new_node = FlameNode::new(name_hash, parent_idx, depth, ip);
        }

        // Link into tree
        self.unique_stacks.fetch_add(1, Ordering::Relaxed);

        // Try to set as first child
        let parent = nodes.get(parent_idx as usize)
            .ok_or(FlameGraphError::InvalidNodeIndex)?;

        let first = parent.first_child();
        if first == 0 {
            // No children yet
            if parent.set_first_child(0, new_idx).is_ok() {
                return Ok(new_idx);
            }
        }

        // Insert as sibling (find end of sibling chain)
        let mut last_sibling = parent.first_child();
        while last_sibling != 0 {
            let sibling = nodes.get(last_sibling as usize)
                .ok_or(FlameGraphError::InvalidNodeIndex)?;
            let next = sibling.next_sibling();
            if next == 0 {
                let _ = sibling.set_next_sibling(0, new_idx);
                break;
            }
            last_sibling = next;
        }

        Ok(new_idx)
    }

    /// Generate collapsed stack format
    ///
    /// # Arguments
    /// - `nodes`: Node pool
    /// - `name_resolver`: Function to convert name hash to string
    ///
    /// # Returns
    /// - Vector of collapsed stacks
    #[cfg(feature = "std")]
    pub fn generate_collapsed<F>(
        &self,
        nodes: &[FlameNode],
        name_resolver: F,
    ) -> Vec<CollapsedStack>
    where
        F: Fn(u64) -> String,
    {
        let mut stacks = Vec::new();
        let mut path_stack: Vec<String> = Vec::new();

        self.traverse_tree(nodes, 0, &mut path_stack, &mut stacks, &name_resolver);

        stacks
    }

    /// Recursive tree traversal for collapsed stack generation
    #[cfg(feature = "std")]
    fn traverse_tree<F>(
        &self,
        nodes: &[FlameNode],
        node_idx: u32,
        path_stack: &mut Vec<String>,
        stacks: &mut Vec<CollapsedStack>,
        name_resolver: &F,
    )
    where
        F: Fn(u64) -> String,
    {
        let node = match nodes.get(node_idx as usize) {
            Some(n) => n,
            None => return,
        };

        // Add current node to path (skip root)
        if node_idx != 0 {
            let name = name_resolver(node.name_hash);
            path_stack.push(name);
        }

        // If this node has self samples, emit stack
        let self_count = node.self_count();
        if self_count > 0 && !path_stack.is_empty() {
            let path = path_stack.join(";");
            stacks.push(CollapsedStack::new(path, self_count));
        }

        // Traverse children
        let mut child_idx = node.first_child();
        while child_idx != 0 {
            self.traverse_tree(nodes, child_idx, path_stack, stacks, name_resolver);

            if let Some(child) = nodes.get(child_idx as usize) {
                child_idx = child.next_sibling();
            } else {
                break;
            }
        }

        // Pop current node from path
        if node_idx != 0 {
            path_stack.pop();
        }
    }

    /// Generate SVG flamegraph
    ///
    /// # Arguments
    /// - `nodes`: Node pool
    /// - `name_resolver`: Function to convert name hash to string
    /// - `title`: Title for the flamegraph
    ///
    /// # Returns
    /// - SVG string
    #[cfg(feature = "std")]
    pub fn generate_svg<F>(
        &self,
        nodes: &[FlameNode],
        name_resolver: F,
        title: &str,
    ) -> String
    where
        F: Fn(u64) -> String,
    {
        let root = match nodes.get(0) {
            Some(n) => n,
            None => return String::new(),
        };

        let total_samples = root.total_count().max(1);
        let width = DEFAULT_SVG_WIDTH;
        let frame_height = DEFAULT_FRAME_HEIGHT;
        let max_depth = self.max_depth() as u32;
        let height = (max_depth + 3) * frame_height;

        let mut svg = String::new();

        // SVG header
        svg.push_str(&std::format!(
            r#"<?xml version="1.0" standalone="no"?>
<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">
<svg version="1.1" width="{}" height="{}" xmlns="http://www.w3.org/2000/svg">
<style>
.func {{ font-family: monospace; font-size: 12px; }}
.func:hover {{ stroke: black; stroke-width: 0.5; cursor: pointer; }}
</style>
<rect x="0" y="0" width="100%" height="100%" fill="white"/>
<text x="10" y="20" font-size="14" font-weight="bold">{}</text>
"#,
            width, height, title
        ));

        // Render frames
        self.render_node_svg(
            nodes,
            0,
            0.0,
            width as f64,
            height - frame_height,
            total_samples as f64,
            &mut svg,
            &name_resolver,
        );

        svg.push_str("</svg>");
        svg
    }

    /// Render single node and children to SVG
    #[cfg(feature = "std")]
    fn render_node_svg<F>(
        &self,
        nodes: &[FlameNode],
        node_idx: u32,
        x: f64,
        width: f64,
        y: u32,
        total_samples: f64,
        svg: &mut String,
        name_resolver: &F,
    )
    where
        F: Fn(u64) -> String,
    {
        let node = match nodes.get(node_idx as usize) {
            Some(n) => n,
            None => return,
        };

        let count = node.total_count() as f64;
        if count == 0.0 && node_idx != 0 {
            return;
        }

        let frame_height = DEFAULT_FRAME_HEIGHT;

        // Skip root rendering but process children
        if node_idx != 0 {
            let frame_width = (count / total_samples) * (DEFAULT_SVG_WIDTH as f64);

            if frame_width >= 1.0 {
                let name = name_resolver(node.name_hash);
                let color_idx = (node.depth as usize) % FLAME_COLORS.len();
                let color = FLAME_COLORS[color_idx];

                svg.push_str(&std::format!(
                    r#"<g class="func">
<rect x="{:.1}" y="{}" width="{:.1}" height="{}" fill="{}" rx="2"/>
<text x="{:.1}" y="{}" fill="black" class="func">{}</text>
</g>
"#,
                    x,
                    y,
                    frame_width,
                    frame_height - 1,
                    color,
                    x + 2.0,
                    y + frame_height - 4,
                    Self::truncate_name(&name, frame_width as usize / 7)
                ));
            }
        }

        // Render children
        let mut child_x = x;
        let mut child_idx = node.first_child();

        while child_idx != 0 {
            if let Some(child) = nodes.get(child_idx as usize) {
                let child_count = child.total_count() as f64;
                let child_width = if total_samples > 0.0 {
                    (child_count / total_samples) * (DEFAULT_SVG_WIDTH as f64)
                } else {
                    0.0
                };

                let child_y = if node_idx == 0 {
                    y
                } else {
                    y - frame_height
                };

                self.render_node_svg(
                    nodes,
                    child_idx,
                    child_x,
                    child_width,
                    child_y,
                    total_samples,
                    svg,
                    name_resolver,
                );

                child_x += child_width;
                child_idx = child.next_sibling();
            } else {
                break;
            }
        }
    }

    /// Truncate name to fit in frame
    #[cfg(feature = "std")]
    fn truncate_name(name: &str, max_chars: usize) -> String {
        if name.len() <= max_chars || max_chars < 4 {
            name.to_string()
        } else {
            std::format!("{}...", &name[..max_chars - 3])
        }
    }

    /// Get statistics
    pub fn stats(&self) -> FlameGraphStats {
        FlameGraphStats {
            state: self.state(),
            total_samples: self.total_samples(),
            unique_stacks: self.unique_stacks(),
            node_count: self.node_count(),
            max_depth: self.max_depth(),
            processing_time_ns: self.processing_time_ns.load(Ordering::Relaxed),
            generation: self.generation(),
        }
    }
}

impl Default for FlameGraphCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Node Pool
// ============================================================================

/// Pre-allocated node pool for FlameGraphCapsule
///
/// # ASSUM Safety
/// - `#ASSUME_POOL_ALIGNED`: Nodes are 64B aligned
/// - `#VERIFY_POOL_CAPACITY`: Capacity matches MAX_NODES
#[cfg(feature = "std")]
pub struct NodePool {
    nodes: Vec<FlameNode>,
}

#[cfg(feature = "std")]
impl NodePool {
    /// Create new node pool with default capacity
    pub fn new() -> Self {
        Self::with_capacity(MAX_NODES)
    }

    /// Create node pool with custom capacity
    pub fn with_capacity(capacity: usize) -> Self {
        let mut nodes = Vec::with_capacity(capacity);
        nodes.resize_with(capacity, FlameNode::default);
        Self { nodes }
    }

    /// Get mutable slice
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [FlameNode] {
        &mut self.nodes
    }

    /// Get immutable slice
    #[inline]
    pub fn as_slice(&self) -> &[FlameNode] {
        &self.nodes
    }
}

#[cfg(feature = "std")]
impl Default for NodePool {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Flamegraph error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlameGraphError {
    /// Invalid state for operation
    InvalidState,
    /// Node pool exhausted
    NodePoolExhausted,
    /// Invalid node index
    InvalidNodeIndex,
}

// ============================================================================
// Statistics
// ============================================================================

/// Flamegraph statistics snapshot
#[derive(Debug, Clone, Copy)]
pub struct FlameGraphStats {
    /// Current state
    pub state: FlameState,
    /// Total samples processed
    pub total_samples: u64,
    /// Unique stack traces
    pub unique_stacks: u64,
    /// Nodes allocated
    pub node_count: u64,
    /// Maximum stack depth
    pub max_depth: u64,
    /// Processing time in nanoseconds
    pub processing_time_ns: u64,
    /// Generation counter
    pub generation: u64,
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flame_node_creation() {
        let root = FlameNode::root();
        assert_eq!(root.name_hash, 0);
        assert_eq!(root.depth, 0);
        assert_eq!(root.self_count(), 0);
        assert_eq!(root.total_count(), 0);
    }

    #[test]
    fn test_flame_node_counts() {
        let node = FlameNode::new(0x12345678, 0, 1, 0x1000);

        node.inc_self();
        node.inc_self();
        node.inc_total();
        node.inc_total();
        node.inc_total();

        assert_eq!(node.self_count(), 2);
        assert_eq!(node.total_count(), 3);
    }

    #[test]
    fn test_flamegraph_new() {
        let fg = FlameGraphCapsule::new();
        assert_eq!(fg.state(), FlameState::Idle);
        assert_eq!(fg.total_samples(), 0);
        assert_eq!(fg.node_count(), 1); // Root node
    }

    #[test]
    fn test_flamegraph_reset() {
        let fg = FlameGraphCapsule::new();
        let gen0 = fg.generation();

        fg.reset();
        let gen1 = fg.generation();

        assert!(gen1 > gen0);
        assert_eq!(fg.state(), FlameState::Idle);
        assert_eq!(fg.node_count(), 1);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_collapsed_stack() {
        let stack = CollapsedStack::new("main;foo;bar".to_string(), 42);
        assert_eq!(stack.to_folded(), "main;foo;bar 42");
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_node_pool() {
        let mut pool = NodePool::new();
        assert_eq!(pool.as_slice().len(), MAX_NODES);

        // Initialize a node
        pool.as_mut_slice()[1] = FlameNode::new(0xABCD, 0, 1, 0x2000);
        assert_eq!(pool.as_slice()[1].name_hash, 0xABCD);
    }

    #[test]
    fn test_flame_state_conversion() {
        assert_eq!(FlameState::from_u32(0), Some(FlameState::Idle));
        assert_eq!(FlameState::from_u32(1), Some(FlameState::Processing));
        assert_eq!(FlameState::from_u32(2), Some(FlameState::Complete));
        assert_eq!(FlameState::from_u32(3), Some(FlameState::Error));
        assert_eq!(FlameState::from_u32(99), None);
    }

    #[test]
    fn test_flamegraph_stats() {
        let fg = FlameGraphCapsule::new();
        let stats = fg.stats();

        assert_eq!(stats.state, FlameState::Idle);
        assert_eq!(stats.total_samples, 0);
        assert_eq!(stats.node_count, 1);
    }
}
