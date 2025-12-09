//! LauncherCapsule Performance Benchmarks (B32 Framework)
//!
//! Measures fundamental capsule operations to validate performance claims.
//! Uses Criterion.rs with 1000+ iterations and 95% confidence intervals.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::sync::Arc;
use tmux_launcher::{LauncherCapsule, SessionState, ComponentState, Layout};

// ============================================================================
// Micro-benchmarks: Fundamental Operations
// ============================================================================

fn bench_new_capsule(c: &mut Criterion) {
    c.bench_function("new_capsule", |b| {
        b.iter(|| {
            black_box(LauncherCapsule::new());
        });
    });
}

fn bench_session_state_read(c: &mut Criterion) {
    let capsule = LauncherCapsule::new();
    c.bench_function("session_state_read", |b| {
        b.iter(|| {
            black_box(capsule.session_state());
        });
    });
}

fn bench_session_generation_read(c: &mut Criterion) {
    let capsule = LauncherCapsule::new();
    c.bench_function("session_generation_read", |b| {
        b.iter(|| {
            black_box(capsule.session_generation());
        });
    });
}

fn bench_state_transition(c: &mut Criterion) {
    c.bench_function("state_transition_idle_to_creating", |b| {
        b.iter(|| {
            let capsule = black_box(LauncherCapsule::new());
            let _ = capsule.transition_state(SessionState::Idle, SessionState::Creating);
        });
    });
}

fn bench_pane_configuration(c: &mut Criterion) {
    let capsule = LauncherCapsule::new();
    c.bench_function("pane_configuration", |b| {
        b.iter(|| {
            let _ = capsule.configure_pane(black_box(0), black_box(tmux_launcher::PaneType::Claude));
        });
    });
}

fn bench_pane_ready(c: &mut Criterion) {
    let capsule = LauncherCapsule::new();
    let _ = capsule.configure_pane(0, tmux_launcher::PaneType::Claude);

    c.bench_function("pane_ready", |b| {
        b.iter(|| {
            let _ = capsule.pane_ready(black_box(0));
        });
    });
}

fn bench_all_panes_ready_check(c: &mut Criterion) {
    let capsule = LauncherCapsule::new();
    for i in 0..3 {
        let _ = capsule.configure_pane(i, tmux_launcher::PaneType::Claude);
        let _ = capsule.pane_ready(i);
    }

    c.bench_function("all_panes_ready_check_3panes", |b| {
        b.iter(|| {
            black_box(capsule.all_panes_ready());
        });
    });
}

fn bench_window_configuration(c: &mut Criterion) {
    let capsule = LauncherCapsule::new();
    c.bench_function("window_configuration", |b| {
        b.iter(|| {
            let _ = capsule.configure_window(black_box(0));
        });
    });
}

fn bench_window_ready(c: &mut Criterion) {
    let capsule = LauncherCapsule::new();
    let _ = capsule.configure_window(0);

    c.bench_function("window_ready", |b| {
        b.iter(|| {
            let _ = capsule.window_ready(black_box(0));
        });
    });
}

fn bench_all_windows_ready_check(c: &mut Criterion) {
    let capsule = LauncherCapsule::new();
    for i in 0..3 {
        let _ = capsule.configure_window(i);
        let _ = capsule.window_ready(i);
    }

    c.bench_function("all_windows_ready_check_3windows", |b| {
        b.iter(|| {
            black_box(capsule.all_windows_ready());
        });
    });
}

fn bench_sync_layout_gen(c: &mut Criterion) {
    let capsule = LauncherCapsule::new();
    c.bench_function("sync_layout_gen", |b| {
        b.iter(|| {
            black_box(capsule.sync_layout_gen());
        });
    });
}

fn bench_sync_window_gen(c: &mut Criterion) {
    let capsule = LauncherCapsule::new();
    c.bench_function("sync_window_gen", |b| {
        b.iter(|| {
            black_box(capsule.sync_window_gen());
        });
    });
}

fn bench_sync_dashboard_gen(c: &mut Criterion) {
    let capsule = LauncherCapsule::new();
    c.bench_function("sync_dashboard_gen", |b| {
        b.iter(|| {
            black_box(capsule.sync_dashboard_gen());
        });
    });
}

fn bench_record_launch(c: &mut Criterion) {
    let capsule = LauncherCapsule::new();
    c.bench_function("record_launch", |b| {
        b.iter(|| {
            capsule.record_launch();
        });
    });
}

fn bench_record_error(c: &mut Criterion) {
    let capsule = LauncherCapsule::new();
    c.bench_function("record_error", |b| {
        b.iter(|| {
            capsule.record_error();
        });
    });
}

fn bench_audit_trail_read(c: &mut Criterion) {
    let capsule = LauncherCapsule::new();
    capsule.record_launch();
    capsule.record_error();

    c.bench_function("audit_trail_read", |b| {
        b.iter(|| {
            black_box(capsule.audit_trail());
        });
    });
}

// ============================================================================
// Macro-benchmarks: Coordinated Operations
// ============================================================================

fn bench_full_pane_setup(c: &mut Criterion) {
    c.bench_function("full_pane_setup_3panes", |b| {
        b.iter(|| {
            let capsule = black_box(LauncherCapsule::new());
            for i in 0..3 {
                let _ = capsule.configure_pane(i, tmux_launcher::PaneType::Claude);
                let _ = capsule.pane_ready(i);
            }
        });
    });
}

fn bench_full_window_setup(c: &mut Criterion) {
    c.bench_function("full_window_setup_3windows", |b| {
        b.iter(|| {
            let capsule = black_box(LauncherCapsule::new());
            for i in 0..3 {
                let _ = capsule.configure_window(i);
                let _ = capsule.window_ready(i);
            }
        });
    });
}

fn bench_full_session_orchestration(c: &mut Criterion) {
    c.bench_function("full_session_orchestration", |b| {
        b.iter(|| {
            let capsule = black_box(LauncherCapsule::new());

            // Transition to Creating
            let _ = capsule.transition_state(SessionState::Idle, SessionState::Creating);

            // Configure 3 panes
            for i in 0..3 {
                let _ = capsule.configure_pane(i, tmux_launcher::PaneType::Claude);
                let _ = capsule.pane_ready(i);
            }

            // Configure 3 windows
            for i in 0..3 {
                let _ = capsule.configure_window(i);
                let _ = capsule.window_ready(i);
            }

            // Sync with other capsules
            let _ = capsule.sync_layout_gen();
            let _ = capsule.sync_window_gen();
            let _ = capsule.sync_dashboard_gen();

            // Transition to Ready
            let _ = capsule.transition_state(SessionState::Creating, SessionState::Ready);

            // Record launch
            capsule.record_launch();
        });
    });
}

// ============================================================================
// Concurrent Operation Benchmarks
// ============================================================================

fn bench_concurrent_pane_configuration(c: &mut Criterion) {
    c.bench_function("concurrent_pane_config_4threads", |b| {
        b.iter(|| {
            let capsule = Arc::new(black_box(LauncherCapsule::new()));
            let mut handles = vec![];

            for i in 0..4 {
                let cap = capsule.clone();
                let handle = std::thread::spawn(move || {
                    let _ = cap.configure_pane(i as u8, tmux_launcher::PaneType::Claude);
                    let _ = cap.pane_ready(i as u8);
                });
                handles.push(handle);
            }

            for handle in handles {
                let _ = handle.join();
            }
        });
    });
}

fn bench_concurrent_state_transitions(c: &mut Criterion) {
    c.bench_function("concurrent_generation_increments_4threads", |b| {
        b.iter(|| {
            let capsule = Arc::new(black_box(LauncherCapsule::new()));
            let mut handles = vec![];

            // Pre-transition to Creating so all threads can increment
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
                let _ = handle.join();
            }
        });
    });
}

// ============================================================================
// Criterion Groups and Main
// ============================================================================

criterion_group!(
    benches,
    // Micro-benchmarks
    bench_new_capsule,
    bench_session_state_read,
    bench_session_generation_read,
    bench_state_transition,
    bench_pane_configuration,
    bench_pane_ready,
    bench_all_panes_ready_check,
    bench_window_configuration,
    bench_window_ready,
    bench_all_windows_ready_check,
    bench_sync_layout_gen,
    bench_sync_window_gen,
    bench_sync_dashboard_gen,
    bench_record_launch,
    bench_record_error,
    bench_audit_trail_read,
    // Macro-benchmarks
    bench_full_pane_setup,
    bench_full_window_setup,
    bench_full_session_orchestration,
    // Concurrent
    bench_concurrent_pane_configuration,
    bench_concurrent_state_transitions,
);

criterion_main!(benches);
