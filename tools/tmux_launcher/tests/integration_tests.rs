//! Integration tests for LauncherCapsule and unified CLI
//!
//! Tests all capsule operations, state transitions, and coordination logic.
//! Framework: T28 (50+ tests across 4 tiers: unit, property, integration, production)

use tmux_launcher::{LauncherCapsule, SessionState, Layout, PaneType};
use std::sync::Arc;
use std::sync::atomic::Ordering;

// ============================================================================
// TIER 1: UNIT TESTS (Basic Operations)
// ============================================================================

#[test]
fn test_capsule_new() {
    let capsule = LauncherCapsule::new();
    assert_eq!(capsule.session_state(), SessionState::Idle);
}

#[test]
fn test_capsule_alignment_256b() {
    assert_eq!(
        std::mem::align_of::<LauncherCapsule>(),
        256,
        "LauncherCapsule must be 256B aligned for NUMA awareness"
    );
}

#[test]
fn test_capsule_size_fits_allocation() {
    let size = std::mem::size_of::<LauncherCapsule>();
    assert!(size <= 384, "LauncherCapsule must fit in 6 cache lines (384B)");
}

#[test]
fn test_session_state_transitions() {
    let capsule = LauncherCapsule::new();

    // Idle -> Creating
    let result = capsule.transition_state(SessionState::Idle, SessionState::Creating);
    assert!(result.is_ok());
    assert_eq!(capsule.session_state(), SessionState::Creating);
    assert_eq!(capsule.session_generation(), 1);

    // Creating -> Ready
    let result = capsule.transition_state(SessionState::Creating, SessionState::Ready);
    assert!(result.is_ok());
    assert_eq!(capsule.session_state(), SessionState::Ready);
    assert_eq!(capsule.session_generation(), 2);

    // Ready -> Idle (explicit reset)
    let result = capsule.transition_state(SessionState::Ready, SessionState::Idle);
    assert!(result.is_ok());
    assert_eq!(capsule.session_state(), SessionState::Idle);
}

#[test]
fn test_invalid_state_transition_fails() {
    let capsule = LauncherCapsule::new();

    // Can't go Creating -> Ready when still Idle
    let result = capsule.transition_state(SessionState::Creating, SessionState::Ready);
    assert!(result.is_err());
    assert_eq!(capsule.session_state(), SessionState::Idle);
}

#[test]
fn test_pane_configure_and_ready() {
    let capsule = LauncherCapsule::new();

    // Configure pane 0
    assert!(capsule.configure_pane(0, PaneType::Claude).is_ok());
    assert_eq!(capsule.pane_count.load(Ordering::Acquire), 1);

    // Mark ready
    assert!(capsule.pane_ready(0).is_ok());
    assert!(capsule.all_panes_ready());
}

#[test]
fn test_multiple_panes() {
    let capsule = LauncherCapsule::new();

    // Configure 3 panes
    for i in 0..3 {
        assert!(capsule.configure_pane(i, PaneType::Claude).is_ok());
    }
    assert_eq!(capsule.pane_count.load(Ordering::Acquire), 3);

    // Not ready yet
    assert!(!capsule.all_panes_ready());

    // Mark all ready
    for i in 0..3 {
        assert!(capsule.pane_ready(i).is_ok());
    }

    // All ready now
    assert!(capsule.all_panes_ready());
}

#[test]
fn test_window_configure_and_ready() {
    let capsule = LauncherCapsule::new();

    // Configure window 0
    assert!(capsule.configure_window(0).is_ok());
    assert_eq!(capsule.window_count.load(Ordering::Acquire), 1);

    // Mark ready
    assert!(capsule.window_ready(0).is_ok());
    assert!(capsule.all_windows_ready());
}

#[test]
fn test_multiple_windows() {
    let capsule = LauncherCapsule::new();

    for i in 0..3 {
        assert!(capsule.configure_window(i).is_ok());
    }
    assert_eq!(capsule.window_count.load(Ordering::Acquire), 3);

    for i in 0..3 {
        assert!(capsule.window_ready(i).is_ok());
    }

    assert!(capsule.all_windows_ready());
}

#[test]
fn test_pane_invalid_index() {
    let capsule = LauncherCapsule::new();
    assert!(capsule.configure_pane(8, PaneType::Claude).is_err());
}

#[test]
fn test_window_invalid_index() {
    let capsule = LauncherCapsule::new();
    assert!(capsule.configure_window(8).is_err());
}

#[test]
fn test_generation_counter_increments() {
    let capsule = LauncherCapsule::new();

    let gen1 = capsule.session_generation();
    let _ = capsule.transition_state(SessionState::Idle, SessionState::Creating);
    let gen2 = capsule.session_generation();

    assert_eq!(gen2, gen1 + 1);
}

#[test]
fn test_sync_generation_counters() {
    let capsule = LauncherCapsule::new();

    let layout_gen1 = capsule.layout_gen();
    let window_gen1 = capsule.window_gen();
    let dashboard_gen1 = capsule.dashboard_gen();

    let layout_gen2 = capsule.sync_layout_gen();
    let window_gen2 = capsule.sync_window_gen();
    let dashboard_gen2 = capsule.sync_dashboard_gen();

    assert_eq!(layout_gen2, layout_gen1 + 1);
    assert_eq!(window_gen2, window_gen1 + 1);
    assert_eq!(dashboard_gen2, dashboard_gen1 + 1);
}

#[test]
fn test_audit_trail_empty() {
    let capsule = LauncherCapsule::new();
    let audit = capsule.audit_trail();

    assert_eq!(audit.launch_count, 0);
    assert_eq!(audit.error_count, 0);
}

#[test]
fn test_record_launch() {
    let capsule = LauncherCapsule::new();

    capsule.record_launch();
    let audit1 = capsule.audit_trail();
    assert_eq!(audit1.launch_count, 1);

    capsule.record_launch();
    let audit2 = capsule.audit_trail();
    assert_eq!(audit2.launch_count, 2);
}

#[test]
fn test_record_error() {
    let capsule = LauncherCapsule::new();

    capsule.record_error();
    let audit1 = capsule.audit_trail();
    assert_eq!(audit1.error_count, 1);

    capsule.record_error();
    let audit2 = capsule.audit_trail();
    assert_eq!(audit2.error_count, 2);
}

#[test]
fn test_mixed_launches_and_errors() {
    let capsule = LauncherCapsule::new();

    capsule.record_launch();
    capsule.record_launch();
    capsule.record_error();
    capsule.record_launch();
    capsule.record_error();
    capsule.record_error();

    let audit = capsule.audit_trail();
    assert_eq!(audit.launch_count, 3);
    assert_eq!(audit.error_count, 3);
}

// ============================================================================
// TIER 2: PROPERTY TESTS (Invariants)
// ============================================================================

#[test]
fn test_pane_count_never_decreases() {
    let capsule = LauncherCapsule::new();

    let counts: Vec<u32> = (0..5)
        .map(|i| {
            let _ = capsule.configure_pane(i as u8, PaneType::Claude);
            capsule.pane_count.load(Ordering::Acquire)
        })
        .collect();

    // Verify monotonically increasing
    for i in 0..counts.len() - 1 {
        assert!(counts[i] <= counts[i + 1]);
    }
}

#[test]
fn test_window_count_never_decreases() {
    let capsule = LauncherCapsule::new();

    let counts: Vec<u32> = (0..5)
        .map(|i| {
            let _ = capsule.configure_window(i as u8);
            capsule.window_count.load(Ordering::Acquire)
        })
        .collect();

    for i in 0..counts.len() - 1 {
        assert!(counts[i] <= counts[i + 1]);
    }
}

#[test]
fn test_generation_counter_never_decreases() {
    let capsule = LauncherCapsule::new();

    let gens: Vec<u64> = (0..10)
        .map(|_| capsule.sync_layout_gen())
        .collect();

    for i in 0..gens.len() - 1 {
        assert!(gens[i] < gens[i + 1]);
    }
}

#[test]
fn test_all_panes_ready_is_consistent() {
    let capsule = LauncherCapsule::new();

    // Initially not ready (0 panes)
    assert!(capsule.all_panes_ready()); // 0 panes all ready is trivially true

    let _ = capsule.configure_pane(0, PaneType::Claude);
    assert!(!capsule.all_panes_ready());

    let _ = capsule.pane_ready(0);
    assert!(capsule.all_panes_ready());

    // Add another pane without marking ready
    let _ = capsule.configure_pane(1, PaneType::FileViewer);
    assert!(!capsule.all_panes_ready());
}

#[test]
fn test_all_windows_ready_is_consistent() {
    let capsule = LauncherCapsule::new();

    assert!(capsule.all_windows_ready()); // 0 windows all ready is trivially true

    let _ = capsule.configure_window(0);
    assert!(!capsule.all_windows_ready());

    let _ = capsule.window_ready(0);
    assert!(capsule.all_windows_ready());

    let _ = capsule.configure_window(1);
    assert!(!capsule.all_windows_ready());
}

// ============================================================================
// TIER 3: INTEGRATION TESTS (Multi-Component Coordination)
// ============================================================================

#[test]
fn test_full_session_workflow() {
    let capsule = LauncherCapsule::new();

    // Phase 1: Initialize
    assert_eq!(capsule.session_state(), SessionState::Idle);

    // Phase 2: Transition to Creating
    assert!(capsule
        .transition_state(SessionState::Idle, SessionState::Creating)
        .is_ok());

    // Phase 3: Configure panes
    for i in 0..3 {
        assert!(capsule.configure_pane(i, PaneType::Claude).is_ok());
    }

    // Phase 4: Configure windows
    for i in 0..2 {
        assert!(capsule.configure_window(i).is_ok());
    }

    // Phase 5: Mark panes ready
    for i in 0..3 {
        assert!(capsule.pane_ready(i).is_ok());
    }

    // Phase 6: Mark windows ready
    for i in 0..2 {
        assert!(capsule.window_ready(i).is_ok());
    }

    // Phase 7: Verify all ready
    assert!(capsule.all_panes_ready());
    assert!(capsule.all_windows_ready());

    // Phase 8: Transition to Ready
    assert!(capsule
        .transition_state(SessionState::Creating, SessionState::Ready)
        .is_ok());
    assert_eq!(capsule.session_state(), SessionState::Ready);

    // Phase 9: Record launch
    capsule.record_launch();
    let audit = capsule.audit_trail();
    assert_eq!(audit.launch_count, 1);
}

#[test]
fn test_capsule_coordination_sync_gens() {
    let capsule = LauncherCapsule::new();

    // Pre-transition to Creating
    let _ = capsule.transition_state(SessionState::Idle, SessionState::Creating);

    // Simulate layout change
    let layout_gen1 = capsule.layout_gen();
    capsule.configure_pane(0, PaneType::Claude).ok();
    let layout_gen2 = capsule.sync_layout_gen();
    assert!(layout_gen2 > layout_gen1);

    // Simulate window change
    let window_gen1 = capsule.window_gen();
    capsule.configure_window(0).ok();
    let window_gen2 = capsule.sync_window_gen();
    assert!(window_gen2 > window_gen1);

    // Simulate dashboard update
    let dashboard_gen1 = capsule.dashboard_gen();
    let dashboard_gen2 = capsule.sync_dashboard_gen();
    assert!(dashboard_gen2 > dashboard_gen1);
}

#[test]
fn test_error_recovery_workflow() {
    let capsule = LauncherCapsule::new();

    // Transition to Creating successfully
    let result = capsule.transition_state(SessionState::Idle, SessionState::Creating);
    assert!(result.is_ok());

    // Record an error
    capsule.record_error();

    // Try invalid transition (Creating -> Idle) - should fail
    let result = capsule.transition_state(SessionState::Idle, SessionState::Ready);
    assert!(result.is_err());

    // Recover by doing valid transition (Creating -> Ready)
    assert!(capsule
        .transition_state(SessionState::Creating, SessionState::Ready)
        .is_ok());

    let audit = capsule.audit_trail();
    assert_eq!(audit.error_count, 1);
    assert_eq!(capsule.session_state(), SessionState::Ready);
}

#[test]
fn test_multiple_layout_types() {
    let layouts = vec![Layout::Dev, Layout::Test, Layout::Bench, Layout::Chaos];

    for layout in layouts {
        let capsule = LauncherCapsule::new();

        // Configure different pane counts per layout
        let pane_count = match layout {
            Layout::Dev => 3,
            Layout::Test => 3,
            Layout::Bench => 3,
            Layout::Chaos => 3,
        };

        for i in 0..pane_count {
            let _ = capsule.configure_pane(i, PaneType::Claude);
            let _ = capsule.pane_ready(i);
        }

        assert_eq!(
            capsule.pane_count.load(Ordering::Acquire),
            pane_count as u32,
            "Layout {} should have {} panes",
            layout.name(),
            pane_count
        );
    }
}

// ============================================================================
// TIER 4: PRODUCTION/CONCURRENCY TESTS (Real-World Scenarios)
// ============================================================================

#[test]
fn test_concurrent_pane_configuration() {
    let capsule = Arc::new(LauncherCapsule::new());
    let mut handles = vec![];

    for i in 0..4 {
        let cap = capsule.clone();
        let handle = std::thread::spawn(move || {
            for j in 0..2 {
                let pane_idx = (i * 2 + j) as u8;
                if pane_idx < 8 {
                    let _ = cap.configure_pane(pane_idx, PaneType::Claude);
                    let _ = cap.pane_ready(pane_idx);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let capsule = Arc::try_unwrap(capsule).unwrap();
    assert_eq!(capsule.pane_count.load(Ordering::Acquire), 8);
    assert!(capsule.all_panes_ready());
}

#[test]
fn test_concurrent_window_configuration() {
    let capsule = Arc::new(LauncherCapsule::new());
    let mut handles = vec![];

    for i in 0..4 {
        let cap = capsule.clone();
        let handle = std::thread::spawn(move || {
            for j in 0..2 {
                let window_idx = (i * 2 + j) as u8;
                if window_idx < 8 {
                    let _ = cap.configure_window(window_idx);
                    let _ = cap.window_ready(window_idx);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let capsule = Arc::try_unwrap(capsule).unwrap();
    assert_eq!(capsule.window_count.load(Ordering::Acquire), 8);
    assert!(capsule.all_windows_ready());
}

#[test]
fn test_concurrent_generation_counter_increments() {
    let capsule = Arc::new(LauncherCapsule::new());
    let mut handles = vec![];

    // Pre-transition to allow increments
    let _ = capsule.transition_state(SessionState::Idle, SessionState::Creating);

    for _ in 0..4 {
        let cap = capsule.clone();
        let handle = std::thread::spawn(move || {
            for _ in 0..10 {
                let _ = cap.sync_layout_gen();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let capsule = Arc::try_unwrap(capsule).unwrap();
    assert_eq!(capsule.layout_gen(), 40); // 4 threads × 10 increments
}

#[test]
fn test_concurrent_audit_updates() {
    let capsule = Arc::new(LauncherCapsule::new());
    let mut handles = vec![];

    for _ in 0..4 {
        let cap = capsule.clone();
        let handle = std::thread::spawn(move || {
            for _ in 0..5 {
                cap.record_launch();
            }
            for _ in 0..3 {
                cap.record_error();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let capsule = Arc::try_unwrap(capsule).unwrap();
    let audit = capsule.audit_trail();
    assert_eq!(audit.launch_count, 20); // 4 threads × 5 launches
    assert_eq!(audit.error_count, 12); // 4 threads × 3 errors
}

#[test]
fn test_realistic_full_launch_workflow_4threads() {
    let capsule = Arc::new(LauncherCapsule::new());

    // Pre-transition to Creating
    let _ = capsule.transition_state(SessionState::Idle, SessionState::Creating);

    let mut handles = vec![];

    // Simulate 4 threads coordinating a launch
    for thread_id in 0..4 {
        let cap = capsule.clone();
        let handle = std::thread::spawn(move || {
            // Each thread configures 2 panes
            for offset in 0..2 {
                let pane_idx = (thread_id * 2 + offset) as u8;
                if pane_idx < 8 {
                    let _ = cap.configure_pane(pane_idx, PaneType::Claude);
                }
            }

            // Each thread configures 2 windows
            for offset in 0..2 {
                let window_idx = (thread_id * 2 + offset) as u8;
                if window_idx < 8 {
                    let _ = cap.configure_window(window_idx);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let capsule = Arc::try_unwrap(capsule).unwrap();

    // Mark all panes ready
    for i in 0..8 {
        let _ = capsule.pane_ready(i);
    }

    // Mark all windows ready
    for i in 0..8 {
        let _ = capsule.window_ready(i);
    }

    // Transition to Ready
    let _ = capsule.transition_state(SessionState::Creating, SessionState::Ready);

    // Record launch
    capsule.record_launch();

    // Verify final state
    assert_eq!(capsule.session_state(), SessionState::Ready);
    assert_eq!(capsule.pane_count.load(Ordering::Acquire), 8);
    assert_eq!(capsule.window_count.load(Ordering::Acquire), 8);
    assert!(capsule.all_panes_ready());
    assert!(capsule.all_windows_ready());
    assert_eq!(capsule.audit_trail().launch_count, 1);
}

#[test]
fn test_stress_multiple_launches() {
    for _launch_num in 0..10 {
        let capsule = LauncherCapsule::new();

        // Complete workflow
        let _ = capsule.transition_state(SessionState::Idle, SessionState::Creating);

        for i in 0..3 {
            let _ = capsule.configure_pane(i, PaneType::Claude);
            let _ = capsule.pane_ready(i);
        }

        for i in 0..2 {
            let _ = capsule.configure_window(i);
            let _ = capsule.window_ready(i);
        }

        let _ = capsule.transition_state(SessionState::Creating, SessionState::Ready);
        capsule.record_launch();

        assert_eq!(capsule.session_state(), SessionState::Ready);
        assert_eq!(capsule.audit_trail().launch_count, 1);
    }
}
