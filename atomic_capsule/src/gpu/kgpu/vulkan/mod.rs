//! KGPU Phase 5: Vulkan Backend Capsule
//!
//! Mock/stub implementation for design validation.
//! Tier: T1+T7 (Atomic coordination + Heterogeneous GPU)
//!
//! # Chaos Compliance
//! - 100% lockfree (no mutex/RwLock)
//! - Cache-aligned capsules (128B, 256B, 512B)
//! - DualAtomicU64 pattern for coordination
//! - Generation counters for ABA prevention
//!
//! # Architecture
//! ```text
//! VkBackendCapsule (512B) - Main orchestrator
//!   |
//!   +-- VkInstanceCapsule (256B) - Vulkan instance management
//!   |     |
//!   |     +-- Physical device enumeration
//!   |
//!   +-- VkDeviceCapsule (512B) - Logical device management
//!   |     |
//!   |     +-- Queue management (graphics, compute, transfer)
//!   |     +-- Resource tracking
//!   |
//!   +-- VkBufferCapsule (128B) - Buffer resources
//!   |
//!   +-- VkImageCapsule (256B) - Image resources
//! ```

pub mod types;
pub mod instance;
pub mod device;
pub mod buffer;
pub mod image;

pub use types::*;
pub use instance::*;
pub use device::*;
pub use buffer::*;
pub use image::*;

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// =============================================================================
// VkBackendCapsule - Main Vulkan Backend Orchestrator
// =============================================================================

/// Backend states for VkBackendCapsule
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VkBackendState {
    /// Backend not initialized
    Uninitialized = 0,
    /// Backend initializing (loading Vulkan)
    Initializing = 1,
    /// Backend ready (Vulkan loaded, no instance)
    Ready = 2,
    /// Backend active (instance created)
    Active = 3,
    /// Backend suspended (temporarily paused)
    Suspended = 4,
    /// Backend shutting down
    ShuttingDown = 5,
    /// Backend terminated
    Terminated = 6,
    /// Backend in error state
    Error = 7,
}

impl VkBackendState {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Uninitialized,
            1 => Self::Initializing,
            2 => Self::Ready,
            3 => Self::Active,
            4 => Self::Suspended,
            5 => Self::ShuttingDown,
            6 => Self::Terminated,
            _ => Self::Error,
        }
    }
}

/// Backend capability flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VkBackendCapabilities(pub u32);

impl VkBackendCapabilities {
    pub const NONE: Self = Self(0);
    pub const GRAPHICS: Self = Self(1 << 0);
    pub const COMPUTE: Self = Self(1 << 1);
    pub const TRANSFER: Self = Self(1 << 2);
    pub const SPARSE_BINDING: Self = Self(1 << 3);
    pub const PROTECTED_MEMORY: Self = Self(1 << 4);
    pub const RAYTRACING: Self = Self(1 << 5);
    pub const MESH_SHADERS: Self = Self(1 << 6);
    pub const DYNAMIC_RENDERING: Self = Self(1 << 7);
    pub const SYNCHRONIZATION2: Self = Self(1 << 8);
    pub const TIMELINE_SEMAPHORES: Self = Self(1 << 9);
    pub const BUFFER_DEVICE_ADDRESS: Self = Self(1 << 10);
    pub const DESCRIPTOR_INDEXING: Self = Self(1 << 11);

    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

/// VkBackendCapsule - Main Vulkan backend orchestrator
///
/// # Chaos Compliance
/// - Tier: T1+T7 (Atomic + Heterogeneous)
/// - Alignment: 512B (cache-line friendly, avoids false sharing)
/// - Pattern: DualAtomicU64 for primary/secondary coordination
/// - ABA Prevention: 48-bit generation counter
///
/// # State Machine
/// ```text
/// UNINITIALIZED -> INITIALIZING -> READY -> ACTIVE
///                                    |         |
///                                    v         v
///                               SUSPENDED <-> ACTIVE
///                                    |
///                                    v
///                              SHUTTING_DOWN -> TERMINATED
/// ```
///
/// # ASSUM Safety Tags
/// #ASSUME_ATOMIC_ORDERING: All state transitions use Acquire/Release ordering
/// #ASSUME_GENERATION_MONOTONIC: Generation counter only increments
/// #ASSUME_HANDLE_VALIDITY: Instance handles validated before use
/// #ASSUME_QUEUE_FAMILY_VALID: Queue family indices validated at discovery
#[repr(C, align(512))]
pub struct VkBackendCapsule {
    /// Primary coordination: state(8) | instance_count(8) | generation(48)
    primary: AtomicU64,
    /// Secondary coordination: device_count(16) | queue_count(16) | capabilities(32)
    secondary: AtomicU64,
    /// Vulkan API version (VK_MAKE_API_VERSION)
    api_version: AtomicU32,
    /// Driver version
    driver_version: AtomicU32,
    /// Instance handles (up to 4 instances for multi-GPU scenarios)
    instance_handles: [AtomicU64; 4],
    /// Number of physical devices discovered
    physical_device_count: AtomicU32,
    /// Currently selected physical device index
    selected_device: AtomicU32,
    /// Graphics queue family index (u32::MAX if not available)
    graphics_queue_family: AtomicU32,
    /// Compute queue family index (u32::MAX if not available)
    compute_queue_family: AtomicU32,
    /// Transfer queue family index (u32::MAX if not available)
    transfer_queue_family: AtomicU32,
    /// Padding for alignment
    _pad0: u32,
    /// Total memory allocations made
    allocations: AtomicU64,
    /// Total command buffer submissions
    submissions: AtomicU64,
    /// Error count for diagnostics
    error_count: AtomicU64,
    /// Last error code
    last_error: AtomicU32,
    /// Reserved for future use
    _reserved: [u8; 396],
}

// Compile-time size and alignment verification
const _: () = {
    assert!(core::mem::size_of::<VkBackendCapsule>() == 512);
    assert!(core::mem::align_of::<VkBackendCapsule>() == 512);
};

impl VkBackendCapsule {
    // Primary field bit layout
    const STATE_SHIFT: u64 = 56;
    const STATE_MASK: u64 = 0xFF << 56;
    const INSTANCE_COUNT_SHIFT: u64 = 48;
    const INSTANCE_COUNT_MASK: u64 = 0xFF << 48;
    const GENERATION_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

    // Secondary field bit layout
    const DEVICE_COUNT_SHIFT: u64 = 48;
    const DEVICE_COUNT_MASK: u64 = 0xFFFF << 48;
    const QUEUE_COUNT_SHIFT: u64 = 32;
    const QUEUE_COUNT_MASK: u64 = 0xFFFF << 32;
    const CAPABILITIES_MASK: u64 = 0xFFFF_FFFF;

    /// Queue family index indicating not available
    pub const QUEUE_FAMILY_IGNORED: u32 = u32::MAX;

    /// Create new uninitialized VkBackendCapsule
    pub const fn new() -> Self {
        Self {
            primary: AtomicU64::new(0),
            secondary: AtomicU64::new(0),
            api_version: AtomicU32::new(0),
            driver_version: AtomicU32::new(0),
            instance_handles: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            physical_device_count: AtomicU32::new(0),
            selected_device: AtomicU32::new(0),
            graphics_queue_family: AtomicU32::new(Self::QUEUE_FAMILY_IGNORED),
            compute_queue_family: AtomicU32::new(Self::QUEUE_FAMILY_IGNORED),
            transfer_queue_family: AtomicU32::new(Self::QUEUE_FAMILY_IGNORED),
            _pad0: 0,
            allocations: AtomicU64::new(0),
            submissions: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            last_error: AtomicU32::new(0),
            _reserved: [0u8; 396],
        }
    }

    // =========================================================================
    // Primary Field Accessors
    // =========================================================================

    /// Get current backend state
    pub fn state(&self) -> VkBackendState {
        let primary = self.primary.load(Ordering::Acquire);
        VkBackendState::from_u8(((primary & Self::STATE_MASK) >> Self::STATE_SHIFT) as u8)
    }

    /// Get instance count
    pub fn instance_count(&self) -> u8 {
        let primary = self.primary.load(Ordering::Acquire);
        ((primary & Self::INSTANCE_COUNT_MASK) >> Self::INSTANCE_COUNT_SHIFT) as u8
    }

    /// Get generation counter (for ABA prevention)
    pub fn generation(&self) -> u64 {
        let primary = self.primary.load(Ordering::Acquire);
        primary & Self::GENERATION_MASK
    }

    // =========================================================================
    // Secondary Field Accessors
    // =========================================================================

    /// Get logical device count
    pub fn device_count(&self) -> u16 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & Self::DEVICE_COUNT_MASK) >> Self::DEVICE_COUNT_SHIFT) as u16
    }

    /// Get total queue count across all devices
    pub fn queue_count(&self) -> u16 {
        let secondary = self.secondary.load(Ordering::Acquire);
        ((secondary & Self::QUEUE_COUNT_MASK) >> Self::QUEUE_COUNT_SHIFT) as u16
    }

    /// Get backend capabilities
    pub fn capabilities(&self) -> VkBackendCapabilities {
        let secondary = self.secondary.load(Ordering::Acquire);
        VkBackendCapabilities((secondary & Self::CAPABILITIES_MASK) as u32)
    }

    // =========================================================================
    // State Transitions
    // =========================================================================

    /// Initialize the backend (UNINITIALIZED -> INITIALIZING -> READY)
    ///
    /// Mock implementation: simulates Vulkan library loading
    pub fn initialize(&self) -> VkResult {
        let current = self.primary.load(Ordering::Acquire);
        let state = VkBackendState::from_u8(((current & Self::STATE_MASK) >> Self::STATE_SHIFT) as u8);

        if state != VkBackendState::Uninitialized {
            return VkResult::ErrorInitializationFailed;
        }

        let generation = (current & Self::GENERATION_MASK) + 1;
        let new_primary = ((VkBackendState::Initializing as u64) << Self::STATE_SHIFT) | generation;

        match self.primary.compare_exchange(
            current,
            new_primary,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                // Mock: Set default API version (Vulkan 1.3)
                self.api_version.store(make_api_version(0, 1, 3, 0), Ordering::Release);

                // Transition to Ready
                let ready_primary = ((VkBackendState::Ready as u64) << Self::STATE_SHIFT) | (generation + 1);
                self.primary.store(ready_primary, Ordering::Release);

                // Set default capabilities
                let caps = VkBackendCapabilities::GRAPHICS.0
                    | VkBackendCapabilities::COMPUTE.0
                    | VkBackendCapabilities::TRANSFER.0;
                self.secondary.store(caps as u64, Ordering::Release);

                VkResult::Success
            }
            Err(_) => VkResult::ErrorInitializationFailed,
        }
    }

    /// Create instance (READY -> ACTIVE)
    ///
    /// Mock implementation: creates a simulated Vulkan instance
    pub fn create_instance(&self, app_name: &str, app_version: u32) -> Result<u64, VkResult> {
        let current = self.primary.load(Ordering::Acquire);
        let state = VkBackendState::from_u8(((current & Self::STATE_MASK) >> Self::STATE_SHIFT) as u8);
        let instance_count = ((current & Self::INSTANCE_COUNT_MASK) >> Self::INSTANCE_COUNT_SHIFT) as u8;

        if state != VkBackendState::Ready && state != VkBackendState::Active {
            return Err(VkResult::ErrorInitializationFailed);
        }

        if instance_count >= 4 {
            return Err(VkResult::ErrorOutOfDeviceMemory);
        }

        // Mock: Generate instance handle
        let handle = generate_mock_handle();

        // Store handle
        self.instance_handles[instance_count as usize].store(handle, Ordering::Release);

        // Update primary: increment instance count, set Active state
        let generation = (current & Self::GENERATION_MASK) + 1;
        let new_instance_count = instance_count + 1;
        let new_primary = ((VkBackendState::Active as u64) << Self::STATE_SHIFT)
            | ((new_instance_count as u64) << Self::INSTANCE_COUNT_SHIFT)
            | generation;

        match self.primary.compare_exchange(
            current,
            new_primary,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                // Mock: Enumerate physical devices (simulate 2 GPUs)
                self.physical_device_count.store(2, Ordering::Release);
                self.selected_device.store(0, Ordering::Release);

                // Set queue families (mock values)
                self.graphics_queue_family.store(0, Ordering::Release);
                self.compute_queue_family.store(1, Ordering::Release);
                self.transfer_queue_family.store(2, Ordering::Release);

                let _ = app_name; // Suppress unused warning in mock
                let _ = app_version;

                Ok(handle)
            }
            Err(_) => Err(VkResult::ErrorInitializationFailed),
        }
    }

    /// Suspend backend (ACTIVE -> SUSPENDED)
    pub fn suspend(&self) -> VkResult {
        let current = self.primary.load(Ordering::Acquire);
        let state = VkBackendState::from_u8(((current & Self::STATE_MASK) >> Self::STATE_SHIFT) as u8);

        if state != VkBackendState::Active {
            return VkResult::ErrorDeviceLost;
        }

        let generation = (current & Self::GENERATION_MASK) + 1;
        let instance_count = (current & Self::INSTANCE_COUNT_MASK) >> Self::INSTANCE_COUNT_SHIFT;
        let new_primary = ((VkBackendState::Suspended as u64) << Self::STATE_SHIFT)
            | (instance_count << Self::INSTANCE_COUNT_SHIFT)
            | generation;

        match self.primary.compare_exchange(
            current,
            new_primary,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => VkResult::Success,
            Err(_) => VkResult::ErrorDeviceLost,
        }
    }

    /// Resume backend (SUSPENDED -> ACTIVE)
    pub fn resume(&self) -> VkResult {
        let current = self.primary.load(Ordering::Acquire);
        let state = VkBackendState::from_u8(((current & Self::STATE_MASK) >> Self::STATE_SHIFT) as u8);

        if state != VkBackendState::Suspended {
            return VkResult::ErrorDeviceLost;
        }

        let generation = (current & Self::GENERATION_MASK) + 1;
        let instance_count = (current & Self::INSTANCE_COUNT_MASK) >> Self::INSTANCE_COUNT_SHIFT;
        let new_primary = ((VkBackendState::Active as u64) << Self::STATE_SHIFT)
            | (instance_count << Self::INSTANCE_COUNT_SHIFT)
            | generation;

        match self.primary.compare_exchange(
            current,
            new_primary,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => VkResult::Success,
            Err(_) => VkResult::ErrorDeviceLost,
        }
    }

    /// Shutdown backend (any -> SHUTTING_DOWN -> TERMINATED)
    pub fn shutdown(&self) -> VkResult {
        let current = self.primary.load(Ordering::Acquire);
        let state = VkBackendState::from_u8(((current & Self::STATE_MASK) >> Self::STATE_SHIFT) as u8);

        if state == VkBackendState::Terminated || state == VkBackendState::ShuttingDown {
            return VkResult::Success;
        }

        let generation = (current & Self::GENERATION_MASK) + 1;
        let new_primary = ((VkBackendState::ShuttingDown as u64) << Self::STATE_SHIFT) | generation;

        match self.primary.compare_exchange(
            current,
            new_primary,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                // Clear instance handles
                for handle in &self.instance_handles {
                    handle.store(0, Ordering::Release);
                }

                // Clear device info
                self.physical_device_count.store(0, Ordering::Release);
                self.selected_device.store(0, Ordering::Release);
                self.graphics_queue_family.store(Self::QUEUE_FAMILY_IGNORED, Ordering::Release);
                self.compute_queue_family.store(Self::QUEUE_FAMILY_IGNORED, Ordering::Release);
                self.transfer_queue_family.store(Self::QUEUE_FAMILY_IGNORED, Ordering::Release);

                // Transition to Terminated
                let terminated_primary = ((VkBackendState::Terminated as u64) << Self::STATE_SHIFT)
                    | (generation + 1);
                self.primary.store(terminated_primary, Ordering::Release);

                // Clear secondary
                self.secondary.store(0, Ordering::Release);

                VkResult::Success
            }
            Err(_) => VkResult::ErrorDeviceLost,
        }
    }

    // =========================================================================
    // Resource Management
    // =========================================================================

    /// Get API version
    pub fn api_version(&self) -> u32 {
        self.api_version.load(Ordering::Acquire)
    }

    /// Get driver version
    pub fn driver_version(&self) -> u32 {
        self.driver_version.load(Ordering::Acquire)
    }

    /// Get physical device count
    pub fn physical_device_count(&self) -> u32 {
        self.physical_device_count.load(Ordering::Acquire)
    }

    /// Get selected device index
    pub fn selected_device(&self) -> u32 {
        self.selected_device.load(Ordering::Acquire)
    }

    /// Select physical device
    pub fn select_device(&self, index: u32) -> VkResult {
        let count = self.physical_device_count.load(Ordering::Acquire);
        if index >= count {
            return VkResult::ErrorDeviceLost;
        }
        self.selected_device.store(index, Ordering::Release);
        VkResult::Success
    }

    /// Get graphics queue family index
    pub fn graphics_queue_family(&self) -> Option<u32> {
        let family = self.graphics_queue_family.load(Ordering::Acquire);
        if family == Self::QUEUE_FAMILY_IGNORED {
            None
        } else {
            Some(family)
        }
    }

    /// Get compute queue family index
    pub fn compute_queue_family(&self) -> Option<u32> {
        let family = self.compute_queue_family.load(Ordering::Acquire);
        if family == Self::QUEUE_FAMILY_IGNORED {
            None
        } else {
            Some(family)
        }
    }

    /// Get transfer queue family index
    pub fn transfer_queue_family(&self) -> Option<u32> {
        let family = self.transfer_queue_family.load(Ordering::Acquire);
        if family == Self::QUEUE_FAMILY_IGNORED {
            None
        } else {
            Some(family)
        }
    }

    /// Get instance handle by index
    pub fn instance_handle(&self, index: usize) -> Option<u64> {
        if index >= 4 {
            return None;
        }
        let handle = self.instance_handles[index].load(Ordering::Acquire);
        if handle == 0 {
            None
        } else {
            Some(handle)
        }
    }

    // =========================================================================
    // Statistics and Diagnostics
    // =========================================================================

    /// Record an allocation
    pub fn record_allocation(&self) {
        self.allocations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a command buffer submission
    pub fn record_submission(&self) {
        self.submissions.fetch_add(1, Ordering::Relaxed);
    }

    /// Get total allocation count
    pub fn allocation_count(&self) -> u64 {
        self.allocations.load(Ordering::Relaxed)
    }

    /// Get total submission count
    pub fn submission_count(&self) -> u64 {
        self.submissions.load(Ordering::Relaxed)
    }

    /// Record an error
    pub fn record_error(&self, error: VkResult) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
        self.last_error.store(error as u32, Ordering::Release);
    }

    /// Get error count
    pub fn error_count(&self) -> u64 {
        self.error_count.load(Ordering::Relaxed)
    }

    /// Get last error
    pub fn last_error(&self) -> VkResult {
        VkResult::from_i32_or_default(self.last_error.load(Ordering::Acquire) as i32)
    }

    /// Update device count in secondary field
    pub fn set_device_count(&self, count: u16) {
        loop {
            let current = self.secondary.load(Ordering::Acquire);
            let new_secondary = (current & !Self::DEVICE_COUNT_MASK)
                | ((count as u64) << Self::DEVICE_COUNT_SHIFT);

            if self.secondary.compare_exchange(
                current,
                new_secondary,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                break;
            }
        }
    }

    /// Update queue count in secondary field
    pub fn set_queue_count(&self, count: u16) {
        loop {
            let current = self.secondary.load(Ordering::Acquire);
            let new_secondary = (current & !Self::QUEUE_COUNT_MASK)
                | ((count as u64) << Self::QUEUE_COUNT_SHIFT);

            if self.secondary.compare_exchange(
                current,
                new_secondary,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                break;
            }
        }
    }

    /// Add capability flag
    pub fn add_capability(&self, cap: VkBackendCapabilities) {
        loop {
            let current = self.secondary.load(Ordering::Acquire);
            let caps = (current & Self::CAPABILITIES_MASK) as u32;
            let new_caps = caps | cap.0;
            let new_secondary = (current & !Self::CAPABILITIES_MASK) | (new_caps as u64);

            if self.secondary.compare_exchange(
                current,
                new_secondary,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                break;
            }
        }
    }

    /// Check if backend is operational
    pub fn is_operational(&self) -> bool {
        matches!(
            self.state(),
            VkBackendState::Ready | VkBackendState::Active
        )
    }

    /// Get full status snapshot (lockfree atomic read)
    pub fn status_snapshot(&self) -> VkBackendStatus {
        VkBackendStatus {
            state: self.state(),
            instance_count: self.instance_count(),
            device_count: self.device_count(),
            queue_count: self.queue_count(),
            capabilities: self.capabilities(),
            generation: self.generation(),
            api_version: self.api_version(),
            physical_device_count: self.physical_device_count(),
            allocations: self.allocation_count(),
            submissions: self.submission_count(),
            error_count: self.error_count(),
        }
    }
}

impl Default for VkBackendCapsule {
    fn default() -> Self {
        Self::new()
    }
}

/// Status snapshot for VkBackendCapsule
#[derive(Debug, Clone)]
pub struct VkBackendStatus {
    pub state: VkBackendState,
    pub instance_count: u8,
    pub device_count: u16,
    pub queue_count: u16,
    pub capabilities: VkBackendCapabilities,
    pub generation: u64,
    pub api_version: u32,
    pub physical_device_count: u32,
    pub allocations: u64,
    pub submissions: u64,
    pub error_count: u64,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // VkBackendCapsule Size/Alignment Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_backend_capsule_size() {
        assert_eq!(core::mem::size_of::<VkBackendCapsule>(), 512);
    }

    #[test]
    fn test_backend_capsule_alignment() {
        assert_eq!(core::mem::align_of::<VkBackendCapsule>(), 512);
    }

    // -------------------------------------------------------------------------
    // VkBackendCapsule Initialization Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_backend_new() {
        let backend = VkBackendCapsule::new();
        assert_eq!(backend.state(), VkBackendState::Uninitialized);
        assert_eq!(backend.instance_count(), 0);
        assert_eq!(backend.generation(), 0);
    }

    #[test]
    fn test_backend_initialize() {
        let backend = VkBackendCapsule::new();
        let result = backend.initialize();
        assert_eq!(result, VkResult::Success);
        assert_eq!(backend.state(), VkBackendState::Ready);
        assert!(backend.generation() > 0);
    }

    #[test]
    fn test_backend_double_initialize() {
        let backend = VkBackendCapsule::new();
        let result1 = backend.initialize();
        let result2 = backend.initialize();
        assert_eq!(result1, VkResult::Success);
        assert_eq!(result2, VkResult::ErrorInitializationFailed);
    }

    // -------------------------------------------------------------------------
    // VkBackendCapsule Instance Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_backend_create_instance() {
        let backend = VkBackendCapsule::new();
        backend.initialize();

        let result = backend.create_instance("TestApp", 1);
        assert!(result.is_ok());

        let handle = result.unwrap();
        assert!(handle > 0);
        assert_eq!(backend.state(), VkBackendState::Active);
        assert_eq!(backend.instance_count(), 1);
    }

    #[test]
    fn test_backend_multiple_instances() {
        let backend = VkBackendCapsule::new();
        backend.initialize();

        let h1 = backend.create_instance("App1", 1).unwrap();
        let h2 = backend.create_instance("App2", 1).unwrap();
        let h3 = backend.create_instance("App3", 1).unwrap();
        let h4 = backend.create_instance("App4", 1).unwrap();

        // All handles should be unique
        assert_ne!(h1, h2);
        assert_ne!(h2, h3);
        assert_ne!(h3, h4);

        assert_eq!(backend.instance_count(), 4);

        // 5th instance should fail
        let h5 = backend.create_instance("App5", 1);
        assert!(h5.is_err());
    }

    #[test]
    fn test_backend_instance_handle_retrieval() {
        let backend = VkBackendCapsule::new();
        backend.initialize();

        let h1 = backend.create_instance("App1", 1).unwrap();

        assert_eq!(backend.instance_handle(0), Some(h1));
        assert_eq!(backend.instance_handle(1), None);
        assert_eq!(backend.instance_handle(4), None); // Out of bounds
    }

    // -------------------------------------------------------------------------
    // VkBackendCapsule State Transition Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_backend_suspend_resume() {
        let backend = VkBackendCapsule::new();
        backend.initialize();
        backend.create_instance("TestApp", 1).unwrap();

        assert_eq!(backend.state(), VkBackendState::Active);

        let suspend_result = backend.suspend();
        assert_eq!(suspend_result, VkResult::Success);
        assert_eq!(backend.state(), VkBackendState::Suspended);

        let resume_result = backend.resume();
        assert_eq!(resume_result, VkResult::Success);
        assert_eq!(backend.state(), VkBackendState::Active);
    }

    #[test]
    fn test_backend_suspend_without_active() {
        let backend = VkBackendCapsule::new();
        backend.initialize();
        // Don't create instance, so state is Ready, not Active

        let result = backend.suspend();
        assert_eq!(result, VkResult::ErrorDeviceLost);
    }

    #[test]
    fn test_backend_shutdown() {
        let backend = VkBackendCapsule::new();
        backend.initialize();
        backend.create_instance("TestApp", 1).unwrap();

        let result = backend.shutdown();
        assert_eq!(result, VkResult::Success);
        assert_eq!(backend.state(), VkBackendState::Terminated);
        assert_eq!(backend.instance_count(), 0);
        assert_eq!(backend.instance_handle(0), None);
    }

    #[test]
    fn test_backend_shutdown_idempotent() {
        let backend = VkBackendCapsule::new();
        backend.initialize();

        backend.shutdown();
        let result = backend.shutdown();

        assert_eq!(result, VkResult::Success);
    }

    // -------------------------------------------------------------------------
    // VkBackendCapsule Device Selection Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_backend_physical_devices() {
        let backend = VkBackendCapsule::new();
        backend.initialize();
        backend.create_instance("TestApp", 1).unwrap();

        // Mock returns 2 physical devices
        assert_eq!(backend.physical_device_count(), 2);
        assert_eq!(backend.selected_device(), 0);
    }

    #[test]
    fn test_backend_select_device() {
        let backend = VkBackendCapsule::new();
        backend.initialize();
        backend.create_instance("TestApp", 1).unwrap();

        let result = backend.select_device(1);
        assert_eq!(result, VkResult::Success);
        assert_eq!(backend.selected_device(), 1);
    }

    #[test]
    fn test_backend_select_invalid_device() {
        let backend = VkBackendCapsule::new();
        backend.initialize();
        backend.create_instance("TestApp", 1).unwrap();

        let result = backend.select_device(99);
        assert_eq!(result, VkResult::ErrorDeviceLost);
    }

    // -------------------------------------------------------------------------
    // VkBackendCapsule Queue Family Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_backend_queue_families() {
        let backend = VkBackendCapsule::new();
        backend.initialize();
        backend.create_instance("TestApp", 1).unwrap();

        assert_eq!(backend.graphics_queue_family(), Some(0));
        assert_eq!(backend.compute_queue_family(), Some(1));
        assert_eq!(backend.transfer_queue_family(), Some(2));
    }

    #[test]
    fn test_backend_queue_families_before_instance() {
        let backend = VkBackendCapsule::new();
        backend.initialize();

        // Before instance creation, queue families are IGNORED
        assert_eq!(backend.graphics_queue_family(), None);
        assert_eq!(backend.compute_queue_family(), None);
        assert_eq!(backend.transfer_queue_family(), None);
    }

    // -------------------------------------------------------------------------
    // VkBackendCapsule Statistics Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_backend_allocation_tracking() {
        let backend = VkBackendCapsule::new();
        backend.initialize();

        assert_eq!(backend.allocation_count(), 0);

        backend.record_allocation();
        backend.record_allocation();
        backend.record_allocation();

        assert_eq!(backend.allocation_count(), 3);
    }

    #[test]
    fn test_backend_submission_tracking() {
        let backend = VkBackendCapsule::new();
        backend.initialize();

        assert_eq!(backend.submission_count(), 0);

        backend.record_submission();
        backend.record_submission();

        assert_eq!(backend.submission_count(), 2);
    }

    #[test]
    fn test_backend_error_tracking() {
        let backend = VkBackendCapsule::new();
        backend.initialize();

        assert_eq!(backend.error_count(), 0);

        backend.record_error(VkResult::ErrorOutOfDeviceMemory);
        backend.record_error(VkResult::ErrorDeviceLost);

        assert_eq!(backend.error_count(), 2);
        assert_eq!(backend.last_error(), VkResult::ErrorDeviceLost);
    }

    // -------------------------------------------------------------------------
    // VkBackendCapsule Secondary Field Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_backend_device_count() {
        let backend = VkBackendCapsule::new();
        backend.initialize();

        backend.set_device_count(3);
        assert_eq!(backend.device_count(), 3);
    }

    #[test]
    fn test_backend_queue_count() {
        let backend = VkBackendCapsule::new();
        backend.initialize();

        backend.set_queue_count(8);
        assert_eq!(backend.queue_count(), 8);
    }

    #[test]
    fn test_backend_capabilities() {
        let backend = VkBackendCapsule::new();
        backend.initialize();

        // Default capabilities set during initialize
        let caps = backend.capabilities();
        assert!(caps.contains(VkBackendCapabilities::GRAPHICS));
        assert!(caps.contains(VkBackendCapabilities::COMPUTE));
        assert!(caps.contains(VkBackendCapabilities::TRANSFER));

        // Add raytracing capability
        backend.add_capability(VkBackendCapabilities::RAYTRACING);
        let caps = backend.capabilities();
        assert!(caps.contains(VkBackendCapabilities::RAYTRACING));
    }

    // -------------------------------------------------------------------------
    // VkBackendCapsule Status Snapshot Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_backend_status_snapshot() {
        let backend = VkBackendCapsule::new();
        backend.initialize();
        backend.create_instance("TestApp", 1).unwrap();
        backend.set_device_count(2);
        backend.set_queue_count(4);
        backend.record_allocation();
        backend.record_submission();

        let status = backend.status_snapshot();

        assert_eq!(status.state, VkBackendState::Active);
        assert_eq!(status.instance_count, 1);
        assert_eq!(status.device_count, 2);
        assert_eq!(status.queue_count, 4);
        assert!(status.generation > 0);
        assert!(status.api_version > 0);
        assert_eq!(status.physical_device_count, 2);
        assert_eq!(status.allocations, 1);
        assert_eq!(status.submissions, 1);
    }

    #[test]
    fn test_backend_is_operational() {
        let backend = VkBackendCapsule::new();
        assert!(!backend.is_operational());

        backend.initialize();
        assert!(backend.is_operational()); // Ready state

        backend.create_instance("TestApp", 1).unwrap();
        assert!(backend.is_operational()); // Active state

        backend.suspend();
        assert!(!backend.is_operational()); // Suspended state

        backend.resume();
        assert!(backend.is_operational()); // Back to Active

        backend.shutdown();
        assert!(!backend.is_operational()); // Terminated state
    }

    // -------------------------------------------------------------------------
    // VkBackendCapsule Concurrent Access Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_backend_concurrent_statistics() {
        use std::sync::Arc;
        use std::thread;

        let backend = Arc::new(VkBackendCapsule::new());
        backend.initialize();

        let mut handles = vec![];

        for _ in 0..4 {
            let backend = Arc::clone(&backend);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    backend.record_allocation();
                    backend.record_submission();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(backend.allocation_count(), 400);
        assert_eq!(backend.submission_count(), 400);
    }

    // -------------------------------------------------------------------------
    // VkBackendCapabilities Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_capabilities_contains() {
        let mut caps = VkBackendCapabilities::NONE;
        assert!(!caps.contains(VkBackendCapabilities::GRAPHICS));

        caps.insert(VkBackendCapabilities::GRAPHICS);
        assert!(caps.contains(VkBackendCapabilities::GRAPHICS));
        assert!(!caps.contains(VkBackendCapabilities::COMPUTE));
    }

    #[test]
    fn test_capabilities_combined() {
        let caps = VkBackendCapabilities(
            VkBackendCapabilities::GRAPHICS.0
            | VkBackendCapabilities::COMPUTE.0
            | VkBackendCapabilities::RAYTRACING.0
        );

        assert!(caps.contains(VkBackendCapabilities::GRAPHICS));
        assert!(caps.contains(VkBackendCapabilities::COMPUTE));
        assert!(caps.contains(VkBackendCapabilities::RAYTRACING));
        assert!(!caps.contains(VkBackendCapabilities::MESH_SHADERS));
    }

    // -------------------------------------------------------------------------
    // Module Re-exports Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_module_exports_types() {
        // Verify types.rs exports are accessible
        let _ = VkResult::Success;
        let _ = VkFormat::R8G8B8A8Unorm;
        let _ = VkImageLayout::Undefined;
    }

    #[test]
    fn test_module_exports_instance() {
        // Verify instance.rs exports are accessible
        let instance = VkInstanceCapsule::new();
        assert_eq!(instance.state(), VK_INSTANCE_STATE_UNINITIALIZED);
    }

    #[test]
    fn test_module_exports_device() {
        // Verify device.rs exports are accessible
        let device = VkDeviceCapsule::new();
        assert_eq!(device.state(), VK_DEVICE_STATE_UNINITIALIZED);
    }

    #[test]
    fn test_module_exports_buffer() {
        // Verify buffer.rs exports are accessible
        let buffer = VkBufferCapsule::new();
        let info = VkBufferCreateInfo::vertex(1024);
        buffer.create(&info);
        assert!(buffer.size() >= 1024);
    }

    #[test]
    fn test_module_exports_image() {
        // Verify image.rs exports are accessible
        let image = VkImageCapsule::new();
        let info = VkImageCreateInfo::texture_2d(256, 256, VkFormat::R8G8B8A8Unorm, 1);
        image.create(&info);
        assert_eq!(image.width(), 256);
    }
}
