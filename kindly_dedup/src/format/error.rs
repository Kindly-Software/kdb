//! Format error types
//!
//! Comprehensive error handling for format readers.

use std::fmt;
use std::io;

/// Format-specific errors
#[derive(Debug)]
pub enum FormatError {
    /// I/O error (file not found, permission denied, etc.)
    Io(io::Error),

    /// JSON parse error
    JsonParse {
        /// Line number (1-indexed)
        line: usize,
        /// Error reason
        reason: String,
    },

    /// CSV parse error
    CsvParse {
        /// Line number (1-indexed)
        line: usize,
        /// Error reason
        reason: String,
    },

    /// Schema mapping error (missing column, type mismatch)
    SchemaMapping(String),

    /// Empty file error
    EmptyFile,

    /// Unknown format (file extension not recognized)
    UnknownFormat(String),

    /// Custom error message
    Custom(String),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::Io(e) => write!(f, "I/O error: {}", e),
            FormatError::JsonParse { line, reason } => {
                write!(f, "JSON parse error at line {}: {}", line, reason)
            }
            FormatError::CsvParse { line, reason } => {
                write!(f, "CSV parse error at line {}: {}", line, reason)
            }
            FormatError::SchemaMapping(msg) => write!(f, "Schema mapping error: {}", msg),
            FormatError::EmptyFile => write!(f, "Empty file"),
            FormatError::UnknownFormat(ext) => write!(f, "Unknown format: .{}", ext),
            FormatError::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for FormatError {}

impl From<io::Error> for FormatError {
    fn from(e: io::Error) -> Self {
        FormatError::Io(e)
    }
}

impl From<atomic_capsule::parallel::ParallelError> for FormatError {
    fn from(e: atomic_capsule::parallel::ParallelError) -> Self {
        FormatError::Custom(format!("Parallel processing error: {:?}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let fmt_err = FormatError::from(io_err);
        assert!(matches!(fmt_err, FormatError::Io(_)));
    }

    #[test]
    fn test_error_display() {
        let err = FormatError::JsonParse {
            line: 42,
            reason: "unexpected end of input".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("line 42"));
        assert!(msg.contains("unexpected end of input"));
    }

    #[test]
    fn test_unknown_format_error() {
        let err = FormatError::UnknownFormat("parquet".to_string());
        let msg = err.to_string();
        assert!(msg.contains("parquet"));
    }
}
