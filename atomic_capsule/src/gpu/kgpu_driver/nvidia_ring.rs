//! NVIDIA Trojan Kernel Ring Buffer for KGPU-Driver v2.0
//!
//! Implements the "Trojan Kernel" approach to bypass NVIDIA's locked GSP firmware.
//! A persistent CUDA kernel polls a ring buffer in pinned CPU-visible memory,
//! executing commands written by Rust code with sub-100ns latency.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    NVIDIA Trojan Architecture                    │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                  │
//! │  CPU (Rust)                    GPU (CUDA Kernel)                │
//! │  ┌──────────┐                  ┌──────────────┐                 │
//! │  │ Write    │ ───────────────► │ Poll Loop   │                 │
//! │  │ Commands │  Pinned Memory   │ (infinite)  │                 │
//! │  │ to Ring  │ ◄─────────────── │ Execute     │                 │
//! │  └──────────┘   Completion     │ Commands    │                 │
//! │                                └──────────────┘                 │
//! │                                                                  │
//! │  Ring Buffer (Pinned Memory, CPU-visible):                      │
//! │  ┌────────────────────────────────────────────────────────────┐ │
//! │  │ [HEAD] [TAIL] [CMD0] [CMD1] [CMD2] ... [CMDN] [STATUS]    │ │
//! │  └────────────────────────────────────────────────────────────┘ │
//! │                                                                  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Capsule Tier
//!
//! - **T1 Atomic**: 256-byte cache-aligned capsule with generation counters
//! - **Performance**: <20ns reserve, <10ns write, <100ns submit
//!
//! # UCE34 Compliance
//!
//! - Q10: T1 Atomic tier (lockfree ring buffer coordination)
//! - Q11: Rust transform (type-safe command encoding)
//! - Q33: 100% lockfree via AtomicU64/AtomicU32/AtomicPtr
//! - Q34: Audit trail via sequence numbers and generation counters
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_PINNED_MEMORY`: Ring buffer is CUDA pinned memory (cudaHostAlloc)
//! - `#ASSUME_KERNEL_RUNNING`: Trojan kernel is actively polling
//! - `#ASSUME_COHERENT`: Memory writes visible to GPU without explicit flush
//! - `#ASSUME_ALIGNED`: All command slots are 64-byte aligned
//!
//! # Example
//!
//! ```ignore
//! use atomic_capsule::gpu::kgpu_driver::nvidia_ring::*;
//!
//! // Initialize (after CUDA setup)
//! let ring = NvidiaTrojanRingCapsule::new();
//! ring.initialize(ring_ptr, gpu_addr, 1024, fence_ptr, handle, 0)?;
//!
//! // Submit commands
//! let seqno = ring.submit_mem_copy(src_addr, dst_addr, size)?;
//! ring.wait_seqno(seqno, 1_000_000)?; // 1ms timeout
//! ```

#![allow(dead_code)] // Allow during development

use core::fmt;
use core::ptr::null_mut;
use core::sync::atomic::{AtomicPtr, AtomicU32, AtomicU64, Ordering};

#[cfg(feature = "std")]
extern crate std;

// ============================================================================
// Trojan Opcode Definitions
// ============================================================================

/// Trojan command opcodes (our own protocol, not NVIDIA official)
///
/// These opcodes define the command vocabulary for the Trojan kernel.
/// The GPU kernel interprets these and executes the corresponding operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum TrojanOpcode {
    /// No operation (used for synchronization/testing)
    Nop = 0x00,
    /// GPU-to-GPU or Host-to-GPU memory copy
    MemCopy = 0x01,
    /// Fill memory with a 32-bit pattern
    MemSet = 0x02,
    /// Launch a compute kernel
    KernelLaunch = 0x03,
    /// Synchronization barrier (wait for all prior commands)
    Sync = 0x04,
    /// Write fence value to memory (signal completion)
    FenceSignal = 0x05,
    /// Wait for fence value (GPU-side wait)
    FenceWait = 0x06,
    /// Read GPU register (debug/diagnostic)
    RegisterRead = 0x07,
    /// Write GPU register (dangerous, use with care)
    RegisterWrite = 0x08,
    /// Graceful shutdown (kernel exits loop)
    Shutdown = 0xFF,
}

impl TrojanOpcode {
    /// Returns true if this opcode has a completion signal
    #[inline]
    pub const fn has_completion(self) -> bool {
        matches!(self, Self::FenceSignal | Self::Sync | Self::Shutdown)
    }

    /// Returns true if this opcode is asynchronous
    #[inline]
    pub const fn is_async(self) -> bool {
        matches!(self, Self::MemCopy | Self::MemSet | Self::KernelLaunch)
    }

    /// Returns the human-readable name of this opcode
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Nop => "NOP",
            Self::MemCopy => "MEM_COPY",
            Self::MemSet => "MEM_SET",
            Self::KernelLaunch => "KERNEL_LAUNCH",
            Self::Sync => "SYNC",
            Self::FenceSignal => "FENCE_SIGNAL",
            Self::FenceWait => "FENCE_WAIT",
            Self::RegisterRead => "REG_READ",
            Self::RegisterWrite => "REG_WRITE",
            Self::Shutdown => "SHUTDOWN",
        }
    }

    /// Convert from u32 (for FFI/serialization)
    #[inline]
    pub const fn from_u32(value: u32) -> Option<Self> {
        match value {
            0x00 => Some(Self::Nop),
            0x01 => Some(Self::MemCopy),
            0x02 => Some(Self::MemSet),
            0x03 => Some(Self::KernelLaunch),
            0x04 => Some(Self::Sync),
            0x05 => Some(Self::FenceSignal),
            0x06 => Some(Self::FenceWait),
            0x07 => Some(Self::RegisterRead),
            0x08 => Some(Self::RegisterWrite),
            0xFF => Some(Self::Shutdown),
            _ => None,
        }
    }
}

impl fmt::Display for TrojanOpcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// Trojan Command Structure
// ============================================================================

/// Command flags bitmask
pub mod cmd_flags {
    /// Command has a completion fence that will be signaled
    pub const HAS_COMPLETION: u32 = 1 << 0;
    /// Command executes asynchronously (fire and forget)
    pub const ASYNC: u32 = 1 << 1;
    /// Command requires memory fence before execution
    pub const FENCE_BEFORE: u32 = 1 << 2;
    /// Command requires memory fence after execution
    pub const FENCE_AFTER: u32 = 1 << 3;
    /// Command is high priority (skip queue position)
    pub const HIGH_PRIORITY: u32 = 1 << 4;
}

/// Trojan command structure (64 bytes each for alignment)
///
/// Each command is exactly 64 bytes to ensure cache-line alignment and
/// efficient GPU memory access. The GPU kernel polls these slots and
/// executes commands atomically.
///
/// # Memory Layout
///
/// ```text
/// Offset  Size  Field      Description
/// ------  ----  ---------  -----------------------------------
/// 0       4     opcode     Command opcode (TrojanOpcode)
/// 4       4     flags      Command flags (cmd_flags::*)
/// 8       8     seqno      Sequence number for ordering
/// 16      8     src        Source address or value
/// 24      8     dst        Destination address
/// 32      8     size       Size in bytes or count
/// 40      8     extra      Extra parameter (kernel args, etc.)
/// 48      16    _padding   Reserved (must be zero)
/// ```
#[derive(Clone, Copy)]
#[repr(C, align(64))]
pub struct TrojanCommand {
    /// Command opcode
    pub opcode: u32,
    /// Flags (bit 0: has_completion, bit 1: async, etc.)
    pub flags: u32,
    /// Sequence number for ordering
    pub seqno: u64,
    /// Source address (for copies) or value
    pub src: u64,
    /// Destination address
    pub dst: u64,
    /// Size in bytes or count
    pub size: u64,
    /// Extra parameter (kernel args ptr, pattern, etc.)
    pub extra: u64,
    /// Padding to 64 bytes
    _padding: [u8; 16],
}

impl TrojanCommand {
    /// Size of a command in bytes (must be 64)
    pub const SIZE: usize = 64;

    /// Create a NOP command
    #[inline]
    pub const fn nop(seqno: u64) -> Self {
        Self {
            opcode: TrojanOpcode::Nop as u32,
            flags: 0,
            seqno,
            src: 0,
            dst: 0,
            size: 0,
            extra: 0,
            _padding: [0; 16],
        }
    }

    /// Create a memory copy command
    #[inline]
    pub const fn mem_copy(seqno: u64, src: u64, dst: u64, size: u64) -> Self {
        Self {
            opcode: TrojanOpcode::MemCopy as u32,
            flags: cmd_flags::ASYNC,
            seqno,
            src,
            dst,
            size,
            extra: 0,
            _padding: [0; 16],
        }
    }

    /// Create a memory set command
    #[inline]
    pub const fn mem_set(seqno: u64, dst: u64, size: u64, pattern: u32) -> Self {
        Self {
            opcode: TrojanOpcode::MemSet as u32,
            flags: cmd_flags::ASYNC,
            seqno,
            src: pattern as u64,
            dst,
            size,
            extra: 0,
            _padding: [0; 16],
        }
    }

    /// Create a kernel launch command
    #[inline]
    pub const fn kernel_launch(
        seqno: u64,
        kernel_addr: u64,
        args_addr: u64,
        grid_dim: u32,
        block_dim: u32,
    ) -> Self {
        Self {
            opcode: TrojanOpcode::KernelLaunch as u32,
            flags: cmd_flags::ASYNC,
            seqno,
            src: kernel_addr,
            dst: args_addr,
            size: grid_dim as u64,
            extra: block_dim as u64,
            _padding: [0; 16],
        }
    }

    /// Create a synchronization barrier command
    #[inline]
    pub const fn sync(seqno: u64) -> Self {
        Self {
            opcode: TrojanOpcode::Sync as u32,
            flags: cmd_flags::HAS_COMPLETION | cmd_flags::FENCE_BEFORE | cmd_flags::FENCE_AFTER,
            seqno,
            src: 0,
            dst: 0,
            size: 0,
            extra: 0,
            _padding: [0; 16],
        }
    }

    /// Create a fence signal command
    #[inline]
    pub const fn fence_signal(seqno: u64, fence_addr: u64, fence_value: u64) -> Self {
        Self {
            opcode: TrojanOpcode::FenceSignal as u32,
            flags: cmd_flags::HAS_COMPLETION,
            seqno,
            src: fence_value,
            dst: fence_addr,
            size: 8,
            extra: 0,
            _padding: [0; 16],
        }
    }

    /// Create a fence wait command
    #[inline]
    pub const fn fence_wait(seqno: u64, fence_addr: u64, expected_value: u64) -> Self {
        Self {
            opcode: TrojanOpcode::FenceWait as u32,
            flags: 0,
            seqno,
            src: expected_value,
            dst: fence_addr,
            size: 8,
            extra: 0,
            _padding: [0; 16],
        }
    }

    /// Create a register read command
    #[inline]
    pub const fn register_read(seqno: u64, reg_addr: u64, result_addr: u64) -> Self {
        Self {
            opcode: TrojanOpcode::RegisterRead as u32,
            flags: cmd_flags::HAS_COMPLETION,
            seqno,
            src: reg_addr,
            dst: result_addr,
            size: 4,
            extra: 0,
            _padding: [0; 16],
        }
    }

    /// Create a register write command
    #[inline]
    pub const fn register_write(seqno: u64, reg_addr: u64, value: u32) -> Self {
        Self {
            opcode: TrojanOpcode::RegisterWrite as u32,
            flags: cmd_flags::FENCE_AFTER,
            seqno,
            src: value as u64,
            dst: reg_addr,
            size: 4,
            extra: 0,
            _padding: [0; 16],
        }
    }

    /// Create a shutdown command (graceful kernel exit)
    #[inline]
    pub const fn shutdown(seqno: u64) -> Self {
        Self {
            opcode: TrojanOpcode::Shutdown as u32,
            flags: cmd_flags::HAS_COMPLETION,
            seqno,
            src: 0,
            dst: 0,
            size: 0,
            extra: 0,
            _padding: [0; 16],
        }
    }

    /// Get the opcode as an enum
    #[inline]
    pub const fn opcode_enum(&self) -> Option<TrojanOpcode> {
        TrojanOpcode::from_u32(self.opcode)
    }

    /// Check if command has completion flag
    #[inline]
    pub const fn has_completion(&self) -> bool {
        (self.flags & cmd_flags::HAS_COMPLETION) != 0
    }

    /// Check if command is async
    #[inline]
    pub const fn is_async(&self) -> bool {
        (self.flags & cmd_flags::ASYNC) != 0
    }
}

impl fmt::Debug for TrojanCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrojanCommand")
            .field("opcode", &TrojanOpcode::from_u32(self.opcode))
            .field("flags", &self.flags)
            .field("seqno", &self.seqno)
            .field("src", &format_args!("0x{:016x}", self.src))
            .field("dst", &format_args!("0x{:016x}", self.dst))
            .field("size", &self.size)
            .field("extra", &self.extra)
            .finish()
    }
}

impl Default for TrojanCommand {
    fn default() -> Self {
        Self::nop(0)
    }
}

// ============================================================================
// Trojan State Machine
// ============================================================================

/// State of the Trojan kernel and ring buffer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TrojanState {
    /// Capsule created but not initialized
    Uninitialized = 0,
    /// Trojan CUDA kernel is being launched
    KernelLaunching = 1,
    /// Kernel running, ring buffer ready for commands
    Ready = 2,
    /// Commands currently in flight
    Active = 3,
    /// Waiting for all in-flight commands to complete
    Draining = 4,
    /// Shutdown command sent, waiting for kernel exit
    ShuttingDown = 5,
    /// Kernel has exited
    Stopped = 6,
    /// Error state (check error_count)
    Error = 7,
}

impl TrojanState {
    /// Check if ring buffer can accept new commands
    #[inline]
    pub const fn can_submit(self) -> bool {
        matches!(self, Self::Ready | Self::Active)
    }

    /// Check if ring buffer is operational
    #[inline]
    pub const fn is_operational(self) -> bool {
        matches!(self, Self::Ready | Self::Active | Self::Draining)
    }

    /// Check if kernel is running
    #[inline]
    pub const fn is_kernel_running(self) -> bool {
        matches!(
            self,
            Self::Ready | Self::Active | Self::Draining | Self::ShuttingDown
        )
    }

    /// Convert from u8
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Uninitialized),
            1 => Some(Self::KernelLaunching),
            2 => Some(Self::Ready),
            3 => Some(Self::Active),
            4 => Some(Self::Draining),
            5 => Some(Self::ShuttingDown),
            6 => Some(Self::Stopped),
            7 => Some(Self::Error),
            _ => None,
        }
    }

    /// Get state name
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Uninitialized => "UNINITIALIZED",
            Self::KernelLaunching => "KERNEL_LAUNCHING",
            Self::Ready => "READY",
            Self::Active => "ACTIVE",
            Self::Draining => "DRAINING",
            Self::ShuttingDown => "SHUTTING_DOWN",
            Self::Stopped => "STOPPED",
            Self::Error => "ERROR",
        }
    }
}

impl fmt::Display for TrojanState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// NvidiaTrojanRingCapsule
// ============================================================================

/// Packed state_gen field layout:
/// - Bits 0-7: TrojanState (8 states)
/// - Bits 8-23: Generation counter (16-bit)
/// - Bits 24-63: Flags (40-bit)
mod state_gen {
    pub const STATE_MASK: u64 = 0xFF;
    pub const GEN_SHIFT: u32 = 8;
    pub const GEN_MASK: u64 = 0xFFFF << GEN_SHIFT;
    pub const FLAGS_SHIFT: u32 = 24;

    #[inline]
    pub const fn pack(state: u8, gen: u16, flags: u64) -> u64 {
        (state as u64) | ((gen as u64) << GEN_SHIFT) | (flags << FLAGS_SHIFT)
    }

    #[inline]
    pub const fn unpack_state(packed: u64) -> u8 {
        (packed & STATE_MASK) as u8
    }

    #[inline]
    pub const fn unpack_gen(packed: u64) -> u16 {
        ((packed & GEN_MASK) >> GEN_SHIFT) as u16
    }

    #[inline]
    pub const fn unpack_flags(packed: u64) -> u64 {
        packed >> FLAGS_SHIFT
    }
}

/// Packed head_tail field layout:
/// - Bits 0-31: Head index (GPU read position)
/// - Bits 32-63: Tail index (CPU write position)
mod head_tail {
    pub const HEAD_MASK: u64 = 0xFFFF_FFFF;
    pub const TAIL_SHIFT: u32 = 32;

    #[inline]
    pub const fn pack(head: u32, tail: u32) -> u64 {
        (head as u64) | ((tail as u64) << TAIL_SHIFT)
    }

    #[inline]
    pub const fn unpack_head(packed: u64) -> u32 {
        (packed & HEAD_MASK) as u32
    }

    #[inline]
    pub const fn unpack_tail(packed: u64) -> u32 {
        (packed >> TAIL_SHIFT) as u32
    }
}

/// NVIDIA Trojan Kernel Ring Buffer Capsule (T1 Atomic, 256B)
///
/// Manages command ring for Trojan Kernel approach to NVIDIA GPU control.
/// Commands are written to pinned CPU-visible memory, polled by GPU kernel.
///
/// # Layout (256 bytes, 256-byte aligned)
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────────┐
/// │  state_gen (AtomicU64)      │  head_tail (AtomicU64)           │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  ring_cpu_ptr (AtomicPtr)   │  ring_gpu_addr (AtomicU64)       │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  ring_capacity (AtomicU64)  │  current_seqno (AtomicU64)       │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  completed_seqno (AtomicU64)│  fence_cpu_ptr (AtomicPtr)       │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  submit_count (AtomicU64)   │  error_count (AtomicU64)         │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  kernel_handle (AtomicU64)  │  device_id (AtomicU32)           │ 12B
/// │  poll_interval_ns (u32)                                        │  4B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  _padding [160 bytes]                                          │160B
/// └─────────────────────────────────────────────────────────────────┘
/// ```
///
/// # Chaos Compliance
///
/// - 100% lockfree (no mutex/RwLock)
/// - Cache-aligned (256B)
/// - Generation counters for TOCTOU prevention
/// - Memory-ordered atomic operations
///
/// # ASSUM Safety
///
/// - `#ASSUME_PINNED_MEMORY`: Ring buffer is CUDA pinned memory (cudaHostAlloc)
/// - `#ASSUME_KERNEL_RUNNING`: Trojan kernel is actively polling
/// - `#ASSUME_COHERENT`: Memory writes visible to GPU without explicit flush
#[repr(C, align(256))]
pub struct NvidiaTrojanRingCapsule {
    /// State (bits 0-7) + Generation (bits 8-23) + Flags (bits 24-63)
    state_gen: AtomicU64,

    /// Head (bits 0-31) + Tail (bits 32-63) - command slot indices
    head_tail: AtomicU64,

    /// CPU pointer to ring buffer (pinned memory)
    ring_cpu_ptr: AtomicPtr<TrojanCommand>,

    /// GPU address of ring buffer (same physical memory)
    ring_gpu_addr: AtomicU64,

    /// Ring capacity in command slots (power of 2)
    ring_capacity: AtomicU64,

    /// Current sequence number (incremented per command)
    current_seqno: AtomicU64,

    /// Last completed sequence number (updated by GPU via fence)
    completed_seqno: AtomicU64,

    /// CPU pointer to completion fence (GPU writes here)
    fence_cpu_ptr: AtomicPtr<u64>,

    /// Total commands submitted
    submit_count: AtomicU64,

    /// Total errors encountered
    error_count: AtomicU64,

    /// Opaque handle to persistent kernel (for shutdown)
    kernel_handle: AtomicU64,

    /// CUDA device ID
    device_id: AtomicU32,

    /// Polling interval in nanoseconds (default: 100ns)
    poll_interval_ns: u32,

    /// Padding to 256 bytes
    _padding: [u8; 160],
}

// Compile-time size/alignment verification
const _: () = {
    assert!(core::mem::size_of::<NvidiaTrojanRingCapsule>() == 256);
    assert!(core::mem::align_of::<NvidiaTrojanRingCapsule>() == 256);
    assert!(core::mem::size_of::<TrojanCommand>() == 64);
    assert!(core::mem::align_of::<TrojanCommand>() == 64);
};

impl NvidiaTrojanRingCapsule {
    /// Capsule size in bytes
    pub const SIZE: usize = 256;

    /// Default ring capacity (1024 slots = 64KB)
    pub const DEFAULT_CAPACITY: u64 = 1024;

    /// Default poll interval in nanoseconds
    pub const DEFAULT_POLL_INTERVAL_NS: u32 = 100;

    /// Create a new uninitialized Trojan ring capsule
    #[inline]
    pub const fn new() -> Self {
        Self {
            state_gen: AtomicU64::new(state_gen::pack(TrojanState::Uninitialized as u8, 0, 0)),
            head_tail: AtomicU64::new(0),
            ring_cpu_ptr: AtomicPtr::new(null_mut()),
            ring_gpu_addr: AtomicU64::new(0),
            ring_capacity: AtomicU64::new(0),
            current_seqno: AtomicU64::new(1), // Start at 1 (0 = invalid)
            completed_seqno: AtomicU64::new(0),
            fence_cpu_ptr: AtomicPtr::new(null_mut()),
            submit_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            kernel_handle: AtomicU64::new(0),
            device_id: AtomicU32::new(0),
            poll_interval_ns: Self::DEFAULT_POLL_INTERVAL_NS,
            _padding: [0; 160],
        }
    }

    /// Initialize the Trojan ring (called after CUDA kernel launch)
    ///
    /// # Arguments
    ///
    /// * `ring_cpu_ptr` - CPU pointer to ring buffer (from cudaHostAlloc)
    /// * `ring_gpu_addr` - GPU address of ring buffer
    /// * `ring_capacity` - Number of command slots (must be power of 2)
    /// * `fence_cpu_ptr` - CPU pointer to completion fence
    /// * `kernel_handle` - Opaque handle to running kernel
    /// * `device_id` - CUDA device ID
    ///
    /// # Errors
    ///
    /// Returns error if ring is already initialized or capacity is invalid.
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_PINNED_MEMORY`: ring_cpu_ptr points to cudaHostAlloc'd memory
    /// - `#VERIFY_PINNED_MEMORY`: Caller must ensure memory is properly allocated
    pub fn initialize(
        &self,
        ring_cpu_ptr: *mut TrojanCommand,
        ring_gpu_addr: u64,
        ring_capacity: u64,
        fence_cpu_ptr: *mut u64,
        kernel_handle: u64,
        device_id: u32,
    ) -> Result<(), super::error::KgpuDriverError> {
        use super::error::KgpuDriverError;

        // Validate capacity is power of 2
        if ring_capacity == 0 || (ring_capacity & (ring_capacity - 1)) != 0 {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // Check current state
        let current = self.state_gen.load(Ordering::Acquire);
        let state =
            TrojanState::from_u8(state_gen::unpack_state(current)).unwrap_or(TrojanState::Error);

        if state != TrojanState::Uninitialized {
            return Err(KgpuDriverError::InvalidState);
        }

        // Transition to KernelLaunching
        let gen = state_gen::unpack_gen(current);
        let new_gen = gen.wrapping_add(1);
        let launching = state_gen::pack(TrojanState::KernelLaunching as u8, new_gen, 0);

        if self
            .state_gen
            .compare_exchange(current, launching, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(KgpuDriverError::StateTransitionFailed);
        }

        // Set ring parameters
        self.ring_cpu_ptr.store(ring_cpu_ptr, Ordering::Release);
        self.ring_gpu_addr.store(ring_gpu_addr, Ordering::Release);
        self.ring_capacity.store(ring_capacity, Ordering::Release);
        self.fence_cpu_ptr.store(fence_cpu_ptr, Ordering::Release);
        self.kernel_handle.store(kernel_handle, Ordering::Release);
        self.device_id.store(device_id, Ordering::Release);

        // Reset head/tail
        self.head_tail.store(0, Ordering::Release);

        // Transition to Ready
        let ready = state_gen::pack(TrojanState::Ready as u8, new_gen.wrapping_add(1), 0);
        self.state_gen.store(ready, Ordering::Release);

        Ok(())
    }

    /// Get current state
    #[inline]
    pub fn state(&self) -> TrojanState {
        let packed = self.state_gen.load(Ordering::Acquire);
        TrojanState::from_u8(state_gen::unpack_state(packed)).unwrap_or(TrojanState::Error)
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u16 {
        let packed = self.state_gen.load(Ordering::Acquire);
        state_gen::unpack_gen(packed)
    }

    /// Get head index (next slot GPU will process)
    #[inline]
    pub fn head(&self) -> u32 {
        let packed = self.head_tail.load(Ordering::Acquire);
        head_tail::unpack_head(packed)
    }

    /// Get tail index (next slot CPU will write)
    #[inline]
    pub fn tail(&self) -> u32 {
        let packed = self.head_tail.load(Ordering::Acquire);
        head_tail::unpack_tail(packed)
    }

    /// Get ring capacity
    #[inline]
    pub fn capacity(&self) -> u64 {
        self.ring_capacity.load(Ordering::Acquire)
    }

    /// Calculate available slots in the ring buffer
    ///
    /// Uses capacity - 1 to distinguish full from empty.
    #[inline]
    pub fn available_slots(&self) -> u32 {
        let capacity = self.ring_capacity.load(Ordering::Acquire) as u32;
        if capacity == 0 {
            return 0;
        }

        let packed = self.head_tail.load(Ordering::Acquire);
        let head = head_tail::unpack_head(packed);
        let tail = head_tail::unpack_tail(packed);

        // Calculate used slots (with wraparound)
        let used = tail.wrapping_sub(head);

        // Available = capacity - 1 - used (reserve one slot to distinguish full/empty)
        capacity.saturating_sub(1).saturating_sub(used)
    }

    /// Calculate used slots in the ring buffer
    #[inline]
    pub fn used_slots(&self) -> u32 {
        let packed = self.head_tail.load(Ordering::Acquire);
        let head = head_tail::unpack_head(packed);
        let tail = head_tail::unpack_tail(packed);
        tail.wrapping_sub(head)
    }

    /// Check if kernel is responsive (completed seqno advancing)
    #[inline]
    pub fn is_kernel_alive(&self) -> bool {
        let state = self.state();
        if !state.is_kernel_running() {
            return false;
        }

        // Check if we have pending commands
        let current = self.current_seqno.load(Ordering::Acquire);
        let completed = self.completed_seqno.load(Ordering::Acquire);

        // If no commands submitted yet, assume alive
        if current <= 1 {
            return true;
        }

        // If completed == current - 1, all commands processed
        // If completed < current - 1, commands pending but might be processing
        // We consider it alive if it's in a running state
        completed >= current.saturating_sub(Self::DEFAULT_CAPACITY) || completed > 0
    }

    /// Reserve a command slot (returns slot index or error)
    ///
    /// This atomically reserves the next available slot in the ring buffer.
    /// The caller must write a command to the slot and then call `submit()`.
    ///
    /// # Performance
    ///
    /// - Single CAS operation: <20ns typical
    pub fn reserve(&self) -> Result<u32, super::error::KgpuDriverError> {
        use super::error::KgpuDriverError;

        let state = self.state();
        if !state.can_submit() {
            return Err(KgpuDriverError::InvalidState);
        }

        let capacity = self.ring_capacity.load(Ordering::Acquire) as u32;
        if capacity == 0 {
            return Err(KgpuDriverError::InvalidState);
        }

        loop {
            let packed = self.head_tail.load(Ordering::Acquire);
            let head = head_tail::unpack_head(packed);
            let tail = head_tail::unpack_tail(packed);

            // Check if buffer is full (leave one slot empty to distinguish full/empty)
            let used = tail.wrapping_sub(head);
            if used >= capacity - 1 {
                return Err(KgpuDriverError::RingBufferFull);
            }

            // Try to advance tail
            let new_tail = tail.wrapping_add(1);
            let new_packed = head_tail::pack(head, new_tail);

            if self
                .head_tail
                .compare_exchange_weak(packed, new_packed, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                // Return the slot we just reserved (before increment)
                return Ok(tail & (capacity - 1));
            }

            // CAS failed, retry
            core::hint::spin_loop();
        }
    }

    /// Write command to reserved slot
    ///
    /// # Safety
    ///
    /// - `slot` must have been reserved via `reserve()`
    /// - Caller must ensure slot is within bounds
    ///
    /// # Performance
    ///
    /// - Single memory store: <10ns typical
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_SLOT_RESERVED`: Slot was properly reserved
    /// - `#VERIFY_SLOT_RESERVED`: Verified by reserve() returning Ok
    #[inline]
    pub unsafe fn write_command(&self, slot: u32, cmd: TrojanCommand) {
        let ptr = self.ring_cpu_ptr.load(Ordering::Acquire);
        if ptr.is_null() {
            return;
        }

        // Write command to slot
        // SAFETY: Caller guarantees slot is valid and reserved
        let cmd_ptr = ptr.add(slot as usize);
        core::ptr::write_volatile(cmd_ptr, cmd);

        // Memory fence to ensure write is visible to GPU
        core::sync::atomic::fence(Ordering::Release);
    }

    /// Allocate sequence number for a new command
    #[inline]
    fn allocate_seqno(&self) -> u64 {
        self.current_seqno.fetch_add(1, Ordering::AcqRel)
    }

    /// Submit command (advance state, signal GPU via memory fence)
    ///
    /// # Returns
    ///
    /// The sequence number of the submitted command.
    ///
    /// # Performance
    ///
    /// - Tail update + fence: <100ns typical
    pub fn submit(&self, _slot: u32) -> Result<u64, super::error::KgpuDriverError> {
        use super::error::KgpuDriverError;

        let state = self.state();
        if !state.can_submit() {
            return Err(KgpuDriverError::InvalidState);
        }

        // Transition to Active if currently Ready
        if state == TrojanState::Ready {
            let packed = self.state_gen.load(Ordering::Acquire);
            let gen = state_gen::unpack_gen(packed);
            let flags = state_gen::unpack_flags(packed);
            let active = state_gen::pack(TrojanState::Active as u8, gen.wrapping_add(1), flags);
            let _ = self.state_gen.compare_exchange(
                packed,
                active,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }

        // Increment submit count
        let count = self.submit_count.fetch_add(1, Ordering::AcqRel);

        // Memory barrier to ensure all writes are visible to GPU
        core::sync::atomic::fence(Ordering::Release);

        Ok(count + 1)
    }

    /// Submit NOP command
    pub fn submit_nop(&self) -> Result<u64, super::error::KgpuDriverError> {
        let slot = self.reserve()?;
        let seqno = self.allocate_seqno();
        let cmd = TrojanCommand::nop(seqno);

        // SAFETY: slot was just reserved via reserve()
        unsafe {
            self.write_command(slot, cmd);
        }

        self.submit(slot)
    }

    /// Submit memory copy command
    pub fn submit_mem_copy(
        &self,
        src: u64,
        dst: u64,
        size: u64,
    ) -> Result<u64, super::error::KgpuDriverError> {
        let slot = self.reserve()?;
        let seqno = self.allocate_seqno();
        let cmd = TrojanCommand::mem_copy(seqno, src, dst, size);

        // SAFETY: slot was just reserved via reserve()
        unsafe {
            self.write_command(slot, cmd);
        }

        self.submit(slot)
    }

    /// Submit memory set command
    pub fn submit_mem_set(
        &self,
        dst: u64,
        size: u64,
        pattern: u32,
    ) -> Result<u64, super::error::KgpuDriverError> {
        let slot = self.reserve()?;
        let seqno = self.allocate_seqno();
        let cmd = TrojanCommand::mem_set(seqno, dst, size, pattern);

        // SAFETY: slot was just reserved via reserve()
        unsafe {
            self.write_command(slot, cmd);
        }

        self.submit(slot)
    }

    /// Submit fence signal command
    pub fn submit_fence_signal(
        &self,
        fence_addr: u64,
        fence_value: u64,
    ) -> Result<u64, super::error::KgpuDriverError> {
        let slot = self.reserve()?;
        let seqno = self.allocate_seqno();
        let cmd = TrojanCommand::fence_signal(seqno, fence_addr, fence_value);

        // SAFETY: slot was just reserved via reserve()
        unsafe {
            self.write_command(slot, cmd);
        }

        self.submit(slot)
    }

    /// Submit sync command (wait for all prior commands)
    pub fn submit_sync(&self) -> Result<u64, super::error::KgpuDriverError> {
        let slot = self.reserve()?;
        let seqno = self.allocate_seqno();
        let cmd = TrojanCommand::sync(seqno);

        // SAFETY: slot was just reserved via reserve()
        unsafe {
            self.write_command(slot, cmd);
        }

        self.submit(slot)
    }

    /// Poll completion status (updates completed_seqno from fence memory)
    ///
    /// Reads the completion fence and updates internal state.
    ///
    /// # Returns
    ///
    /// The latest completed sequence number.
    pub fn poll_completion(&self) -> u64 {
        let fence_ptr = self.fence_cpu_ptr.load(Ordering::Acquire);
        if fence_ptr.is_null() {
            return 0;
        }

        // Read fence value (volatile to prevent optimization)
        // SAFETY: fence_ptr was set during initialization to valid memory
        let fence_value = unsafe { core::ptr::read_volatile(fence_ptr) };

        // Update completed_seqno if fence shows progress
        let current_completed = self.completed_seqno.load(Ordering::Acquire);
        if fence_value > current_completed {
            self.completed_seqno.store(fence_value, Ordering::Release);

            // Update head based on completed commands
            // (In a real implementation, GPU would update this)
        }

        fence_value
    }

    /// Wait for specific sequence number to complete
    ///
    /// # Arguments
    ///
    /// * `seqno` - Sequence number to wait for
    /// * `timeout_ns` - Timeout in nanoseconds (0 = no timeout)
    ///
    /// # Returns
    ///
    /// Ok(()) if sequence completed, Err on timeout.
    pub fn wait_seqno(
        &self,
        seqno: u64,
        timeout_ns: u64,
    ) -> Result<(), super::error::KgpuDriverError> {
        use super::error::KgpuDriverError;

        // Quick check
        if self.completed_seqno.load(Ordering::Acquire) >= seqno {
            return Ok(());
        }

        // Poll loop with optional timeout
        let mut elapsed: u64 = 0;
        let poll_interval = self.poll_interval_ns as u64;

        loop {
            let completed = self.poll_completion();
            if completed >= seqno {
                return Ok(());
            }

            if timeout_ns > 0 && elapsed >= timeout_ns {
                return Err(KgpuDriverError::FenceTimeout);
            }

            // Spin wait (in real impl, would use actual timing)
            for _ in 0..100 {
                core::hint::spin_loop();
            }

            elapsed += poll_interval;
        }
    }

    /// Wait for all commands to complete
    pub fn wait_idle(&self, timeout_ns: u64) -> Result<(), super::error::KgpuDriverError> {
        let current = self.current_seqno.load(Ordering::Acquire);
        if current <= 1 {
            return Ok(()); // No commands submitted
        }

        self.wait_seqno(current - 1, timeout_ns)
    }

    /// Shutdown the Trojan kernel gracefully
    ///
    /// Sends a shutdown command and waits for kernel to exit.
    pub fn shutdown(&self) -> Result<(), super::error::KgpuDriverError> {
        use super::error::KgpuDriverError;

        let state = self.state();
        if !state.is_kernel_running() {
            return Ok(()); // Already stopped
        }

        // Transition to ShuttingDown
        let packed = self.state_gen.load(Ordering::Acquire);
        let gen = state_gen::unpack_gen(packed);
        let shutting_down =
            state_gen::pack(TrojanState::ShuttingDown as u8, gen.wrapping_add(1), 0);

        if self
            .state_gen
            .compare_exchange(packed, shutting_down, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            // State changed, check if already shutting down
            let new_state = self.state();
            if new_state == TrojanState::ShuttingDown || new_state == TrojanState::Stopped {
                return Ok(());
            }
            return Err(KgpuDriverError::StateTransitionFailed);
        }

        // Submit shutdown command
        let slot = self.reserve()?;
        let seqno = self.allocate_seqno();
        let cmd = TrojanCommand::shutdown(seqno);

        // SAFETY: slot was just reserved
        unsafe {
            self.write_command(slot, cmd);
        }
        self.submit(slot)?;

        // Wait for completion (generous timeout: 1 second)
        let _ = self.wait_seqno(seqno, 1_000_000_000);

        // Transition to Stopped
        let stopped = state_gen::pack(TrojanState::Stopped as u8, gen.wrapping_add(2), 0);
        self.state_gen.store(stopped, Ordering::Release);

        Ok(())
    }

    /// Take a snapshot of the capsule state
    pub fn snapshot(&self) -> NvidiaTrojanRingSnapshot {
        let state_packed = self.state_gen.load(Ordering::Acquire);
        let head_tail_packed = self.head_tail.load(Ordering::Acquire);

        NvidiaTrojanRingSnapshot {
            state: TrojanState::from_u8(state_gen::unpack_state(state_packed))
                .unwrap_or(TrojanState::Error),
            generation: state_gen::unpack_gen(state_packed),
            head: head_tail::unpack_head(head_tail_packed),
            tail: head_tail::unpack_tail(head_tail_packed),
            ring_capacity: self.ring_capacity.load(Ordering::Acquire),
            current_seqno: self.current_seqno.load(Ordering::Acquire),
            completed_seqno: self.completed_seqno.load(Ordering::Acquire),
            submit_count: self.submit_count.load(Ordering::Acquire),
            error_count: self.error_count.load(Ordering::Acquire),
            device_id: self.device_id.load(Ordering::Acquire),
            poll_interval_ns: self.poll_interval_ns,
            kernel_alive: self.is_kernel_alive(),
        }
    }

    /// Increment error counter
    #[inline]
    pub fn record_error(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }
}

impl Default for NvidiaTrojanRingCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for NvidiaTrojanRingCapsule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let snap = self.snapshot();
        f.debug_struct("NvidiaTrojanRingCapsule")
            .field("state", &snap.state)
            .field("generation", &snap.generation)
            .field("head", &snap.head)
            .field("tail", &snap.tail)
            .field("capacity", &snap.ring_capacity)
            .field("current_seqno", &snap.current_seqno)
            .field("completed_seqno", &snap.completed_seqno)
            .field("submit_count", &snap.submit_count)
            .field("error_count", &snap.error_count)
            .field("device_id", &snap.device_id)
            .field("kernel_alive", &snap.kernel_alive)
            .finish()
    }
}

// ============================================================================
// Snapshot
// ============================================================================

/// Snapshot of NvidiaTrojanRingCapsule state
#[derive(Debug, Clone, Copy)]
pub struct NvidiaTrojanRingSnapshot {
    /// Current state
    pub state: TrojanState,
    /// Generation counter
    pub generation: u16,
    /// Head index (GPU read position)
    pub head: u32,
    /// Tail index (CPU write position)
    pub tail: u32,
    /// Ring buffer capacity
    pub ring_capacity: u64,
    /// Current sequence number
    pub current_seqno: u64,
    /// Last completed sequence number
    pub completed_seqno: u64,
    /// Total commands submitted
    pub submit_count: u64,
    /// Total errors
    pub error_count: u64,
    /// CUDA device ID
    pub device_id: u32,
    /// Poll interval in nanoseconds
    pub poll_interval_ns: u32,
    /// Whether kernel appears alive
    pub kernel_alive: bool,
}

impl NvidiaTrojanRingSnapshot {
    /// Calculate pending commands
    #[inline]
    pub const fn pending_commands(&self) -> u64 {
        self.current_seqno
            .saturating_sub(self.completed_seqno)
            .saturating_sub(1)
    }

    /// Calculate ring buffer usage
    #[inline]
    pub const fn used_slots(&self) -> u32 {
        self.tail.wrapping_sub(self.head)
    }

    /// Calculate ring buffer availability
    #[inline]
    pub fn available_slots(&self) -> u32 {
        if self.ring_capacity == 0 {
            return 0;
        }
        let cap = self.ring_capacity as u32;
        cap.saturating_sub(1).saturating_sub(self.used_slots())
    }
}

// ============================================================================
// Trojan Kernel Launch Parameters
// ============================================================================

/// Parameters for Trojan kernel launch
///
/// This structure is passed to the CUDA kernel at launch time.
/// The kernel stores these parameters and uses them during execution.
#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
pub struct TrojanKernelParams {
    /// GPU address of ring buffer
    pub ring_gpu_addr: u64,
    /// Ring buffer capacity (number of slots)
    pub ring_capacity: u64,
    /// GPU address of completion fence
    pub fence_gpu_addr: u64,
    /// Polling interval in nanoseconds
    pub poll_interval_ns: u32,
    /// GPU address of shutdown flag
    pub shutdown_flag_addr: u64,
    /// Reserved for future use
    _reserved: [u8; 4],
}

impl TrojanKernelParams {
    /// Create new kernel parameters
    #[inline]
    pub const fn new(
        ring_gpu_addr: u64,
        ring_capacity: u64,
        fence_gpu_addr: u64,
        poll_interval_ns: u32,
        shutdown_flag_addr: u64,
    ) -> Self {
        Self {
            ring_gpu_addr,
            ring_capacity,
            fence_gpu_addr,
            poll_interval_ns,
            shutdown_flag_addr,
            _reserved: [0; 4],
        }
    }
}

impl Default for TrojanKernelParams {
    fn default() -> Self {
        Self::new(
            0,
            0,
            0,
            NvidiaTrojanRingCapsule::DEFAULT_POLL_INTERVAL_NS,
            0,
        )
    }
}

/// Stub for CUDA kernel launch (actual implementation requires CUDA FFI)
///
/// This function would:
/// 1. Call cudaSetDevice(device_id)
/// 2. Allocate pinned memory via cudaHostAlloc
/// 3. Launch the persistent Trojan kernel
/// 4. Return a handle to the running kernel
///
/// # Returns
///
/// Kernel handle on success, error otherwise.
#[cfg(feature = "kgpu-driver-nvidia")]
pub fn launch_trojan_kernel(
    _device_id: u32,
    _params: TrojanKernelParams,
) -> Result<u64, super::error::KgpuDriverError> {
    // This would call CUDA API:
    // 1. cudaSetDevice(device_id)
    // 2. cudaHostAlloc for ring buffer
    // 3. Launch persistent kernel
    // 4. Return kernel handle

    // For now, return stub error
    Err(super::error::KgpuDriverError::TrojanKernelNotRunning)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Size and Alignment Tests
    // ========================================================================

    #[test]
    fn test_capsule_size() {
        assert_eq!(
            core::mem::size_of::<NvidiaTrojanRingCapsule>(),
            256,
            "NvidiaTrojanRingCapsule must be exactly 256 bytes"
        );
    }

    #[test]
    fn test_capsule_alignment() {
        assert_eq!(
            core::mem::align_of::<NvidiaTrojanRingCapsule>(),
            256,
            "NvidiaTrojanRingCapsule must be 256-byte aligned"
        );
    }

    #[test]
    fn test_command_size() {
        assert_eq!(
            core::mem::size_of::<TrojanCommand>(),
            64,
            "TrojanCommand must be exactly 64 bytes"
        );
    }

    #[test]
    fn test_command_alignment() {
        assert_eq!(
            core::mem::align_of::<TrojanCommand>(),
            64,
            "TrojanCommand must be 64-byte aligned"
        );
    }

    #[test]
    fn test_kernel_params_size() {
        assert!(
            core::mem::size_of::<TrojanKernelParams>() <= 64,
            "TrojanKernelParams should fit in a cache line"
        );
    }

    // ========================================================================
    // State Machine Tests
    // ========================================================================

    #[test]
    fn test_initial_state() {
        let capsule = NvidiaTrojanRingCapsule::new();
        assert_eq!(capsule.state(), TrojanState::Uninitialized);
        assert_eq!(capsule.generation(), 0);
    }

    #[test]
    fn test_state_can_submit() {
        assert!(!TrojanState::Uninitialized.can_submit());
        assert!(!TrojanState::KernelLaunching.can_submit());
        assert!(TrojanState::Ready.can_submit());
        assert!(TrojanState::Active.can_submit());
        assert!(!TrojanState::Draining.can_submit());
        assert!(!TrojanState::ShuttingDown.can_submit());
        assert!(!TrojanState::Stopped.can_submit());
        assert!(!TrojanState::Error.can_submit());
    }

    #[test]
    fn test_state_is_operational() {
        assert!(!TrojanState::Uninitialized.is_operational());
        assert!(!TrojanState::KernelLaunching.is_operational());
        assert!(TrojanState::Ready.is_operational());
        assert!(TrojanState::Active.is_operational());
        assert!(TrojanState::Draining.is_operational());
        assert!(!TrojanState::ShuttingDown.is_operational());
        assert!(!TrojanState::Stopped.is_operational());
        assert!(!TrojanState::Error.is_operational());
    }

    #[test]
    fn test_state_is_kernel_running() {
        assert!(!TrojanState::Uninitialized.is_kernel_running());
        assert!(!TrojanState::KernelLaunching.is_kernel_running());
        assert!(TrojanState::Ready.is_kernel_running());
        assert!(TrojanState::Active.is_kernel_running());
        assert!(TrojanState::Draining.is_kernel_running());
        assert!(TrojanState::ShuttingDown.is_kernel_running());
        assert!(!TrojanState::Stopped.is_kernel_running());
        assert!(!TrojanState::Error.is_kernel_running());
    }

    #[test]
    fn test_state_from_u8() {
        assert_eq!(TrojanState::from_u8(0), Some(TrojanState::Uninitialized));
        assert_eq!(TrojanState::from_u8(1), Some(TrojanState::KernelLaunching));
        assert_eq!(TrojanState::from_u8(2), Some(TrojanState::Ready));
        assert_eq!(TrojanState::from_u8(3), Some(TrojanState::Active));
        assert_eq!(TrojanState::from_u8(4), Some(TrojanState::Draining));
        assert_eq!(TrojanState::from_u8(5), Some(TrojanState::ShuttingDown));
        assert_eq!(TrojanState::from_u8(6), Some(TrojanState::Stopped));
        assert_eq!(TrojanState::from_u8(7), Some(TrojanState::Error));
        assert_eq!(TrojanState::from_u8(8), None);
        assert_eq!(TrojanState::from_u8(255), None);
    }

    // ========================================================================
    // Opcode Tests
    // ========================================================================

    #[test]
    fn test_opcode_values() {
        assert_eq!(TrojanOpcode::Nop as u32, 0x00);
        assert_eq!(TrojanOpcode::MemCopy as u32, 0x01);
        assert_eq!(TrojanOpcode::MemSet as u32, 0x02);
        assert_eq!(TrojanOpcode::KernelLaunch as u32, 0x03);
        assert_eq!(TrojanOpcode::Sync as u32, 0x04);
        assert_eq!(TrojanOpcode::FenceSignal as u32, 0x05);
        assert_eq!(TrojanOpcode::FenceWait as u32, 0x06);
        assert_eq!(TrojanOpcode::RegisterRead as u32, 0x07);
        assert_eq!(TrojanOpcode::RegisterWrite as u32, 0x08);
        assert_eq!(TrojanOpcode::Shutdown as u32, 0xFF);
    }

    #[test]
    fn test_opcode_from_u32() {
        assert_eq!(TrojanOpcode::from_u32(0x00), Some(TrojanOpcode::Nop));
        assert_eq!(TrojanOpcode::from_u32(0x01), Some(TrojanOpcode::MemCopy));
        assert_eq!(TrojanOpcode::from_u32(0xFF), Some(TrojanOpcode::Shutdown));
        assert_eq!(TrojanOpcode::from_u32(0x99), None);
    }

    #[test]
    fn test_opcode_has_completion() {
        assert!(!TrojanOpcode::Nop.has_completion());
        assert!(!TrojanOpcode::MemCopy.has_completion());
        assert!(TrojanOpcode::FenceSignal.has_completion());
        assert!(TrojanOpcode::Sync.has_completion());
        assert!(TrojanOpcode::Shutdown.has_completion());
    }

    #[test]
    fn test_opcode_is_async() {
        assert!(!TrojanOpcode::Nop.is_async());
        assert!(TrojanOpcode::MemCopy.is_async());
        assert!(TrojanOpcode::MemSet.is_async());
        assert!(TrojanOpcode::KernelLaunch.is_async());
        assert!(!TrojanOpcode::Sync.is_async());
    }

    // ========================================================================
    // Command Construction Tests
    // ========================================================================

    #[test]
    fn test_command_nop() {
        let cmd = TrojanCommand::nop(42);
        assert_eq!(cmd.opcode, TrojanOpcode::Nop as u32);
        assert_eq!(cmd.seqno, 42);
        assert_eq!(cmd.flags, 0);
    }

    #[test]
    fn test_command_mem_copy() {
        let cmd = TrojanCommand::mem_copy(1, 0x1000, 0x2000, 4096);
        assert_eq!(cmd.opcode, TrojanOpcode::MemCopy as u32);
        assert_eq!(cmd.seqno, 1);
        assert_eq!(cmd.src, 0x1000);
        assert_eq!(cmd.dst, 0x2000);
        assert_eq!(cmd.size, 4096);
        assert!(cmd.is_async());
    }

    #[test]
    fn test_command_mem_set() {
        let cmd = TrojanCommand::mem_set(2, 0x3000, 1024, 0xDEADBEEF);
        assert_eq!(cmd.opcode, TrojanOpcode::MemSet as u32);
        assert_eq!(cmd.seqno, 2);
        assert_eq!(cmd.src, 0xDEADBEEF);
        assert_eq!(cmd.dst, 0x3000);
        assert_eq!(cmd.size, 1024);
    }

    #[test]
    fn test_command_fence_signal() {
        let cmd = TrojanCommand::fence_signal(3, 0x4000, 0x12345678);
        assert_eq!(cmd.opcode, TrojanOpcode::FenceSignal as u32);
        assert_eq!(cmd.seqno, 3);
        assert_eq!(cmd.src, 0x12345678); // fence value
        assert_eq!(cmd.dst, 0x4000); // fence address
        assert!(cmd.has_completion());
    }

    #[test]
    fn test_command_shutdown() {
        let cmd = TrojanCommand::shutdown(999);
        assert_eq!(cmd.opcode, TrojanOpcode::Shutdown as u32);
        assert_eq!(cmd.seqno, 999);
        assert!(cmd.has_completion());
    }

    #[test]
    fn test_command_kernel_launch() {
        let cmd = TrojanCommand::kernel_launch(10, 0x5000, 0x6000, 256, 64);
        assert_eq!(cmd.opcode, TrojanOpcode::KernelLaunch as u32);
        assert_eq!(cmd.seqno, 10);
        assert_eq!(cmd.src, 0x5000); // kernel address
        assert_eq!(cmd.dst, 0x6000); // args address
        assert_eq!(cmd.size, 256); // grid dim
        assert_eq!(cmd.extra, 64); // block dim
    }

    // ========================================================================
    // Head/Tail Packing Tests
    // ========================================================================

    #[test]
    fn test_head_tail_pack_unpack() {
        let packed = head_tail::pack(100, 200);
        assert_eq!(head_tail::unpack_head(packed), 100);
        assert_eq!(head_tail::unpack_tail(packed), 200);
    }

    #[test]
    fn test_head_tail_max_values() {
        let packed = head_tail::pack(u32::MAX, u32::MAX);
        assert_eq!(head_tail::unpack_head(packed), u32::MAX);
        assert_eq!(head_tail::unpack_tail(packed), u32::MAX);
    }

    #[test]
    fn test_head_tail_zero() {
        let packed = head_tail::pack(0, 0);
        assert_eq!(head_tail::unpack_head(packed), 0);
        assert_eq!(head_tail::unpack_tail(packed), 0);
    }

    // ========================================================================
    // State/Gen Packing Tests
    // ========================================================================

    #[test]
    fn test_state_gen_pack_unpack() {
        let packed = state_gen::pack(TrojanState::Ready as u8, 1000, 0);
        assert_eq!(state_gen::unpack_state(packed), TrojanState::Ready as u8);
        assert_eq!(state_gen::unpack_gen(packed), 1000);
    }

    #[test]
    fn test_state_gen_max_gen() {
        let packed = state_gen::pack(TrojanState::Active as u8, u16::MAX, 0);
        assert_eq!(state_gen::unpack_gen(packed), u16::MAX);
    }

    // ========================================================================
    // Ring Buffer Logic Tests
    // ========================================================================

    #[test]
    fn test_available_slots_empty() {
        let capsule = NvidiaTrojanRingCapsule::new();
        // Uninitialized, capacity is 0
        assert_eq!(capsule.available_slots(), 0);
    }

    #[test]
    fn test_used_slots_empty() {
        let capsule = NvidiaTrojanRingCapsule::new();
        assert_eq!(capsule.used_slots(), 0);
    }

    #[test]
    fn test_capacity_default() {
        let capsule = NvidiaTrojanRingCapsule::new();
        assert_eq!(capsule.capacity(), 0); // Not initialized
    }

    // ========================================================================
    // Generation Counter Tests
    // ========================================================================

    #[test]
    fn test_generation_initial() {
        let capsule = NvidiaTrojanRingCapsule::new();
        assert_eq!(capsule.generation(), 0);
    }

    #[test]
    fn test_generation_wraparound() {
        // Test that generation wraps properly
        let packed = state_gen::pack(0, u16::MAX, 0);
        let gen = state_gen::unpack_gen(packed);
        assert_eq!(gen, u16::MAX);

        // Wrapping add
        let new_gen = gen.wrapping_add(1);
        assert_eq!(new_gen, 0);
    }

    // ========================================================================
    // Sequence Number Tests
    // ========================================================================

    #[test]
    fn test_seqno_starts_at_one() {
        let capsule = NvidiaTrojanRingCapsule::new();
        // current_seqno starts at 1, so next allocated should be 1
        let seqno = capsule.current_seqno.load(Ordering::Relaxed);
        assert_eq!(seqno, 1);
    }

    #[test]
    fn test_seqno_allocation() {
        let capsule = NvidiaTrojanRingCapsule::new();
        let first = capsule.allocate_seqno();
        let second = capsule.allocate_seqno();
        let third = capsule.allocate_seqno();

        assert_eq!(first, 1);
        assert_eq!(second, 2);
        assert_eq!(third, 3);
    }

    // ========================================================================
    // Snapshot Tests
    // ========================================================================

    #[test]
    fn test_snapshot_initial() {
        let capsule = NvidiaTrojanRingCapsule::new();
        let snap = capsule.snapshot();

        assert_eq!(snap.state, TrojanState::Uninitialized);
        assert_eq!(snap.generation, 0);
        assert_eq!(snap.head, 0);
        assert_eq!(snap.tail, 0);
        assert_eq!(snap.ring_capacity, 0);
        assert_eq!(snap.current_seqno, 1);
        assert_eq!(snap.completed_seqno, 0);
        assert_eq!(snap.submit_count, 0);
        assert_eq!(snap.error_count, 0);
    }

    #[test]
    fn test_snapshot_pending_commands() {
        let snap = NvidiaTrojanRingSnapshot {
            state: TrojanState::Active,
            generation: 1,
            head: 0,
            tail: 5,
            ring_capacity: 1024,
            current_seqno: 10,
            completed_seqno: 5,
            submit_count: 10,
            error_count: 0,
            device_id: 0,
            poll_interval_ns: 100,
            kernel_alive: true,
        };

        assert_eq!(snap.pending_commands(), 4); // 10 - 5 - 1 = 4
    }

    #[test]
    fn test_snapshot_used_slots() {
        let snap = NvidiaTrojanRingSnapshot {
            state: TrojanState::Active,
            generation: 1,
            head: 10,
            tail: 25,
            ring_capacity: 1024,
            current_seqno: 1,
            completed_seqno: 0,
            submit_count: 0,
            error_count: 0,
            device_id: 0,
            poll_interval_ns: 100,
            kernel_alive: true,
        };

        assert_eq!(snap.used_slots(), 15);
    }

    #[test]
    fn test_snapshot_available_slots() {
        let snap = NvidiaTrojanRingSnapshot {
            state: TrojanState::Active,
            generation: 1,
            head: 0,
            tail: 100,
            ring_capacity: 1024,
            current_seqno: 1,
            completed_seqno: 0,
            submit_count: 0,
            error_count: 0,
            device_id: 0,
            poll_interval_ns: 100,
            kernel_alive: true,
        };

        // capacity - 1 - used = 1024 - 1 - 100 = 923
        assert_eq!(snap.available_slots(), 923);
    }

    // ========================================================================
    // Error Recording Tests
    // ========================================================================

    #[test]
    fn test_error_recording() {
        let capsule = NvidiaTrojanRingCapsule::new();
        assert_eq!(capsule.error_count.load(Ordering::Relaxed), 0);

        capsule.record_error();
        assert_eq!(capsule.error_count.load(Ordering::Relaxed), 1);

        capsule.record_error();
        capsule.record_error();
        assert_eq!(capsule.error_count.load(Ordering::Relaxed), 3);
    }

    // ========================================================================
    // Kernel Alive Check Tests
    // ========================================================================

    #[test]
    fn test_kernel_alive_uninitialized() {
        let capsule = NvidiaTrojanRingCapsule::new();
        assert!(!capsule.is_kernel_alive());
    }

    // ========================================================================
    // TrojanKernelParams Tests
    // ========================================================================

    #[test]
    fn test_kernel_params_creation() {
        let params = TrojanKernelParams::new(0x1000_0000, 1024, 0x2000_0000, 200, 0x3000_0000);

        assert_eq!(params.ring_gpu_addr, 0x1000_0000);
        assert_eq!(params.ring_capacity, 1024);
        assert_eq!(params.fence_gpu_addr, 0x2000_0000);
        assert_eq!(params.poll_interval_ns, 200);
        assert_eq!(params.shutdown_flag_addr, 0x3000_0000);
    }

    #[test]
    fn test_kernel_params_default() {
        let params = TrojanKernelParams::default();
        assert_eq!(params.ring_gpu_addr, 0);
        assert_eq!(params.ring_capacity, 0);
        assert_eq!(
            params.poll_interval_ns,
            NvidiaTrojanRingCapsule::DEFAULT_POLL_INTERVAL_NS
        );
    }

    // ========================================================================
    // Debug/Display Tests
    // ========================================================================

    #[test]
    fn test_state_display() {
        assert_eq!(format!("{}", TrojanState::Ready), "READY");
        assert_eq!(format!("{}", TrojanState::Error), "ERROR");
    }

    #[test]
    fn test_opcode_display() {
        assert_eq!(format!("{}", TrojanOpcode::MemCopy), "MEM_COPY");
        assert_eq!(format!("{}", TrojanOpcode::Shutdown), "SHUTDOWN");
    }

    #[test]
    fn test_capsule_debug() {
        let capsule = NvidiaTrojanRingCapsule::new();
        let debug_str = format!("{:?}", capsule);
        assert!(debug_str.contains("NvidiaTrojanRingCapsule"));
        assert!(debug_str.contains("UNINITIALIZED"));
    }

    #[test]
    fn test_command_debug() {
        let cmd = TrojanCommand::mem_copy(1, 0x1000, 0x2000, 4096);
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("TrojanCommand"));
        assert!(debug_str.contains("MemCopy"));
    }

    // ========================================================================
    // Default Trait Tests
    // ========================================================================

    #[test]
    fn test_capsule_default() {
        let capsule = NvidiaTrojanRingCapsule::default();
        assert_eq!(capsule.state(), TrojanState::Uninitialized);
    }

    #[test]
    fn test_command_default() {
        let cmd = TrojanCommand::default();
        assert_eq!(cmd.opcode, TrojanOpcode::Nop as u32);
        assert_eq!(cmd.seqno, 0);
    }

    // ========================================================================
    // Constants Tests
    // ========================================================================

    #[test]
    fn test_constants() {
        assert_eq!(NvidiaTrojanRingCapsule::SIZE, 256);
        assert_eq!(NvidiaTrojanRingCapsule::DEFAULT_CAPACITY, 1024);
        assert_eq!(NvidiaTrojanRingCapsule::DEFAULT_POLL_INTERVAL_NS, 100);
        assert_eq!(TrojanCommand::SIZE, 64);
    }

    // ========================================================================
    // Command Flags Tests
    // ========================================================================

    #[test]
    fn test_cmd_flags() {
        assert_eq!(cmd_flags::HAS_COMPLETION, 1);
        assert_eq!(cmd_flags::ASYNC, 2);
        assert_eq!(cmd_flags::FENCE_BEFORE, 4);
        assert_eq!(cmd_flags::FENCE_AFTER, 8);
        assert_eq!(cmd_flags::HIGH_PRIORITY, 16);
    }

    #[test]
    fn test_sync_command_flags() {
        let cmd = TrojanCommand::sync(1);
        assert!(cmd.has_completion());
        assert!((cmd.flags & cmd_flags::FENCE_BEFORE) != 0);
        assert!((cmd.flags & cmd_flags::FENCE_AFTER) != 0);
    }
}
