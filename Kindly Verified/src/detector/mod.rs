//! Detector module for AI image detection
//! [TRADE SECRET] - Proprietary detection algorithms
//!
//! This module provides high-performance detection capsules using UCE34 framework:
//! - FeatureExtractorCapsule: T2 SIMD (DINOv2 ViT-L/14)
//! - UncertaintyDetectionCapsule: T3 (Fixed-Point) + T4 (Batch) + T10 (Probabilistic)
//! - ChromaticAberrationCapsule: T2 SIMD + T3 Fixed-Point (Phase 3.2, Task 3/5) - Optical lens signature detection
//! - DemosaicingPatternCapsule: T2 (SIMD) + T3 (Fixed-Point) for Bayer CFA detection
//!
//! ## Architecture
//!
//! **T6 Mixed Multi-Tier Composite**:
//! ```text
//! Image → FeatureExtractor (T2 SIMD, <15ms)
//!       → UncertaintyDetection (T3+T4+T10, <3ms)
//!       → ChromaticAberration (T2+T3, <4ms) [Phase 3.2, Task 3/5]
//!       → DemosaicingPattern (T2+T3, <5ms)
//!       → Ensemble Fusion (T1 Atomic, <1ms)
//!       → Verdict (AI/Real)
//! Total latency: <28ms (with chromatic aberration detection)
//! ```
//!
//! ## Module Exports
//!
//! - `ChromaticAberrationCapsule`: Primary detector capsule
//! - `ChromaticAberrationResult`: Detection results with shift, radial, purple metrics
//! - `ChromaticAberrationTier`: Confidence classification (StrongNatural, Natural, Ambiguous, StrongAI)

#[cfg(feature = "onnx-dinov2")]
pub mod feature_extractor;

pub mod uncertainty;
pub mod ensemble_fusion;
pub mod demosaicing_pattern;
pub mod chromatic_aberration;
pub mod exif_database;

#[cfg(feature = "onnx-dinov2")]
pub use feature_extractor::{
    FeatureExtractorCapsule, FeatureExtractionError, DINOV2_FEATURE_DIM, DINOV2_INPUT_SIZE,
};

pub use uncertainty::{UncertaintyDetectionCapsule, UncertaintyResult, UncertaintyError};
pub use ensemble_fusion::{
    DetectionCoordinationCapsule, DetectionVerdict, EnsembleWeights,
};
pub use demosaicing_pattern::DemosaicingPatternCapsule;
pub use chromatic_aberration::{
    ChromaticAberrationCapsule, ChromaticAberrationResult, ChromaticAberrationTier,
};
pub use exif_database::{
    EXIFCameraDatabaseCapsule, EXIFMetadata, EXIFValidationResult, EXIFDatabaseError,
};

/// Detection result combining all detector outputs
#[derive(Debug, Clone)]
pub struct DetectionResult {
    /// Is image AI-generated (final verdict)
    pub is_ai_generated: bool,

    /// Confidence score (0.0-1.0)
    pub confidence: f32,

    /// Uncertainty metric (if DINOv2 available)
    #[cfg(feature = "onnx-dinov2")]
    pub uncertainty_score: Option<f32>,

    /// Timestamp of detection (ns)
    pub timestamp_ns: u64,
}
