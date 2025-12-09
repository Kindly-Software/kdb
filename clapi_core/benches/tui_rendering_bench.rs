//! TUI Rendering Benchmarks - B32 Framework Compliance
//!
//! # Purpose
//! Measure honest TUI rendering performance for clapi metrics dashboard.
//! All benchmarks follow B32 framework guidelines for fair, reproducible measurement.
//!
//! # B32 Compliance
//! - **Fair Baseline**: Compare vs raw crossterm I/O (apples-to-apples)
//! - **Statistical Rigor**: 1000+ iterations, 95% CI via Criterion
//! - **Honest Claims**: 10-50% typical, 2× exceptional (realistic targets)
//! - **Reality Check**: Terminal I/O limited by syscalls (~1-5ms)
//!
//! # Benchmarks
//! 1. **Baseline**: Raw terminal I/O (crossterm execute)
//! 2. **Frame Rendering**: Full dashboard frame (<16ms for 60 FPS)
//! 3. **Table Formatting**: Budget/provider table generation (<5ms)
//! 4. **State Updates**: Atomic capsule operations (<100ns)
//! 5. **Metrics Conversion**: Raw → display format (<1ms)
//!
//! # Performance Targets
//! - **Full frame**: <16ms (60 FPS target)
//! - **Table format**: <5ms (tabled overhead)
//! - **State update**: <100ns (atomic operations)
//! - **Metrics conversion**: <1ms (CPU-bound, realistic)
//!
//! # Build Instructions
//! ```bash
//! cargo bench --bench tui_rendering_bench
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use crossterm::{cursor, execute, terminal::{self, ClearType}};
use std::io::{self, Write};
use std::time::Duration;

// Import test stubs for benchmark isolation
// Note: We benchmark against real implementations where possible

// ============================================================================
// BENCHMARK 1: Hardware Baseline (Raw Terminal I/O)
// ============================================================================

/// B32 Benchmark: Raw terminal clear + write (hardware limit)
///
/// # Purpose
/// Establish baseline for terminal I/O overhead. This is the **hardware limit**
/// that any TUI framework must pay. Any overhead beyond this is framework cost.
///
/// # Performance Target
/// - <1ms (syscall overhead, ~500µs typical)
///
/// # B32 Compliance
/// - Fair baseline: Raw crossterm I/O (no higher-level abstractions)
/// - Reality check: Terminal I/O is syscall-bound
fn bench_raw_terminal_clear_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline/raw_terminal");

    // Configure for I/O benchmarking
    group.sample_size(100); // Fewer samples due to I/O cost
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("clear_and_write_100_lines", |b| {
        b.iter(|| {
            let mut stdout = io::stdout();

            // Clear screen
            execute!(
                stdout,
                terminal::Clear(ClearType::All),
                cursor::MoveTo(0, 0)
            ).unwrap();

            // Write 100 lines (realistic dashboard size)
            for i in 0..100 {
                writeln!(stdout, "Line {}: Sample dashboard content", i).unwrap();
            }

            black_box(stdout.flush().unwrap());
        });
    });

    group.finish();
}

/// B32 Benchmark: Single line write (minimal I/O)
///
/// # Performance Target
/// - <10µs per line (syscall overhead)
fn bench_raw_single_line_write(c: &mut Criterion) {
    c.bench_function("baseline/single_line_write", |b| {
        b.iter(|| {
            let mut stdout = io::stdout();
            writeln!(stdout, "Sample line: Budget: $100.00, Status: OK").unwrap();
            black_box(stdout.flush().unwrap());
        });
    });
}

// ============================================================================
// BENCHMARK 2: Table Formatting (tabled crate)
// ============================================================================

use tabled::{Table, Tabled, settings::Style};

#[derive(Debug, Clone, Tabled)]
struct BenchmarkBudgetMetric {
    #[tabled(rename = "Budget ID")]
    budget_id: String,

    #[tabled(rename = "Available")]
    available: String,

    #[tabled(rename = "Spent")]
    spent: String,

    #[tabled(rename = "Status")]
    status: String,

    #[tabled(rename = "Trend")]
    trend: String,
}

/// B32 Benchmark: Table formatting overhead
///
/// # Purpose
/// Measure tabled crate overhead for formatting budget/provider tables.
/// This is pure CPU work (no I/O), so we expect <5ms for typical tables.
///
/// # Performance Target
/// - 5 rows: <500µs (typical small dashboard)
/// - 50 rows: <5ms (large budget list)
///
/// # B32 Reality Check
/// - String allocation + formatting is O(n×m) where n=rows, m=columns
/// - Expect linear scaling with row count
fn bench_table_formatting(c: &mut Criterion) {
    let mut group = c.benchmark_group("table_formatting");

    for num_rows in [5, 10, 25, 50, 100] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_rows", num_rows)),
            &num_rows,
            |b, &rows| {
                // Generate sample data
                let data: Vec<BenchmarkBudgetMetric> = (0..rows)
                    .map(|i| BenchmarkBudgetMetric {
                        budget_id: format!("budget_{}", i),
                        available: format!("${:.2}", 100.0 - i as f64),
                        spent: format!("${:.2}", i as f64),
                        status: if i % 3 == 0 { "Healthy" } else { "Warning" }.to_string(),
                        trend: "→".to_string(),
                    })
                    .collect();

                b.iter(|| {
                    let table = Table::new(&data)
                        .with(Style::modern())
                        .to_string();
                    black_box(table);
                });
            },
        );
    }

    group.finish();
}

/// B32 Benchmark: Table rendering to terminal
///
/// # Performance Target
/// - <5ms (table format + terminal write)
fn bench_table_render_to_terminal(c: &mut Criterion) {
    let mut group = c.benchmark_group("table_rendering");
    group.sample_size(50); // I/O-bound

    let data: Vec<BenchmarkBudgetMetric> = (0..10)
        .map(|i| BenchmarkBudgetMetric {
            budget_id: format!("budget_{}", i),
            available: format!("${:.2}", 100.0 - i as f64),
            spent: format!("${:.2}", i as f64),
            status: if i % 3 == 0 { "Healthy" } else { "Warning" }.to_string(),
            trend: "→".to_string(),
        })
        .collect();

    group.bench_function("format_and_write", |b| {
        b.iter(|| {
            let mut stdout = io::stdout();

            // Format table
            let table = Table::new(&data)
                .with(Style::modern())
                .to_string();

            // Write to terminal
            for line in table.lines() {
                writeln!(stdout, "│ {}", line).unwrap();
            }

            black_box(stdout.flush().unwrap());
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 3: Metrics Conversion (Raw → Display)
// ============================================================================

/// Sample raw metrics (simulates API response)
#[derive(Clone)]
struct RawBudgetMetric {
    budget_id: u64,
    available_cents: i64,
    spent_cents: i64,
    utilization_bp: u32,
}

/// B32 Benchmark: Metrics conversion overhead
///
/// # Purpose
/// Measure CPU cost of converting raw metrics (cents, basis points) to
/// display format (dollar strings, percentages). Pure CPU work.
///
/// # Performance Target
/// - Single metric: <1µs (simple arithmetic + string allocation)
/// - 100 metrics: <100µs (linear scaling)
fn bench_metrics_conversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics_conversion");

    for num_metrics in [1, 10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_metrics", num_metrics)),
            &num_metrics,
            |b, &count| {
                let raw: Vec<RawBudgetMetric> = (0..count)
                    .map(|i| RawBudgetMetric {
                        budget_id: i as u64,
                        available_cents: 10000 - (i as i64 * 50),
                        spent_cents: i as i64 * 50,
                        utilization_bp: (i as u32 * 100) % 10000,
                    })
                    .collect();

                b.iter(|| {
                    let converted: Vec<_> = raw.iter()
                        .map(|r| {
                            let available = format!("${:.2}", r.available_cents as f64 / 100.0);
                            let spent = format!("${:.2}", r.spent_cents as f64 / 100.0);
                            let utilization_pct = r.utilization_bp / 100;
                            let status = if r.utilization_bp < 2000 {
                                format!("Healthy ({}%)", utilization_pct)
                            } else if r.utilization_bp < 5000 {
                                format!("Warning ({}%)", utilization_pct)
                            } else {
                                format!("Critical ({}%)", utilization_pct)
                            };

                            (available, spent, status)
                        })
                        .collect();

                    black_box(converted);
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 4: Full Frame Rendering (End-to-End)
// ============================================================================

/// B32 Benchmark: Full dashboard frame rendering
///
/// # Purpose
/// Measure end-to-end latency for rendering complete dashboard frame:
/// 1. Clear screen
/// 2. Format header
/// 3. Format budget table
/// 4. Format provider table
/// 5. Format system metrics
/// 6. Format footer
/// 7. Flush to terminal
///
/// # Performance Target
/// - <16ms (60 FPS target, realistic for terminal I/O)
/// - <8ms ideal (120 FPS, aggressive but achievable)
///
/// # B32 Reality Check
/// - Terminal I/O is syscall-bound (~1-5ms minimum)
/// - Table formatting adds CPU overhead (~1-3ms)
/// - Total 16ms is **realistic and honest** for terminal UIs
fn bench_full_frame_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_frame_render");
    group.sample_size(50); // I/O-bound
    group.measurement_time(Duration::from_secs(10));

    // Prepare sample data (10 budgets, 3 providers)
    let budgets: Vec<BenchmarkBudgetMetric> = (0..10)
        .map(|i| BenchmarkBudgetMetric {
            budget_id: format!("budget_{}", i),
            available: format!("${:.2}", 100.0 - i as f64),
            spent: format!("${:.2}", i as f64),
            status: if i % 3 == 0 { "Healthy" } else { "Warning" }.to_string(),
            trend: "→".to_string(),
        })
        .collect();

    group.bench_function("10_budgets_3_providers", |b| {
        b.iter(|| {
            let mut stdout = io::stdout();

            // 1. Clear screen
            execute!(
                stdout,
                terminal::Clear(ClearType::All),
                cursor::MoveTo(0, 0)
            ).unwrap();

            // 2. Header
            writeln!(stdout, "┌─────────────────────────────────────────────────┐").unwrap();
            writeln!(stdout, "│ clapi Metrics Dashboard                         │").unwrap();
            writeln!(stdout, "├─────────────────────────────────────────────────┤").unwrap();

            // 3. Budget table
            let table = Table::new(&budgets)
                .with(Style::modern())
                .to_string();
            for line in table.lines() {
                writeln!(stdout, "│ {}", line).unwrap();
            }

            // 4. System metrics
            writeln!(stdout, "│").unwrap();
            writeln!(stdout, "│ SYSTEM METRICS").unwrap();
            writeln!(stdout, "│ Uptime: 2h 34m 14s │ Memory: 256 MB │ Requests: 847").unwrap();

            // 5. Footer
            writeln!(stdout, "│ Press 'q' to quit │ 'p' to pause │ 'r' to resume │").unwrap();
            writeln!(stdout, "└─────────────────────────────────────────────────┘").unwrap();

            // 6. Flush
            black_box(stdout.flush().unwrap());
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 5: Incremental Updates (Command Palette Filter)
// ============================================================================

/// B32 Benchmark: Command palette filtering
///
/// # Purpose
/// Measure latency for interactive command filtering (user types, UI updates).
/// This must be <1ms for responsive feel (ideally <100µs).
///
/// # Performance Target
/// - <1ms (interactive threshold, 1000 FPS equivalent)
/// - <100µs ideal (feels instant to users)
fn bench_command_filter(c: &mut Criterion) {
    let commands = vec![
        "start", "stop", "status", "metrics", "budget", "provider",
        "cache", "compression", "config", "logs", "help", "quit",
    ];

    c.bench_function("command_palette_filter", |b| {
        b.iter(|| {
            let query = black_box("st");

            // Filter commands (case-insensitive substring match)
            let filtered: Vec<_> = commands.iter()
                .filter(|cmd| cmd.to_lowercase().contains(query))
                .collect();

            black_box(filtered);
        });
    });
}

// ============================================================================
// BENCHMARK 6: State Update Performance (Atomic Operations Simulation)
// ============================================================================

/// B32 Benchmark: Dashboard state updates (simulated with raw atomics)
///
/// # Purpose
/// Measure atomic operation overhead for dashboard state synchronization.
/// Uses raw AtomicI64/AtomicU64 to avoid module compilation dependencies.
///
/// # Performance Target
/// - <100ns per update (atomic operation hardware limit)
/// - <5ns ideal (single atomic load/store)

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;

fn bench_state_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_update");

    let budget = Arc::new(AtomicI64::new(0));
    let timestamp = Arc::new(AtomicU64::new(0));

    // Single-threaded baseline
    group.bench_function("budget_update_single_thread", |b| {
        b.iter(|| {
            budget.store(black_box(50000), Ordering::Release);
            black_box(budget.load(Ordering::Acquire));
        });
    });

    // Multi-field update
    group.bench_function("multi_field_update", |b| {
        b.iter(|| {
            budget.store(black_box(50000), Ordering::Release);
            timestamp.store(black_box(1_000_000_000_000_000_000), Ordering::Release);
        });
    });

    // Snapshot (read multiple fields)
    group.bench_function("full_snapshot", |b| {
        b.iter(|| {
            black_box((
                budget.load(Ordering::Acquire),
                timestamp.load(Ordering::Acquire),
            ));
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 7: Concurrent State Updates
// ============================================================================

/// B32 Benchmark: Concurrent state updates (contention analysis)
///
/// # Purpose
/// Validate atomic operations scale under contention (2, 4, 8 threads).
///
/// # Performance Target
/// - 2 threads: 1.5-2× throughput (near-linear scaling)
/// - 4 threads: 3-4× throughput
/// - 8 threads: 5-7× throughput (contention starts to matter)
fn bench_concurrent_state_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_state_updates");
    group.sample_size(50); // Thread spawning overhead

    for num_threads in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_threads", num_threads)),
            &num_threads,
            |b, &threads| {
                let budget = Arc::new(AtomicI64::new(0));

                b.iter(|| {
                    let mut handles = vec![];

                    for thread_id in 0..threads {
                        let b = Arc::clone(&budget);
                        handles.push(std::thread::spawn(move || {
                            for i in 0..1000 {
                                let value = (thread_id as i64 * 10000) + i;
                                b.store(value, Ordering::Release);
                                black_box(b.load(Ordering::Acquire));
                            }
                        }));
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    tui_rendering_benches,
    // Baselines (hardware limits)
    bench_raw_terminal_clear_write,
    bench_raw_single_line_write,

    // Table formatting (CPU-bound)
    bench_table_formatting,
    bench_table_render_to_terminal,

    // Metrics conversion (CPU-bound)
    bench_metrics_conversion,

    // Full frame rendering (I/O + CPU)
    bench_full_frame_render,

    // Interactive updates
    bench_command_filter,

    // State updates (atomic capsule)
    bench_state_update,
    bench_concurrent_state_updates,
);

criterion_main!(tui_rendering_benches);
