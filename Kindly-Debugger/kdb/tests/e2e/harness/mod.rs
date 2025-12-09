//! E2E Test Harness Module
//!
//! Provides the complete infrastructure for end-to-end testing of kdb,
//! including process management, debugger control, and output validation.
//!
//! # Architecture
//!
//! The harness consists of four main components:
//!
//! - **ProcessSpawner**: Spawn and manage test target processes
//! - **DebuggerDriver**: Drive kdb programmatically (not via CLI)
//! - **GdbDriver**: Drive GDB via MI for correctness comparison
//! - **OutputValidator**: Validate kdb output against expected values
//!
//! # ASSUM Safety
//!
//! - #ASSUME_TEST_ISOLATION: Each test uses fresh driver/spawner instances
//! - #ASSUME_CLEANUP_ON_DROP: All resources cleaned up via Drop impls
//! - #ASSUME_Chaos_COMPLIANT: Uses lockfree patterns from kdb core
//!
//! # Example
//!
//! ```ignore
//! use kdb::tests::e2e::harness::prelude::*;
//!
//! #[test]
//! fn test_breakpoint_hit() -> E2EResult<()> {
//!     let mut spawner = ProcessSpawner::new();
//!     let mut driver = DebuggerDriver::new();
//!     let validator = OutputValidator::new();
//!
//!     // Spawn test target
//!     let process = spawner.spawn_sleep(60)?;
//!
//!     // Attach and set breakpoint
//!     driver.attach(process.pid)?;
//!     let bp_id = driver.set_breakpoint("0x400000")?;
//!
//!     // Continue and wait for breakpoint
//!     let reason = driver.continue_execution()?;
//!     assert!(matches!(reason, StopReason::Breakpoint(_)));
//!
//!     // Validate state
//!     let regs = driver.get_registers()?;
//!     validator.validate_register(&regs, "rip", 0x400000).into_result()?;
//!
//!     // Cleanup
//!     driver.detach()?;
//!     Ok(())
//! }
//! ```

pub mod debugger_driver;
pub mod error;
pub mod gdb_driver;
pub mod output_validator;
pub mod process_spawner;

// Re-export main types for convenience
pub use debugger_driver::{
    BreakpointId, DebuggerDriver, DebuggerEvent, Registers, SnapshotId, StackFrame, StopReason,
};
pub use error::{E2EError, E2EResult};
pub use gdb_driver::{GdbDriver, GdbMiResponse, GdbRegisters, GdbStackFrame, GdbStopReason};
pub use output_validator::{ComparisonResult, OutputValidator, ValidationConfig};
pub use process_spawner::{ProcessSpawner, SpawnedProcess};

/// Prelude module for convenient imports
///
/// ```ignore
/// use kdb::tests::e2e::harness::prelude::*;
/// ```
pub mod prelude {
    pub use super::debugger_driver::{
        BreakpointId, DebuggerDriver, DebuggerEvent, Registers, SnapshotId, StackFrame, StopReason,
    };
    pub use super::error::{E2EError, E2EResult};
    pub use super::gdb_driver::{GdbDriver, GdbMiResponse, GdbRegisters, GdbStackFrame, GdbStopReason};
    pub use super::output_validator::{ComparisonResult, OutputValidator, ValidationConfig};
    pub use super::process_spawner::{ProcessSpawner, SpawnedProcess};
}

/// Test fixture for common E2E test setup
///
/// Provides a convenient way to set up the common test infrastructure
/// with automatic cleanup.
///
/// # Example
///
/// ```ignore
/// let fixture = E2EFixture::new()?;
/// let process = fixture.spawner.spawn_sleep(60)?;
/// fixture.driver.attach(process.pid)?;
/// // ... test logic ...
/// // Cleanup happens automatically on drop
/// ```
pub struct E2EFixture {
    /// Process spawner for managing test targets
    pub spawner: ProcessSpawner,
    /// kdb driver for debugging operations
    pub driver: DebuggerDriver,
    /// Output validator for assertions
    pub validator: OutputValidator,
}

impl E2EFixture {
    /// Create a new E2E fixture with default configuration
    pub fn new() -> E2EResult<Self> {
        Ok(Self {
            spawner: ProcessSpawner::new(),
            driver: DebuggerDriver::new(),
            validator: OutputValidator::new(),
        })
    }

    /// Create a fixture with strict validation
    pub fn strict() -> E2EResult<Self> {
        Ok(Self {
            spawner: ProcessSpawner::new(),
            driver: DebuggerDriver::new(),
            validator: OutputValidator::with_config(ValidationConfig::strict()),
        })
    }

    /// Create a fixture with lenient validation
    pub fn lenient() -> E2EResult<Self> {
        Ok(Self {
            spawner: ProcessSpawner::new(),
            driver: DebuggerDriver::new(),
            validator: OutputValidator::with_config(ValidationConfig::lenient()),
        })
    }

    /// Quick setup: spawn a sleep process and attach to it
    ///
    /// Returns the process PID for reference.
    pub fn quick_attach(&mut self, sleep_seconds: u32) -> E2EResult<u32> {
        let process = self.spawner.spawn_sleep(sleep_seconds)?;
        let pid = process.pid;
        self.driver.attach(pid)?;
        Ok(pid)
    }

    /// Cleanup: detach from process if attached
    pub fn cleanup(&mut self) -> E2EResult<()> {
        if self.driver.is_attached() {
            self.driver.detach()?;
        }
        self.spawner.cleanup();
        Ok(())
    }
}

impl Default for E2EFixture {
    fn default() -> Self {
        Self::new().expect("Failed to create E2EFixture")
    }
}

impl Drop for E2EFixture {
    fn drop(&mut self) {
        // Best-effort cleanup
        let _ = self.cleanup();
    }
}

/// Comparison fixture for kdb vs GDB testing
///
/// Provides both kdb and GDB drivers for correctness comparison.
///
/// # Example
///
/// ```ignore
/// let fixture = ComparisonFixture::new()?;
/// fixture.spawner.spawn_sleep(60)?;
///
/// // Attach both debuggers
/// fixture.kdb.attach(pid)?;
/// fixture.gdb.attach(pid)?;
///
/// // Compare behavior
/// let kdb_regs = fixture.kdb.get_registers()?;
/// let gdb_regs = fixture.gdb.get_registers()?;
/// fixture.validator.assert_registers_match(&kdb_regs, &gdb_regs);
/// ```
pub struct ComparisonFixture {
    /// Process spawner
    pub spawner: ProcessSpawner,
    /// kdb driver
    pub kdb: DebuggerDriver,
    /// GDB driver
    pub gdb: GdbDriver,
    /// Output validator
    pub validator: OutputValidator,
}

impl ComparisonFixture {
    /// Create a new comparison fixture
    ///
    /// # Errors
    ///
    /// - `SpawnFailed` if GDB cannot be started
    pub fn new() -> E2EResult<Self> {
        Ok(Self {
            spawner: ProcessSpawner::new(),
            kdb: DebuggerDriver::new(),
            gdb: GdbDriver::new()?,
            validator: OutputValidator::new(),
        })
    }

    /// Create a fixture with custom GDB path
    pub fn with_gdb_path(gdb_path: &str) -> E2EResult<Self> {
        Ok(Self {
            spawner: ProcessSpawner::new(),
            kdb: DebuggerDriver::new(),
            gdb: GdbDriver::with_gdb_path(gdb_path)?,
            validator: OutputValidator::new(),
        })
    }

    /// Quick setup: spawn a process and attach both debuggers
    ///
    /// Note: Attaching two debuggers to the same process is not supported
    /// by Linux ptrace. This method spawns TWO processes for parallel testing.
    pub fn quick_attach_parallel(&mut self, sleep_seconds: u32) -> E2EResult<(u32, u32)> {
        // Spawn two processes for parallel debugging
        let kdb_process = self.spawner.spawn_sleep(sleep_seconds)?;
        let kdb_pid = kdb_process.pid;

        let gdb_process = self.spawner.spawn_sleep(sleep_seconds)?;
        let gdb_pid = gdb_process.pid;

        self.kdb.attach(kdb_pid)?;
        self.gdb.attach(gdb_pid)?;

        Ok((kdb_pid, gdb_pid))
    }

    /// Cleanup both debuggers
    pub fn cleanup(&mut self) -> E2EResult<()> {
        if self.kdb.is_attached() {
            self.kdb.detach()?;
        }
        if self.gdb.is_attached() {
            self.gdb.detach()?;
        }
        let _ = self.gdb.quit();
        self.spawner.cleanup();
        Ok(())
    }
}

impl Drop for ComparisonFixture {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_e2e_fixture_creation() {
        let fixture = E2EFixture::new();
        assert!(fixture.is_ok());
    }

    #[test]
    fn test_e2e_fixture_default() {
        let fixture = E2EFixture::default();
        assert!(!fixture.driver.is_attached());
    }

    #[test]
    fn test_prelude_imports() {
        // Just verify the prelude compiles
        use super::prelude::*;

        let _: E2EResult<()> = Ok(());
        let _bp = BreakpointId(1);
        let _snap = SnapshotId(1);
    }
}
