//! PDF Export Error Types

use thiserror::Error;

/// PDF export error types
#[derive(Debug, Error)]
pub enum PdfError {
    /// Audit log error
    #[error("Audit log error: {0}")]
    AuditError(String),

    /// PDF generation error
    #[error("PDF generation error: {0}")]
    GenerationError(String),

    /// I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Invalid state
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Result type for PDF export operations
pub type Result<T> = std::result::Result<T, PdfError>;
