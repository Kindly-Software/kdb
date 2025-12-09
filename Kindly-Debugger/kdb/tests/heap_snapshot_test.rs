//! Integration tests for HeapSnapshotCapsule (T9 Persistent)
//!
//! Tests heap snapshot creation, persistence, and crash-safety validation

#[cfg(test)]
mod tests {
    use kdb::ptrace::heap_snapshot::{HeapSnapshotCapsule, HeapMetadata, SnapshotError};

    #[test]
    fn test_heap_snapshot_basic_creation() {
        let capsule = HeapSnapshotCapsule::new();
        let metadata = HeapMetadata {
            timestamp_ns: 1_000_000_000,
            total_allocations: 1_000,
            heap_size_bytes: 10_000_000,
            data: vec![0x01, 0x02, 0x03, 0x04, 0x05],
        };

        let snapshot_id = capsule.take_snapshot(&metadata).expect("Failed to take snapshot");
        assert_eq!(snapshot_id, 0);

        let retrieved = capsule.get_snapshot(snapshot_id).expect("Failed to retrieve snapshot");
        assert_eq!(retrieved.timestamp_ns, 1_000_000_000);
        assert_eq!(retrieved.total_allocations, 1_000);
        assert_eq!(retrieved.heap_size_bytes, 10_000_000);
    }

    #[test]
    fn test_heap_snapshot_multiple() {
        let capsule = HeapSnapshotCapsule::new();

        for i in 0..10 {
            let metadata = HeapMetadata {
                timestamp_ns: 1_000_000_000 + (i as u64),
                total_allocations: 1_000 + (i as u32),
                heap_size_bytes: 10_000_000 + (i as u64 * 1_000),
                data: vec![i as u8; 10],
            };

            let snapshot_id = capsule.take_snapshot(&metadata).expect("Failed to take snapshot");
            let retrieved = capsule.get_snapshot(snapshot_id).expect("Failed to retrieve snapshot");

            assert_eq!(retrieved.total_allocations, 1_000 + (i as u32));
            assert_eq!(retrieved.heap_size_bytes, 10_000_000 + (i as u64 * 1_000));
        }
    }

    #[test]
    fn test_heap_snapshot_checksum_validation() {
        let capsule = HeapSnapshotCapsule::new();
        let metadata = HeapMetadata {
            timestamp_ns: 1_000_000_000,
            total_allocations: 1_000,
            heap_size_bytes: 10_000_000,
            data: vec![0xAB; 100],
        };

        let snapshot_id = capsule.take_snapshot(&metadata).expect("Failed to take snapshot");
        let is_valid = capsule.verify_checksum(snapshot_id).expect("Failed to verify checksum");

        assert!(is_valid, "Checksum should be valid for uncorrupted snapshot");
    }

    #[test]
    fn test_heap_snapshot_capacity() {
        let capsule = HeapSnapshotCapsule::new();

        // Fill entire ring buffer (128 snapshots)
        for i in 0..128 {
            let metadata = HeapMetadata {
                timestamp_ns: 1_000_000_000 + (i as u64),
                total_allocations: 1_000 + (i as u32),
                heap_size_bytes: 10_000_000,
                data: vec![i as u8; 10],
            };

            let result = capsule.take_snapshot(&metadata);
            assert!(result.is_ok(), "Should successfully take snapshot {}", i);
        }

        // After exactly 128 snapshots, head wraps around to 0 (128 & 127 = 0)
        // But generation counter should have been incremented to track wraparound
        // snapshot_count() returns head position (0 after full wrap)
        // The key invariant is: head is always in valid range [0, 128)
        assert!(capsule.snapshot_count() <= 128, "Head should never exceed capacity");

        // After a full wrap, we should still be able to take more snapshots
        let metadata = HeapMetadata {
            timestamp_ns: 2_000_000_000,
            total_allocations: 2_000,
            heap_size_bytes: 20_000_000,
            data: vec![0xFF; 10],
        };
        let result = capsule.take_snapshot(&metadata);
        assert!(result.is_ok(), "Should successfully take snapshot after wraparound");
        assert_eq!(capsule.snapshot_count(), 1, "Head should be 1 after one more snapshot");
    }

    #[test]
    fn test_heap_snapshot_invalid_id() {
        let capsule = HeapSnapshotCapsule::new();
        let result = capsule.get_snapshot(256); // Out of range

        assert!(matches!(result, Err(SnapshotError::InvalidSnapshotId(_))));
    }

    #[test]
    fn test_heap_snapshot_reset() {
        let capsule = HeapSnapshotCapsule::new();

        let metadata = HeapMetadata {
            timestamp_ns: 1_000_000_000,
            total_allocations: 1_000,
            heap_size_bytes: 10_000_000,
            data: vec![0x01; 10],
        };

        let _ = capsule.take_snapshot(&metadata);
        assert!(capsule.snapshot_count() > 0);

        capsule.reset();
        assert_eq!(capsule.snapshot_count(), 0);
        assert_eq!(capsule.generation(), 0);
    }
}
