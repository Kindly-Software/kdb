//! VkInstanceCapsule - Vulkan Instance Management (Mock)
//!
//! **Tier**: T1 Atomic (<100ns operations)
//! **Size**: 256B cache-aligned
//! **Purpose**: Mock Vulkan instance for design validation
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_MOCK_INSTANCE`: This is a mock, not real Vulkan FFI
//! - `#ASSUME_HANDLE_VALID`: Mock handles are always "valid" (non-zero)
//! - `#ASSUME_STATE_ATOMIC`: All state changes use atomic operations
//!
//! # Memory Layout (256B)
//!
//! ```text
//! Offset  Size    Field
//! 0       8       handle (mock VkInstance)
//! 8       8       primary: state(8) | layer_count(8) | extension_count(8) | generation(40)
//! 16      8       secondary: flags(32) | api_version(32)
//! 24      1       enabled_validation (AtomicBool)
//! 25      1       enabled_debug_utils (AtomicBool)
//! 26      2       padding
//! 28      4       physical_device_count
//! 32      64      physical_devices (8 x 8B)
//! 96      160     reserved
//! ```

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use super::types::{
    generate_mock_handle, VkPhysicalDeviceType, VkQueueFlags, VkResult, VK_API_VERSION_1_3,
};

// ============================================================================
// State Constants
// ============================================================================

/// Instance is not initialized
pub const VK_INSTANCE_STATE_UNINITIALIZED: u8 = 0;
/// Instance is being created
pub const VK_INSTANCE_STATE_CREATING: u8 = 1;
/// Instance is active
pub const VK_INSTANCE_STATE_ACTIVE: u8 = 2;
/// Instance is being destroyed
pub const VK_INSTANCE_STATE_DESTROYING: u8 = 3;
/// Instance has been destroyed
pub const VK_INSTANCE_STATE_DESTROYED: u8 = 4;

/// Vulkan instance state enum (for type-safe API)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VkInstanceState {
    /// Instance is not initialized
    Uninitialized = VK_INSTANCE_STATE_UNINITIALIZED,
    /// Instance is being created
    Creating = VK_INSTANCE_STATE_CREATING,
    /// Instance is active and ready for use
    Active = VK_INSTANCE_STATE_ACTIVE,
    /// Instance is being destroyed
    Destroying = VK_INSTANCE_STATE_DESTROYING,
    /// Instance has been destroyed
    Destroyed = VK_INSTANCE_STATE_DESTROYED,
}

impl From<u8> for VkInstanceState {
    fn from(val: u8) -> Self {
        match val {
            VK_INSTANCE_STATE_UNINITIALIZED => VkInstanceState::Uninitialized,
            VK_INSTANCE_STATE_CREATING => VkInstanceState::Creating,
            VK_INSTANCE_STATE_ACTIVE => VkInstanceState::Active,
            VK_INSTANCE_STATE_DESTROYING => VkInstanceState::Destroying,
            VK_INSTANCE_STATE_DESTROYED => VkInstanceState::Destroyed,
            _ => VkInstanceState::Uninitialized,
        }
    }
}

// ============================================================================
// Bit Field Layouts
// ============================================================================

const STATE_SHIFT: u64 = 56;
const STATE_MASK: u64 = 0xFF << STATE_SHIFT;
const LAYER_COUNT_SHIFT: u64 = 48;
const LAYER_COUNT_MASK: u64 = 0xFF << LAYER_COUNT_SHIFT;
const EXT_COUNT_SHIFT: u64 = 40;
const EXT_COUNT_MASK: u64 = 0xFF << EXT_COUNT_SHIFT;
const GENERATION_MASK: u64 = 0x00_00_FF_FF_FF_FF_FF_FF;

const FLAGS_SHIFT: u64 = 32;
const FLAGS_MASK: u64 = 0xFFFF_FFFF << FLAGS_SHIFT;
const API_VERSION_MASK: u64 = 0x0000_0000_FFFF_FFFF;

// ============================================================================
// Physical Device Info
// ============================================================================

/// Information about a physical device (GPU)
#[derive(Debug, Clone, Copy)]
pub struct VkPhysicalDeviceInfo {
    /// Mock device handle
    pub handle: u64,
    /// Device type
    pub device_type: VkPhysicalDeviceType,
    /// Queue family capabilities
    pub queue_flags: VkQueueFlags,
    /// API version supported
    pub api_version: u32,
    /// Device memory size in bytes
    pub memory_size: u64,
    /// Device name (abbreviated)
    pub name: [u8; 32],
}

impl Default for VkPhysicalDeviceInfo {
    fn default() -> Self {
        Self {
            handle: 0,
            device_type: VkPhysicalDeviceType::Other,
            queue_flags: VkQueueFlags::empty(),
            api_version: VK_API_VERSION_1_3,
            memory_size: 0,
            name: [0u8; 32],
        }
    }
}

// ============================================================================
// Create Info
// ============================================================================

/// Instance creation parameters
#[derive(Debug, Clone)]
pub struct VkInstanceCreateInfo {
    /// Application name
    pub app_name: Option<&'static str>,
    /// Application version
    pub app_version: u32,
    /// Engine name
    pub engine_name: Option<&'static str>,
    /// Engine version
    pub engine_version: u32,
    /// Requested API version
    pub api_version: u32,
    /// Enable validation layers
    pub enable_validation: bool,
    /// Enable debug utils extension
    pub enable_debug_utils: bool,
}

impl Default for VkInstanceCreateInfo {
    fn default() -> Self {
        Self {
            app_name: None,
            app_version: 1,
            engine_name: Some("KGPU"),
            engine_version: 1,
            api_version: VK_API_VERSION_1_3,
            enable_validation: false,
            enable_debug_utils: false,
        }
    }
}

// ============================================================================
// VkInstanceCapsule
// ============================================================================

/// Mock Vulkan Instance Capsule
///
/// Manages Vulkan instance lifecycle and physical device enumeration.
///
/// # Tier: T1 Atomic
/// # Size: 256B cache-aligned
///
/// # ASSUM Safety
///
/// - `#ASSUME_MOCK_INSTANCE`: Mock implementation, no real Vulkan calls
/// - `#ASSUME_STATE_ATOMIC`: All state transitions use CAS
/// - `#ASSUME_HANDLE_NONZERO`: Valid handles are always > 0
#[repr(C, align(256))]
pub struct VkInstanceCapsule {
    /// Mock VkInstance handle
    handle: AtomicU64,

    /// Primary coordination: state(8) | layer_count(8) | ext_count(8) | generation(40)
    primary: AtomicU64,

    /// Secondary coordination: flags(32) | api_version(32)
    secondary: AtomicU64,

    /// Validation layers enabled
    enabled_validation: AtomicBool,

    /// Debug utils enabled
    enabled_debug_utils: AtomicBool,

    /// Padding for alignment
    _padding1: [u8; 2],

    /// Number of physical devices
    physical_device_count: AtomicU32,

    /// Physical device handles (mock)
    physical_devices: [AtomicU64; 8],

    /// Reserved space for future use
    _reserved: [u8; 152],
}

// Compile-time verification
const _: () = {
    assert!(core::mem::size_of::<VkInstanceCapsule>() == 256);
    assert!(core::mem::align_of::<VkInstanceCapsule>() == 256);
};

impl VkInstanceCapsule {
    /// Create a new instance capsule in uninitialized state
    pub const fn new() -> Self {
        Self {
            handle: AtomicU64::new(0),
            primary: AtomicU64::new(0),
            secondary: AtomicU64::new(0),
            enabled_validation: AtomicBool::new(false),
            enabled_debug_utils: AtomicBool::new(false),
            _padding1: [0; 2],
            physical_device_count: AtomicU32::new(0),
            physical_devices: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            _reserved: [0; 152],
        }
    }

    /// Create and initialize an instance
    ///
    /// # Arguments
    ///
    /// * `info` - Instance creation parameters
    ///
    /// # Returns
    ///
    /// `VkResult::Success` on success, error code on failure
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_STATE_TRANSITION`: Transitions from Uninitialized to Active
    /// - `#ASSUME_MOCK_ENUMERATION`: Physical devices are simulated
    pub fn create(&self, info: &VkInstanceCreateInfo) -> VkResult {
        // Check current state
        let current = self.primary.load(Ordering::Acquire);
        let state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

        if state != VK_INSTANCE_STATE_UNINITIALIZED {
            return VkResult::ErrorInitializationFailed;
        }

        // Transition to Creating state
        let gen = current & GENERATION_MASK;
        let new_gen = gen.wrapping_add(1) & GENERATION_MASK;
        let creating = ((VK_INSTANCE_STATE_CREATING as u64) << STATE_SHIFT) | new_gen;

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

        // Store configuration
        self.enabled_validation
            .store(info.enable_validation, Ordering::Release);
        self.enabled_debug_utils
            .store(info.enable_debug_utils, Ordering::Release);

        // Set secondary fields
        let layer_count: u8 = if info.enable_validation { 1 } else { 0 };
        let ext_count: u8 = if info.enable_debug_utils { 1 } else { 0 };

        let secondary = ((0u64) << FLAGS_SHIFT) | (info.api_version as u64);
        self.secondary.store(secondary, Ordering::Release);

        // Enumerate physical devices (mock)
        self.enumerate_physical_devices_internal();

        // Transition to Active
        let active_gen = new_gen.wrapping_add(1) & GENERATION_MASK;
        let active = ((VK_INSTANCE_STATE_ACTIVE as u64) << STATE_SHIFT)
            | ((layer_count as u64) << LAYER_COUNT_SHIFT)
            | ((ext_count as u64) << EXT_COUNT_SHIFT)
            | active_gen;

        self.primary.store(active, Ordering::Release);

        VkResult::Success
    }

    /// Enumerate physical devices (mock implementation)
    ///
    /// Returns information about simulated GPUs.
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_MOCK_DEVICES`: Returns simulated device info
    pub fn enumerate_physical_devices(&self) -> Result<Vec<VkPhysicalDeviceInfo>, VkResult> {
        if self.state() != VK_INSTANCE_STATE_ACTIVE {
            return Err(VkResult::ErrorInitializationFailed);
        }

        let count = self.physical_device_count.load(Ordering::Acquire);
        let mut devices = Vec::with_capacity(count as usize);

        for i in 0..count as usize {
            let handle = self.physical_devices[i].load(Ordering::Acquire);
            if handle != 0 {
                devices.push(self.get_physical_device_info(i));
            }
        }

        Ok(devices)
    }

    /// Get information about a specific physical device
    fn get_physical_device_info(&self, index: usize) -> VkPhysicalDeviceInfo {
        let handle = self.physical_devices[index].load(Ordering::Acquire);

        // Simulate different device types based on index
        let (device_type, memory_size, name_str) = match index {
            0 => (
                VkPhysicalDeviceType::DiscreteGpu,
                8 * 1024 * 1024 * 1024u64, // 8 GB
                "Mock Discrete GPU",
            ),
            1 => (
                VkPhysicalDeviceType::IntegratedGpu,
                2 * 1024 * 1024 * 1024u64, // 2 GB
                "Mock Integrated GPU",
            ),
            _ => (
                VkPhysicalDeviceType::Other,
                1024 * 1024 * 1024u64, // 1 GB
                "Mock Device",
            ),
        };

        let mut name = [0u8; 32];
        let bytes = name_str.as_bytes();
        let len = bytes.len().min(31);
        name[..len].copy_from_slice(&bytes[..len]);

        VkPhysicalDeviceInfo {
            handle,
            device_type,
            queue_flags: VkQueueFlags::GRAPHICS | VkQueueFlags::COMPUTE | VkQueueFlags::TRANSFER,
            api_version: VK_API_VERSION_1_3,
            memory_size,
            name,
        }
    }

    /// Internal: Enumerate physical devices during creation
    fn enumerate_physical_devices_internal(&self) {
        // Mock: Create 2 simulated devices
        let device1 = generate_mock_handle();
        let device2 = generate_mock_handle();

        self.physical_devices[0].store(device1, Ordering::Release);
        self.physical_devices[1].store(device2, Ordering::Release);
        self.physical_device_count.store(2, Ordering::Release);
    }

    /// Destroy the instance
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_STATE_TRANSITION`: Transitions to Destroyed
    /// - `#ASSUME_CLEANUP_COMPLETE`: All resources released (mock)
    pub fn destroy(&self) -> VkResult {
        loop {
            let current = self.primary.load(Ordering::Acquire);
            let state = ((current & STATE_MASK) >> STATE_SHIFT) as u8;

            if state == VK_INSTANCE_STATE_DESTROYED {
                return VkResult::Success; // Already destroyed
            }

            if state != VK_INSTANCE_STATE_ACTIVE {
                return VkResult::ErrorInitializationFailed;
            }

            // Transition to Destroying
            let gen = current & GENERATION_MASK;
            let new_gen = gen.wrapping_add(1) & GENERATION_MASK;
            let destroying = ((VK_INSTANCE_STATE_DESTROYING as u64) << STATE_SHIFT) | new_gen;

            if self
                .primary
                .compare_exchange(current, destroying, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                continue; // Retry
            }

            // Clear physical devices
            for pd in &self.physical_devices {
                pd.store(0, Ordering::Release);
            }
            self.physical_device_count.store(0, Ordering::Release);

            // Clear handle
            self.handle.store(0, Ordering::Release);

            // Transition to Destroyed
            let destroyed_gen = new_gen.wrapping_add(1) & GENERATION_MASK;
            let destroyed = ((VK_INSTANCE_STATE_DESTROYED as u64) << STATE_SHIFT) | destroyed_gen;
            self.primary.store(destroyed, Ordering::Release);

            return VkResult::Success;
        }
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

    /// Get mock handle value
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

    /// Check if validation is enabled
    #[inline]
    pub fn is_validation_enabled(&self) -> bool {
        self.enabled_validation.load(Ordering::Acquire)
    }

    /// Check if debug utils is enabled
    #[inline]
    pub fn is_debug_utils_enabled(&self) -> bool {
        self.enabled_debug_utils.load(Ordering::Acquire)
    }

    /// Get API version
    #[inline]
    pub fn api_version(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Acquire);
        (secondary & API_VERSION_MASK) as u32
    }

    /// Get physical device count
    #[inline]
    pub fn physical_device_count(&self) -> u32 {
        self.physical_device_count.load(Ordering::Acquire)
    }

    /// Get layer count
    #[inline]
    pub fn layer_count(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & LAYER_COUNT_MASK) >> LAYER_COUNT_SHIFT) as u8
    }

    /// Get extension count
    #[inline]
    pub fn extension_count(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & EXT_COUNT_MASK) >> EXT_COUNT_SHIFT) as u8
    }

    /// Check if instance is active
    #[inline]
    pub fn is_active(&self) -> bool {
        self.state() == VK_INSTANCE_STATE_ACTIVE
    }
}

impl Default for VkInstanceCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: All operations are atomic
unsafe impl Send for VkInstanceCapsule {}
unsafe impl Sync for VkInstanceCapsule {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<VkInstanceCapsule>(), 256);
        assert_eq!(core::mem::align_of::<VkInstanceCapsule>(), 256);
    }

    #[test]
    fn test_initial_state() {
        let instance = VkInstanceCapsule::new();
        assert_eq!(instance.state(), VK_INSTANCE_STATE_UNINITIALIZED);
        assert_eq!(instance.handle(), 0);
        assert_eq!(instance.physical_device_count(), 0);
    }

    #[test]
    fn test_create_basic() {
        let instance = VkInstanceCapsule::new();
        let info = VkInstanceCreateInfo::default();

        let result = instance.create(&info);
        assert!(result.is_success());
        assert_eq!(instance.state(), VK_INSTANCE_STATE_ACTIVE);
        assert!(instance.handle() > 0);
    }

    #[test]
    fn test_create_with_validation() {
        let instance = VkInstanceCapsule::new();
        let info = VkInstanceCreateInfo {
            enable_validation: true,
            enable_debug_utils: true,
            ..Default::default()
        };

        let result = instance.create(&info);
        assert!(result.is_success());
        assert!(instance.is_validation_enabled());
        assert!(instance.is_debug_utils_enabled());
        assert_eq!(instance.layer_count(), 1);
        assert_eq!(instance.extension_count(), 1);
    }

    #[test]
    fn test_enumerate_physical_devices() {
        let instance = VkInstanceCapsule::new();
        instance.create(&VkInstanceCreateInfo::default());

        let devices = instance.enumerate_physical_devices().unwrap();
        assert_eq!(devices.len(), 2);

        // First device should be discrete
        assert_eq!(devices[0].device_type, VkPhysicalDeviceType::DiscreteGpu);
        assert!(devices[0].queue_flags.contains(VkQueueFlags::GRAPHICS));

        // Second device should be integrated
        assert_eq!(devices[1].device_type, VkPhysicalDeviceType::IntegratedGpu);
    }

    #[test]
    fn test_enumerate_before_create_fails() {
        let instance = VkInstanceCapsule::new();
        let result = instance.enumerate_physical_devices();
        assert!(result.is_err());
    }

    #[test]
    fn test_destroy() {
        let instance = VkInstanceCapsule::new();
        instance.create(&VkInstanceCreateInfo::default());

        let result = instance.destroy();
        assert!(result.is_success());
        assert_eq!(instance.state(), VK_INSTANCE_STATE_DESTROYED);
        assert_eq!(instance.handle(), 0);
        assert_eq!(instance.physical_device_count(), 0);
    }

    #[test]
    fn test_double_destroy() {
        let instance = VkInstanceCapsule::new();
        instance.create(&VkInstanceCreateInfo::default());

        instance.destroy();
        let result = instance.destroy();
        assert!(result.is_success()); // Idempotent
    }

    #[test]
    fn test_double_create_fails() {
        let instance = VkInstanceCapsule::new();
        instance.create(&VkInstanceCreateInfo::default());

        let result = instance.create(&VkInstanceCreateInfo::default());
        assert!(result.is_error());
    }

    #[test]
    fn test_generation_increments() {
        let instance = VkInstanceCapsule::new();
        let gen0 = instance.generation();

        instance.create(&VkInstanceCreateInfo::default());
        let gen1 = instance.generation();
        assert!(gen1 > gen0);

        instance.destroy();
        let gen2 = instance.generation();
        assert!(gen2 > gen1);
    }

    #[test]
    fn test_api_version() {
        let instance = VkInstanceCapsule::new();
        let info = VkInstanceCreateInfo {
            api_version: VK_API_VERSION_1_3,
            ..Default::default()
        };

        instance.create(&info);
        assert_eq!(instance.api_version(), VK_API_VERSION_1_3);
    }

    #[test]
    fn test_is_active() {
        let instance = VkInstanceCapsule::new();
        assert!(!instance.is_active());

        instance.create(&VkInstanceCreateInfo::default());
        assert!(instance.is_active());

        instance.destroy();
        assert!(!instance.is_active());
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_concurrent_reads() {
        use std::sync::Arc;
        use std::thread;

        let instance = Arc::new(VkInstanceCapsule::new());
        instance.create(&VkInstanceCreateInfo::default());

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let inst = Arc::clone(&instance);
                thread::spawn(move || {
                    for _ in 0..1000 {
                        let _ = inst.state();
                        let _ = inst.handle();
                        let _ = inst.physical_device_count();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert!(instance.is_active());
    }
}
