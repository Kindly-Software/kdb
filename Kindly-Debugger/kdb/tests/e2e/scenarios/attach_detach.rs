//! E2E-01: Basic Attach and Detach Scenarios
//!
//! Tests for process attachment and detachment operations, verifying that
//! kdb can properly attach to running processes and cleanly detach.
//!
//! # Test Coverage
//!
//! - Basic attach/detach lifecycle
//! - Attach to invalid PID (error handling)
//! - Multiple attach attempts (error handling)
//! - Process continues running after detach
//! - Event recording during attach/detach
//!
//! # ASSUM Safety
//!
//! - #ASSUME_PTRACE_AVAILABLE: Tests require CAP_SYS_PTRACE or same UID as target
//! - #ASSUME_SLEEP_EXISTS: Uses /usr/bin/sleep as test target

use super::*;

/// E2E-01: Basic attach and detach workflow
///
/// Tests the fundamental attach/detach lifecycle:
/// 1. Spawn a test process
/// 2. Attach to it
/// 3. Verify attachment state
/// 4. Detach from it
/// 5. Verify process continues running
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_attach_detach_basic() -> E2EResult<()> {
    let mut spawner = ProcessSpawner::new();
    let process = spawner.spawn_sleep(30)?;
    let pid = process.pid;

    let mut kdb = DebuggerDriver::new();

    // Attach
    assert!(kdb.attach(pid).is_ok(), "Failed to attach to process {}", pid);
    assert!(kdb.is_attached(), "Driver should report as attached");
    assert_eq!(kdb.attached_pid(), Some(pid), "Attached PID should match");

    // Verify attach event recorded
    let events = kdb.events();
    assert!(
        events.iter().any(|e| matches!(e, DebuggerEvent::Attached { pid: p } if *p == pid)),
        "Attach event should be recorded"
    );

    // Detach
    assert!(kdb.detach().is_ok(), "Failed to detach from process");
    assert!(!kdb.is_attached(), "Driver should report as not attached");

    // Verify detach event recorded
    let events = kdb.events();
    assert!(
        events.iter().any(|e| matches!(e, DebuggerEvent::Detached { pid: p } if *p == pid)),
        "Detach event should be recorded"
    );

    // Verify process is still running after detach
    let process = spawner.get_by_pid_mut(pid).expect("Process should still exist");
    assert!(process.is_running(), "Process should continue running after detach");

    Ok(())
}

/// E2E-01b: Attach to invalid PID
///
/// Tests error handling when attempting to attach to a non-existent process.
/// The debugger should return an appropriate error.
///
/// NOTE: Currently the DebuggerCapsule::attach_to_process() is a stub that always
/// succeeds. This test will pass once real ptrace implementation is connected.
#[test]
#[ignore = "requires real ptrace implementation (current stub always succeeds)"]
fn test_attach_invalid_pid() {
    let mut kdb = DebuggerDriver::new();

    // Use an extremely high PID that is unlikely to exist
    let result = kdb.attach(99999999);

    assert!(result.is_err(), "Attach to invalid PID should fail");

    // Verify we're still not attached
    assert!(!kdb.is_attached(), "Driver should not be attached after failed attach");
    assert_eq!(kdb.attached_pid(), None, "Attached PID should be None");
}

/// E2E-01c: Double attach attempt
///
/// Tests that attempting to attach while already attached returns an error.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_attach_while_attached() -> E2EResult<()> {
    let mut spawner = ProcessSpawner::new();
    let pid1 = {
        let process1 = spawner.spawn_sleep(30)?;
        process1.pid
    };
    let pid2 = {
        let process2 = spawner.spawn_sleep(30)?;
        process2.pid
    };

    let mut kdb = DebuggerDriver::new();

    // First attach should succeed
    kdb.attach(pid1)?;
    assert!(kdb.is_attached());

    // Second attach should fail
    let result = kdb.attach(pid2);
    assert!(result.is_err(), "Second attach should fail while already attached");

    // Should still be attached to first process
    assert_eq!(kdb.attached_pid(), Some(pid1));

    kdb.detach()?;
    Ok(())
}

/// E2E-01d: Detach when not attached
///
/// Tests error handling when attempting to detach without being attached.
#[test]
fn test_detach_not_attached() {
    let mut kdb = DebuggerDriver::new();

    // Detach without attaching should fail
    let result = kdb.detach();

    assert!(result.is_err(), "Detach without attach should fail");
    assert!(
        matches!(result, Err(E2EError::NotAttached)),
        "Should return NotAttached error"
    );
}

/// E2E-01e: Reattach after detach
///
/// Tests that a driver can attach to a new process after detaching.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_reattach_after_detach() -> E2EResult<()> {
    let mut spawner = ProcessSpawner::new();
    let pid1 = {
        let process1 = spawner.spawn_sleep(30)?;
        process1.pid
    };
    let pid2 = {
        let process2 = spawner.spawn_sleep(30)?;
        process2.pid
    };

    let mut kdb = DebuggerDriver::new();

    // Attach to first process
    kdb.attach(pid1)?;
    assert_eq!(kdb.attached_pid(), Some(pid1));

    // Detach
    kdb.detach()?;
    assert!(!kdb.is_attached());

    // Clear events for clean slate
    kdb.clear_events();

    // Attach to second process
    kdb.attach(pid2)?;
    assert_eq!(kdb.attached_pid(), Some(pid2));

    // Verify new attach event
    let events = kdb.events();
    assert!(
        events.iter().any(|e| matches!(e, DebuggerEvent::Attached { pid } if *pid == pid2)),
        "New attach event should be recorded"
    );

    kdb.detach()?;
    Ok(())
}

/// E2E-01f: Attach event contains correct PID
///
/// Tests that the attach event recorded contains the correct PID.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_attach_event_correctness() -> E2EResult<()> {
    let mut spawner = ProcessSpawner::new();
    let process = spawner.spawn_sleep(30)?;
    let expected_pid = process.pid;

    let mut kdb = DebuggerDriver::new();
    kdb.attach(expected_pid)?;

    let events = kdb.events();
    assert_eq!(events.len(), 1, "Should have exactly one event");

    match &events[0] {
        DebuggerEvent::Attached { pid } => {
            assert_eq!(*pid, expected_pid, "Event PID should match attached PID");
        }
        other => panic!("Expected Attached event, got {:?}", other),
    }

    kdb.detach()?;
    Ok(())
}

/// E2E-01g: Fixture-based attach test
///
/// Tests attach/detach using the E2EFixture convenience wrapper.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_fixture_quick_attach() -> E2EResult<()> {
    let mut fixture = E2EFixture::new()?;

    // Quick attach spawns a process and attaches in one step
    let pid = fixture.quick_attach(30)?;

    assert!(pid > 0, "PID should be valid");
    assert!(fixture.driver.is_attached(), "Should be attached via fixture");

    // Fixture cleanup handles detach automatically
    fixture.cleanup()?;

    assert!(!fixture.driver.is_attached(), "Should not be attached after cleanup");

    Ok(())
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_driver_initial_state() {
        let kdb = DebuggerDriver::new();
        assert!(!kdb.is_attached());
        assert_eq!(kdb.attached_pid(), None);
        assert!(kdb.events().is_empty());
    }
}
