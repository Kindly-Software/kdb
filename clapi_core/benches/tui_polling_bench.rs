//! TUI Metrics Polling Benchmarks - B32 Framework Compliance
//!
//! # Purpose
//! Measure honest atomic update overhead for metrics polling in TUI dashboard.
//! All benchmarks follow B32 framework guidelines for fair, reproducible measurement.
//!
//! # B32 Compliance
//! - **Fair Baseline**: Compare atomic operations against realistic workloads
//! - **Statistical Rigor**: 1000+ iterations, 95% CI via Criterion
//! - **Honest Claims**: <10ns per field update (atomic store)
//! - **Reality Check**: Polling overhead negligible vs HTTP fetch (10-100ms)
//!
//! # Benchmarks
//! 1. **Single Field Update**: One metric update (<10ns)
//! 2. **Batch Field Updates**: All metrics updated (<100ns)
//! 3. **Full Snapshot Read**: Read all metrics (<100ns)
//!
//! # Performance Targets
//! - Single field: <10ns (atomic store, Relaxed ordering)
//! - All fields (5 metrics): <100ns (5× atomic stores)
//! - Snapshot read (5 fields): <100ns (5× atomic loads)
//!
//! # Build Instructions
//! ```bash
//! cargo bench --bench tui_polling_bench
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

// ============================================================================
// MOCK DASHBOARD CONTENT CAPSULE (Matches Production Pattern)
// ============================================================================

/// Dashboard Content Capsule (128B, T1 Atomic)
///
/// Simulates production metrics cache for TUI
#[repr(C, align(128))]
struct DashboardContentCapsule {
    // Hot metrics (first 64B)
    budgets_count: AtomicU32,       // 4B
    providers_count: AtomicU32,     // 4B
    total_requests: AtomicU32,      // 4B
    avg_latency_ms: AtomicU32,      // 4B
    memory_mb: AtomicU32,           // 4B
    uptime_secs: AtomicU64,         // 8B
    last_refresh_ns: AtomicU64,     // 8B
    _padding1: [u8; 36],            // Pad to 64B

    // Cold metrics (second 64B)
    _padding2: [u8; 64],
}

impl DashboardContentCapsule {
    fn new() -> Self {
        Self {
            budgets_count: AtomicU32::new(0),
            providers_count: AtomicU32::new(0),
            total_requests: AtomicU32::new(0),
            avg_latency_ms: AtomicU32::new(0),
            memory_mb: AtomicU32::new(0),
            uptime_secs: AtomicU64::new(0),
            last_refresh_ns: AtomicU64::new(0),
            _padding1: [0; 36],
            _padding2: [0; 64],
        }
    }

    // Single field updates
    #[inline(always)]
    fn update_budgets_count(&self, count: u32) {
        self.budgets_count.store(count, Ordering::Relaxed);
    }

    #[inline(always)]
    fn update_providers_count(&self, count: u32) {
        self.providers_count.store(count, Ordering::Relaxed);
    }

    #[inline(always)]
    fn update_total_requests(&self, count: u32) {
        self.total_requests.store(count, Ordering::Relaxed);
    }

    #[inline(always)]
    fn update_avg_latency(&self, latency_ms: u32) {
        self.avg_latency_ms.store(latency_ms, Ordering::Relaxed);
    }

    #[inline(always)]
    fn update_memory_mb(&self, memory: u32) {
        self.memory_mb.store(memory, Ordering::Relaxed);
    }

    #[inline(always)]
    fn update_uptime(&self, secs: u64) {
        self.uptime_secs.store(secs, Ordering::Relaxed);
    }

    #[inline(always)]
    fn update_last_refresh(&self, timestamp_ns: u64) {
        self.last_refresh_ns.store(timestamp_ns, Ordering::Relaxed);
    }

    // Batch update (all fields)
    fn update_all_metrics(&self, budgets: u32, providers: u32, requests: u32, latency: u32, memory: u32) {
        self.budgets_count.store(budgets, Ordering::Relaxed);
        self.providers_count.store(providers, Ordering::Relaxed);
        self.total_requests.store(requests, Ordering::Relaxed);
        self.avg_latency_ms.store(latency, Ordering::Relaxed);
        self.memory_mb.store(memory, Ordering::Relaxed);
        self.uptime_secs.fetch_add(1, Ordering::Relaxed);
        self.last_refresh_ns.store(now_ns(), Ordering::Relaxed);
    }

    // Snapshot read (all fields)
    fn snapshot(&self) -> DashboardSnapshot {
        DashboardSnapshot {
            budgets_count: self.budgets_count.load(Ordering::Relaxed),
            providers_count: self.providers_count.load(Ordering::Relaxed),
            total_requests: self.total_requests.load(Ordering::Relaxed),
            avg_latency_ms: self.avg_latency_ms.load(Ordering::Relaxed),
            memory_mb: self.memory_mb.load(Ordering::Relaxed),
            uptime_secs: self.uptime_secs.load(Ordering::Relaxed),
            last_refresh_ns: self.last_refresh_ns.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct DashboardSnapshot {
    budgets_count: u32,
    providers_count: u32,
    total_requests: u32,
    avg_latency_ms: u32,
    memory_mb: u32,
    uptime_secs: u64,
    last_refresh_ns: u64,
}

fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

// ============================================================================
// BENCHMARK 1: Single Field Updates
// ============================================================================

/// B32 Benchmark: Single metric field update
///
/// # Purpose
/// Measure atomic store overhead for individual metric updates.
///
/// # Performance Target
/// - <10ns per field (atomic store, Relaxed ordering)
///
/// # B32 Compliance
/// - Fair baseline: Raw atomic operations (hardware limit)
/// - Reality check: Atomic stores are ~5-10ns on modern CPUs
fn bench_single_field_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("polling/single_field");

    let content = DashboardContentCapsule::new();

    group.bench_function("budgets_count", |b| {
        b.iter(|| {
            content.update_budgets_count(black_box(42));
        });
    });

    group.bench_function("providers_count", |b| {
        b.iter(|| {
            content.update_providers_count(black_box(8));
        });
    });

    group.bench_function("total_requests", |b| {
        b.iter(|| {
            content.update_total_requests(black_box(10000));
        });
    });

    group.bench_function("avg_latency", |b| {
        b.iter(|| {
            content.update_avg_latency(black_box(150));
        });
    });

    group.bench_function("memory_mb", |b| {
        b.iter(|| {
            content.update_memory_mb(black_box(256));
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 2: Batch Field Updates
// ============================================================================

/// B32 Benchmark: Batch metric update
///
/// # Purpose
/// Measure overhead of updating all metrics at once (typical polling scenario).
///
/// # Performance Target
/// - <100ns for 5 fields (5× atomic stores + timestamp)
///
/// # B32 Reality Check
/// - Polling interval is typically 1-5 seconds (1,000,000,000 - 5,000,000,000 ns)
/// - Atomic update overhead is <0.00001% of polling interval
fn bench_batch_field_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("polling/batch_update");

    let content = DashboardContentCapsule::new();

    group.bench_function("update_all_metrics", |b| {
        b.iter(|| {
            content.update_all_metrics(
                black_box(42),
                black_box(8),
                black_box(10000),
                black_box(150),
                black_box(256),
            );
        });
    });

    group.bench_function("update_subset_3_fields", |b| {
        b.iter(|| {
            content.update_budgets_count(black_box(42));
            content.update_providers_count(black_box(8));
            content.update_total_requests(black_box(10000));
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 3: Snapshot Reads
// ============================================================================

/// B32 Benchmark: Full metrics snapshot
///
/// # Purpose
/// Measure atomic read overhead for TUI rendering (read all metrics).
///
/// # Performance Target
/// - <100ns for 7 fields (7× atomic loads)
///
/// # B32 Reality Check
/// - TUI rendering is 5-16ms (terminal I/O bound)
/// - Snapshot read overhead is <0.002% of frame time
fn bench_snapshot_reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("polling/snapshot");

    let content = DashboardContentCapsule::new();
    content.update_all_metrics(42, 8, 10000, 150, 256);

    group.bench_function("full_snapshot", |b| {
        b.iter(|| {
            black_box(content.snapshot());
        });
    });

    group.bench_function("partial_snapshot_3_fields", |b| {
        b.iter(|| {
            black_box((
                content.budgets_count.load(Ordering::Relaxed),
                content.providers_count.load(Ordering::Relaxed),
                content.total_requests.load(Ordering::Relaxed),
            ));
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 4: Concurrent Updates (Simulated)
// ============================================================================

/// B32 Benchmark: Simulated concurrent polling
///
/// # Purpose
/// Measure contention-free atomic operations (single writer, single reader).
///
/// # Performance Target
/// - <200ns per update/read cycle (optimistic)
///
/// # B32 Reality Check
/// - No contention in TUI (single polling thread, single render thread)
/// - Atomics provide visibility without locks
fn bench_concurrent_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("polling/concurrent");

    for update_count in [1, 10, 100] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_updates", update_count)),
            &update_count,
            |b, &count| {
                let content = DashboardContentCapsule::new();

                b.iter(|| {
                    // Simulated polling thread (writer)
                    for i in 0..count {
                        content.update_budgets_count(black_box(i));
                    }

                    // Simulated render thread (reader)
                    for _ in 0..count {
                        black_box(content.budgets_count.load(Ordering::Relaxed));
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 5: Timestamp Updates
// ============================================================================

/// B32 Benchmark: Timestamp tracking overhead
///
/// # Purpose
/// Measure cost of timestamp updates (last refresh tracking).
///
/// # Performance Target
/// - <50ns (SystemTime::now + atomic store)
fn bench_timestamp_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("polling/timestamp");

    let content = DashboardContentCapsule::new();

    group.bench_function("update_last_refresh", |b| {
        b.iter(|| {
            content.update_last_refresh(black_box(now_ns()));
        });
    });

    group.bench_function("read_last_refresh", |b| {
        b.iter(|| {
            black_box(content.last_refresh_ns.load(Ordering::Relaxed));
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    polling_benches,
    bench_single_field_updates,
    bench_batch_field_updates,
    bench_snapshot_reads,
    bench_concurrent_access,
    bench_timestamp_updates,
);

criterion_main!(polling_benches);
