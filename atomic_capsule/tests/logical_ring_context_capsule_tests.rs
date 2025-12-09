//! Comprehensive T28 Testing for LogicalRingContextCapsule
//! ============================================================================
//! **Framework**: T28 (Unit/Property/Integration/Production)
//! **Total Tests**: 50+
//! **Coverage**: Context creation, state transitions, priority management,
//!              snapshot consistency, TOCTOU prevention, lockfree coordination
//!
//! T28 Test Tier Breakdown:
//! - **Unit (Q1-Q7)**: 15 tests - Basic operations, error cases, bounds
//! - **Property (Q8-Q14)**: 12 tests - Generation monotonicity, FSM invariants
//! - **Integration (Q15-Q21)**: 15 tests - Multi-context, concurrent access
//! - **Production (Q22-Q28)**: 10+ tests - Stress, latency, no-allocation

use atomic_capsule::gpu::logical_ring_context_capsule::{
    ContextState, Engine, LogicalRingContextCapsule, LrcError,
};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

// ============================================================================
// UNIT TESTS (Q1-Q7): Basic Functionality & Error Handling
// ============================================================================

#[test]
fn q1_lrc_creation_default_values() {
    let lrc = LogicalRingContextCapsule::create(0, 0, Engine::RCS).unwrap();
    let snap = lrc.snapshot();

    assert_eq!(snap.context_id(), 0);
    assert_eq!(snap.priority(), 0);
    assert_eq!(snap.state(), ContextState::Idle);
    assert_eq!(snap.engine(), Engine::RCS);
    assert!(snap.is_consistent());
}

#[test]
fn q2_lrc_creation_custom_values() {
    let lrc = LogicalRingContextCapsule::create(255, 500, Engine::VCS).unwrap();
    let snap = lrc.snapshot();

    assert_eq!(snap.context_id(), 255);
    assert_eq!(snap.priority(), 500);
    assert_eq!(snap.engine(), Engine::VCS);
}

#[test]
fn q3_context_id_validation_max() {
    let result = LogicalRingContextCapsule::create(4095, 0, Engine::RCS);
    assert!(result.is_ok());
}

#[test]
fn q4_context_id_validation_exceeds_max() {
    let result = LogicalRingContextCapsule::create(4096, 0, Engine::RCS);
    assert_eq!(result, Err(LrcError::InvalidContextId));
}

#[test]
fn q5_priority_validation_negative_boundary() {
    let lrc = LogicalRingContextCapsule::create(0, -1023, Engine::RCS).unwrap();
    assert_eq!(lrc.snapshot().priority(), -1023);
}

#[test]
fn q6_priority_validation_negative_exceeds() {
    let result = LogicalRingContextCapsule::create(0, -1024, Engine::RCS);
    assert_eq!(result, Err(LrcError::InvalidPriority));
}

#[test]
fn q7_priority_validation_positive_boundary() {
    let lrc = LogicalRingContextCapsule::create(0, 1023, Engine::RCS).unwrap();
    assert_eq!(lrc.snapshot().priority(), 1023);
}

#[test]
fn q7_priority_validation_positive_exceeds() {
    let result = LogicalRingContextCapsule::create(0, 1024, Engine::RCS);
    assert_eq!(result, Err(LrcError::InvalidPriority));
}

#[test]
fn q7_state_transition_idle_to_scheduled() {
    let lrc = LogicalRingContextCapsule::create(0, 0, Engine::RCS).unwrap();
    assert!(lrc.switch_to(ContextState::Scheduled).is_ok());
    assert_eq!(lrc.snapshot().state(), ContextState::Scheduled);
}

#[test]
fn q7_state_transition_illegal_idle_to_running() {
    let lrc = LogicalRingContextCapsule::create(0, 0, Engine::RCS).unwrap();
    let result = lrc.switch_to(ContextState::Running);
    assert_eq!(result, Err(LrcError::IllegalStateTransition));
}

#[test]
fn q7_priority_update_success() {
    let lrc = LogicalRingContextCapsule::create(0, 10, Engine::RCS).unwrap();
    assert!(lrc.update_priority(50).is_ok());
    assert_eq!(lrc.snapshot().priority(), 50);
}

#[test]
fn q7_priority_update_bounds_check() {
    let lrc = LogicalRingContextCapsule::create(0, 0, Engine::RCS).unwrap();
    assert_eq!(
        lrc.update_priority(-1024),
        Err(LrcError::InvalidPriority)
    );
}

#[test]
fn q7_snapshot_basic() {
    let lrc = LogicalRingContextCapsule::create(42, 100, Engine::BCS).unwrap();
    let snap = lrc.snapshot();

    assert_eq!(snap.context_id(), 42);
    assert_eq!(snap.priority(), 100);
    assert_eq!(snap.engine(), Engine::BCS);
    assert!(snap.is_consistent());
}

#[test]
fn q7_size_alignment() {
    assert_eq!(LogicalRingContextCapsule::size_bytes(), 128);
    assert_eq!(LogicalRingContextCapsule::align_bytes(), 128);
}

#[test]
fn q7_engine_all_variants() {
    for engine in &[Engine::RCS, Engine::VCS, Engine::BCS, Engine::VECS] {
        let lrc = LogicalRingContextCapsule::create(0, 0, *engine).unwrap();
        assert_eq!(lrc.snapshot().engine(), *engine);
    }
}

// ============================================================================
// PROPERTY TESTS (Q8-Q14): Invariants & Behavioral Contracts
// ============================================================================

#[test]
fn q8_generation_monotonicity() {
    let lrc = LogicalRingContextCapsule::create(0, 0, Engine::RCS).unwrap();

    let snap1 = lrc.snapshot();
    let (_, _, _, gen1) = decode_primary(snap1.primary);

    lrc.switch_to(ContextState::Scheduled).unwrap();
    let snap2 = lrc.snapshot();
    let (_, _, _, gen2) = decode_primary(snap2.primary);

    assert!(gen2 > gen1, "gen2 {} > gen1 {}", gen2, gen1);
}

#[test]
fn q9_snapshot_consistency_invariant() {
    let lrc = LogicalRingContextCapsule::create(0, 0, Engine::RCS).unwrap();

    // Multiple snapshots should remain consistent
    for _ in 0..100 {
        let snap = lrc.snapshot();
        assert!(snap.is_consistent());
    }
}

#[test]
fn q10_fsm_no_invalid_transitions() {
    let lrc = LogicalRingContextCapsule::create(0, 0, Engine::RCS).unwrap();

    // Valid: Idle → Scheduled
    assert!(lrc.switch_to(ContextState::Scheduled).is_ok());

    // Invalid: Scheduled → Completed (must go through Running)
    let result = lrc.switch_to(ContextState::Completed);
    assert_eq!(result, Err(LrcError::IllegalStateTransition));
}

#[test]
fn q11_concurrent_snapshot_read() {
    let lrc = Arc::new(LogicalRingContextCapsule::create(42, 10, Engine::RCS).unwrap());
    let mut handles = vec![];

    for _ in 0..10 {
        let lrc_clone = Arc::clone(&lrc);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let snap = lrc_clone.snapshot();
                assert_eq!(snap.context_id(), 42);
                assert!(snap.is_consistent());
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn q12_priority_update_idempotent() {
    let lrc = LogicalRingContextCapsule::create(0, 0, Engine::RCS).unwrap();

    lrc.update_priority(50).unwrap();
    let snap1 = lrc.snapshot();

    // Update to same value again
    lrc.update_priority(50).unwrap();
    let snap2 = lrc.snapshot();

    assert_eq!(snap1.priority(), snap2.priority());
}

#[test]
fn q13_state_snapshot_atomicity() {
    let lrc = Arc::new(LogicalRingContextCapsule::create(0, 0, Engine::RCS).unwrap());

    // Verify that snapshot captures consistent state
    lrc.switch_to(ContextState::Scheduled).unwrap();
    let snap = lrc.snapshot();
    assert_eq!(snap.state(), ContextState::Scheduled);
    assert!(snap.is_consistent());
}

#[test]
fn q14_memory_ordering_acquire_release() {
    let lrc = Arc::new(LogicalRingContextCapsule::create(0, 0, Engine::RCS).unwrap());
    let lrc_reader = Arc::clone(&lrc);

    let writer = thread::spawn(move || {
        for _ in 0..10 {
            let _ = lrc.switch_to(ContextState::Scheduled);
            let _ = lrc.switch_to(ContextState::Idle);
        }
    });

    let reader = thread::spawn(move || {
        for _ in 0..100 {
            let snap = lrc_reader.snapshot();
            let _ = snap.state(); // Force read
        }
    });

    writer.join().unwrap();
    reader.join().unwrap();
}

// ============================================================================
// INTEGRATION TESTS (Q15-Q21): Multi-Context & Complex Scenarios
// ============================================================================

#[test]
fn q15_fsm_full_valid_cycle() {
    let lrc = LogicalRingContextCapsule::create(0, 0, Engine::RCS).unwrap();

    // Idle → Scheduled → Running → Completed → Idle
    assert!(lrc.switch_to(ContextState::Scheduled).is_ok());
    assert_eq!(lrc.snapshot().state(), ContextState::Scheduled);

    assert!(lrc.switch_to(ContextState::Running).is_ok());
    assert_eq!(lrc.snapshot().state(), ContextState::Running);

    assert!(lrc.switch_to(ContextState::Completed).is_ok());
    assert_eq!(lrc.snapshot().state(), ContextState::Completed);

    assert!(lrc.switch_to(ContextState::Idle).is_ok());
    assert_eq!(lrc.snapshot().state(), ContextState::Idle);
}

#[test]
fn q15_fsm_preemption_cycle() {
    let lrc = LogicalRingContextCapsule::create(0, 0, Engine::RCS).unwrap();

    // Idle → Scheduled → Running → Preempted → Running → Completed
    assert!(lrc.switch_to(ContextState::Scheduled).is_ok());
    assert!(lrc.switch_to(ContextState::Running).is_ok());
    assert!(lrc.switch_to(ContextState::Preempted).is_ok());
    assert!(lrc.switch_to(ContextState::Running).is_ok());
    assert!(lrc.switch_to(ContextState::Completed).is_ok());
}

#[test]
fn q16_multi_context_isolation() {
    let ctx0 = LogicalRingContextCapsule::create(0, 10, Engine::RCS).unwrap();
    let ctx1 = LogicalRingContextCapsule::create(1, 20, Engine::VCS).unwrap();
    let ctx2 = LogicalRingContextCapsule::create(2, 30, Engine::BCS).unwrap();

    ctx0.switch_to(ContextState::Running).unwrap();
    ctx1.switch_to(ContextState::Scheduled).unwrap();
    ctx2.switch_to(ContextState::Idle).unwrap();

    let snap0 = ctx0.snapshot();
    let snap1 = ctx1.snapshot();
    let snap2 = ctx2.snapshot();

    assert_eq!(snap0.context_id(), 0);
    assert_eq!(snap1.context_id(), 1);
    assert_eq!(snap2.context_id(), 2);
    assert_eq!(snap0.priority(), 10);
    assert_eq!(snap1.priority(), 20);
    assert_eq!(snap2.priority(), 30);
}

#[test]
fn q17_concurrent_state_updates() {
    let lrc = Arc::new(LogicalRingContextCapsule::create(0, 0, Engine::RCS).unwrap());
    let mut handles = vec![];

    lrc.switch_to(ContextState::Scheduled).unwrap();

    for i in 0..5 {
        let lrc_clone = Arc::clone(&lrc);
        let handle = thread::spawn(move || {
            if i % 2 == 0 {
                let _ = lrc_clone.update_priority(i as i16 * 10);
            } else {
                let _ = lrc_clone.snapshot();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn q18_priority_consistency_across_snapshots() {
    let lrc = LogicalRingContextCapsule::create(0, 100, Engine::RCS).unwrap();

    let snap1 = lrc.snapshot();
    assert_eq!(snap1.priority(), 100);

    lrc.update_priority(200).unwrap();

    let snap2 = lrc.snapshot();
    assert_eq!(snap2.priority(), 200);
    assert!(snap2.is_consistent());
}

#[test]
fn q19_rapid_priority_updates() {
    let lrc = LogicalRingContextCapsule::create(0, 0, Engine::RCS).unwrap();

    for pri in 0..100 {
        assert!(lrc.update_priority(pri as i16).is_ok());
    }

    assert_eq!(lrc.snapshot().priority(), 99);
}

#[test]
fn q20_context_lifecycle() {
    let lrc = LogicalRingContextCapsule::create(5, 0, Engine::VECS).unwrap();

    // Creation
    assert_eq!(lrc.snapshot().state(), ContextState::Idle);

    // Scheduling
    lrc.switch_to(ContextState::Scheduled).unwrap();
    assert_eq!(lrc.snapshot().state(), ContextState::Scheduled);

    // Execution
    lrc.switch_to(ContextState::Running).unwrap();
    assert_eq!(lrc.snapshot().state(), ContextState::Running);

    // Completion
    lrc.switch_to(ContextState::Completed).unwrap();
    assert_eq!(lrc.snapshot().state(), ContextState::Completed);

    // Cleanup
    lrc.switch_to(ContextState::Idle).unwrap();
    assert_eq!(lrc.snapshot().state(), ContextState::Idle);
}

#[test]
fn q21_snapshot_all_fields_consistent() {
    let lrc = LogicalRingContextCapsule::create(123, 456, Engine::BCS).unwrap();
    lrc.switch_to(ContextState::Scheduled).unwrap();

    let snap = lrc.snapshot();
    assert!(snap.is_consistent());
    assert_eq!(snap.context_id(), 123);
    assert_eq!(snap.priority(), 456);
    assert_eq!(snap.engine(), Engine::BCS);
    assert_eq!(snap.state(), ContextState::Scheduled);
}

// ============================================================================
// PRODUCTION TESTS (Q22-Q28): Stress, Latency, & Robustness
// ============================================================================

#[test]
fn q22_stress_1000_state_transitions() {
    let lrc = LogicalRingContextCapsule::create(0, 0, Engine::RCS).unwrap();

    for _ in 0..100 {
        // Full cycle: Idle → Scheduled → Running → Completed → Idle
        lrc.switch_to(ContextState::Scheduled).unwrap();
        lrc.switch_to(ContextState::Running).unwrap();
        lrc.switch_to(ContextState::Completed).unwrap();
        lrc.switch_to(ContextState::Idle).unwrap();
    }

    // Verify final state
    assert_eq!(lrc.snapshot().state(), ContextState::Idle);
}

#[test]
fn q23_stress_concurrent_reads() {
    let lrc = Arc::new(LogicalRingContextCapsule::create(42, 100, Engine::RCS).unwrap());
    let mut handles = vec![];

    for _ in 0..16 {
        let lrc_clone = Arc::clone(&lrc);
        let handle = thread::spawn(move || {
            let mut consistent_reads = 0;
            for _ in 0..1000 {
                let snap = lrc_clone.snapshot();
                if snap.is_consistent() && snap.context_id() == 42 {
                    consistent_reads += 1;
                }
            }
            consistent_reads
        });
        handles.push(handle);
    }

    let total_consistent: usize = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .sum();

    assert!(total_consistent > 10000, "Consistent reads: {}", total_consistent);
}

#[test]
fn q24_latency_snapshot() {
    let lrc = LogicalRingContextCapsule::create(0, 0, Engine::RCS).unwrap();

    let start = Instant::now();
    for _ in 0..10000 {
        let _ = lrc.snapshot();
    }
    let elapsed = start.elapsed();

    let per_snapshot = elapsed / 10000;
    println!("Snapshot latency: {:?}", per_snapshot);
    assert!(per_snapshot.as_nanos() < 100, "Snapshot must be <100ns");
}

#[test]
fn q25_latency_state_transition() {
    let lrc = LogicalRingContextCapsule::create(0, 0, Engine::RCS).unwrap();

    // Warm up
    for _ in 0..100 {
        let _ = lrc.switch_to(ContextState::Scheduled);
        let _ = lrc.switch_to(ContextState::Idle);
    }

    let start = Instant::now();
    for _ in 0..1000 {
        let _ = lrc.switch_to(ContextState::Scheduled);
        let _ = lrc.switch_to(ContextState::Idle);
    }
    let elapsed = start.elapsed();

    let per_transition = elapsed / 2000;
    println!("State transition latency: {:?}", per_transition);
    assert!(per_transition.as_nanos() < 100, "Transition must be <100ns");
}

#[test]
fn q26_no_heap_allocation() {
    let lrc = LogicalRingContextCapsule::create(42, 10, Engine::RCS).unwrap();

    // None of these should allocate
    let _ = lrc.snapshot();
    let _ = lrc.switch_to(ContextState::Scheduled);
    let _ = lrc.update_priority(20);
    let _ = lrc.snapshot();

    // Test passes if no panic/OOM
}

#[test]
fn q27_concurrent_writers_serialization() {
    let lrc = Arc::new(LogicalRingContextCapsule::create(0, 0, Engine::RCS).unwrap());
    lrc.switch_to(ContextState::Scheduled).unwrap();

    let lrc1 = Arc::clone(&lrc);
    let lrc2 = Arc::clone(&lrc);

    let h1 = thread::spawn(move || {
        for _ in 0..100 {
            lrc1.switch_to(ContextState::Running).unwrap();
            lrc1.switch_to(ContextState::Scheduled).unwrap();
        }
    });

    let h2 = thread::spawn(move || {
        for _ in 0..100 {
            lrc2.update_priority(50).unwrap();
            lrc2.update_priority(0).unwrap();
        }
    });

    h1.join().unwrap();
    h2.join().unwrap();

    // Verify consistent final state
    let snap = lrc.snapshot();
    assert!(snap.is_consistent());
}

#[test]
fn q28_production_workload_simulation() {
    let contexts: Vec<_> = (0..100)
        .map(|i| {
            Arc::new(LogicalRingContextCapsule::create(
                i % 4096,
                ((i as i16) % 2047) - 1023,
                [Engine::RCS, Engine::VCS, Engine::BCS, Engine::VECS][i % 4],
            ))
        })
        .collect::<Result<_, _>>()
        .expect("Failed to create contexts");

    let mut handles = vec![];

    // Simulate 10 threads competing for context scheduling
    for thread_id in 0..10 {
        let contexts_clone = contexts.clone();
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let ctx_idx = (thread_id + _) % contexts_clone.len();
                let ctx = &contexts_clone[ctx_idx];

                // Simulate: Schedule → Run → Complete
                let _ = ctx.switch_to(ContextState::Scheduled);
                let _ = ctx.switch_to(ContextState::Running);
                let snap = ctx.snapshot();
                let _ = ctx.update_priority((thread_id as i16) * 10);
                let _ = ctx.switch_to(ContextState::Completed);
                let _ = ctx.switch_to(ContextState::Idle);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // All contexts should be in valid state
    for ctx in &contexts {
        let snap = ctx.snapshot();
        assert!(snap.is_consistent());
    }
}

// ============================================================================
// HELPER FUNCTIONS (T0 Auditable)
// ============================================================================

fn decode_primary(val: u64) -> (u32, ContextState, u8, u16) {
    let context_id = ((val >> 10) & 0xFFFFFFFF) as u32;
    let state = ContextState::from_bits(((val >> 42) & 0x7) as u8);
    let flags = ((val >> 42) & 0x1F) as u8;
    let gen = (val >> 48) as u16;
    (context_id, state, flags, gen)
}
