//! Performance benchmarks for FenceCapsule
//!
//! ## Benchmark Targets (B32 Framework)
//!
//! - Signaled check: <5ns (cached, hot path)
//! - Signal write: <50ns (two-phase commit)
//! - Snapshot read: <10ns (cached, full state)
//! - Wait with immediate signal: <100ns
//!
//! ## B32 Framework Guidelines
//!
//! - Fair baseline: Compare against AtomicU64 (simple counter)
//! - Statistical rigor: Criterion provides 95% confidence intervals
//! - Realistic workload: Simulate GPU fence patterns
//! - Hardware awareness: Consider cache effects
//!
//! ## Realistic Performance Expectations
//!
//! - Typical improvement: 10-50% over naive implementations
//! - Exceptional: 2-10x over mutex-based approaches
//! - Revolutionary: >100x claims require extensive validation

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use kiang::fence::{FenceCapsule, FenceState};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ========== Hot Path Benchmarks ==========

fn bench_is_signaled_cached(c: &mut Criterion) {
    let mut group = c.benchmark_group("fence/is_signaled");

    // Pre-signal fence to ensure cached reads
    let fence = FenceCapsule::new(1);
    fence.signal(1000, 5000);

    group.bench_function("cached_true", |b| {
        b.iter(|| {
            // Hot path: check if signaled (should be ~3-5ns)
            black_box(fence.is_signaled(black_box(500)))
        })
    });

    group.bench_function("cached_false", |b| {
        b.iter(|| {
            // Check not yet signaled
            black_box(fence.is_signaled(black_box(1500)))
        })
    });

    group.finish();
}

fn bench_check_fence_cached(c: &mut Criterion) {
    let fence = FenceCapsule::new(1);
    fence.signal(1000, 5000);

    c.bench_function("fence/check_fence/cached", |b| {
        b.iter(|| {
            // Full state check (should be ~5-10ns cached)
            black_box(fence.check_fence(black_box(500)))
        })
    });
}

fn bench_completed_value(c: &mut Criterion) {
    let fence = FenceCapsule::new(1);
    fence.signal(1000, 5000);

    c.bench_function("fence/completed_value", |b| {
        b.iter(|| {
            // Simple value read (should be ~2-3ns)
            black_box(fence.completed_value())
        })
    });
}

fn bench_read_snapshot(c: &mut Criterion) {
    let fence = FenceCapsule::new(1);
    fence.signal(1000, 5000);

    c.bench_function("fence/read_snapshot", |b| {
        b.iter(|| {
            // Full snapshot read (should be ~5-10ns)
            black_box(fence.read_snapshot())
        })
    });
}

// ========== Writer Benchmarks ==========

fn bench_signal(c: &mut Criterion) {
    let fence = FenceCapsule::new(1);
    let mut value = 0u64;

    c.bench_function("fence/signal", |b| {
        b.iter(|| {
            value += 1;
            fence.signal(black_box(value), black_box(value * 1000));
        })
    });
}

fn bench_signal_with_readers(c: &mut Criterion) {
    let mut group = c.benchmark_group("fence/signal_with_readers");

    for num_readers in [1, 2, 4, 8].iter() {
        let fence = Arc::new(FenceCapsule::new(1));
        let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));

        // Spawn reader threads
        let mut handles = Vec::new();
        for _ in 0..*num_readers {
            let fence_clone = Arc::clone(&fence);
            let stop_clone = Arc::clone(&stop_flag);

            let handle = thread::spawn(move || {
                while !stop_clone.load(std::sync::atomic::Ordering::Relaxed) {
                    black_box(fence_clone.is_signaled(500));
                    thread::yield_now();
                }
            });
            handles.push(handle);
        }

        group.bench_with_input(
            BenchmarkId::from_parameter(num_readers),
            num_readers,
            |b, _| {
                let mut value = 0u64;
                b.iter(|| {
                    value += 1;
                    fence.signal(black_box(value), black_box(value * 1000));
                })
            },
        );

        // Stop readers
        stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
        for handle in handles {
            handle.join().unwrap();
        }
    }

    group.finish();
}

// ========== Comparison Benchmarks ==========

fn bench_comparison_atomic_u64(c: &mut Criterion) {
    // Baseline: Simple AtomicU64 counter (fair comparison)
    use std::sync::atomic::{AtomicU64, Ordering};

    let counter = AtomicU64::new(0);

    c.bench_function("baseline/atomic_u64_load", |b| {
        b.iter(|| black_box(counter.load(Ordering::Relaxed)))
    });

    c.bench_function("baseline/atomic_u64_store", |b| {
        let mut value = 0u64;
        b.iter(|| {
            value += 1;
            counter.store(black_box(value), Ordering::Relaxed);
        })
    });
}

fn bench_comparison_mutex_bool(c: &mut Criterion) {
    // Comparison: Mutex<bool> (traditional approach)
    use std::sync::Mutex;

    let signaled = Mutex::new(false);

    c.bench_function("comparison/mutex_bool_read", |b| {
        b.iter(|| {
            let guard = signaled.lock().unwrap();
            black_box(*guard)
        })
    });

    c.bench_function("comparison/mutex_bool_write", |b| {
        b.iter(|| {
            let mut guard = signaled.lock().unwrap();
            *guard = true;
        })
    });
}

// ========== Realistic Workload Benchmarks ==========

fn bench_gpu_simulation_1khz(c: &mut Criterion) {
    // Simulate GPU GuC scheduler at 1kHz (1ms intervals)
    let mut group = c.benchmark_group("workload/gpu_1khz");
    group.throughput(Throughput::Elements(1));

    let fence = Arc::new(FenceCapsule::new(1));

    // Spawn reader threads (simulate render threads)
    let num_readers = 4;
    let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut handles = Vec::new();

    for _ in 0..num_readers {
        let fence_clone = Arc::clone(&fence);
        let stop_clone = Arc::clone(&stop_flag);

        let handle = thread::spawn(move || {
            let mut checks = 0u64;
            while !stop_clone.load(std::sync::atomic::Ordering::Relaxed) {
                black_box(fence_clone.is_signaled(500));
                checks += 1;
                thread::yield_now();
            }
            checks
        });
        handles.push(handle);
    }

    group.bench_function("signal_check_cycle", |b| {
        let mut value = 0u64;
        b.iter(|| {
            value += 1;
            let now = std::time::Instant::now();
            fence.signal(black_box(value), now.elapsed().as_nanos() as u64);

            // Multiple readers check
            for _ in 0..10 {
                black_box(fence.is_signaled(value));
            }
        })
    });

    // Stop readers
    stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
    for handle in handles {
        let checks = handle.join().unwrap();
        println!("Reader performed {} checks", checks);
    }

    group.finish();
}

fn bench_batch_fence_checks(c: &mut Criterion) {
    // Benchmark checking multiple fences (realistic multi-command scenario)
    let mut group = c.benchmark_group("workload/batch_checks");

    for num_fences in [1, 4, 8, 16, 32].iter() {
        let fences: Vec<_> = (0..*num_fences)
            .map(|i| {
                let f = FenceCapsule::new(i);
                f.signal(1000, 5000);
                f
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(num_fences),
            num_fences,
            |b, _| {
                b.iter(|| {
                    let mut signaled_count = 0;
                    for fence in &fences {
                        if fence.is_signaled(500) {
                            signaled_count += 1;
                        }
                    }
                    black_box(signaled_count)
                })
            },
        );
    }

    group.finish();
}

// ========== Memory Ordering Comparison ==========

fn bench_memory_ordering_comparison(c: &mut Criterion) {
    // Compare Relaxed vs Acquire ordering (validate Q32 assumption)
    use std::sync::atomic::{AtomicU64, Ordering};

    let mut group = c.benchmark_group("memory_ordering");

    let value = AtomicU64::new(0);

    group.bench_function("relaxed_load", |b| {
        b.iter(|| black_box(value.load(Ordering::Relaxed)))
    });

    group.bench_function("acquire_load", |b| {
        b.iter(|| black_box(value.load(Ordering::Acquire)))
    });

    group.bench_function("seqcst_load", |b| {
        b.iter(|| black_box(value.load(Ordering::SeqCst)))
    });

    group.finish();
}

// ========== Contention Benchmarks ==========

fn bench_reader_contention(c: &mut Criterion) {
    // Measure performance under various reader counts
    let mut group = c.benchmark_group("contention/readers");

    for num_readers in [1, 2, 4, 8, 16, 32].iter() {
        let fence = Arc::new(FenceCapsule::new(1));
        fence.signal(1000, 5000);

        let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut handles = Vec::new();

        for _ in 0..*num_readers {
            let fence_clone = Arc::clone(&fence);
            let stop_clone = Arc::clone(&stop_flag);

            let handle = thread::spawn(move || {
                let mut reads = 0u64;
                while !stop_clone.load(std::sync::atomic::Ordering::Relaxed) {
                    black_box(fence_clone.is_signaled(500));
                    reads += 1;
                }
                reads
            });
            handles.push(handle);
        }

        // Let readers run for a bit
        thread::sleep(Duration::from_millis(100));

        group.bench_with_input(
            BenchmarkId::from_parameter(num_readers),
            num_readers,
            |b, _| {
                b.iter(|| {
                    // Measure single read under contention
                    black_box(fence.is_signaled(500))
                })
            },
        );

        // Stop readers
        stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);

        let mut total_reads = 0;
        for handle in handles {
            total_reads += handle.join().unwrap();
        }

        println!("{} readers: {} total reads", num_readers, total_reads);
    }

    group.finish();
}

criterion_group!(
    hot_path,
    bench_is_signaled_cached,
    bench_check_fence_cached,
    bench_completed_value,
    bench_read_snapshot,
);

criterion_group!(writer, bench_signal, bench_signal_with_readers,);

criterion_group!(
    comparison,
    bench_comparison_atomic_u64,
    bench_comparison_mutex_bool,
    bench_memory_ordering_comparison,
);

criterion_group!(
    workload,
    bench_gpu_simulation_1khz,
    bench_batch_fence_checks,
);

criterion_group!(contention, bench_reader_contention,);

criterion_main!(hot_path, writer, comparison, workload, contention);
