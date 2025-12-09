//! Linux HAL - i915 Driver Integration Architecture
//!
//! Core traits and error types for Linux kernel interface integration.
//! Connects GPU HAL capsules to /dev/dri and i915 driver.
//!
//! # Design
//!
//! **Tier**: T1 Atomic + T8 Network (kernel IPC via ioctl)
//! **Portability**: Linux-only (feature-gated: `linux-gpu`)
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                   Linux HAL Integration Layer                    │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                  │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
//! │  │  LinuxPci    │  │  LinuxDrm    │  │  LinuxKms    │          │
//! │  │  Access      │  │  Access      │  │  Access      │          │
//! │  │  (sysfs)     │  │  (ioctl)     │  │  (modesetting)│         │
//! │  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
//! │         │                  │                  │                  │
//! │         ▼                  ▼                  ▼                  │
//! │  ┌──────────────────────────────────────────────────────────┐  │
//! │  │              IntelGpuDevice                               │  │
//! │  │  (Unified device wrapper for i915 driver)                │  │
//! │  └──────────────────────────────────────────────────────────┘  │
//! │         │                  │                  │                  │
//! │         ▼                  ▼                  ▼                  │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
//! │  │ MmioRegion   │  │ DmaBuffer    │  │ PageTable    │          │
//! │  │ Capsule      │  │ Capsule      │  │ Capsule      │          │
//! │  └──────────────┘  └──────────────┘  └──────────────┘          │
//! │                                                                  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # UCE34 Compliance
//!
//! - **Q10**: T1 Atomic (lockfree coordination) + T8 Network (kernel IPC)
//! - **Q33**: ComputationalCapsule patterns (generation counters, cache alignment)
//! - **Q34**: Audit trail design (ioctl logging, error tracking)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_DRM_FD_VALID`: DRM file descriptor is valid after successful open
//! - `#ASSUME_IOCTL_SAFE`: ioctl calls are serialized by kernel (no user-space races)
//! - `#ASSUME_MMIO_MAPPED`: MMIO region mapped correctly via kernel BAR mapping
//! - `#ASSUME_GEM_HANDLE_VALID`: GEM handle valid until gem_close() called
//!
//! # Examples
//!
//! ```ignore
//! use atomic_capsule::gpu::hal::linux_hal::{IntelGpuDevice, LinuxHalError};
//!
//! // Open Intel GPU device
//! let device = IntelGpuDevice::open()?;
//!
//! // Get device info
//! let version = device.get_version()?;
//! println!("DRM version: {}.{}.{}", version.major, version.minor, version.patch);
//!
//! // Create GEM buffer
//! let handle = device.gem_create(4096)?;
//!
//! // Map buffer for CPU access
//! let ptr = device.gem_mmap(handle, 0, 4096)?;
//!
//! // Cleanup
//! device.gem_close(handle)?;
//! ```

use core::fmt;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// ============================================================================
// Linux HAL Error Types
// ============================================================================

/// Linux HAL error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxHalError {
    /// Device not found (no Intel GPU in /dev/dri)
    DeviceNotFound,
    /// Permission denied (need root or video group)
    PermissionDenied,
    /// Device already open
    DeviceAlreadyOpen,
    /// Device not open
    DeviceNotOpen,
    /// Invalid device path
    InvalidDevicePath,
    /// DRM version query failed
    VersionQueryFailed,
    /// Unsupported DRM version
    UnsupportedDrmVersion,
    /// ioctl failed with errno
    IoctlFailed(i32),
    /// GEM create failed
    GemCreateFailed(i32),
    /// GEM close failed
    GemCloseFailed(i32),
    /// GEM mmap failed
    GemMmapFailed(i32),
    /// GEM set domain failed
    GemSetDomainFailed(i32),
    /// BAR mapping failed
    BarMappingFailed(u8),
    /// BAR unmapping failed
    BarUnmappingFailed(u8),
    /// Invalid BAR index (0-5 valid)
    InvalidBarIndex(u8),
    /// PCI config read failed
    PciConfigReadFailed(u16),
    /// PCI config write failed
    PciConfigWriteFailed(u16),
    /// Invalid PCI config offset
    InvalidPciConfigOffset(u16),
    /// KMS mode set failed
    KmsModeFailed,
    /// KMS connector not found
    KmsConnectorNotFound(u32),
    /// KMS CRTC not found
    KmsCrtcNotFound(u32),
    /// Page flip failed
    PageFlipFailed(i32),
    /// Buffer allocation failed
    BufferAllocationFailed,
    /// Invalid GEM handle
    InvalidGemHandle(u32),
    /// Timeout expired
    TimeoutExpired,
    /// Out of memory
    OutOfMemory,
    /// Device removed (hot-unplug)
    DeviceRemoved,
    /// Driver not loaded
    DriverNotLoaded,
    /// Feature not supported
    FeatureNotSupported,
    /// Internal error
    InternalError,
}

impl fmt::Display for LinuxHalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeviceNotFound => write!(f, "Intel GPU device not found in /dev/dri"),
            Self::PermissionDenied => write!(f, "Permission denied (need root or video group)"),
            Self::DeviceAlreadyOpen => write!(f, "Device already open"),
            Self::DeviceNotOpen => write!(f, "Device not open"),
            Self::InvalidDevicePath => write!(f, "Invalid device path"),
            Self::VersionQueryFailed => write!(f, "DRM version query failed"),
            Self::UnsupportedDrmVersion => write!(f, "Unsupported DRM version"),
            Self::IoctlFailed(errno) => write!(f, "ioctl failed: errno {}", errno),
            Self::GemCreateFailed(errno) => write!(f, "GEM create failed: errno {}", errno),
            Self::GemCloseFailed(errno) => write!(f, "GEM close failed: errno {}", errno),
            Self::GemMmapFailed(errno) => write!(f, "GEM mmap failed: errno {}", errno),
            Self::GemSetDomainFailed(errno) => write!(f, "GEM set domain failed: errno {}", errno),
            Self::BarMappingFailed(bar) => write!(f, "BAR {} mapping failed", bar),
            Self::BarUnmappingFailed(bar) => write!(f, "BAR {} unmapping failed", bar),
            Self::InvalidBarIndex(bar) => write!(f, "Invalid BAR index: {} (valid: 0-5)", bar),
            Self::PciConfigReadFailed(offset) => {
                write!(f, "PCI config read failed at offset 0x{:x}", offset)
            }
            Self::PciConfigWriteFailed(offset) => {
                write!(f, "PCI config write failed at offset 0x{:x}", offset)
            }
            Self::InvalidPciConfigOffset(offset) => {
                write!(f, "Invalid PCI config offset: 0x{:x}", offset)
            }
            Self::KmsModeFailed => write!(f, "KMS mode set failed"),
            Self::KmsConnectorNotFound(id) => write!(f, "KMS connector {} not found", id),
            Self::KmsCrtcNotFound(id) => write!(f, "KMS CRTC {} not found", id),
            Self::PageFlipFailed(errno) => write!(f, "Page flip failed: errno {}", errno),
            Self::BufferAllocationFailed => write!(f, "Buffer allocation failed"),
            Self::InvalidGemHandle(handle) => write!(f, "Invalid GEM handle: {}", handle),
            Self::TimeoutExpired => write!(f, "Operation timeout expired"),
            Self::OutOfMemory => write!(f, "Out of memory"),
            Self::DeviceRemoved => write!(f, "Device removed (hot-unplug)"),
            Self::DriverNotLoaded => write!(f, "i915 driver not loaded"),
            Self::FeatureNotSupported => write!(f, "Feature not supported by hardware"),
            Self::InternalError => write!(f, "Internal error"),
        }
    }
}

/// Result type for Linux HAL operations
pub type LinuxHalResult<T> = Result<T, LinuxHalError>;

// ============================================================================
// DRM Types
// ============================================================================

/// DRM version information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrmVersion {
    /// Major version
    pub major: u32,
    /// Minor version
    pub minor: u32,
    /// Patchlevel
    pub patch: u32,
}

impl DrmVersion {
    /// Create new DRM version
    #[inline]
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }

    /// Check if version is at least (major, minor, patch)
    #[inline]
    pub const fn at_least(&self, major: u32, minor: u32, patch: u32) -> bool {
        if self.major > major {
            return true;
        }
        if self.major < major {
            return false;
        }
        if self.minor > minor {
            return true;
        }
        if self.minor < minor {
            return false;
        }
        self.patch >= patch
    }
}

/// DRM capabilities
#[derive(Debug, Clone, Copy)]
pub struct DrmCapabilities {
    /// Supports dumb buffers
    pub dumb_buffer: bool,
    /// Supports vblank high-crtc
    pub vblank_high_crtc: bool,
    /// Supports async page flip
    pub async_page_flip: bool,
    /// Supports cursor width/height
    pub cursor_width: u32,
    pub cursor_height: u32,
    /// Supports prime fd export/import
    pub prime_export: bool,
    pub prime_import: bool,
    /// Supports monotonic timestamp
    pub timestamp_monotonic: bool,
    /// Supports atomic modesetting
    pub atomic_async_page_flip: bool,
    /// Supports syncobj timeline
    pub syncobj_timeline: bool,
}

impl Default for DrmCapabilities {
    fn default() -> Self {
        Self {
            dumb_buffer: false,
            vblank_high_crtc: false,
            async_page_flip: false,
            cursor_width: 64,
            cursor_height: 64,
            prime_export: false,
            prime_import: false,
            timestamp_monotonic: false,
            atomic_async_page_flip: false,
            syncobj_timeline: false,
        }
    }
}

// ============================================================================
// GEM Types
// ============================================================================

/// GEM buffer object handle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GemHandle(pub u32);

impl GemHandle {
    /// Create new GEM handle
    #[inline]
    pub const fn new(handle: u32) -> Self {
        Self(handle)
    }

    /// Get raw handle value
    #[inline]
    pub const fn raw(&self) -> u32 {
        self.0
    }

    /// Check if handle is valid (non-zero)
    #[inline]
    pub const fn is_valid(&self) -> bool {
        self.0 != 0
    }
}

/// GEM buffer domain for cache coherency
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GemDomain {
    /// CPU domain (coherent with CPU caches)
    Cpu = 0x01,
    /// GTT domain (coherent with GPU via GTT)
    Gtt = 0x02,
    /// Render domain (GPU render pipeline)
    Render = 0x04,
    /// Sampler domain (GPU texture sampler)
    Sampler = 0x08,
    /// Display domain (scanout buffer)
    Display = 0x10,
}

/// GEM memory region (Gen12+)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum GemMemoryClass {
    /// System memory (DDR)
    System = 0,
    /// Device memory (VRAM on discrete GPUs)
    Device = 1,
}

// ============================================================================
// Traits: Platform-Specific Abstractions
// ============================================================================

/// PCIe device access via sysfs/ioctl
///
/// Connects to PciDeviceCapsule for lockfree coordination.
/// Linux implementation uses /sys/bus/pci/devices/.../config
pub trait LinuxPciAccess: Send + Sync {
    /// Read 32-bit PCI config space register
    ///
    /// # Arguments
    /// * `offset` - Config space offset (0x00-0xFF for standard, 0x100+ for extended)
    ///
    /// # Performance
    /// ~100μs (sysfs file I/O)
    fn read_config(&self, offset: u16) -> LinuxHalResult<u32>;

    /// Write 32-bit PCI config space register
    ///
    /// # Arguments
    /// * `offset` - Config space offset
    /// * `value` - 32-bit value to write
    ///
    /// # Performance
    /// ~100μs (sysfs file I/O)
    fn write_config(&self, offset: u16, value: u32) -> LinuxHalResult<()>;

    /// Map PCIe BAR to virtual address space
    ///
    /// # Arguments
    /// * `bar_index` - BAR index (0-5)
    ///
    /// # Returns
    /// Virtual address of mapped BAR region
    ///
    /// # Safety
    /// Caller must ensure BAR is not already mapped and is valid for this device.
    fn map_bar(&self, bar_index: u8) -> LinuxHalResult<*mut u8>;

    /// Unmap PCIe BAR from virtual address space
    ///
    /// # Arguments
    /// * `bar_index` - BAR index (0-5)
    fn unmap_bar(&self, bar_index: u8) -> LinuxHalResult<()>;

    /// Get BAR size in bytes
    ///
    /// # Arguments
    /// * `bar_index` - BAR index (0-5)
    fn get_bar_size(&self, bar_index: u8) -> LinuxHalResult<usize>;
}

/// DRM device access
///
/// Connects to DmaBufferCapsule for buffer management.
/// Linux implementation uses /dev/dri/cardN or /dev/dri/renderD128
pub trait LinuxDrmAccess: Send + Sync {
    /// Open DRM device
    ///
    /// # Arguments
    /// * `path` - Device path (e.g., "/dev/dri/card0")
    ///
    /// # Performance
    /// ~1ms (file open + version query)
    fn open_device(path: &str) -> LinuxHalResult<Self>
    where
        Self: Sized;

    /// Close DRM device
    fn close_device(&self) -> LinuxHalResult<()>;

    /// Get DRM version
    fn get_version(&self) -> LinuxHalResult<DrmVersion>;

    /// Get DRM capabilities
    fn get_capabilities(&self) -> LinuxHalResult<DrmCapabilities>;

    /// Execute raw ioctl
    ///
    /// # Safety
    /// Caller must ensure `arg` points to valid memory for the ioctl request.
    unsafe fn ioctl_raw(&self, request: u64, arg: *mut u8) -> LinuxHalResult<()>;
}

/// GEM buffer object management
///
/// Graphics Execution Manager buffer operations.
/// Connects to DmaBufferCapsule for buffer lifetime tracking.
pub trait LinuxGemAccess: Send + Sync {
    /// Create GEM buffer object
    ///
    /// # Arguments
    /// * `size` - Buffer size in bytes (page-aligned)
    ///
    /// # Returns
    /// GEM handle for the created buffer
    ///
    /// # Performance
    /// ~50μs (kernel allocation + GTT entry)
    fn gem_create(&self, size: u64) -> LinuxHalResult<GemHandle>;

    /// Close GEM buffer object
    ///
    /// # Arguments
    /// * `handle` - GEM handle to close
    ///
    /// # Performance
    /// ~10μs (kernel reference drop)
    fn gem_close(&self, handle: GemHandle) -> LinuxHalResult<()>;

    /// Map GEM buffer for CPU access
    ///
    /// # Arguments
    /// * `handle` - GEM handle
    /// * `offset` - Offset into buffer
    /// * `size` - Size to map
    ///
    /// # Returns
    /// Virtual address of mapped region
    ///
    /// # Performance
    /// ~100μs (kernel mmap + TLB flush)
    fn gem_mmap(&self, handle: GemHandle, offset: u64, size: u64) -> LinuxHalResult<*mut u8>;

    /// Unmap GEM buffer
    ///
    /// # Arguments
    /// * `ptr` - Virtual address from gem_mmap
    /// * `size` - Size that was mapped
    fn gem_munmap(&self, ptr: *mut u8, size: u64) -> LinuxHalResult<()>;

    /// Set GEM buffer domain (cache coherency)
    ///
    /// # Arguments
    /// * `handle` - GEM handle
    /// * `read_domains` - Domains for read access
    /// * `write_domain` - Domain for write access
    ///
    /// # Performance
    /// ~50μs (cache flush + domain transition)
    fn gem_set_domain(
        &self,
        handle: GemHandle,
        read_domains: u32,
        write_domain: u32,
    ) -> LinuxHalResult<()>;

    /// Wait for GEM buffer to become idle
    ///
    /// # Arguments
    /// * `handle` - GEM handle
    /// * `timeout_ns` - Timeout in nanoseconds
    fn gem_wait(&self, handle: GemHandle, timeout_ns: i64) -> LinuxHalResult<()>;
}

// ============================================================================
// i915-Specific Types
// ============================================================================

/// Intel GPU generation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum IntelGpuGen {
    /// Unknown generation
    Unknown = 0,
    /// Gen7 (Ivy Bridge, Haswell)
    Gen7 = 7,
    /// Gen8 (Broadwell)
    Gen8 = 8,
    /// Gen9 (Skylake, Kaby Lake, Coffee Lake)
    Gen9 = 9,
    /// Gen11 (Ice Lake)
    Gen11 = 11,
    /// Gen12 (Tiger Lake, Rocket Lake, Alder Lake)
    Gen12 = 12,
    /// Gen12.5 (DG1, DG2)
    Gen12p5 = 13,
    /// Xe HPG (Arc Alchemist)
    XeHpg = 14,
    /// Xe LPG (Meteor Lake)
    XeLpg = 15,
}

impl IntelGpuGen {
    /// Check if generation supports EU/subslice info
    #[inline]
    pub const fn supports_eu_info(&self) -> bool {
        (*self as u8) >= 9
    }

    /// Check if generation supports local memory (VRAM)
    #[inline]
    pub const fn supports_local_memory(&self) -> bool {
        (*self as u8) >= 12
    }

    /// Check if generation supports GuC submission
    #[inline]
    pub const fn supports_guc_submission(&self) -> bool {
        (*self as u8) >= 11
    }
}

/// Intel GPU engine class
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum I915EngineClass {
    /// Render engine (3D pipeline)
    Render = 0,
    /// Copy engine (BLT)
    Copy = 1,
    /// Video decode engine
    Video = 2,
    /// Video enhancement engine
    VideoEnhance = 3,
    /// Compute engine (Gen12+)
    Compute = 4,
}

/// Intel GPU context parameters
#[derive(Debug, Clone, Copy)]
pub struct I915ContextParam {
    /// Context handle
    pub ctx_id: u32,
    /// Parameter size
    pub size: u32,
    /// Parameter ID
    pub param: u64,
    /// Parameter value
    pub value: u64,
}

// ============================================================================
// Coordination State (T1 Atomic)
// ============================================================================

/// Linux HAL coordination state
///
/// Lockfree state tracking for Linux HAL operations.
/// Uses DualAtomicU64 pattern for 128-bit atomic snapshot.
#[repr(C, align(128))]
pub struct LinuxHalState {
    /// Operation counter (monotonic increment)
    operation_count: AtomicU64,
    /// Error counter (increments on any error)
    error_count: AtomicU64,
    /// Last error code (LinuxHalError as u32)
    last_error: AtomicU32,
    /// Generation counter (ABA prevention)
    generation: AtomicU32,
    /// Active GEM handle count
    active_gem_handles: AtomicU32,
    /// Active BAR mapping count
    active_bar_maps: AtomicU32,
    /// Flags (bit 0: device open, bit 1: master, bit 2: atomic enabled)
    flags: AtomicU32,
    /// Reserved for future use
    _reserved: AtomicU32,
    /// Padding to 128B cache line
    _padding: [u8; 80],
}

impl LinuxHalState {
    /// Create new state
    #[inline]
    pub const fn new() -> Self {
        Self {
            operation_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            last_error: AtomicU32::new(0),
            generation: AtomicU32::new(0),
            active_gem_handles: AtomicU32::new(0),
            active_bar_maps: AtomicU32::new(0),
            flags: AtomicU32::new(0),
            _reserved: AtomicU32::new(0),
            _padding: [0u8; 80],
        }
    }

    /// Increment operation count
    #[inline]
    pub fn increment_operation(&self) {
        self.operation_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record error
    ///
    /// Stores a simplified error code (discriminant) for state tracking.
    /// Use error_count() + external logging for full error details.
    #[inline]
    pub fn record_error(&self, error: LinuxHalError) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
        // Store simplified error code (discriminant index)
        // #ASSUME_ERROR_CODE_FITS: LinuxHalError has <256 variants, fits in u32
        let error_code = match error {
            LinuxHalError::DeviceNotFound => 0,
            LinuxHalError::PermissionDenied => 1,
            LinuxHalError::DeviceAlreadyOpen => 2,
            LinuxHalError::DeviceNotOpen => 3,
            LinuxHalError::InvalidDevicePath => 4,
            LinuxHalError::VersionQueryFailed => 5,
            LinuxHalError::UnsupportedDrmVersion => 6,
            LinuxHalError::IoctlFailed(_) => 7,
            LinuxHalError::GemCreateFailed(_) => 8,
            LinuxHalError::GemCloseFailed(_) => 9,
            LinuxHalError::GemMmapFailed(_) => 10,
            LinuxHalError::GemSetDomainFailed(_) => 11,
            LinuxHalError::BarMappingFailed(_) => 12,
            LinuxHalError::BarUnmappingFailed(_) => 13,
            LinuxHalError::InvalidBarIndex(_) => 14,
            LinuxHalError::PciConfigReadFailed(_) => 15,
            LinuxHalError::PciConfigWriteFailed(_) => 16,
            LinuxHalError::InvalidPciConfigOffset(_) => 17,
            LinuxHalError::KmsModeFailed => 18,
            LinuxHalError::KmsConnectorNotFound(_) => 19,
            LinuxHalError::KmsCrtcNotFound(_) => 20,
            LinuxHalError::PageFlipFailed(_) => 21,
            LinuxHalError::BufferAllocationFailed => 22,
            LinuxHalError::InvalidGemHandle(_) => 23,
            LinuxHalError::TimeoutExpired => 24,
            LinuxHalError::OutOfMemory => 25,
            LinuxHalError::DeviceRemoved => 26,
            LinuxHalError::DriverNotLoaded => 27,
            LinuxHalError::FeatureNotSupported => 28,
            LinuxHalError::InternalError => 29,
        };
        self.last_error.store(error_code, Ordering::Release);
    }

    /// Get operation count
    #[inline]
    pub fn operation_count(&self) -> u64 {
        self.operation_count.load(Ordering::Relaxed)
    }

    /// Get error count
    #[inline]
    pub fn error_count(&self) -> u64 {
        self.error_count.load(Ordering::Relaxed)
    }

    /// Increment GEM handle count
    #[inline]
    pub fn increment_gem_handles(&self) {
        self.active_gem_handles.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement GEM handle count
    #[inline]
    pub fn decrement_gem_handles(&self) {
        self.active_gem_handles.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get active GEM handle count
    #[inline]
    pub fn active_gem_handles(&self) -> u32 {
        self.active_gem_handles.load(Ordering::Relaxed)
    }

    /// Set device open flag
    #[inline]
    pub fn set_device_open(&self, open: bool) {
        if open {
            self.flags.fetch_or(0x01, Ordering::Release);
        } else {
            self.flags.fetch_and(!0x01, Ordering::Release);
        }
    }

    /// Check if device is open
    #[inline]
    pub fn is_device_open(&self) -> bool {
        (self.flags.load(Ordering::Acquire) & 0x01) != 0
    }
}

// ============================================================================
// Compile-time Verification
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linux_hal_state_size() {
        assert_eq!(
            core::mem::size_of::<LinuxHalState>(),
            128,
            "LinuxHalState must be 128B"
        );
        assert_eq!(
            core::mem::align_of::<LinuxHalState>(),
            128,
            "LinuxHalState must be 128B-aligned"
        );
    }

    #[test]
    fn test_drm_version_comparison() {
        let v = DrmVersion::new(1, 6, 0);
        assert!(v.at_least(1, 5, 0));
        assert!(v.at_least(1, 6, 0));
        assert!(!v.at_least(1, 7, 0));
        assert!(!v.at_least(2, 0, 0));
    }

    #[test]
    fn test_gem_handle() {
        let h1 = GemHandle::new(42);
        assert_eq!(h1.raw(), 42);
        assert!(h1.is_valid());

        let h2 = GemHandle::new(0);
        assert!(!h2.is_valid());
    }

    #[test]
    fn test_intel_gpu_gen_capabilities() {
        assert!(!IntelGpuGen::Gen8.supports_eu_info());
        assert!(IntelGpuGen::Gen9.supports_eu_info());
        assert!(!IntelGpuGen::Gen9.supports_local_memory());
        assert!(IntelGpuGen::Gen12.supports_local_memory());
    }

    #[test]
    fn test_error_display() {
        let err = LinuxHalError::DeviceNotFound;
        assert!(err.to_string().contains("not found"));

        let err = LinuxHalError::IoctlFailed(22);
        assert!(err.to_string().contains("22"));
    }

    #[test]
    fn test_linux_hal_state_operations() {
        let state = LinuxHalState::new();

        assert_eq!(state.operation_count(), 0);
        state.increment_operation();
        assert_eq!(state.operation_count(), 1);

        assert!(!state.is_device_open());
        state.set_device_open(true);
        assert!(state.is_device_open());
        state.set_device_open(false);
        assert!(!state.is_device_open());
    }
}
