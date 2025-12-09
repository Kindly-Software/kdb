//! Security Configuration Module
//!
//! User-configurable security settings for the Observer/Operator access control system.
//! Provides three security presets (Minimal, Standard, Paranoid) with customizable
//! parameters for key storage, session timeouts, and audit levels.
//!
//! # Security Presets
//!
//! | Preset   | Timeout | Key Storage   | Audit   | Re-auth High Risk |
//! |----------|---------|---------------|---------|-------------------|
//! | Minimal  | None    | Environment   | Minimal | No                |
//! | Standard | 1 hour  | Environment   | Standard| No                |
//! | Paranoid | 5 min   | HSM Required  | Verbose | Yes               |
//!
//! # Example
//!
//! ```rust
//! use kdb::access_control::{SecurityConfig, SecurityPreset, KeyStorageMethod};
//!
//! // Use a preset
//! let config = SecurityConfig::from_preset(SecurityPreset::Standard);
//!
//! // Or customize with builder pattern
//! let custom = SecurityConfig::minimal()
//!     .with_session_timeout(Some(std::time::Duration::from_secs(1800)))
//!     .with_key_storage(KeyStorageMethod::PasswordManager {
//!         command: "op read 'op://vault/kdb-key/key'".to_string(),
//!     });
//! ```
//!
//! # Framework Compliance
//!
//! - **Chaos**: Zero runtime overhead for config access (compile-time constants where possible)
//! - **T28 Q22-Q28**: Production-ready configuration with sensible defaults
//! - **Q34 Audit**: AuditLevel configuration for SOX/SOC2/GDPR/HIPAA compliance

use std::path::PathBuf;
use std::time::Duration;

/// Security preset levels for common deployment scenarios.
///
/// Presets provide sensible defaults for different security requirements:
/// - **Minimal**: Development and debugging (no timeout, env var storage)
/// - **Standard**: Normal production use (1hr timeout, env var storage)
/// - **Paranoid**: Regulated environments (5min timeout, HSM required)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum SecurityPreset {
    /// Minimal security for development environments.
    ///
    /// - No session timeout (sessions persist indefinitely)
    /// - Environment variable key storage (KDB_OPERATOR_KEY)
    /// - Minimal audit logging
    /// - No re-authentication for high-risk operations
    ///
    /// **WARNING**: Not recommended for production use.
    Minimal = 0,

    /// Standard security for typical production deployments.
    ///
    /// - 1 hour session timeout
    /// - Environment variable key storage (KDB_OPERATOR_KEY)
    /// - Standard audit logging (all operations logged)
    /// - No re-authentication for high-risk operations
    ///
    /// Suitable for most production environments without strict compliance requirements.
    #[default]
    Standard = 1,

    /// Paranoid security for compliance-regulated environments.
    ///
    /// - 5 minute session timeout
    /// - Hardware Security Module (HSM) required for key storage
    /// - Verbose audit logging (full context for each operation)
    /// - Re-authentication required for high-risk operations
    ///
    /// Required for SOX/SOC2/HIPAA regulated environments.
    Paranoid = 2,
}

impl SecurityPreset {
    /// Get the session timeout for this preset.
    ///
    /// - Minimal: None (no timeout)
    /// - Standard: 1 hour
    /// - Paranoid: 5 minutes
    #[inline]
    pub const fn session_timeout(&self) -> Option<Duration> {
        match self {
            SecurityPreset::Minimal => None,
            SecurityPreset::Standard => Some(Duration::from_secs(3600)), // 1 hour
            SecurityPreset::Paranoid => Some(Duration::from_secs(300)),  // 5 minutes
        }
    }

    /// Get the audit level for this preset.
    #[inline]
    pub const fn audit_level(&self) -> AuditLevel {
        match self {
            SecurityPreset::Minimal => AuditLevel::Minimal,
            SecurityPreset::Standard => AuditLevel::Standard,
            SecurityPreset::Paranoid => AuditLevel::Verbose,
        }
    }

    /// Check if high-risk operations require re-authentication.
    #[inline]
    pub const fn require_reauth_high_risk(&self) -> bool {
        matches!(self, SecurityPreset::Paranoid)
    }
}

/// Methods for secure operator key storage.
///
/// The operator's Ed25519 private key must be stored securely. This enum
/// defines the supported storage methods, ordered from least to most secure:
///
/// 1. **EnvironmentVariable** - Key in KDB_OPERATOR_KEY (development only)
/// 2. **PasswordManager** - Key retrieved via CLI command (e.g., 1Password, Bitwarden)
/// 3. **EncryptedFile** - Key in encrypted file with passphrase
/// 4. **HardwareSecurityModule** - Key stored in PKCS#11 HSM (most secure)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyStorageMethod {
    /// Operator key stored in environment variable KDB_OPERATOR_KEY.
    ///
    /// The key should be hex-encoded (64 characters for Ed25519 secret key).
    ///
    /// # Security Note
    ///
    /// Environment variables may be visible in /proc, shell history, and logs.
    /// Only recommended for development environments.
    ///
    /// # Example
    ///
    /// ```bash
    /// export KDB_OPERATOR_KEY="abc123...def456"
    /// ```
    EnvironmentVariable,

    /// Operator key retrieved via password manager CLI command.
    ///
    /// The command should output the hex-encoded key to stdout.
    /// Common password managers with CLI support:
    /// - 1Password (`op read 'op://vault/item/field'`)
    /// - Bitwarden (`bw get password kdb-operator-key`)
    /// - pass (`pass show kdb/operator-key`)
    ///
    /// # Example
    ///
    /// ```rust
    /// use kdb::access_control::KeyStorageMethod;
    ///
    /// let storage = KeyStorageMethod::PasswordManager {
    ///     command: "op read 'op://vault/kdb-key/key'".to_string(),
    /// };
    /// ```
    PasswordManager {
        /// Shell command to retrieve the key (executed via `sh -c`).
        command: String,
    },

    /// Operator key stored in Hardware Security Module via PKCS#11.
    ///
    /// HSMs provide the highest security level:
    /// - Key never leaves the hardware device
    /// - Tamper-resistant storage
    /// - Required for Paranoid preset
    ///
    /// # Supported HSMs
    ///
    /// - YubiKey (via PKCS#11)
    /// - Nitrokey
    /// - SoftHSM (for testing)
    /// - Cloud HSM (AWS CloudHSM, Azure Dedicated HSM, Google Cloud HSM)
    ///
    /// # Example
    ///
    /// ```rust
    /// use kdb::access_control::KeyStorageMethod;
    ///
    /// let storage = KeyStorageMethod::HardwareSecurityModule {
    ///     pkcs11_path: "/usr/lib/softhsm/libsofthsm2.so".to_string(),
    /// };
    /// ```
    HardwareSecurityModule {
        /// Path to PKCS#11 library (e.g., `/usr/lib/softhsm/libsofthsm2.so`).
        pkcs11_path: String,
    },

    /// Operator key stored in encrypted file with passphrase.
    ///
    /// The file is encrypted using AES-256-GCM with a key derived from
    /// the passphrase via Argon2id. Passphrase is prompted at session start.
    ///
    /// # File Format
    ///
    /// The encrypted file uses a custom format:
    /// - 16 bytes: Argon2id salt
    /// - 12 bytes: AES-GCM nonce
    /// - 32 bytes: Encrypted Ed25519 secret key
    /// - 16 bytes: AES-GCM auth tag
    ///
    /// # Example
    ///
    /// ```rust
    /// use kdb::access_control::KeyStorageMethod;
    /// use std::path::PathBuf;
    ///
    /// let storage = KeyStorageMethod::EncryptedFile {
    ///     path: PathBuf::from("/home/user/.kdb/operator.key.enc"),
    /// };
    /// ```
    EncryptedFile {
        /// Path to the encrypted key file.
        path: PathBuf,
    },
}

impl Default for KeyStorageMethod {
    /// Default to environment variable storage (simplest setup).
    #[inline]
    fn default() -> Self {
        KeyStorageMethod::EnvironmentVariable
    }
}

impl KeyStorageMethod {
    /// Check if this storage method requires user interaction (passphrase prompt).
    #[inline]
    pub const fn requires_interaction(&self) -> bool {
        matches!(
            self,
            KeyStorageMethod::EncryptedFile { .. }
                | KeyStorageMethod::HardwareSecurityModule { .. }
        )
    }

    /// Check if this storage method provides HSM-level security.
    #[inline]
    pub const fn is_hsm(&self) -> bool {
        matches!(self, KeyStorageMethod::HardwareSecurityModule { .. })
    }
}

/// Audit logging verbosity levels.
///
/// Controls the amount of detail recorded in the Q34 audit trail.
/// Higher levels provide more context for compliance audits but
/// consume more storage and may impact performance slightly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum AuditLevel {
    /// Minimal audit logging (errors and authentication events only).
    ///
    /// Records:
    /// - Session start/end
    /// - Mode transitions (Observer <-> Operator)
    /// - Authentication failures
    /// - Error conditions
    ///
    /// Suitable for development or low-security environments.
    Minimal = 0,

    /// Standard audit logging (all operations with basic context).
    ///
    /// Records everything in Minimal, plus:
    /// - All tool invocations
    /// - Command parameters (redacted where sensitive)
    /// - Operation timestamps
    /// - Session duration
    ///
    /// Suitable for most production environments.
    #[default]
    Standard = 1,

    /// Verbose audit logging (full context for compliance).
    ///
    /// Records everything in Standard, plus:
    /// - Full command parameters (including memory addresses, register values)
    /// - Stack traces for each operation
    /// - Memory access patterns
    /// - Performance metrics
    /// - Challenge-response details (nonces, signatures)
    ///
    /// Required for SOX/SOC2/GDPR/HIPAA compliance.
    Verbose = 2,
}

impl AuditLevel {
    /// Check if this level includes operation logging.
    #[inline]
    pub const fn logs_operations(&self) -> bool {
        !matches!(self, AuditLevel::Minimal)
    }

    /// Check if this level includes full context logging.
    #[inline]
    pub const fn logs_full_context(&self) -> bool {
        matches!(self, AuditLevel::Verbose)
    }
}

/// Complete security configuration for the access control system.
///
/// This struct holds all configurable security parameters. Use
/// `SecurityConfig::from_preset()` for common configurations or
/// the builder pattern for custom setups.
///
/// # Example
///
/// ```rust
/// use kdb::access_control::{SecurityConfig, SecurityPreset, KeyStorageMethod, AuditLevel};
/// use std::time::Duration;
/// use std::path::PathBuf;
///
/// // From preset
/// let standard = SecurityConfig::standard();
///
/// // Custom configuration
/// let custom = SecurityConfig::from_preset(SecurityPreset::Standard)
///     .with_session_timeout(Some(Duration::from_secs(1800))) // 30 minutes
///     .with_key_storage(KeyStorageMethod::PasswordManager {
///         command: "pass show kdb/key".to_string(),
///     })
///     .with_audit_level(AuditLevel::Verbose)
///     .with_audit_log_path(Some(PathBuf::from("/var/log/kdb/audit.log")));
/// ```
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// The security preset this configuration is based on.
    pub preset: SecurityPreset,

    /// Method for retrieving the operator's Ed25519 private key.
    pub key_storage: KeyStorageMethod,

    /// Session timeout duration. `None` means sessions never expire.
    ///
    /// After this duration without activity, the session expires and
    /// re-authentication is required.
    pub session_timeout: Option<Duration>,

    /// Timeout for challenge-response authentication.
    ///
    /// The operator must sign and return the challenge within this duration.
    /// Default: 30 seconds.
    pub challenge_timeout: Duration,

    /// Whether high-risk operations require re-authentication.
    ///
    /// High-risk operations include:
    /// - Memory writes
    /// - Register modifications
    /// - Continuing execution with modified state
    /// - Detaching from process
    pub require_reauth_high_risk: bool,

    /// Audit logging verbosity level.
    pub audit_level: AuditLevel,

    /// Path to write audit logs. `None` uses in-memory audit trail only.
    ///
    /// For compliance, this should point to a persistent, append-only location
    /// with appropriate access controls.
    pub audit_log_path: Option<PathBuf>,
}

impl SecurityConfig {
    /// Create a configuration from a security preset.
    ///
    /// This applies all preset defaults. Use builder methods to customize.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kdb::access_control::{SecurityConfig, SecurityPreset};
    ///
    /// let config = SecurityConfig::from_preset(SecurityPreset::Paranoid);
    /// assert!(config.require_reauth_high_risk);
    /// ```
    pub fn from_preset(preset: SecurityPreset) -> Self {
        let key_storage = match preset {
            SecurityPreset::Paranoid => KeyStorageMethod::HardwareSecurityModule {
                pkcs11_path: "/usr/lib/softhsm/libsofthsm2.so".to_string(),
            },
            _ => KeyStorageMethod::EnvironmentVariable,
        };

        Self {
            preset,
            key_storage,
            session_timeout: preset.session_timeout(),
            challenge_timeout: Duration::from_secs(30),
            require_reauth_high_risk: preset.require_reauth_high_risk(),
            audit_level: preset.audit_level(),
            audit_log_path: None,
        }
    }

    /// Create a minimal security configuration.
    ///
    /// Equivalent to `SecurityConfig::from_preset(SecurityPreset::Minimal)`.
    ///
    /// **WARNING**: Not recommended for production use.
    #[inline]
    pub fn minimal() -> Self {
        Self::from_preset(SecurityPreset::Minimal)
    }

    /// Create a standard security configuration.
    ///
    /// Equivalent to `SecurityConfig::from_preset(SecurityPreset::Standard)`.
    ///
    /// This is the default and recommended for most deployments.
    #[inline]
    pub fn standard() -> Self {
        Self::from_preset(SecurityPreset::Standard)
    }

    /// Create a paranoid security configuration.
    ///
    /// Equivalent to `SecurityConfig::from_preset(SecurityPreset::Paranoid)`.
    ///
    /// Required for SOX/SOC2/HIPAA regulated environments.
    #[inline]
    pub fn paranoid() -> Self {
        Self::from_preset(SecurityPreset::Paranoid)
    }

    /// Set the session timeout duration.
    ///
    /// Pass `None` for sessions that never expire (Minimal preset).
    ///
    /// # Example
    ///
    /// ```rust
    /// use kdb::access_control::SecurityConfig;
    /// use std::time::Duration;
    ///
    /// let config = SecurityConfig::standard()
    ///     .with_session_timeout(Some(Duration::from_secs(1800))); // 30 minutes
    /// ```
    #[inline]
    pub fn with_session_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.session_timeout = timeout;
        self
    }

    /// Set the key storage method.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kdb::access_control::{SecurityConfig, KeyStorageMethod};
    ///
    /// let config = SecurityConfig::standard()
    ///     .with_key_storage(KeyStorageMethod::PasswordManager {
    ///         command: "op read 'op://vault/kdb/key'".to_string(),
    ///     });
    /// ```
    #[inline]
    pub fn with_key_storage(mut self, method: KeyStorageMethod) -> Self {
        self.key_storage = method;
        self
    }

    /// Set the challenge-response timeout.
    ///
    /// This is the maximum time allowed for signing and returning a challenge.
    /// Default is 30 seconds; reduce for higher security.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kdb::access_control::SecurityConfig;
    /// use std::time::Duration;
    ///
    /// let config = SecurityConfig::paranoid()
    ///     .with_challenge_timeout(Duration::from_secs(15)); // 15 seconds
    /// ```
    #[inline]
    pub fn with_challenge_timeout(mut self, timeout: Duration) -> Self {
        self.challenge_timeout = timeout;
        self
    }

    /// Set whether high-risk operations require re-authentication.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kdb::access_control::SecurityConfig;
    ///
    /// let config = SecurityConfig::standard()
    ///     .with_require_reauth_high_risk(true);
    /// ```
    #[inline]
    pub fn with_require_reauth_high_risk(mut self, require: bool) -> Self {
        self.require_reauth_high_risk = require;
        self
    }

    /// Set the audit logging level.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kdb::access_control::{SecurityConfig, AuditLevel};
    ///
    /// let config = SecurityConfig::standard()
    ///     .with_audit_level(AuditLevel::Verbose);
    /// ```
    #[inline]
    pub fn with_audit_level(mut self, level: AuditLevel) -> Self {
        self.audit_level = level;
        self
    }

    /// Set the audit log file path.
    ///
    /// Pass `None` for in-memory audit trail only.
    ///
    /// # Example
    ///
    /// ```rust
    /// use kdb::access_control::SecurityConfig;
    /// use std::path::PathBuf;
    ///
    /// let config = SecurityConfig::paranoid()
    ///     .with_audit_log_path(Some(PathBuf::from("/var/log/kdb/audit.log")));
    /// ```
    #[inline]
    pub fn with_audit_log_path(mut self, path: Option<PathBuf>) -> Self {
        self.audit_log_path = path;
        self
    }

    /// Validate the configuration for internal consistency.
    ///
    /// Returns `Err` if the configuration is invalid:
    /// - Paranoid preset without HSM key storage
    /// - Challenge timeout greater than session timeout
    /// - Invalid file paths
    ///
    /// # Example
    ///
    /// ```rust
    /// use kdb::access_control::{SecurityConfig, SecurityPreset, KeyStorageMethod};
    ///
    /// let mut config = SecurityConfig::paranoid();
    /// config.key_storage = KeyStorageMethod::EnvironmentVariable; // Invalid!
    ///
    /// assert!(config.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<(), SecurityConfigError> {
        // Paranoid preset requires HSM
        if self.preset == SecurityPreset::Paranoid && !self.key_storage.is_hsm() {
            return Err(SecurityConfigError::ParanoidRequiresHsm);
        }

        // Challenge timeout must be less than session timeout (if session timeout exists)
        if let Some(session_timeout) = self.session_timeout {
            if self.challenge_timeout > session_timeout {
                return Err(SecurityConfigError::ChallengeTimeoutTooLong);
            }
        }

        // Validate audit log path if specified
        if let Some(ref path) = self.audit_log_path {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() && !parent.exists() {
                    return Err(SecurityConfigError::AuditLogDirNotFound(
                        parent.to_path_buf(),
                    ));
                }
            }
        }

        Ok(())
    }
}

impl Default for SecurityConfig {
    /// Default to Standard preset.
    #[inline]
    fn default() -> Self {
        Self::standard()
    }
}

/// Errors from security configuration validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityConfigError {
    /// Paranoid preset requires Hardware Security Module for key storage.
    ParanoidRequiresHsm,

    /// Challenge timeout cannot exceed session timeout.
    ChallengeTimeoutTooLong,

    /// Audit log directory does not exist.
    AuditLogDirNotFound(PathBuf),
}

impl std::fmt::Display for SecurityConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurityConfigError::ParanoidRequiresHsm => {
                write!(
                    f,
                    "Paranoid security preset requires HSM key storage (KeyStorageMethod::HardwareSecurityModule)"
                )
            }
            SecurityConfigError::ChallengeTimeoutTooLong => {
                write!(
                    f,
                    "Challenge timeout cannot be longer than session timeout"
                )
            }
            SecurityConfigError::AuditLogDirNotFound(path) => {
                write!(f, "Audit log directory does not exist: {}", path.display())
            }
        }
    }
}

impl std::error::Error for SecurityConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_defaults() {
        // Minimal
        let minimal = SecurityConfig::minimal();
        assert_eq!(minimal.preset, SecurityPreset::Minimal);
        assert!(minimal.session_timeout.is_none());
        assert_eq!(minimal.audit_level, AuditLevel::Minimal);
        assert!(!minimal.require_reauth_high_risk);

        // Standard
        let standard = SecurityConfig::standard();
        assert_eq!(standard.preset, SecurityPreset::Standard);
        assert_eq!(
            standard.session_timeout,
            Some(Duration::from_secs(3600))
        );
        assert_eq!(standard.audit_level, AuditLevel::Standard);
        assert!(!standard.require_reauth_high_risk);

        // Paranoid
        let paranoid = SecurityConfig::paranoid();
        assert_eq!(paranoid.preset, SecurityPreset::Paranoid);
        assert_eq!(
            paranoid.session_timeout,
            Some(Duration::from_secs(300))
        );
        assert_eq!(paranoid.audit_level, AuditLevel::Verbose);
        assert!(paranoid.require_reauth_high_risk);
        assert!(paranoid.key_storage.is_hsm());
    }

    #[test]
    fn test_builder_pattern() {
        let config = SecurityConfig::standard()
            .with_session_timeout(Some(Duration::from_secs(1800)))
            .with_challenge_timeout(Duration::from_secs(15))
            .with_require_reauth_high_risk(true)
            .with_audit_level(AuditLevel::Verbose);

        assert_eq!(config.session_timeout, Some(Duration::from_secs(1800)));
        assert_eq!(config.challenge_timeout, Duration::from_secs(15));
        assert!(config.require_reauth_high_risk);
        assert_eq!(config.audit_level, AuditLevel::Verbose);
    }

    #[test]
    fn test_validation_paranoid_requires_hsm() {
        let mut config = SecurityConfig::paranoid();
        assert!(config.validate().is_ok());

        config.key_storage = KeyStorageMethod::EnvironmentVariable;
        assert_eq!(
            config.validate().unwrap_err(),
            SecurityConfigError::ParanoidRequiresHsm
        );
    }

    #[test]
    fn test_validation_challenge_timeout() {
        let config = SecurityConfig::standard()
            .with_session_timeout(Some(Duration::from_secs(30)))
            .with_challenge_timeout(Duration::from_secs(60)); // Too long!

        assert_eq!(
            config.validate().unwrap_err(),
            SecurityConfigError::ChallengeTimeoutTooLong
        );
    }

    #[test]
    fn test_key_storage_methods() {
        assert!(!KeyStorageMethod::EnvironmentVariable.requires_interaction());
        assert!(!KeyStorageMethod::PasswordManager {
            command: "test".to_string()
        }
        .requires_interaction());
        assert!(KeyStorageMethod::EncryptedFile {
            path: PathBuf::from("/tmp/test.key")
        }
        .requires_interaction());
        assert!(KeyStorageMethod::HardwareSecurityModule {
            pkcs11_path: "/usr/lib/softhsm/libsofthsm2.so".to_string()
        }
        .requires_interaction());
    }

    #[test]
    fn test_audit_level_helpers() {
        assert!(!AuditLevel::Minimal.logs_operations());
        assert!(AuditLevel::Standard.logs_operations());
        assert!(AuditLevel::Verbose.logs_operations());

        assert!(!AuditLevel::Minimal.logs_full_context());
        assert!(!AuditLevel::Standard.logs_full_context());
        assert!(AuditLevel::Verbose.logs_full_context());
    }

    #[test]
    fn test_default_implementations() {
        assert_eq!(SecurityPreset::default(), SecurityPreset::Standard);
        assert_eq!(KeyStorageMethod::default(), KeyStorageMethod::EnvironmentVariable);
        assert_eq!(AuditLevel::default(), AuditLevel::Standard);

        let default_config = SecurityConfig::default();
        assert_eq!(default_config.preset, SecurityPreset::Standard);
    }
}
