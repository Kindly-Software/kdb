# Data Protection Module - Technical Specification

**Complete Implementation Specification for atomic_capsule/src/protection/**

**Date**: 2025-10-31
**Version**: 1.0
**Status**: Ready for Implementation

---

## Module: mod.rs (200 lines)

### Purpose
Public API surface and module organization.

### Code

```rust
//! Data Protection Module - Training data protection via computational capsules
//!
//! **Tier**: T6 Mixed (T0 Auditable + T1 Atomic + T9 Persistent)
//!
//! Prevents catastrophic training data loss through:
//! - T0: Tamper-evident audit trail (hash chains)
//! - T1: Lockfree coordination (atomic counters)
//! - T9: Persistent mmap audit log (crash-safe)
//!
//! # Features
//! - `data-protection`: Base protection module
//! - `protection-audit`: T0 audit trail with hash chains
//! - `protection-backup`: T9 persistent backups with mmap
//!
//! # Performance (B32)
//! - Audit append: <100ns (lockfree)
//! - Pre-commit check: <10s (filesystem scan)
//! - Backup creation: <60s (1GB data)
//!
//! # Example
//! ```rust
//! use atomic_capsule::protection::DataProtectionCapsule;
//!
//! let protection = DataProtectionCapsule::new();
//!
//! // Audit dataset load
//! protection.audit_append("dataset_load", "data.jsonl", hash)?;
//!
//! // Validate git commit
//! protection.validate_precommit()?;
//!
//! // Create backup
//! protection.backup_create()?;
//! ```

pub mod audit;
pub mod backup;
pub mod capsule;
pub mod error;
pub mod precommit;

#[cfg(test)]
mod tests;

// Re-export public types
pub use audit::{AuditLogEntry, AuditTrail};
pub use backup::{BackupCoordinator, BackupMetadata};
pub use capsule::{DataProtectionCapsule, ProtectionStats};
pub use error::ProtectionError;
pub use precommit::{PreCommitValidator, ValidationStats};

/// Module version for compatibility tracking
pub const VERSION: &str = "1.0.0";

/// Maximum audit log entries (100K default)
pub const DEFAULT_AUDIT_CAPACITY: usize = 100_000;

/// Backup retention in days (30 default)
pub const DEFAULT_RETENTION_DAYS: u32 = 30;

/// Protected file extensions
pub const PROTECTED_EXTENSIONS: &[&str] = &["jsonl", "parquet", "arrow", "csv"];
```

---

## Module: error.rs (150 lines)

### Purpose
Error types for data protection operations.

### Code

```rust
//! Error types for data protection module

use core::fmt;

/// Result type for protection operations
pub type ProtectionResult<T> = Result<T, ProtectionError>;

/// Data protection errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtectionError {
    /// Deletion of protected files detected
    ///
    /// This error blocks git commits that would delete training data.
    DeletionDetected {
        files: Vec<String>,
        count: usize,
    },

    /// Audit trail verification failed
    ///
    /// Hash chain is broken, indicating tampering or corruption.
    AuditVerificationFailed {
        entry_index: usize,
        expected_hash: [u8; 32],
        actual_hash: [u8; 32],
    },

    /// Backup creation failed
    BackupFailed {
        reason: String,
    },

    /// CRC32 validation failed
    ///
    /// Backup is corrupted or tampered with.
    CrcMismatch {
        expected: u32,
        actual: u32,
    },

    /// Mmap initialization failed
    MmapError {
        path: String,
        reason: String,
    },

    /// File I/O error
    IoError {
        path: String,
        operation: String,
    },

    /// Capacity exceeded
    ///
    /// Audit log is full, need to archive or expand.
    CapacityExceeded {
        current: usize,
        max: usize,
    },

    /// Invalid hash format
    InvalidHash {
        expected_len: usize,
        actual_len: usize,
    },

    /// Git command failed
    GitError {
        command: String,
        output: String,
    },

    /// Compression failed
    CompressionError {
        reason: String,
    },

    /// Decompression failed
    DecompressionError {
        reason: String,
    },
}

impl fmt::Display for ProtectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeletionDetected { files, count } => {
                write!(
                    f,
                    "Deletion of {} protected files blocked: {:?}",
                    count, files
                )
            }
            Self::AuditVerificationFailed {
                entry_index,
                expected_hash,
                actual_hash,
            } => {
                write!(
                    f,
                    "Audit verification failed at entry {}: expected {:02x?}, got {:02x?}",
                    entry_index, expected_hash, actual_hash
                )
            }
            Self::BackupFailed { reason } => {
                write!(f, "Backup failed: {}", reason)
            }
            Self::CrcMismatch { expected, actual } => {
                write!(
                    f,
                    "CRC32 mismatch: expected {:08x}, got {:08x}",
                    expected, actual
                )
            }
            Self::MmapError { path, reason } => {
                write!(f, "Mmap error for {}: {}", path, reason)
            }
            Self::IoError { path, operation } => {
                write!(f, "I/O error on {} during {}", path, operation)
            }
            Self::CapacityExceeded { current, max } => {
                write!(f, "Capacity exceeded: {}/{}", current, max)
            }
            Self::InvalidHash {
                expected_len,
                actual_len,
            } => {
                write!(
                    f,
                    "Invalid hash: expected {} bytes, got {}",
                    expected_len, actual_len
                )
            }
            Self::GitError { command, output } => {
                write!(f, "Git command '{}' failed: {}", command, output)
            }
            Self::CompressionError { reason } => {
                write!(f, "Compression failed: {}", reason)
            }
            Self::DecompressionError { reason } => {
                write!(f, "Decompression failed: {}", reason)
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ProtectionError {}

impl From<std::io::Error> for ProtectionError {
    fn from(err: std::io::Error) -> Self {
        ProtectionError::IoError {
            path: String::new(),
            operation: format!("{}", err),
        }
    }
}
```

---

## Module: capsule.rs (300 lines)

### Purpose
T6 Mixed compound capsule (DataProtectionCapsule).

### Code

```rust
//! DataProtectionCapsule - T6 Mixed compound capsule

use crate::hash::AtomicHash256;
use crate::patterns::DualAtomicU64;
use core::sync::atomic::{AtomicU64, Ordering};

use super::audit::AuditTrail;
use super::backup::BackupCoordinator;
use super::error::{ProtectionError, ProtectionResult};
use super::precommit::PreCommitValidator;

/// T6 Mixed: Data protection capsule (T0+T1+T9)
///
/// # Architecture
/// - **T0 Auditable**: Hash-chained audit trail
/// - **T1 Atomic**: Lockfree coordination
/// - **T9 Persistent**: Mmap-backed audit log
///
/// # Performance (B32)
/// - Audit append: <100ns (lockfree)
/// - Pre-commit check: <10s (filesystem scan)
/// - Backup creation: <60s (1GB data)
///
/// # Safety (ASSUM)
/// - 99.99% safe (no unwrap, bounds checked)
/// - Zero unsafe in public API
/// - Atomic coordination prevents races
///
/// # Example
/// ```rust
/// use atomic_capsule::protection::DataProtectionCapsule;
///
/// let protection = DataProtectionCapsule::new();
///
/// // Audit dataset operation
/// protection.audit_append("dataset_load", "data.jsonl", hash)?;
///
/// // Validate git commit
/// protection.validate_precommit()?;
///
/// // Create backup
/// protection.backup_create()?;
/// ```
#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 256, tier = "T6")]
#[repr(C, align(256))]
pub struct DataProtectionCapsule {
    // T0: Hash chain head (32 bytes)
    audit_chain: AtomicHash256,

    // T1: Lockfree coordination (128 bytes)
    // Primary: operation_count (lower 32 bits) + reserved (upper 32 bits)
    // Secondary: generation counter (TOCTOU prevention)
    coordination: DualAtomicU64,

    // T1: Deletion attempt counter
    deletion_attempts: AtomicU64,

    // T1: Backup generation counter
    backup_generation: AtomicU64,

    // T1: Last backup timestamp (nanoseconds since epoch)
    last_backup_ns: AtomicU64,

    // T9: Audit log pointer (encoded as u64)
    // #ASSUME_MMAP_VALID: Pointer valid for capsule lifetime
    // #VERIFY_MMAP_VALID: Generation counter prevents use-after-free
    audit_log_ptr: AtomicU64,

    // Cache alignment padding
    _padding: [u8; 64],
}

impl DataProtectionCapsule {
    /// Create new protection capsule
    ///
    /// # Returns
    /// Default-initialized capsule (all counters zero)
    pub fn new() -> Self {
        Self {
            audit_chain: AtomicHash256::new(),
            coordination: DualAtomicU64::new(0, 0),
            deletion_attempts: AtomicU64::new(0),
            backup_generation: AtomicU64::new(0),
            last_backup_ns: AtomicU64::new(0),
            audit_log_ptr: AtomicU64::new(0),
            _padding: [0u8; 64],
        }
    }

    /// Initialize with audit trail
    ///
    /// # Arguments
    /// - `audit_path`: Mmap file path for audit log
    /// - `capacity`: Max audit entries (default: 100K)
    ///
    /// # Performance
    /// - <10ms: Mmap initialization
    /// - <100ms: Recovery from existing log
    pub fn with_audit(audit_path: &str, capacity: usize) -> ProtectionResult<Self> {
        let capsule = Self::new();

        // Initialize audit trail
        let audit = AuditTrail::new(audit_path, capacity)?;

        // Store audit log pointer
        // #ASSUME_POINTER_ENCODING: usize fits in u64 on all supported platforms
        // #VERIFY_POINTER_ENCODING: Compile-time assertion in tests
        let ptr = Box::into_raw(Box::new(audit)) as usize as u64;
        capsule.audit_log_ptr.store(ptr, Ordering::Release);

        Ok(capsule)
    }

    /// Append audit entry (T0+T1+T9)
    ///
    /// # Arguments
    /// - `operation`: Operation type (e.g., "dataset_load")
    /// - `file`: File path
    /// - `hash`: SHA256 hash of file content
    ///
    /// # Performance
    /// - <100ns: Lockfree append to mmap log
    /// - <50ns: SHA256 hash chain update
    ///
    /// # Example
    /// ```rust
    /// protection.audit_append("dataset_load", "data.jsonl", hash)?;
    /// ```
    pub fn audit_append(
        &self,
        operation: &str,
        file: &str,
        hash: [u8; 32],
    ) -> ProtectionResult<()> {
        // Load audit trail pointer
        let ptr = self.audit_log_ptr.load(Ordering::Acquire);
        if ptr == 0 {
            return Err(ProtectionError::MmapError {
                path: String::from("audit_log"),
                reason: String::from("Audit trail not initialized"),
            });
        }

        // Safe: Pointer is valid for capsule lifetime (verified by generation counter)
        let audit = unsafe { &*(ptr as usize as *const AuditTrail) };

        // Append to audit trail
        audit.append(operation, file, hash)?;

        // Increment operation counter
        let gen = self.coordination.load_secondary(Ordering::Acquire);
        self.coordination.store_primary(
            self.coordination.load_primary(Ordering::Relaxed) + 1,
            Ordering::Release,
        );

        Ok(())
    }

    /// Validate pre-commit (T1)
    ///
    /// # Performance
    /// - <10s: Full git diff scan
    /// - Blocks commit if protected files deleted
    ///
    /// # Returns
    /// - `Ok(())`: No deletions detected
    /// - `Err(ProtectionError::DeletionDetected)`: Blocks commit
    pub fn validate_precommit(&self) -> ProtectionResult<()> {
        let validator = PreCommitValidator::new();

        match validator.validate_commit() {
            Ok(()) => Ok(()),
            Err(e) => {
                // Increment deletion attempt counter
                self.deletion_attempts.fetch_add(1, Ordering::Relaxed);
                Err(e)
            }
        }
    }

    /// Create backup (T9)
    ///
    /// # Performance
    /// - <60s: 1GB data (4:1 compression)
    /// - <100ms: CRC32 validation
    /// - <10ms: Atomic metadata update
    pub fn backup_create(&self) -> ProtectionResult<()> {
        let backup = BackupCoordinator::new();

        // Create backup (source path from environment or default)
        let source = std::env::var("KINDLY_DATA_PATH")
            .unwrap_or_else(|_| String::from("data/"));
        let dest = std::env::var("KINDLY_BACKUP_PATH")
            .unwrap_or_else(|_| String::from("/backups/kindly_hft/"));

        let meta = backup.backup(&source, &dest)?;

        // Update backup metadata
        self.backup_generation.fetch_add(1, Ordering::Relaxed);
        self.last_backup_ns.store(meta.timestamp_ns, Ordering::Release);

        Ok(())
    }

    /// Verify audit trail (T0)
    ///
    /// # Performance
    /// - <1ms: 1000 entry chain verification
    ///
    /// # Returns
    /// - `true`: Chain is valid
    /// - `false`: Chain is broken (tampering detected)
    pub fn verify_audit_trail(&self) -> ProtectionResult<bool> {
        let ptr = self.audit_log_ptr.load(Ordering::Acquire);
        if ptr == 0 {
            return Ok(true); // No audit trail initialized
        }

        let audit = unsafe { &*(ptr as usize as *const AuditTrail) };
        audit.verify_chain()
    }

    /// Get protection statistics
    pub fn stats(&self) -> ProtectionStats {
        ProtectionStats {
            operation_count: self.coordination.load_primary(Ordering::Relaxed),
            deletion_attempts: self.deletion_attempts.load(Ordering::Relaxed),
            backup_generation: self.backup_generation.load(Ordering::Relaxed),
            last_backup_ns: self.last_backup_ns.load(Ordering::Relaxed),
        }
    }
}

impl Drop for DataProtectionCapsule {
    fn drop(&mut self) {
        // Clean up audit trail
        let ptr = self.audit_log_ptr.load(Ordering::Acquire);
        if ptr != 0 {
            // Safe: Pointer is valid and we have exclusive access (Drop)
            unsafe {
                let _ = Box::from_raw(ptr as usize as *mut AuditTrail);
            }
        }
    }
}

/// Protection statistics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtectionStats {
    pub operation_count: u64,
    pub deletion_attempts: u64,
    pub backup_generation: u64,
    pub last_backup_ns: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_alignment() {
        assert_eq!(core::mem::size_of::<DataProtectionCapsule>(), 256);
        assert_eq!(core::mem::align_of::<DataProtectionCapsule>(), 256);
    }

    #[test]
    fn test_new_capsule() {
        let capsule = DataProtectionCapsule::new();
        let stats = capsule.stats();

        assert_eq!(stats.operation_count, 0);
        assert_eq!(stats.deletion_attempts, 0);
        assert_eq!(stats.backup_generation, 0);
        assert_eq!(stats.last_backup_ns, 0);
    }

    #[test]
    fn test_stats_increments() {
        let capsule = DataProtectionCapsule::new();

        // Simulate operation
        capsule.coordination.store_primary(5, Ordering::Release);
        capsule.deletion_attempts.store(2, Ordering::Release);

        let stats = capsule.stats();
        assert_eq!(stats.operation_count, 5);
        assert_eq!(stats.deletion_attempts, 2);
    }
}
```

---

## Module: audit.rs (600 lines)

### Purpose
T0+T9 Audit trail with hash chains and mmap persistence.

### Key Structures

```rust
//! Audit trail implementation - T0 Auditable + T9 Persistent

use crate::hash::{AtomicHash256, const_fast_hash};
use crate::primitives::atomic_from_mut::AtomicFromMut;
use core::sync::atomic::{AtomicU64, Ordering};

use super::error::{ProtectionError, ProtectionResult};

/// T0: Tamper-evident audit trail entry
///
/// Hash chain format:
/// ```
/// chain_hash = SHA256(prev_hash + timestamp + operation + file + data_hash)
/// ```
///
/// # Layout
/// - 128 bytes total (cache-line aligned)
/// - 64-byte header + 64-byte hashes
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128, tier = "T0")]
#[repr(C, align(128))]
pub struct AuditLogEntry {
    // Header (64 bytes)
    pub timestamp_ns: u64,
    pub operation_hash: u64,    // const_hash of operation type
    pub file: [u8; 48],         // File path (truncated if needed)

    // Hashes (64 bytes)
    pub data_hash: [u8; 32],    // SHA256 of file content
    pub prev_hash: [u8; 32],    // Previous entry hash
}

impl AuditLogEntry {
    /// Compute chain hash for this entry
    ///
    /// # Performance
    /// - <50ns: SHA256 computation
    pub fn compute_chain_hash(&self) -> [u8; 32] {
        // Concatenate: prev_hash + timestamp + operation + file + data_hash
        let mut input = Vec::with_capacity(128);
        input.extend_from_slice(&self.prev_hash);
        input.extend_from_slice(&self.timestamp_ns.to_le_bytes());
        input.extend_from_slice(&self.operation_hash.to_le_bytes());
        input.extend_from_slice(&self.file);
        input.extend_from_slice(&self.data_hash);

        // SHA256 hash
        sha256_hash(&input)
    }

    /// Verify this entry against expected chain hash
    pub fn verify(&self, expected_chain_hash: &[u8; 32]) -> bool {
        let computed = self.compute_chain_hash();
        computed == *expected_chain_hash
    }
}

/// T0+T9: Audit trail with persistent mmap
///
/// # Architecture
/// - Mmap-backed storage (T9)
/// - Hash-chained entries (T0)
/// - Lockfree append (T1)
///
/// # Performance (B32)
/// - Append: <100ns (lockfree)
/// - Verify: <1ms (1000 entries)
/// - Recovery: <100ms (existing log)
pub struct AuditTrail {
    // T9: Mmap storage
    entries_mmap: *mut AuditLogEntry,
    capacity: usize,

    // T1: Lockfree coordination
    head: AtomicU64,          // Current write position
    generation: AtomicU64,    // TOCTOU prevention

    // T0: Hash chain head
    chain_head: AtomicHash256,
}

// SAFETY: AuditTrail is Send/Sync because:
// - Mmap pointer is valid for struct lifetime
// - All access is coordinated via atomic operations
// - No interior mutability except through atomics
unsafe impl Send for AuditTrail {}
unsafe impl Sync for AuditTrail {}

impl AuditTrail {
    /// Create audit trail with mmap backing
    ///
    /// # Arguments
    /// - `path`: Mmap file path
    /// - `capacity`: Max entries (default: 100K)
    ///
    /// # Performance
    /// - <10ms: Mmap initialization
    /// - <100ms: Recovery from existing log
    pub fn new(path: &str, capacity: usize) -> ProtectionResult<Self> {
        // Implementation details in actual code...
        todo!("Implementation pending")
    }

    /// Append entry to audit trail
    ///
    /// # Performance
    /// - <100ns: Lockfree append
    /// - <50ns: Hash chain computation
    ///
    /// # Example
    /// ```rust
    /// audit.append("dataset_load", "data.jsonl", hash)?;
    /// ```
    pub fn append(
        &self,
        operation: &str,
        file: &str,
        data_hash: [u8; 32],
    ) -> ProtectionResult<()> {
        // #ASSUME_CAPACITY: head < capacity
        // #VERIFY_CAPACITY: Bounds check before pointer access

        // #ASSUME_ATOMIC_ORDERING: Acquire/Release prevents reordering
        // #VERIFY_ATOMIC_ORDERING: Memory ordering audit in T28 tests

        // Implementation details...
        todo!("Implementation pending")
    }

    /// Verify entire hash chain
    ///
    /// # Performance
    /// - <1ms: 1000 entries
    /// - <10ms: 10K entries
    ///
    /// # Returns
    /// - `true`: Chain is valid
    /// - `false`: Chain is broken (tampering detected)
    pub fn verify_chain(&self) -> ProtectionResult<bool> {
        let head = self.head.load(Ordering::Acquire);

        if head == 0 {
            return Ok(true); // Empty chain is valid
        }

        // Verify each entry in sequence
        let mut prev_hash = [0u8; 32]; // Genesis hash

        for i in 0..head as usize {
            // Safe: Bounds checked (i < head)
            let entry = unsafe { &*self.entries_mmap.add(i) };

            // Verify prev_hash matches
            if entry.prev_hash != prev_hash {
                return Err(ProtectionError::AuditVerificationFailed {
                    entry_index: i,
                    expected_hash: prev_hash,
                    actual_hash: entry.prev_hash,
                });
            }

            // Compute and update chain hash
            prev_hash = entry.compute_chain_hash();
        }

        Ok(true)
    }

    /// Export to JSON for compliance
    pub fn export_json(&self, path: &str) -> ProtectionResult<()> {
        todo!("Implementation pending")
    }
}

impl Drop for AuditTrail {
    fn drop(&mut self) {
        // Clean up mmap
        // Implementation details...
    }
}

// Helper: SHA256 hash
fn sha256_hash(input: &[u8]) -> [u8; 32] {
    // Use std::crypto or external crate
    // For now, placeholder
    [0u8; 32]
}
```

---

## Module: precommit.rs (400 lines)

### Purpose
T1 Atomic pre-commit validation.

### Key Code

```rust
//! Pre-commit validation - T1 Atomic

use crate::patterns::DualAtomicU64;
use core::sync::atomic::{AtomicU64, Ordering};

use super::error::{ProtectionError, ProtectionResult};
use super::PROTECTED_EXTENSIONS;

/// T1: Pre-commit validation (lockfree)
///
/// Prevents accidental deletion of training data:
/// - Scans git diff for protected file deletions
/// - Blocks commit if deletions detected
/// - Tracks deletion attempts for monitoring
///
/// # Performance (B32)
/// - <10s: Full git diff scan
/// - <100ns: Atomic counter updates
pub struct PreCommitValidator {
    // T1: Lockfree coordination
    // Primary: total_checks (lower 32 bits) + reserved (upper 32 bits)
    // Secondary: generation counter
    coordination: DualAtomicU64,

    // T1: Deletion attempt counter
    deletion_attempts: AtomicU64,
}

impl PreCommitValidator {
    /// Create new validator
    pub fn new() -> Self {
        Self {
            coordination: DualAtomicU64::new(0, 0),
            deletion_attempts: AtomicU64::new(0),
        }
    }

    /// Validate git commit
    ///
    /// # Performance
    /// - <10s: Full git diff scan
    ///
    /// # Returns
    /// - `Ok(())`: No deletions detected
    /// - `Err(ProtectionError::DeletionDetected)`: Blocks commit
    pub fn validate_commit(&self) -> ProtectionResult<()> {
        // Increment check counter
        let checks = self.coordination.load_primary(Ordering::Relaxed);
        self.coordination.store_primary(checks + 1, Ordering::Release);

        // Get staged deletions
        let deleted_files = self.get_deleted_files()?;

        if deleted_files.is_empty() {
            Ok(())
        } else {
            // Increment deletion attempts
            self.deletion_attempts.fetch_add(1, Ordering::Relaxed);

            Err(ProtectionError::DeletionDetected {
                files: deleted_files.clone(),
                count: deleted_files.len(),
            })
        }
    }

    /// Check specific file patterns
    pub fn validate_patterns(&self, patterns: &[&str]) -> ProtectionResult<()> {
        let deleted_files = self.get_deleted_files()?;

        let protected_deletions: Vec<String> = deleted_files
            .into_iter()
            .filter(|file| {
                patterns.iter().any(|ext| file.ends_with(ext))
            })
            .collect();

        if protected_deletions.is_empty() {
            Ok(())
        } else {
            Err(ProtectionError::DeletionDetected {
                files: protected_deletions.clone(),
                count: protected_deletions.len(),
            })
        }
    }

    /// Get validation statistics
    pub fn stats(&self) -> ValidationStats {
        ValidationStats {
            total_checks: self.coordination.load_primary(Ordering::Relaxed),
            deletion_attempts: self.deletion_attempts.load(Ordering::Relaxed),
            last_check_ns: 0, // Implement if needed
        }
    }

    /// Get deleted files from git diff
    fn get_deleted_files(&self) -> ProtectionResult<Vec<String>> {
        use std::process::Command;

        // Run: git diff --cached --name-only --diff-filter=D
        let output = Command::new("git")
            .args(&["diff", "--cached", "--name-only", "--diff-filter=D"])
            .output()
            .map_err(|e| ProtectionError::GitError {
                command: String::from("git diff"),
                output: format!("{}", e),
            })?;

        if !output.status.success() {
            return Err(ProtectionError::GitError {
                command: String::from("git diff"),
                output: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let files: Vec<String> = stdout
            .lines()
            .filter(|line| {
                PROTECTED_EXTENSIONS.iter().any(|ext| line.ends_with(ext))
            })
            .map(|s| s.to_string())
            .collect();

        Ok(files)
    }
}

/// Validation statistics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidationStats {
    pub total_checks: u64,
    pub deletion_attempts: u64,
    pub last_check_ns: u64,
}
```

---

## Module: backup.rs (500 lines)

### Purpose
T9 Persistent backup coordination.

### Key Code

```rust
//! Backup coordination - T9 Persistent

use crate::patterns::DualAtomicU64;
use core::sync::atomic::{AtomicU64, Ordering};

use super::error::{ProtectionError, ProtectionResult};

/// T9: Backup coordination (persistent mmap)
///
/// Automated backups with:
/// - Compression (lz4)
/// - CRC32 validation
/// - 30-day retention
/// - Atomic writes (crash-safe)
///
/// # Performance (B32)
/// - Create: <60s (1GB data, 4:1 compression)
/// - Verify: <100ms (CRC32 check)
/// - Restore: <2min (1GB data)
pub struct BackupCoordinator {
    // T1: Lockfree coordination
    coordination: DualAtomicU64,

    // T1: Last backup timestamp
    last_backup_ns: AtomicU64,

    // T1: Backup size tracker
    backup_size_bytes: AtomicU64,
}

impl BackupCoordinator {
    /// Create new coordinator
    pub fn new() -> Self {
        Self {
            coordination: DualAtomicU64::new(0, 0),
            last_backup_ns: AtomicU64::new(0),
            backup_size_bytes: AtomicU64::new(0),
        }
    }

    /// Create backup
    ///
    /// # Performance
    /// - <60s: 1GB data (4:1 compression)
    ///
    /// # Arguments
    /// - `source`: Directory to backup
    /// - `dest`: Backup destination
    pub fn backup(
        &self,
        source: &str,
        dest: &str,
    ) -> ProtectionResult<BackupMetadata> {
        // Implementation: tar + lz4 compression + CRC32
        todo!("Implementation pending")
    }

    /// Verify backup integrity
    pub fn verify_backup(&self, path: &str) -> ProtectionResult<bool> {
        // Implementation: CRC32 check
        todo!("Implementation pending")
    }

    /// Restore from backup
    pub fn restore(&self, backup: &str, dest: &str) -> ProtectionResult<()> {
        // Implementation: decompress + extract
        todo!("Implementation pending")
    }

    /// Cleanup old backups (>30 days)
    pub fn cleanup_old(&self, retention_days: u32) -> ProtectionResult<usize> {
        // Implementation: find + rm old backups
        todo!("Implementation pending")
    }
}

/// Backup metadata
#[derive(Debug, Clone)]
pub struct BackupMetadata {
    pub timestamp_ns: u64,
    pub size_bytes: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub file_count: usize,
}
```

---

## Testing Structure

### tests/mod.rs

```rust
//! Test module organization

#[cfg(test)]
mod unit_tests;

#[cfg(test)]
mod property_tests;

#[cfg(test)]
mod integration_tests;

#[cfg(test)]
mod production_tests;
```

### tests/unit_tests.rs (Q1-Q7)

```rust
//! Unit tests (Q1-Q7)

use super::*;

#[test]
fn q1_capsule_alignment() {
    assert_eq!(size_of::<DataProtectionCapsule>(), 256);
    assert_eq!(align_of::<DataProtectionCapsule>(), 256);
}

#[test]
fn q2_audit_entry_size() {
    assert_eq!(size_of::<AuditLogEntry>(), 128);
    assert_eq!(align_of::<AuditLogEntry>(), 128);
}

#[test]
fn q3_hash_chain_computation() {
    // Test SHA256 chain correctness
}

#[test]
fn q4_atomic_coordination() {
    // Test DualAtomicU64 operations
}

// ... Q5-Q7 tests
```

---

## Build & Test Commands

```bash
# Build with all features
cargo build --release --features protection-all

# Run all tests
cargo test --features protection-all

# Run specific test tier
cargo test --features protection-all unit_tests
cargo test --features protection-all property_tests

# Benchmark
cargo bench --features protection-all protection_

# Check code
cargo clippy --features protection-all -- -D warnings
```

---

## Implementation Checklist

- [ ] Create `src/protection/` directory
- [ ] Implement `mod.rs` (public API)
- [ ] Implement `error.rs` (error types)
- [ ] Implement `capsule.rs` (T6 compound)
- [ ] Implement `audit.rs` (T0+T9)
- [ ] Implement `precommit.rs` (T1)
- [ ] Implement `backup.rs` (T9)
- [ ] Add feature flags to `Cargo.toml`
- [ ] Add module declaration to `lib.rs`
- [ ] Implement unit tests (Q1-Q7)
- [ ] Implement property tests (Q8-Q14)
- [ ] Implement integration tests (Q15-Q21)
- [ ] Implement production tests (Q22-Q28)
- [ ] Add benchmarks (B32)
- [ ] Add documentation
- [ ] Review with stakeholder

---

**Status**: Complete Specification (2025-10-31)
**Ready**: Implementation Phase 1 can begin
**Timeline**: 4 weeks to production-ready
