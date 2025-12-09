//! Chaos Testing Infrastructure - Real Failure Injection
//!
//! Provides `ChaosInjector` for injecting real system failures:
//! - OOM via RLIMIT_AS
//! - FD exhaustion via RLIMIT_NOFILE
//! - Signal injection via kill()
//! - Process death via SIGKILL
//!
//! # Safety
//!
//! All modifications are reverted on Drop. Resource limits are stored
//! and restored to prevent test pollution.
//!
//! # ASSUM Tags
//!
//! #ASSUME_RLIMIT_RESTORABLE: Original limits can always be restored
//! #ASSUME_SIGNAL_DELIVERY: Signals are reliably delivered to target process
//! #ASSUME_FORK_SAFE: Fork operations are safe in single-threaded test context
//! #VERIFY_DROP_CLEANUP: Drop implementation restores all modified limits

use std::collections::HashMap;
use std::fmt;
use std::io;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};

use libc::{
    c_int, getrlimit, kill, pid_t, rlimit, rlim_t, setrlimit,
    SIGKILL, SIGUSR1,
};

// Define resource constants with correct types for this platform
#[cfg(target_os = "linux")]
const RLIMIT_AS_VAL: libc::__rlimit_resource_t = libc::RLIMIT_AS;
#[cfg(target_os = "linux")]
const RLIMIT_NOFILE_VAL: libc::__rlimit_resource_t = libc::RLIMIT_NOFILE;

// ============================================================================
// Error Types
// ============================================================================

/// Chaos injection error
#[derive(Debug)]
pub enum ChaosError {
    /// Failed to get resource limit
    GetRlimitFailed(io::Error),
    /// Failed to set resource limit
    SetRlimitFailed(io::Error),
    /// Failed to send signal
    SignalFailed(io::Error),
    /// Process not found
    ProcessNotFound(pid_t),
    /// Fork failed
    ForkFailed(io::Error),
    /// Invalid resource type
    InvalidResource,
    /// Already restored
    AlreadyRestored,
    /// Temporary directory creation failed
    TempDirFailed(io::Error),
}

impl fmt::Display for ChaosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChaosError::GetRlimitFailed(e) => write!(f, "Failed to get rlimit: {}", e),
            ChaosError::SetRlimitFailed(e) => write!(f, "Failed to set rlimit: {}", e),
            ChaosError::SignalFailed(e) => write!(f, "Failed to send signal: {}", e),
            ChaosError::ProcessNotFound(pid) => write!(f, "Process {} not found", pid),
            ChaosError::ForkFailed(e) => write!(f, "Fork failed: {}", e),
            ChaosError::InvalidResource => write!(f, "Invalid resource type"),
            ChaosError::AlreadyRestored => write!(f, "Limits already restored"),
            ChaosError::TempDirFailed(e) => write!(f, "Temp dir creation failed: {}", e),
        }
    }
}

impl std::error::Error for ChaosError {}

/// Result type for chaos operations
pub type ChaosResult<T> = Result<T, ChaosError>;

// ============================================================================
// Resource Type
// ============================================================================

/// Linux resource limits that can be modified
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Resource {
    /// Address space limit (RLIMIT_AS) - controls virtual memory
    AddressSpace,
    /// Open file descriptor limit (RLIMIT_NOFILE)
    OpenFiles,
}

impl Resource {
    /// Convert to libc resource constant
    #[cfg(target_os = "linux")]
    fn to_libc(self) -> libc::__rlimit_resource_t {
        match self {
            Resource::AddressSpace => RLIMIT_AS_VAL,
            Resource::OpenFiles => RLIMIT_NOFILE_VAL,
        }
    }
}

// ============================================================================
// Saved Limit
// ============================================================================

/// Saved resource limit for restoration
#[derive(Debug, Clone, Copy)]
struct SavedLimit {
    soft: rlim_t,
    hard: rlim_t,
}

impl SavedLimit {
    #[allow(dead_code)]
    fn from_rlimit(rl: &rlimit) -> Self {
        Self {
            soft: rl.rlim_cur,
            hard: rl.rlim_max,
        }
    }

    fn to_rlimit(&self) -> rlimit {
        rlimit {
            rlim_cur: self.soft,
            rlim_max: self.hard,
        }
    }
}

// ============================================================================
// ChaosInjector
// ============================================================================

/// Real chaos injector using Linux capabilities.
///
/// Injects real system failures for testing kdb resilience:
/// - OOM conditions via RLIMIT_AS
/// - FD exhaustion via RLIMIT_NOFILE
/// - Signal injection via kill()
/// - Process termination via SIGKILL
///
/// # Safety
///
/// All modifications are automatically reverted when the injector is dropped.
/// Original resource limits are stored and restored.
///
/// # Example
///
/// ```ignore
/// let mut injector = ChaosInjector::new();
///
/// // Inject FD exhaustion (limit to 10 file descriptors)
/// injector.inject_fd_exhaustion(10)?;
///
/// // Test code that should handle FD exhaustion gracefully
/// let result = some_function_that_opens_files();
///
/// // Limits are automatically restored on drop
/// drop(injector);
/// ```
pub struct ChaosInjector {
    /// Original resource limits for restoration
    original_limits: HashMap<Resource, SavedLimit>,
    /// Whether limits have been restored
    restored: AtomicBool,
    /// Spawned child processes (killed on drop)
    children: Vec<Child>,
}

impl ChaosInjector {
    /// Create a new chaos injector.
    ///
    /// The injector starts with no modifications. Call injection methods
    /// to introduce chaos conditions.
    pub fn new() -> Self {
        Self {
            original_limits: HashMap::new(),
            restored: AtomicBool::new(false),
            children: Vec::new(),
        }
    }

    /// Get current resource limit.
    ///
    /// Returns (soft_limit, hard_limit).
    fn get_rlimit(resource: Resource) -> ChaosResult<(rlim_t, rlim_t)> {
        let mut rl = rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };

        // SAFETY: getrlimit is safe with valid resource constant and pointer
        // #ASSUME_RLIMIT_RESTORABLE: Kernel always returns valid current limits
        let ret = unsafe { getrlimit(resource.to_libc(), &mut rl) };

        if ret == 0 {
            Ok((rl.rlim_cur, rl.rlim_max))
        } else {
            Err(ChaosError::GetRlimitFailed(io::Error::last_os_error()))
        }
    }

    /// Set resource limit (soft only, preserving hard).
    fn set_rlimit_soft(resource: Resource, soft: rlim_t) -> ChaosResult<()> {
        let (_, hard) = Self::get_rlimit(resource)?;

        // Cannot set soft higher than hard
        let effective_soft = soft.min(hard);

        let rl = rlimit {
            rlim_cur: effective_soft,
            rlim_max: hard,
        };

        // SAFETY: setrlimit is safe with valid resource constant and pointer
        let ret = unsafe { setrlimit(resource.to_libc(), &rl) };

        if ret == 0 {
            Ok(())
        } else {
            Err(ChaosError::SetRlimitFailed(io::Error::last_os_error()))
        }
    }

    /// Save current limit for later restoration.
    fn save_limit(&mut self, resource: Resource) -> ChaosResult<()> {
        if self.original_limits.contains_key(&resource) {
            // Already saved
            return Ok(());
        }

        let (soft, hard) = Self::get_rlimit(resource)?;
        self.original_limits.insert(
            resource,
            SavedLimit { soft, hard },
        );
        Ok(())
    }

    /// Inject OOM condition by limiting address space.
    ///
    /// Sets RLIMIT_AS to limit virtual memory allocation.
    /// This will cause malloc/mmap to fail when the limit is exceeded.
    ///
    /// # Arguments
    ///
    /// * `limit_mb` - Maximum address space in megabytes
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut injector = ChaosInjector::new();
    /// injector.inject_oom(100)?; // Limit to 100MB
    /// ```
    ///
    /// # Errors
    ///
    /// Returns error if getrlimit/setrlimit fails.
    pub fn inject_oom(&mut self, limit_mb: u64) -> ChaosResult<()> {
        self.save_limit(Resource::AddressSpace)?;

        let limit_bytes = limit_mb * 1024 * 1024;
        Self::set_rlimit_soft(Resource::AddressSpace, limit_bytes)?;

        println!(
            "[ChaosInjector] Injected OOM: RLIMIT_AS set to {} MB",
            limit_mb
        );

        Ok(())
    }

    /// Inject file descriptor exhaustion.
    ///
    /// Sets RLIMIT_NOFILE to limit open file descriptors.
    /// This will cause open() to fail with EMFILE when the limit is exceeded.
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of open file descriptors
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut injector = ChaosInjector::new();
    /// injector.inject_fd_exhaustion(10)?; // Limit to 10 FDs
    /// ```
    ///
    /// # Errors
    ///
    /// Returns error if getrlimit/setrlimit fails.
    pub fn inject_fd_exhaustion(&mut self, limit: u64) -> ChaosResult<()> {
        self.save_limit(Resource::OpenFiles)?;

        Self::set_rlimit_soft(Resource::OpenFiles, limit)?;

        println!(
            "[ChaosInjector] Injected FD exhaustion: RLIMIT_NOFILE set to {}",
            limit
        );

        Ok(())
    }

    /// Send signal to a process.
    ///
    /// Delivers the specified signal to the target process.
    ///
    /// # Arguments
    ///
    /// * `pid` - Target process ID
    /// * `signal` - Signal number (e.g., SIGKILL, SIGUSR1)
    ///
    /// # Common Signals
    ///
    /// - SIGKILL (9): Terminate immediately (cannot be caught)
    /// - SIGTERM (15): Graceful termination
    /// - SIGUSR1 (10): User-defined signal 1
    /// - SIGSTOP (19): Stop process (cannot be caught)
    ///
    /// # Errors
    ///
    /// Returns error if the signal cannot be delivered (e.g., permission denied,
    /// process not found).
    pub fn send_signal(pid: pid_t, signal: c_int) -> ChaosResult<()> {
        // SAFETY: kill() is safe with valid pid and signal
        // #ASSUME_SIGNAL_DELIVERY: Kernel will deliver or return error
        let ret = unsafe { kill(pid, signal) };

        if ret == 0 {
            Ok(())
        } else {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::ESRCH) {
                Err(ChaosError::ProcessNotFound(pid))
            } else {
                Err(ChaosError::SignalFailed(err))
            }
        }
    }

    /// Kill a process with SIGKILL.
    ///
    /// Convenience method for `send_signal(pid, SIGKILL)`.
    pub fn kill_process(pid: pid_t) -> ChaosResult<()> {
        Self::send_signal(pid, SIGKILL)
    }

    /// Send SIGUSR1 to a process (non-fatal signal flood testing).
    pub fn send_sigusr1(pid: pid_t) -> ChaosResult<()> {
        Self::send_signal(pid, SIGUSR1)
    }

    /// Spawn a sleep process for testing.
    ///
    /// Spawns `sleep infinity` and returns the child process.
    /// The process is automatically killed when the injector is dropped.
    ///
    /// # Returns
    ///
    /// The PID of the spawned process.
    pub fn spawn_sleep_target(&mut self) -> ChaosResult<pid_t> {
        let child = Command::new("sleep")
            .arg("infinity")
            .spawn()
            .map_err(ChaosError::ForkFailed)?;

        let pid = child.id() as pid_t;
        self.children.push(child);

        // Small delay to ensure process is fully started
        std::thread::sleep(std::time::Duration::from_millis(10));

        Ok(pid)
    }

    /// Spawn a process that can be debugged.
    ///
    /// Spawns a simple loop program for ptrace testing.
    /// The process is automatically killed when the injector is dropped.
    ///
    /// # Returns
    ///
    /// The PID of the spawned process.
    pub fn spawn_debuggable_target(&mut self) -> ChaosResult<pid_t> {
        // Use 'yes' as a simple busy-wait target
        let child = Command::new("yes")
            .stdout(std::process::Stdio::null())
            .spawn()
            .map_err(ChaosError::ForkFailed)?;

        let pid = child.id() as pid_t;
        self.children.push(child);

        // Small delay to ensure process is fully started
        std::thread::sleep(std::time::Duration::from_millis(10));

        Ok(pid)
    }

    /// Check if a process is still alive.
    pub fn is_process_alive(pid: pid_t) -> bool {
        // Send signal 0 to check if process exists
        // SAFETY: kill(pid, 0) just checks existence
        unsafe { kill(pid, 0) == 0 }
    }

    /// Wait for a process to exit (with timeout).
    ///
    /// Returns true if the process exited, false if timeout.
    /// Uses waitpid with WNOHANG for non-blocking checks.
    pub fn wait_for_exit(pid: pid_t, timeout_ms: u64) -> bool {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);

        while start.elapsed() < timeout {
            // Try waitpid first (for child processes we spawned)
            // SAFETY: waitpid with WNOHANG is safe, returns immediately
            let wait_result = unsafe {
                let mut status: c_int = 0;
                libc::waitpid(pid, &mut status, libc::WNOHANG)
            };

            if wait_result == pid {
                // Process was reaped
                return true;
            } else if wait_result == -1 {
                // Error - check if process doesn't exist (ECHILD)
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::ECHILD) {
                    // No child to wait for - either not our child or already reaped
                    // Fall back to kill check
                    if !Self::is_process_alive(pid) {
                        return true;
                    }
                }
            }

            // Also check with kill(pid, 0)
            if !Self::is_process_alive(pid) {
                return true;
            }

            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        false
    }

    /// Restore all modified resource limits.
    ///
    /// This is automatically called on drop, but can be called manually.
    /// Calling multiple times is safe (no-op after first call).
    pub fn restore(&mut self) -> ChaosResult<()> {
        if self.restored.swap(true, Ordering::SeqCst) {
            return Ok(()); // Already restored
        }

        let mut errors = Vec::new();

        for (resource, saved) in &self.original_limits {
            let rl = saved.to_rlimit();

            // SAFETY: setrlimit is safe with valid resource constant and pointer
            // #VERIFY_DROP_CLEANUP: This restores limits saved in save_limit()
            let ret = unsafe { setrlimit(resource.to_libc(), &rl) };

            if ret != 0 {
                errors.push((*resource, io::Error::last_os_error()));
            } else {
                println!(
                    "[ChaosInjector] Restored {:?} to soft={}, hard={}",
                    resource, saved.soft, saved.hard
                );
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            // Report first error
            Err(ChaosError::SetRlimitFailed(errors[0].1.kind().into()))
        }
    }

    /// Kill all spawned child processes.
    fn cleanup_children(&mut self) {
        for child in &mut self.children {
            let pid = child.id() as pid_t;

            // Try graceful termination first
            let _ = Self::send_signal(pid, libc::SIGTERM);
            std::thread::sleep(std::time::Duration::from_millis(50));

            // Force kill if still running
            if Self::is_process_alive(pid) {
                let _ = Self::kill_process(pid);
            }

            // Wait to avoid zombie
            let _ = child.wait();
        }
        self.children.clear();
    }
}

impl Default for ChaosInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ChaosInjector {
    fn drop(&mut self) {
        // Kill spawned children first
        self.cleanup_children();

        // Then restore resource limits
        if let Err(e) = self.restore() {
            eprintln!("[ChaosInjector] Warning: Failed to restore limits: {}", e);
        }
    }
}

// ============================================================================
// Fork Helper (for tests that need child processes)
// ============================================================================

/// Result of a forked operation.
#[derive(Debug)]
pub enum ForkResult {
    /// Parent process, with child PID
    Parent(pid_t),
    /// Child process
    Child,
}

/// Fork the current process.
///
/// # Safety
///
/// This is unsafe because fork() in a multi-threaded program can lead to
/// deadlocks if any thread holds a lock. Only use in test context where
/// the process is effectively single-threaded.
///
/// # Returns
///
/// - `ForkResult::Parent(child_pid)` in the parent process
/// - `ForkResult::Child` in the child process
///
/// # Errors
///
/// Returns error if fork() fails.
///
/// #ASSUME_FORK_SAFE: Tests run single-threaded, fork is safe
#[cfg(target_os = "linux")]
pub unsafe fn fork() -> ChaosResult<ForkResult> {
    let pid = libc::fork();

    match pid {
        -1 => Err(ChaosError::ForkFailed(io::Error::last_os_error())),
        0 => Ok(ForkResult::Child),
        child_pid => Ok(ForkResult::Parent(child_pid)),
    }
}

// ============================================================================
// Unit Tests for Infrastructure
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chaos_injector_creation() {
        let injector = ChaosInjector::new();
        assert!(injector.original_limits.is_empty());
        assert!(!injector.restored.load(Ordering::Relaxed));
    }

    #[test]
    fn test_get_rlimit() {
        // Should be able to read current limits
        let result = ChaosInjector::get_rlimit(Resource::OpenFiles);
        assert!(result.is_ok());

        let (soft, hard) = result.unwrap();
        assert!(soft > 0, "Soft limit should be positive");
        assert!(hard > 0, "Hard limit should be positive");
        assert!(soft <= hard, "Soft should not exceed hard limit");
    }

    #[test]
    fn test_fd_exhaustion_restore() {
        let mut injector = ChaosInjector::new();

        // Get original limit
        let (original_soft, _) = ChaosInjector::get_rlimit(Resource::OpenFiles).unwrap();

        // Inject FD exhaustion
        let test_limit = 20;
        injector.inject_fd_exhaustion(test_limit).unwrap();

        // Verify limit was changed
        let (new_soft, _) = ChaosInjector::get_rlimit(Resource::OpenFiles).unwrap();
        assert_eq!(new_soft, test_limit, "Limit should be set to test value");

        // Restore
        injector.restore().unwrap();

        // Verify restoration
        let (restored_soft, _) = ChaosInjector::get_rlimit(Resource::OpenFiles).unwrap();
        assert_eq!(restored_soft, original_soft, "Limit should be restored");
    }

    #[test]
    fn test_spawn_and_check_alive() {
        let mut injector = ChaosInjector::new();

        let pid = injector.spawn_sleep_target().unwrap();
        assert!(pid > 0, "PID should be positive");

        // Process should be alive
        assert!(ChaosInjector::is_process_alive(pid), "Process should be alive");

        // Kill it
        ChaosInjector::kill_process(pid).unwrap();

        // Wait for exit (with longer timeout for slow CI systems)
        // SIGKILL cannot be caught, so process WILL die
        assert!(
            ChaosInjector::wait_for_exit(pid, 5000),
            "Process should exit after SIGKILL (timeout 5s)"
        );

        // Should no longer be alive
        assert!(
            !ChaosInjector::is_process_alive(pid),
            "Process should be dead after SIGKILL"
        );
    }

    #[test]
    fn test_send_signal_to_nonexistent() {
        // Use a very high PID that shouldn't exist (max valid PID is ~4M on most systems)
        // PID_MAX is typically 32768 or 4194304
        let result = ChaosInjector::send_signal(999_999_999, libc::SIGTERM);
        assert!(matches!(result, Err(ChaosError::ProcessNotFound(_))));
    }

    #[test]
    fn test_drop_cleanup() {
        let mut injector = ChaosInjector::new();

        // Spawn a child
        let pid = injector.spawn_sleep_target().unwrap();
        assert!(ChaosInjector::is_process_alive(pid));

        // Inject a limit change
        injector.inject_fd_exhaustion(50).unwrap();

        // Drop the injector
        drop(injector);

        // Process should be killed
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(
            !ChaosInjector::is_process_alive(pid),
            "Child process should be killed on drop"
        );
    }

    #[test]
    fn test_multiple_restore_calls() {
        let mut injector = ChaosInjector::new();
        injector.inject_fd_exhaustion(30).unwrap();

        // First restore should succeed
        assert!(injector.restore().is_ok());

        // Second restore should be no-op
        assert!(injector.restore().is_ok());
    }
}
