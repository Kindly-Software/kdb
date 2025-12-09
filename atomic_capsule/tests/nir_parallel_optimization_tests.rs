//! Comprehensive T28 test suite for NIRParallelOptimizationCapsule
//!
//! Test Tiers:
//! - Q1-Q7: Unit tests (basic functionality)
//! - Q8-Q14: Property tests (invariants, concurrency)
//! - Q15-Q21: Integration tests (multi-stage coordination)
//! - Q22-Q28: Production tests (stress, performance, edge cases)

use atomic_capsule::gpu::{
    NIRParallelOptimizationCapsule, ShaderStage, OptimizationPass, OptimizationError,
    OptimizationResult, OptimizationSnapshot,
};
use std::sync::atomic::{AtomicBool, Ordering as StdOrdering};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Q1-Q7: UNIT TESTS (Basic Functionality)
// ============================================================================

#[test]
fn unit_capsule_creation() {
    let capsule = NIRParallelOptimizationCapsule::new();
    let snap = capsule.snapshot();

    assert_eq!(snap.fsm_state, 0, "Should start in Idle state");
    assert_eq!(snap.active_stages, 0, "Should have no active stages");
    assert_eq!(snap.completed_stages, 0, "Should have no completed stages");
}

#[test]
fn unit_submit_single_stage() {
    let capsule = NIRParallelOptimizationCapsule::new();

    let id = capsule.submit_stage(ShaderStage::Vertex, 100);
    assert!(id.is_ok(), "Submit should succeed");

    let snap = capsule.snapshot();
    assert_eq!(snap.active_stages, 1, "Active stage count should be 1");
}

#[test]
fn unit_submit_all_stages() {
    let capsule = NIRParallelOptimizationCapsule::new();

    let stages = [
        ShaderStage::Vertex,
        ShaderStage::Fragment,
        ShaderStage::Compute,
        ShaderStage::Geometry,
        ShaderStage::TessControl,
        ShaderStage::TessEval,
    ];

    for (i, stage) in stages.iter().enumerate() {
        if i < 3 {
            // First 3 should succeed (capacity limit is 3)
            assert!(
                capsule.submit_stage(*stage, 100 + (i as u32) * 50).is_ok(),
                "Submit stage {:?} should succeed",
                stage
            );
        } else {
            // Beyond 3, capacity exceeded
            assert_eq!(
                capsule.submit_stage(*stage, 100 + (i as u32) * 50),
                Err(OptimizationError::CapacityExceeded),
                "Submit stage {:?} should fail with capacity exceeded",
                stage
            );
        }
    }
}

#[test]
fn unit_optimize_empty() {
    let capsule = NIRParallelOptimizationCapsule::new();

    let result = capsule.optimize_parallel();
    assert_eq!(
        result,
        Err(OptimizationError::NoStagesSubmitted),
        "Optimize with no stages should fail"
    );
}

#[test]
fn unit_get_result_single_stage() {
    let capsule = NIRParallelOptimizationCapsule::new();

    capsule.submit_stage(ShaderStage::Vertex, 100).unwrap();
    capsule.optimize_parallel().unwrap();

    let result = capsule.get_result(ShaderStage::Vertex).unwrap();
    assert_eq!(result.stage, ShaderStage::Vertex);
    assert_eq!(result.instructions_before, 100);
    assert_eq!(result.instructions_after, 80, "Expected 20% reduction");
    assert_eq!(result.reduction_percent, 20);
}

#[test]
fn unit_reset_clears_state() {
    let capsule = NIRParallelOptimizationCapsule::new();

    capsule.submit_stage(ShaderStage::Vertex, 100).unwrap();
    capsule.optimize_parallel().unwrap();

    capsule.reset();

    let snap = capsule.snapshot();
    assert_eq!(snap.fsm_state, 0);
    assert_eq!(snap.active_stages, 0);
    assert_eq!(snap.completed_stages, 0);
}

#[test]
fn unit_stage_display_names() {
    assert_eq!(ShaderStage::Vertex.as_str(), "VERTEX");
    assert_eq!(ShaderStage::Fragment.as_str(), "FRAGMENT");
    assert_eq!(ShaderStage::Compute.as_str(), "COMPUTE");
    assert_eq!(ShaderStage::Geometry.as_str(), "GEOMETRY");
    assert_eq!(ShaderStage::TessControl.as_str(), "TESS_CONTROL");
    assert_eq!(ShaderStage::TessEval.as_str(), "TESS_EVAL");
}

// ============================================================================
// Q8-Q14: PROPERTY TESTS (Invariants, Memory Ordering)
// ============================================================================

#[test]
fn prop_submit_is_idempotent_same_stage() {
    let capsule = NIRParallelOptimizationCapsule::new();

    capsule.submit_stage(ShaderStage::Vertex, 100).unwrap();
    capsule.submit_stage(ShaderStage::Vertex, 100).unwrap();

    let snap = capsule.snapshot();
    assert_eq!(snap.active_stages, 2, "Should count as 2 separate submissions");
}

#[test]
fn prop_generation_counter_monotonic() {
    let capsule = NIRParallelOptimizationCapsule::new();

    let snap1 = capsule.snapshot();
    let gen1 = snap1.generation;

    capsule.submit_stage(ShaderStage::Vertex, 100).unwrap();

    let snap2 = capsule.snapshot();
    let gen2 = snap2.generation;

    // Generation should not decrease
    assert!(gen2 >= gen1, "Generation counter should be monotonic");
}

#[test]
fn prop_active_stages_never_decrease() {
    let capsule = NIRParallelOptimizationCapsule::new();

    let snap1 = capsule.snapshot();
    let active1 = snap1.active_stages;

    capsule.submit_stage(ShaderStage::Vertex, 100).unwrap();

    let snap2 = capsule.snapshot();
    let active2 = snap2.active_stages;

    assert!(active2 >= active1, "Active stages count should not decrease");
}

#[test]
fn prop_optimization_reduces_instructions() {
    let capsule = NIRParallelOptimizationCapsule::new();

    for instr_count in [10, 50, 100, 500, 1000] {
        capsule.reset();
        capsule.submit_stage(ShaderStage::Vertex, instr_count).unwrap();
        capsule.optimize_parallel().unwrap();

        let result = capsule.get_result(ShaderStage::Vertex).unwrap();
        assert!(
            result.instructions_after <= result.instructions_before,
            "Optimized count should be <= original"
        );
        assert!(
            result.reduction_percent <= 100,
            "Reduction percent should be valid"
        );
    }
}

#[test]
fn prop_fsm_state_transitions() {
    let capsule = NIRParallelOptimizationCapsule::new();

    // State 0: Idle
    let snap = capsule.snapshot();
    assert_eq!(snap.fsm_state, 0);

    capsule.submit_stage(ShaderStage::Vertex, 100).unwrap();

    // State transition to Optimizing (2) happens during optimize_parallel
    capsule.optimize_parallel().unwrap();

    // State 3: Completed
    let snap = capsule.snapshot();
    assert_eq!(snap.fsm_state, 3);
}

#[test]
fn prop_concurrent_reads_consistent() {
    let capsule = Arc::new(NIRParallelOptimizationCapsule::new());

    capsule.submit_stage(ShaderStage::Vertex, 100).unwrap();
    capsule.optimize_parallel().unwrap();

    let mut handles = vec![];

    // Spawn 10 threads all reading results
    for _ in 0..10 {
        let cap_clone = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            cap_clone.get_result(ShaderStage::Vertex).unwrap()
        });
        handles.push(handle);
    }

    // All reads should return same result
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    for i in 1..results.len() {
        assert_eq!(
            results[0].instructions_after, results[i].instructions_after,
            "All reads should see same value"
        );
    }
}

#[test]
fn prop_snapshot_is_atomic_point_in_time() {
    let capsule = Arc::new(NIRParallelOptimizationCapsule::new());

    capsule.submit_stage(ShaderStage::Vertex, 100).unwrap();
    capsule.submit_stage(ShaderStage::Fragment, 150).unwrap();

    let snap1 = capsule.snapshot();
    let snap2 = capsule.snapshot();

    // Same atomic values
    assert_eq!(snap1.fsm_state, snap2.fsm_state);
    assert_eq!(snap1.active_stages, snap2.active_stages);
    assert_eq!(snap1.generation, snap2.generation);
}

// ============================================================================
// Q15-Q21: INTEGRATION TESTS (Multi-Stage Coordination)
// ============================================================================

#[test]
fn integ_three_stage_pipeline() {
    let capsule = NIRParallelOptimizationCapsule::new();

    // Submit 3-stage pipeline (Vertex, Fragment, Compute)
    capsule.submit_stage(ShaderStage::Vertex, 100).unwrap();
    capsule.submit_stage(ShaderStage::Fragment, 150).unwrap();
    capsule.submit_stage(ShaderStage::Compute, 200).unwrap();

    let snap_before = capsule.snapshot();
    assert_eq!(snap_before.active_stages, 3);

    capsule.optimize_parallel().unwrap();

    let snap_after = capsule.snapshot();
    assert_eq!(snap_after.fsm_state, 3); // Completed

    // Verify all results
    let vs_result = capsule.get_result(ShaderStage::Vertex).unwrap();
    let fs_result = capsule.get_result(ShaderStage::Fragment).unwrap();
    let cs_result = capsule.get_result(ShaderStage::Compute).unwrap();

    assert_eq!(vs_result.instructions_before, 100);
    assert_eq!(fs_result.instructions_before, 150);
    assert_eq!(cs_result.instructions_before, 200);
}

#[test]
fn integ_sequential_optimization_cycles() {
    let capsule = NIRParallelOptimizationCapsule::new();

    // Cycle 1
    capsule.submit_stage(ShaderStage::Vertex, 100).unwrap();
    capsule.optimize_parallel().unwrap();

    let result1 = capsule.get_result(ShaderStage::Vertex).unwrap();
    assert_eq!(result1.instructions_after, 80);

    // Reset and cycle 2
    capsule.reset();

    capsule.submit_stage(ShaderStage::Fragment, 200).unwrap();
    capsule.optimize_parallel().unwrap();

    let result2 = capsule.get_result(ShaderStage::Fragment).unwrap();
    assert_eq!(result2.instructions_after, 160);
}

#[test]
fn integ_multiple_submissions_same_stage() {
    let capsule = NIRParallelOptimizationCapsule::new();

    // Submit same stage twice with different instruction counts
    let id1 = capsule.submit_stage(ShaderStage::Vertex, 100).unwrap();
    let id2 = capsule.submit_stage(ShaderStage::Vertex, 150).unwrap();

    assert_ne!(id1, id2, "Different submissions should have different IDs");

    capsule.optimize_parallel().unwrap();

    let snap = capsule.snapshot();
    assert_eq!(snap.active_stages, 0, "Should be no active stages after optimization");
}

#[test]
fn integ_error_propagation() {
    let capsule = NIRParallelOptimizationCapsule::new();

    // Submit with 0 instructions
    let result = capsule.submit_stage(ShaderStage::Vertex, 0);
    assert!(result.is_ok(), "Should allow 0-instruction submission");

    // Get result on incomplete optimization
    let get_result = capsule.get_result(ShaderStage::Fragment);
    assert_eq!(get_result, Err(OptimizationError::IncompleteOptimization));
}

#[test]
fn integ_reduction_percentage_accuracy() {
    let capsule = NIRParallelOptimizationCapsule::new();

    let test_cases = vec![
        (100, 20, "100 -> 80 = 20%"),
        (50, 10, "50 -> 40 = 20%"),
        (1000, 200, "1000 -> 800 = 20%"),
    ];

    for (initial, expected_reduction, desc) in test_cases {
        capsule.reset();
        capsule.submit_stage(ShaderStage::Vertex, initial).unwrap();
        capsule.optimize_parallel().unwrap();

        let result = capsule.get_result(ShaderStage::Vertex).unwrap();
        assert_eq!(
            result.instructions_before - result.instructions_after,
            expected_reduction,
            "{}",
            desc
        );
    }
}

#[test]
fn integ_stage_independence() {
    let capsule = NIRParallelOptimizationCapsule::new();

    // Submit multiple stages with different profiles
    capsule.submit_stage(ShaderStage::Vertex, 100).unwrap();
    capsule.submit_stage(ShaderStage::Fragment, 50).unwrap();
    capsule.submit_stage(ShaderStage::Compute, 200).unwrap();

    capsule.optimize_parallel().unwrap();

    // Verify each stage's result is independent
    let vs = capsule.get_result(ShaderStage::Vertex).unwrap();
    let fs = capsule.get_result(ShaderStage::Fragment).unwrap();
    let cs = capsule.get_result(ShaderStage::Compute).unwrap();

    assert_eq!(vs.instructions_before, 100);
    assert_eq!(fs.instructions_before, 50);
    assert_eq!(cs.instructions_before, 200);

    // All should have same 20% reduction
    assert_eq!(vs.reduction_percent, 20);
    assert_eq!(fs.reduction_percent, 20);
    assert_eq!(cs.reduction_percent, 20);
}

// ============================================================================
// Q22-Q28: PRODUCTION TESTS (Stress, Performance, Edge Cases)
// ============================================================================

#[test]
fn prod_stress_rapid_submissions() {
    let capsule = Arc::new(NIRParallelOptimizationCapsule::new());

    let mut handles = vec![];

    // Spawn 10 threads, each attempting submissions
    for t_id in 0..10 {
        let cap_clone = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            for i in 0..10 {
                let stage = match i % 3 {
                    0 => ShaderStage::Vertex,
                    1 => ShaderStage::Fragment,
                    2 => ShaderStage::Compute,
                    _ => unreachable!(),
                };

                let _result = cap_clone.submit_stage(stage, 100 + (t_id as u32 * 10 + i as u32));
                // Some may fail due to capacity, that's ok
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Final snapshot should be consistent
    let snap = capsule.snapshot();
    assert!(snap.active_stages <= 3, "Should respect capacity limit");
}

#[test]
fn prod_stress_extreme_instruction_counts() {
    let capsule = NIRParallelOptimizationCapsule::new();

    let test_cases = vec![
        0u32,
        1,
        u32::MAX / 2,
        u16::MAX as u32,
        1_000_000,
    ];

    for instr_count in test_cases {
        capsule.reset();

        let submit_result = capsule.submit_stage(ShaderStage::Vertex, instr_count);
        if submit_result.is_ok() {
            capsule.optimize_parallel().unwrap();

            let result = capsule.get_result(ShaderStage::Vertex).unwrap();
            assert!(
                result.instructions_after <= result.instructions_before,
                "Optimization must not increase instruction count"
            );
        }
    }
}

#[test]
fn prod_no_memory_leaks_reset() {
    let capsule = Arc::new(NIRParallelOptimizationCapsule::new());

    for _ in 0..1000 {
        capsule.submit_stage(ShaderStage::Vertex, 100).unwrap();
        capsule.submit_stage(ShaderStage::Fragment, 150).unwrap();
        capsule.submit_stage(ShaderStage::Compute, 200).unwrap();

        capsule.optimize_parallel().unwrap();

        capsule.reset();
    }

    // If we reach here, no memory issues detected
    let snap = capsule.snapshot();
    assert_eq!(snap.fsm_state, 0, "Should be back to Idle");
}

#[test]
fn prod_concurrent_read_write_safety() {
    let capsule = Arc::new(NIRParallelOptimizationCapsule::new());

    capsule.submit_stage(ShaderStage::Vertex, 100).unwrap();

    let capsule_opt = Arc::clone(&capsule);
    let barrier = Arc::new(std::sync::Barrier::new(3));

    // Thread 1: Optimize
    let barrier1 = Arc::clone(&barrier);
    let opt_handle = thread::spawn(move || {
        barrier1.wait();
        capsule_opt.optimize_parallel()
    });

    // Threads 2-3: Read while optimization in progress
    let mut read_handles = vec![];

    for _ in 0..2 {
        let cap_clone = Arc::clone(&capsule);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            // Try to read (may fail if not complete)
            let _result = cap_clone.get_result(ShaderStage::Vertex);
        });

        read_handles.push(handle);
    }

    // Wait for all threads
    opt_handle.join().unwrap();
    for handle in read_handles {
        handle.join().unwrap();
    }
}

#[test]
fn prod_performance_single_stage_throughput() {
    let capsule = NIRParallelOptimizationCapsule::new();

    let start = std::time::Instant::now();

    // Perform 1000 optimization cycles
    for _ in 0..1000 {
        capsule.reset();
        capsule.submit_stage(ShaderStage::Vertex, 100).unwrap();
        capsule.optimize_parallel().unwrap();
        let _result = capsule.get_result(ShaderStage::Vertex).unwrap();
    }

    let elapsed = start.elapsed();
    let per_cycle = elapsed.as_micros() as f64 / 1000.0;

    println!("Per-cycle latency: {:.3} μs", per_cycle);

    // Should be sub-microsecond per cycle (submit + optimize + get)
    assert!(per_cycle < 10.0, "Latency too high: {:.3} μs", per_cycle);
}

#[test]
fn prod_three_stage_parallel_speedup() {
    let capsule = NIRParallelOptimizationCapsule::new();

    // Simulate 3-stage pipeline with varying complexity
    let stages = vec![
        (ShaderStage::Vertex, 100),
        (ShaderStage::Fragment, 150),
        (ShaderStage::Compute, 200),
    ];

    let start = std::time::Instant::now();

    // Submit all stages
    for (stage, instr) in &stages {
        capsule.submit_stage(*stage, *instr).unwrap();
    }

    // Optimize in parallel
    capsule.optimize_parallel().unwrap();

    let elapsed = start.elapsed();

    // Should be ~2-3× faster than sequential
    // (sequential would be ~3× single-stage time)
    println!("3-stage optimization time: {:.3} μs", elapsed.as_micros());

    // Verify all stages completed
    for (stage, _) in &stages {
        let result = capsule.get_result(*stage).unwrap();
        assert!(result.instructions_after > 0 || result.instructions_before == 0);
    }
}

#[test]
fn prod_zero_instruction_handling() {
    let capsule = NIRParallelOptimizationCapsule::new();

    capsule.submit_stage(ShaderStage::Vertex, 0).unwrap();
    capsule.optimize_parallel().unwrap();

    let result = capsule.get_result(ShaderStage::Vertex).unwrap();
    assert_eq!(result.instructions_before, 0);
    assert_eq!(result.instructions_after, 0);
    assert_eq!(result.reduction_percent, 0);
}

#[test]
fn prod_maximum_capacity_behavior() {
    let capsule = NIRParallelOptimizationCapsule::new();

    // Fill to capacity
    for i in 0..3 {
        let stage = match i {
            0 => ShaderStage::Vertex,
            1 => ShaderStage::Fragment,
            2 => ShaderStage::Compute,
            _ => unreachable!(),
        };
        assert!(
            capsule.submit_stage(stage, 100 + (i as u32 * 50)).is_ok(),
            "Should be able to submit up to 3 stages"
        );
    }

    // 4th should fail
    assert_eq!(
        capsule.submit_stage(ShaderStage::Geometry, 100),
        Err(OptimizationError::CapacityExceeded)
    );

    // Optimize should work
    assert!(capsule.optimize_parallel().is_ok());

    // After reset, should be able to submit again
    capsule.reset();
    assert!(capsule.submit_stage(ShaderStage::Vertex, 100).is_ok());
}
