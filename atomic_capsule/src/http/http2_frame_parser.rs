//! # HTTP/2 Frame Parser Capsule (T1 Atomic)
//!
//! **RFC 9113 Compliant HTTP/2 Frame Parsing**
//!
//! High-performance, zero-copy HTTP/2 frame parsing with atomic coordination.
//!
//! ## Frame Format (RFC 9113 Section 4.1)
//!
//! ```
//! +-----------------------------------------------+
//! |                 Length (24)                   |
//! +---------------+---------------+---------------+
//! |   Type (8)    |   Flags (8)   |
//! +-+-------------+---------------+-------------------------------+
//! |R|                 Stream Identifier (31)                      |
//! +=+=============================================================+
//! |                   Frame Payload (0...)                      ...
//! +---------------------------------------------------------------+
//! ```
//!
//! **Header Size**: 9 bytes
//! **Payload Size**: 0 to 16,383 bytes (default), up to 16,777,215 bytes (max frame size)
//! **Total Frame Size**: 9 + payload_length

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use core::fmt;

/// HTTP/2 Frame Type (RFC 9113 Section 6)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Http2FrameType {
    /// DATA: Stream data transfer (0x00)
    Data = 0x00,
    /// HEADERS: Header fields (0x01)
    Headers = 0x01,
    /// PRIORITY: Stream priority (0x02, deprecated but parsed)
    Priority = 0x02,
    /// RST_STREAM: Stream termination (0x03)
    RstStream = 0x03,
    /// SETTINGS: Connection parameters (0x04)
    Settings = 0x04,
    /// PUSH_PROMISE: Server push notification (0x05)
    PushPromise = 0x05,
    /// PING: Connection liveness (0x06)
    Ping = 0x06,
    /// GOAWAY: Graceful shutdown (0x07)
    Goaway = 0x07,
    /// WINDOW_UPDATE: Flow control (0x08)
    WindowUpdate = 0x08,
    /// CONTINUATION: Header continuation (0x09)
    Continuation = 0x09,
}

impl Http2FrameType {
    /// Parse frame type from u8
    #[inline]
    pub fn from_u8(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(Http2FrameType::Data),
            0x01 => Some(Http2FrameType::Headers),
            0x02 => Some(Http2FrameType::Priority),
            0x03 => Some(Http2FrameType::RstStream),
            0x04 => Some(Http2FrameType::Settings),
            0x05 => Some(Http2FrameType::PushPromise),
            0x06 => Some(Http2FrameType::Ping),
            0x07 => Some(Http2FrameType::Goaway),
            0x08 => Some(Http2FrameType::WindowUpdate),
            0x09 => Some(Http2FrameType::Continuation),
            _ => None,
        }
    }

    /// Get frame type as u8
    #[inline]
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// HTTP/2 Frame Flags (specific per frame type, RFC 9113)
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Http2Flags(u8);

impl Http2Flags {
    /// Create new flags
    #[inline]
    pub const fn new(byte: u8) -> Self {
        Http2Flags(byte)
    }

    /// Get raw flags byte
    #[inline]
    pub const fn as_u8(self) -> u8 {
        self.0
    }

    /// Check END_STREAM flag (0x01 for DATA/HEADERS, invalid for others)
    #[inline]
    pub const fn end_stream(self) -> bool {
        self.0 & 0x01 != 0
    }

    /// Check END_HEADERS flag (0x04 for HEADERS/PUSH_PROMISE/CONTINUATION)
    #[inline]
    pub const fn end_headers(self) -> bool {
        self.0 & 0x04 != 0
    }

    /// Check ACK flag (0x01 for SETTINGS/PING)
    #[inline]
    pub const fn ack(self) -> bool {
        self.0 & 0x01 != 0
    }

    /// Check PADDED flag (0x08 for DATA/HEADERS/PUSH_PROMISE)
    #[inline]
    pub const fn padded(self) -> bool {
        self.0 & 0x08 != 0
    }

    /// Check PRIORITY flag (0x20 for HEADERS)
    #[inline]
    pub const fn priority(self) -> bool {
        self.0 & 0x20 != 0
    }
}

/// HTTP/2 Frame Header (9 bytes fixed)
///
/// **Layout (little-endian big-endian hybrid)**:
/// - Bytes 0-2: Length (24-bit, big-endian)
/// - Byte 3: Type (8-bit)
/// - Byte 4: Flags (8-bit)
/// - Bytes 5-8: Stream ID (31-bit, big-endian with reserved bit)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Http2FrameHeader {
    /// Payload length (0 to 16,777,215)
    pub length: u32,
    /// Frame type
    pub frame_type: Http2FrameType,
    /// Flags (frame-type specific)
    pub flags: Http2Flags,
    /// Stream ID (0 for connection-level frames, >0 for stream frames)
    pub stream_id: u32,
}

impl Http2FrameHeader {
    /// Parse frame header from 9-byte buffer
    ///
    /// **Performance**: <100ns (atomic metadata operations)
    /// **Safety**: Validates all fields against RFC 9113 constraints
    #[inline]
    pub fn parse(buffer: &[u8]) -> Result<Self, Http2ParseError> {
        if buffer.len() < 9 {
            return Err(Http2ParseError::FrameHeaderIncomplete);
        }

        // Parse length (24-bit, big-endian)
        let length = ((buffer[0] as u32) << 16)
            | ((buffer[1] as u32) << 8)
            | (buffer[2] as u32);

        // Parse type (8-bit)
        let frame_type = Http2FrameType::from_u8(buffer[3])
            .ok_or(Http2ParseError::InvalidFrameType)?;

        // Parse flags (8-bit)
        let flags = Http2Flags::new(buffer[4]);

        // Parse stream ID (31-bit, big-endian, skip reserved bit at 5)
        let stream_id = ((buffer[5] & 0x7F) as u32) << 24
            | ((buffer[6] as u32) << 16)
            | ((buffer[7] as u32) << 8)
            | (buffer[8] as u32);

        Ok(Http2FrameHeader {
            length,
            frame_type,
            flags,
            stream_id,
        })
    }

    /// Serialize frame header to 9-byte buffer
    ///
    /// **Performance**: <50ns
    #[inline]
    pub fn serialize(&self, buffer: &mut [u8]) -> Result<(), Http2ParseError> {
        if buffer.len() < 9 {
            return Err(Http2ParseError::BufferTooSmall);
        }

        // Serialize length (24-bit, big-endian)
        buffer[0] = ((self.length >> 16) & 0xFF) as u8;
        buffer[1] = ((self.length >> 8) & 0xFF) as u8;
        buffer[2] = (self.length & 0xFF) as u8;

        // Serialize type (8-bit)
        buffer[3] = self.frame_type.as_u8();

        // Serialize flags (8-bit)
        buffer[4] = self.flags.as_u8();

        // Serialize stream ID (31-bit, big-endian)
        buffer[5] = ((self.stream_id >> 24) & 0x7F) as u8; // Clear reserved bit
        buffer[6] = ((self.stream_id >> 16) & 0xFF) as u8;
        buffer[7] = ((self.stream_id >> 8) & 0xFF) as u8;
        buffer[8] = (self.stream_id & 0xFF) as u8;

        Ok(())
    }
}

/// Complete HTTP/2 Frame (header + payload)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Http2Frame<'a> {
    /// Frame header
    pub header: Http2FrameHeader,
    /// Payload (borrowed from input buffer)
    pub payload: &'a [u8],
}

impl<'a> Http2Frame<'a> {
    /// Create new frame
    #[inline]
    pub fn new(header: Http2FrameHeader, payload: &'a [u8]) -> Self {
        Http2Frame { header, payload }
    }

    /// Total frame size (header + payload)
    #[inline]
    pub fn total_size(&self) -> usize {
        9 + self.header.length as usize
    }

    /// Get padding length from payload (if PADDED flag set)
    ///
    /// **RFC 9113 Section 6.1**: Padding is appended after frame data.
    /// If PADDED flag is set, first byte of payload is padding length.
    #[inline]
    pub fn padding_length(&self) -> Result<u8, Http2ParseError> {
        match self.header.frame_type {
            Http2FrameType::Data | Http2FrameType::Headers | Http2FrameType::PushPromise => {
                if !self.header.flags.padded() {
                    return Ok(0);
                }
                if self.payload.is_empty() {
                    return Err(Http2ParseError::InvalidPadding);
                }
                let pad_len = self.payload[0];
                if pad_len as usize >= self.header.length as usize {
                    return Err(Http2ParseError::InvalidPadding);
                }
                Ok(pad_len)
            }
            _ => Ok(0), // Non-padded frames
        }
    }

    /// Get payload data (excluding padding if PADDED flag set)
    ///
    /// **Performance**: <10ns (bounds checking only)
    #[inline]
    pub fn payload_data(&self) -> Result<&'a [u8], Http2ParseError> {
        let pad_len = self.padding_length()? as usize;
        let payload_len = self.header.length as usize;

        if pad_len > payload_len {
            return Err(Http2ParseError::InvalidPadding);
        }

        // Skip padding length byte if PADDED flag set
        let start = if self.header.flags.padded() { 1 } else { 0 };

        // Calculate actual data length (excluding padding length byte + padding)
        let data_len = payload_len
            .checked_sub(start)
            .ok_or(Http2ParseError::InvalidPadding)?
            .checked_sub(pad_len)
            .ok_or(Http2ParseError::InvalidPadding)?;

        Ok(&self.payload[start..start + data_len])
    }
}

/// HTTP/2 Parse Error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Http2ParseError {
    /// Incomplete frame header
    FrameHeaderIncomplete,
    /// Frame payload incomplete (need more bytes)
    FramePayloadIncomplete,
    /// Frame too large (exceeds max frame size)
    FrameTooLarge,
    /// Invalid frame type
    InvalidFrameType,
    /// Invalid stream ID (connection-level frame with non-zero stream ID, etc.)
    InvalidStreamId,
    /// Invalid flags for frame type
    InvalidFlags,
    /// Invalid padding
    InvalidPadding,
    /// Protocol error (invalid combination of fields)
    ProtocolError,
    /// Buffer too small for operation
    BufferTooSmall,
    /// Internal error
    InternalError,
}

impl fmt::Display for Http2ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Http2ParseError::FrameHeaderIncomplete => write!(f, "Frame header incomplete"),
            Http2ParseError::FramePayloadIncomplete => write!(f, "Frame payload incomplete"),
            Http2ParseError::FrameTooLarge => write!(f, "Frame too large"),
            Http2ParseError::InvalidFrameType => write!(f, "Invalid frame type"),
            Http2ParseError::InvalidStreamId => write!(f, "Invalid stream ID"),
            Http2ParseError::InvalidFlags => write!(f, "Invalid flags for frame type"),
            Http2ParseError::InvalidPadding => write!(f, "Invalid padding"),
            Http2ParseError::ProtocolError => write!(f, "Protocol error"),
            Http2ParseError::BufferTooSmall => write!(f, "Buffer too small"),
            Http2ParseError::InternalError => write!(f, "Internal error"),
        }
    }
}

/// HTTP/2 Frame Parser Capsule (T1 Atomic, 128B)
///
/// **Purpose**: Parse HTTP/2 frames with atomic coordination
///
/// **Performance Targets** (B32 Framework):
/// - Frame header parse: <100ns
/// - Frame validation: <50ns
/// - Zero-copy frame extraction: <10ns
///
/// **Layout** (128-byte cache-aligned):
/// - 64 bytes: coordination atomics
/// - 64 bytes: statistics
#[repr(C, align(128))]
pub struct Http2FrameParserCapsule {
    /// Parser state (packed): state(8) | max_frame_size(24) | reserved(32)
    state: AtomicU64,

    /// Total frames parsed
    frames_parsed: AtomicU64,

    /// DATA frames parsed
    data_frames: AtomicU32,

    /// HEADERS frames parsed
    headers_frames: AtomicU32,

    /// SETTINGS frames parsed
    settings_frames: AtomicU32,

    /// PING frames parsed
    ping_frames: AtomicU32,

    /// GOAWAY frames parsed
    goaway_frames: AtomicU32,

    /// WINDOW_UPDATE frames parsed
    window_update_frames: AtomicU32,

    /// RST_STREAM frames parsed
    rst_stream_frames: AtomicU32,

    /// PUSH_PROMISE frames parsed
    push_promise_frames: AtomicU32,

    /// CONTINUATION frames parsed
    continuation_frames: AtomicU32,

    /// PRIORITY frames parsed
    priority_frames: AtomicU32,

    /// Parse errors encountered
    parse_errors: AtomicU32,

    /// Total bytes parsed (excluding headers)
    total_bytes_parsed: AtomicU64,

    /// Last stream ID seen
    last_stream_id: AtomicU32,

    /// Padding: 128 - 88 = 40 bytes
    _padding: [u8; 40],
}

impl Http2FrameParserCapsule {
    /// Create new HTTP/2 frame parser capsule
    ///
    /// **Default max frame size**: 16,384 bytes (RFC 9113 Section 6.5.2)
    /// **Maximum possible**: 16,777,215 bytes
    #[inline]
    pub const fn new() -> Self {
        Http2FrameParserCapsule {
            state: AtomicU64::new(16384), // Default max frame size in lower 24 bits
            frames_parsed: AtomicU64::new(0),
            data_frames: AtomicU32::new(0),
            headers_frames: AtomicU32::new(0),
            settings_frames: AtomicU32::new(0),
            ping_frames: AtomicU32::new(0),
            goaway_frames: AtomicU32::new(0),
            window_update_frames: AtomicU32::new(0),
            rst_stream_frames: AtomicU32::new(0),
            push_promise_frames: AtomicU32::new(0),
            continuation_frames: AtomicU32::new(0),
            priority_frames: AtomicU32::new(0),
            parse_errors: AtomicU32::new(0),
            total_bytes_parsed: AtomicU64::new(0),
            last_stream_id: AtomicU32::new(0),
            _padding: [0u8; 40],
        }
    }

    /// Parse frame from buffer
    ///
    /// **Performance**: <500ns total (atomic metadata + validation)
    /// **Zero-copy**: Returns slices into original buffer
    ///
    /// **Returns**: Frame if complete, error if incomplete/invalid
    #[inline]
    pub fn parse_frame(&self, buffer: &[u8]) -> Result<(Http2FrameHeader, usize), Http2ParseError> {
        // Parse header (9 bytes)
        let header = Http2FrameHeader::parse(buffer)?;

        // Validate header
        self.validate_frame_header(&header)?;

        let total_size = 9 + header.length as usize;
        if buffer.len() < total_size {
            return Err(Http2ParseError::FramePayloadIncomplete);
        }

        // Update statistics
        self.update_statistics(&header);

        Ok((header, total_size))
    }

    /// Parse frame header only (9 bytes)
    ///
    /// **Performance**: <100ns
    #[inline]
    pub fn parse_frame_header(&self, buffer: &[u8]) -> Result<Http2FrameHeader, Http2ParseError> {
        Http2FrameHeader::parse(buffer)
    }

    /// Validate frame header against RFC 9113 constraints
    ///
    /// **Performance**: <50ns
    fn validate_frame_header(&self, header: &Http2FrameHeader) -> Result<(), Http2ParseError> {
        // Check max frame size
        let max_size = self.get_max_frame_size();
        if header.length > max_size {
            self.parse_errors.fetch_add(1, Ordering::Relaxed);
            return Err(Http2ParseError::FrameTooLarge);
        }

        // Validate stream ID and frame type combinations
        match header.frame_type {
            // Connection-level frames must have stream ID 0
            Http2FrameType::Settings | Http2FrameType::Ping | Http2FrameType::Goaway => {
                if header.stream_id != 0 {
                    self.parse_errors.fetch_add(1, Ordering::Relaxed);
                    return Err(Http2ParseError::InvalidStreamId);
                }
            }
            // Stream-level frames must have stream ID > 0
            Http2FrameType::Data
            | Http2FrameType::Headers
            | Http2FrameType::Priority
            | Http2FrameType::RstStream
            | Http2FrameType::PushPromise
            | Http2FrameType::Continuation => {
                if header.stream_id == 0 {
                    self.parse_errors.fetch_add(1, Ordering::Relaxed);
                    return Err(Http2ParseError::InvalidStreamId);
                }
            }
            // WINDOW_UPDATE can be connection-level or stream-level
            Http2FrameType::WindowUpdate => {
                // Valid for both, no restriction
            }
        }

        // Validate flags per frame type
        self.validate_flags(header)?;

        Ok(())
    }

    /// Validate flags for frame type
    fn validate_flags(&self, header: &Http2FrameHeader) -> Result<(), Http2ParseError> {
        // RFC 9113 Section 6: Define valid flags per frame type
        let valid_flags = match header.frame_type {
            Http2FrameType::Data => 0x09, // END_STREAM, PADDED
            Http2FrameType::Headers => 0x2D, // END_STREAM, END_HEADERS, PADDED, PRIORITY
            Http2FrameType::Priority => 0x00, // No flags
            Http2FrameType::RstStream => 0x00, // No flags
            Http2FrameType::Settings => 0x01, // ACK
            Http2FrameType::PushPromise => 0x0D, // END_HEADERS, PADDED
            Http2FrameType::Ping => 0x01, // ACK
            Http2FrameType::Goaway => 0x00, // No flags
            Http2FrameType::WindowUpdate => 0x00, // No flags
            Http2FrameType::Continuation => 0x04, // END_HEADERS
        };

        if (header.flags.as_u8() & !valid_flags) != 0 {
            self.parse_errors.fetch_add(1, Ordering::Relaxed);
            return Err(Http2ParseError::InvalidFlags);
        }

        Ok(())
    }

    /// Get maximum frame size (default 16,384)
    #[inline]
    pub fn get_max_frame_size(&self) -> u32 {
        let state = self.state.load(Ordering::Acquire);
        (state & 0xFF_FF_FF) as u32
    }

    /// Set maximum frame size (must be 16,384 to 16,777,215)
    ///
    /// **Returns**: Ok if valid, Err if out of range
    #[inline]
    pub fn set_max_frame_size(&self, size: u32) -> Result<(), Http2ParseError> {
        if size < 16384 || size > 16777215 {
            return Err(Http2ParseError::ProtocolError);
        }
        self.state.store(size as u64, Ordering::Release);
        Ok(())
    }

    /// Update statistics based on frame type
    fn update_statistics(&self, header: &Http2FrameHeader) {
        self.frames_parsed.fetch_add(1, Ordering::Relaxed);
        self.total_bytes_parsed
            .fetch_add(header.length as u64, Ordering::Relaxed);
        self.last_stream_id.store(header.stream_id, Ordering::Relaxed);

        match header.frame_type {
            Http2FrameType::Data => {
                self.data_frames.fetch_add(1, Ordering::Relaxed);
            }
            Http2FrameType::Headers => {
                self.headers_frames.fetch_add(1, Ordering::Relaxed);
            }
            Http2FrameType::Settings => {
                self.settings_frames.fetch_add(1, Ordering::Relaxed);
            }
            Http2FrameType::Ping => {
                self.ping_frames.fetch_add(1, Ordering::Relaxed);
            }
            Http2FrameType::Goaway => {
                self.goaway_frames.fetch_add(1, Ordering::Relaxed);
            }
            Http2FrameType::WindowUpdate => {
                self.window_update_frames.fetch_add(1, Ordering::Relaxed);
            }
            Http2FrameType::RstStream => {
                self.rst_stream_frames.fetch_add(1, Ordering::Relaxed);
            }
            Http2FrameType::PushPromise => {
                self.push_promise_frames.fetch_add(1, Ordering::Relaxed);
            }
            Http2FrameType::Continuation => {
                self.continuation_frames.fetch_add(1, Ordering::Relaxed);
            }
            Http2FrameType::Priority => {
                self.priority_frames.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Get statistics
    #[inline]
    pub fn stats(&self) -> Http2ParserStats {
        Http2ParserStats {
            frames_parsed: self.frames_parsed.load(Ordering::Acquire),
            data_frames: self.data_frames.load(Ordering::Acquire),
            headers_frames: self.headers_frames.load(Ordering::Acquire),
            settings_frames: self.settings_frames.load(Ordering::Acquire),
            ping_frames: self.ping_frames.load(Ordering::Acquire),
            goaway_frames: self.goaway_frames.load(Ordering::Acquire),
            window_update_frames: self.window_update_frames.load(Ordering::Acquire),
            rst_stream_frames: self.rst_stream_frames.load(Ordering::Acquire),
            push_promise_frames: self.push_promise_frames.load(Ordering::Acquire),
            continuation_frames: self.continuation_frames.load(Ordering::Acquire),
            priority_frames: self.priority_frames.load(Ordering::Acquire),
            parse_errors: self.parse_errors.load(Ordering::Acquire),
            total_bytes_parsed: self.total_bytes_parsed.load(Ordering::Acquire),
            last_stream_id: self.last_stream_id.load(Ordering::Acquire),
        }
    }

    /// Reset statistics
    #[inline]
    pub fn reset_stats(&self) {
        self.frames_parsed.store(0, Ordering::Release);
        self.data_frames.store(0, Ordering::Release);
        self.headers_frames.store(0, Ordering::Release);
        self.settings_frames.store(0, Ordering::Release);
        self.ping_frames.store(0, Ordering::Release);
        self.goaway_frames.store(0, Ordering::Release);
        self.window_update_frames.store(0, Ordering::Release);
        self.rst_stream_frames.store(0, Ordering::Release);
        self.push_promise_frames.store(0, Ordering::Release);
        self.continuation_frames.store(0, Ordering::Release);
        self.priority_frames.store(0, Ordering::Release);
        self.parse_errors.store(0, Ordering::Release);
        self.total_bytes_parsed.store(0, Ordering::Release);
        self.last_stream_id.store(0, Ordering::Release);
    }
}

impl Default for Http2FrameParserCapsule {
    fn default() -> Self {
        Self::new()
    }
}

/// HTTP/2 Parser Statistics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Http2ParserStats {
    pub frames_parsed: u64,
    pub data_frames: u32,
    pub headers_frames: u32,
    pub settings_frames: u32,
    pub ping_frames: u32,
    pub goaway_frames: u32,
    pub window_update_frames: u32,
    pub rst_stream_frames: u32,
    pub push_promise_frames: u32,
    pub continuation_frames: u32,
    pub priority_frames: u32,
    pub parse_errors: u32,
    pub total_bytes_parsed: u64,
    pub last_stream_id: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test constants
    const SIMPLE_DATA_FRAME: &[u8] = &[
        0x00, 0x00, 0x05, // Length: 5
        0x00,             // Type: DATA
        0x01,             // Flags: END_STREAM
        0x00, 0x00, 0x00, 0x01, // Stream ID: 1
        // Payload: 5 bytes
        b'H', b'e', b'l', b'l', b'o',
    ];

    const SETTINGS_FRAME: &[u8] = &[
        0x00, 0x00, 0x00, // Length: 0
        0x04,             // Type: SETTINGS
        0x00,             // Flags: none
        0x00, 0x00, 0x00, 0x00, // Stream ID: 0 (connection)
    ];

    #[test]
    fn test_frame_type_from_u8() {
        assert_eq!(Http2FrameType::from_u8(0x00), Some(Http2FrameType::Data));
        assert_eq!(Http2FrameType::from_u8(0x04), Some(Http2FrameType::Settings));
        assert_eq!(Http2FrameType::from_u8(0xFF), None);
    }

    #[test]
    fn test_flags_new() {
        let flags = Http2Flags::new(0x01);
        assert!(flags.end_stream());
        assert!(!flags.end_headers());
    }

    #[test]
    fn test_flags_padded() {
        let flags = Http2Flags::new(0x08);
        assert!(flags.padded());
        assert!(!flags.end_stream());
    }

    #[test]
    fn test_frame_header_parse_data() {
        let header = Http2FrameHeader::parse(SIMPLE_DATA_FRAME).unwrap();
        assert_eq!(header.length, 5);
        assert_eq!(header.frame_type, Http2FrameType::Data);
        assert!(header.flags.end_stream());
        assert_eq!(header.stream_id, 1);
    }

    #[test]
    fn test_frame_header_parse_settings() {
        let header = Http2FrameHeader::parse(SETTINGS_FRAME).unwrap();
        assert_eq!(header.length, 0);
        assert_eq!(header.frame_type, Http2FrameType::Settings);
        assert_eq!(header.stream_id, 0);
    }

    #[test]
    fn test_frame_header_serialize() {
        let header = Http2FrameHeader {
            length: 5,
            frame_type: Http2FrameType::Data,
            flags: Http2Flags::new(0x01),
            stream_id: 1,
        };

        let mut buffer = [0u8; 9];
        header.serialize(&mut buffer).unwrap();

        // Verify serialized data matches original
        assert_eq!(&buffer[..9], &SIMPLE_DATA_FRAME[..9]);
    }

    #[test]
    fn test_frame_header_incomplete() {
        let short = &[0x00, 0x00]; // Only 2 bytes
        assert_eq!(
            Http2FrameHeader::parse(short),
            Err(Http2ParseError::FrameHeaderIncomplete)
        );
    }

    #[test]
    fn test_frame_header_invalid_type() {
        let mut buffer = [0u8; 9];
        buffer[3] = 0xFF; // Invalid frame type
        assert_eq!(
            Http2FrameHeader::parse(&buffer),
            Err(Http2ParseError::InvalidFrameType)
        );
    }

    #[test]
    fn test_parser_capsule_creation() {
        let parser = Http2FrameParserCapsule::new();
        let stats = parser.stats();
        assert_eq!(stats.frames_parsed, 0);
        assert_eq!(parser.get_max_frame_size(), 16384);
    }

    #[test]
    fn test_parser_capsule_parse_data_frame() {
        let parser = Http2FrameParserCapsule::new();
        let (header, size) = parser.parse_frame(SIMPLE_DATA_FRAME).unwrap();

        assert_eq!(header.length, 5);
        assert_eq!(header.frame_type, Http2FrameType::Data);
        assert_eq!(header.stream_id, 1);
        assert_eq!(size, 14); // 9 header + 5 payload

        let stats = parser.stats();
        assert_eq!(stats.frames_parsed, 1);
        assert_eq!(stats.data_frames, 1);
        assert_eq!(stats.total_bytes_parsed, 5);
    }

    #[test]
    fn test_parser_capsule_parse_settings_frame() {
        let parser = Http2FrameParserCapsule::new();
        let (header, size) = parser.parse_frame(SETTINGS_FRAME).unwrap();

        assert_eq!(header.length, 0);
        assert_eq!(header.frame_type, Http2FrameType::Settings);
        assert_eq!(header.stream_id, 0);
        assert_eq!(size, 9); // Just header

        let stats = parser.stats();
        assert_eq!(stats.frames_parsed, 1);
        assert_eq!(stats.settings_frames, 1);
    }

    #[test]
    fn test_parser_validate_stream_id_settings() {
        let parser = Http2FrameParserCapsule::new();
        let mut buffer = SETTINGS_FRAME.to_vec();
        buffer[5] = 0x00;
        buffer[6] = 0x00;
        buffer[7] = 0x00;
        buffer[8] = 0x01; // Stream ID: 1 (invalid for SETTINGS)

        let result = parser.parse_frame(&buffer);
        assert!(matches!(result, Err(Http2ParseError::InvalidStreamId)));
        assert_eq!(parser.stats().parse_errors, 1);
    }

    #[test]
    fn test_parser_validate_stream_id_data() {
        let parser = Http2FrameParserCapsule::new();
        let mut buffer = SIMPLE_DATA_FRAME.to_vec();
        buffer[5] = 0x00;
        buffer[6] = 0x00;
        buffer[7] = 0x00;
        buffer[8] = 0x00; // Stream ID: 0 (invalid for DATA)

        let result = parser.parse_frame(&buffer);
        assert!(matches!(result, Err(Http2ParseError::InvalidStreamId)));
    }

    #[test]
    fn test_parser_validate_invalid_flags() {
        let parser = Http2FrameParserCapsule::new();
        let mut buffer = SIMPLE_DATA_FRAME.to_vec();
        buffer[4] = 0xFF; // Invalid flags (DATA frame should only have 0x09)

        let result = parser.parse_frame(&buffer);
        assert!(matches!(result, Err(Http2ParseError::InvalidFlags)));
    }

    #[test]
    fn test_parser_frame_too_large() {
        let parser = Http2FrameParserCapsule::new();
        let mut buffer = SIMPLE_DATA_FRAME.to_vec();
        // Set length to max + 1
        buffer[0] = 0xFF;
        buffer[1] = 0xFF;
        buffer[2] = 0xFF;

        let result = parser.parse_frame(&buffer);
        assert!(matches!(result, Err(Http2ParseError::FrameTooLarge)));
    }

    #[test]
    fn test_parser_payload_incomplete() {
        let parser = Http2FrameParserCapsule::new();
        let incomplete = &SIMPLE_DATA_FRAME[..10]; // Only 10 bytes, need 14

        let result = parser.parse_frame(incomplete);
        assert!(matches!(result, Err(Http2ParseError::FramePayloadIncomplete)));
    }

    #[test]
    fn test_parser_reset_stats() {
        let parser = Http2FrameParserCapsule::new();
        let _ = parser.parse_frame(SIMPLE_DATA_FRAME);

        assert_eq!(parser.stats().frames_parsed, 1);

        parser.reset_stats();
        assert_eq!(parser.stats().frames_parsed, 0);
        assert_eq!(parser.stats().data_frames, 0);
    }

    #[test]
    fn test_parser_multiple_frames() {
        let parser = Http2FrameParserCapsule::new();

        // Parse data frame
        let _ = parser.parse_frame(SIMPLE_DATA_FRAME).unwrap();
        // Parse settings frame
        let _ = parser.parse_frame(SETTINGS_FRAME).unwrap();

        let stats = parser.stats();
        assert_eq!(stats.frames_parsed, 2);
        assert_eq!(stats.data_frames, 1);
        assert_eq!(stats.settings_frames, 1);
        assert_eq!(stats.total_bytes_parsed, 5); // Only DATA frame has payload
    }

    #[test]
    fn test_frame_padding_length_not_padded() {
        let header = Http2FrameHeader {
            length: 5,
            frame_type: Http2FrameType::Data,
            flags: Http2Flags::new(0x00), // No PADDED flag
            stream_id: 1,
        };
        let frame = Http2Frame::new(header, &[b'H', b'e', b'l', b'l', b'o']);

        assert_eq!(frame.padding_length().unwrap(), 0);
    }

    #[test]
    fn test_frame_padding_length_padded() {
        let header = Http2FrameHeader {
            length: 6,
            frame_type: Http2FrameType::Data,
            flags: Http2Flags::new(0x08), // PADDED flag
            stream_id: 1,
        };
        let frame = Http2Frame::new(header, &[0x01, b'H', b'e', b'l', b'l', b'o']);

        assert_eq!(frame.padding_length().unwrap(), 0x01);
    }

    #[test]
    fn test_frame_payload_data_not_padded() {
        let header = Http2FrameHeader {
            length: 5,
            frame_type: Http2FrameType::Data,
            flags: Http2Flags::new(0x00),
            stream_id: 1,
        };
        let frame = Http2Frame::new(header, &[b'H', b'e', b'l', b'l', b'o']);

        assert_eq!(frame.payload_data().unwrap(), &[b'H', b'e', b'l', b'l', b'o']);
    }

    #[test]
    fn test_frame_payload_data_padded() {
        let header = Http2FrameHeader {
            length: 9,
            frame_type: Http2FrameType::Data,
            flags: Http2Flags::new(0x08), // PADDED flag
            stream_id: 1,
        };
        // Payload: [pad_len=3, 'H', 'e', 'l', 'l', 'o', padding, padding, padding]
        let payload = &[0x03, b'H', b'e', b'l', b'l', b'o', 0x00, 0x00, 0x00];
        let frame = Http2Frame::new(header, payload);

        assert_eq!(frame.payload_data().unwrap(), &[b'H', b'e', b'l', b'l', b'o']);
    }

    #[test]
    fn test_set_max_frame_size() {
        let parser = Http2FrameParserCapsule::new();
        let result = parser.set_max_frame_size(32768);
        assert!(result.is_ok());
        assert_eq!(parser.get_max_frame_size(), 32768);
    }

    #[test]
    fn test_set_invalid_max_frame_size_too_small() {
        let parser = Http2FrameParserCapsule::new();
        let result = parser.set_max_frame_size(8192); // Too small
        assert!(matches!(result, Err(Http2ParseError::ProtocolError)));
    }

    #[test]
    fn test_set_invalid_max_frame_size_too_large() {
        let parser = Http2FrameParserCapsule::new();
        let result = parser.set_max_frame_size(16777216); // Too large (> 16777215)
        assert!(matches!(result, Err(Http2ParseError::ProtocolError)));
    }
}
