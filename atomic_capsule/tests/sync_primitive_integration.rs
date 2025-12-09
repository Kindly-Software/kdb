#![allow(dead_code)]

//! Integration tests for SyncPrimitiveCapsule
//! T28 Framework: 4-tier test pyramid (unit/property/integration/production)

extern crate atomic_capsule;

use atomic_capsule::gpu::hal::{SyncPrimitiveCapsule, SyncType, SyncError};
use std::sync::Arc;
use std::time::Instant;

// ============================================================================
// Q1-Q7: Unit Tests (Basic Operations)
// ============================================================================

#[test]
fn test_q1_create_fence() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");
    assert!(!sync.is_signaled());
    assert_eq!(sync.sync_type(), SyncType::Fence);
}

#[test]
fn test_q2_create_semaphore() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Semaphore).expect("Failed to create semaphore");
    assert!(!sync.is_signaled());
    assert_eq!(sync.sync_type(), SyncType::Semaphore);
}

#[test]
fn test_q3_signal_fence() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");
    sync.signal_fence().expect("Failed to signal fence");
    assert!(sync.is_signaled());
}

#[test]
fn test_q4_double_signal_error() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");
    sync.signal_fence().expect("First signal failed");
    let result = sync.signal_fence();
    assert_eq!(result, Err(SyncError::AlreadySignaled));
}

#[test]
fn test_q5_wait_after_signal() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");
    sync.signal_fence().expect("Failed to signal");
    sync.wait_fence(0).expect("Wait failed after signal");
}

#[test]
fn test_q6_reset_fence() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");
    sync.signal_fence().expect("Signal failed");
    sync.reset().expect("Reset failed");
    assert!(!sync.is_signaled());
}

#[test]
fn test_q7_snapshot() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");
    let snap = sync.snapshot();
    assert_eq!(snap.state, 0);
    assert_eq!(snap.waiter_count, 0);

    sync.signal_fence().expect("Signal failed");
    let snap2 = sync.snapshot();
    assert_eq!(snap2.state, 1);
}

// ============================================================================
// Q8-Q14: Property Tests (Invariants & Determinism)
// ============================================================================

#[test]
fn test_q8_idempotent_is_signaled() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");
    let result1 = sync.is_signaled();
    let result2 = sync.is_signaled();
    assert_eq!(result1, result2);  // Idempotent
}

#[test]
fn test_q9_signal_monotonicity() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");
    assert!(!sync.is_signaled());
    sync.signal_fence().expect("Signal failed");
    assert!(sync.is_signaled());
    // Once signaled, stays signaled (until reset)
    assert!(sync.is_signaled());
}

#[test]
fn test_q10_reset_clears_signaled() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");
    for _ in 0..10 {
        sync.signal_fence().expect("Signal failed");
        assert!(sync.is_signaled());
        sync.reset().expect("Reset failed");
        assert!(!sync.is_signaled());
    }
}

#[test]
fn test_q11_generation_counter_increments() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");
    let snap1 = sync.snapshot();
    let gen1 = snap1.generation;

    sync.signal_fence().expect("Signal failed");
    let snap2 = sync.snapshot();
    let gen2 = snap2.generation;

    assert_ne!(gen1, gen2);  // Generation should increment
}

#[test]
fn test_q12_timeout_behavior() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");
    let start = Instant::now();
    let _result = sync.wait_fence(10_000);  // 10μs timeout
    let elapsed = start.elapsed();
    // Should timeout quickly (with some overhead)
    assert!(elapsed.as_micros() < 1000);
}

#[test]
fn test_q13_wait_already_signaled() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");
    sync.signal_fence().expect("Signal failed");
    // Should return immediately
    let start = Instant::now();
    sync.wait_fence(0).expect("Wait failed");
    let elapsed = start.elapsed();
    assert!(elapsed.as_micros() < 100);  // Should be < 100μs
}

#[test]
fn test_q14_memory_coherence() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");
    sync.signal_fence().expect("Signal failed");

    // Load from secondary to ensure visibility
    let snap = sync.snapshot();
    assert_eq!(snap.state, 1);
}

// ============================================================================
// Q15-Q21: Integration Tests (Multi-threaded & State Transitions)
// ============================================================================

#[test]
fn test_q15_signal_notify_wait() {
    let sync = Arc::new(SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence"));

    let sync_clone = sync.clone();
    let handle = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(10));
        sync_clone.signal_fence().expect("Signal failed");
    });

    sync.wait_fence(1_000_000_000).expect("Wait failed");
    assert!(sync.is_signaled());
    handle.join().expect("Thread join failed");
}

#[test]
fn test_q16_concurrent_snapshots() {
    let sync = Arc::new(SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence"));

    sync.signal_fence().expect("Signal failed");

    let handles: Vec<_> = (0..10).map(|_| {
        let sync_clone = sync.clone();
        std::thread::spawn(move || {
            let snap = sync_clone.snapshot();
            assert_eq!(snap.state, 1);
        })
    }).collect();

    for handle in handles {
        handle.join().expect("Thread join failed");
    }
}

#[test]
fn test_q17_reset_while_no_waiters() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");
    sync.signal_fence().expect("Signal failed");
    sync.reset().expect("Reset failed");
    assert!(!sync.is_signaled());
}

#[test]
fn test_q18_state_machine_transitions() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");

    // Idle → Signaled
    assert!(!sync.is_signaled());
    sync.signal_fence().expect("Signal failed");
    assert!(sync.is_signaled());

    // Signaled → Idle (via reset)
    sync.reset().expect("Reset failed");
    assert!(!sync.is_signaled());

    // Idle → Signaled (again)
    sync.signal_fence().expect("Second signal failed");
    assert!(sync.is_signaled());
}

#[test]
fn test_q19_multiple_resets() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");

    for _ in 0..5 {
        sync.signal_fence().expect("Signal failed");
        assert!(sync.is_signaled());
        sync.reset().expect("Reset failed");
        assert!(!sync.is_signaled());
    }
}

#[test]
fn test_q20_snapshot_consistency() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");

    // Multiple snapshots should be consistent
    let snap1 = sync.snapshot();
    let snap2 = sync.snapshot();

    assert_eq!(snap1.state, snap2.state);
    assert_eq!(snap1.waiter_count, snap2.waiter_count);
}

#[test]
fn test_q21_fence_type_consistency() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");
    assert_eq!(sync.sync_type(), SyncType::Fence);

    let sync2 = SyncPrimitiveCapsule::new(SyncType::Semaphore).expect("Failed to create semaphore");
    assert_eq!(sync2.sync_type(), SyncType::Semaphore);
}

// ============================================================================
// Q22-Q28: Production Tests (Stress, Performance, Edge Cases)
// ============================================================================

#[test]
fn test_q22_stress_signal_reset_cycles() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");

    for _ in 0..10_000 {
        sync.signal_fence().expect("Signal failed");
        assert!(sync.is_signaled());
        sync.reset().expect("Reset failed");
        assert!(!sync.is_signaled());
    }
}

#[test]
fn test_q23_1m_is_signaled_calls() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");
    sync.signal_fence().expect("Signal failed");

    let start = Instant::now();
    for _ in 0..1_000_000 {
        let _ = sync.is_signaled();
    }
    let elapsed = start.elapsed();

    // Should be < 10ms for 1M calls (< 10ns per call)
    println!("1M is_signaled() calls: {} ms", elapsed.as_millis());
    assert!(elapsed.as_millis() < 50);  // Relaxed to 50ms for CI
}

#[test]
fn test_q24_concurrent_stress() {
    let sync = Arc::new(SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence"));

    let mut handles = vec![];
    for _ in 0..10 {
        let sync_clone = sync.clone();
        handles.push(std::thread::spawn(move || {
            for _ in 0..1000 {
                let _ = sync_clone.snapshot();
            }
        }));
    }

    for handle in handles {
        handle.join().expect("Thread join failed");
    }
}

#[test]
fn test_q25_snapshot_after_operations() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");

    let snap1 = sync.snapshot();
    assert_eq!(snap1.state, 0);

    sync.signal_fence().expect("Signal failed");
    let snap2 = sync.snapshot();
    assert_eq!(snap2.state, 1);

    sync.reset().expect("Reset failed");
    let snap3 = sync.snapshot();
    assert_eq!(snap3.state, 0);
}

#[test]
fn test_q26_aba_prevention() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");

    // Generate multiple generations
    for _ in 0..10 {
        sync.signal_fence().expect("Signal failed");
        let gen1 = sync.snapshot().generation;
        sync.reset().expect("Reset failed");
        let gen2 = sync.snapshot().generation;
        assert_ne!(gen1, gen2);
    }
}

#[test]
fn test_q27_alignment_check() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Failed to create fence");
    let addr = &sync as *const _ as usize;
    assert_eq!(addr % 128, 0, "SyncPrimitiveCapsule not 128-byte aligned");
}

#[test]
fn test_q28_size_check() {
    use std::mem::size_of;
    assert_eq!(
        size_of::<SyncPrimitiveCapsule>(),
        128,
        "SyncPrimitiveCapsule size must be exactly 128 bytes"
    );
}

// ============================================================================
// Performance Benchmarks (B32 Framework)
// ============================================================================

#[test]
fn bench_signal_fence_performance() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Create failed");

    let start = Instant::now();
    for _ in 0..10_000 {
        sync.signal_fence().expect("Signal failed");
        sync.reset().expect("Reset failed");
    }
    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_nanos() / 20_000;  // 2 ops per iteration

    println!("signal_fence + reset: {} ns/op", ns_per_op);
    // Target: <50ns per signal (baseline: 300ns, 6× speedup)
}

#[test]
fn bench_is_signaled_hot() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Create failed");
    sync.signal_fence().expect("Signal failed");

    let start = Instant::now();
    for _ in 0..1_000_000 {
        let _ = sync.is_signaled();
    }
    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_nanos() / 1_000_000;

    println!("is_signaled (hot cache): {} ns/op", ns_per_op);
    // Target: <10ns (atomic load only)
}

#[test]
fn bench_reset_operation() {
    let start = Instant::now();
    let iterations = 100_000u64;

    for _ in 0..iterations {
        let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Create failed");
        sync.signal_fence().expect("Signal failed");
        sync.reset().expect("Reset failed");
    }
    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_nanos() / iterations as u128;

    println!("reset (with signal): {} ns/op", ns_per_op);
    // Target: <20ns pure reset
}

#[test]
fn bench_wait_uncontended() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Create failed");
    sync.signal_fence().expect("Signal failed");

    let start = Instant::now();
    for _ in 0..10_000 {
        let _ = sync.wait_fence(0);
    }
    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_nanos() / 10_000;

    println!("wait_fence (uncontended): {} ns/op", ns_per_op);
    // Target: <1μs uncontended (baseline: 10μs, 10× speedup)
}

#[test]
fn bench_snapshot_operations() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Create failed");
    sync.signal_fence().expect("Signal failed");

    let start = Instant::now();
    for _ in 0..1_000_000 {
        let _ = sync.snapshot();
    }
    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_nanos() / 1_000_000;

    println!("snapshot: {} ns/op", ns_per_op);
    // Target: <20ns (2 atomic loads)
}

#[test]
fn bench_throughput_query() {
    let sync = SyncPrimitiveCapsule::new(SyncType::Fence).expect("Create failed");
    sync.signal_fence().expect("Signal failed");

    let start = Instant::now();
    let iterations = 100_000_000u64;

    for _ in 0..iterations {
        let _ = sync.is_signaled();
    }

    let elapsed = start.elapsed().as_nanos() as f64;
    let ns_per_op = elapsed / iterations as f64;
    let ops_per_sec = 1e9 / ns_per_op;

    println!("Throughput: {:.0} M is_signaled() calls/sec", ops_per_sec / 1e6);
    // Target: >100M ops/sec
}
