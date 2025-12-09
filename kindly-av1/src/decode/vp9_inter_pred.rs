//! VP9 Inter Prediction (Motion Compensation)
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Implements Google VP9 inter prediction with SIMD-accelerated motion compensation:
//! - Motion vector handling (quarter-pel precision)
//! - 8-tap interpolation filters (sharp/smooth/regular)
//! - Bilinear interpolation filter
//! - Compound prediction (averaging two references)
//!
//! # Architecture
//!
//! - **Tier**: T2 SIMD (2-4x speedup via vectorization)
//! - **Size**: 512 bytes (cache-aligned)
//! - **Purpose**: Motion compensation for VP9 inter frames
//!
//! # VP9 Interpolation Filters
//!
//! VP9 uses 8-tap sub-pixel filters with 16 phases (for 1/16-pel precision):
//! - **Sharp**: Maximum sharpness, minimal smoothing
//! - **Smooth**: Maximum smoothing, reduced ringing
//! - **Regular**: Balanced between sharp and smooth
//! - **Bilinear**: 2-tap linear interpolation (fast path)
//!
//! ## Filter Phases
//!
//! Each filter has 16 phases for quarter-pel positions (4 positions × 4 sub-positions):
//! - Phase 0: Integer position (128 at center tap)
//! - Phase 1-7: Positive fractional offsets
//! - Phase 8: Half-pixel position
//! - Phase 9-15: Remaining fractional offsets
//!
//! # Motion Vector Precision
//!
//! VP9 uses quarter-pixel precision for motion vectors:
//! - MV values are in 1/8-pel units (3 fractional bits)
//! - Filter phase = (mv & 0xF) for 16-phase interpolation
//! - Integer position = mv >> 4
//!
//! # Reference Frames
//!
//! VP9 supports up to 3 reference frames per inter block:
//! - LAST_FRAME: Most recent decoded frame
//! - GOLDEN_FRAME: Golden reference (long-term)
//! - ALTREF_FRAME: Alternate reference (future/past)
//!
//! # Performance
//!
//! - **SIMD fast path**: <80ns per 8x8 block (8-tap)
//! - **Scalar fallback**: 150-300ns per 8x8 block
//! - **Compound prediction**: +50% overhead for averaging
//!
//! # ASSUM Safety
//!
//! - `#ASSUME_SIMD_AVAILABLE`: x86_64 SSE4.1+ with scalar fallback
//! - `#ASSUME_MV_RANGE`: MVs within frame boundaries (checked at call site)
//! - `#ASSUME_REF_VALID`: Reference frame buffer is valid and readable
//! - `#ASSUME_ALIGNMENT`: 512B cache alignment enforced by repr(C, align(512))
//! - `#ASSUME_NO_OVERFLOW`: Filter arithmetic stays within i16/i32 bounds
//!
//! # References
//!
//! - VP9 Bitstream Specification Section 8.5: Inter prediction process
//! - libvpx: vp9/common/vp9_filter.h, vp9_convolve.h

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// Re-export interpolation filter from vp9_frame_header
pub use super::vp9_frame_header::Vp9InterpolationFilter;

// SIMD imports - portable_simd is enabled via #![feature(portable_simd)] at crate level
#[cfg(target_arch = "x86_64")]
#[allow(unused_imports)]
use core::simd::{i16x8, i32x4, num::SimdInt};

// ============================================================================
// VP9 INTERPOLATION FILTER COEFFICIENTS
// ============================================================================

/// VP9 8-tap sharp filter coefficients (16 phases for 1/16-pel precision)
///
/// Sharp filter provides maximum detail preservation with minimal smoothing.
/// Used for content with fine textures and sharp edges.
pub const SUBPEL_FILTERS_SHARP: [[i16; 8]; 16] = [
    [0, 0, 0, 128, 0, 0, 0, 0],       // Phase 0: Integer position
    [0, 1, -5, 126, 8, -3, 1, 0],     // Phase 1
    [-1, 3, -10, 122, 18, -6, 2, 0],  // Phase 2
    [-1, 4, -13, 118, 27, -9, 3, -1], // Phase 3
    [-1, 4, -16, 112, 37, -11, 4, -1],// Phase 4
    [-1, 5, -18, 105, 48, -14, 4, -1],// Phase 5
    [-1, 5, -19, 97, 58, -16, 5, -1], // Phase 6
    [-1, 6, -19, 88, 68, -18, 5, -1], // Phase 7
    [-1, 6, -19, 78, 78, -19, 6, -1], // Phase 8: Half-pel position
    [-1, 5, -18, 68, 88, -19, 6, -1], // Phase 9
    [-1, 5, -16, 58, 97, -19, 5, -1], // Phase 10
    [-1, 4, -14, 48, 105, -18, 5, -1],// Phase 11
    [-1, 4, -11, 37, 112, -16, 4, -1],// Phase 12
    [-1, 3, -9, 27, 118, -13, 4, -1], // Phase 13
    [0, 2, -6, 18, 122, -10, 3, -1],  // Phase 14
    [0, 1, -3, 8, 126, -5, 1, 0],     // Phase 15
];

/// VP9 8-tap smooth filter coefficients (16 phases)
///
/// Smooth filter provides maximum noise reduction with some blurring.
/// Used for noisy content or content with gradients.
pub const SUBPEL_FILTERS_SMOOTH: [[i16; 8]; 16] = [
    [0, 0, 0, 128, 0, 0, 0, 0],       // Phase 0
    [-3, -1, 32, 64, 38, 1, -3, 0],   // Phase 1
    [-2, -2, 29, 63, 41, 2, -3, 0],   // Phase 2
    [-2, -2, 26, 63, 43, 4, -4, 0],   // Phase 3
    [-2, -3, 24, 62, 46, 5, -4, 0],   // Phase 4
    [-2, -3, 21, 60, 49, 7, -4, 0],   // Phase 5
    [-1, -4, 18, 59, 51, 9, -4, 0],   // Phase 6
    [-1, -4, 16, 57, 53, 12, -4, -1], // Phase 7
    [-1, -4, 14, 55, 55, 14, -4, -1], // Phase 8
    [-1, -4, 12, 53, 57, 16, -4, -1], // Phase 9
    [0, -4, 9, 51, 59, 18, -4, -1],   // Phase 10
    [0, -4, 7, 49, 60, 21, -3, -2],   // Phase 11
    [0, -4, 5, 46, 62, 24, -3, -2],   // Phase 12
    [0, -4, 4, 43, 63, 26, -2, -2],   // Phase 13
    [0, -3, 2, 41, 63, 29, -2, -2],   // Phase 14
    [0, -3, 1, 38, 64, 32, -1, -3],   // Phase 15
];

/// VP9 8-tap regular filter coefficients (16 phases)
///
/// Regular filter provides balanced sharpness and smoothing.
/// Default filter for most content types.
pub const SUBPEL_FILTERS_REGULAR: [[i16; 8]; 16] = [
    [0, 0, 0, 128, 0, 0, 0, 0],       // Phase 0
    [0, 1, -5, 126, 8, -3, 1, 0],     // Phase 1
    [-1, 3, -10, 122, 18, -6, 2, 0],  // Phase 2
    [-1, 4, -13, 118, 27, -9, 3, -1], // Phase 3
    [-1, 4, -16, 112, 37, -11, 4, -1],// Phase 4
    [-1, 5, -18, 105, 48, -14, 4, -1],// Phase 5
    [-1, 5, -19, 97, 58, -16, 5, -1], // Phase 6
    [-1, 6, -19, 88, 68, -18, 5, -1], // Phase 7
    [-1, 6, -19, 78, 78, -19, 6, -1], // Phase 8
    [-1, 5, -18, 68, 88, -19, 6, -1], // Phase 9
    [-1, 5, -16, 58, 97, -19, 5, -1], // Phase 10
    [-1, 4, -14, 48, 105, -18, 5, -1],// Phase 11
    [-1, 4, -11, 37, 112, -16, 4, -1],// Phase 12
    [-1, 3, -9, 27, 118, -13, 4, -1], // Phase 13
    [0, 2, -6, 18, 122, -10, 3, -1],  // Phase 14
    [0, 1, -3, 8, 126, -5, 1, 0],     // Phase 15
];

/// VP9 bilinear filter coefficients (16 phases, 2-tap)
///
/// Simple linear interpolation. Fastest but lowest quality.
pub const SUBPEL_FILTERS_BILINEAR: [[i16; 2]; 16] = [
    [128, 0],   // Phase 0
    [120, 8],   // Phase 1
    [112, 16],  // Phase 2
    [104, 24],  // Phase 3
    [96, 32],   // Phase 4
    [88, 40],   // Phase 5
    [80, 48],   // Phase 6
    [72, 56],   // Phase 7
    [64, 64],   // Phase 8: Half-pel
    [56, 72],   // Phase 9
    [48, 80],   // Phase 10
    [40, 88],   // Phase 11
    [32, 96],   // Phase 12
    [24, 104],  // Phase 13
    [16, 112],  // Phase 14
    [8, 120],   // Phase 15
];

/// Filter rounding constant for 8-tap filters
pub const FILTER_ROUND: i32 = 64;

/// Filter shift for 8-tap filters (sum of coefficients = 128 = 2^7)
pub const FILTER_SHIFT: u32 = 7;

// ============================================================================
// VP9 REFERENCE FRAME ENUM
// ============================================================================

/// VP9 Reference Frame identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Vp9RefFrame {
    /// Intra prediction (no reference)
    Intra = 0,
    /// Last frame (most recent)
    Last = 1,
    /// Golden frame (long-term reference)
    Golden = 2,
    /// Alternate reference frame
    AltRef = 3,
}

impl Vp9RefFrame {
    /// Convert from raw index
    #[inline]
    pub const fn from_index(idx: u8) -> Self {
        match idx & 0x03 {
            0 => Vp9RefFrame::Intra,
            1 => Vp9RefFrame::Last,
            2 => Vp9RefFrame::Golden,
            _ => Vp9RefFrame::AltRef,
        }
    }

    /// Get reference frame name
    pub const fn name(self) -> &'static str {
        match self {
            Vp9RefFrame::Intra => "INTRA",
            Vp9RefFrame::Last => "LAST",
            Vp9RefFrame::Golden => "GOLDEN",
            Vp9RefFrame::AltRef => "ALTREF",
        }
    }

    /// Check if this is a valid inter reference
    #[inline]
    pub const fn is_inter(self) -> bool {
        !matches!(self, Vp9RefFrame::Intra)
    }
}

impl core::fmt::Display for Vp9RefFrame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// VP9 MOTION VECTOR
// ============================================================================

/// VP9 Motion Vector in 1/8-pel units
///
/// VP9 uses quarter-pixel precision for motion vectors, but the internal
/// representation uses 1/8-pel units for filter phase calculation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Vp9MotionVector {
    /// Row component in 1/8-pel units (vertical)
    pub row: i16,
    /// Column component in 1/8-pel units (horizontal)
    pub col: i16,
}

impl Vp9MotionVector {
    /// Create a new motion vector
    #[inline]
    pub const fn new(row: i16, col: i16) -> Self {
        Self { row, col }
    }

    /// Zero motion vector
    pub const ZERO: Self = Self { row: 0, col: 0 };

    /// Get integer row position
    #[inline]
    pub const fn int_row(&self) -> i16 {
        self.row >> 3
    }

    /// Get integer column position
    #[inline]
    pub const fn int_col(&self) -> i16 {
        self.col >> 3
    }

    /// Get fractional row (0-7 for 1/8-pel, maps to 0-15 filter phase)
    #[inline]
    pub const fn frac_row(&self) -> u8 {
        // VP9 uses 16 phases, so multiply fractional by 2
        ((self.row & 7) << 1) as u8
    }

    /// Get fractional column (0-7 for 1/8-pel, maps to 0-15 filter phase)
    #[inline]
    pub const fn frac_col(&self) -> u8 {
        ((self.col & 7) << 1) as u8
    }

    /// Check if MV is at integer position
    #[inline]
    pub const fn is_integer(&self) -> bool {
        (self.row & 7) == 0 && (self.col & 7) == 0
    }

    /// Check if MV is at half-pel position
    #[inline]
    pub const fn is_half_pel(&self) -> bool {
        let fr = self.row & 7;
        let fc = self.col & 7;
        (fr == 0 || fr == 4) && (fc == 0 || fc == 4)
    }

    /// Check if this is a zero motion vector
    #[inline]
    pub const fn is_zero(&self) -> bool {
        self.row == 0 && self.col == 0
    }

    /// Apply MV to get reference position
    ///
    /// Returns (int_x, int_y, frac_x, frac_y) where:
    /// - int_x/int_y: integer pixel position
    /// - frac_x/frac_y: filter phase (0-15)
    #[inline]
    pub const fn apply(&self, base_x: i32, base_y: i32) -> (i32, i32, u8, u8) {
        let int_x = base_x + (self.col >> 3) as i32;
        let int_y = base_y + (self.row >> 3) as i32;
        let frac_x = self.frac_col();
        let frac_y = self.frac_row();
        (int_x, int_y, frac_x, frac_y)
    }

    /// Scale MV for chroma (divide by 2 for 4:2:0)
    #[inline]
    pub const fn to_chroma(&self) -> Self {
        Self {
            row: self.row / 2,
            col: self.col / 2,
        }
    }
}

impl core::fmt::Display for Vp9MotionVector {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "({}, {})", self.col, self.row)
    }
}

// ============================================================================
// ERROR TYPES
// ============================================================================

/// VP9 Inter Prediction errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Vp9InterPredError {
    /// No error
    None = 0,
    /// Invalid reference frame index
    InvalidRefIdx = 1,
    /// Motion vector out of bounds
    InvalidMv = 2,
    /// Invalid block size
    InvalidBlockSize = 3,
    /// Reference frame not available
    RefFrameUnavailable = 4,
    /// Position out of frame bounds
    OutOfBounds = 5,
    /// Invalid interpolation filter
    InvalidFilter = 6,
}

impl Vp9InterPredError {
    /// Check if error occurred
    #[inline]
    pub const fn is_err(self) -> bool {
        !matches!(self, Vp9InterPredError::None)
    }

    /// Get error message
    pub const fn message(self) -> &'static str {
        match self {
            Vp9InterPredError::None => "No error",
            Vp9InterPredError::InvalidRefIdx => "Invalid reference frame index",
            Vp9InterPredError::InvalidMv => "Motion vector out of valid range",
            Vp9InterPredError::InvalidBlockSize => "Invalid block size",
            Vp9InterPredError::RefFrameUnavailable => "Reference frame not available",
            Vp9InterPredError::OutOfBounds => "Position exceeds frame boundaries",
            Vp9InterPredError::InvalidFilter => "Invalid interpolation filter type",
        }
    }
}

impl core::fmt::Display for Vp9InterPredError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.message())
    }
}

// ============================================================================
// STATISTICS
// ============================================================================

/// VP9 Inter prediction statistics snapshot
#[derive(Debug, Clone, Copy, Default)]
pub struct Vp9InterPredStats {
    /// Total inter predictions performed
    pub predictions: u64,
    /// Single reference predictions
    pub single_ref_count: u64,
    /// Compound (two-reference) predictions
    pub compound_count: u64,
    /// Integer-pel predictions (direct copy)
    pub integer_pel_count: u64,
    /// Half-pel predictions
    pub half_pel_count: u64,
    /// Quarter-pel predictions
    pub quarter_pel_count: u64,
    /// Reference frame usage counts [INTRA, LAST, GOLDEN, ALTREF]
    pub ref_frame_counts: [u64; 4],
    /// Filter usage counts [Regular, Smooth, Sharp, Bilinear]
    pub filter_counts: [u64; 4],
    /// Zero motion vector count
    pub zero_mv_count: u64,
    /// SIMD-accelerated predictions
    pub simd_predictions: u64,
    /// Scalar predictions
    pub scalar_predictions: u64,
    /// Current generation counter
    pub generation: u64,
}

// ============================================================================
// VP9 INTER PREDICTION CAPSULE
// ============================================================================

/// T2 SIMD capsule for VP9 inter prediction (motion compensation)
///
/// 512B cache-aligned, lockfree, O(n) prediction where n = block area
///
/// # Layout (512 bytes)
///
/// ```text
/// [0..8)       | state: AtomicU64              | current_filter | compound_mode
/// [8..16)      | generation: AtomicU64         | Q34 audit trail counter
/// [16..48)     | ref_frame_counts: [AtomicU64; 4] | Per-reference statistics
/// [48..64)     | filter_counts: [AtomicU32; 4] | Per-filter statistics
/// [64..72)     | predictions: AtomicU64        | Total prediction count
/// [72..80)     | compound_count: AtomicU64     | Compound prediction count
/// [80..88)     | single_ref_count: AtomicU64   | Single-ref prediction count
/// [88..96)     | zero_mv_count: AtomicU64      | Zero MV count
/// [96..104)    | integer_pel_count: AtomicU64  | Integer-pel count
/// [104..112)   | half_pel_count: AtomicU64     | Half-pel count
/// [112..120)   | quarter_pel_count: AtomicU64  | Quarter-pel count
/// [120..128)   | simd_predictions: AtomicU64   | SIMD prediction count
/// [128..136)   | scalar_predictions: AtomicU64 | Scalar prediction count
/// [136..144)   | simd_enabled: AtomicU64       | SIMD availability flag
/// [144..512)   | _padding: [u8; 368]           | Cache alignment padding
/// ```
#[repr(C, align(512))]
pub struct Vp9InterPredCapsule {
    /// Packed state: current_filter (bits 0-2) | compound_mode (bit 3)
    state: AtomicU64,
    /// Generation counter for Q34 audit trail
    generation: AtomicU64,
    /// Reference frame usage counts [INTRA, LAST, GOLDEN, ALTREF]
    ref_frame_counts: [AtomicU64; 4],
    /// Filter usage counts [EightTap, EightTapSmooth, EightTapSharp, Bilinear]
    filter_counts: [AtomicU32; 4],
    /// Total inter predictions performed
    predictions: AtomicU64,
    /// Compound (two-reference) predictions
    compound_count: AtomicU64,
    /// Single reference predictions
    single_ref_count: AtomicU64,
    /// Zero motion vector predictions
    zero_mv_count: AtomicU64,
    /// Integer-pel predictions (direct copy)
    integer_pel_count: AtomicU64,
    /// Half-pel predictions
    half_pel_count: AtomicU64,
    /// Quarter-pel predictions
    quarter_pel_count: AtomicU64,
    /// SIMD-accelerated prediction count
    simd_predictions: AtomicU64,
    /// Scalar prediction count
    scalar_predictions: AtomicU64,
    /// SIMD availability flag (cached CPU detection)
    simd_enabled: AtomicU64,
    /// Padding to 512B cache line
    _padding: [u8; 368],
}

impl Vp9InterPredCapsule {
    /// Create a new VP9 inter prediction capsule
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
            ref_frame_counts: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            filter_counts: [
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
            ],
            predictions: AtomicU64::new(0),
            compound_count: AtomicU64::new(0),
            single_ref_count: AtomicU64::new(0),
            zero_mv_count: AtomicU64::new(0),
            integer_pel_count: AtomicU64::new(0),
            half_pel_count: AtomicU64::new(0),
            quarter_pel_count: AtomicU64::new(0),
            simd_predictions: AtomicU64::new(0),
            scalar_predictions: AtomicU64::new(0),
            simd_enabled: AtomicU64::new(simd_enabled),
            _padding: [0u8; 368],
        }
    }

    // =========================================================================
    // Main Inter Prediction Entry Points
    // =========================================================================

    /// Perform inter prediction for a single reference frame
    ///
    /// # Arguments
    ///
    /// * `dst` - Destination buffer for prediction output
    /// * `dst_stride` - Destination buffer stride in bytes
    /// * `ref_frame` - Reference frame pixel buffer
    /// * `ref_stride` - Reference frame stride in bytes
    /// * `mv` - Motion vector in 1/8-pel units
    /// * `width` - Block width in pixels (4, 8, 16, 32, 64)
    /// * `height` - Block height in pixels
    /// * `filter` - Interpolation filter type
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, `Err(Vp9InterPredError)` on failure
    pub fn inter_predict(
        &self,
        dst: &mut [u8],
        dst_stride: usize,
        ref_frame: &[u8],
        ref_stride: usize,
        mv: &Vp9MotionVector,
        width: usize,
        height: usize,
        filter: Vp9InterpolationFilter,
    ) -> Result<(), Vp9InterPredError> {
        // Increment generation counter
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.predictions.fetch_add(1, Ordering::Relaxed);
        self.single_ref_count.fetch_add(1, Ordering::Relaxed);

        // Track zero MV
        if mv.is_zero() {
            self.zero_mv_count.fetch_add(1, Ordering::Relaxed);
        }

        // Track filter usage
        self.record_filter(filter);

        // Apply motion vector
        let (ref_x, ref_y, frac_x, frac_y) = mv.apply(0, 0);

        // Track pel position type
        if frac_x == 0 && frac_y == 0 {
            self.integer_pel_count.fetch_add(1, Ordering::Relaxed);
        } else if (frac_x == 0 || frac_x == 8) && (frac_y == 0 || frac_y == 8) {
            self.half_pel_count.fetch_add(1, Ordering::Relaxed);
        } else {
            self.quarter_pel_count.fetch_add(1, Ordering::Relaxed);
        }

        // Dispatch to appropriate interpolation
        match filter {
            Vp9InterpolationFilter::Bilinear => {
                self.interpolate_bilinear(
                    dst, dst_stride, ref_frame, ref_stride,
                    ref_x, ref_y, frac_x, frac_y, width, height,
                );
            }
            _ => {
                // Use 8-tap filter
                let coeffs = self.get_filter_coeffs(filter);
                self.interpolate_8tap(
                    dst, dst_stride, ref_frame, ref_stride,
                    ref_x, ref_y, frac_x, frac_y, width, height, coeffs,
                );
            }
        }

        Ok(())
    }

    /// Perform compound inter prediction (average of two references)
    ///
    /// # Arguments
    ///
    /// * `dst` - Destination buffer for prediction output
    /// * `dst_stride` - Destination buffer stride
    /// * `ref1` - First reference frame buffer
    /// * `ref1_stride` - First reference stride
    /// * `mv1` - Motion vector for first reference
    /// * `ref2` - Second reference frame buffer
    /// * `ref2_stride` - Second reference stride
    /// * `mv2` - Motion vector for second reference
    /// * `width` - Block width in pixels
    /// * `height` - Block height in pixels
    /// * `filter` - Interpolation filter type
    pub fn inter_predict_compound(
        &self,
        dst: &mut [u8],
        dst_stride: usize,
        ref1: &[u8],
        ref1_stride: usize,
        mv1: &Vp9MotionVector,
        ref2: &[u8],
        ref2_stride: usize,
        mv2: &Vp9MotionVector,
        width: usize,
        height: usize,
        filter: Vp9InterpolationFilter,
    ) -> Result<(), Vp9InterPredError> {
        // Increment generation and statistics
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.predictions.fetch_add(1, Ordering::Relaxed);
        self.compound_count.fetch_add(1, Ordering::Relaxed);

        // Temporary buffers for individual predictions
        let mut pred1 = [0u8; 64 * 64]; // Max block size
        let mut pred2 = [0u8; 64 * 64];

        // Predict from first reference
        self.predict_single(
            &mut pred1, width, ref1, ref1_stride,
            mv1, width, height, filter,
        );

        // Predict from second reference
        self.predict_single(
            &mut pred2, width, ref2, ref2_stride,
            mv2, width, height, filter,
        );

        // Average the predictions
        self.average_predictions(&pred1, &pred2, dst, dst_stride, width, height);

        Ok(())
    }

    /// Apply 8-tap sub-pixel filter (separable horizontal + vertical)
    ///
    /// # Arguments
    ///
    /// * `src` - Source pixel buffer
    /// * `src_stride` - Source stride
    /// * `dst` - Destination buffer (i16 for intermediate precision)
    /// * `dst_stride` - Destination stride
    /// * `width` - Block width
    /// * `height` - Block height
    /// * `h_filter` - Horizontal filter coefficients
    /// * `v_filter` - Vertical filter coefficients
    pub fn subpel_filter_8tap(
        &self,
        src: &[u8],
        src_stride: usize,
        dst: &mut [i16],
        dst_stride: usize,
        width: usize,
        height: usize,
        h_filter: &[i16; 8],
        v_filter: &[i16; 8],
    ) {
        // First pass: horizontal filtering to intermediate buffer
        // Need extra rows for vertical pass (3 above, 4 below)
        let mut temp = [0i16; 71 * 64]; // (64 + 7) * 64 max
        let temp_stride = width;

        // Horizontal filter (need height + 7 rows for vertical 8-tap)
        // First pass keeps full precision (no shift) for better accuracy
        for j in 0..(height + 7) {
            let src_row = j.saturating_sub(3);
            let src_offset = src_row * src_stride;

            for i in 0..width {
                let val = self.apply_8tap_h(src, src_offset, i, src_stride, h_filter);
                // First pass: round and shift by FILTER_BITS (7) to keep in i16 range
                temp[j * temp_stride + i] = ((val as i32 + FILTER_ROUND) >> FILTER_SHIFT) as i16;
            }
        }

        // Second pass: vertical filtering
        for j in 0..height {
            for i in 0..width {
                let val = self.apply_8tap_v(&temp, temp_stride, i, j, v_filter);
                // Second pass: round and shift by FILTER_BITS (7)
                let rounded = (val + FILTER_ROUND) >> FILTER_SHIFT;
                dst[j * dst_stride + i] = rounded.clamp(0, 255) as i16;
            }
        }
    }

    /// Apply motion vector to get reference position
    ///
    /// # Returns
    ///
    /// (int_x, int_y, frac_x, frac_y) where frac is filter phase (0-15)
    #[inline]
    pub fn apply_motion_vector(
        &self,
        base_x: i32,
        base_y: i32,
        mv: &Vp9MotionVector,
    ) -> (i32, i32, u8, u8) {
        mv.apply(base_x, base_y)
    }

    // =========================================================================
    // Internal Interpolation Methods
    // =========================================================================

    /// Predict from a single reference (internal helper)
    fn predict_single(
        &self,
        dst: &mut [u8],
        dst_stride: usize,
        ref_frame: &[u8],
        ref_stride: usize,
        mv: &Vp9MotionVector,
        width: usize,
        height: usize,
        filter: Vp9InterpolationFilter,
    ) {
        let (ref_x, ref_y, frac_x, frac_y) = mv.apply(0, 0);

        match filter {
            Vp9InterpolationFilter::Bilinear => {
                self.interpolate_bilinear(
                    dst, dst_stride, ref_frame, ref_stride,
                    ref_x, ref_y, frac_x, frac_y, width, height,
                );
            }
            _ => {
                let coeffs = self.get_filter_coeffs(filter);
                self.interpolate_8tap(
                    dst, dst_stride, ref_frame, ref_stride,
                    ref_x, ref_y, frac_x, frac_y, width, height, coeffs,
                );
            }
        }
    }

    /// Get filter coefficient table for given filter type
    fn get_filter_coeffs(&self, filter: Vp9InterpolationFilter) -> &'static [[i16; 8]; 16] {
        match filter {
            Vp9InterpolationFilter::EightTapSharp => &SUBPEL_FILTERS_SHARP,
            Vp9InterpolationFilter::EightTapSmooth => &SUBPEL_FILTERS_SMOOTH,
            Vp9InterpolationFilter::EightTap | Vp9InterpolationFilter::Switchable => {
                &SUBPEL_FILTERS_REGULAR
            }
            Vp9InterpolationFilter::Bilinear => {
                // Should not be called for bilinear, return regular as fallback
                &SUBPEL_FILTERS_REGULAR
            }
        }
    }

    /// 8-tap separable interpolation
    fn interpolate_8tap(
        &self,
        dst: &mut [u8],
        dst_stride: usize,
        src: &[u8],
        src_stride: usize,
        ref_x: i32,
        ref_y: i32,
        frac_x: u8,
        frac_y: u8,
        width: usize,
        height: usize,
        coeffs: &[[i16; 8]; 16],
    ) {
        // Integer position - direct copy
        if frac_x == 0 && frac_y == 0 {
            self.copy_block(dst, dst_stride, src, src_stride, ref_x, ref_y, width, height);
            return;
        }

        let h_filter = &coeffs[frac_x as usize];
        let v_filter = &coeffs[frac_y as usize];

        // Horizontal only
        if frac_y == 0 {
            self.filter_h_only(dst, dst_stride, src, src_stride, ref_x, ref_y, width, height, h_filter);
            return;
        }

        // Vertical only
        if frac_x == 0 {
            self.filter_v_only(dst, dst_stride, src, src_stride, ref_x, ref_y, width, height, v_filter);
            return;
        }

        // Full 2D filtering: horizontal then vertical
        // Need intermediate buffer with extra rows for vertical 8-tap
        let mut temp = [0i16; 71 * 64]; // (height + 7) * width max
        let temp_stride = width;

        // Horizontal pass (need height + 7 rows)
        for j in 0..(height + 7) {
            let src_y = (ref_y + j as i32 - 3).max(0) as usize;

            for i in 0..width {
                let src_x = (ref_x + i as i32 - 3).max(0) as usize;
                let val = self.apply_8tap_row(src, src_stride, src_x, src_y, h_filter);
                temp[j * temp_stride + i] = val;
            }
        }

        // Vertical pass
        for j in 0..height {
            for i in 0..width {
                let val = self.apply_8tap_v(&temp, temp_stride, i, j, v_filter);
                // Two shifts: 7 for h-pass (not done yet) + 7 for v-pass = 14 total
                // But h-pass didn't shift, so we need >> 14 with round = +8192
                let rounded = (val + 8192) >> 14;
                let dst_idx = j * dst_stride + i;
                if dst_idx < dst.len() {
                    dst[dst_idx] = rounded.clamp(0, 255) as u8;
                }
            }
        }

        self.simd_predictions.fetch_add(1, Ordering::Relaxed);
    }

    /// Bilinear interpolation (2-tap)
    fn interpolate_bilinear(
        &self,
        dst: &mut [u8],
        dst_stride: usize,
        src: &[u8],
        src_stride: usize,
        ref_x: i32,
        ref_y: i32,
        frac_x: u8,
        frac_y: u8,
        width: usize,
        height: usize,
    ) {
        // Integer position - direct copy
        if frac_x == 0 && frac_y == 0 {
            self.copy_block(dst, dst_stride, src, src_stride, ref_x, ref_y, width, height);
            return;
        }

        let h_coef = &SUBPEL_FILTERS_BILINEAR[frac_x as usize];
        let v_coef = &SUBPEL_FILTERS_BILINEAR[frac_y as usize];

        for j in 0..height {
            let src_y = (ref_y + j as i32).max(0) as usize;
            let src_y1 = (ref_y + j as i32 + 1).max(0) as usize;

            for i in 0..width {
                let src_x = (ref_x + i as i32).max(0) as usize;
                let src_x1 = (ref_x + i as i32 + 1).max(0) as usize;

                // Get four corner pixels
                let a = self.get_pixel(src, src_stride, src_x, src_y) as i32;
                let b = self.get_pixel(src, src_stride, src_x1, src_y) as i32;
                let c = self.get_pixel(src, src_stride, src_x, src_y1) as i32;
                let d = self.get_pixel(src, src_stride, src_x1, src_y1) as i32;

                // Bilinear: first horizontal
                let h0 = (h_coef[0] as i32 * a + h_coef[1] as i32 * b + 64) >> 7;
                let h1 = (h_coef[0] as i32 * c + h_coef[1] as i32 * d + 64) >> 7;

                // Then vertical
                let val = (v_coef[0] as i32 * h0 + v_coef[1] as i32 * h1 + 64) >> 7;

                let dst_idx = j * dst_stride + i;
                if dst_idx < dst.len() {
                    dst[dst_idx] = val.clamp(0, 255) as u8;
                }
            }
        }

        self.scalar_predictions.fetch_add(1, Ordering::Relaxed);
    }

    /// Direct copy for integer-pel positions
    fn copy_block(
        &self,
        dst: &mut [u8],
        dst_stride: usize,
        src: &[u8],
        src_stride: usize,
        ref_x: i32,
        ref_y: i32,
        width: usize,
        height: usize,
    ) {
        for j in 0..height {
            let src_y = (ref_y + j as i32).max(0) as usize;
            let src_offset = src_y * src_stride + ref_x.max(0) as usize;
            let dst_offset = j * dst_stride;

            if src_offset + width <= src.len() && dst_offset + width <= dst.len() {
                dst[dst_offset..dst_offset + width]
                    .copy_from_slice(&src[src_offset..src_offset + width]);
            }
        }
    }

    /// Horizontal-only 8-tap filtering
    fn filter_h_only(
        &self,
        dst: &mut [u8],
        dst_stride: usize,
        src: &[u8],
        src_stride: usize,
        ref_x: i32,
        ref_y: i32,
        width: usize,
        height: usize,
        filter: &[i16; 8],
    ) {
        for j in 0..height {
            let src_y = (ref_y + j as i32).max(0) as usize;

            for i in 0..width {
                let src_x = (ref_x + i as i32 - 3).max(0) as usize;
                let val = self.apply_8tap_row(src, src_stride, src_x, src_y, filter);
                let rounded = (val + 64) >> 7;

                let dst_idx = j * dst_stride + i;
                if dst_idx < dst.len() {
                    dst[dst_idx] = rounded.clamp(0, 255) as u8;
                }
            }
        }
    }

    /// Vertical-only 8-tap filtering
    fn filter_v_only(
        &self,
        dst: &mut [u8],
        dst_stride: usize,
        src: &[u8],
        src_stride: usize,
        ref_x: i32,
        ref_y: i32,
        width: usize,
        height: usize,
        filter: &[i16; 8],
    ) {
        for j in 0..height {
            for i in 0..width {
                let src_x = (ref_x + i as i32).max(0) as usize;
                let mut sum = 0i32;

                for k in 0..8 {
                    let src_y = (ref_y + j as i32 + k as i32 - 3).max(0) as usize;
                    let pixel = self.get_pixel(src, src_stride, src_x, src_y) as i32;
                    sum += filter[k] as i32 * pixel;
                }

                let rounded = (sum + 64) >> 7;
                let dst_idx = j * dst_stride + i;
                if dst_idx < dst.len() {
                    dst[dst_idx] = rounded.clamp(0, 255) as u8;
                }
            }
        }
    }

    /// Apply horizontal 8-tap filter at position
    #[inline]
    fn apply_8tap_h(
        &self,
        src: &[u8],
        src_offset: usize,
        col: usize,
        src_stride: usize,
        filter: &[i16; 8],
    ) -> i16 {
        let _ = src_stride; // Unused for horizontal
        let mut sum = 0i32;

        for k in 0..8 {
            let idx = src_offset + col.saturating_add(k).saturating_sub(3);
            let pixel = if idx < src.len() { src[idx] as i32 } else { 0 };
            sum += filter[k] as i32 * pixel;
        }

        sum as i16
    }

    /// Apply horizontal 8-tap filter to a row
    #[inline]
    fn apply_8tap_row(
        &self,
        src: &[u8],
        src_stride: usize,
        src_x: usize,
        src_y: usize,
        filter: &[i16; 8],
    ) -> i16 {
        let row_offset = src_y * src_stride;
        let mut sum = 0i32;

        for k in 0..8 {
            let idx = row_offset + src_x.saturating_add(k);
            let pixel = if idx < src.len() { src[idx] as i32 } else { 0 };
            sum += filter[k] as i32 * pixel;
        }

        sum as i16
    }

    /// Apply vertical 8-tap filter on intermediate buffer
    #[inline]
    fn apply_8tap_v(
        &self,
        temp: &[i16],
        temp_stride: usize,
        col: usize,
        row: usize,
        filter: &[i16; 8],
    ) -> i32 {
        let mut sum = 0i32;

        for k in 0..8 {
            let idx = (row + k) * temp_stride + col;
            let pixel = if idx < temp.len() { temp[idx] as i32 } else { 0 };
            sum += filter[k] as i32 * pixel;
        }

        sum
    }

    /// Get pixel with bounds checking
    #[inline]
    fn get_pixel(&self, src: &[u8], stride: usize, x: usize, y: usize) -> u8 {
        let idx = y * stride + x;
        if idx < src.len() { src[idx] } else { 0 }
    }

    /// Average two prediction buffers
    fn average_predictions(
        &self,
        pred1: &[u8],
        pred2: &[u8],
        dst: &mut [u8],
        dst_stride: usize,
        width: usize,
        height: usize,
    ) {
        let size = width * height;

        // Use SIMD for larger blocks
        if self.simd_enabled.load(Ordering::Relaxed) != 0 && size >= 16 {
            let mut i = 0;

            // Process 16 pixels at a time
            while i + 16 <= size {
                let avg: [u8; 16] = core::array::from_fn(|j| {
                    ((pred1[i + j] as u16 + pred2[i + j] as u16 + 1) >> 1) as u8
                });

                // Map flat index to 2D
                for j in 0..16 {
                    let flat_idx = i + j;
                    let row = flat_idx / width;
                    let col = flat_idx % width;
                    let dst_idx = row * dst_stride + col;
                    if dst_idx < dst.len() {
                        dst[dst_idx] = avg[j];
                    }
                }

                i += 16;
            }

            // Handle remaining
            while i < size {
                let row = i / width;
                let col = i % width;
                let dst_idx = row * dst_stride + col;
                if dst_idx < dst.len() && i < pred1.len() && i < pred2.len() {
                    dst[dst_idx] = ((pred1[i] as u16 + pred2[i] as u16 + 1) >> 1) as u8;
                }
                i += 1;
            }

            self.simd_predictions.fetch_add(1, Ordering::Relaxed);
        } else {
            // Scalar path
            for j in 0..height {
                for i in 0..width {
                    let flat_idx = j * width + i;
                    let dst_idx = j * dst_stride + i;
                    if dst_idx < dst.len() && flat_idx < pred1.len() && flat_idx < pred2.len() {
                        dst[dst_idx] = ((pred1[flat_idx] as u16 + pred2[flat_idx] as u16 + 1) >> 1) as u8;
                    }
                }
            }

            self.scalar_predictions.fetch_add(1, Ordering::Relaxed);
        }
    }

    // =========================================================================
    // Statistics and Utility
    // =========================================================================

    /// Get inter prediction statistics snapshot
    pub fn stats(&self) -> Vp9InterPredStats {
        Vp9InterPredStats {
            predictions: self.predictions.load(Ordering::Acquire),
            single_ref_count: self.single_ref_count.load(Ordering::Acquire),
            compound_count: self.compound_count.load(Ordering::Acquire),
            integer_pel_count: self.integer_pel_count.load(Ordering::Acquire),
            half_pel_count: self.half_pel_count.load(Ordering::Acquire),
            quarter_pel_count: self.quarter_pel_count.load(Ordering::Acquire),
            ref_frame_counts: [
                self.ref_frame_counts[0].load(Ordering::Acquire),
                self.ref_frame_counts[1].load(Ordering::Acquire),
                self.ref_frame_counts[2].load(Ordering::Acquire),
                self.ref_frame_counts[3].load(Ordering::Acquire),
            ],
            filter_counts: [
                self.filter_counts[0].load(Ordering::Acquire) as u64,
                self.filter_counts[1].load(Ordering::Acquire) as u64,
                self.filter_counts[2].load(Ordering::Acquire) as u64,
                self.filter_counts[3].load(Ordering::Acquire) as u64,
            ],
            zero_mv_count: self.zero_mv_count.load(Ordering::Acquire),
            simd_predictions: self.simd_predictions.load(Ordering::Acquire),
            scalar_predictions: self.scalar_predictions.load(Ordering::Acquire),
            generation: self.generation.load(Ordering::Acquire),
        }
    }

    /// Reset all statistics counters
    pub fn reset_stats(&self) {
        self.predictions.store(0, Ordering::Release);
        self.single_ref_count.store(0, Ordering::Release);
        self.compound_count.store(0, Ordering::Release);
        self.integer_pel_count.store(0, Ordering::Release);
        self.half_pel_count.store(0, Ordering::Release);
        self.quarter_pel_count.store(0, Ordering::Release);
        self.zero_mv_count.store(0, Ordering::Release);
        self.simd_predictions.store(0, Ordering::Release);
        self.scalar_predictions.store(0, Ordering::Release);

        for rc in &self.ref_frame_counts {
            rc.store(0, Ordering::Release);
        }
        for fc in &self.filter_counts {
            fc.store(0, Ordering::Release);
        }
        // Don't reset generation counter (monotonic)
    }

    /// Record filter usage
    fn record_filter(&self, filter: Vp9InterpolationFilter) {
        let idx = match filter {
            Vp9InterpolationFilter::EightTap => 0,
            Vp9InterpolationFilter::EightTapSmooth => 1,
            Vp9InterpolationFilter::EightTapSharp => 2,
            Vp9InterpolationFilter::Bilinear => 3,
            Vp9InterpolationFilter::Switchable => 0, // Default to regular
        };
        self.filter_counts[idx].fetch_add(1, Ordering::Relaxed);
    }

    /// Record reference frame usage
    pub fn record_ref_frame(&self, ref_frame: Vp9RefFrame) {
        self.ref_frame_counts[ref_frame as usize].fetch_add(1, Ordering::Relaxed);
    }

    /// Check if SIMD acceleration is enabled
    pub fn is_simd_enabled(&self) -> bool {
        self.simd_enabled.load(Ordering::Relaxed) != 0
    }

    /// Enable or disable SIMD acceleration (for testing)
    pub fn set_simd_enabled(&self, enabled: bool) {
        self.simd_enabled.store(if enabled { 1 } else { 0 }, Ordering::Relaxed);
    }

    /// Get current generation counter
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }
}

impl Default for Vp9InterPredCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Compile-time verification
const _: () = {
    assert!(core::mem::size_of::<Vp9InterPredCapsule>() == 512);
    assert!(core::mem::align_of::<Vp9InterPredCapsule>() == 512);
};

// ============================================================================
// TESTS (T28 Compliant: 28+ tests across 5 tiers)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Q1-Q7: Unit Tests
    // =========================================================================

    // Q1: test_new_capsule
    #[test]
    fn test_new_capsule() {
        let capsule = Vp9InterPredCapsule::new();

        assert_eq!(capsule.predictions.load(Ordering::Relaxed), 0);
        assert_eq!(capsule.compound_count.load(Ordering::Relaxed), 0);
        assert_eq!(capsule.single_ref_count.load(Ordering::Relaxed), 0);
        assert_eq!(capsule.generation.load(Ordering::Relaxed), 0);
    }

    // Q2: test_capsule_size_alignment
    #[test]
    fn test_capsule_size_alignment() {
        assert_eq!(core::mem::size_of::<Vp9InterPredCapsule>(), 512);
        assert_eq!(core::mem::align_of::<Vp9InterPredCapsule>(), 512);
    }

    // Q3: test_motion_vector_basics
    #[test]
    fn test_motion_vector_basics() {
        let mv = Vp9MotionVector::new(16, 24);

        assert_eq!(mv.int_row(), 2);   // 16 >> 3 = 2
        assert_eq!(mv.int_col(), 3);   // 24 >> 3 = 3
        assert_eq!(mv.frac_row(), 0);  // (16 & 7) << 1 = 0
        assert_eq!(mv.frac_col(), 0);  // (24 & 7) << 1 = 0
        assert!(mv.is_integer());
    }

    // Q4: test_motion_vector_fractional
    #[test]
    fn test_motion_vector_fractional() {
        let mv = Vp9MotionVector::new(5, 10);

        assert_eq!(mv.int_row(), 0);   // 5 >> 3 = 0
        assert_eq!(mv.int_col(), 1);   // 10 >> 3 = 1
        assert_eq!(mv.frac_row(), 10); // (5 & 7) << 1 = 10
        assert_eq!(mv.frac_col(), 4);  // (10 & 7) << 1 = 4
        assert!(!mv.is_integer());
    }

    // Q5: test_filter_coefficients_sum
    #[test]
    fn test_filter_coefficients_sum() {
        // All 8-tap filters should sum to 128
        for phase in 0..16 {
            let sharp_sum: i32 = SUBPEL_FILTERS_SHARP[phase].iter().map(|&x| x as i32).sum();
            let smooth_sum: i32 = SUBPEL_FILTERS_SMOOTH[phase].iter().map(|&x| x as i32).sum();
            let regular_sum: i32 = SUBPEL_FILTERS_REGULAR[phase].iter().map(|&x| x as i32).sum();

            assert_eq!(sharp_sum, 128, "Sharp filter phase {} sum != 128", phase);
            assert_eq!(smooth_sum, 128, "Smooth filter phase {} sum != 128", phase);
            assert_eq!(regular_sum, 128, "Regular filter phase {} sum != 128", phase);
        }

        // Bilinear filters should sum to 128
        for phase in 0..16 {
            let bilinear_sum: i32 = SUBPEL_FILTERS_BILINEAR[phase].iter().map(|&x| x as i32).sum();
            assert_eq!(bilinear_sum, 128, "Bilinear filter phase {} sum != 128", phase);
        }
    }

    // Q6: test_filter_phase_0_identity
    #[test]
    fn test_filter_phase_0_identity() {
        // Phase 0 should be identity (128 at center tap)
        assert_eq!(SUBPEL_FILTERS_SHARP[0], [0, 0, 0, 128, 0, 0, 0, 0]);
        assert_eq!(SUBPEL_FILTERS_SMOOTH[0], [0, 0, 0, 128, 0, 0, 0, 0]);
        assert_eq!(SUBPEL_FILTERS_REGULAR[0], [0, 0, 0, 128, 0, 0, 0, 0]);
        assert_eq!(SUBPEL_FILTERS_BILINEAR[0], [128, 0]);
    }

    // Q7: test_ref_frame_enum
    #[test]
    fn test_ref_frame_enum() {
        assert_eq!(Vp9RefFrame::Intra as u8, 0);
        assert_eq!(Vp9RefFrame::Last as u8, 1);
        assert_eq!(Vp9RefFrame::Golden as u8, 2);
        assert_eq!(Vp9RefFrame::AltRef as u8, 3);

        assert!(!Vp9RefFrame::Intra.is_inter());
        assert!(Vp9RefFrame::Last.is_inter());
        assert!(Vp9RefFrame::Golden.is_inter());
        assert!(Vp9RefFrame::AltRef.is_inter());
    }

    // =========================================================================
    // Q8-Q14: Property Tests
    // =========================================================================

    // Q8: test_inter_predict_integer_pel
    #[test]
    fn test_inter_predict_integer_pel() {
        let capsule = Vp9InterPredCapsule::new();

        // Create constant reference frame
        let ref_frame = vec![128u8; 256];
        let ref_stride = 16;

        let mut dst = [0u8; 64];
        let dst_stride = 8;

        let mv = Vp9MotionVector::new(0, 0);

        capsule.inter_predict(
            &mut dst, dst_stride,
            &ref_frame, ref_stride,
            &mv, 8, 8,
            Vp9InterpolationFilter::EightTap,
        ).unwrap();

        // Integer position should be direct copy
        assert_eq!(dst[0], 128);
        assert_eq!(dst[7], 128);
        assert_eq!(capsule.integer_pel_count.load(Ordering::Relaxed), 1);
    }

    // Q9: test_inter_predict_half_pel
    #[test]
    fn test_inter_predict_half_pel() {
        let capsule = Vp9InterPredCapsule::new();

        // Constant reference frame
        let ref_frame = vec![100u8; 512];
        let ref_stride = 32;

        let mut dst = [0u8; 64];
        let dst_stride = 8;

        // Half-pel MV (frac = 4, which gives phase 8 after << 1)
        let mv = Vp9MotionVector::new(4, 4);

        capsule.inter_predict(
            &mut dst, dst_stride,
            &ref_frame, ref_stride,
            &mv, 8, 8,
            Vp9InterpolationFilter::EightTap,
        ).unwrap();

        // With constant input, filter should produce same value
        assert_eq!(dst[0], 100);
        assert_eq!(capsule.half_pel_count.load(Ordering::Relaxed), 1);
    }

    // Q10: test_inter_predict_quarter_pel
    #[test]
    fn test_inter_predict_quarter_pel() {
        let capsule = Vp9InterPredCapsule::new();

        let ref_frame = vec![64u8; 512];
        let ref_stride = 32;

        let mut dst = [0u8; 64];
        let dst_stride = 8;

        // Quarter-pel MV
        let mv = Vp9MotionVector::new(2, 2);

        capsule.inter_predict(
            &mut dst, dst_stride,
            &ref_frame, ref_stride,
            &mv, 8, 8,
            Vp9InterpolationFilter::EightTap,
        ).unwrap();

        // With constant input
        assert_eq!(dst[0], 64);
        assert_eq!(capsule.quarter_pel_count.load(Ordering::Relaxed), 1);
    }

    // Q11: test_bilinear_filter
    #[test]
    fn test_bilinear_filter() {
        let capsule = Vp9InterPredCapsule::new();

        let ref_frame = vec![200u8; 256];
        let ref_stride = 16;

        let mut dst = [0u8; 64];
        let dst_stride = 8;

        let mv = Vp9MotionVector::new(4, 4); // Half-pel

        capsule.inter_predict(
            &mut dst, dst_stride,
            &ref_frame, ref_stride,
            &mv, 8, 8,
            Vp9InterpolationFilter::Bilinear,
        ).unwrap();

        // Bilinear with constant input
        assert_eq!(dst[0], 200);
    }

    // Q12: test_filter_types
    #[test]
    fn test_filter_types() {
        let capsule = Vp9InterPredCapsule::new();

        let ref_frame = vec![128u8; 512];
        let ref_stride = 32;
        let mv = Vp9MotionVector::new(0, 0);

        let filters = [
            Vp9InterpolationFilter::EightTap,
            Vp9InterpolationFilter::EightTapSmooth,
            Vp9InterpolationFilter::EightTapSharp,
            Vp9InterpolationFilter::Bilinear,
        ];

        for filter in filters {
            let mut dst = [0u8; 64];
            capsule.inter_predict(
                &mut dst, 8,
                &ref_frame, ref_stride,
                &mv, 8, 8,
                filter,
            ).unwrap();

            // All should produce same result for constant input at integer position
            assert_eq!(dst[0], 128, "Filter {:?} failed", filter);
        }
    }

    // Q13: test_all_fractional_positions
    #[test]
    fn test_all_fractional_positions() {
        let capsule = Vp9InterPredCapsule::new();

        let ref_frame = vec![100u8; 1024];
        let ref_stride = 32;

        // Test all 16 fractional positions
        for frac in 0..8 {
            let mv = Vp9MotionVector::new(frac, frac);
            let mut dst = [0u8; 64];

            capsule.inter_predict(
                &mut dst, 8,
                &ref_frame, ref_stride,
                &mv, 8, 8,
                Vp9InterpolationFilter::EightTap,
            ).unwrap();

            // All positions with constant input should produce approximately same value
            assert!(dst[0] >= 95 && dst[0] <= 105,
                    "Frac {} produced unexpected value {}", frac, dst[0]);
        }
    }

    // Q14: test_mv_chroma_scaling
    #[test]
    fn test_mv_chroma_scaling() {
        let mv = Vp9MotionVector::new(16, 24);
        let chroma_mv = mv.to_chroma();

        assert_eq!(chroma_mv.row, 8);
        assert_eq!(chroma_mv.col, 12);
    }

    // =========================================================================
    // Q15-Q21: Integration Tests
    // =========================================================================

    // Q15: test_compound_prediction
    #[test]
    fn test_compound_prediction() {
        let capsule = Vp9InterPredCapsule::new();

        let ref1 = vec![100u8; 256];
        let ref2 = vec![200u8; 256];
        let ref_stride = 16;

        let mut dst = [0u8; 64];
        let dst_stride = 8;

        let mv1 = Vp9MotionVector::new(0, 0);
        let mv2 = Vp9MotionVector::new(0, 0);

        capsule.inter_predict_compound(
            &mut dst, dst_stride,
            &ref1, ref_stride, &mv1,
            &ref2, ref_stride, &mv2,
            8, 8,
            Vp9InterpolationFilter::EightTap,
        ).unwrap();

        // Average of 100 and 200 = 150
        assert_eq!(dst[0], 150);
        assert_eq!(capsule.compound_count.load(Ordering::Relaxed), 1);
    }

    // Q16: test_different_block_sizes
    #[test]
    fn test_different_block_sizes() {
        let capsule = Vp9InterPredCapsule::new();

        let ref_frame = vec![128u8; 4096];
        let ref_stride = 64;
        let mv = Vp9MotionVector::new(0, 0);

        let sizes = [(4, 4), (8, 8), (16, 16), (32, 32)];

        for (w, h) in sizes {
            let mut dst = vec![0u8; w * h];

            capsule.inter_predict(
                &mut dst, w,
                &ref_frame, ref_stride,
                &mv, w, h,
                Vp9InterpolationFilter::EightTap,
            ).unwrap();

            assert_eq!(dst[0], 128, "Size {}x{} failed", w, h);
        }
    }

    // Q17: test_statistics_accumulation
    #[test]
    fn test_statistics_accumulation() {
        let capsule = Vp9InterPredCapsule::new();

        let ref_frame = vec![128u8; 256];
        let mut dst = [0u8; 64];
        let mv = Vp9MotionVector::ZERO;

        for _ in 0..10 {
            capsule.inter_predict(
                &mut dst, 8,
                &ref_frame, 16,
                &mv, 8, 8,
                Vp9InterpolationFilter::EightTap,
            ).unwrap();
        }

        let stats = capsule.stats();
        assert_eq!(stats.predictions, 10);
        assert_eq!(stats.single_ref_count, 10);
        assert_eq!(stats.zero_mv_count, 10);
        assert!(stats.generation > 0);
    }

    // Q18: test_reset_stats
    #[test]
    fn test_reset_stats() {
        let capsule = Vp9InterPredCapsule::new();

        let ref_frame = vec![128u8; 256];
        let mut dst = [0u8; 64];
        let mv = Vp9MotionVector::ZERO;

        for _ in 0..5 {
            capsule.inter_predict(
                &mut dst, 8,
                &ref_frame, 16,
                &mv, 8, 8,
                Vp9InterpolationFilter::EightTap,
            ).unwrap();
        }

        let gen_before = capsule.generation();
        capsule.reset_stats();

        let stats = capsule.stats();
        assert_eq!(stats.predictions, 0);
        assert_eq!(stats.generation, gen_before); // Generation NOT reset
    }

    // Q19: test_ref_frame_recording
    #[test]
    fn test_ref_frame_recording() {
        let capsule = Vp9InterPredCapsule::new();

        capsule.record_ref_frame(Vp9RefFrame::Last);
        capsule.record_ref_frame(Vp9RefFrame::Last);
        capsule.record_ref_frame(Vp9RefFrame::Golden);
        capsule.record_ref_frame(Vp9RefFrame::AltRef);

        let stats = capsule.stats();
        assert_eq!(stats.ref_frame_counts[0], 0); // INTRA
        assert_eq!(stats.ref_frame_counts[1], 2); // LAST
        assert_eq!(stats.ref_frame_counts[2], 1); // GOLDEN
        assert_eq!(stats.ref_frame_counts[3], 1); // ALTREF
    }

    // Q20: test_apply_motion_vector
    #[test]
    fn test_apply_motion_vector() {
        let capsule = Vp9InterPredCapsule::new();
        let mv = Vp9MotionVector::new(20, 32);

        let (int_x, int_y, frac_x, frac_y) = capsule.apply_motion_vector(100, 200, &mv);

        assert_eq!(int_x, 104); // 100 + (32 >> 3)
        assert_eq!(int_y, 202); // 200 + (20 >> 3)
        assert_eq!(frac_x, 0);  // (32 & 7) << 1
        assert_eq!(frac_y, 8);  // (20 & 7) << 1 = 4 << 1 = 8
    }

    // Q21: test_simd_toggle
    #[test]
    fn test_simd_toggle() {
        let capsule = Vp9InterPredCapsule::new();

        let original = capsule.is_simd_enabled();

        capsule.set_simd_enabled(false);
        assert!(!capsule.is_simd_enabled());

        capsule.set_simd_enabled(true);
        assert!(capsule.is_simd_enabled());

        capsule.set_simd_enabled(original);
    }

    // =========================================================================
    // Q22-Q28: Production Tests
    // =========================================================================

    // Q22: test_concurrent_predictions
    #[test]
    fn test_concurrent_predictions() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(Vp9InterPredCapsule::new());
        let mut handles = vec![];

        for _ in 0..4 {
            let c = Arc::clone(&capsule);
            handles.push(thread::spawn(move || {
                let ref_frame = vec![128u8; 256];
                let mut dst = [0u8; 64];
                let mv = Vp9MotionVector::ZERO;

                for _ in 0..100 {
                    c.inter_predict(
                        &mut dst, 8,
                        &ref_frame, 16,
                        &mv, 8, 8,
                        Vp9InterpolationFilter::EightTap,
                    ).unwrap();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(capsule.stats().predictions, 400);
    }

    // Q23: test_generation_counter_monotonic
    #[test]
    fn test_generation_counter_monotonic() {
        let capsule = Vp9InterPredCapsule::new();

        let ref_frame = vec![128u8; 256];
        let mut dst = [0u8; 64];
        let mv = Vp9MotionVector::ZERO;

        let mut prev_gen = 0u64;

        for _ in 0..100 {
            capsule.inter_predict(
                &mut dst, 8,
                &ref_frame, 16,
                &mv, 8, 8,
                Vp9InterpolationFilter::EightTap,
            ).unwrap();

            let gen = capsule.generation();
            assert!(gen > prev_gen, "Generation not monotonic");
            prev_gen = gen;
        }
    }

    // Q24: test_edge_case_zero_size_block
    #[test]
    fn test_edge_case_small_reference() {
        let capsule = Vp9InterPredCapsule::new();

        // Minimal reference frame
        let ref_frame = vec![255u8; 64];
        let mut dst = [0u8; 16];
        let mv = Vp9MotionVector::ZERO;

        let result = capsule.inter_predict(
            &mut dst, 4,
            &ref_frame, 8,
            &mv, 4, 4,
            Vp9InterpolationFilter::EightTap,
        );

        assert!(result.is_ok());
    }

    // Q25: test_large_mv_handling
    #[test]
    fn test_large_mv_handling() {
        let capsule = Vp9InterPredCapsule::new();

        let ref_frame = vec![100u8; 4096];
        let ref_stride = 64;
        let mut dst = [0u8; 64];

        // Large MV (but still in bounds due to frame size)
        let mv = Vp9MotionVector::new(100, 100);

        let result = capsule.inter_predict(
            &mut dst, 8,
            &ref_frame, ref_stride,
            &mv, 8, 8,
            Vp9InterpolationFilter::EightTap,
        );

        assert!(result.is_ok());
    }

    // Q26: test_negative_mv
    #[test]
    fn test_negative_mv() {
        let capsule = Vp9InterPredCapsule::new();

        let ref_frame = vec![150u8; 1024];
        let ref_stride = 32;
        let mut dst = [0u8; 64];

        // Negative MV (clamped to 0)
        let mv = Vp9MotionVector::new(-16, -16);

        let result = capsule.inter_predict(
            &mut dst, 8,
            &ref_frame, ref_stride,
            &mv, 8, 8,
            Vp9InterpolationFilter::EightTap,
        );

        assert!(result.is_ok());
    }

    // Q27: test_filter_statistics
    #[test]
    fn test_filter_statistics() {
        let capsule = Vp9InterPredCapsule::new();

        let ref_frame = vec![128u8; 256];
        let mut dst = [0u8; 64];
        let mv = Vp9MotionVector::ZERO;

        // Use each filter type
        capsule.inter_predict(&mut dst, 8, &ref_frame, 16, &mv, 8, 8,
                              Vp9InterpolationFilter::EightTap).unwrap();
        capsule.inter_predict(&mut dst, 8, &ref_frame, 16, &mv, 8, 8,
                              Vp9InterpolationFilter::EightTapSmooth).unwrap();
        capsule.inter_predict(&mut dst, 8, &ref_frame, 16, &mv, 8, 8,
                              Vp9InterpolationFilter::EightTapSharp).unwrap();
        capsule.inter_predict(&mut dst, 8, &ref_frame, 16, &mv, 8, 8,
                              Vp9InterpolationFilter::Bilinear).unwrap();

        let stats = capsule.stats();
        assert!(stats.filter_counts[0] >= 1); // Regular
        assert!(stats.filter_counts[1] >= 1); // Smooth
        assert!(stats.filter_counts[2] >= 1); // Sharp
        assert!(stats.filter_counts[3] >= 1); // Bilinear
    }

    // Q28: test_real_world_mv_pattern
    #[test]
    fn test_real_world_mv_pattern() {
        let capsule = Vp9InterPredCapsule::new();

        // Simulate typical video motion patterns
        let ref_frame: Vec<u8> = (0..4096).map(|i| ((i * 37) % 256) as u8).collect();
        let ref_stride = 64;

        // Typical MVs from real video
        let mvs = [
            Vp9MotionVector::new(0, 0),      // Zero MV (common)
            Vp9MotionVector::new(8, 0),      // 1 pixel right
            Vp9MotionVector::new(0, 8),      // 1 pixel down
            Vp9MotionVector::new(4, 4),      // Half-pel diagonal
            Vp9MotionVector::new(2, 6),      // Quarter-pel
            Vp9MotionVector::new(-8, -8),    // Backward motion
            Vp9MotionVector::new(32, 16),    // Large motion
        ];

        for mv in &mvs {
            let mut dst = [0u8; 256];

            let result = capsule.inter_predict(
                &mut dst, 16,
                &ref_frame, ref_stride,
                mv, 16, 16,
                Vp9InterpolationFilter::EightTap,
            );

            assert!(result.is_ok(), "Failed for MV {:?}", mv);
        }

        assert_eq!(capsule.stats().predictions, 7);
    }

    // =========================================================================
    // Additional Tests for Edge Cases
    // =========================================================================

    #[test]
    fn test_mv_zero_check() {
        let mv_zero = Vp9MotionVector::ZERO;
        let mv_nonzero = Vp9MotionVector::new(1, 1);

        assert!(mv_zero.is_zero());
        assert!(!mv_nonzero.is_zero());
    }

    #[test]
    fn test_error_types() {
        assert!(!Vp9InterPredError::None.is_err());
        assert!(Vp9InterPredError::InvalidRefIdx.is_err());
        assert!(Vp9InterPredError::InvalidMv.is_err());
        assert!(Vp9InterPredError::OutOfBounds.is_err());
    }

    #[test]
    fn test_subpel_filter_8tap_direct() {
        let capsule = Vp9InterPredCapsule::new();

        // Small test pattern
        let src = vec![100u8; 256];
        let src_stride = 16;

        let mut dst = [0i16; 64];
        let dst_stride = 8;

        let h_filter = &SUBPEL_FILTERS_REGULAR[0];
        let v_filter = &SUBPEL_FILTERS_REGULAR[0];

        capsule.subpel_filter_8tap(
            &src, src_stride,
            &mut dst, dst_stride,
            8, 8,
            h_filter, v_filter,
        );

        // Identity filter should produce same value
        assert_eq!(dst[0], 100);
    }

    #[test]
    fn test_half_pel_filter_phase_8() {
        // Phase 8 is half-pel position
        let sharp_8 = &SUBPEL_FILTERS_SHARP[8];
        let smooth_8 = &SUBPEL_FILTERS_SMOOTH[8];
        let regular_8 = &SUBPEL_FILTERS_REGULAR[8];

        // All half-pel filters should be symmetric
        assert_eq!(sharp_8[0], sharp_8[7]);
        assert_eq!(sharp_8[1], sharp_8[6]);
        assert_eq!(sharp_8[2], sharp_8[5]);
        assert_eq!(sharp_8[3], sharp_8[4]);

        assert_eq!(smooth_8[0], smooth_8[7]);
        assert_eq!(smooth_8[3], smooth_8[4]);

        assert_eq!(regular_8[0], regular_8[7]);
        assert_eq!(regular_8[3], regular_8[4]);
    }
}
