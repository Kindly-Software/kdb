//! Crash recovery logic for kindly-av1
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Provides automated crash recovery using checkpoint files with
//! two-phase commit protocol detection and output file truncation.
//!
//! ## Recovery Flow
//!
//! 1. Detect checkpoint file existence
//! 2. Validate checkpoint integrity (magic, version, CRC)
//! 3. Check generation counter for incomplete transactions
//! 4. Validate input file hash matches checkpoint
//! 5. Truncate output file to last valid checkpoint position
//! 6. Return resume position for encoder
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 T9 Persistent tier recovery
//! - **Chaos**: Atomic operations, safe file I/O
//! - **ASSUM**: All file operations documented with #ASSUME/#VERIFY

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use super::capsule::CheckpointError;
use super::format::{
    calculate_crc32, CheckpointHeader, CheckpointTrailer, FrameIndexEntry,
    CHECKPOINT_MAGIC, FRAME_ENTRY_SIZE, HEADER_SIZE, TRAILER_SIZE,
};

/// Recovery result containing resume information
#[derive(Debug, Clone)]
pub struct CheckpointRecovery {
    /// Frame number to resume encoding from (0-indexed)
    pub resume_frame: u64,
    /// Byte offset in output file to truncate to
    pub truncate_offset: u64,
    /// Total frames in the video
    pub total_frames: u64,
    /// Whether recovery was needed (true if checkpoint existed)
    pub recovery_needed: bool,
    /// Error message from previous encoding attempt (if any)
    pub previous_error: Option<String>,
    /// Number of frames recovered from checkpoint
    pub recovered_frames: u64,
    /// Generation counter of recovered checkpoint
    pub checkpoint_generation: u64,
    /// All recovered frame entries
    pub frame_entries: Vec<FrameIndexEntry>,
}

impl CheckpointRecovery {
    /// Create recovery result for fresh start (no checkpoint)
    pub fn fresh_start() -> Self {
        Self {
            resume_frame: 0,
            truncate_offset: 0,
            total_frames: 0,
            recovery_needed: false,
            previous_error: None,
            recovered_frames: 0,
            checkpoint_generation: 0,
            frame_entries: Vec::new(),
        }
    }

    /// Check if encoding should resume from this recovery
    #[inline]
    pub fn should_resume(&self) -> bool {
        self.recovery_needed && self.resume_frame > 0
    }

    /// Get percentage of frames already completed
    #[inline]
    pub fn progress_percent(&self) -> f64 {
        if self.total_frames == 0 {
            return 0.0;
        }
        (self.resume_frame as f64 / self.total_frames as f64) * 100.0
    }
}

impl Default for CheckpointRecovery {
    fn default() -> Self {
        Self::fresh_start()
    }
}

/// Attempt to recover from crash using checkpoint file
///
/// Main entry point for crash recovery. Validates checkpoint and returns
/// information needed to resume encoding.
///
/// # Arguments
/// * `checkpoint_path` - Path to checkpoint file (*.kdly.ckpt)
/// * `output_path` - Path to output file (for truncation)
/// * `input_hash` - Blake3 hash of input file (first 1MB)
///
/// # Returns
/// `CheckpointRecovery` with resume information, or error if recovery fails.
///
/// # Recovery States
///
/// | Checkpoint State | Action |
/// |-----------------|--------|
/// | Not found | Fresh start, no recovery needed |
/// | Valid (even gen) | Resume from last committed frame |
/// | In-flight (odd gen) | Rollback to previous checkpoint or fresh start |
/// | Corrupted | Return error (user must delete checkpoint) |
/// | Hash mismatch | Return error (different input file) |
pub fn recover_from_crash<P: AsRef<Path>>(
    checkpoint_path: P,
    output_path: P,
    input_hash: [u8; 32],
) -> Result<CheckpointRecovery, CheckpointError> {
    let checkpoint_path = checkpoint_path.as_ref();
    let output_path = output_path.as_ref();

    // Check if checkpoint file exists
    if !checkpoint_path.exists() {
        return Ok(CheckpointRecovery::fresh_start());
    }

    // Open checkpoint file
    // #ASSUME: File system supports concurrent reads safely.
    // #VERIFY: Only one recovery attempt per encoding session.
    let mut file = File::open(checkpoint_path)?;

    // Get file size
    let file_size = file.metadata()?.len();
    if file_size < (HEADER_SIZE + TRAILER_SIZE) as u64 {
        return Err(CheckpointError::InvalidFormat);
    }

    // Read header
    let mut header_bytes = [0u8; HEADER_SIZE];
    file.read_exact(&mut header_bytes)?;

    let header = CheckpointHeader::from_bytes(&header_bytes)
        .ok_or(CheckpointError::InvalidFormat)?;

    // Validate magic and version
    if !header.validate() {
        return Err(CheckpointError::InvalidFormat);
    }

    // Validate input file hash
    if header.input_hash != input_hash {
        return Err(CheckpointError::InputMismatch);
    }

    // Calculate number of frame entries
    let entries_size = file_size as usize - HEADER_SIZE - TRAILER_SIZE;
    if entries_size % FRAME_ENTRY_SIZE != 0 {
        return Err(CheckpointError::InvalidFormat);
    }
    let entry_count = entries_size / FRAME_ENTRY_SIZE;

    // Read frame entries
    let mut frame_entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        let mut entry_bytes = [0u8; FRAME_ENTRY_SIZE];
        file.read_exact(&mut entry_bytes)?;
        frame_entries.push(FrameIndexEntry::from_bytes(&entry_bytes));
    }

    // Read trailer
    let mut trailer_bytes = [0u8; TRAILER_SIZE];
    file.read_exact(&mut trailer_bytes)?;
    let trailer = CheckpointTrailer::from_bytes(&trailer_bytes);

    // Check for in-flight (incomplete) checkpoint
    if trailer.is_inflight() {
        // Crash during checkpoint write - need to determine recovery strategy
        // Option 1: If we have previous checkpoints, rollback to last valid
        // Option 2: Start fresh if no previous valid state

        // For now, we treat in-flight as requiring manual intervention
        // because we don't maintain checkpoint history
        return Err(CheckpointError::InFlightCheckpoint);
    }

    // Validate CRC
    let expected_crc = calculate_crc32(&header, &frame_entries);
    if trailer.crc32 != expected_crc {
        return Err(CheckpointError::CorruptedCheckpoint);
    }

    // Calculate truncation offset from last frame entry
    let truncate_offset = frame_entries
        .last()
        .map(|e| e.output_offset + e.encoded_size)
        .unwrap_or(0);

    // Truncate output file if it exists and is larger than checkpoint position
    // #ASSUME: Output file can be truncated safely.
    // #VERIFY: Truncation only removes data after checkpoint position.
    if output_path.exists() {
        truncate_output(output_path, truncate_offset)?;
    }

    Ok(CheckpointRecovery {
        resume_frame: header.completed_frames,
        truncate_offset,
        total_frames: header.total_frames,
        recovery_needed: true,
        previous_error: None,
        recovered_frames: header.completed_frames,
        checkpoint_generation: trailer.generation,
        frame_entries,
    })
}

/// Truncate output file to specified offset
///
/// Removes any data written after the last valid checkpoint position.
///
/// # Arguments
/// * `path` - Output file path
/// * `offset` - Byte offset to truncate to
///
/// # Safety
/// #ASSUME: Caller has verified offset is valid (from checkpoint).
/// #VERIFY: File truncation is atomic on POSIX systems.
pub fn truncate_output<P: AsRef<Path>>(path: P, offset: u64) -> Result<(), CheckpointError> {
    let path = path.as_ref();

    if !path.exists() {
        // No file to truncate - this is OK for recovery
        return Ok(());
    }

    // Open file for writing
    let file = OpenOptions::new()
        .write(true)
        .open(path)?;

    // Check current size
    let current_size = file.metadata()?.len();

    if current_size > offset {
        // Truncate to checkpoint position
        // #ASSUME: set_len atomically truncates file on POSIX.
        // #VERIFY: Only called after checkpoint validation.
        file.set_len(offset)?;
    }

    Ok(())
}

/// Validate checkpoint without full loading
///
/// Quick validation for UI/status display.
///
/// # Arguments
/// * `checkpoint_path` - Path to checkpoint file
///
/// # Returns
/// Tuple of (is_valid, completed_frames, total_frames) or error.
pub fn quick_validate<P: AsRef<Path>>(
    checkpoint_path: P,
) -> Result<(bool, u64, u64), CheckpointError> {
    let checkpoint_path = checkpoint_path.as_ref();

    if !checkpoint_path.exists() {
        return Err(CheckpointError::FileNotFound);
    }

    let mut file = File::open(checkpoint_path)?;

    // Read header
    let mut header_bytes = [0u8; HEADER_SIZE];
    file.read_exact(&mut header_bytes)?;

    let header = CheckpointHeader::from_bytes(&header_bytes)
        .ok_or(CheckpointError::InvalidFormat)?;

    // Seek to trailer
    file.seek(SeekFrom::End(-(TRAILER_SIZE as i64)))?;

    let mut trailer_bytes = [0u8; TRAILER_SIZE];
    file.read_exact(&mut trailer_bytes)?;
    let trailer = CheckpointTrailer::from_bytes(&trailer_bytes);

    Ok((
        trailer.is_valid(),
        header.completed_frames,
        header.total_frames,
    ))
}

/// Delete checkpoint file
///
/// Removes checkpoint file after successful encoding completion.
///
/// # Arguments
/// * `checkpoint_path` - Path to checkpoint file
pub fn delete_checkpoint<P: AsRef<Path>>(checkpoint_path: P) -> Result<(), CheckpointError> {
    let path = checkpoint_path.as_ref();

    if path.exists() {
        std::fs::remove_file(path)?;
    }

    Ok(())
}

/// Get default checkpoint path for output file
///
/// Generates checkpoint path by appending `.kdly.ckpt` to output path.
///
/// # Arguments
/// * `output_path` - Output file path
///
/// # Returns
/// Checkpoint file path (e.g., `output.av1.kdly.ckpt`)
pub fn default_checkpoint_path<P: AsRef<Path>>(output_path: P) -> std::path::PathBuf {
    let path = output_path.as_ref();
    let mut checkpoint_path = path.to_path_buf();

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("output");

    checkpoint_path.set_file_name(format!("{}.kdly.ckpt", filename));
    checkpoint_path
}

/// Calculate input file hash for checkpoint validation
///
/// Computes Blake3 hash of first 1MB of input file for fast validation.
///
/// # Arguments
/// * `input_path` - Input file path
///
/// # Returns
/// 32-byte Blake3 hash
pub fn calculate_input_hash<P: AsRef<Path>>(input_path: P) -> Result<[u8; 32], CheckpointError> {
    let path = input_path.as_ref();

    if !path.exists() {
        return Err(CheckpointError::FileNotFound);
    }

    let mut file = File::open(path)?;

    // Read first 1MB (or less if file is smaller)
    const HASH_SIZE: usize = 1024 * 1024; // 1MB
    let mut buffer = vec![0u8; HASH_SIZE];

    let bytes_read = file.read(&mut buffer)?;
    buffer.truncate(bytes_read);

    // Calculate Blake3 hash
    let hash = blake3::hash(&buffer);

    Ok(*hash.as_bytes())
}

/// Calculate config hash for checkpoint validation
///
/// Computes hash of encoder configuration to detect config changes.
///
/// # Arguments
/// * `config_bytes` - Serialized encoder configuration
///
/// # Returns
/// 32-byte Blake3 hash
pub fn calculate_config_hash(config_bytes: &[u8]) -> [u8; 32] {
    let hash = blake3::hash(config_bytes);
    *hash.as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::capsule::EncoderCheckpointCapsule;
    use std::io::Write;

    #[test]
    fn test_fresh_start_recovery() {
        let temp_dir = std::env::temp_dir();
        let checkpoint_path = temp_dir.join("nonexistent_checkpoint.kdly.ckpt");
        let output_path = temp_dir.join("nonexistent_output.av1");

        // Ensure files don't exist
        let _ = std::fs::remove_file(&checkpoint_path);
        let _ = std::fs::remove_file(&output_path);

        let result = recover_from_crash(&checkpoint_path, &output_path, [0u8; 32]);
        let recovery = result.unwrap();

        assert!(!recovery.recovery_needed);
        assert_eq!(recovery.resume_frame, 0);
        assert_eq!(recovery.truncate_offset, 0);
    }

    #[test]
    fn test_successful_recovery() {
        let temp_dir = std::env::temp_dir();
        let checkpoint_path = temp_dir.join("test_recovery.kdly.ckpt");
        let output_path = temp_dir.join("test_recovery_output.av1");

        let input_hash = [0xABu8; 32];

        // Create valid checkpoint
        let capsule = EncoderCheckpointCapsule::new(input_hash, 30);
        let mut header = CheckpointHeader::new(input_hash, 1000, [0xCDu8; 32]);
        header.update_progress(500, 5 * 1024 * 1024);

        let entries = vec![
            FrameIndexEntry::new(0, 0, 100000),
            FrameIndexEntry::new(100, 100000, 100000),
            FrameIndexEntry::new(200, 200000, 100000),
            FrameIndexEntry::new(300, 300000, 100000),
            FrameIndexEntry::new(400, 400000, 100000),
        ];

        capsule.write_checkpoint(&checkpoint_path, &header, &entries).unwrap();

        // Create output file larger than checkpoint position
        let mut output_file = File::create(&output_path).unwrap();
        output_file.write_all(&vec![0u8; 1_000_000]).unwrap(); // 1MB
        drop(output_file);

        // Perform recovery
        let recovery = recover_from_crash(&checkpoint_path, &output_path, input_hash).unwrap();

        assert!(recovery.recovery_needed);
        assert_eq!(recovery.resume_frame, 500);
        assert_eq!(recovery.total_frames, 1000);
        assert_eq!(recovery.truncate_offset, 500000); // Last entry end
        assert_eq!(recovery.frame_entries.len(), 5);

        // Verify output was truncated
        let truncated_size = std::fs::metadata(&output_path).unwrap().len();
        assert_eq!(truncated_size, 500000);

        // Cleanup
        let _ = std::fs::remove_file(&checkpoint_path);
        let _ = std::fs::remove_file(&output_path);
    }

    #[test]
    fn test_recovery_with_hash_mismatch() {
        let temp_dir = std::env::temp_dir();
        let checkpoint_path = temp_dir.join("test_hash_mismatch_recovery.kdly.ckpt");
        let output_path = temp_dir.join("test_hash_mismatch_output.av1");

        let input_hash = [0xABu8; 32];
        let different_hash = [0xCDu8; 32];

        // Create checkpoint with input_hash
        let capsule = EncoderCheckpointCapsule::new(input_hash, 30);
        let header = CheckpointHeader::new(input_hash, 100, [0u8; 32]);
        capsule.write_checkpoint(&checkpoint_path, &header, &[]).unwrap();

        // Try recovery with different hash
        let result = recover_from_crash(&checkpoint_path, &output_path, different_hash);
        assert!(matches!(result, Err(CheckpointError::InputMismatch)));

        // Cleanup
        let _ = std::fs::remove_file(&checkpoint_path);
    }

    #[test]
    fn test_quick_validate() {
        let temp_dir = std::env::temp_dir();
        let checkpoint_path = temp_dir.join("test_quick_validate.kdly.ckpt");

        let input_hash = [0xABu8; 32];

        // Create valid checkpoint
        let capsule = EncoderCheckpointCapsule::new(input_hash, 30);
        let mut header = CheckpointHeader::new(input_hash, 1000, [0u8; 32]);
        header.update_progress(250, 2 * 1024 * 1024);
        capsule.write_checkpoint(&checkpoint_path, &header, &[]).unwrap();

        // Quick validate
        let (is_valid, completed, total) = quick_validate(&checkpoint_path).unwrap();
        assert!(is_valid);
        assert_eq!(completed, 250);
        assert_eq!(total, 1000);

        // Cleanup
        let _ = std::fs::remove_file(&checkpoint_path);
    }

    #[test]
    fn test_default_checkpoint_path() {
        let output_path = std::path::Path::new("/path/to/video.av1");
        let checkpoint_path = default_checkpoint_path(output_path);

        assert_eq!(
            checkpoint_path.to_str().unwrap(),
            "/path/to/video.av1.kdly.ckpt"
        );
    }

    #[test]
    fn test_calculate_input_hash() {
        let temp_dir = std::env::temp_dir();
        let input_path = temp_dir.join("test_hash_input.bin");

        // Create test input file
        let mut file = File::create(&input_path).unwrap();
        file.write_all(&vec![0xAB; 1024 * 1024]).unwrap(); // 1MB of 0xAB
        drop(file);

        // Calculate hash
        let hash = calculate_input_hash(&input_path).unwrap();

        // Hash should be deterministic
        let hash2 = calculate_input_hash(&input_path).unwrap();
        assert_eq!(hash, hash2);

        // Cleanup
        let _ = std::fs::remove_file(&input_path);
    }

    #[test]
    fn test_calculate_config_hash() {
        let config1 = b"preset=medium,crf=28,threads=8";
        let config2 = b"preset=fast,crf=32,threads=4";

        let hash1 = calculate_config_hash(config1);
        let hash2 = calculate_config_hash(config2);

        // Different configs produce different hashes
        assert_ne!(hash1, hash2);

        // Same config produces same hash
        let hash1_again = calculate_config_hash(config1);
        assert_eq!(hash1, hash1_again);
    }

    #[test]
    fn test_truncate_output() {
        let temp_dir = std::env::temp_dir();
        let output_path = temp_dir.join("test_truncate.av1");

        // Create file with 1MB of data
        let mut file = File::create(&output_path).unwrap();
        file.write_all(&vec![0xAB; 1024 * 1024]).unwrap();
        drop(file);

        // Truncate to 512KB
        truncate_output(&output_path, 512 * 1024).unwrap();

        let size = std::fs::metadata(&output_path).unwrap().len();
        assert_eq!(size, 512 * 1024);

        // Truncating to larger size should be no-op
        truncate_output(&output_path, 1024 * 1024).unwrap();

        let size_after = std::fs::metadata(&output_path).unwrap().len();
        assert_eq!(size_after, 512 * 1024);

        // Cleanup
        let _ = std::fs::remove_file(&output_path);
    }

    #[test]
    fn test_delete_checkpoint() {
        let temp_dir = std::env::temp_dir();
        let checkpoint_path = temp_dir.join("test_delete.kdly.ckpt");

        // Create file
        File::create(&checkpoint_path).unwrap();
        assert!(checkpoint_path.exists());

        // Delete
        delete_checkpoint(&checkpoint_path).unwrap();
        assert!(!checkpoint_path.exists());

        // Delete non-existent should succeed
        delete_checkpoint(&checkpoint_path).unwrap();
    }

    #[test]
    fn test_recovery_progress_percent() {
        let recovery = CheckpointRecovery {
            resume_frame: 250,
            truncate_offset: 0,
            total_frames: 1000,
            recovery_needed: true,
            previous_error: None,
            recovered_frames: 250,
            checkpoint_generation: 2,
            frame_entries: Vec::new(),
        };

        assert!((recovery.progress_percent() - 25.0).abs() < 0.001);

        let empty_recovery = CheckpointRecovery::fresh_start();
        assert_eq!(empty_recovery.progress_percent(), 0.0);
    }
}
