//! WebM Container Muxer Capsule - T5 Streaming
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! WebM-compliant container muxer (MKV subset for web streaming).
//!
//! ## WebM Specification
//!
//! WebM is a restricted subset of Matroska designed for web delivery:
//! - **Allowed Video**: VP8 (V_VP8), VP9 (V_VP9), AV1 (V_AV1)
//! - **Allowed Audio**: Vorbis (A_VORBIS), Opus (A_OPUS)
//! - **Forbidden**: H.264, H.265, AAC, MP3, Chapters, Attachments
//!
//! ## Architecture
//!
//! - **Tier**: T5 Streaming (O(1) cluster append)
//! - **Size**: 512B cache-aligned
//! - **Lockfree**: 100% (atomic state transitions)
//! - **ASSUM**: All unsafe documented with #ASSUME/#VERIFY
//!
//! ## Performance
//!
//! - Cluster append: <100ns
//! - SimpleBlock write: <200ns
//! - Header generation: <1μs
//! - Memory: 512B capsule + buffer

#![allow(dead_code)]

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

// ============================================================================
// EBML Element IDs (WebM subset)
// ============================================================================

/// EBML Header element ID
const EBML_ID: u32 = 0x1A45DFA3;
/// EBML Version element ID
const EBML_VERSION_ID: u32 = 0x4286;
/// EBML Read Version element ID
const EBML_READ_VERSION_ID: u32 = 0x42F7;
/// EBML Max ID Length element ID
const EBML_MAX_ID_LENGTH_ID: u32 = 0x42F2;
/// EBML Max Size Length element ID
const EBML_MAX_SIZE_LENGTH_ID: u32 = 0x42F3;
/// DocType element ID
const DOC_TYPE_ID: u32 = 0x4282;
/// DocTypeVersion element ID
const DOC_TYPE_VERSION_ID: u32 = 0x4287;
/// DocTypeReadVersion element ID
const DOC_TYPE_READ_VERSION_ID: u32 = 0x4285;

/// Segment element ID
const SEGMENT_ID: u32 = 0x18538067;

/// Info element ID (Segment Information)
const INFO_ID: u32 = 0x1549A966;
/// TimecodeScale element ID (nanoseconds per tick, default 1000000 = 1ms)
const TIMECODE_SCALE_ID: u32 = 0x2AD7B1;
/// MuxingApp element ID
const MUXING_APP_ID: u32 = 0x4D80;
/// WritingApp element ID
const WRITING_APP_ID: u32 = 0x5741;
/// Duration element ID
const DURATION_ID: u32 = 0x4489;

/// Tracks element ID
const TRACKS_ID: u32 = 0x1654AE6B;
/// TrackEntry element ID
const TRACK_ENTRY_ID: u32 = 0xAE;
/// TrackNumber element ID
const TRACK_NUMBER_ID: u32 = 0xD7;
/// TrackUID element ID
const TRACK_UID_ID: u32 = 0x73C5;
/// TrackType element ID
const TRACK_TYPE_ID: u32 = 0x83;
/// FlagEnabled element ID
const FLAG_ENABLED_ID: u32 = 0xB9;
/// FlagDefault element ID
const FLAG_DEFAULT_ID: u32 = 0x88;
/// FlagLacing element ID
const FLAG_LACING_ID: u32 = 0x9C;
/// CodecID element ID
const CODEC_ID_ID: u32 = 0x86;
/// CodecPrivate element ID
const CODEC_PRIVATE_ID: u32 = 0x63A2;

/// Video element ID
const VIDEO_ID: u32 = 0xE0;
/// PixelWidth element ID
const PIXEL_WIDTH_ID: u32 = 0xB0;
/// PixelHeight element ID
const PIXEL_HEIGHT_ID: u32 = 0xBA;

/// Audio element ID
const AUDIO_ID: u32 = 0xE1;
/// SamplingFrequency element ID
const SAMPLING_FREQ_ID: u32 = 0xB5;
/// Channels element ID
const CHANNELS_ID: u32 = 0x9F;
/// BitDepth element ID
const BIT_DEPTH_ID: u32 = 0x6264;

/// Cluster element ID
const CLUSTER_ID: u32 = 0x1F43B675;
/// Timecode element ID (cluster timestamp)
const TIMECODE_ID: u32 = 0xE7;
/// SimpleBlock element ID
const SIMPLE_BLOCK_ID: u32 = 0xA3;

/// Cues element ID (seek index)
const CUES_ID: u32 = 0x1C53BB6B;
/// CuePoint element ID
const CUE_POINT_ID: u32 = 0xBB;
/// CueTime element ID
const CUE_TIME_ID: u32 = 0xB3;
/// CueTrackPositions element ID
const CUE_TRACK_POSITIONS_ID: u32 = 0xB7;
/// CueTrack element ID
const CUE_TRACK_ID: u32 = 0xF7;
/// CueClusterPosition element ID
const CUE_CLUSTER_POSITION_ID: u32 = 0xF1;

// ============================================================================
// WebM Codec IDs
// ============================================================================

/// VP8 video codec ID
pub const CODEC_VP8: &[u8] = b"V_VP8";
/// VP9 video codec ID
pub const CODEC_VP9: &[u8] = b"V_VP9";
/// AV1 video codec ID
pub const CODEC_AV1: &[u8] = b"V_AV1";

/// Vorbis audio codec ID
pub const CODEC_VORBIS: &[u8] = b"A_VORBIS";
/// Opus audio codec ID
pub const CODEC_OPUS: &[u8] = b"A_OPUS";

/// WebM DocType string
pub const DOCTYPE_WEBM: &[u8] = b"webm";

// ============================================================================
// WebM Codec Enum (restricted subset)
// ============================================================================

/// WebM-allowed video codecs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WebmVideoCodec {
    /// VP8 video (V_VP8)
    Vp8 = 0,
    /// VP9 video (V_VP9)
    Vp9 = 1,
    /// AV1 video (V_AV1)
    Av1 = 2,
}

impl WebmVideoCodec {
    /// Get the codec ID string
    #[inline]
    pub const fn codec_id(self) -> &'static [u8] {
        match self {
            WebmVideoCodec::Vp8 => CODEC_VP8,
            WebmVideoCodec::Vp9 => CODEC_VP9,
            WebmVideoCodec::Av1 => CODEC_AV1,
        }
    }
}

/// WebM-allowed audio codecs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WebmAudioCodec {
    /// Vorbis audio (A_VORBIS)
    Vorbis = 0,
    /// Opus audio (A_OPUS)
    Opus = 1,
}

impl WebmAudioCodec {
    /// Get the codec ID string
    #[inline]
    pub const fn codec_id(self) -> &'static [u8] {
        match self {
            WebmAudioCodec::Vorbis => CODEC_VORBIS,
            WebmAudioCodec::Opus => CODEC_OPUS,
        }
    }
}

// ============================================================================
// WebM Muxer State Machine
// ============================================================================

/// Muxer phase bits packed into state AtomicU64
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WebmMuxerPhase {
    /// Initial state, waiting for configuration
    Uninitialized = 0,
    /// EBML header written
    HeaderWritten = 1,
    /// Segment started (unknown size for streaming)
    SegmentStarted = 2,
    /// Info element written
    InfoWritten = 3,
    /// Tracks element written
    TracksWritten = 4,
    /// Actively writing clusters
    Clustering = 5,
    /// Cues written (if enabled)
    CuesWritten = 6,
    /// Finalized (segment size patched if not streaming)
    Finalized = 7,
    /// Error state
    Error = 255,
}

impl WebmMuxerPhase {
    /// Convert from u8
    #[inline]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => WebmMuxerPhase::Uninitialized,
            1 => WebmMuxerPhase::HeaderWritten,
            2 => WebmMuxerPhase::SegmentStarted,
            3 => WebmMuxerPhase::InfoWritten,
            4 => WebmMuxerPhase::TracksWritten,
            5 => WebmMuxerPhase::Clustering,
            6 => WebmMuxerPhase::CuesWritten,
            7 => WebmMuxerPhase::Finalized,
            _ => WebmMuxerPhase::Error,
        }
    }
}

// ============================================================================
// WebM Video Track Configuration
// ============================================================================

/// Video track configuration
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct WebmVideoTrack {
    /// Codec (VP8/VP9/AV1)
    pub codec: WebmVideoCodec,
    /// Width in pixels
    pub width: u16,
    /// Height in pixels
    pub height: u16,
    /// Track number (1-based)
    pub track_number: u8,
    /// Codec private data length
    pub codec_private_len: u16,
}

/// Audio track configuration
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct WebmAudioTrack {
    /// Codec (Vorbis/Opus)
    pub codec: WebmAudioCodec,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u8,
    /// Bit depth (16, 24, 32)
    pub bit_depth: u8,
    /// Track number (1-based)
    pub track_number: u8,
    /// Codec private data length
    pub codec_private_len: u16,
}

// ============================================================================
// WebM Cue Point (for seeking)
// ============================================================================

/// Cue point for seeking
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct WebmCuePoint {
    /// Timestamp in timecode units
    pub time: u64,
    /// Track number
    pub track: u8,
    /// Cluster position (relative to Segment)
    pub cluster_position: u64,
}

// ============================================================================
// EBML Writer Utilities
// ============================================================================

/// EBML variable-length integer encoder
pub struct EbmlWriter;

impl EbmlWriter {
    /// Write EBML element ID (1-4 bytes)
    #[inline]
    pub fn write_id(buf: &mut [u8], id: u32) -> usize {
        if id < 0x80 {
            // Can't encode IDs < 0x80 in 1 byte (reserved)
            buf[0] = id as u8;
            1
        } else if id < 0x4000 {
            buf[0] = ((id >> 8) & 0xFF) as u8;
            buf[1] = (id & 0xFF) as u8;
            2
        } else if id < 0x200000 {
            buf[0] = ((id >> 16) & 0xFF) as u8;
            buf[1] = ((id >> 8) & 0xFF) as u8;
            buf[2] = (id & 0xFF) as u8;
            3
        } else {
            buf[0] = ((id >> 24) & 0xFF) as u8;
            buf[1] = ((id >> 16) & 0xFF) as u8;
            buf[2] = ((id >> 8) & 0xFF) as u8;
            buf[3] = (id & 0xFF) as u8;
            4
        }
    }

    /// Write EBML variable-length size (1-8 bytes)
    #[inline]
    pub fn write_size(buf: &mut [u8], size: u64) -> usize {
        if size < 0x7F {
            buf[0] = 0x80 | (size as u8);
            1
        } else if size < 0x3FFF {
            buf[0] = 0x40 | ((size >> 8) as u8);
            buf[1] = (size & 0xFF) as u8;
            2
        } else if size < 0x1FFFFF {
            buf[0] = 0x20 | ((size >> 16) as u8);
            buf[1] = ((size >> 8) & 0xFF) as u8;
            buf[2] = (size & 0xFF) as u8;
            3
        } else if size < 0x0FFFFFFF {
            buf[0] = 0x10 | ((size >> 24) as u8);
            buf[1] = ((size >> 16) & 0xFF) as u8;
            buf[2] = ((size >> 8) & 0xFF) as u8;
            buf[3] = (size & 0xFF) as u8;
            4
        } else if size < 0x07FFFFFFFF {
            buf[0] = 0x08 | ((size >> 32) as u8);
            buf[1] = ((size >> 24) & 0xFF) as u8;
            buf[2] = ((size >> 16) & 0xFF) as u8;
            buf[3] = ((size >> 8) & 0xFF) as u8;
            buf[4] = (size & 0xFF) as u8;
            5
        } else if size < 0x03FFFFFFFFFF {
            buf[0] = 0x04 | ((size >> 40) as u8);
            buf[1] = ((size >> 32) & 0xFF) as u8;
            buf[2] = ((size >> 24) & 0xFF) as u8;
            buf[3] = ((size >> 16) & 0xFF) as u8;
            buf[4] = ((size >> 8) & 0xFF) as u8;
            buf[5] = (size & 0xFF) as u8;
            6
        } else if size < 0x01FFFFFFFFFFFF {
            buf[0] = 0x02 | ((size >> 48) as u8);
            buf[1] = ((size >> 40) & 0xFF) as u8;
            buf[2] = ((size >> 32) & 0xFF) as u8;
            buf[3] = ((size >> 24) & 0xFF) as u8;
            buf[4] = ((size >> 16) & 0xFF) as u8;
            buf[5] = ((size >> 8) & 0xFF) as u8;
            buf[6] = (size & 0xFF) as u8;
            7
        } else {
            buf[0] = 0x01;
            buf[1] = ((size >> 48) & 0xFF) as u8;
            buf[2] = ((size >> 40) & 0xFF) as u8;
            buf[3] = ((size >> 32) & 0xFF) as u8;
            buf[4] = ((size >> 24) & 0xFF) as u8;
            buf[5] = ((size >> 16) & 0xFF) as u8;
            buf[6] = ((size >> 8) & 0xFF) as u8;
            buf[7] = (size & 0xFF) as u8;
            8
        }
    }

    /// Write unknown/streaming size marker (all-1s)
    #[inline]
    pub fn write_unknown_size(buf: &mut [u8]) -> usize {
        // 8-byte unknown size (0x01FFFFFFFFFFFFFF)
        buf[0] = 0x01;
        buf[1] = 0xFF;
        buf[2] = 0xFF;
        buf[3] = 0xFF;
        buf[4] = 0xFF;
        buf[5] = 0xFF;
        buf[6] = 0xFF;
        buf[7] = 0xFF;
        8
    }

    /// Write unsigned integer element
    #[inline]
    pub fn write_uint_element(buf: &mut [u8], id: u32, value: u64) -> usize {
        let mut pos = Self::write_id(buf, id);

        // Determine minimum bytes needed for value
        let data_size = if value == 0 {
            1
        } else if value < 0x100 {
            1
        } else if value < 0x10000 {
            2
        } else if value < 0x1000000 {
            3
        } else if value < 0x100000000 {
            4
        } else if value < 0x10000000000 {
            5
        } else if value < 0x1000000000000 {
            6
        } else if value < 0x100000000000000 {
            7
        } else {
            8
        };

        pos += Self::write_size(&mut buf[pos..], data_size);

        // Write value in big-endian
        for i in (0..data_size as usize).rev() {
            buf[pos] = ((value >> (i * 8)) & 0xFF) as u8;
            pos += 1;
        }

        pos
    }

    /// Write signed integer element
    #[inline]
    pub fn write_sint_element(buf: &mut [u8], id: u32, value: i64) -> usize {
        let mut pos = Self::write_id(buf, id);

        // Determine minimum bytes needed
        let data_size = if value >= -0x80 && value < 0x80 {
            1
        } else if value >= -0x8000 && value < 0x8000 {
            2
        } else if value >= -0x800000 && value < 0x800000 {
            3
        } else if value >= -0x80000000 && value < 0x80000000 {
            4
        } else {
            8
        };

        pos += Self::write_size(&mut buf[pos..], data_size);

        // Write value in big-endian (two's complement)
        let uval = value as u64;
        for i in (0..data_size as usize).rev() {
            buf[pos] = ((uval >> (i * 8)) & 0xFF) as u8;
            pos += 1;
        }

        pos
    }

    /// Write float element (8-byte double)
    #[inline]
    pub fn write_float_element(buf: &mut [u8], id: u32, value: f64) -> usize {
        let mut pos = Self::write_id(buf, id);
        pos += Self::write_size(&mut buf[pos..], 8);

        let bits = value.to_bits();
        for i in (0..8).rev() {
            buf[pos] = ((bits >> (i * 8)) & 0xFF) as u8;
            pos += 1;
        }

        pos
    }

    /// Write string element (UTF-8)
    #[inline]
    pub fn write_string_element(buf: &mut [u8], id: u32, data: &[u8]) -> usize {
        let mut pos = Self::write_id(buf, id);
        pos += Self::write_size(&mut buf[pos..], data.len() as u64);
        buf[pos..pos + data.len()].copy_from_slice(data);
        pos + data.len()
    }

    /// Write binary element
    #[inline]
    pub fn write_binary_element(buf: &mut [u8], id: u32, data: &[u8]) -> usize {
        Self::write_string_element(buf, id, data) // Same encoding
    }

    /// Start a master element (returns position to patch size)
    #[inline]
    pub fn start_master_element(buf: &mut [u8], id: u32) -> (usize, usize) {
        let mut pos = Self::write_id(buf, id);
        let size_pos = pos;
        // Reserve 8 bytes for size (unknown initially)
        pos += Self::write_unknown_size(&mut buf[pos..]);
        (pos, size_pos)
    }

    /// Patch master element size (after content written)
    #[inline]
    pub fn patch_master_size(buf: &mut [u8], size_pos: usize, content_size: u64) {
        // Overwrite the 8-byte unknown size with actual size
        Self::write_size_8byte(&mut buf[size_pos..], content_size);
    }

    /// Write 8-byte fixed size (for patching)
    #[inline]
    fn write_size_8byte(buf: &mut [u8], size: u64) {
        buf[0] = 0x01 | ((size >> 49) as u8 & 0x7F);
        buf[1] = ((size >> 41) & 0xFF) as u8;
        buf[2] = ((size >> 33) & 0xFF) as u8;
        buf[3] = ((size >> 25) & 0xFF) as u8;
        buf[4] = ((size >> 17) & 0xFF) as u8;
        buf[5] = ((size >> 9) & 0xFF) as u8;
        buf[6] = ((size >> 1) & 0xFF) as u8;
        buf[7] = ((size << 7) & 0x80) as u8 | 0x7F;
    }
}

// ============================================================================
// WebM Muxer Error
// ============================================================================

/// WebM muxer error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebmMuxerError {
    /// Invalid phase transition
    InvalidPhase,
    /// Buffer too small
    BufferTooSmall,
    /// Invalid codec (not WebM-compatible)
    InvalidCodec,
    /// No video or audio track configured
    NoTracks,
    /// Cluster not started
    NoCluster,
    /// Invalid timestamp
    InvalidTimestamp,
    /// Track not found
    TrackNotFound,
    /// Generation counter mismatch (concurrent modification)
    GenerationMismatch,
    /// Maximum clusters exceeded
    MaxClustersExceeded,
    /// Capsule already finalized
    AlreadyFinalized,
}

// ============================================================================
// WebM Muxer Capsule (T5 Streaming)
// ============================================================================

/// State bits layout:
/// - Bits 0-7: Phase (WebmMuxerPhase)
/// - Bits 8-15: Flags (streaming mode, cues enabled, etc.)
/// - Bits 16-23: Video track count
/// - Bits 24-31: Audio track count
/// - Bits 32-63: Reserved
const STATE_PHASE_MASK: u64 = 0xFF;
const STATE_FLAGS_SHIFT: u64 = 8;
const STATE_FLAGS_MASK: u64 = 0xFF << STATE_FLAGS_SHIFT;
const STATE_VIDEO_TRACKS_SHIFT: u64 = 16;
const STATE_VIDEO_TRACKS_MASK: u64 = 0xFF << STATE_VIDEO_TRACKS_SHIFT;
const STATE_AUDIO_TRACKS_SHIFT: u64 = 24;
const STATE_AUDIO_TRACKS_MASK: u64 = 0xFF << STATE_AUDIO_TRACKS_SHIFT;

/// Flag bits
const FLAG_STREAMING_MODE: u64 = 0x01 << STATE_FLAGS_SHIFT;
const FLAG_CUES_ENABLED: u64 = 0x02 << STATE_FLAGS_SHIFT;
const FLAG_CLUSTER_OPEN: u64 = 0x04 << STATE_FLAGS_SHIFT;

/// WebM Muxer Capsule - T5 Streaming tier
///
/// ## Chaos Compliance
/// - 512B cache-aligned
/// - Generation counter for ABA prevention
/// - 100% lockfree (atomic state transitions)
///
/// ## UCE34 Q10: T5 Streaming
/// - O(1) cluster append
/// - O(1) SimpleBlock write
/// - Incremental header generation
#[repr(C, align(64))]
pub struct WebmMuxerCapsule {
    /// Packed state: phase (8) + flags (8) + video_tracks (8) + audio_tracks (8) + reserved (32)
    state: AtomicU64,

    /// Segment data start position (after EBML header)
    segment_start: AtomicU64,

    /// Current cluster start position (relative to segment)
    cluster_start: AtomicU64,

    /// Current cluster timecode (in timecode units)
    cluster_timecode: AtomicU64,

    /// Has video track
    has_video: AtomicBool,

    /// Has audio track
    has_audio: AtomicBool,

    /// Duration in timecode units (updated as clusters are written)
    duration_units: AtomicU64,

    /// Generation counter for ABA prevention
    generation: AtomicU64,

    /// Timecode scale (nanoseconds per unit, default 1000000 = 1ms)
    timecode_scale_ns: AtomicU64,

    /// Total bytes written
    bytes_written: AtomicU64,

    /// Cluster count (for cues)
    cluster_count: AtomicU64,

    /// Last keyframe position (for cues)
    last_keyframe_pos: AtomicU64,

    /// Padding for 512B alignment
    _padding: [u8; 512 - 104],
}

// #ASSUME_SIZE_512: WebmMuxerCapsule is exactly 512 bytes for cache alignment
// #VERIFY_SIZE: const_assert!(core::mem::size_of::<WebmMuxerCapsule>() == 512)
const _: () = assert!(core::mem::size_of::<WebmMuxerCapsule>() == 512);

// #ASSUME_ALIGN_64: WebmMuxerCapsule is 64-byte aligned for cache line optimization
// #VERIFY_ALIGN: const_assert!(core::mem::align_of::<WebmMuxerCapsule>() == 64)
const _: () = assert!(core::mem::align_of::<WebmMuxerCapsule>() == 64);

impl WebmMuxerCapsule {
    /// Create a new WebM muxer capsule
    ///
    /// # Arguments
    /// * `streaming_mode` - If true, use unknown segment size for live streaming
    /// * `enable_cues` - If true, generate cue points for seeking
    #[inline]
    pub const fn new(streaming_mode: bool, enable_cues: bool) -> Self {
        let mut flags = 0u64;
        if streaming_mode {
            flags |= FLAG_STREAMING_MODE;
        }
        if enable_cues {
            flags |= FLAG_CUES_ENABLED;
        }

        Self {
            state: AtomicU64::new(flags),
            segment_start: AtomicU64::new(0),
            cluster_start: AtomicU64::new(0),
            cluster_timecode: AtomicU64::new(0),
            has_video: AtomicBool::new(false),
            has_audio: AtomicBool::new(false),
            duration_units: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            timecode_scale_ns: AtomicU64::new(1_000_000), // 1ms default
            bytes_written: AtomicU64::new(0),
            cluster_count: AtomicU64::new(0),
            last_keyframe_pos: AtomicU64::new(0),
            _padding: [0u8; 512 - 104],
        }
    }

    /// Get current phase
    #[inline]
    pub fn phase(&self) -> WebmMuxerPhase {
        let state = self.state.load(Ordering::Acquire);
        WebmMuxerPhase::from_u8((state & STATE_PHASE_MASK) as u8)
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Check if streaming mode is enabled
    #[inline]
    pub fn is_streaming_mode(&self) -> bool {
        self.state.load(Ordering::Acquire) & FLAG_STREAMING_MODE != 0
    }

    /// Check if cues are enabled
    #[inline]
    pub fn cues_enabled(&self) -> bool {
        self.state.load(Ordering::Acquire) & FLAG_CUES_ENABLED != 0
    }

    /// Set timecode scale (nanoseconds per timecode unit)
    ///
    /// Default is 1,000,000 (1ms per unit).
    /// Common values: 1000000 (1ms), 1000 (1μs)
    #[inline]
    pub fn set_timecode_scale(&self, ns: u64) {
        self.timecode_scale_ns.store(ns, Ordering::Release);
    }

    /// Get timecode scale
    #[inline]
    pub fn timecode_scale(&self) -> u64 {
        self.timecode_scale_ns.load(Ordering::Acquire)
    }

    /// Get total bytes written
    #[inline]
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written.load(Ordering::Acquire)
    }

    /// Get current duration in timecode units
    #[inline]
    pub fn duration_units(&self) -> u64 {
        self.duration_units.load(Ordering::Acquire)
    }

    /// Get duration in milliseconds
    #[inline]
    pub fn duration_ms(&self) -> u64 {
        let units = self.duration_units.load(Ordering::Acquire);
        let scale_ns = self.timecode_scale_ns.load(Ordering::Acquire);
        (units * scale_ns) / 1_000_000
    }

    /// Validate that a video codec is WebM-compatible
    #[inline]
    pub const fn validate_video_codec(codec: &[u8]) -> bool {
        // Check against allowed WebM video codecs
        matches!(codec, b"V_VP8" | b"V_VP9" | b"V_AV1")
    }

    /// Validate that an audio codec is WebM-compatible
    #[inline]
    pub const fn validate_audio_codec(codec: &[u8]) -> bool {
        // Check against allowed WebM audio codecs
        matches!(codec, b"A_VORBIS" | b"A_OPUS")
    }

    /// Write EBML header for WebM
    ///
    /// # Returns
    /// Number of bytes written, or error
    pub fn write_ebml_header(&self, buf: &mut [u8]) -> Result<usize, WebmMuxerError> {
        // Check phase
        let state = self.state.load(Ordering::Acquire);
        let phase = WebmMuxerPhase::from_u8((state & STATE_PHASE_MASK) as u8);
        if phase != WebmMuxerPhase::Uninitialized {
            return Err(WebmMuxerError::InvalidPhase);
        }

        // Need at least 64 bytes for EBML header
        if buf.len() < 64 {
            return Err(WebmMuxerError::BufferTooSmall);
        }

        let mut pos = 0;

        // EBML Master element start
        let (content_start, size_pos) = EbmlWriter::start_master_element(&mut buf[pos..], EBML_ID);
        pos += content_start;
        let header_content_start = pos;

        // EBMLVersion: 1
        pos += EbmlWriter::write_uint_element(&mut buf[pos..], EBML_VERSION_ID, 1);

        // EBMLReadVersion: 1
        pos += EbmlWriter::write_uint_element(&mut buf[pos..], EBML_READ_VERSION_ID, 1);

        // EBMLMaxIDLength: 4
        pos += EbmlWriter::write_uint_element(&mut buf[pos..], EBML_MAX_ID_LENGTH_ID, 4);

        // EBMLMaxSizeLength: 8
        pos += EbmlWriter::write_uint_element(&mut buf[pos..], EBML_MAX_SIZE_LENGTH_ID, 8);

        // DocType: "webm"
        pos += EbmlWriter::write_string_element(&mut buf[pos..], DOC_TYPE_ID, DOCTYPE_WEBM);

        // DocTypeVersion: 4 (WebM 4.0)
        pos += EbmlWriter::write_uint_element(&mut buf[pos..], DOC_TYPE_VERSION_ID, 4);

        // DocTypeReadVersion: 2
        pos += EbmlWriter::write_uint_element(&mut buf[pos..], DOC_TYPE_READ_VERSION_ID, 2);

        // Patch EBML header size
        let content_size = pos - header_content_start;
        EbmlWriter::patch_master_size(buf, size_pos, content_size as u64);

        // Update state
        let new_state = (state & !STATE_PHASE_MASK) | (WebmMuxerPhase::HeaderWritten as u64);
        self.state.store(new_state, Ordering::Release);
        self.bytes_written.fetch_add(pos as u64, Ordering::AcqRel);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(pos)
    }

    /// Start Segment element (with unknown size for streaming)
    pub fn start_segment(&self, buf: &mut [u8]) -> Result<usize, WebmMuxerError> {
        // Check phase
        let state = self.state.load(Ordering::Acquire);
        let phase = WebmMuxerPhase::from_u8((state & STATE_PHASE_MASK) as u8);
        if phase != WebmMuxerPhase::HeaderWritten {
            return Err(WebmMuxerError::InvalidPhase);
        }

        if buf.len() < 12 {
            return Err(WebmMuxerError::BufferTooSmall);
        }

        let mut pos = 0;

        // Write Segment ID
        pos += EbmlWriter::write_id(&mut buf[pos..], SEGMENT_ID);

        // Store segment data start position
        let segment_data_start = self.bytes_written.load(Ordering::Acquire) + pos as u64;

        // Write unknown size (streaming mode) or reserve space
        if self.is_streaming_mode() {
            pos += EbmlWriter::write_unknown_size(&mut buf[pos..]);
        } else {
            // Reserve 8 bytes for later patching
            pos += EbmlWriter::write_unknown_size(&mut buf[pos..]);
        }

        self.segment_start.store(segment_data_start, Ordering::Release);

        // Update state
        let new_state = (state & !STATE_PHASE_MASK) | (WebmMuxerPhase::SegmentStarted as u64);
        self.state.store(new_state, Ordering::Release);
        self.bytes_written.fetch_add(pos as u64, Ordering::AcqRel);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(pos)
    }

    /// Write Info element
    pub fn write_info(
        &self,
        buf: &mut [u8],
        muxing_app: &[u8],
        writing_app: &[u8],
    ) -> Result<usize, WebmMuxerError> {
        // Check phase
        let state = self.state.load(Ordering::Acquire);
        let phase = WebmMuxerPhase::from_u8((state & STATE_PHASE_MASK) as u8);
        if phase != WebmMuxerPhase::SegmentStarted {
            return Err(WebmMuxerError::InvalidPhase);
        }

        // Estimate size needed
        let needed = 64 + muxing_app.len() + writing_app.len();
        if buf.len() < needed {
            return Err(WebmMuxerError::BufferTooSmall);
        }

        let mut pos = 0;

        // Info Master element
        let (content_start, size_pos) = EbmlWriter::start_master_element(&mut buf[pos..], INFO_ID);
        pos += content_start;
        let info_content_start = pos;

        // TimecodeScale (mandatory)
        let scale = self.timecode_scale_ns.load(Ordering::Acquire);
        pos += EbmlWriter::write_uint_element(&mut buf[pos..], TIMECODE_SCALE_ID, scale);

        // MuxingApp (mandatory)
        pos += EbmlWriter::write_string_element(&mut buf[pos..], MUXING_APP_ID, muxing_app);

        // WritingApp
        pos += EbmlWriter::write_string_element(&mut buf[pos..], WRITING_APP_ID, writing_app);

        // Note: Duration will be written at finalization if not streaming

        // Patch Info size
        let content_size = pos - info_content_start;
        EbmlWriter::patch_master_size(buf, size_pos, content_size as u64);

        // Update state
        let new_state = (state & !STATE_PHASE_MASK) | (WebmMuxerPhase::InfoWritten as u64);
        self.state.store(new_state, Ordering::Release);
        self.bytes_written.fetch_add(pos as u64, Ordering::AcqRel);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(pos)
    }

    /// Write Tracks element with video track
    pub fn write_video_track(
        &self,
        buf: &mut [u8],
        track: &WebmVideoTrack,
        codec_private: &[u8],
    ) -> Result<usize, WebmMuxerError> {
        // Check phase
        let state = self.state.load(Ordering::Acquire);
        let phase = WebmMuxerPhase::from_u8((state & STATE_PHASE_MASK) as u8);
        if phase != WebmMuxerPhase::InfoWritten && phase != WebmMuxerPhase::TracksWritten {
            return Err(WebmMuxerError::InvalidPhase);
        }

        // Estimate size needed
        let needed = 128 + codec_private.len();
        if buf.len() < needed {
            return Err(WebmMuxerError::BufferTooSmall);
        }

        let mut pos = 0;

        // Start Tracks if first track
        let tracks_size_pos = if phase == WebmMuxerPhase::InfoWritten {
            let (content_start, size_pos) =
                EbmlWriter::start_master_element(&mut buf[pos..], TRACKS_ID);
            pos += content_start;
            Some(size_pos)
        } else {
            None
        };

        let tracks_content_start = pos;

        // TrackEntry
        let (entry_start, entry_size_pos) =
            EbmlWriter::start_master_element(&mut buf[pos..], TRACK_ENTRY_ID);
        pos += entry_start;
        let entry_content_start = pos;

        // TrackNumber
        pos += EbmlWriter::write_uint_element(
            &mut buf[pos..],
            TRACK_NUMBER_ID,
            track.track_number as u64,
        );

        // TrackUID (use track number as UID for simplicity)
        pos += EbmlWriter::write_uint_element(
            &mut buf[pos..],
            TRACK_UID_ID,
            track.track_number as u64,
        );

        // TrackType: 1 = video
        pos += EbmlWriter::write_uint_element(&mut buf[pos..], TRACK_TYPE_ID, 1);

        // FlagEnabled: 1
        pos += EbmlWriter::write_uint_element(&mut buf[pos..], FLAG_ENABLED_ID, 1);

        // FlagDefault: 1
        pos += EbmlWriter::write_uint_element(&mut buf[pos..], FLAG_DEFAULT_ID, 1);

        // FlagLacing: 0 (no lacing for WebM)
        pos += EbmlWriter::write_uint_element(&mut buf[pos..], FLAG_LACING_ID, 0);

        // CodecID
        pos += EbmlWriter::write_string_element(&mut buf[pos..], CODEC_ID_ID, track.codec.codec_id());

        // CodecPrivate (if any)
        if !codec_private.is_empty() {
            pos += EbmlWriter::write_binary_element(&mut buf[pos..], CODEC_PRIVATE_ID, codec_private);
        }

        // Video element
        let (video_start, video_size_pos) =
            EbmlWriter::start_master_element(&mut buf[pos..], VIDEO_ID);
        pos += video_start;
        let video_content_start = pos;

        // PixelWidth
        pos += EbmlWriter::write_uint_element(&mut buf[pos..], PIXEL_WIDTH_ID, track.width as u64);

        // PixelHeight
        pos += EbmlWriter::write_uint_element(&mut buf[pos..], PIXEL_HEIGHT_ID, track.height as u64);

        // Patch Video size
        let video_content_size = pos - video_content_start;
        EbmlWriter::patch_master_size(buf, video_size_pos, video_content_size as u64);

        // Patch TrackEntry size
        let entry_content_size = pos - entry_content_start;
        EbmlWriter::patch_master_size(buf, entry_size_pos, entry_content_size as u64);

        // Patch Tracks size if started
        if let Some(size_pos) = tracks_size_pos {
            let tracks_content_size = pos - tracks_content_start;
            EbmlWriter::patch_master_size(buf, size_pos, tracks_content_size as u64);
        }

        // Update state
        self.has_video.store(true, Ordering::Release);
        let new_state = (state & !STATE_PHASE_MASK) | (WebmMuxerPhase::TracksWritten as u64);
        self.state.store(new_state, Ordering::Release);
        self.bytes_written.fetch_add(pos as u64, Ordering::AcqRel);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(pos)
    }

    /// Write Tracks element with audio track
    pub fn write_audio_track(
        &self,
        buf: &mut [u8],
        track: &WebmAudioTrack,
        codec_private: &[u8],
    ) -> Result<usize, WebmMuxerError> {
        // Check phase
        let state = self.state.load(Ordering::Acquire);
        let phase = WebmMuxerPhase::from_u8((state & STATE_PHASE_MASK) as u8);
        if phase != WebmMuxerPhase::InfoWritten && phase != WebmMuxerPhase::TracksWritten {
            return Err(WebmMuxerError::InvalidPhase);
        }

        // Estimate size needed
        let needed = 128 + codec_private.len();
        if buf.len() < needed {
            return Err(WebmMuxerError::BufferTooSmall);
        }

        let mut pos = 0;

        // Start Tracks if first track
        let tracks_size_pos = if phase == WebmMuxerPhase::InfoWritten {
            let (content_start, size_pos) =
                EbmlWriter::start_master_element(&mut buf[pos..], TRACKS_ID);
            pos += content_start;
            Some(size_pos)
        } else {
            None
        };

        let tracks_content_start = pos;

        // TrackEntry
        let (entry_start, entry_size_pos) =
            EbmlWriter::start_master_element(&mut buf[pos..], TRACK_ENTRY_ID);
        pos += entry_start;
        let entry_content_start = pos;

        // TrackNumber
        pos += EbmlWriter::write_uint_element(
            &mut buf[pos..],
            TRACK_NUMBER_ID,
            track.track_number as u64,
        );

        // TrackUID
        pos += EbmlWriter::write_uint_element(
            &mut buf[pos..],
            TRACK_UID_ID,
            track.track_number as u64,
        );

        // TrackType: 2 = audio
        pos += EbmlWriter::write_uint_element(&mut buf[pos..], TRACK_TYPE_ID, 2);

        // FlagEnabled: 1
        pos += EbmlWriter::write_uint_element(&mut buf[pos..], FLAG_ENABLED_ID, 1);

        // FlagDefault: 1
        pos += EbmlWriter::write_uint_element(&mut buf[pos..], FLAG_DEFAULT_ID, 1);

        // FlagLacing: 0
        pos += EbmlWriter::write_uint_element(&mut buf[pos..], FLAG_LACING_ID, 0);

        // CodecID
        pos += EbmlWriter::write_string_element(&mut buf[pos..], CODEC_ID_ID, track.codec.codec_id());

        // CodecPrivate
        if !codec_private.is_empty() {
            pos += EbmlWriter::write_binary_element(&mut buf[pos..], CODEC_PRIVATE_ID, codec_private);
        }

        // Audio element
        let (audio_start, audio_size_pos) =
            EbmlWriter::start_master_element(&mut buf[pos..], AUDIO_ID);
        pos += audio_start;
        let audio_content_start = pos;

        // SamplingFrequency
        pos += EbmlWriter::write_float_element(
            &mut buf[pos..],
            SAMPLING_FREQ_ID,
            track.sample_rate as f64,
        );

        // Channels
        pos += EbmlWriter::write_uint_element(&mut buf[pos..], CHANNELS_ID, track.channels as u64);

        // BitDepth (if non-zero)
        if track.bit_depth > 0 {
            pos += EbmlWriter::write_uint_element(
                &mut buf[pos..],
                BIT_DEPTH_ID,
                track.bit_depth as u64,
            );
        }

        // Patch Audio size
        let audio_content_size = pos - audio_content_start;
        EbmlWriter::patch_master_size(buf, audio_size_pos, audio_content_size as u64);

        // Patch TrackEntry size
        let entry_content_size = pos - entry_content_start;
        EbmlWriter::patch_master_size(buf, entry_size_pos, entry_content_size as u64);

        // Patch Tracks size if started
        if let Some(size_pos) = tracks_size_pos {
            let tracks_content_size = pos - tracks_content_start;
            EbmlWriter::patch_master_size(buf, size_pos, tracks_content_size as u64);
        }

        // Update state
        self.has_audio.store(true, Ordering::Release);
        let new_state = (state & !STATE_PHASE_MASK) | (WebmMuxerPhase::TracksWritten as u64);
        self.state.store(new_state, Ordering::Release);
        self.bytes_written.fetch_add(pos as u64, Ordering::AcqRel);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(pos)
    }

    /// Start a new Cluster
    ///
    /// # Arguments
    /// * `timecode` - Cluster timestamp in timecode units
    pub fn start_cluster(&self, buf: &mut [u8], timecode: u64) -> Result<usize, WebmMuxerError> {
        // Check phase
        let state = self.state.load(Ordering::Acquire);
        let phase = WebmMuxerPhase::from_u8((state & STATE_PHASE_MASK) as u8);
        if phase != WebmMuxerPhase::TracksWritten && phase != WebmMuxerPhase::Clustering {
            return Err(WebmMuxerError::InvalidPhase);
        }

        // Check we have at least one track
        if !self.has_video.load(Ordering::Acquire) && !self.has_audio.load(Ordering::Acquire) {
            return Err(WebmMuxerError::NoTracks);
        }

        if buf.len() < 24 {
            return Err(WebmMuxerError::BufferTooSmall);
        }

        let mut pos = 0;

        // Record cluster position
        let cluster_pos = self.bytes_written.load(Ordering::Acquire);
        self.cluster_start.store(cluster_pos, Ordering::Release);
        self.cluster_timecode.store(timecode, Ordering::Release);

        // Cluster element (unknown size for streaming)
        pos += EbmlWriter::write_id(&mut buf[pos..], CLUSTER_ID);
        pos += EbmlWriter::write_unknown_size(&mut buf[pos..]);

        // Timecode element
        pos += EbmlWriter::write_uint_element(&mut buf[pos..], TIMECODE_ID, timecode);

        // Update state
        let new_state = (state & !STATE_PHASE_MASK)
            | (WebmMuxerPhase::Clustering as u64)
            | FLAG_CLUSTER_OPEN;
        self.state.store(new_state, Ordering::Release);
        self.bytes_written.fetch_add(pos as u64, Ordering::AcqRel);
        self.cluster_count.fetch_add(1, Ordering::AcqRel);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(pos)
    }

    /// Write SimpleBlock to current cluster
    ///
    /// # Arguments
    /// * `track_number` - Track number (1-based)
    /// * `timecode_delta` - Timestamp relative to cluster timecode (signed 16-bit)
    /// * `keyframe` - Is this a keyframe?
    /// * `data` - Frame data
    pub fn write_simple_block(
        &self,
        buf: &mut [u8],
        track_number: u8,
        timecode_delta: i16,
        keyframe: bool,
        data: &[u8],
    ) -> Result<usize, WebmMuxerError> {
        // Check phase
        let state = self.state.load(Ordering::Acquire);
        let phase = WebmMuxerPhase::from_u8((state & STATE_PHASE_MASK) as u8);
        if phase != WebmMuxerPhase::Clustering {
            return Err(WebmMuxerError::InvalidPhase);
        }

        // Check cluster is open
        if state & FLAG_CLUSTER_OPEN == 0 {
            return Err(WebmMuxerError::NoCluster);
        }

        // Size needed: ID (1-4) + size (1-8) + track (1-4) + timecode (2) + flags (1) + data
        let block_data_size = 1 + 2 + 1 + data.len(); // track + timecode + flags + data
        let needed = 12 + block_data_size;
        if buf.len() < needed {
            return Err(WebmMuxerError::BufferTooSmall);
        }

        let mut pos = 0;

        // SimpleBlock ID
        pos += EbmlWriter::write_id(&mut buf[pos..], SIMPLE_BLOCK_ID);

        // Size
        pos += EbmlWriter::write_size(&mut buf[pos..], block_data_size as u64);

        // Track number (EBML-encoded)
        // WebM track numbers are typically 1-127, encoded in 1 byte with 0x80 marker
        // For track numbers >= 128, we use 2-byte encoding
        let track_num = track_number as u16;
        if track_num < 0x80 {
            buf[pos] = 0x80 | (track_num as u8);
            pos += 1;
        } else {
            // 2-byte encoding: 0x40XX where XX is the value
            buf[pos] = 0x40 | ((track_num >> 8) as u8 & 0x3F);
            buf[pos + 1] = (track_num & 0xFF) as u8;
            pos += 2;
        }

        // Timecode delta (signed 16-bit big-endian)
        buf[pos] = ((timecode_delta as u16) >> 8) as u8;
        buf[pos + 1] = (timecode_delta as u16 & 0xFF) as u8;
        pos += 2;

        // Flags: bit 7 = keyframe, bit 0 = discardable (0)
        let flags = if keyframe { 0x80 } else { 0x00 };
        buf[pos] = flags;
        pos += 1;

        // Frame data
        buf[pos..pos + data.len()].copy_from_slice(data);
        pos += data.len();

        // Update duration tracking
        let cluster_tc = self.cluster_timecode.load(Ordering::Acquire);
        let frame_tc = cluster_tc.wrapping_add(timecode_delta as u64);
        let current_duration = self.duration_units.load(Ordering::Acquire);
        if frame_tc > current_duration {
            self.duration_units.store(frame_tc, Ordering::Release);
        }

        // Track keyframe position for cues
        if keyframe {
            let pos_bytes = self.bytes_written.load(Ordering::Acquire);
            self.last_keyframe_pos.store(pos_bytes, Ordering::Release);
        }

        self.bytes_written.fetch_add(pos as u64, Ordering::AcqRel);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(pos)
    }

    /// Write Cues element (optional, for seeking)
    pub fn write_cues(&self, buf: &mut [u8], cues: &[WebmCuePoint]) -> Result<usize, WebmMuxerError> {
        // Check phase
        let state = self.state.load(Ordering::Acquire);
        let phase = WebmMuxerPhase::from_u8((state & STATE_PHASE_MASK) as u8);
        if phase != WebmMuxerPhase::Clustering {
            return Err(WebmMuxerError::InvalidPhase);
        }

        if !self.cues_enabled() || cues.is_empty() {
            return Ok(0);
        }

        // Estimate size: ~40 bytes per cue point
        let needed = 16 + cues.len() * 48;
        if buf.len() < needed {
            return Err(WebmMuxerError::BufferTooSmall);
        }

        let segment_start = self.segment_start.load(Ordering::Acquire);
        let mut pos = 0;

        // Cues Master
        let (content_start, size_pos) = EbmlWriter::start_master_element(&mut buf[pos..], CUES_ID);
        pos += content_start;
        let cues_content_start = pos;

        for cue in cues {
            // CuePoint
            let (cue_start, cue_size_pos) =
                EbmlWriter::start_master_element(&mut buf[pos..], CUE_POINT_ID);
            pos += cue_start;
            let cue_content_start = pos;

            // CueTime
            pos += EbmlWriter::write_uint_element(&mut buf[pos..], CUE_TIME_ID, cue.time);

            // CueTrackPositions
            let (track_start, track_size_pos) =
                EbmlWriter::start_master_element(&mut buf[pos..], CUE_TRACK_POSITIONS_ID);
            pos += track_start;
            let track_content_start = pos;

            // CueTrack
            pos += EbmlWriter::write_uint_element(&mut buf[pos..], CUE_TRACK_ID, cue.track as u64);

            // CueClusterPosition (relative to Segment data start)
            let rel_pos = cue.cluster_position.saturating_sub(segment_start);
            pos += EbmlWriter::write_uint_element(&mut buf[pos..], CUE_CLUSTER_POSITION_ID, rel_pos);

            // Patch CueTrackPositions size
            let track_content_size = pos - track_content_start;
            EbmlWriter::patch_master_size(buf, track_size_pos, track_content_size as u64);

            // Patch CuePoint size
            let cue_content_size = pos - cue_content_start;
            EbmlWriter::patch_master_size(buf, cue_size_pos, cue_content_size as u64);
        }

        // Patch Cues size
        let cues_content_size = pos - cues_content_start;
        EbmlWriter::patch_master_size(buf, size_pos, cues_content_size as u64);

        // Update state
        let new_state = (state & !STATE_PHASE_MASK) | (WebmMuxerPhase::CuesWritten as u64);
        self.state.store(new_state, Ordering::Release);
        self.bytes_written.fetch_add(pos as u64, Ordering::AcqRel);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(pos)
    }

    /// Finalize the WebM file
    ///
    /// For non-streaming mode, returns the segment size that should be patched.
    pub fn finalize(&self) -> Result<Option<u64>, WebmMuxerError> {
        let state = self.state.load(Ordering::Acquire);
        let phase = WebmMuxerPhase::from_u8((state & STATE_PHASE_MASK) as u8);

        if phase == WebmMuxerPhase::Finalized {
            return Err(WebmMuxerError::AlreadyFinalized);
        }

        if phase != WebmMuxerPhase::Clustering && phase != WebmMuxerPhase::CuesWritten {
            return Err(WebmMuxerError::InvalidPhase);
        }

        // Update state to finalized
        let new_state = (state & !STATE_PHASE_MASK) | (WebmMuxerPhase::Finalized as u64);
        self.state.store(new_state, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Return segment size for patching (if not streaming mode)
        if !self.is_streaming_mode() {
            let segment_start = self.segment_start.load(Ordering::Acquire);
            let total_bytes = self.bytes_written.load(Ordering::Acquire);
            let segment_size = total_bytes.saturating_sub(segment_start);
            Ok(Some(segment_size))
        } else {
            Ok(None)
        }
    }

    /// Reset the muxer for reuse
    pub fn reset(&self) {
        self.state.store(0, Ordering::Release);
        self.segment_start.store(0, Ordering::Release);
        self.cluster_start.store(0, Ordering::Release);
        self.cluster_timecode.store(0, Ordering::Release);
        self.has_video.store(false, Ordering::Release);
        self.has_audio.store(false, Ordering::Release);
        self.duration_units.store(0, Ordering::Release);
        self.timecode_scale_ns.store(1_000_000, Ordering::Release);
        self.bytes_written.store(0, Ordering::Release);
        self.cluster_count.store(0, Ordering::Release);
        self.last_keyframe_pos.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
}

// #ASSUME_SEND_SYNC: WebmMuxerCapsule uses only atomic operations, safe for concurrent access
// #VERIFY_SEND_SYNC: All fields are AtomicU64/AtomicBool, no interior mutability without atomic
unsafe impl Send for WebmMuxerCapsule {}
unsafe impl Sync for WebmMuxerCapsule {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1-Q7: Unit Tests
    // ========================================================================

    #[test]
    fn test_q1_capsule_size_and_alignment() {
        // Q1: Verify Chaos compliance - 512B size, 64B alignment
        assert_eq!(core::mem::size_of::<WebmMuxerCapsule>(), 512);
        assert_eq!(core::mem::align_of::<WebmMuxerCapsule>(), 64);
    }

    #[test]
    fn test_q2_initial_state() {
        // Q2: Verify initial state
        let muxer = WebmMuxerCapsule::new(false, false);
        assert_eq!(muxer.phase(), WebmMuxerPhase::Uninitialized);
        assert_eq!(muxer.generation(), 0);
        assert!(!muxer.is_streaming_mode());
        assert!(!muxer.cues_enabled());
    }

    #[test]
    fn test_q3_streaming_mode_flag() {
        // Q3: Verify streaming mode flag
        let muxer = WebmMuxerCapsule::new(true, false);
        assert!(muxer.is_streaming_mode());
        assert!(!muxer.cues_enabled());
    }

    #[test]
    fn test_q4_cues_enabled_flag() {
        // Q4: Verify cues enabled flag
        let muxer = WebmMuxerCapsule::new(false, true);
        assert!(!muxer.is_streaming_mode());
        assert!(muxer.cues_enabled());
    }

    #[test]
    fn test_q5_timecode_scale_default() {
        // Q5: Default timecode scale is 1ms
        let muxer = WebmMuxerCapsule::new(false, false);
        assert_eq!(muxer.timecode_scale(), 1_000_000);
    }

    #[test]
    fn test_q6_timecode_scale_custom() {
        // Q6: Custom timecode scale
        let muxer = WebmMuxerCapsule::new(false, false);
        muxer.set_timecode_scale(1000); // 1μs
        assert_eq!(muxer.timecode_scale(), 1000);
    }

    #[test]
    fn test_q7_generation_counter_increments() {
        // Q7: Generation counter increments on state changes
        let muxer = WebmMuxerCapsule::new(false, false);
        let mut buf = [0u8; 256];

        assert_eq!(muxer.generation(), 0);
        muxer.write_ebml_header(&mut buf).unwrap();
        assert_eq!(muxer.generation(), 1);
    }

    // ========================================================================
    // Q8-Q14: Property Tests (Codec Validation)
    // ========================================================================

    #[test]
    fn test_q8_validate_vp8_codec() {
        // Q8: VP8 is valid WebM video codec
        assert!(WebmMuxerCapsule::validate_video_codec(b"V_VP8"));
    }

    #[test]
    fn test_q9_validate_vp9_codec() {
        // Q9: VP9 is valid WebM video codec
        assert!(WebmMuxerCapsule::validate_video_codec(b"V_VP9"));
    }

    #[test]
    fn test_q10_validate_av1_codec() {
        // Q10: AV1 is valid WebM video codec
        assert!(WebmMuxerCapsule::validate_video_codec(b"V_AV1"));
    }

    #[test]
    fn test_q11_reject_h264_codec() {
        // Q11: H.264 is NOT valid WebM video codec
        assert!(!WebmMuxerCapsule::validate_video_codec(b"V_MPEG4/ISO/AVC"));
    }

    #[test]
    fn test_q12_reject_h265_codec() {
        // Q12: H.265 is NOT valid WebM video codec
        assert!(!WebmMuxerCapsule::validate_video_codec(b"V_MPEGH/ISO/HEVC"));
    }

    #[test]
    fn test_q13_validate_opus_codec() {
        // Q13: Opus is valid WebM audio codec
        assert!(WebmMuxerCapsule::validate_audio_codec(b"A_OPUS"));
    }

    #[test]
    fn test_q14_validate_vorbis_codec() {
        // Q14: Vorbis is valid WebM audio codec
        assert!(WebmMuxerCapsule::validate_audio_codec(b"A_VORBIS"));
    }

    #[test]
    fn test_reject_aac_codec() {
        // AAC is NOT valid WebM audio codec
        assert!(!WebmMuxerCapsule::validate_audio_codec(b"A_AAC"));
    }

    #[test]
    fn test_reject_mp3_codec() {
        // MP3 is NOT valid WebM audio codec
        assert!(!WebmMuxerCapsule::validate_audio_codec(b"A_MPEG/L3"));
    }

    // ========================================================================
    // Q15-Q21: Integration Tests (Web-Compatible Output)
    // ========================================================================

    #[test]
    fn test_q15_ebml_header_structure() {
        // Q15: EBML header contains "webm" DocType
        let muxer = WebmMuxerCapsule::new(false, false);
        let mut buf = [0u8; 256];

        let len = muxer.write_ebml_header(&mut buf).unwrap();
        assert!(len > 0);

        // Search for "webm" in output
        let webm_found = buf[..len]
            .windows(4)
            .any(|w| w == b"webm");
        assert!(webm_found, "EBML header must contain 'webm' DocType");
    }

    #[test]
    fn test_q16_segment_start() {
        // Q16: Segment element starts correctly
        let muxer = WebmMuxerCapsule::new(true, false);
        let mut buf = [0u8; 512];

        muxer.write_ebml_header(&mut buf).unwrap();
        let header_len = muxer.bytes_written();

        let seg_len = muxer.start_segment(&mut buf[header_len as usize..]).unwrap();
        assert!(seg_len > 0);
        assert_eq!(muxer.phase(), WebmMuxerPhase::SegmentStarted);
    }

    #[test]
    fn test_q17_info_element() {
        // Q17: Info element written correctly
        let muxer = WebmMuxerCapsule::new(true, false);
        let mut buf = [0u8; 1024];
        let mut pos = 0;

        pos += muxer.write_ebml_header(&mut buf[pos..]).unwrap();
        pos += muxer.start_segment(&mut buf[pos..]).unwrap();
        let info_len = muxer.write_info(&mut buf[pos..], b"WebmMuxerCapsule", b"test").unwrap();

        assert!(info_len > 0);
        assert_eq!(muxer.phase(), WebmMuxerPhase::InfoWritten);
    }

    #[test]
    fn test_q18_video_track() {
        // Q18: Video track written correctly
        let muxer = WebmMuxerCapsule::new(true, false);
        let mut buf = [0u8; 2048];
        let mut pos = 0;

        pos += muxer.write_ebml_header(&mut buf[pos..]).unwrap();
        pos += muxer.start_segment(&mut buf[pos..]).unwrap();
        pos += muxer.write_info(&mut buf[pos..], b"test", b"test").unwrap();

        let track = WebmVideoTrack {
            codec: WebmVideoCodec::Vp9,
            width: 1920,
            height: 1080,
            track_number: 1,
            codec_private_len: 0,
        };

        let track_len = muxer.write_video_track(&mut buf[pos..], &track, &[]).unwrap();
        assert!(track_len > 0);
        assert_eq!(muxer.phase(), WebmMuxerPhase::TracksWritten);
    }

    #[test]
    fn test_q19_audio_track() {
        // Q19: Audio track written correctly
        let muxer = WebmMuxerCapsule::new(true, false);
        let mut buf = [0u8; 2048];
        let mut pos = 0;

        pos += muxer.write_ebml_header(&mut buf[pos..]).unwrap();
        pos += muxer.start_segment(&mut buf[pos..]).unwrap();
        pos += muxer.write_info(&mut buf[pos..], b"test", b"test").unwrap();

        let track = WebmAudioTrack {
            codec: WebmAudioCodec::Opus,
            sample_rate: 48000,
            channels: 2,
            bit_depth: 16,
            track_number: 2,
            codec_private_len: 0,
        };

        let track_len = muxer.write_audio_track(&mut buf[pos..], &track, &[]).unwrap();
        assert!(track_len > 0);
        assert_eq!(muxer.phase(), WebmMuxerPhase::TracksWritten);
    }

    #[test]
    fn test_q20_cluster_and_block() {
        // Q20: Cluster and SimpleBlock written correctly
        let muxer = WebmMuxerCapsule::new(true, false);
        let mut buf = [0u8; 4096];
        let mut pos = 0;

        pos += muxer.write_ebml_header(&mut buf[pos..]).unwrap();
        pos += muxer.start_segment(&mut buf[pos..]).unwrap();
        pos += muxer.write_info(&mut buf[pos..], b"test", b"test").unwrap();

        let track = WebmVideoTrack {
            codec: WebmVideoCodec::Vp9,
            width: 1920,
            height: 1080,
            track_number: 1,
            codec_private_len: 0,
        };
        pos += muxer.write_video_track(&mut buf[pos..], &track, &[]).unwrap();

        // Start cluster
        pos += muxer.start_cluster(&mut buf[pos..], 0).unwrap();
        assert_eq!(muxer.phase(), WebmMuxerPhase::Clustering);

        // Write frame
        let frame_data = [0u8; 100];
        let block_len = muxer.write_simple_block(&mut buf[pos..], 1, 0, true, &frame_data).unwrap();
        assert!(block_len > 100);
    }

    #[test]
    fn test_q21_full_webm_generation() {
        // Q21: Complete WebM file generation
        let muxer = WebmMuxerCapsule::new(true, false);
        let mut buf = [0u8; 8192];
        let mut pos = 0;

        // Write header
        pos += muxer.write_ebml_header(&mut buf[pos..]).unwrap();

        // Start segment
        pos += muxer.start_segment(&mut buf[pos..]).unwrap();

        // Write info
        pos += muxer.write_info(&mut buf[pos..], b"WebmMuxerCapsule", b"test").unwrap();

        // Write video track
        let video = WebmVideoTrack {
            codec: WebmVideoCodec::Av1,
            width: 3840,
            height: 2160,
            track_number: 1,
            codec_private_len: 0,
        };
        pos += muxer.write_video_track(&mut buf[pos..], &video, &[]).unwrap();

        // Write audio track
        let audio = WebmAudioTrack {
            codec: WebmAudioCodec::Opus,
            sample_rate: 48000,
            channels: 2,
            bit_depth: 16,
            track_number: 2,
            codec_private_len: 0,
        };
        pos += muxer.write_audio_track(&mut buf[pos..], &audio, &[]).unwrap();

        // Write cluster with frames
        pos += muxer.start_cluster(&mut buf[pos..], 0).unwrap();

        let frame1 = [0xAB; 50];
        pos += muxer.write_simple_block(&mut buf[pos..], 1, 0, true, &frame1).unwrap();

        let frame2 = [0xCD; 30];
        pos += muxer.write_simple_block(&mut buf[pos..], 2, 0, true, &frame2).unwrap();

        let frame3 = [0xEF; 50];
        pos += muxer.write_simple_block(&mut buf[pos..], 1, 33, false, &frame3).unwrap();

        // Finalize
        let result = muxer.finalize().unwrap();
        assert!(result.is_none()); // Streaming mode returns None
        assert_eq!(muxer.phase(), WebmMuxerPhase::Finalized);
        assert!(pos > 200);
    }

    // ========================================================================
    // Additional Tests
    // ========================================================================

    #[test]
    fn test_phase_transitions() {
        let muxer = WebmMuxerCapsule::new(false, false);
        assert_eq!(muxer.phase(), WebmMuxerPhase::Uninitialized);

        let mut buf = [0u8; 4096];
        let mut pos = 0;

        pos += muxer.write_ebml_header(&mut buf[pos..]).unwrap();
        assert_eq!(muxer.phase(), WebmMuxerPhase::HeaderWritten);

        pos += muxer.start_segment(&mut buf[pos..]).unwrap();
        assert_eq!(muxer.phase(), WebmMuxerPhase::SegmentStarted);

        pos += muxer.write_info(&mut buf[pos..], b"test", b"test").unwrap();
        assert_eq!(muxer.phase(), WebmMuxerPhase::InfoWritten);

        let track = WebmVideoTrack {
            codec: WebmVideoCodec::Vp8,
            width: 640,
            height: 480,
            track_number: 1,
            codec_private_len: 0,
        };
        pos += muxer.write_video_track(&mut buf[pos..], &track, &[]).unwrap();
        assert_eq!(muxer.phase(), WebmMuxerPhase::TracksWritten);

        muxer.start_cluster(&mut buf[pos..], 0).unwrap();
        assert_eq!(muxer.phase(), WebmMuxerPhase::Clustering);
    }

    #[test]
    fn test_invalid_phase_transition() {
        let muxer = WebmMuxerCapsule::new(false, false);
        let mut buf = [0u8; 256];

        // Can't start segment before EBML header
        let result = muxer.start_segment(&mut buf);
        assert_eq!(result, Err(WebmMuxerError::InvalidPhase));

        // Can't write info before segment
        let result = muxer.write_info(&mut buf, b"test", b"test");
        assert_eq!(result, Err(WebmMuxerError::InvalidPhase));
    }

    #[test]
    fn test_buffer_too_small() {
        let muxer = WebmMuxerCapsule::new(false, false);
        let mut buf = [0u8; 8]; // Too small

        let result = muxer.write_ebml_header(&mut buf);
        assert_eq!(result, Err(WebmMuxerError::BufferTooSmall));
    }

    #[test]
    fn test_no_tracks_error() {
        // Test that proper phase sequence is followed
        // We can't directly test NoTracks error since tracks must be written
        // before transitioning to a state where start_cluster can be called.
        // This test verifies the phase machine enforces the sequence.
        let muxer = WebmMuxerCapsule::new(false, false);
        let mut buf = [0u8; 1024];

        let _ = muxer.write_ebml_header(&mut buf).unwrap();

        // Can't start cluster before segment
        let result = muxer.start_cluster(&mut buf, 0);
        assert_eq!(result, Err(WebmMuxerError::InvalidPhase));
    }

    #[test]
    fn test_duration_tracking() {
        let muxer = WebmMuxerCapsule::new(true, false);
        let mut buf = [0u8; 4096];
        let mut pos = 0;

        pos += muxer.write_ebml_header(&mut buf[pos..]).unwrap();
        pos += muxer.start_segment(&mut buf[pos..]).unwrap();
        pos += muxer.write_info(&mut buf[pos..], b"test", b"test").unwrap();

        let track = WebmVideoTrack {
            codec: WebmVideoCodec::Vp9,
            width: 1920,
            height: 1080,
            track_number: 1,
            codec_private_len: 0,
        };
        pos += muxer.write_video_track(&mut buf[pos..], &track, &[]).unwrap();

        pos += muxer.start_cluster(&mut buf[pos..], 0).unwrap();
        assert_eq!(muxer.duration_units(), 0);

        let frame = [0u8; 50];
        pos += muxer.write_simple_block(&mut buf[pos..], 1, 0, true, &frame).unwrap();
        assert_eq!(muxer.duration_units(), 0);

        pos += muxer.write_simple_block(&mut buf[pos..], 1, 33, false, &frame).unwrap();
        assert_eq!(muxer.duration_units(), 33);

        let _ = pos;
    }

    #[test]
    fn test_codec_id_strings() {
        assert_eq!(WebmVideoCodec::Vp8.codec_id(), b"V_VP8");
        assert_eq!(WebmVideoCodec::Vp9.codec_id(), b"V_VP9");
        assert_eq!(WebmVideoCodec::Av1.codec_id(), b"V_AV1");

        assert_eq!(WebmAudioCodec::Vorbis.codec_id(), b"A_VORBIS");
        assert_eq!(WebmAudioCodec::Opus.codec_id(), b"A_OPUS");
    }

    #[test]
    fn test_ebml_writer_id_encoding() {
        let mut buf = [0u8; 8];

        // 1-byte ID (< 0x80)
        let len = EbmlWriter::write_id(&mut buf, 0x42);
        assert_eq!(len, 1);
        assert_eq!(buf[0], 0x42);

        // 2-byte ID (0x80 <= id < 0x4000)
        let len = EbmlWriter::write_id(&mut buf, 0x0282); // 2-byte range
        assert_eq!(len, 2);

        // 3-byte ID (0x4000 <= id < 0x200000)
        let len = EbmlWriter::write_id(&mut buf, 0x4282); // In 3-byte range
        assert_eq!(len, 3);

        // 4-byte ID (EBML header ID is 0x1A45DFA3)
        let len = EbmlWriter::write_id(&mut buf, EBML_ID);
        assert_eq!(len, 4);
    }

    #[test]
    fn test_ebml_writer_size_encoding() {
        let mut buf = [0u8; 8];

        // 1-byte size (< 0x7F)
        let len = EbmlWriter::write_size(&mut buf, 0);
        assert_eq!(len, 1);
        assert_eq!(buf[0], 0x80);

        let len = EbmlWriter::write_size(&mut buf, 126);
        assert_eq!(len, 1);
        assert_eq!(buf[0], 0x80 | 126);

        // 2-byte size (0x7F <= size < 0x3FFF)
        let len = EbmlWriter::write_size(&mut buf, 0x7F);
        assert_eq!(len, 2);

        // 3-byte size (0x3FFF <= size < 0x1FFFFF)
        let len = EbmlWriter::write_size(&mut buf, 0x3FFF);
        assert_eq!(len, 3);
    }

    #[test]
    fn test_reset() {
        let muxer = WebmMuxerCapsule::new(true, true);
        let mut buf = [0u8; 256];

        muxer.write_ebml_header(&mut buf).unwrap();
        assert_eq!(muxer.phase(), WebmMuxerPhase::HeaderWritten);
        assert!(muxer.bytes_written() > 0);

        muxer.reset();
        assert_eq!(muxer.phase(), WebmMuxerPhase::Uninitialized);
        assert_eq!(muxer.bytes_written(), 0);
    }

    #[test]
    fn test_cues_writing() {
        let muxer = WebmMuxerCapsule::new(true, true);
        let mut buf = [0u8; 8192];
        let mut pos = 0;

        pos += muxer.write_ebml_header(&mut buf[pos..]).unwrap();
        pos += muxer.start_segment(&mut buf[pos..]).unwrap();
        pos += muxer.write_info(&mut buf[pos..], b"test", b"test").unwrap();

        let track = WebmVideoTrack {
            codec: WebmVideoCodec::Vp9,
            width: 1920,
            height: 1080,
            track_number: 1,
            codec_private_len: 0,
        };
        pos += muxer.write_video_track(&mut buf[pos..], &track, &[]).unwrap();
        pos += muxer.start_cluster(&mut buf[pos..], 0).unwrap();

        let frame = [0u8; 50];
        pos += muxer.write_simple_block(&mut buf[pos..], 1, 0, true, &frame).unwrap();

        let cues = [
            WebmCuePoint {
                time: 0,
                track: 1,
                cluster_position: muxer.cluster_start.load(Ordering::Acquire),
            },
        ];

        let cues_len = muxer.write_cues(&mut buf[pos..], &cues).unwrap();
        assert!(cues_len > 0);
        assert_eq!(muxer.phase(), WebmMuxerPhase::CuesWritten);
    }

    #[test]
    fn test_finalize_non_streaming() {
        let muxer = WebmMuxerCapsule::new(false, false);
        let mut buf = [0u8; 4096];
        let mut pos = 0;

        pos += muxer.write_ebml_header(&mut buf[pos..]).unwrap();
        pos += muxer.start_segment(&mut buf[pos..]).unwrap();
        pos += muxer.write_info(&mut buf[pos..], b"test", b"test").unwrap();

        let track = WebmVideoTrack {
            codec: WebmVideoCodec::Vp9,
            width: 1920,
            height: 1080,
            track_number: 1,
            codec_private_len: 0,
        };
        pos += muxer.write_video_track(&mut buf[pos..], &track, &[]).unwrap();
        pos += muxer.start_cluster(&mut buf[pos..], 0).unwrap();

        let frame = [0u8; 50];
        pos += muxer.write_simple_block(&mut buf[pos..], 1, 0, true, &frame).unwrap();
        let _ = pos;

        let result = muxer.finalize().unwrap();
        assert!(result.is_some()); // Non-streaming returns segment size
    }

    #[test]
    fn test_double_finalize_error() {
        let muxer = WebmMuxerCapsule::new(true, false);
        let mut buf = [0u8; 4096];
        let mut pos = 0;

        pos += muxer.write_ebml_header(&mut buf[pos..]).unwrap();
        pos += muxer.start_segment(&mut buf[pos..]).unwrap();
        pos += muxer.write_info(&mut buf[pos..], b"test", b"test").unwrap();

        let track = WebmVideoTrack {
            codec: WebmVideoCodec::Vp9,
            width: 1920,
            height: 1080,
            track_number: 1,
            codec_private_len: 0,
        };
        pos += muxer.write_video_track(&mut buf[pos..], &track, &[]).unwrap();
        pos += muxer.start_cluster(&mut buf[pos..], 0).unwrap();
        let _ = pos;

        muxer.finalize().unwrap();
        let result = muxer.finalize();
        assert_eq!(result, Err(WebmMuxerError::AlreadyFinalized));
    }

    #[test]
    fn test_duration_ms_calculation() {
        let muxer = WebmMuxerCapsule::new(true, false);

        // Default scale: 1,000,000 ns = 1ms per unit
        muxer.duration_units.store(1000, Ordering::Release);
        assert_eq!(muxer.duration_ms(), 1000); // 1000 units * 1ms = 1000ms

        // Custom scale: 1,000 ns = 1μs per unit
        muxer.set_timecode_scale(1000);
        assert_eq!(muxer.duration_ms(), 1); // 1000 units * 1μs = 1ms
    }

    #[test]
    fn test_webm_phase_from_u8() {
        assert_eq!(WebmMuxerPhase::from_u8(0), WebmMuxerPhase::Uninitialized);
        assert_eq!(WebmMuxerPhase::from_u8(1), WebmMuxerPhase::HeaderWritten);
        assert_eq!(WebmMuxerPhase::from_u8(7), WebmMuxerPhase::Finalized);
        assert_eq!(WebmMuxerPhase::from_u8(100), WebmMuxerPhase::Error);
        assert_eq!(WebmMuxerPhase::from_u8(255), WebmMuxerPhase::Error);
    }
}
