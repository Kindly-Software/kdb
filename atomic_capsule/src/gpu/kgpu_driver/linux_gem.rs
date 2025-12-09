//! Linux GEM Buffer Object Subsystem
//!
//! GEM (Graphics Execution Manager) buffer object management for the KGPU-Driver.
//! Provides lockfree, Chaos-compliant buffer allocation, CPU mapping, and DMA-BUF
//! import/export (PRIME) functionality.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                    GEM Buffer Object Lifecycle                       │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │                                                                      │
//! │  Unallocated ──────► Allocated ──────► Mapped ──────► GpuBound      │
//! │       │                  │                │               │          │
//! │       │                  │                │               │          │
//! │       │                  └────────────────┼───────────────┘          │
//! │       │                                   │                          │
//! │       │                  Exported ◄───────┴───────► Imported         │
//! │       │                     │                           │            │
//! │       │                     │                           │            │
//! │       └──────────────► PendingFree ◄────────────────────┘            │
//! │                              │                                       │
//! │                              ▼                                       │
//! │                        Unallocated                                   │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Chaos Compliance
//!
//! - **T1 Atomic**: All state transitions via lockfree CAS operations
//! - **256B Alignment**: 4 cache lines, no false sharing
//! - **Generation Counters**: TOCTOU prevention on all mutations
//! - **100% Lockfree**: No mutex, no RwLock, only atomics
//!
//! # Performance Targets
//!
//! | Operation | Target | Notes |
//! |-----------|--------|-------|
//! | State read | <10ns | Single atomic load |
//! | Create dumb | <1ms | Kernel syscall overhead |
//! | Map buffer | <500us | mmap syscall |
//! | Unmap buffer | <100us | munmap syscall |
//! | PRIME export | <1ms | fd creation |
//! | PRIME import | <1ms | handle creation |
//! | State transition (CAS) | <100ns | Lockfree CAS loop |
//!
//! # DRM ioctls
//!
//! This module wraps the following Linux DRM ioctls:
//! - `DRM_IOCTL_MODE_CREATE_DUMB`: Create dumb buffer (universal)
//! - `DRM_IOCTL_MODE_MAP_DUMB`: Get mmap offset for dumb buffer
//! - `DRM_IOCTL_MODE_DESTROY_DUMB`: Destroy dumb buffer
//! - `DRM_IOCTL_GEM_CLOSE`: Close GEM handle
//! - `DRM_IOCTL_GEM_FLINK`: Create global name (legacy)
//! - `DRM_IOCTL_GEM_OPEN`: Open by global name (legacy)
//! - `DRM_IOCTL_PRIME_HANDLE_TO_FD`: Export to DMA-BUF fd
//! - `DRM_IOCTL_PRIME_FD_TO_HANDLE`: Import from DMA-BUF fd
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_FD_VALID`: DRM device fd is valid and open
//! - `#ASSUME_HANDLE_UNIQUE`: GEM handles are unique per open
//! - `#ASSUME_MMAP_SAFE`: Kernel mmap returns valid memory
//! - `#ASSUME_IOCTL_ATOMIC`: ioctl calls are atomic from kernel perspective
//! - `#ASSUME_DMA_BUF_VALID`: DMA-BUF fds are valid for sharing
//!
//! # UCE34 Compliance
//!
//! - **Q10**: T1 Atomic tier (lockfree coordination via AtomicU64 CAS)
//! - **Q33**: ComputationalCapsule verification (256B, cache-aligned, gen counters)
//! - **Q34**: Audit trail design (generation counters, state tracking)

#![allow(dead_code)] // Allow during development

use core::sync::atomic::{AtomicU64, Ordering};
use core::fmt;

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
use core::ptr;

#[cfg(feature = "std")]
extern crate std;

#[cfg(not(feature = "std"))]
extern crate alloc;

use super::error::{KgpuDriverError, KgpuDriverResult};
use super::platform::MemoryFlags;

// ============================================================================
// DRM ioctl Constants
// ============================================================================

// Dumb buffer ioctls (universal, work on all drivers)
/// DRM_IOCTL_MODE_CREATE_DUMB - Create dumb buffer
/// _IOWR('d', 0xB2, struct drm_mode_create_dumb)
const DRM_IOCTL_MODE_CREATE_DUMB: u64 = 0xC02064B2;

/// DRM_IOCTL_MODE_MAP_DUMB - Get mmap offset for dumb buffer
/// _IOWR('d', 0xB3, struct drm_mode_map_dumb)
const DRM_IOCTL_MODE_MAP_DUMB: u64 = 0xC01064B3;

/// DRM_IOCTL_MODE_DESTROY_DUMB - Destroy dumb buffer
/// _IOWR('d', 0xB4, struct drm_mode_destroy_dumb)
const DRM_IOCTL_MODE_DESTROY_DUMB: u64 = 0xC00464B4;

// GEM generic ioctls
/// DRM_IOCTL_GEM_CLOSE - Close GEM handle
/// _IOW('d', 0x09, struct drm_gem_close)
const DRM_IOCTL_GEM_CLOSE: u64 = 0x40086409;

/// DRM_IOCTL_GEM_FLINK - Create global name (legacy)
/// _IOWR('d', 0x0A, struct drm_gem_flink)
const DRM_IOCTL_GEM_FLINK: u64 = 0xC008640A;

/// DRM_IOCTL_GEM_OPEN - Open by global name (legacy)
/// _IOWR('d', 0x0B, struct drm_gem_open)
const DRM_IOCTL_GEM_OPEN: u64 = 0xC010640B;

// PRIME ioctls (modern DMA-BUF sharing)
/// DRM_IOCTL_PRIME_HANDLE_TO_FD - Export handle to DMA-BUF fd
/// _IOWR('d', 0x2D, struct drm_prime_handle)
const DRM_IOCTL_PRIME_HANDLE_TO_FD: u64 = 0xC00C642D;

/// DRM_IOCTL_PRIME_FD_TO_HANDLE - Import DMA-BUF fd to handle
/// _IOWR('d', 0x2E, struct drm_prime_handle)
const DRM_IOCTL_PRIME_FD_TO_HANDLE: u64 = 0xC00C642E;

// mmap protection flags
/// PROT_READ - Pages may be read
const PROT_READ: i32 = 0x1;
/// PROT_WRITE - Pages may be written
const PROT_WRITE: i32 = 0x2;

// mmap flags
/// MAP_SHARED - Share changes with other processes
const MAP_SHARED: i32 = 0x01;

// ============================================================================
// GEM State
// ============================================================================

/// GEM buffer object state
///
/// Represents the lifecycle state of a GEM buffer object.
///
/// # State Machine
///
/// ```text
/// Unallocated ──► Allocated ──► Mapped ──► GpuBound
///      │              │            │           │
///      │              └────────────┼───────────┘
///      │                           │
///      │         Exported ◄────────┴────────► Imported
///      │             │                            │
///      │             │                            │
///      └──────► PendingFree ◄─────────────────────┘
///                    │
///                    ▼
///              Unallocated
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GemState {
    /// Buffer not allocated
    Unallocated = 0,
    /// Buffer allocated but not mapped
    Allocated = 1,
    /// Buffer mapped to CPU address space
    Mapped = 2,
    /// Buffer bound to GPU virtual address
    GpuBound = 3,
    /// Buffer exported via PRIME (DMA-BUF fd created)
    Exported = 4,
    /// Buffer imported via PRIME (from DMA-BUF fd)
    Imported = 5,
    /// Buffer pending free (waiting for GPU completion)
    PendingFree = 6,
}

impl GemState {
    /// Convert from u8 to GemState
    ///
    /// Unknown values default to Unallocated for safety.
    #[inline]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Unallocated,
            1 => Self::Allocated,
            2 => Self::Mapped,
            3 => Self::GpuBound,
            4 => Self::Exported,
            5 => Self::Imported,
            6 => Self::PendingFree,
            _ => Self::Unallocated, // Safe default
        }
    }

    /// Convert GemState to u8
    #[inline]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Check if buffer is in an allocated state (any non-Unallocated state except PendingFree)
    #[inline]
    pub const fn is_allocated(self) -> bool {
        matches!(
            self,
            Self::Allocated | Self::Mapped | Self::GpuBound | Self::Exported | Self::Imported
        )
    }

    /// Check if buffer can be mapped to CPU
    #[inline]
    pub const fn can_map(self) -> bool {
        matches!(self, Self::Allocated | Self::Exported | Self::Imported)
    }

    /// Check if buffer can be freed
    #[inline]
    pub const fn can_free(self) -> bool {
        matches!(self, Self::Allocated | Self::Mapped | Self::GpuBound | Self::Exported | Self::Imported)
    }

    /// Check if buffer can be exported via PRIME
    #[inline]
    pub const fn can_export(self) -> bool {
        matches!(self, Self::Allocated | Self::Mapped | Self::GpuBound)
    }

    /// Get state name for debugging
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unallocated => "Unallocated",
            Self::Allocated => "Allocated",
            Self::Mapped => "Mapped",
            Self::GpuBound => "GpuBound",
            Self::Exported => "Exported",
            Self::Imported => "Imported",
            Self::PendingFree => "PendingFree",
        }
    }
}

impl fmt::Display for GemState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl Default for GemState {
    #[inline]
    fn default() -> Self {
        Self::Unallocated
    }
}

// ============================================================================
// GEM Flags
// ============================================================================

/// GEM allocation flags
///
/// Control buffer properties like CPU visibility, caching, and usage hints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct GemFlags(u32);

impl GemFlags {
    /// Buffer is CPU mappable
    pub const MAPPABLE: Self = Self(0x0001);
    /// Buffer is GPU-only (not CPU accessible)
    pub const GPU_ONLY: Self = Self(0x0002);
    /// Buffer uses cached memory (better CPU performance)
    pub const CACHED: Self = Self(0x0004);
    /// Buffer uses write-combining (better for streaming writes)
    pub const WC: Self = Self(0x0008);
    /// Buffer can be used for display scanout
    pub const SCANOUT: Self = Self(0x0010);
    /// Buffer is for cursor overlay
    pub const CURSOR: Self = Self(0x0020);
    /// Buffer has linear (non-tiled) layout
    pub const LINEAR: Self = Self(0x0040);
    /// Buffer has tiled layout
    pub const TILED: Self = Self(0x0080);
    /// Buffer is contiguous in physical memory
    pub const CONTIGUOUS: Self = Self(0x0100);
    /// Buffer is for render target
    pub const RENDER: Self = Self(0x0200);
    /// Buffer is for textures
    pub const TEXTURE: Self = Self(0x0400);
    /// Buffer is for vertex data
    pub const VERTEX: Self = Self(0x0800);

    /// Create empty flags
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Create flags from raw bits
    #[inline]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Get raw bits
    #[inline]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Check if flags are empty
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Check if flags contain specific flag
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
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

    /// Convert from MemoryFlags
    #[inline]
    pub const fn from_memory_flags(flags: MemoryFlags) -> Self {
        let mut gem_flags = 0u32;

        if flags.contains(MemoryFlags::CPU_VISIBLE) {
            gem_flags |= Self::MAPPABLE.0;
        }
        if flags.contains(MemoryFlags::WRITE_COMBINE) {
            gem_flags |= Self::WC.0;
        }
        if flags.contains(MemoryFlags::SCANOUT) {
            gem_flags |= Self::SCANOUT.0;
        }
        if flags.contains(MemoryFlags::TEXTURE) {
            gem_flags |= Self::TEXTURE.0;
        }
        if flags.contains(MemoryFlags::VERTEX) {
            gem_flags |= Self::VERTEX.0;
        }

        Self(gem_flags)
    }
}

impl Default for GemFlags {
    #[inline]
    fn default() -> Self {
        Self::MAPPABLE.union(Self::LINEAR)
    }
}

impl core::ops::BitOr for GemFlags {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for GemFlags {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl core::ops::BitAnd for GemFlags {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl core::ops::Not for GemFlags {
    type Output = Self;

    #[inline]
    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

// ============================================================================
// GEM Buffer Capsule (T1 Atomic, 256B)
// ============================================================================

/// GEM Buffer Object Capsule (T1 Atomic, 256B)
///
/// Lockfree GEM buffer management with generation counters for TOCTOU prevention.
/// Provides CPU mapping, GPU binding, and PRIME import/export capabilities.
///
/// # Layout (256 bytes, 256-byte aligned)
///
/// ```text
/// GemBufferCapsule (256B, 4 cache lines)
/// ┌─────────────────────────────────────────────────────────────────┐
/// │  state (AtomicU64)          │  generation (AtomicU64)          │ 16B
/// │  [state:8|flags:24|rsv:32]  │  [gen counter:64]                │
/// ├─────────────────────────────────────────────────────────────────┤
/// │  size (AtomicU64)           │  gpu_va (AtomicU64)              │ 16B
/// │  [buffer size in bytes]     │  [GPU virtual address]           │
/// ├─────────────────────────────────────────────────────────────────┤
/// │  cpu_ptr (AtomicU64)        │  prime_fd (AtomicU64)            │ 16B
/// │  [CPU mapping address]      │  [DMA-BUF fd if exported]        │
/// ├─────────────────────────────────────────────────────────────────┤
/// │  device_gen (AtomicU64)     │  mmap_offset (AtomicU64)         │ 16B
/// │  [owning device gen]        │  [kernel mmap offset]            │
/// ├─────────────────────────────────────────────────────────────────┤
/// │  modifier (AtomicU64)       │  pitch (AtomicU64)               │ 16B
/// │  [tiling/modifier info]     │  [stride for 2D buffers]         │
/// ├─────────────────────────────────────────────────────────────────┤
/// │  handle (AtomicU64)         │  drm_fd (AtomicU64)              │ 16B
/// │  [GEM handle:32|rsv:32]     │  [DRM device fd]                 │
/// ├─────────────────────────────────────────────────────────────────┤
/// │  width (AtomicU64)          │  height (AtomicU64)              │ 16B
/// │  [width for 2D:32|bpp:32]   │  [height for 2D:32|rsv:32]       │
/// ├─────────────────────────────────────────────────────────────────┤
/// │  flink_name (AtomicU64)     │  refcount (AtomicU64)            │ 16B
/// │  [global flink name]        │  [reference count]               │
/// ├─────────────────────────────────────────────────────────────────┤
/// │  _padding [128 bytes]                                          │ 128B
/// └─────────────────────────────────────────────────────────────────┘
/// ```
///
/// # Packed State Layout (first AtomicU64)
///
/// ```text
/// Bits  0-7:   GemState enum value (8 bits)
/// Bits  8-31:  GemFlags (24 bits)
/// Bits 32-63:  Reserved for future use
/// ```
///
/// # ASSUM Safety
///
/// - `#ASSUME_ATOMIC_ALIGNED`: All AtomicU64 fields are 8-byte aligned
/// - `#ASSUME_CACHE_ALIGNED`: Struct is 256B aligned (4 cache lines)
/// - `#ASSUME_GENERATION_MONOTONIC`: Generation counter increments monotonically
#[repr(C, align(256))]
pub struct GemBufferCapsule {
    /// Packed state: [state:8|flags:24|reserved:32]
    state: AtomicU64,

    /// Generation counter for CAS operations (TOCTOU prevention)
    generation: AtomicU64,

    /// Buffer size in bytes
    size: AtomicU64,

    /// GPU virtual address (if bound)
    gpu_va: AtomicU64,

    /// CPU mapping address (if mapped)
    cpu_ptr: AtomicU64,

    /// PRIME fd (if exported, -1 otherwise stored as u64 with sign extension)
    prime_fd: AtomicU64,

    /// DRM device generation (for tracking device handle validity)
    device_gen: AtomicU64,

    /// Kernel mmap offset (for mapping)
    mmap_offset: AtomicU64,

    /// Tiling/modifier info (DRM format modifier)
    modifier: AtomicU64,

    /// Pitch/stride for 2D buffers (bytes per row)
    pitch: AtomicU64,

    /// GEM handle (lower 32 bits) + reserved (upper 32 bits)
    handle: AtomicU64,

    /// DRM device file descriptor
    drm_fd: AtomicU64,

    /// Width (lower 32) and bpp (upper 32) for 2D buffers
    width_bpp: AtomicU64,

    /// Height (lower 32) and reserved (upper 32) for 2D buffers
    height: AtomicU64,

    /// Global flink name (legacy sharing)
    flink_name: AtomicU64,

    /// Reference count for shared buffers
    refcount: AtomicU64,

    /// Padding to reach exactly 256 bytes
    /// 16 AtomicU64 * 8 = 128 bytes of fields
    /// 256 - 128 = 128 bytes padding needed
    _padding: [u8; 128],
}

impl GemBufferCapsule {
    // ========================================================================
    // Constants
    // ========================================================================

    /// Mask for extracting state from packed state (bits 0-7)
    const STATE_MASK: u64 = 0xFF;

    /// Mask for extracting flags from packed state (bits 8-31)
    const FLAGS_MASK: u64 = 0xFFFF_FF00;

    /// Shift amount for flags
    const FLAGS_SHIFT: u32 = 8;

    /// Invalid fd value (stored as u64)
    const INVALID_FD: u64 = u64::MAX;

    /// Invalid handle value
    const INVALID_HANDLE: u64 = 0;

    // ========================================================================
    // Construction
    // ========================================================================

    /// Create a new unallocated GEM buffer capsule
    ///
    /// # Returns
    ///
    /// A new `GemBufferCapsule` in `Unallocated` state with generation 0.
    ///
    /// # Performance
    ///
    /// O(1), ~5ns (just zeroing memory)
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(0), // State::Unallocated, flags 0
            generation: AtomicU64::new(0),
            size: AtomicU64::new(0),
            gpu_va: AtomicU64::new(0),
            cpu_ptr: AtomicU64::new(0),
            prime_fd: AtomicU64::new(Self::INVALID_FD),
            device_gen: AtomicU64::new(0),
            mmap_offset: AtomicU64::new(0),
            modifier: AtomicU64::new(0),
            pitch: AtomicU64::new(0),
            handle: AtomicU64::new(Self::INVALID_HANDLE),
            drm_fd: AtomicU64::new(Self::INVALID_FD),
            width_bpp: AtomicU64::new(0),
            height: AtomicU64::new(0),
            flink_name: AtomicU64::new(0),
            refcount: AtomicU64::new(0),
            _padding: [0u8; 128],
        }
    }

    // ========================================================================
    // State Accessors
    // ========================================================================

    /// Get current buffer state
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn state(&self) -> GemState {
        let v = self.state.load(Ordering::Acquire);
        GemState::from_u8((v & Self::STATE_MASK) as u8)
    }

    /// Get buffer flags
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn flags(&self) -> GemFlags {
        let v = self.state.load(Ordering::Acquire);
        GemFlags::from_bits(((v & Self::FLAGS_MASK) >> Self::FLAGS_SHIFT) as u32)
    }

    /// Get generation counter
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get buffer size in bytes
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn size(&self) -> u64 {
        self.size.load(Ordering::Acquire)
    }

    /// Get GPU virtual address
    ///
    /// # Returns
    ///
    /// GPU virtual address (0 if not bound)
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn gpu_va(&self) -> u64 {
        self.gpu_va.load(Ordering::Acquire)
    }

    /// Get CPU mapping address
    ///
    /// # Returns
    ///
    /// - `Some(ptr)` if buffer is mapped
    /// - `None` if not mapped
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn cpu_address(&self) -> Option<*mut u8> {
        let addr = self.cpu_ptr.load(Ordering::Acquire);
        if addr == 0 {
            None
        } else {
            Some(addr as *mut u8)
        }
    }

    /// Get PRIME fd (if exported)
    ///
    /// # Returns
    ///
    /// - `Some(fd)` if buffer is exported via PRIME
    /// - `None` if not exported
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn prime_fd(&self) -> Option<i32> {
        let fd = self.prime_fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            None
        } else {
            Some(fd as i32)
        }
    }

    /// Get GEM handle
    ///
    /// # Returns
    ///
    /// GEM handle (0 if not allocated)
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn handle(&self) -> u32 {
        (self.handle.load(Ordering::Acquire) & 0xFFFF_FFFF) as u32
    }

    /// Get DRM device fd
    ///
    /// # Returns
    ///
    /// - `Some(fd)` if device fd is set
    /// - `None` if not set
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn drm_fd(&self) -> Option<i32> {
        let fd = self.drm_fd.load(Ordering::Acquire);
        if fd == Self::INVALID_FD {
            None
        } else {
            Some(fd as i32)
        }
    }

    /// Get mmap offset
    ///
    /// # Returns
    ///
    /// Kernel mmap offset for this buffer
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn mmap_offset(&self) -> u64 {
        self.mmap_offset.load(Ordering::Acquire)
    }

    /// Get pitch/stride
    ///
    /// # Returns
    ///
    /// Bytes per row for 2D buffers
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn pitch(&self) -> u32 {
        (self.pitch.load(Ordering::Acquire) & 0xFFFF_FFFF) as u32
    }

    /// Get width for 2D buffers
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn width(&self) -> u32 {
        (self.width_bpp.load(Ordering::Acquire) & 0xFFFF_FFFF) as u32
    }

    /// Get bits per pixel
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn bpp(&self) -> u32 {
        ((self.width_bpp.load(Ordering::Acquire) >> 32) & 0xFFFF_FFFF) as u32
    }

    /// Get height for 2D buffers
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn height(&self) -> u32 {
        (self.height.load(Ordering::Acquire) & 0xFFFF_FFFF) as u32
    }

    /// Get format modifier
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn modifier(&self) -> u64 {
        self.modifier.load(Ordering::Acquire)
    }

    /// Get flink name (legacy sharing)
    ///
    /// # Returns
    ///
    /// Global flink name (0 if not flinked)
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn flink_name(&self) -> u32 {
        (self.flink_name.load(Ordering::Acquire) & 0xFFFF_FFFF) as u32
    }

    /// Get reference count
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn refcount(&self) -> u64 {
        self.refcount.load(Ordering::Acquire)
    }

    /// Check if buffer is mapped
    #[inline]
    pub fn is_mapped(&self) -> bool {
        self.state() == GemState::Mapped
    }

    /// Check if buffer is allocated
    #[inline]
    pub fn is_allocated(&self) -> bool {
        self.state().is_allocated()
    }

    /// Check if buffer is exported
    #[inline]
    pub fn is_exported(&self) -> bool {
        self.state() == GemState::Exported
    }

    /// Check if buffer is imported
    #[inline]
    pub fn is_imported(&self) -> bool {
        self.state() == GemState::Imported
    }

    // ========================================================================
    // State Mutations (Lockfree CAS)
    // ========================================================================

    /// Internal helper: Pack state and flags into a single u64
    #[inline]
    const fn pack_state_flags(state: GemState, flags: GemFlags) -> u64 {
        (state as u64) | ((flags.bits() as u64) << Self::FLAGS_SHIFT)
    }

    /// Internal helper: Increment generation and return new value
    #[inline]
    fn increment_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Allocate a buffer (for dumb buffer creation)
    ///
    /// Transitions from `Unallocated` -> `Allocated` state.
    ///
    /// # Arguments
    ///
    /// * `handle` - GEM handle from kernel
    /// * `size` - Buffer size in bytes
    /// * `flags` - Buffer flags
    /// * `drm_fd` - DRM device fd
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(MemoryInUse)` if already allocated
    ///
    /// # Performance
    ///
    /// <100ns (CAS + stores)
    pub fn allocate(
        &self,
        handle: u32,
        size: u64,
        flags: GemFlags,
        drm_fd: i32,
    ) -> KgpuDriverResult<u64> {
        let old = self.state.load(Ordering::Acquire);
        let old_state = GemState::from_u8((old & Self::STATE_MASK) as u8);

        if old_state != GemState::Unallocated {
            return Err(KgpuDriverError::MemoryInUse);
        }

        let new = Self::pack_state_flags(GemState::Allocated, flags);

        match self.state.compare_exchange(
            old,
            new,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                // #ASSUME_ATOMIC_ALIGNED: These stores are to 8-byte aligned fields
                self.handle.store(handle as u64, Ordering::Release);
                self.size.store(size, Ordering::Release);
                self.drm_fd.store(drm_fd as u64, Ordering::Release);
                self.refcount.store(1, Ordering::Release);
                Ok(self.increment_generation())
            }
            Err(_) => Err(KgpuDriverError::MemoryInUse),
        }
    }

    /// Allocate a 2D dumb buffer
    ///
    /// Transitions from `Unallocated` -> `Allocated` state with 2D properties.
    ///
    /// # Arguments
    ///
    /// * `handle` - GEM handle from kernel
    /// * `width` - Buffer width in pixels
    /// * `height` - Buffer height in pixels
    /// * `bpp` - Bits per pixel
    /// * `pitch` - Bytes per row
    /// * `size` - Total buffer size
    /// * `flags` - Buffer flags
    /// * `drm_fd` - DRM device fd
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(MemoryInUse)` if already allocated
    pub fn allocate_2d(
        &self,
        handle: u32,
        width: u32,
        height: u32,
        bpp: u32,
        pitch: u32,
        size: u64,
        flags: GemFlags,
        drm_fd: i32,
    ) -> KgpuDriverResult<u64> {
        let old = self.state.load(Ordering::Acquire);
        let old_state = GemState::from_u8((old & Self::STATE_MASK) as u8);

        if old_state != GemState::Unallocated {
            return Err(KgpuDriverError::MemoryInUse);
        }

        let new = Self::pack_state_flags(GemState::Allocated, flags);

        match self.state.compare_exchange(
            old,
            new,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                self.handle.store(handle as u64, Ordering::Release);
                self.size.store(size, Ordering::Release);
                self.drm_fd.store(drm_fd as u64, Ordering::Release);
                self.width_bpp.store(
                    (width as u64) | ((bpp as u64) << 32),
                    Ordering::Release,
                );
                self.height.store(height as u64, Ordering::Release);
                self.pitch.store(pitch as u64, Ordering::Release);
                self.refcount.store(1, Ordering::Release);
                Ok(self.increment_generation())
            }
            Err(_) => Err(KgpuDriverError::MemoryInUse),
        }
    }

    /// Mark buffer as mapped
    ///
    /// Transitions from `Allocated` -> `Mapped` state.
    ///
    /// # Arguments
    ///
    /// * `cpu_ptr` - CPU virtual address
    /// * `mmap_offset` - Kernel mmap offset
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(InvalidMemoryHandle)` if not in Allocated state
    pub fn mark_mapped(&self, cpu_ptr: *mut u8, mmap_offset: u64) -> KgpuDriverResult<u64> {
        loop {
            let old = self.state.load(Ordering::Acquire);
            let old_state = GemState::from_u8((old & Self::STATE_MASK) as u8);

            if !old_state.can_map() {
                return Err(KgpuDriverError::InvalidMemoryHandle);
            }

            let flags = GemFlags::from_bits(((old & Self::FLAGS_MASK) >> Self::FLAGS_SHIFT) as u32);
            let new = Self::pack_state_flags(GemState::Mapped, flags);

            match self.state.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.cpu_ptr.store(cpu_ptr as u64, Ordering::Release);
                    self.mmap_offset.store(mmap_offset, Ordering::Release);
                    return Ok(self.increment_generation());
                }
                Err(_) => continue,
            }
        }
    }

    /// Mark buffer as unmapped
    ///
    /// Transitions from `Mapped` -> `Allocated` state.
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(MemoryNotMapped)` if not mapped
    pub fn mark_unmapped(&self) -> KgpuDriverResult<u64> {
        loop {
            let old = self.state.load(Ordering::Acquire);
            let old_state = GemState::from_u8((old & Self::STATE_MASK) as u8);

            if old_state != GemState::Mapped {
                return Err(KgpuDriverError::MemoryNotMapped);
            }

            let flags = GemFlags::from_bits(((old & Self::FLAGS_MASK) >> Self::FLAGS_SHIFT) as u32);
            let new = Self::pack_state_flags(GemState::Allocated, flags);

            match self.state.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.cpu_ptr.store(0, Ordering::Release);
                    return Ok(self.increment_generation());
                }
                Err(_) => continue,
            }
        }
    }

    /// Mark buffer as exported via PRIME
    ///
    /// Transitions to `Exported` state.
    ///
    /// # Arguments
    ///
    /// * `prime_fd` - DMA-BUF file descriptor
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(InvalidState)` if cannot export
    pub fn mark_exported(&self, prime_fd: i32) -> KgpuDriverResult<u64> {
        loop {
            let old = self.state.load(Ordering::Acquire);
            let old_state = GemState::from_u8((old & Self::STATE_MASK) as u8);

            if !old_state.can_export() {
                return Err(KgpuDriverError::InvalidState);
            }

            let flags = GemFlags::from_bits(((old & Self::FLAGS_MASK) >> Self::FLAGS_SHIFT) as u32);
            let new = Self::pack_state_flags(GemState::Exported, flags);

            match self.state.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.prime_fd.store(prime_fd as u64, Ordering::Release);
                    return Ok(self.increment_generation());
                }
                Err(_) => continue,
            }
        }
    }

    /// Initialize as imported buffer (from PRIME fd)
    ///
    /// Sets state to `Imported` for buffers created from DMA-BUF import.
    ///
    /// # Arguments
    ///
    /// * `handle` - GEM handle from import
    /// * `size` - Buffer size
    /// * `drm_fd` - DRM device fd
    /// * `source_fd` - Source DMA-BUF fd
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(MemoryInUse)` if not unallocated
    pub fn initialize_imported(
        &self,
        handle: u32,
        size: u64,
        drm_fd: i32,
        source_fd: i32,
    ) -> KgpuDriverResult<u64> {
        let old = self.state.load(Ordering::Acquire);
        let old_state = GemState::from_u8((old & Self::STATE_MASK) as u8);

        if old_state != GemState::Unallocated {
            return Err(KgpuDriverError::MemoryInUse);
        }

        let flags = GemFlags::MAPPABLE;
        let new = Self::pack_state_flags(GemState::Imported, flags);

        match self.state.compare_exchange(
            old,
            new,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                self.handle.store(handle as u64, Ordering::Release);
                self.size.store(size, Ordering::Release);
                self.drm_fd.store(drm_fd as u64, Ordering::Release);
                self.prime_fd.store(source_fd as u64, Ordering::Release);
                self.refcount.store(1, Ordering::Release);
                Ok(self.increment_generation())
            }
            Err(_) => Err(KgpuDriverError::MemoryInUse),
        }
    }

    /// Mark buffer as GPU bound
    ///
    /// Transitions to `GpuBound` state.
    ///
    /// # Arguments
    ///
    /// * `gpu_va` - GPU virtual address
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(InvalidState)` if cannot bind
    pub fn mark_gpu_bound(&self, gpu_va: u64) -> KgpuDriverResult<u64> {
        loop {
            let old = self.state.load(Ordering::Acquire);
            let old_state = GemState::from_u8((old & Self::STATE_MASK) as u8);

            // Can bind from Allocated or Mapped
            if !matches!(old_state, GemState::Allocated | GemState::Mapped) {
                return Err(KgpuDriverError::InvalidState);
            }

            let flags = GemFlags::from_bits(((old & Self::FLAGS_MASK) >> Self::FLAGS_SHIFT) as u32);
            let new = Self::pack_state_flags(GemState::GpuBound, flags);

            match self.state.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.gpu_va.store(gpu_va, Ordering::Release);
                    return Ok(self.increment_generation());
                }
                Err(_) => continue,
            }
        }
    }

    /// Mark buffer as GPU unbound
    ///
    /// Transitions from `GpuBound` back to `Allocated` or `Mapped`.
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(InvalidState)` if not GPU bound
    pub fn mark_gpu_unbound(&self) -> KgpuDriverResult<u64> {
        loop {
            let old = self.state.load(Ordering::Acquire);
            let old_state = GemState::from_u8((old & Self::STATE_MASK) as u8);

            if old_state != GemState::GpuBound {
                return Err(KgpuDriverError::InvalidState);
            }

            let flags = GemFlags::from_bits(((old & Self::FLAGS_MASK) >> Self::FLAGS_SHIFT) as u32);

            // Return to Mapped if CPU mapping exists, otherwise Allocated
            let cpu_addr = self.cpu_ptr.load(Ordering::Acquire);
            let new_state = if cpu_addr != 0 {
                GemState::Mapped
            } else {
                GemState::Allocated
            };
            let new = Self::pack_state_flags(new_state, flags);

            match self.state.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.gpu_va.store(0, Ordering::Release);
                    return Ok(self.increment_generation());
                }
                Err(_) => continue,
            }
        }
    }

    /// Mark buffer as pending free
    ///
    /// Transitions to `PendingFree` state.
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(InvalidMemoryHandle)` if cannot transition to pending free
    pub fn mark_pending_free(&self) -> KgpuDriverResult<u64> {
        loop {
            let old = self.state.load(Ordering::Acquire);
            let old_state = GemState::from_u8((old & Self::STATE_MASK) as u8);

            if !old_state.can_free() {
                return Err(KgpuDriverError::InvalidMemoryHandle);
            }

            let flags = GemFlags::from_bits(((old & Self::FLAGS_MASK) >> Self::FLAGS_SHIFT) as u32);
            let new = Self::pack_state_flags(GemState::PendingFree, flags);

            match self.state.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Ok(self.increment_generation());
                }
                Err(_) => continue,
            }
        }
    }

    /// Free buffer
    ///
    /// Transitions from any freeable state to `Unallocated`.
    /// Clears all fields.
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(InvalidMemoryHandle)` if not in a freeable state
    pub fn free(&self) -> KgpuDriverResult<u64> {
        loop {
            let old = self.state.load(Ordering::Acquire);
            let old_state = GemState::from_u8((old & Self::STATE_MASK) as u8);

            // Can free from any allocated state or PendingFree
            if !old_state.can_free() && old_state != GemState::PendingFree {
                return Err(KgpuDriverError::InvalidMemoryHandle);
            }

            let new = Self::pack_state_flags(GemState::Unallocated, GemFlags::empty());

            match self.state.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Clear all fields
                    self.handle.store(Self::INVALID_HANDLE, Ordering::Release);
                    self.size.store(0, Ordering::Release);
                    self.gpu_va.store(0, Ordering::Release);
                    self.cpu_ptr.store(0, Ordering::Release);
                    self.prime_fd.store(Self::INVALID_FD, Ordering::Release);
                    self.drm_fd.store(Self::INVALID_FD, Ordering::Release);
                    self.mmap_offset.store(0, Ordering::Release);
                    self.modifier.store(0, Ordering::Release);
                    self.pitch.store(0, Ordering::Release);
                    self.width_bpp.store(0, Ordering::Release);
                    self.height.store(0, Ordering::Release);
                    self.flink_name.store(0, Ordering::Release);
                    self.refcount.store(0, Ordering::Release);
                    return Ok(self.increment_generation());
                }
                Err(_) => continue,
            }
        }
    }

    /// Set flink name (legacy sharing)
    ///
    /// # Arguments
    ///
    /// * `name` - Global flink name
    #[inline]
    pub fn set_flink_name(&self, name: u32) {
        self.flink_name.store(name as u64, Ordering::Release);
    }

    /// Set format modifier
    ///
    /// # Arguments
    ///
    /// * `modifier` - DRM format modifier
    #[inline]
    pub fn set_modifier(&self, modifier: u64) {
        self.modifier.store(modifier, Ordering::Release);
    }

    /// Increment reference count
    ///
    /// # Returns
    ///
    /// New reference count
    #[inline]
    pub fn ref_inc(&self) -> u64 {
        self.refcount.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Decrement reference count
    ///
    /// # Returns
    ///
    /// New reference count
    #[inline]
    pub fn ref_dec(&self) -> u64 {
        let prev = self.refcount.fetch_sub(1, Ordering::AcqRel);
        prev.saturating_sub(1)
    }

    // ========================================================================
    // Snapshots
    // ========================================================================

    /// Take an atomic snapshot of current state
    ///
    /// # Returns
    ///
    /// Immutable `GemBufferSnapshot` with all current values
    ///
    /// # Performance
    ///
    /// <50ns (multiple atomic loads)
    #[inline]
    pub fn snapshot(&self) -> GemBufferSnapshot {
        let state_raw = self.state.load(Ordering::Acquire);

        GemBufferSnapshot {
            state: GemState::from_u8((state_raw & Self::STATE_MASK) as u8),
            flags: GemFlags::from_bits(((state_raw & Self::FLAGS_MASK) >> Self::FLAGS_SHIFT) as u32),
            generation: self.generation.load(Ordering::Acquire),
            handle: self.handle(),
            size: self.size.load(Ordering::Acquire),
            gpu_va: self.gpu_va.load(Ordering::Acquire),
            cpu_ptr: self.cpu_ptr.load(Ordering::Acquire),
            prime_fd: {
                let fd = self.prime_fd.load(Ordering::Acquire);
                if fd == Self::INVALID_FD { -1 } else { fd as i32 }
            },
            drm_fd: {
                let fd = self.drm_fd.load(Ordering::Acquire);
                if fd == Self::INVALID_FD { -1 } else { fd as i32 }
            },
            mmap_offset: self.mmap_offset.load(Ordering::Acquire),
            modifier: self.modifier.load(Ordering::Acquire),
            pitch: self.pitch(),
            width: self.width(),
            height: self.height(),
            bpp: self.bpp(),
            flink_name: self.flink_name(),
            refcount: self.refcount.load(Ordering::Acquire),
        }
    }
}

impl Default for GemBufferCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for GemBufferCapsule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let snap = self.snapshot();
        f.debug_struct("GemBufferCapsule")
            .field("state", &snap.state)
            .field("flags", &snap.flags)
            .field("generation", &snap.generation)
            .field("handle", &snap.handle)
            .field("size", &snap.size)
            .field("gpu_va", &format_args!("0x{:x}", snap.gpu_va))
            .field("cpu_ptr", &format_args!("0x{:x}", snap.cpu_ptr))
            .finish()
    }
}

// Safety: All fields are AtomicU64
// AtomicU64 is Send + Sync, so GemBufferCapsule can be safely shared.
//
// # ASSUM Safety
// - `#ASSUME_ATOMIC_ALIGNED`: AtomicU64 guarantees proper alignment
// - `#ASSUME_CACHE_ALIGNED`: #[repr(C, align(256))] ensures cache alignment
// - `#VERIFY_ATOMIC_SAFE`: All mutable access is through atomic operations
unsafe impl Send for GemBufferCapsule {}
unsafe impl Sync for GemBufferCapsule {}

// ============================================================================
// GEM Buffer Snapshot
// ============================================================================

/// Immutable snapshot of GEM buffer state
///
/// Captured atomically from `GemBufferCapsule::snapshot()`.
#[derive(Debug, Clone, Copy)]
pub struct GemBufferSnapshot {
    /// Buffer state
    pub state: GemState,
    /// Buffer flags
    pub flags: GemFlags,
    /// Generation counter
    pub generation: u64,
    /// GEM handle
    pub handle: u32,
    /// Buffer size in bytes
    pub size: u64,
    /// GPU virtual address
    pub gpu_va: u64,
    /// CPU mapping address
    pub cpu_ptr: u64,
    /// PRIME fd (-1 if not exported)
    pub prime_fd: i32,
    /// DRM device fd (-1 if not set)
    pub drm_fd: i32,
    /// Kernel mmap offset
    pub mmap_offset: u64,
    /// Format modifier
    pub modifier: u64,
    /// Pitch/stride in bytes
    pub pitch: u32,
    /// Width in pixels (2D)
    pub width: u32,
    /// Height in pixels (2D)
    pub height: u32,
    /// Bits per pixel (2D)
    pub bpp: u32,
    /// Flink name (legacy)
    pub flink_name: u32,
    /// Reference count
    pub refcount: u64,
}

impl GemBufferSnapshot {
    /// Check if buffer is allocated
    #[inline]
    pub fn is_allocated(&self) -> bool {
        self.state.is_allocated()
    }

    /// Check if buffer is mapped
    #[inline]
    pub fn is_mapped(&self) -> bool {
        self.state == GemState::Mapped
    }

    /// Check if buffer is exported
    #[inline]
    pub fn is_exported(&self) -> bool {
        self.state == GemState::Exported
    }

    /// Get CPU pointer if mapped
    #[inline]
    pub fn cpu_address(&self) -> Option<*mut u8> {
        if self.cpu_ptr == 0 {
            None
        } else {
            Some(self.cpu_ptr as *mut u8)
        }
    }

    /// Get PRIME fd if exported
    #[inline]
    pub fn get_prime_fd(&self) -> Option<i32> {
        if self.prime_fd < 0 {
            None
        } else {
            Some(self.prime_fd)
        }
    }
}

impl Default for GemBufferSnapshot {
    fn default() -> Self {
        Self {
            state: GemState::Unallocated,
            flags: GemFlags::empty(),
            generation: 0,
            handle: 0,
            size: 0,
            gpu_va: 0,
            cpu_ptr: 0,
            prime_fd: -1,
            drm_fd: -1,
            mmap_offset: 0,
            modifier: 0,
            pitch: 0,
            width: 0,
            height: 0,
            bpp: 0,
            flink_name: 0,
            refcount: 0,
        }
    }
}

// ============================================================================
// DRM ioctl Structures (FFI)
// ============================================================================

/// drm_mode_create_dumb - Create dumb buffer request
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct DrmModeCreateDumb {
    height: u32,
    width: u32,
    bpp: u32,
    flags: u32,
    handle: u32,
    pitch: u32,
    size: u64,
}

/// drm_mode_map_dumb - Map dumb buffer request
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct DrmModeMapDumb {
    handle: u32,
    _pad: u32,
    offset: u64,
}

/// drm_mode_destroy_dumb - Destroy dumb buffer request
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct DrmModeDestroyDumb {
    handle: u32,
}

/// drm_gem_close - Close GEM handle
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct DrmGemClose {
    handle: u32,
    _pad: u32,
}

/// drm_gem_flink - Create global name
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct DrmGemFlink {
    handle: u32,
    name: u32,
}

/// drm_gem_open - Open by global name
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct DrmGemOpen {
    name: u32,
    handle: u32,
    size: u64,
}

/// drm_prime_handle - PRIME import/export
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct DrmPrimeHandle {
    handle: u32,
    flags: u32,
    fd: i32,
}

// PRIME flags
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
const DRM_CLOEXEC: u32 = 0x1;
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
const DRM_RDWR: u32 = 0x2;

// ============================================================================
// Linux DRM GEM Operations
// ============================================================================

#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
mod linux_impl {
    use super::*;

    /// Perform ioctl syscall
    ///
    /// # Safety
    ///
    /// - `fd` must be a valid file descriptor
    /// - `request` must be a valid ioctl request code
    /// - `arg` must point to a valid structure for the request
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_FD_VALID`: fd is open and valid DRM device
    /// - `#ASSUME_IOCTL_ATOMIC`: ioctl is atomic from kernel perspective
    /// - `#VERIFY_FD_VALID`: Caller must ensure fd is valid DRM device
    #[inline]
    unsafe fn ioctl(fd: i32, request: u64, arg: *mut core::ffi::c_void) -> i32 {
        // syscall number for ioctl on x86_64
        #[cfg(target_arch = "x86_64")]
        const SYS_IOCTL: i64 = 16;

        #[cfg(target_arch = "x86_64")]
        {
            let ret: i64;
            core::arch::asm!(
                "syscall",
                inlateout("rax") SYS_IOCTL => ret,
                in("rdi") fd,
                in("rsi") request,
                in("rdx") arg,
                out("rcx") _,
                out("r11") _,
                options(nostack)
            );
            ret as i32
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            // For non-x86_64, use libc
            extern "C" {
                fn ioctl(fd: i32, request: u64, ...) -> i32;
            }
            ioctl(fd, request, arg)
        }
    }

    /// Perform mmap syscall
    ///
    /// # Safety
    ///
    /// - Parameters must be valid for mmap
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_MMAP_SAFE`: Kernel returns valid memory or error
    /// - `#VERIFY_MMAP_SAFE`: Return value must be checked for MAP_FAILED
    #[inline]
    unsafe fn mmap(
        addr: *mut core::ffi::c_void,
        length: usize,
        prot: i32,
        flags: i32,
        fd: i32,
        offset: i64,
    ) -> *mut core::ffi::c_void {
        #[cfg(target_arch = "x86_64")]
        const SYS_MMAP: i64 = 9;

        #[cfg(target_arch = "x86_64")]
        {
            let ret: i64;
            core::arch::asm!(
                "syscall",
                inlateout("rax") SYS_MMAP => ret,
                in("rdi") addr,
                in("rsi") length,
                in("rdx") prot,
                in("r10") flags,
                in("r8") fd,
                in("r9") offset,
                out("rcx") _,
                out("r11") _,
                options(nostack)
            );
            ret as *mut core::ffi::c_void
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            extern "C" {
                fn mmap(
                    addr: *mut core::ffi::c_void,
                    length: usize,
                    prot: i32,
                    flags: i32,
                    fd: i32,
                    offset: i64,
                ) -> *mut core::ffi::c_void;
            }
            mmap(addr, length, prot, flags, fd, offset)
        }
    }

    /// Perform munmap syscall
    ///
    /// # Safety
    ///
    /// - `addr` must be a valid mapped address
    /// - `length` must match the original mapping
    #[inline]
    unsafe fn munmap(addr: *mut core::ffi::c_void, length: usize) -> i32 {
        #[cfg(target_arch = "x86_64")]
        const SYS_MUNMAP: i64 = 11;

        #[cfg(target_arch = "x86_64")]
        {
            let ret: i64;
            core::arch::asm!(
                "syscall",
                inlateout("rax") SYS_MUNMAP => ret,
                in("rdi") addr,
                in("rsi") length,
                out("rcx") _,
                out("r11") _,
                options(nostack)
            );
            ret as i32
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            extern "C" {
                fn munmap(addr: *mut core::ffi::c_void, length: usize) -> i32;
            }
            munmap(addr, length)
        }
    }

    /// Close file descriptor
    ///
    /// # Safety
    ///
    /// - `fd` must be a valid file descriptor
    #[inline]
    unsafe fn close_fd(fd: i32) -> i32 {
        #[cfg(target_arch = "x86_64")]
        const SYS_CLOSE: i64 = 3;

        #[cfg(target_arch = "x86_64")]
        {
            let ret: i64;
            core::arch::asm!(
                "syscall",
                inlateout("rax") SYS_CLOSE => ret,
                in("rdi") fd,
                out("rcx") _,
                out("r11") _,
                options(nostack)
            );
            ret as i32
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            extern "C" {
                fn close(fd: i32) -> i32;
            }
            close(fd)
        }
    }

    // MAP_FAILED constant
    const MAP_FAILED: *mut core::ffi::c_void = !0usize as *mut core::ffi::c_void;

    /// Create a dumb buffer
    ///
    /// Allocates a simple linear buffer via DRM_IOCTL_MODE_CREATE_DUMB.
    /// This is the universal method that works on all DRM drivers.
    ///
    /// # Arguments
    ///
    /// * `capsule` - GemBufferCapsule to initialize
    /// * `drm_fd` - DRM device file descriptor
    /// * `width` - Buffer width in pixels
    /// * `height` - Buffer height in pixels
    /// * `bpp` - Bits per pixel (typically 32)
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(DrmIoctlFailed)` on ioctl failure
    /// - `Err(MemoryInUse)` if capsule already allocated
    ///
    /// # Safety
    ///
    /// - `drm_fd` must be a valid DRM device fd
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_FD_VALID`: drm_fd is valid DRM device
    /// - `#VERIFY_FD_VALID`: Caller responsible for fd validity
    pub fn create_dumb(
        capsule: &GemBufferCapsule,
        drm_fd: i32,
        width: u32,
        height: u32,
        bpp: u32,
    ) -> KgpuDriverResult<u64> {
        let mut req = DrmModeCreateDumb {
            width,
            height,
            bpp,
            flags: 0,
            handle: 0,
            pitch: 0,
            size: 0,
        };

        // #ASSUME_FD_VALID: drm_fd must be valid DRM device
        // #ASSUME_IOCTL_ATOMIC: ioctl is atomic
        let ret = unsafe {
            ioctl(
                drm_fd,
                DRM_IOCTL_MODE_CREATE_DUMB,
                &mut req as *mut _ as *mut core::ffi::c_void,
            )
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        // Initialize capsule with allocated buffer info
        capsule.allocate_2d(
            req.handle,
            req.width,
            req.height,
            req.bpp,
            req.pitch,
            req.size,
            GemFlags::MAPPABLE | GemFlags::LINEAR,
            drm_fd,
        )
    }

    /// Get mmap offset for dumb buffer
    ///
    /// # Arguments
    ///
    /// * `drm_fd` - DRM device fd
    /// * `handle` - GEM handle
    ///
    /// # Returns
    ///
    /// - `Ok(offset)` on success
    /// - `Err(DrmIoctlFailed)` on failure
    pub fn get_dumb_map_offset(drm_fd: i32, handle: u32) -> KgpuDriverResult<u64> {
        let mut req = DrmModeMapDumb {
            handle,
            _pad: 0,
            offset: 0,
        };

        let ret = unsafe {
            ioctl(
                drm_fd,
                DRM_IOCTL_MODE_MAP_DUMB,
                &mut req as *mut _ as *mut core::ffi::c_void,
            )
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        Ok(req.offset)
    }

    /// Map a GEM buffer to CPU address space
    ///
    /// # Arguments
    ///
    /// * `capsule` - GemBufferCapsule to map
    /// * `prot` - Protection flags (PROT_READ | PROT_WRITE)
    ///
    /// # Returns
    ///
    /// - `Ok(ptr)` on success with CPU address
    /// - `Err(MemoryMapFailed)` on mmap failure
    /// - `Err(InvalidMemoryHandle)` if cannot map
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_MMAP_SAFE`: mmap returns valid memory
    /// - `#VERIFY_MMAP_SAFE`: Return checked for MAP_FAILED
    pub fn mmap_buffer(capsule: &GemBufferCapsule, prot: i32) -> KgpuDriverResult<*mut u8> {
        let snap = capsule.snapshot();

        if !snap.state.can_map() {
            return Err(KgpuDriverError::InvalidMemoryHandle);
        }

        let drm_fd = snap.drm_fd;
        if drm_fd < 0 {
            return Err(KgpuDriverError::InvalidMemoryHandle);
        }

        // Get mmap offset if not already set
        let offset = if snap.mmap_offset == 0 {
            get_dumb_map_offset(drm_fd, snap.handle)?
        } else {
            snap.mmap_offset
        };

        // #ASSUME_MMAP_SAFE: Kernel mmap returns valid memory
        let ptr = unsafe {
            mmap(
                ptr::null_mut(),
                snap.size as usize,
                prot,
                MAP_SHARED,
                drm_fd,
                offset as i64,
            )
        };

        // #VERIFY_MMAP_SAFE: Check for MAP_FAILED
        if ptr == MAP_FAILED {
            return Err(KgpuDriverError::MemoryMapFailed);
        }

        capsule.mark_mapped(ptr as *mut u8, offset)?;

        Ok(ptr as *mut u8)
    }

    /// Unmap a GEM buffer from CPU address space
    ///
    /// # Arguments
    ///
    /// * `capsule` - GemBufferCapsule to unmap
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(MemoryNotMapped)` if not mapped
    pub fn munmap_buffer(capsule: &GemBufferCapsule) -> KgpuDriverResult<u64> {
        let snap = capsule.snapshot();

        if snap.state != GemState::Mapped {
            return Err(KgpuDriverError::MemoryNotMapped);
        }

        if snap.cpu_ptr == 0 {
            return Err(KgpuDriverError::MemoryNotMapped);
        }

        let ret = unsafe { munmap(snap.cpu_ptr as *mut core::ffi::c_void, snap.size as usize) };

        if ret < 0 {
            // Even if munmap fails, mark as unmapped since the mapping may be invalid
            // This prevents memory leaks in error paths
        }

        capsule.mark_unmapped()
    }

    /// Destroy a dumb buffer
    ///
    /// # Arguments
    ///
    /// * `capsule` - GemBufferCapsule to destroy
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(InvalidMemoryHandle)` if not allocated
    pub fn destroy_dumb(capsule: &GemBufferCapsule) -> KgpuDriverResult<u64> {
        let snap = capsule.snapshot();

        if !snap.state.can_free() && snap.state != GemState::PendingFree {
            return Err(KgpuDriverError::InvalidMemoryHandle);
        }

        // Unmap if mapped
        if snap.state == GemState::Mapped {
            let _ = munmap_buffer(capsule);
        }

        // Close PRIME fd if exported
        if snap.prime_fd >= 0 {
            unsafe { close_fd(snap.prime_fd); }
        }

        // Destroy dumb buffer
        let mut req = DrmModeDestroyDumb {
            handle: snap.handle,
        };

        let ret = unsafe {
            ioctl(
                snap.drm_fd,
                DRM_IOCTL_MODE_DESTROY_DUMB,
                &mut req as *mut _ as *mut core::ffi::c_void,
            )
        };

        // Even if ioctl fails, free the capsule to prevent leaks
        let gen = capsule.free()?;

        if ret < 0 {
            // Log warning but don't fail - capsule is freed
        }

        Ok(gen)
    }

    /// Close GEM handle
    ///
    /// Closes a GEM handle without destroying the underlying buffer.
    /// Used for imported buffers and reference counting.
    ///
    /// # Arguments
    ///
    /// * `drm_fd` - DRM device fd
    /// * `handle` - GEM handle to close
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err(DrmIoctlFailed)` on failure
    pub fn gem_close(drm_fd: i32, handle: u32) -> KgpuDriverResult<()> {
        let mut req = DrmGemClose {
            handle,
            _pad: 0,
        };

        let ret = unsafe {
            ioctl(
                drm_fd,
                DRM_IOCTL_GEM_CLOSE,
                &mut req as *mut _ as *mut core::ffi::c_void,
            )
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        Ok(())
    }

    /// Export buffer via PRIME (create DMA-BUF fd)
    ///
    /// # Arguments
    ///
    /// * `capsule` - GemBufferCapsule to export
    ///
    /// # Returns
    ///
    /// - `Ok(fd)` on success with DMA-BUF fd
    /// - `Err(DrmIoctlFailed)` on failure
    pub fn export_prime(capsule: &GemBufferCapsule) -> KgpuDriverResult<i32> {
        let snap = capsule.snapshot();

        if !snap.state.can_export() {
            return Err(KgpuDriverError::InvalidState);
        }

        let mut req = DrmPrimeHandle {
            handle: snap.handle,
            flags: DRM_CLOEXEC | DRM_RDWR,
            fd: -1,
        };

        let ret = unsafe {
            ioctl(
                snap.drm_fd,
                DRM_IOCTL_PRIME_HANDLE_TO_FD,
                &mut req as *mut _ as *mut core::ffi::c_void,
            )
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        capsule.mark_exported(req.fd)?;

        Ok(req.fd)
    }

    /// Import buffer via PRIME (from DMA-BUF fd)
    ///
    /// # Arguments
    ///
    /// * `capsule` - GemBufferCapsule to initialize
    /// * `drm_fd` - DRM device fd
    /// * `prime_fd` - DMA-BUF fd to import
    /// * `size` - Buffer size
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(DrmIoctlFailed)` on failure
    pub fn import_prime(
        capsule: &GemBufferCapsule,
        drm_fd: i32,
        prime_fd: i32,
        size: u64,
    ) -> KgpuDriverResult<u64> {
        let mut req = DrmPrimeHandle {
            handle: 0,
            flags: DRM_CLOEXEC,
            fd: prime_fd,
        };

        let ret = unsafe {
            ioctl(
                drm_fd,
                DRM_IOCTL_PRIME_FD_TO_HANDLE,
                &mut req as *mut _ as *mut core::ffi::c_void,
            )
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        capsule.initialize_imported(req.handle, size, drm_fd, prime_fd)
    }

    /// Create flink name (legacy sharing)
    ///
    /// # Arguments
    ///
    /// * `capsule` - GemBufferCapsule to flink
    ///
    /// # Returns
    ///
    /// - `Ok(name)` on success with global name
    /// - `Err(DrmIoctlFailed)` on failure
    pub fn gem_flink(capsule: &GemBufferCapsule) -> KgpuDriverResult<u32> {
        let snap = capsule.snapshot();

        if !snap.state.is_allocated() {
            return Err(KgpuDriverError::InvalidMemoryHandle);
        }

        let mut req = DrmGemFlink {
            handle: snap.handle,
            name: 0,
        };

        let ret = unsafe {
            ioctl(
                snap.drm_fd,
                DRM_IOCTL_GEM_FLINK,
                &mut req as *mut _ as *mut core::ffi::c_void,
            )
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        capsule.set_flink_name(req.name);

        Ok(req.name)
    }

    /// Open buffer by flink name (legacy sharing)
    ///
    /// # Arguments
    ///
    /// * `capsule` - GemBufferCapsule to initialize
    /// * `drm_fd` - DRM device fd
    /// * `name` - Global flink name
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(DrmIoctlFailed)` on failure
    pub fn gem_open(
        capsule: &GemBufferCapsule,
        drm_fd: i32,
        name: u32,
    ) -> KgpuDriverResult<u64> {
        let mut req = DrmGemOpen {
            name,
            handle: 0,
            size: 0,
        };

        let ret = unsafe {
            ioctl(
                drm_fd,
                DRM_IOCTL_GEM_OPEN,
                &mut req as *mut _ as *mut core::ffi::c_void,
            )
        };

        if ret < 0 {
            return Err(KgpuDriverError::DrmIoctlFailed);
        }

        // Initialize as imported (flink is similar to import)
        let gen = capsule.allocate(
            req.handle,
            req.size,
            GemFlags::MAPPABLE,
            drm_fd,
        )?;

        capsule.set_flink_name(name);

        Ok(gen)
    }
}

// Re-export Linux functions at module level
#[cfg(all(feature = "kgpu-driver-linux", target_os = "linux"))]
pub use linux_impl::*;

// ============================================================================
// Compile-Time Assertions
// ============================================================================

// Verify GemBufferCapsule is exactly 256 bytes
const _: () = {
    assert!(
        core::mem::size_of::<GemBufferCapsule>() == 256,
        "GemBufferCapsule must be 256 bytes"
    );
};

// Verify GemBufferCapsule is 256-byte aligned
const _: () = {
    assert!(
        core::mem::align_of::<GemBufferCapsule>() == 256,
        "GemBufferCapsule must be 256-byte aligned"
    );
};

// Verify GemState fits in u8
const _: () = {
    assert!(
        core::mem::size_of::<GemState>() == 1,
        "GemState must be 1 byte"
    );
};

// Verify GemFlags fits in u32
const _: () = {
    assert!(
        core::mem::size_of::<GemFlags>() == 4,
        "GemFlags must be 4 bytes"
    );
};

// ============================================================================
// Tests (T28 Compliant)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem;

    // ========================================================================
    // Tier 1: Unit Tests (Q1-Q7)
    // ========================================================================

    #[test]
    fn test_capsule_size() {
        // T28 Q1: Verify exact size is 256 bytes
        assert_eq!(mem::size_of::<GemBufferCapsule>(), 256);
    }

    #[test]
    fn test_capsule_alignment() {
        // T28 Q2: Verify alignment is 256 bytes (4 cache lines)
        assert_eq!(mem::align_of::<GemBufferCapsule>(), 256);
    }

    #[test]
    fn test_new_capsule_state() {
        // T28 Q3: Verify initial state is Unallocated
        let capsule = GemBufferCapsule::new();
        assert_eq!(capsule.state(), GemState::Unallocated);
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.size(), 0);
        assert_eq!(capsule.handle(), 0);
        assert!(capsule.cpu_address().is_none());
        assert!(capsule.prime_fd().is_none());
    }

    #[test]
    fn test_default_impl() {
        // T28 Q4: Verify Default trait implementation
        let capsule: GemBufferCapsule = Default::default();
        assert_eq!(capsule.state(), GemState::Unallocated);
    }

    #[test]
    fn test_gem_state_from_u8() {
        // T28 Q5: Verify GemState conversion
        assert_eq!(GemState::from_u8(0), GemState::Unallocated);
        assert_eq!(GemState::from_u8(1), GemState::Allocated);
        assert_eq!(GemState::from_u8(2), GemState::Mapped);
        assert_eq!(GemState::from_u8(3), GemState::GpuBound);
        assert_eq!(GemState::from_u8(4), GemState::Exported);
        assert_eq!(GemState::from_u8(5), GemState::Imported);
        assert_eq!(GemState::from_u8(6), GemState::PendingFree);
        assert_eq!(GemState::from_u8(255), GemState::Unallocated); // Unknown -> Unallocated
    }

    #[test]
    fn test_gem_state_predicates() {
        // T28 Q6: Verify state predicates
        assert!(!GemState::Unallocated.is_allocated());
        assert!(GemState::Allocated.is_allocated());
        assert!(GemState::Mapped.is_allocated());
        assert!(GemState::GpuBound.is_allocated());
        assert!(GemState::Exported.is_allocated());
        assert!(GemState::Imported.is_allocated());
        assert!(!GemState::PendingFree.is_allocated());

        assert!(GemState::Allocated.can_map());
        assert!(!GemState::Unallocated.can_map());
        assert!(!GemState::Mapped.can_map());
        assert!(GemState::Exported.can_map());
        assert!(GemState::Imported.can_map());

        assert!(GemState::Allocated.can_free());
        assert!(GemState::Mapped.can_free());
        assert!(!GemState::Unallocated.can_free());

        assert!(GemState::Allocated.can_export());
        assert!(GemState::Mapped.can_export());
        assert!(!GemState::Unallocated.can_export());
    }

    #[test]
    fn test_gem_flags() {
        // T28 Q7: Verify GemFlags operations
        let flags = GemFlags::MAPPABLE | GemFlags::LINEAR;
        assert!(flags.contains(GemFlags::MAPPABLE));
        assert!(flags.contains(GemFlags::LINEAR));
        assert!(!flags.contains(GemFlags::TILED));

        let empty = GemFlags::empty();
        assert!(empty.is_empty());

        let union = GemFlags::MAPPABLE.union(GemFlags::CACHED);
        assert!(union.contains(GemFlags::MAPPABLE));
        assert!(union.contains(GemFlags::CACHED));
    }

    // ========================================================================
    // Tier 2: State Transitions (Q8-Q14)
    // ========================================================================

    #[test]
    fn test_allocate_success() {
        // T28 Q8: Verify Unallocated -> Allocated transition
        let capsule = GemBufferCapsule::new();

        let result = capsule.allocate(42, 4096, GemFlags::MAPPABLE, 5);
        assert!(result.is_ok());

        let gen = result.unwrap();
        assert_eq!(gen, 1);
        assert_eq!(capsule.state(), GemState::Allocated);
        assert_eq!(capsule.handle(), 42);
        assert_eq!(capsule.size(), 4096);
        assert!(capsule.flags().contains(GemFlags::MAPPABLE));
    }

    #[test]
    fn test_allocate_already_allocated() {
        // T28 Q9: Verify allocation fails if not Unallocated
        let capsule = GemBufferCapsule::new();
        capsule.allocate(1, 1024, GemFlags::empty(), 5).unwrap();

        let result = capsule.allocate(2, 2048, GemFlags::empty(), 5);
        assert_eq!(result, Err(KgpuDriverError::MemoryInUse));
    }

    #[test]
    fn test_allocate_2d() {
        // T28 Q10: Verify 2D allocation
        let capsule = GemBufferCapsule::new();

        let result = capsule.allocate_2d(
            100,
            1920,
            1080,
            32,
            1920 * 4,
            1920 * 1080 * 4,
            GemFlags::SCANOUT | GemFlags::LINEAR,
            5,
        );
        assert!(result.is_ok());

        assert_eq!(capsule.width(), 1920);
        assert_eq!(capsule.height(), 1080);
        assert_eq!(capsule.bpp(), 32);
        assert_eq!(capsule.pitch(), 1920 * 4);
    }

    #[test]
    fn test_mark_mapped() {
        // T28 Q11: Verify Allocated -> Mapped transition
        let capsule = GemBufferCapsule::new();
        capsule.allocate(1, 4096, GemFlags::MAPPABLE, 5).unwrap();

        let cpu_ptr = 0x7FFF_0000 as *mut u8;
        let result = capsule.mark_mapped(cpu_ptr, 0x1000);
        assert!(result.is_ok());
        assert_eq!(capsule.state(), GemState::Mapped);
        assert_eq!(capsule.cpu_address(), Some(cpu_ptr));
        assert_eq!(capsule.mmap_offset(), 0x1000);
    }

    #[test]
    fn test_mark_unmapped() {
        // T28 Q12: Verify Mapped -> Allocated transition
        let capsule = GemBufferCapsule::new();
        capsule.allocate(1, 4096, GemFlags::MAPPABLE, 5).unwrap();
        capsule.mark_mapped(0x1000 as *mut u8, 0x2000).unwrap();

        let result = capsule.mark_unmapped();
        assert!(result.is_ok());
        assert_eq!(capsule.state(), GemState::Allocated);
        assert!(capsule.cpu_address().is_none());
    }

    #[test]
    fn test_mark_exported() {
        // T28 Q13: Verify export transition
        let capsule = GemBufferCapsule::new();
        capsule.allocate(1, 4096, GemFlags::MAPPABLE, 5).unwrap();

        let result = capsule.mark_exported(42);
        assert!(result.is_ok());
        assert_eq!(capsule.state(), GemState::Exported);
        assert_eq!(capsule.prime_fd(), Some(42));
    }

    #[test]
    fn test_initialize_imported() {
        // T28 Q14: Verify import initialization
        let capsule = GemBufferCapsule::new();

        let result = capsule.initialize_imported(99, 8192, 5, 77);
        assert!(result.is_ok());
        assert_eq!(capsule.state(), GemState::Imported);
        assert_eq!(capsule.handle(), 99);
        assert_eq!(capsule.size(), 8192);
        assert_eq!(capsule.prime_fd(), Some(77));
    }

    // ========================================================================
    // Tier 3: GPU Binding and Free (Q15-Q21)
    // ========================================================================

    #[test]
    fn test_mark_gpu_bound() {
        // T28 Q15: Verify GPU binding
        let capsule = GemBufferCapsule::new();
        capsule.allocate(1, 4096, GemFlags::MAPPABLE, 5).unwrap();

        let result = capsule.mark_gpu_bound(0xFFFF_0000_0000);
        assert!(result.is_ok());
        assert_eq!(capsule.state(), GemState::GpuBound);
        assert_eq!(capsule.gpu_va(), 0xFFFF_0000_0000);
    }

    #[test]
    fn test_mark_gpu_unbound() {
        // T28 Q16: Verify GPU unbinding
        let capsule = GemBufferCapsule::new();
        capsule.allocate(1, 4096, GemFlags::MAPPABLE, 5).unwrap();
        capsule.mark_gpu_bound(0x1000).unwrap();

        let result = capsule.mark_gpu_unbound();
        assert!(result.is_ok());
        assert_eq!(capsule.state(), GemState::Allocated);
        assert_eq!(capsule.gpu_va(), 0);
    }

    #[test]
    fn test_mark_gpu_unbound_preserves_mapped() {
        // T28 Q17: Verify GPU unbind returns to Mapped if CPU was mapped
        let capsule = GemBufferCapsule::new();
        capsule.allocate(1, 4096, GemFlags::MAPPABLE, 5).unwrap();
        capsule.mark_mapped(0x1000 as *mut u8, 0x2000).unwrap();

        // Internal: mark as GPU bound (bypassing state check for test)
        let old = capsule.state.load(Ordering::Acquire);
        let flags = ((old & GemBufferCapsule::FLAGS_MASK) >> GemBufferCapsule::FLAGS_SHIFT) as u32;
        let new = GemBufferCapsule::pack_state_flags(GemState::GpuBound, GemFlags::from_bits(flags));
        capsule.state.store(new, Ordering::Release);
        capsule.gpu_va.store(0xFFFF, Ordering::Release);

        let result = capsule.mark_gpu_unbound();
        assert!(result.is_ok());
        assert_eq!(capsule.state(), GemState::Mapped);
    }

    #[test]
    fn test_mark_pending_free() {
        // T28 Q18: Verify pending free transition
        let capsule = GemBufferCapsule::new();
        capsule.allocate(1, 4096, GemFlags::MAPPABLE, 5).unwrap();

        let result = capsule.mark_pending_free();
        assert!(result.is_ok());
        assert_eq!(capsule.state(), GemState::PendingFree);
    }

    #[test]
    fn test_free_success() {
        // T28 Q19: Verify free transition
        let capsule = GemBufferCapsule::new();
        capsule.allocate(1, 4096, GemFlags::MAPPABLE, 5).unwrap();

        let result = capsule.free();
        assert!(result.is_ok());
        assert_eq!(capsule.state(), GemState::Unallocated);
        assert_eq!(capsule.handle(), 0);
        assert_eq!(capsule.size(), 0);
    }

    #[test]
    fn test_free_from_pending() {
        // T28 Q20: Verify free from PendingFree state
        let capsule = GemBufferCapsule::new();
        capsule.allocate(1, 4096, GemFlags::MAPPABLE, 5).unwrap();
        capsule.mark_pending_free().unwrap();

        let result = capsule.free();
        assert!(result.is_ok());
        assert_eq!(capsule.state(), GemState::Unallocated);
    }

    #[test]
    fn test_free_not_allocated() {
        // T28 Q21: Verify free fails if not allocated
        let capsule = GemBufferCapsule::new();

        let result = capsule.free();
        assert_eq!(result, Err(KgpuDriverError::InvalidMemoryHandle));
    }

    // ========================================================================
    // Tier 4: Snapshot and Reference Count Tests (Q22-Q28)
    // ========================================================================

    #[test]
    fn test_snapshot_captures_all_state() {
        // T28 Q22: Verify snapshot captures all fields
        let capsule = GemBufferCapsule::new();
        capsule.allocate_2d(42, 800, 600, 32, 3200, 800 * 600 * 4, GemFlags::SCANOUT, 5).unwrap();
        capsule.mark_mapped(0x1234 as *mut u8, 0x5678).unwrap();
        capsule.set_modifier(0xABCD);

        let snap = capsule.snapshot();
        assert_eq!(snap.state, GemState::Mapped);
        assert_eq!(snap.handle, 42);
        assert_eq!(snap.width, 800);
        assert_eq!(snap.height, 600);
        assert_eq!(snap.bpp, 32);
        assert_eq!(snap.pitch, 3200);
        assert_eq!(snap.cpu_ptr, 0x1234);
        assert_eq!(snap.mmap_offset, 0x5678);
        assert_eq!(snap.modifier, 0xABCD);
    }

    #[test]
    fn test_snapshot_predicates() {
        // T28 Q23: Verify snapshot predicate methods
        let snap_unalloc = GemBufferSnapshot::default();
        assert!(!snap_unalloc.is_allocated());
        assert!(!snap_unalloc.is_mapped());
        assert!(!snap_unalloc.is_exported());
        assert!(snap_unalloc.cpu_address().is_none());

        let snap_mapped = GemBufferSnapshot {
            state: GemState::Mapped,
            cpu_ptr: 0x1000,
            ..Default::default()
        };
        assert!(snap_mapped.is_allocated());
        assert!(snap_mapped.is_mapped());
        assert!(snap_mapped.cpu_address().is_some());
    }

    #[test]
    fn test_refcount() {
        // T28 Q24: Verify reference counting
        let capsule = GemBufferCapsule::new();
        capsule.allocate(1, 4096, GemFlags::MAPPABLE, 5).unwrap();

        assert_eq!(capsule.refcount(), 1);

        let new_ref = capsule.ref_inc();
        assert_eq!(new_ref, 2);
        assert_eq!(capsule.refcount(), 2);

        let after_dec = capsule.ref_dec();
        assert_eq!(after_dec, 1);
        assert_eq!(capsule.refcount(), 1);
    }

    #[test]
    fn test_generation_increments() {
        // T28 Q25: Verify generation increments on state changes
        let capsule = GemBufferCapsule::new();
        assert_eq!(capsule.generation(), 0);

        capsule.allocate(1, 1024, GemFlags::MAPPABLE, 5).unwrap();
        assert_eq!(capsule.generation(), 1);

        capsule.mark_mapped(0x1000 as *mut u8, 0x2000).unwrap();
        assert_eq!(capsule.generation(), 2);

        capsule.mark_unmapped().unwrap();
        assert_eq!(capsule.generation(), 3);

        capsule.free().unwrap();
        assert_eq!(capsule.generation(), 4);
    }

    #[test]
    fn test_flink_name() {
        // T28 Q26: Verify flink name handling
        let capsule = GemBufferCapsule::new();
        capsule.allocate(1, 4096, GemFlags::MAPPABLE, 5).unwrap();

        assert_eq!(capsule.flink_name(), 0);

        capsule.set_flink_name(12345);
        assert_eq!(capsule.flink_name(), 12345);
    }

    #[test]
    fn test_modifier() {
        // T28 Q27: Verify modifier handling
        let capsule = GemBufferCapsule::new();
        capsule.allocate(1, 4096, GemFlags::MAPPABLE, 5).unwrap();

        assert_eq!(capsule.modifier(), 0);

        capsule.set_modifier(0x0100_0000_0000_0002); // I915_FORMAT_MOD_X_TILED
        assert_eq!(capsule.modifier(), 0x0100_0000_0000_0002);
    }

    #[test]
    fn test_debug_impl() {
        // T28 Q28: Verify Debug implementation
        let capsule = GemBufferCapsule::new();
        let debug_str = format!("{:?}", capsule);
        assert!(debug_str.contains("GemBufferCapsule"));
        assert!(debug_str.contains("Unallocated"));
    }

    // ========================================================================
    // Tier 5: Determinism Tests (Q29-Q35)
    // ========================================================================

    #[test]
    fn test_error_conditions() {
        // T28 Q29: Verify error conditions
        let capsule = GemBufferCapsule::new();

        // Can't map unallocated
        assert_eq!(
            capsule.mark_mapped(0x1000 as *mut u8, 0),
            Err(KgpuDriverError::InvalidMemoryHandle)
        );

        // Can't unmap unallocated
        assert_eq!(capsule.mark_unmapped(), Err(KgpuDriverError::MemoryNotMapped));

        // Can't export unallocated
        assert_eq!(capsule.mark_exported(1), Err(KgpuDriverError::InvalidState));

        // Can't GPU bind unallocated
        assert_eq!(capsule.mark_gpu_bound(0x1000), Err(KgpuDriverError::InvalidState));
    }

    #[test]
    fn test_gem_state_display() {
        // T28 Q30: Verify GemState Display
        assert_eq!(format!("{}", GemState::Unallocated), "Unallocated");
        assert_eq!(format!("{}", GemState::Allocated), "Allocated");
        assert_eq!(format!("{}", GemState::Mapped), "Mapped");
        assert_eq!(format!("{}", GemState::GpuBound), "GpuBound");
        assert_eq!(format!("{}", GemState::Exported), "Exported");
        assert_eq!(format!("{}", GemState::Imported), "Imported");
        assert_eq!(format!("{}", GemState::PendingFree), "PendingFree");
    }

    #[test]
    fn test_gem_flags_default() {
        // T28 Q31: Verify GemFlags default
        let flags = GemFlags::default();
        assert!(flags.contains(GemFlags::MAPPABLE));
        assert!(flags.contains(GemFlags::LINEAR));
    }

    #[test]
    fn test_gem_flags_from_memory_flags() {
        // T28 Q32: Verify conversion from MemoryFlags
        let mem_flags = MemoryFlags::CPU_VISIBLE | MemoryFlags::SCANOUT;
        let gem_flags = GemFlags::from_memory_flags(mem_flags);

        assert!(gem_flags.contains(GemFlags::MAPPABLE));
        assert!(gem_flags.contains(GemFlags::SCANOUT));
        assert!(!gem_flags.contains(GemFlags::TILED));
    }

    #[test]
    fn test_send_sync_traits() {
        // T28 Q33: Verify Send + Sync implementation
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<GemBufferCapsule>();
        assert_send_sync::<GemBufferSnapshot>();
    }

    #[test]
    fn test_snapshot_default() {
        // T28 Q34: Verify GemBufferSnapshot default
        let snap: GemBufferSnapshot = Default::default();
        assert_eq!(snap.state, GemState::Unallocated);
        assert_eq!(snap.handle, 0);
        assert_eq!(snap.size, 0);
        assert_eq!(snap.prime_fd, -1);
        assert_eq!(snap.drm_fd, -1);
    }

    #[test]
    fn test_concurrent_snapshot_safe() {
        // T28 Q35: Verify snapshot can be taken without data race
        let capsule = GemBufferCapsule::new();
        capsule.allocate(1, 4096, GemFlags::MAPPABLE, 5).unwrap();

        // Take multiple snapshots (single-threaded test)
        let snap1 = capsule.snapshot();
        let snap2 = capsule.snapshot();

        // Both should be consistent
        assert_eq!(snap1.state, snap2.state);
        assert_eq!(snap1.generation, snap2.generation);
        assert_eq!(snap1.handle, snap2.handle);
    }

    // ========================================================================
    // Additional Tests (Q36-Q42)
    // ========================================================================

    #[test]
    fn test_full_lifecycle() {
        // Q36: Test complete buffer lifecycle
        let capsule = GemBufferCapsule::new();

        // Allocate
        capsule.allocate(1, 4096, GemFlags::MAPPABLE, 5).unwrap();
        assert_eq!(capsule.state(), GemState::Allocated);

        // Map
        capsule.mark_mapped(0x1000 as *mut u8, 0x2000).unwrap();
        assert_eq!(capsule.state(), GemState::Mapped);

        // GPU bind
        capsule.mark_gpu_bound(0xFFFF).unwrap();
        assert_eq!(capsule.state(), GemState::GpuBound);

        // GPU unbind (should return to Mapped since CPU was mapped)
        capsule.mark_gpu_unbound().unwrap();
        assert_eq!(capsule.state(), GemState::Mapped);

        // Unmap
        capsule.mark_unmapped().unwrap();
        assert_eq!(capsule.state(), GemState::Allocated);

        // Free
        capsule.free().unwrap();
        assert_eq!(capsule.state(), GemState::Unallocated);
    }

    #[test]
    fn test_export_import_lifecycle() {
        // Q37: Test PRIME export/import lifecycle
        let capsule_export = GemBufferCapsule::new();
        let capsule_import = GemBufferCapsule::new();

        // Allocate and export
        capsule_export.allocate(1, 4096, GemFlags::MAPPABLE, 5).unwrap();
        capsule_export.mark_exported(42).unwrap();
        assert_eq!(capsule_export.state(), GemState::Exported);
        assert_eq!(capsule_export.prime_fd(), Some(42));

        // Import on another capsule
        capsule_import.initialize_imported(2, 4096, 6, 42).unwrap();
        assert_eq!(capsule_import.state(), GemState::Imported);
        assert_eq!(capsule_import.prime_fd(), Some(42));
    }

    #[test]
    fn test_drm_fd_tracking() {
        // Q38: Test DRM fd tracking
        let capsule = GemBufferCapsule::new();
        assert!(capsule.drm_fd().is_none());

        capsule.allocate(1, 4096, GemFlags::MAPPABLE, 7).unwrap();
        assert_eq!(capsule.drm_fd(), Some(7));
    }

    #[test]
    fn test_is_methods() {
        // Q39: Test is_* helper methods
        let capsule = GemBufferCapsule::new();

        assert!(!capsule.is_allocated());
        assert!(!capsule.is_mapped());
        assert!(!capsule.is_exported());
        assert!(!capsule.is_imported());

        capsule.allocate(1, 4096, GemFlags::MAPPABLE, 5).unwrap();
        assert!(capsule.is_allocated());

        capsule.mark_mapped(0x1000 as *mut u8, 0).unwrap();
        assert!(capsule.is_mapped());
    }

    #[test]
    fn test_state_transitions_invalid() {
        // Q40: Test invalid state transitions
        let capsule = GemBufferCapsule::new();
        capsule.allocate(1, 4096, GemFlags::MAPPABLE, 5).unwrap();
        capsule.mark_exported(42).unwrap();

        // Can't GPU bind from Exported
        assert_eq!(capsule.mark_gpu_bound(0x1000), Err(KgpuDriverError::InvalidState));

        // Can't unmap (not mapped)
        assert_eq!(capsule.mark_unmapped(), Err(KgpuDriverError::MemoryNotMapped));
    }

    #[test]
    fn test_large_size_handling() {
        // Q41: Test large buffer sizes
        let capsule = GemBufferCapsule::new();
        let large_size: u64 = 16 * 1024 * 1024 * 1024; // 16 GB

        capsule.allocate(1, large_size, GemFlags::MAPPABLE, 5).unwrap();
        assert_eq!(capsule.size(), large_size);
    }

    #[test]
    fn test_flags_combinations() {
        // Q42: Test various flag combinations
        let capsule = GemBufferCapsule::new();

        let flags = GemFlags::MAPPABLE | GemFlags::SCANOUT | GemFlags::LINEAR | GemFlags::CONTIGUOUS;
        capsule.allocate(1, 4096, flags, 5).unwrap();

        let snap = capsule.snapshot();
        assert!(snap.flags.contains(GemFlags::MAPPABLE));
        assert!(snap.flags.contains(GemFlags::SCANOUT));
        assert!(snap.flags.contains(GemFlags::LINEAR));
        assert!(snap.flags.contains(GemFlags::CONTIGUOUS));
    }
}
