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
//! - **COCA**: 100% computational capsules, cache-aligned (64B/128B/256B)
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
pub mod wiring_capsule;
pub mod metacapsule;
pub mod sub_capsules;

// ============================================================================
// Re-exports from atomic_capsule (Core Encoder Primitives)
// ============================================================================

// State Management
pub use atomic_capsule::encoder::{
    EncoderStateCapsule,
    EncoderState,
    EncoderError,
    SpeedPreset,
    QualityMode,
    PixelFormat,
};

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

// Reference Frame Management
pub use atomic_capsule::encoder::{
    ReferenceFrameCapsule,
    ReferenceType,
};

// Tile Coordination (not ParallelTileEncoder)
pub use atomic_capsule::encoder::{
    TileCoordinatorCapsule,
    TileStatus,
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
pub use wiring_capsule::EncoderWiringCapsule;
pub use metacapsule::KindlyAv1CliMetacapsule;
pub use sub_capsules::EncoderSubCapsules;

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
}
