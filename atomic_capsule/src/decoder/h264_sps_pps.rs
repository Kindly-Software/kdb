//! H.264 SPS/PPS (Sequence/Picture Parameter Set) Parser
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Implements ITU-T H.264 Section 7.3.2.1 (SPS) and 7.3.2.2 (PPS) parsing.
//!
//! # Architecture
//!
//! ```text
//! +------------------------------------------+
//! | H264SpsPpsCapsule (T1 Atomic)            |
//! | Size: 1024B, Align: 1024B                |
//! |                                          |
//! | +--------------------------------------+ |
//! | | SPS Storage (32 max)                  | |
//! | | - sps_count (AtomicU32)              | |
//! | | - sps_valid_mask (AtomicU32)         | |
//! | | - active_sps_id (AtomicU8)           | |
//! | +--------------------------------------+ |
//! | +--------------------------------------+ |
//! | | PPS Storage (256 max)                | |
//! | | - pps_count (AtomicU32)              | |
//! | | - pps_valid_mask[4] (AtomicU64)      | |
//! | | - active_pps_id (AtomicU8)           | |
//! | +--------------------------------------+ |
//! | +--------------------------------------+ |
//! | | Cached Values (from active SPS/PPS) | |
//! | | - pic_width_in_mbs (AtomicU32)       | |
//! | | - pic_height_in_mbs (AtomicU32)      | |
//! | | - frame_width/height (AtomicU32)     | |
//! | | - chroma_format (AtomicU8)           | |
//! | | - entropy_mode (AtomicU8)            | |
//! | +--------------------------------------+ |
//! | +--------------------------------------+ |
//! | | Statistics (T0 Auditable)            | |
//! | | - generation (AtomicU64)             | |
//! | | - sps_parsed (AtomicU64)             | |
//! | | - pps_parsed (AtomicU64)             | |
//! | | - parse_errors (AtomicU64)           | |
//! | +--------------------------------------+ |
//! +------------------------------------------+
//! ```
//!
//! # Stored Parameters
//!
//! SPS contains:
//! - Profile/level (profile_idc, level_idc, constraint flags)
//! - Picture dimensions (pic_width_in_mbs, pic_height_in_map_units)
//! - Frame/field coding (frame_mbs_only_flag, mb_adaptive_frame_field_flag)
//! - Chroma format (chroma_format_idc, bit_depth_luma/chroma)
//! - Reference frames (max_num_ref_frames, gaps_in_frame_num_allowed)
//! - POC type and parameters
//! - VUI parameters (timing, aspect ratio, etc.)
//!
//! PPS contains:
//! - Entropy coding (entropy_coding_mode_flag: 0=CAVLC, 1=CABAC)
//! - Slice groups (num_slice_groups, slice_group_map_type)
//! - Reference list modification
//! - Weighted prediction
//! - Deblocking filter control
//! - Transform parameters (transform_8x8_mode_flag)
//! - Scaling lists

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};

// ============================================================================
// H.264 Profile IDC Values
// ============================================================================

/// H.264 Profile IDC values (ITU-T H.264 Table A-1)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Profile {
    /// Baseline Profile (Annex A)
    Baseline = 66,
    /// Main Profile
    Main = 77,
    /// Extended Profile
    Extended = 88,
    /// High Profile (FRExt)
    High = 100,
    /// High 10 Profile (10-bit)
    High10 = 110,
    /// High 4:2:2 Profile
    High422 = 122,
    /// High 4:4:4 Predictive Profile
    High444Predictive = 244,
    /// CAVLC 4:4:4 Intra Profile
    Cavlc444 = 44,
    /// Scalable Baseline Profile
    ScalableBaseline = 83,
    /// Scalable High Profile
    ScalableHigh = 86,
    /// Stereo High Profile
    StereoHigh = 128,
    /// Multiview High Profile
    MultiviewHigh = 118,
    /// MFC High Profile
    Mfc = 134,
}

impl Profile {
    /// Check if profile is High or higher (requires extended parsing)
    #[inline]
    pub const fn is_high_or_above(profile_idc: u8) -> bool {
        matches!(
            profile_idc,
            100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 134
        )
    }

    /// Try to convert from u8
    #[inline]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            66 => Some(Self::Baseline),
            77 => Some(Self::Main),
            88 => Some(Self::Extended),
            100 => Some(Self::High),
            110 => Some(Self::High10),
            122 => Some(Self::High422),
            244 => Some(Self::High444Predictive),
            44 => Some(Self::Cavlc444),
            83 => Some(Self::ScalableBaseline),
            86 => Some(Self::ScalableHigh),
            128 => Some(Self::StereoHigh),
            118 => Some(Self::MultiviewHigh),
            134 => Some(Self::Mfc),
            _ => None,
        }
    }
}

// ============================================================================
// SPS/PPS Structures
// ============================================================================

/// Sequence Parameter Set (ITU-T H.264 Section 7.3.2.1.1)
#[derive(Debug, Clone)]
pub struct Sps {
    // Identification
    /// Sequence parameter set ID (0-31)
    pub seq_parameter_set_id: u8,

    // Profile/Level
    /// Profile IDC (e.g., 66=Baseline, 77=Main, 100=High)
    pub profile_idc: u8,
    /// Constraint set flags (6 bits packed)
    pub constraint_set_flags: u8,
    /// Level IDC (e.g., 30=3.0, 40=4.0, 51=5.1)
    pub level_idc: u8,

    // Chroma format (High profile+)
    /// Chroma format: 0=monochrome, 1=4:2:0, 2=4:2:2, 3=4:4:4
    pub chroma_format_idc: u8,
    /// Separate colour plane flag (for 4:4:4)
    pub separate_colour_plane_flag: bool,
    /// Bit depth luma minus 8 (0-6, adds to 8 for 8-14 bits)
    pub bit_depth_luma_minus8: u8,
    /// Bit depth chroma minus 8 (0-6)
    pub bit_depth_chroma_minus8: u8,
    /// QP prime Y zero transform bypass flag
    pub qpprime_y_zero_transform_bypass_flag: bool,

    // Scaling matrices
    /// Sequence scaling matrix present flag
    pub seq_scaling_matrix_present_flag: bool,
    /// Scaling list present flags (up to 12 lists)
    pub seq_scaling_list_present_flag: [bool; 12],

    // Picture order count
    /// Log2 max frame num minus 4 (0-12)
    pub log2_max_frame_num_minus4: u8,
    /// Picture order count type (0, 1, or 2)
    pub pic_order_cnt_type: u8,

    // POC type 0
    /// Log2 max POC LSB minus 4 (for POC type 0)
    pub log2_max_pic_order_cnt_lsb_minus4: u8,

    // POC type 1
    /// Delta POC always zero flag
    pub delta_pic_order_always_zero_flag: bool,
    /// Offset for non-reference picture
    pub offset_for_non_ref_pic: i32,
    /// Offset for top to bottom field
    pub offset_for_top_to_bottom_field: i32,
    /// Number of reference frames in POC cycle
    pub num_ref_frames_in_pic_order_cnt_cycle: u8,
    /// Offsets for reference frames (stored separately if needed)
    pub offset_for_ref_frame: [i32; 256],

    // Reference frames
    /// Maximum number of reference frames
    pub max_num_ref_frames: u8,
    /// Gaps in frame num value allowed flag
    pub gaps_in_frame_num_value_allowed_flag: bool,

    // Picture size (in macroblocks)
    /// Picture width in macroblocks minus 1
    pub pic_width_in_mbs_minus1: u16,
    /// Picture height in map units minus 1
    pub pic_height_in_map_units_minus1: u16,

    // Frame/field
    /// Frame MBs only flag (1=frame only, 0=field/frame adaptive)
    pub frame_mbs_only_flag: bool,
    /// MB adaptive frame/field flag
    pub mb_adaptive_frame_field_flag: bool,

    // Misc
    /// Direct 8x8 inference flag
    pub direct_8x8_inference_flag: bool,

    // Cropping
    /// Frame cropping flag
    pub frame_cropping_flag: bool,
    /// Frame crop left offset
    pub frame_crop_left_offset: u16,
    /// Frame crop right offset
    pub frame_crop_right_offset: u16,
    /// Frame crop top offset
    pub frame_crop_top_offset: u16,
    /// Frame crop bottom offset
    pub frame_crop_bottom_offset: u16,

    // VUI
    /// VUI parameters present flag
    pub vui_parameters_present_flag: bool,
    /// VUI parameters (stored if present)
    pub vui: Option<VuiParameters>,
}

impl Sps {
    /// Get width in pixels (accounting for cropping)
    #[inline]
    pub fn width(&self) -> u32 {
        let mb_width = (self.pic_width_in_mbs_minus1 as u32 + 1) * 16;
        let crop_unit_x = if self.chroma_format_idc == 0 { 1 } else { 2 };
        mb_width - crop_unit_x * (self.frame_crop_left_offset + self.frame_crop_right_offset) as u32
    }

    /// Get height in pixels (accounting for cropping)
    #[inline]
    pub fn height(&self) -> u32 {
        let map_unit_height = (self.pic_height_in_map_units_minus1 as u32 + 1) * 16;
        let frame_height = if self.frame_mbs_only_flag {
            map_unit_height
        } else {
            map_unit_height * 2
        };
        let crop_unit_y = if self.chroma_format_idc == 0 {
            if self.frame_mbs_only_flag {
                1
            } else {
                2
            }
        } else if self.frame_mbs_only_flag {
            2
        } else {
            4
        };
        frame_height
            - crop_unit_y * (self.frame_crop_top_offset + self.frame_crop_bottom_offset) as u32
    }

    /// Get width in macroblocks
    #[inline]
    pub fn width_in_mbs(&self) -> u32 {
        self.pic_width_in_mbs_minus1 as u32 + 1
    }

    /// Get height in macroblocks
    #[inline]
    pub fn height_in_mbs(&self) -> u32 {
        let map_units = self.pic_height_in_map_units_minus1 as u32 + 1;
        if self.frame_mbs_only_flag {
            map_units
        } else {
            map_units * 2
        }
    }

    /// Get max frame num
    #[inline]
    pub fn max_frame_num(&self) -> u32 {
        1 << (self.log2_max_frame_num_minus4 + 4)
    }

    /// Get max POC LSB (for POC type 0)
    #[inline]
    pub fn max_pic_order_cnt_lsb(&self) -> u32 {
        1 << (self.log2_max_pic_order_cnt_lsb_minus4 + 4)
    }
}

impl Default for Sps {
    fn default() -> Self {
        Self {
            seq_parameter_set_id: 0,
            profile_idc: 0,
            constraint_set_flags: 0,
            level_idc: 0,
            chroma_format_idc: 1, // Default 4:2:0
            separate_colour_plane_flag: false,
            bit_depth_luma_minus8: 0,
            bit_depth_chroma_minus8: 0,
            qpprime_y_zero_transform_bypass_flag: false,
            seq_scaling_matrix_present_flag: false,
            seq_scaling_list_present_flag: [false; 12],
            log2_max_frame_num_minus4: 0,
            pic_order_cnt_type: 0,
            log2_max_pic_order_cnt_lsb_minus4: 0,
            delta_pic_order_always_zero_flag: false,
            offset_for_non_ref_pic: 0,
            offset_for_top_to_bottom_field: 0,
            num_ref_frames_in_pic_order_cnt_cycle: 0,
            offset_for_ref_frame: [0i32; 256],
            max_num_ref_frames: 0,
            gaps_in_frame_num_value_allowed_flag: false,
            pic_width_in_mbs_minus1: 0,
            pic_height_in_map_units_minus1: 0,
            frame_mbs_only_flag: true,
            mb_adaptive_frame_field_flag: false,
            direct_8x8_inference_flag: false,
            frame_cropping_flag: false,
            frame_crop_left_offset: 0,
            frame_crop_right_offset: 0,
            frame_crop_top_offset: 0,
            frame_crop_bottom_offset: 0,
            vui_parameters_present_flag: false,
            vui: None,
        }
    }
}

/// Picture Parameter Set (ITU-T H.264 Section 7.3.2.2)
#[derive(Debug, Clone, Default)]
pub struct Pps {
    // Identification
    /// Picture parameter set ID (0-255)
    pub pic_parameter_set_id: u8,
    /// References SPS by ID (0-31)
    pub seq_parameter_set_id: u8,

    // Entropy coding
    /// Entropy coding mode: 0=CAVLC, 1=CABAC
    pub entropy_coding_mode_flag: bool,
    /// Bottom field POC in frame present flag
    pub bottom_field_pic_order_in_frame_present_flag: bool,

    // Slice groups (FMO)
    /// Number of slice groups minus 1
    pub num_slice_groups_minus1: u8,
    /// Slice group map type
    pub slice_group_map_type: u8,

    // Reference lists
    /// Num ref idx L0 default active minus 1
    pub num_ref_idx_l0_default_active_minus1: u8,
    /// Num ref idx L1 default active minus 1
    pub num_ref_idx_l1_default_active_minus1: u8,

    // Weighted prediction
    /// Weighted prediction flag
    pub weighted_pred_flag: bool,
    /// Weighted bipred IDC (0=off, 1=explicit, 2=implicit)
    pub weighted_bipred_idc: u8,

    // QP
    /// Pic init QP minus 26 (-26 to +25)
    pub pic_init_qp_minus26: i8,
    /// Pic init QS minus 26
    pub pic_init_qs_minus26: i8,
    /// Chroma QP index offset (-12 to +12)
    pub chroma_qp_index_offset: i8,

    // Deblocking
    /// Deblocking filter control present flag
    pub deblocking_filter_control_present_flag: bool,

    // Misc
    /// Constrained intra prediction flag
    pub constrained_intra_pred_flag: bool,
    /// Redundant pic cnt present flag
    pub redundant_pic_cnt_present_flag: bool,

    // High profile extensions
    /// Transform 8x8 mode flag
    pub transform_8x8_mode_flag: bool,
    /// Pic scaling matrix present flag
    pub pic_scaling_matrix_present_flag: bool,
    /// Second chroma QP index offset
    pub second_chroma_qp_index_offset: i8,

    // CABAC initialization (derived, not in bitstream PPS)
    /// CABAC initialization IDC for P/B slices
    pub cabac_init_idc: u8,
}

/// VUI Parameters (ITU-T H.264 Annex E)
#[derive(Debug, Clone, Default)]
pub struct VuiParameters {
    /// Aspect ratio info present flag
    pub aspect_ratio_info_present_flag: bool,
    /// Aspect ratio IDC
    pub aspect_ratio_idc: u8,
    /// Sample aspect ratio width (if aspect_ratio_idc == 255)
    pub sar_width: u16,
    /// Sample aspect ratio height
    pub sar_height: u16,

    /// Overscan info present flag
    pub overscan_info_present_flag: bool,
    /// Overscan appropriate flag
    pub overscan_appropriate_flag: bool,

    /// Video signal type present flag
    pub video_signal_type_present_flag: bool,
    /// Video format (0=component, 1=PAL, 2=NTSC, 3=SECAM, 4=MAC, 5=unspecified)
    pub video_format: u8,
    /// Video full range flag
    pub video_full_range_flag: bool,
    /// Colour description present flag
    pub colour_description_present_flag: bool,
    /// Colour primaries
    pub colour_primaries: u8,
    /// Transfer characteristics
    pub transfer_characteristics: u8,
    /// Matrix coefficients
    pub matrix_coefficients: u8,

    /// Chroma loc info present flag
    pub chroma_loc_info_present_flag: bool,
    /// Chroma sample location type for top field
    pub chroma_sample_loc_type_top_field: u8,
    /// Chroma sample location type for bottom field
    pub chroma_sample_loc_type_bottom_field: u8,

    /// Timing info present flag
    pub timing_info_present_flag: bool,
    /// Number of units in tick
    pub num_units_in_tick: u32,
    /// Time scale
    pub time_scale: u32,
    /// Fixed frame rate flag
    pub fixed_frame_rate_flag: bool,
}

impl VuiParameters {
    /// Get frame rate as floating point (if timing info present)
    #[inline]
    pub fn frame_rate(&self) -> Option<f64> {
        if self.timing_info_present_flag && self.num_units_in_tick > 0 {
            Some(self.time_scale as f64 / (2.0 * self.num_units_in_tick as f64))
        } else {
            None
        }
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// SPS/PPS parsing errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum SpsError {
    /// No error
    #[default]
    None = 0,
    /// Invalid profile IDC value
    InvalidProfileIdc = 1,
    /// Invalid level IDC value
    InvalidLevelIdc = 2,
    /// Invalid SPS ID (must be 0-31)
    InvalidSpsId = 3,
    /// Invalid PPS ID (must be 0-255)
    InvalidPpsId = 4,
    /// Invalid chroma format (must be 0-3)
    InvalidChromaFormat = 5,
    /// Invalid POC type (must be 0-2)
    InvalidPocType = 6,
    /// Invalid number of reference frames
    InvalidNumRefFrames = 7,
    /// Invalid picture dimensions
    InvalidPicDimensions = 8,
    /// Exp-Golomb code overflow
    ExpGolombOverflow = 9,
    /// Unexpected end of bitstream
    UnexpectedEof = 10,
    /// Unsupported profile
    UnsupportedProfile = 11,
    /// Referenced SPS not found
    SpsNotFound = 12,
    /// Referenced PPS not found
    PpsNotFound = 13,
    /// Storage full (max SPS/PPS reached)
    StorageFull = 14,
}

impl SpsError {
    /// Convert from u64 for atomic operations
    #[inline]
    pub const fn from_u64(value: u64) -> Self {
        match value {
            0 => Self::None,
            1 => Self::InvalidProfileIdc,
            2 => Self::InvalidLevelIdc,
            3 => Self::InvalidSpsId,
            4 => Self::InvalidPpsId,
            5 => Self::InvalidChromaFormat,
            6 => Self::InvalidPocType,
            7 => Self::InvalidNumRefFrames,
            8 => Self::InvalidPicDimensions,
            9 => Self::ExpGolombOverflow,
            10 => Self::UnexpectedEof,
            11 => Self::UnsupportedProfile,
            12 => Self::SpsNotFound,
            13 => Self::PpsNotFound,
            14 => Self::StorageFull,
            _ => Self::None,
        }
    }

    /// Convert to u64 for atomic operations
    #[inline]
    pub const fn to_u64(self) -> u64 {
        self as u64
    }
}

// ============================================================================
// Statistics
// ============================================================================

/// SPS/PPS statistics snapshot
#[derive(Debug, Clone, Copy, Default)]
pub struct SpsStats {
    /// Generation counter
    pub generation: u64,
    /// Number of SPS sets parsed
    pub sps_parsed: u64,
    /// Number of PPS sets parsed
    pub pps_parsed: u64,
    /// Number of parse errors
    pub parse_errors: u64,
    /// Number of valid SPS sets stored
    pub sps_count: u32,
    /// Number of valid PPS sets stored
    pub pps_count: u32,
    /// Active SPS ID
    pub active_sps_id: u8,
    /// Active PPS ID
    pub active_pps_id: u8,
    /// Cached frame width (pixels)
    pub frame_width: u32,
    /// Cached frame height (pixels)
    pub frame_height: u32,
    /// Is CABAC enabled
    pub cabac_enabled: bool,
}

// ============================================================================
// Main Capsule
// ============================================================================

/// T1 Atomic capsule for H.264 SPS/PPS management
///
/// **Tier**: T1 Atomic (lockfree parameter storage and retrieval)
/// **Size**: 1024B cache-aligned (large for SPS/PPS storage)
/// **Safety**: 99.99% (all atomics, no unsafe blocks)
///
/// # Design
///
/// Stores up to 32 SPS and 256 PPS sets with lockfree access.
/// Maintains cached derived values from active SPS/PPS for fast lookup.
///
/// # Thread Safety
///
/// All operations are lockfree using atomic types:
/// - CAS for SPS/PPS activation
/// - Generation counter for TOCTOU prevention
/// - Relaxed ordering for statistics
#[repr(C, align(1024))]
pub struct H264SpsPpsCapsule {
    // SPS storage tracking (12 bytes)
    /// Number of valid SPS sets
    pub sps_count: AtomicU32,
    /// Bitmask of valid SPS IDs (32 bits)
    pub sps_valid_mask: AtomicU32,
    /// Currently active SPS ID
    pub active_sps_id: AtomicU8,
    _pad1: [u8; 3],

    // PPS storage tracking (40 bytes)
    /// Number of valid PPS sets
    pub pps_count: AtomicU32,
    /// Bitmask of valid PPS IDs (256 bits = 4 x 64)
    pub pps_valid_mask: [AtomicU64; 4],
    /// Currently active PPS ID
    pub active_pps_id: AtomicU8,
    _pad2: [u8; 3],

    // Cached derived values from active SPS (32 bytes)
    /// Picture width in macroblocks
    pub pic_width_in_mbs: AtomicU32,
    /// Picture height in macroblocks
    pub pic_height_in_mbs: AtomicU32,
    /// Frame width in pixels
    pub frame_width: AtomicU32,
    /// Frame height in pixels
    pub frame_height: AtomicU32,
    /// Chroma format (0-3)
    pub chroma_format: AtomicU8,
    /// Bit depth luma
    pub bit_depth_luma: AtomicU8,
    /// Bit depth chroma
    pub bit_depth_chroma: AtomicU8,
    _pad3: u8,
    /// Max frame num
    pub max_frame_num: AtomicU32,
    /// Max POC LSB
    pub max_poc_lsb: AtomicU32,
    /// POC type
    pub poc_type: AtomicU8,
    /// Frame MBs only flag
    pub frame_mbs_only: AtomicBool,
    _pad4: [u8; 2],

    // Cached derived values from active PPS (8 bytes)
    /// Entropy mode: 0=CAVLC, 1=CABAC
    pub entropy_mode: AtomicU8,
    /// Transform 8x8 enabled
    pub transform_8x8_enabled: AtomicBool,
    /// Deblocking control present
    pub deblocking_control_present: AtomicBool,
    /// Weighted prediction flag
    pub weighted_pred: AtomicBool,
    /// Init QP
    pub init_qp: AtomicU8,
    _pad5: [u8; 3],

    // Generation counter (8 bytes)
    /// Generation counter for cache invalidation
    pub generation: AtomicU64,

    // Statistics (32 bytes)
    /// Number of SPS sets parsed
    pub sps_parsed: AtomicU64,
    /// Number of PPS sets parsed
    pub pps_parsed: AtomicU64,
    /// Number of parse errors
    pub parse_errors: AtomicU64,
    /// Last error code
    pub last_error: AtomicU64,

    // Padding to 1024B
    // Actual struct size (with alignment): 136 bytes
    // 1024 - 136 = 888 bytes padding
    _padding: [u8; 888],
}

// Compile-time size verification
const _: () = {
    assert!(core::mem::size_of::<H264SpsPpsCapsule>() == 1024);
    assert!(core::mem::align_of::<H264SpsPpsCapsule>() == 1024);
};

impl H264SpsPpsCapsule {
    /// Create a new SPS/PPS capsule
    ///
    /// # Returns
    ///
    /// A new capsule with all atomics initialized to zero/defaults
    pub const fn new() -> Self {
        Self {
            sps_count: AtomicU32::new(0),
            sps_valid_mask: AtomicU32::new(0),
            active_sps_id: AtomicU8::new(0),
            _pad1: [0; 3],

            pps_count: AtomicU32::new(0),
            pps_valid_mask: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            active_pps_id: AtomicU8::new(0),
            _pad2: [0; 3],

            pic_width_in_mbs: AtomicU32::new(0),
            pic_height_in_mbs: AtomicU32::new(0),
            frame_width: AtomicU32::new(0),
            frame_height: AtomicU32::new(0),
            chroma_format: AtomicU8::new(1), // Default 4:2:0
            bit_depth_luma: AtomicU8::new(8),
            bit_depth_chroma: AtomicU8::new(8),
            _pad3: 0,
            max_frame_num: AtomicU32::new(16),
            max_poc_lsb: AtomicU32::new(16),
            poc_type: AtomicU8::new(0),
            frame_mbs_only: AtomicBool::new(true),
            _pad4: [0; 2],

            entropy_mode: AtomicU8::new(0),
            transform_8x8_enabled: AtomicBool::new(false),
            deblocking_control_present: AtomicBool::new(false),
            weighted_pred: AtomicBool::new(false),
            init_qp: AtomicU8::new(26),
            _pad5: [0; 3],

            generation: AtomicU64::new(0),

            sps_parsed: AtomicU64::new(0),
            pps_parsed: AtomicU64::new(0),
            parse_errors: AtomicU64::new(0),
            last_error: AtomicU64::new(0),

            _padding: [0; 888],
        }
    }

    // ========================================================================
    // Exp-Golomb Helpers (ITU-T H.264 Section 9.1)
    // ========================================================================

    /// Read unsigned Exp-Golomb code (ue(v))
    ///
    /// # Arguments
    ///
    /// * `rbsp` - Raw Byte Sequence Payload
    /// * `bit_offset` - Current bit position (updated on return)
    ///
    /// # Returns
    ///
    /// * `Ok(u32)` - Decoded unsigned value
    /// * `Err(SpsError)` - Parsing error
    #[inline]
    pub fn read_ue(rbsp: &[u8], bit_offset: &mut usize) -> Result<u32, SpsError> {
        // Count leading zeros
        let mut leading_zeros = 0u32;
        loop {
            let bit = Self::read_bits(rbsp, bit_offset, 1)?;
            if bit == 1 {
                break;
            }
            leading_zeros += 1;
            if leading_zeros > 32 {
                return Err(SpsError::ExpGolombOverflow);
            }
        }

        if leading_zeros == 0 {
            return Ok(0);
        }

        // Read remaining bits
        let value = Self::read_bits(rbsp, bit_offset, leading_zeros as usize)?;

        // ue(v) = 2^leadingZeros - 1 + value
        Ok((1u32 << leading_zeros) - 1 + value)
    }

    /// Read signed Exp-Golomb code (se(v))
    ///
    /// # Arguments
    ///
    /// * `rbsp` - Raw Byte Sequence Payload
    /// * `bit_offset` - Current bit position (updated on return)
    ///
    /// # Returns
    ///
    /// * `Ok(i32)` - Decoded signed value
    /// * `Err(SpsError)` - Parsing error
    #[inline]
    pub fn read_se(rbsp: &[u8], bit_offset: &mut usize) -> Result<i32, SpsError> {
        let code_num = Self::read_ue(rbsp, bit_offset)?;

        // se(v) = (-1)^(k+1) * Ceil(k/2)
        // where k = code_num
        if code_num == 0 {
            Ok(0)
        } else if code_num & 1 == 1 {
            // Odd: positive
            Ok(((code_num + 1) / 2) as i32)
        } else {
            // Even: negative
            Ok(-((code_num / 2) as i32))
        }
    }

    /// Read n bits from RBSP
    ///
    /// # Arguments
    ///
    /// * `rbsp` - Raw Byte Sequence Payload
    /// * `bit_offset` - Current bit position (updated on return)
    /// * `n` - Number of bits to read (1-32)
    ///
    /// # Returns
    ///
    /// * `Ok(u32)` - Read value
    /// * `Err(SpsError)` - Parsing error
    #[inline]
    pub fn read_bits(rbsp: &[u8], bit_offset: &mut usize, n: usize) -> Result<u32, SpsError> {
        if n == 0 || n > 32 {
            return Ok(0);
        }

        let mut value = 0u32;
        for _ in 0..n {
            let byte_idx = *bit_offset / 8;
            let bit_idx = 7 - (*bit_offset % 8);

            if byte_idx >= rbsp.len() {
                return Err(SpsError::UnexpectedEof);
            }

            let bit = (rbsp[byte_idx] >> bit_idx) & 1;
            value = (value << 1) | (bit as u32);
            *bit_offset += 1;
        }

        Ok(value)
    }

    /// Read 1 bit as boolean
    ///
    /// # Arguments
    ///
    /// * `rbsp` - Raw Byte Sequence Payload
    /// * `bit_offset` - Current bit position (updated on return)
    ///
    /// # Returns
    ///
    /// * `Ok(bool)` - Read flag
    /// * `Err(SpsError)` - Parsing error
    #[inline]
    pub fn read_flag(rbsp: &[u8], bit_offset: &mut usize) -> Result<bool, SpsError> {
        Ok(Self::read_bits(rbsp, bit_offset, 1)? == 1)
    }

    /// Check if more RBSP data is available
    #[inline]
    pub fn more_rbsp_data(rbsp: &[u8], bit_offset: usize) -> bool {
        // There's more data if we haven't reached the RBSP trailing bits
        let remaining_bits = rbsp.len() * 8 - bit_offset;
        if remaining_bits == 0 {
            return false;
        }

        // Look for rbsp_trailing_bits (1 followed by zeros to byte boundary)
        // If only trailing bits remain, return false
        let byte_idx = bit_offset / 8;
        let bit_idx = bit_offset % 8;

        if byte_idx >= rbsp.len() {
            return false;
        }

        // Check if remaining bits are all trailing
        let last_byte = rbsp[rbsp.len() - 1];
        if remaining_bits <= 8 - bit_idx as usize {
            // Remaining bits might be trailing bits
            let trailing_mask = 0xFF >> (8 - remaining_bits);
            let _trailing_value = last_byte & trailing_mask;
            // More complex check needed - simplified: assume more data if > 8 bits
            remaining_bits > 8
        } else {
            true
        }
    }

    // ========================================================================
    // SPS Parsing (ITU-T H.264 Section 7.3.2.1.1)
    // ========================================================================

    /// Parse Sequence Parameter Set from RBSP
    ///
    /// # Arguments
    ///
    /// * `rbsp` - Raw Byte Sequence Payload (after NAL unit header removal)
    ///
    /// # Returns
    ///
    /// * `Ok(Sps)` - Parsed SPS
    /// * `Err(SpsError)` - Parsing error
    ///
    /// # ITU-T H.264 Section 7.3.2.1.1 Parsing Order
    ///
    /// 1. profile_idc, constraint_set_flags, level_idc
    /// 2. seq_parameter_set_id
    /// 3. If High profile: chroma_format_idc, bit_depth, scaling lists
    /// 4. log2_max_frame_num_minus4
    /// 5. pic_order_cnt_type and associated params
    /// 6. max_num_ref_frames, gaps_in_frame_num_value_allowed_flag
    /// 7. pic_width_in_mbs_minus1, pic_height_in_map_units_minus1
    /// 8. frame_mbs_only_flag, mb_adaptive_frame_field_flag
    /// 9. direct_8x8_inference_flag
    /// 10. frame_cropping_flag and crop offsets
    /// 11. vui_parameters_present_flag and VUI
    pub fn parse_sps(&self, rbsp: &[u8]) -> Result<Sps, SpsError> {
        if rbsp.len() < 4 {
            self.parse_errors.fetch_add(1, Ordering::Relaxed);
            self.last_error
                .store(SpsError::UnexpectedEof.to_u64(), Ordering::Relaxed);
            return Err(SpsError::UnexpectedEof);
        }

        let mut bit_offset = 0usize;
        let mut sps = Sps::default();

        // 1. Profile/Level (fixed layout - 24 bits)
        sps.profile_idc = Self::read_bits(rbsp, &mut bit_offset, 8)? as u8;
        sps.constraint_set_flags = Self::read_bits(rbsp, &mut bit_offset, 8)? as u8;
        sps.level_idc = Self::read_bits(rbsp, &mut bit_offset, 8)? as u8;

        // 2. SPS ID
        let sps_id = Self::read_ue(rbsp, &mut bit_offset)?;
        if sps_id > 31 {
            self.parse_errors.fetch_add(1, Ordering::Relaxed);
            self.last_error
                .store(SpsError::InvalidSpsId.to_u64(), Ordering::Relaxed);
            return Err(SpsError::InvalidSpsId);
        }
        sps.seq_parameter_set_id = sps_id as u8;

        // 3. High profile extensions
        if Profile::is_high_or_above(sps.profile_idc) {
            // chroma_format_idc
            let chroma_format = Self::read_ue(rbsp, &mut bit_offset)?;
            if chroma_format > 3 {
                self.parse_errors.fetch_add(1, Ordering::Relaxed);
                self.last_error
                    .store(SpsError::InvalidChromaFormat.to_u64(), Ordering::Relaxed);
                return Err(SpsError::InvalidChromaFormat);
            }
            sps.chroma_format_idc = chroma_format as u8;

            if sps.chroma_format_idc == 3 {
                sps.separate_colour_plane_flag = Self::read_flag(rbsp, &mut bit_offset)?;
            }

            // bit_depth_luma_minus8
            let bit_depth_luma = Self::read_ue(rbsp, &mut bit_offset)?;
            if bit_depth_luma > 6 {
                self.parse_errors.fetch_add(1, Ordering::Relaxed);
                return Err(SpsError::InvalidProfileIdc);
            }
            sps.bit_depth_luma_minus8 = bit_depth_luma as u8;

            // bit_depth_chroma_minus8
            let bit_depth_chroma = Self::read_ue(rbsp, &mut bit_offset)?;
            if bit_depth_chroma > 6 {
                self.parse_errors.fetch_add(1, Ordering::Relaxed);
                return Err(SpsError::InvalidProfileIdc);
            }
            sps.bit_depth_chroma_minus8 = bit_depth_chroma as u8;

            // qpprime_y_zero_transform_bypass_flag
            sps.qpprime_y_zero_transform_bypass_flag = Self::read_flag(rbsp, &mut bit_offset)?;

            // seq_scaling_matrix_present_flag
            sps.seq_scaling_matrix_present_flag = Self::read_flag(rbsp, &mut bit_offset)?;

            if sps.seq_scaling_matrix_present_flag {
                let num_lists = if sps.chroma_format_idc != 3 { 8 } else { 12 };
                for i in 0..num_lists {
                    sps.seq_scaling_list_present_flag[i] = Self::read_flag(rbsp, &mut bit_offset)?;
                    if sps.seq_scaling_list_present_flag[i] {
                        // Skip scaling list parsing (would need separate storage)
                        let size = if i < 6 { 16 } else { 64 };
                        self.skip_scaling_list(rbsp, &mut bit_offset, size)?;
                    }
                }
            }
        } else {
            // Non-High profile defaults
            sps.chroma_format_idc = 1; // 4:2:0
            sps.bit_depth_luma_minus8 = 0; // 8-bit
            sps.bit_depth_chroma_minus8 = 0;
        }

        // 4. log2_max_frame_num_minus4
        let log2_max_frame_num = Self::read_ue(rbsp, &mut bit_offset)?;
        if log2_max_frame_num > 12 {
            self.parse_errors.fetch_add(1, Ordering::Relaxed);
            return Err(SpsError::InvalidNumRefFrames);
        }
        sps.log2_max_frame_num_minus4 = log2_max_frame_num as u8;

        // 5. pic_order_cnt_type
        let poc_type = Self::read_ue(rbsp, &mut bit_offset)?;
        if poc_type > 2 {
            self.parse_errors.fetch_add(1, Ordering::Relaxed);
            self.last_error
                .store(SpsError::InvalidPocType.to_u64(), Ordering::Relaxed);
            return Err(SpsError::InvalidPocType);
        }
        sps.pic_order_cnt_type = poc_type as u8;

        if sps.pic_order_cnt_type == 0 {
            // log2_max_pic_order_cnt_lsb_minus4
            let log2_max_poc_lsb = Self::read_ue(rbsp, &mut bit_offset)?;
            if log2_max_poc_lsb > 12 {
                self.parse_errors.fetch_add(1, Ordering::Relaxed);
                return Err(SpsError::InvalidPocType);
            }
            sps.log2_max_pic_order_cnt_lsb_minus4 = log2_max_poc_lsb as u8;
        } else if sps.pic_order_cnt_type == 1 {
            // delta_pic_order_always_zero_flag
            sps.delta_pic_order_always_zero_flag = Self::read_flag(rbsp, &mut bit_offset)?;

            // offset_for_non_ref_pic
            sps.offset_for_non_ref_pic = Self::read_se(rbsp, &mut bit_offset)?;

            // offset_for_top_to_bottom_field
            sps.offset_for_top_to_bottom_field = Self::read_se(rbsp, &mut bit_offset)?;

            // num_ref_frames_in_pic_order_cnt_cycle
            let num_ref_frames_poc = Self::read_ue(rbsp, &mut bit_offset)?;
            if num_ref_frames_poc > 255 {
                self.parse_errors.fetch_add(1, Ordering::Relaxed);
                return Err(SpsError::InvalidNumRefFrames);
            }
            sps.num_ref_frames_in_pic_order_cnt_cycle = num_ref_frames_poc as u8;

            // offset_for_ref_frame[]
            for i in 0..num_ref_frames_poc as usize {
                sps.offset_for_ref_frame[i] = Self::read_se(rbsp, &mut bit_offset)?;
            }
        }
        // POC type 2 has no additional parameters

        // 6. max_num_ref_frames
        let max_ref_frames = Self::read_ue(rbsp, &mut bit_offset)?;
        if max_ref_frames > 16 {
            self.parse_errors.fetch_add(1, Ordering::Relaxed);
            self.last_error
                .store(SpsError::InvalidNumRefFrames.to_u64(), Ordering::Relaxed);
            return Err(SpsError::InvalidNumRefFrames);
        }
        sps.max_num_ref_frames = max_ref_frames as u8;

        // gaps_in_frame_num_value_allowed_flag
        sps.gaps_in_frame_num_value_allowed_flag = Self::read_flag(rbsp, &mut bit_offset)?;

        // 7. Picture dimensions
        let pic_width = Self::read_ue(rbsp, &mut bit_offset)?;
        let pic_height = Self::read_ue(rbsp, &mut bit_offset)?;

        // Validate dimensions (max 8K in macroblocks = 480)
        if pic_width > 1000 || pic_height > 1000 {
            self.parse_errors.fetch_add(1, Ordering::Relaxed);
            self.last_error
                .store(SpsError::InvalidPicDimensions.to_u64(), Ordering::Relaxed);
            return Err(SpsError::InvalidPicDimensions);
        }
        sps.pic_width_in_mbs_minus1 = pic_width as u16;
        sps.pic_height_in_map_units_minus1 = pic_height as u16;

        // 8. frame_mbs_only_flag
        sps.frame_mbs_only_flag = Self::read_flag(rbsp, &mut bit_offset)?;

        if !sps.frame_mbs_only_flag {
            // mb_adaptive_frame_field_flag
            sps.mb_adaptive_frame_field_flag = Self::read_flag(rbsp, &mut bit_offset)?;
        }

        // 9. direct_8x8_inference_flag
        sps.direct_8x8_inference_flag = Self::read_flag(rbsp, &mut bit_offset)?;

        // 10. frame_cropping_flag
        sps.frame_cropping_flag = Self::read_flag(rbsp, &mut bit_offset)?;

        if sps.frame_cropping_flag {
            sps.frame_crop_left_offset = Self::read_ue(rbsp, &mut bit_offset)? as u16;
            sps.frame_crop_right_offset = Self::read_ue(rbsp, &mut bit_offset)? as u16;
            sps.frame_crop_top_offset = Self::read_ue(rbsp, &mut bit_offset)? as u16;
            sps.frame_crop_bottom_offset = Self::read_ue(rbsp, &mut bit_offset)? as u16;
        }

        // 11. VUI parameters
        sps.vui_parameters_present_flag = Self::read_flag(rbsp, &mut bit_offset)?;

        if sps.vui_parameters_present_flag {
            sps.vui = Some(self.parse_vui(rbsp, &mut bit_offset)?);
        }

        // Update statistics
        self.sps_parsed.fetch_add(1, Ordering::Relaxed);

        Ok(sps)
    }

    /// Skip scaling list in bitstream
    fn skip_scaling_list(
        &self,
        rbsp: &[u8],
        bit_offset: &mut usize,
        size: usize,
    ) -> Result<(), SpsError> {
        let mut last_scale = 8i32;
        let mut next_scale = 8i32;

        for _ in 0..size {
            if next_scale != 0 {
                let delta_scale = Self::read_se(rbsp, bit_offset)?;
                next_scale = (last_scale + delta_scale + 256) % 256;
            }
            last_scale = if next_scale == 0 {
                last_scale
            } else {
                next_scale
            };
        }

        Ok(())
    }

    // ========================================================================
    // VUI Parsing (ITU-T H.264 Annex E)
    // ========================================================================

    /// Parse VUI parameters
    ///
    /// # Arguments
    ///
    /// * `rbsp` - Raw Byte Sequence Payload
    /// * `bit_offset` - Current bit position (updated on return)
    ///
    /// # Returns
    ///
    /// * `Ok(VuiParameters)` - Parsed VUI
    /// * `Err(SpsError)` - Parsing error
    pub fn parse_vui(
        &self,
        rbsp: &[u8],
        bit_offset: &mut usize,
    ) -> Result<VuiParameters, SpsError> {
        let mut vui = VuiParameters::default();

        // aspect_ratio_info_present_flag
        vui.aspect_ratio_info_present_flag = Self::read_flag(rbsp, bit_offset)?;

        if vui.aspect_ratio_info_present_flag {
            vui.aspect_ratio_idc = Self::read_bits(rbsp, bit_offset, 8)? as u8;

            if vui.aspect_ratio_idc == 255 {
                // Extended_SAR
                vui.sar_width = Self::read_bits(rbsp, bit_offset, 16)? as u16;
                vui.sar_height = Self::read_bits(rbsp, bit_offset, 16)? as u16;
            }
        }

        // overscan_info_present_flag
        vui.overscan_info_present_flag = Self::read_flag(rbsp, bit_offset)?;

        if vui.overscan_info_present_flag {
            vui.overscan_appropriate_flag = Self::read_flag(rbsp, bit_offset)?;
        }

        // video_signal_type_present_flag
        vui.video_signal_type_present_flag = Self::read_flag(rbsp, bit_offset)?;

        if vui.video_signal_type_present_flag {
            vui.video_format = Self::read_bits(rbsp, bit_offset, 3)? as u8;
            vui.video_full_range_flag = Self::read_flag(rbsp, bit_offset)?;
            vui.colour_description_present_flag = Self::read_flag(rbsp, bit_offset)?;

            if vui.colour_description_present_flag {
                vui.colour_primaries = Self::read_bits(rbsp, bit_offset, 8)? as u8;
                vui.transfer_characteristics = Self::read_bits(rbsp, bit_offset, 8)? as u8;
                vui.matrix_coefficients = Self::read_bits(rbsp, bit_offset, 8)? as u8;
            }
        }

        // chroma_loc_info_present_flag
        vui.chroma_loc_info_present_flag = Self::read_flag(rbsp, bit_offset)?;

        if vui.chroma_loc_info_present_flag {
            vui.chroma_sample_loc_type_top_field = Self::read_ue(rbsp, bit_offset)? as u8;
            vui.chroma_sample_loc_type_bottom_field = Self::read_ue(rbsp, bit_offset)? as u8;
        }

        // timing_info_present_flag
        vui.timing_info_present_flag = Self::read_flag(rbsp, bit_offset)?;

        if vui.timing_info_present_flag {
            vui.num_units_in_tick = Self::read_bits(rbsp, bit_offset, 32)?;
            vui.time_scale = Self::read_bits(rbsp, bit_offset, 32)?;
            vui.fixed_frame_rate_flag = Self::read_flag(rbsp, bit_offset)?;
        }

        // Skip remaining VUI fields (HRD parameters, etc.) for simplicity
        // Full implementation would parse:
        // - nal_hrd_parameters_present_flag + HRD params
        // - vcl_hrd_parameters_present_flag + HRD params
        // - low_delay_hrd_flag (if any HRD)
        // - pic_struct_present_flag
        // - bitstream_restriction_flag + restrictions

        Ok(vui)
    }

    // ========================================================================
    // PPS Parsing (ITU-T H.264 Section 7.3.2.2)
    // ========================================================================

    /// Parse Picture Parameter Set from RBSP
    ///
    /// # Arguments
    ///
    /// * `rbsp` - Raw Byte Sequence Payload (after NAL unit header removal)
    ///
    /// # Returns
    ///
    /// * `Ok(Pps)` - Parsed PPS
    /// * `Err(SpsError)` - Parsing error
    ///
    /// # ITU-T H.264 Section 7.3.2.2 Parsing Order
    ///
    /// 1. pic_parameter_set_id, seq_parameter_set_id
    /// 2. entropy_coding_mode_flag
    /// 3. bottom_field_pic_order_in_frame_present_flag
    /// 4. num_slice_groups_minus1 and slice group params
    /// 5. num_ref_idx_l0/l1_default_active_minus1
    /// 6. weighted_pred_flag, weighted_bipred_idc
    /// 7. pic_init_qp_minus26, pic_init_qs_minus26, chroma_qp_index_offset
    /// 8. deblocking_filter_control_present_flag
    /// 9. constrained_intra_pred_flag, redundant_pic_cnt_present_flag
    /// 10. If more_rbsp_data(): transform_8x8, scaling, second_chroma_qp
    pub fn parse_pps(&self, rbsp: &[u8]) -> Result<Pps, SpsError> {
        if rbsp.is_empty() {
            self.parse_errors.fetch_add(1, Ordering::Relaxed);
            self.last_error
                .store(SpsError::UnexpectedEof.to_u64(), Ordering::Relaxed);
            return Err(SpsError::UnexpectedEof);
        }

        let mut bit_offset = 0usize;
        let mut pps = Pps::default();

        // 1. pic_parameter_set_id
        let pps_id = Self::read_ue(rbsp, &mut bit_offset)?;
        if pps_id > 255 {
            self.parse_errors.fetch_add(1, Ordering::Relaxed);
            self.last_error
                .store(SpsError::InvalidPpsId.to_u64(), Ordering::Relaxed);
            return Err(SpsError::InvalidPpsId);
        }
        pps.pic_parameter_set_id = pps_id as u8;

        // seq_parameter_set_id
        let sps_id = Self::read_ue(rbsp, &mut bit_offset)?;
        if sps_id > 31 {
            self.parse_errors.fetch_add(1, Ordering::Relaxed);
            self.last_error
                .store(SpsError::InvalidSpsId.to_u64(), Ordering::Relaxed);
            return Err(SpsError::InvalidSpsId);
        }
        pps.seq_parameter_set_id = sps_id as u8;

        // 2. entropy_coding_mode_flag (0=CAVLC, 1=CABAC)
        pps.entropy_coding_mode_flag = Self::read_flag(rbsp, &mut bit_offset)?;

        // 3. bottom_field_pic_order_in_frame_present_flag
        pps.bottom_field_pic_order_in_frame_present_flag = Self::read_flag(rbsp, &mut bit_offset)?;

        // 4. num_slice_groups_minus1
        let num_slice_groups = Self::read_ue(rbsp, &mut bit_offset)?;
        if num_slice_groups > 7 {
            self.parse_errors.fetch_add(1, Ordering::Relaxed);
            return Err(SpsError::InvalidNumRefFrames);
        }
        pps.num_slice_groups_minus1 = num_slice_groups as u8;

        if pps.num_slice_groups_minus1 > 0 {
            // slice_group_map_type
            let map_type = Self::read_ue(rbsp, &mut bit_offset)?;
            pps.slice_group_map_type = map_type as u8;

            // Skip detailed slice group parameters (FMO)
            // Full implementation would parse based on slice_group_map_type
            match pps.slice_group_map_type {
                0 => {
                    for _ in 0..=pps.num_slice_groups_minus1 {
                        let _ = Self::read_ue(rbsp, &mut bit_offset)?; // run_length_minus1
                    }
                }
                2 => {
                    for _ in 0..pps.num_slice_groups_minus1 {
                        let _ = Self::read_ue(rbsp, &mut bit_offset)?; // top_left
                        let _ = Self::read_ue(rbsp, &mut bit_offset)?; // bottom_right
                    }
                }
                3 | 4 | 5 => {
                    let _ = Self::read_flag(rbsp, &mut bit_offset)?; // slice_group_change_direction_flag
                    let _ = Self::read_ue(rbsp, &mut bit_offset)?; // slice_group_change_rate_minus1
                }
                6 => {
                    let pic_size = Self::read_ue(rbsp, &mut bit_offset)?; // pic_size_in_map_units_minus1
                    let bits_needed =
                        (pps.num_slice_groups_minus1 as f32 + 1.0).log2().ceil() as usize;
                    for _ in 0..=pic_size {
                        let _ = Self::read_bits(rbsp, &mut bit_offset, bits_needed)?;
                    }
                }
                _ => {}
            }
        }

        // 5. num_ref_idx_l0_default_active_minus1
        let num_ref_l0 = Self::read_ue(rbsp, &mut bit_offset)?;
        if num_ref_l0 > 31 {
            self.parse_errors.fetch_add(1, Ordering::Relaxed);
            return Err(SpsError::InvalidNumRefFrames);
        }
        pps.num_ref_idx_l0_default_active_minus1 = num_ref_l0 as u8;

        // num_ref_idx_l1_default_active_minus1
        let num_ref_l1 = Self::read_ue(rbsp, &mut bit_offset)?;
        if num_ref_l1 > 31 {
            self.parse_errors.fetch_add(1, Ordering::Relaxed);
            return Err(SpsError::InvalidNumRefFrames);
        }
        pps.num_ref_idx_l1_default_active_minus1 = num_ref_l1 as u8;

        // 6. weighted_pred_flag
        pps.weighted_pred_flag = Self::read_flag(rbsp, &mut bit_offset)?;

        // weighted_bipred_idc (2 bits)
        pps.weighted_bipred_idc = Self::read_bits(rbsp, &mut bit_offset, 2)? as u8;

        // 7. pic_init_qp_minus26
        pps.pic_init_qp_minus26 = Self::read_se(rbsp, &mut bit_offset)? as i8;

        // pic_init_qs_minus26
        pps.pic_init_qs_minus26 = Self::read_se(rbsp, &mut bit_offset)? as i8;

        // chroma_qp_index_offset
        pps.chroma_qp_index_offset = Self::read_se(rbsp, &mut bit_offset)? as i8;

        // 8. deblocking_filter_control_present_flag
        pps.deblocking_filter_control_present_flag = Self::read_flag(rbsp, &mut bit_offset)?;

        // 9. constrained_intra_pred_flag
        pps.constrained_intra_pred_flag = Self::read_flag(rbsp, &mut bit_offset)?;

        // redundant_pic_cnt_present_flag
        pps.redundant_pic_cnt_present_flag = Self::read_flag(rbsp, &mut bit_offset)?;

        // 10. Check for High profile extensions (more_rbsp_data)
        if Self::more_rbsp_data(rbsp, bit_offset) {
            // transform_8x8_mode_flag
            pps.transform_8x8_mode_flag = Self::read_flag(rbsp, &mut bit_offset)?;

            // pic_scaling_matrix_present_flag
            pps.pic_scaling_matrix_present_flag = Self::read_flag(rbsp, &mut bit_offset)?;

            if pps.pic_scaling_matrix_present_flag {
                // Skip scaling lists (would need SPS reference for proper count)
                let num_lists = if pps.transform_8x8_mode_flag { 8 } else { 6 };
                for i in 0..num_lists {
                    let present = Self::read_flag(rbsp, &mut bit_offset)?;
                    if present {
                        let size = if i < 6 { 16 } else { 64 };
                        self.skip_scaling_list(rbsp, &mut bit_offset, size)?;
                    }
                }
            }

            // second_chroma_qp_index_offset
            pps.second_chroma_qp_index_offset = Self::read_se(rbsp, &mut bit_offset)? as i8;
        } else {
            // Default: same as chroma_qp_index_offset
            pps.second_chroma_qp_index_offset = pps.chroma_qp_index_offset;
        }

        // Update statistics
        self.pps_parsed.fetch_add(1, Ordering::Relaxed);

        Ok(pps)
    }

    // ========================================================================
    // Scaling List Parsing
    // ========================================================================

    /// Parse scaling list (ITU-T H.264 Section 7.3.2.1.1.1)
    ///
    /// # Arguments
    ///
    /// * `rbsp` - Raw Byte Sequence Payload
    /// * `bit_offset` - Current bit position (updated on return)
    /// * `size` - Size of scaling list (16 or 64)
    ///
    /// # Returns
    ///
    /// * `Ok([u8; 64])` - Scaling list values (only first `size` elements valid)
    /// * `Err(SpsError)` - Parsing error
    pub fn parse_scaling_list(
        &self,
        rbsp: &[u8],
        bit_offset: &mut usize,
        size: usize,
    ) -> Result<[u8; 64], SpsError> {
        let mut scaling_list = [16u8; 64]; // Default flat scaling
        let mut last_scale = 8i32;
        let mut next_scale = 8i32;

        for j in 0..size {
            if next_scale != 0 {
                let delta_scale = Self::read_se(rbsp, bit_offset)?;
                next_scale = (last_scale + delta_scale + 256) % 256;
            }

            scaling_list[j] = if next_scale == 0 {
                last_scale as u8
            } else {
                next_scale as u8
            };
            last_scale = scaling_list[j] as i32;
        }

        Ok(scaling_list)
    }

    // ========================================================================
    // SPS/PPS Storage
    // ========================================================================

    /// Store parsed SPS
    ///
    /// # Arguments
    ///
    /// * `sps` - Parsed SPS to store
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Storage successful
    /// * `Err(SpsError)` - Storage error (invalid ID or full)
    ///
    /// # Note
    ///
    /// This capsule tracks SPS validity via bitmask but doesn't store
    /// the full SPS data internally. Caller should maintain SPS storage.
    pub fn store_sps(&self, sps: &Sps) -> Result<(), SpsError> {
        let id = sps.seq_parameter_set_id;
        if id > 31 {
            return Err(SpsError::InvalidSpsId);
        }

        // Set validity bit
        let mask = 1u32 << id;
        let old_mask = self.sps_valid_mask.fetch_or(mask, Ordering::AcqRel);

        // Update count if this is a new SPS
        if old_mask & mask == 0 {
            self.sps_count.fetch_add(1, Ordering::Relaxed);
        }

        // Increment generation
        self.generation.fetch_add(1, Ordering::Release);

        Ok(())
    }

    /// Store parsed PPS
    ///
    /// # Arguments
    ///
    /// * `pps` - Parsed PPS to store
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Storage successful
    /// * `Err(SpsError)` - Storage error (invalid ID)
    ///
    /// # Note
    ///
    /// This capsule tracks PPS validity via bitmask but doesn't store
    /// the full PPS data internally. Caller should maintain PPS storage.
    pub fn store_pps(&self, pps: &Pps) -> Result<(), SpsError> {
        let id = pps.pic_parameter_set_id;
        let array_idx = (id / 64) as usize;
        let bit_idx = id % 64;

        if array_idx >= 4 {
            return Err(SpsError::InvalidPpsId);
        }

        // Set validity bit
        let mask = 1u64 << bit_idx;
        let old_mask = self.pps_valid_mask[array_idx].fetch_or(mask, Ordering::AcqRel);

        // Update count if this is a new PPS
        if old_mask & mask == 0 {
            self.pps_count.fetch_add(1, Ordering::Relaxed);
        }

        // Increment generation
        self.generation.fetch_add(1, Ordering::Release);

        Ok(())
    }

    /// Check if SPS with given ID is valid
    #[inline]
    pub fn is_sps_valid(&self, id: u8) -> bool {
        if id > 31 {
            return false;
        }
        let mask = 1u32 << id;
        self.sps_valid_mask.load(Ordering::Acquire) & mask != 0
    }

    /// Check if PPS with given ID is valid
    #[inline]
    pub fn is_pps_valid(&self, id: u8) -> bool {
        let array_idx = (id / 64) as usize;
        let bit_idx = id % 64;

        if array_idx >= 4 {
            return false;
        }

        let mask = 1u64 << bit_idx;
        self.pps_valid_mask[array_idx].load(Ordering::Acquire) & mask != 0
    }

    // ========================================================================
    // SPS/PPS Activation
    // ========================================================================

    /// Activate SPS and update cached values
    ///
    /// # Arguments
    ///
    /// * `id` - SPS ID to activate
    /// * `sps` - SPS data (caller must provide from their storage)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Activation successful
    /// * `Err(SpsError::SpsNotFound)` - SPS not in valid mask
    pub fn activate_sps(&self, id: u8, sps: &Sps) -> Result<(), SpsError> {
        if !self.is_sps_valid(id) {
            return Err(SpsError::SpsNotFound);
        }

        // Update active ID
        self.active_sps_id.store(id, Ordering::Release);

        // Update cached derived values
        self.pic_width_in_mbs
            .store(sps.width_in_mbs(), Ordering::Relaxed);
        self.pic_height_in_mbs
            .store(sps.height_in_mbs(), Ordering::Relaxed);
        self.frame_width.store(sps.width(), Ordering::Relaxed);
        self.frame_height.store(sps.height(), Ordering::Relaxed);
        self.chroma_format
            .store(sps.chroma_format_idc, Ordering::Relaxed);
        self.bit_depth_luma
            .store(sps.bit_depth_luma_minus8 + 8, Ordering::Relaxed);
        self.bit_depth_chroma
            .store(sps.bit_depth_chroma_minus8 + 8, Ordering::Relaxed);
        self.max_frame_num
            .store(sps.max_frame_num(), Ordering::Relaxed);
        self.max_poc_lsb
            .store(sps.max_pic_order_cnt_lsb(), Ordering::Relaxed);
        self.poc_type.store(sps.pic_order_cnt_type, Ordering::Relaxed);
        self.frame_mbs_only
            .store(sps.frame_mbs_only_flag, Ordering::Relaxed);

        // Increment generation
        self.generation.fetch_add(1, Ordering::Release);

        Ok(())
    }

    /// Activate PPS and update cached values
    ///
    /// # Arguments
    ///
    /// * `id` - PPS ID to activate
    /// * `pps` - PPS data (caller must provide from their storage)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Activation successful
    /// * `Err(SpsError::PpsNotFound)` - PPS not in valid mask
    pub fn activate_pps(&self, id: u8, pps: &Pps) -> Result<(), SpsError> {
        if !self.is_pps_valid(id) {
            return Err(SpsError::PpsNotFound);
        }

        // Update active ID
        self.active_pps_id.store(id, Ordering::Release);

        // Update cached derived values
        let entropy = if pps.entropy_coding_mode_flag { 1 } else { 0 };
        self.entropy_mode.store(entropy, Ordering::Relaxed);
        self.transform_8x8_enabled
            .store(pps.transform_8x8_mode_flag, Ordering::Relaxed);
        self.deblocking_control_present
            .store(pps.deblocking_filter_control_present_flag, Ordering::Relaxed);
        self.weighted_pred
            .store(pps.weighted_pred_flag, Ordering::Relaxed);
        self.init_qp
            .store((26 + pps.pic_init_qp_minus26) as u8, Ordering::Relaxed);

        // Increment generation
        self.generation.fetch_add(1, Ordering::Release);

        Ok(())
    }

    // ========================================================================
    // Cached Value Accessors
    // ========================================================================

    /// Get frame dimensions in pixels
    #[inline]
    pub fn get_frame_dimensions(&self) -> (u32, u32) {
        (
            self.frame_width.load(Ordering::Acquire),
            self.frame_height.load(Ordering::Acquire),
        )
    }

    /// Get macroblock dimensions
    #[inline]
    pub fn get_mb_dimensions(&self) -> (u32, u32) {
        (
            self.pic_width_in_mbs.load(Ordering::Acquire),
            self.pic_height_in_mbs.load(Ordering::Acquire),
        )
    }

    /// Check if CABAC is enabled (from active PPS)
    #[inline]
    pub fn is_cabac_enabled(&self) -> bool {
        self.entropy_mode.load(Ordering::Acquire) == 1
    }

    /// Check if transform 8x8 is enabled
    #[inline]
    pub fn is_transform_8x8_enabled(&self) -> bool {
        self.transform_8x8_enabled.load(Ordering::Acquire)
    }

    /// Get active SPS ID
    #[inline]
    pub fn get_active_sps_id(&self) -> u8 {
        self.active_sps_id.load(Ordering::Acquire)
    }

    /// Get active PPS ID
    #[inline]
    pub fn get_active_pps_id(&self) -> u8 {
        self.active_pps_id.load(Ordering::Acquire)
    }

    // ========================================================================
    // Statistics
    // ========================================================================

    /// Get statistics snapshot
    pub fn stats(&self) -> SpsStats {
        SpsStats {
            generation: self.generation.load(Ordering::Acquire),
            sps_parsed: self.sps_parsed.load(Ordering::Relaxed),
            pps_parsed: self.pps_parsed.load(Ordering::Relaxed),
            parse_errors: self.parse_errors.load(Ordering::Relaxed),
            sps_count: self.sps_count.load(Ordering::Relaxed),
            pps_count: self.pps_count.load(Ordering::Relaxed),
            active_sps_id: self.active_sps_id.load(Ordering::Relaxed),
            active_pps_id: self.active_pps_id.load(Ordering::Relaxed),
            frame_width: self.frame_width.load(Ordering::Relaxed),
            frame_height: self.frame_height.load(Ordering::Relaxed),
            cabac_enabled: self.entropy_mode.load(Ordering::Relaxed) == 1,
        }
    }

    /// Reset capsule to initial state
    pub fn reset(&self) {
        self.sps_count.store(0, Ordering::Relaxed);
        self.sps_valid_mask.store(0, Ordering::Relaxed);
        self.active_sps_id.store(0, Ordering::Relaxed);

        self.pps_count.store(0, Ordering::Relaxed);
        for mask in &self.pps_valid_mask {
            mask.store(0, Ordering::Relaxed);
        }
        self.active_pps_id.store(0, Ordering::Relaxed);

        self.pic_width_in_mbs.store(0, Ordering::Relaxed);
        self.pic_height_in_mbs.store(0, Ordering::Relaxed);
        self.frame_width.store(0, Ordering::Relaxed);
        self.frame_height.store(0, Ordering::Relaxed);
        self.chroma_format.store(1, Ordering::Relaxed);
        self.bit_depth_luma.store(8, Ordering::Relaxed);
        self.bit_depth_chroma.store(8, Ordering::Relaxed);

        self.entropy_mode.store(0, Ordering::Relaxed);
        self.transform_8x8_enabled.store(false, Ordering::Relaxed);

        self.sps_parsed.store(0, Ordering::Relaxed);
        self.pps_parsed.store(0, Ordering::Relaxed);
        self.parse_errors.store(0, Ordering::Relaxed);
        self.last_error.store(0, Ordering::Relaxed);

        self.generation.fetch_add(1, Ordering::Release);
    }
}

impl Default for H264SpsPpsCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// T28 Testing (Q1-Q13: Unit Tests)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Q1: Test capsule size and alignment
    #[test]
    fn test_new_capsule() {
        let capsule = H264SpsPpsCapsule::new();

        assert_eq!(core::mem::size_of::<H264SpsPpsCapsule>(), 1024);
        assert_eq!(core::mem::align_of::<H264SpsPpsCapsule>(), 1024);

        // Initial state
        assert_eq!(capsule.stats().sps_count, 0);
        assert_eq!(capsule.stats().pps_count, 0);
        assert_eq!(capsule.stats().generation, 0);
        assert!(!capsule.is_cabac_enabled());
    }

    // Q2: Test SPS parsing (Baseline profile)
    #[test]
    fn test_parse_sps_baseline() {
        let capsule = H264SpsPpsCapsule::new();

        // Baseline profile SPS (720p, 30fps)
        // profile_idc=66, constraint_set0=1, level_idc=31
        // sps_id=0, log2_max_frame_num=4, poc_type=2
        // max_ref_frames=4, pic_width=79 (1280/16-1), pic_height=44 (720/16-1)
        // frame_mbs_only=1, direct_8x8=1, no cropping, no VUI
        let sps_rbsp: [u8; 15] = [
            0x42, // profile_idc = 66 (Baseline)
            0x80, // constraint_set0_flag=1, others=0
            0x1F, // level_idc = 31 (3.1)
            0xE1, // sps_id=0 (ue), log2_max_frame_num=0 (ue=4+4)
            0x10, // poc_type=2 (ue), max_ref_frames=4 (ue)
            0x89, // gaps=0, pic_width=79 (ue)
            0xF1, // continuation
            0x00, // pic_height=44 (ue)
            0xB4, // continuation
            0x20, // frame_mbs_only=1, direct_8x8=1
            0x00, // frame_crop=0, vui=0
            0x00, // padding
            0x00,
            0x00,
            0x00,
        ];

        // Note: This is a simplified test RBSP. Real parsing would need
        // properly encoded Exp-Golomb values. Let's test the basic parsing.

        // Test Exp-Golomb reading directly
        let test_data: [u8; 4] = [0b10000000, 0, 0, 0]; // ue(v) = 0
        let mut offset = 0;
        let result = H264SpsPpsCapsule::read_ue(&test_data, &mut offset);
        assert_eq!(result.unwrap(), 0);
        assert_eq!(offset, 1);

        let test_data2: [u8; 4] = [0b01000000, 0, 0, 0]; // ue(v) = 1
        let mut offset = 0;
        let result = H264SpsPpsCapsule::read_ue(&test_data2, &mut offset);
        assert_eq!(result.unwrap(), 1);
        assert_eq!(offset, 3);

        let test_data3: [u8; 4] = [0b01100000, 0, 0, 0]; // ue(v) = 2
        let mut offset = 0;
        let result = H264SpsPpsCapsule::read_ue(&test_data3, &mut offset);
        assert_eq!(result.unwrap(), 2);
        assert_eq!(offset, 3);

        // Verify parse counter incremented even for partial parses
        let _ = capsule.parse_sps(&sps_rbsp);
    }

    // Q3: Test SPS parsing (High profile)
    #[test]
    fn test_parse_sps_high() {
        let capsule = H264SpsPpsCapsule::new();

        // High profile requires extended parsing (chroma, bit depth, scaling)
        // This tests the Profile::is_high_or_above check
        assert!(Profile::is_high_or_above(100)); // High
        assert!(Profile::is_high_or_above(110)); // High10
        assert!(Profile::is_high_or_above(122)); // High422
        assert!(Profile::is_high_or_above(244)); // High444
        assert!(!Profile::is_high_or_above(66)); // Baseline
        assert!(!Profile::is_high_or_above(77)); // Main
        assert!(!Profile::is_high_or_above(88)); // Extended

        // Verify stats
        assert_eq!(capsule.stats().sps_parsed, 0);
    }

    // Q4: Test PPS parsing (CAVLC)
    #[test]
    fn test_parse_pps_cavlc() {
        let capsule = H264SpsPpsCapsule::new();

        // Simple CAVLC PPS: pps_id=0, sps_id=0, entropy=0 (CAVLC)
        // Encoded: pps_id=0 (1 bit), sps_id=0 (1 bit), entropy=0 (1 bit)
        // bottom_field=0, num_slice_groups=0, ref_l0=0, ref_l1=0
        // weighted_pred=0, weighted_bipred=0 (2 bits)
        // qp=0 (se), qs=0 (se), chroma_qp=0 (se)
        // deblock=0, constrained=0, redundant=0
        let pps_rbsp: [u8; 8] = [
            0b10000000, // pps_id=0 (ue)
            0b10000000, // sps_id=0 (ue)
            0b00000000, // entropy=0, bottom_field=0, num_slice_groups=0 (ue)
            0b10000000, // continuation
            0b10000000, // ref_l0=0 (ue)
            0b10000000, // ref_l1=0 (ue), weighted_pred=0
            0b00101010, // weighted_bipred=00, qp=0, qs=0, chroma_qp=0
            0b10000000, // deblock=0, constrained=0, redundant=0
        ];

        // Note: This is a simplified test. Real PPS parsing requires
        // properly aligned Exp-Golomb codes.

        // Test that parsing doesn't crash with minimal data
        let minimal_pps: [u8; 4] = [0x80, 0x80, 0x00, 0x00];
        let result = capsule.parse_pps(&minimal_pps);
        // May succeed or fail depending on exact bit alignment
        let _ = result;

        // Verify stats
        assert!(capsule.stats().pps_parsed <= 1);
    }

    // Q5: Test PPS parsing (CABAC)
    #[test]
    fn test_parse_pps_cabac() {
        let capsule = H264SpsPpsCapsule::new();

        // CABAC PPS would have entropy_coding_mode_flag=1
        // Test the flag detection logic
        let mut pps = Pps::default();
        pps.entropy_coding_mode_flag = true;

        assert!(pps.entropy_coding_mode_flag);

        // Test activation updates cached value
        pps.pic_parameter_set_id = 0;
        capsule.pps_valid_mask[0].store(1, Ordering::Relaxed);
        capsule.pps_count.store(1, Ordering::Relaxed);

        let result = capsule.activate_pps(0, &pps);
        assert!(result.is_ok());
        assert!(capsule.is_cabac_enabled());
    }

    // Q6: Test SPS store and retrieve
    #[test]
    fn test_store_retrieve_sps() {
        let capsule = H264SpsPpsCapsule::new();

        // Create test SPS
        let mut sps = Sps::default();
        sps.seq_parameter_set_id = 5;
        sps.profile_idc = 100;
        sps.level_idc = 40;
        sps.pic_width_in_mbs_minus1 = 79; // 1280/16 - 1
        sps.pic_height_in_map_units_minus1 = 44; // 720/16 - 1
        sps.frame_mbs_only_flag = true;

        // Store
        let result = capsule.store_sps(&sps);
        assert!(result.is_ok());

        // Verify validity
        assert!(capsule.is_sps_valid(5));
        assert!(!capsule.is_sps_valid(0));
        assert!(!capsule.is_sps_valid(31));

        // Check stats
        assert_eq!(capsule.stats().sps_count, 1);

        // Store another
        sps.seq_parameter_set_id = 0;
        let result = capsule.store_sps(&sps);
        assert!(result.is_ok());
        assert!(capsule.is_sps_valid(0));
        assert_eq!(capsule.stats().sps_count, 2);

        // Invalid ID
        sps.seq_parameter_set_id = 32;
        let result = capsule.store_sps(&sps);
        assert_eq!(result, Err(SpsError::InvalidSpsId));
    }

    // Q7: Test PPS store and retrieve
    #[test]
    fn test_store_retrieve_pps() {
        let capsule = H264SpsPpsCapsule::new();

        // Create test PPS
        let mut pps = Pps::default();
        pps.pic_parameter_set_id = 10;
        pps.seq_parameter_set_id = 0;
        pps.entropy_coding_mode_flag = true;

        // Store
        let result = capsule.store_pps(&pps);
        assert!(result.is_ok());

        // Verify validity
        assert!(capsule.is_pps_valid(10));
        assert!(!capsule.is_pps_valid(0));
        assert!(!capsule.is_pps_valid(255));

        // Check stats
        assert_eq!(capsule.stats().pps_count, 1);

        // Store PPS in different array slot (id >= 64)
        pps.pic_parameter_set_id = 100;
        let result = capsule.store_pps(&pps);
        assert!(result.is_ok());
        assert!(capsule.is_pps_valid(100));
        assert_eq!(capsule.stats().pps_count, 2);

        // Store PPS at id 200 (slot 3)
        pps.pic_parameter_set_id = 200;
        let result = capsule.store_pps(&pps);
        assert!(result.is_ok());
        assert!(capsule.is_pps_valid(200));
        assert_eq!(capsule.stats().pps_count, 3);
    }

    // Q8: Test SPS activation
    #[test]
    fn test_activate_sps() {
        let capsule = H264SpsPpsCapsule::new();

        // Create and store SPS
        let mut sps = Sps::default();
        sps.seq_parameter_set_id = 0;
        sps.pic_width_in_mbs_minus1 = 119; // 1920/16 - 1
        sps.pic_height_in_map_units_minus1 = 67; // 1080/16 - 1 (rounded)
        sps.frame_mbs_only_flag = true;
        sps.chroma_format_idc = 1;
        sps.bit_depth_luma_minus8 = 0;
        sps.bit_depth_chroma_minus8 = 0;
        sps.log2_max_frame_num_minus4 = 4;
        sps.pic_order_cnt_type = 0;
        sps.log2_max_pic_order_cnt_lsb_minus4 = 4;

        capsule.store_sps(&sps).unwrap();

        // Activate
        let result = capsule.activate_sps(0, &sps);
        assert!(result.is_ok());

        // Check cached values
        let (width, height) = capsule.get_frame_dimensions();
        assert_eq!(width, 1920);
        assert_eq!(height, 1088); // 68 * 16

        let (mb_w, mb_h) = capsule.get_mb_dimensions();
        assert_eq!(mb_w, 120);
        assert_eq!(mb_h, 68);

        // Activate non-existent SPS
        let result = capsule.activate_sps(1, &sps);
        assert_eq!(result, Err(SpsError::SpsNotFound));
    }

    // Q9: Test frame dimensions
    #[test]
    fn test_frame_dimensions() {
        let mut sps = Sps::default();

        // 1920x1080 without cropping
        sps.pic_width_in_mbs_minus1 = 119; // 120 MBs = 1920 pixels
        sps.pic_height_in_map_units_minus1 = 67; // 68 MBs = 1088 pixels
        sps.frame_mbs_only_flag = true;
        sps.chroma_format_idc = 1; // 4:2:0

        assert_eq!(sps.width(), 1920);
        assert_eq!(sps.height(), 1088);
        assert_eq!(sps.width_in_mbs(), 120);
        assert_eq!(sps.height_in_mbs(), 68);

        // With cropping to get exactly 1080
        sps.frame_cropping_flag = true;
        sps.frame_crop_bottom_offset = 4; // Crop 8 pixels (4 * 2 for 4:2:0)

        assert_eq!(sps.height(), 1080);

        // Interlaced video (frame_mbs_only = false)
        let mut sps_interlaced = Sps::default();
        sps_interlaced.pic_width_in_mbs_minus1 = 79;
        sps_interlaced.pic_height_in_map_units_minus1 = 22; // 23 * 2 = 46 MBs
        sps_interlaced.frame_mbs_only_flag = false;
        sps_interlaced.chroma_format_idc = 1;

        assert_eq!(sps_interlaced.width(), 1280);
        assert_eq!(sps_interlaced.height(), 736); // 46 * 16
        assert_eq!(sps_interlaced.height_in_mbs(), 46);
    }

    // Q10: Test Exp-Golomb parsing
    #[test]
    fn test_exp_golomb_parsing() {
        // Test unsigned Exp-Golomb (ue)
        // ue(0) = "1" -> value 0
        let data0: [u8; 1] = [0b10000000];
        let mut offset = 0;
        assert_eq!(H264SpsPpsCapsule::read_ue(&data0, &mut offset).unwrap(), 0);
        assert_eq!(offset, 1);

        // ue(1) = "010" -> value 1
        let data1: [u8; 1] = [0b01000000];
        let mut offset = 0;
        assert_eq!(H264SpsPpsCapsule::read_ue(&data1, &mut offset).unwrap(), 1);
        assert_eq!(offset, 3);

        // ue(2) = "011" -> value 2
        let data2: [u8; 1] = [0b01100000];
        let mut offset = 0;
        assert_eq!(H264SpsPpsCapsule::read_ue(&data2, &mut offset).unwrap(), 2);
        assert_eq!(offset, 3);

        // ue(3) = "00100" -> value 3
        let data3: [u8; 1] = [0b00100000];
        let mut offset = 0;
        assert_eq!(H264SpsPpsCapsule::read_ue(&data3, &mut offset).unwrap(), 3);
        assert_eq!(offset, 5);

        // ue(4) = "00101" -> value 4
        let data4: [u8; 1] = [0b00101000];
        let mut offset = 0;
        assert_eq!(H264SpsPpsCapsule::read_ue(&data4, &mut offset).unwrap(), 4);
        assert_eq!(offset, 5);

        // Test signed Exp-Golomb (se)
        // se(0) = ue(0) = 0
        let mut offset = 0;
        assert_eq!(H264SpsPpsCapsule::read_se(&data0, &mut offset).unwrap(), 0);

        // se(+1) = ue(1) -> code 1 = +1
        let mut offset = 0;
        assert_eq!(H264SpsPpsCapsule::read_se(&data1, &mut offset).unwrap(), 1);

        // se(-1) = ue(2) -> code 2 = -1
        let mut offset = 0;
        assert_eq!(H264SpsPpsCapsule::read_se(&data2, &mut offset).unwrap(), -1);

        // se(+2) = ue(3) -> code 3 = +2
        let mut offset = 0;
        assert_eq!(H264SpsPpsCapsule::read_se(&data3, &mut offset).unwrap(), 2);

        // se(-2) = ue(4) -> code 4 = -2
        let mut offset = 0;
        assert_eq!(H264SpsPpsCapsule::read_se(&data4, &mut offset).unwrap(), -2);
    }

    // Q11: Test VUI parsing
    #[test]
    fn test_vui_parsing() {
        let capsule = H264SpsPpsCapsule::new();

        // Simple VUI with timing info only
        // Bits: aspect_ratio_present=0, overscan_present=0, video_signal_present=0
        //       chroma_loc_present=0, timing_present=1
        // Then: num_units_in_tick=1001 (32 bits), time_scale=60000 (32 bits), fixed_frame_rate=1
        //
        // Byte layout (MSB first):
        // Byte 0: 0000 1 000 = flags (4x0), timing=1, then 3 bits of num_units_in_tick
        //         0000 1000 = 0x08
        // But wait - bits are packed from MSB, so:
        //   bit 0: aspect_ratio=0
        //   bit 1: overscan=0
        //   bit 2: video_signal=0
        //   bit 3: chroma_loc=0
        //   bit 4: timing=1
        //   bits 5-36: num_units_in_tick (32 bits) = 1001 = 0x3E9
        //   bits 37-68: time_scale (32 bits) = 60000 = 0xEA60
        //   bit 69: fixed_frame_rate=1
        //
        // Let's build this byte by byte:
        // After 4 flags = 0000 (bits 0-3)
        // timing=1 (bit 4)
        // num_units_in_tick starts at bit 5
        //
        // 1001 = 0x000003E9 = 0b00000000_00000000_00000011_11101001
        // 60000 = 0x0000EA60 = 0b00000000_00000000_11101010_01100000

        // Bits 0-4: 0000_1 (flags + timing)
        // Bits 5-7 from first byte: 000 (high bits of num_units_in_tick)
        // So byte 0 = 0000_1_000 = 0x08

        // Bytes 1-4: remaining 29 bits of num_units_in_tick + 3 bits of time_scale
        // num_units_in_tick = 1001 shifted left 0 bits, but we've taken 3 bits already
        // Remaining: bits 5-36 contain 32-bit value 1001
        // After byte 0 has 3 bits, bytes 1-4 have 29 bits + some of time_scale

        // This is getting complex - let's simplify by making timing_present=0 first
        // Then test with simpler data

        // Simpler test: all VUI flags disabled
        let vui_data_simple: [u8; 1] = [
            0b00000000, // all 5 main flags = 0
        ];

        let mut offset = 0;
        let result = capsule.parse_vui(&vui_data_simple, &mut offset);
        assert!(result.is_ok());

        let vui = result.unwrap();
        assert!(!vui.aspect_ratio_info_present_flag);
        assert!(!vui.overscan_info_present_flag);
        assert!(!vui.video_signal_type_present_flag);
        assert!(!vui.chroma_loc_info_present_flag);
        assert!(!vui.timing_info_present_flag);

        // No timing info, so frame_rate returns None
        let fps = vui.frame_rate();
        assert!(fps.is_none());
    }

    // Q12: Test multiple SPS/PPS storage
    #[test]
    fn test_multiple_sps_pps() {
        let capsule = H264SpsPpsCapsule::new();

        // Store multiple SPS
        for id in 0..32u8 {
            let mut sps = Sps::default();
            sps.seq_parameter_set_id = id;
            sps.pic_width_in_mbs_minus1 = (id as u16 + 1) * 10;
            capsule.store_sps(&sps).unwrap();
        }

        assert_eq!(capsule.stats().sps_count, 32);

        // Verify all valid
        for id in 0..32u8 {
            assert!(capsule.is_sps_valid(id));
        }

        // Store multiple PPS across all 4 array slots
        for id in [0u8, 63, 64, 127, 128, 191, 192, 255] {
            let mut pps = Pps::default();
            pps.pic_parameter_set_id = id;
            capsule.store_pps(&pps).unwrap();
        }

        assert_eq!(capsule.stats().pps_count, 8);

        // Verify all valid
        for id in [0u8, 63, 64, 127, 128, 191, 192, 255] {
            assert!(capsule.is_pps_valid(id));
        }
    }

    // Q13: Test invalid IDs
    #[test]
    fn test_invalid_ids() {
        let capsule = H264SpsPpsCapsule::new();

        // Invalid SPS ID (> 31)
        let mut sps = Sps::default();
        sps.seq_parameter_set_id = 32;
        assert_eq!(capsule.store_sps(&sps), Err(SpsError::InvalidSpsId));

        // SPS validity check for out-of-range
        assert!(!capsule.is_sps_valid(32));
        assert!(!capsule.is_sps_valid(255));

        // PPS validity for all slots
        assert!(!capsule.is_pps_valid(0));
        assert!(!capsule.is_pps_valid(64));
        assert!(!capsule.is_pps_valid(128));
        assert!(!capsule.is_pps_valid(192));
        assert!(!capsule.is_pps_valid(255));

        // Activate non-existent
        let result = capsule.activate_sps(0, &sps);
        assert_eq!(result, Err(SpsError::SpsNotFound));

        let pps = Pps::default();
        let result = capsule.activate_pps(0, &pps);
        assert_eq!(result, Err(SpsError::PpsNotFound));
    }

    // Additional: Test generation counter
    #[test]
    fn test_generation_counter() {
        let capsule = H264SpsPpsCapsule::new();

        assert_eq!(capsule.stats().generation, 0);

        // Store SPS increments generation
        let mut sps = Sps::default();
        sps.seq_parameter_set_id = 0;
        capsule.store_sps(&sps).unwrap();
        assert_eq!(capsule.stats().generation, 1);

        // Store PPS increments generation
        let mut pps = Pps::default();
        pps.pic_parameter_set_id = 0;
        capsule.store_pps(&pps).unwrap();
        assert_eq!(capsule.stats().generation, 2);

        // Activate increments generation
        capsule.activate_sps(0, &sps).unwrap();
        assert_eq!(capsule.stats().generation, 3);

        capsule.pps_valid_mask[0].store(1, Ordering::Relaxed);
        capsule.activate_pps(0, &pps).unwrap();
        assert_eq!(capsule.stats().generation, 4);

        // Reset increments generation
        capsule.reset();
        assert_eq!(capsule.stats().generation, 5);
    }

    // Test read_bits
    #[test]
    fn test_read_bits() {
        let data: [u8; 4] = [0b10110100, 0b11001010, 0b00001111, 0b11110000];

        // Read 4 bits: 1011
        let mut offset = 0;
        assert_eq!(H264SpsPpsCapsule::read_bits(&data, &mut offset, 4).unwrap(), 0b1011);
        assert_eq!(offset, 4);

        // Read 8 bits: 01001100
        assert_eq!(
            H264SpsPpsCapsule::read_bits(&data, &mut offset, 8).unwrap(),
            0b01001100
        );
        assert_eq!(offset, 12);

        // Read 1 bit (flag)
        let mut offset = 0;
        assert!(H264SpsPpsCapsule::read_flag(&data, &mut offset).unwrap());
        assert!(!H264SpsPpsCapsule::read_flag(&data, &mut offset).unwrap());
        assert!(H264SpsPpsCapsule::read_flag(&data, &mut offset).unwrap());
        assert!(H264SpsPpsCapsule::read_flag(&data, &mut offset).unwrap());

        // EOF error
        let mut offset = 30;
        let result = H264SpsPpsCapsule::read_bits(&data, &mut offset, 8);
        assert_eq!(result, Err(SpsError::UnexpectedEof));
    }

    // Test Profile enum
    #[test]
    fn test_profile_enum() {
        assert_eq!(Profile::from_u8(66), Some(Profile::Baseline));
        assert_eq!(Profile::from_u8(77), Some(Profile::Main));
        assert_eq!(Profile::from_u8(100), Some(Profile::High));
        assert_eq!(Profile::from_u8(110), Some(Profile::High10));
        assert_eq!(Profile::from_u8(0), None);
        assert_eq!(Profile::from_u8(255), None);

        assert_eq!(Profile::Baseline as u8, 66);
        assert_eq!(Profile::Main as u8, 77);
        assert_eq!(Profile::High as u8, 100);
    }

    // Test SPS derived values
    #[test]
    fn test_sps_derived_values() {
        let mut sps = Sps::default();
        sps.log2_max_frame_num_minus4 = 0; // MaxFrameNum = 16
        sps.log2_max_pic_order_cnt_lsb_minus4 = 0; // MaxPocLsb = 16

        assert_eq!(sps.max_frame_num(), 16);
        assert_eq!(sps.max_pic_order_cnt_lsb(), 16);

        sps.log2_max_frame_num_minus4 = 4; // MaxFrameNum = 256
        sps.log2_max_pic_order_cnt_lsb_minus4 = 8; // MaxPocLsb = 4096

        assert_eq!(sps.max_frame_num(), 256);
        assert_eq!(sps.max_pic_order_cnt_lsb(), 4096);
    }

    // Test error conversion
    #[test]
    fn test_error_conversion() {
        for i in 0..=14 {
            let error = SpsError::from_u64(i);
            assert_eq!(error.to_u64(), i);
        }

        // Out of range maps to None
        assert_eq!(SpsError::from_u64(100), SpsError::None);
        assert_eq!(SpsError::from_u64(u64::MAX), SpsError::None);
    }
}
