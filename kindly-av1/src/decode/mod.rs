//! H.264/AVC and VP9 Decoder Capsules
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Native video decoder stack implementing ITU-T H.264 and Google VP9 specifications
//! using Chaos (Computational Capsule Architecture) for lockfree, cache-aligned parsing.
//!
//! # Architecture
//!
//! The decoder uses a hierarchical metacapsule pattern with tier-appropriate sub-capsules:
//!
//! ```text
//! H264DecoderMetacapsule (T6 Mixed, 2048B orchestrator)
//! +-----------------------------------------------------------------------+
//! |                                                                       |
//! | +-------------------+  +---------------------+  +------------------+  |
//! | | H264BitstreamCap  |  | CabacDecoderCapsule |  | H264SpsPpsCapsule|  |
//! | | (T2 SIMD, 256B)   |  | (T1 Atomic, 512B)   |  | (T1 Atomic, 512B)|  |
//! | | NAL unit parsing  |  | Entropy decoding    |  | SPS/PPS storage  |  |
//! | +-------------------+  +---------------------+  +------------------+  |
//! |         |                      |                       |              |
//! |         v                      v                       v              |
//! | +-------------------+  +---------------------+  +------------------+  |
//! | | H264MacroblockCap |  | H264TransformCap    |  | H264IntraPredCap |  |
//! | | (T4 Batch, 1024B) |  | (T2 SIMD, 256B)     |  | (T2 SIMD, 256B)  |  |
//! | | MB decoding       |  | IDCT transforms     |  | Intra prediction |  |
//! | +-------------------+  +---------------------+  +------------------+  |
//! |         |                      |                       |              |
//! |         v                      v                       v              |
//! | +-------------------+  +---------------------+  +------------------+  |
//! | | H264InterPredCap  |  | H264DeblockCapsule  |  | H264FrameBufCap  |  |
//! | | (T2 SIMD, 512B)   |  | (T2 SIMD, 256B)     |  | (T5 Stream, 4KB) |  |
//! | | Inter prediction  |  | Deblocking filter   |  | DPB management   |  |
//! | +-------------------+  +---------------------+  +------------------+  |
//! |                                                                       |
//! +-----------------------------------------------------------------------+
//! ```
//!
//! # Capsule Tier Assignments
//!
//! | Capsule | Tier | Size | Purpose | Speedup |
//! |---------|------|------|---------|---------|
//! | H264BitstreamCapsule | T2 SIMD | 256B | NAL parsing with SIMD start code detection | 2-4x |
//! | CabacDecoderCapsule | T1 Atomic | 512B | Context-adaptive binary arithmetic coding | 1.5-2x |
//! | H264SpsPpsCapsule | T1 Atomic | 512B | Sequence/picture parameter set storage | N/A |
//! | H264MacroblockCapsule | T4 Batch | 1024B | Parallel macroblock decoding | 4-8x |
//! | H264TransformCapsule | T2 SIMD | 256B | 4x4/8x8 IDCT transforms | 3-6x |
//! | H264IntraPredCapsule | T2 SIMD | 256B | 16 intra prediction modes | 2-4x |
//! | H264InterPredCapsule | T2 SIMD | 512B | Motion compensation | 2-4x |
//! | H264DeblockCapsule | T2 SIMD | 256B | In-loop deblocking filter | 2-3x |
//! | H264FrameBufferCapsule | T5 Streaming | 4KB | Decoded picture buffer (DPB) | O(1) |
//!
//! # ITU-T H.264 Compliance
//!
//! Implements the following ITU-T H.264 (10/2022) sections:
//!
//! - Annex B: Byte stream format (NAL unit detection)
//! - Section 7: Syntax and semantics (NAL header, exp-golomb)
//! - Section 8: Decoding process (CABAC, transforms, prediction)
//! - Section 9: CABAC entropy coding
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 tier selection per capsule, Q33 derive verification, Q34 audit trails
//! - **Chaos**: 100% lockfree (AtomicU64/AtomicU32 only), cache-aligned (64B/128B/256B)
//! - **ASSUM**: All unsafe blocks documented with #ASSUME/#VERIFY tags
//! - **B32**: Criterion benchmarks with 95% CI, 1000+ iterations
//! - **T28**: 5-tier testing (unit/property/integration/production/determinism)
//!
//! # Usage
//!
//! ```rust,ignore
//! use kindly_av1::decode::{H264BitstreamCapsule, NalUnitType};
//!
//! // Parse H.264 Annex B stream
//! let mut bitstream = H264BitstreamCapsule::new();
//! let nals = bitstream.parse_nal_units(&annex_b_data)?;
//!
//! for nal in &nals {
//!     match nal.nal_unit_type {
//!         NalUnitType::Sps => println!("Found SPS at offset {}", nal.offset),
//!         NalUnitType::Pps => println!("Found PPS at offset {}", nal.offset),
//!         NalUnitType::SliceIdr => println!("Found IDR slice"),
//!         _ => {}
//!     }
//! }
//!
//! let stats = bitstream.stats();
//! println!("Parsed {} NAL units, {} start codes", stats.nals_found, stats.total_start_codes);
//! ```

// Sub-modules - H.264
pub mod h264_bitstream;
pub mod h264_cabac;
pub mod h264_deblock;
pub mod h264_inter_pred;
pub mod h264_intra_pred;
pub mod h264_macroblock;
pub mod h264_sps_pps;
pub mod h264_transform;

// Sub-modules - VP9
pub mod vp9_frame_header;
pub mod vp9_bitstream;
pub mod vp9_inter_pred;
pub mod vp9_loop_filter;
pub mod vp9_intra_pred;
pub mod vp9_transform;

// Sub-modules - AV1
pub mod av1_bitstream;
pub mod av1_inter_pred;
pub mod av1_intra_pred;
pub mod av1_loop_filter;
pub mod av1_sequence_header;
pub mod av1_symbol;
pub mod av1_tile_group;
pub mod av1_transform;

// Re-export H.264 bitstream parser (Phase 2)
pub use h264_bitstream::{
    BitstreamError, BitstreamStats, H264BitstreamCapsule, NalUnit, NalUnitType,
};

// Re-export CABAC decoder (Phase 2 - full implementation)
pub use h264_cabac::{
    CabacContext, CabacContextTable, CabacDecoderCapsule, CabacError, CabacState, CabacStats,
    SliceType, context_idx,
    NUM_CONTEXTS, INLINE_CONTEXTS, RANGE_LPS_TABLE, TRANS_LPS, TRANS_MPS,
};

// Re-export SPS/PPS parser (Phase 2)
pub use h264_sps_pps::{H264SpsPpsCapsule, Pps, Sps, SpsError, SpsStats, Profile, VuiParameters};

// Re-export H.264 transform capsule (Phase 2)
pub use h264_transform::{
    H264TransformCapsule, TransformError, TransformStats, TransformType,
    LEVEL_SCALE_4X4, LEVEL_SCALE_8X8_FLAT,
};

// Re-export H.264 macroblock decoder (Phase 2)
pub use h264_macroblock::{
    H264MacroblockCapsule, MacroblockData, MacroblockError, MacroblockStats,
    MbTypeI, MbTypeP, SubMbTypeP,
    Intra4x4PredMode, Intra16x16PredMode, IntraChromaPredMode,
};

// Re-export H.264 inter prediction capsule (Phase 2)
pub use h264_inter_pred::{
    H264InterPredCapsule, InterPredError, InterPredStats, MotionVector,
    PartitionSize, RefList, LUMA_FILTER_COEFFS,
};

// Re-export H.264 deblocking filter capsule (Phase 2)
pub use h264_deblock::{
    H264DeblockCapsule, DeblockError, DeblockStats, BoundaryStrength,
    FilterMode, EdgeType, MacroblockInfo,
    ALPHA_TABLE, BETA_TABLE, TC0_TABLE,
};

// Re-export H.264 intra prediction capsule (Phase 2)
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

// Re-export VP9 frame header capsule (T1 Atomic)
pub use vp9_frame_header::{
    Vp9FrameHeaderCapsule, Vp9FrameHeaderError, Vp9FrameHeaderStats,
    Vp9FrameType, Vp9Profile, Vp9ColorSpace, Vp9InterpolationFilter,
};

// Re-export VP9 bitstream capsule (T2 SIMD)
pub use vp9_bitstream::{
    Vp9BitstreamCapsule, Vp9BitstreamError, Vp9BitstreamStats, Vp9SuperframeInfo,
    VP9_FRAME_MARKER, VP9_SUPERFRAME_MARKER, VP9_SYNC_CODE,
};

// Re-export VP9 loop filter capsule (T2 SIMD)
pub use vp9_loop_filter::{
    Vp9LoopFilterCapsule, Vp9LoopFilterError, Vp9LoopFilterStats, Vp9LoopFilterParams,
    Vp9RefFrame, Vp9Mode, TxSize,
};

// Re-export VP9 intra prediction capsule (T2 SIMD)
pub use vp9_intra_pred::{
    Vp9IntraPredCapsule, Vp9IntraPredError, Vp9IntraPredStats, Vp9IntraNeighbors,
    Vp9IntraMode, Vp9BlockSize,
};

// Re-export VP9 inter prediction capsule (T2 SIMD)
pub use vp9_inter_pred::{
    Vp9InterPredCapsule, Vp9InterPredError, Vp9InterPredStats,
    Vp9MotionVector, Vp9RefFrame as Vp9InterRefFrame,
    SUBPEL_FILTERS_SHARP, SUBPEL_FILTERS_SMOOTH, SUBPEL_FILTERS_REGULAR, SUBPEL_FILTERS_BILINEAR,
    FILTER_ROUND, FILTER_SHIFT,
};

// Re-export VP9 transform capsule (T2 SIMD)
// Note: TxSize and TxType are prefixed with Vp9Tx to avoid conflict with vp9_loop_filter::TxSize
pub use vp9_transform::{
    Vp9TransformCapsule, Vp9TransformError, Vp9TransformStats,
    TxSize as Vp9TxSize, TxType as Vp9TxType, TransformKind as Vp9TransformKind,
};

// Re-export AV1 symbol decoder capsule (T1 Atomic)
pub use av1_symbol::{
    Av1SymbolDecoderCapsule, Av1SymbolError, Av1SymbolState, Av1SymbolStats,
    create_uniform_cdf, create_cdf_from_weights,
    SYMBOL_BITS, CDF_PROB_BITS, CDF_PROB_TOP, MIN_RANGE, MAX_RANGE, MAX_CDF_SYMBOLS,
};

// Re-export AV1 sequence header capsule (T1 Atomic)
pub use av1_sequence_header::{
    Av1SequenceHeaderCapsule, Av1SequenceHeaderError, Av1SequenceHeaderStats,
    Av1Profile, Av1ColorPrimaries, Av1TransferCharacteristics,
    Av1MatrixCoefficients, Av1ChromaSamplePosition,
    MAX_OPERATING_POINTS, MAX_FRAME_WIDTH, MAX_FRAME_HEIGHT, NUM_REF_FRAMES,
    SELECT_SCREEN_CONTENT_TOOLS, SELECT_INTEGER_MV,
};

// Re-export AV1 bitstream capsule (T5 Streaming)
pub use av1_bitstream::{
    Av1BitstreamCapsule, Av1BitstreamStats, Av1Error, ObuHeader, ObuType, TemporalUnit,
};

// Re-export AV1 tile group capsule (T4 Batch)
pub use av1_tile_group::{
    Av1TileGroupCapsule, Av1TileGroupError, Av1TileGroupStats, Av1TileCoords,
    AV1_MAX_TILE_COLS, AV1_MAX_TILE_ROWS, AV1_MAX_TILES,
    AV1_SB_SIZE_64, AV1_SB_SIZE_128, AV1_INLINE_TILE_OFFSETS,
    state_flags as av1_tile_state_flags,
};

// Re-export AV1 intra prediction capsule (T2 SIMD)
pub use av1_intra_pred::{
    Av1IntraPredCapsule, Av1IntraPredError, Av1IntraPredStats, Av1IntraNeighbors,
    Av1IntraMode, Av1FilterIntraMode,
    NOMINAL_ANGLES, MAX_ANGLE_DELTA, DR_INTRA_DERIVATIVE,
    SMOOTH_WEIGHTS_4, SMOOTH_WEIGHTS_8, SMOOTH_WEIGHTS_16, SMOOTH_WEIGHTS_32, SMOOTH_WEIGHTS_64,
    FILTER_INTRA_TAPS,
};

// Re-export AV1 transform capsule (T2 SIMD)
pub use av1_transform::{
    Av1TransformCapsule, Av1TransformError, Av1TransformStats,
    Av1TxType, Av1TxSize, Av1TransformKind,
};

// Re-export AV1 loop filter capsule (T2 SIMD)
pub use av1_loop_filter::{
    Av1LoopFilterCapsule, Av1LoopFilterError, Av1LoopFilterStats,
    Av1RestorationType, CDEF_DIRECTIONS, DEBLOCK_ALPHA_TABLE, DEBLOCK_BETA_TABLE,
};

/// Phase flags for decoder metacapsule coordination
///
/// Each bit represents completion of a specific initialization/decoding phase.
/// Used with `DualAtomicU64` pattern for lockfree coordination.
pub mod phase_flags {
    /// Bitstream capsule initialized
    pub const BITSTREAM_READY: u64 = 1 << 0;
    /// CABAC decoder initialized
    pub const CABAC_READY: u64 = 1 << 1;
    /// SPS parsed successfully
    pub const SPS_PARSED: u64 = 1 << 2;
    /// PPS parsed successfully
    pub const PPS_PARSED: u64 = 1 << 3;
    /// First IDR slice found
    pub const IDR_FOUND: u64 = 1 << 4;
    /// Frame buffer initialized
    pub const DPB_READY: u64 = 1 << 5;
    /// Decoder ready for decoding
    pub const DECODER_READY: u64 = BITSTREAM_READY | CABAC_READY | SPS_PARSED | PPS_PARSED;
    /// All initialization complete
    pub const ALL_INIT_COMPLETE: u64 = DECODER_READY | IDR_FOUND | DPB_READY;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all public types are accessible
        let _bitstream = H264BitstreamCapsule::new();
        let _cabac = CabacDecoderCapsule::new();
        let _sps_pps = H264SpsPpsCapsule::new();
    }

    #[test]
    fn test_vp9_module_exports() {
        // Verify VP9 types are accessible
        let _vp9_header = Vp9FrameHeaderCapsule::new();
        assert_eq!(core::mem::size_of::<Vp9FrameHeaderCapsule>(), 1024);
    }

    #[test]
    fn test_vp9_loop_filter_exports() {
        // Verify VP9 loop filter types are accessible
        let capsule = Vp9LoopFilterCapsule::new();
        assert_eq!(core::mem::size_of::<Vp9LoopFilterCapsule>(), 256);
        assert_eq!(capsule.level(), 0);

        // Test parameter creation
        let params = Vp9LoopFilterParams::with_level(32, 4);
        assert_eq!(params.level, 32);
        assert_eq!(params.sharpness, 4);

        // Test reference frame types
        assert_eq!(Vp9RefFrame::Intra.delta_index(), 0);
        assert_eq!(Vp9RefFrame::Last.delta_index(), 1);

        // Test transform sizes
        assert_eq!(TxSize::Tx8x8.size_pixels(), 8);
    }

    #[test]
    fn test_phase_flags() {
        use phase_flags::*;

        // Verify flags are distinct powers of 2
        assert_eq!(BITSTREAM_READY.count_ones(), 1);
        assert_eq!(CABAC_READY.count_ones(), 1);
        assert_eq!(SPS_PARSED.count_ones(), 1);
        assert_eq!(PPS_PARSED.count_ones(), 1);
        assert_eq!(IDR_FOUND.count_ones(), 1);
        assert_eq!(DPB_READY.count_ones(), 1);

        // Verify no overlap between base flags
        let all_flags = BITSTREAM_READY | CABAC_READY | SPS_PARSED | PPS_PARSED | IDR_FOUND | DPB_READY;
        assert_eq!(all_flags.count_ones(), 6);
    }

    #[test]
    fn test_decoder_ready_composition() {
        use phase_flags::*;

        // DECODER_READY should include mandatory flags
        assert_eq!(DECODER_READY & BITSTREAM_READY, BITSTREAM_READY);
        assert_eq!(DECODER_READY & CABAC_READY, CABAC_READY);
        assert_eq!(DECODER_READY & SPS_PARSED, SPS_PARSED);
        assert_eq!(DECODER_READY & PPS_PARSED, PPS_PARSED);

        // IDR_FOUND and DPB_READY are not in DECODER_READY
        assert_eq!(DECODER_READY & IDR_FOUND, 0);
        assert_eq!(DECODER_READY & DPB_READY, 0);
    }

    #[test]
    fn test_all_init_complete_composition() {
        use phase_flags::*;

        // ALL_INIT_COMPLETE includes everything
        assert_eq!(ALL_INIT_COMPLETE & DECODER_READY, DECODER_READY);
        assert_eq!(ALL_INIT_COMPLETE & IDR_FOUND, IDR_FOUND);
        assert_eq!(ALL_INIT_COMPLETE & DPB_READY, DPB_READY);
    }

    #[test]
    fn test_vp9_transform_exports() {
        // Verify VP9 transform capsule is accessible
        let capsule = Vp9TransformCapsule::new();
        assert_eq!(core::mem::size_of::<Vp9TransformCapsule>(), 256);
        assert_eq!(capsule.generation(), 0);

        // Test transform size enum
        assert_eq!(Vp9TxSize::Tx4x4.dimension(), 4);
        assert_eq!(Vp9TxSize::Tx8x8.dimension(), 8);
        assert_eq!(Vp9TxSize::Tx16x16.dimension(), 16);
        assert_eq!(Vp9TxSize::Tx32x32.dimension(), 32);

        // Test transform type enum
        assert_eq!(Vp9TxType::DctDct.row_type(), Vp9TransformKind::Dct);
        assert_eq!(Vp9TxType::AdstAdst.col_type(), Vp9TransformKind::Adst);

        // Test basic transform
        let input = [0i16; 16];
        let mut output = [0i16; 16];
        capsule.idct_4x4(&input, &mut output);
        assert_eq!(capsule.stats().transforms_4x4, 1);
    }

    #[test]
    fn test_vp9_intra_pred_exports() {
        // Verify VP9 intra prediction types are accessible
        let capsule = Vp9IntraPredCapsule::new();
        assert_eq!(core::mem::size_of::<Vp9IntraPredCapsule>(), 512);
        assert_eq!(capsule.generation(), 0);

        // Test mode enum
        assert_eq!(Vp9IntraMode::from_u8(0), Some(Vp9IntraMode::DcPred));
        assert_eq!(Vp9IntraMode::from_u8(9), Some(Vp9IntraMode::TmPred));
        assert!(!Vp9IntraMode::DcPred.is_directional());
        assert!(Vp9IntraMode::D45Pred.is_directional());

        // Test block size enum
        assert_eq!(Vp9BlockSize::Block4x4.size(), 4);
        assert_eq!(Vp9BlockSize::Block64x64.size(), 64);

        // Test neighbors
        let neighbors = Vp9IntraNeighbors::with_value(100);
        assert!(neighbors.above_available);
        assert!(neighbors.left_available);

        // Test basic prediction
        let mut dst = [0u8; 16];
        let result = capsule.predict(Vp9IntraMode::DcPred, &mut dst, 4, 4, &neighbors);
        assert!(result.is_ok());
        for p in dst.iter() {
            assert_eq!(*p, 100);
        }
    }

    #[test]
    fn test_av1_symbol_decoder_exports() {
        // Verify AV1 symbol decoder types are accessible
        let capsule = Av1SymbolDecoderCapsule::new();
        assert_eq!(core::mem::size_of::<Av1SymbolDecoderCapsule>(), 256);
        assert_eq!(core::mem::align_of::<Av1SymbolDecoderCapsule>(), 128);
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.get_state(), Av1SymbolState::Uninitialized);

        // Test CDF helpers
        let cdf4 = create_uniform_cdf(4);
        assert_eq!(cdf4.len(), 4);
        assert_eq!(cdf4[3], CDF_PROB_TOP as u16);

        let cdf_weighted = create_cdf_from_weights(&[3, 1]);
        assert_eq!(cdf_weighted.len(), 2);
        assert_eq!(cdf_weighted[1], CDF_PROB_TOP as u16);

        // Test constants
        assert_eq!(SYMBOL_BITS, 15);
        assert_eq!(CDF_PROB_BITS, 15);
        assert_eq!(CDF_PROB_TOP, 32768);
        assert_eq!(MIN_RANGE, 256);
        assert_eq!(MAX_RANGE, 65536);
        assert_eq!(MAX_CDF_SYMBOLS, 16);
    }

    #[test]
    fn test_av1_symbol_decoder_init_and_decode() {
        let capsule = Av1SymbolDecoderCapsule::new();

        // Test initialization
        let data = [0x80, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        assert!(capsule.init(&data).is_ok());
        assert_eq!(capsule.get_state(), Av1SymbolState::Initialized);
        assert!(capsule.is_ready());

        // Test boolean decoding
        let result = capsule.decode_bool_eq_prob(&data);
        assert!(result.is_ok());
        assert_eq!(capsule.get_state(), Av1SymbolState::Decoding);

        // Test symbol decoding
        let cdf = create_uniform_cdf(4);
        let symbol_result = capsule.decode_symbol(&cdf, &data);
        assert!(symbol_result.is_ok());
        assert!(symbol_result.unwrap() < 4);

        // Test stats
        let stats = capsule.stats();
        assert!(stats.bools_decoded > 0);
        assert!(stats.symbols_decoded > 0);
    }

    #[test]
    fn test_av1_tile_group_exports() {
        // Verify AV1 tile group capsule types are accessible
        let capsule = Av1TileGroupCapsule::new();
        assert_eq!(core::mem::size_of::<Av1TileGroupCapsule>(), 512);
        assert_eq!(core::mem::align_of::<Av1TileGroupCapsule>(), 128);
        assert_eq!(capsule.generation(), 0);

        // Test constants
        assert_eq!(AV1_MAX_TILE_COLS, 64);
        assert_eq!(AV1_MAX_TILE_ROWS, 64);
        assert_eq!(AV1_MAX_TILES, 4096);
        assert_eq!(AV1_SB_SIZE_64, 64);
        assert_eq!(AV1_SB_SIZE_128, 128);
        assert_eq!(AV1_INLINE_TILE_OFFSETS, 32);

        // Test tile info parsing
        assert!(capsule.parse_tile_info(1920, 1080, AV1_SB_SIZE_64).is_ok());
        assert_eq!(capsule.tile_cols(), 1);
        assert_eq!(capsule.tile_rows(), 1);
        assert!(capsule.is_uniform_tile_spacing());
        assert!(capsule.generation() > 0);

        // Test grid configuration
        assert!(capsule.configure_tile_grid(4, 4, 2, 2, true).is_ok());
        assert_eq!(capsule.tile_cols(), 4);
        assert_eq!(capsule.tile_rows(), 4);
        assert_eq!(capsule.num_tiles(), 16);

        // Test tile coordinates
        let (col, row) = capsule.get_tile_coords(5);
        assert_eq!(col, 1);
        assert_eq!(row, 1);

        // Test error types
        assert_eq!(
            format!("{}", Av1TileGroupError::InvalidTileCols),
            "invalid tile column count"
        );
    }

    #[test]
    fn test_av1_sequence_header_exports() {
        // Verify AV1 sequence header capsule types are accessible
        let capsule = Av1SequenceHeaderCapsule::new();
        assert_eq!(core::mem::size_of::<Av1SequenceHeaderCapsule>(), 512);
        assert_eq!(core::mem::align_of::<Av1SequenceHeaderCapsule>(), 128);
        assert_eq!(capsule.generation(), 0);

        // Test constants
        assert_eq!(MAX_OPERATING_POINTS, 32);
        assert_eq!(MAX_FRAME_WIDTH, 65536);
        assert_eq!(MAX_FRAME_HEIGHT, 65536);
        assert_eq!(NUM_REF_FRAMES, 8);

        // Test profile enum
        assert_eq!(Av1Profile::from_bits(0), Some(Av1Profile::Main));
        assert_eq!(Av1Profile::from_bits(1), Some(Av1Profile::High));
        assert_eq!(Av1Profile::from_bits(2), Some(Av1Profile::Professional));
        assert_eq!(Av1Profile::from_bits(3), None);
        assert!(!Av1Profile::Main.supports_12bit());
        assert!(Av1Profile::Professional.supports_12bit());
        assert!(Av1Profile::High.supports_444());

        // Test color primaries enum
        assert_eq!(Av1ColorPrimaries::from_u8(1), Av1ColorPrimaries::Bt709);
        assert_eq!(Av1ColorPrimaries::from_u8(9), Av1ColorPrimaries::Bt2020);

        // Test transfer characteristics enum
        assert!(Av1TransferCharacteristics::from_u8(16).is_hdr());
        assert!(Av1TransferCharacteristics::from_u8(18).is_hdr());
        assert!(!Av1TransferCharacteristics::from_u8(1).is_hdr());

        // Test matrix coefficients enum
        assert!(Av1MatrixCoefficients::from_u8(0).is_rgb());
        assert!(!Av1MatrixCoefficients::from_u8(1).is_rgb());

        // Test chroma sample position enum
        assert_eq!(Av1ChromaSamplePosition::from_bits(0), Av1ChromaSamplePosition::Unknown);
        assert_eq!(Av1ChromaSamplePosition::from_bits(2), Av1ChromaSamplePosition::Colocated);

        // Test capsule accessors (default values before parsing)
        assert_eq!(capsule.seq_profile(), Av1Profile::Main);
        assert!(!capsule.still_picture());
        assert_eq!(capsule.get_bit_depth(), 0);
        // Note: get_num_planes() returns 1 when mono_chrome bit is 0 in default state
        // because bit_depth_config default is 0, and (0 & (1 << 4)) == 0 means !mono_chrome -> 3 planes
        assert_eq!(capsule.get_num_planes(), 3);  // Default: not mono_chrome

        // Test reset
        capsule.reset();
        assert_eq!(capsule.generation(), 1);

        // Test error types
        assert_eq!(
            format!("{}", Av1SequenceHeaderError::InvalidProfile),
            "Invalid profile (must be 0-2)"
        );
    }

    #[test]
    fn test_av1_bitstream_capsule_exports() {
        // Verify AV1 bitstream capsule types are accessible
        let capsule = Av1BitstreamCapsule::new();
        assert_eq!(core::mem::size_of::<Av1BitstreamCapsule>(), 512);
        assert_eq!(core::mem::align_of::<Av1BitstreamCapsule>(), 128);
        assert_eq!(capsule.generation(), 0);

        // Test OBU type enum (from_u8 returns ObuType directly, Reserved for invalid)
        assert_eq!(ObuType::from_u8(1), ObuType::SequenceHeader);
        assert_eq!(ObuType::from_u8(2), ObuType::TemporalDelimiter);
        assert_eq!(ObuType::from_u8(3), ObuType::FrameHeader);
        assert_eq!(ObuType::from_u8(4), ObuType::TileGroup);
        assert_eq!(ObuType::from_u8(5), ObuType::Metadata);
        assert_eq!(ObuType::from_u8(6), ObuType::Frame);
        assert_eq!(ObuType::from_u8(7), ObuType::RedundantFrameHeader);
        assert_eq!(ObuType::from_u8(8), ObuType::TileList);
        assert_eq!(ObuType::from_u8(15), ObuType::Padding);
        assert_eq!(ObuType::from_u8(10), ObuType::Reserved); // Invalid returns Reserved

        // Test OBU type classification
        assert!(ObuType::SequenceHeader.is_header());
        assert!(!ObuType::TileGroup.is_header());
        assert!(ObuType::TileGroup.has_frame_data());
        assert!(!ObuType::Metadata.has_frame_data());

        // Test LEB128 decoding
        let leb128_10 = [0x0A]; // 10
        let result = capsule.read_leb128(&leb128_10);
        assert!(result.is_ok());
        let (value, bytes) = result.unwrap();
        assert_eq!(value, 10);
        assert_eq!(bytes, 1);

        let leb128_300 = [0xAC, 0x02]; // 300 (172 + 128)
        let result2 = capsule.read_leb128(&leb128_300);
        assert!(result2.is_ok());
        let (value2, bytes2) = result2.unwrap();
        assert_eq!(value2, 300);
        assert_eq!(bytes2, 2);

        // Test OBU header parsing
        // 0x12 = type 2 (temporal delimiter), has_size=1
        let obu_header = [0x12, 0x00];
        let result = capsule.parse_obu_header(&obu_header);
        assert!(result.is_ok());
        let header = result.unwrap();
        assert_eq!(header.obu_type, ObuType::TemporalDelimiter);
        assert!(!header.obu_extension_flag);
        assert!(header.obu_has_size_field);
    }

    #[test]
    fn test_av1_bitstream_temporal_unit_parsing() {
        let capsule = Av1BitstreamCapsule::new();

        // Build a proper temporal unit:
        // - Sequence Header (type=1, has_size=1, size=5)
        // - Frame (type=6, has_size=1, size=5)
        let mut temporal_unit = Vec::new();

        // Sequence Header: 0x0A = type 1, has_size=1
        temporal_unit.push(0x0A);
        temporal_unit.push(0x05); // size=5
        temporal_unit.extend_from_slice(&[0x00; 5]); // 5 bytes payload

        // Frame: 0x32 = type 6, has_size=1
        temporal_unit.push(0x32);
        temporal_unit.push(0x05); // size=5
        temporal_unit.extend_from_slice(&[0x00; 5]); // 5 bytes payload

        let result = capsule.parse_temporal_unit(&temporal_unit);
        assert!(result.is_ok());
        let tu = result.unwrap();
        assert!(tu.has_sequence_header);
        assert_eq!(tu.obus.len(), 2);
        assert!(capsule.generation() > 0);

        // Test stats
        let stats = capsule.stats();
        assert!(stats.obus_parsed >= 2);
        assert!(stats.sequence_headers_seen >= 1);
    }

    #[test]
    fn test_av1_bitstream_error_types() {
        // Test error types are accessible
        assert_eq!(
            format!("{}", Av1Error::UnexpectedEof),
            "Unexpected end of stream"
        );
        assert_eq!(
            format!("{}", Av1Error::InvalidObuType),
            "Invalid OBU type"
        );
        assert_eq!(
            format!("{}", Av1Error::ObuForbiddenBitSet),
            "OBU forbidden bit set"
        );
        assert_eq!(
            format!("{}", Av1Error::Leb128Overflow),
            "LEB128 overflow"
        );
    }

    #[test]
    fn test_av1_loop_filter_exports() {
        // Verify AV1 loop filter capsule types are accessible
        let capsule = Av1LoopFilterCapsule::new();
        assert_eq!(core::mem::size_of::<Av1LoopFilterCapsule>(), 512);
        assert_eq!(core::mem::align_of::<Av1LoopFilterCapsule>(), 128);
        assert_eq!(capsule.generation(), 0);

        // Test constants
        assert_eq!(CDEF_DIRECTIONS.len(), 8);
        assert_eq!(DEBLOCK_ALPHA_TABLE.len(), 64);
        assert_eq!(DEBLOCK_BETA_TABLE.len(), 64);

        // Test restoration type enum
        assert_eq!(Av1RestorationType::from_u8(0), Some(Av1RestorationType::None));
        assert_eq!(Av1RestorationType::from_u8(1), Some(Av1RestorationType::Wiener));
        assert_eq!(Av1RestorationType::from_u8(2), Some(Av1RestorationType::SgrProj));
        assert_eq!(Av1RestorationType::from_u8(3), Some(Av1RestorationType::Switchable));
        assert!(!Av1RestorationType::None.is_active());
        assert!(Av1RestorationType::Wiener.is_active());

        // Test configuration
        assert!(capsule.configure_deblock(32, 32, 16, 16, 4).is_ok());
        assert!(capsule.is_deblock_enabled());
        assert_eq!(capsule.filter_level_y_v(), 32);

        // Test error types
        assert_eq!(
            format!("{}", Av1LoopFilterError::InvalidLevel),
            "Filter level must be 0-63"
        );
        assert_eq!(
            format!("{}", Av1LoopFilterError::InvalidCdefStrength),
            "CDEF strength must be 0-63"
        );
    }

    #[test]
    fn test_av1_loop_filter_cdef_and_lrf() {
        let capsule = Av1LoopFilterCapsule::new();

        // Test CDEF configuration
        let y_strengths = [0x24, 0x12, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0];
        let uv_strengths = [0x12, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0];
        assert!(capsule.configure_cdef(4, 2, &y_strengths, &uv_strengths).is_ok());
        assert!(capsule.is_cdef_enabled());
        assert_eq!(capsule.cdef_damping(), 4);

        // Test LRF configuration
        assert!(capsule.configure_lrf(Av1RestorationType::Wiener, Av1RestorationType::None, 6).is_ok());
        assert!(capsule.is_lrf_enabled());
        assert_eq!(capsule.lr_type_y(), Av1RestorationType::Wiener);
        assert_eq!(capsule.lr_type_uv(), Av1RestorationType::None);

        // Test superblock processing
        let mut sb = vec![128u8; 64 * 64];
        assert!(capsule.process_superblock(&mut sb, 0, 0).is_ok());
        let stats = capsule.stats();
        assert!(stats.superblocks_processed >= 1);
    }

    #[test]
    fn test_av1_intra_pred_exports() {
        // Verify AV1 intra prediction capsule types are accessible
        let capsule = Av1IntraPredCapsule::new();
        assert_eq!(core::mem::size_of::<Av1IntraPredCapsule>(), 512);
        assert_eq!(core::mem::align_of::<Av1IntraPredCapsule>(), 128);
        assert_eq!(capsule.generation(), 0);

        // Test mode enums
        assert_eq!(Av1IntraMode::from_u8(0), Some(Av1IntraMode::DcPred));
        assert_eq!(Av1IntraMode::from_u8(12), Some(Av1IntraMode::PaethPred));
        assert!(Av1IntraMode::D45Pred.is_directional());
        assert!(!Av1IntraMode::DcPred.is_directional());

        // Test filter intra modes
        assert_eq!(Av1FilterIntraMode::from_u8(0), Some(Av1FilterIntraMode::FilterDcPred));
        assert_eq!(Av1FilterIntraMode::from_u8(4), Some(Av1FilterIntraMode::FilterPaethPred));

        // Test neighbors
        let neighbors = Av1IntraNeighbors::with_value(128);
        assert!(neighbors.above_available);
        assert!(neighbors.left_available);

        // Test basic DC prediction via main predict() method (which increments stats)
        let top = [128u8; 8];
        let left = [128u8; 8];
        let mut dst = [0u8; 16];
        let result = capsule.predict(Av1IntraMode::DcPred, &top, &left, &mut dst, 4, 4);
        assert!(result.is_ok());

        // Test stats (only incremented when called through predict())
        let stats = capsule.stats();
        assert!(stats.dc_predictions > 0);

        // Test constants
        assert_eq!(NOMINAL_ANGLES.len(), 8);
        assert_eq!(MAX_ANGLE_DELTA, 3);
        assert_eq!(DR_INTRA_DERIVATIVE.len(), 56);
        assert_eq!(SMOOTH_WEIGHTS_4.len(), 4);
        assert_eq!(SMOOTH_WEIGHTS_8.len(), 8);
        assert_eq!(SMOOTH_WEIGHTS_16.len(), 16);
        assert_eq!(SMOOTH_WEIGHTS_32.len(), 32);
        assert_eq!(SMOOTH_WEIGHTS_64.len(), 64);
        assert!(FILTER_INTRA_TAPS.len() > 0);
    }

    #[test]
    fn test_av1_intra_pred_all_modes() {
        let capsule = Av1IntraPredCapsule::new();
        let top = [128u8; 8];
        let left = [128u8; 8];
        let mut dst = [0u8; 64];

        // Test all 13 modes (4x4 block)
        let modes = [
            Av1IntraMode::DcPred,
            Av1IntraMode::VPred,
            Av1IntraMode::HPred,
            Av1IntraMode::D45Pred,
            Av1IntraMode::D135Pred,
            Av1IntraMode::D113Pred,
            Av1IntraMode::D157Pred,
            Av1IntraMode::D203Pred,
            Av1IntraMode::D67Pred,
            Av1IntraMode::SmoothPred,
            Av1IntraMode::SmoothVPred,
            Av1IntraMode::SmoothHPred,
            Av1IntraMode::PaethPred,
        ];

        for mode in modes.iter() {
            dst.fill(0);
            let result = capsule.predict(*mode, &top, &left, &mut dst, 4, 4);
            assert!(result.is_ok(), "Failed on mode {:?}", mode);
        }
    }

    #[test]
    fn test_av1_intra_pred_filter_intra() {
        let capsule = Av1IntraPredCapsule::new();
        let top = [100u8; 8];
        let left = [100u8; 8];
        let mut dst = [0u8; 16];

        // Test all 5 filter intra modes (mode is u8 in actual API)
        for mode_idx in 0..5u8 {
            dst.fill(0);
            let result = capsule.predict_filter_intra(mode_idx, &top, &left, &mut dst, 4, 4);
            assert!(result.is_ok(), "Filter intra failed on mode {}", mode_idx);
        }

        let stats = capsule.stats();
        assert!(stats.filter_intra_predictions > 0);
    }

    #[test]
    fn test_av1_intra_pred_cfl() {
        let capsule = Av1IntraPredCapsule::new();
        let ac_pred = [0i16; 16]; // AC component (residual)
        let dc_pred = 128u16; // DC prediction value
        let mut dst = [0u8; 16];

        // Test CfL prediction with various alpha values
        for alpha in [-8i8, -4, 0, 4, 8] {
            dst.fill(0);
            let result = capsule.predict_cfl(&ac_pred, dc_pred, alpha, &mut dst);
            assert!(result.is_ok(), "CfL failed with alpha {}", alpha);
        }

        let stats = capsule.stats();
        assert!(stats.cfl_predictions > 0);
    }
}
