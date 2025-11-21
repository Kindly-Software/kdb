//! # T28 Comprehensive Tests - LeaderElectionCapsule
//!
//! **Framework**: T28 (Q1-Q28) - 4 tiers of testing
//! **Target**: 28 tests (7 unit + 7 property + 7 integration + 7 production)
//!
//! ## Test Tiers
//! - **Q1-Q7 (Unit)**: Layout, alignment, basic operations
//! - **Q8-Q14 (Property)**: Invariants, generation counters, epoch monotonicity
//! - **Q15-Q21 (Integration)**: Multi-node coordination, failover scenarios
//! - **Q22-Q28 (Production)**: Stress tests, concurrent elections, performance validation

use atomic_capsule::patterns::{
    ElectionResult, LeaderElectionCapsule, LeaderInfo, LeaderState,
};
use core::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Q1-Q7: Unit Tests (Layout + Basic Operations)
// ============================================================================

#[test]
fn q1_test_layout_size() {
    assert_eq!(
        core::mem::size_of::<LeaderElectionCapsule>(),
        128,
        "Size must be 128 bytes (WarmTier)"
    );
}

#[test]
fn q2_test_layout_alignment() {
    assert_eq!(
        core::mem::align_of::<LeaderElectionCapsule>(),
        128,
        "Alignment must be 128 bytes (WarmTier)"
    );
}

#[test]
fn q3_test_new_default() {
    let election = LeaderElectionCapsule::new();
    assert_eq!(election.current_epoch(), 0, "Initial epoch should be 0");
    assert_eq!(
        election.current_state(),
        LeaderState::NoLeader,
        "Initial state should be NoLeader"
    );
    assert!(
        election.check_leader().is_none(),
        "No leader should exist initially"
    );
}

#[test]
fn q4_test_single_vote() {
    let election = LeaderElectionCapsule::new();
    let result = election.vote(1, 1);
    assert!(
        matches!(result, ElectionResult::BecameLeader { epoch: 1 }),
        "Node 1 should become leader"
    );

    let info = election.check_leader().expect("Leader should exist");
    assert_eq!(info.epoch, 1);
    assert_eq!(info.leader_id, 1);
    assert_eq!(info.state, LeaderState::LeaderActive);
}

#[test]
fn q5_test_check_leader_none() {
    let election = LeaderElectionCapsule::new();
    assert!(election.check_leader().is_none(), "No leader initially");
}

#[test]
fn q6_test_trigger_failover_basic() {
    let election = LeaderElectionCapsule::new();
    election.vote(1, 1);

    let new_epoch = election.trigger_failover();
    assert_eq!(new_epoch, 2, "Epoch should increment to 2");
    assert!(
        election.check_leader().is_none(),
        "Leader should be cleared after failover"
    );
}

#[test]
fn q7_test_mark_suspected() {
    let election = LeaderElectionCapsule::new();
    election.vote(1, 1);

    assert!(election.mark_suspected(), "Should transition to suspected");
    assert_eq!(
        election.current_state(),
        LeaderState::LeaderSuspected,
        "State should be LeaderSuspected"
    );
}

// ============================================================================
// Q8-Q14: Property Tests (Invariants + Epoch Monotonicity)
// ============================================================================

#[test]
fn q8_test_epoch_monotonic_increase() {
    let election = LeaderElectionCapsule::new();

    // Vote for epoch 1
    election.vote(1, 1);
    assert_eq!(election.current_epoch(), 1);

    // Vote for epoch 2
    election.vote(2, 2);
    assert_eq!(election.current_epoch(), 2);

    // Vote for epoch 3
    election.vote(3, 3);
    assert_eq!(election.current_epoch(), 3);

    // All epochs should increase monotonically
}

#[test]
fn q9_test_stale_epoch_rejected() {
    let election = LeaderElectionCapsule::new();

    // Establish epoch 5
    election.vote(1, 5);

    // Try to vote for epoch 3 (stale)
    let result = election.vote(2, 3);
    assert!(
        matches!(result, ElectionResult::StaleEpoch { current_epoch: 5 }),
        "Stale epoch should be rejected"
    );
}

#[test]
fn q10_test_leader_uniqueness_per_epoch() {
    let election = LeaderElectionCapsule::new();

    // Node 1 becomes leader for epoch 1
    let result1 = election.vote(1, 1);
    assert!(matches!(result1, ElectionResult::BecameLeader { epoch: 1 }));

    // Node 2 tries to become leader for same epoch
    let result2 = election.vote(2, 1);
    assert!(
        matches!(
            result2,
            ElectionResult::LeaderElsewhere {
                epoch: 1,
                leader_id: 1
            }
        ),
        "Only one leader per epoch"
    );
}

#[test]
fn q11_test_no_split_brain() {
    let election = Arc::new(LeaderElectionCapsule::new());
    let epoch = 1;

    // 4 nodes vote concurrently
    let handles: Vec<_> = (1..=4)
        .map(|node_id| {
            let election = Arc::clone(&election);
            thread::spawn(move || election.vote(node_id, epoch))
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Count leaders
    let leaders: Vec<_> = results
        .iter()
        .filter(|r| matches!(r, ElectionResult::BecameLeader { .. }))
        .collect();

    assert_eq!(
        leaders.len(),
        1,
        "Exactly one leader should be elected (no split-brain)"
    );
}

#[test]
fn q12_test_failover_epoch_increment() {
    let election = LeaderElectionCapsule::new();

    for i in 1..=10 {
        election.vote(i, i);
        let new_epoch = election.trigger_failover();
        assert_eq!(
            new_epoch,
            i + 1,
            "Failover should increment epoch by 1"
        );
    }
}

#[test]
fn q13_test_suspected_state_transition() {
    let election = LeaderElectionCapsule::new();

    // NoLeader -> cannot mark suspected
    assert!(!election.mark_suspected());

    // LeaderActive -> can mark suspected
    election.vote(1, 1);
    assert!(election.mark_suspected());
    assert_eq!(election.current_state(), LeaderState::LeaderSuspected);

    // LeaderSuspected -> cannot mark suspected again
    assert!(!election.mark_suspected());
}

#[test]
fn q14_test_epoch_overflow_safety() {
    let election = LeaderElectionCapsule::new();

    // Vote for near-max epoch (48-bit max)
    let near_max = LeaderElectionCapsule::MAX_EPOCH - 1;
    election.vote(1, near_max);

    // Trigger failover (should clamp at MAX_EPOCH)
    let new_epoch = election.trigger_failover();
    assert_eq!(
        new_epoch,
        LeaderElectionCapsule::MAX_EPOCH,
        "Epoch should clamp at MAX_EPOCH"
    );
}

// ============================================================================
// Q15-Q21: Integration Tests (Multi-Node Coordination)
// ============================================================================

#[test]
fn q15_test_multi_node_sequential() {
    let election = LeaderElectionCapsule::new();

    // 10 nodes vote sequentially, each for new epoch
    for i in 1..=10 {
        let result = election.vote(i, i);
        assert!(matches!(result, ElectionResult::BecameLeader { epoch: _ }));

        let info = election.check_leader().unwrap();
        assert_eq!(info.leader_id, i);
    }
}

#[test]
fn q16_test_concurrent_votes_same_epoch() {
    let election = Arc::new(LeaderElectionCapsule::new());
    let epoch = 1;

    // 16 nodes vote concurrently for epoch 1
    let handles: Vec<_> = (1..=16)
        .map(|node_id| {
            let election = Arc::clone(&election);
            thread::spawn(move || election.vote(node_id, epoch))
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Exactly one BecameLeader, others LeaderElsewhere or Contention
    let became_leader = results
        .iter()
        .filter(|r| matches!(r, ElectionResult::BecameLeader { .. }))
        .count();
    assert_eq!(became_leader, 1, "Exactly one node should become leader");

    // Check final leader state
    let info = election.check_leader().unwrap();
    assert_eq!(info.epoch, epoch);
}

#[test]
fn q17_test_failover_coordination() {
    let election = Arc::new(LeaderElectionCapsule::new());

    // Node 1 becomes leader
    election.vote(1, 1);

    // 8 nodes trigger failover concurrently
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let election = Arc::clone(&election);
            thread::spawn(move || election.trigger_failover())
        })
        .collect();

    let epochs: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // All epochs should be >= 2 (at least one increment)
    for epoch in epochs {
        assert!(epoch >= 2, "Epoch should be at least 2 after failover");
    }
}

#[test]
fn q18_test_mark_suspected_concurrent() {
    let election = Arc::new(LeaderElectionCapsule::new());
    election.vote(1, 1);

    // 8 threads mark suspected concurrently
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let election = Arc::clone(&election);
            thread::spawn(move || election.mark_suspected())
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // At least one should succeed
    let succeeded = results.iter().filter(|&&r| r).count();
    assert!(succeeded >= 1, "At least one mark_suspected should succeed");

    // Final state should be LeaderSuspected
    assert_eq!(election.current_state(), LeaderState::LeaderSuspected);
}

#[test]
fn q19_test_election_after_suspected() {
    let election = LeaderElectionCapsule::new();

    // Node 1 becomes leader
    election.vote(1, 1);

    // Mark suspected
    election.mark_suspected();

    // Node 2 votes for new epoch
    let result = election.vote(2, 2);
    assert!(matches!(result, ElectionResult::BecameLeader { epoch: 2 }));

    // Check new leader
    let info = election.check_leader().unwrap();
    assert_eq!(info.leader_id, 2);
    assert_eq!(info.state, LeaderState::LeaderActive);
}

#[test]
fn q20_test_epoch_jump() {
    let election = LeaderElectionCapsule::new();

    // Start at epoch 1
    election.vote(1, 1);

    // Jump to epoch 10 (simulating network partition recovery)
    let result = election.vote(2, 10);
    assert!(matches!(result, ElectionResult::BecameLeader { epoch: 10 }));

    // Verify epoch jump
    assert_eq!(election.current_epoch(), 10);
}

#[test]
fn q21_test_rapid_failover_sequence() {
    let election = LeaderElectionCapsule::new();

    // Simulate 100 rapid failovers
    for i in 1..=100 {
        election.vote(i, i);
        election.trigger_failover();
    }

    // Final epoch should be 101
    let final_epoch = election.current_epoch();
    assert_eq!(final_epoch, 101, "Epoch should be 101 after 100 failovers");
}

// ============================================================================
// Q22-Q28: Production Stress Tests
// ============================================================================

#[test]
fn q22_test_high_contention_election() {
    let election = Arc::new(LeaderElectionCapsule::new());
    let epoch = 1;

    // 64 nodes vote concurrently (high contention)
    let handles: Vec<_> = (1..=64)
        .map(|node_id| {
            let election = Arc::clone(&election);
            thread::spawn(move || election.vote(node_id, epoch))
        })
        .collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Exactly one leader
    let became_leader = results
        .iter()
        .filter(|r| matches!(r, ElectionResult::BecameLeader { .. }))
        .count();
    assert_eq!(
        became_leader, 1,
        "Exactly one leader under high contention"
    );
}

#[test]
fn q23_test_mixed_operations_concurrent() {
    let election = Arc::new(LeaderElectionCapsule::new());

    // Initial leader
    election.vote(1, 1);

    // Mixed operations: vote, check_leader, trigger_failover, mark_suspected
    let handles: Vec<_> = (0..32)
        .map(|i| {
            let election = Arc::clone(&election);
            thread::spawn(move || match i % 4 {
                0 => {
                    election.vote(i, i + 1);
                }
                1 => {
                    election.check_leader();
                }
                2 => {
                    election.trigger_failover();
                }
                _ => {
                    election.mark_suspected();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Should not panic or deadlock
}

#[test]
fn q24_test_sustained_load() {
    let election = Arc::new(LeaderElectionCapsule::new());
    let iterations = 10_000;

    // 8 threads, each performing 10K operations
    let handles: Vec<_> = (0..8)
        .map(|thread_id| {
            let election = Arc::clone(&election);
            thread::spawn(move || {
                for i in 0..iterations {
                    let epoch = (thread_id * iterations + i) as u64 + 1;
                    election.vote(thread_id + 1, epoch);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Verify final state is valid
    let info = election.check_leader();
    assert!(info.is_some() || election.current_state() == LeaderState::NoLeader);
}

#[test]
fn q25_test_epoch_progression_monotonic() {
    let election = Arc::new(LeaderElectionCapsule::new());
    let max_epoch = Arc::new(AtomicU64::new(0));

    // 16 threads, each voting for increasing epochs
    let handles: Vec<_> = (0..16)
        .map(|thread_id| {
            let election = Arc::clone(&election);
            let max_epoch = Arc::clone(&max_epoch);
            thread::spawn(move || {
                for i in 1..=1000 {
                    let epoch = (thread_id * 1000 + i) as u64;
                    election.vote(thread_id + 1, epoch);
                    max_epoch.fetch_max(epoch, Ordering::SeqCst);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Final epoch should be >= max observed epoch
    let final_epoch = election.current_epoch();
    let observed_max = max_epoch.load(Ordering::SeqCst);
    assert!(
        final_epoch >= observed_max,
        "Final epoch should be >= max observed"
    );
}

#[test]
fn q26_test_failover_storm() {
    let election = Arc::new(LeaderElectionCapsule::new());

    // Initial leader
    election.vote(1, 1);

    // 32 threads trigger failover concurrently 100 times each
    let handles: Vec<_> = (0..32)
        .map(|_| {
            let election = Arc::clone(&election);
            thread::spawn(move || {
                for _ in 0..100 {
                    election.trigger_failover();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Epoch should have increased significantly
    let final_epoch = election.current_epoch();
    assert!(final_epoch > 1, "Epoch should increase after failover storm");
}

#[test]
fn q27_test_check_leader_concurrent_reads() {
    let election = Arc::new(LeaderElectionCapsule::new());
    election.vote(1, 1);

    // 64 threads read leader info concurrently
    let handles: Vec<_> = (0..64)
        .map(|_| {
            let election = Arc::clone(&election);
            thread::spawn(move || {
                for _ in 0..10_000 {
                    let info = election.check_leader();
                    if let Some(leader) = info {
                        assert!(
                            leader.leader_id > 0,
                            "Leader ID should be valid"
                        );
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn q28_test_production_simulation() {
    let election = Arc::new(LeaderElectionCapsule::new());

    // Simulate 3 nodes in production:
    // - Node 1: Leader (periodic heartbeat checks)
    // - Node 2: Follower (monitors leader)
    // - Node 3: Observer (reads leader state)

    let handles: Vec<_> = (1..=3)
        .map(|node_id| {
            let election = Arc::clone(&election);
            thread::spawn(move || match node_id {
                1 => {
                    // Leader: Maintain leadership
                    election.vote(1, 1);
                    for _ in 0..100 {
                        election.check_leader();
                        thread::sleep(std::time::Duration::from_micros(10));
                    }
                }
                2 => {
                    // Follower: Monitor and trigger failover after detecting leader loss
                    thread::sleep(std::time::Duration::from_millis(1));
                    if election.check_leader().is_some() {
                        election.mark_suspected();
                        thread::sleep(std::time::Duration::from_micros(100));
                        election.trigger_failover();
                        election.vote(2, 2);
                    }
                }
                3 => {
                    // Observer: Read leader state
                    for _ in 0..1000 {
                        election.check_leader();
                    }
                }
                _ => unreachable!(),
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Verify final state is consistent
    let info = election.check_leader();
    assert!(info.is_some() || election.current_state() == LeaderState::NoLeader);
}
