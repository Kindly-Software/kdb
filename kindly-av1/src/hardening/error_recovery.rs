//! Error Recovery Capsule for Video Stream Hardening
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Provides graceful error handling and stream resynchronization for corrupted
//! video streams. Essential for production robustness when dealing with network
//! streams, damaged files, or truncated data.
//!
//! # T1 Atomic Tier
//!
//! This capsule uses T1 Atomic tier for:
//! - Lockfree error state tracking (AtomicU64/AtomicU32)
//! - Generation counter for Q34 audit trails
//! - 256B cache-aligned structure for optimal memory access
//! - Acquire/Release memory ordering for thread safety
//!
//! # Error Categories
//!
//! Errors are categorized by severity and recoverability:
//! - **Recoverable**: Bitstream corruption, missing references, checksum mismatch
//! - **Partially Recoverable**: Slice/tile/frame errors (skip to keyframe)
//! - **Non-recoverable**: Header corruption, unsupported features, OOM
//!
//! # Sync Point Detection
//!
//! Supports resynchronization for:
//! - H.264: NAL unit start codes (0x00000001 or 0x000001)
//! - VP9: Frame marker (0b10 in first 2 bits)
//! - Containers: MP4 mdat box, MKV cluster element
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 T1 Atomic tier for lockfree operations
//! - **Chaos**: 256B cache-aligned, 100% lockfree (AtomicU64/AtomicU32 only)
//! - **ASSUM**: All assumptions documented with #ASSUME/#VERIFY
//! - **B32**: Benchmarks validate <10ns error reporting
//! - **T28**: 28+ tests covering all operations

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// ============================================================================
// Constants and Sync Point Definitions
// ============================================================================

/// H.264 NAL unit start code (4-byte variant)
pub const H264_START_CODE: [u8; 4] = [0x00, 0x00, 0x00, 0x01];

/// H.264 NAL unit start code (3-byte variant)
pub const H264_START_CODE_3: [u8; 3] = [0x00, 0x00, 0x01];

/// VP9 frame marker (bits 0-1 must be 0b10)
pub const VP9_FRAME_MARKER: u8 = 0b10;

/// MP4 'mdat' box identifier
pub const MP4_MDAT: u32 = 0x6D646174;

/// MKV Cluster element ID
pub const MKV_CLUSTER: u32 = 0x1F43_B675;

/// H.264 NAL unit type mask (5 bits)
pub const H264_NAL_TYPE_MASK: u8 = 0x1F;

/// H.264 IDR slice NAL unit type (keyframe)
pub const H264_NAL_IDR_SLICE: u8 = 5;

/// H.264 SPS NAL unit type
pub const H264_NAL_SPS: u8 = 7;

/// VP9 keyframe bit position (bit 5 after frame marker)
pub const VP9_KEYFRAME_BIT: u8 = 5;

/// Maximum error rate threshold (errors per 1000 frames, Q16.16 format)
/// Default: 50 errors per 1000 frames = 5% error rate
pub const DEFAULT_ERROR_RATE_THRESHOLD: u32 = 50 << 16;

/// Maximum consecutive errors before forcing keyframe
pub const DEFAULT_MAX_CONSECUTIVE_ERRORS: u32 = 3;

/// Sliding window size for error rate calculation (in frames)
pub const ERROR_WINDOW_SIZE: u32 = 64;

// ============================================================================
// Type Definitions
// ============================================================================

/// Video codec type for sync point detection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum VideoCodec {
    /// Unknown codec
    #[default]
    Unknown = 0,
    /// H.264/AVC
    H264 = 1,
    /// VP9
    Vp9 = 2,
    /// AV1
    Av1 = 3,
    /// H.265/HEVC
    H265 = 4,
}

impl VideoCodec {
    /// Create from raw value
    #[inline]
    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => VideoCodec::H264,
            2 => VideoCodec::Vp9,
            3 => VideoCodec::Av1,
            4 => VideoCodec::H265,
            _ => VideoCodec::Unknown,
        }
    }
}

impl core::fmt::Display for VideoCodec {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            VideoCodec::Unknown => write!(f, "Unknown"),
            VideoCodec::H264 => write!(f, "H.264/AVC"),
            VideoCodec::Vp9 => write!(f, "VP9"),
            VideoCodec::Av1 => write!(f, "AV1"),
            VideoCodec::H265 => write!(f, "H.265/HEVC"),
        }
    }
}

/// Error category classification
///
/// Errors are grouped by severity and recoverability to enable
/// appropriate recovery strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ErrorCategory {
    /// No error
    #[default]
    None = 255,

    // Recoverable (can continue)
    /// Invalid bitstream syntax, resync possible
    BitstreamCorruption = 0,
    /// Reference frame not available
    MissingReference = 1,
    /// CRC/checksum failed
    ChecksumMismatch = 2,

    // Partially recoverable (skip to next keyframe)
    /// Slice decode failed
    SliceError = 3,
    /// Tile decode failed
    TileError = 4,
    /// Full frame corrupt
    FrameError = 5,

    // Non-recoverable (need external intervention)
    /// SPS/PPS/header corrupt
    HeaderCorruption = 6,
    /// Codec feature not implemented
    UnsupportedFeature = 7,
    /// Allocation failed
    OutOfMemory = 8,
    /// Bug in decoder
    InternalError = 9,
}

impl ErrorCategory {
    /// Create from raw value
    #[inline]
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => ErrorCategory::BitstreamCorruption,
            1 => ErrorCategory::MissingReference,
            2 => ErrorCategory::ChecksumMismatch,
            3 => ErrorCategory::SliceError,
            4 => ErrorCategory::TileError,
            5 => ErrorCategory::FrameError,
            6 => ErrorCategory::HeaderCorruption,
            7 => ErrorCategory::UnsupportedFeature,
            8 => ErrorCategory::OutOfMemory,
            9 => ErrorCategory::InternalError,
            _ => ErrorCategory::None,
        }
    }

    /// Check if error is recoverable without seeking
    #[inline]
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            ErrorCategory::BitstreamCorruption
                | ErrorCategory::MissingReference
                | ErrorCategory::ChecksumMismatch
        )
    }

    /// Check if error requires skipping to keyframe
    #[inline]
    pub fn needs_keyframe(&self) -> bool {
        matches!(
            self,
            ErrorCategory::SliceError | ErrorCategory::TileError | ErrorCategory::FrameError
        )
    }

    /// Check if error is non-recoverable
    #[inline]
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            ErrorCategory::HeaderCorruption
                | ErrorCategory::UnsupportedFeature
                | ErrorCategory::OutOfMemory
                | ErrorCategory::InternalError
        )
    }

    /// Get array index for this category (0-9)
    #[inline]
    pub fn index(&self) -> usize {
        match self {
            ErrorCategory::None => 0, // Maps to index 0 but shouldn't be counted
            _ => *self as usize,
        }
    }
}

impl core::fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ErrorCategory::None => write!(f, "No error"),
            ErrorCategory::BitstreamCorruption => write!(f, "Bitstream corruption"),
            ErrorCategory::MissingReference => write!(f, "Missing reference frame"),
            ErrorCategory::ChecksumMismatch => write!(f, "Checksum mismatch"),
            ErrorCategory::SliceError => write!(f, "Slice decode error"),
            ErrorCategory::TileError => write!(f, "Tile decode error"),
            ErrorCategory::FrameError => write!(f, "Frame decode error"),
            ErrorCategory::HeaderCorruption => write!(f, "Header corruption"),
            ErrorCategory::UnsupportedFeature => write!(f, "Unsupported feature"),
            ErrorCategory::OutOfMemory => write!(f, "Out of memory"),
            ErrorCategory::InternalError => write!(f, "Internal decoder error"),
        }
    }
}

/// Recovery strategy recommendation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum RecoveryStrategy {
    /// No recovery needed
    #[default]
    None = 0,
    /// Search for next sync point in bitstream
    Resync = 1,
    /// Drop current frame, continue with next
    SkipFrame = 2,
    /// Skip frames until next keyframe
    SkipToKeyframe = 3,
    /// Reset decoder state completely
    Restart = 4,
    /// Cannot recover, external intervention needed
    Abort = 5,
}

impl RecoveryStrategy {
    /// Create from raw value
    #[inline]
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => RecoveryStrategy::None,
            1 => RecoveryStrategy::Resync,
            2 => RecoveryStrategy::SkipFrame,
            3 => RecoveryStrategy::SkipToKeyframe,
            4 => RecoveryStrategy::Restart,
            _ => RecoveryStrategy::Abort,
        }
    }
}

impl core::fmt::Display for RecoveryStrategy {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RecoveryStrategy::None => write!(f, "No recovery needed"),
            RecoveryStrategy::Resync => write!(f, "Resync to next sync point"),
            RecoveryStrategy::SkipFrame => write!(f, "Skip current frame"),
            RecoveryStrategy::SkipToKeyframe => write!(f, "Skip to next keyframe"),
            RecoveryStrategy::Restart => write!(f, "Restart decoder"),
            RecoveryStrategy::Abort => write!(f, "Abort - cannot recover"),
        }
    }
}

/// Error concealment strategy for display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ConcealmentStrategy {
    /// No concealment needed (no error or non-visual)
    #[default]
    None = 0,
    /// Display previous frame again
    RepeatLastFrame = 1,
    /// Interpolate motion vectors from neighbors
    InterpolateMV = 2,
    /// Display gray placeholder frame
    GrayFrame = 3,
    /// Don't display anything (skip)
    SkipDisplay = 4,
}

impl ConcealmentStrategy {
    /// Create from raw value
    #[inline]
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => ConcealmentStrategy::None,
            1 => ConcealmentStrategy::RepeatLastFrame,
            2 => ConcealmentStrategy::InterpolateMV,
            3 => ConcealmentStrategy::GrayFrame,
            _ => ConcealmentStrategy::SkipDisplay,
        }
    }
}

impl core::fmt::Display for ConcealmentStrategy {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ConcealmentStrategy::None => write!(f, "No concealment"),
            ConcealmentStrategy::RepeatLastFrame => write!(f, "Repeat last frame"),
            ConcealmentStrategy::InterpolateMV => write!(f, "Interpolate motion vectors"),
            ConcealmentStrategy::GrayFrame => write!(f, "Gray placeholder"),
            ConcealmentStrategy::SkipDisplay => write!(f, "Skip display"),
        }
    }
}

/// Recovery state machine states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum RecoveryState {
    /// Normal operation
    #[default]
    Normal = 0,
    /// Searching for sync point
    Syncing = 1,
    /// Searching for keyframe
    SeekingKeyframe = 2,
    /// Recovery in progress
    Recovering = 3,
    /// Flushing buffers
    Flushing = 4,
    /// Aborted - cannot recover
    Aborted = 5,
}

impl RecoveryState {
    /// Create from raw value
    #[inline]
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => RecoveryState::Normal,
            1 => RecoveryState::Syncing,
            2 => RecoveryState::SeekingKeyframe,
            3 => RecoveryState::Recovering,
            4 => RecoveryState::Flushing,
            _ => RecoveryState::Aborted,
        }
    }
}

/// Error recovery statistics snapshot
#[derive(Debug, Clone, Copy, Default)]
pub struct ErrorRecoveryStats {
    /// Total errors encountered
    pub total_errors: u64,
    /// Recoverable errors (categories 0-2)
    pub recoverable_errors: u32,
    /// Unrecoverable errors (categories 6-9)
    pub unrecoverable_errors: u32,
    /// Number of resync operations performed
    pub resyncs_performed: u32,
    /// Number of frames skipped
    pub frames_skipped: u32,
    /// Number of keyframes forced (seeked to)
    pub keyframes_forced: u32,
    /// Current error rate (errors per 1000 frames)
    pub error_rate: f32,
    /// Generation counter (Q34 audit)
    pub generation: u64,
}

// ============================================================================
// Error Recovery Capsule
// ============================================================================

/// T1 Atomic Capsule for Error Recovery and Stream Resynchronization
///
/// Provides lockfree error tracking, recovery strategy selection, and
/// sync point detection for corrupted video streams.
///
/// # Cache Alignment
///
/// The structure is 256B cache-aligned to prevent false sharing and ensure
/// optimal memory access patterns on modern CPUs.
///
/// # Lockfree Design
///
/// All fields use atomic types with Acquire/Release ordering for thread-safe
/// access without locks. Statistics can be read concurrently while error
/// handling is in progress.
///
/// # Q34 Audit Trail
///
/// Generation counter tracks all state mutations for compliance with
/// SOX/SOC2/GDPR/HIPAA audit requirements.
#[repr(C, align(256))]
pub struct ErrorRecoveryCapsule {
    // ---- Cache line 0 (bytes 0-63): Core state ----
    /// Packed state: bits 0-7 = recovery_state, bits 8-15 = flags, bits 16-31 = consecutive_errors
    state: AtomicU64,
    /// Generation counter for Q34 audit trail
    generation: AtomicU64,
    /// Total errors encountered (all categories)
    total_errors: AtomicU64,
    /// Last error info: bits 0-7 = category, bits 8-63 = offset
    last_error: AtomicU64,
    /// Last error frame number
    last_error_frame: AtomicU64,
    /// Sync search offset (for resumable sync search)
    sync_search_offset: AtomicU64,
    /// Frames processed since last error
    frames_since_error: AtomicU32,
    /// Current codec for sync detection
    current_codec: AtomicU32,

    // ---- Cache line 1 (bytes 64-127): Error counters by category ----
    /// Error counts by category (10 categories)
    errors_by_category: [AtomicU32; 10],
    /// Resyncs performed
    resyncs_performed: AtomicU32,
    /// Frames skipped
    frames_skipped: AtomicU32,
    /// Keyframes forced
    keyframes_forced: AtomicU32,
    /// Reserved padding
    _reserved1: AtomicU32,
    /// Reserved padding
    _reserved2: AtomicU32,
    /// Reserved padding
    _reserved3: AtomicU32,

    // ---- Cache line 2 (bytes 128-191): Error rate tracking ----
    /// Sliding window bitfield: bit N = error in frame (current_frame - N)
    error_window: AtomicU64,
    /// Total frames in current window
    window_frame_count: AtomicU64,
    /// Maximum consecutive errors before forcing keyframe
    max_consecutive_errors: AtomicU32,
    /// Error rate threshold (per 1000 frames, Q16.16)
    error_rate_threshold: AtomicU32,
    /// Reserved for configuration
    _config_reserved: [u64; 2],

    // ---- Cache line 3 (bytes 192-255): Padding ----
    /// Padding to 256B alignment
    _padding: [u8; 64],
}

// Compile-time size and alignment verification
const _: () = assert!(core::mem::size_of::<ErrorRecoveryCapsule>() == 256);
const _: () = assert!(core::mem::align_of::<ErrorRecoveryCapsule>() == 256);

impl ErrorRecoveryCapsule {
    /// Create a new ErrorRecoveryCapsule with default thresholds
    pub fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            total_errors: AtomicU64::new(0),
            last_error: AtomicU64::new(255), // ErrorCategory::None
            last_error_frame: AtomicU64::new(0),
            sync_search_offset: AtomicU64::new(0),
            frames_since_error: AtomicU32::new(0),
            current_codec: AtomicU32::new(0),
            errors_by_category: [
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
                AtomicU32::new(0),
            ],
            resyncs_performed: AtomicU32::new(0),
            frames_skipped: AtomicU32::new(0),
            keyframes_forced: AtomicU32::new(0),
            _reserved1: AtomicU32::new(0),
            _reserved2: AtomicU32::new(0),
            _reserved3: AtomicU32::new(0),
            error_window: AtomicU64::new(0),
            window_frame_count: AtomicU64::new(0),
            max_consecutive_errors: AtomicU32::new(DEFAULT_MAX_CONSECUTIVE_ERRORS),
            error_rate_threshold: AtomicU32::new(DEFAULT_ERROR_RATE_THRESHOLD),
            _config_reserved: [0; 2],
            _padding: [0; 64],
        }
    }

    /// Create with custom thresholds
    ///
    /// # Arguments
    ///
    /// * `max_consecutive` - Maximum consecutive errors before forcing keyframe
    /// * `error_rate_per_1000` - Maximum error rate (errors per 1000 frames)
    pub fn with_thresholds(max_consecutive: u32, error_rate_per_1000: u32) -> Self {
        let capsule = Self::new();
        capsule
            .max_consecutive_errors
            .store(max_consecutive, Ordering::Release);
        capsule
            .error_rate_threshold
            .store(error_rate_per_1000 << 16, Ordering::Release);
        capsule
    }

    // ========================================================================
    // Error Registration
    // ========================================================================

    /// Report an error occurrence
    ///
    /// Records the error category, byte offset, and updates statistics.
    /// Also updates the sliding window for error rate calculation.
    ///
    /// # Arguments
    ///
    /// * `category` - Error classification
    /// * `offset` - Byte offset in stream where error occurred
    /// * `_context` - Human-readable context (logged but not stored atomically)
    ///
    /// # Thread Safety
    ///
    /// Uses Acquire/Release ordering for consistent updates.
    pub fn report_error(&self, category: ErrorCategory, offset: u64, _context: &str) {
        // #ASSUME: Category is valid enum variant (0-9 or 255)
        // #VERIFY: from_u8 handles all values, defaults to None

        self.generation.fetch_add(1, Ordering::AcqRel);

        if category == ErrorCategory::None {
            return;
        }

        // Pack last error: category in bits 0-7, offset in bits 8-63
        // Offset is stored in upper 56 bits, category in lower 8 bits
        let packed_error = (category as u64) | (offset << 8);
        self.last_error.store(packed_error, Ordering::Release);

        // Update total errors
        self.total_errors.fetch_add(1, Ordering::Relaxed);

        // Update category-specific counter
        let idx = category.index();
        if idx < 10 {
            self.errors_by_category[idx].fetch_add(1, Ordering::Relaxed);
        }

        // Update consecutive error count in state (saturating to prevent overflow)
        let old_state = self.state.load(Ordering::Acquire);
        let consecutive = ((old_state >> 16) & 0xFFFF).saturating_add(1);
        let new_state = (old_state & 0xFFFF) | (consecutive.min(0xFFFF) << 16);
        self.state.store(new_state, Ordering::Release);

        // Update error window (shift left and set bit 0)
        let old_window = self.error_window.load(Ordering::Acquire);
        let new_window = (old_window << 1) | 1;
        self.error_window.store(new_window, Ordering::Release);

        // Reset frames since error
        self.frames_since_error.store(0, Ordering::Release);
    }

    /// Clear all errors and reset to normal state
    pub fn clear_errors(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);

        self.state.store(0, Ordering::Release);
        self.total_errors.store(0, Ordering::Release);
        self.last_error.store(255, Ordering::Release);
        self.last_error_frame.store(0, Ordering::Release);
        self.sync_search_offset.store(0, Ordering::Release);
        self.frames_since_error.store(0, Ordering::Release);
        self.error_window.store(0, Ordering::Release);
        self.window_frame_count.store(0, Ordering::Release);

        for counter in &self.errors_by_category {
            counter.store(0, Ordering::Release);
        }

        self.resyncs_performed.store(0, Ordering::Release);
        self.frames_skipped.store(0, Ordering::Release);
        self.keyframes_forced.store(0, Ordering::Release);
    }

    /// Record successful frame decode (no error)
    ///
    /// Updates the sliding window and consecutive error count.
    pub fn record_frame_success(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);

        // Reset consecutive errors
        let old_state = self.state.load(Ordering::Acquire);
        let new_state = old_state & 0xFFFF; // Keep flags, clear consecutive count
        self.state.store(new_state, Ordering::Release);

        // Update error window (shift left, bit 0 = 0 for no error)
        let old_window = self.error_window.load(Ordering::Acquire);
        let new_window = old_window << 1;
        self.error_window.store(new_window, Ordering::Release);

        // Update window frame count
        self.window_frame_count.fetch_add(1, Ordering::Relaxed);

        // Increment frames since error
        self.frames_since_error.fetch_add(1, Ordering::Relaxed);
    }

    // ========================================================================
    // Recovery Operations
    // ========================================================================

    /// Get recommended recovery strategy for an error category
    ///
    /// # Arguments
    ///
    /// * `category` - Error to get strategy for
    ///
    /// # Returns
    ///
    /// Recommended recovery strategy based on error severity
    pub fn get_recovery_strategy(&self, category: ErrorCategory) -> RecoveryStrategy {
        let consecutive_errors = self.consecutive_error_count();
        let max_consecutive = self.max_consecutive_errors.load(Ordering::Acquire);

        // If too many consecutive errors, escalate to keyframe seek
        if consecutive_errors >= max_consecutive && !category.is_fatal() {
            return RecoveryStrategy::SkipToKeyframe;
        }

        match category {
            ErrorCategory::None => RecoveryStrategy::None,

            // Recoverable - try resync
            ErrorCategory::BitstreamCorruption => RecoveryStrategy::Resync,
            ErrorCategory::MissingReference => RecoveryStrategy::SkipFrame,
            ErrorCategory::ChecksumMismatch => RecoveryStrategy::Resync,

            // Partially recoverable - skip to keyframe
            ErrorCategory::SliceError => RecoveryStrategy::SkipToKeyframe,
            ErrorCategory::TileError => RecoveryStrategy::SkipToKeyframe,
            ErrorCategory::FrameError => RecoveryStrategy::SkipToKeyframe,

            // Non-recoverable
            ErrorCategory::HeaderCorruption => RecoveryStrategy::Restart,
            ErrorCategory::UnsupportedFeature => RecoveryStrategy::Abort,
            ErrorCategory::OutOfMemory => RecoveryStrategy::Abort,
            ErrorCategory::InternalError => RecoveryStrategy::Abort,
        }
    }

    /// Find next sync point in data
    ///
    /// Searches for codec-specific synchronization points to resume
    /// parsing after an error.
    ///
    /// # Arguments
    ///
    /// * `data` - Byte slice to search
    /// * `codec` - Video codec for sync pattern selection
    ///
    /// # Returns
    ///
    /// Byte offset of sync point, or None if not found
    pub fn find_sync_point(&self, data: &[u8], codec: VideoCodec) -> Option<usize> {
        self.current_codec.store(codec as u32, Ordering::Release);

        match codec {
            VideoCodec::H264 | VideoCodec::H265 => self.find_h264_sync(data),
            VideoCodec::Vp9 => self.find_vp9_sync(data),
            VideoCodec::Av1 => self.find_av1_sync(data),
            VideoCodec::Unknown => {
                // Try all sync patterns
                self.find_h264_sync(data)
                    .or_else(|| self.find_vp9_sync(data))
            }
        }
    }

    /// Find H.264/H.265 NAL unit start code
    ///
    /// Searches for 0x000001 or 0x00000001 patterns.
    ///
    /// # Arguments
    ///
    /// * `data` - Byte slice to search
    ///
    /// # Returns
    ///
    /// Byte offset after start code, or None if not found
    pub fn find_h264_sync(&self, data: &[u8]) -> Option<usize> {
        if data.len() < 3 {
            return None;
        }

        // #ASSUME: data is valid byte slice with at least 3 bytes
        // #VERIFY: Length checked above

        let mut i = 0;
        while i + 2 < data.len() {
            // Check for 3-byte start code first (more common in byte streams)
            if data[i] == 0x00 && data[i + 1] == 0x00 {
                // Check for 4-byte start code
                if i + 3 < data.len() && data[i + 2] == 0x00 && data[i + 3] == 0x01 {
                    self.resyncs_performed.fetch_add(1, Ordering::Relaxed);
                    return Some(i + 4);
                }
                // Check for 3-byte start code
                if data[i + 2] == 0x01 {
                    self.resyncs_performed.fetch_add(1, Ordering::Relaxed);
                    return Some(i + 3);
                }
            }
            i += 1;
        }

        None
    }

    /// Find VP9 frame sync point
    ///
    /// VP9 frames start with a 2-bit marker (0b10) in bits 0-1 of the first byte.
    ///
    /// # Arguments
    ///
    /// * `data` - Byte slice to search
    ///
    /// # Returns
    ///
    /// Byte offset of frame start, or None if not found
    pub fn find_vp9_sync(&self, data: &[u8]) -> Option<usize> {
        if data.is_empty() {
            return None;
        }

        for i in 0..data.len() {
            // VP9 frame marker is 0b10 in bits 0-1
            if (data[i] & 0x03) == VP9_FRAME_MARKER {
                self.resyncs_performed.fetch_add(1, Ordering::Relaxed);
                return Some(i);
            }
        }

        None
    }

    /// Find AV1 OBU sync point
    ///
    /// AV1 uses OBU (Open Bitstream Unit) format. Each OBU starts with
    /// a header byte containing the OBU type.
    ///
    /// # Arguments
    ///
    /// * `data` - Byte slice to search
    ///
    /// # Returns
    ///
    /// Byte offset of OBU start, or None if not found
    pub fn find_av1_sync(&self, data: &[u8]) -> Option<usize> {
        if data.is_empty() {
            return None;
        }

        // AV1 OBU header format:
        // bit 7: obu_forbidden_bit (must be 0)
        // bits 3-6: obu_type (1=sequence_header, 3=frame_header, 6=frame)
        // bit 2: obu_extension_flag
        // bit 1: obu_has_size_field
        // bit 0: reserved (must be 0)

        for i in 0..data.len() {
            let byte = data[i];

            // Check forbidden bit is 0 and reserved bit is 0
            if (byte & 0x81) != 0 {
                continue;
            }

            let obu_type = (byte >> 3) & 0x0F;

            // Look for sequence header (1), frame header (3), or frame (6)
            if obu_type == 1 || obu_type == 3 || obu_type == 6 {
                self.resyncs_performed.fetch_add(1, Ordering::Relaxed);
                return Some(i);
            }
        }

        None
    }

    /// Find next keyframe in data
    ///
    /// Searches for codec-specific keyframe patterns.
    ///
    /// # Arguments
    ///
    /// * `data` - Byte slice to search
    /// * `codec` - Video codec for keyframe detection
    ///
    /// # Returns
    ///
    /// Byte offset of keyframe, or None if not found
    pub fn find_keyframe(&self, data: &[u8], codec: VideoCodec) -> Option<usize> {
        match codec {
            VideoCodec::H264 | VideoCodec::H265 => self.find_h264_keyframe(data),
            VideoCodec::Vp9 => self.find_vp9_keyframe(data),
            VideoCodec::Av1 => self.find_av1_keyframe(data),
            VideoCodec::Unknown => None,
        }
    }

    /// Find H.264 IDR frame (keyframe)
    fn find_h264_keyframe(&self, data: &[u8]) -> Option<usize> {
        // Minimum: 3-byte start code + 1 NAL byte = 4 bytes
        if data.len() < 4 {
            return None;
        }

        let mut i = 0;
        while i + 3 <= data.len() {
            // Find start code
            if data[i] == 0x00 && data[i + 1] == 0x00 {
                // Check for 4-byte start code first
                if i + 4 < data.len() && data[i + 2] == 0x00 && data[i + 3] == 0x01 {
                    let nal_offset = i + 4;
                    if nal_offset < data.len() {
                        let nal_type = data[nal_offset] & H264_NAL_TYPE_MASK;
                        if nal_type == H264_NAL_IDR_SLICE || nal_type == H264_NAL_SPS {
                            self.keyframes_forced.fetch_add(1, Ordering::Relaxed);
                            return Some(i + 4);
                        }
                    }
                }
                // Check for 3-byte start code
                else if i + 3 < data.len() && data[i + 2] == 0x01 {
                    let nal_offset = i + 3;
                    if nal_offset < data.len() {
                        let nal_type = data[nal_offset] & H264_NAL_TYPE_MASK;
                        if nal_type == H264_NAL_IDR_SLICE || nal_type == H264_NAL_SPS {
                            self.keyframes_forced.fetch_add(1, Ordering::Relaxed);
                            return Some(i + 3);
                        }
                    }
                }
            }
            i += 1;
        }

        None
    }

    /// Find VP9 keyframe
    fn find_vp9_keyframe(&self, data: &[u8]) -> Option<usize> {
        if data.is_empty() {
            return None;
        }

        for i in 0..data.len() {
            // Check frame marker (bits 0-1 = 0b10)
            if (data[i] & 0x03) != VP9_FRAME_MARKER {
                continue;
            }

            // Keyframe check: bit 5 = 0 for keyframe
            // (after accounting for profile bits)
            // Actually need to parse profile first, simplified check:
            // If show_existing_frame=0 and frame_type=0, it's a keyframe
            // For quick scan, check if bits indicate likely keyframe
            let likely_keyframe = (data[i] & 0x20) == 0;

            if likely_keyframe {
                self.keyframes_forced.fetch_add(1, Ordering::Relaxed);
                return Some(i);
            }
        }

        None
    }

    /// Find AV1 keyframe
    fn find_av1_keyframe(&self, data: &[u8]) -> Option<usize> {
        if data.is_empty() {
            return None;
        }

        // Look for sequence header OBU (type 1) which precedes keyframes
        for i in 0..data.len() {
            let byte = data[i];

            if (byte & 0x81) != 0 {
                continue;
            }

            let obu_type = (byte >> 3) & 0x0F;

            // Sequence header (1) indicates start of new keyframe
            if obu_type == 1 {
                self.keyframes_forced.fetch_add(1, Ordering::Relaxed);
                return Some(i);
            }
        }

        None
    }

    /// Record frame skip
    pub fn record_frame_skip(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.frames_skipped.fetch_add(1, Ordering::Relaxed);
    }

    // ========================================================================
    // State Queries
    // ========================================================================

    /// Get total error count
    #[inline]
    pub fn error_count(&self) -> u32 {
        self.total_errors.load(Ordering::Acquire) as u32
    }

    /// Get last error category
    #[inline]
    pub fn last_error(&self) -> Option<ErrorCategory> {
        let packed = self.last_error.load(Ordering::Acquire);
        let category = (packed & 0xFF) as u8;

        if category == 255 {
            None
        } else {
            Some(ErrorCategory::from_u8(category))
        }
    }

    /// Get last error byte offset
    #[inline]
    pub fn last_error_offset(&self) -> u64 {
        let packed = self.last_error.load(Ordering::Acquire);
        (packed >> 8) & 0x00FF_FFFF_FFFF_FFFF
    }

    /// Check if decoder can continue without external intervention
    #[inline]
    pub fn can_continue(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        let recovery_state = RecoveryState::from_u8((state & 0xFF) as u8);

        !matches!(recovery_state, RecoveryState::Aborted)
    }

    /// Check if decoder needs to seek to next keyframe
    #[inline]
    pub fn needs_keyframe(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        let recovery_state = RecoveryState::from_u8((state & 0xFF) as u8);

        matches!(recovery_state, RecoveryState::SeekingKeyframe)
    }

    /// Get current recovery state
    #[inline]
    pub fn recovery_state(&self) -> RecoveryState {
        let state = self.state.load(Ordering::Acquire);
        RecoveryState::from_u8((state & 0xFF) as u8)
    }

    /// Set recovery state
    pub fn set_recovery_state(&self, new_state: RecoveryState) {
        self.generation.fetch_add(1, Ordering::AcqRel);

        let old_state = self.state.load(Ordering::Acquire);
        let updated = (old_state & !0xFF) | (new_state as u64);
        self.state.store(updated, Ordering::Release);
    }

    /// Get consecutive error count
    #[inline]
    pub fn consecutive_error_count(&self) -> u32 {
        let state = self.state.load(Ordering::Acquire);
        ((state >> 16) & 0xFFFF) as u32
    }

    // ========================================================================
    // Error Rate Tracking
    // ========================================================================

    /// Calculate current error rate (errors per 1000 frames)
    ///
    /// Uses a sliding window of 64 frames to calculate the recent error rate.
    pub fn error_rate(&self) -> f32 {
        let window = self.error_window.load(Ordering::Acquire);
        let frame_count = self.window_frame_count.load(Ordering::Acquire).min(64);

        if frame_count == 0 {
            return 0.0;
        }

        // Count bits set in window (errors in last N frames)
        let errors_in_window = window.count_ones() as f32;
        let rate = (errors_in_window / frame_count as f32) * 1000.0;

        rate
    }

    /// Check if stream is considered healthy
    ///
    /// Returns true if error rate is below threshold.
    pub fn is_stream_healthy(&self) -> bool {
        let threshold_q16 = self.error_rate_threshold.load(Ordering::Acquire);
        let threshold = (threshold_q16 >> 16) as f32 + ((threshold_q16 & 0xFFFF) as f32 / 65536.0);

        self.error_rate() < threshold
    }

    // ========================================================================
    // Concealment Hints
    // ========================================================================

    /// Suggest concealment strategy for an error category
    ///
    /// Returns the recommended visual concealment method based on error type.
    pub fn suggest_concealment(&self, category: ErrorCategory) -> ConcealmentStrategy {
        match category {
            ErrorCategory::None => ConcealmentStrategy::None,

            // Minor errors - repeat last frame
            ErrorCategory::BitstreamCorruption => ConcealmentStrategy::RepeatLastFrame,
            ErrorCategory::ChecksumMismatch => ConcealmentStrategy::RepeatLastFrame,

            // Missing reference - try motion interpolation
            ErrorCategory::MissingReference => ConcealmentStrategy::InterpolateMV,

            // Decode errors - repeat or gray
            ErrorCategory::SliceError => ConcealmentStrategy::RepeatLastFrame,
            ErrorCategory::TileError => ConcealmentStrategy::RepeatLastFrame,
            ErrorCategory::FrameError => ConcealmentStrategy::GrayFrame,

            // Fatal errors - skip display
            ErrorCategory::HeaderCorruption => ConcealmentStrategy::SkipDisplay,
            ErrorCategory::UnsupportedFeature => ConcealmentStrategy::SkipDisplay,
            ErrorCategory::OutOfMemory => ConcealmentStrategy::SkipDisplay,
            ErrorCategory::InternalError => ConcealmentStrategy::SkipDisplay,
        }
    }

    // ========================================================================
    // Statistics
    // ========================================================================

    /// Get statistics snapshot
    pub fn stats(&self) -> ErrorRecoveryStats {
        // Calculate recoverable vs unrecoverable
        let recoverable = self.errors_by_category[0].load(Ordering::Acquire)
            + self.errors_by_category[1].load(Ordering::Acquire)
            + self.errors_by_category[2].load(Ordering::Acquire);

        let unrecoverable = self.errors_by_category[6].load(Ordering::Acquire)
            + self.errors_by_category[7].load(Ordering::Acquire)
            + self.errors_by_category[8].load(Ordering::Acquire)
            + self.errors_by_category[9].load(Ordering::Acquire);

        ErrorRecoveryStats {
            total_errors: self.total_errors.load(Ordering::Acquire),
            recoverable_errors: recoverable,
            unrecoverable_errors: unrecoverable,
            resyncs_performed: self.resyncs_performed.load(Ordering::Acquire),
            frames_skipped: self.frames_skipped.load(Ordering::Acquire),
            keyframes_forced: self.keyframes_forced.load(Ordering::Acquire),
            error_rate: self.error_rate(),
            generation: self.generation.load(Ordering::Acquire),
        }
    }

    /// Get generation counter (Q34 audit)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get error count for a specific category
    pub fn error_count_by_category(&self, category: ErrorCategory) -> u32 {
        let idx = category.index();
        if idx < 10 {
            self.errors_by_category[idx].load(Ordering::Acquire)
        } else {
            0
        }
    }
}

impl Default for ErrorRecoveryCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: ErrorRecoveryCapsule uses only atomic types for shared state
// #ASSUME: All atomic operations use appropriate memory ordering
// #VERIFY: Acquire/Release pairs ensure visibility across threads
unsafe impl Send for ErrorRecoveryCapsule {}
unsafe impl Sync for ErrorRecoveryCapsule {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // T28 Q1-Q7: Unit Tests
    // =========================================================================

    /// Q1: Test capsule creation and initial state
    #[test]
    fn test_q1_new_capsule() {
        let capsule = ErrorRecoveryCapsule::new();

        assert_eq!(capsule.error_count(), 0);
        assert_eq!(capsule.last_error(), None);
        assert!(capsule.can_continue());
        assert!(!capsule.needs_keyframe());
        assert_eq!(capsule.generation(), 0);
        assert!(capsule.is_stream_healthy());
    }

    /// Q2: Test error reporting basic functionality
    #[test]
    fn test_q2_report_error_basic() {
        let capsule = ErrorRecoveryCapsule::new();

        capsule.report_error(ErrorCategory::BitstreamCorruption, 1234, "test error");

        assert_eq!(capsule.error_count(), 1);
        assert_eq!(capsule.last_error(), Some(ErrorCategory::BitstreamCorruption));
        assert_eq!(capsule.generation(), 1);
    }

    /// Q3: Test error category classification
    #[test]
    fn test_q3_error_category_classification() {
        // Recoverable
        assert!(ErrorCategory::BitstreamCorruption.is_recoverable());
        assert!(ErrorCategory::MissingReference.is_recoverable());
        assert!(ErrorCategory::ChecksumMismatch.is_recoverable());

        // Needs keyframe
        assert!(ErrorCategory::SliceError.needs_keyframe());
        assert!(ErrorCategory::TileError.needs_keyframe());
        assert!(ErrorCategory::FrameError.needs_keyframe());

        // Fatal
        assert!(ErrorCategory::HeaderCorruption.is_fatal());
        assert!(ErrorCategory::UnsupportedFeature.is_fatal());
        assert!(ErrorCategory::OutOfMemory.is_fatal());
        assert!(ErrorCategory::InternalError.is_fatal());

        // None
        assert!(!ErrorCategory::None.is_recoverable());
        assert!(!ErrorCategory::None.needs_keyframe());
        assert!(!ErrorCategory::None.is_fatal());
    }

    /// Q4: Test recovery strategy selection
    #[test]
    fn test_q4_recovery_strategy_selection() {
        let capsule = ErrorRecoveryCapsule::new();

        // Recoverable -> Resync/SkipFrame
        assert_eq!(
            capsule.get_recovery_strategy(ErrorCategory::BitstreamCorruption),
            RecoveryStrategy::Resync
        );
        assert_eq!(
            capsule.get_recovery_strategy(ErrorCategory::MissingReference),
            RecoveryStrategy::SkipFrame
        );

        // Partial -> SkipToKeyframe
        assert_eq!(
            capsule.get_recovery_strategy(ErrorCategory::SliceError),
            RecoveryStrategy::SkipToKeyframe
        );

        // Fatal -> Restart/Abort
        assert_eq!(
            capsule.get_recovery_strategy(ErrorCategory::HeaderCorruption),
            RecoveryStrategy::Restart
        );
        assert_eq!(
            capsule.get_recovery_strategy(ErrorCategory::OutOfMemory),
            RecoveryStrategy::Abort
        );
    }

    /// Q5: Test clear_errors functionality
    #[test]
    fn test_q5_clear_errors() {
        let capsule = ErrorRecoveryCapsule::new();

        // Add some errors
        capsule.report_error(ErrorCategory::BitstreamCorruption, 100, "error 1");
        capsule.report_error(ErrorCategory::FrameError, 200, "error 2");

        assert_eq!(capsule.error_count(), 2);

        // Clear errors
        capsule.clear_errors();

        assert_eq!(capsule.error_count(), 0);
        assert_eq!(capsule.last_error(), None);
        assert!(capsule.can_continue());
    }

    /// Q6: Test consecutive error tracking
    #[test]
    fn test_q6_consecutive_error_tracking() {
        let capsule = ErrorRecoveryCapsule::new();

        assert_eq!(capsule.consecutive_error_count(), 0);

        capsule.report_error(ErrorCategory::BitstreamCorruption, 100, "error 1");
        assert_eq!(capsule.consecutive_error_count(), 1);

        capsule.report_error(ErrorCategory::BitstreamCorruption, 200, "error 2");
        assert_eq!(capsule.consecutive_error_count(), 2);

        // Record success - should reset consecutive count
        capsule.record_frame_success();
        assert_eq!(capsule.consecutive_error_count(), 0);
    }

    /// Q7: Test generation counter updates
    #[test]
    fn test_q7_generation_counter() {
        let capsule = ErrorRecoveryCapsule::new();

        assert_eq!(capsule.generation(), 0);

        capsule.report_error(ErrorCategory::BitstreamCorruption, 100, "test");
        assert_eq!(capsule.generation(), 1);

        capsule.record_frame_success();
        assert_eq!(capsule.generation(), 2);

        capsule.clear_errors();
        assert_eq!(capsule.generation(), 3);
    }

    // =========================================================================
    // T28 Q8-Q14: Property-based Tests
    // =========================================================================

    /// Q8: Test H.264 sync point detection
    #[test]
    fn test_q8_h264_sync_detection() {
        let capsule = ErrorRecoveryCapsule::new();

        // 4-byte start code
        let data_4byte = [0x00, 0x00, 0x00, 0x01, 0x65, 0x88];
        let result = capsule.find_h264_sync(&data_4byte);
        assert_eq!(result, Some(4));

        // 3-byte start code
        let data_3byte = [0x00, 0x00, 0x01, 0x65, 0x88];
        let result = capsule.find_h264_sync(&data_3byte);
        assert_eq!(result, Some(3));

        // No start code
        let data_none = [0x00, 0x00, 0x02, 0x65, 0x88];
        let result = capsule.find_h264_sync(&data_none);
        assert_eq!(result, None);
    }

    /// Q9: Test VP9 sync point detection
    #[test]
    fn test_q9_vp9_sync_detection() {
        let capsule = ErrorRecoveryCapsule::new();

        // Valid VP9 frame marker (bits 0-1 = 0b10)
        let data_valid = [0xFF, 0xFF, 0x42, 0x00]; // 0x42 = 0b01000010, bits 0-1 = 0b10
        let result = capsule.find_vp9_sync(&data_valid);
        assert_eq!(result, Some(2));

        // First byte is valid
        let data_first = [0x82, 0x00, 0x00]; // 0x82 = 0b10000010
        let result = capsule.find_vp9_sync(&data_first);
        assert_eq!(result, Some(0));

        // No valid marker
        let data_none = [0x01, 0x03, 0x00]; // No byte has bits 0-1 = 0b10
        let result = capsule.find_vp9_sync(&data_none);
        assert_eq!(result, None);
    }

    /// Q10: Test error rate calculation
    #[test]
    fn test_q10_error_rate_calculation() {
        let capsule = ErrorRecoveryCapsule::new();

        // No frames - rate should be 0
        assert_eq!(capsule.error_rate(), 0.0);

        // Add 10 successful frames
        for _ in 0..10 {
            capsule.record_frame_success();
        }

        // Add 1 error
        capsule.report_error(ErrorCategory::BitstreamCorruption, 0, "test");

        // Error rate should be ~100 errors per 1000 frames (1/11 * 1000)
        let rate = capsule.error_rate();
        assert!(rate > 80.0 && rate < 110.0, "Rate was {}", rate);
    }

    /// Q11: Test error window sliding
    #[test]
    fn test_q11_error_window_sliding() {
        let capsule = ErrorRecoveryCapsule::new();

        // Record 5 successes
        for _ in 0..5 {
            capsule.record_frame_success();
        }

        // Record 1 error
        capsule.report_error(ErrorCategory::BitstreamCorruption, 0, "test");

        // Record 10 more successes
        for _ in 0..10 {
            capsule.record_frame_success();
        }

        // Error rate should decrease as window slides
        let rate = capsule.error_rate();
        assert!(rate < 100.0, "Rate was {}", rate);
    }

    /// Q12: Test recovery state transitions
    #[test]
    fn test_q12_recovery_state_transitions() {
        let capsule = ErrorRecoveryCapsule::new();

        assert_eq!(capsule.recovery_state(), RecoveryState::Normal);

        capsule.set_recovery_state(RecoveryState::Syncing);
        assert_eq!(capsule.recovery_state(), RecoveryState::Syncing);

        capsule.set_recovery_state(RecoveryState::SeekingKeyframe);
        assert_eq!(capsule.recovery_state(), RecoveryState::SeekingKeyframe);
        assert!(capsule.needs_keyframe());

        capsule.set_recovery_state(RecoveryState::Normal);
        assert!(!capsule.needs_keyframe());
    }

    /// Q13: Test concealment strategy suggestions
    #[test]
    fn test_q13_concealment_strategy() {
        let capsule = ErrorRecoveryCapsule::new();

        assert_eq!(
            capsule.suggest_concealment(ErrorCategory::None),
            ConcealmentStrategy::None
        );
        assert_eq!(
            capsule.suggest_concealment(ErrorCategory::BitstreamCorruption),
            ConcealmentStrategy::RepeatLastFrame
        );
        assert_eq!(
            capsule.suggest_concealment(ErrorCategory::MissingReference),
            ConcealmentStrategy::InterpolateMV
        );
        assert_eq!(
            capsule.suggest_concealment(ErrorCategory::FrameError),
            ConcealmentStrategy::GrayFrame
        );
        assert_eq!(
            capsule.suggest_concealment(ErrorCategory::OutOfMemory),
            ConcealmentStrategy::SkipDisplay
        );
    }

    /// Q14: Test error count by category
    #[test]
    fn test_q14_error_count_by_category() {
        let capsule = ErrorRecoveryCapsule::new();

        capsule.report_error(ErrorCategory::BitstreamCorruption, 0, "1");
        capsule.report_error(ErrorCategory::BitstreamCorruption, 0, "2");
        capsule.report_error(ErrorCategory::FrameError, 0, "3");

        assert_eq!(
            capsule.error_count_by_category(ErrorCategory::BitstreamCorruption),
            2
        );
        assert_eq!(capsule.error_count_by_category(ErrorCategory::FrameError), 1);
        assert_eq!(
            capsule.error_count_by_category(ErrorCategory::OutOfMemory),
            0
        );
    }

    // =========================================================================
    // T28 Q15-Q21: Integration Tests
    // =========================================================================

    /// Q15: Test full error/recovery cycle
    #[test]
    fn test_q15_full_error_recovery_cycle() {
        let capsule = ErrorRecoveryCapsule::new();

        // Start with successful frames
        for _ in 0..5 {
            capsule.record_frame_success();
        }

        // Encounter bitstream error
        capsule.report_error(ErrorCategory::BitstreamCorruption, 1000, "corrupt data");
        let strategy = capsule.get_recovery_strategy(ErrorCategory::BitstreamCorruption);
        assert_eq!(strategy, RecoveryStrategy::Resync);

        // Simulate resync
        let data = [0x00, 0x00, 0x00, 0x01, 0x67];
        let sync_point = capsule.find_sync_point(&data, VideoCodec::H264);
        assert!(sync_point.is_some());

        // Resume successful decoding
        capsule.record_frame_success();
        assert!(capsule.can_continue());
    }

    /// Q16: Test consecutive error escalation
    #[test]
    fn test_q16_consecutive_error_escalation() {
        let capsule = ErrorRecoveryCapsule::with_thresholds(3, 50);

        // Report 3 consecutive errors (at threshold)
        capsule.report_error(ErrorCategory::BitstreamCorruption, 0, "1");
        capsule.report_error(ErrorCategory::BitstreamCorruption, 0, "2");
        capsule.report_error(ErrorCategory::BitstreamCorruption, 0, "3");

        // Next recoverable error should escalate to SkipToKeyframe
        let strategy = capsule.get_recovery_strategy(ErrorCategory::BitstreamCorruption);
        assert_eq!(strategy, RecoveryStrategy::SkipToKeyframe);
    }

    /// Q17: Test H.264 keyframe detection
    #[test]
    fn test_q17_h264_keyframe_detection() {
        let capsule = ErrorRecoveryCapsule::new();

        // IDR slice (NAL type 5)
        let data_idr = [0x00, 0x00, 0x00, 0x01, 0x65]; // 0x65 = 0b01100101, type = 5
        let result = capsule.find_keyframe(&data_idr, VideoCodec::H264);
        assert_eq!(result, Some(4));

        // SPS (NAL type 7)
        let data_sps = [0x00, 0x00, 0x01, 0x67]; // 0x67 = 0b01100111, type = 7
        let result = capsule.find_keyframe(&data_sps, VideoCodec::H264);
        assert_eq!(result, Some(3));

        // Non-IDR slice (NAL type 1) - not a keyframe
        let data_p = [0x00, 0x00, 0x00, 0x01, 0x41]; // 0x41 = 0b01000001, type = 1
        let result = capsule.find_keyframe(&data_p, VideoCodec::H264);
        assert_eq!(result, None);
    }

    /// Q18: Test VP9 keyframe detection
    #[test]
    fn test_q18_vp9_keyframe_detection() {
        let capsule = ErrorRecoveryCapsule::new();

        // VP9 keyframe: frame_marker=10, show_existing=0, frame_type=0 (key)
        // 0x42 = 0b01000010: bits 0-1=10 (marker), bit 4=0 (show_exist), bit 5=0 (keyframe)
        let data_key = [0x42, 0x00, 0x00];
        let result = capsule.find_keyframe(&data_key, VideoCodec::Vp9);
        assert_eq!(result, Some(0));
    }

    /// Q19: Test statistics snapshot
    #[test]
    fn test_q19_statistics_snapshot() {
        let capsule = ErrorRecoveryCapsule::new();

        // Generate some activity
        capsule.report_error(ErrorCategory::BitstreamCorruption, 0, "1");
        capsule.report_error(ErrorCategory::ChecksumMismatch, 0, "2");
        capsule.report_error(ErrorCategory::OutOfMemory, 0, "3");

        let _data = [0x00, 0x00, 0x01, 0x65];
        capsule.find_h264_sync(&_data);
        capsule.record_frame_skip();

        let stats = capsule.stats();

        assert_eq!(stats.total_errors, 3);
        assert_eq!(stats.recoverable_errors, 2); // BitstreamCorruption + ChecksumMismatch
        assert_eq!(stats.unrecoverable_errors, 1); // OutOfMemory
        assert_eq!(stats.resyncs_performed, 1);
        assert_eq!(stats.frames_skipped, 1);
        assert!(stats.generation > 0);
    }

    /// Q20: Test stream health assessment
    #[test]
    fn test_q20_stream_health() {
        let capsule = ErrorRecoveryCapsule::with_thresholds(3, 100); // 10% threshold

        // Healthy stream - many successes, few errors
        for _ in 0..100 {
            capsule.record_frame_success();
        }
        capsule.report_error(ErrorCategory::BitstreamCorruption, 0, "minor");

        assert!(capsule.is_stream_healthy());

        // Make it unhealthy - many errors
        for _ in 0..20 {
            capsule.report_error(ErrorCategory::BitstreamCorruption, 0, "bad");
        }

        assert!(!capsule.is_stream_healthy());
    }

    /// Q21: Test codec-specific sync detection
    #[test]
    fn test_q21_codec_specific_sync() {
        let capsule = ErrorRecoveryCapsule::new();

        // H.264 data
        let h264_data = [0x00, 0x00, 0x00, 0x01, 0x65];
        let result = capsule.find_sync_point(&h264_data, VideoCodec::H264);
        assert!(result.is_some());

        // VP9 data
        let vp9_data = [0x82, 0x00, 0x00]; // Frame marker 0b10
        let result = capsule.find_sync_point(&vp9_data, VideoCodec::Vp9);
        assert!(result.is_some());

        // Unknown codec - tries all
        let result = capsule.find_sync_point(&h264_data, VideoCodec::Unknown);
        assert!(result.is_some());
    }

    // =========================================================================
    // T28 Q22-Q28: Production Tests
    // =========================================================================

    /// Q22: Test concurrent access safety
    #[test]
    fn test_q22_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(ErrorRecoveryCapsule::new());
        let mut handles = vec![];

        // Spawn multiple threads reporting errors
        for i in 0u8..4u8 {
            let capsule_clone = Arc::clone(&capsule);
            handles.push(thread::spawn(move || {
                for j in 0u8..100u8 {
                    capsule_clone.report_error(
                        ErrorCategory::from_u8((i.wrapping_add(j)) % 10),
                        (i as u64).wrapping_mul(100).wrapping_add(j as u64),
                        "concurrent test",
                    );
                    capsule_clone.record_frame_success();
                    let _ = capsule_clone.stats();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should complete without panics
        assert!(capsule.error_count() > 0);
    }

    /// Q23: Test capsule size and alignment
    #[test]
    fn test_q23_capsule_size_alignment() {
        assert_eq!(
            core::mem::size_of::<ErrorRecoveryCapsule>(),
            256,
            "Capsule must be 256B for T1 Atomic tier"
        );
        assert_eq!(
            core::mem::align_of::<ErrorRecoveryCapsule>(),
            256,
            "Capsule must be 256B aligned"
        );
    }

    /// Q24: Test error category exhaustive coverage
    #[test]
    fn test_q24_error_category_coverage() {
        // Test all 10 error categories
        for i in 0u8..10 {
            let category = ErrorCategory::from_u8(i);
            assert_ne!(category, ErrorCategory::None);

            // Each category should have exactly one classification
            let is_rec = category.is_recoverable();
            let is_key = category.needs_keyframe();
            let is_fat = category.is_fatal();

            // At most one should be true
            let count = [is_rec, is_key, is_fat].iter().filter(|&&x| x).count();
            assert!(count <= 1, "Category {:?} has multiple classifications", category);
        }
    }

    /// Q25: Test AV1 sync point detection
    #[test]
    fn test_q25_av1_sync_detection() {
        let capsule = ErrorRecoveryCapsule::new();

        // AV1 sequence header OBU (type 1)
        // Header: forbidden=0, type=1 (bits 3-6), extension=0, has_size=1, reserved=0
        // 0b0_0001_0_1_0 = 0x0A
        let data_seq = [0x0A, 0x00, 0x00];
        let result = capsule.find_av1_sync(&data_seq);
        assert_eq!(result, Some(0));

        // AV1 frame OBU (type 6)
        // Header: forbidden=0, type=6 (bits 3-6), extension=0, has_size=1, reserved=0
        // 0b0_0110_0_1_0 = 0x32
        let data_frame = [0xFF, 0x32, 0x00];
        let result = capsule.find_av1_sync(&data_frame);
        assert_eq!(result, Some(1));
    }

    /// Q26: Test custom thresholds
    #[test]
    fn test_q26_custom_thresholds() {
        let capsule = ErrorRecoveryCapsule::with_thresholds(5, 200);

        // Report 4 errors (below threshold of 5)
        for _ in 0..4 {
            capsule.report_error(ErrorCategory::BitstreamCorruption, 0, "test");
        }

        // Should still suggest Resync, not SkipToKeyframe
        let strategy = capsule.get_recovery_strategy(ErrorCategory::BitstreamCorruption);
        assert_eq!(strategy, RecoveryStrategy::Resync);

        // Report 5th error (at threshold)
        capsule.report_error(ErrorCategory::BitstreamCorruption, 0, "test");

        // Now should escalate
        let strategy = capsule.get_recovery_strategy(ErrorCategory::BitstreamCorruption);
        assert_eq!(strategy, RecoveryStrategy::SkipToKeyframe);
    }

    /// Q27: Test aborted state prevents continuation
    #[test]
    fn test_q27_aborted_state() {
        let capsule = ErrorRecoveryCapsule::new();

        assert!(capsule.can_continue());

        capsule.set_recovery_state(RecoveryState::Aborted);

        assert!(!capsule.can_continue());
    }

    /// Q28: Test display implementations
    #[test]
    fn test_q28_display_implementations() {
        // ErrorCategory
        assert_eq!(
            format!("{}", ErrorCategory::BitstreamCorruption),
            "Bitstream corruption"
        );
        assert_eq!(
            format!("{}", ErrorCategory::OutOfMemory),
            "Out of memory"
        );

        // RecoveryStrategy
        assert_eq!(
            format!("{}", RecoveryStrategy::Resync),
            "Resync to next sync point"
        );
        assert_eq!(
            format!("{}", RecoveryStrategy::Abort),
            "Abort - cannot recover"
        );

        // ConcealmentStrategy
        assert_eq!(
            format!("{}", ConcealmentStrategy::RepeatLastFrame),
            "Repeat last frame"
        );

        // VideoCodec
        assert_eq!(format!("{}", VideoCodec::H264), "H.264/AVC");
        assert_eq!(format!("{}", VideoCodec::Av1), "AV1");
    }

    // =========================================================================
    // Additional Edge Case Tests
    // =========================================================================

    /// Test empty data handling
    #[test]
    fn test_empty_data() {
        let capsule = ErrorRecoveryCapsule::new();

        assert_eq!(capsule.find_h264_sync(&[]), None);
        assert_eq!(capsule.find_vp9_sync(&[]), None);
        assert_eq!(capsule.find_av1_sync(&[]), None);
        assert_eq!(capsule.find_keyframe(&[], VideoCodec::H264), None);
    }

    /// Test small data handling
    #[test]
    fn test_small_data() {
        let capsule = ErrorRecoveryCapsule::new();

        // Too small for start code
        assert_eq!(capsule.find_h264_sync(&[0x00]), None);
        assert_eq!(capsule.find_h264_sync(&[0x00, 0x00]), None);

        // Single byte VP9 check should work
        assert_eq!(capsule.find_vp9_sync(&[0x82]), Some(0));
    }

    /// Test default implementation
    #[test]
    fn test_default_impl() {
        let capsule = ErrorRecoveryCapsule::default();
        assert_eq!(capsule.error_count(), 0);
        assert_eq!(capsule.generation(), 0);
    }

    /// Test reporting None error does nothing
    #[test]
    fn test_report_none_error() {
        let capsule = ErrorRecoveryCapsule::new();

        capsule.report_error(ErrorCategory::None, 0, "should not count");

        assert_eq!(capsule.error_count(), 0);
        assert_eq!(capsule.generation(), 1); // Generation still increments
    }

    /// Test last error offset
    #[test]
    fn test_last_error_offset() {
        let capsule = ErrorRecoveryCapsule::new();

        capsule.report_error(ErrorCategory::BitstreamCorruption, 0x1234_5678_9ABC, "test");

        let offset = capsule.last_error_offset();
        assert_eq!(offset, 0x1234_5678_9ABC);
    }

    /// Test multiple codec sync in unknown mode
    #[test]
    fn test_unknown_codec_fallback() {
        let capsule = ErrorRecoveryCapsule::new();

        // Data that matches VP9 but not H.264
        let data = [0x82, 0x00, 0x00]; // VP9 marker only

        let result = capsule.find_sync_point(&data, VideoCodec::Unknown);
        assert!(result.is_some());
    }
}
