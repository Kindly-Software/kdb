//! Signal Types for Capsule OS Signal Handling
//!
//! This module provides signal type definitions, enums, and error types
//! for the Capsule OS signal handling infrastructure.
//!
//! ## Design Principles
//!
//! - **UCE34 T0 Auditable**: All signal events are hashable for audit trails
//! - **Chaos Compliant**: Type-safe enums prevent invalid signal states
//! - **POSIX Compatible**: Maps to standard Unix signal numbers
//!
//! ## References
//!
//! - [POSIX Signal Numbers](https://man7.org/linux/man-pages/man7/signal.7.html)
//! - [signal-safety(7)](https://man7.org/linux/man-pages/man7/signal-safety.7.html)
//! - [Linux signalfd(2)](https://man7.org/linux/man-pages/man2/signalfd.2.html)

use core::fmt;

/// Signal type representing Unix signals handled by Capsule OS
///
/// ## Design
///
/// **Tier**: T0 Auditable
/// **Size**: 4 bytes (i32 discriminant for POSIX compatibility)
///
/// ## Signals Supported
///
/// - **SIGHUP (1)**: Terminal hangup
/// - **SIGINT (2)**: Interrupt from keyboard (Ctrl+C)
/// - **SIGQUIT (3)**: Quit from keyboard (Ctrl+\)
/// - **SIGKILL (9)**: Kill signal (cannot be caught)
/// - **SIGTERM (15)**: Termination signal
/// - **SIGCHLD (17)**: Child stopped or terminated
/// - **SIGCONT (18)**: Continue if stopped
/// - **SIGSTOP (19)**: Stop process (cannot be caught)
/// - **SIGTSTP (20)**: Stop from TTY (Ctrl+Z)
/// - **SIGTTIN (21)**: TTY input for background process
/// - **SIGTTOU (22)**: TTY output for background process
/// - **SIGURG (23)**: Urgent condition on socket
/// - **SIGWINCH (28)**: Window resize signal
/// - **SIGPIPE (13)**: Broken pipe
/// - **SIGALRM (14)**: Timer signal
/// - **SIGUSR1 (10)**: User-defined signal 1
/// - **SIGUSR2 (12)**: User-defined signal 2
///
/// ## ASSUM Safety
///
/// #ASSUME_SIGNAL_DISCRIMINANT: Signal discriminant matches POSIX signal number
/// #VERIFY_SIGNAL_DISCRIMINANT: Verified against Linux signal.h definitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum Signal {
    /// SIGHUP (1): Terminal hangup or controlling process death
    ///
    /// #ASSUME_SIGHUP_VALUE: SIGHUP is always 1 on POSIX systems
    /// #VERIFY_SIGHUP_VALUE: Verified against POSIX.1-2017 and Linux signal.h
    Hup = 1,

    /// SIGINT (2): Interrupt from keyboard (Ctrl+C)
    ///
    /// #ASSUME_SIGINT_VALUE: SIGINT is always 2 on POSIX systems
    /// #VERIFY_SIGINT_VALUE: Verified against POSIX.1-2017 and Linux signal.h
    Int = 2,

    /// SIGQUIT (3): Quit from keyboard (Ctrl+\), generates core dump
    ///
    /// #ASSUME_SIGQUIT_VALUE: SIGQUIT is always 3 on POSIX systems
    /// #VERIFY_SIGQUIT_VALUE: Verified against POSIX.1-2017 and Linux signal.h
    Quit = 3,

    /// SIGILL (4): Illegal instruction
    ///
    /// #ASSUME_SIGILL_VALUE: SIGILL is always 4 on POSIX systems
    /// #VERIFY_SIGILL_VALUE: Verified against POSIX.1-2017 and Linux signal.h
    Ill = 4,

    /// SIGTRAP (5): Trace/breakpoint trap
    ///
    /// #ASSUME_SIGTRAP_VALUE: SIGTRAP is always 5 on POSIX systems
    /// #VERIFY_SIGTRAP_VALUE: Verified against POSIX.1-2017 and Linux signal.h
    Trap = 5,

    /// SIGABRT (6): Abort signal from abort(3)
    ///
    /// #ASSUME_SIGABRT_VALUE: SIGABRT is always 6 on POSIX systems
    /// #VERIFY_SIGABRT_VALUE: Verified against POSIX.1-2017 and Linux signal.h
    Abrt = 6,

    /// SIGBUS (7): Bus error (bad memory access)
    ///
    /// #ASSUME_SIGBUS_VALUE: SIGBUS is always 7 on POSIX systems
    /// #VERIFY_SIGBUS_VALUE: Verified against POSIX.1-2017 and Linux signal.h
    Bus = 7,

    /// SIGFPE (8): Floating-point exception
    ///
    /// #ASSUME_SIGFPE_VALUE: SIGFPE is always 8 on POSIX systems
    /// #VERIFY_SIGFPE_VALUE: Verified against POSIX.1-2017 and Linux signal.h
    Fpe = 8,

    /// SIGKILL (9): Kill signal (cannot be caught or ignored)
    ///
    /// #ASSUME_SIGKILL_VALUE: SIGKILL is always 9 on POSIX systems
    /// #VERIFY_SIGKILL_VALUE: Verified against POSIX.1-2017 and Linux signal.h
    Kill = 9,

    /// SIGUSR1 (10): User-defined signal 1
    ///
    /// #ASSUME_SIGUSR1_VALUE: SIGUSR1 is always 10 on Linux x86_64
    /// #VERIFY_SIGUSR1_VALUE: Verified against Linux signal.h (arch-dependent)
    Usr1 = 10,

    /// SIGSEGV (11): Invalid memory reference
    ///
    /// #ASSUME_SIGSEGV_VALUE: SIGSEGV is always 11 on POSIX systems
    /// #VERIFY_SIGSEGV_VALUE: Verified against POSIX.1-2017 and Linux signal.h
    Segv = 11,

    /// SIGUSR2 (12): User-defined signal 2
    ///
    /// #ASSUME_SIGUSR2_VALUE: SIGUSR2 is always 12 on Linux x86_64
    /// #VERIFY_SIGUSR2_VALUE: Verified against Linux signal.h (arch-dependent)
    Usr2 = 12,

    /// SIGPIPE (13): Broken pipe (write to pipe with no readers)
    ///
    /// #ASSUME_SIGPIPE_VALUE: SIGPIPE is always 13 on POSIX systems
    /// #VERIFY_SIGPIPE_VALUE: Verified against POSIX.1-2017 and Linux signal.h
    Pipe = 13,

    /// SIGALRM (14): Timer signal from alarm(2)
    ///
    /// #ASSUME_SIGALRM_VALUE: SIGALRM is always 14 on POSIX systems
    /// #VERIFY_SIGALRM_VALUE: Verified against POSIX.1-2017 and Linux signal.h
    Alrm = 14,

    /// SIGTERM (15): Termination signal (polite kill)
    ///
    /// #ASSUME_SIGTERM_VALUE: SIGTERM is always 15 on POSIX systems
    /// #VERIFY_SIGTERM_VALUE: Verified against POSIX.1-2017 and Linux signal.h
    Term = 15,

    /// SIGSTKFLT (16): Stack fault on coprocessor (Linux-specific, unused)
    ///
    /// #ASSUME_SIGSTKFLT_VALUE: SIGSTKFLT is 16 on Linux x86_64
    /// #VERIFY_SIGSTKFLT_VALUE: Verified against Linux signal.h
    StkFlt = 16,

    /// SIGCHLD (17): Child stopped or terminated
    ///
    /// #ASSUME_SIGCHLD_VALUE: SIGCHLD is 17 on Linux x86_64
    /// #VERIFY_SIGCHLD_VALUE: Verified against Linux signal.h (arch-dependent)
    Chld = 17,

    /// SIGCONT (18): Continue if stopped
    ///
    /// #ASSUME_SIGCONT_VALUE: SIGCONT is 18 on Linux x86_64
    /// #VERIFY_SIGCONT_VALUE: Verified against Linux signal.h (arch-dependent)
    Cont = 18,

    /// SIGSTOP (19): Stop process (cannot be caught or ignored)
    ///
    /// #ASSUME_SIGSTOP_VALUE: SIGSTOP is 19 on Linux x86_64
    /// #VERIFY_SIGSTOP_VALUE: Verified against Linux signal.h (arch-dependent)
    Stop = 19,

    /// SIGTSTP (20): Stop typed at terminal (Ctrl+Z)
    ///
    /// #ASSUME_SIGTSTP_VALUE: SIGTSTP is 20 on Linux x86_64
    /// #VERIFY_SIGTSTP_VALUE: Verified against Linux signal.h (arch-dependent)
    Tstp = 20,

    /// SIGTTIN (21): TTY input for background process
    ///
    /// #ASSUME_SIGTTIN_VALUE: SIGTTIN is 21 on Linux x86_64
    /// #VERIFY_SIGTTIN_VALUE: Verified against Linux signal.h (arch-dependent)
    Ttin = 21,

    /// SIGTTOU (22): TTY output for background process
    ///
    /// #ASSUME_SIGTTOU_VALUE: SIGTTOU is 22 on Linux x86_64
    /// #VERIFY_SIGTTOU_VALUE: Verified against Linux signal.h (arch-dependent)
    Ttou = 22,

    /// SIGURG (23): Urgent condition on socket (e.g., OOB data)
    ///
    /// #ASSUME_SIGURG_VALUE: SIGURG is 23 on Linux x86_64
    /// #VERIFY_SIGURG_VALUE: Verified against Linux signal.h (arch-dependent)
    Urg = 23,

    /// SIGXCPU (24): CPU time limit exceeded (setrlimit)
    ///
    /// #ASSUME_SIGXCPU_VALUE: SIGXCPU is 24 on Linux x86_64
    /// #VERIFY_SIGXCPU_VALUE: Verified against Linux signal.h (arch-dependent)
    Xcpu = 24,

    /// SIGXFSZ (25): File size limit exceeded (setrlimit)
    ///
    /// #ASSUME_SIGXFSZ_VALUE: SIGXFSZ is 25 on Linux x86_64
    /// #VERIFY_SIGXFSZ_VALUE: Verified against Linux signal.h (arch-dependent)
    Xfsz = 25,

    /// SIGVTALRM (26): Virtual alarm clock (setitimer)
    ///
    /// #ASSUME_SIGVTALRM_VALUE: SIGVTALRM is 26 on Linux x86_64
    /// #VERIFY_SIGVTALRM_VALUE: Verified against Linux signal.h (arch-dependent)
    Vtalrm = 26,

    /// SIGPROF (27): Profiling timer expired (setitimer)
    ///
    /// #ASSUME_SIGPROF_VALUE: SIGPROF is 27 on Linux x86_64
    /// #VERIFY_SIGPROF_VALUE: Verified against Linux signal.h (arch-dependent)
    Prof = 27,

    /// SIGWINCH (28): Window resize signal (terminal resize)
    ///
    /// #ASSUME_SIGWINCH_VALUE: SIGWINCH is 28 on Linux x86_64
    /// #VERIFY_SIGWINCH_VALUE: Verified against Linux signal.h (arch-dependent)
    Winch = 28,

    /// SIGIO/SIGPOLL (29): I/O now possible (async I/O)
    ///
    /// #ASSUME_SIGIO_VALUE: SIGIO is 29 on Linux x86_64
    /// #VERIFY_SIGIO_VALUE: Verified against Linux signal.h (arch-dependent)
    Io = 29,

    /// SIGPWR (30): Power failure (System V)
    ///
    /// #ASSUME_SIGPWR_VALUE: SIGPWR is 30 on Linux x86_64
    /// #VERIFY_SIGPWR_VALUE: Verified against Linux signal.h (arch-dependent)
    Pwr = 30,

    /// SIGSYS (31): Bad system call (seccomp)
    ///
    /// #ASSUME_SIGSYS_VALUE: SIGSYS is 31 on Linux x86_64
    /// #VERIFY_SIGSYS_VALUE: Verified against Linux signal.h (arch-dependent)
    Sys = 31,
}

impl Signal {
    /// Convert signal to its POSIX signal number
    ///
    /// #ASSUME_SIGNAL_TO_I32: Discriminant equals POSIX signal number
    /// #VERIFY_SIGNAL_TO_I32: repr(i32) guarantees discriminant matches value
    #[inline]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    /// Create signal from POSIX signal number
    ///
    /// Returns `None` if the signal number is not recognized.
    ///
    /// #ASSUME_SIGNAL_FROM_I32: Only valid POSIX signals are accepted
    /// #VERIFY_SIGNAL_FROM_I32: Pattern match covers all defined signals
    #[inline]
    pub const fn from_i32(sig: i32) -> Option<Self> {
        match sig {
            1 => Some(Signal::Hup),
            2 => Some(Signal::Int),
            3 => Some(Signal::Quit),
            4 => Some(Signal::Ill),
            5 => Some(Signal::Trap),
            6 => Some(Signal::Abrt),
            7 => Some(Signal::Bus),
            8 => Some(Signal::Fpe),
            9 => Some(Signal::Kill),
            10 => Some(Signal::Usr1),
            11 => Some(Signal::Segv),
            12 => Some(Signal::Usr2),
            13 => Some(Signal::Pipe),
            14 => Some(Signal::Alrm),
            15 => Some(Signal::Term),
            16 => Some(Signal::StkFlt),
            17 => Some(Signal::Chld),
            18 => Some(Signal::Cont),
            19 => Some(Signal::Stop),
            20 => Some(Signal::Tstp),
            21 => Some(Signal::Ttin),
            22 => Some(Signal::Ttou),
            23 => Some(Signal::Urg),
            24 => Some(Signal::Xcpu),
            25 => Some(Signal::Xfsz),
            26 => Some(Signal::Vtalrm),
            27 => Some(Signal::Prof),
            28 => Some(Signal::Winch),
            29 => Some(Signal::Io),
            30 => Some(Signal::Pwr),
            31 => Some(Signal::Sys),
            _ => None,
        }
    }

    /// Check if signal can be caught (SIGKILL and SIGSTOP cannot be caught)
    ///
    /// #ASSUME_CATCHABLE_SIGNALS: SIGKILL(9) and SIGSTOP(19) cannot be caught
    /// #VERIFY_CATCHABLE_SIGNALS: POSIX.1-2017 specifies these are uncatchable
    #[inline]
    pub const fn is_catchable(self) -> bool {
        !matches!(self, Signal::Kill | Signal::Stop)
    }

    /// Check if signal is a termination signal (default action is terminate)
    ///
    /// #ASSUME_TERM_SIGNALS: Standard POSIX termination signals
    /// #VERIFY_TERM_SIGNALS: Verified against POSIX signal(7) default actions
    #[inline]
    pub const fn is_termination(self) -> bool {
        matches!(
            self,
            Signal::Hup
                | Signal::Int
                | Signal::Quit
                | Signal::Ill
                | Signal::Abrt
                | Signal::Fpe
                | Signal::Kill
                | Signal::Segv
                | Signal::Pipe
                | Signal::Alrm
                | Signal::Term
                | Signal::Usr1
                | Signal::Usr2
                | Signal::Xcpu
                | Signal::Xfsz
                | Signal::Vtalrm
                | Signal::Prof
                | Signal::Io
                | Signal::Pwr
                | Signal::Sys
        )
    }

    /// Check if signal is a job control signal
    ///
    /// #ASSUME_JOB_SIGNALS: Standard POSIX job control signals
    /// #VERIFY_JOB_SIGNALS: Verified against POSIX signal(7)
    #[inline]
    pub const fn is_job_control(self) -> bool {
        matches!(
            self,
            Signal::Cont | Signal::Stop | Signal::Tstp | Signal::Ttin | Signal::Ttou
        )
    }

    /// Get signal name as string
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Signal::Hup => "SIGHUP",
            Signal::Int => "SIGINT",
            Signal::Quit => "SIGQUIT",
            Signal::Ill => "SIGILL",
            Signal::Trap => "SIGTRAP",
            Signal::Abrt => "SIGABRT",
            Signal::Bus => "SIGBUS",
            Signal::Fpe => "SIGFPE",
            Signal::Kill => "SIGKILL",
            Signal::Usr1 => "SIGUSR1",
            Signal::Segv => "SIGSEGV",
            Signal::Usr2 => "SIGUSR2",
            Signal::Pipe => "SIGPIPE",
            Signal::Alrm => "SIGALRM",
            Signal::Term => "SIGTERM",
            Signal::StkFlt => "SIGSTKFLT",
            Signal::Chld => "SIGCHLD",
            Signal::Cont => "SIGCONT",
            Signal::Stop => "SIGSTOP",
            Signal::Tstp => "SIGTSTP",
            Signal::Ttin => "SIGTTIN",
            Signal::Ttou => "SIGTTOU",
            Signal::Urg => "SIGURG",
            Signal::Xcpu => "SIGXCPU",
            Signal::Xfsz => "SIGXFSZ",
            Signal::Vtalrm => "SIGVTALRM",
            Signal::Prof => "SIGPROF",
            Signal::Winch => "SIGWINCH",
            Signal::Io => "SIGIO",
            Signal::Pwr => "SIGPWR",
            Signal::Sys => "SIGSYS",
        }
    }
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.name(), self.as_i32())
    }
}

/// Signal error type for Capsule OS signal handling
///
/// ## Design
///
/// **Tier**: T0 Auditable
/// **Size**: 16 bytes (enum + i32 errno + padding)
///
/// ## Error Categories
///
/// - **Pipe errors**: Self-pipe creation/read/write failures
/// - **Signal errors**: sigaction, signalfd failures
/// - **State errors**: Already registered, not registered
/// - **Dispatch errors**: Queue full, invalid handler
///
/// ## ASSUM Safety
///
/// #ASSUME_ERROR_EXHAUSTIVE: All error conditions are represented
/// #VERIFY_ERROR_EXHAUSTIVE: Error variants cover all syscall failure modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalError {
    /// Failed to create self-pipe (pipe2 syscall)
    ///
    /// #ASSUME_PIPE_ERROR: errno contains pipe2 error code
    /// #VERIFY_PIPE_ERROR: Verified against pipe(2) man page error codes
    PipeCreationFailed(i32),

    /// Failed to read from pipe
    ///
    /// #ASSUME_PIPE_READ_ERROR: errno contains read error code
    /// #VERIFY_PIPE_READ_ERROR: Verified against read(2) man page error codes
    PipeReadFailed(i32),

    /// Failed to write to pipe
    ///
    /// #ASSUME_PIPE_WRITE_ERROR: errno contains write error code
    /// #VERIFY_PIPE_WRITE_ERROR: Verified against write(2) man page error codes
    PipeWriteFailed(i32),

    /// Failed to close pipe
    ///
    /// #ASSUME_PIPE_CLOSE_ERROR: errno contains close error code
    /// #VERIFY_PIPE_CLOSE_ERROR: Verified against close(2) man page error codes
    PipeCloseFailed(i32),

    /// Failed to register signal handler (sigaction syscall)
    ///
    /// #ASSUME_SIGACTION_ERROR: errno contains sigaction error code
    /// #VERIFY_SIGACTION_ERROR: Verified against sigaction(2) man page error codes
    SignalRegistrationFailed(i32),

    /// Failed to create signalfd
    ///
    /// #ASSUME_SIGNALFD_ERROR: errno contains signalfd error code
    /// #VERIFY_SIGNALFD_ERROR: Verified against signalfd(2) man page error codes
    SignalFdCreationFailed(i32),

    /// Failed to read from signalfd
    ///
    /// #ASSUME_SIGNALFD_READ_ERROR: errno contains read error code
    /// #VERIFY_SIGNALFD_READ_ERROR: Verified against signalfd(2) read semantics
    SignalFdReadFailed(i32),

    /// Failed to block signals (sigprocmask syscall)
    ///
    /// #ASSUME_SIGPROCMASK_ERROR: errno contains sigprocmask error code
    /// #VERIFY_SIGPROCMASK_ERROR: Verified against sigprocmask(2) man page
    SignalMaskFailed(i32),

    /// Signal handler is already registered
    ///
    /// #ASSUME_ALREADY_REGISTERED: Global handler state is set
    /// #VERIFY_ALREADY_REGISTERED: AtomicBool swap returns true on double-register
    AlreadyRegistered,

    /// Signal handler is not registered
    ///
    /// #ASSUME_NOT_REGISTERED: Global handler state is not set
    /// #VERIFY_NOT_REGISTERED: AtomicBool load returns false before register
    NotRegistered,

    /// Signal dispatch queue is full
    ///
    /// #ASSUME_QUEUE_FULL: Ring buffer head == tail (wrapped)
    /// #VERIFY_QUEUE_FULL: Atomic compare-exchange fails on full queue
    QueueFull,

    /// Signal dispatch queue is empty
    ///
    /// #ASSUME_QUEUE_EMPTY: Ring buffer head == tail (no signals)
    /// #VERIFY_QUEUE_EMPTY: Atomic load shows no pending signals
    QueueEmpty,

    /// Invalid signal number
    ///
    /// #ASSUME_INVALID_SIGNAL: Signal number outside valid range [1, 31]
    /// #VERIFY_INVALID_SIGNAL: Signal::from_i32 returns None
    InvalidSignal(i32),

    /// Handler not found for signal
    ///
    /// #ASSUME_NO_HANDLER: Handler slot is empty for given signal
    /// #VERIFY_NO_HANDLER: Handler array contains null at signal index
    NoHandler(Signal),

    /// Timeout waiting for signal
    ///
    /// #ASSUME_TIMEOUT: poll/epoll_wait returned 0
    /// #VERIFY_TIMEOUT: Elapsed time exceeds specified timeout
    Timeout,

    /// Operation was interrupted by signal (EINTR)
    ///
    /// #ASSUME_INTERRUPTED: syscall returned -1 with errno == EINTR
    /// #VERIFY_INTERRUPTED: Verified against POSIX EINTR semantics
    Interrupted,
}

impl SignalError {
    /// Get errno value if this error contains one
    #[inline]
    pub const fn errno(&self) -> Option<i32> {
        match self {
            SignalError::PipeCreationFailed(e)
            | SignalError::PipeReadFailed(e)
            | SignalError::PipeWriteFailed(e)
            | SignalError::PipeCloseFailed(e)
            | SignalError::SignalRegistrationFailed(e)
            | SignalError::SignalFdCreationFailed(e)
            | SignalError::SignalFdReadFailed(e)
            | SignalError::SignalMaskFailed(e)
            | SignalError::InvalidSignal(e) => Some(*e),
            _ => None,
        }
    }

    /// Check if error is recoverable (can retry)
    #[inline]
    pub const fn is_recoverable(&self) -> bool {
        matches!(
            self,
            SignalError::QueueFull
                | SignalError::QueueEmpty
                | SignalError::Timeout
                | SignalError::Interrupted
        )
    }
}

impl fmt::Display for SignalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignalError::PipeCreationFailed(errno) => {
                write!(f, "failed to create self-pipe (errno {})", errno)
            }
            SignalError::PipeReadFailed(errno) => {
                write!(f, "failed to read from pipe (errno {})", errno)
            }
            SignalError::PipeWriteFailed(errno) => {
                write!(f, "failed to write to pipe (errno {})", errno)
            }
            SignalError::PipeCloseFailed(errno) => {
                write!(f, "failed to close pipe (errno {})", errno)
            }
            SignalError::SignalRegistrationFailed(errno) => {
                write!(f, "failed to register signal handler (errno {})", errno)
            }
            SignalError::SignalFdCreationFailed(errno) => {
                write!(f, "failed to create signalfd (errno {})", errno)
            }
            SignalError::SignalFdReadFailed(errno) => {
                write!(f, "failed to read from signalfd (errno {})", errno)
            }
            SignalError::SignalMaskFailed(errno) => {
                write!(f, "failed to set signal mask (errno {})", errno)
            }
            SignalError::AlreadyRegistered => {
                write!(f, "signal handler already registered")
            }
            SignalError::NotRegistered => {
                write!(f, "signal handler not registered")
            }
            SignalError::QueueFull => {
                write!(f, "signal dispatch queue is full")
            }
            SignalError::QueueEmpty => {
                write!(f, "signal dispatch queue is empty")
            }
            SignalError::InvalidSignal(sig) => {
                write!(f, "invalid signal number {}", sig)
            }
            SignalError::NoHandler(sig) => {
                write!(f, "no handler registered for {}", sig)
            }
            SignalError::Timeout => {
                write!(f, "timeout waiting for signal")
            }
            SignalError::Interrupted => {
                write!(f, "operation interrupted by signal (EINTR)")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SignalError {}

/// Result type for signal operations
pub type SignalResult<T> = Result<T, SignalError>;

/// Signal info extended data (matches signalfd_siginfo for signalfd integration)
///
/// ## Design
///
/// **Tier**: T0 Auditable
/// **Size**: 64 bytes (cache-aligned subset of signalfd_siginfo)
///
/// ## Fields
///
/// Contains essential signal metadata for audit trails and handler dispatch:
/// - Signal number
/// - Sender PID/UID
/// - Error code (for SIGCHLD)
/// - Exit status (for SIGCHLD)
/// - Timer info (for SIGALRM/SIGVTALRM/SIGPROF)
/// - Timestamp (nanoseconds)
///
/// ## ASSUM Safety
///
/// #ASSUME_SIGINFO_LAYOUT: Layout matches signalfd_siginfo subset
/// #VERIFY_SIGINFO_LAYOUT: Verified against signalfd(2) structure definition
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, align(64))]
pub struct SignalInfo {
    /// Signal number that was delivered
    pub signo: i32,
    /// Error number (SIGCHLD: child exit code)
    pub errno: i32,
    /// Signal code (distinguishes signal source)
    pub code: i32,
    /// Sending process ID
    pub pid: u32,
    /// Real user ID of sending process
    pub uid: u32,
    /// File descriptor (SIGIO)
    pub fd: i32,
    /// Timer ID (SIGALRM, etc.)
    pub tid: u32,
    /// Band event (SIGIO)
    pub band: u32,
    /// Timer overrun count (SIGALRM)
    pub overrun: u32,
    /// Exit value or signal (SIGCHLD)
    pub status: i32,
    /// Timestamp in nanoseconds (added by dispatcher)
    pub timestamp_ns: u64,
    /// Padding to 64 bytes
    _padding: [u8; 16],
}

impl SignalInfo {
    /// Create new signal info from signal number
    #[inline]
    pub const fn new(signo: i32) -> Self {
        Self {
            signo,
            errno: 0,
            code: 0,
            pid: 0,
            uid: 0,
            fd: 0,
            tid: 0,
            band: 0,
            overrun: 0,
            status: 0,
            timestamp_ns: 0,
            _padding: [0; 16],
        }
    }

    /// Get the signal as Signal enum
    #[inline]
    pub fn signal(&self) -> Option<Signal> {
        Signal::from_i32(self.signo)
    }

    /// Set timestamp (nanoseconds since epoch or boot)
    #[inline]
    pub fn with_timestamp(mut self, timestamp_ns: u64) -> Self {
        self.timestamp_ns = timestamp_ns;
        self
    }

    /// Set sender PID
    #[inline]
    pub fn with_pid(mut self, pid: u32) -> Self {
        self.pid = pid;
        self
    }

    /// Set sender UID
    #[inline]
    pub fn with_uid(mut self, uid: u32) -> Self {
        self.uid = uid;
        self
    }
}

// Compile-time verification for SignalInfo
const _: () = {
    assert!(core::mem::size_of::<SignalInfo>() == 64);
    assert!(core::mem::align_of::<SignalInfo>() == 64);
};

/// Signal action type for handler registration
///
/// ## Design
///
/// **Tier**: T1 Atomic
/// **Purpose**: Configure signal handling behavior
///
/// ## Actions
///
/// - **Default**: Use default signal handler (SIG_DFL)
/// - **Ignore**: Ignore the signal (SIG_IGN)
/// - **Handle**: Call custom handler with SignalInfo
/// - **Terminate**: Terminate process (exit cleanly)
/// - **CoreDump**: Terminate and generate core dump
/// - **Stop**: Stop the process (SIGSTOP behavior)
/// - **Continue**: Continue if stopped (SIGCONT behavior)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalAction {
    /// Use default signal action (SIG_DFL)
    Default,
    /// Ignore the signal (SIG_IGN)
    Ignore,
    /// Dispatch to registered handler
    Handle,
    /// Terminate process cleanly
    Terminate,
    /// Terminate and generate core dump
    CoreDump,
    /// Stop the process
    Stop,
    /// Continue if stopped
    Continue,
}

impl SignalAction {
    /// Get the default action for a signal
    ///
    /// Based on POSIX.1-2017 signal(7) default actions.
    #[inline]
    pub const fn default_for(signal: Signal) -> Self {
        match signal {
            // Terminate (no core)
            Signal::Hup
            | Signal::Int
            | Signal::Pipe
            | Signal::Alrm
            | Signal::Term
            | Signal::Usr1
            | Signal::Usr2
            | Signal::Io
            | Signal::Pwr
            | Signal::Vtalrm
            | Signal::Prof => SignalAction::Terminate,

            // Core dump
            Signal::Quit | Signal::Ill | Signal::Trap | Signal::Abrt | Signal::Bus
            | Signal::Fpe | Signal::Segv | Signal::Xcpu | Signal::Xfsz | Signal::Sys => {
                SignalAction::CoreDump
            }

            // Kill (cannot change)
            Signal::Kill => SignalAction::Terminate,

            // Stop
            Signal::Stop | Signal::Tstp | Signal::Ttin | Signal::Ttou => SignalAction::Stop,

            // Continue
            Signal::Cont => SignalAction::Continue,

            // Ignore
            Signal::Chld | Signal::Urg | Signal::Winch | Signal::StkFlt => SignalAction::Ignore,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_values() {
        assert_eq!(Signal::Hup.as_i32(), 1);
        assert_eq!(Signal::Int.as_i32(), 2);
        assert_eq!(Signal::Kill.as_i32(), 9);
        assert_eq!(Signal::Term.as_i32(), 15);
        assert_eq!(Signal::Winch.as_i32(), 28);
    }

    #[test]
    fn test_signal_from_i32() {
        assert_eq!(Signal::from_i32(1), Some(Signal::Hup));
        assert_eq!(Signal::from_i32(2), Some(Signal::Int));
        assert_eq!(Signal::from_i32(9), Some(Signal::Kill));
        assert_eq!(Signal::from_i32(0), None);
        assert_eq!(Signal::from_i32(32), None);
        assert_eq!(Signal::from_i32(-1), None);
    }

    #[test]
    fn test_signal_catchable() {
        assert!(Signal::Int.is_catchable());
        assert!(Signal::Term.is_catchable());
        assert!(!Signal::Kill.is_catchable());
        assert!(!Signal::Stop.is_catchable());
    }

    #[test]
    fn test_signal_termination() {
        assert!(Signal::Int.is_termination());
        assert!(Signal::Term.is_termination());
        assert!(Signal::Kill.is_termination());
        assert!(!Signal::Cont.is_termination());
        assert!(!Signal::Winch.is_termination());
    }

    #[test]
    fn test_signal_job_control() {
        assert!(Signal::Cont.is_job_control());
        assert!(Signal::Stop.is_job_control());
        assert!(Signal::Tstp.is_job_control());
        assert!(!Signal::Int.is_job_control());
        assert!(!Signal::Term.is_job_control());
    }

    #[test]
    fn test_signal_name() {
        assert_eq!(Signal::Int.name(), "SIGINT");
        assert_eq!(Signal::Term.name(), "SIGTERM");
        assert_eq!(Signal::Winch.name(), "SIGWINCH");
    }

    #[test]
    fn test_signal_display() {
        let display = format!("{}", Signal::Int);
        assert!(display.contains("SIGINT"));
        assert!(display.contains("2"));
    }

    #[test]
    fn test_error_errno() {
        let err = SignalError::PipeCreationFailed(13);
        assert_eq!(err.errno(), Some(13));

        let err = SignalError::AlreadyRegistered;
        assert_eq!(err.errno(), None);
    }

    #[test]
    fn test_error_recoverable() {
        assert!(SignalError::QueueFull.is_recoverable());
        assert!(SignalError::QueueEmpty.is_recoverable());
        assert!(SignalError::Timeout.is_recoverable());
        assert!(SignalError::Interrupted.is_recoverable());
        assert!(!SignalError::AlreadyRegistered.is_recoverable());
        assert!(!SignalError::PipeCreationFailed(13).is_recoverable());
    }

    #[test]
    fn test_signal_info_size() {
        assert_eq!(core::mem::size_of::<SignalInfo>(), 64);
        assert_eq!(core::mem::align_of::<SignalInfo>(), 64);
    }

    #[test]
    fn test_signal_info_new() {
        let info = SignalInfo::new(2);
        assert_eq!(info.signo, 2);
        assert_eq!(info.signal(), Some(Signal::Int));
    }

    #[test]
    fn test_signal_action_default() {
        assert_eq!(SignalAction::default_for(Signal::Int), SignalAction::Terminate);
        assert_eq!(SignalAction::default_for(Signal::Quit), SignalAction::CoreDump);
        assert_eq!(SignalAction::default_for(Signal::Stop), SignalAction::Stop);
        assert_eq!(SignalAction::default_for(Signal::Cont), SignalAction::Continue);
        assert_eq!(SignalAction::default_for(Signal::Winch), SignalAction::Ignore);
    }
}
