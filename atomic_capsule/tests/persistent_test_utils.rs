//! Common Test Utilities for T9 Persistent Capsule
//!
//! # T28 Framework Support
//!
//! Provides utilities for:
//! - Temporary file creation (isolated tests)
//! - Mmap setup/teardown (memory-mapped files)
//! - Alignment verification (hardware requirements)
//! - Process simulation (multi-process tests)
//! - Consistency checks (crash recovery)

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tempfile::TempDir;

// ============================================================================
// TEMPORARY FILE MANAGEMENT
// ============================================================================

/// Create isolated temporary file for testing
///
/// # Returns
///
/// Tuple of (TempDir, PathBuf) - keep TempDir alive to prevent deletion
///
/// # Example
///
/// ```rust,ignore
/// let (dir, path) = create_temp_file("test.mmap");
/// // Use path...
/// // dir cleanup on drop
/// ```
pub fn create_temp_file(name: &str) -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    let path = dir.path().join(name);
    (dir, path)
}

/// Create temporary file with specific size
///
/// # Arguments
///
/// * `name` - File name
/// * `size_bytes` - Size to preallocate
///
/// # Returns
///
/// Tuple of (TempDir, PathBuf, File)
pub fn create_temp_file_with_size(name: &str, size_bytes: u64) -> (TempDir, PathBuf, File) {
    let (dir, path) = create_temp_file(name);

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&path)
        .expect("Failed to create file");

    file.set_len(size_bytes).expect("Failed to set file size");

    (dir, path, file)
}

// ============================================================================
// MMAP CAPSULE SETUP
// ============================================================================

/// Create persistent atomic capsule for testing
///
/// # Performance
///
/// <100ns initialization
#[cfg(all(feature = "mmap-persistence", feature = "nightly-atomic"))]
pub fn create_persistent_atomic<T>() -> (TempDir, PathBuf, atomic_capsule::persistence::MmapManager)
{
    use atomic_capsule::persistence::{MmapLayout, MmapManager};

    let (dir, path) = create_temp_file("persistent_atomic.mmap");

    // Create layout: 4KB page, 1 region
    let layout = MmapLayout::new(4096, 1).expect("Failed to create layout");

    // Create manager
    let manager = MmapManager::new(&path, &layout).expect("Failed to create mmap");

    (dir, path, manager)
}

// ============================================================================
// ALIGNMENT VERIFICATION
// ============================================================================

/// Verify that offset is aligned to requirement
///
/// # Returns
///
/// `Ok(())` if aligned, `Err(&str)` with message if not
pub fn verify_alignment(offset: usize, required: usize) -> Result<(), String> {
    if offset % required != 0 {
        return Err(format!(
            "Misaligned: offset={} (required {} alignment)",
            offset, required
        ));
    }
    Ok(())
}

/// Verify that pointer is aligned
pub fn verify_ptr_alignment<T>(ptr: *const T, required: usize) -> Result<(), String> {
    let addr = ptr as usize;
    verify_alignment(addr, required)
}

// ============================================================================
// PROCESS SIMULATION (Multi-Process Tests)
// ============================================================================

/// Spawn child process to simulate multi-process access
///
/// # Arguments
///
/// * `binary` - Path to test binary
/// * `args` - Command-line arguments
///
/// # Returns
///
/// Child process handle
pub fn spawn_test_process(binary: &str, args: &[&str]) -> std::io::Result<Child> {
    Command::new(binary).args(args).spawn()
}

/// Kill process to simulate crash
///
/// # Safety
///
/// Sends SIGKILL on Unix, TerminateProcess on Windows
pub fn kill_process(mut child: Child) -> std::io::Result<()> {
    child.kill()
}

// ============================================================================
// CONSISTENCY CHECKS
// ============================================================================

/// Wait for async flush to complete
///
/// # Arguments
///
/// * `max_wait_ms` - Maximum time to wait (milliseconds)
pub fn wait_for_consistency(max_wait_ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(max_wait_ms));
}

/// Atomic counter for test coordination
pub struct TestCounter {
    value: Arc<AtomicU64>,
}

impl TestCounter {
    /// Create new test counter
    pub fn new() -> Self {
        Self {
            value: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Increment counter
    pub fn increment(&self) -> u64 {
        self.value.fetch_add(1, Ordering::SeqCst)
    }

    /// Get current value
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::SeqCst)
    }

    /// Clone for multi-thread access
    pub fn clone(&self) -> Self {
        Self {
            value: Arc::clone(&self.value),
        }
    }
}

impl Default for TestCounter {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// HASH VERIFICATION
// ============================================================================

/// Compute FNV-1a hash for test verification
///
/// Matches PersistentAtomic::compute_hash() implementation
pub fn compute_test_hash(value: u64, generation: u64, timestamp: u64) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;

    // Hash value (8 bytes)
    for &byte in &value.to_le_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    // Hash generation (8 bytes)
    for &byte in &generation.to_le_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    // Hash timestamp (8 bytes)
    for &byte in &timestamp.to_le_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    hash
}

// ============================================================================
// FILE CORRUPTION SIMULATION
// ============================================================================

/// Corrupt byte at specific offset (for corruption tests)
///
/// # Safety
///
/// Intentionally corrupts file for testing recovery
pub fn corrupt_file_at_offset(path: &Path, offset: u64, corrupt_byte: u8) -> std::io::Result<()> {
    use std::io::{Seek, SeekFrom};

    let mut file = OpenOptions::new().write(true).open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    file.write_all(&[corrupt_byte])?;
    file.sync_all()?;
    Ok(())
}

// ============================================================================
// BENCHMARKING HELPERS
// ============================================================================

/// Measure operation latency (B32 compliance)
///
/// # Returns
///
/// Average latency in nanoseconds over N iterations
pub fn measure_latency<F>(mut operation: F, iterations: usize) -> u64
where
    F: FnMut(),
{
    let start = std::time::Instant::now();

    for _ in 0..iterations {
        operation();
    }

    let elapsed = start.elapsed();
    elapsed.as_nanos() as u64 / iterations as u64
}

/// Measure throughput (operations per second)
pub fn measure_throughput<F>(mut operation: F, duration_secs: u64) -> u64
where
    F: FnMut(),
{
    let start = std::time::Instant::now();
    let mut count = 0u64;

    while start.elapsed().as_secs() < duration_secs {
        operation();
        count += 1;
    }

    count / duration_secs
}

// ============================================================================
// ASSERTIONS
// ============================================================================

/// Assert value is within range
pub fn assert_within_range(value: u64, min: u64, max: u64, label: &str) {
    assert!(
        value >= min && value <= max,
        "{} out of range: {} (expected {}..{})",
        label,
        value,
        min,
        max
    );
}

/// Assert latency meets target (B32)
pub fn assert_latency_target(actual_ns: u64, target_ns: u64, operation: &str) {
    assert!(
        actual_ns <= target_ns,
        "{} latency exceeded: {}ns > {}ns target",
        operation,
        actual_ns,
        target_ns
    );
}
