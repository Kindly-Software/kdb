//! # Futex Error Types
//!
//! **UCE34 T0 Auditable: Error types for futex operations**
//!
//! ## Error Mapping to Linux errno
//!
//! | FutexErrorKind    | Linux errno | Value | Description                        |
//! |-------------------|-------------|-------|-------------------------------------|
//! | WouldBlock        | EAGAIN      | -11   | Value mismatch, retry              |
//! | TimedOut          | ETIMEDOUT   | -110  | Timeout expired                    |
//! | Interrupted       | EINTR       | -4    | Interrupted by signal              |
//! | InvalidAddress    | EFAULT      | -14   | Invalid futex address              |
//! | InvalidOperation  | EINVAL      | -22   | Invalid operation or arguments     |
//! | NoMemory          | ENOMEM      | -12   | Out of memory for waiter allocation|
//! | Deadlock          | EDEADLK     | -35   | Deadlock detected (PI futex)       |
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_ERROR_CODES`: errno values match Linux kernel
//! - `#VERIFY_ERROR_CODES`: Validated against glibc expectations

use core::fmt;

/// Futex error kind - maps to Linux errno values
///
/// # ASSUM Framework
/// - `#ASSUME_ERRNO_STABLE`: Linux errno values are stable ABI
/// - `#VERIFY_ERRNO_STABLE`: Part of POSIX standard since 1988
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum FutexErrorKind {
    /// EAGAIN (-11): Futex value changed, operation would block
    ///
    /// Returned when FUTEX_WAIT detects the futex word doesn't match
    /// the expected value. Caller should retry the entire sequence.
    WouldBlock = -11,

    /// ETIMEDOUT (-110): Operation timed out
    ///
    /// Returned when the specified timeout expired before wake.
    /// Caller can retry or handle timeout condition.
    TimedOut = -110,

    /// EINTR (-4): Operation interrupted by signal
    ///
    /// Returned when a signal was delivered while waiting.
    /// Caller should check for pending signals and retry.
    ///
    /// # ASSUM_SIGNAL_SAFE
    /// - Signal handling must be coordinated with scheduler
    /// - Interruption leaves futex state consistent
    Interrupted = -4,

    /// EFAULT (-14): Invalid memory address
    ///
    /// Returned when the futex address is:
    /// - Not mapped in process address space
    /// - Not aligned to 4 bytes (32-bit futex)
    /// - In kernel memory (invalid for userspace futex)
    InvalidAddress = -14,

    /// EINVAL (-22): Invalid argument
    ///
    /// Returned when:
    /// - Operation code is unknown
    /// - Flags contain invalid combination
    /// - Wake count is negative
    /// - Bitset is zero (for FUTEX_*_BITSET)
    InvalidOperation = -22,

    /// ENOMEM (-12): Out of memory
    ///
    /// Returned when:
    /// - Cannot allocate waiter entry
    /// - Hash table bucket overflow
    /// - Queue capacity exceeded
    NoMemory = -12,

    /// EDEADLK (-35): Deadlock detected
    ///
    /// Returned for PI (priority inheritance) futexes when:
    /// - Circular wait detected
    /// - Owner would block on itself
    Deadlock = -35,

    /// ENOSYS (-38): Function not implemented
    ///
    /// Returned when:
    /// - PI futex operations not available
    /// - futex2 operations on older kernel emulation
    NotImplemented = -38,

    /// ESRCH (-3): No such process
    ///
    /// Returned for PI futexes when:
    /// - Owner thread no longer exists
    /// - Task lookup failed
    NoSuchProcess = -3,

    /// EPERM (-1): Operation not permitted
    ///
    /// Returned when:
    /// - Non-owner tries to unlock PI futex
    /// - Permission check fails
    PermissionDenied = -1,
}

impl FutexErrorKind {
    /// Convert to Linux errno value (negative)
    ///
    /// # Returns
    /// Negative errno value suitable for syscall return
    #[inline]
    pub const fn to_errno(self) -> i32 {
        self as i32
    }

    /// Convert from Linux errno value
    ///
    /// # Arguments
    /// - `errno`: Negative errno value
    ///
    /// # Returns
    /// Corresponding FutexErrorKind, or InvalidOperation for unknown
    pub const fn from_errno(errno: i32) -> Self {
        match errno {
            -11 => Self::WouldBlock,
            -110 => Self::TimedOut,
            -4 => Self::Interrupted,
            -14 => Self::InvalidAddress,
            -22 => Self::InvalidOperation,
            -12 => Self::NoMemory,
            -35 => Self::Deadlock,
            -38 => Self::NotImplemented,
            -3 => Self::NoSuchProcess,
            -1 => Self::PermissionDenied,
            _ => Self::InvalidOperation,
        }
    }

    /// Check if error is retryable
    ///
    /// # Returns
    /// true if caller should retry the operation
    #[inline]
    pub const fn is_retryable(self) -> bool {
        matches!(self, Self::WouldBlock | Self::Interrupted)
    }

    /// Check if error is transient
    ///
    /// # Returns
    /// true if error may resolve with time
    #[inline]
    pub const fn is_transient(self) -> bool {
        matches!(self, Self::WouldBlock | Self::Interrupted | Self::NoMemory)
    }

    /// Get human-readable error name
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WouldBlock => "EAGAIN",
            Self::TimedOut => "ETIMEDOUT",
            Self::Interrupted => "EINTR",
            Self::InvalidAddress => "EFAULT",
            Self::InvalidOperation => "EINVAL",
            Self::NoMemory => "ENOMEM",
            Self::Deadlock => "EDEADLK",
            Self::NotImplemented => "ENOSYS",
            Self::NoSuchProcess => "ESRCH",
            Self::PermissionDenied => "EPERM",
        }
    }
}

impl fmt::Display for FutexErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WouldBlock => write!(f, "EAGAIN: futex value changed"),
            Self::TimedOut => write!(f, "ETIMEDOUT: operation timed out"),
            Self::Interrupted => write!(f, "EINTR: interrupted by signal"),
            Self::InvalidAddress => write!(f, "EFAULT: invalid futex address"),
            Self::InvalidOperation => write!(f, "EINVAL: invalid operation or arguments"),
            Self::NoMemory => write!(f, "ENOMEM: out of memory"),
            Self::Deadlock => write!(f, "EDEADLK: deadlock detected"),
            Self::NotImplemented => write!(f, "ENOSYS: not implemented"),
            Self::NoSuchProcess => write!(f, "ESRCH: no such process"),
            Self::PermissionDenied => write!(f, "EPERM: permission denied"),
        }
    }
}

/// Futex error with additional context
///
/// # Layout (32 bytes)
/// - kind: 4 bytes (FutexErrorKind)
/// - address: 8 bytes (futex address that caused error)
/// - expected: 4 bytes (expected value for WouldBlock)
/// - actual: 4 bytes (actual value for WouldBlock)
/// - operation: 4 bytes (operation that failed)
/// - _padding: 8 bytes
///
/// # ASSUM Framework
/// - `#ASSUME_ERROR_SMALL`: Error fits in 32 bytes for stack allocation
/// - `#VERIFY_ERROR_SMALL`: Prevents heap allocation on error path
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, align(8))]
pub struct FutexError {
    /// Error kind (maps to errno)
    pub kind: FutexErrorKind,

    /// Futex address that caused the error (for debugging)
    ///
    /// # ASSUM_ADDRESS_VALID
    /// - Address is caller-provided, may be invalid
    /// - Used only for error reporting, not dereferenced
    pub address: u64,

    /// Expected futex value (for WouldBlock errors)
    pub expected: u32,

    /// Actual futex value observed (for WouldBlock errors)
    pub actual: u32,

    /// Operation code that failed
    pub operation: u32,

    /// Padding for alignment
    _padding: u32,
}

impl FutexError {
    /// Create new FutexError
    ///
    /// # Arguments
    /// - `kind`: Error kind
    /// - `address`: Futex address
    /// - `operation`: Operation code
    #[inline]
    pub const fn new(kind: FutexErrorKind, address: u64, operation: u32) -> Self {
        Self {
            kind,
            address,
            expected: 0,
            actual: 0,
            operation,
            _padding: 0,
        }
    }

    /// Create WouldBlock error with value mismatch details
    ///
    /// # Arguments
    /// - `address`: Futex address
    /// - `expected`: Expected value
    /// - `actual`: Actual value observed
    /// - `operation`: Operation code
    #[inline]
    pub const fn would_block(address: u64, expected: u32, actual: u32, operation: u32) -> Self {
        Self {
            kind: FutexErrorKind::WouldBlock,
            address,
            expected,
            actual,
            operation,
            _padding: 0,
        }
    }

    /// Create TimedOut error
    #[inline]
    pub const fn timed_out(address: u64, operation: u32) -> Self {
        Self::new(FutexErrorKind::TimedOut, address, operation)
    }

    /// Create InvalidAddress error
    #[inline]
    pub const fn invalid_address(address: u64, operation: u32) -> Self {
        Self::new(FutexErrorKind::InvalidAddress, address, operation)
    }

    /// Create InvalidOperation error
    #[inline]
    pub const fn invalid_operation(operation: u32) -> Self {
        Self::new(FutexErrorKind::InvalidOperation, 0, operation)
    }

    /// Create NoMemory error
    #[inline]
    pub const fn no_memory(address: u64, operation: u32) -> Self {
        Self::new(FutexErrorKind::NoMemory, address, operation)
    }

    /// Convert to Linux errno value
    #[inline]
    pub const fn to_errno(&self) -> i32 {
        self.kind.to_errno()
    }

    /// Check if error is retryable
    #[inline]
    pub const fn is_retryable(&self) -> bool {
        self.kind.is_retryable()
    }
}

impl fmt::Display for FutexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FutexError {{ kind: {}, address: {:#x}, operation: {} }}",
            self.kind, self.address, self.operation
        )
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FutexError {}

// Compile-time size verification
const _: () = {
    assert!(core::mem::size_of::<FutexError>() == 32);
    assert!(core::mem::align_of::<FutexError>() == 8);
    assert!(core::mem::size_of::<FutexErrorKind>() == 4);
};
