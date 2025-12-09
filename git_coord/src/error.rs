//! Error types for git coordination.
//!
//! Comprehensive error handling following UCE34 Tier Reference error handling patterns.
//! All errors are structured, recoverable, and provide rich context.

use std::path::PathBuf;
use std::io;

/// Lock-specific errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LockError {
    /// Lock is currently held by another instance
    #[error("Lock held by instance {0}, generation {1}")]
    Held(u32, u32),

    /// Lock acquisition timeout
    #[error("Lock timeout after {0} attempts")]
    Timeout(u32),

    /// Stale lock detected (heartbeat expired)
    #[error("Stale lock from instance {0}, heartbeat expired")]
    Stale(u32),

    /// Lock state corrupted
    #[error("Lock state corrupted: {0}")]
    Corrupted(&'static str),
}

/// Queue-specific errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum QueueError {
    /// Queue is full
    #[error("Queue full (capacity: {0})")]
    Full(usize),

    /// Queue is empty
    #[error("Queue empty")]
    Empty,

    /// Queue state corrupted
    #[error("Queue state corrupted: {0}")]
    Corrupted(&'static str),
}

/// Main coordinator error type
#[derive(Debug, thiserror::Error)]
pub enum CoordinatorError {
    /// Path is not a git repository
    #[error("Not a git repository: {0}")]
    NotGitRepo(PathBuf),

    /// Lock error
    #[error("Lock error: {0}")]
    Lock(#[from] LockError),

    /// Queue error
    #[error("Queue error: {0}")]
    Queue(#[from] QueueError),

    /// Lock acquisition timeout
    #[error("Lock timeout after retries")]
    Timeout,

    /// Coordinator state corrupted
    #[error("Coordinator state corrupted: {0}")]
    CorruptedState(String),

    /// Git operation failed
    #[error("Git operation failed: {0}")]
    GitError(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Audit trail error
    #[error("Audit error: {0}")]
    Audit(String),
}

/// Result type for coordinator operations
pub type Result<T> = std::result::Result<T, CoordinatorError>;
