//! Error Types for Shell Alias Management

use crate::daemon::DaemonError;
use std::fmt;

/// Result type for alias operations
pub type AliasResult<T> = Result<T, AliasError>;

/// Error type for shell alias operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AliasError {
    /// Alias already exists
    AlreadyExists {
        /// Name of the existing alias
        name: String,
    },

    /// Alias not found
    NotFound {
        /// Name of the missing alias
        name: String,
    },

    /// Invalid alias name (contains invalid characters)
    InvalidName {
        /// The invalid alias name
        name: String,
        /// Reason why it's invalid
        reason: String,
    },

    /// Command does not exist in PATH
    CommandNotFound {
        /// The command that doesn't exist
        command: String,
    },

    /// Shell config file not found
    ConfigNotFound {
        /// Path to the missing config file
        path: String,
    },

    /// Permission denied accessing config file
    PermissionDenied {
        /// Path to the inaccessible file
        path: String,
    },

    /// I/O error reading or writing config
    IoError {
        /// Description of the I/O error
        message: String,
    },

    /// Daemon coordination error
    DaemonError {
        /// The underlying daemon error
        error: String,
    },

    /// Shell type not supported
    UnsupportedShell {
        /// The unsupported shell type
        shell: String,
    },

    /// Config file is corrupt or unparseable
    ParseError {
        /// Description of the parse error
        message: String,
    },
}

impl fmt::Display for AliasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AliasError::AlreadyExists { name } => {
                write!(f, "Alias '{}' already exists", name)
            }
            AliasError::NotFound { name } => {
                write!(f, "Alias '{}' not found", name)
            }
            AliasError::InvalidName { name, reason } => {
                write!(f, "Invalid alias name '{}': {}", name, reason)
            }
            AliasError::CommandNotFound { command } => {
                write!(f, "Command '{}' not found in PATH", command)
            }
            AliasError::ConfigNotFound { path } => {
                write!(f, "Shell config file not found: {}", path)
            }
            AliasError::PermissionDenied { path } => {
                write!(f, "Permission denied: {}", path)
            }
            AliasError::IoError { message } => {
                write!(f, "I/O error: {}", message)
            }
            AliasError::DaemonError { error } => {
                write!(f, "Daemon coordination error: {}", error)
            }
            AliasError::UnsupportedShell { shell } => {
                write!(f, "Unsupported shell: {}", shell)
            }
            AliasError::ParseError { message } => {
                write!(f, "Parse error: {}", message)
            }
        }
    }
}

impl std::error::Error for AliasError {}

impl From<std::io::Error> for AliasError {
    fn from(err: std::io::Error) -> Self {
        AliasError::IoError {
            message: err.to_string(),
        }
    }
}

impl From<DaemonError> for AliasError {
    fn from(err: DaemonError) -> Self {
        AliasError::DaemonError {
            error: err.to_string(),
        }
    }
}
