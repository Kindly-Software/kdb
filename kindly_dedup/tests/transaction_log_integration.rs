//! Integration tests for TransactionLogCapsule
//!
//! Comprehensive tests covering:
//! - Unit tests: Basic functionality
//! - Property tests: Invariant validation
//! - Integration tests: Batch operations
//! - Production tests: Crash recovery simulation

#![cfg(feature = "batch-lsh")]

use kindly_dedup::lsh::{TransactionLogCapsule, LshEntry};
use std::path::PathBuf;
use std::fs;

/// Create temporary test log path
fn temp_log_path(test_name: &str) -> PathBuf {
    let temp_dir = std::env::temp_dir();
    temp_dir.join(format!("kindly_dedup_{}.log", test_name))
}

/// Clean up test log file
fn cleanup_log(path: &PathBuf) {
    let _ = fs::remove_file(path);
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[test]
fn test_lsh_entry_basic() {
    let entry = LshEntry::new(0, 0x1234567890abcdef, 42);
    assert_eq!(entry.band_idx, 0);
    assert_eq!(entry.hash, 0x1234567890abcdef);
    assert_eq!(entry.doc_id, 42);
}

#[test]
fn test_lsh_entry_serialization() {
    let entry = LshEntry::new(3, 0x9876543210fedcba, 100);
    let bytes = entry.to_bytes();
    assert_eq!(bytes.len(), 20);

    let deserialized = LshEntry::from_bytes(&bytes);
    assert_eq!(deserialized.band_idx, entry.band_idx);
    assert_eq!(deserialized.hash, entry.hash);
    assert_eq!(deserialized.doc_id, entry.doc_id);
}

#[test]
fn test_transaction_log_new() {
    let path = temp_log_path("new");
    let log = TransactionLogCapsule::new(path.to_str().unwrap()).unwrap();
    assert_eq!(log.get_generation(), 0);
    assert_eq!(log.get_checksum(), 0);
    assert_eq!(log.get_bytes_written(), 0);
    cleanup_log(&path);
}

#[test]
fn test_transaction_log_path_validation() {
    let long_path = "a".repeat(300);
    assert!(TransactionLogCapsule::new(&long_path).is_err());
}

// ============================================================================
// PROPERTY TESTS
// ============================================================================

#[test]
fn test_generation_always_increases() {
    let path = temp_log_path("generation_increase");
    let log = TransactionLogCapsule::new(path.to_str().unwrap()).unwrap();

    let gen0 = log.get_generation();
    let batch1 = vec![LshEntry::new(0, 0x111, 1)];
    let _ = log.append_batch(&batch1);
    let gen1 = log.get_generation();

    let batch2 = vec![LshEntry::new(1, 0x222, 2)];
    let _ = log.append_batch(&batch2);
    let gen2 = log.get_generation();

    assert!(gen1 > gen0);
    assert!(gen2 > gen1);
    cleanup_log(&path);
}

#[test]
fn test_batch_deterministic() {
    let path = temp_log_path("batch_deterministic");
    let log = TransactionLogCapsule::new(path.to_str().unwrap()).unwrap();

    let batch = vec![
        LshEntry::new(0, 0x111, 1),
        LshEntry::new(1, 0x222, 2),
        LshEntry::new(2, 0x333, 3),
    ];

    let gen1 = log.append_batch(&batch).unwrap();
    let gen2 = log.append_batch(&batch).unwrap();

    // Same input, different generation
    assert_ne!(gen1, gen2);

    // But both should replay successfully
    let replayed = log.replay().unwrap();
    assert_eq!(replayed.len(), 2);
    for (i, batch_data) in replayed.iter().enumerate() {
        assert_eq!(batch_data.len(), 3);
        assert_eq!(batch_data[0].doc_id, (i + 1) as u32);
    }

    cleanup_log(&path);
}

// ============================================================================
// INTEGRATION TESTS
// ============================================================================

#[test]
fn test_single_batch_append_replay() {
    let path = temp_log_path("single_batch");
    let log = TransactionLogCapsule::new(path.to_str().unwrap()).unwrap();

    let batch = vec![
        LshEntry::new(0, 0xaaaa, 10),
        LshEntry::new(1, 0xbbbb, 20),
        LshEntry::new(2, 0xcccc, 30),
    ];

    let gen = log.append_batch(&batch).unwrap();
    assert_eq!(gen, 0);

    let replayed = log.replay().unwrap();
    assert_eq!(replayed.len(), 1);
    assert_eq!(replayed[0].len(), 3);
    assert_eq!(replayed[0][0].doc_id, 10);
    assert_eq!(replayed[0][1].doc_id, 20);
    assert_eq!(replayed[0][2].doc_id, 30);

    cleanup_log(&path);
}

#[test]
fn test_multiple_batches_append_replay() {
    let path = temp_log_path("multi_batch");
    let log = TransactionLogCapsule::new(path.to_str().unwrap()).unwrap();

    let batch1 = vec![LshEntry::new(0, 0x1111, 100)];
    let batch2 = vec![
        LshEntry::new(1, 0x2222, 200),
        LshEntry::new(2, 0x3333, 300),
    ];
    let batch3 = vec![LshEntry::new(3, 0x4444, 400)];

    let gen1 = log.append_batch(&batch1).unwrap();
    let gen2 = log.append_batch(&batch2).unwrap();
    let gen3 = log.append_batch(&batch3).unwrap();

    assert_eq!(gen1, 0);
    assert_eq!(gen2, 1);
    assert_eq!(gen3, 2);

    let replayed = log.replay().unwrap();
    assert_eq!(replayed.len(), 3);
    assert_eq!(replayed[0][0].doc_id, 100);
    assert_eq!(replayed[1][0].doc_id, 200);
    assert_eq!(replayed[1][1].doc_id, 300);
    assert_eq!(replayed[2][0].doc_id, 400);

    cleanup_log(&path);
}

#[test]
fn test_truncate_clears_log() {
    let path = temp_log_path("truncate");
    let log = TransactionLogCapsule::new(path.to_str().unwrap()).unwrap();

    let batch = vec![LshEntry::new(0, 0xffff, 999)];
    let _ = log.append_batch(&batch).unwrap();

    // Verify batch was written
    let replayed_before = log.replay().unwrap();
    assert_eq!(replayed_before.len(), 1);

    // Truncate
    let _ = log.truncate().unwrap();

    // Verify log is cleared
    assert_eq!(log.get_generation(), 0);
    let replayed_after = log.replay().unwrap();
    assert_eq!(replayed_after.len(), 0);

    cleanup_log(&path);
}

#[test]
fn test_verify_checksum_valid_log() {
    let path = temp_log_path("checksum_valid");
    let log = TransactionLogCapsule::new(path.to_str().unwrap()).unwrap();

    let batch = vec![LshEntry::new(0, 0x1234, 42)];
    let _ = log.append_batch(&batch).unwrap();

    let valid = log.verify_checksum().unwrap();
    assert!(valid);

    cleanup_log(&path);
}

// ============================================================================
// PRODUCTION TESTS (Crash Recovery Simulation)
// ============================================================================

#[test]
fn test_crash_recovery_basic() {
    let path = temp_log_path("crash_recovery_basic");

    // Phase 1: Write batches
    {
        let log = TransactionLogCapsule::new(path.to_str().unwrap()).unwrap();

        let batch1 = vec![LshEntry::new(0, 0x1111, 1)];
        let batch2 = vec![LshEntry::new(1, 0x2222, 2)];
        let batch3 = vec![LshEntry::new(2, 0x3333, 3)];

        let _ = log.append_batch(&batch1).unwrap();
        let _ = log.append_batch(&batch2).unwrap();
        let _ = log.append_batch(&batch3).unwrap();
    } // Scope end simulates "crash" (log dropped, file remains)

    // Phase 2: Recovery (reopen log)
    {
        let recovered_log = TransactionLogCapsule::new(path.to_str().unwrap()).unwrap();
        let replayed = recovered_log.replay().unwrap();

        // All batches should be recoverable
        assert_eq!(replayed.len(), 3);
        assert_eq!(replayed[0][0].doc_id, 1);
        assert_eq!(replayed[1][0].doc_id, 2);
        assert_eq!(replayed[2][0].doc_id, 3);
    }

    cleanup_log(&path);
}

#[test]
fn test_large_batch_write() {
    let path = temp_log_path("large_batch");
    let log = TransactionLogCapsule::new(path.to_str().unwrap()).unwrap();

    // Create a large batch (10,000 entries)
    let mut large_batch = Vec::new();
    for i in 0..10_000 {
        large_batch.push(LshEntry::new(
            (i % 5) as u32,
            (i as u64).wrapping_mul(0x0123456789abcdef),
            i as u32,
        ));
    }

    let gen = log.append_batch(&large_batch).unwrap();
    assert_eq!(gen, 0);

    let replayed = log.replay().unwrap();
    assert_eq!(replayed.len(), 1);
    assert_eq!(replayed[0].len(), 10_000);

    // Verify first and last entries
    assert_eq!(replayed[0][0].doc_id, 0);
    assert_eq!(replayed[0][9_999].doc_id, 9_999);

    cleanup_log(&path);
}

#[test]
fn test_sequential_batches_ordering() {
    let path = temp_log_path("ordering");
    let log = TransactionLogCapsule::new(path.to_str().unwrap()).unwrap();

    // Write 100 small batches
    for i in 0..100 {
        let batch = vec![LshEntry::new(
            (i % 5) as u32,
            (i as u64).wrapping_mul(0xdeadbeef),
            i as u32,
        )];
        let gen = log.append_batch(&batch).unwrap();
        assert_eq!(gen, i as u64);
    }

    // Verify all batches replay in order
    let replayed = log.replay().unwrap();
    assert_eq!(replayed.len(), 100);

    for (idx, batch) in replayed.iter().enumerate() {
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].doc_id, idx as u32);
    }

    cleanup_log(&path);
}

#[test]
fn test_empty_log_replay() {
    let path = temp_log_path("empty_log");
    let log = TransactionLogCapsule::new(path.to_str().unwrap()).unwrap();

    // Replay without writing anything
    let replayed = log.replay().unwrap();
    assert_eq!(replayed.len(), 0);

    cleanup_log(&path);
}

#[test]
fn test_recovery_preserves_data() {
    let path = temp_log_path("recovery_preservation");

    // Write data
    {
        let log = TransactionLogCapsule::new(path.to_str().unwrap()).unwrap();
        for i in 0..50 {
            let batch = vec![
                LshEntry::new(0, (i as u64) * 0x100, i as u32),
                LshEntry::new(1, (i as u64) * 0x200, (i + 50) as u32),
            ];
            let _ = log.append_batch(&batch).unwrap();
        }
    }

    // Recover and verify
    {
        let recovered_log = TransactionLogCapsule::new(path.to_str().unwrap()).unwrap();
        let replayed = recovered_log.replay().unwrap();

        assert_eq!(replayed.len(), 50);
        for (idx, batch) in replayed.iter().enumerate() {
            assert_eq!(batch.len(), 2);
            assert_eq!(batch[0].doc_id, idx as u32);
            assert_eq!(batch[1].doc_id, (idx + 50) as u32);
        }
    }

    cleanup_log(&path);
}
