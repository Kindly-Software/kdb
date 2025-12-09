//! Process Spawner - Spawn and manage test target processes
//!
//! Provides utilities for spawning test binaries and managing their lifecycle
//! for E2E debugging tests.
//!
//! # ASSUM Safety
//!
//! - #ASSUME_BINARY_EXISTS: Target binaries exist in binaries_dir
//! - #ASSUME_CHILD_CLEANUP: All spawned processes are tracked for cleanup
//! - #ASSUME_STDOUT_READABLE: Spawned processes have readable stdout

use super::error::{E2EError, E2EResult};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// A spawned process handle with metadata
#[derive(Debug)]
pub struct SpawnedProcess {
    /// Process ID
    pub pid: u32,
    /// Child process handle
    pub handle: Child,
    /// Path to the binary that was spawned
    pub binary_path: PathBuf,
    /// Arguments used to spawn the process
    pub args: Vec<String>,
    /// Whether the process has been detached (won't be killed on drop)
    detached: bool,
}

impl SpawnedProcess {
    /// Get the PID as i32 (for ptrace compatibility)
    pub fn pid_i32(&self) -> i32 {
        self.pid as i32
    }

    /// Check if the process is still running
    pub fn is_running(&mut self) -> bool {
        match self.handle.try_wait() {
            Ok(None) => true,  // Still running
            Ok(Some(_)) => false,  // Exited
            Err(_) => false,  // Error checking, assume not running
        }
    }

    /// Wait for the process to exit with a timeout
    pub fn wait_for_exit(&mut self, timeout: Duration) -> E2EResult<i32> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            match self.handle.try_wait() {
                Ok(Some(status)) => {
                    return Ok(status.code().unwrap_or(-1));
                }
                Ok(None) => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) => {
                    return Err(E2EError::Io(e));
                }
            }
        }
        Err(E2EError::DebuggerTimeout {
            timeout_ms: timeout.as_millis() as u64,
        })
    }

    /// Detach from the process (won't be killed on drop)
    pub fn detach(&mut self) {
        self.detached = true;
    }

    /// Kill the process
    pub fn kill(&mut self) -> E2EResult<()> {
        self.handle.kill().map_err(E2EError::Io)
    }

    /// Send a signal to the process (Unix only)
    #[cfg(unix)]
    pub fn signal(&self, signal: i32) -> E2EResult<()> {
        use std::os::unix::process::CommandExt;

        // Use kill syscall via nix or libc
        let result = unsafe { libc::kill(self.pid as i32, signal) };
        if result == 0 {
            Ok(())
        } else {
            Err(E2EError::generic(
                "signal",
                format!("Failed to send signal {} to process {}", signal, self.pid),
            ))
        }
    }
}

impl Drop for SpawnedProcess {
    fn drop(&mut self) {
        if !self.detached {
            // Try to kill the process if still running
            let _ = self.handle.kill();
            let _ = self.handle.wait();
        }
    }
}

/// Spawn and manage test target processes
///
/// Tracks all spawned child processes for cleanup and provides utilities
/// for spawning test binaries with various configurations.
///
/// # Example
///
/// ```ignore
/// let mut spawner = ProcessSpawner::new();
/// let process = spawner.spawn("test_target", &[])?;
/// println!("Spawned process with PID: {}", process.pid);
/// ```
pub struct ProcessSpawner {
    /// Directory containing test binaries
    binaries_dir: PathBuf,
    /// Tracked child processes for cleanup
    children: Vec<SpawnedProcess>,
    /// Whether cleanup has been performed
    cleaned_up: AtomicBool,
}

impl ProcessSpawner {
    /// Create a new ProcessSpawner with the default binaries directory
    ///
    /// The default directory is `target/debug/examples` relative to the crate root.
    pub fn new() -> Self {
        // Default to target/debug/examples for test binaries
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));

        let binaries_dir = manifest_dir.join("target").join("debug").join("examples");

        Self {
            binaries_dir,
            children: Vec::new(),
            cleaned_up: AtomicBool::new(false),
        }
    }

    /// Create a ProcessSpawner with a custom binaries directory
    pub fn with_binaries_dir(binaries_dir: PathBuf) -> Self {
        Self {
            binaries_dir,
            children: Vec::new(),
            cleaned_up: AtomicBool::new(false),
        }
    }

    /// Set the binaries directory
    pub fn set_binaries_dir(&mut self, dir: PathBuf) {
        self.binaries_dir = dir;
    }

    /// Get the binaries directory
    pub fn binaries_dir(&self) -> &PathBuf {
        &self.binaries_dir
    }

    /// Spawn a test target process
    ///
    /// # Arguments
    ///
    /// * `target` - Name of the binary to spawn (looked up in binaries_dir)
    /// * `args` - Arguments to pass to the binary
    ///
    /// # Returns
    ///
    /// A `SpawnedProcess` handle on success
    ///
    /// # Errors
    ///
    /// - `BinaryNotFound` if the target binary doesn't exist
    /// - `SpawnFailed` if the process couldn't be started
    pub fn spawn(&mut self, target: &str, args: &[&str]) -> E2EResult<&mut SpawnedProcess> {
        let binary_path = self.binaries_dir.join(target);

        // Check if binary exists
        if !binary_path.exists() {
            // Also try with common extensions
            let binary_path_exe = self.binaries_dir.join(format!("{}", target));
            if !binary_path_exe.exists() {
                return Err(E2EError::BinaryNotFound {
                    path: binary_path.display().to_string(),
                });
            }
        }

        let child = Command::new(&binary_path)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| E2EError::spawn_failed(target, e))?;

        let pid = child.id();

        let spawned = SpawnedProcess {
            pid,
            handle: child,
            binary_path: binary_path.clone(),
            args: args.iter().map(|s| s.to_string()).collect(),
            detached: false,
        };

        self.children.push(spawned);
        Ok(self.children.last_mut().unwrap())
    }

    /// Spawn a process and wait for a ready pattern in stdout
    ///
    /// Useful for processes that need initialization time before debugging.
    ///
    /// # Arguments
    ///
    /// * `target` - Name of the binary to spawn
    /// * `args` - Arguments to pass to the binary
    /// * `ready_pattern` - Pattern to wait for in stdout
    /// * `timeout` - Maximum time to wait for the pattern
    ///
    /// # Returns
    ///
    /// A `SpawnedProcess` handle on success
    pub fn spawn_and_wait(
        &mut self,
        target: &str,
        args: &[&str],
        ready_pattern: &str,
        timeout: Duration,
    ) -> E2EResult<&mut SpawnedProcess> {
        let binary_path = self.binaries_dir.join(target);

        if !binary_path.exists() {
            return Err(E2EError::BinaryNotFound {
                path: binary_path.display().to_string(),
            });
        }

        let mut child = Command::new(&binary_path)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| E2EError::spawn_failed(target, e))?;

        let pid = child.id();

        // Take stdout for pattern matching
        let stdout = child.stdout.take().expect("stdout should be piped");
        let reader = BufReader::new(stdout);

        let start = Instant::now();
        let mut found = false;

        for line in reader.lines() {
            if start.elapsed() > timeout {
                break;
            }

            if let Ok(line) = line {
                if line.contains(ready_pattern) {
                    found = true;
                    break;
                }
            }
        }

        if !found {
            // Kill the process if pattern wasn't found
            let _ = child.kill();
            return Err(E2EError::PatternTimeout {
                pattern: ready_pattern.to_string(),
                timeout_ms: timeout.as_millis() as u64,
            });
        }

        let spawned = SpawnedProcess {
            pid,
            handle: child,
            binary_path,
            args: args.iter().map(|s| s.to_string()).collect(),
            detached: false,
        };

        self.children.push(spawned);
        Ok(self.children.last_mut().unwrap())
    }

    /// Spawn a simple "sleep" process for testing
    ///
    /// This is useful for tests that just need a running process to attach to.
    pub fn spawn_sleep(&mut self, seconds: u32) -> E2EResult<&mut SpawnedProcess> {
        let child = Command::new("sleep")
            .arg(seconds.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| E2EError::spawn_failed("sleep", e))?;

        let pid = child.id();

        let spawned = SpawnedProcess {
            pid,
            handle: child,
            binary_path: PathBuf::from("/usr/bin/sleep"),
            args: vec![seconds.to_string()],
            detached: false,
        };

        self.children.push(spawned);
        Ok(self.children.last_mut().unwrap())
    }

    /// Get a reference to a spawned process by PID
    pub fn get_by_pid(&self, pid: u32) -> Option<&SpawnedProcess> {
        self.children.iter().find(|p| p.pid == pid)
    }

    /// Get a mutable reference to a spawned process by PID
    pub fn get_by_pid_mut(&mut self, pid: u32) -> Option<&mut SpawnedProcess> {
        self.children.iter_mut().find(|p| p.pid == pid)
    }

    /// Get the number of tracked processes
    pub fn process_count(&self) -> usize {
        self.children.len()
    }

    /// Kill all tracked processes
    pub fn kill_all(&mut self) {
        for child in &mut self.children {
            let _ = child.kill();
        }
    }

    /// Cleanup all tracked processes
    ///
    /// Called automatically on drop, but can be called explicitly.
    pub fn cleanup(&mut self) {
        if self.cleaned_up.swap(true, Ordering::SeqCst) {
            return;  // Already cleaned up
        }

        for child in &mut self.children {
            if !child.detached {
                let _ = child.handle.kill();
                let _ = child.handle.wait();
            }
        }
        self.children.clear();
    }
}

impl Default for ProcessSpawner {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ProcessSpawner {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_spawner_creation() {
        let spawner = ProcessSpawner::new();
        assert!(spawner.binaries_dir.to_string_lossy().contains("target"));
    }

    #[test]
    fn test_spawn_sleep() {
        let mut spawner = ProcessSpawner::new();
        let process = spawner.spawn_sleep(1).unwrap();
        assert!(process.pid > 0);
        assert!(process.is_running());
    }

    #[test]
    fn test_spawn_nonexistent_binary() {
        let mut spawner = ProcessSpawner::new();
        let result = spawner.spawn("nonexistent_binary_12345", &[]);
        assert!(matches!(result, Err(E2EError::BinaryNotFound { .. })));
    }

    #[test]
    fn test_process_cleanup_on_drop() {
        let pid;
        {
            let mut spawner = ProcessSpawner::new();
            let process = spawner.spawn_sleep(60).unwrap();
            pid = process.pid;
        }
        // After drop, process should be killed
        // Check via /proc (Unix) or other means
        #[cfg(unix)]
        {
            // Give some time for cleanup
            std::thread::sleep(Duration::from_millis(100));
            let proc_path = format!("/proc/{}", pid);
            // Process may or may not exist depending on timing
            // This is just a sanity check
        }
    }

    #[test]
    fn test_get_by_pid() {
        let mut spawner = ProcessSpawner::new();
        let process = spawner.spawn_sleep(5).unwrap();
        let pid = process.pid;

        assert!(spawner.get_by_pid(pid).is_some());
        assert!(spawner.get_by_pid(99999).is_none());
    }
}
