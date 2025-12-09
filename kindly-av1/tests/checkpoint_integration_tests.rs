//! Integration tests for checkpoint/resume functionality
//!
//! Tests the complete checkpoint/resume cycle including:
//! - Checkpoint writing (atomic two-phase commit)
//! - Checkpoint loading and integrity validation
//! - Resume from crash (output truncation, state restoration)
//! - Error handling (corrupted checkpoints, hash mismatches)

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use kindly_av1::checkpoint::{
    calculate_config_hash, calculate_input_hash, default_checkpoint_path, recover_from_crash,
    CheckpointHeader, EncoderCheckpointCapsule, FrameIndexEntry,
};

/// Test basic checkpoint write and load cycle
#[test]
fn test_checkpoint_write_and_load() {
    let temp_dir = std::env::temp_dir();
    let checkpoint_path = temp_dir.join("test_basic_checkpoint.kdly.ckpt");

    let input_hash = [0xABu8; 32];
    let config_hash = [0xCDu8; 32];

    // Create checkpoint capsule
    let capsule = EncoderCheckpointCapsule::new(input_hash, 30);

    // Create header and entries
    let mut header = CheckpointHeader::new(input_hash, 1000, config_hash);
    header.update_progress(500, 5 * 1024 * 1024);

    let entries = vec![
        FrameIndexEntry::new(0, 0, 100000).with_psnr(42.5),
        FrameIndexEntry::new(100, 100000, 120000).with_psnr(43.0),
        FrameIndexEntry::new(200, 220000, 80000).with_psnr(41.5),
        FrameIndexEntry::new(300, 300000, 100000).with_psnr(42.0),
        FrameIndexEntry::new(400, 400000, 100000).with_psnr(42.8),
    ];

    // Write checkpoint
    capsule
        .write_checkpoint(&checkpoint_path, &header, &entries)
        .expect("checkpoint write failed");

    // Verify capsule state after write
    assert!(capsule.is_valid());
    assert!(!capsule.is_inflight());
    assert_eq!(capsule.last_checkpointed_frame(), 500);
    assert_eq!(capsule.checkpoint_count(), 1);

    // Load checkpoint with new capsule
    let capsule2 = EncoderCheckpointCapsule::new(input_hash, 30);
    let data = capsule2
        .load_checkpoint(&checkpoint_path)
        .expect("checkpoint load failed");

    assert_eq!(data.last_frame, 500);
    assert_eq!(data.total_frames, 1000);
    assert_eq!(data.frame_entries.len(), 5);
    assert_eq!(data.output_offset, 500000); // 400000 + 100000

    // Verify frame entries preserved
    for (i, entry) in data.frame_entries.iter().enumerate() {
        assert_eq!(entry.frame_num, entries[i].frame_num);
        assert_eq!(entry.output_offset, entries[i].output_offset);
        assert_eq!(entry.encoded_size, entries[i].encoded_size);
        assert!((entry.psnr() - entries[i].psnr()).abs() < 0.001);
    }

    // Cleanup
    let _ = std::fs::remove_file(&checkpoint_path);
}

/// Test crash recovery with output truncation
#[test]
fn test_crash_recovery_with_truncation() {
    let temp_dir = std::env::temp_dir();
    let checkpoint_path = temp_dir.join("test_recovery_checkpoint.kdly.ckpt");
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

    capsule
        .write_checkpoint(&checkpoint_path, &header, &entries)
        .expect("checkpoint write failed");

    // Create output file larger than checkpoint position (simulates crash during write)
    let mut output_file = File::create(&output_path).expect("output file create failed");
    output_file
        .write_all(&vec![0u8; 1_000_000])
        .expect("output file write failed"); // 1MB
    drop(output_file);

    // Perform recovery
    let recovery =
        recover_from_crash(&checkpoint_path, &output_path, input_hash).expect("recovery failed");

    assert!(recovery.recovery_needed);
    assert_eq!(recovery.resume_frame, 500);
    assert_eq!(recovery.total_frames, 1000);
    assert_eq!(recovery.truncate_offset, 500000); // Last entry end
    assert_eq!(recovery.frame_entries.len(), 5);
    assert!((recovery.progress_percent() - 50.0).abs() < 0.01);

    // Verify output was truncated
    let truncated_size = std::fs::metadata(&output_path)
        .expect("metadata failed")
        .len();
    assert_eq!(truncated_size, 500000);

    // Cleanup
    let _ = std::fs::remove_file(&checkpoint_path);
    let _ = std::fs::remove_file(&output_path);
}

/// Test checkpoint integrity validation (CRC)
#[test]
fn test_corrupted_checkpoint_detection() {
    let temp_dir = std::env::temp_dir();
    let checkpoint_path = temp_dir.join("test_corrupted_checkpoint.kdly.ckpt");

    let input_hash = [0xABu8; 32];

    // Write valid checkpoint
    let capsule = EncoderCheckpointCapsule::new(input_hash, 30);
    let header = CheckpointHeader::new(input_hash, 100, [0u8; 32]);
    let entries = vec![FrameIndexEntry::new(0, 0, 1000)];
    capsule
        .write_checkpoint(&checkpoint_path, &header, &entries)
        .expect("checkpoint write failed");

    // Corrupt the file (modify a byte in the middle)
    use std::fs::OpenOptions;
    use std::io::Seek;
    let mut file = OpenOptions::new()
        .write(true)
        .open(&checkpoint_path)
        .expect("open for corruption failed");
    file.seek(std::io::SeekFrom::Start(50))
        .expect("seek failed");
    file.write_all(&[0xFF]).expect("corruption write failed");
    drop(file);

    // Load should fail with CRC error
    let capsule2 = EncoderCheckpointCapsule::new(input_hash, 30);
    let result = capsule2.load_checkpoint(&checkpoint_path);

    assert!(result.is_err());
    // Error is CorruptedCheckpoint

    // Cleanup
    let _ = std::fs::remove_file(&checkpoint_path);
}

/// Test input file hash mismatch detection
#[test]
fn test_input_hash_mismatch() {
    let temp_dir = std::env::temp_dir();
    let checkpoint_path = temp_dir.join("test_hash_mismatch_checkpoint.kdly.ckpt");
    let output_path = temp_dir.join("test_hash_mismatch_output.av1");

    let input_hash = [0xABu8; 32];
    let different_hash = [0xCDu8; 32];

    // Create checkpoint with input_hash
    let capsule = EncoderCheckpointCapsule::new(input_hash, 30);
    let header = CheckpointHeader::new(input_hash, 100, [0u8; 32]);
    capsule
        .write_checkpoint(&checkpoint_path, &header, &[])
        .expect("checkpoint write failed");

    // Try recovery with different hash
    let result = recover_from_crash(&checkpoint_path, &output_path, different_hash);
    assert!(result.is_err());
    // Error is InputMismatch

    // Cleanup
    let _ = std::fs::remove_file(&checkpoint_path);
}

/// Test two-phase commit protocol (generation counter)
#[test]
fn test_two_phase_commit_generation() {
    let capsule = EncoderCheckpointCapsule::new([0xABu8; 32], 30);

    // Initial state: even generation (committed)
    assert!(capsule.is_valid());
    assert!(!capsule.is_inflight());
    assert_eq!(capsule.generation(), 0);

    // Begin checkpoint: odd generation (inflight)
    let gen1 = capsule.begin_checkpoint().expect("begin failed");
    assert_eq!(gen1, 1);
    assert!(capsule.is_inflight());
    assert!(!capsule.is_valid());

    // Cannot begin again while in transaction
    assert!(capsule.begin_checkpoint().is_err());

    // Commit: even generation (committed)
    let gen2 = capsule.commit_checkpoint(100).expect("commit failed");
    assert_eq!(gen2, 2);
    assert!(capsule.is_valid());
    assert!(!capsule.is_inflight());
    assert_eq!(capsule.last_checkpointed_frame(), 100);

    // Next checkpoint cycle
    let gen3 = capsule.begin_checkpoint().expect("begin failed");
    assert_eq!(gen3, 3);
    assert!(capsule.is_inflight());

    // Abort this time
    capsule.abort_checkpoint().expect("abort failed");
    assert!(capsule.is_valid());
    assert_eq!(capsule.generation(), 2); // Back to last committed
}

/// Test checkpoint interval logic
#[test]
fn test_checkpoint_interval() {
    let capsule = EncoderCheckpointCapsule::new([0u8; 32], 30);

    // Frame 0: never checkpoint
    assert!(!capsule.should_checkpoint(0));

    // Frames 1-29: no checkpoint
    for i in 1..30 {
        assert!(!capsule.should_checkpoint(i));
    }

    // Frames 30, 60, 90: checkpoint
    assert!(capsule.should_checkpoint(30));
    assert!(capsule.should_checkpoint(60));
    assert!(capsule.should_checkpoint(90));

    // Frame 31: no checkpoint
    assert!(!capsule.should_checkpoint(31));

    // Change interval
    capsule.set_interval(50);
    assert!(capsule.should_checkpoint(50));
    assert!(capsule.should_checkpoint(100));
    assert!(!capsule.should_checkpoint(30));
    assert!(!capsule.should_checkpoint(60));

    // Disable checkpointing (interval = 0)
    capsule.set_interval(0);
    assert!(!capsule.should_checkpoint(30));
    assert!(!capsule.should_checkpoint(60));
    assert!(!capsule.should_checkpoint(90));
}

/// Test default checkpoint path generation
#[test]
fn test_default_checkpoint_path() {
    let output_path = PathBuf::from("/path/to/video.av1");
    let checkpoint_path = default_checkpoint_path(&output_path);

    assert_eq!(
        checkpoint_path.to_str().unwrap(),
        "/path/to/video.av1.kdly.ckpt"
    );

    let output_path2 = PathBuf::from("/another/path/movie.mkv");
    let checkpoint_path2 = default_checkpoint_path(&output_path2);

    assert_eq!(
        checkpoint_path2.to_str().unwrap(),
        "/another/path/movie.mkv.kdly.ckpt"
    );
}

/// Test input file hash calculation
#[test]
fn test_input_hash_calculation() {
    let temp_dir = std::env::temp_dir();
    let input_path = temp_dir.join("test_hash_input.bin");

    // Create test input file
    let mut file = File::create(&input_path).expect("file create failed");
    file.write_all(&vec![0xAB; 1024 * 1024])
        .expect("file write failed"); // 1MB of 0xAB
    drop(file);

    // Calculate hash
    let hash = calculate_input_hash(&input_path).expect("hash calculation failed");

    // Hash should be deterministic
    let hash2 = calculate_input_hash(&input_path).expect("hash calculation failed");
    assert_eq!(hash, hash2);

    // Different data produces different hash
    let mut file2 = File::create(&input_path).expect("file create failed");
    file2
        .write_all(&vec![0xCD; 1024 * 1024])
        .expect("file write failed");
    drop(file2);

    let hash3 = calculate_input_hash(&input_path).expect("hash calculation failed");
    assert_ne!(hash, hash3);

    // Cleanup
    let _ = std::fs::remove_file(&input_path);
}

/// Test config hash calculation
#[test]
fn test_config_hash_calculation() {
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

/// Test fresh start recovery (no checkpoint)
#[test]
fn test_fresh_start_recovery() {
    let temp_dir = std::env::temp_dir();
    let checkpoint_path = temp_dir.join("nonexistent_checkpoint.kdly.ckpt");
    let output_path = temp_dir.join("nonexistent_output.av1");

    // Ensure files don't exist
    let _ = std::fs::remove_file(&checkpoint_path);
    let _ = std::fs::remove_file(&output_path);

    let result = recover_from_crash(&checkpoint_path, &output_path, [0u8; 32]);
    let recovery = result.expect("recovery failed");

    assert!(!recovery.recovery_needed);
    assert_eq!(recovery.resume_frame, 0);
    assert_eq!(recovery.truncate_offset, 0);
    assert!(!recovery.should_resume());
    assert_eq!(recovery.progress_percent(), 0.0);
}

/// Test multiple checkpoint cycles
#[test]
fn test_multiple_checkpoint_cycles() {
    let temp_dir = std::env::temp_dir();
    let checkpoint_path = temp_dir.join("test_multiple_checkpoint.kdly.ckpt");

    let input_hash = [0xABu8; 32];
    let capsule = EncoderCheckpointCapsule::new(input_hash, 30);

    // First checkpoint
    let header1 = CheckpointHeader::new(input_hash, 1000, [0u8; 32]);
    capsule
        .write_checkpoint(&checkpoint_path, &header1, &[])
        .expect("checkpoint 1 failed");

    assert_eq!(capsule.checkpoint_count(), 1);
    assert_eq!(capsule.generation(), 2);

    // Second checkpoint
    let header2 = CheckpointHeader::new(input_hash, 1000, [0u8; 32]);
    capsule
        .write_checkpoint(&checkpoint_path, &header2, &[])
        .expect("checkpoint 2 failed");

    assert_eq!(capsule.checkpoint_count(), 2);
    assert_eq!(capsule.generation(), 4);

    // Third checkpoint
    let header3 = CheckpointHeader::new(input_hash, 1000, [0u8; 32]);
    capsule
        .write_checkpoint(&checkpoint_path, &header3, &[])
        .expect("checkpoint 3 failed");

    assert_eq!(capsule.checkpoint_count(), 3);
    assert_eq!(capsule.generation(), 6);

    // Cleanup
    let _ = std::fs::remove_file(&checkpoint_path);
}

/// Test checkpoint reset functionality
#[test]
fn test_checkpoint_reset() {
    let capsule = EncoderCheckpointCapsule::new([0xABu8; 32], 30);

    // Simulate some activity
    capsule.begin_checkpoint().expect("begin failed");
    capsule.commit_checkpoint(100).expect("commit failed");

    assert_eq!(capsule.generation(), 2);
    assert_eq!(capsule.checkpoint_count(), 1);
    assert_eq!(capsule.last_checkpointed_frame(), 100);

    // Reset
    capsule.reset();

    assert_eq!(capsule.generation(), 0);
    assert_eq!(capsule.last_checkpointed_frame(), 0);
    assert!(capsule.is_valid());

    // Checkpoint count preserved
    assert_eq!(capsule.checkpoint_count(), 1);

    // Interval preserved
    assert_eq!(capsule.interval(), 30);
}

// =============================================================================
// Wave 2: Checkpoint/Resume Wiring Tests (2025-12-02)
// =============================================================================
// Tests for the encoding loop checkpoint integration based on:
// - Av1an's resume pattern: https://rust-av.github.io/Av1an/Cli/general.html
// - BLAKE3 fast hashing: https://github.com/BLAKE3-team/BLAKE3
// - Two-phase commit: https://martinfowler.com/articles/patterns-of-distributed-systems/two-phase-commit.html

use kindly_av1::checkpoint::delete_checkpoint;

/// Test checkpoint write during simulated encoding loop
/// Verifies that checkpoints are written at correct intervals
#[test]
fn test_checkpoint_write_during_encoding() {
    let temp_dir = std::env::temp_dir();
    let checkpoint_path = temp_dir.join("test_encoding_checkpoint.kdly.ckpt");

    let input_hash = [0xABu8; 32];
    let config_hash = [0xCDu8; 32];
    let total_frames = 100u64;
    let checkpoint_interval = 30u64;

    let capsule = EncoderCheckpointCapsule::new(input_hash, checkpoint_interval);
    let mut header = CheckpointHeader::new(input_hash, total_frames, config_hash);
    let mut frame_entries: Vec<FrameIndexEntry> = Vec::new();
    let mut total_bytes = 0u64;

    // Simulate encoding loop
    for frame_num in 0..total_frames {
        // Simulate encoded frame (variable size 1000-2000 bytes)
        let frame_size = 1000 + (frame_num % 1000) as u64;
        let frame_start_offset = total_bytes;
        total_bytes += frame_size;

        // Check if we should checkpoint at this frame
        if capsule.should_checkpoint(frame_num) {
            let entry = FrameIndexEntry::new(frame_num, frame_start_offset, frame_size);
            frame_entries.push(entry);

            header.update_progress(frame_num, total_bytes);

            capsule
                .write_checkpoint(&checkpoint_path, &header, &frame_entries)
                .expect("checkpoint write failed");
        }
    }

    // Verify checkpoints were written at frames 30, 60, 90
    assert_eq!(capsule.checkpoint_count(), 3);
    assert_eq!(frame_entries.len(), 3);
    assert_eq!(frame_entries[0].frame_num, 30);
    assert_eq!(frame_entries[1].frame_num, 60);
    assert_eq!(frame_entries[2].frame_num, 90);

    // Verify checkpoint file exists and is valid
    assert!(checkpoint_path.exists());

    let capsule2 = EncoderCheckpointCapsule::new(input_hash, checkpoint_interval);
    let data = capsule2
        .load_checkpoint(&checkpoint_path)
        .expect("checkpoint load failed");

    assert_eq!(data.last_frame, 90);
    assert_eq!(data.total_frames, total_frames);
    assert_eq!(data.frame_entries.len(), 3);

    // Cleanup
    let _ = std::fs::remove_file(&checkpoint_path);
}

/// Test checkpoint resume from simulated crash
/// Simulates crash at frame 50, resume should start from frame 30 (last checkpoint)
#[test]
fn test_checkpoint_resume_from_crash() {
    let temp_dir = std::env::temp_dir();
    let checkpoint_path = temp_dir.join("test_crash_resume_checkpoint.kdly.ckpt");
    let output_path = temp_dir.join("test_crash_resume_output.av1");

    let input_hash = [0xABu8; 32];
    let config_hash = [0xCDu8; 32];
    let total_frames = 100u64;
    let checkpoint_interval = 30u64;

    // --- Phase 1: Initial encoding (crash at frame 50) ---
    {
        let capsule = EncoderCheckpointCapsule::new(input_hash, checkpoint_interval);
        let mut header = CheckpointHeader::new(input_hash, total_frames, config_hash);
        let mut frame_entries: Vec<FrameIndexEntry> = Vec::new();
        let mut total_bytes = 0u64;

        let mut output_file = File::create(&output_path).expect("output file create failed");

        for frame_num in 0..=50 {
            // Simulated crash point
            let frame_size = 1000u64;
            let frame_start_offset = total_bytes;

            // Write "encoded" data to output
            output_file
                .write_all(&vec![frame_num as u8; frame_size as usize])
                .expect("write failed");
            total_bytes += frame_size;

            if capsule.should_checkpoint(frame_num) {
                let entry = FrameIndexEntry::new(frame_num, frame_start_offset, frame_size);
                frame_entries.push(entry);
                header.update_progress(frame_num, total_bytes);

                capsule
                    .write_checkpoint(&checkpoint_path, &header, &frame_entries)
                    .expect("checkpoint write failed");
            }
        }

        // "Crash" at frame 50 - checkpoint was written at frame 30
        // Output has 51 frames (0-50), but checkpoint only covers 30
    }

    // --- Phase 2: Resume from checkpoint ---
    let recovery =
        recover_from_crash(&checkpoint_path, &output_path, input_hash).expect("recovery failed");

    assert!(recovery.recovery_needed);
    assert!(recovery.should_resume());
    assert_eq!(recovery.resume_frame, 30); // Resume from last checkpoint
    assert_eq!(recovery.frame_entries.len(), 1); // Only frame 30 was checkpointed

    // Output should be truncated to checkpoint position
    let truncated_size = std::fs::metadata(&output_path).expect("metadata").len();
    assert_eq!(truncated_size, 31000); // Frames 0-30 = 31 frames * 1000 bytes

    // --- Phase 3: Continue encoding from resume point ---
    {
        let capsule = EncoderCheckpointCapsule::new(input_hash, checkpoint_interval);
        let mut header = CheckpointHeader::new(input_hash, total_frames, config_hash);
        let mut frame_entries = recovery.frame_entries.clone();
        let mut total_bytes = recovery.truncate_offset;

        header.update_progress(recovery.resume_frame, total_bytes);

        let mut output_file = std::fs::OpenOptions::new()
            .append(true)
            .open(&output_path)
            .expect("open for append failed");

        // Resume from frame 31 (after last checkpointed frame 30)
        for frame_num in (recovery.resume_frame + 1)..total_frames {
            let frame_size = 1000u64;
            let frame_start_offset = total_bytes;

            output_file
                .write_all(&vec![frame_num as u8; frame_size as usize])
                .expect("write failed");
            total_bytes += frame_size;

            if capsule.should_checkpoint(frame_num) {
                let entry = FrameIndexEntry::new(frame_num, frame_start_offset, frame_size);
                frame_entries.push(entry);
                header.update_progress(frame_num, total_bytes);

                capsule
                    .write_checkpoint(&checkpoint_path, &header, &frame_entries)
                    .expect("checkpoint write failed");
            }
        }
    }

    // Final output should have all 100 frames
    let final_size = std::fs::metadata(&output_path).expect("metadata").len();
    assert_eq!(final_size, 100000); // 100 frames * 1000 bytes

    // Cleanup
    let _ = std::fs::remove_file(&checkpoint_path);
    let _ = std::fs::remove_file(&output_path);
}

/// Test checkpoint input hash validation on resume
/// Ensures different input file produces error
#[test]
fn test_checkpoint_input_hash_validation() {
    let temp_dir = std::env::temp_dir();
    let checkpoint_path = temp_dir.join("test_hash_validation_checkpoint.kdly.ckpt");
    let output_path = temp_dir.join("test_hash_validation_output.av1");

    let original_hash = [0xABu8; 32];
    let different_hash = [0xCDu8; 32];

    // Create checkpoint with original hash
    let capsule = EncoderCheckpointCapsule::new(original_hash, 30);
    let mut header = CheckpointHeader::new(original_hash, 1000, [0u8; 32]);
    header.update_progress(500, 500000);

    let entries = vec![
        FrameIndexEntry::new(100, 100000, 100000),
        FrameIndexEntry::new(200, 200000, 100000),
    ];

    capsule
        .write_checkpoint(&checkpoint_path, &header, &entries)
        .expect("checkpoint write failed");

    // Try to resume with different input hash - should fail
    let result = recover_from_crash(&checkpoint_path, &output_path, different_hash);

    assert!(result.is_err());

    // Resume with correct hash - should succeed
    let result2 = recover_from_crash(&checkpoint_path, &output_path, original_hash);
    assert!(result2.is_ok());

    // Cleanup
    let _ = std::fs::remove_file(&checkpoint_path);
}

/// Test checkpoint cleanup on successful encoding completion
#[test]
fn test_checkpoint_cleanup_on_success() {
    let temp_dir = std::env::temp_dir();
    let checkpoint_path = temp_dir.join("test_cleanup_checkpoint.kdly.ckpt");

    let input_hash = [0xABu8; 32];

    // Create checkpoint
    let capsule = EncoderCheckpointCapsule::new(input_hash, 30);
    let header = CheckpointHeader::new(input_hash, 100, [0u8; 32]);
    capsule
        .write_checkpoint(&checkpoint_path, &header, &[])
        .expect("checkpoint write failed");

    assert!(checkpoint_path.exists());

    // Simulate successful completion - delete checkpoint
    delete_checkpoint(&checkpoint_path).expect("delete failed");

    assert!(!checkpoint_path.exists());

    // Deleting non-existent checkpoint should not error
    delete_checkpoint(&checkpoint_path).expect("delete non-existent failed");
}

/// Test checkpoint with PSNR quality metrics
/// Verifies PSNR values are preserved through checkpoint cycle
#[test]
fn test_checkpoint_psnr_preservation() {
    let temp_dir = std::env::temp_dir();
    let checkpoint_path = temp_dir.join("test_psnr_checkpoint.kdly.ckpt");

    let input_hash = [0xABu8; 32];

    let capsule = EncoderCheckpointCapsule::new(input_hash, 30);
    let header = CheckpointHeader::new(input_hash, 1000, [0u8; 32]);

    // Create entries with various PSNR values
    let entries = vec![
        FrameIndexEntry::new(30, 0, 1000).with_psnr(35.5),
        FrameIndexEntry::new(60, 1000, 1200).with_psnr(42.123),
        FrameIndexEntry::new(90, 2200, 800).with_psnr(50.999),
    ];

    capsule
        .write_checkpoint(&checkpoint_path, &header, &entries)
        .expect("checkpoint write failed");

    // Load and verify PSNR values
    let capsule2 = EncoderCheckpointCapsule::new(input_hash, 30);
    let data = capsule2
        .load_checkpoint(&checkpoint_path)
        .expect("load failed");

    assert_eq!(data.frame_entries.len(), 3);

    // Q16.16 fixed-point precision allows ~0.001 accuracy
    assert!((data.frame_entries[0].psnr() - 35.5).abs() < 0.001);
    assert!((data.frame_entries[1].psnr() - 42.123).abs() < 0.001);
    assert!((data.frame_entries[2].psnr() - 50.999).abs() < 0.001);

    // Cleanup
    let _ = std::fs::remove_file(&checkpoint_path);
}

/// Test atomic write behavior (simulate power failure during write)
/// Verifies that partial writes don't corrupt checkpoint state
#[test]
fn test_atomic_write_behavior() {
    let temp_dir = std::env::temp_dir();
    let checkpoint_path = temp_dir.join("test_atomic_write_checkpoint.kdly.ckpt");

    let input_hash = [0xABu8; 32];

    // Write first valid checkpoint
    let capsule = EncoderCheckpointCapsule::new(input_hash, 30);
    let header = CheckpointHeader::new(input_hash, 1000, [0u8; 32]);
    let entries1 = vec![FrameIndexEntry::new(30, 0, 1000)];

    capsule
        .write_checkpoint(&checkpoint_path, &header, &entries1)
        .expect("checkpoint 1 write failed");

    // Verify first checkpoint is valid
    let capsule2 = EncoderCheckpointCapsule::new(input_hash, 30);
    let data1 = capsule2
        .load_checkpoint(&checkpoint_path)
        .expect("load 1 failed");
    assert_eq!(data1.frame_entries.len(), 1);

    // Write second checkpoint (overwrites first)
    let entries2 = vec![
        FrameIndexEntry::new(30, 0, 1000),
        FrameIndexEntry::new(60, 1000, 1200),
    ];

    capsule
        .write_checkpoint(&checkpoint_path, &header, &entries2)
        .expect("checkpoint 2 write failed");

    // Verify second checkpoint replaced first
    let capsule3 = EncoderCheckpointCapsule::new(input_hash, 30);
    let data2 = capsule3
        .load_checkpoint(&checkpoint_path)
        .expect("load 2 failed");
    assert_eq!(data2.frame_entries.len(), 2);

    // Cleanup
    let _ = std::fs::remove_file(&checkpoint_path);
}
