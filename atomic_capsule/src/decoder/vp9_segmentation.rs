//! VP9 Segmentation Capsule (T4 Batch Tier)
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Implements VP9 segmentation parsing and feature lookup per Google VP9 specification.
//! VP9 supports up to 8 segments with 4 per-segment features (ALT_Q, ALT_LF, REF_FRAME, SKIP).
//!
//! # Architecture
//!
//! This capsule provides:
//! 1. Segmentation header parsing from boolean decoder
//! 2. Feature enable/disable tracking (8 segments x 4 features = 32 bits)
//! 3. Feature data storage (8 segments x 4 features x 16-bit values)
//! 4. Tree probability tables for entropy decoding
//! 5. Temporal prediction probabilities
//!
//! # State Machine
//!
//! ```text
//! Disabled -> Enabled (parsing) -> Ready -> Updating (per-frame) -> Ready
//! ```
//!
//! # UCE34/Chaos Compliance
//!
//! - **Q10**: T4 Batch tier (batch segment map processing)
//! - **Q33**: 100% lockfree (AtomicU64/AtomicU32/AtomicU8)
//! - **Q34**: Generation counter for audit trail
//! - 512B cache-aligned
//!
//! # VP9 Specification Reference
//!
//! - Section 7.2.4: Segmentation params syntax
//! - Section 8.4: Segment feature data storage
//! - Section 8.3.5: Segment ID decoding

use core::sync::atomic::{AtomicU64, AtomicU32, AtomicU8, Ordering};

// ============================================================================
// Constants and Types
// ============================================================================

/// Maximum number of segments in VP9
pub const VP9_MAX_SEGMENTS: usize = 8;

/// Number of segment features per segment
pub const VP9_SEG_LVL_MAX: usize = 4;

/// Segment tree probability count (7 for 8-way tree)
pub const VP9_TREE_PROBS: usize = 7;

/// Prediction probability count for temporal mode
pub const VP9_PRED_PROBS: usize = 3;

/// Segment feature types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SegFeature {
    /// Delta quantizer adjustment
    AltQ = 0,
    /// Delta loop filter strength adjustment
    AltLf = 1,
    /// Reference frame constraint
    RefFrame = 2,
    /// Skip residual coding
    Skip = 3,
}

impl SegFeature {
    /// Get feature index
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Create from index (with bounds checking)
    #[inline]
    pub const fn from_index(idx: usize) -> Option<Self> {
        match idx {
            0 => Some(Self::AltQ),
            1 => Some(Self::AltLf),
            2 => Some(Self::RefFrame),
            3 => Some(Self::Skip),
            _ => None,
        }
    }

    /// Get the number of bits for this feature's data
    /// VP9 spec: ALT_Q uses 8 bits signed, ALT_LF uses 6 bits signed,
    /// REF_FRAME uses 2 bits unsigned, SKIP uses 0 bits (flag only)
    #[inline]
    pub const fn data_bits(self) -> u8 {
        match self {
            Self::AltQ => 8,
            Self::AltLf => 6,
            Self::RefFrame => 2,
            Self::Skip => 0,
        }
    }

    /// Whether this feature's data is signed
    #[inline]
    pub const fn is_signed(self) -> bool {
        match self {
            Self::AltQ | Self::AltLf => true,
            Self::RefFrame | Self::Skip => false,
        }
    }
}

impl From<u8> for SegFeature {
    fn from(v: u8) -> Self {
        match v & 3 {
            0 => Self::AltQ,
            1 => Self::AltLf,
            2 => Self::RefFrame,
            _ => Self::Skip,
        }
    }
}

/// Segmentation state flags (packed into state AtomicU64)
pub mod seg_flags {
    /// Segmentation enabled
    pub const ENABLED: u64 = 1 << 0;
    /// Update map for this frame
    pub const UPDATE_MAP: u64 = 1 << 1;
    /// Update data for this frame
    pub const UPDATE_DATA: u64 = 1 << 2;
    /// Absolute values (not delta)
    pub const ABS_DELTA: u64 = 1 << 3;
    /// Temporal prediction enabled
    pub const TEMPORAL_UPDATE: u64 = 1 << 4;
    /// Ready for decoding
    pub const READY: u64 = 1 << 5;
    /// Error state
    pub const ERROR: u64 = 1 << 6;
    /// Initialization complete
    pub const INITIALIZED: u64 = 1 << 7;
}

/// VP9 Segmentation errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[repr(u8)]
pub enum Vp9SegmentationError {
    /// No error
    #[error("no error")]
    None = 0,
    /// Invalid segment ID (>= 8)
    #[error("invalid segment ID")]
    InvalidSegmentId = 1,
    /// Invalid feature index (>= 4)
    #[error("invalid feature index")]
    InvalidFeature = 2,
    /// Boolean decoder error
    #[error("boolean decoder error")]
    BoolDecoderError = 3,
    /// Segmentation not enabled
    #[error("segmentation not enabled")]
    NotEnabled = 4,
    /// Invalid state for operation
    #[error("invalid state")]
    InvalidState = 5,
    /// Data range overflow
    #[error("data range overflow")]
    DataOverflow = 6,
}

/// VP9 Segmentation statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct Vp9SegmentationStats {
    /// Parse operations performed
    pub parse_count: u64,
    /// Feature lookups performed
    pub feature_lookups: u64,
    /// Segment map updates
    pub map_updates: u64,
    /// Active segments count (0-8)
    pub active_segments: u8,
    /// Total features enabled across all segments
    pub features_enabled: u8,
    /// Generation counter
    pub generation: u64,
}

// ============================================================================
// Mock Boolean Decoder (for testing without full VP9 decoder)
// ============================================================================

/// Mock VP9 boolean decoder for testing
/// In production, this would be the actual Vp9BoolDecoderCapsule
#[derive(Debug)]
pub struct Vp9BoolDecoderCapsule {
    /// Data buffer
    data: Vec<u8>,
    /// Current byte position
    pos: usize,
    /// Current bit position within byte
    bit_pos: u8,
}

impl Vp9BoolDecoderCapsule {
    /// Create new boolean decoder from data
    pub fn new(data: &[u8]) -> Self {
        Self {
            data: data.to_vec(),
            pos: 0,
            bit_pos: 0,
        }
    }

    /// Read a single bit (equiprobable)
    pub fn read_bit(&mut self) -> Result<bool, Vp9SegmentationError> {
        if self.pos >= self.data.len() {
            return Err(Vp9SegmentationError::BoolDecoderError);
        }
        let bit = (self.data[self.pos] >> (7 - self.bit_pos)) & 1;
        self.bit_pos += 1;
        if self.bit_pos >= 8 {
            self.bit_pos = 0;
            self.pos += 1;
        }
        Ok(bit != 0)
    }

    /// Read N bits as unsigned integer
    pub fn read_literal(&mut self, bits: u8) -> Result<u32, Vp9SegmentationError> {
        let mut value = 0u32;
        for _ in 0..bits {
            value = (value << 1) | (self.read_bit()? as u32);
        }
        Ok(value)
    }

    /// Read a probability value (8 bits, or use default)
    pub fn read_prob(&mut self, use_default: bool, default: u8) -> Result<u8, Vp9SegmentationError> {
        if use_default {
            Ok(default)
        } else {
            Ok(self.read_literal(8)? as u8)
        }
    }

    /// Read signed value (sign bit + magnitude)
    pub fn read_signed(&mut self, bits: u8) -> Result<i16, Vp9SegmentationError> {
        let magnitude = self.read_literal(bits)? as i16;
        let sign = self.read_bit()?;
        if sign {
            Ok(-magnitude)
        } else {
            Ok(magnitude)
        }
    }
}

// ============================================================================
// Vp9SegmentationCapsule - T4 Batch Tier
// ============================================================================

/// T4 Batch capsule for VP9 segmentation
///
/// This capsule manages VP9 segmentation state including:
/// - Segmentation enable/disable and update flags
/// - Per-segment feature enables (32 bits for 8 segments x 4 features)
/// - Per-segment feature data (signed 16-bit values)
/// - Tree and prediction probabilities for entropy decoding
///
/// # Layout (512B cache-aligned)
///
/// ```text
/// Offset  Field               Size    Description
/// ------  -----               ----    -----------
/// 0       state               8       Segmentation flags (enabled|update_map|update_data|abs_delta|temporal)
/// 8       feature_enables     4       Packed bitfield (8 segments x 4 features = 32 bits)
/// 12      last_error          4       Last error code
/// 16-79   feature_data[8]     64      8 x AtomicU64 (4 features x 16 bits per segment)
/// 80      tree_probs          8       7 tree probabilities packed (7 x 8 bits = 56 bits)
/// 88      pred_probs          4       3 prediction probabilities (3 x 8 bits = 24 bits)
/// 92      _pad0               4       Padding
/// 96      generation          8       Generation counter (Q34 audit)
/// 104     parse_count         8       Parse operations
/// 112     feature_lookups     8       Feature lookup count
/// 120     map_updates         8       Map update count
/// 128     segments_used       4       Bitmap of segments in use
/// 132     _pad1               4       Padding
/// 136-511 _padding            376     Padding to 512B
/// ```
#[repr(C, align(512))]
pub struct Vp9SegmentationCapsule {
    // Segmentation state flags (8 bytes)
    /// Packed state flags: enabled | update_map | update_data | abs_delta | temporal
    state: AtomicU64,

    // Feature enables (4 bytes + 4 padding)
    /// Packed feature enables: bit (seg * 4 + feature) = enabled
    feature_enables: AtomicU32,

    /// Last error code
    last_error: AtomicU32,

    // Feature data: 8 segments, each with 4 features (64 bytes)
    // Each segment packs 4 x 16-bit signed values into one AtomicU64
    // Layout per segment: [ALT_Q:16][ALT_LF:16][REF_FRAME:16][SKIP:16]
    /// Feature data for all 8 segments
    feature_data: [AtomicU64; VP9_MAX_SEGMENTS],

    // Probabilities (12 bytes + 4 padding)
    /// Tree probabilities (7 x 8 bits = 56 bits)
    tree_probs: AtomicU64,

    /// Prediction probabilities (3 x 8 bits = 24 bits)
    pred_probs: AtomicU32,

    /// Padding
    _pad0: u32,

    // Statistics (40 bytes)
    /// Generation counter (Q34 audit trail)
    generation: AtomicU64,

    /// Parse operations count
    parse_count: AtomicU64,

    /// Feature lookup count
    feature_lookups: AtomicU64,

    /// Segment map update count
    map_updates: AtomicU64,

    /// Bitmap of segments actually used in frame
    segments_used: AtomicU32,

    /// Padding
    _pad1: u32,

    // Padding to 512 bytes
    _padding: [u8; 376],
}

// Safety: Vp9SegmentationCapsule only contains atomic types and padding
// #ASSUME: All fields are either atomic or padding bytes with no invariants
// #VERIFY: Verified via manual inspection - no raw pointers, no references
unsafe impl Send for Vp9SegmentationCapsule {}
unsafe impl Sync for Vp9SegmentationCapsule {}

impl Default for Vp9SegmentationCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl Vp9SegmentationCapsule {
    /// Create a new VP9 segmentation capsule with default values
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            feature_enables: AtomicU32::new(0),
            last_error: AtomicU32::new(0),
            feature_data: [
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
            ],
            tree_probs: AtomicU64::new(0x00FF_FFFF_FFFF_FFFF), // Default 255 for all 7 probs
            pred_probs: AtomicU32::new(0x00FF_FFFF), // Default 255 for all 3 probs
            _pad0: 0,
            generation: AtomicU64::new(0),
            parse_count: AtomicU64::new(0),
            feature_lookups: AtomicU64::new(0),
            map_updates: AtomicU64::new(0),
            segments_used: AtomicU32::new(0),
            _pad1: 0,
            _padding: [0u8; 376],
        }
    }

    /// Reset capsule to initial state (segmentation disabled)
    pub fn reset(&self) {
        self.state.store(0, Ordering::Release);
        self.feature_enables.store(0, Ordering::Release);
        self.last_error.store(0, Ordering::Release);
        for seg in 0..VP9_MAX_SEGMENTS {
            self.feature_data[seg].store(0, Ordering::Release);
        }
        self.tree_probs.store(0x00FF_FFFF_FFFF_FFFF, Ordering::Release);
        self.pred_probs.store(0x00FF_FFFF, Ordering::Release);
        self.segments_used.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    // ========================================================================
    // State Management
    // ========================================================================

    /// Check if segmentation is enabled
    #[inline]
    pub fn is_enabled(&self) -> bool {
        (self.state.load(Ordering::Acquire) & seg_flags::ENABLED) != 0
    }

    /// Check if map update is enabled for this frame
    #[inline]
    pub fn is_update_map(&self) -> bool {
        (self.state.load(Ordering::Acquire) & seg_flags::UPDATE_MAP) != 0
    }

    /// Check if data update is enabled for this frame
    #[inline]
    pub fn is_update_data(&self) -> bool {
        (self.state.load(Ordering::Acquire) & seg_flags::UPDATE_DATA) != 0
    }

    /// Check if using absolute values (not delta)
    #[inline]
    pub fn is_abs_delta(&self) -> bool {
        (self.state.load(Ordering::Acquire) & seg_flags::ABS_DELTA) != 0
    }

    /// Check if temporal prediction is enabled
    #[inline]
    pub fn is_temporal_update(&self) -> bool {
        (self.state.load(Ordering::Acquire) & seg_flags::TEMPORAL_UPDATE) != 0
    }

    /// Get last error
    #[inline]
    pub fn last_error(&self) -> Vp9SegmentationError {
        match self.last_error.load(Ordering::Acquire) {
            0 => Vp9SegmentationError::None,
            1 => Vp9SegmentationError::InvalidSegmentId,
            2 => Vp9SegmentationError::InvalidFeature,
            3 => Vp9SegmentationError::BoolDecoderError,
            4 => Vp9SegmentationError::NotEnabled,
            5 => Vp9SegmentationError::InvalidState,
            6 => Vp9SegmentationError::DataOverflow,
            _ => Vp9SegmentationError::None,
        }
    }

    // ========================================================================
    // Segmentation Header Parsing
    // ========================================================================

    /// Parse segmentation header from boolean decoder
    ///
    /// VP9 Spec Section 7.2.4: segmentation_params()
    ///
    /// # Arguments
    /// * `bool_dec` - VP9 boolean decoder
    ///
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(Vp9SegmentationError)` on failure
    pub fn parse_segmentation(
        &self,
        bool_dec: &mut Vp9BoolDecoderCapsule,
    ) -> Result<(), Vp9SegmentationError> {
        // Read segmentation_enabled
        let enabled = bool_dec.read_bit()?;

        if !enabled {
            // Clear enabled flag, keep other state for reference frame
            let mut state = self.state.load(Ordering::Acquire);
            state &= !seg_flags::ENABLED;
            self.state.store(state, Ordering::Release);
            self.parse_count.fetch_add(1, Ordering::Relaxed);
            self.generation.fetch_add(1, Ordering::AcqRel);
            return Ok(());
        }

        let mut state_flags = seg_flags::ENABLED | seg_flags::INITIALIZED;

        // Read segmentation_update_map
        let update_map = bool_dec.read_bit()?;
        if update_map {
            state_flags |= seg_flags::UPDATE_MAP;

            // Read tree probabilities (7 values)
            let mut tree_probs_packed = 0u64;
            for i in 0..VP9_TREE_PROBS {
                let prob_present = bool_dec.read_bit()?;
                let prob = if prob_present {
                    bool_dec.read_literal(8)? as u8
                } else {
                    255 // Default probability
                };
                tree_probs_packed |= (prob as u64) << (i * 8);
            }
            self.tree_probs.store(tree_probs_packed, Ordering::Release);

            // Read temporal_update flag
            let temporal_update = bool_dec.read_bit()?;
            if temporal_update {
                state_flags |= seg_flags::TEMPORAL_UPDATE;

                // Read prediction probabilities (3 values)
                let mut pred_probs_packed = 0u32;
                for i in 0..VP9_PRED_PROBS {
                    let prob_present = bool_dec.read_bit()?;
                    let prob = if prob_present {
                        bool_dec.read_literal(8)? as u8
                    } else {
                        255 // Default probability
                    };
                    pred_probs_packed |= (prob as u32) << (i * 8);
                }
                self.pred_probs.store(pred_probs_packed, Ordering::Release);
            }
        }

        // Read segmentation_update_data
        let update_data = bool_dec.read_bit()?;
        if update_data {
            state_flags |= seg_flags::UPDATE_DATA;

            // Read segmentation_abs_or_delta_update
            let abs_delta = bool_dec.read_bit()?;
            if abs_delta {
                state_flags |= seg_flags::ABS_DELTA;
            }

            // Read feature data for each segment
            let mut feature_enables = 0u32;

            for seg in 0..VP9_MAX_SEGMENTS {
                let mut seg_data = 0u64;

                for feature_idx in 0..VP9_SEG_LVL_MAX {
                    let feature = SegFeature::from_index(feature_idx).unwrap();
                    let feature_enabled = bool_dec.read_bit()?;

                    if feature_enabled {
                        // Set enable bit
                        let enable_bit = (seg * VP9_SEG_LVL_MAX + feature_idx) as u32;
                        feature_enables |= 1 << enable_bit;

                        // Read feature data (if feature has data)
                        let bits = feature.data_bits();
                        if bits > 0 {
                            let value = if feature.is_signed() {
                                bool_dec.read_signed(bits)?
                            } else {
                                bool_dec.read_literal(bits)? as i16
                            };
                            // Pack into segment data (16 bits per feature)
                            seg_data |= ((value as u16) as u64) << (feature_idx * 16);
                        }
                    }
                }

                self.feature_data[seg].store(seg_data, Ordering::Release);
            }

            self.feature_enables.store(feature_enables, Ordering::Release);
        }

        state_flags |= seg_flags::READY;
        self.state.store(state_flags, Ordering::Release);
        self.parse_count.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::AcqRel);

        Ok(())
    }

    // ========================================================================
    // Feature Access
    // ========================================================================

    /// Check if a feature is active for a segment
    ///
    /// # Arguments
    /// * `segment_id` - Segment ID (0-7)
    /// * `feature` - Feature type
    ///
    /// # Returns
    /// * `true` if feature is enabled for this segment
    #[inline]
    pub fn segment_feature_active(&self, segment_id: u8, feature: SegFeature) -> bool {
        if segment_id >= VP9_MAX_SEGMENTS as u8 {
            return false;
        }

        // Check if segmentation is enabled
        if !self.is_enabled() {
            return false;
        }

        let enable_bit = (segment_id as usize) * VP9_SEG_LVL_MAX + feature.index();
        let enables = self.feature_enables.load(Ordering::Acquire);

        self.feature_lookups.fetch_add(1, Ordering::Relaxed);

        (enables & (1 << enable_bit)) != 0
    }

    /// Get feature data for a segment
    ///
    /// # Arguments
    /// * `segment_id` - Segment ID (0-7)
    /// * `feature` - Feature type
    ///
    /// # Returns
    /// * Feature data value (signed 16-bit)
    #[inline]
    pub fn segment_feature_data(&self, segment_id: u8, feature: SegFeature) -> i16 {
        if segment_id >= VP9_MAX_SEGMENTS as u8 {
            return 0;
        }

        let seg_data = self.feature_data[segment_id as usize].load(Ordering::Acquire);
        let shift = feature.index() * 16;
        let value = ((seg_data >> shift) & 0xFFFF) as u16;

        self.feature_lookups.fetch_add(1, Ordering::Relaxed);

        // Sign-extend if needed
        if feature.is_signed() && (value & 0x8000) != 0 {
            value as i16
        } else {
            value as i16
        }
    }

    /// Set feature enable state for a segment
    ///
    /// # Arguments
    /// * `segment_id` - Segment ID (0-7)
    /// * `feature` - Feature type
    /// * `enabled` - Whether to enable the feature
    pub fn set_segment_feature_enabled(
        &self,
        segment_id: u8,
        feature: SegFeature,
        enabled: bool,
    ) -> Result<(), Vp9SegmentationError> {
        if segment_id >= VP9_MAX_SEGMENTS as u8 {
            self.last_error.store(Vp9SegmentationError::InvalidSegmentId as u32, Ordering::Release);
            return Err(Vp9SegmentationError::InvalidSegmentId);
        }

        let enable_bit = (segment_id as usize) * VP9_SEG_LVL_MAX + feature.index();

        loop {
            let current = self.feature_enables.load(Ordering::Acquire);
            let new_value = if enabled {
                current | (1 << enable_bit)
            } else {
                current & !(1 << enable_bit)
            };

            if self.feature_enables.compare_exchange_weak(
                current,
                new_value,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                break;
            }
        }

        self.generation.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    /// Set feature data for a segment
    ///
    /// # Arguments
    /// * `segment_id` - Segment ID (0-7)
    /// * `feature` - Feature type
    /// * `data` - Feature data value
    pub fn set_segment_feature_data(
        &self,
        segment_id: u8,
        feature: SegFeature,
        data: i16,
    ) -> Result<(), Vp9SegmentationError> {
        if segment_id >= VP9_MAX_SEGMENTS as u8 {
            self.last_error.store(Vp9SegmentationError::InvalidSegmentId as u32, Ordering::Release);
            return Err(Vp9SegmentationError::InvalidSegmentId);
        }

        let shift = feature.index() * 16;
        let mask = 0xFFFFu64 << shift;
        let value = ((data as u16) as u64) << shift;

        loop {
            let current = self.feature_data[segment_id as usize].load(Ordering::Acquire);
            let new_value = (current & !mask) | value;

            if self.feature_data[segment_id as usize].compare_exchange_weak(
                current,
                new_value,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                break;
            }
        }

        self.generation.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    // ========================================================================
    // Probability Access
    // ========================================================================

    /// Get tree probabilities as array
    pub fn tree_probs(&self) -> [u8; VP9_TREE_PROBS] {
        let packed = self.tree_probs.load(Ordering::Acquire);
        [
            (packed & 0xFF) as u8,
            ((packed >> 8) & 0xFF) as u8,
            ((packed >> 16) & 0xFF) as u8,
            ((packed >> 24) & 0xFF) as u8,
            ((packed >> 32) & 0xFF) as u8,
            ((packed >> 40) & 0xFF) as u8,
            ((packed >> 48) & 0xFF) as u8,
        ]
    }

    /// Get prediction probabilities as array
    pub fn pred_probs(&self) -> [u8; VP9_PRED_PROBS] {
        let packed = self.pred_probs.load(Ordering::Acquire);
        [
            (packed & 0xFF) as u8,
            ((packed >> 8) & 0xFF) as u8,
            ((packed >> 16) & 0xFF) as u8,
        ]
    }

    /// Set tree probability at index
    pub fn set_tree_prob(&self, idx: usize, prob: u8) -> Result<(), Vp9SegmentationError> {
        if idx >= VP9_TREE_PROBS {
            return Err(Vp9SegmentationError::InvalidFeature);
        }

        let shift = idx * 8;
        let mask = 0xFFu64 << shift;
        let value = (prob as u64) << shift;

        loop {
            let current = self.tree_probs.load(Ordering::Acquire);
            let new_value = (current & !mask) | value;

            if self.tree_probs.compare_exchange_weak(
                current,
                new_value,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                break;
            }
        }

        Ok(())
    }

    /// Set prediction probability at index
    pub fn set_pred_prob(&self, idx: usize, prob: u8) -> Result<(), Vp9SegmentationError> {
        if idx >= VP9_PRED_PROBS {
            return Err(Vp9SegmentationError::InvalidFeature);
        }

        let shift = idx * 8;
        let mask = 0xFFu32 << shift;
        let value = (prob as u32) << shift;

        loop {
            let current = self.pred_probs.load(Ordering::Acquire);
            let new_value = (current & !mask) | value;

            if self.pred_probs.compare_exchange_weak(
                current,
                new_value,
                Ordering::AcqRel,
                Ordering::Acquire,
            ).is_ok() {
                break;
            }
        }

        Ok(())
    }

    // ========================================================================
    // Segment Map Operations
    // ========================================================================

    /// Get segment ID for a macroblock position
    ///
    /// Note: This method requires external segment map storage.
    /// The segment map is stored separately (one byte per 8x8 block).
    ///
    /// # Arguments
    /// * `mi_row` - Macroblock row index
    /// * `mi_col` - Macroblock column index
    /// * `mi_cols` - Total columns in frame
    /// * `segment_map` - External segment map array
    ///
    /// # Returns
    /// * Segment ID (0-7)
    #[inline]
    pub fn get_segment_id(
        &self,
        mi_row: u32,
        mi_col: u32,
        mi_cols: u32,
        segment_map: &[AtomicU8],
    ) -> u8 {
        if !self.is_enabled() {
            return 0;
        }

        let idx = (mi_row * mi_cols + mi_col) as usize;
        if idx >= segment_map.len() {
            return 0;
        }

        segment_map[idx].load(Ordering::Acquire) & 7
    }

    /// Update segment ID for a macroblock position
    ///
    /// # Arguments
    /// * `mi_row` - Macroblock row index
    /// * `mi_col` - Macroblock column index
    /// * `segment_id` - New segment ID (0-7)
    /// * `mi_cols` - Total columns in frame
    /// * `segment_map` - External segment map array
    pub fn update_segment_map(
        &self,
        mi_row: u32,
        mi_col: u32,
        segment_id: u8,
        mi_cols: u32,
        segment_map: &[AtomicU8],
    ) -> Result<(), Vp9SegmentationError> {
        if segment_id >= VP9_MAX_SEGMENTS as u8 {
            self.last_error.store(Vp9SegmentationError::InvalidSegmentId as u32, Ordering::Release);
            return Err(Vp9SegmentationError::InvalidSegmentId);
        }

        let idx = (mi_row * mi_cols + mi_col) as usize;
        if idx >= segment_map.len() {
            return Err(Vp9SegmentationError::DataOverflow);
        }

        segment_map[idx].store(segment_id, Ordering::Release);

        // Mark segment as used
        self.segments_used.fetch_or(1 << segment_id, Ordering::AcqRel);
        self.map_updates.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }

    /// Batch update segment map for a region
    ///
    /// # Arguments
    /// * `start_row` - Starting macroblock row
    /// * `start_col` - Starting macroblock column
    /// * `rows` - Number of rows
    /// * `cols` - Number of columns
    /// * `segment_id` - Segment ID to set (0-7)
    /// * `mi_cols` - Total columns in frame
    /// * `segment_map` - External segment map array
    pub fn batch_update_segment_map(
        &self,
        start_row: u32,
        start_col: u32,
        rows: u32,
        cols: u32,
        segment_id: u8,
        mi_cols: u32,
        segment_map: &[AtomicU8],
    ) -> Result<u32, Vp9SegmentationError> {
        if segment_id >= VP9_MAX_SEGMENTS as u8 {
            self.last_error.store(Vp9SegmentationError::InvalidSegmentId as u32, Ordering::Release);
            return Err(Vp9SegmentationError::InvalidSegmentId);
        }

        let mut count = 0u32;

        for row in start_row..(start_row + rows) {
            for col in start_col..(start_col + cols) {
                let idx = (row * mi_cols + col) as usize;
                if idx < segment_map.len() {
                    segment_map[idx].store(segment_id, Ordering::Release);
                    count += 1;
                }
            }
        }

        // Mark segment as used
        self.segments_used.fetch_or(1 << segment_id, Ordering::AcqRel);
        self.map_updates.fetch_add(count as u64, Ordering::Relaxed);

        Ok(count)
    }

    // ========================================================================
    // Statistics
    // ========================================================================

    /// Get statistics snapshot
    pub fn stats(&self) -> Vp9SegmentationStats {
        let enables = self.feature_enables.load(Ordering::Acquire);
        let active = (0..VP9_MAX_SEGMENTS)
            .filter(|&seg| {
                let base = seg * VP9_SEG_LVL_MAX;
                (enables >> base) & 0xF != 0
            })
            .count() as u8;

        Vp9SegmentationStats {
            parse_count: self.parse_count.load(Ordering::Acquire),
            feature_lookups: self.feature_lookups.load(Ordering::Acquire),
            map_updates: self.map_updates.load(Ordering::Acquire),
            active_segments: active,
            features_enabled: enables.count_ones() as u8,
            generation: self.generation.load(Ordering::Acquire),
        }
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get segments in use bitmap
    #[inline]
    pub fn segments_used(&self) -> u8 {
        self.segments_used.load(Ordering::Acquire) as u8
    }
}

// ============================================================================
// Compile-Time Size Verification
// ============================================================================

const _: () = {
    // Verify capsule size is exactly 512 bytes
    assert!(core::mem::size_of::<Vp9SegmentationCapsule>() == 512);
    // Verify alignment is 512 bytes
    assert!(core::mem::align_of::<Vp9SegmentationCapsule>() == 512);
};

// ============================================================================
// Tests (T28 5-Tier Testing)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Q1-Q7: Unit Tests
    // ========================================================================

    #[test]
    fn q1_capsule_creation() {
        let capsule = Vp9SegmentationCapsule::new();
        assert!(!capsule.is_enabled());
        assert_eq!(capsule.generation(), 0);
    }

    #[test]
    fn q2_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<Vp9SegmentationCapsule>(), 512);
        assert_eq!(core::mem::align_of::<Vp9SegmentationCapsule>(), 512);
    }

    #[test]
    fn q3_seg_feature_enum() {
        assert_eq!(SegFeature::AltQ.index(), 0);
        assert_eq!(SegFeature::AltLf.index(), 1);
        assert_eq!(SegFeature::RefFrame.index(), 2);
        assert_eq!(SegFeature::Skip.index(), 3);

        assert_eq!(SegFeature::AltQ.data_bits(), 8);
        assert_eq!(SegFeature::AltLf.data_bits(), 6);
        assert_eq!(SegFeature::RefFrame.data_bits(), 2);
        assert_eq!(SegFeature::Skip.data_bits(), 0);

        assert!(SegFeature::AltQ.is_signed());
        assert!(SegFeature::AltLf.is_signed());
        assert!(!SegFeature::RefFrame.is_signed());
        assert!(!SegFeature::Skip.is_signed());
    }

    #[test]
    fn q4_feature_enable_disable() {
        let capsule = Vp9SegmentationCapsule::new();

        // Enable feature
        capsule.set_segment_feature_enabled(0, SegFeature::AltQ, true).unwrap();
        let enables = capsule.feature_enables.load(Ordering::Acquire);
        assert_eq!(enables & 1, 1);

        // Disable feature
        capsule.set_segment_feature_enabled(0, SegFeature::AltQ, false).unwrap();
        let enables = capsule.feature_enables.load(Ordering::Acquire);
        assert_eq!(enables & 1, 0);
    }

    #[test]
    fn q5_feature_data_read_write() {
        let capsule = Vp9SegmentationCapsule::new();

        // Write positive value
        capsule.set_segment_feature_data(0, SegFeature::AltQ, 42).unwrap();
        assert_eq!(capsule.segment_feature_data(0, SegFeature::AltQ), 42);

        // Write negative value
        capsule.set_segment_feature_data(0, SegFeature::AltQ, -10).unwrap();
        assert_eq!(capsule.segment_feature_data(0, SegFeature::AltQ), -10);

        // Write to different segment
        capsule.set_segment_feature_data(3, SegFeature::AltLf, -5).unwrap();
        assert_eq!(capsule.segment_feature_data(3, SegFeature::AltLf), -5);
    }

    #[test]
    fn q6_state_flags() {
        let capsule = Vp9SegmentationCapsule::new();

        // Initially disabled
        assert!(!capsule.is_enabled());
        assert!(!capsule.is_update_map());
        assert!(!capsule.is_update_data());

        // Set flags manually
        capsule.state.store(
            seg_flags::ENABLED | seg_flags::UPDATE_MAP | seg_flags::ABS_DELTA,
            Ordering::Release,
        );

        assert!(capsule.is_enabled());
        assert!(capsule.is_update_map());
        assert!(!capsule.is_update_data());
        assert!(capsule.is_abs_delta());
        assert!(!capsule.is_temporal_update());
    }

    #[test]
    fn q7_reset() {
        let capsule = Vp9SegmentationCapsule::new();

        // Set some state
        capsule.state.store(seg_flags::ENABLED, Ordering::Release);
        capsule.set_segment_feature_data(0, SegFeature::AltQ, 100).unwrap();
        capsule.set_segment_feature_enabled(0, SegFeature::AltQ, true).unwrap();

        let gen_before_reset = capsule.generation();

        // Reset
        capsule.reset();

        assert!(!capsule.is_enabled());
        assert_eq!(capsule.segment_feature_data(0, SegFeature::AltQ), 0);
        assert!(capsule.generation() > gen_before_reset); // Reset increments generation
    }

    // ========================================================================
    // Q8-Q14: Property Tests
    // ========================================================================

    #[test]
    fn q8_all_segments_addressable() {
        let capsule = Vp9SegmentationCapsule::new();

        for seg in 0..VP9_MAX_SEGMENTS {
            for feature in [SegFeature::AltQ, SegFeature::AltLf, SegFeature::RefFrame, SegFeature::Skip] {
                capsule.set_segment_feature_enabled(seg as u8, feature, true).unwrap();
                capsule.set_segment_feature_data(seg as u8, feature, (seg + 1) as i16).unwrap();
            }
        }

        // Verify all written correctly
        for seg in 0..VP9_MAX_SEGMENTS {
            for feature in [SegFeature::AltQ, SegFeature::AltLf, SegFeature::RefFrame, SegFeature::Skip] {
                let data = capsule.segment_feature_data(seg as u8, feature);
                assert_eq!(data, (seg + 1) as i16);
            }
        }
    }

    #[test]
    fn q9_invalid_segment_id_rejected() {
        let capsule = Vp9SegmentationCapsule::new();

        let result = capsule.set_segment_feature_enabled(8, SegFeature::AltQ, true);
        assert_eq!(result, Err(Vp9SegmentationError::InvalidSegmentId));

        let result = capsule.set_segment_feature_data(255, SegFeature::AltQ, 10);
        assert_eq!(result, Err(Vp9SegmentationError::InvalidSegmentId));
    }

    #[test]
    fn q10_feature_data_range() {
        let capsule = Vp9SegmentationCapsule::new();

        // Test extremes
        capsule.set_segment_feature_data(0, SegFeature::AltQ, i16::MAX).unwrap();
        assert_eq!(capsule.segment_feature_data(0, SegFeature::AltQ), i16::MAX);

        capsule.set_segment_feature_data(0, SegFeature::AltQ, i16::MIN).unwrap();
        assert_eq!(capsule.segment_feature_data(0, SegFeature::AltQ), i16::MIN);
    }

    #[test]
    fn q11_tree_probs_access() {
        let capsule = Vp9SegmentationCapsule::new();

        // Default should be all 255
        let probs = capsule.tree_probs();
        for prob in probs.iter() {
            assert_eq!(*prob, 255);
        }

        // Set individual prob
        capsule.set_tree_prob(0, 128).unwrap();
        capsule.set_tree_prob(3, 64).unwrap();

        let probs = capsule.tree_probs();
        assert_eq!(probs[0], 128);
        assert_eq!(probs[3], 64);
        assert_eq!(probs[6], 255); // Unchanged
    }

    #[test]
    fn q12_pred_probs_access() {
        let capsule = Vp9SegmentationCapsule::new();

        // Default should be all 255
        let probs = capsule.pred_probs();
        for prob in probs.iter() {
            assert_eq!(*prob, 255);
        }

        // Set individual prob
        capsule.set_pred_prob(0, 128).unwrap();
        capsule.set_pred_prob(2, 32).unwrap();

        let probs = capsule.pred_probs();
        assert_eq!(probs[0], 128);
        assert_eq!(probs[1], 255);
        assert_eq!(probs[2], 32);
    }

    #[test]
    fn q13_feature_isolation() {
        let capsule = Vp9SegmentationCapsule::new();

        // Set one feature
        capsule.set_segment_feature_data(0, SegFeature::AltQ, 100).unwrap();

        // Other features should be 0
        assert_eq!(capsule.segment_feature_data(0, SegFeature::AltLf), 0);
        assert_eq!(capsule.segment_feature_data(0, SegFeature::RefFrame), 0);
        assert_eq!(capsule.segment_feature_data(0, SegFeature::Skip), 0);

        // Other segments should be 0
        assert_eq!(capsule.segment_feature_data(1, SegFeature::AltQ), 0);
    }

    #[test]
    fn q14_generation_counter_increments() {
        let capsule = Vp9SegmentationCapsule::new();

        let gen0 = capsule.generation();

        capsule.set_segment_feature_data(0, SegFeature::AltQ, 1).unwrap();
        let gen1 = capsule.generation();
        assert!(gen1 > gen0);

        capsule.set_segment_feature_enabled(1, SegFeature::Skip, true).unwrap();
        let gen2 = capsule.generation();
        assert!(gen2 > gen1);

        capsule.reset();
        let gen3 = capsule.generation();
        assert!(gen3 > gen2);
    }

    // ========================================================================
    // Q15-Q21: Integration Tests
    // ========================================================================

    #[test]
    fn q15_parse_disabled_segmentation() {
        let capsule = Vp9SegmentationCapsule::new();

        // Single bit = 0 (disabled)
        let data = [0x00u8];
        let mut bool_dec = Vp9BoolDecoderCapsule::new(&data);

        capsule.parse_segmentation(&mut bool_dec).unwrap();

        assert!(!capsule.is_enabled());
    }

    #[test]
    fn q16_parse_enabled_no_updates() {
        let capsule = Vp9SegmentationCapsule::new();

        // enabled=1, update_map=0 (next 2 bits not read)
        // We need: bit 0 = 1 (enabled), bit 1 = 0 (no update_map), bit 2 = 0 (no update_data)
        // Binary: 100 = 0x80 in MSB-first
        let data = [0x80u8];
        let mut bool_dec = Vp9BoolDecoderCapsule::new(&data);

        capsule.parse_segmentation(&mut bool_dec).unwrap();

        assert!(capsule.is_enabled());
        assert!(!capsule.is_update_map());
    }

    #[test]
    fn q17_parse_with_update_map() {
        let capsule = Vp9SegmentationCapsule::new();

        // enabled=1, update_map=1, then 7 probs (each needs present bit)
        // We'll have present=0 for all probs (use defaults)
        // Then temporal_update=0, update_data=0
        // Bits: 1 1 0 0 0 0 0 0 0 0 0
        //       e um p0 p1 p2 p3 p4 p5 p6 tu ud
        // That's 0xC0 0x00
        let data = [0xC0u8, 0x00];
        let mut bool_dec = Vp9BoolDecoderCapsule::new(&data);

        capsule.parse_segmentation(&mut bool_dec).unwrap();

        assert!(capsule.is_enabled());
        assert!(capsule.is_update_map());
        assert!(!capsule.is_temporal_update());
        assert!(!capsule.is_update_data());
    }

    #[test]
    fn q18_segment_map_operations() {
        let capsule = Vp9SegmentationCapsule::new();
        capsule.state.store(seg_flags::ENABLED, Ordering::Release);

        // Create segment map for 4x4 frame
        let segment_map: Vec<AtomicU8> = (0..16).map(|_| AtomicU8::new(0)).collect();

        // Update some positions
        capsule.update_segment_map(0, 0, 1, 4, &segment_map).unwrap();
        capsule.update_segment_map(1, 1, 2, 4, &segment_map).unwrap();
        capsule.update_segment_map(2, 2, 3, 4, &segment_map).unwrap();

        // Verify
        assert_eq!(capsule.get_segment_id(0, 0, 4, &segment_map), 1);
        assert_eq!(capsule.get_segment_id(1, 1, 4, &segment_map), 2);
        assert_eq!(capsule.get_segment_id(2, 2, 4, &segment_map), 3);
        assert_eq!(capsule.get_segment_id(0, 1, 4, &segment_map), 0); // Unchanged

        // Check segments used bitmap
        assert_eq!(capsule.segments_used() & 0xE, 0xE); // Segments 1, 2, 3 used
    }

    #[test]
    fn q19_batch_segment_map_update() {
        let capsule = Vp9SegmentationCapsule::new();
        capsule.state.store(seg_flags::ENABLED, Ordering::Release);

        // Create segment map for 8x8 frame
        let segment_map: Vec<AtomicU8> = (0..64).map(|_| AtomicU8::new(0)).collect();

        // Batch update a 4x4 region
        let count = capsule.batch_update_segment_map(2, 2, 4, 4, 5, 8, &segment_map).unwrap();

        assert_eq!(count, 16);

        // Verify the region
        for row in 2..6 {
            for col in 2..6 {
                assert_eq!(capsule.get_segment_id(row, col, 8, &segment_map), 5);
            }
        }

        // Verify outside region unchanged
        assert_eq!(capsule.get_segment_id(0, 0, 8, &segment_map), 0);
        assert_eq!(capsule.get_segment_id(1, 1, 8, &segment_map), 0);
    }

    #[test]
    fn q20_stats_tracking() {
        let capsule = Vp9SegmentationCapsule::new();
        capsule.state.store(seg_flags::ENABLED, Ordering::Release);

        // Enable some features
        capsule.set_segment_feature_enabled(0, SegFeature::AltQ, true).unwrap();
        capsule.set_segment_feature_enabled(1, SegFeature::Skip, true).unwrap();

        // Do some lookups
        capsule.segment_feature_active(0, SegFeature::AltQ);
        capsule.segment_feature_data(0, SegFeature::AltQ);

        let stats = capsule.stats();
        assert!(stats.feature_lookups >= 2);
        assert_eq!(stats.features_enabled, 2);
        assert_eq!(stats.active_segments, 2);
    }

    #[test]
    fn q21_segment_feature_active_requires_enabled() {
        let capsule = Vp9SegmentationCapsule::new();

        // Enable feature but not segmentation
        capsule.set_segment_feature_enabled(0, SegFeature::AltQ, true).unwrap();

        // Should return false because segmentation disabled
        assert!(!capsule.segment_feature_active(0, SegFeature::AltQ));

        // Enable segmentation
        capsule.state.store(seg_flags::ENABLED, Ordering::Release);

        // Now should return true
        assert!(capsule.segment_feature_active(0, SegFeature::AltQ));
    }

    // ========================================================================
    // Q22-Q28: Production Tests
    // ========================================================================

    #[test]
    fn q22_concurrent_feature_updates() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(Vp9SegmentationCapsule::new());

        let handles: Vec<_> = (0..8)
            .map(|seg| {
                let c = Arc::clone(&capsule);
                thread::spawn(move || {
                    for i in 0..100 {
                        c.set_segment_feature_data(seg, SegFeature::AltQ, i).unwrap();
                        c.set_segment_feature_enabled(seg, SegFeature::AltQ, i % 2 == 0).unwrap();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // All operations completed without panic
        assert!(capsule.generation() > 0);
    }

    #[test]
    fn q23_concurrent_segment_map_updates() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(Vp9SegmentationCapsule::new());
        capsule.state.store(seg_flags::ENABLED, Ordering::Release);

        let segment_map: Arc<Vec<AtomicU8>> = Arc::new((0..1024).map(|_| AtomicU8::new(0)).collect());

        let handles: Vec<_> = (0..4)
            .map(|t| {
                let c = Arc::clone(&capsule);
                let sm = Arc::clone(&segment_map);
                thread::spawn(move || {
                    for i in 0..100 {
                        let row = ((t * 8) + (i % 8)) as u32;
                        let col = (i % 32) as u32;
                        let seg = (t % 8) as u8;
                        let _ = c.update_segment_map(row, col, seg, 32, &sm);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let stats = capsule.stats();
        assert!(stats.map_updates > 0);
    }

    #[test]
    fn q24_real_vp9_segmentation_pattern() {
        let capsule = Vp9SegmentationCapsule::new();
        capsule.state.store(seg_flags::ENABLED | seg_flags::UPDATE_DATA, Ordering::Release);

        // Typical VP9 segmentation for ROI encoding:
        // Segment 0: Background (Q+8)
        // Segment 1: Face/ROI (Q-16, higher quality)
        // Segment 2: Text regions (skip mode)

        capsule.set_segment_feature_enabled(0, SegFeature::AltQ, true).unwrap();
        capsule.set_segment_feature_data(0, SegFeature::AltQ, 8).unwrap();

        capsule.set_segment_feature_enabled(1, SegFeature::AltQ, true).unwrap();
        capsule.set_segment_feature_data(1, SegFeature::AltQ, -16).unwrap();

        capsule.set_segment_feature_enabled(2, SegFeature::Skip, true).unwrap();

        // Verify pattern
        assert!(capsule.segment_feature_active(0, SegFeature::AltQ));
        assert_eq!(capsule.segment_feature_data(0, SegFeature::AltQ), 8);

        assert!(capsule.segment_feature_active(1, SegFeature::AltQ));
        assert_eq!(capsule.segment_feature_data(1, SegFeature::AltQ), -16);

        assert!(capsule.segment_feature_active(2, SegFeature::Skip));
    }

    #[test]
    fn q25_loop_filter_segmentation() {
        let capsule = Vp9SegmentationCapsule::new();
        capsule.state.store(seg_flags::ENABLED | seg_flags::UPDATE_DATA, Ordering::Release);

        // Setup loop filter adjustments per segment
        for seg in 0..VP9_MAX_SEGMENTS {
            let lf_delta = (seg as i16 - 4) * 2; // Range: -8 to +6
            capsule.set_segment_feature_enabled(seg as u8, SegFeature::AltLf, true).unwrap();
            capsule.set_segment_feature_data(seg as u8, SegFeature::AltLf, lf_delta).unwrap();
        }

        // Verify all loop filter deltas
        for seg in 0..VP9_MAX_SEGMENTS {
            let expected = (seg as i16 - 4) * 2;
            assert_eq!(capsule.segment_feature_data(seg as u8, SegFeature::AltLf), expected);
        }
    }

    #[test]
    fn q26_reference_frame_constraint() {
        let capsule = Vp9SegmentationCapsule::new();
        capsule.state.store(seg_flags::ENABLED | seg_flags::UPDATE_DATA, Ordering::Release);

        // Constrain segment 0 to LAST frame only (ref_frame = 1)
        capsule.set_segment_feature_enabled(0, SegFeature::RefFrame, true).unwrap();
        capsule.set_segment_feature_data(0, SegFeature::RefFrame, 1).unwrap();

        // Constrain segment 1 to GOLDEN frame (ref_frame = 2)
        capsule.set_segment_feature_enabled(1, SegFeature::RefFrame, true).unwrap();
        capsule.set_segment_feature_data(1, SegFeature::RefFrame, 2).unwrap();

        assert!(capsule.segment_feature_active(0, SegFeature::RefFrame));
        assert_eq!(capsule.segment_feature_data(0, SegFeature::RefFrame), 1);

        assert!(capsule.segment_feature_active(1, SegFeature::RefFrame));
        assert_eq!(capsule.segment_feature_data(1, SegFeature::RefFrame), 2);
    }

    #[test]
    fn q27_abs_vs_delta_mode() {
        let capsule = Vp9SegmentationCapsule::new();

        // Delta mode (default)
        capsule.state.store(seg_flags::ENABLED | seg_flags::UPDATE_DATA, Ordering::Release);
        assert!(!capsule.is_abs_delta());

        // Absolute mode
        capsule.state.store(
            seg_flags::ENABLED | seg_flags::UPDATE_DATA | seg_flags::ABS_DELTA,
            Ordering::Release,
        );
        assert!(capsule.is_abs_delta());
    }

    #[test]
    fn q28_temporal_prediction_mode() {
        let capsule = Vp9SegmentationCapsule::new();

        capsule.state.store(
            seg_flags::ENABLED | seg_flags::UPDATE_MAP | seg_flags::TEMPORAL_UPDATE,
            Ordering::Release,
        );

        // Set prediction probs
        capsule.set_pred_prob(0, 200).unwrap();
        capsule.set_pred_prob(1, 180).unwrap();
        capsule.set_pred_prob(2, 160).unwrap();

        assert!(capsule.is_temporal_update());

        let probs = capsule.pred_probs();
        assert_eq!(probs[0], 200);
        assert_eq!(probs[1], 180);
        assert_eq!(probs[2], 160);
    }

    // ========================================================================
    // Q29-Q35: Determinism Tests (Additional)
    // ========================================================================

    #[test]
    fn q29_deterministic_initialization() {
        let cap1 = Vp9SegmentationCapsule::new();
        let cap2 = Vp9SegmentationCapsule::new();

        assert_eq!(cap1.is_enabled(), cap2.is_enabled());
        assert_eq!(cap1.tree_probs(), cap2.tree_probs());
        assert_eq!(cap1.pred_probs(), cap2.pred_probs());
    }

    #[test]
    fn q30_deterministic_feature_storage() {
        let cap1 = Vp9SegmentationCapsule::new();
        let cap2 = Vp9SegmentationCapsule::new();

        // Same operations on both
        for i in 0..8 {
            cap1.set_segment_feature_data(i, SegFeature::AltQ, (i as i16) * 10).unwrap();
            cap2.set_segment_feature_data(i, SegFeature::AltQ, (i as i16) * 10).unwrap();
        }

        // Verify identical results
        for i in 0..8 {
            assert_eq!(
                cap1.segment_feature_data(i, SegFeature::AltQ),
                cap2.segment_feature_data(i, SegFeature::AltQ)
            );
        }
    }

    #[test]
    fn q31_error_code_consistency() {
        let capsule = Vp9SegmentationCapsule::new();

        let r1 = capsule.set_segment_feature_data(8, SegFeature::AltQ, 0);
        let r2 = capsule.set_segment_feature_data(8, SegFeature::AltQ, 0);

        assert_eq!(r1, r2);
        assert_eq!(r1, Err(Vp9SegmentationError::InvalidSegmentId));
    }

    #[test]
    fn q32_prob_bounds() {
        let capsule = Vp9SegmentationCapsule::new();

        // Invalid tree prob index
        let result = capsule.set_tree_prob(7, 128);
        assert_eq!(result, Err(Vp9SegmentationError::InvalidFeature));

        // Invalid pred prob index
        let result = capsule.set_pred_prob(3, 128);
        assert_eq!(result, Err(Vp9SegmentationError::InvalidFeature));

        // Valid bounds
        assert!(capsule.set_tree_prob(6, 128).is_ok());
        assert!(capsule.set_pred_prob(2, 128).is_ok());
    }

    #[test]
    fn q33_from_index_roundtrip() {
        for i in 0..4 {
            let feature = SegFeature::from_index(i).unwrap();
            assert_eq!(feature.index(), i);
        }

        // Invalid index returns None
        assert!(SegFeature::from_index(4).is_none());
        assert!(SegFeature::from_index(255).is_none());
    }

    #[test]
    fn q34_audit_trail_generation() {
        let capsule = Vp9SegmentationCapsule::new();

        let initial_gen = capsule.generation();

        // Each modifying operation should increment generation
        capsule.set_segment_feature_data(0, SegFeature::AltQ, 1).unwrap();
        assert!(capsule.generation() > initial_gen);

        let gen1 = capsule.generation();
        capsule.set_segment_feature_enabled(1, SegFeature::Skip, true).unwrap();
        assert!(capsule.generation() > gen1);

        let gen2 = capsule.generation();
        capsule.reset();
        assert!(capsule.generation() > gen2);
    }

    #[test]
    fn q35_bool_decoder_mock() {
        // Test the mock bool decoder
        let data = [0b10101010, 0b11001100];
        let mut dec = Vp9BoolDecoderCapsule::new(&data);

        // Read bits MSB first
        assert_eq!(dec.read_bit().unwrap(), true);  // 1
        assert_eq!(dec.read_bit().unwrap(), false); // 0
        assert_eq!(dec.read_bit().unwrap(), true);  // 1
        assert_eq!(dec.read_bit().unwrap(), false); // 0

        // Read literal
        let val = dec.read_literal(4).unwrap();
        assert_eq!(val, 0b1010); // Rest of first byte
    }
}
