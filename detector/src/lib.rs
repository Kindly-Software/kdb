//! AI Image Detection via Computational Capsule Architecture
//!
//! **Phase 1**: T6 Mixed Composite (T1+T2+T3 flat layout)
//! - T1 Atomic: Lockfree coordination (fusion)
//! - T2 SIMD: Frequency/noise analysis (optional)
//! - T3 Fixed-Point: Deterministic statistical thresholds
//!
//! **Performance Targets**:
//! - Frequency analysis: 40ms
//! - Statistical tests: 20ms
//! - Noise analysis: 30ms
//! - Fusion: <1ms
//! - **Total**: <100ms per image
//!
//! **Frameworks**:
//! - UCE34 Q1-Q34: Systematic discovery
//! - ASSUM: 99.99% safe (lockfree atomics)
//! - T28: 5+ comprehensive tests
//! - B32: Honest benchmarking

pub mod detector;

pub use detector::{
    AIImageDetectorCapsule,
    DetectionError,
    DetectionState,
    DetectionVerdict,
    ImageFormat,
    ImageInput,
};
