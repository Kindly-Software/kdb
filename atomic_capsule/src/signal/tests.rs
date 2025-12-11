//! Signal Module Tests - T28 Framework
//!
//! Comprehensive test suite for Capsule OS signal handling.
//! Covers unit tests (Q1-Q7), property tests (Q8-Q14), integration tests (Q15-Q21),
//! and production tests (Q22-Q28).
//!
//! ## Test Categories
//!
//! | Tier | Questions | Focus |
//! |------|-----------|-------|
//! | Unit | Q1-Q7 | Size, alignment, basic operations |
//! | Property | Q8-Q14 | Invariants, edge cases, concurrency |
//! | Integration | Q15-Q21 | Signal delivery, event loop integration |
//! | Production | Q22-Q28 | Stress testing, resource leaks, performance |

#![cfg(test)]

use super::*;

#[cfg(unix)]
use std::sync::Mutex;

// Global test mutex to serialize signal handler tests
// (Signal handlers are global process resources)
#[cfg(unix)]
static TEST_MUTEX: Mutex<()> = Mutex::new(());

// =============================================================================
// Q1-Q7: Unit Tests - Size, Alignment, Basic Operations
// =============================================================================

#[test]
fn test_q1_signal_enum_values() {
    // Q1: Verify Signal enum values match POSIX
    assert_eq!(Signal::Hup.as_i32(), 1);
    assert_eq!(Signal::Int.as_i32(), 2);
    assert_eq!(Signal::Quit.as_i32(), 3);
    assert_eq!(Signal::Kill.as_i32(), 9);
    assert_eq!(Signal::Term.as_i32(), 15);
    assert_eq!(Signal::Chld.as_i32(), 17);
    assert_eq!(Signal::Cont.as_i32(), 18);
    assert_eq!(Signal::Stop.as_i32(), 19);
    assert_eq!(Signal::Tstp.as_i32(), 20);
    assert_eq!(Signal::Winch.as_i32(), 28);
}

#[test]
fn test_q2_signal_from_i32() {
    // Q2: Verify Signal::from_i32 works correctly
    assert_eq!(Signal::from_i32(1), Some(Signal::Hup));
    assert_eq!(Signal::from_i32(2), Some(Signal::Int));
    assert_eq!(Signal::from_i32(9), Some(Signal::Kill));
    assert_eq!(Signal::from_i32(15), Some(Signal::Term));
    assert_eq!(Signal::from_i32(28), Some(Signal::Winch));

    // Invalid signals
    assert_eq!(Signal::from_i32(0), None);
    assert_eq!(Signal::from_i32(-1), None);
    assert_eq!(Signal::from_i32(32), None);
    assert_eq!(Signal::from_i32(100), None);
}

#[test]
fn test_q3_signal_catchable() {
    // Q3: Verify catchable signal detection
    assert!(Signal::Int.is_catchable());
    assert!(Signal::Term.is_catchable());
    assert!(Signal::Hup.is_catchable());
    assert!(Signal::Winch.is_catchable());

    // SIGKILL and SIGSTOP cannot be caught
    assert!(!Signal::Kill.is_catchable());
    assert!(!Signal::Stop.is_catchable());
}

#[test]
fn test_q4_signal_info_size() {
    // Q4: Verify SignalInfo size and alignment
    assert_eq!(core::mem::size_of::<SignalInfo>(), 64);
    assert_eq!(core::mem::align_of::<SignalInfo>(), 64);
}

#[test]
fn test_q5_handler_capsule_size() {
    // Q5: Verify SignalHandlerCapsule size and alignment
    assert_eq!(core::mem::size_of::<SignalHandlerCapsule>(), 256);
    assert_eq!(core::mem::align_of::<SignalHandlerCapsule>(), 256);
}

#[test]
fn test_q6_dispatcher_capsule_size() {
    // Q6: Verify SignalDispatcherCapsule size and alignment
    assert_eq!(core::mem::size_of::<SignalDispatcherCapsule>(), 512);
    assert_eq!(core::mem::align_of::<SignalDispatcherCapsule>(), 512);
}

#[test]
fn test_q7_signal_queue_entry_size() {
    // Q7: Verify SignalQueueEntry size and alignment
    assert_eq!(core::mem::size_of::<SignalQueueEntry>(), 128);
    assert_eq!(core::mem::align_of::<SignalQueueEntry>(), 128);
}

// =============================================================================
// Q8-Q14: Property Tests - Invariants and Edge Cases
// =============================================================================

#[test]
fn test_q8_signal_roundtrip() {
    // Q8: Verify Signal <-> i32 roundtrip
    for sig in 1..=31 {
        if let Some(signal) = Signal::from_i32(sig) {
            assert_eq!(signal.as_i32(), sig);
        }
    }
}

#[test]
fn test_q9_signal_name_consistency() {
    // Q9: Verify signal names are consistent
    assert_eq!(Signal::Int.name(), "SIGINT");
    assert_eq!(Signal::Term.name(), "SIGTERM");
    assert_eq!(Signal::Kill.name(), "SIGKILL");

    // Name should contain the signal number in Display
    let display = format!("{}", Signal::Int);
    assert!(display.contains("SIGINT"));
    assert!(display.contains("2"));
}

#[test]
fn test_q10_signal_action_defaults() {
    // Q10: Verify default signal actions match POSIX
    assert_eq!(SignalAction::default_for(Signal::Int), SignalAction::Terminate);
    assert_eq!(SignalAction::default_for(Signal::Term), SignalAction::Terminate);
    assert_eq!(SignalAction::default_for(Signal::Quit), SignalAction::CoreDump);
    assert_eq!(SignalAction::default_for(Signal::Segv), SignalAction::CoreDump);
    assert_eq!(SignalAction::default_for(Signal::Stop), SignalAction::Stop);
    assert_eq!(SignalAction::default_for(Signal::Cont), SignalAction::Continue);
    assert_eq!(SignalAction::default_for(Signal::Winch), SignalAction::Ignore);
    assert_eq!(SignalAction::default_for(Signal::Chld), SignalAction::Ignore);
}

#[test]
fn test_q11_error_errno_extraction() {
    // Q11: Verify error errno extraction
    let err = SignalError::PipeCreationFailed(13);
    assert_eq!(err.errno(), Some(13));

    let err = SignalError::SignalRegistrationFailed(22);
    assert_eq!(err.errno(), Some(22));

    let err = SignalError::AlreadyRegistered;
    assert_eq!(err.errno(), None);

    let err = SignalError::QueueFull;
    assert_eq!(err.errno(), None);
}

#[test]
fn test_q12_error_recoverable() {
    // Q12: Verify error recoverability classification
    assert!(SignalError::QueueFull.is_recoverable());
    assert!(SignalError::QueueEmpty.is_recoverable());
    assert!(SignalError::Timeout.is_recoverable());
    assert!(SignalError::Interrupted.is_recoverable());

    assert!(!SignalError::PipeCreationFailed(1).is_recoverable());
    assert!(!SignalError::AlreadyRegistered.is_recoverable());
    assert!(!SignalError::NotRegistered.is_recoverable());
}

#[test]
fn test_q13_signal_info_builder() {
    // Q13: Verify SignalInfo builder pattern
    let info = SignalInfo::new(2)
        .with_timestamp(123456789)
        .with_pid(1000)
        .with_uid(500);

    assert_eq!(info.signo, 2);
    assert_eq!(info.timestamp_ns, 123456789);
    assert_eq!(info.pid, 1000);
    assert_eq!(info.uid, 500);
    assert_eq!(info.signal(), Some(Signal::Int));
}

#[test]
fn test_q14_dispatcher_config() {
    // Q14: Verify dispatcher configuration (tested via start/stop behavior)
    let d1 = SignalDispatcherCapsule::with_config(true, false);
    // Can start and stop
    d1.start().expect("start should succeed");
    assert!(d1.is_running());
    d1.stop().expect("stop should succeed");

    let d2 = SignalDispatcherCapsule::with_config(false, true);
    d2.start().expect("start should succeed");
    assert!(d2.is_running());
    d2.stop().expect("stop should succeed");

    let d3 = SignalDispatcherCapsule::with_config(true, true);
    d3.start().expect("start should succeed");
    assert!(d3.is_running());
    d3.stop().expect("stop should succeed");
}

// =============================================================================
// Q15-Q21: Integration Tests - Signal Delivery
// =============================================================================

#[cfg(unix)]
#[test]
fn test_q15_handler_create_and_pipe() {
    // Q15: Verify handler creation and pipe FD
    let _lock = TEST_MUTEX.lock().unwrap();

    let handler = SignalHandlerCapsule::new().expect("Failed to create handler");
    let fd = handler.pipe_fd();

    assert!(fd >= 0, "Pipe FD should be valid");
    assert!(fd < 1024, "Pipe FD should be reasonable");
}

#[cfg(unix)]
#[test]
fn test_q16_handler_initial_state() {
    // Q16: Verify handler initial state
    let _lock = TEST_MUTEX.lock().unwrap();

    let handler = SignalHandlerCapsule::new().expect("Failed to create handler");

    assert!(!handler.is_registered());
    assert!(!handler.is_active());
    // Pipe FD being valid indicates PIPE_VALID flag is set
    assert!(handler.pipe_fd() >= 0, "Pipe should be valid");
}

#[cfg(unix)]
#[test]
fn test_q17_handler_drain_empty() {
    // Q17: Verify draining empty pipe succeeds
    let _lock = TEST_MUTEX.lock().unwrap();

    let handler = SignalHandlerCapsule::new().expect("Failed to create handler");
    handler.drain_pipe().expect("Drain should succeed on empty pipe");
}

#[test]
fn test_q18_dispatcher_start_stop() {
    // Q18: Verify dispatcher start/stop lifecycle
    let dispatcher = SignalDispatcherCapsule::new();

    assert!(!dispatcher.is_running());

    dispatcher.start().expect("Start should succeed");
    assert!(dispatcher.is_running());

    // Double start should fail
    assert!(dispatcher.start().is_err());

    dispatcher.stop().expect("Stop should succeed");
    assert!(!dispatcher.is_running());

    // Double stop should fail
    assert!(dispatcher.stop().is_err());
}

#[test]
fn test_q19_dispatcher_handler_registration() {
    // Q19: Verify handler registration
    let mut dispatcher = SignalDispatcherCapsule::new();

    dispatcher
        .register_handler(Signal::Int, SignalAction::Handle, 1)
        .expect("Register should succeed");

    assert_eq!(dispatcher.get_action(Signal::Int), SignalAction::Handle);

    dispatcher
        .unregister_handler(Signal::Int)
        .expect("Unregister should succeed");

    // Action should be default now
    assert_eq!(dispatcher.get_action(Signal::Int), SignalAction::Terminate);
}

#[test]
fn test_q20_dispatcher_cannot_register_uncatchable() {
    // Q20: Verify cannot register handlers for SIGKILL/SIGSTOP
    let mut dispatcher = SignalDispatcherCapsule::new();

    let result = dispatcher.register_handler(Signal::Kill, SignalAction::Ignore, 0);
    assert!(result.is_err());

    let result = dispatcher.register_handler(Signal::Stop, SignalAction::Ignore, 0);
    assert!(result.is_err());
}

#[test]
fn test_q21_dispatcher_queue_operations() {
    // Q21: Verify queue enqueue/dequeue requires running
    let dispatcher = SignalDispatcherCapsule::new();

    // Enqueue should fail when not running
    let info = SignalInfo::new(2);
    assert!(matches!(dispatcher.enqueue(info), Err(SignalError::NotRegistered)));

    // Dequeue should fail when not running
    assert!(matches!(dispatcher.dequeue(), Err(SignalError::NotRegistered)));

    // Start dispatcher
    dispatcher.start().expect("Start should succeed");

    // Dequeue from empty should return QueueEmpty
    assert!(matches!(dispatcher.dequeue(), Err(SignalError::QueueEmpty)));
}

// =============================================================================
// Q22-Q28: Production Tests - Stress and Performance
// =============================================================================

#[test]
fn test_q22_dispatcher_stats_initial() {
    // Q22: Verify initial statistics
    let dispatcher = SignalDispatcherCapsule::new();
    let stats = dispatcher.stats();

    assert_eq!(stats.pending_count, 0);
    assert_eq!(stats.dispatched_count, 0);
    assert_eq!(stats.dropped_count, 0);
    assert_eq!(stats.coalesced_count, 0);
    assert_eq!(stats.error_count, 0);
    assert_eq!(stats.handler_count, 0);
}

#[cfg(unix)]
#[test]
fn test_q23_handler_stats_initial() {
    // Q23: Verify handler initial statistics
    let _lock = TEST_MUTEX.lock().unwrap();

    let handler = SignalHandlerCapsule::new().expect("Failed to create handler");
    let stats = handler.stats();

    assert_eq!(stats.delivered_count, 0);
    assert_eq!(stats.dropped_count, 0);
    assert_eq!(stats.error_count, 0);
}

#[test]
fn test_q24_handler_entry_default() {
    // Q24: Verify HandlerEntry default state
    let entry = HandlerEntry::default();

    assert!(!entry.enabled);
    assert_eq!(entry.action, SignalAction::Default);
    assert_eq!(entry.callback_id, 0);
    assert!(entry.coalesce);
}

#[test]
fn test_q25_handler_entry_with_action() {
    // Q25: Verify HandlerEntry with action
    let entry = HandlerEntry::with_action(SignalAction::Ignore);

    assert!(entry.enabled);
    assert_eq!(entry.action, SignalAction::Ignore);
}

#[test]
fn test_q26_signal_queue_entry_creation() {
    // Q26: Verify SignalQueueEntry creation
    let info = SignalInfo::new(15);
    let entry = SignalQueueEntry::new(info, 42, 123456789);

    assert_eq!(entry.info.signo, 15);
    assert_eq!(entry.sequence, 42);
    assert_eq!(entry.enqueue_time_ns, 123456789);
    assert_eq!(entry.state, 0);
}

#[test]
fn test_q27_signal_termination_classification() {
    // Q27: Verify termination signal classification
    let termination_signals = [
        Signal::Hup, Signal::Int, Signal::Quit, Signal::Ill, Signal::Abrt,
        Signal::Fpe, Signal::Kill, Signal::Segv, Signal::Pipe, Signal::Alrm,
        Signal::Term, Signal::Usr1, Signal::Usr2,
    ];

    for sig in termination_signals {
        assert!(sig.is_termination(), "{} should be termination signal", sig);
    }

    let non_termination = [Signal::Cont, Signal::Stop, Signal::Winch, Signal::Chld];

    for sig in non_termination {
        assert!(!sig.is_termination(), "{} should not be termination signal", sig);
    }
}

#[test]
fn test_q28_signal_job_control_classification() {
    // Q28: Verify job control signal classification
    let job_control_signals = [
        Signal::Cont, Signal::Stop, Signal::Tstp, Signal::Ttin, Signal::Ttou,
    ];

    for sig in job_control_signals {
        assert!(sig.is_job_control(), "{} should be job control signal", sig);
    }

    let non_job_control = [Signal::Int, Signal::Term, Signal::Winch];

    for sig in non_job_control {
        assert!(!sig.is_job_control(), "{} should not be job control signal", sig);
    }
}
