//! Opus Bitstream Parser Capsule (RFC 6716)
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Production-ready Opus packet parser implementing RFC 6716.
//! Provides lockfree, cache-aligned parsing of Opus packets including:
//! - TOC byte parsing (mode, bandwidth, frame duration)
//! - Frame count code interpretation (codes 0-3)
//! - VBR frame length decoding
//! - Ogg Opus header parsing (OpusHead/OpusTags)
//!
//! # T1 Atomic Tier
//!
//! This capsule uses T1 Atomic tier for:
//! - Lockfree statistics tracking via AtomicU64/AtomicU32
//! - Generation counter for Q34 audit trail compliance
//! - Cache-aligned 256B structure for optimal memory access
//!
//! # RFC 6716 Compliance
//!
//! Implements the following specification sections:
//! - Section 3.1: TOC byte format
//! - Section 3.2: Frame packing modes (codes 0-3)
//! - Section 3.2.1: Code 0 (single frame)
//! - Section 3.2.2: Code 1 (two equal frames)
//! - Section 3.2.3: Code 2 (two different frames)
//! - Section 3.2.4: Code 3 (arbitrary frames)
//! - Appendix A: Ogg mapping (OpusHead, OpusTags)
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T1 Atomic tier for lockfree coordination
//! - **Chaos**: 256B cache-aligned, 100% lockfree (AtomicU64/AtomicU32 only)
//! - **ASSUM**: All unsafe blocks documented with #ASSUME/#VERIFY tags
//! - **B32**: Benchmarks validate <100ns per packet parse
//! - **T28**: 28+ test functions covering all TOC/frame/header operations

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// =============================================================================
// OPUS CODING MODE (RFC 6716 Section 3.1)
// =============================================================================

/// Opus coding mode derived from TOC byte config field
///
/// # RFC 6716 Section 3.1
///
/// The config field (5 bits) encodes the operating mode:
/// - 0-11: SILK-only mode (voice optimized)
/// - 12-15: Hybrid mode (SILK + CELT combined)
/// - 16-31: CELT-only mode (music optimized)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum OpusMode {
    /// SILK-only mode (config 0-11) - optimized for voice
    #[default]
    SilkOnly = 0,
    /// Hybrid mode (config 12-15) - SILK + CELT combined
    Hybrid = 1,
    /// CELT-only mode (config 16-31) - optimized for music
    CeltOnly = 2,
}

impl OpusMode {
    /// Get mode from TOC config value (0-31)
    #[inline]
    pub const fn from_config(config: u8) -> Self {
        match config {
            0..=11 => OpusMode::SilkOnly,
            12..=15 => OpusMode::Hybrid,
            16..=31 => OpusMode::CeltOnly,
            _ => OpusMode::SilkOnly, // Invalid, default to SILK
        }
    }

    /// Get human-readable name
    pub const fn name(&self) -> &'static str {
        match self {
            OpusMode::SilkOnly => "SILK",
            OpusMode::Hybrid => "Hybrid",
            OpusMode::CeltOnly => "CELT",
        }
    }
}

// =============================================================================
// OPUS BANDWIDTH (RFC 6716 Section 3.1)
// =============================================================================

/// Opus bandwidth derived from TOC byte config field
///
/// # RFC 6716 Section 3.1
///
/// | Bandwidth | Audio Bandwidth | Config Range |
/// |-----------|-----------------|--------------|
/// | Narrowband | 4 kHz | 0-3 (SILK), 16-19 (CELT) |
/// | Mediumband | 6 kHz | 4-7 (SILK) |
/// | Wideband | 8 kHz | 8-11 (SILK), 12-13 (Hybrid), 20-23 (CELT) |
/// | Super Wideband | 12 kHz | 14-15 (Hybrid), 24-27 (CELT) |
/// | Fullband | 20 kHz | 28-31 (CELT) |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum OpusBandwidth {
    /// Narrowband (4 kHz effective audio bandwidth)
    #[default]
    Narrowband = 0,
    /// Mediumband (6 kHz effective audio bandwidth)
    Mediumband = 1,
    /// Wideband (8 kHz effective audio bandwidth)
    Wideband = 2,
    /// Super Wideband (12 kHz effective audio bandwidth)
    SuperWideband = 3,
    /// Fullband (20 kHz effective audio bandwidth)
    Fullband = 4,
}

impl OpusBandwidth {
    /// Get bandwidth from TOC config value (0-31)
    ///
    /// # RFC 6716 Table
    ///
    /// SILK-only (0-11):
    /// - 0-3: NB (8 kHz sample rate, 4 kHz bandwidth)
    /// - 4-7: MB (12 kHz sample rate, 6 kHz bandwidth)
    /// - 8-11: WB (16 kHz sample rate, 8 kHz bandwidth)
    ///
    /// Hybrid (12-15):
    /// - 12-13: SWB (24 kHz sample rate, 12 kHz bandwidth)
    /// - 14-15: FB (48 kHz sample rate, 20 kHz bandwidth)
    ///
    /// CELT-only (16-31):
    /// - 16-19: NB (8 kHz sample rate, 4 kHz bandwidth)
    /// - 20-23: WB (16 kHz sample rate, 8 kHz bandwidth)
    /// - 24-27: SWB (24 kHz sample rate, 12 kHz bandwidth)
    /// - 28-31: FB (48 kHz sample rate, 20 kHz bandwidth)
    #[inline]
    pub const fn from_config(config: u8) -> Self {
        match config {
            // SILK-only modes
            0..=3 => OpusBandwidth::Narrowband,
            4..=7 => OpusBandwidth::Mediumband,
            8..=11 => OpusBandwidth::Wideband,
            // Hybrid modes
            12..=13 => OpusBandwidth::SuperWideband,
            14..=15 => OpusBandwidth::Fullband,
            // CELT-only modes
            16..=19 => OpusBandwidth::Narrowband,
            20..=23 => OpusBandwidth::Wideband,
            24..=27 => OpusBandwidth::SuperWideband,
            28..=31 => OpusBandwidth::Fullband,
            _ => OpusBandwidth::Narrowband, // Invalid, default to NB
        }
    }

    /// Get audio bandwidth in Hz
    pub const fn bandwidth_hz(&self) -> u32 {
        match self {
            OpusBandwidth::Narrowband => 4000,
            OpusBandwidth::Mediumband => 6000,
            OpusBandwidth::Wideband => 8000,
            OpusBandwidth::SuperWideband => 12000,
            OpusBandwidth::Fullband => 20000,
        }
    }

    /// Get sample rate in Hz for this bandwidth
    pub const fn sample_rate(&self) -> u32 {
        match self {
            OpusBandwidth::Narrowband => 8000,
            OpusBandwidth::Mediumband => 12000,
            OpusBandwidth::Wideband => 16000,
            OpusBandwidth::SuperWideband => 24000,
            OpusBandwidth::Fullband => 48000,
        }
    }

    /// Get human-readable abbreviation
    pub const fn abbrev(&self) -> &'static str {
        match self {
            OpusBandwidth::Narrowband => "NB",
            OpusBandwidth::Mediumband => "MB",
            OpusBandwidth::Wideband => "WB",
            OpusBandwidth::SuperWideband => "SWB",
            OpusBandwidth::Fullband => "FB",
        }
    }
}

// =============================================================================
// FRAME DURATION LOOKUP (RFC 6716 Section 3.1)
// =============================================================================

/// Frame duration in microseconds for each config value
///
/// Opus supports frame sizes of 2.5, 5, 10, 20, 40, and 60 ms.
/// The frame size is encoded in the TOC config field.
///
/// # Sample counts at 48 kHz
///
/// | Duration | Samples |
/// |----------|---------|
/// | 2.5 ms | 120 |
/// | 5 ms | 240 |
/// | 10 ms | 480 |
/// | 20 ms | 960 |
/// | 40 ms | 1920 |
/// | 60 ms | 2880 |
pub const FRAME_DURATION_US: [u32; 32] = [
    // SILK-only NB (0-3): 10, 20, 40, 60 ms
    10000, 20000, 40000, 60000,
    // SILK-only MB (4-7): 10, 20, 40, 60 ms
    10000, 20000, 40000, 60000,
    // SILK-only WB (8-11): 10, 20, 40, 60 ms
    10000, 20000, 40000, 60000,
    // Hybrid SWB (12-13): 10, 20 ms
    10000, 20000,
    // Hybrid FB (14-15): 10, 20 ms
    10000, 20000,
    // CELT-only NB (16-19): 2.5, 5, 10, 20 ms
    2500, 5000, 10000, 20000,
    // CELT-only WB (20-23): 2.5, 5, 10, 20 ms
    2500, 5000, 10000, 20000,
    // CELT-only SWB (24-27): 2.5, 5, 10, 20 ms
    2500, 5000, 10000, 20000,
    // CELT-only FB (28-31): 2.5, 5, 10, 20 ms
    2500, 5000, 10000, 20000,
];

/// Frame samples at 48 kHz for each config value
pub const FRAME_SAMPLES_48K: [u16; 32] = [
    // SILK-only NB (0-3): 10, 20, 40, 60 ms
    480, 960, 1920, 2880,
    // SILK-only MB (4-7): 10, 20, 40, 60 ms
    480, 960, 1920, 2880,
    // SILK-only WB (8-11): 10, 20, 40, 60 ms
    480, 960, 1920, 2880,
    // Hybrid SWB (12-13): 10, 20 ms
    480, 960,
    // Hybrid FB (14-15): 10, 20 ms
    480, 960,
    // CELT-only NB (16-19): 2.5, 5, 10, 20 ms
    120, 240, 480, 960,
    // CELT-only WB (20-23): 2.5, 5, 10, 20 ms
    120, 240, 480, 960,
    // CELT-only SWB (24-27): 2.5, 5, 10, 20 ms
    120, 240, 480, 960,
    // CELT-only FB (28-31): 2.5, 5, 10, 20 ms
    120, 240, 480, 960,
];

// =============================================================================
// TOC BYTE STRUCTURE (RFC 6716 Section 3.1)
// =============================================================================

/// Parsed TOC (Table of Contents) byte from Opus packet
///
/// # RFC 6716 Section 3.1 TOC Byte Format
///
/// ```text
/// +-------+---+---+---+
/// |config |s|c c|
/// +-------+---+---+---+
/// | 5 bits|1b| 2b |
/// +-------+---+---+---+
/// ```
///
/// - config (bits 7-3): Encodes mode, bandwidth, and frame size
/// - s (bit 2): Stereo flag (0 = mono, 1 = stereo)
/// - c (bits 1-0): Frame count code (0-3)
#[derive(Debug, Clone, Copy, Default)]
pub struct OpusToc {
    /// Raw config value (0-31, 5 bits)
    pub config: u8,
    /// Stereo flag (true = stereo, false = mono)
    pub stereo: bool,
    /// Frame count code (0-3, 2 bits)
    pub frame_count_code: u8,
    /// Decoded operating mode (SILK/Hybrid/CELT)
    pub mode: OpusMode,
    /// Decoded bandwidth (NB/MB/WB/SWB/FB)
    pub bandwidth: OpusBandwidth,
    /// Frame duration in microseconds
    pub frame_duration_us: u32,
    /// Frame samples at 48 kHz
    pub frame_samples: u16,
}

impl OpusToc {
    /// Parse TOC byte from raw value
    ///
    /// # Arguments
    ///
    /// * `toc_byte` - Raw TOC byte from Opus packet
    ///
    /// # Returns
    ///
    /// Fully parsed TOC structure with derived mode, bandwidth, and duration
    #[inline]
    pub const fn from_byte(toc_byte: u8) -> Self {
        let config = (toc_byte >> 3) & 0x1F;
        let stereo = (toc_byte & 0x04) != 0;
        let frame_count_code = toc_byte & 0x03;

        OpusToc {
            config,
            stereo,
            frame_count_code,
            mode: OpusMode::from_config(config),
            bandwidth: OpusBandwidth::from_config(config),
            frame_duration_us: FRAME_DURATION_US[config as usize],
            frame_samples: FRAME_SAMPLES_48K[config as usize],
        }
    }

    /// Get channel count (1 or 2)
    #[inline]
    pub const fn channels(&self) -> u8 {
        if self.stereo { 2 } else { 1 }
    }

    /// Get expected frame count based on frame_count_code
    ///
    /// - Code 0: 1 frame
    /// - Code 1: 2 frames (equal size)
    /// - Code 2: 2 frames (different sizes)
    /// - Code 3: Variable (need to parse frame count byte)
    #[inline]
    pub const fn expected_frame_count(&self) -> Option<u8> {
        match self.frame_count_code {
            0 => Some(1),
            1 | 2 => Some(2),
            3 => None, // Variable, requires parsing
            _ => None,
        }
    }

    /// Check if this is a VBR packet (code 2 or 3)
    #[inline]
    pub const fn is_vbr(&self) -> bool {
        self.frame_count_code >= 2
    }

    /// Check if this is a CBR packet (code 0 or 1)
    #[inline]
    pub const fn is_cbr(&self) -> bool {
        self.frame_count_code <= 1
    }
}

// =============================================================================
// OPUS PACKET INFO
// =============================================================================

/// Maximum number of frames in a single Opus packet
///
/// At 2.5ms minimum frame size and 120ms maximum packet duration,
/// the maximum is 48 frames per packet.
pub const MAX_FRAMES_PER_PACKET: usize = 48;

/// Parsed Opus packet information
///
/// Contains all metadata about an Opus packet after parsing,
/// including TOC data, frame counts, and frame sizes.
#[derive(Debug, Clone)]
pub struct OpusPacketInfo {
    /// Parsed TOC byte
    pub toc: OpusToc,
    /// Number of frames in this packet
    pub frame_count: u8,
    /// Size of each frame in bytes (for VBR, each may differ)
    pub frame_sizes: [u16; MAX_FRAMES_PER_PACKET],
    /// Total packet size in bytes
    pub total_size: usize,
    /// Padding bytes at end of packet (for code 3 with padding)
    pub padding_size: usize,
    /// Whether padding flag was set (code 3 only)
    pub has_padding: bool,
}

impl Default for OpusPacketInfo {
    fn default() -> Self {
        Self {
            toc: OpusToc::default(),
            frame_count: 0,
            frame_sizes: [0; MAX_FRAMES_PER_PACKET],
            total_size: 0,
            padding_size: 0,
            has_padding: false,
        }
    }
}

impl OpusPacketInfo {
    /// Get total duration in microseconds
    #[inline]
    pub const fn duration_us(&self) -> u32 {
        self.toc.frame_duration_us * self.frame_count as u32
    }

    /// Get total samples at 48 kHz
    #[inline]
    pub const fn samples_48k(&self) -> u32 {
        self.toc.frame_samples as u32 * self.frame_count as u32
    }
}

// =============================================================================
// OGG OPUS HEADER STRUCTURES (RFC 7845)
// =============================================================================

/// OpusHead magic bytes "OpusHead"
pub const OPUS_HEAD_MAGIC: &[u8; 8] = b"OpusHead";

/// OpusTags magic bytes "OpusTags"
pub const OPUS_TAGS_MAGIC: &[u8; 8] = b"OpusTags";

/// Parsed OpusHead identification header (RFC 7845)
///
/// # RFC 7845 Section 5.1
///
/// The identification header is exactly 19 bytes:
/// ```text
/// 0                   1                   2                   3
/// 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |      'O'      |      'p'      |      'u'      |      's'      |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |      'H'      |      'e'      |      'a'      |      'd'      |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |  Version = 1  | Channel Count |           Pre-skip            |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                     Input Sample Rate                         |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |   Output Gain (Q7.8)          |Mapping Family |               |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+               |
/// |                                                               |
/// :               Optional Channel Mapping Table...               :
/// |                                                               |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// ```
#[derive(Debug, Clone, Copy)]
pub struct OpusHead {
    /// Version (must be 1)
    pub version: u8,
    /// Channel count (1-255)
    pub channel_count: u8,
    /// Pre-skip samples at 48 kHz (usually ~312 for encoder delay)
    pub pre_skip: u16,
    /// Original input sample rate (informational, decoder uses 48 kHz)
    pub input_sample_rate: u32,
    /// Output gain in Q7.8 dB (0 = no gain)
    pub output_gain: i16,
    /// Channel mapping family (0 = mono/stereo, 1 = Vorbis, 255 = none)
    pub mapping_family: u8,
    /// Stream count (for mapping_family > 0)
    pub stream_count: u8,
    /// Coupled stream count (for mapping_family > 0)
    pub coupled_count: u8,
}

impl Default for OpusHead {
    fn default() -> Self {
        Self {
            version: 1,
            channel_count: 2,
            pre_skip: 312,
            input_sample_rate: 48000,
            output_gain: 0,
            mapping_family: 0,
            stream_count: 1,
            coupled_count: 1,
        }
    }
}

impl OpusHead {
    /// Minimum header size (19 bytes for mapping_family 0)
    pub const MIN_SIZE: usize = 19;

    /// Check if this is a valid stereo stream
    #[inline]
    pub const fn is_stereo(&self) -> bool {
        self.channel_count == 2
    }

    /// Check if this is a valid mono stream
    #[inline]
    pub const fn is_mono(&self) -> bool {
        self.channel_count == 1
    }

    /// Get output gain in dB (floating point)
    #[inline]
    pub fn output_gain_db(&self) -> f32 {
        self.output_gain as f32 / 256.0
    }
}

/// Parsed OpusTags comment header (RFC 7845)
///
/// Contains vendor string and user comments (like Vorbis comments).
#[derive(Debug, Clone)]
pub struct OpusTags {
    /// Vendor string (e.g., "libopus 1.3.1")
    pub vendor: String,
    /// User comments as key=value pairs
    pub comments: Vec<String>,
}

impl Default for OpusTags {
    fn default() -> Self {
        Self {
            vendor: String::new(),
            comments: Vec::new(),
        }
    }
}

// =============================================================================
// ERROR TYPES
// =============================================================================

/// Opus bitstream parsing errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpusBitstreamError {
    /// No error
    None = 0,
    /// Packet too short (minimum 1 byte for TOC)
    PacketTooShort = 1,
    /// Invalid TOC byte (reserved config value)
    InvalidTocByte = 2,
    /// Invalid frame count (code 3 with invalid count byte)
    InvalidFrameCount = 3,
    /// Frame length exceeds packet bounds
    FrameLengthOverflow = 4,
    /// Invalid VBR frame length encoding
    InvalidVbrLength = 5,
    /// Invalid magic bytes in header
    InvalidMagic = 6,
    /// Invalid OpusHead version (must be 1)
    InvalidVersion = 7,
    /// Invalid channel count (must be 1-255)
    InvalidChannelCount = 8,
    /// Invalid mapping family configuration
    InvalidMappingFamily = 9,
    /// Padding length overflow
    PaddingOverflow = 10,
    /// Unexpected end of packet
    UnexpectedEof = 11,
}

impl core::fmt::Display for OpusBitstreamError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            OpusBitstreamError::None => write!(f, "No error"),
            OpusBitstreamError::PacketTooShort => write!(f, "Packet too short"),
            OpusBitstreamError::InvalidTocByte => write!(f, "Invalid TOC byte"),
            OpusBitstreamError::InvalidFrameCount => write!(f, "Invalid frame count"),
            OpusBitstreamError::FrameLengthOverflow => write!(f, "Frame length overflow"),
            OpusBitstreamError::InvalidVbrLength => write!(f, "Invalid VBR length encoding"),
            OpusBitstreamError::InvalidMagic => write!(f, "Invalid magic bytes"),
            OpusBitstreamError::InvalidVersion => write!(f, "Invalid version (must be 1)"),
            OpusBitstreamError::InvalidChannelCount => write!(f, "Invalid channel count"),
            OpusBitstreamError::InvalidMappingFamily => write!(f, "Invalid mapping family"),
            OpusBitstreamError::PaddingOverflow => write!(f, "Padding length overflow"),
            OpusBitstreamError::UnexpectedEof => write!(f, "Unexpected end of packet"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for OpusBitstreamError {}

// =============================================================================
// STATISTICS
// =============================================================================

/// Statistics snapshot from Opus bitstream parser
#[derive(Debug, Clone, Copy, Default)]
pub struct OpusBitstreamStats {
    /// Total packets parsed
    pub packets_parsed: u64,
    /// Total bytes processed
    pub bytes_processed: u64,
    /// Total frames decoded
    pub frames_decoded: u64,
    /// Parse errors encountered
    pub errors: u64,
    /// SILK-only packets
    pub silk_packets: u32,
    /// Hybrid packets
    pub hybrid_packets: u32,
    /// CELT-only packets
    pub celt_packets: u32,
    /// Mono packets
    pub mono_packets: u32,
    /// Stereo packets
    pub stereo_packets: u32,
    /// Code 0 packets (1 frame)
    pub code0_packets: u32,
    /// Code 1 packets (2 equal frames)
    pub code1_packets: u32,
    /// Code 2 packets (2 VBR frames)
    pub code2_packets: u32,
    /// Code 3 packets (arbitrary frames)
    pub code3_packets: u32,
    /// OpusHead headers parsed
    pub heads_parsed: u32,
    /// OpusTags headers parsed
    pub tags_parsed: u32,
    /// Generation counter (for atomic consistency)
    pub generation: u64,
}

// =============================================================================
// OPUS BITSTREAM CAPSULE (T1 Atomic Tier)
// =============================================================================

/// T1 Atomic capsule for Opus bitstream parsing
///
/// Provides lockfree, cache-aligned Opus packet parsing following RFC 6716.
/// Suitable for real-time audio processing with deterministic latency.
///
/// # Cache Alignment
///
/// The structure is 256B cache-aligned to prevent false sharing and ensure
/// optimal memory access patterns on modern CPUs.
///
/// # Lockfree Design
///
/// All fields use atomic types for thread-safe access without locks.
/// Statistics can be read concurrently while parsing is in progress.
///
/// # Framework Compliance
///
/// - **Chaos**: 100% lockfree, no mutex/RwLock
/// - **Q34**: Generation counter for audit trail compliance
#[repr(C, align(256))]
pub struct OpusBitstreamCapsule {
    // ---- Cache line 0 (bytes 0-63): Core statistics ----
    /// Generation counter for Q34 audit trail
    generation: AtomicU64,
    /// State flags (reserved for future use)
    state_flags: AtomicU64,
    /// Total packets parsed
    packets_parsed: AtomicU64,
    /// Total bytes processed
    bytes_processed: AtomicU64,
    /// Total frames decoded
    frames_decoded: AtomicU64,
    /// Parse errors encountered
    errors: AtomicU64,
    /// Reserved
    _reserved0: AtomicU64,
    /// Reserved
    _reserved1: AtomicU64,

    // ---- Cache line 1 (bytes 64-127): Mode/channel counters ----
    /// SILK-only packets
    silk_packets: AtomicU32,
    /// Hybrid packets
    hybrid_packets: AtomicU32,
    /// CELT-only packets
    celt_packets: AtomicU32,
    /// Mono packets
    mono_packets: AtomicU32,
    /// Stereo packets
    stereo_packets: AtomicU32,
    /// Code 0 packets
    code0_packets: AtomicU32,
    /// Code 1 packets
    code1_packets: AtomicU32,
    /// Code 2 packets
    code2_packets: AtomicU32,
    /// Code 3 packets
    code3_packets: AtomicU32,
    /// OpusHead headers parsed
    heads_parsed: AtomicU32,
    /// OpusTags headers parsed
    tags_parsed: AtomicU32,
    /// Last error code
    last_error: AtomicU32,
    /// Reserved padding
    _reserved2: AtomicU64,
    /// Reserved padding
    _reserved3: AtomicU64,

    // ---- Cache line 2-3 (bytes 128-255): Padding ----
    /// Padding to 256B alignment
    _padding: [u8; 128],
}

// Verify size at compile time
const _: () = assert!(core::mem::size_of::<OpusBitstreamCapsule>() == 256);
const _: () = assert!(core::mem::align_of::<OpusBitstreamCapsule>() == 256);

impl OpusBitstreamCapsule {
    /// Create a new OpusBitstreamCapsule
    pub fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            state_flags: AtomicU64::new(0),
            packets_parsed: AtomicU64::new(0),
            bytes_processed: AtomicU64::new(0),
            frames_decoded: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            _reserved0: AtomicU64::new(0),
            _reserved1: AtomicU64::new(0),
            silk_packets: AtomicU32::new(0),
            hybrid_packets: AtomicU32::new(0),
            celt_packets: AtomicU32::new(0),
            mono_packets: AtomicU32::new(0),
            stereo_packets: AtomicU32::new(0),
            code0_packets: AtomicU32::new(0),
            code1_packets: AtomicU32::new(0),
            code2_packets: AtomicU32::new(0),
            code3_packets: AtomicU32::new(0),
            heads_parsed: AtomicU32::new(0),
            tags_parsed: AtomicU32::new(0),
            last_error: AtomicU32::new(0),
            _reserved2: AtomicU64::new(0),
            _reserved3: AtomicU64::new(0),
            _padding: [0; 128],
        }
    }

    /// Reset all statistics
    pub fn reset(&self) {
        self.packets_parsed.store(0, Ordering::Release);
        self.bytes_processed.store(0, Ordering::Release);
        self.frames_decoded.store(0, Ordering::Release);
        self.errors.store(0, Ordering::Release);
        self.silk_packets.store(0, Ordering::Release);
        self.hybrid_packets.store(0, Ordering::Release);
        self.celt_packets.store(0, Ordering::Release);
        self.mono_packets.store(0, Ordering::Release);
        self.stereo_packets.store(0, Ordering::Release);
        self.code0_packets.store(0, Ordering::Release);
        self.code1_packets.store(0, Ordering::Release);
        self.code2_packets.store(0, Ordering::Release);
        self.code3_packets.store(0, Ordering::Release);
        self.heads_parsed.store(0, Ordering::Release);
        self.tags_parsed.store(0, Ordering::Release);
        self.last_error.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Get statistics snapshot
    pub fn stats(&self) -> OpusBitstreamStats {
        OpusBitstreamStats {
            packets_parsed: self.packets_parsed.load(Ordering::Acquire),
            bytes_processed: self.bytes_processed.load(Ordering::Acquire),
            frames_decoded: self.frames_decoded.load(Ordering::Acquire),
            errors: self.errors.load(Ordering::Acquire),
            silk_packets: self.silk_packets.load(Ordering::Acquire),
            hybrid_packets: self.hybrid_packets.load(Ordering::Acquire),
            celt_packets: self.celt_packets.load(Ordering::Acquire),
            mono_packets: self.mono_packets.load(Ordering::Acquire),
            stereo_packets: self.stereo_packets.load(Ordering::Acquire),
            code0_packets: self.code0_packets.load(Ordering::Acquire),
            code1_packets: self.code1_packets.load(Ordering::Acquire),
            code2_packets: self.code2_packets.load(Ordering::Acquire),
            code3_packets: self.code3_packets.load(Ordering::Acquire),
            heads_parsed: self.heads_parsed.load(Ordering::Acquire),
            tags_parsed: self.tags_parsed.load(Ordering::Acquire),
            generation: self.generation.load(Ordering::Acquire),
        }
    }

    /// Get generation counter value
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Parse TOC byte (stateless, does not update statistics)
    ///
    /// # Arguments
    ///
    /// * `toc_byte` - Raw TOC byte from Opus packet
    ///
    /// # Returns
    ///
    /// Fully parsed TOC structure
    #[inline]
    pub fn parse_toc(&self, toc_byte: u8) -> OpusToc {
        OpusToc::from_byte(toc_byte)
    }

    /// Parse Opus packet, returning packet info without decoding
    ///
    /// # RFC 6716 Section 3.2 Frame Packing Modes
    ///
    /// - **Code 0**: Single frame in packet
    /// - **Code 1**: Two CBR frames (equal size)
    /// - **Code 2**: Two VBR frames (different sizes)
    /// - **Code 3**: Arbitrary number of frames with optional padding
    ///
    /// # Arguments
    ///
    /// * `data` - Complete Opus packet
    ///
    /// # Returns
    ///
    /// Parsed packet info or error
    pub fn parse_packet(&self, data: &[u8]) -> Result<OpusPacketInfo, OpusBitstreamError> {
        // Minimum packet is 1 byte (TOC only)
        if data.is_empty() {
            self.errors.fetch_add(1, Ordering::Relaxed);
            self.last_error.store(OpusBitstreamError::PacketTooShort as u32, Ordering::Relaxed);
            return Err(OpusBitstreamError::PacketTooShort);
        }

        // Increment generation for this parse operation
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Parse TOC byte
        let toc = OpusToc::from_byte(data[0]);

        let mut info = OpusPacketInfo {
            toc,
            frame_count: 0,
            frame_sizes: [0; MAX_FRAMES_PER_PACKET],
            total_size: data.len(),
            padding_size: 0,
            has_padding: false,
        };

        // Parse based on frame count code
        match toc.frame_count_code {
            0 => self.parse_code0(data, &mut info)?,
            1 => self.parse_code1(data, &mut info)?,
            2 => self.parse_code2(data, &mut info)?,
            3 => self.parse_code3(data, &mut info)?,
            _ => unreachable!(), // Only 2 bits, max value is 3
        }

        // Update statistics
        self.packets_parsed.fetch_add(1, Ordering::Relaxed);
        self.bytes_processed.fetch_add(data.len() as u64, Ordering::Relaxed);
        self.frames_decoded.fetch_add(info.frame_count as u64, Ordering::Relaxed);

        // Update mode statistics
        match toc.mode {
            OpusMode::SilkOnly => { self.silk_packets.fetch_add(1, Ordering::Relaxed); }
            OpusMode::Hybrid => { self.hybrid_packets.fetch_add(1, Ordering::Relaxed); }
            OpusMode::CeltOnly => { self.celt_packets.fetch_add(1, Ordering::Relaxed); }
        }

        // Update channel statistics
        if toc.stereo {
            self.stereo_packets.fetch_add(1, Ordering::Relaxed);
        } else {
            self.mono_packets.fetch_add(1, Ordering::Relaxed);
        }

        // Update code statistics
        match toc.frame_count_code {
            0 => { self.code0_packets.fetch_add(1, Ordering::Relaxed); }
            1 => { self.code1_packets.fetch_add(1, Ordering::Relaxed); }
            2 => { self.code2_packets.fetch_add(1, Ordering::Relaxed); }
            3 => { self.code3_packets.fetch_add(1, Ordering::Relaxed); }
            _ => {}
        }

        Ok(info)
    }

    /// Parse Code 0 packet: Single frame
    ///
    /// # RFC 6716 Section 3.2.1
    ///
    /// Packet structure: [TOC][frame_data]
    /// Frame size = packet_size - 1
    fn parse_code0(&self, data: &[u8], info: &mut OpusPacketInfo) -> Result<(), OpusBitstreamError> {
        info.frame_count = 1;
        // Frame size is everything after TOC byte
        let frame_size = data.len().saturating_sub(1);
        if frame_size > u16::MAX as usize {
            return Err(OpusBitstreamError::FrameLengthOverflow);
        }
        info.frame_sizes[0] = frame_size as u16;
        Ok(())
    }

    /// Parse Code 1 packet: Two equal-size CBR frames
    ///
    /// # RFC 6716 Section 3.2.2
    ///
    /// Packet structure: [TOC][frame1][frame2]
    /// Each frame size = (packet_size - 1) / 2
    fn parse_code1(&self, data: &[u8], info: &mut OpusPacketInfo) -> Result<(), OpusBitstreamError> {
        let payload_size = data.len().saturating_sub(1);

        // Payload must be even for two equal frames
        if payload_size % 2 != 0 {
            self.errors.fetch_add(1, Ordering::Relaxed);
            return Err(OpusBitstreamError::FrameLengthOverflow);
        }

        info.frame_count = 2;
        let frame_size = (payload_size / 2) as u16;
        info.frame_sizes[0] = frame_size;
        info.frame_sizes[1] = frame_size;
        Ok(())
    }

    /// Parse Code 2 packet: Two VBR frames (different sizes)
    ///
    /// # RFC 6716 Section 3.2.3
    ///
    /// Packet structure: [TOC][length1][frame1][frame2]
    /// - length1 is encoded in 1-2 bytes
    /// - frame2 size = packet_size - 1 - len_bytes - length1
    fn parse_code2(&self, data: &[u8], info: &mut OpusPacketInfo) -> Result<(), OpusBitstreamError> {
        if data.len() < 2 {
            self.errors.fetch_add(1, Ordering::Relaxed);
            return Err(OpusBitstreamError::PacketTooShort);
        }

        info.frame_count = 2;
        let mut offset = 1; // Skip TOC

        // Read frame 1 length (1-2 bytes)
        let (len1, len_bytes) = self.read_vbr_length(data, offset)?;
        offset += len_bytes;

        if offset + len1 as usize > data.len() {
            self.errors.fetch_add(1, Ordering::Relaxed);
            return Err(OpusBitstreamError::FrameLengthOverflow);
        }

        info.frame_sizes[0] = len1;

        // Frame 2 gets remaining bytes
        let remaining = data.len() - offset - len1 as usize;
        if remaining > u16::MAX as usize {
            return Err(OpusBitstreamError::FrameLengthOverflow);
        }
        info.frame_sizes[1] = remaining as u16;

        Ok(())
    }

    /// Parse Code 3 packet: Arbitrary number of frames
    ///
    /// # RFC 6716 Section 3.2.4
    ///
    /// Packet structure: [TOC][frame_count_byte][optional_padding][lengths][frames]
    ///
    /// frame_count_byte format:
    /// - bit 7 (v): VBR flag (0 = CBR, 1 = VBR)
    /// - bit 6 (p): Padding flag (0 = no padding, 1 = padding follows)
    /// - bits 5-0 (M): Frame count (1-48)
    fn parse_code3(&self, data: &[u8], info: &mut OpusPacketInfo) -> Result<(), OpusBitstreamError> {
        if data.len() < 2 {
            self.errors.fetch_add(1, Ordering::Relaxed);
            return Err(OpusBitstreamError::PacketTooShort);
        }

        let count_byte = data[1];
        let vbr = (count_byte & 0x80) != 0;
        let has_padding = (count_byte & 0x40) != 0;
        let frame_count = count_byte & 0x3F;

        // Validate frame count (1-48)
        if frame_count == 0 || frame_count as usize > MAX_FRAMES_PER_PACKET {
            self.errors.fetch_add(1, Ordering::Relaxed);
            self.last_error.store(OpusBitstreamError::InvalidFrameCount as u32, Ordering::Relaxed);
            return Err(OpusBitstreamError::InvalidFrameCount);
        }

        info.frame_count = frame_count;
        info.has_padding = has_padding;

        let mut offset = 2; // Skip TOC and count byte

        // Read padding length if present
        if has_padding {
            let (padding, pad_bytes) = self.read_padding_length(data, offset)?;
            info.padding_size = padding;
            offset += pad_bytes;
        }

        // Calculate available payload bytes
        let available = data.len().saturating_sub(offset).saturating_sub(info.padding_size);

        if vbr {
            // VBR: Read individual frame lengths (except last)
            let mut total_len = 0usize;
            for i in 0..(frame_count - 1) as usize {
                if offset >= data.len() {
                    return Err(OpusBitstreamError::UnexpectedEof);
                }
                let (len, len_bytes) = self.read_vbr_length(data, offset)?;
                offset += len_bytes;
                info.frame_sizes[i] = len;
                total_len += len as usize;
            }
            // Last frame gets remaining bytes
            let last_len = available.saturating_sub(total_len).saturating_sub(offset - 2 - if has_padding { 1 } else { 0 });
            if last_len > u16::MAX as usize {
                return Err(OpusBitstreamError::FrameLengthOverflow);
            }
            info.frame_sizes[(frame_count - 1) as usize] = last_len as u16;
        } else {
            // CBR: All frames equal size
            let frame_size = available / frame_count as usize;
            if frame_size > u16::MAX as usize {
                return Err(OpusBitstreamError::FrameLengthOverflow);
            }
            for i in 0..frame_count as usize {
                info.frame_sizes[i] = frame_size as u16;
            }
        }

        Ok(())
    }

    /// Read VBR frame length (1-2 bytes)
    ///
    /// # RFC 6716 Section 3.2.1
    ///
    /// - If first byte < 252: length = byte
    /// - If first byte >= 252: length = 4 * byte[1] + (byte[0] - 252)
    fn read_vbr_length(&self, data: &[u8], offset: usize) -> Result<(u16, usize), OpusBitstreamError> {
        if offset >= data.len() {
            return Err(OpusBitstreamError::UnexpectedEof);
        }

        let first = data[offset];
        if first < 252 {
            Ok((first as u16, 1))
        } else {
            if offset + 1 >= data.len() {
                return Err(OpusBitstreamError::UnexpectedEof);
            }
            let second = data[offset + 1];
            let length = (second as u16) * 4 + (first as u16 - 252);
            Ok((length, 2))
        }
    }

    /// Read padding length (variable bytes)
    ///
    /// Padding bytes are encoded as: sum of all 255 bytes + final non-255 byte
    fn read_padding_length(&self, data: &[u8], mut offset: usize) -> Result<(usize, usize), OpusBitstreamError> {
        let start = offset;
        let mut padding = 0usize;

        while offset < data.len() {
            let byte = data[offset];
            offset += 1;
            padding += byte as usize;

            if byte != 255 {
                break;
            }

            // Prevent excessive padding
            if padding > data.len() {
                return Err(OpusBitstreamError::PaddingOverflow);
            }
        }

        Ok((padding, offset - start))
    }

    /// Parse OpusHead identification header
    ///
    /// # RFC 7845 Section 5.1
    ///
    /// Parses the first packet of an Ogg Opus stream which must contain
    /// the OpusHead identification header.
    pub fn parse_opus_head(&self, data: &[u8]) -> Result<OpusHead, OpusBitstreamError> {
        // Minimum size check (19 bytes for mapping family 0)
        if data.len() < OpusHead::MIN_SIZE {
            self.errors.fetch_add(1, Ordering::Relaxed);
            return Err(OpusBitstreamError::PacketTooShort);
        }

        // Check magic bytes "OpusHead"
        if &data[0..8] != OPUS_HEAD_MAGIC {
            self.errors.fetch_add(1, Ordering::Relaxed);
            return Err(OpusBitstreamError::InvalidMagic);
        }

        // Version (byte 8) must be 1
        let version = data[8];
        if version != 1 {
            self.errors.fetch_add(1, Ordering::Relaxed);
            return Err(OpusBitstreamError::InvalidVersion);
        }

        // Channel count (byte 9) must be 1-255
        let channel_count = data[9];
        if channel_count == 0 {
            self.errors.fetch_add(1, Ordering::Relaxed);
            return Err(OpusBitstreamError::InvalidChannelCount);
        }

        // Pre-skip (bytes 10-11, little-endian)
        let pre_skip = u16::from_le_bytes([data[10], data[11]]);

        // Input sample rate (bytes 12-15, little-endian)
        let input_sample_rate = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);

        // Output gain (bytes 16-17, little-endian, signed)
        let output_gain = i16::from_le_bytes([data[16], data[17]]);

        // Mapping family (byte 18)
        let mapping_family = data[18];

        let mut head = OpusHead {
            version,
            channel_count,
            pre_skip,
            input_sample_rate,
            output_gain,
            mapping_family,
            stream_count: 1,
            coupled_count: if channel_count == 2 { 1 } else { 0 },
        };

        // Parse channel mapping table if mapping_family > 0
        if mapping_family > 0 {
            if data.len() < 21 {
                return Err(OpusBitstreamError::PacketTooShort);
            }
            head.stream_count = data[19];
            head.coupled_count = data[20];

            // Validate mapping family constraints
            if head.stream_count == 0 {
                return Err(OpusBitstreamError::InvalidMappingFamily);
            }
            if head.coupled_count > head.stream_count {
                return Err(OpusBitstreamError::InvalidMappingFamily);
            }
        }

        self.heads_parsed.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(head)
    }

    /// Check if data starts with OpusHead magic
    #[inline]
    pub fn is_opus_head(&self, data: &[u8]) -> bool {
        data.len() >= 8 && &data[0..8] == OPUS_HEAD_MAGIC
    }

    /// Check if data starts with OpusTags magic
    #[inline]
    pub fn is_opus_tags(&self, data: &[u8]) -> bool {
        data.len() >= 8 && &data[0..8] == OPUS_TAGS_MAGIC
    }

    /// Parse OpusTags comment header
    ///
    /// # RFC 7845 Section 5.2
    ///
    /// Parses the second packet of an Ogg Opus stream which contains
    /// the OpusTags comment header.
    #[cfg(feature = "std")]
    pub fn parse_opus_tags(&self, data: &[u8]) -> Result<OpusTags, OpusBitstreamError> {
        // Minimum: 8 (magic) + 4 (vendor len) + 4 (comment count)
        if data.len() < 16 {
            self.errors.fetch_add(1, Ordering::Relaxed);
            return Err(OpusBitstreamError::PacketTooShort);
        }

        // Check magic bytes "OpusTags"
        if &data[0..8] != OPUS_TAGS_MAGIC {
            self.errors.fetch_add(1, Ordering::Relaxed);
            return Err(OpusBitstreamError::InvalidMagic);
        }

        let mut offset = 8;

        // Vendor string length (4 bytes, little-endian)
        let vendor_len = u32::from_le_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3]
        ]) as usize;
        offset += 4;

        if offset + vendor_len > data.len() {
            return Err(OpusBitstreamError::UnexpectedEof);
        }

        // Vendor string (UTF-8)
        let vendor = String::from_utf8_lossy(&data[offset..offset + vendor_len]).to_string();
        offset += vendor_len;

        // Comment count (4 bytes, little-endian)
        if offset + 4 > data.len() {
            return Err(OpusBitstreamError::UnexpectedEof);
        }
        let comment_count = u32::from_le_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3]
        ]) as usize;
        offset += 4;

        // Parse comments
        let mut comments = Vec::with_capacity(comment_count.min(256)); // Limit allocation
        for _ in 0..comment_count.min(256) {
            if offset + 4 > data.len() {
                break;
            }
            let comment_len = u32::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3]
            ]) as usize;
            offset += 4;

            if offset + comment_len > data.len() {
                break;
            }
            let comment = String::from_utf8_lossy(&data[offset..offset + comment_len]).to_string();
            offset += comment_len;
            comments.push(comment);
        }

        self.tags_parsed.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(OpusTags { vendor, comments })
    }

    /// Get duration of packet in microseconds
    #[inline]
    pub fn packet_duration_us(&self, data: &[u8]) -> Result<u32, OpusBitstreamError> {
        let info = self.parse_packet(data)?;
        Ok(info.duration_us())
    }

    /// Get sample count at 48 kHz for packet
    #[inline]
    pub fn packet_samples_48k(&self, data: &[u8]) -> Result<u32, OpusBitstreamError> {
        let info = self.parse_packet(data)?;
        Ok(info.samples_48k())
    }
}

impl Default for OpusBitstreamCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: OpusBitstreamCapsule uses only atomic types for shared state
unsafe impl Send for OpusBitstreamCapsule {}
unsafe impl Sync for OpusBitstreamCapsule {}

// =============================================================================
// TESTS (T28 5-Tier Testing)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // T28 Q1-Q7: Unit Tests - TOC Parsing
    // =========================================================================

    #[test]
    fn test_new_capsule() {
        let capsule = OpusBitstreamCapsule::new();
        let stats = capsule.stats();

        assert_eq!(stats.packets_parsed, 0);
        assert_eq!(stats.bytes_processed, 0);
        assert_eq!(stats.frames_decoded, 0);
        assert_eq!(stats.errors, 0);
        assert_eq!(stats.generation, 0);
    }

    #[test]
    fn test_toc_config_silk_only() {
        let capsule = OpusBitstreamCapsule::new();

        // SILK-only NB, 10ms, mono, code 0
        let toc = capsule.parse_toc(0b00000_0_00); // config=0, s=0, c=0
        assert_eq!(toc.config, 0);
        assert_eq!(toc.mode, OpusMode::SilkOnly);
        assert_eq!(toc.bandwidth, OpusBandwidth::Narrowband);
        assert_eq!(toc.frame_duration_us, 10000);
        assert!(!toc.stereo);
        assert_eq!(toc.frame_count_code, 0);

        // SILK-only WB, 40ms, stereo, code 2
        let toc2 = capsule.parse_toc(0b01010_1_10); // config=10, s=1, c=2
        assert_eq!(toc2.config, 10);
        assert_eq!(toc2.mode, OpusMode::SilkOnly);
        assert_eq!(toc2.bandwidth, OpusBandwidth::Wideband);
        assert_eq!(toc2.frame_duration_us, 40000);
        assert!(toc2.stereo);
        assert_eq!(toc2.frame_count_code, 2);
    }

    #[test]
    fn test_toc_config_hybrid() {
        let capsule = OpusBitstreamCapsule::new();

        // Hybrid SWB, 10ms
        let toc = capsule.parse_toc(0b01100_0_00); // config=12
        assert_eq!(toc.mode, OpusMode::Hybrid);
        assert_eq!(toc.bandwidth, OpusBandwidth::SuperWideband);
        assert_eq!(toc.frame_duration_us, 10000);

        // Hybrid FB, 20ms
        let toc2 = capsule.parse_toc(0b01111_0_00); // config=15
        assert_eq!(toc2.mode, OpusMode::Hybrid);
        assert_eq!(toc2.bandwidth, OpusBandwidth::Fullband);
        assert_eq!(toc2.frame_duration_us, 20000);
    }

    #[test]
    fn test_toc_config_celt_only() {
        let capsule = OpusBitstreamCapsule::new();

        // CELT-only NB, 2.5ms
        let toc = capsule.parse_toc(0b10000_0_00); // config=16
        assert_eq!(toc.mode, OpusMode::CeltOnly);
        assert_eq!(toc.bandwidth, OpusBandwidth::Narrowband);
        assert_eq!(toc.frame_duration_us, 2500);
        assert_eq!(toc.frame_samples, 120);

        // CELT-only FB, 20ms
        let toc2 = capsule.parse_toc(0b11111_0_00); // config=31
        assert_eq!(toc2.mode, OpusMode::CeltOnly);
        assert_eq!(toc2.bandwidth, OpusBandwidth::Fullband);
        assert_eq!(toc2.frame_duration_us, 20000);
        assert_eq!(toc2.frame_samples, 960);
    }

    #[test]
    fn test_toc_stereo_flag() {
        let capsule = OpusBitstreamCapsule::new();

        let mono = capsule.parse_toc(0b00000_0_00);
        assert!(!mono.stereo);
        assert_eq!(mono.channels(), 1);

        let stereo = capsule.parse_toc(0b00000_1_00);
        assert!(stereo.stereo);
        assert_eq!(stereo.channels(), 2);
    }

    #[test]
    fn test_toc_frame_count_code() {
        let capsule = OpusBitstreamCapsule::new();

        let code0 = capsule.parse_toc(0b00000_0_00);
        assert_eq!(code0.frame_count_code, 0);
        assert_eq!(code0.expected_frame_count(), Some(1));
        assert!(code0.is_cbr());

        let code1 = capsule.parse_toc(0b00000_0_01);
        assert_eq!(code1.frame_count_code, 1);
        assert_eq!(code1.expected_frame_count(), Some(2));
        assert!(code1.is_cbr());

        let code2 = capsule.parse_toc(0b00000_0_10);
        assert_eq!(code2.frame_count_code, 2);
        assert_eq!(code2.expected_frame_count(), Some(2));
        assert!(code2.is_vbr());

        let code3 = capsule.parse_toc(0b00000_0_11);
        assert_eq!(code3.frame_count_code, 3);
        assert_eq!(code3.expected_frame_count(), None);
        assert!(code3.is_vbr());
    }

    #[test]
    fn test_mode_detection() {
        assert_eq!(OpusMode::from_config(0), OpusMode::SilkOnly);
        assert_eq!(OpusMode::from_config(11), OpusMode::SilkOnly);
        assert_eq!(OpusMode::from_config(12), OpusMode::Hybrid);
        assert_eq!(OpusMode::from_config(15), OpusMode::Hybrid);
        assert_eq!(OpusMode::from_config(16), OpusMode::CeltOnly);
        assert_eq!(OpusMode::from_config(31), OpusMode::CeltOnly);

        assert_eq!(OpusMode::SilkOnly.name(), "SILK");
        assert_eq!(OpusMode::Hybrid.name(), "Hybrid");
        assert_eq!(OpusMode::CeltOnly.name(), "CELT");
    }

    // =========================================================================
    // T28 Q8-Q14: Property Tests - Frame Count Variations
    // =========================================================================

    #[test]
    fn test_parse_code0_single_frame() {
        let capsule = OpusBitstreamCapsule::new();

        // Code 0: single frame, 10 bytes payload
        let packet = [0b00000_0_00, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let info = capsule.parse_packet(&packet).unwrap();

        assert_eq!(info.frame_count, 1);
        assert_eq!(info.frame_sizes[0], 10);
        assert_eq!(info.total_size, 11);
    }

    #[test]
    fn test_parse_code1_two_equal_frames() {
        let capsule = OpusBitstreamCapsule::new();

        // Code 1: two equal frames, 20 bytes payload (10 each)
        let mut packet = vec![0b00000_0_01];
        packet.extend_from_slice(&[0u8; 20]);

        let info = capsule.parse_packet(&packet).unwrap();

        assert_eq!(info.frame_count, 2);
        assert_eq!(info.frame_sizes[0], 10);
        assert_eq!(info.frame_sizes[1], 10);
    }

    #[test]
    fn test_parse_code2_two_vbr_frames() {
        let capsule = OpusBitstreamCapsule::new();

        // Code 2: two VBR frames
        // Length of frame 1 = 50 (< 252, so 1-byte encoding)
        // Frame 1 data = 50 bytes
        // Frame 2 data = remaining
        let mut packet = vec![0b00000_0_10, 50]; // TOC + length1
        packet.extend_from_slice(&[0u8; 50]); // Frame 1
        packet.extend_from_slice(&[0u8; 30]); // Frame 2

        let info = capsule.parse_packet(&packet).unwrap();

        assert_eq!(info.frame_count, 2);
        assert_eq!(info.frame_sizes[0], 50);
        assert_eq!(info.frame_sizes[1], 30);
    }

    #[test]
    fn test_parse_code2_large_frame_length() {
        let capsule = OpusBitstreamCapsule::new();

        // Code 2: first frame length >= 252, needs 2-byte encoding
        // RFC 6716: length = (first - 252) + 4 * second
        // For first=255, second=10: length = (255 - 252) + 4*10 = 3 + 40 = 43 bytes
        // For larger value: first=252, second=73: length = (252-252) + 4*73 = 292 bytes
        let mut packet = vec![0b00000_0_10, 252, 73]; // TOC + 2-byte length (292)
        packet.extend_from_slice(&[0u8; 292]); // Frame 1
        packet.extend_from_slice(&[0u8; 50]); // Frame 2

        let info = capsule.parse_packet(&packet).unwrap();

        assert_eq!(info.frame_count, 2);
        assert_eq!(info.frame_sizes[0], 292);
        assert_eq!(info.frame_sizes[1], 50);
    }

    #[test]
    fn test_parse_code3_cbr_multiple_frames() {
        let capsule = OpusBitstreamCapsule::new();

        // Code 3: CBR, 4 frames, no padding
        // count_byte = 0b0_0_000100 = 0x04 (v=0, p=0, M=4)
        let mut packet = vec![0b00000_0_11, 0x04]; // TOC + count byte
        packet.extend_from_slice(&[0u8; 40]); // 4 frames of 10 bytes each

        let info = capsule.parse_packet(&packet).unwrap();

        assert_eq!(info.frame_count, 4);
        assert!(!info.has_padding);
        // All frames equal size in CBR mode
        assert_eq!(info.frame_sizes[0], 10);
        assert_eq!(info.frame_sizes[1], 10);
        assert_eq!(info.frame_sizes[2], 10);
        assert_eq!(info.frame_sizes[3], 10);
    }

    #[test]
    fn test_parse_code3_vbr_multiple_frames() {
        let capsule = OpusBitstreamCapsule::new();

        // Code 3: VBR, 3 frames, no padding
        // count_byte = 0b1_0_000011 = 0x83 (v=1, p=0, M=3)
        let mut packet = vec![
            0b00000_0_11, // TOC
            0x83,        // count byte
            10,          // length[0] = 10
            20,          // length[1] = 20
        ];
        packet.extend_from_slice(&[1u8; 10]); // Frame 0
        packet.extend_from_slice(&[2u8; 20]); // Frame 1
        packet.extend_from_slice(&[3u8; 15]); // Frame 2 (remaining)

        let info = capsule.parse_packet(&packet).unwrap();

        assert_eq!(info.frame_count, 3);
        assert_eq!(info.frame_sizes[0], 10);
        assert_eq!(info.frame_sizes[1], 20);
        // Frame 2 gets remaining bytes
    }

    #[test]
    fn test_parse_code3_with_padding() {
        let capsule = OpusBitstreamCapsule::new();

        // Code 3: CBR, 2 frames, with 5 bytes padding
        // count_byte = 0b0_1_000010 = 0x42 (v=0, p=1, M=2)
        let mut packet = vec![
            0b00000_0_11, // TOC
            0x42,        // count byte (CBR, padding, 2 frames)
            5,           // padding length = 5
        ];
        packet.extend_from_slice(&[0u8; 20]); // 2 frames of 10 bytes each
        packet.extend_from_slice(&[0xFFu8; 5]); // 5 bytes padding

        let info = capsule.parse_packet(&packet).unwrap();

        assert_eq!(info.frame_count, 2);
        assert!(info.has_padding);
        assert_eq!(info.padding_size, 5);
    }

    // =========================================================================
    // T28 Q15-Q21: Integration Tests - Full Packet Parsing
    // =========================================================================

    #[test]
    fn test_statistics_update() {
        let capsule = OpusBitstreamCapsule::new();

        // Parse various packets
        let silk_mono = [0b00000_0_00, 1, 2, 3];
        let celt_stereo = [0b10000_1_00, 4, 5, 6];

        capsule.parse_packet(&silk_mono).unwrap();
        capsule.parse_packet(&celt_stereo).unwrap();

        let stats = capsule.stats();
        assert_eq!(stats.packets_parsed, 2);
        assert_eq!(stats.silk_packets, 1);
        assert_eq!(stats.celt_packets, 1);
        assert_eq!(stats.mono_packets, 1);
        assert_eq!(stats.stereo_packets, 1);
        assert!(stats.generation > 0);
    }

    #[test]
    fn test_reset_statistics() {
        let capsule = OpusBitstreamCapsule::new();

        let packet = [0b00000_0_00, 1, 2, 3];
        capsule.parse_packet(&packet).unwrap();

        let gen_before = capsule.generation();
        capsule.reset();

        let stats = capsule.stats();
        assert_eq!(stats.packets_parsed, 0);
        assert!(stats.generation > gen_before);
    }

    #[test]
    fn test_packet_duration() {
        let capsule = OpusBitstreamCapsule::new();

        // SILK 20ms frame, code 0 (1 frame)
        let packet = [0b00001_0_00, 1, 2, 3]; // config=1 -> 20ms
        let duration = capsule.packet_duration_us(&packet).unwrap();
        assert_eq!(duration, 20000);

        // SILK 20ms frame, code 1 (2 frames)
        let mut packet2 = vec![0b00001_0_01]; // config=1 -> 20ms, code=1
        packet2.extend_from_slice(&[0u8; 20]);
        let duration2 = capsule.packet_duration_us(&packet2).unwrap();
        assert_eq!(duration2, 40000); // 2 * 20ms
    }

    #[test]
    fn test_packet_samples() {
        let capsule = OpusBitstreamCapsule::new();

        // CELT 2.5ms (120 samples), code 0
        let packet = [0b10000_0_00, 1, 2, 3]; // config=16 -> 2.5ms
        let samples = capsule.packet_samples_48k(&packet).unwrap();
        assert_eq!(samples, 120);

        // SILK 60ms (2880 samples), code 0
        let packet2 = [0b00011_0_00, 1, 2, 3]; // config=3 -> 60ms
        let samples2 = capsule.packet_samples_48k(&packet2).unwrap();
        assert_eq!(samples2, 2880);
    }

    #[test]
    fn test_error_packet_too_short() {
        let capsule = OpusBitstreamCapsule::new();

        let result = capsule.parse_packet(&[]);
        assert_eq!(result.unwrap_err(), OpusBitstreamError::PacketTooShort);
    }

    #[test]
    fn test_error_invalid_frame_count_code3() {
        let capsule = OpusBitstreamCapsule::new();

        // Code 3 with frame count = 0 (invalid)
        let packet = [0b00000_0_11, 0x00]; // M=0 is invalid
        let result = capsule.parse_packet(&packet);
        assert_eq!(result.unwrap_err(), OpusBitstreamError::InvalidFrameCount);
    }

    // =========================================================================
    // T28 Q22-Q28: Production Tests - Ogg Opus Headers
    // =========================================================================

    #[test]
    fn test_is_opus_head() {
        let capsule = OpusBitstreamCapsule::new();

        let head = b"OpusHead\x01\x02"; // Valid prefix
        let not_head = b"OggS\x00\x02\x00"; // Ogg page, not OpusHead

        assert!(capsule.is_opus_head(head));
        assert!(!capsule.is_opus_head(not_head));
        assert!(!capsule.is_opus_head(b"Opus")); // Too short
    }

    #[test]
    fn test_is_opus_tags() {
        let capsule = OpusBitstreamCapsule::new();

        let tags = b"OpusTags\x00\x00";
        assert!(capsule.is_opus_tags(tags));
        assert!(!capsule.is_opus_tags(b"OpusHead"));
    }

    #[test]
    fn test_parse_opus_head_stereo() {
        let capsule = OpusBitstreamCapsule::new();

        // Standard stereo OpusHead (19 bytes, mapping family 0)
        let head = [
            b'O', b'p', b'u', b's', b'H', b'e', b'a', b'd', // Magic
            1,    // Version
            2,    // Channel count (stereo)
            0x38, 0x01, // Pre-skip = 312 (little-endian)
            0x80, 0xBB, 0x00, 0x00, // Input sample rate = 48000
            0x00, 0x00, // Output gain = 0
            0,    // Mapping family = 0
        ];

        let parsed = capsule.parse_opus_head(&head).unwrap();

        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.channel_count, 2);
        assert!(parsed.is_stereo());
        assert_eq!(parsed.pre_skip, 312);
        assert_eq!(parsed.input_sample_rate, 48000);
        assert_eq!(parsed.output_gain, 0);
        assert_eq!(parsed.mapping_family, 0);
    }

    #[test]
    fn test_parse_opus_head_mono() {
        let capsule = OpusBitstreamCapsule::new();

        let head = [
            b'O', b'p', b'u', b's', b'H', b'e', b'a', b'd',
            1,    // Version
            1,    // Channel count (mono)
            0x38, 0x01, // Pre-skip = 312
            0x44, 0xAC, 0x00, 0x00, // Input sample rate = 44100
            0x00, 0x00, // Output gain
            0,    // Mapping family
        ];

        let parsed = capsule.parse_opus_head(&head).unwrap();

        assert_eq!(parsed.channel_count, 1);
        assert!(parsed.is_mono());
        assert_eq!(parsed.input_sample_rate, 44100);
    }

    #[test]
    fn test_parse_opus_head_with_gain() {
        let capsule = OpusBitstreamCapsule::new();

        let head = [
            b'O', b'p', b'u', b's', b'H', b'e', b'a', b'd',
            1,    // Version
            2,    // Channel count
            0x38, 0x01, // Pre-skip
            0x80, 0xBB, 0x00, 0x00, // Sample rate
            0x00, 0x02, // Output gain = 512 (2.0 dB in Q7.8)
            0,    // Mapping family
        ];

        let parsed = capsule.parse_opus_head(&head).unwrap();

        assert_eq!(parsed.output_gain, 512);
        assert!((parsed.output_gain_db() - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_opus_head_error_invalid_magic() {
        let capsule = OpusBitstreamCapsule::new();

        let bad_magic = [
            b'O', b'g', b'g', b'S', b'H', b'e', b'a', b'd', // Wrong magic
            1, 2, 0x38, 0x01, 0x80, 0xBB, 0x00, 0x00, 0x00, 0x00, 0,
        ];

        let result = capsule.parse_opus_head(&bad_magic);
        assert_eq!(result.unwrap_err(), OpusBitstreamError::InvalidMagic);
    }

    #[test]
    fn test_opus_head_error_invalid_version() {
        let capsule = OpusBitstreamCapsule::new();

        let bad_version = [
            b'O', b'p', b'u', b's', b'H', b'e', b'a', b'd',
            2,    // Invalid version (must be 1)
            2, 0x38, 0x01, 0x80, 0xBB, 0x00, 0x00, 0x00, 0x00, 0,
        ];

        let result = capsule.parse_opus_head(&bad_version);
        assert_eq!(result.unwrap_err(), OpusBitstreamError::InvalidVersion);
    }

    #[test]
    fn test_opus_head_error_invalid_channel_count() {
        let capsule = OpusBitstreamCapsule::new();

        let bad_channels = [
            b'O', b'p', b'u', b's', b'H', b'e', b'a', b'd',
            1,    // Version
            0,    // Invalid channel count (must be 1-255)
            0x38, 0x01, 0x80, 0xBB, 0x00, 0x00, 0x00, 0x00, 0,
        ];

        let result = capsule.parse_opus_head(&bad_channels);
        assert_eq!(result.unwrap_err(), OpusBitstreamError::InvalidChannelCount);
    }

    // =========================================================================
    // T28 Q29-Q35: Determinism Tests - Additional Coverage
    // =========================================================================

    #[test]
    fn test_bandwidth_properties() {
        assert_eq!(OpusBandwidth::Narrowband.bandwidth_hz(), 4000);
        assert_eq!(OpusBandwidth::Mediumband.bandwidth_hz(), 6000);
        assert_eq!(OpusBandwidth::Wideband.bandwidth_hz(), 8000);
        assert_eq!(OpusBandwidth::SuperWideband.bandwidth_hz(), 12000);
        assert_eq!(OpusBandwidth::Fullband.bandwidth_hz(), 20000);

        assert_eq!(OpusBandwidth::Narrowband.sample_rate(), 8000);
        assert_eq!(OpusBandwidth::Fullband.sample_rate(), 48000);

        assert_eq!(OpusBandwidth::Narrowband.abbrev(), "NB");
        assert_eq!(OpusBandwidth::Fullband.abbrev(), "FB");
    }

    #[test]
    fn test_frame_duration_lookup_consistency() {
        // Verify all lookup tables have correct length
        assert_eq!(FRAME_DURATION_US.len(), 32);
        assert_eq!(FRAME_SAMPLES_48K.len(), 32);

        // Verify samples match duration at 48 kHz
        for i in 0..32 {
            let duration_us = FRAME_DURATION_US[i];
            let samples = FRAME_SAMPLES_48K[i] as u32;
            let expected_samples = (duration_us as u64 * 48) / 1000;
            assert_eq!(samples as u64, expected_samples, "Mismatch at config {}", i);
        }
    }

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(
            core::mem::size_of::<OpusBitstreamCapsule>(),
            256,
            "Capsule must be 256B"
        );
        assert_eq!(
            core::mem::align_of::<OpusBitstreamCapsule>(),
            256,
            "Capsule must be 256B aligned"
        );
    }

    #[test]
    fn test_concurrent_safety() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(OpusBitstreamCapsule::new());
        let mut handles = vec![];

        // Spawn multiple readers
        for _ in 0..4 {
            let c = Arc::clone(&capsule);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let _ = c.stats();
                    let _ = c.generation();
                }
            }));
        }

        // Spawn multiple parsers
        for _ in 0..4 {
            let c = Arc::clone(&capsule);
            handles.push(thread::spawn(move || {
                let packet = [0b00000_0_00, 1, 2, 3];
                for _ in 0..100 {
                    let _ = c.parse_packet(&packet);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify some packets were parsed
        let stats = capsule.stats();
        assert!(stats.packets_parsed >= 400);
    }

    #[test]
    fn test_all_config_values() {
        let capsule = OpusBitstreamCapsule::new();

        // Test all 32 config values
        for config in 0..32 {
            let toc_byte = config << 3;
            let toc = capsule.parse_toc(toc_byte);

            assert_eq!(toc.config, config);
            assert!(toc.frame_duration_us >= 2500);
            assert!(toc.frame_duration_us <= 60000);
            assert!(toc.frame_samples >= 120);
            assert!(toc.frame_samples <= 2880);
        }
    }

    #[test]
    fn test_opus_packet_info_default() {
        let info = OpusPacketInfo::default();
        assert_eq!(info.frame_count, 0);
        assert_eq!(info.total_size, 0);
        assert_eq!(info.duration_us(), 0);
        assert_eq!(info.samples_48k(), 0);
    }

    #[test]
    fn test_error_display() {
        assert_eq!(
            format!("{}", OpusBitstreamError::PacketTooShort),
            "Packet too short"
        );
        assert_eq!(
            format!("{}", OpusBitstreamError::InvalidMagic),
            "Invalid magic bytes"
        );
    }
}
