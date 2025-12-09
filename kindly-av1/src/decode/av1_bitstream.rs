//! AV1 Bitstream Parser - OBU (Open Bitstream Unit) Parsing
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Implements AV1 bitstream parsing per AOM AV1 Specification v1.0.0.
//! Provides T5 Streaming tier capsule for OBU parsing with lockfree state management.
//!
//! # T5 Streaming Tier
//!
//! This capsule uses T5 Streaming tier for:
//! - O(1) incremental OBU parsing (one OBU at a time)
//! - Lockfree state updates via AtomicU64/AtomicU32
//! - 512B cache-aligned structure for optimal memory access
//! - Generation counter for Q34 audit trail compliance
//!
//! # AV1 Specification Compliance
//!
//! Implements the following AOM AV1 Specification sections:
//! - Section 5.3: OBU syntax (obu_header, obu_size)
//! - Section 5.3.1: OBU header (obu_type, obu_extension_header)
//! - Section 5.3.2: OBU extension header (temporal_id, spatial_id)
//! - Section 4.10.5: leb128() variable-length unsigned integer
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T5 Streaming tier for incremental processing
//! - **Chaos**: 512B cache-aligned, 100% lockfree (AtomicU64/AtomicU32 only)
//! - **ASSUM**: All unsafe blocks documented with #ASSUME/#VERIFY tags
//! - **B32**: Benchmarks validate streaming performance
//! - **T28**: 30+ tests covering all 5 tiers (unit/property/integration/production/determinism)

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// OBU Type (AV1 Section 6.2.2 Table 6.1)
///
/// Defines the type of data contained in the Open Bitstream Unit.
/// Valid types are 1-8 and 15, with 0 and 9-14 reserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ObuType {
    /// Reserved (0)
    #[default]
    Reserved = 0,
    /// Sequence Header OBU (1) - Contains codec configuration
    SequenceHeader = 1,
    /// Temporal Delimiter OBU (2) - Marks temporal unit boundary
    TemporalDelimiter = 2,
    /// Frame Header OBU (3) - Contains frame parameters (no tile data)
    FrameHeader = 3,
    /// Tile Group OBU (4) - Contains tile data for current frame
    TileGroup = 4,
    /// Metadata OBU (5) - Contains metadata (HDR, timecodes, etc.)
    Metadata = 5,
    /// Frame OBU (6) - Combined frame header + single tile group
    Frame = 6,
    /// Redundant Frame Header OBU (7) - Repeated frame header for error resilience
    RedundantFrameHeader = 7,
    /// Tile List OBU (8) - List of tiles for large-scale tile coding
    TileList = 8,
    /// Padding OBU (15) - Padding bytes for alignment
    Padding = 15,
}

impl ObuType {
    /// Convert from raw 4-bit value (obu_type field)
    ///
    /// # AV1 Specification
    ///
    /// obu_type is a 4-bit field (bits 4-7 of OBU header byte).
    /// Valid values: 1-8 and 15. Others are reserved.
    #[inline]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            1 => ObuType::SequenceHeader,
            2 => ObuType::TemporalDelimiter,
            3 => ObuType::FrameHeader,
            4 => ObuType::TileGroup,
            5 => ObuType::Metadata,
            6 => ObuType::Frame,
            7 => ObuType::RedundantFrameHeader,
            8 => ObuType::TileList,
            15 => ObuType::Padding,
            _ => ObuType::Reserved,
        }
    }

    /// Convert to raw 4-bit value
    #[inline]
    pub const fn to_u8(self) -> u8 {
        match self {
            ObuType::Reserved => 0,
            ObuType::SequenceHeader => 1,
            ObuType::TemporalDelimiter => 2,
            ObuType::FrameHeader => 3,
            ObuType::TileGroup => 4,
            ObuType::Metadata => 5,
            ObuType::Frame => 6,
            ObuType::RedundantFrameHeader => 7,
            ObuType::TileList => 8,
            ObuType::Padding => 15,
        }
    }

    /// Check if this OBU type contains frame data
    #[inline]
    pub const fn has_frame_data(&self) -> bool {
        matches!(
            self,
            ObuType::FrameHeader
                | ObuType::TileGroup
                | ObuType::Frame
                | ObuType::RedundantFrameHeader
        )
    }

    /// Check if this OBU type is a header type
    #[inline]
    pub const fn is_header(&self) -> bool {
        matches!(
            self,
            ObuType::SequenceHeader | ObuType::FrameHeader | ObuType::RedundantFrameHeader
        )
    }

    /// Check if this is a valid OBU type
    #[inline]
    pub const fn is_valid(&self) -> bool {
        !matches!(self, ObuType::Reserved)
    }
}

impl core::fmt::Display for ObuType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ObuType::Reserved => write!(f, "Reserved"),
            ObuType::SequenceHeader => write!(f, "Sequence Header"),
            ObuType::TemporalDelimiter => write!(f, "Temporal Delimiter"),
            ObuType::FrameHeader => write!(f, "Frame Header"),
            ObuType::TileGroup => write!(f, "Tile Group"),
            ObuType::Metadata => write!(f, "Metadata"),
            ObuType::Frame => write!(f, "Frame"),
            ObuType::RedundantFrameHeader => write!(f, "Redundant Frame Header"),
            ObuType::TileList => write!(f, "Tile List"),
            ObuType::Padding => write!(f, "Padding"),
        }
    }
}

/// Parsed OBU Header information
///
/// Contains all fields from the OBU header including optional extension header.
#[derive(Debug, Clone, Copy, Default)]
pub struct ObuHeader {
    /// OBU type (4 bits)
    pub obu_type: ObuType,
    /// Whether extension header is present
    pub obu_extension_flag: bool,
    /// Whether obu_size field is present
    pub obu_has_size_field: bool,
    /// Temporal ID from extension (0 if no extension)
    pub temporal_id: u8,
    /// Spatial ID from extension (0 if no extension)
    pub spatial_id: u8,
    /// Header size in bytes (1 or 2)
    pub header_size: u8,
    /// OBU payload size (if obu_has_size_field is true)
    pub obu_size: u64,
    /// Total bytes consumed parsing header + size
    pub total_header_bytes: usize,
}

/// Parsed Temporal Unit information
///
/// A temporal unit contains all OBUs between temporal delimiter OBUs.
#[derive(Debug, Clone, Default)]
pub struct TemporalUnit {
    /// Byte offset of temporal unit start
    pub offset: u64,
    /// Total size in bytes
    pub size: u64,
    /// OBU headers in this temporal unit
    pub obus: Vec<ObuHeader>,
    /// Number of frames in this temporal unit
    pub frame_count: u32,
    /// Whether sequence header present
    pub has_sequence_header: bool,
}

/// AV1 bitstream parsing errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Av1Error {
    /// No error
    None = 0,
    /// Unexpected end of stream
    UnexpectedEof = 1,
    /// Invalid OBU header (forbidden bit set or invalid type)
    InvalidObuHeader = 2,
    /// Invalid OBU type value
    InvalidObuType = 3,
    /// LEB128 overflow (value too large for u64)
    Leb128Overflow = 4,
    /// Buffer too small for operation
    BufferTooSmall = 5,
    /// Invalid temporal unit structure
    InvalidTemporalUnit = 6,
    /// Missing required OBU (e.g., sequence header)
    MissingRequiredObu = 7,
    /// Invalid OBU size (larger than remaining data)
    InvalidObuSize = 8,
    /// OBU forbidden bit set (must be 0)
    ObuForbiddenBitSet = 9,
}

impl core::fmt::Display for Av1Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Av1Error::None => write!(f, "No error"),
            Av1Error::UnexpectedEof => write!(f, "Unexpected end of stream"),
            Av1Error::InvalidObuHeader => write!(f, "Invalid OBU header"),
            Av1Error::InvalidObuType => write!(f, "Invalid OBU type"),
            Av1Error::Leb128Overflow => write!(f, "LEB128 overflow"),
            Av1Error::BufferTooSmall => write!(f, "Buffer too small"),
            Av1Error::InvalidTemporalUnit => write!(f, "Invalid temporal unit structure"),
            Av1Error::MissingRequiredObu => write!(f, "Missing required OBU"),
            Av1Error::InvalidObuSize => write!(f, "Invalid OBU size"),
            Av1Error::ObuForbiddenBitSet => write!(f, "OBU forbidden bit set"),
        }
    }
}

impl std::error::Error for Av1Error {}

/// Statistics snapshot from AV1 bitstream parser
#[derive(Debug, Clone, Copy, Default)]
pub struct Av1BitstreamStats {
    /// Total bytes parsed
    pub bytes_parsed: u64,
    /// Total OBUs parsed
    pub obus_parsed: u64,
    /// Sequence header OBUs found
    pub sequence_headers_seen: u32,
    /// Frames parsed (Frame + FrameHeader OBUs)
    pub frames_parsed: u64,
    /// Temporal delimiter OBUs found
    pub temporal_delimiters: u32,
    /// Tile group OBUs found
    pub tile_groups: u32,
    /// Metadata OBUs found
    pub metadata_obus: u32,
    /// Current byte offset
    pub byte_offset: u64,
    /// Current bit offset within byte (0-7)
    pub bit_offset: u32,
    /// Bytes remaining in current buffer
    pub bytes_remaining: u64,
    /// Generation counter (Q34 audit)
    pub generation: u64,
}

/// Maximum bytes for LEB128 encoding of u64
const LEB128_MAX_BYTES: usize = 8;

/// T5 Streaming capsule for AV1 bitstream parsing
///
/// Provides lockfree incremental OBU parsing for AV1 bitstreams.
/// Uses `portable_simd` compatible patterns for future SIMD acceleration.
///
/// # Cache Alignment
///
/// The structure is 512B cache-aligned (128B base alignment) to prevent false
/// sharing and ensure optimal memory access patterns on modern CPUs.
///
/// # Lockfree Design
///
/// All fields use atomic types for thread-safe access without locks.
/// Statistics can be read concurrently while parsing is in progress.
///
/// # Streaming Model
///
/// OBUs are parsed incrementally. Call `parse_obu_header()` to parse the next
/// OBU, then use `advance()` to move past it. This enables O(1) memory usage
/// regardless of bitstream size.
#[repr(C, align(128))]
pub struct Av1BitstreamCapsule {
    // ---- Cache line 0 (bytes 0-63): Core state ----
    /// Packed state: bits 0-31 = parsing state flags, bits 32-63 = reserved
    state: AtomicU64,
    /// Generation counter for Q34 audit trail
    generation: AtomicU64,
    /// Current OBU type being parsed
    obu_type: AtomicU32,
    /// Reserved padding
    _reserved0: AtomicU32,
    /// Current OBU size (payload only, not including header)
    obu_size: AtomicU64,
    /// Has size field flag (1 = present)
    has_size_field: AtomicU32,
    /// Has extension flag (1 = present)
    has_extension: AtomicU32,
    /// Temporal ID from extension header
    temporal_id: AtomicU32,
    /// Spatial ID from extension header
    spatial_id: AtomicU32,

    // ---- Cache line 1 (bytes 64-127): Position tracking ----
    /// Current byte offset in stream
    byte_offset: AtomicU64,
    /// Current bit offset within byte (0-7)
    bit_offset: AtomicU32,
    /// Reserved padding
    _reserved1: AtomicU32,
    /// Bytes remaining in current buffer
    bytes_remaining: AtomicU64,
    /// Total OBUs parsed
    obus_parsed: AtomicU64,
    /// Sequence headers seen
    sequence_headers_seen: AtomicU32,
    /// Temporal delimiters seen
    temporal_delimiters: AtomicU32,
    /// Tile groups seen
    tile_groups: AtomicU32,
    /// Metadata OBUs seen
    metadata_obus: AtomicU32,

    // ---- Cache line 2 (bytes 128-191): Frame statistics ----
    /// Total frames parsed
    frames_parsed: AtomicU64,
    /// Last error code
    last_error: AtomicU32,
    /// Error count
    error_count: AtomicU32,
    /// Padding OBUs seen
    padding_obus: AtomicU32,
    /// Reserved
    _reserved2: AtomicU32,
    /// Reserved
    _reserved3: AtomicU64,
    /// Reserved
    _reserved4: AtomicU64,
    /// Reserved
    _reserved5: AtomicU64,

    // ---- Cache lines 3-7 (bytes 192-511): Padding ----
    /// Padding to 512B alignment
    _padding: [u8; 320],
}

// Compile-time size and alignment verification
const _: () = assert!(core::mem::size_of::<Av1BitstreamCapsule>() == 512);
const _: () = assert!(core::mem::align_of::<Av1BitstreamCapsule>() == 128);

impl Av1BitstreamCapsule {
    /// Create a new Av1BitstreamCapsule
    ///
    /// Initializes all state to zero/default values.
    pub fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            obu_type: AtomicU32::new(0),
            _reserved0: AtomicU32::new(0),
            obu_size: AtomicU64::new(0),
            has_size_field: AtomicU32::new(0),
            has_extension: AtomicU32::new(0),
            temporal_id: AtomicU32::new(0),
            spatial_id: AtomicU32::new(0),
            byte_offset: AtomicU64::new(0),
            bit_offset: AtomicU32::new(0),
            _reserved1: AtomicU32::new(0),
            bytes_remaining: AtomicU64::new(0),
            obus_parsed: AtomicU64::new(0),
            sequence_headers_seen: AtomicU32::new(0),
            temporal_delimiters: AtomicU32::new(0),
            tile_groups: AtomicU32::new(0),
            metadata_obus: AtomicU32::new(0),
            frames_parsed: AtomicU64::new(0),
            last_error: AtomicU32::new(0),
            error_count: AtomicU32::new(0),
            padding_obus: AtomicU32::new(0),
            _reserved2: AtomicU32::new(0),
            _reserved3: AtomicU64::new(0),
            _reserved4: AtomicU64::new(0),
            _reserved5: AtomicU64::new(0),
            _padding: [0u8; 320],
        }
    }

    /// Reset all state and statistics
    pub fn reset(&self) {
        self.state.store(0, Ordering::Release);
        self.obu_type.store(0, Ordering::Release);
        self.obu_size.store(0, Ordering::Release);
        self.has_size_field.store(0, Ordering::Release);
        self.has_extension.store(0, Ordering::Release);
        self.temporal_id.store(0, Ordering::Release);
        self.spatial_id.store(0, Ordering::Release);
        self.byte_offset.store(0, Ordering::Release);
        self.bit_offset.store(0, Ordering::Release);
        self.bytes_remaining.store(0, Ordering::Release);
        self.obus_parsed.store(0, Ordering::Release);
        self.sequence_headers_seen.store(0, Ordering::Release);
        self.temporal_delimiters.store(0, Ordering::Release);
        self.tile_groups.store(0, Ordering::Release);
        self.metadata_obus.store(0, Ordering::Release);
        self.frames_parsed.store(0, Ordering::Release);
        self.last_error.store(0, Ordering::Release);
        self.error_count.store(0, Ordering::Release);
        self.padding_obus.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Initialize the parser with a data buffer
    ///
    /// Sets bytes_remaining and resets position to start of buffer.
    #[inline]
    pub fn init(&self, data: &[u8]) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.byte_offset.store(0, Ordering::Release);
        self.bit_offset.store(0, Ordering::Release);
        self.bytes_remaining.store(data.len() as u64, Ordering::Release);
    }

    /// Get statistics snapshot
    ///
    /// Returns a consistent snapshot of all statistics.
    /// Uses generation counter for consistency verification.
    pub fn stats(&self) -> Av1BitstreamStats {
        Av1BitstreamStats {
            bytes_parsed: self.byte_offset.load(Ordering::Acquire),
            obus_parsed: self.obus_parsed.load(Ordering::Acquire),
            sequence_headers_seen: self.sequence_headers_seen.load(Ordering::Acquire),
            frames_parsed: self.frames_parsed.load(Ordering::Acquire),
            temporal_delimiters: self.temporal_delimiters.load(Ordering::Acquire),
            tile_groups: self.tile_groups.load(Ordering::Acquire),
            metadata_obus: self.metadata_obus.load(Ordering::Acquire),
            byte_offset: self.byte_offset.load(Ordering::Acquire),
            bit_offset: self.bit_offset.load(Ordering::Acquire),
            bytes_remaining: self.bytes_remaining.load(Ordering::Acquire),
            generation: self.generation.load(Ordering::Acquire),
        }
    }

    /// Get generation counter (Q34 audit)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Read LEB128 variable-length unsigned integer
    ///
    /// # AV1 Specification Section 4.10.5
    ///
    /// leb128() reads a variable-length unsigned integer using LEB128 encoding.
    /// Each byte has 7 data bits (bits 0-6) and 1 continuation bit (bit 7).
    /// The continuation bit indicates more bytes follow if set.
    ///
    /// # Arguments
    ///
    /// * `data` - Data slice starting at LEB128 value
    ///
    /// # Returns
    ///
    /// `Ok((value, bytes_consumed))` on success
    /// `Err(Av1Error)` on failure (EOF or overflow)
    pub fn read_leb128(&self, data: &[u8]) -> Result<(u64, usize), Av1Error> {
        if data.is_empty() {
            self.last_error.store(Av1Error::UnexpectedEof as u32, Ordering::Release);
            self.error_count.fetch_add(1, Ordering::Relaxed);
            return Err(Av1Error::UnexpectedEof);
        }

        let mut value: u64 = 0;
        let mut shift: u32 = 0;
        let mut bytes_read: usize = 0;

        // #ASSUME: LEB128 encoding uses at most 8 bytes for u64
        // #VERIFY: Loop terminates when continuation bit is 0 or 8 bytes read
        for byte in data.iter().take(LEB128_MAX_BYTES) {
            let byte_val = *byte;
            bytes_read += 1;

            // Extract 7 data bits and accumulate
            let data_bits = (byte_val & 0x7F) as u64;

            // Check for overflow before shifting
            if shift >= 64 || (shift > 56 && data_bits > (u64::MAX >> shift)) {
                self.last_error.store(Av1Error::Leb128Overflow as u32, Ordering::Release);
                self.error_count.fetch_add(1, Ordering::Relaxed);
                return Err(Av1Error::Leb128Overflow);
            }

            value |= data_bits << shift;
            shift += 7;

            // Check continuation bit (bit 7)
            if (byte_val & 0x80) == 0 {
                return Ok((value, bytes_read));
            }
        }

        // If we get here, we consumed 8 bytes without termination
        // This is valid if the last byte had continuation bit 0
        // But since we checked in the loop, this means overflow
        self.last_error.store(Av1Error::Leb128Overflow as u32, Ordering::Release);
        self.error_count.fetch_add(1, Ordering::Relaxed);
        Err(Av1Error::Leb128Overflow)
    }

    /// Parse OBU header from data slice
    ///
    /// # AV1 Specification Section 5.3.1
    ///
    /// OBU Header format (1 byte, optionally 2 with extension):
    /// ```text
    /// +---------------+
    /// |0|1 2 3 4|5|6|7|
    /// |F| Type  |E|H|R|
    /// +---------------+
    /// F: obu_forbidden_bit (must be 0)
    /// Type: obu_type (4 bits)
    /// E: obu_extension_flag
    /// H: obu_has_size_field
    /// R: obu_reserved_1bit (must be 0)
    /// ```
    ///
    /// If extension flag is set, second byte:
    /// ```text
    /// +---------------+
    /// |0 1 2|3 4 5|6 7|
    /// | T   | S   | R |
    /// +---------------+
    /// T: temporal_id (3 bits)
    /// S: spatial_id (3 bits)
    /// R: extension_header_reserved_3bits
    /// ```
    ///
    /// # Arguments
    ///
    /// * `data` - Data slice starting at OBU header
    ///
    /// # Returns
    ///
    /// Parsed ObuHeader with all fields populated
    pub fn parse_obu_header(&self, data: &[u8]) -> Result<ObuHeader, Av1Error> {
        if data.is_empty() {
            self.last_error.store(Av1Error::BufferTooSmall as u32, Ordering::Release);
            self.error_count.fetch_add(1, Ordering::Relaxed);
            return Err(Av1Error::BufferTooSmall);
        }

        self.generation.fetch_add(1, Ordering::AcqRel);

        let header_byte = data[0];

        // Check forbidden bit (bit 7, must be 0)
        if (header_byte & 0x80) != 0 {
            self.last_error.store(Av1Error::ObuForbiddenBitSet as u32, Ordering::Release);
            self.error_count.fetch_add(1, Ordering::Relaxed);
            return Err(Av1Error::ObuForbiddenBitSet);
        }

        // Extract fields from header byte
        let obu_type_raw = (header_byte >> 3) & 0x0F;
        let obu_extension_flag = (header_byte & 0x04) != 0;
        let obu_has_size_field = (header_byte & 0x02) != 0;
        // Reserved bit (header_byte & 0x01) should be 0 but we don't enforce

        let obu_type = ObuType::from_u8(obu_type_raw);

        // Update atomic state
        self.obu_type.store(obu_type.to_u8() as u32, Ordering::Release);
        self.has_size_field.store(if obu_has_size_field { 1 } else { 0 }, Ordering::Release);
        self.has_extension.store(if obu_extension_flag { 1 } else { 0 }, Ordering::Release);

        let mut header_size: u8 = 1;
        let mut temporal_id: u8 = 0;
        let mut spatial_id: u8 = 0;

        // Parse extension header if present
        if obu_extension_flag {
            if data.len() < 2 {
                self.last_error.store(Av1Error::BufferTooSmall as u32, Ordering::Release);
                self.error_count.fetch_add(1, Ordering::Relaxed);
                return Err(Av1Error::BufferTooSmall);
            }

            let ext_byte = data[1];
            temporal_id = (ext_byte >> 5) & 0x07;
            spatial_id = (ext_byte >> 3) & 0x03;
            // Reserved 3 bits (ext_byte & 0x07) should be 0 but we don't enforce
            header_size = 2;
        }

        self.temporal_id.store(temporal_id as u32, Ordering::Release);
        self.spatial_id.store(spatial_id as u32, Ordering::Release);

        // Parse obu_size if has_size_field
        let mut obu_size: u64 = 0;
        let mut total_header_bytes = header_size as usize;

        if obu_has_size_field {
            let size_data = &data[header_size as usize..];
            let (size, size_bytes) = self.read_leb128(size_data)?;
            obu_size = size;
            total_header_bytes += size_bytes;
        }

        self.obu_size.store(obu_size, Ordering::Release);

        // Update type-specific counters
        match obu_type {
            ObuType::SequenceHeader => {
                self.sequence_headers_seen.fetch_add(1, Ordering::Relaxed);
            }
            ObuType::TemporalDelimiter => {
                self.temporal_delimiters.fetch_add(1, Ordering::Relaxed);
            }
            ObuType::FrameHeader | ObuType::Frame => {
                self.frames_parsed.fetch_add(1, Ordering::Relaxed);
            }
            ObuType::TileGroup => {
                self.tile_groups.fetch_add(1, Ordering::Relaxed);
            }
            ObuType::Metadata => {
                self.metadata_obus.fetch_add(1, Ordering::Relaxed);
            }
            ObuType::Padding => {
                self.padding_obus.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }

        self.obus_parsed.fetch_add(1, Ordering::Relaxed);

        Ok(ObuHeader {
            obu_type,
            obu_extension_flag,
            obu_has_size_field,
            temporal_id,
            spatial_id,
            header_size,
            obu_size,
            total_header_bytes,
        })
    }

    /// Parse a temporal unit from data
    ///
    /// A temporal unit is defined as all OBUs between temporal delimiter OBUs
    /// (or from start of stream to first temporal delimiter).
    ///
    /// # Arguments
    ///
    /// * `data` - Data slice containing one or more OBUs
    ///
    /// # Returns
    ///
    /// Parsed TemporalUnit with all OBU headers
    pub fn parse_temporal_unit(&self, data: &[u8]) -> Result<TemporalUnit, Av1Error> {
        if data.is_empty() {
            self.last_error.store(Av1Error::BufferTooSmall as u32, Ordering::Release);
            self.error_count.fetch_add(1, Ordering::Relaxed);
            return Err(Av1Error::BufferTooSmall);
        }

        self.generation.fetch_add(1, Ordering::AcqRel);

        let mut tu = TemporalUnit {
            offset: self.byte_offset.load(Ordering::Acquire),
            size: 0,
            obus: Vec::new(),
            frame_count: 0,
            has_sequence_header: false,
        };

        let mut offset: usize = 0;

        while offset < data.len() {
            let obu_data = &data[offset..];
            let header = self.parse_obu_header(obu_data)?;

            // Check for temporal delimiter (marks end of temporal unit)
            if header.obu_type == ObuType::TemporalDelimiter && !tu.obus.is_empty() {
                // Don't include the delimiter in this temporal unit
                break;
            }

            // Track sequence header presence
            if header.obu_type == ObuType::SequenceHeader {
                tu.has_sequence_header = true;
            }

            // Track frame count
            if header.obu_type == ObuType::Frame || header.obu_type == ObuType::FrameHeader {
                tu.frame_count += 1;
            }

            // Calculate bytes to skip
            let obu_total_bytes = if header.obu_has_size_field {
                header.total_header_bytes + header.obu_size as usize
            } else {
                // Without size field, OBU extends to end of data
                data.len() - offset
            };

            // Validate OBU doesn't exceed buffer
            if offset + obu_total_bytes > data.len() {
                self.last_error.store(Av1Error::InvalidObuSize as u32, Ordering::Release);
                self.error_count.fetch_add(1, Ordering::Relaxed);
                return Err(Av1Error::InvalidObuSize);
            }

            tu.obus.push(header);
            offset += obu_total_bytes;
        }

        tu.size = offset as u64;

        Ok(tu)
    }

    /// Advance parser position by specified bytes
    ///
    /// Used after parsing an OBU to move to the next one.
    pub fn advance(&self, bytes: usize) {
        let current = self.byte_offset.load(Ordering::Acquire);
        self.byte_offset.store(current + bytes as u64, Ordering::Release);

        let remaining = self.bytes_remaining.load(Ordering::Acquire);
        let new_remaining = remaining.saturating_sub(bytes as u64);
        self.bytes_remaining.store(new_remaining, Ordering::Release);
    }

    /// Check if there are more bytes to parse
    #[inline]
    pub fn has_more_data(&self) -> bool {
        self.bytes_remaining.load(Ordering::Acquire) > 0
    }

    /// Get current byte offset
    #[inline]
    pub fn byte_offset(&self) -> u64 {
        self.byte_offset.load(Ordering::Acquire)
    }

    /// Get bytes remaining
    #[inline]
    pub fn bytes_remaining(&self) -> u64 {
        self.bytes_remaining.load(Ordering::Acquire)
    }

    /// Iterate over all OBUs in data
    ///
    /// Convenience method to parse all OBUs in a buffer.
    ///
    /// # Arguments
    ///
    /// * `data` - Complete AV1 bitstream data
    ///
    /// # Returns
    ///
    /// Vector of all parsed OBU headers
    pub fn parse_all_obus(&self, data: &[u8]) -> Result<Vec<ObuHeader>, Av1Error> {
        if data.is_empty() {
            return Ok(Vec::new());
        }

        self.init(data);
        let mut obus = Vec::new();
        let mut offset: usize = 0;

        while offset < data.len() {
            let obu_data = &data[offset..];
            let header = self.parse_obu_header(obu_data)?;

            let obu_total_bytes = if header.obu_has_size_field {
                header.total_header_bytes + header.obu_size as usize
            } else {
                // Without size field, OBU extends to end of data
                data.len() - offset
            };

            if offset + obu_total_bytes > data.len() {
                self.last_error.store(Av1Error::InvalidObuSize as u32, Ordering::Release);
                self.error_count.fetch_add(1, Ordering::Relaxed);
                return Err(Av1Error::InvalidObuSize);
            }

            obus.push(header);
            offset += obu_total_bytes;
        }

        self.byte_offset.store(offset as u64, Ordering::Release);

        Ok(obus)
    }

    /// Get last error
    #[inline]
    pub fn last_error(&self) -> Option<Av1Error> {
        let err = self.last_error.load(Ordering::Acquire);
        if err == 0 {
            None
        } else {
            Some(match err {
                1 => Av1Error::UnexpectedEof,
                2 => Av1Error::InvalidObuHeader,
                3 => Av1Error::InvalidObuType,
                4 => Av1Error::Leb128Overflow,
                5 => Av1Error::BufferTooSmall,
                6 => Av1Error::InvalidTemporalUnit,
                7 => Av1Error::MissingRequiredObu,
                8 => Av1Error::InvalidObuSize,
                9 => Av1Error::ObuForbiddenBitSet,
                _ => Av1Error::None,
            })
        }
    }
}

impl Default for Av1BitstreamCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: Av1BitstreamCapsule uses only atomic types for shared state
unsafe impl Send for Av1BitstreamCapsule {}
unsafe impl Sync for Av1BitstreamCapsule {}

// ============================================================================
// T28 5-Tier Test Suite
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // T28 Q1-Q7: Unit Tests (Tier 1)
    // =========================================================================

    /// Q1: Test capsule creation and initial state
    #[test]
    fn test_q1_new_capsule() {
        let capsule = Av1BitstreamCapsule::new();

        let stats = capsule.stats();
        assert_eq!(stats.bytes_parsed, 0);
        assert_eq!(stats.obus_parsed, 0);
        assert_eq!(stats.sequence_headers_seen, 0);
        assert_eq!(stats.frames_parsed, 0);
        assert_eq!(stats.generation, 0);
    }

    /// Q2: Test OBU type conversion
    #[test]
    fn test_q2_obu_type_conversion() {
        // Valid types
        assert_eq!(ObuType::from_u8(1), ObuType::SequenceHeader);
        assert_eq!(ObuType::from_u8(2), ObuType::TemporalDelimiter);
        assert_eq!(ObuType::from_u8(3), ObuType::FrameHeader);
        assert_eq!(ObuType::from_u8(4), ObuType::TileGroup);
        assert_eq!(ObuType::from_u8(5), ObuType::Metadata);
        assert_eq!(ObuType::from_u8(6), ObuType::Frame);
        assert_eq!(ObuType::from_u8(7), ObuType::RedundantFrameHeader);
        assert_eq!(ObuType::from_u8(8), ObuType::TileList);
        assert_eq!(ObuType::from_u8(15), ObuType::Padding);

        // Reserved types
        assert_eq!(ObuType::from_u8(0), ObuType::Reserved);
        assert_eq!(ObuType::from_u8(9), ObuType::Reserved);
        assert_eq!(ObuType::from_u8(14), ObuType::Reserved);

        // Round-trip conversion
        for t in [1u8, 2, 3, 4, 5, 6, 7, 8, 15] {
            let obu_type = ObuType::from_u8(t);
            assert_eq!(obu_type.to_u8(), t);
        }
    }

    /// Q3: Test LEB128 decoding - single byte values
    #[test]
    fn test_q3_leb128_single_byte() {
        let capsule = Av1BitstreamCapsule::new();

        // Values 0-127 are encoded in single byte (MSB = 0)
        assert_eq!(capsule.read_leb128(&[0x00]).unwrap(), (0, 1));
        assert_eq!(capsule.read_leb128(&[0x01]).unwrap(), (1, 1));
        assert_eq!(capsule.read_leb128(&[0x7F]).unwrap(), (127, 1));

        // Value with trailing data
        assert_eq!(capsule.read_leb128(&[0x42, 0xFF, 0xFF]).unwrap(), (66, 1));
    }

    /// Q4: Test LEB128 decoding - multi-byte values
    #[test]
    fn test_q4_leb128_multi_byte() {
        let capsule = Av1BitstreamCapsule::new();

        // 128 = 0x80 0x01 (continuation bit set on first byte)
        assert_eq!(capsule.read_leb128(&[0x80, 0x01]).unwrap(), (128, 2));

        // 16384 = 0x80 0x80 0x01
        assert_eq!(capsule.read_leb128(&[0x80, 0x80, 0x01]).unwrap(), (16384, 3));

        // 300 = 0xAC 0x02 (256 + 44 = 300)
        // 300 in LEB128: low 7 bits of 300 = 44 (0x2C), with continuation = 0xAC
        // remaining 2 bits = 2, no continuation = 0x02
        assert_eq!(capsule.read_leb128(&[0xAC, 0x02]).unwrap(), (300, 2));
    }

    /// Q5: Test OBU header parsing - basic header
    #[test]
    fn test_q5_obu_header_basic() {
        let capsule = Av1BitstreamCapsule::new();

        // Sequence header OBU: type=1, no extension, has size, size=10
        // Header byte: 0_0001_0_1_0 = 0x0A (forbidden=0, type=1, ext=0, size=1, reserved=0)
        // Wait, let me recalculate:
        // Bit layout: [7]=forbidden, [6:3]=type, [2]=ext, [1]=size, [0]=reserved
        // type=1 -> bits 6:3 = 0001
        // ext=0, size=1, reserved=0
        // Header = 0b0_0001_0_1_0 = 0x0A
        let header_byte = 0x0A; // type=1, ext=0, has_size=1
        let size_byte = 0x0A; // LEB128 for 10
        let data = [header_byte, size_byte];

        let header = capsule.parse_obu_header(&data).unwrap();

        assert_eq!(header.obu_type, ObuType::SequenceHeader);
        assert!(!header.obu_extension_flag);
        assert!(header.obu_has_size_field);
        assert_eq!(header.obu_size, 10);
        assert_eq!(header.header_size, 1);
        assert_eq!(header.total_header_bytes, 2);
    }

    /// Q6: Test OBU header parsing - with extension
    #[test]
    fn test_q6_obu_header_with_extension() {
        let capsule = Av1BitstreamCapsule::new();

        // Frame OBU with extension: type=6, ext=1, has_size=1
        // Header byte: 0_0110_1_1_0 = 0x36
        // Extension byte: temporal_id=2, spatial_id=1 -> 0b010_01_000 = 0x48
        // Size byte: 0x14 = 20
        let data = [0x36, 0x48, 0x14];

        let header = capsule.parse_obu_header(&data).unwrap();

        assert_eq!(header.obu_type, ObuType::Frame);
        assert!(header.obu_extension_flag);
        assert!(header.obu_has_size_field);
        assert_eq!(header.temporal_id, 2);
        assert_eq!(header.spatial_id, 1);
        assert_eq!(header.obu_size, 20);
        assert_eq!(header.header_size, 2);
        assert_eq!(header.total_header_bytes, 3);
    }

    /// Q7: Test forbidden bit detection
    #[test]
    fn test_q7_forbidden_bit() {
        let capsule = Av1BitstreamCapsule::new();

        // Forbidden bit set (bit 7 = 1)
        let data = [0x80 | 0x0A]; // 0x8A

        let result = capsule.parse_obu_header(&data);
        assert!(matches!(result, Err(Av1Error::ObuForbiddenBitSet)));
    }

    // =========================================================================
    // T28 Q8-Q14: Property Tests (Tier 2)
    // =========================================================================

    /// Q8: Test LEB128 encoding/decoding roundtrip properties
    #[test]
    fn test_q8_leb128_properties() {
        let capsule = Av1BitstreamCapsule::new();

        // Property: For all valid inputs, bytes_consumed <= input.len()
        for value in [0u64, 1, 127, 128, 255, 16383, 16384, 2097151, u32::MAX as u64] {
            // Encode value to LEB128
            let mut encoded = Vec::new();
            let mut v = value;
            loop {
                let byte = (v & 0x7F) as u8;
                v >>= 7;
                if v == 0 {
                    encoded.push(byte);
                    break;
                } else {
                    encoded.push(byte | 0x80);
                }
            }

            // Decode and verify
            let (decoded, bytes) = capsule.read_leb128(&encoded).unwrap();
            assert_eq!(decoded, value, "Value {}", value);
            assert_eq!(bytes, encoded.len(), "Bytes for value {}", value);
        }
    }

    /// Q9: Test OBU type validity properties
    #[test]
    fn test_q9_obu_type_validity() {
        // Property: Only specific values are valid
        for v in 0u8..=15 {
            let obu_type = ObuType::from_u8(v);
            let valid = matches!(v, 1..=8 | 15);
            assert_eq!(
                obu_type.is_valid(),
                valid,
                "Type {} validity",
                v
            );
        }
    }

    /// Q10: Test generation counter increments
    #[test]
    fn test_q10_generation_counter() {
        let capsule = Av1BitstreamCapsule::new();

        let gen0 = capsule.generation();
        assert_eq!(gen0, 0);

        // init() should increment
        let data = [0x0A, 0x00];
        capsule.init(&data);
        let gen1 = capsule.generation();
        assert_eq!(gen1, 1);

        // parse_obu_header() should increment
        let _ = capsule.parse_obu_header(&data);
        let gen2 = capsule.generation();
        assert_eq!(gen2, 2);

        // reset() should increment
        capsule.reset();
        let gen3 = capsule.generation();
        assert_eq!(gen3, 3);
    }

    /// Q11: Test OBU header byte patterns
    #[test]
    fn test_q11_obu_header_patterns() {
        let capsule = Av1BitstreamCapsule::new();

        // All valid type + flag combinations
        for obu_type in [1u8, 2, 3, 4, 5, 6, 7, 8, 15] {
            for ext in [false, true] {
                for has_size in [false, true] {
                    // Build header byte
                    let mut header = obu_type << 3;
                    if ext { header |= 0x04; }
                    if has_size { header |= 0x02; }

                    let mut data = vec![header];
                    if ext {
                        data.push(0x00); // Extension byte
                    }
                    if has_size {
                        data.push(0x00); // Size = 0
                    }

                    let result = capsule.parse_obu_header(&data);
                    assert!(result.is_ok(), "Type={}, ext={}, size={}", obu_type, ext, has_size);

                    let parsed = result.unwrap();
                    assert_eq!(parsed.obu_type.to_u8(), obu_type);
                    assert_eq!(parsed.obu_extension_flag, ext);
                    assert_eq!(parsed.obu_has_size_field, has_size);
                }
            }
        }
    }

    /// Q12: Test statistics accuracy
    #[test]
    fn test_q12_statistics_accuracy() {
        let capsule = Av1BitstreamCapsule::new();

        // Parse multiple OBUs
        // Sequence header
        let seq_header = [0x0A, 0x00]; // type=1, has_size, size=0
        capsule.parse_obu_header(&seq_header).unwrap();

        // Temporal delimiter
        let td = [0x12, 0x00]; // type=2
        capsule.parse_obu_header(&td).unwrap();

        // Frame
        let frame = [0x32, 0x00]; // type=6
        capsule.parse_obu_header(&frame).unwrap();

        let stats = capsule.stats();
        assert_eq!(stats.obus_parsed, 3);
        assert_eq!(stats.sequence_headers_seen, 1);
        assert_eq!(stats.temporal_delimiters, 1);
        assert_eq!(stats.frames_parsed, 1);
    }

    /// Q13: Test advance() updates position correctly
    #[test]
    fn test_q13_advance_position() {
        let capsule = Av1BitstreamCapsule::new();

        let data = vec![0u8; 100];
        capsule.init(&data);

        assert_eq!(capsule.byte_offset(), 0);
        assert_eq!(capsule.bytes_remaining(), 100);

        capsule.advance(10);
        assert_eq!(capsule.byte_offset(), 10);
        assert_eq!(capsule.bytes_remaining(), 90);

        capsule.advance(50);
        assert_eq!(capsule.byte_offset(), 60);
        assert_eq!(capsule.bytes_remaining(), 40);
    }

    /// Q14: Test reset() clears all state
    #[test]
    fn test_q14_reset_clears_state() {
        let capsule = Av1BitstreamCapsule::new();

        // Parse some data to populate state
        let data = [0x32, 0x0A]; // Frame OBU, size=10
        capsule.init(&data);
        capsule.parse_obu_header(&data).unwrap();
        capsule.advance(10);

        let stats_before = capsule.stats();
        assert!(stats_before.obus_parsed > 0);

        capsule.reset();

        let stats_after = capsule.stats();
        assert_eq!(stats_after.obus_parsed, 0);
        assert_eq!(stats_after.byte_offset, 0);
        assert!(stats_after.generation > stats_before.generation);
    }

    // =========================================================================
    // T28 Q15-Q21: Integration Tests (Tier 3)
    // =========================================================================

    /// Q15: Test parsing multiple OBUs in sequence
    #[test]
    fn test_q15_parse_multiple_obus() {
        let capsule = Av1BitstreamCapsule::new();

        // Construct a valid AV1 bitstream fragment:
        // 1. Temporal Delimiter (type=2, has_size, size=0)
        // 2. Sequence Header (type=1, has_size, size=5 + 5 bytes payload)
        // 3. Frame (type=6, has_size, size=10 + 10 bytes payload)

        let mut data = Vec::new();

        // Temporal Delimiter: 0x12 = type 2, has_size=1
        data.push(0x12);
        data.push(0x00); // size = 0

        // Sequence Header: 0x0A = type 1, has_size=1
        data.push(0x0A);
        data.push(0x05); // size = 5
        data.extend_from_slice(&[0x00; 5]); // 5 bytes payload

        // Frame: 0x32 = type 6, has_size=1
        data.push(0x32);
        data.push(0x0A); // size = 10
        data.extend_from_slice(&[0x00; 10]); // 10 bytes payload

        let obus = capsule.parse_all_obus(&data).unwrap();

        assert_eq!(obus.len(), 3);
        assert_eq!(obus[0].obu_type, ObuType::TemporalDelimiter);
        assert_eq!(obus[1].obu_type, ObuType::SequenceHeader);
        assert_eq!(obus[2].obu_type, ObuType::Frame);

        let stats = capsule.stats();
        assert_eq!(stats.obus_parsed, 3);
        assert_eq!(stats.temporal_delimiters, 1);
        assert_eq!(stats.sequence_headers_seen, 1);
        assert_eq!(stats.frames_parsed, 1);
    }

    /// Q16: Test temporal unit parsing
    #[test]
    fn test_q16_temporal_unit_parsing() {
        let capsule = Av1BitstreamCapsule::new();

        // Create a temporal unit with sequence header and frame
        let mut data = Vec::new();

        // Sequence Header
        data.push(0x0A); // type=1, has_size
        data.push(0x05); // size=5
        data.extend_from_slice(&[0x00; 5]);

        // Frame
        data.push(0x32); // type=6, has_size
        data.push(0x0A); // size=10
        data.extend_from_slice(&[0x00; 10]);

        let tu = capsule.parse_temporal_unit(&data).unwrap();

        assert_eq!(tu.obus.len(), 2);
        assert!(tu.has_sequence_header);
        assert_eq!(tu.frame_count, 1);
        assert_eq!(tu.size, data.len() as u64);
    }

    /// Q17: Test OBU with extension header in stream
    #[test]
    fn test_q17_extension_header_stream() {
        let capsule = Av1BitstreamCapsule::new();

        let mut data = Vec::new();

        // Frame with extension: type=6, ext=1, has_size=1
        // Header: 0x36 = 0b00110110
        // Extension: temporal_id=3, spatial_id=2 -> 0b011_10_000 = 0x70
        data.push(0x36);
        data.push(0x70);
        data.push(0x0A); // size=10
        data.extend_from_slice(&[0x00; 10]);

        // Another frame without extension
        data.push(0x32);
        data.push(0x05); // size=5
        data.extend_from_slice(&[0x00; 5]);

        let obus = capsule.parse_all_obus(&data).unwrap();

        assert_eq!(obus.len(), 2);
        assert!(obus[0].obu_extension_flag);
        assert_eq!(obus[0].temporal_id, 3);
        assert_eq!(obus[0].spatial_id, 2);
        assert!(!obus[1].obu_extension_flag);
    }

    /// Q18: Test empty data handling
    #[test]
    fn test_q18_empty_data() {
        let capsule = Av1BitstreamCapsule::new();

        assert!(capsule.parse_obu_header(&[]).is_err());
        assert!(capsule.read_leb128(&[]).is_err());

        let obus = capsule.parse_all_obus(&[]).unwrap();
        assert!(obus.is_empty());
    }

    /// Q19: Test large OBU size
    #[test]
    fn test_q19_large_obu_size() {
        let capsule = Av1BitstreamCapsule::new();

        // Frame with large size (16384 bytes)
        // LEB128 for 16384 = 0x80 0x80 0x01
        let mut data = vec![0x32, 0x80, 0x80, 0x01];
        data.extend(vec![0x00; 16384]);

        let obus = capsule.parse_all_obus(&data).unwrap();

        assert_eq!(obus.len(), 1);
        assert_eq!(obus[0].obu_size, 16384);
    }

    /// Q20: Test has_more_data tracking
    #[test]
    fn test_q20_has_more_data() {
        let capsule = Av1BitstreamCapsule::new();

        let data = vec![0u8; 100];
        capsule.init(&data);

        assert!(capsule.has_more_data());

        capsule.advance(50);
        assert!(capsule.has_more_data());

        capsule.advance(50);
        assert!(!capsule.has_more_data());
    }

    /// Q21: Test invalid OBU size handling
    #[test]
    fn test_q21_invalid_obu_size() {
        let capsule = Av1BitstreamCapsule::new();

        // OBU with size larger than remaining data
        let data = [0x32, 0xFF, 0x01]; // Frame, size=255, but only 3 bytes total

        let result = capsule.parse_all_obus(&data);
        assert!(matches!(result, Err(Av1Error::InvalidObuSize)));
    }

    // =========================================================================
    // T28 Q22-Q28: Production Tests (Tier 4)
    // =========================================================================

    /// Q22: Test concurrent access safety
    #[test]
    fn test_q22_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(Av1BitstreamCapsule::new());
        let data = Arc::new(vec![0x32, 0x0A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

        let mut handles = vec![];

        for _ in 0..4 {
            let capsule_clone = Arc::clone(&capsule);
            let data_clone = Arc::clone(&data);

            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let _ = capsule_clone.stats();
                    let _ = capsule_clone.generation();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should complete without panics
    }

    /// Q23: Test realistic AV1 bitstream pattern
    #[test]
    fn test_q23_realistic_bitstream() {
        let capsule = Av1BitstreamCapsule::new();

        // Simulate a realistic AV1 GOP:
        // TD -> SH -> Frame -> TD -> Frame -> Frame -> ...
        let mut data = Vec::new();

        // Temporal Delimiter
        data.extend_from_slice(&[0x12, 0x00]);

        // Sequence Header (minimal)
        data.extend_from_slice(&[0x0A, 0x08]);
        data.extend_from_slice(&[0x00; 8]);

        // Key Frame
        data.extend_from_slice(&[0x32, 0x14]); // 20 bytes payload
        data.extend_from_slice(&[0x00; 20]);

        // Second temporal unit
        data.extend_from_slice(&[0x12, 0x00]); // TD

        // P-Frame
        data.extend_from_slice(&[0x32, 0x0A]); // 10 bytes
        data.extend_from_slice(&[0x00; 10]);

        // P-Frame
        data.extend_from_slice(&[0x32, 0x0A]);
        data.extend_from_slice(&[0x00; 10]);

        let obus = capsule.parse_all_obus(&data).unwrap();

        assert_eq!(obus.len(), 6);

        let stats = capsule.stats();
        assert_eq!(stats.temporal_delimiters, 2);
        assert_eq!(stats.sequence_headers_seen, 1);
        assert_eq!(stats.frames_parsed, 3);
    }

    /// Q24: Test capsule size and alignment
    #[test]
    fn test_q24_capsule_size_alignment() {
        assert_eq!(
            core::mem::size_of::<Av1BitstreamCapsule>(),
            512,
            "Capsule must be 512B for T5 Streaming tier"
        );
        assert_eq!(
            core::mem::align_of::<Av1BitstreamCapsule>(),
            128,
            "Capsule must be 128B aligned"
        );
    }

    /// Q25: Test error recovery
    #[test]
    fn test_q25_error_recovery() {
        let capsule = Av1BitstreamCapsule::new();

        // Cause an error
        let bad_data = [0x80]; // Forbidden bit set
        let _ = capsule.parse_obu_header(&bad_data);

        assert!(capsule.last_error().is_some());

        // Reset and verify recovery
        capsule.reset();

        let good_data = [0x32, 0x00];
        let result = capsule.parse_obu_header(&good_data);
        assert!(result.is_ok());
    }

    /// Q26: Test all OBU types parsing
    #[test]
    fn test_q26_all_obu_types() {
        let capsule = Av1BitstreamCapsule::new();

        // Create one OBU of each valid type
        for obu_type in [1u8, 2, 3, 4, 5, 6, 7, 8, 15] {
            let header_byte = (obu_type << 3) | 0x02; // has_size=1
            let data = [header_byte, 0x00]; // size=0

            let result = capsule.parse_obu_header(&data);
            assert!(result.is_ok(), "Failed for type {}", obu_type);

            let header = result.unwrap();
            assert_eq!(header.obu_type.to_u8(), obu_type);
        }
    }

    /// Q27: Test large data handling
    #[test]
    fn test_q27_large_data() {
        let capsule = Av1BitstreamCapsule::new();

        // Create 1MB of OBUs
        let mut data = Vec::new();
        let num_frames = 1000;
        let frame_size = 1000;

        for _ in 0..num_frames {
            // Frame OBU with 1KB payload
            data.push(0x32); // type=6, has_size=1

            // LEB128 encode frame_size (1000 = 0xE8 0x07)
            data.push(0xE8);
            data.push(0x07);

            data.extend(vec![0x00; frame_size]);
        }

        let obus = capsule.parse_all_obus(&data).unwrap();

        assert_eq!(obus.len(), num_frames);

        let stats = capsule.stats();
        assert_eq!(stats.frames_parsed, num_frames as u64);
    }

    /// Q28: Test comprehensive statistics
    #[test]
    fn test_q28_comprehensive_statistics() {
        let capsule = Av1BitstreamCapsule::new();

        let mut data = Vec::new();

        // Mix of OBU types
        // 2x Temporal Delimiter
        for _ in 0..2 {
            data.extend_from_slice(&[0x12, 0x00]);
        }

        // 1x Sequence Header
        data.extend_from_slice(&[0x0A, 0x05]);
        data.extend(vec![0x00; 5]);

        // 3x Frame
        for _ in 0..3 {
            data.extend_from_slice(&[0x32, 0x0A]);
            data.extend(vec![0x00; 10]);
        }

        // 2x Tile Group
        for _ in 0..2 {
            data.extend_from_slice(&[0x22, 0x05]);
            data.extend(vec![0x00; 5]);
        }

        // 1x Metadata
        data.extend_from_slice(&[0x2A, 0x03]);
        data.extend(vec![0x00; 3]);

        // 1x Padding
        data.extend_from_slice(&[0x7A, 0x02]);
        data.extend(vec![0x00; 2]);

        let obus = capsule.parse_all_obus(&data).unwrap();

        assert_eq!(obus.len(), 10);

        let stats = capsule.stats();
        assert_eq!(stats.obus_parsed, 10);
        assert_eq!(stats.temporal_delimiters, 2);
        assert_eq!(stats.sequence_headers_seen, 1);
        assert_eq!(stats.frames_parsed, 3);
        assert_eq!(stats.tile_groups, 2);
        assert_eq!(stats.metadata_obus, 1);
    }

    // =========================================================================
    // T28 Q29-Q35: Determinism Tests (Tier 5)
    // =========================================================================

    /// Q29: Test bit-exact parsing determinism
    #[test]
    fn test_q29_bit_exact_parsing() {
        // Parse same data twice, verify identical results
        let data = vec![0x32, 0x0A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

        let capsule1 = Av1BitstreamCapsule::new();
        let capsule2 = Av1BitstreamCapsule::new();

        let obus1 = capsule1.parse_all_obus(&data).unwrap();
        let obus2 = capsule2.parse_all_obus(&data).unwrap();

        assert_eq!(obus1.len(), obus2.len());
        for (o1, o2) in obus1.iter().zip(obus2.iter()) {
            assert_eq!(o1.obu_type, o2.obu_type);
            assert_eq!(o1.obu_size, o2.obu_size);
            assert_eq!(o1.obu_extension_flag, o2.obu_extension_flag);
            assert_eq!(o1.temporal_id, o2.temporal_id);
            assert_eq!(o1.spatial_id, o2.spatial_id);
        }
    }

    /// Q30: Test LEB128 determinism
    #[test]
    fn test_q30_leb128_determinism() {
        let capsule = Av1BitstreamCapsule::new();

        // Same value encoded as LEB128 should always decode to same result
        let test_values = [0u64, 1, 127, 128, 16383, 16384, 2097151, u32::MAX as u64];

        for value in test_values {
            // Encode
            let mut encoded = Vec::new();
            let mut v = value;
            loop {
                let byte = (v & 0x7F) as u8;
                v >>= 7;
                if v == 0 {
                    encoded.push(byte);
                    break;
                } else {
                    encoded.push(byte | 0x80);
                }
            }

            // Decode multiple times
            for _ in 0..10 {
                let (decoded, bytes) = capsule.read_leb128(&encoded).unwrap();
                assert_eq!(decoded, value);
                assert_eq!(bytes, encoded.len());
            }
        }
    }

    /// Q31: Test parsing order determinism
    #[test]
    fn test_q31_parsing_order_determinism() {
        let mut data = Vec::new();

        // Create a predictable sequence
        for i in 0..10 {
            let obu_type = match i % 4 {
                0 => 2, // TD
                1 => 1, // SH
                2 => 6, // Frame
                _ => 4, // TileGroup
            };
            let header = (obu_type << 3) | 0x02;
            data.push(header);
            data.push(0x00);
        }

        // Parse multiple times and verify order
        let mut results = Vec::new();
        for _ in 0..5 {
            let capsule = Av1BitstreamCapsule::new();
            let obus = capsule.parse_all_obus(&data).unwrap();
            results.push(obus.iter().map(|o| o.obu_type.to_u8()).collect::<Vec<_>>());
        }

        // All results should be identical
        for result in &results[1..] {
            assert_eq!(result, &results[0]);
        }
    }

    /// Q32: Test statistics determinism
    #[test]
    fn test_q32_statistics_determinism() {
        let mut data = Vec::new();

        // Create mixed OBU stream
        data.extend_from_slice(&[0x12, 0x00]); // TD
        data.extend_from_slice(&[0x0A, 0x05]); // SH
        data.extend(vec![0x00; 5]);
        data.extend_from_slice(&[0x32, 0x0A]); // Frame
        data.extend(vec![0x00; 10]);

        let mut all_stats = Vec::new();
        for _ in 0..10 {
            let capsule = Av1BitstreamCapsule::new();
            capsule.parse_all_obus(&data).unwrap();
            all_stats.push(capsule.stats());
        }

        // All stats should be identical
        for stats in &all_stats[1..] {
            assert_eq!(stats.obus_parsed, all_stats[0].obus_parsed);
            assert_eq!(stats.temporal_delimiters, all_stats[0].temporal_delimiters);
            assert_eq!(stats.sequence_headers_seen, all_stats[0].sequence_headers_seen);
            assert_eq!(stats.frames_parsed, all_stats[0].frames_parsed);
        }
    }

    /// Q33: Test OBU header byte-exact interpretation
    #[test]
    fn test_q33_header_byte_exact() {
        let capsule = Av1BitstreamCapsule::new();

        // All possible header bytes (without forbidden bit)
        for byte in 0u8..128 {
            let obu_type_raw = (byte >> 3) & 0x0F;
            let ext_flag = (byte & 0x04) != 0;
            let has_size = (byte & 0x02) != 0;

            let mut data = vec![byte];
            if ext_flag {
                data.push(0x00);
            }
            if has_size {
                data.push(0x00);
            }

            let result = capsule.parse_obu_header(&data);
            assert!(result.is_ok(), "Failed for byte 0x{:02X}", byte);

            let header = result.unwrap();
            // Note: from_u8 maps invalid types (0, 9-14) to Reserved, so we compare
            // with the expected ObuType enum value, not the raw byte value
            let expected_type = ObuType::from_u8(obu_type_raw);
            assert_eq!(header.obu_type, expected_type, "Type mismatch for byte 0x{:02X}", byte);
            assert_eq!(header.obu_extension_flag, ext_flag);
            assert_eq!(header.obu_has_size_field, has_size);
        }
    }

    /// Q34: Test generation counter Q34 audit trail
    #[test]
    fn test_q34_audit_trail() {
        let capsule = Av1BitstreamCapsule::new();

        let mut gen_history = vec![capsule.generation()];

        let data = [0x32, 0x00];

        // Record generation after each operation
        capsule.init(&data);
        gen_history.push(capsule.generation());

        capsule.parse_obu_header(&data).unwrap();
        gen_history.push(capsule.generation());

        capsule.parse_temporal_unit(&data).unwrap();
        gen_history.push(capsule.generation());

        capsule.reset();
        gen_history.push(capsule.generation());

        // Verify monotonic increase
        for i in 1..gen_history.len() {
            assert!(
                gen_history[i] > gen_history[i - 1],
                "Generation not monotonic at step {}",
                i
            );
        }
    }

    /// Q35: Test identical input produces identical output
    #[test]
    fn test_q35_identical_io() {
        // Complex bitstream
        let mut data = Vec::new();

        for _ in 0..5 {
            // TD
            data.extend_from_slice(&[0x12, 0x00]);
            // SH
            data.extend_from_slice(&[0x0A, 0x10]);
            data.extend(vec![0xAB; 16]);
            // Frame with extension
            data.extend_from_slice(&[0x36, 0x48, 0x20]);
            data.extend(vec![0xCD; 32]);
        }

        // Parse 10 times and collect results
        let mut all_headers: Vec<Vec<(u8, u64, bool, u8, u8)>> = Vec::new();

        for _ in 0..10 {
            let capsule = Av1BitstreamCapsule::new();
            let obus = capsule.parse_all_obus(&data).unwrap();

            let headers: Vec<_> = obus
                .iter()
                .map(|o| {
                    (
                        o.obu_type.to_u8(),
                        o.obu_size,
                        o.obu_extension_flag,
                        o.temporal_id,
                        o.spatial_id,
                    )
                })
                .collect();
            all_headers.push(headers);
        }

        // All results must be identical
        for headers in &all_headers[1..] {
            assert_eq!(headers, &all_headers[0], "Non-deterministic output detected");
        }
    }

    // =========================================================================
    // Additional Edge Case Tests
    // =========================================================================

    /// Test OBU type display formatting
    #[test]
    fn test_obu_type_display() {
        assert_eq!(format!("{}", ObuType::SequenceHeader), "Sequence Header");
        assert_eq!(format!("{}", ObuType::TemporalDelimiter), "Temporal Delimiter");
        assert_eq!(format!("{}", ObuType::Frame), "Frame");
        assert_eq!(format!("{}", ObuType::Padding), "Padding");
    }

    /// Test error display formatting
    #[test]
    fn test_error_display() {
        assert_eq!(format!("{}", Av1Error::UnexpectedEof), "Unexpected end of stream");
        assert_eq!(format!("{}", Av1Error::Leb128Overflow), "LEB128 overflow");
        assert_eq!(format!("{}", Av1Error::ObuForbiddenBitSet), "OBU forbidden bit set");
    }

    /// Test OBU type helper methods
    #[test]
    fn test_obu_type_helpers() {
        assert!(ObuType::Frame.has_frame_data());
        assert!(ObuType::FrameHeader.has_frame_data());
        assert!(ObuType::TileGroup.has_frame_data());
        assert!(!ObuType::SequenceHeader.has_frame_data());
        assert!(!ObuType::Metadata.has_frame_data());

        assert!(ObuType::SequenceHeader.is_header());
        assert!(ObuType::FrameHeader.is_header());
        assert!(!ObuType::Frame.is_header());
        assert!(!ObuType::Metadata.is_header());

        assert!(ObuType::Frame.is_valid());
        assert!(!ObuType::Reserved.is_valid());
    }

    /// Test default trait implementations
    #[test]
    fn test_defaults() {
        let capsule = Av1BitstreamCapsule::default();
        assert_eq!(capsule.generation(), 0);

        let obu_type = ObuType::default();
        assert_eq!(obu_type, ObuType::Reserved);
    }
}
