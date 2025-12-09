// AI Image Detector Module
//
// **Phase 1 Architecture**: T6 Mixed Composite (T1+T2+T3 flat layout)
// **Verification Expert Integration**: Compile-time capsule verification

pub mod capsule;
pub mod coordination;

// Re-exports
pub use capsule::{
    AIImageDetectorCapsule,
    DetectionError,
    DetectionVerdict,
    ImageFormat,
    ImageInput,
};
pub use coordination::{DetectionCoordinationCapsule, DetectionState};
