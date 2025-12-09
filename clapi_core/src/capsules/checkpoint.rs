//! Checkpoint - Durable persistence for timeline recovery (E7/E21)
//!
//! ## Purpose
//! Save pending timeline events to disk with fsync() durability for recovery after crashes.
//! Enables zero data loss with atomic file operations and proper permissions (0600).
//!
//! ## Safety Assumptions (ASSUM Framework)
//! - #ASSUME: Filesystem supports fsync() with durable persistence guarantees
//! - #VERIFY: Integration tests validate recovery from checkpoint
//! - #ASSUME: Atomic rename operation (POSIX compliant filesystems)
//! - #VERIFY: Crash recovery tests validate atomic semantics
//! - #ASSUME: serde_json serialization is deterministic
//! - #VERIFY: Unit tests validate serialization round-trips

use serde::{Serialize, Deserialize};
use std::fs;
use std::path::Path;
use std::time::SystemTime;
use crate::error::{ClapiError, ClapiResult};

/// Timeline checkpoint for crash recovery
///
/// Stores pending events that haven't been flushed to timeline buckets.
/// Supports atomic persistence via temp file + rename pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Pending event timestamps (epoch seconds)
    pub pending: Vec<u64>,

    /// Last checkpoint update time
    #[serde(with = "system_time_serde")]
    pub last_updated: SystemTime,

    /// Generation counter (monotonically increasing)
    pub generation: u64,
}

impl Checkpoint {
    /// Create new empty checkpoint
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            last_updated: SystemTime::now(),
            generation: 0,
        }
    }

    /// Create checkpoint from pending events
    pub fn from_pending(pending: Vec<u64>) -> Self {
        Self {
            pending,
            last_updated: SystemTime::now(),
            generation: 0,
        }
    }

    /// Save checkpoint to disk with fsync() durability (E7)
    ///
    /// # Durability Guarantee
    /// - Write to temporary file first (crash safe)
    /// - Sync to disk (fsync) before rename
    /// - Atomic rename (POSIX semantics)
    /// - Set permissions 0600 (owner-only)
    ///
    /// # Performance
    /// - Target: <10ms for 1000 events (<100KB JSON)
    /// - Dominated by fsync() latency (disk dependent)
    pub fn save(&self, path: &Path) -> ClapiResult<()> {
        // Serialize to JSON
        let data = serde_json::to_string(self).map_err(|e| {
            ClapiError::IoError(format!("Failed to serialize checkpoint: {}", e))
        })?;

        // Write to temporary file first (atomic)
        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, &data).map_err(|e| {
            ClapiError::IoError(format!("Failed to write checkpoint temp file: {}", e))
        })?;

        // Sync to disk (durability guarantee - E7 requirement)
        let file = fs::OpenOptions::new()
            .write(true)
            .open(&tmp_path)
            .map_err(|e| {
                ClapiError::IoError(format!("Failed to open temp file for sync: {}", e))
            })?;

        file.sync_all().map_err(|e| {
            ClapiError::IoError(format!("Failed to sync checkpoint to disk: {}", e))
        })?;

        // Atomic rename (POSIX semantics)
        fs::rename(&tmp_path, path).map_err(|e| {
            ClapiError::IoError(format!("Failed to rename checkpoint file: {}", e))
        })?;

        // Set permissions 0600 (owner-only, E21 security requirement)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            fs::set_permissions(path, perms).map_err(|e| {
                ClapiError::IoError(format!("Failed to set checkpoint permissions: {}", e))
            })?;
        }

        Ok(())
    }

    /// Load checkpoint from disk
    ///
    /// # Returns
    /// - Ok(Checkpoint) if file exists and valid
    /// - Ok(empty Checkpoint) if file doesn't exist
    /// - Err if file exists but corrupted
    pub fn load(path: &Path) -> ClapiResult<Self> {
        if !path.exists() {
            return Ok(Checkpoint::new());
        }

        let data = fs::read_to_string(path).map_err(|e| {
            ClapiError::IoError(format!("Failed to read checkpoint file: {}", e))
        })?;

        serde_json::from_str(&data).map_err(|e| {
            ClapiError::IoError(format!("Failed to deserialize checkpoint: {}", e))
        })
    }

    /// Clear checkpoint file (delete)
    pub fn clear(path: &Path) -> ClapiResult<()> {
        if path.exists() {
            fs::remove_file(path).map_err(|e| {
                ClapiError::IoError(format!("Failed to remove checkpoint file: {}", e))
            })?;
        }
        Ok(())
    }

    /// Get pending event count
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Check if checkpoint is empty
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

impl Default for Checkpoint {
    fn default() -> Self {
        Self::new()
    }
}

/// Serde serialization for SystemTime (as epoch seconds)
mod system_time_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let secs = time
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        secs.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(UNIX_EPOCH + std::time::Duration::from_secs(secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_checkpoint_creation() {
        let checkpoint = Checkpoint::new();
        assert_eq!(checkpoint.pending_count(), 0);
        assert!(checkpoint.is_empty());
        assert_eq!(checkpoint.generation, 0);
    }

    #[test]
    fn test_checkpoint_from_pending() {
        let pending = vec![1000, 1001, 1002];
        let checkpoint = Checkpoint::from_pending(pending.clone());

        assert_eq!(checkpoint.pending_count(), 3);
        assert_eq!(checkpoint.pending, pending);
    }

    #[test]
    fn test_checkpoint_save_load() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_checkpoint.json");

        // Clean up any existing file
        let _ = fs::remove_file(&path);

        // Create and save checkpoint
        let checkpoint = Checkpoint::from_pending(vec![1000, 1001, 1002]);
        assert!(checkpoint.save(&path).is_ok());

        // Verify file exists
        assert!(path.exists());

        // Load checkpoint
        let loaded = Checkpoint::load(&path).unwrap();
        assert_eq!(loaded.pending_count(), 3);
        assert_eq!(loaded.pending, checkpoint.pending);

        // Clean up
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_checkpoint_load_nonexistent() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("nonexistent_checkpoint.json");

        // Ensure file doesn't exist
        let _ = fs::remove_file(&path);

        // Load should return empty checkpoint
        let checkpoint = Checkpoint::load(&path).unwrap();
        assert!(checkpoint.is_empty());
    }

    #[test]
    fn test_checkpoint_clear() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_clear_checkpoint.json");

        // Create checkpoint
        let checkpoint = Checkpoint::from_pending(vec![1000]);
        assert!(checkpoint.save(&path).is_ok());
        assert!(path.exists());

        // Clear checkpoint
        assert!(Checkpoint::clear(&path).is_ok());
        assert!(!path.exists());

        // Clear nonexistent should succeed
        assert!(Checkpoint::clear(&path).is_ok());
    }

    #[test]
    fn test_checkpoint_atomic_save() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_atomic_checkpoint.json");

        // Clean up
        let _ = fs::remove_file(&path);

        // Create checkpoint
        let checkpoint1 = Checkpoint::from_pending(vec![1000]);
        assert!(checkpoint1.save(&path).is_ok());

        // Overwrite with new checkpoint (atomic replace)
        let checkpoint2 = Checkpoint::from_pending(vec![2000, 2001]);
        assert!(checkpoint2.save(&path).is_ok());

        // Load should get latest
        let loaded = Checkpoint::load(&path).unwrap();
        assert_eq!(loaded.pending_count(), 2);
        assert_eq!(loaded.pending, vec![2000, 2001]);

        // Clean up
        let _ = fs::remove_file(&path);
    }

    #[test]
    #[cfg(unix)]
    fn test_checkpoint_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_permissions_checkpoint.json");

        // Clean up
        let _ = fs::remove_file(&path);

        // Create and save checkpoint
        let checkpoint = Checkpoint::from_pending(vec![1000]);
        assert!(checkpoint.save(&path).is_ok());

        // Verify permissions (0600)
        let metadata = fs::metadata(&path).unwrap();
        let permissions = metadata.permissions();
        assert_eq!(permissions.mode() & 0o777, 0o600);

        // Clean up
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_checkpoint_corrupted_file() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_corrupted_checkpoint.json");

        // Write invalid JSON
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(b"{ invalid json }").unwrap();

        // Load should fail
        assert!(Checkpoint::load(&path).is_err());

        // Clean up
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_checkpoint_serialization_deterministic() {
        let checkpoint = Checkpoint::from_pending(vec![1000, 1001, 1002]);

        // Serialize twice
        let json1 = serde_json::to_string(&checkpoint).unwrap();
        let json2 = serde_json::to_string(&checkpoint).unwrap();

        // Should be identical (deterministic serialization)
        // Note: last_updated may differ, so compare pending only
        let parsed1: serde_json::Value = serde_json::from_str(&json1).unwrap();
        let parsed2: serde_json::Value = serde_json::from_str(&json2).unwrap();

        assert_eq!(parsed1["pending"], parsed2["pending"]);
        assert_eq!(parsed1["generation"], parsed2["generation"]);
    }
}
