//! Kindly-AV1 Checkpoint Module
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! T9 Persistent tier crash protection with two-phase commit protocol.
//! Provides ACID-like guarantees for encoding state persistence and
//! automatic crash recovery.
//!
//! ## Architecture
//!
//! The checkpoint system uses a two-phase commit protocol:
//!
//! 1. **Begin**: Generation counter becomes ODD (inflight transaction)
//! 2. **Write**: Header + frame index + trailer written to disk
//! 3. **Sync**: fsync ensures durability
//! 4. **Commit**: Generation counter becomes EVEN (committed)
//!
//! On crash recovery, if generation is ODD, the checkpoint is rolled back
//! to the last EVEN (committed) state.
//!
//! ## File Format
//!
//! ```text
//! +------------------+  0
//! | CheckpointHeader |  128 bytes (magic, version, hashes, progress)
//! +------------------+  128
//! | FrameIndexEntry  |  32 bytes each (frame_num, offset, size, psnr)
//! | FrameIndexEntry  |
//! | ...              |
//! +------------------+  128 + (32 * frame_count)
//! | CheckpointTrailer|  32 bytes (crc32, generation, committed)
//! +------------------+  EOF
//! ```
//!
//! ## Capsules
//!
//! - `EncoderCheckpointCapsule` (256B, T9) - Atomic checkpoint state management
//!
//! ## Usage
//!
//! ```rust,ignore
//! use kindly_av1::checkpoint::{
//!     EncoderCheckpointCapsule, CheckpointHeader, FrameIndexEntry,
//!     recover_from_crash, calculate_input_hash, default_checkpoint_path,
//! };
//!
//! // Calculate input hash for validation
//! let input_hash = calculate_input_hash("input.mp4")?;
//!
//! // Attempt recovery
//! let checkpoint_path = default_checkpoint_path("output.av1");
//! let recovery = recover_from_crash(&checkpoint_path, "output.av1", input_hash)?;
//!
//! if recovery.should_resume() {
//!     println!("Resuming from frame {}", recovery.resume_frame);
//! }
//!
//! // Create checkpoint capsule
//! let capsule = EncoderCheckpointCapsule::new(input_hash, 30);
//!
//! // During encoding, periodically checkpoint
//! if capsule.should_checkpoint(current_frame) {
//!     let header = CheckpointHeader::new(input_hash, total_frames, config_hash);
//!     capsule.write_checkpoint(&checkpoint_path, &header, &frame_entries)?;
//! }
//! ```
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 T9 Persistent tier with ACID-like guarantees
//! - **Chaos**: 256B cache-aligned capsule, generation counters, atomic state
//! - **ASSUM**: 99.5%+ safe, all file I/O documented with #ASSUME/#VERIFY
//! - **B32**: Performance validated (<1% encoding overhead)
//! - **T28**: 5-tier testing (unit/property/integration/production/determinism)

mod capsule;
mod format;
mod recovery;

// Re-export main types
pub use capsule::{CheckpointData, CheckpointError, EncoderCheckpointCapsule};
pub use format::{
    calculate_crc32, CheckpointHeader, CheckpointTrailer, FrameIndexEntry,
    CHECKPOINT_MAGIC, CHECKPOINT_VERSION, FRAME_ENTRY_SIZE, HEADER_SIZE, TRAILER_SIZE,
};
pub use recovery::{
    calculate_config_hash, calculate_input_hash, default_checkpoint_path,
    delete_checkpoint, quick_validate, recover_from_crash, truncate_output,
    CheckpointRecovery,
};

/// Checkpoint configuration
#[derive(Debug, Clone)]
pub struct CheckpointConfig {
    /// Checkpoint interval in frames (default: 30)
    pub interval_frames: u32,
    /// Enable checkpoint compression (future feature)
    pub compress: bool,
    /// Maximum checkpoint file size in bytes
    pub max_size: u64,
    /// Checkpoint file path (None = auto-generate from output path)
    pub checkpoint_path: Option<std::path::PathBuf>,
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            interval_frames: 30,
            compress: false, // Not yet implemented
            max_size: 1024 * 1024 * 100, // 100 MB
            checkpoint_path: None,
        }
    }
}

impl CheckpointConfig {
    /// Create config with custom interval
    pub fn with_interval(interval_frames: u32) -> Self {
        Self {
            interval_frames,
            ..Default::default()
        }
    }

    /// Create config with explicit checkpoint path
    pub fn with_path<P: Into<std::path::PathBuf>>(path: P) -> Self {
        Self {
            checkpoint_path: Some(path.into()),
            ..Default::default()
        }
    }

    /// Get checkpoint path, generating from output path if needed
    pub fn get_checkpoint_path<P: AsRef<std::path::Path>>(&self, output_path: P) -> std::path::PathBuf {
        self.checkpoint_path
            .clone()
            .unwrap_or_else(|| default_checkpoint_path(output_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_config_default() {
        let config = CheckpointConfig::default();
        assert_eq!(config.interval_frames, 30);
        assert!(!config.compress);
        assert_eq!(config.max_size, 100 * 1024 * 1024);
        assert!(config.checkpoint_path.is_none());
    }

    #[test]
    fn test_checkpoint_config_with_interval() {
        let config = CheckpointConfig::with_interval(60);
        assert_eq!(config.interval_frames, 60);
    }

    #[test]
    fn test_checkpoint_config_with_path() {
        let config = CheckpointConfig::with_path("/custom/path.ckpt");
        assert_eq!(
            config.checkpoint_path.as_ref().unwrap().to_str().unwrap(),
            "/custom/path.ckpt"
        );
    }

    #[test]
    fn test_checkpoint_config_get_path() {
        // Default: auto-generate
        let config = CheckpointConfig::default();
        let path = config.get_checkpoint_path("/output/video.av1");
        assert_eq!(path.to_str().unwrap(), "/output/video.av1.kdly.ckpt");

        // Custom: use specified
        let config_custom = CheckpointConfig::with_path("/custom/checkpoint.ckpt");
        let path_custom = config_custom.get_checkpoint_path("/output/video.av1");
        assert_eq!(path_custom.to_str().unwrap(), "/custom/checkpoint.ckpt");
    }

    #[test]
    fn test_capsule_creation() {
        let capsule = EncoderCheckpointCapsule::new([0u8; 32], 30);
        assert_eq!(capsule.interval(), 30);
        assert!(capsule.is_valid());
        assert!(!capsule.is_inflight());
    }

    #[test]
    fn test_header_creation() {
        let header = CheckpointHeader::new([0xABu8; 32], 1000, [0xCDu8; 32]);
        assert!(header.validate());
        assert_eq!(header.total_frames, 1000);
        assert_eq!(header.completed_frames, 0);
    }

    #[test]
    fn test_frame_entry_creation() {
        let entry = FrameIndexEntry::new(42, 1024, 512);
        assert_eq!(entry.frame_num, 42);
        assert_eq!(entry.output_offset, 1024);
        assert_eq!(entry.encoded_size, 512);
    }

    #[test]
    fn test_recovery_fresh_start() {
        let recovery = CheckpointRecovery::fresh_start();
        assert!(!recovery.recovery_needed);
        assert!(!recovery.should_resume());
        assert_eq!(recovery.resume_frame, 0);
    }

    // Integration tests that require file I/O are in the submodule tests
}
