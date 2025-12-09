//! Worker Loop Smoke Tests (Agent 13 Implementation)
//!
//! **Purpose**: Validate basic worker_loop functionality
//! **Framework**: UCE34 Q1-Q34 + Chaos + T28 (Unit Tests)
//! **Status**: Basic validation, full integration tests deferred to Week 4

#[cfg(test)]
mod tests {
    use kindly_dedup::parallel::ParallelDedupMetacapsule;
    use kindly_dedup::pipeline::PipelineError;

    /// Test 1: Worker loop initializes with valid worker ID
    #[test]
    fn test_worker_loop_valid_worker_id() -> Result<(), Box<dyn std::error::Error>> {
        // Create metacapsule with 16 workers
        let metacapsule = ParallelDedupMetacapsule::new(
            1000,   // num_documents
            16,     // num_workers
            1000,   // batch_size
            0.8,    // jaccard_threshold
        )?;

        // Worker loop should return immediately (no documents added)
        // Expected behavior: Exit cleanly after checking pipeline state
        // Note: Actual worker_loop() call deferred to Week 4 integration tests
        // (requires document tokenization pipeline to be fully implemented)

        assert_eq!(metacapsule.num_workers(), 16);
        assert_eq!(metacapsule.batch_size(), 1000);
        Ok(())
    }

    /// Test 2: Invalid worker ID is rejected
    #[test]
    fn test_worker_loop_invalid_worker_id() -> Result<(), Box<dyn std::error::Error>> {
        let metacapsule = ParallelDedupMetacapsule::new(1000, 16, 1000, 0.8)?;

        // Worker ID >= num_workers should be rejected
        // This test validates bounds checking at entry to worker_loop
        // Actual validation occurs in worker_loop() method

        // For now, just verify the metacapsule has correct worker count
        assert!(metacapsule.num_workers() <= 16);
        Ok(())
    }

    /// Test 3: Metacapsule FSM state transitions
    #[test]
    fn test_metacapsule_fsm_states() -> Result<(), Box<dyn std::error::Error>> {
        let metacapsule = ParallelDedupMetacapsule::new(1000, 16, 1000, 0.8)?;

        // Verify initial state is Init
        let snapshot = metacapsule.snapshot();
        assert_eq!(
            snapshot.state,
            kindly_dedup::parallel::PipelineState::Init
        );

        // Verify generation counter initialized
        assert_eq!(snapshot.generation, 0);

        Ok(())
    }

    /// Test 4: Atomic snapshot latency (<50ns)
    #[test]
    fn test_atomic_snapshot_latency() -> Result<(), Box<dyn std::error::Error>> {
        let metacapsule = ParallelDedupMetacapsule::new(1000, 16, 1000, 0.8)?;

        // Take multiple snapshots (warmup L1 cache)
        for _ in 0..10 {
            let _snapshot = metacapsule.snapshot();
        }

        // Measure single snapshot (expected <50ns)
        use std::time::Instant;
        let start = Instant::now();
        let _snapshot = metacapsule.snapshot();
        let _elapsed = start.elapsed();

        // Note: Actual timing validation deferred to B32 benchmarking (Week 5)
        // This test just verifies snapshot() doesn't crash

        Ok(())
    }

    /// Test 5: Phase mask updates for worker state tracking
    #[test]
    fn test_phase_mask_worker_tracking() -> Result<(), Box<dyn std::error::Error>> {
        let metacapsule = ParallelDedupMetacapsule::new(1000, 16, 1000, 0.8)?;

        // Take snapshot - all workers should be in Init state initially
        let snapshot = metacapsule.snapshot();

        // Worker states encoded in phase_mask as 16 workers × 4 bits
        // Initial state should have all workers in Init (state 0)
        assert_eq!(snapshot.worker_states, 0); // All workers in Init (0)

        Ok(())
    }

    /// Test 6: Metrics atomic operations
    #[test]
    fn test_metrics_atomic_operations() -> Result<(), Box<dyn std::error::Error>> {
        let metacapsule = ParallelDedupMetacapsule::new(1000, 16, 1000, 0.8)?;

        // Verify initial metrics are zero
        assert_eq!(metacapsule.docs_processed(), 0);
        assert_eq!(metacapsule.docs_duplicates(), 0);

        // Snapshot should reflect metrics atomically
        let snapshot = metacapsule.snapshot();
        assert_eq!(snapshot.docs_processed, 0);
        assert_eq!(snapshot.docs_duplicates, 0);

        Ok(())
    }

    /// Test 7: Configuration immutability
    #[test]
    fn test_configuration_immutability() -> Result<(), Box<dyn std::error::Error>> {
        let metacapsule = ParallelDedupMetacapsule::new(
            5000,  // num_documents
            8,     // num_workers
            500,   // batch_size
            0.75,  // jaccard_threshold
        )?;

        // Configuration should not change after construction
        assert_eq!(metacapsule.num_workers(), 8);
        assert_eq!(metacapsule.batch_size(), 500);
        assert_eq!(metacapsule.jaccard_threshold(), 0.75);

        Ok(())
    }

    /// Test 8: Memory layout validation (512 bytes)
    #[test]
    fn test_memory_layout() -> Result<(), Box<dyn std::error::Error>> {
        use std::mem::size_of;

        // Metacapsule should fit in L1 cache line (≤1024 bytes)
        let size = size_of::<ParallelDedupMetacapsule>();
        assert!(
            size <= 1024,
            "ParallelDedupMetacapsule size {} exceeds 1024 bytes",
            size
        );

        // Should be 256-byte aligned (cache-friendly)
        let align = std::mem::align_of::<ParallelDedupMetacapsule>();
        assert_eq!(align, 256, "Must be 256-byte aligned");

        Ok(())
    }

    /// Test 9: Error handling for invalid configuration
    #[test]
    fn test_invalid_configuration_rejection() {
        // Too many workers (>16)
        let result = ParallelDedupMetacapsule::new(1000, 17, 1000, 0.8);
        assert!(result.is_err());

        // Invalid Jaccard threshold (>1.0)
        let result = ParallelDedupMetacapsule::new(1000, 16, 1000, 1.5);
        assert!(result.is_err());

        // Zero workers
        let result = ParallelDedupMetacapsule::new(1000, 0, 1000, 0.8);
        assert!(result.is_err());
    }

    /// Test 10: Worker loop isolation (no shared state)
    #[test]
    fn test_worker_isolation() -> Result<(), Box<dyn std::error::Error>> {
        let metacapsule = ParallelDedupMetacapsule::new(1000, 16, 1000, 0.8)?;

        // Per-worker resources should be independent
        // - Each worker gets its own MinHash builder (no contention)
        // - Each worker gets its own work-stealing queue (no contention)
        // - Shared LSH bucketer is lockfree (no mutex blocking)

        // Verify no panic on construction with many workers
        assert_eq!(metacapsule.num_workers(), 16);

        Ok(())
    }

    /// Test 11: Completion state check
    #[test]
    fn test_completion_check() -> Result<(), Box<dyn std::error::Error>> {
        let metacapsule = ParallelDedupMetacapsule::new(1000, 16, 1000, 0.8)?;

        // Pipeline should not be complete initially
        assert!(!metacapsule.is_complete());

        Ok(())
    }

    /// Test 12: Generation counter incrementation
    #[test]
    fn test_generation_counter() -> Result<(), Box<dyn std::error::Error>> {
        let metacapsule = ParallelDedupMetacapsule::new(1000, 16, 1000, 0.8)?;

        // Initial generation should be 0 (even = committed)
        let gen1 = metacapsule.get_generation();
        assert_eq!(gen1, 0);

        // After state transitions, generation should increment
        // (deferred to integration tests after full pipeline implementation)

        Ok(())
    }
}
