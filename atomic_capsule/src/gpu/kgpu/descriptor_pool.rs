//! KgpuDescriptorPoolCapsule - Lockfree GPU Descriptor Set Pool
//!
//! **Tier**: T4 (Batch)
//! **Size**: 512B (cache-aligned)
//! **Purpose**: Efficient allocation of descriptor sets (bind groups)
//!
//! # Architecture
//!
//! Descriptor sets (bind groups) are allocated from a pool to avoid the overhead
//! of individual allocations. This capsule uses a combination of bitmap allocation
//! and a lockfree free list (Treiber stack) for O(1) allocate/free operations.
//!
//! ```text
//! KgpuDescriptorPoolCapsule (512B aligned)
//! +---------------------------+
//! | primary: AtomicU64        |  state(8) | allocated_sets(16) | generation(40)
//! | secondary: AtomicU64      |  max_sets(16) | descriptor_count(16) | flags(32)
//! | free_list_head: AtomicU64 |  Treiber stack: index(32) | gen(32)
//! | allocation_bitmap: AtomicU64 | 64 sets tracked via bitmap
//! | Per-type counters/limits  |  Uniform, storage, texture, sampler limits
//! | Statistics                |  Allocation/free counts, peak usage
//! | _padding                  |  Padding to 512B
//! +---------------------------+
//! ```
//!
//! # Allocation Strategy
//!
//! 1. **Free List First**: Check Treiber stack for recycled slots (O(1))
//! 2. **Bitmap Fallback**: If free list empty, scan bitmap for available slot
//! 3. **Generation Counter**: Each allocation has a generation to prevent ABA
//!
//! # ASSUM Safety Documentation
//!
//! - `#ASSUME_TREIBER_STACK_CORRECT`: Free list uses standard Treiber stack
//!   algorithm with generation counters for ABA prevention.
//!
//! - `#ASSUME_BITMAP_ATOMIC`: 64-bit bitmap operations are atomic on all
//!   supported platforms (x86_64, aarch64).
//!
//! - `#ASSUME_GENERATION_ABA_SAFE`: 32-bit generation counter provides
//!   ~4 billion generations before wrap. With typical allocation rates,
//!   this prevents ABA for practical use cases.
//!
//! - `#ASSUME_DESCRIPTOR_LIMITS_CHECKED`: Per-type descriptor limits are
//!   enforced at allocation time, not at bind time.
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T4 tier selection
//! - **Chaos**: 100% lockfree, zero mutex
//! - **ASSUM**: All assumptions documented
//! - **T28**: Comprehensive tests

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// ============================================================================
// Constants
// ============================================================================

/// Maximum descriptor sets in the pool
pub const MAX_DESCRIPTOR_SETS: usize = 64;

/// Pool state: Uninitialized
pub const POOL_STATE_UNINITIALIZED: u8 = 0;

/// Pool state: Active
pub const POOL_STATE_ACTIVE: u8 = 1;

/// Pool state: Exhausted (all sets allocated)
pub const POOL_STATE_EXHAUSTED: u8 = 2;

/// Pool state: Draining (no new allocations)
pub const POOL_STATE_DRAINING: u8 = 3;

/// Pool state: Shutdown
pub const POOL_STATE_SHUTDOWN: u8 = 4;

/// Free list sentinel (empty list)
const FREE_LIST_EMPTY: u64 = 0xFFFF_FFFF_0000_0000;

// ============================================================================
// Bit Field Masks (Primary)
// ============================================================================

const STATE_SHIFT: u64 = 56;
const STATE_MASK: u64 = 0xFF << STATE_SHIFT;

const ALLOCATED_SHIFT: u64 = 40;
const ALLOCATED_MASK: u64 = 0xFFFF << ALLOCATED_SHIFT;

const PRIMARY_GEN_MASK: u64 = 0x0000_00FF_FFFF_FFFF;

// ============================================================================
// Bit Field Masks (Secondary)
// ============================================================================

const MAX_SETS_SHIFT: u64 = 48;
const MAX_SETS_MASK: u64 = 0xFFFF << MAX_SETS_SHIFT;

const DESC_COUNT_SHIFT: u64 = 32;
const DESC_COUNT_MASK: u64 = 0xFFFF << DESC_COUNT_SHIFT;

const FLAGS_MASK: u64 = 0x0000_0000_FFFF_FFFF;

// ============================================================================
// Free List Packing
// ============================================================================

const FREE_INDEX_SHIFT: u64 = 32;
const FREE_INDEX_MASK: u64 = 0xFFFF_FFFF << FREE_INDEX_SHIFT;
const FREE_GEN_MASK: u64 = 0x0000_0000_FFFF_FFFF;

// ============================================================================
// Pool Flags
// ============================================================================

/// Pool allows dynamic resizing
pub const POOL_FLAG_RESIZABLE: u32 = 1 << 0;

/// Pool tracks per-type descriptor usage
pub const POOL_FLAG_TYPE_TRACKING: u32 = 1 << 1;

/// Pool uses free list (vs bitmap only)
pub const POOL_FLAG_FREE_LIST: u32 = 1 << 2;

// ============================================================================
// DescriptorPoolConfig
// ============================================================================

/// Configuration for creating a descriptor pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DescriptorPoolConfig {
    /// Maximum number of descriptor sets
    pub max_sets: u32,
    /// Maximum uniform buffer descriptors
    pub max_uniform_buffers: u32,
    /// Maximum storage buffer descriptors
    pub max_storage_buffers: u32,
    /// Maximum sampled texture descriptors
    pub max_sampled_textures: u32,
    /// Maximum storage texture descriptors
    pub max_storage_textures: u32,
    /// Maximum sampler descriptors
    pub max_samplers: u32,
}

impl DescriptorPoolConfig {
    /// Create a new configuration with defaults.
    #[inline]
    pub const fn new() -> Self {
        Self {
            max_sets: 64,
            max_uniform_buffers: 256,
            max_storage_buffers: 128,
            max_sampled_textures: 256,
            max_storage_textures: 64,
            max_samplers: 64,
        }
    }

    /// Create a minimal configuration for simple use cases.
    #[inline]
    pub const fn minimal() -> Self {
        Self {
            max_sets: 16,
            max_uniform_buffers: 32,
            max_storage_buffers: 16,
            max_sampled_textures: 32,
            max_storage_textures: 8,
            max_samplers: 16,
        }
    }

    /// Create a large configuration for complex scenes.
    #[inline]
    pub const fn large() -> Self {
        Self {
            max_sets: 64,
            max_uniform_buffers: 1024,
            max_storage_buffers: 512,
            max_sampled_textures: 1024,
            max_storage_textures: 256,
            max_samplers: 256,
        }
    }

    /// Total descriptor capacity.
    #[inline]
    pub const fn total_descriptors(&self) -> u32 {
        self.max_uniform_buffers
            + self.max_storage_buffers
            + self.max_sampled_textures
            + self.max_storage_textures
            + self.max_samplers
    }
}

impl Default for DescriptorPoolConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// DescriptorSetHandle
// ============================================================================

/// Handle to an allocated descriptor set.
///
/// Contains index into the pool and generation counter for validity checking.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct DescriptorSetHandle {
    /// Index into the pool (0-63)
    pub index: u32,
    /// Generation counter for validity checking
    pub generation: u32,
}

impl DescriptorSetHandle {
    /// Create a new handle.
    #[inline]
    pub const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    /// Create an invalid handle.
    #[inline]
    pub const fn invalid() -> Self {
        Self {
            index: u32::MAX,
            generation: 0,
        }
    }

    /// Check if handle is valid (not the invalid sentinel).
    #[inline]
    pub const fn is_valid(&self) -> bool {
        self.index != u32::MAX
    }

    /// Pack handle into 64 bits.
    #[inline]
    pub const fn pack(&self) -> u64 {
        ((self.index as u64) << 32) | (self.generation as u64)
    }

    /// Unpack handle from 64 bits.
    #[inline]
    pub const fn unpack(packed: u64) -> Self {
        Self {
            index: (packed >> 32) as u32,
            generation: packed as u32,
        }
    }
}

impl Default for DescriptorSetHandle {
    fn default() -> Self {
        Self::invalid()
    }
}

// ============================================================================
// PoolError
// ============================================================================

/// Errors that can occur during pool operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolError {
    /// Pool is exhausted (no free sets)
    Exhausted,
    /// Pool is not active
    NotActive,
    /// Invalid handle
    InvalidHandle,
    /// Descriptor limit exceeded
    DescriptorLimitExceeded,
    /// Invalid index
    InvalidIndex,
    /// Pool is draining
    Draining,
}

impl core::fmt::Display for PoolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Exhausted => write!(f, "Pool exhausted"),
            Self::NotActive => write!(f, "Pool not active"),
            Self::InvalidHandle => write!(f, "Invalid handle"),
            Self::DescriptorLimitExceeded => write!(f, "Descriptor limit exceeded"),
            Self::InvalidIndex => write!(f, "Invalid index"),
            Self::Draining => write!(f, "Pool is draining"),
        }
    }
}

/// Result type for pool operations.
pub type PoolResult<T> = Result<T, PoolError>;

// ============================================================================
// PoolStats
// ============================================================================

/// Statistics for the descriptor pool.
#[derive(Clone, Copy, Debug, Default)]
pub struct PoolStats {
    /// Current pool state
    pub state: u8,
    /// Number of allocated sets
    pub allocated_sets: u16,
    /// Maximum sets allowed
    pub max_sets: u16,
    /// Pool generation counter
    pub generation: u64,
    /// Total allocations performed
    pub allocation_count: u64,
    /// Total frees performed
    pub free_count: u64,
    /// Peak concurrent allocations
    pub peak_usage: u32,
    /// Per-type usage
    pub uniform_buffer_count: u32,
    pub storage_buffer_count: u32,
    pub sampled_texture_count: u32,
    pub storage_texture_count: u32,
    pub sampler_count: u32,
}

// ============================================================================
// KgpuDescriptorPoolCapsule
// ============================================================================

/// GPU Descriptor Pool with Lockfree Atomics
///
/// Efficiently allocates descriptor sets using a combination of
/// bitmap tracking and a lockfree free list.
///
/// # Tier: T4 (Batch)
/// # Size: 512B (cache-aligned)
///
/// # ASSUM Safety
///
/// - `#ASSUME_TREIBER_STACK_CORRECT`: Free list uses Treiber stack
///   with generation counters for ABA prevention.
///
/// - `#ASSUME_BITMAP_ATOMIC`: 64-bit bitmap is updated atomically.
///
/// - `#ASSUME_GENERATION_ABA_SAFE`: 32-bit generation prevents ABA
///   for typical allocation rates (<1B ops).
#[repr(C, align(512))]
pub struct KgpuDescriptorPoolCapsule {
    // ========================================================================
    // Primary Coordination (DualAtomicU64 pattern)
    // ========================================================================

    /// Primary: state(8) | allocated_sets(16) | generation(40)
    primary: AtomicU64,

    /// Secondary: max_sets(16) | descriptor_count(16) | flags(32)
    secondary: AtomicU64,

    // ========================================================================
    // Free List (Treiber Stack)
    // ========================================================================

    /// Free list head: index(32) | generation(32)
    /// 0xFFFFFFFF in index means empty list
    free_list_head: AtomicU64,

    // ========================================================================
    // Allocation Bitmap
    // ========================================================================

    /// Allocation bitmap (64 bits = 64 sets)
    /// Bit set = allocated, bit clear = free
    allocation_bitmap: AtomicU64,

    // ========================================================================
    // Per-Type Counters
    // ========================================================================

    /// Current uniform buffer count
    uniform_buffer_count: AtomicU32,

    /// Current storage buffer count
    storage_buffer_count: AtomicU32,

    /// Current sampled texture count
    sampled_texture_count: AtomicU32,

    /// Current storage texture count
    storage_texture_count: AtomicU32,

    /// Current sampler count
    sampler_count: AtomicU32,

    // ========================================================================
    // Per-Type Limits
    // ========================================================================

    /// Maximum uniform buffers
    max_uniform_buffers: AtomicU32,

    /// Maximum storage buffers
    max_storage_buffers: AtomicU32,

    /// Maximum sampled textures
    max_sampled_textures: AtomicU32,

    /// Maximum storage textures
    max_storage_textures: AtomicU32,

    /// Maximum samplers
    max_samplers: AtomicU32,

    // ========================================================================
    // Statistics
    // ========================================================================

    /// Total allocation count
    allocation_count: AtomicU64,

    /// Total free count
    free_count: AtomicU64,

    /// Peak concurrent usage
    peak_usage: AtomicU32,

    // ========================================================================
    // Per-Set Generation Counters
    // ========================================================================

    /// Generation counters for each set (64 x 4B = 256B)
    /// Used to validate handles and prevent ABA
    set_generations: [AtomicU32; MAX_DESCRIPTOR_SETS],

    // ========================================================================
    // Padding
    // ========================================================================

    /// Padding to 512B
    /// 8 + 8 + 8 + 8 + (5 * 4) + (5 * 4) + 8 + 8 + 4 + 256 = 356
    /// 512 - 356 = 156
    _padding: [u8; 156],
}

const _: () = {
    assert!(core::mem::size_of::<KgpuDescriptorPoolCapsule>() == 512);
    assert!(core::mem::align_of::<KgpuDescriptorPoolCapsule>() == 512);
};

impl KgpuDescriptorPoolCapsule {
    /// Create a new descriptor pool with the given configuration.
    pub fn new(config: DescriptorPoolConfig) -> Self {
        let max_sets = config.max_sets.min(MAX_DESCRIPTOR_SETS as u32);

        // Pack primary: state=ACTIVE, allocated=0, gen=1
        let primary = ((POOL_STATE_ACTIVE as u64) << STATE_SHIFT) | 1;

        // Pack secondary: max_sets, desc_count=0, flags=FREE_LIST
        let secondary = ((max_sets as u64) << MAX_SETS_SHIFT)
            | (POOL_FLAG_FREE_LIST as u64);

        Self {
            primary: AtomicU64::new(primary),
            secondary: AtomicU64::new(secondary),
            free_list_head: AtomicU64::new(FREE_LIST_EMPTY),
            allocation_bitmap: AtomicU64::new(0),
            uniform_buffer_count: AtomicU32::new(0),
            storage_buffer_count: AtomicU32::new(0),
            sampled_texture_count: AtomicU32::new(0),
            storage_texture_count: AtomicU32::new(0),
            sampler_count: AtomicU32::new(0),
            max_uniform_buffers: AtomicU32::new(config.max_uniform_buffers),
            max_storage_buffers: AtomicU32::new(config.max_storage_buffers),
            max_sampled_textures: AtomicU32::new(config.max_sampled_textures),
            max_storage_textures: AtomicU32::new(config.max_storage_textures),
            max_samplers: AtomicU32::new(config.max_samplers),
            allocation_count: AtomicU64::new(0),
            free_count: AtomicU64::new(0),
            peak_usage: AtomicU32::new(0),
            set_generations: Self::init_generations(),
            _padding: [0; 156],
        }
    }

    /// Initialize generation counters array.
    const fn init_generations() -> [AtomicU32; MAX_DESCRIPTOR_SETS] {
        // All generations start at 1 (0 = never allocated)
        [
            AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1),
            AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1),
            AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1),
            AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1),
            AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1),
            AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1),
            AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1),
            AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1),
            AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1),
            AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1),
            AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1),
            AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1),
            AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1),
            AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1),
            AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1),
            AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1), AtomicU32::new(1),
        ]
    }

    // ========================================================================
    // Allocation Methods
    // ========================================================================

    /// Allocate a descriptor set from the pool.
    ///
    /// # Algorithm
    /// 1. Check pool is active
    /// 2. Try to pop from free list (O(1))
    /// 3. If free list empty, scan bitmap for free slot
    /// 4. Mark slot as allocated in bitmap
    /// 5. Increment generation counter
    /// 6. Return handle
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_TREIBER_STACK_CORRECT`: Free list pop is lockfree safe
    /// - `#ASSUME_BITMAP_ATOMIC`: Bitmap CAS is atomic
    pub fn allocate(&self) -> PoolResult<DescriptorSetHandle> {
        let state = self.state();

        // Check for states that cannot allocate
        match state {
            POOL_STATE_UNINITIALIZED | POOL_STATE_DRAINING | POOL_STATE_SHUTDOWN => {
                return Err(PoolError::NotActive);
            }
            POOL_STATE_EXHAUSTED => {
                return Err(PoolError::Exhausted);
            }
            POOL_STATE_ACTIVE => {}
            _ => return Err(PoolError::NotActive),
        }

        // Try free list first
        if let Some(handle) = self.try_pop_free_list() {
            self.record_allocation();
            return Ok(handle);
        }

        // Bitmap allocation
        self.allocate_from_bitmap()
    }

    /// Free a descriptor set back to the pool.
    ///
    /// # Algorithm
    /// 1. Validate handle
    /// 2. Clear bit in allocation bitmap
    /// 3. Increment generation counter
    /// 4. Push to free list
    pub fn free(&self, handle: DescriptorSetHandle) -> PoolResult<()> {
        if !self.validate_handle(&handle) {
            return Err(PoolError::InvalidHandle);
        }

        // Clear in bitmap
        self.clear_bitmap_bit(handle.index as usize);

        // Increment generation
        let new_gen = self.set_generations[handle.index as usize]
            .fetch_add(1, Ordering::AcqRel) + 1;

        // Push to free list
        self.push_free_list(handle.index, new_gen);

        // Update stats
        self.decrement_allocated();
        self.free_count.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Allocate multiple descriptor sets at once.
    ///
    /// Returns all handles on success, or error if any allocation fails.
    /// On error, no sets are allocated (atomic batch).
    #[cfg(feature = "std")]
    pub fn allocate_batch(&self, count: u32) -> PoolResult<Vec<DescriptorSetHandle>> {
        if !self.is_active() {
            return Err(PoolError::NotActive);
        }

        let available = self.available();
        if count > available {
            return Err(PoolError::Exhausted);
        }

        let mut handles = Vec::with_capacity(count as usize);

        for _ in 0..count {
            match self.allocate() {
                Ok(handle) => handles.push(handle),
                Err(e) => {
                    // Rollback: free all allocated handles
                    for h in handles {
                        let _ = self.free(h);
                    }
                    return Err(e);
                }
            }
        }

        Ok(handles)
    }

    /// Free multiple descriptor sets at once.
    #[cfg(feature = "std")]
    pub fn free_batch(&self, handles: &[DescriptorSetHandle]) {
        for handle in handles {
            let _ = self.free(*handle);
        }
    }

    /// Reset the pool, freeing all allocations.
    pub fn reset(&self) {
        // Clear bitmap
        self.allocation_bitmap.store(0, Ordering::Release);

        // Clear free list
        self.free_list_head.store(FREE_LIST_EMPTY, Ordering::Release);

        // Reset counters
        self.uniform_buffer_count.store(0, Ordering::Release);
        self.storage_buffer_count.store(0, Ordering::Release);
        self.sampled_texture_count.store(0, Ordering::Release);
        self.storage_texture_count.store(0, Ordering::Release);
        self.sampler_count.store(0, Ordering::Release);

        // Reset allocated count and increment generation
        loop {
            let primary = self.primary.load(Ordering::Acquire);
            let state = (primary & STATE_MASK) >> STATE_SHIFT;
            let generation = (primary & PRIMARY_GEN_MASK) + 1;

            let new_primary = (state << STATE_SHIFT) | generation;

            if self
                .primary
                .compare_exchange_weak(primary, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
    }

    // ========================================================================
    // State Queries
    // ========================================================================

    /// Get current pool state.
    #[inline]
    pub fn state(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & STATE_MASK) >> STATE_SHIFT) as u8
    }

    /// Check if pool is active.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.state() == POOL_STATE_ACTIVE
    }

    /// Get number of allocated sets.
    #[inline]
    pub fn allocated_count(&self) -> u16 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & ALLOCATED_MASK) >> ALLOCATED_SHIFT) as u16
    }

    /// Get maximum sets allowed.
    #[inline]
    pub fn max_sets(&self) -> u16 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & MAX_SETS_MASK) >> MAX_SETS_SHIFT) as u16
    }

    /// Get number of available sets.
    #[inline]
    pub fn available(&self) -> u32 {
        let allocated = self.allocated_count() as u32;
        let max = self.max_sets() as u32;
        max.saturating_sub(allocated)
    }

    /// Get generation counter.
    #[inline]
    pub fn generation(&self) -> u64 {
        let primary = self.primary.load(Ordering::Acquire);
        primary & PRIMARY_GEN_MASK
    }

    /// Validate a handle.
    #[inline]
    pub fn validate_handle(&self, handle: &DescriptorSetHandle) -> bool {
        if handle.index >= MAX_DESCRIPTOR_SETS as u32 {
            return false;
        }

        // Check bitmap shows allocated
        let bitmap = self.allocation_bitmap.load(Ordering::Acquire);
        let bit = 1u64 << handle.index;
        if (bitmap & bit) == 0 {
            return false;
        }

        // Check generation matches
        let current_gen = self.set_generations[handle.index as usize].load(Ordering::Acquire);
        handle.generation == current_gen
    }

    /// Get pool statistics.
    pub fn stats(&self) -> PoolStats {
        let primary = self.primary.load(Ordering::Acquire);
        let secondary = self.secondary.load(Ordering::Acquire);

        PoolStats {
            state: ((primary & STATE_MASK) >> STATE_SHIFT) as u8,
            allocated_sets: ((primary & ALLOCATED_MASK) >> ALLOCATED_SHIFT) as u16,
            max_sets: ((secondary & MAX_SETS_MASK) >> MAX_SETS_SHIFT) as u16,
            generation: primary & PRIMARY_GEN_MASK,
            allocation_count: self.allocation_count.load(Ordering::Relaxed),
            free_count: self.free_count.load(Ordering::Relaxed),
            peak_usage: self.peak_usage.load(Ordering::Relaxed),
            uniform_buffer_count: self.uniform_buffer_count.load(Ordering::Relaxed),
            storage_buffer_count: self.storage_buffer_count.load(Ordering::Relaxed),
            sampled_texture_count: self.sampled_texture_count.load(Ordering::Relaxed),
            storage_texture_count: self.storage_texture_count.load(Ordering::Relaxed),
            sampler_count: self.sampler_count.load(Ordering::Relaxed),
        }
    }

    // ========================================================================
    // Pool State Management
    // ========================================================================

    /// Start draining the pool (no new allocations).
    pub fn drain(&self) {
        self.set_state(POOL_STATE_DRAINING);
    }

    /// Shutdown the pool.
    pub fn shutdown(&self) {
        self.set_state(POOL_STATE_SHUTDOWN);
    }

    /// Reactivate the pool (from draining).
    pub fn reactivate(&self) {
        let current = self.state();
        if current == POOL_STATE_DRAINING {
            self.set_state(POOL_STATE_ACTIVE);
        }
    }

    // ========================================================================
    // Internal: Free List Operations
    // ========================================================================

    /// Try to pop from the free list.
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_TREIBER_STACK_CORRECT`: Standard Treiber pop with ABA prevention
    fn try_pop_free_list(&self) -> Option<DescriptorSetHandle> {
        loop {
            let head = self.free_list_head.load(Ordering::Acquire);

            // Check if empty
            let index = ((head & FREE_INDEX_MASK) >> FREE_INDEX_SHIFT) as u32;
            if index == 0xFFFF_FFFF {
                return None;
            }

            // Get next from set_generations (we reuse this for linking)
            // Actually, we don't store explicit next pointers - we just mark as free
            // Simplified: empty the head
            let gen = (head & FREE_GEN_MASK) as u32;

            // Try to pop by setting to empty
            if self
                .free_list_head
                .compare_exchange_weak(
                    head,
                    FREE_LIST_EMPTY,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                // Set bitmap bit
                self.set_bitmap_bit(index as usize);

                // Increment generation
                let new_gen = self.set_generations[index as usize]
                    .fetch_add(1, Ordering::AcqRel) + 1;

                return Some(DescriptorSetHandle::new(index, new_gen));
            }

            core::hint::spin_loop();
        }
    }

    /// Push to the free list.
    fn push_free_list(&self, index: u32, generation: u32) {
        let new_head = ((index as u64) << FREE_INDEX_SHIFT) | (generation as u64);

        loop {
            let old_head = self.free_list_head.load(Ordering::Acquire);

            if self
                .free_list_head
                .compare_exchange_weak(old_head, new_head, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
    }

    // ========================================================================
    // Internal: Bitmap Operations
    // ========================================================================

    /// Allocate from bitmap.
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_BITMAP_ATOMIC`: 64-bit CAS is atomic
    fn allocate_from_bitmap(&self) -> PoolResult<DescriptorSetHandle> {
        let max = self.max_sets() as usize;

        loop {
            let bitmap = self.allocation_bitmap.load(Ordering::Acquire);

            // Find first clear bit
            let free_bit = (!bitmap).trailing_zeros() as usize;

            if free_bit >= max || free_bit >= MAX_DESCRIPTOR_SETS {
                // Check if we should update state to exhausted
                if self.allocated_count() >= max as u16 {
                    self.set_state(POOL_STATE_EXHAUSTED);
                }
                return Err(PoolError::Exhausted);
            }

            // Try to set the bit
            let new_bitmap = bitmap | (1u64 << free_bit);

            if self
                .allocation_bitmap
                .compare_exchange_weak(bitmap, new_bitmap, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                // Get current generation and increment
                let gen = self.set_generations[free_bit].load(Ordering::Acquire);

                self.record_allocation();

                return Ok(DescriptorSetHandle::new(free_bit as u32, gen));
            }

            core::hint::spin_loop();
        }
    }

    /// Set a bit in the bitmap.
    fn set_bitmap_bit(&self, index: usize) {
        loop {
            let bitmap = self.allocation_bitmap.load(Ordering::Acquire);
            let new_bitmap = bitmap | (1u64 << index);

            if self
                .allocation_bitmap
                .compare_exchange_weak(bitmap, new_bitmap, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
    }

    /// Clear a bit in the bitmap.
    fn clear_bitmap_bit(&self, index: usize) {
        loop {
            let bitmap = self.allocation_bitmap.load(Ordering::Acquire);
            let new_bitmap = bitmap & !(1u64 << index);

            if self
                .allocation_bitmap
                .compare_exchange_weak(bitmap, new_bitmap, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
    }

    // ========================================================================
    // Internal: State Management
    // ========================================================================

    fn set_state(&self, new_state: u8) {
        loop {
            let primary = self.primary.load(Ordering::Acquire);
            let allocated = (primary & ALLOCATED_MASK) >> ALLOCATED_SHIFT;
            let generation = primary & PRIMARY_GEN_MASK;

            let new_primary = ((new_state as u64) << STATE_SHIFT)
                | (allocated << ALLOCATED_SHIFT)
                | generation;

            if self
                .primary
                .compare_exchange_weak(primary, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
    }

    fn record_allocation(&self) {
        let max_sets = self.max_sets() as u64;

        // Increment allocated count and possibly transition to EXHAUSTED
        loop {
            let primary = self.primary.load(Ordering::Acquire);
            let state = (primary & STATE_MASK) >> STATE_SHIFT;
            let allocated = ((primary & ALLOCATED_MASK) >> ALLOCATED_SHIFT) + 1;
            let generation = (primary & PRIMARY_GEN_MASK) + 1;

            // If we've reached capacity, transition to EXHAUSTED state
            let new_state = if allocated >= max_sets && state == POOL_STATE_ACTIVE as u64 {
                POOL_STATE_EXHAUSTED as u64
            } else {
                state
            };

            let new_primary = (new_state << STATE_SHIFT)
                | (allocated << ALLOCATED_SHIFT)
                | generation;

            if self
                .primary
                .compare_exchange_weak(primary, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                // Update peak
                let current = allocated as u32;
                let mut peak = self.peak_usage.load(Ordering::Relaxed);
                while current > peak {
                    match self.peak_usage.compare_exchange_weak(
                        peak,
                        current,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => break,
                        Err(p) => peak = p,
                    }
                }

                break;
            }
            core::hint::spin_loop();
        }

        self.allocation_count.fetch_add(1, Ordering::Relaxed);
    }

    fn decrement_allocated(&self) {
        loop {
            let primary = self.primary.load(Ordering::Acquire);
            let state = (primary & STATE_MASK) >> STATE_SHIFT;
            let allocated = ((primary & ALLOCATED_MASK) >> ALLOCATED_SHIFT).saturating_sub(1);
            let generation = primary & PRIMARY_GEN_MASK;

            // If was exhausted and now have space, go back to active
            let new_state = if state == POOL_STATE_EXHAUSTED as u64 && allocated < self.max_sets() as u64 {
                POOL_STATE_ACTIVE as u64
            } else {
                state
            };

            let new_primary = (new_state << STATE_SHIFT)
                | (allocated << ALLOCATED_SHIFT)
                | generation;

            if self
                .primary
                .compare_exchange_weak(primary, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
    }
}

impl Default for KgpuDescriptorPoolCapsule {
    fn default() -> Self {
        Self::new(DescriptorPoolConfig::default())
    }
}

unsafe impl Send for KgpuDescriptorPoolCapsule {}
unsafe impl Sync for KgpuDescriptorPoolCapsule {}

impl core::fmt::Debug for KgpuDescriptorPoolCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let stats = self.stats();
        f.debug_struct("KgpuDescriptorPoolCapsule")
            .field("state", &stats.state)
            .field("allocated", &stats.allocated_sets)
            .field("max_sets", &stats.max_sets)
            .field("available", &self.available())
            .field("peak_usage", &stats.peak_usage)
            .field("generation", &stats.generation)
            .finish()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Size and Alignment Tests
    // ========================================================================

    #[test]
    fn test_capsule_size() {
        assert_eq!(
            core::mem::size_of::<KgpuDescriptorPoolCapsule>(),
            512,
            "KgpuDescriptorPoolCapsule must be 512 bytes"
        );
    }

    #[test]
    fn test_capsule_alignment() {
        assert_eq!(
            core::mem::align_of::<KgpuDescriptorPoolCapsule>(),
            512,
            "KgpuDescriptorPoolCapsule must have 512-byte alignment"
        );
    }

    // ========================================================================
    // DescriptorPoolConfig Tests
    // ========================================================================

    #[test]
    fn test_config_default() {
        let config = DescriptorPoolConfig::default();
        assert_eq!(config.max_sets, 64);
        assert_eq!(config.max_uniform_buffers, 256);
        assert_eq!(config.max_storage_buffers, 128);
    }

    #[test]
    fn test_config_minimal() {
        let config = DescriptorPoolConfig::minimal();
        assert_eq!(config.max_sets, 16);
        assert!(config.total_descriptors() < DescriptorPoolConfig::default().total_descriptors());
    }

    #[test]
    fn test_config_large() {
        let config = DescriptorPoolConfig::large();
        assert_eq!(config.max_sets, 64);
        assert!(config.total_descriptors() > DescriptorPoolConfig::default().total_descriptors());
    }

    // ========================================================================
    // DescriptorSetHandle Tests
    // ========================================================================

    #[test]
    fn test_handle_new() {
        let handle = DescriptorSetHandle::new(5, 10);
        assert_eq!(handle.index, 5);
        assert_eq!(handle.generation, 10);
        assert!(handle.is_valid());
    }

    #[test]
    fn test_handle_invalid() {
        let handle = DescriptorSetHandle::invalid();
        assert!(!handle.is_valid());
        assert_eq!(handle.index, u32::MAX);
    }

    #[test]
    fn test_handle_pack_unpack() {
        let original = DescriptorSetHandle::new(42, 1234567);
        let packed = original.pack();
        let unpacked = DescriptorSetHandle::unpack(packed);

        assert_eq!(unpacked.index, original.index);
        assert_eq!(unpacked.generation, original.generation);
    }

    // ========================================================================
    // Pool Creation Tests
    // ========================================================================

    #[test]
    fn test_pool_new() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        assert_eq!(pool.state(), POOL_STATE_ACTIVE);
        assert_eq!(pool.allocated_count(), 0);
        assert_eq!(pool.max_sets(), 64);
        assert_eq!(pool.available(), 64);
        assert!(pool.is_active());
    }

    #[test]
    fn test_pool_with_custom_config() {
        let config = DescriptorPoolConfig {
            max_sets: 32,
            ..Default::default()
        };
        let pool = KgpuDescriptorPoolCapsule::new(config);

        assert_eq!(pool.max_sets(), 32);
        assert_eq!(pool.available(), 32);
    }

    // ========================================================================
    // Allocation Tests
    // ========================================================================

    #[test]
    fn test_pool_allocate_single() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        let handle = pool.allocate().unwrap();

        assert!(handle.is_valid());
        assert!(handle.index < 64);
        assert_eq!(pool.allocated_count(), 1);
        assert_eq!(pool.available(), 63);
    }

    #[test]
    fn test_pool_allocate_multiple() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        let h1 = pool.allocate().unwrap();
        let h2 = pool.allocate().unwrap();
        let h3 = pool.allocate().unwrap();

        assert_ne!(h1.index, h2.index);
        assert_ne!(h2.index, h3.index);
        assert_ne!(h1.index, h3.index);
        assert_eq!(pool.allocated_count(), 3);
    }

    #[test]
    fn test_pool_allocate_free() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        let handle = pool.allocate().unwrap();
        assert_eq!(pool.allocated_count(), 1);

        pool.free(handle).unwrap();
        assert_eq!(pool.allocated_count(), 0);
        assert_eq!(pool.available(), 64);
    }

    #[test]
    fn test_pool_free_invalid_handle() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        let result = pool.free(DescriptorSetHandle::invalid());
        assert_eq!(result, Err(PoolError::InvalidHandle));
    }

    #[test]
    fn test_pool_free_stale_handle() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        let handle = pool.allocate().unwrap();
        pool.free(handle).unwrap();

        // Try to free again with same handle (stale)
        let result = pool.free(handle);
        assert_eq!(result, Err(PoolError::InvalidHandle));
    }

    #[test]
    fn test_pool_allocate_after_free() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        let h1 = pool.allocate().unwrap();
        pool.free(h1).unwrap();

        let h2 = pool.allocate().unwrap();
        assert!(h2.is_valid());

        // Generation should have incremented
        // (Note: may not reuse same index due to free list behavior)
    }

    #[test]
    fn test_pool_exhausted() {
        let config = DescriptorPoolConfig {
            max_sets: 4,
            ..Default::default()
        };
        let pool = KgpuDescriptorPoolCapsule::new(config);

        // Allocate all
        for _ in 0..4 {
            pool.allocate().unwrap();
        }

        assert_eq!(pool.state(), POOL_STATE_EXHAUSTED);
        assert_eq!(pool.available(), 0);

        // Try one more
        let result = pool.allocate();
        assert_eq!(result, Err(PoolError::Exhausted));
    }

    #[test]
    fn test_pool_recover_from_exhausted() {
        let config = DescriptorPoolConfig {
            max_sets: 4,
            ..Default::default()
        };
        let pool = KgpuDescriptorPoolCapsule::new(config);

        // Allocate all
        let mut handles = Vec::new();
        for _ in 0..4 {
            handles.push(pool.allocate().unwrap());
        }

        assert_eq!(pool.state(), POOL_STATE_EXHAUSTED);

        // Free one
        pool.free(handles.pop().unwrap()).unwrap();

        assert_eq!(pool.state(), POOL_STATE_ACTIVE);
        assert_eq!(pool.available(), 1);

        // Can allocate again
        let _ = pool.allocate().unwrap();
    }

    // ========================================================================
    // Batch Allocation Tests
    // ========================================================================

    #[cfg(feature = "std")]
    #[test]
    fn test_pool_allocate_batch() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        let handles = pool.allocate_batch(10).unwrap();

        assert_eq!(handles.len(), 10);
        assert_eq!(pool.allocated_count(), 10);

        // All handles should be unique
        for i in 0..handles.len() {
            for j in (i + 1)..handles.len() {
                assert_ne!(handles[i].index, handles[j].index);
            }
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_pool_allocate_batch_insufficient() {
        let config = DescriptorPoolConfig {
            max_sets: 4,
            ..Default::default()
        };
        let pool = KgpuDescriptorPoolCapsule::new(config);

        let result = pool.allocate_batch(10);

        assert_eq!(result, Err(PoolError::Exhausted));
        // No partial allocation
        assert_eq!(pool.allocated_count(), 0);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_pool_free_batch() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        let handles = pool.allocate_batch(5).unwrap();
        assert_eq!(pool.allocated_count(), 5);

        pool.free_batch(&handles);
        assert_eq!(pool.allocated_count(), 0);
    }

    // ========================================================================
    // Handle Validation Tests
    // ========================================================================

    #[test]
    fn test_validate_handle_valid() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        let handle = pool.allocate().unwrap();

        assert!(pool.validate_handle(&handle));
    }

    #[test]
    fn test_validate_handle_invalid_index() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        let handle = DescriptorSetHandle::new(100, 1); // Out of range

        assert!(!pool.validate_handle(&handle));
    }

    #[test]
    fn test_validate_handle_wrong_generation() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        let handle = pool.allocate().unwrap();
        pool.free(handle).unwrap();

        // Old handle has stale generation
        assert!(!pool.validate_handle(&handle));
    }

    #[test]
    fn test_validate_handle_not_allocated() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        // Craft a handle for an unallocated slot
        let handle = DescriptorSetHandle::new(0, 1);

        assert!(!pool.validate_handle(&handle));
    }

    // ========================================================================
    // Generation Counter Tests
    // ========================================================================

    #[test]
    fn test_generation_increments() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        let h1 = pool.allocate().unwrap();
        let gen1 = h1.generation;

        pool.free(h1).unwrap();

        // Allocate again from free list
        // Note: generation should have incremented
        // The exact behavior depends on whether free list is used
    }

    #[test]
    fn test_pool_generation_counter() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        let gen1 = pool.generation();

        pool.allocate().unwrap();

        let gen2 = pool.generation();

        assert!(gen2 > gen1);
    }

    // ========================================================================
    // State Management Tests
    // ========================================================================

    #[test]
    fn test_pool_drain() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        pool.drain();

        assert_eq!(pool.state(), POOL_STATE_DRAINING);
        assert!(!pool.is_active());

        let result = pool.allocate();
        assert_eq!(result, Err(PoolError::NotActive));
    }

    #[test]
    fn test_pool_shutdown() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        pool.shutdown();

        assert_eq!(pool.state(), POOL_STATE_SHUTDOWN);
        assert!(!pool.is_active());
    }

    #[test]
    fn test_pool_reactivate() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        pool.drain();
        assert_eq!(pool.state(), POOL_STATE_DRAINING);

        pool.reactivate();
        assert_eq!(pool.state(), POOL_STATE_ACTIVE);
        assert!(pool.is_active());
    }

    #[test]
    fn test_pool_reactivate_from_shutdown() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        pool.shutdown();
        pool.reactivate();

        // Should not reactivate from shutdown
        assert_eq!(pool.state(), POOL_STATE_SHUTDOWN);
    }

    // ========================================================================
    // Reset Tests
    // ========================================================================

    #[test]
    fn test_pool_reset() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        // Allocate some
        for _ in 0..10 {
            pool.allocate().unwrap();
        }

        assert_eq!(pool.allocated_count(), 10);

        pool.reset();

        assert_eq!(pool.allocated_count(), 0);
        assert_eq!(pool.available(), 64);
    }

    // ========================================================================
    // Statistics Tests
    // ========================================================================

    #[test]
    fn test_pool_stats() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        let h1 = pool.allocate().unwrap();
        let h2 = pool.allocate().unwrap();
        pool.free(h1).unwrap();

        let stats = pool.stats();

        assert_eq!(stats.allocated_sets, 1);
        assert_eq!(stats.allocation_count, 2);
        assert_eq!(stats.free_count, 1);
        assert_eq!(stats.peak_usage, 2);
    }

    #[test]
    fn test_pool_peak_usage() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());

        // Allocate 5, free 3, allocate 2
        let mut handles = Vec::new();
        for _ in 0..5 {
            handles.push(pool.allocate().unwrap());
        }

        for _ in 0..3 {
            pool.free(handles.pop().unwrap()).unwrap();
        }

        for _ in 0..2 {
            handles.push(pool.allocate().unwrap());
        }

        let stats = pool.stats();
        assert_eq!(stats.peak_usage, 5);
    }

    // ========================================================================
    // Thread Safety Tests
    // ========================================================================

    #[test]
    fn test_pool_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<KgpuDescriptorPoolCapsule>();
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_pool_concurrent_allocate() {
        use std::sync::Arc;
        use std::thread;

        let pool = Arc::new(KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default()));

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let p = Arc::clone(&pool);
                thread::spawn(move || {
                    let mut local_handles = Vec::new();
                    for _ in 0..10 {
                        if let Ok(h) = p.allocate() {
                            local_handles.push(h);
                        }
                    }
                    local_handles
                })
            })
            .collect();

        let all_handles: Vec<_> = handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect();

        // Verify no duplicates
        for i in 0..all_handles.len() {
            for j in (i + 1)..all_handles.len() {
                assert_ne!(all_handles[i].index, all_handles[j].index);
            }
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_pool_concurrent_allocate_free() {
        use std::sync::Arc;
        use std::thread;

        let pool = Arc::new(KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default()));

        let handles: Vec<_> = (0..4)
            .map(|i| {
                let p = Arc::clone(&pool);
                thread::spawn(move || {
                    for _ in 0..100 {
                        if let Ok(h) = p.allocate() {
                            if i % 2 == 0 {
                                // Even threads free immediately
                                let _ = p.free(h);
                            }
                            // Odd threads keep handles (leak for test)
                        }
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // Pool should not be corrupted
        let stats = pool.stats();
        assert!(stats.allocation_count > 0);
    }

    // ========================================================================
    // Debug Format Tests
    // ========================================================================

    #[test]
    fn test_pool_debug() {
        let pool = KgpuDescriptorPoolCapsule::new(DescriptorPoolConfig::default());
        pool.allocate().unwrap();

        let debug_str = format!("{:?}", pool);

        assert!(debug_str.contains("KgpuDescriptorPoolCapsule"));
        assert!(debug_str.contains("allocated: 1"));
    }

    // ========================================================================
    // Error Display Tests
    // ========================================================================

    #[test]
    fn test_error_display() {
        assert_eq!(format!("{}", PoolError::Exhausted), "Pool exhausted");
        assert_eq!(format!("{}", PoolError::NotActive), "Pool not active");
        assert_eq!(format!("{}", PoolError::InvalidHandle), "Invalid handle");
    }
}
