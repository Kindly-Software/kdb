//! Concurrent Load Benchmark - Hash Performance Under Realistic Load
//!
//! ## Purpose
//! Measure hash performance under realistic clapi_core load conditions:
//! - 1000s of concurrent requests/sec
//! - 8+ threads competing for CPU
//! - Hash verification on every request
//!
//! ## Why This Matters
//! Single-threaded micro-benchmarks show SIMD slower (1.69ns vs 1.64ns for 4 fields)
//! due to setup overhead. But under load, SIMD may win due to:
//! - Fewer total instructions (less CPU contention)
//! - Better instruction pipeline utilization
//! - Less cache pollution
//! - Thermal efficiency
//!
//! ## Test Scenarios
//! 1. Single-threaded baseline (reconfirm micro-benchmark)
//! 2. 4 threads (typical server)
//! 3. 8 threads (saturated CPU)
//! 4. 16 threads (over-subscribed, contention)
//!
//! ## Success Criteria
//! - If SIMD wins at 8+ threads: Enable simd-hashing by default
//! - If scalar wins at all thread counts: Keep scalar, disable SIMD
//! - Measure p50, p99, p999 latency (not just mean)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[cfg(feature = "simd-hashing")]
use atomic_capsule::hash::simd_fast_hash_multi;

use atomic_capsule::hash::{best_hash, scalar_fast_hash};

/// Simulate realistic clapi_core request pattern
///
/// Each request:
/// 1. Load 6 fields from capsule (budget, spent, count, generation, deductions, failures)
/// 2. Compute hash
/// 3. Store hash atomically
#[derive(Clone)]
struct RequestCapsuleMock {
    budget_cents: Arc<AtomicU64>,
    total_spent: Arc<AtomicU64>,
    request_count: Arc<AtomicU64>,
    generation: Arc<AtomicU64>,
    deduction_count: Arc<AtomicU64>,
    failed_deductions: Arc<AtomicU64>,
    hash: Arc<AtomicU64>,
}

impl RequestCapsuleMock {
    fn new() -> Self {
        Self {
            budget_cents: Arc::new(AtomicU64::new(100_00)),
            total_spent: Arc::new(AtomicU64::new(0)),
            request_count: Arc::new(AtomicU64::new(0)),
            generation: Arc::new(AtomicU64::new(1)),
            deduction_count: Arc::new(AtomicU64::new(0)),
            failed_deductions: Arc::new(AtomicU64::new(0)),
            hash: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Scalar hash (baseline)
    fn compute_hash_scalar(&self) -> u64 {
        let fields = [
            self.budget_cents.load(Ordering::Relaxed),
            self.total_spent.load(Ordering::Relaxed),
            self.request_count.load(Ordering::Relaxed),
            self.generation.load(Ordering::Relaxed),
            self.deduction_count.load(Ordering::Relaxed),
            self.failed_deductions.load(Ordering::Relaxed),
        ];
        scalar_fast_hash(&fields)
    }

    /// SIMD hash (optimized)
    #[cfg(feature = "simd-hashing")]
    fn compute_hash_simd(&self) -> u64 {
        let fields = [
            self.budget_cents.load(Ordering::Relaxed),
            self.total_spent.load(Ordering::Relaxed),
            self.request_count.load(Ordering::Relaxed),
            self.generation.load(Ordering::Relaxed),
            self.deduction_count.load(Ordering::Relaxed),
            self.failed_deductions.load(Ordering::Relaxed),
        ];
        simd_fast_hash_multi(&fields)
    }

    /// Best hash (automatic dispatch)
    fn compute_hash_best(&self) -> u64 {
        let fields = [
            self.budget_cents.load(Ordering::Relaxed),
            self.total_spent.load(Ordering::Relaxed),
            self.request_count.load(Ordering::Relaxed),
            self.generation.load(Ordering::Relaxed),
            self.deduction_count.load(Ordering::Relaxed),
            self.failed_deductions.load(Ordering::Relaxed),
        ];
        best_hash(&fields)
    }

    /// Simulate full request: load + hash + store
    fn process_request_scalar(&self) {
        let hash = self.compute_hash_scalar();
        self.hash.store(hash, Ordering::Relaxed);
    }

    #[cfg(feature = "simd-hashing")]
    fn process_request_simd(&self) {
        let hash = self.compute_hash_simd();
        self.hash.store(hash, Ordering::Relaxed);
    }

    fn process_request_best(&self) {
        let hash = self.compute_hash_best();
        self.hash.store(hash, Ordering::Relaxed);
    }
}

// ============================================================================
// Benchmark 1: Single-Threaded Baseline (Reconfirm Micro-Benchmark)
// ============================================================================

fn bench_single_thread_scalar(c: &mut Criterion) {
    let capsule = RequestCapsuleMock::new();

    c.bench_function("single_thread_scalar", |b| {
        b.iter(|| capsule.process_request_scalar())
    });
}

#[cfg(feature = "simd-hashing")]
fn bench_single_thread_simd(c: &mut Criterion) {
    let capsule = RequestCapsuleMock::new();

    c.bench_function("single_thread_simd", |b| {
        b.iter(|| capsule.process_request_simd())
    });
}

fn bench_single_thread_best(c: &mut Criterion) {
    let capsule = RequestCapsuleMock::new();

    c.bench_function("single_thread_best", |b| {
        b.iter(|| capsule.process_request_best())
    });
}

// ============================================================================
// Benchmark 2: Concurrent Load (4/8/16 Threads)
// ============================================================================

/// Concurrent throughput test
///
/// Spawns N threads, each processing 100K requests
/// Measures total throughput (requests/sec)
fn bench_concurrent_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_load");
    group.warm_up_time(Duration::from_secs(5));
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(20); // Fewer samples for longer benchmarks

    for thread_count in [1, 4, 8, 16].iter() {
        let requests_per_thread = 100_000;
        group.throughput(Throughput::Elements(
            (thread_count * requests_per_thread) as u64,
        ));

        // Scalar baseline
        group.bench_with_input(
            BenchmarkId::new("scalar", thread_count),
            thread_count,
            |b, &threads| {
                b.iter(|| {
                    let capsule = RequestCapsuleMock::new();
                    let handles: Vec<_> = (0..threads)
                        .map(|_| {
                            let capsule_clone = capsule.clone();
                            thread::spawn(move || {
                                for _ in 0..requests_per_thread {
                                    capsule_clone.process_request_scalar();
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                })
            },
        );

        // SIMD optimized
        #[cfg(feature = "simd-hashing")]
        group.bench_with_input(
            BenchmarkId::new("simd", thread_count),
            thread_count,
            |b, &threads| {
                b.iter(|| {
                    let capsule = RequestCapsuleMock::new();
                    let handles: Vec<_> = (0..threads)
                        .map(|_| {
                            let capsule_clone = capsule.clone();
                            thread::spawn(move || {
                                for _ in 0..requests_per_thread {
                                    capsule_clone.process_request_simd();
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                })
            },
        );

        // Best hash (automatic)
        group.bench_with_input(
            BenchmarkId::new("best", thread_count),
            thread_count,
            |b, &threads| {
                b.iter(|| {
                    let capsule = RequestCapsuleMock::new();
                    let handles: Vec<_> = (0..threads)
                        .map(|_| {
                            let capsule_clone = capsule.clone();
                            thread::spawn(move || {
                                for _ in 0..requests_per_thread {
                                    capsule_clone.process_request_best();
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark 3: Sustained Load (60 seconds, measure p99/p999)
// ============================================================================

/// Sustained load test (production-like)
///
/// Run for 60 seconds at 10K req/s
/// Measure latency distribution (p50, p99, p999)
fn bench_sustained_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("sustained_load");
    group.warm_up_time(Duration::from_secs(10));
    group.measurement_time(Duration::from_secs(60));
    group.sample_size(10);

    let requests_per_sec = 10_000;
    let duration_secs = 60;
    let total_requests = requests_per_sec * duration_secs;

    group.throughput(Throughput::Elements(total_requests as u64));

    // Scalar
    group.bench_function("scalar_sustained", |b| {
        b.iter(|| {
            let capsule = RequestCapsuleMock::new();
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let capsule_clone = capsule.clone();
                    thread::spawn(move || {
                        for _ in 0..(total_requests / 8) {
                            capsule_clone.process_request_scalar();
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        })
    });

    // SIMD
    #[cfg(feature = "simd-hashing")]
    group.bench_function("simd_sustained", |b| {
        b.iter(|| {
            let capsule = RequestCapsuleMock::new();
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let capsule_clone = capsule.clone();
                    thread::spawn(move || {
                        for _ in 0..(total_requests / 8) {
                            capsule_clone.process_request_simd();
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        })
    });

    group.finish();
}

// ============================================================================
// Benchmark 4: Cache Pressure Test
// ============================================================================

/// Test with large working set (cache pollution)
///
/// Simulate 10K concurrent budgets (realistic clapi_core scale)
/// Each thread randomly accesses different capsules
fn bench_cache_pressure(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_pressure");
    group.warm_up_time(Duration::from_secs(5));
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(10);

    let capsule_count = 10_000;
    let capsules: Vec<RequestCapsuleMock> = (0..capsule_count)
        .map(|_| RequestCapsuleMock::new())
        .collect();
    let capsules_arc = Arc::new(capsules);

    // Scalar
    group.bench_function("scalar_cache_pressure", |b| {
        b.iter(|| {
            let capsules_clone = Arc::clone(&capsules_arc);
            let handles: Vec<_> = (0..8)
                .map(|thread_id| {
                    let capsules_thread = Arc::clone(&capsules_clone);
                    thread::spawn(move || {
                        for i in 0..10_000 {
                            let idx = (thread_id * 1250 + i) % capsule_count;
                            capsules_thread[idx].process_request_scalar();
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        })
    });

    // SIMD
    #[cfg(feature = "simd-hashing")]
    group.bench_function("simd_cache_pressure", |b| {
        b.iter(|| {
            let capsules_clone = Arc::clone(&capsules_arc);
            let handles: Vec<_> = (0..8)
                .map(|thread_id| {
                    let capsules_thread = Arc::clone(&capsules_clone);
                    thread::spawn(move || {
                        for i in 0..10_000 {
                            let idx = (thread_id * 1250 + i) % capsule_count;
                            capsules_thread[idx].process_request_simd();
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        })
    });

    group.finish();
}

#[cfg(feature = "simd-hashing")]
criterion_group!(
    benches,
    bench_single_thread_scalar,
    bench_single_thread_simd,
    bench_single_thread_best,
    bench_concurrent_load,
    bench_sustained_load,
    bench_cache_pressure,
);

#[cfg(not(feature = "simd-hashing"))]
criterion_group!(
    benches,
    bench_single_thread_scalar,
    bench_single_thread_best,
    bench_concurrent_load,
);

criterion_main!(benches);
