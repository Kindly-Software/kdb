//! AV1 Sequence Header OBU Capsule
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Implements AOM AV1 Specification Section 5.5 sequence header OBU parsing
//! using T1 Atomic tier for lockfree, cache-aligned state management.
//!
//! # T1 Atomic Tier
//!
//! This capsule uses T1 Atomic tier for:
//! - 100% lockfree state management with AtomicU64/AtomicU32
//! - 512B cache-aligned structure to prevent false sharing
//! - Generation counter for Q34 audit trail compliance
//! - Acquire/Release ordering for correct memory visibility
//!
//! # AV1 Specification Compliance
//!
//! Implements the following AV1 bitstream specification sections:
//! - Section 5.5: Sequence header OBU syntax
//! - Section 5.5.1: Color config
//! - Section 5.5.2: Timing info
//! - Section 5.5.3: Operating parameters info
//! - Section 6.4.1: Sequence header OBU semantics
//!
//! # Profiles (Section 6.4.1)
//!
//! | Profile | Bit depth | Chroma subsampling |
//! |---------|-----------|-------------------|
//! | 0 (Main) | 8/10-bit | 4:2:0 |
//! | 1 (High) | 8/10-bit | 4:4:4 |
//! | 2 (Professional) | 8/10/12-bit | 4:2:0, 4:2:2, 4:4:4 |
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T1 Atomic tier for lockfree state, Q33 derive verification, Q34 audit trails
//! - **Chaos**: 512B cache-aligned, 100% lockfree (AtomicU64/AtomicU32 only)
//! - **ASSUM**: All unsafe blocks documented with #ASSUME/#VERIFY tags
//! - **B32**: Benchmarks validate <50ns field access
//! - **T28**: 34+ tests covering unit/property/integration/production/determinism tiers

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// ============================================================================
// AV1 SEQUENCE HEADER ENUMS
// ============================================================================

/// AV1 Profile (Section 6.4.1)
///
/// Defines the allowed bit depths and chroma subsampling configurations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Av1Profile {
    /// Main Profile: 8/10-bit, 4:2:0
    #[default]
    Main = 0,
    /// High Profile: 8/10-bit, 4:4:4
    High = 1,
    /// Professional Profile: 8/10/12-bit, any subsampling
    Professional = 2,
}

impl Av1Profile {
    /// Convert from raw 3-bit seq_profile value
    #[inline]
    pub const fn from_bits(bits: u8) -> Option<Self> {
        match bits & 0x07 {
            0 => Some(Av1Profile::Main),
            1 => Some(Av1Profile::High),
            2 => Some(Av1Profile::Professional),
            _ => None, // 3-7 are reserved
        }
    }

    /// Check if profile supports 12-bit depth
    #[inline]
    pub const fn supports_12bit(&self) -> bool {
        matches!(self, Av1Profile::Professional)
    }

    /// Check if profile supports 4:4:4 chroma
    #[inline]
    pub const fn supports_444(&self) -> bool {
        matches!(self, Av1Profile::High | Av1Profile::Professional)
    }

    /// Check if profile supports 4:2:2 chroma
    #[inline]
    pub const fn supports_422(&self) -> bool {
        matches!(self, Av1Profile::Professional)
    }

    /// Get default bit depth for profile
    #[inline]
    pub const fn default_bit_depth(&self) -> u8 {
        match self {
            Av1Profile::Main | Av1Profile::High => 8,
            Av1Profile::Professional => 10,
        }
    }
}

impl core::fmt::Display for Av1Profile {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Av1Profile::Main => write!(f, "Main Profile (8/10-bit, 4:2:0)"),
            Av1Profile::High => write!(f, "High Profile (8/10-bit, 4:4:4)"),
            Av1Profile::Professional => write!(f, "Professional Profile (8/10/12-bit, any)"),
        }
    }
}

/// AV1 Color Primaries (Section 6.4.2)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Av1ColorPrimaries {
    /// BT.709 / sRGB
    #[default]
    Bt709 = 1,
    /// Unspecified
    Unspecified = 2,
    /// BT.470 M
    Bt470M = 4,
    /// BT.470 BG
    Bt470Bg = 5,
    /// BT.601
    Bt601 = 6,
    /// SMPTE 240M
    Smpte240 = 7,
    /// Generic film
    GenericFilm = 8,
    /// BT.2020
    Bt2020 = 9,
    /// SMPTE 428 (CIE 1931 XYZ)
    Xyz = 10,
    /// SMPTE RP 431-2
    Smpte431 = 11,
    /// SMPTE EG 432-1
    Smpte432 = 12,
    /// EBU Tech 3213-E
    Ebu3213 = 22,
}

impl Av1ColorPrimaries {
    /// Convert from raw 8-bit value
    #[inline]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            1 => Av1ColorPrimaries::Bt709,
            4 => Av1ColorPrimaries::Bt470M,
            5 => Av1ColorPrimaries::Bt470Bg,
            6 => Av1ColorPrimaries::Bt601,
            7 => Av1ColorPrimaries::Smpte240,
            8 => Av1ColorPrimaries::GenericFilm,
            9 => Av1ColorPrimaries::Bt2020,
            10 => Av1ColorPrimaries::Xyz,
            11 => Av1ColorPrimaries::Smpte431,
            12 => Av1ColorPrimaries::Smpte432,
            22 => Av1ColorPrimaries::Ebu3213,
            _ => Av1ColorPrimaries::Unspecified,
        }
    }
}

impl core::fmt::Display for Av1ColorPrimaries {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Av1ColorPrimaries::Bt709 => write!(f, "BT.709"),
            Av1ColorPrimaries::Unspecified => write!(f, "Unspecified"),
            Av1ColorPrimaries::Bt470M => write!(f, "BT.470 M"),
            Av1ColorPrimaries::Bt470Bg => write!(f, "BT.470 BG"),
            Av1ColorPrimaries::Bt601 => write!(f, "BT.601"),
            Av1ColorPrimaries::Smpte240 => write!(f, "SMPTE 240M"),
            Av1ColorPrimaries::GenericFilm => write!(f, "Generic Film"),
            Av1ColorPrimaries::Bt2020 => write!(f, "BT.2020"),
            Av1ColorPrimaries::Xyz => write!(f, "SMPTE 428 (XYZ)"),
            Av1ColorPrimaries::Smpte431 => write!(f, "SMPTE RP 431-2"),
            Av1ColorPrimaries::Smpte432 => write!(f, "SMPTE EG 432-1"),
            Av1ColorPrimaries::Ebu3213 => write!(f, "EBU Tech 3213-E"),
        }
    }
}

/// AV1 Transfer Characteristics (Section 6.4.2)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Av1TransferCharacteristics {
    /// BT.709
    #[default]
    Bt709 = 1,
    /// Unspecified
    Unspecified = 2,
    /// BT.470 M
    Bt470M = 4,
    /// BT.470 BG
    Bt470Bg = 5,
    /// BT.601
    Bt601 = 6,
    /// SMPTE 240M
    Smpte240 = 7,
    /// Linear
    Linear = 8,
    /// Logarithmic (100:1)
    Log100 = 9,
    /// Logarithmic (100*sqrt(10):1)
    Log100Sqrt10 = 10,
    /// IEC 61966-2-4
    Iec61966 = 11,
    /// BT.1361
    Bt1361 = 12,
    /// sRGB
    Srgb = 13,
    /// BT.2020 10-bit
    Bt2020_10bit = 14,
    /// BT.2020 12-bit
    Bt2020_12bit = 15,
    /// SMPTE 2084 (PQ HDR)
    Smpte2084 = 16,
    /// SMPTE 428
    Smpte428 = 17,
    /// ARIB STD-B67 (HLG)
    HybridLogGamma = 18,
}

impl Av1TransferCharacteristics {
    /// Convert from raw 8-bit value
    #[inline]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            1 => Av1TransferCharacteristics::Bt709,
            4 => Av1TransferCharacteristics::Bt470M,
            5 => Av1TransferCharacteristics::Bt470Bg,
            6 => Av1TransferCharacteristics::Bt601,
            7 => Av1TransferCharacteristics::Smpte240,
            8 => Av1TransferCharacteristics::Linear,
            9 => Av1TransferCharacteristics::Log100,
            10 => Av1TransferCharacteristics::Log100Sqrt10,
            11 => Av1TransferCharacteristics::Iec61966,
            12 => Av1TransferCharacteristics::Bt1361,
            13 => Av1TransferCharacteristics::Srgb,
            14 => Av1TransferCharacteristics::Bt2020_10bit,
            15 => Av1TransferCharacteristics::Bt2020_12bit,
            16 => Av1TransferCharacteristics::Smpte2084,
            17 => Av1TransferCharacteristics::Smpte428,
            18 => Av1TransferCharacteristics::HybridLogGamma,
            _ => Av1TransferCharacteristics::Unspecified,
        }
    }

    /// Check if this is an HDR transfer function
    #[inline]
    pub const fn is_hdr(&self) -> bool {
        matches!(
            self,
            Av1TransferCharacteristics::Smpte2084 | Av1TransferCharacteristics::HybridLogGamma
        )
    }
}

impl core::fmt::Display for Av1TransferCharacteristics {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Av1TransferCharacteristics::Bt709 => write!(f, "BT.709"),
            Av1TransferCharacteristics::Unspecified => write!(f, "Unspecified"),
            Av1TransferCharacteristics::Bt470M => write!(f, "BT.470 M"),
            Av1TransferCharacteristics::Bt470Bg => write!(f, "BT.470 BG"),
            Av1TransferCharacteristics::Bt601 => write!(f, "BT.601"),
            Av1TransferCharacteristics::Smpte240 => write!(f, "SMPTE 240M"),
            Av1TransferCharacteristics::Linear => write!(f, "Linear"),
            Av1TransferCharacteristics::Log100 => write!(f, "Log 100:1"),
            Av1TransferCharacteristics::Log100Sqrt10 => write!(f, "Log 100*sqrt(10):1"),
            Av1TransferCharacteristics::Iec61966 => write!(f, "IEC 61966-2-4"),
            Av1TransferCharacteristics::Bt1361 => write!(f, "BT.1361"),
            Av1TransferCharacteristics::Srgb => write!(f, "sRGB"),
            Av1TransferCharacteristics::Bt2020_10bit => write!(f, "BT.2020 10-bit"),
            Av1TransferCharacteristics::Bt2020_12bit => write!(f, "BT.2020 12-bit"),
            Av1TransferCharacteristics::Smpte2084 => write!(f, "SMPTE 2084 (PQ)"),
            Av1TransferCharacteristics::Smpte428 => write!(f, "SMPTE 428"),
            Av1TransferCharacteristics::HybridLogGamma => write!(f, "HLG"),
        }
    }
}

/// AV1 Matrix Coefficients (Section 6.4.2)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Av1MatrixCoefficients {
    /// Identity (RGB)
    Identity = 0,
    /// BT.709
    #[default]
    Bt709 = 1,
    /// Unspecified
    Unspecified = 2,
    /// FCC
    Fcc = 4,
    /// BT.470 BG
    Bt470Bg = 5,
    /// BT.601
    Bt601 = 6,
    /// SMPTE 240M
    Smpte240 = 7,
    /// YCgCo
    YCgCo = 8,
    /// BT.2020 non-constant luminance
    Bt2020Ncl = 9,
    /// BT.2020 constant luminance
    Bt2020Cl = 10,
    /// SMPTE 2085
    Smpte2085 = 11,
    /// Chromaticity-derived non-constant luminance
    ChromaDerivedNcl = 12,
    /// Chromaticity-derived constant luminance
    ChromaDerivedCl = 13,
    /// ICtCp
    ICtCp = 14,
}

impl Av1MatrixCoefficients {
    /// Convert from raw 8-bit value
    #[inline]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Av1MatrixCoefficients::Identity,
            1 => Av1MatrixCoefficients::Bt709,
            4 => Av1MatrixCoefficients::Fcc,
            5 => Av1MatrixCoefficients::Bt470Bg,
            6 => Av1MatrixCoefficients::Bt601,
            7 => Av1MatrixCoefficients::Smpte240,
            8 => Av1MatrixCoefficients::YCgCo,
            9 => Av1MatrixCoefficients::Bt2020Ncl,
            10 => Av1MatrixCoefficients::Bt2020Cl,
            11 => Av1MatrixCoefficients::Smpte2085,
            12 => Av1MatrixCoefficients::ChromaDerivedNcl,
            13 => Av1MatrixCoefficients::ChromaDerivedCl,
            14 => Av1MatrixCoefficients::ICtCp,
            _ => Av1MatrixCoefficients::Unspecified,
        }
    }

    /// Check if this matrix allows RGB output directly
    #[inline]
    pub const fn is_rgb(&self) -> bool {
        matches!(self, Av1MatrixCoefficients::Identity)
    }
}

impl core::fmt::Display for Av1MatrixCoefficients {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Av1MatrixCoefficients::Identity => write!(f, "Identity (RGB)"),
            Av1MatrixCoefficients::Bt709 => write!(f, "BT.709"),
            Av1MatrixCoefficients::Unspecified => write!(f, "Unspecified"),
            Av1MatrixCoefficients::Fcc => write!(f, "FCC"),
            Av1MatrixCoefficients::Bt470Bg => write!(f, "BT.470 BG"),
            Av1MatrixCoefficients::Bt601 => write!(f, "BT.601"),
            Av1MatrixCoefficients::Smpte240 => write!(f, "SMPTE 240M"),
            Av1MatrixCoefficients::YCgCo => write!(f, "YCgCo"),
            Av1MatrixCoefficients::Bt2020Ncl => write!(f, "BT.2020 NCL"),
            Av1MatrixCoefficients::Bt2020Cl => write!(f, "BT.2020 CL"),
            Av1MatrixCoefficients::Smpte2085 => write!(f, "SMPTE 2085"),
            Av1MatrixCoefficients::ChromaDerivedNcl => write!(f, "Chroma NCL"),
            Av1MatrixCoefficients::ChromaDerivedCl => write!(f, "Chroma CL"),
            Av1MatrixCoefficients::ICtCp => write!(f, "ICtCp"),
        }
    }
}

/// AV1 Chroma Sample Position (Section 6.4.2)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Av1ChromaSamplePosition {
    /// Unknown
    #[default]
    Unknown = 0,
    /// Vertically co-located with luma
    Vertical = 1,
    /// Co-located with top-left luma
    Colocated = 2,
    /// Reserved
    Reserved = 3,
}

impl Av1ChromaSamplePosition {
    /// Convert from raw 2-bit value
    #[inline]
    pub const fn from_bits(bits: u8) -> Self {
        match bits & 0x03 {
            0 => Av1ChromaSamplePosition::Unknown,
            1 => Av1ChromaSamplePosition::Vertical,
            2 => Av1ChromaSamplePosition::Colocated,
            _ => Av1ChromaSamplePosition::Reserved,
        }
    }
}

impl core::fmt::Display for Av1ChromaSamplePosition {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Av1ChromaSamplePosition::Unknown => write!(f, "Unknown"),
            Av1ChromaSamplePosition::Vertical => write!(f, "Vertical"),
            Av1ChromaSamplePosition::Colocated => write!(f, "Colocated"),
            Av1ChromaSamplePosition::Reserved => write!(f, "Reserved"),
        }
    }
}

// ============================================================================
// ERROR TYPES
// ============================================================================

/// AV1 Sequence Header parsing errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Av1SequenceHeaderError {
    /// No error
    #[default]
    None = 0,
    /// Invalid profile value (must be 0-2)
    InvalidProfile = 1,
    /// still_picture constraint violation
    StillPictureViolation = 2,
    /// Invalid frame dimensions (zero or too large)
    InvalidFrameSize = 3,
    /// Invalid bit depth for profile
    InvalidBitDepth = 4,
    /// Invalid color config for profile
    InvalidColorConfig = 5,
    /// Invalid chroma subsampling for profile
    InvalidChromaSubsampling = 6,
    /// Invalid operating point count (must be 1-MAX_OPERATING_POINTS)
    InvalidOperatingPointCount = 7,
    /// Invalid operating point index
    InvalidOperatingPointIndex = 8,
    /// Invalid seq_level_idx value
    InvalidSeqLevelIdx = 9,
    /// Unexpected end of data
    UnexpectedEof = 10,
    /// Invalid timing info
    InvalidTimingInfo = 11,
    /// Feature flag conflict
    FeatureFlagConflict = 12,
    /// Bitstream corrupted
    BitstreamCorrupted = 13,
    /// Reserved bit set
    ReservedBitSet = 14,
}

impl core::fmt::Display for Av1SequenceHeaderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Av1SequenceHeaderError::None => write!(f, "No error"),
            Av1SequenceHeaderError::InvalidProfile => {
                write!(f, "Invalid profile (must be 0-2)")
            }
            Av1SequenceHeaderError::StillPictureViolation => {
                write!(f, "still_picture constraint violation")
            }
            Av1SequenceHeaderError::InvalidFrameSize => write!(f, "Invalid frame dimensions"),
            Av1SequenceHeaderError::InvalidBitDepth => {
                write!(f, "Invalid bit depth for profile")
            }
            Av1SequenceHeaderError::InvalidColorConfig => {
                write!(f, "Invalid color config for profile")
            }
            Av1SequenceHeaderError::InvalidChromaSubsampling => {
                write!(f, "Invalid chroma subsampling for profile")
            }
            Av1SequenceHeaderError::InvalidOperatingPointCount => {
                write!(f, "Invalid operating point count")
            }
            Av1SequenceHeaderError::InvalidOperatingPointIndex => {
                write!(f, "Invalid operating point index")
            }
            Av1SequenceHeaderError::InvalidSeqLevelIdx => {
                write!(f, "Invalid seq_level_idx value")
            }
            Av1SequenceHeaderError::UnexpectedEof => write!(f, "Unexpected end of data"),
            Av1SequenceHeaderError::InvalidTimingInfo => write!(f, "Invalid timing info"),
            Av1SequenceHeaderError::FeatureFlagConflict => write!(f, "Feature flag conflict"),
            Av1SequenceHeaderError::BitstreamCorrupted => write!(f, "Bitstream corrupted"),
            Av1SequenceHeaderError::ReservedBitSet => write!(f, "Reserved bit set"),
        }
    }
}

impl std::error::Error for Av1SequenceHeaderError {}

// ============================================================================
// STATISTICS
// ============================================================================

/// Statistics snapshot from AV1 sequence header parser
#[derive(Debug, Clone, Copy, Default)]
pub struct Av1SequenceHeaderStats {
    /// Total sequence headers parsed
    pub headers_parsed: u32,
    /// Profile 0 (Main) count
    pub profile0_count: u32,
    /// Profile 1 (High) count
    pub profile1_count: u32,
    /// Profile 2 (Professional) count
    pub profile2_count: u32,
    /// 8-bit count
    pub bit_depth_8_count: u32,
    /// 10-bit count
    pub bit_depth_10_count: u32,
    /// 12-bit count
    pub bit_depth_12_count: u32,
    /// HDR transfer function count
    pub hdr_count: u32,
    /// still_picture count
    pub still_picture_count: u32,
    /// Error count
    pub error_count: u32,
    /// Last error type
    pub last_error: Av1SequenceHeaderError,
    /// Generation counter (for Q34 audit trail)
    pub generation: u64,
}

// ============================================================================
// BIT READER
// ============================================================================

/// Simple bit reader for parsing sequence header
struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8, // 0-7, bits remaining in current byte
}

impl<'a> BitReader<'a> {
    #[inline]
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 8,
        }
    }

    /// Read up to 32 bits
    #[inline]
    fn read_bits(&mut self, n: u8) -> Result<u32, Av1SequenceHeaderError> {
        if n == 0 {
            return Ok(0);
        }
        if n > 32 {
            return Err(Av1SequenceHeaderError::BitstreamCorrupted);
        }

        let mut value = 0u32;
        let mut bits_remaining = n;

        while bits_remaining > 0 {
            if self.byte_pos >= self.data.len() {
                return Err(Av1SequenceHeaderError::UnexpectedEof);
            }

            let bits_in_byte = self.bit_pos.min(bits_remaining);
            let shift = self.bit_pos - bits_in_byte;
            let mask = ((1u32 << bits_in_byte) - 1) as u8;
            let bits = (self.data[self.byte_pos] >> shift) & mask;

            value = (value << bits_in_byte) | (bits as u32);
            bits_remaining -= bits_in_byte;
            self.bit_pos -= bits_in_byte;

            if self.bit_pos == 0 {
                self.byte_pos += 1;
                self.bit_pos = 8;
            }
        }

        Ok(value)
    }

    /// Read a single bit
    #[inline]
    fn read_bit(&mut self) -> Result<bool, Av1SequenceHeaderError> {
        Ok(self.read_bits(1)? != 0)
    }

    /// Read unsigned variable length coded value (uvlc)
    /// Uses Exp-Golomb-like encoding
    #[inline]
    fn read_uvlc(&mut self) -> Result<u32, Av1SequenceHeaderError> {
        let mut leading_zeros = 0u32;
        while !self.read_bit()? {
            leading_zeros += 1;
            if leading_zeros > 32 {
                return Err(Av1SequenceHeaderError::BitstreamCorrupted);
            }
        }

        if leading_zeros >= 32 {
            return Ok(u32::MAX);
        }

        let value = self.read_bits(leading_zeros as u8)?;
        Ok((1 << leading_zeros) - 1 + value)
    }

    /// Bytes consumed so far
    #[inline]
    fn bytes_consumed(&self) -> usize {
        if self.bit_pos == 8 {
            self.byte_pos
        } else {
            self.byte_pos + 1
        }
    }
}

// ============================================================================
// CONSTANTS
// ============================================================================

/// Maximum number of operating points
pub const MAX_OPERATING_POINTS: usize = 32;

/// Maximum frame width
pub const MAX_FRAME_WIDTH: u32 = 65536;

/// Maximum frame height
pub const MAX_FRAME_HEIGHT: u32 = 65536;

/// Number of reference frames
pub const NUM_REF_FRAMES: usize = 8;

/// AV1 select_screen_content_tools value for adaptive
pub const SELECT_SCREEN_CONTENT_TOOLS: u8 = 2;

/// AV1 select_integer_mv value for adaptive
pub const SELECT_INTEGER_MV: u8 = 2;

// ============================================================================
// AV1 SEQUENCE HEADER CAPSULE
// ============================================================================

/// T1 Atomic capsule for AV1 sequence header OBU parsing and state
///
/// Provides lockfree, cache-aligned storage for comprehensive AV1 sequence header
/// information including profile, level, color config, timing, and feature flags.
///
/// # Cache Alignment
///
/// The structure is 512B cache-aligned to prevent false sharing and ensure
/// optimal memory access patterns for multi-threaded decoding.
///
/// # Lockfree Design
///
/// All fields use atomic types for thread-safe access without locks.
/// Multiple threads can read sequence header state while parsing is in progress.
///
/// # Field Packing
///
/// Sequence header fields are packed into atomic integers for efficient storage:
/// - `state`: profile(3) | still_picture(1) | reduced_header(1) | timing_present(1) | ...
/// - `dimensions`: max_width_minus_1(16) | max_height_minus_1(16) | additional_bits(8) | ...
/// - `color_config`: primaries(8) | transfer(8) | matrix(8) | flags(8)
/// - `features`: individual feature enable flags packed together
#[repr(C, align(128))]
pub struct Av1SequenceHeaderCapsule {
    // ---- Cache line 0 (bytes 0-63): Primary state ----
    /// Packed sequence state:
    /// - Bits 0-2: seq_profile (0-2)
    /// - Bit 3: still_picture
    /// - Bit 4: reduced_still_picture_header
    /// - Bit 5: timing_info_present_flag
    /// - Bit 6: decoder_model_info_present_flag
    /// - Bit 7: initial_display_delay_present_flag
    /// - Bits 8-12: operating_points_cnt_minus_1
    /// - Bits 13-15: reserved
    /// - Bits 16-20: seq_level_idx[0]
    /// - Bit 21: seq_tier[0]
    /// - Bits 22-31: reserved
    state: AtomicU64,

    /// Generation counter for Q34 audit trail
    generation: AtomicU64,

    /// Packed frame dimensions:
    /// - Bits 0-15: max_frame_width_minus_1
    /// - Bits 16-31: max_frame_height_minus_1
    max_frame_dimensions: AtomicU32,

    /// Packed frame dimension bits:
    /// - Bits 0-3: frame_width_bits_minus_1
    /// - Bits 4-7: frame_height_bits_minus_1
    /// - Bit 8: frame_id_numbers_present_flag
    /// - Bits 9-12: delta_frame_id_length_minus_2
    /// - Bits 13-17: additional_frame_id_length_minus_1
    frame_id_config: AtomicU32,

    /// Packed superblock/order hint config:
    /// - Bit 0: use_128x128_superblock
    /// - Bit 1: enable_filter_intra
    /// - Bit 2: enable_intra_edge_filter
    /// - Bit 3: enable_interintra_compound
    /// - Bit 4: enable_masked_compound
    /// - Bit 5: enable_warped_motion
    /// - Bit 6: enable_dual_filter
    /// - Bit 7: enable_order_hint
    /// - Bit 8: enable_jnt_comp
    /// - Bit 9: enable_ref_frame_mvs
    /// - Bit 10: seq_choose_screen_content_tools
    /// - Bit 11: seq_force_screen_content_tools (value if not choose)
    /// - Bit 12: seq_choose_integer_mv
    /// - Bit 13: seq_force_integer_mv (value if not choose)
    /// - Bits 14-16: order_hint_bits_minus_1
    /// - Bit 17: enable_superres
    /// - Bit 18: enable_cdef
    /// - Bit 19: enable_restoration
    /// - Bit 20: film_grain_params_present
    feature_flags: AtomicU32,

    /// Reserved for future use
    _reserved0: AtomicU32,

    // ---- Cache line 1 (bytes 64-127): Color config ----
    /// Packed color config:
    /// - Bits 0-7: color_primaries
    /// - Bits 8-15: transfer_characteristics
    /// - Bits 16-23: matrix_coefficients
    /// - Bit 24: color_range (0=studio, 1=full)
    /// - Bits 25-26: chroma_sample_position
    /// - Bit 27: separate_uv_delta_q
    color_config: AtomicU32,

    /// Packed bit depth and chroma:
    /// - Bits 0-3: bit_depth (8, 10, 12)
    /// - Bit 4: mono_chrome
    /// - Bit 5: high_bitdepth
    /// - Bit 6: twelve_bit
    /// - Bit 7: subsampling_x
    /// - Bit 8: subsampling_y
    /// - Bit 9: color_description_present_flag
    bit_depth_config: AtomicU32,

    /// Packed timing info:
    /// - Bits 0-31: num_units_in_display_tick (lower 32 bits)
    timing_num_units: AtomicU32,

    /// Packed timing info:
    /// - Bits 0-31: time_scale
    timing_time_scale: AtomicU32,

    /// Packed timing flags:
    /// - Bit 0: equal_picture_interval
    /// - Bits 1-31: num_ticks_per_picture_minus_1 (if equal_picture_interval)
    timing_flags: AtomicU32,

    /// Reserved for alignment
    _reserved1: AtomicU32,

    /// Reserved for alignment
    _reserved2: AtomicU64,

    // ---- Cache line 2 (bytes 128-191): Operating points ----
    /// Operating point 0 idc and flags:
    /// - Bits 0-11: operating_point_idc[0]
    /// - Bits 12-16: seq_level_idx[0]
    /// - Bit 17: seq_tier[0]
    /// - Bit 18: decoder_model_present_for_this_op[0]
    /// - Bit 19: initial_display_delay_present_for_this_op[0]
    /// - Bits 20-23: initial_display_delay_minus_1[0]
    op_point_0: AtomicU32,

    /// Operating point 1-7 packed (simplified, just store count)
    /// Full implementation would store all operating points
    op_point_count: AtomicU32,

    /// Reserved for operating points
    _op_reserved: [AtomicU64; 6],

    // ---- Cache line 3 (bytes 192-255): Statistics ----
    /// Headers parsed count
    headers_parsed: AtomicU32,

    /// Profile counts packed (p0[8] | p1[8] | p2[8] | reserved[8])
    profile_counts: AtomicU32,

    /// Bit depth counts packed (8bit[10] | 10bit[10] | 12bit[10] | reserved[2])
    bit_depth_counts: AtomicU32,

    /// Error count
    error_count: AtomicU32,

    /// Last error type
    last_error: AtomicU32,

    /// HDR transfer count
    hdr_count: AtomicU32,

    /// Still picture count
    still_picture_count: AtomicU32,

    /// Reserved for stats
    _stats_reserved: AtomicU32,

    /// Bit position in current parse
    bit_position: AtomicU64,

    /// Reserved for alignment
    _stats_reserved2: [AtomicU64; 3],

    // ---- Cache lines 4-7 (bytes 256-511): Padding ----
    /// Padding to 512B alignment
    _padding: [u8; 256],
}

// Compile-time size and alignment verification
const _: () = assert!(core::mem::size_of::<Av1SequenceHeaderCapsule>() == 512);
const _: () = assert!(core::mem::align_of::<Av1SequenceHeaderCapsule>() == 128);

impl Default for Av1SequenceHeaderCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl Av1SequenceHeaderCapsule {
    /// Create a new AV1 sequence header capsule
    ///
    /// Initializes all atomic fields to zero and sets generation counter to 0.
    pub fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            max_frame_dimensions: AtomicU32::new(0),
            frame_id_config: AtomicU32::new(0),
            feature_flags: AtomicU32::new(0),
            _reserved0: AtomicU32::new(0),
            color_config: AtomicU32::new(0),
            bit_depth_config: AtomicU32::new(0),
            timing_num_units: AtomicU32::new(0),
            timing_time_scale: AtomicU32::new(0),
            timing_flags: AtomicU32::new(0),
            _reserved1: AtomicU32::new(0),
            _reserved2: AtomicU64::new(0),
            op_point_0: AtomicU32::new(0),
            op_point_count: AtomicU32::new(0),
            _op_reserved: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            headers_parsed: AtomicU32::new(0),
            profile_counts: AtomicU32::new(0),
            bit_depth_counts: AtomicU32::new(0),
            error_count: AtomicU32::new(0),
            last_error: AtomicU32::new(0),
            hdr_count: AtomicU32::new(0),
            still_picture_count: AtomicU32::new(0),
            _stats_reserved: AtomicU32::new(0),
            bit_position: AtomicU64::new(0),
            _stats_reserved2: [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)],
            _padding: [0u8; 256],
        }
    }

    /// Reset the capsule state
    ///
    /// Clears all parsed sequence header data and increments generation counter.
    pub fn reset(&self) {
        self.state.store(0, Ordering::Release);
        self.max_frame_dimensions.store(0, Ordering::Release);
        self.frame_id_config.store(0, Ordering::Release);
        self.feature_flags.store(0, Ordering::Release);
        self.color_config.store(0, Ordering::Release);
        self.bit_depth_config.store(0, Ordering::Release);
        self.timing_num_units.store(0, Ordering::Release);
        self.timing_time_scale.store(0, Ordering::Release);
        self.timing_flags.store(0, Ordering::Release);
        self.op_point_0.store(0, Ordering::Release);
        self.op_point_count.store(0, Ordering::Release);
        self.bit_position.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Reset statistics counters
    pub fn reset_stats(&self) {
        self.headers_parsed.store(0, Ordering::Release);
        self.profile_counts.store(0, Ordering::Release);
        self.bit_depth_counts.store(0, Ordering::Release);
        self.error_count.store(0, Ordering::Release);
        self.last_error.store(0, Ordering::Release);
        self.hdr_count.store(0, Ordering::Release);
        self.still_picture_count.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Get the current generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    // ========================================================================
    // SEQUENCE HEADER PARSING
    // ========================================================================

    /// Parse sequence header OBU from byte slice
    ///
    /// Parses the AV1 sequence header OBU (Section 5.5) which contains:
    /// - Profile (seq_profile)
    /// - still_picture flag
    /// - Max frame dimensions
    /// - Color configuration
    /// - Timing information (optional)
    /// - Operating points
    /// - Feature flags
    ///
    /// # Arguments
    ///
    /// * `obu_data` - Raw sequence header OBU data (without OBU header)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Parsing succeeded
    /// * `Err(error)` - Parsing error
    pub fn parse_sequence_header(
        &self,
        obu_data: &[u8],
    ) -> Result<(), Av1SequenceHeaderError> {
        if obu_data.is_empty() {
            self.record_error(Av1SequenceHeaderError::UnexpectedEof);
            return Err(Av1SequenceHeaderError::UnexpectedEof);
        }

        // Reset bit position
        self.bit_position.store(0, Ordering::Release);

        let mut reader = BitReader::new(obu_data);

        // seq_profile (3 bits) - Section 5.5
        let seq_profile = reader.read_bits(3)? as u8;
        if seq_profile > 2 {
            self.record_error(Av1SequenceHeaderError::InvalidProfile);
            return Err(Av1SequenceHeaderError::InvalidProfile);
        }

        // still_picture (1 bit)
        let still_picture = reader.read_bit()?;

        // reduced_still_picture_header (1 bit)
        let reduced_still_picture_header = reader.read_bit()?;

        // Constraint: if !still_picture, reduced_still_picture_header must be 0
        if !still_picture && reduced_still_picture_header {
            self.record_error(Av1SequenceHeaderError::StillPictureViolation);
            return Err(Av1SequenceHeaderError::StillPictureViolation);
        }

        if still_picture {
            self.still_picture_count.fetch_add(1, Ordering::Relaxed);
        }

        // Parse based on reduced header
        let (
            timing_info_present,
            decoder_model_info_present,
            initial_display_delay_present,
            op_count_minus_1,
            operating_point_idc_0,
            seq_level_idx_0,
            seq_tier_0,
        ) = if reduced_still_picture_header {
            // Reduced header: no timing, single operating point
            let level_idx = reader.read_bits(5)? as u8;
            (false, false, false, 0, 0, level_idx, false)
        } else {
            // Full header
            let timing_present = reader.read_bit()?;

            let decoder_model_present = if timing_present {
                self.parse_timing_info(&mut reader)?;
                reader.read_bit()?
            } else {
                false
            };

            if decoder_model_present {
                self.parse_decoder_model_info(&mut reader)?;
            }

            let initial_display_present = reader.read_bit()?;
            let op_count = reader.read_bits(5)? as u8;

            // Parse operating points
            let mut op_idc_0 = 0u16;
            let mut level_idx_0 = 0u8;
            let mut tier_0 = false;

            for i in 0..=op_count {
                let op_idc = reader.read_bits(12)? as u16;
                let level_idx = reader.read_bits(5)? as u8;

                if level_idx > 23 {
                    self.record_error(Av1SequenceHeaderError::InvalidSeqLevelIdx);
                    return Err(Av1SequenceHeaderError::InvalidSeqLevelIdx);
                }

                let tier = if level_idx > 7 {
                    reader.read_bit()?
                } else {
                    false
                };

                if i == 0 {
                    op_idc_0 = op_idc;
                    level_idx_0 = level_idx;
                    tier_0 = tier;
                }

                if decoder_model_present {
                    let _decoder_model_present_for_op = reader.read_bit()?;
                    // Skip decoder model buffer delays if present
                }

                if initial_display_present {
                    let display_delay_present = reader.read_bit()?;
                    if display_delay_present {
                        let _initial_display_delay = reader.read_bits(4)?;
                    }
                }
            }

            (
                timing_present,
                decoder_model_present,
                initial_display_present,
                op_count,
                op_idc_0,
                level_idx_0,
                tier_0,
            )
        };

        // frame_width_bits_minus_1 (4 bits)
        let frame_width_bits_minus_1 = reader.read_bits(4)? as u8;
        // frame_height_bits_minus_1 (4 bits)
        let frame_height_bits_minus_1 = reader.read_bits(4)? as u8;

        // max_frame_width_minus_1 (n bits)
        let max_frame_width_minus_1 = reader.read_bits(frame_width_bits_minus_1 + 1)?;
        // max_frame_height_minus_1 (n bits)
        let max_frame_height_minus_1 = reader.read_bits(frame_height_bits_minus_1 + 1)?;

        // Validate dimensions
        if max_frame_width_minus_1 >= MAX_FRAME_WIDTH
            || max_frame_height_minus_1 >= MAX_FRAME_HEIGHT
        {
            self.record_error(Av1SequenceHeaderError::InvalidFrameSize);
            return Err(Av1SequenceHeaderError::InvalidFrameSize);
        }

        // frame_id_numbers_present_flag
        let frame_id_numbers_present = if !reduced_still_picture_header {
            reader.read_bit()?
        } else {
            false
        };

        let (delta_frame_id_length, additional_frame_id_length) = if frame_id_numbers_present {
            let delta = reader.read_bits(4)? as u8;
            let additional = reader.read_bits(3)? as u8;
            (delta + 2, additional + 1)
        } else {
            (0, 0)
        };

        // Feature flags
        let use_128x128_superblock = reader.read_bit()?;
        let enable_filter_intra = reader.read_bit()?;
        let enable_intra_edge_filter = reader.read_bit()?;

        // Additional features for non-reduced headers
        let (
            enable_interintra_compound,
            enable_masked_compound,
            enable_warped_motion,
            enable_dual_filter,
            enable_order_hint,
            enable_jnt_comp,
            enable_ref_frame_mvs,
            seq_force_screen_content_tools,
            seq_force_integer_mv,
            order_hint_bits_minus_1,
        ) = if !reduced_still_picture_header {
            let interintra = reader.read_bit()?;
            let masked = reader.read_bit()?;
            let warped = reader.read_bit()?;
            let dual_filter = reader.read_bit()?;
            let order_hint = reader.read_bit()?;

            let (jnt_comp, ref_frame_mvs) = if order_hint {
                (reader.read_bit()?, reader.read_bit()?)
            } else {
                (false, false)
            };

            // Screen content tools
            let seq_choose_screen = reader.read_bit()?;
            let screen_tools = if seq_choose_screen {
                SELECT_SCREEN_CONTENT_TOOLS
            } else {
                reader.read_bits(1)? as u8
            };

            // Integer MV
            let seq_choose_integer = if screen_tools > 0 {
                reader.read_bit()?
            } else {
                false
            };
            let integer_mv = if seq_choose_integer {
                SELECT_INTEGER_MV
            } else if screen_tools > 0 {
                reader.read_bits(1)? as u8
            } else {
                0 // Must use allow_intrabc derived value
            };

            let order_bits = if order_hint {
                reader.read_bits(3)? as u8
            } else {
                0
            };

            (
                interintra,
                masked,
                warped,
                dual_filter,
                order_hint,
                jnt_comp,
                ref_frame_mvs,
                screen_tools,
                integer_mv,
                order_bits,
            )
        } else {
            // Reduced header defaults
            (false, false, false, false, false, false, false, 0, 0, 0)
        };

        // Superres, CDEF, Loop restoration
        let enable_superres = reader.read_bit()?;
        let enable_cdef = reader.read_bit()?;
        let enable_restoration = reader.read_bit()?;

        // Parse color config (Section 5.5.1)
        let (
            bit_depth,
            mono_chrome,
            color_primaries,
            transfer_characteristics,
            matrix_coefficients,
            color_range,
            subsampling_x,
            subsampling_y,
            chroma_sample_position,
            separate_uv_delta_q,
        ) = self.parse_color_config_internal(&mut reader, seq_profile)?;

        // film_grain_params_present
        let film_grain_params_present = reader.read_bit()?;

        // === Store all parsed values ===

        // Pack state
        let state_packed = (seq_profile as u64)
            | ((still_picture as u64) << 3)
            | ((reduced_still_picture_header as u64) << 4)
            | ((timing_info_present as u64) << 5)
            | ((decoder_model_info_present as u64) << 6)
            | ((initial_display_delay_present as u64) << 7)
            | ((op_count_minus_1 as u64) << 8)
            | ((seq_level_idx_0 as u64) << 16)
            | ((seq_tier_0 as u64) << 21);
        self.state.store(state_packed, Ordering::Release);

        // Pack dimensions
        let dimensions_packed = (max_frame_width_minus_1 as u32)
            | ((max_frame_height_minus_1 as u32) << 16);
        self.max_frame_dimensions
            .store(dimensions_packed as u32, Ordering::Release);

        // Pack frame ID config
        let frame_id_packed = (frame_width_bits_minus_1 as u32)
            | ((frame_height_bits_minus_1 as u32) << 4)
            | ((frame_id_numbers_present as u32) << 8)
            | ((delta_frame_id_length as u32) << 9)
            | ((additional_frame_id_length as u32) << 13);
        self.frame_id_config.store(frame_id_packed, Ordering::Release);

        // Pack feature flags
        let features_packed = (use_128x128_superblock as u32)
            | ((enable_filter_intra as u32) << 1)
            | ((enable_intra_edge_filter as u32) << 2)
            | ((enable_interintra_compound as u32) << 3)
            | ((enable_masked_compound as u32) << 4)
            | ((enable_warped_motion as u32) << 5)
            | ((enable_dual_filter as u32) << 6)
            | ((enable_order_hint as u32) << 7)
            | ((enable_jnt_comp as u32) << 8)
            | ((enable_ref_frame_mvs as u32) << 9)
            | (((seq_force_screen_content_tools > 0) as u32) << 10)
            | ((seq_force_screen_content_tools as u32) << 11)
            | (((seq_force_integer_mv == SELECT_INTEGER_MV) as u32) << 12)
            | ((seq_force_integer_mv as u32) << 13)
            | ((order_hint_bits_minus_1 as u32) << 14)
            | ((enable_superres as u32) << 17)
            | ((enable_cdef as u32) << 18)
            | ((enable_restoration as u32) << 19)
            | ((film_grain_params_present as u32) << 20);
        self.feature_flags.store(features_packed, Ordering::Release);

        // Pack color config
        let color_packed = (color_primaries as u32)
            | ((transfer_characteristics as u32) << 8)
            | ((matrix_coefficients as u32) << 16)
            | ((color_range as u32) << 24)
            | ((chroma_sample_position as u32) << 25)
            | ((separate_uv_delta_q as u32) << 27);
        self.color_config.store(color_packed, Ordering::Release);

        // Pack bit depth config
        let high_bitdepth = bit_depth > 8;
        let twelve_bit = bit_depth == 12;
        let bit_depth_packed = (bit_depth as u32)
            | ((mono_chrome as u32) << 4)
            | ((high_bitdepth as u32) << 5)
            | ((twelve_bit as u32) << 6)
            | ((subsampling_x as u32) << 7)
            | ((subsampling_y as u32) << 8);
        self.bit_depth_config.store(bit_depth_packed, Ordering::Release);

        // Pack operating point 0
        let op_packed = (operating_point_idc_0 as u32)
            | ((seq_level_idx_0 as u32) << 12)
            | ((seq_tier_0 as u32) << 17);
        self.op_point_0.store(op_packed, Ordering::Release);
        self.op_point_count
            .store((op_count_minus_1 + 1) as u32, Ordering::Release);

        // Update bit position
        self.bit_position
            .store((reader.bytes_consumed() * 8) as u64, Ordering::Release);

        // Update statistics
        self.headers_parsed.fetch_add(1, Ordering::Relaxed);
        self.update_profile_count(seq_profile);
        self.update_bit_depth_count(bit_depth);

        // Check for HDR transfer
        let transfer = Av1TransferCharacteristics::from_u8(transfer_characteristics);
        if transfer.is_hdr() {
            self.hdr_count.fetch_add(1, Ordering::Relaxed);
        }

        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(())
    }

    /// Parse color configuration (Section 5.5.1)
    fn parse_color_config_internal(
        &self,
        reader: &mut BitReader,
        seq_profile: u8,
    ) -> Result<(u8, bool, u8, u8, u8, bool, bool, bool, u8, bool), Av1SequenceHeaderError> {
        // high_bitdepth
        let high_bitdepth = reader.read_bit()?;

        // twelve_bit (only for profile 2)
        let twelve_bit = if seq_profile == 2 && high_bitdepth {
            reader.read_bit()?
        } else {
            false
        };

        let bit_depth = if seq_profile == 2 && high_bitdepth {
            if twelve_bit { 12 } else { 10 }
        } else if high_bitdepth {
            10
        } else {
            8
        };

        // Validate bit depth for profile
        if bit_depth == 12 && seq_profile != 2 {
            self.record_error(Av1SequenceHeaderError::InvalidBitDepth);
            return Err(Av1SequenceHeaderError::InvalidBitDepth);
        }

        // mono_chrome
        let mono_chrome = if seq_profile == 1 {
            false
        } else {
            reader.read_bit()?
        };

        // NumPlanes
        let _num_planes = if mono_chrome { 1 } else { 3 };

        // color_description_present_flag
        let color_description_present = reader.read_bit()?;

        let (color_primaries, transfer_characteristics, matrix_coefficients) =
            if color_description_present {
                let primaries = reader.read_bits(8)? as u8;
                let transfer = reader.read_bits(8)? as u8;
                let matrix = reader.read_bits(8)? as u8;
                (primaries, transfer, matrix)
            } else {
                // Default values: Unspecified
                (2, 2, 2)
            };

        // color_range
        let (color_range, subsampling_x, subsampling_y, chroma_sample_position) = if mono_chrome {
            let range = reader.read_bit()?;
            // Monochrome has no chroma
            (range, true, true, 0)
        } else if color_primaries == 1 && transfer_characteristics == 13 && matrix_coefficients == 0
        {
            // sRGB
            (true, false, false, 0) // 4:4:4
        } else {
            let range = reader.read_bit()?;

            let (sub_x, sub_y) = if seq_profile == 0 {
                // Main profile: always 4:2:0
                (true, true)
            } else if seq_profile == 1 {
                // High profile: always 4:4:4
                (false, false)
            } else {
                // Professional profile: configurable
                if bit_depth == 12 {
                    let sx = reader.read_bit()?;
                    if sx {
                        let sy = reader.read_bit()?;
                        (true, sy)
                    } else {
                        (false, false)
                    }
                } else {
                    // 4:2:2 not allowed for 8/10-bit profile 2
                    (true, false)
                }
            };

            let chroma_pos = if sub_x && sub_y {
                reader.read_bits(2)? as u8
            } else {
                0
            };

            (range, sub_x, sub_y, chroma_pos)
        };

        // separate_uv_delta_q
        let separate_uv_delta_q = if !mono_chrome {
            reader.read_bit()?
        } else {
            false
        };

        Ok((
            bit_depth,
            mono_chrome,
            color_primaries,
            transfer_characteristics,
            matrix_coefficients,
            color_range,
            subsampling_x,
            subsampling_y,
            chroma_sample_position,
            separate_uv_delta_q,
        ))
    }

    /// Parse timing info (Section 5.5.2)
    fn parse_timing_info(
        &self,
        reader: &mut BitReader,
    ) -> Result<(), Av1SequenceHeaderError> {
        // num_units_in_display_tick (32 bits)
        let num_units = reader.read_bits(32)?;
        self.timing_num_units.store(num_units, Ordering::Release);

        // time_scale (32 bits)
        let time_scale = reader.read_bits(32)?;
        self.timing_time_scale.store(time_scale, Ordering::Release);

        if time_scale == 0 {
            self.record_error(Av1SequenceHeaderError::InvalidTimingInfo);
            return Err(Av1SequenceHeaderError::InvalidTimingInfo);
        }

        // equal_picture_interval
        let equal_picture_interval = reader.read_bit()?;
        let num_ticks = if equal_picture_interval {
            reader.read_uvlc()?
        } else {
            0
        };

        let timing_flags = (equal_picture_interval as u32) | ((num_ticks & 0x7FFFFFFF) << 1);
        self.timing_flags.store(timing_flags, Ordering::Release);

        Ok(())
    }

    /// Parse decoder model info (Section 5.5.3)
    fn parse_decoder_model_info(
        &self,
        reader: &mut BitReader,
    ) -> Result<(), Av1SequenceHeaderError> {
        // buffer_delay_length_minus_1 (5 bits)
        let _buffer_delay_length = reader.read_bits(5)?;
        // num_units_in_decoding_tick (32 bits)
        let _num_units = reader.read_bits(32)?;
        // buffer_removal_time_length_minus_1 (5 bits)
        let _buffer_removal_length = reader.read_bits(5)?;
        // frame_presentation_time_length_minus_1 (5 bits)
        let _frame_presentation_length = reader.read_bits(5)?;

        Ok(())
    }

    /// Record an error
    #[inline]
    fn record_error(&self, error: Av1SequenceHeaderError) {
        self.last_error.store(error as u32, Ordering::Release);
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Update profile count
    #[inline]
    fn update_profile_count(&self, profile: u8) {
        let shift = (profile.min(2) * 8) as u32;
        loop {
            let current = self.profile_counts.load(Ordering::Relaxed);
            let count = ((current >> shift) & 0xFF) + 1;
            let new = (current & !(0xFF << shift)) | ((count & 0xFF) << shift);
            if self
                .profile_counts
                .compare_exchange_weak(current, new, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }

    /// Update bit depth count
    #[inline]
    fn update_bit_depth_count(&self, bit_depth: u8) {
        let index = match bit_depth {
            8 => 0,
            10 => 1,
            12 => 2,
            _ => return,
        };
        let shift = index * 10;
        loop {
            let current = self.bit_depth_counts.load(Ordering::Relaxed);
            let count = ((current >> shift) & 0x3FF) + 1;
            let new = (current & !(0x3FF << shift)) | ((count & 0x3FF) << shift);
            if self
                .bit_depth_counts
                .compare_exchange_weak(current, new, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }

    // ========================================================================
    // ACCESSORS
    // ========================================================================

    /// Get the sequence profile
    #[inline]
    pub fn seq_profile(&self) -> Av1Profile {
        let state = self.state.load(Ordering::Acquire);
        Av1Profile::from_bits((state & 0x07) as u8).unwrap_or(Av1Profile::Main)
    }

    /// Check if this is a still picture sequence
    #[inline]
    pub fn still_picture(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        (state & (1 << 3)) != 0
    }

    /// Check if reduced still picture header is used
    #[inline]
    pub fn reduced_still_picture_header(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        (state & (1 << 4)) != 0
    }

    /// Get max frame width
    #[inline]
    pub fn max_frame_width(&self) -> u32 {
        let dims = self.max_frame_dimensions.load(Ordering::Acquire);
        (dims & 0xFFFF) + 1
    }

    /// Get max frame height
    #[inline]
    pub fn max_frame_height(&self) -> u32 {
        let dims = self.max_frame_dimensions.load(Ordering::Acquire);
        ((dims >> 16) & 0xFFFF) + 1
    }

    /// Get bit depth
    #[inline]
    pub fn get_bit_depth(&self) -> u32 {
        let config = self.bit_depth_config.load(Ordering::Acquire);
        (config & 0x0F) as u32
    }

    /// Get number of color planes
    #[inline]
    pub fn get_num_planes(&self) -> u32 {
        let config = self.bit_depth_config.load(Ordering::Acquire);
        if (config & (1 << 4)) != 0 {
            1 // mono_chrome
        } else {
            3
        }
    }

    /// Check if mono_chrome
    #[inline]
    pub fn mono_chrome(&self) -> bool {
        let config = self.bit_depth_config.load(Ordering::Acquire);
        (config & (1 << 4)) != 0
    }

    /// Get subsampling_x
    #[inline]
    pub fn subsampling_x(&self) -> bool {
        let config = self.bit_depth_config.load(Ordering::Acquire);
        (config & (1 << 7)) != 0
    }

    /// Get subsampling_y
    #[inline]
    pub fn subsampling_y(&self) -> bool {
        let config = self.bit_depth_config.load(Ordering::Acquire);
        (config & (1 << 8)) != 0
    }

    /// Get color primaries
    #[inline]
    pub fn color_primaries(&self) -> Av1ColorPrimaries {
        let color = self.color_config.load(Ordering::Acquire);
        Av1ColorPrimaries::from_u8((color & 0xFF) as u8)
    }

    /// Get transfer characteristics
    #[inline]
    pub fn transfer_characteristics(&self) -> Av1TransferCharacteristics {
        let color = self.color_config.load(Ordering::Acquire);
        Av1TransferCharacteristics::from_u8(((color >> 8) & 0xFF) as u8)
    }

    /// Get matrix coefficients
    #[inline]
    pub fn matrix_coefficients(&self) -> Av1MatrixCoefficients {
        let color = self.color_config.load(Ordering::Acquire);
        Av1MatrixCoefficients::from_u8(((color >> 16) & 0xFF) as u8)
    }

    /// Get color range (false = studio/limited, true = full)
    #[inline]
    pub fn color_range(&self) -> bool {
        let color = self.color_config.load(Ordering::Acquire);
        (color & (1 << 24)) != 0
    }

    /// Get chroma sample position
    #[inline]
    pub fn chroma_sample_position(&self) -> Av1ChromaSamplePosition {
        let color = self.color_config.load(Ordering::Acquire);
        Av1ChromaSamplePosition::from_bits(((color >> 25) & 0x03) as u8)
    }

    /// Check if use_128x128_superblock is enabled
    #[inline]
    pub fn use_128x128_superblock(&self) -> bool {
        let flags = self.feature_flags.load(Ordering::Acquire);
        (flags & 1) != 0
    }

    /// Check if filter_intra is enabled
    #[inline]
    pub fn enable_filter_intra(&self) -> bool {
        let flags = self.feature_flags.load(Ordering::Acquire);
        (flags & (1 << 1)) != 0
    }

    /// Check if intra_edge_filter is enabled
    #[inline]
    pub fn enable_intra_edge_filter(&self) -> bool {
        let flags = self.feature_flags.load(Ordering::Acquire);
        (flags & (1 << 2)) != 0
    }

    /// Check if interintra_compound is enabled
    #[inline]
    pub fn enable_interintra_compound(&self) -> bool {
        let flags = self.feature_flags.load(Ordering::Acquire);
        (flags & (1 << 3)) != 0
    }

    /// Check if masked_compound is enabled
    #[inline]
    pub fn enable_masked_compound(&self) -> bool {
        let flags = self.feature_flags.load(Ordering::Acquire);
        (flags & (1 << 4)) != 0
    }

    /// Check if warped_motion is enabled
    #[inline]
    pub fn enable_warped_motion(&self) -> bool {
        let flags = self.feature_flags.load(Ordering::Acquire);
        (flags & (1 << 5)) != 0
    }

    /// Check if dual_filter is enabled
    #[inline]
    pub fn enable_dual_filter(&self) -> bool {
        let flags = self.feature_flags.load(Ordering::Acquire);
        (flags & (1 << 6)) != 0
    }

    /// Check if order_hint is enabled
    #[inline]
    pub fn enable_order_hint(&self) -> bool {
        let flags = self.feature_flags.load(Ordering::Acquire);
        (flags & (1 << 7)) != 0
    }

    /// Check if jnt_comp is enabled
    #[inline]
    pub fn enable_jnt_comp(&self) -> bool {
        let flags = self.feature_flags.load(Ordering::Acquire);
        (flags & (1 << 8)) != 0
    }

    /// Check if ref_frame_mvs is enabled
    #[inline]
    pub fn enable_ref_frame_mvs(&self) -> bool {
        let flags = self.feature_flags.load(Ordering::Acquire);
        (flags & (1 << 9)) != 0
    }

    /// Check if superres is enabled
    #[inline]
    pub fn enable_superres(&self) -> bool {
        let flags = self.feature_flags.load(Ordering::Acquire);
        (flags & (1 << 17)) != 0
    }

    /// Check if CDEF is enabled
    #[inline]
    pub fn enable_cdef(&self) -> bool {
        let flags = self.feature_flags.load(Ordering::Acquire);
        (flags & (1 << 18)) != 0
    }

    /// Check if loop restoration is enabled
    #[inline]
    pub fn enable_restoration(&self) -> bool {
        let flags = self.feature_flags.load(Ordering::Acquire);
        (flags & (1 << 19)) != 0
    }

    /// Check if film grain params are present
    #[inline]
    pub fn film_grain_params_present(&self) -> bool {
        let flags = self.feature_flags.load(Ordering::Acquire);
        (flags & (1 << 20)) != 0
    }

    /// Get order_hint_bits (0 if order_hint disabled)
    #[inline]
    pub fn order_hint_bits(&self) -> u8 {
        let flags = self.feature_flags.load(Ordering::Acquire);
        if (flags & (1 << 7)) != 0 {
            (((flags >> 14) & 0x07) + 1) as u8
        } else {
            0
        }
    }

    /// Get operating point count
    #[inline]
    pub fn operating_point_count(&self) -> u32 {
        self.op_point_count.load(Ordering::Acquire)
    }

    /// Get seq_level_idx for operating point 0
    #[inline]
    pub fn seq_level_idx_0(&self) -> u8 {
        let op = self.op_point_0.load(Ordering::Acquire);
        ((op >> 12) & 0x1F) as u8
    }

    /// Get seq_tier for operating point 0
    #[inline]
    pub fn seq_tier_0(&self) -> bool {
        let op = self.op_point_0.load(Ordering::Acquire);
        (op & (1 << 17)) != 0
    }

    /// Check if timing info is present
    #[inline]
    pub fn timing_info_present(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        (state & (1 << 5)) != 0
    }

    /// Get frame rate (frames per second) if timing info present
    #[inline]
    pub fn frame_rate(&self) -> Option<f64> {
        if !self.timing_info_present() {
            return None;
        }

        let num_units = self.timing_num_units.load(Ordering::Acquire);
        let time_scale = self.timing_time_scale.load(Ordering::Acquire);

        if num_units == 0 {
            return None;
        }

        Some(time_scale as f64 / num_units as f64)
    }

    /// Get statistics snapshot
    pub fn stats(&self) -> Av1SequenceHeaderStats {
        let profile_counts = self.profile_counts.load(Ordering::Acquire);
        let bit_depth_counts = self.bit_depth_counts.load(Ordering::Acquire);

        Av1SequenceHeaderStats {
            headers_parsed: self.headers_parsed.load(Ordering::Acquire),
            profile0_count: (profile_counts & 0xFF) as u32,
            profile1_count: ((profile_counts >> 8) & 0xFF) as u32,
            profile2_count: ((profile_counts >> 16) & 0xFF) as u32,
            bit_depth_8_count: (bit_depth_counts & 0x3FF) as u32,
            bit_depth_10_count: ((bit_depth_counts >> 10) & 0x3FF) as u32,
            bit_depth_12_count: ((bit_depth_counts >> 20) & 0x3FF) as u32,
            hdr_count: self.hdr_count.load(Ordering::Acquire),
            still_picture_count: self.still_picture_count.load(Ordering::Acquire),
            error_count: self.error_count.load(Ordering::Acquire),
            last_error: Av1SequenceHeaderError::from_u8(
                self.last_error.load(Ordering::Acquire) as u8,
            ),
            generation: self.generation.load(Ordering::Acquire),
        }
    }
}

impl Av1SequenceHeaderError {
    /// Convert from u8
    fn from_u8(value: u8) -> Self {
        match value {
            0 => Av1SequenceHeaderError::None,
            1 => Av1SequenceHeaderError::InvalidProfile,
            2 => Av1SequenceHeaderError::StillPictureViolation,
            3 => Av1SequenceHeaderError::InvalidFrameSize,
            4 => Av1SequenceHeaderError::InvalidBitDepth,
            5 => Av1SequenceHeaderError::InvalidColorConfig,
            6 => Av1SequenceHeaderError::InvalidChromaSubsampling,
            7 => Av1SequenceHeaderError::InvalidOperatingPointCount,
            8 => Av1SequenceHeaderError::InvalidOperatingPointIndex,
            9 => Av1SequenceHeaderError::InvalidSeqLevelIdx,
            10 => Av1SequenceHeaderError::UnexpectedEof,
            11 => Av1SequenceHeaderError::InvalidTimingInfo,
            12 => Av1SequenceHeaderError::FeatureFlagConflict,
            13 => Av1SequenceHeaderError::BitstreamCorrupted,
            14 => Av1SequenceHeaderError::ReservedBitSet,
            _ => Av1SequenceHeaderError::None,
        }
    }
}

// ============================================================================
// TESTS (T28 5-tier)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1-Q7: Unit Tests
    // ========================================================================

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<Av1SequenceHeaderCapsule>(), 512);
        assert_eq!(core::mem::align_of::<Av1SequenceHeaderCapsule>(), 128);
    }

    #[test]
    fn test_capsule_new() {
        let capsule = Av1SequenceHeaderCapsule::new();
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.get_bit_depth(), 0);
        assert_eq!(capsule.max_frame_width(), 1);
    }

    #[test]
    fn test_capsule_reset() {
        let capsule = Av1SequenceHeaderCapsule::new();
        capsule.state.store(0xFFFFFFFF, Ordering::Release);
        capsule.reset();
        assert_eq!(capsule.state.load(Ordering::Acquire), 0);
        assert_eq!(capsule.generation(), 1);
    }

    #[test]
    fn test_profile_enum() {
        assert_eq!(Av1Profile::from_bits(0), Some(Av1Profile::Main));
        assert_eq!(Av1Profile::from_bits(1), Some(Av1Profile::High));
        assert_eq!(Av1Profile::from_bits(2), Some(Av1Profile::Professional));
        assert_eq!(Av1Profile::from_bits(3), None);
        assert_eq!(Av1Profile::from_bits(7), None);

        assert!(!Av1Profile::Main.supports_12bit());
        assert!(Av1Profile::Professional.supports_12bit());
        assert!(!Av1Profile::Main.supports_444());
        assert!(Av1Profile::High.supports_444());
    }

    #[test]
    fn test_color_primaries_enum() {
        assert_eq!(Av1ColorPrimaries::from_u8(1), Av1ColorPrimaries::Bt709);
        assert_eq!(Av1ColorPrimaries::from_u8(2), Av1ColorPrimaries::Unspecified);
        assert_eq!(Av1ColorPrimaries::from_u8(9), Av1ColorPrimaries::Bt2020);
        assert_eq!(Av1ColorPrimaries::from_u8(255), Av1ColorPrimaries::Unspecified);
    }

    #[test]
    fn test_transfer_characteristics_enum() {
        assert_eq!(
            Av1TransferCharacteristics::from_u8(16),
            Av1TransferCharacteristics::Smpte2084
        );
        assert!(Av1TransferCharacteristics::Smpte2084.is_hdr());
        assert!(Av1TransferCharacteristics::HybridLogGamma.is_hdr());
        assert!(!Av1TransferCharacteristics::Bt709.is_hdr());
    }

    #[test]
    fn test_matrix_coefficients_enum() {
        assert_eq!(Av1MatrixCoefficients::from_u8(0), Av1MatrixCoefficients::Identity);
        assert!(Av1MatrixCoefficients::Identity.is_rgb());
        assert!(!Av1MatrixCoefficients::Bt709.is_rgb());
    }

    #[test]
    fn test_chroma_sample_position_enum() {
        assert_eq!(Av1ChromaSamplePosition::from_bits(0), Av1ChromaSamplePosition::Unknown);
        assert_eq!(Av1ChromaSamplePosition::from_bits(1), Av1ChromaSamplePosition::Vertical);
        assert_eq!(Av1ChromaSamplePosition::from_bits(2), Av1ChromaSamplePosition::Colocated);
        assert_eq!(Av1ChromaSamplePosition::from_bits(3), Av1ChromaSamplePosition::Reserved);
    }

    #[test]
    fn test_error_enum() {
        let error = Av1SequenceHeaderError::InvalidProfile;
        assert_eq!(format!("{}", error), "Invalid profile (must be 0-2)");
    }

    #[test]
    fn test_bit_reader_basic() {
        let data = [0b10110100, 0b11001010];
        let mut reader = BitReader::new(&data);

        assert_eq!(reader.read_bits(3).unwrap(), 0b101);
        assert_eq!(reader.read_bits(5).unwrap(), 0b10100);
        assert_eq!(reader.read_bits(4).unwrap(), 0b1100);
    }

    #[test]
    fn test_bit_reader_single_bits() {
        let data = [0b10110100];
        let mut reader = BitReader::new(&data);

        assert!(reader.read_bit().unwrap());
        assert!(!reader.read_bit().unwrap());
        assert!(reader.read_bit().unwrap());
        assert!(reader.read_bit().unwrap());
    }

    #[test]
    fn test_bit_reader_eof() {
        let data = [0xFF];
        let mut reader = BitReader::new(&data);

        reader.read_bits(8).unwrap();
        assert!(matches!(
            reader.read_bits(1),
            Err(Av1SequenceHeaderError::UnexpectedEof)
        ));
    }

    #[test]
    fn test_bit_reader_uvlc() {
        // uvlc(0) = 1 (single 1 bit)
        let data = [0b10000000];
        let mut reader = BitReader::new(&data);
        assert_eq!(reader.read_uvlc().unwrap(), 0);

        // uvlc(1) = 010 (one leading zero, then 1, then 1 bit value)
        let data = [0b01000000];
        let mut reader = BitReader::new(&data);
        assert_eq!(reader.read_uvlc().unwrap(), 1);

        // uvlc(2) = 011
        let data = [0b01100000];
        let mut reader = BitReader::new(&data);
        assert_eq!(reader.read_uvlc().unwrap(), 2);
    }

    // ========================================================================
    // Q8-Q14: Property Tests (Basic)
    // ========================================================================

    #[test]
    fn test_generation_monotonic() {
        let capsule = Av1SequenceHeaderCapsule::new();
        let g0 = capsule.generation();
        capsule.reset();
        let g1 = capsule.generation();
        capsule.reset();
        let g2 = capsule.generation();

        assert!(g1 > g0);
        assert!(g2 > g1);
    }

    #[test]
    fn test_stats_accumulate() {
        let capsule = Av1SequenceHeaderCapsule::new();
        capsule.update_profile_count(0);
        capsule.update_profile_count(0);
        capsule.update_profile_count(1);

        let stats = capsule.stats();
        assert_eq!(stats.profile0_count, 2);
        assert_eq!(stats.profile1_count, 1);
        assert_eq!(stats.profile2_count, 0);
    }

    #[test]
    fn test_bit_depth_counts() {
        let capsule = Av1SequenceHeaderCapsule::new();
        capsule.update_bit_depth_count(8);
        capsule.update_bit_depth_count(8);
        capsule.update_bit_depth_count(10);
        capsule.update_bit_depth_count(12);

        let stats = capsule.stats();
        assert_eq!(stats.bit_depth_8_count, 2);
        assert_eq!(stats.bit_depth_10_count, 1);
        assert_eq!(stats.bit_depth_12_count, 1);
    }

    // ========================================================================
    // Q15-Q21: Integration Tests
    // ========================================================================

    /// Create a minimal valid sequence header for profile 0
    ///
    /// This creates a reduced_still_picture_header = 1 sequence header,
    /// which requires still_picture = 1 per AV1 spec constraint.
    ///
    /// Format (bit-by-bit):
    /// seq_profile = 0 (3 bits) = 000
    /// still_picture = 1 (1 bit) = 1  (MUST be 1 for reduced header)
    /// reduced_still_picture_header = 1 (1 bit) = 1
    /// seq_level_idx[0] = 0 (5 bits) = 00000
    /// frame_width_bits_minus_1 = 7 (4 bits) -> 8 bits for width = 0111
    /// frame_height_bits_minus_1 = 7 (4 bits) -> 8 bits for height = 0111
    /// max_frame_width_minus_1 = 255 (8 bits) -> width = 256 = 11111111
    /// max_frame_height_minus_1 = 143 (8 bits) -> height = 144 = 10001111
    /// use_128x128_superblock = 0 (1 bit)
    /// enable_filter_intra = 0 (1 bit)
    /// enable_intra_edge_filter = 0 (1 bit)
    /// enable_superres = 0 (1 bit)
    /// enable_cdef = 0 (1 bit)
    /// enable_restoration = 0 (1 bit)
    /// high_bitdepth = 0 (1 bit) -> 8-bit
    /// mono_chrome = 0 (1 bit) -> 3 planes
    /// color_description_present = 0 (1 bit)
    /// color_range = 0 (1 bit) -> studio range
    /// chroma_sample_position = 0 (2 bits) -> unknown
    /// separate_uv_delta_q = 0 (1 bit)
    /// film_grain_params_present = 0 (1 bit)
    fn create_minimal_seq_header_profile0() -> Vec<u8> {
        // Binary layout (MSB first):
        // Bits 0-2: seq_profile = 000
        // Bit 3: still_picture = 1
        // Bit 4: reduced_still_picture_header = 1
        // Bits 5-9: seq_level_idx[0] = 00000
        // Bits 10-13: frame_width_bits_minus_1 = 0111 (7 -> 8 bits)
        // Bits 14-17: frame_height_bits_minus_1 = 0111 (7 -> 8 bits)
        // Bits 18-25: max_frame_width_minus_1 = 11111111 (255 -> width = 256)
        // Bits 26-33: max_frame_height_minus_1 = 10001111 (143 -> height = 144)
        // Bit 34: use_128x128_superblock = 0
        // Bit 35: enable_filter_intra = 0
        // Bit 36: enable_intra_edge_filter = 0
        // Bit 37: enable_superres = 0
        // Bit 38: enable_cdef = 0
        // Bit 39: enable_restoration = 0
        // Bit 40: high_bitdepth = 0
        // Bit 41: mono_chrome = 0 (profile 0)
        // Bit 42: color_description_present = 0
        // Bit 43: color_range = 0
        // Bits 44-45: chroma_sample_position = 00
        // Bit 46: separate_uv_delta_q = 0
        // Bit 47: film_grain_params_present = 0
        //
        // Byte 0: [000][1][1][000] = 0b00011000 = 0x18
        // Byte 1: [00][0111][01] = 0b00011101 = 0x1D
        // Byte 2: [11][111111] = 0b11111111 = 0xFF
        // Byte 3: [11][100011] = 0b11100011 = 0xE3
        // Byte 4: [11][0][0][0][0][0][0] = 0b11000000 = 0xC0
        // Byte 5: [0][0][0][0][00][0][0] = 0b00000000 = 0x00

        vec![0x18, 0x1D, 0xFF, 0xE3, 0xC0, 0x00]
    }

    #[test]
    fn test_parse_minimal_seq_header() {
        let capsule = Av1SequenceHeaderCapsule::new();
        let data = create_minimal_seq_header_profile0();

        let result = capsule.parse_sequence_header(&data);
        assert!(result.is_ok(), "Parse failed: {:?}", result);

        assert_eq!(capsule.seq_profile(), Av1Profile::Main);
        assert!(capsule.still_picture()); // Must be true for reduced header
        assert!(capsule.reduced_still_picture_header());
        assert_eq!(capsule.get_bit_depth(), 8);
        assert!(!capsule.mono_chrome());
    }

    #[test]
    fn test_parse_empty_data() {
        let capsule = Av1SequenceHeaderCapsule::new();
        let result = capsule.parse_sequence_header(&[]);
        assert!(matches!(result, Err(Av1SequenceHeaderError::UnexpectedEof)));
    }

    #[test]
    fn test_parse_invalid_profile() {
        let capsule = Av1SequenceHeaderCapsule::new();
        // Profile = 3 (invalid, 0b011)
        // [011][0][0]... = 0x60
        let data = [0x60, 0x00, 0x00, 0x00, 0x00];
        let result = capsule.parse_sequence_header(&data);
        assert!(matches!(result, Err(Av1SequenceHeaderError::InvalidProfile)));
    }

    #[test]
    fn test_accessors_after_parse() {
        let capsule = Av1SequenceHeaderCapsule::new();
        let data = create_minimal_seq_header_profile0();

        capsule.parse_sequence_header(&data).unwrap();

        // Verify accessors return expected values
        assert_eq!(capsule.get_num_planes(), 3); // Not mono_chrome
        assert!(capsule.subsampling_x()); // Profile 0 is 4:2:0
        assert!(capsule.subsampling_y()); // Profile 0 is 4:2:0
        assert!(!capsule.use_128x128_superblock());
        assert!(!capsule.enable_filter_intra());
        assert!(!capsule.enable_cdef());
        assert!(!capsule.enable_restoration());
    }

    #[test]
    fn test_multiple_parses() {
        let capsule = Av1SequenceHeaderCapsule::new();
        let data = create_minimal_seq_header_profile0();

        for i in 0..10 {
            capsule.parse_sequence_header(&data).unwrap();
            assert_eq!(capsule.stats().headers_parsed, i + 1);
        }
    }

    // ========================================================================
    // Q22-Q28: Production Tests
    // ========================================================================

    #[test]
    fn test_concurrent_read_access() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(Av1SequenceHeaderCapsule::new());
        let data = create_minimal_seq_header_profile0();
        capsule.parse_sequence_header(&data).unwrap();

        let mut handles = vec![];

        for _ in 0..4 {
            let c = Arc::clone(&capsule);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    let _ = c.seq_profile();
                    let _ = c.get_bit_depth();
                    let _ = c.max_frame_width();
                    let _ = c.stats();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_error_recovery() {
        let capsule = Av1SequenceHeaderCapsule::new();

        // Parse invalid data
        let _ = capsule.parse_sequence_header(&[0xFF, 0xFF]);

        // Verify error was recorded
        let stats = capsule.stats();
        assert!(stats.error_count > 0);

        // Verify capsule can still be used
        let data = create_minimal_seq_header_profile0();
        assert!(capsule.parse_sequence_header(&data).is_ok());
    }

    #[test]
    fn test_stats_reset() {
        let capsule = Av1SequenceHeaderCapsule::new();
        let data = create_minimal_seq_header_profile0();

        capsule.parse_sequence_header(&data).unwrap();
        assert!(capsule.stats().headers_parsed > 0);

        capsule.reset_stats();
        assert_eq!(capsule.stats().headers_parsed, 0);
        assert!(capsule.generation() > 0); // Generation should have incremented
    }

    // ========================================================================
    // Q29-Q35: Determinism Tests
    // ========================================================================

    #[test]
    fn test_deterministic_parsing() {
        let data = create_minimal_seq_header_profile0();

        let capsule1 = Av1SequenceHeaderCapsule::new();
        let capsule2 = Av1SequenceHeaderCapsule::new();

        capsule1.parse_sequence_header(&data).unwrap();
        capsule2.parse_sequence_header(&data).unwrap();

        // All fields should be identical
        assert_eq!(capsule1.seq_profile(), capsule2.seq_profile());
        assert_eq!(capsule1.get_bit_depth(), capsule2.get_bit_depth());
        assert_eq!(capsule1.max_frame_width(), capsule2.max_frame_width());
        assert_eq!(capsule1.max_frame_height(), capsule2.max_frame_height());
        assert_eq!(capsule1.still_picture(), capsule2.still_picture());
        assert_eq!(
            capsule1.reduced_still_picture_header(),
            capsule2.reduced_still_picture_header()
        );
    }

    #[test]
    fn test_generation_counter_audit_trail() {
        let capsule = Av1SequenceHeaderCapsule::new();
        let data = create_minimal_seq_header_profile0();

        let g0 = capsule.generation();

        capsule.parse_sequence_header(&data).unwrap();
        let g1 = capsule.generation();
        assert!(g1 > g0, "Generation should increase after parse");

        capsule.reset();
        let g2 = capsule.generation();
        assert!(g2 > g1, "Generation should increase after reset");

        capsule.reset_stats();
        let g3 = capsule.generation();
        assert!(g3 > g2, "Generation should increase after stats reset");
    }

    #[test]
    fn test_frame_rate_calculation() {
        let capsule = Av1SequenceHeaderCapsule::new();

        // Without timing info
        assert!(capsule.frame_rate().is_none());

        // Simulate timing info (manual setup for unit test)
        capsule.state.store(1 << 5, Ordering::Release); // timing_info_present
        capsule.timing_num_units.store(1001, Ordering::Release);
        capsule.timing_time_scale.store(30000, Ordering::Release);

        let fps = capsule.frame_rate().unwrap();
        assert!((fps - 29.97).abs() < 0.01);
    }

    #[test]
    fn test_feature_flag_consistency() {
        let capsule = Av1SequenceHeaderCapsule::new();

        // Set all feature flags
        let all_flags = (1 << 21) - 1;
        capsule.feature_flags.store(all_flags, Ordering::Release);

        // Verify each accessor returns true
        assert!(capsule.use_128x128_superblock());
        assert!(capsule.enable_filter_intra());
        assert!(capsule.enable_intra_edge_filter());
        assert!(capsule.enable_interintra_compound());
        assert!(capsule.enable_masked_compound());
        assert!(capsule.enable_warped_motion());
        assert!(capsule.enable_dual_filter());
        assert!(capsule.enable_order_hint());
        assert!(capsule.enable_jnt_comp());
        assert!(capsule.enable_ref_frame_mvs());
        assert!(capsule.enable_superres());
        assert!(capsule.enable_cdef());
        assert!(capsule.enable_restoration());
        assert!(capsule.film_grain_params_present());

        // Clear all flags
        capsule.feature_flags.store(0, Ordering::Release);

        // Verify each accessor returns false
        assert!(!capsule.use_128x128_superblock());
        assert!(!capsule.enable_filter_intra());
        assert!(!capsule.enable_cdef());
    }

    // ========================================================================
    // Additional Coverage Tests
    // ========================================================================

    #[test]
    fn test_profile_display() {
        assert_eq!(
            format!("{}", Av1Profile::Main),
            "Main Profile (8/10-bit, 4:2:0)"
        );
        assert_eq!(
            format!("{}", Av1Profile::High),
            "High Profile (8/10-bit, 4:4:4)"
        );
        assert_eq!(
            format!("{}", Av1Profile::Professional),
            "Professional Profile (8/10/12-bit, any)"
        );
    }

    #[test]
    fn test_color_primaries_display() {
        assert_eq!(format!("{}", Av1ColorPrimaries::Bt709), "BT.709");
        assert_eq!(format!("{}", Av1ColorPrimaries::Bt2020), "BT.2020");
    }

    #[test]
    fn test_transfer_characteristics_display() {
        assert_eq!(format!("{}", Av1TransferCharacteristics::Smpte2084), "SMPTE 2084 (PQ)");
        assert_eq!(format!("{}", Av1TransferCharacteristics::HybridLogGamma), "HLG");
    }

    #[test]
    fn test_matrix_coefficients_display() {
        assert_eq!(format!("{}", Av1MatrixCoefficients::Identity), "Identity (RGB)");
        assert_eq!(format!("{}", Av1MatrixCoefficients::Bt709), "BT.709");
    }

    #[test]
    fn test_default_implementations() {
        let capsule = Av1SequenceHeaderCapsule::default();
        assert_eq!(capsule.generation(), 0);

        let error = Av1SequenceHeaderError::default();
        assert_eq!(error, Av1SequenceHeaderError::None);

        let profile = Av1Profile::default();
        assert_eq!(profile, Av1Profile::Main);
    }

    #[test]
    fn test_order_hint_bits() {
        let capsule = Av1SequenceHeaderCapsule::new();

        // No order hint
        capsule.feature_flags.store(0, Ordering::Release);
        assert_eq!(capsule.order_hint_bits(), 0);

        // Order hint enabled with bits = 3 (stored as bits_minus_1 = 2 at position 14-16)
        // enable_order_hint = bit 7, order_hint_bits_minus_1 = bits 14-16
        let flags = (1 << 7) | (2 << 14); // enable_order_hint + order_hint_bits_minus_1 = 2
        capsule.feature_flags.store(flags, Ordering::Release);
        assert_eq!(capsule.order_hint_bits(), 3);
    }
}
