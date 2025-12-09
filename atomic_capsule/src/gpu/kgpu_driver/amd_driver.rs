//! AMD amdgpu DRM Driver Integration - KGPU-Driver v2.0 Phase 5
//!
//! Provides AMD-specific DRM operations for the amdgpu kernel driver, wrapping
//! amdgpu-specific ioctls for GEM buffer creation, command submission (CS) via IB,
//! context creation, and hardware IP queries.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                        Application / KGPU API                          │
//! └─────────────────────────────────────────────────────────────────────────┘
//!                                    │
//!                                    ▼
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                    AmdgpuDriver (this module)                           │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌───────────────┐  │
//! │  │ GEM Ops     │  │ Context Ops │  │ CS Submit   │  │ Info Queries  │  │
//! │  │ create/mmap │  │ create/free │  │ via IB      │  │ VRAM/GTT/etc  │  │
//! │  └─────────────┘  └─────────────┘  └─────────────┘  └───────────────┘  │
//! │                                    │                                    │
//! │  ┌────────────────────────────────────────────────────────────────────┐ │
//! │  │ AmdgpuContextCapsule (256B, T1 Atomic, lockfree state management) │ │
//! │  └────────────────────────────────────────────────────────────────────┘ │
//! └─────────────────────────────────────────────────────────────────────────┘
//!                                    │
//!                                    ▼ (ioctl)
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                     Linux Kernel amdgpu Driver                          │
//! │                     /dev/dri/card* | /dev/dri/renderD*                  │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Chaos Mandate
//!
//! - **100% Lockfree**: NO mutex, NO RwLock - atomics only
//! - **DualAtomicU64 Pattern**: State + generation counter in single atomic
//! - **256B Alignment**: 4 cache lines for optimal memory bandwidth
//! - **T1 Atomic Tier**: <100ns state operations
//!
//! # Memory Domains
//!
//! AMD GPUs have multiple memory domains:
//! - **VRAM**: Fast GPU-local video memory
//! - **GTT**: System memory accessible via GART
//! - **CPU**: CPU-visible memory for uploads
//! - **GDS**: Global Data Share (compute-only)
//! - **GWS**: Global Wave Sync (compute-only)
//! - **OA**: Ordered Append (for append buffers)
//!
//! # Hardware IP Types
//!
//! - **GFX**: Graphics engine (3D rendering)
//! - **SDMA**: System DMA for memory transfers
//! - **VCN**: Video Core Next (encode/decode)
//! - **VPE**: Video Processing Engine (RDNA3+)
//! - **JPEG**: JPEG encode/decode
//!
//! # ASSUM Tags
//!
//! - `#ASSUME_FD_VALID`: File descriptor is valid open amdgpu device
//! - `#ASSUME_IOCTL_SAFE`: amdgpu ioctls follow documented behavior
//! - `#ASSUME_BO_VALID`: Buffer object handles are valid
//! - `#ASSUME_CTX_VALID`: Context handles are valid
//! - `#ASSUME_ATOMIC_ALIGNED`: All AtomicU64/U32 fields are properly aligned
//!
//! # UCE34 Compliance
//!
//! - **Q10**: T1 Atomic tier (lockfree coordination)
//! - **Q33**: ComputationalCapsule verification (256B, generation counters)
//! - **Q34**: Audit trail design (submit_count, error_count for SOX/SOC2)

#![allow(dead_code)] // Allow during development

use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use core::fmt;

#[cfg(feature = "std")]
extern crate std;
#[cfg(feature = "std")]
use std::vec::Vec;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use super::error::{KgpuDriverError, KgpuDriverResult};
use super::amd_ring::{AmdCpRingCapsule, AmdQueueType};
use super::vendor::GpuGeneration;

// ============================================================================
// amdgpu ioctl Constants (from linux/amdgpu_drm.h)
// ============================================================================

/// Base number for amdgpu-specific ioctls (0x40 + DRM_COMMAND_BASE)
const DRM_AMDGPU_BASE: u8 = 0x40;

/// DRM ioctl magic number
const DRM_IOCTL_BASE: u8 = b'd';

// amdgpu ioctl command numbers (from amdgpu_drm.h)
/// Create GEM buffer object
pub const DRM_AMDGPU_GEM_CREATE: u8 = 0x00;
/// Map GEM buffer for CPU access
pub const DRM_AMDGPU_GEM_MMAP: u8 = 0x01;
/// Create GPU context
pub const DRM_AMDGPU_CTX: u8 = 0x02;
/// Create BO list for submission
pub const DRM_AMDGPU_BO_LIST: u8 = 0x03;
/// Command submission
pub const DRM_AMDGPU_CS: u8 = 0x04;
/// Query device/HW info
pub const DRM_AMDGPU_INFO: u8 = 0x05;
/// Wait on GEM buffer
pub const DRM_AMDGPU_GEM_WAIT_IDLE: u8 = 0x06;
/// GPU virtual address operations
pub const DRM_AMDGPU_GEM_VA: u8 = 0x07;
/// Query fence status
pub const DRM_AMDGPU_WAIT_CS: u8 = 0x08;
/// Set GEM metadata
pub const DRM_AMDGPU_GEM_METADATA: u8 = 0x09;
/// User mode operations
pub const DRM_AMDGPU_GEM_OP: u8 = 0x0A;
/// GPUVM operations
pub const DRM_AMDGPU_VM: u8 = 0x0B;
/// Fence to handle conversion
pub const DRM_AMDGPU_FENCE_TO_HANDLE: u8 = 0x0C;
/// Scheduling operations
pub const DRM_AMDGPU_SCHED: u8 = 0x0D;

// Encoded ioctl values (using _IOWR macro encoding)
// Format: direction(2) | size(14) | type(8) | nr(8)
// _IOWR = 0xC0000000 | (size << 16) | (type << 8) | nr

/// DRM_IOCTL_AMDGPU_GEM_CREATE: _IOWR('d', 0x40, struct drm_amdgpu_gem_create)
const DRM_IOCTL_AMDGPU_GEM_CREATE: u64 = 0xC0206440;
/// DRM_IOCTL_AMDGPU_GEM_MMAP: _IOWR('d', 0x41, struct drm_amdgpu_gem_mmap)
const DRM_IOCTL_AMDGPU_GEM_MMAP: u64 = 0xC0106441;
/// DRM_IOCTL_AMDGPU_CTX: _IOWR('d', 0x42, union drm_amdgpu_ctx)
const DRM_IOCTL_AMDGPU_CTX: u64 = 0xC0206442;
/// DRM_IOCTL_AMDGPU_BO_LIST: _IOWR('d', 0x43, union drm_amdgpu_bo_list)
const DRM_IOCTL_AMDGPU_BO_LIST: u64 = 0xC0106443;
/// DRM_IOCTL_AMDGPU_CS: _IOWR('d', 0x44, union drm_amdgpu_cs)
const DRM_IOCTL_AMDGPU_CS: u64 = 0xC0506444;
/// DRM_IOCTL_AMDGPU_INFO: _IOW('d', 0x45, struct drm_amdgpu_info)
const DRM_IOCTL_AMDGPU_INFO: u64 = 0x40506445;
/// DRM_IOCTL_AMDGPU_GEM_WAIT_IDLE: _IOWR('d', 0x46, union drm_amdgpu_gem_wait_idle)
const DRM_IOCTL_AMDGPU_GEM_WAIT_IDLE: u64 = 0xC0106446;
/// DRM_IOCTL_AMDGPU_GEM_VA: _IOW('d', 0x47, struct drm_amdgpu_gem_va)
const DRM_IOCTL_AMDGPU_GEM_VA: u64 = 0x40306447;
/// DRM_IOCTL_AMDGPU_WAIT_CS: _IOWR('d', 0x48, union drm_amdgpu_wait_cs)
const DRM_IOCTL_AMDGPU_WAIT_CS: u64 = 0xC0206448;

// ============================================================================
// Memory Domain Bitflags
// ============================================================================

/// amdgpu memory domain flags (where buffer can be placed)
///
/// These flags control where GEM buffer objects are allocated and
/// which memory pools they can migrate between.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct AmdgpuDomain(pub u32);

impl AmdgpuDomain {
    /// CPU-visible system memory (for uploads/downloads)
    pub const CPU: Self = Self(0x1);
    /// GTT (Graphics Translation Table) - system memory mapped for GPU access
    pub const GTT: Self = Self(0x2);
    /// VRAM - dedicated GPU video memory (fastest)
    pub const VRAM: Self = Self(0x4);
    /// GDS (Global Data Share) - on-chip memory for compute
    pub const GDS: Self = Self(0x8);
    /// GWS (Global Wave Sync) - compute synchronization
    pub const GWS: Self = Self(0x10);
    /// OA (Ordered Append) - for append buffers
    pub const OA: Self = Self(0x20);
    /// Doorbell - MMIO doorbell pages
    pub const DOORBELL: Self = Self(0x40);

    /// Create new domain flags
    #[inline]
    pub const fn new(bits: u32) -> Self {
        Self(bits)
    }

    /// Get raw bits
    #[inline]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Check if domain contains CPU
    #[inline]
    pub const fn contains_cpu(self) -> bool {
        (self.0 & Self::CPU.0) != 0
    }

    /// Check if domain contains GTT
    #[inline]
    pub const fn contains_gtt(self) -> bool {
        (self.0 & Self::GTT.0) != 0
    }

    /// Check if domain contains VRAM
    #[inline]
    pub const fn contains_vram(self) -> bool {
        (self.0 & Self::VRAM.0) != 0
    }

    /// Check if any domain is set
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Combine with another domain
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Check if GPU-accessible (VRAM or GTT)
    #[inline]
    pub const fn is_gpu_accessible(self) -> bool {
        self.contains_vram() || self.contains_gtt()
    }

    /// Get human-readable name
    pub fn name(self) -> &'static str {
        match self.0 {
            0x1 => "CPU",
            0x2 => "GTT",
            0x4 => "VRAM",
            0x8 => "GDS",
            0x10 => "GWS",
            0x20 => "OA",
            0x40 => "DOORBELL",
            _ => "MIXED",
        }
    }
}

impl Default for AmdgpuDomain {
    fn default() -> Self {
        Self::VRAM
    }
}

impl core::ops::BitOr for AmdgpuDomain {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitAnd for AmdgpuDomain {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl fmt::Display for AmdgpuDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.contains_cpu() { parts.push("CPU"); }
        if self.contains_gtt() { parts.push("GTT"); }
        if self.contains_vram() { parts.push("VRAM"); }
        if (self.0 & Self::GDS.0) != 0 { parts.push("GDS"); }
        if (self.0 & Self::GWS.0) != 0 { parts.push("GWS"); }
        if (self.0 & Self::OA.0) != 0 { parts.push("OA"); }
        if (self.0 & Self::DOORBELL.0) != 0 { parts.push("DOORBELL"); }
        if parts.is_empty() {
            write!(f, "NONE")
        } else {
            write!(f, "{}", parts.join("|"))
        }
    }
}

// ============================================================================
// Buffer Object Flags
// ============================================================================

/// amdgpu buffer object creation flags
///
/// These flags control buffer object behavior including CPU access,
/// encryption, and placement preferences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct AmdgpuBoFlags(pub u64);

impl AmdgpuBoFlags {
    /// CPU can access this buffer (requires mmap)
    pub const CPU_ACCESS_REQUIRED: Self = Self(1 << 0);
    /// Don't evict this buffer (pin in memory)
    pub const NO_CPU_ACCESS: Self = Self(1 << 1);
    /// Use 64KB pages for VRAM
    pub const CPU_GTT_USWC: Self = Self(1 << 2);
    /// Clear VRAM on allocation (security)
    pub const VRAM_CLEARED: Self = Self(1 << 3);
    /// Contiguous VRAM allocation
    pub const VRAM_CONTIGUOUS: Self = Self(1 << 4);
    /// Encrypted buffer (TMZ - Trusted Memory Zone)
    pub const ENCRYPTED: Self = Self(1 << 5);
    /// KFD (Kernel Fusion Driver) allocation
    pub const KFD: Self = Self(1 << 6);
    /// Shadow buffer for VM page tables
    pub const SHADOW: Self = Self(1 << 7);
    /// Explicitly synchronize access
    pub const EXPLICIT_SYNC: Self = Self(1 << 8);
    /// Memory at specific offset (for sparse)
    pub const MEMORY_OFFSET: Self = Self(1 << 9);
    /// Discardable buffer (can be evicted entirely)
    pub const DISCARDABLE: Self = Self(1 << 10);

    /// Create new flags
    #[inline]
    pub const fn new(bits: u64) -> Self {
        Self(bits)
    }

    /// Get raw bits
    #[inline]
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Empty flags
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Check if flag is set
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    /// Combine with another flag set
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl Default for AmdgpuBoFlags {
    fn default() -> Self {
        Self::empty()
    }
}

impl core::ops::BitOr for AmdgpuBoFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitAnd for AmdgpuBoFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

// ============================================================================
// Hardware IP Types
// ============================================================================

/// AMD GPU hardware IP (Intellectual Property) block types
///
/// Different IP blocks handle different workloads:
/// - GFX: Graphics engine (3D rendering, shaders)
/// - SDMA: System DMA for fast memory transfers
/// - VCN: Video Core Next (hardware video codec)
/// - VPE: Video Processing Engine (RDNA3+)
/// - JPEG: Hardware JPEG codec
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum AmdgpuHwIp {
    /// Graphics engine (3D rendering)
    Gfx = 0,
    /// Compute engine (GPGPU)
    Compute = 1,
    /// SDMA engine 0
    Dma = 2,
    /// UVD (Unified Video Decoder) - legacy
    Uvd = 3,
    /// VCE (Video Compression Engine) - legacy
    Vce = 4,
    /// UVD encode - legacy
    UvdEnc = 5,
    /// VCN decode (Video Core Next)
    VcnDec = 6,
    /// VCN encode
    VcnEnc = 7,
    /// VCN JPEG decode
    VcnJpeg = 8,
    /// VPE (Video Processing Engine) - RDNA3+
    Vpe = 9,
}

impl AmdgpuHwIp {
    /// Convert from u32
    #[inline]
    pub const fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::Gfx),
            1 => Some(Self::Compute),
            2 => Some(Self::Dma),
            3 => Some(Self::Uvd),
            4 => Some(Self::Vce),
            5 => Some(Self::UvdEnc),
            6 => Some(Self::VcnDec),
            7 => Some(Self::VcnEnc),
            8 => Some(Self::VcnJpeg),
            9 => Some(Self::Vpe),
            _ => None,
        }
    }

    /// Convert to u32
    #[inline]
    pub const fn to_u32(self) -> u32 {
        self as u32
    }

    /// Get human-readable name
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Gfx => "GFX",
            Self::Compute => "Compute",
            Self::Dma => "SDMA",
            Self::Uvd => "UVD",
            Self::Vce => "VCE",
            Self::UvdEnc => "UVD Encode",
            Self::VcnDec => "VCN Decode",
            Self::VcnEnc => "VCN Encode",
            Self::VcnJpeg => "VCN JPEG",
            Self::Vpe => "VPE",
        }
    }

    /// Map to AmdQueueType for ring buffer operations
    #[inline]
    pub const fn to_queue_type(self) -> AmdQueueType {
        match self {
            Self::Gfx => AmdQueueType::Gfx,
            Self::Compute => AmdQueueType::Compute,
            Self::Dma => AmdQueueType::Dma,
            Self::Uvd | Self::UvdEnc => AmdQueueType::UvdDec,
            Self::Vce => AmdQueueType::UvdEnc,
            Self::VcnDec | Self::VcnEnc | Self::VcnJpeg => AmdQueueType::Vcn,
            Self::Vpe => AmdQueueType::Vcn, // VPE uses VCN-like queue
        }
    }

    /// Check if this is a video IP block
    #[inline]
    pub const fn is_video(self) -> bool {
        matches!(
            self,
            Self::Uvd | Self::Vce | Self::UvdEnc |
            Self::VcnDec | Self::VcnEnc | Self::VcnJpeg | Self::Vpe
        )
    }
}

impl fmt::Display for AmdgpuHwIp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// Context Operations
// ============================================================================

/// amdgpu context operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AmdgpuCtxOp {
    /// Allocate a new context
    AllocCtx = 1,
    /// Free an existing context
    FreeCtx = 2,
    /// Query context state
    QueryState = 3,
    /// Query reset state for context
    QueryState2 = 4,
    /// Set stable power state
    SetStablePstate = 5,
}

impl AmdgpuCtxOp {
    /// Convert from u32
    #[inline]
    pub const fn from_u32(v: u32) -> Option<Self> {
        match v {
            1 => Some(Self::AllocCtx),
            2 => Some(Self::FreeCtx),
            3 => Some(Self::QueryState),
            4 => Some(Self::QueryState2),
            5 => Some(Self::SetStablePstate),
            _ => None,
        }
    }
}

// ============================================================================
// Info Query Types
// ============================================================================

/// amdgpu info query types (for DRM_AMDGPU_INFO ioctl)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AmdgpuInfoId {
    /// Device ID and revision
    AcqDeviceId = 0x00,
    /// Number of hardware IPs
    NumHwIps = 0x01,
    /// Hardware IP info
    HwIpInfo = 0x02,
    /// Firmware version info
    FwVersion = 0x03,
    /// Number of compute units
    HwIpCount = 0x04,
    /// Timestamp frequency
    TimestampFreq = 0x05,
    /// GPU clock frequency
    Clocks = 0x06,
    /// VRAM usage
    VramUsage = 0x07,
    /// GTT usage
    GttUsage = 0x08,
    /// Heap info (VRAM/GTT/visible)
    HeapInfo = 0x09,
    /// Memory info
    MemoryInfo = 0x0A,
    /// Read MMIO register
    ReadMmr = 0x0B,
    /// Device info (family, CU count, etc.)
    DevInfo = 0x0C,
    /// Visible VRAM usage
    VisVramUsage = 0x0D,
    /// Number of enabled shader engines
    NumEvictions = 0x0E,
    /// Video memory info
    VideoMemInfo = 0x0F,
    /// Sensor info (temp, fan, power)
    SensorInfo = 0x10,
    /// VBIOS info
    VbiosInfo = 0x11,
    /// Video capabilities
    VideoCaps = 0x12,
    /// GDS (Global Data Share) info
    GdsInfo = 0x13,
    /// Max IBS (Instruction Buffer Size)
    MaxIbs = 0x14,
}

impl AmdgpuInfoId {
    /// Convert from u32
    #[inline]
    pub const fn from_u32(v: u32) -> Option<Self> {
        match v {
            0x00 => Some(Self::AcqDeviceId),
            0x01 => Some(Self::NumHwIps),
            0x02 => Some(Self::HwIpInfo),
            0x03 => Some(Self::FwVersion),
            0x04 => Some(Self::HwIpCount),
            0x05 => Some(Self::TimestampFreq),
            0x06 => Some(Self::Clocks),
            0x07 => Some(Self::VramUsage),
            0x08 => Some(Self::GttUsage),
            0x09 => Some(Self::HeapInfo),
            0x0A => Some(Self::MemoryInfo),
            0x0B => Some(Self::ReadMmr),
            0x0C => Some(Self::DevInfo),
            0x0D => Some(Self::VisVramUsage),
            0x0E => Some(Self::NumEvictions),
            0x0F => Some(Self::VideoMemInfo),
            0x10 => Some(Self::SensorInfo),
            0x11 => Some(Self::VbiosInfo),
            0x12 => Some(Self::VideoCaps),
            0x13 => Some(Self::GdsInfo),
            0x14 => Some(Self::MaxIbs),
            _ => None,
        }
    }

    /// Get human-readable name
    pub const fn name(self) -> &'static str {
        match self {
            Self::AcqDeviceId => "ACCEL_DEVICE_ID",
            Self::NumHwIps => "NUM_HW_IPS",
            Self::HwIpInfo => "HW_IP_INFO",
            Self::FwVersion => "FW_VERSION",
            Self::HwIpCount => "HW_IP_COUNT",
            Self::TimestampFreq => "TIMESTAMP_FREQ",
            Self::Clocks => "CLOCKS",
            Self::VramUsage => "VRAM_USAGE",
            Self::GttUsage => "GTT_USAGE",
            Self::HeapInfo => "HEAP_INFO",
            Self::MemoryInfo => "MEMORY_INFO",
            Self::ReadMmr => "READ_MMR",
            Self::DevInfo => "DEV_INFO",
            Self::VisVramUsage => "VIS_VRAM_USAGE",
            Self::NumEvictions => "NUM_EVICTIONS",
            Self::VideoMemInfo => "VIDEO_MEM_INFO",
            Self::SensorInfo => "SENSOR_INFO",
            Self::VbiosInfo => "VBIOS_INFO",
            Self::VideoCaps => "VIDEO_CAPS",
            Self::GdsInfo => "GDS_INFO",
            Self::MaxIbs => "MAX_IBS",
        }
    }
}

// ============================================================================
// FFI Structures (for ioctl)
// ============================================================================

/// drm_amdgpu_gem_create_in structure (input for GEM_CREATE)
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct AmdgpuGemCreateIn {
    /// Size in bytes
    pub bo_size: u64,
    /// Alignment requirement (power of 2)
    pub alignment: u64,
    /// Memory domains (AmdgpuDomain flags)
    pub domains: u64,
    /// Creation flags (AmdgpuBoFlags)
    pub domain_flags: u64,
}

/// drm_amdgpu_gem_create_out structure (output from GEM_CREATE)
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct AmdgpuGemCreateOut {
    /// GEM handle
    pub handle: u32,
    /// Padding
    pub _pad: u32,
}

/// Full GEM create request (union of in/out)
#[derive(Clone, Copy)]
#[repr(C)]
pub union AmdgpuGemCreateFfi {
    /// Input parameters
    pub r#in: AmdgpuGemCreateIn,
    /// Output parameters
    pub out: AmdgpuGemCreateOut,
}

impl Default for AmdgpuGemCreateFfi {
    fn default() -> Self {
        Self { r#in: AmdgpuGemCreateIn::default() }
    }
}

/// drm_amdgpu_gem_mmap_in structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct AmdgpuGemMmapIn {
    /// GEM handle to map
    pub handle: u32,
    /// Padding
    pub _pad: u32,
}

/// drm_amdgpu_gem_mmap_out structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct AmdgpuGemMmapOut {
    /// Offset for mmap (fake offset into DRM file)
    pub addr_ptr: u64,
}

/// Full GEM mmap request
#[derive(Clone, Copy)]
#[repr(C)]
pub union AmdgpuGemMmapFfi {
    /// Input parameters
    pub r#in: AmdgpuGemMmapIn,
    /// Output parameters
    pub out: AmdgpuGemMmapOut,
}

impl Default for AmdgpuGemMmapFfi {
    fn default() -> Self {
        Self { r#in: AmdgpuGemMmapIn::default() }
    }
}

/// drm_amdgpu_ctx_in structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct AmdgpuCtxIn {
    /// Context operation (AmdgpuCtxOp)
    pub op: u32,
    /// Flags
    pub flags: u32,
    /// Context handle (for free/query operations)
    pub ctx_id: u32,
    /// Priority
    pub priority: u32,
}

/// drm_amdgpu_ctx_out structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct AmdgpuCtxOut {
    /// State (for query operations)
    pub state: u64,
    /// Allocated context handle
    pub ctx_id: u32,
    /// Padding
    pub _pad: u32,
}

/// Full context request
#[derive(Clone, Copy)]
#[repr(C)]
pub union AmdgpuCtxFfi {
    /// Input parameters
    pub r#in: AmdgpuCtxIn,
    /// Output parameters
    pub out: AmdgpuCtxOut,
}

impl Default for AmdgpuCtxFfi {
    fn default() -> Self {
        Self { r#in: AmdgpuCtxIn::default() }
    }
}

/// Indirect Buffer descriptor for command submission
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct AmdgpuCsIb {
    /// GPU virtual address of IB
    pub va_start: u64,
    /// Number of DWORDs in IB
    pub ib_bytes: u32,
    /// Flags
    pub flags: u32,
}

/// Command submission chunk header
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct AmdgpuCsChunk {
    /// Chunk type ID
    pub chunk_id: u32,
    /// Size of chunk data
    pub length_dw: u32,
    /// Pointer to chunk data
    pub chunk_data: u64,
}

/// Command submission request input
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct AmdgpuCsIn {
    /// Context handle
    pub ctx_id: u32,
    /// BO list handle
    pub bo_list_handle: u32,
    /// Number of chunks
    pub num_chunks: u32,
    /// Flags
    pub flags: u32,
    /// Pointer to chunks array
    pub chunks: u64,
}

/// Command submission request output
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct AmdgpuCsOut {
    /// Fence sequence number
    pub handle: u64,
}

/// Full CS request
#[derive(Clone, Copy)]
#[repr(C)]
pub union AmdgpuCsFfi {
    /// Input parameters
    pub r#in: AmdgpuCsIn,
    /// Output parameters
    pub out: AmdgpuCsOut,
}

impl Default for AmdgpuCsFfi {
    fn default() -> Self {
        Self { r#in: AmdgpuCsIn::default() }
    }
}

/// Info query request
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct AmdgpuInfoFfi {
    /// Query type (AmdgpuInfoId)
    pub query: u32,
    /// Padding
    pub _pad: u32,
    /// Return buffer pointer
    pub return_pointer: u64,
    /// Return buffer size
    pub return_size: u32,
    /// Query-specific data
    pub query_param: u32,
}

/// Device info returned by AMDGPU_INFO_DEV_INFO
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct AmdgpuDevInfo {
    /// Device ID
    pub device_id: u32,
    /// Chip revision
    pub chip_rev: u32,
    /// External revision
    pub external_rev: u32,
    /// PCI revision
    pub pci_rev: u32,
    /// GPU family
    pub family: u32,
    /// Number of compute units
    pub num_cu: u32,
    /// Number of shader engines
    pub num_se: u32,
    /// Number of shader arrays per SE
    pub num_sa_per_se: u32,
    /// Number of CUs per shader array
    pub num_cu_per_sa: u32,
    /// Number of RBs per SE
    pub num_rb_per_se: u32,
    /// Max engine clock (MHz)
    pub max_engine_clk: u32,
    /// Max memory clock (MHz)
    pub max_memory_clk: u32,
    /// Total VRAM size (bytes)
    pub vram_size: u64,
    /// Visible VRAM size (bytes)
    pub visible_vram_size: u64,
    /// GTT size (bytes)
    pub gtt_size: u64,
    /// VRAM bit width
    pub vram_bit_width: u32,
    /// VRAM type (GDDR5, HBM, etc.)
    pub vram_type: u32,
    /// GPU counter frequency (Hz)
    pub gpu_counter_freq: u64,
    /// Virtual address bits
    pub virtual_address_bits: u32,
    /// Virtual address alignment
    pub virtual_address_alignment: u32,
    /// PCI domain
    pub pci_domain: u32,
    /// PCI bus
    pub pci_bus: u32,
    /// PCI device
    pub pci_device: u32,
    /// PCI function
    pub pci_function: u32,
    /// CU active bitmap
    pub cu_active_bitmap: [u32; 4],
    /// CU AO bitmap
    pub cu_ao_bitmap: [u32; 4],
    /// High priority GFX timeout (ms)
    pub high_va_offset: u64,
    /// High priority compute timeout (ms)
    pub high_va_max: u64,
    /// GDS size
    pub gds_size: u32,
    /// GWS per GFX
    pub gws_per_gfx: u32,
    /// GWS per compute
    pub gws_per_compute: u32,
    /// OA per compute
    pub oa_per_compute: u32,
    /// TCP per SH
    pub tcp_cache_size: u32,
    /// GL1 cache size
    pub gl1_cache_size: u32,
    /// GL2 cache size
    pub gl2_cache_size: u32,
    /// Padding
    pub _pad: u32,
}

// ============================================================================
// Context State
// ============================================================================

/// Context lifecycle state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AmdgpuContextState {
    /// Context not allocated
    Unallocated = 0,
    /// Context ready for use
    Ready = 1,
    /// Context has pending work
    Active = 2,
    /// Context in error state (GPU reset needed)
    Error = 3,
    /// Context suspended (power management)
    Suspended = 4,
}

impl AmdgpuContextState {
    /// Convert from u8
    #[inline]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Unallocated,
            1 => Self::Ready,
            2 => Self::Active,
            3 => Self::Error,
            4 => Self::Suspended,
            _ => Self::Unallocated,
        }
    }

    /// Convert to u8
    #[inline]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Check if context is usable
    #[inline]
    pub const fn is_usable(self) -> bool {
        matches!(self, Self::Ready | Self::Active)
    }

    /// Get human-readable name
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unallocated => "Unallocated",
            Self::Ready => "Ready",
            Self::Active => "Active",
            Self::Error => "Error",
            Self::Suspended => "Suspended",
        }
    }
}

impl fmt::Display for AmdgpuContextState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// AmdgpuContextCapsule (T1 Atomic, 256B)
// ============================================================================

/// AMD GPU Context Capsule (T1 Atomic, 256B)
///
/// Manages amdgpu context state with lockfree atomic operations.
/// Provides thread-safe state tracking for GPU contexts with
/// generation counters for TOCTOU prevention.
///
/// # Layout (256 bytes, 256-byte aligned)
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────────┐
/// │  state_gen (AtomicU64)      │  ctx_id (AtomicU32)              │ 12B
/// │  priority (AtomicU32)                                          │  4B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  submit_count (AtomicU64)   │  error_count (AtomicU64)         │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  pending_fence (AtomicU64)  │  completed_fence (AtomicU64)     │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  vram_usage (AtomicU64)     │  gtt_usage (AtomicU64)           │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  bo_count (AtomicU32)       │  hw_ip (u8) │ ring_id (u8)       │  6B
/// │  vmid (u8)                  │  _reserved (u8)                   │  2B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  drm_fd (AtomicU32)         │  flags (AtomicU32)               │  8B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  _padding [176 bytes]                                          │176B
/// └─────────────────────────────────────────────────────────────────┘
/// ```
///
/// # Chaos Compliance
///
/// - **T1 Atomic Tier**: All state via AtomicU64/AtomicU32
/// - **256B Aligned**: 4 cache lines for optimal bandwidth
/// - **Generation Counters**: TOCTOU prevention
/// - **100% Lockfree**: NO mutex/RwLock
///
/// # ASSUM Tags
///
/// - `#ASSUME_ATOMIC_ALIGNED`: All AtomicU64/U32 fields properly aligned
/// - `#ASSUME_CTX_VALID`: Context ID from kernel is valid when state != Unallocated
/// - `#ASSUME_GENERATION_MONOTONIC`: Generation wraps at 65535
#[repr(C, align(256))]
pub struct AmdgpuContextCapsule {
    /// State (bits 0-7) + Generation (bits 8-23) + Reserved (bits 24-63)
    ///
    /// # Bit Layout
    /// - Bits  0-7:  AmdgpuContextState enum value (0-4)
    /// - Bits  8-23: Generation counter (0-65535, wrapping)
    /// - Bits 24-63: Reserved for future flags
    state_gen: AtomicU64,

    /// Kernel-assigned context ID
    ctx_id: AtomicU32,

    /// Context priority (0 = normal, higher = more priority)
    priority: AtomicU32,

    /// Total submissions made with this context
    submit_count: AtomicU64,

    /// Total errors encountered
    error_count: AtomicU64,

    /// Highest submitted fence value
    pending_fence: AtomicU64,

    /// Highest completed fence value
    completed_fence: AtomicU64,

    /// Estimated VRAM usage (bytes)
    vram_usage: AtomicU64,

    /// Estimated GTT usage (bytes)
    gtt_usage: AtomicU64,

    /// Number of active buffer objects
    bo_count: AtomicU32,

    /// Hardware IP type (AmdgpuHwIp)
    hw_ip: u8,

    /// Ring ID within IP
    ring_id: u8,

    /// Virtual machine ID
    vmid: u8,

    /// Reserved
    _reserved: u8,

    /// DRM file descriptor for this context
    drm_fd: AtomicU32,

    /// Context flags
    flags: AtomicU32,

    /// Padding to 256 bytes
    /// Fields: 8 + 4 + 4 + 8 + 8 + 8 + 8 + 8 + 8 + 4 + 4 + 4 + 4 = 80 bytes
    /// Padding needed: 256 - 80 = 176 bytes
    _padding: [u8; 176],
}

impl AmdgpuContextCapsule {
    // Constants for bit manipulation
    const STATE_MASK: u64 = 0xFF;
    const GEN_MASK: u64 = 0xFFFF00;
    const GEN_SHIFT: u32 = 8;

    /// Create a new unallocated context capsule
    ///
    /// # Returns
    ///
    /// New `AmdgpuContextCapsule` in `Unallocated` state
    ///
    /// # Performance
    ///
    /// O(1), ~10ns (zeroing 256 bytes)
    #[inline]
    pub const fn new() -> Self {
        Self {
            state_gen: AtomicU64::new(0),
            ctx_id: AtomicU32::new(0),
            priority: AtomicU32::new(0),
            submit_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            pending_fence: AtomicU64::new(0),
            completed_fence: AtomicU64::new(0),
            vram_usage: AtomicU64::new(0),
            gtt_usage: AtomicU64::new(0),
            bo_count: AtomicU32::new(0),
            hw_ip: 0,
            ring_id: 0,
            vmid: 0,
            _reserved: 0,
            drm_fd: AtomicU32::new(0),
            flags: AtomicU32::new(0),
            _padding: [0u8; 176],
        }
    }

    /// Initialize context after allocation from kernel
    ///
    /// Transitions from `Unallocated` -> `Ready`.
    ///
    /// # Arguments
    ///
    /// * `ctx_id` - Context ID from kernel allocation
    /// * `drm_fd` - DRM file descriptor
    /// * `hw_ip` - Hardware IP type
    /// * `ring_id` - Ring ID within IP
    /// * `priority` - Context priority
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(InvalidState)` if already allocated
    ///
    /// # Performance
    ///
    /// <100ns (CAS + stores)
    pub fn initialize(
        &self,
        ctx_id: u32,
        drm_fd: i32,
        _hw_ip: AmdgpuHwIp,  // TODO: Pack into AtomicU32 with ring_id/vmid
        _ring_id: u8,        // TODO: Pack into AtomicU32 with hw_ip/vmid
        priority: u32,
    ) -> KgpuDriverResult<u16> {
        loop {
            let old = self.state_gen.load(Ordering::Acquire);
            let old_state = AmdgpuContextState::from_u8((old & Self::STATE_MASK) as u8);

            if old_state != AmdgpuContextState::Unallocated {
                return Err(KgpuDriverError::InvalidState);
            }

            let old_gen = ((old & Self::GEN_MASK) >> Self::GEN_SHIFT) as u16;
            let new_gen = old_gen.wrapping_add(1);
            let new = (AmdgpuContextState::Ready as u64) | ((new_gen as u64) << Self::GEN_SHIFT);

            match self.state_gen.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Store other fields
                    self.ctx_id.store(ctx_id, Ordering::Release);
                    self.drm_fd.store(drm_fd as u32, Ordering::Release);
                    self.priority.store(priority, Ordering::Release);
                    // Note: hw_ip, ring_id, vmid are non-atomic u8 fields set at construction
                    // time only. For runtime modification, they should be packed into an
                    // AtomicU32. Currently using default values from new().
                    return Ok(new_gen);
                }
                Err(_) => continue,
            }
        }
    }

    /// Get current state
    #[inline]
    pub fn state(&self) -> AmdgpuContextState {
        let v = self.state_gen.load(Ordering::Acquire);
        AmdgpuContextState::from_u8((v & Self::STATE_MASK) as u8)
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u16 {
        let v = self.state_gen.load(Ordering::Acquire);
        ((v & Self::GEN_MASK) >> Self::GEN_SHIFT) as u16
    }

    /// Get context ID
    #[inline]
    pub fn ctx_id(&self) -> u32 {
        self.ctx_id.load(Ordering::Acquire)
    }

    /// Get DRM file descriptor
    #[inline]
    pub fn drm_fd(&self) -> i32 {
        self.drm_fd.load(Ordering::Acquire) as i32
    }

    /// Get priority
    #[inline]
    pub fn priority(&self) -> u32 {
        self.priority.load(Ordering::Acquire)
    }

    /// Get hardware IP type
    #[inline]
    pub fn hw_ip(&self) -> AmdgpuHwIp {
        AmdgpuHwIp::from_u32(self.hw_ip as u32).unwrap_or(AmdgpuHwIp::Gfx)
    }

    /// Get ring ID
    #[inline]
    pub fn ring_id(&self) -> u8 {
        self.ring_id
    }

    /// Get VMID
    #[inline]
    pub fn vmid(&self) -> u8 {
        self.vmid
    }

    /// Get submit count
    #[inline]
    pub fn submit_count(&self) -> u64 {
        self.submit_count.load(Ordering::Acquire)
    }

    /// Get error count
    #[inline]
    pub fn error_count(&self) -> u64 {
        self.error_count.load(Ordering::Acquire)
    }

    /// Get pending fence value
    #[inline]
    pub fn pending_fence(&self) -> u64 {
        self.pending_fence.load(Ordering::Acquire)
    }

    /// Get completed fence value
    #[inline]
    pub fn completed_fence(&self) -> u64 {
        self.completed_fence.load(Ordering::Acquire)
    }

    /// Get VRAM usage
    #[inline]
    pub fn vram_usage(&self) -> u64 {
        self.vram_usage.load(Ordering::Acquire)
    }

    /// Get GTT usage
    #[inline]
    pub fn gtt_usage(&self) -> u64 {
        self.gtt_usage.load(Ordering::Acquire)
    }

    /// Get BO count
    #[inline]
    pub fn bo_count(&self) -> u32 {
        self.bo_count.load(Ordering::Acquire)
    }

    /// Check if any work is pending (pending_fence > completed_fence)
    #[inline]
    pub fn has_pending_work(&self) -> bool {
        self.pending_fence.load(Ordering::Acquire) > self.completed_fence.load(Ordering::Acquire)
    }

    /// Increment submit count and update pending fence
    ///
    /// # Arguments
    ///
    /// * `fence` - Fence value for this submission
    ///
    /// # Returns
    ///
    /// Previous submit count
    #[inline]
    pub fn record_submit(&self, fence: u64) -> u64 {
        self.pending_fence.fetch_max(fence, Ordering::AcqRel);
        self.submit_count.fetch_add(1, Ordering::AcqRel)
    }

    /// Update completed fence value
    ///
    /// # Arguments
    ///
    /// * `fence` - Completed fence value
    #[inline]
    pub fn record_completion(&self, fence: u64) {
        self.completed_fence.fetch_max(fence, Ordering::AcqRel);
    }

    /// Increment error count
    #[inline]
    pub fn record_error(&self) -> u64 {
        self.error_count.fetch_add(1, Ordering::AcqRel)
    }

    /// Update memory usage
    ///
    /// # Arguments
    ///
    /// * `vram_delta` - Change in VRAM usage (can be negative via saturating)
    /// * `gtt_delta` - Change in GTT usage
    #[inline]
    pub fn update_memory_usage(&self, vram_delta: i64, gtt_delta: i64) {
        if vram_delta >= 0 {
            self.vram_usage.fetch_add(vram_delta as u64, Ordering::Relaxed);
        } else {
            let abs = (-vram_delta) as u64;
            loop {
                let current = self.vram_usage.load(Ordering::Acquire);
                let new = current.saturating_sub(abs);
                if self.vram_usage.compare_exchange_weak(
                    current, new, Ordering::AcqRel, Ordering::Acquire
                ).is_ok() {
                    break;
                }
            }
        }

        if gtt_delta >= 0 {
            self.gtt_usage.fetch_add(gtt_delta as u64, Ordering::Relaxed);
        } else {
            let abs = (-gtt_delta) as u64;
            loop {
                let current = self.gtt_usage.load(Ordering::Acquire);
                let new = current.saturating_sub(abs);
                if self.gtt_usage.compare_exchange_weak(
                    current, new, Ordering::AcqRel, Ordering::Acquire
                ).is_ok() {
                    break;
                }
            }
        }
    }

    /// Increment BO count
    #[inline]
    pub fn add_bo(&self) -> u32 {
        self.bo_count.fetch_add(1, Ordering::AcqRel)
    }

    /// Decrement BO count
    #[inline]
    pub fn remove_bo(&self) -> u32 {
        loop {
            let current = self.bo_count.load(Ordering::Acquire);
            if current == 0 {
                return 0;
            }
            let new = current - 1;
            if self.bo_count.compare_exchange_weak(
                current, new, Ordering::AcqRel, Ordering::Acquire
            ).is_ok() {
                return new;
            }
        }
    }

    /// Transition to Active state
    ///
    /// Called when first submission is made.
    pub fn mark_active(&self) -> KgpuDriverResult<u16> {
        loop {
            let old = self.state_gen.load(Ordering::Acquire);
            let old_state = AmdgpuContextState::from_u8((old & Self::STATE_MASK) as u8);

            if old_state != AmdgpuContextState::Ready {
                return Err(KgpuDriverError::InvalidState);
            }

            let old_gen = ((old & Self::GEN_MASK) >> Self::GEN_SHIFT) as u16;
            let new_gen = old_gen.wrapping_add(1);
            let new = (AmdgpuContextState::Active as u64) | ((new_gen as u64) << Self::GEN_SHIFT);

            match self.state_gen.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(new_gen),
                Err(_) => continue,
            }
        }
    }

    /// Transition back to Ready state
    ///
    /// Called when all pending work completes.
    pub fn mark_idle(&self) -> KgpuDriverResult<u16> {
        loop {
            let old = self.state_gen.load(Ordering::Acquire);
            let old_state = AmdgpuContextState::from_u8((old & Self::STATE_MASK) as u8);

            if old_state != AmdgpuContextState::Active {
                return Err(KgpuDriverError::InvalidState);
            }

            let old_gen = ((old & Self::GEN_MASK) >> Self::GEN_SHIFT) as u16;
            let new_gen = old_gen.wrapping_add(1);
            let new = (AmdgpuContextState::Ready as u64) | ((new_gen as u64) << Self::GEN_SHIFT);

            match self.state_gen.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(new_gen),
                Err(_) => continue,
            }
        }
    }

    /// Mark context as in error state (GPU reset needed)
    pub fn mark_error(&self) -> KgpuDriverResult<u16> {
        loop {
            let old = self.state_gen.load(Ordering::Acquire);
            let old_gen = ((old & Self::GEN_MASK) >> Self::GEN_SHIFT) as u16;
            let new_gen = old_gen.wrapping_add(1);
            let new = (AmdgpuContextState::Error as u64) | ((new_gen as u64) << Self::GEN_SHIFT);

            match self.state_gen.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.record_error();
                    return Ok(new_gen);
                }
                Err(_) => continue,
            }
        }
    }

    /// Mark context as freed
    pub fn mark_freed(&self) -> KgpuDriverResult<u16> {
        loop {
            let old = self.state_gen.load(Ordering::Acquire);
            let old_gen = ((old & Self::GEN_MASK) >> Self::GEN_SHIFT) as u16;
            let new_gen = old_gen.wrapping_add(1);
            let new = (AmdgpuContextState::Unallocated as u64) | ((new_gen as u64) << Self::GEN_SHIFT);

            match self.state_gen.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.ctx_id.store(0, Ordering::Release);
                    return Ok(new_gen);
                }
                Err(_) => continue,
            }
        }
    }

    /// Take an atomic snapshot of current state
    #[inline]
    pub fn snapshot(&self) -> AmdgpuContextSnapshot {
        let state_gen = self.state_gen.load(Ordering::Acquire);

        AmdgpuContextSnapshot {
            state: AmdgpuContextState::from_u8((state_gen & Self::STATE_MASK) as u8),
            generation: ((state_gen & Self::GEN_MASK) >> Self::GEN_SHIFT) as u16,
            ctx_id: self.ctx_id.load(Ordering::Acquire),
            priority: self.priority.load(Ordering::Acquire),
            submit_count: self.submit_count.load(Ordering::Acquire),
            error_count: self.error_count.load(Ordering::Acquire),
            pending_fence: self.pending_fence.load(Ordering::Acquire),
            completed_fence: self.completed_fence.load(Ordering::Acquire),
            vram_usage: self.vram_usage.load(Ordering::Acquire),
            gtt_usage: self.gtt_usage.load(Ordering::Acquire),
            bo_count: self.bo_count.load(Ordering::Acquire),
            hw_ip: AmdgpuHwIp::from_u32(self.hw_ip as u32).unwrap_or(AmdgpuHwIp::Gfx),
            ring_id: self.ring_id,
            vmid: self.vmid,
        }
    }
}

impl Default for AmdgpuContextCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for AmdgpuContextCapsule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let snap = self.snapshot();
        f.debug_struct("AmdgpuContextCapsule")
            .field("state", &snap.state)
            .field("generation", &snap.generation)
            .field("ctx_id", &snap.ctx_id)
            .field("hw_ip", &snap.hw_ip)
            .field("submit_count", &snap.submit_count)
            .field("pending_fence", &snap.pending_fence)
            .field("completed_fence", &snap.completed_fence)
            .finish()
    }
}

// Safety: All fields are atomic or immutable after initialization
unsafe impl Send for AmdgpuContextCapsule {}
unsafe impl Sync for AmdgpuContextCapsule {}

// Compile-time size/alignment assertions
const _: () = {
    assert!(
        core::mem::size_of::<AmdgpuContextCapsule>() == 256,
        "AmdgpuContextCapsule must be 256 bytes"
    );
    assert!(
        core::mem::align_of::<AmdgpuContextCapsule>() == 256,
        "AmdgpuContextCapsule must be 256-byte aligned"
    );
};

// ============================================================================
// Context Snapshot
// ============================================================================

/// Immutable snapshot of amdgpu context state
#[derive(Debug, Clone, Copy)]
pub struct AmdgpuContextSnapshot {
    /// Current state
    pub state: AmdgpuContextState,
    /// Generation counter
    pub generation: u16,
    /// Context ID
    pub ctx_id: u32,
    /// Priority
    pub priority: u32,
    /// Total submissions
    pub submit_count: u64,
    /// Total errors
    pub error_count: u64,
    /// Pending fence value
    pub pending_fence: u64,
    /// Completed fence value
    pub completed_fence: u64,
    /// VRAM usage
    pub vram_usage: u64,
    /// GTT usage
    pub gtt_usage: u64,
    /// BO count
    pub bo_count: u32,
    /// Hardware IP
    pub hw_ip: AmdgpuHwIp,
    /// Ring ID
    pub ring_id: u8,
    /// VMID
    pub vmid: u8,
}

impl AmdgpuContextSnapshot {
    /// Check if context is usable
    #[inline]
    pub fn is_usable(&self) -> bool {
        self.state.is_usable()
    }

    /// Check if work is pending
    #[inline]
    pub fn has_pending_work(&self) -> bool {
        self.pending_fence > self.completed_fence
    }

    /// Get total memory usage (VRAM + GTT)
    #[inline]
    pub fn total_memory_usage(&self) -> u64 {
        self.vram_usage.saturating_add(self.gtt_usage)
    }
}

impl Default for AmdgpuContextSnapshot {
    fn default() -> Self {
        Self {
            state: AmdgpuContextState::Unallocated,
            generation: 0,
            ctx_id: 0,
            priority: 0,
            submit_count: 0,
            error_count: 0,
            pending_fence: 0,
            completed_fence: 0,
            vram_usage: 0,
            gtt_usage: 0,
            bo_count: 0,
            hw_ip: AmdgpuHwIp::Gfx,
            ring_id: 0,
            vmid: 0,
        }
    }
}

// ============================================================================
// AmdgpuDriver Implementation
// ============================================================================

/// AMD GPU Driver for amdgpu DRM interface
///
/// Provides high-level operations for AMD GPU access via the amdgpu
/// kernel driver. Wraps ioctls for GEM, context, and command submission.
///
/// # Thread Safety
///
/// All operations are designed to be thread-safe. Internal state is
/// managed via atomic operations on capsules.
#[cfg(all(feature = "kgpu-driver-amd", target_os = "linux"))]
pub struct AmdgpuDriver {
    /// DRM file descriptor
    drm_fd: i32,
    /// Device info
    dev_info: AmdgpuDevInfo,
    /// GPU generation
    generation: GpuGeneration,
}

#[cfg(all(feature = "kgpu-driver-amd", target_os = "linux"))]
impl AmdgpuDriver {
    // ========================================================================
    // Construction
    // ========================================================================

    /// Create a new driver instance for an open DRM device
    ///
    /// # Arguments
    ///
    /// * `drm_fd` - Open file descriptor for amdgpu DRM device
    ///
    /// # Returns
    ///
    /// - `Ok(AmdgpuDriver)` on success
    /// - `Err` if device info query fails
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_FD_VALID`: drm_fd is valid open amdgpu device
    /// - `#VERIFY_FD_VALID`: Kernel validates in ioctl handlers
    pub fn new(drm_fd: i32) -> KgpuDriverResult<Self> {
        if drm_fd < 0 {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // Query device info
        let dev_info = Self::query_device_info_internal(drm_fd)?;

        // Determine GPU generation from device info
        let generation = Self::detect_generation(&dev_info);

        Ok(Self {
            drm_fd,
            dev_info,
            generation,
        })
    }

    /// Get DRM file descriptor
    #[inline]
    pub fn drm_fd(&self) -> i32 {
        self.drm_fd
    }

    /// Get device info
    #[inline]
    pub fn dev_info(&self) -> &AmdgpuDevInfo {
        &self.dev_info
    }

    /// Get GPU generation
    #[inline]
    pub fn generation(&self) -> GpuGeneration {
        self.generation
    }

    // ========================================================================
    // GEM Operations
    // ========================================================================

    /// Create a GEM buffer object
    ///
    /// # Arguments
    ///
    /// * `size` - Buffer size in bytes
    /// * `alignment` - Alignment requirement (power of 2, 0 for default)
    /// * `domains` - Memory domains for placement
    /// * `flags` - Buffer creation flags
    ///
    /// # Returns
    ///
    /// - `Ok(handle)` - GEM handle for the created buffer
    /// - `Err(OutOfMemory)` - Not enough memory
    /// - `Err(InvalidParameter)` - Invalid parameters
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_IOCTL_SAFE`: Kernel validates all parameters
    /// - `#VERIFY_IOCTL_SAFE`: amdgpu driver handles invalid params
    pub fn gem_create(
        &self,
        size: u64,
        alignment: u64,
        domains: AmdgpuDomain,
        flags: AmdgpuBoFlags,
    ) -> KgpuDriverResult<u32> {
        if size == 0 {
            return Err(KgpuDriverError::InvalidSize);
        }

        let mut req = AmdgpuGemCreateFfi::default();
        // Write to `in` field (writing to union doesn't require unsafe)
        req.r#in.bo_size = size;
        req.r#in.alignment = if alignment > 0 { alignment } else { 4096 };
        req.r#in.domains = domains.bits() as u64;
        req.r#in.domain_flags = flags.bits();

        // #ASSUME_IOCTL_SAFE: amdgpu GEM_CREATE is well-documented
        // #VERIFY_IOCTL_SAFE: Kernel validates size, alignment, domains
        // SAFETY: ioctl with properly initialized structure, union read after ioctl
        let ret = unsafe {
            let r = libc::ioctl(
                self.drm_fd,
                DRM_IOCTL_AMDGPU_GEM_CREATE as libc::c_ulong,
                &mut req as *mut _,
            );
            if r < 0 {
                return Err(Self::errno_to_error());
            }
            // Read from `out` field - requires unsafe (different union variant)
            req.out.handle
        };

        Ok(ret)
    }

    /// Get mmap offset for a GEM buffer
    ///
    /// # Arguments
    ///
    /// * `handle` - GEM handle
    ///
    /// # Returns
    ///
    /// - `Ok(offset)` - Fake offset for use with mmap
    /// - `Err` - Operation failed
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_BO_VALID`: GEM handle is valid
    pub fn gem_mmap_offset(&self, handle: u32) -> KgpuDriverResult<u64> {
        let mut req = AmdgpuGemMmapFfi::default();
        // Write to `in` field (writing to union doesn't require unsafe)
        req.r#in.handle = handle;

        // SAFETY: ioctl with properly initialized structure, union read after ioctl
        let offset = unsafe {
            let ret = libc::ioctl(
                self.drm_fd,
                DRM_IOCTL_AMDGPU_GEM_MMAP as libc::c_ulong,
                &mut req as *mut _,
            );
            if ret < 0 {
                return Err(Self::errno_to_error());
            }
            // Read from `out` field - requires unsafe (different union variant)
            req.out.addr_ptr
        };

        Ok(offset)
    }

    /// Close a GEM buffer handle
    ///
    /// # Arguments
    ///
    /// * `handle` - GEM handle to close
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err` if close fails
    pub fn gem_close(&self, handle: u32) -> KgpuDriverResult<()> {
        // Use the generic DRM GEM_CLOSE ioctl
        #[repr(C)]
        struct DrmGemClose {
            handle: u32,
            pad: u32,
        }

        let req = DrmGemClose { handle, pad: 0 };
        const DRM_IOCTL_GEM_CLOSE: u64 = 0x40086409;

        // SAFETY: ioctl with properly initialized structure
        let ret = unsafe {
            libc::ioctl(
                self.drm_fd,
                DRM_IOCTL_GEM_CLOSE as libc::c_ulong,
                &req as *const _,
            )
        };

        if ret < 0 {
            return Err(Self::errno_to_error());
        }

        Ok(())
    }

    // ========================================================================
    // Context Operations
    // ========================================================================

    /// Allocate a new GPU context
    ///
    /// # Arguments
    ///
    /// * `priority` - Context priority (0 = normal)
    ///
    /// # Returns
    ///
    /// - `Ok(ctx_id)` - Allocated context ID
    /// - `Err` - Allocation failed
    pub fn ctx_alloc(&self, priority: u32) -> KgpuDriverResult<u32> {
        let mut req = AmdgpuCtxFfi::default();
        // Write to `in` field (writing to union doesn't require unsafe)
        req.r#in.op = AmdgpuCtxOp::AllocCtx as u32;
        req.r#in.priority = priority;

        // SAFETY: ioctl with properly initialized structure, union read after ioctl
        let ctx_id = unsafe {
            let ret = libc::ioctl(
                self.drm_fd,
                DRM_IOCTL_AMDGPU_CTX as libc::c_ulong,
                &mut req as *mut _,
            );
            if ret < 0 {
                return Err(Self::errno_to_error());
            }
            // Read from `out` field - requires unsafe (different union variant)
            req.out.ctx_id
        };

        Ok(ctx_id)
    }

    /// Free a GPU context
    ///
    /// # Arguments
    ///
    /// * `ctx_id` - Context ID to free
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err(InvalidParameter)` if context invalid
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_CTX_VALID`: ctx_id is a valid allocated context
    pub fn ctx_free(&self, ctx_id: u32) -> KgpuDriverResult<()> {
        let mut req = AmdgpuCtxFfi::default();
        // Write to `in` field (writing to union doesn't require unsafe)
        req.r#in.op = AmdgpuCtxOp::FreeCtx as u32;
        req.r#in.ctx_id = ctx_id;

        // SAFETY: ioctl with properly initialized structure
        let ret = unsafe {
            libc::ioctl(
                self.drm_fd,
                DRM_IOCTL_AMDGPU_CTX as libc::c_ulong,
                &mut req as *mut _,
            )
        };

        if ret < 0 {
            return Err(Self::errno_to_error());
        }

        Ok(())
    }

    /// Query context state
    ///
    /// # Arguments
    ///
    /// * `ctx_id` - Context ID to query
    ///
    /// # Returns
    ///
    /// - `Ok((state, needs_reset))` - Context state and reset flag
    /// - `Err` - Query failed
    pub fn ctx_query_state(&self, ctx_id: u32) -> KgpuDriverResult<(u64, bool)> {
        let mut req = AmdgpuCtxFfi::default();
        // Write to `in` field (writing to union doesn't require unsafe)
        req.r#in.op = AmdgpuCtxOp::QueryState as u32;
        req.r#in.ctx_id = ctx_id;

        // SAFETY: ioctl with properly initialized structure, union read after ioctl
        let state = unsafe {
            let ret = libc::ioctl(
                self.drm_fd,
                DRM_IOCTL_AMDGPU_CTX as libc::c_ulong,
                &mut req as *mut _,
            );
            if ret < 0 {
                return Err(Self::errno_to_error());
            }
            // Read from `out` field - requires unsafe (different union variant)
            req.out.state
        };

        // State bit 0 = needs reset
        let needs_reset = (state & 1) != 0;

        Ok((state, needs_reset))
    }

    // ========================================================================
    // Command Submission
    // ========================================================================

    /// Submit commands via indirect buffer
    ///
    /// # Arguments
    ///
    /// * `ctx_capsule` - Context capsule for submission tracking
    /// * `ring` - Ring buffer capsule
    /// * `ib_va` - GPU virtual address of indirect buffer
    /// * `ib_size_dwords` - Size of IB in DWORDs
    /// * `hw_ip` - Hardware IP to submit to
    /// * `ring_id` - Ring ID within IP
    ///
    /// # Returns
    ///
    /// - `Ok(fence_seq)` - Fence sequence number for this submission
    /// - `Err` - Submission failed
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_IB_VALID`: ib_va points to valid GPU memory with valid PM4
    /// - `#ASSUME_CTX_VALID`: Context is allocated and not in error state
    pub fn submit_ib(
        &self,
        ctx_capsule: &AmdgpuContextCapsule,
        _ring: &AmdCpRingCapsule,
        ib_va: u64,
        ib_size_dwords: u32,
        hw_ip: AmdgpuHwIp,
        ring_id: u8,
    ) -> KgpuDriverResult<u64> {
        // Validate context state
        if !ctx_capsule.state().is_usable() {
            return Err(KgpuDriverError::InvalidState);
        }

        let ctx_id = ctx_capsule.ctx_id();

        // Build IB descriptor
        let ib = AmdgpuCsIb {
            va_start: ib_va,
            ib_bytes: ib_size_dwords * 4, // Convert to bytes
            flags: 0,
        };

        // Build chunk for IB
        let ib_chunk = AmdgpuCsChunk {
            chunk_id: 1, // AMDGPU_CHUNK_ID_IB
            length_dw: (core::mem::size_of::<AmdgpuCsIb>() / 4) as u32,
            chunk_data: &ib as *const _ as u64,
        };

        // Build CS request
        let mut req = AmdgpuCsFfi::default();
        // Write to `in` field (writing to union doesn't require unsafe)
        req.r#in.ctx_id = ctx_id;
        req.r#in.bo_list_handle = 0; // No BO list for simple submit
        req.r#in.num_chunks = 1;
        req.r#in.flags = (hw_ip.to_u32() & 0xFF) | ((ring_id as u32) << 8);
        req.r#in.chunks = &ib_chunk as *const _ as u64;

        // SAFETY: ioctl with properly initialized structure, union read after ioctl
        // #ASSUME_IB_VALID: IB contains valid PM4 packets
        let fence_seq = unsafe {
            let ret = libc::ioctl(
                self.drm_fd,
                DRM_IOCTL_AMDGPU_CS as libc::c_ulong,
                &mut req as *mut _,
            );
            if ret < 0 {
                ctx_capsule.record_error();
                return Err(Self::errno_to_error());
            }
            // Read from `out` field - requires unsafe (different union variant)
            req.out.handle
        };

        // Update context tracking
        ctx_capsule.record_submit(fence_seq);

        Ok(fence_seq)
    }

    /// Submit ring buffer commands to GPU
    ///
    /// Integrates with AmdCpRingCapsule for PM4 packet submission.
    ///
    /// # Arguments
    ///
    /// * `ctx_capsule` - Context for submission
    /// * `ring` - Ring buffer with commands
    ///
    /// # Returns
    ///
    /// - `Ok(fence)` - Fence value for this submission
    /// - `Err` - Submission failed
    pub fn submit_ring(
        &self,
        ctx_capsule: &AmdgpuContextCapsule,
        ring: &AmdCpRingCapsule,
    ) -> KgpuDriverResult<u64> {
        // Validate states
        if !ctx_capsule.state().is_usable() {
            return Err(KgpuDriverError::InvalidState);
        }
        if !ring.state().is_operational() {
            return Err(KgpuDriverError::InvalidState);
        }

        // Get ring buffer info
        let _ring_base = ring.ring_base();  // Reserved for future IB submission
        let wptr = ring.wptr();

        // For now, we simulate submission by calling ring.submit()
        // TODO: In a real driver, we'd use the ring buffer as an IB via _ring_base
        let fence = ring.submit(wptr)?;

        // Update context tracking
        ctx_capsule.record_submit(fence);

        // Transition context to Active if needed
        if ctx_capsule.state() == AmdgpuContextState::Ready {
            let _ = ctx_capsule.mark_active();
        }

        Ok(fence)
    }

    // ========================================================================
    // Info Queries
    // ========================================================================

    /// Query device info
    fn query_device_info_internal(drm_fd: i32) -> KgpuDriverResult<AmdgpuDevInfo> {
        let mut dev_info = AmdgpuDevInfo::default();

        let mut req = AmdgpuInfoFfi {
            query: AmdgpuInfoId::DevInfo as u32,
            _pad: 0,
            return_pointer: &mut dev_info as *mut _ as u64,
            return_size: core::mem::size_of::<AmdgpuDevInfo>() as u32,
            query_param: 0,
        };

        // SAFETY: ioctl with properly initialized structure
        let ret = unsafe {
            libc::ioctl(
                drm_fd,
                DRM_IOCTL_AMDGPU_INFO as libc::c_ulong,
                &mut req as *mut _,
            )
        };

        if ret < 0 {
            return Err(Self::errno_to_error());
        }

        Ok(dev_info)
    }

    /// Query VRAM usage
    pub fn query_vram_usage(&self) -> KgpuDriverResult<u64> {
        let mut usage: u64 = 0;

        let mut req = AmdgpuInfoFfi {
            query: AmdgpuInfoId::VramUsage as u32,
            _pad: 0,
            return_pointer: &mut usage as *mut _ as u64,
            return_size: 8,
            query_param: 0,
        };

        // SAFETY: ioctl with properly initialized structure
        let ret = unsafe {
            libc::ioctl(
                self.drm_fd,
                DRM_IOCTL_AMDGPU_INFO as libc::c_ulong,
                &mut req as *mut _,
            )
        };

        if ret < 0 {
            return Err(Self::errno_to_error());
        }

        Ok(usage)
    }

    /// Query GTT usage
    pub fn query_gtt_usage(&self) -> KgpuDriverResult<u64> {
        let mut usage: u64 = 0;

        let mut req = AmdgpuInfoFfi {
            query: AmdgpuInfoId::GttUsage as u32,
            _pad: 0,
            return_pointer: &mut usage as *mut _ as u64,
            return_size: 8,
            query_param: 0,
        };

        // SAFETY: ioctl with properly initialized structure
        let ret = unsafe {
            libc::ioctl(
                self.drm_fd,
                DRM_IOCTL_AMDGPU_INFO as libc::c_ulong,
                &mut req as *mut _,
            )
        };

        if ret < 0 {
            return Err(Self::errno_to_error());
        }

        Ok(usage)
    }

    /// Query firmware version
    ///
    /// # Arguments
    ///
    /// * `fw_type` - Firmware type (0=VCE, 1=UVD, 2=GMC, 3=GFX_ME, etc.)
    ///
    /// # Returns
    ///
    /// - `Ok((version, feature))` - Firmware version and feature mask
    pub fn query_firmware_version(&self, fw_type: u32) -> KgpuDriverResult<(u32, u32)> {
        #[repr(C)]
        struct FwVersion {
            version: u32,
            feature: u32,
        }

        let mut fw_ver = FwVersion { version: 0, feature: 0 };

        let mut req = AmdgpuInfoFfi {
            query: AmdgpuInfoId::FwVersion as u32,
            _pad: 0,
            return_pointer: &mut fw_ver as *mut _ as u64,
            return_size: 8,
            query_param: fw_type,
        };

        // SAFETY: ioctl with properly initialized structure
        let ret = unsafe {
            libc::ioctl(
                self.drm_fd,
                DRM_IOCTL_AMDGPU_INFO as libc::c_ulong,
                &mut req as *mut _,
            )
        };

        if ret < 0 {
            return Err(Self::errno_to_error());
        }

        Ok((fw_ver.version, fw_ver.feature))
    }

    /// Query sensor info (temperature, fan, power)
    ///
    /// # Arguments
    ///
    /// * `sensor_type` - Sensor type to query
    ///
    /// # Returns
    ///
    /// - `Ok(value)` - Sensor value
    pub fn query_sensor(&self, sensor_type: u32) -> KgpuDriverResult<u32> {
        let mut value: u32 = 0;

        let mut req = AmdgpuInfoFfi {
            query: AmdgpuInfoId::SensorInfo as u32,
            _pad: 0,
            return_pointer: &mut value as *mut _ as u64,
            return_size: 4,
            query_param: sensor_type,
        };

        // SAFETY: ioctl with properly initialized structure
        let ret = unsafe {
            libc::ioctl(
                self.drm_fd,
                DRM_IOCTL_AMDGPU_INFO as libc::c_ulong,
                &mut req as *mut _,
            )
        };

        if ret < 0 {
            return Err(Self::errno_to_error());
        }

        Ok(value)
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    /// Detect GPU generation from device info
    fn detect_generation(dev_info: &AmdgpuDevInfo) -> GpuGeneration {
        // AMD family IDs (from amd/amdgpu/amdgpu.h)
        const FAMILY_SI: u32 = 110;      // Southern Islands (GCN 1.0)
        const FAMILY_CI: u32 = 120;      // Sea Islands (GCN 2.0)
        const FAMILY_KV: u32 = 125;      // Kaveri (APU)
        const FAMILY_VI: u32 = 130;      // Volcanic Islands (GCN 3.0)
        const FAMILY_CZ: u32 = 135;      // Carrizo (APU)
        const FAMILY_AI: u32 = 141;      // Vega (GCN 5.0)
        const FAMILY_RV: u32 = 142;      // Raven (APU)
        const FAMILY_NV: u32 = 143;      // Navi (RDNA 1.0)
        const FAMILY_VGH: u32 = 144;     // Van Gogh (APU)
        const FAMILY_GC_10_3_0: u32 = 145;  // RDNA 2.0
        const FAMILY_GC_10_3_6: u32 = 146;
        const FAMILY_GC_10_3_7: u32 = 147;
        const FAMILY_GC_11_0_0: u32 = 148;  // RDNA 3.0
        const FAMILY_GC_11_0_1: u32 = 149;
        const FAMILY_GC_11_5_0: u32 = 150;  // RDNA 3.5

        match dev_info.family {
            FAMILY_SI => GpuGeneration::AmdGcn1,
            FAMILY_CI | FAMILY_KV => GpuGeneration::AmdGcn2,
            FAMILY_VI | FAMILY_CZ => GpuGeneration::AmdGcn3,
            FAMILY_AI | FAMILY_RV => GpuGeneration::AmdGcn5,
            FAMILY_NV | FAMILY_VGH => GpuGeneration::AmdRdna1,
            FAMILY_GC_10_3_0 | FAMILY_GC_10_3_6 | FAMILY_GC_10_3_7 => GpuGeneration::AmdRdna2,
            FAMILY_GC_11_0_0 | FAMILY_GC_11_0_1 => GpuGeneration::AmdRdna3,
            FAMILY_GC_11_5_0 => GpuGeneration::AmdRdna4,
            _ => GpuGeneration::Unknown,
        }
    }

    /// Convert errno to KgpuDriverError
    fn errno_to_error() -> KgpuDriverError {
        // SAFETY: errno is thread-local
        let errno = unsafe { *libc::__errno_location() };

        match errno {
            libc::ENOENT | libc::ENODEV => KgpuDriverError::DeviceNotFound,
            libc::EACCES | libc::EPERM => KgpuDriverError::PermissionDenied,
            libc::EBUSY => KgpuDriverError::DeviceBusy,
            libc::EINVAL => KgpuDriverError::InvalidParameter,
            libc::ENOMEM => KgpuDriverError::OutOfDeviceMemory,
            libc::ENOSPC => KgpuDriverError::OutOfDeviceMemory,
            libc::ETIMEDOUT => KgpuDriverError::CommandTimeout,
            _ => KgpuDriverError::DrmIoctlFailed,
        }
    }
}

#[cfg(all(feature = "kgpu-driver-amd", target_os = "linux"))]
impl fmt::Debug for AmdgpuDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AmdgpuDriver")
            .field("drm_fd", &self.drm_fd)
            .field("generation", &self.generation)
            .field("device_id", &self.dev_info.device_id)
            .field("num_cu", &self.dev_info.num_cu)
            .field("vram_size", &self.dev_info.vram_size)
            .finish()
    }
}

// ============================================================================
// Tests (T28 Compliant)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem;

    // ========================================================================
    // Q1-Q7: Unit Tests - Struct Layout and Size
    // ========================================================================

    #[test]
    fn test_context_capsule_size() {
        // T28 Q1: Verify AmdgpuContextCapsule is exactly 256 bytes
        assert_eq!(mem::size_of::<AmdgpuContextCapsule>(), 256);
    }

    #[test]
    fn test_context_capsule_alignment() {
        // T28 Q2: Verify 256-byte alignment
        assert_eq!(mem::align_of::<AmdgpuContextCapsule>(), 256);
    }

    #[test]
    fn test_context_capsule_new() {
        // T28 Q3: Verify initial state
        let ctx = AmdgpuContextCapsule::new();
        assert_eq!(ctx.state(), AmdgpuContextState::Unallocated);
        assert_eq!(ctx.generation(), 0);
        assert_eq!(ctx.ctx_id(), 0);
        assert_eq!(ctx.submit_count(), 0);
    }

    #[test]
    fn test_domain_flags() {
        // T28 Q4: Verify domain bitflags
        let vram = AmdgpuDomain::VRAM;
        let gtt = AmdgpuDomain::GTT;
        let both = vram | gtt;

        assert!(both.contains_vram());
        assert!(both.contains_gtt());
        assert!(!both.contains_cpu());
        assert!(both.is_gpu_accessible());
    }

    #[test]
    fn test_bo_flags() {
        // T28 Q5: Verify BO flags
        let flags = AmdgpuBoFlags::CPU_ACCESS_REQUIRED | AmdgpuBoFlags::VRAM_CLEARED;
        assert!(flags.contains(AmdgpuBoFlags::CPU_ACCESS_REQUIRED));
        assert!(flags.contains(AmdgpuBoFlags::VRAM_CLEARED));
        assert!(!flags.contains(AmdgpuBoFlags::ENCRYPTED));
    }

    #[test]
    fn test_hw_ip_types() {
        // T28 Q6: Verify hardware IP types
        assert_eq!(AmdgpuHwIp::Gfx.to_u32(), 0);
        assert_eq!(AmdgpuHwIp::Compute.to_u32(), 1);
        assert_eq!(AmdgpuHwIp::Dma.to_u32(), 2);
        assert!(AmdgpuHwIp::VcnDec.is_video());
        assert!(!AmdgpuHwIp::Gfx.is_video());
    }

    #[test]
    fn test_hw_ip_to_queue_type() {
        // T28 Q7: Verify IP to queue type mapping
        assert_eq!(AmdgpuHwIp::Gfx.to_queue_type(), AmdQueueType::Gfx);
        assert_eq!(AmdgpuHwIp::Compute.to_queue_type(), AmdQueueType::Compute);
        assert_eq!(AmdgpuHwIp::Dma.to_queue_type(), AmdQueueType::Dma);
    }

    // ========================================================================
    // Q8-Q14: Unit Tests - State Transitions
    // ========================================================================

    #[test]
    fn test_context_initialize() {
        // T28 Q8: Verify context initialization
        let ctx = AmdgpuContextCapsule::new();
        let result = ctx.initialize(42, 10, AmdgpuHwIp::Gfx, 0, 0);

        assert!(result.is_ok());
        assert_eq!(ctx.state(), AmdgpuContextState::Ready);
        assert_eq!(ctx.generation(), 1);
        assert_eq!(ctx.ctx_id(), 42);
        assert_eq!(ctx.drm_fd(), 10);
    }

    #[test]
    fn test_context_double_initialize() {
        // T28 Q9: Verify double initialization fails
        let ctx = AmdgpuContextCapsule::new();
        ctx.initialize(1, 10, AmdgpuHwIp::Gfx, 0, 0).unwrap();

        let result = ctx.initialize(2, 11, AmdgpuHwIp::Compute, 0, 0);
        assert_eq!(result, Err(KgpuDriverError::InvalidState));
    }

    #[test]
    fn test_context_mark_active() {
        // T28 Q10: Verify Ready -> Active transition
        let ctx = AmdgpuContextCapsule::new();
        ctx.initialize(1, 10, AmdgpuHwIp::Gfx, 0, 0).unwrap();

        let result = ctx.mark_active();
        assert!(result.is_ok());
        assert_eq!(ctx.state(), AmdgpuContextState::Active);
        assert_eq!(ctx.generation(), 2);
    }

    #[test]
    fn test_context_mark_idle() {
        // T28 Q11: Verify Active -> Ready transition
        let ctx = AmdgpuContextCapsule::new();
        ctx.initialize(1, 10, AmdgpuHwIp::Gfx, 0, 0).unwrap();
        ctx.mark_active().unwrap();

        let result = ctx.mark_idle();
        assert!(result.is_ok());
        assert_eq!(ctx.state(), AmdgpuContextState::Ready);
    }

    #[test]
    fn test_context_mark_error() {
        // T28 Q12: Verify error state transition
        let ctx = AmdgpuContextCapsule::new();
        ctx.initialize(1, 10, AmdgpuHwIp::Gfx, 0, 0).unwrap();

        ctx.mark_error().unwrap();
        assert_eq!(ctx.state(), AmdgpuContextState::Error);
        assert!(!ctx.state().is_usable());
        assert_eq!(ctx.error_count(), 1);
    }

    #[test]
    fn test_context_mark_freed() {
        // T28 Q13: Verify freed transition
        let ctx = AmdgpuContextCapsule::new();
        ctx.initialize(1, 10, AmdgpuHwIp::Gfx, 0, 0).unwrap();

        ctx.mark_freed().unwrap();
        assert_eq!(ctx.state(), AmdgpuContextState::Unallocated);
        assert_eq!(ctx.ctx_id(), 0);
    }

    #[test]
    fn test_context_state_predicates() {
        // T28 Q14: Verify state predicates
        assert!(!AmdgpuContextState::Unallocated.is_usable());
        assert!(AmdgpuContextState::Ready.is_usable());
        assert!(AmdgpuContextState::Active.is_usable());
        assert!(!AmdgpuContextState::Error.is_usable());
        assert!(!AmdgpuContextState::Suspended.is_usable());
    }

    // ========================================================================
    // Q15-Q21: Unit Tests - Memory and Fence Tracking
    // ========================================================================

    #[test]
    fn test_context_record_submit() {
        // T28 Q15: Verify submit tracking
        let ctx = AmdgpuContextCapsule::new();
        ctx.initialize(1, 10, AmdgpuHwIp::Gfx, 0, 0).unwrap();

        ctx.record_submit(100);
        assert_eq!(ctx.submit_count(), 1);
        assert_eq!(ctx.pending_fence(), 100);

        ctx.record_submit(200);
        assert_eq!(ctx.submit_count(), 2);
        assert_eq!(ctx.pending_fence(), 200);
    }

    #[test]
    fn test_context_record_completion() {
        // T28 Q16: Verify completion tracking
        let ctx = AmdgpuContextCapsule::new();
        ctx.initialize(1, 10, AmdgpuHwIp::Gfx, 0, 0).unwrap();

        ctx.record_submit(100);
        assert!(ctx.has_pending_work());

        ctx.record_completion(100);
        assert_eq!(ctx.completed_fence(), 100);
        assert!(!ctx.has_pending_work());
    }

    #[test]
    fn test_context_memory_usage() {
        // T28 Q17: Verify memory usage tracking
        let ctx = AmdgpuContextCapsule::new();
        ctx.initialize(1, 10, AmdgpuHwIp::Gfx, 0, 0).unwrap();

        ctx.update_memory_usage(1024 * 1024, 512 * 1024);
        assert_eq!(ctx.vram_usage(), 1024 * 1024);
        assert_eq!(ctx.gtt_usage(), 512 * 1024);

        ctx.update_memory_usage(-(512 * 1024), 0);
        assert_eq!(ctx.vram_usage(), 512 * 1024);
    }

    #[test]
    fn test_context_bo_count() {
        // T28 Q18: Verify BO counting
        let ctx = AmdgpuContextCapsule::new();
        ctx.initialize(1, 10, AmdgpuHwIp::Gfx, 0, 0).unwrap();

        ctx.add_bo();
        ctx.add_bo();
        assert_eq!(ctx.bo_count(), 2);

        ctx.remove_bo();
        assert_eq!(ctx.bo_count(), 1);
    }

    #[test]
    fn test_context_snapshot() {
        // T28 Q19: Verify snapshot captures all fields
        let ctx = AmdgpuContextCapsule::new();
        ctx.initialize(42, 10, AmdgpuHwIp::Compute, 1, 5).unwrap();
        ctx.mark_active().unwrap();
        ctx.record_submit(100);
        ctx.update_memory_usage(1024, 512);

        let snap = ctx.snapshot();
        assert_eq!(snap.state, AmdgpuContextState::Active);
        assert_eq!(snap.ctx_id, 42);
        assert_eq!(snap.priority, 5);
        assert_eq!(snap.pending_fence, 100);
        assert_eq!(snap.vram_usage, 1024);
        assert_eq!(snap.gtt_usage, 512);
    }

    #[test]
    fn test_snapshot_total_memory() {
        // T28 Q20: Verify snapshot memory calculation
        let ctx = AmdgpuContextCapsule::new();
        ctx.initialize(1, 10, AmdgpuHwIp::Gfx, 0, 0).unwrap();
        ctx.update_memory_usage(1024, 512);

        let snap = ctx.snapshot();
        assert_eq!(snap.total_memory_usage(), 1024 + 512);
    }

    #[test]
    fn test_domain_display() {
        // T28 Q21: Verify domain display
        let domains = AmdgpuDomain::VRAM | AmdgpuDomain::GTT;
        let display = format!("{}", domains);
        assert!(display.contains("VRAM"));
        assert!(display.contains("GTT"));
    }

    // ========================================================================
    // Q22-Q28: Unit Tests - FFI Structures
    // ========================================================================

    #[test]
    fn test_gem_create_struct_size() {
        // T28 Q22: Verify GEM create struct sizes
        assert_eq!(mem::size_of::<AmdgpuGemCreateIn>(), 32);
        assert_eq!(mem::size_of::<AmdgpuGemCreateOut>(), 8);
    }

    #[test]
    fn test_gem_mmap_struct_size() {
        // T28 Q23: Verify GEM mmap struct sizes
        assert_eq!(mem::size_of::<AmdgpuGemMmapIn>(), 8);
        assert_eq!(mem::size_of::<AmdgpuGemMmapOut>(), 8);
    }

    #[test]
    fn test_ctx_struct_size() {
        // T28 Q24: Verify context struct sizes
        assert_eq!(mem::size_of::<AmdgpuCtxIn>(), 16);
        assert_eq!(mem::size_of::<AmdgpuCtxOut>(), 16);
    }

    #[test]
    fn test_cs_ib_struct() {
        // T28 Q25: Verify CS IB struct
        assert_eq!(mem::size_of::<AmdgpuCsIb>(), 16);
        let ib = AmdgpuCsIb {
            va_start: 0x1000_0000,
            ib_bytes: 256,
            flags: 0,
        };
        assert_eq!(ib.va_start, 0x1000_0000);
    }

    #[test]
    fn test_info_struct_size() {
        // T28 Q26: Verify info query struct
        assert_eq!(mem::size_of::<AmdgpuInfoFfi>(), 24);
    }

    #[test]
    fn test_dev_info_struct() {
        // T28 Q27: Verify device info struct is reasonably sized
        assert!(mem::size_of::<AmdgpuDevInfo>() <= 512);
    }

    #[test]
    fn test_info_id_names() {
        // T28 Q28: Verify info ID names
        assert_eq!(AmdgpuInfoId::VramUsage.name(), "VRAM_USAGE");
        assert_eq!(AmdgpuInfoId::DevInfo.name(), "DEV_INFO");
        assert_eq!(AmdgpuInfoId::SensorInfo.name(), "SENSOR_INFO");
    }

    // ========================================================================
    // Q29-Q35: Determinism Tests
    // ========================================================================

    #[test]
    fn test_generation_monotonic() {
        // T28 Q29: Verify generation increments monotonically
        let ctx = AmdgpuContextCapsule::new();
        assert_eq!(ctx.generation(), 0);

        ctx.initialize(1, 10, AmdgpuHwIp::Gfx, 0, 0).unwrap();
        assert_eq!(ctx.generation(), 1);

        ctx.mark_active().unwrap();
        assert_eq!(ctx.generation(), 2);

        ctx.mark_idle().unwrap();
        assert_eq!(ctx.generation(), 3);

        ctx.mark_error().unwrap();
        assert_eq!(ctx.generation(), 4);
    }

    #[test]
    fn test_ctx_op_values() {
        // T28 Q30: Verify context op values
        assert_eq!(AmdgpuCtxOp::AllocCtx as u32, 1);
        assert_eq!(AmdgpuCtxOp::FreeCtx as u32, 2);
        assert_eq!(AmdgpuCtxOp::QueryState as u32, 3);
    }

    #[test]
    fn test_info_id_values() {
        // T28 Q31: Verify info ID values
        assert_eq!(AmdgpuInfoId::VramUsage as u32, 0x07);
        assert_eq!(AmdgpuInfoId::GttUsage as u32, 0x08);
        assert_eq!(AmdgpuInfoId::DevInfo as u32, 0x0C);
    }

    #[test]
    fn test_domain_union() {
        // T28 Q32: Verify domain union operation
        let a = AmdgpuDomain::VRAM;
        let b = AmdgpuDomain::GTT;
        let c = a.union(b);

        assert_eq!(c.bits(), 0x6); // 0x4 | 0x2
        assert!(c.contains_vram());
        assert!(c.contains_gtt());
    }

    #[test]
    fn test_flags_union() {
        // T28 Q33: Verify flags union operation
        let a = AmdgpuBoFlags::CPU_ACCESS_REQUIRED;
        let b = AmdgpuBoFlags::VRAM_CLEARED;
        let c = a.union(b);

        assert!(c.contains(a));
        assert!(c.contains(b));
    }

    #[test]
    fn test_send_sync_traits() {
        // T28 Q34: Verify Send + Sync implementation
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AmdgpuContextCapsule>();
        assert_send_sync::<AmdgpuContextSnapshot>();
    }

    #[test]
    fn test_debug_impl() {
        // T28 Q35: Verify Debug implementation
        let ctx = AmdgpuContextCapsule::new();
        ctx.initialize(42, 10, AmdgpuHwIp::Gfx, 0, 0).unwrap();

        let debug_str = format!("{:?}", ctx);
        assert!(debug_str.contains("AmdgpuContextCapsule"));
        assert!(debug_str.contains("Ready"));
        assert!(debug_str.contains("42"));
    }

    // ========================================================================
    // Additional Tests for Coverage
    // ========================================================================

    #[test]
    fn test_hw_ip_from_u32() {
        // Verify from_u32 round-trips
        for ip in [
            AmdgpuHwIp::Gfx,
            AmdgpuHwIp::Compute,
            AmdgpuHwIp::Dma,
            AmdgpuHwIp::VcnDec,
        ] {
            let v = ip.to_u32();
            let restored = AmdgpuHwIp::from_u32(v);
            assert_eq!(restored, Some(ip));
        }

        // Invalid value
        assert_eq!(AmdgpuHwIp::from_u32(100), None);
    }

    #[test]
    fn test_info_id_from_u32() {
        // Verify from_u32 for info IDs
        assert_eq!(AmdgpuInfoId::from_u32(0x07), Some(AmdgpuInfoId::VramUsage));
        assert_eq!(AmdgpuInfoId::from_u32(0x0C), Some(AmdgpuInfoId::DevInfo));
        assert_eq!(AmdgpuInfoId::from_u32(0xFF), None);
    }

    #[test]
    fn test_context_state_from_u8() {
        // Verify state from_u8
        assert_eq!(AmdgpuContextState::from_u8(0), AmdgpuContextState::Unallocated);
        assert_eq!(AmdgpuContextState::from_u8(1), AmdgpuContextState::Ready);
        assert_eq!(AmdgpuContextState::from_u8(2), AmdgpuContextState::Active);
        assert_eq!(AmdgpuContextState::from_u8(3), AmdgpuContextState::Error);
        assert_eq!(AmdgpuContextState::from_u8(4), AmdgpuContextState::Suspended);
        assert_eq!(AmdgpuContextState::from_u8(99), AmdgpuContextState::Unallocated);
    }

    #[test]
    fn test_domain_empty() {
        // Verify empty domain
        let empty = AmdgpuDomain::new(0);
        assert!(empty.is_empty());
        assert!(!empty.is_gpu_accessible());
    }

    #[test]
    fn test_bo_flags_empty() {
        // Verify empty flags
        let empty = AmdgpuBoFlags::empty();
        assert_eq!(empty.bits(), 0);
        assert!(!empty.contains(AmdgpuBoFlags::CPU_ACCESS_REQUIRED));
    }

    #[test]
    fn test_remove_bo_at_zero() {
        // Verify remove_bo doesn't underflow
        let ctx = AmdgpuContextCapsule::new();
        ctx.initialize(1, 10, AmdgpuHwIp::Gfx, 0, 0).unwrap();

        assert_eq!(ctx.bo_count(), 0);
        ctx.remove_bo();
        assert_eq!(ctx.bo_count(), 0); // Still 0, didn't underflow
    }

    #[test]
    fn test_memory_usage_negative_saturates() {
        // Verify negative memory delta saturates to 0
        let ctx = AmdgpuContextCapsule::new();
        ctx.initialize(1, 10, AmdgpuHwIp::Gfx, 0, 0).unwrap();

        ctx.update_memory_usage(100, 100);
        ctx.update_memory_usage(-200, -200);

        assert_eq!(ctx.vram_usage(), 0);
        assert_eq!(ctx.gtt_usage(), 0);
    }

    #[test]
    fn test_ctx_op_from_u32() {
        // Verify context op from_u32
        assert_eq!(AmdgpuCtxOp::from_u32(1), Some(AmdgpuCtxOp::AllocCtx));
        assert_eq!(AmdgpuCtxOp::from_u32(2), Some(AmdgpuCtxOp::FreeCtx));
        assert_eq!(AmdgpuCtxOp::from_u32(99), None);
    }

    #[test]
    fn test_domain_bitand() {
        // Verify bitand operation
        let both = AmdgpuDomain::VRAM | AmdgpuDomain::GTT;
        let only_vram = both & AmdgpuDomain::VRAM;

        assert!(only_vram.contains_vram());
        assert!(!only_vram.contains_gtt());
    }

    #[test]
    fn test_default_impls() {
        // Verify Default implementations
        let domain: AmdgpuDomain = Default::default();
        assert_eq!(domain, AmdgpuDomain::VRAM);

        let flags: AmdgpuBoFlags = Default::default();
        assert_eq!(flags.bits(), 0);

        let snap: AmdgpuContextSnapshot = Default::default();
        assert_eq!(snap.state, AmdgpuContextState::Unallocated);
    }

    #[test]
    fn test_ioctl_constants() {
        // Verify ioctl constant encoding
        assert_eq!(DRM_AMDGPU_GEM_CREATE, 0x00);
        assert_eq!(DRM_AMDGPU_CS, 0x04);
        assert_eq!(DRM_AMDGPU_INFO, 0x05);

        // Verify encoded ioctl values have correct base
        assert_eq!(DRM_IOCTL_AMDGPU_GEM_CREATE & 0xFF, 0x40);
    }
}
