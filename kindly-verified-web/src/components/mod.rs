// Component modules for all 11 capsules
pub mod effects;      // Core effects (5 capsules: NeomorphButton, ForensicDashboard, etc.)
pub mod processing;   // Processing (2 capsules: WebWorkerProcessor, ProgressiveImage)
pub mod data;         // Data handling (2 capsules: DetectionHistory, ExportButton)
pub mod upload;       // Upload (1 capsule: BatchUpload via batch_upload.rs)
pub mod ui;           // UI (1 capsule: ProgressBar)

// Re-exports for convenient access
pub use upload::*;
