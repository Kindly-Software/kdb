//! MP4 Track metadata capsule
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Manages track-level metadata from trak/tkhd/mdia/mdhd boxes.
//! Each video/audio track in an MP4 file gets its own capsule instance.
//!
//! ## T1 Atomic Tier (UCE34 Q10)
//!
//! This capsule uses 100% lockfree atomics for thread-safe metadata access.
//! All fields use AtomicU32/AtomicU64 with Acquire/Release ordering.
//!
//! ## Box Parsing
//!
//! Parses the following ISO Base Media File Format boxes:
//! - tkhd (Track Header): track_id, duration, width/height
//! - mdhd (Media Header): timescale, duration
//! - hdlr (Handler Reference): track type (vide, soun, etc.)
//! - stsd (Sample Description): codec info
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 T1 Atomic tier, Q33 lockfree, Q34 generation counter
//! - **Chaos**: 256B cache-aligned, 100% lockfree
//! - **ASSUM**: All bounds checks verified
//! - **T28**: 10 unit tests (Q1-Q10)

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Track type (from hdlr box handler_type)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TrackType {
    /// Unknown track type
    Unknown = 0,
    /// Video track ('vide')
    Video = 1,
    /// Audio track ('soun')
    Audio = 2,
    /// Hint track ('hint')
    Hint = 3,
    /// Metadata track ('meta')
    Meta = 4,
    /// Text/subtitle track ('text' or 'sbtl')
    Text = 5,
}

impl TrackType {
    /// Convert from u32 value
    #[inline]
    pub const fn from_u32(value: u32) -> Self {
        match value {
            1 => TrackType::Video,
            2 => TrackType::Audio,
            3 => TrackType::Hint,
            4 => TrackType::Meta,
            5 => TrackType::Text,
            _ => TrackType::Unknown,
        }
    }

    /// Convert from FourCC bytes
    #[inline]
    pub const fn from_fourcc(fourcc: [u8; 4]) -> Self {
        match &fourcc {
            b"vide" => TrackType::Video,
            b"soun" => TrackType::Audio,
            b"hint" => TrackType::Hint,
            b"meta" => TrackType::Meta,
            b"text" | b"sbtl" => TrackType::Text,
            _ => TrackType::Unknown,
        }
    }
}

/// Video codec identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VideoCodec {
    /// Unknown video codec
    Unknown = 0,
    /// H.264/AVC (avc1/avc3)
    H264 = 1,
    /// H.265/HEVC (hvc1/hev1)
    H265 = 2,
    /// VP9 (vp09)
    Vp9 = 3,
    /// AV1 (av01)
    Av1 = 4,
    /// MPEG-4 Visual (mp4v)
    Mpeg4 = 5,
}

impl VideoCodec {
    /// Convert from u32 value
    #[inline]
    pub const fn from_u32(value: u32) -> Self {
        match value {
            1 => VideoCodec::H264,
            2 => VideoCodec::H265,
            3 => VideoCodec::Vp9,
            4 => VideoCodec::Av1,
            5 => VideoCodec::Mpeg4,
            _ => VideoCodec::Unknown,
        }
    }

    /// Convert from FourCC bytes
    #[inline]
    pub const fn from_fourcc(fourcc: [u8; 4]) -> Self {
        match &fourcc {
            b"avc1" | b"avc3" => VideoCodec::H264,
            b"hvc1" | b"hev1" => VideoCodec::H265,
            b"vp09" => VideoCodec::Vp9,
            b"av01" => VideoCodec::Av1,
            b"mp4v" => VideoCodec::Mpeg4,
            _ => VideoCodec::Unknown,
        }
    }
}

/// Audio codec identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AudioCodec {
    /// Unknown audio codec
    Unknown = 0,
    /// AAC (mp4a)
    Aac = 1,
    /// MP3 (.mp3)
    Mp3 = 2,
    /// Opus
    Opus = 3,
    /// FLAC (fLaC)
    Flac = 4,
    /// AC-3 (ac-3)
    Ac3 = 5,
    /// E-AC-3 (ec-3)
    Eac3 = 6,
}

impl AudioCodec {
    /// Convert from u32 value
    #[inline]
    pub const fn from_u32(value: u32) -> Self {
        match value {
            1 => AudioCodec::Aac,
            2 => AudioCodec::Mp3,
            3 => AudioCodec::Opus,
            4 => AudioCodec::Flac,
            5 => AudioCodec::Ac3,
            6 => AudioCodec::Eac3,
            _ => AudioCodec::Unknown,
        }
    }

    /// Convert from FourCC bytes
    #[inline]
    pub const fn from_fourcc(fourcc: [u8; 4]) -> Self {
        match &fourcc {
            b"mp4a" => AudioCodec::Aac,
            b".mp3" => AudioCodec::Mp3,
            b"Opus" => AudioCodec::Opus,
            b"fLaC" => AudioCodec::Flac,
            b"ac-3" => AudioCodec::Ac3,
            b"ec-3" => AudioCodec::Eac3,
            _ => AudioCodec::Unknown,
        }
    }
}

/// Track flags bitfield
pub mod track_flags {
    /// Track is enabled
    pub const ENABLED: u64 = 1 << 0;
    /// Track is used in the movie
    pub const IN_MOVIE: u64 = 1 << 1;
    /// Track is used in preview
    pub const IN_PREVIEW: u64 = 1 << 2;
    /// Track is the default for its type
    pub const IS_DEFAULT: u64 = 1 << 3;
    /// tkhd box has been parsed
    pub const PARSED_TKHD: u64 = 1 << 8;
    /// mdhd box has been parsed
    pub const PARSED_MDHD: u64 = 1 << 9;
    /// hdlr box has been parsed
    pub const PARSED_HDLR: u64 = 1 << 10;
    /// stsd box has been parsed
    pub const PARSED_STSD: u64 = 1 << 11;
    /// All required boxes have been parsed
    pub const FULLY_PARSED: u64 = PARSED_TKHD | PARSED_MDHD | PARSED_HDLR | PARSED_STSD;
}

/// Error type for track parsing operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackError {
    /// Data buffer too short for operation
    BufferTooShort {
        /// Required size
        required: usize,
        /// Actual size
        actual: usize,
    },
    /// Invalid box version
    InvalidVersion(u8),
    /// Unknown handler type
    UnknownHandler([u8; 4]),
    /// Unknown codec
    UnknownCodec([u8; 4]),
    /// Invalid fixed-point value
    InvalidFixedPoint,
}

impl core::fmt::Display for TrackError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TrackError::BufferTooShort { required, actual } => {
                write!(f, "Buffer too short: need {} bytes, got {}", required, actual)
            }
            TrackError::InvalidVersion(v) => write!(f, "Invalid box version: {}", v),
            TrackError::UnknownHandler(h) => {
                write!(f, "Unknown handler type: {:?}", core::str::from_utf8(h).unwrap_or("????"))
            }
            TrackError::UnknownCodec(c) => {
                write!(f, "Unknown codec: {:?}", core::str::from_utf8(c).unwrap_or("????"))
            }
            TrackError::InvalidFixedPoint => write!(f, "Invalid fixed-point value"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TrackError {}

/// Atomic snapshot of track metadata
#[derive(Debug, Clone)]
pub struct TrackSnapshot {
    /// Track ID
    pub track_id: u32,
    /// Track type
    pub track_type: TrackType,
    /// Timescale (units per second)
    pub timescale: u32,
    /// Duration in timescale units
    pub duration: u64,
    /// Video width (0 for non-video)
    pub width: u32,
    /// Video height (0 for non-video)
    pub height: u32,
    /// Video codec (Unknown for non-video)
    pub video_codec: VideoCodec,
    /// Audio sample rate (0 for non-audio)
    pub sample_rate: u32,
    /// Audio channel count (0 for non-audio)
    pub channel_count: u32,
    /// Bits per sample (0 for non-audio)
    pub bits_per_sample: u32,
    /// Audio codec (Unknown for non-audio)
    pub audio_codec: AudioCodec,
    /// Total sample count
    pub sample_count: u64,
    /// Keyframe count
    pub keyframe_count: u64,
    /// Codec configuration data offset
    pub codec_config_offset: u64,
    /// Codec configuration data size
    pub codec_config_size: u32,
    /// Generation counter at snapshot time
    pub generation: u64,
    /// Track flags
    pub flags: u64,
}

impl TrackSnapshot {
    /// Duration in seconds
    #[inline]
    pub fn duration_seconds(&self) -> f64 {
        if self.timescale == 0 {
            0.0
        } else {
            self.duration as f64 / self.timescale as f64
        }
    }

    /// Check if track is fully parsed
    #[inline]
    pub fn is_fully_parsed(&self) -> bool {
        (self.flags & track_flags::FULLY_PARSED) == track_flags::FULLY_PARSED
    }

    /// Check if track is enabled
    #[inline]
    pub fn is_enabled(&self) -> bool {
        (self.flags & track_flags::ENABLED) != 0
    }
}

/// T1 Atomic capsule for MP4 track metadata
///
/// 256B cache-aligned for optimal memory access patterns.
/// All fields use atomic types for lockfree thread-safe access.
#[repr(C, align(256))]
pub struct Mp4TrackCapsule {
    // Track identification (8 bytes)
    /// Track ID from tkhd box
    pub track_id: AtomicU32,
    /// Track type (TrackType as u32)
    pub track_type: AtomicU32,

    // Timing information from mdhd (12 bytes)
    /// Timescale: units per second
    pub timescale: AtomicU32,
    /// Duration in timescale units
    pub duration: AtomicU64,

    // Video-specific from stsd (12 bytes)
    /// Video width in pixels
    pub width: AtomicU32,
    /// Video height in pixels
    pub height: AtomicU32,
    /// Video codec (VideoCodec as u32)
    pub video_codec: AtomicU32,

    // Audio-specific from stsd (16 bytes)
    /// Audio sample rate in Hz
    pub sample_rate: AtomicU32,
    /// Number of audio channels
    pub channel_count: AtomicU32,
    /// Bits per audio sample
    pub bits_per_sample: AtomicU32,
    /// Audio codec (AudioCodec as u32)
    pub audio_codec: AtomicU32,

    // Sample counts (16 bytes)
    /// Total number of samples
    pub sample_count: AtomicU64,
    /// Number of keyframes (sync samples)
    pub keyframe_count: AtomicU64,

    // Codec configuration (12 bytes)
    /// Offset to codec config data in file
    pub codec_config_offset: AtomicU64,
    /// Size of codec config data
    pub codec_config_size: AtomicU32,

    // State and coordination (20 bytes)
    /// Generation counter for audit trail (Q34)
    pub generation: AtomicU64,
    /// Track flags (enabled, parsed status, etc.)
    pub flags: AtomicU64,
    /// Reserved for future use
    _reserved: AtomicU32,

    // Padding to 256B
    // Field sizes (with alignment):
    // - track_id + track_type: 4 + 4 = 8 bytes
    // - timescale: 4 bytes + 4 padding (duration requires 8-alignment)
    // - duration: 8 bytes
    // - width + height + video_codec: 4 + 4 + 4 = 12 bytes + 4 padding
    // - sample_rate + channel_count + bits_per_sample + audio_codec: 16 bytes
    // - sample_count + keyframe_count: 16 bytes
    // - codec_config_offset: 8 bytes
    // - codec_config_size: 4 bytes + 4 padding (generation requires 8-alignment)
    // - generation + flags: 16 bytes
    // - _reserved: 4 bytes + 4 padding (to 8-byte aligned)
    // Total with internal padding: 8 + 8 + 8 + 16 + 16 + 16 + 8 + 8 + 16 + 8 = 112 bytes
    // Padding needed: 256 - 112 = 144 bytes
    _padding: [u8; 144],
}

// Verify size at compile time
const _: () = assert!(core::mem::size_of::<Mp4TrackCapsule>() == 256);
const _: () = assert!(core::mem::align_of::<Mp4TrackCapsule>() == 256);

impl Mp4TrackCapsule {
    /// Create a new track capsule with default values
    #[inline]
    pub const fn new() -> Self {
        Self {
            track_id: AtomicU32::new(0),
            track_type: AtomicU32::new(TrackType::Unknown as u32),
            timescale: AtomicU32::new(0),
            duration: AtomicU64::new(0),
            width: AtomicU32::new(0),
            height: AtomicU32::new(0),
            video_codec: AtomicU32::new(VideoCodec::Unknown as u32),
            sample_rate: AtomicU32::new(0),
            channel_count: AtomicU32::new(0),
            bits_per_sample: AtomicU32::new(0),
            audio_codec: AtomicU32::new(AudioCodec::Unknown as u32),
            sample_count: AtomicU64::new(0),
            keyframe_count: AtomicU64::new(0),
            codec_config_offset: AtomicU64::new(0),
            codec_config_size: AtomicU32::new(0),
            generation: AtomicU64::new(0),
            flags: AtomicU64::new(0),
            _reserved: AtomicU32::new(0),
            _padding: [0u8; 144],
        }
    }

    /// Create a new track capsule with the given track ID
    #[inline]
    pub const fn with_id(track_id: u32) -> Self {
        Self {
            track_id: AtomicU32::new(track_id),
            track_type: AtomicU32::new(TrackType::Unknown as u32),
            timescale: AtomicU32::new(0),
            duration: AtomicU64::new(0),
            width: AtomicU32::new(0),
            height: AtomicU32::new(0),
            video_codec: AtomicU32::new(VideoCodec::Unknown as u32),
            sample_rate: AtomicU32::new(0),
            channel_count: AtomicU32::new(0),
            bits_per_sample: AtomicU32::new(0),
            audio_codec: AtomicU32::new(AudioCodec::Unknown as u32),
            sample_count: AtomicU64::new(0),
            keyframe_count: AtomicU64::new(0),
            codec_config_offset: AtomicU64::new(0),
            codec_config_size: AtomicU32::new(0),
            generation: AtomicU64::new(0),
            flags: AtomicU64::new(0),
            _reserved: AtomicU32::new(0),
            _padding: [0u8; 144],
        }
    }

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

    /// Parse tkhd (Track Header) box
    ///
    /// # Arguments
    /// * `data` - Box payload (after size and type fields)
    /// * `version` - Box version (0 or 1)
    ///
    /// # Box Layout
    /// Version 0:
    /// - creation_time(4), modification_time(4), track_id(4), reserved(4), duration(4)
    /// - Then: reserved(8), layer(2), alternate_group(2), volume(2), reserved(2), matrix(36), width(4), height(4)
    ///
    /// Version 1:
    /// - creation_time(8), modification_time(8), track_id(4), reserved(4), duration(8)
    /// - Then: same as version 0
    pub fn parse_tkhd(&self, data: &[u8], version: u8) -> Result<(), TrackError> {
        // Calculate required size based on version
        let (header_size, duration_offset, post_header_offset) = match version {
            0 => {
                // 4 + 4 + 4 + 4 + 4 = 20 bytes before post-header
                // post-header: 8 + 2 + 2 + 2 + 2 + 36 + 4 + 4 = 60 bytes
                (20usize, 16usize, 20usize)
            }
            1 => {
                // 8 + 8 + 4 + 4 + 8 = 32 bytes before post-header
                (32usize, 24usize, 32usize)
            }
            _ => return Err(TrackError::InvalidVersion(version)),
        };

        let required_size = header_size + 60; // 60 bytes for post-header fields
        if data.len() < required_size {
            return Err(TrackError::BufferTooShort {
                required: required_size,
                actual: data.len(),
            });
        }

        // Parse track_id (same position in both versions, after creation/modification time)
        let track_id_offset = if version == 0 { 8 } else { 16 };
        let track_id = u32::from_be_bytes([
            data[track_id_offset],
            data[track_id_offset + 1],
            data[track_id_offset + 2],
            data[track_id_offset + 3],
        ]);

        // Parse duration
        let duration = if version == 0 {
            u32::from_be_bytes([
                data[duration_offset],
                data[duration_offset + 1],
                data[duration_offset + 2],
                data[duration_offset + 3],
            ]) as u64
        } else {
            u64::from_be_bytes([
                data[duration_offset],
                data[duration_offset + 1],
                data[duration_offset + 2],
                data[duration_offset + 3],
                data[duration_offset + 4],
                data[duration_offset + 5],
                data[duration_offset + 6],
                data[duration_offset + 7],
            ])
        };

        // Parse width and height from post-header
        // Offset: post_header_offset + 8(reserved) + 2(layer) + 2(alt_group) + 2(volume) + 2(reserved) + 36(matrix) = post_header_offset + 52
        let width_offset = post_header_offset + 52;
        let height_offset = width_offset + 4;

        // Width and height are 16.16 fixed-point
        let width_fixed = u32::from_be_bytes([
            data[width_offset],
            data[width_offset + 1],
            data[width_offset + 2],
            data[width_offset + 3],
        ]);
        let height_fixed = u32::from_be_bytes([
            data[height_offset],
            data[height_offset + 1],
            data[height_offset + 2],
            data[height_offset + 3],
        ]);

        // Convert 16.16 fixed-point to integer (take upper 16 bits)
        let width = width_fixed >> 16;
        let height = height_fixed >> 16;

        // Parse flags from the first byte after version (track_header flags are in byte 1-3)
        // Actually, the flags are part of the fullbox header which should be passed separately
        // For now, we'll extract flags from byte 1 (assuming data starts after version byte)
        // Standard tkhd flags: track_enabled=0x1, track_in_movie=0x2, track_in_preview=0x4

        // Store parsed values atomically
        self.track_id.store(track_id, Ordering::Release);
        self.duration.store(duration, Ordering::Release);
        self.width.store(width, Ordering::Release);
        self.height.store(height, Ordering::Release);

        // Mark tkhd as parsed and bump generation
        self.set_flag(track_flags::PARSED_TKHD);
        self.bump_generation();

        Ok(())
    }

    /// Parse mdhd (Media Header) box
    ///
    /// # Arguments
    /// * `data` - Box payload (after size and type fields)
    /// * `version` - Box version (0 or 1)
    ///
    /// # Box Layout
    /// Version 0:
    /// - creation_time(4), modification_time(4), timescale(4), duration(4)
    /// - language(2), pre_defined(2)
    ///
    /// Version 1:
    /// - creation_time(8), modification_time(8), timescale(4), duration(8)
    /// - language(2), pre_defined(2)
    pub fn parse_mdhd(&self, data: &[u8], version: u8) -> Result<(), TrackError> {
        let (timescale_offset, duration_offset, required_size) = match version {
            0 => (8usize, 12usize, 20usize), // 4+4+4+4 + 2+2 = 20
            1 => (16usize, 20usize, 32usize), // 8+8+4+8 + 2+2 = 32
            _ => return Err(TrackError::InvalidVersion(version)),
        };

        if data.len() < required_size {
            return Err(TrackError::BufferTooShort {
                required: required_size,
                actual: data.len(),
            });
        }

        // Parse timescale (always 4 bytes)
        let timescale = u32::from_be_bytes([
            data[timescale_offset],
            data[timescale_offset + 1],
            data[timescale_offset + 2],
            data[timescale_offset + 3],
        ]);

        // Parse duration
        let duration = if version == 0 {
            u32::from_be_bytes([
                data[duration_offset],
                data[duration_offset + 1],
                data[duration_offset + 2],
                data[duration_offset + 3],
            ]) as u64
        } else {
            u64::from_be_bytes([
                data[duration_offset],
                data[duration_offset + 1],
                data[duration_offset + 2],
                data[duration_offset + 3],
                data[duration_offset + 4],
                data[duration_offset + 5],
                data[duration_offset + 6],
                data[duration_offset + 7],
            ])
        };

        // Store parsed values
        self.timescale.store(timescale, Ordering::Release);
        self.duration.store(duration, Ordering::Release);

        // Mark mdhd as parsed
        self.set_flag(track_flags::PARSED_MDHD);
        self.bump_generation();

        Ok(())
    }

    /// Parse hdlr (Handler Reference) box
    ///
    /// # Arguments
    /// * `data` - Box payload (after size and type fields)
    ///
    /// # Box Layout
    /// - pre_defined(4), handler_type(4), reserved(12), name(variable, null-terminated)
    pub fn parse_hdlr(&self, data: &[u8]) -> Result<(), TrackError> {
        const REQUIRED_SIZE: usize = 20; // 4 + 4 + 12 minimum

        if data.len() < REQUIRED_SIZE {
            return Err(TrackError::BufferTooShort {
                required: REQUIRED_SIZE,
                actual: data.len(),
            });
        }

        // Parse handler_type at offset 4
        let handler_type: [u8; 4] = [data[4], data[5], data[6], data[7]];
        let track_type = TrackType::from_fourcc(handler_type);

        // Store track type
        self.track_type.store(track_type as u32, Ordering::Release);

        // Mark hdlr as parsed
        self.set_flag(track_flags::PARSED_HDLR);
        self.bump_generation();

        Ok(())
    }

    /// Parse stsd (Sample Description) entry
    ///
    /// # Arguments
    /// * `data` - Sample entry data (after entry_count)
    ///
    /// # Box Layout for Visual Sample Entry (78 bytes base):
    /// - size(4), type(4), reserved(6), data_reference_index(2)
    /// - pre_defined(2), reserved(2), pre_defined(12)
    /// - width(2), height(2), horizresolution(4), vertresolution(4)
    /// - reserved(4), frame_count(2), compressor_name(32), depth(2), pre_defined(2)
    /// - Then codec-specific boxes (avcC, hvcC, av1C, etc.)
    ///
    /// # Box Layout for Audio Sample Entry (28 bytes base):
    /// - size(4), type(4), reserved(6), data_reference_index(2)
    /// - version(2), revision_level(2), vendor(4)
    /// - channel_count(2), sample_size(2), compression_id(2), packet_size(2)
    /// - sample_rate(4, 16.16 fixed-point)
    pub fn parse_stsd_entry(&self, data: &[u8]) -> Result<(), TrackError> {
        const MIN_SIZE: usize = 16; // size + type + reserved + data_ref_index

        if data.len() < MIN_SIZE {
            return Err(TrackError::BufferTooShort {
                required: MIN_SIZE,
                actual: data.len(),
            });
        }

        // Parse codec type (FourCC at offset 4)
        let codec_fourcc: [u8; 4] = [data[4], data[5], data[6], data[7]];

        // Determine if this is video or audio based on track type
        let track_type = TrackType::from_u32(self.track_type.load(Ordering::Acquire));

        match track_type {
            TrackType::Video => {
                const VISUAL_MIN_SIZE: usize = 78;
                if data.len() < VISUAL_MIN_SIZE {
                    return Err(TrackError::BufferTooShort {
                        required: VISUAL_MIN_SIZE,
                        actual: data.len(),
                    });
                }

                // Parse video codec
                let video_codec = VideoCodec::from_fourcc(codec_fourcc);
                self.video_codec.store(video_codec as u32, Ordering::Release);

                // Parse width/height at offset 24 and 26 (2 bytes each)
                let width = u16::from_be_bytes([data[24], data[25]]) as u32;
                let height = u16::from_be_bytes([data[26], data[27]]) as u32;

                self.width.store(width, Ordering::Release);
                self.height.store(height, Ordering::Release);
            }
            TrackType::Audio => {
                const AUDIO_MIN_SIZE: usize = 28;
                if data.len() < AUDIO_MIN_SIZE {
                    return Err(TrackError::BufferTooShort {
                        required: AUDIO_MIN_SIZE,
                        actual: data.len(),
                    });
                }

                // Parse audio codec
                let audio_codec = AudioCodec::from_fourcc(codec_fourcc);
                self.audio_codec.store(audio_codec as u32, Ordering::Release);

                // Parse channel_count at offset 16 (2 bytes)
                let channel_count = u16::from_be_bytes([data[16], data[17]]) as u32;
                self.channel_count.store(channel_count, Ordering::Release);

                // Parse sample_size (bits per sample) at offset 18 (2 bytes)
                let bits_per_sample = u16::from_be_bytes([data[18], data[19]]) as u32;
                self.bits_per_sample.store(bits_per_sample, Ordering::Release);

                // Parse sample_rate at offset 24 (4 bytes, 16.16 fixed-point)
                let sample_rate_fixed = u32::from_be_bytes([data[24], data[25], data[26], data[27]]);
                let sample_rate = sample_rate_fixed >> 16;
                self.sample_rate.store(sample_rate, Ordering::Release);
            }
            _ => {
                // For other track types, just note the codec type
                // We still mark it as parsed
            }
        }

        // Mark stsd as parsed
        self.set_flag(track_flags::PARSED_STSD);
        self.bump_generation();

        Ok(())
    }

    /// Get the track type
    #[inline]
    pub fn track_type(&self) -> TrackType {
        TrackType::from_u32(self.track_type.load(Ordering::Acquire))
    }

    /// Get the video codec (returns Unknown for non-video tracks)
    #[inline]
    pub fn video_codec(&self) -> VideoCodec {
        VideoCodec::from_u32(self.video_codec.load(Ordering::Acquire))
    }

    /// Get the audio codec (returns Unknown for non-audio tracks)
    #[inline]
    pub fn audio_codec(&self) -> AudioCodec {
        AudioCodec::from_u32(self.audio_codec.load(Ordering::Acquire))
    }

    /// Get duration in seconds
    #[inline]
    pub fn duration_seconds(&self) -> f64 {
        let timescale = self.timescale.load(Ordering::Acquire);
        if timescale == 0 {
            0.0
        } else {
            let duration = self.duration.load(Ordering::Acquire);
            duration as f64 / timescale as f64
        }
    }

    /// Check if all required boxes have been parsed
    #[inline]
    pub fn is_fully_parsed(&self) -> bool {
        let flags = self.flags.load(Ordering::Acquire);
        (flags & track_flags::FULLY_PARSED) == track_flags::FULLY_PARSED
    }

    /// Get an atomic snapshot of all track metadata
    pub fn snapshot(&self) -> TrackSnapshot {
        // Use Acquire ordering to ensure we see all previous writes
        TrackSnapshot {
            track_id: self.track_id.load(Ordering::Acquire),
            track_type: TrackType::from_u32(self.track_type.load(Ordering::Acquire)),
            timescale: self.timescale.load(Ordering::Acquire),
            duration: self.duration.load(Ordering::Acquire),
            width: self.width.load(Ordering::Acquire),
            height: self.height.load(Ordering::Acquire),
            video_codec: VideoCodec::from_u32(self.video_codec.load(Ordering::Acquire)),
            sample_rate: self.sample_rate.load(Ordering::Acquire),
            channel_count: self.channel_count.load(Ordering::Acquire),
            bits_per_sample: self.bits_per_sample.load(Ordering::Acquire),
            audio_codec: AudioCodec::from_u32(self.audio_codec.load(Ordering::Acquire)),
            sample_count: self.sample_count.load(Ordering::Acquire),
            keyframe_count: self.keyframe_count.load(Ordering::Acquire),
            codec_config_offset: self.codec_config_offset.load(Ordering::Acquire),
            codec_config_size: self.codec_config_size.load(Ordering::Acquire),
            generation: self.generation.load(Ordering::Acquire),
            flags: self.flags.load(Ordering::Acquire),
        }
    }
}

impl Default for Mp4TrackCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Send + Sync are safe because all fields are atomic
// #ASSUME: AtomicU32/AtomicU64 are inherently thread-safe
// #VERIFY: All operations use proper memory ordering (Acquire/Release)
unsafe impl Send for Mp4TrackCapsule {}
unsafe impl Sync for Mp4TrackCapsule {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Q1: Test default track construction
    #[test]
    fn test_new_track_defaults() {
        let track = Mp4TrackCapsule::new();

        assert_eq!(track.track_id.load(Ordering::Relaxed), 0);
        assert_eq!(track.track_type(), TrackType::Unknown);
        assert_eq!(track.timescale.load(Ordering::Relaxed), 0);
        assert_eq!(track.duration.load(Ordering::Relaxed), 0);
        assert_eq!(track.width.load(Ordering::Relaxed), 0);
        assert_eq!(track.height.load(Ordering::Relaxed), 0);
        assert_eq!(track.video_codec(), VideoCodec::Unknown);
        assert_eq!(track.audio_codec(), AudioCodec::Unknown);
        assert_eq!(track.generation.load(Ordering::Relaxed), 0);
        assert!(!track.is_fully_parsed());
    }

    /// Q2: Test track construction with ID
    #[test]
    fn test_with_id() {
        let track = Mp4TrackCapsule::with_id(42);

        assert_eq!(track.track_id.load(Ordering::Relaxed), 42);
        assert_eq!(track.track_type(), TrackType::Unknown);
        assert!(!track.is_fully_parsed());
    }

    /// Q3: Test tkhd version 0 parsing
    #[test]
    fn test_parse_tkhd_v0() {
        let track = Mp4TrackCapsule::new();

        // Build a version 0 tkhd payload (80 bytes total)
        // creation_time(4), modification_time(4), track_id(4), reserved(4), duration(4)
        // + reserved(8), layer(2), alternate_group(2), volume(2), reserved(2), matrix(36), width(4), height(4)
        let mut data = vec![0u8; 80];

        // Track ID at offset 8 (after creation/modification time)
        data[8..12].copy_from_slice(&1u32.to_be_bytes());

        // Duration at offset 16
        data[16..20].copy_from_slice(&30000u32.to_be_bytes());

        // Width at offset 72 (20 + 52), 16.16 fixed-point
        let width_fixed: u32 = 1920 << 16;
        data[72..76].copy_from_slice(&width_fixed.to_be_bytes());

        // Height at offset 76
        let height_fixed: u32 = 1080 << 16;
        data[76..80].copy_from_slice(&height_fixed.to_be_bytes());

        let result = track.parse_tkhd(&data, 0);
        assert!(result.is_ok());

        assert_eq!(track.track_id.load(Ordering::Relaxed), 1);
        assert_eq!(track.duration.load(Ordering::Relaxed), 30000);
        assert_eq!(track.width.load(Ordering::Relaxed), 1920);
        assert_eq!(track.height.load(Ordering::Relaxed), 1080);
        assert!(track.flags.load(Ordering::Relaxed) & track_flags::PARSED_TKHD != 0);
        assert_eq!(track.generation.load(Ordering::Relaxed), 1);
    }

    /// Q4: Test tkhd version 1 parsing
    #[test]
    fn test_parse_tkhd_v1() {
        let track = Mp4TrackCapsule::new();

        // Build a version 1 tkhd payload (92 bytes total)
        // creation_time(8), modification_time(8), track_id(4), reserved(4), duration(8)
        // + reserved(8), layer(2), alternate_group(2), volume(2), reserved(2), matrix(36), width(4), height(4)
        let mut data = vec![0u8; 92];

        // Track ID at offset 16 (after creation/modification time)
        data[16..20].copy_from_slice(&2u32.to_be_bytes());

        // Duration at offset 24 (8 bytes for v1)
        data[24..32].copy_from_slice(&5_000_000_000u64.to_be_bytes());

        // Width at offset 84 (32 + 52)
        let width_fixed: u32 = 3840 << 16;
        data[84..88].copy_from_slice(&width_fixed.to_be_bytes());

        // Height at offset 88
        let height_fixed: u32 = 2160 << 16;
        data[88..92].copy_from_slice(&height_fixed.to_be_bytes());

        let result = track.parse_tkhd(&data, 1);
        assert!(result.is_ok());

        assert_eq!(track.track_id.load(Ordering::Relaxed), 2);
        assert_eq!(track.duration.load(Ordering::Relaxed), 5_000_000_000);
        assert_eq!(track.width.load(Ordering::Relaxed), 3840);
        assert_eq!(track.height.load(Ordering::Relaxed), 2160);
    }

    /// Q5: Test mdhd parsing
    #[test]
    fn test_parse_mdhd() {
        let track = Mp4TrackCapsule::new();

        // Build a version 0 mdhd payload
        // creation_time(4), modification_time(4), timescale(4), duration(4), language(2), pre_defined(2)
        let mut data = vec![0u8; 20];

        // Timescale at offset 8
        data[8..12].copy_from_slice(&48000u32.to_be_bytes());

        // Duration at offset 12
        data[12..16].copy_from_slice(&4800000u32.to_be_bytes()); // 100 seconds at 48kHz

        let result = track.parse_mdhd(&data, 0);
        assert!(result.is_ok());

        assert_eq!(track.timescale.load(Ordering::Relaxed), 48000);
        assert_eq!(track.duration.load(Ordering::Relaxed), 4800000);
        assert!(track.flags.load(Ordering::Relaxed) & track_flags::PARSED_MDHD != 0);

        // Verify duration calculation
        let duration_secs = track.duration_seconds();
        assert!((duration_secs - 100.0).abs() < 0.001);
    }

    /// Q6: Test hdlr parsing for video track
    #[test]
    fn test_parse_hdlr_video() {
        let track = Mp4TrackCapsule::new();

        // Build hdlr payload
        // pre_defined(4), handler_type(4), reserved(12)
        let mut data = vec![0u8; 20];

        // Handler type at offset 4 = "vide"
        data[4..8].copy_from_slice(b"vide");

        let result = track.parse_hdlr(&data);
        assert!(result.is_ok());

        assert_eq!(track.track_type(), TrackType::Video);
        assert!(track.flags.load(Ordering::Relaxed) & track_flags::PARSED_HDLR != 0);
    }

    /// Q7: Test hdlr parsing for audio track
    #[test]
    fn test_parse_hdlr_audio() {
        let track = Mp4TrackCapsule::new();

        let mut data = vec![0u8; 20];
        data[4..8].copy_from_slice(b"soun");

        let result = track.parse_hdlr(&data);
        assert!(result.is_ok());

        assert_eq!(track.track_type(), TrackType::Audio);
    }

    /// Q8: Test track flags
    #[test]
    fn test_track_flags() {
        let track = Mp4TrackCapsule::new();

        // Initially no flags set
        assert_eq!(track.flags.load(Ordering::Relaxed), 0);

        // Set enabled flag
        track.set_flag(track_flags::ENABLED);
        assert!(track.flags.load(Ordering::Relaxed) & track_flags::ENABLED != 0);

        // Set multiple flags
        track.set_flag(track_flags::IN_MOVIE);
        track.set_flag(track_flags::IN_PREVIEW);

        let flags = track.flags.load(Ordering::Relaxed);
        assert!(flags & track_flags::ENABLED != 0);
        assert!(flags & track_flags::IN_MOVIE != 0);
        assert!(flags & track_flags::IN_PREVIEW != 0);
    }

    /// Q9: Test duration calculation
    #[test]
    fn test_duration_calculation() {
        let track = Mp4TrackCapsule::new();

        // Set timescale to 30000 (common for 29.97 fps video)
        track.timescale.store(30000, Ordering::Relaxed);

        // Set duration to 300000 (10 seconds)
        track.duration.store(300000, Ordering::Relaxed);

        let duration = track.duration_seconds();
        assert!((duration - 10.0).abs() < 0.001);

        // Test with zero timescale (should return 0)
        track.timescale.store(0, Ordering::Relaxed);
        assert_eq!(track.duration_seconds(), 0.0);
    }

    /// Q10: Test fully parsed check
    #[test]
    fn test_fully_parsed_check() {
        let track = Mp4TrackCapsule::new();

        // Not fully parsed initially
        assert!(!track.is_fully_parsed());

        // Parse all required boxes
        let tkhd_data = vec![0u8; 80];
        let mdhd_data = vec![0u8; 20];
        let hdlr_data = {
            let mut d = vec![0u8; 20];
            d[4..8].copy_from_slice(b"vide");
            d
        };
        let stsd_data = vec![0u8; 78];

        track.parse_tkhd(&tkhd_data, 0).unwrap();
        track.parse_mdhd(&mdhd_data, 0).unwrap();
        track.parse_hdlr(&hdlr_data).unwrap();
        track.parse_stsd_entry(&stsd_data).unwrap();

        // Now fully parsed
        assert!(track.is_fully_parsed());

        // Verify generation was bumped 4 times
        assert_eq!(track.generation.load(Ordering::Relaxed), 4);
    }

    /// Additional test: Snapshot functionality
    #[test]
    fn test_snapshot() {
        let track = Mp4TrackCapsule::with_id(1);

        // Set some values
        track.track_type.store(TrackType::Video as u32, Ordering::Relaxed);
        track.timescale.store(24000, Ordering::Relaxed);
        track.duration.store(2400000, Ordering::Relaxed);
        track.width.store(1920, Ordering::Relaxed);
        track.height.store(1080, Ordering::Relaxed);
        track.video_codec.store(VideoCodec::Av1 as u32, Ordering::Relaxed);
        track.flags.store(track_flags::ENABLED | track_flags::FULLY_PARSED, Ordering::Relaxed);

        let snapshot = track.snapshot();

        assert_eq!(snapshot.track_id, 1);
        assert_eq!(snapshot.track_type, TrackType::Video);
        assert_eq!(snapshot.timescale, 24000);
        assert_eq!(snapshot.duration, 2400000);
        assert_eq!(snapshot.width, 1920);
        assert_eq!(snapshot.height, 1080);
        assert_eq!(snapshot.video_codec, VideoCodec::Av1);
        assert!(snapshot.is_enabled());
        assert!(snapshot.is_fully_parsed());
        assert!((snapshot.duration_seconds() - 100.0).abs() < 0.001);
    }

    /// Test capsule size and alignment
    #[test]
    fn test_capsule_layout() {
        assert_eq!(core::mem::size_of::<Mp4TrackCapsule>(), 256);
        assert_eq!(core::mem::align_of::<Mp4TrackCapsule>(), 256);
    }

    /// Test error cases
    #[test]
    fn test_parse_errors() {
        let track = Mp4TrackCapsule::new();

        // Buffer too short for tkhd
        let result = track.parse_tkhd(&[0u8; 10], 0);
        assert!(matches!(result, Err(TrackError::BufferTooShort { .. })));

        // Invalid version
        let result = track.parse_tkhd(&[0u8; 80], 2);
        assert!(matches!(result, Err(TrackError::InvalidVersion(2))));

        // Buffer too short for mdhd
        let result = track.parse_mdhd(&[0u8; 10], 0);
        assert!(matches!(result, Err(TrackError::BufferTooShort { .. })));

        // Buffer too short for hdlr
        let result = track.parse_hdlr(&[0u8; 10]);
        assert!(matches!(result, Err(TrackError::BufferTooShort { .. })));
    }

    /// Test codec detection
    #[test]
    fn test_codec_detection() {
        // Video codecs
        assert_eq!(VideoCodec::from_fourcc(*b"avc1"), VideoCodec::H264);
        assert_eq!(VideoCodec::from_fourcc(*b"avc3"), VideoCodec::H264);
        assert_eq!(VideoCodec::from_fourcc(*b"hvc1"), VideoCodec::H265);
        assert_eq!(VideoCodec::from_fourcc(*b"hev1"), VideoCodec::H265);
        assert_eq!(VideoCodec::from_fourcc(*b"vp09"), VideoCodec::Vp9);
        assert_eq!(VideoCodec::from_fourcc(*b"av01"), VideoCodec::Av1);
        assert_eq!(VideoCodec::from_fourcc(*b"mp4v"), VideoCodec::Mpeg4);
        assert_eq!(VideoCodec::from_fourcc(*b"xxxx"), VideoCodec::Unknown);

        // Audio codecs
        assert_eq!(AudioCodec::from_fourcc(*b"mp4a"), AudioCodec::Aac);
        assert_eq!(AudioCodec::from_fourcc(*b".mp3"), AudioCodec::Mp3);
        assert_eq!(AudioCodec::from_fourcc(*b"Opus"), AudioCodec::Opus);
        assert_eq!(AudioCodec::from_fourcc(*b"fLaC"), AudioCodec::Flac);
        assert_eq!(AudioCodec::from_fourcc(*b"ac-3"), AudioCodec::Ac3);
        assert_eq!(AudioCodec::from_fourcc(*b"ec-3"), AudioCodec::Eac3);
        assert_eq!(AudioCodec::from_fourcc(*b"xxxx"), AudioCodec::Unknown);
    }

    /// Test track type detection
    #[test]
    fn test_track_type_detection() {
        assert_eq!(TrackType::from_fourcc(*b"vide"), TrackType::Video);
        assert_eq!(TrackType::from_fourcc(*b"soun"), TrackType::Audio);
        assert_eq!(TrackType::from_fourcc(*b"hint"), TrackType::Hint);
        assert_eq!(TrackType::from_fourcc(*b"meta"), TrackType::Meta);
        assert_eq!(TrackType::from_fourcc(*b"text"), TrackType::Text);
        assert_eq!(TrackType::from_fourcc(*b"sbtl"), TrackType::Text);
        assert_eq!(TrackType::from_fourcc(*b"xxxx"), TrackType::Unknown);
    }
}
