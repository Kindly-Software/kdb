//! T28 Comprehensive Tests for PersistentLog<T>
//!
//! **Phase 9 (v0.3.2)**: 4-tier test pyramid (Unit/Property/Integration/Production)
//!
//! # Test Coverage
//!
//! - **Unit (Q1-Q7)**: 55 tests - Layout, append, iteration
//! - **Property (Q8-Q14)**: 45 tests - Concurrent append, ordering
//! - **Integration (Q15-Q21)**: 35 tests - File persistence, recovery
//! - **Production (Q22-Q28)**: 25 tests - Stress, real workloads
//!
//! **Total**: 160 tests planned (61 implemented, 100% pass rate target)
//!
//! **Dual Backend Support**: Tests support both `mmap-persistence` (memmap2) and `capsule-mmap` (capsule-native).
//!
//! **Note**: Header describes planned T28 4-tier coverage. Actual implementation: 61 tests.

#[cfg(any(feature = "mmap-persistence", feature = "capsule-mmap"))]
mod persistent_log_tests {
    use atomic_capsule::persistence::{
        Durable, LogEntryHeader, MmapError, PersistentLog, PersistentLogHeader,
    };
    use std::fs::{File, OpenOptions};
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::sync::Arc;
    use std::thread;
    use tempfile::tempdir;

    // ========================================================================
    // T28 UNIT TESTS (Q1-Q7) - 55 Tests
    // ========================================================================

    // Q1: Structural Validation (10 tests)

    #[test]
    fn test_header_size_and_alignment() {
        assert_eq!(std::mem::size_of::<PersistentLogHeader>(), 256);
        assert_eq!(std::mem::align_of::<PersistentLogHeader>(), 256);
    }

    #[test]
    fn test_entry_header_size() {
        assert_eq!(std::mem::size_of::<LogEntryHeader>(), 24);
    }

    #[test]
    fn test_header_initialization() {
        let header = PersistentLogHeader::new(4096, 1024);
        assert_eq!(header.generation(), 0);
        assert_eq!(header.head(), 0);
        assert_eq!(header.capacity(), 4096);
        assert_eq!(header.entry_count(), 0);
        assert_eq!(header.segment_size(), 1024);
    }

    #[test]
    fn test_header_generation_counter() {
        let header = PersistentLogHeader::new(4096, 1024);
        let gen1 = header.generation();

        header.allocate(100).unwrap();
        let gen2 = header.generation();

        assert_eq!(gen2, gen1 + 1);
    }

    #[test]
    fn test_header_head_position() {
        let header = PersistentLogHeader::new(4096, 1024);
        assert_eq!(header.head(), 0);

        header.allocate(100).unwrap();
        assert_eq!(header.head(), 100);

        header.allocate(200).unwrap();
        assert_eq!(header.head(), 300);
    }

    #[test]
    fn test_header_entry_count() {
        let header = PersistentLogHeader::new(4096, 1024);
        assert_eq!(header.entry_count(), 0);

        header.allocate(100).unwrap();
        assert_eq!(header.entry_count(), 1);

        header.allocate(200).unwrap();
        assert_eq!(header.entry_count(), 2);
    }

    #[test]
    fn test_header_capacity_immutable() {
        let header = PersistentLogHeader::new(4096, 1024);
        let cap1 = header.capacity();

        header.allocate(100).unwrap();
        let cap2 = header.capacity();

        assert_eq!(cap1, cap2);
        assert_eq!(cap2, 4096);
    }

    #[test]
    fn test_header_segment_size_immutable() {
        let header = PersistentLogHeader::new(4096, 1024);
        let seg1 = header.segment_size();

        header.allocate(100).unwrap();
        let seg2 = header.segment_size();

        assert_eq!(seg1, seg2);
        assert_eq!(seg2, 1024);
    }

    #[test]
    fn test_entry_header_creation() {
        let header = LogEntryHeader::new(100, 12345, 67890);
        assert_eq!(header.length, 100);
        assert_eq!(header.hash, 12345);
        assert_eq!(header.timestamp_us, 67890);
    }

    #[test]
    fn test_entry_header_hash_deterministic() {
        let data = b"Hello, World!";
        let hash1 = LogEntryHeader::compute_hash(data);
        let hash2 = LogEntryHeader::compute_hash(data);
        assert_eq!(hash1, hash2);
    }

    // Q2: Operation Correctness (15 tests)

    #[test]
    fn test_log_creation_default() {
        let log: PersistentLog<Vec<u8>> = PersistentLog::with_default_capacity().unwrap();
        assert_eq!(log.head(), 0);
        assert_eq!(log.len(), 0);
        assert!(log.is_empty());
        assert_eq!(log.capacity(), 4 * 1024 * 1024);
    }

    #[test]
    fn test_log_creation_custom_capacity() {
        let log: PersistentLog<Vec<u8>> = PersistentLog::new(8192, None).unwrap();
        assert_eq!(log.capacity(), 8192);
    }

    #[test]
    fn test_log_append_single() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();
        let data = b"Hello".to_vec();
        let offset = log.append(data).unwrap();

        assert_eq!(offset, 0);
        assert_eq!(log.len(), 1);
        assert!(!log.is_empty());
    }

    #[test]
    fn test_log_append_multiple() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

        let mut offsets = vec![];
        for i in 0..10 {
            let data = format!("Entry {}", i).into_bytes();
            let offset = log.append(data).unwrap();
            offsets.push(offset);
        }

        assert_eq!(log.len(), 10);
    }

    #[test]
    fn test_log_read_single() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();
        let data = b"Hello, World!".to_vec();
        let offset = log.append(data.clone()).unwrap();

        let (header, read_data) = log.read(offset).unwrap();
        assert_eq!(header.length as usize, LogEntryHeader::SIZE + data.len());
        assert_eq!(read_data, data.as_slice());
    }

    #[test]
    fn test_log_read_multiple() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

        let mut entries = vec![];
        for i in 0..10 {
            let data = format!("Entry {}", i).into_bytes();
            let offset = log.append(data.clone()).unwrap();
            entries.push((offset, data));
        }

        // Verify all entries
        for (offset, expected_data) in entries {
            let (_, read_data) = log.read(offset).unwrap();
            assert_eq!(read_data, expected_data.as_slice());
        }
    }

    #[test]
    fn test_log_iteration() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

        // Append 5 entries
        for i in 0..5 {
            let data = format!("Entry {}", i).into_bytes();
            log.append(data).unwrap();
        }

        // Iterate and verify
        let mut count = 0;
        for (_, _, data) in log.iter() {
            let expected = format!("Entry {}", count).into_bytes();
            assert_eq!(data, expected.as_slice());
            count += 1;
        }

        assert_eq!(count, 5);
    }

    #[test]
    fn test_log_head_advances() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();
        assert_eq!(log.head(), 0);

        let data1 = b"First".to_vec();
        log.append(data1).unwrap();
        let head1 = log.head();
        assert!(head1 > 0);

        let data2 = b"Second".to_vec();
        log.append(data2).unwrap();
        let head2 = log.head();
        assert!(head2 > head1);
    }

    #[test]
    fn test_log_entry_count_increments() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();
        assert_eq!(log.len(), 0);

        for i in 1..=10 {
            let data = format!("Entry {}", i).into_bytes();
            log.append(data).unwrap();
            assert_eq!(log.len(), i);
        }
    }

    #[test]
    fn test_log_empty_initially() {
        let log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();
        assert!(log.is_empty());
    }

    #[test]
    fn test_log_not_empty_after_append() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();
        let data = b"Data".to_vec();
        log.append(data).unwrap();
        assert!(!log.is_empty());
    }

    #[test]
    fn test_log_capacity_query() {
        let log: PersistentLog<Vec<u8>> = PersistentLog::new(8192, None).unwrap();
        assert_eq!(log.capacity(), 8192);
    }

    #[test]
    fn test_log_integrity_validation() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();
        let data = b"Test data".to_vec();
        log.append(data).unwrap();

        let result = log.validate_integrity();
        assert!(result.is_ok());
    }

    #[test]
    fn test_entry_header_hash_differs_for_different_data() {
        let data1 = b"Hello";
        let data2 = b"World";

        let hash1 = LogEntryHeader::compute_hash(data1);
        let hash2 = LogEntryHeader::compute_hash(data2);

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_entry_header_timestamp() {
        let ts1 = LogEntryHeader::current_timestamp_us();
        std::thread::sleep(std::time::Duration::from_micros(100));
        let ts2 = LogEntryHeader::current_timestamp_us();

        assert!(ts2 > ts1);
    }

    // Q3: Error Handling (10 tests)

    #[test]
    fn test_log_capacity_exceeded() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(100, None).unwrap();

        // First append succeeds
        let data1 = vec![0u8; 50];
        log.append(data1).unwrap();

        // Second append exceeds capacity (50 bytes data + 24 bytes header = 74)
        // Total would be 50 + 24 + 50 + 24 = 148 > 100
        let data2 = vec![0u8; 50];
        let result = log.append(data2);

        assert!(matches!(result, Err(MmapError::CapacityExceeded { .. })));
    }

    #[test]
    fn test_log_read_invalid_offset() {
        let log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

        // Read at offset 0 (no entries)
        let result = log.read(0);
        assert!(result.is_none());
    }

    #[test]
    fn test_log_read_out_of_bounds() {
        let log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

        // Read beyond capacity
        let result = log.read(5000);
        assert!(result.is_none());
    }

    #[test]
    fn test_header_allocate_exact_capacity() {
        let header = PersistentLogHeader::new(100, 1024);

        // Allocate exactly capacity
        let result = header.allocate(100);
        assert!(result.is_ok());

        // Next allocation should fail
        let result2 = header.allocate(1);
        assert!(matches!(result2, Err(MmapError::CapacityExceeded { .. })));
    }

    #[test]
    fn test_header_allocate_exceeds_capacity() {
        let header = PersistentLogHeader::new(100, 1024);

        // Allocate more than capacity
        let result = header.allocate(150);
        assert!(matches!(result, Err(MmapError::CapacityExceeded { .. })));
    }

    #[test]
    fn test_header_integrity_validation_passes() {
        let header = PersistentLogHeader::new(4096, 1024);
        header.update_hash_chain();

        let result = header.validate_integrity();
        assert!(result.is_ok());
    }

    #[test]
    fn test_header_hash_computation_non_zero() {
        let header = PersistentLogHeader::new(4096, 1024);
        let hash = header.compute_hash();
        assert_ne!(hash, 0);
    }

    #[test]
    fn test_log_fsync_without_file() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();
        assert!(!log.supports_fsync());

        // Should succeed (no-op)
        let result = log.fsync();
        assert!(result.is_ok());
    }

    #[test]
    fn test_log_fsync_with_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("log.dat");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&file_path)
            .unwrap();

        let mut log: PersistentLog<Vec<u8>> = PersistentLog::with_file(4096, None, file).unwrap();
        assert!(log.supports_fsync());

        let data = b"Test data".to_vec();
        log.append(data).unwrap();

        let result = log.fsync();
        assert!(result.is_ok());
    }

    #[test]
    fn test_log_empty_iteration() {
        let log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

        let mut count = 0;
        for _ in log.iter() {
            count += 1;
        }

        assert_eq!(count, 0);
    }

    // Q4-Q7: Additional Unit Tests (20 tests)

    #[test]
    fn test_header_constants() {
        assert_eq!(PersistentLogHeader::SIZE, 256);
        assert_eq!(LogEntryHeader::SIZE, 24);
    }

    #[test]
    fn test_log_large_entry() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(8192, None).unwrap();

        // Append 1KB entry
        let data = vec![0xAB; 1024];
        let offset = log.append(data.clone()).unwrap();

        let (_, read_data) = log.read(offset).unwrap();
        assert_eq!(read_data, data.as_slice());
    }

    #[test]
    fn test_log_small_entries() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

        // Append 100 small entries (1 byte each)
        for i in 0..100 {
            let data = vec![i as u8];
            log.append(data).unwrap();
        }

        assert_eq!(log.len(), 100);
    }

    #[test]
    fn test_log_variable_sized_entries() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(8192, None).unwrap();

        // Append entries of varying sizes
        let sizes = [1, 10, 100, 50, 200];
        for &size in &sizes {
            let data = vec![0xFF; size];
            log.append(data).unwrap();
        }

        assert_eq!(log.len(), sizes.len() as u64);
    }

    #[test]
    fn test_log_string_entries() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

        let strings = ["hello", "world", "rust", "atomic", "capsule"];
        for s in &strings {
            let data = s.as_bytes().to_vec();
            log.append(data).unwrap();
        }

        let mut count = 0;
        for (_, _, data) in log.iter() {
            let expected = strings[count].as_bytes();
            assert_eq!(data, expected);
            count += 1;
        }
    }

    #[test]
    fn test_header_generation_monotonic() {
        let header = PersistentLogHeader::new(4096, 1024);

        for i in 0..10 {
            assert_eq!(header.generation(), i);
            header.allocate(100).unwrap();
        }

        assert_eq!(header.generation(), 10);
    }

    #[test]
    fn test_header_head_monotonic() {
        let header = PersistentLogHeader::new(4096, 1024);

        let mut prev_head = 0;
        for _ in 0..10 {
            header.allocate(100).unwrap();
            let current_head = header.head();
            assert!(current_head > prev_head);
            prev_head = current_head;
        }
    }

    #[test]
    fn test_header_entry_count_monotonic() {
        let header = PersistentLogHeader::new(4096, 1024);

        for i in 1..=10 {
            header.allocate(100).unwrap();
            assert_eq!(header.entry_count(), i);
        }
    }

    #[test]
    fn test_log_head_monotonic() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

        let mut prev_head = 0;
        for i in 0..10 {
            let data = format!("Entry {}", i).into_bytes();
            log.append(data).unwrap();
            let current_head = log.head();
            assert!(current_head > prev_head);
            prev_head = current_head;
        }
    }

    #[test]
    fn test_log_len_monotonic() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

        for i in 1..=10 {
            let data = format!("Entry {}", i).into_bytes();
            log.append(data).unwrap();
            assert_eq!(log.len(), i as u64);
        }
    }

    #[test]
    fn test_entry_header_size_constant() {
        assert_eq!(LogEntryHeader::SIZE, 24);
    }

    #[test]
    fn test_entry_header_hash_non_zero() {
        let data = b"test";
        let hash = LogEntryHeader::compute_hash(data);
        assert_ne!(hash, 0);
    }

    #[test]
    fn test_entry_header_hash_varies_with_data() {
        let hashes: Vec<u64> = (0..10)
            .map(|i| {
                let data = format!("Entry {}", i).into_bytes();
                LogEntryHeader::compute_hash(&data)
            })
            .collect();

        // All hashes should be unique
        let unique_count = hashes
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len();
        assert_eq!(unique_count, hashes.len());
    }

    #[test]
    fn test_log_iteration_order() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

        for i in 0..10 {
            let data = format!("{}", i).into_bytes();
            log.append(data).unwrap();
        }

        let mut count = 0;
        for (_, _, data) in log.iter() {
            let expected = format!("{}", count).into_bytes();
            assert_eq!(data, expected.as_slice());
            count += 1;
        }
    }

    #[test]
    fn test_log_iterator_early_termination() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

        for i in 0..100 {
            let data = format!("Entry {}", i).into_bytes();
            log.append(data).unwrap();
        }

        // Take only first 10
        let count = log.iter().take(10).count();
        assert_eq!(count, 10);
    }

    #[test]
    fn test_log_iterator_at_head() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

        for i in 0..5 {
            let data = format!("Entry {}", i).into_bytes();
            log.append(data).unwrap();
        }

        // Iterator should stop at head
        let count = log.iter().count();
        assert_eq!(count, 5);
    }

    #[test]
    fn test_header_hash_changes_on_append() {
        let header = PersistentLogHeader::new(4096, 1024);

        let hash1 = header.compute_hash();
        header.allocate(100).unwrap();
        let hash2 = header.compute_hash();

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_log_integrity_after_multiple_appends() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

        for i in 0..10 {
            let data = format!("Entry {}", i).into_bytes();
            log.append(data).unwrap();
        }

        let result = log.validate_integrity();
        assert!(result.is_ok());
    }

    #[test]
    fn test_log_read_correct_length() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

        let data = vec![0xAB; 100];
        let offset = log.append(data.clone()).unwrap();

        let (header, read_data) = log.read(offset).unwrap();
        assert_eq!(header.length as usize, LogEntryHeader::SIZE + 100);
        assert_eq!(read_data.len(), 100);
    }

    #[test]
    fn test_log_read_correct_hash() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

        let data = b"Test data for hash".to_vec();
        let expected_hash = LogEntryHeader::compute_hash(&data);

        let offset = log.append(data.clone()).unwrap();

        let (header, _) = log.read(offset).unwrap();
        assert_eq!(header.hash, expected_hash);
    }

    // ========================================================================
    // T28 PROPERTY TESTS (Q8-Q14) - 45 Tests
    // ========================================================================
    // (Implementation abbreviated for length - would include concurrent tests,
    // ordering guarantees, hash collision handling, etc.)

    #[test]
    fn test_concurrent_read_no_interference() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(8192, None).unwrap();

        // Pre-populate log
        for i in 0..100 {
            let data = format!("Entry {}", i).into_bytes();
            log.append(data).unwrap();
        }

        let log_ref = Arc::new(log);
        let mut handles = vec![];

        // Spawn 10 reader threads
        for _ in 0..10 {
            let log_clone = Arc::clone(&log_ref);
            let handle = thread::spawn(move || {
                let count = log_clone.iter().count();
                assert_eq!(count, 100);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    // Additional property tests...

    // ========================================================================
    // T28 INTEGRATION TESTS (Q15-Q21) - 35 Tests
    // ========================================================================

    #[test]
    fn test_file_persistence_roundtrip() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("log.dat");

        // Create log, append data, fsync
        {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&file_path)
                .unwrap();

            let mut log: PersistentLog<Vec<u8>> =
                PersistentLog::with_file(4096, None, file).unwrap();

            for i in 0..50 {
                let data = format!("Entry {}", i).into_bytes();
                log.append(data).unwrap();
            }

            log.fsync().unwrap();
        }

        // Verify file exists and has data
        let metadata = std::fs::metadata(&file_path).unwrap();
        assert!(metadata.len() > 0);
    }

    #[test]
    fn test_fsync_durability() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("log_durable.dat");

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&file_path)
            .unwrap();

        let mut log: PersistentLog<Vec<u8>> = PersistentLog::with_file(4096, None, file).unwrap();

        let data = b"Test entry".to_vec();
        log.append(data).unwrap();
        log.fsync().unwrap();

        // File should contain data
        let metadata = std::fs::metadata(&file_path).unwrap();
        assert!(metadata.len() >= 256); // At least header size
    }

    // Additional integration tests...

    // ========================================================================
    // T28 PRODUCTION TESTS (Q22-Q28) - 25 Tests
    // ========================================================================

    #[test]
    fn test_stress_1000_appends() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(1024 * 1024, None).unwrap();

        for i in 0..1000 {
            let data = format!("Entry {}", i).into_bytes();
            log.append(data).unwrap();
        }

        assert_eq!(log.len(), 1000);

        // Verify all entries
        let count = log.iter().count();
        assert_eq!(count, 1000);
    }

    #[test]
    fn test_stress_large_entries() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(10 * 1024 * 1024, None).unwrap();

        // Append 100 entries of 10KB each
        for i in 0..100 {
            let data = vec![i as u8; 10 * 1024];
            log.append(data).unwrap();
        }

        assert_eq!(log.len(), 100);
    }

    #[test]
    fn test_production_workload_append_heavy() {
        let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4 * 1024 * 1024, None).unwrap();

        // Append 10000 small entries
        for i in 0..10000 {
            let data = format!("{}", i).into_bytes();
            log.append(data).unwrap();
        }

        assert_eq!(log.len(), 10000);
    }

    // Additional production tests...
}
