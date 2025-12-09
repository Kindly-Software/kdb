//! Video Decoder Capsules
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! T1/T2 tier computational capsules for H.264, HEVC/H.265, VP9, and AV1 video decoding.
//! 100% lockfree, cache-aligned, Chaos compliant.
//!
//! # Supported Codecs
//!
//! | Codec | Feature | Compliance |
//! |-------|---------|------------|
//! | H.264/AVC | `decoder-h264` | ITU-T H.264 (10/2022) |
//! | HEVC/H.265 | `decoder-hevc` | ITU-T H.265 (08/2021) |
//! | VP9 | `decoder-vp9` | Google VP9 Specification |
//! | AV1 | `decoder-av1` | AOMedia AV1 Specification |
//!
//! # Architecture
//!
//! Each codec implements a hierarchical metacapsule pattern with tier-appropriate sub-capsules:
//!
//! ```text
//! DecoderMetacapsule (T6 Mixed, 2048B orchestrator)
//! +-----------------------------------------------------------------------+
//! | BitstreamCapsule (T2 SIMD) -> EntropyDecoder (T1 Atomic)              |
//! | -> HeaderParser (T1 Atomic) -> TransformCapsule (T2 SIMD)             |
//! | -> PredictionCapsule (T2 SIMD) -> LoopFilterCapsule (T2 SIMD)         |
//! +-----------------------------------------------------------------------+
//! ```
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 tier selection per capsule, Q33 derive verification, Q34 audit trails
//! - **Chaos**: 100% lockfree (AtomicU64/AtomicU32 only), cache-aligned (64B/128B/256B)
//! - **ASSUM**: All unsafe blocks documented with #ASSUME/#VERIFY tags
//! - **B32**: Criterion benchmarks with 95% CI, 1000+ iterations
//! - **T28**: 5-tier testing (unit/property/integration/production/determinism)

// ============================================================================
// H.264/AVC DECODER CAPSULES
// ============================================================================

#[cfg(feature = "decoder-h264")]
pub mod h264_bitstream;
#[cfg(feature = "decoder-h264")]
pub mod h264_cabac;
#[cfg(feature = "decoder-h264")]
pub mod h264_deblock;
#[cfg(feature = "decoder-h264")]
pub mod h264_inter_pred;
#[cfg(feature = "decoder-h264")]
pub mod h264_intra_pred;
#[cfg(feature = "decoder-h264")]
pub mod h264_macroblock;
#[cfg(feature = "decoder-h264")]
pub mod h264_sps_pps;
#[cfg(feature = "decoder-h264")]
pub mod h264_transform;

// ============================================================================
// VP9 DECODER CAPSULES
// ============================================================================

#[cfg(feature = "decoder-vp9")]
pub mod vp9_bitstream;
#[cfg(feature = "decoder-vp9")]
pub mod vp9_bool;
#[cfg(feature = "decoder-vp9")]
pub mod vp9_frame_header;
#[cfg(feature = "decoder-vp9")]
pub mod vp9_inter_pred;
#[cfg(feature = "decoder-vp9")]
pub mod vp9_intra_pred;
#[cfg(feature = "decoder-vp9")]
pub mod vp9_loop_filter;
#[cfg(feature = "decoder-vp9")]
pub mod vp9_segmentation;
#[cfg(feature = "decoder-vp9")]
pub mod vp9_transform;

// ============================================================================
// AV1 DECODER CAPSULES
// ============================================================================

#[cfg(feature = "decoder-av1")]
pub mod av1_bitstream;
#[cfg(feature = "decoder-av1")]
pub mod av1_inter_pred;
#[cfg(feature = "decoder-av1")]
pub mod av1_intra_pred;
#[cfg(feature = "decoder-av1")]
pub mod av1_loop_filter;
#[cfg(feature = "decoder-av1")]
pub mod av1_sequence_header;
#[cfg(feature = "decoder-av1")]
pub mod av1_symbol;
#[cfg(feature = "decoder-av1")]
pub mod av1_tile_group;
#[cfg(feature = "decoder-av1")]
pub mod av1_transform;

// ============================================================================
// HEVC/H.265 DECODER CAPSULES
// ============================================================================

#[cfg(feature = "decoder-hevc")]
pub mod hevc_bitstream;
#[cfg(feature = "decoder-hevc")]
pub mod hevc_cabac;
#[cfg(feature = "decoder-hevc")]
pub mod hevc_ctu;
#[cfg(feature = "decoder-hevc")]
pub mod hevc_inter_pred;
#[cfg(feature = "decoder-hevc")]
pub mod hevc_intra_pred;
#[cfg(feature = "decoder-hevc")]
pub mod hevc_loop_filter;
#[cfg(feature = "decoder-hevc")]
pub mod hevc_slice;
#[cfg(feature = "decoder-hevc")]
pub mod hevc_transform;

// ============================================================================
// H.264 RE-EXPORTS
// ============================================================================

#[cfg(feature = "decoder-h264")]
pub use h264_bitstream::{
    BitstreamError, BitstreamStats, H264BitstreamCapsule, NalUnit, NalUnitType,
};

#[cfg(feature = "decoder-h264")]
pub use h264_cabac::{
    CabacContext, CabacContextTable, CabacDecoderCapsule, CabacError, CabacState, CabacStats,
    SliceType, context_idx,
    NUM_CONTEXTS, INLINE_CONTEXTS, RANGE_LPS_TABLE, TRANS_LPS, TRANS_MPS,
};

#[cfg(feature = "decoder-h264")]
pub use h264_sps_pps::{H264SpsPpsCapsule, Pps, Sps, SpsError, SpsStats, Profile, VuiParameters};

#[cfg(feature = "decoder-h264")]
pub use h264_transform::{
    H264TransformCapsule, TransformError, TransformStats, TransformType,
    LEVEL_SCALE_4X4, LEVEL_SCALE_8X8_FLAT,
};

#[cfg(feature = "decoder-h264")]
pub use h264_macroblock::{
    H264MacroblockCapsule, MacroblockData, MacroblockError, MacroblockStats,
    MbTypeI, MbTypeP, SubMbTypeP,
    Intra4x4PredMode, Intra16x16PredMode, IntraChromaPredMode,
};

#[cfg(feature = "decoder-h264")]
pub use h264_inter_pred::{
    H264InterPredCapsule, InterPredError, InterPredStats, MotionVector,
    PartitionSize, RefList, LUMA_FILTER_COEFFS,
};

#[cfg(feature = "decoder-h264")]
pub use h264_deblock::{
    H264DeblockCapsule, DeblockError, DeblockStats, BoundaryStrength,
    FilterMode, EdgeType, MacroblockInfo,
    ALPHA_TABLE, BETA_TABLE, TC0_TABLE,
};

#[cfg(feature = "decoder-h264")]
pub use h264_intra_pred::{
    H264IntraPredCapsule, IntraPredError, IntraPredStats,
    Neighbors4x4, Neighbors8x8, Neighbors16x16,
    INTRA_4X4_VERTICAL, INTRA_4X4_HORIZONTAL, INTRA_4X4_DC,
    INTRA_4X4_DIAGONAL_DOWN_LEFT, INTRA_4X4_DIAGONAL_DOWN_RIGHT,
    INTRA_4X4_VERTICAL_RIGHT, INTRA_4X4_HORIZONTAL_DOWN,
    INTRA_4X4_VERTICAL_LEFT, INTRA_4X4_HORIZONTAL_UP,
    INTRA_16X16_VERTICAL, INTRA_16X16_HORIZONTAL, INTRA_16X16_DC, INTRA_16X16_PLANE,
    INTRA_CHROMA_DC, INTRA_CHROMA_HORIZONTAL, INTRA_CHROMA_VERTICAL, INTRA_CHROMA_PLANE,
};

// ============================================================================
// VP9 RE-EXPORTS
// ============================================================================

#[cfg(feature = "decoder-vp9")]
pub use vp9_frame_header::{
    Vp9FrameHeaderCapsule, Vp9FrameHeaderError, Vp9FrameHeaderStats,
    Vp9FrameType, Vp9Profile, Vp9ColorSpace, Vp9InterpolationFilter,
};

#[cfg(feature = "decoder-vp9")]
pub use vp9_bitstream::{
    Vp9BitstreamCapsule, Vp9BitstreamError, Vp9BitstreamStats, Vp9SuperframeInfo,
    VP9_FRAME_MARKER, VP9_SUPERFRAME_MARKER, VP9_SYNC_CODE,
};

#[cfg(feature = "decoder-vp9")]
pub use vp9_loop_filter::{
    Vp9LoopFilterCapsule, Vp9LoopFilterError, Vp9LoopFilterStats, Vp9LoopFilterParams,
    Vp9RefFrame, Vp9Mode, TxSize,
};

#[cfg(feature = "decoder-vp9")]
pub use vp9_intra_pred::{
    Vp9IntraPredCapsule, Vp9IntraPredError, Vp9IntraPredStats, Vp9IntraNeighbors,
    Vp9IntraMode, Vp9BlockSize,
};

#[cfg(feature = "decoder-vp9")]
pub use vp9_inter_pred::{
    Vp9InterPredCapsule, Vp9InterPredError, Vp9InterPredStats,
    Vp9MotionVector, Vp9RefFrame as Vp9InterRefFrame,
    SUBPEL_FILTERS_SHARP, SUBPEL_FILTERS_SMOOTH, SUBPEL_FILTERS_REGULAR, SUBPEL_FILTERS_BILINEAR,
    FILTER_ROUND, FILTER_SHIFT,
};

#[cfg(feature = "decoder-vp9")]
pub use vp9_transform::{
    Vp9TransformCapsule, Vp9TransformError, Vp9TransformStats,
    TxSize as Vp9TxSize, TxType as Vp9TxType, TransformKind as Vp9TransformKind,
};

// ============================================================================
// AV1 RE-EXPORTS
// ============================================================================

#[cfg(feature = "decoder-av1")]
pub use av1_symbol::{
    Av1SymbolDecoderCapsule, Av1SymbolError, Av1SymbolState, Av1SymbolStats,
    create_uniform_cdf, create_cdf_from_weights,
    SYMBOL_BITS, CDF_PROB_BITS, CDF_PROB_TOP, MIN_RANGE, MAX_RANGE, MAX_CDF_SYMBOLS,
};

#[cfg(feature = "decoder-av1")]
pub use av1_sequence_header::{
    Av1SequenceHeaderCapsule, Av1SequenceHeaderError, Av1SequenceHeaderStats,
    Av1Profile, Av1ColorPrimaries, Av1TransferCharacteristics,
    Av1MatrixCoefficients, Av1ChromaSamplePosition,
    MAX_OPERATING_POINTS, MAX_FRAME_WIDTH, MAX_FRAME_HEIGHT, NUM_REF_FRAMES,
    SELECT_SCREEN_CONTENT_TOOLS, SELECT_INTEGER_MV,
};

#[cfg(feature = "decoder-av1")]
pub use av1_bitstream::{
    Av1BitstreamCapsule, Av1BitstreamStats, Av1Error, ObuHeader, ObuType, TemporalUnit,
};

#[cfg(feature = "decoder-av1")]
pub use av1_tile_group::{
    Av1TileGroupCapsule, Av1TileGroupError, Av1TileGroupStats, Av1TileCoords,
    AV1_MAX_TILE_COLS, AV1_MAX_TILE_ROWS, AV1_MAX_TILES,
    AV1_SB_SIZE_64, AV1_SB_SIZE_128, AV1_INLINE_TILE_OFFSETS,
    state_flags as av1_tile_state_flags,
};

#[cfg(feature = "decoder-av1")]
pub use av1_intra_pred::{
    Av1IntraPredCapsule, Av1IntraPredError, Av1IntraPredStats, Av1IntraNeighbors,
    Av1IntraMode, Av1FilterIntraMode,
    NOMINAL_ANGLES, MAX_ANGLE_DELTA, DR_INTRA_DERIVATIVE,
    SMOOTH_WEIGHTS_4, SMOOTH_WEIGHTS_8, SMOOTH_WEIGHTS_16, SMOOTH_WEIGHTS_32, SMOOTH_WEIGHTS_64,
    FILTER_INTRA_TAPS,
};

#[cfg(feature = "decoder-av1")]
pub use av1_transform::{
    Av1TransformCapsule, Av1TransformError, Av1TransformStats,
    Av1TxType, Av1TxSize, Av1TransformKind,
};

#[cfg(feature = "decoder-av1")]
pub use av1_loop_filter::{
    Av1LoopFilterCapsule, Av1LoopFilterError, Av1LoopFilterStats,
    Av1RestorationType, CDEF_DIRECTIONS, DEBLOCK_ALPHA_TABLE, DEBLOCK_BETA_TABLE,
};

#[cfg(feature = "decoder-av1")]
pub use av1_inter_pred::{
    Av1InterPredCapsule, Av1InterPredError, Av1InterPredStats,
    Av1MotionVector, Av1InterpFilter,
};

// ============================================================================
// HEVC/H.265 RE-EXPORTS
// ============================================================================

#[cfg(feature = "decoder-hevc")]
pub use hevc_bitstream::{
    HevcBitstreamCapsule, HevcBitstreamError, HevcBitstreamStats, HevcBitstreamSnapshot,
    HevcNalUnitType, HevcNalUnit, HevcBitReader,
    HevcVps, HevcSps, HevcPps, HevcShortTermRefPicSet,
    HevcProfile, HevcTier, HevcLevel, HevcProfileTierLevel, HevcChromaFormat,
    parser_state as hevc_parser_state,
    HEVC_NAL_HEADER_SIZE, HEVC_MAX_VPS_ID, HEVC_MAX_SPS_ID, HEVC_MAX_PPS_ID,
    HEVC_MAX_SUB_LAYERS_MINUS1, HEVC_MAX_REF_FRAMES, HEVC_MAX_SHORT_TERM_RPS,
    HEVC_MAX_LONG_TERM_REF_PICS, HEVC_MAX_CTB_SIZE_LOG2, HEVC_MIN_CTB_SIZE_LOG2,
    HEVC_MAX_TB_SIZE_LOG2, HEVC_MIN_TB_SIZE_LOG2, HEVC_MAX_WIDTH, HEVC_MAX_HEIGHT,
};

#[cfg(feature = "decoder-hevc")]
pub use hevc_cabac::{
    HevcCabacCapsule, HevcCabacContext, HevcCabacContextTable, HevcCabacError,
    HevcCabacState, HevcCabacStats, HevcSliceType, hevc_context_idx,
    HEVC_NUM_CONTEXTS, HEVC_INLINE_CONTEXTS,
    HEVC_RANGE_LPS_TABLE, HEVC_TRANS_LPS, HEVC_TRANS_MPS,
    HEVC_INIT_VALUES_I, HEVC_INIT_VALUES_P, HEVC_INIT_VALUES_B,
    NUM_CTX_SAO_MERGE, NUM_CTX_SAO_TYPE, NUM_CTX_SPLIT_CU_FLAG,
    NUM_CTX_SKIP_FLAG, NUM_CTX_CU_QP_DELTA, NUM_CTX_PRED_MODE,
    NUM_CTX_PART_MODE, NUM_CTX_MERGE_FLAG, NUM_CTX_MERGE_IDX,
    NUM_CTX_INTER_DIR, NUM_CTX_REF_PIC, NUM_CTX_MVD, NUM_CTX_MVP_FLAG,
    NUM_CTX_SPLIT_TRANSFORM, NUM_CTX_CBF_LUMA, NUM_CTX_CBF_CHROMA,
    NUM_CTX_TRANSFORM_SKIP, NUM_CTX_LAST_SIG_XY_PREFIX,
    NUM_CTX_CODED_SUB_BLOCK, NUM_CTX_SIG_COEFF_FLAG,
    NUM_CTX_COEFF_ABS_GREATER1, NUM_CTX_COEFF_ABS_GREATER2,
};

#[cfg(feature = "decoder-hevc")]
pub use hevc_inter_pred::{
    HevcInterPredCapsule, HevcInterPredError, HevcInterPredStats,
    HevcMotionVector, MergeCandidate, WeightedPredParams, WeightedPredMode,
    LUMA_FILTER, CHROMA_FILTER, FILTER_ROUND, FILTER_SHIFT,
    FILTER_ROUND_2D, FILTER_SHIFT_2D, MAX_BLOCK_DIM, MAX_PU_SIZE,
    MAX_MERGE_CANDIDATES, MAX_AMVP_CANDIDATES,
};

#[cfg(feature = "decoder-hevc")]
pub use hevc_transform::{
    HevcTransformCapsule, HevcTransformError, HevcTransformStats, HevcTransformType,
    DCT4, DCT8, DST4, DCT16_EVEN, DCT16_ODD, DCT32_ODD,
};

#[cfg(feature = "decoder-hevc")]
pub use hevc_loop_filter::{
    HevcLoopFilterCapsule, HevcLoopFilterError, HevcLoopFilterStats,
    HevcSaoType, HevcSaoEdgeClass, HevcBlockInfo, HevcSaoParams,
    HEVC_BETA_TABLE, HEVC_TC_TABLE, SAO_EO_CATEGORIES,
    SAO_NUM_BANDS, SAO_NUM_EO_CLASSES, SAO_NUM_OFFSETS,
};

#[cfg(feature = "decoder-hevc")]
pub use hevc_slice::{
    HevcSliceCapsule, HevcSliceError, HevcSliceHeader, HevcSliceStats,
    HevcSliceType as HevcSliceTypeSlice, HevcNalType, HevcTileInfo, HevcTileCoords,
    state_flags as hevc_slice_state_flags,
    HEVC_MAX_TILE_COLS, HEVC_MAX_TILE_ROWS, HEVC_MAX_TILES,
    HEVC_MAX_CTU_SIZE, HEVC_MIN_CTU_SIZE, HEVC_MAX_WPP_ENTRY_POINTS,
    HEVC_INLINE_TILE_OFFSETS, HEVC_INLINE_WPP_ENTRY_POINTS,
    HEVC_MAX_REF_IDX_L0, HEVC_MAX_REF_IDX_L1,
    HEVC_MAX_SLICE_QP_DELTA, HEVC_MIN_SLICE_QP_DELTA, HEVC_MAX_POC,
};

#[cfg(feature = "decoder-hevc")]
pub use hevc_ctu::{
    HevcCtuCapsule, HevcCtuError, HevcCtuStats, HevcCuData,
    HevcPredMode, HevcPartMode, HevcIntraMode, HevcSliceType as HevcCtuSliceType,
    HEVC_MAX_CU_DEPTH, HEVC_MAX_TU_DEPTH,
    HEVC_NUM_INTRA_MODES, HEVC_INTRA_PLANAR, HEVC_INTRA_DC, HEVC_INTRA_ANGULAR_START,
};

#[cfg(feature = "decoder-hevc")]
pub use hevc_intra_pred::{
    HevcIntraPredCapsule, HevcIntraPredError, HevcIntraPredStats, HevcIntraRefs,
    HevcIntraMode as HevcIntraPredMode,
    INTRA_PRED_ANGLE, INV_ANGLE, INTRA_FILTER_FLAG,
};

// ============================================================================
// PHASE FLAGS
// ============================================================================

/// Phase flags for decoder metacapsule coordination
///
/// Each bit represents completion of a specific initialization/decoding phase.
/// Used with `DualAtomicU64` pattern for lockfree coordination.
pub mod phase_flags {
    /// Bitstream capsule initialized
    pub const BITSTREAM_READY: u64 = 1 << 0;
    /// Entropy decoder initialized
    pub const ENTROPY_READY: u64 = 1 << 1;
    /// Sequence/SPS parsed successfully
    pub const SEQUENCE_PARSED: u64 = 1 << 2;
    /// Picture/PPS parsed successfully
    pub const PICTURE_PARSED: u64 = 1 << 3;
    /// First keyframe/IDR found
    pub const KEYFRAME_FOUND: u64 = 1 << 4;
    /// Frame buffer initialized
    pub const BUFFER_READY: u64 = 1 << 5;
    /// Decoder ready for decoding
    pub const DECODER_READY: u64 = BITSTREAM_READY | ENTROPY_READY | SEQUENCE_PARSED | PICTURE_PARSED;
    /// All initialization complete
    pub const ALL_INIT_COMPLETE: u64 = DECODER_READY | KEYFRAME_FOUND | BUFFER_READY;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_phase_flags() {
        use super::phase_flags::*;

        // Verify flags are distinct powers of 2
        assert_eq!(BITSTREAM_READY.count_ones(), 1);
        assert_eq!(ENTROPY_READY.count_ones(), 1);
        assert_eq!(SEQUENCE_PARSED.count_ones(), 1);
        assert_eq!(PICTURE_PARSED.count_ones(), 1);
        assert_eq!(KEYFRAME_FOUND.count_ones(), 1);
        assert_eq!(BUFFER_READY.count_ones(), 1);

        // Verify no overlap between base flags
        let all_flags = BITSTREAM_READY | ENTROPY_READY | SEQUENCE_PARSED | PICTURE_PARSED | KEYFRAME_FOUND | BUFFER_READY;
        assert_eq!(all_flags.count_ones(), 6);
    }
}
