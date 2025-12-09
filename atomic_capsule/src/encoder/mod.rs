//! AV1 Encoder Capsules - Lockfree Video Encoding Primitives
//!
//! This module provides computational capsules for AV1 video encoding, replacing rav1e
//! with 100% lockfree, cache-aligned primitives following UCE34/Chaos framework.
//!
//! # Architecture
//!
//! ## Phase 1: Intra-Only Encoder (8 capsules, 8-12 weeks)
//! - EncoderStateCapsule (T1): Central coordination (64B)
//! - FrameBufferCapsule (T1): Frame management (128B)
//! - IntraPredictionCapsule (T2): 56 prediction modes (256B)
//! - DctTransformCapsule (T2): Chen-Wang DCT (256B)
//! - QuantizationCapsule (T3): Q16.16 deterministic (128B)
//! - EntropyCoderCapsule (T2): Daala range coder (256B)
//! - TileCoordinatorCapsule (T4): Parallel tiles (128B)
//! - ObuBitstreamWriterCapsule (T5): AV1 bitstream (128B)
//!
//! ## Phase 2: Full Encoder (15-20 capsules, 6-12 months)
//! - All Phase 1 capsules (foundation)
//! - MotionEstimationCapsule (T2): Inter-frame prediction
//! - TemporalRDOCapsule (T4+T5): Rate-distortion optimization
//! - LookaheadCapsule (T4): 10-40 frame buffer
//! - ReferenceFrameCapsule (T1+T4): Reference management
//! - LoopFilterCapsule (T2): Deblocking
//! - CdefFilterCapsule (T2): 8 directional filters
//! - LrfCapsule (T2): Loop restoration
//! - GopCoordinatorCapsule (T6): GOP structure
//!
//! # Performance Targets (Phase 1)
//! - 1024×1024 encode: <250ms (vs 500ms rav1e)
//! - State query: <50ns
//! - State update: <100ns
//! - Intra prediction: <1μs per block
//! - DCT transform: <500ns per 32×32 block
//! - Quantization: <200ns per block (deterministic Q16.16)
//! - Entropy coding: <2μs per tile
//! - Tile coordination: <5μs parallel dispatch
//!
//! # Framework Compliance
//! - UCE34: Q10 T1-T6 Mixed, Q33 lockfree, Q34 audit trails
//! - Chaos: 100% computational capsules, cache-aligned
//! - ASSUM: 99.99% safe, all assumptions documented
//! - B32: Fair baseline (rav1e), 2-5× speedup target
//! - T28: 28 tests per capsule (unit/property/integration/production)
//! - I20: Zero breaking changes, feature-gated
//!
//! # Trade Secret Protection
//! - AV1 encoder capsule architecture is proprietary
//! - 100% lockfree encoder orchestration (world's first)
//! - DualAtomicU64 coordination patterns for video encoding
//! - NEVER push to public repositories
//! - LOCAL COMMITS ONLY with [TRADE SECRET] tag

pub mod frame_buffer;
pub mod state;
pub mod quantization;
pub mod tile_coordinator;
#[deprecated(since = "0.9.0", note = "Use dct_transform_simd instead - SOTA 2025 Chen-Wang DCT with portable_simd")]
pub mod dct_transform;
pub mod dct_transform_simd; // T2 SIMD tier - SOTA 2025 Chen-Wang DCT with portable_simd
#[deprecated(since = "0.9.0", note = "Use obu_bitstream_v2 instead - SOTA 2025 SIMD-accelerated bit packing (4× faster)")]
pub mod obu_bitstream;
pub mod obu_bitstream_v2; // T5 Streaming tier - SOTA 2025 SIMD-accelerated bit packing (4× faster)
pub mod frame_header_impl; // Spec-compliant AV1 frame header implementation
pub mod sequence_header_impl; // Spec-compliant AV1 sequence header implementation
pub mod entropy_coder;
pub mod entropy_coder_simd; // T2 SIMD tier - SOTA 2025 Daala range coder with AVX2 acceleration
pub mod cdf_adaptation; // T6 Mixed tier - SOTA CDF adaptation with variable rate
pub mod gop_coordinator; // T6 Mixed tier - GOP structure coordination
pub mod gop_coordinator_v2; // T6 Mixed tier - SOTA 2025 GOP coordination (Netflix/SVT-AV1/Google)
#[deprecated(since = "0.9.0", note = "Use reference_frame_v2 instead - SOTA 2025 with improved reference management")]
pub mod reference_frame;
pub mod reference_frame_v2;

// Phase 2 placeholder - file not yet implemented
// #[cfg(feature = "encoder")]
// pub mod superresolution;

#[cfg(feature = "portable_simd")]
#[deprecated(since = "0.9.0", note = "Use intra_prediction_v2 instead - SOTA 2025 with 10-20× faster mode pruning")]
pub mod intra_prediction;

// ============================================================================
// V2 SOTA 2025 Encoder Capsules (Netflix/SVT-AV1/JPEG-XL techniques)
// ============================================================================

// Intra Prediction V2 - SOTA 2025 Fast Mode Pruning (10-20× speedup, 128B)
#[cfg(feature = "portable_simd")]
pub mod intra_prediction_v2;

#[cfg(feature = "portable_simd")]
pub mod loop_filter;

#[cfg(feature = "portable_simd")]
#[deprecated(since = "0.9.0", note = "Use cdef_filter_v2 instead - SOTA 2025 with 8-direction SIMD and noise-adaptive filtering")]
pub mod cdef_filter;

// Loop Restoration Filter (T2 SIMD, 256B)
#[cfg(feature = "portable_simd")]
#[deprecated(since = "0.9.0", note = "Use loop_restoration_v2 instead - SOTA 2025 with integral image O(1) and separable Wiener")]
pub mod lrf;

// CDEF Filter V2 - SOTA 2025 (8-direction SIMD, noise-adaptive, 256B)
#[cfg(feature = "portable_simd")]
pub mod cdef_filter_v2;

// Loop Restoration Filter V2 - SOTA 2025 (integral image O(1), separable Wiener, 512B)
#[cfg(feature = "portable_simd")]
pub mod loop_restoration_v2;

// Film Grain Synthesis (T2 SIMD, 256B)
#[cfg(feature = "portable_simd")]
#[deprecated(since = "0.9.0", note = "Use film_grain_v2 instead - SOTA 2025 with Netflix AR(1), JPEG-XL separable, SVT-AV1 SIMD (10× speedup)")]
pub mod film_grain;

// Film Grain Synthesis V2 - SOTA 2025 (Netflix/JPEG-XL/SVT-AV1, 10× speedup, 256B)
#[cfg(feature = "portable_simd")]
pub mod film_grain_v2;

// Superresolution (T2 SIMD, 256B)
#[cfg(feature = "portable_simd")]
#[deprecated(since = "0.9.0", note = "Use superresolution_v2 instead - SOTA 2025 with 4× speedup")]
pub mod superresolution;

// Superresolution V2 - SOTA 2025 (4× speedup, AOM 2024 spec, 256B)
#[cfg(feature = "portable_simd")]
pub mod superresolution_v2;

// Motion estimation (T7 Heterogeneous, GPU-accelerated, Wave 3)
#[cfg(feature = "portable_simd")]
#[deprecated(since = "0.9.0", note = "Use motion_estimation_v2 instead - SOTA 2025 AVX2 with diamond search")]
pub mod motion_estimation;

// Hierarchical Motion Estimation (T2 SIMD + T4 Batch, 256B, SOTA)
#[cfg(feature = "portable_simd")]
pub mod hierarchical_me;

// Motion Estimation V2 (T2 SIMD + T4 Batch, 512B, SOTA 2025 AVX2)
pub mod motion_estimation_v2;

// ============================================================================
// End V2 SOTA 2025 Capsules
// ============================================================================

// Film grain analysis (T2 SIMD, 256B, Wave 3)
#[cfg(feature = "portable_simd")]
pub mod film_grain_analysis;

pub mod temporal_rdo;
pub mod lookahead;
pub mod tile_parallel; // T4 Batch - Parallel tile encoding (Wave 2)
pub mod rate_control_v2; // T3 Fixed-Point - SOTA 2025 Capped CRF with Q16.16
pub mod rate_control_cbr; // T6 Mixed - CBR rate control with HRD VBV buffer model

// Scene Detection (T6 Mixed: T2 SIMD + T3 Fixed-Point, Wave 2)
#[cfg(feature = "portable_simd")]
pub mod scene_detection;

// T6 Mixed tier metacapsule - AV1 encoder orchestration
pub mod encoder_metacapsule;

// Phase 4: T2+T3 SIMD Q16.16 Quantization (5-10× compound speedup)
#[cfg(feature = "portable_simd")]
pub mod simd_q16_quantization;

// Psychovisual optimization (T6 Mixed: T2 SIMD + T3 Fixed-Point)
#[cfg(feature = "portable_simd")]
pub mod psychovisual;

// Inter-frame prediction (T6 Mixed, requires SIMD for 8-tap filters)
#[cfg(feature = "portable_simd")]
#[deprecated(since = "0.9.0", note = "Use inter_prediction_v2 instead - SOTA 2025 with SIMD 8-tap interpolation")]
pub mod inter_prediction;

// Inter Prediction V2 - SOTA 2025 (SIMD 8-tap, compound, warped motion, 512B)
#[cfg(feature = "portable_simd")]
pub mod inter_prediction_v2;

// Entropy Coder V2 - SOTA 2025 (placeholder - not yet implemented)
// #[cfg(feature = "portable_simd")]
// pub mod entropy_coder_v2;

// DCT Transform V2 - SOTA 2025 (placeholder - not yet implemented)
// #[cfg(feature = "portable_simd")]
// pub mod dct_transform_v2;

// Film grain module already declared above (line 84-85)

pub use frame_buffer::FrameBufferCapsule;
#[allow(deprecated)]
pub use reference_frame::{ReferenceFrameCapsule, ReferenceType};
pub use state::EncoderStateCapsule;
pub use quantization::QuantizationCapsule;
pub use tile_coordinator::{TileCoordinatorCapsule, TileStatus};
#[allow(deprecated)]
pub use dct_transform::DctTransformCapsule;
#[allow(deprecated)]
pub use obu_bitstream::{ObuBitstreamWriterCapsule, ObuType, FrameType, BitWriter};
pub use obu_bitstream_v2::ObuBitstreamCapsuleV2;
pub use entropy_coder::EntropyCoderCapsule;
pub use entropy_coder_simd::{EntropyCoderCapsuleSIMD, CoefficientContexts as CoefficientContextsSIMD};
pub use gop_coordinator::{
    GopCoordinatorCapsule, GopCoordinatorCapsuleV3, FrameType as GopFrameType,
    Av1RefFrame, GopLookupEntry,
};
pub use gop_coordinator_v2::{GopCoordinatorCapsuleV2, FrameType as GopFrameTypeV2, GopMode};
pub use cdf_adaptation::{CDFAdaptationCapsule, CdfContextType, AdaptationMode};

#[cfg(feature = "portable_simd")]
#[allow(deprecated)]
pub use intra_prediction::{IntraPredictionCapsule, IntraMode};

// Intra Prediction V2 - SOTA 2025 Fast Mode Pruning
#[cfg(feature = "portable_simd")]
pub use intra_prediction_v2::{
    IntraPredictionCapsule as IntraPredictionCapsuleV2,
    IntraMode as IntraModeV2,
    ModeGroup,
};

#[cfg(feature = "portable_simd")]
pub use loop_filter::{LoopFilterCapsule, FilterType, EdgeType};

#[cfg(feature = "portable_simd")]
#[allow(deprecated)]
pub use cdef_filter::{
    CdefFilterCapsule, DIR_VERTICAL, DIR_HORIZONTAL, DIR_DIAGONAL_45, DIR_DIAGONAL_135,
};

// Loop Restoration Filter (T2 SIMD, 256B)
#[cfg(feature = "portable_simd")]
#[allow(deprecated)]
pub use lrf::{LrfCapsule, RestorationType};

// CDEF Filter V2 - SOTA 2025
#[cfg(feature = "portable_simd")]
pub use cdef_filter_v2::{
    CdefFilterCapsuleV2, DIR_VERTICAL as V2_DIR_VERTICAL, DIR_HORIZONTAL as V2_DIR_HORIZONTAL,
    DIR_DIAGONAL_45 as V2_DIR_DIAGONAL_45, DIR_DIAGONAL_135 as V2_DIR_DIAGONAL_135,
};

// Loop Restoration Filter V2 - SOTA 2025
#[cfg(feature = "portable_simd")]
pub use loop_restoration_v2::{
    LoopRestorationCapsuleV2, RestorationType as RestorationTypeV2, RESTORATION_UNIT_SIZE,
};

// Motion estimation (T7 Heterogeneous, Wave 3)
#[cfg(feature = "portable_simd")]
#[allow(deprecated)]
pub use motion_estimation::{
    MotionEstimationCapsule, MotionVector as MeMotionVector, SearchAlgorithm, SubPixelMode, BlockSize,
};

// Hierarchical Motion Estimation (T2 SIMD + T4 Batch, SOTA)
#[cfg(feature = "portable_simd")]
pub use hierarchical_me::{
    HierarchicalMECapsule, MotionVector as HierarchicalMV, SearchMethod, SubpelMode,
};

// Motion Estimation V2 (T2 SIMD + T4 Batch, SOTA 2025 AVX2)
pub use motion_estimation_v2::{
    MotionEstimationCapsuleV2, MotionVector as MEv2MotionVector, DiamondSearchIterator,
};

// Film grain analysis (T2 SIMD, Wave 3)
#[cfg(feature = "portable_simd")]
pub use film_grain_analysis::{FilmGrainAnalysisCapsule, ScalingPoint as GrainScalingPoint};

pub use temporal_rdo::{
    TemporalRDOCapsule, FrameType as RdoFrameType, RdCandidate,
    Candidate, MotionVector as RdoMotionVector, IntraMode as RdoIntraMode, TxSize, PartitionType,
};
// Re-export plain MotionVector for benchmarks
pub use temporal_rdo::MotionVector;

// Lookahead for scene detection (T5 Streaming)
pub use lookahead::{
    LookaheadCapsule, FrameType as LookaheadFrameType,
    DEFAULT_SCENE_THRESHOLD, MAX_LOOKAHEAD_DEPTH,
};

// Tile parallel encoding (T4 Batch)
pub use tile_parallel::TileParallelEncoderCapsule;

// Rate control v2 (T3 Fixed-Point - SOTA 2025 Capped CRF)
pub use rate_control_v2::{RateControlCapsule, RateControlMode};

// CBR Rate control (T6 Mixed - HRD VBV buffer model)
pub use rate_control_cbr::CbrRateControlCapsule;

// DCT Transform SIMD (SOTA 2025 Chen-Wang DCT with butterfly operations)
#[cfg(feature = "portable_simd")]
pub use dct_transform_simd::{
    DctTransformCapsule as DctTransformCapsuleSIMD,
    TransformType as DctTransformType,
    TransformSize as DctTransformSize,
};

// T6 Mixed tier metacapsule - AV1 encoder orchestration
pub use encoder_metacapsule::{Av1EncoderMetacapsule, EncoderPhase, EncoderState as MetacapsuleEncoderState};

// Inter-frame prediction (T6 Mixed, requires SIMD)
#[cfg(feature = "portable_simd")]
#[allow(deprecated)]
pub use inter_prediction::{
    InterPredictionCapsule, CompoundMode, MotionMode, InterpolationFilter, MotionVector as InterMotionVector,
};

// Inter Prediction V2 - SOTA 2025 (SIMD 8-tap, compound, warped motion)
#[cfg(feature = "portable_simd")]
pub use inter_prediction_v2::{
    InterPredictionCapsuleV2,
    CompoundMode as CompoundModeV2,
    MotionMode as MotionModeV2,
    InterpolationFilter as InterpolationFilterV2,
    MotionVector as InterMotionVectorV2,
};

// Superresolution (T2 SIMD, 256B)
#[cfg(feature = "portable_simd")]
#[allow(deprecated)]
pub use superresolution::SuperresolutionCapsule;

// Superresolution V2 - SOTA 2025 (4× speedup, AOM 2024 spec, 256B)
#[cfg(feature = "portable_simd")]
pub use superresolution_v2::SuperresolutionCapsuleV2;

// Reference Frame V2 - SOTA 2025
pub use reference_frame_v2::{
    ReferenceFrameCapsuleV2,
    ReferenceTypeV2,
};

// Motion Estimation V2 - SOTA 2025 (already exported above at line 238-240)
// OBU Bitstream V2 - SOTA 2025 (already exported above at line 182)
// GOP Coordinator V2 - SOTA 2025 (already exported above at line 186)

// Film Grain Synthesis (T2 SIMD, 256B)
#[cfg(feature = "portable_simd")]
#[allow(deprecated)]
pub use film_grain::{FilmGrainCapsule, ScalingPoint};

// Film Grain Synthesis V2 - SOTA 2025 (Netflix/JPEG-XL/SVT-AV1, 10× speedup)
#[cfg(feature = "portable_simd")]
pub use film_grain_v2::{
    FilmGrainCapsuleV2,
    ScalingPoint as ScalingPointV2,
};

// Psychovisual optimization (T6 Mixed: T2 SIMD + T3 Fixed-Point)
#[cfg(feature = "portable_simd")]
pub use psychovisual::{PsychovisualCapsule, Q8_8, Q16_16};

// Scene Detection (T6 Mixed: T2 SIMD + T3 Fixed-Point, Wave 2)
#[cfg(feature = "portable_simd")]
pub use scene_detection::{SceneDetectionCapsule, SceneDetectionStats};

// Phase 4: T2+T3 SIMD Q16.16 Quantization
#[cfg(feature = "portable_simd")]
pub use simd_q16_quantization::{
    quantize_block_simd_q16,
    compute_rd_cost_simd_q16,
};

/// AV1 encoder formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PixelFormat {
    /// YUV 4:2:0 (most common, half resolution chroma)
    Yuv420 = 0,
    /// YUV 4:2:2 (broadcast, half horizontal chroma)
    Yuv422 = 1,
    /// YUV 4:4:4 (full resolution chroma)
    Yuv444 = 2,
    /// Monochrome (grayscale only)
    Monochrome = 3,
}

/// Encoder speed presets (0 = slowest/best, 10 = fastest/lower quality)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum SpeedPreset {
    /// Slowest, best quality (for archival)
    Slowest = 0,
    /// Very slow, high quality
    VerySlow = 1,
    /// Slow, high quality
    Slow = 2,
    /// Medium-slow, good quality
    MediumSlow = 3,
    /// Medium, balanced (default)
    Medium = 4,
    /// Medium-fast, good speed
    MediumFast = 5,
    /// Fast, acceptable quality
    Fast = 6,
    /// Very fast, lower quality
    VeryFast = 7,
    /// Faster, low quality
    Faster = 8,
    /// Very fast, minimal quality
    VeryFaster = 9,
    /// Fastest, lowest quality (for previews)
    Fastest = 10,
}

impl Default for SpeedPreset {
    fn default() -> Self {
        SpeedPreset::Medium
    }
}

/// Encoder quality mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum QualityMode {
    /// Constant Quality (CQ) - target visual quality
    ConstantQuality = 0,
    /// Constant Bitrate (CBR) - target bitrate
    ConstantBitrate = 1,
    /// Variable Bitrate (VBR) - average bitrate
    VariableBitrate = 2,
    /// Lossless - no quality loss
    Lossless = 3,
}

impl Default for QualityMode {
    fn default() -> Self {
        QualityMode::ConstantQuality
    }
}

/// Encoder state (internal coordination)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EncoderState {
    /// Idle, ready to encode
    Idle = 0,
    /// Encoding in progress
    Encoding = 1,
    /// Flushing final frames
    Flushing = 2,
    /// Completed, all frames encoded
    Completed = 3,
    /// Error state
    Error = 4,
}

impl Default for EncoderState {
    fn default() -> Self {
        EncoderState::Idle
    }
}

/// Error types for encoder operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderError {
    /// Invalid configuration (dimensions, speed, etc.)
    InvalidConfig,
    /// Buffer overflow (frame queue full)
    BufferOverflow,
    /// Invalid state transition
    InvalidState,
    /// Encoding failed
    EncodingFailed,
    /// Bitstream write failed
    BitstreamError,
}

impl core::fmt::Display for EncoderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EncoderError::InvalidConfig => write!(f, "Invalid encoder configuration"),
            EncoderError::BufferOverflow => write!(f, "Frame buffer overflow"),
            EncoderError::InvalidState => write!(f, "Invalid encoder state transition"),
            EncoderError::EncodingFailed => write!(f, "Encoding operation failed"),
            EncoderError::BitstreamError => write!(f, "Bitstream write error"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EncoderError {}
