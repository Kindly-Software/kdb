//! Package Manager Error Types
//!
//! **Tier**: T0 (Foundation)
//! **Chaos Compliance**: 100% safe, no unsafe code
//!
//! Comprehensive error types for package management operations following
//! the thiserror pattern for rich error context.

use core::fmt;

#[cfg(feature = "std")]
use std::path::PathBuf;

/// Package manager result type alias
pub type PkgResult<T> = Result<T, PkgError>;

/// Package manager error enumeration
///
/// All errors include rich context for debugging and audit trails.
/// Each variant maps to a specific failure mode in the package lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PkgError {
    // ========================================================================
    // Database Errors (T9 Persistent)
    // ========================================================================

    /// Package not found in database
    PackageNotFound {
        /// Package name that was not found
        name: String,
    },

    /// Package version not found
    VersionNotFound {
        /// Package name
        name: String,
        /// Requested version constraint
        constraint: String,
    },

    /// Database corruption detected
    DatabaseCorruption {
        /// Description of corruption
        description: String,
        /// Offset where corruption was detected
        offset: u64,
    },

    /// Database is locked by another process
    DatabaseLocked {
        /// PID holding the lock (if known)
        holder_pid: Option<u32>,
    },

    /// Database version mismatch
    DatabaseVersionMismatch {
        /// Expected version
        expected: u32,
        /// Actual version found
        actual: u32,
    },

    /// Insufficient space for database operation
    DatabaseFull {
        /// Required bytes
        required: u64,
        /// Available bytes
        available: u64,
    },

    // ========================================================================
    // Dependency Resolution Errors (T4 Batch)
    // ========================================================================

    /// Dependency conflict detected
    DependencyConflict {
        /// Package that has the conflict
        package: String,
        /// Conflicting package
        conflicts_with: String,
        /// Reason for conflict
        reason: String,
    },

    /// Unsatisfiable dependency
    UnsatisfiableDependency {
        /// Package requiring the dependency
        package: String,
        /// Dependency that cannot be satisfied
        dependency: String,
        /// Version constraint
        constraint: String,
    },

    /// Circular dependency detected
    CircularDependency {
        /// Packages involved in the cycle
        cycle: Vec<String>,
    },

    /// Resolution timeout (SAT solver limit)
    ResolutionTimeout {
        /// Number of packages being resolved
        package_count: usize,
        /// Time spent in milliseconds
        elapsed_ms: u64,
    },

    /// Too many resolution candidates
    ResolutionOverflow {
        /// Number of candidates
        candidates: usize,
        /// Maximum allowed
        limit: usize,
    },

    // ========================================================================
    // Repository Errors (T1+T9)
    // ========================================================================

    /// Repository not found
    RepositoryNotFound {
        /// Repository URL or identifier
        repository: String,
    },

    /// Repository metadata parse error
    RepositoryParseError {
        /// Repository identifier
        repository: String,
        /// Parse error description
        error: String,
    },

    /// Repository signature verification failed
    RepositorySignatureInvalid {
        /// Repository identifier
        repository: String,
        /// Expected key fingerprint
        expected_key: String,
    },

    /// Repository refresh failed
    RepositoryRefreshFailed {
        /// Repository identifier
        repository: String,
        /// Failure reason
        reason: String,
    },

    // ========================================================================
    // Verification Errors (T0+T1)
    // ========================================================================

    /// Package checksum mismatch
    ChecksumMismatch {
        /// Package name
        package: String,
        /// Expected checksum (hex)
        expected: String,
        /// Actual checksum (hex)
        actual: String,
    },

    /// Package signature verification failed
    SignatureInvalid {
        /// Package name
        package: String,
        /// Key fingerprint used
        key_fingerprint: String,
    },

    /// Package archive corrupted
    ArchiveCorrupted {
        /// Package name
        package: String,
        /// Corruption description
        description: String,
    },

    // ========================================================================
    // Transaction Errors (T1)
    // ========================================================================

    /// Transaction conflict
    TransactionConflict {
        /// Transaction ID that conflicted
        transaction_id: u64,
        /// Resource that conflicted
        resource: String,
    },

    /// Transaction rollback required
    TransactionRollback {
        /// Transaction ID
        transaction_id: u64,
        /// Reason for rollback
        reason: String,
    },

    /// Transaction already committed
    TransactionCommitted {
        /// Transaction ID
        transaction_id: u64,
    },

    /// Transaction timed out
    TransactionTimeout {
        /// Transaction ID
        transaction_id: u64,
        /// Timeout in milliseconds
        timeout_ms: u64,
    },

    // ========================================================================
    // Installation Errors
    // ========================================================================

    /// Pre-installation script failed
    PreInstallFailed {
        /// Package name
        package: String,
        /// Exit code
        exit_code: i32,
    },

    /// Post-installation script failed
    PostInstallFailed {
        /// Package name
        package: String,
        /// Exit code
        exit_code: i32,
    },

    /// File extraction failed
    ExtractionFailed {
        /// Package name
        package: String,
        /// File path that failed
        #[cfg(feature = "std")]
        path: PathBuf,
        #[cfg(not(feature = "std"))]
        path: String,
        /// Error description
        error: String,
    },

    /// Configuration file conflict
    ConfigConflict {
        /// Package name
        package: String,
        /// Conflicting config file path
        #[cfg(feature = "std")]
        path: PathBuf,
        #[cfg(not(feature = "std"))]
        path: String,
    },

    /// Insufficient disk space
    InsufficientSpace {
        /// Required bytes
        required: u64,
        /// Available bytes
        available: u64,
        /// Mount point
        #[cfg(feature = "std")]
        mount_point: PathBuf,
        #[cfg(not(feature = "std"))]
        mount_point: String,
    },

    /// Permission denied
    PermissionDenied {
        /// Operation that was denied
        operation: String,
        /// Path or resource
        #[cfg(feature = "std")]
        path: PathBuf,
        #[cfg(not(feature = "std"))]
        path: String,
    },

    // ========================================================================
    // Download Errors (T4+T8)
    // ========================================================================

    /// Download failed
    DownloadFailed {
        /// URL that failed
        url: String,
        /// HTTP status code (if applicable)
        status_code: Option<u16>,
        /// Error description
        error: String,
    },

    /// Download timeout
    DownloadTimeout {
        /// URL that timed out
        url: String,
        /// Timeout in seconds
        timeout_seconds: u32,
    },

    /// Network unreachable
    NetworkUnreachable {
        /// Host that was unreachable
        host: String,
    },

    // ========================================================================
    // Version Errors
    // ========================================================================

    /// Invalid version string
    InvalidVersion {
        /// The invalid version string
        version: String,
        /// Parsing error description
        error: String,
    },

    /// Invalid version constraint
    InvalidConstraint {
        /// The invalid constraint string
        constraint: String,
        /// Parsing error description
        error: String,
    },

    /// Version downgrade not allowed
    DowngradeNotAllowed {
        /// Package name
        package: String,
        /// Installed version
        installed: String,
        /// Requested version
        requested: String,
    },

    // ========================================================================
    // State Errors
    // ========================================================================

    /// Invalid package state transition
    InvalidStateTransition {
        /// Package name
        package: String,
        /// Current state
        from_state: String,
        /// Attempted state
        to_state: String,
    },

    /// Package in broken state
    BrokenPackage {
        /// Package name
        package: String,
        /// Current state
        state: String,
        /// Description of issue
        description: String,
    },

    // ========================================================================
    // Generic Errors
    // ========================================================================

    /// I/O error
    IoError {
        /// Operation that failed
        operation: String,
        /// Error description
        error: String,
    },

    /// Internal error (bug)
    InternalError {
        /// Error description
        description: String,
    },

    /// Operation cancelled by user
    Cancelled,

    /// Feature not implemented
    NotImplemented {
        /// Feature name
        feature: String,
    },
}

impl fmt::Display for PkgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Database errors
            PkgError::PackageNotFound { name } => {
                write!(f, "package '{}' not found", name)
            }
            PkgError::VersionNotFound { name, constraint } => {
                write!(f, "no version of '{}' satisfies constraint '{}'", name, constraint)
            }
            PkgError::DatabaseCorruption { description, offset } => {
                write!(f, "database corruption at offset {}: {}", offset, description)
            }
            PkgError::DatabaseLocked { holder_pid } => {
                match holder_pid {
                    Some(pid) => write!(f, "database locked by process {}", pid),
                    None => write!(f, "database locked by another process"),
                }
            }
            PkgError::DatabaseVersionMismatch { expected, actual } => {
                write!(f, "database version mismatch: expected {}, found {}", expected, actual)
            }
            PkgError::DatabaseFull { required, available } => {
                write!(f, "database full: need {} bytes, only {} available", required, available)
            }

            // Dependency errors
            PkgError::DependencyConflict { package, conflicts_with, reason } => {
                write!(f, "dependency conflict: '{}' conflicts with '{}': {}", package, conflicts_with, reason)
            }
            PkgError::UnsatisfiableDependency { package, dependency, constraint } => {
                write!(f, "unsatisfiable dependency: '{}' requires '{}' {}", package, dependency, constraint)
            }
            PkgError::CircularDependency { cycle } => {
                write!(f, "circular dependency: {}", cycle.join(" -> "))
            }
            PkgError::ResolutionTimeout { package_count, elapsed_ms } => {
                write!(f, "dependency resolution timeout: {} packages, {}ms elapsed", package_count, elapsed_ms)
            }
            PkgError::ResolutionOverflow { candidates, limit } => {
                write!(f, "too many resolution candidates: {} (limit: {})", candidates, limit)
            }

            // Repository errors
            PkgError::RepositoryNotFound { repository } => {
                write!(f, "repository '{}' not found", repository)
            }
            PkgError::RepositoryParseError { repository, error } => {
                write!(f, "failed to parse repository '{}': {}", repository, error)
            }
            PkgError::RepositorySignatureInvalid { repository, expected_key } => {
                write!(f, "invalid signature for repository '{}' (expected key: {})", repository, expected_key)
            }
            PkgError::RepositoryRefreshFailed { repository, reason } => {
                write!(f, "failed to refresh repository '{}': {}", repository, reason)
            }

            // Verification errors
            PkgError::ChecksumMismatch { package, expected, actual } => {
                write!(f, "checksum mismatch for '{}': expected {}, got {}", package, expected, actual)
            }
            PkgError::SignatureInvalid { package, key_fingerprint } => {
                write!(f, "invalid signature for '{}' (key: {})", package, key_fingerprint)
            }
            PkgError::ArchiveCorrupted { package, description } => {
                write!(f, "corrupted archive for '{}': {}", package, description)
            }

            // Transaction errors
            PkgError::TransactionConflict { transaction_id, resource } => {
                write!(f, "transaction {} conflicts on resource '{}'", transaction_id, resource)
            }
            PkgError::TransactionRollback { transaction_id, reason } => {
                write!(f, "transaction {} rolled back: {}", transaction_id, reason)
            }
            PkgError::TransactionCommitted { transaction_id } => {
                write!(f, "transaction {} already committed", transaction_id)
            }
            PkgError::TransactionTimeout { transaction_id, timeout_ms } => {
                write!(f, "transaction {} timed out after {}ms", transaction_id, timeout_ms)
            }

            // Installation errors
            PkgError::PreInstallFailed { package, exit_code } => {
                write!(f, "pre-install script failed for '{}': exit code {}", package, exit_code)
            }
            PkgError::PostInstallFailed { package, exit_code } => {
                write!(f, "post-install script failed for '{}': exit code {}", package, exit_code)
            }
            PkgError::ExtractionFailed { package, path, error } => {
                write!(f, "extraction failed for '{}' at {:?}: {}", package, path, error)
            }
            PkgError::ConfigConflict { package, path } => {
                write!(f, "config conflict for '{}' at {:?}", package, path)
            }
            PkgError::InsufficientSpace { required, available, mount_point } => {
                write!(f, "insufficient space at {:?}: need {} bytes, have {}", mount_point, required, available)
            }
            PkgError::PermissionDenied { operation, path } => {
                write!(f, "permission denied: {} on {:?}", operation, path)
            }

            // Download errors
            PkgError::DownloadFailed { url, status_code, error } => {
                match status_code {
                    Some(code) => write!(f, "download failed for '{}' (HTTP {}): {}", url, code, error),
                    None => write!(f, "download failed for '{}': {}", url, error),
                }
            }
            PkgError::DownloadTimeout { url, timeout_seconds } => {
                write!(f, "download timeout for '{}' after {}s", url, timeout_seconds)
            }
            PkgError::NetworkUnreachable { host } => {
                write!(f, "network unreachable: cannot connect to '{}'", host)
            }

            // Version errors
            PkgError::InvalidVersion { version, error } => {
                write!(f, "invalid version '{}': {}", version, error)
            }
            PkgError::InvalidConstraint { constraint, error } => {
                write!(f, "invalid version constraint '{}': {}", constraint, error)
            }
            PkgError::DowngradeNotAllowed { package, installed, requested } => {
                write!(f, "downgrade not allowed for '{}': {} -> {}", package, installed, requested)
            }

            // State errors
            PkgError::InvalidStateTransition { package, from_state, to_state } => {
                write!(f, "invalid state transition for '{}': {} -> {}", package, from_state, to_state)
            }
            PkgError::BrokenPackage { package, state, description } => {
                write!(f, "broken package '{}' (state: {}): {}", package, state, description)
            }

            // Generic errors
            PkgError::IoError { operation, error } => {
                write!(f, "I/O error during {}: {}", operation, error)
            }
            PkgError::InternalError { description } => {
                write!(f, "internal error: {}", description)
            }
            PkgError::Cancelled => write!(f, "operation cancelled"),
            PkgError::NotImplemented { feature } => {
                write!(f, "feature not implemented: {}", feature)
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PkgError {}

// ============================================================================
// Error Category Classification (for metrics/audit)
// ============================================================================

/// Error category for classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Database-related errors
    Database,
    /// Dependency resolution errors
    Dependency,
    /// Repository errors
    Repository,
    /// Verification/integrity errors
    Verification,
    /// Transaction errors
    Transaction,
    /// Installation errors
    Installation,
    /// Download/network errors
    Network,
    /// Version parsing errors
    Version,
    /// State machine errors
    State,
    /// Generic/other errors
    Other,
}

impl PkgError {
    /// Get the error category for metrics
    pub const fn category(&self) -> ErrorCategory {
        match self {
            PkgError::PackageNotFound { .. }
            | PkgError::VersionNotFound { .. }
            | PkgError::DatabaseCorruption { .. }
            | PkgError::DatabaseLocked { .. }
            | PkgError::DatabaseVersionMismatch { .. }
            | PkgError::DatabaseFull { .. } => ErrorCategory::Database,

            PkgError::DependencyConflict { .. }
            | PkgError::UnsatisfiableDependency { .. }
            | PkgError::CircularDependency { .. }
            | PkgError::ResolutionTimeout { .. }
            | PkgError::ResolutionOverflow { .. } => ErrorCategory::Dependency,

            PkgError::RepositoryNotFound { .. }
            | PkgError::RepositoryParseError { .. }
            | PkgError::RepositorySignatureInvalid { .. }
            | PkgError::RepositoryRefreshFailed { .. } => ErrorCategory::Repository,

            PkgError::ChecksumMismatch { .. }
            | PkgError::SignatureInvalid { .. }
            | PkgError::ArchiveCorrupted { .. } => ErrorCategory::Verification,

            PkgError::TransactionConflict { .. }
            | PkgError::TransactionRollback { .. }
            | PkgError::TransactionCommitted { .. }
            | PkgError::TransactionTimeout { .. } => ErrorCategory::Transaction,

            PkgError::PreInstallFailed { .. }
            | PkgError::PostInstallFailed { .. }
            | PkgError::ExtractionFailed { .. }
            | PkgError::ConfigConflict { .. }
            | PkgError::InsufficientSpace { .. }
            | PkgError::PermissionDenied { .. } => ErrorCategory::Installation,

            PkgError::DownloadFailed { .. }
            | PkgError::DownloadTimeout { .. }
            | PkgError::NetworkUnreachable { .. } => ErrorCategory::Network,

            PkgError::InvalidVersion { .. }
            | PkgError::InvalidConstraint { .. }
            | PkgError::DowngradeNotAllowed { .. } => ErrorCategory::Version,

            PkgError::InvalidStateTransition { .. }
            | PkgError::BrokenPackage { .. } => ErrorCategory::State,

            PkgError::IoError { .. }
            | PkgError::InternalError { .. }
            | PkgError::Cancelled
            | PkgError::NotImplemented { .. } => ErrorCategory::Other,
        }
    }

    /// Check if error is recoverable (can retry)
    pub const fn is_recoverable(&self) -> bool {
        matches!(
            self,
            PkgError::DatabaseLocked { .. }
                | PkgError::ResolutionTimeout { .. }
                | PkgError::RepositoryRefreshFailed { .. }
                | PkgError::TransactionConflict { .. }
                | PkgError::TransactionTimeout { .. }
                | PkgError::DownloadFailed { .. }
                | PkgError::DownloadTimeout { .. }
                | PkgError::NetworkUnreachable { .. }
        )
    }

    /// Check if error requires manual intervention
    pub const fn requires_intervention(&self) -> bool {
        matches!(
            self,
            PkgError::DatabaseCorruption { .. }
                | PkgError::DependencyConflict { .. }
                | PkgError::CircularDependency { .. }
                | PkgError::SignatureInvalid { .. }
                | PkgError::ArchiveCorrupted { .. }
                | PkgError::ConfigConflict { .. }
                | PkgError::PermissionDenied { .. }
                | PkgError::DowngradeNotAllowed { .. }
                | PkgError::BrokenPackage { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = PkgError::PackageNotFound {
            name: "nginx".to_string(),
        };
        assert_eq!(format!("{}", err), "package 'nginx' not found");
    }

    #[test]
    fn test_error_category() {
        let err = PkgError::DependencyConflict {
            package: "a".to_string(),
            conflicts_with: "b".to_string(),
            reason: "version mismatch".to_string(),
        };
        assert_eq!(err.category(), ErrorCategory::Dependency);
    }

    #[test]
    fn test_recoverable_errors() {
        let recoverable = PkgError::DatabaseLocked { holder_pid: Some(1234) };
        assert!(recoverable.is_recoverable());

        let not_recoverable = PkgError::DatabaseCorruption {
            description: "header invalid".to_string(),
            offset: 0,
        };
        assert!(!not_recoverable.is_recoverable());
    }

    #[test]
    fn test_intervention_required() {
        let needs_intervention = PkgError::SignatureInvalid {
            package: "malware".to_string(),
            key_fingerprint: "ABC123".to_string(),
        };
        assert!(needs_intervention.requires_intervention());

        let auto_recover = PkgError::DownloadTimeout {
            url: "https://example.com".to_string(),
            timeout_seconds: 30,
        };
        assert!(!auto_recover.requires_intervention());
    }
}
