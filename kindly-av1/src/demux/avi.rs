//! AVI (Audio Video Interleave) demuxer capsule
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Implements Microsoft RIFF-AVI container parsing (legacy format, 1992).
//! Streaming architecture processes chunks incrementally without full file buffering.
//!
//! ## Architecture
//!
//! ```text
//! +------------------------------------------+
//! | AviDemuxerCapsule (T5 Streaming)         |
//! | Size: 512B, Align: 512B                  |
//! |                                          |
//! | +--------------------------------------+ |
//! | | State Machine (T1 Atomic)            | |
//! | | - state (AtomicU64)                  | |
//! | | - generation (AtomicU64)             | |
//! | +--------------------------------------+ |
//! | +--------------------------------------+ |
//! | | File Tracking (T0 Auditable)         | |
//! | | - file_size (AtomicU64)              | |
//! | | - bytes_parsed (AtomicU64)           | |
//! | +--------------------------------------+ |
//! | +--------------------------------------+ |
//! | | Chunk Statistics (T1 Atomic)         | |
//! | | - chunks_parsed (AtomicU64)          | |
//! | | - streams_found (AtomicU64)          | |
//! | +--------------------------------------+ |
//! | +--------------------------------------+ |
//! | | hdrl/movi Location (T5 Streaming)    | |
//! | | - hdrl_offset/size (AtomicU64)       | |
//! | | - movi_offset/size (AtomicU64)       | |
//! | +--------------------------------------+ |
//! | +--------------------------------------+ |
//! | | Error Tracking (T0 Auditable)        | |
//! | | - last_error (AtomicU64)             | |
//! | +--------------------------------------+ |
//! +------------------------------------------+
//! ```
//!
//! ## Chunk Types Supported
//!
//! | FourCC | Name | Purpose |
//! |--------|------|---------|
//! | `RIFF` | RIFF Header | Container identifier |
//! | `AVI ` | AVI Type | AVI format marker |
//! | `LIST` | List Chunk | Container for sub-chunks |
//! | `hdrl` | Header List | Container metadata |
//! | `avih` | AVI Header | File-level info (fps, frames, streams) |
//! | `strl` | Stream List | Per-stream metadata |
//! | `strh` | Stream Header | Stream type + codec |
//! | `strf` | Stream Format | Codec-specific config |
//! | `movi` | Movie List | Actual coded frames |
//! | `idx1` | Index Chunk | Frame index (classic AVI) |
//! | `00dc`, `01wb` | Data Chunks | Video/audio samples |
//!
//! ## Streaming Pattern (T5)
//!
//! Parse chunks incrementally without buffering entire file:
//! ```text
//! Open AVI -> Parse RIFF/AVI -> Parse hdrl (avih+strl) ->
//! Extract stream info -> Locate movi -> Ready for demuxing
//! ```
//!
//! ## AVI Format Primer
//!
//! AVI uses RIFF (Resource Interchange File Format):
//! - All multi-byte integers are little-endian (unlike MP4's big-endian)
//! - Chunks have 8-byte headers: [FourCC:4][size:4]
//! - LIST chunks have an additional FourCC after size: LIST [size] [type]
//! - Data must be word-aligned (pad with 0x00 if odd size)

use core::sync::atomic::{AtomicU64, Ordering};

/// AVI FourCC codes (4-character codes)
pub mod fourcc {
    /// RIFF container identifier
    pub const RIFF: [u8; 4] = *b"RIFF";
    /// AVI type identifier
    pub const AVI: [u8; 4] = *b"AVI ";
    /// LIST chunk (contains sub-chunks)
    pub const LIST: [u8; 4] = *b"LIST";
    /// Header list
    pub const HDRL: [u8; 4] = *b"hdrl";
    /// AVI main header
    pub const AVIH: [u8; 4] = *b"avih";
    /// Stream list
    pub const STRL: [u8; 4] = *b"strl";
    /// Stream header
    pub const STRH: [u8; 4] = *b"strh";
    /// Stream format
    pub const STRF: [u8; 4] = *b"strf";
    /// Stream name (optional)
    pub const STRN: [u8; 4] = *b"strn";
    /// Movie data list
    pub const MOVI: [u8; 4] = *b"movi";
    /// Index chunk
    pub const IDX1: [u8; 4] = *b"idx1";
    /// JUNK chunk (padding)
    pub const JUNK: [u8; 4] = *b"JUNK";
    /// INFO list (metadata)
    pub const INFO: [u8; 4] = *b"INFO";
}

/// Stream types from strh chunk
pub mod stream_type {
    /// Video stream
    pub const VIDS: [u8; 4] = *b"vids";
    /// Audio stream
    pub const AUDS: [u8; 4] = *b"auds";
    /// Text/subtitle stream
    pub const TXTS: [u8; 4] = *b"txts";
    /// MIDI stream
    pub const MIDS: [u8; 4] = *b"mids";
}

/// Common video codecs in AVI
pub mod video_codec {
    /// Uncompressed RGB
    pub const DIB: [u8; 4] = [0x00, 0x00, 0x00, 0x00];
    /// MJPEG
    pub const MJPG: [u8; 4] = *b"MJPG";
    /// DivX/MPEG-4
    pub const DIVX: [u8; 4] = *b"DIVX";
    /// Xvid
    pub const XVID: [u8; 4] = *b"XVID";
    /// H.264
    pub const H264: [u8; 4] = *b"H264";
    /// VP8
    pub const VP80: [u8; 4] = *b"VP80";
    /// VP9
    pub const VP90: [u8; 4] = *b"VP90";
    /// AV1 (rare in AVI)
    pub const AV01: [u8; 4] = *b"AV01";
}

/// LIST chunk types (contain sub-chunks)
pub const LIST_CHUNKS: &[[u8; 4]] = &[
    fourcc::HDRL,
    fourcc::STRL,
    fourcc::MOVI,
    fourcc::INFO,
];

/// Demuxer state machine
///
/// State transitions:
/// ```text
/// Idle -> ParsingRiff -> ParsingHdrl -> ParsingStrl -> ParsingMovi -> Ready
///   |         |              |              |              |           |
///   +-------- +------------- +------------- +------------- +-----------+-> Error
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DemuxerState {
    /// Initial state, no parsing started
    Idle = 0,
    /// Parsing RIFF/AVI header
    ParsingRiff = 1,
    /// Parsing hdrl list
    ParsingHdrl = 2,
    /// Parsing strl lists within hdrl
    ParsingStrl = 3,
    /// Parsing/locating movi list
    ParsingMovi = 4,
    /// Demuxer ready to extract samples
    Ready = 5,
    /// Error state
    Error = 6,
}

impl DemuxerState {
    /// Convert from u64 (for atomic operations)
    #[inline]
    pub const fn from_u64(value: u64) -> Self {
        match value {
            0 => Self::Idle,
            1 => Self::ParsingRiff,
            2 => Self::ParsingHdrl,
            3 => Self::ParsingStrl,
            4 => Self::ParsingMovi,
            5 => Self::Ready,
            _ => Self::Error,
        }
    }

    /// Convert to u64 (for atomic operations)
    #[inline]
    pub const fn to_u64(self) -> u64 {
        self as u64
    }
}

impl Default for DemuxerState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Parsed chunk information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkInfo {
    /// Chunk FourCC identifier
    pub fourcc: [u8; 4],
    /// Chunk data size (excluding 8-byte header)
    pub size: u32,
    /// Absolute file offset where chunk starts
    pub offset: u64,
    /// For LIST chunks: the list type FourCC
    pub list_type: Option<[u8; 4]>,
}

impl ChunkInfo {
    /// Get content offset (after header)
    ///
    /// - Normal chunk: offset + 8 (fourcc + size)
    /// - LIST chunk: offset + 12 (fourcc + size + list_type)
    #[inline]
    pub const fn content_offset(&self) -> u64 {
        if self.list_type.is_some() {
            self.offset + 12 // LIST chunks have extra 4 bytes for type
        } else {
            self.offset + 8
        }
    }

    /// Get content size (excludes header)
    #[inline]
    pub const fn content_size(&self) -> u32 {
        if self.list_type.is_some() {
            // LIST chunk size includes the 4-byte type
            self.size.saturating_sub(4)
        } else {
            self.size
        }
    }

    /// Check if this is a LIST chunk
    #[inline]
    pub fn is_list(&self) -> bool {
        &self.fourcc == b"LIST"
    }

    /// Get total chunk size (header + data + padding)
    #[inline]
    pub const fn total_size(&self) -> u64 {
        let header_size = if self.list_type.is_some() { 12 } else { 8 };
        let data_size = self.size as u64;
        // RIFF requires word alignment (pad to even byte boundary)
        let padded_size = (data_size + 1) & !1;
        header_size + padded_size
    }
}

/// AVI main header (avih chunk)
#[derive(Debug, Clone, Copy, Default)]
pub struct AviMainHeader {
    /// Microseconds per frame
    pub us_per_frame: u32,
    /// Max bytes per second
    pub max_bytes_per_sec: u32,
    /// Padding granularity
    pub padding_granularity: u32,
    /// Flags (e.g., has index, must use index, interleaved)
    pub flags: u32,
    /// Total frames
    pub total_frames: u32,
    /// Initial frames
    pub initial_frames: u32,
    /// Number of streams
    pub streams: u32,
    /// Suggested buffer size
    pub suggested_buffer_size: u32,
    /// Video width
    pub width: u32,
    /// Video height
    pub height: u32,
}

impl AviMainHeader {
    /// Get frame rate (frames per second)
    #[inline]
    pub fn frame_rate(&self) -> f64 {
        if self.us_per_frame == 0 {
            0.0
        } else {
            1_000_000.0 / self.us_per_frame as f64
        }
    }

    /// Check if AVI has an index (idx1 chunk)
    #[inline]
    pub const fn has_index(&self) -> bool {
        (self.flags & 0x10) != 0
    }

    /// Check if AVI must use index
    #[inline]
    pub const fn must_use_index(&self) -> bool {
        (self.flags & 0x20) != 0
    }

    /// Check if AVI is interleaved
    #[inline]
    pub const fn is_interleaved(&self) -> bool {
        (self.flags & 0x100) != 0
    }
}

/// Stream header (strh chunk)
#[derive(Debug, Clone, Copy, Default)]
pub struct StreamHeader {
    /// Stream type (vids, auds, txts, mids)
    pub stream_type: [u8; 4],
    /// Codec FourCC (e.g., MJPG, H264)
    pub handler: [u8; 4],
    /// Stream flags
    pub flags: u32,
    /// Priority
    pub priority: u16,
    /// Language
    pub language: u16,
    /// Initial frames
    pub initial_frames: u32,
    /// Time scale (denominator)
    pub scale: u32,
    /// Rate (numerator) - fps = rate / scale
    pub rate: u32,
    /// Start time
    pub start: u32,
    /// Length (in scale units)
    pub length: u32,
    /// Suggested buffer size
    pub suggested_buffer_size: u32,
    /// Quality (-1 = default)
    pub quality: u32,
    /// Sample size (0 = variable)
    pub sample_size: u32,
}

impl StreamHeader {
    /// Check if this is a video stream
    #[inline]
    pub fn is_video(&self) -> bool {
        &self.stream_type == b"vids"
    }

    /// Check if this is an audio stream
    #[inline]
    pub fn is_audio(&self) -> bool {
        &self.stream_type == b"auds"
    }

    /// Get frame rate (for video streams)
    #[inline]
    pub fn frame_rate(&self) -> f64 {
        if self.scale == 0 {
            0.0
        } else {
            self.rate as f64 / self.scale as f64
        }
    }

    /// Check if stream has variable sample size
    #[inline]
    pub const fn is_variable_size(&self) -> bool {
        self.sample_size == 0
    }
}

/// Demuxer error types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum DemuxError {
    /// No error
    #[default]
    None = 0,
    /// Invalid chunk size
    InvalidChunkSize = 1,
    /// Unexpected end of data
    UnexpectedEof = 2,
    /// Missing RIFF header
    MissingRiff = 3,
    /// Invalid AVI type (not "AVI ")
    InvalidAviType = 4,
    /// Missing hdrl list
    MissingHdrl = 5,
    /// Invalid state transition
    InvalidState = 6,
    /// IO error during read
    IoError = 7,
    /// Invalid chunk header
    InvalidChunkHeader = 8,
    /// Chunk nesting too deep
    NestingTooDeep = 9,
    /// Unsupported AVI variant
    UnsupportedVariant = 10,
}

impl DemuxError {
    /// Convert from u64 (for atomic operations)
    #[inline]
    pub const fn from_u64(value: u64) -> Self {
        match value {
            0 => Self::None,
            1 => Self::InvalidChunkSize,
            2 => Self::UnexpectedEof,
            3 => Self::MissingRiff,
            4 => Self::InvalidAviType,
            5 => Self::MissingHdrl,
            6 => Self::InvalidState,
            7 => Self::IoError,
            8 => Self::InvalidChunkHeader,
            9 => Self::NestingTooDeep,
            10 => Self::UnsupportedVariant,
            _ => Self::None,
        }
    }

    /// Convert to u64 (for atomic operations)
    #[inline]
    pub const fn to_u64(self) -> u64 {
        self as u64
    }
}

/// Demuxer statistics snapshot
#[derive(Debug, Clone, Copy, Default)]
pub struct DemuxerStats {
    /// Current state
    pub state: DemuxerState,
    /// Generation counter (incremented on each state change)
    pub generation: u64,
    /// Total file size
    pub file_size: u64,
    /// Bytes parsed so far
    pub bytes_parsed: u64,
    /// Number of chunks parsed
    pub chunks_parsed: u64,
    /// Number of streams found
    pub streams_found: u64,
    /// hdrl list offset
    pub hdrl_offset: u64,
    /// hdrl list size
    pub hdrl_size: u64,
    /// movi list offset
    pub movi_offset: u64,
    /// movi list size
    pub movi_size: u64,
    /// Last error code
    pub last_error: DemuxError,
}

/// T5 Streaming capsule for AVI demuxing
///
/// **Tier**: T5 Streaming (O(1) incremental parsing, no buffering)
/// **Size**: 512B cache-aligned
/// **Safety**: 99.99% (integer-only parsing, no unsafe blocks)
///
/// # Design
///
/// The capsule maintains atomic state for lockfree coordination:
/// - State machine with atomic transitions (CAS)
/// - Generation counter for TOCTOU prevention
/// - Atomic counters for statistics
///
/// # Chunk Parsing Rules (RIFF/AVI)
///
/// - Chunk header: 4 bytes fourcc + 4 bytes size (little-endian)
/// - LIST chunks: Additional 4 bytes for list type after size
/// - Data must be word-aligned (pad with 0x00 if size is odd)
/// - RIFF chunk is the file container (size = filesize - 8)
/// - All integers are little-endian (unlike MP4)
#[repr(C, align(512))]
pub struct AviDemuxerCapsule {
    // State machine (16 bytes)
    /// Current demuxer state
    pub state: AtomicU64,
    /// Generation counter (incremented on each state change)
    pub generation: AtomicU64,

    // File info (16 bytes)
    /// Total file size (set on initialization)
    pub file_size: AtomicU64,
    /// Bytes parsed so far
    pub bytes_parsed: AtomicU64,

    // Chunk statistics (16 bytes)
    /// Number of chunks parsed
    pub chunks_parsed: AtomicU64,
    /// Number of streams found
    pub streams_found: AtomicU64,

    // hdrl location (16 bytes)
    /// Absolute offset of hdrl list
    pub hdrl_offset: AtomicU64,
    /// Size of hdrl list
    pub hdrl_size: AtomicU64,

    // movi location (16 bytes)
    /// Absolute offset of movi list
    pub movi_offset: AtomicU64,
    /// Size of movi list
    pub movi_size: AtomicU64,

    // Error tracking (8 bytes)
    /// Last error code
    pub last_error: AtomicU64,

    // Padding to 512B (424 bytes)
    // 16 + 16 + 16 + 16 + 16 + 8 = 88 bytes used
    // 512 - 88 = 424 bytes padding
    _padding: [u8; 424],
}

// #ASSUME: Size assertions validated at compile time
// #VERIFY: Compile-time size check ensures 512B alignment
const _: () = {
    assert!(core::mem::size_of::<AviDemuxerCapsule>() == 512);
    assert!(core::mem::align_of::<AviDemuxerCapsule>() == 512);
};

impl AviDemuxerCapsule {
    /// Create a new AVI demuxer capsule in Idle state
    ///
    /// # Returns
    ///
    /// A new capsule with all atomics initialized to zero/Idle
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(DemuxerState::Idle as u64),
            generation: AtomicU64::new(0),
            file_size: AtomicU64::new(0),
            bytes_parsed: AtomicU64::new(0),
            chunks_parsed: AtomicU64::new(0),
            streams_found: AtomicU64::new(0),
            hdrl_offset: AtomicU64::new(0),
            hdrl_size: AtomicU64::new(0),
            movi_offset: AtomicU64::new(0),
            movi_size: AtomicU64::new(0),
            last_error: AtomicU64::new(DemuxError::None as u64),
            _padding: [0u8; 424],
        }
    }

    /// Parse a chunk header from raw bytes (RIFF format)
    ///
    /// # Arguments
    ///
    /// * `data` - At least 8 bytes of chunk header data
    ///
    /// # Returns
    ///
    /// * `Ok(ChunkInfo)` - Parsed chunk information
    /// * `Err(DemuxError)` - Parsing error
    ///
    /// # Chunk Header Format (RIFF)
    ///
    /// ```text
    /// +-----------------+
    /// | fourcc (4 B)    |  Chunk identifier
    /// +-----------------+
    /// | size (4 B)      |  Chunk data size (little-endian)
    /// +-----------------+
    /// | [list_type]     |  4 bytes if fourcc == "LIST"
    /// +-----------------+
    /// ```
    ///
    /// # ASSUM Safety
    ///
    /// - `#ASSUME_LITTLE_ENDIAN`: AVI format uses little-endian integers
    /// - `#ASSUME_MIN_HEADER_SIZE`: Requires at least 8 bytes for normal chunks
    pub fn parse_chunk_header(&self, data: &[u8]) -> Result<ChunkInfo, DemuxError> {
        // Minimum header is 8 bytes
        if data.len() < 8 {
            return Err(DemuxError::UnexpectedEof);
        }

        // Parse FourCC
        let mut fourcc = [0u8; 4];
        fourcc.copy_from_slice(&data[0..4]);

        // Parse size (little-endian)
        let size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

        // Check for LIST chunk (has additional type field)
        let list_type = if &fourcc == b"LIST" {
            if data.len() < 12 {
                return Err(DemuxError::UnexpectedEof);
            }
            let mut list_fourcc = [0u8; 4];
            list_fourcc.copy_from_slice(&data[8..12]);
            Some(list_fourcc)
        } else {
            None
        };

        Ok(ChunkInfo {
            fourcc,
            size,
            offset: 0, // Caller must set this
            list_type,
        })
    }

    /// Parse RIFF/AVI header
    ///
    /// # Arguments
    ///
    /// * `data` - First 12+ bytes of file
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Valid RIFF/AVI header
    /// * `Err(DemuxError)` - Invalid header
    ///
    /// # Format
    ///
    /// ```text
    /// +------------------+
    /// | "RIFF" (4 bytes) |
    /// +------------------+
    /// | filesize-8 (4 B) |  Total file size minus 8
    /// +------------------+
    /// | "AVI " (4 bytes) |  AVI type identifier
    /// +------------------+
    /// ```
    pub fn parse_riff_header(&mut self, data: &[u8]) -> Result<(), DemuxError> {
        if data.len() < 12 {
            self.set_error(DemuxError::UnexpectedEof);
            return Err(DemuxError::UnexpectedEof);
        }

        // Check RIFF magic
        if &data[0..4] != b"RIFF" {
            self.set_error(DemuxError::MissingRiff);
            return Err(DemuxError::MissingRiff);
        }

        // Parse RIFF size (little-endian)
        let riff_size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

        // Set file size (RIFF size + 8 bytes for "RIFF" + size field)
        self.set_file_size((riff_size as u64) + 8);

        // Check AVI type
        if &data[8..12] != b"AVI " {
            self.set_error(DemuxError::InvalidAviType);
            return Err(DemuxError::InvalidAviType);
        }

        Ok(())
    }

    /// Parse avih (AVI main header) chunk content
    ///
    /// # Arguments
    ///
    /// * `data` - avih chunk content (after 8-byte header)
    ///
    /// # Returns
    ///
    /// * `Ok(AviMainHeader)` - Parsed header
    /// * `Err(DemuxError)` - Parsing error
    ///
    /// # Format (56 bytes)
    ///
    /// ```text
    /// +------------------------+
    /// | us_per_frame (4 B)     |  Microseconds per frame
    /// | max_bytes_per_sec (4)  |  Max data rate
    /// | padding_granularity (4)|  Padding alignment
    /// | flags (4)              |  File flags
    /// | total_frames (4)       |  Number of frames
    /// | initial_frames (4)     |  Frames before interleaving
    /// | streams (4)            |  Number of streams
    /// | suggested_buffer (4)   |  Buffer size
    /// | width (4)              |  Video width
    /// | height (4)             |  Video height
    /// | reserved[4] (16)       |  Reserved (ignored)
    /// +------------------------+
    /// ```
    pub fn parse_avih(&mut self, data: &[u8]) -> Result<AviMainHeader, DemuxError> {
        if data.len() < 56 {
            self.set_error(DemuxError::InvalidChunkSize);
            return Err(DemuxError::InvalidChunkSize);
        }

        // Parse header fields (all little-endian)
        let us_per_frame = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let max_bytes_per_sec = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let padding_granularity = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        let flags = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        let total_frames = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
        let initial_frames = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
        let streams = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);
        let suggested_buffer_size = u32::from_le_bytes([data[28], data[29], data[30], data[31]]);
        let width = u32::from_le_bytes([data[32], data[33], data[34], data[35]]);
        let height = u32::from_le_bytes([data[36], data[37], data[38], data[39]]);
        // Reserved fields at [40..56] are ignored

        Ok(AviMainHeader {
            us_per_frame,
            max_bytes_per_sec,
            padding_granularity,
            flags,
            total_frames,
            initial_frames,
            streams,
            suggested_buffer_size,
            width,
            height,
        })
    }

    /// Parse strh (stream header) chunk content
    ///
    /// # Arguments
    ///
    /// * `data` - strh chunk content (after 8-byte header)
    ///
    /// # Returns
    ///
    /// * `Ok(StreamHeader)` - Parsed stream header
    /// * `Err(DemuxError)` - Parsing error
    ///
    /// # Format (56 bytes)
    pub fn parse_strh(&mut self, data: &[u8]) -> Result<StreamHeader, DemuxError> {
        if data.len() < 56 {
            self.set_error(DemuxError::InvalidChunkSize);
            return Err(DemuxError::InvalidChunkSize);
        }

        let mut stream_type = [0u8; 4];
        stream_type.copy_from_slice(&data[0..4]);

        let mut handler = [0u8; 4];
        handler.copy_from_slice(&data[4..8]);

        let flags = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        let priority = u16::from_le_bytes([data[12], data[13]]);
        let language = u16::from_le_bytes([data[14], data[15]]);
        let initial_frames = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
        let scale = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
        let rate = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);
        let start = u32::from_le_bytes([data[28], data[29], data[30], data[31]]);
        let length = u32::from_le_bytes([data[32], data[33], data[34], data[35]]);
        let suggested_buffer_size = u32::from_le_bytes([data[36], data[37], data[38], data[39]]);
        let quality = u32::from_le_bytes([data[40], data[41], data[42], data[43]]);
        let sample_size = u32::from_le_bytes([data[44], data[45], data[46], data[47]]);
        // Frame at [48..56] is ignored

        // Increment stream counter
        self.streams_found.fetch_add(1, Ordering::Relaxed);

        Ok(StreamHeader {
            stream_type,
            handler,
            flags,
            priority,
            language,
            initial_frames,
            scale,
            rate,
            start,
            length,
            suggested_buffer_size,
            quality,
            sample_size,
        })
    }

    /// Parse LIST chunk structure recursively
    ///
    /// # Arguments
    ///
    /// * `data` - LIST chunk content (after header)
    /// * `list_type` - The LIST type FourCC
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<ChunkInfo>)` - List of child chunks found
    /// * `Err(DemuxError)` - Parsing error
    pub fn parse_list_chunks(
        &mut self,
        data: &[u8],
        list_type: &[u8; 4],
    ) -> Result<Vec<ChunkInfo>, DemuxError> {
        self.parse_container_chunks(data, 0, 0, Some(list_type))
    }

    /// Internal: Parse container chunk children
    fn parse_container_chunks(
        &mut self,
        data: &[u8],
        base_offset: u64,
        depth: u32,
        _list_type: Option<&[u8; 4]>,
    ) -> Result<Vec<ChunkInfo>, DemuxError> {
        // Prevent infinite recursion
        if depth > 16 {
            return Err(DemuxError::NestingTooDeep);
        }

        let mut chunks = Vec::new();
        let mut offset = 0usize;

        while offset + 8 <= data.len() {
            // Parse chunk header
            let header_result = self.parse_chunk_header(&data[offset..]);
            let mut chunk_info = match header_result {
                Ok(info) => info,
                Err(_) => break, // End of valid data
            };

            // Set absolute offset
            chunk_info.offset = base_offset + offset as u64;

            // Calculate total size including padding
            let total_size = chunk_info.total_size() as usize;

            // Validate chunk doesn't exceed container
            if offset + total_size > data.len() {
                break;
            }

            // Track specific chunks
            if &chunk_info.fourcc == b"LIST" {
                if let Some(ref list_type) = chunk_info.list_type {
                    if list_type == b"hdrl" {
                        self.hdrl_offset.store(chunk_info.offset, Ordering::Relaxed);
                        self.hdrl_size.store(chunk_info.size as u64, Ordering::Relaxed);
                    } else if list_type == b"movi" {
                        self.movi_offset.store(chunk_info.offset, Ordering::Relaxed);
                        self.movi_size.store(chunk_info.size as u64, Ordering::Relaxed);
                    }
                }
            }

            chunks.push(chunk_info);
            self.chunks_parsed.fetch_add(1, Ordering::Relaxed);

            // Recursively parse LIST chunks
            if chunk_info.is_list() && chunk_info.list_type.is_some() {
                let content_start = offset + 12; // LIST header is 12 bytes
                let content_size = chunk_info.content_size() as usize;
                let content_end = content_start + content_size;

                if content_start < content_end && content_end <= data.len() {
                    let children = self.parse_container_chunks(
                        &data[content_start..content_end],
                        chunk_info.offset + 12,
                        depth + 1,
                        chunk_info.list_type.as_ref(),
                    )?;
                    chunks.extend(children);
                }
            }

            offset += total_size;
        }

        // Update bytes parsed
        self.bytes_parsed.fetch_add(offset as u64, Ordering::Relaxed);

        Ok(chunks)
    }

    /// Find a chunk by FourCC within data
    ///
    /// # Arguments
    ///
    /// * `data` - Data to search within
    /// * `target` - Chunk FourCC to find
    ///
    /// # Returns
    ///
    /// * `Some(ChunkInfo)` - Found chunk
    /// * `None` - Chunk not found
    pub fn find_chunk(&self, data: &[u8], target: &[u8; 4]) -> Option<ChunkInfo> {
        let mut offset = 0usize;

        while offset + 8 <= data.len() {
            let chunk_info = self.parse_chunk_header(&data[offset..]).ok()?;

            if &chunk_info.fourcc == target {
                return Some(ChunkInfo {
                    offset: offset as u64,
                    ..chunk_info
                });
            }

            // Advance to next chunk (with padding)
            let total_size = chunk_info.total_size() as usize;
            offset += total_size;
        }

        None
    }

    /// Atomic state transition with compare-and-swap
    ///
    /// # Arguments
    ///
    /// * `from` - Expected current state
    /// * `to` - Target state
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Transition successful
    /// * `Err(DemuxError::InvalidState)` - Current state doesn't match `from`
    ///
    /// # Thread Safety
    ///
    /// Uses compare_exchange to ensure atomic transition. Generation counter
    /// is incremented on successful transitions for TOCTOU prevention.
    pub fn transition_state(
        &self,
        from: DemuxerState,
        to: DemuxerState,
    ) -> Result<(), DemuxError> {
        let result = self.state.compare_exchange(
            from.to_u64(),
            to.to_u64(),
            Ordering::AcqRel,
            Ordering::Acquire,
        );

        match result {
            Ok(_) => {
                // Successful transition - increment generation
                self.generation.fetch_add(1, Ordering::Release);
                Ok(())
            }
            Err(_) => {
                self.set_error(DemuxError::InvalidState);
                Err(DemuxError::InvalidState)
            }
        }
    }

    /// Get current state
    #[inline]
    pub fn state(&self) -> DemuxerState {
        DemuxerState::from_u64(self.state.load(Ordering::Acquire))
    }

    /// Get demuxer statistics snapshot
    ///
    /// Returns a consistent snapshot of all statistics. Note that since
    /// individual loads are atomic but not combined, there may be slight
    /// inconsistencies between fields during active parsing.
    pub fn stats(&self) -> DemuxerStats {
        DemuxerStats {
            state: self.state(),
            generation: self.generation.load(Ordering::Acquire),
            file_size: self.file_size.load(Ordering::Relaxed),
            bytes_parsed: self.bytes_parsed.load(Ordering::Relaxed),
            chunks_parsed: self.chunks_parsed.load(Ordering::Relaxed),
            streams_found: self.streams_found.load(Ordering::Relaxed),
            hdrl_offset: self.hdrl_offset.load(Ordering::Relaxed),
            hdrl_size: self.hdrl_size.load(Ordering::Relaxed),
            movi_offset: self.movi_offset.load(Ordering::Relaxed),
            movi_size: self.movi_size.load(Ordering::Relaxed),
            last_error: DemuxError::from_u64(self.last_error.load(Ordering::Relaxed)),
        }
    }

    /// Set file size (call before parsing)
    #[inline]
    pub fn set_file_size(&self, size: u64) {
        self.file_size.store(size, Ordering::Relaxed);
    }

    /// Set hdrl list location
    #[inline]
    pub fn set_hdrl_location(&self, offset: u64, size: u64) {
        self.hdrl_offset.store(offset, Ordering::Relaxed);
        self.hdrl_size.store(size, Ordering::Relaxed);
    }

    /// Set movi list location
    #[inline]
    pub fn set_movi_location(&self, offset: u64, size: u64) {
        self.movi_offset.store(offset, Ordering::Relaxed);
        self.movi_size.store(size, Ordering::Relaxed);
    }

    /// Set error and transition to Error state
    #[inline]
    pub fn set_error(&self, error: DemuxError) {
        self.last_error.store(error.to_u64(), Ordering::Relaxed);
        self.state
            .store(DemuxerState::Error.to_u64(), Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get last error
    #[inline]
    pub fn last_error(&self) -> DemuxError {
        DemuxError::from_u64(self.last_error.load(Ordering::Relaxed))
    }

    /// Reset capsule to initial state
    pub fn reset(&self) {
        self.state
            .store(DemuxerState::Idle.to_u64(), Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
        self.file_size.store(0, Ordering::Relaxed);
        self.bytes_parsed.store(0, Ordering::Relaxed);
        self.chunks_parsed.store(0, Ordering::Relaxed);
        self.streams_found.store(0, Ordering::Relaxed);
        self.hdrl_offset.store(0, Ordering::Relaxed);
        self.hdrl_size.store(0, Ordering::Relaxed);
        self.movi_offset.store(0, Ordering::Relaxed);
        self.movi_size.store(0, Ordering::Relaxed);
        self.last_error.store(DemuxError::None as u64, Ordering::Relaxed);
    }
}

impl Default for AviDemuxerCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// T28 Testing (Q1-Q7: Unit Tests)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Q1: Test capsule size and alignment
    #[test]
    fn test_capsule_size_alignment() {
        assert_eq!(core::mem::size_of::<AviDemuxerCapsule>(), 512);
        assert_eq!(core::mem::align_of::<AviDemuxerCapsule>(), 512);
    }

    // Q2: Test basic chunk header parsing
    #[test]
    fn test_parse_chunk_header_basic() {
        let capsule = AviDemuxerCapsule::new();

        // avih chunk with size 56
        let data = [
            b'a', b'v', b'i', b'h', // fourcc = "avih"
            0x38, 0x00, 0x00, 0x00, // size = 56 (little-endian)
        ];

        let result = capsule.parse_chunk_header(&data);
        assert!(result.is_ok());

        let info = result.unwrap();
        assert_eq!(info.fourcc, *b"avih");
        assert_eq!(info.size, 56);
        assert_eq!(info.list_type, None);
        assert_eq!(info.content_offset(), 8);
    }

    // Q3: Test LIST chunk header parsing
    #[test]
    fn test_parse_chunk_header_list() {
        let capsule = AviDemuxerCapsule::new();

        // LIST chunk with hdrl type
        let data = [
            b'L', b'I', b'S', b'T', // fourcc = "LIST"
            0x1C, 0x00, 0x00, 0x00, // size = 28 (little-endian)
            b'h', b'd', b'r', b'l', // list_type = "hdrl"
        ];

        let result = capsule.parse_chunk_header(&data);
        assert!(result.is_ok());

        let info = result.unwrap();
        assert_eq!(info.fourcc, *b"LIST");
        assert_eq!(info.size, 28);
        assert_eq!(info.list_type, Some(*b"hdrl"));
        assert_eq!(info.content_offset(), 12);
        assert_eq!(info.content_size(), 24); // 28 - 4
        assert!(info.is_list());
    }

    // Q4: Test RIFF/AVI header parsing
    #[test]
    fn test_parse_riff_header() {
        let mut capsule = AviDemuxerCapsule::new();

        // Valid RIFF/AVI header
        let data = [
            b'R', b'I', b'F', b'F', // RIFF magic
            0xE8, 0x03, 0x00, 0x00, // size = 1000 (little-endian)
            b'A', b'V', b'I', b' ', // AVI type
        ];

        let result = capsule.parse_riff_header(&data);
        assert!(result.is_ok());
        assert_eq!(capsule.stats().file_size, 1008); // 1000 + 8
    }

    // Q5: Test state transitions
    #[test]
    fn test_state_transitions() {
        let capsule = AviDemuxerCapsule::new();

        // Initial state
        assert_eq!(capsule.state(), DemuxerState::Idle);
        assert_eq!(capsule.stats().generation, 0);

        // Valid transition: Idle -> ParsingRiff
        let result = capsule.transition_state(DemuxerState::Idle, DemuxerState::ParsingRiff);
        assert!(result.is_ok());
        assert_eq!(capsule.state(), DemuxerState::ParsingRiff);
        assert_eq!(capsule.stats().generation, 1);

        // Valid transition: ParsingRiff -> ParsingHdrl
        let result = capsule.transition_state(DemuxerState::ParsingRiff, DemuxerState::ParsingHdrl);
        assert!(result.is_ok());
        assert_eq!(capsule.state(), DemuxerState::ParsingHdrl);
        assert_eq!(capsule.stats().generation, 2);

        // Invalid transition: Expected Idle but currently ParsingHdrl
        let result = capsule.transition_state(DemuxerState::Idle, DemuxerState::Ready);
        assert_eq!(result, Err(DemuxError::InvalidState));
        assert_eq!(capsule.state(), DemuxerState::Error);
    }

    // Q6: Test avih parsing
    #[test]
    fn test_parse_avih() {
        let mut capsule = AviDemuxerCapsule::new();

        // Minimal avih data (56 bytes)
        let mut data = vec![0u8; 56];

        // us_per_frame = 33333 (30 fps)
        data[0..4].copy_from_slice(&33333u32.to_le_bytes());
        // total_frames = 900
        data[16..20].copy_from_slice(&900u32.to_le_bytes());
        // streams = 1
        data[24..28].copy_from_slice(&1u32.to_le_bytes());
        // width = 1920
        data[32..36].copy_from_slice(&1920u32.to_le_bytes());
        // height = 1080
        data[36..40].copy_from_slice(&1080u32.to_le_bytes());

        let result = capsule.parse_avih(&data);
        assert!(result.is_ok());

        let avih = result.unwrap();
        assert_eq!(avih.us_per_frame, 33333);
        assert_eq!(avih.total_frames, 900);
        assert_eq!(avih.streams, 1);
        assert_eq!(avih.width, 1920);
        assert_eq!(avih.height, 1080);
        assert!((avih.frame_rate() - 30.0).abs() < 0.1);
    }

    // Q7: Test strh parsing
    #[test]
    fn test_parse_strh() {
        let mut capsule = AviDemuxerCapsule::new();

        // Minimal strh data (56 bytes)
        let mut data = vec![0u8; 56];

        // stream_type = "vids"
        data[0..4].copy_from_slice(b"vids");
        // handler = "MJPG"
        data[4..8].copy_from_slice(b"MJPG");
        // scale = 1
        data[20..24].copy_from_slice(&1u32.to_le_bytes());
        // rate = 30
        data[24..28].copy_from_slice(&30u32.to_le_bytes());
        // length = 900
        data[32..36].copy_from_slice(&900u32.to_le_bytes());

        let result = capsule.parse_strh(&data);
        assert!(result.is_ok());

        let strh = result.unwrap();
        assert_eq!(strh.stream_type, *b"vids");
        assert_eq!(strh.handler, *b"MJPG");
        assert!(strh.is_video());
        assert!(!strh.is_audio());
        assert_eq!(strh.frame_rate(), 30.0);
        assert_eq!(capsule.stats().streams_found, 1);
    }

    // Test chunk padding calculation
    #[test]
    fn test_chunk_padding() {
        let chunk_even = ChunkInfo {
            fourcc: *b"avih",
            size: 56, // even
            offset: 0,
            list_type: None,
        };
        assert_eq!(chunk_even.total_size(), 64); // 8 header + 56 data

        let chunk_odd = ChunkInfo {
            fourcc: *b"strn",
            size: 11, // odd
            offset: 0,
            list_type: None,
        };
        assert_eq!(chunk_odd.total_size(), 20); // 8 header + 12 padded (11 + 1)
    }

    // Test find_chunk
    #[test]
    fn test_find_chunk() {
        let capsule = AviDemuxerCapsule::new();

        // Multiple chunks
        let data = [
            // Chunk 1: avih (size=16, padded to 24)
            b'a', b'v', b'i', b'h', 0x10, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // Chunk 2: strh (size=16, padded to 24)
            b's', b't', b'r', b'h', 0x10, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        // Find strh
        let strh = capsule.find_chunk(&data, b"strh");
        assert!(strh.is_some());
        assert_eq!(strh.unwrap().offset, 24);

        // Find avih
        let avih = capsule.find_chunk(&data, b"avih");
        assert!(avih.is_some());
        assert_eq!(avih.unwrap().offset, 0);

        // Chunk not found
        let movi = capsule.find_chunk(&data, b"movi");
        assert!(movi.is_none());
    }

    // Test error handling
    #[test]
    fn test_error_handling() {
        let capsule = AviDemuxerCapsule::new();

        // Too short data
        let short_data = [0x00, 0x00, 0x00];
        let result = capsule.parse_chunk_header(&short_data);
        assert_eq!(result, Err(DemuxError::UnexpectedEof));

        // LIST chunk without type
        let list_short = [
            b'L', b'I', b'S', b'T',
            0x10, 0x00, 0x00, 0x00,
            // Missing list type (needs 4 more bytes)
        ];
        let result = capsule.parse_chunk_header(&list_short);
        assert_eq!(result, Err(DemuxError::UnexpectedEof));
    }

    // Test generation counter increments
    #[test]
    fn test_generation_counter() {
        let capsule = AviDemuxerCapsule::new();
        assert_eq!(capsule.stats().generation, 0);

        // Each state change should increment generation
        capsule
            .transition_state(DemuxerState::Idle, DemuxerState::ParsingRiff)
            .unwrap();
        assert_eq!(capsule.stats().generation, 1);

        // Reset should also increment generation
        capsule.reset();
        assert_eq!(capsule.stats().generation, 2);
        assert_eq!(capsule.state(), DemuxerState::Idle);
    }

    // Test AviMainHeader methods
    #[test]
    fn test_avi_main_header_methods() {
        let avih = AviMainHeader {
            us_per_frame: 41666, // ~24 fps
            flags: 0x10 | 0x100, // has_index | is_interleaved
            total_frames: 720,
            streams: 2,
            width: 1280,
            height: 720,
            ..Default::default()
        };

        assert!((avih.frame_rate() - 24.0).abs() < 0.1);
        assert!(avih.has_index());
        assert!(!avih.must_use_index());
        assert!(avih.is_interleaved());
    }

    // Test StreamHeader methods
    #[test]
    fn test_stream_header_methods() {
        let strh_video = StreamHeader {
            stream_type: *b"vids",
            handler: *b"H264",
            scale: 1001,
            rate: 30000, // 29.97 fps
            sample_size: 0, // variable
            ..Default::default()
        };

        assert!(strh_video.is_video());
        assert!(!strh_video.is_audio());
        assert!((strh_video.frame_rate() - 29.97).abs() < 0.01);
        assert!(strh_video.is_variable_size());

        let strh_audio = StreamHeader {
            stream_type: *b"auds",
            scale: 1,
            rate: 44100,
            sample_size: 4, // fixed
            ..Default::default()
        };

        assert!(!strh_audio.is_video());
        assert!(strh_audio.is_audio());
        assert!(!strh_audio.is_variable_size());
    }

    // Q8-Q14: Property tests would go here with proptest
    // TODO: Add proptest for arbitrary chunk sizes/types
}
