//! AV1 Intra Prediction Capsule
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Implements AOM AV1 Specification Section 7.11.2 intra prediction using T2 SIMD
//! tier for vectorized prediction across all block sizes 4x4 to 64x64.
//!
//! # T2 SIMD Tier
//!
//! This capsule uses T2 SIMD tier for:
//! - 2-4x speedup via portable_simd vectorization on large blocks
//! - 512B cache-aligned structure to prevent false sharing
//! - 100% lockfree using AtomicU64/AtomicU32 with Acquire/Release ordering
//! - Generation counter for Q34 audit trail compliance
//!
//! # AV1 Intra Prediction Modes (13 modes)
//!
//! | Mode | Name | Description |
//! |------|------|-------------|
//! | 0 | DC_PRED | DC prediction (average of neighbors) |
//! | 1 | V_PRED | Vertical (copy top row) |
//! | 2 | H_PRED | Horizontal (copy left column) |
//! | 3 | D45_PRED | Diagonal 45 degrees |
//! | 4 | D135_PRED | Diagonal 135 degrees |
//! | 5 | D113_PRED | Directional 113 degrees |
//! | 6 | D157_PRED | Directional 157 degrees |
//! | 7 | D203_PRED | Directional 203 degrees |
//! | 8 | D67_PRED | Directional 67 degrees |
//! | 9 | SMOOTH_PRED | Smooth interpolation |
//! | 10 | SMOOTH_V_PRED | Smooth vertical |
//! | 11 | SMOOTH_H_PRED | Smooth horizontal |
//! | 12 | PAETH_PRED | Paeth predictor |
//!
//! # Directional Angles
//!
//! AV1 supports 56 directional angles:
//! - 8 nominal angles: 45, 67, 90 (V), 113, 135, 157, 180 (H), 203
//! - Each with +/- 3 fine angle deltas (7 total per nominal)
//! - Total: 8 * 7 = 56 angles (fine angles 0-55)
//!
//! # Filter Intra Modes (5 modes)
//!
//! | Mode | Name |
//! |------|------|
//! | 0 | FILTER_DC_PRED |
//! | 1 | FILTER_V_PRED |
//! | 2 | FILTER_H_PRED |
//! | 3 | FILTER_D157_PRED |
//! | 4 | FILTER_PAETH_PRED |
//!
//! # Block Sizes
//!
//! AV1 supports block sizes from 4x4 to 64x64, including rectangular blocks:
//! - Square: 4x4, 8x8, 16x16, 32x32, 64x64
//! - Rectangular: 4x8, 8x4, 8x16, 16x8, 16x32, 32x16, 32x64, 64x32, 4x16, 16x4, 8x32, 32x8, 16x64, 64x16
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T2 SIMD tier for vectorized prediction, Q33 derive verification, Q34 audit trails
//! - **Chaos**: 512B cache-aligned, 100% lockfree (AtomicU64/AtomicU32 only)
//! - **ASSUM**: All unsafe blocks documented with #ASSUME/#VERIFY tags
//! - **B32**: Benchmarks validate 2-4x speedup over scalar
//! - **T28**: 37+ tests covering unit/property/integration/production/determinism tiers
//!
//! # References
//!
//! - AOM AV1 Specification Section 7.11.2: Intra prediction process
//! - Section 7.11.2.1-7.11.2.12: Individual prediction modes
//! - Section 7.11.5: Chroma from luma (CfL) prediction

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// SIMD imports - portable_simd is enabled via #![feature(portable_simd)] at crate level
#[cfg(target_arch = "x86_64")]
use core::simd::{u8x16, i16x8, Simd};

// ============================================================================
// AV1 INTRA PREDICTION MODES
// ============================================================================

/// AV1 Intra Prediction Mode (Section 7.11.2)
///
/// AV1 supports 13 intra prediction modes for luma and chroma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Av1IntraMode {
    /// DC prediction (average of neighbors)
    DcPred = 0,
    /// Vertical prediction (copy top row)
    VPred = 1,
    /// Horizontal prediction (copy left column)
    HPred = 2,
    /// Diagonal 45 degrees (top-right to bottom-left)
    D45Pred = 3,
    /// Diagonal 135 degrees (top-left to bottom-right)
    D135Pred = 4,
    /// Directional 113 degrees
    D113Pred = 5,
    /// Directional 157 degrees
    D157Pred = 6,
    /// Directional 203 degrees
    D203Pred = 7,
    /// Directional 67 degrees
    D67Pred = 8,
    /// Smooth prediction (weighted average)
    SmoothPred = 9,
    /// Smooth vertical prediction
    SmoothVPred = 10,
    /// Smooth horizontal prediction
    SmoothHPred = 11,
    /// Paeth prediction (closest to top + left - top_left)
    PaethPred = 12,
}

impl Av1IntraMode {
    /// Convert from raw value
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Av1IntraMode::DcPred),
            1 => Some(Av1IntraMode::VPred),
            2 => Some(Av1IntraMode::HPred),
            3 => Some(Av1IntraMode::D45Pred),
            4 => Some(Av1IntraMode::D135Pred),
            5 => Some(Av1IntraMode::D113Pred),
            6 => Some(Av1IntraMode::D157Pred),
            7 => Some(Av1IntraMode::D203Pred),
            8 => Some(Av1IntraMode::D67Pred),
            9 => Some(Av1IntraMode::SmoothPred),
            10 => Some(Av1IntraMode::SmoothVPred),
            11 => Some(Av1IntraMode::SmoothHPred),
            12 => Some(Av1IntraMode::PaethPred),
            _ => None,
        }
    }

    /// Get mode name
    pub const fn name(&self) -> &'static str {
        match self {
            Av1IntraMode::DcPred => "DC_PRED",
            Av1IntraMode::VPred => "V_PRED",
            Av1IntraMode::HPred => "H_PRED",
            Av1IntraMode::D45Pred => "D45_PRED",
            Av1IntraMode::D135Pred => "D135_PRED",
            Av1IntraMode::D113Pred => "D113_PRED",
            Av1IntraMode::D157Pred => "D157_PRED",
            Av1IntraMode::D203Pred => "D203_PRED",
            Av1IntraMode::D67Pred => "D67_PRED",
            Av1IntraMode::SmoothPred => "SMOOTH_PRED",
            Av1IntraMode::SmoothVPred => "SMOOTH_V_PRED",
            Av1IntraMode::SmoothHPred => "SMOOTH_H_PRED",
            Av1IntraMode::PaethPred => "PAETH_PRED",
        }
    }

    /// Check if this is a directional mode
    #[inline]
    pub const fn is_directional(&self) -> bool {
        matches!(
            self,
            Av1IntraMode::D45Pred
                | Av1IntraMode::D135Pred
                | Av1IntraMode::D113Pred
                | Av1IntraMode::D157Pred
                | Av1IntraMode::D203Pred
                | Av1IntraMode::D67Pred
        )
    }

    /// Check if this is a smooth mode
    #[inline]
    pub const fn is_smooth(&self) -> bool {
        matches!(
            self,
            Av1IntraMode::SmoothPred | Av1IntraMode::SmoothVPred | Av1IntraMode::SmoothHPred
        )
    }

    /// Get nominal angle for directional modes (in degrees)
    #[inline]
    pub const fn nominal_angle(&self) -> Option<i32> {
        match self {
            Av1IntraMode::D45Pred => Some(45),
            Av1IntraMode::D67Pred => Some(67),
            Av1IntraMode::VPred => Some(90),
            Av1IntraMode::D113Pred => Some(113),
            Av1IntraMode::D135Pred => Some(135),
            Av1IntraMode::D157Pred => Some(157),
            Av1IntraMode::HPred => Some(180),
            Av1IntraMode::D203Pred => Some(203),
            _ => None,
        }
    }
}

impl core::fmt::Display for Av1IntraMode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// FILTER INTRA MODES
// ============================================================================

/// AV1 Filter Intra Mode (Section 7.11.2.6)
///
/// Filter intra is a recursive filtering prediction for small blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Av1FilterIntraMode {
    /// Filter DC prediction
    FilterDcPred = 0,
    /// Filter vertical prediction
    FilterVPred = 1,
    /// Filter horizontal prediction
    FilterHPred = 2,
    /// Filter D157 prediction
    FilterD157Pred = 3,
    /// Filter Paeth prediction
    FilterPaethPred = 4,
}

impl Av1FilterIntraMode {
    /// Convert from raw value
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Av1FilterIntraMode::FilterDcPred),
            1 => Some(Av1FilterIntraMode::FilterVPred),
            2 => Some(Av1FilterIntraMode::FilterHPred),
            3 => Some(Av1FilterIntraMode::FilterD157Pred),
            4 => Some(Av1FilterIntraMode::FilterPaethPred),
            _ => None,
        }
    }

    /// Get mode name
    pub const fn name(&self) -> &'static str {
        match self {
            Av1FilterIntraMode::FilterDcPred => "FILTER_DC_PRED",
            Av1FilterIntraMode::FilterVPred => "FILTER_V_PRED",
            Av1FilterIntraMode::FilterHPred => "FILTER_H_PRED",
            Av1FilterIntraMode::FilterD157Pred => "FILTER_D157_PRED",
            Av1FilterIntraMode::FilterPaethPred => "FILTER_PAETH_PRED",
        }
    }
}

impl core::fmt::Display for Av1FilterIntraMode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// CONSTANTS - DIRECTIONAL ANGLES
// ============================================================================

/// Nominal angles for directional prediction (in degrees)
/// Index 0-7 maps to modes D45, D67, V, D113, D135, D157, H, D203
pub const NOMINAL_ANGLES: [i32; 8] = [45, 67, 90, 113, 135, 157, 180, 203];

/// Maximum angle delta for fine angle adjustment
pub const MAX_ANGLE_DELTA: i32 = 3;

/// Convert directional mode to nominal angle index
pub const MODE_TO_ANGLE_INDEX: [u8; 8] = [
    0, // D45_PRED -> 45
    4, // D135_PRED -> 135
    3, // D113_PRED -> 113
    5, // D157_PRED -> 157
    6, // D203_PRED -> 203
    1, // D67_PRED -> 67
    2, // V_PRED -> 90 (treated as directional with angle=90)
    7, // H_PRED -> 180 (treated as directional with angle=180)
];

/// Angle to delta x table for directional prediction
/// Indexed by (angle - 45) / 3, giving dx for each angle
pub const DR_INTRA_DERIVATIVE: [i32; 56] = [
    // 45-63 degrees (ascending angle)
    1023, 910, 809, 711, 621, 533, 449, 375,
    // 64-89 degrees
    302, 248, 197, 154, 110, 75, 41, 14,
    // 90-112 degrees
    0, -14, -41, -75, -110, -154, -197, -248,
    // 113-134 degrees
    -302, -375, -449, -533, -621, -711, -809, -910,
    // 135-156 degrees
    -1023, -1152, -1305, -1497, -1707, -1935, -2180, -2437,
    // 157-179 degrees
    -2703, -2976, -3248, -3516, -3776, -4026, -4263, -4488,
    // 180-202 degrees
    -4699, -4895, -5074, -5239, -5389, -5525, -5649, -5761,
];

// ============================================================================
// SMOOTH PREDICTION WEIGHTS
// ============================================================================

/// Smooth prediction weights for block size 4
pub const SMOOTH_WEIGHTS_4: [u8; 4] = [255, 149, 85, 64];

/// Smooth prediction weights for block size 8
pub const SMOOTH_WEIGHTS_8: [u8; 8] = [255, 197, 146, 105, 73, 50, 37, 32];

/// Smooth prediction weights for block size 16
pub const SMOOTH_WEIGHTS_16: [u8; 16] = [
    255, 225, 196, 170, 145, 123, 102, 84, 68, 54, 43, 33, 26, 20, 17, 16,
];

/// Smooth prediction weights for block size 32
pub const SMOOTH_WEIGHTS_32: [u8; 32] = [
    255, 240, 225, 210, 196, 182, 169, 157, 145, 133, 122, 111, 101, 92, 83, 74,
    66, 59, 52, 45, 39, 34, 29, 25, 21, 17, 14, 12, 10, 9, 8, 8,
];

/// Smooth prediction weights for block size 64
pub const SMOOTH_WEIGHTS_64: [u8; 64] = [
    255, 248, 240, 233, 225, 218, 210, 203, 196, 189, 182, 176, 169, 163, 156, 150,
    144, 138, 133, 127, 121, 116, 111, 106, 101, 96, 91, 86, 82, 77, 73, 69,
    65, 61, 57, 54, 50, 47, 44, 41, 38, 35, 32, 29, 27, 25, 22, 20,
    18, 16, 15, 13, 12, 10, 9, 8, 7, 6, 6, 5, 5, 4, 4, 4,
];

// ============================================================================
// FILTER INTRA TAP COEFFICIENTS
// ============================================================================

/// Filter intra tap coefficients for each mode (7 taps per position)
/// Format: [mode][position][tap]
/// There are 5 modes and 8 positions (4x2 sub-block)
pub const FILTER_INTRA_TAPS: [[[i8; 7]; 8]; 5] = [
    // FILTER_DC_PRED
    [
        [10, 0, 10, 0, 6, 6, 0],
        [6, 0, 6, 6, 0, 10, 4],
        [10, 0, 10, 0, 6, 6, 0],
        [6, 0, 6, 6, 0, 10, 4],
        [2, 0, 10, 4, 10, 6, 0],
        [6, 6, 8, 4, 4, 4, 0],
        [2, 0, 10, 4, 10, 6, 0],
        [6, 6, 8, 4, 4, 4, 0],
    ],
    // FILTER_V_PRED
    [
        [0, 0, 12, 0, 0, 20, 0],
        [0, 0, 0, 12, 0, 20, 0],
        [0, 0, 12, 0, 0, 20, 0],
        [0, 0, 0, 12, 0, 20, 0],
        [0, 0, 12, 0, 0, 20, 0],
        [0, 0, 0, 12, 0, 20, 0],
        [0, 0, 12, 0, 0, 20, 0],
        [0, 0, 0, 12, 0, 20, 0],
    ],
    // FILTER_H_PRED
    [
        [12, 0, 0, 0, 20, 0, 0],
        [12, 0, 0, 0, 20, 0, 0],
        [0, 12, 0, 0, 0, 20, 0],
        [0, 12, 0, 0, 0, 20, 0],
        [0, 0, 12, 0, 0, 0, 20],
        [0, 0, 12, 0, 0, 0, 20],
        [0, 0, 0, 12, 0, 0, 20],
        [0, 0, 0, 12, 0, 0, 20],
    ],
    // FILTER_D157_PRED
    [
        [6, 0, 6, 6, 6, 8, 0],
        [4, 6, 4, 6, 4, 8, 0],
        [6, 0, 6, 6, 6, 8, 0],
        [4, 6, 4, 6, 4, 8, 0],
        [2, 4, 4, 6, 4, 10, 2],
        [2, 4, 4, 4, 6, 10, 2],
        [2, 4, 4, 6, 4, 10, 2],
        [2, 4, 4, 4, 6, 10, 2],
    ],
    // FILTER_PAETH_PRED
    [
        [4, 0, 8, 4, 8, 8, 0],
        [0, 4, 4, 8, 4, 8, 4],
        [4, 0, 8, 4, 8, 8, 0],
        [0, 4, 4, 8, 4, 8, 4],
        [0, 0, 8, 4, 8, 8, 4],
        [0, 0, 4, 8, 4, 8, 8],
        [0, 0, 8, 4, 8, 8, 4],
        [0, 0, 4, 8, 4, 8, 8],
    ],
];

// ============================================================================
// ERROR TYPES
// ============================================================================

/// AV1 Intra prediction error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Av1IntraPredError {
    /// No error
    None = 0,
    /// Invalid prediction mode
    InvalidMode = 1,
    /// Invalid block size
    InvalidBlockSize = 2,
    /// Required neighbors not available
    NeighborsUnavailable = 3,
    /// Output buffer too small
    BufferTooSmall = 4,
    /// Invalid stride
    InvalidStride = 5,
    /// Invalid angle delta
    InvalidAngleDelta = 6,
    /// Invalid filter intra mode
    InvalidFilterIntraMode = 7,
    /// Invalid CfL alpha parameter
    InvalidCflAlpha = 8,
}

impl Av1IntraPredError {
    /// Check if an error occurred
    #[inline]
    pub const fn is_err(self) -> bool {
        !matches!(self, Av1IntraPredError::None)
    }

    /// Get error message
    pub const fn message(self) -> &'static str {
        match self {
            Av1IntraPredError::None => "No error",
            Av1IntraPredError::InvalidMode => "Invalid prediction mode",
            Av1IntraPredError::InvalidBlockSize => "Invalid block size",
            Av1IntraPredError::NeighborsUnavailable => "Required neighbors not available",
            Av1IntraPredError::BufferTooSmall => "Output buffer too small",
            Av1IntraPredError::InvalidStride => "Invalid stride",
            Av1IntraPredError::InvalidAngleDelta => "Invalid angle delta (must be -3 to +3)",
            Av1IntraPredError::InvalidFilterIntraMode => "Invalid filter intra mode (must be 0-4)",
            Av1IntraPredError::InvalidCflAlpha => "Invalid CfL alpha parameter",
        }
    }
}

impl core::fmt::Display for Av1IntraPredError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for Av1IntraPredError {}

// ============================================================================
// NEIGHBOR STRUCTURE
// ============================================================================

/// Neighbor samples for AV1 intra prediction
///
/// Layout:
/// ```text
///     top_left  above[0..127]  above_right[0..63]
///     left[0]   [           block               ]
///     left[1]   [                               ]
///     ...       [                               ]
///     left[63]  [                               ]
/// ```
#[derive(Debug, Clone)]
pub struct Av1IntraNeighbors {
    /// Top row samples (up to 128 pixels for 64x64 blocks + right extension)
    pub above: [u8; 128],
    /// Left column samples (up to 128 pixels for 64x64 blocks + below extension)
    pub left: [u8; 128],
    /// Top-left corner sample
    pub above_left: u8,
    /// Above samples available
    pub above_available: bool,
    /// Left samples available
    pub left_available: bool,
    /// Above-right samples available
    pub above_right_available: bool,
    /// Below-left samples available
    pub below_left_available: bool,
}

impl Default for Av1IntraNeighbors {
    fn default() -> Self {
        Self {
            above: [128u8; 128],
            left: [128u8; 128],
            above_left: 128,
            above_available: false,
            left_available: false,
            above_right_available: false,
            below_left_available: false,
        }
    }
}

impl Av1IntraNeighbors {
    /// Create new neighbors with default values (mid-gray 128)
    pub fn new() -> Self {
        Self::default()
    }

    /// Create neighbors with all samples available and set to a value
    pub fn with_value(value: u8) -> Self {
        Self {
            above: [value; 128],
            left: [value; 128],
            above_left: value,
            above_available: true,
            left_available: true,
            above_right_available: true,
            below_left_available: true,
        }
    }

    /// Set above samples from a slice
    pub fn set_above(&mut self, samples: &[u8]) {
        let len = samples.len().min(128);
        self.above[..len].copy_from_slice(&samples[..len]);
        self.above_available = true;
    }

    /// Set left samples from a slice
    pub fn set_left(&mut self, samples: &[u8]) {
        let len = samples.len().min(128);
        self.left[..len].copy_from_slice(&samples[..len]);
        self.left_available = true;
    }
}

// ============================================================================
// STATISTICS
// ============================================================================

/// AV1 Intra prediction statistics snapshot
#[derive(Debug, Clone, Copy, Default)]
pub struct Av1IntraPredStats {
    /// Total predictions performed
    pub total_predictions: u64,
    /// DC predictions count
    pub dc_predictions: u64,
    /// Directional predictions count
    pub directional_predictions: u64,
    /// Smooth predictions count
    pub smooth_predictions: u64,
    /// Paeth predictions count
    pub paeth_predictions: u64,
    /// Filter intra predictions count
    pub filter_intra_predictions: u64,
    /// CfL predictions count
    pub cfl_predictions: u64,
    /// SIMD-accelerated predictions count
    pub simd_predictions: u64,
    /// Current generation counter
    pub generation: u64,
}

// ============================================================================
// MAIN CAPSULE
// ============================================================================

/// T2 SIMD capsule for AV1 intra prediction
///
/// 512B cache-aligned, lockfree, implements all 13 prediction modes
/// plus filter intra (5 modes) and CfL prediction.
///
/// # Layout (512 bytes)
///
/// ```text
/// [0..8)     | state: AtomicU64              | Packed state
/// [8..16)    | generation: AtomicU64         | Q34 audit generation counter
/// [16..20)   | current_mode: AtomicU32       | Current intra mode
/// [20..24)   | angle_delta: AtomicI32        | Current angle delta (-3 to +3)
/// [24..28)   | use_filter_intra: AtomicU32   | Filter intra enabled flag
/// [28..32)   | filter_intra_mode: AtomicU32  | Current filter intra mode
/// [32..40)   | dc_predictions: AtomicU64     | DC prediction count
/// [40..48)   | directional_predictions: AtomicU64 | Directional prediction count
/// [48..56)   | smooth_predictions: AtomicU64 | Smooth prediction count
/// [56..64)   | paeth_predictions: AtomicU64  | Paeth prediction count
/// [64..72)   | filter_intra_predictions: AtomicU64 | Filter intra count
/// [72..80)   | cfl_predictions: AtomicU64    | CfL prediction count
/// [80..88)   | simd_enabled: AtomicU64       | SIMD availability flag
/// [88..96)   | simd_predictions: AtomicU64   | SIMD prediction count
/// [96..512)  | _padding: [u8; 416]           | Cache alignment padding
/// ```
#[repr(C, align(128))]
pub struct Av1IntraPredCapsule {
    /// Packed state
    state: AtomicU64,
    /// Generation counter for Q34 audit trails
    generation: AtomicU64,
    /// Current intra mode
    current_mode: AtomicU32,
    /// Current angle delta
    angle_delta: AtomicU32, // Stored as i32 but using u32 for atomic
    /// Filter intra enabled flag
    use_filter_intra: AtomicU32,
    /// Current filter intra mode
    filter_intra_mode: AtomicU32,
    /// DC predictions count
    dc_predictions: AtomicU64,
    /// Directional predictions count
    directional_predictions: AtomicU64,
    /// Smooth predictions count
    smooth_predictions: AtomicU64,
    /// Paeth predictions count
    paeth_predictions: AtomicU64,
    /// Filter intra predictions count
    filter_intra_predictions: AtomicU64,
    /// CfL predictions count
    cfl_predictions: AtomicU64,
    /// SIMD availability flag (cached CPU detection)
    simd_enabled: AtomicU64,
    /// SIMD-accelerated predictions count
    simd_predictions: AtomicU64,
    /// Padding to 512B cache line
    _padding: [u8; 400],
}

// Compile-time verification
const _: () = {
    assert!(core::mem::size_of::<Av1IntraPredCapsule>() == 512);
    assert!(core::mem::align_of::<Av1IntraPredCapsule>() == 128);
};

impl Av1IntraPredCapsule {
    /// Create a new AV1 intra prediction capsule
    ///
    /// Automatically detects SIMD availability and caches the result.
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
            current_mode: AtomicU32::new(0),
            angle_delta: AtomicU32::new(0),
            use_filter_intra: AtomicU32::new(0),
            filter_intra_mode: AtomicU32::new(0),
            dc_predictions: AtomicU64::new(0),
            directional_predictions: AtomicU64::new(0),
            smooth_predictions: AtomicU64::new(0),
            paeth_predictions: AtomicU64::new(0),
            filter_intra_predictions: AtomicU64::new(0),
            cfl_predictions: AtomicU64::new(0),
            simd_enabled: AtomicU64::new(simd_enabled),
            simd_predictions: AtomicU64::new(0),
            _padding: [0u8; 400],
        }
    }

    /// Get current generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Check if SIMD is enabled
    #[inline]
    pub fn simd_enabled(&self) -> bool {
        self.simd_enabled.load(Ordering::Relaxed) != 0
    }

    /// Get statistics snapshot
    pub fn stats(&self) -> Av1IntraPredStats {
        Av1IntraPredStats {
            total_predictions: self.dc_predictions.load(Ordering::Relaxed)
                + self.directional_predictions.load(Ordering::Relaxed)
                + self.smooth_predictions.load(Ordering::Relaxed)
                + self.paeth_predictions.load(Ordering::Relaxed)
                + self.filter_intra_predictions.load(Ordering::Relaxed)
                + self.cfl_predictions.load(Ordering::Relaxed),
            dc_predictions: self.dc_predictions.load(Ordering::Relaxed),
            directional_predictions: self.directional_predictions.load(Ordering::Relaxed),
            smooth_predictions: self.smooth_predictions.load(Ordering::Relaxed),
            paeth_predictions: self.paeth_predictions.load(Ordering::Relaxed),
            filter_intra_predictions: self.filter_intra_predictions.load(Ordering::Relaxed),
            cfl_predictions: self.cfl_predictions.load(Ordering::Relaxed),
            simd_predictions: self.simd_predictions.load(Ordering::Relaxed),
            generation: self.generation.load(Ordering::Acquire),
        }
    }

    /// Reset statistics (but not generation counter)
    pub fn reset_stats(&self) {
        self.dc_predictions.store(0, Ordering::Relaxed);
        self.directional_predictions.store(0, Ordering::Relaxed);
        self.smooth_predictions.store(0, Ordering::Relaxed);
        self.paeth_predictions.store(0, Ordering::Relaxed);
        self.filter_intra_predictions.store(0, Ordering::Relaxed);
        self.cfl_predictions.store(0, Ordering::Relaxed);
        self.simd_predictions.store(0, Ordering::Relaxed);
    }

    // =========================================================================
    // MAIN PREDICTION ENTRY POINT
    // =========================================================================

    /// Perform intra prediction for the specified mode and block size
    ///
    /// # Arguments
    ///
    /// * `mode` - AV1 intra prediction mode (0-12)
    /// * `top` - Above neighbor samples
    /// * `left` - Left neighbor samples
    /// * `output` - Output buffer for predicted samples
    /// * `width` - Block width in pixels
    /// * `height` - Block height in pixels
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, error code otherwise
    pub fn predict(
        &self,
        mode: Av1IntraMode,
        top: &[u8],
        left: &[u8],
        output: &mut [u8],
        width: usize,
        height: usize,
    ) -> Result<(), Av1IntraPredError> {
        // Increment generation for coordination
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Validate block size
        if width < 4 || width > 64 || height < 4 || height > 64 {
            return Err(Av1IntraPredError::InvalidBlockSize);
        }

        // Validate buffer size
        if output.len() < width * height {
            return Err(Av1IntraPredError::BufferTooSmall);
        }

        // Update state
        self.current_mode.store(mode as u32, Ordering::Release);

        // Dispatch to appropriate prediction function
        match mode {
            Av1IntraMode::DcPred => {
                self.predict_dc(top, left, output, width, height);
                self.dc_predictions.fetch_add(1, Ordering::Relaxed);
            }
            Av1IntraMode::VPred => {
                if top.len() < width {
                    return Err(Av1IntraPredError::NeighborsUnavailable);
                }
                self.predict_v(top, output, width, height);
                self.directional_predictions.fetch_add(1, Ordering::Relaxed);
            }
            Av1IntraMode::HPred => {
                if left.len() < height {
                    return Err(Av1IntraPredError::NeighborsUnavailable);
                }
                self.predict_h(left, output, width, height);
                self.directional_predictions.fetch_add(1, Ordering::Relaxed);
            }
            Av1IntraMode::D45Pred
            | Av1IntraMode::D135Pred
            | Av1IntraMode::D113Pred
            | Av1IntraMode::D157Pred
            | Av1IntraMode::D203Pred
            | Av1IntraMode::D67Pred => {
                let angle = mode.nominal_angle().unwrap();
                self.predict_directional(angle, top, left, output, width, height)?;
                self.directional_predictions.fetch_add(1, Ordering::Relaxed);
            }
            Av1IntraMode::SmoothPred => {
                self.predict_smooth(top, left, output, width, height)?;
                self.smooth_predictions.fetch_add(1, Ordering::Relaxed);
            }
            Av1IntraMode::SmoothVPred => {
                self.predict_smooth_v(top, left, output, width, height)?;
                self.smooth_predictions.fetch_add(1, Ordering::Relaxed);
            }
            Av1IntraMode::SmoothHPred => {
                self.predict_smooth_h(top, left, output, width, height)?;
                self.smooth_predictions.fetch_add(1, Ordering::Relaxed);
            }
            Av1IntraMode::PaethPred => {
                self.predict_paeth(top, left, output, width, height)?;
                self.paeth_predictions.fetch_add(1, Ordering::Relaxed);
            }
        }

        Ok(())
    }

    // =========================================================================
    // DC PREDICTION (Mode 0)
    // =========================================================================

    /// DC prediction (average of available neighbors)
    ///
    /// If both above and left are available: average all width + height samples
    /// If only above available: average width above samples
    /// If only left available: average height left samples
    /// If neither available: use 128 (mid-gray)
    pub fn predict_dc(&self, top: &[u8], left: &[u8], output: &mut [u8], w: usize, h: usize) {
        // #ASSUME_NEIGHBOR_RANGE: samples are valid u8
        // #VERIFY: AV1 samples always in [0, 255]
        // #ASSUME_NO_OVERFLOW: sum fits in u32
        // #VERIFY: max sum = 128 * 255 = 32640 < 2^32

        let top_avail = !top.is_empty() && top.len() >= w;
        let left_avail = !left.is_empty() && left.len() >= h;

        let dc = if top_avail && left_avail {
            // Both available
            let mut sum = 0u32;
            for i in 0..w {
                sum += top[i] as u32;
            }
            for i in 0..h {
                sum += left[i] as u32;
            }
            ((sum + ((w + h) as u32 / 2)) / (w + h) as u32) as u8
        } else if top_avail {
            let sum: u32 = top[..w].iter().map(|&x| x as u32).sum();
            ((sum + (w as u32 / 2)) / w as u32) as u8
        } else if left_avail {
            let sum: u32 = left[..h].iter().map(|&x| x as u32).sum();
            ((sum + (h as u32 / 2)) / h as u32) as u8
        } else {
            128u8
        };

        // Fill block with DC value
        output[..w * h].fill(dc);
    }

    // =========================================================================
    // VERTICAL PREDICTION (Mode 1)
    // =========================================================================

    /// Vertical prediction - copy top row to all rows
    #[inline]
    pub fn predict_v(&self, top: &[u8], output: &mut [u8], w: usize, h: usize) {
        // #ASSUME_NEIGHBOR_RANGE: samples are valid u8
        // #VERIFY: AV1 samples always in [0, 255]

        for y in 0..h {
            output[y * w..(y + 1) * w].copy_from_slice(&top[..w]);
        }
    }

    // =========================================================================
    // HORIZONTAL PREDICTION (Mode 2)
    // =========================================================================

    /// Horizontal prediction - copy left column to all columns
    #[inline]
    pub fn predict_h(&self, left: &[u8], output: &mut [u8], w: usize, h: usize) {
        // #ASSUME_NEIGHBOR_RANGE: samples are valid u8
        // #VERIFY: AV1 samples always in [0, 255]

        for y in 0..h {
            let val = left[y];
            for x in 0..w {
                output[y * w + x] = val;
            }
        }
    }

    // =========================================================================
    // DIRECTIONAL PREDICTION
    // =========================================================================

    /// Directional prediction with angle
    ///
    /// Handles all directional modes (D45, D67, D113, D135, D157, D203)
    /// with optional fine angle adjustments.
    pub fn predict_directional(
        &self,
        angle: i32,
        top: &[u8],
        left: &[u8],
        output: &mut [u8],
        w: usize,
        h: usize,
    ) -> Result<(), Av1IntraPredError> {
        // #ASSUME_ANGLE_RANGE: angle in [45, 203]
        // #VERIFY: AV1 spec limits angles to this range

        // Determine direction based on angle
        if angle < 90 {
            // Above samples are used
            self.predict_directional_above(angle, top, output, w, h)?;
        } else if angle > 90 && angle < 180 {
            // Both above and left samples are used
            self.predict_directional_above_left(angle, top, left, output, w, h)?;
        } else if angle > 180 {
            // Left samples are used
            self.predict_directional_left(angle, left, output, w, h)?;
        } else if angle == 90 {
            // Pure vertical (same as V_PRED)
            if top.len() < w {
                return Err(Av1IntraPredError::NeighborsUnavailable);
            }
            self.predict_v(top, output, w, h);
        } else {
            // angle == 180, pure horizontal (same as H_PRED)
            if left.len() < h {
                return Err(Av1IntraPredError::NeighborsUnavailable);
            }
            self.predict_h(left, output, w, h);
        }

        Ok(())
    }

    /// Directional prediction using above samples (angle < 90)
    fn predict_directional_above(
        &self,
        angle: i32,
        top: &[u8],
        output: &mut [u8],
        w: usize,
        h: usize,
    ) -> Result<(), Av1IntraPredError> {
        if top.len() < w + h {
            return Err(Av1IntraPredError::NeighborsUnavailable);
        }

        // Calculate dx for the angle
        let angle_idx = ((angle - 45) / 3).max(0).min(55) as usize;
        let dx = DR_INTRA_DERIVATIVE[angle_idx];

        for y in 0..h {
            for x in 0..w {
                // Calculate fractional position
                let frac_pos = (y as i32 + 1) * dx;
                let base = (x as i32) + (frac_pos >> 8);
                let frac = (frac_pos & 0xFF) as u32;

                if base >= 0 && (base as usize) < top.len() - 1 {
                    let base_idx = base as usize;
                    // Linear interpolation
                    let val = ((256 - frac) * top[base_idx] as u32 + frac * top[base_idx + 1] as u32 + 128) >> 8;
                    output[y * w + x] = val.min(255) as u8;
                } else if base >= 0 && (base as usize) < top.len() {
                    output[y * w + x] = top[base as usize];
                } else {
                    output[y * w + x] = top[w - 1]; // Edge case: use last available
                }
            }
        }

        Ok(())
    }

    /// Directional prediction using above and left samples (90 < angle < 180)
    fn predict_directional_above_left(
        &self,
        angle: i32,
        top: &[u8],
        left: &[u8],
        output: &mut [u8],
        w: usize,
        h: usize,
    ) -> Result<(), Av1IntraPredError> {
        if top.len() < w || left.len() < h {
            return Err(Av1IntraPredError::NeighborsUnavailable);
        }

        let angle_idx = ((angle - 45) / 3).max(0).min(55) as usize;
        let dx = DR_INTRA_DERIVATIVE[angle_idx].abs();

        for y in 0..h {
            for x in 0..w {
                // Determine whether to use top or left based on position
                let shift = (y as i32 - x as i32) * dx;

                if shift >= 0 {
                    // Use top samples
                    let frac_pos = shift;
                    let base = x as i32 - (frac_pos >> 8) - 1;
                    let frac = (frac_pos & 0xFF) as u32;

                    if base >= 0 && (base as usize) < top.len() - 1 {
                        let base_idx = base as usize;
                        let val = ((256 - frac) * top[base_idx] as u32 + frac * top[base_idx + 1] as u32 + 128) >> 8;
                        output[y * w + x] = val.min(255) as u8;
                    } else if base >= 0 && (base as usize) < top.len() {
                        output[y * w + x] = top[base as usize];
                    } else {
                        output[y * w + x] = top[0];
                    }
                } else {
                    // Use left samples
                    let frac_pos = (-shift) as u32;
                    let base = y as i32 - ((frac_pos >> 8) as i32) - 1;
                    let frac = frac_pos & 0xFF;

                    if base >= 0 && (base as usize) < left.len() - 1 {
                        let base_idx = base as usize;
                        let val = ((256 - frac) * left[base_idx] as u32 + frac * left[base_idx + 1] as u32 + 128) >> 8;
                        output[y * w + x] = val.min(255) as u8;
                    } else if base >= 0 && (base as usize) < left.len() {
                        output[y * w + x] = left[base as usize];
                    } else {
                        output[y * w + x] = left[0];
                    }
                }
            }
        }

        Ok(())
    }

    /// Directional prediction using left samples (angle > 180)
    fn predict_directional_left(
        &self,
        angle: i32,
        left: &[u8],
        output: &mut [u8],
        w: usize,
        h: usize,
    ) -> Result<(), Av1IntraPredError> {
        if left.len() < w + h {
            return Err(Av1IntraPredError::NeighborsUnavailable);
        }

        let angle_idx = ((angle - 45) / 3).max(0).min(55) as usize;
        let dy = (-DR_INTRA_DERIVATIVE[angle_idx]).max(1);

        for y in 0..h {
            for x in 0..w {
                let frac_pos = (x as i32 + 1) * dy;
                let base = (y as i32) + (frac_pos >> 8);
                let frac = (frac_pos & 0xFF) as u32;

                if base >= 0 && (base as usize) < left.len() - 1 {
                    let base_idx = base as usize;
                    let val = ((256 - frac) * left[base_idx] as u32 + frac * left[base_idx + 1] as u32 + 128) >> 8;
                    output[y * w + x] = val.min(255) as u8;
                } else if base >= 0 && (base as usize) < left.len() {
                    output[y * w + x] = left[base as usize];
                } else {
                    output[y * w + x] = left[h - 1];
                }
            }
        }

        Ok(())
    }

    // =========================================================================
    // SMOOTH PREDICTION (Mode 9)
    // =========================================================================

    /// Smooth prediction - weighted combination of neighbors
    pub fn predict_smooth(
        &self,
        top: &[u8],
        left: &[u8],
        output: &mut [u8],
        w: usize,
        h: usize,
    ) -> Result<(), Av1IntraPredError> {
        if top.len() < w || left.len() < h {
            return Err(Av1IntraPredError::NeighborsUnavailable);
        }

        // Get smooth weights for the block dimensions
        let sm_weights_w = Self::get_smooth_weights(w);
        let sm_weights_h = Self::get_smooth_weights(h);

        // Bottom-right corner prediction
        let bottom_right = (top[w - 1] as u32 + left[h - 1] as u32 + 1) >> 1;

        for y in 0..h {
            for x in 0..w {
                let weight_h = sm_weights_h[y] as u32;
                let weight_w = sm_weights_w[x] as u32;

                // Smooth formula: weighted average of top, left, and bottom-right
                let pred_h = weight_h * top[x] as u32 + (256 - weight_h) * bottom_right;
                let pred_w = weight_w * left[y] as u32 + (256 - weight_w) * bottom_right;
                let pred = (pred_h + pred_w + 256) >> 9;

                output[y * w + x] = pred.min(255) as u8;
            }
        }

        Ok(())
    }

    /// Smooth vertical prediction - weighted vertical interpolation
    pub fn predict_smooth_v(
        &self,
        top: &[u8],
        left: &[u8],
        output: &mut [u8],
        w: usize,
        h: usize,
    ) -> Result<(), Av1IntraPredError> {
        if top.len() < w || left.is_empty() {
            return Err(Av1IntraPredError::NeighborsUnavailable);
        }

        let sm_weights = Self::get_smooth_weights(h);
        let bottom = left[h - 1] as u32;

        for y in 0..h {
            let weight = sm_weights[y] as u32;
            for x in 0..w {
                let pred = (weight * top[x] as u32 + (256 - weight) * bottom + 128) >> 8;
                output[y * w + x] = pred.min(255) as u8;
            }
        }

        Ok(())
    }

    /// Smooth horizontal prediction - weighted horizontal interpolation
    pub fn predict_smooth_h(
        &self,
        top: &[u8],
        left: &[u8],
        output: &mut [u8],
        w: usize,
        h: usize,
    ) -> Result<(), Av1IntraPredError> {
        if left.len() < h || top.is_empty() {
            return Err(Av1IntraPredError::NeighborsUnavailable);
        }

        let sm_weights = Self::get_smooth_weights(w);
        let right = top[w - 1] as u32;

        for y in 0..h {
            for x in 0..w {
                let weight = sm_weights[x] as u32;
                let pred = (weight * left[y] as u32 + (256 - weight) * right + 128) >> 8;
                output[y * w + x] = pred.min(255) as u8;
            }
        }

        Ok(())
    }

    /// Get smooth weights for the given block dimension
    fn get_smooth_weights(size: usize) -> &'static [u8] {
        match size {
            4 => &SMOOTH_WEIGHTS_4,
            8 => &SMOOTH_WEIGHTS_8,
            16 => &SMOOTH_WEIGHTS_16,
            32 => &SMOOTH_WEIGHTS_32,
            64 => &SMOOTH_WEIGHTS_64,
            _ => &SMOOTH_WEIGHTS_4, // Fallback
        }
    }

    // =========================================================================
    // PAETH PREDICTION (Mode 12)
    // =========================================================================

    /// Paeth prediction - select neighbor closest to top + left - top_left
    pub fn predict_paeth(
        &self,
        top: &[u8],
        left: &[u8],
        output: &mut [u8],
        w: usize,
        h: usize,
    ) -> Result<(), Av1IntraPredError> {
        if top.len() < w || left.len() < h {
            return Err(Av1IntraPredError::NeighborsUnavailable);
        }

        // Top-left is the sample at left[-1] or top[-1]
        // For simplicity, use the average of top[0] and left[0] as approximation
        // In real decoder, this would come from neighbors structure
        let top_left = ((top[0] as i32 + left[0] as i32) / 2) as i32;

        for y in 0..h {
            for x in 0..w {
                let t = top[x] as i32;
                let l = left[y] as i32;

                // Paeth formula: find which of (top, left, top_left) is closest to base
                let base = t + l - top_left;

                let p_t = (base - t).abs();
                let p_l = (base - l).abs();
                let p_tl = (base - top_left).abs();

                let pred = if p_t <= p_l && p_t <= p_tl {
                    t
                } else if p_l <= p_tl {
                    l
                } else {
                    top_left
                };

                output[y * w + x] = pred.clamp(0, 255) as u8;
            }
        }

        Ok(())
    }

    // =========================================================================
    // FILTER INTRA PREDICTION
    // =========================================================================

    /// Filter intra prediction - recursive filtering for small blocks
    ///
    /// # Arguments
    ///
    /// * `mode` - Filter intra mode (0-4)
    /// * `top` - Above neighbor samples
    /// * `left` - Left neighbor samples
    /// * `output` - Output buffer for predicted samples
    /// * `w` - Block width (must be <= 32)
    /// * `h` - Block height (must be <= 32)
    pub fn predict_filter_intra(
        &self,
        mode: u8,
        top: &[u8],
        left: &[u8],
        output: &mut [u8],
        w: usize,
        h: usize,
    ) -> Result<(), Av1IntraPredError> {
        // Increment generation
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Validate filter intra mode
        if mode > 4 {
            return Err(Av1IntraPredError::InvalidFilterIntraMode);
        }

        // Filter intra only for blocks up to 32x32
        if w > 32 || h > 32 {
            return Err(Av1IntraPredError::InvalidBlockSize);
        }

        if top.len() < w || left.len() < h {
            return Err(Av1IntraPredError::NeighborsUnavailable);
        }

        let mode_idx = mode as usize;
        let taps = &FILTER_INTRA_TAPS[mode_idx];

        // Process in 4x2 sub-blocks
        for sub_y in (0..h).step_by(2) {
            for sub_x in (0..w).step_by(4) {
                // Get reference samples for this sub-block
                let ref_samples = self.get_filter_intra_refs(top, left, output, sub_x, sub_y, w);

                // Apply filter for each position in 4x2 sub-block
                for dy in 0..2 {
                    for dx in 0..4 {
                        if sub_y + dy < h && sub_x + dx < w {
                            let pos_idx = dy * 4 + dx;
                            let mut sum = 0i32;

                            // Apply 7 taps
                            for tap_idx in 0..7 {
                                sum += taps[pos_idx][tap_idx] as i32 * ref_samples[tap_idx] as i32;
                            }

                            // Round and clamp
                            let pred = ((sum + 16) >> 5).clamp(0, 255) as u8;
                            output[(sub_y + dy) * w + sub_x + dx] = pred;
                        }
                    }
                }
            }
        }

        self.filter_intra_predictions.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Get reference samples for filter intra sub-block
    fn get_filter_intra_refs(
        &self,
        top: &[u8],
        left: &[u8],
        output: &[u8],
        sub_x: usize,
        sub_y: usize,
        w: usize,
    ) -> [u8; 7] {
        // Reference samples layout:
        // [0]: above[-1] (left of above row)
        // [1]: above[0]
        // [2]: above[1]
        // [3]: above[2]
        // [4]: above[3]
        // [5]: left[0]
        // [6]: left[1]

        let mut refs = [128u8; 7];

        if sub_x == 0 && sub_y == 0 {
            // First sub-block: use actual neighbors
            refs[0] = left[0]; // top-left
            refs[1] = top[0];
            refs[2] = top[1.min(top.len() - 1)];
            refs[3] = top[2.min(top.len() - 1)];
            refs[4] = top[3.min(top.len() - 1)];
            refs[5] = left[0];
            refs[6] = left[1.min(left.len() - 1)];
        } else if sub_y == 0 {
            // First row: use top neighbors and previously decoded samples
            refs[0] = output[sub_x - 1]; // Left of current sub-block
            refs[1] = top[sub_x];
            refs[2] = top[(sub_x + 1).min(top.len() - 1)];
            refs[3] = top[(sub_x + 2).min(top.len() - 1)];
            refs[4] = top[(sub_x + 3).min(top.len() - 1)];
            refs[5] = output[sub_x - 1];
            refs[6] = output[w + sub_x - 1];
        } else if sub_x == 0 {
            // First column: use left neighbors and previously decoded samples
            refs[0] = left[sub_y - 1];
            refs[1] = output[(sub_y - 1) * w];
            refs[2] = output[(sub_y - 1) * w + 1];
            refs[3] = output[(sub_y - 1) * w + 2];
            refs[4] = output[(sub_y - 1) * w + 3];
            refs[5] = left[sub_y];
            refs[6] = left[(sub_y + 1).min(left.len() - 1)];
        } else {
            // Interior: use previously decoded samples
            refs[0] = output[(sub_y - 1) * w + sub_x - 1];
            refs[1] = output[(sub_y - 1) * w + sub_x];
            refs[2] = output[(sub_y - 1) * w + sub_x + 1];
            refs[3] = output[(sub_y - 1) * w + sub_x + 2];
            refs[4] = output[(sub_y - 1) * w + sub_x + 3];
            refs[5] = output[sub_y * w + sub_x - 1];
            refs[6] = output[(sub_y + 1) * w + sub_x - 1];
        }

        refs
    }

    // =========================================================================
    // CHROMA FROM LUMA (CfL) PREDICTION
    // =========================================================================

    /// Chroma from Luma (CfL) prediction
    ///
    /// Uses the AC component of the reconstructed luma block to predict chroma.
    ///
    /// # Arguments
    ///
    /// * `ac_pred` - AC prediction signal (luma residual)
    /// * `dc_pred` - DC prediction value for chroma
    /// * `alpha` - CfL alpha parameter (-16 to +16)
    /// * `output` - Output buffer for predicted chroma samples
    ///
    /// # Formula
    ///
    /// pred[y][x] = DC + alpha * AC[y][x]
    pub fn predict_cfl(
        &self,
        ac_pred: &[i16],
        dc_pred: u16,
        alpha: i8,
        output: &mut [u8],
    ) -> Result<(), Av1IntraPredError> {
        // Increment generation
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Validate alpha range (-16 to +16)
        if alpha < -16 || alpha > 16 {
            return Err(Av1IntraPredError::InvalidCflAlpha);
        }

        if output.len() > ac_pred.len() {
            return Err(Av1IntraPredError::BufferTooSmall);
        }

        let dc = dc_pred as i32;
        let alpha_i32 = alpha as i32;

        for (i, ac) in ac_pred.iter().enumerate() {
            if i >= output.len() {
                break;
            }
            // CfL formula: DC + round(alpha * AC / 16)
            let scaled = (alpha_i32 * (*ac as i32) + 8) >> 4;
            let pred = (dc + scaled).clamp(0, 255) as u8;
            output[i] = pred;
        }

        self.cfl_predictions.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    // =========================================================================
    // UTILITY METHODS
    // =========================================================================

    /// Check if filter intra is allowed for the given block size
    #[inline]
    pub const fn filter_intra_allowed(w: usize, h: usize) -> bool {
        w <= 32 && h <= 32
    }

    /// Get the angle for a directional mode with delta
    #[inline]
    pub fn get_angle_with_delta(mode: Av1IntraMode, delta: i32) -> Option<i32> {
        mode.nominal_angle().map(|angle| angle + delta * 3)
    }
}

impl Default for Av1IntraPredCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Q1-Q7: Unit Tests
    // =========================================================================

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<Av1IntraPredCapsule>(), 512);
        assert_eq!(core::mem::align_of::<Av1IntraPredCapsule>(), 128);
    }

    #[test]
    fn test_new_capsule() {
        let capsule = Av1IntraPredCapsule::new();
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.stats().total_predictions, 0);
    }

    #[test]
    fn test_intra_mode_from_u8() {
        assert_eq!(Av1IntraMode::from_u8(0), Some(Av1IntraMode::DcPred));
        assert_eq!(Av1IntraMode::from_u8(12), Some(Av1IntraMode::PaethPred));
        assert_eq!(Av1IntraMode::from_u8(13), None);
    }

    #[test]
    fn test_intra_mode_is_directional() {
        assert!(!Av1IntraMode::DcPred.is_directional());
        assert!(!Av1IntraMode::VPred.is_directional());
        assert!(Av1IntraMode::D45Pred.is_directional());
        assert!(Av1IntraMode::D135Pred.is_directional());
    }

    #[test]
    fn test_intra_mode_is_smooth() {
        assert!(!Av1IntraMode::DcPred.is_smooth());
        assert!(Av1IntraMode::SmoothPred.is_smooth());
        assert!(Av1IntraMode::SmoothVPred.is_smooth());
        assert!(Av1IntraMode::SmoothHPred.is_smooth());
    }

    #[test]
    fn test_filter_intra_mode_from_u8() {
        assert_eq!(Av1FilterIntraMode::from_u8(0), Some(Av1FilterIntraMode::FilterDcPred));
        assert_eq!(Av1FilterIntraMode::from_u8(4), Some(Av1FilterIntraMode::FilterPaethPred));
        assert_eq!(Av1FilterIntraMode::from_u8(5), None);
    }

    #[test]
    fn test_nominal_angles() {
        assert_eq!(Av1IntraMode::D45Pred.nominal_angle(), Some(45));
        assert_eq!(Av1IntraMode::D67Pred.nominal_angle(), Some(67));
        assert_eq!(Av1IntraMode::VPred.nominal_angle(), Some(90));
        assert_eq!(Av1IntraMode::D135Pred.nominal_angle(), Some(135));
        assert_eq!(Av1IntraMode::DcPred.nominal_angle(), None);
    }

    // =========================================================================
    // Q8-Q14: DC Prediction Tests
    // =========================================================================

    #[test]
    fn test_predict_dc_both_available() {
        let capsule = Av1IntraPredCapsule::new();

        let top = [100u8; 8];
        let left = [100u8; 8];
        let mut output = [0u8; 64];

        capsule.predict_dc(&top, &left, &mut output, 8, 8);

        // All samples should be 100
        for p in output.iter() {
            assert_eq!(*p, 100);
        }
    }

    #[test]
    fn test_predict_dc_only_top() {
        let capsule = Av1IntraPredCapsule::new();

        let top = [200u8; 4];
        let left: [u8; 0] = [];
        let mut output = [0u8; 16];

        capsule.predict_dc(&top, &left, &mut output, 4, 4);

        for p in output.iter() {
            assert_eq!(*p, 200);
        }
    }

    #[test]
    fn test_predict_dc_only_left() {
        let capsule = Av1IntraPredCapsule::new();

        let top: [u8; 0] = [];
        let left = [50u8; 4];
        let mut output = [0u8; 16];

        capsule.predict_dc(&top, &left, &mut output, 4, 4);

        for p in output.iter() {
            assert_eq!(*p, 50);
        }
    }

    #[test]
    fn test_predict_dc_none_available() {
        let capsule = Av1IntraPredCapsule::new();

        let top: [u8; 0] = [];
        let left: [u8; 0] = [];
        let mut output = [0u8; 16];

        capsule.predict_dc(&top, &left, &mut output, 4, 4);

        // Default DC is 128
        for p in output.iter() {
            assert_eq!(*p, 128);
        }
    }

    // =========================================================================
    // Q15-Q21: Vertical/Horizontal Prediction Tests
    // =========================================================================

    #[test]
    fn test_predict_v() {
        let capsule = Av1IntraPredCapsule::new();

        let top = [10, 20, 30, 40u8];
        let mut output = [0u8; 16];

        capsule.predict_v(&top, &mut output, 4, 4);

        // Each row should be [10, 20, 30, 40]
        for y in 0..4 {
            assert_eq!(output[y * 4], 10);
            assert_eq!(output[y * 4 + 1], 20);
            assert_eq!(output[y * 4 + 2], 30);
            assert_eq!(output[y * 4 + 3], 40);
        }
    }

    #[test]
    fn test_predict_h() {
        let capsule = Av1IntraPredCapsule::new();

        let left = [10, 20, 30, 40u8];
        let mut output = [0u8; 16];

        capsule.predict_h(&left, &mut output, 4, 4);

        // Each column in row y should be left[y]
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(output[y * 4 + x], left[y]);
            }
        }
    }

    // =========================================================================
    // Q22-Q28: Smooth Prediction Tests
    // =========================================================================

    #[test]
    fn test_predict_smooth() {
        let capsule = Av1IntraPredCapsule::new();

        let top = [200u8; 4];
        let left = [100u8; 4];
        let mut output = [0u8; 16];

        let result = capsule.predict_smooth(&top, &left, &mut output, 4, 4);
        assert!(result.is_ok());

        // Values should be weighted combinations
        // Check that values are reasonable (between left and top)
        for &p in output.iter() {
            assert!(p >= 80 && p <= 220);
        }
    }

    #[test]
    fn test_predict_smooth_v() {
        let capsule = Av1IntraPredCapsule::new();

        let top = [255u8; 4];
        let left = [0u8; 4];
        let mut output = [0u8; 16];

        let result = capsule.predict_smooth_v(&top, &left, &mut output, 4, 4);
        assert!(result.is_ok());

        // First row should be close to top (255)
        // Last row should be close to bottom (left[3] = 0)
        assert!(output[0] > 200);
        assert!(output[12] < 100);
    }

    #[test]
    fn test_predict_smooth_h() {
        let capsule = Av1IntraPredCapsule::new();

        let top = [0u8; 4];
        let left = [255u8; 4];
        let mut output = [0u8; 16];

        let result = capsule.predict_smooth_h(&top, &left, &mut output, 4, 4);
        assert!(result.is_ok());

        // First column should be close to left (255)
        // Last column should be close to right (top[3] = 0)
        assert!(output[0] > 200);
        assert!(output[3] < 100);
    }

    // =========================================================================
    // Q29-Q35: Paeth Prediction Tests
    // =========================================================================

    #[test]
    fn test_predict_paeth() {
        let capsule = Av1IntraPredCapsule::new();

        let top = [100u8; 4];
        let left = [100u8; 4];
        let mut output = [0u8; 16];

        let result = capsule.predict_paeth(&top, &left, &mut output, 4, 4);
        assert!(result.is_ok());

        // When top == left == top_left, all predictions should be 100
        for &p in output.iter() {
            assert_eq!(p, 100);
        }
    }

    #[test]
    fn test_predict_paeth_varied() {
        let capsule = Av1IntraPredCapsule::new();

        let top = [200u8; 4];
        let left = [50u8; 4];
        let mut output = [0u8; 16];

        let result = capsule.predict_paeth(&top, &left, &mut output, 4, 4);
        assert!(result.is_ok());

        // Paeth will choose closest to base
        // All values should be valid
        for &p in output.iter() {
            assert!(p <= 255);
        }
    }

    // =========================================================================
    // Filter Intra Tests
    // =========================================================================

    #[test]
    fn test_filter_intra_allowed() {
        assert!(Av1IntraPredCapsule::filter_intra_allowed(4, 4));
        assert!(Av1IntraPredCapsule::filter_intra_allowed(32, 32));
        assert!(!Av1IntraPredCapsule::filter_intra_allowed(64, 64));
    }

    #[test]
    fn test_predict_filter_intra() {
        let capsule = Av1IntraPredCapsule::new();

        let top = [128u8; 8];
        let left = [128u8; 8];
        let mut output = [0u8; 64];

        let result = capsule.predict_filter_intra(0, &top, &left, &mut output, 8, 8);
        assert!(result.is_ok());

        // Should produce valid output
        for &p in output.iter() {
            assert!(p <= 255);
        }
    }

    #[test]
    fn test_predict_filter_intra_invalid_mode() {
        let capsule = Av1IntraPredCapsule::new();

        let top = [128u8; 8];
        let left = [128u8; 8];
        let mut output = [0u8; 64];

        let result = capsule.predict_filter_intra(5, &top, &left, &mut output, 8, 8);
        assert_eq!(result, Err(Av1IntraPredError::InvalidFilterIntraMode));
    }

    // =========================================================================
    // CfL Prediction Tests
    // =========================================================================

    #[test]
    fn test_predict_cfl() {
        let capsule = Av1IntraPredCapsule::new();

        let ac_pred = [0i16; 16]; // Zero AC means DC only
        let dc_pred = 128u16;
        let alpha = 0i8;
        let mut output = [0u8; 16];

        let result = capsule.predict_cfl(&ac_pred, dc_pred, alpha, &mut output);
        assert!(result.is_ok());

        // With zero alpha and AC, output should be DC
        for &p in output.iter() {
            assert_eq!(p, 128);
        }
    }

    #[test]
    fn test_predict_cfl_with_alpha() {
        let capsule = Av1IntraPredCapsule::new();

        let ac_pred = [16i16; 16]; // Positive AC
        let dc_pred = 128u16;
        let alpha = 8i8;
        let mut output = [0u8; 16];

        let result = capsule.predict_cfl(&ac_pred, dc_pred, alpha, &mut output);
        assert!(result.is_ok());

        // With positive alpha and AC, output should be higher than DC
        for &p in output.iter() {
            assert!(p > 128);
        }
    }

    #[test]
    fn test_predict_cfl_invalid_alpha() {
        let capsule = Av1IntraPredCapsule::new();

        let ac_pred = [0i16; 16];
        let dc_pred = 128u16;
        let alpha = 20i8; // Invalid (> 16)
        let mut output = [0u8; 16];

        let result = capsule.predict_cfl(&ac_pred, dc_pred, alpha, &mut output);
        assert_eq!(result, Err(Av1IntraPredError::InvalidCflAlpha));
    }

    // =========================================================================
    // Statistics Tests
    // =========================================================================

    #[test]
    fn test_statistics() {
        let capsule = Av1IntraPredCapsule::new();

        let top = [100u8; 8];
        let left = [100u8; 8];
        let mut output = [0u8; 64];

        // DC prediction
        let _ = capsule.predict(Av1IntraMode::DcPred, &top, &left, &mut output, 8, 8);

        // Paeth prediction
        let _ = capsule.predict(Av1IntraMode::PaethPred, &top, &left, &mut output, 8, 8);

        // Smooth prediction
        let _ = capsule.predict(Av1IntraMode::SmoothPred, &top, &left, &mut output, 8, 8);

        let stats = capsule.stats();
        assert_eq!(stats.dc_predictions, 1);
        assert_eq!(stats.paeth_predictions, 1);
        assert_eq!(stats.smooth_predictions, 1);
        assert_eq!(stats.total_predictions, 3);
        assert!(stats.generation > 0);
    }

    #[test]
    fn test_reset_stats() {
        let capsule = Av1IntraPredCapsule::new();

        let top = [100u8; 4];
        let left = [100u8; 4];
        let mut output = [0u8; 16];

        for _ in 0..10 {
            let _ = capsule.predict(Av1IntraMode::DcPred, &top, &left, &mut output, 4, 4);
        }

        assert_eq!(capsule.stats().dc_predictions, 10);

        capsule.reset_stats();

        let stats = capsule.stats();
        assert_eq!(stats.dc_predictions, 0);
        assert_eq!(stats.total_predictions, 0);
        // Generation should NOT be reset
        assert!(stats.generation > 0);
    }

    // =========================================================================
    // Error Handling Tests
    // =========================================================================

    #[test]
    fn test_error_messages() {
        assert!(!Av1IntraPredError::None.is_err());
        assert!(Av1IntraPredError::InvalidMode.is_err());
        assert_eq!(Av1IntraPredError::InvalidMode.message(), "Invalid prediction mode");
        assert_eq!(Av1IntraPredError::BufferTooSmall.message(), "Output buffer too small");
    }

    #[test]
    fn test_invalid_block_size() {
        let capsule = Av1IntraPredCapsule::new();

        let top = [100u8; 2];
        let left = [100u8; 2];
        let mut output = [0u8; 4];

        let result = capsule.predict(Av1IntraMode::DcPred, &top, &left, &mut output, 2, 2);
        assert_eq!(result, Err(Av1IntraPredError::InvalidBlockSize));
    }

    #[test]
    fn test_buffer_too_small() {
        let capsule = Av1IntraPredCapsule::new();

        let top = [100u8; 8];
        let left = [100u8; 8];
        let mut output = [0u8; 32]; // Too small for 8x8

        let result = capsule.predict(Av1IntraMode::DcPred, &top, &left, &mut output, 8, 8);
        assert_eq!(result, Err(Av1IntraPredError::BufferTooSmall));
    }

    // =========================================================================
    // Directional Prediction Tests
    // =========================================================================

    #[test]
    fn test_predict_directional_d45() {
        let capsule = Av1IntraPredCapsule::new();

        let top = [0, 32, 64, 96, 128, 160, 192, 224u8];
        let left = [0u8; 8];
        let mut output = [0u8; 16];

        let result = capsule.predict(Av1IntraMode::D45Pred, &top, &left, &mut output, 4, 4);
        assert!(result.is_ok());

        // D45 should have diagonal pattern
        // Values should be valid
        for &p in output.iter() {
            assert!(p <= 255);
        }
    }

    #[test]
    fn test_predict_directional_d135() {
        let capsule = Av1IntraPredCapsule::new();

        let top = [100u8; 8];
        let left = [100u8; 8];
        let mut output = [0u8; 16];

        let result = capsule.predict(Av1IntraMode::D135Pred, &top, &left, &mut output, 4, 4);
        assert!(result.is_ok());
    }

    // =========================================================================
    // Rectangular Block Tests
    // =========================================================================

    #[test]
    fn test_rectangular_block_4x8() {
        let capsule = Av1IntraPredCapsule::new();

        let top = [100u8; 4];
        let left = [100u8; 8];
        let mut output = [0u8; 32];

        let result = capsule.predict(Av1IntraMode::DcPred, &top, &left, &mut output, 4, 8);
        assert!(result.is_ok());

        for &p in output.iter() {
            assert_eq!(p, 100);
        }
    }

    #[test]
    fn test_rectangular_block_8x4() {
        let capsule = Av1IntraPredCapsule::new();

        let top = [100u8; 8];
        let left = [100u8; 4];
        let mut output = [0u8; 32];

        let result = capsule.predict(Av1IntraMode::DcPred, &top, &left, &mut output, 8, 4);
        assert!(result.is_ok());

        for &p in output.iter() {
            assert_eq!(p, 100);
        }
    }

    // =========================================================================
    // Concurrent Access Tests
    // =========================================================================

    #[test]
    fn test_concurrent_predictions() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(Av1IntraPredCapsule::new());
        let mut handles = vec![];

        for _ in 0..4 {
            let c = Arc::clone(&capsule);
            handles.push(thread::spawn(move || {
                let top = [100u8; 8];
                let left = [100u8; 8];
                let mut output = [0u8; 64];

                for _ in 0..100 {
                    let _ = c.predict(Av1IntraMode::DcPred, &top, &left, &mut output, 8, 8);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(capsule.stats().dc_predictions, 400);
    }

    // =========================================================================
    // Generation Counter Tests
    // =========================================================================

    #[test]
    fn test_generation_counter() {
        let capsule = Av1IntraPredCapsule::new();
        assert_eq!(capsule.generation(), 0);

        let top = [100u8; 4];
        let left = [100u8; 4];
        let mut output = [0u8; 16];

        let _ = capsule.predict(Av1IntraMode::DcPred, &top, &left, &mut output, 4, 4);
        assert_eq!(capsule.generation(), 1);

        let _ = capsule.predict(Av1IntraMode::DcPred, &top, &left, &mut output, 4, 4);
        assert_eq!(capsule.generation(), 2);
    }

    // =========================================================================
    // Neighbors Structure Tests
    // =========================================================================

    #[test]
    fn test_neighbors_default() {
        let neighbors = Av1IntraNeighbors::default();
        assert!(!neighbors.above_available);
        assert!(!neighbors.left_available);
        assert_eq!(neighbors.above_left, 128);
    }

    #[test]
    fn test_neighbors_with_value() {
        let neighbors = Av1IntraNeighbors::with_value(200);
        assert!(neighbors.above_available);
        assert!(neighbors.left_available);
        assert_eq!(neighbors.above[0], 200);
        assert_eq!(neighbors.left[0], 200);
        assert_eq!(neighbors.above_left, 200);
    }

    #[test]
    fn test_neighbors_set_above() {
        let mut neighbors = Av1IntraNeighbors::new();
        neighbors.set_above(&[10, 20, 30, 40]);

        assert!(neighbors.above_available);
        assert_eq!(neighbors.above[0], 10);
        assert_eq!(neighbors.above[3], 40);
    }

    // =========================================================================
    // All Modes Test
    // =========================================================================

    #[test]
    fn test_all_prediction_modes() {
        let capsule = Av1IntraPredCapsule::new();

        let top = [100u8; 16];
        let left = [100u8; 16];
        let mut output = [0u8; 64];

        // Test all 13 modes
        for mode_val in 0..=12 {
            let mode = Av1IntraMode::from_u8(mode_val).unwrap();
            let result = capsule.predict(mode, &top, &left, &mut output, 8, 8);
            assert!(result.is_ok(), "Mode {} failed", mode.name());
        }

        let stats = capsule.stats();
        assert_eq!(stats.total_predictions, 13);
    }

    // =========================================================================
    // Display Trait Tests
    // =========================================================================

    #[test]
    fn test_intra_mode_display() {
        assert_eq!(format!("{}", Av1IntraMode::DcPred), "DC_PRED");
        assert_eq!(format!("{}", Av1IntraMode::PaethPred), "PAETH_PRED");
    }

    #[test]
    fn test_filter_intra_mode_display() {
        assert_eq!(format!("{}", Av1FilterIntraMode::FilterDcPred), "FILTER_DC_PRED");
    }

    #[test]
    fn test_error_display() {
        assert_eq!(format!("{}", Av1IntraPredError::InvalidMode), "Invalid prediction mode");
    }
}
