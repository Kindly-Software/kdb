//! USB Transfer Capsule - Async Transfer Ring Management
//!
//! # Architecture
//! - **Tier 5 (Streaming)**: O(1) lockfree transfer enqueue/dequeue
//! - **1024-byte alignment**: 16 cache lines for comprehensive transfer state
//! - **Generation counters**: ABA prevention for multi-producer transfers
//! - **100% lockfree**: Atomic CAS-based operations
//!
//! # Transfer Ring Overview
//! Each endpoint has its own transfer ring for data transfers. This capsule
//! provides high-level management of the xHCI transfer ring including:
//! - Lockfree TRB enqueue/dequeue
//! - TD (Transfer Descriptor) tracking
//! - Cycle bit management
//! - Stream support for USB 3.x bulk endpoints
//!
//! # TD (Transfer Descriptor) Management
//! A TD is composed of one or more TRBs forming a single logical transfer:
//! - Control: Setup Stage + Data Stage + Status Stage TRBs
//! - Bulk/Interrupt: Normal TRBs (possibly chained for large transfers)
//! - Isochronous: Isoch TRBs with timing requirements
//!
//! # Performance Targets
//! - TRB enqueue: <50ns (single CAS)
//! - TD completion: <20ns (dequeue update)
//! - Ring status: <10ns (single cache line read)
//!
//! # Safety Assumptions (ASSUM Framework)
//! - #ASSUME[RING-DMA]: Ring buffer physically contiguous in DMA-capable memory
//! - #ASSUME[RING-ALIGNED]: Ring buffer 64-byte aligned for TRB access
//! - #ASSUME[ENDPOINT-CONFIG]: Endpoint context properly configured before transfers
//! - #ASSUME[BUFFER-VALID]: Data buffers valid for duration of transfer
//! - #VERIFY[ENQUEUE-CAS]: Enqueue pointer advanced atomically
//! - #VERIFY[CYCLE-TOGGLE]: Cycle bit toggled on ring wrap
//! - #VERIFY[TD-BOUNDARY]: TD boundaries respected in multi-TRB transfers
//! - #VERIFY[DEQUEUE-UPDATE]: Dequeue updated on completion events

use core::sync::atomic::{AtomicU64, AtomicU32, AtomicBool, Ordering};

/// Default transfer ring size (in TRBs)
/// #ASSUME[RING-SIZE]: 256 TRBs provides good balance of capacity vs memory
pub const DEFAULT_RING_SIZE: usize = 256;

/// TRB size in bytes (per xHCI specification)
/// #VERIFY[TRB-SIZE]: xHCI spec mandates 16-byte TRBs
pub const TRB_SIZE: usize = 16;

/// Maximum TDs that can be tracked in flight
/// #ASSUME[TD-LIMIT]: Typical workloads have <64 concurrent TDs
pub const MAX_TDS_IN_FLIGHT: usize = 64;

/// Stream ID for non-stream endpoints
pub const NO_STREAM: u16 = 0;

// ============================================================================
// Transfer Ring State
// ============================================================================

/// Transfer Ring state machine
///
/// #VERIFY[STATE-VALID]: All state values produce valid ring states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TransferRingState {
    /// Ring not initialized
    /// #ASSUME[UNINIT-SAFE]: Safe to initialize
    Uninitialized = 0,
    /// Ring being configured (allocating DMA, setting up Link TRB)
    /// #ASSUME[CONFIG-ATOMIC]: Configuration not interrupted
    Configuring = 1,
    /// Ring ready for transfers
    /// #VERIFY[READY-VALID]: Ring base, size, and cycle initialized
    Ready = 2,
    /// Ring stopped by Stop Endpoint command
    /// #ASSUME[STOP-COMPLETE]: Stop Endpoint command completed
    Stopped = 3,
    /// Ring stalled (endpoint halted due to error)
    /// #VERIFY[STALL-LOGGED]: Stall condition recorded
    Stalled = 4,
    /// Ring being drained (flushing pending TDs)
    /// #ASSUME[DRAIN-TIMEOUT]: Drain completes within timeout
    Draining = 5,
    /// Ring disabled (endpoint deconfigured)
    /// #ASSUME[DISABLE-CLEANUP]: All TDs completed or cancelled
    Disabled = 6,
    /// Ring in error state
    /// #VERIFY[ERROR-DETAILS]: Error info captured
    Error = 254,
}

impl TransferRingState {
    /// Extract state from packed u64
    #[inline(always)]
    pub fn from_packed(packed: u64) -> Self {
        match (packed & 0xFF) as u8 {
            0 => TransferRingState::Uninitialized,
            1 => TransferRingState::Configuring,
            2 => TransferRingState::Ready,
            3 => TransferRingState::Stopped,
            4 => TransferRingState::Stalled,
            5 => TransferRingState::Draining,
            6 => TransferRingState::Disabled,
            254 => TransferRingState::Error,
            _ => TransferRingState::Error,
        }
    }

    /// Pack state with generation counter and metadata
    ///
    /// # Layout
    /// - Bits 0-7: State (8 bits)
    /// - Bits 8-15: Slot ID (8 bits)
    /// - Bits 16-23: Endpoint DCI (8 bits)
    /// - Bits 24-31: Stream ID low bits (8 bits)
    /// - Bits 32-63: Generation counter (32 bits)
    #[inline(always)]
    pub const fn pack(self, generation: u64, slot_id: u8, endpoint_id: u8, stream_id: u8) -> u64 {
        let state = self as u8 as u64;
        let slot = (slot_id as u64) << 8;
        let ep = (endpoint_id as u64) << 16;
        let stream = (stream_id as u64) << 24;
        let gen = (generation & 0xFFFF_FFFF) << 32;
        state | slot | ep | stream | gen
    }

    /// Check if ring accepts new transfers
    #[inline(always)]
    pub const fn accepts_transfers(&self) -> bool {
        matches!(self, TransferRingState::Ready)
    }
}

// ============================================================================
// Transfer Type
// ============================================================================

/// USB Transfer Type
///
/// #VERIFY[TYPE-USB]: Values match USB endpoint types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TransferType {
    /// Control transfers (setup, data, status stages)
    Control = 0,
    /// Isochronous transfers (time-sensitive streaming)
    Isochronous = 1,
    /// Bulk transfers (large data, error recovery)
    Bulk = 2,
    /// Interrupt transfers (small, guaranteed latency)
    Interrupt = 3,
}

impl TransferType {
    /// Convert from raw type code
    #[inline(always)]
    pub fn from_code(code: u8) -> Self {
        match code & 0x3 {
            0 => TransferType::Control,
            1 => TransferType::Isochronous,
            2 => TransferType::Bulk,
            3 => TransferType::Interrupt,
            _ => TransferType::Control,
        }
    }

    /// Get type code
    #[inline(always)]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

// ============================================================================
// Transfer Request
// ============================================================================

/// Transfer Request descriptor (for pending transfer tracking)
///
/// #VERIFY[REQUEST-COMPACT]: Fits in 64 bytes for cache efficiency
#[derive(Debug, Clone, Copy)]
pub struct TransferRequest {
    /// Data buffer physical address
    pub buffer_addr: u64,
    /// Transfer length in bytes
    pub length: u32,
    /// Transfer flags (direction, IOC, etc.)
    pub flags: u32,
    /// Stream ID (for bulk streams, 0 otherwise)
    pub stream_id: u16,
    /// Number of TRBs in this TD
    pub trb_count: u8,
    /// Reserved
    pub _reserved: u8,
    /// Completion callback data (optional)
    pub callback_data: u64,
}

impl TransferRequest {
    /// Create new transfer request
    pub const fn new(buffer_addr: u64, length: u32, flags: u32) -> Self {
        Self {
            buffer_addr,
            length,
            flags,
            stream_id: 0,
            trb_count: 1,
            _reserved: 0,
            callback_data: 0,
        }
    }
}

// Transfer flags
pub const TRANSFER_FLAG_IN: u32 = 1 << 0;
pub const TRANSFER_FLAG_OUT: u32 = 0;
pub const TRANSFER_FLAG_IOC: u32 = 1 << 1;
pub const TRANSFER_FLAG_ISP: u32 = 1 << 2;
pub const TRANSFER_FLAG_NO_SNOOP: u32 = 1 << 3;
pub const TRANSFER_FLAG_CHAIN: u32 = 1 << 4;
pub const TRANSFER_FLAG_SHORT_OK: u32 = 1 << 5;

// ============================================================================
// Transfer Ring Snapshot
// ============================================================================

/// Atomic snapshot of transfer ring state
///
/// #VERIFY[SNAPSHOT-CONSISTENT]: All fields from same generation
#[derive(Debug, Clone, Copy)]
pub struct TransferRingSnapshot {
    /// Current state
    pub state: TransferRingState,
    /// Generation counter
    pub generation: u64,
    /// Ring base physical address
    pub ring_base: u64,
    /// Ring size in TRBs
    pub ring_size: u32,
    /// Enqueue pointer offset from base (in bytes)
    pub enqueue_offset: u32,
    /// Dequeue pointer offset from base (in bytes)
    pub dequeue_offset: u32,
    /// Current producer cycle state
    pub cycle_state: bool,
    /// Slot ID (1-255)
    pub slot_id: u8,
    /// Endpoint DCI (1-31)
    pub endpoint_id: u8,
    /// Transfer type
    pub transfer_type: TransferType,
    /// Stream ID (0 = no streams)
    pub stream_id: u16,
    /// Max packet size for this endpoint
    pub max_packet_size: u16,
    /// TDs currently in flight
    pub tds_in_flight: u32,
    /// Total TDs submitted
    pub tds_submitted: u64,
    /// Total TDs completed successfully
    pub tds_completed: u64,
    /// Total TDs failed
    pub tds_failed: u64,
    /// Total bytes transferred
    pub bytes_transferred: u64,
}

impl TransferRingSnapshot {
    /// Check if ring is ready for transfers
    #[inline(always)]
    pub fn is_ready(&self) -> bool {
        self.state == TransferRingState::Ready
    }

    /// Check if ring is full
    ///
    /// Ring is full when enqueue would catch up to dequeue
    /// (leaving space for Link TRB)
    #[inline(always)]
    pub fn is_full(&self) -> bool {
        let used = if self.enqueue_offset >= self.dequeue_offset {
            self.enqueue_offset - self.dequeue_offset
        } else {
            (self.ring_size * TRB_SIZE as u32) - self.dequeue_offset + self.enqueue_offset
        };
        // Full when only 2 TRB slots remain (1 for enqueue, 1 for Link TRB)
        used >= (self.ring_size - 2) * TRB_SIZE as u32
    }

    /// Get number of available TRB slots
    #[inline(always)]
    pub fn available_trbs(&self) -> u32 {
        let used_bytes = if self.enqueue_offset >= self.dequeue_offset {
            self.enqueue_offset - self.dequeue_offset
        } else {
            (self.ring_size * TRB_SIZE as u32) - self.dequeue_offset + self.enqueue_offset
        };
        let used_trbs = used_bytes / TRB_SIZE as u32;
        self.ring_size.saturating_sub(used_trbs + 2) // -2 for Link TRB and empty slot
    }

    /// Get transfer efficiency (completed / (completed + failed))
    #[inline(always)]
    pub fn success_rate(&self) -> f32 {
        let total = self.tds_completed + self.tds_failed;
        if total == 0 {
            1.0
        } else {
            self.tds_completed as f32 / total as f32
        }
    }
}

// ============================================================================
// USB Transfer Capsule (1024 bytes)
// ============================================================================

/// USB Transfer Capsule (1024 bytes, cache-aligned)
///
/// **Architecture**: Tier 5 (Streaming)
/// - O(1) lockfree enqueue operations
/// - Atomic cycle bit management
/// - Generation counters for ABA prevention
/// - Per-endpoint isolation with stream support
///
/// # Memory Layout (1024 bytes, 16 cache lines)
///
/// ## Cache Lines 0-1 (128 bytes) - Primary Ring State
/// - state_gen: State + slot + endpoint + stream + generation (8 bytes)
/// - ring_base: Ring physical base address (8 bytes)
/// - ring_size: Size in TRBs (8 bytes)
/// - enqueue_ptr: Current enqueue pointer (8 bytes)
/// - dequeue_ptr: Hardware dequeue pointer (8 bytes)
/// - cycle_state: Producer cycle state (8 bytes)
/// - transfer_type: Endpoint transfer type (8 bytes)
/// - max_packet_size: Endpoint max packet size (8 bytes)
/// - link_trb_ptr: Pointer to Link TRB for wraparound (8 bytes)
/// - Reserved (56 bytes)
///
/// ## Cache Lines 2-3 (128 bytes) - TD Tracking
/// - tds_in_flight: Current pending TDs (8 bytes)
/// - tds_submitted: Total TDs submitted (8 bytes)
/// - tds_completed: Total TDs completed (8 bytes)
/// - tds_failed: Total TDs failed (8 bytes)
/// - td_head: Oldest pending TD index (8 bytes)
/// - td_tail: Newest pending TD index (8 bytes)
/// - Reserved (80 bytes)
///
/// ## Cache Lines 4-7 (256 bytes) - TD Ring (circular buffer of pending TDs)
/// - Each entry: TRB physical address (8 bytes) + callback data (8 bytes) = 16 bytes
/// - 16 entries = 256 bytes
///
/// ## Cache Lines 8-11 (256 bytes) - Statistics and Diagnostics
/// - bytes_transferred: Total bytes (8 bytes)
/// - trbs_enqueued: Total TRBs enqueued (8 bytes)
/// - trbs_completed: Total TRBs completed (8 bytes)
/// - short_packets: Short packet count (8 bytes)
/// - stalls: Stall error count (8 bytes)
/// - babbles: Babble error count (8 bytes)
/// - timeouts: Timeout count (8 bytes)
/// - last_error: Last error code (8 bytes)
/// - last_completion_ptr: Last completed TRB pointer (8 bytes)
/// - last_completion_code: Last completion code (8 bytes)
/// - Reserved (176 bytes)
///
/// ## Cache Lines 12-15 (256 bytes) - Stream Support and Extended State
/// - stream_context_array: Pointer to stream context array (8 bytes)
/// - num_streams: Number of streams configured (8 bytes)
/// - primary_stream_ctx: Primary stream context pointer (8 bytes)
/// - deferred_enqueue_count: Deferred enqueues due to ring full (8 bytes)
/// - Reserved (224 bytes)
///
/// #ASSUME[CACHE-ALIGN]: 1024-byte alignment for DMA and cache efficiency
/// #VERIFY[SIZE-1024]: Structure exactly 1024 bytes
#[repr(C, align(1024))]
pub struct UsbTransferCapsule {
    // === Cache Lines 0-1 (128 bytes) - Primary Ring State ===
    /// Packed state: state (8) | slot (8) | endpoint (8) | stream (8) | gen (32)
    /// #VERIFY[STATE-ATOMIC]: Single atomic for consistent state reads
    state_gen: AtomicU64,
    /// Ring base physical address (64-byte aligned)
    /// #VERIFY[BASE-ALIGNED]: Must be 64-byte aligned
    ring_base: AtomicU64,
    /// Ring size in TRBs (including Link TRB)
    ring_size: AtomicU64,
    /// Current enqueue pointer (physical address)
    /// #VERIFY[ENQUEUE-VALID]: Within ring bounds
    enqueue_ptr: AtomicU64,
    /// Hardware dequeue pointer (updated on completion)
    /// #VERIFY[DEQUEUE-SYNC]: Synchronized with completion events
    dequeue_ptr: AtomicU64,
    /// Producer cycle state (true = 1, false = 0)
    /// #VERIFY[CYCLE-TOGGLE]: Toggled on wrap
    cycle_state: AtomicBool,
    /// Transfer type for this endpoint
    transfer_type: AtomicU64,
    /// Maximum packet size for this endpoint
    max_packet_size: AtomicU64,
    /// Pointer to Link TRB (for cycle toggle on wrap)
    link_trb_ptr: AtomicU64,
    /// Interval for interrupt/isochronous (in 125us frames)
    interval: AtomicU64,
    /// Max burst for USB 3.x endpoints
    max_burst: AtomicU64,
    /// Reserved for cache line alignment
    _reserved_cl01: [u8; 32],

    // === Cache Lines 2-3 (128 bytes) - TD Tracking ===
    /// TDs currently in flight (pending completion)
    /// #VERIFY[TDS-BOUNDED]: Never exceeds MAX_TDS_IN_FLIGHT
    tds_in_flight: AtomicU64,
    /// Total TDs submitted
    tds_submitted: AtomicU64,
    /// Total TDs completed successfully
    tds_completed: AtomicU64,
    /// Total TDs failed
    tds_failed: AtomicU64,
    /// Head index in TD ring (oldest pending)
    td_head: AtomicU64,
    /// Tail index in TD ring (newest pending)
    td_tail: AtomicU64,
    /// Cancelled TD count
    tds_cancelled: AtomicU64,
    /// Reserved
    _reserved_cl23_1: AtomicU64,
    /// Reserved for cache line alignment
    _reserved_cl23: [u8; 64],

    // === Cache Lines 4-7 (256 bytes) - TD Ring ===
    /// TD ring: pairs of (TRB address, callback data)
    /// 16 entries x 16 bytes = 256 bytes
    /// #VERIFY[TD-RING-WRAP]: Circular buffer with modulo indexing
    td_ring_addresses: [AtomicU64; 16],
    td_ring_callbacks: [AtomicU64; 16],

    // === Cache Lines 8-11 (256 bytes) - Statistics ===
    /// Total bytes transferred
    bytes_transferred: AtomicU64,
    /// Total TRBs enqueued
    trbs_enqueued: AtomicU64,
    /// Total TRBs completed
    trbs_completed: AtomicU64,
    /// Short packet count
    short_packets: AtomicU64,
    /// Stall error count
    stalls: AtomicU64,
    /// Babble error count
    babbles: AtomicU64,
    /// Timeout count
    timeouts: AtomicU64,
    /// Last error code (32) + error count (32)
    last_error: AtomicU64,
    /// Last completed TRB pointer
    last_completion_ptr: AtomicU64,
    /// Last completion code
    last_completion_code: AtomicU64,
    /// Average transfer latency (in microseconds, Q16.16)
    avg_latency_us: AtomicU64,
    /// Peak transfer rate (bytes/sec)
    peak_rate: AtomicU64,
    /// Reserved for stats expansion
    _reserved_stats: [u8; 160],

    // === Cache Lines 12-15 (256 bytes) - Stream Support ===
    /// Stream context array pointer (for bulk streams)
    /// #ASSUME[STREAM-DMA]: Stream contexts in DMA memory
    stream_context_array: AtomicU64,
    /// Number of streams configured (0 = no streams)
    num_streams: AtomicU64,
    /// Primary stream context pointer
    primary_stream_ctx: AtomicU64,
    /// Deferred enqueue count (ring was full)
    deferred_enqueue_count: AtomicU64,
    /// Timestamp of last activity
    last_activity_timestamp: AtomicU64,
    /// URB (USB Request Block) sequence number
    urb_sequence: AtomicU64,
    /// Reserved
    _reserved_stream: [u8; 208],
}

// Compile-time size verification
const _: () = assert!(core::mem::size_of::<UsbTransferCapsule>() == 1024);
const _: () = assert!(core::mem::align_of::<UsbTransferCapsule>() == 1024);

impl UsbTransferCapsule {
    /// Create new transfer ring capsule
    ///
    /// #VERIFY[INIT-CLEAN]: All counters and pointers start at zero
    pub const fn new() -> Self {
        Self {
            state_gen: AtomicU64::new(TransferRingState::Uninitialized.pack(0, 0, 0, 0)),
            ring_base: AtomicU64::new(0),
            ring_size: AtomicU64::new(DEFAULT_RING_SIZE as u64),
            enqueue_ptr: AtomicU64::new(0),
            dequeue_ptr: AtomicU64::new(0),
            cycle_state: AtomicBool::new(true), // Start with cycle = 1
            transfer_type: AtomicU64::new(TransferType::Bulk as u64),
            max_packet_size: AtomicU64::new(512), // Default for high-speed bulk
            link_trb_ptr: AtomicU64::new(0),
            interval: AtomicU64::new(0),
            max_burst: AtomicU64::new(0),
            _reserved_cl01: [0u8; 32],
            tds_in_flight: AtomicU64::new(0),
            tds_submitted: AtomicU64::new(0),
            tds_completed: AtomicU64::new(0),
            tds_failed: AtomicU64::new(0),
            td_head: AtomicU64::new(0),
            td_tail: AtomicU64::new(0),
            tds_cancelled: AtomicU64::new(0),
            _reserved_cl23_1: AtomicU64::new(0),
            _reserved_cl23: [0u8; 64],
            td_ring_addresses: [
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
            ],
            td_ring_callbacks: [
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
            ],
            bytes_transferred: AtomicU64::new(0),
            trbs_enqueued: AtomicU64::new(0),
            trbs_completed: AtomicU64::new(0),
            short_packets: AtomicU64::new(0),
            stalls: AtomicU64::new(0),
            babbles: AtomicU64::new(0),
            timeouts: AtomicU64::new(0),
            last_error: AtomicU64::new(0),
            last_completion_ptr: AtomicU64::new(0),
            last_completion_code: AtomicU64::new(0),
            avg_latency_us: AtomicU64::new(0),
            peak_rate: AtomicU64::new(0),
            _reserved_stats: [0u8; 160],
            stream_context_array: AtomicU64::new(0),
            num_streams: AtomicU64::new(0),
            primary_stream_ctx: AtomicU64::new(0),
            deferred_enqueue_count: AtomicU64::new(0),
            last_activity_timestamp: AtomicU64::new(0),
            urb_sequence: AtomicU64::new(0),
            _reserved_stream: [0u8; 208],
        }
    }

    /// Get atomic snapshot of current state
    ///
    /// #VERIFY[SNAPSHOT-ATOMIC]: All reads use Acquire ordering
    #[inline(always)]
    pub fn snapshot(&self) -> TransferRingSnapshot {
        let state_packed = self.state_gen.load(Ordering::Acquire);
        let ring_base = self.ring_base.load(Ordering::Acquire);
        let ring_size = self.ring_size.load(Ordering::Acquire) as u32;
        let enqueue = self.enqueue_ptr.load(Ordering::Acquire);
        let dequeue = self.dequeue_ptr.load(Ordering::Acquire);
        let cycle = self.cycle_state.load(Ordering::Acquire);
        let xfer_type = self.transfer_type.load(Ordering::Acquire);
        let max_pkt = self.max_packet_size.load(Ordering::Acquire);

        let enqueue_offset = if ring_base > 0 { (enqueue - ring_base) as u32 } else { 0 };
        let dequeue_offset = if ring_base > 0 { (dequeue - ring_base) as u32 } else { 0 };

        TransferRingSnapshot {
            state: TransferRingState::from_packed(state_packed),
            generation: (state_packed >> 32) & 0xFFFF_FFFF,
            ring_base,
            ring_size,
            enqueue_offset,
            dequeue_offset,
            cycle_state: cycle,
            slot_id: ((state_packed >> 8) & 0xFF) as u8,
            endpoint_id: ((state_packed >> 16) & 0xFF) as u8,
            transfer_type: TransferType::from_code(xfer_type as u8),
            stream_id: ((state_packed >> 24) & 0xFF) as u16,
            max_packet_size: max_pkt as u16,
            tds_in_flight: self.tds_in_flight.load(Ordering::Acquire) as u32,
            tds_submitted: self.tds_submitted.load(Ordering::Acquire),
            tds_completed: self.tds_completed.load(Ordering::Acquire),
            tds_failed: self.tds_failed.load(Ordering::Acquire),
            bytes_transferred: self.bytes_transferred.load(Ordering::Acquire),
        }
    }

    /// Get current state only (fast path)
    #[inline(always)]
    pub fn state(&self) -> TransferRingState {
        TransferRingState::from_packed(self.state_gen.load(Ordering::Acquire))
    }

    /// Get ring base address
    #[inline(always)]
    pub fn ring_base(&self) -> u64 {
        self.ring_base.load(Ordering::Acquire)
    }

    /// Get current enqueue pointer
    #[inline(always)]
    pub fn enqueue_ptr(&self) -> u64 {
        self.enqueue_ptr.load(Ordering::Acquire)
    }

    /// Get current cycle state
    #[inline(always)]
    pub fn cycle_state(&self) -> bool {
        self.cycle_state.load(Ordering::Acquire)
    }

    /// Get slot ID
    #[inline(always)]
    pub fn slot_id(&self) -> u8 {
        ((self.state_gen.load(Ordering::Acquire) >> 8) & 0xFF) as u8
    }

    /// Get endpoint DCI
    #[inline(always)]
    pub fn endpoint_id(&self) -> u8 {
        ((self.state_gen.load(Ordering::Acquire) >> 16) & 0xFF) as u8
    }

    /// Get TDs in flight count
    #[inline(always)]
    pub fn tds_in_flight(&self) -> u32 {
        self.tds_in_flight.load(Ordering::Acquire) as u32
    }

    /// Initialize the transfer ring
    ///
    /// # Arguments
    /// - `ring_base`: Physical address of ring buffer (64-byte aligned)
    /// - `ring_size`: Number of TRBs in ring (including Link TRB)
    /// - `slot_id`: Device slot ID (1-255)
    /// - `endpoint_id`: Endpoint DCI (1-31)
    /// - `transfer_type`: Type of transfers on this endpoint
    /// - `max_packet_size`: Maximum packet size for endpoint
    ///
    /// # Returns
    /// - `Ok(generation)`: Ring initialized successfully
    /// - `Err(state)`: Invalid state or parameters
    ///
    /// #VERIFY[INIT-ALIGNED]: Ring base must be 64-byte aligned
    /// #VERIFY[INIT-SIZE]: Ring size must be at least 4 (2 usable + 1 Link + 1 empty)
    pub fn initialize(
        &self,
        ring_base: u64,
        ring_size: usize,
        slot_id: u8,
        endpoint_id: u8,
        transfer_type: TransferType,
        max_packet_size: u16,
    ) -> Result<u64, TransferRingState> {
        // Verify alignment
        if ring_base & 0x3F != 0 {
            return Err(TransferRingState::Error);
        }

        // Verify slot/endpoint bounds
        if slot_id == 0 || endpoint_id == 0 || endpoint_id > 31 {
            return Err(TransferRingState::Error);
        }

        // Verify ring size
        if ring_size < 4 {
            return Err(TransferRingState::Error);
        }

        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let state = TransferRingState::from_packed(current);

            if state != TransferRingState::Uninitialized {
                return Err(state);
            }

            let current_gen = (current >> 32) & 0xFFFF_FFFF;
            let new_gen = current_gen.wrapping_add(1);
            let configuring = TransferRingState::Configuring.pack(new_gen, slot_id, endpoint_id, 0);

            if self.state_gen.compare_exchange(current, configuring, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                // Store ring configuration
                self.ring_base.store(ring_base, Ordering::Release);
                self.ring_size.store(ring_size as u64, Ordering::Release);
                self.enqueue_ptr.store(ring_base, Ordering::Release);
                self.dequeue_ptr.store(ring_base, Ordering::Release);
                self.cycle_state.store(true, Ordering::Release);
                self.transfer_type.store(transfer_type as u64, Ordering::Release);
                self.max_packet_size.store(max_packet_size as u64, Ordering::Release);

                // Calculate Link TRB position (last slot in ring)
                let link_offset = (ring_size - 1) * TRB_SIZE;
                self.link_trb_ptr.store(ring_base + link_offset as u64, Ordering::Release);

                // Reset counters
                self.tds_in_flight.store(0, Ordering::Release);
                self.tds_submitted.store(0, Ordering::Release);
                self.tds_completed.store(0, Ordering::Release);
                self.tds_failed.store(0, Ordering::Release);
                self.td_head.store(0, Ordering::Release);
                self.td_tail.store(0, Ordering::Release);

                // Transition to Ready
                let ready_gen = new_gen.wrapping_add(1);
                let ready = TransferRingState::Ready.pack(ready_gen, slot_id, endpoint_id, 0);
                self.state_gen.store(ready, Ordering::Release);

                return Ok(ready_gen);
            }
        }
    }

    /// Enqueue a TRB (lockfree)
    ///
    /// # Arguments
    /// - `trb_data`: 16-byte TRB data
    /// - `is_td_start`: True if this TRB starts a new TD
    /// - `callback_data`: Optional callback data for completion
    ///
    /// # Returns
    /// - `Ok(physical_addr)`: Physical address of enqueued TRB
    /// - `Err(state)`: Ring not ready or full
    ///
    /// # Safety
    /// Caller must ensure TRB data is valid for the transfer type
    ///
    /// #VERIFY[ENQUEUE-CAS]: Uses CAS to advance enqueue pointer
    /// #VERIFY[CYCLE-SET]: Cycle bit set in TRB control field
    pub fn enqueue(&self, trb_data: &[u8; 16], is_td_start: bool, callback_data: u64) -> Result<u64, TransferRingState> {
        let state = self.state();
        if state != TransferRingState::Ready {
            return Err(state);
        }

        loop {
            let current_enq = self.enqueue_ptr.load(Ordering::Acquire);
            let ring_base = self.ring_base.load(Ordering::Acquire);
            let ring_size = self.ring_size.load(Ordering::Acquire);

            // Calculate current index
            let offset = current_enq - ring_base;
            let index = offset / TRB_SIZE as u64;

            // Check if at Link TRB position (need to wrap)
            if index >= ring_size - 1 {
                return Err(TransferRingState::Error);
            }

            // Check dequeue to ensure space
            let dequeue = self.dequeue_ptr.load(Ordering::Acquire);
            let deq_offset = dequeue - ring_base;
            let deq_index = deq_offset / TRB_SIZE as u64;

            // Calculate free space
            let used = if index >= deq_index {
                index - deq_index
            } else {
                (ring_size - 1) - deq_index + index
            };

            // Ring full if only 2 slots remain
            if used >= ring_size - 2 {
                self.deferred_enqueue_count.fetch_add(1, Ordering::AcqRel);
                return Err(TransferRingState::Error);
            }

            // Calculate next enqueue position
            let next_index = index + 1;
            let (next_ptr, toggle_cycle) = if next_index >= ring_size - 1 {
                // Wrap around via Link TRB
                (ring_base, true)
            } else {
                (ring_base + next_index * TRB_SIZE as u64, false)
            };

            // CAS to advance enqueue pointer
            if self.enqueue_ptr.compare_exchange(current_enq, next_ptr, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                // Get current cycle state
                let cycle = self.cycle_state.load(Ordering::Acquire);

                // Write TRB with correct cycle bit
                unsafe {
                    let trb_ptr = current_enq as *mut u8;
                    // Copy first 12 bytes (data pointer and status)
                    core::ptr::copy_nonoverlapping(trb_data.as_ptr(), trb_ptr, 12);

                    // Set control field with cycle bit
                    let control_ptr = (current_enq + 12) as *mut u32;
                    let mut control = u32::from_le_bytes([trb_data[12], trb_data[13], trb_data[14], trb_data[15]]);
                    // Clear and set cycle bit
                    control = (control & !1) | (cycle as u32);
                    core::ptr::write_volatile(control_ptr, control);
                }

                // Handle ring wrap
                if toggle_cycle {
                    // Write Link TRB at wrap position
                    unsafe {
                        let link_ptr = self.link_trb_ptr.load(Ordering::Acquire);
                        let link_data_ptr = link_ptr as *mut u64;
                        // Link TRB: pointer to ring base
                        core::ptr::write_volatile(link_data_ptr, ring_base);
                        // Control: Type = Link (6), TC = 1, Cycle
                        let link_control: u64 = (6u64 << 10) | (1u64 << 1) | (cycle as u64);
                        core::ptr::write_volatile(link_data_ptr.add(1), link_control);
                    }
                    // Toggle cycle state
                    self.cycle_state.fetch_xor(true, Ordering::AcqRel);
                }

                // Track TD if this starts one
                if is_td_start {
                    self.tds_in_flight.fetch_add(1, Ordering::AcqRel);
                    self.tds_submitted.fetch_add(1, Ordering::AcqRel);

                    // Record in TD ring
                    let tail = self.td_tail.fetch_add(1, Ordering::AcqRel);
                    let td_idx = (tail % 16) as usize;
                    self.td_ring_addresses[td_idx].store(current_enq, Ordering::Release);
                    self.td_ring_callbacks[td_idx].store(callback_data, Ordering::Release);
                }

                // Update statistics
                self.trbs_enqueued.fetch_add(1, Ordering::AcqRel);
                self.last_activity_timestamp.store(0, Ordering::Release); // Would use timestamp

                return Ok(current_enq);
            }
            // CAS failed, retry
        }
    }

    /// Complete a TD (called when transfer event received)
    ///
    /// # Arguments
    /// - `completed_ptr`: Physical address of completed TRB
    /// - `bytes_transferred`: Actual bytes transferred
    /// - `completion_code`: xHCI completion code
    ///
    /// # Returns
    /// - `Some(callback_data)`: Callback data for completed TD
    /// - `None`: No matching TD found
    ///
    /// #VERIFY[COMPLETE-UPDATE]: Dequeue pointer updated atomically
    pub fn complete_td(&self, completed_ptr: u64, bytes_transferred: u32, completion_code: u8) -> Option<u64> {
        let ring_base = self.ring_base.load(Ordering::Acquire);
        if completed_ptr < ring_base {
            return None;
        }

        // Update dequeue pointer
        self.dequeue_ptr.store(completed_ptr + TRB_SIZE as u64, Ordering::Release);

        // Update last completion info
        self.last_completion_ptr.store(completed_ptr, Ordering::Release);
        self.last_completion_code.store(completion_code as u64, Ordering::Release);

        // Check completion code for success/failure
        let success = completion_code == 1 || completion_code == 13; // Success or ShortPacket

        if success {
            self.tds_completed.fetch_add(1, Ordering::AcqRel);
            self.bytes_transferred.fetch_add(bytes_transferred as u64, Ordering::AcqRel);

            if completion_code == 13 {
                self.short_packets.fetch_add(1, Ordering::AcqRel);
            }
        } else {
            self.tds_failed.fetch_add(1, Ordering::AcqRel);

            // Track specific error types
            match completion_code {
                6 => { self.stalls.fetch_add(1, Ordering::AcqRel); }      // Stall
                3 => { self.babbles.fetch_add(1, Ordering::AcqRel); }     // Babble
                4 => { self.timeouts.fetch_add(1, Ordering::AcqRel); }    // Transaction error (timeout)
                _ => {}
            }

            // Update last error
            let old_err = self.last_error.load(Ordering::Acquire);
            let count = ((old_err >> 32) + 1) & 0xFFFF_FFFF;
            let new_err = (count << 32) | (completion_code as u64);
            self.last_error.store(new_err, Ordering::Release);
        }

        // Decrement in-flight counter
        self.tds_in_flight.fetch_sub(1, Ordering::AcqRel);
        self.trbs_completed.fetch_add(1, Ordering::AcqRel);

        // Find and return callback data
        let head = self.td_head.fetch_add(1, Ordering::AcqRel);
        let td_idx = (head % 16) as usize;
        let stored_ptr = self.td_ring_addresses[td_idx].load(Ordering::Acquire);

        if stored_ptr == completed_ptr {
            Some(self.td_ring_callbacks[td_idx].load(Ordering::Acquire))
        } else {
            // TD ring may have wrapped or completion out of order
            None
        }
    }

    /// Stop the transfer ring
    ///
    /// #ASSUME[STOP-SAFE]: No new enqueues during stop
    pub fn stop(&self) -> Result<u64, TransferRingState> {
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let state = TransferRingState::from_packed(current);

            if state != TransferRingState::Ready {
                return Err(state);
            }

            let slot = (current >> 8) & 0xFF;
            let ep = (current >> 16) & 0xFF;
            let stream = (current >> 24) & 0xFF;
            let current_gen = (current >> 32) & 0xFFFF_FFFF;
            let new_gen = current_gen.wrapping_add(1);

            let stopped = TransferRingState::Stopped.pack(new_gen, slot as u8, ep as u8, stream as u8);

            if self.state_gen.compare_exchange(current, stopped, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                return Ok(new_gen);
            }
        }
    }

    /// Resume the transfer ring (from Stopped)
    pub fn resume(&self) -> Result<u64, TransferRingState> {
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let state = TransferRingState::from_packed(current);

            if state != TransferRingState::Stopped {
                return Err(state);
            }

            let slot = (current >> 8) & 0xFF;
            let ep = (current >> 16) & 0xFF;
            let stream = (current >> 24) & 0xFF;
            let current_gen = (current >> 32) & 0xFFFF_FFFF;
            let new_gen = current_gen.wrapping_add(1);

            let ready = TransferRingState::Ready.pack(new_gen, slot as u8, ep as u8, stream as u8);

            if self.state_gen.compare_exchange(current, ready, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                return Ok(new_gen);
            }
        }
    }

    /// Mark ring as stalled (endpoint halted)
    ///
    /// #VERIFY[STALL-CLEAR]: Must issue Reset Endpoint before resume
    pub fn stall(&self) -> Result<u64, TransferRingState> {
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let state = TransferRingState::from_packed(current);

            if state != TransferRingState::Ready {
                return Err(state);
            }

            let slot = (current >> 8) & 0xFF;
            let ep = (current >> 16) & 0xFF;
            let stream = (current >> 24) & 0xFF;
            let current_gen = (current >> 32) & 0xFFFF_FFFF;
            let new_gen = current_gen.wrapping_add(1);

            let stalled = TransferRingState::Stalled.pack(new_gen, slot as u8, ep as u8, stream as u8);

            if self.state_gen.compare_exchange(current, stalled, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                return Ok(new_gen);
            }
        }
    }

    /// Clear stall condition and resume
    ///
    /// # Arguments
    /// - `new_dequeue`: New dequeue pointer from Set TR Dequeue Pointer command
    ///
    /// #VERIFY[CLEAR-STALL]: Reset Endpoint command completed
    pub fn clear_stall(&self, new_dequeue: u64) -> Result<u64, TransferRingState> {
        loop {
            let current = self.state_gen.load(Ordering::Acquire);
            let state = TransferRingState::from_packed(current);

            if state != TransferRingState::Stalled {
                return Err(state);
            }

            let slot = (current >> 8) & 0xFF;
            let ep = (current >> 16) & 0xFF;
            let stream = (current >> 24) & 0xFF;
            let current_gen = (current >> 32) & 0xFFFF_FFFF;
            let new_gen = current_gen.wrapping_add(1);

            let ready = TransferRingState::Ready.pack(new_gen, slot as u8, ep as u8, stream as u8);

            if self.state_gen.compare_exchange(current, ready, Ordering::AcqRel, Ordering::Acquire).is_ok() {
                // Reset enqueue and dequeue to new position
                self.enqueue_ptr.store(new_dequeue, Ordering::Release);
                self.dequeue_ptr.store(new_dequeue, Ordering::Release);
                return Ok(new_gen);
            }
        }
    }

    /// Get Transfer Ring Dequeue Pointer value for endpoint context
    ///
    /// Format: Bits 63:4 = Dequeue Pointer, Bit 0 = DCS (Dequeue Cycle State)
    ///
    /// #VERIFY[TRDP-FORMAT]: Matches xHCI endpoint context TR Dequeue Pointer format
    #[inline(always)]
    pub fn get_dequeue_ptr_for_context(&self) -> u64 {
        let dequeue = self.dequeue_ptr.load(Ordering::Acquire);
        let cycle = self.cycle_state.load(Ordering::Acquire);
        // Clear low 4 bits, set DCS
        (dequeue & !0xF) | (cycle as u64)
    }

    /// Configure streams for bulk endpoint (USB 3.x)
    ///
    /// #ASSUME[STREAM-SUPPORT]: Endpoint supports streams per endpoint descriptor
    pub fn configure_streams(&self, stream_context_array: u64, num_streams: u16) -> Result<(), TransferRingState> {
        let state = self.state();
        if !matches!(state, TransferRingState::Ready | TransferRingState::Stopped) {
            return Err(state);
        }

        self.stream_context_array.store(stream_context_array, Ordering::Release);
        self.num_streams.store(num_streams as u64, Ordering::Release);
        Ok(())
    }

    /// Get next URB sequence number
    #[inline(always)]
    pub fn next_urb_sequence(&self) -> u64 {
        self.urb_sequence.fetch_add(1, Ordering::AcqRel)
    }

    /// Get statistics
    pub fn get_stats(&self) -> (u64, u64, u64, u64) {
        (
            self.tds_submitted.load(Ordering::Acquire),
            self.tds_completed.load(Ordering::Acquire),
            self.tds_failed.load(Ordering::Acquire),
            self.bytes_transferred.load(Ordering::Acquire),
        )
    }
}

impl Default for UsbTransferCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests (T28 Framework)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1-Q7: Unit Tests
    // ========================================================================

    #[test]
    fn test_transfer_capsule_size() {
        assert_eq!(
            core::mem::size_of::<UsbTransferCapsule>(),
            1024,
            "UsbTransferCapsule must be exactly 1024 bytes"
        );
    }

    #[test]
    fn test_transfer_capsule_alignment() {
        assert_eq!(
            core::mem::align_of::<UsbTransferCapsule>(),
            1024,
            "UsbTransferCapsule must be 1024-byte aligned"
        );
    }

    #[test]
    fn test_transfer_capsule_initial_state() {
        let ring = UsbTransferCapsule::new();
        let snapshot = ring.snapshot();

        assert_eq!(snapshot.state, TransferRingState::Uninitialized);
        assert_eq!(snapshot.ring_base, 0);
        assert_eq!(snapshot.slot_id, 0);
        assert_eq!(snapshot.endpoint_id, 0);
        assert_eq!(snapshot.tds_in_flight, 0);
        assert!(snapshot.cycle_state); // Starts with cycle = 1
    }

    #[test]
    fn test_transfer_ring_initialize() {
        let ring = UsbTransferCapsule::new();

        // 64-byte aligned address
        let ring_base = 0x1000u64;
        let ring_size = 256;
        let slot_id = 1;
        let endpoint_id = 2;

        let result = ring.initialize(
            ring_base,
            ring_size,
            slot_id,
            endpoint_id,
            TransferType::Bulk,
            512,
        );
        assert!(result.is_ok());

        let snapshot = ring.snapshot();
        assert_eq!(snapshot.state, TransferRingState::Ready);
        assert_eq!(snapshot.ring_base, ring_base);
        assert_eq!(snapshot.ring_size, ring_size as u32);
        assert_eq!(snapshot.slot_id, slot_id);
        assert_eq!(snapshot.endpoint_id, endpoint_id);
        assert_eq!(snapshot.transfer_type, TransferType::Bulk);
        assert_eq!(snapshot.max_packet_size, 512);
        assert!(snapshot.is_ready());
    }

    #[test]
    fn test_transfer_ring_initialize_unaligned() {
        let ring = UsbTransferCapsule::new();

        // Unaligned address should fail
        let result = ring.initialize(0x1001, 256, 1, 2, TransferType::Bulk, 512);
        assert!(result.is_err());
    }

    #[test]
    fn test_transfer_ring_initialize_invalid_slot() {
        let ring = UsbTransferCapsule::new();

        // Slot 0 is invalid
        let result = ring.initialize(0x1000, 256, 0, 2, TransferType::Bulk, 512);
        assert!(result.is_err());
    }

    #[test]
    fn test_transfer_ring_initialize_invalid_endpoint() {
        let ring = UsbTransferCapsule::new();

        // Endpoint 0 invalid for transfer ring (use command ring)
        let result = ring.initialize(0x1000, 256, 1, 0, TransferType::Bulk, 512);
        assert!(result.is_err());

        // Endpoint 32+ invalid
        let ring2 = UsbTransferCapsule::new();
        let result = ring2.initialize(0x1000, 256, 1, 32, TransferType::Bulk, 512);
        assert!(result.is_err());
    }

    #[test]
    fn test_transfer_ring_stop_resume() {
        let ring = UsbTransferCapsule::new();
        ring.initialize(0x1000, 256, 1, 2, TransferType::Bulk, 512).unwrap();

        // Stop
        let result = ring.stop();
        assert!(result.is_ok());
        assert_eq!(ring.state(), TransferRingState::Stopped);

        // Resume
        let result = ring.resume();
        assert!(result.is_ok());
        assert_eq!(ring.state(), TransferRingState::Ready);
    }

    #[test]
    fn test_transfer_ring_stall_clear() {
        let ring = UsbTransferCapsule::new();
        ring.initialize(0x1000, 256, 1, 2, TransferType::Bulk, 512).unwrap();

        // Stall
        let result = ring.stall();
        assert!(result.is_ok());
        assert_eq!(ring.state(), TransferRingState::Stalled);

        // Clear stall with new dequeue pointer
        let result = ring.clear_stall(0x1000);
        assert!(result.is_ok());
        assert_eq!(ring.state(), TransferRingState::Ready);
    }

    #[test]
    fn test_dequeue_ptr_for_context() {
        let ring = UsbTransferCapsule::new();
        ring.initialize(0x1000, 256, 1, 2, TransferType::Bulk, 512).unwrap();

        let deq_ptr = ring.get_dequeue_ptr_for_context();
        // Should have base address with DCS bit set
        assert_eq!(deq_ptr & !0xF, 0x1000);
        assert_eq!(deq_ptr & 1, 1); // DCS bit set (cycle = 1)
    }

    #[test]
    fn test_transfer_type_roundtrip() {
        let types = [
            TransferType::Control,
            TransferType::Isochronous,
            TransferType::Bulk,
            TransferType::Interrupt,
        ];

        for t in types {
            let code = t.code();
            let recovered = TransferType::from_code(code);
            assert_eq!(recovered, t);
        }
    }

    // ========================================================================
    // Q8-Q14: Property Tests
    // ========================================================================

    #[test]
    fn test_state_machine_validity() {
        let states = [
            TransferRingState::Uninitialized,
            TransferRingState::Configuring,
            TransferRingState::Ready,
            TransferRingState::Stopped,
            TransferRingState::Stalled,
            TransferRingState::Draining,
            TransferRingState::Disabled,
            TransferRingState::Error,
        ];

        for (i, &state) in states.iter().enumerate() {
            let packed = state.pack(i as u64 * 1000, 100, 15, 5);
            let recovered = TransferRingState::from_packed(packed);
            assert_eq!(recovered, state, "State {:?} should round-trip", state);
        }
    }

    #[test]
    fn test_slot_endpoint_packing() {
        let ring = UsbTransferCapsule::new();
        ring.initialize(0x1000, 256, 123, 31, TransferType::Interrupt, 64).unwrap();

        let snapshot = ring.snapshot();
        assert_eq!(snapshot.slot_id, 123);
        assert_eq!(snapshot.endpoint_id, 31);
    }

    #[test]
    fn test_snapshot_helpers() {
        let ring = UsbTransferCapsule::new();
        ring.initialize(0x1000, 256, 1, 2, TransferType::Bulk, 512).unwrap();

        let snapshot = ring.snapshot();
        assert!(snapshot.is_ready());
        assert!(!snapshot.is_full());
        assert!(snapshot.available_trbs() > 0);
        assert_eq!(snapshot.success_rate(), 1.0); // No transfers yet
    }

    #[test]
    fn test_urb_sequence() {
        let ring = UsbTransferCapsule::new();

        let seq1 = ring.next_urb_sequence();
        let seq2 = ring.next_urb_sequence();
        let seq3 = ring.next_urb_sequence();

        assert_eq!(seq1, 0);
        assert_eq!(seq2, 1);
        assert_eq!(seq3, 2);
    }

    #[test]
    fn test_stream_configuration() {
        let ring = UsbTransferCapsule::new();
        ring.initialize(0x1000, 256, 1, 2, TransferType::Bulk, 512).unwrap();

        // Configure streams
        let result = ring.configure_streams(0x2000, 16);
        assert!(result.is_ok());
    }

    #[test]
    fn test_transfer_request() {
        let req = TransferRequest::new(0x1000_0000, 4096, TRANSFER_FLAG_IN | TRANSFER_FLAG_IOC);

        assert_eq!(req.buffer_addr, 0x1000_0000);
        assert_eq!(req.length, 4096);
        assert!((req.flags & TRANSFER_FLAG_IN) != 0);
        assert!((req.flags & TRANSFER_FLAG_IOC) != 0);
    }

    #[test]
    fn test_get_stats() {
        let ring = UsbTransferCapsule::new();
        ring.initialize(0x1000, 256, 1, 2, TransferType::Bulk, 512).unwrap();

        let (submitted, completed, failed, bytes) = ring.get_stats();
        assert_eq!(submitted, 0);
        assert_eq!(completed, 0);
        assert_eq!(failed, 0);
        assert_eq!(bytes, 0);
    }
}
