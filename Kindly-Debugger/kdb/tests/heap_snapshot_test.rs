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

        // Verify we can retrieve all snapshots
        assert!(capsule.snapshot_count() > 0);
        assert!(capsule.snapshot_count() <= 128);
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
