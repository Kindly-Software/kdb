//! TOML Configuration Loader for SecurityConfig
//!
//! Provides loading of [`SecurityConfig`] from TOML configuration files,
//! enabling user-customizable security settings without recompilation.
//!
//! # Configuration File Location
//!
//! Default: `~/.config/kdb/security.toml`
//!
//! # Example Configuration
//!
//! ```toml
//! # KDB Security Configuration
//!
//! [security]
//! # Preset: "minimal", "standard", "paranoid", or "custom"
//! preset = "standard"
//!
//! [key_storage]
//! # Method: "env", "password_manager", "hsm", "encrypted_file"
//! method = "env"
//! # For env: variable name (default: KDB_OPERATOR_KEY)
//! env_var = "KDB_OPERATOR_KEY"
//! # For password_manager: CLI command to retrieve key
//! command = "op read 'op://Private/kdb-operator-key/private_key'"
//! # For hsm: PKCS#11 library path
//! pkcs11_path = "/usr/lib/softhsm/libsofthsm2.so"
//! # For encrypted_file: path to encrypted key file
//! key_file = "~/.config/kdb/operator.key.enc"
//!
//! [timeouts]
//! # Operator session timeout: "5m", "30m", "1h", "never"
//! session_timeout = "1h"
//! # Challenge expiry: "30s", "60s", "120s"
//! challenge_timeout = "30s"
//!
//! [audit]
//! # Level: "minimal", "standard", "verbose"
//! level = "standard"
//! # Log file path (optional, default: ~/.local/share/kdb/audit.log)
//! log_path = "~/.local/share/kdb/audit.log"
//!
//! [advanced]
//! # Require re-authentication for high-risk operations (write_memory, write_registers)
//! require_reauth_high_risk = false
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use kdb::access_control::{load_from_file, load_default, load_from_str};
//! use std::path::Path;
//!
//! // Load from default location (~/.config/kdb/security.toml)
//! let config = load_default().unwrap_or_default();
//!
//! // Load from specific file
//! let config = load_from_file(Path::new("/etc/kdb/security.toml"))?;
//!
//! // Load from string (for testing)
//! let toml = r#"
//! [security]
//! preset = "paranoid"
//! "#;
//! let config = load_from_str(toml)?;
//! ```
//!
//! # Framework Compliance
//!
//! - **Chaos**: Pure Rust, no unsafe code, minimal dependencies
//! - **T28**: Comprehensive test coverage (10+ tests)
//! - **Q34**: Configuration changes are audit-logged when applied

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use super::{AuditLevel, KeyStorageMethod, SecurityConfig, SecurityPreset};

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur when loading configuration from TOML files.
#[derive(Debug)]
pub enum ConfigLoadError {
    /// I/O error reading configuration file.
    IoError(std::io::Error),

    /// TOML parsing error.
    TomlParseError(toml::de::Error),

    /// Invalid security preset value.
    ///
    /// Valid values: "minimal", "standard", "paranoid", "custom"
    InvalidPreset(String),

    /// Invalid key storage method.
    ///
    /// Valid values: "env", "password_manager", "hsm", "encrypted_file"
    InvalidKeyStorageMethod(String),

    /// Invalid duration string.
    ///
    /// Valid formats: "30s", "5m", "1h", "never"
    InvalidDuration(String),

    /// Invalid audit level.
    ///
    /// Valid values: "minimal", "standard", "verbose"
    InvalidAuditLevel(String),

    /// Failed to expand path (e.g., ~ expansion failed).
    PathExpansionError(String),

    /// Missing required field for key storage method.
    MissingKeyStorageField {
        method: String,
        field: String,
    },
}

impl std::fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigLoadError::IoError(e) => write!(f, "I/O error reading config: {}", e),
            ConfigLoadError::TomlParseError(e) => write!(f, "TOML parse error: {}", e),
            ConfigLoadError::InvalidPreset(s) => {
                write!(f, "Invalid preset '{}' (expected: minimal, standard, paranoid, custom)", s)
            }
            ConfigLoadError::InvalidKeyStorageMethod(s) => {
                write!(f, "Invalid key storage method '{}' (expected: env, password_manager, hsm, encrypted_file)", s)
            }
            ConfigLoadError::InvalidDuration(s) => {
                write!(f, "Invalid duration '{}' (expected: 30s, 5m, 1h, never)", s)
            }
            ConfigLoadError::InvalidAuditLevel(s) => {
                write!(f, "Invalid audit level '{}' (expected: minimal, standard, verbose)", s)
            }
            ConfigLoadError::PathExpansionError(s) => {
                write!(f, "Path expansion failed: {}", s)
            }
            ConfigLoadError::MissingKeyStorageField { method, field } => {
                write!(f, "Key storage method '{}' requires field '{}'", method, field)
            }
        }
    }
}

impl std::error::Error for ConfigLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigLoadError::IoError(e) => Some(e),
            ConfigLoadError::TomlParseError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ConfigLoadError {
    fn from(err: std::io::Error) -> Self {
        ConfigLoadError::IoError(err)
    }
}

impl From<toml::de::Error> for ConfigLoadError {
    fn from(err: toml::de::Error) -> Self {
        ConfigLoadError::TomlParseError(err)
    }
}

// ============================================================================
// TOML Intermediate Structs (Serde Parsing)
// ============================================================================

/// Root TOML configuration structure.
#[derive(Debug, Deserialize, Default)]
struct TomlConfig {
    security: Option<SecuritySection>,
    key_storage: Option<KeyStorageSection>,
    timeouts: Option<TimeoutsSection>,
    audit: Option<AuditSection>,
    advanced: Option<AdvancedSection>,
}

/// Security section: preset selection.
#[derive(Debug, Deserialize, Default)]
struct SecuritySection {
    /// Security preset: "minimal", "standard", "paranoid", or "custom"
    preset: Option<String>,
}

/// Key storage section: method and method-specific fields.
#[derive(Debug, Deserialize, Default)]
struct KeyStorageSection {
    /// Method: "env", "password_manager", "hsm", "encrypted_file"
    method: Option<String>,
    /// Environment variable name (for method="env")
    env_var: Option<String>,
    /// CLI command to retrieve key (for method="password_manager")
    command: Option<String>,
    /// PKCS#11 library path (for method="hsm")
    pkcs11_path: Option<String>,
    /// Encrypted key file path (for method="encrypted_file")
    key_file: Option<String>,
}

/// Timeouts section: session and challenge timeouts.
#[derive(Debug, Deserialize, Default)]
struct TimeoutsSection {
    /// Session timeout: "5m", "30m", "1h", "never"
    session_timeout: Option<String>,
    /// Challenge timeout: "30s", "60s", "120s"
    challenge_timeout: Option<String>,
}

/// Audit section: logging level and path.
#[derive(Debug, Deserialize, Default)]
struct AuditSection {
    /// Audit level: "minimal", "standard", "verbose"
    level: Option<String>,
    /// Audit log file path
    log_path: Option<String>,
}

/// Advanced section: additional security options.
#[derive(Debug, Deserialize, Default)]
struct AdvancedSection {
    /// Require re-authentication for high-risk operations
    require_reauth_high_risk: Option<bool>,
}

// ============================================================================
// Public API
// ============================================================================

/// Load [`SecurityConfig`] from a TOML file.
///
/// Parses the specified TOML file and converts it to a `SecurityConfig`.
/// Path expansion (e.g., `~` to home directory) is automatically performed
/// on all path fields.
///
/// # Arguments
///
/// * `path` - Path to the TOML configuration file
///
/// # Returns
///
/// * `Ok(SecurityConfig)` - Successfully parsed configuration
/// * `Err(ConfigLoadError)` - File not found, parse error, or invalid values
///
/// # Example
///
/// ```rust,ignore
/// use kdb::access_control::load_from_file;
/// use std::path::Path;
///
/// let config = load_from_file(Path::new("/etc/kdb/security.toml"))?;
/// println!("Loaded preset: {:?}", config.preset);
/// ```
pub fn load_from_file(path: &Path) -> Result<SecurityConfig, ConfigLoadError> {
    let content = std::fs::read_to_string(path)?;
    load_from_str(&content)
}

/// Load [`SecurityConfig`] from the default location.
///
/// Default location: `~/.config/kdb/security.toml`
///
/// If the file does not exist, returns `SecurityConfig::standard()` as the default.
///
/// # Returns
///
/// * `Ok(SecurityConfig)` - Successfully parsed configuration or default
/// * `Err(ConfigLoadError)` - Parse error or invalid values (not returned for missing file)
///
/// # Example
///
/// ```rust,ignore
/// use kdb::access_control::load_default;
///
/// // Load from ~/.config/kdb/security.toml or use defaults
/// let config = load_default()?;
/// ```
pub fn load_default() -> Result<SecurityConfig, ConfigLoadError> {
    let default_path = expand_path("~/.config/kdb/security.toml")?;

    if !default_path.exists() {
        // Return default config if file doesn't exist
        return Ok(SecurityConfig::standard());
    }

    load_from_file(&default_path)
}

/// Load [`SecurityConfig`] from a TOML string.
///
/// Useful for testing or embedding configuration in other formats.
///
/// # Arguments
///
/// * `toml_content` - TOML configuration as a string
///
/// # Returns
///
/// * `Ok(SecurityConfig)` - Successfully parsed configuration
/// * `Err(ConfigLoadError)` - Parse error or invalid values
///
/// # Example
///
/// ```rust
/// use kdb::access_control::load_from_str;
///
/// let config = load_from_str(r#"
/// [security]
/// preset = "standard"
///
/// [timeouts]
/// session_timeout = "30m"
/// "#).unwrap();
///
/// assert_eq!(config.session_timeout, Some(std::time::Duration::from_secs(1800)));
/// ```
pub fn load_from_str(toml_content: &str) -> Result<SecurityConfig, ConfigLoadError> {
    let toml_config: TomlConfig = toml::from_str(toml_content)?;
    convert_toml_to_config(toml_config)
}

// ============================================================================
// Internal Helpers
// ============================================================================

/// Expand `~` to home directory in path strings.
///
/// # Arguments
///
/// * `path` - Path string potentially starting with `~/`
///
/// # Returns
///
/// * `Ok(PathBuf)` - Expanded path
/// * `Err(ConfigLoadError)` - HOME environment variable not set
fn expand_path(path: &str) -> Result<PathBuf, ConfigLoadError> {
    if path.starts_with("~/") {
        let home = std::env::var("HOME")
            .map_err(|_| ConfigLoadError::PathExpansionError("HOME environment variable not set".into()))?;
        Ok(PathBuf::from(format!("{}{}", home, &path[1..])))
    } else if path == "~" {
        let home = std::env::var("HOME")
            .map_err(|_| ConfigLoadError::PathExpansionError("HOME environment variable not set".into()))?;
        Ok(PathBuf::from(home))
    } else {
        Ok(PathBuf::from(path))
    }
}

/// Parse duration string to `Option<Duration>`.
///
/// Supported formats:
/// - `"never"` -> `None`
/// - `"30s"` -> 30 seconds
/// - `"5m"` -> 5 minutes
/// - `"1h"` -> 1 hour
/// - `"2d"` -> 2 days
///
/// # Arguments
///
/// * `s` - Duration string
///
/// # Returns
///
/// * `Ok(Option<Duration>)` - Parsed duration or `None` for "never"
/// * `Err(ConfigLoadError)` - Invalid format
fn parse_duration(s: &str) -> Result<Option<Duration>, ConfigLoadError> {
    let s = s.trim().to_lowercase();

    if s == "never" || s == "none" || s.is_empty() {
        return Ok(None);
    }

    // Extract numeric part and unit
    let (num_str, unit) = if s.ends_with('s') {
        (&s[..s.len()-1], "s")
    } else if s.ends_with('m') {
        (&s[..s.len()-1], "m")
    } else if s.ends_with('h') {
        (&s[..s.len()-1], "h")
    } else if s.ends_with('d') {
        (&s[..s.len()-1], "d")
    } else {
        return Err(ConfigLoadError::InvalidDuration(s));
    };

    let num: u64 = num_str.parse().map_err(|_| ConfigLoadError::InvalidDuration(s.clone()))?;

    let seconds = match unit {
        "s" => num,
        "m" => num * 60,
        "h" => num * 3600,
        "d" => num * 86400,
        _ => return Err(ConfigLoadError::InvalidDuration(s)),
    };

    Ok(Some(Duration::from_secs(seconds)))
}

/// Parse preset string to [`SecurityPreset`].
fn parse_preset(s: &str) -> Result<SecurityPreset, ConfigLoadError> {
    match s.trim().to_lowercase().as_str() {
        "minimal" => Ok(SecurityPreset::Minimal),
        "standard" => Ok(SecurityPreset::Standard),
        "paranoid" => Ok(SecurityPreset::Paranoid),
        "custom" => Ok(SecurityPreset::Standard), // Custom starts from Standard
        _ => Err(ConfigLoadError::InvalidPreset(s.to_string())),
    }
}

/// Parse audit level string to [`AuditLevel`].
fn parse_audit_level(s: &str) -> Result<AuditLevel, ConfigLoadError> {
    match s.trim().to_lowercase().as_str() {
        "minimal" => Ok(AuditLevel::Minimal),
        "standard" => Ok(AuditLevel::Standard),
        "verbose" => Ok(AuditLevel::Verbose),
        _ => Err(ConfigLoadError::InvalidAuditLevel(s.to_string())),
    }
}

/// Parse key storage section to [`KeyStorageMethod`].
fn parse_key_storage(section: &KeyStorageSection) -> Result<KeyStorageMethod, ConfigLoadError> {
    let method = section.method.as_deref().unwrap_or("env");

    match method.trim().to_lowercase().as_str() {
        "env" | "environment" | "environment_variable" => {
            // env_var is optional, defaults to KDB_OPERATOR_KEY
            Ok(KeyStorageMethod::EnvironmentVariable)
        }
        "password_manager" | "pw" | "pm" => {
            let command = section.command.as_ref().ok_or_else(|| {
                ConfigLoadError::MissingKeyStorageField {
                    method: method.to_string(),
                    field: "command".to_string(),
                }
            })?;
            Ok(KeyStorageMethod::PasswordManager {
                command: command.clone(),
            })
        }
        "hsm" | "hardware_security_module" => {
            let pkcs11_path = section.pkcs11_path.as_ref().ok_or_else(|| {
                ConfigLoadError::MissingKeyStorageField {
                    method: method.to_string(),
                    field: "pkcs11_path".to_string(),
                }
            })?;
            Ok(KeyStorageMethod::HardwareSecurityModule {
                pkcs11_path: pkcs11_path.clone(),
            })
        }
        "encrypted_file" | "file" => {
            let key_file = section.key_file.as_ref().ok_or_else(|| {
                ConfigLoadError::MissingKeyStorageField {
                    method: method.to_string(),
                    field: "key_file".to_string(),
                }
            })?;
            let expanded_path = expand_path(key_file)?;
            Ok(KeyStorageMethod::EncryptedFile {
                path: expanded_path,
            })
        }
        _ => Err(ConfigLoadError::InvalidKeyStorageMethod(method.to_string())),
    }
}

/// Convert parsed TOML config to [`SecurityConfig`].
fn convert_toml_to_config(toml: TomlConfig) -> Result<SecurityConfig, ConfigLoadError> {
    // Start with preset (defaults to Standard)
    let preset = if let Some(ref sec) = toml.security {
        if let Some(ref preset_str) = sec.preset {
            parse_preset(preset_str)?
        } else {
            SecurityPreset::Standard
        }
    } else {
        SecurityPreset::Standard
    };

    // Start with preset defaults
    let mut config = SecurityConfig::from_preset(preset);

    // Override key storage if specified
    if let Some(ref key_storage) = toml.key_storage {
        if key_storage.method.is_some() {
            config.key_storage = parse_key_storage(key_storage)?;
        }
    }

    // Override timeouts if specified
    if let Some(ref timeouts) = toml.timeouts {
        if let Some(ref session_timeout) = timeouts.session_timeout {
            config.session_timeout = parse_duration(session_timeout)?;
        }
        if let Some(ref challenge_timeout) = timeouts.challenge_timeout {
            if let Some(duration) = parse_duration(challenge_timeout)? {
                config.challenge_timeout = duration;
            }
        }
    }

    // Override audit settings if specified
    if let Some(ref audit) = toml.audit {
        if let Some(ref level) = audit.level {
            config.audit_level = parse_audit_level(level)?;
        }
        if let Some(ref log_path) = audit.log_path {
            let expanded = expand_path(log_path)?;
            config.audit_log_path = Some(expanded);
        }
    }

    // Override advanced settings if specified
    if let Some(ref advanced) = toml.advanced {
        if let Some(require_reauth) = advanced.require_reauth_high_risk {
            config.require_reauth_high_risk = require_reauth;
        }
    }

    Ok(config)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_complete_valid_config() {
        let toml = r#"
[security]
preset = "standard"

[key_storage]
method = "password_manager"
command = "op read 'op://Private/kdb-key/key'"

[timeouts]
session_timeout = "30m"
challenge_timeout = "60s"

[audit]
level = "verbose"
log_path = "/var/log/kdb/audit.log"

[advanced]
require_reauth_high_risk = true
"#;

        let config = load_from_str(toml).unwrap();

        assert_eq!(config.preset, SecurityPreset::Standard);
        assert!(matches!(config.key_storage, KeyStorageMethod::PasswordManager { .. }));
        assert_eq!(config.session_timeout, Some(Duration::from_secs(1800)));
        assert_eq!(config.challenge_timeout, Duration::from_secs(60));
        assert_eq!(config.audit_level, AuditLevel::Verbose);
        assert_eq!(config.audit_log_path, Some(PathBuf::from("/var/log/kdb/audit.log")));
        assert!(config.require_reauth_high_risk);
    }

    #[test]
    fn test_parse_minimal_config_only_preset() {
        let toml = r#"
[security]
preset = "paranoid"
"#;

        let config = load_from_str(toml).unwrap();

        // Should have paranoid defaults
        assert_eq!(config.preset, SecurityPreset::Paranoid);
        assert!(config.key_storage.is_hsm());
        assert_eq!(config.session_timeout, Some(Duration::from_secs(300)));
        assert_eq!(config.audit_level, AuditLevel::Verbose);
        assert!(config.require_reauth_high_risk);
    }

    #[test]
    fn test_parse_empty_config() {
        let toml = "";

        let config = load_from_str(toml).unwrap();

        // Should have standard defaults
        assert_eq!(config.preset, SecurityPreset::Standard);
        assert_eq!(config.session_timeout, Some(Duration::from_secs(3600)));
    }

    #[test]
    fn test_parse_all_duration_formats() {
        // Seconds
        assert_eq!(parse_duration("30s").unwrap(), Some(Duration::from_secs(30)));
        assert_eq!(parse_duration("120s").unwrap(), Some(Duration::from_secs(120)));

        // Minutes
        assert_eq!(parse_duration("5m").unwrap(), Some(Duration::from_secs(300)));
        assert_eq!(parse_duration("30m").unwrap(), Some(Duration::from_secs(1800)));

        // Hours
        assert_eq!(parse_duration("1h").unwrap(), Some(Duration::from_secs(3600)));
        assert_eq!(parse_duration("24h").unwrap(), Some(Duration::from_secs(86400)));

        // Days
        assert_eq!(parse_duration("1d").unwrap(), Some(Duration::from_secs(86400)));
        assert_eq!(parse_duration("7d").unwrap(), Some(Duration::from_secs(604800)));

        // Never
        assert_eq!(parse_duration("never").unwrap(), None);
        assert_eq!(parse_duration("NEVER").unwrap(), None);
        assert_eq!(parse_duration("none").unwrap(), None);
    }

    #[test]
    fn test_invalid_preset_error() {
        let toml = r#"
[security]
preset = "ultra_secure"
"#;

        let result = load_from_str(toml);
        assert!(matches!(result, Err(ConfigLoadError::InvalidPreset(_))));
    }

    #[test]
    fn test_invalid_duration_error() {
        let toml = r#"
[timeouts]
session_timeout = "invalid"
"#;

        let result = load_from_str(toml);
        assert!(matches!(result, Err(ConfigLoadError::InvalidDuration(_))));
    }

    #[test]
    fn test_invalid_audit_level_error() {
        let toml = r#"
[audit]
level = "extreme"
"#;

        let result = load_from_str(toml);
        assert!(matches!(result, Err(ConfigLoadError::InvalidAuditLevel(_))));
    }

    #[test]
    fn test_invalid_key_storage_method_error() {
        let toml = r#"
[key_storage]
method = "blockchain"
"#;

        let result = load_from_str(toml);
        assert!(matches!(result, Err(ConfigLoadError::InvalidKeyStorageMethod(_))));
    }

    #[test]
    fn test_missing_key_storage_field_error() {
        let toml = r#"
[key_storage]
method = "password_manager"
# Missing 'command' field
"#;

        let result = load_from_str(toml);
        assert!(matches!(result, Err(ConfigLoadError::MissingKeyStorageField { .. })));
    }

    #[test]
    fn test_path_expansion() {
        // Note: This test requires HOME to be set
        if std::env::var("HOME").is_ok() {
            let home = std::env::var("HOME").unwrap();

            let expanded = expand_path("~/test/path").unwrap();
            assert_eq!(expanded, PathBuf::from(format!("{}/test/path", home)));

            let expanded = expand_path("~").unwrap();
            assert_eq!(expanded, PathBuf::from(&home));

            // Absolute path should be unchanged
            let expanded = expand_path("/etc/kdb/config.toml").unwrap();
            assert_eq!(expanded, PathBuf::from("/etc/kdb/config.toml"));

            // Relative path should be unchanged
            let expanded = expand_path("config.toml").unwrap();
            assert_eq!(expanded, PathBuf::from("config.toml"));
        }
    }

    #[test]
    fn test_key_storage_env_method() {
        let toml = r#"
[key_storage]
method = "env"
env_var = "MY_CUSTOM_KEY"
"#;

        let config = load_from_str(toml).unwrap();
        assert!(matches!(config.key_storage, KeyStorageMethod::EnvironmentVariable));
    }

    #[test]
    fn test_key_storage_hsm_method() {
        let toml = r#"
[key_storage]
method = "hsm"
pkcs11_path = "/usr/lib/softhsm/libsofthsm2.so"
"#;

        let config = load_from_str(toml).unwrap();
        assert!(matches!(config.key_storage, KeyStorageMethod::HardwareSecurityModule { .. }));
        if let KeyStorageMethod::HardwareSecurityModule { pkcs11_path } = &config.key_storage {
            assert_eq!(pkcs11_path, "/usr/lib/softhsm/libsofthsm2.so");
        }
    }

    #[test]
    fn test_key_storage_encrypted_file_method() {
        let toml = r#"
[key_storage]
method = "encrypted_file"
key_file = "/tmp/test.key.enc"
"#;

        let config = load_from_str(toml).unwrap();
        assert!(matches!(config.key_storage, KeyStorageMethod::EncryptedFile { .. }));
        if let KeyStorageMethod::EncryptedFile { path } = &config.key_storage {
            assert_eq!(path, &PathBuf::from("/tmp/test.key.enc"));
        }
    }

    #[test]
    fn test_preset_case_insensitive() {
        for (input, expected) in [
            ("minimal", SecurityPreset::Minimal),
            ("MINIMAL", SecurityPreset::Minimal),
            ("Minimal", SecurityPreset::Minimal),
            ("standard", SecurityPreset::Standard),
            ("STANDARD", SecurityPreset::Standard),
            ("paranoid", SecurityPreset::Paranoid),
            ("PARANOID", SecurityPreset::Paranoid),
        ] {
            let toml = format!(r#"[security]
preset = "{}"
"#, input);
            let config = load_from_str(&toml).unwrap();
            assert_eq!(config.preset, expected, "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_audit_level_case_insensitive() {
        for (input, expected) in [
            ("minimal", AuditLevel::Minimal),
            ("MINIMAL", AuditLevel::Minimal),
            ("standard", AuditLevel::Standard),
            ("STANDARD", AuditLevel::Standard),
            ("verbose", AuditLevel::Verbose),
            ("VERBOSE", AuditLevel::Verbose),
        ] {
            let toml = format!(r#"[audit]
level = "{}"
"#, input);
            let config = load_from_str(&toml).unwrap();
            assert_eq!(config.audit_level, expected, "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_key_storage_method_aliases() {
        // Test various aliases for key storage methods
        for method in ["env", "environment", "environment_variable"] {
            let toml = format!(r#"[key_storage]
method = "{}"
"#, method);
            let config = load_from_str(&toml).unwrap();
            assert!(matches!(config.key_storage, KeyStorageMethod::EnvironmentVariable));
        }

        for method in ["password_manager", "pw", "pm"] {
            let toml = format!(r#"[key_storage]
method = "{}"
command = "test command"
"#, method);
            let config = load_from_str(&toml).unwrap();
            assert!(matches!(config.key_storage, KeyStorageMethod::PasswordManager { .. }));
        }

        for method in ["hsm", "hardware_security_module"] {
            let toml = format!(r#"[key_storage]
method = "{}"
pkcs11_path = "/lib/test.so"
"#, method);
            let config = load_from_str(&toml).unwrap();
            assert!(matches!(config.key_storage, KeyStorageMethod::HardwareSecurityModule { .. }));
        }

        for method in ["encrypted_file", "file"] {
            let toml = format!(r#"[key_storage]
method = "{}"
key_file = "/tmp/test.key"
"#, method);
            let config = load_from_str(&toml).unwrap();
            assert!(matches!(config.key_storage, KeyStorageMethod::EncryptedFile { .. }));
        }
    }

    #[test]
    fn test_error_display() {
        let errors = vec![
            ConfigLoadError::InvalidPreset("bad".into()),
            ConfigLoadError::InvalidKeyStorageMethod("bad".into()),
            ConfigLoadError::InvalidDuration("bad".into()),
            ConfigLoadError::InvalidAuditLevel("bad".into()),
            ConfigLoadError::PathExpansionError("HOME not set".into()),
            ConfigLoadError::MissingKeyStorageField {
                method: "hsm".into(),
                field: "pkcs11_path".into(),
            },
        ];

        for error in errors {
            let display = format!("{}", error);
            assert!(!display.is_empty(), "Error display should not be empty");
        }
    }

    #[test]
    fn test_partial_override() {
        // Start with paranoid preset but override some settings
        let toml = r#"
[security]
preset = "paranoid"

[timeouts]
session_timeout = "2h"

[advanced]
require_reauth_high_risk = false
"#;

        let config = load_from_str(toml).unwrap();

        // Preset should be paranoid
        assert_eq!(config.preset, SecurityPreset::Paranoid);
        // But session timeout should be overridden
        assert_eq!(config.session_timeout, Some(Duration::from_secs(7200)));
        // And require_reauth should be overridden
        assert!(!config.require_reauth_high_risk);
        // But HSM requirement should still be from paranoid preset
        assert!(config.key_storage.is_hsm());
        // And audit level should still be verbose
        assert_eq!(config.audit_level, AuditLevel::Verbose);
    }

    #[test]
    fn test_custom_preset_uses_standard_base() {
        let toml = r#"
[security]
preset = "custom"
"#;

        let config = load_from_str(toml).unwrap();

        // Custom should use standard as base
        assert_eq!(config.preset, SecurityPreset::Standard);
        assert_eq!(config.session_timeout, Some(Duration::from_secs(3600)));
    }
}
