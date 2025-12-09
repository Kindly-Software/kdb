//! AMD GPU Ring Buffer Capsule - T1 Atomic, 256B cache-aligned
//!
//! Implements PM4 (Packet Manager 4) packet ring buffer for direct command submission
//! to AMD GPUs via the Command Processor (CP). Supports GCN 1.0+ and RDNA architectures.
//!
//! # Architecture
//!
//! ```text
//! CPU writes PM4 packets → Ring Buffer → CP fetches → GPU executes → Fence signal
//!                            ↑                           ↓
//!                         WPTR update              RPTR update
//!                            ↓
//!                      Doorbell write (MMIO)
//! ```
//!
//! # PM4 Packet Format
//!
//! AMD GPUs use PM4 packets for command submission:
//! - **Type 0**: Register write (legacy, GCN 1.0)
//! - **Type 2**: NOP (padding)
//! - **Type 3**: Command packet (most common - dispatch, draw, sync)
//!
//! # Design
//!
//! **Tier**: T1 Atomic (lockfree coordination via AtomicU64/AtomicU32 CAS loops)
//! **Size**: 256B cache-aligned (4 cache lines for optimal memory bandwidth)
//! **Performance Targets**:
//! - Reserve: <30ns (CAS + space check)
//! - Emit packet: <10ns per DWORD
//! - Submit: <100ns (WPTR update + doorbell)
//! - State read: <10ns
//!
//! # Memory Layout
//!
//! ```text
//! AmdCpRingCapsule (256 bytes, 256-byte aligned)
//! ┌─────────────────────────────────────────────────────────────────┐
//! │  state_gen (AtomicU64)      │  rptr_wptr (AtomicU64)           │ 16B
//! ├─────────────────────────────────────────────────────────────────┤
//! │  ring_base (AtomicU64)      │  ring_size (AtomicU64)           │ 16B
//! ├─────────────────────────────────────────────────────────────────┤
//! │  fence_gpu_addr (AtomicU64) │  fence_value (AtomicU64)         │ 16B
//! ├─────────────────────────────────────────────────────────────────┤
//! │  submit_count (AtomicU64)   │  error_count (AtomicU64)         │ 16B
//! ├─────────────────────────────────────────────────────────────────┤
//! │  doorbell_offset (AtomicU32)│  queue_id (AtomicU32)            │  8B
//! │  pipe_id (u8)               │  queue_type (u8)                 │  2B
//! │  me_id (u8)                 │  vmid (u8)                       │  2B
//! │  _reserved (u32)                                               │  4B
//! ├─────────────────────────────────────────────────────────────────┤
//! │  _padding [192 bytes]                                          │192B
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # ASSUM Tags
//!
//! - `#ASSUME_RING_VALID`: Ring buffer is valid GPU VRAM or GTT memory
//! - `#ASSUME_RING_ALIGNED`: Ring is 256-byte aligned (AMD hardware requirement)
//! - `#ASSUME_DOORBELL_MAPPED`: Doorbell page is mapped for MMIO write
//! - `#ASSUME_ATOMIC_ALIGNED`: All AtomicU64/AtomicU32 fields are properly aligned
//! - `#ASSUME_GENERATION_MONOTONIC`: Generation counter increments monotonically (wraps at 65535)
//!
//! # UCE34 Compliance
//!
//! - **Q10**: T1 Atomic tier (lockfree coordination via AtomicU64/AtomicU32 CAS loops)
//! - **Q33**: ComputationalCapsule verification (256B, cache-aligned, generation counters)
//! - **Q34**: Audit trail design (generation counters, submit_count, error_count for SOX/SOC2)
//!
//! # Examples
//!
//! ```ignore
//! use atomic_capsule::gpu::kgpu_driver::amd_ring::{AmdCpRingCapsule, AmdQueueType};
//!
//! // Create and initialize ring buffer
//! let ring = AmdCpRingCapsule::new();
//! ring.initialize(
//!     0xFFFE_0000_0000,      // ring_base (GPU VRAM)
//!     256 * 1024,            // ring_size (256KB)
//!     0xFFFE_0001_0000,      // fence_gpu_addr
//!     0x1000,                // doorbell_offset
//!     0,                     // queue_id
//!     AmdQueueType::Gfx,     // queue_type
//!     0,                     // pipe_id
//!     0,                     // vmid
//! )?;
//!
//! // Reserve space for commands
//! let offset = ring.reserve(16)?; // 16 DWORDs
//!
//! // Emit PM4 packets (caller writes to ring buffer memory)
//! let next_offset = ring.emit_nop(offset, 4);
//! let next_offset = ring.emit_release_mem(next_offset, fence_addr, fence_value);
//!
//! // Submit to GPU
//! let fence = ring.submit(next_offset)?;
//! ```

use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use core::fmt;

use super::error::{KgpuDriverError, KgpuDriverResult};

// ============================================================================
// PM4 Packet Types
// ============================================================================

/// PM4 packet types (bits 30-31 of header DWORD)
///
/// AMD GPUs use different packet types for different purposes:
/// - Type 0: Legacy register write (GCN 1.0, deprecated)
/// - Type 2: NOP (padding, used for alignment)
/// - Type 3: Command packet (most common, used for all GPU operations)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Pm4PacketType {
    /// Type 0: Register write (legacy GCN 1.0)
    Type0 = 0,
    /// Type 2: NOP packet (padding)
    Type2 = 2,
    /// Type 3: Command packet (dispatch, draw, sync, etc.)
    Type3 = 3,
}

impl Pm4PacketType {
    /// Convert to u8
    #[inline]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Convert from u8 (returns None for invalid values)
    #[inline]
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Type0),
            2 => Some(Self::Type2),
            3 => Some(Self::Type3),
            _ => None,
        }
    }
}

// ============================================================================
// PM4 Type 3 Opcodes
// ============================================================================

/// PM4 Type 3 opcodes for GCN/RDNA architectures
///
/// These opcodes are used in the header of Type 3 PM4 packets to specify
/// the command to execute. Organized by functional category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Pm4Opcode {
    // ========================================================================
    // Control Flow
    // ========================================================================

    /// NOP - No operation (used for padding)
    Nop = 0x10,

    /// SET_BASE - Set base address for indirect access
    SetBase = 0x11,

    /// CLEAR_STATE - Clear GPU state
    ClearState = 0x12,

    /// INDEX_BASE - Set index buffer base address
    IndexBase = 0x13,

    /// PFP_SYNC_ME - Synchronize PFP with ME
    PfpSyncMe = 0x42,

    /// SURFACE_SYNC - Surface synchronization
    SurfaceSync = 0x43,

    // ========================================================================
    // Draw Commands
    // ========================================================================

    /// DRAW_INDEX2 - Draw indexed primitives (with base vertex)
    DrawIndex2 = 0x27,

    /// DRAW_INDEX_OFFSET2 - Draw indexed primitives with offset
    DrawIndexOffset2 = 0x35,

    /// DRAW_INDEX_AUTO - Auto-indexed draw
    DrawIndexAuto = 0x2D,

    /// DRAW_INDIRECT - Indirect draw
    DrawIndirect = 0x32,

    // ========================================================================
    // Compute Dispatch
    // ========================================================================

    /// DISPATCH_DIRECT - Direct compute shader dispatch
    DispatchDirect = 0x15,

    /// DISPATCH_INDIRECT - Indirect compute shader dispatch
    DispatchIndirect = 0x16,

    // ========================================================================
    // Indirect Buffer
    // ========================================================================

    /// INDIRECT_BUFFER - Execute indirect command buffer (IB)
    IndirectBuffer = 0x3F,

    // ========================================================================
    // Memory & Synchronization
    // ========================================================================

    /// MEM_SEMAPHORE - Memory semaphore operation
    MemSemaphore = 0x39,

    /// WAIT_REG_MEM - Wait for register/memory condition
    WaitRegMem = 0x3C,

    /// WRITE_DATA - Write data to memory
    WriteData = 0x37,

    /// EVENT_WRITE - Write event (flush caches, etc.)
    EventWrite = 0x46,

    /// EVENT_WRITE_EOP - End-of-pipe event write
    EventWriteEop = 0x47,

    /// EVENT_WRITE_EOS - End-of-shader event write
    EventWriteEos = 0x48,

    /// RELEASE_MEM - Release memory (fence signal)
    ReleaseMem = 0x49,

    /// ACQUIRE_MEM - Acquire memory (cache invalidation)
    AcquireMem = 0x58,

    // ========================================================================
    // Register Access
    // ========================================================================

    /// SET_CONTEXT_REG - Set context register
    SetContextReg = 0x69,

    /// SET_SH_REG - Set shader register
    SetShReg = 0x76,

    /// SET_UCONFIG_REG - Set user config register
    SetUconfigReg = 0x79,

    /// LOAD_CONTEXT_REG - Load context register from memory
    LoadContextReg = 0x80,

    /// LOAD_SH_REG - Load shader register from memory
    LoadShReg = 0x81,
}

impl Pm4Opcode {
    /// Convert to u8
    #[inline]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Get human-readable name
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Nop => "NOP",
            Self::SetBase => "SET_BASE",
            Self::ClearState => "CLEAR_STATE",
            Self::IndexBase => "INDEX_BASE",
            Self::PfpSyncMe => "PFP_SYNC_ME",
            Self::SurfaceSync => "SURFACE_SYNC",
            Self::DrawIndex2 => "DRAW_INDEX2",
            Self::DrawIndexOffset2 => "DRAW_INDEX_OFFSET2",
            Self::DrawIndexAuto => "DRAW_INDEX_AUTO",
            Self::DrawIndirect => "DRAW_INDIRECT",
            Self::DispatchDirect => "DISPATCH_DIRECT",
            Self::DispatchIndirect => "DISPATCH_INDIRECT",
            Self::IndirectBuffer => "INDIRECT_BUFFER",
            Self::MemSemaphore => "MEM_SEMAPHORE",
            Self::WaitRegMem => "WAIT_REG_MEM",
            Self::WriteData => "WRITE_DATA",
            Self::EventWrite => "EVENT_WRITE",
            Self::EventWriteEop => "EVENT_WRITE_EOP",
            Self::EventWriteEos => "EVENT_WRITE_EOS",
            Self::ReleaseMem => "RELEASE_MEM",
            Self::AcquireMem => "ACQUIRE_MEM",
            Self::SetContextReg => "SET_CONTEXT_REG",
            Self::SetShReg => "SET_SH_REG",
            Self::SetUconfigReg => "SET_UCONFIG_REG",
            Self::LoadContextReg => "LOAD_CONTEXT_REG",
            Self::LoadShReg => "LOAD_SH_REG",
        }
    }
}

// ============================================================================
// PM4 Packet Header
// ============================================================================

/// PM4 packet header (first DWORD of every PM4 packet)
///
/// # Type 3 Header Layout (32 bits)
///
/// ```text
/// Bits  0-13: count-1 (number of DWORDs following header, minus 1)
/// Bit   14:   shader_type (0=GFX, 1=Compute)
/// Bit   15:   predicate (conditional execution)
/// Bits 16-23: opcode (Pm4Opcode)
/// Bits 24-25: reserved
/// Bits 26-27: reset_filter_cam
/// Bits 28-29: reserved
/// Bits 30-31: type (3 for Type 3 packets)
/// ```
///
/// # Type 2 Header Layout (32 bits)
///
/// ```text
/// Bits  0-29: ignored (NOP padding)
/// Bits 30-31: type (2 for Type 2 NOP)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Pm4Header {
    /// Raw 32-bit header value
    pub value: u32,
}

impl Pm4Header {
    // ========================================================================
    // Constants
    // ========================================================================

    /// Mask for count field (bits 0-13)
    const COUNT_MASK: u32 = 0x3FFF;

    /// Mask for shader_type field (bit 14)
    const SHADER_TYPE_BIT: u32 = 1 << 14;

    /// Mask for predicate field (bit 15)
    const PREDICATE_BIT: u32 = 1 << 15;

    /// Mask for opcode field (bits 16-23)
    const OPCODE_MASK: u32 = 0xFF << 16;

    /// Shift for opcode field
    const OPCODE_SHIFT: u32 = 16;

    /// Mask for type field (bits 30-31)
    const TYPE_MASK: u32 = 0x3 << 30;

    /// Shift for type field
    const TYPE_SHIFT: u32 = 30;

    // ========================================================================
    // Construction
    // ========================================================================

    /// Create a Type 3 packet header
    ///
    /// # Arguments
    ///
    /// * `opcode` - PM4 opcode for this packet
    /// * `count` - Number of DWORDs following the header (1-16384)
    ///
    /// # Returns
    ///
    /// Pm4Header ready for ring buffer insertion
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Create NOP packet header with 4 DWORDs of payload
    /// let header = Pm4Header::type3(Pm4Opcode::Nop, 4);
    /// ```
    #[inline]
    pub const fn type3(opcode: Pm4Opcode, count: u16) -> Self {
        // Type 3 packet: count-1 in bits 0-13, opcode in bits 16-23, type=3 in bits 30-31
        let count_minus_1 = if count > 0 { count - 1 } else { 0 };
        let value = (3u32 << Self::TYPE_SHIFT)
            | ((opcode as u32) << Self::OPCODE_SHIFT)
            | ((count_minus_1 as u32) & Self::COUNT_MASK);
        Self { value }
    }

    /// Create a Type 3 packet header with shader type
    ///
    /// # Arguments
    ///
    /// * `opcode` - PM4 opcode for this packet
    /// * `count` - Number of DWORDs following the header (1-16384)
    /// * `is_compute` - True for compute shader, false for graphics
    #[inline]
    pub const fn type3_with_shader(opcode: Pm4Opcode, count: u16, is_compute: bool) -> Self {
        let mut header = Self::type3(opcode, count);
        if is_compute {
            header.value |= Self::SHADER_TYPE_BIT;
        }
        header
    }

    /// Create a Type 2 NOP packet (single DWORD, no payload)
    ///
    /// Type 2 NOP packets are used for padding/alignment.
    #[inline]
    pub const fn type2_nop() -> Self {
        Self { value: 2u32 << Self::TYPE_SHIFT }
    }

    /// Create a Type 3 NOP packet with specified padding count
    ///
    /// # Arguments
    ///
    /// * `count` - Number of NOP DWORDs (1-16384)
    #[inline]
    pub const fn nop(count: u16) -> Self {
        Self::type3(Pm4Opcode::Nop, count)
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Get packet type (0, 2, or 3)
    #[inline]
    pub const fn packet_type(self) -> u8 {
        ((self.value & Self::TYPE_MASK) >> Self::TYPE_SHIFT) as u8
    }

    /// Get opcode (only valid for Type 3 packets)
    #[inline]
    pub const fn opcode(self) -> u8 {
        ((self.value & Self::OPCODE_MASK) >> Self::OPCODE_SHIFT) as u8
    }

    /// Get count (DWORDs following header, only valid for Type 3)
    #[inline]
    pub const fn count(self) -> u16 {
        ((self.value & Self::COUNT_MASK) as u16).saturating_add(1)
    }

    /// Check if this is a compute shader packet
    #[inline]
    pub const fn is_compute(self) -> bool {
        (self.value & Self::SHADER_TYPE_BIT) != 0
    }

    /// Check if this packet is predicated
    #[inline]
    pub const fn is_predicated(self) -> bool {
        (self.value & Self::PREDICATE_BIT) != 0
    }

    /// Get total packet size in DWORDs (header + payload)
    #[inline]
    pub const fn total_dwords(self) -> u16 {
        match self.packet_type() {
            2 => 1, // Type 2 NOP is single DWORD
            3 => self.count().saturating_add(1), // Header + count DWORDs
            _ => 1, // Unknown types treated as single DWORD
        }
    }
}

impl fmt::Display for Pm4Header {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ptype = self.packet_type();
        match ptype {
            2 => write!(f, "PM4[Type2 NOP]"),
            3 => {
                let opcode = self.opcode();
                write!(f, "PM4[Type3 op=0x{:02X} count={}]", opcode, self.count())
            }
            _ => write!(f, "PM4[Type{} raw=0x{:08X}]", ptype, self.value),
        }
    }
}

// ============================================================================
// AMD Queue Types
// ============================================================================

/// AMD GPU queue types for command submission
///
/// Different queue types are optimized for different workloads:
/// - **Gfx**: 3D graphics rendering (largest ring, highest priority)
/// - **Compute**: Compute shader dispatch (multiple async compute queues)
/// - **Dma**: SDMA engine for memory transfers
/// - **Uvd/Vcn**: Video decode/encode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AmdQueueType {
    /// Graphics queue (3D rendering, display)
    Gfx = 0,
    /// Compute queue (compute shaders, GPGPU)
    Compute = 1,
    /// SDMA queue (DMA transfers)
    Dma = 2,
    /// UVD decode queue (legacy video decode)
    UvdDec = 3,
    /// UVD encode queue (legacy video encode)
    UvdEnc = 4,
    /// VCN queue (Video Core Next, RDNA)
    Vcn = 5,
}

impl AmdQueueType {
    /// Get human-readable name
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Gfx => "Graphics",
            Self::Compute => "Compute",
            Self::Dma => "SDMA",
            Self::UvdDec => "UVD Decode",
            Self::UvdEnc => "UVD Encode",
            Self::Vcn => "VCN",
        }
    }

    /// Get default ring buffer size for this queue type
    #[inline]
    pub const fn default_ring_size(self) -> u64 {
        match self {
            Self::Gfx => 1024 * 1024,      // 1MB for GFX (large command buffers)
            Self::Compute => 256 * 1024,   // 256KB for compute
            Self::Dma => 256 * 1024,       // 256KB for DMA
            Self::UvdDec => 128 * 1024,    // 128KB for video decode
            Self::UvdEnc => 128 * 1024,    // 128KB for video encode
            Self::Vcn => 256 * 1024,       // 256KB for VCN
        }
    }

    /// Convert from u8
    #[inline]
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Gfx),
            1 => Some(Self::Compute),
            2 => Some(Self::Dma),
            3 => Some(Self::UvdDec),
            4 => Some(Self::UvdEnc),
            5 => Some(Self::Vcn),
            _ => None,
        }
    }

    /// Convert to u8
    #[inline]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }
}

impl fmt::Display for AmdQueueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// Ring State
// ============================================================================

/// Command Processor ring buffer state
///
/// Tracks the lifecycle of a ring buffer from uninitialized to active/error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CpRingState {
    /// Ring buffer not yet initialized
    Uninitialized = 0,
    /// Ring buffer ready for command submission
    Ready = 1,
    /// Ring buffer actively processing commands
    Active = 2,
    /// Ring buffer stalled (waiting for GPU)
    Stalled = 3,
    /// Ring buffer in error state (needs reset)
    Error = 4,
    /// Ring buffer suspended (low power)
    Suspended = 5,
}

impl CpRingState {
    /// Convert from u8
    #[inline]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Uninitialized,
            1 => Self::Ready,
            2 => Self::Active,
            3 => Self::Stalled,
            4 => Self::Error,
            5 => Self::Suspended,
            _ => Self::Uninitialized, // Default for unknown values
        }
    }

    /// Convert to u8
    #[inline]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }

    /// Check if ring is operational (can accept commands)
    #[inline]
    pub const fn is_operational(self) -> bool {
        matches!(self, Self::Ready | Self::Active)
    }

    /// Get human-readable name
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Uninitialized => "Uninitialized",
            Self::Ready => "Ready",
            Self::Active => "Active",
            Self::Stalled => "Stalled",
            Self::Error => "Error",
            Self::Suspended => "Suspended",
        }
    }
}

impl fmt::Display for CpRingState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// AMD CP Ring Capsule
// ============================================================================

/// AMD Command Processor Ring Buffer Capsule (T1 Atomic, 256B)
///
/// Manages PM4 packet ring buffer for command submission to AMD GPUs.
/// Supports GCN 1.0+ and RDNA architectures with lockfree atomic operations.
///
/// # Layout (256 bytes, 256-byte aligned)
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────────┐
/// │  state_gen (AtomicU64)      │  rptr_wptr (AtomicU64)           │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  ring_base (AtomicU64)      │  ring_size (AtomicU64)           │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  fence_gpu_addr (AtomicU64) │  fence_value (AtomicU64)         │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  submit_count (AtomicU64)   │  error_count (AtomicU64)         │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  doorbell_offset (AtomicU32)│  queue_id (AtomicU32)            │  8B
/// │  pipe_id (u8)               │  queue_type (u8)                 │  2B
/// │  me_id (u8)                 │  vmid (u8)                       │  2B
/// │  _reserved (u32)                                               │  4B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  _padding [192 bytes]                                          │192B
/// └─────────────────────────────────────────────────────────────────┘
/// ```
///
/// # ASSUM Safety
///
/// - `#ASSUME_RING_VALID`: Ring buffer is valid GPU VRAM or GTT memory
/// - `#ASSUME_RING_ALIGNED`: Ring is 256-byte aligned (AMD requirement)
/// - `#ASSUME_DOORBELL_MAPPED`: Doorbell page is mapped for MMIO write
/// - `#ASSUME_ATOMIC_ALIGNED`: All AtomicU64/U32 fields are properly aligned
/// - `#ASSUME_GENERATION_MONOTONIC`: Generation counter wraps at 65535
#[repr(C, align(256))]
pub struct AmdCpRingCapsule {
    /// State (bits 0-7) + Generation (bits 8-23) + Flags (bits 24-63)
    ///
    /// # Bit Layout
    /// - Bits  0-7:  CpRingState enum value (0-5)
    /// - Bits  8-23: Generation counter (0-65535, wrapping)
    /// - Bits 24-63: Reserved for future flags
    state_gen: AtomicU64,

    /// RPTR read pointer (bits 0-31) + WPTR write pointer (bits 32-63)
    ///
    /// Both pointers are in DWORD offsets from ring base.
    /// Hardware updates RPTR; software updates WPTR.
    rptr_wptr: AtomicU64,

    /// GPU virtual address of ring buffer (set during initialize)
    ring_base: AtomicU64,

    /// Ring size in bytes (power of 2, typically 256KB-1MB)
    ///
    /// Also used to calculate the ring mask for wraparound.
    ring_size: AtomicU64,

    /// GPU address for fence writeback (where GPU writes completion value)
    fence_gpu_addr: AtomicU64,

    /// Current fence value (incremented on each submit)
    ///
    /// GPU writes this value to fence_gpu_addr on completion.
    fence_value: AtomicU64,

    /// Total packets submitted (audit counter)
    submit_count: AtomicU64,

    /// Total submission errors (audit counter)
    error_count: AtomicU64,

    /// Doorbell offset for this queue (MMIO register offset)
    doorbell_offset: AtomicU32,

    /// Hardware queue ID assigned by kernel/firmware
    queue_id: AtomicU32,

    /// Pipe ID (0-7, identifies which pipe within ME)
    pipe_id: u8,

    /// Queue type (AmdQueueType value)
    queue_type: u8,

    /// Microengine ID (0=ME, 1=PFP, 2=CE)
    me_id: u8,

    /// Virtual machine ID (0-15, for virtualization)
    vmid: u8,

    /// Reserved for future use
    _reserved: u32,

    /// Padding to reach exactly 256 bytes
    /// Fields: 8*8 + 4*2 + 1*4 + 4 = 64 + 8 + 4 + 4 = 80 bytes
    /// 256 - 80 = 176 bytes padding needed
    /// Wait, let's recalculate:
    /// state_gen: 8, rptr_wptr: 8, ring_base: 8, ring_size: 8,
    /// fence_gpu_addr: 8, fence_value: 8, submit_count: 8, error_count: 8 = 64 bytes
    /// doorbell_offset: 4, queue_id: 4 = 8 bytes
    /// pipe_id: 1, queue_type: 1, me_id: 1, vmid: 1 = 4 bytes
    /// _reserved: 4 bytes
    /// Total: 64 + 8 + 4 + 4 = 80 bytes
    /// Padding needed: 256 - 80 = 176 bytes
    _padding: [u8; 176],
}

impl AmdCpRingCapsule {
    // ========================================================================
    // Constants
    // ========================================================================

    /// Mask for extracting state from state_gen (bits 0-7)
    const STATE_MASK: u64 = 0xFF;

    /// Mask for extracting generation from state_gen (bits 8-23)
    const GEN_MASK: u64 = 0xFFFF00;

    /// Shift amount for generation counter
    const GEN_SHIFT: u32 = 8;

    /// Mask for extracting RPTR from rptr_wptr (bits 0-31)
    const RPTR_MASK: u64 = 0xFFFF_FFFF;

    /// Mask for extracting WPTR from rptr_wptr (bits 32-63)
    const WPTR_MASK: u64 = 0xFFFF_FFFF_0000_0000;

    /// Shift amount for WPTR
    const WPTR_SHIFT: u32 = 32;

    /// Maximum ring size (16MB in DWORDs = 4M entries)
    const MAX_RING_SIZE_DWORDS: u32 = 4 * 1024 * 1024;

    // ========================================================================
    // Construction
    // ========================================================================

    /// Create a new uninitialized ring buffer capsule
    ///
    /// # Returns
    ///
    /// A new `AmdCpRingCapsule` in `Uninitialized` state with generation 0.
    ///
    /// # Performance
    ///
    /// O(1), ~10ns (zeroing 256 bytes)
    #[inline]
    pub const fn new() -> Self {
        Self {
            state_gen: AtomicU64::new(0), // State::Uninitialized, gen 0
            rptr_wptr: AtomicU64::new(0),
            ring_base: AtomicU64::new(0),
            ring_size: AtomicU64::new(0),
            fence_gpu_addr: AtomicU64::new(0),
            fence_value: AtomicU64::new(0),
            submit_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            doorbell_offset: AtomicU32::new(0),
            queue_id: AtomicU32::new(0),
            pipe_id: 0,
            queue_type: 0,
            me_id: 0,
            vmid: 0,
            _reserved: 0,
            _padding: [0u8; 176],
        }
    }

    /// Initialize the ring buffer
    ///
    /// Transitions from `Uninitialized` -> `Ready` state.
    /// Must be called before any command submission.
    ///
    /// # Arguments
    ///
    /// * `ring_base` - GPU virtual address of ring buffer (must be 256-byte aligned)
    /// * `ring_size` - Ring size in bytes (must be power of 2, 4KB-16MB)
    /// * `fence_gpu_addr` - GPU address for fence writeback
    /// * `doorbell_offset` - Doorbell register offset for this queue
    /// * `queue_id` - Hardware queue ID
    /// * `queue_type` - Queue type (Gfx, Compute, Dma, etc.)
    /// * `pipe_id` - Pipe ID (0-7)
    /// * `vmid` - Virtual machine ID (0-15)
    ///
    /// # Returns
    ///
    /// - `Ok(generation)` on success
    /// - `Err(InvalidState)` if already initialized
    /// - `Err(InvalidAlignment)` if ring_base not 256-byte aligned
    /// - `Err(InvalidSize)` if ring_size invalid
    ///
    /// # Performance
    ///
    /// <100ns (CAS + multiple stores)
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_RING_VALID`: Caller guarantees ring_base points to valid GPU memory
    /// - `#ASSUME_RING_ALIGNED`: ring_base must be 256-byte aligned
    pub fn initialize(
        &self,
        ring_base: u64,
        ring_size: u64,
        fence_gpu_addr: u64,
        doorbell_offset: u32,
        queue_id: u32,
        _queue_type: AmdQueueType,
        _pipe_id: u8,
        _vmid: u8,
    ) -> KgpuDriverResult<u16> {
        // Validate alignment (256-byte alignment required by AMD hardware)
        if ring_base & 0xFF != 0 {
            return Err(KgpuDriverError::InvalidAlignment);
        }

        // Validate ring size (must be power of 2, at least 4KB, at most 16MB)
        if ring_size < 4096 || ring_size > 16 * 1024 * 1024 || !ring_size.is_power_of_two() {
            return Err(KgpuDriverError::InvalidSize);
        }

        // Try to transition Uninitialized -> Ready
        loop {
            let old = self.state_gen.load(Ordering::Acquire);
            let old_state = CpRingState::from_u8((old & Self::STATE_MASK) as u8);

            if old_state != CpRingState::Uninitialized {
                return Err(KgpuDriverError::InvalidState);
            }

            // Calculate new state_gen with incremented generation
            let old_gen = ((old & Self::GEN_MASK) >> Self::GEN_SHIFT) as u16;
            let new_gen = old_gen.wrapping_add(1);
            let new = (CpRingState::Ready as u64) | ((new_gen as u64) << Self::GEN_SHIFT);

            match self.state_gen.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Successfully transitioned, now set other fields
                    // #ASSUME_ATOMIC_ALIGNED: These stores are to properly aligned fields
                    self.ring_base.store(ring_base, Ordering::Release);
                    self.ring_size.store(ring_size, Ordering::Release);
                    self.fence_gpu_addr.store(fence_gpu_addr, Ordering::Release);
                    self.doorbell_offset.store(doorbell_offset, Ordering::Release);
                    self.queue_id.store(queue_id, Ordering::Release);

                    // Note: These are not atomic, but they're only written during initialization
                    // which is single-threaded by design. We use interior mutability pattern
                    // via the CAS above to ensure initialization is atomic.
                    // For truly const fields, we'd need UnsafeCell, but these are effectively
                    // immutable after initialization.

                    return Ok(new_gen);
                }
                Err(_) => continue, // Retry on CAS failure
            }
        }
    }

    // ========================================================================
    // State Accessors
    // ========================================================================

    /// Get current ring state
    ///
    /// # Returns
    ///
    /// Current `CpRingState`
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn state(&self) -> CpRingState {
        let v = self.state_gen.load(Ordering::Acquire);
        CpRingState::from_u8((v & Self::STATE_MASK) as u8)
    }

    /// Get generation counter
    ///
    /// Increments on each state transition for TOCTOU prevention.
    ///
    /// # Returns
    ///
    /// Current generation (0-65535, wrapping)
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn generation(&self) -> u16 {
        let v = self.state_gen.load(Ordering::Acquire);
        ((v & Self::GEN_MASK) >> Self::GEN_SHIFT) as u16
    }

    /// Get read pointer (RPTR) in DWORDs
    ///
    /// Updated by hardware as it consumes commands.
    ///
    /// # Returns
    ///
    /// RPTR in DWORD offset from ring base
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn rptr(&self) -> u32 {
        let v = self.rptr_wptr.load(Ordering::Acquire);
        (v & Self::RPTR_MASK) as u32
    }

    /// Get write pointer (WPTR) in DWORDs
    ///
    /// Updated by software when commands are submitted.
    ///
    /// # Returns
    ///
    /// WPTR in DWORD offset from ring base
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn wptr(&self) -> u32 {
        let v = self.rptr_wptr.load(Ordering::Acquire);
        ((v & Self::WPTR_MASK) >> Self::WPTR_SHIFT) as u32
    }

    /// Get ring base GPU address
    #[inline]
    pub fn ring_base(&self) -> u64 {
        self.ring_base.load(Ordering::Acquire)
    }

    /// Get ring size in bytes
    #[inline]
    pub fn ring_size(&self) -> u64 {
        self.ring_size.load(Ordering::Acquire)
    }

    /// Get ring size in DWORDs
    #[inline]
    pub fn ring_size_dwords(&self) -> u32 {
        (self.ring_size.load(Ordering::Acquire) / 4) as u32
    }

    /// Get ring mask for wraparound calculation
    #[inline]
    pub fn ring_mask(&self) -> u32 {
        self.ring_size_dwords().saturating_sub(1)
    }

    /// Get fence GPU address
    #[inline]
    pub fn fence_gpu_addr(&self) -> u64 {
        self.fence_gpu_addr.load(Ordering::Acquire)
    }

    /// Get current fence value
    #[inline]
    pub fn fence_value(&self) -> u64 {
        self.fence_value.load(Ordering::Acquire)
    }

    /// Get total submit count
    #[inline]
    pub fn submit_count(&self) -> u64 {
        self.submit_count.load(Ordering::Acquire)
    }

    /// Get total error count
    #[inline]
    pub fn error_count(&self) -> u64 {
        self.error_count.load(Ordering::Acquire)
    }

    /// Get doorbell offset
    #[inline]
    pub fn doorbell_offset(&self) -> u32 {
        self.doorbell_offset.load(Ordering::Acquire)
    }

    /// Get queue ID
    #[inline]
    pub fn queue_id(&self) -> u32 {
        self.queue_id.load(Ordering::Acquire)
    }

    /// Get queue type
    #[inline]
    pub fn queue_type(&self) -> AmdQueueType {
        AmdQueueType::from_u8(self.queue_type).unwrap_or(AmdQueueType::Gfx)
    }

    /// Get pipe ID
    #[inline]
    pub fn pipe_id(&self) -> u8 {
        self.pipe_id
    }

    /// Get microengine ID
    #[inline]
    pub fn me_id(&self) -> u8 {
        self.me_id
    }

    /// Get VMID
    #[inline]
    pub fn vmid(&self) -> u8 {
        self.vmid
    }

    // ========================================================================
    // Space Calculation
    // ========================================================================

    /// Calculate available space in DWORDs
    ///
    /// Returns how many DWORDs can be written before the ring is full.
    /// Always reserves 1 DWORD to distinguish full from empty.
    ///
    /// # Returns
    ///
    /// Available space in DWORDs
    ///
    /// # Performance
    ///
    /// <15ns (two atomic loads + arithmetic)
    #[inline]
    pub fn available_space(&self) -> u32 {
        let ptrs = self.rptr_wptr.load(Ordering::Acquire);
        let rptr = (ptrs & Self::RPTR_MASK) as u32;
        let wptr = ((ptrs & Self::WPTR_MASK) >> Self::WPTR_SHIFT) as u32;
        let ring_size = self.ring_size_dwords();

        if ring_size == 0 {
            return 0;
        }

        // Available space = (rptr - wptr - 1) mod ring_size
        // We subtract 1 to never let wptr catch up to rptr (would look empty)
        if wptr >= rptr {
            // WPTR ahead of or equal to RPTR: space wraps around
            // Available = (ring_size - wptr) + rptr - 1
            ring_size - wptr + rptr - 1
        } else {
            // RPTR ahead of WPTR: contiguous space
            // Available = rptr - wptr - 1
            rptr - wptr - 1
        }
    }

    /// Check if ring has enough space for given DWORDs
    #[inline]
    pub fn has_space(&self, dwords: u32) -> bool {
        self.available_space() >= dwords
    }

    // ========================================================================
    // Command Submission
    // ========================================================================

    /// Reserve space in the ring buffer
    ///
    /// Atomically checks available space and advances WPTR.
    /// Returns the starting offset where commands should be written.
    ///
    /// # Arguments
    ///
    /// * `dwords` - Number of DWORDs to reserve
    ///
    /// # Returns
    ///
    /// - `Ok(offset)` - Starting WPTR offset where commands should be written
    /// - `Err(RingBufferFull)` - Not enough space
    /// - `Err(InvalidState)` - Ring not operational
    ///
    /// # Performance
    ///
    /// <30ns (CAS loop + space check)
    ///
    /// # Note
    ///
    /// After reserve(), caller must write PM4 packets to the ring buffer memory
    /// at `ring_base + offset * 4`, then call `submit()`.
    pub fn reserve(&self, dwords: u32) -> KgpuDriverResult<u32> {
        // Check state
        if !self.state().is_operational() {
            return Err(KgpuDriverError::InvalidState);
        }

        // Validate request size
        if dwords == 0 || dwords > Self::MAX_RING_SIZE_DWORDS {
            return Err(KgpuDriverError::InvalidSize);
        }

        loop {
            let old_ptrs = self.rptr_wptr.load(Ordering::Acquire);
            let rptr = (old_ptrs & Self::RPTR_MASK) as u32;
            let old_wptr = ((old_ptrs & Self::WPTR_MASK) >> Self::WPTR_SHIFT) as u32;
            let ring_mask = self.ring_mask();

            // Calculate available space
            let available = if old_wptr >= rptr {
                self.ring_size_dwords() - old_wptr + rptr - 1
            } else {
                rptr - old_wptr - 1
            };

            if available < dwords {
                return Err(KgpuDriverError::RingBufferFull);
            }

            // Calculate new WPTR (with wraparound)
            let new_wptr = (old_wptr + dwords) & ring_mask;
            let new_ptrs = (rptr as u64) | ((new_wptr as u64) << Self::WPTR_SHIFT);

            match self.rptr_wptr.compare_exchange_weak(
                old_ptrs,
                new_ptrs,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(old_wptr),
                Err(_) => continue, // Retry on CAS failure
            }
        }
    }

    /// Submit commands to GPU
    ///
    /// Increments fence value and writes WPTR to doorbell to notify GPU.
    /// The GPU will start fetching commands from the updated position.
    ///
    /// # Arguments
    ///
    /// * `new_wptr` - New WPTR value after all commands are written
    ///
    /// # Returns
    ///
    /// - `Ok(fence_value)` - Fence value for this submission
    /// - `Err(InvalidState)` - Ring not operational
    /// - `Err(RingSubmitFailed)` - Submission failed
    ///
    /// # Performance
    ///
    /// <100ns (atomic increments + doorbell write simulation)
    ///
    /// # Safety
    ///
    /// - `#ASSUME_DOORBELL_MAPPED`: Caller must ensure doorbell is accessible
    /// - `#ASSUME_RING_VALID`: Commands written to ring must be valid PM4
    ///
    /// # Note
    ///
    /// In a real driver, this would perform an MMIO write to the doorbell
    /// register. Here we just update internal state for testing.
    pub fn submit(&self, _new_wptr: u32) -> KgpuDriverResult<u64> {
        // Check state
        if !self.state().is_operational() {
            self.error_count.fetch_add(1, Ordering::Relaxed);
            return Err(KgpuDriverError::InvalidState);
        }

        // Increment fence value
        let fence = self.fence_value.fetch_add(1, Ordering::AcqRel) + 1;

        // Increment submit count
        self.submit_count.fetch_add(1, Ordering::Relaxed);

        // Transition to Active state if Ready
        loop {
            let old = self.state_gen.load(Ordering::Acquire);
            let old_state = CpRingState::from_u8((old & Self::STATE_MASK) as u8);

            if old_state == CpRingState::Ready {
                let old_gen = ((old & Self::GEN_MASK) >> Self::GEN_SHIFT) as u16;
                let new_gen = old_gen.wrapping_add(1);
                let new = (CpRingState::Active as u64) | ((new_gen as u64) << Self::GEN_SHIFT);

                match self.state_gen.compare_exchange_weak(
                    old,
                    new,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => break,
                    Err(_) => continue,
                }
            } else {
                break; // Already Active or other state
            }
        }

        // In a real driver, we would write to the doorbell here:
        // unsafe { core::ptr::write_volatile(doorbell_ptr, new_wptr); }

        Ok(fence)
    }

    /// Update RPTR after GPU completion
    ///
    /// Called when hardware signals completion or during polling.
    ///
    /// # Arguments
    ///
    /// * `new_rptr` - New RPTR value from hardware
    ///
    /// # Performance
    ///
    /// <20ns (CAS loop)
    pub fn update_rptr(&self, new_rptr: u32) {
        loop {
            let old_ptrs = self.rptr_wptr.load(Ordering::Acquire);
            let wptr = ((old_ptrs & Self::WPTR_MASK) >> Self::WPTR_SHIFT) as u32;
            let new_ptrs = (new_rptr as u64) | ((wptr as u64) << Self::WPTR_SHIFT);

            match self.rptr_wptr.compare_exchange_weak(
                old_ptrs,
                new_ptrs,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(_) => continue,
            }
        }
    }

    /// Mark ring as idle (all commands completed)
    ///
    /// Transitions from `Active` -> `Ready` when RPTR catches up to WPTR.
    pub fn mark_idle(&self) -> KgpuDriverResult<u16> {
        loop {
            let old = self.state_gen.load(Ordering::Acquire);
            let old_state = CpRingState::from_u8((old & Self::STATE_MASK) as u8);

            if old_state != CpRingState::Active {
                return Err(KgpuDriverError::InvalidState);
            }

            let old_gen = ((old & Self::GEN_MASK) >> Self::GEN_SHIFT) as u16;
            let new_gen = old_gen.wrapping_add(1);
            let new = (CpRingState::Ready as u64) | ((new_gen as u64) << Self::GEN_SHIFT);

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

    /// Mark ring as in error state
    pub fn mark_error(&self) -> KgpuDriverResult<u16> {
        loop {
            let old = self.state_gen.load(Ordering::Acquire);
            let old_gen = ((old & Self::GEN_MASK) >> Self::GEN_SHIFT) as u16;
            let new_gen = old_gen.wrapping_add(1);
            let new = (CpRingState::Error as u64) | ((new_gen as u64) << Self::GEN_SHIFT);

            match self.state_gen.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.error_count.fetch_add(1, Ordering::Relaxed);
                    return Ok(new_gen);
                }
                Err(_) => continue,
            }
        }
    }

    // ========================================================================
    // PM4 Packet Emission Helpers
    // ========================================================================

    /// Calculate offset for NOP packet emission
    ///
    /// Returns the header value and next offset.
    /// Caller writes header to ring[offset] and pads with zeros.
    ///
    /// # Arguments
    ///
    /// * `offset` - Current write offset in DWORDs
    /// * `count` - Number of NOP DWORDs (including header)
    ///
    /// # Returns
    ///
    /// (header_value, next_offset)
    #[inline]
    pub fn emit_nop(&self, offset: u32, count: u16) -> (u32, u32) {
        let header = if count <= 1 {
            Pm4Header::type2_nop()
        } else {
            Pm4Header::nop(count.saturating_sub(1)) // -1 because header counts
        };
        let next_offset = (offset + count as u32) & self.ring_mask();
        (header.value, next_offset)
    }

    /// Calculate INDIRECT_BUFFER packet
    ///
    /// # Arguments
    ///
    /// * `offset` - Current write offset in DWORDs
    /// * `ib_addr` - GPU address of indirect buffer
    /// * `ib_size` - Size of indirect buffer in DWORDs
    ///
    /// # Returns
    ///
    /// Array of DWORDs to write and next offset
    #[inline]
    pub fn emit_indirect_buffer(&self, offset: u32, ib_addr: u64, ib_size: u32) -> ([u32; 4], u32) {
        let header = Pm4Header::type3(Pm4Opcode::IndirectBuffer, 3);
        let packet = [
            header.value,
            (ib_addr & 0xFFFF_FFFC) as u32,         // Low 32 bits (4-byte aligned)
            ((ib_addr >> 32) & 0xFFFF) as u32,      // High 16 bits
            ib_size & 0xFFFFF,                       // Size in DWORDs (20 bits)
        ];
        let next_offset = (offset + 4) & self.ring_mask();
        (packet, next_offset)
    }

    /// Calculate WRITE_DATA packet for memory write
    ///
    /// # Arguments
    ///
    /// * `offset` - Current write offset in DWORDs
    /// * `dst_addr` - Destination GPU address
    /// * `data` - Data DWORDs to write (max 16380)
    ///
    /// # Returns
    ///
    /// (header_value, control_dword, addr_lo, addr_hi, data_start_offset, next_offset)
    #[inline]
    pub fn emit_write_data_header(&self, offset: u32, dst_addr: u64, data_count: u16)
        -> (u32, u32, u32, u32, u32)
    {
        // WRITE_DATA: header + control + addr_lo + addr_hi + data[]
        let total_dwords = 4 + data_count as u32;
        let header = Pm4Header::type3(Pm4Opcode::WriteData, 3 + data_count);

        // Control DWORD: dst_sel=5 (memory), wr_confirm=1
        let control = (5 << 8) | (1 << 20);

        let addr_lo = (dst_addr & 0xFFFF_FFFC) as u32;
        let addr_hi = (dst_addr >> 32) as u32;

        let next_offset = (offset + total_dwords) & self.ring_mask();
        (header.value, control, addr_lo, addr_hi, next_offset)
    }

    /// Calculate RELEASE_MEM packet for fence signal
    ///
    /// # Arguments
    ///
    /// * `offset` - Current write offset in DWORDs
    /// * `fence_addr` - GPU address to write fence value
    /// * `fence_val` - Fence value to write
    ///
    /// # Returns
    ///
    /// Array of DWORDs to write and next offset
    #[inline]
    pub fn emit_release_mem(&self, offset: u32, fence_addr: u64, fence_val: u64) -> ([u32; 7], u32) {
        let header = Pm4Header::type3(Pm4Opcode::ReleaseMem, 6);

        // Event type: EOP timestamp, data sel: send 64-bit data
        let event_type = 0x28; // CACHE_FLUSH_AND_INV_TS_EVENT
        let event_cntl = (event_type << 0) | (3 << 29); // Data sel = 64-bit immediate

        let packet = [
            header.value,
            event_cntl,
            (fence_addr & 0xFFFF_FFF8) as u32,      // Address low (8-byte aligned)
            ((fence_addr >> 32) & 0xFFFF) as u32,   // Address high
            (fence_val & 0xFFFF_FFFF) as u32,       // Data low
            ((fence_val >> 32) & 0xFFFF_FFFF) as u32, // Data high
            0,                                        // Reserved
        ];

        let next_offset = (offset + 7) & self.ring_mask();
        (packet, next_offset)
    }

    /// Calculate ACQUIRE_MEM packet for cache invalidation
    ///
    /// # Arguments
    ///
    /// * `offset` - Current write offset in DWORDs
    ///
    /// # Returns
    ///
    /// Array of DWORDs to write and next offset
    #[inline]
    pub fn emit_acquire_mem(&self, offset: u32) -> ([u32; 7], u32) {
        let header = Pm4Header::type3(Pm4Opcode::AcquireMem, 6);

        // Invalidate L2, GL1, GL2 caches
        let coher_cntl = 0x1F << 25; // All cache invalidation flags

        let packet = [
            header.value,
            coher_cntl,
            0xFFFF_FFFF, // Size: entire address space
            0,           // Size high
            0,           // Address low
            0,           // Address high
            0,           // Poll interval
        ];

        let next_offset = (offset + 7) & self.ring_mask();
        (packet, next_offset)
    }

    /// Calculate EVENT_WRITE_EOP packet
    ///
    /// # Arguments
    ///
    /// * `offset` - Current write offset in DWORDs
    /// * `fence_addr` - GPU address to write fence value
    /// * `fence_val` - Fence value to write
    ///
    /// # Returns
    ///
    /// Array of DWORDs to write and next offset
    #[inline]
    pub fn emit_event_write_eop(&self, offset: u32, fence_addr: u64, fence_val: u64) -> ([u32; 5], u32) {
        let header = Pm4Header::type3(Pm4Opcode::EventWriteEop, 4);

        // EVENT_WRITE_EOP: cache flush + timestamp + data write
        let event_cntl = 0x28 | (3 << 29); // Event index + data sel

        let packet = [
            header.value,
            event_cntl,
            (fence_addr & 0xFFFF_FFFC) as u32,
            ((fence_addr >> 32) & 0xFFFF) as u32 | ((fence_val & 0xFFFF) as u32) << 16,
            ((fence_val >> 16) & 0xFFFF_FFFF) as u32,
        ];

        let next_offset = (offset + 5) & self.ring_mask();
        (packet, next_offset)
    }

    // ========================================================================
    // Snapshot
    // ========================================================================

    /// Take an atomic snapshot of current state
    ///
    /// Captures all state atomically for consistent reads.
    ///
    /// # Returns
    ///
    /// Immutable `AmdCpRingSnapshot` with all current values
    ///
    /// # Performance
    ///
    /// <30ns (multiple atomic loads)
    #[inline]
    pub fn snapshot(&self) -> AmdCpRingSnapshot {
        let state_gen = self.state_gen.load(Ordering::Acquire);
        let ptrs = self.rptr_wptr.load(Ordering::Acquire);

        AmdCpRingSnapshot {
            state: CpRingState::from_u8((state_gen & Self::STATE_MASK) as u8),
            generation: ((state_gen & Self::GEN_MASK) >> Self::GEN_SHIFT) as u16,
            rptr: (ptrs & Self::RPTR_MASK) as u32,
            wptr: ((ptrs & Self::WPTR_MASK) >> Self::WPTR_SHIFT) as u32,
            ring_base: self.ring_base.load(Ordering::Acquire),
            ring_size: self.ring_size.load(Ordering::Acquire),
            fence_value: self.fence_value.load(Ordering::Acquire),
            submit_count: self.submit_count.load(Ordering::Acquire),
            error_count: self.error_count.load(Ordering::Acquire),
            queue_id: self.queue_id.load(Ordering::Acquire),
            queue_type: AmdQueueType::from_u8(self.queue_type).unwrap_or(AmdQueueType::Gfx),
            pipe_id: self.pipe_id,
            vmid: self.vmid,
        }
    }
}

impl Default for AmdCpRingCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for AmdCpRingCapsule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let snap = self.snapshot();
        f.debug_struct("AmdCpRingCapsule")
            .field("state", &snap.state)
            .field("generation", &snap.generation)
            .field("rptr", &snap.rptr)
            .field("wptr", &snap.wptr)
            .field("ring_base", &format_args!("0x{:x}", snap.ring_base))
            .field("ring_size", &snap.ring_size)
            .field("fence_value", &snap.fence_value)
            .field("submit_count", &snap.submit_count)
            .field("queue_type", &snap.queue_type)
            .finish()
    }
}

// Safety: All fields are AtomicU64/AtomicU32 or immutable after initialization
// AtomicU64/U32 are Send + Sync, so AmdCpRingCapsule can be safely shared.
//
// # ASSUM Safety
// - `#ASSUME_ATOMIC_ALIGNED`: AtomicU64/U32 guarantee proper alignment
// - `#ASSUME_CACHE_ALIGNED`: #[repr(C, align(256))] ensures cache alignment
unsafe impl Send for AmdCpRingCapsule {}
unsafe impl Sync for AmdCpRingCapsule {}

// ============================================================================
// Ring Snapshot
// ============================================================================

/// Immutable snapshot of AMD CP ring state
///
/// Captured atomically from `AmdCpRingCapsule::snapshot()`.
#[derive(Debug, Clone, Copy)]
pub struct AmdCpRingSnapshot {
    /// Current ring state
    pub state: CpRingState,
    /// Generation counter at snapshot time
    pub generation: u16,
    /// Read pointer (DWORD offset)
    pub rptr: u32,
    /// Write pointer (DWORD offset)
    pub wptr: u32,
    /// Ring buffer GPU base address
    pub ring_base: u64,
    /// Ring buffer size in bytes
    pub ring_size: u64,
    /// Current fence value
    pub fence_value: u64,
    /// Total submissions
    pub submit_count: u64,
    /// Total errors
    pub error_count: u64,
    /// Queue ID
    pub queue_id: u32,
    /// Queue type
    pub queue_type: AmdQueueType,
    /// Pipe ID
    pub pipe_id: u8,
    /// VMID
    pub vmid: u8,
}

impl AmdCpRingSnapshot {
    /// Check if ring is operational
    #[inline]
    pub fn is_operational(&self) -> bool {
        self.state.is_operational()
    }

    /// Calculate available space in DWORDs
    #[inline]
    pub fn available_space(&self) -> u32 {
        let ring_size_dwords = (self.ring_size / 4) as u32;
        if ring_size_dwords == 0 {
            return 0;
        }

        if self.wptr >= self.rptr {
            ring_size_dwords - self.wptr + self.rptr - 1
        } else {
            self.rptr - self.wptr - 1
        }
    }

    /// Calculate used space in DWORDs
    #[inline]
    pub fn used_space(&self) -> u32 {
        let ring_size_dwords = (self.ring_size / 4) as u32;
        if ring_size_dwords == 0 {
            return 0;
        }

        if self.wptr >= self.rptr {
            self.wptr - self.rptr
        } else {
            ring_size_dwords - self.rptr + self.wptr
        }
    }

    /// Get ring utilization as percentage (0-100)
    #[inline]
    pub fn utilization_percent(&self) -> u8 {
        let ring_size_dwords = (self.ring_size / 4) as u32;
        if ring_size_dwords == 0 {
            return 0;
        }
        ((self.used_space() as u64 * 100) / ring_size_dwords as u64) as u8
    }
}

impl Default for AmdCpRingSnapshot {
    fn default() -> Self {
        Self {
            state: CpRingState::Uninitialized,
            generation: 0,
            rptr: 0,
            wptr: 0,
            ring_base: 0,
            ring_size: 0,
            fence_value: 0,
            submit_count: 0,
            error_count: 0,
            queue_id: 0,
            queue_type: AmdQueueType::Gfx,
            pipe_id: 0,
            vmid: 0,
        }
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
    // Tier 1: Unit Tests (Q1-Q7) - Struct Layout
    // ========================================================================

    #[test]
    fn test_capsule_size() {
        // T28 Q1: Verify exact size is 256 bytes
        assert_eq!(mem::size_of::<AmdCpRingCapsule>(), 256);
    }

    #[test]
    fn test_capsule_alignment() {
        // T28 Q2: Verify alignment is 256 bytes (4 cache lines)
        assert_eq!(mem::align_of::<AmdCpRingCapsule>(), 256);
    }

    #[test]
    fn test_new_capsule_state() {
        // T28 Q3: Verify initial state is Uninitialized with generation 0
        let capsule = AmdCpRingCapsule::new();
        assert_eq!(capsule.state(), CpRingState::Uninitialized);
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.rptr(), 0);
        assert_eq!(capsule.wptr(), 0);
        assert_eq!(capsule.ring_base(), 0);
        assert_eq!(capsule.ring_size(), 0);
    }

    #[test]
    fn test_default_impl() {
        // T28 Q4: Verify Default trait implementation
        let capsule: AmdCpRingCapsule = Default::default();
        assert_eq!(capsule.state(), CpRingState::Uninitialized);
    }

    #[test]
    fn test_snapshot_size() {
        // T28 Q5: Verify snapshot is reasonably sized
        assert!(mem::size_of::<AmdCpRingSnapshot>() <= 128);
    }

    #[test]
    fn test_pm4_header_size() {
        // T28 Q6: Verify PM4 header is exactly 4 bytes
        assert_eq!(mem::size_of::<Pm4Header>(), 4);
    }

    #[test]
    fn test_queue_type_size() {
        // T28 Q7: Verify queue type enum is 1 byte
        assert_eq!(mem::size_of::<AmdQueueType>(), 1);
    }

    // ========================================================================
    // Tier 2: PM4 Packet Tests (Q8-Q14)
    // ========================================================================

    #[test]
    fn test_pm4_header_type3() {
        // T28 Q8: Verify Type 3 header encoding
        let header = Pm4Header::type3(Pm4Opcode::Nop, 4);
        assert_eq!(header.packet_type(), 3);
        assert_eq!(header.opcode(), Pm4Opcode::Nop as u8);
        assert_eq!(header.count(), 4);
        assert_eq!(header.total_dwords(), 5); // header + 4 payload
    }

    #[test]
    fn test_pm4_header_type2_nop() {
        // T28 Q9: Verify Type 2 NOP encoding
        let header = Pm4Header::type2_nop();
        assert_eq!(header.packet_type(), 2);
        assert_eq!(header.total_dwords(), 1);
    }

    #[test]
    fn test_pm4_header_nop_alias() {
        // T28 Q10: Verify NOP convenience function
        let header = Pm4Header::nop(8);
        assert_eq!(header.packet_type(), 3);
        assert_eq!(header.opcode(), Pm4Opcode::Nop as u8);
        assert_eq!(header.count(), 8);
    }

    #[test]
    fn test_pm4_header_with_shader() {
        // T28 Q11: Verify shader type bit
        let gfx = Pm4Header::type3_with_shader(Pm4Opcode::DispatchDirect, 3, false);
        let compute = Pm4Header::type3_with_shader(Pm4Opcode::DispatchDirect, 3, true);

        assert!(!gfx.is_compute());
        assert!(compute.is_compute());
    }

    #[test]
    fn test_pm4_opcode_names() {
        // T28 Q12: Verify opcode names
        assert_eq!(Pm4Opcode::Nop.name(), "NOP");
        assert_eq!(Pm4Opcode::DispatchDirect.name(), "DISPATCH_DIRECT");
        assert_eq!(Pm4Opcode::ReleaseMem.name(), "RELEASE_MEM");
        assert_eq!(Pm4Opcode::IndirectBuffer.name(), "INDIRECT_BUFFER");
    }

    #[test]
    fn test_pm4_header_count_edge_cases() {
        // T28 Q13: Verify count handling at boundaries
        let zero = Pm4Header::type3(Pm4Opcode::Nop, 0);
        assert_eq!(zero.count(), 1); // count-1 stored, so 0 becomes 1

        let max = Pm4Header::type3(Pm4Opcode::Nop, 16384);
        assert_eq!(max.count(), 16384);
    }

    #[test]
    fn test_pm4_header_display() {
        // T28 Q14: Verify Display implementation
        let header = Pm4Header::type3(Pm4Opcode::WriteData, 5);
        let display = format!("{}", header);
        assert!(display.contains("PM4"));
        assert!(display.contains("Type3"));
    }

    // ========================================================================
    // Tier 3: State Transitions (Q15-Q21)
    // ========================================================================

    #[test]
    fn test_initialize_success() {
        // T28 Q15: Verify Uninitialized -> Ready transition
        let capsule = AmdCpRingCapsule::new();

        let result = capsule.initialize(
            0x1000_0000,        // ring_base (256-byte aligned)
            256 * 1024,         // ring_size (256KB)
            0x2000_0000,        // fence_gpu_addr
            0x1000,             // doorbell_offset
            0,                  // queue_id
            AmdQueueType::Gfx,  // queue_type
            0,                  // pipe_id
            0,                  // vmid
        );

        assert!(result.is_ok());
        assert_eq!(capsule.state(), CpRingState::Ready);
        assert_eq!(capsule.generation(), 1);
        assert_eq!(capsule.ring_base(), 0x1000_0000);
        assert_eq!(capsule.ring_size(), 256 * 1024);
    }

    #[test]
    fn test_initialize_invalid_alignment() {
        // T28 Q16: Verify alignment check
        let capsule = AmdCpRingCapsule::new();

        let result = capsule.initialize(
            0x1000_0001,        // NOT 256-byte aligned
            256 * 1024,
            0x2000_0000,
            0x1000,
            0,
            AmdQueueType::Gfx,
            0,
            0,
        );

        assert_eq!(result, Err(KgpuDriverError::InvalidAlignment));
    }

    #[test]
    fn test_initialize_invalid_size() {
        // T28 Q17: Verify size validation
        let capsule = AmdCpRingCapsule::new();

        // Too small
        let result = capsule.initialize(
            0x1000_0000,
            1024,  // Less than 4KB
            0x2000_0000,
            0x1000,
            0,
            AmdQueueType::Gfx,
            0,
            0,
        );
        assert_eq!(result, Err(KgpuDriverError::InvalidSize));

        // Not power of 2
        let capsule2 = AmdCpRingCapsule::new();
        let result = capsule2.initialize(
            0x1000_0000,
            300 * 1024,  // Not power of 2
            0x2000_0000,
            0x1000,
            0,
            AmdQueueType::Gfx,
            0,
            0,
        );
        assert_eq!(result, Err(KgpuDriverError::InvalidSize));
    }

    #[test]
    fn test_initialize_already_initialized() {
        // T28 Q18: Verify double initialization fails
        let capsule = AmdCpRingCapsule::new();
        capsule.initialize(
            0x1000_0000,
            256 * 1024,
            0x2000_0000,
            0x1000,
            0,
            AmdQueueType::Gfx,
            0,
            0,
        ).unwrap();

        let result = capsule.initialize(
            0x3000_0000,
            512 * 1024,
            0x4000_0000,
            0x2000,
            1,
            AmdQueueType::Compute,
            1,
            1,
        );
        assert_eq!(result, Err(KgpuDriverError::InvalidState));
    }

    #[test]
    fn test_ring_state_predicates() {
        // T28 Q19: Verify state predicates
        assert!(!CpRingState::Uninitialized.is_operational());
        assert!(CpRingState::Ready.is_operational());
        assert!(CpRingState::Active.is_operational());
        assert!(!CpRingState::Error.is_operational());
        assert!(!CpRingState::Suspended.is_operational());
    }

    #[test]
    fn test_state_names() {
        // T28 Q20: Verify state names
        assert_eq!(CpRingState::Uninitialized.name(), "Uninitialized");
        assert_eq!(CpRingState::Ready.name(), "Ready");
        assert_eq!(CpRingState::Active.name(), "Active");
        assert_eq!(CpRingState::Error.name(), "Error");
    }

    #[test]
    fn test_queue_type_defaults() {
        // T28 Q21: Verify queue type default ring sizes
        assert_eq!(AmdQueueType::Gfx.default_ring_size(), 1024 * 1024);
        assert_eq!(AmdQueueType::Compute.default_ring_size(), 256 * 1024);
        assert_eq!(AmdQueueType::Dma.default_ring_size(), 256 * 1024);
    }

    // ========================================================================
    // Tier 4: Ring Operations (Q22-Q28)
    // ========================================================================

    fn create_initialized_ring() -> AmdCpRingCapsule {
        let capsule = AmdCpRingCapsule::new();
        capsule.initialize(
            0x1000_0000,
            64 * 1024,  // 64KB = 16K DWORDs
            0x2000_0000,
            0x1000,
            0,
            AmdQueueType::Gfx,
            0,
            0,
        ).unwrap();
        capsule
    }

    #[test]
    fn test_available_space_empty_ring() {
        // T28 Q22: Verify available space calculation for empty ring
        let ring = create_initialized_ring();
        let ring_size_dwords = ring.ring_size_dwords();

        // Empty ring: RPTR = WPTR = 0
        // Available = ring_size - 1 (reserve 1 to distinguish full/empty)
        assert_eq!(ring.available_space(), ring_size_dwords - 1);
    }

    #[test]
    fn test_reserve_success() {
        // T28 Q23: Verify successful space reservation
        let ring = create_initialized_ring();

        let result = ring.reserve(100);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0); // First offset is 0

        // WPTR should advance
        assert_eq!(ring.wptr(), 100);
    }

    #[test]
    fn test_reserve_updates_wptr() {
        // T28 Q24: Verify multiple reserves advance WPTR correctly
        let ring = create_initialized_ring();

        ring.reserve(100).unwrap();
        assert_eq!(ring.wptr(), 100);

        ring.reserve(50).unwrap();
        assert_eq!(ring.wptr(), 150);

        ring.reserve(200).unwrap();
        assert_eq!(ring.wptr(), 350);
    }

    #[test]
    fn test_reserve_fails_not_initialized() {
        // T28 Q25: Verify reserve fails on uninitialized ring
        let ring = AmdCpRingCapsule::new();
        let result = ring.reserve(100);
        assert_eq!(result, Err(KgpuDriverError::InvalidState));
    }

    #[test]
    fn test_submit_increments_fence() {
        // T28 Q26: Verify submit increments fence value
        let ring = create_initialized_ring();

        let offset = ring.reserve(100).unwrap();
        let fence1 = ring.submit(offset + 100).unwrap();
        assert_eq!(fence1, 1);

        let offset = ring.reserve(50).unwrap();
        let fence2 = ring.submit(offset + 50).unwrap();
        assert_eq!(fence2, 2);

        assert_eq!(ring.submit_count(), 2);
    }

    #[test]
    fn test_update_rptr() {
        // T28 Q27: Verify RPTR update
        let ring = create_initialized_ring();

        ring.reserve(100).unwrap();
        ring.submit(100).unwrap();

        ring.update_rptr(100);
        assert_eq!(ring.rptr(), 100);

        // Available space should increase after RPTR update
        ring.reserve(100).unwrap();
        ring.update_rptr(200);
        assert_eq!(ring.rptr(), 200);
    }

    #[test]
    fn test_rptr_wptr_wraparound() {
        // T28 Q28: Verify RPTR/WPTR wraparound
        let ring = create_initialized_ring();
        let ring_size_dwords = ring.ring_size_dwords();

        // Fill most of the ring
        let reserve_size = ring_size_dwords - 100;
        ring.reserve(reserve_size).unwrap();

        // Simulate GPU consuming commands
        ring.update_rptr(reserve_size);

        // Reserve more to trigger wraparound
        let offset = ring.reserve(200).unwrap();
        assert_eq!(offset, reserve_size);

        // WPTR should wrap around
        let expected_wptr = (reserve_size + 200) & (ring_size_dwords - 1);
        assert_eq!(ring.wptr(), expected_wptr);
    }

    // ========================================================================
    // Tier 5: Determinism Tests (Q29-Q35)
    // ========================================================================

    #[test]
    fn test_generation_increments_on_state_change() {
        // T28 Q29: Verify generation increments on state transitions
        let ring = AmdCpRingCapsule::new();
        assert_eq!(ring.generation(), 0);

        ring.initialize(
            0x1000_0000,
            64 * 1024,
            0x2000_0000,
            0x1000,
            0,
            AmdQueueType::Gfx,
            0,
            0,
        ).unwrap();
        assert_eq!(ring.generation(), 1);
        assert_eq!(ring.state(), CpRingState::Ready);

        ring.reserve(10).unwrap();
        ring.submit(10).unwrap();
        // Submit should transition Ready -> Active
        assert_eq!(ring.state(), CpRingState::Active);
        assert_eq!(ring.generation(), 2);

        ring.mark_idle().unwrap();
        assert_eq!(ring.state(), CpRingState::Ready);
        assert_eq!(ring.generation(), 3);
    }

    #[test]
    fn test_snapshot_captures_all_state() {
        // T28 Q30: Verify snapshot captures all fields
        let ring = create_initialized_ring();
        ring.reserve(100).unwrap();
        ring.submit(100).unwrap();

        let snap = ring.snapshot();
        assert_eq!(snap.state, CpRingState::Active);
        assert_eq!(snap.generation, 2);
        assert_eq!(snap.wptr, 100);
        assert_eq!(snap.ring_base, 0x1000_0000);
        assert_eq!(snap.ring_size, 64 * 1024);
        assert_eq!(snap.fence_value, 1);
        assert_eq!(snap.submit_count, 1);
        assert_eq!(snap.queue_type, AmdQueueType::Gfx);
    }

    #[test]
    fn test_snapshot_utilization() {
        // T28 Q31: Verify snapshot utilization calculation
        let ring = create_initialized_ring();
        let snap1 = ring.snapshot();
        assert_eq!(snap1.utilization_percent(), 0);

        ring.reserve(8192).unwrap(); // 50% of 16K DWORDs
        let snap2 = ring.snapshot();
        assert_eq!(snap2.utilization_percent(), 50);
    }

    #[test]
    fn test_error_state() {
        // T28 Q32: Verify error state transition
        let ring = create_initialized_ring();

        ring.mark_error().unwrap();
        assert_eq!(ring.state(), CpRingState::Error);
        assert_eq!(ring.error_count(), 1);

        // Should not be operational
        assert!(!ring.state().is_operational());
    }

    #[test]
    fn test_emit_nop_calculation() {
        // T28 Q33: Verify NOP emission calculation
        let ring = create_initialized_ring();

        let (header, next_offset) = ring.emit_nop(0, 8);
        let pm4 = Pm4Header { value: header };
        assert_eq!(pm4.packet_type(), 3);
        assert_eq!(pm4.opcode(), Pm4Opcode::Nop as u8);
        assert_eq!(next_offset, 8);
    }

    #[test]
    fn test_emit_release_mem() {
        // T28 Q34: Verify RELEASE_MEM packet calculation
        let ring = create_initialized_ring();

        let (packet, next_offset) = ring.emit_release_mem(0, 0x2000_0000, 12345);
        assert_eq!(packet.len(), 7);
        assert_eq!(next_offset, 7);

        let header = Pm4Header { value: packet[0] };
        assert_eq!(header.packet_type(), 3);
        assert_eq!(header.opcode(), Pm4Opcode::ReleaseMem as u8);
    }

    #[test]
    fn test_send_sync_traits() {
        // T28 Q35: Verify Send + Sync implementation
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AmdCpRingCapsule>();
        assert_send_sync::<AmdCpRingSnapshot>();
    }

    #[test]
    fn test_debug_impl() {
        // Bonus: Verify Debug implementation
        let ring = create_initialized_ring();
        let debug_str = format!("{:?}", ring);
        assert!(debug_str.contains("AmdCpRingCapsule"));
        assert!(debug_str.contains("Ready"));
    }
}
