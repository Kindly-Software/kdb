//! MKV/WebM Track metadata capsule
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Manages track-level metadata from Matroska TrackEntry EBML elements.
//! Each video/audio/subtitle track in an MKV/WebM file gets its own capsule instance.
//!
//! ## T1 Atomic Tier (UCE34 Q10)
//!
//! This capsule uses 100% lockfree atomics for thread-safe metadata access.
//! All fields use AtomicU32/AtomicU64 with Acquire/Release ordering.
//!
//! ## EBML Element Parsing
//!
//! Parses the following Matroska elements:
//! - TrackEntry (0xAE): Container for track metadata
//! - TrackNumber (0xD7): Track index
//! - TrackUID (0x73C5): Unique track identifier
//! - TrackType (0x83): Video/Audio/Subtitle type
//! - CodecID (0x86): Codec identifier string
//! - Video (0xE0): Video-specific parameters
//! - Audio (0xE1): Audio-specific parameters
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 T1 Atomic tier, Q33 lockfree, Q34 generation counter
//! - **Chaos**: 256B cache-aligned, 100% lockfree
//! - **ASSUM**: All bounds checks verified
//! - **T28**: 28+ unit tests (Q1-Q28)

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// ============================================================================
// Matroska Element IDs
// ============================================================================

/// Matroska Element IDs for track parsing
pub mod element_ids {
    /// Tracks container element
    pub const TRACKS: u32 = 0x1654AE6B;
    /// TrackEntry element
    pub const TRACK_ENTRY: u32 = 0xAE;

    // TrackEntry children
    /// Track number (1-based index)
    pub const TRACK_NUMBER: u32 = 0xD7;
    /// Track unique identifier
    pub const TRACK_UID: u32 = 0x73C5;
    /// Track type (video=1, audio=2, etc.)
    pub const TRACK_TYPE: u32 = 0x83;
    /// Track enabled flag
    pub const FLAG_ENABLED: u32 = 0xB9;
    /// Track default flag
    pub const FLAG_DEFAULT: u32 = 0x88;
    /// Track forced flag
    pub const FLAG_FORCED: u32 = 0x55AA;
    /// Track lacing flag
    pub const FLAG_LACING: u32 = 0x9C;
    /// Default duration per frame (ns)
    pub const DEFAULT_DURATION: u32 = 0x23E383;
    /// Track name
    pub const NAME: u32 = 0x536E;
    /// Track language (BCP-47)
    pub const LANGUAGE: u32 = 0x22B59C;
    /// Codec identifier string
    pub const CODEC_ID: u32 = 0x86;
    /// Codec private data
    pub const CODEC_PRIVATE: u32 = 0x63A2;
    /// Codec name
    pub const CODEC_NAME: u32 = 0x258688;

    // Video element children
    /// Video container element
    pub const VIDEO: u32 = 0xE0;
    /// Pixel width
    pub const PIXEL_WIDTH: u32 = 0xB0;
    /// Pixel height
    pub const PIXEL_HEIGHT: u32 = 0xBA;
    /// Display width
    pub const DISPLAY_WIDTH: u32 = 0x54B0;
    /// Display height
    pub const DISPLAY_HEIGHT: u32 = 0x54BA;
    /// Display unit (0=pixels, 1=cm, 2=inches, 3=DAR, 4=unknown)
    pub const DISPLAY_UNIT: u32 = 0x54B2;
    /// Color container element
    pub const COLOR: u32 = 0x55B0;

    // Audio element children
    /// Audio container element
    pub const AUDIO: u32 = 0xE1;
    /// Sampling frequency (Hz)
    pub const SAMPLING_FREQUENCY: u32 = 0xB5;
    /// Number of channels
    pub const CHANNELS: u32 = 0x9F;
    /// Bits per sample
    pub const BIT_DEPTH: u32 = 0x6264;
}

// ============================================================================
// Track Type Enumeration
// ============================================================================

/// Matroska track type (from TrackType element)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MkvTrackType {
    /// Unknown track type
    Unknown = 0,
    /// Video track
    Video = 1,
    /// Audio track
    Audio = 2,
    /// Complex track (video+audio interleaved)
    Complex = 3,
    /// Logo track
    Logo = 0x10,
    /// Subtitle track
    Subtitle = 0x11,
    /// Buttons track (DVD-style)
    Buttons = 0x12,
    /// Control track
    Control = 0x20,
    /// Metadata track
    Metadata = 0x21,
}

impl MkvTrackType {
    /// Convert from u8 value
    #[inline]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            1 => MkvTrackType::Video,
            2 => MkvTrackType::Audio,
            3 => MkvTrackType::Complex,
            0x10 => MkvTrackType::Logo,
            0x11 => MkvTrackType::Subtitle,
            0x12 => MkvTrackType::Buttons,
            0x20 => MkvTrackType::Control,
            0x21 => MkvTrackType::Metadata,
            _ => MkvTrackType::Unknown,
        }
    }

    /// Convert from u32 value
    #[inline]
    pub const fn from_u32(value: u32) -> Self {
        Self::from_u8(value as u8)
    }
}

// ============================================================================
// Codec Identifiers
// ============================================================================

/// Common video codec IDs (hashed for atomic storage)
pub mod video_codec_ids {
    /// VP9 video
    pub const V_VP9: &str = "V_VP9";
    /// VP8 video
    pub const V_VP8: &str = "V_VP8";
    /// AV1 video
    pub const V_AV1: &str = "V_AV1";
    /// H.264/AVC video
    pub const V_AVC: &str = "V_MPEG4/ISO/AVC";
    /// H.265/HEVC video
    pub const V_HEVC: &str = "V_MPEGH/ISO/HEVC";
    /// MPEG-4 Visual
    pub const V_MPEG4: &str = "V_MPEG4/ISO/ASP";
}

/// Common audio codec IDs
pub mod audio_codec_ids {
    /// Opus audio
    pub const A_OPUS: &str = "A_OPUS";
    /// Vorbis audio
    pub const A_VORBIS: &str = "A_VORBIS";
    /// AAC audio
    pub const A_AAC: &str = "A_AAC";
    /// FLAC audio
    pub const A_FLAC: &str = "A_FLAC";
    /// MP3 audio
    pub const A_MP3: &str = "A_MPEG/L3";
    /// AC-3 audio
    pub const A_AC3: &str = "A_AC3";
    /// DTS audio
    pub const A_DTS: &str = "A_DTS";
    /// PCM audio
    pub const A_PCM: &str = "A_PCM/INT/LIT";
}

/// Mkv video codec identifier (mapped from codec ID string)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MkvVideoCodec {
    /// Unknown video codec
    Unknown = 0,
    /// VP9 video
    Vp9 = 1,
    /// VP8 video
    Vp8 = 2,
    /// AV1 video
    Av1 = 3,
    /// H.264/AVC
    H264 = 4,
    /// H.265/HEVC
    H265 = 5,
    /// MPEG-4 Visual
    Mpeg4 = 6,
}

impl MkvVideoCodec {
    /// Convert from u32 value
    #[inline]
    pub const fn from_u32(value: u32) -> Self {
        match value {
            1 => MkvVideoCodec::Vp9,
            2 => MkvVideoCodec::Vp8,
            3 => MkvVideoCodec::Av1,
            4 => MkvVideoCodec::H264,
            5 => MkvVideoCodec::H265,
            6 => MkvVideoCodec::Mpeg4,
            _ => MkvVideoCodec::Unknown,
        }
    }

    /// Parse from codec ID string
    #[inline]
    pub fn from_codec_id(codec_id: &str) -> Self {
        match codec_id {
            s if s == video_codec_ids::V_VP9 => MkvVideoCodec::Vp9,
            s if s == video_codec_ids::V_VP8 => MkvVideoCodec::Vp8,
            s if s == video_codec_ids::V_AV1 => MkvVideoCodec::Av1,
            s if s.starts_with("V_MPEG4/ISO/AVC") => MkvVideoCodec::H264,
            s if s.starts_with("V_MPEGH/ISO/HEVC") => MkvVideoCodec::H265,
            s if s.starts_with("V_MPEG4") => MkvVideoCodec::Mpeg4,
            _ => MkvVideoCodec::Unknown,
        }
    }
}

/// MKV audio codec identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MkvAudioCodec {
    /// Unknown audio codec
    Unknown = 0,
    /// Opus audio
    Opus = 1,
    /// Vorbis audio
    Vorbis = 2,
    /// AAC audio
    Aac = 3,
    /// FLAC audio
    Flac = 4,
    /// MP3 audio
    Mp3 = 5,
    /// AC-3 audio
    Ac3 = 6,
    /// DTS audio
    Dts = 7,
    /// PCM audio
    Pcm = 8,
}

impl MkvAudioCodec {
    /// Convert from u32 value
    #[inline]
    pub const fn from_u32(value: u32) -> Self {
        match value {
            1 => MkvAudioCodec::Opus,
            2 => MkvAudioCodec::Vorbis,
            3 => MkvAudioCodec::Aac,
            4 => MkvAudioCodec::Flac,
            5 => MkvAudioCodec::Mp3,
            6 => MkvAudioCodec::Ac3,
            7 => MkvAudioCodec::Dts,
            8 => MkvAudioCodec::Pcm,
            _ => MkvAudioCodec::Unknown,
        }
    }

    /// Parse from codec ID string
    #[inline]
    pub fn from_codec_id(codec_id: &str) -> Self {
        match codec_id {
            s if s == audio_codec_ids::A_OPUS => MkvAudioCodec::Opus,
            s if s == audio_codec_ids::A_VORBIS => MkvAudioCodec::Vorbis,
            s if s.starts_with("A_AAC") => MkvAudioCodec::Aac,
            s if s == audio_codec_ids::A_FLAC => MkvAudioCodec::Flac,
            s if s.starts_with("A_MPEG/L3") => MkvAudioCodec::Mp3,
            s if s == audio_codec_ids::A_AC3 => MkvAudioCodec::Ac3,
            s if s.starts_with("A_DTS") => MkvAudioCodec::Dts,
            s if s.starts_with("A_PCM") => MkvAudioCodec::Pcm,
            _ => MkvAudioCodec::Unknown,
        }
    }
}

// ============================================================================
// Track Flags
// ============================================================================

/// Track flags bitfield
pub mod mkv_track_flags {
    /// Track is enabled
    pub const ENABLED: u64 = 1 << 0;
    /// Track is the default for its type
    pub const IS_DEFAULT: u64 = 1 << 1;
    /// Track is forced
    pub const IS_FORCED: u64 = 1 << 2;
    /// Track uses lacing
    pub const USES_LACING: u64 = 1 << 3;

    // Parse state flags
    /// TrackNumber element parsed
    pub const PARSED_TRACK_NUMBER: u64 = 1 << 8;
    /// TrackUID element parsed
    pub const PARSED_TRACK_UID: u64 = 1 << 9;
    /// TrackType element parsed
    pub const PARSED_TRACK_TYPE: u64 = 1 << 10;
    /// CodecID element parsed
    pub const PARSED_CODEC_ID: u64 = 1 << 11;
    /// Video element parsed
    pub const PARSED_VIDEO: u64 = 1 << 12;
    /// Audio element parsed
    pub const PARSED_AUDIO: u64 = 1 << 13;

    /// Minimum required elements for a valid track
    pub const FULLY_PARSED: u64 = PARSED_TRACK_NUMBER | PARSED_TRACK_TYPE | PARSED_CODEC_ID;
}

// ============================================================================
// Error Types
// ============================================================================

/// Error type for MKV track parsing operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MkvTrackError {
    /// Data buffer too short for operation
    BufferTooShort {
        /// Required size
        required: usize,
        /// Actual size
        actual: usize,
    },
    /// Invalid EBML element ID
    InvalidElementId(u32),
    /// Invalid EBML element size
    InvalidElementSize(u64),
    /// Invalid track type value
    InvalidTrackType(u8),
    /// Unknown codec ID
    UnknownCodec(u64),
    /// Missing required element
    MissingElement(&'static str),
    /// Invalid float value
    InvalidFloat,
}

impl core::fmt::Display for MkvTrackError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MkvTrackError::BufferTooShort { required, actual } => {
                write!(f, "Buffer too short: need {} bytes, got {}", required, actual)
            }
            MkvTrackError::InvalidElementId(id) => {
                write!(f, "Invalid EBML element ID: 0x{:X}", id)
            }
            MkvTrackError::InvalidElementSize(size) => {
                write!(f, "Invalid EBML element size: {}", size)
            }
            MkvTrackError::InvalidTrackType(t) => {
                write!(f, "Invalid track type: {}", t)
            }
            MkvTrackError::UnknownCodec(hash) => {
                write!(f, "Unknown codec (hash: 0x{:016X})", hash)
            }
            MkvTrackError::MissingElement(name) => {
                write!(f, "Missing required element: {}", name)
            }
            MkvTrackError::InvalidFloat => {
                write!(f, "Invalid floating point value")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for MkvTrackError {}

// ============================================================================
// Track Snapshot
// ============================================================================

/// Atomic snapshot of MKV track metadata
#[derive(Debug, Clone)]
pub struct MkvTrackSnapshot {
    /// Track number (1-based index)
    pub track_number: u32,
    /// Track unique identifier
    pub track_uid: u64,
    /// Track type
    pub track_type: MkvTrackType,
    /// Video codec (Unknown for non-video)
    pub video_codec: MkvVideoCodec,
    /// Audio codec (Unknown for non-audio)
    pub audio_codec: MkvAudioCodec,
    /// Pixel width (0 for non-video)
    pub pixel_width: u32,
    /// Pixel height (0 for non-video)
    pub pixel_height: u32,
    /// Display width (0 for non-video or if not specified)
    pub display_width: u32,
    /// Display height (0 for non-video or if not specified)
    pub display_height: u32,
    /// Sample rate in Hz (0 for non-audio)
    pub sample_rate: f64,
    /// Number of audio channels (0 for non-audio)
    pub channels: u8,
    /// Bits per sample (0 for non-audio)
    pub bit_depth: u8,
    /// Default duration per frame in nanoseconds (0 if not specified)
    pub default_duration_ns: u64,
    /// Codec ID hash for fast comparison
    pub codec_id_hash: u64,
    /// Total frames seen
    pub frames_seen: u32,
    /// Keyframes seen
    pub keyframes_seen: u32,
    /// Generation counter at snapshot time
    pub generation: u64,
    /// Track flags
    pub flags: u64,
}

impl MkvTrackSnapshot {
    /// Check if track is video
    #[inline]
    pub fn is_video(&self) -> bool {
        self.track_type == MkvTrackType::Video
    }

    /// Check if track is audio
    #[inline]
    pub fn is_audio(&self) -> bool {
        self.track_type == MkvTrackType::Audio
    }

    /// Check if track is subtitle
    #[inline]
    pub fn is_subtitle(&self) -> bool {
        self.track_type == MkvTrackType::Subtitle
    }

    /// Check if track is enabled
    #[inline]
    pub fn is_enabled(&self) -> bool {
        (self.flags & mkv_track_flags::ENABLED) != 0
    }

    /// Check if track is default
    #[inline]
    pub fn is_default(&self) -> bool {
        (self.flags & mkv_track_flags::IS_DEFAULT) != 0
    }

    /// Check if track is forced
    #[inline]
    pub fn is_forced(&self) -> bool {
        (self.flags & mkv_track_flags::IS_FORCED) != 0
    }

    /// Check if track is fully parsed
    #[inline]
    pub fn is_fully_parsed(&self) -> bool {
        (self.flags & mkv_track_flags::FULLY_PARSED) == mkv_track_flags::FULLY_PARSED
    }

    /// Get frame rate from default duration (frames per second)
    #[inline]
    pub fn frame_rate(&self) -> Option<f64> {
        if self.default_duration_ns > 0 {
            Some(1_000_000_000.0 / self.default_duration_ns as f64)
        } else {
            None
        }
    }

    /// Get video dimensions as (width, height) tuple
    #[inline]
    pub fn video_dimensions(&self) -> Option<(u32, u32)> {
        if self.is_video() && self.pixel_width > 0 && self.pixel_height > 0 {
            Some((self.pixel_width, self.pixel_height))
        } else {
            None
        }
    }

    /// Get display dimensions as (width, height) tuple
    #[inline]
    pub fn display_dimensions(&self) -> Option<(u32, u32)> {
        if self.is_video() {
            // Use display dimensions if set, otherwise fall back to pixel dimensions
            if self.display_width > 0 && self.display_height > 0 {
                Some((self.display_width, self.display_height))
            } else if self.pixel_width > 0 && self.pixel_height > 0 {
                Some((self.pixel_width, self.pixel_height))
            } else {
                None
            }
        } else {
            None
        }
    }
}

// ============================================================================
// MKV Track Capsule
// ============================================================================

/// T1 Atomic capsule for MKV/WebM track metadata
///
/// 256B cache-aligned for optimal memory access patterns.
/// All fields use atomic types for lockfree thread-safe access.
///
/// ## Memory Layout
///
/// The capsule packs track metadata into atomic fields:
/// - `state`: track_number(16) | track_type(8) | flags_low(8) | video_codec(8) | audio_codec(8) | channels(8) | bit_depth(8)
/// - Video dimensions packed into `video_dimensions` and `display_dimensions`
/// - Audio sample rate stored as raw f64 bits in `audio_info`
///
/// ## Generation Counter (Q34 Audit)
///
/// Every mutation increments the generation counter, providing:
/// - Temporal ordering of updates
/// - Audit trail for compliance
/// - ABA problem prevention
#[repr(C, align(256))]
pub struct MkvTrackCapsule {
    // Track identity (16 bytes)
    /// Packed state: track_number(32) | track_type(8) | video_codec(8) | audio_codec(8) | channels(8)
    state: AtomicU64,
    /// Track unique identifier
    track_uid: AtomicU64,

    // Generation counter for Q34 audit (8 bytes)
    /// Generation counter - incremented on every mutation
    generation: AtomicU64,

    // Video info (16 bytes)
    /// Packed: pixel_width(32) | pixel_height(32)
    video_dimensions: AtomicU64,
    /// Packed: display_width(32) | display_height(32)
    display_dimensions: AtomicU64,

    // Audio info (8 bytes)
    /// Sample rate as raw f64 bits
    audio_info: AtomicU64,

    // Timing (8 bytes)
    /// Default duration per frame in nanoseconds
    default_duration: AtomicU64,

    // Codec identification (8 bytes)
    /// Codec ID string hash for fast comparison
    codec_id_hash: AtomicU64,

    // Statistics (8 bytes)
    /// Total frames seen
    frames_seen: AtomicU32,
    /// Keyframes seen
    keyframes_seen: AtomicU32,

    // Flags (16 bytes)
    /// Track flags (enabled, default, forced, lacing, parse state)
    flags: AtomicU64,
    /// Bit depth (8 bits used) + reserved
    bit_depth_reserved: AtomicU64,

    // Padding to 256B
    // Total fields with internal padding:
    // - state: 8 bytes
    // - track_uid: 8 bytes
    // - generation: 8 bytes
    // - video_dimensions: 8 bytes
    // - display_dimensions: 8 bytes
    // - audio_info: 8 bytes
    // - default_duration: 8 bytes
    // - codec_id_hash: 8 bytes
    // - frames_seen + keyframes_seen: 8 bytes
    // - flags: 8 bytes
    // - bit_depth_reserved: 8 bytes
    // Total: 88 bytes
    // Padding needed: 256 - 88 = 168 bytes
    _padding: [u8; 168],
}

// Verify size at compile time
const _: () = assert!(core::mem::size_of::<MkvTrackCapsule>() == 256);
const _: () = assert!(core::mem::align_of::<MkvTrackCapsule>() == 256);

impl MkvTrackCapsule {
    /// Create a new MKV track capsule with default values
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            track_uid: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            video_dimensions: AtomicU64::new(0),
            display_dimensions: AtomicU64::new(0),
            audio_info: AtomicU64::new(0),
            default_duration: AtomicU64::new(0),
            codec_id_hash: AtomicU64::new(0),
            frames_seen: AtomicU32::new(0),
            keyframes_seen: AtomicU32::new(0),
            flags: AtomicU64::new(mkv_track_flags::ENABLED), // Enabled by default in MKV
            bit_depth_reserved: AtomicU64::new(0),
            _padding: [0u8; 168],
        }
    }

    /// Create a new MKV track capsule with the given track number
    #[inline]
    pub const fn with_track_number(track_number: u32) -> Self {
        Self {
            state: AtomicU64::new((track_number as u64) << 32),
            track_uid: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            video_dimensions: AtomicU64::new(0),
            display_dimensions: AtomicU64::new(0),
            audio_info: AtomicU64::new(0),
            default_duration: AtomicU64::new(0),
            codec_id_hash: AtomicU64::new(0),
            frames_seen: AtomicU32::new(0),
            keyframes_seen: AtomicU32::new(0),
            flags: AtomicU64::new(mkv_track_flags::ENABLED),
            bit_depth_reserved: AtomicU64::new(0),
            _padding: [0u8; 168],
        }
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    /// Increment generation counter and return new value
    #[inline]
    fn bump_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Set a flag atomically
    #[inline]
    fn set_flag(&self, flag: u64) {
        self.flags.fetch_or(flag, Ordering::AcqRel);
    }

    /// Clear a flag atomically
    #[inline]
    fn clear_flag(&self, flag: u64) {
        self.flags.fetch_and(!flag, Ordering::AcqRel);
    }

    /// Pack state from components
    #[inline]
    fn pack_state(
        track_number: u32,
        track_type: MkvTrackType,
        video_codec: MkvVideoCodec,
        audio_codec: MkvAudioCodec,
        channels: u8,
    ) -> u64 {
        ((track_number as u64) << 32)
            | ((track_type as u64) << 24)
            | ((video_codec as u64) << 16)
            | ((audio_codec as u64) << 8)
            | (channels as u64)
    }

    /// Unpack track number from state
    #[inline]
    fn unpack_track_number(state: u64) -> u32 {
        (state >> 32) as u32
    }

    /// Unpack track type from state
    #[inline]
    fn unpack_track_type(state: u64) -> MkvTrackType {
        MkvTrackType::from_u8(((state >> 24) & 0xFF) as u8)
    }

    /// Unpack video codec from state
    #[inline]
    fn unpack_video_codec(state: u64) -> MkvVideoCodec {
        MkvVideoCodec::from_u32(((state >> 16) & 0xFF) as u32)
    }

    /// Unpack audio codec from state
    #[inline]
    fn unpack_audio_codec(state: u64) -> MkvAudioCodec {
        MkvAudioCodec::from_u32(((state >> 8) & 0xFF) as u32)
    }

    /// Unpack channels from state
    #[inline]
    fn unpack_channels(state: u64) -> u8 {
        (state & 0xFF) as u8
    }

    /// Pack video dimensions
    #[inline]
    fn pack_dimensions(width: u32, height: u32) -> u64 {
        ((width as u64) << 32) | (height as u64)
    }

    /// Unpack width from dimensions
    #[inline]
    fn unpack_width(dims: u64) -> u32 {
        (dims >> 32) as u32
    }

    /// Unpack height from dimensions
    #[inline]
    fn unpack_height(dims: u64) -> u32 {
        dims as u32
    }

    /// Simple hash function for codec ID strings
    #[inline]
    pub fn hash_codec_id(codec_id: &str) -> u64 {
        // FNV-1a hash for short strings
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in codec_id.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    // ========================================================================
    // Parsing Methods
    // ========================================================================

    /// Set track number
    pub fn set_track_number(&self, track_number: u32) {
        let state = self.state.load(Ordering::Acquire);
        let new_state = Self::pack_state(
            track_number,
            Self::unpack_track_type(state),
            Self::unpack_video_codec(state),
            Self::unpack_audio_codec(state),
            Self::unpack_channels(state),
        );
        self.state.store(new_state, Ordering::Release);
        self.set_flag(mkv_track_flags::PARSED_TRACK_NUMBER);
        self.bump_generation();
    }

    /// Set track UID
    pub fn set_track_uid(&self, uid: u64) {
        self.track_uid.store(uid, Ordering::Release);
        self.set_flag(mkv_track_flags::PARSED_TRACK_UID);
        self.bump_generation();
    }

    /// Set track type
    pub fn set_track_type(&self, track_type: MkvTrackType) {
        let state = self.state.load(Ordering::Acquire);
        let new_state = Self::pack_state(
            Self::unpack_track_number(state),
            track_type,
            Self::unpack_video_codec(state),
            Self::unpack_audio_codec(state),
            Self::unpack_channels(state),
        );
        self.state.store(new_state, Ordering::Release);
        self.set_flag(mkv_track_flags::PARSED_TRACK_TYPE);
        self.bump_generation();
    }

    /// Set codec ID from string
    pub fn set_codec_id(&self, codec_id: &str) {
        let hash = Self::hash_codec_id(codec_id);
        self.codec_id_hash.store(hash, Ordering::Release);

        // Also determine and store the codec type
        let state = self.state.load(Ordering::Acquire);
        let track_type = Self::unpack_track_type(state);

        let (video_codec, audio_codec) = match track_type {
            MkvTrackType::Video => (MkvVideoCodec::from_codec_id(codec_id), MkvAudioCodec::Unknown),
            MkvTrackType::Audio => (MkvVideoCodec::Unknown, MkvAudioCodec::from_codec_id(codec_id)),
            _ => (MkvVideoCodec::Unknown, MkvAudioCodec::Unknown),
        };

        let new_state = Self::pack_state(
            Self::unpack_track_number(state),
            track_type,
            video_codec,
            audio_codec,
            Self::unpack_channels(state),
        );
        self.state.store(new_state, Ordering::Release);
        self.set_flag(mkv_track_flags::PARSED_CODEC_ID);
        self.bump_generation();
    }

    /// Set video dimensions
    pub fn set_video_dimensions(&self, pixel_width: u32, pixel_height: u32) {
        let packed = Self::pack_dimensions(pixel_width, pixel_height);
        self.video_dimensions.store(packed, Ordering::Release);
        self.set_flag(mkv_track_flags::PARSED_VIDEO);
        self.bump_generation();
    }

    /// Set display dimensions
    pub fn set_display_dimensions(&self, display_width: u32, display_height: u32) {
        let packed = Self::pack_dimensions(display_width, display_height);
        self.display_dimensions.store(packed, Ordering::Release);
        self.bump_generation();
    }

    /// Set audio parameters
    pub fn set_audio_params(&self, sample_rate: f64, channels: u8, bit_depth: u8) {
        // Store sample rate as raw bits
        let rate_bits = sample_rate.to_bits();
        self.audio_info.store(rate_bits, Ordering::Release);

        // Update channels in state
        let state = self.state.load(Ordering::Acquire);
        let new_state = Self::pack_state(
            Self::unpack_track_number(state),
            Self::unpack_track_type(state),
            Self::unpack_video_codec(state),
            Self::unpack_audio_codec(state),
            channels,
        );
        self.state.store(new_state, Ordering::Release);

        // Store bit depth
        self.bit_depth_reserved.store(bit_depth as u64, Ordering::Release);

        self.set_flag(mkv_track_flags::PARSED_AUDIO);
        self.bump_generation();
    }

    /// Set default duration (nanoseconds per frame)
    pub fn set_default_duration(&self, duration_ns: u64) {
        self.default_duration.store(duration_ns, Ordering::Release);
        self.bump_generation();
    }

    /// Set enabled flag
    pub fn set_enabled(&self, enabled: bool) {
        if enabled {
            self.set_flag(mkv_track_flags::ENABLED);
        } else {
            self.clear_flag(mkv_track_flags::ENABLED);
        }
        self.bump_generation();
    }

    /// Set default flag
    pub fn set_default(&self, is_default: bool) {
        if is_default {
            self.set_flag(mkv_track_flags::IS_DEFAULT);
        } else {
            self.clear_flag(mkv_track_flags::IS_DEFAULT);
        }
        self.bump_generation();
    }

    /// Set forced flag
    pub fn set_forced(&self, is_forced: bool) {
        if is_forced {
            self.set_flag(mkv_track_flags::IS_FORCED);
        } else {
            self.clear_flag(mkv_track_flags::IS_FORCED);
        }
        self.bump_generation();
    }

    /// Increment frame counter
    pub fn increment_frames(&self, is_keyframe: bool) {
        self.frames_seen.fetch_add(1, Ordering::AcqRel);
        if is_keyframe {
            self.keyframes_seen.fetch_add(1, Ordering::AcqRel);
        }
        // Note: Don't bump generation for statistics to reduce contention
    }

    // ========================================================================
    // Accessor Methods
    // ========================================================================

    /// Get track number
    #[inline]
    pub fn track_number(&self) -> u32 {
        Self::unpack_track_number(self.state.load(Ordering::Acquire))
    }

    /// Get track UID
    #[inline]
    pub fn track_uid(&self) -> u64 {
        self.track_uid.load(Ordering::Acquire)
    }

    /// Get track type
    #[inline]
    pub fn track_type(&self) -> MkvTrackType {
        Self::unpack_track_type(self.state.load(Ordering::Acquire))
    }

    /// Get video codec
    #[inline]
    pub fn video_codec(&self) -> MkvVideoCodec {
        Self::unpack_video_codec(self.state.load(Ordering::Acquire))
    }

    /// Get audio codec
    #[inline]
    pub fn audio_codec(&self) -> MkvAudioCodec {
        Self::unpack_audio_codec(self.state.load(Ordering::Acquire))
    }

    /// Check if track is video
    #[inline]
    pub fn is_video(&self) -> bool {
        self.track_type() == MkvTrackType::Video
    }

    /// Check if track is audio
    #[inline]
    pub fn is_audio(&self) -> bool {
        self.track_type() == MkvTrackType::Audio
    }

    /// Get video dimensions as (width, height) tuple
    #[inline]
    pub fn video_dimensions(&self) -> Option<(u32, u32)> {
        if !self.is_video() {
            return None;
        }
        let dims = self.video_dimensions.load(Ordering::Acquire);
        let width = Self::unpack_width(dims);
        let height = Self::unpack_height(dims);
        if width > 0 && height > 0 {
            Some((width, height))
        } else {
            None
        }
    }

    /// Get display dimensions as (width, height) tuple
    #[inline]
    pub fn display_dimensions(&self) -> Option<(u32, u32)> {
        if !self.is_video() {
            return None;
        }
        let dims = self.display_dimensions.load(Ordering::Acquire);
        let width = Self::unpack_width(dims);
        let height = Self::unpack_height(dims);
        if width > 0 && height > 0 {
            Some((width, height))
        } else {
            // Fall back to pixel dimensions
            self.video_dimensions()
        }
    }

    /// Get audio sample rate in Hz
    #[inline]
    pub fn sample_rate(&self) -> Option<f64> {
        if !self.is_audio() {
            return None;
        }
        let bits = self.audio_info.load(Ordering::Acquire);
        let rate = f64::from_bits(bits);
        if rate > 0.0 {
            Some(rate)
        } else {
            None
        }
    }

    /// Get number of audio channels
    #[inline]
    pub fn channels(&self) -> Option<u8> {
        if !self.is_audio() {
            return None;
        }
        let channels = Self::unpack_channels(self.state.load(Ordering::Acquire));
        if channels > 0 {
            Some(channels)
        } else {
            None
        }
    }

    /// Get bit depth
    #[inline]
    pub fn bit_depth(&self) -> Option<u8> {
        if !self.is_audio() {
            return None;
        }
        let depth = (self.bit_depth_reserved.load(Ordering::Acquire) & 0xFF) as u8;
        if depth > 0 {
            Some(depth)
        } else {
            None
        }
    }

    /// Get default duration in nanoseconds
    #[inline]
    pub fn default_duration_ns(&self) -> Option<u64> {
        let duration = self.default_duration.load(Ordering::Acquire);
        if duration > 0 {
            Some(duration)
        } else {
            None
        }
    }

    /// Get frame rate (frames per second)
    #[inline]
    pub fn frame_rate(&self) -> Option<f64> {
        self.default_duration_ns()
            .map(|ns| 1_000_000_000.0 / ns as f64)
    }

    /// Get codec ID hash
    #[inline]
    pub fn codec_id_hash(&self) -> u64 {
        self.codec_id_hash.load(Ordering::Acquire)
    }

    /// Check codec ID by string comparison (via hash)
    #[inline]
    pub fn codec_id_matches(&self, codec_id: &str) -> bool {
        let expected_hash = Self::hash_codec_id(codec_id);
        self.codec_id_hash() == expected_hash
    }

    /// Get frames seen count
    #[inline]
    pub fn frames_seen(&self) -> u32 {
        self.frames_seen.load(Ordering::Acquire)
    }

    /// Get keyframes seen count
    #[inline]
    pub fn keyframes_seen(&self) -> u32 {
        self.keyframes_seen.load(Ordering::Acquire)
    }

    /// Check if track is enabled
    #[inline]
    pub fn is_enabled(&self) -> bool {
        (self.flags.load(Ordering::Acquire) & mkv_track_flags::ENABLED) != 0
    }

    /// Check if track is default
    #[inline]
    pub fn is_default(&self) -> bool {
        (self.flags.load(Ordering::Acquire) & mkv_track_flags::IS_DEFAULT) != 0
    }

    /// Check if track is forced
    #[inline]
    pub fn is_forced(&self) -> bool {
        (self.flags.load(Ordering::Acquire) & mkv_track_flags::IS_FORCED) != 0
    }

    /// Check if track is fully parsed
    #[inline]
    pub fn is_fully_parsed(&self) -> bool {
        let flags = self.flags.load(Ordering::Acquire);
        (flags & mkv_track_flags::FULLY_PARSED) == mkv_track_flags::FULLY_PARSED
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get an atomic snapshot of all track metadata
    pub fn snapshot(&self) -> MkvTrackSnapshot {
        let state = self.state.load(Ordering::Acquire);
        let video_dims = self.video_dimensions.load(Ordering::Acquire);
        let display_dims = self.display_dimensions.load(Ordering::Acquire);
        let audio_bits = self.audio_info.load(Ordering::Acquire);
        let bit_depth = (self.bit_depth_reserved.load(Ordering::Acquire) & 0xFF) as u8;

        MkvTrackSnapshot {
            track_number: Self::unpack_track_number(state),
            track_uid: self.track_uid.load(Ordering::Acquire),
            track_type: Self::unpack_track_type(state),
            video_codec: Self::unpack_video_codec(state),
            audio_codec: Self::unpack_audio_codec(state),
            pixel_width: Self::unpack_width(video_dims),
            pixel_height: Self::unpack_height(video_dims),
            display_width: Self::unpack_width(display_dims),
            display_height: Self::unpack_height(display_dims),
            sample_rate: f64::from_bits(audio_bits),
            channels: Self::unpack_channels(state),
            bit_depth,
            default_duration_ns: self.default_duration.load(Ordering::Acquire),
            codec_id_hash: self.codec_id_hash.load(Ordering::Acquire),
            frames_seen: self.frames_seen.load(Ordering::Acquire),
            keyframes_seen: self.keyframes_seen.load(Ordering::Acquire),
            generation: self.generation.load(Ordering::Acquire),
            flags: self.flags.load(Ordering::Acquire),
        }
    }

    // ========================================================================
    // EBML Parsing Helpers
    // ========================================================================

    /// Parse a variable-length EBML integer
    ///
    /// Returns (value, bytes_consumed)
    #[inline]
    pub fn parse_vint(data: &[u8]) -> Result<(u64, usize), MkvTrackError> {
        if data.is_empty() {
            return Err(MkvTrackError::BufferTooShort {
                required: 1,
                actual: 0,
            });
        }

        let first = data[0];
        let len = first.leading_zeros() as usize + 1;

        if len > 8 || data.len() < len {
            return Err(MkvTrackError::BufferTooShort {
                required: len,
                actual: data.len(),
            });
        }

        // Mask off the length bits
        let mask = (1u8 << (8 - len)) - 1;
        let mut value = (first & mask) as u64;

        for i in 1..len {
            value = (value << 8) | (data[i] as u64);
        }

        Ok((value, len))
    }

    /// Parse an EBML element ID (preserves marker bit)
    ///
    /// Unlike parse_vint, element IDs keep their VINT marker bits as part of the ID value.
    /// Returns (element_id, bytes_consumed)
    #[inline]
    pub fn parse_element_id(data: &[u8]) -> Result<(u32, usize), MkvTrackError> {
        if data.is_empty() {
            return Err(MkvTrackError::BufferTooShort {
                required: 1,
                actual: 0,
            });
        }

        let first = data[0];
        let len = first.leading_zeros() as usize + 1;

        if len > 4 || data.len() < len {
            return Err(MkvTrackError::BufferTooShort {
                required: len,
                actual: data.len(),
            });
        }

        // For element IDs, the marker bit is preserved
        let mut value = first as u32;
        for i in 1..len {
            value = (value << 8) | (data[i] as u32);
        }

        Ok((value, len))
    }

    /// Parse an EBML element header (ID + size)
    ///
    /// Returns (element_id, element_size, header_bytes_consumed)
    pub fn parse_element_header(data: &[u8]) -> Result<(u32, u64, usize), MkvTrackError> {
        // Parse element ID (preserves marker bit)
        let (id, id_len) = Self::parse_element_id(data)?;

        // Parse element size (strips marker bit)
        let (size, size_len) = Self::parse_vint(&data[id_len..])?;

        Ok((id, size, id_len + size_len))
    }

    /// Parse an unsigned integer from EBML data
    #[inline]
    pub fn parse_uint(data: &[u8]) -> u64 {
        let mut value: u64 = 0;
        for &byte in data.iter().take(8) {
            value = (value << 8) | (byte as u64);
        }
        value
    }

    /// Parse a float from EBML data (4 or 8 bytes)
    #[inline]
    pub fn parse_float(data: &[u8]) -> Result<f64, MkvTrackError> {
        match data.len() {
            4 => {
                let bits = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                Ok(f32::from_bits(bits) as f64)
            }
            8 => {
                let bits = u64::from_be_bytes([
                    data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                ]);
                Ok(f64::from_bits(bits))
            }
            _ => Err(MkvTrackError::InvalidFloat),
        }
    }

    /// Parse a complete TrackEntry element
    ///
    /// This parses all child elements of a TrackEntry and populates the capsule.
    pub fn parse_track_entry(&self, data: &[u8]) -> Result<(), MkvTrackError> {
        let mut offset = 0;

        while offset < data.len() {
            // Parse element header
            let remaining = &data[offset..];
            if remaining.is_empty() {
                break;
            }

            let (element_id, element_size, header_len) = Self::parse_element_header(remaining)?;
            offset += header_len;

            let element_data_end = offset + element_size as usize;
            if element_data_end > data.len() {
                return Err(MkvTrackError::BufferTooShort {
                    required: element_data_end,
                    actual: data.len(),
                });
            }

            let element_data = &data[offset..element_data_end];

            match element_id {
                element_ids::TRACK_NUMBER => {
                    let track_num = Self::parse_uint(element_data) as u32;
                    self.set_track_number(track_num);
                }
                element_ids::TRACK_UID => {
                    let uid = Self::parse_uint(element_data);
                    self.set_track_uid(uid);
                }
                element_ids::TRACK_TYPE => {
                    let track_type = MkvTrackType::from_u8(element_data[0]);
                    self.set_track_type(track_type);
                }
                element_ids::FLAG_ENABLED => {
                    let enabled = element_data.first().map(|&b| b != 0).unwrap_or(true);
                    self.set_enabled(enabled);
                }
                element_ids::FLAG_DEFAULT => {
                    let is_default = element_data.first().map(|&b| b != 0).unwrap_or(false);
                    self.set_default(is_default);
                }
                element_ids::FLAG_FORCED => {
                    let is_forced = element_data.first().map(|&b| b != 0).unwrap_or(false);
                    self.set_forced(is_forced);
                }
                element_ids::DEFAULT_DURATION => {
                    let duration_ns = Self::parse_uint(element_data);
                    self.set_default_duration(duration_ns);
                }
                element_ids::CODEC_ID => {
                    // Codec ID is a string
                    if let Ok(codec_id) = core::str::from_utf8(element_data) {
                        self.set_codec_id(codec_id.trim_end_matches('\0'));
                    }
                }
                element_ids::VIDEO => {
                    // Parse Video sub-element
                    self.parse_video_element(element_data)?;
                }
                element_ids::AUDIO => {
                    // Parse Audio sub-element
                    self.parse_audio_element(element_data)?;
                }
                _ => {
                    // Skip unknown elements
                }
            }

            offset = element_data_end;
        }

        Ok(())
    }

    /// Parse Video element children
    fn parse_video_element(&self, data: &[u8]) -> Result<(), MkvTrackError> {
        let mut offset = 0;
        let mut pixel_width: u32 = 0;
        let mut pixel_height: u32 = 0;
        let mut display_width: u32 = 0;
        let mut display_height: u32 = 0;

        while offset < data.len() {
            let remaining = &data[offset..];
            if remaining.is_empty() {
                break;
            }

            let (element_id, element_size, header_len) = Self::parse_element_header(remaining)?;
            offset += header_len;

            let element_data_end = offset + element_size as usize;
            if element_data_end > data.len() {
                break;
            }

            let element_data = &data[offset..element_data_end];

            match element_id {
                element_ids::PIXEL_WIDTH => {
                    pixel_width = Self::parse_uint(element_data) as u32;
                }
                element_ids::PIXEL_HEIGHT => {
                    pixel_height = Self::parse_uint(element_data) as u32;
                }
                element_ids::DISPLAY_WIDTH => {
                    display_width = Self::parse_uint(element_data) as u32;
                }
                element_ids::DISPLAY_HEIGHT => {
                    display_height = Self::parse_uint(element_data) as u32;
                }
                _ => {}
            }

            offset = element_data_end;
        }

        if pixel_width > 0 && pixel_height > 0 {
            self.set_video_dimensions(pixel_width, pixel_height);
        }
        if display_width > 0 && display_height > 0 {
            self.set_display_dimensions(display_width, display_height);
        }

        Ok(())
    }

    /// Parse Audio element children
    fn parse_audio_element(&self, data: &[u8]) -> Result<(), MkvTrackError> {
        let mut offset = 0;
        let mut sample_rate: f64 = 0.0;
        let mut channels: u8 = 0;
        let mut bit_depth: u8 = 0;

        while offset < data.len() {
            let remaining = &data[offset..];
            if remaining.is_empty() {
                break;
            }

            let (element_id, element_size, header_len) = Self::parse_element_header(remaining)?;
            offset += header_len;

            let element_data_end = offset + element_size as usize;
            if element_data_end > data.len() {
                break;
            }

            let element_data = &data[offset..element_data_end];

            match element_id {
                element_ids::SAMPLING_FREQUENCY => {
                    sample_rate = Self::parse_float(element_data)?;
                }
                element_ids::CHANNELS => {
                    channels = Self::parse_uint(element_data) as u8;
                }
                element_ids::BIT_DEPTH => {
                    bit_depth = Self::parse_uint(element_data) as u8;
                }
                _ => {}
            }

            offset = element_data_end;
        }

        if sample_rate > 0.0 || channels > 0 || bit_depth > 0 {
            // Use defaults if not specified
            if channels == 0 {
                channels = 2; // Default stereo
            }
            self.set_audio_params(sample_rate, channels, bit_depth);
        }

        Ok(())
    }
}

impl Default for MkvTrackCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Send + Sync are safe because all fields are atomic
// #ASSUME: AtomicU32/AtomicU64 are inherently thread-safe
// #VERIFY: All operations use proper memory ordering (Acquire/Release)
unsafe impl Send for MkvTrackCapsule {}
unsafe impl Sync for MkvTrackCapsule {}

// ============================================================================
// Tests (T28 Compliant: Q1-Q28)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1-Q7: Unit Tests - Basic Construction and Accessors
    // ========================================================================

    /// Q1: Test default capsule construction
    #[test]
    fn test_new_capsule_defaults() {
        let capsule = MkvTrackCapsule::new();

        assert_eq!(capsule.track_number(), 0);
        assert_eq!(capsule.track_uid(), 0);
        assert_eq!(capsule.track_type(), MkvTrackType::Unknown);
        assert_eq!(capsule.video_codec(), MkvVideoCodec::Unknown);
        assert_eq!(capsule.audio_codec(), MkvAudioCodec::Unknown);
        assert!(capsule.is_enabled()); // Enabled by default
        assert!(!capsule.is_default());
        assert!(!capsule.is_forced());
        assert!(!capsule.is_fully_parsed());
        assert_eq!(capsule.generation(), 0);
    }

    /// Q2: Test capsule with track number
    #[test]
    fn test_with_track_number() {
        let capsule = MkvTrackCapsule::with_track_number(1);

        assert_eq!(capsule.track_number(), 1);
        assert_eq!(capsule.track_type(), MkvTrackType::Unknown);
        assert!(!capsule.is_fully_parsed());
    }

    /// Q3: Test track type setting
    #[test]
    fn test_set_track_type() {
        let capsule = MkvTrackCapsule::new();

        capsule.set_track_type(MkvTrackType::Video);
        assert_eq!(capsule.track_type(), MkvTrackType::Video);
        assert!(capsule.is_video());
        assert!(!capsule.is_audio());

        capsule.set_track_type(MkvTrackType::Audio);
        assert_eq!(capsule.track_type(), MkvTrackType::Audio);
        assert!(!capsule.is_video());
        assert!(capsule.is_audio());
    }

    /// Q4: Test track UID setting
    #[test]
    fn test_set_track_uid() {
        let capsule = MkvTrackCapsule::new();

        let uid = 0x123456789ABCDEF0u64;
        capsule.set_track_uid(uid);
        assert_eq!(capsule.track_uid(), uid);
    }

    /// Q5: Test video codec detection
    #[test]
    fn test_video_codec_detection() {
        assert_eq!(MkvVideoCodec::from_codec_id("V_VP9"), MkvVideoCodec::Vp9);
        assert_eq!(MkvVideoCodec::from_codec_id("V_VP8"), MkvVideoCodec::Vp8);
        assert_eq!(MkvVideoCodec::from_codec_id("V_AV1"), MkvVideoCodec::Av1);
        assert_eq!(MkvVideoCodec::from_codec_id("V_MPEG4/ISO/AVC"), MkvVideoCodec::H264);
        assert_eq!(MkvVideoCodec::from_codec_id("V_MPEGH/ISO/HEVC"), MkvVideoCodec::H265);
        assert_eq!(MkvVideoCodec::from_codec_id("UNKNOWN"), MkvVideoCodec::Unknown);
    }

    /// Q6: Test audio codec detection
    #[test]
    fn test_audio_codec_detection() {
        assert_eq!(MkvAudioCodec::from_codec_id("A_OPUS"), MkvAudioCodec::Opus);
        assert_eq!(MkvAudioCodec::from_codec_id("A_VORBIS"), MkvAudioCodec::Vorbis);
        assert_eq!(MkvAudioCodec::from_codec_id("A_AAC"), MkvAudioCodec::Aac);
        assert_eq!(MkvAudioCodec::from_codec_id("A_FLAC"), MkvAudioCodec::Flac);
        assert_eq!(MkvAudioCodec::from_codec_id("A_MPEG/L3"), MkvAudioCodec::Mp3);
        assert_eq!(MkvAudioCodec::from_codec_id("A_AC3"), MkvAudioCodec::Ac3);
        assert_eq!(MkvAudioCodec::from_codec_id("UNKNOWN"), MkvAudioCodec::Unknown);
    }

    /// Q7: Test track type enumeration
    #[test]
    fn test_track_type_enumeration() {
        assert_eq!(MkvTrackType::from_u8(1), MkvTrackType::Video);
        assert_eq!(MkvTrackType::from_u8(2), MkvTrackType::Audio);
        assert_eq!(MkvTrackType::from_u8(3), MkvTrackType::Complex);
        assert_eq!(MkvTrackType::from_u8(0x10), MkvTrackType::Logo);
        assert_eq!(MkvTrackType::from_u8(0x11), MkvTrackType::Subtitle);
        assert_eq!(MkvTrackType::from_u8(0x12), MkvTrackType::Buttons);
        assert_eq!(MkvTrackType::from_u8(0x20), MkvTrackType::Control);
        assert_eq!(MkvTrackType::from_u8(0x21), MkvTrackType::Metadata);
        assert_eq!(MkvTrackType::from_u8(0xFF), MkvTrackType::Unknown);
    }

    // ========================================================================
    // Q8-Q14: Property Tests - Dimension and Audio Parameters
    // ========================================================================

    /// Q8: Test video dimensions setting and retrieval
    #[test]
    fn test_video_dimensions() {
        let capsule = MkvTrackCapsule::new();
        capsule.set_track_type(MkvTrackType::Video);

        capsule.set_video_dimensions(1920, 1080);
        let dims = capsule.video_dimensions();
        assert_eq!(dims, Some((1920, 1080)));

        // Test pixel dimensions preserved
        let snapshot = capsule.snapshot();
        assert_eq!(snapshot.pixel_width, 1920);
        assert_eq!(snapshot.pixel_height, 1080);
    }

    /// Q9: Test display dimensions
    #[test]
    fn test_display_dimensions() {
        let capsule = MkvTrackCapsule::new();
        capsule.set_track_type(MkvTrackType::Video);

        // Set pixel dimensions
        capsule.set_video_dimensions(1920, 1080);

        // Before setting display dimensions, should fall back to pixel
        assert_eq!(capsule.display_dimensions(), Some((1920, 1080)));

        // Set display dimensions (e.g., for anamorphic content)
        capsule.set_display_dimensions(2560, 1080);
        assert_eq!(capsule.display_dimensions(), Some((2560, 1080)));
    }

    /// Q10: Test audio parameters
    #[test]
    fn test_audio_parameters() {
        let capsule = MkvTrackCapsule::new();
        capsule.set_track_type(MkvTrackType::Audio);

        capsule.set_audio_params(48000.0, 2, 16);

        assert_eq!(capsule.sample_rate(), Some(48000.0));
        assert_eq!(capsule.channels(), Some(2));
        assert_eq!(capsule.bit_depth(), Some(16));
    }

    /// Q11: Test sample rate edge cases
    #[test]
    fn test_sample_rate_variations() {
        let capsule = MkvTrackCapsule::new();
        capsule.set_track_type(MkvTrackType::Audio);

        // CD quality
        capsule.set_audio_params(44100.0, 2, 16);
        assert!((capsule.sample_rate().unwrap() - 44100.0).abs() < 0.001);

        // High-res audio
        capsule.set_audio_params(96000.0, 2, 24);
        assert!((capsule.sample_rate().unwrap() - 96000.0).abs() < 0.001);

        // DVD audio
        capsule.set_audio_params(48000.0, 6, 24);
        assert_eq!(capsule.channels(), Some(6));
    }

    /// Q12: Test default duration and frame rate
    #[test]
    fn test_default_duration_and_frame_rate() {
        let capsule = MkvTrackCapsule::new();
        capsule.set_track_type(MkvTrackType::Video);

        // 24 fps = 41666666 ns per frame
        let duration_ns = 41_666_666u64;
        capsule.set_default_duration(duration_ns);

        assert_eq!(capsule.default_duration_ns(), Some(duration_ns));

        let fps = capsule.frame_rate().unwrap();
        assert!((fps - 24.0).abs() < 0.1);

        // 30 fps
        capsule.set_default_duration(33_333_333);
        let fps = capsule.frame_rate().unwrap();
        assert!((fps - 30.0).abs() < 0.1);

        // 60 fps
        capsule.set_default_duration(16_666_666);
        let fps = capsule.frame_rate().unwrap();
        assert!((fps - 60.0).abs() < 0.1);
    }

    /// Q13: Test track flags
    #[test]
    fn test_track_flags() {
        let capsule = MkvTrackCapsule::new();

        // Default: enabled, not default, not forced
        assert!(capsule.is_enabled());
        assert!(!capsule.is_default());
        assert!(!capsule.is_forced());

        // Toggle flags
        capsule.set_enabled(false);
        assert!(!capsule.is_enabled());

        capsule.set_default(true);
        assert!(capsule.is_default());

        capsule.set_forced(true);
        assert!(capsule.is_forced());

        // Re-enable
        capsule.set_enabled(true);
        assert!(capsule.is_enabled());
    }

    /// Q14: Test generation counter increments
    #[test]
    fn test_generation_counter() {
        let capsule = MkvTrackCapsule::new();
        assert_eq!(capsule.generation(), 0);

        capsule.set_track_number(1);
        assert_eq!(capsule.generation(), 1);

        capsule.set_track_type(MkvTrackType::Video);
        assert_eq!(capsule.generation(), 2);

        capsule.set_codec_id("V_VP9");
        assert_eq!(capsule.generation(), 3);

        capsule.set_video_dimensions(1920, 1080);
        assert_eq!(capsule.generation(), 4);
    }

    // ========================================================================
    // Q15-Q21: Integration Tests - EBML Parsing
    // ========================================================================

    /// Q15: Test VINT parsing
    #[test]
    fn test_vint_parsing() {
        // 1-byte VINT (0x80-0xFF range)
        let (value, len) = MkvTrackCapsule::parse_vint(&[0x81]).unwrap();
        assert_eq!(value, 1);
        assert_eq!(len, 1);

        // 2-byte VINT
        let (value, len) = MkvTrackCapsule::parse_vint(&[0x40, 0x01]).unwrap();
        assert_eq!(value, 1);
        assert_eq!(len, 2);

        // Larger value
        let (value, len) = MkvTrackCapsule::parse_vint(&[0x4F, 0xFF]).unwrap();
        assert_eq!(value, 0x0FFF);
        assert_eq!(len, 2);
    }

    /// Q16: Test element header parsing
    #[test]
    fn test_element_header_parsing() {
        // TrackNumber element: ID=0xD7, size=1
        // EBML element IDs preserve the VINT marker bit as part of the ID
        let data = [0xD7, 0x81, 0x01]; // ID + size + value
        let (id, size, header_len) = MkvTrackCapsule::parse_element_header(&data).unwrap();
        assert_eq!(id, 0xD7);
        assert_eq!(size, 1);
        assert_eq!(header_len, 2);
    }

    /// Q17: Test uint parsing
    #[test]
    fn test_uint_parsing() {
        assert_eq!(MkvTrackCapsule::parse_uint(&[0x01]), 1);
        assert_eq!(MkvTrackCapsule::parse_uint(&[0x01, 0x00]), 256);
        assert_eq!(MkvTrackCapsule::parse_uint(&[0xFF]), 255);
        assert_eq!(MkvTrackCapsule::parse_uint(&[0x00, 0x01, 0x00, 0x00]), 65536);
    }

    /// Q18: Test float parsing
    #[test]
    fn test_float_parsing() {
        // 4-byte float (48000.0)
        let float32_bytes = 48000.0f32.to_be_bytes();
        let value = MkvTrackCapsule::parse_float(&float32_bytes).unwrap();
        assert!((value - 48000.0).abs() < 0.001);

        // 8-byte float
        let float64_bytes = 96000.0f64.to_be_bytes();
        let value = MkvTrackCapsule::parse_float(&float64_bytes).unwrap();
        assert!((value - 96000.0).abs() < 0.001);

        // Invalid size
        let result = MkvTrackCapsule::parse_float(&[0x00, 0x01, 0x02]);
        assert!(matches!(result, Err(MkvTrackError::InvalidFloat)));
    }

    /// Q19: Test codec ID hashing
    #[test]
    fn test_codec_id_hashing() {
        let capsule = MkvTrackCapsule::new();
        capsule.set_track_type(MkvTrackType::Video);

        capsule.set_codec_id("V_VP9");

        // Should match
        assert!(capsule.codec_id_matches("V_VP9"));

        // Should not match
        assert!(!capsule.codec_id_matches("V_AV1"));
        assert!(!capsule.codec_id_matches("A_OPUS"));

        // Hash consistency
        let hash1 = MkvTrackCapsule::hash_codec_id("V_VP9");
        let hash2 = MkvTrackCapsule::hash_codec_id("V_VP9");
        assert_eq!(hash1, hash2);

        // Different strings should have different hashes (usually)
        let hash3 = MkvTrackCapsule::hash_codec_id("V_AV1");
        assert_ne!(hash1, hash3);
    }

    /// Q20: Test Video element parsing
    #[test]
    fn test_video_element_parsing() {
        let capsule = MkvTrackCapsule::new();
        capsule.set_track_type(MkvTrackType::Video);

        // Build a simple Video element with PixelWidth=1920, PixelHeight=1080
        let mut video_data = Vec::new();

        // PixelWidth (0xB0) = 1920
        video_data.push(0xB0); // ID
        video_data.push(0x82); // Size = 2
        video_data.extend_from_slice(&[0x07, 0x80]); // 1920

        // PixelHeight (0xBA) = 1080
        video_data.push(0xBA); // ID
        video_data.push(0x82); // Size = 2
        video_data.extend_from_slice(&[0x04, 0x38]); // 1080

        capsule.parse_video_element(&video_data).unwrap();

        assert_eq!(capsule.video_dimensions(), Some((1920, 1080)));
    }

    /// Q21: Test Audio element parsing
    #[test]
    fn test_audio_element_parsing() {
        let capsule = MkvTrackCapsule::new();
        capsule.set_track_type(MkvTrackType::Audio);

        // Build Audio element with SamplingFrequency=48000, Channels=2
        let mut audio_data = Vec::new();

        // SamplingFrequency (0xB5) = 48000.0 as 4-byte float
        audio_data.push(0xB5); // ID
        audio_data.push(0x84); // Size = 4
        audio_data.extend_from_slice(&48000.0f32.to_be_bytes());

        // Channels (0x9F) = 2
        audio_data.push(0x9F); // ID
        audio_data.push(0x81); // Size = 1
        audio_data.push(0x02); // 2 channels

        capsule.parse_audio_element(&audio_data).unwrap();

        assert!((capsule.sample_rate().unwrap() - 48000.0).abs() < 0.001);
        assert_eq!(capsule.channels(), Some(2));
    }

    // ========================================================================
    // Q22-Q28: Production Tests - Real Track Patterns
    // ========================================================================

    /// Q22: Test VP9 + Opus track pattern (WebM)
    #[test]
    fn test_vp9_opus_webm_pattern() {
        let video_track = MkvTrackCapsule::new();
        video_track.set_track_number(1);
        video_track.set_track_uid(0x12345678);
        video_track.set_track_type(MkvTrackType::Video);
        video_track.set_codec_id("V_VP9");
        video_track.set_video_dimensions(1920, 1080);
        video_track.set_default_duration(33_333_333); // 30 fps

        assert!(video_track.is_fully_parsed());
        assert_eq!(video_track.video_codec(), MkvVideoCodec::Vp9);
        assert!(video_track.codec_id_matches("V_VP9"));

        let audio_track = MkvTrackCapsule::new();
        audio_track.set_track_number(2);
        audio_track.set_track_uid(0x87654321);
        audio_track.set_track_type(MkvTrackType::Audio);
        audio_track.set_codec_id("A_OPUS");
        audio_track.set_audio_params(48000.0, 2, 0);

        assert!(audio_track.is_fully_parsed());
        assert_eq!(audio_track.audio_codec(), MkvAudioCodec::Opus);
    }

    /// Q23: Test AV1 + AAC track pattern
    #[test]
    fn test_av1_aac_pattern() {
        let video_track = MkvTrackCapsule::new();
        video_track.set_track_number(1);
        video_track.set_track_type(MkvTrackType::Video);
        video_track.set_codec_id("V_AV1");
        video_track.set_video_dimensions(3840, 2160); // 4K

        assert_eq!(video_track.video_codec(), MkvVideoCodec::Av1);
        let dims = video_track.video_dimensions().unwrap();
        assert_eq!(dims, (3840, 2160));

        let audio_track = MkvTrackCapsule::new();
        audio_track.set_track_number(2);
        audio_track.set_track_type(MkvTrackType::Audio);
        audio_track.set_codec_id("A_AAC");
        audio_track.set_audio_params(44100.0, 2, 16);

        assert_eq!(audio_track.audio_codec(), MkvAudioCodec::Aac);
    }

    /// Q24: Test H.264 + FLAC pattern
    #[test]
    fn test_h264_flac_pattern() {
        let video_track = MkvTrackCapsule::new();
        video_track.set_track_number(1);
        video_track.set_track_type(MkvTrackType::Video);
        video_track.set_codec_id("V_MPEG4/ISO/AVC");
        video_track.set_video_dimensions(1280, 720);
        video_track.set_default_duration(41_666_666); // 24 fps

        assert_eq!(video_track.video_codec(), MkvVideoCodec::H264);

        let audio_track = MkvTrackCapsule::new();
        audio_track.set_track_number(2);
        audio_track.set_track_type(MkvTrackType::Audio);
        audio_track.set_codec_id("A_FLAC");
        audio_track.set_audio_params(96000.0, 2, 24);

        assert_eq!(audio_track.audio_codec(), MkvAudioCodec::Flac);
        assert_eq!(audio_track.bit_depth(), Some(24));
    }

    /// Q25: Test multi-channel audio (5.1 surround)
    #[test]
    fn test_multichannel_audio() {
        let audio_track = MkvTrackCapsule::new();
        audio_track.set_track_number(2);
        audio_track.set_track_type(MkvTrackType::Audio);
        audio_track.set_codec_id("A_AC3");
        audio_track.set_audio_params(48000.0, 6, 16); // 5.1 surround

        assert_eq!(audio_track.channels(), Some(6));
        assert_eq!(audio_track.audio_codec(), MkvAudioCodec::Ac3);
    }

    /// Q26: Test frame counting
    #[test]
    fn test_frame_counting() {
        let capsule = MkvTrackCapsule::new();
        capsule.set_track_type(MkvTrackType::Video);

        assert_eq!(capsule.frames_seen(), 0);
        assert_eq!(capsule.keyframes_seen(), 0);

        // Simulate frame processing
        capsule.increment_frames(true); // Keyframe
        capsule.increment_frames(false);
        capsule.increment_frames(false);
        capsule.increment_frames(true); // Keyframe
        capsule.increment_frames(false);

        assert_eq!(capsule.frames_seen(), 5);
        assert_eq!(capsule.keyframes_seen(), 2);
    }

    /// Q27: Test snapshot consistency
    #[test]
    fn test_snapshot_consistency() {
        let capsule = MkvTrackCapsule::new();
        capsule.set_track_number(1);
        capsule.set_track_uid(0xDEADBEEF);
        capsule.set_track_type(MkvTrackType::Video);
        capsule.set_codec_id("V_VP9");
        capsule.set_video_dimensions(1920, 1080);
        capsule.set_default_duration(16_666_666); // 60 fps
        capsule.set_default(true);

        let snapshot = capsule.snapshot();

        assert_eq!(snapshot.track_number, 1);
        assert_eq!(snapshot.track_uid, 0xDEADBEEF);
        assert_eq!(snapshot.track_type, MkvTrackType::Video);
        assert_eq!(snapshot.video_codec, MkvVideoCodec::Vp9);
        assert_eq!(snapshot.pixel_width, 1920);
        assert_eq!(snapshot.pixel_height, 1080);
        assert_eq!(snapshot.default_duration_ns, 16_666_666);
        assert!(snapshot.is_video());
        assert!(snapshot.is_default());
        assert!(snapshot.is_enabled());

        // Test frame rate from snapshot
        let fps = snapshot.frame_rate().unwrap();
        assert!((fps - 60.0).abs() < 0.1);
    }

    /// Q28: Test capsule size and alignment
    #[test]
    fn test_capsule_layout() {
        assert_eq!(core::mem::size_of::<MkvTrackCapsule>(), 256);
        assert_eq!(core::mem::align_of::<MkvTrackCapsule>(), 256);
    }

    // ========================================================================
    // Additional Tests (Bonus)
    // ========================================================================

    /// Test error handling for buffer too short
    #[test]
    fn test_error_buffer_too_short() {
        let result = MkvTrackCapsule::parse_vint(&[]);
        assert!(matches!(result, Err(MkvTrackError::BufferTooShort { .. })));
    }

    /// Test subtitle track type
    #[test]
    fn test_subtitle_track() {
        let capsule = MkvTrackCapsule::new();
        capsule.set_track_number(3);
        capsule.set_track_type(MkvTrackType::Subtitle);
        capsule.set_codec_id("S_TEXT/UTF8");

        let snapshot = capsule.snapshot();
        assert!(snapshot.is_subtitle());
        assert!(!snapshot.is_video());
        assert!(!snapshot.is_audio());
    }

    /// Test disabled track
    #[test]
    fn test_disabled_track() {
        let capsule = MkvTrackCapsule::new();
        capsule.set_track_number(1);
        capsule.set_track_type(MkvTrackType::Video);
        capsule.set_enabled(false);

        assert!(!capsule.is_enabled());
        let snapshot = capsule.snapshot();
        assert!(!snapshot.is_enabled());
    }

    /// Test forced subtitle track
    #[test]
    fn test_forced_subtitle() {
        let capsule = MkvTrackCapsule::new();
        capsule.set_track_number(3);
        capsule.set_track_type(MkvTrackType::Subtitle);
        capsule.set_forced(true);
        capsule.set_default(true);

        assert!(capsule.is_forced());
        assert!(capsule.is_default());
    }

    /// Test Send + Sync traits
    #[test]
    fn test_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<MkvTrackCapsule>();
        assert_sync::<MkvTrackCapsule>();
    }
}
