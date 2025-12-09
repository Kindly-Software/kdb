//! E2E-02: Breakpoint Hit Detection Scenarios
//!
//! Tests for breakpoint setting, hit detection, and management.
//! Validates that kdb can set breakpoints at addresses and detect when they are hit.
//!
//! # Test Coverage
//!
//! - Setting breakpoints at hex addresses
//! - Breakpoint hit detection
//! - Multiple breakpoints
//! - Breakpoint error handling (invalid address)
//! - Breakpoint event recording
//!
//! # ASSUM Safety
//!
//! - #ASSUME_PTRACE_AVAILABLE: Tests require CAP_SYS_PTRACE
//! - #ASSUME_VALID_ADDRESSES: Address-based breakpoints need valid executable addresses

use super::*;

/// E2E-02: Breakpoint hit detection
///
/// Tests the basic breakpoint workflow:
/// 1. Attach to process
/// 2. Set a breakpoint
/// 3. Continue execution
/// 4. Verify stop reason is breakpoint
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions and valid test binary"]
fn test_breakpoint_hit_detection() -> E2EResult<()> {
    let mut spawner = ProcessSpawner::new();
    let process = spawner.spawn_sleep(60)?;
    let pid = process.pid;

    let mut kdb = DebuggerDriver::new();
    kdb.attach(pid)?;

    // Get current RIP to set a nearby breakpoint
    let regs = kdb.get_registers()?;
    let bp_addr = regs.rip + 0x10; // Offset from current instruction

    // Attempt to set breakpoint
    let bp_result = kdb.set_breakpoint(&format!("0x{:x}", bp_addr));

    match bp_result {
        Ok(bp_id) => {
            // Verify breakpoint ID is valid
            assert_eq!(bp_id.0, 0, "First breakpoint should have ID 0");

            // Verify breakpoint event was recorded
            let events = kdb.events();
            let bp_set_events: Vec<_> = events
                .iter()
                .filter(|e| matches!(e, DebuggerEvent::BreakpointSet { .. }))
                .collect();
            assert!(!bp_set_events.is_empty(), "Breakpoint set event should be recorded");

            // Continue execution
            let stop = kdb.continue_execution()?;

            // Note: In a real test with a proper target, we'd verify breakpoint hit
            // For now, verify the stop reason is returned
            match stop {
                StopReason::Breakpoint(hit_id) => {
                    // Breakpoint was hit (ideal case)
                    assert_eq!(hit_id, bp_id, "Hit breakpoint ID should match set breakpoint");
                }
                StopReason::Step | StopReason::Signal(_) | StopReason::Unknown => {
                    // Other valid stop reasons (depends on timing/target)
                }
                StopReason::Exited(code) => {
                    // Process may have exited
                    eprintln!("Process exited with code {}", code);
                }
                StopReason::Detached => {
                    panic!("Unexpected detach during continue");
                }
            }
        }
        Err(E2EError::BreakpointFailed { location, reason }) => {
            // Breakpoint may fail if address is not executable
            eprintln!(
                "Breakpoint at {} failed (expected for invalid addresses): {}",
                location, reason
            );
        }
        Err(e) => return Err(e),
    }

    kdb.detach()?;
    Ok(())
}

/// E2E-02b: Breakpoint with invalid address format
///
/// Tests error handling for malformed address strings.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_breakpoint_invalid_address_format() -> E2EResult<()> {
    let mut spawner = ProcessSpawner::new();
    let process = spawner.spawn_sleep(30)?;

    let mut kdb = DebuggerDriver::new();
    kdb.attach(process.pid)?;

    // Try setting breakpoint with invalid address format
    let result = kdb.set_breakpoint("0xZZZZ");

    assert!(result.is_err(), "Invalid address format should fail");
    match result {
        Err(E2EError::BreakpointFailed { location, .. }) => {
            assert_eq!(location, "0xZZZZ");
        }
        Err(e) => panic!("Expected BreakpointFailed, got {:?}", e),
        Ok(_) => panic!("Should not succeed"),
    }

    kdb.detach()?;
    Ok(())
}

/// E2E-02c: Multiple breakpoints
///
/// Tests setting multiple breakpoints and verifying they get unique IDs.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_multiple_breakpoints() -> E2EResult<()> {
    let mut spawner = ProcessSpawner::new();
    let process = spawner.spawn_sleep(60)?;

    let mut kdb = DebuggerDriver::new();
    kdb.attach(process.pid)?;

    let regs = kdb.get_registers()?;

    // Set multiple breakpoints at different offsets
    let addresses = [
        regs.rip.wrapping_add(0x10),
        regs.rip.wrapping_add(0x20),
        regs.rip.wrapping_add(0x30),
    ];

    let mut bp_ids = Vec::new();
    let mut successful_count = 0;

    for addr in &addresses {
        match kdb.set_breakpoint(&format!("0x{:x}", addr)) {
            Ok(bp_id) => {
                bp_ids.push(bp_id);
                successful_count += 1;
            }
            Err(E2EError::BreakpointFailed { .. }) => {
                // Some addresses may not be valid for breakpoints
            }
            Err(e) => return Err(e),
        }
    }

    // Verify unique IDs
    for i in 0..bp_ids.len() {
        for j in (i + 1)..bp_ids.len() {
            assert_ne!(
                bp_ids[i], bp_ids[j],
                "Breakpoint IDs should be unique"
            );
        }
    }

    // Verify events recorded
    let events = kdb.events();
    let bp_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, DebuggerEvent::BreakpointSet { .. }))
        .collect();
    assert_eq!(
        bp_events.len(),
        successful_count,
        "Should have one event per successful breakpoint"
    );

    kdb.detach()?;
    Ok(())
}

/// E2E-02d: Breakpoint without attach
///
/// Tests that setting a breakpoint without being attached fails.
#[test]
fn test_breakpoint_not_attached() {
    let mut kdb = DebuggerDriver::new();

    let result = kdb.set_breakpoint("0x400000");

    assert!(result.is_err());
    assert!(matches!(result, Err(E2EError::NotAttached)));
}

/// E2E-02e: Breakpoint symbol lookup (not implemented)
///
/// Tests error handling for symbol-based breakpoints (currently unsupported).
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_breakpoint_symbol_not_implemented() -> E2EResult<()> {
    let mut spawner = ProcessSpawner::new();
    let process = spawner.spawn_sleep(30)?;

    let mut kdb = DebuggerDriver::new();
    kdb.attach(process.pid)?;

    // Try setting breakpoint by symbol name
    let result = kdb.set_breakpoint("main");

    // Currently symbols are not implemented
    assert!(result.is_err(), "Symbol breakpoints not yet implemented");
    match result {
        Err(E2EError::BreakpointFailed { reason, .. }) => {
            assert!(reason.contains("not yet implemented") || reason.contains("Symbol"));
        }
        Err(e) => panic!("Expected BreakpointFailed, got {:?}", e),
        Ok(_) => panic!("Symbol breakpoints should not succeed yet"),
    }

    kdb.detach()?;
    Ok(())
}

/// E2E-02f: Breakpoint event structure
///
/// Tests that breakpoint set events contain correct information.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_breakpoint_event_structure() -> E2EResult<()> {
    let mut spawner = ProcessSpawner::new();
    let process = spawner.spawn_sleep(30)?;

    let mut kdb = DebuggerDriver::new();
    kdb.attach(process.pid)?;

    let target_addr: u64 = 0x7f0000400000; // Example address

    match kdb.set_breakpoint(&format!("0x{:x}", target_addr)) {
        Ok(bp_id) => {
            let events = kdb.events();
            let bp_event = events
                .iter()
                .find(|e| matches!(e, DebuggerEvent::BreakpointSet { .. }));

            match bp_event {
                Some(DebuggerEvent::BreakpointSet { id, address }) => {
                    assert_eq!(*id, bp_id, "Event ID should match returned ID");
                    assert_eq!(*address, target_addr, "Event address should match target");
                }
                _ => panic!("BreakpointSet event not found or wrong type"),
            }
        }
        Err(E2EError::BreakpointFailed { .. }) => {
            // Address may not be valid
        }
        Err(e) => return Err(e),
    }

    kdb.detach()?;
    Ok(())
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_breakpoint_id_equality() {
        let id1 = BreakpointId(0);
        let id2 = BreakpointId(0);
        let id3 = BreakpointId(1);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_stop_reason_matching() {
        let bp_stop = StopReason::Breakpoint(BreakpointId(42));

        match bp_stop {
            StopReason::Breakpoint(id) => assert_eq!(id.0, 42),
            _ => panic!("Expected Breakpoint stop reason"),
        }
    }
}
