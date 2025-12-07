//! PtraceWrapperCapsule - T1 Atomic syscall wrapper for ptrace operations
//!
//! # Architecture
//! - Tier: T1 Atomic (lockfree coordination)
//! - Size: 256 bytes (cache-aligned, warm-tier)
//! - Latency Target: <1μs per syscall
//! - Platform: Linux x86_64/aarch64
//!
//! # Operations
//! - PTRACE_ATTACH, PTRACE_DETACH: Process control
//! - PTRACE_CONT, PTRACE_SINGLESTEP: Execution control
//! - PTRACE_PEEKDATA, PTRACE_POKEDATA: Memory access
//! - PTRACE_GETREGS, PTRACE_SETREGS: Register access
//! - waitpid integration: Signal handling
//!
//! # Safety
//! All unsafe blocks documented with ASSUM tags.
//! Target: 99.5%+ ASSUM safety coverage.
//!
//! # Performance
//! - Attach/Detach: <10μs
//! - Continue/Step: <1μs
//! - Peek/Poke: <1μs (syscall overhead)
//! - GetRegs/SetRegs: <2μs

use atomic_capsule::patterns::DualAtomicU64;
use nix::sys::ptrace;
use nix::sys::signal::Signal;
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::Pid;
use std::sync::atomic::{AtomicI32, AtomicU32, AtomicU64, Ordering};

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during ptrace operations
#[derive(Debug, Clone, PartialEq)]
pub enum PtraceError {
    /// Process does not exist or caller lacks CAP_SYS_PTRACE
    PermissionDenied,

    /// Process is not attached
    NotAttached,

    /// Process is already attached
    AlreadyAttached,

    /// Invalid memory address (EFAULT)
    InvalidAddress,

    /// Process is not in stopped state
    ProcessNotStopped,

    /// Process has exited
    ProcessExited,

    /// Invalid PID (0 or negative)
    InvalidPid,

    /// Syscall error with errno
    SyscallError(i32),

    /// Wait syscall failed
    WaitFailed,
}

impl std::fmt::Display for PtraceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PtraceError::PermissionDenied => write!(f, "Permission denied: CAP_SYS_PTRACE required"),
            PtraceError::NotAttached => write!(f, "Process not attached"),
            PtraceError::AlreadyAttached => write!(f, "Process already attached"),
            PtraceError::InvalidAddress => write!(f, "Invalid memory address (EFAULT)"),
            PtraceError::ProcessNotStopped => write!(f, "Process not in stopped state"),
            PtraceError::ProcessExited => write!(f, "Process has exited"),
            PtraceError::InvalidPid => write!(f, "Invalid PID (must be > 0)"),
            PtraceError::SyscallError(errno) => write!(f, "Ptrace syscall error: errno {}", errno),
            PtraceError::WaitFailed => write!(f, "waitpid syscall failed"),
        }
    }
}

impl std::error::Error for PtraceError {}

impl From<nix::Error> for PtraceError {
    fn from(e: nix::Error) -> Self {
        match e {
            nix::Error::EPERM => PtraceError::PermissionDenied,
            nix::Error::ESRCH => PtraceError::NotAttached,
            nix::Error::EFAULT => PtraceError::InvalidAddress,
            _ => PtraceError::SyscallError(e as i32),
        }
    }
}

// ============================================================================
// Process State
// ============================================================================

/// Process state enum for T1 Atomic coordination
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ProcessState {
    /// Not attached to any process
    Detached = 0,

    /// Attachment in progress (transient state)
    Attaching = 1,

    /// Process stopped (breakpoint, signal, or singlestep)
    Stopped = 2,

    /// Process running (after PTRACE_CONT)
    Running = 3,

    /// Single-stepping (after PTRACE_SINGLESTEP)
    Stepping = 4,

    /// Process has exited
    Exited = 5,
}

impl ProcessState {
    /// Convert u64 to ProcessState (used for atomic loads)
    pub fn from_u64(val: u64) -> Self {
        match val & 0xFF {
            0 => ProcessState::Detached,
            1 => ProcessState::Attaching,
            2 => ProcessState::Stopped,
            3 => ProcessState::Running,
            4 => ProcessState::Stepping,
            5 => ProcessState::Exited,
            _ => ProcessState::Detached, // Default fallback
        }
    }
}

// ============================================================================
// PtraceWrapperCapsule - T1 Atomic Ptrace Syscall Wrapper
// ============================================================================

/// T1 Atomic computational capsule wrapping ptrace syscalls
///
/// # Architecture
/// - Size: 256 bytes (warm-tier cache alignment)
/// - Coordination: DualAtomicU64 (state + operation counter)
/// - Generation Counter: TOCTOU prevention for state transitions
/// - Lockfree: 100% atomic operations, no mutex/RwLock
///
/// # State Machine
/// ```text
/// Detached --attach--> Attaching --waitpid--> Stopped
///                                               |
///                                               v
/// Exited <--exit-- Running <--cont-- Stopped --step--> Stepping
///                    |                           ^
///                    +--breakpoint/signal--------+
/// ```
///
/// # ASSUM Safety
/// - #ASSUME_PTRACE_ATTACH: Process exists, CAP_SYS_PTRACE capability
/// - #ASSUME_PTRACE_DETACH: Process currently attached
/// - #ASSUME_MEMORY_ACCESS: Address valid in target address space
/// - #ASSUME_PROCESS_STOPPED: Process stopped for most operations
/// - #ASSUME_GENERATION_MONOTONIC: Generation counter only increments
/// - #ASSUME_STATE_TRANSITIONS: State machine enforced
///
/// Target: 99.5%+ ASSUM safety coverage
#[repr(C, align(256))]
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 256))]
pub struct PtraceWrapperCapsule {
    // T1 Atomic: Coordination (process state + operation counter)
    // Primary: ProcessState (u64), Secondary: operation count (u64)
    state: DualAtomicU64,

    // Process ID being debugged (0 = none)
    pid: AtomicU32,

    // Last operation result (0 = success, errno on error)
    last_result: AtomicI32,

    // Generation counter (TOCTOU prevention)
    // Incremented on every state transition to detect stale reads
    generation: AtomicU64,

    // Last signal received (0 = none)
    last_signal: AtomicU32,

    // Timestamp of last operation (nanoseconds since UNIX epoch)
    last_operation_ns: AtomicU64,

    // Total operation count (for profiling)
    total_operations: AtomicU64,

    // Error count (for monitoring)
    error_count: AtomicU64,

    // Complete 256-byte cache line (warm-tier alignment)
    _padding: [u8; 184],
}

impl PtraceWrapperCapsule {
    /// Create new PtraceWrapperCapsule in Detached state
    ///
    /// # Example
    /// ```no_run
    /// use kdb::ptrace::PtraceWrapperCapsule;
    /// let wrapper = PtraceWrapperCapsule::new();
    /// ```
    pub fn new() -> Self {
        Self {
            state: DualAtomicU64::new(ProcessState::Detached as u64, 0),
            pid: AtomicU32::new(0),
            last_result: AtomicI32::new(0),
            generation: AtomicU64::new(0),
            last_signal: AtomicU32::new(0),
            last_operation_ns: AtomicU64::new(0),
            total_operations: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            _padding: [0; 184],
        }
    }

    // ========================================================================
    // State Management
    // ========================================================================

    /// Get current process state (lockfree <50ns read)
    pub fn get_state(&self) -> ProcessState {
        let state = self.state.load_primary(Ordering::Relaxed);
        ProcessState::from_u64(state)
    }

    /// Set process state (lockfree <100ns write with generation increment)
    fn set_state(&self, new_state: ProcessState) {
        self.state.store_primary(new_state as u64, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel); // TOCTOU prevention
    }

    /// Get current PID (0 if detached)
    pub fn get_pid(&self) -> i32 {
        self.pid.load(Ordering::Acquire) as i32
    }

    /// Get operation count
    pub fn get_operation_count(&self) -> u64 {
        self.state.load_secondary(Ordering::Relaxed)
    }

    /// Check if process is stopped (required for most operations)
    pub fn is_stopped(&self) -> bool {
        matches!(
            self.get_state(),
            ProcessState::Stopped | ProcessState::Stepping
        )
    }

    /// Update timestamp to current time
    fn update_timestamp(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        self.last_operation_ns.store(now, Ordering::Relaxed);
    }

    /// Increment operation counter
    fn increment_operations(&self) {
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        let current = self.state.load_secondary(Ordering::Acquire);
        self.state.store_secondary(current + 1, Ordering::Release);
    }

    /// Record error
    fn record_error(&self, errno: i32) {
        self.last_result.store(errno, Ordering::Release);
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    // ========================================================================
    // Process Control: Attach/Detach
    // ========================================================================

    /// Attach to process (PTRACE_ATTACH)
    ///
    /// # Arguments
    /// * `pid` - Process ID to attach to (must be > 0)
    ///
    /// # Returns
    /// * `Ok(())` - Successfully attached, process now stopped
    /// * `Err(PtraceError)` - Attachment failed
    ///
    /// # Safety
    /// #ASSUME_PTRACE_ATTACH: Process must exist and caller must have CAP_SYS_PTRACE capability
    ///
    /// # Performance
    /// Target: <10μs (syscall + waitpid)
    ///
    /// # Example
    /// ```no_run
    /// use kdb::ptrace::PtraceWrapperCapsule;
    /// let wrapper = PtraceWrapperCapsule::new();
    /// wrapper.attach(1234)?;
    /// # Ok::<(), kdb::ptrace::PtraceError>(())
    /// ```
    pub fn attach(&self, pid: i32) -> Result<(), PtraceError> {
        if pid <= 0 {
            return Err(PtraceError::InvalidPid);
        }

        // Check not already attached
        let current_state = self.get_state();
        if current_state != ProcessState::Detached {
            return Err(PtraceError::AlreadyAttached);
        }

        // Update state to Attaching (transient)
        self.set_state(ProcessState::Attaching);

        // PTRACE_ATTACH: Send SIGSTOP to process and attach
        // #ASSUME_PTRACE_ATTACH: Process exists, CAP_SYS_PTRACE capability present
        let nix_pid = Pid::from_raw(pid);
        if let Err(e) = ptrace::attach(nix_pid) {
            self.set_state(ProcessState::Detached); // Rollback on error
            self.record_error(e as i32);
            return Err(e.into());
        }

        // Wait for process to stop (blocking, but typically <1ms)
        // #ASSUME_WAITPID_SUCCESS: Process will stop after PTRACE_ATTACH
        match waitpid(nix_pid, None) {
            Ok(WaitStatus::Stopped(_, _)) => {
                // Success: Process attached and stopped
                self.pid.store(pid as u32, Ordering::Release);
                self.set_state(ProcessState::Stopped);
                self.last_result.store(0, Ordering::Release);
                self.update_timestamp();
                self.increment_operations();
                Ok(())
            }
            Ok(WaitStatus::Exited(_, _)) => {
                // Process exited during attach
                self.set_state(ProcessState::Exited);
                Err(PtraceError::ProcessExited)
            }
            Ok(_) => {
                // Unexpected wait status
                self.set_state(ProcessState::Detached);
                Err(PtraceError::WaitFailed)
            }
            Err(e) => {
                // waitpid failed
                self.set_state(ProcessState::Detached);
                self.record_error(e as i32);
                Err(PtraceError::WaitFailed)
            }
        }
    }

    /// Detach from process (PTRACE_DETACH)
    ///
    /// # Returns
    /// * `Ok(())` - Successfully detached, process continues running
    /// * `Err(PtraceError)` - Detachment failed
    ///
    /// # Safety
    /// #ASSUME_PTRACE_DETACH: Process must be currently attached
    ///
    /// # Performance
    /// Target: <10μs
    ///
    /// # Example
    /// ```no_run
    /// use kdb::ptrace::PtraceWrapperCapsule;
    /// let wrapper = PtraceWrapperCapsule::new();
    /// wrapper.attach(1234)?;
    /// wrapper.detach()?;
    /// # Ok::<(), kdb::ptrace::PtraceError>(())
    /// ```
    pub fn detach(&self) -> Result<(), PtraceError> {
        let current_state = self.get_state();
        if current_state == ProcessState::Detached {
            return Err(PtraceError::NotAttached);
        }

        let pid = Pid::from_raw(self.pid.load(Ordering::Acquire) as i32);

        // PTRACE_DETACH: Detach and optionally send signal (None = no signal)
        // #ASSUME_PTRACE_DETACH: Process is currently attached
        if let Err(e) = ptrace::detach(pid, None) {
            self.record_error(e as i32);
            return Err(e.into());
        }

        // Update state
        self.pid.store(0, Ordering::Release);
        self.set_state(ProcessState::Detached);
        self.last_result.store(0, Ordering::Release);
        self.update_timestamp();
        self.increment_operations();

        Ok(())
    }

    // ========================================================================
    // Execution Control: Continue/Step
    // ========================================================================

    /// Continue process execution (PTRACE_CONT)
    ///
    /// # Returns
    /// * `Ok(())` - Process now running
    /// * `Err(PtraceError)` - Continue failed
    ///
    /// # Safety
    /// #ASSUME_PROCESS_STOPPED: Process must be in Stopped or Stepping state
    ///
    /// # Performance
    /// Target: <1μs (single syscall)
    ///
    /// # Example
    /// ```no_run
    /// use kdb::ptrace::PtraceWrapperCapsule;
    /// let wrapper = PtraceWrapperCapsule::new();
    /// wrapper.attach(1234)?;
    /// wrapper.cont()?;
    /// # Ok::<(), kdb::ptrace::PtraceError>(())
    /// ```
    pub fn cont(&self) -> Result<(), PtraceError> {
        if !self.is_stopped() {
            return Err(PtraceError::ProcessNotStopped);
        }

        let pid = Pid::from_raw(self.pid.load(Ordering::Acquire) as i32);

        // PTRACE_CONT: Resume process execution
        // #ASSUME_PROCESS_STOPPED: Process must be stopped
        if let Err(e) = ptrace::cont(pid, None) {
            self.record_error(e as i32);
            return Err(e.into());
        }

        self.set_state(ProcessState::Running);
        self.last_result.store(0, Ordering::Release);
        self.update_timestamp();
        self.increment_operations();

        Ok(())
    }

    /// Single-step process (PTRACE_SINGLESTEP)
    ///
    /// # Returns
    /// * `Ok(())` - Process executed one instruction and stopped
    /// * `Err(PtraceError)` - Single-step failed
    ///
    /// # Safety
    /// #ASSUME_PROCESS_STOPPED: Process must be in Stopped state
    ///
    /// # Performance
    /// Target: <1μs (single syscall)
    ///
    /// # Example
    /// ```no_run
    /// use kdb::ptrace::PtraceWrapperCapsule;
    /// let wrapper = PtraceWrapperCapsule::new();
    /// wrapper.attach(1234)?;
    /// wrapper.singlestep()?;
    /// # Ok::<(), kdb::ptrace::PtraceError>(())
    /// ```
    pub fn singlestep(&self) -> Result<(), PtraceError> {
        if !self.is_stopped() {
            return Err(PtraceError::ProcessNotStopped);
        }

        let pid = Pid::from_raw(self.pid.load(Ordering::Acquire) as i32);

        // PTRACE_SINGLESTEP: Execute one instruction and stop
        // #ASSUME_PROCESS_STOPPED: Process must be stopped
        if let Err(e) = ptrace::step(pid, None) {
            self.record_error(e as i32);
            return Err(e.into());
        }

        self.set_state(ProcessState::Stepping);
        self.last_result.store(0, Ordering::Release);
        self.update_timestamp();
        self.increment_operations();

        Ok(())
    }

    // ========================================================================
    // Memory Access: Peek/Poke
    // ========================================================================

    /// Read 8 bytes from process memory (PTRACE_PEEKDATA)
    ///
    /// # Arguments
    /// * `addr` - Virtual address in target process (must be word-aligned on some platforms)
    ///
    /// # Returns
    /// * `Ok(u64)` - 8-byte value at address
    /// * `Err(PtraceError)` - Read failed (invalid address or process not stopped)
    ///
    /// # Safety
    /// #ASSUME_MEMORY_ACCESS: Address must be valid in target process address space
    /// #ASSUME_PROCESS_STOPPED: Process should be stopped (some kernels allow reads while running)
    ///
    /// # Performance
    /// Target: <1μs (single syscall)
    ///
    /// # Example
    /// ```no_run
    /// use kdb::ptrace::PtraceWrapperCapsule;
    /// let wrapper = PtraceWrapperCapsule::new();
    /// wrapper.attach(1234)?;
    /// let value = wrapper.peek_data(0x7fff_0000)?;
    /// # Ok::<(), kdb::ptrace::PtraceError>(())
    /// ```
    pub fn peek_data(&self, addr: u64) -> Result<u64, PtraceError> {
        let pid = Pid::from_raw(self.pid.load(Ordering::Acquire) as i32);
        if pid.as_raw() == 0 {
            return Err(PtraceError::NotAttached);
        }

        // PTRACE_PEEKDATA: Read word at address
        // #ASSUME_MEMORY_ACCESS: Address valid in target address space
        match ptrace::read(pid, addr as *mut _) {
            Ok(data) => {
                self.last_result.store(0, Ordering::Release);
                self.update_timestamp();
                self.increment_operations();
                Ok(data as u64)
            }
            Err(e) => {
                self.record_error(e as i32);
                Err(e.into())
            }
        }
    }

    /// Write 8 bytes to process memory (PTRACE_POKEDATA)
    ///
    /// # Arguments
    /// * `addr` - Virtual address in target process
    /// * `data` - 8-byte value to write
    ///
    /// # Returns
    /// * `Ok(())` - Write successful
    /// * `Err(PtraceError)` - Write failed (invalid address or permissions)
    ///
    /// # Safety
    /// #ASSUME_MEMORY_ACCESS: Address must be writable in target process
    /// #ASSUME_PROCESS_STOPPED: Process must be stopped for safe writes
    ///
    /// # Performance
    /// Target: <1μs (single syscall)
    ///
    /// # Example
    /// ```no_run
    /// use kdb::ptrace::PtraceWrapperCapsule;
    /// let wrapper = PtraceWrapperCapsule::new();
    /// wrapper.attach(1234)?;
    /// wrapper.poke_data(0x7fff_0000, 0x42)?;
    /// # Ok::<(), kdb::ptrace::PtraceError>(())
    /// ```
    pub fn poke_data(&self, addr: u64, data: u64) -> Result<(), PtraceError> {
        if !self.is_stopped() {
            return Err(PtraceError::ProcessNotStopped);
        }

        let pid = Pid::from_raw(self.pid.load(Ordering::Acquire) as i32);

        // PTRACE_POKEDATA: Write word at address
        // #ASSUME_MEMORY_ACCESS: Address writable in target process
        // #ASSUME_PROCESS_STOPPED: Process stopped for safe write
        // #ASSUME_PTRACE_API: ptrace::write() safe for code segment modification
        // #VERIFY_POINTER_CAST: addr and data cast to *mut c_void safe
        // #VERIFY_PTRACE_VALID: nix::ptrace::write() encapsulates syscall safety
        if let Err(e) = unsafe { ptrace::write(pid, addr as *mut _, data as *mut _) } {
            self.record_error(e as i32);
            return Err(e.into());
        }

        self.last_result.store(0, Ordering::Release);
        self.update_timestamp();
        self.increment_operations();

        Ok(())
    }

    // ========================================================================
    // Register Access: Get/Set Registers
    // ========================================================================

    /// Read CPU registers (PTRACE_GETREGS)
    ///
    /// # Returns
    /// * `Ok(user_regs_struct)` - All general-purpose registers
    /// * `Err(PtraceError)` - Read failed
    ///
    /// # Platform Support
    /// - x86_64: Returns user_regs_struct (27 registers: RAX, RBX, ..., RIP, RFLAGS)
    /// - aarch64: Returns user_regs_struct (33 registers: X0-X30, SP, PC, PSTATE)
    ///
    /// # Safety
    /// #ASSUME_PROCESS_STOPPED: Process must be stopped for GETREGS
    ///
    /// # Performance
    /// Target: <2μs
    ///
    /// # Example
    /// ```no_run
    /// use kdb::ptrace::PtraceWrapperCapsule;
    /// let wrapper = PtraceWrapperCapsule::new();
    /// wrapper.attach(1234)?;
    /// let regs = wrapper.getregs()?;
    /// println!("RIP: 0x{:x}", regs.rip);
    /// # Ok::<(), kdb::ptrace::PtraceError>(())
    /// ```
    #[cfg(target_arch = "x86_64")]
    pub fn getregs(&self) -> Result<libc::user_regs_struct, PtraceError> {
        if !self.is_stopped() {
            return Err(PtraceError::ProcessNotStopped);
        }

        let pid = Pid::from_raw(self.pid.load(Ordering::Acquire) as i32);

        // PTRACE_GETREGS: Read all general-purpose registers
        // #ASSUME_PROCESS_STOPPED: Process must be stopped
        match ptrace::getregs(pid) {
            Ok(regs) => {
                self.last_result.store(0, Ordering::Release);
                self.update_timestamp();
                self.increment_operations();
                Ok(regs)
            }
            Err(e) => {
                self.record_error(e as i32);
                Err(e.into())
            }
        }
    }

    /// Write CPU registers (PTRACE_SETREGS)
    ///
    /// # Arguments
    /// * `regs` - Register values to write
    ///
    /// # Returns
    /// * `Ok(())` - Registers updated
    /// * `Err(PtraceError)` - Write failed
    ///
    /// # Safety
    /// #ASSUME_PROCESS_STOPPED: Process must be stopped for SETREGS
    /// #ASSUME_VALID_REGISTERS: Register values must be valid (no reserved bits set)
    ///
    /// # Performance
    /// Target: <2μs
    ///
    /// # Example
    /// ```no_run
    /// use kdb::ptrace::PtraceWrapperCapsule;
    /// let wrapper = PtraceWrapperCapsule::new();
    /// wrapper.attach(1234)?;
    /// let mut regs = wrapper.getregs()?;
    /// regs.rip += 1; // Skip instruction
    /// wrapper.setregs(&regs)?;
    /// # Ok::<(), kdb::ptrace::PtraceError>(())
    /// ```
    #[cfg(target_arch = "x86_64")]
    pub fn setregs(&self, regs: &libc::user_regs_struct) -> Result<(), PtraceError> {
        if !self.is_stopped() {
            return Err(PtraceError::ProcessNotStopped);
        }

        let pid = Pid::from_raw(self.pid.load(Ordering::Acquire) as i32);

        // PTRACE_SETREGS: Write all general-purpose registers
        // #ASSUME_PROCESS_STOPPED: Process must be stopped
        // #ASSUME_VALID_REGISTERS: Register values valid
        if let Err(e) = ptrace::setregs(pid, *regs) {
            self.record_error(e as i32);
            return Err(e.into());
        }

        self.last_result.store(0, Ordering::Release);
        self.update_timestamp();
        self.increment_operations();

        Ok(())
    }

    // ========================================================================
    // Signal Handling: Wait for Events
    // ========================================================================

    /// Wait for process to stop (blocking)
    ///
    /// # Returns
    /// * `Ok(WaitStatus)` - Process stopped due to signal, breakpoint, or exit
    /// * `Err(PtraceError)` - Wait failed
    ///
    /// # Blocking
    /// This call blocks until the process stops. Use in dedicated thread or async context.
    ///
    /// # State Transitions
    /// - Running → Stopped (on signal/breakpoint)
    /// - Running → Exited (on process exit)
    /// - Stepping → Stopped (after single-step)
    ///
    /// # Safety
    /// #ASSUME_PROCESS_RUNNING: Process must be running or stepping
    ///
    /// # Performance
    /// Blocking (typically <1ms for breakpoint, longer for signals)
    ///
    /// # Example
    /// ```no_run
    /// use kdb::ptrace::PtraceWrapperCapsule;
    /// use nix::sys::wait::WaitStatus;
    /// let wrapper = PtraceWrapperCapsule::new();
    /// wrapper.attach(1234)?;
    /// wrapper.cont()?;
    /// match wrapper.wait()? {
    ///     WaitStatus::Stopped(_, sig) => println!("Stopped by signal: {:?}", sig),
    ///     WaitStatus::Exited(_, code) => println!("Exited with code: {}", code),
    ///     _ => {}
    /// }
    /// # Ok::<(), kdb::ptrace::PtraceError>(())
    /// ```
    pub fn wait(&self) -> Result<WaitStatus, PtraceError> {
        let pid = Pid::from_raw(self.pid.load(Ordering::Acquire) as i32);
        if pid.as_raw() == 0 {
            return Err(PtraceError::NotAttached);
        }

        // waitpid: Block until process stops
        // #ASSUME_PROCESS_RUNNING: Process is running or stepping
        match waitpid(pid, None) {
            Ok(status) => {
                // Update state based on wait status
                match status {
                    WaitStatus::Stopped(_, sig) => {
                        self.set_state(ProcessState::Stopped);
                        self.last_signal.store(sig as i32 as u32, Ordering::Release);
                    }
                    WaitStatus::Exited(_, _) => {
                        self.set_state(ProcessState::Exited);
                    }
                    _ => {}
                }

                self.last_result.store(0, Ordering::Release);
                self.update_timestamp();
                self.increment_operations();

                Ok(status)
            }
            Err(e) => {
                self.record_error(e as i32);
                Err(PtraceError::WaitFailed)
            }
        }
    }

    // ========================================================================
    // Diagnostics & Monitoring
    // ========================================================================

    /// Get last signal received (0 if none)
    pub fn get_last_signal(&self) -> u32 {
        self.last_signal.load(Ordering::Relaxed)
    }

    /// Get error count
    pub fn get_error_count(&self) -> u64 {
        self.error_count.load(Ordering::Relaxed)
    }

    /// Get last operation timestamp (nanoseconds since UNIX epoch)
    pub fn get_last_operation_ns(&self) -> u64 {
        self.last_operation_ns.load(Ordering::Relaxed)
    }

    /// Get generation counter (for TOCTOU detection)
    pub fn get_generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }
}

impl Default for PtraceWrapperCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn test_size() {
        assert_eq!(
            size_of::<PtraceWrapperCapsule>(),
            256,
            "PtraceWrapperCapsule must be exactly 256 bytes"
        );
    }

    #[test]
    fn test_alignment() {
        assert_eq!(
            align_of::<PtraceWrapperCapsule>(),
            256,
            "PtraceWrapperCapsule must be 256-byte aligned"
        );
    }

    #[test]
    fn test_new() {
        let wrapper = PtraceWrapperCapsule::new();
        assert_eq!(wrapper.get_state(), ProcessState::Detached);
        assert_eq!(wrapper.get_pid(), 0);
        assert_eq!(wrapper.get_operation_count(), 0);
    }

    #[test]
    fn test_state_transitions() {
        let wrapper = PtraceWrapperCapsule::new();

        // Initial state
        assert_eq!(wrapper.get_state(), ProcessState::Detached);

        // Simulate state changes
        wrapper.set_state(ProcessState::Attaching);
        assert_eq!(wrapper.get_state(), ProcessState::Attaching);

        wrapper.set_state(ProcessState::Stopped);
        assert_eq!(wrapper.get_state(), ProcessState::Stopped);
        assert!(wrapper.is_stopped());

        wrapper.set_state(ProcessState::Running);
        assert_eq!(wrapper.get_state(), ProcessState::Running);
        assert!(!wrapper.is_stopped());
    }

    #[test]
    fn test_generation_counter() {
        let wrapper = PtraceWrapperCapsule::new();
        let gen1 = wrapper.get_generation();

        wrapper.set_state(ProcessState::Stopped);
        let gen2 = wrapper.get_generation();

        assert_eq!(gen2, gen1 + 1, "Generation counter must increment on state change");
    }

    #[test]
    fn test_error_handling() {
        let wrapper = PtraceWrapperCapsule::new();

        // Try to detach without attaching
        let result = wrapper.detach();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PtraceError::NotAttached);

        // Try to continue without being stopped
        let result = wrapper.cont();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), PtraceError::ProcessNotStopped);
    }

    #[test]
    fn test_invalid_pid() {
        let wrapper = PtraceWrapperCapsule::new();

        // Invalid PIDs
        assert_eq!(wrapper.attach(0).unwrap_err(), PtraceError::InvalidPid);
        assert_eq!(wrapper.attach(-1).unwrap_err(), PtraceError::InvalidPid);
    }

    // Note: Integration tests with real processes require root/CAP_SYS_PTRACE
    // and are in examples/integration_tests.rs
}
