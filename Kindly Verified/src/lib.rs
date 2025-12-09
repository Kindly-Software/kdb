//! kindly-verified: Forensic-Grade AI Image Detection System
//!
//! **Version**: 0.2.9 (Phase 2.9 - Production Ready)
//! **Framework**: UCE34 Systematic Discovery + Chaos Computational Capsules
//! **Architecture**: T6 Mixed Multi-Tier Composite (T1 + T2 + T3 + T4 + T10)
//!
//! **Trade Secret**: [TRADE SECRET] - Proprietary detection algorithms
//!
//! This library provides forensic-grade detection of AI-generated images using
//! computational capsule architecture for lockfree, deterministic performance.
//!
//! ## Example
//!
//! ```ignore
//! use kindly_verified::forensic::PRNUAnalysisCapsule;
//!
//! let mut capsule = PRNUAnalysisCapsule::new();
//! let (pce, confidence_tier) = capsule.analyze_prnu_robust(&image_data, None)?;
//! ```

#![forbid(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

pub mod forensic;
pub mod error;
pub mod detector;  // Phase 3.2: DINOv2 ONNX Integration

pub use error::DetectionError;
pub use forensic::{PRNUAnalysisCapsule, PRNUConfidenceTier, BenfordJPEGCapsule, BenfordConfidenceTier};
pub use detector::{
    DetectionCoordinationCapsule, DetectionVerdict, EnsembleWeights,
    ChromaticAberrationCapsule, ChromaticAberrationResult, ChromaticAberrationTier,
};
