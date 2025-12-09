//! Integration tests for GpuFrameSyncCapsule
//!
//! T28 5-tier test suite:
//! - Q1-Q7: Unit tests (8 tests)
//! - Q8-Q14: Property tests (4 tests)
//! - Q15-Q21: Integration tests (4 tests)
//! - Q29-Q35: Determinism tests (2 tests)

use atomic_capsule::terminal::render::{FrameSyncStats, GpuFrameSyncCapsule};

// ============================================================================
// Q1-Q7: Unit Tests (8 tests)
// ============================================================================

#[test]
fn test_new_initializes_correctly() {
    let sync = GpuFrameSyncCapsule::new(60, true);
    let stats = sync.stats();

    assert_eq!(stats.current_frame, 0);
    assert_eq!(stats.current_fence, 0);
    assert_eq!(stats.frames_submitted, 0);
    assert_eq!(stats.frames_completed, 0);
    assert_eq!(stats.frames_dropped, 0);
}

#[test]
fn test_begin_frame_increments() {
    let sync = GpuFrameSyncCapsule::new(60, false);

    let frame1 = sync.begin_frame();
    assert_eq!(frame1, 1);

    let frame2 = sync.begin_frame();
    assert_eq!(frame2, 2);

    let frame3 = sync.begin_frame();
    assert_eq!(frame3, 3);
}

#[test]
fn test_submit_frame_marks_submitted() {
    let sync = GpuFrameSyncCapsule::new(60, false);

    sync.begin_frame();
    assert!(!sync.is_submitted());

    sync.submit_frame(100);
    assert!(sync.is_submitted());
    assert_eq!(sync.current_fence(), 100);

    let stats = sync.stats();
    assert_eq!(stats.frames_submitted, 1);
}

#[test]
fn test_poll_completion_marks_completed() {
    let sync = GpuFrameSyncCapsule::new(60, false);

    sync.begin_frame();
    sync.submit_frame(100);

    assert!(!sync.is_completed());
    assert!(!sync.poll_completion(99)); // Not completed yet

    assert!(sync.poll_completion(100)); // Completed
    assert!(sync.is_completed());

    let stats = sync.stats();
    assert_eq!(stats.frames_completed, 1);
}

#[test]
fn test_signal_vsync_detects_dropped_frames() {
    let sync = GpuFrameSyncCapsule::new(60, true);

    sync.begin_frame();
    sync.submit_frame(100);

    // Don't complete frame before vsync
    sync.signal_vsync();

    let stats = sync.stats();
    assert_eq!(stats.frames_dropped, 1);
}

#[test]
fn test_signal_vsync_no_drop_when_completed() {
    let sync = GpuFrameSyncCapsule::new(60, true);

    sync.begin_frame();
    sync.submit_frame(100);
    sync.poll_completion(100); // Complete before vsync

    sync.signal_vsync();

    let stats = sync.stats();
    assert_eq!(stats.frames_dropped, 0);
}

#[test]
fn test_should_drop_frame_respects_vsync() {
    let sync = GpuFrameSyncCapsule::new(60, false);
    sync.begin_frame();

    // Vsync disabled, never drop
    assert!(!sync.should_drop_frame());
}

#[test]
fn test_stats_snapshot_consistency() {
    let sync = GpuFrameSyncCapsule::new(120, true);

    sync.begin_frame();
    sync.submit_frame(1);
    sync.poll_completion(1);

    sync.begin_frame();
    sync.submit_frame(2);

    let stats = sync.stats();
    assert_eq!(stats.current_frame, 2);
    assert_eq!(stats.current_fence, 2);
    assert_eq!(stats.frames_submitted, 2);
    assert_eq!(stats.frames_completed, 1);
}

// ============================================================================
// Q8-Q14: Property Tests (4 tests)
// ============================================================================

#[test]
fn test_frame_numbers_monotonic() {
    let sync = GpuFrameSyncCapsule::new(60, false);

    let mut last_frame = 0u64;
    for _ in 0..1000 {
        let frame = sync.begin_frame();
        assert!(frame > last_frame, "Frame numbers must be monotonic");
        last_frame = frame;
    }
}

#[test]
fn test_fence_values_never_decrease() {
    let sync = GpuFrameSyncCapsule::new(60, false);

    let mut last_fence = 0u64;
    for i in 1..=100 {
        sync.begin_frame();
        sync.submit_frame(i * 10);

        let fence = sync.current_fence();
        assert!(fence >= last_fence, "Fence values must never decrease");
        last_fence = fence;
    }
}

#[test]
fn test_completed_never_exceeds_submitted() {
    let sync = GpuFrameSyncCapsule::new(60, false);

    for i in 1..=50 {
        sync.begin_frame();
        sync.submit_frame(i * 10);

        if i % 2 == 0 {
            sync.poll_completion(i * 10);
        }

        let stats = sync.stats();
        assert!(
            stats.frames_completed <= stats.frames_submitted,
            "Completed frames must never exceed submitted"
        );
    }
}

#[test]
fn test_state_transitions_valid() {
    let sync = GpuFrameSyncCapsule::new(60, false);

    // Start: not submitted, not completed
    assert!(!sync.is_submitted());
    assert!(!sync.is_completed());

    sync.begin_frame();
    // After begin: still not submitted
    assert!(!sync.is_submitted());

    sync.submit_frame(100);
    // After submit: submitted, not completed
    assert!(sync.is_submitted());
    assert!(!sync.is_completed());

    sync.poll_completion(100);
    // After completion: both submitted and completed
    assert!(sync.is_submitted());
    assert!(sync.is_completed());

    sync.begin_frame();
    // After new frame: flags cleared
    assert!(!sync.is_submitted());
    assert!(!sync.is_completed());
}

// ============================================================================
// Q15-Q21: Integration Tests (4 tests)
// ============================================================================

#[test]
fn test_multi_frame_pipeline() {
    let sync = GpuFrameSyncCapsule::new(60, false);

    // Simulate 3 frames in flight
    let _frame1 = sync.begin_frame();
    sync.submit_frame(100);

    let _frame2 = sync.begin_frame();
    sync.submit_frame(200);

    let _frame3 = sync.begin_frame();
    sync.submit_frame(300);

    // Complete out of order
    assert!(sync.poll_completion(300)); // Frame 3 completes first
    assert!(sync.poll_completion(100)); // Frame 1 completes
    assert!(sync.poll_completion(200)); // Frame 2 completes

    let stats = sync.stats();
    assert_eq!(stats.current_frame, 3);
    assert_eq!(stats.frames_submitted, 3);
    assert_eq!(stats.frames_completed, 3);
}

#[test]
fn test_vsync_timing_simulation() {
    let sync = GpuFrameSyncCapsule::new(60, true);

    // Frame 1: completes before vsync
    sync.begin_frame();
    sync.submit_frame(100);
    sync.poll_completion(100);
    sync.signal_vsync();

    // Frame 2: misses vsync
    sync.begin_frame();
    sync.submit_frame(200);
    sync.signal_vsync(); // Vsync before completion

    let stats = sync.stats();
    assert_eq!(stats.frames_dropped, 1);
    assert_eq!(stats.frames_completed, 1);
}

#[test]
fn test_wait_completion_succeeds() {
    let sync = GpuFrameSyncCapsule::new(60, false);

    sync.begin_frame();
    sync.submit_frame(100);

    // Simulate immediate completion
    let result = sync.wait_completion(100, 1000);
    assert!(result.is_ok());
    assert!(sync.is_completed());
}

#[test]
fn test_concurrent_frame_stats() {
    let sync = GpuFrameSyncCapsule::new(144, false);

    // Rapidly cycle frames
    for i in 1..=100 {
        sync.begin_frame();
        sync.submit_frame(i * 10);

        if i % 3 == 0 {
            sync.poll_completion(i * 10);
        }
    }

    let stats = sync.stats();
    assert_eq!(stats.current_frame, 100);
    assert_eq!(stats.frames_submitted, 100);
    assert!(stats.frames_completed >= 30); // At least 1/3 completed
}

// ============================================================================
// Q29-Q35: Determinism Tests (2 tests)
// ============================================================================

#[test]
fn test_timing_reproducibility() {
    // Same sequence should produce same stats
    let run = |vsync: bool| -> FrameSyncStats {
        let sync = GpuFrameSyncCapsule::new(60, vsync);

        for i in 1..=50 {
            sync.begin_frame();
            sync.submit_frame(i * 10);
            if i % 2 == 0 {
                sync.poll_completion(i * 10);
            }
            if vsync && i % 5 == 0 {
                sync.signal_vsync();
            }
        }

        sync.stats()
    };

    let stats1 = run(true);
    let stats2 = run(true);

    assert_eq!(stats1.frames_submitted, stats2.frames_submitted);
    assert_eq!(stats1.frames_completed, stats2.frames_completed);
}

#[test]
fn test_state_machine_determinism() {
    let sync = GpuFrameSyncCapsule::new(60, false);

    // Predefined sequence
    let sequence = [(1u64, 100u64), (2, 200), (3, 300), (4, 400), (5, 500)];

    for (expected_frame, fence) in sequence.iter() {
        let frame = sync.begin_frame();
        assert_eq!(frame, *expected_frame);

        sync.submit_frame(*fence);
        assert_eq!(sync.current_fence(), *fence);

        sync.poll_completion(*fence);
        assert!(sync.is_completed());
    }

    let stats = sync.stats();
    assert_eq!(stats.current_frame, 5);
    assert_eq!(stats.frames_submitted, 5);
    assert_eq!(stats.frames_completed, 5);
}

// ============================================================================
// Additional Coverage Tests
// ============================================================================

#[test]
fn test_default_constructor() {
    let sync = GpuFrameSyncCapsule::default();
    let stats = sync.stats();
    assert_eq!(stats.current_frame, 0);
}

#[test]
fn test_size_and_alignment() {
    assert_eq!(core::mem::size_of::<GpuFrameSyncCapsule>(), 128);
    assert_eq!(core::mem::align_of::<GpuFrameSyncCapsule>(), 64);
}
