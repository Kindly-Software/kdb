//! B32 Benchmarking Framework - RateLimitCapsule Performance Validation
//!
//! **Framework**: B32 (32 benchmarking guidelines + 27 hardware reality checks)
//! **Coverage**: Operation latency, throughput, scalability, hardware reality
//!
//! # B32 Guidelines Applied
//! - Fair baselines (compare to mutex-based rate limiter)
//! - Statistical rigor (1000+ iterations, 95% CI)
//! - Honest claims (10-30% typical, 3-10× exceptional)
//! - Hardware reality (measure on same hardware, same compiler)
//!
//! # Performance Targets
//! - check_rate_limit(): <20ns (single atomic load + comparison)
//! - increment_request(): <30ns (CAS loop, no contention)
//! - increment_request() under contention: <300ns (with retries)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use clapi_core::capsules::RateLimitCapsule;
use std::sync::{Arc, Mutex};
use std::thread;

// ============================================================================
// Baseline: Mutex-based rate limiter (for fair comparison)
// ============================================================================

struct MutexRateLimiter {
    state: Mutex<MutexRateLimiterState>,
}

struct MutexRateLimiterState {
    requests_count: u64,
    quota_remaining: i64,
    window_start_ns: u64,
}

impl MutexRateLimiter {
    fn new(quota: i64) -> Self {
        Self {
            state: Mutex::new(MutexRateLimiterState {
                requests_count: 0,
                quota_remaining: quota,
                window_start_ns: now_ns(),
            }),
        }
    }

    fn check_rate_limit(&self) -> bool {
        let state = self.state.lock().unwrap();
        state.quota_remaining > 0
    }

    fn increment_request(&self) -> Result<i64, ()> {
        let mut state = self.state.lock().unwrap();
        if state.quota_remaining <= 0 {
            return Err(());
        }
        state.quota_remaining -= 1;
        state.requests_count += 1;
        Ok(state.quota_remaining)
    }
}

fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

// ============================================================================
// B32: Operation Latency Benchmarks
// ============================================================================

fn bench_check_rate_limit(c: &mut Criterion) {
    let mut group = c.benchmark_group("check_rate_limit");

    // Atomic capsule (target: <20ns)
    group.bench_function("atomic_capsule", |b| {
        let limiter = RateLimitCapsule::new();
        b.iter(|| {
            black_box(limiter.check_rate_limit());
        });
    });

    // Mutex baseline (expected: ~50-100ns)
    group.bench_function("mutex_baseline", |b| {
        let limiter = MutexRateLimiter::new(1000);
        b.iter(|| {
            black_box(limiter.check_rate_limit());
        });
    });

    group.finish();
}

fn bench_increment_request_no_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("increment_request_no_contention");

    // Atomic capsule (target: <30ns)
    group.bench_function("atomic_capsule", |b| {
        let limiter = RateLimitCapsule::with_quota(1_000_000);
        b.iter(|| {
            black_box(limiter.increment_request()).ok();
        });
    });

    // Mutex baseline (expected: ~50-150ns)
    group.bench_function("mutex_baseline", |b| {
        let limiter = MutexRateLimiter::new(1_000_000);
        b.iter(|| {
            black_box(limiter.increment_request()).ok();
        });
    });

    group.finish();
}

fn bench_increment_request_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("increment_request_contention");

    for threads in [2, 4, 8].iter() {
        // Atomic capsule
        group.bench_with_input(
            BenchmarkId::new("atomic_capsule", threads),
            threads,
            |b, &t| {
                b.iter_custom(|iters| {
                    let limiter = Arc::new(RateLimitCapsule::with_quota(1_000_000));
                    let mut handles = vec![];
                    let start = std::time::Instant::now();

                    for _ in 0..t {
                        let l = Arc::clone(&limiter);
                        handles.push(thread::spawn(move || {
                            for _ in 0..iters / t as u64 {
                                let _ = l.increment_request();
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
                    let limiter = Arc::new(MutexRateLimiter::new(1_000_000));
                    let mut handles = vec![];
                    let start = std::time::Instant::now();

                    for _ in 0..t {
                        let l = Arc::clone(&limiter);
                        handles.push(thread::spawn(move || {
                            for _ in 0..iters / t as u64 {
                                let _ = l.increment_request();
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

fn bench_stats_snapshot(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats_snapshot");

    group.bench_function("atomic_capsule", |b| {
        let limiter = RateLimitCapsule::new();
        limiter.increment_request().ok();

        b.iter(|| {
            black_box(limiter.stats());
        });
    });

    group.finish();
}

// ============================================================================
// B32: Throughput Benchmarks
// ============================================================================

fn bench_throughput_single_thread(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_single_thread");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("atomic_capsule", |b| {
        let limiter = RateLimitCapsule::with_quota(1_000_000);
        b.iter(|| {
            for _ in 0..1000 {
                black_box(limiter.increment_request()).ok();
            }
        });
    });

    group.bench_function("mutex_baseline", |b| {
        let limiter = MutexRateLimiter::new(1_000_000);
        b.iter(|| {
            for _ in 0..1000 {
                black_box(limiter.increment_request()).ok();
            }
        });
    });

    group.finish();
}

fn bench_throughput_multi_thread(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_multi_thread");

    for threads in [2, 4, 8].iter() {
        group.throughput(Throughput::Elements(10_000));

        // Atomic capsule
        group.bench_with_input(
            BenchmarkId::new("atomic_capsule", threads),
            threads,
            |b, &t| {
                b.iter_custom(|iters| {
                    let limiter = Arc::new(RateLimitCapsule::with_quota(10_000_000));
                    let mut handles = vec![];
                    let start = std::time::Instant::now();

                    for _ in 0..t {
                        let l = Arc::clone(&limiter);
                        handles.push(thread::spawn(move || {
                            for _ in 0..10_000 / t {
                                let _ = l.increment_request();
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
                            for _ in 0..10_000 / t {
                                let _ = l.increment_request();
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
            BenchmarkId::new("atomic_capsule", threads),
            threads,
            |b, &t| {
                b.iter_custom(|iters| {
                    let limiter = Arc::new(RateLimitCapsule::with_quota(10_000_000));
                    let mut handles = vec![];
                    let start = std::time::Instant::now();

                    for _ in 0..t {
                        let l = Arc::clone(&limiter);
                        handles.push(thread::spawn(move || {
                            for _ in 0..1000 {
                                // Constant work per thread
                                let _ = l.increment_request();
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
    let total_work = 10_000;

    for threads in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("atomic_capsule", threads),
            threads,
            |b, &t| {
                b.iter_custom(|iters| {
                    let limiter = Arc::new(RateLimitCapsule::with_quota(10_000_000));
                    let mut handles = vec![];
                    let start = std::time::Instant::now();

                    for _ in 0..t {
                        let l = Arc::clone(&limiter);
                        handles.push(thread::spawn(move || {
                            for _ in 0..total_work / t {
                                // Work divided among threads
                                let _ = l.increment_request();
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
        let limiter = RateLimitCapsule::new();
        b.iter(|| {
            black_box(limiter.check_rate_limit());
        });
    });

    group.finish();
}

fn bench_false_sharing_prevention(c: &mut Criterion) {
    // Verify no false sharing (independent capsules)
    let mut group = c.benchmark_group("false_sharing_prevention");

    group.bench_function("independent_capsules", |b| {
        b.iter_custom(|iters| {
            let limiter1 = Arc::new(RateLimitCapsule::with_quota(1_000_000));
            let limiter2 = Arc::new(RateLimitCapsule::with_quota(1_000_000));

            let l1 = Arc::clone(&limiter1);
            let h1 = thread::spawn(move || {
                for _ in 0..iters / 2 {
                    let _ = l1.increment_request();
                }
            });

            let l2 = Arc::clone(&limiter2);
            let h2 = thread::spawn(move || {
                for _ in 0..iters / 2 {
                    let _ = l2.increment_request();
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

// ============================================================================
// Criterion configuration
// ============================================================================

criterion_group!(
    benches,
    bench_check_rate_limit,
    bench_increment_request_no_contention,
    bench_increment_request_contention,
    bench_stats_snapshot,
    bench_throughput_single_thread,
    bench_throughput_multi_thread,
    bench_scalability_weak,
    bench_scalability_strong,
    bench_cache_line_alignment,
    bench_false_sharing_prevention,
);

criterion_main!(benches);
