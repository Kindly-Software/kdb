//! T28 Q1-Q7: Unit Tests for TerminalShellCapsule
//!
//! ## Test Coverage
//!
//! - Q1: Size and alignment verification
//! - Q2: Initial state correctness
//! - Q3: State transitions
//! - Q4: Buffer operations (ring buffer)
//! - Q5: Signal enum values
//! - Q6: Job struct layout
//! - Q7: Metrics tracking

#![cfg(all(unix, feature = "tui-terminal", feature = "terminal-unix"))]

use atomic_capsule::terminal::shell::{TerminalShellCapsule, ShellState, ShellError, Signal, Job};

// ============================================================================
// Q1: SIZE AND ALIGNMENT VERIFICATION
// ============================================================================

#[test]
fn q1_shell_capsule_size() {
    assert_eq!(
        core::mem::size_of::<TerminalShellCapsule>(),
        1024,
        "TerminalShellCapsule must be exactly 1024 bytes"
    );
}

#[test]
fn q1_shell_capsule_alignment() {
    assert_eq!(
        core::mem::align_of::<TerminalShellCapsule>(),
        64,
        "TerminalShellCapsule must be 64-byte aligned (cache line)"
    );
}

#[test]
fn q1_job_size() {
    assert_eq!(
        core::mem::size_of::<Job>(),
        16,
        "Job must be 16 bytes (4+4+1+3 padding)"
    );
}

#[test]
fn q1_shell_state_size() {
    assert_eq!(
        core::mem::size_of::<ShellState>(),
        1,
        "ShellState must be 1 byte (u8 repr)"
    );
}

// ============================================================================
// Q2: INITIAL STATE CORRECTNESS
// ============================================================================

#[test]
fn q2_new_shell_not_started() {
    let shell = TerminalShellCapsule::new();
    assert_eq!(shell.state(), ShellState::NotStarted);
}

#[test]
fn q2_new_shell_not_running() {
    let shell = TerminalShellCapsule::new();
    assert!(!shell.is_running());
}

#[test]
fn q2_new_shell_no_exit_code() {
    let shell = TerminalShellCapsule::new();
    assert_eq!(shell.exit_code(), None);
}

#[test]
fn q2_new_shell_default_size() {
    let shell = TerminalShellCapsule::new();
    assert_eq!(shell.size(), (80, 24));
}

#[test]
fn q2_new_shell_zero_metrics() {
    let shell = TerminalShellCapsule::new();
    assert_eq!(shell.bytes_read(), 0);
    assert_eq!(shell.bytes_written(), 0);
    assert_eq!(shell.generation(), 0);
}

#[test]
fn q2_new_shell_empty_buffers() {
    let shell = TerminalShellCapsule::new();
    assert!(!shell.has_data());
    assert_eq!(shell.read_available(), 0);
    assert_eq!(shell.write_space(), 255); // Ring buffer max fill
}

#[test]
fn q2_new_shell_no_jobs() {
    let shell = TerminalShellCapsule::new();
    assert_eq!(shell.jobs().len(), 0);
}

#[test]
fn q2_default_impl() {
    let shell = TerminalShellCapsule::default();
    assert_eq!(shell.state(), ShellState::NotStarted);
    assert_eq!(shell.size(), (80, 24));
}

// ============================================================================
// Q3: STATE TRANSITIONS
// ============================================================================

#[test]
fn q3_shell_state_from_u8() {
    assert_eq!(ShellState::from(0), ShellState::NotStarted);
    assert_eq!(ShellState::from(1), ShellState::Starting);
    assert_eq!(ShellState::from(2), ShellState::Running);
    assert_eq!(ShellState::from(3), ShellState::Stopped);
    assert_eq!(ShellState::from(4), ShellState::Exited);
    assert_eq!(ShellState::from(5), ShellState::Error);
    assert_eq!(ShellState::from(255), ShellState::Error); // Invalid -> Error
}

#[test]
fn q3_shell_state_enum_values() {
    assert_eq!(ShellState::NotStarted as u8, 0);
    assert_eq!(ShellState::Starting as u8, 1);
    assert_eq!(ShellState::Running as u8, 2);
    assert_eq!(ShellState::Stopped as u8, 3);
    assert_eq!(ShellState::Exited as u8, 4);
    assert_eq!(ShellState::Error as u8, 5);
}

#[test]
fn q3_shell_state_eq() {
    assert_eq!(ShellState::NotStarted, ShellState::NotStarted);
    assert_ne!(ShellState::Running, ShellState::Stopped);
}

#[test]
fn q3_shell_state_clone_copy() {
    let state = ShellState::Running;
    let state2 = state; // Copy
    let state3 = state.clone(); // Clone
    assert_eq!(state, state2);
    assert_eq!(state, state3);
}

// ============================================================================
// Q4: BUFFER OPERATIONS
// ============================================================================

#[test]
fn q4_empty_buffer_has_no_data() {
    let shell = TerminalShellCapsule::new();
    assert!(!shell.has_data());
}

#[test]
fn q4_empty_buffer_available_zero() {
    let shell = TerminalShellCapsule::new();
    assert_eq!(shell.read_available(), 0);
}

#[test]
fn q4_empty_buffer_write_space_max() {
    let shell = TerminalShellCapsule::new();
    // Ring buffer reserves 1 slot to distinguish full from empty
    assert_eq!(shell.write_space(), 255);
}

#[test]
fn q4_read_from_not_running_fails() {
    let shell = TerminalShellCapsule::new();
    let mut buf = [0u8; 64];
    let result = shell.read(&mut buf);
    assert!(result.is_err());
    match result {
        Err(ShellError::NotRunning) => {},
        _ => panic!("Expected NotRunning error"),
    }
}

#[test]
fn q4_write_to_not_running_fails() {
    let shell = TerminalShellCapsule::new();
    let result = shell.write(b"test");
    assert!(result.is_err());
    match result {
        Err(ShellError::NotRunning) => {},
        _ => panic!("Expected NotRunning error"),
    }
}

// ============================================================================
// Q5: SIGNAL ENUM VALUES
// ============================================================================

#[test]
fn q5_signal_interrupt() {
    assert_eq!(Signal::Interrupt as i32, 2); // SIGINT
}

#[test]
fn q5_signal_quit() {
    assert_eq!(Signal::Quit as i32, 3); // SIGQUIT
}

#[test]
fn q5_signal_kill() {
    assert_eq!(Signal::Kill as i32, 9); // SIGKILL
}

#[test]
fn q5_signal_terminate() {
    assert_eq!(Signal::Terminate as i32, 15); // SIGTERM
}

#[test]
fn q5_signal_continue() {
    assert_eq!(Signal::Continue as i32, 18); // SIGCONT
}

#[test]
fn q5_signal_stop() {
    assert_eq!(Signal::Stop as i32, 19); // SIGSTOP
}

#[test]
fn q5_signal_window_change() {
    assert_eq!(Signal::WindowChange as i32, 28); // SIGWINCH
}

#[test]
fn q5_signal_eq() {
    assert_eq!(Signal::Interrupt, Signal::Interrupt);
    assert_ne!(Signal::Interrupt, Signal::Kill);
}

#[test]
fn q5_signal_clone_copy() {
    let sig = Signal::Interrupt;
    let sig2 = sig; // Copy
    let sig3 = sig.clone(); // Clone
    assert_eq!(sig, sig2);
    assert_eq!(sig, sig3);
}

// ============================================================================
// Q6: JOB STRUCT LAYOUT
// ============================================================================

#[test]
fn q6_job_size_16_bytes() {
    assert_eq!(core::mem::size_of::<Job>(), 16);
}

#[test]
fn q6_job_fields() {
    let job = Job {
        pid: 1234,
        pgid: 1234,
        state: 0, // Running
        _reserved: [0; 3],
    };

    assert_eq!(job.pid, 1234);
    assert_eq!(job.pgid, 1234);
    assert_eq!(job.state, 0);
}

#[test]
fn q6_job_clone_copy() {
    let job = Job {
        pid: 1234,
        pgid: 1234,
        state: 0,
        _reserved: [0; 3],
    };

    let job2 = job; // Copy
    let job3 = job.clone(); // Clone

    assert_eq!(job.pid, job2.pid);
    assert_eq!(job.pid, job3.pid);
}

// ============================================================================
// Q7: METRICS TRACKING
// ============================================================================

#[test]
fn q7_initial_generation_zero() {
    let shell = TerminalShellCapsule::new();
    assert_eq!(shell.generation(), 0);
}

#[test]
fn q7_initial_bytes_zero() {
    let shell = TerminalShellCapsule::new();
    assert_eq!(shell.bytes_read(), 0);
    assert_eq!(shell.bytes_written(), 0);
}

#[test]
fn q7_initial_last_activity_zero() {
    let shell = TerminalShellCapsule::new();
    assert_eq!(shell.last_activity_ns(), 0);
}

// ============================================================================
// ERROR TYPE TESTS
// ============================================================================

#[test]
fn q7_shell_error_display() {
    let err = ShellError::NotRunning;
    assert_eq!(format!("{}", err), "Shell not running");

    let err = ShellError::AlreadyRunning;
    assert_eq!(format!("{}", err), "Shell already running");

    let err = ShellError::BufferFull;
    assert_eq!(format!("{}", err), "Buffer full");

    let err = ShellError::BufferEmpty;
    assert_eq!(format!("{}", err), "Buffer empty");
}

#[test]
fn q7_shell_error_debug() {
    let err = ShellError::NotRunning;
    assert!(format!("{:?}", err).contains("NotRunning"));
}
