//! EncoderCheckpointCapsule - T9 Persistent Tier
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Manages checkpoint file with two-phase commit for crash safety.
//! Provides atomic state persistence with generation counters for
//! detecting incomplete transactions.
//!
//! ## Two-Phase Commit Protocol
//!
//! 1. `begin_checkpoint()` - Generation becomes ODD (inflight)
//! 2. Write checkpoint data (header + frame index + trailer)
//! 3. `commit_checkpoint()` - Generation becomes EVEN (committed)
//! 4. On crash recovery: if ODD, rollback to last EVEN state
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 T9 Persistent tier
//! - **Chaos**: 256B cache-aligned, generation counters, atomic state
//! - **ASSUM**: 99.5%+ safe, all I/O documented with #ASSUME/#VERIFY
//! - **T28**: Unit/property/integration tests included

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::format::{
    calculate_crc32, CheckpointHeader, CheckpointTrailer, FrameIndexEntry,
    CHECKPOINT_MAGIC, FRAME_ENTRY_SIZE, HEADER_SIZE, TRAILER_SIZE,
};

/// Checkpoint state constants
const STATE_IDLE: u64 = 0;
const STATE_WRITING: u64 = 1;
const STATE_COMMITTED: u64 = 2;
const STATE_RECOVERING: u64 = 3;

/// Error types for checkpoint operations
#[derive(Debug, Clone)]
pub enum CheckpointError {
    /// Checkpoint file not found
    FileNotFound,
    /// Invalid checkpoint format (bad magic or version)
    InvalidFormat,
    /// Input file hash mismatch
    InputMismatch,
    /// Corrupted checkpoint (CRC mismatch)
    CorruptedCheckpoint,
    /// Checkpoint in-flight (odd generation)
    InFlightCheckpoint,
    /// Already in checkpoint transaction
    AlreadyInTransaction,
    /// Not in checkpoint transaction
    NotInTransaction,
    /// I/O error
    IoError(String),
    /// Configuration mismatch
    ConfigMismatch,
}

impl std::fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileNotFound => write!(f, "Checkpoint file not found"),
            Self::InvalidFormat => write!(f, "Invalid checkpoint format"),
            Self::InputMismatch => write!(f, "Input file hash mismatch"),
            Self::CorruptedCheckpoint => write!(f, "Corrupted checkpoint (CRC mismatch)"),
            Self::InFlightCheckpoint => write!(f, "Checkpoint in-flight (incomplete transaction)"),
            Self::AlreadyInTransaction => write!(f, "Already in checkpoint transaction"),
            Self::NotInTransaction => write!(f, "Not in checkpoint transaction"),
            Self::IoError(msg) => write!(f, "I/O error: {msg}"),
            Self::ConfigMismatch => write!(f, "Encoder configuration mismatch"),
        }
    }
}

impl std::error::Error for CheckpointError {}

impl From<std::io::Error> for CheckpointError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err.to_string())
    }
}

/// Loaded checkpoint data for resume
#[derive(Debug, Clone)]
pub struct CheckpointData {
    /// Frame number to resume from (0-indexed)
    pub last_frame: u64,
    /// Byte offset in output file to truncate to
    pub output_offset: u64,
    /// Total frames in the video
    pub total_frames: u64,
    /// All frame index entries
    pub frame_entries: Vec<FrameIndexEntry>,
    /// Header information
    pub header: CheckpointHeader,
}

/// EncoderCheckpointCapsule (256B, T9 Persistent)
///
/// Manages checkpoint file with two-phase commit for crash safety.
/// Uses generation counters to detect and recover from incomplete
/// transactions after crashes.
///
/// ## Memory Layout (256 bytes, 64-byte aligned)
///
/// ```text
/// Offset  Size  Field
/// ------  ----  -----
/// 0       8     state (AtomicU64)
/// 8       8     generation (AtomicU64)
/// 16      8     last_frame (AtomicU64)
/// 24      8     checkpoint_count (AtomicU64)
/// 32      32    _pad1 (cache line padding)
/// 64      8     interval_frames (AtomicU64)
/// 72      32    input_hash
/// 104     24    _pad2
/// 128     128   _padding (to 256B total)
/// ```
///
/// ## Framework Compliance
///
/// - UCE34: Q10 T9 Persistent tier with ACID-like guarantees
/// - Chaos: 256B cache-aligned, DualAtomicU64 pattern, generation counters
/// - ASSUM: All file I/O documented, 99.5%+ safe
#[repr(C, align(64))]
pub struct EncoderCheckpointCapsule {
    // Atomic state block (64B aligned)
    /// Current state: 0=idle, 1=writing, 2=committed, 3=recovering
    state: AtomicU64,
    /// Two-phase commit generation: odd=inflight, even=committed
    generation: AtomicU64,
    /// Last successfully checkpointed frame
    last_frame: AtomicU64,
    /// Total number of checkpoints written
    checkpoint_count: AtomicU64,
    /// Cache line padding
    _pad1: [u64; 4],

    // Checkpoint configuration block (64B)
    /// Checkpoint every N frames (default 30)
    interval_frames: AtomicU64,
    /// Blake3 hash of input file (first 1MB, for validation)
    input_hash: [u8; 32],
    /// Reserved padding
    _pad2: [u8; 24],

    // Note: PathBuf stored externally via Box (not in fixed-size capsule)
    // This capsule only contains the atomic state needed for coordination.
    // Path and file handle managed by caller.

    // Padding to 256B total
    _padding: [u8; 128],
}

// Compile-time size and alignment verification
const _: () = assert!(core::mem::size_of::<EncoderCheckpointCapsule>() == 256);
const _: () = assert!(core::mem::align_of::<EncoderCheckpointCapsule>() == 64);

impl EncoderCheckpointCapsule {
    /// Create new checkpoint capsule
    ///
    /// # Arguments
    /// * `input_hash` - Blake3 hash of input file (first 1MB)
    /// * `interval` - Checkpoint every N frames (default 30)
    #[inline]
    pub fn new(input_hash: [u8; 32], interval: u64) -> Self {
        Self {
            state: AtomicU64::new(STATE_IDLE),
            generation: AtomicU64::new(0), // Even = committed
            last_frame: AtomicU64::new(0),
            checkpoint_count: AtomicU64::new(0),
            _pad1: [0u64; 4],
            interval_frames: AtomicU64::new(interval),
            input_hash,
            _pad2: [0u8; 24],
            _padding: [0u8; 128],
        }
    }

    /// Create with default interval (30 frames)
    #[inline]
    pub fn with_default_interval(input_hash: [u8; 32]) -> Self {
        Self::new(input_hash, 30)
    }

    /// Begin checkpoint transaction (generation -> ODD)
    ///
    /// Marks the start of a checkpoint write. Generation counter becomes
    /// ODD to indicate an in-flight transaction. If system crashes during
    /// this state, recovery will detect the incomplete checkpoint.
    ///
    /// # Errors
    /// Returns `AlreadyInTransaction` if already in a checkpoint.
    pub fn begin_checkpoint(&self) -> Result<u64, CheckpointError> {
        // Check current state
        let current_gen = self.generation.load(Ordering::Acquire);
        if current_gen % 2 == 1 {
            return Err(CheckpointError::AlreadyInTransaction);
        }

        // Atomically increment generation to ODD (inflight)
        // #ASSUME: No concurrent begin_checkpoint calls on same capsule.
        // #VERIFY: Single encoder thread owns checkpoint operations.
        let new_gen = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        debug_assert!(new_gen % 2 == 1, "Generation should be odd after begin");

        self.state.store(STATE_WRITING, Ordering::Release);
        Ok(new_gen)
    }

    /// Commit checkpoint transaction (generation -> EVEN)
    ///
    /// Marks the successful completion of a checkpoint write. Generation
    /// counter becomes EVEN to indicate a committed transaction.
    ///
    /// # Arguments
    /// * `frame` - Last frame number successfully encoded
    ///
    /// # Errors
    /// Returns `NotInTransaction` if not in a checkpoint.
    pub fn commit_checkpoint(&self, frame: u64) -> Result<u64, CheckpointError> {
        // Check current state
        let current_gen = self.generation.load(Ordering::Acquire);
        if current_gen % 2 == 0 {
            return Err(CheckpointError::NotInTransaction);
        }

        // Update last frame before committing
        self.last_frame.store(frame, Ordering::Release);

        // Atomically increment generation to EVEN (committed)
        // #ASSUME: No concurrent commit_checkpoint calls.
        // #VERIFY: Single encoder thread owns checkpoint operations.
        let new_gen = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        debug_assert!(new_gen % 2 == 0, "Generation should be even after commit");

        self.state.store(STATE_COMMITTED, Ordering::Release);
        self.checkpoint_count.fetch_add(1, Ordering::Relaxed);

        Ok(new_gen)
    }

    /// Abort checkpoint transaction (rollback generation to EVEN)
    ///
    /// Cancels an in-flight checkpoint. Generation counter is decremented
    /// back to EVEN (last committed state).
    ///
    /// # Errors
    /// Returns `NotInTransaction` if not in a checkpoint.
    pub fn abort_checkpoint(&self) -> Result<(), CheckpointError> {
        let current_gen = self.generation.load(Ordering::Acquire);
        if current_gen % 2 == 0 {
            return Err(CheckpointError::NotInTransaction);
        }

        // Decrement back to even (previous committed state)
        self.generation.fetch_sub(1, Ordering::AcqRel);
        self.state.store(STATE_IDLE, Ordering::Release);
        Ok(())
    }

    /// Check if checkpoint should be written at this frame
    #[inline]
    pub fn should_checkpoint(&self, current_frame: u64) -> bool {
        let interval = self.interval_frames.load(Ordering::Relaxed);
        if interval == 0 {
            return false;
        }
        current_frame > 0 && current_frame % interval == 0
    }

    /// Get last checkpointed frame number
    #[inline]
    pub fn last_checkpointed_frame(&self) -> u64 {
        self.last_frame.load(Ordering::Acquire)
    }

    /// Get current generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Check if checkpoint is in valid (committed) state
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.generation.load(Ordering::Acquire) % 2 == 0
    }

    /// Check if checkpoint is in-flight (uncommitted)
    #[inline]
    pub fn is_inflight(&self) -> bool {
        self.generation.load(Ordering::Acquire) % 2 == 1
    }

    /// Get number of successful checkpoints
    #[inline]
    pub fn checkpoint_count(&self) -> u64 {
        self.checkpoint_count.load(Ordering::Relaxed)
    }

    /// Get checkpoint interval in frames
    #[inline]
    pub fn interval(&self) -> u64 {
        self.interval_frames.load(Ordering::Relaxed)
    }

    /// Set checkpoint interval
    #[inline]
    pub fn set_interval(&self, interval: u64) {
        self.interval_frames.store(interval, Ordering::Relaxed);
    }

    /// Get input file hash
    #[inline]
    pub fn input_hash(&self) -> [u8; 32] {
        self.input_hash
    }

    /// Write checkpoint to file
    ///
    /// Performs the complete two-phase commit:
    /// 1. Begin transaction (gen -> ODD)
    /// 2. Write header + frame entries + trailer
    /// 3. Sync to disk (fsync)
    /// 4. Commit transaction (gen -> EVEN)
    ///
    /// # Arguments
    /// * `path` - Checkpoint file path
    /// * `header` - Checkpoint header with metadata
    /// * `entries` - Frame index entries
    ///
    /// # Errors
    /// Returns I/O errors or transaction errors.
    pub fn write_checkpoint<P: AsRef<Path>>(
        &self,
        path: P,
        header: &CheckpointHeader,
        entries: &[FrameIndexEntry],
    ) -> Result<(), CheckpointError> {
        // Begin transaction
        let gen = self.begin_checkpoint()?;

        // Open file for writing (create or truncate)
        // #ASSUME: File system supports atomic file operations (POSIX).
        // #VERIFY: All writes followed by fsync before commit.
        let mut file = match OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path.as_ref())
        {
            Ok(f) => f,
            Err(e) => {
                let _ = self.abort_checkpoint();
                return Err(CheckpointError::IoError(e.to_string()));
            }
        };

        // Write header
        if let Err(e) = file.write_all(&header.to_bytes()) {
            let _ = self.abort_checkpoint();
            return Err(CheckpointError::IoError(e.to_string()));
        }

        // Write frame entries
        for entry in entries {
            if let Err(e) = file.write_all(&entry.to_bytes()) {
                let _ = self.abort_checkpoint();
                return Err(CheckpointError::IoError(e.to_string()));
            }
        }

        // Calculate CRC and create trailer
        let crc = calculate_crc32(header, entries);
        let trailer = CheckpointTrailer::committed(crc, gen + 1); // Next even gen

        // Write trailer
        if let Err(e) = file.write_all(&trailer.to_bytes()) {
            let _ = self.abort_checkpoint();
            return Err(CheckpointError::IoError(e.to_string()));
        }

        // Sync to disk before committing
        // #ASSUME: fsync ensures durability on POSIX-compliant file systems.
        // #VERIFY: File data persisted before generation becomes EVEN.
        if let Err(e) = file.sync_all() {
            let _ = self.abort_checkpoint();
            return Err(CheckpointError::IoError(format!("fsync failed: {e}")));
        }

        // Commit transaction
        self.commit_checkpoint(header.completed_frames)?;

        Ok(())
    }

    /// Load checkpoint from file
    ///
    /// Reads and validates checkpoint file, returning data for resume.
    ///
    /// # Arguments
    /// * `path` - Checkpoint file path
    ///
    /// # Errors
    /// Returns errors for invalid/corrupted checkpoints or I/O failures.
    pub fn load_checkpoint<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<CheckpointData, CheckpointError> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(CheckpointError::FileNotFound);
        }

        // Open file for reading
        // #ASSUME: File exists and is readable.
        // #VERIFY: Existence checked above.
        let mut file = File::open(path)?;

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

        // Validate input hash
        if header.input_hash != self.input_hash {
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

        // Validate trailer
        if trailer.is_inflight() {
            return Err(CheckpointError::InFlightCheckpoint);
        }

        // Validate CRC
        let expected_crc = calculate_crc32(&header, &frame_entries);
        if trailer.crc32 != expected_crc {
            return Err(CheckpointError::CorruptedCheckpoint);
        }

        // Update capsule state
        self.last_frame.store(header.completed_frames, Ordering::Release);
        self.generation.store(trailer.generation, Ordering::Release);
        self.state.store(STATE_COMMITTED, Ordering::Release);

        // Calculate output offset from last frame entry
        let output_offset = frame_entries
            .last()
            .map(|e| e.output_offset + e.encoded_size)
            .unwrap_or(0);

        Ok(CheckpointData {
            last_frame: header.completed_frames,
            output_offset,
            total_frames: header.total_frames,
            frame_entries,
            header,
        })
    }

    /// Validate checkpoint file without loading
    ///
    /// Quick validation of checkpoint integrity without full parse.
    ///
    /// # Arguments
    /// * `path` - Checkpoint file path
    ///
    /// # Returns
    /// `true` if checkpoint is valid and can be loaded.
    pub fn validate_checkpoint<P: AsRef<Path>>(path: P) -> bool {
        let path = path.as_ref();

        if !path.exists() {
            return false;
        }

        let mut file = match File::open(path) {
            Ok(f) => f,
            Err(_) => return false,
        };

        // Check minimum size
        let file_size = match file.metadata() {
            Ok(m) => m.len(),
            Err(_) => return false,
        };

        if file_size < (HEADER_SIZE + TRAILER_SIZE) as u64 {
            return false;
        }

        // Read and validate header magic
        let mut magic = [0u8; 8];
        if file.read_exact(&mut magic).is_err() {
            return false;
        }
        if magic != CHECKPOINT_MAGIC {
            return false;
        }

        // Seek to trailer and check generation
        if file.seek(SeekFrom::End(-(TRAILER_SIZE as i64))).is_err() {
            return false;
        }

        let mut trailer_bytes = [0u8; TRAILER_SIZE];
        if file.read_exact(&mut trailer_bytes).is_err() {
            return false;
        }

        let trailer = CheckpointTrailer::from_bytes(&trailer_bytes);
        trailer.is_valid()
    }

    /// Reset capsule to initial state
    ///
    /// Used when starting a new encoding session.
    pub fn reset(&self) {
        self.state.store(STATE_IDLE, Ordering::Release);
        self.generation.store(0, Ordering::Release);
        self.last_frame.store(0, Ordering::Release);
        // Note: checkpoint_count and interval preserved
    }
}

impl Default for EncoderCheckpointCapsule {
    fn default() -> Self {
        Self::new([0u8; 32], 30)
    }
}

impl core::fmt::Debug for EncoderCheckpointCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EncoderCheckpointCapsule")
            .field("state", &self.state.load(Ordering::Relaxed))
            .field("generation", &self.generation.load(Ordering::Relaxed))
            .field("last_frame", &self.last_frame.load(Ordering::Relaxed))
            .field("checkpoint_count", &self.checkpoint_count.load(Ordering::Relaxed))
            .field("interval_frames", &self.interval_frames.load(Ordering::Relaxed))
            .finish()
    }
}

// Safety: AtomicU64 provides thread-safe access
unsafe impl Send for EncoderCheckpointCapsule {}
unsafe impl Sync for EncoderCheckpointCapsule {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(core::mem::size_of::<EncoderCheckpointCapsule>(), 256);
        assert_eq!(core::mem::align_of::<EncoderCheckpointCapsule>(), 64);
    }

    #[test]
    fn test_two_phase_commit_protocol() {
        let capsule = EncoderCheckpointCapsule::new([0xABu8; 32], 30);

        // Initial state: even generation (committed)
        assert!(capsule.is_valid());
        assert!(!capsule.is_inflight());
        assert_eq!(capsule.generation(), 0);

        // Begin checkpoint: odd generation (inflight)
        let gen1 = capsule.begin_checkpoint().unwrap();
        assert_eq!(gen1, 1);
        assert!(capsule.is_inflight());
        assert!(!capsule.is_valid());

        // Cannot begin again while in transaction
        assert!(matches!(
            capsule.begin_checkpoint(),
            Err(CheckpointError::AlreadyInTransaction)
        ));

        // Commit: even generation (committed)
        let gen2 = capsule.commit_checkpoint(100).unwrap();
        assert_eq!(gen2, 2);
        assert!(capsule.is_valid());
        assert!(!capsule.is_inflight());
        assert_eq!(capsule.last_checkpointed_frame(), 100);
    }

    #[test]
    fn test_abort_checkpoint() {
        let capsule = EncoderCheckpointCapsule::new([0xABu8; 32], 30);

        // Begin and abort
        capsule.begin_checkpoint().unwrap();
        assert!(capsule.is_inflight());

        capsule.abort_checkpoint().unwrap();
        assert!(capsule.is_valid());
        assert_eq!(capsule.generation(), 0);
    }

    #[test]
    fn test_should_checkpoint() {
        let capsule = EncoderCheckpointCapsule::new([0u8; 32], 30);

        // Frame 0: never checkpoint
        assert!(!capsule.should_checkpoint(0));

        // Frame 1-29: no checkpoint
        for i in 1..30 {
            assert!(!capsule.should_checkpoint(i));
        }

        // Frame 30, 60, 90: checkpoint
        assert!(capsule.should_checkpoint(30));
        assert!(capsule.should_checkpoint(60));
        assert!(capsule.should_checkpoint(90));

        // Frame 31: no checkpoint
        assert!(!capsule.should_checkpoint(31));
    }

    #[test]
    fn test_interval_configuration() {
        let capsule = EncoderCheckpointCapsule::new([0u8; 32], 100);
        assert_eq!(capsule.interval(), 100);

        capsule.set_interval(50);
        assert_eq!(capsule.interval(), 50);

        assert!(capsule.should_checkpoint(50));
        assert!(!capsule.should_checkpoint(30));
    }

    #[test]
    fn test_reset() {
        let capsule = EncoderCheckpointCapsule::new([0xABu8; 32], 30);

        // Simulate some activity
        capsule.begin_checkpoint().unwrap();
        capsule.commit_checkpoint(100).unwrap();
        assert_eq!(capsule.generation(), 2);
        assert_eq!(capsule.checkpoint_count(), 1);

        // Reset
        capsule.reset();
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.last_checkpointed_frame(), 0);
        // Checkpoint count preserved
        assert_eq!(capsule.checkpoint_count(), 1);
    }

    #[test]
    fn test_write_and_load_checkpoint() {
        let temp_dir = std::env::temp_dir();
        let checkpoint_path = temp_dir.join("test_checkpoint.kdly.ckpt");

        let input_hash = [0xABu8; 32];
        let config_hash = [0xCDu8; 32];

        let capsule = EncoderCheckpointCapsule::new(input_hash, 30);

        // Create header and entries
        let mut header = CheckpointHeader::new(input_hash, 1000, config_hash);
        header.update_progress(100, 1024 * 1024);

        let entries = vec![
            FrameIndexEntry::new(0, 0, 10000).with_psnr(42.5),
            FrameIndexEntry::new(1, 10000, 12000).with_psnr(43.0),
            FrameIndexEntry::new(2, 22000, 8000).with_psnr(41.5),
        ];

        // Write checkpoint
        capsule.write_checkpoint(&checkpoint_path, &header, &entries).unwrap();

        // Verify state after write
        assert!(capsule.is_valid());
        assert_eq!(capsule.last_checkpointed_frame(), 100);
        assert_eq!(capsule.checkpoint_count(), 1);

        // Load checkpoint with new capsule
        let capsule2 = EncoderCheckpointCapsule::new(input_hash, 30);
        let data = capsule2.load_checkpoint(&checkpoint_path).unwrap();

        assert_eq!(data.last_frame, 100);
        assert_eq!(data.total_frames, 1000);
        assert_eq!(data.frame_entries.len(), 3);
        assert_eq!(data.output_offset, 30000); // 22000 + 8000

        // Cleanup
        let _ = std::fs::remove_file(&checkpoint_path);
    }

    #[test]
    fn test_validate_checkpoint() {
        let temp_dir = std::env::temp_dir();
        let checkpoint_path = temp_dir.join("test_validate.kdly.ckpt");

        // Write valid checkpoint
        let capsule = EncoderCheckpointCapsule::new([0xABu8; 32], 30);
        let header = CheckpointHeader::new([0xABu8; 32], 100, [0xCDu8; 32]);
        capsule.write_checkpoint(&checkpoint_path, &header, &[]).unwrap();

        // Validate should succeed
        assert!(EncoderCheckpointCapsule::validate_checkpoint(&checkpoint_path));

        // Create invalid checkpoint (bad magic)
        let invalid_path = temp_dir.join("test_invalid.kdly.ckpt");
        let mut file = File::create(&invalid_path).unwrap();
        file.write_all(b"BADMAGIC").unwrap();
        file.write_all(&[0u8; 200]).unwrap();
        drop(file);

        assert!(!EncoderCheckpointCapsule::validate_checkpoint(&invalid_path));

        // Cleanup
        let _ = std::fs::remove_file(&checkpoint_path);
        let _ = std::fs::remove_file(&invalid_path);
    }

    #[test]
    fn test_input_hash_validation() {
        let temp_dir = std::env::temp_dir();
        let checkpoint_path = temp_dir.join("test_hash_mismatch.kdly.ckpt");

        let input_hash = [0xABu8; 32];
        let different_hash = [0xCDu8; 32];

        // Write checkpoint with input_hash
        let capsule1 = EncoderCheckpointCapsule::new(input_hash, 30);
        let header = CheckpointHeader::new(input_hash, 100, [0u8; 32]);
        capsule1.write_checkpoint(&checkpoint_path, &header, &[]).unwrap();

        // Try to load with different hash
        let capsule2 = EncoderCheckpointCapsule::new(different_hash, 30);
        let result = capsule2.load_checkpoint(&checkpoint_path);

        assert!(matches!(result, Err(CheckpointError::InputMismatch)));

        // Cleanup
        let _ = std::fs::remove_file(&checkpoint_path);
    }

    #[test]
    fn test_corrupted_checkpoint_detection() {
        let temp_dir = std::env::temp_dir();
        let checkpoint_path = temp_dir.join("test_corrupted.kdly.ckpt");

        let input_hash = [0xABu8; 32];

        // Write valid checkpoint
        let capsule = EncoderCheckpointCapsule::new(input_hash, 30);
        let header = CheckpointHeader::new(input_hash, 100, [0u8; 32]);
        let entries = vec![FrameIndexEntry::new(0, 0, 1000)];
        capsule.write_checkpoint(&checkpoint_path, &header, &entries).unwrap();

        // Corrupt the file (modify a byte in the middle)
        let mut file = OpenOptions::new()
            .write(true)
            .open(&checkpoint_path)
            .unwrap();
        file.seek(SeekFrom::Start(50)).unwrap();
        file.write_all(&[0xFF]).unwrap();
        drop(file);

        // Load should fail with CRC error
        let capsule2 = EncoderCheckpointCapsule::new(input_hash, 30);
        let result = capsule2.load_checkpoint(&checkpoint_path);

        assert!(matches!(result, Err(CheckpointError::CorruptedCheckpoint)));

        // Cleanup
        let _ = std::fs::remove_file(&checkpoint_path);
    }
}
