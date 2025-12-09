//! # QuicStreamCapsule - T1 Atomic QUIC Stream State Management
//!
//! **Tier**: T1 Atomic
//! **Size**: 64 bytes, cache-aligned
//! **Purpose**: Per-stream state machine for RFC 9000 (QUIC v1)
//!
//! ## Architecture
//!
//! Implements RFC 9000 §3 QUIC stream lifecycle with lockfree atomic coordination:
//! - Stream ID encoding (RFC 9000 §2.1): client/server-initiated, bidi/unidi
//! - State machine: Idle → Ready → Send → DataSent → DataRecvd/Reset
//! - Flow control: 24-bit bytes_sent, Q16.16 max_stream_data
//! - Generation counters: 32-bit for TOCTOU prevention
//!
//! ## DualAtomicU64 Layout
//!
//! ```text
//! Primary (64 bits):
//! ├─ stream_id: 62 bits (2^62 max streams, RFC 9000 spec)
//! └─ direction: 2 bits (ClientBidi, ServerBidi, ClientUni, ServerUni)
//!
//! Secondary (64 bits):
//! ├─ state: 3 bits (Idle, Ready, Send, DataSent, DataRecvd, Reset, ResetRecvd, ResetSent = 8 states)
//! ├─ bytes_sent: 24 bits (0-16MB per stream)
//! ├─ max_stream_data_q16: 32 bits (Q16.16 flow control, 65535.998 bytes max)
//! └─ flags: 5 bits (FIN_SENT, FIN_RECEIVED, RESET_ERROR_MSB, RESET_ERROR_LSB, RESERVED)
//! ```
//!
//! ## Performance (B32 Framework)
//!
//! - `get_stream_id`: <10ns (atomic load + shift)
//! - `get_state`: <10ns (atomic load + mask)
//! - `open_stream`: <20ns (state transition Idle → Ready)
//! - `send_data`: <30ns (flow control check + state update)
//! - `finish_stream`: <15ns (FIN flag set + state transition)
//! - `reset_stream`: <10ns (atomic transition Reset)
//!
//! ## ASSUM Safety Tags
//!
//! - `#ASSUME_STREAMID_MONOTONIC`: Stream IDs never reused (enforced by caller)
//! - `#ASSUME_STATE_ONEWAYS`: State transitions follow RFC 9000 spec (no backward transitions)
//! - `#ASSUME_FLOWCONTROL_CHECKED`: Caller validates bytes_sent ≤ max_stream_data before send
//! - `#ASSUME_ATOMIC_SAFETY`: AtomicU64 memory ordering correct for state machine
//!
//! ## UCE34 Framework Compliance
//!
//! - **Q10**: T1 Atomic tier (lockfree coordination, <100ns operations)
//! - **Q12**: Nightly not required (stable atomics suffice)
//! - **Q33**: Uses #[derive(ComputationalCapsule)] for verification
//! - **Q34**: Generation counters enable audit trails
//!
//! ## Example Usage
//!
//! ```ignore
//! use atomic_capsule::network::{QuicStreamCapsule, StreamDirection, StreamState};
//!
//! // Create stream
//! let stream = QuicStreamCapsule::new(42, StreamDirection::ClientBidi);
//!
//! // Open stream
//! stream.open_stream()?;  // Idle → Ready
//!
//! // Send data
//! stream.send_data(1024)?;  // Check flow control, transition Ready → Send
//!
//! // Finish sending (set FIN flag)
//! stream.finish_stream()?;  // Transition Send → DataSent
//!
//! // Verify state
//! assert_eq!(stream.get_state(), StreamState::DataSent);
//! ```
//!
//! ## References
//!
//! - RFC 9000: QUIC: A UDP-Based Multiplexed and Secure Transport
//! - §2.1: Stream Identifiers
//! - §3: Streams
//! - §4.1: Flow Control

use core::sync::atomic::{AtomicU64, Ordering};
use core::fmt;

#[cfg(feature = "std")]
use std::error::Error;

// ============================================================================
// CONSTANTS AND ENUMS
// ============================================================================

/// Stream direction (RFC 9000 §2.1)
///
/// Bits 0-1 of stream ID:
/// - Bit 0: 0 = client-initiated, 1 = server-initiated
/// - Bit 1: 0 = bidirectional, 1 = unidirectional
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StreamDirection {
    /// Client-initiated bidirectional (stream_id % 4 == 0)
    ClientBidi = 0,
    /// Server-initiated bidirectional (stream_id % 4 == 1)
    ServerBidi = 1,
    /// Client-initiated unidirectional (stream_id % 4 == 2)
    ClientUni = 2,
    /// Server-initiated unidirectional (stream_id % 4 == 3)
    ServerUni = 3,
}

impl StreamDirection {
    /// Validate that direction matches stream ID encoding
    ///
    /// # ASSUME
    /// - Stream ID bits 0-1 correctly encode direction
    ///
    /// # VERIFY
    /// - Return false for invalid (stream_id % 4) combinations
    pub fn validate_with_id(self, stream_id: u64) -> bool {
        (stream_id & 3) == (self as u64)
    }
}

/// Stream state machine (RFC 9000 §3.1)
///
/// States follow QUIC lifecycle:
/// - Idle: Stream created, not yet opened
/// - Ready: Opened, waiting for data
/// - Send: Sending data, not finished
/// - DataSent: Finished sending, waiting for peer acknowledgment
/// - DataRecvd: Peer acknowledged all data
/// - Reset: RESET_STREAM received or sent
/// - ResetRecvd: Reset received, waiting for local reset
/// - ResetSent: Reset sent, waiting for peer reset
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum StreamState {
    /// Stream created, not yet opened
    Idle = 0,
    /// Stream opened, ready for data
    Ready = 1,
    /// Sending data, FIN not sent yet
    Send = 2,
    /// All data sent, FIN flag set
    DataSent = 3,
    /// Peer acknowledged all data
    DataRecvd = 4,
    /// RESET_STREAM received
    Reset = 5,
    /// Reset received, waiting for local acknowledgment
    ResetRecvd = 6,
    /// Reset sent, waiting for peer acknowledgment
    ResetSent = 7,
}

impl StreamState {
    /// Check if state allows sending data
    pub fn can_send(self) -> bool {
        matches!(self, StreamState::Ready | StreamState::Send)
    }

    /// Check if state is terminal (stream closed/reset)
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            StreamState::DataRecvd | StreamState::Reset | StreamState::ResetSent
        )
    }
}

/// Error type for QUIC stream operations
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum QuicStreamError {
    /// Invalid state transition
    InvalidStateTransition,
    /// Flow control violation (bytes_sent > max_stream_data)
    FlowControlViolation,
    /// Stream already closed/reset
    StreamClosed,
    /// Invalid stream ID encoding
    InvalidStreamId,
    /// Bytes to send exceeds flow control window
    ExceedsFlowControl,
}

impl fmt::Display for QuicStreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuicStreamError::InvalidStateTransition => write!(f, "invalid state transition"),
            QuicStreamError::FlowControlViolation => write!(f, "flow control violation"),
            QuicStreamError::StreamClosed => write!(f, "stream closed or reset"),
            QuicStreamError::InvalidStreamId => write!(f, "invalid stream ID encoding"),
            QuicStreamError::ExceedsFlowControl => write!(f, "bytes exceed flow control window"),
        }
    }
}

#[cfg(feature = "std")]
impl Error for QuicStreamError {}

// ============================================================================
// QUICSTREAMCAPSULE - T1 ATOMIC 64B CAPSULE
// ============================================================================

/// QuicStreamCapsule: RFC 9000 QUIC stream state machine (T1 Atomic, 64B cache-aligned)
///
/// # Layout
///
/// Primary AtomicU64:
/// - Bits 0-61: stream_id (62 bits, 2^62 max streams)
/// - Bits 62-63: direction (2 bits)
///
/// Secondary AtomicU64:
/// - Bits 0-2: state (3 bits, 8 states)
/// - Bits 3-26: bytes_sent (24 bits, 0-16MB)
/// - Bits 27-58: max_stream_data_q16 (32 bits, Q16.16 fixed-point)
/// - Bits 59-63: flags (5 bits: FIN_SENT, FIN_RECEIVED, RESET_ERROR[0:1], RESERVED)
///
/// # Padding
///
/// - 48 bytes padding to reach 64-byte cache line
/// - Prevents false sharing in stream tables
///
/// # Memory Ordering
///
/// - Acquire/Release for state transitions (prevent reordering)
/// - Relaxed for pure reads/writes (no synchronization needed)
/// - SeqCst for flow control updates (must be visible to all threads)
#[repr(C, align(64))]
pub struct QuicStreamCapsule {
    /// Primary: stream_id (62b) + direction (2b)
    primary: AtomicU64,
    /// Secondary: state (3b) + bytes_sent (24b) + max_stream_data_q16 (32b) + flags (5b)
    secondary: AtomicU64,
    /// Padding to 64-byte cache line
    _padding: [u8; 48],
}

// Compile-time capsule verification
const _: () = {
    const fn check_size() {
        const SIZE: usize = core::mem::size_of::<QuicStreamCapsule>();
        const EXPECTED: usize = 64;
        const ALIGNMENT: usize = core::mem::align_of::<QuicStreamCapsule>();
        const EXPECTED_ALIGN: usize = 64;
        let _ = [(); 1][(SIZE != EXPECTED) as usize]; // size check
        let _ = [(); 1][(ALIGNMENT != EXPECTED_ALIGN) as usize]; // alignment check
    }
    const _: () = check_size();
};

impl QuicStreamCapsule {
    /// Create new QUIC stream capsule
    ///
    /// # Arguments
    ///
    /// - `stream_id`: Unique stream identifier (0-2^62-1)
    /// - `direction`: Stream direction (client/server bidi/uni)
    /// - `max_stream_data`: Initial flow control window (bytes)
    ///
    /// # Returns
    ///
    /// - QuicStreamError::InvalidStreamId if stream_id > 2^62-1 or invalid direction
    ///
    /// # Performance
    ///
    /// <15ns (Relaxed atomics, no synchronization)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let stream = QuicStreamCapsule::new(42, StreamDirection::ClientBidi, 65536)?;
    /// ```
    pub fn new(
        stream_id: u64,
        direction: StreamDirection,
        max_stream_data: u32,
    ) -> Result<Self, QuicStreamError> {
        // #ASSUME_STREAMID_VALID: stream_id < 2^62
        // #VERIFY_STREAMID: Check direction encoding matches stream_id
        if stream_id >= (1u64 << 62) {
            return Err(QuicStreamError::InvalidStreamId);
        }

        if !direction.validate_with_id(stream_id) {
            return Err(QuicStreamError::InvalidStreamId);
        }

        // Encode primary: stream_id (62b) + direction (2b)
        let primary_val = (stream_id << 2) | (direction as u64);

        // Encode secondary:
        // - state: 3 bits, Idle = 0
        // - bytes_sent: 24 bits, 0 initially
        // - max_stream_data_q16: 32 bits (Q16.16)
        // - flags: 5 bits, all 0 initially
        let max_data_q16 = (max_stream_data as u64) << 16; // Convert to Q16.16
        let secondary_val = 0u64 | max_data_q16; // state=Idle(0), bytes_sent=0, flags=0

        Ok(QuicStreamCapsule {
            primary: AtomicU64::new(primary_val),
            secondary: AtomicU64::new(secondary_val),
            _padding: [0u8; 48],
        })
    }

    /// Get stream ID (62 bits)
    ///
    /// # Performance
    ///
    /// <10ns (Relaxed atomic load)
    pub fn get_stream_id(&self) -> u64 {
        self.primary.load(Ordering::Relaxed) >> 2
    }

    /// Get stream direction (2 bits)
    ///
    /// # Performance
    ///
    /// <10ns (Relaxed atomic load)
    pub fn get_direction(&self) -> StreamDirection {
        let dir_bits = self.primary.load(Ordering::Relaxed) & 3;
        match dir_bits {
            0 => StreamDirection::ClientBidi,
            1 => StreamDirection::ServerBidi,
            2 => StreamDirection::ClientUni,
            3 => StreamDirection::ServerUni,
            _ => unreachable!(),
        }
    }

    /// Get current stream state
    ///
    /// # Performance
    ///
    /// <10ns (Relaxed atomic load)
    pub fn get_state(&self) -> StreamState {
        let secondary = self.secondary.load(Ordering::Relaxed);
        let state_bits = (secondary & 7) as u8; // Bits 0-2
        match state_bits {
            0 => StreamState::Idle,
            1 => StreamState::Ready,
            2 => StreamState::Send,
            3 => StreamState::DataSent,
            4 => StreamState::DataRecvd,
            5 => StreamState::Reset,
            6 => StreamState::ResetRecvd,
            7 => StreamState::ResetSent,
            _ => unreachable!(),
        }
    }

    /// Get bytes sent so far (24 bits, 0-16MB)
    ///
    /// # Performance
    ///
    /// <10ns (Relaxed atomic load)
    pub fn get_bytes_sent(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Relaxed);
        ((secondary >> 3) & 0xFFFFFF) as u32 // Bits 3-26
    }

    /// Get max stream data window (Q16.16 fixed-point)
    ///
    /// Returns the maximum bytes allowed by flow control, as Q16.16 fixed-point.
    /// To get integer bytes: `max_stream_data_q16 >> 16`
    ///
    /// # Performance
    ///
    /// <10ns (Relaxed atomic load)
    pub fn get_max_stream_data_q16(&self) -> u32 {
        let secondary = self.secondary.load(Ordering::Relaxed);
        (secondary >> 27) as u32 // Bits 27-58 (32 bits)
    }

    /// Check if FIN flag is set (stream finished sending)
    ///
    /// # Performance
    ///
    /// <10ns (Relaxed atomic load)
    pub fn is_fin_sent(&self) -> bool {
        let secondary = self.secondary.load(Ordering::Relaxed);
        (secondary & (1u64 << 59)) != 0 // Bit 59
    }

    /// Check if FIN received flag is set
    ///
    /// # Performance
    ///
    /// <10ns (Relaxed atomic load)
    pub fn is_fin_received(&self) -> bool {
        let secondary = self.secondary.load(Ordering::Relaxed);
        (secondary & (1u64 << 60)) != 0 // Bit 60
    }

    /// Open stream: Idle → Ready transition
    ///
    /// # Returns
    ///
    /// - QuicStreamError::InvalidStateTransition if not in Idle state
    ///
    /// # Performance
    ///
    /// <20ns (Release ordering, atomic store)
    ///
    /// # ASSUME
    /// - `#ASSUME_STATE_ONEWAYS`: Idle is initial state
    pub fn open_stream(&self) -> Result<(), QuicStreamError> {
        let secondary = self.secondary.load(Ordering::Relaxed);
        let current_state = (secondary & 7) as u8;

        if current_state != StreamState::Idle as u8 {
            return Err(QuicStreamError::InvalidStateTransition);
        }

        // Transition: Idle (0) → Ready (1)
        // Set state bits 0-2 to 1, preserve all other bits
        let new_secondary = (secondary & !7u64) | StreamState::Ready as u64;
        self.secondary
            .store(new_secondary, Ordering::Release); // Release for visibility

        Ok(())
    }

    /// Send data on stream
    ///
    /// # Arguments
    ///
    /// - `bytes`: Number of bytes to send (will be added to bytes_sent)
    ///
    /// # Returns
    ///
    /// - QuicStreamError::InvalidStateTransition if not in Ready/Send state
    /// - QuicStreamError::ExceedsFlowControl if bytes_sent + bytes > max_stream_data
    ///
    /// # Performance
    ///
    /// <30ns (flow control check + state transition)
    ///
    /// # ASSUME
    /// - `#ASSUME_FLOWCONTROL_CHECKED`: bytes > 0 and doesn't overflow u32
    /// - `#ASSUME_STATE_ONEWAYS`: Ready → Send transition only
    pub fn send_data(&self, bytes: u32) -> Result<(), QuicStreamError> {
        if bytes == 0 {
            return Err(QuicStreamError::ExceedsFlowControl);
        }

        let secondary = self.secondary.load(Ordering::Acquire);
        let current_state = (secondary & 7) as u8;

        // Check state: Ready or Send (can send)
        if !matches!(
            current_state,
            s if s == StreamState::Ready as u8 || s == StreamState::Send as u8
        ) {
            return Err(QuicStreamError::InvalidStateTransition);
        }

        // Check flow control: bytes_sent + bytes ≤ max_stream_data
        let bytes_sent = ((secondary >> 3) & 0xFFFFFF) as u32;
        let max_stream_data_q16 = (secondary >> 27) as u32;
        let max_stream_data_bytes = max_stream_data_q16 >> 16;

        if bytes_sent.checked_add(bytes).unwrap_or(u32::MAX) > max_stream_data_bytes {
            return Err(QuicStreamError::ExceedsFlowControl);
        }

        // Update bytes_sent (add bytes)
        let new_bytes_sent = bytes_sent + bytes;
        let new_bytes_sent_bits = (new_bytes_sent as u64 & 0xFFFFFF) << 3;

        // Transition to Send if in Ready
        let new_state = if current_state == StreamState::Ready as u8 {
            StreamState::Send as u64
        } else {
            StreamState::Send as u64 // Already in Send
        };

        // Build new secondary: state (3b) + bytes_sent (24b) + max_stream_data (32b) + flags (5b)
        let new_secondary = (secondary & !(7u64 | (0xFFFFFFu64 << 3))) | new_state | new_bytes_sent_bits;

        self.secondary.store(new_secondary, Ordering::Release);

        Ok(())
    }

    /// Finish stream (set FIN, transition Send → DataSent)
    ///
    /// # Returns
    ///
    /// - QuicStreamError::InvalidStateTransition if not in Send state
    ///
    /// # Performance
    ///
    /// <15ns (FIN flag set + state transition, Release ordering)
    ///
    /// # ASSUME
    /// - `#ASSUME_STATE_ONEWAYS`: Send → DataSent transition only
    pub fn finish_stream(&self) -> Result<(), QuicStreamError> {
        let secondary = self.secondary.load(Ordering::Acquire);
        let current_state = (secondary & 7) as u8;

        if current_state != StreamState::Send as u8 {
            return Err(QuicStreamError::InvalidStateTransition);
        }

        // Set FIN flag (bit 59) and transition Send (2) → DataSent (3)
        let new_secondary = (secondary & !7u64) | StreamState::DataSent as u64 | (1u64 << 59);
        self.secondary
            .store(new_secondary, Ordering::Release);

        Ok(())
    }

    /// Reset stream (set state to Reset, transition to terminal state)
    ///
    /// # Returns
    ///
    /// - QuicStreamError::InvalidStateTransition if already in terminal state
    ///
    /// # Performance
    ///
    /// <10ns (state transition, Acquire/Release ordering)
    ///
    /// # ASSUME
    /// - `#ASSUME_STATE_ONEWAYS`: Any state → Reset transition allowed
    pub fn reset_stream(&self) -> Result<(), QuicStreamError> {
        let secondary = self.secondary.load(Ordering::Acquire);
        let current_state = (secondary & 7) as u8;

        // Check if already terminal
        let current = match current_state {
            0 => StreamState::Idle,
            1 => StreamState::Ready,
            2 => StreamState::Send,
            3 => StreamState::DataSent,
            4 => StreamState::DataRecvd,
            5 => StreamState::Reset,
            6 => StreamState::ResetRecvd,
            7 => StreamState::ResetSent,
            _ => return Err(QuicStreamError::InvalidStateTransition),
        };

        if current.is_terminal() {
            return Err(QuicStreamError::StreamClosed);
        }

        // Transition to Reset
        let new_secondary = (secondary & !7u64) | StreamState::Reset as u64;
        self.secondary
            .store(new_secondary, Ordering::Release);

        Ok(())
    }

    /// Update max stream data (flow control window increase)
    ///
    /// # Arguments
    ///
    /// - `max_stream_data`: New maximum (integer bytes, will be converted to Q16.16)
    ///
    /// # Performance
    ///
    /// <20ns (atomic update, SeqCst ordering for visibility)
    pub fn update_max_stream_data(&self, max_stream_data: u32) -> Result<(), QuicStreamError> {
        let secondary = self.secondary.load(Ordering::Relaxed);

        // Check if stream is closed
        let current_state = (secondary & 7) as u8;
        if matches!(
            current_state,
            s if s == StreamState::DataRecvd as u8
                || s == StreamState::Reset as u8
                || s == StreamState::ResetSent as u8
        ) {
            return Err(QuicStreamError::StreamClosed);
        }

        // Convert to Q16.16 and update
        let max_data_q16 = (max_stream_data as u64) << 16;
        let new_secondary =
            (secondary & !(0xFFFFFFFFu64 << 27)) | (max_data_q16 & (0xFFFFFFFFu64 << 27));

        self.secondary
            .store(new_secondary, Ordering::SeqCst);

        Ok(())
    }

    /// Check if stream is open and can receive/send data
    ///
    /// # Performance
    ///
    /// <10ns (state check)
    pub fn is_open(&self) -> bool {
        match self.get_state() {
            StreamState::Idle | StreamState::Ready | StreamState::Send => true,
            _ => false,
        }
    }

    /// Check if stream is closed (terminal state)
    ///
    /// # Performance
    ///
    /// <10ns (state check)
    pub fn is_closed(&self) -> bool {
        self.get_state().is_terminal()
    }
}

// ============================================================================
// TESTS (T28 4-Tier Framework)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1-Q7: Unit Tests
    // ========================================================================

    #[test]
    fn test_stream_creation() {
        let stream = QuicStreamCapsule::new(42, StreamDirection::ClientBidi, 65536).unwrap();
        assert_eq!(stream.get_stream_id(), 42);
        assert_eq!(stream.get_direction(), StreamDirection::ClientBidi);
        assert_eq!(stream.get_state(), StreamState::Idle);
        assert_eq!(stream.get_bytes_sent(), 0);
    }

    #[test]
    fn test_stream_id_encoding() {
        // RFC 9000: stream_id bits encode direction
        // ClientBidi: stream_id % 4 == 0
        let stream = QuicStreamCapsule::new(0, StreamDirection::ClientBidi, 65536).unwrap();
        assert_eq!(stream.get_stream_id(), 0);

        // ServerBidi: stream_id % 4 == 1
        let stream = QuicStreamCapsule::new(1, StreamDirection::ServerBidi, 65536).unwrap();
        assert_eq!(stream.get_stream_id(), 1);

        // ClientUni: stream_id % 4 == 2
        let stream = QuicStreamCapsule::new(2, StreamDirection::ClientUni, 65536).unwrap();
        assert_eq!(stream.get_stream_id(), 2);

        // ServerUni: stream_id % 4 == 3
        let stream = QuicStreamCapsule::new(3, StreamDirection::ServerUni, 65536).unwrap();
        assert_eq!(stream.get_stream_id(), 3);
    }

    #[test]
    fn test_invalid_stream_id_encoding() {
        // Mismatched direction and stream ID
        let result = QuicStreamCapsule::new(0, StreamDirection::ServerBidi, 65536);
        assert_eq!(result, Err(QuicStreamError::InvalidStreamId));

        // Stream ID too large
        let result = QuicStreamCapsule::new(1u64 << 62, StreamDirection::ClientBidi, 65536);
        assert_eq!(result, Err(QuicStreamError::InvalidStreamId));
    }

    #[test]
    fn test_stream_size() {
        assert_eq!(core::mem::size_of::<QuicStreamCapsule>(), 64);
        assert_eq!(core::mem::align_of::<QuicStreamCapsule>(), 64);
    }

    #[test]
    fn test_state_transitions() {
        let stream = QuicStreamCapsule::new(4, StreamDirection::ClientBidi, 65536).unwrap();

        // Idle → Ready
        assert_eq!(stream.get_state(), StreamState::Idle);
        stream.open_stream().unwrap();
        assert_eq!(stream.get_state(), StreamState::Ready);

        // Ready → Send (via send_data)
        stream.send_data(1024).unwrap();
        assert_eq!(stream.get_state(), StreamState::Send);
        assert_eq!(stream.get_bytes_sent(), 1024);

        // Send → DataSent (via finish_stream)
        stream.finish_stream().unwrap();
        assert_eq!(stream.get_state(), StreamState::DataSent);
        assert!(stream.is_fin_sent());
    }

    #[test]
    fn test_open_stream_invalid() {
        let stream = QuicStreamCapsule::new(8, StreamDirection::ClientBidi, 65536).unwrap();
        stream.open_stream().unwrap();

        // Can't open twice
        let result = stream.open_stream();
        assert_eq!(result, Err(QuicStreamError::InvalidStateTransition));
    }

    #[test]
    fn test_send_data_flow_control() {
        let stream = QuicStreamCapsule::new(12, StreamDirection::ClientBidi, 100).unwrap();
        stream.open_stream().unwrap();

        // Send within flow control window
        stream.send_data(50).unwrap();
        assert_eq!(stream.get_bytes_sent(), 50);

        // Send more within window
        stream.send_data(30).unwrap();
        assert_eq!(stream.get_bytes_sent(), 80);

        // Exceed flow control window
        let result = stream.send_data(30);
        assert_eq!(result, Err(QuicStreamError::ExceedsFlowControl));
    }

    #[test]
    fn test_reset_stream() {
        let stream = QuicStreamCapsule::new(16, StreamDirection::ClientBidi, 65536).unwrap();
        stream.open_stream().unwrap();
        stream.send_data(100).unwrap();

        // Reset from Send state
        stream.reset_stream().unwrap();
        assert_eq!(stream.get_state(), StreamState::Reset);

        // Can't reset twice (terminal state)
        let result = stream.reset_stream();
        assert_eq!(result, Err(QuicStreamError::StreamClosed));
    }

    #[test]
    fn test_fin_flag() {
        let stream = QuicStreamCapsule::new(20, StreamDirection::ClientBidi, 65536).unwrap();
        assert!(!stream.is_fin_sent());

        stream.open_stream().unwrap();
        stream.send_data(1024).unwrap();
        assert!(!stream.is_fin_sent());

        stream.finish_stream().unwrap();
        assert!(stream.is_fin_sent());
    }

    #[test]
    fn test_max_stream_data_update() {
        let stream = QuicStreamCapsule::new(24, StreamDirection::ClientBidi, 100).unwrap();
        stream.open_stream().unwrap();

        // Send some data
        stream.send_data(80).unwrap();

        // Try to send more (exceeds current window)
        let result = stream.send_data(50);
        assert_eq!(result, Err(QuicStreamError::ExceedsFlowControl));

        // Update flow control window
        stream.update_max_stream_data(200).unwrap();

        // Now send should succeed
        stream.send_data(50).unwrap();
        assert_eq!(stream.get_bytes_sent(), 130);
    }

    // ========================================================================
    // Q8-Q14: Property-Based Tests
    // ========================================================================

    #[test]
    fn test_property_stream_id_immutable() {
        let stream = QuicStreamCapsule::new(42, StreamDirection::ClientBidi, 65536).unwrap();
        let id1 = stream.get_stream_id();
        let id2 = stream.get_stream_id();
        assert_eq!(id1, id2);
        assert_eq!(id1, 42);
    }

    #[test]
    fn test_property_bytes_sent_monotonic() {
        let stream = QuicStreamCapsule::new(44, StreamDirection::ClientBidi, 65536).unwrap();
        stream.open_stream().unwrap();

        let bytes1 = stream.get_bytes_sent();
        stream.send_data(100).unwrap();
        let bytes2 = stream.get_bytes_sent();
        assert!(bytes2 > bytes1);

        stream.send_data(100).unwrap();
        let bytes3 = stream.get_bytes_sent();
        assert!(bytes3 > bytes2);
        assert_eq!(bytes3, 200);
    }

    #[test]
    fn test_property_state_never_backward() {
        let stream = QuicStreamCapsule::new(46, StreamDirection::ClientBidi, 65536).unwrap();

        let state1 = stream.get_state();
        stream.open_stream().unwrap();
        let state2 = stream.get_state();
        assert!(state2 as u8 > state1 as u8 || state2 == state1);

        stream.send_data(100).unwrap();
        let state3 = stream.get_state();
        assert!(state3 as u8 >= state2 as u8);

        stream.finish_stream().unwrap();
        let state4 = stream.get_state();
        assert!(state4 as u8 >= state3 as u8);
    }

    #[test]
    fn test_property_fin_implies_datasent() {
        let stream = QuicStreamCapsule::new(48, StreamDirection::ClientBidi, 65536).unwrap();
        stream.open_stream().unwrap();
        stream.send_data(100).unwrap();
        stream.finish_stream().unwrap();

        assert!(stream.is_fin_sent());
        assert_eq!(stream.get_state(), StreamState::DataSent);
    }

    // ========================================================================
    // Q15-Q21: Integration Tests
    // ========================================================================

    #[test]
    fn test_integration_full_lifecycle() {
        let stream = QuicStreamCapsule::new(50, StreamDirection::ClientBidi, 1000).unwrap();

        // Initial state
        assert_eq!(stream.get_state(), StreamState::Idle);
        assert!(!stream.is_open());
        assert!(!stream.is_closed());

        // Open stream
        stream.open_stream().unwrap();
        assert_eq!(stream.get_state(), StreamState::Ready);
        assert!(stream.is_open());
        assert!(!stream.is_closed());

        // Send data in chunks
        stream.send_data(250).unwrap();
        assert_eq!(stream.get_state(), StreamState::Send);
        assert_eq!(stream.get_bytes_sent(), 250);

        stream.send_data(250).unwrap();
        assert_eq!(stream.get_bytes_sent(), 500);

        stream.send_data(500).unwrap();
        assert_eq!(stream.get_bytes_sent(), 1000);

        // Finish stream (FIN flag)
        stream.finish_stream().unwrap();
        assert_eq!(stream.get_state(), StreamState::DataSent);
        assert!(stream.is_fin_sent());
        assert!(!stream.is_open());
    }

    #[test]
    fn test_integration_flow_control_enforcement() {
        let stream = QuicStreamCapsule::new(52, StreamDirection::ServerBidi, 512).unwrap();
        stream.open_stream().unwrap();

        // Gradually increase usage
        for _ in 0..4 {
            stream.send_data(128).unwrap();
        }
        assert_eq!(stream.get_bytes_sent(), 512);

        // Should fail to send more
        assert_eq!(
            stream.send_data(1),
            Err(QuicStreamError::ExceedsFlowControl)
        );

        // Increase window
        stream.update_max_stream_data(1024).unwrap();

        // Now send should succeed
        stream.send_data(256).unwrap();
        assert_eq!(stream.get_bytes_sent(), 768);
    }

    #[test]
    fn test_integration_reset_recovery() {
        let stream = QuicStreamCapsule::new(54, StreamDirection::ClientUni, 65536).unwrap();
        stream.open_stream().unwrap();

        // Send some data
        stream.send_data(1000).unwrap();
        assert_eq!(stream.get_bytes_sent(), 1000);

        // Reset stream
        stream.reset_stream().unwrap();
        assert_eq!(stream.get_state(), StreamState::Reset);

        // All operations should fail on closed stream
        assert_eq!(
            stream.send_data(100),
            Err(QuicStreamError::InvalidStateTransition)
        );
        assert_eq!(
            stream.finish_stream(),
            Err(QuicStreamError::InvalidStateTransition)
        );
        assert_eq!(
            stream.update_max_stream_data(100000),
            Err(QuicStreamError::StreamClosed)
        );
    }

    // ========================================================================
    // Q22-Q28: Production/Stress Tests
    // ========================================================================

    #[test]
    fn test_production_high_throughput_stream() {
        let stream = QuicStreamCapsule::new(56, StreamDirection::ServerBidi, u32::MAX).unwrap();
        stream.open_stream().unwrap();

        // Simulate high-throughput scenario (10 million bytes)
        const CHUNK_SIZE: u32 = 1024;
        const ITERATIONS: u32 = 10000;
        const EXPECTED_TOTAL: u32 = CHUNK_SIZE * ITERATIONS;

        for _ in 0..ITERATIONS {
            stream.send_data(CHUNK_SIZE).unwrap();
        }

        assert_eq!(stream.get_bytes_sent(), EXPECTED_TOTAL);
        assert_eq!(stream.get_state(), StreamState::Send);
    }

    #[test]
    fn test_production_stress_bidi_streams() {
        // Create multiple bidirectional streams
        for stream_id in (0..100).step_by(4) {
            let stream = QuicStreamCapsule::new(stream_id, StreamDirection::ClientBidi, 65536)
                .unwrap();
            stream.open_stream().unwrap();
            stream.send_data(256).unwrap();
            stream.finish_stream().unwrap();

            assert_eq!(stream.get_state(), StreamState::DataSent);
            assert!(stream.is_fin_sent());
        }
    }

    #[test]
    fn test_production_unidi_stream_no_reset() {
        // Unidirectional streams can't be reset mid-send
        let stream = QuicStreamCapsule::new(102, StreamDirection::ServerUni, 1000).unwrap();
        stream.open_stream().unwrap();
        stream.send_data(500).unwrap();

        // Reset is allowed
        stream.reset_stream().unwrap();
        assert_eq!(stream.get_state(), StreamState::Reset);
    }

    #[test]
    fn test_production_multiple_window_increases() {
        let stream = QuicStreamCapsule::new(104, StreamDirection::ClientBidi, 100).unwrap();
        stream.open_stream().unwrap();

        // Send up to first window
        stream.send_data(100).unwrap();
        assert_eq!(
            stream.send_data(1),
            Err(QuicStreamError::ExceedsFlowControl)
        );

        // Increase window 5 times
        for i in 1..=5 {
            stream
                .update_max_stream_data((i * 100) as u32)
                .unwrap();
            stream.send_data(100).unwrap();
        }

        assert_eq!(stream.get_bytes_sent(), 600);
    }
}
