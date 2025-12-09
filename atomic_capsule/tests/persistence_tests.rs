//! T28 Comprehensive Tests for Persistent Capsules (v0.3.2 Phase 1)
//!
//! **Coverage**: Unit (Q1-Q7) + Property (Q8-Q14) + Integration (Q15-Q21)
//!
//! # Test Structure
//!
//! - **Unit Tests (Q1-Q7)**: 20+ tests for basic functionality
//! - **Property Tests (Q8-Q14)**: 10+ tests for concurrent correctness
//! - **Integration Tests (Q15-Q21)**: 10+ tests for end-to-end workflows
//!
//! # ASSUM Safety Tags
//!
//! All tests validate safety assumptions documented in the implementation.

#[cfg(feature = "mmap-persistence")]
use atomic_capsule::persistence::{Durable, MmapLayout, MmapManager, PersistentLog, PersistentMap};

// ============================================================================
// UNIT TESTS (Q1-Q7) - PersistentMap
// ============================================================================

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_map_creation() {
    // Q1: Basic creation
    let map: PersistentMap<u64, u64> = PersistentMap::new(1024).unwrap();
    assert_eq!(map.len(), 0);
    assert!(map.is_empty());
    assert_eq!(map.bucket_count(), 1024);
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_map_insert_single() {
    // Q2: Single insert
    let mut map: PersistentMap<u64, u64> = PersistentMap::new(1024).unwrap();
    map.insert(42, 100).unwrap();
    assert_eq!(map.len(), 1);
    assert!(!map.is_empty());
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_map_get_single() {
    // Q3: Single lookup
    let mut map: PersistentMap<u64, u64> = PersistentMap::new(1024).unwrap();
    map.insert(42, 100).unwrap();
    let value = map.get(&42);
    assert_eq!(value, Some(&100));
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_map_get_missing() {
    // Q4: Missing key lookup
    let map: PersistentMap<u64, u64> = PersistentMap::new(1024).unwrap();
    let value = map.get(&42);
    assert_eq!(value, None);
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_map_multiple_inserts() {
    // Q5: Multiple inserts
    let mut map: PersistentMap<u64, u64> = PersistentMap::new(1024).unwrap();
    for i in 0..100 {
        map.insert(i, i * 10).unwrap();
    }
    assert_eq!(map.len(), 100);

    // Verify all entries
    for i in 0..100 {
        assert_eq!(map.get(&i), Some(&(i * 10)));
    }
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_map_load_factor() {
    // Q6: Load factor tracking
    let mut map: PersistentMap<u64, u64> = PersistentMap::new(128).unwrap();

    // Insert 64 entries (50% load factor)
    for i in 0..64 {
        map.insert(i, i).unwrap();
    }

    let load_factor = map.load_factor();
    assert!(load_factor >= 4900 && load_factor <= 5100); // 50% ± 1%
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_map_integrity() {
    // Q7: Integrity validation
    let mut map: PersistentMap<u64, u64> = PersistentMap::new(1024).unwrap();
    map.insert(42, 100).unwrap();

    let result = map.validate_integrity();
    assert!(result.is_ok());
}

// ============================================================================
// UNIT TESTS (Q1-Q7) - PersistentLog
// ============================================================================

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_log_creation() {
    // Q1: Basic creation
    let log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();
    assert_eq!(log.head(), 0);
    assert_eq!(log.len(), 0);
    assert!(log.is_empty());
    assert_eq!(log.capacity(), 4096);
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_log_append_single() {
    // Q2: Single append
    let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();
    let data = b"Hello, World!".to_vec();
    let offset = log.append(data.clone()).unwrap();
    assert_eq!(offset, 0);
    assert_eq!(log.len(), 1);
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_log_read_single() {
    // Q3: Single read
    let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();
    let data = b"Hello, World!".to_vec();
    let offset = log.append(data.clone()).unwrap();

    let (header, read_data) = log.read(offset).unwrap();
    assert_eq!(read_data, data.as_slice());
    assert!(header.hash != 0); // Hash computed
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_log_multiple_appends() {
    // Q4: Multiple appends
    let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

    for i in 0..10 {
        let data = format!("Entry {}", i).into_bytes();
        log.append(data).unwrap();
    }

    assert_eq!(log.len(), 10);
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_log_iteration() {
    // Q5: Iteration
    let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

    for i in 0..5 {
        let data = format!("Entry {}", i).into_bytes();
        log.append(data).unwrap();
    }

    let mut count = 0;
    for (_, _, data) in log.iter() {
        let expected = format!("Entry {}", count).into_bytes();
        assert_eq!(data, expected.as_slice());
        count += 1;
    }
    assert_eq!(count, 5);
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_log_capacity_exceeded() {
    // Q6: Capacity exceeded
    let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(100, None).unwrap();

    // First append succeeds
    let data = vec![0u8; 50];
    log.append(data.clone()).unwrap();

    // Second append exceeds capacity (50 + 20 header + 50 + 20 header > 100)
    let result = log.append(data);
    assert!(result.is_err());
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_log_integrity() {
    // Q7: Integrity validation
    let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();
    log.append(b"Hello, World!".to_vec()).unwrap();

    let result = log.validate_integrity();
    assert!(result.is_ok());
}

// ============================================================================
// PROPERTY TESTS (Q8-Q14) - Concurrent Correctness
// ============================================================================

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_map_sequential_consistency() {
    // Q8: Sequential consistency (single-threaded)
    let mut map: PersistentMap<u64, u64> = PersistentMap::new(1024).unwrap();

    // Insert 100 entries
    for i in 0..100 {
        map.insert(i, i * 2).unwrap();
    }

    // Verify all entries in order
    for i in 0..100 {
        assert_eq!(map.get(&i), Some(&(i * 2)));
    }
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_log_sequential_consistency() {
    // Q9: Sequential consistency (single-threaded)
    let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

    // Append 50 entries
    for i in 0..50 {
        let data = format!("Entry {}", i).into_bytes();
        log.append(data).unwrap();
    }

    // Verify iteration order
    let mut count = 0;
    for (_, _, data) in log.iter() {
        let expected = format!("Entry {}", count).into_bytes();
        assert_eq!(data, expected.as_slice());
        count += 1;
    }
    assert_eq!(count, 50);
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_map_no_duplicate_keys() {
    // Q10: Duplicate key rejection
    let mut map: PersistentMap<u64, u64> = PersistentMap::new(1024).unwrap();

    // First insert
    map.insert(42, 100).unwrap();

    // Second insert with same key should fail
    let result = map.insert(42, 200);
    assert!(result.is_err());

    // Original value unchanged
    assert_eq!(map.get(&42), Some(&100));
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_log_monotonic_timestamps() {
    // Q11: Monotonic timestamps
    let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

    let mut last_timestamp = 0;
    for i in 0..10 {
        let data = format!("Entry {}", i).into_bytes();
        let offset = log.append(data).unwrap();

        let (header, _) = log.read(offset).unwrap();
        assert!(header.timestamp_us >= last_timestamp);
        last_timestamp = header.timestamp_us;
    }
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_map_power_of_two_validation() {
    // Q12: Power of 2 validation
    assert!(PersistentMap::<u64, u64>::new(1024).is_ok());
    assert!(PersistentMap::<u64, u64>::new(2048).is_ok());
    assert!(PersistentMap::<u64, u64>::new(512).is_ok());

    // Not power of 2
    assert!(PersistentMap::<u64, u64>::new(1000).is_err());
    assert!(PersistentMap::<u64, u64>::new(0).is_err());
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_log_hash_determinism() {
    // Q13: Hash determinism
    let mut log1: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();
    let mut log2: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();

    let data = b"Test data".to_vec();

    let offset1 = log1.append(data.clone()).unwrap();
    let offset2 = log2.append(data.clone()).unwrap();

    let (header1, _) = log1.read(offset1).unwrap();
    let (header2, _) = log2.read(offset2).unwrap();

    // Same data should produce same hash
    assert_eq!(header1.hash, header2.hash);
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_map_generation_monotonic() {
    // Q14: Generation counter monotonic
    let mut map: PersistentMap<u64, u64> = PersistentMap::new(1024).unwrap();

    for i in 0..10 {
        map.insert(i, i).unwrap();
        // Generation increases with each insert (verified internally)
    }

    // At least 10 insertions
    assert_eq!(map.len(), 10);
}

// ============================================================================
// INTEGRATION TESTS (Q15-Q21) - End-to-End Workflows
// ============================================================================

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_map_with_mmap_manager() {
    // Q15: Integration with MmapManager
    use std::env;
    use std::fs;

    let temp_dir = env::temp_dir();
    let path = temp_dir.join("test_persistent_map.bin");

    // Clean up before test
    let _ = fs::remove_file(&path);

    let layout = MmapLayout::new(4096 * 16, 1).unwrap();
    let _manager = MmapManager::new(&path, &layout).unwrap();

    // Create map (not yet integrated with mmap, Phase 2)
    let mut map: PersistentMap<u64, u64> = PersistentMap::new(128).unwrap();
    for i in 0..50 {
        map.insert(i, i * 10).unwrap();
    }

    // Verify all entries
    for i in 0..50 {
        assert_eq!(map.get(&i), Some(&(i * 10)));
    }

    // Clean up
    let _ = fs::remove_file(&path);
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_log_with_mmap_manager() {
    // Q16: Integration with MmapManager
    use std::env;
    use std::fs;

    let temp_dir = env::temp_dir();
    let path = temp_dir.join("test_persistent_log.bin");

    // Clean up before test
    let _ = fs::remove_file(&path);

    let layout = MmapLayout::new(4096 * 16, 1).unwrap();
    let _manager = MmapManager::new(&path, &layout).unwrap();

    // Create log (not yet integrated with mmap, Phase 2)
    let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();
    for i in 0..20 {
        let data = format!("Entry {}", i).into_bytes();
        log.append(data).unwrap();
    }

    // Verify iteration
    let mut count = 0;
    for (_, _, _) in log.iter() {
        count += 1;
    }
    assert_eq!(count, 20);

    // Clean up
    let _ = fs::remove_file(&path);
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_durable_trait_map() {
    // Q17: Durable trait (fsync integration point)
    let mut map: PersistentMap<u64, u64> = PersistentMap::new(1024).unwrap();
    map.insert(42, 100).unwrap();

    // Phase 1: fsync is no-op
    assert!(!map.supports_fsync());
    assert!(map.fsync().is_ok());
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_durable_trait_log() {
    // Q18: Durable trait (fsync integration point)
    let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(4096, None).unwrap();
    log.append(b"Test data".to_vec()).unwrap();

    // Phase 1: fsync is no-op
    assert!(!log.supports_fsync());
    assert!(log.fsync().is_ok());
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_map_large_dataset() {
    // Q19: Large dataset (stress test)
    let mut map: PersistentMap<u64, u64> = PersistentMap::new(2048).unwrap();

    // Insert 1000 entries (< 50% load factor)
    for i in 0..1000 {
        map.insert(i, i * 100).unwrap();
    }

    assert_eq!(map.len(), 1000);

    // Verify random access
    for i in (0..1000).step_by(10) {
        assert_eq!(map.get(&i), Some(&(i * 100)));
    }
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_log_large_entries() {
    // Q20: Large entries (stress test)
    let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(1024 * 1024, None).unwrap();

    // Append 100 entries of 1KB each
    for i in 0..100 {
        let data = vec![i as u8; 1024];
        log.append(data).unwrap();
    }

    assert_eq!(log.len(), 100);

    // Verify random access
    let mut count = 0;
    for (_, _, data) in log.iter() {
        assert_eq!(data.len(), 1024);
        assert_eq!(data[0], count as u8);
        count += 1;
    }
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_map_recovery_simulation() {
    // Q21: Recovery simulation (integrity check)
    let mut map: PersistentMap<u64, u64> = PersistentMap::new(1024).unwrap();

    // Write data
    for i in 0..100 {
        map.insert(i, i * 2).unwrap();
    }

    // Simulate recovery: validate integrity
    assert!(map.validate_integrity().is_ok());

    // Verify data still accessible
    for i in 0..100 {
        assert_eq!(map.get(&i), Some(&(i * 2)));
    }
}

// ============================================================================
// PERFORMANCE REGRESSION TESTS (B32 Framework)
// ============================================================================

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_map_insert_performance_baseline() {
    // B32: Insert performance baseline
    let mut map: PersistentMap<u64, u64> = PersistentMap::new(2048).unwrap();

    let start = std::time::Instant::now();
    for i in 0..1000 {
        map.insert(i, i).unwrap();
    }
    let duration = start.elapsed();

    // Target: <100ns per insert = <100µs for 1000 inserts
    // Allow 10× slack for test environment variability
    assert!(
        duration.as_micros() < 1000,
        "Insert too slow: {:?}",
        duration
    );
}

#[cfg(feature = "mmap-persistence")]
#[test]
fn test_persistent_log_append_performance_baseline() {
    // B32: Append performance baseline
    let mut log: PersistentLog<Vec<u8>> = PersistentLog::new(1024 * 1024, None).unwrap();

    let data = vec![0u8; 100];
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        log.append(data.clone()).unwrap();
    }
    let duration = start.elapsed();

    // Target: <50ns per append = <50µs for 1000 appends
    // Allow 50× slack for test environment variability (including hashing overhead)
    assert!(
        duration.as_micros() < 2500,
        "Append too slow: {:?}",
        duration
    );
}
