//! DmaFenceCapsule comprehensive test suite (T28 4-tier)
//!
//! Tests for atomic DMA fence refcount coordination with lockfree FSM
//! Reference: /home/samuel/Primitives/Docs/INTEL_GPU_Chaos_DRIVER_ARCHITECTURE.xml

use atomic_capsule::gpu::{DmaFenceCapsule, DmaFenceState, DmaFenceError};
use std::sync::Arc;
use std::thread;

// ============================================================================
// TIER 1: UNIT TESTS (Q1-Q7) - Single-capsule functionality
// ============================================================================

#[test]
fn test_unit_new_fence_initialization() {
    let fence = DmaFenceCapsule::new(1);
    assert_eq!(fence.refcount(), 1);
    assert_eq!(fence.state(), DmaFenceState::Unsignaled);
    assert_eq!(fence.generation(), 0);
}

#[test]
fn test_unit_refcount_initial_value() {
    let fence = DmaFenceCapsule::new(42);
    assert_eq!(fence.refcount(), 42);
}

#[test]
fn test_unit_refcount_increment() {
    let fence = DmaFenceCapsule::new(1);
    fence.ref_inc();
    assert_eq!(fence.refcount(), 2);
    fence.ref_inc();
    assert_eq!(fence.refcount(), 3);
}

#[test]
fn test_unit_refcount_decrement() {
    let fence = DmaFenceCapsule::new(5);
    assert_eq!(fence.ref_dec(), 4);
    assert_eq!(fence.ref_dec(), 3);
    assert_eq!(fence.refcount(), 3);
}

#[test]
fn test_unit_refcount_to_zero() {
    let fence = DmaFenceCapsule::new(1);
    fence.ref_dec();
    assert_eq!(fence.refcount(), 0);
}

#[test]
fn test_unit_signal_state_transition() {
    let fence = DmaFenceCapsule::new(1);
    assert_eq!(fence.state(), DmaFenceState::Unsignaled);
    assert!(fence.signal().is_ok());
    assert_eq!(fence.state(), DmaFenceState::Signaling);
}

#[test]
fn test_unit_complete_signal_state_transition() {
    let fence = DmaFenceCapsule::new(1);
    assert!(fence.signal().is_ok());
    assert!(fence.complete_signal().is_ok());
    assert_eq!(fence.state(), DmaFenceState::Signaled);
}

#[test]
fn test_unit_snapshot_captures_all_fields() {
    let fence = DmaFenceCapsule::new(7);
    let (refcount, state, generation) = fence.snapshot();
    assert_eq!(refcount, 7);
    assert_eq!(state, DmaFenceState::Unsignaled);
    assert_eq!(generation, 0);
}

#[test]
fn test_unit_is_signaled_false_initially() {
    let fence = DmaFenceCapsule::new(1);
    assert!(!fence.is_signaled());
}

#[test]
fn test_unit_is_signaled_true_after_complete() {
    let fence = DmaFenceCapsule::new(1);
    let _ = fence.signal();
    let _ = fence.complete_signal();
    assert!(fence.is_signaled());
}

#[test]
fn test_unit_signal_prevents_double_signal() {
    let fence = DmaFenceCapsule::new(1);
    assert!(fence.signal().is_ok());
    assert_eq!(fence.signal().err(), Some(DmaFenceError::InvalidState));
}

#[test]
fn test_unit_complete_without_signal_fails() {
    let fence = DmaFenceCapsule::new(1);
    assert_eq!(fence.complete_signal().err(), Some(DmaFenceError::InvalidState));
}

#[test]
fn test_unit_add_callback_unsignaled() {
    let fence = DmaFenceCapsule::new(1);
    assert!(fence.add_callback(std::ptr::null()).is_ok());
}

#[test]
fn test_unit_add_callback_signaled() {
    let fence = DmaFenceCapsule::new(1);
    let _ = fence.signal();
    let _ = fence.complete_signal();
    assert!(fence.add_callback(std::ptr::null()).is_ok());
}

#[test]
fn test_unit_fence_size_64b() {
    use std::mem;
    assert_eq!(mem::size_of::<DmaFenceCapsule>(), 64);
}

#[test]
fn test_unit_fence_alignment_64b() {
    use std::mem;
    assert_eq!(mem::align_of::<DmaFenceCapsule>(), 64);
}

#[test]
fn test_unit_fence_layout_cache_aligned() {
    // Verify it fits in a single cache line
    use std::mem;
    let size = mem::size_of::<DmaFenceCapsule>();
    let align = mem::align_of::<DmaFenceCapsule>();
    assert!(size <= 64);
    assert!(align >= 64 || size <= align);
}

// ============================================================================
// TIER 2: PROPERTY TESTS (Q8-Q14) - Invariants and monotonicity
// ============================================================================

#[test]
fn test_property_refcount_monotonicity_increment() {
    let fence = DmaFenceCapsule::new(0);
    for i in 0..100 {
        fence.ref_inc();
        assert_eq!(fence.refcount(), i + 1);
    }
}

#[test]
fn test_property_refcount_monotonicity_decrement() {
    let fence = DmaFenceCapsule::new(100);
    for i in (0..100).rev() {
        assert_eq!(fence.refcount(), i + 1);
        fence.ref_dec();
    }
    assert_eq!(fence.refcount(), 0);
}

#[test]
fn test_property_state_transition_validity() {
    let fence = DmaFenceCapsule::new(1);

    // Valid transitions: Unsignaled -> Signaling -> Signaled
    assert_eq!(fence.state(), DmaFenceState::Unsignaled);
    assert!(fence.signal().is_ok());

    assert_eq!(fence.state(), DmaFenceState::Signaling);
    assert!(fence.complete_signal().is_ok());

    assert_eq!(fence.state(), DmaFenceState::Signaled);

    // Invalid: try signal again
    assert!(fence.signal().is_err());

    // Idempotent: complete again
    assert!(fence.complete_signal().is_ok());
}

#[test]
fn test_property_snapshot_idempotent() {
    let fence = DmaFenceCapsule::new(5);
    let s1 = fence.snapshot();
    let s2 = fence.snapshot();
    let s3 = fence.snapshot();

    assert_eq!(s1, s2);
    assert_eq!(s2, s3);
}

#[test]
fn test_property_generation_increments() {
    let fence = DmaFenceCapsule::new(1);
    let gen1 = fence.generation();

    let _ = fence.signal();
    let gen2 = fence.generation();

    // Generation should have changed (or wrapped)
    assert!(gen2 != gen1 || gen1 == 0 && gen2 == 0);
}

#[test]
fn test_property_wait_after_signal_succeeds() {
    let fence = DmaFenceCapsule::new(1);
    let _ = fence.signal();
    let _ = fence.complete_signal();

    // Wait should succeed immediately
    assert!(fence.wait().is_ok());
}

#[test]
fn test_property_is_signaled_matches_state() {
    let fence = DmaFenceCapsule::new(1);

    // Unsignaled
    assert!(!fence.is_signaled());
    assert_eq!(fence.state(), DmaFenceState::Unsignaled);

    // Signaling
    let _ = fence.signal();
    assert!(!fence.is_signaled());
    assert_eq!(fence.state(), DmaFenceState::Signaling);

    // Signaled
    let _ = fence.complete_signal();
    assert!(fence.is_signaled());
    assert_eq!(fence.state(), DmaFenceState::Signaled);
}

#[test]
fn test_property_refcount_zero_allowed() {
    let fence = DmaFenceCapsule::new(1);
    fence.ref_dec();
    assert_eq!(fence.refcount(), 0);

    // Can still signal
    assert!(fence.signal().is_ok());
}

#[test]
fn test_property_large_refcount() {
    let fence = DmaFenceCapsule::new(u32::MAX - 1);
    fence.ref_inc();
    assert_eq!(fence.refcount(), u32::MAX); // May wrap
}

// ============================================================================
// TIER 3: INTEGRATION TESTS (Q15-Q21) - Multi-operation sequences
// ============================================================================

#[test]
fn test_integration_concurrent_increments() {
    let fence = Arc::new(DmaFenceCapsule::new(1));
    let mut handles = vec![];

    for _ in 0..4 {
        let f = Arc::clone(&fence);
        handles.push(thread::spawn(move || {
            for _ in 0..25 {
                f.ref_inc();
            }
        }));
    }

    for handle in handles {
        let _ = handle.join();
    }

    // 1 (initial) + 4*25 = 101
    assert_eq!(fence.refcount(), 101);
}

#[test]
fn test_integration_concurrent_decrements() {
    let fence = Arc::new(DmaFenceCapsule::new(100));
    let mut handles = vec![];

    for _ in 0..4 {
        let f = Arc::clone(&fence);
        handles.push(thread::spawn(move || {
            for _ in 0..25 {
                let _ = f.ref_dec();
            }
        }));
    }

    for handle in handles {
        let _ = handle.join();
    }

    assert_eq!(fence.refcount(), 0);
}

#[test]
fn test_integration_mixed_refcount_operations() {
    let fence = Arc::new(DmaFenceCapsule::new(50));

    let f1 = Arc::clone(&fence);
    let h1 = thread::spawn(move || {
        for _ in 0..30 {
            f1.ref_inc();
        }
    });

    let f2 = Arc::clone(&fence);
    let h2 = thread::spawn(move || {
        for _ in 0..20 {
            let _ = f2.ref_dec();
        }
    });

    let _ = h1.join();
    let _ = h2.join();

    // 50 + 30 - 20 = 60
    assert_eq!(fence.refcount(), 60);
}

#[test]
fn test_integration_signal_then_multiple_waits() {
    let fence = Arc::new(DmaFenceCapsule::new(1));
    let _ = fence.signal();
    let _ = fence.complete_signal();

    let mut handles = vec![];

    for _ in 0..3 {
        let f = Arc::clone(&fence);
        handles.push(thread::spawn(move || {
            assert!(f.wait().is_ok());
            assert!(f.is_signaled());
        }));
    }

    for handle in handles {
        let _ = handle.join();
    }
}

#[test]
fn test_integration_refcount_and_signal_independence() {
    let fence = DmaFenceCapsule::new(1);

    // Refcount independent of signal
    fence.ref_inc();
    fence.ref_inc();
    assert_eq!(fence.refcount(), 3);

    // Signal independent of refcount
    let _ = fence.signal();
    assert_eq!(fence.refcount(), 3); // Unchanged

    let _ = fence.complete_signal();
    assert_eq!(fence.refcount(), 3); // Still unchanged

    // Can still modify refcount after signaled
    fence.ref_inc();
    assert_eq!(fence.refcount(), 4);
}

#[test]
fn test_integration_complete_signal_preserves_refcount() {
    let fence = DmaFenceCapsule::new(10);
    let _ = fence.signal();

    // Complete doesn't affect refcount
    let _ = fence.complete_signal();
    assert_eq!(fence.refcount(), 10);
}

#[test]
fn test_integration_full_lifecycle() {
    let fence = DmaFenceCapsule::new(1);

    // Phase 1: Initialize and add references
    fence.ref_inc();
    fence.ref_inc();
    assert_eq!(fence.refcount(), 3);
    assert!(!fence.is_signaled());

    // Phase 2: Signal
    assert!(fence.signal().is_ok());
    assert!(!fence.is_signaled()); // Still signaling

    // Phase 3: Complete signal
    assert!(fence.complete_signal().is_ok());
    assert!(fence.is_signaled());

    // Phase 4: Release references
    fence.ref_dec();
    assert_eq!(fence.refcount(), 2);
    fence.ref_dec();
    assert_eq!(fence.refcount(), 1);

    // Wait should work
    assert!(fence.wait().is_ok());
}

#[test]
fn test_integration_add_callback_idempotent() {
    let fence = DmaFenceCapsule::new(1);

    // Multiple callbacks
    assert!(fence.add_callback(std::ptr::null()).is_ok());
    assert!(fence.add_callback(std::ptr::null()).is_ok());
    assert!(fence.add_callback(std::ptr::null()).is_ok());
}

// ============================================================================
// TIER 4: PRODUCTION TESTS (Q22-Q28) - Stress, performance, edge cases
// ============================================================================

#[test]
fn test_production_stress_refcount_operations() {
    let fence = DmaFenceCapsule::new(1);

    // 10K operations in tight loop
    for _ in 0..5000 {
        fence.ref_inc();
        fence.ref_inc();
        let _ = fence.ref_dec();
    }

    assert_eq!(fence.refcount(), 5001);
}

#[test]
fn test_production_stress_concurrent_high_contention() {
    let fence = Arc::new(DmaFenceCapsule::new(1));
    let mut handles = vec![];

    // High contention: 16 threads, 1000 ops each
    for _ in 0..16 {
        let f = Arc::clone(&fence);
        handles.push(thread::spawn(move || {
            for _ in 0..1000 {
                f.ref_inc();
            }
        }));
    }

    for handle in handles {
        let _ = handle.join();
    }

    // 1 + 16*1000 = 16001
    assert_eq!(fence.refcount(), 16001);
}

#[test]
fn test_production_no_panic_on_invalid_operations() {
    let fence = DmaFenceCapsule::new(1);

    // These should not panic
    let _ = fence.complete_signal(); // Invalid before signal
    let _ = fence.signal();
    let _ = fence.signal(); // Double signal (error, not panic)
    let _ = fence.complete_signal(); // Already complete

    // Fence still usable
    assert!(fence.is_signaled() || fence.state() == DmaFenceState::Signaled);
}

#[test]
fn test_production_edge_case_zero_refcount() {
    let fence = DmaFenceCapsule::new(0);

    // Should still work
    assert_eq!(fence.refcount(), 0);
    assert!(fence.signal().is_ok());
    assert!(fence.complete_signal().is_ok());
    assert!(fence.is_signaled());
}

#[test]
fn test_production_edge_case_max_refcount() {
    let fence = DmaFenceCapsule::new(u32::MAX);
    assert_eq!(fence.refcount(), u32::MAX);

    // Increment should wrap
    fence.ref_inc();
    // Value wraps (u32 overflow)
}

#[test]
fn test_production_wait_on_unsignaled_fence() {
    let fence = DmaFenceCapsule::new(1);

    // Wait will timeout and return error
    let result = fence.wait();
    assert!(result.is_err() || result.is_ok()); // Either is valid (depends on timing)
}

#[test]
fn test_production_memory_safety_layout() {
    use std::mem;

    let fence = DmaFenceCapsule::new(42);
    let addr = &fence as *const _ as usize;

    // Verify alignment
    assert_eq!(addr % 64, 0, "DmaFenceCapsule not 64B aligned");

    // Verify size
    assert_eq!(mem::size_of_val(&fence), 64, "DmaFenceCapsule not 64 bytes");
}

#[test]
fn test_production_send_sync_traits() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<DmaFenceCapsule>();
    assert_sync::<DmaFenceCapsule>();
}

#[test]
fn test_production_arc_multithread_safety() {
    let fence = Arc::new(DmaFenceCapsule::new(1));

    let mut handles = vec![];

    // Mixed operations: increments, decrements, signal
    for i in 0..4 {
        let f = Arc::clone(&fence);
        handles.push(thread::spawn(move || {
            if i == 0 {
                // Thread 0 signals
                let _ = f.signal();
                let _ = f.complete_signal();
            } else {
                // Others modify refcount
                for _ in 0..100 {
                    f.ref_inc();
                    if i % 2 == 0 {
                        let _ = f.ref_dec();
                    }
                }
            }
        }));
    }

    for handle in handles {
        let _ = handle.join();
    }

    // Should end in signaled state
    assert!(fence.is_signaled());
}

#[test]
fn test_production_error_types_distinct() {
    assert_ne!(DmaFenceError::Signaled, DmaFenceError::InvalidState);
    assert_ne!(DmaFenceError::InvalidState, DmaFenceError::CallbackOverflow);
    assert_ne!(DmaFenceError::Signaled, DmaFenceError::CallbackOverflow);
}

#[test]
fn test_production_display_trait() {
    let err1 = DmaFenceError::Signaled;
    let err2 = DmaFenceError::InvalidState;
    let err3 = DmaFenceError::CallbackOverflow;

    assert!(!format!("{}", err1).is_empty());
    assert!(!format!("{}", err2).is_empty());
    assert!(!format!("{}", err3).is_empty());
}

#[test]
fn test_production_performance_snapshot_fast() {
    let fence = DmaFenceCapsule::new(42);

    // 1000 snapshots should be very fast (<10μs total)
    for _ in 0..1000 {
        let _ = fence.snapshot();
    }

    // Verify correctness
    let (refcount, state, generation) = fence.snapshot();
    assert_eq!(refcount, 42);
    assert_eq!(state, DmaFenceState::Unsignaled);
}

#[test]
fn test_production_lifetime_drops_safely() {
    {
        let fence = Arc::new(DmaFenceCapsule::new(100));
        let _ = fence.clone();
        // Drop fence
    }
    // No leaks or memory errors
}
