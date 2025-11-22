//! # DaemonLockCapsule Error Types
//!
//! **UCE34 Tier 1 Atomic Capsule error handling for daemon coordination.**
//!
//! ## Error Categories
//! - **LockHeld**: Another process holds the lock
//! - **LockTimeout**: Acquisition timed out waiting for lock
//! - **QueueFull**: Result queue capacity exceeded

use std::fmt;

/// Error types for daemon locking operations
///
/// # ASSUM Framework
/// - `#ASSUME_ERROR_SEMANTICS`: Each error variant uniquely identifies the failure mode
/// - `#VERIFY_ERROR_SEMANTICS`: Tests validate error conditions (unit tests)
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DaemonError {
    /// Lock is held by another process
    ///
    /// Contains the PID of the process holding the lock
    LockHeld { holder_pid: u32 },

    /// Lock acquisition timed out
    ///
    /// Contains the number of nanoseconds waited
    LockTimeout { waited_ns: u64 },

    /// Result queue reached capacity
    ///
    /// Contains the queue capacity in elements
    QueueFull { capacity: usize },

    /// Process ID is invalid (must be > 0)
    InvalidPid,

    /// Invalid internal state
    InvalidState,
}

impl fmt::Display for DaemonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DaemonError::LockHeld { holder_pid } => {
                write!(f, "Lock held by process {}", holder_pid)
            }
            DaemonError::LockTimeout { waited_ns } => {
                write!(f, "Lock acquisition timed out after {}ns", waited_ns)
            }
            DaemonError::QueueFull { capacity } => {
                write!(f, "Result queue full (capacity: {})", capacity)
            }
            DaemonError::InvalidPid => {
                write!(f, "Invalid process ID (must be > 0)")
            }
            DaemonError::InvalidState => {
                write!(f, "Invalid internal state (indicates bug)")
            }
        }
    }
}

impl std::error::Error for DaemonError {}

/// Result type for daemon operations
///
/// Convenience alias for `Result<T, DaemonError>`
pub type DaemonResult<T> = Result<T, DaemonError>;
