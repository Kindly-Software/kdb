//! TUI Server Process State Benchmarks - B32 Framework Compliance
//!
//! # Purpose
//! Measure honest atomic operation overhead for server status tracking.
//! All benchmarks follow B32 framework guidelines for fair, reproducible measurement.
//!
//! # B32 Compliance
//! - **Fair Baseline**: Raw atomic operations (hardware limit)
//! - **Statistical Rigor**: 1000+ iterations, 95% CI via Criterion
//! - **Honest Claims**: <20ns state check, <50ns uptime calculation
//! - **Reality Check**: Status checks are negligible vs subprocess management
//!
//! # Benchmarks
//! 1. **State Checks**: is_running(), uptime() (<20ns)
//! 2. **Counter Updates**: Atomic increments (<10ns)
//! 3. **Timestamp Operations**: Error timestamps (<50ns)
//!
//! # Performance Targets
//! - State check: <20ns (atomic load)
//! - Uptime calc: <50ns (atomic load + arithmetic)
//! - Counter update: <10ns (atomic fetch_add)
//!
//! # Build Instructions
//! ```bash
//! cargo bench --bench tui_server_bench
//! ```

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// MOCK SERVER STATUS CAPSULE (Matches Production Pattern)
// ============================================================================

/// Server Status Capsule (64B, T1 Atomic)
///
/// Simulates production server process status tracking
#[repr(C, align(64))]
struct ServerStatusCapsule {
    running: AtomicBool,               // Server running status
    uptime_secs: AtomicU64,            // Server uptime (seconds)
    total_requests: AtomicU64,         // Total requests processed
    active_requests: AtomicU32,        // Currently in-flight requests
    last_error_timestamp_ns: AtomicU64, // Last error timestamp
    _padding: [u8; 24],                // Complete 64B cache line
}

impl ServerStatusCapsule {
    fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            uptime_secs: AtomicU64::new(0),
            total_requests: AtomicU64::new(0),
            active_requests: AtomicU32::new(0),
            last_error_timestamp_ns: AtomicU64::new(0),
            _padding: [0; 24],
        }
    }

    // State checks
    #[inline(always)]
    fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    #[inline(always)]
    fn set_running(&self, running: bool) {
        self.running.store(running, Ordering::Release);
    }

    // Uptime operations
    #[inline(always)]
    fn uptime_secs(&self) -> u64 {
        self.uptime_secs.load(Ordering::Acquire)
    }

    #[inline(always)]
    fn increment_uptime(&self) {
        self.uptime_secs.fetch_add(1, Ordering::AcqRel);
    }

    /// Format uptime as human-readable string
    ///
    /// Performance: <50ns (arithmetic operations)
    fn uptime_formatted(&self) -> String {
        let secs = self.uptime_secs();
        let hours = secs / 3600;
        let minutes = (secs % 3600) / 60;
        let seconds = secs % 60;

        if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, seconds)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }

    // Request counters
    #[inline(always)]
    fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Acquire)
    }

    #[inline(always)]
    fn increment_total_requests(&self) {
        self.total_requests.fetch_add(1, Ordering::AcqRel);
    }

    #[inline(always)]
    fn active_requests(&self) -> u32 {
        self.active_requests.load(Ordering::Acquire)
    }

    #[inline(always)]
    fn increment_active_requests(&self) {
        self.active_requests.fetch_add(1, Ordering::AcqRel);
    }

    #[inline(always)]
    fn decrement_active_requests(&self) {
        self.active_requests.fetch_update(
            Ordering::AcqRel,
            Ordering::Acquire,
            |val| Some(val.saturating_sub(1)),
        ).ok();
    }

    // Error tracking
    #[inline(always)]
    fn record_error(&self) {
        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        self.last_error_timestamp_ns.store(now_ns, Ordering::Release);
    }

    #[inline(always)]
    fn last_error_timestamp_ns(&self) -> u64 {
        self.last_error_timestamp_ns.load(Ordering::Acquire)
    }
}

// ============================================================================
// BENCHMARK 1: Server State Checks
// ============================================================================

/// B32 Benchmark: Server running state check
///
/// # Purpose
/// Measure atomic load overhead for is_running() check.
///
/// # Performance Target
/// - <20ns (single atomic load, Acquire ordering)
///
/// # B32 Compliance
/// - Fair baseline: Raw atomic operation (hardware limit)
/// - Reality check: AtomicBool load is ~5-10ns on modern CPUs
fn bench_state_checks(c: &mut Criterion) {
    let mut group = c.benchmark_group("server/state_check");

    let server = ServerStatusCapsule::new();
    server.set_running(true);

    group.bench_function("is_running", |b| {
        b.iter(|| {
            black_box(server.is_running());
        });
    });

    group.bench_function("set_running_true", |b| {
        b.iter(|| {
            server.set_running(black_box(true));
        });
    });

    group.bench_function("set_running_false", |b| {
        b.iter(|| {
            server.set_running(black_box(false));
        });
    });

    group.bench_function("toggle_running", |b| {
        b.iter(|| {
            let current = server.is_running();
            server.set_running(!current);
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 2: Uptime Operations
// ============================================================================

/// B32 Benchmark: Uptime tracking
///
/// # Purpose
/// Measure uptime read/increment overhead.
///
/// # Performance Target
/// - Read: <10ns (atomic load)
/// - Increment: <10ns (atomic fetch_add)
/// - Formatted: <50ns (load + arithmetic + format allocation)
///
/// # B32 Reality Check
/// - Uptime increment happens once per second
/// - Overhead is negligible compared to 1-second interval
fn bench_uptime_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("server/uptime");

    let server = ServerStatusCapsule::new();

    group.bench_function("read_uptime", |b| {
        b.iter(|| {
            black_box(server.uptime_secs());
        });
    });

    group.bench_function("increment_uptime", |b| {
        b.iter(|| {
            server.increment_uptime();
        });
    });

    // Format uptime (includes String allocation)
    group.bench_function("format_uptime_seconds", |b| {
        server.uptime_secs.store(30, Ordering::Release);
        b.iter(|| {
            black_box(server.uptime_formatted());
        });
    });

    group.bench_function("format_uptime_minutes", |b| {
        server.uptime_secs.store(125, Ordering::Release); // 2m 5s
        b.iter(|| {
            black_box(server.uptime_formatted());
        });
    });

    group.bench_function("format_uptime_hours", |b| {
        server.uptime_secs.store(3661, Ordering::Release); // 1h 1m 1s
        b.iter(|| {
            black_box(server.uptime_formatted());
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 3: Request Counters
// ============================================================================

/// B32 Benchmark: Request counter operations
///
/// # Purpose
/// Measure atomic increment/decrement overhead for request tracking.
///
/// # Performance Target
/// - Increment: <10ns (atomic fetch_add)
/// - Decrement: <20ns (atomic fetch_update with saturating_sub)
/// - Read: <10ns (atomic load)
///
/// # B32 Reality Check
/// - Request counters updated per request
/// - Overhead is <0.001% of typical request latency (10-100ms)
fn bench_request_counters(c: &mut Criterion) {
    let mut group = c.benchmark_group("server/counters");

    let server = ServerStatusCapsule::new();

    // Total requests counter
    group.bench_function("increment_total_requests", |b| {
        b.iter(|| {
            server.increment_total_requests();
        });
    });

    group.bench_function("read_total_requests", |b| {
        b.iter(|| {
            black_box(server.total_requests());
        });
    });

    // Active requests counter
    group.bench_function("increment_active_requests", |b| {
        b.iter(|| {
            server.increment_active_requests();
        });
    });

    group.bench_function("decrement_active_requests", |b| {
        b.iter(|| {
            server.increment_active_requests(); // Ensure non-zero
            server.decrement_active_requests();
        });
    });

    group.bench_function("read_active_requests", |b| {
        b.iter(|| {
            black_box(server.active_requests());
        });
    });

    // Request lifecycle (increment active → increment total → decrement active)
    group.bench_function("request_lifecycle", |b| {
        b.iter(|| {
            server.increment_active_requests();
            server.increment_total_requests();
            server.decrement_active_requests();
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 4: Error Timestamp Operations
// ============================================================================

/// B32 Benchmark: Error timestamp tracking
///
/// # Purpose
/// Measure overhead of error timestamp updates.
///
/// # Performance Target
/// - Record error: <50ns (SystemTime::now + atomic store)
/// - Read timestamp: <10ns (atomic load)
///
/// # B32 Reality Check
/// - Error timestamps updated on error events (infrequent)
/// - SystemTime::now is the dominant cost (~20-40ns)
fn bench_error_timestamps(c: &mut Criterion) {
    let mut group = c.benchmark_group("server/error_timestamp");

    let server = ServerStatusCapsule::new();

    group.bench_function("record_error", |b| {
        b.iter(|| {
            server.record_error();
        });
    });

    group.bench_function("read_error_timestamp", |b| {
        b.iter(|| {
            black_box(server.last_error_timestamp_ns());
        });
    });

    group.bench_function("error_cycle_record_read", |b| {
        b.iter(|| {
            server.record_error();
            black_box(server.last_error_timestamp_ns());
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 5: Full Status Snapshot
// ============================================================================

/// B32 Benchmark: Complete status snapshot
///
/// # Purpose
/// Measure overhead of reading all server status fields at once.
///
/// # Performance Target
/// - <100ns (5 atomic loads)
///
/// # B32 Reality Check
/// - TUI rendering reads status once per frame (60 FPS = 16ms interval)
/// - Snapshot overhead is <0.001% of frame time
fn bench_status_snapshot(c: &mut Criterion) {
    let mut group = c.benchmark_group("server/snapshot");

    let server = ServerStatusCapsule::new();
    server.set_running(true);
    server.uptime_secs.store(3661, Ordering::Release);
    server.total_requests.store(10000, Ordering::Release);
    server.active_requests.store(42, Ordering::Release);
    server.record_error();

    group.bench_function("full_snapshot", |b| {
        b.iter(|| {
            black_box((
                server.is_running(),
                server.uptime_secs(),
                server.total_requests(),
                server.active_requests(),
                server.last_error_timestamp_ns(),
            ));
        });
    });

    group.bench_function("partial_snapshot_3_fields", |b| {
        b.iter(|| {
            black_box((
                server.is_running(),
                server.uptime_secs(),
                server.total_requests(),
            ));
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    server_benches,
    bench_state_checks,
    bench_uptime_operations,
    bench_request_counters,
    bench_error_timestamps,
    bench_status_snapshot,
);

criterion_main!(server_benches);
