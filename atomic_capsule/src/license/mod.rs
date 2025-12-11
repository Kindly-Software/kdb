//! # License Entanglement Module - Cryptographic Computation Binding
//!
//! **[TRADE SECRET] - Breakthrough DRM Innovation**
//!
//! This module implements a revolutionary approach to license protection where the license key
//! IS the computation, not a gate around it. Traditional DRM can be bypassed by NOPing out checks.
//! CHAOS DRM cannot - wrong license = wrong computation = garbage output.
//!
//! ## UCE34 Framework Compliance (Q1-Q34)
//!
//! **Q10 Tier Selection**: T6 Mixed (T0 Auditable + T1 Atomic + T3 Fixed-Point)
//! - T0: Hash-chain audit trail with license anchoring
//! - T1: Atomic state coordination with license transforms
//! - T3: Fixed-point feature computations entangled with license
//!
//! **Q33 Lockfree**: 100% lockfree, no Mutex/RwLock
//! **Q34 Audit**: Hash-chain audit trail with license anchoring (Q34 compliant)
//!
//! ## Core Innovation
//!
//! ```rust,ignore
//! // Traditional DRM (breakable):
//! if !check_license() { exit(); }  // NOP this out
//!
//! // CHAOS DRM (unbreakable):
//! let next = state ^ license_transform;  // Wrong key = garbage
//! // There's no check to bypass - license IS the math
//! ```
//!
//! ## Architecture
//!
//! - **LicenseEntangledCapsule** (128B): Core computation entangled with license
//! - **EntangledGeneration** (64B): Generation counter incorporating license rotation
//! - **LicenseAuditCapsule** (256B): Q34 compliant hash-chain with license anchoring
//!
//! ## Security Model
//!
//! 1. **Cryptographic Entanglement**: State transitions XOR'd with SHA256(Ed25519_signature)[0..8]
//! 2. **Feature Dispatch**: Signature bits determine operation paths (no feature flags to patch)
//! 3. **Audit Anchoring**: Hash-chain includes license transform (tampering detectable)
//! 4. **Generation Binding**: Counter incorporates license rotation schedule
//!
//! ## Performance (B32 Targets)
//! - State transition: <15ns (single XOR + atomic)
//! - Feature operation: <25ns (signature bit check + dispatch)
//! - Audit append: <100ns (hash-chain with license anchor)
//! - Integrity verify: <50ns (transform validation)
//!
//! ## Legal Framework
//! - DMCA §1201 anti-circumvention protection (cryptographic access control)
//! - Trade secret: Computational entanglement architecture is confidential IP
//! - License terms: License IS computation, not external validation

pub mod entangled_capsule;
pub mod audit;
pub mod generation;

// Re-export main types
pub use entangled_capsule::{
    LicenseEntangledCapsule, License, LicenseError, LicenseFeatures, ComputationResult,
};
pub use audit::{LicenseAuditCapsule, LicenseAuditEntry, AuditAnchor};
pub use generation::{EntangledGeneration, RotationSchedule};
