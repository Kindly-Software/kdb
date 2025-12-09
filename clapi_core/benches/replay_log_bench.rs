//! Replay Log Benchmarks (B32 Framework)
//!
//! **Performance Targets**:
//! - Append: <100ns (lockfree CAS loop vs 1ms sync I/O = 10,000× speedup)
//! - Hash chain verification: ~80ns per link
//! - Export: <1ms for 100 entries
//!
//! **Baseline**: Synchronous file I/O (~1ms per write)
//! **Statistical Rigor**: 1000+ iterations, 95% CI
//! **Fair Comparison**: Ring buffer vs sync I/O (not strawman)

use clapi_core::replay_log::{ReplayLog, export::{export_json, export_csv, export_binary}};
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::Duration;
use tempfile::NamedTempFile;

// ============================================================================
// BASELINE: Synchronous File I/O (Fair Comparison)
// ============================================================================

fn baseline_sync_io_append() -> std::io::Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let temp_file = NamedTempFile::new()?;
    let mut file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(temp_file.path())?;

    // Single write (simulates one log entry)
    let data = b"request_hash,response_hash,timestamp,provider,latency,cost\n";
    file.write_all(data)?;
    file.sync_all()?; // Force fsync (realistic baseline)

    Ok(())
}

// ============================================================================
// BENCHMARK 1: Append Performance
// ============================================================================

fn bench_append(c: &mut Criterion) {
    let mut group = c.benchmark_group("replay_log_append");

    // Baseline: Sync I/O append (~1ms)
    group.bench_function("baseline_sync_io", |b| {
        b.iter(|| {
            baseline_sync_io_append().expect("sync I/O should succeed");
        });
    });

    // Optimized: Ring buffer append (<100ns)
    group.bench_function("ring_buffer_append", |b| {
        let log = ReplayLog::new(100_000);

        b.iter(|| {
            log.append(
                black_box(0x1234567890ABCDEF),
                black_box(0xFEDCBA0987654321),
                black_box(42),
                black_box(150_000),
                black_box(50_00),
            )
            .expect("append should succeed");
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 2: Hash Chain Verification
// ============================================================================

fn bench_hash_chain_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_chain_verification");

    // Vary chain length (10, 100, 1000 entries)
    for count in [10, 100, 1000].iter() {
        let log = ReplayLog::new(*count);

        // Populate log
        for i in 0..*count {
            log.append(i as u64, i as u64 * 2, i as u64, 1000, 100)
                .expect("append should succeed");
        }

        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, _| {
            b.iter(|| {
                log.verify_integrity().expect("verification should succeed");
            });
        });
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 3: Export Performance
// ============================================================================

fn bench_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("export");

    // Create log with 100 entries
    let log = ReplayLog::new(1000);
    for i in 0..100 {
        log.append(i, i * 2, i * 3, i * 1000, i * 100)
            .expect("append should succeed");
    }

    // JSON export
    group.bench_function("export_json_100_entries", |b| {
        let temp_file = NamedTempFile::new().expect("create temp file");
        let path = temp_file.path().to_str().unwrap().to_string();

        b.iter(|| {
            log.export_json(&path).expect("export should succeed");
        });
    });

    // CSV export
    group.bench_function("export_csv_100_entries", |b| {
        let temp_file = NamedTempFile::new().expect("create temp file");
        let path = temp_file.path().to_str().unwrap().to_string();

        b.iter(|| {
            log.export_csv(&path).expect("export should succeed");
        });
    });

    // Binary export
    group.bench_function("export_binary_100_entries", |b| {
        let temp_file = NamedTempFile::new().expect("create temp file");
        let path = temp_file.path().to_str().unwrap().to_string();

        b.iter(|| {
            log.export_binary(&path).expect("export should succeed");
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK 4: Concurrent Append (Scalability)
// ============================================================================

fn bench_concurrent_append(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;

    let mut group = c.benchmark_group("concurrent_append");

    // Vary thread count (1, 2, 4, 8 threads)
    for thread_count in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            thread_count,
            |b, &tc| {
                b.iter(|| {
                    let log = Arc::new(ReplayLog::new(100_000));
                    let mut handles = vec![];

                    for thread_id in 0..tc {
                        let log_clone = Arc::clone(&log);
                        let handle = thread::spawn(move || {
                            for i in 0..100 {
                                let value = (thread_id * 1000 + i) as u64;
                                let _ = log_clone.append(value, value * 2, thread_id as u64, 1000, 100);
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().expect("thread should complete");
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 5: Memory Overhead
// ============================================================================

fn bench_memory_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_overhead");

    // Measure allocation time for different capacities
    for capacity in [1_000, 10_000, 100_000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(capacity),
            capacity,
            |b, &cap| {
                b.iter(|| {
                    let _log = ReplayLog::new(cap);
                });
            },
        );
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .sample_size(1000);
    targets =
        bench_append,
        bench_hash_chain_verification,
        bench_export,
        bench_concurrent_append,
        bench_memory_overhead,
}

criterion_main!(benches);
