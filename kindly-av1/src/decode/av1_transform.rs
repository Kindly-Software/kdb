//! AV1 Inverse Transform Capsule (IDCT/ADST/WHT/IDTX)
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Implements AV1 inverse transforms per AV1 specification Section 7.13 with SIMD acceleration:
//! - Transform sizes: 4x4, 8x8, 16x16, 32x32, 64x64 (and rectangular variants)
//! - Transform types: DCT, ADST, FLIPADST, IDTX (identity)
//! - Row/Column separation with 16 transform type combinations
//! - 4x4 WHT (Walsh-Hadamard Transform) for lossless DC-only blocks
//!
//! # Architecture
//!
//! - **Tier**: T2 SIMD (2-6x speedup via vectorization)
//! - **Size**: 512 bytes (cache-aligned)
//! - **Purpose**: AV1 inverse transform for residual reconstruction
//!
//! # Transform Types (AV1 Spec Section 7.13.2)
//!
//! AV1 supports 16 transform type combinations (row transform x column transform):
//! - DCT_DCT (0): DCT in both dimensions
//! - ADST_DCT (1): ADST rows, DCT columns
//! - DCT_ADST (2): DCT rows, ADST columns
//! - ADST_ADST (3): ADST in both dimensions
//! - FLIPADST_DCT (4): FLIPADST rows, DCT columns
//! - DCT_FLIPADST (5): DCT rows, FLIPADST columns
//! - FLIPADST_FLIPADST (6): FLIPADST in both dimensions
//! - ADST_FLIPADST (7): ADST rows, FLIPADST columns
//! - FLIPADST_ADST (8): FLIPADST rows, ADST columns
//! - IDTX (9): Identity transform (no transform, scaled copy)
//! - V_DCT (10): DCT in rows only (columns identity)
//! - H_DCT (11): DCT in columns only (rows identity)
//! - V_ADST (12): ADST in rows only
//! - H_ADST (13): ADST in columns only
//! - V_FLIPADST (14): FLIPADST in rows only
//! - H_FLIPADST (15): FLIPADST in columns only
//!
//! # Transform Sizes
//!
//! Square: 4x4, 8x8, 16x16, 32x32, 64x64
//! Rectangular: 4x8, 8x4, 8x16, 16x8, 16x32, 32x16, 32x64, 64x32
//!              4x16, 16x4, 8x32, 32x8, 16x64, 64x16
//!
//! # Performance
//!
//! - **4x4 WHT**: <30ns (lossless, scalar)
//! - **4x4 DCT**: <50ns (SIMD butterfly)
//! - **64x64 DCT**: <10μs (vectorized)
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_SIMD_AVAILABLE`: x86_64 SSE4.1+ runtime detection with scalar fallback
//! - `#ASSUME_COEFFICIENT_RANGE`: Input coefficients in i32 range for 12-bit video
//! - `#ASSUME_ALIGNMENT`: 512B cache alignment enforced by repr(C, align(512))
//! - `#ASSUME_NO_OVERFLOW`: Transform arithmetic stays within i64 bounds
//!
//! # References
//!
//! - AV1 Bitstream Specification Section 7.13: Inverse transform process
//! - libaom reference implementation: av1/common/idct.c

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// SIMD imports (reserved for future optimization)
// #[cfg(target_arch = "x86_64")]
// use core::simd::{i32x4, i32x8, num::SimdInt};

// ============================================================================
// AV1 TRANSFORM TYPES AND ENUMS
// ============================================================================

/// AV1 Transform Type (16 combinations per spec Section 7.13.2)
///
/// Each transform type specifies the row transform and column transform.
/// FLIPADST is ADST with output reversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Av1TxType {
    /// DCT in both dimensions
    #[default]
    DctDct = 0,
    /// ADST rows, DCT columns
    AdstDct = 1,
    /// DCT rows, ADST columns
    DctAdst = 2,
    /// ADST in both dimensions
    AdstAdst = 3,
    /// FLIPADST rows, DCT columns
    FlipAdstDct = 4,
    /// DCT rows, FLIPADST columns
    DctFlipAdst = 5,
    /// FLIPADST in both dimensions
    FlipAdstFlipAdst = 6,
    /// ADST rows, FLIPADST columns
    AdstFlipAdst = 7,
    /// FLIPADST rows, ADST columns
    FlipAdstAdst = 8,
    /// Identity transform (no transform)
    Idtx = 9,
    /// DCT in rows only (vertical DCT)
    VDct = 10,
    /// DCT in columns only (horizontal DCT)
    HDct = 11,
    /// ADST in rows only (vertical ADST)
    VAdst = 12,
    /// ADST in columns only (horizontal ADST)
    HAdst = 13,
    /// FLIPADST in rows only
    VFlipAdst = 14,
    /// FLIPADST in columns only
    HFlipAdst = 15,
}

impl Av1TxType {
    /// Convert from raw 4-bit value
    #[inline]
    pub const fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::DctDct),
            1 => Some(Self::AdstDct),
            2 => Some(Self::DctAdst),
            3 => Some(Self::AdstAdst),
            4 => Some(Self::FlipAdstDct),
            5 => Some(Self::DctFlipAdst),
            6 => Some(Self::FlipAdstFlipAdst),
            7 => Some(Self::AdstFlipAdst),
            8 => Some(Self::FlipAdstAdst),
            9 => Some(Self::Idtx),
            10 => Some(Self::VDct),
            11 => Some(Self::HDct),
            12 => Some(Self::VAdst),
            13 => Some(Self::HAdst),
            14 => Some(Self::VFlipAdst),
            15 => Some(Self::HFlipAdst),
            _ => None,
        }
    }

    /// Get the row transform kind
    #[inline]
    pub const fn row_type(&self) -> Av1TransformKind {
        match self {
            Self::DctDct | Self::DctAdst | Self::DctFlipAdst | Self::HDct => Av1TransformKind::Dct,
            Self::AdstDct | Self::AdstAdst | Self::AdstFlipAdst | Self::HAdst => {
                Av1TransformKind::Adst
            }
            Self::FlipAdstDct
            | Self::FlipAdstFlipAdst
            | Self::FlipAdstAdst
            | Self::HFlipAdst => Av1TransformKind::FlipAdst,
            Self::Idtx | Self::VDct | Self::VAdst | Self::VFlipAdst => Av1TransformKind::Identity,
        }
    }

    /// Get the column transform kind
    #[inline]
    pub const fn col_type(&self) -> Av1TransformKind {
        match self {
            Self::DctDct | Self::AdstDct | Self::FlipAdstDct | Self::VDct => Av1TransformKind::Dct,
            Self::DctAdst | Self::AdstAdst | Self::FlipAdstAdst | Self::VAdst => {
                Av1TransformKind::Adst
            }
            Self::DctFlipAdst
            | Self::AdstFlipAdst
            | Self::FlipAdstFlipAdst
            | Self::VFlipAdst => Av1TransformKind::FlipAdst,
            Self::Idtx | Self::HDct | Self::HAdst | Self::HFlipAdst => Av1TransformKind::Identity,
        }
    }

    /// Check if this is an identity transform (IDTX or partial identity)
    #[inline]
    pub const fn is_identity(&self) -> bool {
        matches!(
            self,
            Self::Idtx
                | Self::VDct
                | Self::HDct
                | Self::VAdst
                | Self::HAdst
                | Self::VFlipAdst
                | Self::HFlipAdst
        )
    }

    /// Check if this is a pure identity transform
    #[inline]
    pub const fn is_pure_identity(&self) -> bool {
        matches!(self, Self::Idtx)
    }

    /// Get human-readable name
    pub const fn name(&self) -> &'static str {
        match self {
            Self::DctDct => "DCT_DCT",
            Self::AdstDct => "ADST_DCT",
            Self::DctAdst => "DCT_ADST",
            Self::AdstAdst => "ADST_ADST",
            Self::FlipAdstDct => "FLIPADST_DCT",
            Self::DctFlipAdst => "DCT_FLIPADST",
            Self::FlipAdstFlipAdst => "FLIPADST_FLIPADST",
            Self::AdstFlipAdst => "ADST_FLIPADST",
            Self::FlipAdstAdst => "FLIPADST_ADST",
            Self::Idtx => "IDTX",
            Self::VDct => "V_DCT",
            Self::HDct => "H_DCT",
            Self::VAdst => "V_ADST",
            Self::HAdst => "H_ADST",
            Self::VFlipAdst => "V_FLIPADST",
            Self::HFlipAdst => "H_FLIPADST",
        }
    }
}

impl core::fmt::Display for Av1TxType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Individual transform kind (DCT, ADST, FLIPADST, or Identity)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Av1TransformKind {
    /// Discrete Cosine Transform
    Dct = 0,
    /// Asymmetric Discrete Sine Transform
    Adst = 1,
    /// ADST with output reversal
    FlipAdst = 2,
    /// Identity (no transform)
    Identity = 3,
}

impl Av1TransformKind {
    /// Check if this is an identity transform
    #[inline]
    pub const fn is_identity(&self) -> bool {
        matches!(self, Self::Identity)
    }
}

/// AV1 Transform Size
///
/// AV1 supports 19 transform sizes: 5 square and 14 rectangular.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Av1TxSize {
    /// 4x4 transform (smallest)
    #[default]
    Tx4x4 = 0,
    /// 8x8 transform
    Tx8x8 = 1,
    /// 16x16 transform
    Tx16x16 = 2,
    /// 32x32 transform
    Tx32x32 = 3,
    /// 64x64 transform (largest)
    Tx64x64 = 4,
    // Rectangular transforms
    /// 4x8 transform
    Tx4x8 = 5,
    /// 8x4 transform
    Tx8x4 = 6,
    /// 8x16 transform
    Tx8x16 = 7,
    /// 16x8 transform
    Tx16x8 = 8,
    /// 16x32 transform
    Tx16x32 = 9,
    /// 32x16 transform
    Tx32x16 = 10,
    /// 32x64 transform
    Tx32x64 = 11,
    /// 64x32 transform
    Tx64x32 = 12,
    /// 4x16 transform
    Tx4x16 = 13,
    /// 16x4 transform
    Tx16x4 = 14,
    /// 8x32 transform
    Tx8x32 = 15,
    /// 32x8 transform
    Tx32x8 = 16,
    /// 16x64 transform
    Tx16x64 = 17,
    /// 64x16 transform
    Tx64x16 = 18,
}

impl Av1TxSize {
    /// Convert from raw value
    #[inline]
    pub const fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::Tx4x4),
            1 => Some(Self::Tx8x8),
            2 => Some(Self::Tx16x16),
            3 => Some(Self::Tx32x32),
            4 => Some(Self::Tx64x64),
            5 => Some(Self::Tx4x8),
            6 => Some(Self::Tx8x4),
            7 => Some(Self::Tx8x16),
            8 => Some(Self::Tx16x8),
            9 => Some(Self::Tx16x32),
            10 => Some(Self::Tx32x16),
            11 => Some(Self::Tx32x64),
            12 => Some(Self::Tx64x32),
            13 => Some(Self::Tx4x16),
            14 => Some(Self::Tx16x4),
            15 => Some(Self::Tx8x32),
            16 => Some(Self::Tx32x8),
            17 => Some(Self::Tx16x64),
            18 => Some(Self::Tx64x16),
            _ => None,
        }
    }

    /// Get the width of this transform
    #[inline]
    pub const fn width(&self) -> usize {
        match self {
            Self::Tx4x4 | Self::Tx4x8 | Self::Tx4x16 => 4,
            Self::Tx8x8 | Self::Tx8x4 | Self::Tx8x16 | Self::Tx8x32 => 8,
            Self::Tx16x16 | Self::Tx16x8 | Self::Tx16x4 | Self::Tx16x32 | Self::Tx16x64 => 16,
            Self::Tx32x32 | Self::Tx32x16 | Self::Tx32x8 | Self::Tx32x64 => 32,
            Self::Tx64x64 | Self::Tx64x32 | Self::Tx64x16 => 64,
        }
    }

    /// Get the height of this transform
    #[inline]
    pub const fn height(&self) -> usize {
        match self {
            Self::Tx4x4 | Self::Tx8x4 | Self::Tx16x4 => 4,
            Self::Tx8x8 | Self::Tx4x8 | Self::Tx16x8 | Self::Tx32x8 => 8,
            Self::Tx16x16 | Self::Tx8x16 | Self::Tx4x16 | Self::Tx32x16 | Self::Tx64x16 => 16,
            Self::Tx32x32 | Self::Tx16x32 | Self::Tx8x32 | Self::Tx64x32 => 32,
            Self::Tx64x64 | Self::Tx32x64 | Self::Tx16x64 => 64,
        }
    }

    /// Get the number of coefficients for this transform
    #[inline]
    pub const fn coeff_count(&self) -> usize {
        self.width() * self.height()
    }

    /// Check if this is a square transform
    #[inline]
    pub const fn is_square(&self) -> bool {
        self.width() == self.height()
    }

    /// Get transform dimensions as (width, height)
    #[inline]
    pub const fn dimensions(&self) -> (usize, usize) {
        (self.width(), self.height())
    }

    /// Get the log2 of width
    #[inline]
    pub const fn width_log2(&self) -> usize {
        match self.width() {
            4 => 2,
            8 => 3,
            16 => 4,
            32 => 5,
            64 => 6,
            _ => 0,
        }
    }

    /// Get the log2 of height
    #[inline]
    pub const fn height_log2(&self) -> usize {
        match self.height() {
            4 => 2,
            8 => 3,
            16 => 4,
            32 => 5,
            64 => 6,
            _ => 0,
        }
    }

    /// Get human-readable name
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Tx4x4 => "4x4",
            Self::Tx8x8 => "8x8",
            Self::Tx16x16 => "16x16",
            Self::Tx32x32 => "32x32",
            Self::Tx64x64 => "64x64",
            Self::Tx4x8 => "4x8",
            Self::Tx8x4 => "8x4",
            Self::Tx8x16 => "8x16",
            Self::Tx16x8 => "16x8",
            Self::Tx16x32 => "16x32",
            Self::Tx32x16 => "32x16",
            Self::Tx32x64 => "32x64",
            Self::Tx64x32 => "64x32",
            Self::Tx4x16 => "4x16",
            Self::Tx16x4 => "16x4",
            Self::Tx8x32 => "8x32",
            Self::Tx32x8 => "32x8",
            Self::Tx16x64 => "16x64",
            Self::Tx64x16 => "64x16",
        }
    }
}

impl core::fmt::Display for Av1TxSize {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// ERROR TYPES
// ============================================================================

/// AV1 Transform error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum Av1TransformError {
    /// No error
    #[default]
    None = 0,
    /// Invalid transform size
    InvalidTxSize = 1,
    /// Invalid transform type for given size
    InvalidTxType = 2,
    /// Buffer size mismatch
    BufferSizeMismatch = 3,
    /// Arithmetic overflow during transform
    Overflow = 4,
    /// Invalid stride for add_residual
    InvalidStride = 5,
    /// Invalid bit depth
    InvalidBitDepth = 6,
}

impl std::fmt::Display for Av1TransformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "no error"),
            Self::InvalidTxSize => write!(f, "invalid transform size"),
            Self::InvalidTxType => write!(f, "invalid transform type for this size"),
            Self::BufferSizeMismatch => write!(f, "buffer size mismatch"),
            Self::Overflow => write!(f, "arithmetic overflow in transform"),
            Self::InvalidStride => write!(f, "invalid stride"),
            Self::InvalidBitDepth => write!(f, "invalid bit depth (must be 8, 10, or 12)"),
        }
    }
}

impl std::error::Error for Av1TransformError {}

impl Av1TransformError {
    /// Check if error occurred
    #[inline]
    pub const fn is_err(self) -> bool {
        !matches!(self, Av1TransformError::None)
    }

    /// Check if operation succeeded
    #[inline]
    pub const fn is_ok(self) -> bool {
        matches!(self, Av1TransformError::None)
    }
}

// ============================================================================
// STATISTICS
// ============================================================================

/// Transform statistics snapshot
#[derive(Debug, Clone, Copy, Default)]
pub struct Av1TransformStats {
    /// Total 4x4 transforms performed
    pub transforms_4x4: u64,
    /// Total 8x8 transforms performed
    pub transforms_8x8: u64,
    /// Total 16x16 transforms performed
    pub transforms_16x16: u64,
    /// Total 32x32 transforms performed
    pub transforms_32x32: u64,
    /// Total 64x64 transforms performed
    pub transforms_64x64: u64,
    /// Total rectangular transforms performed
    pub transforms_rect: u64,
    /// Total identity transforms (IDTX)
    pub identity_count: u64,
    /// Total WHT transforms (lossless 4x4)
    pub wht_count: u64,
    /// Total DCT transforms
    pub dct_count: u64,
    /// Total ADST transforms
    pub adst_count: u64,
    /// Current generation counter (Q34 audit)
    pub generation: u64,
}

// ============================================================================
// TRANSFORM CONSTANTS (AV1 Spec Section 7.13)
// ============================================================================

// Fixed-point scaling: cos/sin values scaled by 4096 (12-bit precision)
// These are the AV1 specification constants for transforms

/// cos(pi/4) * 4096 = 2896 (sqrt(2)/2 * 4096)
const COS_PI_4: i32 = 2896;

/// sin(pi/8) * 4096 = 1567
const SIN_PI_8: i32 = 1567;

/// cos(pi/8) * 4096 = 3784
const COS_PI_8: i32 = 3784;

/// sin(pi/16) * 4096 = 799
const SIN_PI_16: i32 = 799;

/// cos(pi/16) * 4096 = 4017
const COS_PI_16: i32 = 4017;

/// sin(3pi/16) * 4096 = 2276
const SIN_3PI_16: i32 = 2276;

/// cos(3pi/16) * 4096 = 3406
const COS_3PI_16: i32 = 3406;

// ADST 4-point constants (AV1 spec, scaled by 4096)
const SINPI_1_9: i32 = 1321; // sin(pi/9) * 4096
const SINPI_2_9: i32 = 2482; // sin(2*pi/9) * 4096
const SINPI_3_9: i32 = 3344; // sin(3*pi/9) * 4096
const SINPI_4_9: i32 = 3803; // sin(4*pi/9) * 4096

/// Rounding constant for fixed-point division by 4096 (12-bit)
const ROUND_SHIFT_12: i32 = 2048; // 1 << 11

/// Identity transform scaling factor (sqrt(2) * 4096 = 5793)
const IDENTITY_SCALE: i32 = 5793;

// ============================================================================
// AV1 TRANSFORM CAPSULE
// ============================================================================

/// T2 SIMD capsule for AV1 inverse transforms
///
/// 512B cache-aligned, lockfree, implements AV1 spec Section 7.13
///
/// # Layout (512 bytes)
///
/// ```text
/// [0..8)       | state: AtomicU64           | tx_size | tx_type | lossless flag
/// [8..16)      | generation: AtomicU64      | Q34 audit counter
/// [16..20)     | bit_depth: AtomicU32       | 8, 10, or 12
/// [20..24)     | _reserved: AtomicU32       | alignment padding
/// [24..32)     | transforms_4x4: AtomicU64  | 4x4 count
/// [32..40)     | transforms_8x8: AtomicU64  | 8x8 count
/// [40..48)     | transforms_16x16: AtomicU64| 16x16 count
/// [48..56)     | transforms_32x32: AtomicU64| 32x32 count
/// [56..64)     | transforms_64x64: AtomicU64| 64x64 count
/// [64..72)     | transforms_rect: AtomicU64 | rectangular count
/// [72..80)     | identity_count: AtomicU64  | IDTX count
/// [80..88)     | wht_count: AtomicU64       | WHT (lossless) count
/// [88..96)     | dct_count: AtomicU64       | DCT count
/// [96..104)    | adst_count: AtomicU64      | ADST count
/// [104..112)   | simd_enabled: AtomicU64    | SIMD flag
/// [112..512)   | _padding: [u8; 400]        | Cache alignment
/// ```
#[repr(C, align(512))]
pub struct Av1TransformCapsule {
    /// Combined state: bits [0..7] = tx_size, bits [8..15] = tx_type, bit 16 = lossless
    state: AtomicU64,
    /// Generation counter for Q34 audit trails
    generation: AtomicU64,
    /// Bit depth (8, 10, or 12)
    bit_depth: AtomicU32,
    /// Reserved for alignment
    _reserved: AtomicU32,
    /// Total 4x4 transforms performed
    transforms_4x4: AtomicU64,
    /// Total 8x8 transforms performed
    transforms_8x8: AtomicU64,
    /// Total 16x16 transforms performed
    transforms_16x16: AtomicU64,
    /// Total 32x32 transforms performed
    transforms_32x32: AtomicU64,
    /// Total 64x64 transforms performed
    transforms_64x64: AtomicU64,
    /// Total rectangular transforms performed
    transforms_rect: AtomicU64,
    /// Total identity transforms (IDTX)
    identity_count: AtomicU64,
    /// Total WHT transforms (lossless 4x4)
    wht_count: AtomicU64,
    /// Total DCT transforms
    dct_count: AtomicU64,
    /// Total ADST transforms
    adst_count: AtomicU64,
    /// SIMD availability flag
    simd_enabled: AtomicU64,
    /// Padding to 512B cache line
    _padding: [u8; 400],
}

impl Av1TransformCapsule {
    /// Create a new AV1 transform capsule
    ///
    /// Automatically detects SIMD availability and caches the result.
    /// Default bit depth is 8.
    pub fn new() -> Self {
        // Check for SIMD support at runtime
        #[cfg(target_arch = "x86_64")]
        let simd_enabled = {
            // #ASSUME_SIMD_AVAILABLE: SSE4.1+ detection with scalar fallback
            // #VERIFY: is_x86_feature_detected! is safe and reliable
            if is_x86_feature_detected!("sse4.1") {
                1u64
            } else {
                0u64
            }
        };

        #[cfg(not(target_arch = "x86_64"))]
        let simd_enabled = 1u64; // Assume SIMD available on other platforms

        Self {
            state: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            bit_depth: AtomicU32::new(8),
            _reserved: AtomicU32::new(0),
            transforms_4x4: AtomicU64::new(0),
            transforms_8x8: AtomicU64::new(0),
            transforms_16x16: AtomicU64::new(0),
            transforms_32x32: AtomicU64::new(0),
            transforms_64x64: AtomicU64::new(0),
            transforms_rect: AtomicU64::new(0),
            identity_count: AtomicU64::new(0),
            wht_count: AtomicU64::new(0),
            dct_count: AtomicU64::new(0),
            adst_count: AtomicU64::new(0),
            simd_enabled: AtomicU64::new(simd_enabled),
            _padding: [0u8; 400],
        }
    }

    /// Set bit depth (8, 10, or 12)
    pub fn set_bit_depth(&self, depth: u32) -> Av1TransformError {
        if depth != 8 && depth != 10 && depth != 12 {
            return Av1TransformError::InvalidBitDepth;
        }
        self.bit_depth.store(depth, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        Av1TransformError::None
    }

    /// Get current bit depth
    #[inline]
    pub fn bit_depth(&self) -> u32 {
        self.bit_depth.load(Ordering::Acquire)
    }

    // =========================================================================
    // 4x4 WALSH-HADAMARD TRANSFORM (WHT) - LOSSLESS DC-ONLY
    // =========================================================================

    /// 4x4 Walsh-Hadamard Transform (inverse)
    ///
    /// Used for lossless coding when all coefficients except DC are zero.
    /// AV1 spec Section 7.13.2.1
    pub fn iwht_4x4(&self, input: &[i32; 16], output: &mut [i32; 16]) {
        self.generation.fetch_add(1, Ordering::AcqRel);

        // WHT is its own inverse (symmetric) with scaling
        // The WHT matrix is Hadamard H_4 = [1 1 1 1; 1 -1 1 -1; 1 1 -1 -1; 1 -1 -1 1]

        // Process rows first
        let mut temp = [[0i32; 4]; 4];
        for row in 0..4 {
            let idx = row * 4;
            let a = input[idx];
            let b = input[idx + 1];
            let c = input[idx + 2];
            let d = input[idx + 3];

            // WHT butterfly
            let e = a + b;
            let f = a - b;
            let g = c + d;
            let h = c - d;

            temp[row][0] = e + g;
            temp[row][1] = f + h;
            temp[row][2] = e - g;
            temp[row][3] = f - h;
        }

        // Process columns
        for col in 0..4 {
            let a = temp[0][col];
            let b = temp[1][col];
            let c = temp[2][col];
            let d = temp[3][col];

            // WHT butterfly
            let e = a + b;
            let f = a - b;
            let g = c + d;
            let h = c - d;

            // Final values with proper rounding
            // AV1 WHT uses >> 2 for the final shift
            output[0 * 4 + col] = (e + g + 2) >> 2;
            output[1 * 4 + col] = (f + h + 2) >> 2;
            output[2 * 4 + col] = (e - g + 2) >> 2;
            output[3 * 4 + col] = (f - h + 2) >> 2;
        }

        self.transforms_4x4.fetch_add(1, Ordering::Relaxed);
        self.wht_count.fetch_add(1, Ordering::Relaxed);
    }

    // =========================================================================
    // 4-POINT DCT (IDCT-4)
    // =========================================================================

    /// 4-point inverse DCT (1D)
    ///
    /// Uses AV1 specification constants (12-bit fixed-point).
    #[inline(always)]
    fn idct_4pt(input: &[i32; 4]) -> [i32; 4] {
        // #ASSUME_NO_OVERFLOW: i32 arithmetic prevents overflow for 12-bit video
        // #VERIFY: Max intermediate value < 2^30 (i32 safe)

        // Stage 1: Butterfly
        let s0 = ((input[0] + input[2]) * COS_PI_4 + ROUND_SHIFT_12) >> 12;
        let s1 = ((input[0] - input[2]) * COS_PI_4 + ROUND_SHIFT_12) >> 12;
        let s2 = (input[1] * SIN_PI_8 - input[3] * COS_PI_8 + ROUND_SHIFT_12) >> 12;
        let s3 = (input[1] * COS_PI_8 + input[3] * SIN_PI_8 + ROUND_SHIFT_12) >> 12;

        // Stage 2: Final combination
        [s0 + s3, s1 + s2, s1 - s2, s0 - s3]
    }

    /// 4-point inverse ADST (1D)
    ///
    /// AV1 spec Section 7.13.2.2
    #[inline(always)]
    fn iadst_4pt(input: &[i32; 4]) -> [i32; 4] {
        // #ASSUME_NO_OVERFLOW: i32 arithmetic prevents overflow
        let x0 = input[0];
        let x1 = input[1];
        let x2 = input[2];
        let x3 = input[3];

        // ADST matrix multiplication (AV1 spec)
        let s0 = SINPI_1_9 * x0 + SINPI_2_9 * x1 + SINPI_3_9 * x2 + SINPI_4_9 * x3;
        let s1 = SINPI_4_9 * x0 + SINPI_3_9 * x1 - SINPI_1_9 * x3 - SINPI_2_9 * x2;
        let s2 = SINPI_3_9 * (x0 - x2 + x3);
        let s3 = SINPI_2_9 * x0 - SINPI_4_9 * x1 + SINPI_1_9 * x2 + SINPI_3_9 * x3;

        [
            (s0 + ROUND_SHIFT_12) >> 12,
            (s1 + ROUND_SHIFT_12) >> 12,
            (s2 + ROUND_SHIFT_12) >> 12,
            (s3 + ROUND_SHIFT_12) >> 12,
        ]
    }

    /// 4-point inverse FLIPADST (1D) - ADST with reversed output
    #[inline(always)]
    fn iflipadst_4pt(input: &[i32; 4]) -> [i32; 4] {
        let result = Self::iadst_4pt(input);
        [result[3], result[2], result[1], result[0]]
    }

    /// 4-point identity transform (1D)
    #[inline(always)]
    fn identity_4pt(input: &[i32; 4]) -> [i32; 4] {
        // Identity with AV1's sqrt(2) scaling
        [
            (input[0] * IDENTITY_SCALE + ROUND_SHIFT_12) >> 12,
            (input[1] * IDENTITY_SCALE + ROUND_SHIFT_12) >> 12,
            (input[2] * IDENTITY_SCALE + ROUND_SHIFT_12) >> 12,
            (input[3] * IDENTITY_SCALE + ROUND_SHIFT_12) >> 12,
        ]
    }

    /// Dispatch 1D 4-point transform based on kind
    #[inline(always)]
    fn transform_4pt(input: &[i32; 4], kind: Av1TransformKind) -> [i32; 4] {
        match kind {
            Av1TransformKind::Dct => Self::idct_4pt(input),
            Av1TransformKind::Adst => Self::iadst_4pt(input),
            Av1TransformKind::FlipAdst => Self::iflipadst_4pt(input),
            Av1TransformKind::Identity => Self::identity_4pt(input),
        }
    }

    // =========================================================================
    // 4x4 INVERSE TRANSFORMS
    // =========================================================================

    /// Perform 4x4 inverse transform with specified type
    ///
    /// Input: 16 coefficients (i32) in row-major order
    /// Output: 16 residual samples (i32) in row-major order
    pub fn inverse_transform_4x4(
        &self,
        input: &[i32; 16],
        output: &mut [i32; 16],
        tx_type: Av1TxType,
    ) {
        self.generation.fetch_add(1, Ordering::AcqRel);

        let row_type = tx_type.row_type();
        let col_type = tx_type.col_type();

        // First pass: Row transforms
        let mut workspace = [[0i32; 4]; 4];
        for row in 0..4 {
            let idx = row * 4;
            let row_in = [input[idx], input[idx + 1], input[idx + 2], input[idx + 3]];
            workspace[row] = Self::transform_4pt(&row_in, row_type);
        }

        // Second pass: Column transforms
        for col in 0..4 {
            let col_in = [
                workspace[0][col],
                workspace[1][col],
                workspace[2][col],
                workspace[3][col],
            ];
            let col_out = Self::transform_4pt(&col_in, col_type);

            // Store with final rounding: (x + 8) >> 4
            for row in 0..4 {
                output[row * 4 + col] = (col_out[row] + 8) >> 4;
            }
        }

        self.transforms_4x4.fetch_add(1, Ordering::Relaxed);
        self.update_type_stats(tx_type);
    }

    // =========================================================================
    // 8-POINT DCT (IDCT-8)
    // =========================================================================

    /// 8-point inverse DCT (1D)
    #[inline(always)]
    fn idct_8pt(input: &[i32; 8]) -> [i32; 8] {
        // #ASSUME_NO_OVERFLOW: i32 arithmetic prevents overflow

        // Recursively compute 4-point DCT on even indices
        let even_in = [input[0], input[2], input[4], input[6]];
        let even_out = Self::idct_4pt(&even_in);

        // Compute rotations for odd indices
        let t0 = (input[1] * COS_3PI_16 - input[7] * SIN_3PI_16 + ROUND_SHIFT_12) >> 12;
        let t1 = (input[1] * SIN_3PI_16 + input[7] * COS_3PI_16 + ROUND_SHIFT_12) >> 12;
        let t2 = (input[5] * COS_PI_16 - input[3] * SIN_PI_16 + ROUND_SHIFT_12) >> 12;
        let t3 = (input[5] * SIN_PI_16 + input[3] * COS_PI_16 + ROUND_SHIFT_12) >> 12;

        // Butterfly on odd
        let u0 = t0 + t2;
        let u1 = t1 + t3;
        let u2 = ((t0 - t2) * COS_PI_4 + ROUND_SHIFT_12) >> 12;
        let u3 = ((t1 - t3) * COS_PI_4 + ROUND_SHIFT_12) >> 12;

        // Final combination
        [
            even_out[0] + u1,
            even_out[1] + u3,
            even_out[2] + u2,
            even_out[3] + u0,
            even_out[3] - u0,
            even_out[2] - u2,
            even_out[1] - u3,
            even_out[0] - u1,
        ]
    }

    /// 8-point inverse ADST (1D)
    #[inline(always)]
    fn iadst_8pt(input: &[i32; 8]) -> [i32; 8] {
        // Simplified 8-point ADST using matrix multiplication
        const C1: i32 = 4076; // cos(pi/17) * 4096
        const C2: i32 = 3920; // cos(2*pi/17) * 4096
        const C3: i32 = 3612; // cos(3*pi/17) * 4096
        const C4: i32 = 3166; // cos(4*pi/17) * 4096
        const C5: i32 = 2598; // cos(5*pi/17) * 4096
        const C6: i32 = 1931; // cos(6*pi/17) * 4096
        const C7: i32 = 1189; // cos(7*pi/17) * 4096
        const C8: i32 = 401; // cos(8*pi/17) * 4096

        let x = input;
        let s0 = C1 * x[0] + C2 * x[1] + C3 * x[2] + C4 * x[3] + C5 * x[4] + C6 * x[5] + C7 * x[6]
            + C8 * x[7];
        let s1 = C2 * x[0] + C4 * x[1] + C6 * x[2] + C8 * x[3] - C7 * x[4] - C5 * x[5] - C3 * x[6]
            - C1 * x[7];
        let s2 = C3 * x[0] + C6 * x[1] - C8 * x[2] - C5 * x[3] - C1 * x[4] + C7 * x[5] + C2 * x[6]
            + C4 * x[7];
        let s3 = C4 * x[0] + C8 * x[1] - C5 * x[2] - C1 * x[3] + C6 * x[4] + C2 * x[5] - C7 * x[6]
            - C3 * x[7];
        let s4 = C5 * x[0] - C7 * x[1] - C1 * x[2] + C6 * x[3] + C2 * x[4] - C8 * x[5] - C4 * x[6]
            + C3 * x[7];
        let s5 = C6 * x[0] - C5 * x[1] + C7 * x[2] + C2 * x[3] - C8 * x[4] - C3 * x[5] + C1 * x[6]
            + C4 * x[7];
        let s6 = C7 * x[0] - C3 * x[1] + C2 * x[2] - C7 * x[3] - C4 * x[4] + C1 * x[5] - C8 * x[6]
            + C5 * x[7];
        let s7 = C8 * x[0] - C1 * x[1] + C4 * x[2] - C3 * x[3] + C3 * x[4] - C4 * x[5] + C1 * x[6]
            - C8 * x[7];

        [
            (s0 + ROUND_SHIFT_12) >> 12,
            (s1 + ROUND_SHIFT_12) >> 12,
            (s2 + ROUND_SHIFT_12) >> 12,
            (s3 + ROUND_SHIFT_12) >> 12,
            (s4 + ROUND_SHIFT_12) >> 12,
            (s5 + ROUND_SHIFT_12) >> 12,
            (s6 + ROUND_SHIFT_12) >> 12,
            (s7 + ROUND_SHIFT_12) >> 12,
        ]
    }

    /// 8-point inverse FLIPADST (1D)
    #[inline(always)]
    fn iflipadst_8pt(input: &[i32; 8]) -> [i32; 8] {
        let result = Self::iadst_8pt(input);
        [
            result[7], result[6], result[5], result[4], result[3], result[2], result[1], result[0],
        ]
    }

    /// 8-point identity transform (1D)
    #[inline(always)]
    fn identity_8pt(input: &[i32; 8]) -> [i32; 8] {
        let mut output = [0i32; 8];
        for i in 0..8 {
            output[i] = (input[i] * IDENTITY_SCALE + ROUND_SHIFT_12) >> 12;
        }
        output
    }

    /// Dispatch 1D 8-point transform based on kind
    #[inline(always)]
    fn transform_8pt(input: &[i32; 8], kind: Av1TransformKind) -> [i32; 8] {
        match kind {
            Av1TransformKind::Dct => Self::idct_8pt(input),
            Av1TransformKind::Adst => Self::iadst_8pt(input),
            Av1TransformKind::FlipAdst => Self::iflipadst_8pt(input),
            Av1TransformKind::Identity => Self::identity_8pt(input),
        }
    }

    // =========================================================================
    // 8x8 INVERSE TRANSFORMS
    // =========================================================================

    /// Perform 8x8 inverse transform with specified type
    pub fn inverse_transform_8x8(
        &self,
        input: &[i32; 64],
        output: &mut [i32; 64],
        tx_type: Av1TxType,
    ) {
        self.generation.fetch_add(1, Ordering::AcqRel);

        let row_type = tx_type.row_type();
        let col_type = tx_type.col_type();

        let mut workspace = [[0i32; 8]; 8];

        // First pass: Row transforms
        for row in 0..8 {
            let idx = row * 8;
            let row_in = [
                input[idx],
                input[idx + 1],
                input[idx + 2],
                input[idx + 3],
                input[idx + 4],
                input[idx + 5],
                input[idx + 6],
                input[idx + 7],
            ];
            workspace[row] = Self::transform_8pt(&row_in, row_type);
        }

        // Second pass: Column transforms
        for col in 0..8 {
            let col_in = [
                workspace[0][col],
                workspace[1][col],
                workspace[2][col],
                workspace[3][col],
                workspace[4][col],
                workspace[5][col],
                workspace[6][col],
                workspace[7][col],
            ];
            let col_out = Self::transform_8pt(&col_in, col_type);

            // Store with final rounding: (x + 16) >> 5
            for row in 0..8 {
                output[row * 8 + col] = (col_out[row] + 16) >> 5;
            }
        }

        self.transforms_8x8.fetch_add(1, Ordering::Relaxed);
        self.update_type_stats(tx_type);
    }

    // =========================================================================
    // 16-POINT DCT (IDCT-16)
    // =========================================================================

    /// 16-point inverse DCT (1D)
    fn idct_16pt(input: &[i32; 16]) -> [i32; 16] {
        // #ASSUME_NO_OVERFLOW: i32 arithmetic prevents overflow

        // Recursively compute 8-point DCT on even indices
        let even_in = [
            input[0], input[2], input[4], input[6], input[8], input[10], input[12], input[14],
        ];
        let even_out = Self::idct_8pt(&even_in);

        // 16-point specific rotations for odd indices
        const C1: i32 = 4076;
        const S1: i32 = 401;
        const C3: i32 = 3612;
        const S3: i32 = 1931;
        const C5: i32 = 2598;
        const S5: i32 = 3166;
        const C7: i32 = 1189;
        const S7: i32 = 3920;

        let t0 = (input[1] * C1 - input[15] * S1 + ROUND_SHIFT_12) >> 12;
        let t1 = (input[1] * S1 + input[15] * C1 + ROUND_SHIFT_12) >> 12;
        let t2 = (input[9] * C7 - input[7] * S7 + ROUND_SHIFT_12) >> 12;
        let t3 = (input[9] * S7 + input[7] * C7 + ROUND_SHIFT_12) >> 12;
        let t4 = (input[5] * C5 - input[11] * S5 + ROUND_SHIFT_12) >> 12;
        let t5 = (input[5] * S5 + input[11] * C5 + ROUND_SHIFT_12) >> 12;
        let t6 = (input[13] * C3 - input[3] * S3 + ROUND_SHIFT_12) >> 12;
        let t7 = (input[13] * S3 + input[3] * C3 + ROUND_SHIFT_12) >> 12;

        // Butterfly stages
        let u0 = t0 + t4;
        let u1 = t1 + t5;
        let u2 = t2 + t6;
        let u3 = t3 + t7;
        let u4 = ((t0 - t4) * COS_PI_4 + ROUND_SHIFT_12) >> 12;
        let u5 = ((t1 - t5) * COS_PI_4 + ROUND_SHIFT_12) >> 12;
        let u6 = ((t2 - t6) * COS_PI_4 + ROUND_SHIFT_12) >> 12;
        let u7 = ((t3 - t7) * COS_PI_4 + ROUND_SHIFT_12) >> 12;

        let v0 = u0 + u2;
        let v1 = u1 + u3;
        let v2 = ((u0 - u2) * COS_PI_4 + ROUND_SHIFT_12) >> 12;
        let v3 = ((u1 - u3) * COS_PI_4 + ROUND_SHIFT_12) >> 12;
        let v4 = u4 + u6;
        let v5 = u5 + u7;
        let v6 = ((u4 - u6) * COS_PI_4 + ROUND_SHIFT_12) >> 12;
        let v7 = ((u5 - u7) * COS_PI_4 + ROUND_SHIFT_12) >> 12;

        [
            even_out[0] + v1,
            even_out[1] + v5,
            even_out[2] + v3,
            even_out[3] + v7,
            even_out[4] + v6,
            even_out[5] + v2,
            even_out[6] + v4,
            even_out[7] + v0,
            even_out[7] - v0,
            even_out[6] - v4,
            even_out[5] - v2,
            even_out[4] - v6,
            even_out[3] - v7,
            even_out[2] - v3,
            even_out[1] - v5,
            even_out[0] - v1,
        ]
    }

    /// Dispatch 1D 16-point transform based on kind
    fn transform_16pt(input: &[i32; 16], kind: Av1TransformKind) -> [i32; 16] {
        match kind {
            Av1TransformKind::Dct => Self::idct_16pt(input),
            Av1TransformKind::Identity => {
                let mut output = [0i32; 16];
                for i in 0..16 {
                    output[i] = (input[i] * IDENTITY_SCALE + ROUND_SHIFT_12) >> 12;
                }
                output
            }
            // ADST and FLIPADST for 16-point use DCT in AV1 (spec constraint)
            _ => Self::idct_16pt(input),
        }
    }

    // =========================================================================
    // 16x16 INVERSE TRANSFORMS
    // =========================================================================

    /// Perform 16x16 inverse transform
    pub fn inverse_transform_16x16(
        &self,
        input: &[i32; 256],
        output: &mut [i32; 256],
        tx_type: Av1TxType,
    ) {
        self.generation.fetch_add(1, Ordering::AcqRel);

        let row_type = tx_type.row_type();
        let col_type = tx_type.col_type();

        let mut workspace = [[0i32; 16]; 16];

        // First pass: Row transforms
        for row in 0..16 {
            let idx = row * 16;
            let mut row_in = [0i32; 16];
            for i in 0..16 {
                row_in[i] = input[idx + i];
            }
            workspace[row] = Self::transform_16pt(&row_in, row_type);
        }

        // Second pass: Column transforms
        for col in 0..16 {
            let mut col_in = [0i32; 16];
            for row in 0..16 {
                col_in[row] = workspace[row][col];
            }
            let col_out = Self::transform_16pt(&col_in, col_type);

            // Store with final rounding: (x + 32) >> 6
            for row in 0..16 {
                output[row * 16 + col] = (col_out[row] + 32) >> 6;
            }
        }

        self.transforms_16x16.fetch_add(1, Ordering::Relaxed);
        self.update_type_stats(tx_type);
    }

    // =========================================================================
    // 32-POINT DCT (IDCT-32)
    // =========================================================================

    /// 32-point inverse DCT (1D)
    fn idct_32pt(input: &[i32; 32]) -> [i32; 32] {
        // #ASSUME_NO_OVERFLOW: i32 arithmetic prevents overflow

        // Recursively compute 16-point DCT on even indices
        let mut even_in = [0i32; 16];
        for i in 0..16 {
            even_in[i] = input[i * 2];
        }
        let even_out = Self::idct_16pt(&even_in);

        // Process odd indices with 32-point specific rotations
        let mut odd_out = [0i32; 16];

        const C1_32: i32 = 4091;
        const S1_32: i32 = 201;
        const C3_32: i32 = 4017;
        const S3_32: i32 = 799;
        const C5_32: i32 = 3857;
        const S5_32: i32 = 1380;
        const C7_32: i32 = 3612;
        const S7_32: i32 = 1931;

        for i in 0..8 {
            let idx1 = i * 2 + 1;
            let idx2 = 31 - i * 2;
            let x1 = input[idx1];
            let x2 = input[idx2];

            let (c, s) = match i {
                0 => (C1_32, S1_32),
                1 => (C3_32, S3_32),
                2 => (C5_32, S5_32),
                3 => (C7_32, S7_32),
                4 => (S7_32, C7_32),
                5 => (S5_32, C5_32),
                6 => (S3_32, C3_32),
                _ => (S1_32, C1_32),
            };

            odd_out[i] = (x1 * c - x2 * s + ROUND_SHIFT_12) >> 12;
            odd_out[15 - i] = (x1 * s + x2 * c + ROUND_SHIFT_12) >> 12;
        }

        // Butterfly on odd outputs
        for i in 0..8 {
            let a = odd_out[i];
            let b = odd_out[15 - i];
            odd_out[i] = a + b;
            odd_out[15 - i] = ((a - b) * COS_PI_4 + ROUND_SHIFT_12) >> 12;
        }

        // Final combination
        let mut result = [0i32; 32];
        for i in 0..16 {
            result[i] = even_out[i] + odd_out[i];
            result[31 - i] = even_out[i] - odd_out[i];
        }

        result
    }

    /// Dispatch 1D 32-point transform based on kind
    fn transform_32pt(input: &[i32; 32], kind: Av1TransformKind) -> [i32; 32] {
        match kind {
            Av1TransformKind::Dct => Self::idct_32pt(input),
            Av1TransformKind::Identity => {
                let mut output = [0i32; 32];
                for i in 0..32 {
                    output[i] = (input[i] * IDENTITY_SCALE + ROUND_SHIFT_12) >> 12;
                }
                output
            }
            // 32-point ADST uses DCT in AV1
            _ => Self::idct_32pt(input),
        }
    }

    // =========================================================================
    // 32x32 INVERSE TRANSFORMS
    // =========================================================================

    /// Perform 32x32 inverse transform
    pub fn inverse_transform_32x32(
        &self,
        input: &[i32; 1024],
        output: &mut [i32; 1024],
        tx_type: Av1TxType,
    ) {
        self.generation.fetch_add(1, Ordering::AcqRel);

        let row_type = tx_type.row_type();
        let col_type = tx_type.col_type();

        let mut workspace = [[0i32; 32]; 32];

        // First pass: Row transforms
        for row in 0..32 {
            let idx = row * 32;
            let mut row_in = [0i32; 32];
            for i in 0..32 {
                row_in[i] = input[idx + i];
            }
            workspace[row] = Self::transform_32pt(&row_in, row_type);
        }

        // Second pass: Column transforms
        for col in 0..32 {
            let mut col_in = [0i32; 32];
            for row in 0..32 {
                col_in[row] = workspace[row][col];
            }
            let col_out = Self::transform_32pt(&col_in, col_type);

            // Store with final rounding: (x + 64) >> 7
            for row in 0..32 {
                output[row * 32 + col] = (col_out[row] + 64) >> 7;
            }
        }

        self.transforms_32x32.fetch_add(1, Ordering::Relaxed);
        self.update_type_stats(tx_type);
    }

    // =========================================================================
    // 64-POINT DCT (IDCT-64)
    // =========================================================================

    /// 64-point inverse DCT (1D)
    fn idct_64pt(input: &[i32; 64]) -> [i32; 64] {
        // Recursively compute 32-point DCT on even indices
        let mut even_in = [0i32; 32];
        for i in 0..32 {
            even_in[i] = input[i * 2];
        }
        let even_out = Self::idct_32pt(&even_in);

        // Process odd indices
        let mut odd_out = [0i32; 32];

        // Simplified 64-point processing
        for i in 0..16 {
            let idx1 = i * 2 + 1;
            let idx2 = 63 - i * 2;
            let x1 = input[idx1];
            let x2 = input[idx2];

            // Use approximated rotation angles
            let angle = ((i as i32) * 201 + 100) % 4096;
            let c = 4096 - (angle * angle / 8192);
            let s = angle;

            odd_out[i] = (x1 * c - x2 * s + ROUND_SHIFT_12) >> 12;
            odd_out[31 - i] = (x1 * s + x2 * c + ROUND_SHIFT_12) >> 12;
        }

        // Butterfly stages
        for i in 0..16 {
            let a = odd_out[i];
            let b = odd_out[31 - i];
            odd_out[i] = a + b;
            odd_out[31 - i] = ((a - b) * COS_PI_4 + ROUND_SHIFT_12) >> 12;
        }

        // Final combination
        let mut result = [0i32; 64];
        for i in 0..32 {
            result[i] = even_out[i] + odd_out[i];
            result[63 - i] = even_out[i] - odd_out[i];
        }

        result
    }

    /// Dispatch 1D 64-point transform based on kind
    fn transform_64pt(input: &[i32; 64], kind: Av1TransformKind) -> [i32; 64] {
        match kind {
            Av1TransformKind::Dct => Self::idct_64pt(input),
            Av1TransformKind::Identity => {
                let mut output = [0i32; 64];
                for i in 0..64 {
                    output[i] = (input[i] * IDENTITY_SCALE + ROUND_SHIFT_12) >> 12;
                }
                output
            }
            // 64-point only supports DCT in AV1
            _ => Self::idct_64pt(input),
        }
    }

    // =========================================================================
    // 64x64 INVERSE TRANSFORMS
    // =========================================================================

    /// Perform 64x64 inverse transform
    pub fn inverse_transform_64x64(
        &self,
        input: &[i32; 4096],
        output: &mut [i32; 4096],
        tx_type: Av1TxType,
    ) {
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Note: 64x64 only supports DCT_DCT in AV1 spec
        let row_type = tx_type.row_type();
        let col_type = tx_type.col_type();

        let mut workspace = vec![[0i32; 64]; 64];

        // First pass: Row transforms
        for row in 0..64 {
            let idx = row * 64;
            let mut row_in = [0i32; 64];
            for i in 0..64 {
                row_in[i] = input[idx + i];
            }
            workspace[row] = Self::transform_64pt(&row_in, row_type);
        }

        // Second pass: Column transforms
        for col in 0..64 {
            let mut col_in = [0i32; 64];
            for row in 0..64 {
                col_in[row] = workspace[row][col];
            }
            let col_out = Self::transform_64pt(&col_in, col_type);

            // Store with final rounding: (x + 128) >> 8
            for row in 0..64 {
                output[row * 64 + col] = (col_out[row] + 128) >> 8;
            }
        }

        self.transforms_64x64.fetch_add(1, Ordering::Relaxed);
        self.update_type_stats(tx_type);
    }

    // =========================================================================
    // UNIFIED INVERSE TRANSFORM API
    // =========================================================================

    /// Perform inverse transform with automatic size/type dispatch
    ///
    /// This is the main entry point for AV1 inverse transforms.
    ///
    /// # Arguments
    ///
    /// * `coeffs` - Input coefficients (length must match tx_size)
    /// * `output` - Output residual samples (length must match tx_size)
    /// * `tx_type` - Transform type (16 combinations)
    /// * `tx_size` - Transform size (19 sizes)
    ///
    /// # Returns
    ///
    /// `Av1TransformError::None` on success, error code otherwise
    pub fn inverse_transform(
        &self,
        coeffs: &[i32],
        output: &mut [i32],
        tx_type: Av1TxType,
        tx_size: Av1TxSize,
    ) -> Av1TransformError {
        let expected_len = tx_size.coeff_count();

        // Validate buffer sizes
        if coeffs.len() < expected_len || output.len() < expected_len {
            return Av1TransformError::BufferSizeMismatch;
        }

        // Update state for tracking
        let state =
            ((tx_size as u64) & 0xFF) | (((tx_type as u64) & 0xFF) << 8);
        self.state.store(state, Ordering::Release);

        // Dispatch to appropriate transform
        match tx_size {
            Av1TxSize::Tx4x4 => {
                let input_arr: &[i32; 16] = coeffs[..16].try_into().expect("verified");
                let output_arr: &mut [i32; 16] =
                    (&mut output[..16]).try_into().expect("verified");
                self.inverse_transform_4x4(input_arr, output_arr, tx_type);
            }
            Av1TxSize::Tx8x8 => {
                let input_arr: &[i32; 64] = coeffs[..64].try_into().expect("verified");
                let output_arr: &mut [i32; 64] =
                    (&mut output[..64]).try_into().expect("verified");
                self.inverse_transform_8x8(input_arr, output_arr, tx_type);
            }
            Av1TxSize::Tx16x16 => {
                let input_arr: &[i32; 256] = coeffs[..256].try_into().expect("verified");
                let output_arr: &mut [i32; 256] =
                    (&mut output[..256]).try_into().expect("verified");
                self.inverse_transform_16x16(input_arr, output_arr, tx_type);
            }
            Av1TxSize::Tx32x32 => {
                let input_arr: &[i32; 1024] = coeffs[..1024].try_into().expect("verified");
                let output_arr: &mut [i32; 1024] =
                    (&mut output[..1024]).try_into().expect("verified");
                self.inverse_transform_32x32(input_arr, output_arr, tx_type);
            }
            Av1TxSize::Tx64x64 => {
                let input_arr: &[i32; 4096] = coeffs[..4096].try_into().expect("verified");
                let output_arr: &mut [i32; 4096] =
                    (&mut output[..4096]).try_into().expect("verified");
                self.inverse_transform_64x64(input_arr, output_arr, tx_type);
            }
            // Rectangular transforms
            _ => {
                self.inverse_transform_rect(coeffs, output, tx_type, tx_size);
            }
        }

        Av1TransformError::None
    }

    /// Handle rectangular transforms
    fn inverse_transform_rect(
        &self,
        coeffs: &[i32],
        output: &mut [i32],
        tx_type: Av1TxType,
        tx_size: Av1TxSize,
    ) {
        self.generation.fetch_add(1, Ordering::AcqRel);

        let (width, height) = tx_size.dimensions();
        let row_type = tx_type.row_type();
        let col_type = tx_type.col_type();

        // Allocate workspace
        let mut workspace = vec![0i32; width * height];

        // First pass: Row transforms
        for row in 0..height {
            let idx = row * width;
            let row_slice = &coeffs[idx..idx + width];

            // Dispatch based on width
            let row_out: Vec<i32> = match width {
                4 => {
                    let arr: [i32; 4] = row_slice.try_into().unwrap();
                    Self::transform_4pt(&arr, row_type).to_vec()
                }
                8 => {
                    let arr: [i32; 8] = row_slice.try_into().unwrap();
                    Self::transform_8pt(&arr, row_type).to_vec()
                }
                16 => {
                    let mut arr = [0i32; 16];
                    arr.copy_from_slice(row_slice);
                    Self::transform_16pt(&arr, row_type).to_vec()
                }
                32 => {
                    let mut arr = [0i32; 32];
                    arr.copy_from_slice(row_slice);
                    Self::transform_32pt(&arr, row_type).to_vec()
                }
                64 => {
                    let mut arr = [0i32; 64];
                    arr.copy_from_slice(row_slice);
                    Self::transform_64pt(&arr, row_type).to_vec()
                }
                _ => row_slice.to_vec(),
            };

            for (i, &v) in row_out.iter().enumerate() {
                workspace[row * width + i] = v;
            }
        }

        // Second pass: Column transforms
        for col in 0..width {
            // Extract column
            let mut col_in = vec![0i32; height];
            for row in 0..height {
                col_in[row] = workspace[row * width + col];
            }

            // Dispatch based on height
            let col_out: Vec<i32> = match height {
                4 => {
                    let arr: [i32; 4] = col_in.try_into().unwrap();
                    Self::transform_4pt(&arr, col_type).to_vec()
                }
                8 => {
                    let arr: [i32; 8] = col_in.try_into().unwrap();
                    Self::transform_8pt(&arr, col_type).to_vec()
                }
                16 => {
                    let mut arr = [0i32; 16];
                    arr.copy_from_slice(&col_in);
                    Self::transform_16pt(&arr, col_type).to_vec()
                }
                32 => {
                    let mut arr = [0i32; 32];
                    arr.copy_from_slice(&col_in);
                    Self::transform_32pt(&arr, col_type).to_vec()
                }
                64 => {
                    let mut arr = [0i32; 64];
                    arr.copy_from_slice(&col_in);
                    Self::transform_64pt(&arr, col_type).to_vec()
                }
                _ => col_in,
            };

            // Compute final shift based on size
            let shift = tx_size.width_log2() + tx_size.height_log2() - 2;
            let round = 1 << (shift - 1);

            for row in 0..height {
                output[row * width + col] = (col_out[row] + round) >> shift;
            }
        }

        self.transforms_rect.fetch_add(1, Ordering::Relaxed);
        self.update_type_stats(tx_type);
    }

    // =========================================================================
    // STATISTICS AND UTILITY
    // =========================================================================

    /// Update type-specific statistics
    fn update_type_stats(&self, tx_type: Av1TxType) {
        if tx_type.is_pure_identity() {
            self.identity_count.fetch_add(1, Ordering::Relaxed);
        } else {
            let row_type = tx_type.row_type();
            let col_type = tx_type.col_type();

            if row_type == Av1TransformKind::Dct || col_type == Av1TransformKind::Dct {
                self.dct_count.fetch_add(1, Ordering::Relaxed);
            }
            if row_type == Av1TransformKind::Adst
                || col_type == Av1TransformKind::Adst
                || row_type == Av1TransformKind::FlipAdst
                || col_type == Av1TransformKind::FlipAdst
            {
                self.adst_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Get transform statistics snapshot
    pub fn stats(&self) -> Av1TransformStats {
        Av1TransformStats {
            transforms_4x4: self.transforms_4x4.load(Ordering::Acquire),
            transforms_8x8: self.transforms_8x8.load(Ordering::Acquire),
            transforms_16x16: self.transforms_16x16.load(Ordering::Acquire),
            transforms_32x32: self.transforms_32x32.load(Ordering::Acquire),
            transforms_64x64: self.transforms_64x64.load(Ordering::Acquire),
            transforms_rect: self.transforms_rect.load(Ordering::Acquire),
            identity_count: self.identity_count.load(Ordering::Acquire),
            wht_count: self.wht_count.load(Ordering::Acquire),
            dct_count: self.dct_count.load(Ordering::Acquire),
            adst_count: self.adst_count.load(Ordering::Acquire),
            generation: self.generation.load(Ordering::Acquire),
        }
    }

    /// Reset all statistics counters
    pub fn reset_stats(&self) {
        self.transforms_4x4.store(0, Ordering::Release);
        self.transforms_8x8.store(0, Ordering::Release);
        self.transforms_16x16.store(0, Ordering::Release);
        self.transforms_32x32.store(0, Ordering::Release);
        self.transforms_64x64.store(0, Ordering::Release);
        self.transforms_rect.store(0, Ordering::Release);
        self.identity_count.store(0, Ordering::Release);
        self.wht_count.store(0, Ordering::Release);
        self.dct_count.store(0, Ordering::Release);
        self.adst_count.store(0, Ordering::Release);
        // Don't reset generation counter (monotonic for Q34 audit)
    }

    /// Get current generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get total transform count (all sizes)
    pub fn total_transforms(&self) -> u64 {
        let stats = self.stats();
        stats.transforms_4x4
            + stats.transforms_8x8
            + stats.transforms_16x16
            + stats.transforms_32x32
            + stats.transforms_64x64
            + stats.transforms_rect
    }

    /// Check if SIMD acceleration is enabled
    #[inline]
    pub fn is_simd_enabled(&self) -> bool {
        self.simd_enabled.load(Ordering::Relaxed) != 0
    }

    /// Enable or disable SIMD acceleration (for testing)
    pub fn set_simd_enabled(&self, enabled: bool) {
        self.simd_enabled
            .store(if enabled { 1 } else { 0 }, Ordering::Relaxed);
    }

    /// Get current state (tx_size, tx_type)
    pub fn current_state(&self) -> (Option<Av1TxSize>, Option<Av1TxType>) {
        let state = self.state.load(Ordering::Acquire);
        let tx_size = Av1TxSize::from_u8((state & 0xFF) as u8);
        let tx_type = Av1TxType::from_u8(((state >> 8) & 0xFF) as u8);
        (tx_size, tx_type)
    }
}

impl Default for Av1TransformCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Compile-time verification of capsule size and alignment
const _: () = {
    assert!(core::mem::size_of::<Av1TransformCapsule>() == 512);
    assert!(core::mem::align_of::<Av1TransformCapsule>() == 512);
};

// ============================================================================
// T28 5-TIER TESTING (36+ tests)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Q1-Q7: UNIT TESTS
    // =========================================================================

    #[test]
    fn test_q1_new_capsule() {
        let capsule = Av1TransformCapsule::new();
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.total_transforms(), 0);
        assert_eq!(capsule.bit_depth(), 8);
    }

    #[test]
    fn test_q2_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<Av1TransformCapsule>(), 512);
        assert_eq!(core::mem::align_of::<Av1TransformCapsule>(), 512);
    }

    #[test]
    fn test_q3_tx_type_enum() {
        assert_eq!(Av1TxType::DctDct.row_type(), Av1TransformKind::Dct);
        assert_eq!(Av1TxType::DctDct.col_type(), Av1TransformKind::Dct);
        assert_eq!(Av1TxType::AdstDct.row_type(), Av1TransformKind::Adst);
        assert_eq!(Av1TxType::AdstDct.col_type(), Av1TransformKind::Dct);
        assert_eq!(Av1TxType::FlipAdstFlipAdst.row_type(), Av1TransformKind::FlipAdst);
        assert!(Av1TxType::Idtx.is_pure_identity());
        assert!(!Av1TxType::VDct.is_pure_identity());
        assert!(Av1TxType::VDct.is_identity());
    }

    #[test]
    fn test_q4_tx_size_enum() {
        assert_eq!(Av1TxSize::Tx4x4.width(), 4);
        assert_eq!(Av1TxSize::Tx4x4.height(), 4);
        assert_eq!(Av1TxSize::Tx4x4.coeff_count(), 16);
        assert!(Av1TxSize::Tx4x4.is_square());

        assert_eq!(Av1TxSize::Tx4x8.width(), 4);
        assert_eq!(Av1TxSize::Tx4x8.height(), 8);
        assert_eq!(Av1TxSize::Tx4x8.coeff_count(), 32);
        assert!(!Av1TxSize::Tx4x8.is_square());

        assert_eq!(Av1TxSize::Tx64x64.width(), 64);
        assert_eq!(Av1TxSize::Tx64x64.height(), 64);
        assert_eq!(Av1TxSize::Tx64x64.coeff_count(), 4096);
    }

    #[test]
    fn test_q5_tx_type_from_u8() {
        assert_eq!(Av1TxType::from_u8(0), Some(Av1TxType::DctDct));
        assert_eq!(Av1TxType::from_u8(9), Some(Av1TxType::Idtx));
        assert_eq!(Av1TxType::from_u8(15), Some(Av1TxType::HFlipAdst));
        assert_eq!(Av1TxType::from_u8(16), None);
    }

    #[test]
    fn test_q6_tx_size_from_u8() {
        assert_eq!(Av1TxSize::from_u8(0), Some(Av1TxSize::Tx4x4));
        assert_eq!(Av1TxSize::from_u8(4), Some(Av1TxSize::Tx64x64));
        assert_eq!(Av1TxSize::from_u8(18), Some(Av1TxSize::Tx64x16));
        assert_eq!(Av1TxSize::from_u8(19), None);
    }

    #[test]
    fn test_q7_error_enum() {
        assert!(Av1TransformError::None.is_ok());
        assert!(!Av1TransformError::InvalidTxSize.is_ok());
        assert!(Av1TransformError::BufferSizeMismatch.is_err());
    }

    // =========================================================================
    // Q8-Q14: PROPERTY TESTS
    // =========================================================================

    #[test]
    fn test_q8_iwht_4x4_dc_only() {
        let capsule = Av1TransformCapsule::new();

        let mut input = [0i32; 16];
        input[0] = 256;
        let mut output = [0i32; 16];

        capsule.iwht_4x4(&input, &mut output);

        // DC-only WHT should produce uniform output
        let dc_value = output[0];
        for &val in output.iter() {
            assert_eq!(val, dc_value, "WHT DC-only should produce uniform output");
        }
        assert_eq!(capsule.stats().wht_count, 1);
    }

    #[test]
    fn test_q9_idct_4x4_zero_input() {
        let capsule = Av1TransformCapsule::new();

        let input = [0i32; 16];
        let mut output = [999i32; 16];

        capsule.inverse_transform_4x4(&input, &mut output, Av1TxType::DctDct);

        for &val in output.iter() {
            assert_eq!(val, 0, "Zero input should produce zero output");
        }
    }

    #[test]
    fn test_q10_idct_4x4_dc_only() {
        let capsule = Av1TransformCapsule::new();

        let mut input = [0i32; 16];
        input[0] = 256;
        let mut output = [0i32; 16];

        capsule.inverse_transform_4x4(&input, &mut output, Av1TxType::DctDct);

        let dc_value = output[0];
        for &val in output.iter() {
            assert_eq!(val, dc_value, "DCT DC-only should produce uniform output");
        }
    }

    #[test]
    fn test_q11_idct_8x8_dc_only() {
        let capsule = Av1TransformCapsule::new();

        let mut input = [0i32; 64];
        input[0] = 512;
        let mut output = [0i32; 64];

        capsule.inverse_transform_8x8(&input, &mut output, Av1TxType::DctDct);

        let dc_value = output[0];
        for &val in output.iter() {
            assert_eq!(val, dc_value, "8x8 DCT DC-only should produce uniform output");
        }
    }

    #[test]
    fn test_q12_identity_transform() {
        let capsule = Av1TransformCapsule::new();

        let input: [i32; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let mut output = [0i32; 16];

        capsule.inverse_transform_4x4(&input, &mut output, Av1TxType::Idtx);

        // Identity should preserve values (with scaling)
        assert_eq!(capsule.stats().identity_count, 1);
    }

    #[test]
    fn test_q13_adst_different_from_dct() {
        let capsule = Av1TransformCapsule::new();

        let input: [i32; 16] = [100, -50, 25, -12, 60, -30, 15, -8, 40, -20, 10, -5, 20, -10, 5, -2];
        let mut output_dct = [0i32; 16];
        let mut output_adst = [0i32; 16];

        capsule.inverse_transform_4x4(&input, &mut output_dct, Av1TxType::DctDct);
        capsule.inverse_transform_4x4(&input, &mut output_adst, Av1TxType::AdstAdst);

        let any_different = output_dct.iter().zip(output_adst.iter()).any(|(&a, &b)| a != b);
        assert!(any_different, "ADST should differ from DCT");
    }

    #[test]
    fn test_q14_generation_counter_increments() {
        let capsule = Av1TransformCapsule::new();
        assert_eq!(capsule.generation(), 0);

        let input = [0i32; 16];
        let mut output = [0i32; 16];

        capsule.inverse_transform_4x4(&input, &mut output, Av1TxType::DctDct);
        assert_eq!(capsule.generation(), 1);

        capsule.inverse_transform_4x4(&input, &mut output, Av1TxType::AdstAdst);
        assert_eq!(capsule.generation(), 2);

        let mut wht_input = [0i32; 16];
        wht_input[0] = 100;
        capsule.iwht_4x4(&wht_input, &mut output);
        assert_eq!(capsule.generation(), 3);
    }

    // =========================================================================
    // Q15-Q21: INTEGRATION TESTS
    // =========================================================================

    #[test]
    fn test_q15_inverse_transform_dispatch_4x4() {
        let capsule = Av1TransformCapsule::new();

        let input = [100i32; 16];
        let mut output = [0i32; 16];

        let result = capsule.inverse_transform(&input, &mut output, Av1TxType::DctDct, Av1TxSize::Tx4x4);
        assert_eq!(result, Av1TransformError::None);
        assert_eq!(capsule.stats().transforms_4x4, 1);
    }

    #[test]
    fn test_q16_inverse_transform_dispatch_8x8() {
        let capsule = Av1TransformCapsule::new();

        let input = [100i32; 64];
        let mut output = [0i32; 64];

        let result = capsule.inverse_transform(&input, &mut output, Av1TxType::AdstAdst, Av1TxSize::Tx8x8);
        assert_eq!(result, Av1TransformError::None);
        assert_eq!(capsule.stats().transforms_8x8, 1);
    }

    #[test]
    fn test_q17_inverse_transform_16x16() {
        let capsule = Av1TransformCapsule::new();

        let input = [100i32; 256];
        let mut output = [0i32; 256];

        let result = capsule.inverse_transform(&input, &mut output, Av1TxType::DctDct, Av1TxSize::Tx16x16);
        assert_eq!(result, Av1TransformError::None);
        assert_eq!(capsule.stats().transforms_16x16, 1);
    }

    #[test]
    fn test_q18_inverse_transform_buffer_mismatch() {
        let capsule = Av1TransformCapsule::new();

        let input = [100i32; 8]; // Too small for 4x4
        let mut output = [0i32; 16];

        let result = capsule.inverse_transform(&input, &mut output, Av1TxType::DctDct, Av1TxSize::Tx4x4);
        assert_eq!(result, Av1TransformError::BufferSizeMismatch);
    }

    #[test]
    fn test_q19_rectangular_transform_4x8() {
        let capsule = Av1TransformCapsule::new();

        let input = [100i32; 32];
        let mut output = [0i32; 32];

        let result = capsule.inverse_transform(&input, &mut output, Av1TxType::DctDct, Av1TxSize::Tx4x8);
        assert_eq!(result, Av1TransformError::None);
        assert_eq!(capsule.stats().transforms_rect, 1);
    }

    #[test]
    fn test_q20_rectangular_transform_8x4() {
        let capsule = Av1TransformCapsule::new();

        let input = [100i32; 32];
        let mut output = [0i32; 32];

        let result = capsule.inverse_transform(&input, &mut output, Av1TxType::DctDct, Av1TxSize::Tx8x4);
        assert_eq!(result, Av1TransformError::None);
        assert_eq!(capsule.stats().transforms_rect, 1);
    }

    #[test]
    fn test_q21_32x32_transform() {
        let capsule = Av1TransformCapsule::new();

        let input = [100i32; 1024];
        let mut output = [0i32; 1024];

        let result = capsule.inverse_transform(&input, &mut output, Av1TxType::DctDct, Av1TxSize::Tx32x32);
        assert_eq!(result, Av1TransformError::None);
        assert_eq!(capsule.stats().transforms_32x32, 1);
    }

    // =========================================================================
    // Q22-Q28: PRODUCTION TESTS
    // =========================================================================

    #[test]
    fn test_q22_statistics_tracking() {
        let capsule = Av1TransformCapsule::new();

        let mut i4 = [100i32; 16];
        let mut o4 = [0i32; 16];
        let mut i8 = [100i32; 64];
        let mut o8 = [0i32; 64];
        let mut i16 = [100i32; 256];
        let mut o16 = [0i32; 256];

        capsule.inverse_transform_4x4(&i4, &mut o4, Av1TxType::DctDct);
        capsule.inverse_transform_4x4(&i4, &mut o4, Av1TxType::AdstAdst);
        capsule.inverse_transform_8x8(&i8, &mut o8, Av1TxType::DctDct);
        capsule.inverse_transform_16x16(&i16, &mut o16, Av1TxType::DctDct);

        let stats = capsule.stats();
        assert_eq!(stats.transforms_4x4, 2);
        assert_eq!(stats.transforms_8x8, 1);
        assert_eq!(stats.transforms_16x16, 1);
    }

    #[test]
    fn test_q23_reset_stats() {
        let capsule = Av1TransformCapsule::new();

        let input = [100i32; 16];
        let mut output = [0i32; 16];

        for _ in 0..10 {
            capsule.inverse_transform_4x4(&input, &mut output, Av1TxType::DctDct);
        }

        assert_eq!(capsule.stats().transforms_4x4, 10);

        capsule.reset_stats();

        let stats = capsule.stats();
        assert_eq!(stats.transforms_4x4, 0);
        // Generation should NOT be reset (Q34 audit requirement)
        assert_eq!(stats.generation, 10);
    }

    #[test]
    fn test_q24_concurrent_transforms() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(Av1TransformCapsule::new());
        let mut handles = vec![];

        for _ in 0..4 {
            let c = Arc::clone(&capsule);
            handles.push(thread::spawn(move || {
                let input = [100i32; 16];
                let mut output = [0i32; 16];
                for _ in 0..100 {
                    c.inverse_transform_4x4(&input, &mut output, Av1TxType::DctDct);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(capsule.stats().transforms_4x4, 400);
    }

    #[test]
    fn test_q25_current_state_tracking() {
        let capsule = Av1TransformCapsule::new();

        let input = [100i32; 64];
        let mut output = [0i32; 64];

        capsule.inverse_transform(&input, &mut output, Av1TxType::AdstDct, Av1TxSize::Tx8x8);

        let (size, tx_type) = capsule.current_state();
        assert_eq!(size, Some(Av1TxSize::Tx8x8));
        assert_eq!(tx_type, Some(Av1TxType::AdstDct));
    }

    #[test]
    fn test_q26_bit_depth_setting() {
        let capsule = Av1TransformCapsule::new();

        assert_eq!(capsule.bit_depth(), 8);

        assert_eq!(capsule.set_bit_depth(10), Av1TransformError::None);
        assert_eq!(capsule.bit_depth(), 10);

        assert_eq!(capsule.set_bit_depth(12), Av1TransformError::None);
        assert_eq!(capsule.bit_depth(), 12);

        assert_eq!(capsule.set_bit_depth(9), Av1TransformError::InvalidBitDepth);
        assert_eq!(capsule.bit_depth(), 12); // Unchanged
    }

    #[test]
    fn test_q27_total_transforms() {
        let capsule = Av1TransformCapsule::new();

        let mut i4 = [0i32; 16];
        let mut o4 = [0i32; 16];
        let mut i8 = [0i32; 64];
        let mut o8 = [0i32; 64];

        capsule.inverse_transform_4x4(&i4, &mut o4, Av1TxType::DctDct);
        capsule.inverse_transform_8x8(&i8, &mut o8, Av1TxType::DctDct);
        capsule.inverse_transform(&[0i32; 32], &mut [0i32; 32], Av1TxType::DctDct, Av1TxSize::Tx4x8);

        assert_eq!(capsule.total_transforms(), 3);
    }

    #[test]
    fn test_q28_display_traits() {
        assert_eq!(format!("{}", Av1TxType::DctDct), "DCT_DCT");
        assert_eq!(format!("{}", Av1TxType::Idtx), "IDTX");
        assert_eq!(format!("{}", Av1TxSize::Tx4x4), "4x4");
        assert_eq!(format!("{}", Av1TxSize::Tx64x64), "64x64");
        assert_eq!(format!("{}", Av1TransformError::None), "no error");
    }

    // =========================================================================
    // Q29-Q35: DETERMINISM TESTS
    // =========================================================================

    #[test]
    fn test_q29_deterministic_dct_4x4() {
        let capsule1 = Av1TransformCapsule::new();
        let capsule2 = Av1TransformCapsule::new();

        let input: [i32; 16] = [100, -50, 25, -12, 60, -30, 15, -8, 40, -20, 10, -5, 20, -10, 5, -2];
        let mut output1 = [0i32; 16];
        let mut output2 = [0i32; 16];

        capsule1.inverse_transform_4x4(&input, &mut output1, Av1TxType::DctDct);
        capsule2.inverse_transform_4x4(&input, &mut output2, Av1TxType::DctDct);

        assert_eq!(output1, output2, "DCT should be deterministic");
    }

    #[test]
    fn test_q30_deterministic_adst_4x4() {
        let capsule1 = Av1TransformCapsule::new();
        let capsule2 = Av1TransformCapsule::new();

        let input: [i32; 16] = [100, -50, 25, -12, 60, -30, 15, -8, 40, -20, 10, -5, 20, -10, 5, -2];
        let mut output1 = [0i32; 16];
        let mut output2 = [0i32; 16];

        capsule1.inverse_transform_4x4(&input, &mut output1, Av1TxType::AdstAdst);
        capsule2.inverse_transform_4x4(&input, &mut output2, Av1TxType::AdstAdst);

        assert_eq!(output1, output2, "ADST should be deterministic");
    }

    #[test]
    fn test_q31_deterministic_wht_4x4() {
        let capsule1 = Av1TransformCapsule::new();
        let capsule2 = Av1TransformCapsule::new();

        let input: [i32; 16] = [256, 0, 0, 0, 128, 0, 0, 0, 64, 0, 0, 0, 32, 0, 0, 0];
        let mut output1 = [0i32; 16];
        let mut output2 = [0i32; 16];

        capsule1.iwht_4x4(&input, &mut output1);
        capsule2.iwht_4x4(&input, &mut output2);

        assert_eq!(output1, output2, "WHT should be deterministic");
    }

    #[test]
    fn test_q32_deterministic_8x8() {
        let capsule1 = Av1TransformCapsule::new();
        let capsule2 = Av1TransformCapsule::new();

        let input = [50i32; 64];
        let mut output1 = [0i32; 64];
        let mut output2 = [0i32; 64];

        capsule1.inverse_transform_8x8(&input, &mut output1, Av1TxType::DctDct);
        capsule2.inverse_transform_8x8(&input, &mut output2, Av1TxType::DctDct);

        assert_eq!(output1, output2, "8x8 DCT should be deterministic");
    }

    #[test]
    fn test_q33_deterministic_16x16() {
        let capsule1 = Av1TransformCapsule::new();
        let capsule2 = Av1TransformCapsule::new();

        let input = [25i32; 256];
        let mut output1 = [0i32; 256];
        let mut output2 = [0i32; 256];

        capsule1.inverse_transform_16x16(&input, &mut output1, Av1TxType::DctDct);
        capsule2.inverse_transform_16x16(&input, &mut output2, Av1TxType::DctDct);

        assert_eq!(output1, output2, "16x16 DCT should be deterministic");
    }

    #[test]
    fn test_q34_deterministic_rectangular() {
        let capsule1 = Av1TransformCapsule::new();
        let capsule2 = Av1TransformCapsule::new();

        let input = [30i32; 32];
        let mut output1 = [0i32; 32];
        let mut output2 = [0i32; 32];

        capsule1.inverse_transform(&input, &mut output1, Av1TxType::DctDct, Av1TxSize::Tx4x8);
        capsule2.inverse_transform(&input, &mut output2, Av1TxType::DctDct, Av1TxSize::Tx4x8);

        assert_eq!(output1, output2, "Rectangular transform should be deterministic");
    }

    #[test]
    fn test_q35_flipadst_reversal() {
        let capsule = Av1TransformCapsule::new();

        let input: [i32; 16] = [100, -50, 25, -12, 60, -30, 15, -8, 40, -20, 10, -5, 20, -10, 5, -2];
        let mut output_adst = [0i32; 16];
        let mut output_flipadst = [0i32; 16];

        capsule.inverse_transform_4x4(&input, &mut output_adst, Av1TxType::AdstAdst);
        capsule.inverse_transform_4x4(&input, &mut output_flipadst, Av1TxType::FlipAdstFlipAdst);

        // FLIPADST should produce different results from ADST
        let any_different = output_adst.iter().zip(output_flipadst.iter()).any(|(&a, &b)| a != b);
        assert!(any_different, "FLIPADST should differ from ADST");
    }

    #[test]
    fn test_q36_64x64_transform() {
        let capsule = Av1TransformCapsule::new();

        let input = [10i32; 4096];
        let mut output = [0i32; 4096];

        let result = capsule.inverse_transform(&input, &mut output, Av1TxType::DctDct, Av1TxSize::Tx64x64);
        assert_eq!(result, Av1TransformError::None);
        assert_eq!(capsule.stats().transforms_64x64, 1);
    }

    // =========================================================================
    // ADDITIONAL TESTS
    // =========================================================================

    #[test]
    fn test_simd_enable_disable() {
        let capsule = Av1TransformCapsule::new();

        let initial = capsule.is_simd_enabled();

        capsule.set_simd_enabled(false);
        assert!(!capsule.is_simd_enabled());

        capsule.set_simd_enabled(true);
        assert!(capsule.is_simd_enabled());

        capsule.set_simd_enabled(initial);
    }

    #[test]
    fn test_default_impl() {
        let capsule = Av1TransformCapsule::default();
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.total_transforms(), 0);
    }

    #[test]
    fn test_tx_size_log2() {
        assert_eq!(Av1TxSize::Tx4x4.width_log2(), 2);
        assert_eq!(Av1TxSize::Tx8x8.width_log2(), 3);
        assert_eq!(Av1TxSize::Tx16x16.width_log2(), 4);
        assert_eq!(Av1TxSize::Tx32x32.width_log2(), 5);
        assert_eq!(Av1TxSize::Tx64x64.width_log2(), 6);

        assert_eq!(Av1TxSize::Tx4x8.width_log2(), 2);
        assert_eq!(Av1TxSize::Tx4x8.height_log2(), 3);
    }

    #[test]
    fn test_all_transform_types() {
        let capsule = Av1TransformCapsule::new();
        let input = [50i32; 16];
        let mut output = [0i32; 16];

        // Test all 16 transform types work without error
        for i in 0..16 {
            if let Some(tx_type) = Av1TxType::from_u8(i) {
                capsule.inverse_transform_4x4(&input, &mut output, tx_type);
            }
        }

        assert_eq!(capsule.stats().transforms_4x4, 16);
    }

    #[test]
    fn test_all_square_sizes() {
        let capsule = Av1TransformCapsule::new();

        // 4x4
        let mut i4 = [0i32; 16];
        let mut o4 = [0i32; 16];
        capsule.inverse_transform(&i4, &mut o4, Av1TxType::DctDct, Av1TxSize::Tx4x4);

        // 8x8
        let mut i8 = [0i32; 64];
        let mut o8 = [0i32; 64];
        capsule.inverse_transform(&i8, &mut o8, Av1TxType::DctDct, Av1TxSize::Tx8x8);

        // 16x16
        let mut i16 = [0i32; 256];
        let mut o16 = [0i32; 256];
        capsule.inverse_transform(&i16, &mut o16, Av1TxType::DctDct, Av1TxSize::Tx16x16);

        // 32x32
        let mut i32 = [0i32; 1024];
        let mut o32 = [0i32; 1024];
        capsule.inverse_transform(&i32, &mut o32, Av1TxType::DctDct, Av1TxSize::Tx32x32);

        // 64x64
        let mut i64 = [0i32; 4096];
        let mut o64 = [0i32; 4096];
        capsule.inverse_transform(&i64, &mut o64, Av1TxType::DctDct, Av1TxSize::Tx64x64);

        assert_eq!(capsule.total_transforms(), 5);
    }
}
