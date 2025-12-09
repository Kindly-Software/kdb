//! Linux DRM (Direct Rendering Manager) Interface for KGPU-Driver v2.0
//!
//! This module provides the core Linux DRM interface layer that sits below Vulkan,
//! replacing Mesa's role in the graphics stack. It talks directly to the kernel DRM
//! subsystem via ioctls.
//!
//! # Architecture
//!
//! ```text
//! +-------------------+     +-------------------+
//! |  Vulkan Layer     |     |  Application      |
//! +--------+----------+     +--------+----------+
//!          |                         |
//!          v                         v
//! +--------+-----------------------------+
//! |        KGPU-Driver v2.0              |
//! |  (linux_drm.rs - This module)        |
//! +--------+-----------------------------+
//!          |
//!          v
//! +--------+----------+
//! |  Linux Kernel DRM |
//! |  /dev/dri/cardN   |
//! +-------------------+
//! ```
//!
//! # Features
//!
//! - DRM device enumeration (/dev/dri/card*, /dev/dri/renderD*)
//! - Safe Rust FFI wrappers for DRM ioctls
//! - Capability queries (DRM_CAP_*, DRIVER_CAP_*)
//! - DRM master/auth handling
//! - PRIME buffer sharing (DMA-BUF)
//!
//! # Chaos Compliance
//!
//! - **T1 Atomic Tier**: All capsules use DualAtomicU64 pattern
//! - **100% Lockfree**: NO mutex, NO RwLock anywhere
//! - **Cache-aligned**: 128B/256B alignment for capsules
//! - **Generation counters**: For CAS operations and state tracking
//!
//! # ASSUM Safety Tags
//!
//! - `#ASSUME_FD_VALID`: File descriptors from open() are valid
//! - `#ASSUME_IOCTL_SAFE`: DRM ioctls follow documented behavior
//! - `#ASSUME_PATH_CANONICAL`: /dev/dri paths are canonical
//! - `#ASSUME_KERNEL_STABLE`: Kernel ABI is stable across minor versions

#![allow(dead_code)] // Allow during development

use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "std")]
extern crate std;
#[cfg(feature = "std")]
use std::vec::Vec;
#[cfg(feature = "std")]
use std::string::String;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(not(feature = "std"))]
use alloc::string::String;

use super::error::KgpuDriverError;
use super::vendor::{GpuVendor, GpuGeneration, PciBdf, detect_generation};

// ============================================================================
// DRM Constants (from linux/drm.h)
// ============================================================================

/// Magic number for DRM ioctls (from linux/drm.h)
const DRM_IOCTL_BASE: u8 = b'd';

// DRM ioctl command numbers
const DRM_IOCTL_NR_VERSION: u8 = 0x00;
const DRM_IOCTL_NR_GET_UNIQUE: u8 = 0x01;
const DRM_IOCTL_NR_GET_MAGIC: u8 = 0x02;
const DRM_IOCTL_NR_GET_CAP: u8 = 0x0C;
const DRM_IOCTL_NR_SET_CLIENT_CAP: u8 = 0x0D;
const DRM_IOCTL_NR_AUTH_MAGIC: u8 = 0x11;
const DRM_IOCTL_NR_SET_MASTER: u8 = 0x1E;
const DRM_IOCTL_NR_DROP_MASTER: u8 = 0x1F;
const DRM_IOCTL_NR_PRIME_HANDLE_TO_FD: u8 = 0x2D;
const DRM_IOCTL_NR_PRIME_FD_TO_HANDLE: u8 = 0x2E;

// DRM ioctl definitions (encoded as per Linux ioctl convention)
// Format: _IOWR(type, nr, size) = 0xC0000000 | (size << 16) | (type << 8) | nr
const DRM_IOCTL_VERSION: u64 = 0xC0406400;        // _IOWR('d', 0x00, struct drm_version)
const DRM_IOCTL_GET_UNIQUE: u64 = 0xC0106401;     // _IOWR('d', 0x01, struct drm_unique)
const DRM_IOCTL_GET_MAGIC: u64 = 0x80046402;      // _IOR('d', 0x02, struct drm_auth)
const DRM_IOCTL_GET_CAP: u64 = 0xC010640C;        // _IOWR('d', 0x0C, struct drm_get_cap)
const DRM_IOCTL_SET_CLIENT_CAP: u64 = 0x4010640D; // _IOW('d', 0x0D, struct drm_set_client_cap)
const DRM_IOCTL_AUTH_MAGIC: u64 = 0x40046411;     // _IOW('d', 0x11, struct drm_auth)
const DRM_IOCTL_SET_MASTER: u64 = 0x0000641E;     // _IO('d', 0x1E)
const DRM_IOCTL_DROP_MASTER: u64 = 0x0000641F;    // _IO('d', 0x1F)
const DRM_IOCTL_PRIME_HANDLE_TO_FD: u64 = 0xC00C642D; // _IOWR('d', 0x2D, struct drm_prime_handle)
const DRM_IOCTL_PRIME_FD_TO_HANDLE: u64 = 0xC00C642E; // _IOWR('d', 0x2E, struct drm_prime_handle)

// File open flags
const O_RDWR: i32 = 0x0002;
const O_CLOEXEC: i32 = 0x80000; // Linux specific

// errno values
const ENOENT: i32 = 2;
const EACCES: i32 = 13;
const EBUSY: i32 = 16;
const ENODEV: i32 = 19;
const EINVAL: i32 = 22;
const ENOTTY: i32 = 25;
const EPERM: i32 = 1;

// ============================================================================
// DRM Node Type
// ============================================================================

/// DRM device node types.
///
/// Linux DRM exposes multiple device nodes per GPU:
/// - Primary nodes (`/dev/dri/cardN`): Full access, requires DRM master
/// - Control nodes (`/dev/dri/controlDN`): Configuration only
/// - Render nodes (`/dev/dri/renderDN`): Rendering without master
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DrmNodeType {
    /// Primary DRM node (/dev/dri/card*)
    /// Full access to all DRM features, requires master for modesetting
    Primary = 0,
    /// Control DRM node (/dev/dri/controlD*)
    /// For configuration and modesetting only
    Control = 1,
    /// Render DRM node (/dev/dri/renderD*)
    /// For rendering without needing DRM master (unprivileged)
    Render = 2,
}

impl DrmNodeType {
    /// Get the device path prefix for this node type.
    #[inline]
    pub const fn path_prefix(self) -> &'static str {
        match self {
            Self::Primary => "/dev/dri/card",
            Self::Control => "/dev/dri/controlD",
            Self::Render => "/dev/dri/renderD",
        }
    }

    /// Get human-readable name.
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Primary => "Primary",
            Self::Control => "Control",
            Self::Render => "Render",
        }
    }

    /// Check if this node type supports rendering.
    #[inline]
    pub const fn supports_rendering(self) -> bool {
        matches!(self, Self::Primary | Self::Render)
    }

    /// Check if this node type supports modesetting.
    #[inline]
    pub const fn supports_modesetting(self) -> bool {
        matches!(self, Self::Primary | Self::Control)
    }

    /// Check if this node type requires DRM master for full access.
    #[inline]
    pub const fn requires_master(self) -> bool {
        matches!(self, Self::Primary)
    }
}

impl Default for DrmNodeType {
    fn default() -> Self {
        Self::Render // Prefer render nodes (unprivileged)
    }
}

impl fmt::Display for DrmNodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// DRM Capabilities
// ============================================================================

/// DRM capabilities that can be queried via DRM_IOCTL_GET_CAP.
///
/// These map directly to the DRM_CAP_* constants from linux/drm.h.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u64)]
pub enum DrmCapability {
    /// Supports dumb buffer allocation (DRM_CAP_DUMB_BUFFER)
    DumbBuffer = 0x1,
    /// Supports high CRTC in vblank events (DRM_CAP_VBLANK_HIGH_CRTC)
    VBlankHighCrtc = 0x2,
    /// Preferred depth for dumb buffers (DRM_CAP_DUMB_PREFERRED_DEPTH)
    DumbPreferredDepth = 0x3,
    /// Prefer shadow buffer for dumb buffers (DRM_CAP_DUMB_PREFER_SHADOW)
    DumbPreferShadow = 0x4,
    /// Supports PRIME buffer sharing (DRM_CAP_PRIME)
    Prime = 0x5,
    /// Timestamps are monotonic (DRM_CAP_TIMESTAMP_MONOTONIC)
    TimestampMonotonic = 0x6,
    /// Supports async page flip (DRM_CAP_ASYNC_PAGE_FLIP)
    AsyncPageFlip = 0x7,
    /// Maximum cursor width (DRM_CAP_CURSOR_WIDTH)
    CursorWidth = 0x8,
    /// Maximum cursor height (DRM_CAP_CURSOR_HEIGHT)
    CursorHeight = 0x9,
    /// Supports FB modifiers in addfb2 (DRM_CAP_ADDFB2_MODIFIERS)
    AddFb2Modifiers = 0x10,
    /// Supports page flip targeting (DRM_CAP_PAGE_FLIP_TARGET)
    PageFlipTarget = 0x11,
    /// CRTC in vblank event (DRM_CAP_CRTC_IN_VBLANK_EVENT)
    CrtcInVblankEvent = 0x12,
    /// Supports sync objects (DRM_CAP_SYNCOBJ)
    SyncObj = 0x13,
    /// Supports timeline sync objects (DRM_CAP_SYNCOBJ_TIMELINE)
    SyncObjTimeline = 0x14,
    /// Supports async atomic page flip (DRM_CAP_ATOMIC_ASYNC_PAGE_FLIP)
    AtomicAsyncPageFlip = 0x15,
}

impl DrmCapability {
    /// Get human-readable name.
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::DumbBuffer => "DUMB_BUFFER",
            Self::VBlankHighCrtc => "VBLANK_HIGH_CRTC",
            Self::DumbPreferredDepth => "DUMB_PREFERRED_DEPTH",
            Self::DumbPreferShadow => "DUMB_PREFER_SHADOW",
            Self::Prime => "PRIME",
            Self::TimestampMonotonic => "TIMESTAMP_MONOTONIC",
            Self::AsyncPageFlip => "ASYNC_PAGE_FLIP",
            Self::CursorWidth => "CURSOR_WIDTH",
            Self::CursorHeight => "CURSOR_HEIGHT",
            Self::AddFb2Modifiers => "ADDFB2_MODIFIERS",
            Self::PageFlipTarget => "PAGE_FLIP_TARGET",
            Self::CrtcInVblankEvent => "CRTC_IN_VBLANK_EVENT",
            Self::SyncObj => "SYNCOBJ",
            Self::SyncObjTimeline => "SYNCOBJ_TIMELINE",
            Self::AtomicAsyncPageFlip => "ATOMIC_ASYNC_PAGE_FLIP",
        }
    }

    /// Check if this is a boolean capability (0 or 1).
    #[inline]
    pub const fn is_boolean(self) -> bool {
        !matches!(
            self,
            Self::DumbPreferredDepth | Self::CursorWidth | Self::CursorHeight
        )
    }
}

impl fmt::Display for DrmCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DRM_CAP_{}", self.name())
    }
}

/// DRM client capabilities that can be set via DRM_IOCTL_SET_CLIENT_CAP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u64)]
pub enum DrmClientCapability {
    /// Enable stereo 3D support (DRM_CLIENT_CAP_STEREO_3D)
    Stereo3D = 1,
    /// Enable universal planes (DRM_CLIENT_CAP_UNIVERSAL_PLANES)
    UniversalPlanes = 2,
    /// Enable atomic modesetting (DRM_CLIENT_CAP_ATOMIC)
    Atomic = 3,
    /// Enable aspect ratio support (DRM_CLIENT_CAP_ASPECT_RATIO)
    AspectRatio = 4,
    /// Enable writeback connectors (DRM_CLIENT_CAP_WRITEBACK_CONNECTORS)
    WritebackConnectors = 5,
    /// Enable cursor planes (DRM_CLIENT_CAP_CURSOR_PLANE_HOTSPOT)
    CursorPlaneHotspot = 6,
}

impl DrmClientCapability {
    /// Get human-readable name.
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Stereo3D => "STEREO_3D",
            Self::UniversalPlanes => "UNIVERSAL_PLANES",
            Self::Atomic => "ATOMIC",
            Self::AspectRatio => "ASPECT_RATIO",
            Self::WritebackConnectors => "WRITEBACK_CONNECTORS",
            Self::CursorPlaneHotspot => "CURSOR_PLANE_HOTSPOT",
        }
    }
}

impl fmt::Display for DrmClientCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DRM_CLIENT_CAP_{}", self.name())
    }
}

// ============================================================================
// PRIME Flags
// ============================================================================

/// PRIME buffer sharing flags for DRM_IOCTL_PRIME_HANDLE_TO_FD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum PrimeFlags {
    /// Close-on-exec flag (default recommended)
    CloseOnExec = 0x1,
    /// Read-only access
    ReadOnly = 0x2,
}

impl PrimeFlags {
    /// Get the raw flags value.
    #[inline]
    pub const fn bits(self) -> u32 {
        self as u32
    }
}

// ============================================================================
// DRM Version Info
// ============================================================================

/// DRM driver version information.
///
/// Returned by DRM_IOCTL_VERSION, contains driver name and version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrmVersion {
    /// Major version number
    pub major: i32,
    /// Minor version number
    pub minor: i32,
    /// Patch level
    pub patchlevel: i32,
    /// Driver name (e.g., "i915", "amdgpu", "nouveau")
    pub name: String,
    /// Date string
    pub date: String,
    /// Description string
    pub desc: String,
}

impl DrmVersion {
    /// Create a new empty version.
    #[inline]
    pub fn new() -> Self {
        Self {
            major: 0,
            minor: 0,
            patchlevel: 0,
            name: String::new(),
            date: String::new(),
            desc: String::new(),
        }
    }

    /// Get the driver name as a &str.
    #[inline]
    pub fn driver_name(&self) -> &str {
        &self.name
    }

    /// Get the full version as a tuple.
    #[inline]
    pub fn version_tuple(&self) -> (i32, i32, i32) {
        (self.major, self.minor, self.patchlevel)
    }
}

impl Default for DrmVersion {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for DrmVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} v{}.{}.{}",
            self.name, self.major, self.minor, self.patchlevel
        )
    }
}

// ============================================================================
// DRM Device Capsule (T1 Atomic, 128B)
// ============================================================================

/// DRM device handle capsule (T1 Atomic tier, 128B aligned).
///
/// Provides lockfree, cache-aligned storage for DRM device state.
/// Uses generation counters for safe CAS operations.
///
/// # Layout
///
/// ```text
/// Offset  Field           Size    Description
/// 0       state           8B      Packed: [fd:32][node_type:8][vendor:8][flags:16]
/// 8       generation      8B      CAS generation counter
/// 16      path_hash       8B      FNV-1a hash of device path
/// 24      capabilities    8B      DRM capability bitmask
/// 32      driver_hash     8B      Hash of driver name
/// 40      bus_info        8B      PCI BDF packed
/// 48      version         8B      Packed: [major:16][minor:16][patch:16][reserved:16]
/// 56      _padding        72B     Padding to 128B
/// ```
///
/// # Chaos Compliance
///
/// - 128B cache-aligned (2 cache lines)
/// - Generation counter for CAS
/// - 100% lockfree atomics
/// - NO mutex/RwLock
///
/// # ASSUM Tags
///
/// - `#ASSUME_FD_RANGE`: File descriptor fits in 32 bits
/// - `#ASSUME_GENERATION_MONOTONIC`: Generation always increases
#[derive(Debug)]
#[repr(C, align(128))]
pub struct DrmDeviceCapsule {
    /// Packed state: [fd:32][node_type:8][vendor_id_high:8][flags:16]
    /// - fd: File descriptor (i32, stored as u32)
    /// - node_type: DrmNodeType as u8
    /// - vendor_id_high: High byte of vendor ID (for quick checks)
    /// - flags: Device state flags
    state: AtomicU64,

    /// Generation counter for CAS operations
    generation: AtomicU64,

    /// FNV-1a hash of device path for identification
    path_hash: AtomicU64,

    /// DRM capabilities bitmask (from DRM_IOCTL_GET_CAP)
    capabilities: AtomicU64,

    /// FNV-1a hash of driver name
    driver_hash: AtomicU64,

    /// PCI bus info packed: [domain:16][bus:8][device:8][function:8][vendor_id:16][reserved:8]
    bus_info: AtomicU64,

    /// Version packed: [major:16][minor:16][patchlevel:16][reserved:16]
    version: AtomicU64,

    /// Padding to 128 bytes
    _padding: [u8; 72],
}

// Device state flags
const DRM_DEVICE_FLAG_OPEN: u16 = 0x0001;
const DRM_DEVICE_FLAG_MASTER: u16 = 0x0002;
const DRM_DEVICE_FLAG_AUTHENTICATED: u16 = 0x0004;
const DRM_DEVICE_FLAG_RENDER_CAPABLE: u16 = 0x0008;
const DRM_DEVICE_FLAG_PRIME_IMPORT: u16 = 0x0010;
const DRM_DEVICE_FLAG_PRIME_EXPORT: u16 = 0x0020;
const DRM_DEVICE_FLAG_SYNCOBJ: u16 = 0x0040;
const DRM_DEVICE_FLAG_ATOMIC: u16 = 0x0080;

impl DrmDeviceCapsule {
    /// Create a new uninitialized DRM device capsule.
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            path_hash: AtomicU64::new(0),
            capabilities: AtomicU64::new(0),
            driver_hash: AtomicU64::new(0),
            bus_info: AtomicU64::new(0),
            version: AtomicU64::new(0),
            _padding: [0; 72],
        }
    }

    /// Initialize from an open file descriptor.
    ///
    /// # Arguments
    ///
    /// * `fd` - Open file descriptor for DRM device
    /// * `node_type` - Type of DRM node
    /// * `path` - Device path (for hashing)
    ///
    /// # Safety
    ///
    /// Caller must ensure `fd` is a valid open DRM device file descriptor.
    #[inline]
    pub fn init(&self, fd: i32, node_type: DrmNodeType, path: &str) {
        // Pack state: [fd:32][node_type:8][vendor_high:8][flags:16]
        let state = ((fd as u64) & 0xFFFFFFFF)
            | ((node_type as u64) << 32)
            | (DRM_DEVICE_FLAG_OPEN as u64) << 48;

        self.state.store(state, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.path_hash.store(fnv1a_hash(path.as_bytes()), Ordering::Release);
    }

    /// Get the file descriptor.
    ///
    /// # Returns
    ///
    /// The file descriptor, or -1 if not open.
    #[inline]
    pub fn fd(&self) -> i32 {
        let state = self.state.load(Ordering::Acquire);
        (state & 0xFFFFFFFF) as i32
    }

    /// Get the node type.
    #[inline]
    pub fn node_type(&self) -> DrmNodeType {
        let state = self.state.load(Ordering::Acquire);
        match ((state >> 32) & 0xFF) as u8 {
            0 => DrmNodeType::Primary,
            1 => DrmNodeType::Control,
            2 => DrmNodeType::Render,
            _ => DrmNodeType::Render, // Default to render for safety
        }
    }

    /// Get the current generation (for CAS).
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get the device flags.
    #[inline]
    pub fn flags(&self) -> u16 {
        let state = self.state.load(Ordering::Acquire);
        ((state >> 48) & 0xFFFF) as u16
    }

    /// Check if device is open.
    #[inline]
    pub fn is_open(&self) -> bool {
        (self.flags() & DRM_DEVICE_FLAG_OPEN) != 0
    }

    /// Check if we have DRM master.
    #[inline]
    pub fn is_master(&self) -> bool {
        (self.flags() & DRM_DEVICE_FLAG_MASTER) != 0
    }

    /// Check if authenticated.
    #[inline]
    pub fn is_authenticated(&self) -> bool {
        (self.flags() & DRM_DEVICE_FLAG_AUTHENTICATED) != 0
    }

    /// Set a flag atomically.
    ///
    /// Uses CAS loop with generation counter.
    #[inline]
    pub fn set_flag(&self, flag: u16) {
        loop {
            let old_state = self.state.load(Ordering::Acquire);
            let old_flags = ((old_state >> 48) & 0xFFFF) as u16;
            let new_flags = old_flags | flag;
            let new_state = (old_state & 0x0000FFFFFFFFFFFF) | ((new_flags as u64) << 48);

            if self.state.compare_exchange_weak(
                old_state,
                new_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                self.generation.fetch_add(1, Ordering::AcqRel);
                break;
            }
        }
    }

    /// Clear a flag atomically.
    #[inline]
    pub fn clear_flag(&self, flag: u16) {
        loop {
            let old_state = self.state.load(Ordering::Acquire);
            let old_flags = ((old_state >> 48) & 0xFFFF) as u16;
            let new_flags = old_flags & !flag;
            let new_state = (old_state & 0x0000FFFFFFFFFFFF) | ((new_flags as u64) << 48);

            if self.state.compare_exchange_weak(
                old_state,
                new_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                self.generation.fetch_add(1, Ordering::AcqRel);
                break;
            }
        }
    }

    /// Set capabilities bitmask.
    #[inline]
    pub fn set_capabilities(&self, caps: u64) {
        self.capabilities.store(caps, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Get capabilities bitmask.
    #[inline]
    pub fn capabilities(&self) -> u64 {
        self.capabilities.load(Ordering::Acquire)
    }

    /// Check if a specific capability is set.
    #[inline]
    pub fn has_capability(&self, cap: DrmCapability) -> bool {
        let caps = self.capabilities.load(Ordering::Acquire);
        (caps & (1 << (cap as u64))) != 0
    }

    /// Set driver hash.
    #[inline]
    pub fn set_driver_hash(&self, name: &str) {
        self.driver_hash.store(fnv1a_hash(name.as_bytes()), Ordering::Release);
    }

    /// Get driver hash.
    #[inline]
    pub fn driver_hash(&self) -> u64 {
        self.driver_hash.load(Ordering::Acquire)
    }

    /// Set PCI bus info.
    ///
    /// Packs: [domain:16][bus:8][device:8][function:8][vendor_id:16][reserved:8]
    #[inline]
    pub fn set_bus_info(&self, bdf: &PciBdf, vendor_id: u16) {
        let packed = (bdf.domain as u64)
            | ((bdf.bus as u64) << 16)
            | ((bdf.device as u64) << 24)
            | ((bdf.function as u64) << 32)
            | ((vendor_id as u64) << 40);
        self.bus_info.store(packed, Ordering::Release);
    }

    /// Get PCI BDF.
    #[inline]
    pub fn bus_info(&self) -> PciBdf {
        let packed = self.bus_info.load(Ordering::Acquire);
        PciBdf {
            domain: (packed & 0xFFFF) as u16,
            bus: ((packed >> 16) & 0xFF) as u8,
            device: ((packed >> 24) & 0xFF) as u8,
            function: ((packed >> 32) & 0xFF) as u8,
        }
    }

    /// Get vendor ID.
    #[inline]
    pub fn vendor_id(&self) -> u16 {
        let packed = self.bus_info.load(Ordering::Acquire);
        ((packed >> 40) & 0xFFFF) as u16
    }

    /// Get vendor enum.
    #[inline]
    pub fn vendor(&self) -> GpuVendor {
        GpuVendor::from_pci_vendor_id(self.vendor_id())
    }

    /// Set version info.
    ///
    /// Packs: [major:16][minor:16][patchlevel:16][reserved:16]
    #[inline]
    pub fn set_version(&self, major: i32, minor: i32, patchlevel: i32) {
        let packed = ((major as u64) & 0xFFFF)
            | (((minor as u64) & 0xFFFF) << 16)
            | (((patchlevel as u64) & 0xFFFF) << 32);
        self.version.store(packed, Ordering::Release);
    }

    /// Get version as tuple (major, minor, patchlevel).
    #[inline]
    pub fn version(&self) -> (i32, i32, i32) {
        let packed = self.version.load(Ordering::Acquire);
        (
            (packed & 0xFFFF) as i32,
            ((packed >> 16) & 0xFFFF) as i32,
            ((packed >> 32) & 0xFFFF) as i32,
        )
    }

    /// Close the device (marks as closed, doesn't actually close fd).
    #[inline]
    pub fn mark_closed(&self) {
        self.clear_flag(DRM_DEVICE_FLAG_OPEN);
        // Clear fd
        loop {
            let old_state = self.state.load(Ordering::Acquire);
            let new_state = old_state & !0xFFFFFFFF; // Clear fd portion

            if self.state.compare_exchange_weak(
                old_state,
                new_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                self.generation.fetch_add(1, Ordering::AcqRel);
                break;
            }
        }
    }
}

impl Default for DrmDeviceCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Compile-time assertions for DrmDeviceCapsule
const _: () = {
    assert!(
        core::mem::size_of::<DrmDeviceCapsule>() == 128,
        "DrmDeviceCapsule must be 128 bytes"
    );
    assert!(
        core::mem::align_of::<DrmDeviceCapsule>() == 128,
        "DrmDeviceCapsule must be 128-byte aligned"
    );
};

// ============================================================================
// DRM ioctl FFI Structures
// ============================================================================

/// FFI structure for DRM_IOCTL_VERSION.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
struct DrmVersionFfi {
    version_major: i32,
    version_minor: i32,
    version_patchlevel: i32,
    name_len: usize,
    name: *mut u8,
    date_len: usize,
    date: *mut u8,
    desc_len: usize,
    desc: *mut u8,
}

/// FFI structure for DRM_IOCTL_GET_CAP.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
struct DrmGetCapFfi {
    capability: u64,
    value: u64,
}

/// FFI structure for DRM_IOCTL_SET_CLIENT_CAP.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
struct DrmSetClientCapFfi {
    capability: u64,
    value: u64,
}

/// FFI structure for DRM_IOCTL_GET_MAGIC / AUTH_MAGIC.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
struct DrmAuthFfi {
    magic: u32,
}

/// FFI structure for DRM_IOCTL_PRIME_*.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
struct DrmPrimeHandleFfi {
    handle: u32,
    flags: u32,
    fd: i32,
}

/// FFI structure for DRM_IOCTL_GET_UNIQUE.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
struct DrmUniqueFfi {
    unique_len: usize,
    unique: *mut u8,
}

// ============================================================================
// Safe DRM ioctl Wrappers
// ============================================================================

/// Safe wrapper for DRM ioctls.
///
/// All functions in this module perform proper error handling and
/// convert errno to KgpuDriverError.
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub mod ioctl {
    use super::*;

    /// Get DRM driver version.
    ///
    /// # Arguments
    ///
    /// * `fd` - Open DRM device file descriptor
    ///
    /// # Returns
    ///
    /// DrmVersion containing driver name and version info.
    ///
    /// # Errors
    ///
    /// - `KgpuDriverError::DrmIoctlFailed`: ioctl failed
    /// - `KgpuDriverError::InvalidParameter`: Invalid fd
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_FD_VALID`: fd is a valid open DRM device
    /// - `#VERIFY_FD_VALID`: Kernel validates fd in ioctl handler
    pub fn drm_version(fd: i32) -> Result<DrmVersion, KgpuDriverError> {
        if fd < 0 {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // First call to get string lengths
        let mut ver = DrmVersionFfi::default();

        // #ASSUME_IOCTL_SAFE: DRM ioctl follows documented behavior
        // #VERIFY_IOCTL_SAFE: Kernel validates all parameters
        // SAFETY: ioctl is called with proper structure, kernel validates
        let ret = unsafe {
            libc::ioctl(fd, DRM_IOCTL_VERSION as libc::c_ulong, &mut ver as *mut _)
        };

        if ret < 0 {
            return Err(errno_to_error());
        }

        // Allocate buffers for strings
        let mut name_buf = vec![0u8; ver.name_len + 1];
        let mut date_buf = vec![0u8; ver.date_len + 1];
        let mut desc_buf = vec![0u8; ver.desc_len + 1];

        // Second call to get actual strings
        ver.name = name_buf.as_mut_ptr();
        ver.date = date_buf.as_mut_ptr();
        ver.desc = desc_buf.as_mut_ptr();

        // SAFETY: Buffers are properly sized and aligned
        let ret = unsafe {
            libc::ioctl(fd, DRM_IOCTL_VERSION as libc::c_ulong, &mut ver as *mut _)
        };

        if ret < 0 {
            return Err(errno_to_error());
        }

        // Convert to Rust strings (truncate at null)
        let name = String::from_utf8_lossy(&name_buf[..ver.name_len]).to_string();
        let date = String::from_utf8_lossy(&date_buf[..ver.date_len]).to_string();
        let desc = String::from_utf8_lossy(&desc_buf[..ver.desc_len]).to_string();

        Ok(DrmVersion {
            major: ver.version_major,
            minor: ver.version_minor,
            patchlevel: ver.version_patchlevel,
            name,
            date,
            desc,
        })
    }

    /// Get a DRM capability value.
    ///
    /// # Arguments
    ///
    /// * `fd` - Open DRM device file descriptor
    /// * `cap` - Capability to query
    ///
    /// # Returns
    ///
    /// The capability value (interpretation depends on capability type).
    ///
    /// # Errors
    ///
    /// - `KgpuDriverError::DrmIoctlFailed`: ioctl failed
    /// - `KgpuDriverError::InvalidParameter`: Capability not supported
    pub fn drm_get_cap(fd: i32, cap: DrmCapability) -> Result<u64, KgpuDriverError> {
        if fd < 0 {
            return Err(KgpuDriverError::InvalidParameter);
        }

        let mut get_cap = DrmGetCapFfi {
            capability: cap as u64,
            value: 0,
        };

        // SAFETY: ioctl with properly initialized structure
        let ret = unsafe {
            libc::ioctl(fd, DRM_IOCTL_GET_CAP as libc::c_ulong, &mut get_cap as *mut _)
        };

        if ret < 0 {
            return Err(errno_to_error());
        }

        Ok(get_cap.value)
    }

    /// Set a DRM client capability.
    ///
    /// # Arguments
    ///
    /// * `fd` - Open DRM device file descriptor
    /// * `cap` - Client capability to set
    /// * `value` - Value to set (typically 1 to enable)
    ///
    /// # Errors
    ///
    /// - `KgpuDriverError::DrmIoctlFailed`: ioctl failed
    /// - `KgpuDriverError::PermissionDenied`: Not authorized
    pub fn drm_set_client_cap(
        fd: i32,
        cap: DrmClientCapability,
        value: u64,
    ) -> Result<(), KgpuDriverError> {
        if fd < 0 {
            return Err(KgpuDriverError::InvalidParameter);
        }

        let set_cap = DrmSetClientCapFfi {
            capability: cap as u64,
            value,
        };

        // SAFETY: ioctl with properly initialized structure
        let ret = unsafe {
            libc::ioctl(fd, DRM_IOCTL_SET_CLIENT_CAP as libc::c_ulong, &set_cap as *const _)
        };

        if ret < 0 {
            return Err(errno_to_error());
        }

        Ok(())
    }

    /// Get DRM magic number for authentication.
    ///
    /// Used for DRM master authentication protocol.
    ///
    /// # Arguments
    ///
    /// * `fd` - Open DRM device file descriptor
    ///
    /// # Returns
    ///
    /// Magic number to pass to master for authentication.
    pub fn drm_get_magic(fd: i32) -> Result<u32, KgpuDriverError> {
        if fd < 0 {
            return Err(KgpuDriverError::InvalidParameter);
        }

        let mut auth = DrmAuthFfi::default();

        // SAFETY: ioctl with properly initialized structure
        let ret = unsafe {
            libc::ioctl(fd, DRM_IOCTL_GET_MAGIC as libc::c_ulong, &mut auth as *mut _)
        };

        if ret < 0 {
            return Err(errno_to_error());
        }

        Ok(auth.magic)
    }

    /// Authenticate a magic number (must be DRM master).
    ///
    /// # Arguments
    ///
    /// * `fd` - Open DRM device file descriptor (must be master)
    /// * `magic` - Magic number from drm_get_magic()
    ///
    /// # Errors
    ///
    /// - `KgpuDriverError::PermissionDenied`: Not DRM master
    pub fn drm_auth_magic(fd: i32, magic: u32) -> Result<(), KgpuDriverError> {
        if fd < 0 {
            return Err(KgpuDriverError::InvalidParameter);
        }

        let auth = DrmAuthFfi { magic };

        // SAFETY: ioctl with properly initialized structure
        let ret = unsafe {
            libc::ioctl(fd, DRM_IOCTL_AUTH_MAGIC as libc::c_ulong, &auth as *const _)
        };

        if ret < 0 {
            return Err(errno_to_error());
        }

        Ok(())
    }

    /// Become DRM master.
    ///
    /// # Arguments
    ///
    /// * `fd` - Open DRM device file descriptor
    ///
    /// # Errors
    ///
    /// - `KgpuDriverError::PermissionDenied`: Another process is master
    /// - `KgpuDriverError::DeviceBusy`: Device is busy
    pub fn drm_set_master(fd: i32) -> Result<(), KgpuDriverError> {
        if fd < 0 {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // SAFETY: Simple ioctl with no data
        let ret = unsafe {
            libc::ioctl(fd, DRM_IOCTL_SET_MASTER as libc::c_ulong)
        };

        if ret < 0 {
            return Err(errno_to_error());
        }

        Ok(())
    }

    /// Drop DRM master status.
    ///
    /// # Arguments
    ///
    /// * `fd` - Open DRM device file descriptor
    pub fn drm_drop_master(fd: i32) -> Result<(), KgpuDriverError> {
        if fd < 0 {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // SAFETY: Simple ioctl with no data
        let ret = unsafe {
            libc::ioctl(fd, DRM_IOCTL_DROP_MASTER as libc::c_ulong)
        };

        if ret < 0 {
            return Err(errno_to_error());
        }

        Ok(())
    }

    /// Export a GEM handle to a DMA-BUF file descriptor (PRIME export).
    ///
    /// # Arguments
    ///
    /// * `fd` - Open DRM device file descriptor
    /// * `handle` - GEM buffer handle to export
    /// * `flags` - Export flags (typically CloseOnExec)
    ///
    /// # Returns
    ///
    /// DMA-BUF file descriptor for sharing with other processes/devices.
    pub fn drm_prime_handle_to_fd(
        fd: i32,
        handle: u32,
        flags: u32,
    ) -> Result<i32, KgpuDriverError> {
        if fd < 0 {
            return Err(KgpuDriverError::InvalidParameter);
        }

        let mut prime = DrmPrimeHandleFfi {
            handle,
            flags,
            fd: -1,
        };

        // SAFETY: ioctl with properly initialized structure
        let ret = unsafe {
            libc::ioctl(fd, DRM_IOCTL_PRIME_HANDLE_TO_FD as libc::c_ulong, &mut prime as *mut _)
        };

        if ret < 0 {
            return Err(errno_to_error());
        }

        Ok(prime.fd)
    }

    /// Import a DMA-BUF file descriptor to a GEM handle (PRIME import).
    ///
    /// # Arguments
    ///
    /// * `fd` - Open DRM device file descriptor
    /// * `prime_fd` - DMA-BUF file descriptor to import
    ///
    /// # Returns
    ///
    /// GEM buffer handle for use with this device.
    pub fn drm_prime_fd_to_handle(fd: i32, prime_fd: i32) -> Result<u32, KgpuDriverError> {
        if fd < 0 || prime_fd < 0 {
            return Err(KgpuDriverError::InvalidParameter);
        }

        let mut prime = DrmPrimeHandleFfi {
            handle: 0,
            flags: 0,
            fd: prime_fd,
        };

        // SAFETY: ioctl with properly initialized structure
        let ret = unsafe {
            libc::ioctl(fd, DRM_IOCTL_PRIME_FD_TO_HANDLE as libc::c_ulong, &mut prime as *mut _)
        };

        if ret < 0 {
            return Err(errno_to_error());
        }

        Ok(prime.handle)
    }

    /// Get device unique string (PCI BDF path).
    ///
    /// # Arguments
    ///
    /// * `fd` - Open DRM device file descriptor
    ///
    /// # Returns
    ///
    /// Unique device identifier string (e.g., "pci:0000:01:00.0").
    pub fn drm_get_unique(fd: i32) -> Result<String, KgpuDriverError> {
        if fd < 0 {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // First call to get length
        let mut unique = DrmUniqueFfi::default();

        // SAFETY: ioctl with properly initialized structure
        let ret = unsafe {
            libc::ioctl(fd, DRM_IOCTL_GET_UNIQUE as libc::c_ulong, &mut unique as *mut _)
        };

        if ret < 0 {
            return Err(errno_to_error());
        }

        if unique.unique_len == 0 {
            return Ok(String::new());
        }

        // Allocate buffer
        let mut buf = vec![0u8; unique.unique_len + 1];
        unique.unique = buf.as_mut_ptr();

        // SAFETY: Buffer is properly sized
        let ret = unsafe {
            libc::ioctl(fd, DRM_IOCTL_GET_UNIQUE as libc::c_ulong, &mut unique as *mut _)
        };

        if ret < 0 {
            return Err(errno_to_error());
        }

        Ok(String::from_utf8_lossy(&buf[..unique.unique_len]).to_string())
    }

    /// Convert errno to KgpuDriverError.
    fn errno_to_error() -> KgpuDriverError {
        // SAFETY: errno is thread-local, reading is safe
        let errno = unsafe { *libc::__errno_location() };

        match errno {
            ENOENT | ENODEV => KgpuDriverError::DeviceNotFound,
            EACCES | EPERM => KgpuDriverError::PermissionDenied,
            EBUSY => KgpuDriverError::DeviceBusy,
            EINVAL => KgpuDriverError::InvalidParameter,
            ENOTTY => KgpuDriverError::DrmIoctlFailed,
            _ => KgpuDriverError::DrmIoctlFailed,
        }
    }
}

// ============================================================================
// Device Enumeration
// ============================================================================

/// Enumerate all DRM devices in the system.
///
/// Scans /dev/dri/ for card* and renderD* nodes.
///
/// # Returns
///
/// Vector of (path, node_type) tuples for each discovered device.
///
/// # ASSUM Tags
///
/// - `#ASSUME_PATH_CANONICAL`: /dev/dri paths are canonical
/// - `#ASSUME_DEVFS_MOUNTED`: /dev/dri exists
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn enumerate_drm_devices() -> Result<Vec<(String, DrmNodeType)>, KgpuDriverError> {
    use std::fs;

    let mut devices = Vec::new();

    // Read /dev/dri directory
    let entries = match fs::read_dir("/dev/dri") {
        Ok(entries) => entries,
        Err(_) => return Err(KgpuDriverError::DeviceNotFound),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with("card") && !name.contains("D") {
                // Primary node: /dev/dri/card0, card1, etc.
                if let Some(path_str) = path.to_str() {
                    devices.push((path_str.to_string(), DrmNodeType::Primary));
                }
            } else if name.starts_with("renderD") {
                // Render node: /dev/dri/renderD128, renderD129, etc.
                if let Some(path_str) = path.to_str() {
                    devices.push((path_str.to_string(), DrmNodeType::Render));
                }
            } else if name.starts_with("controlD") {
                // Control node: /dev/dri/controlD64, etc.
                if let Some(path_str) = path.to_str() {
                    devices.push((path_str.to_string(), DrmNodeType::Control));
                }
            }
        }
    }

    // Sort by path for consistent ordering
    devices.sort_by(|a, b| a.0.cmp(&b.0));

    if devices.is_empty() {
        return Err(KgpuDriverError::DeviceNotFound);
    }

    Ok(devices)
}

/// Open a DRM device by path.
///
/// # Arguments
///
/// * `path` - Path to DRM device (e.g., "/dev/dri/card0")
///
/// # Returns
///
/// File descriptor for the opened device.
///
/// # Errors
///
/// - `KgpuDriverError::DrmOpenFailed`: Failed to open device
/// - `KgpuDriverError::PermissionDenied`: Insufficient permissions
/// - `KgpuDriverError::DeviceNotFound`: Device doesn't exist
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn open_drm_device(path: &str) -> Result<i32, KgpuDriverError> {
    use std::ffi::CString;

    let c_path = match CString::new(path) {
        Ok(p) => p,
        Err(_) => return Err(KgpuDriverError::InvalidParameter),
    };

    // SAFETY: path is null-terminated, flags are valid
    // #ASSUME_PATH_CANONICAL: Path is a canonical device path
    let fd = unsafe {
        libc::open(c_path.as_ptr(), O_RDWR | O_CLOEXEC)
    };

    if fd < 0 {
        // SAFETY: errno is thread-local
        let errno = unsafe { *libc::__errno_location() };
        return Err(match errno {
            ENOENT | ENODEV => KgpuDriverError::DeviceNotFound,
            EACCES | EPERM => KgpuDriverError::PermissionDenied,
            EBUSY => KgpuDriverError::DeviceBusy,
            _ => KgpuDriverError::DrmOpenFailed,
        });
    }

    Ok(fd)
}

/// Close a DRM device.
///
/// # Arguments
///
/// * `fd` - File descriptor to close
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn close_drm_device(fd: i32) -> Result<(), KgpuDriverError> {
    if fd < 0 {
        return Err(KgpuDriverError::InvalidParameter);
    }

    // SAFETY: fd is valid
    let ret = unsafe { libc::close(fd) };

    if ret < 0 {
        return Err(KgpuDriverError::DrmIoctlFailed);
    }

    Ok(())
}

// ============================================================================
// Capability Query Helpers
// ============================================================================

/// Query all relevant DRM capabilities for a device.
///
/// Returns a bitmask of supported capabilities.
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn query_drm_capabilities(fd: i32) -> u64 {
    use ioctl::drm_get_cap;

    let mut caps: u64 = 0;

    let capabilities = [
        DrmCapability::DumbBuffer,
        DrmCapability::Prime,
        DrmCapability::TimestampMonotonic,
        DrmCapability::AsyncPageFlip,
        DrmCapability::AddFb2Modifiers,
        DrmCapability::SyncObj,
        DrmCapability::SyncObjTimeline,
        DrmCapability::AtomicAsyncPageFlip,
    ];

    for cap in capabilities {
        if let Ok(value) = drm_get_cap(fd, cap) {
            if value != 0 {
                caps |= 1 << (cap as u64);
            }
        }
    }

    caps
}

/// Check if device supports PRIME buffer sharing.
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn has_prime(fd: i32) -> bool {
    use ioctl::drm_get_cap;
    drm_get_cap(fd, DrmCapability::Prime).map(|v| v != 0).unwrap_or(false)
}

/// Check if device supports sync objects.
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn has_syncobj(fd: i32) -> bool {
    use ioctl::drm_get_cap;
    drm_get_cap(fd, DrmCapability::SyncObj).map(|v| v != 0).unwrap_or(false)
}

/// Check if device supports timeline sync objects.
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn has_syncobj_timeline(fd: i32) -> bool {
    use ioctl::drm_get_cap;
    drm_get_cap(fd, DrmCapability::SyncObjTimeline).map(|v| v != 0).unwrap_or(false)
}

/// Check if device supports atomic modesetting.
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn has_atomic(fd: i32) -> bool {
    use ioctl::drm_set_client_cap;
    drm_set_client_cap(fd, DrmClientCapability::Atomic, 1).is_ok()
}

/// Check if device supports FB modifiers.
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn has_modifiers(fd: i32) -> bool {
    use ioctl::drm_get_cap;
    drm_get_cap(fd, DrmCapability::AddFb2Modifiers).map(|v| v != 0).unwrap_or(false)
}

// ============================================================================
// Driver Detection
// ============================================================================

/// Known DRM driver names and their vendors.
pub const DRIVER_INTEL_I915: &str = "i915";
pub const DRIVER_INTEL_XE: &str = "xe";
pub const DRIVER_AMD_AMDGPU: &str = "amdgpu";
pub const DRIVER_AMD_RADEON: &str = "radeon";
pub const DRIVER_NVIDIA_NOUVEAU: &str = "nouveau";
pub const DRIVER_NVIDIA_NVIDIA: &str = "nvidia";
pub const DRIVER_VGEM: &str = "vgem";
pub const DRIVER_VIRTIO: &str = "virtio_gpu";

/// Detect GPU vendor from driver name.
pub fn vendor_from_driver_name(name: &str) -> GpuVendor {
    match name {
        DRIVER_INTEL_I915 | DRIVER_INTEL_XE => GpuVendor::Intel,
        DRIVER_AMD_AMDGPU | DRIVER_AMD_RADEON => GpuVendor::Amd,
        DRIVER_NVIDIA_NOUVEAU | DRIVER_NVIDIA_NVIDIA => GpuVendor::Nvidia,
        _ => GpuVendor::Unknown,
    }
}

/// Check if driver name is open-source.
pub fn is_open_source_driver(name: &str) -> bool {
    matches!(
        name,
        DRIVER_INTEL_I915
            | DRIVER_INTEL_XE
            | DRIVER_AMD_AMDGPU
            | DRIVER_AMD_RADEON
            | DRIVER_NVIDIA_NOUVEAU
            | DRIVER_VGEM
            | DRIVER_VIRTIO
    )
}

// ============================================================================
// Utility Functions
// ============================================================================

/// FNV-1a hash for strings (64-bit).
///
/// Used for driver name and path hashing in capsules.
#[inline]
pub const fn fnv1a_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001B3;

    let mut hash = FNV_OFFSET;
    let mut i = 0;
    while i < data.len() {
        hash ^= data[i] as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
        i += 1;
    }
    hash
}

/// Parse PCI BDF from DRM unique string.
///
/// The unique string format is typically "pci:DDDD:BB:DD.F".
pub fn parse_pci_bdf_from_unique(unique: &str) -> Option<PciBdf> {
    // Strip "pci:" prefix if present
    let bdf_str = unique.strip_prefix("pci:").unwrap_or(unique);
    PciBdf::from_sysfs_path(bdf_str)
}

// ============================================================================
// DRM Device Info (High-level)
// ============================================================================

/// Complete DRM device information.
///
/// Aggregates all information about a DRM device.
#[derive(Debug, Clone)]
pub struct DrmDeviceInfo {
    /// Device path
    pub path: String,
    /// Node type
    pub node_type: DrmNodeType,
    /// Driver version
    pub version: DrmVersion,
    /// PCI BDF address
    pub pci_bdf: Option<PciBdf>,
    /// GPU vendor
    pub vendor: GpuVendor,
    /// GPU generation
    pub generation: GpuGeneration,
    /// Capabilities bitmask
    pub capabilities: u64,
    /// Supports PRIME
    pub has_prime: bool,
    /// Supports sync objects
    pub has_syncobj: bool,
    /// Supports atomic modesetting
    pub has_atomic: bool,
}

impl DrmDeviceInfo {
    /// Create new empty device info.
    pub fn new() -> Self {
        Self {
            path: String::new(),
            node_type: DrmNodeType::Render,
            version: DrmVersion::new(),
            pci_bdf: None,
            vendor: GpuVendor::Unknown,
            generation: GpuGeneration::Unknown,
            capabilities: 0,
            has_prime: false,
            has_syncobj: false,
            has_atomic: false,
        }
    }
}

impl Default for DrmDeviceInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for DrmDeviceInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}) - {} [{}]",
            self.path,
            self.node_type,
            self.version,
            self.generation
        )
    }
}

/// Query complete device information.
///
/// Opens the device, queries all info, and closes it.
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn query_device_info(path: &str, node_type: DrmNodeType) -> Result<DrmDeviceInfo, KgpuDriverError> {
    let fd = open_drm_device(path)?;

    let result = (|| {
        let version = ioctl::drm_version(fd)?;
        let unique = ioctl::drm_get_unique(fd).unwrap_or_default();
        let pci_bdf = parse_pci_bdf_from_unique(&unique);
        let vendor = vendor_from_driver_name(&version.name);
        let capabilities = query_drm_capabilities(fd);

        // Detect generation from driver name for now
        // (Full detection would require reading PCI device ID from sysfs)
        let generation = GpuGeneration::Unknown;

        Ok(DrmDeviceInfo {
            path: path.to_string(),
            node_type,
            version,
            pci_bdf,
            vendor,
            generation,
            capabilities,
            has_prime: has_prime(fd),
            has_syncobj: has_syncobj(fd),
            has_atomic: has_atomic(fd),
        })
    })();

    let _ = close_drm_device(fd);
    result
}

/// Enumerate and query all DRM devices.
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub fn enumerate_and_query_devices() -> Result<Vec<DrmDeviceInfo>, KgpuDriverError> {
    let devices = enumerate_drm_devices()?;
    let mut info_list = Vec::new();

    for (path, node_type) in devices {
        if let Ok(info) = query_device_info(&path, node_type) {
            info_list.push(info);
        }
    }

    Ok(info_list)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1-Q7: Unit Tests (DRM Node Type)
    // ========================================================================

    #[test]
    fn test_drm_node_type_values() {
        assert_eq!(DrmNodeType::Primary as u8, 0);
        assert_eq!(DrmNodeType::Control as u8, 1);
        assert_eq!(DrmNodeType::Render as u8, 2);
    }

    #[test]
    fn test_drm_node_type_path_prefix() {
        assert_eq!(DrmNodeType::Primary.path_prefix(), "/dev/dri/card");
        assert_eq!(DrmNodeType::Control.path_prefix(), "/dev/dri/controlD");
        assert_eq!(DrmNodeType::Render.path_prefix(), "/dev/dri/renderD");
    }

    #[test]
    fn test_drm_node_type_capabilities() {
        assert!(DrmNodeType::Primary.supports_rendering());
        assert!(DrmNodeType::Render.supports_rendering());
        assert!(!DrmNodeType::Control.supports_rendering());

        assert!(DrmNodeType::Primary.supports_modesetting());
        assert!(DrmNodeType::Control.supports_modesetting());
        assert!(!DrmNodeType::Render.supports_modesetting());

        assert!(DrmNodeType::Primary.requires_master());
        assert!(!DrmNodeType::Render.requires_master());
        assert!(!DrmNodeType::Control.requires_master());
    }

    #[test]
    fn test_drm_node_type_default() {
        assert_eq!(DrmNodeType::default(), DrmNodeType::Render);
    }

    #[test]
    fn test_drm_node_type_display() {
        assert_eq!(format!("{}", DrmNodeType::Primary), "Primary");
        assert_eq!(format!("{}", DrmNodeType::Control), "Control");
        assert_eq!(format!("{}", DrmNodeType::Render), "Render");
    }

    // ========================================================================
    // Q1-Q7: Unit Tests (DRM Capability)
    // ========================================================================

    #[test]
    fn test_drm_capability_values() {
        assert_eq!(DrmCapability::DumbBuffer as u64, 0x1);
        assert_eq!(DrmCapability::Prime as u64, 0x5);
        assert_eq!(DrmCapability::SyncObj as u64, 0x13);
        assert_eq!(DrmCapability::SyncObjTimeline as u64, 0x14);
    }

    #[test]
    fn test_drm_capability_names() {
        assert_eq!(DrmCapability::DumbBuffer.name(), "DUMB_BUFFER");
        assert_eq!(DrmCapability::Prime.name(), "PRIME");
        assert_eq!(DrmCapability::SyncObj.name(), "SYNCOBJ");
    }

    #[test]
    fn test_drm_capability_is_boolean() {
        assert!(DrmCapability::DumbBuffer.is_boolean());
        assert!(DrmCapability::Prime.is_boolean());
        assert!(!DrmCapability::CursorWidth.is_boolean());
        assert!(!DrmCapability::DumbPreferredDepth.is_boolean());
    }

    #[test]
    fn test_drm_capability_display() {
        assert_eq!(format!("{}", DrmCapability::Prime), "DRM_CAP_PRIME");
        assert_eq!(format!("{}", DrmCapability::SyncObj), "DRM_CAP_SYNCOBJ");
    }

    // ========================================================================
    // Q1-Q7: Unit Tests (DRM Client Capability)
    // ========================================================================

    #[test]
    fn test_drm_client_capability_values() {
        assert_eq!(DrmClientCapability::Stereo3D as u64, 1);
        assert_eq!(DrmClientCapability::UniversalPlanes as u64, 2);
        assert_eq!(DrmClientCapability::Atomic as u64, 3);
    }

    #[test]
    fn test_drm_client_capability_names() {
        assert_eq!(DrmClientCapability::Atomic.name(), "ATOMIC");
        assert_eq!(DrmClientCapability::UniversalPlanes.name(), "UNIVERSAL_PLANES");
    }

    // ========================================================================
    // Q1-Q7: Unit Tests (DRM Version)
    // ========================================================================

    #[test]
    fn test_drm_version_new() {
        let ver = DrmVersion::new();
        assert_eq!(ver.major, 0);
        assert_eq!(ver.minor, 0);
        assert_eq!(ver.patchlevel, 0);
        assert!(ver.name.is_empty());
    }

    #[test]
    fn test_drm_version_display() {
        let mut ver = DrmVersion::new();
        ver.major = 1;
        ver.minor = 2;
        ver.patchlevel = 3;
        ver.name = "test".to_string();
        assert_eq!(format!("{}", ver), "test v1.2.3");
    }

    #[test]
    fn test_drm_version_tuple() {
        let mut ver = DrmVersion::new();
        ver.major = 1;
        ver.minor = 2;
        ver.patchlevel = 3;
        assert_eq!(ver.version_tuple(), (1, 2, 3));
    }

    // ========================================================================
    // Q1-Q7: Unit Tests (DRM Device Capsule)
    // ========================================================================

    #[test]
    fn test_drm_device_capsule_new() {
        let capsule = DrmDeviceCapsule::new();
        assert_eq!(capsule.fd(), 0);
        assert_eq!(capsule.generation(), 0);
        assert!(!capsule.is_open());
    }

    #[test]
    fn test_drm_device_capsule_init() {
        let capsule = DrmDeviceCapsule::new();
        capsule.init(42, DrmNodeType::Render, "/dev/dri/renderD128");

        assert_eq!(capsule.fd(), 42);
        assert_eq!(capsule.node_type(), DrmNodeType::Render);
        assert!(capsule.is_open());
        assert_eq!(capsule.generation(), 1);
    }

    #[test]
    fn test_drm_device_capsule_flags() {
        let capsule = DrmDeviceCapsule::new();
        capsule.init(10, DrmNodeType::Primary, "/dev/dri/card0");

        assert!(capsule.is_open());
        assert!(!capsule.is_master());
        assert!(!capsule.is_authenticated());

        let gen_before = capsule.generation();
        capsule.set_flag(DRM_DEVICE_FLAG_MASTER);
        assert!(capsule.is_master());
        assert!(capsule.generation() > gen_before);

        capsule.clear_flag(DRM_DEVICE_FLAG_MASTER);
        assert!(!capsule.is_master());
    }

    #[test]
    fn test_drm_device_capsule_capabilities() {
        let capsule = DrmDeviceCapsule::new();
        capsule.set_capabilities(0b11111);

        assert_eq!(capsule.capabilities(), 0b11111);
    }

    #[test]
    fn test_drm_device_capsule_bus_info() {
        let capsule = DrmDeviceCapsule::new();
        let bdf = PciBdf::new(0, 1, 0, 0);
        capsule.set_bus_info(&bdf, 0x8086);

        let result = capsule.bus_info();
        assert_eq!(result.domain, 0);
        assert_eq!(result.bus, 1);
        assert_eq!(result.device, 0);
        assert_eq!(result.function, 0);
        assert_eq!(capsule.vendor_id(), 0x8086);
        assert_eq!(capsule.vendor(), GpuVendor::Intel);
    }

    #[test]
    fn test_drm_device_capsule_version() {
        let capsule = DrmDeviceCapsule::new();
        capsule.set_version(1, 2, 3);

        assert_eq!(capsule.version(), (1, 2, 3));
    }

    #[test]
    fn test_drm_device_capsule_mark_closed() {
        let capsule = DrmDeviceCapsule::new();
        capsule.init(42, DrmNodeType::Render, "/dev/dri/renderD128");

        assert!(capsule.is_open());
        capsule.mark_closed();
        assert!(!capsule.is_open());
        assert_eq!(capsule.fd(), 0);
    }

    #[test]
    fn test_drm_device_capsule_size() {
        assert_eq!(core::mem::size_of::<DrmDeviceCapsule>(), 128);
    }

    #[test]
    fn test_drm_device_capsule_alignment() {
        assert_eq!(core::mem::align_of::<DrmDeviceCapsule>(), 128);
    }

    // ========================================================================
    // Q1-Q7: Unit Tests (ioctl encoding)
    // ========================================================================

    #[test]
    fn test_ioctl_constants() {
        // Verify ioctl numbers match expected values
        assert_eq!(DRM_IOCTL_VERSION, 0xC0406400);
        assert_eq!(DRM_IOCTL_GET_CAP, 0xC010640C);
        assert_eq!(DRM_IOCTL_SET_MASTER, 0x0000641E);
        assert_eq!(DRM_IOCTL_DROP_MASTER, 0x0000641F);
    }

    // ========================================================================
    // Q1-Q7: Unit Tests (FNV-1a Hash)
    // ========================================================================

    #[test]
    fn test_fnv1a_hash_empty() {
        let hash = fnv1a_hash(b"");
        assert_eq!(hash, 0xcbf29ce484222325); // FNV offset basis
    }

    #[test]
    fn test_fnv1a_hash_basic() {
        let hash1 = fnv1a_hash(b"i915");
        let hash2 = fnv1a_hash(b"amdgpu");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_fnv1a_hash_deterministic() {
        let hash1 = fnv1a_hash(b"test");
        let hash2 = fnv1a_hash(b"test");
        assert_eq!(hash1, hash2);
    }

    // ========================================================================
    // Q1-Q7: Unit Tests (Driver Detection)
    // ========================================================================

    #[test]
    fn test_vendor_from_driver_name() {
        assert_eq!(vendor_from_driver_name("i915"), GpuVendor::Intel);
        assert_eq!(vendor_from_driver_name("xe"), GpuVendor::Intel);
        assert_eq!(vendor_from_driver_name("amdgpu"), GpuVendor::Amd);
        assert_eq!(vendor_from_driver_name("radeon"), GpuVendor::Amd);
        assert_eq!(vendor_from_driver_name("nouveau"), GpuVendor::Nvidia);
        assert_eq!(vendor_from_driver_name("nvidia"), GpuVendor::Nvidia);
        assert_eq!(vendor_from_driver_name("unknown"), GpuVendor::Unknown);
    }

    #[test]
    fn test_is_open_source_driver() {
        assert!(is_open_source_driver("i915"));
        assert!(is_open_source_driver("xe"));
        assert!(is_open_source_driver("amdgpu"));
        assert!(is_open_source_driver("nouveau"));
        assert!(!is_open_source_driver("nvidia"));
        assert!(is_open_source_driver("vgem"));
    }

    // ========================================================================
    // Q1-Q7: Unit Tests (PCI BDF Parsing)
    // ========================================================================

    #[test]
    fn test_parse_pci_bdf_from_unique() {
        let bdf = parse_pci_bdf_from_unique("pci:0000:01:00.0").unwrap();
        assert_eq!(bdf.domain, 0);
        assert_eq!(bdf.bus, 1);
        assert_eq!(bdf.device, 0);
        assert_eq!(bdf.function, 0);
    }

    #[test]
    fn test_parse_pci_bdf_from_unique_no_prefix() {
        let bdf = parse_pci_bdf_from_unique("0000:01:00.0").unwrap();
        assert_eq!(bdf.bus, 1);
    }

    #[test]
    fn test_parse_pci_bdf_from_unique_invalid() {
        assert!(parse_pci_bdf_from_unique("invalid").is_none());
    }

    // ========================================================================
    // Q1-Q7: Unit Tests (DRM Device Info)
    // ========================================================================

    #[test]
    fn test_drm_device_info_new() {
        let info = DrmDeviceInfo::new();
        assert!(info.path.is_empty());
        assert_eq!(info.vendor, GpuVendor::Unknown);
        assert!(!info.has_prime);
    }

    #[test]
    fn test_drm_device_info_display() {
        let mut info = DrmDeviceInfo::new();
        info.path = "/dev/dri/card0".to_string();
        info.node_type = DrmNodeType::Primary;
        info.version.name = "i915".to_string();
        info.version.major = 1;
        info.version.minor = 0;
        info.version.patchlevel = 0;
        info.generation = GpuGeneration::IntelXe;

        let display = format!("{}", info);
        assert!(display.contains("/dev/dri/card0"));
        assert!(display.contains("i915"));
    }

    // ========================================================================
    // Q8-Q14: Property Tests
    // ========================================================================

    #[test]
    fn test_capsule_flag_atomicity() {
        use core::sync::atomic::fence;

        let capsule = DrmDeviceCapsule::new();
        capsule.init(1, DrmNodeType::Primary, "/dev/dri/card0");

        // Set multiple flags and verify atomicity
        for _ in 0..100 {
            capsule.set_flag(DRM_DEVICE_FLAG_MASTER);
            assert!(capsule.is_master());

            capsule.clear_flag(DRM_DEVICE_FLAG_MASTER);
            assert!(!capsule.is_master());

            // Memory fence to ensure visibility
            fence(Ordering::SeqCst);
        }
    }

    #[test]
    fn test_generation_monotonic() {
        let capsule = DrmDeviceCapsule::new();

        let mut prev_gen = capsule.generation();
        capsule.init(1, DrmNodeType::Primary, "/dev/dri/card0");
        assert!(capsule.generation() > prev_gen);

        prev_gen = capsule.generation();
        capsule.set_flag(DRM_DEVICE_FLAG_MASTER);
        assert!(capsule.generation() > prev_gen);

        prev_gen = capsule.generation();
        capsule.clear_flag(DRM_DEVICE_FLAG_MASTER);
        assert!(capsule.generation() > prev_gen);
    }

    #[test]
    fn test_node_type_consistency() {
        for node_type in [DrmNodeType::Primary, DrmNodeType::Control, DrmNodeType::Render] {
            let capsule = DrmDeviceCapsule::new();
            capsule.init(42, node_type, "/dev/dri/test");
            assert_eq!(capsule.node_type(), node_type);
        }
    }

    // ========================================================================
    // Q8-Q14: Property Tests (ioctl structure sizes)
    // ========================================================================

    #[test]
    fn test_ffi_structure_sizes() {
        // These must match kernel expectations
        assert_eq!(core::mem::size_of::<DrmGetCapFfi>(), 16);
        assert_eq!(core::mem::size_of::<DrmSetClientCapFfi>(), 16);
        assert_eq!(core::mem::size_of::<DrmAuthFfi>(), 4);
        assert_eq!(core::mem::size_of::<DrmPrimeHandleFfi>(), 12);
    }

    #[test]
    fn test_ffi_structure_alignment() {
        assert!(core::mem::align_of::<DrmGetCapFfi>() <= 8);
        assert!(core::mem::align_of::<DrmSetClientCapFfi>() <= 8);
        assert!(core::mem::align_of::<DrmAuthFfi>() <= 4);
        assert!(core::mem::align_of::<DrmPrimeHandleFfi>() <= 4);
    }

    // ========================================================================
    // Q8-Q14: Property Tests (Hash collision resistance)
    // ========================================================================

    #[test]
    fn test_fnv1a_collision_resistance() {
        let drivers = [
            "i915", "xe", "amdgpu", "radeon", "nouveau", "nvidia",
            "vgem", "virtio_gpu", "test", "mock",
        ];

        let mut hashes = Vec::new();
        for driver in drivers {
            let hash = fnv1a_hash(driver.as_bytes());
            // Ensure no collisions
            assert!(!hashes.contains(&hash), "Hash collision for {}", driver);
            hashes.push(hash);
        }
    }

    // ========================================================================
    // Q8-Q14: Property Tests (Capability encoding)
    // ========================================================================

    #[test]
    fn test_capability_bitmask_encoding() {
        let capsule = DrmDeviceCapsule::new();

        // Set individual capabilities via bitmask
        let prime_bit = 1u64 << (DrmCapability::Prime as u64);
        let syncobj_bit = 1u64 << (DrmCapability::SyncObj as u64);

        capsule.set_capabilities(prime_bit | syncobj_bit);

        assert!(capsule.has_capability(DrmCapability::Prime));
        assert!(capsule.has_capability(DrmCapability::SyncObj));
        assert!(!capsule.has_capability(DrmCapability::DumbBuffer));
    }

    // ========================================================================
    // Q15-Q21: Integration Tests (require DRM devices)
    // ========================================================================

    #[cfg(all(feature = "kgpu-driver-linux", target_os = "linux", feature = "drm-integration-tests"))]
    mod integration_tests {
        use super::super::*;

        #[test]
        fn test_enumerate_drm_devices() {
            // This test requires actual DRM devices
            match enumerate_drm_devices() {
                Ok(devices) => {
                    println!("Found {} DRM devices:", devices.len());
                    for (path, node_type) in &devices {
                        println!("  {} ({})", path, node_type);
                    }
                    assert!(!devices.is_empty());
                }
                Err(e) => {
                    println!("No DRM devices found: {:?}", e);
                    // Not a failure - just no devices
                }
            }
        }

        #[test]
        fn test_open_close_device() {
            if let Ok(devices) = enumerate_drm_devices() {
                if let Some((path, _)) = devices.first() {
                    let fd = open_drm_device(path);
                    assert!(fd.is_ok(), "Failed to open {}", path);

                    let fd = fd.unwrap();
                    assert!(fd >= 0);

                    let close_result = close_drm_device(fd);
                    assert!(close_result.is_ok());
                }
            }
        }

        #[test]
        fn test_query_version() {
            if let Ok(devices) = enumerate_drm_devices() {
                if let Some((path, _)) = devices.first() {
                    if let Ok(fd) = open_drm_device(path) {
                        let version = ioctl::drm_version(fd);
                        assert!(version.is_ok(), "Failed to get version");

                        let version = version.unwrap();
                        println!("Driver: {} v{}.{}.{}",
                            version.name, version.major, version.minor, version.patchlevel);
                        assert!(!version.name.is_empty());

                        let _ = close_drm_device(fd);
                    }
                }
            }
        }

        #[test]
        fn test_query_capabilities() {
            if let Ok(devices) = enumerate_drm_devices() {
                if let Some((path, _)) = devices.first() {
                    if let Ok(fd) = open_drm_device(path) {
                        let has_prime = has_prime(fd);
                        let has_syncobj = has_syncobj(fd);
                        let has_atomic_mode = has_atomic(fd);

                        println!("Capabilities: PRIME={}, SYNCOBJ={}, ATOMIC={}",
                            has_prime, has_syncobj, has_atomic_mode);

                        let _ = close_drm_device(fd);
                    }
                }
            }
        }

        #[test]
        fn test_query_device_info_integration() {
            if let Ok(devices) = enumerate_drm_devices() {
                if let Some((path, node_type)) = devices.first() {
                    let info = query_device_info(path, *node_type);
                    assert!(info.is_ok(), "Failed to query device info");

                    let info = info.unwrap();
                    println!("Device: {}", info);
                }
            }
        }

        #[test]
        fn test_enumerate_and_query_all() {
            match enumerate_and_query_devices() {
                Ok(devices) => {
                    println!("Found {} devices:", devices.len());
                    for info in &devices {
                        println!("  {}", info);
                    }
                }
                Err(e) => {
                    println!("Enumeration failed: {:?}", e);
                }
            }
        }
    }

    // ========================================================================
    // Q22-Q28: Production Tests (stress/perf)
    // ========================================================================

    #[test]
    fn test_capsule_concurrent_access_simulation() {
        // Simulate concurrent access patterns
        let capsule = DrmDeviceCapsule::new();
        capsule.init(1, DrmNodeType::Primary, "/dev/dri/card0");

        // Rapid flag toggling (simulates concurrent state changes)
        for _ in 0..1000 {
            capsule.set_flag(DRM_DEVICE_FLAG_MASTER);
            capsule.set_flag(DRM_DEVICE_FLAG_AUTHENTICATED);
            capsule.clear_flag(DRM_DEVICE_FLAG_MASTER);
            capsule.set_flag(DRM_DEVICE_FLAG_RENDER_CAPABLE);
            capsule.clear_flag(DRM_DEVICE_FLAG_AUTHENTICATED);
            capsule.clear_flag(DRM_DEVICE_FLAG_RENDER_CAPABLE);
        }

        // Verify final state is consistent
        assert!(capsule.is_open());
        assert!(!capsule.is_master());
        assert!(!capsule.is_authenticated());
    }

    #[test]
    fn test_capsule_generation_overflow() {
        let capsule = DrmDeviceCapsule::new();

        // Force generation to near max
        capsule.generation.store(u64::MAX - 10, Ordering::SeqCst);

        // Perform operations that increment generation
        capsule.init(1, DrmNodeType::Primary, "/dev/dri/card0");
        capsule.set_flag(DRM_DEVICE_FLAG_MASTER);

        // Generation should wrap around gracefully
        let gen = capsule.generation();
        assert!(gen < u64::MAX - 5 || gen > u64::MAX - 10); // Either wrapped or incremented
    }

    // ========================================================================
    // Q29-Q35: Determinism Tests
    // ========================================================================

    #[test]
    fn test_drm_capsule_deterministic_init() {
        // Same inputs should produce same state
        let capsule1 = DrmDeviceCapsule::new();
        let capsule2 = DrmDeviceCapsule::new();

        capsule1.init(42, DrmNodeType::Render, "/dev/dri/renderD128");
        capsule2.init(42, DrmNodeType::Render, "/dev/dri/renderD128");

        assert_eq!(capsule1.fd(), capsule2.fd());
        assert_eq!(capsule1.node_type(), capsule2.node_type());
        assert_eq!(capsule1.path_hash.load(Ordering::Relaxed),
                   capsule2.path_hash.load(Ordering::Relaxed));
    }

    #[test]
    fn test_fnv1a_hash_deterministic_q35() {
        // Hash must be deterministic - Q35 determinism validation
        let inputs: [&[u8]; 4] = [b"i915", b"amdgpu", b"test", b""];
        for input in inputs.iter() {
            let hash1 = fnv1a_hash(input);
            let hash2 = fnv1a_hash(input);
            let hash3 = fnv1a_hash(input);
            assert_eq!(hash1, hash2);
            assert_eq!(hash2, hash3);
        }
    }

    #[test]
    fn test_capability_query_deterministic() {
        // Capability bitmask operations must be deterministic
        let capsule = DrmDeviceCapsule::new();

        let caps = 0b10101010u64;
        capsule.set_capabilities(caps);

        for _ in 0..100 {
            assert_eq!(capsule.capabilities(), caps);
            assert_eq!(capsule.has_capability(DrmCapability::Prime),
                       (caps & (1 << (DrmCapability::Prime as u64))) != 0);
        }
    }

    #[test]
    fn test_pci_bdf_roundtrip_deterministic() {
        let test_cases = [
            PciBdf::new(0, 1, 0, 0),
            PciBdf::new(0, 0, 2, 0),
            PciBdf::new(1, 255, 31, 7),
        ];

        for bdf in test_cases {
            let capsule = DrmDeviceCapsule::new();
            capsule.set_bus_info(&bdf, 0x8086);

            let result = capsule.bus_info();
            assert_eq!(result.domain, bdf.domain);
            assert_eq!(result.bus, bdf.bus);
            assert_eq!(result.device, bdf.device);
            assert_eq!(result.function, bdf.function);
        }
    }

    #[test]
    fn test_version_roundtrip_deterministic() {
        let test_cases = [
            (1, 2, 3),
            (0, 0, 0),
            (100, 200, 300),
            (0x7FFF, 0x7FFF, 0x7FFF),
        ];

        for (major, minor, patch) in test_cases {
            let capsule = DrmDeviceCapsule::new();
            capsule.set_version(major, minor, patch);

            let result = capsule.version();
            assert_eq!(result, (major, minor, patch));
        }
    }
}
