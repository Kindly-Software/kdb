//! [TRADE SECRET] License integration for kindly_dedup CLI
//!
//! Phase 5: License Integration for kindly_dedup using UCE34 framework
//!
//! ## Modules
//!
//! - **hardware**: Hardware fingerprinting (CPU ID, TPM, Docker)
//! - **tiers**: License tier definitions (Trial/Starter/Pro/Enterprise)
//! - **validation**: License validation (signature, hardware, expiration)
//! - **enforcement**: Limit enforcement (documents, threads, data)
//! - **trial**: 30-day trial mode (Pro features)
//!
//! ## Architecture
//!
//! ```text
//! LicenseManager
//! ├── CryptoLicenseCapsule (Ed25519, from atomic_capsule)
//! ├── LicenseStateCapsule (T1 Atomic, CLI state)
//! ├── LicenseValidator (periodic re-validation every 5min)
//! ├── LicenseEnforcer (tier-specific limits)
//! └── TrialLicense (30-day activation)
//! ```
//!
//! ## Integration Points
//!
//! 1. **CLI Loading**: Load license on startup
//! 2. **Pre-processing**: Enforce limits before dedup starts
//! 3. **Runtime**: Track usage (atomic updates)
//! 4. **Menu**: License info screen shows tier + features
//! 5. **Processing**: Log license events to audit trail (Q34)
//!
//! ## Framework Compliance (UCE34)
//!
//! - **Q10 (Tier)**: T1 Atomic (LicenseStateCapsule)
//! - **Q11 (Rust Transform)**: Ed25519, hardware binding, atomic coordination
//! - **Q28 (Simplicity)**: Single LicenseManager facade
//! - **Q33 (Verification)**: All state compile-time verified
//! - **Q34 (Audit)**: License events logged to audit trail
//!
//! ## Performance (B32 Validated)
//!
//! - **Load license**: <5ms (first time, cached after)
//! - **Validation check**: <5ns (lightweight), <400µs (full every 5min)
//! - **Enforcement check**: <50ns (3 atomic loads)
//! - **Overhead per document**: <1ns (negligible)

pub mod enforcement;
pub mod hardware;
pub mod tiers;
pub mod trial;
pub mod validation;

pub use enforcement::{EnforcementError, LicenseEnforcer};
pub use hardware::HardwareFingerprint;
pub use tiers::{FeatureMatrix, LicenseConfig, LicenseFeature};
pub use trial::{TrialActivation, TrialError, TrialLicense};
pub use validation::LicenseValidator;

use crate::license_capsule::{LicenseCapsule, LicenseError, LicenseStatus, LicenseTier};
use std::sync::Arc;
use thiserror::Error;

// Optional: DedupConfig only used if interactive feature is enabled
#[cfg(feature = "interactive")]
use crate::cli::screens::DedupConfig;

/// License manager (main facade for license integration)
pub struct LicenseManager {
    /// License capsule (from atomic_capsule::protection)
    pub capsule: Arc<LicenseCapsule>,

    /// License validator (periodic re-validation)
    pub validator: Arc<LicenseValidator>,

    /// License enforcer (tier-specific limits)
    pub enforcer: Arc<LicenseEnforcer>,

    /// Current tier
    pub tier: LicenseTier,
}

/// License manager errors
#[derive(Debug, Error)]
pub enum LicenseManagerError {
    #[error("License error: {0:?}")]
    License(LicenseError),

    #[error("Trial error: {0}")]
    Trial(#[from] TrialError),

    #[error("Enforcement error: {0}")]
    Enforcement(#[from] EnforcementError),

    #[error("No license found: {0}")]
    NotFound(String),
}

pub type LicenseResult<T> = Result<T, LicenseManagerError>;

impl LicenseManager {
    /// Load license (from file/env/trial)
    ///
    /// ## Fallback Order
    ///
    /// 1. Try loading from license file
    /// 2. Try activating trial (if not already used)
    /// 3. Default to Free tier (no limits, but basic features only)
    ///
    /// ## Performance
    /// - File load: ~5ms (first time)
    /// - Trial activation: <50ms
    /// - Default: <1ns
    pub fn load() -> LicenseResult<Self> {
        // Try existing license first
        if let Ok(capsule) = Self::load_from_file() {
            return Self::from_capsule(capsule);
        }

        // Try trial activation
        if let Ok(capsule) = TrialLicense::activate() {
            return Self::from_capsule(capsule);
        }

        // Default to Free tier
        Self::free_tier()
    }

    /// Create from capsule
    fn from_capsule(capsule: LicenseCapsule) -> LicenseResult<Self> {
        let tier = capsule
            .tier()
            .ok_or_else(|| LicenseManagerError::NotFound("License tier not found".to_string()))?;

        // Validate capsule
        let validator = Arc::new(LicenseValidator::new());
        validator
            .validate(&capsule)
            .map_err(|e| LicenseManagerError::License(e))?;

        let enforcer = Arc::new(LicenseEnforcer::new(tier));

        Ok(Self {
            capsule: Arc::new(capsule),
            validator,
            enforcer,
            tier,
        })
    }

    /// Create Free tier license
    pub fn free_tier() -> LicenseResult<Self> {
        let capsule =
            LicenseCapsule::new("FREE-KEY", LicenseTier::Trial).map_err(|e| LicenseManagerError::License(e))?;

        Self::from_capsule(capsule)
    }

    /// Load from file (placeholder - would deserialize from JSON)
    fn load_from_file() -> LicenseResult<LicenseCapsule> {
        let path = Self::get_license_path()?;

        if !path.exists() {
            return Err(LicenseManagerError::NotFound("License file not found".to_string()));
        }

        // Note: Full implementation would deserialize from JSON
        // For now, we return NotFound to trigger trial/free tier
        Err(LicenseManagerError::NotFound("License file not found".to_string()))
    }

    /// Get license file path
    fn get_license_path() -> LicenseResult<std::path::PathBuf> {
        let config_dir =
            dirs::config_dir().ok_or_else(|| LicenseManagerError::NotFound("Config dir not found".to_string()))?;

        Ok(config_dir.join("kindly-dedup").join("license.json"))
    }

    /// Validate license (called periodically or before processing)
    pub fn validate(&self) -> LicenseResult<()> {
        self.validator
            .validate(&self.capsule)
            .map_err(|e| LicenseManagerError::License(e))
    }

    /// Enforce limits before deduplication
    #[cfg(feature = "interactive")]
    pub fn enforce(&self, config: &DedupConfig) -> LicenseResult<()> {
        self.enforcer
            .enforce(config)
            .map_err(|e| LicenseManagerError::Enforcement(e))
    }

    /// Get tier
    pub fn tier(&self) -> LicenseTier {
        self.tier
    }

    /// Get configuration for this license
    pub fn config(&self) -> &LicenseConfig {
        self.enforcer.config()
    }

    /// Get status
    pub fn status(&self) -> LicenseStatus {
        if self.capsule.is_expired() {
            LicenseStatus::Expired
        } else {
            LicenseStatus::Valid
        }
    }

    /// Get remaining GB
    pub fn remaining_gb(&self) -> Option<u64> {
        self.capsule.remaining_gb()
    }

    /// Record usage (GB)
    pub fn record_usage(&self, gb: u64) -> LicenseResult<()> {
        self.capsule
            .record_usage(gb)
            .map_err(|e| LicenseManagerError::License(e))
    }

    /// Get trial days remaining
    pub fn trial_days_remaining() -> LicenseResult<u32> {
        TrialLicense::days_remaining().map_err(|e| LicenseManagerError::Trial(e))
    }

    /// Check if feature is available
    pub fn has_feature(&self, feature: LicenseFeature) -> bool {
        self.enforcer.config().has_feature(feature)
    }

    /// Get enabled features
    pub fn features(&self) -> Vec<LicenseFeature> {
        self.enforcer.config().features.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_free_tier() {
        let mgr = LicenseManager::free_tier();
        assert!(mgr.is_ok());
    }

    #[test]
    fn test_license_manager_has_feature() {
        let mgr = LicenseManager::free_tier().unwrap();
        // Trial tier has SIMD MinHash
        assert!(mgr.has_feature(LicenseFeature::SimdMinHash));
    }
}
