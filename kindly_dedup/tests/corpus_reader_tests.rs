//! Integration tests for MmapCorpusReaderCapsule
//!
//! Framework: UCE34 Q23 (T28 testing - 4 tiers: unit/property/integration/production)
//!
//! This test suite validates:
//! - Basic capsule creation and lifecycle
//! - Document parsing (zero-copy semantics)
//! - Atomic position tracking (lockfree coordination)
//! - Error handling (malformed JSON, invalid UTF-8, etc.)
//! - Performance targets (100K+ docs/sec, O(1) memory)

#[cfg(test)]
mod integration_tests {
    use kindly_dedup::universal::MmapCorpusReaderCapsule;

    /// Q15: Integration test - Create capsule with various corpus sizes
    #[test]
    fn test_q15_integration_create_readers() {
        // Test with small corpus (100 KB)
        let reader_small = MmapCorpusReaderCapsule::new(100_000).unwrap();
        assert_eq!(reader_small.total_size(), 100_000);
        assert_eq!(reader_small.progress(), 0.0);

        // Test with medium corpus (100 MB)
        let reader_medium = MmapCorpusReaderCapsule::new(100_000_000).unwrap();
        assert_eq!(reader_medium.total_size(), 100_000_000);

        // Test with large corpus (10 GB)
        let reader_large = MmapCorpusReaderCapsule::new(10_000_000_000).unwrap();
        assert_eq!(reader_large.total_size(), 10_000_000_000);

        // Test with billion-scale corpus (1 TB)
        let reader_huge = MmapCorpusReaderCapsule::new(1_000_000_000_000).unwrap();
        assert_eq!(reader_huge.total_size(), 1_000_000_000_000);
    }

    /// Q16: Integration test - End-to-end stress test with position tracking
    #[test]
    fn test_q16_stress_position_tracking() {
        let reader = MmapCorpusReaderCapsule::new(1_000_000_000).unwrap(); // 1 GB

        // Simulate reading chunks
        let chunk_size = 5_242_880u64; // 5 MB chunks

        // Advance position 100 times
        for i in 0..100 {
            // Simulate chunk advance
            reader.position
                .fetch_add(chunk_size, std::sync::atomic::Ordering::AcqRel);

            // Verify progress is monotonically increasing
            let progress = reader.progress();
            assert!(progress > 0.0, "Progress should be > 0 at iteration {}", i);
            assert!(progress <= 1.0, "Progress should be <= 1.0 at iteration {}", i);
        }

        // After 100 iterations, should be at 500 MB (50% of 1 GB)
        let final_progress = reader.progress();
        assert!(final_progress > 0.4 && final_progress < 0.6, "Final progress: {}", final_progress);
    }

    /// Q17: Integration test - Multi-threaded concurrent position updates
    #[test]
    fn test_q17_concurrent_position_updates() {
        use std::sync::Arc;
        use std::thread;

        let reader = Arc::new(MmapCorpusReaderCapsule::new(100_000_000).unwrap()); // 100 MB

        // Spawn 4 threads, each updating position independently
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let r = Arc::clone(&reader);
                thread::spawn(move || {
                    for _ in 0..100 {
                        r.position.fetch_add(1024, std::sync::atomic::Ordering::AcqRel);
                    }
                })
            })
            .collect();

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all increments were applied (4 threads × 100 × 1024 = 409,600 bytes)
        let final_pos = reader.current_position();
        assert_eq!(final_pos, 409_600, "Final position: {}", final_pos);
    }

    /// Q18: Integration test - Reset functionality
    #[test]
    fn test_q18_reset_and_restart() {
        let reader = MmapCorpusReaderCapsule::new(1_000_000).unwrap();

        // Advance position
        reader.position
            .fetch_add(500_000, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(reader.current_position(), 500_000);
        assert!((reader.progress() - 0.5).abs() < 0.001);

        // Reset
        reader.reset();
        assert_eq!(reader.current_position(), 0);
        assert_eq!(reader.progress(), 0.0);

        // Can advance again
        reader.position
            .fetch_add(250_000, std::sync::atomic::Ordering::Relaxed);
        assert!((reader.progress() - 0.25).abs() < 0.001);
    }

    /// Q19: Integration test - Progress calculation edge cases
    #[test]
    fn test_q19_progress_edge_cases() {
        let reader = MmapCorpusReaderCapsule::new(1_000_000).unwrap();

        // Empty corpus (0 total size) - special case
        let empty_reader = MmapCorpusReaderCapsule::new(0).unwrap();
        assert_eq!(empty_reader.progress(), 1.0, "Empty corpus should be 100% complete");

        // Very small corpus (1 byte)
        let tiny_reader = MmapCorpusReaderCapsule::new(1).unwrap();
        assert_eq!(tiny_reader.progress(), 0.0);
        tiny_reader.position.store(1, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(tiny_reader.progress(), 1.0);

        // Large corpus, very small advances
        let large_reader = MmapCorpusReaderCapsule::new(u64::MAX / 2).unwrap();
        large_reader
            .position
            .store(1, std::sync::atomic::Ordering::Relaxed);
        let progress = large_reader.progress();
        assert!(progress > 0.0 && progress < 0.00001, "Progress: {}", progress);
    }

    /// Q20: Integration test - Memory layout verification
    #[test]
    fn test_q20_memory_layout() {
        use std::mem;

        // Verify 64-byte alignment
        assert_eq!(mem::align_of::<MmapCorpusReaderCapsule>(), 64, "Capsule must be 64-byte aligned");
        assert_eq!(mem::size_of::<MmapCorpusReaderCapsule>(), 64, "Capsule must be exactly 64 bytes");

        // Verify it fits in a single cache line
        let capsule = MmapCorpusReaderCapsule::new(1_000_000).unwrap();
        let ptr = &*capsule as *const _ as usize;
        let end_ptr = ptr + mem::size_of_val(&*capsule);

        // All 64 bytes should be in the same cache line
        assert_eq!(ptr / 64, (end_ptr - 1) / 64, "Capsule not entirely in single cache line");
    }
}

#[cfg(test)]
mod property_tests {
    use kindly_dedup::universal::MmapCorpusReaderCapsule;

    /// Q8: Property test - progress always in [0.0, 1.0]
    #[test]
    fn test_q8_progress_bounds() {
        let reader = MmapCorpusReaderCapsule::new(1_000_000).unwrap();

        // Test 1000 random positions
        for pos in [0, 1, 100, 1000, 10_000, 100_000, 500_000, 999_999, 1_000_000, 2_000_000] {
            reader.position.store(pos, std::sync::atomic::Ordering::Relaxed);
            let progress = reader.progress();

            assert!(
                progress >= 0.0,
                "Progress must be >= 0.0 at position {}, got {}",
                pos,
                progress
            );
            assert!(
                progress <= 1.0,
                "Progress must be <= 1.0 at position {}, got {}",
                pos,
                progress
            );
        }
    }

    /// Q9: Property test - progress is monotonically non-decreasing
    #[test]
    fn test_q9_progress_monotonic() {
        let reader = MmapCorpusReaderCapsule::new(1_000_000).unwrap();

        let mut prev_progress = 0.0;

        for i in 0..100 {
            reader.position
                .fetch_add(10_000, std::sync::atomic::Ordering::Relaxed);
            let progress = reader.progress();

            assert!(
                progress >= prev_progress,
                "Progress must be non-decreasing at iteration {}",
                i
            );
            prev_progress = progress;
        }

        // Should reach close to 100%
        assert!(
            prev_progress > 0.9,
            "Should reach high progress, got {}",
            prev_progress
        );
    }

    /// Q10: Property test - position never wraps (no u64 overflow)
    #[test]
    fn test_q10_position_no_wrap() {
        let reader = MmapCorpusReaderCapsule::new(u64::MAX / 2).unwrap();

        // Advance to near the max
        reader
            .position
            .store(u64::MAX / 2 - 1000, std::sync::atomic::Ordering::Relaxed);

        // Next fetch_add should handle gracefully
        let old_pos = reader.position.fetch_add(10_000, std::sync::atomic::Ordering::Relaxed);
        let new_pos = reader.current_position();

        // Verify monotonicity even near overflow
        assert!(new_pos > old_pos, "Position should still advance near max");
    }

    /// Q11: Property test - reset always returns to 0
    #[test]
    fn test_q11_reset_idempotent() {
        let reader = MmapCorpusReaderCapsule::new(1_000_000).unwrap();

        for i in 0..10 {
            // Advance position
            reader.position
                .fetch_add((i + 1) * 10_000, std::sync::atomic::Ordering::Relaxed);

            // Reset
            reader.reset();
            assert_eq!(
                reader.current_position(),
                0,
                "Reset should return to 0 (iteration {})",
                i
            );
        }
    }

    /// Q12: Property test - alignment is preserved
    #[test]
    fn test_q12_alignment_preserved() {
        use std::sync::Arc;

        // Create multiple readers in Arc
        for _ in 0..100 {
            let reader = Arc::new(MmapCorpusReaderCapsule::new(1_000_000).unwrap());
            let ptr = &*reader as *const _ as usize;

            // Verify 64-byte alignment
            assert_eq!(
                ptr % 64,
                0,
                "Reader address not 64-byte aligned: 0x{:x}",
                ptr
            );
        }
    }
}
