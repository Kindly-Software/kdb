//! Precommit Guard Capsule - T1 Atomic Deletion Detection
//!
//! **Phase 3 Data Protection**: Git pre-commit hook to prevent training data deletion
//!
//! # Architecture
//!
//! **Tier 1 (Atomic)**: Lockfree state management for deletion detection
//!
//! # Performance (B32 Targets)
//! - Scan: <10s for full repository
//! - State update: <10ns (atomic operations)
//!
//! # Safety
//!
//! 99.99% safe - No unwrap(), all operations return Result

use crate::error::AuditError;
use crate::patterns::dual_atomic::DualAtomicU64;
use core::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// PRECOMMIT RESULT
// ============================================================================

/// Result of pre-commit validation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrecommitResult {
    /// Number of files scanned
    pub files_scanned: usize,

    /// Number of deletions detected
    pub deletions_detected: usize,

    /// Number of training data files affected
    pub training_files_affected: usize,

    /// Whether commit should be blocked
    pub should_block: bool,
}

impl PrecommitResult {
    /// Create result indicating commit should proceed
    pub fn allow(files_scanned: usize) -> Self {
        Self {
            files_scanned,
            deletions_detected: 0,
            training_files_affected: 0,
            should_block: false,
        }
    }

    /// Create result indicating commit should be blocked
    pub fn block(files_scanned: usize, deletions: usize, training_files: usize) -> Self {
        Self {
            files_scanned,
            deletions_detected: deletions,
            training_files_affected: training_files,
            should_block: true,
        }
    }
}

// ============================================================================
// PRECOMMIT GUARD CAPSULE (128 bytes, T1 Atomic)
// ============================================================================

/// Precommit Guard Capsule - Atomic deletion detection
///
/// **UCE34 Q10**: T1 Atomic tier
///
/// # Performance
/// - State update: <10ns (atomic operations)
/// - Scan coordination: <5ns per file
///
/// # Safety
/// - 100% lockfree atomic operations
/// - No unwrap() - all operations return Result
#[repr(C, align(256))]
pub struct PrecommitGuardCapsule {
    /// Total scans performed
    scan_count: AtomicU64,

    /// Total deletions detected across all scans
    total_deletions: AtomicU64,

    /// Total commits blocked
    commits_blocked: AtomicU64,

    /// Last scan timestamp (nanoseconds)
    last_scan_ns: AtomicU64,

    /// Coordination for concurrent access
    /// Primary: Generation counter
    /// Secondary: Scan state flags
    coordination: DualAtomicU64,

    /// Padding to 256 bytes (align=256, size=256)
    /// 256 - (32 + 128) = 256 - 160 = 96 bytes
    /// 4 × AtomicU64: 32, DualAtomicU64: 128, padding: 96
    _padding: [u8; 96],
}

impl PrecommitGuardCapsule {
    /// Create new precommit guard capsule
    pub fn new() -> Self {
        Self {
            scan_count: AtomicU64::new(0),
            total_deletions: AtomicU64::new(0),
            commits_blocked: AtomicU64::new(0),
            last_scan_ns: AtomicU64::new(0),
            coordination: DualAtomicU64::new(0, 0),
            _padding: [0u8; 96],
        }
    }

    /// Scan for training data deletions
    ///
    /// # Arguments
    /// * `deleted_files` - List of file paths being deleted
    ///
    /// # Returns
    /// PrecommitResult indicating whether to block commit
    ///
    /// # Performance
    /// <10s target for full repository scan
    pub fn scan_deletions(&self, deleted_files: &[&str]) -> Result<PrecommitResult, AuditError> {
        // Update scan count
        self.scan_count.fetch_add(1, Ordering::Relaxed);

        // Update last scan timestamp
        #[cfg(feature = "std")]
        {
            use std::time::{SystemTime, UNIX_EPOCH};
            if let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) {
                self.last_scan_ns
                    .store(duration.as_nanos() as u64, Ordering::Relaxed);
            }
        }

        // Count training data files being deleted
        let training_files_affected = deleted_files
            .iter()
            .filter(|path| Self::is_training_data(path))
            .count();

        let should_block = training_files_affected > 0;

        // Update statistics
        if deleted_files.len() > 0 {
            self.total_deletions
                .fetch_add(deleted_files.len() as u64, Ordering::Relaxed);
        }

        if should_block {
            self.commits_blocked.fetch_add(1, Ordering::Relaxed);
        }

        // Update coordination generation
        self.coordination.fetch_add_primary(1, Ordering::Release);

        let result = if should_block {
            PrecommitResult::block(
                deleted_files.len(),
                deleted_files.len(),
                training_files_affected,
            )
        } else {
            PrecommitResult::allow(deleted_files.len())
        };

        Ok(result)
    }

    /// Check if file path is training data
    fn is_training_data(path: &str) -> bool {
        // Training data patterns:
        // - *.jsonl files in data/ directory
        // - training_*.json files
        // - *_train.json files
        path.ends_with(".jsonl")
            || path.contains("/data/")
            || path.contains("training_")
            || path.contains("_train")
            || path.ends_with("_train.json")
    }

    /// Get total scans performed
    pub fn scan_count(&self) -> u64 {
        self.scan_count.load(Ordering::Relaxed)
    }

    /// Get total deletions detected
    pub fn total_deletions(&self) -> u64 {
        self.total_deletions.load(Ordering::Relaxed)
    }

    /// Get total commits blocked
    pub fn commits_blocked(&self) -> u64 {
        self.commits_blocked.load(Ordering::Relaxed)
    }

    /// Get last scan timestamp
    pub fn last_scan_ns(&self) -> u64 {
        self.last_scan_ns.load(Ordering::Relaxed)
    }

    /// Check specific file path
    ///
    /// # Arguments
    /// * `path` - File path to check
    ///
    /// # Returns
    /// true if file is training data and should be protected
    pub fn is_protected_file(&self, path: &str) -> bool {
        Self::is_training_data(path)
    }
}

impl Default for PrecommitGuardCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Compile-time verification (Q33 mandatory)
// Note: With align(256), 256 bytes of fields rounds to 512 bytes (next multiple of 256)
crate::verify_capsule_properties!(PrecommitGuardCapsule, 256, 512);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_training_data_detection() {
        assert!(PrecommitGuardCapsule::is_training_data(
            "data/training.jsonl"
        ));
        assert!(PrecommitGuardCapsule::is_training_data(
            "training_500k.json"
        ));
        assert!(PrecommitGuardCapsule::is_training_data("multi_train.json"));
        assert!(PrecommitGuardCapsule::is_training_data(
            "/path/to/data/examples.jsonl"
        ));

        // Non-training files
        assert!(!PrecommitGuardCapsule::is_training_data("README.md"));
        assert!(!PrecommitGuardCapsule::is_training_data("src/main.rs"));
        assert!(!PrecommitGuardCapsule::is_training_data("config.toml"));
    }

    #[test]
    fn test_scan_no_deletions() {
        let guard = PrecommitGuardCapsule::new();
        let result = guard.scan_deletions(&[]).unwrap();

        assert!(!result.should_block);
        assert_eq!(result.deletions_detected, 0);
        assert_eq!(result.training_files_affected, 0);
        assert_eq!(guard.scan_count(), 1);
        assert_eq!(guard.commits_blocked(), 0);
    }

    #[test]
    fn test_scan_safe_deletions() {
        let guard = PrecommitGuardCapsule::new();
        let deleted = vec!["README.md", "docs/guide.md", "scripts/build.sh"];
        let result = guard.scan_deletions(&deleted).unwrap();

        assert!(!result.should_block);
        assert_eq!(result.training_files_affected, 0);
        assert_eq!(guard.scan_count(), 1);
        assert_eq!(guard.commits_blocked(), 0);
    }

    #[test]
    fn test_scan_training_deletion() {
        let guard = PrecommitGuardCapsule::new();
        let deleted = vec!["data/training.jsonl", "README.md"];
        let result = guard.scan_deletions(&deleted).unwrap();

        assert!(result.should_block);
        assert_eq!(result.training_files_affected, 1);
        assert_eq!(guard.scan_count(), 1);
        assert_eq!(guard.commits_blocked(), 1);
        assert_eq!(guard.total_deletions(), 2);
    }

    #[test]
    fn test_multiple_scans() {
        let guard = PrecommitGuardCapsule::new();

        // First scan - safe
        let result1 = guard.scan_deletions(&["README.md"]).unwrap();
        assert!(!result1.should_block);

        // Second scan - blocked
        let result2 = guard.scan_deletions(&["data/training_500k.json"]).unwrap();
        assert!(result2.should_block);

        // Third scan - safe
        let result3 = guard.scan_deletions(&["src/main.rs"]).unwrap();
        assert!(!result3.should_block);

        assert_eq!(guard.scan_count(), 3);
        assert_eq!(guard.commits_blocked(), 1);
        assert_eq!(guard.total_deletions(), 3);
    }

    #[test]
    fn test_is_protected_file() {
        let guard = PrecommitGuardCapsule::new();

        assert!(guard.is_protected_file("data/train.jsonl"));
        assert!(guard.is_protected_file("training_data.json"));
        assert!(!guard.is_protected_file("README.md"));
    }
}
