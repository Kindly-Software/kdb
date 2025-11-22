//! Error types for logging module
//!
//! # UCE34 Tier: T0 Auditable (error classification)

/// Log error types
///
/// # Variants
///
/// - `RingFull`: Ring buffer reached capacity and entry was dropped
/// - `ParseError`: Failed to parse RUST_LOG environment variable
/// - `InvalidLevel`: Invalid log level string
///
/// # Examples
///
/// ```ignore
/// use atomic_capsule::logging::{LogError, LogResult};
///
/// match some_log_operation() {
///     Ok(_) => println!("Success"),
///     Err(LogError::RingFull { capacity }) => {
///         eprintln!("Ring buffer full at capacity {}", capacity);
///     },
///     Err(LogError::InvalidLevel { level }) => {
///         eprintln!("Invalid log level: {}", level);
///     },
///     Err(LogError::ParseError { reason }) => {
///         eprintln!("Parse error: {}", reason);
///     },
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogError {
    /// Ring buffer full (capacity exceeded)
    ///
    /// Entry was dropped due to ring buffer overflow.
    /// This is graceful degradation - production continues without blocking.
    RingFull { capacity: usize },

    /// Failed to parse RUST_LOG environment variable
    ///
    /// The RUST_LOG string contained invalid syntax or values.
    ParseError { reason: String },

    /// Invalid log level string
    ///
    /// Log level must be one of: "off", "error", "warn", "info", "debug", "trace"
    InvalidLevel { level: String },
}

impl std::fmt::Display for LogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RingFull { capacity } => {
                write!(f, "Ring buffer full (capacity: {})", capacity)
            }
            Self::ParseError { reason } => {
                write!(f, "Failed to parse RUST_LOG: {}", reason)
            }
            Self::InvalidLevel { level } => {
                write!(f, "Invalid log level: {}", level)
            }
        }
    }
}

impl std::error::Error for LogError {}

/// Result type for logging operations
///
/// # Examples
///
/// ```ignore
/// use atomic_capsule::logging::{LogResult, LogEntry};
///
/// fn record_log_safely(entry: LogEntry) -> LogResult<()> {
///     // ... do logging ...
///     Ok(())
/// }
/// ```
pub type Result<T> = std::result::Result<T, LogError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_error_ring_full() {
        let err = LogError::RingFull { capacity: 16384 };
        assert_eq!(err.to_string(), "Ring buffer full (capacity: 16384)");
    }

    #[test]
    fn test_log_error_invalid_level() {
        let err = LogError::InvalidLevel {
            level: "invalid".to_string(),
        };
        assert_eq!(err.to_string(), "Invalid log level: invalid");
    }

    #[test]
    fn test_log_error_parse_error() {
        let err = LogError::ParseError {
            reason: "missing equals".to_string(),
        };
        assert_eq!(err.to_string(), "Failed to parse RUST_LOG: missing equals");
    }

    #[test]
    fn test_log_error_debug() {
        let err = LogError::RingFull { capacity: 100 };
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("RingFull"));
        assert!(debug_str.contains("100"));
    }

    #[test]
    fn test_log_error_equality() {
        let err1 = LogError::RingFull { capacity: 100 };
        let err2 = LogError::RingFull { capacity: 100 };
        let err3 = LogError::RingFull { capacity: 200 };

        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }
}
