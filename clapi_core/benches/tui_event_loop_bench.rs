//! TUI Event Loop Benchmarks - B32 Framework Compliance
//!
//! # Purpose
//! Measure honest event loop performance for TUI application.
//! All benchmarks follow B32 framework guidelines for fair, reproducible measurement.
//!
//! # B32 Compliance
//! - **Fair Baseline**: Compare against realistic event processing
//! - **Statistical Rigor**: 1000+ iterations, 95% CI via Criterion
//! - **Honest Claims**: <5ms per iteration (60 FPS target = 16ms budget)
//! - **Reality Check**: Terminal I/O dominates (5-10ms), not event processing
//!
//! # Benchmarks
//! 1. **Empty Event Loop**: Poll + no events (<1ms)
//! 2. **Key Event Handling**: Dispatch keyboard events (<100ns)
//! 3. **State Updates**: Atomic transitions (<50ns)
//! 4. **Full Iteration**: Complete event loop cycle (<5ms)
//!
//! # Performance Targets
//! - Empty poll: <1ms (crossterm event::poll overhead)
//! - Key dispatch: <100ns (atomic state updates)
//! - Full iteration: <5ms (97% headroom vs 16ms frame budget)
//!
//! # Build Instructions
//! ```bash
//! cargo bench --bench tui_event_loop_bench
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{Duration, Instant};

// ============================================================================
// MOCK TUI APP CAPSULE (Matches Production Pattern)
// ============================================================================

/// Application state enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum AppState {
    Running = 0,
    Paused = 1,
    Exiting = 2,
}

impl From<u8> for AppState {
    fn from(value: u8) -> Self {
        match value {
            0 => AppState::Running,
            1 => AppState::Paused,
            2 => AppState::Exiting,
            _ => AppState::Running,
        }
    }
}

/// TUI Application Capsule (64B, T1 Atomic)
#[repr(C, align(64))]
struct TuiAppCapsule {
    state: AtomicU8,            // Current app state
    should_quit: AtomicBool,    // Quit requested
    should_refresh: AtomicBool, // Refresh requested
    _padding: [u8; 61],         // Complete 64B cache line
}

impl TuiAppCapsule {
    fn new() -> Self {
        Self {
            state: AtomicU8::new(AppState::Running as u8),
            should_quit: AtomicBool::new(false),
            should_refresh: AtomicBool::new(true),
            _padding: [0; 61],
        }
    }

    #[inline(always)]
    fn state(&self) -> AppState {
        AppState::from(self.state.load(Ordering::Relaxed))
    }

    #[inline(always)]
    fn set_state(&self, new_state: AppState) {
        self.state.store(new_state as u8, Ordering::Release);
    }

    #[inline(always)]
    fn should_quit(&self) -> bool {
        self.should_quit.load(Ordering::Relaxed)
    }

    #[inline(always)]
    fn request_quit(&self) {
        self.should_quit.store(true, Ordering::Release);
        self.set_state(AppState::Exiting);
    }

    #[inline(always)]
    fn should_refresh(&self) -> bool {
        self.should_refresh.load(Ordering::Relaxed)
    }

    #[inline(always)]
    fn request_refresh(&self) {
        self.should_refresh.store(true, Ordering::Release);
    }

    #[inline(always)]
    fn clear_refresh(&self) {
        self.should_refresh.store(false, Ordering::Relaxed);
    }

    #[inline(always)]
    fn pause(&self) {
        self.set_state(AppState::Paused);
    }

    #[inline(always)]
    fn resume(&self) {
        self.set_state(AppState::Running);
        self.request_refresh();
    }
}

/// Mock key event (matches crossterm KeyCode)
#[derive(Debug, Clone, Copy)]
enum MockKeyCode {
    Char(char),
    Esc,
    Enter,
    Left,
    Right,
    Up,
    Down,
}

/// Mock key event handler (simulates production event handling)
fn handle_mock_key_event(app: &TuiAppCapsule, key: MockKeyCode) {
    match key {
        MockKeyCode::Char('q') | MockKeyCode::Esc => {
            app.request_quit();
        }
        MockKeyCode::Char('p') => {
            if app.state() == AppState::Running {
                app.pause();
            } else {
                app.resume();
            }
        }
        MockKeyCode::Char('r') => {
            app.request_refresh();
        }
        MockKeyCode::Enter => {
            // Execute command (no-op in mock)
            black_box(42);
        }
        MockKeyCode::Left | MockKeyCode::Right | MockKeyCode::Up | MockKeyCode::Down => {
            // Navigation (no-op in mock)
            black_box(42);
        }
        _ => {}
    }
}

// ============================================================================
// BENCHMARK 1: Event Loop State Checks
// ============================================================================

/// B32 Benchmark: Event loop state checks
///
/// # Purpose
/// Measure atomic load overhead for loop condition checks.
///
/// # Performance Target
/// - <10ns per check (atomic load, Relaxed ordering)
///
/// # B32 Reality Check
/// - Loop runs 60 times per second (16ms interval)
/// - State check overhead: <0.001% of loop iteration
fn bench_state_checks(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_loop/state_check");

    let app = TuiAppCapsule::new();

    group.bench_function("should_quit", |b| {
        b.iter(|| {
            black_box(app.should_quit());
        });
    });

    group.bench_function("should_refresh", |b| {
        b.iter(|| {
            black_box(app.should_refresh());
        });
    });

    group.bench_function("current_state", |b| {
        b.iter(|| {
            black_box(app.state());
        });
    });

    group.bench_function("all_checks", |b| {
        b.iter(|| {
            black_box((
                app.should_quit(),
                app.should_refresh(),
                app.state(),
            ));
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 2: Key Event Dispatch
// ============================================================================

/// B32 Benchmark: Keyboard event dispatch
///
/// # Purpose
/// Measure overhead of key event handling (atomic state updates).
///
/// # Performance Target
/// - <100ns per event (atomic updates + pattern matching)
///
/// # B32 Reality Check
/// - Human keystroke speed: ~50-200ms per key
/// - Event dispatch overhead: <0.1% of keystroke latency
fn bench_key_event_dispatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_loop/key_dispatch");

    let app = TuiAppCapsule::new();

    group.bench_function("quit_key", |b| {
        b.iter(|| {
            let app = TuiAppCapsule::new();
            handle_mock_key_event(&app, black_box(MockKeyCode::Char('q')));
        });
    });

    group.bench_function("pause_key", |b| {
        b.iter(|| {
            let app = TuiAppCapsule::new();
            handle_mock_key_event(&app, black_box(MockKeyCode::Char('p')));
        });
    });

    group.bench_function("refresh_key", |b| {
        b.iter(|| {
            let app = TuiAppCapsule::new();
            handle_mock_key_event(&app, black_box(MockKeyCode::Char('r')));
        });
    });

    group.bench_function("arrow_key", |b| {
        b.iter(|| {
            let app = TuiAppCapsule::new();
            handle_mock_key_event(&app, black_box(MockKeyCode::Left));
        });
    });

    group.bench_function("enter_key", |b| {
        b.iter(|| {
            let app = TuiAppCapsule::new();
            handle_mock_key_event(&app, black_box(MockKeyCode::Enter));
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 3: State Transitions
// ============================================================================

/// B32 Benchmark: Application state transitions
///
/// # Purpose
/// Measure atomic state machine transition overhead.
///
/// # Performance Target
/// - <50ns per transition (atomic store + refresh flag)
fn bench_state_transitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_loop/state_transition");

    let app = TuiAppCapsule::new();

    group.bench_function("pause", |b| {
        b.iter(|| {
            app.pause();
        });
    });

    group.bench_function("resume", |b| {
        b.iter(|| {
            app.resume();
        });
    });

    group.bench_function("request_quit", |b| {
        b.iter(|| {
            let app = TuiAppCapsule::new();
            app.request_quit();
        });
    });

    group.bench_function("toggle_pause_resume", |b| {
        b.iter(|| {
            app.pause();
            app.resume();
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 4: Refresh Flag Operations
// ============================================================================

/// B32 Benchmark: Refresh flag management
///
/// # Purpose
/// Measure atomic boolean flag overhead for frame rendering control.
///
/// # Performance Target
/// - <20ns per operation (atomic load/store)
fn bench_refresh_flag(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_loop/refresh_flag");

    let app = TuiAppCapsule::new();

    group.bench_function("request_refresh", |b| {
        b.iter(|| {
            app.request_refresh();
        });
    });

    group.bench_function("clear_refresh", |b| {
        b.iter(|| {
            app.clear_refresh();
        });
    });

    group.bench_function("check_should_refresh", |b| {
        b.iter(|| {
            black_box(app.should_refresh());
        });
    });

    group.bench_function("refresh_cycle", |b| {
        b.iter(|| {
            app.request_refresh();
            black_box(app.should_refresh());
            app.clear_refresh();
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 5: Event Loop Iteration Simulation
// ============================================================================

/// B32 Benchmark: Simulated event loop iteration
///
/// # Purpose
/// Measure complete event loop cycle WITHOUT terminal I/O.
///
/// # Performance Target
/// - <1ms per iteration (no events, no rendering)
///
/// # B32 Reality Check
/// - 60 FPS target = 16ms per frame
/// - Event processing budget: 5ms
/// - Rendering budget: 11ms
/// - This benchmark measures event processing only
fn bench_event_loop_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_loop/iteration");
    group.sample_size(50); // Reduce sample size for longer operations

    // Iteration with no events
    group.bench_function("no_events_no_render", |b| {
        b.iter(|| {
            let app = TuiAppCapsule::new();

            // Check quit condition
            black_box(app.should_quit());

            // No events to process

            // Check refresh (but don't render)
            if app.should_refresh() {
                app.clear_refresh();
            }
        });
    });

    // Iteration with key event
    group.bench_function("with_key_event_no_render", |b| {
        b.iter(|| {
            let app = TuiAppCapsule::new();

            // Check quit condition
            black_box(app.should_quit());

            // Process key event
            handle_mock_key_event(&app, MockKeyCode::Char('r'));

            // Check refresh
            if app.should_refresh() {
                app.clear_refresh();
            }
        });
    });

    // Multiple iterations (simulates 10 frames)
    group.bench_function("ten_iterations_no_events", |b| {
        b.iter(|| {
            let app = TuiAppCapsule::new();

            for _ in 0..10 {
                black_box(app.should_quit());
                if app.should_refresh() {
                    app.clear_refresh();
                }
            }
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 6: Frame Timing Simulation
// ============================================================================

/// B32 Benchmark: Frame timing overhead
///
/// # Purpose
/// Measure overhead of frame rate limiting logic.
///
/// # Performance Target
/// - <100ns per iteration (time measurement + sleep calculation)
///
/// # B32 Reality Check
/// - Frame budget: 16ms @ 60 FPS
/// - Timing overhead: <0.001% of frame time
fn bench_frame_timing(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_loop/frame_timing");

    let frame_duration = Duration::from_millis(16); // 60 FPS

    group.bench_function("timing_measurement", |b| {
        b.iter(|| {
            let frame_start = Instant::now();

            // Simulate work (no-op)
            black_box(42);

            let elapsed = frame_start.elapsed();
            black_box(elapsed);
        });
    });

    group.bench_function("sleep_calculation", |b| {
        b.iter(|| {
            let frame_start = Instant::now();

            // Simulate work
            black_box(42);

            let elapsed = frame_start.elapsed();
            if elapsed < frame_duration {
                let sleep_time = frame_duration - elapsed;
                black_box(sleep_time);
            }
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    event_loop_benches,
    bench_state_checks,
    bench_key_event_dispatch,
    bench_state_transitions,
    bench_refresh_flag,
    bench_event_loop_iteration,
    bench_frame_timing,
);

criterion_main!(event_loop_benches);
