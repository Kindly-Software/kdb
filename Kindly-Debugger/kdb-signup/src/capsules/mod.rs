//! Computational Capsules for KDB Signup
//!
//! UCE34/Chaos compliant capsules - 100% lockfree, no mutex.
//!
//! # Available Capsules
//!
//! - [`EmailVerificationCapsule`] (T1 Atomic, 256B): Token generation and verification
//! - [`LicenseGeneratorCapsule`] (T1 Atomic, 512B): Ed25519 license key generation with promo tracking
//! - [`UserRegistrationCapsule`] (T1 Atomic, 256B): User signup with built-in rate limiting
//!
//! # Planned Capsules
//!
//! - `SignupCounterCapsule` (T1 Atomic): Atomic signup counter with generation
//! - `DisposableEmailCapsule` (T2 SIMD): Fast disposable email detection
//!
//! # Framework Compliance
//!
//! - All capsules use cache-aligned (64B/128B) layouts
//! - Generation counters for TOCTOU prevention
//! - DualAtomicU64 pattern for compound state
//! - Ed25519 signing for cryptographic security

pub mod email_verification;
pub mod license_generator;
pub mod user_registration;

// Re-export commonly used types
pub use email_verification::{
    EmailVerificationCapsule, VerificationError, VerificationStats, VerificationToken,
};
pub use license_generator::{
    LicenseError, LicenseGeneratorCapsule, LicenseKey, LicenseStats, SubscriptionTier,
    PROMO_DURATION_SECS,
};
pub use user_registration::{
    PendingUser, RegistrationStats, SignupError, UserRegistrationCapsule,
};

// Future capsule modules:
// pub mod signup_counter;
// pub mod disposable_email;
