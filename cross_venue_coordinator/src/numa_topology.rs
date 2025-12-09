//! NUMA Topology and Cache-Optimized Layouts
//!
//! NUMA-aware memory allocation and cache optimization for
//! cross-venue coordination on multi-socket systems.

use core::alloc::Layout;
use crate::types::VenueId;

/// NUMA node identifier
pub type NumaNode = u32;

/// NUMA-aware allocation strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumaStrategy {
    /// Allocate on local NUMA node
    Local,
    /// Allocate on specific NUMA node
    Specific(NumaNode),
    /// Round-robin allocation across NUMA nodes
    RoundRobin,
    /// Interleaved allocation across all NUMA nodes
    Interleaved,
    /// First-touch allocation (use default system policy)
    FirstTouch,
}

/// Cache-optimized memory layout configuration
#[derive(Debug, Clone, PartialEq)]
pub struct CacheOptimizedLayout {
    /// Cache line size in bytes
    pub cache_line_size: usize,
    /// Page size in bytes
    pub page_size: usize,
    /// Huge page size in bytes (if available)
    pub huge_page_size: Option<usize>,
    /// Preferred NUMA strategy
    pub numa_strategy: NumaStrategy,
    /// Enable memory prefetching
    pub enable_prefetch: bool,
    /// Memory alignment requirements
    pub alignment: usize,
}

impl Default for CacheOptimizedLayout {
    fn default() -> Self {
        Self {
            cache_line_size: 64,  // x86_64 cache line size
            page_size: 4096,      // Standard page size
            huge_page_size: Some(2 * 1024 * 1024), // 2MB huge pages
            numa_strategy: NumaStrategy::FirstTouch,
            enable_prefetch: true,
            alignment: 128,       // Conservative alignment for coordination structures
        }
    }
}

impl CacheOptimizedLayout {
    /// Create layout optimized for Intel systems
    pub fn intel_optimized() -> Self {
        Self {
            cache_line_size: 64,
            page_size: 4096,
            huge_page_size: Some(2 * 1024 * 1024),
            numa_strategy: NumaStrategy::Local,
            enable_prefetch: true,
            alignment: 128,
        }
    }

    /// Create layout optimized for AMD systems
    pub fn amd_optimized() -> Self {
        Self {
            cache_line_size: 64,
            page_size: 4096,
            huge_page_size: Some(2 * 1024 * 1024),
            numa_strategy: NumaStrategy::Interleaved,
            enable_prefetch: true,
            alignment: 128,
        }
    }

    /// Calculate optimal layout for venue array
    pub fn venue_array_layout(&self, num_venues: usize) -> Layout {
        let venue_size = 128; // Each venue is cache-line aligned
        let total_size = num_venues * venue_size;
        let aligned_size = self.align_to_page(total_size);

        Layout::from_size_align(aligned_size, self.alignment)
            .expect("Valid layout")
    }

    /// Calculate optimal layout for coordination state
    pub fn coordination_state_layout(&self) -> Layout {
        let state_size = 256; // Coordination state structure size
        let aligned_size = self.align_to_cache_line(state_size);

        Layout::from_size_align(aligned_size, self.cache_line_size)
            .expect("Valid layout")
    }

    /// Align size to cache line boundary
    pub fn align_to_cache_line(&self, size: usize) -> usize {
        (size + self.cache_line_size - 1) & !(self.cache_line_size - 1)
    }

    /// Align size to page boundary
    pub fn align_to_page(&self, size: usize) -> usize {
        (size + self.page_size - 1) & !(self.page_size - 1)
    }

    /// Align size to huge page boundary (if available)
    pub fn align_to_huge_page(&self, size: usize) -> Option<usize> {
        self.huge_page_size.map(|huge_page_size| {
            (size + huge_page_size - 1) & !(huge_page_size - 1)
        })
    }

    /// Calculate memory layout for specific venue
    pub fn venue_layout(&self, venue_id: VenueId) -> VenueLayout {
        let base_offset = venue_id * 128; // 128 bytes per venue
        let cache_line_offset = base_offset / self.cache_line_size;

        VenueLayout {
            venue_id,
            offset: base_offset,
            size: 128,
            cache_line: cache_line_offset,
            numa_node: self.calculate_numa_node(venue_id),
            alignment: self.cache_line_size,
        }
    }

    /// Calculate optimal NUMA node for venue
    fn calculate_numa_node(&self, venue_id: VenueId) -> Option<NumaNode> {
        match self.numa_strategy {
            NumaStrategy::Local => None, // Use local node
            NumaStrategy::Specific(node) => Some(node),
            NumaStrategy::RoundRobin => {
                // Assume 2 NUMA nodes for simplicity
                Some((venue_id % 2) as NumaNode)
            }
            NumaStrategy::Interleaved => None, // Let system handle interleaving
            NumaStrategy::FirstTouch => None,
        }
    }
}

/// Venue-specific memory layout information
#[derive(Debug, Clone, PartialEq)]
pub struct VenueLayout {
    /// Venue identifier
    pub venue_id: VenueId,
    /// Memory offset from base address
    pub offset: usize,
    /// Memory size in bytes
    pub size: usize,
    /// Cache line number
    pub cache_line: usize,
    /// Preferred NUMA node
    pub numa_node: Option<NumaNode>,
    /// Memory alignment
    pub alignment: usize,
}

impl VenueLayout {
    /// Check if venue shares cache line with another venue
    pub fn shares_cache_line(&self, other: &VenueLayout) -> bool {
        self.cache_line == other.cache_line
    }

    /// Calculate distance to another venue in cache lines
    pub fn cache_line_distance(&self, other: &VenueLayout) -> usize {
        if self.cache_line >= other.cache_line {
            self.cache_line - other.cache_line
        } else {
            other.cache_line - self.cache_line
        }
    }

    /// Check if venue is on same NUMA node as another venue
    pub fn same_numa_node(&self, other: &VenueLayout) -> bool {
        match (self.numa_node, other.numa_node) {
            (Some(a), Some(b)) => a == b,
            (None, None) => true, // Both use system default
            _ => false,
        }
    }
}

/// NUMA-aware allocation interface
pub struct NumaAwareAllocation {
    /// Cache layout configuration
    layout_config: CacheOptimizedLayout,
    /// Current allocation strategy
    current_strategy: NumaStrategy,
    /// Allocation statistics
    stats: AllocationStats,
}

/// Allocation statistics
#[derive(Debug, Clone, Default)]
pub struct AllocationStats {
    /// Total allocations performed
    pub total_allocations: u64,
    /// Allocations per NUMA node
    pub numa_allocations: [u64; 8], // Support up to 8 NUMA nodes
    /// Cache-aligned allocations
    pub cache_aligned_allocations: u64,
    /// Huge page allocations
    pub huge_page_allocations: u64,
    /// Failed allocations
    pub failed_allocations: u64,
}

impl NumaAwareAllocation {
    /// Create new NUMA-aware allocator
    pub fn new(layout_config: CacheOptimizedLayout) -> Self {
        Self {
            current_strategy: layout_config.numa_strategy,
            layout_config,
            stats: AllocationStats::default(),
        }
    }

    /// Allocate memory for venue array with optimal layout
    pub fn allocate_venue_array(&mut self, num_venues: usize) -> Result<VenueArrayAllocation, AllocationError> {
        let layout = self.layout_config.venue_array_layout(num_venues);

        // In a real implementation, this would use system NUMA APIs
        // For now, we simulate the allocation
        let allocation = VenueArrayAllocation {
            base_address: 0x1000_0000, // Simulated address
            size: layout.size(),
            alignment: layout.align(),
            numa_node: self.select_numa_node(),
            venue_layouts: (0..num_venues)
                .map(|venue_id| self.layout_config.venue_layout(venue_id))
                .collect(),
        };

        self.stats.total_allocations += 1;
        if layout.align() >= self.layout_config.cache_line_size {
            self.stats.cache_aligned_allocations += 1;
        }

        Ok(allocation)
    }

    /// Allocate memory for coordination state
    pub fn allocate_coordination_state(&mut self) -> Result<CoordinationStateAllocation, AllocationError> {
        let layout = self.layout_config.coordination_state_layout();

        let allocation = CoordinationStateAllocation {
            base_address: 0x2000_0000, // Simulated address
            size: layout.size(),
            alignment: layout.align(),
            numa_node: self.select_numa_node(),
        };

        self.stats.total_allocations += 1;
        self.stats.cache_aligned_allocations += 1;

        Ok(allocation)
    }

    /// Select optimal NUMA node based on strategy
    fn select_numa_node(&self) -> Option<NumaNode> {
        match self.current_strategy {
            NumaStrategy::Local => None, // Use current CPU's NUMA node
            NumaStrategy::Specific(node) => Some(node),
            NumaStrategy::RoundRobin => {
                // Simple round-robin between 2 nodes
                let total_allocs = self.stats.total_allocations;
                Some((total_allocs % 2) as NumaNode)
            }
            NumaStrategy::Interleaved => None, // Let system handle
            NumaStrategy::FirstTouch => None,
        }
    }

    /// Get allocation statistics
    pub fn stats(&self) -> &AllocationStats {
        &self.stats
    }

    /// Reset allocation statistics
    pub fn reset_stats(&mut self) {
        self.stats = AllocationStats::default();
    }

    /// Update NUMA strategy
    pub fn set_numa_strategy(&mut self, strategy: NumaStrategy) {
        self.current_strategy = strategy;
    }

    /// Get current layout configuration
    pub fn layout_config(&self) -> &CacheOptimizedLayout {
        &self.layout_config
    }
}

/// Venue array allocation result
#[derive(Debug)]
pub struct VenueArrayAllocation {
    /// Base memory address
    pub base_address: usize,
    /// Total allocated size
    pub size: usize,
    /// Memory alignment
    pub alignment: usize,
    /// NUMA node (if specified)
    pub numa_node: Option<NumaNode>,
    /// Per-venue layout information
    pub venue_layouts: Vec<VenueLayout>,
}

impl VenueArrayAllocation {
    /// Get layout for specific venue
    pub fn venue_layout(&self, venue_id: VenueId) -> Option<&VenueLayout> {
        self.venue_layouts.get(venue_id)
    }

    /// Check for cache line conflicts between venues
    pub fn check_cache_conflicts(&self) -> Vec<(VenueId, VenueId)> {
        let mut conflicts = Vec::new();

        for (i, layout_a) in self.venue_layouts.iter().enumerate() {
            for (j, layout_b) in self.venue_layouts.iter().enumerate() {
                if i < j && layout_a.shares_cache_line(layout_b) {
                    conflicts.push((i, j));
                }
            }
        }

        conflicts
    }

    /// Calculate total cache lines used
    pub fn total_cache_lines(&self) -> usize {
        self.venue_layouts
            .iter()
            .map(|layout| layout.cache_line)
            .max()
            .map(|max| max + 1)
            .unwrap_or(0)
    }

    /// Group venues by NUMA node
    pub fn venues_by_numa_node(&self) -> std::collections::HashMap<Option<NumaNode>, Vec<VenueId>> {
        let mut groups = std::collections::HashMap::new();

        for (venue_id, layout) in self.venue_layouts.iter().enumerate() {
            groups.entry(layout.numa_node).or_insert_with(Vec::new).push(venue_id);
        }

        groups
    }
}

/// Coordination state allocation result
#[derive(Debug)]
pub struct CoordinationStateAllocation {
    /// Base memory address
    pub base_address: usize,
    /// Allocated size
    pub size: usize,
    /// Memory alignment
    pub alignment: usize,
    /// NUMA node (if specified)
    pub numa_node: Option<NumaNode>,
}

/// Allocation errors
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AllocationError {
    /// Insufficient memory
    #[error("Insufficient memory for allocation of {size} bytes")]
    InsufficientMemory { size: usize },

    /// Invalid alignment
    #[error("Invalid alignment: {alignment}")]
    InvalidAlignment { alignment: usize },

    /// NUMA node not available
    #[error("NUMA node {node} not available")]
    NumaNodeUnavailable { node: NumaNode },

    /// System allocation failure
    #[error("System allocation failed: {reason}")]
    SystemFailure { reason: String },
}

/// Cache prefetch utilities
pub struct CachePrefetch;

impl CachePrefetch {
    /// Prefetch cache line for read access
    #[cfg(target_arch = "x86_64")]
    #[allow(unsafe_code)]
    pub fn prefetch_read(address: *const u8) {
        unsafe {
            core::arch::x86_64::_mm_prefetch(address as *const i8, core::arch::x86_64::_MM_HINT_T0);
        }
    }

    /// Prefetch cache line for write access
    #[cfg(target_arch = "x86_64")]
    #[allow(unsafe_code)]
    pub fn prefetch_write(address: *const u8) {
        unsafe {
            core::arch::x86_64::_mm_prefetch(address as *const i8, core::arch::x86_64::_MM_HINT_T1);
        }
    }

    /// Prefetch for non-temporal access (streaming)
    #[cfg(target_arch = "x86_64")]
    #[allow(unsafe_code)]
    pub fn prefetch_non_temporal(address: *const u8) {
        unsafe {
            core::arch::x86_64::_mm_prefetch(address as *const i8, core::arch::x86_64::_MM_HINT_NTA);
        }
    }

    /// No-op prefetch for unsupported architectures
    #[cfg(not(target_arch = "x86_64"))]
    pub fn prefetch_read(_address: *const u8) {
        // No-op
    }

    #[cfg(not(target_arch = "x86_64"))]
    pub fn prefetch_write(_address: *const u8) {
        // No-op
    }

    #[cfg(not(target_arch = "x86_64"))]
    pub fn prefetch_non_temporal(_address: *const u8) {
        // No-op
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_optimized_layout() {
        let layout = CacheOptimizedLayout::default();

        assert_eq!(layout.cache_line_size, 64);
        assert_eq!(layout.align_to_cache_line(100), 128);
        assert_eq!(layout.align_to_page(5000), 8192);

        let venue_layout = layout.venue_layout(0);
        assert_eq!(venue_layout.venue_id, 0);
        assert_eq!(venue_layout.offset, 0);
        assert_eq!(venue_layout.size, 128);
    }

    #[test]
    fn test_venue_layout() {
        let layout = CacheOptimizedLayout::default();
        let venue0 = layout.venue_layout(0);
        let venue1 = layout.venue_layout(1);

        assert!(!venue0.shares_cache_line(&venue1));
        assert_eq!(venue0.cache_line_distance(&venue1), 2); // 128 bytes apart = 2 cache lines
    }

    #[test]
    fn test_numa_aware_allocation() {
        let layout_config = CacheOptimizedLayout::intel_optimized();
        let mut allocator = NumaAwareAllocation::new(layout_config);

        let venue_array = allocator.allocate_venue_array(4).unwrap();
        assert_eq!(venue_array.venue_layouts.len(), 4);
        assert_eq!(venue_array.total_cache_lines(), 8); // 4 venues * 2 cache lines each

        let coord_state = allocator.allocate_coordination_state().unwrap();
        assert!(coord_state.size >= 256);

        let stats = allocator.stats();
        assert_eq!(stats.total_allocations, 2);
        assert_eq!(stats.cache_aligned_allocations, 2);
    }

    #[test]
    fn test_allocation_stats() {
        let mut stats = AllocationStats::default();
        stats.total_allocations = 10;
        stats.cache_aligned_allocations = 8;
        stats.failed_allocations = 1;

        assert_eq!(stats.total_allocations, 10);
        assert_eq!(stats.cache_aligned_allocations, 8);
    }

    #[test]
    fn test_cache_conflicts() {
        let layout = CacheOptimizedLayout::default();
        let mut allocator = NumaAwareAllocation::new(layout);

        let allocation = allocator.allocate_venue_array(16).unwrap();
        let conflicts = allocation.check_cache_conflicts();

        // With 128-byte venues and 64-byte cache lines, no conflicts expected
        assert!(conflicts.is_empty());
    }
}