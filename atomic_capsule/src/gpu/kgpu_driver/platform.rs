//! GPU Platform Abstraction Layer
//!
//! Core trait for dual-target support (Linux DRM vs Capsule-OS direct).
//!
//! # Architecture
//!
//! ```text
//! +-------------------+     +-------------------+
//! |  GpuPlatform<T>   |     |  GpuPlatform<T>   |
//! |  (Linux DRM)      |     |  (Capsule-OS)     |
//! +--------+----------+     +--------+----------+
//!          |                         |
//!          v                         v
//! +--------+----------+     +--------+----------+
//! |  DRM/GEM ioctls   |     |  MMIO Direct      |
//! |  /dev/dri/cardN   |     |  PCI Config Space |
//! +-------------------+     +-------------------+
//! ```
//!
//! # Chaos Compliance
//!
//! - **T0 Auditable**: GpuDeviceInfo with fixed-size types
//! - **T1 Atomic**: Lockfree handle management
//! - 100% lockfree (no mutex/RwLock)
//! - `#[repr(C)]` on all FFI types
//! - Cache-aligned where needed (64B)
//! - Generation counters for mutable state
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_PLATFORM_VALID`: Platform implementations are valid
//! - `#ASSUME_HANDLE_UNIQUE`: Device handles are unique per open
//! - `#ASSUME_FFI_SAFE`: FFI calls are properly synchronized

#![allow(dead_code)] // Allow during development

use core::fmt::{self, Debug};

#[cfg(feature = "std")]
extern crate std;
#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use super::vendor::GpuGeneration;
use super::error::KgpuDriverError;

// ============================================================================
// Memory Flags (bitflags)
// ============================================================================

/// Memory allocation flags for GPU buffers.
///
/// These flags control visibility, caching, and usage of GPU memory.
///
/// # Example
///
/// ```ignore
/// let flags = MemoryFlags::GPU_VISIBLE | MemoryFlags::CPU_VISIBLE | MemoryFlags::COHERENT;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct MemoryFlags(u32);

impl MemoryFlags {
    /// Memory is visible to the GPU
    pub const GPU_VISIBLE: Self = Self(0x0001);
    /// Memory is visible to the CPU (mappable)
    pub const CPU_VISIBLE: Self = Self(0x0002);
    /// Memory is coherent (no explicit flush needed)
    pub const COHERENT: Self = Self(0x0004);
    /// Memory is uncached (bypass cache)
    pub const UNCACHED: Self = Self(0x0008);
    /// Memory uses write-combine (for streaming writes)
    pub const WRITE_COMBINE: Self = Self(0x0010);
    /// Memory can be used for display scanout
    pub const SCANOUT: Self = Self(0x0020);
    /// Memory is for command buffers
    pub const COMMAND: Self = Self(0x0040);
    /// Memory is for shader code
    pub const SHADER: Self = Self(0x0080);
    /// Memory is for vertex/index buffers
    pub const VERTEX: Self = Self(0x0100);
    /// Memory is for texture data
    pub const TEXTURE: Self = Self(0x0200);
    /// Memory is for uniform/constant buffers
    pub const UNIFORM: Self = Self(0x0400);
    /// Memory is for storage buffers (SSBO)
    pub const STORAGE: Self = Self(0x0800);

    /// Create empty flags
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Create flags from raw value
    #[inline]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Get raw bits
    #[inline]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Check if flags contain a specific flag
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Check if flags are empty
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Union of two flag sets
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Intersection of two flag sets
    #[inline]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Difference of two flag sets
    #[inline]
    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// Toggle flags
    #[inline]
    pub const fn toggle(self, other: Self) -> Self {
        Self(self.0 ^ other.0)
    }
}

impl core::ops::BitOr for MemoryFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for MemoryFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl core::ops::BitAnd for MemoryFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl core::ops::BitAndAssign for MemoryFlags {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl core::ops::BitXor for MemoryFlags {
    type Output = Self;

    #[inline]
    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

impl core::ops::Not for MemoryFlags {
    type Output = Self;

    #[inline]
    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

impl Default for MemoryFlags {
    #[inline]
    fn default() -> Self {
        Self::GPU_VISIBLE
    }
}

// ============================================================================
// Queue Type
// ============================================================================

/// GPU command queue type.
///
/// Different queue types have different capabilities and priorities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum QueueType {
    /// Graphics queue (rendering, rasterization)
    Graphics = 0,
    /// Compute queue (GPGPU, shaders)
    Compute = 1,
    /// Transfer/DMA queue (memory copies)
    Transfer = 2,
    /// Video decode queue (hardware video decoding)
    VideoDecode = 3,
    /// Video encode queue (hardware video encoding)
    VideoEncode = 4,
    /// Sparse binding queue (virtual memory management)
    SparseBind = 5,
}

impl QueueType {
    /// All queue types
    pub const ALL: [Self; 6] = [
        Self::Graphics,
        Self::Compute,
        Self::Transfer,
        Self::VideoDecode,
        Self::VideoEncode,
        Self::SparseBind,
    ];

    /// Get queue type name
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Graphics => "Graphics",
            Self::Compute => "Compute",
            Self::Transfer => "Transfer",
            Self::VideoDecode => "VideoDecode",
            Self::VideoEncode => "VideoEncode",
            Self::SparseBind => "SparseBind",
        }
    }

    /// Check if queue supports graphics operations
    #[inline]
    pub const fn supports_graphics(self) -> bool {
        matches!(self, Self::Graphics)
    }

    /// Check if queue supports compute operations
    #[inline]
    pub const fn supports_compute(self) -> bool {
        matches!(self, Self::Graphics | Self::Compute)
    }

    /// Check if queue supports transfer operations
    #[inline]
    pub const fn supports_transfer(self) -> bool {
        matches!(self, Self::Graphics | Self::Compute | Self::Transfer)
    }
}

impl fmt::Display for QueueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// Submission ID
// ============================================================================

/// Command submission identifier.
///
/// A unique ID returned when commands are submitted to a queue.
/// Used for tracking and waiting on command completion.
///
/// # Layout
///
/// ```text
/// [63:48] queue_index (16 bits)
/// [47:0]  sequence_number (48 bits)
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SubmissionId(pub u64);

impl SubmissionId {
    /// Invalid/null submission ID
    pub const INVALID: Self = Self(0);

    /// Create from queue index and sequence number
    #[inline]
    pub const fn new(queue_index: u16, sequence: u64) -> Self {
        Self(((queue_index as u64) << 48) | (sequence & 0x0000_FFFF_FFFF_FFFF))
    }

    /// Get the queue index
    #[inline]
    pub const fn queue_index(self) -> u16 {
        (self.0 >> 48) as u16
    }

    /// Get the sequence number
    #[inline]
    pub const fn sequence(self) -> u64 {
        self.0 & 0x0000_FFFF_FFFF_FFFF
    }

    /// Check if this is a valid submission ID
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }

    /// Get raw value
    #[inline]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl Debug for SubmissionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SubmissionId")
            .field("queue", &self.queue_index())
            .field("seq", &self.sequence())
            .finish()
    }
}

impl fmt::Display for SubmissionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sub[Q{}:{}]", self.queue_index(), self.sequence())
    }
}

impl Default for SubmissionId {
    #[inline]
    fn default() -> Self {
        Self::INVALID
    }
}

// ============================================================================
// Fence Handle
// ============================================================================

/// GPU synchronization fence handle.
///
/// Fences are used for CPU-GPU synchronization.
///
/// # Layout
///
/// ```text
/// [63:56] device_index (8 bits)
/// [55:48] fence_type (8 bits)
/// [47:0]  fence_id (48 bits)
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct FenceHandle(pub u64);

impl FenceHandle {
    /// Invalid/null fence handle
    pub const INVALID: Self = Self(0);

    /// Create from device index, fence type, and ID
    #[inline]
    pub const fn new(device: u8, fence_type: u8, id: u64) -> Self {
        Self(
            ((device as u64) << 56)
                | ((fence_type as u64) << 48)
                | (id & 0x0000_FFFF_FFFF_FFFF),
        )
    }

    /// Get the device index
    #[inline]
    pub const fn device_index(self) -> u8 {
        (self.0 >> 56) as u8
    }

    /// Get the fence type
    #[inline]
    pub const fn fence_type(self) -> u8 {
        ((self.0 >> 48) & 0xFF) as u8
    }

    /// Get the fence ID
    #[inline]
    pub const fn fence_id(self) -> u64 {
        self.0 & 0x0000_FFFF_FFFF_FFFF
    }

    /// Check if this is a valid fence handle
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }

    /// Get raw value
    #[inline]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl Debug for FenceHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FenceHandle")
            .field("device", &self.device_index())
            .field("type", &self.fence_type())
            .field("id", &self.fence_id())
            .finish()
    }
}

impl fmt::Display for FenceHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Fence[D{}:T{}:{}]",
            self.device_index(),
            self.fence_type(),
            self.fence_id()
        )
    }
}

impl Default for FenceHandle {
    #[inline]
    fn default() -> Self {
        Self::INVALID
    }
}

// ============================================================================
// Firmware Types
// ============================================================================

/// GPU firmware type.
///
/// Different GPUs require different firmware components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FirmwareType {
    /// Intel Graphics Micro Controller (GuC)
    /// - Workload scheduling
    /// - Power management
    GuC = 0,

    /// Intel HEVC Micro Controller (HuC)
    /// - Media decode authentication
    /// - DRM content protection
    HuC = 1,

    /// AMD Platform Security Processor (PSP)
    /// - Secure boot
    /// - Memory encryption
    Psp = 2,

    /// NVIDIA GPU System Processor (GSP)
    /// - System management
    /// - Note: Bypassed via Trojan kernel approach
    Gsp = 3,

    /// Display Micro Controller Unit (DMCU)
    /// - Display power management
    /// - Adaptive backlight
    Dmcu = 4,

    /// Video BIOS (VBIOS)
    /// - GPU initialization
    /// - Display configuration
    Vbios = 5,

    /// Generic microcontroller firmware
    Generic = 255,
}

impl FirmwareType {
    /// Get firmware type name
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::GuC => "GuC",
            Self::HuC => "HuC",
            Self::Psp => "PSP",
            Self::Gsp => "GSP",
            Self::Dmcu => "DMCU",
            Self::Vbios => "VBIOS",
            Self::Generic => "Generic",
        }
    }

    /// Get typical firmware size (bytes)
    #[inline]
    pub const fn typical_size(self) -> usize {
        match self {
            Self::GuC => 256 * 1024,      // 256 KB
            Self::HuC => 512 * 1024,      // 512 KB
            Self::Psp => 2 * 1024 * 1024, // 2 MB
            Self::Gsp => 4 * 1024 * 1024, // 4 MB
            Self::Dmcu => 128 * 1024,     // 128 KB
            Self::Vbios => 256 * 1024,    // 256 KB
            Self::Generic => 1024 * 1024, // 1 MB default
        }
    }
}

impl fmt::Display for FirmwareType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// Firmware Status
// ============================================================================

/// Firmware loading/running status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FirmwareStatus {
    /// Firmware not loaded
    NotLoaded = 0,
    /// Firmware currently loading
    Loading = 1,
    /// Firmware running successfully
    Running = 2,
    /// Firmware load/run failed
    Failed = 3,
    /// Firmware bypassed (e.g., NVIDIA Trojan approach)
    Bypassed = 4,
    /// Firmware disabled by configuration
    Disabled = 5,
    /// Firmware not required for this hardware
    NotRequired = 6,
}

impl FirmwareStatus {
    /// Check if firmware is operational
    #[inline]
    pub const fn is_operational(self) -> bool {
        matches!(self, Self::Running | Self::Bypassed | Self::NotRequired)
    }

    /// Check if firmware had an error
    #[inline]
    pub const fn is_error(self) -> bool {
        matches!(self, Self::Failed)
    }

    /// Check if firmware is loading
    #[inline]
    pub const fn is_loading(self) -> bool {
        matches!(self, Self::Loading)
    }

    /// Get status name
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::NotLoaded => "NotLoaded",
            Self::Loading => "Loading",
            Self::Running => "Running",
            Self::Failed => "Failed",
            Self::Bypassed => "Bypassed",
            Self::Disabled => "Disabled",
            Self::NotRequired => "NotRequired",
        }
    }
}

impl fmt::Display for FirmwareStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl Default for FirmwareStatus {
    #[inline]
    fn default() -> Self {
        Self::NotLoaded
    }
}

// ============================================================================
// GPU Device Info
// ============================================================================

/// Maximum length of device name (fixed-size for no_std)
pub const GPU_DEVICE_NAME_LEN: usize = 64;

/// GPU device information.
///
/// Fixed-size structure for no_std compatibility.
/// All fields are `#[repr(C)]` for FFI safety.
///
/// # Size: 128 bytes (cache-line aligned)
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C, align(64))]
pub struct GpuDeviceInfo {
    /// PCI vendor ID (e.g., 0x8086 = Intel, 0x1002 = AMD, 0x10DE = NVIDIA)
    pub vendor_id: u16,
    /// PCI device ID
    pub device_id: u16,
    /// PCI subsystem vendor ID
    pub subsys_vendor: u16,
    /// PCI subsystem device ID
    pub subsys_device: u16,
    /// PCI revision
    pub revision: u8,
    /// GPU generation (from vendor module)
    pub generation: GpuGeneration,
    /// Reserved for alignment (2 bytes)
    _reserved: [u8; 2],
    /// Device name (fixed-size, null-terminated UTF-8)
    pub name: [u8; GPU_DEVICE_NAME_LEN],
    /// Video RAM size in bytes
    pub vram_size: u64,
    /// Shared system memory size in bytes
    pub shared_mem_size: u64,
    /// Maximum compute units / execution units
    pub max_compute_units: u32,
    /// Maximum clock speed in MHz
    pub max_clock_mhz: u32,
    /// Supported queue type bitmask
    pub queue_support: u32,
    /// Number of available queues
    pub num_queues: u8,
    /// PCI bus number
    pub pci_bus: u8,
    /// PCI device number
    pub pci_device: u8,
    /// PCI function number
    pub pci_function: u8,
}

impl GpuDeviceInfo {
    /// Create a new device info with defaults
    #[inline]
    pub const fn new() -> Self {
        Self {
            vendor_id: 0,
            device_id: 0,
            subsys_vendor: 0,
            subsys_device: 0,
            revision: 0,
            generation: GpuGeneration::Unknown,
            _reserved: [0; 2],
            name: [0; GPU_DEVICE_NAME_LEN],
            vram_size: 0,
            shared_mem_size: 0,
            max_compute_units: 0,
            max_clock_mhz: 0,
            queue_support: 0,
            num_queues: 0,
            pci_bus: 0,
            pci_device: 0,
            pci_function: 0,
        }
    }

    /// Get device name as a string slice
    #[inline]
    pub fn name_str(&self) -> &str {
        // Find null terminator
        let len = self
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(GPU_DEVICE_NAME_LEN);
        // SAFETY: We only store valid UTF-8 in name
        core::str::from_utf8(&self.name[..len]).unwrap_or("Unknown")
    }

    /// Set device name from string
    #[inline]
    pub fn set_name(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let len = bytes.len().min(GPU_DEVICE_NAME_LEN - 1);
        self.name[..len].copy_from_slice(&bytes[..len]);
        self.name[len] = 0; // Null terminate
    }

    /// Check if queue type is supported
    #[inline]
    pub const fn supports_queue(&self, queue: QueueType) -> bool {
        (self.queue_support & (1 << (queue as u32))) != 0
    }

    /// Get PCI BDF (Bus:Device.Function) string
    #[cfg(feature = "std")]
    pub fn pci_bdf_string(&self) -> std::string::String {
        std::format!(
            "{:02x}:{:02x}.{}",
            self.pci_bus, self.pci_device, self.pci_function
        )
    }

    /// Get total memory (VRAM + shared)
    #[inline]
    pub const fn total_memory(&self) -> u64 {
        self.vram_size.saturating_add(self.shared_mem_size)
    }
}

impl Default for GpuDeviceInfo {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Debug for GpuDeviceInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GpuDeviceInfo")
            .field("vendor_id", &format_args!("0x{:04X}", self.vendor_id))
            .field("device_id", &format_args!("0x{:04X}", self.device_id))
            .field("generation", &self.generation)
            .field("name", &self.name_str())
            .field("vram_size", &self.vram_size)
            .field("compute_units", &self.max_compute_units)
            .finish()
    }
}

// ============================================================================
// GPU Platform Trait
// ============================================================================

/// Core GPU platform abstraction trait.
///
/// This trait abstracts over the underlying platform (Linux DRM vs Capsule-OS direct),
/// providing a unified interface for GPU operations.
///
/// # Type Parameters
///
/// - `DeviceHandle`: Platform-specific device handle type
/// - `MemoryHandle`: Platform-specific memory allocation handle type
/// - `FenceHandle`: Platform-specific fence handle type
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync` to allow concurrent access.
/// Implementations must be 100% lockfree (Chaos mandate).
///
/// # Example
///
/// ```ignore
/// use atomic_capsule::gpu::kgpu_driver::{GpuPlatform, LinuxGpuPlatform};
///
/// // Enumerate devices
/// let devices = LinuxGpuPlatform::enumerate_devices()?;
///
/// // Open first device
/// let handle = LinuxGpuPlatform::open_device(0)?;
///
/// // Allocate memory
/// let mem = LinuxGpuPlatform::alloc_memory(
///     handle,
///     1024 * 1024,
///     MemoryFlags::GPU_VISIBLE | MemoryFlags::CPU_VISIBLE,
/// )?;
///
/// // Map for CPU access
/// let ptr = LinuxGpuPlatform::map_memory(handle, mem)?;
///
/// // Submit commands
/// let cmd_data = [/* command bytes */];
/// let sub_id = LinuxGpuPlatform::submit_commands(handle, QueueType::Graphics, &cmd_data)?;
///
/// // Wait for completion
/// LinuxGpuPlatform::wait_submission(handle, sub_id)?;
///
/// // Cleanup
/// LinuxGpuPlatform::unmap_memory(handle, mem)?;
/// LinuxGpuPlatform::free_memory(handle, mem)?;
/// LinuxGpuPlatform::close_device(handle)?;
/// ```
pub trait GpuPlatform: Send + Sync {
    /// Platform-specific device handle
    type DeviceHandle: Copy + Send + Sync + Debug;

    /// Platform-specific memory allocation handle
    type MemoryHandle: Copy + Send + Sync + Debug;

    /// Platform-specific fence handle (may differ from generic FenceHandle)
    type PlatformFenceHandle: Copy + Send + Sync + Debug;

    // ========================================================================
    // Device Enumeration
    // ========================================================================

    /// Enumerate all available GPU devices.
    ///
    /// # Returns
    ///
    /// Vector of device information for all detected GPUs.
    ///
    /// # Errors
    ///
    /// - `KgpuDriverError::EnumerationFailed`: Device enumeration failed
    /// - `KgpuDriverError::NoDevice`: No GPU devices found
    fn enumerate_devices() -> Result<Vec<GpuDeviceInfo>, KgpuDriverError>;

    /// Open a GPU device by index.
    ///
    /// # Arguments
    ///
    /// * `device_index` - Index from `enumerate_devices()` result
    ///
    /// # Returns
    ///
    /// Platform-specific device handle.
    ///
    /// # Errors
    ///
    /// - `KgpuDriverError::InvalidDevice`: Device index out of range
    /// - `KgpuDriverError::DeviceBusy`: Device already in use
    /// - `KgpuDriverError::PermissionDenied`: Insufficient permissions
    fn open_device(device_index: usize) -> Result<Self::DeviceHandle, KgpuDriverError>;

    /// Close a GPU device.
    ///
    /// # Arguments
    ///
    /// * `handle` - Device handle from `open_device()`
    ///
    /// # Safety
    ///
    /// All resources (memory, fences) must be freed before closing.
    fn close_device(handle: Self::DeviceHandle) -> Result<(), KgpuDriverError>;

    /// Get device information for an open device.
    ///
    /// # Arguments
    ///
    /// * `handle` - Device handle from `open_device()`
    fn get_device_info(handle: Self::DeviceHandle) -> Result<GpuDeviceInfo, KgpuDriverError>;

    // ========================================================================
    // Memory Management
    // ========================================================================

    /// Allocate GPU memory.
    ///
    /// # Arguments
    ///
    /// * `handle` - Device handle
    /// * `size` - Allocation size in bytes
    /// * `flags` - Memory type flags
    ///
    /// # Returns
    ///
    /// Platform-specific memory handle.
    ///
    /// # Errors
    ///
    /// - `KgpuDriverError::OutOfMemory`: Insufficient GPU memory
    /// - `KgpuDriverError::InvalidFlags`: Invalid flag combination
    fn alloc_memory(
        handle: Self::DeviceHandle,
        size: usize,
        flags: MemoryFlags,
    ) -> Result<Self::MemoryHandle, KgpuDriverError>;

    /// Free GPU memory.
    ///
    /// # Arguments
    ///
    /// * `handle` - Device handle
    /// * `mem` - Memory handle from `alloc_memory()`
    fn free_memory(
        handle: Self::DeviceHandle,
        mem: Self::MemoryHandle,
    ) -> Result<(), KgpuDriverError>;

    /// Map GPU memory for CPU access.
    ///
    /// # Arguments
    ///
    /// * `handle` - Device handle
    /// * `mem` - Memory handle with `CPU_VISIBLE` flag
    ///
    /// # Returns
    ///
    /// Pointer to mapped memory region.
    ///
    /// # Safety
    ///
    /// The returned pointer is valid until `unmap_memory()` is called.
    /// Memory must have been allocated with `MemoryFlags::CPU_VISIBLE`.
    fn map_memory(
        handle: Self::DeviceHandle,
        mem: Self::MemoryHandle,
    ) -> Result<*mut u8, KgpuDriverError>;

    /// Unmap GPU memory from CPU access.
    ///
    /// # Arguments
    ///
    /// * `handle` - Device handle
    /// * `mem` - Memory handle
    fn unmap_memory(
        handle: Self::DeviceHandle,
        mem: Self::MemoryHandle,
    ) -> Result<(), KgpuDriverError>;

    /// Get memory size.
    ///
    /// # Arguments
    ///
    /// * `handle` - Device handle
    /// * `mem` - Memory handle
    fn get_memory_size(
        handle: Self::DeviceHandle,
        mem: Self::MemoryHandle,
    ) -> Result<usize, KgpuDriverError>;

    // ========================================================================
    // Command Submission
    // ========================================================================

    /// Submit commands to a queue.
    ///
    /// # Arguments
    ///
    /// * `handle` - Device handle
    /// * `queue` - Queue type to submit to
    /// * `commands` - Raw command buffer bytes
    ///
    /// # Returns
    ///
    /// Submission ID for tracking.
    ///
    /// # Errors
    ///
    /// - `KgpuDriverError::QueueNotSupported`: Queue type not available
    /// - `KgpuDriverError::CommandBufferTooLarge`: Commands exceed limit
    fn submit_commands(
        handle: Self::DeviceHandle,
        queue: QueueType,
        commands: &[u8],
    ) -> Result<SubmissionId, KgpuDriverError>;

    /// Wait for a submission to complete.
    ///
    /// # Arguments
    ///
    /// * `handle` - Device handle
    /// * `id` - Submission ID from `submit_commands()`
    ///
    /// # Errors
    ///
    /// - `KgpuDriverError::Timeout`: Wait timed out
    /// - `KgpuDriverError::SubmissionFailed`: GPU execution failed
    fn wait_submission(
        handle: Self::DeviceHandle,
        id: SubmissionId,
    ) -> Result<(), KgpuDriverError>;

    /// Check if a submission has completed (non-blocking).
    ///
    /// # Arguments
    ///
    /// * `handle` - Device handle
    /// * `id` - Submission ID
    ///
    /// # Returns
    ///
    /// `true` if completed, `false` if still pending.
    fn is_submission_complete(
        handle: Self::DeviceHandle,
        id: SubmissionId,
    ) -> Result<bool, KgpuDriverError>;

    // ========================================================================
    // Synchronization
    // ========================================================================

    /// Create a GPU fence.
    ///
    /// # Arguments
    ///
    /// * `handle` - Device handle
    ///
    /// # Returns
    ///
    /// Platform-specific fence handle.
    fn create_fence(
        handle: Self::DeviceHandle,
    ) -> Result<Self::PlatformFenceHandle, KgpuDriverError>;

    /// Wait for a fence to be signaled.
    ///
    /// # Arguments
    ///
    /// * `handle` - Device handle
    /// * `fence` - Fence handle
    /// * `timeout_ns` - Timeout in nanoseconds (0 = poll, u64::MAX = infinite)
    ///
    /// # Returns
    ///
    /// `true` if fence signaled, `false` if timeout.
    fn wait_fence(
        handle: Self::DeviceHandle,
        fence: Self::PlatformFenceHandle,
        timeout_ns: u64,
    ) -> Result<bool, KgpuDriverError>;

    /// Destroy a fence.
    ///
    /// # Arguments
    ///
    /// * `handle` - Device handle
    /// * `fence` - Fence handle
    fn destroy_fence(
        handle: Self::DeviceHandle,
        fence: Self::PlatformFenceHandle,
    ) -> Result<(), KgpuDriverError>;

    /// Reset a fence to unsignaled state.
    ///
    /// # Arguments
    ///
    /// * `handle` - Device handle
    /// * `fence` - Fence handle
    fn reset_fence(
        handle: Self::DeviceHandle,
        fence: Self::PlatformFenceHandle,
    ) -> Result<(), KgpuDriverError>;

    // ========================================================================
    // Firmware
    // ========================================================================

    /// Load firmware to the GPU.
    ///
    /// # Arguments
    ///
    /// * `handle` - Device handle
    /// * `fw_type` - Firmware type to load
    /// * `data` - Firmware binary data
    ///
    /// # Errors
    ///
    /// - `KgpuDriverError::FirmwareLoadFailed`: Load failed
    /// - `KgpuDriverError::FirmwareNotSupported`: FW type not supported
    fn load_firmware(
        handle: Self::DeviceHandle,
        fw_type: FirmwareType,
        data: &[u8],
    ) -> Result<(), KgpuDriverError>;

    /// Get firmware status.
    ///
    /// # Arguments
    ///
    /// * `handle` - Device handle
    /// * `fw_type` - Firmware type to query
    fn firmware_status(
        handle: Self::DeviceHandle,
        fw_type: FirmwareType,
    ) -> Result<FirmwareStatus, KgpuDriverError>;
}

// ============================================================================
// Platform Stubs
// ============================================================================

/// Linux platform using DRM/GEM/KMS.
///
/// Uses kernel ioctls for GPU operations:
/// - Device enumeration via `/dev/dri/cardN`
/// - Memory via GEM (Graphics Execution Manager)
/// - Display via KMS (Kernel Mode Setting)
/// - Commands via DRM (Direct Rendering Manager)
#[cfg(feature = "kgpu-driver-linux")]
pub struct LinuxGpuPlatform;

/// DRM device handle (file descriptor wrapper)
#[cfg(feature = "kgpu-driver-linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct DrmDeviceHandle(pub i32);

/// GEM buffer object handle
#[cfg(feature = "kgpu-driver-linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct GemHandle(pub u32);

/// DRM syncobj handle
#[cfg(feature = "kgpu-driver-linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct SyncobjHandle(pub u32);

/// Capsule-OS platform with direct hardware access.
///
/// Bypasses the kernel for direct GPU control:
/// - Direct MMIO register access
/// - Direct DMA buffer management
/// - Direct ring buffer submission
///
/// # Safety
///
/// Requires kernel-level privileges or custom kernel support.
#[cfg(feature = "kgpu-driver-capsule-os")]
pub struct CapsuleOsGpuPlatform;

/// Direct device handle (MMIO base + size)
#[cfg(feature = "kgpu-driver-capsule-os")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct DirectDeviceHandle {
    /// MMIO base address
    pub mmio_base: usize,
    /// MMIO region size
    pub mmio_size: usize,
    /// Device index
    pub device_index: u8,
}

/// Direct memory handle (physical address + size)
#[cfg(feature = "kgpu-driver-capsule-os")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct DirectMemoryHandle {
    /// Physical address
    pub phys_addr: u64,
    /// Size in bytes
    pub size: usize,
    /// Virtual address (if mapped)
    pub virt_addr: usize,
}

/// Direct fence handle (memory-mapped spinlock)
#[cfg(feature = "kgpu-driver-capsule-os")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct DirectFenceHandle {
    /// Fence value address
    pub value_addr: usize,
    /// Expected value when signaled
    pub signal_value: u64,
}

// ============================================================================
// Compile-Time Assertions
// ============================================================================

// Verify GpuDeviceInfo is cache-line aligned and properly sized
const _: () = {
    assert!(
        core::mem::size_of::<GpuDeviceInfo>() == 128,
        "GpuDeviceInfo must be 128 bytes"
    );
    assert!(
        core::mem::align_of::<GpuDeviceInfo>() == 64,
        "GpuDeviceInfo must be 64-byte aligned"
    );
};

// Verify SubmissionId is transparent
const _: () = {
    assert!(
        core::mem::size_of::<SubmissionId>() == 8,
        "SubmissionId must be 8 bytes"
    );
};

// Verify FenceHandle is transparent
const _: () = {
    assert!(
        core::mem::size_of::<FenceHandle>() == 8,
        "FenceHandle must be 8 bytes"
    );
};

// Verify MemoryFlags is transparent
const _: () = {
    assert!(
        core::mem::size_of::<MemoryFlags>() == 4,
        "MemoryFlags must be 4 bytes"
    );
};

// Verify QueueType fits in u8
const _: () = {
    assert!(
        core::mem::size_of::<QueueType>() == 1,
        "QueueType must be 1 byte"
    );
};

// Verify FirmwareType fits in u8
const _: () = {
    assert!(
        core::mem::size_of::<FirmwareType>() == 1,
        "FirmwareType must be 1 byte"
    );
};

// Verify FirmwareStatus fits in u8
const _: () = {
    assert!(
        core::mem::size_of::<FirmwareStatus>() == 1,
        "FirmwareStatus must be 1 byte"
    );
};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // MemoryFlags Tests
    // ========================================================================

    #[test]
    fn test_memory_flags_empty() {
        let flags = MemoryFlags::empty();
        assert!(flags.is_empty());
        assert_eq!(flags.bits(), 0);
    }

    #[test]
    fn test_memory_flags_single() {
        let flags = MemoryFlags::GPU_VISIBLE;
        assert!(!flags.is_empty());
        assert!(flags.contains(MemoryFlags::GPU_VISIBLE));
        assert!(!flags.contains(MemoryFlags::CPU_VISIBLE));
    }

    #[test]
    fn test_memory_flags_bitor() {
        let flags = MemoryFlags::GPU_VISIBLE | MemoryFlags::CPU_VISIBLE;
        assert!(flags.contains(MemoryFlags::GPU_VISIBLE));
        assert!(flags.contains(MemoryFlags::CPU_VISIBLE));
        assert!(!flags.contains(MemoryFlags::COHERENT));
    }

    #[test]
    fn test_memory_flags_bitand() {
        let a = MemoryFlags::GPU_VISIBLE | MemoryFlags::CPU_VISIBLE;
        let b = MemoryFlags::CPU_VISIBLE | MemoryFlags::COHERENT;
        let c = a & b;
        assert!(c.contains(MemoryFlags::CPU_VISIBLE));
        assert!(!c.contains(MemoryFlags::GPU_VISIBLE));
        assert!(!c.contains(MemoryFlags::COHERENT));
    }

    #[test]
    fn test_memory_flags_union() {
        let a = MemoryFlags::GPU_VISIBLE;
        let b = MemoryFlags::CPU_VISIBLE;
        let c = a.union(b);
        assert!(c.contains(MemoryFlags::GPU_VISIBLE));
        assert!(c.contains(MemoryFlags::CPU_VISIBLE));
    }

    #[test]
    fn test_memory_flags_intersection() {
        let a = MemoryFlags::GPU_VISIBLE | MemoryFlags::CPU_VISIBLE;
        let b = MemoryFlags::CPU_VISIBLE | MemoryFlags::COHERENT;
        let c = a.intersection(b);
        assert_eq!(c, MemoryFlags::CPU_VISIBLE);
    }

    #[test]
    fn test_memory_flags_difference() {
        let a = MemoryFlags::GPU_VISIBLE | MemoryFlags::CPU_VISIBLE;
        let b = MemoryFlags::CPU_VISIBLE;
        let c = a.difference(b);
        assert!(c.contains(MemoryFlags::GPU_VISIBLE));
        assert!(!c.contains(MemoryFlags::CPU_VISIBLE));
    }

    #[test]
    fn test_memory_flags_toggle() {
        let a = MemoryFlags::GPU_VISIBLE | MemoryFlags::CPU_VISIBLE;
        let b = MemoryFlags::CPU_VISIBLE | MemoryFlags::COHERENT;
        let c = a.toggle(b);
        assert!(c.contains(MemoryFlags::GPU_VISIBLE));
        assert!(!c.contains(MemoryFlags::CPU_VISIBLE));
        assert!(c.contains(MemoryFlags::COHERENT));
    }

    #[test]
    fn test_memory_flags_size() {
        assert_eq!(core::mem::size_of::<MemoryFlags>(), 4);
    }

    // ========================================================================
    // QueueType Tests
    // ========================================================================

    #[test]
    fn test_queue_type_values() {
        assert_eq!(QueueType::Graphics as u8, 0);
        assert_eq!(QueueType::Compute as u8, 1);
        assert_eq!(QueueType::Transfer as u8, 2);
        assert_eq!(QueueType::VideoDecode as u8, 3);
        assert_eq!(QueueType::VideoEncode as u8, 4);
        assert_eq!(QueueType::SparseBind as u8, 5);
    }

    #[test]
    fn test_queue_type_names() {
        assert_eq!(QueueType::Graphics.name(), "Graphics");
        assert_eq!(QueueType::Compute.name(), "Compute");
        assert_eq!(QueueType::Transfer.name(), "Transfer");
    }

    #[test]
    fn test_queue_type_capabilities() {
        assert!(QueueType::Graphics.supports_graphics());
        assert!(!QueueType::Compute.supports_graphics());

        assert!(QueueType::Graphics.supports_compute());
        assert!(QueueType::Compute.supports_compute());
        assert!(!QueueType::Transfer.supports_compute());

        assert!(QueueType::Graphics.supports_transfer());
        assert!(QueueType::Compute.supports_transfer());
        assert!(QueueType::Transfer.supports_transfer());
        assert!(!QueueType::VideoDecode.supports_transfer());
    }

    #[test]
    fn test_queue_type_size() {
        assert_eq!(core::mem::size_of::<QueueType>(), 1);
    }

    // ========================================================================
    // SubmissionId Tests
    // ========================================================================

    #[test]
    fn test_submission_id_new() {
        let id = SubmissionId::new(5, 12345);
        assert_eq!(id.queue_index(), 5);
        assert_eq!(id.sequence(), 12345);
    }

    #[test]
    fn test_submission_id_invalid() {
        let id = SubmissionId::INVALID;
        assert!(!id.is_valid());
        assert_eq!(id.raw(), 0);
    }

    #[test]
    fn test_submission_id_transparency() {
        let id = SubmissionId(0x0005_0000_0000_3039); // queue=5, seq=12345
        assert_eq!(id.queue_index(), 5);
        assert_eq!(id.sequence(), 12345);
    }

    #[test]
    fn test_submission_id_size() {
        assert_eq!(core::mem::size_of::<SubmissionId>(), 8);
    }

    #[test]
    fn test_submission_id_max_values() {
        let id = SubmissionId::new(0xFFFF, 0x0000_FFFF_FFFF_FFFF);
        assert_eq!(id.queue_index(), 0xFFFF);
        assert_eq!(id.sequence(), 0x0000_FFFF_FFFF_FFFF);
    }

    // ========================================================================
    // FenceHandle Tests
    // ========================================================================

    #[test]
    fn test_fence_handle_new() {
        let fence = FenceHandle::new(2, 1, 9999);
        assert_eq!(fence.device_index(), 2);
        assert_eq!(fence.fence_type(), 1);
        assert_eq!(fence.fence_id(), 9999);
    }

    #[test]
    fn test_fence_handle_invalid() {
        let fence = FenceHandle::INVALID;
        assert!(!fence.is_valid());
        assert_eq!(fence.raw(), 0);
    }

    #[test]
    fn test_fence_handle_size() {
        assert_eq!(core::mem::size_of::<FenceHandle>(), 8);
    }

    // ========================================================================
    // FirmwareType Tests
    // ========================================================================

    #[test]
    fn test_firmware_type_values() {
        assert_eq!(FirmwareType::GuC as u8, 0);
        assert_eq!(FirmwareType::HuC as u8, 1);
        assert_eq!(FirmwareType::Psp as u8, 2);
        assert_eq!(FirmwareType::Gsp as u8, 3);
        assert_eq!(FirmwareType::Dmcu as u8, 4);
    }

    #[test]
    fn test_firmware_type_names() {
        assert_eq!(FirmwareType::GuC.name(), "GuC");
        assert_eq!(FirmwareType::HuC.name(), "HuC");
        assert_eq!(FirmwareType::Psp.name(), "PSP");
        assert_eq!(FirmwareType::Gsp.name(), "GSP");
    }

    #[test]
    fn test_firmware_type_sizes() {
        assert!(FirmwareType::GuC.typical_size() > 0);
        assert!(FirmwareType::Psp.typical_size() > FirmwareType::GuC.typical_size());
    }

    #[test]
    fn test_firmware_type_size() {
        assert_eq!(core::mem::size_of::<FirmwareType>(), 1);
    }

    // ========================================================================
    // FirmwareStatus Tests
    // ========================================================================

    #[test]
    fn test_firmware_status_values() {
        assert_eq!(FirmwareStatus::NotLoaded as u8, 0);
        assert_eq!(FirmwareStatus::Loading as u8, 1);
        assert_eq!(FirmwareStatus::Running as u8, 2);
        assert_eq!(FirmwareStatus::Failed as u8, 3);
        assert_eq!(FirmwareStatus::Bypassed as u8, 4);
    }

    #[test]
    fn test_firmware_status_operational() {
        assert!(!FirmwareStatus::NotLoaded.is_operational());
        assert!(!FirmwareStatus::Loading.is_operational());
        assert!(FirmwareStatus::Running.is_operational());
        assert!(!FirmwareStatus::Failed.is_operational());
        assert!(FirmwareStatus::Bypassed.is_operational());
        assert!(FirmwareStatus::NotRequired.is_operational());
    }

    #[test]
    fn test_firmware_status_error() {
        assert!(!FirmwareStatus::NotLoaded.is_error());
        assert!(!FirmwareStatus::Loading.is_error());
        assert!(!FirmwareStatus::Running.is_error());
        assert!(FirmwareStatus::Failed.is_error());
        assert!(!FirmwareStatus::Bypassed.is_error());
    }

    #[test]
    fn test_firmware_status_loading() {
        assert!(!FirmwareStatus::NotLoaded.is_loading());
        assert!(FirmwareStatus::Loading.is_loading());
        assert!(!FirmwareStatus::Running.is_loading());
    }

    #[test]
    fn test_firmware_status_size() {
        assert_eq!(core::mem::size_of::<FirmwareStatus>(), 1);
    }

    // ========================================================================
    // GpuDeviceInfo Tests
    // ========================================================================

    #[test]
    fn test_gpu_device_info_new() {
        let info = GpuDeviceInfo::new();
        assert_eq!(info.vendor_id, 0);
        assert_eq!(info.device_id, 0);
        assert_eq!(info.vram_size, 0);
    }

    #[test]
    fn test_gpu_device_info_name() {
        let mut info = GpuDeviceInfo::new();
        info.set_name("Test GPU");
        assert_eq!(info.name_str(), "Test GPU");
    }

    #[test]
    fn test_gpu_device_info_name_truncation() {
        let mut info = GpuDeviceInfo::new();
        let long_name = "A".repeat(100);
        info.set_name(&long_name);
        assert_eq!(info.name_str().len(), GPU_DEVICE_NAME_LEN - 1);
    }

    #[test]
    fn test_gpu_device_info_queue_support() {
        let mut info = GpuDeviceInfo::new();
        info.queue_support = (1 << QueueType::Graphics as u32)
            | (1 << QueueType::Compute as u32)
            | (1 << QueueType::Transfer as u32);

        assert!(info.supports_queue(QueueType::Graphics));
        assert!(info.supports_queue(QueueType::Compute));
        assert!(info.supports_queue(QueueType::Transfer));
        assert!(!info.supports_queue(QueueType::VideoDecode));
    }

    #[test]
    fn test_gpu_device_info_total_memory() {
        let mut info = GpuDeviceInfo::new();
        info.vram_size = 8 * 1024 * 1024 * 1024; // 8 GB
        info.shared_mem_size = 4 * 1024 * 1024 * 1024; // 4 GB
        assert_eq!(info.total_memory(), 12 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_gpu_device_info_size() {
        assert_eq!(core::mem::size_of::<GpuDeviceInfo>(), 128);
    }

    #[test]
    fn test_gpu_device_info_alignment() {
        assert_eq!(core::mem::align_of::<GpuDeviceInfo>(), 64);
    }

    // ========================================================================
    // Default Implementations
    // ========================================================================

    #[test]
    fn test_defaults() {
        let _flags: MemoryFlags = Default::default();
        let _sub: SubmissionId = Default::default();
        let _fence: FenceHandle = Default::default();
        let _status: FirmwareStatus = Default::default();
        let _info: GpuDeviceInfo = Default::default();
    }

    // ========================================================================
    // Display Implementations
    // ========================================================================

    #[test]
    #[cfg(feature = "std")]
    fn test_display_implementations() {
        use std::string::ToString;

        let queue = QueueType::Graphics;
        assert!(!queue.to_string().is_empty());

        let sub = SubmissionId::new(1, 100);
        assert!(sub.to_string().contains("Q1"));

        let fence = FenceHandle::new(0, 1, 50);
        assert!(fence.to_string().contains("Fence"));

        let fw_type = FirmwareType::GuC;
        assert_eq!(fw_type.to_string(), "GuC");

        let status = FirmwareStatus::Running;
        assert_eq!(status.to_string(), "Running");
    }
}
