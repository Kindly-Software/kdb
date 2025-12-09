//! Terminal Operation Errors
//!
//! Error types for terminal operations following T0 Auditable tier patterns.
//! Based on research of Rust terminal library best practices and crossterm design.
//!
//! ## Design Principles
//!
//! - **Domain-Specific**: Terminal-specific error types (not generic I/O errors)
//! - **Copy-able**: All errors are Copy for zero-allocation error handling
//! - **Informative**: Rich context with errno codes for debugging
//! - **No Allocation**: String-free for no_std compatibility
//!
//! ## References
//!
//! - [Rust Error Handling Guide 2025](https://markaicode.com/rust-error-handling-2025-guide/)
//! - [Crossterm Design Patterns](https://github.com/crossterm-rs/crossterm)
//! - [Error Handling Best Practices](https://greptime.com/blogs/2024-05-07-error-rust)

use core::fmt;

#[cfg(feature = "std")]
use std::error::Error;

/// Terminal operation errors
///
/// # Design
///
/// - **T0 Auditable**: All errors are traceable for audit trails
/// - **Zero-Copy**: No heap allocations (Copy + Clone)
/// - **errno Context**: Preserve system error codes for debugging
///
/// # Examples
///
/// ```rust
/// use atomic_capsule::terminal::TerminalError;
///
/// let err = TerminalError::NotATty;
/// assert_eq!(format!("{}", err), "Not a TTY device");
///
/// let err = TerminalError::IoError(5); // EIO
/// assert!(format!("{}", err).contains("errno 5"));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalError {
    /// Not a TTY device
    ///
    /// # Use Case
    /// Detect when stdin/stdout is redirected to a file or pipe
    NotATty,

    /// Failed to get terminal attributes
    ///
    /// # Fields
    /// - `errno`: System error code (POSIX errno)
    ///
    /// # Common Causes
    /// - File descriptor is not a terminal
    /// - Permission denied (EACCES)
    /// - Invalid file descriptor (EBADF)
    GetAttrFailed(i32),

    /// Failed to set terminal attributes
    ///
    /// # Fields
    /// - `errno`: System error code (POSIX errno)
    ///
    /// # Common Causes
    /// - Terminal already in use by another process
    /// - Invalid terminal settings
    /// - Permission denied
    SetAttrFailed(i32),

    /// Raw mode already enabled
    ///
    /// # Use Case
    /// Prevent double-enabling raw mode which can corrupt terminal state
    AlreadyRawMode,

    /// Not in raw mode
    ///
    /// # Use Case
    /// Prevent disabling raw mode when it's not enabled
    NotRawMode,

    /// I/O error with errno
    ///
    /// # Fields
    /// - `errno`: System error code (POSIX errno)
    ///
    /// # Common Codes
    /// - EIO (5): I/O error
    /// - EINTR (4): Interrupted system call
    /// - EAGAIN (11): Resource temporarily unavailable
    IoError(i32),

    /// Event queue full
    ///
    /// # Use Case
    /// Back-pressure when terminal input queue is full
    QueueFull,

    /// Parse error
    ///
    /// # Use Case
    /// Invalid escape sequence or malformed terminal input
    ParseError,

    /// Timeout
    ///
    /// # Use Case
    /// Operation timed out waiting for terminal input
    Timeout,

    /// Unsupported operation
    ///
    /// # Use Case
    /// Platform doesn't support requested terminal operation
    Unsupported,

    /// Invalid state transition
    ///
    /// # Use Case
    /// Attempted state transition that violates the lifecycle state machine
    InvalidState,

    /// Already running
    ///
    /// # Use Case
    /// Terminal is already started/running
    AlreadyRunning,

    /// Not running
    ///
    /// # Use Case
    /// Terminal is not started/running (cannot perform operation)
    NotRunning,

    /// Initialization failed
    ///
    /// # Use Case
    /// Failed to initialize platform backend
    InitializationFailed,
}

impl fmt::Display for TerminalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TerminalError::NotATty => {
                write!(f, "Not a TTY device")
            }
            TerminalError::GetAttrFailed(errno) => {
                write!(f, "Failed to get terminal attributes (errno {})", errno)
            }
            TerminalError::SetAttrFailed(errno) => {
                write!(f, "Failed to set terminal attributes (errno {})", errno)
            }
            TerminalError::AlreadyRawMode => {
                write!(f, "Raw mode already enabled")
            }
            TerminalError::NotRawMode => {
                write!(f, "Not in raw mode")
            }
            TerminalError::IoError(errno) => {
                write!(f, "I/O error (errno {})", errno)
            }
            TerminalError::QueueFull => {
                write!(f, "Event queue full")
            }
            TerminalError::ParseError => {
                write!(f, "Parse error")
            }
            TerminalError::Timeout => {
                write!(f, "Operation timed out")
            }
            TerminalError::Unsupported => {
                write!(f, "Unsupported operation")
            }
            TerminalError::InvalidState => {
                write!(f, "Invalid state transition")
            }
            TerminalError::AlreadyRunning => {
                write!(f, "Terminal already running")
            }
            TerminalError::NotRunning => {
                write!(f, "Terminal not running")
            }
            TerminalError::InitializationFailed => {
                write!(f, "Terminal initialization failed")
            }
        }
    }
}

#[cfg(feature = "std")]
impl Error for TerminalError {}

// ============================================================================
// FROM CONVERSIONS (for convenience functions in mod.rs)
// ============================================================================

impl From<super::mode::RawModeError> for TerminalError {
    fn from(err: super::mode::RawModeError) -> Self {
        use super::mode::RawModeError;
        match err {
            RawModeError::NotATty => TerminalError::NotATty,
            RawModeError::GetAttrFailed(errno) => TerminalError::GetAttrFailed(errno),
            RawModeError::SetAttrFailed(errno) => TerminalError::SetAttrFailed(errno),
            RawModeError::AlreadyInMode => TerminalError::AlreadyRawMode,
            RawModeError::InvalidStateTransition { from: _, to: _ } => TerminalError::NotRawMode,
            RawModeError::OriginalTermiosNotSaved => TerminalError::IoError(0),
        }
    }
}

impl From<super::mode::ScreenError> for TerminalError {
    fn from(err: super::mode::ScreenError) -> Self {
        use super::mode::ScreenError;
        match err {
            ScreenError::WriteFailed(errno) => TerminalError::IoError(errno),
            ScreenError::AlreadyInScreen => TerminalError::AlreadyRawMode,
            ScreenError::InvalidStateTransition { from: _, to: _ } => TerminalError::NotRawMode,
            ScreenError::NotATty => TerminalError::NotATty,
            #[cfg(feature = "std")]
            ScreenError::IoError(_msg) => TerminalError::IoError(0),
        }
    }
}

impl From<super::mode::CursorError> for TerminalError {
    fn from(err: super::mode::CursorError) -> Self {
        use super::mode::CursorError;
        match err {
            CursorError::WriteFailed(errno) => TerminalError::IoError(errno),
            CursorError::NotATty => TerminalError::NotATty,
            CursorError::InvalidPosition { x: _, y: _ } => TerminalError::IoError(22), // EINVAL
            #[cfg(feature = "std")]
            CursorError::IoError(_msg) => TerminalError::IoError(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_a_tty_display() {
        let err = TerminalError::NotATty;
        let display = format!("{}", err);
        assert!(display.contains("Not a TTY"));
    }

    #[test]
    fn test_get_attr_failed_display() {
        let err = TerminalError::GetAttrFailed(5); // EIO
        let display = format!("{}", err);
        assert!(display.contains("get terminal attributes"));
        assert!(display.contains("errno 5"));
    }

    #[test]
    fn test_set_attr_failed_display() {
        let err = TerminalError::SetAttrFailed(13); // EACCES
        let display = format!("{}", err);
        assert!(display.contains("set terminal attributes"));
        assert!(display.contains("errno 13"));
    }

    #[test]
    fn test_io_error_display() {
        let err = TerminalError::IoError(4); // EINTR
        let display = format!("{}", err);
        assert!(display.contains("I/O error"));
        assert!(display.contains("errno 4"));
    }

    #[test]
    fn test_queue_full_display() {
        let err = TerminalError::QueueFull;
        let display = format!("{}", err);
        assert!(display.contains("queue full"));
    }

    #[test]
    fn test_parse_error_display() {
        let err = TerminalError::ParseError;
        let display = format!("{}", err);
        assert!(display.contains("Parse error"));
    }

    #[test]
    fn test_timeout_display() {
        let err = TerminalError::Timeout;
        let display = format!("{}", err);
        assert!(display.contains("timed out"));
    }

    #[test]
    fn test_unsupported_display() {
        let err = TerminalError::Unsupported;
        let display = format!("{}", err);
        assert!(display.contains("Unsupported"));
    }

    #[test]
    fn test_error_equality() {
        let err1 = TerminalError::GetAttrFailed(5);
        let err2 = TerminalError::GetAttrFailed(5);
        assert_eq!(err1, err2);

        let err3 = TerminalError::GetAttrFailed(6);
        assert_ne!(err1, err3);
    }

    #[test]
    fn test_error_copy() {
        let err1 = TerminalError::NotATty;
        let err2 = err1; // Copy, not move
        assert_eq!(err1, err2);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_error_trait() {
        let err = TerminalError::IoError(5);
        let _err_trait: &dyn Error = &err;
    }
}
