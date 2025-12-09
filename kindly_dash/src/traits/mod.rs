//! Public traits for integrating with kindly_dash
//!
//! Core traits:
//! - [MetricsSource]: Provides metrics data to the dashboard
//! - [AuditableCapsule]: Tier 0 hash chain integrity for all state-modifying capsules (TODO: Phase 4)
//! - Tier hierarchy: T1-T6 auditable capsule traits (TODO: Phase 5)

// TODO: Phase 4 - Auditable trait (Q34 Auditability)
// pub mod auditable;

pub mod metrics_source;

// TODO: Phase 5 - Tier hierarchy trait (UCE34 tier composition)
// pub mod tier_hierarchy;

// Re-export for convenience
pub use metrics_source::MetricsSource;

// TODO: Phase 4 exports
// pub use auditable::{AuditableCapsule, CapsuleAuditTrail, CapsuleSnapshot};

// TODO: Phase 5 exports
// pub use tier_hierarchy::{
//     AtomicAuditableCapsule, BatchAuditableCapsule, FixedPointAuditableCapsule,
//     MixedAuditableCapsule, SimdAuditableCapsule, StreamingAuditableCapsule, TIER_1_ATOMIC,
//     TIER_2_SIMD, TIER_3_FIXED_POINT, TIER_4_BATCH, TIER_5_STREAMING, TIER_6_MIXED,
// };
