//! B32-Compliant Benchmark: Provider Circuit Status & Routing Performance
//!
//! **Framework**: B32 (32 benchmarking guidelines + 50 hardware reality checks)
//! **Baseline**: Direct provider selection (no health tracking) - FAIR BASELINE
//! **Comparison**: RoutingCapsule128 with health-aware selection
//!
//! ## Architecture Comparison
//!
//! ### Baseline: Direct Provider Selection
//! - Selection: <5ns (array index lookup)
//! - No health tracking
//! - No failover logic
//! - Performance: Minimal overhead (but no resilience!)
//!
//! ### RoutingCapsule128: Health-Aware Routing (128B Atomic Capsule)
//! - Selection: <80ns (atomic load + state check + failover logic)
//! - Health tracking: Packed AtomicU64 (provider_id | state | latency | generation)
//! - Automatic failover: Primary → Fallback transition
//! - Generation counter: TOCTOU prevention
//!
//! ## Expected Results (B32 Reality Checks)
//!
//! | Operation | RoutingCapsule | Direct | Overhead | Reality Check |
//! |-----------|----------------|--------|----------|---------------|
//! | select_provider() | ~80ns | ~5ns | +75ns | K2: 2 atomic loads + state check |
//! | update_state() | ~30ns | N/A | N/A | K2: CAS loop with generation |
//! | get_primary_id() | ~10ns | ~5ns | +5ns | K2: Single atomic load + shift |
//! | Concurrent (4T) | ~120ns | ~5ns | +115ns | K12: Lockfree routing under contention |
//! | Failover check | ~15ns | N/A | N/A | K2: State unpacking + comparison |
//!
//! **B32 K27 Reality**: 80ns routing overhead is REALISTIC for health-aware selection
//! - 2 atomic loads (primary + fallback state): ~10ns each
//! - State unpacking + health check: ~30ns
//! - Request counter increment: ~20ns
//! - Tradeoff: Resilience vs minimal overhead
//!
//! ## B32 Compliance
//!
//! - **B1: Fair Baseline**: Direct selection (minimal, not strawman)
//! - **B2: Statistical Rigor**: 95% CI, 1000+ samples, Criterion default
//! - **B3: Realistic Workloads**: Production-like routing patterns
//! - **B4: Contention Scenarios**: 1/4/8 thread scaling tests
//! - **B5: Full Disclosure**: Complete methodology documentation

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use clapi_core::capsules::{ProviderState, RoutingCapsule128};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ============================================================================
// Baseline: Direct Provider Selection (No Health Tracking)
// ============================================================================

/// Direct provider selection (no health tracking, no failover)
///
/// **Purpose**: Fair baseline for overhead measurement
/// **Performance**: <5ns per selection (array index)
/// **Safety**: NONE - no failover, no health awareness
struct DirectProviderSelection {
    primary_id: u16,
    fallback_id: u16,
}

impl DirectProviderSelection {
    fn new(primary_id: u16, fallback_id: u16) -> Self {
        Self {
            primary_id,
            fallback_id,
        }
    }

    #[inline(always)]
    fn select_provider(&self) -> u16 {
        self.primary_id // Always return primary (no health check)
    }

    #[inline(always)]
    fn get_primary_id(&self) -> u16 {
        self.primary_id
    }

    #[inline(always)]
    fn get_fallback_id(&self) -> u16 {
        self.fallback_id
    }
}

// ============================================================================
// B2: Benchmark 1 - Provider Selection (Primary Healthy)
// ============================================================================

/// Benchmark 1: Provider selection with healthy primary
///
/// **Expected**: RoutingCapsule ~80ns, Direct ~5ns (+75ns overhead)
/// **Reality Check (K2)**: 2 atomic loads + state check + counter increment
fn bench_select_provider_healthy(c: &mut Criterion) {
    let mut group = c.benchmark_group("provider_select_healthy");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    // RoutingCapsule128
    group.bench_function("routing_capsule_128", |b| {
        let capsule = RoutingCapsule128::new(0, 1);
        b.iter(|| black_box(capsule.select_provider()))
    });

    // Direct selection baseline
    group.bench_function("direct_selection_baseline", |b| {
        let direct = DirectProviderSelection::new(0, 1);
        b.iter(|| black_box(direct.select_provider()))
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 2 - Provider Selection (Primary Unavailable, Failover)
// ============================================================================

/// Benchmark 2: Provider selection with primary unavailable (failover)
///
/// **Expected**: RoutingCapsule ~90ns (primary check + fallback check)
/// **Reality Check (K2)**: 2 atomic loads + 2 state checks + counter increment
fn bench_select_provider_failover(c: &mut Criterion) {
    let mut group = c.benchmark_group("provider_select_failover");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    // RoutingCapsule128 (primary unavailable)
    group.bench_function("routing_capsule_128_failover", |b| {
        let capsule = RoutingCapsule128::new(0, 1);
        capsule.update_state(0, ProviderState::Unavailable, 0); // Mark primary unavailable
        b.iter(|| black_box(capsule.select_provider()))
    });

    // Direct selection baseline (no failover logic)
    group.bench_function("direct_selection_baseline", |b| {
        let direct = DirectProviderSelection::new(0, 1);
        b.iter(|| black_box(direct.select_provider()))
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 3 - Provider Selection (All Unavailable)
// ============================================================================

/// Benchmark 3: Provider selection with all providers unavailable
///
/// **Expected**: RoutingCapsule ~100ns (primary + fallback check + error path)
/// **Reality Check (K2)**: 2 atomic loads + 2 state checks + failure counter
fn bench_select_provider_all_unavailable(c: &mut Criterion) {
    let mut group = c.benchmark_group("provider_select_all_unavailable");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    group.bench_function("routing_capsule_128_all_unavailable", |b| {
        let capsule = RoutingCapsule128::new(0, 1);
        capsule.update_state(0, ProviderState::Unavailable, 0);
        capsule.update_state(1, ProviderState::CircuitOpen, 0);
        b.iter(|| black_box(capsule.select_provider()))
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 4 - Update Provider State
// ============================================================================

/// Benchmark 4: Update provider health state
///
/// **Expected**: RoutingCapsule ~30ns (CAS loop + generation increment)
/// **Reality Check (K2)**: CAS loop with packed state update
fn bench_update_state(c: &mut Criterion) {
    let mut group = c.benchmark_group("provider_update_state");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    group.bench_function("routing_capsule_128", |b| {
        let capsule = RoutingCapsule128::new(0, 1);
        let mut counter = 0u16;
        b.iter(|| {
            counter += 1;
            let state = if counter % 2 == 0 {
                ProviderState::Healthy
            } else {
                ProviderState::Degraded
            };
            capsule.update_state(0, state, counter % 1000);
        })
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 5 - Get Provider ID
// ============================================================================

/// Benchmark 5: Get primary/fallback provider ID
///
/// **Expected**: RoutingCapsule ~10ns (atomic load + bit shift)
/// **Reality Check (K2)**: Single atomic load + unpacking
fn bench_get_provider_id(c: &mut Criterion) {
    let mut group = c.benchmark_group("provider_get_id");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    // RoutingCapsule128
    group.bench_function("routing_capsule_128_get_primary", |b| {
        let capsule = RoutingCapsule128::new(0, 1);
        b.iter(|| black_box(capsule.get_primary_id()))
    });

    group.bench_function("routing_capsule_128_get_fallback", |b| {
        let capsule = RoutingCapsule128::new(0, 1);
        b.iter(|| black_box(capsule.get_fallback_id()))
    });

    // Direct selection baseline
    group.bench_function("direct_selection_baseline", |b| {
        let direct = DirectProviderSelection::new(0, 1);
        b.iter(|| black_box(direct.get_primary_id()))
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 6 - Get Request/Failure Counts
// ============================================================================

/// Benchmark 6: Get request and failure counters
///
/// **Expected**: RoutingCapsule ~10ns (single atomic load)
/// **Reality Check (K2)**: Relaxed atomic load overhead
fn bench_get_counters(c: &mut Criterion) {
    let mut group = c.benchmark_group("provider_get_counters");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    group.bench_function("routing_capsule_128_request_count", |b| {
        let capsule = RoutingCapsule128::new(0, 1);
        // Pre-populate counters
        for _ in 0..100 {
            let _ = capsule.select_provider();
        }
        b.iter(|| black_box(capsule.request_count()))
    });

    group.bench_function("routing_capsule_128_failure_count", |b| {
        let capsule = RoutingCapsule128::new(0, 1);
        b.iter(|| black_box(capsule.failure_count()))
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 7 - Realistic Routing Pattern (90% Primary, 10% Failover)
// ============================================================================

/// Benchmark 7: Realistic routing pattern with occasional failover
///
/// **Expected**: RoutingCapsule ~82ns average (80ns × 0.9 + 90ns × 0.1)
/// **Reality Check (K27)**: Production-like routing with degraded primary
fn bench_realistic_routing_pattern(c: &mut Criterion) {
    let mut group = c.benchmark_group("provider_realistic_pattern");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    group.bench_function("routing_capsule_128", |b| {
        let capsule = RoutingCapsule128::new(0, 1);
        let mut counter = 0usize;
        b.iter(|| {
            counter += 1;

            // 90% healthy primary, 10% degraded (triggers health check logic)
            if counter % 10 == 0 {
                capsule.update_state(0, ProviderState::Degraded, 1500);
            } else {
                capsule.update_state(0, ProviderState::Healthy, 100);
            }

            black_box(capsule.select_provider())
        })
    });

    group.finish();
}

// ============================================================================
// B4: Benchmark 8 - Concurrent Selection (4 Threads)
// ============================================================================

/// Benchmark 8: Concurrent provider selection (4 threads)
///
/// **Expected**: RoutingCapsule ~120ns, Direct ~5ns (lockfree scaling)
/// **Reality Check (K12)**: Minimal contention on read-heavy workload
fn bench_concurrent_selection_4t(c: &mut Criterion) {
    let mut group = c.benchmark_group("provider_concurrent_selection_4t");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(100);

    let num_threads = 4;
    let ops_per_thread = 1000;

    group.throughput(Throughput::Elements((num_threads * ops_per_thread) as u64));

    // RoutingCapsule128
    group.bench_function("routing_capsule_128_4t", |b| {
        let capsule = Arc::new(RoutingCapsule128::new(0, 1));
        b.iter(|| {
            let handles: Vec<_> = (0..num_threads)
                .map(|_| {
                    let c = Arc::clone(&capsule);
                    thread::spawn(move || {
                        for _ in 0..ops_per_thread {
                            let _ = c.select_provider();
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    // Direct selection baseline
    group.bench_function("direct_selection_baseline_4t", |b| {
        let direct = Arc::new(DirectProviderSelection::new(0, 1));
        b.iter(|| {
            let handles: Vec<_> = (0..num_threads)
                .map(|_| {
                    let d = Arc::clone(&direct);
                    thread::spawn(move || {
                        for _ in 0..ops_per_thread {
                            black_box(d.select_provider());
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
// B4: Benchmark 9 - Concurrent Update State (4 Threads)
// ============================================================================

/// Benchmark 9: Concurrent state updates (4 threads)
///
/// **Expected**: RoutingCapsule ~60ns (CAS contention on state updates)
/// **Reality Check (K12)**: Moderate contention on write-heavy workload
fn bench_concurrent_update_4t(c: &mut Criterion) {
    let mut group = c.benchmark_group("provider_concurrent_update_4t");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(100);

    let num_threads = 4;
    let ops_per_thread = 1000;

    group.throughput(Throughput::Elements((num_threads * ops_per_thread) as u64));

    group.bench_function("routing_capsule_128_4t", |b| {
        let capsule = Arc::new(RoutingCapsule128::new(0, 1));
        b.iter(|| {
            let handles: Vec<_> = (0..num_threads)
                .map(|tid| {
                    let c = Arc::clone(&capsule);
                    thread::spawn(move || {
                        for i in 0..ops_per_thread {
                            let provider_id = if tid % 2 == 0 { 0 } else { 1 };
                            let state = if i % 10 == 0 {
                                ProviderState::Degraded
                            } else {
                                ProviderState::Healthy
                            };
                            let latency = (tid * 100 + i % 500) as u16;
                            c.update_state(provider_id, state, latency);
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
// B4: Benchmark 10 - Concurrent Mixed Workload (8 Threads)
// ============================================================================

/// Benchmark 10: Concurrent mixed workload (80% reads, 20% writes, 8 threads)
///
/// **Expected**: RoutingCapsule ~100ns average (80ns × 0.8 + 60ns × 0.2)
/// **Reality Check (K12)**: Lockfree scaling with read-heavy mixed workload
fn bench_concurrent_mixed_8t(c: &mut Criterion) {
    let mut group = c.benchmark_group("provider_concurrent_mixed_8t");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(100);

    let num_threads = 8;
    let ops_per_thread = 500;

    group.throughput(Throughput::Elements((num_threads * ops_per_thread) as u64));

    group.bench_function("routing_capsule_128_8t", |b| {
        let capsule = Arc::new(RoutingCapsule128::new(0, 1));
        b.iter(|| {
            let handles: Vec<_> = (0..num_threads)
                .map(|tid| {
                    let c = Arc::clone(&capsule);
                    thread::spawn(move || {
                        for i in 0..ops_per_thread {
                            // 80% reads (select), 20% writes (update)
                            if i % 5 == 0 {
                                let state = if i % 10 == 0 {
                                    ProviderState::Degraded
                                } else {
                                    ProviderState::Healthy
                                };
                                c.update_state(tid as u16 % 2, state, (i % 1000) as u16);
                            } else {
                                let _ = c.select_provider();
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
// B2: Benchmark 11 - Circuit Generation Increment
// ============================================================================

/// Benchmark 11: Circuit breaker generation counter increment
///
/// **Expected**: RoutingCapsule ~15ns (atomic load + comparison)
/// **Reality Check (K2)**: Single atomic load overhead
fn bench_circuit_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("provider_circuit_generation");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    group.bench_function("routing_capsule_128", |b| {
        let capsule = RoutingCapsule128::new(0, 1);
        b.iter(|| black_box(capsule.circuit_generation()))
    });

    group.finish();
}

// ============================================================================
// B3: Benchmark 12 - Scaling Validation (1/2/4/8 Threads)
// ============================================================================

/// Benchmark 12: Scaling validation across thread counts
///
/// **Expected**: Linear scaling up to 8 threads for read-heavy workload
/// **Reality Check (K23)**: Lockfree should scale near-linearly for reads
fn bench_scaling_validation(c: &mut Criterion) {
    for num_threads in [1, 2, 4, 8] {
        let mut group = c.benchmark_group(format!("provider_scaling_{}t", num_threads));
        group.warm_up_time(Duration::from_secs(3));
        group.measurement_time(Duration::from_secs(15));
        group.sample_size(100);

        let ops_per_thread = 1000;
        group.throughput(Throughput::Elements((num_threads * ops_per_thread) as u64));

        group.bench_function("routing_capsule_128", |b| {
            let capsule = Arc::new(RoutingCapsule128::new(0, 1));
            b.iter(|| {
                let handles: Vec<_> = (0..num_threads)
                    .map(|_| {
                        let c = Arc::clone(&capsule);
                        thread::spawn(move || {
                            for _ in 0..ops_per_thread {
                                let _ = c.select_provider();
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
        bench_select_provider_healthy,
        bench_select_provider_failover,
        bench_select_provider_all_unavailable,
        bench_update_state,
        bench_get_provider_id,
        bench_get_counters,
        bench_realistic_routing_pattern,
        bench_concurrent_selection_4t,
        bench_concurrent_update_4t,
        bench_concurrent_mixed_8t,
        bench_circuit_generation,
        bench_scaling_validation
}

criterion_main!(benches);
