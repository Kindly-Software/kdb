//! Property Tests for Queue and Batch Coordinators
//!
//! Validates correctness properties following ASSUM framework and UCE32 methodology.
//!
//! # Test Categories (UCE30 Empirical Validation)
//!
//! 1. **Load Balancing Properties**: Queue selection distributes load fairly
//! 2. **Atomic Safety Properties**: No lost updates, no torn reads
//! 3. **Batching Properties**: Threshold and deadline logic correctness
//! 4. **Concurrent Stress**: Multi-threaded correctness under contention

use proptest::prelude::*;
use std::sync::Arc;
use std::thread;

// Import modules under test
use kiang::batch_coordinator::{BatchCoordinator, BatchHintCapsule, BatchState};
use kiang::command::{Command, CommandType};
use kiang::queue_coordinator::{QueueCoordinatorCapsule, QueueId, QueueState};

// ============================================================================
// Property Tests - Queue Coordinator
// ============================================================================

proptest! {
    /// Property: Queue selection should balance load across available queues
    ///
    /// #VERIFY_INVARIANT: Total load distributed fairly across queues
    #[test]
    fn prop_queue_selection_balances_load(
        render_load in 0u16..10000,
        compute_load in 0u16..10000,
        copy_load in 0u16..10000,
        video_load in 0u16..10000,
    ) {
        let qcc = QueueCoordinatorCapsule::new();

        // Publish state with varying loads
        let state = QueueState {
            active_queues: 0b1111,
            render_load,
            compute_load,
            copy_load,
            video_load,
            render_priority: 128,
            compute_priority: 128,
            copy_priority: 128,
            video_priority: 128,
            hints: 0,
        };
        qcc.publish(state);

        // For render commands, should select least loaded of render/compute
        let queue = qcc.select_queue(CommandType::Render, 128);
        let expected = if render_load <= compute_load {
            QueueId::Render0
        } else {
            QueueId::Render1
        };
        prop_assert_eq!(queue, expected, "Render queue selection should prefer least loaded");

        // For copy commands, should select least loaded of copy/video
        let queue = qcc.select_queue(CommandType::Copy, 128);
        let is_copy_selected = matches!(queue, QueueId::Copy | QueueId::CopyDma);
        prop_assert!(is_copy_selected, "Copy queue should be selected");
    }

    /// Property: Load updates are atomic and cumulative
    ///
    /// #VERIFY_COUNTER_ACCURACY: Sum of deltas equals final load
    #[test]
    fn prop_load_updates_atomic(
        deltas in prop::collection::vec(-100i16..100i16, 1..50),
    ) {
        let qcc = QueueCoordinatorCapsule::new();

        // Publish initial state
        qcc.publish(QueueState::new_all_active());

        // Apply all deltas
        let mut expected_load = 0i32;
        for delta in &deltas {
            qcc.update_load(QueueId::Render0, *delta);
            expected_load += *delta as i32;
            expected_load = expected_load.clamp(0, 65535);
        }

        // Read final state
        let state = qcc.read().unwrap();
        prop_assert_eq!(
            state.render_load as i32,
            expected_load,
            "Load should equal sum of deltas (clamped)"
        );
    }

    /// Property: Version consistency prevents torn reads
    ///
    /// #VERIFY_INVARIANT: Head version always matches tail version for committed state
    #[test]
    fn prop_version_consistency(
        render_load in 0u16..65535,
        compute_load in 0u16..65535,
    ) {
        let qcc = QueueCoordinatorCapsule::new();

        // Publish state
        let state = QueueState {
            active_queues: 0b1111,
            render_load,
            compute_load,
            copy_load: 0,
            video_load: 0,
            render_priority: 128,
            compute_priority: 128,
            copy_priority: 128,
            video_priority: 128,
            hints: 0,
        };
        qcc.publish(state);

        // Read should always succeed with consistent state
        let read_state = qcc.read();
        prop_assert!(read_state.is_some(), "Published state should be readable");

        if let Some(s) = read_state {
            prop_assert_eq!(s.render_load, render_load, "Render load should match");
            prop_assert_eq!(s.compute_load, compute_load, "Compute load should match");
        }
    }
}

// ============================================================================
// Property Tests - Batch Coordinator
// ============================================================================

proptest! {
    /// Property: Batching respects threshold constraint
    ///
    /// #VERIFY_INVARIANT: Should batch when pending < threshold
    #[test]
    fn prop_batch_threshold_respected(
        threshold in 1u16..1000,
        pending in 0u16..2000,
    ) {
        let bhc = BatchHintCapsule::with_thresholds(threshold, 10000);

        // Set pending count
        for _ in 0..pending {
            bhc.increment_pending_render();
        }

        let cmd = Command {
            cmd_type: CommandType::Render,
            buffer_id: 1,
            size: 1024,
            priority: 128,
        };

        let should_batch = bhc.should_batch(&cmd, 0);

        if pending < threshold {
            prop_assert!(should_batch, "Should batch when below threshold");
        } else {
            prop_assert!(!should_batch, "Should NOT batch when at/above threshold");
        }
    }

    /// Property: Batching respects deadline constraint
    ///
    /// #VERIFY_INVARIANT: Should not batch when age >= deadline
    #[test]
    fn prop_batch_deadline_respected(
        deadline_us in 100u16..5000,
        age_us in 0u32..10000,
    ) {
        let bhc = BatchHintCapsule::with_thresholds(10000, deadline_us); // High threshold

        bhc.record_submission_time(1000); // Oldest at t=1000

        let cmd = Command {
            cmd_type: CommandType::Render,
            buffer_id: 1,
            size: 1024,
            priority: 128,
        };

        let current_time = 1000 + age_us;
        let should_batch = bhc.should_batch(&cmd, current_time);

        if age_us < deadline_us as u32 {
            prop_assert!(should_batch, "Should batch when age < deadline");
        } else {
            prop_assert!(!should_batch, "Should NOT batch when age >= deadline");
        }
    }

    /// Property: Pending counters are accurate
    ///
    /// #VERIFY_COUNTER_ACCURACY: Increments minus decrements equals final count
    #[test]
    fn prop_pending_counter_accuracy(
        increments in 0usize..100,
        decrements in 0usize..100,
    ) {
        let bhc = BatchHintCapsule::with_thresholds(1000, 5000);

        // Apply increments
        for _ in 0..increments {
            bhc.increment_pending_render();
        }

        // Apply decrements (capped at increments)
        let actual_decrements = decrements.min(increments);
        for _ in 0..actual_decrements {
            bhc.decrement_pending_render();
        }

        let state = bhc.read().unwrap();
        let expected = (increments - actual_decrements) as u16;
        prop_assert_eq!(
            state.pending_render,
            expected,
            "Pending count should equal increments - decrements"
        );
    }
}

// ============================================================================
// Concurrent Stress Tests
// ============================================================================

#[test]
fn test_concurrent_queue_coordinator_updates() {
    // #VERIFY_TOCTOU_PREVENTED: Concurrent updates don't cause races
    let qcc = Arc::new(QueueCoordinatorCapsule::new());
    qcc.publish(QueueState::new_all_active());

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let qcc_clone = Arc::clone(&qcc);
            thread::spawn(move || {
                for _ in 0..1000 {
                    // Each thread increments load 1000 times
                    let delta = if i % 2 == 0 { 1 } else { -1 };
                    qcc_clone.update_load(QueueId::Render0, delta);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // With 2 threads adding and 2 subtracting, final load should be ~0
    // (exact value depends on timing, but should be stable)
    let state = qcc.read().unwrap();
    assert!(
        state.render_load < 100,
        "Concurrent updates should balance out (got {})",
        state.render_load
    );
}

#[test]
fn test_concurrent_batch_coordinator_updates() {
    // #VERIFY_METRIC_ATOMIC: Concurrent counter updates are accurate
    let bhc = Arc::new(BatchHintCapsule::with_thresholds(10000, 5000));

    let handles: Vec<_> = (0..8)
        .map(|i| {
            let bhc_clone = Arc::clone(&bhc);
            thread::spawn(move || {
                for _ in 0..500 {
                    if i % 2 == 0 {
                        bhc_clone.increment_pending_render();
                    } else {
                        bhc_clone.increment_pending_compute();
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let state = bhc.read().unwrap();
    // 4 threads * 500 increments each
    assert_eq!(state.pending_render, 2000, "Render count should be 2000");
    assert_eq!(state.pending_compute, 2000, "Compute count should be 2000");
}

#[test]
fn test_concurrent_read_write() {
    // #VERIFY_STATE_MACHINE: Readers never see invalid intermediate state
    let qcc = Arc::new(QueueCoordinatorCapsule::new());
    qcc.publish(QueueState::new_all_active());

    let writer = {
        let qcc_clone = Arc::clone(&qcc);
        thread::spawn(move || {
            for i in 0..1000 {
                let state = QueueState {
                    active_queues: 0b1111,
                    render_load: i,
                    compute_load: i * 2,
                    copy_load: i * 3,
                    video_load: i * 4,
                    render_priority: 128,
                    compute_priority: 128,
                    copy_priority: 128,
                    video_priority: 128,
                    hints: i as u32,
                };
                qcc_clone.publish(state);
            }
        })
    };

    let readers: Vec<_> = (0..4)
        .map(|_| {
            let qcc_clone = Arc::clone(&qcc);
            thread::spawn(move || {
                let mut valid_reads = 0;
                for _ in 0..1000 {
                    if let Some(state) = qcc_clone.read() {
                        // Verify relationships are consistent
                        assert_eq!(
                            state.compute_load,
                            state.render_load * 2,
                            "Compute load should be 2x render load"
                        );
                        assert_eq!(
                            state.copy_load,
                            state.render_load * 3,
                            "Copy load should be 3x render load"
                        );
                        valid_reads += 1;
                    }
                }
                valid_reads
            })
        })
        .collect();

    writer.join().unwrap();

    let mut total_valid = 0;
    for handle in readers {
        total_valid += handle.join().unwrap();
    }

    // Should have many valid reads (exact count depends on timing)
    assert!(total_valid > 100, "Should have many valid reads");
}

// ============================================================================
// Load Balancing Fairness Tests
// ============================================================================

#[test]
fn test_load_balancing_fairness() {
    // #VERIFY_INVARIANT: Queue selection distributes load fairly over time
    let qcc = QueueCoordinatorCapsule::new();
    qcc.publish(QueueState::new_all_active());

    let mut render0_count = 0;
    let mut render1_count = 0;

    // Simulate 1000 render command submissions
    for i in 0..1000 {
        let queue = qcc.select_queue(CommandType::Render, 128);

        // Update load based on selection
        match queue {
            QueueId::Render0 => {
                render0_count += 1;
                qcc.update_load(QueueId::Render0, 1);
            }
            QueueId::Render1 => {
                render1_count += 1;
                qcc.update_load(QueueId::Render1, 1);
            }
            _ => panic!("Unexpected queue for render command"),
        }

        // Simulate command completion (reduce load)
        if i % 10 == 0 {
            qcc.update_load(QueueId::Render0, -1);
            qcc.update_load(QueueId::Render1, -1);
        }
    }

    // Load should be relatively balanced (within 20% of each other)
    let total = render0_count + render1_count;
    let render0_pct = (render0_count as f64 / total as f64) * 100.0;
    let render1_pct = (render1_count as f64 / total as f64) * 100.0;

    assert!(
        (render0_pct - 50.0).abs() < 20.0,
        "Render0 should get ~50% of load (got {:.1}%)",
        render0_pct
    );
    assert!(
        (render1_pct - 50.0).abs() < 20.0,
        "Render1 should get ~50% of load (got {:.1}%)",
        render1_pct
    );
}

#[test]
fn test_batch_coordinator_integration() {
    // Integration test: BatchCoordinator with realistic usage
    let coordinator = BatchCoordinator::with_thresholds(16, 1000);

    let cmd = Command {
        cmd_type: CommandType::Render,
        buffer_id: 1,
        size: 1024,
        priority: 128,
    };

    // Initially should batch (no pending commands)
    assert!(coordinator.should_batch(&cmd));

    // Add commands until threshold
    for i in 0..20 {
        coordinator.increment_pending_render();

        let should_batch = coordinator.should_batch(&cmd);
        if i < 16 {
            assert!(should_batch, "Should batch below threshold (i={})", i);
        } else {
            assert!(
                !should_batch,
                "Should NOT batch at/above threshold (i={})",
                i
            );
        }
    }

    // Verify state
    let state = coordinator.read_state().unwrap();
    assert_eq!(state.pending_render, 20);
    assert_eq!(state.batch_threshold, 16);
}
