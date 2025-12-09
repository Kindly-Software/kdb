//! H.264 Macroblock Decoder
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Implements ITU-T H.264 Section 7.3.5 macroblock layer decoding.
//!
//! # Architecture
//!
//! A macroblock is the basic unit of H.264 decoding:
//! - 16x16 luma samples
//! - 8x8 Cb chroma samples (4:2:0)
//! - 8x8 Cr chroma samples (4:2:0)
//!
//! # Macroblock Types
//!
//! - I_NxN: Intra 4x4 or 8x8 prediction
//! - I_16x16: Intra 16x16 prediction
//! - I_PCM: Raw PCM samples
//! - P_L0_16x16: Inter 16x16 from L0
//! - P_8x8: Inter with 8x8 sub-partitions
//! - B_*: Bi-predictive modes
//!
//! # UCE34/Chaos Compliance
//!
//! - **Q10**: T4 Batch tier (macroblock-level batch processing)
//! - **Q33**: 100% lockfree (AtomicU64/AtomicU32 only)
//! - **Q34**: Generation counter for audit trail
//! - 512B cache-aligned
//!
//! # References
//!
//! - ITU-T H.264 Section 7.3.5 (Macroblock layer syntax)
//! - ITU-T H.264 Section 6.4.11 (Neighbor derivation)
//! - ITU-T H.264 Table 7-11 (I slice mb_type)
//! - ITU-T H.264 Table 7-13 (P slice mb_type)
//! - ITU-T H.264 Table 7-17 (sub_mb_type for P slices)

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use super::h264_cabac::{
    CabacContextTable, CabacDecoderCapsule, CabacError, SliceType,
    context_idx,
};

// ============================================================================
// Macroblock Type Enumerations (ITU-T H.264 Tables 7-11, 7-13, 7-14)
// ============================================================================

/// Macroblock type for I slices (Table 7-11)
///
/// I slice macroblocks use only intra prediction (no motion compensation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MbTypeI {
    /// Intra 4x4 or 8x8 prediction
    INxN = 0,
    /// Intra 16x16, cbp_luma=0, cbp_chroma=0, pred_mode=0 (Vertical)
    I16x16_0_0_0 = 1,
    /// Intra 16x16, cbp_luma=0, cbp_chroma=0, pred_mode=1 (Horizontal)
    I16x16_1_0_0 = 2,
    /// Intra 16x16, cbp_luma=0, cbp_chroma=0, pred_mode=2 (DC)
    I16x16_2_0_0 = 3,
    /// Intra 16x16, cbp_luma=0, cbp_chroma=0, pred_mode=3 (Plane)
    I16x16_3_0_0 = 4,
    /// Intra 16x16, cbp_luma=0, cbp_chroma=1, pred_mode=0
    I16x16_0_1_0 = 5,
    /// Intra 16x16, cbp_luma=0, cbp_chroma=1, pred_mode=1
    I16x16_1_1_0 = 6,
    /// Intra 16x16, cbp_luma=0, cbp_chroma=1, pred_mode=2
    I16x16_2_1_0 = 7,
    /// Intra 16x16, cbp_luma=0, cbp_chroma=1, pred_mode=3
    I16x16_3_1_0 = 8,
    /// Intra 16x16, cbp_luma=0, cbp_chroma=2, pred_mode=0
    I16x16_0_2_0 = 9,
    /// Intra 16x16, cbp_luma=0, cbp_chroma=2, pred_mode=1
    I16x16_1_2_0 = 10,
    /// Intra 16x16, cbp_luma=0, cbp_chroma=2, pred_mode=2
    I16x16_2_2_0 = 11,
    /// Intra 16x16, cbp_luma=0, cbp_chroma=2, pred_mode=3
    I16x16_3_2_0 = 12,
    /// Intra 16x16, cbp_luma=15, cbp_chroma=0, pred_mode=0
    I16x16_0_0_1 = 13,
    /// Intra 16x16, cbp_luma=15, cbp_chroma=0, pred_mode=1
    I16x16_1_0_1 = 14,
    /// Intra 16x16, cbp_luma=15, cbp_chroma=0, pred_mode=2
    I16x16_2_0_1 = 15,
    /// Intra 16x16, cbp_luma=15, cbp_chroma=0, pred_mode=3
    I16x16_3_0_1 = 16,
    /// Intra 16x16, cbp_luma=15, cbp_chroma=1, pred_mode=0
    I16x16_0_1_1 = 17,
    /// Intra 16x16, cbp_luma=15, cbp_chroma=1, pred_mode=1
    I16x16_1_1_1 = 18,
    /// Intra 16x16, cbp_luma=15, cbp_chroma=1, pred_mode=2
    I16x16_2_1_1 = 19,
    /// Intra 16x16, cbp_luma=15, cbp_chroma=1, pred_mode=3
    I16x16_3_1_1 = 20,
    /// Intra 16x16, cbp_luma=15, cbp_chroma=2, pred_mode=0
    I16x16_0_2_1 = 21,
    /// Intra 16x16, cbp_luma=15, cbp_chroma=2, pred_mode=1
    I16x16_1_2_1 = 22,
    /// Intra 16x16, cbp_luma=15, cbp_chroma=2, pred_mode=2
    I16x16_2_2_1 = 23,
    /// Intra 16x16, cbp_luma=15, cbp_chroma=2, pred_mode=3
    I16x16_3_2_1 = 24,
    /// Raw PCM samples (no prediction/transform)
    IPCM = 25,
}

impl MbTypeI {
    /// Check if this is an Intra 16x16 type
    #[inline]
    pub const fn is_intra_16x16(&self) -> bool {
        (*self as u8) >= 1 && (*self as u8) <= 24
    }

    /// Check if this is I_PCM type
    #[inline]
    pub const fn is_pcm(&self) -> bool {
        matches!(self, Self::IPCM)
    }

    /// Check if this is I_NxN type (4x4 or 8x8)
    #[inline]
    pub const fn is_intra_nxn(&self) -> bool {
        matches!(self, Self::INxN)
    }

    /// Extract Intra16x16PredMode from mb_type (0-3)
    #[inline]
    pub const fn intra_16x16_pred_mode(&self) -> Option<u8> {
        if self.is_intra_16x16() {
            Some(((*self as u8) - 1) % 4)
        } else {
            None
        }
    }

    /// Extract CodedBlockPatternLuma from mb_type (0 or 15)
    #[inline]
    pub const fn cbp_luma(&self) -> Option<u8> {
        if self.is_intra_16x16() {
            let val = (*self as u8) - 1;
            if val >= 12 {
                Some(15)
            } else {
                Some(0)
            }
        } else {
            None
        }
    }

    /// Extract CodedBlockPatternChroma from mb_type (0, 1, or 2)
    #[inline]
    pub const fn cbp_chroma(&self) -> Option<u8> {
        if self.is_intra_16x16() {
            let val = (*self as u8) - 1;
            Some(((val % 12) / 4) as u8)
        } else {
            None
        }
    }
}

impl From<u8> for MbTypeI {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::INxN,
            1 => Self::I16x16_0_0_0,
            2 => Self::I16x16_1_0_0,
            3 => Self::I16x16_2_0_0,
            4 => Self::I16x16_3_0_0,
            5 => Self::I16x16_0_1_0,
            6 => Self::I16x16_1_1_0,
            7 => Self::I16x16_2_1_0,
            8 => Self::I16x16_3_1_0,
            9 => Self::I16x16_0_2_0,
            10 => Self::I16x16_1_2_0,
            11 => Self::I16x16_2_2_0,
            12 => Self::I16x16_3_2_0,
            13 => Self::I16x16_0_0_1,
            14 => Self::I16x16_1_0_1,
            15 => Self::I16x16_2_0_1,
            16 => Self::I16x16_3_0_1,
            17 => Self::I16x16_0_1_1,
            18 => Self::I16x16_1_1_1,
            19 => Self::I16x16_2_1_1,
            20 => Self::I16x16_3_1_1,
            21 => Self::I16x16_0_2_1,
            22 => Self::I16x16_1_2_1,
            23 => Self::I16x16_2_2_1,
            24 => Self::I16x16_3_2_1,
            25 => Self::IPCM,
            _ => Self::INxN, // Default to I_NxN for invalid values
        }
    }
}

/// Macroblock type for P slices (Table 7-13)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MbTypeP {
    /// Inter 16x16 prediction from L0
    PL0_16x16 = 0,
    /// Inter 16x8 prediction (2 partitions) from L0
    PL0L0_16x8 = 1,
    /// Inter 8x16 prediction (2 partitions) from L0
    PL0L0_8x16 = 2,
    /// Inter 8x8 prediction (4 partitions)
    P8x8 = 3,
    /// Inter 8x8 with ref_idx_l0 = 0 for all partitions
    P8x8ref0 = 4,
    /// Skip mode (motion inferred from neighbors)
    PSkip = 5,
}

impl MbTypeP {
    /// Number of partitions for this mb_type
    #[inline]
    pub const fn num_partitions(&self) -> u8 {
        match self {
            Self::PL0_16x16 | Self::PSkip => 1,
            Self::PL0L0_16x8 | Self::PL0L0_8x16 => 2,
            Self::P8x8 | Self::P8x8ref0 => 4,
        }
    }

    /// Check if this is a skip type
    #[inline]
    pub const fn is_skip(&self) -> bool {
        matches!(self, Self::PSkip)
    }

    /// Partition width for given partition index
    #[inline]
    pub const fn partition_width(&self, _part_idx: u8) -> u8 {
        match self {
            Self::PL0_16x16 | Self::PL0L0_16x8 | Self::PSkip => 16,
            Self::PL0L0_8x16 | Self::P8x8 | Self::P8x8ref0 => 8,
        }
    }

    /// Partition height for given partition index
    #[inline]
    pub const fn partition_height(&self, _part_idx: u8) -> u8 {
        match self {
            Self::PL0_16x16 | Self::PL0L0_8x16 | Self::PSkip => 16,
            Self::PL0L0_16x8 | Self::P8x8 | Self::P8x8ref0 => 8,
        }
    }
}

impl From<u8> for MbTypeP {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::PL0_16x16,
            1 => Self::PL0L0_16x8,
            2 => Self::PL0L0_8x16,
            3 => Self::P8x8,
            4 => Self::P8x8ref0,
            5 => Self::PSkip,
            _ => Self::PL0_16x16, // Default
        }
    }
}

/// Sub-macroblock type for P slices (Table 7-17)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SubMbTypeP {
    /// 8x8 sub-partition
    PL0_8x8 = 0,
    /// 8x4 sub-partition (2 sub-blocks)
    PL0_8x4 = 1,
    /// 4x8 sub-partition (2 sub-blocks)
    PL0_4x8 = 2,
    /// 4x4 sub-partition (4 sub-blocks)
    PL0_4x4 = 3,
}

impl SubMbTypeP {
    /// Number of sub-partitions
    #[inline]
    pub const fn num_sub_partitions(&self) -> u8 {
        match self {
            Self::PL0_8x8 => 1,
            Self::PL0_8x4 | Self::PL0_4x8 => 2,
            Self::PL0_4x4 => 4,
        }
    }

    /// Sub-partition width
    #[inline]
    pub const fn sub_width(&self) -> u8 {
        match self {
            Self::PL0_8x8 | Self::PL0_8x4 => 8,
            Self::PL0_4x8 | Self::PL0_4x4 => 4,
        }
    }

    /// Sub-partition height
    #[inline]
    pub const fn sub_height(&self) -> u8 {
        match self {
            Self::PL0_8x8 | Self::PL0_4x8 => 8,
            Self::PL0_8x4 | Self::PL0_4x4 => 4,
        }
    }
}

impl From<u8> for SubMbTypeP {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::PL0_8x8,
            1 => Self::PL0_8x4,
            2 => Self::PL0_4x8,
            3 => Self::PL0_4x4,
            _ => Self::PL0_8x8, // Default
        }
    }
}

// ============================================================================
// Intra Prediction Modes (ITU-T H.264 Tables 8-2, 8-3, 8-4)
// ============================================================================

/// Intra 4x4 prediction mode (Table 8-2)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Intra4x4PredMode {
    /// Vertical prediction (samples from above)
    Vertical = 0,
    /// Horizontal prediction (samples from left)
    Horizontal = 1,
    /// DC prediction (average of neighbors)
    DC = 2,
    /// Diagonal down-left prediction
    DiagonalDownLeft = 3,
    /// Diagonal down-right prediction
    DiagonalDownRight = 4,
    /// Vertical-right prediction
    VerticalRight = 5,
    /// Horizontal-down prediction
    HorizontalDown = 6,
    /// Vertical-left prediction
    VerticalLeft = 7,
    /// Horizontal-up prediction
    HorizontalUp = 8,
}

impl From<u8> for Intra4x4PredMode {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::Vertical,
            1 => Self::Horizontal,
            2 => Self::DC,
            3 => Self::DiagonalDownLeft,
            4 => Self::DiagonalDownRight,
            5 => Self::VerticalRight,
            6 => Self::HorizontalDown,
            7 => Self::VerticalLeft,
            8 => Self::HorizontalUp,
            _ => Self::DC, // Default
        }
    }
}

/// Intra 16x16 prediction mode (Table 8-3)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Intra16x16PredMode {
    /// Vertical prediction
    Vertical = 0,
    /// Horizontal prediction
    Horizontal = 1,
    /// DC prediction
    DC = 2,
    /// Plane prediction
    Plane = 3,
}

impl From<u8> for Intra16x16PredMode {
    fn from(v: u8) -> Self {
        match v % 4 {
            0 => Self::Vertical,
            1 => Self::Horizontal,
            2 => Self::DC,
            3 => Self::Plane,
            _ => Self::DC,
        }
    }
}

/// Intra chroma prediction mode (Table 8-4)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum IntraChromaPredMode {
    /// DC prediction
    DC = 0,
    /// Horizontal prediction
    Horizontal = 1,
    /// Vertical prediction
    Vertical = 2,
    /// Plane prediction
    Plane = 3,
}

impl From<u8> for IntraChromaPredMode {
    fn from(v: u8) -> Self {
        match v % 4 {
            0 => Self::DC,
            1 => Self::Horizontal,
            2 => Self::Vertical,
            3 => Self::Plane,
            _ => Self::DC,
        }
    }
}

// ============================================================================
// Macroblock Data Structure
// ============================================================================

/// Decoded macroblock data container
///
/// Contains all decoded syntax elements for a single macroblock.
#[derive(Debug, Clone)]
pub struct MacroblockData {
    /// Macroblock type (raw value)
    pub mb_type: u8,
    /// Macroblock X position in picture
    pub mb_x: u16,
    /// Macroblock Y position in picture
    pub mb_y: u16,

    // Intra prediction modes
    /// Intra 4x4 prediction modes for 16 4x4 blocks
    pub intra_4x4_pred_modes: [u8; 16],
    /// Intra 16x16 prediction mode
    pub intra_16x16_pred_mode: u8,
    /// Intra chroma prediction mode
    pub intra_chroma_pred_mode: u8,

    // Inter prediction
    /// Reference indices for L0 (4 8x8 partitions)
    pub ref_idx_l0: [i8; 4],
    /// Reference indices for L1 (4 8x8 partitions)
    pub ref_idx_l1: [i8; 4],
    /// Motion vector differences L0 (16 4x4 blocks)
    pub mvd_l0: [(i16, i16); 16],
    /// Motion vector differences L1 (16 4x4 blocks)
    pub mvd_l1: [(i16, i16); 16],

    // Coded block pattern
    /// CBP luma (4 bits for 8x8 blocks)
    pub cbp_luma: u8,
    /// CBP chroma (0=none, 1=DC only, 2=DC+AC)
    pub cbp_chroma: u8,

    // QP values
    /// Luma QP
    pub qp_y: u8,
    /// Cb chroma QP
    pub qp_cb: u8,
    /// Cr chroma QP
    pub qp_cr: u8,

    // Residual data (transform coefficients)
    /// Luma residual (16 4x4 blocks, each with 16 coefficients)
    pub residual_luma: [[i16; 16]; 16],
    /// Cb chroma residual (4 4x4 blocks)
    pub residual_cb: [[i16; 16]; 4],
    /// Cr chroma residual (4 4x4 blocks)
    pub residual_cr: [[i16; 16]; 4],

    // DC coefficients (for Intra16x16)
    /// Luma DC coefficients
    pub luma_dc: [i16; 16],
    /// Cb DC coefficients
    pub cb_dc: [i16; 4],
    /// Cr DC coefficients
    pub cr_dc: [i16; 4],
}

impl Default for MacroblockData {
    fn default() -> Self {
        Self {
            mb_type: 0,
            mb_x: 0,
            mb_y: 0,
            intra_4x4_pred_modes: [0; 16],
            intra_16x16_pred_mode: 0,
            intra_chroma_pred_mode: 0,
            ref_idx_l0: [-1; 4],
            ref_idx_l1: [-1; 4],
            mvd_l0: [(0, 0); 16],
            mvd_l1: [(0, 0); 16],
            cbp_luma: 0,
            cbp_chroma: 0,
            qp_y: 26,
            qp_cb: 26,
            qp_cr: 26,
            residual_luma: [[0; 16]; 16],
            residual_cb: [[0; 16]; 4],
            residual_cr: [[0; 16]; 4],
            luma_dc: [0; 16],
            cb_dc: [0; 4],
            cr_dc: [0; 4],
        }
    }
}

// ============================================================================
// Macroblock Error Types
// ============================================================================

/// Macroblock decoding errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MacroblockError {
    /// No error
    None = 0,
    /// Invalid macroblock type
    InvalidMbType = 1,
    /// Invalid sub-macroblock type
    InvalidSubMbType = 2,
    /// Invalid prediction mode
    InvalidPredMode = 3,
    /// Invalid coded block pattern
    InvalidCbp = 4,
    /// CABAC decoding error
    CabacError = 5,
    /// Out of bounds access
    OutOfBounds = 6,
    /// Invalid reference index
    InvalidRefIdx = 7,
    /// Invalid motion vector
    InvalidMvd = 8,
}

impl From<CabacError> for MacroblockError {
    fn from(_: CabacError) -> Self {
        Self::CabacError
    }
}

// ============================================================================
// Macroblock Statistics
// ============================================================================

/// Macroblock statistics snapshot
#[derive(Debug, Clone, Copy, Default)]
pub struct MacroblockStats {
    /// Total macroblocks decoded
    pub mbs_decoded: u64,
    /// Intra macroblocks decoded
    pub intra_mbs: u64,
    /// Inter macroblocks decoded
    pub inter_mbs: u64,
    /// Skip macroblocks decoded
    pub skip_mbs: u64,
    /// PCM macroblocks decoded
    pub pcm_mbs: u64,
    /// Current macroblock X position
    pub current_mb_x: u32,
    /// Current macroblock Y position
    pub current_mb_y: u32,
    /// Current generation
    pub generation: u64,
}

// ============================================================================
// H264MacroblockCapsule - T4 Batch Tier
// ============================================================================

/// T4 Batch capsule for H.264 macroblock decoding
///
/// This capsule manages the decoding state for macroblock-level operations.
/// It coordinates with CabacDecoderCapsule for entropy decoding.
///
/// # Layout (512B cache-aligned)
///
/// ```text
/// Offset  Field                   Size    Description
/// ------  -----                   ----    -----------
/// 0       mb_x                    4       Current MB X position
/// 4       mb_y                    4       Current MB Y position
/// 8       mb_addr                 8       Linear macroblock address
/// 16      pic_width_in_mbs        4       Picture width in MBs
/// 20      pic_height_in_mbs       4       Picture height in MBs
/// 24      mbs_decoded             8       Total MBs decoded (stats)
/// 32      intra_mbs               8       Intra MBs decoded (stats)
/// 40      inter_mbs               8       Inter MBs decoded (stats)
/// 48      skip_mbs                8       Skip MBs decoded (stats)
/// 56      pcm_mbs                 8       PCM MBs decoded (stats)
/// 64      slice_type              4       Current slice type
/// 68      slice_qp                4       Current slice QP
/// 72      generation              8       Generation counter (Q34)
/// 80      mb_avail_a              8       Left neighbor available
/// 88      mb_avail_b              8       Above neighbor available
/// 96      mb_avail_c              8       Above-right neighbor available
/// 104     mb_avail_d              8       Above-left neighbor available
/// 112     prev_mb_skip_flag       4       Previous MB skip flag
/// 116     prev_mb_type            4       Previous MB type
/// 120     transform_8x8_mode      4       Transform 8x8 mode flag
/// 124     constrained_intra       4       Constrained intra prediction flag
/// 128     _padding                384     Padding to 512B
/// ```
#[repr(C, align(512))]
pub struct H264MacroblockCapsule {
    // Current macroblock position (16 bytes)
    /// Current macroblock X coordinate
    pub mb_x: AtomicU32,
    /// Current macroblock Y coordinate
    pub mb_y: AtomicU32,
    /// Linear macroblock address (mb_y * width + mb_x)
    pub mb_addr: AtomicU64,

    // Frame dimensions (8 bytes)
    /// Picture width in macroblocks
    pub pic_width_in_mbs: AtomicU32,
    /// Picture height in macroblocks
    pub pic_height_in_mbs: AtomicU32,

    // Statistics (40 bytes)
    /// Total macroblocks decoded
    pub mbs_decoded: AtomicU64,
    /// Intra macroblocks decoded
    pub intra_mbs: AtomicU64,
    /// Inter macroblocks decoded
    pub inter_mbs: AtomicU64,
    /// Skip macroblocks decoded
    pub skip_mbs: AtomicU64,
    /// PCM macroblocks decoded
    pub pcm_mbs: AtomicU64,

    // Current slice info (8 bytes)
    /// Current slice type (0=P, 1=B, 2=I, 3=SP, 4=SI)
    pub slice_type: AtomicU32,
    /// Current slice QP value
    pub slice_qp: AtomicU32,

    // Generation counter (8 bytes)
    /// Generation counter for audit trail (Q34)
    pub generation: AtomicU64,

    // Neighbor availability flags (32 bytes)
    /// mbAddrA (left) available
    pub mb_avail_a: AtomicU64,
    /// mbAddrB (above) available
    pub mb_avail_b: AtomicU64,
    /// mbAddrC (above-right) available
    pub mb_avail_c: AtomicU64,
    /// mbAddrD (above-left) available
    pub mb_avail_d: AtomicU64,

    // Previous MB state (16 bytes)
    /// Previous macroblock skip flag
    prev_mb_skip_flag: AtomicU32,
    /// Previous macroblock type
    prev_mb_type: AtomicU32,
    /// Transform 8x8 mode enabled
    transform_8x8_mode: AtomicU32,
    /// Constrained intra prediction flag
    constrained_intra: AtomicU32,

    // Padding to 512 bytes
    _padding: [u8; 384],
}

// Safety: H264MacroblockCapsule only contains atomic types
unsafe impl Send for H264MacroblockCapsule {}
unsafe impl Sync for H264MacroblockCapsule {}

impl Default for H264MacroblockCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl H264MacroblockCapsule {
    /// Create a new macroblock decoder capsule
    pub const fn new() -> Self {
        Self {
            mb_x: AtomicU32::new(0),
            mb_y: AtomicU32::new(0),
            mb_addr: AtomicU64::new(0),
            pic_width_in_mbs: AtomicU32::new(0),
            pic_height_in_mbs: AtomicU32::new(0),
            mbs_decoded: AtomicU64::new(0),
            intra_mbs: AtomicU64::new(0),
            inter_mbs: AtomicU64::new(0),
            skip_mbs: AtomicU64::new(0),
            pcm_mbs: AtomicU64::new(0),
            slice_type: AtomicU32::new(2), // Default to I slice
            slice_qp: AtomicU32::new(26),  // Default QP
            generation: AtomicU64::new(0),
            mb_avail_a: AtomicU64::new(0),
            mb_avail_b: AtomicU64::new(0),
            mb_avail_c: AtomicU64::new(0),
            mb_avail_d: AtomicU64::new(0),
            prev_mb_skip_flag: AtomicU32::new(0),
            prev_mb_type: AtomicU32::new(0),
            transform_8x8_mode: AtomicU32::new(0),
            constrained_intra: AtomicU32::new(0),
            _padding: [0u8; 384],
        }
    }

    /// Set frame dimensions in macroblocks
    pub fn set_frame_dimensions(&self, width_mbs: u32, height_mbs: u32) {
        self.pic_width_in_mbs.store(width_mbs, Ordering::Release);
        self.pic_height_in_mbs.store(height_mbs, Ordering::Release);

        // Reset position to start
        self.mb_x.store(0, Ordering::Release);
        self.mb_y.store(0, Ordering::Release);
        self.mb_addr.store(0, Ordering::Release);

        // Update neighbor availability
        self.update_neighbor_availability();

        // Increment generation
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Set current slice info
    pub fn set_slice_info(&self, slice_type: u8, qp: u8) {
        self.slice_type.store(slice_type as u32, Ordering::Release);
        self.slice_qp.store(qp as u32, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Set transform 8x8 mode flag
    pub fn set_transform_8x8_mode(&self, enabled: bool) {
        self.transform_8x8_mode
            .store(if enabled { 1 } else { 0 }, Ordering::Release);
    }

    /// Set constrained intra prediction flag
    pub fn set_constrained_intra_pred(&self, enabled: bool) {
        self.constrained_intra
            .store(if enabled { 1 } else { 0 }, Ordering::Release);
    }

    /// Update neighbor availability based on current position
    ///
    /// ITU-T H.264 Section 6.4.11 - Derivation process for neighboring macroblocks
    pub fn update_neighbor_availability(&self) {
        let mb_x = self.mb_x.load(Ordering::Acquire);
        let mb_y = self.mb_y.load(Ordering::Acquire);
        let width = self.pic_width_in_mbs.load(Ordering::Acquire);

        // mbAddrA (left) available if not at left edge
        let avail_a = mb_x > 0;

        // mbAddrB (above) available if not at top edge
        let avail_b = mb_y > 0;

        // mbAddrC (above-right) available if not at right edge and not at top
        let avail_c = mb_y > 0 && mb_x < width.saturating_sub(1);

        // mbAddrD (above-left) available if not at left edge and not at top
        let avail_d = mb_y > 0 && mb_x > 0;

        self.mb_avail_a.store(avail_a as u64, Ordering::Release);
        self.mb_avail_b.store(avail_b as u64, Ordering::Release);
        self.mb_avail_c.store(avail_c as u64, Ordering::Release);
        self.mb_avail_d.store(avail_d as u64, Ordering::Release);
    }

    /// Advance to next macroblock
    ///
    /// Returns `false` when reaching the end of the frame.
    pub fn advance_mb(&self) -> bool {
        let mb_x = self.mb_x.load(Ordering::Acquire);
        let mb_y = self.mb_y.load(Ordering::Acquire);
        let width = self.pic_width_in_mbs.load(Ordering::Acquire);
        let height = self.pic_height_in_mbs.load(Ordering::Acquire);

        if width == 0 || height == 0 {
            return false;
        }

        let new_x = mb_x + 1;

        if new_x >= width {
            // Move to next row
            let new_y = mb_y + 1;
            if new_y >= height {
                // End of frame
                return false;
            }
            self.mb_x.store(0, Ordering::Release);
            self.mb_y.store(new_y, Ordering::Release);
            self.mb_addr.store(new_y as u64 * width as u64, Ordering::Release);
        } else {
            // Stay on same row
            self.mb_x.store(new_x, Ordering::Release);
            self.mb_addr.fetch_add(1, Ordering::AcqRel);
        }

        // Update neighbor availability for new position
        self.update_neighbor_availability();

        // Increment decoded count
        self.mbs_decoded.fetch_add(1, Ordering::Relaxed);

        true
    }

    /// Decode macroblock type for I slices
    ///
    /// ITU-T H.264 Section 9.3.3.1.1.1 - Binarization of mb_type
    pub fn decode_mb_type_i(
        &self,
        cabac: &CabacDecoderCapsule,
        data: &[u8],
        ctx_table: &CabacContextTable,
    ) -> Result<MbTypeI, MacroblockError> {
        // Check for I_PCM first using terminate context
        // Actually in CABAC, mb_type for I slices uses prefix/suffix binarization

        // Context for I slice mb_type (Table 9-11)
        let ctx_base = context_idx::MB_TYPE_I;

        // Decode prefix using truncated unary
        // First bin: ctx_base + ctxIdxInc (0 for no neighbor info)
        let ctx_inc = self.get_mb_type_ctx_inc();
        let bin0 = cabac.decode_decision(data, ctx_table, ctx_base + ctx_inc)?;

        if bin0 == 0 {
            // I_NxN (mb_type = 0)
            self.intra_mbs.fetch_add(1, Ordering::Relaxed);
            return Ok(MbTypeI::INxN);
        }

        // Check for terminating (I_PCM)
        let bin1 = cabac.decode_terminate(data)?;
        if bin1 {
            self.pcm_mbs.fetch_add(1, Ordering::Relaxed);
            return Ok(MbTypeI::IPCM);
        }

        // Decode remaining bins for I_16x16 types (1-24)
        // Binarization: 1, then 2 bins for pred_mode (FL), 2 bins for cbp_chroma (TU), 1 bin for cbp_luma

        // Decode Intra16x16PredMode (2 bins, fixed length)
        let pred_mode_bin0 = cabac.decode_decision(data, ctx_table, ctx_base + 1)?;
        let pred_mode_bin1 = cabac.decode_decision(data, ctx_table, ctx_base + 2)?;
        let pred_mode = (pred_mode_bin1 << 1) | pred_mode_bin0;

        // Decode CodedBlockPatternChroma (truncated unary, max=2)
        let cbp_chroma = cabac.decode_truncated_unary(data, ctx_table, ctx_base + 3, 2)?;

        // Decode CodedBlockPatternLuma flag
        let cbp_luma_flag = cabac.decode_decision(data, ctx_table, ctx_base + 4)?;
        let cbp_luma = if cbp_luma_flag == 1 { 15u8 } else { 0u8 };

        // Calculate mb_type value
        // mb_type = 1 + pred_mode + 4*cbp_chroma + (cbp_luma ? 12 : 0)
        let mb_type_val =
            1 + pred_mode as u8 + 4 * cbp_chroma as u8 + if cbp_luma > 0 { 12 } else { 0 };

        self.intra_mbs.fetch_add(1, Ordering::Relaxed);
        Ok(MbTypeI::from(mb_type_val))
    }

    /// Decode macroblock type for P slices
    ///
    /// ITU-T H.264 Section 9.3.3.1.1.2 - Binarization of mb_type for P/SP slices
    pub fn decode_mb_type_p(
        &self,
        cabac: &CabacDecoderCapsule,
        data: &[u8],
        ctx_table: &CabacContextTable,
    ) -> Result<MbTypeP, MacroblockError> {
        let ctx_base = context_idx::MB_TYPE_P_SP;

        // First bin
        let bin0 = cabac.decode_decision(data, ctx_table, ctx_base)?;

        if bin0 == 0 {
            // P_L0_16x16 (prefix = 0)
            self.inter_mbs.fetch_add(1, Ordering::Relaxed);
            return Ok(MbTypeP::PL0_16x16);
        }

        // Second bin
        let bin1 = cabac.decode_decision(data, ctx_table, ctx_base + 1)?;

        if bin1 == 0 {
            // Third bin determines 16x8 vs 8x16
            let bin2 = cabac.decode_decision(data, ctx_table, ctx_base + 2)?;
            self.inter_mbs.fetch_add(1, Ordering::Relaxed);
            if bin2 == 0 {
                return Ok(MbTypeP::PL0L0_16x8);
            } else {
                return Ok(MbTypeP::PL0L0_8x16);
            }
        }

        // P_8x8 or P_8x8ref0
        let bin2 = cabac.decode_decision(data, ctx_table, ctx_base + 2)?;
        self.inter_mbs.fetch_add(1, Ordering::Relaxed);

        if bin2 == 0 {
            Ok(MbTypeP::P8x8)
        } else {
            Ok(MbTypeP::P8x8ref0)
        }
    }

    /// Decode sub-macroblock type for P slices
    ///
    /// ITU-T H.264 Table 9-12
    pub fn decode_sub_mb_type_p(
        &self,
        cabac: &CabacDecoderCapsule,
        data: &[u8],
        ctx_table: &CabacContextTable,
    ) -> Result<SubMbTypeP, MacroblockError> {
        let ctx_base = context_idx::SUB_MB_TYPE_P_SP;

        // Truncated unary binarization
        let bin0 = cabac.decode_decision(data, ctx_table, ctx_base)?;

        if bin0 == 0 {
            return Ok(SubMbTypeP::PL0_8x8);
        }

        let bin1 = cabac.decode_decision(data, ctx_table, ctx_base + 1)?;

        if bin1 == 0 {
            return Ok(SubMbTypeP::PL0_8x4);
        }

        let bin2 = cabac.decode_decision(data, ctx_table, ctx_base + 2)?;

        if bin2 == 0 {
            Ok(SubMbTypeP::PL0_4x8)
        } else {
            Ok(SubMbTypeP::PL0_4x4)
        }
    }

    /// Decode intra 4x4 prediction modes
    ///
    /// ITU-T H.264 Section 9.3.3.1.1.3
    pub fn decode_intra_pred_modes(
        &self,
        cabac: &CabacDecoderCapsule,
        data: &[u8],
        ctx_table: &CabacContextTable,
        _mb_type: MbTypeI,
    ) -> Result<[u8; 16], MacroblockError> {
        let mut modes = [0u8; 16];

        // Only decode for I_NxN type
        for block_idx in 0..16 {
            // Decode prev_intra4x4_pred_mode_flag
            let prev_flag = cabac.decode_decision(
                data,
                ctx_table,
                context_idx::PREV_INTRA4X4_PRED_MODE_FLAG,
            )?;

            if prev_flag == 1 {
                // Use predicted mode (would come from neighbors in real decoder)
                // For now, use DC as default
                modes[block_idx] = Intra4x4PredMode::DC as u8;
            } else {
                // Decode rem_intra4x4_pred_mode (3 bins, fixed length)
                let rem = cabac.decode_fixed_length(data, 3)? as u8;
                modes[block_idx] = rem;
            }
        }

        Ok(modes)
    }

    /// Decode reference index
    ///
    /// ITU-T H.264 Section 9.3.3.1.1.6
    pub fn decode_ref_idx(
        &self,
        cabac: &CabacDecoderCapsule,
        data: &[u8],
        ctx_table: &CabacContextTable,
        list: u8,
        max_ref_idx: u8,
    ) -> Result<i8, MacroblockError> {
        if max_ref_idx == 0 {
            return Ok(0);
        }

        let ctx_base = if list == 0 {
            context_idx::REF_IDX_L0
        } else {
            context_idx::REF_IDX_L1
        };

        // Decode using truncated unary with context increment
        let ref_idx = cabac.decode_unary_ctx_inc(
            data,
            ctx_table,
            ctx_base,
            2, // Max context increment
            max_ref_idx as u32,
        )?;

        Ok(ref_idx as i8)
    }

    /// Decode motion vector difference
    ///
    /// ITU-T H.264 Section 9.3.3.1.1.7
    pub fn decode_mvd(
        &self,
        cabac: &CabacDecoderCapsule,
        data: &[u8],
        ctx_table: &CabacContextTable,
        list: u8,
        is_y: bool,
    ) -> Result<i16, MacroblockError> {
        let ctx_base = if list == 0 {
            context_idx::MVD_L0
        } else {
            context_idx::MVD_L1
        };

        // Context offset for x vs y component
        let ctx_offset = if is_y { 3 } else { 0 };

        // Decode abs_mvd using unary/exp-golomb hybrid
        // First 9 values use unary, then exp-golomb for larger values

        let abs_value = self.decode_mvd_abs(cabac, data, ctx_table, ctx_base + ctx_offset)?;

        if abs_value == 0 {
            return Ok(0);
        }

        // Decode sign
        let sign = cabac.decode_bypass(data)?;

        let value = if sign == 1 {
            -(abs_value as i16)
        } else {
            abs_value as i16
        };

        Ok(value)
    }

    /// Decode absolute motion vector difference value
    fn decode_mvd_abs(
        &self,
        cabac: &CabacDecoderCapsule,
        data: &[u8],
        ctx_table: &CabacContextTable,
        ctx_base: usize,
    ) -> Result<u16, MacroblockError> {
        // Decode prefix using unary with context increment (max 9)
        let prefix = cabac.decode_unary_ctx_inc(data, ctx_table, ctx_base, 4, 9)?;

        if prefix < 9 {
            return Ok(prefix as u16);
        }

        // Decode suffix using exp-golomb k=3
        let suffix = cabac.decode_exp_golomb(data, 3)?;

        Ok((9 + suffix) as u16)
    }

    /// Decode coded block pattern
    ///
    /// ITU-T H.264 Section 9.3.3.1.1.5
    pub fn decode_cbp(
        &self,
        cabac: &CabacDecoderCapsule,
        data: &[u8],
        ctx_table: &CabacContextTable,
        is_intra: bool,
    ) -> Result<(u8, u8), MacroblockError> {
        let _ = is_intra; // Used for context selection in full implementation

        // Decode CBP luma (4 bits for 8x8 blocks)
        let mut cbp_luma = 0u8;
        for i in 0..4 {
            let ctx = context_idx::CBP_LUMA + i;
            let bit = cabac.decode_decision(data, ctx_table, ctx)?;
            cbp_luma |= bit << i;
        }

        // Decode CBP chroma (0, 1, or 2)
        let cbp_chroma = cabac.decode_truncated_unary(data, ctx_table, context_idx::CBP_CHROMA, 2)?;

        Ok((cbp_luma, cbp_chroma as u8))
    }

    /// Decode a 4x4 residual block
    ///
    /// ITU-T H.264 Section 9.3.3.1.2
    pub fn decode_residual_block_4x4(
        &self,
        cabac: &CabacDecoderCapsule,
        data: &[u8],
        ctx_table: &CabacContextTable,
        block_idx: usize,
        max_coeff: usize,
    ) -> Result<[i16; 16], MacroblockError> {
        let mut coeffs = [0i16; 16];

        if block_idx > 25 {
            return Err(MacroblockError::OutOfBounds);
        }

        // Decode coded_block_flag
        let ctx_cbf = context_idx::CODED_BLOCK_FLAG_LUMA_AC + (block_idx & 3);
        let coded = cabac.decode_decision(data, ctx_table, ctx_cbf)?;

        if coded == 0 {
            return Ok(coeffs);
        }

        // Decode significant coefficient flags and levels
        let ctx_sig_base = context_idx::SIG_COEFF_FLAG_LUMA;
        let ctx_last_base = context_idx::LAST_SIG_COEFF_FLAG_LUMA;
        let ctx_level_base = context_idx::COEFF_ABS_LEVEL_MINUS1_LUMA;

        let max_num = max_coeff.min(16);
        let mut num_coeff = 0usize;
        let mut coeff_positions = [0usize; 16];

        // Scan for significant coefficients
        for scan_idx in 0..max_num {
            let ctx_sig = ctx_sig_base + scan_idx.min(14);
            let sig = cabac.decode_decision(data, ctx_table, ctx_sig)?;

            if sig == 1 {
                coeff_positions[num_coeff] = scan_idx;
                num_coeff += 1;

                // Check for last coefficient
                if scan_idx < max_num - 1 {
                    let ctx_last = ctx_last_base + scan_idx.min(14);
                    let last = cabac.decode_decision(data, ctx_table, ctx_last)?;
                    if last == 1 {
                        break;
                    }
                }
            }
        }

        // Decode coefficient levels in reverse order
        let mut num_t1 = 0usize;
        let mut num_large = 0usize;

        for i in (0..num_coeff).rev() {
            let pos = coeff_positions[i];

            // Context for coeff_abs_level_minus1
            let ctx_inc = if num_t1 < 1 {
                num_large.min(4)
            } else {
                5 + num_large.min(4)
            };
            let ctx = ctx_level_base + ctx_inc;

            // Decode level using unary prefix + exp-golomb suffix
            let level_minus1 = self.decode_coeff_abs_level(cabac, data, ctx_table, ctx)?;

            // Decode sign
            let sign = cabac.decode_bypass(data)?;

            let level = (level_minus1 + 1) as i16;
            coeffs[pos] = if sign == 1 { -level } else { level };

            // Update counters for context selection
            if level_minus1 == 0 {
                num_t1 += 1;
            } else {
                num_large += 1;
            }
        }

        Ok(coeffs)
    }

    /// Decode coefficient absolute level
    fn decode_coeff_abs_level(
        &self,
        cabac: &CabacDecoderCapsule,
        data: &[u8],
        ctx_table: &CabacContextTable,
        ctx_base: usize,
    ) -> Result<u16, MacroblockError> {
        // Decode prefix using unary
        let prefix = cabac.decode_unary(data, ctx_table, ctx_base, 14)?;

        if prefix < 14 {
            return Ok(prefix as u16);
        }

        // Decode suffix using exp-golomb k=0
        let suffix = cabac.decode_exp_golomb(data, 0)?;

        Ok((14 + suffix) as u16)
    }

    /// Get context increment for mb_type based on neighbor availability
    fn get_mb_type_ctx_inc(&self) -> usize {
        let avail_a = self.mb_avail_a.load(Ordering::Acquire) != 0;
        let avail_b = self.mb_avail_b.load(Ordering::Acquire) != 0;

        // Context increment based on neighbors (simplified)
        // In full implementation, would check neighbor mb_types
        match (avail_a, avail_b) {
            (false, false) => 0,
            (true, false) | (false, true) => 1,
            (true, true) => 2,
        }
    }

    /// Get statistics snapshot
    pub fn stats(&self) -> MacroblockStats {
        MacroblockStats {
            mbs_decoded: self.mbs_decoded.load(Ordering::Acquire),
            intra_mbs: self.intra_mbs.load(Ordering::Acquire),
            inter_mbs: self.inter_mbs.load(Ordering::Acquire),
            skip_mbs: self.skip_mbs.load(Ordering::Acquire),
            pcm_mbs: self.pcm_mbs.load(Ordering::Acquire),
            current_mb_x: self.mb_x.load(Ordering::Acquire),
            current_mb_y: self.mb_y.load(Ordering::Acquire),
            generation: self.generation.load(Ordering::Acquire),
        }
    }

    /// Get current macroblock position
    #[inline]
    pub fn position(&self) -> (u32, u32) {
        (
            self.mb_x.load(Ordering::Acquire),
            self.mb_y.load(Ordering::Acquire),
        )
    }

    /// Get current macroblock address
    #[inline]
    pub fn mb_address(&self) -> u64 {
        self.mb_addr.load(Ordering::Acquire)
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Check if left neighbor (A) is available
    #[inline]
    pub fn is_left_available(&self) -> bool {
        self.mb_avail_a.load(Ordering::Acquire) != 0
    }

    /// Check if above neighbor (B) is available
    #[inline]
    pub fn is_above_available(&self) -> bool {
        self.mb_avail_b.load(Ordering::Acquire) != 0
    }

    /// Check if above-right neighbor (C) is available
    #[inline]
    pub fn is_above_right_available(&self) -> bool {
        self.mb_avail_c.load(Ordering::Acquire) != 0
    }

    /// Check if above-left neighbor (D) is available
    #[inline]
    pub fn is_above_left_available(&self) -> bool {
        self.mb_avail_d.load(Ordering::Acquire) != 0
    }

    /// Reset to initial state
    pub fn reset(&self) {
        self.mb_x.store(0, Ordering::Release);
        self.mb_y.store(0, Ordering::Release);
        self.mb_addr.store(0, Ordering::Release);
        self.mbs_decoded.store(0, Ordering::Release);
        self.intra_mbs.store(0, Ordering::Release);
        self.inter_mbs.store(0, Ordering::Release);
        self.skip_mbs.store(0, Ordering::Release);
        self.pcm_mbs.store(0, Ordering::Release);
        self.prev_mb_skip_flag.store(0, Ordering::Release);
        self.prev_mb_type.store(0, Ordering::Release);
        self.update_neighbor_availability();
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
}

// ============================================================================
// Compile-Time Verification
// ============================================================================

const _: () = {
    // Verify H264MacroblockCapsule is exactly 512 bytes
    assert!(core::mem::size_of::<H264MacroblockCapsule>() == 512);
    // Verify 512-byte alignment
    assert!(core::mem::align_of::<H264MacroblockCapsule>() == 512);
    // Verify MbTypeI fits in u8
    assert!(core::mem::size_of::<MbTypeI>() == 1);
    // Verify MbTypeP fits in u8
    assert!(core::mem::size_of::<MbTypeP>() == 1);
    // Verify SubMbTypeP fits in u8
    assert!(core::mem::size_of::<SubMbTypeP>() == 1);
};

// ============================================================================
// Tests (T28 Compliance)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Q1: test_new_capsule
    #[test]
    fn test_new_capsule() {
        let capsule = H264MacroblockCapsule::new();

        assert_eq!(capsule.mb_x.load(Ordering::Relaxed), 0);
        assert_eq!(capsule.mb_y.load(Ordering::Relaxed), 0);
        assert_eq!(capsule.mb_addr.load(Ordering::Relaxed), 0);
        assert_eq!(capsule.generation.load(Ordering::Relaxed), 0);
        assert_eq!(capsule.mbs_decoded.load(Ordering::Relaxed), 0);

        // Verify size and alignment
        assert_eq!(core::mem::size_of::<H264MacroblockCapsule>(), 512);
        assert_eq!(core::mem::align_of::<H264MacroblockCapsule>(), 512);
    }

    // Q2: test_frame_dimensions
    #[test]
    fn test_frame_dimensions() {
        let capsule = H264MacroblockCapsule::new();

        // Set 1920x1080 (120x68 macroblocks)
        capsule.set_frame_dimensions(120, 68);

        assert_eq!(capsule.pic_width_in_mbs.load(Ordering::Relaxed), 120);
        assert_eq!(capsule.pic_height_in_mbs.load(Ordering::Relaxed), 68);
        assert_eq!(capsule.mb_x.load(Ordering::Relaxed), 0);
        assert_eq!(capsule.mb_y.load(Ordering::Relaxed), 0);
        assert!(capsule.generation.load(Ordering::Relaxed) > 0);
    }

    // Q3: test_neighbor_availability
    #[test]
    fn test_neighbor_availability() {
        let capsule = H264MacroblockCapsule::new();
        capsule.set_frame_dimensions(10, 10);

        // At (0, 0) - no neighbors
        assert!(!capsule.is_left_available());
        assert!(!capsule.is_above_available());
        assert!(!capsule.is_above_right_available());
        assert!(!capsule.is_above_left_available());

        // Move to (1, 0) - only left available
        capsule.advance_mb();
        assert!(capsule.is_left_available());
        assert!(!capsule.is_above_available());

        // Move to (0, 1) - only above available
        capsule.mb_x.store(0, Ordering::Release);
        capsule.mb_y.store(1, Ordering::Release);
        capsule.update_neighbor_availability();
        assert!(!capsule.is_left_available());
        assert!(capsule.is_above_available());

        // Move to (1, 1) - all except above-right at edge check
        capsule.mb_x.store(1, Ordering::Release);
        capsule.mb_y.store(1, Ordering::Release);
        capsule.update_neighbor_availability();
        assert!(capsule.is_left_available());
        assert!(capsule.is_above_available());
        assert!(capsule.is_above_right_available());
        assert!(capsule.is_above_left_available());
    }

    // Q4: test_advance_mb
    #[test]
    fn test_advance_mb() {
        let capsule = H264MacroblockCapsule::new();
        capsule.set_frame_dimensions(4, 3); // 4x3 = 12 macroblocks

        // Start at (0, 0)
        assert_eq!(capsule.position(), (0, 0));

        // Advance through first row
        assert!(capsule.advance_mb()); // (1, 0)
        assert_eq!(capsule.position(), (1, 0));

        assert!(capsule.advance_mb()); // (2, 0)
        assert!(capsule.advance_mb()); // (3, 0)
        assert_eq!(capsule.position(), (3, 0));

        // Wrap to second row
        assert!(capsule.advance_mb()); // (0, 1)
        assert_eq!(capsule.position(), (0, 1));

        // Continue advancing
        for _ in 0..7 {
            assert!(capsule.advance_mb());
        }

        // Should be at (3, 2) now
        assert_eq!(capsule.position(), (3, 2));

        // Next advance should return false (end of frame)
        assert!(!capsule.advance_mb());
    }

    // Q5: test_mb_type_enums
    #[test]
    fn test_mb_type_enums() {
        // Test MbTypeI conversions
        assert_eq!(MbTypeI::from(0), MbTypeI::INxN);
        assert_eq!(MbTypeI::from(1), MbTypeI::I16x16_0_0_0);
        assert_eq!(MbTypeI::from(25), MbTypeI::IPCM);

        // Test I_16x16 properties
        assert!(MbTypeI::I16x16_0_0_0.is_intra_16x16());
        assert!(!MbTypeI::INxN.is_intra_16x16());
        assert!(MbTypeI::IPCM.is_pcm());

        // Test prediction mode extraction
        assert_eq!(MbTypeI::I16x16_0_0_0.intra_16x16_pred_mode(), Some(0));
        assert_eq!(MbTypeI::I16x16_1_0_0.intra_16x16_pred_mode(), Some(1));
        assert_eq!(MbTypeI::I16x16_2_0_0.intra_16x16_pred_mode(), Some(2));
        assert_eq!(MbTypeI::I16x16_3_0_0.intra_16x16_pred_mode(), Some(3));

        // Test CBP extraction
        assert_eq!(MbTypeI::I16x16_0_0_0.cbp_luma(), Some(0));
        assert_eq!(MbTypeI::I16x16_0_0_1.cbp_luma(), Some(15));
        assert_eq!(MbTypeI::I16x16_0_0_0.cbp_chroma(), Some(0));
        assert_eq!(MbTypeI::I16x16_0_1_0.cbp_chroma(), Some(1));
        assert_eq!(MbTypeI::I16x16_0_2_0.cbp_chroma(), Some(2));

        // Test MbTypeP
        assert_eq!(MbTypeP::from(0), MbTypeP::PL0_16x16);
        assert_eq!(MbTypeP::PL0_16x16.num_partitions(), 1);
        assert_eq!(MbTypeP::P8x8.num_partitions(), 4);
        assert!(MbTypeP::PSkip.is_skip());

        // Test SubMbTypeP
        assert_eq!(SubMbTypeP::PL0_8x8.num_sub_partitions(), 1);
        assert_eq!(SubMbTypeP::PL0_4x4.num_sub_partitions(), 4);
        assert_eq!(SubMbTypeP::PL0_8x8.sub_width(), 8);
        assert_eq!(SubMbTypeP::PL0_4x4.sub_width(), 4);
    }

    // Q6: test_pred_mode_enums
    #[test]
    fn test_pred_mode_enums() {
        // Test Intra4x4PredMode
        assert_eq!(Intra4x4PredMode::from(0), Intra4x4PredMode::Vertical);
        assert_eq!(Intra4x4PredMode::from(1), Intra4x4PredMode::Horizontal);
        assert_eq!(Intra4x4PredMode::from(2), Intra4x4PredMode::DC);
        assert_eq!(
            Intra4x4PredMode::from(8),
            Intra4x4PredMode::HorizontalUp
        );

        // Test Intra16x16PredMode
        assert_eq!(Intra16x16PredMode::from(0), Intra16x16PredMode::Vertical);
        assert_eq!(Intra16x16PredMode::from(3), Intra16x16PredMode::Plane);

        // Test IntraChromaPredMode
        assert_eq!(IntraChromaPredMode::from(0), IntraChromaPredMode::DC);
        assert_eq!(IntraChromaPredMode::from(1), IntraChromaPredMode::Horizontal);
        assert_eq!(IntraChromaPredMode::from(3), IntraChromaPredMode::Plane);
    }

    // Q7: test_statistics
    #[test]
    fn test_statistics() {
        let capsule = H264MacroblockCapsule::new();
        capsule.set_frame_dimensions(10, 10);

        // Initial stats
        let stats = capsule.stats();
        assert_eq!(stats.mbs_decoded, 0);
        assert_eq!(stats.intra_mbs, 0);

        // Simulate some decoding
        capsule.intra_mbs.store(5, Ordering::Release);
        capsule.inter_mbs.store(10, Ordering::Release);
        capsule.skip_mbs.store(3, Ordering::Release);

        // Advance a few MBs
        for _ in 0..5 {
            capsule.advance_mb();
        }

        let stats = capsule.stats();
        assert_eq!(stats.mbs_decoded, 5);
        assert_eq!(stats.intra_mbs, 5);
        assert_eq!(stats.inter_mbs, 10);
        assert_eq!(stats.skip_mbs, 3);
        assert!(stats.generation > 0);
    }

    // Q8: test_slice_info
    #[test]
    fn test_slice_info() {
        let capsule = H264MacroblockCapsule::new();

        // Set I slice with QP=30
        capsule.set_slice_info(SliceType::I as u8, 30);

        assert_eq!(capsule.slice_type.load(Ordering::Relaxed), SliceType::I as u32);
        assert_eq!(capsule.slice_qp.load(Ordering::Relaxed), 30);

        // Set P slice with QP=26
        capsule.set_slice_info(SliceType::P as u8, 26);

        assert_eq!(capsule.slice_type.load(Ordering::Relaxed), SliceType::P as u32);
        assert_eq!(capsule.slice_qp.load(Ordering::Relaxed), 26);
    }

    // Additional: test_macroblock_data_default
    #[test]
    fn test_macroblock_data_default() {
        let data = MacroblockData::default();

        assert_eq!(data.mb_type, 0);
        assert_eq!(data.mb_x, 0);
        assert_eq!(data.mb_y, 0);
        assert_eq!(data.qp_y, 26);
        assert_eq!(data.ref_idx_l0, [-1; 4]);
        assert_eq!(data.cbp_luma, 0);
        assert_eq!(data.cbp_chroma, 0);
    }

    // Additional: test_reset
    #[test]
    fn test_reset() {
        let capsule = H264MacroblockCapsule::new();
        capsule.set_frame_dimensions(10, 10);

        // Advance some
        for _ in 0..5 {
            capsule.advance_mb();
        }

        let gen_before = capsule.generation();
        capsule.reset();

        assert_eq!(capsule.position(), (0, 0));
        assert_eq!(capsule.stats().mbs_decoded, 0);
        assert!(capsule.generation() > gen_before);
    }

    // Additional: test_edge_cases
    #[test]
    fn test_edge_cases() {
        let capsule = H264MacroblockCapsule::new();

        // Zero dimensions - advance should return false
        capsule.set_frame_dimensions(0, 0);
        assert!(!capsule.advance_mb());

        // 1x1 frame
        capsule.set_frame_dimensions(1, 1);
        assert!(!capsule.advance_mb()); // Already at (0,0), can't advance

        // Reset and try normal operation
        capsule.reset();
        capsule.set_frame_dimensions(2, 2);
        assert!(capsule.advance_mb()); // (0,0) -> (1,0)
        assert!(capsule.advance_mb()); // (1,0) -> (0,1)
        assert!(capsule.advance_mb()); // (0,1) -> (1,1)
        assert!(!capsule.advance_mb()); // End of frame
    }

    // Additional: test_mb_type_i_all_variants
    #[test]
    fn test_mb_type_i_all_variants() {
        // Test all 26 I slice mb_type values round-trip correctly
        for val in 0..=25 {
            let mb_type = MbTypeI::from(val);
            match mb_type {
                MbTypeI::INxN => assert_eq!(val, 0),
                MbTypeI::IPCM => assert_eq!(val, 25),
                _ => {
                    // All I_16x16 variants
                    assert!(val >= 1 && val <= 24);
                    assert!(mb_type.is_intra_16x16());
                }
            }
        }
    }
}
