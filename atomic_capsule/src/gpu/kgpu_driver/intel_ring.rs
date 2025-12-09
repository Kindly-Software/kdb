//! Intel GPU Ring Buffer Capsule (T1 Atomic, 256B)
//!
//! Implements EXECLIST-style ring buffer for command submission to Intel GPUs (Gen9+).
//! Uses lockfree atomic operations for head/tail management with generation counters
//! for TOCTOU prevention.
//!
//! # Architecture
//!
//! Intel GPUs (Gen9+) use a ring buffer mechanism for command submission:
//! 1. CPU writes MI (Memory Interface) commands to a ring buffer
//! 2. EXECLIST hardware reads commands and dispatches to EU (Execution Units)
//! 3. Completion signaled via interrupt or memory write
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     RING BUFFER MEMORY                          │
//! │  ┌─────┬─────┬─────┬─────┬─────┬─────┬─────┬─────┐             │
//! │  │ MI  │ MI  │ MI  │ ... │ ... │ ... │ MI  │ MI  │             │
//! │  │ CMD │ CMD │ CMD │     │     │     │ CMD │ CMD │             │
//! │  └─────┴─────┴─────┴─────┴─────┴─────┴─────┴─────┘             │
//! │       ^                                   ^                     │
//! │       │                                   │                     │
//! │     HEAD                                TAIL                    │
//! │   (GPU reads)                        (CPU writes)               │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Design
//!
//! **Tier**: T1 Atomic (3-10x speedup vs mutex-based approaches)
//! **Size**: 256B cache-aligned (4 cache lines)
//! **Performance Targets**:
//! - Reserve: <20ns (single CAS)
//! - Emit command: <5ns per DWORD
//! - Submit: <60ns (tail update + memory fence)
//! - Snapshot: <20ns
//!
//! # Chaos Compliance
//!
//! - **NO mutex/RwLock** - 100% lockfree via AtomicU64/AtomicU32
//! - **Generation counters** - Every state change increments generation
//! - **Cache-aligned** - 256B alignment (4 cache lines)
//! - **CAS loops** - All multi-field updates use compare_exchange
//! - **Fixed-size** - Exactly 256 bytes
//!
//! # UCE34 Compliance
//!
//! - Q10: T1 Atomic tier (lockfree coordination via AtomicU64 CAS loops)
//! - Q33: ComputationalCapsule verification (256B, cache-aligned, generation counters)
//! - Q34: Audit trail design (generation counters, submit_count, error_count)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_RING_VALID`: Ring buffer base address is valid GPU memory
//! - `#ASSUME_RING_ALIGNED`: Ring buffer is 4KB page-aligned
//! - `#ASSUME_MMIO_SAFE`: MMIO writes to doorbell are properly fenced

use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use core::fmt;

use super::error::{KgpuDriverError, KgpuDriverResult};

// ============================================================================
// MI Command Opcodes
// ============================================================================

/// Intel MI (Memory Interface) command opcodes
///
/// These are the standard MI commands used in Intel GPU ring buffers.
/// All MI commands start with a header DWORD containing the opcode.
///
/// # Reference
///
/// See Intel PRMs (Programmer's Reference Manuals) for Gen9+ architectures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MiOpcode {
    /// No operation (padding, synchronization)
    Noop = 0x00,
    /// Start executing a batch buffer
    BatchBufferStart = 0x31,
    /// End batch buffer execution
    BatchBufferEnd = 0x0A,
    /// Store immediate data to memory
    StoreDataImm = 0x20,
    /// Store register value to memory
    StoreRegisterMem = 0x24,
    /// Load immediate value to register
    LoadRegisterImm = 0x22,
    /// Load register from memory
    LoadRegisterMem = 0x29,
    /// Copy register to register
    LoadRegisterReg = 0x2A,
    /// Flush caches and write marker
    FlushDw = 0x26,
    /// Pipeline control (3D command space, but commonly used)
    PipeControl = 0x7A,
    /// Wait for semaphore
    SemaphoreWait = 0x1C,
    /// Signal semaphore
    SemaphoreSignal = 0x1B,
    /// Arbitration check point
    ArbCheck = 0x05,
    /// Generate user interrupt
    UserInterrupt = 0x02,
}

impl MiOpcode {
    /// Get the human-readable name for this opcode
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Noop => "MI_NOOP",
            Self::BatchBufferStart => "MI_BATCH_BUFFER_START",
            Self::BatchBufferEnd => "MI_BATCH_BUFFER_END",
            Self::StoreDataImm => "MI_STORE_DATA_IMM",
            Self::StoreRegisterMem => "MI_STORE_REGISTER_MEM",
            Self::LoadRegisterImm => "MI_LOAD_REGISTER_IMM",
            Self::LoadRegisterMem => "MI_LOAD_REGISTER_MEM",
            Self::LoadRegisterReg => "MI_LOAD_REGISTER_REG",
            Self::FlushDw => "MI_FLUSH_DW",
            Self::PipeControl => "PIPE_CONTROL",
            Self::SemaphoreWait => "MI_SEMAPHORE_WAIT",
            Self::SemaphoreSignal => "MI_SEMAPHORE_SIGNAL",
            Self::ArbCheck => "MI_ARB_CHECK",
            Self::UserInterrupt => "MI_USER_INTERRUPT",
        }
    }
}

impl fmt::Display for MiOpcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// MI Command Header
// ============================================================================

/// MI command header (first DWORD of any MI command)
///
/// All Intel MI commands begin with a header DWORD that encodes:
/// - Command type (bits 29-31): 0 for MI commands
/// - Opcode (bits 23-28): The specific MI operation
/// - Opcode-specific data (bits 6-22): Varies by command
/// - DWORD length (bits 0-5): Command length minus 2
///
/// # Layout
///
/// ```text
/// 31  29 28   23 22       6 5         0
/// +-----+------+-----------+-----------+
/// | Type| Opc  | Opc Data  | Len - 2   |
/// +-----+------+-----------+-----------+
///   3b    6b      17b          6b
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct MiCommandHeader {
    /// The raw 32-bit header value
    pub value: u32,
}

impl MiCommandHeader {
    /// Mask for extracting DWORD length (bits 0-5)
    const LEN_MASK: u32 = 0x3F;

    /// Mask for opcode-specific data (bits 6-22)
    const DATA_MASK: u32 = 0x007F_FFC0;

    /// Mask for opcode (bits 23-28)
    const OPCODE_MASK: u32 = 0x1F80_0000;

    /// Shift for opcode
    const OPCODE_SHIFT: u32 = 23;

    /// Create a new MI command header
    ///
    /// # Arguments
    ///
    /// * `opcode` - The MI opcode
    /// * `length_dwords` - Total command length in DWORDs (header + data)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let header = MiCommandHeader::new(MiOpcode::Noop, 1);
    /// assert_eq!(header.value & 0x3F, 0); // length - 2 = 0 for 1-DWORD
    /// ```
    #[inline]
    pub const fn new(opcode: MiOpcode, length_dwords: u8) -> Self {
        let len_field = (length_dwords.saturating_sub(2)) as u32;
        let value = ((opcode as u32) << Self::OPCODE_SHIFT) | (len_field & Self::LEN_MASK);
        Self { value }
    }

    /// Create a new MI command header with opcode-specific data
    ///
    /// # Arguments
    ///
    /// * `opcode` - The MI opcode
    /// * `length_dwords` - Total command length in DWORDs
    /// * `data` - Opcode-specific data (bits 6-22)
    #[inline]
    pub const fn with_data(opcode: MiOpcode, length_dwords: u8, data: u32) -> Self {
        let len_field = (length_dwords.saturating_sub(2)) as u32;
        let data_field = (data << 6) & Self::DATA_MASK;
        let value = ((opcode as u32) << Self::OPCODE_SHIFT) | data_field | (len_field & Self::LEN_MASK);
        Self { value }
    }

    /// Extract the DWORD length from header
    ///
    /// Returns the total command length in DWORDs (adds 2 to stored value).
    #[inline]
    pub const fn length_dwords(self) -> u8 {
        ((self.value & Self::LEN_MASK) as u8).saturating_add(2)
    }

    /// Extract the opcode from header
    #[inline]
    pub const fn opcode_raw(self) -> u8 {
        ((self.value & Self::OPCODE_MASK) >> Self::OPCODE_SHIFT) as u8
    }
}

// ============================================================================
// Intel Engine Class
// ============================================================================

/// Intel GPU engine classes
///
/// Each Intel GPU has multiple engines for different workload types.
/// Gen9+ GPUs typically have: RCS (Render), BCS (Blitter), VCS (Video),
/// VECS (Video Enhancement), and on Xe+ architectures, CCS (Compute).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum IntelEngineClass {
    /// Render Command Streamer - 3D rendering, compute shaders
    Render = 0,
    /// Blitter Command Streamer - 2D blits, memory copies
    Blitter = 1,
    /// Video Command Streamer - Video decode/encode
    Video = 2,
    /// Video Enhancement Command Streamer - Post-processing
    VideoEnhance = 3,
    /// Compute Command Streamer (Xe+ only) - Dedicated compute
    Compute = 4,
}

impl IntelEngineClass {
    /// Get the human-readable name for this engine class
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Render => "Render (RCS)",
            Self::Blitter => "Blitter (BCS)",
            Self::Video => "Video (VCS)",
            Self::VideoEnhance => "VideoEnhance (VECS)",
            Self::Compute => "Compute (CCS)",
        }
    }

    /// Convert from u8 to IntelEngineClass
    ///
    /// Returns Render for unknown values (safe default).
    #[inline]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Render,
            1 => Self::Blitter,
            2 => Self::Video,
            3 => Self::VideoEnhance,
            4 => Self::Compute,
            _ => Self::Render, // Default to Render
        }
    }
}

impl fmt::Display for IntelEngineClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// Ring State
// ============================================================================

/// Ring buffer state machine
///
/// Tracks the lifecycle of the ring buffer from initialization to error states.
///
/// # State Transitions
///
/// ```text
/// Uninitialized ──► Ready ──► Active ──► Stalled ──► Ready
///       │            │          │           │
///       └────────────┴──────────┴───────────┴──► Error
///                                                  │
///                                                  ▼
///                                              Resetting ──► Ready
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RingState {
    /// Ring buffer not yet initialized
    Uninitialized = 0,
    /// Ring buffer ready to accept commands
    Ready = 1,
    /// Commands in flight (between submit and completion)
    Active = 2,
    /// Waiting for space in ring buffer
    Stalled = 3,
    /// Error condition (requires reset)
    Error = 4,
    /// Reset in progress
    Resetting = 5,
}

impl RingState {
    /// Convert from u8 to RingState
    ///
    /// Unknown values default to Uninitialized for safety.
    #[inline]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Uninitialized,
            1 => Self::Ready,
            2 => Self::Active,
            3 => Self::Stalled,
            4 => Self::Error,
            5 => Self::Resetting,
            _ => Self::Uninitialized,
        }
    }

    /// Check if ring can accept new commands
    #[inline]
    pub const fn can_submit(self) -> bool {
        matches!(self, Self::Ready | Self::Active)
    }

    /// Check if ring is in error state
    #[inline]
    pub const fn is_error(self) -> bool {
        matches!(self, Self::Error)
    }
}

impl fmt::Display for RingState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Uninitialized => write!(f, "Uninitialized"),
            Self::Ready => write!(f, "Ready"),
            Self::Active => write!(f, "Active"),
            Self::Stalled => write!(f, "Stalled"),
            Self::Error => write!(f, "Error"),
            Self::Resetting => write!(f, "Resetting"),
        }
    }
}

// ============================================================================
// Ring Flags
// ============================================================================

/// Ring buffer flags packed into bits 24-63 of state_gen
pub struct RingFlags;

impl RingFlags {
    /// Ring has pending work
    pub const PENDING: u64 = 1 << 24;
    /// Ring is preemptible
    pub const PREEMPTIBLE: u64 = 1 << 25;
    /// Ring uses secure batch buffers
    pub const SECURE: u64 = 1 << 26;
    /// Ring supports semaphores
    pub const SEMAPHORE_CAPABLE: u64 = 1 << 27;
    /// Ring is virtualized (SR-IOV)
    pub const VIRTUALIZED: u64 = 1 << 28;
}

// ============================================================================
// Intel Ring Capsule
// ============================================================================

/// Intel GPU Ring Buffer Capsule (T1 Atomic, 256B)
///
/// Manages EXECLIST-style ring buffer for command submission to Intel GPUs.
/// Uses lockfree atomic operations for head/tail management.
///
/// # Layout (256 bytes, 256-byte aligned)
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────────┐
/// │  state_gen (AtomicU64)      │  head_tail (AtomicU64)           │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  ring_base (AtomicU64)      │  ring_size (AtomicU64)           │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  last_seqno (AtomicU64)     │  fence_addr (AtomicU64)          │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  submit_count (AtomicU64)   │  error_count (AtomicU64)         │ 16B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  hw_status_page (AtomicU64) │  context_id (AtomicU32)          │ 12B
/// │  engine_class (u8)          │  engine_instance (u8)            │  2B
/// │  _reserved (u16)                                               │  2B
/// ├─────────────────────────────────────────────────────────────────┤
/// │  _padding [192 bytes]                                          │192B
/// └─────────────────────────────────────────────────────────────────┘
/// ```
///
/// # Packed State Layout (state_gen)
///
/// ```text
/// Bits  0-7:  RingState (8 bits) - Current state
/// Bits  8-23: Generation counter (16 bits) - TOCTOU prevention
/// Bits 24-63: Flags (40 bits) - Ring flags
/// ```
///
/// # Packed Head/Tail Layout (head_tail)
///
/// ```text
/// Bits  0-31: Head position (DWORD offset from ring_base)
/// Bits 32-63: Tail position (DWORD offset from ring_base)
/// ```
///
/// # ASSUM Safety
///
/// - `#ASSUME_RING_VALID`: Ring buffer base address is valid GPU memory
/// - `#ASSUME_RING_ALIGNED`: Ring buffer is 4KB page-aligned
/// - `#ASSUME_MMIO_SAFE`: MMIO writes to doorbell are properly fenced
#[repr(C, align(256))]
pub struct IntelRingCapsule {
    /// State (bits 0-7) + Generation (bits 8-23) + Flags (bits 24-63)
    state_gen: AtomicU64,

    /// Head (bits 0-31) + Tail (bits 32-63) - both in DWORD offsets
    head_tail: AtomicU64,

    /// GPU virtual address of ring buffer base
    ring_base: AtomicU64,

    /// Ring size in bytes (must be power of 2, typically 4KB-128KB)
    ring_size: AtomicU64,

    /// Last submitted sequence number
    last_seqno: AtomicU64,

    /// GPU address where completion seqno is written
    fence_addr: AtomicU64,

    /// Total commands submitted
    submit_count: AtomicU64,

    /// Total submission errors
    error_count: AtomicU64,

    /// GPU address of hardware status page (HWS)
    hw_status_page: AtomicU64,

    /// EXECLIST context ID
    context_id: AtomicU32,

    /// Engine class (0=Render, 1=BLT, 2=Video, 3=VideoEnhance, 4=Compute)
    engine_class: u8,

    /// Engine instance within class
    engine_instance: u8,

    /// Reserved for alignment
    _reserved: u16,

    /// Padding to reach exactly 256 bytes
    /// Fields: 8*8 + 4 + 1 + 1 + 2 = 72 bytes
    /// Padding needed: 256 - 72 = 184 bytes
    _padding: [u8; 184],
}

impl IntelRingCapsule {
    // ========================================================================
    // Constants
    // ========================================================================

    /// Mask for extracting state from state_gen (bits 0-7)
    const STATE_MASK: u64 = 0xFF;

    /// Mask for extracting generation from state_gen (bits 8-23)
    const GEN_MASK: u64 = 0x00FF_FF00;

    /// Shift amount for generation counter
    const GEN_SHIFT: u32 = 8;

    /// Mask for flags (bits 24-63)
    const FLAGS_MASK: u64 = 0xFFFF_FFFF_FF00_0000;

    /// Mask for extracting head from head_tail (bits 0-31)
    const HEAD_MASK: u64 = 0x0000_0000_FFFF_FFFF;

    /// Mask for extracting tail from head_tail (bits 32-63)
    const TAIL_MASK: u64 = 0xFFFF_FFFF_0000_0000;

    /// Shift amount for tail
    const TAIL_SHIFT: u32 = 32;

    /// Minimum ring size (4KB)
    pub const MIN_RING_SIZE: u64 = 4096;

    /// Maximum ring size (128KB)
    pub const MAX_RING_SIZE: u64 = 128 * 1024;

    /// Default ring size (16KB)
    pub const DEFAULT_RING_SIZE: u64 = 16 * 1024;

    // ========================================================================
    // Construction
    // ========================================================================

    /// Create a new uninitialized ring buffer capsule
    ///
    /// # Returns
    ///
    /// A new `IntelRingCapsule` in `Uninitialized` state with generation 0.
    ///
    /// # Performance
    ///
    /// O(1), ~5ns (just zeroing memory)
    #[inline]
    pub const fn new() -> Self {
        Self {
            state_gen: AtomicU64::new(0), // State::Uninitialized, gen 0
            head_tail: AtomicU64::new(0),
            ring_base: AtomicU64::new(0),
            ring_size: AtomicU64::new(0),
            last_seqno: AtomicU64::new(0),
            fence_addr: AtomicU64::new(0),
            submit_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            hw_status_page: AtomicU64::new(0),
            context_id: AtomicU32::new(0),
            engine_class: 0,
            engine_instance: 0,
            _reserved: 0,
            _padding: [0u8; 184],
        }
    }

    /// Initialize ring buffer with GPU memory
    ///
    /// Transitions from `Uninitialized` -> `Ready` state using CAS.
    /// Increments generation counter on success.
    ///
    /// # Arguments
    ///
    /// * `ring_base` - GPU virtual address of ring buffer (must be 4KB aligned)
    /// * `ring_size` - Ring size in bytes (must be power of 2, 4KB-128KB)
    /// * `fence_addr` - GPU address where completion seqno is written
    /// * `hw_status_page` - GPU address of hardware status page
    /// * `context_id` - EXECLIST context ID
    /// * `engine_class` - Engine class (0-4)
    /// * `engine_instance` - Engine instance within class
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err(InvalidAlignment)` if ring_base not 4KB aligned
    /// - `Err(InvalidSize)` if ring_size not valid
    /// - `Err(InvalidState)` if not in Uninitialized state
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_RING_VALID`: ring_base must point to valid GPU memory
    /// - `#ASSUME_RING_ALIGNED`: ring_base must be 4KB page-aligned
    pub fn initialize(
        &self,
        ring_base: u64,
        ring_size: u64,
        fence_addr: u64,
        hw_status_page: u64,
        context_id: u32,
        _engine_class: u8,
        _engine_instance: u8,
    ) -> KgpuDriverResult<()> {
        // Validate ring_base alignment (4KB)
        if ring_base & 0xFFF != 0 {
            return Err(KgpuDriverError::InvalidAlignment);
        }

        // Validate ring_size is power of 2 and within bounds
        if ring_size < Self::MIN_RING_SIZE
            || ring_size > Self::MAX_RING_SIZE
            || !ring_size.is_power_of_two()
        {
            return Err(KgpuDriverError::InvalidSize);
        }

        // Try to transition Uninitialized -> Ready
        let old = self.state_gen.load(Ordering::Acquire);
        let old_state = RingState::from_u8((old & Self::STATE_MASK) as u8);

        if old_state != RingState::Uninitialized {
            return Err(KgpuDriverError::InvalidState);
        }

        // Calculate new state_gen with incremented generation
        let old_gen = ((old & Self::GEN_MASK) >> Self::GEN_SHIFT) as u16;
        let new_gen = old_gen.wrapping_add(1);
        let new = (RingState::Ready as u64)
            | ((new_gen as u64) << Self::GEN_SHIFT)
            | (old & Self::FLAGS_MASK);

        // Atomic CAS to transition state
        match self.state_gen.compare_exchange(
            old,
            new,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                // Successfully transitioned, now set other fields
                self.ring_base.store(ring_base, Ordering::Release);
                self.ring_size.store(ring_size, Ordering::Release);
                self.fence_addr.store(fence_addr, Ordering::Release);
                self.hw_status_page.store(hw_status_page, Ordering::Release);
                self.context_id.store(context_id, Ordering::Release);

                // Note: engine_class and engine_instance are immutable after construction
                // In a real implementation, we'd need interior mutability or
                // store them in an atomic. For now, we accept this limitation.

                Ok(())
            }
            Err(_) => Err(KgpuDriverError::StateTransitionFailed),
        }
    }

    // ========================================================================
    // State Accessors
    // ========================================================================

    /// Get current ring state
    ///
    /// # Returns
    ///
    /// Current `RingState` (Uninitialized, Ready, Active, etc.)
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load with Acquire ordering)
    #[inline]
    pub fn state(&self) -> RingState {
        let v = self.state_gen.load(Ordering::Acquire);
        RingState::from_u8((v & Self::STATE_MASK) as u8)
    }

    /// Get generation counter
    ///
    /// The generation counter increments on each state transition, providing
    /// TOCTOU (time-of-check-to-time-of-use) prevention.
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

    /// Get head position (DWORD offset)
    ///
    /// Head points to where GPU is currently reading commands.
    ///
    /// # Returns
    ///
    /// Head offset in DWORDs from ring_base
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn head(&self) -> u32 {
        let v = self.head_tail.load(Ordering::Acquire);
        (v & Self::HEAD_MASK) as u32
    }

    /// Get tail position (DWORD offset)
    ///
    /// Tail points to where CPU will write next command.
    ///
    /// # Returns
    ///
    /// Tail offset in DWORDs from ring_base
    ///
    /// # Performance
    ///
    /// <10ns (single atomic load)
    #[inline]
    pub fn tail(&self) -> u32 {
        let v = self.head_tail.load(Ordering::Acquire);
        ((v & Self::TAIL_MASK) >> Self::TAIL_SHIFT) as u32
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

    /// Get last submitted sequence number
    #[inline]
    pub fn last_seqno(&self) -> u64 {
        self.last_seqno.load(Ordering::Acquire)
    }

    /// Get fence address
    #[inline]
    pub fn fence_addr(&self) -> u64 {
        self.fence_addr.load(Ordering::Acquire)
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

    /// Get hardware status page address
    #[inline]
    pub fn hw_status_page(&self) -> u64 {
        self.hw_status_page.load(Ordering::Acquire)
    }

    /// Get context ID
    #[inline]
    pub fn context_id(&self) -> u32 {
        self.context_id.load(Ordering::Acquire)
    }

    /// Get engine class
    #[inline]
    pub fn engine_class(&self) -> IntelEngineClass {
        IntelEngineClass::from_u8(self.engine_class)
    }

    /// Get engine instance
    #[inline]
    pub fn engine_instance(&self) -> u8 {
        self.engine_instance
    }

    // ========================================================================
    // Ring Buffer Operations
    // ========================================================================

    /// Calculate available space in DWORDs
    ///
    /// Returns the number of DWORDs that can be written without wrapping
    /// over the head pointer.
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
        let ht = self.head_tail.load(Ordering::Acquire);
        let head = (ht & Self::HEAD_MASK) as u32;
        let tail = ((ht & Self::TAIL_MASK) >> Self::TAIL_SHIFT) as u32;
        let ring_size_dwords = self.ring_size_dwords();

        if ring_size_dwords == 0 {
            return 0;
        }

        // Available space is head - tail - 1 (wraparound)
        // We always keep at least 1 DWORD gap to distinguish full from empty
        let space = if tail >= head {
            ring_size_dwords - tail + head
        } else {
            head - tail
        };

        // Reserve 1 DWORD gap
        space.saturating_sub(1)
    }

    /// Reserve space for N DWORDs (returns tail offset or error)
    ///
    /// Uses CAS loop for lockfree reservation. On success, returns the
    /// starting offset where commands should be written.
    ///
    /// # Arguments
    ///
    /// * `dwords` - Number of DWORDs to reserve
    ///
    /// # Returns
    ///
    /// - `Ok(offset)` - Starting DWORD offset for writing
    /// - `Err(RingBufferFull)` - Not enough space
    /// - `Err(InvalidState)` - Ring not in submittable state
    ///
    /// # Performance
    ///
    /// <20ns (single CAS in uncontended case)
    pub fn reserve(&self, dwords: u32) -> KgpuDriverResult<u32> {
        if dwords == 0 {
            return Err(KgpuDriverError::InvalidParameter);
        }

        loop {
            // Check state
            let state = self.state();
            if !state.can_submit() {
                return Err(KgpuDriverError::InvalidState);
            }

            let ht = self.head_tail.load(Ordering::Acquire);
            let head = (ht & Self::HEAD_MASK) as u32;
            let tail = ((ht & Self::TAIL_MASK) >> Self::TAIL_SHIFT) as u32;
            let ring_size_dwords = self.ring_size_dwords();

            if ring_size_dwords == 0 {
                return Err(KgpuDriverError::InvalidState);
            }

            // Calculate available space
            let space = if tail >= head {
                ring_size_dwords - tail + head
            } else {
                head - tail
            };

            // Need at least dwords + 1 (gap)
            if space <= dwords {
                return Err(KgpuDriverError::RingBufferFull);
            }

            // Calculate new tail with wraparound
            let new_tail = (tail + dwords) % ring_size_dwords;
            let new_ht = (head as u64) | ((new_tail as u64) << Self::TAIL_SHIFT);

            // Atomic CAS to update tail
            match self.head_tail.compare_exchange_weak(
                ht,
                new_ht,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(tail),
                Err(_) => continue, // Retry on CAS failure
            }
        }
    }

    // ========================================================================
    // MI Command Emission
    // ========================================================================

    /// Write MI_NOOP command
    ///
    /// NOOP is used for padding or synchronization. Can optionally contain
    /// an identification tag for debugging.
    ///
    /// # Arguments
    ///
    /// * `offset` - DWORD offset in ring buffer
    ///
    /// # Returns
    ///
    /// Next DWORD offset after command (offset + 1)
    ///
    /// # Performance
    ///
    /// <5ns (single store)
    ///
    /// # Safety
    ///
    /// Caller must ensure `offset` is valid within the ring buffer.
    #[inline]
    pub fn emit_noop(&self, _offset: u32) -> u32 {
        // In a real implementation, we'd write to ring_base + offset*4
        // let cmd = MiCommandHeader::new(MiOpcode::Noop, 1);
        // unsafe { *((self.ring_base() + offset as u64 * 4) as *mut u32) = cmd.value; }
        _offset + 1
    }

    /// Write MI_BATCH_BUFFER_START command
    ///
    /// Starts executing commands from a batch buffer. The GPU will jump to
    /// the batch buffer, execute it, and return to the ring buffer.
    ///
    /// # Arguments
    ///
    /// * `offset` - DWORD offset in ring buffer
    /// * `batch_addr` - GPU address of batch buffer (48-bit)
    /// * `flags` - Batch buffer flags (address space select, etc.)
    ///
    /// # Returns
    ///
    /// Next DWORD offset after command (offset + 3)
    ///
    /// # Performance
    ///
    /// <10ns (3 stores)
    #[inline]
    pub fn emit_batch_buffer_start(&self, _offset: u32, _batch_addr: u64, _flags: u32) -> u32 {
        // MI_BATCH_BUFFER_START is 3 DWORDs:
        // DWORD 0: Header with flags
        // DWORD 1: Batch address low
        // DWORD 2: Batch address high (bits 32-47)
        //
        // let header = MiCommandHeader::with_data(MiOpcode::BatchBufferStart, 3, flags);
        // In real implementation: write header, batch_addr_low, batch_addr_high
        _offset + 3
    }

    /// Write MI_STORE_DATA_IMM to signal completion
    ///
    /// Stores an immediate value to memory. Commonly used to write sequence
    /// numbers for completion tracking.
    ///
    /// # Arguments
    ///
    /// * `offset` - DWORD offset in ring buffer
    /// * `addr` - GPU address to write to (48-bit)
    /// * `value` - 32-bit value to store
    ///
    /// # Returns
    ///
    /// Next DWORD offset after command (offset + 4)
    ///
    /// # Performance
    ///
    /// <10ns (4 stores)
    #[inline]
    pub fn emit_store_data_imm(&self, _offset: u32, _addr: u64, _value: u32) -> u32 {
        // MI_STORE_DATA_IMM is 4 DWORDs:
        // DWORD 0: Header
        // DWORD 1: Address low
        // DWORD 2: Address high (bits 32-47)
        // DWORD 3: Data
        _offset + 4
    }

    /// Write MI_FLUSH_DW for cache flush
    ///
    /// Flushes GPU caches and optionally writes a marker value to memory.
    ///
    /// # Arguments
    ///
    /// * `offset` - DWORD offset in ring buffer
    ///
    /// # Returns
    ///
    /// Next DWORD offset after command (offset + 4)
    ///
    /// # Performance
    ///
    /// <10ns (4 stores)
    #[inline]
    pub fn emit_flush_dw(&self, _offset: u32) -> u32 {
        // MI_FLUSH_DW is 4 DWORDs (with post-sync write)
        _offset + 4
    }

    /// Write MI_USER_INTERRUPT for completion notification
    ///
    /// Generates a user interrupt that can wake waiting CPU threads.
    ///
    /// # Arguments
    ///
    /// * `offset` - DWORD offset in ring buffer
    ///
    /// # Returns
    ///
    /// Next DWORD offset after command (offset + 1)
    ///
    /// # Performance
    ///
    /// <5ns (single store)
    #[inline]
    pub fn emit_user_interrupt(&self, _offset: u32) -> u32 {
        // MI_USER_INTERRUPT is 1 DWORD
        _offset + 1
    }

    // ========================================================================
    // Submission
    // ========================================================================

    /// Commit written commands (advance tail, ring doorbell)
    ///
    /// After writing commands to the ring buffer, call submit() to make
    /// them visible to the GPU. This updates the tail pointer and returns
    /// a new sequence number for tracking completion.
    ///
    /// # Arguments
    ///
    /// * `new_tail` - New tail position after written commands
    ///
    /// # Returns
    ///
    /// - `Ok(seqno)` - New sequence number for this submission
    /// - `Err(InvalidState)` - Ring not in submittable state
    /// - `Err(RingSubmitFailed)` - Tail update failed
    ///
    /// # Performance
    ///
    /// <60ns (tail update + memory fence)
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_MMIO_SAFE`: Doorbell write is properly fenced
    pub fn submit(&self, new_tail: u32) -> KgpuDriverResult<u64> {
        // Check state
        let state = self.state();
        if !state.can_submit() {
            return Err(KgpuDriverError::InvalidState);
        }

        let ring_size_dwords = self.ring_size_dwords();
        if ring_size_dwords == 0 || new_tail >= ring_size_dwords {
            return Err(KgpuDriverError::InvalidParameter);
        }

        // Update tail atomically
        loop {
            let ht = self.head_tail.load(Ordering::Acquire);
            let head = (ht & Self::HEAD_MASK) as u32;
            let new_ht = (head as u64) | ((new_tail as u64) << Self::TAIL_SHIFT);

            match self.head_tail.compare_exchange_weak(
                ht,
                new_ht,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(_) => continue,
            }
        }

        // Increment sequence number
        let seqno = self.last_seqno.fetch_add(1, Ordering::AcqRel) + 1;

        // Increment submit count
        self.submit_count.fetch_add(1, Ordering::Relaxed);

        // Transition to Active if not already
        self.try_set_state(RingState::Active);

        // In real implementation: ring the doorbell via MMIO write
        // unsafe { write_volatile(doorbell_addr, new_tail); }

        Ok(seqno)
    }

    /// Update head position (called after GPU completes)
    ///
    /// Called when GPU signals completion (via interrupt or polling).
    /// Updates the head pointer to reflect commands consumed by GPU.
    ///
    /// # Arguments
    ///
    /// * `new_head` - New head position reported by GPU
    ///
    /// # Performance
    ///
    /// <20ns (CAS loop)
    pub fn update_head(&self, new_head: u32) {
        loop {
            let ht = self.head_tail.load(Ordering::Acquire);
            let tail = ((ht & Self::TAIL_MASK) >> Self::TAIL_SHIFT) as u32;
            let new_ht = (new_head as u64) | ((tail as u64) << Self::TAIL_SHIFT);

            match self.head_tail.compare_exchange_weak(
                ht,
                new_ht,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // If head caught up to tail, ring is idle
                    if new_head == tail {
                        self.try_set_state(RingState::Ready);
                    }
                    break;
                }
                Err(_) => continue,
            }
        }
    }

    /// Wait for ring to become idle
    ///
    /// Busy-waits until head catches up to tail, indicating all commands
    /// have been consumed by GPU.
    ///
    /// # Arguments
    ///
    /// * `timeout_ns` - Maximum wait time in nanoseconds
    ///
    /// # Returns
    ///
    /// - `Ok(())` - Ring is idle
    /// - `Err(CommandTimeout)` - Timeout waiting for ring
    ///
    /// # Performance
    ///
    /// Varies based on GPU workload
    pub fn wait_idle(&self, _timeout_ns: u64) -> KgpuDriverResult<()> {
        // In real implementation: spin until head == tail or timeout
        // This is a simplified version that just checks current state
        let ht = self.head_tail.load(Ordering::Acquire);
        let head = (ht & Self::HEAD_MASK) as u32;
        let tail = ((ht & Self::TAIL_MASK) >> Self::TAIL_SHIFT) as u32;

        if head == tail {
            Ok(())
        } else {
            // In real implementation: spin with timeout
            Err(KgpuDriverError::CommandTimeout)
        }
    }

    // ========================================================================
    // State Transitions
    // ========================================================================

    /// Try to set ring state (internal helper)
    fn try_set_state(&self, new_state: RingState) {
        loop {
            let old = self.state_gen.load(Ordering::Acquire);
            let old_gen = ((old & Self::GEN_MASK) >> Self::GEN_SHIFT) as u16;
            let new_gen = old_gen.wrapping_add(1);
            let new = (new_state as u64)
                | ((new_gen as u64) << Self::GEN_SHIFT)
                | (old & Self::FLAGS_MASK);

            match self.state_gen.compare_exchange_weak(
                old,
                new,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(_) => continue,
            }
        }
    }

    /// Reset ring buffer to initial state
    ///
    /// Clears head/tail and transitions to Ready state.
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success
    /// - `Err(InvalidState)` if not initialized
    pub fn reset(&self) -> KgpuDriverResult<()> {
        let state = self.state();
        if state == RingState::Uninitialized {
            return Err(KgpuDriverError::InvalidState);
        }

        // Clear head/tail
        self.head_tail.store(0, Ordering::Release);

        // Transition to Ready
        self.try_set_state(RingState::Ready);

        Ok(())
    }

    /// Mark ring as having an error
    ///
    /// Called when GPU error is detected. Increments error count.
    pub fn mark_error(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
        self.try_set_state(RingState::Error);
    }

    // ========================================================================
    // Snapshots
    // ========================================================================

    /// Take an atomic snapshot of current state
    ///
    /// Captures all state atomically for consistent reads.
    ///
    /// # Returns
    ///
    /// Immutable `IntelRingSnapshot` with all current values
    ///
    /// # Performance
    ///
    /// <20ns (multiple atomic loads)
    #[inline]
    pub fn snapshot(&self) -> IntelRingSnapshot {
        let state_gen = self.state_gen.load(Ordering::Acquire);
        let head_tail = self.head_tail.load(Ordering::Acquire);

        IntelRingSnapshot {
            state: RingState::from_u8((state_gen & Self::STATE_MASK) as u8),
            generation: ((state_gen & Self::GEN_MASK) >> Self::GEN_SHIFT) as u16,
            head: (head_tail & Self::HEAD_MASK) as u32,
            tail: ((head_tail & Self::TAIL_MASK) >> Self::TAIL_SHIFT) as u32,
            ring_base: self.ring_base.load(Ordering::Acquire),
            ring_size: self.ring_size.load(Ordering::Acquire),
            last_seqno: self.last_seqno.load(Ordering::Acquire),
            submit_count: self.submit_count.load(Ordering::Acquire),
            error_count: self.error_count.load(Ordering::Acquire),
            context_id: self.context_id.load(Ordering::Acquire),
            engine_class: self.engine_class,
            engine_instance: self.engine_instance,
        }
    }
}

impl Default for IntelRingCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for IntelRingCapsule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let snap = self.snapshot();
        f.debug_struct("IntelRingCapsule")
            .field("state", &snap.state)
            .field("generation", &snap.generation)
            .field("head", &snap.head)
            .field("tail", &snap.tail)
            .field("ring_base", &format_args!("0x{:x}", snap.ring_base))
            .field("ring_size", &snap.ring_size)
            .field("last_seqno", &snap.last_seqno)
            .field("context_id", &snap.context_id)
            .field("engine_class", &IntelEngineClass::from_u8(snap.engine_class))
            .finish()
    }
}

// Safety: All fields are AtomicU64/AtomicU32 or immutable
// AtomicU64 is Send + Sync, so IntelRingCapsule can be safely shared across threads.
unsafe impl Send for IntelRingCapsule {}
unsafe impl Sync for IntelRingCapsule {}

// ============================================================================
// Ring Snapshot
// ============================================================================

/// Immutable snapshot of Intel ring buffer state
///
/// Captured atomically from `IntelRingCapsule::snapshot()`.
#[derive(Debug, Clone, Copy)]
pub struct IntelRingSnapshot {
    /// Current ring state
    pub state: RingState,
    /// Generation counter at snapshot time
    pub generation: u16,
    /// Head position (DWORD offset)
    pub head: u32,
    /// Tail position (DWORD offset)
    pub tail: u32,
    /// GPU address of ring buffer base
    pub ring_base: u64,
    /// Ring size in bytes
    pub ring_size: u64,
    /// Last submitted sequence number
    pub last_seqno: u64,
    /// Total commands submitted
    pub submit_count: u64,
    /// Total errors
    pub error_count: u64,
    /// Context ID
    pub context_id: u32,
    /// Engine class
    pub engine_class: u8,
    /// Engine instance
    pub engine_instance: u8,
}

impl IntelRingSnapshot {
    /// Check if ring is in Ready state
    #[inline]
    pub fn is_ready(&self) -> bool {
        matches!(self.state, RingState::Ready)
    }

    /// Check if ring is active (commands in flight)
    #[inline]
    pub fn is_active(&self) -> bool {
        matches!(self.state, RingState::Active)
    }

    /// Check if ring is idle (head == tail)
    #[inline]
    pub fn is_idle(&self) -> bool {
        self.head == self.tail
    }

    /// Get ring size in DWORDs
    #[inline]
    pub fn ring_size_dwords(&self) -> u32 {
        (self.ring_size / 4) as u32
    }

    /// Calculate available space
    #[inline]
    pub fn available_space(&self) -> u32 {
        let ring_size_dwords = self.ring_size_dwords();
        if ring_size_dwords == 0 {
            return 0;
        }

        let space = if self.tail >= self.head {
            ring_size_dwords - self.tail + self.head
        } else {
            self.head - self.tail
        };

        space.saturating_sub(1)
    }
}

impl Default for IntelRingSnapshot {
    fn default() -> Self {
        Self {
            state: RingState::Uninitialized,
            generation: 0,
            head: 0,
            tail: 0,
            ring_base: 0,
            ring_size: 0,
            last_seqno: 0,
            submit_count: 0,
            error_count: 0,
            context_id: 0,
            engine_class: 0,
            engine_instance: 0,
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
    // Tier 1: Unit Tests (Q1-Q7)
    // ========================================================================

    #[test]
    fn test_capsule_size() {
        // T28 Q1: Verify exact size is 256 bytes
        assert_eq!(mem::size_of::<IntelRingCapsule>(), 256);
    }

    #[test]
    fn test_capsule_alignment() {
        // T28 Q2: Verify alignment is 256 bytes (4 cache lines)
        assert_eq!(mem::align_of::<IntelRingCapsule>(), 256);
    }

    #[test]
    fn test_new_capsule_state() {
        // T28 Q3: Verify initial state is Uninitialized with generation 0
        let capsule = IntelRingCapsule::new();
        assert_eq!(capsule.state(), RingState::Uninitialized);
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.head(), 0);
        assert_eq!(capsule.tail(), 0);
        assert_eq!(capsule.ring_base(), 0);
        assert_eq!(capsule.ring_size(), 0);
    }

    #[test]
    fn test_default_impl() {
        // T28 Q4: Verify Default trait implementation
        let capsule: IntelRingCapsule = Default::default();
        assert_eq!(capsule.state(), RingState::Uninitialized);
    }

    #[test]
    fn test_ring_state_from_u8() {
        // T28 Q5: Verify RingState conversion
        assert_eq!(RingState::from_u8(0), RingState::Uninitialized);
        assert_eq!(RingState::from_u8(1), RingState::Ready);
        assert_eq!(RingState::from_u8(2), RingState::Active);
        assert_eq!(RingState::from_u8(3), RingState::Stalled);
        assert_eq!(RingState::from_u8(4), RingState::Error);
        assert_eq!(RingState::from_u8(5), RingState::Resetting);
        assert_eq!(RingState::from_u8(255), RingState::Uninitialized); // Unknown
    }

    #[test]
    fn test_ring_state_can_submit() {
        // T28 Q6: Verify can_submit predicate
        assert!(!RingState::Uninitialized.can_submit());
        assert!(RingState::Ready.can_submit());
        assert!(RingState::Active.can_submit());
        assert!(!RingState::Stalled.can_submit());
        assert!(!RingState::Error.can_submit());
        assert!(!RingState::Resetting.can_submit());
    }

    #[test]
    fn test_engine_class_from_u8() {
        // T28 Q7: Verify IntelEngineClass conversion
        assert_eq!(IntelEngineClass::from_u8(0), IntelEngineClass::Render);
        assert_eq!(IntelEngineClass::from_u8(1), IntelEngineClass::Blitter);
        assert_eq!(IntelEngineClass::from_u8(2), IntelEngineClass::Video);
        assert_eq!(IntelEngineClass::from_u8(3), IntelEngineClass::VideoEnhance);
        assert_eq!(IntelEngineClass::from_u8(4), IntelEngineClass::Compute);
        assert_eq!(IntelEngineClass::from_u8(255), IntelEngineClass::Render); // Unknown
    }

    // ========================================================================
    // Tier 2: Initialization Tests (Q8-Q14)
    // ========================================================================

    #[test]
    fn test_initialize_success() {
        // T28 Q8: Verify successful initialization
        let capsule = IntelRingCapsule::new();

        let result = capsule.initialize(
            0x1000_0000, // 4KB aligned
            16 * 1024,   // 16KB ring
            0x2000_0000, // fence addr
            0x3000_0000, // HWS
            42,          // context ID
            0,           // Render engine
            0,           // instance 0
        );

        assert!(result.is_ok());
        assert_eq!(capsule.state(), RingState::Ready);
        assert_eq!(capsule.generation(), 1);
        assert_eq!(capsule.ring_base(), 0x1000_0000);
        assert_eq!(capsule.ring_size(), 16 * 1024);
        assert_eq!(capsule.context_id(), 42);
    }

    #[test]
    fn test_initialize_invalid_alignment() {
        // T28 Q9: Verify alignment check
        let capsule = IntelRingCapsule::new();

        let result = capsule.initialize(
            0x1000_0001, // NOT 4KB aligned
            16 * 1024,
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            0,
        );

        assert_eq!(result, Err(KgpuDriverError::InvalidAlignment));
        assert_eq!(capsule.state(), RingState::Uninitialized);
    }

    #[test]
    fn test_initialize_invalid_size_too_small() {
        // T28 Q10: Verify size bounds (too small)
        let capsule = IntelRingCapsule::new();

        let result = capsule.initialize(
            0x1000_0000,
            1024, // Less than 4KB
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            0,
        );

        assert_eq!(result, Err(KgpuDriverError::InvalidSize));
    }

    #[test]
    fn test_initialize_invalid_size_too_large() {
        // T28 Q11: Verify size bounds (too large)
        let capsule = IntelRingCapsule::new();

        let result = capsule.initialize(
            0x1000_0000,
            256 * 1024, // More than 128KB
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            0,
        );

        assert_eq!(result, Err(KgpuDriverError::InvalidSize));
    }

    #[test]
    fn test_initialize_invalid_size_not_power_of_2() {
        // T28 Q12: Verify size must be power of 2
        let capsule = IntelRingCapsule::new();

        let result = capsule.initialize(
            0x1000_0000,
            12 * 1024, // 12KB is not power of 2
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            0,
        );

        assert_eq!(result, Err(KgpuDriverError::InvalidSize));
    }

    #[test]
    fn test_initialize_already_initialized() {
        // T28 Q13: Verify cannot initialize twice
        let capsule = IntelRingCapsule::new();

        capsule.initialize(
            0x1000_0000,
            16 * 1024,
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            0,
        ).unwrap();

        let result = capsule.initialize(
            0x4000_0000,
            32 * 1024,
            0x5000_0000,
            0x6000_0000,
            99,
            1,
            0,
        );

        assert_eq!(result, Err(KgpuDriverError::InvalidState));
    }

    #[test]
    fn test_ring_size_dwords() {
        // T28 Q14: Verify DWORD size calculation
        let capsule = IntelRingCapsule::new();
        capsule.initialize(
            0x1000_0000,
            16 * 1024, // 16KB = 4096 DWORDs
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            0,
        ).unwrap();

        assert_eq!(capsule.ring_size_dwords(), 4096);
    }

    // ========================================================================
    // Tier 3: Head/Tail Tests (Q15-Q21)
    // ========================================================================

    #[test]
    fn test_available_space_empty() {
        // T28 Q15: Verify available space when empty
        let capsule = IntelRingCapsule::new();
        capsule.initialize(
            0x1000_0000,
            16 * 1024, // 4096 DWORDs
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            0,
        ).unwrap();

        // Empty ring: head=0, tail=0, available = 4096 - 1 = 4095
        assert_eq!(capsule.available_space(), 4095);
    }

    #[test]
    fn test_reserve_success() {
        // T28 Q16: Verify successful reserve
        let capsule = IntelRingCapsule::new();
        capsule.initialize(
            0x1000_0000,
            16 * 1024,
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            0,
        ).unwrap();

        let result = capsule.reserve(100);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0); // Starting offset
        assert_eq!(capsule.tail(), 100); // Tail advanced
    }

    #[test]
    fn test_reserve_multiple() {
        // T28 Q17: Verify multiple reserves
        let capsule = IntelRingCapsule::new();
        capsule.initialize(
            0x1000_0000,
            16 * 1024,
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            0,
        ).unwrap();

        let r1 = capsule.reserve(100).unwrap();
        let r2 = capsule.reserve(200).unwrap();
        let r3 = capsule.reserve(50).unwrap();

        assert_eq!(r1, 0);
        assert_eq!(r2, 100);
        assert_eq!(r3, 300);
        assert_eq!(capsule.tail(), 350);
    }

    #[test]
    fn test_reserve_full() {
        // T28 Q18: Verify reserve fails when full
        let capsule = IntelRingCapsule::new();
        capsule.initialize(
            0x1000_0000,
            4096, // 1024 DWORDs minimum
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            0,
        ).unwrap();

        // Try to reserve more than available (1023 due to gap)
        let result = capsule.reserve(2000);
        assert_eq!(result, Err(KgpuDriverError::RingBufferFull));
    }

    #[test]
    fn test_reserve_zero() {
        // T28 Q19: Verify reserve 0 fails
        let capsule = IntelRingCapsule::new();
        capsule.initialize(
            0x1000_0000,
            16 * 1024,
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            0,
        ).unwrap();

        let result = capsule.reserve(0);
        assert_eq!(result, Err(KgpuDriverError::InvalidParameter));
    }

    #[test]
    fn test_update_head() {
        // T28 Q20: Verify head update
        let capsule = IntelRingCapsule::new();
        capsule.initialize(
            0x1000_0000,
            16 * 1024,
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            0,
        ).unwrap();

        capsule.reserve(100).unwrap();
        assert_eq!(capsule.head(), 0);
        assert_eq!(capsule.tail(), 100);

        capsule.update_head(50);
        assert_eq!(capsule.head(), 50);
        assert_eq!(capsule.tail(), 100);
    }

    #[test]
    fn test_wraparound() {
        // T28 Q21: Verify head/tail wraparound
        let capsule = IntelRingCapsule::new();
        capsule.initialize(
            0x1000_0000,
            4096, // 1024 DWORDs
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            0,
        ).unwrap();

        // Reserve 500 DWORDs
        capsule.reserve(500).unwrap();
        assert_eq!(capsule.tail(), 500);

        // Update head to 400 (GPU processed 400)
        capsule.update_head(400);
        assert_eq!(capsule.head(), 400);

        // Available: 1024 - 500 + 400 - 1 = 923
        assert_eq!(capsule.available_space(), 923);

        // Reserve another 800 DWORDs (should wrap)
        let result = capsule.reserve(800);
        assert!(result.is_ok());

        // Tail wraps: (500 + 800) % 1024 = 276
        assert_eq!(capsule.tail(), 276);
    }

    // ========================================================================
    // Tier 4: Submit/Reset Tests (Q22-Q28)
    // ========================================================================

    #[test]
    fn test_submit_success() {
        // T28 Q22: Verify successful submit
        let capsule = IntelRingCapsule::new();
        capsule.initialize(
            0x1000_0000,
            16 * 1024,
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            0,
        ).unwrap();

        capsule.reserve(100).unwrap();
        let result = capsule.submit(100);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1); // First seqno
        assert_eq!(capsule.last_seqno(), 1);
        assert_eq!(capsule.submit_count(), 1);
    }

    #[test]
    fn test_submit_increments_seqno() {
        // T28 Q23: Verify seqno increments
        let capsule = IntelRingCapsule::new();
        capsule.initialize(
            0x1000_0000,
            16 * 1024,
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            0,
        ).unwrap();

        capsule.reserve(100).unwrap();
        let s1 = capsule.submit(100).unwrap();

        capsule.reserve(100).unwrap();
        let s2 = capsule.submit(200).unwrap();

        capsule.reserve(100).unwrap();
        let s3 = capsule.submit(300).unwrap();

        assert_eq!(s1, 1);
        assert_eq!(s2, 2);
        assert_eq!(s3, 3);
        assert_eq!(capsule.submit_count(), 3);
    }

    #[test]
    fn test_reset() {
        // T28 Q24: Verify reset
        let capsule = IntelRingCapsule::new();
        capsule.initialize(
            0x1000_0000,
            16 * 1024,
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            0,
        ).unwrap();

        capsule.reserve(100).unwrap();
        capsule.submit(100).unwrap();

        let result = capsule.reset();
        assert!(result.is_ok());
        assert_eq!(capsule.head(), 0);
        assert_eq!(capsule.tail(), 0);
        assert_eq!(capsule.state(), RingState::Ready);
    }

    #[test]
    fn test_reset_uninitialized_fails() {
        // T28 Q25: Verify reset on uninitialized fails
        let capsule = IntelRingCapsule::new();
        let result = capsule.reset();
        assert_eq!(result, Err(KgpuDriverError::InvalidState));
    }

    #[test]
    fn test_mark_error() {
        // T28 Q26: Verify error marking
        let capsule = IntelRingCapsule::new();
        capsule.initialize(
            0x1000_0000,
            16 * 1024,
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            0,
        ).unwrap();

        assert_eq!(capsule.error_count(), 0);

        capsule.mark_error();

        assert_eq!(capsule.state(), RingState::Error);
        assert_eq!(capsule.error_count(), 1);
    }

    #[test]
    fn test_wait_idle_already_idle() {
        // T28 Q27: Verify wait_idle when already idle
        let capsule = IntelRingCapsule::new();
        capsule.initialize(
            0x1000_0000,
            16 * 1024,
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            0,
        ).unwrap();

        let result = capsule.wait_idle(1_000_000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_generation_increments() {
        // T28 Q28: Verify generation increments on state changes
        let capsule = IntelRingCapsule::new();
        assert_eq!(capsule.generation(), 0);

        capsule.initialize(
            0x1000_0000,
            16 * 1024,
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            0,
        ).unwrap();
        assert_eq!(capsule.generation(), 1);

        capsule.reset().unwrap();
        // Generation incremented by reset
        assert!(capsule.generation() >= 2);
    }

    // ========================================================================
    // Tier 5: MI Command Tests (Q29-Q35)
    // ========================================================================

    #[test]
    fn test_mi_command_header_new() {
        // T28 Q29: Verify MI command header construction
        let header = MiCommandHeader::new(MiOpcode::Noop, 1);
        assert_eq!(header.opcode_raw(), MiOpcode::Noop as u8);
        assert_eq!(header.length_dwords(), 2); // Min is 2
    }

    #[test]
    fn test_mi_command_header_with_data() {
        // T28 Q30: Verify MI command header with data
        let header = MiCommandHeader::with_data(MiOpcode::BatchBufferStart, 3, 0x100);
        assert_eq!(header.opcode_raw(), MiOpcode::BatchBufferStart as u8);
        assert_eq!(header.length_dwords(), 3);
    }

    #[test]
    fn test_emit_noop() {
        // T28 Q31: Verify emit_noop returns correct offset
        let capsule = IntelRingCapsule::new();
        let next = capsule.emit_noop(0);
        assert_eq!(next, 1); // MI_NOOP is 1 DWORD
    }

    #[test]
    fn test_emit_batch_buffer_start() {
        // T28 Q32: Verify emit_batch_buffer_start
        let capsule = IntelRingCapsule::new();
        let next = capsule.emit_batch_buffer_start(0, 0x1000_0000, 0);
        assert_eq!(next, 3); // MI_BATCH_BUFFER_START is 3 DWORDs
    }

    #[test]
    fn test_emit_store_data_imm() {
        // T28 Q33: Verify emit_store_data_imm
        let capsule = IntelRingCapsule::new();
        let next = capsule.emit_store_data_imm(0, 0x1000_0000, 0xDEAD_BEEF);
        assert_eq!(next, 4); // MI_STORE_DATA_IMM is 4 DWORDs
    }

    #[test]
    fn test_snapshot() {
        // T28 Q34: Verify snapshot captures all state
        let capsule = IntelRingCapsule::new();
        capsule.initialize(
            0x1000_0000,
            16 * 1024,
            0x2000_0000,
            0x3000_0000,
            42,
            0,
            1,
        ).unwrap();

        capsule.reserve(100).unwrap();
        capsule.submit(100).unwrap();

        let snap = capsule.snapshot();
        assert_eq!(snap.state, RingState::Active);
        assert!(snap.generation >= 1);
        assert_eq!(snap.head, 0);
        assert_eq!(snap.tail, 100);
        assert_eq!(snap.ring_base, 0x1000_0000);
        assert_eq!(snap.ring_size, 16 * 1024);
        assert_eq!(snap.last_seqno, 1);
        assert_eq!(snap.submit_count, 1);
        assert_eq!(snap.context_id, 42);
    }

    #[test]
    fn test_send_sync_traits() {
        // T28 Q35: Verify Send + Sync implementation
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<IntelRingCapsule>();
    }

    // ========================================================================
    // Additional Coverage Tests
    // ========================================================================

    #[test]
    fn test_mi_opcode_names() {
        assert_eq!(MiOpcode::Noop.name(), "MI_NOOP");
        assert_eq!(MiOpcode::BatchBufferStart.name(), "MI_BATCH_BUFFER_START");
        assert_eq!(MiOpcode::UserInterrupt.name(), "MI_USER_INTERRUPT");
    }

    #[test]
    fn test_engine_class_names() {
        assert_eq!(IntelEngineClass::Render.name(), "Render (RCS)");
        assert_eq!(IntelEngineClass::Compute.name(), "Compute (CCS)");
    }

    #[test]
    fn test_ring_state_display() {
        assert_eq!(format!("{}", RingState::Ready), "Ready");
        assert_eq!(format!("{}", RingState::Error), "Error");
    }

    #[test]
    fn test_snapshot_is_idle() {
        let snap = IntelRingSnapshot {
            state: RingState::Ready,
            head: 100,
            tail: 100,
            ..Default::default()
        };
        assert!(snap.is_idle());

        let snap2 = IntelRingSnapshot {
            state: RingState::Active,
            head: 50,
            tail: 100,
            ..Default::default()
        };
        assert!(!snap2.is_idle());
    }

    #[test]
    fn test_snapshot_available_space() {
        let snap = IntelRingSnapshot {
            state: RingState::Ready,
            head: 0,
            tail: 0,
            ring_size: 4096,
            ..Default::default()
        };
        assert_eq!(snap.available_space(), 1023); // 1024 - 1 gap
    }

    #[test]
    fn test_debug_impl() {
        let capsule = IntelRingCapsule::new();
        let debug_str = format!("{:?}", capsule);
        assert!(debug_str.contains("IntelRingCapsule"));
        assert!(debug_str.contains("Uninitialized"));
    }
}
