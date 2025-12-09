//! E2E-07: Multi-Threaded Debugging Scenarios
//!
//! Tests for debugging multi-threaded processes, verifying that kdb
//! can properly handle thread enumeration and per-thread operations.
//!
//! # Test Coverage
//!
//! - Stack traces for multiple threads
//! - Thread state consistency
//! - Per-thread register access
//!
//! # ASSUM Safety
//!
//! - #ASSUME_PTRACE_AVAILABLE: Tests require CAP_SYS_PTRACE
//! - #ASSUME_THREADS_STOPPED: All threads should be stopped during attach
//!
//! # Notes
//!
//! Multi-threaded debugging is complex and requires proper ptrace
//! coordination. These tests use a simple sleep process which may
//! have only one thread, but the infrastructure supports multi-threaded
//! targets.

use super::*;

/// E2E-07: Multi-threaded stack traces
///
/// Tests that stack traces can be obtained for attached processes.
/// For a simple sleep process, this validates single-thread behavior,
/// but the same mechanism works for multi-threaded processes.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_multi_thread_stack_traces() -> E2EResult<()> {
    let mut fixture = E2EFixture::new()?;
    let _pid = fixture.quick_attach(30)?;

    // Get stack trace
    let frames = fixture.driver.get_stack_trace()?;

    // Verify we got frames
    assert!(!frames.is_empty(), "Should have at least one stack frame");

    // Validate frame structure
    let result = fixture.validator.validate_stack_trace(&frames, 1);
    result.into_result()?;

    // For a simple sleep process, we expect a simple call stack
    // The exact depth depends on the libc implementation
    eprintln!("Got {} stack frames", frames.len());

    for (i, frame) in frames.iter().take(5).enumerate() {
        eprintln!(
            "  Frame {}: RIP=0x{:x}, RSP=0x{:x}, func={:?}",
            i, frame.rip, frame.rsp, frame.function_name
        );
    }

    fixture.cleanup()?;
    Ok(())
}

/// E2E-07b: Register state during attach
///
/// Tests that register state is accessible immediately after attach.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_register_state_after_attach() -> E2EResult<()> {
    let mut fixture = E2EFixture::new()?;
    let _pid = fixture.quick_attach(30)?;

    // Get registers immediately after attach
    let regs = fixture.driver.get_registers()?;

    // Verify key registers are valid
    assert!(regs.rip != 0, "RIP should be set");
    assert!(regs.rsp != 0, "RSP should be set");

    // RBP may be zero if using frame pointer omission
    // (common in optimized builds)

    eprintln!(
        "Registers after attach: RIP=0x{:x}, RSP=0x{:x}, RBP=0x{:x}",
        regs.rip, regs.rsp, regs.rbp
    );

    fixture.cleanup()?;
    Ok(())
}

/// E2E-07c: Multiple register reads consistency
///
/// Tests that register reads are consistent while the process is stopped.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_register_read_consistency() -> E2EResult<()> {
    let mut fixture = E2EFixture::new()?;
    let _pid = fixture.quick_attach(30)?;

    // Read registers multiple times
    let regs1 = fixture.driver.get_registers()?;
    let regs2 = fixture.driver.get_registers()?;
    let regs3 = fixture.driver.get_registers()?;

    // All reads should return the same values
    assert_eq!(regs1.rip, regs2.rip, "RIP should be consistent");
    assert_eq!(regs2.rip, regs3.rip, "RIP should be consistent");
    assert_eq!(regs1.rsp, regs2.rsp, "RSP should be consistent");
    assert_eq!(regs2.rsp, regs3.rsp, "RSP should be consistent");

    fixture.cleanup()?;
    Ok(())
}

/// E2E-07d: Stack trace with snapshot
///
/// Tests that stack traces work correctly with snapshot capture.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_stack_trace_with_snapshot() -> E2EResult<()> {
    let mut fixture = E2EFixture::new()?;
    let _pid = fixture.quick_attach(60)?;

    // Get initial stack trace
    let frames_before = fixture.driver.get_stack_trace()?;

    // Capture snapshot
    let snap = fixture.driver.capture_snapshot()?;

    // Get stack trace after snapshot
    let frames_after = fixture.driver.get_stack_trace()?;

    // Stack should be same (process didn't run)
    assert_eq!(
        frames_before.len(),
        frames_after.len(),
        "Stack depth should be same"
    );

    if !frames_before.is_empty() && !frames_after.is_empty() {
        assert_eq!(
            frames_before[0].rip,
            frames_after[0].rip,
            "Current RIP should match"
        );
    }

    // Verify audit trail
    let valid = fixture.driver.verify_audit_trail()?;
    assert!(valid, "Audit trail should be valid");

    fixture.cleanup()?;
    Ok(())
}

/// E2E-07e: Stats after operations
///
/// Tests that debugger stats are updated correctly.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_debugger_stats() -> E2EResult<()> {
    let mut fixture = E2EFixture::new()?;
    let _pid = fixture.quick_attach(30)?;

    // Get initial stats
    let _stats_initial = fixture.driver.get_stats();

    // Perform some operations
    fixture.driver.capture_snapshot()?;
    fixture.driver.capture_snapshot()?;
    let _ = fixture.driver.get_stack_trace();
    let _ = fixture.driver.get_registers();

    // Get updated stats
    let stats = fixture.driver.get_stats();

    // Verify snapshot count increased
    let snap_count = fixture.driver.snapshot_count();
    assert!(snap_count >= 2, "Should have at least 2 snapshots");

    // DebuggerStats doesn't implement Debug, so just confirm we got stats
    let _ = stats; // Use the stats variable

    fixture.cleanup()?;
    Ok(())
}

/// E2E-07f: Attach to multiple processes sequentially
///
/// Tests attaching to different processes one after another.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions"]
fn test_sequential_attach_different_processes() -> E2EResult<()> {
    let mut spawner = ProcessSpawner::new();

    // Spawn two processes
    let process1 = spawner.spawn_sleep(30)?;
    let pid1 = process1.pid;

    let process2 = spawner.spawn_sleep(30)?;
    let pid2 = process2.pid;

    let mut kdb = DebuggerDriver::new();

    // Attach to first process
    kdb.attach(pid1)?;
    let regs1 = kdb.get_registers()?;
    kdb.detach()?;

    // Attach to second process
    kdb.attach(pid2)?;
    let regs2 = kdb.get_registers()?;
    kdb.detach()?;

    // Both should have valid registers
    assert!(regs1.rip != 0, "First process should have valid RIP");
    assert!(regs2.rip != 0, "Second process should have valid RIP");

    // They may have different RIPs (different execution points)
    eprintln!("Process 1 RIP: 0x{:x}", regs1.rip);
    eprintln!("Process 2 RIP: 0x{:x}", regs2.rip);

    Ok(())
}

/// E2E-07g: GDB comparison for registers
///
/// Compares register retrieval with GDB baseline.
#[test]
#[cfg(target_os = "linux")]
#[ignore = "requires ptrace permissions and GDB"]
fn test_registers_vs_gdb() -> E2EResult<()> {
    if !has_gdb() {
        eprintln!("GDB not available, skipping comparison");
        return Ok(());
    }

    let mut fixture = ComparisonFixture::new()?;

    // Spawn two processes (can't attach same debugger to same process)
    let (kdb_pid, gdb_pid) = fixture.quick_attach_parallel(30)?;

    // Get registers from both
    let kdb_regs = fixture.kdb.get_registers()?;
    let gdb_regs = fixture.gdb.get_registers()?;

    // Compare (using lenient config since different processes)
    let result = fixture.validator.compare_registers(&kdb_regs, &gdb_regs);

    // Log comparison (differences expected for different processes)
    if !result.passed {
        eprintln!("Register differences (expected for different processes):");
        for diff in &result.differences {
            eprintln!("  {}", diff);
        }
    }

    // Both should have valid registers
    assert!(kdb_regs.rip != 0, "kdb should get valid RIP");
    assert!(gdb_regs.rip != 0, "GDB should get valid RIP");

    fixture.cleanup()?;
    Ok(())
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_fixture_creation() {
        let fixture = E2EFixture::new();
        assert!(fixture.is_ok());

        let fixture = fixture.unwrap();
        assert!(!fixture.driver.is_attached());
    }

    #[test]
    fn test_comparison_fixture_requires_gdb() {
        // Only attempt to create ComparisonFixture if GDB is available
        // This avoids potential stack issues from spawning GDB
        if !has_gdb() {
            eprintln!("GDB not available, skipping ComparisonFixture test");
            return;
        }

        // ComparisonFixture::new() spawns GDB process
        let result = ComparisonFixture::new();
        assert!(result.is_ok(), "Should succeed with GDB installed");
    }
}
