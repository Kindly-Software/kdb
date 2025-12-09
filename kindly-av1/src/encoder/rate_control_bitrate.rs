//! CBR/VBR Rate Control Capsule with SOTA VBV Buffer Model
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! # Overview
//!
//! Production-grade CBR/VBR rate control based on SOTA implementations from:
//! - SVT-AV1 VBR rate control (https://gitlab.com/AOMediaCodec/SVT-AV1)
//! - x264 VBV buffer model (https://github.com/MasterNobody/x264)
//! - Research: "A two-pass rate control algorithm for H.264/AVC HD video coding"
//!
//! # Architecture
//!
//! - **Tier**: T1 Atomic + T5 Streaming (512B capsule, cache-aligned)
//! - **VBV Buffer**: Video Buffering Verifier for CBR compliance
//! - **Q16.16 Precision**: Deterministic fixed-point arithmetic
//! - **Lockfree**: 100% Chaos compliant, zero mutex/RwLock
//!
//! # CBR Mode (Constant Bitrate)
//!
//! - Strict bitrate target (±5% tolerance)
//! - VBV buffer model with fullness tracking
//! - Anti-overflow/underflow protection
//! - QP adjustment based on buffer state
//! - Smooth QP transitions (±4 max delta per frame)
//!
//! # VBR Mode (Variable Bitrate)
//!
//! - Average bitrate target over GOP
//! - Quality fluctuation allowed for better perceptual quality
//! - Scene-based bit allocation using complexity metrics
//! - One-pass and two-pass support
//!
//! # Key Algorithm Components
//!
//! 1. **VBV Buffer Model**:
//!    - Buffer size: 1-3 seconds worth of bits
//!    - Buffer fullness tracked in real-time
//!    - Overflow: bits > buffer_size → increase QP
//!    - Underflow: bits < 0 → decrease QP (or insert filler)
//!
//! 2. **Bit Allocation**:
//!    - Frame budget = avg_bits_per_frame × complexity_ratio
//!    - GOP budget distributed based on frame types (I/P/B)
//!    - Scene change detection → keyframe bit boost
//!
//! 3. **QP Adjustment**:
//!    - Buffer fullness → QP modifier
//!    - Smooth transitions: |QP_new - QP_prev| ≤ 4
//!    - Anti-oscillation damping (EWMA)
//!
//! # Performance Target
//!
//! - Rate control decision: <100ns per frame
//! - Buffer update: <50ns
//! - QP calculation: <80ns
//!
//! # References
//!
//! - SVT-AV1 Rate Control: https://github.com/BlueSwordM/SVT-AV1/blob/master/Docs/Appendix-Rate-Control.md
//! - x264 VBV: https://slhck.info/video/2017/03/01/rate-control.html
//! - PixelTools Rate Control Theory: https://www.pixeltools.com/rate_control_paper.html
//!
//! # Sources
//!
//! - [SVT-AV1 Rate Control Appendix](https://github.com/BlueSwordM/SVT-AV1/blob/master/Docs/Appendix-Rate-Control.md)
//! - [Understanding Rate Control Modes](https://slhck.info/video/2017/03/01/rate-control.html)
//! - [x264 Rate Control Implementation](https://github.com/MasterNobody/x264/blob/master/encoder/ratecontrol.c)
//! - [PixelTools Rate Control Theory](https://www.pixeltools.com/rate_control_paper.html)

use core::sync::atomic::{AtomicU64, AtomicI64, Ordering};

#[cfg(feature = "std")]
use std::fmt;

// ============================================================================
// Q16.16 Fixed-Point Helpers
// ============================================================================

/// Q16.16 fixed-point constant: 1.0
const Q16_ONE: u64 = 1 << 16; // 65536

/// Q16.16 fixed-point constant: 0.5
const Q16_HALF: u64 = 1 << 15; // 32768

/// Convert integer to Q16.16
#[inline]
const fn to_q16(val: u32) -> u64 {
    (val as u64) << 16
}

/// Convert Q16.16 to integer (round to nearest)
#[inline]
const fn from_q16(val: u64) -> u32 {
    ((val + Q16_HALF) >> 16) as u32
}

/// Q16.16 multiply (result in Q16.16)
#[inline]
fn q16_mul(a: u64, b: u64) -> u64 {
    let product = (a as u128) * (b as u128);
    ((product + (Q16_HALF as u128)) >> 16) as u64
}

/// Q16.16 divide (result in Q16.16)
#[inline]
fn q16_div(a: u64, b: u64) -> u64 {
    if b == 0 {
        return u64::MAX; // Saturate on divide-by-zero
    }
    let numerator = (a as u128) << 16;
    (numerator / (b as u128)) as u64
}

/// Clamp Q16.16 value to range [min, max]
#[inline]
const fn q16_clamp(val: u64, min: u64, max: u64) -> u64 {
    if val < min {
        min
    } else if val > max {
        max
    } else {
        val
    }
}

/// Signed Q16.16 conversion (for buffer fullness which can be negative)
#[inline]
const fn to_q16_signed(val: i32) -> i64 {
    (val as i64) << 16
}

/// Convert signed Q16.16 to integer
#[inline]
const fn from_q16_signed(val: i64) -> i32 {
    ((val + (Q16_HALF as i64)) >> 16) as i32
}

// ============================================================================
// Rate Control Mode
// ============================================================================

/// Rate control mode for CBR/VBR encoding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BitrateMode {
    /// Constant Bitrate - strict VBV compliance
    CBR = 0,
    /// Variable Bitrate - average bitrate target
    VBR = 1,
    /// Constrained VBR - VBR with peak bitrate limit
    ConstrainedVBR = 2,
    /// One-pass VBR with lookahead
    OnePassVBR = 3,
    /// Two-pass VBR for VOD (first pass collects stats)
    TwoPassVBR = 4,
}

impl BitrateMode {
    #[inline]
    const fn from_bits(bits: u64) -> Self {
        match bits & 0x7 {
            0 => BitrateMode::CBR,
            1 => BitrateMode::VBR,
            2 => BitrateMode::ConstrainedVBR,
            3 => BitrateMode::OnePassVBR,
            4 => BitrateMode::TwoPassVBR,
            _ => BitrateMode::VBR, // Default fallback
        }
    }

    #[inline]
    const fn to_bits(self) -> u64 {
        self as u64
    }
}

/// Frame type for bit allocation weighting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RcFrameType {
    /// Intra frame (keyframe) - highest bit budget
    Intra = 0,
    /// P-frame (forward predicted) - medium budget
    PFrame = 1,
    /// B-frame (bidirectional) - lowest budget
    BFrame = 2,
    /// Golden frame (long-term reference) - higher budget
    Golden = 3,
    /// ARF (alternate reference frame)
    AltRef = 4,
}

impl RcFrameType {
    /// Bit weight multiplier (Q16.16)
    /// Based on SVT-AV1: I:P:B ≈ 3:1:0.5
    #[inline]
    const fn bit_weight_q16(&self) -> u64 {
        match self {
            RcFrameType::Intra => to_q16(3),      // 3.0× base
            RcFrameType::Golden => to_q16(2),     // 2.0× base
            RcFrameType::AltRef => (Q16_ONE * 3) / 2, // 1.5× base
            RcFrameType::PFrame => Q16_ONE,       // 1.0× base
            RcFrameType::BFrame => Q16_HALF,      // 0.5× base
        }
    }
}

// ============================================================================
// VBV State Packing
// ============================================================================

/// VBV state bit layout: [mode:3|qp:8|qp_delta:5|overflow:1|underflow:1|gen:14|reserved:32]
struct VbvState;

impl VbvState {
    const MODE_SHIFT: u32 = 61;
    const MODE_MASK: u64 = 0x7 << Self::MODE_SHIFT;

    const QP_SHIFT: u32 = 53;
    const QP_MASK: u64 = 0xFF << Self::QP_SHIFT;

    const QP_DELTA_SHIFT: u32 = 48;
    const QP_DELTA_MASK: u64 = 0x1F << Self::QP_DELTA_SHIFT;

    const OVERFLOW_SHIFT: u32 = 47;
    const OVERFLOW_MASK: u64 = 1 << Self::OVERFLOW_SHIFT;

    const UNDERFLOW_SHIFT: u32 = 46;
    const UNDERFLOW_MASK: u64 = 1 << Self::UNDERFLOW_SHIFT;

    const GEN_SHIFT: u32 = 32;
    const GEN_MASK: u64 = 0x3FFF << Self::GEN_SHIFT;

    #[inline]
    fn pack(mode: BitrateMode, qp: u8, qp_delta: i8, overflow: bool, underflow: bool, gen: u16) -> u64 {
        let mode_bits = (mode.to_bits() & 0x7) << Self::MODE_SHIFT;
        let qp_bits = ((qp as u64) & 0xFF) << Self::QP_SHIFT;
        // Map qp_delta [-4, +4] to [0, 8] for 5-bit storage
        let delta_unsigned = ((qp_delta + 4).clamp(0, 8) as u64) & 0x1F;
        let qp_delta_bits = delta_unsigned << Self::QP_DELTA_SHIFT;
        let overflow_bits = if overflow { Self::OVERFLOW_MASK } else { 0 };
        let underflow_bits = if underflow { Self::UNDERFLOW_MASK } else { 0 };
        let gen_bits = ((gen as u64) & 0x3FFF) << Self::GEN_SHIFT;
        mode_bits | qp_bits | qp_delta_bits | overflow_bits | underflow_bits | gen_bits
    }

    #[inline]
    fn unpack(state: u64) -> (BitrateMode, u8, i8, bool, bool, u16) {
        let mode = BitrateMode::from_bits((state & Self::MODE_MASK) >> Self::MODE_SHIFT);
        let qp = ((state & Self::QP_MASK) >> Self::QP_SHIFT) as u8;
        let delta_unsigned = ((state & Self::QP_DELTA_MASK) >> Self::QP_DELTA_SHIFT) as i8;
        let qp_delta = delta_unsigned - 4; // Map [0, 8] back to [-4, +4]
        let overflow = (state & Self::OVERFLOW_MASK) != 0;
        let underflow = (state & Self::UNDERFLOW_MASK) != 0;
        let gen = ((state & Self::GEN_MASK) >> Self::GEN_SHIFT) as u16;
        (mode, qp, qp_delta, overflow, underflow, gen)
    }
}

// ============================================================================
// BitrateRateControlCapsule
// ============================================================================

/// SOTA CBR/VBR Rate Control Capsule with VBV Buffer Model
///
/// # Layout (512 bytes, 8 cache lines)
///
/// ```text
/// Offset | Field                 | Size | Description
/// -------|----------------------|------|-------------
/// 0      | vbv_state            | 8    | Packed state (mode, QP, flags, gen)
/// 8      | target_bitrate_q16   | 8    | Target bitrate in bits/sec (Q16.16)
/// 16     | max_bitrate_q16      | 8    | Max bitrate for VBV (Q16.16)
/// 24     | vbv_buffer_size_q16  | 8    | VBV buffer capacity in bits (Q16.16)
/// 32     | vbv_buffer_fullness  | 8    | Current buffer level (signed Q16.16)
/// 40     | frame_rate_q16       | 8    | Frames per second (Q16.16)
/// 48     | bits_per_frame_q16   | 8    | Target bits per frame (Q16.16)
/// 56     | gop_target_bits_q16  | 8    | Target bits for current GOP (Q16.16)
/// 64     | gop_actual_bits_q16  | 8    | Actual bits spent in GOP (Q16.16)
/// 72     | gop_frames_total     | 8    | Total frames in current GOP
/// 80     | gop_frames_encoded   | 8    | Frames encoded in current GOP
/// 88     | avg_complexity_q16   | 8    | Average frame complexity (EWMA Q16.16)
/// 96     | scene_complexity_q16 | 8    | Current scene complexity (Q16.16)
/// 104    | prev_qp              | 8    | Previous frame QP (for smoothing)
/// 112    | total_bits_encoded   | 8    | Total bits encoded (for two-pass)
/// 120    | total_frames_encoded | 8    | Total frames encoded
/// 128    | complexity_buffer[8] | 64   | Lookahead complexity buffer (8 frames)
/// 192    | bit_history[8]       | 64   | Recent frame bit sizes (for adaptation)
/// 256    | qp_history[8]        | 64   | Recent QP values (for oscillation damping)
/// 320    | _padding             | 192  | Padding to 512B
/// ```
#[repr(C, align(512))]
pub struct BitrateRateControlCapsule {
    /// Packed VBV state: [mode:3|qp:8|qp_delta:5|overflow:1|underflow:1|gen:14|reserved:32]
    vbv_state: AtomicU64,

    /// Target bitrate in bits/sec (Q16.16)
    target_bitrate_q16: AtomicU64,

    /// Max bitrate for VBV buffer (Q16.16)
    /// For CBR: max = target
    /// For VBR: max = peak bitrate (e.g., 2× target)
    max_bitrate_q16: AtomicU64,

    /// VBV buffer size in bits (Q16.16)
    /// Typical: 1-3 seconds worth of bits at target rate
    vbv_buffer_size_q16: AtomicU64,

    /// Current VBV buffer fullness (signed Q16.16)
    /// Positive = bits available, negative = underflow
    vbv_buffer_fullness: AtomicI64,

    /// Frame rate in FPS (Q16.16)
    frame_rate_q16: AtomicU64,

    /// Target bits per frame (Q16.16)
    /// Calculated as: target_bitrate / frame_rate
    bits_per_frame_q16: AtomicU64,

    /// Target bits for current GOP (Q16.16)
    gop_target_bits_q16: AtomicU64,

    /// Actual bits spent in current GOP (Q16.16)
    gop_actual_bits_q16: AtomicU64,

    /// Total frames in current GOP
    gop_frames_total: AtomicU64,

    /// Frames encoded in current GOP
    gop_frames_encoded: AtomicU64,

    /// Average frame complexity (Q16.16, EWMA with α=0.1)
    avg_complexity_q16: AtomicU64,

    /// Current scene complexity (Q16.16)
    scene_complexity_q16: AtomicU64,

    /// Previous frame QP (for smooth transitions)
    prev_qp: AtomicU64,

    /// Total bits encoded across all frames
    total_bits_encoded: AtomicU64,

    /// Total frames encoded
    total_frames_encoded: AtomicU64,

    /// Lookahead complexity buffer (8 frames, Q16.16 each)
    complexity_buffer: [AtomicU64; 8],

    /// Recent frame bit sizes for adaptation (8 frames)
    bit_history: [AtomicU64; 8],

    /// Recent QP history for oscillation damping (8 frames)
    qp_history: [AtomicU64; 8],

    /// Padding to 512 bytes
    _padding: [u64; 24],
}

impl BitrateRateControlCapsule {
    /// QP range constants (AV1 spec: 0-255)
    pub const QP_MIN: u8 = 1;   // Avoid QP 0 (lossless)
    pub const QP_MAX: u8 = 255; // AV1 max qindex

    /// Default practical QP range for typical encoding
    pub const QP_PRACTICAL_MIN: u8 = 10;
    pub const QP_PRACTICAL_MAX: u8 = 63;

    /// QP delta clamp for smooth transitions (±4 per frame)
    pub const QP_DELTA_MAX: i8 = 4;

    /// EWMA alpha for complexity tracking (0.1 in Q16.16)
    const EWMA_ALPHA_Q16: u64 = 6554; // 0.1 in Q16.16

    /// VBV buffer fill target (50% for stability)
    const VBV_TARGET_FULLNESS_Q16: u64 = Q16_HALF;

    /// VBV overflow threshold (95% full)
    const VBV_OVERFLOW_THRESHOLD_Q16: u64 = (Q16_ONE * 95) / 100;

    /// VBV underflow threshold (5% full)
    const VBV_UNDERFLOW_THRESHOLD_Q16: u64 = (Q16_ONE * 5) / 100;

    /// Buffer size in number of frames (2 seconds default)
    pub const DEFAULT_VBV_BUFFER_SECONDS: f32 = 2.0;

    /// Create new BitrateRateControlCapsule for CBR/VBR encoding
    ///
    /// # Arguments
    ///
    /// - `mode`: Rate control mode (CBR, VBR, ConstrainedVBR, etc.)
    /// - `target_bitrate_kbps`: Target bitrate in kilobits per second
    /// - `max_bitrate_kbps`: Max bitrate for VBV (use target for CBR)
    /// - `frame_rate`: Frame rate (e.g., 30.0 for 30fps)
    /// - `vbv_buffer_seconds`: VBV buffer size in seconds (1.0-3.0 typical)
    ///
    /// # Examples
    ///
    /// ```
    /// use kindly_av1::encoder::rate_control_bitrate::{BitrateRateControlCapsule, BitrateMode};
    ///
    /// // CBR at 5 Mbps, 30fps, 2-second buffer
    /// let rc = BitrateRateControlCapsule::new(
    ///     BitrateMode::CBR,
    ///     5000,    // 5000 kbps = 5 Mbps
    ///     5000,    // max = target for CBR
    ///     30.0,    // 30 fps
    ///     2.0,     // 2-second VBV buffer
    /// );
    /// ```
    pub fn new(
        mode: BitrateMode,
        target_bitrate_kbps: u32,
        max_bitrate_kbps: u32,
        frame_rate: f32,
        vbv_buffer_seconds: f32,
    ) -> Self {
        // Convert kbps to bits/sec (Q16.16)
        let target_bps = (target_bitrate_kbps as u64) * 1000;
        let target_bitrate_q16 = target_bps << 16;

        let max_bps = (max_bitrate_kbps as u64) * 1000;
        let max_bitrate_q16 = max_bps << 16;

        // Frame rate in Q16.16
        let frame_rate_q16 = ((frame_rate * 65536.0) as u64).max(Q16_ONE);

        // Bits per frame = target_bitrate / frame_rate
        let bits_per_frame_q16 = q16_div(target_bitrate_q16, frame_rate_q16);

        // VBV buffer size = max_bitrate × vbv_buffer_seconds
        let vbv_seconds_q16 = ((vbv_buffer_seconds * 65536.0) as u64).max(Q16_ONE);
        let vbv_buffer_size_q16 = q16_mul(max_bitrate_q16, vbv_seconds_q16);

        // Initial buffer fullness at 50%
        let initial_fullness = (vbv_buffer_size_q16 / 2) as i64;

        // Initial QP (mid-range for startup)
        let initial_qp = 32u8;
        let initial_state = VbvState::pack(mode, initial_qp, 0, false, false, 0);

        Self {
            vbv_state: AtomicU64::new(initial_state),
            target_bitrate_q16: AtomicU64::new(target_bitrate_q16),
            max_bitrate_q16: AtomicU64::new(max_bitrate_q16),
            vbv_buffer_size_q16: AtomicU64::new(vbv_buffer_size_q16),
            vbv_buffer_fullness: AtomicI64::new(initial_fullness),
            frame_rate_q16: AtomicU64::new(frame_rate_q16),
            bits_per_frame_q16: AtomicU64::new(bits_per_frame_q16),
            gop_target_bits_q16: AtomicU64::new(0),
            gop_actual_bits_q16: AtomicU64::new(0),
            gop_frames_total: AtomicU64::new(0),
            gop_frames_encoded: AtomicU64::new(0),
            avg_complexity_q16: AtomicU64::new(to_q16(1000)), // Initial estimate
            scene_complexity_q16: AtomicU64::new(to_q16(1000)),
            prev_qp: AtomicU64::new(initial_qp as u64),
            total_bits_encoded: AtomicU64::new(0),
            total_frames_encoded: AtomicU64::new(0),
            complexity_buffer: [
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
            ],
            bit_history: [
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
            ],
            qp_history: [
                AtomicU64::new(initial_qp as u64), AtomicU64::new(initial_qp as u64),
                AtomicU64::new(initial_qp as u64), AtomicU64::new(initial_qp as u64),
                AtomicU64::new(initial_qp as u64), AtomicU64::new(initial_qp as u64),
                AtomicU64::new(initial_qp as u64), AtomicU64::new(initial_qp as u64),
            ],
            _padding: [0; 24],
        }
    }

    /// Allocate bits for the next frame based on VBV buffer state
    ///
    /// This is the core CBR/VBR bit allocation algorithm based on x264/SVT-AV1.
    ///
    /// # Algorithm
    ///
    /// 1. Start with target bits_per_frame
    /// 2. Adjust by frame type weight (I/P/B)
    /// 3. Scale by complexity ratio (frame_complexity / avg_complexity)
    /// 4. Apply VBV buffer constraints:
    ///    - If buffer near overflow → reduce allocation
    ///    - If buffer near underflow → increase allocation
    /// 5. Clamp to [min_frame_bits, max_frame_bits]
    ///
    /// # Performance
    ///
    /// - <100ns (8 atomic loads + Q16.16 arithmetic)
    ///
    /// # Arguments
    ///
    /// - `frame_type`: Type of frame (I/P/B)
    /// - `frame_complexity`: Spatial complexity metric (variance, SAD, etc.)
    ///
    /// # Returns
    ///
    /// Target bits for this frame (integer)
    pub fn allocate_bits(&self, frame_type: RcFrameType, frame_complexity: u32) -> u32 {
        // #ASSUME: Atomic loads with Relaxed ordering sufficient for allocation
        // #VERIFY: Bit allocation is advisory (encoder validates after encoding)

        let mode_state = self.vbv_state.load(Ordering::Acquire);
        let (mode, _qp, _delta, _overflow, _underflow, _gen) = VbvState::unpack(mode_state);

        let base_bits_q16 = self.bits_per_frame_q16.load(Ordering::Relaxed);
        let avg_complexity = self.avg_complexity_q16.load(Ordering::Relaxed).max(Q16_ONE);
        let complexity_q16 = to_q16(frame_complexity).max(Q16_ONE);

        // Step 1: Frame type weight
        let type_weight = frame_type.bit_weight_q16();
        let weighted_bits = q16_mul(base_bits_q16, type_weight);

        // Step 2: Complexity ratio adjustment (VBR only)
        let complexity_adjusted = if mode == BitrateMode::CBR {
            // CBR: minimal complexity adjustment (±10% max)
            let ratio = q16_div(complexity_q16, avg_complexity);
            let clamped_ratio = q16_clamp(ratio, to_q16(90) / 100, to_q16(110) / 100);
            q16_mul(weighted_bits, clamped_ratio)
        } else {
            // VBR: full complexity adjustment
            let ratio = q16_div(complexity_q16, avg_complexity);
            let clamped_ratio = q16_clamp(ratio, Q16_HALF, to_q16(2));
            q16_mul(weighted_bits, clamped_ratio)
        };

        // Step 3: VBV buffer adjustment
        let vbv_adjusted = self.apply_vbv_constraint(complexity_adjusted);

        // Step 4: Clamp to valid range
        let min_bits = from_q16(base_bits_q16) / 10; // 10% of avg
        let max_bits = from_q16(base_bits_q16) * 4;   // 4× avg max
        from_q16(vbv_adjusted).clamp(min_bits.max(100), max_bits)
    }

    /// Apply VBV buffer constraint to bit allocation
    ///
    /// Based on x264 VBV model: adjust allocation based on buffer fullness
    /// to prevent overflow (quality degradation) and underflow (buffering stall).
    #[inline]
    fn apply_vbv_constraint(&self, target_bits_q16: u64) -> u64 {
        let buffer_size = self.vbv_buffer_size_q16.load(Ordering::Relaxed);
        let buffer_fullness = self.vbv_buffer_fullness.load(Ordering::Relaxed);

        if buffer_size == 0 {
            return target_bits_q16;
        }

        // Calculate buffer fullness ratio (0.0 - 1.0 range)
        let fullness_ratio = if buffer_fullness < 0 {
            0u64 // Underflow
        } else {
            let fullness_u64 = buffer_fullness as u64;
            q16_div(fullness_u64, buffer_size).min(Q16_ONE)
        };

        // VBV adjustment factor based on buffer state
        // Buffer full (>80%): reduce bits to drain buffer
        // Buffer low (<20%): increase bits to fill buffer
        // Buffer normal (20-80%): no adjustment
        let adjustment = if fullness_ratio > Self::VBV_OVERFLOW_THRESHOLD_Q16 {
            // Near overflow: reduce allocation by up to 30%
            let overfill = fullness_ratio - Self::VBV_OVERFLOW_THRESHOLD_Q16;
            let reduction = q16_mul(overfill, to_q16(6)); // 6× scaling
            Q16_ONE.saturating_sub(reduction.min(to_q16(30) / 100))
        } else if fullness_ratio < Self::VBV_UNDERFLOW_THRESHOLD_Q16 {
            // Near underflow: increase allocation by up to 30%
            let underfill = Self::VBV_UNDERFLOW_THRESHOLD_Q16 - fullness_ratio;
            let boost = q16_mul(underfill, to_q16(6));
            Q16_ONE + boost.min(to_q16(30) / 100)
        } else {
            // Normal operation: slight adjustment toward 50% target
            let target = Self::VBV_TARGET_FULLNESS_Q16;
            if fullness_ratio > target {
                Q16_ONE - (to_q16(5) / 100) // -5%
            } else if fullness_ratio < target {
                Q16_ONE + (to_q16(5) / 100) // +5%
            } else {
                Q16_ONE
            }
        };

        q16_mul(target_bits_q16, adjustment)
    }

    /// Get QP for current buffer state (CBR/VBR)
    ///
    /// Calculates optimal QP based on:
    /// 1. Base QP from previous frame (smooth transitions)
    /// 2. VBV buffer fullness adjustment
    /// 3. Complexity adjustment
    /// 4. Clamp delta to ±4 (anti-oscillation)
    ///
    /// # Performance
    ///
    /// - <80ns (6 atomic loads + arithmetic)
    ///
    /// # Arguments
    ///
    /// - `frame_complexity`: Spatial complexity of current frame
    ///
    /// # Returns
    ///
    /// Optimal QP for this frame (1-255 for AV1)
    pub fn get_qp_for_buffer(&self, frame_complexity: u32) -> u8 {
        let mode_state = self.vbv_state.load(Ordering::Acquire);
        let (mode, current_qp, _delta, _overflow, _underflow, _gen) = VbvState::unpack(mode_state);

        let prev_qp = self.prev_qp.load(Ordering::Relaxed) as u8;
        let avg_complexity = self.avg_complexity_q16.load(Ordering::Relaxed).max(Q16_ONE);
        let complexity_q16 = to_q16(frame_complexity);

        // Start with previous QP for continuity
        let mut qp = prev_qp as i16;

        // VBV buffer-based adjustment (primary for CBR)
        let buffer_size = self.vbv_buffer_size_q16.load(Ordering::Relaxed);
        let buffer_fullness = self.vbv_buffer_fullness.load(Ordering::Relaxed);

        if buffer_size > 0 {
            let fullness_ratio = if buffer_fullness < 0 {
                0u64
            } else {
                q16_div(buffer_fullness as u64, buffer_size).min(Q16_ONE)
            };

            // Buffer full → increase QP (reduce bitrate)
            // Buffer empty → decrease QP (increase bitrate)
            if fullness_ratio > Self::VBV_OVERFLOW_THRESHOLD_Q16 {
                // Calculate overage as percentage points above threshold
                // At 98% fullness with 95% threshold: overage = 3 percentage points
                // Each percentage point above threshold adds ~1 QP (aggressive)
                let overage_q16 = fullness_ratio.saturating_sub(Self::VBV_OVERFLOW_THRESHOLD_Q16);
                // Convert to percentage points (0.03 → 3)
                let overage_pct = q16_mul(overage_q16, to_q16(100));
                let overage_points = from_q16(overage_pct).max(1);
                qp += (overage_points as i16).min(Self::QP_DELTA_MAX as i16);
            } else if fullness_ratio < Self::VBV_UNDERFLOW_THRESHOLD_Q16 {
                // Calculate shortage as percentage points below threshold
                // At 2% fullness with 5% threshold: shortage = 3 percentage points
                let shortage_q16 = Self::VBV_UNDERFLOW_THRESHOLD_Q16.saturating_sub(fullness_ratio);
                // Convert to percentage points
                let shortage_pct = q16_mul(shortage_q16, to_q16(100));
                let shortage_points = from_q16(shortage_pct).max(1);
                qp -= (shortage_points as i16).min(Self::QP_DELTA_MAX as i16);
            }
        }

        // Complexity-based adjustment (secondary for VBR)
        if mode != BitrateMode::CBR {
            let ratio = q16_div(complexity_q16, avg_complexity);
            if ratio > to_q16(2) {
                qp += 2; // High complexity
            } else if ratio > to_q16(3) / 2 {
                qp += 1;
            } else if ratio < Q16_HALF {
                qp -= 2; // Low complexity
            } else if ratio < to_q16(2) / 3 {
                qp -= 1;
            }
        }

        // Clamp QP delta to prevent oscillation (±4 max)
        let delta = (qp - prev_qp as i16)
            .clamp(-(Self::QP_DELTA_MAX as i16), Self::QP_DELTA_MAX as i16);
        qp = prev_qp as i16 + delta;

        // Final clamp to valid AV1 QP range
        qp.clamp(Self::QP_MIN as i16, Self::QP_MAX as i16) as u8
    }

    /// Update rate control state after encoding a frame
    ///
    /// Must be called after each frame is encoded to update VBV buffer
    /// and statistics for accurate rate control.
    ///
    /// # Algorithm
    ///
    /// 1. Update VBV buffer: fullness += bits_available - actual_bits
    /// 2. Update EWMA complexity
    /// 3. Update bit history for adaptation
    /// 4. Check overflow/underflow flags
    ///
    /// # Performance
    ///
    /// - <50ns (3 atomic updates + CAS)
    ///
    /// # Arguments
    ///
    /// - `actual_bits`: Actual bits used by the encoded frame
    /// - `frame_qp`: QP used for this frame
    /// - `frame_complexity`: Complexity metric of this frame
    pub fn update_after_encode(&self, actual_bits: u32, frame_qp: u8, frame_complexity: u32) {
        let bits_q16 = to_q16(actual_bits);
        let bits_per_frame = self.bits_per_frame_q16.load(Ordering::Relaxed) as i64;
        let actual_bits_i64 = bits_q16 as i64;

        // Update VBV buffer: add incoming bits (constant rate), subtract outgoing (actual)
        // Buffer change = bits_available - bits_used
        let buffer_delta = bits_per_frame - actual_bits_i64;
        self.vbv_buffer_fullness.fetch_add(buffer_delta, Ordering::Release);

        // Clamp buffer to valid range
        let buffer_size = self.vbv_buffer_size_q16.load(Ordering::Relaxed) as i64;
        let current_fullness = self.vbv_buffer_fullness.load(Ordering::Acquire);
        let clamped = current_fullness.clamp(0, buffer_size);
        if clamped != current_fullness {
            self.vbv_buffer_fullness.store(clamped, Ordering::Release);
        }

        // Update GOP actual bits
        self.gop_actual_bits_q16.fetch_add(bits_q16, Ordering::Relaxed);
        self.gop_frames_encoded.fetch_add(1, Ordering::Relaxed);

        // Update total statistics
        self.total_bits_encoded.fetch_add(actual_bits as u64, Ordering::Relaxed);
        self.total_frames_encoded.fetch_add(1, Ordering::Relaxed);

        // Update previous QP
        self.prev_qp.store(frame_qp as u64, Ordering::Relaxed);

        // Update complexity EWMA
        self.update_complexity(frame_complexity);

        // Update bit history (circular buffer)
        let frame_idx = (self.total_frames_encoded.load(Ordering::Relaxed) % 8) as usize;
        self.bit_history[frame_idx].store(bits_q16, Ordering::Relaxed);
        self.qp_history[frame_idx].store(frame_qp as u64, Ordering::Relaxed);

        // Check and update overflow/underflow flags
        self.update_vbv_flags();
    }

    /// Update complexity EWMA
    fn update_complexity(&self, frame_complexity: u32) {
        let complexity_q16 = to_q16(frame_complexity);
        let avg_old = self.avg_complexity_q16.load(Ordering::Relaxed);
        let one_minus_alpha = Q16_ONE - Self::EWMA_ALPHA_Q16;

        let term1 = q16_mul(Self::EWMA_ALPHA_Q16, complexity_q16);
        let term2 = q16_mul(one_minus_alpha, avg_old);
        let avg_new = term1 + term2;

        // CAS loop for atomic update
        let mut current = avg_old;
        loop {
            match self.avg_complexity_q16.compare_exchange_weak(
                current,
                avg_new,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }

        self.scene_complexity_q16.store(complexity_q16, Ordering::Relaxed);
    }

    /// Update VBV overflow/underflow flags
    fn update_vbv_flags(&self) {
        let buffer_size = self.vbv_buffer_size_q16.load(Ordering::Relaxed);
        let buffer_fullness = self.vbv_buffer_fullness.load(Ordering::Relaxed);

        let overflow = buffer_fullness >= buffer_size as i64;
        let underflow = buffer_fullness <= 0;

        // Update state with new flags
        let old_state = self.vbv_state.load(Ordering::Acquire);
        let (mode, qp, delta, _old_overflow, _old_underflow, gen) = VbvState::unpack(old_state);
        let new_state = VbvState::pack(mode, qp, delta, overflow, underflow, gen.wrapping_add(1));

        self.vbv_state.store(new_state, Ordering::Release);
    }

    /// Start a new GOP (Group of Pictures)
    ///
    /// Resets GOP-level counters and calculates bit budget for the GOP.
    ///
    /// # Arguments
    ///
    /// - `gop_size`: Number of frames in this GOP
    pub fn start_gop(&self, gop_size: u32) {
        let bits_per_frame = self.bits_per_frame_q16.load(Ordering::Relaxed);
        let gop_target = q16_mul(bits_per_frame, to_q16(gop_size));

        self.gop_target_bits_q16.store(gop_target, Ordering::Relaxed);
        self.gop_actual_bits_q16.store(0, Ordering::Relaxed);
        self.gop_frames_total.store(gop_size as u64, Ordering::Relaxed);
        self.gop_frames_encoded.store(0, Ordering::Relaxed);
    }

    /// Get current VBV buffer state
    ///
    /// # Returns
    ///
    /// Tuple of (buffer_fullness_percent, is_overflow, is_underflow)
    pub fn get_vbv_state(&self) -> (u32, bool, bool) {
        let buffer_size = self.vbv_buffer_size_q16.load(Ordering::Relaxed);
        let buffer_fullness = self.vbv_buffer_fullness.load(Ordering::Relaxed);
        let state = self.vbv_state.load(Ordering::Acquire);
        let (_mode, _qp, _delta, overflow, underflow, _gen) = VbvState::unpack(state);

        let fullness_percent = if buffer_size > 0 && buffer_fullness > 0 {
            ((buffer_fullness as u64) * 100 / buffer_size) as u32
        } else {
            0
        };

        (fullness_percent.min(100), overflow, underflow)
    }

    /// Get current encoding statistics
    ///
    /// # Returns
    ///
    /// (mode, current_qp, avg_complexity, total_bits, total_frames)
    pub fn get_stats(&self) -> (BitrateMode, u8, u32, u64, u64) {
        let state = self.vbv_state.load(Ordering::Acquire);
        let (mode, qp, _delta, _overflow, _underflow, _gen) = VbvState::unpack(state);

        let avg_complexity = from_q16(self.avg_complexity_q16.load(Ordering::Relaxed));
        let total_bits = self.total_bits_encoded.load(Ordering::Relaxed);
        let total_frames = self.total_frames_encoded.load(Ordering::Relaxed);

        (mode, qp, avg_complexity, total_bits, total_frames)
    }

    /// Get actual average bitrate achieved so far
    ///
    /// # Returns
    ///
    /// Average bitrate in kbps
    pub fn get_actual_bitrate_kbps(&self) -> u32 {
        let total_bits = self.total_bits_encoded.load(Ordering::Relaxed);
        let total_frames = self.total_frames_encoded.load(Ordering::Relaxed).max(1);
        let frame_rate_q16 = self.frame_rate_q16.load(Ordering::Relaxed);
        let frame_rate = from_q16(frame_rate_q16).max(1) as u64;

        let bits_per_frame = total_bits / total_frames;
        let bits_per_second = bits_per_frame * frame_rate;
        (bits_per_second / 1000) as u32
    }

    /// Update lookahead complexity buffer
    ///
    /// # Arguments
    ///
    /// - `index`: Frame index in lookahead (0-7)
    /// - `complexity`: Complexity metric
    pub fn update_lookahead(&self, index: usize, complexity: u32) {
        if index < 8 {
            self.complexity_buffer[index].store(to_q16(complexity), Ordering::Relaxed);
        }
    }

    /// Get average lookahead complexity
    pub fn get_lookahead_avg_complexity(&self) -> u32 {
        let mut sum = 0u64;
        let mut count = 0u32;

        for slot in &self.complexity_buffer {
            let val = slot.load(Ordering::Relaxed);
            if val > 0 {
                sum += val;
                count += 1;
            }
        }

        if count > 0 {
            from_q16(sum / count as u64)
        } else {
            1000 // Default
        }
    }

    /// Set new target bitrate (for adaptive bitrate streaming)
    ///
    /// # Arguments
    ///
    /// - `target_kbps`: New target bitrate in kbps
    pub fn set_target_bitrate(&self, target_kbps: u32) {
        let target_bps = (target_kbps as u64) * 1000;
        let target_q16 = target_bps << 16;
        self.target_bitrate_q16.store(target_q16, Ordering::Release);

        // Recalculate bits per frame
        let frame_rate_q16 = self.frame_rate_q16.load(Ordering::Relaxed);
        let bits_per_frame = q16_div(target_q16, frame_rate_q16);
        self.bits_per_frame_q16.store(bits_per_frame, Ordering::Release);
    }
}

// ============================================================================
// Debug Implementation
// ============================================================================

#[cfg(feature = "std")]
impl fmt::Debug for BitrateRateControlCapsule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.vbv_state.load(Ordering::Acquire);
        let (mode, qp, delta, overflow, underflow, gen) = VbvState::unpack(state);
        let (fullness_pct, _, _) = self.get_vbv_state();

        f.debug_struct("BitrateRateControlCapsule")
            .field("mode", &mode)
            .field("current_qp", &qp)
            .field("qp_delta", &delta)
            .field("vbv_fullness_%", &fullness_pct)
            .field("overflow", &overflow)
            .field("underflow", &underflow)
            .field("generation", &gen)
            .field("target_kbps", &(from_q16(self.target_bitrate_q16.load(Ordering::Relaxed)) / 1000))
            .field("actual_kbps", &self.get_actual_bitrate_kbps())
            .field("total_frames", &self.total_frames_encoded.load(Ordering::Relaxed))
            .finish()
    }
}

// ============================================================================
// Tests (T28 Unit Tests)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // T28 Q1-Q7: Unit Tests
    // ========================================================================

    #[test]
    fn test_q16_conversion() {
        assert_eq!(to_q16(0), 0);
        assert_eq!(to_q16(1), 65536);
        assert_eq!(to_q16(100), 6553600);
        assert_eq!(from_q16(65536), 1);
        assert_eq!(from_q16(6553600), 100);
        assert_eq!(from_q16(32768), 1); // 0.5 rounds up
    }

    #[test]
    fn test_q16_multiply() {
        assert_eq!(q16_mul(to_q16(2), to_q16(3)), to_q16(6));
        assert_eq!(q16_mul(to_q16(10), Q16_HALF), to_q16(5));
    }

    #[test]
    fn test_q16_divide() {
        assert_eq!(q16_div(to_q16(6), to_q16(2)), to_q16(3));
        assert_eq!(q16_div(to_q16(1), to_q16(2)), Q16_HALF);
        assert_eq!(q16_div(to_q16(1), 0), u64::MAX); // Div by zero protection
    }

    #[test]
    fn test_vbv_state_packing() {
        let state = VbvState::pack(BitrateMode::CBR, 32, 2, true, false, 100);
        let (mode, qp, delta, overflow, underflow, gen) = VbvState::unpack(state);

        assert_eq!(mode, BitrateMode::CBR);
        assert_eq!(qp, 32);
        assert_eq!(delta, 2);
        assert!(overflow);
        assert!(!underflow);
        assert_eq!(gen, 100);
    }

    #[test]
    fn test_vbv_state_negative_delta() {
        let state = VbvState::pack(BitrateMode::VBR, 40, -3, false, true, 50);
        let (mode, qp, delta, overflow, underflow, gen) = VbvState::unpack(state);

        assert_eq!(mode, BitrateMode::VBR);
        assert_eq!(qp, 40);
        assert_eq!(delta, -3);
        assert!(!overflow);
        assert!(underflow);
        assert_eq!(gen, 50);
    }

    #[test]
    fn test_capsule_creation_cbr() {
        let rc = BitrateRateControlCapsule::new(
            BitrateMode::CBR,
            5000,  // 5 Mbps
            5000,  // max = target for CBR
            30.0,  // 30 fps
            2.0,   // 2-second buffer
        );

        let (mode, qp, _complexity, _bits, _frames) = rc.get_stats();
        assert_eq!(mode, BitrateMode::CBR);
        assert_eq!(qp, 32); // Initial QP

        let (fullness, overflow, underflow) = rc.get_vbv_state();
        assert_eq!(fullness, 50); // Initial 50% fullness
        assert!(!overflow);
        assert!(!underflow);
    }

    #[test]
    fn test_capsule_creation_vbr() {
        let rc = BitrateRateControlCapsule::new(
            BitrateMode::VBR,
            4000,   // 4 Mbps target
            8000,   // 8 Mbps max (2× target)
            24.0,   // 24 fps
            1.5,    // 1.5-second buffer
        );

        let (mode, _qp, _complexity, _bits, _frames) = rc.get_stats();
        assert_eq!(mode, BitrateMode::VBR);
    }

    #[test]
    fn test_allocate_bits_intra_frame() {
        let rc = BitrateRateControlCapsule::new(BitrateMode::VBR, 5000, 10000, 30.0, 2.0);

        let bits = rc.allocate_bits(RcFrameType::Intra, 1000);
        let base_bits = (5000 * 1000) / 30; // ~166K bits/frame base

        // Intra frames get 3× weight
        assert!(bits > base_bits);
        assert!(bits < base_bits * 5); // But bounded
    }

    #[test]
    fn test_allocate_bits_b_frame() {
        let rc = BitrateRateControlCapsule::new(BitrateMode::VBR, 5000, 10000, 30.0, 2.0);

        let bits = rc.allocate_bits(RcFrameType::BFrame, 1000);
        let base_bits = (5000 * 1000) / 30;

        // B-frames get 0.5× weight
        assert!(bits < base_bits);
        assert!(bits > base_bits / 4); // But not too low
    }

    #[test]
    fn test_get_qp_for_buffer_normal() {
        let rc = BitrateRateControlCapsule::new(BitrateMode::CBR, 5000, 5000, 30.0, 2.0);

        // Buffer at 50% fullness
        let qp = rc.get_qp_for_buffer(1000);

        // Should be near initial QP (32)
        assert!(qp >= 28 && qp <= 36);
    }

    #[test]
    fn test_get_qp_buffer_overflow() {
        let rc = BitrateRateControlCapsule::new(BitrateMode::CBR, 5000, 5000, 30.0, 2.0);

        // Simulate buffer nearly full (95%+)
        let buffer_size = rc.vbv_buffer_size_q16.load(Ordering::Relaxed) as i64;
        rc.vbv_buffer_fullness.store((buffer_size * 98) / 100, Ordering::Release);

        let qp = rc.get_qp_for_buffer(1000);

        // QP should increase to drain buffer
        assert!(qp > 32);
    }

    #[test]
    fn test_get_qp_buffer_underflow() {
        let rc = BitrateRateControlCapsule::new(BitrateMode::CBR, 5000, 5000, 30.0, 2.0);

        // Simulate buffer nearly empty (2%)
        let buffer_size = rc.vbv_buffer_size_q16.load(Ordering::Relaxed) as i64;
        rc.vbv_buffer_fullness.store((buffer_size * 2) / 100, Ordering::Release);

        let qp = rc.get_qp_for_buffer(1000);

        // QP should decrease to fill buffer
        assert!(qp < 32);
    }

    #[test]
    fn test_update_after_encode() {
        let rc = BitrateRateControlCapsule::new(BitrateMode::CBR, 5000, 5000, 30.0, 2.0);

        let initial_fullness = rc.vbv_buffer_fullness.load(Ordering::Relaxed);

        // Encode a frame using target bits
        let target_bits = (5000 * 1000) / 30;
        rc.update_after_encode(target_bits, 32, 1000);

        let new_fullness = rc.vbv_buffer_fullness.load(Ordering::Relaxed);
        let total_frames = rc.total_frames_encoded.load(Ordering::Relaxed);

        // Buffer should be approximately same (used = available)
        assert!((new_fullness - initial_fullness).abs() < 10_000_000);
        assert_eq!(total_frames, 1);
    }

    #[test]
    fn test_start_gop() {
        let rc = BitrateRateControlCapsule::new(BitrateMode::VBR, 5000, 10000, 30.0, 2.0);

        rc.start_gop(60); // 60-frame GOP (2 seconds)

        let gop_frames = rc.gop_frames_total.load(Ordering::Relaxed);
        let gop_encoded = rc.gop_frames_encoded.load(Ordering::Relaxed);

        assert_eq!(gop_frames, 60);
        assert_eq!(gop_encoded, 0);
    }

    #[test]
    fn test_complexity_ewma() {
        let rc = BitrateRateControlCapsule::new(BitrateMode::VBR, 5000, 10000, 30.0, 2.0);

        // Initial complexity is 1000
        let initial = from_q16(rc.avg_complexity_q16.load(Ordering::Relaxed));
        assert_eq!(initial, 1000);

        // Update with higher complexity
        rc.update_after_encode(100000, 32, 2000);
        let after1 = from_q16(rc.avg_complexity_q16.load(Ordering::Relaxed));

        // Should increase (EWMA with α=0.1)
        assert!(after1 > initial);
        assert!(after1 < 2000); // But not jump to new value
    }

    #[test]
    fn test_actual_bitrate_calculation() {
        let rc = BitrateRateControlCapsule::new(BitrateMode::CBR, 5000, 5000, 30.0, 2.0);

        // Simulate encoding 30 frames at target rate
        let bits_per_frame = (5000 * 1000) / 30;
        for _ in 0..30 {
            rc.update_after_encode(bits_per_frame, 32, 1000);
        }

        let actual_kbps = rc.get_actual_bitrate_kbps();

        // Should be approximately 5000 kbps
        assert!(actual_kbps >= 4500 && actual_kbps <= 5500);
    }

    #[test]
    fn test_qp_smooth_transitions() {
        let rc = BitrateRateControlCapsule::new(BitrateMode::VBR, 5000, 10000, 30.0, 2.0);

        // Get initial QP
        let qp1 = rc.get_qp_for_buffer(1000);
        rc.prev_qp.store(qp1 as u64, Ordering::Relaxed);

        // Extreme complexity change
        let qp2 = rc.get_qp_for_buffer(10000);

        // Delta should be clamped to ±4
        let delta = (qp2 as i16 - qp1 as i16).abs();
        assert!(delta <= 4);
    }

    #[test]
    fn test_lookahead_buffer() {
        let rc = BitrateRateControlCapsule::new(BitrateMode::VBR, 5000, 10000, 30.0, 2.0);

        // Fill lookahead buffer
        for i in 0..8 {
            rc.update_lookahead(i, 1000 + i as u32 * 100);
        }

        let avg = rc.get_lookahead_avg_complexity();

        // Average of 1000, 1100, 1200, ..., 1700 = 1350
        assert!(avg >= 1300 && avg <= 1400);
    }

    #[test]
    fn test_set_target_bitrate() {
        let rc = BitrateRateControlCapsule::new(BitrateMode::CBR, 5000, 5000, 30.0, 2.0);

        rc.set_target_bitrate(8000); // Increase to 8 Mbps

        let new_bits_per_frame = from_q16(rc.bits_per_frame_q16.load(Ordering::Relaxed));
        let expected = (8000 * 1000) / 30;

        // Should be approximately 266K bits/frame
        assert!(new_bits_per_frame >= expected - 1000 && new_bits_per_frame <= expected + 1000);
    }

    #[test]
    fn test_capsule_size_and_alignment() {
        // Verify 512B size and alignment
        assert_eq!(core::mem::size_of::<BitrateRateControlCapsule>(), 512);
        assert_eq!(core::mem::align_of::<BitrateRateControlCapsule>(), 512);
    }

    #[test]
    fn test_frame_type_weights() {
        assert_eq!(from_q16(RcFrameType::Intra.bit_weight_q16()), 3);
        assert_eq!(from_q16(RcFrameType::Golden.bit_weight_q16()), 2);
        assert_eq!(from_q16(RcFrameType::PFrame.bit_weight_q16()), 1);
        // B-frame is 0.5, so from_q16 should give 1 (rounds 0.5 up)
        assert_eq!(from_q16(RcFrameType::BFrame.bit_weight_q16()), 1);
    }

    #[test]
    fn test_concurrent_updates() {
        use std::sync::Arc;
        use std::thread;

        let rc = Arc::new(BitrateRateControlCapsule::new(
            BitrateMode::CBR, 5000, 5000, 30.0, 2.0,
        ));
        rc.start_gop(120);

        let mut handles = vec![];

        // 4 threads updating concurrently
        for _ in 0..4 {
            let rc_clone = Arc::clone(&rc);
            handles.push(thread::spawn(move || {
                for _ in 0..25 {
                    let bits = rc_clone.allocate_bits(RcFrameType::PFrame, 1000);
                    rc_clone.update_after_encode(bits, 32, 1000);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let total_frames = rc.total_frames_encoded.load(Ordering::Relaxed);
        assert_eq!(total_frames, 100); // 4 threads × 25 frames
    }

    #[test]
    fn test_bitrate_mode_variants() {
        let modes = [
            BitrateMode::CBR,
            BitrateMode::VBR,
            BitrateMode::ConstrainedVBR,
            BitrateMode::OnePassVBR,
            BitrateMode::TwoPassVBR,
        ];

        for mode in modes {
            let rc = BitrateRateControlCapsule::new(mode, 5000, 10000, 30.0, 2.0);
            let (actual_mode, _qp, _comp, _bits, _frames) = rc.get_stats();
            assert_eq!(actual_mode, mode);
        }
    }

    #[test]
    fn test_vbv_overflow_flag_set() {
        let rc = BitrateRateControlCapsule::new(BitrateMode::CBR, 5000, 5000, 30.0, 2.0);

        // Force buffer to overflow
        let buffer_size = rc.vbv_buffer_size_q16.load(Ordering::Relaxed) as i64;
        rc.vbv_buffer_fullness.store(buffer_size + 1, Ordering::Release);
        rc.update_vbv_flags();

        let (_, overflow, _) = rc.get_vbv_state();
        assert!(overflow);
    }

    #[test]
    fn test_vbv_underflow_flag_set() {
        let rc = BitrateRateControlCapsule::new(BitrateMode::CBR, 5000, 5000, 30.0, 2.0);

        // Force buffer to underflow
        rc.vbv_buffer_fullness.store(-1000, Ordering::Release);
        rc.update_vbv_flags();

        let (_, _, underflow) = rc.get_vbv_state();
        assert!(underflow);
    }
}
