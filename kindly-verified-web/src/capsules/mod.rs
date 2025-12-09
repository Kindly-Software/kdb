/// Computational capsules for kindly-verified-web effects
///
/// All capsules are:
/// - 100% lockfree (no mutex/RwLock)
/// - Cache-aligned (64B/128B/256B/16KB/1024B)
/// - Chaos compliant (computational capsule architecture)
/// - Framework validated (UCE34, ASSUM, B32, T28, I20)
///
/// ## Capsule Inventory (11 total, 239+ tests)
///
/// **Core Effects (5 capsules, 71 tests)**:
/// 1. **NeomorphGlassButtonCapsule** (64B, T1+T3): Soft 3D button with glassmorphism
/// 2. **ForensicDashboardCapsule** (384B, T2+T5+T1): 10-bar animated detector dashboard
/// 3. **ParallaxHeroCapsule** (128B, T1+T3+T5): 3-layer depth scrolling
/// 4. **ParticleScanningCapsule** (16KB, T2+T4+T5): 500 particles physics simulation
/// 5. **LiquidMorphingMeterCapsule** (1152B, T2+T3+T5): Metaball confidence meter
///
/// **Processing & Data (6 capsules, 168 tests)**:
/// 6. **WebWorkerBackgroundProcessingCapsule** (256B, T5+T1): Lockfree job queue for Web Workers
/// 7. **DetectionHistoryCapsule** (64B, T9+T1): Persistent IndexedDB storage with Q34 audit trail
/// 8. **ProgressiveImageLoaderCapsule** (2560B, T5+T4): Progressive JPEG/PNG decode with blur-to-sharp
/// 9. **ExportResultsCapsule** (256B, T4+T0): PDF/JSON/CSV export with Q34 audit trails (28 tests)
/// 10. **BatchUploadCapsule** (1024B, T4+T5): Parallel image upload with lockfree work-stealing queue
/// 11. **ProgressBarCapsule** (64B, T1+T3): Smooth progress animation with Byzantine colors (28 tests)
/// 12. **ZeroTrustSessionCapsule** (64B, T1+T0+T10): Continuous session verification with risk scoring (28 tests)

pub mod neomorph_button;
pub mod forensic_dashboard;
pub mod parallax_hero;
pub mod particle_scanning;
pub mod liquid_morphing;
pub mod web_worker;
pub mod detection_history;
pub mod progressive_loader;
pub mod export_results;
pub mod batch_upload;
pub mod progress_bar;
pub mod security;

// Re-exports
pub use neomorph_button::NeomorphGlassButtonCapsule;
pub use forensic_dashboard::{ForensicDashboardCapsule, BarData};
pub use parallax_hero::ParallaxHeroCapsule;
pub use particle_scanning::{
    ParticleScanningCapsule, Particle, ParticleData, DetectorResult, colors,
};
pub use liquid_morphing::{LiquidMorphingMeterCapsule, ShapeState};
pub use web_worker::{
    WebWorkerBackgroundProcessingCapsule, JobId, JobStatus, WorkerState, DetectionResult,
};
pub use detection_history::{
    DetectionHistoryCapsule, DetectionEntry, DetectorResults, ComparisonView, StorageError,
};
pub use progressive_loader::{
    ProgressiveImageLoaderCapsule, ImageFormat, DecodeStage, DecodeProgress,
    ImagePreview, DecodeError,
};
pub use export_results::{
    ExportResultsCapsule, ExportFormat, DetectionEntry as ExportDetectionEntry,
    DetectorResult as ExportDetectorResult, ByzantineColors,
};
pub use batch_upload::{
    BatchUploadCapsule, BatchStats, ImageFile, UploadResult,
};
pub use progress_bar::ProgressBarCapsule;
pub use security::{
    ZeroTrustSessionCapsule, SessionState, VerificationResult, RiskLevel, RequestMetadata,
    SessionAuditEntry, calculate_risk_score, verify_audit_trail_integrity,
    ConstantTimeOpsCapsule, ConstTimeResult, BehavioralAnomalyCapsule,
};
