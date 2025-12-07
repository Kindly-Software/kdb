//! Security Validation Module
//!
//! Provides comprehensive security checks for MCP server operations.
//! Prevents privilege escalation and validates all external inputs.

use std::io;

/// Error types for security validation
#[derive(Debug)]
pub enum SecurityError {
    /// Invalid PID (zero, negative, or out of range)
    InvalidPid(i32),

    /// Process does not exist
    ProcessNotFound(i32),

    /// Permission denied (UID mismatch without CAP_SYS_PTRACE)
    PermissionDenied { pid: i32, reason: String },

    /// Protected system process (kernel, init)
    ProtectedProcess(i32),

    /// Process already being traced
    AlreadyAttached(i32),

    /// I/O error reading /proc
    ProcError(io::Error),
}

impl std::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurityError::InvalidPid(pid) => write!(f, "Invalid PID: {}", pid),
            SecurityError::ProcessNotFound(pid) => write!(f, "Process not found: {}", pid),
            SecurityError::PermissionDenied { pid, reason } => {
                write!(f, "Permission denied for PID {}: {}", pid, reason)
            }
            SecurityError::ProtectedProcess(pid) => {
                write!(f, "Protected system process: {}", pid)
            }
            SecurityError::AlreadyAttached(pid) => {
                write!(f, "Process already being traced: {}", pid)
            }
            SecurityError::ProcError(err) => write!(f, "Proc error: {}", err),
        }
    }
}

impl std::error::Error for SecurityError {}

impl From<io::Error> for SecurityError {
    fn from(err: io::Error) -> Self {
        SecurityError::ProcError(err)
    }
}

/// Protected PIDs that cannot be attached to
const PROTECTED_PIDS: &[i32] = &[
    0,  // Kernel scheduler
    1,  // init/systemd
];

/// Linux capability: CAP_SYS_PTRACE (bit 19)
const CAP_SYS_PTRACE: u64 = 1u64 << 19;

/// Simple PID validation wrapper for tests (returns bool instead of Result)
///
/// Returns true if PID is valid for attach, false otherwise.
pub fn validate_pid(pid: i32) -> bool {
    validate_pid_attach(pid).is_ok()
}

/// Validate PID before allowing attach operation
///
/// Checks:
/// 1. Basic range validation (pid > 0)
/// 2. Process exists (/proc/{pid} exists)
/// 3. UID validation (same user or CAP_SYS_PTRACE)
/// 4. Protected process blacklist
/// 5. Not already being traced
///
/// # Security
///
/// #ASSUME_UID_SUFFICIENT: UID matching is sufficient for same-user attach
/// #VERIFY: Test with different UIDs, validate rejection
///
/// #ASSUME_PROC_EXISTS: /proc/{pid} existence means process is alive
/// #VERIFY: Test with stale PIDs, race conditions
///
/// #ASSUME_CAPABILITY_ACCURATE: CapEff reflects current capabilities
/// #VERIFY: Test with CAP_SYS_PTRACE set/unset
pub fn validate_pid_attach(pid: i32) -> Result<(), SecurityError> {
    // 1. Blacklist critical system processes FIRST (PID 0 and 1 are special)
    if PROTECTED_PIDS.contains(&pid) {
        return Err(SecurityError::ProtectedProcess(pid));
    }

    // 2. Basic range check (negative PIDs only, 0 handled above)
    if pid < 0 {
        return Err(SecurityError::InvalidPid(pid));
    }

    // 3. Check PID exists
    let proc_path = format!("/proc/{}", pid);
    if !std::path::Path::new(&proc_path).exists() {
        return Err(SecurityError::ProcessNotFound(pid));
    }

    // 4. UID validation (can only attach to own processes or with CAP_SYS_PTRACE)
    let proc_uid = get_process_uid(pid)?;
    let my_uid = unsafe { libc::getuid() };

    if proc_uid != my_uid {
        // Check if we have CAP_SYS_PTRACE
        if !has_capability(CAP_SYS_PTRACE)? {
            return Err(SecurityError::PermissionDenied {
                pid,
                reason: format!(
                    "Cannot attach to other user's process (UID {} vs {}). Requires CAP_SYS_PTRACE.",
                    my_uid, proc_uid
                ),
            });
        }
    }

    // 5. Check if already being traced
    if is_already_traced(pid)? {
        return Err(SecurityError::AlreadyAttached(pid));
    }

    Ok(())
}

/// Get the real UID of a process by reading /proc/{pid}/status
///
/// #ASSUME_STATUS_FORMAT: /proc/{pid}/status format is stable
/// #VERIFY: Test on Ubuntu 22.04+, kernel 5.15+
fn get_process_uid(pid: i32) -> Result<u32, io::Error> {
    let status = std::fs::read_to_string(format!("/proc/{}/status", pid))?;

    // Parse "Uid: 1000 1000 1000 1000" line (real, effective, saved, filesystem)
    for line in status.lines() {
        if line.starts_with("Uid:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                return parts[1]
                    .parse::<u32>()
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UID"));
            }
        }
    }

    Err(io::Error::new(io::ErrorKind::NotFound, "UID not found in /proc/status"))
}

/// Check if current process has a specific capability
///
/// Reads /proc/self/status and parses CapEff (effective capabilities) line.
///
/// #ASSUME_CAPEFF_BITMASK: CapEff is a hexadecimal bitmask of capabilities
/// #VERIFY: Test with CAP_SYS_PTRACE set (via setcap or root)
fn has_capability(cap: u64) -> Result<bool, io::Error> {
    let status = std::fs::read_to_string("/proc/self/status")?;

    // Parse "CapEff: 0000000000000000" line (hexadecimal)
    for line in status.lines() {
        if line.starts_with("CapEff:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let cap_eff = u64::from_str_radix(parts[1], 16)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid CapEff"))?;

                return Ok((cap_eff & cap) != 0);
            }
        }
    }

    Err(io::Error::new(io::ErrorKind::NotFound, "CapEff not found in /proc/status"))
}

/// Check if a process is already being traced by another debugger
///
/// Reads /proc/{pid}/status and checks TracerPid field.
/// TracerPid: 0 = not traced, >0 = PID of tracer
///
/// #ASSUME_TRACERPID_ACCURATE: TracerPid reflects current ptrace state
/// #VERIFY: Test with GDB attached, validate detection
fn is_already_traced(pid: i32) -> Result<bool, io::Error> {
    let status = std::fs::read_to_string(format!("/proc/{}/status", pid))?;

    // Parse "TracerPid: 0" line
    for line in status.lines() {
        if line.starts_with("TracerPid:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let tracer_pid = parts[1]
                    .parse::<i32>()
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid TracerPid"))?;

                return Ok(tracer_pid != 0);
            }
        }
    }

    Err(io::Error::new(io::ErrorKind::NotFound, "TracerPid not found in /proc/status"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_negative_pid() {
        let result = validate_pid_attach(-1);
        assert!(matches!(result, Err(SecurityError::InvalidPid(-1))));
    }

    #[test]
    fn test_validate_zero_pid() {
        let result = validate_pid_attach(0);
        assert!(matches!(result, Err(SecurityError::ProtectedProcess(0))));
    }

    #[test]
    fn test_validate_init_pid() {
        let result = validate_pid_attach(1);
        assert!(matches!(result, Err(SecurityError::ProtectedProcess(1))));
    }

    #[test]
    fn test_validate_nonexistent_pid() {
        let result = validate_pid_attach(999999);
        assert!(matches!(result, Err(SecurityError::ProcessNotFound(999999))));
    }

    #[test]
    fn test_validate_self_pid() {
        // Should succeed (same UID)
        let pid = std::process::id() as i32;
        let result = validate_pid_attach(pid);
        assert!(result.is_ok(), "Should allow attaching to own process: {:?}", result);
    }

    #[test]
    fn test_get_process_uid_self() {
        let pid = std::process::id() as i32;
        let uid = get_process_uid(pid).expect("Should read own UID");
        let expected_uid = unsafe { libc::getuid() };
        assert_eq!(uid, expected_uid, "UID mismatch");
    }

    #[test]
    fn test_has_capability() {
        // Non-root process should not have CAP_SYS_PTRACE
        let has_ptrace = has_capability(CAP_SYS_PTRACE)
            .expect("Should read capabilities");

        if unsafe { libc::getuid() } != 0 {
            assert!(!has_ptrace, "Non-root should not have CAP_SYS_PTRACE");
        }
    }

    #[test]
    fn test_is_already_traced_self() {
        let pid = std::process::id() as i32;
        let traced = is_already_traced(pid).expect("Should read TracerPid");
        assert!(!traced, "Test process should not be traced");
    }
}
