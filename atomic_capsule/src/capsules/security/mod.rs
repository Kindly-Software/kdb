// atomic_capsule/src/capsules/security/mod.rs
// Security-related computational capsules
//
// Week 8 Additions:
// - federated_learning.rs: Privacy-preserving distributed ML (FederatedGradientBuffer 128B)
// - xai_audit.rs: Explainable AI decision audit trail (XAIDecisionRecord 64B, 256-entry ring)
// - behavioral_anomaly_v2.rs: Extended with AdversarialDefense (64B)

pub mod adaptive_rate_limiter;
pub mod advanced_bot_detector;
pub mod advanced_bot_detector_v2;
pub mod supply_chain_verifier;
pub mod constant_time_ops;
pub mod behavioral_anomaly;
pub mod behavioral_anomaly_v2;
pub mod attention_weights;
pub mod zero_trust_session;
pub mod ja3_database;
pub mod temporal_bot;

// Week 8: Federated Learning + XAI Explainability
#[cfg(feature = "security-federated-learning")]
pub mod federated_learning;

#[cfg(feature = "security-xai-audit")]
pub mod xai_audit;

#[cfg(feature = "security-prompt-injection")]
pub mod prompt_injection_detector;

#[cfg(feature = "security-jailbreak-defender")]
pub mod jailbreak_defender;

#[cfg(feature = "security-jailbreak-defender")]
pub mod jailbreak_calibration;

#[cfg(feature = "security-data-exfiltration")]
pub mod data_exfiltration_guard;

#[cfg(feature = "security-false-positive-mitigation")]
pub mod false_positive_mitigation;

pub use adaptive_rate_limiter::{
    AdaptiveRateLimiterCapsule, RateLimitError, RateLimiterStats,
};

pub use advanced_bot_detector::{
    AdvancedBotDetectorCapsule, ConfidenceScore, Decision, DetectionSignals, Statistics,
};

pub use advanced_bot_detector_v2::{
    AdvancedBotDetectorV2, DecisionV2, EvaluationResult,
    MouseBehaviorCapsule, KeystrokeDynamicsCapsule, JA3FingerprintCapsule,
    TemporalPatternCapsule, OriginalSignalsCapsule,
};

pub use constant_time_ops::ConstantTimeOpsCapsule;

pub use behavioral_anomaly::{
    BehavioralAnomalyCapsule, AnomalyType, Decision as AnomalyDecision, ModelId,
};

// Week 7: BehavioralAnomalyCapsuleV2 - Enhanced ML-Based Zero-Day Detection
pub use behavioral_anomaly_v2::{
    BehavioralAnomalyCapsuleV2, AnomalyTypeV2, DecisionV2 as AnomalyDecisionV2,
    ExternalModelId, TinyMLTreeId, CompactTreeNode,
    NUM_EXTERNAL_MODELS, NUM_TINYML_TREES, TOTAL_MODELS, NUM_ANOMALY_TYPES,
};

// Week 7: AttentionWeightsCapsule - Confidence-Based Ensemble Voting
pub use attention_weights::{
    AttentionWeightsCapsule, MAX_MODELS, ModelCategory,
};

pub use zero_trust_session::{
    ZeroTrustSessionCapsule, SessionError, SessionState,
};

#[cfg(feature = "supply-chain-verifier")]
pub use supply_chain_verifier::{
    SupplyChainVerifierCapsule, VerificationConfig, VerificationError, VerificationReport,
    VerificationStats,
};

#[cfg(feature = "security-prompt-injection")]
pub use prompt_injection_detector::{
    PromptInjectionDetectorCapsule, RiskScore, Decision as InjectionDecision,
    DetectionBreakdown, Statistics as InjectionStatistics, EMBEDDING_DIM,
};

#[cfg(feature = "security-jailbreak-defender")]
pub use jailbreak_defender::{
    JailbreakDefenderCapsule, AttackPattern, Decision as JailbreakDecision, ThreatScore, MinHashSignature,
};

#[cfg(feature = "security-data-exfiltration")]
pub use data_exfiltration_guard::{
    DataExfiltrationGuardCapsule, PIIPatternType, ThreatScore as ExfiltrationThreatScore,
    ValidationResult, AuditEntry, Statistics as ExfiltrationStatistics,
};

#[cfg(feature = "security-false-positive-mitigation")]
pub use false_positive_mitigation::{
    FalsePositiveMitigationCapsule, CombinedThreatScore, ConsensusDecision, ThresholdLevel,
    MitigationStats, ValidationError,
};

#[cfg(all(
    feature = "security-false-positive-mitigation",
    feature = "security-prompt-injection",
    feature = "security-jailbreak-defender",
    feature = "security-data-exfiltration"
))]
pub use false_positive_mitigation::SecureLlmValidator;

// JA3 Fingerprint Database (Week 5 Bot Detection)
pub use ja3_database::{
    JA3DatabaseCapsule, Ja3Hash, Ja3LookupResult, Ja3Statistics, BotCategory,
    KNOWN_BOT_JA3_HASHES,
};

// Temporal Bot Detection (Week 5 Bot Detection)
pub use temporal_bot::{
    TemporalBotCapsule, TemporalDetection, TemporalStatistics,
    thresholds as temporal_thresholds,
};

// Week 4: Mouse and Keystroke Dynamics (T6 Mixed Capsules)
// Behavioral biometrics achieving 87% accuracy (SOTA 2024-2025)
// These are enhanced implementations with Welford online variance and detailed CV calculation
pub mod mouse_dynamics;
pub mod keystroke_dynamics;

// Week 4 exports - Enhanced behavioral biometrics capsules
pub use mouse_dynamics::{
    MouseDynamicsCapsule, MousePoint, MouseEvaluation, MouseStatistics,
    BotScore as MouseBotScore,
};
pub use keystroke_dynamics::{
    KeystrokeDynamicsCapsule as KeystrokeDynamicsCapsuleV2, KeyEvent, KeyEventType,
    KeystrokeEvaluation, KeystrokeStatistics, BotScore as KeystrokeBotScore,
};

// Week 8: Federated Learning exports
#[cfg(feature = "security-federated-learning")]
pub use federated_learning::{
    FederatedGradientBuffer, FederatedError, FederatedStats,
    AggregationMode, PrivacyBudgetState, ClientStatus,
    MAX_GRADIENT_DIM, MAX_CLIENTS, DEFAULT_EPSILON, DEFAULT_DELTA,
};

// Week 8: XAI Audit Trail exports
#[cfg(feature = "security-xai-audit")]
pub use xai_audit::{
    XAIDecisionRecord, XAIAuditRing, XAIAuditStats,
    TopContributor, FeatureId, DecisionOutcome,
    compute_shap_importance, compute_integrated_gradients,
    MAX_FEATURES as XAI_MAX_FEATURES, MAX_TOP_CONTRIBUTORS, AUDIT_RING_CAPACITY,
};

// Week 8: Adversarial Defense exports (always available with behavioral_anomaly_v2)
pub use behavioral_anomaly_v2::{
    AdversarialDefense, AdversarialAttackType, DefenseAction, AdversarialCheckResult,
    ADV_MAX_FEATURES,
};
