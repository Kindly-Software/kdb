//! VkDeviceCapsule - Vulkan Logical Device (Mock)
//!
//! **Tier**: T1+T7 (Atomic + Heterogeneous GPU)
//! **Size**: 512B cache-aligned
//! **Purpose**: Mock Vulkan logical device for design validation
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_MOCK_DEVICE`: This is a mock, not real Vulkan FFI
//! - `#ASSUME_HANDLE_VALID`: Mock handles are always "valid" (non-zero)
//! - `#ASSUME_STATE_ATOMIC`: All state changes use atomic operations
//!
//! # Memory Layout (512B)
//!
//! ```text
//! Offset  Size    Field
//! 0       8       handle (mock VkDevice)
//! 8       8       primary: state(8) | queue_count(8) | generation(48)
//! 16      8       secondary: enabled_features(32) | limits_hash(32)
//! 24      8       graphics_queue
//! 32      8       compute_queue
//! 40      8       transfer_queue
//! 48      4       buffer_count
//! 52      4       image_count
//! 56      4       sampler_count
//! 60      4       descriptor_set_count
//! 64      4       pipeline_count
//! 68      4       padding
//! 72      8       device_local_allocated
//! 80      8       host_visible_allocated
//! 88      424     reserved
//! ```

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use super::types::{generate_mock_handle, VkQueueFlags, VkResult, VK_API_VERSION_1_3};

// ============================================================================
// State Constants
// ============================================================================

/// Device is not initialized
pub const VK_DEVICE_STATE_UNINITIALIZED: u8 = 0;
/// Device is being created
pub const VK_DEVICE_STATE_CREATING: u8 = 1;
/// Device is active
pub const VK_DEVICE_STATE_ACTIVE: u8 = 2;
/// Device is idle (waiting)
pub const VK_DEVICE_STATE_IDLE: u8 = 3;
/// Device has been lost
pub const VK_DEVICE_STATE_LOST: u8 = 4;
/// Device is being destroyed
pub const VK_DEVICE_STATE_DESTROYING: u8 = 5;
/// Device has been destroyed
pub const VK_DEVICE_STATE_DESTROYED: u8 = 6;

/// Vulkan device state enum (for type-safe API)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VkDeviceState {
    /// Device is not initialized
    Uninitialized = VK_DEVICE_STATE_UNINITIALIZED,
    /// Device is being created
    Creating = VK_DEVICE_STATE_CREATING,
    /// Device is active and ready for use
    Active = VK_DEVICE_STATE_ACTIVE,
    /// Device is idle (waiting for work)
    Idle = VK_DEVICE_STATE_IDLE,
    /// Device has been lost (GPU reset, etc.)
    Lost = VK_DEVICE_STATE_LOST,
    /// Device is being destroyed
    Destroying = VK_DEVICE_STATE_DESTROYING,
    /// Device has been destroyed
    Destroyed = VK_DEVICE_STATE_DESTROYED,
}

impl From<u8> for VkDeviceState {
    fn from(val: u8) -> Self {
        match val {
            VK_DEVICE_STATE_UNINITIALIZED => VkDeviceState::Uninitialized,
            VK_DEVICE_STATE_CREATING => VkDeviceState::Creating,
            VK_DEVICE_STATE_ACTIVE => VkDeviceState::Active,
            VK_DEVICE_STATE_IDLE => VkDeviceState::Idle,
            VK_DEVICE_STATE_LOST => VkDeviceState::Lost,
            VK_DEVICE_STATE_DESTROYING => VkDeviceState::Destroying,
            VK_DEVICE_STATE_DESTROYED => VkDeviceState::Destroyed,
            _ => VkDeviceState::Uninitialized,
        }
    }
}

/// Vulkan device features bitflags (wrapper for type-safe API)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VkDeviceFeatures(pub u32);

impl VkDeviceFeatures {
    /// Geometry shader support
    pub const GEOMETRY_SHADER: Self = Self(VK_FEATURE_GEOMETRY_SHADER);
    /// Tessellation shader support
    pub const TESSELLATION_SHADER: Self = Self(VK_FEATURE_TESSELLATION_SHADER);
    /// Multi-viewport support
    pub const MULTI_VIEWPORT: Self = Self(VK_FEATURE_MULTI_VIEWPORT);
    /// Sampler anisotropy support
    pub const SAMPLER_ANISOTROPY: Self = Self(VK_FEATURE_SAMPLER_ANISOTROPY);
    /// Texture compression BC support
    pub const TEXTURE_COMPRESSION_BC: Self = Self(VK_FEATURE_TEXTURE_COMPRESSION_BC);
    /// 64-bit integer shader support
    pub const SHADER_INT64: Self = Self(VK_FEATURE_SHADER_INT64);
    /// 16-bit float shader support
    pub const SHADER_FLOAT16: Self = Self(VK_FEATURE_SHADER_FLOAT16);
    /// Timeline semaphore support
    pub const TIMELINE_SEMAPHORE: Self = Self(VK_FEATURE_TIMELINE_SEMAPHORE);
    /// Buffer device address support
    pub const BUFFER_DEVICE_ADDRESS: Self = Self(VK_FEATURE_BUFFER_DEVICE_ADDRESS);
    /// Descriptor indexing support
    pub const DESCRIPTOR_INDEXING: Self = Self(VK_FEATURE_DESCRIPTOR_INDEXING);

    /// Returns true if features contain the specified feature
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns the raw feature bits
    #[inline]
    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl core::ops::BitOr for VkDeviceFeatures {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

/// Queue family information
#[derive(Debug, Clone, Copy, Default)]
pub struct VkQueueInfo {
    /// Queue family index
    pub family_index: u32,
    /// Queue index within family
    pub queue_index: u32,
    /// Queue flags (capabilities)
    pub flags: VkQueueFlags,
    /// Queue priority (0.0 - 1.0)
    pub priority: f32,
}

// ============================================================================
// Bit Field Layouts
// ============================================================================

const STATE_SHIFT: u64 = 56;
const STATE_MASK: u64 = 0xFF << STATE_SHIFT;
const QUEUE_COUNT_SHIFT: u64 = 48;
const QUEUE_COUNT_MASK: u64 = 0xFF << QUEUE_COUNT_SHIFT;
const GENERATION_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

const FEATURES_SHIFT: u64 = 32;
const FEATURES_MASK: u64 = 0xFFFF_FFFF << FEATURES_SHIFT;
const LIMITS_HASH_MASK: u64 = 0x0000_0000_FFFF_FFFF;

// ============================================================================
// Feature Flags
// ============================================================================

/// Device supports geometry shaders
pub const VK_FEATURE_GEOMETRY_SHADER: u32 = 1 << 0;
/// Device supports tessellation shaders
pub const VK_FEATURE_TESSELLATION_SHADER: u32 = 1 << 1;
/// Device supports multi-viewport
pub const VK_FEATURE_MULTI_VIEWPORT: u32 = 1 << 2;
/// Device supports sampler anisotropy
pub const VK_FEATURE_SAMPLER_ANISOTROPY: u32 = 1 << 3;
/// Device supports texture compression (BC)
pub const VK_FEATURE_TEXTURE_COMPRESSION_BC: u32 = 1 << 4;
/// Device supports 64-bit atomics
pub const VK_FEATURE_SHADER_INT64: u32 = 1 << 5;
/// Device supports 16-bit floats
pub const VK_FEATURE_SHADER_FLOAT16: u32 = 1 << 6;
/// Device supports timeline semaphores
pub const VK_FEATURE_TIMELINE_SEMAPHORE: u32 = 1 << 7;
/// Device supports buffer device address
pub const VK_FEATURE_BUFFER_DEVICE_ADDRESS: u32 = 1 << 8;
/// Device supports descriptor indexing
pub const VK_FEATURE_DESCRIPTOR_INDEXING: u32 = 1 << 9;

// ============================================================================
// Create Info
// ============================================================================

/// Device creation parameters
#[derive(Debug, Clone)]
pub struct VkDeviceCreateInfo {
    /// Physical device index
    pub physical_device: u32,
    /// Queue flags to request
    pub queue_flags: VkQueueFlags,
    /// Features to enable
    pub enabled_features: u32,
}

impl Default for VkDeviceCreateInfo {
    fn default() -> Self {
        Self {
            physical_device: 0,
            queue_flags: VkQueueFlags::GRAPHICS | VkQueueFlags::COMPUTE | VkQueueFlags::TRANSFER,
            enabled_features: VK_FEATURE_SAMPLER_ANISOTROPY
                | VK_FEATURE_TIMELINE_SEMAPHORE
                | VK_FEATURE_BUFFER_DEVICE_ADDRESS,
        }
    }
}

// ============================================================================
// VkDeviceCapsule
// ============================================================================

/// Mock Vulkan Logical Device Capsule
///
/// Manages device resources including queues, memory, and resource tracking.
///
/// # Tier: T1+T7 (Atomic + Heterogeneous)
/// # Size: 512B cache-aligned
///
/// # ASSUM Safety
///
/// - `#ASSUME_MOCK_DEVICE`: Mock implementation, no real Vulkan calls
/// - `#ASSUME_STATE_ATOMIC`: All state transitions use CAS
/// - `#ASSUME_RESOURCE_TRACKING`: Resource counts maintained atomically
#[repr(C, align(512))]
pub struct VkDeviceCapsule {
    /// Mock VkDevice handle
    handle: AtomicU64,

    /// Primary coordination: state(8) | queue_count(8) | generation(48)
    primary: AtomicU64,

    /// Secondary coordination: enabled_features(32) | limits_hash(32)
    secondary: AtomicU64,

    /// Graphics queue handle
    graphics_queue: AtomicU64,

    /// Compute queue handle
    compute_queue: AtomicU64,

    /// Transfer queue handle
    transfer_queue: AtomicU64,

    /// Buffer count
    buffer_count: AtomicU32,

    /// Image count
    image_count: AtomicU32,

    /// Sampler count
    sampler_count: AtomicU32,

    /// Descriptor set count
    descriptor_set_count: AtomicU32,

    /// Pipeline count
    pipeline_count: AtomicU32,

    /// Padding
    _padding: AtomicU32,

    /// Device-local memory allocated (bytes)
    device_local_allocated: AtomicU64,

    /// Host-visible memory allocated (bytes)
    host_visible_allocated: AtomicU64,

    /// Reserved space
    _reserved: [u8; 424],
}

// Compile-time verification
const _: () = {
    assert!(core::mem::size_of::<VkDeviceCapsule>() == 512);
    assert!(core::mem::align_of::<VkDeviceCapsule>() == 512);
};

impl VkDeviceCapsule {
    /// Create a new device capsule in uninitialized state
    pub const fn new() -> Self {
        Self {
            handle: AtomicU64::new(0),
            primary: AtomicU64::new(0),
            secondary: AtomicU64::new(0),
            graphics_queue: AtomicU64::new(0),
            compute_queue: AtomicU64::new(0),
            transfer_queue: AtomicU64::new(0),
            buffer_count: AtomicU32::new(0),
            image_count: AtomicU32::new(0),
            sampler_count: AtomicU32::new(0),
            descriptor_set_count: AtomicU32::new(0),
            pipeline_count: AtomicU32::new(0),
            _padding: AtomicU32::new(0),
            device_local_allocated: AtomicU64::new(0),
            host_visible_allocated: AtomicU64::new(0),
            _reserved: [0; 424],
        }
    }

    /// Create and initialize a device
    ///
    /// # Arguments
    ///
    /// * `info` - Device creation parameters
    ///
    /// # Returns
    ///
    /// `VkResult::Success` on success
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_STATE_TRANSITION`: Transitions from Uninitialized to Active
    /// - `#ASSUME_QUEUE_CREATION`: Mock queues are created
    pub fn create(&self, info: &VkDeviceCreateInfo) -> VkResult {
        let current = self.primary.load(Ordering::Acquire);
        let state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

        if state != VK_DEVICE_STATE_UNINITIALIZED {
            return VkResult::ErrorInitializationFailed;
        }

        // Transition to Creating
        let gen = current & GENERATION_MASK;
        let new_gen = gen.wrapping_add(1) & GENERATION_MASK;
        let creating = ((VK_DEVICE_STATE_CREATING as u64) << STATE_SHIFT) | new_gen;

        if self
            .primary
            .compare_exchange(current, creating, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return VkResult::ErrorInitializationFailed;
        }

        // Generate mock handle
        let handle = generate_mock_handle();
        self.handle.store(handle, Ordering::Release);

        // Create queues based on requested flags
        let mut queue_count = 0u8;

        if info.queue_flags.contains(VkQueueFlags::GRAPHICS) {
            let queue = generate_mock_handle();
            self.graphics_queue.store(queue, Ordering::Release);
            queue_count += 1;
        }

        if info.queue_flags.contains(VkQueueFlags::COMPUTE) {
            let queue = generate_mock_handle();
            self.compute_queue.store(queue, Ordering::Release);
            queue_count += 1;
        }

        if info.queue_flags.contains(VkQueueFlags::TRANSFER) {
            let queue = generate_mock_handle();
            self.transfer_queue.store(queue, Ordering::Release);
            queue_count += 1;
        }

        // Store enabled features
        let limits_hash = 0x12345678u32; // Mock hash
        let secondary = ((info.enabled_features as u64) << FEATURES_SHIFT) | (limits_hash as u64);
        self.secondary.store(secondary, Ordering::Release);

        // Transition to Active
        let active_gen = new_gen.wrapping_add(1) & GENERATION_MASK;
        let active = ((VK_DEVICE_STATE_ACTIVE as u64) << STATE_SHIFT)
            | ((queue_count as u64) << QUEUE_COUNT_SHIFT)
            | active_gen;

        self.primary.store(active, Ordering::Release);

        VkResult::Success
    }

    /// Wait for device idle
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_MOCK_IDLE`: Mock immediately returns success
    pub fn wait_idle(&self) -> VkResult {
        if self.state() != VK_DEVICE_STATE_ACTIVE {
            return VkResult::ErrorDeviceLost;
        }
        VkResult::Success
    }

    /// Destroy the device
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_STATE_TRANSITION`: Transitions to Destroyed
    /// - `#ASSUME_RESOURCE_CLEANUP`: All resources cleared (mock)
    pub fn destroy(&self) -> VkResult {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

            if state == VK_DEVICE_STATE_DESTROYED {
                return VkResult::Success;
            }

            if state != VK_DEVICE_STATE_ACTIVE && state != VK_DEVICE_STATE_IDLE {
                return VkResult::ErrorDeviceLost;
            }

            // Transition to Destroying
            let gen = current & GENERATION_MASK;
            let new_gen = gen.wrapping_add(1) & GENERATION_MASK;
            let destroying = ((VK_DEVICE_STATE_DESTROYING as u64) << STATE_SHIFT) | new_gen;

            if self
                .primary
                .compare_exchange(current, destroying, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                continue;
            }

            // Clear resources
            self.graphics_queue.store(0, Ordering::Release);
            self.compute_queue.store(0, Ordering::Release);
            self.transfer_queue.store(0, Ordering::Release);
            self.buffer_count.store(0, Ordering::Release);
            self.image_count.store(0, Ordering::Release);
            self.sampler_count.store(0, Ordering::Release);
            self.descriptor_set_count.store(0, Ordering::Release);
            self.pipeline_count.store(0, Ordering::Release);
            self.device_local_allocated.store(0, Ordering::Release);
            self.host_visible_allocated.store(0, Ordering::Release);
            self.handle.store(0, Ordering::Release);

            // Transition to Destroyed
            let destroyed_gen = new_gen.wrapping_add(1) & GENERATION_MASK;
            let destroyed = ((VK_DEVICE_STATE_DESTROYED as u64) << STATE_SHIFT) | destroyed_gen;
            self.primary.store(destroyed, Ordering::Release);

            return VkResult::Success;
        }
    }

    // ========================================================================
    // Resource Tracking
    // ========================================================================

    /// Increment buffer count and return new value
    #[inline]
    pub fn increment_buffer_count(&self) -> u32 {
        self.buffer_count.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Decrement buffer count and return new value
    #[inline]
    pub fn decrement_buffer_count(&self) -> u32 {
        self.buffer_count.fetch_sub(1, Ordering::Relaxed) - 1
    }

    /// Increment image count
    #[inline]
    pub fn increment_image_count(&self) -> u32 {
        self.image_count.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Decrement image count
    #[inline]
    pub fn decrement_image_count(&self) -> u32 {
        self.image_count.fetch_sub(1, Ordering::Relaxed) - 1
    }

    /// Increment sampler count
    #[inline]
    pub fn increment_sampler_count(&self) -> u32 {
        self.sampler_count.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Increment descriptor set count
    #[inline]
    pub fn increment_descriptor_set_count(&self) -> u32 {
        self.descriptor_set_count.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Increment pipeline count
    #[inline]
    pub fn increment_pipeline_count(&self) -> u32 {
        self.pipeline_count.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Track device-local memory allocation
    #[inline]
    pub fn track_device_local_alloc(&self, size: u64) -> u64 {
        self.device_local_allocated.fetch_add(size, Ordering::Relaxed) + size
    }

    /// Track device-local memory free
    #[inline]
    pub fn track_device_local_free(&self, size: u64) -> u64 {
        self.device_local_allocated.fetch_sub(size, Ordering::Relaxed) - size
    }

    /// Track host-visible memory allocation
    #[inline]
    pub fn track_host_visible_alloc(&self, size: u64) -> u64 {
        self.host_visible_allocated.fetch_add(size, Ordering::Relaxed) + size
    }

    /// Track host-visible memory free
    #[inline]
    pub fn track_host_visible_free(&self, size: u64) -> u64 {
        self.host_visible_allocated.fetch_sub(size, Ordering::Relaxed) - size
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

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        let primary = self.primary.load(Ordering::Acquire);
        primary & GENERATION_MASK
    }

    /// Get queue count
    #[inline]
    pub fn queue_count(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & QUEUE_COUNT_MASK) >> QUEUE_COUNT_SHIFT) as u8
    }

    /// Get enabled features
    #[inline]
    pub fn enabled_features(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & FEATURES_MASK) >> FEATURES_SHIFT) as u32
    }

    /// Check if feature is enabled
    #[inline]
    pub fn has_feature(&self, feature: u32) -> bool {
        (self.enabled_features() & feature) == feature
    }

    /// Get graphics queue handle
    #[inline]
    pub fn graphics_queue(&self) -> u64 {
        self.graphics_queue.load(Ordering::Acquire)
    }

    /// Get compute queue handle
    #[inline]
    pub fn compute_queue(&self) -> u64 {
        self.compute_queue.load(Ordering::Acquire)
    }

    /// Get transfer queue handle
    #[inline]
    pub fn transfer_queue(&self) -> u64 {
        self.transfer_queue.load(Ordering::Acquire)
    }

    /// Get buffer count
    #[inline]
    pub fn buffer_count(&self) -> u32 {
        self.buffer_count.load(Ordering::Acquire)
    }

    /// Get image count
    #[inline]
    pub fn image_count(&self) -> u32 {
        self.image_count.load(Ordering::Acquire)
    }

    /// Get sampler count
    #[inline]
    pub fn sampler_count(&self) -> u32 {
        self.sampler_count.load(Ordering::Acquire)
    }

    /// Get descriptor set count
    #[inline]
    pub fn descriptor_set_count(&self) -> u32 {
        self.descriptor_set_count.load(Ordering::Acquire)
    }

    /// Get pipeline count
    #[inline]
    pub fn pipeline_count(&self) -> u32 {
        self.pipeline_count.load(Ordering::Acquire)
    }

    /// Get device-local allocated memory
    #[inline]
    pub fn device_local_allocated(&self) -> u64 {
        self.device_local_allocated.load(Ordering::Acquire)
    }

    /// Get host-visible allocated memory
    #[inline]
    pub fn host_visible_allocated(&self) -> u64 {
        self.host_visible_allocated.load(Ordering::Acquire)
    }

    /// Check if device is active
    #[inline]
    pub fn is_active(&self) -> bool {
        self.state() == VK_DEVICE_STATE_ACTIVE
    }
}

impl Default for VkDeviceCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: All operations are atomic
unsafe impl Send for VkDeviceCapsule {}
unsafe impl Sync for VkDeviceCapsule {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<VkDeviceCapsule>(), 512);
        assert_eq!(core::mem::align_of::<VkDeviceCapsule>(), 512);
    }

    #[test]
    fn test_initial_state() {
        let device = VkDeviceCapsule::new();
        assert_eq!(device.state(), VK_DEVICE_STATE_UNINITIALIZED);
        assert_eq!(device.handle(), 0);
        assert_eq!(device.queue_count(), 0);
    }

    #[test]
    fn test_create_basic() {
        let device = VkDeviceCapsule::new();
        let info = VkDeviceCreateInfo::default();

        let result = device.create(&info);
        assert!(result.is_success());
        assert_eq!(device.state(), VK_DEVICE_STATE_ACTIVE);
        assert!(device.handle() > 0);
        assert_eq!(device.queue_count(), 3); // Graphics + Compute + Transfer
    }

    #[test]
    fn test_create_with_queues() {
        let device = VkDeviceCapsule::new();
        let info = VkDeviceCreateInfo {
            queue_flags: VkQueueFlags::GRAPHICS | VkQueueFlags::COMPUTE,
            ..Default::default()
        };

        device.create(&info);
        assert!(device.graphics_queue() > 0);
        assert!(device.compute_queue() > 0);
        assert_eq!(device.transfer_queue(), 0); // Not requested
        assert_eq!(device.queue_count(), 2);
    }

    #[test]
    fn test_create_with_features() {
        let device = VkDeviceCapsule::new();
        let info = VkDeviceCreateInfo {
            enabled_features: VK_FEATURE_GEOMETRY_SHADER | VK_FEATURE_TESSELLATION_SHADER,
            ..Default::default()
        };

        device.create(&info);
        assert!(device.has_feature(VK_FEATURE_GEOMETRY_SHADER));
        assert!(device.has_feature(VK_FEATURE_TESSELLATION_SHADER));
        assert!(!device.has_feature(VK_FEATURE_MULTI_VIEWPORT));
    }

    #[test]
    fn test_resource_tracking() {
        let device = VkDeviceCapsule::new();
        device.create(&VkDeviceCreateInfo::default());

        // Track buffers
        assert_eq!(device.increment_buffer_count(), 1);
        assert_eq!(device.increment_buffer_count(), 2);
        assert_eq!(device.buffer_count(), 2);
        assert_eq!(device.decrement_buffer_count(), 1);
        assert_eq!(device.buffer_count(), 1);

        // Track images
        assert_eq!(device.increment_image_count(), 1);
        assert_eq!(device.image_count(), 1);

        // Track memory
        device.track_device_local_alloc(1024);
        device.track_device_local_alloc(2048);
        assert_eq!(device.device_local_allocated(), 3072);

        device.track_host_visible_alloc(512);
        assert_eq!(device.host_visible_allocated(), 512);
    }

    #[test]
    fn test_wait_idle() {
        let device = VkDeviceCapsule::new();
        device.create(&VkDeviceCreateInfo::default());

        let result = device.wait_idle();
        assert!(result.is_success());
    }

    #[test]
    fn test_wait_idle_before_create() {
        let device = VkDeviceCapsule::new();
        let result = device.wait_idle();
        assert!(result.is_error());
    }

    #[test]
    fn test_destroy() {
        let device = VkDeviceCapsule::new();
        device.create(&VkDeviceCreateInfo::default());
        device.increment_buffer_count();
        device.track_device_local_alloc(1024);

        let result = device.destroy();
        assert!(result.is_success());
        assert_eq!(device.state(), VK_DEVICE_STATE_DESTROYED);
        assert_eq!(device.handle(), 0);
        assert_eq!(device.buffer_count(), 0);
        assert_eq!(device.device_local_allocated(), 0);
    }

    #[test]
    fn test_double_destroy() {
        let device = VkDeviceCapsule::new();
        device.create(&VkDeviceCreateInfo::default());
        device.destroy();

        let result = device.destroy();
        assert!(result.is_success()); // Idempotent
    }

    #[test]
    fn test_generation_increments() {
        let device = VkDeviceCapsule::new();
        let gen0 = device.generation();

        device.create(&VkDeviceCreateInfo::default());
        let gen1 = device.generation();
        assert!(gen1 > gen0);

        device.destroy();
        let gen2 = device.generation();
        assert!(gen2 > gen1);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_resource_tracking() {
        use std::sync::Arc;
        use std::thread;

        let device = Arc::new(VkDeviceCapsule::new());
        device.create(&VkDeviceCreateInfo::default());

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let dev = Arc::clone(&device);
                thread::spawn(move || {
                    for _ in 0..100 {
                        dev.increment_buffer_count();
                        dev.track_device_local_alloc(1024);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(device.buffer_count(), 400);
        assert_eq!(device.device_local_allocated(), 400 * 1024);
    }
}
