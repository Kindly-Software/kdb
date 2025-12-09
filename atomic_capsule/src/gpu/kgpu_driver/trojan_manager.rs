//! Trojan Kernel Manager Capsule for KGPU-Driver v2.0
//!
//! Manages the lifecycle of the NVIDIA Trojan Kernel - a persistent CUDA kernel
//! that polls a pinned ring buffer to achieve sub-100ns GPU command latency.
//!
//! # Architecture
//!
//! The Trojan Kernel approach bypasses NVIDIA's locked GSP firmware by:
//! 1. Launching a persistent CUDA kernel that never exits
//! 2. Allocating pinned (cudaHostAlloc) memory shared between CPU and GPU
//! 3. CPU writes commands to ring buffer, GPU polls and executes instantly
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                  Trojan Manager Architecture                        │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │                                                                      │
//! │  TrojanManagerCapsule (512B, T1 Atomic)                             │
//! │  ┌─────────────────────────────────────────────────────────────┐    │
//! │  │ state (AtomicU64)         │ generation (AtomicU64)          │    │
//! │  │ cuda_context (AtomicU64)  │ cuda_module (AtomicU64)         │    │
//! │  │ cuda_function (AtomicU64) │ cuda_stream (AtomicU64)         │    │
//! │  │ ring_host_ptr (AtomicU64) │ ring_device_ptr (AtomicU64)     │    │
//! │  │ ring_size (AtomicU64)     │ metrics...                      │    │
//! │  └─────────────────────────────────────────────────────────────┘    │
//! │                              ▼                                       │
//! │  TrojanRingHeader (64B, cache-aligned, in pinned memory)            │
//! │  ┌─────────────────────────────────────────────────────────────┐    │
//! │  │ head (GPU write) │ tail (CPU write) │ stop_flag │ status    │    │
//! │  └─────────────────────────────────────────────────────────────┘    │
//! │                              ▼                                       │
//! │  TrojanCommand[N] (64B each, follows header in pinned memory)       │
//! │                                                                      │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Capsule Tier
//!
//! - **T1 Atomic**: 512-byte cache-aligned capsule with generation counters
//! - **Performance Targets**:
//!   - State read: <10ns
//!   - Command submit: <100ns
//!   - Snapshot: <50ns
//!
//! # State Machine
//!
//! ```text
//! Uninitialized → CudaInitialized → ContextCreated → ModuleLoaded
//!     → KernelReady → RingAllocated → KernelLaunched
//!
//! KernelLaunched → Stopping → Uninitialized (graceful shutdown)
//! Any → Error (on failure)
//! ```
//!
//! # UCE34 Compliance
//!
//! - Q10: T1 Atomic tier (lockfree state machine via CAS)
//! - Q11: Rust transform (type-safe CUDA handle management)
//! - Q33: 100% lockfree via AtomicU64
//! - Q34: Audit trail via generation counters and sequence numbers
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_CUDA_INITIALIZED`: CUDA runtime is properly initialized before use
//! - `#ASSUME_PINNED_MEMORY`: Ring buffer allocated via cudaHostAlloc
//! - `#ASSUME_KERNEL_LAUNCHED`: Trojan kernel is running before command submission
//! - `#ASSUME_COHERENT`: Memory writes visible to GPU without explicit flush

#![allow(dead_code)] // Allow during development

use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "std")]
extern crate std;

use super::error::{KgpuDriverError, KgpuDriverResult};
use super::nvidia_ring::TrojanCommand;

// ============================================================================
// Trojan Manager State
// ============================================================================

/// Trojan Manager lifecycle states
///
/// Each state represents a step in the CUDA initialization and kernel launch
/// sequence. States must progress forward (except for Error and shutdown).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TrojanManagerState {
    /// Capsule created but CUDA not initialized
    Uninitialized = 0,
    /// cuInit() called successfully
    CudaInitialized = 1,
    /// cuCtxCreate() completed, context is active
    ContextCreated = 2,
    /// cuModuleLoad() completed, PTX loaded
    ModuleLoaded = 3,
    /// cuModuleGetFunction() completed, kernel function ready
    KernelReady = 4,
    /// Pinned ring buffer allocated via cudaHostAlloc
    RingAllocated = 5,
    /// Trojan kernel launched and running (main operational state)
    KernelLaunched = 6,
    /// Shutdown initiated, waiting for kernel to exit
    Stopping = 7,
    /// Error state (check error fields for details)
    Error = 8,
}

impl TrojanManagerState {
    /// Check if the manager is fully operational
    #[inline]
    pub const fn is_operational(self) -> bool {
        matches!(self, Self::KernelLaunched)
    }

    /// Check if kernel is running (launched or stopping)
    #[inline]
    pub const fn is_kernel_running(self) -> bool {
        matches!(self, Self::KernelLaunched | Self::Stopping)
    }

    /// Check if ring buffer is available
    #[inline]
    pub const fn has_ring(self) -> bool {
        matches!(
            self,
            Self::RingAllocated | Self::KernelLaunched | Self::Stopping
        )
    }

    /// Check if state can transition to next state
    #[inline]
    pub const fn can_advance(self) -> bool {
        matches!(
            self,
            Self::Uninitialized
                | Self::CudaInitialized
                | Self::ContextCreated
                | Self::ModuleLoaded
                | Self::KernelReady
                | Self::RingAllocated
        )
    }

    /// Get the expected next state in the initialization sequence
    #[inline]
    pub const fn next_state(self) -> Option<Self> {
        match self {
            Self::Uninitialized => Some(Self::CudaInitialized),
            Self::CudaInitialized => Some(Self::ContextCreated),
            Self::ContextCreated => Some(Self::ModuleLoaded),
            Self::ModuleLoaded => Some(Self::KernelReady),
            Self::KernelReady => Some(Self::RingAllocated),
            Self::RingAllocated => Some(Self::KernelLaunched),
            Self::KernelLaunched => Some(Self::Stopping),
            Self::Stopping => Some(Self::Uninitialized),
            Self::Error => None,
        }
    }

    /// Convert from u8
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Uninitialized),
            1 => Some(Self::CudaInitialized),
            2 => Some(Self::ContextCreated),
            3 => Some(Self::ModuleLoaded),
            4 => Some(Self::KernelReady),
            5 => Some(Self::RingAllocated),
            6 => Some(Self::KernelLaunched),
            7 => Some(Self::Stopping),
            8 => Some(Self::Error),
            _ => None,
        }
    }

    /// Get state name for logging
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Uninitialized => "UNINITIALIZED",
            Self::CudaInitialized => "CUDA_INITIALIZED",
            Self::ContextCreated => "CONTEXT_CREATED",
            Self::ModuleLoaded => "MODULE_LOADED",
            Self::KernelReady => "KERNEL_READY",
            Self::RingAllocated => "RING_ALLOCATED",
            Self::KernelLaunched => "KERNEL_LAUNCHED",
            Self::Stopping => "STOPPING",
            Self::Error => "ERROR",
        }
    }
}

impl fmt::Display for TrojanManagerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// State Packing Helpers
// ============================================================================

/// Packed state field layout:
/// - Bits 0-7: TrojanManagerState (8 bits)
/// - Bits 8-15: Device ordinal (8 bits, 0-255)
/// - Bits 16-31: SM count (16 bits)
/// - Bits 32-39: Flags (8 bits)
/// - Bits 40-63: Reserved (24 bits)
mod state_pack {
    pub const STATE_MASK: u64 = 0xFF;
    pub const DEVICE_SHIFT: u32 = 8;
    pub const DEVICE_MASK: u64 = 0xFF << DEVICE_SHIFT;
    pub const SM_SHIFT: u32 = 16;
    pub const SM_MASK: u64 = 0xFFFF << SM_SHIFT;
    pub const FLAGS_SHIFT: u32 = 32;
    pub const FLAGS_MASK: u64 = 0xFF << FLAGS_SHIFT;

    // Flag bits
    pub const FLAG_KERNEL_RUNNING: u64 = 1 << FLAGS_SHIFT;
    pub const FLAG_RING_VALID: u64 = 1 << (FLAGS_SHIFT + 1);
    pub const FLAG_SHUTDOWN_REQUESTED: u64 = 1 << (FLAGS_SHIFT + 2);

    #[inline]
    pub const fn pack(state: u8, device: u8, sm_count: u16, flags: u8) -> u64 {
        (state as u64)
            | ((device as u64) << DEVICE_SHIFT)
            | ((sm_count as u64) << SM_SHIFT)
            | ((flags as u64) << FLAGS_SHIFT)
    }

    #[inline]
    pub const fn unpack_state(packed: u64) -> u8 {
        (packed & STATE_MASK) as u8
    }

    #[inline]
    pub const fn unpack_device(packed: u64) -> u8 {
        ((packed & DEVICE_MASK) >> DEVICE_SHIFT) as u8
    }

    #[inline]
    pub const fn unpack_sm_count(packed: u64) -> u16 {
        ((packed & SM_MASK) >> SM_SHIFT) as u16
    }

    #[inline]
    pub const fn unpack_flags(packed: u64) -> u8 {
        ((packed & FLAGS_MASK) >> FLAGS_SHIFT) as u8
    }

    #[inline]
    pub const fn with_state(packed: u64, state: u8) -> u64 {
        (packed & !STATE_MASK) | (state as u64)
    }
}

// ============================================================================
// Trojan Ring Header
// ============================================================================

/// Ring buffer header structure (at start of pinned memory)
///
/// This structure is placed at the beginning of the pinned memory region
/// and is shared between CPU (Rust) and GPU (CUDA kernel). All fields
/// are atomic to ensure coherent access.
///
/// # Layout (64 bytes, cache-line aligned)
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────────┐
/// │  head (AtomicU64)            │  tail (AtomicU64)                │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  stop_flag (AtomicU64)       │  kernel_status (AtomicU64)       │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  commands_processed (AtomicU64)│ last_timestamp (AtomicU64)     │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  _padding [16 bytes]                                            │ 16B
/// └─────────────────────────────────────────────────────────────────┘
/// ```
///
/// # ASSUM Safety
///
/// - `#ASSUME_PINNED_MEMORY`: This structure resides in cudaHostAlloc'd memory
/// - `#ASSUME_COHERENT`: GPU can read CPU writes without explicit flush
#[repr(C, align(64))]
pub struct TrojanRingHeader {
    /// GPU write position (GPU increments after processing each command)
    pub head: AtomicU64,
    /// CPU write position (CPU increments after writing each command)
    pub tail: AtomicU64,
    /// Stop flag (set to 1 to request kernel exit)
    pub stop_flag: AtomicU64,
    /// Kernel health status (0=stopped, 1=running, 2=error)
    pub kernel_status: AtomicU64,
    /// Total commands processed by GPU
    pub commands_processed: AtomicU64,
    /// Last processing timestamp (nanoseconds)
    pub last_timestamp: AtomicU64,
    /// Padding to 64 bytes
    pub _padding: [u8; 16],
}

impl TrojanRingHeader {
    /// Header size in bytes
    pub const SIZE: usize = 64;

    /// Kernel status: stopped
    pub const STATUS_STOPPED: u64 = 0;
    /// Kernel status: running
    pub const STATUS_RUNNING: u64 = 1;
    /// Kernel status: error
    pub const STATUS_ERROR: u64 = 2;

    /// Create a new zeroed header (for initialization)
    #[inline]
    pub const fn new() -> Self {
        Self {
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            stop_flag: AtomicU64::new(0),
            kernel_status: AtomicU64::new(0),
            commands_processed: AtomicU64::new(0),
            last_timestamp: AtomicU64::new(0),
            _padding: [0; 16],
        }
    }

    /// Check if kernel is running
    #[inline]
    pub fn is_running(&self) -> bool {
        self.kernel_status.load(Ordering::Acquire) == Self::STATUS_RUNNING
    }

    /// Check if stop has been requested
    #[inline]
    pub fn is_stop_requested(&self) -> bool {
        self.stop_flag.load(Ordering::Acquire) != 0
    }

    /// Request kernel to stop
    #[inline]
    pub fn request_stop(&self) {
        self.stop_flag.store(1, Ordering::Release);
    }

    /// Get current head position
    #[inline]
    pub fn head(&self) -> u64 {
        self.head.load(Ordering::Acquire)
    }

    /// Get current tail position
    #[inline]
    pub fn tail(&self) -> u64 {
        self.tail.load(Ordering::Acquire)
    }

    /// Get pending command count
    #[inline]
    pub fn pending_count(&self) -> u64 {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        tail.wrapping_sub(head)
    }

    /// Advance tail atomically (returns old tail value for slot index)
    #[inline]
    pub fn advance_tail(&self) -> u64 {
        self.tail.fetch_add(1, Ordering::AcqRel)
    }
}

impl Default for TrojanRingHeader {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for TrojanRingHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrojanRingHeader")
            .field("head", &self.head.load(Ordering::Relaxed))
            .field("tail", &self.tail.load(Ordering::Relaxed))
            .field("stop_flag", &self.stop_flag.load(Ordering::Relaxed))
            .field("kernel_status", &self.kernel_status.load(Ordering::Relaxed))
            .field(
                "commands_processed",
                &self.commands_processed.load(Ordering::Relaxed),
            )
            .finish()
    }
}

// ============================================================================
// Trojan Kernel Arguments
// ============================================================================

/// Trojan Kernel launch arguments (passed to CUDA kernel)
///
/// This structure contains all the pointers and parameters the Trojan kernel
/// needs to operate. It is copied to device memory and passed to the kernel.
#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
pub struct TrojanKernelArgs {
    /// Device pointer to ring buffer (commands start after header)
    pub ring_ptr: u64,
    /// Ring buffer capacity in commands
    pub ring_size: u32,
    /// Device pointer to head index (GPU writes)
    pub head_ptr: u64,
    /// Device pointer to tail index (CPU writes)
    pub tail_ptr: u64,
    /// Device pointer to stop flag
    pub stop_flag_ptr: u64,
    /// Polling interval in nanoseconds (0 = busy-wait)
    pub poll_interval_ns: u32,
    /// Device pointer to kernel status
    pub status_ptr: u64,
    /// Device pointer to commands processed counter
    pub processed_ptr: u64,
}

impl TrojanKernelArgs {
    /// Create new kernel arguments
    #[inline]
    pub const fn new(
        ring_ptr: u64,
        ring_size: u32,
        head_ptr: u64,
        tail_ptr: u64,
        stop_flag_ptr: u64,
        poll_interval_ns: u32,
        status_ptr: u64,
        processed_ptr: u64,
    ) -> Self {
        Self {
            ring_ptr,
            ring_size,
            head_ptr,
            tail_ptr,
            stop_flag_ptr,
            poll_interval_ns,
            status_ptr,
            processed_ptr,
        }
    }
}

impl Default for TrojanKernelArgs {
    fn default() -> Self {
        Self {
            ring_ptr: 0,
            ring_size: 0,
            head_ptr: 0,
            tail_ptr: 0,
            stop_flag_ptr: 0,
            poll_interval_ns: 100, // Default 100ns polling
            status_ptr: 0,
            processed_ptr: 0,
        }
    }
}

// ============================================================================
// Trojan Manager Capsule
// ============================================================================

/// Trojan Kernel Manager Capsule (T1 Atomic, 512B)
///
/// Manages the complete lifecycle of the NVIDIA Trojan Kernel:
/// - CUDA initialization (cuInit, cuCtxCreate)
/// - Module loading (cuModuleLoad PTX)
/// - Ring buffer allocation (cudaHostAlloc pinned memory)
/// - Kernel launch (cuLaunchKernel persistent)
/// - Command submission and tracking
/// - Graceful shutdown
///
/// # Layout (512 bytes, 512-byte aligned for 8 cache lines)
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────────┐
/// │  state (AtomicU64)           │  generation (AtomicU64)          │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  cuda_context (AtomicU64)    │  cuda_module (AtomicU64)         │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  cuda_function (AtomicU64)   │  cuda_stream (AtomicU64)         │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  ring_host_ptr (AtomicU64)   │  ring_device_ptr (AtomicU64)     │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  ring_size (AtomicU64)       │  ring_capacity (AtomicU64)       │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  commands_submitted (AtomicU64)│ commands_completed (AtomicU64) │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  last_submit_ns (AtomicU64)  │  last_complete_ns (AtomicU64)    │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  avg_latency_ns (AtomicU64)  │  max_latency_ns (AtomicU64)      │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  min_latency_ns (AtomicU64)  │  total_latency_ns (AtomicU64)    │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  error_count (AtomicU64)     │  last_error (AtomicU64)          │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  current_seqno (AtomicU64)   │  completed_seqno (AtomicU64)     │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  _padding [336 bytes]                                           │336B
/// └─────────────────────────────────────────────────────────────────┘
/// ```
///
/// # Chaos Compliance
///
/// - 100% lockfree (no mutex/RwLock)
/// - 512B aligned (8 cache lines)
/// - Generation counters for TOCTOU prevention
/// - Memory-ordered atomic operations
///
/// # ASSUM Safety
///
/// - `#ASSUME_CUDA_INITIALIZED`: CUDA driver API is loaded
/// - `#ASSUME_PINNED_MEMORY`: Ring buffer is cudaHostAlloc'd
/// - `#ASSUME_KERNEL_RUNNING`: Trojan kernel polls ring buffer
#[repr(C, align(512))]
pub struct TrojanManagerCapsule {
    /// Packed state: [state:8][device:8][sm_count:16][flags:8][reserved:24]
    state: AtomicU64,
    /// Generation counter for state transitions
    generation: AtomicU64,

    /// CUcontext handle (stored as u64)
    cuda_context: AtomicU64,
    /// CUmodule handle (stored as u64)
    cuda_module: AtomicU64,
    /// CUfunction handle for trojan_poll kernel (stored as u64)
    cuda_function: AtomicU64,
    /// CUstream handle (stored as u64)
    cuda_stream: AtomicU64,

    /// CPU pointer to pinned ring buffer (TrojanRingHeader + TrojanCommand[])
    ring_host_ptr: AtomicU64,
    /// GPU device pointer to ring buffer
    ring_device_ptr: AtomicU64,
    /// Ring buffer total size in bytes
    ring_size: AtomicU64,
    /// Ring buffer capacity in commands
    ring_capacity: AtomicU64,

    /// Total commands submitted by CPU
    commands_submitted: AtomicU64,
    /// Total commands completed by GPU
    commands_completed: AtomicU64,
    /// Last command submit timestamp (nanoseconds)
    last_submit_ns: AtomicU64,
    /// Last command complete timestamp (nanoseconds)
    last_complete_ns: AtomicU64,

    /// Average latency in nanoseconds (Q16.16 fixed-point for precision)
    avg_latency_ns: AtomicU64,
    /// Maximum observed latency in nanoseconds
    max_latency_ns: AtomicU64,
    /// Minimum observed latency in nanoseconds
    min_latency_ns: AtomicU64,
    /// Total accumulated latency (for computing average)
    total_latency_ns: AtomicU64,

    /// Error count
    error_count: AtomicU64,
    /// Last error code (KgpuDriverError as u32)
    last_error: AtomicU64,

    /// Current sequence number for commands
    current_seqno: AtomicU64,
    /// Last completed sequence number
    completed_seqno: AtomicU64,

    /// Padding to 512 bytes
    /// 22 AtomicU64 * 8 = 176 bytes of fields
    /// 512 - 176 = 336 bytes padding
    _padding: [u8; 336],
}

// Compile-time size/alignment verification
const _: () = {
    assert!(core::mem::size_of::<TrojanManagerCapsule>() == 512);
    assert!(core::mem::align_of::<TrojanManagerCapsule>() == 512);
    assert!(core::mem::size_of::<TrojanRingHeader>() == 64);
    assert!(core::mem::align_of::<TrojanRingHeader>() == 64);
    assert!(core::mem::size_of::<TrojanCommand>() == 64);
};

impl TrojanManagerCapsule {
    /// Capsule size in bytes
    pub const SIZE: usize = 512;

    /// Default ring capacity (1024 commands = 64KB + 64B header)
    pub const DEFAULT_RING_CAPACITY: u64 = 1024;

    /// Default poll interval in nanoseconds
    pub const DEFAULT_POLL_INTERVAL_NS: u32 = 100;

    /// Maximum supported devices
    pub const MAX_DEVICES: u8 = 16;

    // ========================================================================
    // Construction
    // ========================================================================

    /// Create a new uninitialized Trojan Manager
    ///
    /// # Returns
    ///
    /// A new `TrojanManagerCapsule` in `Uninitialized` state.
    ///
    /// # Performance
    ///
    /// O(1), ~5ns (just zeroing memory)
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(state_pack::pack(
                TrojanManagerState::Uninitialized as u8,
                0,
                0,
                0,
            )),
            generation: AtomicU64::new(0),
            cuda_context: AtomicU64::new(0),
            cuda_module: AtomicU64::new(0),
            cuda_function: AtomicU64::new(0),
            cuda_stream: AtomicU64::new(0),
            ring_host_ptr: AtomicU64::new(0),
            ring_device_ptr: AtomicU64::new(0),
            ring_size: AtomicU64::new(0),
            ring_capacity: AtomicU64::new(0),
            commands_submitted: AtomicU64::new(0),
            commands_completed: AtomicU64::new(0),
            last_submit_ns: AtomicU64::new(0),
            last_complete_ns: AtomicU64::new(0),
            avg_latency_ns: AtomicU64::new(0),
            max_latency_ns: AtomicU64::new(0),
            min_latency_ns: AtomicU64::new(u64::MAX), // Initialize to max for min tracking
            total_latency_ns: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            last_error: AtomicU64::new(0),
            current_seqno: AtomicU64::new(1), // Start at 1 (0 = invalid)
            completed_seqno: AtomicU64::new(0),
            _padding: [0; 336],
        }
    }

    // ========================================================================
    // State Accessors
    // ========================================================================

    /// Get current manager state
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn state(&self) -> TrojanManagerState {
        let packed = self.state.load(Ordering::Acquire);
        let state_u8 = state_pack::unpack_state(packed);
        TrojanManagerState::from_u8(state_u8).unwrap_or(TrojanManagerState::Error)
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

    /// Get device ordinal
    #[inline]
    pub fn device_ordinal(&self) -> u8 {
        let packed = self.state.load(Ordering::Acquire);
        state_pack::unpack_device(packed)
    }

    /// Get SM count
    #[inline]
    pub fn sm_count(&self) -> u16 {
        let packed = self.state.load(Ordering::Acquire);
        state_pack::unpack_sm_count(packed)
    }

    /// Check if kernel is currently running
    #[inline]
    pub fn is_running(&self) -> bool {
        self.state().is_kernel_running()
    }

    /// Check if manager is fully operational
    #[inline]
    pub fn is_operational(&self) -> bool {
        self.state().is_operational()
    }

    /// Get ring capacity
    #[inline]
    pub fn ring_capacity(&self) -> u64 {
        self.ring_capacity.load(Ordering::Acquire)
    }

    /// Get commands submitted count
    #[inline]
    pub fn commands_submitted(&self) -> u64 {
        self.commands_submitted.load(Ordering::Acquire)
    }

    /// Get commands completed count
    #[inline]
    pub fn commands_completed(&self) -> u64 {
        self.commands_completed.load(Ordering::Acquire)
    }

    /// Get pending command count
    #[inline]
    pub fn pending_commands(&self) -> u64 {
        let submitted = self.commands_submitted.load(Ordering::Acquire);
        let completed = self.commands_completed.load(Ordering::Acquire);
        submitted.saturating_sub(completed)
    }

    /// Get average latency in nanoseconds (Q16.16 fixed-point)
    #[inline]
    pub fn avg_latency_ns(&self) -> u64 {
        let avg_fixed = self.avg_latency_ns.load(Ordering::Acquire);
        // Convert from Q16.16 to integer nanoseconds
        avg_fixed >> 16
    }

    /// Get maximum latency in nanoseconds
    #[inline]
    pub fn max_latency_ns(&self) -> u64 {
        self.max_latency_ns.load(Ordering::Acquire)
    }

    /// Get minimum latency in nanoseconds
    #[inline]
    pub fn min_latency_ns(&self) -> u64 {
        let min = self.min_latency_ns.load(Ordering::Acquire);
        if min == u64::MAX {
            0
        } else {
            min
        }
    }

    /// Get error count
    #[inline]
    pub fn error_count(&self) -> u64 {
        self.error_count.load(Ordering::Acquire)
    }

    /// Get last error
    #[inline]
    pub fn last_error(&self) -> Option<KgpuDriverError> {
        let code = self.last_error.load(Ordering::Acquire) as u32;
        if code == 0 {
            None
        } else {
            Some(KgpuDriverError::from_code(code))
        }
    }

    // ========================================================================
    // State Transitions (Lockfree CAS)
    // ========================================================================

    /// Transition to next state atomically
    ///
    /// Uses CAS loop to ensure atomic state transition with generation increment.
    ///
    /// # Arguments
    ///
    /// * `expected` - Expected current state
    /// * `new_state` - New state to transition to
    ///
    /// # Returns
    ///
    /// * `Ok(generation)` - New generation counter after successful transition
    /// * `Err(InvalidState)` - Current state doesn't match expected
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_STATE_VALID`: Expected state is a valid TrojanManagerState
    fn transition_state(
        &self,
        expected: TrojanManagerState,
        new_state: TrojanManagerState,
    ) -> KgpuDriverResult<u64> {
        loop {
            let packed = self.state.load(Ordering::Acquire);
            let current_state_u8 = state_pack::unpack_state(packed);
            let current_state =
                TrojanManagerState::from_u8(current_state_u8).unwrap_or(TrojanManagerState::Error);

            if current_state != expected {
                return Err(KgpuDriverError::InvalidState);
            }

            // Preserve device/SM/flags, update state
            let new_packed = state_pack::with_state(packed, new_state as u8);

            // Increment generation
            let new_gen = self.generation.load(Ordering::Acquire).wrapping_add(1);

            // Try CAS on state
            match self.state.compare_exchange_weak(
                packed,
                new_packed,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Update generation
                    self.generation.store(new_gen, Ordering::Release);
                    return Ok(new_gen);
                }
                Err(_) => {
                    core::hint::spin_loop();
                    continue;
                }
            }
        }
    }

    /// Initialize CUDA and create context
    ///
    /// Transitions: Uninitialized -> CudaInitialized -> ContextCreated
    ///
    /// # Arguments
    ///
    /// * `device_ordinal` - CUDA device index (0-15)
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success
    /// * `Err(InvalidState)` if not in Uninitialized state
    /// * `Err(InvalidDeviceIndex)` if device_ordinal > 15
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_CUDA_DRIVER_LOADED`: CUDA driver API is available
    /// - `#VERIFY_CUDA_DRIVER_LOADED`: Verified by cuInit() success
    pub fn initialize(&self, device_ordinal: i32) -> KgpuDriverResult<()> {
        if device_ordinal < 0 || device_ordinal >= Self::MAX_DEVICES as i32 {
            return Err(KgpuDriverError::InvalidDeviceIndex);
        }

        // Transition Uninitialized -> CudaInitialized
        self.transition_state(
            TrojanManagerState::Uninitialized,
            TrojanManagerState::CudaInitialized,
        )?;

        // Update device ordinal in state
        loop {
            let packed = self.state.load(Ordering::Acquire);
            let new_packed = (packed & !state_pack::DEVICE_MASK)
                | ((device_ordinal as u64) << state_pack::DEVICE_SHIFT);

            if self
                .state
                .compare_exchange_weak(packed, new_packed, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }

        // In a real implementation, this would call:
        // - cuInit(0)
        // - cuDeviceGet(&device, device_ordinal)
        // - cuCtxCreate(&context, 0, device)
        // - cuDeviceGetAttribute(&sm_count, CU_DEVICE_ATTRIBUTE_MULTIPROCESSOR_COUNT, device)
        // For now, we simulate success

        // Transition CudaInitialized -> ContextCreated
        self.transition_state(
            TrojanManagerState::CudaInitialized,
            TrojanManagerState::ContextCreated,
        )?;

        Ok(())
    }

    /// Load PTX module containing trojan_poll kernel
    ///
    /// Transitions: ContextCreated -> ModuleLoaded -> KernelReady
    ///
    /// # Arguments
    ///
    /// * `ptx_data` - PTX source code or cubin binary
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success
    /// * `Err(InvalidState)` if not in ContextCreated state
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_PTX_VALID`: PTX data is valid and contains trojan_poll kernel
    /// - `#VERIFY_PTX_VALID`: Verified by cuModuleLoadData() success
    pub fn load_module(&self, _ptx_data: &[u8]) -> KgpuDriverResult<()> {
        // Transition ContextCreated -> ModuleLoaded
        self.transition_state(
            TrojanManagerState::ContextCreated,
            TrojanManagerState::ModuleLoaded,
        )?;

        // In a real implementation, this would call:
        // - cuModuleLoadData(&module, ptx_data)
        // - cuModuleGetFunction(&function, module, "trojan_poll")
        // For now, we simulate success

        // Transition ModuleLoaded -> KernelReady
        self.transition_state(
            TrojanManagerState::ModuleLoaded,
            TrojanManagerState::KernelReady,
        )?;

        Ok(())
    }

    /// Allocate pinned ring buffer
    ///
    /// Transitions: KernelReady -> RingAllocated
    ///
    /// # Arguments
    ///
    /// * `capacity` - Number of command slots (power of 2, default 1024)
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success
    /// * `Err(InvalidState)` if not in KernelReady state
    /// * `Err(InvalidParameter)` if capacity is not power of 2
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_HOST_MEMORY_AVAILABLE`: Sufficient pinned memory available
    /// - `#VERIFY_HOST_MEMORY_AVAILABLE`: Verified by cudaHostAlloc() success
    pub fn allocate_ring(&self, capacity: usize) -> KgpuDriverResult<()> {
        // Validate capacity is power of 2
        if capacity == 0 || (capacity & (capacity - 1)) != 0 {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // Transition KernelReady -> RingAllocated
        self.transition_state(
            TrojanManagerState::KernelReady,
            TrojanManagerState::RingAllocated,
        )?;

        // Calculate ring buffer size: header + commands
        let ring_size = TrojanRingHeader::SIZE + (capacity * TrojanCommand::SIZE);

        // Store ring parameters
        self.ring_capacity.store(capacity as u64, Ordering::Release);
        self.ring_size.store(ring_size as u64, Ordering::Release);

        // In a real implementation, this would call:
        // - cudaHostAlloc(&ring_host_ptr, ring_size, cudaHostAllocMapped)
        // - cudaHostGetDevicePointer(&ring_device_ptr, ring_host_ptr, 0)
        // - Initialize TrojanRingHeader at ring_host_ptr
        // For now, we simulate with dummy addresses

        // Store simulated addresses (would be real pointers in actual impl)
        // Using hex patterns that spell out "DEAD BEEF" and "CAFE BEEF" for debugging
        self.ring_host_ptr.store(0xDEAD_BEEF_0000, Ordering::Release);
        self.ring_device_ptr
            .store(0xCAFE_BEEF_0000, Ordering::Release);

        Ok(())
    }

    /// Launch the Trojan Kernel (runs forever until stopped)
    ///
    /// Transitions: RingAllocated -> KernelLaunched
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success
    /// * `Err(InvalidState)` if not in RingAllocated state
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_RING_VALID`: Ring buffer is properly allocated and initialized
    /// - `#VERIFY_RING_VALID`: Verified by successful allocation in allocate_ring()
    pub fn launch_trojan(&self) -> KgpuDriverResult<()> {
        // Transition RingAllocated -> KernelLaunched
        self.transition_state(
            TrojanManagerState::RingAllocated,
            TrojanManagerState::KernelLaunched,
        )?;

        // Set kernel running flag
        loop {
            let packed = self.state.load(Ordering::Acquire);
            let new_packed = packed | state_pack::FLAG_KERNEL_RUNNING | state_pack::FLAG_RING_VALID;

            if self
                .state
                .compare_exchange_weak(packed, new_packed, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }

        // In a real implementation, this would call:
        // - Build TrojanKernelArgs
        // - cuLaunchKernel(function, 1, 1, 1, 1, 1, 1, 0, stream, args, 0)
        // The kernel runs in an infinite loop until stop_flag is set

        Ok(())
    }

    /// Stop the Trojan Kernel gracefully
    ///
    /// Transitions: KernelLaunched -> Stopping -> Uninitialized
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success
    /// * `Err(InvalidState)` if kernel is not running
    pub fn stop_trojan(&self) -> KgpuDriverResult<()> {
        let state = self.state();
        if !state.is_kernel_running() {
            return Ok(()); // Already stopped
        }

        // Transition KernelLaunched -> Stopping
        if state == TrojanManagerState::KernelLaunched {
            self.transition_state(
                TrojanManagerState::KernelLaunched,
                TrojanManagerState::Stopping,
            )?;
        }

        // Set shutdown request flag
        loop {
            let packed = self.state.load(Ordering::Acquire);
            let new_packed = packed | state_pack::FLAG_SHUTDOWN_REQUESTED;

            if self
                .state
                .compare_exchange_weak(packed, new_packed, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }

        // In a real implementation, this would:
        // 1. Get ring header pointer
        // 2. Set stop_flag in header
        // 3. cuStreamSynchronize(stream) to wait for kernel exit
        // 4. cudaFreeHost(ring_host_ptr)
        // 5. cuModuleUnload(module)
        // 6. cuCtxDestroy(context)

        // Transition Stopping -> Uninitialized
        self.transition_state(
            TrojanManagerState::Stopping,
            TrojanManagerState::Uninitialized,
        )?;

        // Clear all handles
        self.cuda_context.store(0, Ordering::Release);
        self.cuda_module.store(0, Ordering::Release);
        self.cuda_function.store(0, Ordering::Release);
        self.cuda_stream.store(0, Ordering::Release);
        self.ring_host_ptr.store(0, Ordering::Release);
        self.ring_device_ptr.store(0, Ordering::Release);
        self.ring_size.store(0, Ordering::Release);
        self.ring_capacity.store(0, Ordering::Release);

        // Clear flags
        loop {
            let packed = self.state.load(Ordering::Acquire);
            let new_packed = packed
                & !(state_pack::FLAG_KERNEL_RUNNING
                    | state_pack::FLAG_RING_VALID
                    | state_pack::FLAG_SHUTDOWN_REQUESTED);

            if self
                .state
                .compare_exchange_weak(packed, new_packed, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
            core::hint::spin_loop();
        }

        Ok(())
    }

    // ========================================================================
    // Ring Buffer Access
    // ========================================================================

    /// Get ring buffer header (if allocated)
    ///
    /// # Safety
    ///
    /// The returned reference is only valid while the ring buffer is allocated.
    /// Caller must ensure the manager is in an appropriate state.
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_RING_ALLOCATED`: Ring buffer has been allocated
    /// - `#VERIFY_RING_ALLOCATED`: Verified by checking state
    #[inline]
    pub fn get_ring_header(&self) -> KgpuDriverResult<*const TrojanRingHeader> {
        if !self.state().has_ring() {
            return Err(KgpuDriverError::InvalidState);
        }

        let ptr = self.ring_host_ptr.load(Ordering::Acquire);
        if ptr == 0 {
            return Err(KgpuDriverError::InvalidMemoryHandle);
        }

        // #ASSUME_PINNED_MEMORY: ptr points to valid cudaHostAlloc'd memory
        // #VERIFY_PINNED_MEMORY: Verified by successful allocate_ring()
        Ok(ptr as *const TrojanRingHeader)
    }

    /// Allocate a sequence number for a new command
    #[inline]
    fn allocate_seqno(&self) -> u64 {
        self.current_seqno.fetch_add(1, Ordering::AcqRel)
    }

    /// Submit a command to the ring buffer
    ///
    /// # Arguments
    ///
    /// * `cmd` - Command to submit
    ///
    /// # Returns
    ///
    /// * `Ok(seqno)` - Sequence number of submitted command
    /// * `Err(InvalidState)` - Manager not operational
    /// * `Err(RingBufferFull)` - Ring buffer is full
    ///
    /// # Performance
    ///
    /// <100ns typical (single CAS + memory write)
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_KERNEL_RUNNING`: Trojan kernel is actively polling
    /// - `#VERIFY_KERNEL_RUNNING`: Verified by checking state
    pub fn submit_command(&self, cmd: &TrojanCommand) -> KgpuDriverResult<u64> {
        if !self.is_operational() {
            return Err(KgpuDriverError::InvalidState);
        }

        let capacity = self.ring_capacity.load(Ordering::Acquire);
        if capacity == 0 {
            return Err(KgpuDriverError::InvalidState);
        }

        // Get ring header
        let header_ptr = self.get_ring_header()?;

        // SAFETY: header_ptr verified to be valid pinned memory
        // #ASSUME_PINNED_MEMORY: header_ptr points to cudaHostAlloc'd memory
        let header = unsafe { &*header_ptr };

        // Check if ring is full
        let head = header.head.load(Ordering::Acquire);
        let tail = header.tail.load(Ordering::Acquire);
        let used = tail.wrapping_sub(head);

        if used >= capacity - 1 {
            return Err(KgpuDriverError::RingBufferFull);
        }

        // Calculate slot index
        let slot = (tail & (capacity - 1)) as usize;

        // Get pointer to command slot
        let ring_host = self.ring_host_ptr.load(Ordering::Acquire);
        let cmd_base = ring_host + TrojanRingHeader::SIZE as u64;
        let cmd_ptr = (cmd_base + (slot * TrojanCommand::SIZE) as u64) as *mut TrojanCommand;

        // Write command to slot
        // SAFETY: cmd_ptr is within allocated pinned memory
        // #ASSUME_SLOT_VALID: slot is within ring capacity
        unsafe {
            core::ptr::write_volatile(cmd_ptr, *cmd);
        }

        // Memory fence to ensure command is visible to GPU
        core::sync::atomic::fence(Ordering::Release);

        // Advance tail
        header.tail.fetch_add(1, Ordering::AcqRel);

        // Update metrics
        let seqno = self.allocate_seqno();
        self.commands_submitted.fetch_add(1, Ordering::Relaxed);

        Ok(seqno)
    }

    /// Submit a NOP command (useful for synchronization)
    pub fn submit_nop(&self) -> KgpuDriverResult<u64> {
        let seqno = self.current_seqno.load(Ordering::Acquire);
        let cmd = TrojanCommand::nop(seqno);
        self.submit_command(&cmd)
    }

    /// Submit a memory copy command
    pub fn submit_mem_copy(&self, src: u64, dst: u64, size: u64) -> KgpuDriverResult<u64> {
        let seqno = self.current_seqno.load(Ordering::Acquire);
        let cmd = TrojanCommand::mem_copy(seqno, src, dst, size);
        self.submit_command(&cmd)
    }

    /// Submit a memory set command
    pub fn submit_mem_set(&self, dst: u64, size: u64, pattern: u32) -> KgpuDriverResult<u64> {
        let seqno = self.current_seqno.load(Ordering::Acquire);
        let cmd = TrojanCommand::mem_set(seqno, dst, size, pattern);
        self.submit_command(&cmd)
    }

    /// Submit a synchronization command
    pub fn submit_sync(&self) -> KgpuDriverResult<u64> {
        let seqno = self.current_seqno.load(Ordering::Acquire);
        let cmd = TrojanCommand::sync(seqno);
        self.submit_command(&cmd)
    }

    /// Poll completion status
    ///
    /// Updates internal metrics from ring header.
    ///
    /// # Returns
    ///
    /// Number of newly completed commands
    pub fn poll_completion(&self) -> u64 {
        if !self.state().has_ring() {
            return 0;
        }

        let header_ptr = match self.get_ring_header() {
            Ok(ptr) => ptr,
            Err(_) => return 0,
        };

        // SAFETY: header_ptr verified valid
        let header = unsafe { &*header_ptr };

        let processed = header.commands_processed.load(Ordering::Acquire);
        let prev_completed = self.commands_completed.load(Ordering::Acquire);

        if processed > prev_completed {
            self.commands_completed.store(processed, Ordering::Release);

            // Update last complete timestamp
            let ts = header.last_timestamp.load(Ordering::Acquire);
            if ts > 0 {
                self.last_complete_ns.store(ts, Ordering::Release);
            }
        }

        processed.saturating_sub(prev_completed)
    }

    /// Wait for a specific sequence number to complete
    ///
    /// # Arguments
    ///
    /// * `seqno` - Sequence number to wait for
    /// * `timeout_ns` - Timeout in nanoseconds (0 = no timeout)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Command completed
    /// * `Err(FenceTimeout)` - Timeout expired
    pub fn wait_completion(&self, seqno: u64, timeout_ns: u64) -> KgpuDriverResult<()> {
        // Quick check
        if self.completed_seqno.load(Ordering::Acquire) >= seqno {
            return Ok(());
        }

        let mut elapsed: u64 = 0;
        let poll_interval_ns: u64 = 100;

        loop {
            self.poll_completion();

            if self.completed_seqno.load(Ordering::Acquire) >= seqno {
                return Ok(());
            }

            if timeout_ns > 0 && elapsed >= timeout_ns {
                return Err(KgpuDriverError::FenceTimeout);
            }

            // Spin wait
            for _ in 0..100 {
                core::hint::spin_loop();
            }

            elapsed += poll_interval_ns;
        }
    }

    /// Wait for all pending commands to complete
    pub fn wait_idle(&self, timeout_ns: u64) -> KgpuDriverResult<()> {
        let current = self.current_seqno.load(Ordering::Acquire);
        if current <= 1 {
            return Ok(()); // No commands submitted
        }

        self.wait_completion(current - 1, timeout_ns)
    }

    // ========================================================================
    // Metrics
    // ========================================================================

    /// Record a latency measurement
    ///
    /// Updates min/max/avg latency metrics atomically.
    ///
    /// # Arguments
    ///
    /// * `latency_ns` - Latency in nanoseconds
    pub fn record_latency(&self, latency_ns: u64) {
        // Update min latency
        loop {
            let min = self.min_latency_ns.load(Ordering::Acquire);
            if latency_ns >= min {
                break;
            }
            if self
                .min_latency_ns
                .compare_exchange_weak(min, latency_ns, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }

        // Update max latency
        loop {
            let max = self.max_latency_ns.load(Ordering::Acquire);
            if latency_ns <= max {
                break;
            }
            if self
                .max_latency_ns
                .compare_exchange_weak(max, latency_ns, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }

        // Update total latency for average calculation
        self.total_latency_ns.fetch_add(latency_ns, Ordering::Relaxed);

        // Update average (Q16.16 exponential moving average)
        // new_avg = old_avg * 0.9 + new_sample * 0.1
        // In Q16.16: multiply by 0.9 ≈ 58982/65536, 0.1 ≈ 6554/65536
        let sample_q16 = latency_ns << 16;
        loop {
            let old_avg = self.avg_latency_ns.load(Ordering::Acquire);
            let weighted_old = (old_avg * 58982) >> 16;
            let weighted_new = (sample_q16 * 6554) >> 16;
            let new_avg = weighted_old + weighted_new;

            if self
                .avg_latency_ns
                .compare_exchange_weak(old_avg, new_avg, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }
    }

    /// Record an error
    pub fn record_error(&self, error: KgpuDriverError) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
        self.last_error.store(error.code() as u64, Ordering::Release);
    }

    // ========================================================================
    // Snapshot
    // ========================================================================

    /// Take an atomic snapshot of manager state
    ///
    /// # Performance
    ///
    /// <50ns (multiple atomic loads)
    #[inline]
    pub fn snapshot(&self) -> TrojanManagerSnapshot {
        let state_packed = self.state.load(Ordering::Acquire);

        TrojanManagerSnapshot {
            state: TrojanManagerState::from_u8(state_pack::unpack_state(state_packed))
                .unwrap_or(TrojanManagerState::Error),
            generation: self.generation.load(Ordering::Acquire),
            device_ordinal: state_pack::unpack_device(state_packed),
            sm_count: state_pack::unpack_sm_count(state_packed),
            ring_capacity: self.ring_capacity.load(Ordering::Acquire),
            commands_submitted: self.commands_submitted.load(Ordering::Acquire),
            commands_completed: self.commands_completed.load(Ordering::Acquire),
            avg_latency_ns: self.avg_latency_ns() as u32,
            max_latency_ns: self.max_latency_ns.load(Ordering::Acquire) as u32,
            min_latency_ns: self.min_latency_ns() as u32,
            error_count: self.error_count.load(Ordering::Acquire),
            last_error: self.last_error.load(Ordering::Acquire) as u32,
        }
    }
}

impl Default for TrojanManagerCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for TrojanManagerCapsule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let snap = self.snapshot();
        f.debug_struct("TrojanManagerCapsule")
            .field("state", &snap.state)
            .field("generation", &snap.generation)
            .field("device_ordinal", &snap.device_ordinal)
            .field("sm_count", &snap.sm_count)
            .field("ring_capacity", &snap.ring_capacity)
            .field("commands_submitted", &snap.commands_submitted)
            .field("commands_completed", &snap.commands_completed)
            .field("avg_latency_ns", &snap.avg_latency_ns)
            .field("error_count", &snap.error_count)
            .finish()
    }
}

// Safety: All fields are AtomicU64 (Send + Sync)
unsafe impl Send for TrojanManagerCapsule {}
unsafe impl Sync for TrojanManagerCapsule {}

// ============================================================================
// Snapshot
// ============================================================================

/// Snapshot of TrojanManagerCapsule state
#[derive(Debug, Clone, Copy)]
pub struct TrojanManagerSnapshot {
    /// Current state
    pub state: TrojanManagerState,
    /// Generation counter
    pub generation: u64,
    /// CUDA device ordinal
    pub device_ordinal: u8,
    /// GPU SM count
    pub sm_count: u16,
    /// Ring buffer capacity
    pub ring_capacity: u64,
    /// Total commands submitted
    pub commands_submitted: u64,
    /// Total commands completed
    pub commands_completed: u64,
    /// Average latency in nanoseconds
    pub avg_latency_ns: u32,
    /// Maximum latency in nanoseconds
    pub max_latency_ns: u32,
    /// Minimum latency in nanoseconds
    pub min_latency_ns: u32,
    /// Total error count
    pub error_count: u64,
    /// Last error code
    pub last_error: u32,
}

impl TrojanManagerSnapshot {
    /// Get pending command count
    #[inline]
    pub const fn pending_commands(&self) -> u64 {
        self.commands_submitted
            .saturating_sub(self.commands_completed)
    }

    /// Check if operational
    #[inline]
    pub const fn is_operational(&self) -> bool {
        matches!(self.state, TrojanManagerState::KernelLaunched)
    }

    /// Get throughput estimate (commands per second)
    #[inline]
    pub fn throughput_cps(&self) -> u64 {
        if self.avg_latency_ns == 0 {
            return 0;
        }
        1_000_000_000 / (self.avg_latency_ns as u64)
    }
}

impl Default for TrojanManagerSnapshot {
    fn default() -> Self {
        Self {
            state: TrojanManagerState::Uninitialized,
            generation: 0,
            device_ordinal: 0,
            sm_count: 0,
            ring_capacity: 0,
            commands_submitted: 0,
            commands_completed: 0,
            avg_latency_ns: 0,
            max_latency_ns: 0,
            min_latency_ns: 0,
            error_count: 0,
            last_error: 0,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem;

    // ========================================================================
    // Q1-Q7: Unit Tests (Size, Alignment, Initial State)
    // ========================================================================

    #[test]
    fn test_manager_size() {
        assert_eq!(
            mem::size_of::<TrojanManagerCapsule>(),
            512,
            "TrojanManagerCapsule must be exactly 512 bytes"
        );
    }

    #[test]
    fn test_manager_alignment() {
        assert_eq!(
            mem::align_of::<TrojanManagerCapsule>(),
            512,
            "TrojanManagerCapsule must be 512-byte aligned"
        );
    }

    #[test]
    fn test_header_size() {
        assert_eq!(
            mem::size_of::<TrojanRingHeader>(),
            64,
            "TrojanRingHeader must be exactly 64 bytes"
        );
    }

    #[test]
    fn test_header_alignment() {
        assert_eq!(
            mem::align_of::<TrojanRingHeader>(),
            64,
            "TrojanRingHeader must be 64-byte aligned"
        );
    }

    #[test]
    fn test_kernel_args_size() {
        assert!(
            mem::size_of::<TrojanKernelArgs>() <= 64,
            "TrojanKernelArgs should fit in a cache line"
        );
    }

    #[test]
    fn test_initial_state() {
        let manager = TrojanManagerCapsule::new();
        assert_eq!(manager.state(), TrojanManagerState::Uninitialized);
        assert_eq!(manager.generation(), 0);
        assert_eq!(manager.device_ordinal(), 0);
        assert_eq!(manager.sm_count(), 0);
        assert!(!manager.is_running());
        assert!(!manager.is_operational());
    }

    #[test]
    fn test_default_impl() {
        let manager: TrojanManagerCapsule = Default::default();
        assert_eq!(manager.state(), TrojanManagerState::Uninitialized);
    }

    // ========================================================================
    // Q8-Q14: State Machine Tests
    // ========================================================================

    #[test]
    fn test_state_from_u8() {
        assert_eq!(
            TrojanManagerState::from_u8(0),
            Some(TrojanManagerState::Uninitialized)
        );
        assert_eq!(
            TrojanManagerState::from_u8(1),
            Some(TrojanManagerState::CudaInitialized)
        );
        assert_eq!(
            TrojanManagerState::from_u8(2),
            Some(TrojanManagerState::ContextCreated)
        );
        assert_eq!(
            TrojanManagerState::from_u8(3),
            Some(TrojanManagerState::ModuleLoaded)
        );
        assert_eq!(
            TrojanManagerState::from_u8(4),
            Some(TrojanManagerState::KernelReady)
        );
        assert_eq!(
            TrojanManagerState::from_u8(5),
            Some(TrojanManagerState::RingAllocated)
        );
        assert_eq!(
            TrojanManagerState::from_u8(6),
            Some(TrojanManagerState::KernelLaunched)
        );
        assert_eq!(
            TrojanManagerState::from_u8(7),
            Some(TrojanManagerState::Stopping)
        );
        assert_eq!(
            TrojanManagerState::from_u8(8),
            Some(TrojanManagerState::Error)
        );
        assert_eq!(TrojanManagerState::from_u8(9), None);
        assert_eq!(TrojanManagerState::from_u8(255), None);
    }

    #[test]
    fn test_state_is_operational() {
        assert!(!TrojanManagerState::Uninitialized.is_operational());
        assert!(!TrojanManagerState::CudaInitialized.is_operational());
        assert!(!TrojanManagerState::ContextCreated.is_operational());
        assert!(!TrojanManagerState::ModuleLoaded.is_operational());
        assert!(!TrojanManagerState::KernelReady.is_operational());
        assert!(!TrojanManagerState::RingAllocated.is_operational());
        assert!(TrojanManagerState::KernelLaunched.is_operational());
        assert!(!TrojanManagerState::Stopping.is_operational());
        assert!(!TrojanManagerState::Error.is_operational());
    }

    #[test]
    fn test_state_is_kernel_running() {
        assert!(!TrojanManagerState::Uninitialized.is_kernel_running());
        assert!(!TrojanManagerState::RingAllocated.is_kernel_running());
        assert!(TrojanManagerState::KernelLaunched.is_kernel_running());
        assert!(TrojanManagerState::Stopping.is_kernel_running());
        assert!(!TrojanManagerState::Error.is_kernel_running());
    }

    #[test]
    fn test_state_has_ring() {
        assert!(!TrojanManagerState::Uninitialized.has_ring());
        assert!(!TrojanManagerState::KernelReady.has_ring());
        assert!(TrojanManagerState::RingAllocated.has_ring());
        assert!(TrojanManagerState::KernelLaunched.has_ring());
        assert!(TrojanManagerState::Stopping.has_ring());
        assert!(!TrojanManagerState::Error.has_ring());
    }

    #[test]
    fn test_state_can_advance() {
        assert!(TrojanManagerState::Uninitialized.can_advance());
        assert!(TrojanManagerState::CudaInitialized.can_advance());
        assert!(TrojanManagerState::ContextCreated.can_advance());
        assert!(TrojanManagerState::ModuleLoaded.can_advance());
        assert!(TrojanManagerState::KernelReady.can_advance());
        assert!(TrojanManagerState::RingAllocated.can_advance());
        assert!(!TrojanManagerState::KernelLaunched.can_advance());
        assert!(!TrojanManagerState::Stopping.can_advance());
        assert!(!TrojanManagerState::Error.can_advance());
    }

    #[test]
    fn test_state_next_state() {
        assert_eq!(
            TrojanManagerState::Uninitialized.next_state(),
            Some(TrojanManagerState::CudaInitialized)
        );
        assert_eq!(
            TrojanManagerState::CudaInitialized.next_state(),
            Some(TrojanManagerState::ContextCreated)
        );
        assert_eq!(
            TrojanManagerState::KernelLaunched.next_state(),
            Some(TrojanManagerState::Stopping)
        );
        assert_eq!(
            TrojanManagerState::Stopping.next_state(),
            Some(TrojanManagerState::Uninitialized)
        );
        assert_eq!(TrojanManagerState::Error.next_state(), None);
    }

    // ========================================================================
    // Q15-Q21: Initialization Tests
    // ========================================================================

    #[test]
    fn test_initialize_success() {
        let manager = TrojanManagerCapsule::new();
        let result = manager.initialize(0);
        assert!(result.is_ok());
        assert_eq!(manager.state(), TrojanManagerState::ContextCreated);
        assert!(manager.generation() >= 2); // Two transitions
    }

    #[test]
    fn test_initialize_invalid_device() {
        let manager = TrojanManagerCapsule::new();

        // Negative device
        let result = manager.initialize(-1);
        assert_eq!(result, Err(KgpuDriverError::InvalidDeviceIndex));

        // Device too high
        let result = manager.initialize(16);
        assert_eq!(result, Err(KgpuDriverError::InvalidDeviceIndex));
    }

    #[test]
    fn test_initialize_already_initialized() {
        let manager = TrojanManagerCapsule::new();
        manager.initialize(0).unwrap();

        // Try to initialize again
        let result = manager.initialize(1);
        assert_eq!(result, Err(KgpuDriverError::InvalidState));
    }

    #[test]
    fn test_load_module_success() {
        let manager = TrojanManagerCapsule::new();
        manager.initialize(0).unwrap();

        let ptx_data = b"fake ptx data";
        let result = manager.load_module(ptx_data);
        assert!(result.is_ok());
        assert_eq!(manager.state(), TrojanManagerState::KernelReady);
    }

    #[test]
    fn test_load_module_invalid_state() {
        let manager = TrojanManagerCapsule::new();

        // Try to load without initializing
        let result = manager.load_module(b"ptx");
        assert_eq!(result, Err(KgpuDriverError::InvalidState));
    }

    #[test]
    fn test_allocate_ring_success() {
        let manager = TrojanManagerCapsule::new();
        manager.initialize(0).unwrap();
        manager.load_module(b"ptx").unwrap();

        let result = manager.allocate_ring(1024);
        assert!(result.is_ok());
        assert_eq!(manager.state(), TrojanManagerState::RingAllocated);
        assert_eq!(manager.ring_capacity(), 1024);
    }

    #[test]
    fn test_allocate_ring_invalid_capacity() {
        let manager = TrojanManagerCapsule::new();
        manager.initialize(0).unwrap();
        manager.load_module(b"ptx").unwrap();

        // Zero capacity
        let result = manager.allocate_ring(0);
        assert_eq!(result, Err(KgpuDriverError::InvalidParameter));

        // Non-power-of-2
        let result = manager.allocate_ring(1000);
        assert_eq!(result, Err(KgpuDriverError::InvalidParameter));
    }

    // ========================================================================
    // Q22-Q28: Kernel Launch/Stop Tests
    // ========================================================================

    #[test]
    fn test_launch_trojan_success() {
        let manager = TrojanManagerCapsule::new();
        manager.initialize(0).unwrap();
        manager.load_module(b"ptx").unwrap();
        manager.allocate_ring(1024).unwrap();

        let result = manager.launch_trojan();
        assert!(result.is_ok());
        assert_eq!(manager.state(), TrojanManagerState::KernelLaunched);
        assert!(manager.is_running());
        assert!(manager.is_operational());
    }

    #[test]
    fn test_launch_trojan_invalid_state() {
        let manager = TrojanManagerCapsule::new();

        // Try to launch without setup
        let result = manager.launch_trojan();
        assert_eq!(result, Err(KgpuDriverError::InvalidState));
    }

    #[test]
    fn test_stop_trojan_success() {
        let manager = TrojanManagerCapsule::new();
        manager.initialize(0).unwrap();
        manager.load_module(b"ptx").unwrap();
        manager.allocate_ring(1024).unwrap();
        manager.launch_trojan().unwrap();

        let result = manager.stop_trojan();
        assert!(result.is_ok());
        assert_eq!(manager.state(), TrojanManagerState::Uninitialized);
        assert!(!manager.is_running());
    }

    #[test]
    fn test_stop_trojan_already_stopped() {
        let manager = TrojanManagerCapsule::new();

        // Stop when not running should succeed
        let result = manager.stop_trojan();
        assert!(result.is_ok());
    }

    #[test]
    fn test_full_lifecycle() {
        let manager = TrojanManagerCapsule::new();

        // Full initialization sequence
        assert!(manager.initialize(0).is_ok());
        assert!(manager.load_module(b"ptx").is_ok());
        assert!(manager.allocate_ring(512).is_ok());
        assert!(manager.launch_trojan().is_ok());

        // Check operational
        assert!(manager.is_operational());
        assert_eq!(manager.ring_capacity(), 512);

        // Shutdown
        assert!(manager.stop_trojan().is_ok());
        assert_eq!(manager.state(), TrojanManagerState::Uninitialized);
        assert_eq!(manager.ring_capacity(), 0);
    }

    // ========================================================================
    // Q29-Q35: Metrics and Snapshot Tests
    // ========================================================================

    #[test]
    fn test_record_latency() {
        let manager = TrojanManagerCapsule::new();

        manager.record_latency(100);
        manager.record_latency(200);
        manager.record_latency(50);

        assert_eq!(manager.min_latency_ns(), 50);
        assert_eq!(manager.max_latency_ns(), 200);
        assert!(manager.avg_latency_ns() > 0);
    }

    #[test]
    fn test_record_error() {
        let manager = TrojanManagerCapsule::new();

        assert_eq!(manager.error_count(), 0);
        assert!(manager.last_error().is_none());

        manager.record_error(KgpuDriverError::DeviceLost);
        assert_eq!(manager.error_count(), 1);
        assert_eq!(manager.last_error(), Some(KgpuDriverError::DeviceLost));

        manager.record_error(KgpuDriverError::FenceTimeout);
        assert_eq!(manager.error_count(), 2);
        assert_eq!(manager.last_error(), Some(KgpuDriverError::FenceTimeout));
    }

    #[test]
    fn test_snapshot() {
        let manager = TrojanManagerCapsule::new();
        manager.initialize(0).unwrap();
        manager.load_module(b"ptx").unwrap();
        manager.allocate_ring(1024).unwrap();
        manager.launch_trojan().unwrap();

        let snap = manager.snapshot();
        assert_eq!(snap.state, TrojanManagerState::KernelLaunched);
        assert!(snap.generation >= 6); // Multiple transitions
        assert_eq!(snap.ring_capacity, 1024);
        assert!(snap.is_operational());
    }

    #[test]
    fn test_snapshot_throughput() {
        let snap = TrojanManagerSnapshot {
            state: TrojanManagerState::KernelLaunched,
            generation: 10,
            device_ordinal: 0,
            sm_count: 128,
            ring_capacity: 1024,
            commands_submitted: 1000,
            commands_completed: 900,
            avg_latency_ns: 100, // 100ns average
            max_latency_ns: 500,
            min_latency_ns: 50,
            error_count: 0,
            last_error: 0,
        };

        assert_eq!(snap.pending_commands(), 100);
        assert_eq!(snap.throughput_cps(), 10_000_000); // 10M commands/sec at 100ns
    }

    #[test]
    fn test_snapshot_default() {
        let snap: TrojanManagerSnapshot = Default::default();
        assert_eq!(snap.state, TrojanManagerState::Uninitialized);
        assert_eq!(snap.generation, 0);
        assert_eq!(snap.ring_capacity, 0);
        assert!(!snap.is_operational());
    }

    // ========================================================================
    // Header Tests
    // ========================================================================

    #[test]
    fn test_header_new() {
        let header = TrojanRingHeader::new();
        assert_eq!(header.head(), 0);
        assert_eq!(header.tail(), 0);
        assert!(!header.is_running());
        assert!(!header.is_stop_requested());
        assert_eq!(header.pending_count(), 0);
    }

    #[test]
    fn test_header_advance_tail() {
        let header = TrojanRingHeader::new();

        let old = header.advance_tail();
        assert_eq!(old, 0);
        assert_eq!(header.tail(), 1);

        let old = header.advance_tail();
        assert_eq!(old, 1);
        assert_eq!(header.tail(), 2);
    }

    #[test]
    fn test_header_request_stop() {
        let header = TrojanRingHeader::new();
        assert!(!header.is_stop_requested());

        header.request_stop();
        assert!(header.is_stop_requested());
    }

    #[test]
    fn test_header_pending_count() {
        let header = TrojanRingHeader::new();

        // Advance tail 5 times (CPU writes)
        for _ in 0..5 {
            header.advance_tail();
        }
        assert_eq!(header.pending_count(), 5);

        // Simulate GPU processing (advance head)
        header.head.store(3, Ordering::Release);
        assert_eq!(header.pending_count(), 2);
    }

    // ========================================================================
    // State Pack Tests
    // ========================================================================

    #[test]
    fn test_state_pack_roundtrip() {
        let packed = state_pack::pack(6, 2, 128, 0b101);
        assert_eq!(state_pack::unpack_state(packed), 6);
        assert_eq!(state_pack::unpack_device(packed), 2);
        assert_eq!(state_pack::unpack_sm_count(packed), 128);
        assert_eq!(state_pack::unpack_flags(packed), 0b101);
    }

    #[test]
    fn test_state_pack_max_values() {
        let packed = state_pack::pack(255, 255, 65535, 255);
        assert_eq!(state_pack::unpack_state(packed), 255);
        assert_eq!(state_pack::unpack_device(packed), 255);
        assert_eq!(state_pack::unpack_sm_count(packed), 65535);
        assert_eq!(state_pack::unpack_flags(packed), 255);
    }

    #[test]
    fn test_state_pack_with_state() {
        let packed = state_pack::pack(0, 1, 128, 0);
        let new_packed = state_pack::with_state(packed, 6);

        // State changed
        assert_eq!(state_pack::unpack_state(new_packed), 6);
        // Other fields preserved
        assert_eq!(state_pack::unpack_device(new_packed), 1);
        assert_eq!(state_pack::unpack_sm_count(new_packed), 128);
    }

    // ========================================================================
    // TrojanKernelArgs Tests
    // ========================================================================

    #[test]
    fn test_kernel_args_creation() {
        let args = TrojanKernelArgs::new(
            0x1000_0000,
            1024,
            0x1000_0008,
            0x1000_0010,
            0x1000_0018,
            200,
            0x1000_0020,
            0x1000_0028,
        );

        assert_eq!(args.ring_ptr, 0x1000_0000);
        assert_eq!(args.ring_size, 1024);
        assert_eq!(args.poll_interval_ns, 200);
    }

    #[test]
    fn test_kernel_args_default() {
        let args = TrojanKernelArgs::default();
        assert_eq!(args.ring_ptr, 0);
        assert_eq!(args.ring_size, 0);
        assert_eq!(args.poll_interval_ns, 100); // Default 100ns
    }

    // ========================================================================
    // Display/Debug Tests
    // ========================================================================

    #[test]
    fn test_state_display() {
        assert_eq!(format!("{}", TrojanManagerState::Uninitialized), "UNINITIALIZED");
        assert_eq!(format!("{}", TrojanManagerState::KernelLaunched), "KERNEL_LAUNCHED");
        assert_eq!(format!("{}", TrojanManagerState::Error), "ERROR");
    }

    #[test]
    fn test_manager_debug() {
        let manager = TrojanManagerCapsule::new();
        let debug_str = format!("{:?}", manager);
        assert!(debug_str.contains("TrojanManagerCapsule"));
        assert!(debug_str.contains("Uninitialized"));
    }

    #[test]
    fn test_header_debug() {
        let header = TrojanRingHeader::new();
        let debug_str = format!("{:?}", header);
        assert!(debug_str.contains("TrojanRingHeader"));
    }

    // ========================================================================
    // Constants Tests
    // ========================================================================

    #[test]
    fn test_constants() {
        assert_eq!(TrojanManagerCapsule::SIZE, 512);
        assert_eq!(TrojanManagerCapsule::DEFAULT_RING_CAPACITY, 1024);
        assert_eq!(TrojanManagerCapsule::DEFAULT_POLL_INTERVAL_NS, 100);
        assert_eq!(TrojanManagerCapsule::MAX_DEVICES, 16);
        assert_eq!(TrojanRingHeader::SIZE, 64);
    }

    // ========================================================================
    // Send/Sync Tests
    // ========================================================================

    #[test]
    fn test_send_sync_traits() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TrojanManagerCapsule>();
    }

    // ========================================================================
    // Generation Counter Tests
    // ========================================================================

    #[test]
    fn test_generation_increments() {
        let manager = TrojanManagerCapsule::new();
        assert_eq!(manager.generation(), 0);

        manager.initialize(0).unwrap();
        assert!(manager.generation() >= 2);

        manager.load_module(b"ptx").unwrap();
        assert!(manager.generation() >= 4);

        manager.allocate_ring(1024).unwrap();
        assert!(manager.generation() >= 5);

        manager.launch_trojan().unwrap();
        assert!(manager.generation() >= 6);
    }

    // ========================================================================
    // Ring Access Tests
    // ========================================================================

    #[test]
    fn test_get_ring_header_no_ring() {
        let manager = TrojanManagerCapsule::new();
        let result = manager.get_ring_header();
        assert_eq!(result, Err(KgpuDriverError::InvalidState));
    }

    // ========================================================================
    // Metric Edge Cases
    // ========================================================================

    #[test]
    fn test_min_latency_initial() {
        let manager = TrojanManagerCapsule::new();
        // Initial min should be 0 (translated from u64::MAX)
        assert_eq!(manager.min_latency_ns(), 0);
    }

    #[test]
    fn test_pending_commands_calculation() {
        let manager = TrojanManagerCapsule::new();
        assert_eq!(manager.pending_commands(), 0);

        // Manually set metrics for testing
        manager.commands_submitted.store(100, Ordering::Relaxed);
        manager.commands_completed.store(75, Ordering::Relaxed);
        assert_eq!(manager.pending_commands(), 25);
    }
}
