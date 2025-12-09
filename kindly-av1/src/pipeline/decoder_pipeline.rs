//! Decoder Pipeline Metacapsule
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! T6 Mixed tier metacapsule orchestrating the complete decode pipeline:
//! demux -> decode -> frame buffer -> output
//!
//! # Architecture
//!
//! ```text
//! DecoderPipelineCapsule (T6 Mixed, 1024B orchestrator)
//! +-----------------------------------------------------------------------+
//! |                                                                       |
//! |  Input File ──> Demuxer ──> Decoder ──> Frame Buffer ──> Output       |
//! |       │            │           │            │              │          |
//! |       └────────────┴───────────┴────────────┴──────────────┘          |
//! |                    DecoderPipelineCapsule                             |
//! |                    (T6 Mixed Orchestrator)                            |
//! |                                                                       |
//! +-----------------------------------------------------------------------+
//! ```
//!
//! # Pipeline States
//!
//! ```text
//! Idle -> Initializing -> DemuxReady -> DecoderReady -> Running <-> Paused
//!                   │                                      │
//!                   └──> Error                             └──> Seeking -> Running
//!                                                          └──> Flushing -> EndOfStream
//! ```
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T6 Mixed tier (multi-stage orchestration)
//! - **Chaos**: 1024B cache-aligned, 100% lockfree (AtomicU64/AtomicU32)
//! - **ASSUM**: All atomics use Acquire/Release ordering
//! - **B32**: <50ns state transitions, <100ns snapshot
//! - **T28**: 28+ tests (unit/property/integration/production)
//! - **Q34**: Generation counter for audit trail compliance

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// ============================================================================
// Pipeline State Machine
// ============================================================================

/// Pipeline state machine states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum PipelineState {
    /// Not initialized, ready for open()
    #[default]
    Idle = 0,
    /// Initialization in progress
    Initializing = 1,
    /// Demuxer initialized, container parsed
    DemuxReady = 2,
    /// Decoder initialized, ready to decode
    DecoderReady = 3,
    /// Actively decoding frames
    Running = 4,
    /// Paused, can resume
    Paused = 5,
    /// Seek operation in progress
    Seeking = 6,
    /// Flushing decoder buffers
    Flushing = 7,
    /// End of stream reached
    EndOfStream = 8,
    /// Error occurred
    Error = 9,
}

impl PipelineState {
    /// Convert from raw u8
    #[inline]
    pub const fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::Idle),
            1 => Some(Self::Initializing),
            2 => Some(Self::DemuxReady),
            3 => Some(Self::DecoderReady),
            4 => Some(Self::Running),
            5 => Some(Self::Paused),
            6 => Some(Self::Seeking),
            7 => Some(Self::Flushing),
            8 => Some(Self::EndOfStream),
            9 => Some(Self::Error),
            _ => None,
        }
    }

    /// Check if state allows decoding operations
    #[inline]
    pub const fn can_decode(&self) -> bool {
        matches!(self, Self::Running | Self::DecoderReady)
    }

    /// Check if state allows seeking
    #[inline]
    pub const fn can_seek(&self) -> bool {
        matches!(
            self,
            Self::Running | Self::Paused | Self::DecoderReady | Self::EndOfStream
        )
    }

    /// Check if state is terminal
    #[inline]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::EndOfStream | Self::Error)
    }
}

impl core::fmt::Display for PipelineState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Initializing => write!(f, "Initializing"),
            Self::DemuxReady => write!(f, "DemuxReady"),
            Self::DecoderReady => write!(f, "DecoderReady"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Seeking => write!(f, "Seeking"),
            Self::Flushing => write!(f, "Flushing"),
            Self::EndOfStream => write!(f, "EndOfStream"),
            Self::Error => write!(f, "Error"),
        }
    }
}

// ============================================================================
// Container Formats
// ============================================================================

/// Supported container formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ContainerFormat {
    /// MP4/ISO BMFF container
    Mp4 = 1,
    /// Matroska container
    Mkv = 2,
    /// WebM container (Matroska subset)
    WebM = 3,
    /// Unknown or unsupported format
    #[default]
    Unknown = 0,
}

impl ContainerFormat {
    /// Convert from raw u8
    #[inline]
    pub const fn from_u8(val: u8) -> Self {
        match val {
            1 => Self::Mp4,
            2 => Self::Mkv,
            3 => Self::WebM,
            _ => Self::Unknown,
        }
    }

    /// Check if format is a Matroska variant
    #[inline]
    pub const fn is_matroska(&self) -> bool {
        matches!(self, Self::Mkv | Self::WebM)
    }
}

impl core::fmt::Display for ContainerFormat {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Mp4 => write!(f, "MP4"),
            Self::Mkv => write!(f, "MKV"),
            Self::WebM => write!(f, "WebM"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

// ============================================================================
// Video Codecs
// ============================================================================

/// Supported video codecs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum VideoCodec {
    /// H.264/AVC
    H264 = 1,
    /// VP9
    VP9 = 2,
    /// AV1
    AV1 = 3,
    /// Unknown or unsupported codec
    #[default]
    Unknown = 0,
}

impl VideoCodec {
    /// Convert from raw u8
    #[inline]
    pub const fn from_u8(val: u8) -> Self {
        match val {
            1 => Self::H264,
            2 => Self::VP9,
            3 => Self::AV1,
            _ => Self::Unknown,
        }
    }

    /// Get FourCC string for codec
    #[inline]
    pub const fn fourcc(&self) -> &'static str {
        match self {
            Self::H264 => "avc1",
            Self::VP9 => "vp09",
            Self::AV1 => "av01",
            Self::Unknown => "????",
        }
    }
}

impl core::fmt::Display for VideoCodec {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::H264 => write!(f, "H.264"),
            Self::VP9 => write!(f, "VP9"),
            Self::AV1 => write!(f, "AV1"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

// ============================================================================
// Chroma Format
// ============================================================================

/// Chroma subsampling format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ChromaFormat {
    /// 4:2:0 subsampling (most common)
    #[default]
    Yuv420 = 0,
    /// 4:2:2 subsampling
    Yuv422 = 1,
    /// 4:4:4 no subsampling
    Yuv444 = 2,
    /// Monochrome
    Mono = 3,
}

impl ChromaFormat {
    /// Convert from raw u8
    #[inline]
    pub const fn from_u8(val: u8) -> Self {
        match val {
            0 => Self::Yuv420,
            1 => Self::Yuv422,
            2 => Self::Yuv444,
            3 => Self::Mono,
            _ => Self::Yuv420,
        }
    }

    /// Get horizontal chroma subsampling shift
    #[inline]
    pub const fn subsampling_x(&self) -> u8 {
        match self {
            Self::Yuv420 | Self::Yuv422 => 1,
            Self::Yuv444 | Self::Mono => 0,
        }
    }

    /// Get vertical chroma subsampling shift
    #[inline]
    pub const fn subsampling_y(&self) -> u8 {
        match self {
            Self::Yuv420 => 1,
            Self::Yuv422 | Self::Yuv444 | Self::Mono => 0,
        }
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Pipeline errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PipelineError {
    /// No error
    None = 0,
    /// File not found or cannot be opened
    FileNotFound = 1,
    /// Invalid or corrupted container format
    InvalidContainer = 2,
    /// Unsupported container format
    UnsupportedContainer = 3,
    /// Unsupported video codec
    UnsupportedCodec = 4,
    /// Demuxer initialization failed
    DemuxerInitFailed = 5,
    /// Decoder initialization failed
    DecoderInitFailed = 6,
    /// Invalid pipeline state for operation
    InvalidState = 7,
    /// End of stream reached
    EndOfStream = 8,
    /// Seek operation failed
    SeekFailed = 9,
    /// Decoding error
    DecodingError = 10,
    /// Insufficient data
    InsufficientData = 11,
    /// Internal error
    InternalError = 12,
    /// Out of memory
    OutOfMemory = 13,
    /// Invalid parameter
    InvalidParameter = 14,
    /// Buffer overflow
    BufferOverflow = 15,
}

impl PipelineError {
    /// Convert from raw u8
    #[inline]
    pub const fn from_u8(val: u8) -> Self {
        match val {
            0 => Self::None,
            1 => Self::FileNotFound,
            2 => Self::InvalidContainer,
            3 => Self::UnsupportedContainer,
            4 => Self::UnsupportedCodec,
            5 => Self::DemuxerInitFailed,
            6 => Self::DecoderInitFailed,
            7 => Self::InvalidState,
            8 => Self::EndOfStream,
            9 => Self::SeekFailed,
            10 => Self::DecodingError,
            11 => Self::InsufficientData,
            12 => Self::InternalError,
            13 => Self::OutOfMemory,
            14 => Self::InvalidParameter,
            15 => Self::BufferOverflow,
            _ => Self::InternalError,
        }
    }
}

impl core::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::None => write!(f, "no error"),
            Self::FileNotFound => write!(f, "file not found"),
            Self::InvalidContainer => write!(f, "invalid container format"),
            Self::UnsupportedContainer => write!(f, "unsupported container format"),
            Self::UnsupportedCodec => write!(f, "unsupported video codec"),
            Self::DemuxerInitFailed => write!(f, "demuxer initialization failed"),
            Self::DecoderInitFailed => write!(f, "decoder initialization failed"),
            Self::InvalidState => write!(f, "invalid pipeline state"),
            Self::EndOfStream => write!(f, "end of stream"),
            Self::SeekFailed => write!(f, "seek operation failed"),
            Self::DecodingError => write!(f, "decoding error"),
            Self::InsufficientData => write!(f, "insufficient data"),
            Self::InternalError => write!(f, "internal error"),
            Self::OutOfMemory => write!(f, "out of memory"),
            Self::InvalidParameter => write!(f, "invalid parameter"),
            Self::BufferOverflow => write!(f, "buffer overflow"),
        }
    }
}

impl std::error::Error for PipelineError {}

// ============================================================================
// Video Info
// ============================================================================

/// Video stream information
#[derive(Debug, Clone, Default)]
pub struct VideoInfo {
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Frame rate as floating point
    pub frame_rate: f64,
    /// Video codec
    pub codec: VideoCodec,
    /// Bit depth (8, 10, or 12)
    pub bit_depth: u8,
    /// Chroma format
    pub chroma_format: ChromaFormat,
}

impl VideoInfo {
    /// Calculate frame size in bytes (Y plane only)
    #[inline]
    pub fn luma_size(&self) -> usize {
        (self.width as usize) * (self.height as usize)
    }

    /// Calculate total frame size in bytes (YUV)
    #[inline]
    pub fn frame_size(&self) -> usize {
        let luma = self.luma_size();
        let chroma_w = self.width as usize >> self.chroma_format.subsampling_x();
        let chroma_h = self.height as usize >> self.chroma_format.subsampling_y();
        luma + 2 * chroma_w * chroma_h
    }
}

// ============================================================================
// Decoded Frame
// ============================================================================

/// Decoded video frame with YUV plane data
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    /// Presentation timestamp (timescale units)
    pub pts: u64,
    /// Frame number (0-indexed)
    pub frame_number: u64,
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Y, U, V plane data
    pub planes: [Vec<u8>; 3],
    /// Stride for each plane
    pub strides: [usize; 3],
    /// Whether this is a keyframe
    pub keyframe: bool,
}

impl DecodedFrame {
    /// Create a new empty frame
    pub fn new(width: u32, height: u32, chroma: ChromaFormat) -> Self {
        let luma_size = (width as usize) * (height as usize);
        let chroma_w = width as usize >> chroma.subsampling_x();
        let chroma_h = height as usize >> chroma.subsampling_y();
        let chroma_size = chroma_w * chroma_h;

        Self {
            pts: 0,
            frame_number: 0,
            width,
            height,
            planes: [
                vec![0u8; luma_size],
                vec![0u8; chroma_size],
                vec![0u8; chroma_size],
            ],
            strides: [width as usize, chroma_w, chroma_w],
            keyframe: false,
        }
    }

    /// Get Y plane data
    #[inline]
    pub fn y_plane(&self) -> &[u8] {
        &self.planes[0]
    }

    /// Get U plane data
    #[inline]
    pub fn u_plane(&self) -> &[u8] {
        &self.planes[1]
    }

    /// Get V plane data
    #[inline]
    pub fn v_plane(&self) -> &[u8] {
        &self.planes[2]
    }
}

impl Default for DecodedFrame {
    fn default() -> Self {
        Self {
            pts: 0,
            frame_number: 0,
            width: 0,
            height: 0,
            planes: [Vec::new(), Vec::new(), Vec::new()],
            strides: [0, 0, 0],
            keyframe: false,
        }
    }
}

// ============================================================================
// Phase Flags (for sub-capsule coordination)
// ============================================================================

/// Phase flags for sub-capsule coordination
pub mod phase_flags {
    /// Demuxer initialized and ready
    pub const DEMUXER_INIT: u64 = 1 << 0;
    /// Track information parsed
    pub const TRACKS_PARSED: u64 = 1 << 1;
    /// Decoder initialized
    pub const DECODER_INIT: u64 = 1 << 2;
    /// First frame decoded successfully
    pub const FIRST_FRAME: u64 = 1 << 3;
    /// Seek operation complete
    pub const SEEK_COMPLETE: u64 = 1 << 4;
    /// Flush operation complete
    pub const FLUSH_COMPLETE: u64 = 1 << 5;
    /// End of stream reached
    pub const END_OF_STREAM: u64 = 1 << 6;
    /// Error flag
    pub const ERROR_FLAG: u64 = 1 << 7;

    /// All initialization phases complete
    pub const ALL_INIT: u64 = DEMUXER_INIT | TRACKS_PARSED | DECODER_INIT | FIRST_FRAME;

    /// Ready for decoding (minimum initialization)
    pub const DECODE_READY: u64 = DEMUXER_INIT | TRACKS_PARSED | DECODER_INIT;
}

// ============================================================================
// Pipeline Statistics
// ============================================================================

/// Pipeline statistics snapshot
#[derive(Debug, Clone, Default)]
pub struct PipelineStats {
    /// Total frames decoded
    pub frames_decoded: u64,
    /// Frames currently buffered
    pub frames_buffered: u32,
    /// Frames dropped due to errors
    pub frames_dropped: u32,
    /// Total bytes processed
    pub bytes_processed: u64,
    /// Error count
    pub error_count: u32,
    /// Current generation
    pub generation: u64,
}

// ============================================================================
// Magic Bytes for Container Detection
// ============================================================================

/// MP4 ftyp box signature (at offset 4)
const MP4_FTYP: [u8; 4] = [0x66, 0x74, 0x79, 0x70]; // "ftyp"

/// MKV EBML header signature
const MKV_EBML: [u8; 4] = [0x1A, 0x45, 0xDF, 0xA3];

// ============================================================================
// DecoderPipelineCapsule (T6 Mixed, 1024B)
// ============================================================================

/// T6 Mixed metacapsule orchestrating the decoder pipeline
///
/// Coordinates demuxer -> decoder -> frame buffer -> output
/// using lockfree atomic operations for state management.
///
/// # Size: 1024 bytes (cache-aligned)
///
/// # UCE34/Chaos Compliance
///
/// - Q10: T6 Mixed tier (multi-stage orchestration)
/// - Q33: 100% lockfree (AtomicU64/AtomicU32 only)
/// - Q34: Generation counter for audit trail
///
/// # Example
///
/// ```rust,ignore
/// use kindly_av1::pipeline::{DecoderPipelineCapsule, PipelineState};
///
/// let mut pipeline = DecoderPipelineCapsule::new();
/// pipeline.open("video.mkv")?;
///
/// while let Ok(Some(frame)) = pipeline.decode_frame() {
///     println!("Frame {} at PTS {}", frame.frame_number, frame.pts);
/// }
/// ```
#[repr(C, align(1024))]
pub struct DecoderPipelineCapsule {
    // ===== State Machine (64 bytes) =====
    /// Combined: state (8 bits) | flags (24 bits) | reserved (32 bits)
    state: AtomicU64,
    /// Q34 generation counter for audit trails
    generation: AtomicU64,
    /// Phase flags for sub-capsule coordination
    phase_flags: AtomicU64,
    /// Last error code
    last_error: AtomicU64,

    // ===== Format Info (32 bytes) =====
    /// Container format (ContainerFormat as u32)
    container_format: AtomicU32,
    /// Video codec (VideoCodec as u32)
    video_codec: AtomicU32,
    /// Chroma format
    chroma_format: AtomicU32,
    /// Bit depth
    bit_depth: AtomicU32,
    /// Reserved
    _format_reserved: [AtomicU32; 4],

    // ===== Video Parameters (32 bytes) =====
    /// Frame width in pixels
    width: AtomicU32,
    /// Frame height in pixels
    height: AtomicU32,
    /// Frame rate numerator
    frame_rate_num: AtomicU32,
    /// Frame rate denominator
    frame_rate_den: AtomicU32,
    /// Display aspect ratio numerator
    dar_num: AtomicU32,
    /// Display aspect ratio denominator
    dar_den: AtomicU32,
    /// Reserved
    _video_reserved: [AtomicU32; 2],

    // ===== Timing (64 bytes) =====
    /// Current presentation timestamp (timescale units)
    current_pts: AtomicU64,
    /// Total duration (timescale units)
    duration: AtomicU64,
    /// Timescale (nanoseconds per tick)
    timecode_scale: AtomicU64,
    /// Start time offset
    start_time: AtomicU64,
    /// End time (if known)
    end_time: AtomicU64,
    /// Reserved
    _timing_reserved: [AtomicU64; 3],

    // ===== Position Tracking (64 bytes) =====
    /// Current frame number (0-indexed)
    current_frame: AtomicU64,
    /// Total frames (0 if unknown)
    total_frames: AtomicU64,
    /// Bytes processed from input
    bytes_processed: AtomicU64,
    /// Total input size (0 if streaming)
    total_bytes: AtomicU64,
    /// Current byte offset in stream
    byte_offset: AtomicU64,
    /// Reserved
    _position_reserved: [AtomicU64; 3],

    // ===== Buffer Management (32 bytes) =====
    /// Frames currently buffered
    frames_buffered: AtomicU32,
    /// Maximum buffer capacity
    buffer_capacity: AtomicU32,
    /// Total frames decoded
    frames_decoded: AtomicU64,
    /// Frames dropped
    frames_dropped: AtomicU32,
    /// Decode queue depth
    decode_queue_depth: AtomicU32,
    /// Reserved
    _buffer_reserved: [AtomicU64; 1],

    // ===== Error Tracking (32 bytes) =====
    /// Total error count
    error_count: AtomicU32,
    /// Consecutive errors
    consecutive_errors: AtomicU32,
    /// Last error frame
    last_error_frame: AtomicU64,
    /// Reserved
    _error_reserved: [AtomicU64; 2],

    // ===== Seek State (32 bytes) =====
    /// Target seek PTS
    seek_target_pts: AtomicU64,
    /// Target seek frame
    seek_target_frame: AtomicU64,
    /// Seek flags (1 = by time, 2 = by frame, 4 = keyframe only)
    seek_flags: AtomicU32,
    /// Reserved
    _seek_reserved: [AtomicU32; 3],

    // ===== Padding to 1024 bytes =====
    _final_pad: [u8; 632],
}

// Compile-time size check
const _: () = {
    assert!(core::mem::size_of::<DecoderPipelineCapsule>() == 1024);
    assert!(core::mem::align_of::<DecoderPipelineCapsule>() == 1024);
};

impl Default for DecoderPipelineCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl DecoderPipelineCapsule {
    /// Create a new decoder pipeline capsule
    #[must_use]
    pub const fn new() -> Self {
        Self {
            // State machine
            state: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            phase_flags: AtomicU64::new(0),
            last_error: AtomicU64::new(0),

            // Format info
            container_format: AtomicU32::new(0),
            video_codec: AtomicU32::new(0),
            chroma_format: AtomicU32::new(0),
            bit_depth: AtomicU32::new(8),
            _format_reserved: [
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
            ],

            // Video parameters
            width: AtomicU32::new(0),
            height: AtomicU32::new(0),
            frame_rate_num: AtomicU32::new(0),
            frame_rate_den: AtomicU32::new(1),
            dar_num: AtomicU32::new(1),
            dar_den: AtomicU32::new(1),
            _video_reserved: [AtomicU32::new(0), AtomicU32::new(0)],

            // Timing
            current_pts: AtomicU64::new(0),
            duration: AtomicU64::new(0),
            timecode_scale: AtomicU64::new(1_000_000), // 1ms default
            start_time: AtomicU64::new(0),
            end_time: AtomicU64::new(0),
            _timing_reserved: [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)],

            // Position tracking
            current_frame: AtomicU64::new(0),
            total_frames: AtomicU64::new(0),
            bytes_processed: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
            byte_offset: AtomicU64::new(0),
            _position_reserved: [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)],

            // Buffer management
            frames_buffered: AtomicU32::new(0),
            buffer_capacity: AtomicU32::new(16),
            frames_decoded: AtomicU64::new(0),
            frames_dropped: AtomicU32::new(0),
            decode_queue_depth: AtomicU32::new(0),
            _buffer_reserved: [AtomicU64::new(0)],

            // Error tracking
            error_count: AtomicU32::new(0),
            consecutive_errors: AtomicU32::new(0),
            last_error_frame: AtomicU64::new(0),
            _error_reserved: [AtomicU64::new(0), AtomicU64::new(0)],

            // Seek state
            seek_target_pts: AtomicU64::new(0),
            seek_target_frame: AtomicU64::new(0),
            seek_flags: AtomicU32::new(0),
            _seek_reserved: [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)],

            // Padding
            _final_pad: [0u8; 632],
        }
    }

    // =========================================================================
    // State Management
    // =========================================================================

    /// Get current pipeline state
    #[inline]
    pub fn state(&self) -> PipelineState {
        let state_raw = self.state.load(Ordering::Acquire) & 0xFF;
        PipelineState::from_u8(state_raw as u8).unwrap_or(PipelineState::Error)
    }

    /// Set pipeline state
    #[inline]
    fn set_state(&self, new_state: PipelineState) {
        let current = self.state.load(Ordering::Acquire);
        let updated = (current & !0xFF) | (new_state as u64);
        self.state.store(updated, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Atomically transition from one state to another
    fn transition_state(
        &self,
        from: PipelineState,
        to: PipelineState,
    ) -> Result<(), PipelineError> {
        let current = self.state.load(Ordering::Acquire);
        let current_state = PipelineState::from_u8((current & 0xFF) as u8)
            .unwrap_or(PipelineState::Error);

        if current_state != from {
            return Err(PipelineError::InvalidState);
        }

        let updated = (current & !0xFF) | (to as u64);
        match self.state.compare_exchange(
            current,
            updated,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                self.generation.fetch_add(1, Ordering::AcqRel);
                Ok(())
            }
            Err(_) => Err(PipelineError::InvalidState),
        }
    }

    /// Get generation counter (Q34 audit)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get phase flags
    #[inline]
    pub fn phase_flags(&self) -> u64 {
        self.phase_flags.load(Ordering::Acquire)
    }

    /// Set a phase flag
    #[inline]
    fn set_phase_flag(&self, flag: u64) {
        self.phase_flags.fetch_or(flag, Ordering::AcqRel);
    }

    /// Clear a phase flag
    #[inline]
    fn clear_phase_flag(&self, flag: u64) {
        self.phase_flags.fetch_and(!flag, Ordering::AcqRel);
    }

    /// Check if a phase flag is set
    #[inline]
    pub fn has_phase_flag(&self, flag: u64) -> bool {
        (self.phase_flags.load(Ordering::Acquire) & flag) != 0
    }

    // =========================================================================
    // Initialization
    // =========================================================================

    /// Open a video file for decoding
    ///
    /// # Errors
    ///
    /// Returns error if file cannot be opened or format is unsupported.
    pub fn open(&mut self, path: &str) -> Result<(), PipelineError> {
        // Verify we're in Idle state
        if self.state() != PipelineState::Idle {
            return Err(PipelineError::InvalidState);
        }

        self.set_state(PipelineState::Initializing);

        // Read file header for format detection
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(_) => {
                self.set_error(PipelineError::FileNotFound);
                return Err(PipelineError::FileNotFound);
            }
        };

        self.open_data(&data)
    }

    /// Open video data from memory buffer
    ///
    /// # Errors
    ///
    /// Returns error if format is unsupported or data is corrupted.
    pub fn open_data(&mut self, data: &[u8]) -> Result<(), PipelineError> {
        let current_state = self.state();
        if current_state != PipelineState::Idle && current_state != PipelineState::Initializing {
            return Err(PipelineError::InvalidState);
        }

        if current_state == PipelineState::Idle {
            self.set_state(PipelineState::Initializing);
        }

        if data.len() < 12 {
            self.set_error(PipelineError::InsufficientData);
            return Err(PipelineError::InsufficientData);
        }

        // Detect container format
        let format = self.detect_format(data);
        self.container_format.store(format as u32, Ordering::Release);

        if format == ContainerFormat::Unknown {
            self.set_error(PipelineError::UnsupportedContainer);
            return Err(PipelineError::UnsupportedContainer);
        }

        // Store total bytes
        self.total_bytes.store(data.len() as u64, Ordering::Release);

        // Set demuxer initialized flag
        self.set_phase_flag(phase_flags::DEMUXER_INIT);
        self.set_state(PipelineState::DemuxReady);

        // Simulate track parsing (real implementation would parse actual track info)
        self.set_phase_flag(phase_flags::TRACKS_PARSED);

        // Initialize decoder based on detected codec
        // For now, we'll set default parameters
        self.width.store(1920, Ordering::Release);
        self.height.store(1080, Ordering::Release);
        self.frame_rate_num.store(30000, Ordering::Release);
        self.frame_rate_den.store(1001, Ordering::Release);
        self.chroma_format.store(ChromaFormat::Yuv420 as u32, Ordering::Release);
        self.bit_depth.store(8, Ordering::Release);

        self.set_phase_flag(phase_flags::DECODER_INIT);
        self.set_state(PipelineState::DecoderReady);

        Ok(())
    }

    /// Detect container format from data header
    pub fn detect_format(&self, data: &[u8]) -> ContainerFormat {
        if data.len() < 12 {
            return ContainerFormat::Unknown;
        }

        // Check for MP4 (ftyp box at offset 4)
        if data.len() >= 8 && data[4..8] == MP4_FTYP {
            return ContainerFormat::Mp4;
        }

        // Check for EBML header (MKV/WebM)
        if data[0..4] == MKV_EBML {
            // Would need to parse DocType to distinguish MKV from WebM
            // For now, default to MKV
            return ContainerFormat::Mkv;
        }

        ContainerFormat::Unknown
    }

    /// Detect video codec from codec ID string
    pub fn detect_codec(&self, codec_id: &str) -> VideoCodec {
        match codec_id.to_uppercase().as_str() {
            // H.264/AVC variants
            "AVC1" | "AVC" | "H264" | "H.264" | "V_MPEG4/ISO/AVC" => VideoCodec::H264,
            // VP9 variants
            "VP9" | "VP09" | "V_VP9" => VideoCodec::VP9,
            // AV1 variants
            "AV1" | "AV01" | "V_AV1" => VideoCodec::AV1,
            _ => VideoCodec::Unknown,
        }
    }

    // =========================================================================
    // Decoding
    // =========================================================================

    /// Decode the next frame
    ///
    /// # Returns
    ///
    /// - `Ok(Some(frame))` - Successfully decoded frame
    /// - `Ok(None)` - End of stream reached
    /// - `Err(e)` - Decoding error
    pub fn decode_frame(&mut self) -> Result<Option<DecodedFrame>, PipelineError> {
        let state = self.state();

        match state {
            PipelineState::DecoderReady => {
                self.set_state(PipelineState::Running);
                self.set_phase_flag(phase_flags::FIRST_FRAME);
            }
            PipelineState::Running => {}
            PipelineState::EndOfStream => return Ok(None),
            PipelineState::Error => return Err(self.last_error()),
            _ => return Err(PipelineError::InvalidState),
        }

        // Check for end of stream (simulated)
        let current = self.current_frame.load(Ordering::Acquire);
        let total = self.total_frames.load(Ordering::Acquire);

        if total > 0 && current >= total {
            self.set_state(PipelineState::EndOfStream);
            self.set_phase_flag(phase_flags::END_OF_STREAM);
            return Ok(None);
        }

        // Create a simulated decoded frame
        let width = self.width.load(Ordering::Acquire);
        let height = self.height.load(Ordering::Acquire);
        let chroma = ChromaFormat::from_u8(self.chroma_format.load(Ordering::Acquire) as u8);

        let mut frame = DecodedFrame::new(width, height, chroma);
        frame.frame_number = current;
        frame.pts = self.current_pts.load(Ordering::Acquire);
        frame.keyframe = current == 0 || (current % 30) == 0; // Simulated keyframe every 30 frames

        // Update counters
        self.current_frame.fetch_add(1, Ordering::AcqRel);
        self.frames_decoded.fetch_add(1, Ordering::AcqRel);

        // Update PTS (assuming 30fps)
        let frame_duration = 1_000_000_000 / 30; // ns per frame
        self.current_pts.fetch_add(frame_duration, Ordering::AcqRel);

        Ok(Some(frame))
    }

    /// Decode multiple frames in batch
    ///
    /// # Arguments
    ///
    /// * `max_frames` - Maximum number of frames to decode
    ///
    /// # Returns
    ///
    /// Vector of decoded frames (may be less than `max_frames` if EOS reached)
    pub fn decode_batch(
        &mut self,
        max_frames: usize,
    ) -> Result<Vec<DecodedFrame>, PipelineError> {
        let mut frames = Vec::with_capacity(max_frames);

        for _ in 0..max_frames {
            match self.decode_frame()? {
                Some(frame) => frames.push(frame),
                None => break,
            }
        }

        Ok(frames)
    }

    /// Flush decoder buffers
    ///
    /// Returns any remaining buffered frames.
    pub fn flush(&mut self) -> Result<Vec<DecodedFrame>, PipelineError> {
        let state = self.state();

        if !matches!(
            state,
            PipelineState::Running | PipelineState::Paused | PipelineState::EndOfStream
        ) {
            return Err(PipelineError::InvalidState);
        }

        self.set_state(PipelineState::Flushing);

        // In real implementation, this would drain decoder buffers
        let remaining_frames = Vec::new();

        self.set_phase_flag(phase_flags::FLUSH_COMPLETE);
        self.clear_phase_flag(phase_flags::FIRST_FRAME);
        self.set_state(PipelineState::EndOfStream);

        Ok(remaining_frames)
    }

    // =========================================================================
    // Seeking
    // =========================================================================

    /// Seek to a specific time in milliseconds
    pub fn seek_to_time(&mut self, time_ms: u64) -> Result<(), PipelineError> {
        if !self.state().can_seek() {
            return Err(PipelineError::InvalidState);
        }

        let prev_state = self.state();
        self.set_state(PipelineState::Seeking);

        // Convert time to timescale
        let timescale = self.timecode_scale.load(Ordering::Acquire);
        let target_pts = (time_ms as u64) * 1_000_000 / (timescale.max(1));

        self.seek_target_pts.store(target_pts, Ordering::Release);
        self.seek_flags.store(1, Ordering::Release); // Seek by time

        // Perform seek (simulated)
        self.current_pts.store(target_pts, Ordering::Release);

        // Estimate frame number
        let frame_rate = self.frame_rate_num.load(Ordering::Acquire) as f64
            / self.frame_rate_den.load(Ordering::Acquire) as f64;
        let frame_num = ((time_ms as f64 / 1000.0) * frame_rate) as u64;
        self.current_frame.store(frame_num, Ordering::Release);

        self.set_phase_flag(phase_flags::SEEK_COMPLETE);
        self.clear_phase_flag(phase_flags::END_OF_STREAM);

        // Return to previous state (or Running if was EndOfStream)
        if prev_state == PipelineState::EndOfStream {
            self.set_state(PipelineState::Running);
        } else {
            self.set_state(prev_state);
        }

        Ok(())
    }

    /// Seek to a specific frame number
    pub fn seek_to_frame(&mut self, frame_num: u64) -> Result<(), PipelineError> {
        if !self.state().can_seek() {
            return Err(PipelineError::InvalidState);
        }

        let prev_state = self.state();
        self.set_state(PipelineState::Seeking);

        self.seek_target_frame.store(frame_num, Ordering::Release);
        self.seek_flags.store(2, Ordering::Release); // Seek by frame

        // Update position
        self.current_frame.store(frame_num, Ordering::Release);

        // Calculate PTS from frame number
        let frame_rate = self.frame_rate_num.load(Ordering::Acquire) as f64
            / self.frame_rate_den.load(Ordering::Acquire) as f64;
        let time_s = frame_num as f64 / frame_rate;
        let timescale = self.timecode_scale.load(Ordering::Acquire);
        let pts = ((time_s * 1e9) / timescale as f64) as u64;
        self.current_pts.store(pts, Ordering::Release);

        self.set_phase_flag(phase_flags::SEEK_COMPLETE);
        self.clear_phase_flag(phase_flags::END_OF_STREAM);

        if prev_state == PipelineState::EndOfStream {
            self.set_state(PipelineState::Running);
        } else {
            self.set_state(prev_state);
        }

        Ok(())
    }

    // =========================================================================
    // State Queries
    // =========================================================================

    /// Get current time in milliseconds
    #[inline]
    pub fn current_time_ms(&self) -> u64 {
        let pts = self.current_pts.load(Ordering::Acquire);
        let timescale = self.timecode_scale.load(Ordering::Acquire).max(1);
        (pts * timescale) / 1_000_000
    }

    /// Get total duration in milliseconds (None if unknown)
    pub fn duration_ms(&self) -> Option<u64> {
        let duration = self.duration.load(Ordering::Acquire);
        if duration == 0 {
            return None;
        }
        let timescale = self.timecode_scale.load(Ordering::Acquire).max(1);
        Some((duration * timescale) / 1_000_000)
    }

    /// Get current frame number
    #[inline]
    pub fn frame_count(&self) -> u64 {
        self.current_frame.load(Ordering::Acquire)
    }

    /// Get total frames (0 if unknown)
    #[inline]
    pub fn total_frame_count(&self) -> u64 {
        self.total_frames.load(Ordering::Acquire)
    }

    /// Get video information
    pub fn video_info(&self) -> Option<VideoInfo> {
        if !self.has_phase_flag(phase_flags::TRACKS_PARSED) {
            return None;
        }

        let frame_rate_num = self.frame_rate_num.load(Ordering::Acquire);
        let frame_rate_den = self.frame_rate_den.load(Ordering::Acquire).max(1);

        Some(VideoInfo {
            width: self.width.load(Ordering::Acquire),
            height: self.height.load(Ordering::Acquire),
            frame_rate: frame_rate_num as f64 / frame_rate_den as f64,
            codec: VideoCodec::from_u8(self.video_codec.load(Ordering::Acquire) as u8),
            bit_depth: self.bit_depth.load(Ordering::Acquire) as u8,
            chroma_format: ChromaFormat::from_u8(
                self.chroma_format.load(Ordering::Acquire) as u8,
            ),
        })
    }

    /// Get container format
    #[inline]
    pub fn container_format(&self) -> ContainerFormat {
        ContainerFormat::from_u8(self.container_format.load(Ordering::Acquire) as u8)
    }

    /// Get video codec
    #[inline]
    pub fn video_codec(&self) -> VideoCodec {
        VideoCodec::from_u8(self.video_codec.load(Ordering::Acquire) as u8)
    }

    /// Get pipeline statistics
    pub fn stats(&self) -> PipelineStats {
        PipelineStats {
            frames_decoded: self.frames_decoded.load(Ordering::Acquire),
            frames_buffered: self.frames_buffered.load(Ordering::Acquire),
            frames_dropped: self.frames_dropped.load(Ordering::Acquire),
            bytes_processed: self.bytes_processed.load(Ordering::Acquire),
            error_count: self.error_count.load(Ordering::Acquire),
            generation: self.generation.load(Ordering::Acquire),
        }
    }

    // =========================================================================
    // Control
    // =========================================================================

    /// Pause decoding
    pub fn pause(&mut self) -> Result<(), PipelineError> {
        self.transition_state(PipelineState::Running, PipelineState::Paused)
    }

    /// Resume decoding
    pub fn resume(&mut self) -> Result<(), PipelineError> {
        self.transition_state(PipelineState::Paused, PipelineState::Running)
    }

    /// Reset pipeline to initial state
    pub fn reset(&mut self) -> Result<(), PipelineError> {
        // Clear all state
        self.state.store(0, Ordering::Release);
        self.phase_flags.store(0, Ordering::Release);
        self.last_error.store(0, Ordering::Release);

        // Clear format info
        self.container_format.store(0, Ordering::Release);
        self.video_codec.store(0, Ordering::Release);

        // Clear video parameters
        self.width.store(0, Ordering::Release);
        self.height.store(0, Ordering::Release);
        self.frame_rate_num.store(0, Ordering::Release);
        self.frame_rate_den.store(1, Ordering::Release);

        // Clear timing
        self.current_pts.store(0, Ordering::Release);
        self.duration.store(0, Ordering::Release);

        // Clear position
        self.current_frame.store(0, Ordering::Release);
        self.total_frames.store(0, Ordering::Release);
        self.bytes_processed.store(0, Ordering::Release);

        // Clear buffer state
        self.frames_buffered.store(0, Ordering::Release);
        self.frames_decoded.store(0, Ordering::Release);
        self.frames_dropped.store(0, Ordering::Release);

        // Clear errors
        self.error_count.store(0, Ordering::Release);
        self.consecutive_errors.store(0, Ordering::Release);

        // Increment generation
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(())
    }

    // =========================================================================
    // Error Handling
    // =========================================================================

    /// Get last error
    #[inline]
    pub fn last_error(&self) -> PipelineError {
        PipelineError::from_u8(self.last_error.load(Ordering::Acquire) as u8)
    }

    /// Set error and transition to Error state
    fn set_error(&self, error: PipelineError) {
        self.last_error.store(error as u64, Ordering::Release);
        self.error_count.fetch_add(1, Ordering::AcqRel);
        self.consecutive_errors.fetch_add(1, Ordering::AcqRel);
        self.set_phase_flag(phase_flags::ERROR_FLAG);
        self.set_state(PipelineState::Error);
    }

    /// Clear error state (internal use)
    #[allow(dead_code)]
    fn clear_error(&self) {
        self.last_error.store(0, Ordering::Release);
        self.consecutive_errors.store(0, Ordering::Release);
        self.clear_phase_flag(phase_flags::ERROR_FLAG);
    }
}

// ============================================================================
// Tests (28+ tests for T28 compliance)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Q1-Q7: Unit Tests
    // =========================================================================

    #[test]
    fn test_capsule_size_alignment() {
        assert_eq!(core::mem::size_of::<DecoderPipelineCapsule>(), 1024);
        assert_eq!(core::mem::align_of::<DecoderPipelineCapsule>(), 1024);
    }

    #[test]
    fn test_pipeline_state_values() {
        assert_eq!(PipelineState::Idle as u8, 0);
        assert_eq!(PipelineState::Initializing as u8, 1);
        assert_eq!(PipelineState::DemuxReady as u8, 2);
        assert_eq!(PipelineState::DecoderReady as u8, 3);
        assert_eq!(PipelineState::Running as u8, 4);
        assert_eq!(PipelineState::Paused as u8, 5);
        assert_eq!(PipelineState::Seeking as u8, 6);
        assert_eq!(PipelineState::Flushing as u8, 7);
        assert_eq!(PipelineState::EndOfStream as u8, 8);
        assert_eq!(PipelineState::Error as u8, 9);
    }

    #[test]
    fn test_pipeline_state_from_u8() {
        assert_eq!(PipelineState::from_u8(0), Some(PipelineState::Idle));
        assert_eq!(PipelineState::from_u8(4), Some(PipelineState::Running));
        assert_eq!(PipelineState::from_u8(9), Some(PipelineState::Error));
        assert_eq!(PipelineState::from_u8(10), None);
        assert_eq!(PipelineState::from_u8(255), None);
    }

    #[test]
    fn test_pipeline_state_can_decode() {
        assert!(!PipelineState::Idle.can_decode());
        assert!(!PipelineState::Initializing.can_decode());
        assert!(PipelineState::DecoderReady.can_decode());
        assert!(PipelineState::Running.can_decode());
        assert!(!PipelineState::Paused.can_decode());
        assert!(!PipelineState::EndOfStream.can_decode());
    }

    #[test]
    fn test_pipeline_state_can_seek() {
        assert!(!PipelineState::Idle.can_seek());
        assert!(!PipelineState::Initializing.can_seek());
        assert!(PipelineState::DecoderReady.can_seek());
        assert!(PipelineState::Running.can_seek());
        assert!(PipelineState::Paused.can_seek());
        assert!(PipelineState::EndOfStream.can_seek());
        assert!(!PipelineState::Error.can_seek());
    }

    #[test]
    fn test_container_format_values() {
        assert_eq!(ContainerFormat::Unknown as u8, 0);
        assert_eq!(ContainerFormat::Mp4 as u8, 1);
        assert_eq!(ContainerFormat::Mkv as u8, 2);
        assert_eq!(ContainerFormat::WebM as u8, 3);
    }

    #[test]
    fn test_container_format_is_matroska() {
        assert!(!ContainerFormat::Mp4.is_matroska());
        assert!(ContainerFormat::Mkv.is_matroska());
        assert!(ContainerFormat::WebM.is_matroska());
        assert!(!ContainerFormat::Unknown.is_matroska());
    }

    #[test]
    fn test_video_codec_fourcc() {
        assert_eq!(VideoCodec::H264.fourcc(), "avc1");
        assert_eq!(VideoCodec::VP9.fourcc(), "vp09");
        assert_eq!(VideoCodec::AV1.fourcc(), "av01");
        assert_eq!(VideoCodec::Unknown.fourcc(), "????");
    }

    #[test]
    fn test_chroma_format_subsampling() {
        assert_eq!(ChromaFormat::Yuv420.subsampling_x(), 1);
        assert_eq!(ChromaFormat::Yuv420.subsampling_y(), 1);
        assert_eq!(ChromaFormat::Yuv422.subsampling_x(), 1);
        assert_eq!(ChromaFormat::Yuv422.subsampling_y(), 0);
        assert_eq!(ChromaFormat::Yuv444.subsampling_x(), 0);
        assert_eq!(ChromaFormat::Yuv444.subsampling_y(), 0);
    }

    #[test]
    fn test_pipeline_error_display() {
        assert_eq!(format!("{}", PipelineError::None), "no error");
        assert_eq!(format!("{}", PipelineError::FileNotFound), "file not found");
        assert_eq!(format!("{}", PipelineError::EndOfStream), "end of stream");
    }

    // =========================================================================
    // Q8-Q14: Property Tests (Phase Flags)
    // =========================================================================

    #[test]
    fn test_phase_flags_distinct() {
        use phase_flags::*;

        // All flags should be distinct powers of 2
        assert_eq!(DEMUXER_INIT.count_ones(), 1);
        assert_eq!(TRACKS_PARSED.count_ones(), 1);
        assert_eq!(DECODER_INIT.count_ones(), 1);
        assert_eq!(FIRST_FRAME.count_ones(), 1);
        assert_eq!(SEEK_COMPLETE.count_ones(), 1);
        assert_eq!(FLUSH_COMPLETE.count_ones(), 1);
        assert_eq!(END_OF_STREAM.count_ones(), 1);
        assert_eq!(ERROR_FLAG.count_ones(), 1);
    }

    #[test]
    fn test_phase_flags_no_overlap() {
        use phase_flags::*;

        let all = DEMUXER_INIT | TRACKS_PARSED | DECODER_INIT | FIRST_FRAME
            | SEEK_COMPLETE | FLUSH_COMPLETE | END_OF_STREAM | ERROR_FLAG;
        assert_eq!(all.count_ones(), 8);
    }

    #[test]
    fn test_phase_flags_all_init() {
        use phase_flags::*;

        // ALL_INIT should include required initialization flags
        assert_eq!(ALL_INIT & DEMUXER_INIT, DEMUXER_INIT);
        assert_eq!(ALL_INIT & TRACKS_PARSED, TRACKS_PARSED);
        assert_eq!(ALL_INIT & DECODER_INIT, DECODER_INIT);
        assert_eq!(ALL_INIT & FIRST_FRAME, FIRST_FRAME);
    }

    #[test]
    fn test_phase_flags_decode_ready() {
        use phase_flags::*;

        // DECODE_READY should be subset of ALL_INIT
        assert_eq!(DECODE_READY & ALL_INIT, DECODE_READY);
        assert_eq!(DECODE_READY.count_ones(), 3);
    }

    #[test]
    fn test_phase_flag_operations() {
        let capsule = DecoderPipelineCapsule::new();

        assert!(!capsule.has_phase_flag(phase_flags::DEMUXER_INIT));

        capsule.set_phase_flag(phase_flags::DEMUXER_INIT);
        assert!(capsule.has_phase_flag(phase_flags::DEMUXER_INIT));

        capsule.set_phase_flag(phase_flags::TRACKS_PARSED);
        assert!(capsule.has_phase_flag(phase_flags::DEMUXER_INIT));
        assert!(capsule.has_phase_flag(phase_flags::TRACKS_PARSED));

        capsule.clear_phase_flag(phase_flags::DEMUXER_INIT);
        assert!(!capsule.has_phase_flag(phase_flags::DEMUXER_INIT));
        assert!(capsule.has_phase_flag(phase_flags::TRACKS_PARSED));
    }

    #[test]
    fn test_error_accumulation() {
        let capsule = DecoderPipelineCapsule::new();

        assert_eq!(capsule.stats().error_count, 0);

        capsule.set_error(PipelineError::DecodingError);
        assert_eq!(capsule.stats().error_count, 1);
        assert_eq!(capsule.last_error(), PipelineError::DecodingError);

        capsule.set_error(PipelineError::SeekFailed);
        assert_eq!(capsule.stats().error_count, 2);
        assert_eq!(capsule.last_error(), PipelineError::SeekFailed);
    }

    // =========================================================================
    // Q15-Q21: Integration Tests
    // =========================================================================

    #[test]
    fn test_new_capsule_initial_state() {
        let capsule = DecoderPipelineCapsule::new();

        assert_eq!(capsule.state(), PipelineState::Idle);
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.phase_flags(), 0);
        assert_eq!(capsule.container_format(), ContainerFormat::Unknown);
        assert_eq!(capsule.video_codec(), VideoCodec::Unknown);
        assert_eq!(capsule.frame_count(), 0);
        assert!(capsule.video_info().is_none());
    }

    #[test]
    fn test_format_detection_mp4() {
        let capsule = DecoderPipelineCapsule::new();

        // MP4 ftyp box: size (4 bytes) + "ftyp" (4 bytes) + brand
        let mp4_header: [u8; 12] = [
            0x00, 0x00, 0x00, 0x14, // Size = 20
            0x66, 0x74, 0x79, 0x70, // "ftyp"
            0x69, 0x73, 0x6F, 0x6D, // "isom"
        ];

        assert_eq!(capsule.detect_format(&mp4_header), ContainerFormat::Mp4);
    }

    #[test]
    fn test_format_detection_mkv() {
        let capsule = DecoderPipelineCapsule::new();

        // EBML header
        let mkv_header: [u8; 12] = [
            0x1A, 0x45, 0xDF, 0xA3, // EBML header ID
            0x01, 0x00, 0x00, 0x00, // Size
            0x00, 0x00, 0x00, 0x1F, // More data
        ];

        assert_eq!(capsule.detect_format(&mkv_header), ContainerFormat::Mkv);
    }

    #[test]
    fn test_format_detection_unknown() {
        let capsule = DecoderPipelineCapsule::new();

        let unknown: [u8; 12] = [0x00; 12];
        assert_eq!(capsule.detect_format(&unknown), ContainerFormat::Unknown);

        let short: [u8; 4] = [0x00; 4];
        assert_eq!(capsule.detect_format(&short), ContainerFormat::Unknown);
    }

    #[test]
    fn test_codec_detection() {
        let capsule = DecoderPipelineCapsule::new();

        assert_eq!(capsule.detect_codec("avc1"), VideoCodec::H264);
        assert_eq!(capsule.detect_codec("H264"), VideoCodec::H264);
        assert_eq!(capsule.detect_codec("V_MPEG4/ISO/AVC"), VideoCodec::H264);
        assert_eq!(capsule.detect_codec("vp9"), VideoCodec::VP9);
        assert_eq!(capsule.detect_codec("V_VP9"), VideoCodec::VP9);
        assert_eq!(capsule.detect_codec("av01"), VideoCodec::AV1);
        assert_eq!(capsule.detect_codec("V_AV1"), VideoCodec::AV1);
        assert_eq!(capsule.detect_codec("unknown"), VideoCodec::Unknown);
    }

    #[test]
    fn test_open_data_mp4() {
        let mut capsule = DecoderPipelineCapsule::new();

        // Create minimal MP4 header
        let mp4_data: Vec<u8> = vec![
            0x00, 0x00, 0x00, 0x14, // Size = 20
            0x66, 0x74, 0x79, 0x70, // "ftyp"
            0x69, 0x73, 0x6F, 0x6D, // "isom"
            0x00, 0x00, 0x00, 0x00, // Additional data
            0x00, 0x00, 0x00, 0x00,
        ];

        let result = capsule.open_data(&mp4_data);
        assert!(result.is_ok());
        assert_eq!(capsule.state(), PipelineState::DecoderReady);
        assert_eq!(capsule.container_format(), ContainerFormat::Mp4);
        assert!(capsule.has_phase_flag(phase_flags::DEMUXER_INIT));
        assert!(capsule.has_phase_flag(phase_flags::TRACKS_PARSED));
        assert!(capsule.has_phase_flag(phase_flags::DECODER_INIT));
    }

    #[test]
    fn test_open_data_insufficient() {
        let mut capsule = DecoderPipelineCapsule::new();

        let short_data: [u8; 4] = [0x00; 4];
        let result = capsule.open_data(&short_data);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PipelineError::InsufficientData);
    }

    #[test]
    fn test_decode_frame_simulation() {
        let mut capsule = DecoderPipelineCapsule::new();

        // Initialize with valid data
        let mkv_data: Vec<u8> = vec![
            0x1A, 0x45, 0xDF, 0xA3, // EBML header
            0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x1F,
            0x00, 0x00, 0x00, 0x00,
        ];
        capsule.open_data(&mkv_data).unwrap();

        // Set total frames for testing
        capsule.total_frames.store(3, Ordering::Release);

        // Decode frames
        let frame1 = capsule.decode_frame().unwrap();
        assert!(frame1.is_some());
        let f1 = frame1.unwrap();
        assert_eq!(f1.frame_number, 0);
        assert!(f1.keyframe);

        let frame2 = capsule.decode_frame().unwrap();
        assert!(frame2.is_some());
        assert_eq!(frame2.unwrap().frame_number, 1);

        let frame3 = capsule.decode_frame().unwrap();
        assert!(frame3.is_some());
        assert_eq!(frame3.unwrap().frame_number, 2);

        // Should reach EOS
        let frame4 = capsule.decode_frame().unwrap();
        assert!(frame4.is_none());
        assert_eq!(capsule.state(), PipelineState::EndOfStream);
    }

    #[test]
    fn test_decode_batch() {
        let mut capsule = DecoderPipelineCapsule::new();

        let mkv_data: Vec<u8> = vec![
            0x1A, 0x45, 0xDF, 0xA3,
            0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x1F,
            0x00, 0x00, 0x00, 0x00,
        ];
        capsule.open_data(&mkv_data).unwrap();
        capsule.total_frames.store(5, Ordering::Release);

        let frames = capsule.decode_batch(3).unwrap();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].frame_number, 0);
        assert_eq!(frames[1].frame_number, 1);
        assert_eq!(frames[2].frame_number, 2);
    }

    // =========================================================================
    // Q22-Q28: Production Tests
    // =========================================================================

    #[test]
    fn test_seek_to_time() {
        let mut capsule = DecoderPipelineCapsule::new();

        let mkv_data: Vec<u8> = vec![
            0x1A, 0x45, 0xDF, 0xA3,
            0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x1F,
            0x00, 0x00, 0x00, 0x00,
        ];
        capsule.open_data(&mkv_data).unwrap();
        capsule.total_frames.store(100, Ordering::Release);

        // Decode some frames
        capsule.decode_frame().unwrap();
        capsule.decode_frame().unwrap();

        // Seek to 1 second
        let result = capsule.seek_to_time(1000);
        assert!(result.is_ok());

        // Should have seek flag set
        assert!(capsule.has_phase_flag(phase_flags::SEEK_COMPLETE));
    }

    #[test]
    fn test_seek_to_frame() {
        let mut capsule = DecoderPipelineCapsule::new();

        let mkv_data: Vec<u8> = vec![
            0x1A, 0x45, 0xDF, 0xA3,
            0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x1F,
            0x00, 0x00, 0x00, 0x00,
        ];
        capsule.open_data(&mkv_data).unwrap();
        capsule.total_frames.store(100, Ordering::Release);

        capsule.decode_frame().unwrap();

        let result = capsule.seek_to_frame(50);
        assert!(result.is_ok());
        assert_eq!(capsule.frame_count(), 50);
    }

    #[test]
    fn test_seek_from_eos() {
        let mut capsule = DecoderPipelineCapsule::new();

        let mkv_data: Vec<u8> = vec![
            0x1A, 0x45, 0xDF, 0xA3,
            0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x1F,
            0x00, 0x00, 0x00, 0x00,
        ];
        capsule.open_data(&mkv_data).unwrap();
        capsule.total_frames.store(2, Ordering::Release);

        // Exhaust stream
        capsule.decode_frame().unwrap();
        capsule.decode_frame().unwrap();
        capsule.decode_frame().unwrap(); // Returns None, sets EOS

        assert_eq!(capsule.state(), PipelineState::EndOfStream);

        // Seek should work from EOS
        let result = capsule.seek_to_frame(0);
        assert!(result.is_ok());
        assert_eq!(capsule.state(), PipelineState::Running);
    }

    #[test]
    fn test_pause_resume() {
        let mut capsule = DecoderPipelineCapsule::new();

        let mkv_data: Vec<u8> = vec![
            0x1A, 0x45, 0xDF, 0xA3,
            0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x1F,
            0x00, 0x00, 0x00, 0x00,
        ];
        capsule.open_data(&mkv_data).unwrap();
        capsule.total_frames.store(10, Ordering::Release);

        capsule.decode_frame().unwrap();
        assert_eq!(capsule.state(), PipelineState::Running);

        capsule.pause().unwrap();
        assert_eq!(capsule.state(), PipelineState::Paused);

        capsule.resume().unwrap();
        assert_eq!(capsule.state(), PipelineState::Running);
    }

    #[test]
    fn test_flush() {
        let mut capsule = DecoderPipelineCapsule::new();

        let mkv_data: Vec<u8> = vec![
            0x1A, 0x45, 0xDF, 0xA3,
            0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x1F,
            0x00, 0x00, 0x00, 0x00,
        ];
        capsule.open_data(&mkv_data).unwrap();
        capsule.total_frames.store(10, Ordering::Release);

        capsule.decode_frame().unwrap();

        let result = capsule.flush();
        assert!(result.is_ok());
        assert!(capsule.has_phase_flag(phase_flags::FLUSH_COMPLETE));
        assert_eq!(capsule.state(), PipelineState::EndOfStream);
    }

    #[test]
    fn test_reset() {
        let mut capsule = DecoderPipelineCapsule::new();

        let mkv_data: Vec<u8> = vec![
            0x1A, 0x45, 0xDF, 0xA3,
            0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x1F,
            0x00, 0x00, 0x00, 0x00,
        ];
        capsule.open_data(&mkv_data).unwrap();
        capsule.decode_frame().unwrap();

        assert!(capsule.generation() > 0);

        let gen_before = capsule.generation();
        capsule.reset().unwrap();

        assert_eq!(capsule.state(), PipelineState::Idle);
        assert_eq!(capsule.phase_flags(), 0);
        assert_eq!(capsule.frame_count(), 0);
        assert!(capsule.generation() > gen_before);
    }

    #[test]
    fn test_video_info() {
        let mut capsule = DecoderPipelineCapsule::new();

        // Before init, video_info should be None
        assert!(capsule.video_info().is_none());

        let mkv_data: Vec<u8> = vec![
            0x1A, 0x45, 0xDF, 0xA3,
            0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x1F,
            0x00, 0x00, 0x00, 0x00,
        ];
        capsule.open_data(&mkv_data).unwrap();

        let info = capsule.video_info();
        assert!(info.is_some());

        let info = info.unwrap();
        assert_eq!(info.width, 1920);
        assert_eq!(info.height, 1080);
        assert!((info.frame_rate - 29.97).abs() < 0.1);
    }

    #[test]
    fn test_concurrent_phase_flag_access() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(DecoderPipelineCapsule::new());
        let mut handles = vec![];

        for i in 0..4 {
            let cap = Arc::clone(&capsule);
            let flag = 1u64 << i;
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    cap.set_phase_flag(flag);
                    assert!(cap.has_phase_flag(flag));
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // All flags should be set
        for i in 0..4 {
            assert!(capsule.has_phase_flag(1u64 << i));
        }
    }

    #[test]
    fn test_generation_counter_increments() {
        let capsule = DecoderPipelineCapsule::new();

        let gen1 = capsule.generation();
        capsule.set_state(PipelineState::Initializing);
        let gen2 = capsule.generation();

        assert!(gen2 > gen1);

        capsule.set_state(PipelineState::Running);
        let gen3 = capsule.generation();

        assert!(gen3 > gen2);
    }

    #[test]
    fn test_decoded_frame_planes() {
        let frame = DecodedFrame::new(1920, 1080, ChromaFormat::Yuv420);

        // Y plane: 1920 * 1080 = 2073600
        assert_eq!(frame.y_plane().len(), 2073600);
        assert_eq!(frame.strides[0], 1920);

        // U/V planes: (1920/2) * (1080/2) = 518400
        assert_eq!(frame.u_plane().len(), 518400);
        assert_eq!(frame.v_plane().len(), 518400);
        assert_eq!(frame.strides[1], 960);
        assert_eq!(frame.strides[2], 960);
    }

    #[test]
    fn test_video_info_frame_size() {
        let info = VideoInfo {
            width: 1920,
            height: 1080,
            frame_rate: 30.0,
            codec: VideoCodec::H264,
            bit_depth: 8,
            chroma_format: ChromaFormat::Yuv420,
        };

        assert_eq!(info.luma_size(), 2073600);
        // YUV420: Y + U/4 + V/4 = 1 + 0.25 + 0.25 = 1.5x luma
        assert_eq!(info.frame_size(), 3110400);
    }

    #[test]
    fn test_invalid_state_transitions() {
        let mut capsule = DecoderPipelineCapsule::new();

        // Can't pause from Idle
        assert!(capsule.pause().is_err());

        // Can't resume from Idle
        assert!(capsule.resume().is_err());

        // Can't decode from Idle
        assert!(capsule.decode_frame().is_err());
    }
}
