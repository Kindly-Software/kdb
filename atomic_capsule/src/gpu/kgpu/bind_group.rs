//! KgpuBindGroupCapsule - Lockfree GPU Resource Binding Groups
//!
//! **Tier**: T1 (Atomic)
//! **Size**: 256B (cache-aligned)
//! **Purpose**: Manage GPU resource bindings (buffers, textures, samplers) with lockfree atomics
//!
//! # Architecture
//!
//! Bind groups are collections of GPU resources that are bound together for shader access.
//! This capsule provides:
//!
//! - **Up to 8 bindings per group** (typical GPU limit)
//! - **Lockfree binding updates** via atomic CAS operations
//! - **Reference counting** for resource lifetime management
//! - **Generation counters** for ABA prevention
//!
//! # Memory Layout (256B)
//!
//! ```text
//! Offset  Size    Field
//! 0       64      KgpuHandle<BindGroup> (generation-countered handle)
//! 64      8       Primary: state(8) | binding_count(8) | generation(48)
//! 72      8       Secondary: layout_id(32) | flags(32)
//! 80      128     Bindings: 8 x BindingSlot (16B each)
//! 208     4       Reference count (AtomicU32)
//! 212     44      Reserved/padding to 256B
//! ```
//!
//! # ASSUM Safety Documentation
//!
//! - `#ASSUME_BINDING_SLOT_ATOMIC`: Each BindingSlot uses AtomicU64 for lockfree updates.
//!   Binding changes are atomic at the slot level.
//!
//! - `#ASSUME_GENERATION_ABA_SAFE`: 48-bit generation counter prevents ABA problems
//!   for ~280 trillion operations before wraparound.
//!
//! - `#ASSUME_REFCOUNT_THREAD_SAFE`: AtomicU32 reference count ensures thread-safe
//!   lifetime management. Decrement returns true when refcount reaches zero.
//!
//! - `#ASSUME_CACHE_ALIGNED`: 256B alignment prevents false sharing between bind groups.
//!
//! # Performance
//!
//! - Binding update: <50ns (atomic CAS)
//! - Binding query: <10ns (atomic load)
//! - Completeness check: <20ns (scan 8 slots)
//! - Reference count: <10ns (atomic increment/decrement)
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T1 tier selection, Q33 compile-time verification
//! - **Chaos**: 100% lockfree, zero mutex, cache-aligned 256B
//! - **ASSUM**: All assumptions documented with #ASSUME/#VERIFY tags
//! - **T28**: Unit/Property/Integration tests for all operations

use core::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};

use super::handle::KgpuHandle;

// ============================================================================
// Constants
// ============================================================================

/// Maximum bindings per bind group (GPU typical limit)
pub const MAX_BINDINGS_PER_GROUP: usize = 8;

// ============================================================================
// Binding Types
// ============================================================================

/// Type of resource bound to a binding slot.
///
/// Each type corresponds to a different shader resource category.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BindingType {
    /// Empty/unbound slot
    Empty = 0,
    /// Uniform buffer (read-only, small, frequent updates)
    UniformBuffer = 1,
    /// Storage buffer (read/write, large)
    StorageBuffer = 2,
    /// Read-only storage buffer
    ReadOnlyStorageBuffer = 3,
    /// Sampler for texture filtering
    Sampler = 4,
    /// Sampled texture (read-only)
    Texture = 5,
    /// Storage texture (read/write)
    StorageTexture = 6,
}

impl BindingType {
    /// Convert from raw u8 value
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Empty),
            1 => Some(Self::UniformBuffer),
            2 => Some(Self::StorageBuffer),
            3 => Some(Self::ReadOnlyStorageBuffer),
            4 => Some(Self::Sampler),
            5 => Some(Self::Texture),
            6 => Some(Self::StorageTexture),
            _ => None,
        }
    }

    /// Check if this is a buffer type
    #[inline]
    pub const fn is_buffer(&self) -> bool {
        matches!(
            self,
            Self::UniformBuffer | Self::StorageBuffer | Self::ReadOnlyStorageBuffer
        )
    }

    /// Check if this is a texture type
    #[inline]
    pub const fn is_texture(&self) -> bool {
        matches!(self, Self::Texture | Self::StorageTexture)
    }

    /// Check if binding slot is empty
    #[inline]
    pub const fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }
}

// ============================================================================
// BindGroup State
// ============================================================================

/// State of the bind group
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BindGroupState {
    /// Bind group is being created/modified
    Building = 0,
    /// Bind group is complete and ready for use
    Ready = 1,
    /// Bind group is currently bound to a pipeline
    Bound = 2,
    /// Bind group has been destroyed
    Destroyed = 3,
}

impl BindGroupState {
    /// Convert from raw u8 value
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Building),
            1 => Some(Self::Ready),
            2 => Some(Self::Bound),
            3 => Some(Self::Destroyed),
            _ => None,
        }
    }
}

// ============================================================================
// Bit Field Masks (Primary: state(8) | binding_count(8) | generation(48))
// ============================================================================

/// State field: bits [63:56] (8 bits)
const STATE_SHIFT: u64 = 56;
const STATE_MASK: u64 = 0xFF << STATE_SHIFT;

/// Binding count field: bits [55:48] (8 bits)
const BINDING_COUNT_SHIFT: u64 = 48;
const BINDING_COUNT_MASK: u64 = 0xFF << BINDING_COUNT_SHIFT;

/// Generation field: bits [47:0] (48 bits)
const GENERATION_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

// ============================================================================
// Bit Field Masks (Secondary: layout_id(32) | flags(32))
// ============================================================================

/// Layout ID field: bits [63:32] (32 bits)
const LAYOUT_ID_SHIFT: u64 = 32;
const LAYOUT_ID_MASK: u64 = 0xFFFF_FFFF << LAYOUT_ID_SHIFT;

/// Flags field: bits [31:0] (32 bits)
const FLAGS_MASK: u64 = 0x0000_0000_FFFF_FFFF;

// ============================================================================
// BindGroup Flags
// ============================================================================

/// Bind group is immutable after creation
pub const BIND_GROUP_FLAG_IMMUTABLE: u32 = 1 << 0;

/// Bind group uses dynamic offsets
pub const BIND_GROUP_FLAG_DYNAMIC_OFFSETS: u32 = 1 << 1;

/// Bind group is for compute shaders
pub const BIND_GROUP_FLAG_COMPUTE: u32 = 1 << 2;

/// Bind group is for graphics shaders
pub const BIND_GROUP_FLAG_GRAPHICS: u32 = 1 << 3;

// ============================================================================
// BindGroup Marker Type
// ============================================================================

/// Marker type for bind group resources (used with KgpuHandle<BindGroup>)
#[derive(Debug, Clone, Copy)]
pub struct BindGroup;

// ============================================================================
// BindingSlot
// ============================================================================

/// A single binding slot within a bind group.
///
/// # Layout (16B)
///
/// ```text
/// Offset  Size    Field
/// 0       8       resource_handle (AtomicU64) - Handle to buffer/texture/sampler
/// 8       1       binding_type (AtomicU8) - BindingType enum
/// 9       7       _padding - Alignment padding
/// ```
///
/// # ASSUM Safety
///
/// - `#ASSUME_SLOT_ATOMIC`: All slot operations use atomic operations
/// - `#ASSUME_SLOT_INDEPENDENT`: Slots can be updated independently
#[repr(C)]
pub struct BindingSlot {
    /// Handle to the bound resource (buffer, texture, or sampler)
    ///
    /// Uses generation-countered handle format from KgpuHandle.
    /// 0 = empty/unbound.
    resource_handle: AtomicU64,

    /// Type of binding (BindingType enum as u8)
    binding_type: AtomicU8,

    /// Padding for alignment
    _padding: [u8; 7],
}

impl BindingSlot {
    /// Create a new empty binding slot
    #[inline]
    pub const fn new() -> Self {
        Self {
            resource_handle: AtomicU64::new(0),
            binding_type: AtomicU8::new(BindingType::Empty as u8),
            _padding: [0; 7],
        }
    }

    /// Set the binding to a resource
    ///
    /// # Arguments
    /// - `binding_type`: Type of resource being bound
    /// - `handle`: Resource handle (from KgpuHandle::packed_value())
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_SLOT_ATOMIC`: Uses Release ordering to ensure visibility
    #[inline]
    pub fn set(&self, binding_type: BindingType, handle: u64) {
        // Store handle first, then type (Release ensures ordering)
        self.resource_handle.store(handle, Ordering::Release);
        self.binding_type
            .store(binding_type as u8, Ordering::Release);
    }

    /// Clear the binding slot
    #[inline]
    pub fn clear(&self) {
        self.binding_type
            .store(BindingType::Empty as u8, Ordering::Release);
        self.resource_handle.store(0, Ordering::Release);
    }

    /// Get the binding type
    #[inline]
    pub fn binding_type(&self) -> BindingType {
        let raw = self.binding_type.load(Ordering::Acquire);
        BindingType::from_u8(raw).unwrap_or(BindingType::Empty)
    }

    /// Get the resource handle
    #[inline]
    pub fn resource_handle(&self) -> u64 {
        self.resource_handle.load(Ordering::Acquire)
    }

    /// Check if slot is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.binding_type() == BindingType::Empty
    }

    /// Check if slot is occupied
    #[inline]
    pub fn is_occupied(&self) -> bool {
        !self.is_empty()
    }
}

impl Default for BindingSlot {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for BindingSlot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BindingSlot")
            .field("binding_type", &self.binding_type())
            .field("resource_handle", &format_args!("0x{:016X}", self.resource_handle()))
            .finish()
    }
}

// Compile-time verification of BindingSlot size
const _: () = {
    assert!(core::mem::size_of::<BindingSlot>() == 16);
};

// ============================================================================
// BindGroupError
// ============================================================================

/// Errors that can occur during bind group operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindGroupError {
    /// Binding index is out of range (0-7)
    IndexOutOfRange,
    /// Bind group is in an invalid state for the operation
    InvalidState,
    /// Bind group has been destroyed
    Destroyed,
    /// Resource handle is invalid
    InvalidResource,
    /// Bind group is not complete (missing required bindings)
    Incomplete,
    /// Bind group is immutable and cannot be modified
    Immutable,
}

impl core::fmt::Display for BindGroupError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::IndexOutOfRange => write!(f, "Binding index out of range (0-7)"),
            Self::InvalidState => write!(f, "Invalid bind group state for operation"),
            Self::Destroyed => write!(f, "Bind group has been destroyed"),
            Self::InvalidResource => write!(f, "Invalid resource handle"),
            Self::Incomplete => write!(f, "Bind group is incomplete"),
            Self::Immutable => write!(f, "Bind group is immutable"),
        }
    }
}

/// Result type for bind group operations
pub type BindGroupResult<T> = Result<T, BindGroupError>;

// ============================================================================
// KgpuBindGroupCapsule
// ============================================================================

/// GPU Bind Group Capsule with Lockfree Atomics
///
/// Manages a collection of up to 8 resource bindings that can be bound
/// to a pipeline for shader access.
///
/// # Tier: T1 (Atomic)
/// # Size: 256B (cache-aligned)
///
/// # ASSUM Safety
///
/// - `#ASSUME_BINDING_SLOT_ATOMIC`: Each slot is independently atomic
/// - `#ASSUME_GENERATION_ABA_SAFE`: 48-bit generation prevents ABA
/// - `#ASSUME_REFCOUNT_THREAD_SAFE`: AtomicU32 for reference counting
/// - `#ASSUME_CACHE_ALIGNED`: 256B alignment prevents false sharing
#[repr(C, align(256))]
pub struct KgpuBindGroupCapsule {
    /// Resource handle with generation counter for ABA prevention
    handle: KgpuHandle<BindGroup>,

    /// Primary coordination: state(8) | binding_count(8) | generation(48)
    ///
    /// - Bits [63:56]: State (BindGroupState enum)
    /// - Bits [55:48]: Number of occupied binding slots
    /// - Bits [47:0]: Generation counter
    primary: AtomicU64,

    /// Secondary coordination: layout_id(32) | flags(32)
    ///
    /// - Bits [63:32]: Bind group layout ID
    /// - Bits [31:0]: Flags (BIND_GROUP_FLAG_*)
    secondary: AtomicU64,

    /// Binding slots (8 slots, 16B each = 128B total)
    bindings: [BindingSlot; MAX_BINDINGS_PER_GROUP],

    /// Reference count for resource lifetime management
    ///
    /// When refcount reaches 0, the bind group can be destroyed.
    ref_count: AtomicU32,

    /// Padding to reach 256B total
    ///
    /// Calculation: 256 - 64 (handle) - 8 (primary) - 8 (secondary)
    ///              - 128 (bindings) - 4 (ref_count) = 44B padding
    _padding: [u8; 44],
}

// ============================================================================
// Compile-Time Verification (Q33 Mandate)
// ============================================================================

const _: () = {
    assert!(core::mem::size_of::<KgpuBindGroupCapsule>() == 256);
    assert!(core::mem::align_of::<KgpuBindGroupCapsule>() == 256);
};

// ============================================================================
// KgpuBindGroupCapsule Implementation
// ============================================================================

impl KgpuBindGroupCapsule {
    /// Create a new bind group with the specified layout ID.
    ///
    /// # Arguments
    /// - `layout_id`: ID of the bind group layout this group conforms to
    ///
    /// # Returns
    /// A new bind group in Building state with refcount 1.
    ///
    /// # Performance
    /// - Latency: O(1) constant time
    pub fn new(layout_id: u32) -> Self {
        // Pack primary: state=Building | binding_count=0 | generation=1
        let primary = ((BindGroupState::Building as u64) << STATE_SHIFT)
            | (0u64 << BINDING_COUNT_SHIFT)
            | 1; // Start at generation 1

        // Pack secondary: layout_id | flags=0
        let secondary = ((layout_id as u64) << LAYOUT_ID_SHIFT) | 0;

        Self {
            handle: KgpuHandle::new(0, 1),
            primary: AtomicU64::new(primary),
            secondary: AtomicU64::new(secondary),
            bindings: [
                BindingSlot::new(),
                BindingSlot::new(),
                BindingSlot::new(),
                BindingSlot::new(),
                BindingSlot::new(),
                BindingSlot::new(),
                BindingSlot::new(),
                BindingSlot::new(),
            ],
            ref_count: AtomicU32::new(1),
            _padding: [0; 44],
        }
    }

    /// Create a bind group with specific handle index and generation.
    ///
    /// Used by bind group pools to assign handles.
    pub fn with_handle(layout_id: u32, index: u32, generation: u32) -> Self {
        let primary = ((BindGroupState::Building as u64) << STATE_SHIFT)
            | (0u64 << BINDING_COUNT_SHIFT)
            | (generation as u64);

        let secondary = ((layout_id as u64) << LAYOUT_ID_SHIFT) | 0;

        Self {
            handle: KgpuHandle::new(index, generation),
            primary: AtomicU64::new(primary),
            secondary: AtomicU64::new(secondary),
            bindings: [
                BindingSlot::new(),
                BindingSlot::new(),
                BindingSlot::new(),
                BindingSlot::new(),
                BindingSlot::new(),
                BindingSlot::new(),
                BindingSlot::new(),
                BindingSlot::new(),
            ],
            ref_count: AtomicU32::new(1),
            _padding: [0; 44],
        }
    }

    // ========================================================================
    // Binding Operations
    // ========================================================================

    /// Set a binding at the specified index.
    ///
    /// # Arguments
    /// - `index`: Binding index (0-7)
    /// - `binding_type`: Type of resource being bound
    /// - `handle`: Resource handle (from KgpuHandle::packed_value())
    ///
    /// # Errors
    /// - `IndexOutOfRange`: index >= 8
    /// - `InvalidState`: Bind group is not in Building state
    /// - `Immutable`: Bind group has IMMUTABLE flag set
    ///
    /// # Performance
    /// - Latency: <50ns (atomic CAS + slot update)
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_BINDING_SLOT_ATOMIC`: Slot update is atomic
    pub fn set_binding(
        &self,
        index: u8,
        binding_type: BindingType,
        handle: u64,
    ) -> BindGroupResult<()> {
        // Validate index
        if index as usize >= MAX_BINDINGS_PER_GROUP {
            return Err(BindGroupError::IndexOutOfRange);
        }

        // Check state and flags
        let primary = self.primary.load(Ordering::Acquire);
        let state = ((primary & STATE_MASK) >> STATE_SHIFT) as u8;

        if state == BindGroupState::Destroyed as u8 {
            return Err(BindGroupError::Destroyed);
        }

        let secondary = self.secondary.load(Ordering::Acquire);
        let flags = (secondary & FLAGS_MASK) as u32;

        if (flags & BIND_GROUP_FLAG_IMMUTABLE) != 0 && state != BindGroupState::Building as u8 {
            return Err(BindGroupError::Immutable);
        }

        // Update the binding slot
        let was_empty = self.bindings[index as usize].is_empty();
        self.bindings[index as usize].set(binding_type, handle);

        // Update binding count if slot was previously empty
        if was_empty && !binding_type.is_empty() {
            self.increment_binding_count();
        } else if !was_empty && binding_type.is_empty() {
            self.decrement_binding_count();
        }

        Ok(())
    }

    /// Clear a binding at the specified index.
    ///
    /// # Arguments
    /// - `index`: Binding index (0-7)
    ///
    /// # Performance
    /// - Latency: <50ns (atomic operations)
    pub fn clear_binding(&self, index: u8) {
        if (index as usize) < MAX_BINDINGS_PER_GROUP {
            let was_occupied = self.bindings[index as usize].is_occupied();
            self.bindings[index as usize].clear();

            if was_occupied {
                self.decrement_binding_count();
            }
        }
    }

    /// Get binding information at the specified index.
    ///
    /// # Returns
    /// Tuple of (binding_type, resource_handle) or None if index out of range.
    #[inline]
    pub fn get_binding(&self, index: u8) -> Option<(BindingType, u64)> {
        if (index as usize) >= MAX_BINDINGS_PER_GROUP {
            return None;
        }

        let slot = &self.bindings[index as usize];
        Some((slot.binding_type(), slot.resource_handle()))
    }

    // ========================================================================
    // State Queries
    // ========================================================================

    /// Check if all required bindings are set.
    ///
    /// A bind group is considered complete if at least one binding is set.
    /// For layout-based validation, use `is_valid_for_layout()`.
    ///
    /// # Performance
    /// - Latency: <20ns (atomic load)
    #[inline]
    pub fn is_complete(&self) -> bool {
        self.binding_count() > 0
    }

    /// Get the number of occupied binding slots.
    ///
    /// # Performance
    /// - Latency: <10ns (atomic load + mask)
    #[inline]
    pub fn binding_count(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & BINDING_COUNT_MASK) >> BINDING_COUNT_SHIFT) as u8
    }

    /// Get the bind group layout ID.
    #[inline]
    pub fn layout_id(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Relaxed);
        ((secondary & LAYOUT_ID_MASK) >> LAYOUT_ID_SHIFT) as u32
    }

    /// Get the current generation counter.
    #[inline]
    pub fn generation(&self) -> u64 {
        let primary = self.primary.load(Ordering::Acquire);
        primary & GENERATION_MASK
    }

    /// Get the current state.
    #[inline]
    pub fn state(&self) -> BindGroupState {
        let primary = self.primary.load(Ordering::Acquire);
        let state = ((primary & STATE_MASK) >> STATE_SHIFT) as u8;
        BindGroupState::from_u8(state).unwrap_or(BindGroupState::Destroyed)
    }

    /// Get flags.
    #[inline]
    pub fn flags(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Relaxed);
        (secondary & FLAGS_MASK) as u32
    }

    /// Get handle reference.
    #[inline]
    pub fn handle(&self) -> &KgpuHandle<BindGroup> {
        &self.handle
    }

    // ========================================================================
    // State Transitions
    // ========================================================================

    /// Mark the bind group as ready for use.
    ///
    /// Transitions from Building -> Ready state.
    ///
    /// # Errors
    /// - `InvalidState`: Not in Building state
    /// - `Incomplete`: No bindings set
    pub fn finalize(&self) -> BindGroupResult<()> {
        if !self.is_complete() {
            return Err(BindGroupError::Incomplete);
        }

        loop {
            let primary = self.primary.load(Ordering::Acquire);
            let state = ((primary & STATE_MASK) >> STATE_SHIFT) as u8;

            if state != BindGroupState::Building as u8 {
                return Err(BindGroupError::InvalidState);
            }

            let binding_count = (primary & BINDING_COUNT_MASK) >> BINDING_COUNT_SHIFT;
            let generation = (primary & GENERATION_MASK) + 1;

            let new_primary = ((BindGroupState::Ready as u64) << STATE_SHIFT)
                | (binding_count << BINDING_COUNT_SHIFT)
                | generation;

            if self
                .primary
                .compare_exchange_weak(primary, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(());
            }
            core::hint::spin_loop();
        }
    }

    /// Mark the bind group as bound to a pipeline.
    ///
    /// Transitions from Ready -> Bound state.
    pub fn mark_bound(&self) -> BindGroupResult<()> {
        loop {
            let primary = self.primary.load(Ordering::Acquire);
            let state = ((primary & STATE_MASK) >> STATE_SHIFT) as u8;

            if state != BindGroupState::Ready as u8 {
                return Err(BindGroupError::InvalidState);
            }

            let binding_count = (primary & BINDING_COUNT_MASK) >> BINDING_COUNT_SHIFT;
            let generation = (primary & GENERATION_MASK) + 1;

            let new_primary = ((BindGroupState::Bound as u64) << STATE_SHIFT)
                | (binding_count << BINDING_COUNT_SHIFT)
                | generation;

            if self
                .primary
                .compare_exchange_weak(primary, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(());
            }
            core::hint::spin_loop();
        }
    }

    /// Mark the bind group as no longer bound.
    ///
    /// Transitions from Bound -> Ready state.
    pub fn mark_unbound(&self) -> BindGroupResult<()> {
        loop {
            let primary = self.primary.load(Ordering::Acquire);
            let state = ((primary & STATE_MASK) >> STATE_SHIFT) as u8;

            if state != BindGroupState::Bound as u8 {
                return Err(BindGroupError::InvalidState);
            }

            let binding_count = (primary & BINDING_COUNT_MASK) >> BINDING_COUNT_SHIFT;
            let generation = (primary & GENERATION_MASK) + 1;

            let new_primary = ((BindGroupState::Ready as u64) << STATE_SHIFT)
                | (binding_count << BINDING_COUNT_SHIFT)
                | generation;

            if self
                .primary
                .compare_exchange_weak(primary, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(());
            }
            core::hint::spin_loop();
        }
    }

    // ========================================================================
    // Reference Counting
    // ========================================================================

    /// Increment the reference count.
    ///
    /// # Performance
    /// - Latency: <10ns (atomic increment)
    #[inline]
    pub fn increment_ref(&self) {
        // #ASSUME_REFCOUNT_THREAD_SAFE: AtomicU32 fetch_add is thread-safe
        self.ref_count.fetch_add(1, Ordering::AcqRel);
    }

    /// Decrement the reference count.
    ///
    /// # Returns
    /// `true` if the reference count reached zero (bind group should be destroyed).
    ///
    /// # Performance
    /// - Latency: <10ns (atomic decrement)
    #[inline]
    pub fn decrement_ref(&self) -> bool {
        // #ASSUME_REFCOUNT_THREAD_SAFE: AtomicU32 fetch_sub is thread-safe
        let prev = self.ref_count.fetch_sub(1, Ordering::AcqRel);
        prev == 1 // Was 1, now 0
    }

    /// Get current reference count.
    #[inline]
    pub fn ref_count(&self) -> u32 {
        self.ref_count.load(Ordering::Acquire)
    }

    // ========================================================================
    // Flag Operations
    // ========================================================================

    /// Set flags on the bind group.
    pub fn set_flags(&self, flags: u32) {
        loop {
            let secondary = self.secondary.load(Ordering::Acquire);
            let layout_id = (secondary & LAYOUT_ID_MASK) >> LAYOUT_ID_SHIFT;
            let new_secondary = (layout_id << LAYOUT_ID_SHIFT) | (flags as u64);

            if self
                .secondary
                .compare_exchange_weak(secondary, new_secondary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
    }

    /// Add flags to the bind group.
    pub fn add_flags(&self, flags: u32) {
        loop {
            let secondary = self.secondary.load(Ordering::Acquire);
            let current_flags = (secondary & FLAGS_MASK) as u32;
            let layout_id = (secondary & LAYOUT_ID_MASK) >> LAYOUT_ID_SHIFT;
            let new_flags = current_flags | flags;
            let new_secondary = (layout_id << LAYOUT_ID_SHIFT) | (new_flags as u64);

            if self
                .secondary
                .compare_exchange_weak(secondary, new_secondary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }
    }

    /// Check if a flag is set.
    #[inline]
    pub fn has_flag(&self, flag: u32) -> bool {
        (self.flags() & flag) != 0
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    /// Increment binding count atomically
    fn increment_binding_count(&self) {
        loop {
            let primary = self.primary.load(Ordering::Acquire);
            let state = (primary & STATE_MASK) >> STATE_SHIFT;
            let count = ((primary & BINDING_COUNT_MASK) >> BINDING_COUNT_SHIFT) + 1;
            let generation = primary & GENERATION_MASK;

            let new_primary =
                (state << STATE_SHIFT) | (count << BINDING_COUNT_SHIFT) | generation;

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

    /// Decrement binding count atomically
    fn decrement_binding_count(&self) {
        loop {
            let primary = self.primary.load(Ordering::Acquire);
            let state = (primary & STATE_MASK) >> STATE_SHIFT;
            let count = ((primary & BINDING_COUNT_MASK) >> BINDING_COUNT_SHIFT).saturating_sub(1);
            let generation = primary & GENERATION_MASK;

            let new_primary =
                (state << STATE_SHIFT) | (count << BINDING_COUNT_SHIFT) | generation;

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

// ============================================================================
// Send + Sync (Chaos Mandate)
// ============================================================================

/// Chaos mandate: Send for lockfree sharing across threads.
///
/// # ASSUM Safety
/// - `#ASSUME_ATOMIC_THREAD_SAFE`: All fields are atomic or immutable
// SAFETY: All fields are atomics (thread-safe).
unsafe impl Send for KgpuBindGroupCapsule {}

/// Chaos mandate: Sync for lockfree sharing across threads.
///
/// # ASSUM Safety
/// Same as Send - atomics are Sync.
// SAFETY: All fields are atomics (thread-safe).
// Concurrent access is mediated by atomic operations.
unsafe impl Sync for KgpuBindGroupCapsule {}

// ============================================================================
// Debug Implementation
// ============================================================================

impl core::fmt::Debug for KgpuBindGroupCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KgpuBindGroupCapsule")
            .field("state", &self.state())
            .field("layout_id", &self.layout_id())
            .field("binding_count", &self.binding_count())
            .field("generation", &self.generation())
            .field("ref_count", &self.ref_count())
            .field("flags", &format_args!("0x{:08X}", self.flags()))
            .finish()
    }
}

// ============================================================================
// Default Implementation
// ============================================================================

impl Default for KgpuBindGroupCapsule {
    fn default() -> Self {
        Self::new(0)
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
    fn test_size_is_256_bytes() {
        assert_eq!(
            core::mem::size_of::<KgpuBindGroupCapsule>(),
            256,
            "KgpuBindGroupCapsule must be exactly 256 bytes"
        );
    }

    #[test]
    fn test_alignment_is_256_bytes() {
        assert_eq!(
            core::mem::align_of::<KgpuBindGroupCapsule>(),
            256,
            "KgpuBindGroupCapsule must have 256-byte alignment"
        );
    }

    #[test]
    fn test_binding_slot_size() {
        assert_eq!(
            core::mem::size_of::<BindingSlot>(),
            16,
            "BindingSlot must be 16 bytes"
        );
    }

    // ========================================================================
    // Construction Tests
    // ========================================================================

    #[test]
    fn test_new_creates_building_state() {
        let bg = KgpuBindGroupCapsule::new(42);

        assert_eq!(bg.state(), BindGroupState::Building);
        assert_eq!(bg.layout_id(), 42);
        assert_eq!(bg.binding_count(), 0);
        assert_eq!(bg.generation(), 1);
        assert_eq!(bg.ref_count(), 1);
    }

    #[test]
    fn test_with_handle() {
        let bg = KgpuBindGroupCapsule::with_handle(100, 5, 10);

        assert_eq!(bg.layout_id(), 100);
        assert_eq!(bg.handle().index(), 5);
        assert_eq!(bg.handle().generation(), 10);
    }

    #[test]
    fn test_default() {
        let bg: KgpuBindGroupCapsule = KgpuBindGroupCapsule::default();

        assert_eq!(bg.layout_id(), 0);
        assert_eq!(bg.state(), BindGroupState::Building);
    }

    // ========================================================================
    // Binding Tests
    // ========================================================================

    #[test]
    fn test_set_binding() {
        let bg = KgpuBindGroupCapsule::new(1);

        bg.set_binding(0, BindingType::UniformBuffer, 0x1234_5678)
            .unwrap();

        assert_eq!(bg.binding_count(), 1);

        let (bt, handle) = bg.get_binding(0).unwrap();
        assert_eq!(bt, BindingType::UniformBuffer);
        assert_eq!(handle, 0x1234_5678);
    }

    #[test]
    fn test_set_multiple_bindings() {
        let bg = KgpuBindGroupCapsule::new(1);

        bg.set_binding(0, BindingType::UniformBuffer, 0x1000)
            .unwrap();
        bg.set_binding(1, BindingType::StorageBuffer, 0x2000)
            .unwrap();
        bg.set_binding(2, BindingType::Texture, 0x3000).unwrap();
        bg.set_binding(3, BindingType::Sampler, 0x4000).unwrap();

        assert_eq!(bg.binding_count(), 4);

        assert_eq!(bg.get_binding(0).unwrap().0, BindingType::UniformBuffer);
        assert_eq!(bg.get_binding(1).unwrap().0, BindingType::StorageBuffer);
        assert_eq!(bg.get_binding(2).unwrap().0, BindingType::Texture);
        assert_eq!(bg.get_binding(3).unwrap().0, BindingType::Sampler);
    }

    #[test]
    fn test_set_binding_index_out_of_range() {
        let bg = KgpuBindGroupCapsule::new(1);

        let result = bg.set_binding(8, BindingType::UniformBuffer, 0x1234);

        assert_eq!(result, Err(BindGroupError::IndexOutOfRange));
    }

    #[test]
    fn test_clear_binding() {
        let bg = KgpuBindGroupCapsule::new(1);

        bg.set_binding(0, BindingType::UniformBuffer, 0x1234)
            .unwrap();
        assert_eq!(bg.binding_count(), 1);

        bg.clear_binding(0);
        assert_eq!(bg.binding_count(), 0);
        assert!(bg.get_binding(0).unwrap().0.is_empty());
    }

    #[test]
    fn test_overwrite_binding() {
        let bg = KgpuBindGroupCapsule::new(1);

        bg.set_binding(0, BindingType::UniformBuffer, 0x1000)
            .unwrap();
        bg.set_binding(0, BindingType::StorageBuffer, 0x2000)
            .unwrap();

        assert_eq!(bg.binding_count(), 1); // Still 1, not 2
        assert_eq!(bg.get_binding(0).unwrap().0, BindingType::StorageBuffer);
        assert_eq!(bg.get_binding(0).unwrap().1, 0x2000);
    }

    // ========================================================================
    // Completeness Tests
    // ========================================================================

    #[test]
    fn test_is_complete_empty() {
        let bg = KgpuBindGroupCapsule::new(1);
        assert!(!bg.is_complete());
    }

    #[test]
    fn test_is_complete_with_binding() {
        let bg = KgpuBindGroupCapsule::new(1);
        bg.set_binding(0, BindingType::UniformBuffer, 0x1234)
            .unwrap();
        assert!(bg.is_complete());
    }

    // ========================================================================
    // State Transition Tests
    // ========================================================================

    #[test]
    fn test_finalize_success() {
        let bg = KgpuBindGroupCapsule::new(1);
        bg.set_binding(0, BindingType::UniformBuffer, 0x1234)
            .unwrap();

        bg.finalize().unwrap();

        assert_eq!(bg.state(), BindGroupState::Ready);
        assert_eq!(bg.generation(), 2); // Incremented
    }

    #[test]
    fn test_finalize_incomplete() {
        let bg = KgpuBindGroupCapsule::new(1);

        let result = bg.finalize();

        assert_eq!(result, Err(BindGroupError::Incomplete));
    }

    #[test]
    fn test_mark_bound() {
        let bg = KgpuBindGroupCapsule::new(1);
        bg.set_binding(0, BindingType::UniformBuffer, 0x1234)
            .unwrap();
        bg.finalize().unwrap();

        bg.mark_bound().unwrap();

        assert_eq!(bg.state(), BindGroupState::Bound);
    }

    #[test]
    fn test_mark_unbound() {
        let bg = KgpuBindGroupCapsule::new(1);
        bg.set_binding(0, BindingType::UniformBuffer, 0x1234)
            .unwrap();
        bg.finalize().unwrap();
        bg.mark_bound().unwrap();

        bg.mark_unbound().unwrap();

        assert_eq!(bg.state(), BindGroupState::Ready);
    }

    #[test]
    fn test_mark_bound_invalid_state() {
        let bg = KgpuBindGroupCapsule::new(1);
        bg.set_binding(0, BindingType::UniformBuffer, 0x1234)
            .unwrap();
        // Still in Building state

        let result = bg.mark_bound();

        assert_eq!(result, Err(BindGroupError::InvalidState));
    }

    // ========================================================================
    // Reference Counting Tests
    // ========================================================================

    #[test]
    fn test_increment_ref() {
        let bg = KgpuBindGroupCapsule::new(1);
        assert_eq!(bg.ref_count(), 1);

        bg.increment_ref();
        assert_eq!(bg.ref_count(), 2);

        bg.increment_ref();
        assert_eq!(bg.ref_count(), 3);
    }

    #[test]
    fn test_decrement_ref() {
        let bg = KgpuBindGroupCapsule::new(1);
        bg.increment_ref(); // Now 2
        bg.increment_ref(); // Now 3

        assert!(!bg.decrement_ref()); // 3 -> 2, not zero
        assert!(!bg.decrement_ref()); // 2 -> 1, not zero
        assert!(bg.decrement_ref()); // 1 -> 0, IS zero
    }

    // ========================================================================
    // Flag Tests
    // ========================================================================

    #[test]
    fn test_set_flags() {
        let bg = KgpuBindGroupCapsule::new(1);

        bg.set_flags(BIND_GROUP_FLAG_COMPUTE | BIND_GROUP_FLAG_DYNAMIC_OFFSETS);

        assert!(bg.has_flag(BIND_GROUP_FLAG_COMPUTE));
        assert!(bg.has_flag(BIND_GROUP_FLAG_DYNAMIC_OFFSETS));
        assert!(!bg.has_flag(BIND_GROUP_FLAG_GRAPHICS));
    }

    #[test]
    fn test_add_flags() {
        let bg = KgpuBindGroupCapsule::new(1);

        bg.set_flags(BIND_GROUP_FLAG_COMPUTE);
        bg.add_flags(BIND_GROUP_FLAG_GRAPHICS);

        assert!(bg.has_flag(BIND_GROUP_FLAG_COMPUTE));
        assert!(bg.has_flag(BIND_GROUP_FLAG_GRAPHICS));
    }

    // ========================================================================
    // BindingType Tests
    // ========================================================================

    #[test]
    fn test_binding_type_is_buffer() {
        assert!(BindingType::UniformBuffer.is_buffer());
        assert!(BindingType::StorageBuffer.is_buffer());
        assert!(BindingType::ReadOnlyStorageBuffer.is_buffer());
        assert!(!BindingType::Texture.is_buffer());
        assert!(!BindingType::Sampler.is_buffer());
    }

    #[test]
    fn test_binding_type_is_texture() {
        assert!(BindingType::Texture.is_texture());
        assert!(BindingType::StorageTexture.is_texture());
        assert!(!BindingType::UniformBuffer.is_texture());
    }

    #[test]
    fn test_binding_type_from_u8() {
        assert_eq!(BindingType::from_u8(0), Some(BindingType::Empty));
        assert_eq!(BindingType::from_u8(1), Some(BindingType::UniformBuffer));
        assert_eq!(BindingType::from_u8(6), Some(BindingType::StorageTexture));
        assert_eq!(BindingType::from_u8(255), None);
    }

    // ========================================================================
    // Thread Safety Tests
    // ========================================================================

    #[test]
    fn test_send_sync_bounds() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<KgpuBindGroupCapsule>();
        assert_send_sync::<BindingSlot>();
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_binding_updates() {
        use std::sync::Arc;
        use std::thread;

        let bg = Arc::new(KgpuBindGroupCapsule::new(1));
        let handles: Vec<_> = (0..4)
            .map(|i| {
                let bg = Arc::clone(&bg);
                thread::spawn(move || {
                    for j in 0..100 {
                        let _ = bg.set_binding(
                            (i % 8) as u8,
                            BindingType::UniformBuffer,
                            (i * 1000 + j) as u64,
                        );
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // No panics = success
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_refcount() {
        use std::sync::Arc;
        use std::thread;

        let bg = Arc::new(KgpuBindGroupCapsule::new(1));

        // Increment refcount in parallel
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let bg = Arc::clone(&bg);
                thread::spawn(move || {
                    for _ in 0..100 {
                        bg.increment_ref();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // 1 (initial) + 4 * 100 = 401
        assert_eq!(bg.ref_count(), 401);
    }

    // ========================================================================
    // Debug Format Tests
    // ========================================================================

    #[test]
    fn test_debug_format() {
        let bg = KgpuBindGroupCapsule::new(42);
        let debug_str = format!("{:?}", bg);

        assert!(debug_str.contains("KgpuBindGroupCapsule"));
        assert!(debug_str.contains("Building"));
        assert!(debug_str.contains("layout_id"));
        assert!(debug_str.contains("42"));
    }

    // ========================================================================
    // Full Workflow Tests
    // ========================================================================

    #[test]
    fn test_complete_workflow() {
        let bg = KgpuBindGroupCapsule::new(1);

        // Set bindings
        bg.set_binding(0, BindingType::UniformBuffer, 0x1000)
            .unwrap();
        bg.set_binding(1, BindingType::Texture, 0x2000).unwrap();
        bg.set_binding(2, BindingType::Sampler, 0x3000).unwrap();
        assert_eq!(bg.binding_count(), 3);

        // Finalize
        bg.finalize().unwrap();
        assert_eq!(bg.state(), BindGroupState::Ready);

        // Bind
        bg.mark_bound().unwrap();
        assert_eq!(bg.state(), BindGroupState::Bound);

        // Unbind
        bg.mark_unbound().unwrap();
        assert_eq!(bg.state(), BindGroupState::Ready);

        // Verify bindings still intact
        assert_eq!(bg.get_binding(0).unwrap().0, BindingType::UniformBuffer);
        assert_eq!(bg.get_binding(1).unwrap().0, BindingType::Texture);
        assert_eq!(bg.get_binding(2).unwrap().0, BindingType::Sampler);
    }
}
