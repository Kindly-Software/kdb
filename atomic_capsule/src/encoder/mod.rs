//! AV1 Encoder Capsules - Lockfree Video Encoding Primitives
//!
//! This module provides computational capsules for AV1 video encoding, replacing rav1e
//! with 100% lockfree, cache-aligned primitives following UCE34/COCA framework.
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
//! - COCA: 100% computational capsules, cache-aligned
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
pub mod dct_transform;

pub use frame_buffer::FrameBufferCapsule;
pub use state::EncoderStateCapsule;
pub use quantization::QuantizationCapsule;
pub use tile_coordinator::{TileCoordinatorCapsule, TileStatus};
pub use dct_transform::DctTransformCapsule;

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
