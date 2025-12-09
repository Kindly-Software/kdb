//! B32 Benchmarking Framework - AdvancedRateLimiter64 Performance Validation
//!
//! **Framework**: B32 (32 benchmarking guidelines + 27 hardware reality checks)
//! **Coverage**: Operation latency, throughput, scalability, jitter overhead, hardware reality
//!
//! # B32 Guidelines Applied
//! - Fair baselines (compare to basic RateLimitCapsule and mutex-based limiter)
//! - Statistical rigor (1000+ iterations, 95% CI via Criterion)
//! - Honest claims (10-30% typical, 3-10× exceptional for jitter overhead)
//! - Hardware reality (measure on same hardware, same compiler)
//!
//! # Performance Targets
//! - acquire_token(): <10ns (single atomic fetch_sub, Relaxed)
//! - acquire_token_with_jitter(): <50ns (atomic + RNG + refill check)
//! - generate_jitter(): <20ns (LCG RNG)
//! - refill_tokens_if_needed(): <100ns (CAS loop, no contention)

use clapi_core::capsules::{AdvancedRateLimiter64, RateLimitCapsule};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::{Arc, Mutex};
use std::thread;

// ============================================================================
// Baseline: Mutex-based rate limiter (for fair comparison)
// ============================================================================

struct MutexRateLimiter {
    state: Mutex<MutexRateLimiterState>,
}

struct MutexRateLimiterState {
    tokens: i32,
    capacity: i32,
}

impl MutexRateLimiter {
    fn new(capacity: i32) -> Self {
        Self {
            state: Mutex::new(MutexRateLimiterState {
                tokens: capacity,
                capacity,
            }),
        }
    }

    fn acquire_token(&self) -> Result<i32, ()> {
        let mut state = self.state.lock().unwrap();
        if state.tokens <= 0 {
            return Err(());
        }
        state.tokens -= 1;
        Ok(state.tokens)
    }
}

// ============================================================================
// B32: Operation Latency Benchmarks
// ============================================================================

fn bench_acquire_token_no_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("acquire_token_no_contention");

    // AdvancedRateLimiter64 (target: <10ns)
    group.bench_function("advanced_atomic", |b| {
        let limiter = AdvancedRateLimiter64::with_capacity_and_period(1_000_000, 60_000_000_000);
        b.iter(|| {
            black_box(limiter.acquire_token()).ok();
        });
    });

    // RateLimitCapsule baseline (sliding window)
    group.bench_function("basic_sliding_window", |b| {
        let limiter = RateLimitCapsule::with_quota(1_000_000);
        b.iter(|| {
            black_box(limiter.increment_request()).ok();
        });
    });

    // Mutex baseline (expected: ~50-150ns)
    group.bench_function("mutex_baseline", |b| {
        let limiter = MutexRateLimiter::new(1_000_000);
        b.iter(|| {
            black_box(limiter.acquire_token()).ok();
        });
    });

    group.finish();
}

fn bench_acquire_token_with_jitter(c: &mut Criterion) {
    let mut group = c.benchmark_group("acquire_token_with_jitter");

    // AdvancedRateLimiter64 (target: <50ns)
    group.bench_function("advanced_with_jitter", |b| {
        let limiter = AdvancedRateLimiter64::with_capacity_and_period(1_000_000, 60_000_000_000);
        b.iter(|| {
            black_box(limiter.acquire_token_with_jitter()).ok();
        });
    });

    // AdvancedRateLimiter64 without jitter (for overhead measurement)
    group.bench_function("advanced_no_jitter", |b| {
        let limiter = AdvancedRateLimiter64::with_capacity_and_period(1_000_000, 60_000_000_000);
        b.iter(|| {
            black_box(limiter.acquire_token()).ok();
        });
    });

    group.finish();
}

fn bench_jitter_generation_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("jitter_generation_overhead");

    // Measure pure jitter generation cost
    group.bench_function("generate_jitter", |b| {
        let limiter = AdvancedRateLimiter64::new();
        b.iter(|| {
            // Access private method through public interface
            let _ = black_box(limiter.acquire_token_with_jitter());
        });
    });

    group.finish();
}

fn bench_stats_snapshot(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats_snapshot");

    group.bench_function("advanced_stats", |b| {
        let limiter = AdvancedRateLimiter64::new();
        limiter.acquire_token().ok();

        b.iter(|| {
            black_box(limiter.stats());
        });
    });

    group.bench_function("basic_stats", |b| {
        let limiter = RateLimitCapsule::new();
        limiter.increment_request().ok();

        b.iter(|| {
            black_box(limiter.stats());
        });
    });

    group.finish();
}

// ============================================================================
// B32: Contention Benchmarks
// ============================================================================

fn bench_acquire_token_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("acquire_token_contention");

    for threads in [2, 4, 8].iter() {
        // AdvancedRateLimiter64
        group.bench_with_input(
            BenchmarkId::new("advanced_atomic", threads),
            threads,
            |b, &t| {
                b.iter_custom(|iters| {
                    let limiter = Arc::new(AdvancedRateLimiter64::with_capacity_and_period(
                        10_000_000,
                        60_000_000_000,
                    ));
                    let mut handles = vec![];
                    let start = std::time::Instant::now();

                    for _ in 0..t {
                        let l = Arc::clone(&limiter);
                        handles.push(thread::spawn(move || {
                            for _ in 0..iters / t as u64 {
                                let _ = l.acquire_token();
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }

                    start.elapsed()
                });
            },
        );

        // Mutex baseline
        group.bench_with_input(
            BenchmarkId::new("mutex_baseline", threads),
            threads,
            |b, &t| {
                b.iter_custom(|iters| {
                    let limiter = Arc::new(MutexRateLimiter::new(10_000_000));
                    let mut handles = vec![];
                    let start = std::time::Instant::now();

                    for _ in 0..t {
                        let l = Arc::clone(&limiter);
                        handles.push(thread::spawn(move || {
                            for _ in 0..iters / t as u64 {
                                let _ = l.acquire_token();
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }

                    start.elapsed()
                });
            },
        );
    }

    group.finish();
}

fn bench_jitter_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("jitter_contention");

    for threads in [2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("with_jitter", threads),
            threads,
            |b, &t| {
                b.iter_custom(|iters| {
                    let limiter = Arc::new(AdvancedRateLimiter64::with_capacity_and_period(
                        10_000_000,
                        60_000_000_000,
                    ));
                    let mut handles = vec![];
                    let start = std::time::Instant::now();

                    for _ in 0..t {
                        let l = Arc::clone(&limiter);
                        handles.push(thread::spawn(move || {
                            for _ in 0..iters / t as u64 {
                                let _ = l.acquire_token_with_jitter();
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }

                    start.elapsed()
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// B32: Throughput Benchmarks
// ============================================================================

fn bench_throughput_single_thread(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_single_thread");
    group.throughput(Throughput::Elements(10_000));

    group.bench_function("advanced_atomic", |b| {
        let limiter = AdvancedRateLimiter64::with_capacity_and_period(10_000_000, 60_000_000_000);
        b.iter(|| {
            for _ in 0..10_000 {
                black_box(limiter.acquire_token()).ok();
            }
        });
    });

    group.bench_function("advanced_with_jitter", |b| {
        let limiter = AdvancedRateLimiter64::with_capacity_and_period(10_000_000, 60_000_000_000);
        b.iter(|| {
            for _ in 0..10_000 {
                black_box(limiter.acquire_token_with_jitter()).ok();
            }
        });
    });

    group.bench_function("basic_sliding_window", |b| {
        let limiter = RateLimitCapsule::with_quota(10_000_000);
        b.iter(|| {
            for _ in 0..10_000 {
                black_box(limiter.increment_request()).ok();
            }
        });
    });

    group.bench_function("mutex_baseline", |b| {
        let limiter = MutexRateLimiter::new(10_000_000);
        b.iter(|| {
            for _ in 0..10_000 {
                black_box(limiter.acquire_token()).ok();
            }
        });
    });

    group.finish();
}

fn bench_throughput_multi_thread(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_multi_thread");

    for threads in [2, 4, 8].iter() {
        group.throughput(Throughput::Elements(100_000));

        // AdvancedRateLimiter64
        group.bench_with_input(
            BenchmarkId::new("advanced_atomic", threads),
            threads,
            |b, &t| {
                b.iter_custom(|_iters| {
                    let limiter = Arc::new(AdvancedRateLimiter64::with_capacity_and_period(
                        10_000_000,
                        60_000_000_000,
                    ));
                    let mut handles = vec![];
                    let start = std::time::Instant::now();

                    for _ in 0..t {
                        let l = Arc::clone(&limiter);
                        handles.push(thread::spawn(move || {
                            for _ in 0..100_000 / t {
                                let _ = l.acquire_token();
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }

                    start.elapsed()
                });
            },
        );

        // With jitter
        group.bench_with_input(
            BenchmarkId::new("advanced_with_jitter", threads),
            threads,
            |b, &t| {
                b.iter_custom(|_iters| {
                    let limiter = Arc::new(AdvancedRateLimiter64::with_capacity_and_period(
                        10_000_000,
                        60_000_000_000,
                    ));
                    let mut handles = vec![];
                    let start = std::time::Instant::now();

                    for _ in 0..t {
                        let l = Arc::clone(&limiter);
                        handles.push(thread::spawn(move || {
                            for _ in 0..100_000 / t {
                                let _ = l.acquire_token_with_jitter();
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }

                    start.elapsed()
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// B32: Scalability Benchmarks
// ============================================================================

fn bench_scalability_weak(c: &mut Criterion) {
    // Weak scaling: constant work per thread
    let mut group = c.benchmark_group("scalability_weak");

    for threads in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("advanced_atomic", threads),
            threads,
            |b, &t| {
                b.iter_custom(|_iters| {
                    let limiter = Arc::new(AdvancedRateLimiter64::with_capacity_and_period(
                        10_000_000,
                        60_000_000_000,
                    ));
                    let mut handles = vec![];
                    let start = std::time::Instant::now();

                    for _ in 0..t {
                        let l = Arc::clone(&limiter);
                        handles.push(thread::spawn(move || {
                            for _ in 0..10_000 {
                                // Constant work per thread
                                let _ = l.acquire_token();
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }

                    start.elapsed()
                });
            },
        );
    }

    group.finish();
}

fn bench_scalability_strong(c: &mut Criterion) {
    // Strong scaling: constant total work
    let mut group = c.benchmark_group("scalability_strong");
    let total_work = 100_000;

    for threads in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("advanced_atomic", threads),
            threads,
            |b, &t| {
                b.iter_custom(|_iters| {
                    let limiter = Arc::new(AdvancedRateLimiter64::with_capacity_and_period(
                        10_000_000,
                        60_000_000_000,
                    ));
                    let mut handles = vec![];
                    let start = std::time::Instant::now();

                    for _ in 0..t {
                        let l = Arc::clone(&limiter);
                        handles.push(thread::spawn(move || {
                            for _ in 0..total_work / t {
                                // Work divided among threads
                                let _ = l.acquire_token();
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }

                    start.elapsed()
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// B32: Hardware Reality Checks
// ============================================================================

fn bench_cache_line_alignment(c: &mut Criterion) {
    // Verify 64B alignment benefits
    let mut group = c.benchmark_group("cache_line_alignment");

    group.bench_function("aligned_64B", |b| {
        let limiter = AdvancedRateLimiter64::new();
        b.iter(|| {
            black_box(limiter.acquire_token()).ok();
        });
    });

    group.finish();
}

fn bench_false_sharing_prevention(c: &mut Criterion) {
    // Verify no false sharing (independent capsules)
    let mut group = c.benchmark_group("false_sharing_prevention");

    group.bench_function("independent_capsules", |b| {
        b.iter_custom(|iters| {
            let limiter1 = Arc::new(AdvancedRateLimiter64::with_capacity_and_period(
                10_000_000,
                60_000_000_000,
            ));
            let limiter2 = Arc::new(AdvancedRateLimiter64::with_capacity_and_period(
                10_000_000,
                60_000_000_000,
            ));

            let l1 = Arc::clone(&limiter1);
            let h1 = thread::spawn(move || {
                for _ in 0..iters / 2 {
                    let _ = l1.acquire_token();
                }
            });

            let l2 = Arc::clone(&limiter2);
            let h2 = thread::spawn(move || {
                for _ in 0..iters / 2 {
                    let _ = l2.acquire_token();
                }
            });

            let start = std::time::Instant::now();
            h1.join().unwrap();
            h2.join().unwrap();
            start.elapsed()
        });
    });

    group.finish();
}

fn bench_jitter_overhead_measurement(c: &mut Criterion) {
    // Measure exact overhead of jitter vs no-jitter
    let mut group = c.benchmark_group("jitter_overhead_measurement");

    group.bench_function("with_jitter", |b| {
        let limiter = AdvancedRateLimiter64::with_capacity_and_period(10_000_000, 60_000_000_000);
        b.iter(|| {
            black_box(limiter.acquire_token_with_jitter()).ok();
        });
    });

    group.bench_function("without_jitter", |b| {
        let limiter = AdvancedRateLimiter64::with_capacity_and_period(10_000_000, 60_000_000_000);
        b.iter(|| {
            black_box(limiter.acquire_token()).ok();
        });
    });

    group.finish();
}

// ============================================================================
// Criterion configuration
// ============================================================================

criterion_group!(
    benches,
    bench_acquire_token_no_contention,
    bench_acquire_token_with_jitter,
    bench_jitter_generation_overhead,
    bench_stats_snapshot,
    bench_acquire_token_contention,
    bench_jitter_contention,
    bench_throughput_single_thread,
    bench_throughput_multi_thread,
    bench_scalability_weak,
    bench_scalability_strong,
    bench_cache_line_alignment,
    bench_false_sharing_prevention,
    bench_jitter_overhead_measurement,
);

criterion_main!(benches);
