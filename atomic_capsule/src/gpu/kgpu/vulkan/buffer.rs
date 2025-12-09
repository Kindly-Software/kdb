//! VkBufferCapsule - Vulkan Buffer (Mock)
//!
//! **Tier**: T1 Atomic
//! **Size**: 128B cache-aligned
//! **Purpose**: Mock Vulkan buffer for design validation
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_MOCK_BUFFER`: This is a mock, not real Vulkan FFI
//! - `#ASSUME_HANDLE_VALID`: Mock handles are always "valid" (non-zero)
//! - `#ASSUME_STATE_ATOMIC`: All state changes use atomic operations
//! - `#ASSUME_MEMORY_BINDING`: Memory binding is simulated

use core::ptr::null_mut;
use core::sync::atomic::{AtomicPtr, AtomicU64, Ordering};

use super::types::{generate_mock_handle, VkBufferUsageFlags, VkMemoryPropertyFlags, VkResult};

// ============================================================================
// State Constants
// ============================================================================

/// Buffer is not initialized
pub const VK_BUFFER_STATE_UNINITIALIZED: u8 = 0;
/// Buffer is created but not bound
pub const VK_BUFFER_STATE_CREATED: u8 = 1;
/// Buffer is bound to memory
pub const VK_BUFFER_STATE_BOUND: u8 = 2;
/// Buffer is mapped for CPU access
pub const VK_BUFFER_STATE_MAPPED: u8 = 3;
/// Buffer is in use by GPU
pub const VK_BUFFER_STATE_IN_GPU_USE: u8 = 4;
/// Buffer has been destroyed
pub const VK_BUFFER_STATE_DESTROYED: u8 = 5;

/// Buffer state enum for type-safe state management
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VkBufferState {
    /// Buffer is not initialized
    Uninitialized = VK_BUFFER_STATE_UNINITIALIZED,
    /// Buffer is created but not bound
    Created = VK_BUFFER_STATE_CREATED,
    /// Buffer is bound to memory
    Bound = VK_BUFFER_STATE_BOUND,
    /// Buffer is mapped for CPU access
    Mapped = VK_BUFFER_STATE_MAPPED,
    /// Buffer is in use by GPU
    InGpuUse = VK_BUFFER_STATE_IN_GPU_USE,
    /// Buffer has been destroyed
    Destroyed = VK_BUFFER_STATE_DESTROYED,
}

impl From<u8> for VkBufferState {
    fn from(value: u8) -> Self {
        match value {
            VK_BUFFER_STATE_UNINITIALIZED => VkBufferState::Uninitialized,
            VK_BUFFER_STATE_CREATED => VkBufferState::Created,
            VK_BUFFER_STATE_BOUND => VkBufferState::Bound,
            VK_BUFFER_STATE_MAPPED => VkBufferState::Mapped,
            VK_BUFFER_STATE_IN_GPU_USE => VkBufferState::InGpuUse,
            VK_BUFFER_STATE_DESTROYED => VkBufferState::Destroyed,
            _ => VkBufferState::Uninitialized,
        }
    }
}

impl VkBufferState {
    /// Get the raw u8 value
    #[inline]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

// ============================================================================
// Bit Field Layouts
// ============================================================================

const STATE_SHIFT: u64 = 56;
const STATE_MASK: u64 = 0xFF << STATE_SHIFT;
const USAGE_SHIFT: u64 = 40;
const USAGE_MASK: u64 = 0xFFFF << USAGE_SHIFT;
const GENERATION_MASK: u64 = 0x0000_00FF_FFFF_FFFF;

// ============================================================================
// Create Info
// ============================================================================

/// Buffer creation parameters
#[derive(Debug, Clone)]
pub struct VkBufferCreateInfo {
    /// Size in bytes
    pub size: u64,
    /// Usage flags
    pub usage: VkBufferUsageFlags,
    /// Memory properties
    pub memory_properties: VkMemoryPropertyFlags,
}

impl VkBufferCreateInfo {
    /// Create a vertex buffer create info
    pub fn vertex(size: u64) -> Self {
        Self {
            size,
            usage: VkBufferUsageFlags::VERTEX_BUFFER | VkBufferUsageFlags::TRANSFER_DST,
            memory_properties: VkMemoryPropertyFlags::DEVICE_LOCAL,
        }
    }

    /// Create an index buffer create info
    pub fn index(size: u64) -> Self {
        Self {
            size,
            usage: VkBufferUsageFlags::INDEX_BUFFER | VkBufferUsageFlags::TRANSFER_DST,
            memory_properties: VkMemoryPropertyFlags::DEVICE_LOCAL,
        }
    }

    /// Create a uniform buffer create info
    pub fn uniform(size: u64) -> Self {
        Self {
            size,
            usage: VkBufferUsageFlags::UNIFORM_BUFFER,
            memory_properties: VkMemoryPropertyFlags::HOST_VISIBLE
                | VkMemoryPropertyFlags::HOST_COHERENT,
        }
    }

    /// Create a staging buffer create info
    pub fn staging(size: u64) -> Self {
        Self {
            size,
            usage: VkBufferUsageFlags::TRANSFER_SRC,
            memory_properties: VkMemoryPropertyFlags::HOST_VISIBLE
                | VkMemoryPropertyFlags::HOST_COHERENT,
        }
    }

    /// Create a storage buffer create info
    pub fn storage(size: u64) -> Self {
        Self {
            size,
            usage: VkBufferUsageFlags::STORAGE_BUFFER | VkBufferUsageFlags::TRANSFER_DST,
            memory_properties: VkMemoryPropertyFlags::DEVICE_LOCAL,
        }
    }
}

// ============================================================================
// VkBufferCapsule
// ============================================================================

/// Mock Vulkan Buffer Capsule
///
/// # Tier: T1 Atomic
/// # Size: 128B cache-aligned
///
/// # Memory Layout
///
/// - handle: Mock VkBuffer handle
/// - memory: Mock VkDeviceMemory handle
/// - primary: state(8) | usage(16) | generation(40)
/// - size: Buffer size in bytes
/// - offset: Memory offset
/// - mapped_ptr: Mapped memory pointer (if mapped)
///
/// # ASSUM Safety
///
/// - `#ASSUME_MOCK_BUFFER`: No real Vulkan operations
/// - `#ASSUME_STATE_ATOMIC`: Lockfree state management
/// - `#ASSUME_MAP_VALID`: Mapped pointer checked before use
#[repr(C, align(128))]
pub struct VkBufferCapsule {
    /// Mock VkBuffer handle
    handle: AtomicU64,

    /// Mock VkDeviceMemory handle
    memory: AtomicU64,

    /// Primary coordination: state(8) | usage(16) | generation(40)
    primary: AtomicU64,

    /// Buffer size in bytes
    size: AtomicU64,

    /// Memory offset (for sub-allocation)
    offset: AtomicU64,

    /// Mapped memory pointer
    mapped_ptr: AtomicPtr<u8>,

    /// Memory properties
    memory_properties: AtomicU64,

    /// Reserved for alignment
    _reserved: [u8; 64],
}

// Compile-time verification
const _: () = {
    assert!(core::mem::size_of::<VkBufferCapsule>() == 128);
    assert!(core::mem::align_of::<VkBufferCapsule>() == 128);
};

impl VkBufferCapsule {
    /// Create a new buffer capsule in uninitialized state
    pub const fn new() -> Self {
        Self {
            handle: AtomicU64::new(0),
            memory: AtomicU64::new(0),
            primary: AtomicU64::new(0),
            size: AtomicU64::new(0),
            offset: AtomicU64::new(0),
            mapped_ptr: AtomicPtr::new(null_mut()),
            memory_properties: AtomicU64::new(0),
            _reserved: [0; 64],
        }
    }

    /// Create and initialize a buffer
    ///
    /// # Arguments
    ///
    /// * `info` - Buffer creation parameters
    ///
    /// # Returns
    ///
    /// `VkResult::Success` on success
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_STATE_TRANSITION`: Uninitialized -> Created
    /// - `#ASSUME_SIZE_VALID`: Size must be > 0
    pub fn create(&self, info: &VkBufferCreateInfo) -> VkResult {
        if info.size == 0 {
            return VkResult::ErrorInitializationFailed;
        }

        let current = self.primary.load(Ordering::Acquire);
        let state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

        if state != VK_BUFFER_STATE_UNINITIALIZED {
            return VkResult::ErrorInitializationFailed;
        }

        // Generate handle
        let handle = generate_mock_handle();
        self.handle.store(handle, Ordering::Release);

        // Store size and usage
        self.size.store(info.size, Ordering::Release);
        self.memory_properties
            .store(info.memory_properties.0 as u64, Ordering::Release);

        // Update primary with state and usage
        let gen = (current & GENERATION_MASK).wrapping_add(1) & GENERATION_MASK;
        let new_primary = ((VK_BUFFER_STATE_CREATED as u64) << STATE_SHIFT)
            | ((info.usage.0 as u64) << USAGE_SHIFT)
            | gen;

        self.primary.store(new_primary, Ordering::Release);

        VkResult::Success
    }

    /// Bind memory to the buffer
    ///
    /// # Arguments
    ///
    /// * `memory` - Mock memory handle
    /// * `offset` - Offset within memory
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_MEMORY_VALID`: Memory handle assumed valid (mock)
    /// - `#ASSUME_STATE_TRANSITION`: Created -> Bound
    pub fn bind_memory(&self, memory: u64, offset: u64) -> VkResult {
        if self.state() != VK_BUFFER_STATE_CREATED {
            return VkResult::ErrorInitializationFailed;
        }

        self.memory.store(memory, Ordering::Release);
        self.offset.store(offset, Ordering::Release);

        // Update state to Bound
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let gen = (current & GENERATION_MASK).wrapping_add(1) & GENERATION_MASK;
            let usage = current & USAGE_MASK;
            let new_primary = ((VK_BUFFER_STATE_BOUND as u64) << STATE_SHIFT) | usage | gen;

            if self
                .primary
                .compare_exchange(current, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }

        VkResult::Success
    }

    /// Map buffer memory for CPU access
    ///
    /// # Returns
    ///
    /// Mock pointer (always non-null on success)
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_MAPPABLE`: Buffer must have HOST_VISIBLE memory
    /// - `#ASSUME_STATE_TRANSITION`: Bound -> Mapped
    pub fn map(&self) -> Result<*mut u8, VkResult> {
        let state = self.state();
        if state != VK_BUFFER_STATE_BOUND {
            return Err(VkResult::ErrorMemoryMapFailed);
        }

        // Check if mappable
        let props = VkMemoryPropertyFlags(self.memory_properties.load(Ordering::Acquire) as u32);
        if !props.contains(VkMemoryPropertyFlags::HOST_VISIBLE) {
            return Err(VkResult::ErrorMemoryMapFailed);
        }

        // Generate mock pointer (page-aligned)
        let size = self.size.load(Ordering::Acquire);
        let mock_ptr = (0x7FFF_0000_0000u64 + size) as *mut u8;

        self.mapped_ptr.store(mock_ptr, Ordering::Release);

        // Update state
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let gen = (current & GENERATION_MASK).wrapping_add(1) & GENERATION_MASK;
            let usage = current & USAGE_MASK;
            let new_primary = ((VK_BUFFER_STATE_MAPPED as u64) << STATE_SHIFT) | usage | gen;

            if self
                .primary
                .compare_exchange(current, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }

        Ok(mock_ptr)
    }

    /// Unmap buffer memory
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_STATE_TRANSITION`: Mapped -> Bound
    pub fn unmap(&self) -> VkResult {
        if self.state() != VK_BUFFER_STATE_MAPPED {
            return VkResult::ErrorMemoryMapFailed;
        }

        self.mapped_ptr.store(null_mut(), Ordering::Release);

        // Update state
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let gen = (current & GENERATION_MASK).wrapping_add(1) & GENERATION_MASK;
            let usage = current & USAGE_MASK;
            let new_primary = ((VK_BUFFER_STATE_BOUND as u64) << STATE_SHIFT) | usage | gen;

            if self
                .primary
                .compare_exchange(current, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }

        VkResult::Success
    }

    /// Destroy the buffer
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_STATE_TRANSITION`: Any -> Destroyed
    /// - `#ASSUME_CLEANUP`: All resources cleared
    pub fn destroy(&self) -> VkResult {
        let state = self.state();
        if state == VK_BUFFER_STATE_DESTROYED {
            return VkResult::Success;
        }

        if state == VK_BUFFER_STATE_MAPPED {
            self.unmap();
        }

        // Clear all fields
        self.handle.store(0, Ordering::Release);
        self.memory.store(0, Ordering::Release);
        self.size.store(0, Ordering::Release);
        self.offset.store(0, Ordering::Release);
        self.mapped_ptr.store(null_mut(), Ordering::Release);

        // Update state
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let gen = (current & GENERATION_MASK).wrapping_add(1) & GENERATION_MASK;
            let new_primary = ((VK_BUFFER_STATE_DESTROYED as u64) << STATE_SHIFT) | gen;

            if self
                .primary
                .compare_exchange(current, new_primary, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }

        VkResult::Success
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Get current state
    #[inline]
    pub fn state(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & STATE_MASK) >> STATE_SHIFT) as u8
    }

    /// Get mock handle
    #[inline]
    pub fn handle(&self) -> u64 {
        self.handle.load(Ordering::Acquire)
    }

    /// Get memory handle
    #[inline]
    pub fn memory(&self) -> u64 {
        self.memory.load(Ordering::Acquire)
    }

    /// Get buffer size
    #[inline]
    pub fn size(&self) -> u64 {
        self.size.load(Ordering::Acquire)
    }

    /// Get memory offset
    #[inline]
    pub fn offset(&self) -> u64 {
        self.offset.load(Ordering::Acquire)
    }

    /// Get mapped pointer (may be null)
    #[inline]
    pub fn mapped_ptr(&self) -> *mut u8 {
        self.mapped_ptr.load(Ordering::Acquire)
    }

    /// Get usage flags
    #[inline]
    pub fn usage(&self) -> VkBufferUsageFlags {
        let primary = self.primary.load(Ordering::Acquire);
        VkBufferUsageFlags(((primary & USAGE_MASK) >> USAGE_SHIFT) as u32)
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        let primary = self.primary.load(Ordering::Acquire);
        primary & GENERATION_MASK
    }

    /// Check if buffer is mapped
    #[inline]
    pub fn is_mapped(&self) -> bool {
        self.state() == VK_BUFFER_STATE_MAPPED
    }

    /// Check if buffer is bound
    #[inline]
    pub fn is_bound(&self) -> bool {
        let state = self.state();
        state == VK_BUFFER_STATE_BOUND || state == VK_BUFFER_STATE_MAPPED
    }
}

impl Default for VkBufferCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: All operations are atomic, no raw pointer dereferencing
unsafe impl Send for VkBufferCapsule {}
unsafe impl Sync for VkBufferCapsule {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<VkBufferCapsule>(), 128);
        assert_eq!(core::mem::align_of::<VkBufferCapsule>(), 128);
    }

    #[test]
    fn test_initial_state() {
        let buffer = VkBufferCapsule::new();
        assert_eq!(buffer.state(), VK_BUFFER_STATE_UNINITIALIZED);
        assert_eq!(buffer.handle(), 0);
        assert_eq!(buffer.size(), 0);
    }

    #[test]
    fn test_create_vertex_buffer() {
        let buffer = VkBufferCapsule::new();
        let info = VkBufferCreateInfo::vertex(1024);

        let result = buffer.create(&info);
        assert!(result.is_success());
        assert_eq!(buffer.state(), VK_BUFFER_STATE_CREATED);
        assert!(buffer.handle() > 0);
        assert_eq!(buffer.size(), 1024);
        assert!(buffer
            .usage()
            .contains(VkBufferUsageFlags::VERTEX_BUFFER));
    }

    #[test]
    fn test_create_index_buffer() {
        let buffer = VkBufferCapsule::new();
        let info = VkBufferCreateInfo::index(512);

        buffer.create(&info);
        assert!(buffer.usage().contains(VkBufferUsageFlags::INDEX_BUFFER));
    }

    #[test]
    fn test_create_uniform_buffer() {
        let buffer = VkBufferCapsule::new();
        let info = VkBufferCreateInfo::uniform(256);

        buffer.create(&info);
        assert!(buffer
            .usage()
            .contains(VkBufferUsageFlags::UNIFORM_BUFFER));
    }

    #[test]
    fn test_create_storage_buffer() {
        let buffer = VkBufferCapsule::new();
        let info = VkBufferCreateInfo::storage(4096);

        buffer.create(&info);
        assert!(buffer
            .usage()
            .contains(VkBufferUsageFlags::STORAGE_BUFFER));
    }

    #[test]
    fn test_create_staging_buffer() {
        let buffer = VkBufferCapsule::new();
        let info = VkBufferCreateInfo::staging(2048);

        buffer.create(&info);
        assert!(buffer.usage().contains(VkBufferUsageFlags::TRANSFER_SRC));
    }

    #[test]
    fn test_create_zero_size_fails() {
        let buffer = VkBufferCapsule::new();
        let info = VkBufferCreateInfo {
            size: 0,
            usage: VkBufferUsageFlags::VERTEX_BUFFER,
            memory_properties: VkMemoryPropertyFlags::DEVICE_LOCAL,
        };

        let result = buffer.create(&info);
        assert!(result.is_error());
    }

    #[test]
    fn test_bind_memory() {
        let buffer = VkBufferCapsule::new();
        buffer.create(&VkBufferCreateInfo::vertex(1024));

        let memory = generate_mock_handle();
        let result = buffer.bind_memory(memory, 0);

        assert!(result.is_success());
        assert_eq!(buffer.state(), VK_BUFFER_STATE_BOUND);
        assert_eq!(buffer.memory(), memory);
        assert_eq!(buffer.offset(), 0);
    }

    #[test]
    fn test_bind_memory_with_offset() {
        let buffer = VkBufferCapsule::new();
        buffer.create(&VkBufferCreateInfo::vertex(1024));

        let memory = generate_mock_handle();
        buffer.bind_memory(memory, 256);

        assert_eq!(buffer.offset(), 256);
    }

    #[test]
    fn test_map_uniform_buffer() {
        let buffer = VkBufferCapsule::new();
        buffer.create(&VkBufferCreateInfo::uniform(256));
        buffer.bind_memory(generate_mock_handle(), 0);

        let result = buffer.map();
        assert!(result.is_ok());
        assert_eq!(buffer.state(), VK_BUFFER_STATE_MAPPED);
        assert!(!buffer.mapped_ptr().is_null());
    }

    #[test]
    fn test_map_device_local_fails() {
        let buffer = VkBufferCapsule::new();
        buffer.create(&VkBufferCreateInfo::vertex(1024)); // Device local
        buffer.bind_memory(generate_mock_handle(), 0);

        let result = buffer.map();
        assert!(result.is_err());
    }

    #[test]
    fn test_unmap() {
        let buffer = VkBufferCapsule::new();
        buffer.create(&VkBufferCreateInfo::uniform(256));
        buffer.bind_memory(generate_mock_handle(), 0);
        buffer.map().unwrap();

        let result = buffer.unmap();
        assert!(result.is_success());
        assert_eq!(buffer.state(), VK_BUFFER_STATE_BOUND);
        assert!(buffer.mapped_ptr().is_null());
    }

    #[test]
    fn test_destroy() {
        let buffer = VkBufferCapsule::new();
        buffer.create(&VkBufferCreateInfo::vertex(1024));
        buffer.bind_memory(generate_mock_handle(), 0);

        let result = buffer.destroy();
        assert!(result.is_success());
        assert_eq!(buffer.state(), VK_BUFFER_STATE_DESTROYED);
        assert_eq!(buffer.handle(), 0);
    }

    #[test]
    fn test_destroy_mapped_buffer() {
        let buffer = VkBufferCapsule::new();
        buffer.create(&VkBufferCreateInfo::uniform(256));
        buffer.bind_memory(generate_mock_handle(), 0);
        buffer.map().unwrap();

        let result = buffer.destroy();
        assert!(result.is_success());
        assert!(buffer.mapped_ptr().is_null());
    }

    #[test]
    fn test_double_destroy() {
        let buffer = VkBufferCapsule::new();
        buffer.create(&VkBufferCreateInfo::vertex(1024));
        buffer.destroy();

        let result = buffer.destroy();
        assert!(result.is_success()); // Idempotent
    }

    #[test]
    fn test_generation_increments() {
        let buffer = VkBufferCapsule::new();
        let gen0 = buffer.generation();

        buffer.create(&VkBufferCreateInfo::vertex(1024));
        let gen1 = buffer.generation();
        assert!(gen1 > gen0);

        buffer.bind_memory(generate_mock_handle(), 0);
        let gen2 = buffer.generation();
        assert!(gen2 > gen1);
    }

    #[test]
    fn test_is_bound() {
        let buffer = VkBufferCapsule::new();
        assert!(!buffer.is_bound());

        buffer.create(&VkBufferCreateInfo::uniform(256));
        assert!(!buffer.is_bound());

        buffer.bind_memory(generate_mock_handle(), 0);
        assert!(buffer.is_bound());

        buffer.map().unwrap();
        assert!(buffer.is_bound()); // Mapped implies bound
    }
}
