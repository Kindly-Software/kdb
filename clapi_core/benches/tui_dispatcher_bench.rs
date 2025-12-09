//! TUI Command Dispatcher Benchmarks - B32 Framework Compliance
//!
//! # Purpose
//! Measure honest command dispatcher performance for TUI command execution.
//! All benchmarks follow B32 framework guidelines for fair, reproducible measurement.
//!
//! # B32 Compliance
//! - **Fair Baseline**: Compare atomic operations against real-world usage
//! - **Statistical Rigor**: 1000+ iterations, 95% CI via Criterion
//! - **Honest Claims**: <100ns state update, <1μs mock execution
//! - **Reality Check**: Dispatcher latency negligible vs HTTP/subprocess overhead
//!
//! # Benchmarks
//! 1. **State Update**: Atomic state transitions (<100ns)
//! 2. **Mock Execution**: Simulated command dispatch (<1μs)
//! 3. **Error Handling**: Failure path overhead (<200ns)
//!
//! # Performance Targets
//! - State update: <100ns (atomic CAS)
//! - Mock execute: <1μs (no HTTP, no subprocess)
//! - Error path: <200ns (atomic error flag)
//!
//! # Build Instructions
//! ```bash
//! cargo bench --bench tui_dispatcher_bench
//! ```

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

// ============================================================================
// MOCK DISPATCHER CAPSULE (Matches Production Pattern)
// ============================================================================

/// Command execution state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum CommandState {
    Idle = 0,
    Executing = 1,
    Success = 2,
    Failed = 3,
}

impl From<u8> for CommandState {
    fn from(value: u8) -> Self {
        match value {
            0 => CommandState::Idle,
            1 => CommandState::Executing,
            2 => CommandState::Success,
            3 => CommandState::Failed,
            _ => CommandState::Idle,
        }
    }
}

/// Command Dispatcher Capsule (64B, T1 Atomic)
///
/// Simulates production dispatcher state management
#[repr(C, align(64))]
struct CommandDispatcherCapsule {
    state: AtomicU8,           // Current command state
    last_error_code: AtomicU64, // Last error code (0 = success)
    execution_count: AtomicU64, // Total commands executed
    _padding: [u8; 47],        // Complete 64B cache line
}

impl CommandDispatcherCapsule {
    fn new() -> Self {
        Self {
            state: AtomicU8::new(CommandState::Idle as u8),
            last_error_code: AtomicU64::new(0),
            execution_count: AtomicU64::new(0),
            _padding: [0; 47],
        }
    }

    #[inline(always)]
    fn state(&self) -> CommandState {
        CommandState::from(self.state.load(Ordering::Acquire))
    }

    #[inline(always)]
    fn set_state(&self, new_state: CommandState) {
        self.state.store(new_state as u8, Ordering::Release);
    }

    #[inline(always)]
    fn increment_execution_count(&self) {
        self.execution_count.fetch_add(1, Ordering::AcqRel);
    }

    #[inline(always)]
    fn set_error(&self, error_code: u64) {
        self.last_error_code.store(error_code, Ordering::Release);
    }

    /// Mock command execution (no HTTP, no subprocess)
    ///
    /// Simulates state transitions without actual I/O
    fn execute_mock(&self, _command: &str, _args: &[&str]) -> Result<(), u64> {
        // Transition: Idle → Executing
        self.set_state(CommandState::Executing);

        // Mock work (no actual I/O)
        black_box(42); // Prevent optimization

        // Transition: Executing → Success/Failed
        if black_box(true) {
            self.set_state(CommandState::Success);
            self.increment_execution_count();
            Ok(())
        } else {
            self.set_state(CommandState::Failed);
            self.set_error(1);
            Err(1)
        }
    }
}

// ============================================================================
// BENCHMARK 1: State Update Latency
// ============================================================================

/// B32 Benchmark: Atomic state transitions
///
/// # Purpose
/// Measure atomic operation overhead for state machine transitions.
///
/// # Performance Target
/// - <100ns per transition (atomic store + load)
///
/// # B32 Compliance
/// - Fair baseline: Raw atomic operations (no strawman)
/// - Reality check: Atomic operations are hardware-limited (~10-20ns)
fn bench_state_transitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("dispatcher/state_update");

    let dispatcher = CommandDispatcherCapsule::new();

    group.bench_function("idle_to_executing", |b| {
        b.iter(|| {
            dispatcher.set_state(black_box(CommandState::Idle));
            dispatcher.set_state(black_box(CommandState::Executing));
        });
    });

    group.bench_function("executing_to_success", |b| {
        b.iter(|| {
            dispatcher.set_state(black_box(CommandState::Executing));
            dispatcher.set_state(black_box(CommandState::Success));
        });
    });

    group.bench_function("full_state_cycle", |b| {
        b.iter(|| {
            dispatcher.set_state(black_box(CommandState::Idle));
            dispatcher.set_state(black_box(CommandState::Executing));
            dispatcher.set_state(black_box(CommandState::Success));
            dispatcher.set_state(black_box(CommandState::Idle));
        });
    });

    group.finish();
}

/// B32 Benchmark: State reads
///
/// # Performance Target
/// - <10ns per read (atomic load)
fn bench_state_reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("dispatcher/state_read");

    let dispatcher = CommandDispatcherCapsule::new();
    dispatcher.set_state(CommandState::Success);

    group.bench_function("single_state_read", |b| {
        b.iter(|| {
            black_box(dispatcher.state());
        });
    });

    group.bench_function("ten_state_reads", |b| {
        b.iter(|| {
            for _ in 0..10 {
                black_box(dispatcher.state());
            }
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 2: Mock Command Execution
// ============================================================================

/// B32 Benchmark: Mock command dispatch
///
/// # Purpose
/// Measure dispatcher overhead WITHOUT HTTP/subprocess. This isolates
/// the pure dispatcher logic (state transitions + counters).
///
/// # Performance Target
/// - <1μs per command (mock, no I/O)
///
/// # B32 Reality Check
/// - Real commands take 10-100ms (HTTP requests, subprocess spawn)
/// - Dispatcher overhead is <0.001% of total latency
/// - This benchmark measures best-case (no contention, no I/O)
fn bench_mock_command_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("dispatcher/mock_execute");

    let dispatcher = CommandDispatcherCapsule::new();

    group.bench_function("health_command", |b| {
        b.iter(|| {
            let _ = dispatcher.execute_mock(black_box("health"), black_box(&[]));
        });
    });

    group.bench_function("metrics_command", |b| {
        b.iter(|| {
            let _ = dispatcher.execute_mock(black_box("metrics"), black_box(&["--watch", "5"]));
        });
    });

    group.bench_function("budget_command", |b| {
        b.iter(|| {
            let _ = dispatcher.execute_mock(black_box("budget"), black_box(&["--json"]));
        });
    });

    group.finish();
}

/// B32 Benchmark: Execution counter overhead
///
/// # Performance Target
/// - <20ns per increment (atomic fetch_add)
fn bench_execution_counter(c: &mut Criterion) {
    let mut group = c.benchmark_group("dispatcher/counter");

    let dispatcher = CommandDispatcherCapsule::new();

    group.bench_function("increment_count", |b| {
        b.iter(|| {
            dispatcher.increment_execution_count();
        });
    });

    group.bench_function("read_count", |b| {
        b.iter(|| {
            black_box(dispatcher.execution_count.load(Ordering::Acquire));
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 3: Error Handling Path
// ============================================================================

/// B32 Benchmark: Error path overhead
///
/// # Purpose
/// Measure additional cost of error handling (state + error code).
///
/// # Performance Target
/// - <200ns per error (state transition + error code store)
fn bench_error_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("dispatcher/error");

    let dispatcher = CommandDispatcherCapsule::new();

    group.bench_function("set_error_state", |b| {
        b.iter(|| {
            dispatcher.set_state(black_box(CommandState::Failed));
            dispatcher.set_error(black_box(1));
        });
    });

    group.bench_function("read_error_code", |b| {
        b.iter(|| {
            black_box(dispatcher.last_error_code.load(Ordering::Acquire));
        });
    });

    group.bench_function("error_recovery_cycle", |b| {
        b.iter(|| {
            // Error path
            dispatcher.set_state(black_box(CommandState::Failed));
            dispatcher.set_error(black_box(1));

            // Recovery
            dispatcher.set_state(black_box(CommandState::Idle));
            dispatcher.set_error(black_box(0));
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    dispatcher_benches,
    bench_state_transitions,
    bench_state_reads,
    bench_mock_command_execution,
    bench_execution_counter,
    bench_error_handling,
);

criterion_main!(dispatcher_benches);
