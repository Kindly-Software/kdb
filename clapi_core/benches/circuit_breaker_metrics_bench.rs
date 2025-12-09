//! B32-Compliant Benchmark: CircuitBreakerCapsule Performance Validation
//!
//! **Framework**: B32 (32 benchmarking guidelines + 50 hardware reality checks)
//! **Baseline**: No circuit breaker (always allow) - FAIR BASELINE
//! **Comparison**: CircuitBreakerCapsule (64B atomic capsule)
//!
//! ## Architecture Comparison
//!
//! ### Baseline: No Protection (Always Allow)
//! - Check: <1ns (no operation)
//! - Record: <1ns (no operation)
//! - Performance: Minimal overhead (but no protection!)
//!
//! ### CircuitBreakerCapsule: 64B Atomic Capsule
//! - Check: <10ns (single atomic load + bit unpacking)
//! - Record: <20ns (CAS loop with backoff)
//! - State transition: <30ns (CAS loop with generation increment)
//! - Safety: Packed state + generation counters (TOCTOU prevention)
//!
//! ## Expected Results (B32 Reality Checks)
//!
//! | Operation | CircuitBreaker | No Protection | Overhead | Reality Check |
//! |-----------|----------------|---------------|----------|---------------|
//! | allows_operation() | ~10ns | ~1ns | +9ns | K2: Single atomic load |
//! | record_success() | ~20ns | ~1ns | +19ns | K2: CAS loop typical |
//! | record_failure() | ~20ns | ~1ns | +19ns | K2: CAS loop typical |
//! | State transition | ~30ns | N/A | N/A | K2: CAS + generation increment |
//! | Concurrent (4T) | ~40ns | ~1ns | +39ns | K12: Lockfree scaling |
//! | Circuit trip | <5ns | N/A | N/A | K2: Cached state load |
//!
//! **B32 K27 Reality**: 10-20ns overhead for circuit breaker protection is REALISTIC
//! - Single atomic load for check (~10ns)
//! - CAS loop for updates (~20ns typical)
//! - Tradeoff: Safety vs minimal overhead
//!
//! ## B32 Compliance
//!
//! - **B1: Fair Baseline**: No protection (minimal, not strawman mutex)
//! - **B2: Statistical Rigor**: 95% CI, 1000+ samples, Criterion default
//! - **B3: Realistic Workloads**: Production-like failure patterns
//! - **B4: Contention Scenarios**: 1/4/8 thread scaling tests
//! - **B5: Full Disclosure**: Complete methodology documentation

use clapi_core::capsules::circuit_breaker_capsule::CircuitBreakerCapsule;
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ============================================================================
// Baseline: No Circuit Breaker (Always Allow)
// ============================================================================

/// No-op circuit breaker (always allows, no protection)
///
/// **Purpose**: Fair baseline for overhead measurement
/// **Performance**: <1ns per operation (minimal overhead)
/// **Safety**: NONE - always allows operations
struct NoCircuitBreaker {
    _dummy: u8,
}

impl NoCircuitBreaker {
    fn new() -> Self {
        Self { _dummy: 0 }
    }

    #[inline(always)]
    fn allows_operation(&self) -> bool {
        true // Always allow (no protection)
    }

    #[inline(always)]
    fn record_success(&self) {
        // No-op
    }

    #[inline(always)]
    fn record_failure(&self) {
        // No-op
    }
}

// ============================================================================
// B2: Benchmark 1 - Check Operation (allows_operation)
// ============================================================================

/// Benchmark 1: Circuit breaker check operation
///
/// **Expected**: CircuitBreaker ~10ns, NoProtection ~1ns (+9ns overhead)
/// **Reality Check (K2)**: Single atomic load + bit unpacking overhead
fn bench_allows_operation(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker_check");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    // CircuitBreakerCapsule (64B atomic)
    group.bench_function("circuit_breaker_capsule", |b| {
        let breaker = CircuitBreakerCapsule::new();
        b.iter(|| black_box(breaker.allows_operation()))
    });

    // No protection baseline
    group.bench_function("no_protection_baseline", |b| {
        let breaker = NoCircuitBreaker::new();
        b.iter(|| black_box(breaker.allows_operation()))
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 2 - Record Success
// ============================================================================

/// Benchmark 2: Record successful operation
///
/// **Expected**: CircuitBreaker ~20ns, NoProtection ~1ns (+19ns overhead)
/// **Reality Check (K2)**: CAS loop with generation increment
fn bench_record_success(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker_record_success");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    // CircuitBreakerCapsule
    group.bench_function("circuit_breaker_capsule", |b| {
        let breaker = CircuitBreakerCapsule::new();
        b.iter(|| {
            breaker.record_success();
        })
    });

    // No protection baseline
    group.bench_function("no_protection_baseline", |b| {
        let breaker = NoCircuitBreaker::new();
        b.iter(|| {
            breaker.record_success();
        })
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 3 - Record Failure
// ============================================================================

/// Benchmark 3: Record failed operation
///
/// **Expected**: CircuitBreaker ~20ns, NoProtection ~1ns (+19ns overhead)
/// **Reality Check (K2)**: CAS loop with failure counter increment
fn bench_record_failure(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker_record_failure");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    // CircuitBreakerCapsule
    group.bench_function("circuit_breaker_capsule", |b| {
        let breaker = CircuitBreakerCapsule::new();
        b.iter(|| {
            breaker.record_failure();
        })
    });

    // No protection baseline
    group.bench_function("no_protection_baseline", |b| {
        let breaker = NoCircuitBreaker::new();
        b.iter(|| {
            breaker.record_failure();
        })
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 4 - State Snapshot
// ============================================================================

/// Benchmark 4: Get circuit breaker state snapshot
///
/// **Expected**: CircuitBreaker ~30ns (4 atomic loads + unpacking)
/// **Reality Check (K2)**: Multiple atomic loads for full state
fn bench_get_state(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker_get_state");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    group.bench_function("circuit_breaker_capsule", |b| {
        let breaker = CircuitBreakerCapsule::new();
        b.iter(|| black_box(breaker.get_state()))
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 5 - Open Circuit (State Transition)
// ============================================================================

/// Benchmark 5: Manual circuit open (state transition)
///
/// **Expected**: CircuitBreaker ~30ns (CAS loop + generation increment + timestamp)
/// **Reality Check (K2)**: CAS loop with multiple field updates
fn bench_open_circuit(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker_open");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    group.bench_function("circuit_breaker_capsule", |b| {
        let breaker = CircuitBreakerCapsule::new();
        b.iter(|| {
            breaker.open_circuit();
            breaker.reset(); // Reset for next iteration
        })
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 6 - Realistic Failure Pattern
// ============================================================================

/// Benchmark 6: Realistic failure pattern (90% success, 10% failure)
///
/// **Expected**: CircuitBreaker ~18ns average (20ns × 0.9 + 20ns × 0.1)
/// **Reality Check (K27)**: Production-like workload with mixed success/failure
fn bench_realistic_pattern(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker_realistic_pattern");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    group.bench_function("circuit_breaker_capsule", |b| {
        let breaker = CircuitBreakerCapsule::new();
        let mut counter = 0usize;
        b.iter(|| {
            counter += 1;
            if counter % 10 == 0 {
                breaker.record_failure();
            } else {
                breaker.record_success();
            }
        })
    });

    group.finish();
}

// ============================================================================
// B4: Benchmark 7 - Concurrent Check (4 Threads)
// ============================================================================

/// Benchmark 7: Concurrent circuit breaker checks (4 threads)
///
/// **Expected**: CircuitBreaker ~40ns, NoProtection ~1ns (lockfree scaling)
/// **Reality Check (K12)**: Minimal contention on read-only checks
fn bench_concurrent_check_4t(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker_concurrent_check_4t");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(100);

    let num_threads = 4;
    let ops_per_thread = 1000;

    group.throughput(Throughput::Elements((num_threads * ops_per_thread) as u64));

    // CircuitBreakerCapsule
    group.bench_function("circuit_breaker_capsule_4t", |b| {
        let breaker = Arc::new(CircuitBreakerCapsule::new());
        b.iter(|| {
            let handles: Vec<_> = (0..num_threads)
                .map(|_| {
                    let b = Arc::clone(&breaker);
                    thread::spawn(move || {
                        for _ in 0..ops_per_thread {
                            black_box(b.allows_operation());
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    // No protection baseline
    group.bench_function("no_protection_baseline_4t", |b| {
        let breaker = Arc::new(NoCircuitBreaker::new());
        b.iter(|| {
            let handles: Vec<_> = (0..num_threads)
                .map(|_| {
                    let b = Arc::clone(&breaker);
                    thread::spawn(move || {
                        for _ in 0..ops_per_thread {
                            black_box(b.allows_operation());
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// B4: Benchmark 8 - Concurrent Record (4 Threads)
// ============================================================================

/// Benchmark 8: Concurrent record operations (4 threads)
///
/// **Expected**: CircuitBreaker ~50ns, NoProtection ~1ns (CAS contention)
/// **Reality Check (K12)**: Moderate contention on CAS writes
fn bench_concurrent_record_4t(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker_concurrent_record_4t");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(100);

    let num_threads = 4;
    let ops_per_thread = 1000;

    group.throughput(Throughput::Elements((num_threads * ops_per_thread) as u64));

    // CircuitBreakerCapsule
    group.bench_function("circuit_breaker_capsule_4t", |b| {
        let breaker = Arc::new(CircuitBreakerCapsule::new());
        b.iter(|| {
            let handles: Vec<_> = (0..num_threads)
                .map(|tid| {
                    let b = Arc::clone(&breaker);
                    thread::spawn(move || {
                        for i in 0..ops_per_thread {
                            // 90% success, 10% failure (realistic pattern)
                            if (tid * 1000 + i) % 10 == 0 {
                                b.record_failure();
                            } else {
                                b.record_success();
                            }
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// B4: Benchmark 9 - Concurrent Record (8 Threads)
// ============================================================================

/// Benchmark 9: Concurrent record operations (8 threads)
///
/// **Expected**: CircuitBreaker ~80ns (higher CAS contention)
/// **Reality Check (K12)**: Lockfree scaling degrades beyond 4 threads
fn bench_concurrent_record_8t(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker_concurrent_record_8t");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(100);

    let num_threads = 8;
    let ops_per_thread = 500;

    group.throughput(Throughput::Elements((num_threads * ops_per_thread) as u64));

    group.bench_function("circuit_breaker_capsule_8t", |b| {
        let breaker = Arc::new(CircuitBreakerCapsule::new());
        b.iter(|| {
            let handles: Vec<_> = (0..num_threads)
                .map(|tid| {
                    let b = Arc::clone(&breaker);
                    thread::spawn(move || {
                        for i in 0..ops_per_thread {
                            if (tid * 500 + i) % 10 == 0 {
                                b.record_failure();
                            } else {
                                b.record_success();
                            }
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 10 - Circuit Trip Detection
// ============================================================================

/// Benchmark 10: Circuit trip detection (cache-hot state check)
///
/// **Expected**: CircuitBreaker <5ns (cached state load)
/// **Reality Check (K2)**: L1 cache hit for recently checked state
fn bench_circuit_trip_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker_trip_check");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    group.bench_function("circuit_breaker_capsule_closed", |b| {
        let breaker = CircuitBreakerCapsule::new();
        b.iter(|| black_box(breaker.allows_operation()))
    });

    group.bench_function("circuit_breaker_capsule_open", |b| {
        let breaker = CircuitBreakerCapsule::new();
        breaker.open_circuit();
        b.iter(|| black_box(breaker.allows_operation()))
    });

    group.finish();
}

// ============================================================================
// B2: Criterion Configuration (Statistical Rigor)
// ============================================================================

criterion_group! {
    name = benches;
    config = Criterion::default()
        .confidence_level(0.95)      // B2: 95% confidence intervals
        .significance_level(0.05)
        .noise_threshold(0.05);
    targets =
        bench_allows_operation,
        bench_record_success,
        bench_record_failure,
        bench_get_state,
        bench_open_circuit,
        bench_realistic_pattern,
        bench_concurrent_check_4t,
        bench_concurrent_record_4t,
        bench_concurrent_record_8t,
        bench_circuit_trip_check
}

criterion_main!(benches);
