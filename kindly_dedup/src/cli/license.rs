// [TRADE SECRET] License validation and management for kindly_dedup CLI

use crate::license_capsule::{LicenseCapsule, LicenseError, LicenseStatus, LicenseTier};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// License CLI errors
#[derive(Debug, Error)]
pub enum LicenseCliError {
    #[error("License file not found: {0}")]
    LicenseNotFound(String),

    #[error("License validation failed: {0}")]
    ValidationFailed(String),

    #[error("License is expired")]
    Expired,

    #[error("License is revoked")]
    Revoked,

    #[error("Invalid license format: {0}")]
    InvalidFormat(String),

    #[error("License key format invalid")]
    InvalidKeyFormat,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] toml::de::Error),

    #[error("GB limit exceeded")]
    LimitExceeded,

    #[error("License error: {0}")]
    LicenseError(String),
}

pub type LicenseCliResult<T> = Result<T, LicenseCliError>;

/// License configuration (stored in ~/.kindly-dedup/license.toml)
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct LicenseConfig {
    pub key: String,
    pub tier: String,
    pub created_at: String,
}

/// Load license from file or environment variable
pub fn load_license(key: Option<&str>) -> LicenseCliResult<LicenseCapsule> {
    // 1. Try explicit command-line key
    if let Some(key) = key {
        return validate_license_key(key);
    }

    // 2. Try environment variable
    if let Ok(key) = std::env::var("KINDLY_DEDUP_LICENSE_KEY") {
        return validate_license_key(&key);
    }

    // 3. Try config file
    let config_path = get_license_config_path()?;
    if config_path.exists() {
        let config_str = fs::read_to_string(&config_path)?;
        let config: LicenseConfig = toml::from_str(&config_str).map_err(|e| LicenseCliError::SerializationError(e))?;
        return validate_license_key(&config.key);
    }

    // 4. No license found
    Err(LicenseCliError::LicenseNotFound(format!(
        "No license found. Provide via:\n  \
            1. --license-key option\n  \
            2. KINDLY_DEDUP_LICENSE_KEY env var\n  \
            3. {}/license.toml file",
        get_config_dir().display()
    )))
}

/// Validate license key format and create capsule
fn validate_license_key(key: &str) -> LicenseCliResult<LicenseCapsule> {
    // License key format: "KINDLY-<TIER>-<UUID>"
    // Example: "KINDLY-PRO-550e8400-e29b-41d4-a716-446655440000"

    let parts: Vec<&str> = key.split('-').collect();
    if parts.len() < 3 {
        return Err(LicenseCliError::InvalidFormat(
            "License key format: KINDLY-<TIER>-<UUID>".to_string(),
        ));
    }

    if parts[0] != "KINDLY" {
        return Err(LicenseCliError::InvalidFormat(
            "License key must start with KINDLY-".to_string(),
        ));
    }

    // Determine tier from key
    let tier = match parts[1] {
        "PRO" => LicenseTier::Pro,
        "STARTER" => LicenseTier::Starter,
        "ENTERPRISE" => LicenseTier::Enterprise,
        "TRIAL" => LicenseTier::Trial,
        _ => return Err(LicenseCliError::InvalidFormat("Unknown license tier".to_string())),
    };

    // Create license capsule
    LicenseCapsule::new(key, tier).map_err(|e| LicenseCliError::LicenseError(e.to_string()))
}

/// Validate license before processing
pub async fn validate_before_processing(license: &LicenseCapsule) -> LicenseCliResult<()> {
    match license
        .validate()
        .map_err(|e| LicenseCliError::LicenseError(e.to_string()))?
    {
        LicenseStatus::Valid => Ok(()),
        LicenseStatus::Expired => Err(LicenseCliError::Expired),
        LicenseStatus::Revoked => Err(LicenseCliError::Revoked),
    }
}

/// Record usage in license
pub async fn record_usage(license: &LicenseCapsule, gb: u64) -> LicenseCliResult<()> {
    license
        .record_usage(gb)
        .map_err(|e| LicenseCliError::LicenseError(e.to_string()))
}

/// Get remaining GB quota (or None if unlimited)
pub fn get_remaining_quota(license: &LicenseCapsule) -> Option<u64> {
    license.remaining_gb()
}

/// Save license to config file
pub fn save_license(key: &str, tier: &str) -> LicenseCliResult<PathBuf> {
    let config_dir = get_config_dir();
    fs::create_dir_all(&config_dir)?;

    let config_path = config_dir.join("license.toml");

    let config = LicenseConfig {
        key: key.to_string(),
        tier: tier.to_string(),
        created_at: chrono::Local::now().to_rfc3339(),
    };

    let config_str = toml::to_string_pretty(&config).map_err(LicenseCliError::SerializationError)?;

    fs::write(&config_path, config_str)?;

    Ok(config_path)
}

/// Get license config directory
fn get_config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("KINDLY_DEDUP_CONFIG_DIR") {
        PathBuf::from(dir)
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".kindly-dedup")
    }
}

/// Get license config file path
fn get_license_config_path() -> LicenseCliResult<PathBuf> {
    let config_dir = get_config_dir();
    Ok(config_dir.join("license.toml"))
}

/// Print license info
pub fn print_license_info(license: &LicenseCapsule) {
    println!("╭─ kindly_dedup License ─╮");
    println!("│");
    if let Some(tier) = license.tier() {
        println!("│  Tier: {:?}", tier);
    }
    println!("│  Created: {}", format_timestamp(license.created()));
    println!("│  Expires: {}", format_timestamp(license.expiry()));

    if let Some(remaining) = license.remaining_gb() {
        println!("│  GB Used: {} / {}", license.used_gb(), remaining + license.used_gb());
        println!("│  GB Remaining: {}", remaining);
    } else {
        println!("│  GB Used: {} (unlimited)", license.used_gb());
    }

    println!("│  Last Used: {}", format_timestamp(license.last_used()));
    println!("│");
    println!("╰─────────────────────────╯");
}

/// Format unix timestamp as human-readable string
fn format_timestamp(ts: u64) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let d = UNIX_EPOCH + std::time::Duration::from_secs(ts);
    let datetime: chrono::DateTime<chrono::Local> = d.into();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_license_key_parsing() {
        let key = "KINDLY-PRO-550e8400-e29b-41d4-a716-446655440000";
        assert!(validate_license_key(key).is_ok());
    }

    #[test]
    fn test_invalid_license_key() {
        let key = "INVALID-PRO-123";
        assert!(validate_license_key(key).is_err());
    }

    #[test]
    fn test_config_dir() {
        let dir = get_config_dir();
        assert!(!dir.as_os_str().is_empty());
    }
}
