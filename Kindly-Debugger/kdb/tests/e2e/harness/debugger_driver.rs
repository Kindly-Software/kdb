//! Debugger Driver - Programmatic control of kdb debugger
//!
//! Provides a high-level API for driving the kdb debugger in E2E tests.
//! Uses the kdb library directly (not CLI) for maximum control and performance.
//!
//! # ASSUM Safety
//!
//! - #ASSUME_SINGLE_ATTACH: Only one process attached at a time
//! - #ASSUME_STOPPED_FOR_OPS: Process must be stopped for register/memory reads
//! - #ASSUME_AUDIT_ENABLED: Audit trail always active for Q34 compliance

use super::error::{E2EError, E2EResult};
use kdb::debugger::{DebuggerCapsule, DebuggerStats};
use kdb::time_travel::ReplayEngineCapsule;
use std::sync::atomic::Ordering;

/// Unique identifier for a breakpoint
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BreakpointId(pub usize);

/// Unique identifier for a snapshot
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SnapshotId(pub u64);

/// Reason why execution stopped
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// Hit a breakpoint
    Breakpoint(BreakpointId),
    /// Single-step completed
    Step,
    /// Signal received
    Signal(i32),
    /// Process exited
    Exited(i32),
    /// Detached from process
    Detached,
    /// Unknown reason
    Unknown,
}

/// CPU register state
#[derive(Debug, Clone, Default)]
pub struct Registers {
    /// Instruction pointer (RIP on x86_64)
    pub rip: u64,
    /// Stack pointer (RSP on x86_64)
    pub rsp: u64,
    /// Base pointer (RBP on x86_64)
    pub rbp: u64,
    /// General purpose registers
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    /// Flags register (RFLAGS on x86_64)
    pub rflags: u64,
}

impl Registers {
    /// Create registers from raw values
    pub fn from_rip_rsp(rip: u64, rsp: u64) -> Self {
        Self {
            rip,
            rsp,
            ..Default::default()
        }
    }
}

/// Stack frame information
#[derive(Debug, Clone)]
pub struct StackFrame {
    /// Frame index (0 = current, 1 = caller, etc.)
    pub index: usize,
    /// Instruction pointer for this frame
    pub rip: u64,
    /// Stack pointer for this frame
    pub rsp: u64,
    /// Frame pointer for this frame
    pub rbp: u64,
    /// Function name (if resolved)
    pub function_name: Option<String>,
    /// Source file and line (if available)
    pub source_location: Option<(String, u32)>,
}

/// Event recorded by the debugger
#[derive(Debug, Clone)]
pub enum DebuggerEvent {
    /// Attached to a process
    Attached { pid: u32 },
    /// Detached from a process
    Detached { pid: u32 },
    /// Breakpoint set
    BreakpointSet { id: BreakpointId, address: u64 },
    /// Breakpoint hit
    BreakpointHit { id: BreakpointId, rip: u64 },
    /// Step executed
    Stepped { rip: u64 },
    /// Step backward (time-travel)
    SteppedBackward { rip: u64, snapshot_id: SnapshotId },
    /// Snapshot captured
    SnapshotCaptured { id: SnapshotId, rip: u64 },
    /// Execution continued
    Continued,
    /// Error occurred
    Error { message: String },
}

/// Drive kdb debugger programmatically
///
/// This driver wraps the kdb library's `DebuggerCapsule` and provides
/// a high-level API suitable for E2E testing.
///
/// # Example
///
/// ```ignore
/// let mut driver = DebuggerDriver::new();
/// driver.attach(pid)?;
/// let bp = driver.set_breakpoint("main")?;
/// let reason = driver.continue_execution()?;
/// let regs = driver.get_registers()?;
/// driver.detach()?;
/// ```
pub struct DebuggerDriver {
    /// The underlying debugger capsule (heap-allocated due to size)
    debugger: Box<DebuggerCapsule>,
    /// Events recorded during debugging session
    events: Vec<DebuggerEvent>,
    /// Currently attached PID (None if not attached)
    attached_pid: Option<u32>,
    /// Breakpoint counter for ID generation
    breakpoint_counter: usize,
}

impl DebuggerDriver {
    /// Create a new DebuggerDriver
    ///
    /// The debugger is initialized but not attached to any process.
    pub fn new() -> Self {
        Self {
            debugger: Box::new(DebuggerCapsule::new(0)),
            events: Vec::new(),
            attached_pid: None,
            breakpoint_counter: 0,
        }
    }

    /// Check if attached to a process
    pub fn is_attached(&self) -> bool {
        self.attached_pid.is_some()
    }

    /// Get the attached PID
    pub fn attached_pid(&self) -> Option<u32> {
        self.attached_pid
    }

    /// Get recorded events
    pub fn events(&self) -> &[DebuggerEvent] {
        &self.events
    }

    /// Clear recorded events
    pub fn clear_events(&mut self) {
        self.events.clear();
    }

    /// Attach to a process
    ///
    /// # Arguments
    ///
    /// * `pid` - Process ID to attach to
    ///
    /// # Errors
    ///
    /// - `AttachFailed` if attachment fails
    pub fn attach(&mut self, pid: u32) -> E2EResult<()> {
        if self.attached_pid.is_some() {
            return Err(E2EError::generic(
                "attach",
                "Already attached to a process",
            ));
        }

        // Create a new debugger capsule for this PID
        self.debugger = Box::new(DebuggerCapsule::new(pid as u64));

        // Attempt to attach
        self.debugger
            .attach_to_process(pid as u64)
            .map_err(|e| E2EError::attach_failed(pid, e))?;

        self.attached_pid = Some(pid);
        self.events.push(DebuggerEvent::Attached { pid });

        Ok(())
    }

    /// Detach from the current process
    ///
    /// # Errors
    ///
    /// - `NotAttached` if not attached to any process
    /// - `DetachFailed` if detachment fails
    pub fn detach(&mut self) -> E2EResult<()> {
        let pid = self.attached_pid.ok_or(E2EError::NotAttached)?;

        // The debugger capsule doesn't have an explicit detach method,
        // but we reset our state
        self.attached_pid = None;
        self.events.push(DebuggerEvent::Detached { pid });

        Ok(())
    }

    /// Set a breakpoint at an address or symbol
    ///
    /// # Arguments
    ///
    /// * `addr_or_symbol` - Address (hex with 0x prefix) or symbol name
    ///
    /// # Returns
    ///
    /// A `BreakpointId` on success
    ///
    /// # Errors
    ///
    /// - `NotAttached` if not attached
    /// - `BreakpointFailed` if breakpoint couldn't be set
    pub fn set_breakpoint(&mut self, addr_or_symbol: &str) -> E2EResult<BreakpointId> {
        if !self.is_attached() {
            return Err(E2EError::NotAttached);
        }

        // Parse address
        let address = if addr_or_symbol.starts_with("0x") || addr_or_symbol.starts_with("0X") {
            u64::from_str_radix(&addr_or_symbol[2..], 16)
                .map_err(|_| E2EError::breakpoint_failed(addr_or_symbol, "Invalid address format"))?
        } else {
            // Symbol lookup would go here
            // For now, return an error for symbols
            return Err(E2EError::breakpoint_failed(
                addr_or_symbol,
                "Symbol lookup not yet implemented",
            ));
        };

        // Set the breakpoint
        let _bp_idx = self
            .debugger
            .set_breakpoint(address)
            .map_err(|e| E2EError::breakpoint_failed(addr_or_symbol, e))?;

        let id = BreakpointId(self.breakpoint_counter);
        self.breakpoint_counter += 1;

        self.events.push(DebuggerEvent::BreakpointSet { id, address });

        Ok(id)
    }

    /// Continue execution until a stop event
    ///
    /// # Returns
    ///
    /// The reason why execution stopped
    ///
    /// # Errors
    ///
    /// - `NotAttached` if not attached
    pub fn continue_execution(&mut self) -> E2EResult<StopReason> {
        if !self.is_attached() {
            return Err(E2EError::NotAttached);
        }

        self.debugger
            .continue_execution()
            .map_err(|e| E2EError::generic("continue", e))?;

        self.events.push(DebuggerEvent::Continued);

        // In a real implementation, we would wait for a stop event
        // For now, return Step as the default stop reason
        Ok(StopReason::Step)
    }

    /// Single-step one instruction
    ///
    /// # Errors
    ///
    /// - `NotAttached` if not attached
    /// - `StepFailed` if step fails
    pub fn step(&mut self) -> E2EResult<()> {
        if !self.is_attached() {
            return Err(E2EError::NotAttached);
        }

        let new_rip = self
            .debugger
            .step_instruction()
            .map_err(|e| E2EError::StepFailed { reason: e.to_string() })?;

        self.events.push(DebuggerEvent::Stepped { rip: new_rip });

        Ok(())
    }

    /// Step backward (time-travel) to the previous snapshot
    ///
    /// # Errors
    ///
    /// - `NotAttached` if not attached
    /// - `StepFailed` if no previous snapshot exists
    pub fn step_backward(&mut self) -> E2EResult<()> {
        if !self.is_attached() {
            return Err(E2EError::NotAttached);
        }

        let rip = self
            .debugger
            .step_backward()
            .map_err(|e| E2EError::StepFailed { reason: e.to_string() })?;

        let (current_snapshot, _) = self.debugger.replay_engine.get_stats();
        let snapshot_id = SnapshotId(current_snapshot);

        self.events.push(DebuggerEvent::SteppedBackward { rip, snapshot_id });

        Ok(())
    }

    /// Capture a time-travel snapshot
    ///
    /// # Returns
    ///
    /// A `SnapshotId` for the captured snapshot
    ///
    /// # Errors
    ///
    /// - `NotAttached` if not attached
    /// - `SnapshotFailed` if snapshot capture fails
    pub fn capture_snapshot(&mut self) -> E2EResult<SnapshotId> {
        if !self.is_attached() {
            return Err(E2EError::NotAttached);
        }

        let rip = self.debugger.execution.get_rip();
        let rsp = self.debugger.execution.rsp.load(Ordering::Relaxed);

        let id = self
            .debugger
            .replay_engine
            .take_snapshot(rip, rsp)
            .map_err(|e| E2EError::SnapshotFailed { reason: e.to_string() })?;

        let snapshot_id = SnapshotId(id);
        self.events.push(DebuggerEvent::SnapshotCaptured { id: snapshot_id, rip });

        Ok(snapshot_id)
    }

    /// Get current CPU registers
    ///
    /// # Returns
    ///
    /// The current register state
    ///
    /// # Errors
    ///
    /// - `NotAttached` if not attached
    /// - `RegisterReadFailed` if register read fails
    pub fn get_registers(&self) -> E2EResult<Registers> {
        if !self.is_attached() {
            return Err(E2EError::NotAttached);
        }

        // Read from the debugger's execution state
        let rip = self.debugger.execution.get_rip();
        let rsp = self.debugger.execution.rsp.load(Ordering::Relaxed);
        let rbp = self.debugger.execution.rbp.load(Ordering::Relaxed);

        Ok(Registers {
            rip,
            rsp,
            rbp,
            ..Default::default()
        })
    }

    /// Get the current stack trace
    ///
    /// # Returns
    ///
    /// A vector of stack frames (innermost first)
    ///
    /// # Errors
    ///
    /// - `NotAttached` if not attached
    /// - `StackTraceFailed` if stack unwinding fails
    pub fn get_stack_trace(&self) -> E2EResult<Vec<StackFrame>> {
        if !self.is_attached() {
            return Err(E2EError::NotAttached);
        }

        let trace = self
            .debugger
            .get_stack_trace()
            .map_err(|e| E2EError::StackTraceFailed { reason: e.to_string() })?;

        // Convert to our StackFrame type
        let frames = trace
            .iter()
            .enumerate()
            .map(|(index, &rip)| StackFrame {
                index,
                rip,
                rsp: 0, // Would need to read from capsule
                rbp: 0,
                function_name: None,
                source_location: None,
            })
            .collect();

        Ok(frames)
    }

    /// Verify the audit trail integrity (Q34 compliance)
    ///
    /// # Returns
    ///
    /// `true` if the audit trail is valid, `false` otherwise
    ///
    /// # Errors
    ///
    /// - `AuditVerificationFailed` if verification encounters an error
    pub fn verify_audit_trail(&self) -> E2EResult<bool> {
        self.debugger
            .replay_engine
            .verify_hash_chain(0)
            .map_err(|e| E2EError::AuditVerificationFailed { reason: e.to_string() })
    }

    /// Get the root hash of the audit trail
    pub fn get_audit_root_hash(&self) -> u64 {
        self.debugger.replay_engine.get_root_hash()
    }

    /// Get debugger statistics
    pub fn get_stats(&self) -> DebuggerStats {
        self.debugger.get_stats()
    }

    /// Get the total number of snapshots taken
    pub fn snapshot_count(&self) -> u64 {
        let (_, total) = self.debugger.replay_engine.get_stats();
        total
    }

    /// Navigate to a specific snapshot
    ///
    /// # Arguments
    ///
    /// * `snapshot_id` - The snapshot to navigate to
    ///
    /// # Returns
    ///
    /// The register state at that snapshot
    pub fn goto_snapshot(&mut self, snapshot_id: SnapshotId) -> E2EResult<Registers> {
        if !self.is_attached() {
            return Err(E2EError::NotAttached);
        }

        let (_, rip, rsp) = self
            .debugger
            .replay_engine
            .jump_to_snapshot(snapshot_id.0)
            .map_err(|e| E2EError::SnapshotFailed { reason: e.to_string() })?;

        Ok(Registers::from_rip_rsp(rip, rsp))
    }
}

impl Default for DebuggerDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_driver_creation() {
        let driver = DebuggerDriver::new();
        assert!(!driver.is_attached());
        assert!(driver.events().is_empty());
    }

    #[test]
    fn test_not_attached_errors() {
        let driver = DebuggerDriver::new();
        assert!(matches!(driver.get_registers(), Err(E2EError::NotAttached)));
        assert!(matches!(driver.get_stack_trace(), Err(E2EError::NotAttached)));
    }

    #[test]
    fn test_breakpoint_id() {
        let id1 = BreakpointId(1);
        let id2 = BreakpointId(1);
        let id3 = BreakpointId(2);
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_snapshot_id() {
        let id1 = SnapshotId(100);
        let id2 = SnapshotId(100);
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_registers_default() {
        let regs = Registers::default();
        assert_eq!(regs.rip, 0);
        assert_eq!(regs.rsp, 0);
    }

    #[test]
    fn test_audit_trail_verification() {
        let driver = DebuggerDriver::new();
        // Empty audit trail should be valid
        let result = driver.verify_audit_trail();
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
}
