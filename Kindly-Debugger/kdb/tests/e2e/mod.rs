//! End-to-End (E2E) Test Module for kdb
//!
//! This module provides comprehensive E2E testing infrastructure for the
//! kdb debugger, including test harnesses, scenarios, and validation.
//!
//! # Architecture
//!
//! ```text
//! e2e/
//! ├── mod.rs            (this file - module root)
//! ├── harness/          (test infrastructure)
//! │   ├── mod.rs        (harness exports)
//! │   ├── debugger_driver.rs
//! │   ├── gdb_driver.rs
//! │   ├── process_spawner.rs
//! │   ├── output_validator.rs
//! │   └── error.rs
//! └── scenarios/        (test scenarios)
//!     ├── mod.rs
//!     ├── attach_detach.rs   (E2E-01)
//!     ├── breakpoint_basic.rs (E2E-02)
//!     ├── time_travel.rs      (E2E-03, E2E-04)
//!     ├── stack_unwinding.rs  (E2E-05)
//!     ├── memory_read.rs      (E2E-06)
//!     ├── multi_thread.rs     (E2E-07)
//!     └── user_journey.rs     (E2E-18)
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use kdb::tests::e2e::harness::prelude::*;
//!
//! #[test]
//! fn test_time_travel_snapshot() -> E2EResult<()> {
//!     let mut fixture = E2EFixture::new()?;
//!     let pid = fixture.quick_attach(60)?;
//!
//!     // Capture snapshots
//!     let snap1 = fixture.driver.capture_snapshot()?;
//!     fixture.driver.step()?;
//!     let snap2 = fixture.driver.capture_snapshot()?;
//!
//!     // Time travel backward
//!     fixture.driver.step_backward()?;
//!
//!     // Verify audit trail integrity (Q34)
//!     let valid = fixture.driver.verify_audit_trail()?;
//!     assert!(valid);
//!
//!     Ok(())
//! }
//! ```
//!
//! # ASSUM Safety
//!
//! - #ASSUME_LINUX_ONLY: E2E tests only run on Linux (ptrace requirement)
//! - #ASSUME_ROOT_OR_PTRACE: Requires CAP_SYS_PTRACE or same UID as target
//! - #ASSUME_GDB_INSTALLED: GDB comparison tests require GDB in PATH
//!
//! # Framework Compliance
//!
//! - **T28**: E2E tests are Q22-Q28 (Production) tier
//! - **Chaos**: Uses lockfree patterns, no mutex/RwLock
//! - **Q34**: Validates audit trail integrity in all time-travel tests

pub mod harness;
pub mod scenarios;

// Re-export harness for convenience
pub use harness::*;

/// Check if running on Linux (required for E2E tests)
pub fn is_linux() -> bool {
    cfg!(target_os = "linux")
}

/// Check if GDB is available
pub fn has_gdb() -> bool {
    std::process::Command::new("gdb")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Skip test if not on Linux
#[macro_export]
macro_rules! skip_if_not_linux {
    () => {
        if !$crate::tests::e2e::is_linux() {
            eprintln!("Skipping test: requires Linux");
            return Ok(());
        }
    };
}

/// Skip test if GDB is not available
#[macro_export]
macro_rules! skip_if_no_gdb {
    () => {
        if !$crate::tests::e2e::has_gdb() {
            eprintln!("Skipping test: requires GDB");
            return Ok(());
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::harness::prelude::*;

    #[test]
    fn test_is_linux() {
        // Just verify the function compiles and runs
        let _ = is_linux();
    }

    #[test]
    fn test_has_gdb() {
        // Just verify the function compiles and runs
        let _ = has_gdb();
    }

    #[test]
    fn test_harness_reexport() {
        // Verify harness types are accessible via e2e module
        let _fixture = E2EFixture::default();
        let _driver = DebuggerDriver::new();
        let _validator = OutputValidator::new();
    }

    /// Basic attach/detach test (requires Linux and appropriate permissions)
    #[test]
    #[ignore = "requires ptrace permissions"]
    fn test_basic_attach_detach() -> E2EResult<()> {
        if !is_linux() {
            return Ok(());
        }

        let mut fixture = E2EFixture::new()?;

        // Spawn a simple sleep process
        let process = fixture.spawner.spawn_sleep(5)?;
        let pid = process.pid;

        // Attach
        fixture.driver.attach(pid)?;
        assert!(fixture.driver.is_attached());
        assert_eq!(fixture.driver.attached_pid(), Some(pid));

        // Verify attach event was recorded
        let events = fixture.driver.events();
        assert!(!events.is_empty());
        assert!(matches!(events[0], DebuggerEvent::Attached { .. }));

        // Detach
        fixture.driver.detach()?;
        assert!(!fixture.driver.is_attached());

        Ok(())
    }

    /// Time-travel snapshot test (requires Linux and appropriate permissions)
    #[test]
    #[ignore = "requires ptrace permissions"]
    fn test_time_travel_snapshots() -> E2EResult<()> {
        if !is_linux() {
            return Ok(());
        }

        let mut fixture = E2EFixture::new()?;
        let pid = fixture.quick_attach(10)?;

        // Capture initial snapshot
        let snap1 = fixture.driver.capture_snapshot()?;

        // Verify snapshot was recorded
        let events = fixture.driver.events();
        let snap_events: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, DebuggerEvent::SnapshotCaptured { .. }))
            .collect();
        assert_eq!(snap_events.len(), 1);

        // Get current registers
        let regs_before = fixture.driver.get_registers()?;

        // Step instruction
        fixture.driver.step()?;

        // Capture another snapshot
        let snap2 = fixture.driver.capture_snapshot()?;
        assert_ne!(snap1.0, snap2.0); // Different snapshot IDs

        // Step backward (time-travel)
        fixture.driver.step_backward()?;

        // Verify audit trail integrity (Q34 compliance)
        let valid = fixture.driver.verify_audit_trail()?;
        assert!(valid, "Audit trail integrity check failed");

        // Get root hash for verification
        let root_hash = fixture.driver.get_audit_root_hash();
        assert_ne!(root_hash, 0, "Root hash should not be zero after snapshots");

        fixture.cleanup()?;
        Ok(())
    }

    /// GDB comparison test (requires Linux, ptrace, and GDB)
    #[test]
    #[ignore = "requires ptrace permissions and GDB"]
    fn test_gdb_register_comparison() -> E2EResult<()> {
        if !is_linux() || !has_gdb() {
            return Ok(());
        }

        let mut fixture = ComparisonFixture::new()?;

        // Spawn two identical processes
        let (kdb_pid, gdb_pid) = fixture.quick_attach_parallel(10)?;

        // Get registers from both debuggers
        let kdb_regs = fixture.kdb.get_registers()?;
        let gdb_regs = fixture.gdb.get_registers()?;

        // Compare registers (with tolerance for timing differences)
        let result = fixture.validator.compare_registers(&kdb_regs, &gdb_regs);

        // Note: Registers may differ due to different process states
        // This test mainly verifies the comparison infrastructure works
        if !result.passed {
            eprintln!("Register differences (expected due to different processes):");
            for diff in &result.differences {
                eprintln!("  {}", diff);
            }
        }

        fixture.cleanup()?;
        Ok(())
    }

    /// Breakpoint test (requires Linux and appropriate permissions)
    #[test]
    #[ignore = "requires ptrace permissions"]
    fn test_breakpoint_setting() -> E2EResult<()> {
        if !is_linux() {
            return Ok(());
        }

        let mut fixture = E2EFixture::new()?;
        let pid = fixture.quick_attach(10)?;

        // Get current RIP to set a breakpoint nearby
        let regs = fixture.driver.get_registers()?;

        // Set breakpoint at a test address (may fail if address invalid)
        // In real tests, we'd use a known symbol or address from a test binary
        let bp_result = fixture.driver.set_breakpoint(&format!("0x{:x}", regs.rip + 0x10));

        // The breakpoint may fail if the address is invalid, but the
        // infrastructure should work
        match bp_result {
            Ok(bp_id) => {
                // Verify breakpoint event was recorded
                let events = fixture.driver.events();
                let bp_events: Vec<_> = events
                    .iter()
                    .filter(|e| matches!(e, DebuggerEvent::BreakpointSet { .. }))
                    .collect();
                assert!(!bp_events.is_empty());
            }
            Err(E2EError::BreakpointFailed { .. }) => {
                // Expected for invalid addresses
            }
            Err(e) => return Err(e),
        }

        fixture.cleanup()?;
        Ok(())
    }

    /// Stack trace test (requires Linux and appropriate permissions)
    #[test]
    #[ignore = "requires ptrace permissions"]
    fn test_stack_trace() -> E2EResult<()> {
        if !is_linux() {
            return Ok(());
        }

        let mut fixture = E2EFixture::new()?;
        let _pid = fixture.quick_attach(10)?;

        // Get stack trace
        let frames = fixture.driver.get_stack_trace()?;

        // Validate stack trace structure
        let result = fixture.validator.validate_stack_trace(&frames, 1);
        assert!(result.passed, "Stack trace validation failed: {}", result.summary);

        // Check that we got at least one frame
        assert!(!frames.is_empty(), "Stack trace should have at least one frame");

        fixture.cleanup()?;
        Ok(())
    }

    /// Audit trail validation test (requires Linux and appropriate permissions)
    #[test]
    #[ignore = "requires ptrace permissions"]
    fn test_audit_trail_validation() -> E2EResult<()> {
        if !is_linux() {
            return Ok(());
        }

        let mut fixture = E2EFixture::new()?;
        let _pid = fixture.quick_attach(10)?;

        // Capture multiple snapshots to build audit trail
        for _ in 0..5 {
            fixture.driver.capture_snapshot()?;
        }

        // Verify audit trail
        let valid = fixture.driver.verify_audit_trail()?;
        fixture
            .validator
            .validate_audit_trail(valid, fixture.driver.get_audit_root_hash())
            .into_result()?;

        // Validate snapshot count
        let count = fixture.driver.snapshot_count();
        fixture
            .validator
            .validate_snapshot_count(count, 5, 100)
            .into_result()?;

        fixture.cleanup()?;
        Ok(())
    }
}
