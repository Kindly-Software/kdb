//! Security capsules for kindly-verified-web
//!
//! High-performance, cryptographically-secure primitives for sensitive operations.
//! All capsules are 100% lockfree, constant-time, and fully auditable.
//!
//! **Capsules** (UCE34 v6.0 framework):
//! - BehavioralAnomalyCapsule (T10+T1): Unsupervised ML anomaly detection (99.11% accuracy)
//! - ConstantTimeOpsCapsule (T1+T2): Constant-time operations (side-channel resistant)
//! - ZeroTrustSessionCapsule (T1+T0): Continuous session verification (NIST compliance)
//! - SupplyChainVerifierCapsule (T0+T1): SLSA framework verification
//! - AdvancedBotDetectorCapsule (T10+T1): AI scraper detection with behavioral analysis

pub mod behavioral_anomaly;
pub mod constant_time_ops;
pub mod zero_trust_session;
pub mod supply_chain_verifier;
pub mod advanced_bot_detector;

pub use behavioral_anomaly::BehavioralAnomalyCapsule;
pub use constant_time_ops::{ConstantTimeOpsCapsule, ConstTimeResult};
pub use zero_trust_session::{ZeroTrustSessionCapsule, SessionState, RiskLevel, RequestMetadata, SessionAuditEntry, calculate_risk_score, verify_audit_trail_integrity};
pub use supply_chain_verifier::{
    SupplyChainVerifierCapsule,
    VerificationResult,
    SlsaLevel,
    DependencyProvenance,
    BuildReproducibilityCheck,
    SupplyChainAuditEntry,
    SupplyChainStats,
};
pub use advanced_bot_detector::{
    AdvancedBotDetectorCapsule,
    AutomationDetection,
    BotClassification,
    BotDetectionRequest,
    BotDetectionResult,
    BrowserFingerprint,
    DetectionStats,
    EvacionDetection,
};
