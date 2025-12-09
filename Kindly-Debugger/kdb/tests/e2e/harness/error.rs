//! E2E Error Types
//!
//! Unified error handling for E2E test harness using thiserror.
//!
//! # ASSUM Safety
//!
//! - #ASSUME_ERROR_CONTEXT: All errors provide useful context for debugging
//! - #ASSUME_DISPLAY_IMPL: All errors implement Display via thiserror

use std::io;
use thiserror::Error;

/// E2E test harness errors
#[derive(Error, Debug)]
pub enum E2EError {
    /// Process spawn failed
    #[error("Failed to spawn process '{target}': {source}")]
    SpawnFailed {
        target: String,
        #[source]
        source: io::Error,
    },

    /// Process not found in binaries directory
    #[error("Binary not found: {path}")]
    BinaryNotFound { path: String },

    /// Process exited unexpectedly
    #[error("Process exited unexpectedly with status: {status}")]
    ProcessExited { status: i32 },

    /// Process wait pattern timeout
    #[error("Timeout waiting for pattern '{pattern}' after {timeout_ms}ms")]
    PatternTimeout { pattern: String, timeout_ms: u64 },

    /// Debugger attach failed
    #[error("Failed to attach to process {pid}: {reason}")]
    AttachFailed { pid: u32, reason: String },

    /// Debugger detach failed
    #[error("Failed to detach from process {pid}: {reason}")]
    DetachFailed { pid: u32, reason: String },

    /// Breakpoint operation failed
    #[error("Breakpoint operation failed at '{location}': {reason}")]
    BreakpointFailed { location: String, reason: String },

    /// Step operation failed
    #[error("Step operation failed: {reason}")]
    StepFailed { reason: String },

    /// Snapshot operation failed
    #[error("Snapshot operation failed: {reason}")]
    SnapshotFailed { reason: String },

    /// Register read failed
    #[error("Failed to read registers: {reason}")]
    RegisterReadFailed { reason: String },

    /// Stack trace failed
    #[error("Failed to get stack trace: {reason}")]
    StackTraceFailed { reason: String },

    /// Memory read failed
    #[error("Failed to read memory at 0x{addr:x}: {reason}")]
    MemoryReadFailed { addr: u64, reason: String },

    /// Audit trail verification failed
    #[error("Audit trail verification failed: {reason}")]
    AuditVerificationFailed { reason: String },

    /// GDB communication error
    #[error("GDB communication error: {reason}")]
    GdbCommunicationError { reason: String },

    /// GDB MI parse error
    #[error("Failed to parse GDB/MI response: {response}")]
    GdbMiParseError { response: String },

    /// Validation mismatch
    #[error("Validation mismatch: expected {expected}, got {actual}")]
    ValidationMismatch { expected: String, actual: String },

    /// IO error wrapper
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Generic error with context
    #[error("{context}: {message}")]
    Generic { context: String, message: String },

    /// Process not running
    #[error("Process {pid} is not running")]
    ProcessNotRunning { pid: u32 },

    /// Invalid PID
    #[error("Invalid PID: {pid}")]
    InvalidPid { pid: u32 },

    /// Timeout waiting for debugger operation
    #[error("Debugger operation timed out after {timeout_ms}ms")]
    DebuggerTimeout { timeout_ms: u64 },

    /// Not attached to any process
    #[error("Not attached to any process")]
    NotAttached,
}

impl E2EError {
    /// Create a generic error with context
    pub fn generic(context: impl Into<String>, message: impl Into<String>) -> Self {
        E2EError::Generic {
            context: context.into(),
            message: message.into(),
        }
    }

    /// Create a spawn error
    pub fn spawn_failed(target: impl Into<String>, source: io::Error) -> Self {
        E2EError::SpawnFailed {
            target: target.into(),
            source,
        }
    }

    /// Create an attach error
    pub fn attach_failed(pid: u32, reason: impl Into<String>) -> Self {
        E2EError::AttachFailed {
            pid,
            reason: reason.into(),
        }
    }

    /// Create a breakpoint error
    pub fn breakpoint_failed(location: impl Into<String>, reason: impl Into<String>) -> Self {
        E2EError::BreakpointFailed {
            location: location.into(),
            reason: reason.into(),
        }
    }

    /// Create a validation mismatch error
    pub fn validation_mismatch(expected: impl Into<String>, actual: impl Into<String>) -> Self {
        E2EError::ValidationMismatch {
            expected: expected.into(),
            actual: actual.into(),
        }
    }
}

/// Result type for E2E operations
pub type E2EResult<T> = Result<T, E2EError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = E2EError::SpawnFailed {
            target: "test_binary".to_string(),
            source: io::Error::new(io::ErrorKind::NotFound, "not found"),
        };
        assert!(err.to_string().contains("test_binary"));
    }

    #[test]
    fn test_generic_error() {
        let err = E2EError::generic("test_context", "test_message");
        assert!(err.to_string().contains("test_context"));
        assert!(err.to_string().contains("test_message"));
    }

    #[test]
    fn test_validation_mismatch() {
        let err = E2EError::validation_mismatch("expected_value", "actual_value");
        let msg = err.to_string();
        assert!(msg.contains("expected_value"));
        assert!(msg.contains("actual_value"));
    }
}
