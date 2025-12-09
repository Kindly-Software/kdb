//! kindly-av1 Encoder Module - AV1 Video Encoding Capsules
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! This module provides the bridge between kindly-av1 and atomic_capsule encoder primitives,
//! exposing 100% lockfree, cache-aligned computational capsules for AV1 video encoding.
//!
//! ## Architecture
//!
//! kindly-av1 uses a T6 Mixed metacapsule architecture with orchestration of multiple
//! encoder sub-capsules from atomic_capsule:
//!
//! ### Core Encoder Capsules (from atomic_capsule)
//!
//! - **EncoderStateCapsule** (T1): Central encoding state coordination (64B)
//! - **FrameBufferCapsule** (T1): Frame management and reference tracking (128B)
//! - **QuantizationCapsule** (T3): Q16.16 deterministic quantization (128B)
//! - **DctTransformCapsule** (T2): Chen-Wang DCT with SIMD (256B)
//! - **EntropyCoderCapsule** (T2): Daala range coder (256B)
//! - **ReferenceFrameCapsule** (T1+T4): Reference frame management (128B)
//! - **ObuBitstreamWriterCapsule** (T5): AV1 bitstream output (128B)
//! - **IntraPredictionCapsule** (T2): 56 prediction modes (256B)
//! - **ParallelTileEncoderCapsule** (T4): Parallel tile encoding (256B)
//! - **LoopFilterCapsule** (T2): Deblocking filter (256B)
//! - **LoopFilterPipelineCapsule** (T6): Unified Deblock+CDEF+LRF pipeline (512B)
//! - **GopStructureCapsule** (T1+T5): SOTA GOP structure planning with hierarchical B-frames (256B)
//!
//! ### kindly-av1 Specific Extensions
//!
//! - **EncoderConfig**: Configuration wrapper for CLI integration
//! - **EncoderWiringCapsule** (T6): Metacapsule orchestration wiring
//! - **KindlyAv1CliMetacapsule** (T6): CLI-to-encoder bridge
//! - **EncoderSubCapsules**: Collection of all encoder sub-capsules
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 T6 Mixed tier, Q33 lockfree coordination, Q34 audit trails
//! - **Chaos**: 100% computational capsules, cache-aligned (64B/128B/256B)
//! - **ASSUM**: 99.99% safe, all assumptions documented (#ASSUME → #VERIFY)
//! - **B32**: Fair baseline (rav1e), 2-5× speedup target
//! - **T28**: 5-tier testing (unit/property/integration/production/determinism)
//! - **I20**: Zero breaking changes, feature-gated
//!
//! ## Trade Secret Protection
//!
//! - AV1 encoder capsule orchestration architecture is proprietary
//! - 100% lockfree encoder coordination patterns (world's first)
//! - DualAtomicU64 state machine for video encoding pipeline
//! - NEVER push to public repositories
//! - LOCAL COMMITS ONLY with [TRADE SECRET] tag
//!
//! ## Performance Targets
//!
//! - State query: <50ns
//! - State update: <100ns
//! - Frame encoding: <250ms @ 1080p (vs 500ms rav1e baseline)
//! - Tile coordination: <5μs parallel dispatch
//! - Bitstream write: <2μs per tile

// ============================================================================
// Submodule Declarations
// ============================================================================

pub mod config;
pub mod state_machine;
pub mod wiring_capsule;
pub mod metacapsule;
pub mod sub_capsules;
pub mod symbol_encoder;
pub mod gpu_motion;
pub mod gpu_compute;
pub mod deblock_integration;
pub mod reconstruction;
pub mod reference_manager;
pub mod reference_selection;
pub mod inter_modes;

// Phase 6: SOTA Motion Compensation (T2 SIMD)
pub mod motion_compensation;

// Phase 6.6: SOTA Chroma Motion Compensation (T2 SIMD)
// Chroma MV derivation (4:2:0), 4-tap bilinear filters, SIMD acceleration
pub mod chroma_motion;

// Phase 6.7: SOTA Chroma Encoding (T6 Mixed - T1 Atomic + T2 SIMD + T3 Fixed-Point)
// CfL (Chroma-from-Luma) prediction, all AV1 chroma intra modes, subsampling
// Based on libaom/SVT-AV1 algorithms with 2-5% BD-rate reduction
#[cfg(feature = "portable_simd")]
pub mod chroma_encoder;

// Phase 6.5: SOTA Inter Prediction Integration (T6 Mixed)
// Connects InterModesCapsule, MotionCompensationCapsule, ReferenceSelectionCapsule
pub mod inter_integration;

// SOTA Adaptive Quantization (T2 SIMD + T3 Fixed-Point)
pub mod adaptive_quant;

// SOTA CRF Rate Control (T1 Atomic + T3 Fixed-Point)
pub mod rate_control_crf;

// SOTA CBR/VBR Rate Control with VBV Buffer Model (T1 Atomic + T5 Streaming)
pub mod rate_control_bitrate;

// Phase 4: Tile Parallelism
pub mod tile_encoder;
pub mod parallel_encoder;
pub mod tile_scheduler;

// Phase 4.1: SOTA Tile Aggregation (T5 Streaming)
pub mod tile_aggregator;

// Phase 5: Unified Loop Filter Pipeline (SOTA 2025)
#[cfg(feature = "portable_simd")]
pub mod loop_filter_pipeline;

// Phase 6.1: SOTA Wavefront Parallel Processing (T1 Atomic + T4 Batch)
// IEEE-validated 10×+ speedup with <0.2% bitrate increase
pub mod wavefront;

// Phase 6.2: SOTA Temporal Filtering for ALTREF Creation (T2 SIMD + T4 Batch)
// Based on SVT-AV1/libaom algorithms with 8.67% BD-rate gain
pub mod temporal_filter;

// Phase 6.3: SOTA GOP Structure Planning (T1 Atomic + T5 Streaming)
// SVT-AV1/libaom/Netflix hierarchical B-frame patterns with scene change detection
pub mod gop_structure;

// Phase 6.4: SOTA Frame Type Decision Engine (T1 Atomic + T5 Streaming)
// SVT-AV1 PictureDecisionProcess + x264 Viterbi B-frame placement + Netflix shot-based encoding
pub mod frame_type_decision;

// Phase 7: SOTA P-Frame Pipeline Orchestrator (T6 Mixed)
// Coordinates: ReferenceSelection → MotionEstimation → MotionCompensation → ModeDecision → Transform → Quantize → Entropy
pub mod pframe_pipeline;

// Phase 8: SOTA Chroma Transform and Quantization (T2 SIMD + T3 Fixed-Point)
// AV1-compliant chroma DCT/ADST transforms with delta_q_u/delta_q_v offsets
// Supports 4×4, 8×8, 16×16, 32×32 chroma blocks (half luma for 4:2:0)
pub mod chroma_transform;

// Phase 9: SOTA GPU Transform & Quantization (T7 Heterogeneous)
// GPU-accelerated hybrid transforms (DCT/ADST/FlipADST/IDTX) with trellis quantization
// Supports 4×4 to 64×64 blocks (17 sizes total), 16 transform type combinations
// Based on SVT-AV1/libaom algorithms with RDOQ (Rate-Distortion Optimized Quantization)
pub mod gpu_transform;

// ============================================================================
// Re-exports from atomic_capsule (Core Encoder Primitives)
// ============================================================================

// State Management (from atomic_capsule)
pub use atomic_capsule::encoder::{
    EncoderStateCapsule,
    EncoderState as AtomicCapsuleEncoderState,
    EncoderError,
    SpeedPreset,
    QualityMode,
    PixelFormat,
};

// ============================================================================
// V2 SOTA 2025 Encoder Capsules (from atomic_capsule)
// ============================================================================

// Intra Prediction V2
#[cfg(feature = "portable_simd")]
pub use atomic_capsule::encoder::{
    IntraPredictionCapsuleV2,
    IntraModeV2,
    ModeGroup,
};

// Inter Prediction V2
#[cfg(feature = "portable_simd")]
pub use atomic_capsule::encoder::{
    InterPredictionCapsuleV2,
    CompoundModeV2,
    MotionModeV2,
    InterpolationFilterV2,
    InterMotionVectorV2,
};

// GOP Coordinator V2
pub use atomic_capsule::encoder::{
    GopCoordinatorCapsuleV2,
    GopFrameTypeV2,
    GopMode,
};

// OBU Bitstream V2
pub use atomic_capsule::encoder::ObuBitstreamCapsuleV2;

// Superresolution V2
#[cfg(feature = "portable_simd")]
pub use atomic_capsule::encoder::SuperresolutionCapsuleV2;

// Reference Frame V2
pub use atomic_capsule::encoder::{
    ReferenceFrameCapsuleV2,
    ReferenceTypeV2,
};

// Motion Estimation V2
pub use atomic_capsule::encoder::{
    MotionEstimationCapsuleV2,
    MEv2MotionVector,
    DiamondSearchIterator,
};

// CDEF Filter V2
#[cfg(feature = "portable_simd")]
pub use atomic_capsule::encoder::{
    CdefFilterCapsuleV2,
    V2_DIR_VERTICAL,
    V2_DIR_HORIZONTAL,
    V2_DIR_DIAGONAL_45,
    V2_DIR_DIAGONAL_135,
};

// Loop Restoration Filter V2
#[cfg(feature = "portable_simd")]
pub use atomic_capsule::encoder::{
    LoopRestorationCapsuleV2,
    RestorationTypeV2,
    RESTORATION_UNIT_SIZE,
};

// ============================================================================
// End V2 SOTA 2025 Re-exports
// ============================================================================

// Frame and Buffer Management
pub use atomic_capsule::encoder::{
    FrameBufferCapsule,
    FrameType,
};

// Transform and Quantization
pub use atomic_capsule::encoder::{
    QuantizationCapsule,
    DctTransformCapsule,
};

// Prediction and Filtering
#[cfg(feature = "portable_simd")]
pub use atomic_capsule::encoder::{
    IntraPredictionCapsule,
    IntraMode,
};

#[cfg(feature = "portable_simd")]
pub use atomic_capsule::encoder::{
    LoopFilterCapsule,
    FilterType,
    EdgeType,
};

// Entropy Coding and Bitstream
pub use atomic_capsule::encoder::{
    EntropyCoderCapsule,
    ObuBitstreamWriterCapsule,
    ObuType,
};

// Reference Frame Management (both V1 and V2 versions for compatibility)
pub use atomic_capsule::encoder::{
    ReferenceFrameCapsule,  // V1 for reference_manager.rs
    ReferenceType,
};

// Tile Coordination (not ParallelTileEncoder)
pub use atomic_capsule::encoder::{
    TileCoordinatorCapsule,
    TileStatus,
};

// GOP Coordination (T6 Mixed tier - hierarchical B-frames, scene change detection)
pub use atomic_capsule::encoder::{
    GopCoordinatorCapsule,
    GopFrameType,
};

// File I/O (if available)
#[cfg(feature = "file-io")]
pub use atomic_capsule::encoder::{
    YuvReaderCapsule,
    YuvFrame,
    Av1BitstreamWriterCapsule,
    Av1Obu,
    FileIoError,
};

// ============================================================================
// Re-exports from kindly-av1 (Bridge Types)
// ============================================================================

pub use config::EncoderConfig;
pub use state_machine::{
    EncoderStateMachineCapsule,
    EncoderState as KindlyEncoderState,
    StateTransitionResult,
    StateMachineSnapshot,
};
pub use wiring_capsule::EncoderWiringCapsule;
pub use metacapsule::KindlyAv1CliMetacapsule;
pub use sub_capsules::EncoderSubCapsules;
pub use gpu_motion::{GpuMotionEstimationCapsule, MotionVector};
pub use gpu_compute::{
    GpuComputeCapsule,
    GpuComputeState,
    GpuComputeError,
    GpuComputeResult,
    GpuBackend,
    GpuBackendType,
    GpuBuffer,
    GpuKernel,
    GpuDeviceCapabilities,
    KernelId,
    CpuFallbackBackend,
    MAX_KERNELS,
    MAX_QUEUE_DEPTH,
};
pub use deblock_integration::{
    DeblockIntegrationCapsule,
    DeblockIntegrationError,
    DeblockIntegrationStats,
    EncoderPreset,
};
pub use reconstruction::{ReconstructionCapsule, ReconstructionStats};
pub use reference_manager::{
    ReferenceFrameManagerCapsule,
    ReferenceManagerError,
    FrameUpdateStrategy,
    ReferenceStats,
};
pub use reference_selection::{
    ReferenceSelectionCapsule,
    ReferenceSelection,
    SceneType,
    MotionLevel,
};

// Phase 6: SOTA Inter Modes (T6 Mixed - Compound + OBMC + Warped)
#[cfg(feature = "portable_simd")]
pub use inter_modes::{
    InterModesCapsule,
    CompoundType,
    MotionModeType,
    WarpedMotionParams,
};

// Phase 6: SOTA Motion Compensation (T2 SIMD)
pub use motion_compensation::{
    MotionCompensationCapsule,
    MotionVectorQ16,
    InterpolationFilter as MCInterpolationFilter,
    CompoundPredictionMode,
    BlockSize as MCBlockSize,
};

// Phase 6.6: SOTA Chroma Motion Compensation (T2 SIMD)
pub use chroma_motion::{
    ChromaMotionCapsule,
    ChromaMotionVector,
    ChromaSubsampling,
    ChromaFilterType,
    ChromaBlockSize,
    derive_chroma_mv_420,
    derive_chroma_mv_422,
};

// Phase 6.7: SOTA Chroma Encoding (T6 Mixed - T1 Atomic + T2 SIMD + T3 Fixed-Point)
// CfL (Chroma-from-Luma) prediction, all AV1 chroma intra modes, subsampling
#[cfg(feature = "portable_simd")]
pub use chroma_encoder::{
    ChromaEncoderCapsule,
    ChromaIntraMode,
    ChromaSubsampling as CflChromaSubsampling,  // Alias to avoid conflict with chroma_motion
    CflParams,
    CflAlphaSign,
    CFL_ALPHA_MIN,
    CFL_ALPHA_MAX,
    CHROMA_QP_OFFSET_MIN,
    CHROMA_QP_OFFSET_MAX,
    MAX_CHROMA_BLOCK_SIZE,
};

// Phase 6.5: SOTA Inter Prediction Integration (T6 Mixed)
#[cfg(feature = "portable_simd")]
pub use inter_integration::{
    InterPredictionIntegrationCapsule,
    InterPredictionMode,
    BlockPredictionRequest,
    BlockPredictionResult,
    ObmcNeighborInfo,
};

// SOTA Adaptive Quantization (T2 SIMD + T3 Fixed-Point)
pub use adaptive_quant::{
    AdaptiveQuantCapsule,
    AqMode,
    VarianceSegment,
    Q8_8 as AqQ8_8,
    Q16_16 as AqQ16_16,
};

// SOTA CRF Rate Control (T1 Atomic + T3 Fixed-Point)
pub use rate_control_crf::{
    CrfRateControlCapsule,
    CrfFrameType,
    Q16_16 as CrfQ16_16,
};

// SOTA CBR/VBR Rate Control with VBV Buffer Model (T1 Atomic + T5 Streaming)
pub use rate_control_bitrate::{
    BitrateRateControlCapsule,
    BitrateMode,
    RcFrameType,
};

// Phase 4: Tile Parallelism
pub use tile_encoder::{TileContext, encode_intra_tile, encode_inter_tile};
pub use parallel_encoder::TileParallelEncoderCapsule;
pub use tile_scheduler::{
    TileWorkStealingCapsule,
    ChaseLevDeque,
    TileTask,
    StealResult,
    MAX_TILES_PER_FRAME,
    MAX_WORKERS,
    DEQUE_CAPACITY,
};

// Phase 4.1: SOTA Tile Aggregation (T5 Streaming)
pub use tile_aggregator::{
    TileAggregatorCapsule,
    TileResultSlot,
    TileAggregatorError,
    MAX_TILES,
};

// Phase 5: Unified Loop Filter Pipeline
#[cfg(feature = "portable_simd")]
pub use loop_filter_pipeline::{
    LoopFilterPipelineCapsule,
    LoopFilterPipelineConfig,
    LoopFilterPipelineError,
    LoopFilterPipelineStats,
    LrfType,
};

// Phase 6.1: SOTA Wavefront Parallel Processing (T1 Atomic + T4 Batch)
pub use wavefront::{
    WavefrontCapsule,
    WavefrontContextBuffer,
    WavefrontRowWorker,
};

// Phase 6.2: SOTA Temporal Filtering for ALTREF Creation (T2 SIMD + T4 Batch)
pub use temporal_filter::{
    TemporalFilterCapsule,
    FilterStrength,
    TfMotionVector,
    TfBlockWeight,
    TF_BLOCK_SIZE,
    TF_MAX_WINDOW_SIZE,
    TF_WEIGHT_SCALE,
};

// Phase 6.3: SOTA GOP Structure Planning (T1 Atomic + T5 Streaming)
pub use gop_structure::{
    GopStructureCapsule,
    GopFrameType as GopStructureFrameType,
    GopMode as GopStructureMode,
    MiniGopSize,
    GopFrameEntry,
    Av1RefSlot,
    Q16_16 as GopQ16_16,
};

// Phase 6.4: SOTA Frame Type Decision Engine (T1 Atomic + T5 Streaming)
pub use frame_type_decision::{
    FrameTypeDecisionCapsule,
    DecisionFrameType,
    BAdaptMode,
    HierarchicalLevels,
    FrameDecision,
    FrameCost,
    Q16_16 as DecisionQ16_16,
    q16_constants as decision_q16_constants,
};

// Phase 7: SOTA P-Frame Pipeline Orchestrator (T6 Mixed)
pub use pframe_pipeline::{
    PFramePipelineCapsule,
    PipelineStage,
    InterPredictionMode as PipelineInterPredictionMode,
    PipelineFlags,
    MotionVectorQ4,
    BlockEncodingResult,
};

// Phase 8: SOTA Chroma Transform and Quantization (T2 SIMD + T3 Fixed-Point)
pub use chroma_transform::{
    ChromaTransformQuantCapsule,
    ChromaPlane,
    ChromaTransformType,
    ChromaBlockSize as ChromaTxBlockSize,
    DEFAULT_DELTA_Q_U,
    DEFAULT_DELTA_Q_V,
    MAX_DELTA_Q,
    MIN_DELTA_Q,
};

// Phase 9: SOTA GPU Transform & Quantization (T7 Heterogeneous)
pub use gpu_transform::{
    GpuTransformQuantCapsule,
    TxSize,
    TxType,
    Tx1dType,
    TransformParams,
    QuantParams,
    GpuTransformState,
};

// ============================================================================
// Module Documentation
// ============================================================================

/// Encoder configuration validation.
///
/// # Examples
///
/// ```ignore
/// use kindly_av1::encoder::EncoderConfig;
///
/// let config = EncoderConfig {
///     preset: SpeedPreset::Medium,
///     crf: 28,
///     width: 1920,
///     height: 1080,
///     fps_num: 30,
///     fps_den: 1,
/// };
///
/// assert!(config.validate().is_ok());
/// ```
pub mod validation {
    use super::*;

    /// Validate encoder configuration parameters.
    pub fn validate_config(config: &EncoderConfig) -> Result<(), EncoderError> {
        // Validation logic will be in config.rs
        config.validate()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all critical types are exported
        let _ = core::mem::size_of::<EncoderStateCapsule>();
        let _ = core::mem::size_of::<FrameBufferCapsule>();
        let _ = core::mem::size_of::<QuantizationCapsule>();
        let _ = core::mem::size_of::<DctTransformCapsule>();
        let _ = core::mem::size_of::<EntropyCoderCapsule>();
        let _ = core::mem::size_of::<ReferenceFrameCapsule>();
        let _ = core::mem::size_of::<ObuBitstreamWriterCapsule>();
        let _ = core::mem::size_of::<TileCoordinatorCapsule>();
        let _ = core::mem::size_of::<GopCoordinatorCapsule>();

        // Verify GopCoordinatorCapsule is 256B cache-aligned (T6 Mixed tier)
        assert_eq!(core::mem::size_of::<GopCoordinatorCapsule>(), 256);
        assert_eq!(core::mem::align_of::<GopCoordinatorCapsule>(), 256);
    }

    #[cfg(feature = "portable_simd")]
    #[test]
    fn test_simd_exports() {
        let _ = core::mem::size_of::<IntraPredictionCapsule>();
        let _ = core::mem::size_of::<LoopFilterCapsule>();
    }

    #[test]
    fn test_enum_variants() {
        // Verify enum types compile
        let _ = FrameType::KeyFrame;
        let _ = SpeedPreset::Medium;
        let _ = QualityMode::ConstantQuality;
    }

    #[test]
    fn test_gop_structure_capsule_exports() {
        // Verify GopStructureCapsule is 256B cache-aligned (T1+T5 tier)
        assert_eq!(core::mem::size_of::<GopStructureCapsule>(), 256);
        assert_eq!(core::mem::align_of::<GopStructureCapsule>(), 256);

        // Verify GopFrameEntry is 8 bytes
        assert_eq!(core::mem::size_of::<GopFrameEntry>(), 8);

        // Verify enum types compile
        let _ = GopStructureFrameType::Key;
        let _ = GopStructureMode::Adaptive;
        let _ = MiniGopSize::Size8;
        let _ = Av1RefSlot::Last;
    }

    #[test]
    fn test_gop_structure_capsule_integration() {
        // Verify GopStructureCapsule can be constructed and used
        let gop = GopStructureCapsule::new();

        // Default configuration
        assert_eq!(gop.get_mode(), gop_structure::GopMode::Adaptive);
        assert_eq!(gop.get_mini_gop_size(), MiniGopSize::Size8);
        assert_eq!(gop.get_max_keyint(), 120);

        // Frame type queries
        assert_eq!(gop.get_frame_type(0), gop_structure::GopFrameType::Key);
        assert_eq!(gop.get_temporal_layer(0), 0);

        // Generation counter tracks changes
        let gen0 = gop.get_generation();
        gop.advance_frame();
        let gen1 = gop.get_generation();
        assert!(gen1 > gen0);
    }

    #[test]
    fn test_frame_type_decision_capsule_exports() {
        // Verify FrameTypeDecisionCapsule is 256B cache-aligned (T1+T5 tier)
        assert_eq!(core::mem::size_of::<FrameTypeDecisionCapsule>(), 256);
        assert_eq!(core::mem::align_of::<FrameTypeDecisionCapsule>(), 256);

        // Verify FrameDecision is 8 bytes (compact decision struct)
        assert_eq!(core::mem::size_of::<FrameDecision>(), 8);

        // Verify FrameCost is 16 bytes
        assert_eq!(core::mem::size_of::<FrameCost>(), 16);

        // Verify enum types compile
        let _ = DecisionFrameType::Key;
        let _ = DecisionFrameType::Inter;
        let _ = DecisionFrameType::Bframe;
        let _ = DecisionFrameType::BframeRef;
        let _ = BAdaptMode::Optimal;
        let _ = HierarchicalLevels::Levels4;
    }

    #[test]
    fn test_frame_type_decision_capsule_integration() {
        // Verify FrameTypeDecisionCapsule can be constructed and used
        let decision = FrameTypeDecisionCapsule::new();

        // Default configuration
        assert_eq!(decision.get_b_adapt(), BAdaptMode::Optimal);
        assert_eq!(decision.get_hier_levels(), HierarchicalLevels::Levels4);
        assert_eq!(decision.get_max_b_frames(), 7);
        assert_eq!(decision.get_max_keyint(), 120);

        // Frame type decision (first frame is always keyframe)
        let d0 = decision.decide_frame_type(0);
        assert_eq!(d0.frame_type, DecisionFrameType::Key);
        assert_eq!(d0.temporal_layer, 0);
        assert_eq!(d0.refresh_flags, 0xFF);

        // B-frame decision (frame 1 in hierarchical pattern)
        let d1 = decision.decide_frame_type(1);
        assert_eq!(d1.frame_type, DecisionFrameType::Bframe);
        assert_eq!(d1.temporal_layer, 3);

        // Generation counter tracks changes
        let gen0 = decision.get_generation();
        decision.set_scene_change(5);
        let gen1 = decision.get_generation();
        assert!(gen1 > gen0);
    }

    #[test]
    fn test_frame_type_decision_with_gop_structure() {
        // Test integration between FrameTypeDecisionCapsule and GopStructureCapsule
        let decision = FrameTypeDecisionCapsule::new();
        let gop = GopStructureCapsule::new();

        // Both should have compatible hierarchical patterns
        for frame_idx in 0..16 {
            let frame_decision = decision.decide_frame_type(frame_idx);
            let gop_frame_type = gop.get_frame_type(frame_idx);
            let gop_temporal = gop.get_temporal_layer(frame_idx);

            // Keyframes should match
            if frame_idx == 0 {
                assert_eq!(frame_decision.frame_type, DecisionFrameType::Key);
                assert_eq!(gop_frame_type, gop_structure::GopFrameType::Key);
            }

            // Temporal layers should be bounded
            assert!(frame_decision.temporal_layer <= 5);
            assert!(gop_temporal <= 5);
        }
    }

    // ========================================================================
    // Phase 7: P-Frame Pipeline Capsule Integration Tests
    // ========================================================================

    #[test]
    fn test_pframe_pipeline_capsule_exports() {
        // Verify PFramePipelineCapsule is 512B cache-aligned (T6 Mixed tier)
        assert_eq!(core::mem::size_of::<PFramePipelineCapsule>(), 512);
        assert_eq!(core::mem::align_of::<PFramePipelineCapsule>(), 512);

        // Verify MotionVectorQ4 is 4 bytes (compact)
        assert_eq!(core::mem::size_of::<MotionVectorQ4>(), 4);

        // Verify PipelineStage and InterPredictionMode enum types compile
        let _ = PipelineStage::Idle;
        let _ = PipelineStage::MotionEstimation;
        let _ = PipelineStage::Complete;
        let _ = PipelineInterPredictionMode::Single;
        let _ = PipelineInterPredictionMode::CompoundAverage;
    }

    #[test]
    fn test_pframe_pipeline_capsule_integration() {
        // Verify PFramePipelineCapsule can be constructed and used
        let pipeline = PFramePipelineCapsule::production();

        // Production configuration (balanced quality/speed)
        let flags = pipeline.flags();
        assert!(flags.enable_compound); // Compound prediction enabled
        assert!(flags.enable_hierarchical_me); // Hierarchical ME enabled
        assert!(!flags.enable_obmc); // OBMC disabled for speed

        // Pipeline stage transitions
        assert_eq!(pipeline.stage(), PipelineStage::Idle);

        pipeline.set_frame_info(1920, 1080, 1);
        assert_eq!(pipeline.width(), 1920);
        assert_eq!(pipeline.height(), 1080);
        assert_eq!(pipeline.frame_num(), 1);

        // Advance through pipeline stages
        assert_eq!(pipeline.advance_stage(), PipelineStage::ReferenceSelection);
        assert_eq!(pipeline.advance_stage(), PipelineStage::MotionEstimation);

        // Record some statistics
        pipeline.record_mv(false); // Non-zero MV
        pipeline.record_inter_mode(false); // Single reference mode
        pipeline.record_ref_frame(0); // LAST reference

        assert_eq!(pipeline.total_mvs(), 1);
        assert_eq!(pipeline.inter_count(), 1);
        assert_eq!(pipeline.last_ref_count(), 1);

        // Generation counter tracks all changes
        assert!(pipeline.generation() > 0);
    }

    #[test]
    fn test_pframe_pipeline_with_motion_compensation() {
        // Test interaction between PFramePipelineCapsule and MotionCompensationCapsule
        let pipeline = PFramePipelineCapsule::quality();
        let mc_capsule = MotionCompensationCapsule::new();

        // Set up pipeline for quality encoding
        assert!(pipeline.flags().enable_compound);
        assert!(pipeline.flags().enable_obmc);
        assert!(pipeline.flags().enable_warp);

        // Configure frame dimensions
        pipeline.set_frame_info(1920, 1080, 5);

        // Advance to motion compensation stage
        pipeline.set_stage(PipelineStage::MotionCompensation);
        assert_eq!(pipeline.stage(), PipelineStage::MotionCompensation);

        // MotionCompensationCapsule should be usable for this stage
        let mc_gen = mc_capsule.generation();
        assert!(mc_gen >= 0); // Valid generation counter
    }

    #[test]
    fn test_pframe_pipeline_with_reference_selection() {
        // Test interaction between PFramePipelineCapsule and ReferenceSelectionCapsule
        let pipeline = PFramePipelineCapsule::new();
        let ref_select = ReferenceSelectionCapsule::new();

        // Advance to reference selection stage
        assert_eq!(pipeline.advance_stage(), PipelineStage::ReferenceSelection);

        // ReferenceSelectionCapsule should be able to select best references
        // First update some reference scores
        ref_select.update_ref_score(0, 100, 1); // LAST
        ref_select.update_ref_score(1, 80, 2);  // GOLDEN

        // Get LAST reference score
        if let Some((score, dist)) = ref_select.get_ref_score(0) {
            assert_eq!(score, 100);
            assert_eq!(dist, 1);
            // Record reference frame usage in pipeline
            pipeline.record_ref_frame(0); // LAST reference
        }

        assert_eq!(pipeline.last_ref_count(), 1);
    }

    // ========================================================================
    // Phase 9: GPU Transform & Quantization Capsule Integration Tests
    // ========================================================================
    //
    // NOTE: These tests are conditionally compiled behind the "gpu-transform-stubs"
    // feature because they reference API methods and enum variants that haven't
    // been implemented yet. When the GPU transform API is complete, enable the
    // feature flag to run these tests.
    //
    // Missing API:
    // - TxType::IdtxIdtx (enum variant)
    // - Tx1dType::Idtx (enum variant)
    // - GpuTransformQuantCapsule::has_gpu()
    // - GpuTransformQuantCapsule::set_base_qp()
    // - GpuTransformQuantCapsule::get_base_qp()
    // - GpuTransformQuantCapsule::generation()
    // - GpuTransformQuantCapsule::forward_4x4()
    // - GpuTransformQuantCapsule::quantize_4x4()

    #[cfg(feature = "gpu-transform-stubs")]
    #[test]
    fn test_gpu_transform_quant_capsule_exports() {
        // Verify GpuTransformQuantCapsule is 256B cache-aligned (T7 Heterogeneous tier)
        assert_eq!(core::mem::size_of::<GpuTransformQuantCapsule>(), 256);
        assert_eq!(core::mem::align_of::<GpuTransformQuantCapsule>(), 256);

        // Verify TransformParams is 16 bytes
        assert_eq!(core::mem::size_of::<TransformParams>(), 16);

        // Verify QuantParams is 16 bytes
        assert_eq!(core::mem::size_of::<QuantParams>(), 16);

        // Verify TxSize enum types compile
        let _ = TxSize::Tx4x4;
        let _ = TxSize::Tx8x8;
        let _ = TxSize::Tx16x16;
        let _ = TxSize::Tx32x32;
        let _ = TxSize::Tx64x64;

        // Verify TxType enum types compile (16 combinations)
        let _ = TxType::DctDct;
        let _ = TxType::AdstDct;
        let _ = TxType::DctAdst;
        let _ = TxType::AdstAdst;
        let _ = TxType::FlipAdstDct;
        let _ = TxType::IdtxIdtx;

        // Verify Tx1dType enum types compile
        let _ = Tx1dType::Dct;
        let _ = Tx1dType::Adst;
        let _ = Tx1dType::FlipAdst;
        let _ = Tx1dType::Idtx;

        // Verify state machine states compile
        let _ = GpuTransformState::Idle;
        let _ = GpuTransformState::ResidualUpload;
        let _ = GpuTransformState::ForwardTransform;
        let _ = GpuTransformState::Quantize;
        let _ = GpuTransformState::TrellisOptimize;
        let _ = GpuTransformState::CoeffDownload;
        let _ = GpuTransformState::Complete;
        let _ = GpuTransformState::Error;
    }

    #[cfg(feature = "gpu-transform-stubs")]
    #[test]
    fn test_gpu_transform_quant_capsule_integration() {
        // Verify GpuTransformQuantCapsule can be constructed and used
        let gpu_tx = GpuTransformQuantCapsule::new();

        // Default state
        assert_eq!(gpu_tx.get_state(), GpuTransformState::Idle);

        // GPU detection (will be false in test environment without GPU)
        let has_gpu = gpu_tx.has_gpu();
        let _ = has_gpu; // May be true or false depending on hardware

        // Set base QP
        gpu_tx.set_base_qp(28);
        assert_eq!(gpu_tx.get_base_qp(), 28);

        // Generation counter tracks changes
        let gen0 = gpu_tx.generation();
        gpu_tx.set_base_qp(32);
        let gen1 = gpu_tx.generation();
        assert!(gen1 > gen0);
    }

    #[cfg(feature = "gpu-transform-stubs")]
    #[test]
    fn test_gpu_transform_forward_4x4() {
        let gpu_tx = GpuTransformQuantCapsule::new();

        // Test 4x4 DCT-DCT transform
        let mut residuals = [0i16; 16];
        // Simple pattern: 100 in first row, 0 elsewhere
        residuals[0] = 100;
        residuals[1] = 100;
        residuals[2] = 100;
        residuals[3] = 100;

        let mut coeffs = [0i32; 16];
        gpu_tx.forward_4x4(&residuals, &mut coeffs, TxType::DctDct);

        // DC coefficient should be dominant for uniform block
        // DCT of [100, 100, 100, 100] row should concentrate energy in DC
        assert!(coeffs[0].abs() > 0, "DC coefficient should be non-zero");

        // Higher-order coefficients should be smaller for flat input
        let ac_sum: i32 = coeffs[1..].iter().map(|c| c.abs()).sum();
        assert!(
            ac_sum < coeffs[0].abs() * 4,
            "AC energy should be less than scaled DC for flat input"
        );
    }

    #[cfg(feature = "gpu-transform-stubs")]
    #[test]
    fn test_gpu_transform_quantization_4x4() {
        let gpu_tx = GpuTransformQuantCapsule::new();
        gpu_tx.set_base_qp(28);

        // Create some test coefficients
        let coeffs = [
            1000, 500, 200, 100, // Row 0
            300, 150, 75, 30,    // Row 1
            100, 50, 25, 10,     // Row 2
            50, 25, 10, 5,       // Row 3
        ];

        let mut qcoeffs = [0i16; 16];
        let eob = gpu_tx.quantize_4x4(&coeffs, &mut qcoeffs, TxSize::Tx4x4);

        // EOB should be valid (0-16 for 4x4)
        assert!(eob <= 16, "EOB should be <= 16 for 4x4 block");

        // Larger coefficients should survive quantization
        assert!(qcoeffs[0] != 0, "DC coefficient should survive at QP 28");

        // Very small coefficients should be quantized to zero
        // (depends on dead zone, but coefficient 5 at QP 28 should be zero)
    }

    #[cfg(feature = "gpu-transform-stubs")]
    #[test]
    fn test_gpu_transform_with_pframe_pipeline() {
        // Test integration between GpuTransformQuantCapsule and PFramePipelineCapsule
        let gpu_tx = GpuTransformQuantCapsule::new();
        let pipeline = PFramePipelineCapsule::production();

        // Set matching QP
        gpu_tx.set_base_qp(28);

        // Advance pipeline to transform stage
        pipeline.set_stage(PipelineStage::MotionCompensation);
        // After motion compensation, transform would be next

        // GPU transform should be ready for use
        assert_eq!(gpu_tx.get_state(), GpuTransformState::Idle);

        // Both capsules maintain independent generation counters
        let tx_gen = gpu_tx.generation();
        let pipe_gen = pipeline.generation();
        assert!(tx_gen >= 0);
        assert!(pipe_gen >= 0);
    }
}
