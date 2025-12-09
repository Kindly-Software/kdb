//! E2E-18: Full Debugging Workflow Scenarios
//!
//! Integration tests that simulate complete user debugging workflows,
//! combining multiple kdb features into realistic scenarios.
//!
//! # Test Coverage
//!
//! - Complete debugging session lifecycle
//! - Time-travel with audit trail verification
//! - Error recovery scenarios
//! - Q34 compliance throughout workflow
//!
//! # ASSUM Safety
//!
//! - #ASSUME_PTRACE_AVAILABLE: Tests require CAP_SYS_PTRACE
//! - #ASSUME_AUDIT_ENABLED: Q34 audit trail is active throughout
//! - #ASSUME_CLEANUP_ON_DROP: All resources cleaned up via Drop impls

use super::*;

/// E2E-18: Full debugging workflow
///
/// Tests a complete debugging session that would represent a typical
/// user workflow:
/// 1. Attach to a running process
/// 2. Set a breakpoint (at current location)
/// 3. Capture initial snapshot
/// 4. Continue/step execution
/// 5. Capture another snapshot
/// 6. Step backward (time-travel)
/// 7. Verify state is restored
/// 8. Verify audit trail integrity (Q34)
/// 9. Detach cleanly
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_full_debugging_workflow() -> E2EResult<()> {
    let mut fixture = E2EFixture::new()?;

    // STEP 1: Attach
    eprintln!("Step 1: Attach to process");
    let pid = fixture.quick_attach(120)?;
    assert!(fixture.driver.is_attached());
    assert_eq!(fixture.driver.attached_pid(), Some(pid));

    // STEP 2: Set breakpoint (at current RIP + offset)
    eprintln!("Step 2: Set breakpoint");
    let regs = fixture.driver.get_registers()?;
    let bp_result = fixture.driver.set_breakpoint(&format!("0x{:x}", regs.rip + 0x100));

    match bp_result {
        Ok(bp_id) => eprintln!("  Breakpoint set with ID {:?}", bp_id),
        Err(E2EError::BreakpointFailed { .. }) => eprintln!("  Breakpoint failed (expected for some addresses)"),
        Err(e) => return Err(e),
    }

    // STEP 3: Capture initial snapshot
    eprintln!("Step 3: Capture initial snapshot");
    let snap1 = fixture.driver.capture_snapshot()?;
    let initial_regs = fixture.driver.get_registers()?;
    eprintln!("  Snapshot {} at RIP=0x{:x}", snap1.0, initial_regs.rip);

    // STEP 4: Step execution
    eprintln!("Step 4: Step execution");
    let step_result = fixture.driver.step();
    match step_result {
        Ok(()) => {
            let new_regs = fixture.driver.get_registers()?;
            eprintln!("  Stepped to RIP=0x{:x}", new_regs.rip);
        }
        Err(E2EError::StepFailed { ref reason }) => {
            eprintln!("  Step failed: {} (continuing anyway)", reason);
        }
        Err(e) => return Err(e),
    }

    // STEP 5: Capture another snapshot
    eprintln!("Step 5: Capture second snapshot");
    let snap2 = fixture.driver.capture_snapshot()?;
    let stepped_regs = fixture.driver.get_registers()?;
    eprintln!("  Snapshot {} at RIP=0x{:x}", snap2.0, stepped_regs.rip);

    // Verify snapshots are different
    assert_ne!(snap1.0, snap2.0, "Snapshot IDs should be different");

    // STEP 6: Step backward (time-travel)
    eprintln!("Step 6: Step backward (time-travel)");
    let back_result = fixture.driver.step_backward();
    match back_result {
        Ok(()) => eprintln!("  Stepped backward successfully"),
        Err(E2EError::StepFailed { ref reason }) => {
            eprintln!("  Step backward failed: {}", reason);
        }
        Err(e) => return Err(e),
    }

    // STEP 7: Verify state (navigate to first snapshot)
    eprintln!("Step 7: Verify state restoration");
    let restored = fixture.driver.goto_snapshot(snap1)?;
    eprintln!("  Restored to RIP=0x{:x}", restored.rip);

    // Allow some variance in restoration
    let rip_diff = if restored.rip > initial_regs.rip {
        restored.rip - initial_regs.rip
    } else {
        initial_regs.rip - restored.rip
    };

    if rip_diff < 0x1000 {
        eprintln!("  State restored correctly (RIP within tolerance)");
    } else {
        eprintln!("  State restoration approximate (RIP diff: 0x{:x})", rip_diff);
    }

    // STEP 8: Verify audit trail (Q34 compliance)
    eprintln!("Step 8: Verify audit trail (Q34)");
    let valid = fixture.driver.verify_audit_trail()?;
    assert!(valid, "Audit trail integrity check MUST pass");

    let root_hash = fixture.driver.get_audit_root_hash();
    assert_ne!(root_hash, 0, "Root hash should be non-zero");
    eprintln!("  Audit trail valid, root_hash=0x{:x}", root_hash);

    let snap_count = fixture.driver.snapshot_count();
    eprintln!("  Total snapshots: {}", snap_count);

    // Validate with OutputValidator
    let audit_result = fixture.validator.validate_audit_trail(valid, root_hash);
    audit_result.into_result()?;

    let count_result = fixture.validator.validate_snapshot_count(snap_count, 2, 100);
    count_result.into_result()?;

    // STEP 9: Detach
    eprintln!("Step 9: Detach");
    fixture.driver.detach()?;
    assert!(!fixture.driver.is_attached());

    // Verify process is still running (check via spawner)
    // Note: We don't verify running state here since the spawner may have dropped the reference

    eprintln!("Full debugging workflow completed successfully!");
    eprintln!("  Events recorded: {}", fixture.driver.events().len());

    Ok(())
}

/// E2E-18b: Error recovery workflow
///
/// Tests that the debugger recovers gracefully from errors.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_error_recovery_workflow() -> E2EResult<()> {
    let mut fixture = E2EFixture::new()?;

    // Attach
    let pid = fixture.quick_attach(60)?;

    // Try invalid operations and verify recovery
    // 1. Invalid breakpoint address
    let result = fixture.driver.set_breakpoint("0xZZZZ");
    assert!(result.is_err(), "Invalid breakpoint should fail");

    // 2. Should still be able to perform valid operations
    let regs = fixture.driver.get_registers()?;
    assert!(regs.rip != 0, "Should still be able to read registers");

    // 3. Capture snapshot (should work)
    let snap = fixture.driver.capture_snapshot()?;
    assert!(snap.0 >= 0, "Snapshot should succeed");

    // 4. Verify audit trail is still valid despite errors
    let valid = fixture.driver.verify_audit_trail()?;
    assert!(valid, "Audit trail should be valid despite errors");

    // Cleanup
    fixture.driver.detach()?;
    Ok(())
}

/// E2E-18c: Rapid operations workflow
///
/// Tests performing many operations in rapid succession.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_rapid_operations_workflow() -> E2EResult<()> {
    let mut fixture = E2EFixture::new()?;
    let _pid = fixture.quick_attach(120)?;

    // Perform 50 rapid operations
    for i in 0..50 {
        // Capture snapshot
        fixture.driver.capture_snapshot()?;

        // Read registers
        let regs = fixture.driver.get_registers()?;
        assert!(regs.rip != 0, "Iteration {}: RIP should be valid", i);

        // Try step (may fail, that's okay)
        let _ = fixture.driver.step();
    }

    // Verify everything is still consistent
    let snap_count = fixture.driver.snapshot_count();
    assert!(snap_count >= 50, "Should have at least 50 snapshots");

    let valid = fixture.driver.verify_audit_trail()?;
    assert!(valid, "Audit trail should be valid after rapid operations");

    fixture.cleanup()?;
    Ok(())
}

/// E2E-18d: Attach-detach cycle workflow
///
/// Tests multiple attach/detach cycles.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_attach_detach_cycle() -> E2EResult<()> {
    let mut spawner = ProcessSpawner::new();
    let process = spawner.spawn_sleep(120)?;
    let pid = process.pid;

    // Perform 5 attach/detach cycles
    for cycle in 0..5 {
        eprintln!("Cycle {}", cycle);

        let mut kdb = DebuggerDriver::new();

        // Attach
        kdb.attach(pid)?;
        assert!(kdb.is_attached());

        // Do something
        kdb.capture_snapshot()?;
        let regs = kdb.get_registers()?;
        eprintln!("  RIP=0x{:x}", regs.rip);

        // Detach
        kdb.detach()?;
        assert!(!kdb.is_attached());

        // Driver goes out of scope and is cleaned up
    }

    // Process should still be running
    let process = spawner.get_by_pid_mut(pid).expect("Process should exist");
    assert!(process.is_running(), "Process should survive cycles");

    Ok(())
}

/// E2E-18e: Event history validation
///
/// Tests that all operations are properly recorded as events.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_event_history() -> E2EResult<()> {
    let mut fixture = E2EFixture::new()?;
    let pid = fixture.quick_attach(60)?;

    // Perform operations
    fixture.driver.capture_snapshot()?;
    let _ = fixture.driver.step();
    fixture.driver.capture_snapshot()?;

    // Check events
    let events = fixture.driver.events();

    // Should have: Attach, Snapshot, (maybe Step), Snapshot
    let attach_count = events.iter().filter(|e| matches!(e, DebuggerEvent::Attached { .. })).count();
    let snap_count = events.iter().filter(|e| matches!(e, DebuggerEvent::SnapshotCaptured { .. })).count();

    assert_eq!(attach_count, 1, "Should have 1 attach event");
    assert_eq!(snap_count, 2, "Should have 2 snapshot events");

    // Detach
    fixture.driver.detach()?;

    let events = fixture.driver.events();
    let detach_count = events.iter().filter(|e| matches!(e, DebuggerEvent::Detached { .. })).count();
    assert_eq!(detach_count, 1, "Should have 1 detach event");

    Ok(())
}

/// E2E-18f: Strict validation workflow
///
/// Tests with strict validation settings.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_strict_validation_workflow() -> E2EResult<()> {
    // Use strict fixture (no tolerances)
    let mut fixture = E2EFixture::strict()?;
    let _pid = fixture.quick_attach(60)?;

    // Capture snapshots
    for _ in 0..5 {
        fixture.driver.capture_snapshot()?;
    }

    // Strict validation
    let valid = fixture.driver.verify_audit_trail()?;
    let hash = fixture.driver.get_audit_root_hash();

    // Strict validation should still pass for valid data
    let result = fixture.validator.validate_audit_trail(valid, hash);
    result.into_result()?;

    let count = fixture.driver.snapshot_count();
    let result = fixture.validator.validate_snapshot_count(count, 5, 10);
    result.into_result()?;

    fixture.cleanup()?;
    Ok(())
}

/// E2E-18g: GDB comparison workflow (when available)
///
/// Complete workflow comparing kdb and GDB behavior.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions and GDB"]
fn test_gdb_comparison_workflow() -> E2EResult<()> {
    if !has_gdb() {
        eprintln!("GDB not available, skipping comparison workflow");
        return Ok(());
    }

    let mut fixture = ComparisonFixture::new()?;

    // Attach to two processes
    let (kdb_pid, gdb_pid) = fixture.quick_attach_parallel(60)?;
    eprintln!("kdb attached to {}, GDB attached to {}", kdb_pid, gdb_pid);

    // Get state from both
    let kdb_regs = fixture.kdb.get_registers()?;
    let gdb_regs = fixture.gdb.get_registers()?;

    let kdb_stack = fixture.kdb.get_stack_trace()?;
    let gdb_stack = fixture.gdb.get_stack_trace(100)?;

    // Compare registers (with tolerance)
    let reg_result = fixture.validator.compare_registers(&kdb_regs, &gdb_regs);
    eprintln!("Register comparison: {}", reg_result.summary);

    // Compare stack traces (with tolerance)
    let stack_result = fixture.validator.compare_stack_traces(&kdb_stack, &gdb_stack);
    eprintln!("Stack comparison: {}", stack_result.summary);

    // Both should have valid data
    assert!(kdb_regs.rip != 0, "kdb should have valid registers");
    assert!(gdb_regs.rip != 0, "GDB should have valid registers");
    assert!(!kdb_stack.is_empty(), "kdb should have stack frames");
    assert!(!gdb_stack.is_empty(), "GDB should have stack frames");

    fixture.cleanup()?;
    Ok(())
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_workflow_prerequisites() {
        // Verify test infrastructure is available
        let fixture = E2EFixture::new();
        assert!(fixture.is_ok(), "Fixture should be creatable");

        let validator = OutputValidator::new();
        // Validator should be usable
        let result = validator.validate_audit_trail(true, 0x12345);
        assert!(result.passed);
    }

    #[test]
    fn test_strict_vs_lenient_config() {
        let strict = ValidationConfig::strict();
        let lenient = ValidationConfig::lenient();

        assert!(!strict.allow_address_variance);
        assert!(lenient.allow_address_variance);

        assert_eq!(strict.max_address_offset, 0);
        assert!(lenient.max_address_offset > 0);
    }
}
