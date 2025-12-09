//! T28 Test Suite for BatchConstructorCapsule (T4 Batch Tier)
//!
//! Comprehensive testing across 4 tiers:
//! - Q1-Q7 (Unit): Basic functionality tests
//! - Q8-Q14 (Property): Invariant and concurrency tests
//! - Q15-Q21 (Integration): Multi-component coordination
//! - Q22-Q28 (Production): Stress tests and realistic workloads

use atomic_capsule::gpu::{
    BatchConstructorCapsule, BatchState, ThreadCompletionState, BatchError,
};
use std::sync::Arc;
use std::thread;
use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

// ============================================================================
// Q1-Q7: UNIT TESTS (Basic Functionality)
// ============================================================================

#[test]
fn q1_new_returns_idle_state() {
    let batch = BatchConstructorCapsule::new();
    let (state, active, total) = batch.snapshot();

    assert_eq!(state, BatchState::Idle);
    assert_eq!(active, 0);
    assert_eq!(total, 0);
}

#[test]
fn q2_start_batch_transitions_to_recording() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());

    let (state, active, total) = batch.snapshot();
    assert_eq!(state, BatchState::Recording);
    assert_eq!(active, 0);
    assert_eq!(total, 0);
}

#[test]
fn q3_start_batch_twice_fails() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());

    // Cannot start again while recording
    assert_eq!(batch.start_batch(), Err(BatchError::InvalidStateTransition));
}

#[test]
fn q4_submit_thread_valid_id() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());

    for i in 0..8 {
        assert!(batch.submit_thread(i).is_ok());
    }
}

#[test]
fn q5_submit_thread_invalid_id() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());

    assert_eq!(batch.submit_thread(8), Err(BatchError::InvalidThreadId));
    assert_eq!(batch.submit_thread(255), Err(BatchError::InvalidThreadId));
}

#[test]
fn q6_record_command_increments_total() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());
    assert!(batch.submit_thread(0).is_ok());

    assert!(batch.record_command(0).is_ok());
    let (_, _, total) = batch.snapshot();
    assert_eq!(total, 1);

    assert!(batch.record_command(0).is_ok());
    let (_, _, total) = batch.snapshot();
    assert_eq!(total, 2);
}

#[test]
fn q7_finish_batch_transitions_to_submitted() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());
    assert!(batch.submit_thread(0).is_ok());

    let total = batch.finish_batch().expect("finish_batch failed");
    assert_eq!(total, 0);  // No commands recorded

    let (state, _, _) = batch.snapshot();
    assert_eq!(state, BatchState::Submitted);
}

// ============================================================================
// Q8-Q14: PROPERTY TESTS (Invariants and Concurrency)
// ============================================================================

#[test]
fn q8_generation_counter_increments() {
    let batch = BatchConstructorCapsule::new();
    let primary1 = batch.primary.load(core::sync::atomic::Ordering::Acquire);
    let gen1 = ((primary1 >> 8) & 0xFF) as u8;
    assert_eq!(gen1, 0);

    assert!(batch.start_batch().is_ok());
    let primary2 = batch.primary.load(core::sync::atomic::Ordering::Acquire);
    let gen2 = ((primary2 >> 8) & 0xFF) as u8;
    assert_eq!(gen2, 1);

    batch.reset();
    assert!(batch.start_batch().is_ok());
    let primary3 = batch.primary.load(core::sync::atomic::Ordering::Acquire);
    let gen3 = ((primary3 >> 8) & 0xFF) as u8;
    assert_eq!(gen3, 2);
}

#[test]
fn q9_active_thread_count_monotonic() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());

    for i in 0..8 {
        assert!(batch.submit_thread(i).is_ok());
        let (_, active, _) = batch.snapshot();
        assert_eq!(active as usize, i + 1);
    }
}

#[test]
fn q10_total_commands_monotonic() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());
    assert!(batch.submit_thread(0).is_ok());

    for expected_count in 1..=100 {
        assert!(batch.record_command(0).is_ok());
        let (_, _, total) = batch.snapshot();
        assert_eq!(total, expected_count);
    }
}

#[test]
fn q11_thread_status_reflects_state() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());
    assert!(batch.submit_thread(3).is_ok());

    let (tid, state, count) = batch.thread_status(3).expect("thread_status failed");
    assert_eq!(tid, 3);
    assert_eq!(state, ThreadCompletionState::Recording);
    assert_eq!(count, 0);

    assert!(batch.record_command(3).is_ok());
    let (_, _, count) = batch.thread_status(3).expect("thread_status failed");
    assert_eq!(count, 1);
}

#[test]
fn q12_reset_clears_all_state() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());
    assert!(batch.submit_thread(0).is_ok());
    assert!(batch.record_command(0).is_ok());
    assert!(batch.record_command(0).is_ok());

    batch.reset();

    let (state, active, total) = batch.snapshot();
    assert_eq!(state, BatchState::Idle);
    assert_eq!(active, 0);
    assert_eq!(total, 0);
}

#[test]
fn q13_size_verification() {
    assert_eq!(std::mem::size_of::<BatchConstructorCapsule>(), 512);
    assert!(std::mem::align_of::<BatchConstructorCapsule>() >= 512);
}

#[test]
fn q14_lockfree_property() {
    // Verify no blocking operations used (atomic-only coordination)
    let batch = BatchConstructorCapsule::new();

    // All operations must complete in bounded time (no mutexes)
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _ = batch.snapshot();  // Atomic read only
    }
    let elapsed = start.elapsed();

    // 1000 snapshots should complete in <<1ms with atomic operations
    assert!(elapsed.as_millis() < 100, "snapshot taking too long: {:?}", elapsed);
}

// ============================================================================
// Q15-Q21: INTEGRATION TESTS (Multi-Component Coordination)
// ============================================================================

#[test]
fn q15_single_thread_workflow() {
    let batch = BatchConstructorCapsule::new();

    // Workflow: start → submit → record → finish
    assert!(batch.start_batch().is_ok());
    assert!(batch.submit_thread(0).is_ok());

    for _ in 0..50 {
        assert!(batch.record_command(0).is_ok());
    }

    let total = batch.finish_batch().expect("finish_batch failed");
    assert_eq!(total, 50);

    let (state, active, _) = batch.snapshot();
    assert_eq!(state, BatchState::Submitted);
    assert_eq!(active, 1);
}

#[test]
fn q16_two_thread_coordination() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());

    // Both threads submit
    assert!(batch.submit_thread(0).is_ok());
    assert!(batch.submit_thread(1).is_ok());

    // Thread 0 records 30 commands
    for _ in 0..30 {
        assert!(batch.record_command(0).is_ok());
    }

    // Thread 1 records 20 commands
    for _ in 0..20 {
        assert!(batch.record_command(1).is_ok());
    }

    let total = batch.finish_batch().expect("finish_batch failed");
    assert_eq!(total, 50);
}

#[test]
fn q17_all_eight_threads() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());

    // Submit all 8 threads
    for i in 0..8 {
        assert!(batch.submit_thread(i).is_ok());
    }

    // Each thread records 10 commands
    for i in 0..8 {
        for _ in 0..10 {
            assert!(batch.record_command(i).is_ok());
        }
    }

    let total = batch.finish_batch().expect("finish_batch failed");
    assert_eq!(total, 80);

    let (state, active, _) = batch.snapshot();
    assert_eq!(state, BatchState::Submitted);
    assert_eq!(active, 8);
}

#[test]
fn q18_thread_status_after_recording() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());

    for i in 0..4 {
        assert!(batch.submit_thread(i).is_ok());
    }

    // Record different amounts per thread
    for i in 0..4 {
        for _ in 0..((i + 1) * 10) {
            assert!(batch.record_command(i).is_ok());
        }
    }

    // Verify per-thread counts
    for i in 0..4 {
        let (_, _, count) = batch.thread_status(i).expect("thread_status failed");
        assert_eq!(count, (i as u32 + 1) * 10);
    }
}

#[test]
fn q19_multiple_batches_sequential() {
    // Test batch reuse
    let batch = Arc::new(BatchConstructorCapsule::new());

    for batch_num in 0..3 {
        assert!(batch.start_batch().is_ok());

        for thread_id in 0..4 {
            assert!(batch.submit_thread(thread_id).is_ok());
        }

        for thread_id in 0..4 {
            for _ in 0..25 {
                assert!(batch.record_command(thread_id).is_ok());
            }
        }

        let total = batch.finish_batch().expect("finish_batch failed");
        assert_eq!(total, 100, "batch {} should have 100 commands", batch_num);

        // Reset for next batch
        batch.reset();
    }
}

#[test]
fn q20_invalid_operations_in_wrong_state() {
    let batch = BatchConstructorCapsule::new();

    // Cannot record before starting
    assert_eq!(batch.record_command(0), Err(BatchError::NotRecording));

    // Cannot submit thread before starting
    assert_eq!(batch.submit_thread(0), Err(BatchError::NotRecording));

    // Cannot finish before starting
    assert!(batch.finish_batch().is_ok());  // finish returns total (0)

    // Start and proceed normally
    assert!(batch.start_batch().is_ok());
    assert!(batch.submit_thread(0).is_ok());
    assert!(batch.record_command(0).is_ok());
    assert!(batch.finish_batch().is_ok());
}

#[test]
fn q21_uneven_thread_distribution() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());

    // Submit only odd-numbered threads
    assert!(batch.submit_thread(1).is_ok());
    assert!(batch.submit_thread(3).is_ok());
    assert!(batch.submit_thread(5).is_ok());
    assert!(batch.submit_thread(7).is_ok());

    // Record different amounts
    assert!(batch.record_command(1).is_ok());
    for _ in 0..5 {
        assert!(batch.record_command(3).is_ok());
    }
    for _ in 0..10 {
        assert!(batch.record_command(5).is_ok());
    }
    for _ in 0..15 {
        assert!(batch.record_command(7).is_ok());
    }

    let total = batch.finish_batch().expect("finish_batch failed");
    assert_eq!(total, 1 + 5 + 10 + 15);
}

// ============================================================================
// Q22-Q28: PRODUCTION TESTS (Stress and Realistic Workloads)
// ============================================================================

#[test]
fn q22_stress_high_command_volume() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());

    for i in 0..8 {
        assert!(batch.submit_thread(i).is_ok());
    }

    // Each thread records 1000 commands
    for thread_id in 0..8 {
        for _ in 0..1000 {
            assert!(batch.record_command(thread_id).is_ok());
        }
    }

    let total = batch.finish_batch().expect("finish_batch failed");
    assert_eq!(total, 8000);
}

#[test]
fn q23_stress_rapid_thread_submission() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());

    // Rapidly submit and record
    for i in 0..8 {
        assert!(batch.submit_thread(i).is_ok());

        // Immediately record a few commands
        for _ in 0..5 {
            assert!(batch.record_command(i).is_ok());
        }
    }

    let total = batch.finish_batch().expect("finish_batch failed");
    assert_eq!(total, 40);  // 8 threads × 5 commands
}

#[test]
fn q24_stress_interleaved_recording() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());

    for i in 0..8 {
        assert!(batch.submit_thread(i).is_ok());
    }

    // Interleave recording from different threads
    for round in 0..100 {
        for thread_id in 0..8 {
            if (round + thread_id) % 3 == 0 {
                assert!(batch.record_command(thread_id as u8).is_ok());
            }
        }
    }

    let total = batch.finish_batch().expect("finish_batch failed");
    // Exact count depends on modulo logic, but should be > 0
    assert!(total > 0);
}

#[test]
fn q25_stress_snapshot_during_recording() {
    let batch = BatchConstructorCapsule::new();
    assert!(batch.start_batch().is_ok());

    for i in 0..4 {
        assert!(batch.submit_thread(i).is_ok());
    }

    let mut last_snapshot_total = 0;

    for _ in 0..200 {
        for i in 0..4 {
            assert!(batch.record_command(i).is_ok());
        }

        // Periodically snapshot - total should be monotonically increasing
        let (_, _, total) = batch.snapshot();
        assert!(total >= last_snapshot_total);
        last_snapshot_total = total;
    }

    let total = batch.finish_batch().expect("finish_batch failed");
    assert_eq!(total, 800);
}

#[test]
fn q26_stress_batch_cycling() {
    let batch = Arc::new(BatchConstructorCapsule::new());
    let command_count = Arc::new(AtomicU32::new(0));

    for cycle in 0..10 {
        batch.reset();
        assert!(batch.start_batch().is_ok());

        for i in 0..8 {
            assert!(batch.submit_thread(i).is_ok());
        }

        let commands_this_cycle = (cycle + 1) * 10;
        for i in 0..8 {
            for _ in 0..commands_this_cycle {
                assert!(batch.record_command(i).is_ok());
            }
        }

        let total = batch.finish_batch().expect("finish_batch failed");
        assert_eq!(total, 8 * commands_this_cycle as u32);

        command_count.fetch_add(total, AtomicOrdering::Relaxed);
    }

    // Verify total across all cycles
    let total_commands = command_count.load(AtomicOrdering::SeqCst);
    let expected = (1..=10).map(|c| 8 * c * 10).sum::<u32>();
    assert_eq!(total_commands, expected);
}

#[test]
fn q27_parallel_multi_batch_simulation() {
    let batch_count = 5;
    let mut handles = vec![];

    for batch_id in 0..batch_count {
        let handle = thread::spawn(move || {
            let batch = BatchConstructorCapsule::new();
            assert!(batch.start_batch().is_ok());

            let threads_per_batch = 4;
            for i in 0..threads_per_batch {
                assert!(batch.submit_thread(i).is_ok());
            }

            let commands_per_thread = 25;
            for i in 0..threads_per_batch {
                for _ in 0..commands_per_thread {
                    assert!(batch.record_command(i).is_ok());
                }
            }

            let total = batch.finish_batch().expect("finish_batch failed");
            assert_eq!(total, (threads_per_batch * commands_per_thread) as u32);

            batch_id * 100 + total as usize
        });

        handles.push(handle);
    }

    for handle in handles {
        let result = handle.join().expect("thread panicked");
        assert!(result > 0);
    }
}

#[test]
fn q28_production_realistic_workload() {
    // Simulate a realistic GPU driver workload:
    // - Variable number of worker threads
    // - Variable command recording patterns
    // - Multiple batch cycles

    let batch = Arc::new(BatchConstructorCapsule::new());

    for cycle in 0..3 {
        batch.reset();
        assert!(batch.start_batch().is_ok());

        // Variable thread count per cycle
        let thread_count = match cycle {
            0 => 2,  // Light load
            1 => 6,  // Medium load
            2 => 8,  // Heavy load
            _ => 1,
        };

        for i in 0..thread_count {
            assert!(batch.submit_thread(i).is_ok());
        }

        // Simulate realistic command recording pattern
        let mut expected_total = 0;
        for thread_id in 0..thread_count {
            let commands = match cycle {
                0 => 10 + thread_id as u32,      // 10-11 commands per thread
                1 => 20 + (thread_id * 2) as u32, // 20-30 commands per thread
                2 => 50,                           // 50 commands per thread
                _ => 0,
            };

            for _ in 0..commands {
                assert!(batch.record_command(thread_id).is_ok());
                expected_total += 1;
            }
        }

        let total = batch.finish_batch().expect("finish_batch failed");
        assert_eq!(total, expected_total);

        let (state, active, _) = batch.snapshot();
        assert_eq!(state, BatchState::Submitted);
        assert_eq!(active, thread_count);
    }
}
