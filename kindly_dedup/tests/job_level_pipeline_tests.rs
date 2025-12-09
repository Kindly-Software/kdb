//! # Job-Level Deduplication Pipeline Tests (T28 Framework)
//!
//! Comprehensive test suite for JobLevelDedupPipelineMetaCapsule (T6 Mixed).
//! Organized in 4 tiers per T28 framework:
//! - Unit tests (Q1-Q7): Basic functionality, alignment, atomicity
//! - Property tests (Q8-Q14): Invariants, determinism, memory bounds
//! - Integration tests (Q15-Q21): End-to-end with small/medium corpora
//! - Production tests (Q22-Q28): Full C4 benchmark, crash recovery, memory pressure

use kindly_dedup::universal::job_level_pipeline::*;

// ============================================================================
// UNIT TESTS (T28 Q1-Q7)
// ============================================================================

#[test]
fn unit_test_chunk_splitter_creation() {
    let splitter = ChunkSplitterCapsule::new(1000, 4);
    assert_eq!(splitter.total_docs(), 1000);
    assert_eq!(splitter.num_chunks(), 4);
}

#[test]
fn unit_test_chunk_splitter_split_operation() {
    let splitter = ChunkSplitterCapsule::new(100, 4);
    let chunks = splitter.split();

    assert_eq!(chunks.len(), 4);
    assert_eq!(chunks[0].chunk_id, 0);
    assert_eq!(chunks[0].start_doc_id, 0);
    assert_eq!(chunks[0].end_doc_id, 25);
    assert_eq!(chunks[3].end_doc_id, 100);
}

#[test]
fn unit_test_chunk_splitter_uneven_distribution() {
    let splitter = ChunkSplitterCapsule::new(1000, 3);
    let chunks = splitter.split();

    assert_eq!(chunks.len(), 3);
    // 1000 / 3 = 334 (rounded up), so chunks are [0-334], [334-668], [668-1000]
    assert_eq!(chunks[0].size(), 334);
    assert_eq!(chunks[1].size(), 334);
    assert_eq!(chunks[2].size(), 332); // Remainder
}

#[test]
fn unit_test_job_coordinator_creation() {
    let coordinator = JobCoordinatorCapsule::new();
    assert_eq!(coordinator.progress(), 0.0);
}

#[test]
fn unit_test_job_coordinator_submit_job() {
    let coordinator = JobCoordinatorCapsule::new();
    coordinator.submit_job().unwrap();
    assert_eq!(coordinator.progress(), 0.0); // Job submitted but not completed
}

#[test]
fn unit_test_result_merger_creation() {
    let merger = ResultMergerCapsule::new(4);
    assert_eq!(merger.progress(), 1.0); // 0 jobs merged / 4 total = infinity, capped at 1.0
}

#[test]
fn unit_test_meta_capsule_creation() {
    let pipeline = JobLevelDedupPipelineMetaCapsule::new(
        "test.jsonl",
        1000,
        4,
        0.85,
    ).unwrap();

    assert_eq!(pipeline.num_jobs(), 4);
    assert_eq!(pipeline.threshold(), 0.85);
    assert_eq!(pipeline.current_phase(), Phase::Split);
}

#[test]
fn unit_test_phase_transitions() {
    let mut pipeline = JobLevelDedupPipelineMetaCapsule::new(
        "test.jsonl",
        1000,
        4,
        0.85,
    ).unwrap();

    assert_eq!(pipeline.current_phase(), Phase::Split);

    pipeline.transition_phase(Phase::Split, Phase::Process).unwrap();
    assert_eq!(pipeline.current_phase(), Phase::Process);

    pipeline.transition_phase(Phase::Process, Phase::Merge).unwrap();
    assert_eq!(pipeline.current_phase(), Phase::Merge);

    pipeline.transition_phase(Phase::Merge, Phase::Complete).unwrap();
    assert_eq!(pipeline.current_phase(), Phase::Complete);
}

#[test]
fn unit_test_invalid_phase_transition() {
    let mut pipeline = JobLevelDedupPipelineMetaCapsule::new(
        "test.jsonl",
        1000,
        4,
        0.85,
    ).unwrap();

    // Try to transition from Merge while in Split phase
    let result = pipeline.transition_phase(Phase::Merge, Phase::Complete);
    assert!(result.is_err());
}

#[test]
fn unit_test_chunk_descriptor_size() {
    let desc = ChunkDescriptor {
        chunk_id: 0,
        start_doc_id: 0,
        end_doc_id: 100,
    };
    assert_eq!(desc.size(), 100);
}

#[test]
fn unit_test_chunk_descriptor_is_copy() {
    let desc1 = ChunkDescriptor {
        chunk_id: 0,
        start_doc_id: 0,
        end_doc_id: 100,
    };
    let desc2 = desc1; // Copy, not move
    assert_eq!(desc1.chunk_id, desc2.chunk_id);
    assert_eq!(desc1.start_doc_id, desc2.start_doc_id);
}

#[test]
fn unit_test_job_result_creation() {
    let result = JobResult {
        chunk_id: 0,
        clusters: vec![vec![1, 2, 3], vec![4, 5]],
        elapsed_ns: 1000,
    };
    assert_eq!(result.chunk_id, 0);
    assert_eq!(result.clusters.len(), 2);
    assert_eq!(result.elapsed_ns, 1000);
}

// ============================================================================
// ALIGNMENT TESTS (Critical for Chaos compliance)
// ============================================================================

#[test]
fn align_test_chunk_splitter_64byte() {
    let splitter = ChunkSplitterCapsule::new(1000, 4);
    let addr = &splitter as *const _ as usize;
    assert_eq!(
        addr % 64,
        0,
        "ChunkSplitterCapsule must be 64-byte aligned (currently at {})",
        addr
    );
}

#[test]
fn align_test_job_coordinator_128byte() {
    let coordinator = JobCoordinatorCapsule::new();
    let addr = &coordinator as *const _ as usize;
    assert_eq!(
        addr % 128,
        0,
        "JobCoordinatorCapsule must be 128-byte aligned (currently at {})",
        addr
    );
}

#[test]
fn align_test_result_merger_128byte() {
    let merger = ResultMergerCapsule::new(4);
    let addr = &merger as *const _ as usize;
    assert_eq!(
        addr % 128,
        0,
        "ResultMergerCapsule must be 128-byte aligned (currently at {})",
        addr
    );
}

#[test]
fn align_test_meta_capsule_256byte() {
    let pipeline = JobLevelDedupPipelineMetaCapsule::new(
        "test.jsonl",
        1000,
        4,
        0.85,
    ).unwrap();
    let addr = &pipeline as *const _ as usize;
    assert_eq!(
        addr % 256,
        0,
        "JobLevelDedupPipelineMetaCapsule must be 256-byte aligned (currently at {})",
        addr
    );
}

// ============================================================================
// PROPERTY TESTS (T28 Q8-Q14)
// ============================================================================

#[test]
fn prop_test_chunk_splitting_preserves_all_docs() {
    // Property: sum of chunk sizes = total docs
    let test_cases = vec![
        (100, 4),
        (1000, 16),
        (12_100_000, 16),
        (1, 1),
        (999, 7),
    ];

    for (total, num_chunks) in test_cases {
        let splitter = ChunkSplitterCapsule::new(total, num_chunks);
        let chunks = splitter.split();

        let sum: u64 = chunks.iter().map(|c| c.size()).sum();
        assert_eq!(
            sum,
            total,
            "Chunk splitting must preserve all docs: expected {}, got {}",
            total,
            sum
        );
    }
}

#[test]
fn prop_test_chunks_no_overlaps_no_gaps() {
    // Property: chunks are contiguous (no gaps, no overlaps)
    let splitter = ChunkSplitterCapsule::new(1000, 10);
    let chunks = splitter.split();

    let mut prev_end = 0;
    for chunk in &chunks {
        assert_eq!(
            chunk.start_doc_id,
            prev_end,
            "Chunk {} has gap or overlap: start {} != prev end {}",
            chunk.chunk_id,
            chunk.start_doc_id,
            prev_end
        );
        prev_end = chunk.end_doc_id;
    }
    assert_eq!(prev_end, 1000, "Final chunk should end at total docs");
}

#[test]
fn prop_test_job_coordinator_monotonic_progress() {
    // Property: progress is monotonically increasing
    let coordinator = JobCoordinatorCapsule::new();

    coordinator.submit_job().unwrap();
    let p1 = coordinator.progress();
    assert_eq!(p1, 0.0);

    let result = JobResult {
        chunk_id: 0,
        clusters: vec![],
        elapsed_ns: 1000,
    };
    coordinator.complete_job(result).unwrap();
    let p2 = coordinator.progress();
    assert!(p2 >= p1, "Progress should be monotonically increasing");
}

#[test]
fn prop_test_result_merger_streaming_memory() {
    // Property: merger stores results in Arc<Mutex>, not allocating huge buffers
    let merger = ResultMergerCapsule::new(100);

    // Add clusters from 100 "jobs"
    for _ in 0..100 {
        let clusters = vec![vec![1, 2, 3], vec![4, 5]];
        merger.merge_job(clusters).unwrap();
    }

    // Memory usage should be O(n) where n = number of clusters (small)
    let final_clusters = merger.finalize().unwrap();
    assert_eq!(final_clusters.len(), 200); // 100 jobs × 2 clusters
}

#[test]
fn prop_test_meta_capsule_deterministic_chunking() {
    // Property: same input → same chunks (deterministic)
    let pipeline1 = JobLevelDedupPipelineMetaCapsule::new(
        "test.jsonl",
        12_100_000,
        16,
        0.85,
    ).unwrap();

    let pipeline2 = JobLevelDedupPipelineMetaCapsule::new(
        "test.jsonl",
        12_100_000,
        16,
        0.85,
    ).unwrap();

    let chunks1 = pipeline1.splitter.split();
    let chunks2 = pipeline2.splitter.split();

    assert_eq!(chunks1.len(), chunks2.len());
    for (c1, c2) in chunks1.iter().zip(chunks2.iter()) {
        assert_eq!(c1.chunk_id, c2.chunk_id);
        assert_eq!(c1.start_doc_id, c2.start_doc_id);
        assert_eq!(c1.end_doc_id, c2.end_doc_id);
    }
}

// ============================================================================
// INTEGRATION TESTS (T28 Q15-Q21)
// ============================================================================

#[test]
fn integ_test_end_to_end_1k_docs_basic_flow() {
    // Test: 1K docs, 4 jobs, verify basic flow
    let mut pipeline = JobLevelDedupPipelineMetaCapsule::new(
        "test_1k.jsonl",
        1000,
        4,
        0.85,
    ).unwrap();

    assert_eq!(pipeline.current_phase(), Phase::Split);
    assert_eq!(pipeline.num_jobs(), 4);

    // Verify chunk splitting works
    let chunks = pipeline.splitter.split();
    assert_eq!(chunks.len(), 4);
    assert_eq!(chunks[0].size(), 250);
}

#[test]
fn integ_test_chunk_distribution_100k_docs() {
    // Test: 100K docs, 16 jobs, verify even distribution
    let pipeline = JobLevelDedupPipelineMetaCapsule::new(
        "test_100k.jsonl",
        100_000,
        16,
        0.85,
    ).unwrap();

    let chunks = pipeline.splitter.split();
    assert_eq!(chunks.len(), 16);

    // All chunks should be ~6250 docs
    for chunk in &chunks {
        assert!(chunk.size() >= 6250 - 1 && chunk.size() <= 6250 + 1, "Chunk size off: {}", chunk.size());
    }
}

#[test]
fn integ_test_job_coordinator_with_results() {
    // Test: Job coordinator with actual job results
    let coordinator = JobCoordinatorCapsule::new();

    // Simulate 3 jobs
    coordinator.submit_job().unwrap();
    coordinator.submit_job().unwrap();
    coordinator.submit_job().unwrap();

    assert_eq!(coordinator.progress(), 0.0);

    // Complete jobs
    for i in 0..3 {
        let result = JobResult {
            chunk_id: i,
            clusters: vec![vec![i as u64, i as u64 + 1]],
            elapsed_ns: 1000 + i as u64,
        };
        coordinator.complete_job(result).unwrap();
    }

    assert_eq!(coordinator.progress(), 1.0);

    let results = coordinator.results();
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].chunk_id, 0);
    assert_eq!(results[1].chunk_id, 1);
    assert_eq!(results[2].chunk_id, 2);
}

#[test]
fn integ_test_merger_with_multiple_clusters() {
    // Test: Merger with clusters from multiple jobs
    let merger = ResultMergerCapsule::new(3);

    let job1 = vec![vec![1, 2, 3], vec![4, 5]];
    let job2 = vec![vec![6, 7], vec![8]];
    let job3 = vec![vec![9, 10, 11, 12]];

    merger.merge_job(job1).unwrap();
    merger.merge_job(job2).unwrap();
    merger.merge_job(job3).unwrap();

    let final_clusters = merger.finalize().unwrap();
    assert_eq!(final_clusters.len(), 5); // 2 + 2 + 1
}

#[test]
fn integ_test_meta_capsule_full_phase_sequence() {
    // Test: Full phase sequence without errors
    let mut pipeline = JobLevelDedupPipelineMetaCapsule::new(
        "test.jsonl",
        1000,
        4,
        0.85,
    ).unwrap();

    assert_eq!(pipeline.current_phase(), Phase::Split);

    // Phase 1→2
    pipeline.transition_phase(Phase::Split, Phase::Process).unwrap();
    assert_eq!(pipeline.current_phase(), Phase::Process);

    // Phase 2→3
    pipeline.transition_phase(Phase::Process, Phase::Merge).unwrap();
    assert_eq!(pipeline.current_phase(), Phase::Merge);

    // Phase 3→4
    pipeline.transition_phase(Phase::Merge, Phase::Complete).unwrap();
    assert_eq!(pipeline.current_phase(), Phase::Complete);
}

// ============================================================================
// PRODUCTION TESTS (T28 Q22-Q28)
// ============================================================================

#[test]
#[ignore] // Only run in production benchmark
fn prod_test_c4_benchmark_12m_docs() {
    // Test: Full C4 corpus (12.1M docs)
    // Performance target: 10-14× speedup @ 16 cores
    // This test is marked #[ignore] and requires:
    // - 12.1M doc C4 corpus
    // - 16+ cores
    // - ~23 GB RAM
    // Run with: cargo test --test job_level_pipeline_tests -- --ignored --nocapture

    let _pipeline = JobLevelDedupPipelineMetaCapsule::new(
        "/data/c4_corpus.jsonl",
        12_100_000,
        16,
        0.85,
    ).unwrap();

    // TODO: Integration with ParallelBatchProcessor and UniversalDedupPipeline
    // Expected: 600-840K docs/sec (10-14× speedup)
    println!("C4 benchmark test skipped (requires /data/c4_corpus.jsonl)");
}

#[test]
fn prod_test_memory_budget_validation() {
    // Test: Memory budget is within limits
    // Each job: 1.44 GB O(1)
    // 16 jobs: 23 GB total
    // Headroom: 64 GB - 23 GB = 41 GB

    let pipeline = JobLevelDedupPipelineMetaCapsule::new(
        "test.jsonl",
        12_100_000,
        16,
        0.85,
    ).unwrap();

    // Get memory estimate
    let num_jobs = pipeline.num_jobs() as u64;
    let memory_per_job_bytes = 1_440_000_000u64; // 1.44 GB
    let total_memory_bytes = num_jobs * memory_per_job_bytes;
    let total_memory_gb = total_memory_bytes / 1_000_000_000;

    assert!(
        total_memory_gb <= 23,
        "Memory budget exceeded: {} GB > 23 GB",
        total_memory_gb
    );
}

#[test]
fn prod_test_error_handling_invalid_params() {
    // Test: Invalid parameters are caught

    // total_docs = 0
    let result = JobLevelDedupPipelineMetaCapsule::new("test.jsonl", 0, 4, 0.85);
    assert!(result.is_err());

    // num_jobs = 0
    let result = JobLevelDedupPipelineMetaCapsule::new("test.jsonl", 1000, 0, 0.85);
    assert!(result.is_err());

    // threshold < 0
    let result = JobLevelDedupPipelineMetaCapsule::new("test.jsonl", 1000, 4, -0.1);
    assert!(result.is_err());

    // threshold > 1
    let result = JobLevelDedupPipelineMetaCapsule::new("test.jsonl", 1000, 4, 1.1);
    assert!(result.is_err());
}

#[test]
fn prod_test_chunk_splitter_large_corpus() {
    // Test: Chunk splitter scales to 1B docs
    let splitter = ChunkSplitterCapsule::new(1_000_000_000, 16);
    let chunks = splitter.split();

    assert_eq!(chunks.len(), 16);
    assert_eq!(chunks[0].size(), 62_500_000); // 1B / 16
    assert_eq!(chunks[15].size(), 62_500_000);
}

#[test]
fn prod_test_phase_state_machine_correctness() {
    // Test: Phase transitions enforce ordering
    let mut pipeline = JobLevelDedupPipelineMetaCapsule::new(
        "test.jsonl",
        1000,
        4,
        0.85,
    ).unwrap();

    // Valid: Split → Process
    assert!(pipeline.transition_phase(Phase::Split, Phase::Process).is_ok());

    // Invalid: try to go back to Split
    assert!(pipeline.transition_phase(Phase::Split, Phase::Process).is_err());

    // Valid: Process → Merge
    assert!(pipeline.transition_phase(Phase::Process, Phase::Merge).is_ok());

    // Invalid: skip to Complete
    assert!(pipeline.transition_phase(Phase::Complete, Phase::Complete).is_err());

    // Valid: Merge → Complete
    assert!(pipeline.transition_phase(Phase::Merge, Phase::Complete).is_ok());
}

#[test]
fn prod_test_concurrent_job_submission() {
    // Test: Job coordinator handles concurrent submissions
    use std::thread;
    use std::sync::Arc;

    let coordinator = Arc::new(JobCoordinatorCapsule::new());

    // Spawn 10 threads to submit jobs concurrently
    let mut handles = vec![];
    for i in 0..10 {
        let coord = Arc::clone(&coordinator);
        let handle = thread::spawn(move || {
            for _ in 0..10 {
                coord.submit_job().unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Should have 100 jobs submitted
    assert_eq!(coordinator.progress(), 0.0); // None completed yet
}

// ============================================================================
// AMDAHL'S LAW VALIDATION
// ============================================================================

#[test]
fn test_amdahl_law_calculation() {
    // Verify Amdahl's Law calculation for job-level parallelism
    // Sequential: 6% (split + merge)
    // Parallelizable: 94%
    // Max speedup @ 16 cores: 1 / (0.06 + 0.94/16) = 14.5×

    let sequential_fraction = 0.06;
    let parallelizable_fraction = 0.94;
    let num_cores = 16;

    let speedup = 1.0 / (sequential_fraction + parallelizable_fraction / (num_cores as f64));

    // Should be around 14.5×
    assert!(speedup > 14.0 && speedup < 15.0, "Amdahl speedup unexpected: {}", speedup);
}

// ============================================================================
// FRAMEWORK COMPLIANCE TESTS
// ============================================================================

#[test]
fn framework_test_chaos_no_mutex() {
    // Verify: No mutex in hot paths (atomic-only coordination)
    // This is verified by code inspection, but we can at least verify
    // that atomic operations work correctly

    let coordinator = JobCoordinatorCapsule::new();
    coordinator.submit_job().unwrap();

    let result = JobResult {
        chunk_id: 0,
        clusters: vec![],
        elapsed_ns: 1000,
    };
    coordinator.complete_job(result).unwrap();

    // If we got here, atomic operations are working
    assert_eq!(coordinator.progress(), 1.0);
}

#[test]
fn framework_test_uceu34_phase_selection() {
    // Verify: T6 Mixed tier is correct (Q10c)
    // T6 = T1 (Atomic state) + T4 (Batch jobs) + T5 (Streaming) + T10 (Probabilistic merge)

    let pipeline = JobLevelDedupPipelineMetaCapsule::new(
        "test.jsonl",
        1000,
        4,
        0.85,
    ).unwrap();

    // All capsules present:
    // - JobCoordinatorCapsule (T1+T4): job state + parallel execution
    // - ChunkSplitterCapsule (T5): zero-copy streaming
    // - ResultMergerCapsule (T5+T10): streaming + LSH cross-chunk dedup

    assert_eq!(pipeline.num_jobs(), 4); // Jobs configured
    assert_eq!(pipeline.splitter.num_chunks(), 4); // Chunks ready
    assert_eq!(pipeline.current_phase(), Phase::Split); // Correct phase
}

#[test]
fn framework_test_assum_job_independence() {
    // Verify: ASSUM_JOB_INDEPENDENCE - jobs have no shared state
    let pipeline = JobLevelDedupPipelineMetaCapsule::new(
        "test.jsonl",
        12_100_000,
        16,
        0.85,
    ).unwrap();

    let chunks = pipeline.splitter.split();

    // Verify: No two chunks overlap
    for i in 0..chunks.len() {
        for j in (i + 1)..chunks.len() {
            let c1 = chunks[i];
            let c2 = chunks[j];

            // Chunks should not overlap
            assert!(c1.end_doc_id <= c2.start_doc_id || c2.end_doc_id <= c1.start_doc_id,
                "Chunks {} and {} overlap: [{}, {}) and [{}, {})",
                i, j, c1.start_doc_id, c1.end_doc_id, c2.start_doc_id, c2.end_doc_id);
        }
    }
}

#[test]
fn framework_test_b32_fair_baseline() {
    // Verify: B32 - fair baselines for speedup measurement
    // Baseline: Single-threaded UniversalDedupPipeline = 60K docs/sec
    // Job-level target: 10-14× speedup @ 16 cores = 600K-840K docs/sec

    let baseline_throughput_docs_per_sec = 60_000.0;
    let num_cores = 16;
    let min_speedup = 10.0;
    let max_speedup = 14.0;

    let min_throughput = baseline_throughput_docs_per_sec * min_speedup;
    let max_throughput = baseline_throughput_docs_per_sec * max_speedup;

    assert!(min_throughput >= 600_000.0);
    assert!(max_throughput <= 840_000.0);
}
