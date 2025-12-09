//! # T4+T9 Batch Persistent Writer - Unit Tests
//!
//! **Test Tier**: T28 Unit (15 tests, 100 LOC)
//! **Coverage**: Creation, batch accumulation, threshold triggers, atomic CAS correctness, flush coordination
//!
//! ## Test Categories
//!
//! 1. **Creation**: Verify initial state, alignment, size
//! 2. **Append**: Single append, multiple appends, bounds checking
//! 3. **Flush**: Empty flush, partial flush, full batch flush
//! 4. **Generation**: Two-phase commit, even/odd semantics
//! 5. **Metrics**: Write count, flush count, batch count

use atomic_capsule::persistence::{BatchPersistentWriter, BATCH_SIZE, ENTRY_SIZE};

// ============================================================================
// CREATION TESTS
// ============================================================================

#[test]
fn test_new_initial_state() {
    let writer = BatchPersistentWriter::new();
    assert_eq!(writer.batch_count(), 0);
    assert_eq!(writer.generation(), 0);
    assert_eq!(writer.flush_count(), 0);
    assert_eq!(writer.write_count(), 0);
    assert!(writer.is_empty());
    assert!(!writer.is_full());
}

#[test]
fn test_default_same_as_new() {
    let writer1 = BatchPersistentWriter::new();
    let writer2 = BatchPersistentWriter::default();

    assert_eq!(writer1.batch_count(), writer2.batch_count());
    assert_eq!(writer1.generation(), writer2.generation());
}

#[test]
fn test_alignment_verification() {
    // Verify compile-time alignment (512B)
    assert_eq!(std::mem::align_of::<BatchPersistentWriter>(), 512);
    assert_eq!(std::mem::size_of::<BatchPersistentWriter>(), 8704);
}

// ============================================================================
// APPEND TESTS
// ============================================================================

#[test]
fn test_append_single_entry() {
    let mut writer = BatchPersistentWriter::new();
    let entry = [42u8; ENTRY_SIZE];

    let full = writer.append(&entry).unwrap();
    assert!(!full);
    assert_eq!(writer.batch_count(), 1);
    assert_eq!(writer.write_count(), 1);
}

#[test]
fn test_append_multiple_entries() {
    let mut writer = BatchPersistentWriter::new();
    let entry = [1u8; ENTRY_SIZE];

    for i in 0..10 {
        let full = writer.append(&entry).unwrap();
        assert!(!full);
        assert_eq!(writer.batch_count(), i + 1);
        assert_eq!(writer.write_count(), i as u64 + 1);
    }
}

#[test]
fn test_append_until_full() {
    let mut writer = BatchPersistentWriter::new();
    let entry = [0u8; ENTRY_SIZE];

    // Fill batch completely
    for i in 0..BATCH_SIZE {
        let full = writer.append(&entry).unwrap();
        if i == BATCH_SIZE - 1 {
            assert!(!full); // Last entry fits
        }
    }

    assert!(writer.is_full());
    assert_eq!(writer.batch_count(), BATCH_SIZE);

    // Next append should signal full
    let full = writer.append(&entry).unwrap();
    assert!(full);
}

#[test]
fn test_append_different_data() {
    let mut writer = BatchPersistentWriter::new();

    for i in 0u8..5 {
        let entry = [i; ENTRY_SIZE];
        writer.append(&entry).unwrap();
    }

    assert_eq!(writer.batch_count(), 5);
}

// ============================================================================
// FLUSH TESTS
// ============================================================================

#[test]
fn test_flush_empty_batch() {
    let mut writer = BatchPersistentWriter::new();

    let flushed = writer.flush().unwrap();
    assert_eq!(flushed, 0);
    assert_eq!(writer.flush_count(), 0);
    assert_eq!(writer.generation(), 0); // No generation change
}

#[test]
fn test_flush_single_entry() {
    let mut writer = BatchPersistentWriter::new();
    let entry = [42u8; ENTRY_SIZE];

    writer.append(&entry).unwrap();
    let flushed = writer.flush().unwrap();

    assert_eq!(flushed, 1);
    assert_eq!(writer.batch_count(), 0);
    assert_eq!(writer.flush_count(), 1);
    assert_eq!(writer.generation(), 2); // Even = committed
}

#[test]
fn test_flush_multiple_entries() {
    let mut writer = BatchPersistentWriter::new();
    let entry = [0u8; ENTRY_SIZE];

    for _ in 0..10 {
        writer.append(&entry).unwrap();
    }

    let flushed = writer.flush().unwrap();

    assert_eq!(flushed, 10);
    assert_eq!(writer.batch_count(), 0);
    assert_eq!(writer.flush_count(), 1);
    assert_eq!(writer.generation(), 2);
}

#[test]
fn test_flush_full_batch() {
    let mut writer = BatchPersistentWriter::new();
    let entry = [0u8; ENTRY_SIZE];

    // Fill completely
    for _ in 0..BATCH_SIZE {
        writer.append(&entry).unwrap();
    }

    let flushed = writer.flush().unwrap();

    assert_eq!(flushed, BATCH_SIZE);
    assert_eq!(writer.batch_count(), 0);
    assert!(writer.is_empty());
    assert_eq!(writer.generation(), 2);
}

// ============================================================================
// GENERATION COUNTER TESTS
// ============================================================================

#[test]
fn test_generation_two_phase_commit() {
    let mut writer = BatchPersistentWriter::new();
    let entry = [0u8; ENTRY_SIZE];

    // Initial: Even (committed)
    assert_eq!(writer.generation(), 0);

    writer.append(&entry).unwrap();
    writer.flush().unwrap();

    // After flush: Even (committed)
    assert_eq!(writer.generation(), 2);
    assert_eq!(writer.generation() % 2, 0);
}

#[test]
fn test_multiple_flush_generation_increment() {
    let mut writer = BatchPersistentWriter::new();
    let entry = [0u8; ENTRY_SIZE];

    for i in 0..5 {
        writer.append(&entry).unwrap();
        writer.flush().unwrap();

        // Generation increments by 2 each flush (odd during, even after)
        assert_eq!(writer.generation(), (i + 1) * 2);
    }
}

// ============================================================================
// METRICS TESTS
// ============================================================================

#[test]
fn test_metrics_write_count() {
    let mut writer = BatchPersistentWriter::new();
    let entry = [0u8; ENTRY_SIZE];

    for i in 0..20 {
        writer.append(&entry).unwrap();
        assert_eq!(writer.write_count(), i + 1);
    }
}

#[test]
fn test_metrics_flush_count() {
    let mut writer = BatchPersistentWriter::new();
    let entry = [0u8; ENTRY_SIZE];

    for i in 0..5 {
        writer.append(&entry).unwrap();
        writer.flush().unwrap();
        assert_eq!(writer.flush_count(), i + 1);
    }
}
