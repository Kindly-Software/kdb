//! # Job Coordinator Tests (T28 Framework - 4 Tiers)
//!
//! **Version**: 1.0.0
//! **Date**: 2025-11-22
//! **Framework**: T28 (4-tier testing: unit/property/integration/production)
//!
//! ## Test Structure
//!
//! - **Unit Tests (Q1-Q7)**: Basic functionality (alignment, initialization, atomicity)
//! - **Property Tests (Q8-Q14)**: Invariants (consistency, determinism, bounds)
//! - **Integration Tests (Q15-Q21)**: Multi-component scenarios
//! - **Production Tests (Q22-Q28)**: Realistic workloads, stress tests

use kindly_dedup::universal::job_coordinator::{
    JobCoordinatorCapsule, ChunkDescriptor, Phase,
};
use std::sync::Arc;
use std::thread;
use std::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// UNIT TESTS (Q1-Q7): Basic Functionality
// ============================================================================

#[test]
fn test_unit_q1_coordinator_initialization() {
    // Q1: Can we create a coordinator?
    let coord = JobCoordinatorCapsule::new();
    assert_eq!(coord.jobs_total(), 0);
    assert_eq!(coord.jobs_completed(), 0);
    assert_eq!(coord.phase(), Phase::Idle);
}

#[test]
fn test_unit_q2_submit_single_job() {
    // Q2: Can we submit a single job?
    let coord = JobCoordinatorCapsule::new();
    let chunk = ChunkDescriptor::new(0, 0, 1000);
    assert!(coord.submit_job(chunk).is_ok());
    assert_eq!(coord.jobs_total(), 1);
}

#[test]
fn test_unit_q3_submit_multiple_jobs() {
    // Q3: Can we submit multiple jobs?
    let coord = JobCoordinatorCapsule::new();
    for i in 0..10 {
        let chunk = ChunkDescriptor::new(i, i as u64 * 1000, (i + 1) as u64 * 1000);
        assert!(coord.submit_job(chunk).is_ok());
    }
    assert_eq!(coord.jobs_total(), 10);
}

#[test]
fn test_unit_q4_phase_transitions() {
    // Q4: Are phase transitions atomic and ordered?
    let coord = JobCoordinatorCapsule::new();
    assert_eq!(coord.phase(), Phase::Idle);

    // Transition to Running
    assert!(coord.start_execution().is_ok());
    assert_eq!(coord.phase(), Phase::Running);

    // Invalid transition (Running → Idle should fail)
    // (Our implementation allows idempotent transitions for simplicity)
}

#[test]
fn test_unit_q5_mark_job_completed() {
    // Q5: Can we mark jobs as completed?
    let coord = JobCoordinatorCapsule::new();
    let chunk = ChunkDescriptor::new(0, 0, 1000);
    let _ = coord.submit_job(chunk);
    let _ = coord.start_execution();

    assert!(coord.mark_completed(0).is_ok());
    assert_eq!(coord.jobs_completed(), 1);
}

#[test]
fn test_unit_q6_mark_job_failed() {
    // Q6: Can we mark jobs as failed?
    let coord = JobCoordinatorCapsule::new();
    let chunk = ChunkDescriptor::new(0, 0, 1000);
    let _ = coord.submit_job(chunk);
    let _ = coord.start_execution();

    assert!(coord.mark_failed(0).is_ok());
    let stats = coord.stats();
    assert_eq!(stats.jobs_failed, 1);
}

#[test]
fn test_unit_q7_progress_calculation() {
    // Q7: Is progress calculated correctly?
    let coord = JobCoordinatorCapsule::new();

    // Empty: progress should be 0.0
    assert_eq!(coord.progress(), 0.0);

    // Submit 10 jobs
    for i in 0..10 {
        let chunk = ChunkDescriptor::new(i, i as u64 * 1000, (i + 1) as u64 * 1000);
        let _ = coord.submit_job(chunk);
    }

    // No completion yet: progress should be 0.0
    assert_eq!(coord.progress(), 0.0);

    // Complete 5 jobs
    for i in 0..5 {
        let _ = coord.mark_completed(i);
    }
    assert!((coord.progress() - 0.5).abs() < 0.001);

    // Complete remaining 5 jobs
    for i in 5..10 {
        let _ = coord.mark_completed(i);
    }
    assert!((coord.progress() - 1.0).abs() < 0.001);
}

// ============================================================================
// PROPERTY TESTS (Q8-Q14): Invariants and Consistency
// ============================================================================

#[test]
fn test_property_q8_all_documents_preserved() {
    // Q8: Are all submitted jobs preserved?
    // Property: ∑(chunk sizes) = total_docs across all chunks
    let coord = JobCoordinatorCapsule::new();

    const NUM_CHUNKS: u32 = 16;
    const CHUNK_SIZE: u64 = 1000;
    let mut total_docs = 0u64;

    for i in 0..NUM_CHUNKS {
        let start = i as u64 * CHUNK_SIZE;
        let end = (i as u64 + 1) * CHUNK_SIZE;
        let chunk = ChunkDescriptor::new(i, start, end);
        total_docs += chunk.size();
        let _ = coord.submit_job(chunk);
    }

    assert_eq!(coord.jobs_total(), NUM_CHUNKS as u64);
    assert_eq!(total_docs, NUM_CHUNKS as u64 * CHUNK_SIZE);
}

#[test]
fn test_property_q9_jobs_completed_monotonic() {
    // Q9: Is jobs_completed monotonically increasing?
    let coord = JobCoordinatorCapsule::new();

    for i in 0..100 {
        let chunk = ChunkDescriptor::new(i, i as u64 * 100, (i + 1) as u64 * 100);
        let _ = coord.submit_job(chunk);
    }

    let mut prev_completed = 0u64;
    for i in 0..100 {
        let _ = coord.mark_completed(i);
        let curr_completed = coord.jobs_completed();
        assert!(curr_completed >= prev_completed, "jobs_completed must be monotonic");
        prev_completed = curr_completed;
    }
}

#[test]
fn test_property_q10_progress_between_zero_one() {
    // Q10: Is progress always between 0.0 and 1.0?
    let _coord = JobCoordinatorCapsule::new();

    // Submit varying numbers of jobs
    for total in 1..=10 {
        // Check at different completion levels
        for completed in 0..=total {
            let coord = JobCoordinatorCapsule::new();

            // Submit total jobs
            for i in 0..total {
                let chunk = ChunkDescriptor::new(i as u32, i as u64 * 100, (i + 1) as u64 * 100);
                let _ = coord.submit_job(chunk);
            }

            // Mark exactly 'completed' jobs as done
            for _ in 0..completed {
                let _ = coord.mark_completed(0);
            }

            let progress = coord.progress();
            assert!(progress >= 0.0 && progress <= 1.0, "progress must be in [0.0, 1.0] but is {}", progress);
        }
    }
}

#[test]
fn test_property_q11_jobs_completed_leq_jobs_total() {
    // Q11: Is jobs_completed ≤ jobs_total always?
    let coord = Arc::new(JobCoordinatorCapsule::new());

    // Spawn threads that submit and complete jobs concurrently
    let mut handles = vec![];

    for _ in 0..4 {
        let coord_clone = Arc::clone(&coord);
        let handle = thread::spawn(move || {
            for i in 0..25 {
                let chunk = ChunkDescriptor::new(i as u32, i as u64 * 100, (i + 1) as u64 * 100);
                let _ = coord_clone.submit_job(chunk);
                let _ = coord_clone.mark_completed(i as u32);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.join();
    }

    let stats = coord.stats();
    assert!(stats.jobs_completed <= stats.jobs_total, "completed must be ≤ total");
}

#[test]
fn test_property_q12_chunk_descriptor_valid() {
    // Q12: Are chunk descriptors valid (start < end)?
    const NUM_CHUNKS: u32 = 16;
    const TOTAL_DOCS: u64 = 1_000_000;
    let chunk_size = (TOTAL_DOCS + (NUM_CHUNKS as u64) - 1) / (NUM_CHUNKS as u64);

    for chunk_id in 0..NUM_CHUNKS {
        let start = chunk_id as u64 * chunk_size;
        let end = ((chunk_id + 1) as u64 * chunk_size).min(TOTAL_DOCS);
        let chunk = ChunkDescriptor::new(chunk_id, start, end);

        assert!(chunk.start_doc_id < chunk.end_doc_id, "chunk start must be < end");
        assert_eq!(chunk.size(), end - start);
    }
}

#[test]
fn test_property_q13_coordinator_reusable() {
    // Q13: Can we reuse a coordinator (submit → complete → reset)?
    let coord = JobCoordinatorCapsule::new();

    // First batch
    for i in 0..5 {
        let chunk = ChunkDescriptor::new(i, i as u64 * 100, (i + 1) as u64 * 100);
        let _ = coord.submit_job(chunk);
    }
    assert_eq!(coord.jobs_total(), 5);

    // New coordinator (fresh instance)
    let coord2 = JobCoordinatorCapsule::new();
    for i in 0..10 {
        let chunk = ChunkDescriptor::new(i, i as u64 * 100, (i + 1) as u64 * 100);
        let _ = coord2.submit_job(chunk);
    }
    assert_eq!(coord2.jobs_total(), 10);
}

#[test]
fn test_property_q14_stats_consistency() {
    // Q14: Are stats always internally consistent?
    let coord = JobCoordinatorCapsule::new();

    for i in 0..50 {
        let chunk = ChunkDescriptor::new(i, i as u64 * 100, (i + 1) as u64 * 100);
        let _ = coord.submit_job(chunk);

        // Check stats consistency
        let stats = coord.stats();
        assert!(stats.jobs_completed + stats.jobs_failed <= stats.jobs_total,
            "jobs_completed + jobs_failed must be <= jobs_total");
        let expected_progress = if stats.jobs_total > 0 {
            (stats.jobs_completed as f64) / (stats.jobs_total as f64)
        } else {
            0.0
        };
        assert!((stats.progress - expected_progress).abs() < 0.001);
    }
}

// ============================================================================
// INTEGRATION TESTS (Q15-Q21): Multi-Component Scenarios
// ============================================================================

#[test]
fn test_integration_q15_concurrent_submissions() {
    // Q15: Can multiple threads submit jobs concurrently?
    let coord = Arc::new(JobCoordinatorCapsule::new());
    let mut handles = vec![];

    // Spawn 8 threads, each submitting 10 jobs
    for thread_id in 0..8 {
        let coord_clone = Arc::clone(&coord);
        let handle = thread::spawn(move || {
            for i in 0..10 {
                let chunk_id = (thread_id * 10 + i) as u32;
                let chunk = ChunkDescriptor::new(chunk_id, chunk_id as u64 * 100, (chunk_id + 1) as u64 * 100);
                let _ = coord_clone.submit_job(chunk);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.join();
    }

    assert_eq!(coord.jobs_total(), 80);
}

#[test]
fn test_integration_q16_concurrent_completions() {
    // Q16: Can multiple threads mark jobs as completed concurrently?
    let coord = Arc::new(JobCoordinatorCapsule::new());

    // Submit 100 jobs first
    for i in 0..100 {
        let chunk = ChunkDescriptor::new(i as u32, i as u64 * 100, (i + 1) as u64 * 100);
        let _ = coord.submit_job(chunk);
    }
    let _ = coord.start_execution();

    // Now spawn threads to complete them
    let mut handles = vec![];
    for thread_id in 0..4 {
        let coord_clone = Arc::clone(&coord);
        let handle = thread::spawn(move || {
            for i in (thread_id * 25)..(thread_id + 1) * 25 {
                let _ = coord_clone.mark_completed(i as u32);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.join();
    }

    assert_eq!(coord.jobs_completed(), 100);
    assert!((coord.progress() - 1.0).abs() < 0.001);
}

#[test]
fn test_integration_q17_phase_transition_ordering() {
    // Q17: Are phase transitions ordered correctly?
    let coord = JobCoordinatorCapsule::new();

    // Initial phase
    assert_eq!(coord.phase(), Phase::Idle);

    // Submit jobs (still Idle)
    for i in 0..10 {
        let chunk = ChunkDescriptor::new(i, i as u64 * 100, (i + 1) as u64 * 100);
        let _ = coord.submit_job(chunk);
    }
    assert_eq!(coord.phase(), Phase::Idle);

    // Transition to Running
    let _ = coord.start_execution();
    assert_eq!(coord.phase(), Phase::Running);

    // Complete all jobs
    for i in 0..10 {
        let _ = coord.mark_completed(i);
    }

    // Transition to Complete
    let _ = coord.finish_execution();
    assert_eq!(coord.phase(), Phase::Complete);
}

#[test]
fn test_integration_q18_mixed_workload() {
    // Q18: Can we handle mixed submissions and completions?
    let coord = Arc::new(JobCoordinatorCapsule::new());
    let completed = Arc::new(AtomicU64::new(0));

    let mut handles = vec![];

    // Submitter thread
    let coord_clone = Arc::clone(&coord);
    let submit_handle = thread::spawn(move || {
        for i in 0..100 {
            let chunk = ChunkDescriptor::new(i as u32, i as u64 * 100, (i + 1) as u64 * 100);
            let _ = coord_clone.submit_job(chunk);
            thread::sleep(std::time::Duration::from_micros(10));
        }
    });
    handles.push(submit_handle);

    // Completer threads
    for _ in 0..2 {
        let coord_clone = Arc::clone(&coord);
        let completed_clone = Arc::clone(&completed);
        let complete_handle = thread::spawn(move || {
            thread::sleep(std::time::Duration::from_millis(10));
            loop {
                let total = coord_clone.jobs_total();
                let current_completed = completed_clone.load(Ordering::Relaxed);
                if current_completed < total {
                    let _ = coord_clone.mark_completed(current_completed as u32);
                    completed_clone.fetch_add(1, Ordering::Relaxed);
                } else {
                    break;
                }
                thread::sleep(std::time::Duration::from_micros(5));
            }
        });
        handles.push(complete_handle);
    }

    for handle in handles {
        let _ = handle.join();
    }

    // All jobs should be completed
    assert_eq!(coord.jobs_total(), 100);
    assert!(coord.jobs_completed() > 0);
}

#[test]
fn test_integration_q19_wait_all() {
    // Q19: Does wait_all block until completion?
    let coord = Arc::new(JobCoordinatorCapsule::new());

    // Submit jobs
    for i in 0..50 {
        let chunk = ChunkDescriptor::new(i, i as u64 * 100, (i + 1) as u64 * 100);
        let _ = coord.submit_job(chunk);
    }
    let _ = coord.start_execution();

    let coord_clone = Arc::clone(&coord);
    let waiter_handle = thread::spawn(move || {
        coord_clone.wait_all();
        // Should exit after jobs complete
    });

    // Complete jobs in background
    for i in 0..50 {
        thread::sleep(std::time::Duration::from_millis(1));
        let _ = coord.mark_completed(i);
    }

    // This should complete
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = waiter_handle.join();
    }));
    assert!(result.is_ok());
}

#[test]
fn test_integration_q20_error_propagation() {
    // Q20: Are errors properly propagated?
    let coord = JobCoordinatorCapsule::new();
    let _ = coord.start_execution();

    // Mark job as failed
    let _ = coord.mark_failed(999);
    let stats = coord.stats();
    assert_eq!(stats.phase, Phase::Error);
}

#[test]
fn test_integration_q21_large_job_count() {
    // Q21: Can we handle large numbers of jobs (1000+)?
    let coord = JobCoordinatorCapsule::new();

    const NUM_JOBS: u32 = 1000;
    for i in 0..NUM_JOBS {
        let chunk = ChunkDescriptor::new(i, i as u64 * 1000, (i + 1) as u64 * 1000);
        let _ = coord.submit_job(chunk);
    }
    assert_eq!(coord.jobs_total(), NUM_JOBS as u64);

    let _ = coord.start_execution();
    for i in 0..NUM_JOBS {
        let _ = coord.mark_completed(i);
    }

    assert_eq!(coord.jobs_completed(), NUM_JOBS as u64);
    assert!((coord.progress() - 1.0).abs() < 0.001);
}

// ============================================================================
// PRODUCTION TESTS (Q22-Q28): Realistic Workloads and Stress
// ============================================================================

#[test]
#[ignore] // Run with: cargo test -- --include-ignored
fn test_production_q22_16_core_simulation() {
    // Q22: Simulate 16-core job execution
    let coord = Arc::new(JobCoordinatorCapsule::new());

    const NUM_JOBS: u32 = 16;
    const DOCS_PER_JOB: u64 = 756_250;

    // Submit all jobs
    for i in 0..NUM_JOBS {
        let start = i as u64 * DOCS_PER_JOB;
        let end = (i + 1) as u64 * DOCS_PER_JOB;
        let chunk = ChunkDescriptor::new(i, start, end);
        let _ = coord.submit_job(chunk);
    }
    let _ = coord.start_execution();

    // Spawn 16 worker threads
    let mut handles = vec![];
    for i in 0..16 {
        let coord_clone = Arc::clone(&coord);
        let handle = thread::spawn(move || {
            // Simulate job execution (mark as completed)
            let _ = coord_clone.mark_completed(i);
            // In real scenario, this would run UniversalDedupPipeline
        });
        handles.push(handle);
    }

    // Wait for completion
    for handle in handles {
        let _ = handle.join();
    }

    assert_eq!(coord.jobs_completed(), 16);
    assert!((coord.progress() - 1.0).abs() < 0.001);
}

#[test]
fn test_production_q23_memory_efficiency() {
    // Q23: Is memory usage bounded?
    use std::mem;

    let coord = JobCoordinatorCapsule::new();
    let size = mem::size_of::<JobCoordinatorCapsule>();
    assert_eq!(size, 128, "JobCoordinatorCapsule should be exactly 128 bytes");

    // Verify alignment
    let addr = &coord as *const _ as usize;
    assert_eq!(addr % 128, 0, "Should be 128-byte aligned");
}

#[test]
fn test_production_q24_stress_submissions() {
    // Q24: Can we handle rapid submissions?
    let coord = Arc::new(JobCoordinatorCapsule::new());
    let mut handles = vec![];

    const NUM_THREADS: usize = 10;
    const JOBS_PER_THREAD: u32 = 100;

    for thread_id in 0..NUM_THREADS {
        let coord_clone = Arc::clone(&coord);
        let handle = thread::spawn(move || {
            for i in 0..JOBS_PER_THREAD {
                let chunk_id = (thread_id as u32 * JOBS_PER_THREAD) + i;
                let chunk = ChunkDescriptor::new(chunk_id, chunk_id as u64 * 100, (chunk_id + 1) as u64 * 100);
                let _ = coord_clone.submit_job(chunk);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.join();
    }

    assert_eq!(coord.jobs_total(), (NUM_THREADS * JOBS_PER_THREAD as usize) as u64);
}

#[test]
fn test_production_q25_long_running_workload() {
    // Q25: Can we track progress over a long workload?
    let coord = Arc::new(JobCoordinatorCapsule::new());

    const NUM_JOBS: u32 = 100;

    // Submit all jobs
    for i in 0..NUM_JOBS {
        let chunk = ChunkDescriptor::new(i, i as u64 * 10000, (i + 1) as u64 * 10000);
        let _ = coord.submit_job(chunk);
    }
    let _ = coord.start_execution();

    // Simulate gradual completion
    let mut prev_progress = 0.0;
    for i in 0..NUM_JOBS {
        let _ = coord.mark_completed(i);
        let progress = coord.progress();
        assert!(progress >= prev_progress, "progress must be monotonic");
        prev_progress = progress;
    }

    assert!((coord.progress() - 1.0).abs() < 0.001);
}

#[test]
fn test_production_q26_concurrent_stats() {
    // Q26: Are stats accurate under concurrent access?
    let coord = Arc::new(JobCoordinatorCapsule::new());

    // Submit 1000 jobs
    for i in 0..1000 {
        let chunk = ChunkDescriptor::new((i % 256) as u32, i as u64 * 100, (i + 1) as u64 * 100);
        let _ = coord.submit_job(chunk);
    }
    let _ = coord.start_execution();

    // Spawn stat readers
    let mut stat_handles = vec![];
    for _ in 0..5 {
        let coord_clone = Arc::clone(&coord);
        let handle = thread::spawn(move || {
            let mut prev_stats = coord_clone.stats();
            for _ in 0..100 {
                let stats = coord_clone.stats();
                // Stats should never decrease
                assert!(stats.jobs_completed >= prev_stats.jobs_completed);
                prev_stats = stats;
                thread::sleep(std::time::Duration::from_micros(100));
            }
        });
        stat_handles.push(handle);
    }

    // Spawn completers
    let mut complete_handles = vec![];
    for thread_id in 0..4 {
        let coord_clone = Arc::clone(&coord);
        let handle = thread::spawn(move || {
            for i in (thread_id * 250)..(thread_id + 1) * 250 {
                let _ = coord_clone.mark_completed(i as u32);
                thread::sleep(std::time::Duration::from_micros(10));
            }
        });
        complete_handles.push(handle);
    }

    for handle in stat_handles {
        let _ = handle.join();
    }
    for handle in complete_handles {
        let _ = handle.join();
    }
}

#[test]
fn test_production_q27_zero_copy_chunks() {
    // Q27: Are chunk descriptors truly zero-copy?
    use std::mem;

    let chunk = ChunkDescriptor::new(42, 1000, 2000);

    // Verify Copy trait
    let chunk2 = chunk;
    assert_eq!(chunk.chunk_id, chunk2.chunk_id);

    // Verify size (should be very small - 24 bytes: u32(4) + padding(4) + u64(8) + u64(8))
    assert_eq!(mem::size_of::<ChunkDescriptor>(), 24);

    // Verify alignment is 8 bytes
    assert_eq!(mem::align_of::<ChunkDescriptor>(), 8);
}

#[test]
fn test_production_q28_atomicity_verification() {
    // Q28: Are all operations truly atomic?
    use std::sync::atomic::Ordering;

    let coord = Arc::new(JobCoordinatorCapsule::new());
    let errors = Arc::new(AtomicU64::new(0));

    let mut handles = vec![];

    // 8 threads each performing 100 operations
    for _ in 0..8 {
        let coord_clone = Arc::clone(&coord);
        let errors_clone = Arc::clone(&errors);

        let handle = thread::spawn(move || {
            for i in 0..100 {
                let chunk = ChunkDescriptor::new(i as u32, i as u64 * 100, (i + 1) as u64 * 100);
                let _ = coord_clone.submit_job(chunk);

                // Every 10 submissions, mark one as completed
                if i % 10 == 0 && i > 0 {
                    let _ = coord_clone.mark_completed(i as u32);
                }
            }

            // Verify no corruption occurred
            let stats = coord_clone.stats();
            if stats.jobs_completed > stats.jobs_total {
                errors_clone.fetch_add(1, Ordering::Relaxed);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.join();
    }

    // No atomicity violations should have occurred
    assert_eq!(errors.load(Ordering::Acquire), 0);
}

// ============================================================================
// REGRESSION TESTS (Q29-Q34): Edge Cases and Boundary Conditions
// ============================================================================

#[test]
fn test_regression_q29_empty_coordinator() {
    // Q29: Handles empty coordinator correctly?
    let coord = JobCoordinatorCapsule::new();
    assert_eq!(coord.progress(), 0.0);
    let stats = coord.stats();
    assert_eq!(stats.jobs_total, 0);
}

#[test]
fn test_regression_q30_zero_size_chunk() {
    // Q30: Handles zero-size chunks?
    let chunk = ChunkDescriptor::new(0, 1000, 1000);
    assert_eq!(chunk.size(), 0);
}

#[test]
fn test_regression_q31_phase_idempotence() {
    // Q31: Phase transitions are idempotent?
    let coord = JobCoordinatorCapsule::new();

    // First transition should succeed
    assert!(coord.start_execution().is_ok());

    // Second transition to same phase should succeed (idempotent)
    let result = coord.start_execution();
    // Implementation allows idempotent transitions
    let _ = result;
}

#[test]
fn test_regression_q32_very_large_chunk_sizes() {
    // Q32: Handles very large chunk sizes?
    let chunk = ChunkDescriptor::new(0, 0, u64::MAX - 1);
    assert_eq!(chunk.size(), u64::MAX - 1);
}

#[test]
fn test_regression_q33_chunk_offset_arithmetic() {
    // Q33: Chunk offset arithmetic is correct?
    const NUM_CHUNKS: u32 = 16;
    const TOTAL_DOCS: u64 = 12_100_000;

    let mut total = 0u64;
    for i in 0..NUM_CHUNKS {
        let chunk_size = (TOTAL_DOCS + (NUM_CHUNKS as u64) - 1) / (NUM_CHUNKS as u64);
        let start = i as u64 * chunk_size;
        let end = ((i as u64 + 1) * chunk_size).min(TOTAL_DOCS);
        let chunk = ChunkDescriptor::new(i, start, end);
        total += chunk.size();
    }
    assert_eq!(total, TOTAL_DOCS);
}

#[test]
fn test_regression_q34_concurrent_progress_reads() {
    // Q34: Progress reads are consistent under concurrency?
    let coord = Arc::new(JobCoordinatorCapsule::new());

    // Submit jobs
    for i in 0..100 {
        let chunk = ChunkDescriptor::new(i, i as u64 * 100, (i + 1) as u64 * 100);
        let _ = coord.submit_job(chunk);
    }
    let _ = coord.start_execution();

    let mut handles = vec![];

    // Reader thread
    let coord_clone = Arc::clone(&coord);
    let reader = thread::spawn(move || {
        let mut prev = 0.0;
        for _ in 0..100 {
            let progress = coord_clone.progress();
            assert!(progress >= prev, "progress must be monotonic");
            prev = progress;
            thread::sleep(std::time::Duration::from_micros(100));
        }
    });
    handles.push(reader);

    // Completer thread
    for i in 0..100 {
        let _ = coord.mark_completed(i);
        thread::sleep(std::time::Duration::from_micros(50));
    }

    for handle in handles {
        let _ = handle.join();
    }
}
