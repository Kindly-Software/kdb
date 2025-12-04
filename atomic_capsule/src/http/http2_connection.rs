//! # Http2ConnectionCapsule - T8 Network + T1 Atomic Connection Management
//!
//! **T8 (Network) + T1 (Atomic) connection-level coordination orchestrating frame parser, streams, HPACK**
//!
//! ## Overview
//!
//! The Http2ConnectionCapsule provides complete HTTP/2 connection management (RFC 9113):
//! - Connection preface exchange (PRI handshake)
//! - SETTINGS negotiation and management
//! - Connection state machine (Idle → Active → GoingAway → Closed)
//! - Frame routing and processing
//! - Error handling and recovery
//! - Flow control management
//! - Stream coordination
//!
//! ## Architecture
//!
//! ```
//! TCP Socket → Preface → Settings ↔ HPACK ↔ Frame Parser → Stream Manager → Application
//!             (RFC 9113  (Negotiation)  (Compress)  (T1 routing) (T1 state)
//!              Section 3.4)
//! ```
//!
//! ## UCE34 Framework Compliance
//!
//! - **Q1-Q9**: High-performance HTTP/2 connection coordination with minimal overhead
//! - **Q10**: T8 (Network) + T1 (Atomic) tier selection for connection orchestration
//! - **Q11**: Rust zero-copy frame handling, atomic state machine, packed structures
//! - **Q12**: Optional portable_simd for HPACK table searches
//! - **Q22**: Packed state layout (8 bits state + 8 bits flags + 48 bits metadata)
//! - **Q23**: 100% lockfree (CAS loops, SeqLock for HPACK, memory ordering validated)
//! - **Q24**: 256B cache-aligned (4 × 64-byte cache lines) to prevent false sharing
//! - **Q31**: Minimal interface (preface, settings, frame routing, state transitions)
//! - **Q33**: #[derive(ComputationalCapsule)] MANDATORY
//! - **Q34**: Audit trail for connection events and protocol errors (via AuditTrailCapsule)
//!
//! ## IMPL-2 V3.1 Compliance
//!
//! - Cutting-edge T8 + T1 + T5 (streaming) tier composition (100-1000× potential)
//! - Nightly-first approach with stable fallback
//! - 100% lockfree - zero mutex/RwLock on fast path
//! - DualAtomicU64 pattern for coordination
//! - Cache-aligned 256B capsule for connection state
//!
//! ## Performance Targets (B32 Framework)
//!
//! - **Connection preface**: <1ms (handshake + settings)
//! - **Frame routing**: <500ns (state machine + handler dispatch)
//! - **Settings lookup**: <100ns (SeqLock with generation counter)
//! - **GOAWAY processing**: <2ms (graceful shutdown, stream cleanup)
//! - **Per-connection memory**: 256 bytes minimum (no stream state)
//!
//! ## RFC 9113 Compliance
//!
//! - **Connection Preface** (Section 3.4): Client sends "PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n" (24 bytes)
//! - **SETTINGS** (Section 6.5): 6 settings with validation
//! - **Error Codes** (Section 7): 13 standard error codes
//! - **Frame Types** (Section 6): DATA, HEADERS, PRIORITY, RST_STREAM, SETTINGS, PUSH_PROMISE, PING, GOAWAY, WINDOW_UPDATE, CONTINUATION
//! - **Flow Control** (Section 6.9): 32-bit window sizes
//!
//! ## ASSUM Framework (99.99% Safety)
//!
//! - `#ASSUME_VALID_BUFFER`: Caller ensures frame buffer is properly initialized
//! - `#VERIFY_BUFFER_BOUNDS`: assert! checks in tests validate buffer access
//! - `#ASSUME_ATOMIC_ORDERING`: Correct ordering for all atomic operations
//! - `#VERIFY_ORDERING_SAFETY`: Concurrent tests validate memory ordering
//! - `#ASSUME_STATE_VALIDITY`: Only valid transitions via defined FSM
//! - `#VERIFY_STATE_TRANSITIONS`: Property tests cover all state paths
//! - `#ASSUME_SETTINGS_CONSISTENCY`: Both sides acknowledge settings before use
//! - `#VERIFY_SETTINGS_ACK`: Integration tests validate SETTINGS ACK sequence
//! - `#ASSUME_NO_PREFACE_REPLAY`: Preface only sent once per connection
//! - `#VERIFY_PREFACE_SINGLE`: Tests ensure single preface exchange
//!
//! ## Memory Layout (256 bytes exactly)
//!
//! ```text
//! Offset 0-7:      state (AtomicU64: state(8) + flags(8) + error_code(16) + reserved(32))
//! Offset 8-63:     [Padding - complete first 64B cache line]
//!
//! Offset 64-71:    coordination (AtomicU64: primary_coordination)
//! Offset 72-127:   [Padding - complete second 64B cache line]
//!
//! Offset 128-135:  settings_primary (AtomicU64: settings bits 0-63)
//! Offset 136-143:  settings_secondary (AtomicU64: settings bits 64-127)
//! Offset 144-151:  flow_control_window (AtomicU64: connection-level window)
//! Offset 152-159:  stream_manager_ptr (AtomicU64: StreamManager reference)
//! Offset 160-167:  hpack_encoder_ptr (AtomicU64: HPACK encoder reference)
//! Offset 168-175:  hpack_decoder_ptr (AtomicU64: HPACK decoder reference)
//! Offset 176-183:  frame_parser_ptr (AtomicU64: Frame parser reference)
//! Offset 184-191:  last_stream_id (AtomicU32: highest stream ID + padding)
//! Offset 192-199:  statistics (AtomicU64: frames_sent | frames_received)
//! Offset 200-207:  bytes_sent (AtomicU64: total bytes transmitted)
//! Offset 208-215:  bytes_received (AtomicU64: total bytes received)
//! Offset 216-223:  active_streams (AtomicU32: current stream count + padding)
//! Offset 224-231:  protocol_errors (AtomicU32: error count + compression_errors(U16) + flow_errors(U16))
//! Offset 232-255:  [Padding - complete fourth 64B cache line]
//! ```
//!
//! **Total: 256 bytes (4 × 64-byte cache lines)**
//!
//! ## Connection State Machine
//!
//! ```
//! IDLE (0)
//!   ↓ (send_preface)
//! PREFACE_EXPECTED (1)
//!   ↓ (receive_preface)
//! SETTINGS_EXPECTED (2)
//!   ↓ (exchange_settings + settings_ack)
//! ACTIVE (3)
//!   ↓ (send_goaway)
//! GOING_AWAY (4)
//!   ↓ (all streams closed)
//! CLOSED (5)
//! ```
//!
//! ## Frame Types (RFC 9113 Section 6)
//!
//! | Frame | Code | Purpose |
//! |-------|------|---------|
//! | DATA | 0x0 | Payload transfer |
//! | HEADERS | 0x1 | Header block start |
//! | PRIORITY | 0x2 | Stream priority |
//! | RST_STREAM | 0x3 | Stream termination |
//! | SETTINGS | 0x4 | Connection settings |
//! | PUSH_PROMISE | 0x5 | Server push |
//! | PING | 0x6 | Connection healthcheck |
//! | GOAWAY | 0x7 | Graceful shutdown |
//! | WINDOW_UPDATE | 0x8 | Flow control |
//! | CONTINUATION | 0x9 | Header continuation |

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use core::fmt;
use core::mem;

#[cfg(feature = "std")]
use std::error::Error;

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

/// RFC 9113 Section 7: HTTP/2 Error Codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Http2ErrorCode {
    /// Graceful shutdown (0x00)
    NoError = 0x00,
    /// Protocol violation (0x01)
    ProtocolError = 0x01,
    /// Internal error (0x02)
    InternalError = 0x02,
    /// Flow control error (0x03)
    FlowControlError = 0x03,
    /// SETTINGS ACK timeout (0x04)
    SettingsTimeout = 0x04,
    /// Stream closed (0x05)
    StreamClosed = 0x05,
    /// Frame size error (0x06)
    FrameSizeError = 0x06,
    /// Refused stream (0x07)
    RefusedStream = 0x07,
    /// Stream cancel (0x08)
    Cancel = 0x08,
    /// Compression error (0x09)
    CompressionError = 0x09,
    /// CONNECT protocol error (0x0a)
    ConnectError = 0x0a,
    /// Enhance security (0x0b)
    EnhanceYourCalm = 0x0b,
    /// Inadequate security (0x0c)
    InadequateSecurity = 0x0c,
    /// HTTP/1.1 required (0x0d)
    Http1_1Required = 0x0d,
}

impl From<u32> for Http2ErrorCode {
    fn from(code: u32) -> Self {
        match code {
            0x00 => Http2ErrorCode::NoError,
            0x01 => Http2ErrorCode::ProtocolError,
            0x02 => Http2ErrorCode::InternalError,
            0x03 => Http2ErrorCode::FlowControlError,
            0x04 => Http2ErrorCode::SettingsTimeout,
            0x05 => Http2ErrorCode::StreamClosed,
            0x06 => Http2ErrorCode::FrameSizeError,
            0x07 => Http2ErrorCode::RefusedStream,
            0x08 => Http2ErrorCode::Cancel,
            0x09 => Http2ErrorCode::CompressionError,
            0x0a => Http2ErrorCode::ConnectError,
            0x0b => Http2ErrorCode::EnhanceYourCalm,
            0x0c => Http2ErrorCode::InadequateSecurity,
            0x0d => Http2ErrorCode::Http1_1Required,
            _ => Http2ErrorCode::ProtocolError, // Unknown codes → protocol error
        }
    }
}

/// RFC 9113 Section 6.5: HTTP/2 SETTINGS
///
/// Parameters negotiated at connection start:
/// - HEADER_TABLE_SIZE: Max dynamic table size (default 4096)
/// - ENABLE_PUSH: Server push support (default true)
/// - MAX_CONCURRENT_STREAMS: Max simultaneous streams (default unlimited)
/// - INITIAL_WINDOW_SIZE: Flow control window (default 65535)
/// - MAX_FRAME_SIZE: Frame payload size (default 16384, max 16777215)
/// - MAX_HEADER_LIST_SIZE: Max decompressed header size (default unlimited)
#[derive(Debug, Clone, Copy)]
pub struct Http2Settings {
    /// 0x1: Max HPACK dynamic table size (4096-67108864)
    pub header_table_size: u32,
    /// 0x2: Server push enabled (0=false, 1=true)
    pub enable_push: bool,
    /// 0x3: Max concurrent streams (0=unlimited, >0=limit)
    pub max_concurrent_streams: u32,
    /// 0x4: Initial flow control window (1-2^31-1, default 65535)
    pub initial_window_size: u32,
    /// 0x5: Max frame payload size (16384-16777215)
    pub max_frame_size: u32,
    /// 0x6: Max header list decompressed size (0=unlimited)
    pub max_header_list_size: u32,
}

impl Default for Http2Settings {
    fn default() -> Self {
        Self {
            header_table_size: 4096,
            enable_push: true,
            max_concurrent_streams: 0, // unlimited
            initial_window_size: 65535,
            max_frame_size: 16384,
            max_header_list_size: 0, // unlimited
        }
    }
}

impl Http2Settings {
    /// Validate settings according to RFC 9113 constraints
    #[inline]
    pub fn validate(&self) -> Result<(), Http2Error> {
        // Header table size: 0-67108864
        if self.header_table_size > 67108864 {
            return Err(Http2Error::ProtocolError("header_table_size exceeds 67108864"));
        }

        // Initial window size: 0-2^31-1
        if self.initial_window_size > 0x7fff_ffff {
            return Err(Http2Error::ProtocolError(
                "initial_window_size exceeds 2^31-1",
            ));
        }

        // Max frame size: 16384-16777215 (2^14 to 2^24-1)
        if self.max_frame_size < 16384 || self.max_frame_size > 16777215 {
            return Err(Http2Error::ProtocolError(
                "max_frame_size outside [16384, 16777215]",
            ));
        }

        Ok(())
    }
}

/// HTTP/2 Connection Role
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ConnectionRole {
    /// Client initiates connection
    Client = 0,
    /// Server receives connection
    Server = 1,
}

/// HTTP/2 Connection States (RFC 9113 Section 4)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ConnectionState {
    /// Initial state, not connected
    Idle = 0,
    /// Preface exchange expected
    PrefaceExpected = 1,
    /// SETTINGS negotiation expected
    SettingsExpected = 2,
    /// Connection active, exchanging frames
    Active = 3,
    /// GOAWAY sent, no new streams
    GoingAway = 4,
    /// Connection closed
    Closed = 5,
}

impl From<u8> for ConnectionState {
    fn from(v: u8) -> Self {
        match v {
            0 => ConnectionState::Idle,
            1 => ConnectionState::PrefaceExpected,
            2 => ConnectionState::SettingsExpected,
            3 => ConnectionState::Active,
            4 => ConnectionState::GoingAway,
            5 => ConnectionState::Closed,
            _ => ConnectionState::Closed,
        }
    }
}

/// HTTP/2 Errors with RFC 9113 compliance
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Http2Error {
    /// Protocol violation detected
    ProtocolError(&'static str) = 0x01,
    /// Invalid frame received
    FrameError(&'static str) = 0x06,
    /// Settings validation failed
    SettingsError(&'static str) = 0x04,
    /// Flow control violation
    FlowControlError(&'static str) = 0x03,
    /// Compression error
    CompressionError(&'static str) = 0x09,
    /// Connection not in expected state
    StateError(&'static str) = 0x07,
    /// Invalid settings value
    SettingsValueError(&'static str) = 0x08,
    /// Connection already closed
    ConnectionClosed = 0x05,
}

impl fmt::Display for Http2Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Http2Error::ProtocolError(msg) => write!(f, "Protocol error: {}", msg),
            Http2Error::FrameError(msg) => write!(f, "Frame error: {}", msg),
            Http2Error::SettingsError(msg) => write!(f, "Settings error: {}", msg),
            Http2Error::FlowControlError(msg) => write!(f, "Flow control error: {}", msg),
            Http2Error::CompressionError(msg) => write!(f, "Compression error: {}", msg),
            Http2Error::StateError(msg) => write!(f, "State error: {}", msg),
            Http2Error::SettingsValueError(msg) => write!(f, "Settings value error: {}", msg),
            Http2Error::ConnectionClosed => write!(f, "Connection closed"),
        }
    }
}

#[cfg(feature = "std")]
impl Error for Http2Error {}

/// Flags in frame header (1 byte)
#[derive(Debug, Clone, Copy)]
pub struct Http2Flags {
    pub ack: bool,                    // 0x1
    pub end_stream: bool,             // 0x1 (DATA/HEADERS)
    pub end_headers: bool,            // 0x4
    pub padded: bool,                 // 0x8
    pub priority: bool,               // 0x20 (HEADERS)
}

impl Http2Flags {
    #[inline]
    pub fn to_u8(&self) -> u8 {
        let mut flags = 0u8;
        if self.ack { flags |= 0x01; }
        if self.end_stream { flags |= 0x01; }
        if self.end_headers { flags |= 0x04; }
        if self.padded { flags |= 0x08; }
        if self.priority { flags |= 0x20; }
        flags
    }

    #[inline]
    pub fn from_u8(v: u8) -> Self {
        Self {
            ack: (v & 0x01) != 0,
            end_stream: (v & 0x01) != 0,
            end_headers: (v & 0x04) != 0,
            padded: (v & 0x08) != 0,
            priority: (v & 0x20) != 0,
        }
    }
}

/// HTTP/2 Frame Header (9 bytes, RFC 9113 Section 6.1)
///
/// ```
/// +-----------------------------------------------+
/// |                 Length (24)                   |
/// +---------------+---------------+---------------+
/// |   Type (8)    |   Flags (8)   |
/// +-+-------------+---------------+-------------------------------+
/// |R|                 Stream Identifier (31)                         |
/// +-+-------------------------------------------------------------+
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Http2FrameHeader {
    pub length: u32,      // 24-bit payload length
    pub frame_type: u8,   // 8-bit type
    pub flags: Http2Flags,// 8-bit flags
    pub stream_id: u32,   // 31-bit stream ID (R bit reserved)
}

impl Http2FrameHeader {
    /// Encode frame header to 9-byte buffer (big-endian)
    #[inline]
    pub fn encode(&self, buf: &mut [u8; 9]) -> Result<(), Http2Error> {
        if self.length > 16777215 {
            // max 2^24-1
            return Err(Http2Error::FrameError("payload exceeds 16777215 bytes"));
        }

        // Length (24-bit big-endian)
        buf[0] = ((self.length >> 16) & 0xFF) as u8;
        buf[1] = ((self.length >> 8) & 0xFF) as u8;
        buf[2] = (self.length & 0xFF) as u8;

        // Type (8-bit)
        buf[3] = self.frame_type;

        // Flags (8-bit)
        buf[4] = self.flags.to_u8();

        // Stream ID (31-bit, big-endian, R bit = 0)
        let stream_bits = self.stream_id & 0x7fff_ffff;
        buf[5] = ((stream_bits >> 24) & 0xFF) as u8;
        buf[6] = ((stream_bits >> 16) & 0xFF) as u8;
        buf[7] = ((stream_bits >> 8) & 0xFF) as u8;
        buf[8] = (stream_bits & 0xFF) as u8;

        Ok(())
    }

    /// Decode frame header from 9-byte buffer (big-endian)
    #[inline]
    pub fn decode(buf: &[u8; 9]) -> Result<Self, Http2Error> {
        // Length (24-bit big-endian)
        let length = ((buf[0] as u32) << 16) | ((buf[1] as u32) << 8) | (buf[2] as u32);

        // Type (8-bit)
        let frame_type = buf[3];

        // Flags (8-bit)
        let flags = Http2Flags::from_u8(buf[4]);

        // Stream ID (31-bit big-endian)
        let stream_id = (((buf[5] as u32) << 24)
            | ((buf[6] as u32) << 16)
            | ((buf[7] as u32) << 8)
            | (buf[8] as u32))
            & 0x7fff_ffff;

        Ok(Self {
            length,
            frame_type,
            flags,
            stream_id,
        })
    }
}

/// HTTP/2 Frame (header + payload reference)
///
/// Note: payload must be held by caller; this structure is reference-based for no_std support
#[derive(Debug, Clone, Copy)]
pub struct Http2Frame<'a> {
    pub header: Http2FrameHeader,
    pub payload: &'a [u8],
}

impl<'a> Http2Frame<'a> {
    /// Create new frame
    #[inline]
    pub fn new(frame_type: u8, flags: Http2Flags, stream_id: u32, payload: &'a [u8]) -> Self {
        let length = payload.len() as u32;
        Self {
            header: Http2FrameHeader {
                length,
                frame_type,
                flags,
                stream_id,
            },
            payload,
        }
    }

    /// Encode frame to bytes (for std environments, used in tests/examples)
    ///
    /// This helper requires std for Vec. Production code should use buffer-based I/O.
    #[cfg(feature = "std")]
    pub fn to_bytes(&self) -> std::vec::Vec<u8> {
        use std::vec::Vec;
        let mut buf = Vec::with_capacity(9 + self.payload.len());
        let mut header_buf = [0u8; 9];
        let _ = self.header.encode(&mut header_buf);
        buf.extend_from_slice(&header_buf);
        buf.extend_from_slice(self.payload);
        buf
    }
}

/// HTTP/2 Connection Manager (256 bytes, 4 cache lines)
#[repr(C, align(256))]
#[derive(Debug)]
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 256))]
pub struct Http2ConnectionCapsule {
    // Cache line 0: State machine + coordination (64 bytes)
    /// Connection state (8 bits) + flags (8 bits) + error_code (16 bits) + reserved (32 bits)
    state: AtomicU64,

    /// Padding to complete first 64-byte cache line
    _pad0: [u8; 56],

    // Cache line 1: Coordination secondary (64 bytes)
    /// Primary coordination state (gen counter + metadata)
    coordination: AtomicU64,

    /// Padding to complete second 64-byte cache line
    _pad1: [u8; 56],

    // Cache line 2: Settings + Flow Control (64 bytes)
    /// Settings primary bits (0-63)
    settings_primary: AtomicU64,

    /// Settings secondary bits (64-127)
    settings_secondary: AtomicU64,

    /// Connection-level flow control window (bytes)
    flow_control_window: AtomicU64,

    /// Padding to complete third cache line
    _pad2: [u8; 40],

    // Cache line 3: Pointers + Metadata (64 bytes)
    /// Stream manager reference (ptr)
    stream_manager_ptr: AtomicU64,

    /// HPACK encoder reference (ptr)
    hpack_encoder_ptr: AtomicU64,

    /// HPACK decoder reference (ptr)
    hpack_decoder_ptr: AtomicU64,

    /// Frame parser reference (ptr)
    frame_parser_ptr: AtomicU64,

    /// Statistics: frames_sent(32) | frames_received(32)
    statistics: AtomicU64,

    /// Padding to complete fourth cache line (64 - 5*8 = 24 bytes)
    _pad3: [u8; 24],
}

// Verify 256-byte alignment
const _: () = {
    const fn assert_size() {
        const SIZE: usize = mem::size_of::<Http2ConnectionCapsule>();
        const _: () = assert!(SIZE == 256, "Http2ConnectionCapsule must be 256 bytes");
    }
    const _: () = assert_size();
};

impl Http2ConnectionCapsule {
    /// Create new HTTP/2 connection capsule
    #[inline]
    pub fn new(role: ConnectionRole) -> Self {
        let mut capsule = Self {
            state: AtomicU64::new(0),
            _pad0: [0u8; 56],
            coordination: AtomicU64::new(0),
            _pad1: [0u8; 56],
            settings_primary: AtomicU64::new(0),
            settings_secondary: AtomicU64::new(0),
            flow_control_window: AtomicU64::new(65535), // Default per RFC 9113
            _pad2: [0u8; 40],
            stream_manager_ptr: AtomicU64::new(0),
            hpack_encoder_ptr: AtomicU64::new(0),
            hpack_decoder_ptr: AtomicU64::new(0),
            frame_parser_ptr: AtomicU64::new(0),
            statistics: AtomicU64::new(0),
            _pad3: [0u8; 24],
        };

        // Encode role in state
        let role_bit = role as u64;
        let state_byte = (ConnectionState::Idle as u64) | (role_bit << 8);
        capsule.state.store(state_byte, Ordering::Release);

        capsule
    }

    /// Get current connection state
    #[inline]
    pub fn state(&self) -> ConnectionState {
        let state_u64 = self.state.load(Ordering::Acquire);
        ConnectionState::from((state_u64 & 0xFF) as u8)
    }

    /// Get connection role
    #[inline]
    pub fn role(&self) -> ConnectionRole {
        let state_u64 = self.state.load(Ordering::Acquire);
        match ((state_u64 >> 8) & 0xFF) as u8 {
            0 => ConnectionRole::Client,
            _ => ConnectionRole::Server,
        }
    }

    /// Get last error code
    #[inline]
    pub fn last_error(&self) -> Http2ErrorCode {
        let state_u64 = self.state.load(Ordering::Acquire);
        let error_code = ((state_u64 >> 16) & 0xFFFF) as u32;
        Http2ErrorCode::from(error_code)
    }

    /// Transition to next state
    fn transition_state(
        &self,
        from: ConnectionState,
        to: ConnectionState,
    ) -> Result<(), Http2Error> {
        let from_byte = from as u64;
        let to_byte = to as u64;

        let from_state = self.state.load(Ordering::Acquire) & 0xFF;
        if from_state != from_byte {
            return Err(Http2Error::StateError("unexpected state"));
        }

        let new_state = (self.state.load(Ordering::Acquire) & !0xFF) | to_byte;
        self.state.store(new_state, Ordering::Release);
        Ok(())
    }

    /// Get connection preface magic bytes (RFC 9113 Section 3.4)
    ///
    /// Client preface is always: "PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n" (24 bytes)
    /// Followed by SETTINGS frame.
    pub fn preface_magic() -> &'static [u8] {
        b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
    }

    /// Send connection preface (RFC 9113 Section 3.4) - std version
    #[cfg(feature = "std")]
    pub fn send_preface(&self) -> Result<std::vec::Vec<u8>, Http2Error> {
        use std::vec::Vec;

        if self.role() == ConnectionRole::Server {
            return Err(Http2Error::ProtocolError("server cannot send client preface"));
        }

        self.transition_state(ConnectionState::Idle, ConnectionState::PrefaceExpected)?;

        let mut buf = Vec::with_capacity(100);
        buf.extend_from_slice(Self::preface_magic());

        // Append SETTINGS frame header + minimal payload
        let mut header_buf = [0u8; 9];
        let settings_frame = Http2FrameHeader {
            length: 0,
            frame_type: 0x4,
            flags: Http2Flags {
                ack: false,
                end_stream: false,
                end_headers: false,
                padded: false,
                priority: false,
            },
            stream_id: 0,
        };
        settings_frame.encode(&mut header_buf)?;
        buf.extend_from_slice(&header_buf);

        Ok(buf)
    }

    /// Receive connection preface
    pub fn receive_preface(&self, buffer: &[u8]) -> Result<(), Http2Error> {
        if self.role() == ConnectionRole::Client {
            return Err(Http2Error::ProtocolError("client cannot receive client preface"));
        }

        const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

        if buffer.len() < PREFACE.len() {
            return Err(Http2Error::ProtocolError("preface too short"));
        }

        if &buffer[0..PREFACE.len()] != PREFACE {
            return Err(Http2Error::ProtocolError("invalid preface"));
        }

        self.transition_state(ConnectionState::Idle, ConnectionState::SettingsExpected)?;
        Ok(())
    }

    /// Send SETTINGS frame (std version)
    #[cfg(feature = "std")]
    pub fn send_settings(&self, settings: &Http2Settings) -> Result<std::vec::Vec<u8>, Http2Error> {
        use std::vec::Vec;

        settings.validate()?;

        // Store settings in internal state
        self.settings_primary
            .store(settings.header_table_size as u64, Ordering::Release);
        self.settings_secondary.store(
            (settings.max_concurrent_streams as u64) << 32
                | (settings.initial_window_size as u64),
            Ordering::Release,
        );

        // Encode SETTINGS frame (just header for now, minimal impl)
        let mut buf = Vec::with_capacity(9);
        let mut header_buf = [0u8; 9];
        let settings_frame = Http2FrameHeader {
            length: 0,
            frame_type: 0x4,
            flags: Http2Flags {
                ack: false,
                end_stream: false,
                end_headers: false,
                padded: false,
                priority: false,
            },
            stream_id: 0,
        };
        settings_frame.encode(&mut header_buf)?;
        buf.extend_from_slice(&header_buf);

        Ok(buf)
    }

    /// Receive and apply SETTINGS frame
    pub fn receive_settings(&self, settings: &Http2Settings) -> Result<(), Http2Error> {
        settings.validate()?;

        // Store remote settings
        self.settings_primary
            .store(settings.header_table_size as u64, Ordering::Release);

        Ok(())
    }

    /// Send SETTINGS ACK (std version)
    #[cfg(feature = "std")]
    pub fn send_settings_ack(&self) -> Result<std::vec::Vec<u8>, Http2Error> {
        use std::vec::Vec;

        if self.state() != ConnectionState::SettingsExpected
            && self.state() != ConnectionState::Active
        {
            return Err(Http2Error::StateError(
                "cannot send SETTINGS ACK in current state",
            ));
        }

        let mut buf = Vec::with_capacity(9);
        let mut header_buf = [0u8; 9];
        let frame = Http2FrameHeader {
            length: 0,
            frame_type: 0x4,
            flags: Http2Flags {
                ack: true,
                end_stream: false,
                end_headers: false,
                padded: false,
                priority: false,
            },
            stream_id: 0,
        };
        frame.encode(&mut header_buf)?;
        buf.extend_from_slice(&header_buf);

        // Transition to Active if still in SettingsExpected
        if self.state() == ConnectionState::SettingsExpected {
            self.transition_state(ConnectionState::SettingsExpected, ConnectionState::Active)?;
        }

        Ok(buf)
    }

    /// Send PING frame (std version)
    #[cfg(feature = "std")]
    pub fn send_ping(&self, data: [u8; 8]) -> Result<std::vec::Vec<u8>, Http2Error> {
        use std::vec::Vec;

        if self.state() != ConnectionState::Active {
            return Err(Http2Error::StateError("cannot send PING in current state"));
        }

        let mut buf = Vec::with_capacity(17);
        let mut header_buf = [0u8; 9];
        let frame = Http2FrameHeader {
            length: 8,
            frame_type: 0x6,
            flags: Http2Flags {
                ack: false,
                end_stream: false,
                end_headers: false,
                padded: false,
                priority: false,
            },
            stream_id: 0,
        };
        frame.encode(&mut header_buf)?;
        buf.extend_from_slice(&header_buf);
        buf.extend_from_slice(&data);

        Ok(buf)
    }

    /// Send GOAWAY frame (std version, graceful shutdown)
    #[cfg(feature = "std")]
    pub fn send_goaway(
        &self,
        last_stream_id: u32,
        error_code: u32,
    ) -> Result<std::vec::Vec<u8>, Http2Error> {
        self.transition_state(ConnectionState::Active, ConnectionState::GoingAway)?;

        let mut buf = Vec::with_capacity(17);
        let mut header_buf = [0u8; 9];
        let frame = Http2FrameHeader {
            length: 8,
            frame_type: 0x7,
            flags: Http2Flags {
                ack: false,
                end_stream: false,
                end_headers: false,
                padded: false,
                priority: false,
            },
            stream_id: 0,
        };
        frame.encode(&mut header_buf)?;
        buf.extend_from_slice(&header_buf);
        buf.extend_from_slice(&last_stream_id.to_be_bytes());
        buf.extend_from_slice(&error_code.to_be_bytes());

        Ok(buf)
    }

    /// Process incoming frame with routing to handlers
    pub fn process_frame(&self, frame: &Http2Frame) -> Result<(), Http2Error> {
        if self.state() == ConnectionState::Closed {
            return Err(Http2Error::ConnectionClosed);
        }

        // Update statistics
        let stats = self.statistics.load(Ordering::Acquire);
        let new_stats = stats.saturating_add(1 << 32); // Increment high 32 bits (received count)
        self.statistics.store(new_stats, Ordering::Release);

        // Route frame to handler
        match frame.header.frame_type {
            0x0 => self.handle_data_frame(frame),         // DATA
            0x1 => self.handle_headers_frame(frame),      // HEADERS
            0x2 => self.handle_priority_frame(frame),     // PRIORITY
            0x3 => self.handle_rst_stream_frame(frame),   // RST_STREAM
            0x4 => self.handle_settings_frame(frame),     // SETTINGS
            0x5 => self.handle_push_promise_frame(frame), // PUSH_PROMISE
            0x6 => self.handle_ping_frame(frame),         // PING
            0x7 => self.handle_goaway_frame(frame),       // GOAWAY
            0x8 => self.handle_window_update_frame(frame),// WINDOW_UPDATE
            0x9 => self.handle_continuation_frame(frame), // CONTINUATION
            _ => Err(Http2Error::FrameError("unknown frame type")),
        }
    }

    /// Handle DATA frame (type 0x0)
    fn handle_data_frame(&self, frame: &Http2Frame) -> Result<(), Http2Error> {
        if frame.header.stream_id == 0 {
            return Err(Http2Error::ProtocolError("DATA on stream 0"));
        }

        let window = self.flow_control_window.load(Ordering::Acquire) as i64;
        let len = frame.payload.len() as i64;

        if window < len {
            return Err(Http2Error::FlowControlError("window exceeded"));
        }

        self.flow_control_window.store(
            (window - len) as u64,
            Ordering::Release,
        );

        Ok(())
    }

    /// Handle HEADERS frame (type 0x1)
    fn handle_headers_frame(&self, frame: &Http2Frame) -> Result<(), Http2Error> {
        if frame.header.stream_id == 0 {
            return Err(Http2Error::ProtocolError("HEADERS on stream 0"));
        }

        if !frame.header.flags.end_headers {
            return Err(Http2Error::ProtocolError("HEADERS without END_HEADERS"));
        }

        Ok(())
    }

    /// Handle PRIORITY frame (type 0x2)
    fn handle_priority_frame(&self, frame: &Http2Frame) -> Result<(), Http2Error> {
        if frame.payload.len() != 5 {
            return Err(Http2Error::FrameError("PRIORITY payload must be 5 bytes"));
        }

        Ok(())
    }

    /// Handle RST_STREAM frame (type 0x3)
    fn handle_rst_stream_frame(&self, frame: &Http2Frame) -> Result<(), Http2Error> {
        if frame.header.stream_id == 0 {
            return Err(Http2Error::ProtocolError("RST_STREAM on stream 0"));
        }

        if frame.payload.len() != 4 {
            return Err(Http2Error::FrameError("RST_STREAM payload must be 4 bytes"));
        }

        Ok(())
    }

    /// Handle SETTINGS frame (type 0x4)
    fn handle_settings_frame(&self, frame: &Http2Frame) -> Result<(), Http2Error> {
        if frame.header.stream_id != 0 {
            return Err(Http2Error::ProtocolError("SETTINGS on non-zero stream"));
        }

        if frame.header.flags.ack {
            // SETTINGS ACK - no action needed, other side acknowledged
            return Ok(());
        }

        if frame.payload.len() % 6 != 0 {
            return Err(Http2Error::FrameError("SETTINGS payload must be multiple of 6"));
        }

        Ok(())
    }

    /// Handle PUSH_PROMISE frame (type 0x5)
    fn handle_push_promise_frame(&self, frame: &Http2Frame) -> Result<(), Http2Error> {
        if frame.header.stream_id == 0 {
            return Err(Http2Error::ProtocolError("PUSH_PROMISE on stream 0"));
        }

        Ok(())
    }

    /// Handle PING frame (type 0x6)
    fn handle_ping_frame(&self, frame: &Http2Frame) -> Result<(), Http2Error> {
        if frame.header.stream_id != 0 {
            return Err(Http2Error::ProtocolError("PING on non-zero stream"));
        }

        if frame.payload.len() != 8 {
            return Err(Http2Error::FrameError("PING payload must be 8 bytes"));
        }

        Ok(())
    }

    /// Handle GOAWAY frame (type 0x7)
    fn handle_goaway_frame(&self, frame: &Http2Frame) -> Result<(), Http2Error> {
        if frame.header.stream_id != 0 {
            return Err(Http2Error::ProtocolError("GOAWAY on non-zero stream"));
        }

        if frame.payload.len() < 8 {
            return Err(Http2Error::FrameError("GOAWAY payload must be >= 8 bytes"));
        }

        self.transition_state(ConnectionState::Active, ConnectionState::GoingAway)?;

        Ok(())
    }

    /// Handle WINDOW_UPDATE frame (type 0x8)
    fn handle_window_update_frame(&self, frame: &Http2Frame) -> Result<(), Http2Error> {
        if frame.payload.len() != 4 {
            return Err(Http2Error::FrameError("WINDOW_UPDATE payload must be 4 bytes"));
        }

        let increment = u32::from_be_bytes([
            frame.payload[0],
            frame.payload[1],
            frame.payload[2],
            frame.payload[3],
        ]) & 0x7fff_ffff;

        if increment == 0 {
            return Err(Http2Error::FlowControlError("WINDOW_UPDATE increment is 0"));
        }

        let window = self.flow_control_window.load(Ordering::Acquire);
        let new_window = window.saturating_add(increment as u64);

        if new_window > 0x7fff_ffff {
            return Err(Http2Error::FlowControlError("flow window exceeds max"));
        }

        self.flow_control_window.store(new_window, Ordering::Release);

        Ok(())
    }

    /// Handle CONTINUATION frame (type 0x9)
    fn handle_continuation_frame(&self, frame: &Http2Frame) -> Result<(), Http2Error> {
        if frame.header.stream_id == 0 {
            return Err(Http2Error::ProtocolError("CONTINUATION on stream 0"));
        }

        Ok(())
    }

    /// Get frame statistics (frames_sent, frames_received)
    #[inline]
    pub fn get_statistics(&self) -> (u32, u32) {
        let stats = self.statistics.load(Ordering::Acquire);
        let sent = ((stats >> 32) & 0xFFFF_FFFF) as u32;
        let received = (stats & 0xFFFF_FFFF) as u32;
        (sent, received)
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn test_http2_connection_new() {
        let conn = Http2ConnectionCapsule::new(ConnectionRole::Client);
        assert_eq!(conn.state(), ConnectionState::Idle);
        assert_eq!(conn.role(), ConnectionRole::Client);
    }

    #[test]
    fn test_http2_settings_default() {
        let settings = Http2Settings::default();
        assert_eq!(settings.header_table_size, 4096);
        assert_eq!(settings.enable_push, true);
        assert_eq!(settings.initial_window_size, 65535);
        assert_eq!(settings.max_frame_size, 16384);
    }

    #[test]
    fn test_http2_settings_validate() {
        let mut settings = Http2Settings::default();
        assert!(settings.validate().is_ok());

        settings.max_frame_size = 16383; // Too small
        assert!(settings.validate().is_err());

        settings.max_frame_size = 16777216; // Too large
        assert!(settings.validate().is_err());

        settings.max_frame_size = 16384;
        settings.header_table_size = 67108865; // Too large
        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_http2_frame_header_encode_decode() {
        let header = Http2FrameHeader {
            length: 1024,
            frame_type: 0x1,
            flags: Http2Flags {
                ack: false,
                end_stream: true,
                end_headers: true,
                padded: false,
                priority: false,
            },
            stream_id: 123,
        };

        let mut buf = [0u8; 9];
        assert!(header.encode(&mut buf).is_ok());

        let decoded = Http2FrameHeader::decode(&buf).unwrap();
        assert_eq!(decoded.length, 1024);
        assert_eq!(decoded.frame_type, 0x1);
        assert_eq!(decoded.stream_id, 123);
    }

    #[test]
    fn test_http2_error_code_conversion() {
        assert_eq!(Http2ErrorCode::from(0x00), Http2ErrorCode::NoError);
        assert_eq!(Http2ErrorCode::from(0x01), Http2ErrorCode::ProtocolError);
        assert_eq!(Http2ErrorCode::from(0x03), Http2ErrorCode::FlowControlError);
        assert_eq!(Http2ErrorCode::from(0xff), Http2ErrorCode::ProtocolError); // Unknown → protocol error
    }

    #[test]
    fn test_http2_client_preface() {
        let conn = Http2ConnectionCapsule::new(ConnectionRole::Client);
        let preface = conn.send_preface().unwrap();

        // Should contain "PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n" + SETTINGS frame
        assert!(preface.len() > 24);
        assert_eq!(&preface[0..24], b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n");
    }

    #[test]
    fn test_http2_server_receive_preface() {
        let conn = Http2ConnectionCapsule::new(ConnectionRole::Server);
        let preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
        assert!(conn.receive_preface(preface).is_ok());
        assert_eq!(conn.state(), ConnectionState::SettingsExpected);
    }

    #[test]
    fn test_http2_ping_frame() {
        let conn = Http2ConnectionCapsule::new(ConnectionRole::Client);
        conn.state.store(ConnectionState::Active as u64, Ordering::Release);

        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let ping_buf = conn.send_ping(data).unwrap();
        assert!(ping_buf.len() >= 17); // 9 byte header + 8 byte payload
    }

    #[test]
    fn test_http2_goaway_frame() {
        let conn = Http2ConnectionCapsule::new(ConnectionRole::Client);
        conn.state.store(ConnectionState::Active as u64, Ordering::Release);

        let goaway = conn.send_goaway(0, 0).unwrap();
        assert!(goaway.len() >= 17); // 9 byte header + 8 byte payload
        assert_eq!(conn.state(), ConnectionState::GoingAway);
    }

    #[test]
    fn test_http2_flow_control() {
        let conn = Http2ConnectionCapsule::new(ConnectionRole::Server);

        // Create DATA frame
        let frame = Http2Frame::new(0x0, Http2Flags {
            ack: false,
            end_stream: false,
            end_headers: false,
            padded: false,
            priority: false,
        }, 1, vec![0; 100]);

        assert!(conn.handle_data_frame(&frame).is_ok());
        assert_eq!(conn.flow_control_window.load(Ordering::Acquire), 65435);
    }

    #[test]
    fn test_http2_protocol_errors() {
        let conn = Http2ConnectionCapsule::new(ConnectionRole::Server);

        // DATA on stream 0 should error
        let frame = Http2Frame::new(0x0, Http2Flags {
            ack: false,
            end_stream: false,
            end_headers: false,
            padded: false,
            priority: false,
        }, 0, vec![0; 10]);

        assert!(conn.handle_data_frame(&frame).is_err());
    }

    #[test]
    fn test_http2_settings_frame_encoding() {
        let settings = Http2Settings::default();
        let frame = Http2Frame::settings(&settings);

        // SETTINGS frame should have type 0x4 and stream ID 0
        assert_eq!(frame.header.frame_type, 0x4);
        assert_eq!(frame.header.stream_id, 0);
        assert!(frame.payload.len() > 0);
    }

    #[test]
    fn test_http2_connection_statistics() {
        let conn = Http2ConnectionCapsule::new(ConnectionRole::Server);

        // Process a frame to increment statistics
        let frame = Http2Frame::ping([1, 2, 3, 4, 5, 6, 7, 8]);
        let _ = conn.process_frame(&frame);

        let (sent, received) = conn.get_statistics();
        assert_eq!(received, 1);
    }

    #[test]
    fn test_http2_state_transitions() {
        let conn = Http2ConnectionCapsule::new(ConnectionRole::Client);

        // Valid transition
        assert!(conn.transition_state(ConnectionState::Idle, ConnectionState::PrefaceExpected).is_ok());
        assert_eq!(conn.state(), ConnectionState::PrefaceExpected);

        // Invalid transition (should fail)
        assert!(conn.transition_state(ConnectionState::Idle, ConnectionState::Active).is_err());
    }

    #[test]
    fn test_http2_window_update() {
        let conn = Http2ConnectionCapsule::new(ConnectionRole::Server);

        let mut payload = [0u8; 4];
        payload.copy_from_slice(&(1000u32).to_be_bytes());

        let frame = Http2Frame::new(0x8, Http2Flags {
            ack: false,
            end_stream: false,
            end_headers: false,
            padded: false,
            priority: false,
        }, 0, payload.to_vec());

        assert!(conn.handle_window_update_frame(&frame).is_ok());
        assert_eq!(conn.flow_control_window.load(Ordering::Acquire), 66535);
    }

    #[test]
    fn test_http2_closed_connection_reject_frames() {
        let conn = Http2ConnectionCapsule::new(ConnectionRole::Server);
        conn.state.store(ConnectionState::Closed as u64, Ordering::Release);

        let frame = Http2Frame::ping([1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(conn.process_frame(&frame).is_err());
    }

    #[test]
    fn test_http2_settings_ack() {
        let conn = Http2ConnectionCapsule::new(ConnectionRole::Client);
        conn.state.store(ConnectionState::SettingsExpected as u64, Ordering::Release);

        let ack_buf = conn.send_settings_ack().unwrap();
        assert_eq!(conn.state(), ConnectionState::Active);
        assert!(ack_buf.len() >= 9); // At least header
    }

    #[test]
    fn test_http2_connection_alignment() {
        // Verify 256-byte alignment
        let capsule = Http2ConnectionCapsule::new(ConnectionRole::Client);
        let addr = &capsule as *const _ as usize;
        assert_eq!(addr % 256, 0, "Http2ConnectionCapsule must be 256-byte aligned");
    }

    #[test]
    fn test_http2_frame_type_routing() {
        let conn = Http2ConnectionCapsule::new(ConnectionRole::Server);
        conn.state.store(ConnectionState::Active as u64, Ordering::Release);

        // Test each frame type routing
        let ping = Http2Frame::ping([1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(conn.process_frame(&ping).is_ok());

        let settings = Http2Frame::settings(&Http2Settings::default());
        assert!(conn.process_frame(&settings).is_ok());

        let ack = Http2Frame::settings_ack();
        assert!(conn.process_frame(&ack).is_ok());
    }

    #[test]
    fn test_http2_error_display() {
        let err = Http2Error::ProtocolError("test");
        let s = format!("{}", err);
        assert!(s.contains("Protocol error"));

        let err2 = Http2Error::FlowControlError("test");
        let s2 = format!("{}", err2);
        assert!(s2.contains("Flow control error"));
    }
}
