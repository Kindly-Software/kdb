//! kindly-av1 - GPU-Accelerated AV1 Video Encoder Library
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! This library provides the core encoding functionality for kindly-av1,
//! built on the COCA (Computational Capsule Architecture) framework.
//!
//! ## Architecture
//!
//! kindly-av1 uses a T6 Mixed metacapsule architecture with 12 sub-capsules:
//!
//! - `LicenseVerificationCapsule` (T1) - License key validation
//! - `FileInputCapsule` (T5) - Memory-mapped video input
//! - `FileOutputCapsule` (T5) - Atomic bitstream output
//! - `GpuMotionEstimationCapsule` (T7) - GPU motion vectors
//! - `GpuTransformCapsule` (T7) - GPU DCT/ADST transforms
//! - `GpuQuantizationCapsule` (T2+T7) - Quantization with AVX2/GPU
//! - `EntropyCoderCapsule` (T2) - ANS encoding
//! - `LoopFilterCapsule` (T2) - CDEF + LRF
//! - `CheckpointCapsule` (T9) - Crash recovery
//! - `ProgressCapsule` (T1) - Real-time metrics
//! - `TuiDashboardCapsule` (T1) - Terminal UI
//! - `BitstreamWriterCapsule` (T5) - AV1 OBU output
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 T6 Mixed tier, Q33 derive verification
//! - **COCA**: 100% lockfree, cache-aligned capsules
//! - **ASSUM**: All unsafe documented with #ASSUME/#VERIFY
//! - **B32**: Criterion benchmarks with 95% CI
//! - **T28**: 5-tier testing (unit/property/integration/production/determinism)

#![feature(portable_simd)]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod cli;
pub mod demux;
pub mod decode;
pub mod encoder;
pub mod file;
pub mod checkpoint;
pub mod progress;
pub mod license;
pub mod pipeline;
pub mod hardening;
pub mod protection;
pub mod obs;

// Re-export common types from CLI module (legacy API)
pub use cli::legacy::{EncodeArgs, EncodingPreset, GpuBackend};

// Re-export key capsule types
// NOTE: Encoder functionality integrated into atomic_capsule crate
// pub use encoder::{
//     KindlyAv1CliMetacapsule,
//     GpuMotionEstimationCapsule,
//     GpuMotionBackend,
//     GpuMotionError,
//     GpuMotionStats,
//     MotionVector as Av1MotionVector,
//     SearchAlgorithm,
// };
pub use license::LicenseVerificationCapsule;
pub use checkpoint::EncoderCheckpointCapsule;
pub use progress::ProgressCapsule;
pub use demux::{
    // Container detection
    Mp4DemuxerCapsule, ContainerDetectorCapsule, ContainerFormat,
    // WebM validation (T1 Atomic)
    WebMValidatorCapsule, WebMValidationError, EbmlHeader, MkvTrackCapsule,
    WebmLacingType, ValidationSnapshot, WEBM_VIDEO_CODECS, WEBM_AUDIO_CODECS,
    // MKV cluster parsing (T4 Batch)
    MkvClusterCapsule, MkvClusterError, BlockHeader, BlockInfo, ClusterHeader,
    LacingType, ClusterStats, cluster_state,
    // MKV cues/seeking (T4 Batch)
    MkvCuesCapsule, MkvCuesError, CuePoint, SeekTarget, MkvCuesState, MkvCuesStats,
    MKV_CUES, CUE_POINT, CUE_TIME, CUE_TRACK_POSITIONS, CUE_TRACK,
    CUE_CLUSTER_POSITION, CUE_RELATIVE_POSITION, CUE_DURATION, CUE_BLOCK_NUMBER,
    MAX_INLINE_CUES,
    // MKV track atomic capsule (T1 Atomic, 256B aligned)
    MkvTrackAtomicCapsule, MkvTrackSnapshot, MkvTrackError,
    MkvTrackType, MkvVideoCodec, MkvAudioCodec,
    mkv_track_element_ids, video_codec_ids, audio_codec_ids, mkv_track_flags,
    // MKV demuxer (T5 Streaming - EBML header/segment parsing)
    MkvDemuxerCapsule, MkvError, MkvDemuxerState, MkvEbmlHeader, MkvSegmentInfo, MkvInfo,
    EbmlElement, MkvDemuxerStats, mkv_element_ids, is_master_element, MASTER_ELEMENTS,
};
pub use decode::{
    // H.264 Bitstream Parser (T2 SIMD)
    H264BitstreamCapsule, NalUnit, NalUnitType, BitstreamError, BitstreamStats,
    // H.264 Transform (T2 SIMD)
    H264TransformCapsule, TransformType, TransformError, TransformStats,
    // H.264 CABAC Entropy Decoder (T1 Atomic)
    CabacDecoderCapsule, CabacContextTable, CabacContext, CabacError, CabacState, CabacStats,
    SliceType,
    // H.264 SPS/PPS (T1 Atomic)
    H264SpsPpsCapsule, Sps, Pps, SpsError, SpsStats, Profile, VuiParameters,
    // H.264 Inter Prediction / Motion Compensation (T2 SIMD)
    H264InterPredCapsule, InterPredError, InterPredStats, MotionVector as H264MotionVector, PartitionSize, RefList,
    // H.264 Deblocking Filter (T2 SIMD)
    H264DeblockCapsule, DeblockError, DeblockStats, BoundaryStrength, FilterMode, EdgeType,
    MacroblockInfo, ALPHA_TABLE, BETA_TABLE, TC0_TABLE,
    // VP9 Frame Header (T1 Atomic)
    Vp9FrameHeaderCapsule, Vp9FrameHeaderError, Vp9FrameHeaderStats,
    Vp9FrameType, Vp9Profile, Vp9ColorSpace, Vp9InterpolationFilter,
    // VP9 Bitstream Parser (T2 SIMD)
    Vp9BitstreamCapsule, Vp9BitstreamError, Vp9BitstreamStats, Vp9SuperframeInfo,
    VP9_FRAME_MARKER, VP9_SUPERFRAME_MARKER, VP9_SYNC_CODE,
    // VP9 Loop Filter (T2 SIMD)
    Vp9LoopFilterCapsule, Vp9LoopFilterError, Vp9LoopFilterStats, Vp9LoopFilterParams,
    Vp9RefFrame, Vp9Mode, TxSize,
    // VP9 Inter Prediction / Motion Compensation (T2 SIMD)
    Vp9InterPredCapsule, Vp9InterPredError, Vp9InterPredStats, Vp9MotionVector,
    Vp9InterRefFrame, SUBPEL_FILTERS_SHARP, SUBPEL_FILTERS_SMOOTH, SUBPEL_FILTERS_REGULAR,
    SUBPEL_FILTERS_BILINEAR, FILTER_ROUND, FILTER_SHIFT,
    // VP9 Transform (T2 SIMD)
    Vp9TransformCapsule, Vp9TransformError, Vp9TransformStats,
    Vp9TxSize, Vp9TxType, Vp9TransformKind,
    // VP9 Intra Prediction (T2 SIMD)
    Vp9IntraPredCapsule, Vp9IntraPredError, Vp9IntraPredStats, Vp9IntraNeighbors,
    Vp9IntraMode, Vp9BlockSize,
    // AV1 Symbol Decoder (T1 Atomic)
    Av1SymbolDecoderCapsule, Av1SymbolError, Av1SymbolState, Av1SymbolStats,
    create_uniform_cdf, create_cdf_from_weights,
    SYMBOL_BITS, CDF_PROB_BITS, CDF_PROB_TOP,
    // AV1 Sequence Header (T1 Atomic)
    Av1SequenceHeaderCapsule, Av1SequenceHeaderError, Av1SequenceHeaderStats,
    Av1Profile, Av1ColorPrimaries, Av1TransferCharacteristics,
    Av1MatrixCoefficients, Av1ChromaSamplePosition,
    MAX_OPERATING_POINTS, MAX_FRAME_WIDTH, MAX_FRAME_HEIGHT, NUM_REF_FRAMES,
    // AV1 Transform (T2 SIMD)
    Av1TransformCapsule, Av1TransformError, Av1TransformStats,
    Av1TxType, Av1TxSize, Av1TransformKind,
    // AV1 Loop Filter (T2 SIMD)
    Av1LoopFilterCapsule, Av1LoopFilterError, Av1LoopFilterStats,
    Av1RestorationType, CDEF_DIRECTIONS, DEBLOCK_ALPHA_TABLE, DEBLOCK_BETA_TABLE,
};
pub use pipeline::{
    // Decoder Pipeline Metacapsule (T6 Mixed, 1024B)
    DecoderPipelineCapsule, PipelineState, PipelineError, PipelineStats,
    PipelineContainerFormat, PipelineVideoCodec, PipelineChromaFormat,
    DecodedFrame, VideoInfo, pipeline_phase_flags,
    // Tile Decoder (T4 Batch)
    TileDecoderCapsule, TileGrid, TileInfo, TileWork,
    TileState, DecoderState, TileDecoderStats, TileDecoderError,
    MAX_TILE_COLS, MAX_TILE_ROWS, MAX_INLINE_TILES,
    WORK_QUEUE_CAPACITY, DEFAULT_WORKERS,
    // Frame Buffer Pool (T4 Batch)
    FrameBufferPoolCapsule, PoolConfig, FrameBufferHandle,
    FrameBufferState, ChromaFormat, FrameBuffer,
    FramePoolStats, FramePoolError,
    // Output Formatter (T2 SIMD)
    OutputFormatterCapsule, OutputFormat,
    ColorSpace, ColorRange,
    OutputFormatterStats, OutputError,
};
pub use hardening::{
    // Bounds Checker (T1 Atomic)
    BoundsCheckerCapsule, BoundsCheckerStats,
    BoundsCheckType, BoundsViolation, bounds_flags,
    // Error Recovery (T1 Atomic)
    ErrorRecoveryCapsule, ErrorRecoveryStats,
    ErrorCategory, RecoveryStrategy, ConcealmentStrategy, RecoveryState,
    VideoCodec as ErrorRecoveryVideoCodec,  // Aliased to avoid collision
    H264_START_CODE, H264_START_CODE_3, MP4_MDAT, MKV_CLUSTER,
    H264_NAL_TYPE_MASK, H264_NAL_IDR_SLICE, H264_NAL_SPS, VP9_KEYFRAME_BIT,
    DEFAULT_ERROR_RATE_THRESHOLD, DEFAULT_MAX_CONSECUTIVE_ERRORS, ERROR_WINDOW_SIZE,
    // Benchmark Harness (T1 Atomic)
    BenchmarkHarnessCapsule, B32Config, BenchmarkResult, BenchmarkStats, Comparison,
    BenchmarkTarget, MetricType,
    // Fuzz Harness (T4 Batch)
    FuzzHarnessCapsule, FuzzTarget, MutationStrategy, CrashType,
    FuzzError, FuzzResult, FuzzSummary, FuzzStats, CorpusEntry,
    INTERESTING_U8, INTERESTING_U16, INTERESTING_U32,
    H264_NAL_TYPES, VP9_FRAME_TYPES, MAX_CORPUS_ENTRIES, MAX_MUTATION_SIZE,
    state_flags,
};
pub use protection::{
    // Hardware ID (T1 Atomic)
    HardwareIdCapsule, HardwareIdError,
    // Protection System
    ProtectionError,
    // Layer Constants
    LAYER_BUILD_HARDENING, LAYER_CRYPTO_LICENSE, LAYER_ENCRYPTED_STATE,
    LAYER_REMOTE_ATTESTATION, LAYER_TPM_BINDING, LAYER_OBFUSCATION,
    LAYER_FUZZY_EXTRACTOR, LAYER_ANOMALY_DETECTOR, LAYER_MEMORY_ENCRYPTION,
    LAYER_KERNEL_PROTECTION, LAYER_OBSERVABILITY, NUM_LAYERS,
};

// OBS Integration (Phase 1: Text File Output)
pub use obs::{ObsStatusWriterCapsule, ObsStatusFormat, ObsStatusError, ObsStatusSnapshot, ObsOptions};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Maximum supported resolution (8K)
pub const MAX_WIDTH: u32 = 7680;
/// Maximum supported height (8K)
pub const MAX_HEIGHT: u32 = 4320;

/// AV1 encoding result
pub type Result<T> = std::result::Result<T, Error>;

/// AV1 encoding errors
#[derive(Debug)]
pub enum Error {
    /// License verification failed
    LicenseError(String),

    /// Input file error
    InputError(String),

    /// Output file error
    OutputError(String),

    /// Encoding error
    EncodingError(String),

    /// GPU error
    GpuError(String),

    /// Checkpoint error
    CheckpointError(String),

    /// Invalid configuration
    ConfigError(String),

    /// IO error
    IoError(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LicenseError(msg) => write!(f, "License verification failed: {}", msg),
            Self::InputError(msg) => write!(f, "Input file error: {}", msg),
            Self::OutputError(msg) => write!(f, "Output file error: {}", msg),
            Self::EncodingError(msg) => write!(f, "Encoding error: {}", msg),
            Self::GpuError(msg) => write!(f, "GPU error: {}", msg),
            Self::CheckpointError(msg) => write!(f, "Checkpoint error: {}", msg),
            Self::ConfigError(msg) => write!(f, "Invalid configuration: {}", msg),
            Self::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_max_resolution() {
        assert_eq!(MAX_WIDTH, 7680);
        assert_eq!(MAX_HEIGHT, 4320);
    }
}
