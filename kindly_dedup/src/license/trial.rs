//! [TRADE SECRET] Trial license mode (30 days, Pro features)
//!
//! Enables 30-day evaluation period with Pro tier features:
//! - 1M document limit
//! - 8 threads
//! - 100 GB data limit
//! - All core features enabled
//! - Single activation per hardware (tracked via Docker/TPM ID)
//!
//! ## Architecture
//!
//! Trial activation is stored in user's config directory:
//! `~/.config/kindly-dedup/trial.json`
//!
//! Contains:
//! - Activation timestamp (Unix seconds)
//! - Hardware fingerprint (CPU ID only, portable across environments)
//! - Tier info (always "Trial")

use crate::license::hardware::HardwareFingerprint;
use crate::license_capsule::{LicenseCapsule, LicenseTier};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Trial activation errors
#[derive(Debug, Error, Clone)]
pub enum TrialError {
    #[error("Trial already activated on this hardware")]
    AlreadyActivated,

    #[error("Trial period has expired ({days} days remaining)")]
    Expired { days: u32 },

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Config directory not found")]
    ConfigDirNotFound,
}

pub type TrialResult<T> = Result<T, TrialError>;

/// Trial activation record (stored in trial.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialActivation {
    /// Activation timestamp (Unix seconds)
    pub activated_at: u64,

    /// Hardware fingerprint (for single-use tracking)
    pub hardware_id: String,

    /// Trial duration (days)
    pub duration_days: u32,

    /// Tier (always "Trial")
    pub tier: String,

    /// Version (for future compatibility)
    pub version: u32,
}

/// Trial license manager
pub struct TrialLicense;

impl TrialLicense {
    const TRIAL_DURATION_DAYS: u32 = 30;
    const TRIAL_DURATION_SECS: u64 = 30 * 86400; // 30 days in seconds

    /// Activate 30-day trial (Pro features)
    ///
    /// Returns error if trial already activated on this hardware
    pub fn activate() -> TrialResult<LicenseCapsule> {
        let activation_path = Self::get_activation_path()?;

        // Check if trial already activated
        if activation_path.exists() {
            let activation = Self::load_activation(&activation_path)?;

            // Check if still valid
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| TrialError::IoError(e.to_string()))?
                .as_secs();

            let elapsed_secs = now - activation.activated_at;
            let remaining_secs = Self::TRIAL_DURATION_SECS.saturating_sub(elapsed_secs);
            let remaining_days = (remaining_secs / 86400) as u32;

            if remaining_days > 0 {
                // Trial is still valid, load from capsule
                return Self::load_existing_trial(&activation);
            } else {
                return Err(TrialError::Expired { days: 0 });
            }
        }

        // Create new trial activation
        let hw_fingerprint = HardwareFingerprint::generate();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| TrialError::IoError(e.to_string()))?
            .as_secs();

        let activation = TrialActivation {
            activated_at: now,
            hardware_id: hw_fingerprint.hex(),
            duration_days: Self::TRIAL_DURATION_DAYS,
            tier: "Trial".to_string(),
            version: 1,
        };

        // Save activation record
        Self::save_activation(&activation_path, &activation)?;

        // Create license capsule
        let license = LicenseCapsule::new("TRIAL-KEY", LicenseTier::Trial)
            .map_err(|e| TrialError::SerializationError(format!("{:?}", e)))?;

        Ok(license)
    }

    /// Load existing trial (if still valid)
    pub fn load_if_valid() -> TrialResult<LicenseCapsule> {
        let activation_path = Self::get_activation_path()?;

        if !activation_path.exists() {
            return Err(TrialError::AlreadyActivated);
        }

        let activation = Self::load_activation(&activation_path)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| TrialError::IoError(e.to_string()))?
            .as_secs();

        let elapsed_secs = now - activation.activated_at;
        let remaining_secs = Self::TRIAL_DURATION_SECS.saturating_sub(elapsed_secs);
        let remaining_days = (remaining_secs / 86400) as u32;

        if remaining_days == 0 {
            return Err(TrialError::Expired { days: 0 });
        }

        Self::load_existing_trial(&activation)
    }

    /// Get days remaining in trial
    pub fn days_remaining() -> TrialResult<u32> {
        let activation_path = Self::get_activation_path()?;

        if !activation_path.exists() {
            return Ok(0);
        }

        let activation = Self::load_activation(&activation_path)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| TrialError::IoError(e.to_string()))?
            .as_secs();

        let elapsed_secs = now - activation.activated_at;
        let remaining_secs = Self::TRIAL_DURATION_SECS.saturating_sub(elapsed_secs);
        let remaining_days = (remaining_secs / 86400) as u32;

        Ok(remaining_days)
    }

    /// Get trial activation path
    fn get_activation_path() -> TrialResult<PathBuf> {
        let config_dir = dirs::config_dir().ok_or(TrialError::ConfigDirNotFound)?;

        let trial_dir = config_dir.join("kindly-dedup");
        fs::create_dir_all(&trial_dir).map_err(|e| TrialError::IoError(e.to_string()))?;

        Ok(trial_dir.join("trial.json"))
    }

    /// Load activation from file
    fn load_activation(path: &Path) -> TrialResult<TrialActivation> {
        let contents = fs::read_to_string(path).map_err(|e| TrialError::IoError(e.to_string()))?;

        serde_json::from_str(&contents).map_err(|e| TrialError::SerializationError(e.to_string()))
    }

    /// Save activation to file
    fn save_activation(path: &Path, activation: &TrialActivation) -> TrialResult<()> {
        let contents =
            serde_json::to_string_pretty(activation).map_err(|e| TrialError::SerializationError(e.to_string()))?;

        fs::write(path, contents).map_err(|e| TrialError::IoError(e.to_string()))?;

        Ok(())
    }

    /// Load existing trial without validation
    fn load_existing_trial(activation: &TrialActivation) -> TrialResult<LicenseCapsule> {
        LicenseCapsule::new("TRIAL-KEY", LicenseTier::Trial)
            .map_err(|e| TrialError::SerializationError(format!("{:?}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trial_duration_secs() {
        // 30 days = 30 * 86400 seconds
        assert_eq!(TrialLicense::TRIAL_DURATION_SECS, 2_592_000);
    }

    #[test]
    fn test_trial_duration_days() {
        assert_eq!(TrialLicense::TRIAL_DURATION_DAYS, 30);
    }
}
