//! Intel i915 DRM Driver Integration
//!
//! Chaos-compliant Intel GPU driver using i915 kernel interface.
//! Provides lockfree capsules for GEM buffer management, context creation,
//! and command submission via EXECBUFFER2.
//!
//! # Architecture
//!
//! ```text
//! +------------------------+
//! |   I915ContextCapsule   |  <- T1 Atomic (256B aligned)
//! |  [state|gen|priority]  |
//! +------------------------+
//!            |
//!            v
//! +------------------------+
//! |     I915Driver         |  <- Stateless ioctl wrapper
//! | [gem|ctx|exec|query]   |
//! +------------------------+
//!            |
//!            v
//! +------------------------+
//! |   /dev/dri/card*       |  <- DRM device node
//! |   (i915 kernel driver) |
//! +------------------------+
//! ```
//!
//! # Chaos Compliance
//!
//! - 100% lockfree (NO mutex, NO RwLock)
//! - DualAtomicU64 pattern with generation counters
//! - 256B alignment (4 cache lines)
//! - T1 Atomic tier (<100ns state operations)
//! - All unsafe blocks have #ASSUME/#VERIFY tags
//!
//! # Supported Operations
//!
//! | Category | Operations |
//! |----------|------------|
//! | GEM | create, mmap_offset, set_tiling, get_tiling, close |
//! | Context | create, destroy, setparam, getparam |
//! | Execution | execbuffer2, wait, busy |
//! | Query | getparam, has_feature, chipset_id |
//! | Firmware | guc_status, huc_status |
//!
//! # Engine Classes
//!
//! | Class | Purpose | Instances |
//! |-------|---------|-----------|
//! | Render (RCS) | 3D/Compute | 1 |
//! | Copy (BCS) | Blitter | 1-2 |
//! | Video (VCS) | Decode | 2-4 |
//! | VideoEnhance (VECS) | Post-process | 1-2 |
//! | Compute (CCS) | Gen12.5+ | 0-4 |
//!
//! # Safety
//!
//! All unsafe operations documented with ASSUM tags:
//! - `#ASSUME_IOCTL_SAFE`: ioctl syscall with valid fd
//! - `#ASSUME_PTR_VALID`: Pointer from kernel is valid
//! - `#ASSUME_MMAP_SAFE`: mmap region is accessible

#![allow(dead_code)]

use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use core::mem::MaybeUninit;

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
use std::os::unix::io::RawFd;

#[cfg(not(feature = "std"))]
type RawFd = i32;

use crate::gpu::kgpu_driver::error::{KgpuDriverError, KgpuDriverResult};
use crate::gpu::kgpu_driver::vendor::GpuGeneration;

// ============================================================================
// i915 ioctl Constants
// ============================================================================

/// DRM ioctl base
const DRM_IOCTL_BASE: u64 = 0x64; // 'd'

/// i915 driver-specific ioctl offset
const DRM_COMMAND_BASE: u64 = 0x40;

// i915 ioctl numbers (relative to DRM_COMMAND_BASE)
const DRM_I915_GETPARAM: u64 = 0x06;
const DRM_I915_GEM_CREATE: u64 = 0x1b;
const DRM_I915_GEM_PREAD: u64 = 0x1c;
const DRM_I915_GEM_PWRITE: u64 = 0x1d;
const DRM_I915_GEM_MMAP: u64 = 0x1e;
const DRM_I915_GEM_MMAP_GTT: u64 = 0x24;
const DRM_I915_GEM_MMAP_OFFSET: u64 = 0x25;
const DRM_I915_GEM_SET_DOMAIN: u64 = 0x1f;
const DRM_I915_GEM_SW_FINISH: u64 = 0x20;
const DRM_I915_GEM_SET_TILING: u64 = 0x21;
const DRM_I915_GEM_GET_TILING: u64 = 0x22;
const DRM_I915_GEM_GET_APERTURE: u64 = 0x23;
const DRM_I915_GEM_EXECBUFFER2: u64 = 0x29;
const DRM_I915_GEM_EXECBUFFER2_WR: u64 = 0x29;
const DRM_I915_GEM_BUSY: u64 = 0x2c;
const DRM_I915_GEM_THROTTLE: u64 = 0x2b;
const DRM_I915_GEM_CONTEXT_CREATE: u64 = 0x2d;
const DRM_I915_GEM_CONTEXT_CREATE_EXT: u64 = 0x2d;
const DRM_I915_GEM_CONTEXT_DESTROY: u64 = 0x2e;
const DRM_I915_GEM_CONTEXT_GETPARAM: u64 = 0x34;
const DRM_I915_GEM_CONTEXT_SETPARAM: u64 = 0x35;
const DRM_I915_GEM_WAIT: u64 = 0x2c;
const DRM_I915_GEM_CREATE_EXT: u64 = 0x3d;
const DRM_I915_GEM_VM_CREATE: u64 = 0x3a;
const DRM_I915_GEM_VM_DESTROY: u64 = 0x3b;
const DRM_I915_QUERY: u64 = 0x39;
const DRM_I915_GEM_CLOSE: u64 = 0x09;

// ============================================================================
// ioctl Encoding Macros (as const functions)
// ============================================================================

/// Build DRM ioctl number for write operations (user -> kernel)
#[inline]
const fn drm_ioctl_iow(nr: u64, size: usize) -> u64 {
    // _IOW('d', DRM_COMMAND_BASE + nr, type)
    // Direction: 1 (write) << 30
    // Size: size << 16
    // Type: 'd' << 8
    // Number: DRM_COMMAND_BASE + nr
    (1u64 << 30) | ((size as u64) << 16) | (DRM_IOCTL_BASE << 8) | (DRM_COMMAND_BASE + nr)
}

/// Build DRM ioctl number for read/write operations
#[inline]
const fn drm_ioctl_iowr(nr: u64, size: usize) -> u64 {
    // _IOWR('d', DRM_COMMAND_BASE + nr, type)
    // Direction: 3 (read|write) << 30
    (3u64 << 30) | ((size as u64) << 16) | (DRM_IOCTL_BASE << 8) | (DRM_COMMAND_BASE + nr)
}

/// Build DRM ioctl number for read operations (kernel -> user)
#[inline]
const fn drm_ioctl_ior(nr: u64, size: usize) -> u64 {
    // _IOR('d', DRM_COMMAND_BASE + nr, type)
    // Direction: 2 (read) << 30
    (2u64 << 30) | ((size as u64) << 16) | (DRM_IOCTL_BASE << 8) | (DRM_COMMAND_BASE + nr)
}

/// Build DRM ioctl number for no-data operations
#[inline]
const fn drm_ioctl_io(nr: u64) -> u64 {
    // _IO('d', DRM_COMMAND_BASE + nr)
    (DRM_IOCTL_BASE << 8) | (DRM_COMMAND_BASE + nr)
}

// Pre-computed ioctl numbers
const IOCTL_I915_GETPARAM: u64 = drm_ioctl_iowr(DRM_I915_GETPARAM, 16);
const IOCTL_I915_GEM_CREATE: u64 = drm_ioctl_iowr(DRM_I915_GEM_CREATE, 16);
const IOCTL_I915_GEM_MMAP_OFFSET: u64 = drm_ioctl_iowr(DRM_I915_GEM_MMAP_OFFSET, 32);
const IOCTL_I915_GEM_SET_TILING: u64 = drm_ioctl_iowr(DRM_I915_GEM_SET_TILING, 24);
const IOCTL_I915_GEM_GET_TILING: u64 = drm_ioctl_iowr(DRM_I915_GEM_GET_TILING, 16);
const IOCTL_I915_GEM_EXECBUFFER2: u64 = drm_ioctl_iow(DRM_I915_GEM_EXECBUFFER2, 104);
const IOCTL_I915_GEM_CONTEXT_CREATE: u64 = drm_ioctl_iowr(DRM_I915_GEM_CONTEXT_CREATE, 8);
const IOCTL_I915_GEM_CONTEXT_DESTROY: u64 = drm_ioctl_iow(DRM_I915_GEM_CONTEXT_DESTROY, 4);
const IOCTL_I915_GEM_CONTEXT_GETPARAM: u64 = drm_ioctl_iowr(DRM_I915_GEM_CONTEXT_GETPARAM, 24);
const IOCTL_I915_GEM_CONTEXT_SETPARAM: u64 = drm_ioctl_iowr(DRM_I915_GEM_CONTEXT_SETPARAM, 24);
const IOCTL_I915_GEM_BUSY: u64 = drm_ioctl_iowr(DRM_I915_GEM_BUSY, 8);
const IOCTL_I915_GEM_WAIT: u64 = drm_ioctl_iowr(DRM_I915_GEM_WAIT, 24);
const IOCTL_I915_GEM_CLOSE: u64 = drm_ioctl_iow(DRM_I915_GEM_CLOSE, 8);
const IOCTL_I915_GEM_THROTTLE: u64 = drm_ioctl_io(DRM_I915_GEM_THROTTLE);
const IOCTL_I915_QUERY: u64 = drm_ioctl_iowr(DRM_I915_QUERY, 16);
const IOCTL_I915_GEM_VM_CREATE: u64 = drm_ioctl_iowr(DRM_I915_GEM_VM_CREATE, 16);
const IOCTL_I915_GEM_VM_DESTROY: u64 = drm_ioctl_iow(DRM_I915_GEM_VM_DESTROY, 8);
const IOCTL_I915_GEM_CREATE_EXT: u64 = drm_ioctl_iowr(DRM_I915_GEM_CREATE_EXT, 40);

// ============================================================================
// Engine Classes
// ============================================================================

/// Intel GPU engine classes
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum I915EngineClass {
    /// Render engine (RCS) - 3D and compute
    Render = 0,
    /// Copy engine (BCS) - Blitter
    Copy = 1,
    /// Video decode engine (VCS)
    Video = 2,
    /// Video enhancement engine (VECS)
    VideoEnhance = 3,
    /// Compute engine (CCS) - Gen12.5+
    Compute = 4,
}

impl I915EngineClass {
    /// Convert from u8
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Render),
            1 => Some(Self::Copy),
            2 => Some(Self::Video),
            3 => Some(Self::VideoEnhance),
            4 => Some(Self::Compute),
            _ => None,
        }
    }

    /// Engine class name
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Render => "rcs",
            Self::Copy => "bcs",
            Self::Video => "vcs",
            Self::VideoEnhance => "vecs",
            Self::Compute => "ccs",
        }
    }

    /// Full name
    pub const fn full_name(&self) -> &'static str {
        match self {
            Self::Render => "Render Command Streamer",
            Self::Copy => "Blitter Command Streamer",
            Self::Video => "Video Command Streamer",
            Self::VideoEnhance => "Video Enhancement Command Streamer",
            Self::Compute => "Compute Command Streamer",
        }
    }
}

// ============================================================================
// Tiling Modes
// ============================================================================

/// GEM buffer tiling modes
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum I915TilingMode {
    /// Linear (no tiling)
    None = 0,
    /// X-major tiling (legacy, Gen2-Gen8)
    X = 1,
    /// Y-major tiling (Gen4+)
    Y = 2,
    /// Tile4 / Yf tiling (Gen9+)
    Yf = 3,
    /// Ys tiling (Gen12+, large pages)
    Ys = 4,
}

impl I915TilingMode {
    /// Convert from u32
    pub const fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::None),
            1 => Some(Self::X),
            2 => Some(Self::Y),
            3 => Some(Self::Yf),
            4 => Some(Self::Ys),
            _ => None,
        }
    }

    /// Minimum generation supporting this tiling mode
    pub const fn min_generation(&self) -> GpuGeneration {
        match self {
            Self::None | Self::X | Self::Y => GpuGeneration::IntelGen9,
            Self::Yf => GpuGeneration::IntelGen9,
            Self::Ys => GpuGeneration::IntelGen12,
        }
    }

    /// Tile width in bytes
    pub const fn tile_width(&self) -> u32 {
        match self {
            Self::None => 1,
            Self::X => 512,
            Self::Y | Self::Yf => 128,
            Self::Ys => 256,
        }
    }

    /// Tile height in rows
    pub const fn tile_height(&self) -> u32 {
        match self {
            Self::None => 1,
            Self::X => 8,
            Self::Y | Self::Yf => 32,
            Self::Ys => 64,
        }
    }
}

// ============================================================================
// i915 Parameters
// ============================================================================

/// i915 getparam parameter IDs
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum I915Param {
    /// Chipset ID (PCI device ID)
    ChipsetId = 1,
    /// Has GEM support
    HasGem = 2,
    /// Number of render slices
    NumSlices = 3,
    /// Has page flipping
    HasPageFlipping = 4,
    /// Has overlay
    HasOverlay = 5,
    /// Has exec no reloc
    HasExecNoReloc = 12,
    /// Has exec handle LUT
    HasExecHandleLut = 13,
    /// Has pooled EU
    HasPooledEu = 22,
    /// Min EU in pool
    MinEuInPool = 23,
    /// Has scheduler
    HasScheduler = 25,
    /// EU total count
    EuTotal = 26,
    /// Subslice total count
    SubsliceTotal = 27,
    /// Slice mask
    SliceMask = 29,
    /// Subslice mask
    SubsliceMask = 30,
    /// Has exec fence
    HasExecFence = 44,
    /// Has exec capture
    HasExecCapture = 45,
    /// Has exec batch first
    HasExecBatchFirst = 48,
    /// Has exec softpin
    HasExecSoftpin = 49,
    /// Has exec async
    HasExecAsync = 53,
    /// Has GuC submission
    HasGucSubmission = 55,
    /// Mmap version (GTT vs offset)
    MmapVersion = 56,
    /// Has context isolation
    HasContextIsolation = 50,
    /// CS timestamp frequency
    CsTimestampFrequency = 51,
    /// Has userptr probe
    HasUserptrProbe = 54,
    /// Has VM bind
    HasVmBind = 66,
    /// Has Flat CCS (Gen12.5+)
    HasFlatCcs = 67,
}

impl I915Param {
    /// Convert from i32
    pub const fn from_i32(v: i32) -> Option<Self> {
        match v {
            1 => Some(Self::ChipsetId),
            2 => Some(Self::HasGem),
            3 => Some(Self::NumSlices),
            4 => Some(Self::HasPageFlipping),
            5 => Some(Self::HasOverlay),
            12 => Some(Self::HasExecNoReloc),
            13 => Some(Self::HasExecHandleLut),
            22 => Some(Self::HasPooledEu),
            23 => Some(Self::MinEuInPool),
            25 => Some(Self::HasScheduler),
            26 => Some(Self::EuTotal),
            27 => Some(Self::SubsliceTotal),
            29 => Some(Self::SliceMask),
            30 => Some(Self::SubsliceMask),
            44 => Some(Self::HasExecFence),
            45 => Some(Self::HasExecCapture),
            48 => Some(Self::HasExecBatchFirst),
            49 => Some(Self::HasExecSoftpin),
            50 => Some(Self::HasContextIsolation),
            51 => Some(Self::CsTimestampFrequency),
            53 => Some(Self::HasExecAsync),
            54 => Some(Self::HasUserptrProbe),
            55 => Some(Self::HasGucSubmission),
            56 => Some(Self::MmapVersion),
            66 => Some(Self::HasVmBind),
            67 => Some(Self::HasFlatCcs),
            _ => None,
        }
    }
}

// ============================================================================
// Context Parameters
// ============================================================================

/// Context parameter IDs
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum I915ContextParam {
    /// Ban period (deprecated)
    BanPeriod = 0x1,
    /// Context is recoverable
    Recoverable = 0x3,
    /// Context priority (-1023 to 1023)
    Priority = 0x4,
    /// SSEU configuration
    Sseu = 0x5,
    /// Associated VM
    Vm = 0x6,
    /// Engine configuration
    Engines = 0x7,
    /// Persistence (keep running after close)
    Persistence = 0x8,
    /// Protected content
    Protected = 0x9,
}

// ============================================================================
// Context State Encoding
// ============================================================================

/// Context state flags (bits 48-63 of state word)
pub mod context_flags {
    /// Context is active
    pub const ACTIVE: u16 = 1 << 0;
    /// Context is banned (reset too many times)
    pub const BANNED: u16 = 1 << 1;
    /// Context uses protected content
    pub const PROTECTED: u16 = 1 << 2;
    /// Context is persistent
    pub const PERSISTENT: u16 = 1 << 3;
    /// Context has custom SSEU
    pub const CUSTOM_SSEU: u16 = 1 << 4;
    /// Context uses vm_bind
    pub const VM_BIND: u16 = 1 << 5;
}

/// Reset reasons
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetReason {
    /// No reset
    None = 0,
    /// GPU hang detected
    GpuHang = 1,
    /// Batch timeout
    BatchTimeout = 2,
    /// Context banned
    Banned = 3,
    /// User requested
    UserRequest = 4,
    /// Hardware error
    HardwareError = 5,
}

// ============================================================================
// ioctl Structures
// ============================================================================

/// drm_i915_getparam structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct I915Getparam {
    /// Parameter ID (I915Param)
    pub param: i32,
    /// Padding
    pub _pad: i32,
    /// Pointer to value (output)
    pub value: *mut i32,
}

/// drm_i915_gem_create structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct I915GemCreate {
    /// Requested size in bytes
    pub size: u64,
    /// Returned GEM handle (output)
    pub handle: u32,
    /// Padding
    pub _pad: u32,
}

/// drm_i915_gem_mmap_offset structure (Gen12+)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct I915GemMmapOffset {
    /// GEM handle
    pub handle: u32,
    /// Padding
    pub _pad: u32,
    /// Returned mmap offset (output)
    pub offset: u64,
    /// Mmap flags
    pub flags: u64,
    /// Extensions pointer
    pub extensions: u64,
}

/// Mmap offset flags
pub mod mmap_offset_flags {
    /// GTT mmap (cached, needs flush)
    pub const GTT: u64 = 0;
    /// Write-combining (uncached)
    pub const WC: u64 = 1;
    /// Write-back (cached)
    pub const WB: u64 = 2;
    /// Uncached
    pub const UC: u64 = 3;
    /// Fixed mmap (Gen12.5+)
    pub const FIXED: u64 = 4;
}

/// drm_i915_gem_set_tiling structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct I915GemSetTiling {
    /// GEM handle
    pub handle: u32,
    /// Tiling mode (I915TilingMode)
    pub tiling_mode: u32,
    /// Stride in bytes
    pub stride: u32,
    /// Swizzle mode (output)
    pub swizzle_mode: u32,
}

/// drm_i915_gem_get_tiling structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct I915GemGetTiling {
    /// GEM handle
    pub handle: u32,
    /// Tiling mode (output)
    pub tiling_mode: u32,
    /// Swizzle mode (output)
    pub swizzle_mode: u32,
    /// Physical swizzle (output)
    pub phys_swizzle_mode: u32,
}

/// drm_i915_gem_exec_object2 structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct I915GemExecObject2 {
    /// GEM handle
    pub handle: u32,
    /// Number of relocations
    pub relocation_count: u32,
    /// Pointer to relocations array
    pub relocs_ptr: u64,
    /// Required alignment
    pub alignment: u64,
    /// Presumed GPU offset
    pub offset: u64,
    /// Flags
    pub flags: u64,
    /// Reserved for extensions
    pub rsvd1: u64,
    /// Reserved
    pub rsvd2: u64,
}

/// Exec object flags
pub mod exec_object_flags {
    /// Object needs GPU access
    pub const NEEDS_FENCE: u64 = 1 << 0;
    /// Object needs GTT space
    pub const NEEDS_GTT: u64 = 1 << 1;
    /// Object is write target
    pub const WRITE: u64 = 1 << 2;
    /// Softpin - use provided offset
    pub const PINNED: u64 = 1 << 4;
    /// High memory zone preferred
    pub const SUPPORTS_48B_ADDRESS: u64 = 1 << 3;
    /// Capture on hang
    pub const CAPTURE: u64 = 1 << 7;
    /// Async binding
    pub const ASYNC: u64 = 1 << 8;
}

/// drm_i915_gem_execbuffer2 structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct I915GemExecbuffer2 {
    /// Pointer to exec objects array
    pub buffers_ptr: u64,
    /// Number of exec objects
    pub buffer_count: u32,
    /// Batch start offset
    pub batch_start_offset: u32,
    /// Batch length
    pub batch_len: u32,
    /// DR1 (deprecated)
    pub dr1: u32,
    /// DR4 (deprecated)
    pub dr4: u32,
    /// Number of cliprects (deprecated)
    pub num_cliprects: u32,
    /// Pointer to cliprects (deprecated)
    pub cliprects_ptr: u64,
    /// Flags
    pub flags: u64,
    /// Output fence (if requested)
    pub rsvd1: u64,
    /// Reserved
    pub rsvd2: u64,
}

/// Execbuffer flags
pub mod exec_flags {
    /// Ring selector mask
    pub const RING_MASK: u64 = 0x3f;
    /// Default ring
    pub const DEFAULT: u64 = 0;
    /// Render ring
    pub const RENDER: u64 = 1;
    /// BSD (video) ring
    pub const BSD: u64 = 2;
    /// BLT ring
    pub const BLT: u64 = 3;
    /// VEBOX ring
    pub const VEBOX: u64 = 4;
    /// Fence output
    pub const FENCE_OUT: u64 = 1 << 17;
    /// Fence input
    pub const FENCE_IN: u64 = 1 << 16;
    /// No relocation
    pub const NO_RELOC: u64 = 1 << 11;
    /// Handle LUT
    pub const HANDLE_LUT: u64 = 1 << 12;
    /// Batch first
    pub const BATCH_FIRST: u64 = 1 << 18;
    /// Secure batch
    pub const SECURE: u64 = 1 << 9;
    /// Fence array
    pub const FENCE_ARRAY: u64 = 1 << 19;
    /// Use context
    pub const USE_EXTENSIONS: u64 = 1 << 20;
}

/// drm_i915_gem_context_create structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct I915GemContextCreate {
    /// Context ID (output)
    pub ctx_id: u32,
    /// Flags
    pub flags: u32,
}

/// Context create flags
pub mod context_create_flags {
    /// Recoverable context
    pub const RECOVERABLE: u32 = 1 << 0;
    /// Use extensions
    pub const USE_EXTENSIONS: u32 = 1 << 1;
}

/// drm_i915_gem_context_destroy structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct I915GemContextDestroy {
    /// Context ID
    pub ctx_id: u32,
    /// Padding
    pub _pad: u32,
}

/// drm_i915_gem_context_param structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct I915GemContextParam {
    /// Context ID
    pub ctx_id: u32,
    /// Size (for variable-length params)
    pub size: u32,
    /// Parameter ID
    pub param: u64,
    /// Value
    pub value: u64,
}

/// drm_i915_gem_busy structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct I915GemBusy {
    /// GEM handle
    pub handle: u32,
    /// Busy flags (output)
    pub busy: u32,
}

/// drm_i915_gem_wait structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct I915GemWait {
    /// GEM handle
    pub handle: u32,
    /// Flags
    pub flags: u32,
    /// Timeout in nanoseconds (input/output)
    pub timeout_ns: i64,
}

/// drm_i915_gem_close structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct I915GemClose {
    /// GEM handle
    pub handle: u32,
    /// Padding
    pub _pad: u32,
}

/// drm_i915_query_item structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct I915QueryItem {
    /// Query ID
    pub query_id: u64,
    /// Length (input/output)
    pub length: i32,
    /// Flags
    pub flags: u32,
    /// Data pointer
    pub data_ptr: u64,
}

/// Query IDs
pub mod query_id {
    /// Topology info
    pub const TOPOLOGY_INFO: u64 = 1;
    /// Engine info
    pub const ENGINE_INFO: u64 = 2;
    /// Performance info
    pub const PERF_INFO: u64 = 3;
    /// Memory regions
    pub const MEMORY_REGIONS: u64 = 4;
    /// HWConfig (Xe+)
    pub const HWCONFIG: u64 = 5;
}

/// drm_i915_query structure
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct I915Query {
    /// Number of items
    pub num_items: u32,
    /// Flags
    pub flags: u32,
    /// Pointer to query items array
    pub items_ptr: u64,
}

// ============================================================================
// I915ContextCapsule - T1 Atomic Tier (256B aligned)
// ============================================================================

/// Snapshot of I915ContextCapsule state
#[derive(Debug, Clone, Copy)]
pub struct I915ContextSnapshot {
    /// Context ID
    pub ctx_id: u32,
    /// Engine class
    pub engine_class: I915EngineClass,
    /// Engine instance
    pub engine_instance: u8,
    /// Context flags
    pub flags: u16,
    /// Generation counter
    pub generation: u64,
    /// Context priority (-1023 to 1023)
    pub priority: i64,
    /// Preemption timeout (ns)
    pub preemption_timeout: u64,
    /// Is recoverable
    pub recoverable: bool,
    /// Current sequence number
    pub seqno: u64,
    /// Completed sequence number
    pub completed_seqno: u64,
    /// Batch count
    pub batch_count: u64,
    /// Total execution time (ns)
    pub exec_time_ns: u64,
    /// Reset count
    pub reset_count: u64,
    /// Last reset reason
    pub last_reset_reason: ResetReason,
}

/// Intel i915 context capsule
///
/// T1 Atomic tier capsule for managing i915 GPU contexts.
/// 256B aligned for 4 cache lines, lockfree atomic operations.
///
/// # State Encoding (64-bit)
///
/// ```text
/// [63:48] flags (16 bits) - context_flags::*
/// [47:40] engine_instance (8 bits)
/// [39:32] engine_class (8 bits) - I915EngineClass
/// [31:0]  ctx_id (32 bits) - kernel context handle
/// ```
///
/// # Safety
///
/// - All operations are lockfree using atomic instructions
/// - Generation counter prevents ABA problems
/// - State transitions are atomic (single CAS)
#[repr(C, align(256))]
pub struct I915ContextCapsule {
    /// Packed state: [flags:16][instance:8][class:8][ctx_id:32]
    state: AtomicU64,
    /// Generation counter for ABA prevention
    generation: AtomicU64,
    /// Context priority (-1023 to 1023)
    priority: AtomicU64,
    /// Preemption timeout in nanoseconds
    preemption_timeout: AtomicU64,
    /// Recoverable flag (non-zero = true)
    recoverable: AtomicU64,
    /// Current sequence number
    seqno: AtomicU64,
    /// Completed sequence number
    completed_seqno: AtomicU64,
    /// Batch submission count
    batch_count: AtomicU64,
    /// Total execution time (ns)
    exec_time_ns: AtomicU64,
    /// Reset count
    reset_count: AtomicU64,
    /// Last reset reason (ResetReason as u64)
    last_reset_reason: AtomicU64,
    /// Padding to 256 bytes
    _padding: [u8; 168],
}

impl I915ContextCapsule {
    /// Create new context capsule
    ///
    /// # Arguments
    ///
    /// * `ctx_id` - Kernel context ID
    /// * `engine_class` - Target engine class
    /// * `engine_instance` - Engine instance (0 for default)
    pub const fn new(ctx_id: u32, engine_class: I915EngineClass, engine_instance: u8) -> Self {
        let state = Self::pack_state(ctx_id, engine_class, engine_instance, context_flags::ACTIVE);
        Self {
            state: AtomicU64::new(state),
            generation: AtomicU64::new(1),
            priority: AtomicU64::new(0), // Default priority
            preemption_timeout: AtomicU64::new(640_000_000), // 640ms default
            recoverable: AtomicU64::new(1), // Recoverable by default
            seqno: AtomicU64::new(0),
            completed_seqno: AtomicU64::new(0),
            batch_count: AtomicU64::new(0),
            exec_time_ns: AtomicU64::new(0),
            reset_count: AtomicU64::new(0),
            last_reset_reason: AtomicU64::new(ResetReason::None as u64),
            _padding: [0u8; 168],
        }
    }

    /// Pack state into u64
    #[inline]
    const fn pack_state(ctx_id: u32, class: I915EngineClass, instance: u8, flags: u16) -> u64 {
        (ctx_id as u64)
            | ((class as u64) << 32)
            | ((instance as u64) << 40)
            | ((flags as u64) << 48)
    }

    /// Unpack state from u64
    #[inline]
    const fn unpack_state(state: u64) -> (u32, u8, u8, u16) {
        let ctx_id = state as u32;
        let class = ((state >> 32) & 0xFF) as u8;
        let instance = ((state >> 40) & 0xFF) as u8;
        let flags = ((state >> 48) & 0xFFFF) as u16;
        (ctx_id, class, instance, flags)
    }

    /// Take atomic snapshot
    ///
    /// # Chaos Compliance
    ///
    /// - Lockfree: All loads use Acquire ordering
    /// - <10ns typical latency
    pub fn snapshot(&self) -> I915ContextSnapshot {
        let state = self.state.load(Ordering::Acquire);
        let (ctx_id, class_u8, instance, flags) = Self::unpack_state(state);

        let engine_class = I915EngineClass::from_u8(class_u8).unwrap_or(I915EngineClass::Render);

        I915ContextSnapshot {
            ctx_id,
            engine_class,
            engine_instance: instance,
            flags,
            generation: self.generation.load(Ordering::Acquire),
            priority: self.priority.load(Ordering::Acquire) as i64,
            preemption_timeout: self.preemption_timeout.load(Ordering::Acquire),
            recoverable: self.recoverable.load(Ordering::Acquire) != 0,
            seqno: self.seqno.load(Ordering::Acquire),
            completed_seqno: self.completed_seqno.load(Ordering::Acquire),
            batch_count: self.batch_count.load(Ordering::Acquire),
            exec_time_ns: self.exec_time_ns.load(Ordering::Acquire),
            reset_count: self.reset_count.load(Ordering::Acquire),
            last_reset_reason: ResetReason::from_u8(
                self.last_reset_reason.load(Ordering::Acquire) as u8
            ),
        }
    }

    /// Get context ID
    #[inline]
    pub fn ctx_id(&self) -> u32 {
        self.state.load(Ordering::Acquire) as u32
    }

    /// Get engine class
    #[inline]
    pub fn engine_class(&self) -> I915EngineClass {
        let class_u8 = ((self.state.load(Ordering::Acquire) >> 32) & 0xFF) as u8;
        I915EngineClass::from_u8(class_u8).unwrap_or(I915EngineClass::Render)
    }

    /// Get engine instance
    #[inline]
    pub fn engine_instance(&self) -> u8 {
        ((self.state.load(Ordering::Acquire) >> 40) & 0xFF) as u8
    }

    /// Get context flags
    #[inline]
    pub fn flags(&self) -> u16 {
        ((self.state.load(Ordering::Acquire) >> 48) & 0xFFFF) as u16
    }

    /// Check if context is active
    #[inline]
    pub fn is_active(&self) -> bool {
        self.flags() & context_flags::ACTIVE != 0
    }

    /// Check if context is banned
    #[inline]
    pub fn is_banned(&self) -> bool {
        self.flags() & context_flags::BANNED != 0
    }

    /// Set context flags atomically
    pub fn set_flags(&self, new_flags: u16) -> u16 {
        loop {
            let old_state = self.state.load(Ordering::Acquire);
            let (ctx_id, class, instance, _old_flags) = Self::unpack_state(old_state);
            let new_state = Self::pack_state(
                ctx_id,
                I915EngineClass::from_u8(class).unwrap_or(I915EngineClass::Render),
                instance,
                new_flags,
            );

            match self.state.compare_exchange_weak(
                old_state,
                new_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.generation.fetch_add(1, Ordering::AcqRel);
                    return new_flags;
                }
                Err(_) => continue,
            }
        }
    }

    /// Add flag atomically
    pub fn add_flag(&self, flag: u16) {
        loop {
            let old_state = self.state.load(Ordering::Acquire);
            let (ctx_id, class, instance, old_flags) = Self::unpack_state(old_state);
            let new_flags = old_flags | flag;
            let new_state = Self::pack_state(
                ctx_id,
                I915EngineClass::from_u8(class).unwrap_or(I915EngineClass::Render),
                instance,
                new_flags,
            );

            match self.state.compare_exchange_weak(
                old_state,
                new_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.generation.fetch_add(1, Ordering::AcqRel);
                    return;
                }
                Err(_) => continue,
            }
        }
    }

    /// Remove flag atomically
    pub fn remove_flag(&self, flag: u16) {
        loop {
            let old_state = self.state.load(Ordering::Acquire);
            let (ctx_id, class, instance, old_flags) = Self::unpack_state(old_state);
            let new_flags = old_flags & !flag;
            let new_state = Self::pack_state(
                ctx_id,
                I915EngineClass::from_u8(class).unwrap_or(I915EngineClass::Render),
                instance,
                new_flags,
            );

            match self.state.compare_exchange_weak(
                old_state,
                new_state,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.generation.fetch_add(1, Ordering::AcqRel);
                    return;
                }
                Err(_) => continue,
            }
        }
    }

    /// Set priority
    pub fn set_priority(&self, priority: i64) {
        self.priority.store(priority as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Get priority
    #[inline]
    pub fn priority(&self) -> i64 {
        self.priority.load(Ordering::Acquire) as i64
    }

    /// Set preemption timeout
    pub fn set_preemption_timeout(&self, timeout_ns: u64) {
        self.preemption_timeout.store(timeout_ns, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Set recoverable
    pub fn set_recoverable(&self, recoverable: bool) {
        self.recoverable.store(recoverable as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Allocate next sequence number
    pub fn alloc_seqno(&self) -> u64 {
        let seqno = self.seqno.fetch_add(1, Ordering::AcqRel) + 1;
        self.batch_count.fetch_add(1, Ordering::Relaxed);
        seqno
    }

    /// Mark sequence number as completed
    pub fn complete_seqno(&self, seqno: u64) {
        // Only update if this is newer than current completed
        loop {
            let current = self.completed_seqno.load(Ordering::Acquire);
            if seqno <= current {
                return;
            }
            match self.completed_seqno.compare_exchange_weak(
                current,
                seqno,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return,
                Err(_) => continue,
            }
        }
    }

    /// Add execution time
    pub fn add_exec_time(&self, ns: u64) {
        self.exec_time_ns.fetch_add(ns, Ordering::Relaxed);
    }

    /// Record reset
    pub fn record_reset(&self, reason: ResetReason) {
        self.reset_count.fetch_add(1, Ordering::Relaxed);
        self.last_reset_reason.store(reason as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Mark as banned if too many resets
        if self.reset_count.load(Ordering::Acquire) >= 5 {
            self.add_flag(context_flags::BANNED);
            self.remove_flag(context_flags::ACTIVE);
        }
    }

    /// Mark context as destroyed
    pub fn destroy(&self) {
        self.remove_flag(context_flags::ACTIVE);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Verify size is 256 bytes
    pub const fn verify_size() {
        const_assert_eq(core::mem::size_of::<I915ContextCapsule>(), 256);
    }
}

impl ResetReason {
    /// Convert from u8
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::GpuHang,
            2 => Self::BatchTimeout,
            3 => Self::Banned,
            4 => Self::UserRequest,
            5 => Self::HardwareError,
            _ => Self::None,
        }
    }
}

// Compile-time size assertion
const fn const_assert_eq(a: usize, b: usize) {
    if a != b {
        panic!("Size mismatch");
    }
}

// ============================================================================
// I915Driver - Stateless ioctl Wrapper
// ============================================================================

/// Intel i915 driver interface
///
/// Provides stateless wrappers around i915 DRM ioctls.
/// All state is managed by capsules, this struct only provides operations.
pub struct I915Driver;

impl I915Driver {
    // ------------------------------------------------------------------------
    // Parameter Queries
    // ------------------------------------------------------------------------

    /// Query an i915 parameter
    ///
    /// # Arguments
    ///
    /// * `fd` - DRM device file descriptor
    /// * `param` - Parameter to query
    ///
    /// # Returns
    ///
    /// Parameter value or error
    ///
    /// # Safety
    ///
    /// #ASSUME_IOCTL_SAFE: fd is valid DRM device, param is valid I915Param
    /// #VERIFY_IOCTL_SAFE: Kernel validates fd and param, returns EINVAL on failure
    #[cfg(feature = "std")]
    pub fn getparam(fd: RawFd, param: I915Param) -> KgpuDriverResult<i32> {
        let mut value: i32 = 0;
        let mut args = I915Getparam {
            param: param as i32,
            _pad: 0,
            value: &mut value as *mut i32,
        };

        // #ASSUME_IOCTL_SAFE: ioctl with valid fd and properly initialized args
        // #VERIFY_IOCTL_SAFE: Kernel returns negative errno on failure
        let ret = unsafe {
            libc::ioctl(fd, IOCTL_I915_GETPARAM as libc::c_ulong, &mut args)
        };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(errno_to_error("i915_getparam", errno));
        }

        Ok(value)
    }

    /// Check if a feature is supported
    #[cfg(feature = "std")]
    pub fn has_feature(fd: RawFd, param: I915Param) -> bool {
        Self::getparam(fd, param).map(|v| v != 0).unwrap_or(false)
    }

    /// Get chipset ID (PCI device ID)
    #[cfg(feature = "std")]
    pub fn chipset_id(fd: RawFd) -> KgpuDriverResult<u32> {
        Self::getparam(fd, I915Param::ChipsetId).map(|v| v as u32)
    }

    /// Get EU count
    #[cfg(feature = "std")]
    pub fn eu_total(fd: RawFd) -> KgpuDriverResult<u32> {
        Self::getparam(fd, I915Param::EuTotal).map(|v| v as u32)
    }

    /// Get subslice count
    #[cfg(feature = "std")]
    pub fn subslice_total(fd: RawFd) -> KgpuDriverResult<u32> {
        Self::getparam(fd, I915Param::SubsliceTotal).map(|v| v as u32)
    }

    /// Get CS timestamp frequency
    #[cfg(feature = "std")]
    pub fn cs_timestamp_frequency(fd: RawFd) -> KgpuDriverResult<u64> {
        Self::getparam(fd, I915Param::CsTimestampFrequency).map(|v| v as u64)
    }

    // ------------------------------------------------------------------------
    // GEM Buffer Operations
    // ------------------------------------------------------------------------

    /// Create a GEM buffer object
    ///
    /// # Arguments
    ///
    /// * `fd` - DRM device file descriptor
    /// * `size` - Buffer size in bytes
    ///
    /// # Returns
    ///
    /// GEM handle on success
    ///
    /// # Safety
    ///
    /// #ASSUME_IOCTL_SAFE: fd is valid DRM device
    /// #VERIFY_IOCTL_SAFE: Kernel validates size, returns ENOMEM if too large
    #[cfg(feature = "std")]
    pub fn gem_create(fd: RawFd, size: u64) -> KgpuDriverResult<u32> {
        let mut args = I915GemCreate {
            size,
            handle: 0,
            _pad: 0,
        };

        // #ASSUME_IOCTL_SAFE: ioctl with valid fd and size
        // #VERIFY_IOCTL_SAFE: Kernel returns handle or error
        let ret = unsafe {
            libc::ioctl(fd, IOCTL_I915_GEM_CREATE as libc::c_ulong, &mut args)
        };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(errno_to_error("gem_create", errno));
        }

        Ok(args.handle)
    }

    /// Get mmap offset for a GEM buffer (Gen12+ preferred)
    ///
    /// # Arguments
    ///
    /// * `fd` - DRM device file descriptor
    /// * `handle` - GEM handle
    /// * `flags` - Mmap flags (mmap_offset_flags)
    ///
    /// # Returns
    ///
    /// Mmap offset for use with mmap()
    #[cfg(feature = "std")]
    pub fn gem_mmap_offset(fd: RawFd, handle: u32, flags: u64) -> KgpuDriverResult<u64> {
        let mut args = I915GemMmapOffset {
            handle,
            _pad: 0,
            offset: 0,
            flags,
            extensions: 0,
        };

        // #ASSUME_IOCTL_SAFE: ioctl with valid fd and handle
        // #VERIFY_IOCTL_SAFE: Kernel validates handle, returns offset
        let ret = unsafe {
            libc::ioctl(fd, IOCTL_I915_GEM_MMAP_OFFSET as libc::c_ulong, &mut args)
        };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(errno_to_error("gem_mmap_offset", errno));
        }

        Ok(args.offset)
    }

    /// Set tiling mode for a GEM buffer
    ///
    /// # Arguments
    ///
    /// * `fd` - DRM device file descriptor
    /// * `handle` - GEM handle
    /// * `mode` - Tiling mode
    /// * `stride` - Row stride in bytes
    ///
    /// # Returns
    ///
    /// Swizzle mode assigned by kernel
    #[cfg(feature = "std")]
    pub fn gem_set_tiling(
        fd: RawFd,
        handle: u32,
        mode: I915TilingMode,
        stride: u32,
    ) -> KgpuDriverResult<u32> {
        let mut args = I915GemSetTiling {
            handle,
            tiling_mode: mode as u32,
            stride,
            swizzle_mode: 0,
        };

        // #ASSUME_IOCTL_SAFE: ioctl with valid fd, handle, mode, stride
        // #VERIFY_IOCTL_SAFE: Kernel validates all args, returns swizzle
        let ret = unsafe {
            libc::ioctl(fd, IOCTL_I915_GEM_SET_TILING as libc::c_ulong, &mut args)
        };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(errno_to_error("gem_set_tiling", errno));
        }

        Ok(args.swizzle_mode)
    }

    /// Get tiling mode for a GEM buffer
    #[cfg(feature = "std")]
    pub fn gem_get_tiling(fd: RawFd, handle: u32) -> KgpuDriverResult<(I915TilingMode, u32)> {
        let mut args = I915GemGetTiling {
            handle,
            tiling_mode: 0,
            swizzle_mode: 0,
            phys_swizzle_mode: 0,
        };

        let ret = unsafe {
            libc::ioctl(fd, IOCTL_I915_GEM_GET_TILING as libc::c_ulong, &mut args)
        };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(errno_to_error("gem_get_tiling", errno));
        }

        let mode = I915TilingMode::from_u32(args.tiling_mode).unwrap_or(I915TilingMode::None);
        Ok((mode, args.swizzle_mode))
    }

    /// Close a GEM buffer handle
    #[cfg(feature = "std")]
    pub fn gem_close(fd: RawFd, handle: u32) -> KgpuDriverResult<()> {
        let args = I915GemClose {
            handle,
            _pad: 0,
        };

        let ret = unsafe {
            libc::ioctl(fd, IOCTL_I915_GEM_CLOSE as libc::c_ulong, &args)
        };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(errno_to_error("gem_close", errno));
        }

        Ok(())
    }

    /// Check if a GEM buffer is busy
    #[cfg(feature = "std")]
    pub fn gem_busy(fd: RawFd, handle: u32) -> KgpuDriverResult<u32> {
        let mut args = I915GemBusy {
            handle,
            busy: 0,
        };

        let ret = unsafe {
            libc::ioctl(fd, IOCTL_I915_GEM_BUSY as libc::c_ulong, &mut args)
        };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(errno_to_error("gem_busy", errno));
        }

        Ok(args.busy)
    }

    /// Wait for a GEM buffer to become idle
    ///
    /// # Arguments
    ///
    /// * `fd` - DRM device file descriptor
    /// * `handle` - GEM handle
    /// * `timeout_ns` - Timeout in nanoseconds (-1 for infinite)
    ///
    /// # Returns
    ///
    /// Remaining timeout on success (0 if timeout expired)
    #[cfg(feature = "std")]
    pub fn gem_wait(fd: RawFd, handle: u32, timeout_ns: i64) -> KgpuDriverResult<i64> {
        let mut args = I915GemWait {
            handle,
            flags: 0,
            timeout_ns,
        };

        let ret = unsafe {
            libc::ioctl(fd, IOCTL_I915_GEM_WAIT as libc::c_ulong, &mut args)
        };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            // ETIME means timeout expired, not an error
            if errno == libc::ETIME {
                return Ok(0);
            }
            return Err(errno_to_error("gem_wait", errno));
        }

        Ok(args.timeout_ns)
    }

    /// Throttle GEM operations (wait for ring to drain)
    #[cfg(feature = "std")]
    pub fn gem_throttle(fd: RawFd) -> KgpuDriverResult<()> {
        let ret = unsafe {
            libc::ioctl(fd, IOCTL_I915_GEM_THROTTLE as libc::c_ulong, core::ptr::null_mut::<()>())
        };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(errno_to_error("gem_throttle", errno));
        }

        Ok(())
    }

    // ------------------------------------------------------------------------
    // Context Operations
    // ------------------------------------------------------------------------

    /// Create a new GPU context
    ///
    /// # Arguments
    ///
    /// * `fd` - DRM device file descriptor
    /// * `flags` - Context creation flags
    ///
    /// # Returns
    ///
    /// Context ID on success
    #[cfg(feature = "std")]
    pub fn context_create(fd: RawFd, flags: u32) -> KgpuDriverResult<u32> {
        let mut args = I915GemContextCreate {
            ctx_id: 0,
            flags,
        };

        // #ASSUME_IOCTL_SAFE: ioctl with valid fd and flags
        // #VERIFY_IOCTL_SAFE: Kernel creates context, returns ID
        let ret = unsafe {
            libc::ioctl(fd, IOCTL_I915_GEM_CONTEXT_CREATE as libc::c_ulong, &mut args)
        };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(errno_to_error("context_create", errno));
        }

        Ok(args.ctx_id)
    }

    /// Create a context with capsule tracking
    #[cfg(feature = "std")]
    pub fn context_create_capsule(
        fd: RawFd,
        engine_class: I915EngineClass,
        engine_instance: u8,
        flags: u32,
    ) -> KgpuDriverResult<I915ContextCapsule> {
        let ctx_id = Self::context_create(fd, flags)?;
        Ok(I915ContextCapsule::new(ctx_id, engine_class, engine_instance))
    }

    /// Destroy a GPU context
    #[cfg(feature = "std")]
    pub fn context_destroy(fd: RawFd, ctx_id: u32) -> KgpuDriverResult<()> {
        let args = I915GemContextDestroy {
            ctx_id,
            _pad: 0,
        };

        let ret = unsafe {
            libc::ioctl(fd, IOCTL_I915_GEM_CONTEXT_DESTROY as libc::c_ulong, &args)
        };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(errno_to_error("context_destroy", errno));
        }

        Ok(())
    }

    /// Destroy context and update capsule
    #[cfg(feature = "std")]
    pub fn context_destroy_capsule(fd: RawFd, capsule: &I915ContextCapsule) -> KgpuDriverResult<()> {
        let ctx_id = capsule.ctx_id();
        Self::context_destroy(fd, ctx_id)?;
        capsule.destroy();
        Ok(())
    }

    /// Get context parameter
    #[cfg(feature = "std")]
    pub fn context_getparam(
        fd: RawFd,
        ctx_id: u32,
        param: I915ContextParam,
    ) -> KgpuDriverResult<u64> {
        let mut args = I915GemContextParam {
            ctx_id,
            size: 0,
            param: param as u64,
            value: 0,
        };

        let ret = unsafe {
            libc::ioctl(fd, IOCTL_I915_GEM_CONTEXT_GETPARAM as libc::c_ulong, &mut args)
        };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(errno_to_error("context_getparam", errno));
        }

        Ok(args.value)
    }

    /// Set context parameter
    #[cfg(feature = "std")]
    pub fn context_setparam(
        fd: RawFd,
        ctx_id: u32,
        param: I915ContextParam,
        value: u64,
    ) -> KgpuDriverResult<()> {
        let mut args = I915GemContextParam {
            ctx_id,
            size: 0,
            param: param as u64,
            value,
        };

        let ret = unsafe {
            libc::ioctl(fd, IOCTL_I915_GEM_CONTEXT_SETPARAM as libc::c_ulong, &mut args)
        };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(errno_to_error("context_setparam", errno));
        }

        Ok(())
    }

    /// Set context priority
    #[cfg(feature = "std")]
    pub fn context_set_priority(fd: RawFd, ctx_id: u32, priority: i64) -> KgpuDriverResult<()> {
        // Priority is signed, but ioctl uses u64
        Self::context_setparam(fd, ctx_id, I915ContextParam::Priority, priority as u64)
    }

    /// Set context priority and update capsule
    #[cfg(feature = "std")]
    pub fn context_set_priority_capsule(
        fd: RawFd,
        capsule: &I915ContextCapsule,
        priority: i64,
    ) -> KgpuDriverResult<()> {
        Self::context_set_priority(fd, capsule.ctx_id(), priority)?;
        capsule.set_priority(priority);
        Ok(())
    }

    /// Set context recoverable flag
    #[cfg(feature = "std")]
    pub fn context_set_recoverable(
        fd: RawFd,
        ctx_id: u32,
        recoverable: bool,
    ) -> KgpuDriverResult<()> {
        Self::context_setparam(fd, ctx_id, I915ContextParam::Recoverable, recoverable as u64)
    }

    /// Set context recoverable and update capsule
    #[cfg(feature = "std")]
    pub fn context_set_recoverable_capsule(
        fd: RawFd,
        capsule: &I915ContextCapsule,
        recoverable: bool,
    ) -> KgpuDriverResult<()> {
        Self::context_set_recoverable(fd, capsule.ctx_id(), recoverable)?;
        capsule.set_recoverable(recoverable);
        Ok(())
    }

    // ------------------------------------------------------------------------
    // Command Submission
    // ------------------------------------------------------------------------

    /// Submit commands via EXECBUFFER2
    ///
    /// # Arguments
    ///
    /// * `fd` - DRM device file descriptor
    /// * `ctx_id` - Context ID (0 for default)
    /// * `buffers` - Exec objects array
    /// * `batch_handle` - Handle of batch buffer
    /// * `batch_len` - Length of batch in bytes
    /// * `flags` - Execution flags
    ///
    /// # Returns
    ///
    /// Output fence (if FENCE_OUT flag set), otherwise 0
    ///
    /// # Safety
    ///
    /// #ASSUME_BATCH_VALID: Batch buffer contains valid GPU commands
    /// #VERIFY_BATCH_VALID: Kernel validates command stream, rejects invalid
    #[cfg(feature = "std")]
    pub fn execbuffer2(
        fd: RawFd,
        ctx_id: u32,
        buffers: &mut [I915GemExecObject2],
        batch_start_offset: u32,
        batch_len: u32,
        flags: u64,
    ) -> KgpuDriverResult<i32> {
        if buffers.is_empty() {
            return Err(invalid_argument_error());
        }

        // Context ID is encoded in flags for newer kernels
        let flags_with_ctx = flags | ((ctx_id as u64) << 32);

        let mut args = I915GemExecbuffer2 {
            buffers_ptr: buffers.as_ptr() as u64,
            buffer_count: buffers.len() as u32,
            batch_start_offset,
            batch_len,
            dr1: 0,
            dr4: 0,
            num_cliprects: 0,
            cliprects_ptr: 0,
            flags: flags_with_ctx,
            rsvd1: 0,
            rsvd2: 0,
        };

        // #ASSUME_IOCTL_SAFE: ioctl with valid fd and properly formed execbuffer
        // #VERIFY_IOCTL_SAFE: Kernel validates all buffers and batch, returns error
        let ret = unsafe {
            libc::ioctl(fd, IOCTL_I915_GEM_EXECBUFFER2 as libc::c_ulong, &mut args)
        };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(errno_to_error("execbuffer2", errno));
        }

        // Return fence if requested
        if flags & exec_flags::FENCE_OUT != 0 {
            Ok(args.rsvd1 as i32)
        } else {
            Ok(0)
        }
    }

    /// Submit commands with capsule tracking
    #[cfg(feature = "std")]
    pub fn execbuffer2_capsule(
        fd: RawFd,
        capsule: &I915ContextCapsule,
        buffers: &mut [I915GemExecObject2],
        batch_start_offset: u32,
        batch_len: u32,
        flags: u64,
    ) -> KgpuDriverResult<(u64, i32)> {
        let seqno = capsule.alloc_seqno();
        let fence = Self::execbuffer2(
            fd,
            capsule.ctx_id(),
            buffers,
            batch_start_offset,
            batch_len,
            flags,
        )?;
        Ok((seqno, fence))
    }

    // ------------------------------------------------------------------------
    // Query Operations
    // ------------------------------------------------------------------------

    /// Query driver information
    #[cfg(feature = "std")]
    pub fn query(fd: RawFd, items: &mut [I915QueryItem]) -> KgpuDriverResult<()> {
        if items.is_empty() {
            return Ok(());
        }

        let mut args = I915Query {
            num_items: items.len() as u32,
            flags: 0,
            items_ptr: items.as_ptr() as u64,
        };

        let ret = unsafe {
            libc::ioctl(fd, IOCTL_I915_QUERY as libc::c_ulong, &mut args)
        };

        if ret < 0 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(errno_to_error("query", errno));
        }

        Ok(())
    }

    /// Query engine topology
    #[cfg(feature = "std")]
    pub fn query_engines(fd: RawFd) -> KgpuDriverResult<EngineInfo> {
        // First query to get size
        let mut item = I915QueryItem {
            query_id: query_id::ENGINE_INFO,
            length: 0,
            flags: 0,
            data_ptr: 0,
        };

        Self::query(fd, core::slice::from_mut(&mut item))?;

        if item.length <= 0 {
            return Err(query_error());
        }

        // Allocate buffer and query again
        let mut buffer = vec![0u8; item.length as usize];
        item.data_ptr = buffer.as_ptr() as u64;

        Self::query(fd, core::slice::from_mut(&mut item))?;

        // Parse engine info
        EngineInfo::parse(&buffer)
    }

    // ------------------------------------------------------------------------
    // Firmware Status
    // ------------------------------------------------------------------------

    /// Check GuC submission status
    #[cfg(feature = "std")]
    pub fn guc_status(fd: RawFd) -> KgpuDriverResult<bool> {
        Self::has_feature(fd, I915Param::HasGucSubmission).then_some(true).ok_or_else(|| {
            query_error()
        })
    }

    /// Get HuC authentication status
    ///
    /// Returns true if HuC is authenticated and ready
    #[cfg(feature = "std")]
    pub fn huc_status(fd: RawFd) -> KgpuDriverResult<bool> {
        // HuC status is queried via debugfs or specific param
        // For now, check if protected content is available (implies HuC)
        Ok(Self::has_feature(fd, I915Param::HasContextIsolation))
    }
}

// ============================================================================
// Engine Info
// ============================================================================

/// Parsed engine information
#[derive(Debug, Clone)]
pub struct EngineInfo {
    /// Available engines
    pub engines: Vec<EngineInstance>,
}

/// Single engine instance
#[derive(Debug, Clone, Copy)]
pub struct EngineInstance {
    /// Engine class
    pub class: I915EngineClass,
    /// Instance number within class
    pub instance: u16,
    /// Engine flags
    pub flags: u64,
    /// Capabilities
    pub capabilities: u64,
}

impl EngineInfo {
    /// Parse engine info from query buffer
    fn parse(data: &[u8]) -> KgpuDriverResult<Self> {
        if data.len() < 8 {
            return Err(query_error());
        }

        // First 4 bytes: number of engines
        let num_engines = u32::from_ne_bytes([data[0], data[1], data[2], data[3]]) as usize;

        // Each engine entry is 24 bytes
        // struct drm_i915_engine_info {
        //     struct i915_engine_class_instance engine;  // 4 bytes
        //     __u32 rsvd0;
        //     __u64 flags;
        //     __u64 capabilities;
        // }

        let mut engines = Vec::with_capacity(num_engines);
        let mut offset = 8; // Skip header

        for _ in 0..num_engines {
            if offset + 24 > data.len() {
                break;
            }

            let class_u16 = u16::from_ne_bytes([data[offset], data[offset + 1]]);
            let instance = u16::from_ne_bytes([data[offset + 2], data[offset + 3]]);
            let flags = u64::from_ne_bytes([
                data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11],
                data[offset + 12], data[offset + 13], data[offset + 14], data[offset + 15],
            ]);
            let capabilities = u64::from_ne_bytes([
                data[offset + 16], data[offset + 17], data[offset + 18], data[offset + 19],
                data[offset + 20], data[offset + 21], data[offset + 22], data[offset + 23],
            ]);

            if let Some(class) = I915EngineClass::from_u8(class_u16 as u8) {
                engines.push(EngineInstance {
                    class,
                    instance,
                    flags,
                    capabilities,
                });
            }

            offset += 24;
        }

        Ok(Self { engines })
    }

    /// Get engines by class
    pub fn by_class(&self, class: I915EngineClass) -> impl Iterator<Item = &EngineInstance> {
        self.engines.iter().filter(move |e| e.class == class)
    }

    /// Count engines by class
    pub fn count_class(&self, class: I915EngineClass) -> usize {
        self.by_class(class).count()
    }

    /// Get first engine of class
    pub fn first_of_class(&self, class: I915EngineClass) -> Option<&EngineInstance> {
        self.by_class(class).next()
    }
}

// ============================================================================
// Error Helpers
// ============================================================================

/// Map errno to KgpuDriverError
fn errno_to_error(_name: &str, errno: i32) -> KgpuDriverError {
    match errno {
        libc::ENOENT => KgpuDriverError::DeviceNotFound,
        libc::ENOMEM => KgpuDriverError::OutOfDeviceMemory,
        libc::EACCES | libc::EPERM => KgpuDriverError::PermissionDenied,
        libc::EINVAL => KgpuDriverError::InvalidParameter,
        libc::EBUSY => KgpuDriverError::DeviceBusy,
        libc::ETIMEDOUT | libc::ETIME => KgpuDriverError::CommandTimeout,
        libc::ENODEV => KgpuDriverError::DeviceNotSupported,
        libc::EBADF => KgpuDriverError::InvalidMemoryHandle,
        _ => KgpuDriverError::DrmIoctlFailed,
    }
}

/// Error for invalid arguments
fn invalid_argument_error() -> KgpuDriverError {
    KgpuDriverError::InvalidParameter
}

/// Error for query failures
fn query_error() -> KgpuDriverError {
    KgpuDriverError::DrmIoctlFailed
}

// ============================================================================
// Vec for no_std
// ============================================================================

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg(not(feature = "std"))]
use alloc::format;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // Q1-Q7: Unit Tests (Basic Functionality)
    // ------------------------------------------------------------------------

    #[test]
    fn test_engine_class_values() {
        assert_eq!(I915EngineClass::Render as u8, 0);
        assert_eq!(I915EngineClass::Copy as u8, 1);
        assert_eq!(I915EngineClass::Video as u8, 2);
        assert_eq!(I915EngineClass::VideoEnhance as u8, 3);
        assert_eq!(I915EngineClass::Compute as u8, 4);
    }

    #[test]
    fn test_engine_class_from_u8() {
        assert_eq!(I915EngineClass::from_u8(0), Some(I915EngineClass::Render));
        assert_eq!(I915EngineClass::from_u8(1), Some(I915EngineClass::Copy));
        assert_eq!(I915EngineClass::from_u8(2), Some(I915EngineClass::Video));
        assert_eq!(I915EngineClass::from_u8(3), Some(I915EngineClass::VideoEnhance));
        assert_eq!(I915EngineClass::from_u8(4), Some(I915EngineClass::Compute));
        assert_eq!(I915EngineClass::from_u8(5), None);
        assert_eq!(I915EngineClass::from_u8(255), None);
    }

    #[test]
    fn test_engine_class_names() {
        assert_eq!(I915EngineClass::Render.name(), "rcs");
        assert_eq!(I915EngineClass::Copy.name(), "bcs");
        assert_eq!(I915EngineClass::Video.name(), "vcs");
        assert_eq!(I915EngineClass::VideoEnhance.name(), "vecs");
        assert_eq!(I915EngineClass::Compute.name(), "ccs");
    }

    #[test]
    fn test_tiling_mode_values() {
        assert_eq!(I915TilingMode::None as u32, 0);
        assert_eq!(I915TilingMode::X as u32, 1);
        assert_eq!(I915TilingMode::Y as u32, 2);
        assert_eq!(I915TilingMode::Yf as u32, 3);
        assert_eq!(I915TilingMode::Ys as u32, 4);
    }

    #[test]
    fn test_tiling_mode_from_u32() {
        assert_eq!(I915TilingMode::from_u32(0), Some(I915TilingMode::None));
        assert_eq!(I915TilingMode::from_u32(1), Some(I915TilingMode::X));
        assert_eq!(I915TilingMode::from_u32(2), Some(I915TilingMode::Y));
        assert_eq!(I915TilingMode::from_u32(3), Some(I915TilingMode::Yf));
        assert_eq!(I915TilingMode::from_u32(4), Some(I915TilingMode::Ys));
        assert_eq!(I915TilingMode::from_u32(5), None);
    }

    #[test]
    fn test_tiling_dimensions() {
        assert_eq!(I915TilingMode::None.tile_width(), 1);
        assert_eq!(I915TilingMode::None.tile_height(), 1);

        assert_eq!(I915TilingMode::X.tile_width(), 512);
        assert_eq!(I915TilingMode::X.tile_height(), 8);

        assert_eq!(I915TilingMode::Y.tile_width(), 128);
        assert_eq!(I915TilingMode::Y.tile_height(), 32);

        assert_eq!(I915TilingMode::Ys.tile_width(), 256);
        assert_eq!(I915TilingMode::Ys.tile_height(), 64);
    }

    #[test]
    fn test_param_values() {
        assert_eq!(I915Param::ChipsetId as i32, 1);
        assert_eq!(I915Param::HasGem as i32, 2);
        assert_eq!(I915Param::EuTotal as i32, 26);
        assert_eq!(I915Param::HasVmBind as i32, 66);
    }

    #[test]
    fn test_context_flags() {
        assert_eq!(context_flags::ACTIVE, 1);
        assert_eq!(context_flags::BANNED, 2);
        assert_eq!(context_flags::PROTECTED, 4);
        assert_eq!(context_flags::PERSISTENT, 8);
        assert_eq!(context_flags::CUSTOM_SSEU, 16);
        assert_eq!(context_flags::VM_BIND, 32);
    }

    #[test]
    fn test_reset_reason_from_u8() {
        assert_eq!(ResetReason::from_u8(0), ResetReason::None);
        assert_eq!(ResetReason::from_u8(1), ResetReason::GpuHang);
        assert_eq!(ResetReason::from_u8(2), ResetReason::BatchTimeout);
        assert_eq!(ResetReason::from_u8(3), ResetReason::Banned);
        assert_eq!(ResetReason::from_u8(4), ResetReason::UserRequest);
        assert_eq!(ResetReason::from_u8(5), ResetReason::HardwareError);
        assert_eq!(ResetReason::from_u8(255), ResetReason::None);
    }

    // ------------------------------------------------------------------------
    // Q8-Q14: Capsule Tests (State Management)
    // ------------------------------------------------------------------------

    #[test]
    fn test_context_capsule_size() {
        assert_eq!(core::mem::size_of::<I915ContextCapsule>(), 256);
        assert_eq!(core::mem::align_of::<I915ContextCapsule>(), 256);
    }

    #[test]
    fn test_context_capsule_new() {
        let capsule = I915ContextCapsule::new(42, I915EngineClass::Render, 0);

        assert_eq!(capsule.ctx_id(), 42);
        assert_eq!(capsule.engine_class(), I915EngineClass::Render);
        assert_eq!(capsule.engine_instance(), 0);
        assert!(capsule.is_active());
        assert!(!capsule.is_banned());
        assert_eq!(capsule.generation(), 1);
    }

    #[test]
    fn test_context_capsule_state_packing() {
        let capsule = I915ContextCapsule::new(0xDEADBEEF, I915EngineClass::Video, 3);

        assert_eq!(capsule.ctx_id(), 0xDEADBEEF);
        assert_eq!(capsule.engine_class(), I915EngineClass::Video);
        assert_eq!(capsule.engine_instance(), 3);
    }

    #[test]
    fn test_context_capsule_snapshot() {
        let capsule = I915ContextCapsule::new(123, I915EngineClass::Copy, 1);
        capsule.set_priority(-100);

        let snapshot = capsule.snapshot();

        assert_eq!(snapshot.ctx_id, 123);
        assert_eq!(snapshot.engine_class, I915EngineClass::Copy);
        assert_eq!(snapshot.engine_instance, 1);
        assert_eq!(snapshot.priority, -100);
        assert!(snapshot.recoverable);
        assert_eq!(snapshot.seqno, 0);
        assert_eq!(snapshot.batch_count, 0);
    }

    #[test]
    fn test_context_capsule_flags() {
        let capsule = I915ContextCapsule::new(1, I915EngineClass::Render, 0);

        assert!(capsule.is_active());
        assert!(!capsule.is_banned());

        capsule.add_flag(context_flags::PROTECTED);
        assert_eq!(capsule.flags() & context_flags::PROTECTED, context_flags::PROTECTED);

        capsule.add_flag(context_flags::BANNED);
        assert!(capsule.is_banned());

        capsule.remove_flag(context_flags::ACTIVE);
        assert!(!capsule.is_active());
    }

    #[test]
    fn test_context_capsule_seqno() {
        let capsule = I915ContextCapsule::new(1, I915EngineClass::Render, 0);

        assert_eq!(capsule.alloc_seqno(), 1);
        assert_eq!(capsule.alloc_seqno(), 2);
        assert_eq!(capsule.alloc_seqno(), 3);

        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.batch_count, 3);
    }

    #[test]
    fn test_context_capsule_complete_seqno() {
        let capsule = I915ContextCapsule::new(1, I915EngineClass::Render, 0);

        capsule.alloc_seqno(); // 1
        capsule.alloc_seqno(); // 2
        capsule.alloc_seqno(); // 3

        capsule.complete_seqno(2);
        assert_eq!(capsule.snapshot().completed_seqno, 2);

        // Should not go backwards
        capsule.complete_seqno(1);
        assert_eq!(capsule.snapshot().completed_seqno, 2);

        capsule.complete_seqno(3);
        assert_eq!(capsule.snapshot().completed_seqno, 3);
    }

    #[test]
    fn test_context_capsule_reset() {
        let capsule = I915ContextCapsule::new(1, I915EngineClass::Render, 0);

        capsule.record_reset(ResetReason::GpuHang);
        assert_eq!(capsule.snapshot().reset_count, 1);
        assert_eq!(capsule.snapshot().last_reset_reason, ResetReason::GpuHang);

        // After 5 resets, should be banned
        for _ in 0..4 {
            capsule.record_reset(ResetReason::BatchTimeout);
        }

        assert!(capsule.is_banned());
        assert!(!capsule.is_active());
    }

    #[test]
    fn test_context_capsule_destroy() {
        let capsule = I915ContextCapsule::new(1, I915EngineClass::Render, 0);

        assert!(capsule.is_active());
        capsule.destroy();
        assert!(!capsule.is_active());
    }

    #[test]
    fn test_context_capsule_priority() {
        let capsule = I915ContextCapsule::new(1, I915EngineClass::Render, 0);

        assert_eq!(capsule.priority(), 0);

        capsule.set_priority(1023);
        assert_eq!(capsule.priority(), 1023);

        capsule.set_priority(-1023);
        assert_eq!(capsule.priority(), -1023);
    }

    #[test]
    fn test_context_capsule_exec_time() {
        let capsule = I915ContextCapsule::new(1, I915EngineClass::Render, 0);

        capsule.add_exec_time(1000);
        capsule.add_exec_time(2000);
        capsule.add_exec_time(3000);

        assert_eq!(capsule.snapshot().exec_time_ns, 6000);
    }

    #[test]
    fn test_context_capsule_generation_increment() {
        let capsule = I915ContextCapsule::new(1, I915EngineClass::Render, 0);
        let initial_gen = capsule.generation();

        capsule.set_priority(100);
        assert_eq!(capsule.generation(), initial_gen + 1);

        capsule.add_flag(context_flags::PROTECTED);
        assert_eq!(capsule.generation(), initial_gen + 2);

        capsule.remove_flag(context_flags::PROTECTED);
        assert_eq!(capsule.generation(), initial_gen + 3);
    }

    // ------------------------------------------------------------------------
    // Q15-Q21: Structure Tests (Memory Layout)
    // ------------------------------------------------------------------------

    #[test]
    fn test_gem_create_size() {
        assert_eq!(core::mem::size_of::<I915GemCreate>(), 16);
    }

    #[test]
    fn test_gem_mmap_offset_size() {
        assert_eq!(core::mem::size_of::<I915GemMmapOffset>(), 32);
    }

    #[test]
    fn test_gem_set_tiling_size() {
        assert_eq!(core::mem::size_of::<I915GemSetTiling>(), 16);
    }

    #[test]
    fn test_gem_exec_object2_size() {
        assert_eq!(core::mem::size_of::<I915GemExecObject2>(), 56);
    }

    #[test]
    fn test_gem_execbuffer2_size() {
        assert_eq!(core::mem::size_of::<I915GemExecbuffer2>(), 64);
    }

    #[test]
    fn test_gem_context_create_size() {
        assert_eq!(core::mem::size_of::<I915GemContextCreate>(), 8);
    }

    #[test]
    fn test_gem_context_param_size() {
        assert_eq!(core::mem::size_of::<I915GemContextParam>(), 24);
    }

    #[test]
    fn test_getparam_size() {
        assert_eq!(core::mem::size_of::<I915Getparam>(), 16);
    }

    #[test]
    fn test_query_item_size() {
        assert_eq!(core::mem::size_of::<I915QueryItem>(), 24);
    }

    #[test]
    fn test_query_size() {
        assert_eq!(core::mem::size_of::<I915Query>(), 16);
    }

    // ------------------------------------------------------------------------
    // Q22-Q28: ioctl Tests (Encoding)
    // ------------------------------------------------------------------------

    #[test]
    fn test_ioctl_encoding_iow() {
        let ioctl = drm_ioctl_iow(DRM_I915_GEM_EXECBUFFER2, 104);
        // Direction: 1 (write) at bits 30-31
        // Size: 104 at bits 16-29
        // Type: 'd' (0x64) at bits 8-15
        // Number: 0x40 + 0x29 = 0x69 at bits 0-7
        assert_eq!(ioctl & 0xFF, 0x69); // Number
        assert_eq!((ioctl >> 8) & 0xFF, 0x64); // Type 'd'
        assert_eq!((ioctl >> 16) & 0x3FFF, 104); // Size
        assert_eq!((ioctl >> 30) & 0x3, 1); // Direction (write)
    }

    #[test]
    fn test_ioctl_encoding_iowr() {
        let ioctl = drm_ioctl_iowr(DRM_I915_GEM_CREATE, 16);
        // Direction: 3 (read|write) at bits 30-31
        assert_eq!((ioctl >> 30) & 0x3, 3);
        assert_eq!((ioctl >> 16) & 0x3FFF, 16);
    }

    #[test]
    fn test_ioctl_encoding_ior() {
        let ioctl = drm_ioctl_ior(DRM_I915_GETPARAM, 16);
        // Direction: 2 (read) at bits 30-31
        assert_eq!((ioctl >> 30) & 0x3, 2);
    }

    #[test]
    fn test_ioctl_encoding_io() {
        let ioctl = drm_ioctl_io(DRM_I915_GEM_THROTTLE);
        // Direction: 0 (none) at bits 30-31
        assert_eq!((ioctl >> 30) & 0x3, 0);
        assert_eq!(ioctl & 0xFF, (0x40 + DRM_I915_GEM_THROTTLE) as u64);
    }

    #[test]
    fn test_exec_flags() {
        assert_eq!(exec_flags::DEFAULT, 0);
        assert_eq!(exec_flags::RENDER, 1);
        assert_eq!(exec_flags::BSD, 2);
        assert_eq!(exec_flags::BLT, 3);
        assert_eq!(exec_flags::VEBOX, 4);
        assert_eq!(exec_flags::NO_RELOC, 1 << 11);
        assert_eq!(exec_flags::HANDLE_LUT, 1 << 12);
        assert_eq!(exec_flags::FENCE_OUT, 1 << 17);
    }

    #[test]
    fn test_exec_object_flags() {
        assert_eq!(exec_object_flags::NEEDS_FENCE, 1);
        assert_eq!(exec_object_flags::NEEDS_GTT, 2);
        assert_eq!(exec_object_flags::WRITE, 4);
        assert_eq!(exec_object_flags::PINNED, 16);
    }

    #[test]
    fn test_mmap_offset_flags() {
        assert_eq!(mmap_offset_flags::GTT, 0);
        assert_eq!(mmap_offset_flags::WC, 1);
        assert_eq!(mmap_offset_flags::WB, 2);
        assert_eq!(mmap_offset_flags::UC, 3);
        assert_eq!(mmap_offset_flags::FIXED, 4);
    }

    #[test]
    fn test_context_create_flags() {
        assert_eq!(context_create_flags::RECOVERABLE, 1);
        assert_eq!(context_create_flags::USE_EXTENSIONS, 2);
    }

    // ------------------------------------------------------------------------
    // Q29-Q35: Engine Info Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_engine_instance_struct() {
        let instance = EngineInstance {
            class: I915EngineClass::Render,
            instance: 0,
            flags: 0,
            capabilities: 0xFF,
        };

        assert_eq!(instance.class, I915EngineClass::Render);
        assert_eq!(instance.instance, 0);
        assert_eq!(instance.capabilities, 0xFF);
    }

    #[test]
    fn test_engine_info_empty() {
        let data = [0u8; 8];
        let info = EngineInfo::parse(&data).unwrap();
        assert!(info.engines.is_empty());
    }

    #[test]
    fn test_engine_info_by_class() {
        let info = EngineInfo {
            engines: vec![
                EngineInstance { class: I915EngineClass::Render, instance: 0, flags: 0, capabilities: 0 },
                EngineInstance { class: I915EngineClass::Copy, instance: 0, flags: 0, capabilities: 0 },
                EngineInstance { class: I915EngineClass::Video, instance: 0, flags: 0, capabilities: 0 },
                EngineInstance { class: I915EngineClass::Video, instance: 1, flags: 0, capabilities: 0 },
            ],
        };

        assert_eq!(info.count_class(I915EngineClass::Render), 1);
        assert_eq!(info.count_class(I915EngineClass::Copy), 1);
        assert_eq!(info.count_class(I915EngineClass::Video), 2);
        assert_eq!(info.count_class(I915EngineClass::Compute), 0);
    }

    #[test]
    fn test_engine_info_first_of_class() {
        let info = EngineInfo {
            engines: vec![
                EngineInstance { class: I915EngineClass::Render, instance: 0, flags: 0, capabilities: 0 },
                EngineInstance { class: I915EngineClass::Video, instance: 0, flags: 0, capabilities: 0 },
                EngineInstance { class: I915EngineClass::Video, instance: 1, flags: 0, capabilities: 0 },
            ],
        };

        let render = info.first_of_class(I915EngineClass::Render).unwrap();
        assert_eq!(render.instance, 0);

        let video = info.first_of_class(I915EngineClass::Video).unwrap();
        assert_eq!(video.instance, 0);

        assert!(info.first_of_class(I915EngineClass::Compute).is_none());
    }

    // ------------------------------------------------------------------------
    // Additional Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_context_param_values() {
        assert_eq!(I915ContextParam::BanPeriod as u32, 0x1);
        assert_eq!(I915ContextParam::Recoverable as u32, 0x3);
        assert_eq!(I915ContextParam::Priority as u32, 0x4);
        assert_eq!(I915ContextParam::Sseu as u32, 0x5);
        assert_eq!(I915ContextParam::Vm as u32, 0x6);
        assert_eq!(I915ContextParam::Engines as u32, 0x7);
        assert_eq!(I915ContextParam::Persistence as u32, 0x8);
        assert_eq!(I915ContextParam::Protected as u32, 0x9);
    }

    #[test]
    fn test_query_ids() {
        assert_eq!(query_id::TOPOLOGY_INFO, 1);
        assert_eq!(query_id::ENGINE_INFO, 2);
        assert_eq!(query_id::PERF_INFO, 3);
        assert_eq!(query_id::MEMORY_REGIONS, 4);
        assert_eq!(query_id::HWCONFIG, 5);
    }

    #[test]
    fn test_state_pack_unpack_roundtrip() {
        let ctx_id = 0xABCD1234u32;
        let class = I915EngineClass::Video;
        let instance = 7u8;
        let flags = context_flags::ACTIVE | context_flags::PROTECTED;

        let packed = I915ContextCapsule::pack_state(ctx_id, class, instance, flags);
        let (u_ctx_id, u_class, u_instance, u_flags) = I915ContextCapsule::unpack_state(packed);

        assert_eq!(u_ctx_id, ctx_id);
        assert_eq!(u_class, class as u8);
        assert_eq!(u_instance, instance);
        assert_eq!(u_flags, flags);
    }

    #[test]
    fn test_capsule_concurrent_seqno_alloc() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(I915ContextCapsule::new(1, I915EngineClass::Render, 0));
        let mut handles = vec![];

        for _ in 0..4 {
            let c = Arc::clone(&capsule);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    c.alloc_seqno();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // Should have exactly 4000 allocations
        assert_eq!(capsule.snapshot().batch_count, 4000);
        // Seqno should be 4000
        assert_eq!(capsule.snapshot().seqno, 4000);
    }

    #[test]
    fn test_capsule_concurrent_flag_modification() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(I915ContextCapsule::new(1, I915EngineClass::Render, 0));
        let mut handles = vec![];

        // Thread 1: Add protected flag
        let c1 = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                c1.add_flag(context_flags::PROTECTED);
                std::thread::yield_now();
                c1.remove_flag(context_flags::PROTECTED);
            }
        }));

        // Thread 2: Add persistent flag
        let c2 = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                c2.add_flag(context_flags::PERSISTENT);
                std::thread::yield_now();
                c2.remove_flag(context_flags::PERSISTENT);
            }
        }));

        for h in handles {
            h.join().unwrap();
        }

        // Context should still be active and not banned
        assert!(capsule.is_active());
        assert!(!capsule.is_banned());
    }
}
