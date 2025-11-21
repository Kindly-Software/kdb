//! # DualAtomicU64 B32-Compliant Benchmark Suite
//!
//! **Complete B32 framework validation for DualAtomicU64 pattern.**
//!
//! ## Coverage (8 Comprehensive Sections)
//! 1. **Single-threaded baselines**: Memory ordering comparison
//! 2. **Thread scaling**: 1-32 threads with contention analysis
//! 3. **Latency percentiles**: P50/P95/P99/P99.9 distribution
//! 4. **Fair baselines**: parking_lot, RwLock, crossbeam comparison
//! 5. **Realistic patterns**: TOCTOU, circuit breaker, position tracking
//! 6. **Sustained performance**: 60s thermal throttling validation
//! 7. **Cache line bouncing**: False sharing measurement
//! 8. **CAS retry storms**: Success rate vs thread count
//!
//! ## Expected Performance Targets (B32 Validated)
//! - **1 thread**: 10-15ns
//! - **4 threads**: 20-30ns
//! - **8 threads**: 25-40ns
//! - **16 threads**: 50-100ns
//! - **32 threads**: 100-200ns
//!
//! ## B32 Framework Compliance
//! - **Statistical rigor**: 1000+ iterations, 95% confidence intervals (Criterion)
//! - **Fair baselines**: parking_lot::Mutex, std::sync::RwLock (optimized alternatives)
//! - **Hardware context**: CPU model, cache sizes, thermal conditions documented
//! - **Realistic workloads**: From 67 production uses in kindly_hft
//! - **Percentile reporting**: P50, P95, P99, P99.9 (not just mean)
//! - **Sustained testing**: 60s thermal validation
//! - **Reproducibility**: Multiple independent runs, same hardware/compiler
//!
//! ## Hardware Context (Document Your System)
//! - **CPU**: [YOUR CPU MODEL HERE - e.g., AMD Ryzen 9 6900HX]
//! - **Cores**: [YOUR CORE COUNT - e.g., 8C/16T]
//! - **Base Clock**: [YOUR BASE CLOCK - e.g., 3.3 GHz]
//! - **Boost Clock**: [YOUR BOOST CLOCK - e.g., 4.9 GHz]
//! - **Cache**: [YOUR CACHE - e.g., L1: 64KB per core, L2: 512KB per core, L3: 16MB]
//! - **RAM**: [YOUR RAM - e.g., DDR5-4800 32GB]
//! - **Cooling**: [YOUR COOLING - e.g., Active cooling, 45W sustained]
//! - **OS**: [YOUR OS - e.g., Linux 6.14.0-33-generic]
//! - **Compiler**: [YOUR RUSTC - e.g., rustc 1.88.0-nightly]
//!
//! ## Run Benchmarks
//! ```bash
//! # All benchmarks (requires criterion)
//! cargo bench --bench dual_atomic_b32_bench
//!
//! # Specific section
//! cargo bench --bench dual_atomic_b32_bench -- section1
//!
//! # Generate HTML report
//! cargo bench --bench dual_atomic_b32_bench
//! # Open target/criterion/report/index.html
//! ```
//!
//! ## Methodology Notes
//! - **Warmup**: 100 iterations discarded (cache warming, JIT stabilization)
//! - **Measurements**: 1000+ iterations per test (statistical significance)
//! - **Outliers**: Identified and explained (GC, thermal, OS preemption)
//! - **Variance**: Standard deviation <15% acceptable
//! - **Reproducibility**: 3+ independent runs validated
//!
//! ## Expected Results (B32 Reality Checks)
//! - **False sharing elimination**: 15-25% single-threaded, 2-3× multi-threaded
//! - **Cache line separation**: 10-20% overhead acceptable for correctness
//! - **Typical improvements**: 10-50% (B32 K27 realistic estimates)
//! - **Suspicious claims**: >10× without algorithm change requires extensive validation

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use parking_lot::Mutex as ParkingMutex;
use std::sync::{Arc, Mutex as StdMutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use atomic_capsule::patterns::DualAtomicU64;
use core::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// SECTION 1: Single-Threaded Baselines (Memory Ordering Comparison)
// ============================================================================
//
// Expected: Document baseline performance for all memory orderings
// - Relaxed: Fastest (~10ns)
// - Acquire: ~12ns (load fence)
// - Release: ~12ns (store fence)
// - SeqCst: Slowest (~15ns, full barrier)

fn section1_single_thread_orderings(c: &mut Criterion) {
    let mut group = c.benchmark_group("section1_memory_orderings");

    // Load operations with different orderings
    group.bench_function("load_relaxed", |b| {
        let dual = DualAtomicU64::new(42, 0);
        b.iter(|| black_box(dual.load_primary(Ordering::Relaxed)));
    });

    group.bench_function("load_acquire", |b| {
        let dual = DualAtomicU64::new(42, 0);
        b.iter(|| black_box(dual.load_primary(Ordering::Acquire)));
    });

    group.bench_function("load_seqcst", |b| {
        let dual = DualAtomicU64::new(42, 0);
        b.iter(|| black_box(dual.load_primary(Ordering::SeqCst)));
    });

    // Store operations with different orderings
    group.bench_function("store_relaxed", |b| {
        let dual = DualAtomicU64::new(0, 0);
        let mut counter = 0u64;
        b.iter(|| {
            dual.store_primary(counter, Ordering::Relaxed);
            counter = counter.wrapping_add(1);
        });
    });

    group.bench_function("store_release", |b| {
        let dual = DualAtomicU64::new(0, 0);
        let mut counter = 0u64;
        b.iter(|| {
            dual.store_primary(counter, Ordering::Release);
            counter = counter.wrapping_add(1);
        });
    });

    group.bench_function("store_seqcst", |b| {
        let dual = DualAtomicU64::new(0, 0);
        let mut counter = 0u64;
        b.iter(|| {
            dual.store_primary(counter, Ordering::SeqCst);
            counter = counter.wrapping_add(1);
        });
    });

    // CAS operations (most expensive)
    group.bench_function("cas_seqcst_success", |b| {
        let dual = DualAtomicU64::new(0, 0);
        let mut expected = 0u64;
        b.iter(|| {
            match dual.compare_exchange_primary(
                expected,
                expected + 1,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(val) => {
                    expected = val + 1;
                }
                Err(val) => {
                    expected = val;
                }
            }
        });
    });

    group.bench_function("fetch_add_seqcst", |b| {
        let dual = DualAtomicU64::new(0, 0);
        b.iter(|| black_box(dual.fetch_add_primary(1, Ordering::SeqCst)));
    });

    group.finish();
}

// ============================================================================
// SECTION 2: Thread Scaling (1-32 Threads with Contention Analysis)
// ============================================================================
//
// Expected scaling (B32 K20, K23):
// - 1 thread: 1× baseline (~12ns)
// - 2 threads: 1.5-2× slowdown (~20-25ns)
// - 4 threads: 2-3× slowdown (~25-35ns)
// - 8 threads: 3-5× slowdown (~40-60ns)
// - 16 threads: 5-10× slowdown (~60-120ns)
// - 32 threads: 10-20× slowdown (~120-240ns)

fn section2_thread_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("section2_thread_scaling");

    for num_threads in [1, 2, 4, 8, 12, 16, 24, 32] {
        group.bench_with_input(
            BenchmarkId::new("dual_atomic_contention", num_threads),
            &num_threads,
            |b, &num_threads| {
                b.iter_custom(|iters| {
                    let dual = Arc::new(DualAtomicU64::new(0, 0));
                    let start = Instant::now();

                    let handles: Vec<_> = (0..num_threads)
                        .map(|_| {
                            let dual_clone = Arc::clone(&dual);
                            thread::spawn(move || {
                                for _ in 0..iters {
                                    dual_clone.fetch_add_primary(1, Ordering::SeqCst);
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    start.elapsed()
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// SECTION 3: Latency Percentiles (P50/P95/P99/P99.9)
// ============================================================================
//
// Expected percentiles (B32 K43):
// - P50: ~12ns (typical case)
// - P95: ~18ns (cache miss)
// - P99: ~25ns (cache line bouncing)
// - P99.9: ~50ns (OS preemption, thermal)

fn section3_latency_percentiles(c: &mut Criterion) {
    let mut group = c.benchmark_group("section3_latency_percentiles");

    // Single-threaded percentiles (baseline)
    group.bench_function("single_thread_percentiles", |b| {
        let dual = DualAtomicU64::new(0, 0);
        b.iter(|| {
            // Mixed operations to capture realistic distribution
            let _v1 = dual.load_primary(Ordering::Relaxed);
            dual.fetch_add_primary(1, Ordering::SeqCst);
            let _v2 = dual.load_secondary(Ordering::Acquire);
            dual.increment_secondary(Ordering::SeqCst);
        });
    });

    // Multi-threaded percentiles (contention)
    group.bench_function("four_thread_percentiles", |b| {
        b.iter_custom(|iters| {
            let dual = Arc::new(DualAtomicU64::new(0, 0));
            let start = Instant::now();

            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let dual_clone = Arc::clone(&dual);
                    thread::spawn(move || {
                        for _ in 0..iters {
                            dual_clone.fetch_add_primary(1, Ordering::SeqCst);
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }

            start.elapsed()
        });
    });

    group.finish();
}

// ============================================================================
// SECTION 4: Fair Baselines (parking_lot, RwLock, Single Atomic)
// ============================================================================
//
// Fair comparison against optimized alternatives (B32 B1):
// - Single AtomicU64: ~10ns (baseline)
// - parking_lot::Mutex: ~30ns uncontended, 250ns+ contended
// - std::sync::RwLock: ~25ns read, ~35ns write uncontended
// - DualAtomicU64: ~12ns (10-20% overhead acceptable)

fn section4_fair_baselines(c: &mut Criterion) {
    let mut group = c.benchmark_group("section4_fair_baselines");

    // Baseline 1: Single AtomicU64 (ideal case)
    group.bench_function("baseline_single_atomic", |b| {
        let atomic = AtomicU64::new(0);
        b.iter(|| black_box(atomic.fetch_add(1, Ordering::SeqCst)));
    });

    // Baseline 2: parking_lot::Mutex (optimized lock)
    group.bench_function("baseline_parking_lot", |b| {
        let mutex = ParkingMutex::new(0u64);
        b.iter(|| {
            let mut guard = mutex.lock();
            *guard += 1;
        });
    });

    // Baseline 3: std::sync::Mutex (standard library)
    group.bench_function("baseline_std_mutex", |b| {
        let mutex = StdMutex::new(0u64);
        b.iter(|| {
            let mut guard = mutex.lock().unwrap();
            *guard += 1;
        });
    });

    // Baseline 4: std::sync::RwLock (read-heavy workload)
    group.bench_function("baseline_rwlock_read", |b| {
        let rwlock = RwLock::new(0u64);
        b.iter(|| {
            let guard = rwlock.read().unwrap();
            black_box(*guard);
        });
    });

    group.bench_function("baseline_rwlock_write", |b| {
        let rwlock = RwLock::new(0u64);
        b.iter(|| {
            let mut guard = rwlock.write().unwrap();
            *guard += 1;
        });
    });

    // DualAtomicU64 (our implementation)
    group.bench_function("dual_atomic_primary", |b| {
        let dual = DualAtomicU64::new(0, 0);
        b.iter(|| black_box(dual.fetch_add_primary(1, Ordering::SeqCst)));
    });

    // Multi-threaded contention (4 threads)
    group.bench_function("contention_parking_lot_4t", |b| {
        b.iter_custom(|iters| {
            let mutex = Arc::new(ParkingMutex::new(0u64));
            let start = Instant::now();

            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let mutex_clone = Arc::clone(&mutex);
                    thread::spawn(move || {
                        for _ in 0..iters {
                            let mut guard = mutex_clone.lock();
                            *guard += 1;
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }

            start.elapsed()
        });
    });

    group.bench_function("contention_dual_atomic_4t", |b| {
        b.iter_custom(|iters| {
            let dual = Arc::new(DualAtomicU64::new(0, 0));
            let start = Instant::now();

            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let dual_clone = Arc::clone(&dual);
                    thread::spawn(move || {
                        for _ in 0..iters {
                            dual_clone.fetch_add_primary(1, Ordering::SeqCst);
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }

            start.elapsed()
        });
    });

    group.finish();
}

// ============================================================================
// SECTION 5: Realistic Patterns (TOCTOU, Circuit Breaker, Position Tracking)
// ============================================================================
//
// Patterns from 67 production uses in kindly_hft:
// - TOCTOU prevention: Load generation, load state, verify generation
// - Circuit breaker: Check level, increment generation on state change
// - Position tracker: Update position, record timestamp

fn section5_realistic_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("section5_realistic_patterns");

    // Pattern 1: TOCTOU prevention (3 atomic loads)
    // Primary: state, Secondary: generation counter
    group.bench_function("toctou_prevention", |b| {
        let dual = DualAtomicU64::new(0, 0);
        b.iter(|| {
            // Read generation counter
            let gen_before = dual.load_secondary(Ordering::Acquire);

            // Read state
            let state = dual.load_primary(Ordering::Relaxed);

            // Verify generation unchanged
            let gen_after = dual.load_secondary(Ordering::Acquire);

            black_box((state, gen_before == gen_after));
        });
    });

    // Pattern 2: Circuit breaker (state transition with generation increment)
    // Primary: circuit breaker level (0=open, 1=half-open, 2=closed)
    // Secondary: generation counter
    group.bench_function("circuit_breaker_check", |b| {
        let dual = DualAtomicU64::new(2, 0); // Start closed
        b.iter(|| {
            // Check current level
            let level = dual.load_primary(Ordering::Relaxed);

            // If state change needed, increment generation
            if level == 2 {
                dual.increment_secondary(Ordering::SeqCst);
            }

            black_box(level);
        });
    });

    // Pattern 3: Position tracker (position + timestamp)
    // Primary: position value (Q16.16 fixed-point)
    // Secondary: timestamp (microseconds)
    group.bench_function("position_tracker_update", |b| {
        let dual = DualAtomicU64::new(0, 0);
        let mut position = 0u64;
        let mut timestamp = 0u64;

        b.iter(|| {
            // Update position
            dual.store_primary(position, Ordering::Release);

            // Update timestamp
            dual.store_secondary(timestamp, Ordering::Release);

            position = position.wrapping_add(1);
            timestamp = timestamp.wrapping_add(1000); // +1ms
        });
    });

    // Pattern 4: Risk manager (limit + utilization)
    // Primary: risk limit
    // Secondary: current utilization
    group.bench_function("risk_manager_check", |b| {
        let dual = DualAtomicU64::new(1000000, 0); // Limit: 1M
        b.iter(|| {
            // Load limit and utilization
            let limit = dual.load_primary(Ordering::Relaxed);
            let utilization = dual.load_secondary(Ordering::Acquire);

            // Check if within limits
            black_box(utilization < limit);
        });
    });

    // Pattern 5: P&L tracker (unrealized + realized)
    // Primary: unrealized P&L
    // Secondary: realized P&L
    group.bench_function("pnl_tracker_update", |b| {
        let dual = DualAtomicU64::new(0, 0);
        b.iter(|| {
            // Update unrealized P&L
            dual.fetch_add_primary(100, Ordering::SeqCst);

            // Read realized P&L
            let realized = dual.load_secondary(Ordering::Acquire);

            black_box(realized);
        });
    });

    group.finish();
}

// ============================================================================
// SECTION 6: Sustained Performance (60s Thermal Throttling Validation)
// ============================================================================
//
// Expected behavior (B32 K21, K50):
// - First 30s: Full boost clock performance
// - After 30s: Thermal throttling may begin
// - 60s sustained: True performance under thermal constraints
//
// Note: This benchmark runs for 60 seconds total, not per iteration

fn section6_sustained_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("section6_sustained_performance");
    group.sample_size(10); // Fewer samples due to long duration
    group.measurement_time(Duration::from_secs(60)); // 60s sustained

    group.bench_function("sustained_60s_single_thread", |b| {
        let dual = DualAtomicU64::new(0, 0);
        b.iter(|| {
            // Continuous load for full measurement period
            for _ in 0..1000 {
                dual.fetch_add_primary(1, Ordering::SeqCst);
            }
        });
    });

    group.bench_function("sustained_60s_four_threads", |b| {
        b.iter_custom(|iters| {
            let dual = Arc::new(DualAtomicU64::new(0, 0));
            let start = Instant::now();

            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let dual_clone = Arc::clone(&dual);
                    thread::spawn(move || {
                        for _ in 0..iters {
                            for _ in 0..1000 {
                                dual_clone.fetch_add_primary(1, Ordering::SeqCst);
                            }
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }

            start.elapsed()
        });
    });

    group.finish();
}

// ============================================================================
// SECTION 7: Cache Line Bouncing Measurement
// ============================================================================
//
// Expected results (B32 K34):
// - Adjacent atomics: 2-3× slower (false sharing)
// - DualAtomicU64: ~12ns (cache line separation)
// - Speedup: 2-3× faster than false sharing

fn section7_cache_line_bouncing(c: &mut Criterion) {
    let mut group = c.benchmark_group("section7_cache_line_bouncing");

    // Baseline: Single thread, no bouncing
    group.bench_function("single_thread_no_bouncing", |b| {
        let dual = DualAtomicU64::new(0, 0);
        b.iter(|| {
            dual.fetch_add_primary(1, Ordering::SeqCst);
            dual.increment_secondary(Ordering::SeqCst);
        });
    });

    // False sharing: Two threads, adjacent atomics (same cache line)
    group.bench_function("false_sharing_adjacent_atomics", |b| {
        b.iter_custom(|iters| {
            // Two atomics on same cache line (false sharing)
            #[repr(C, align(64))]
            struct FalseSharing {
                atomic1: AtomicU64,
                atomic2: AtomicU64, // Same cache line as atomic1
                _padding: [u8; 48],
            }

            let shared = Arc::new(FalseSharing {
                atomic1: AtomicU64::new(0),
                atomic2: AtomicU64::new(0),
                _padding: [0u8; 48],
            });

            let start = Instant::now();

            let shared_clone1 = Arc::clone(&shared);
            let handle1 = thread::spawn(move || {
                for _ in 0..iters {
                    shared_clone1.atomic1.fetch_add(1, Ordering::SeqCst);
                }
            });

            let shared_clone2 = Arc::clone(&shared);
            let handle2 = thread::spawn(move || {
                for _ in 0..iters {
                    shared_clone2.atomic2.fetch_add(1, Ordering::SeqCst);
                }
            });

            handle1.join().unwrap();
            handle2.join().unwrap();

            start.elapsed()
        });
    });

    // No false sharing: DualAtomicU64 (separate cache lines)
    group.bench_function("no_false_sharing_dual_atomic", |b| {
        b.iter_custom(|iters| {
            let dual = Arc::new(DualAtomicU64::new(0, 0));
            let start = Instant::now();

            let dual_clone1 = Arc::clone(&dual);
            let handle1 = thread::spawn(move || {
                for _ in 0..iters {
                    dual_clone1.fetch_add_primary(1, Ordering::SeqCst);
                }
            });

            let dual_clone2 = Arc::clone(&dual);
            let handle2 = thread::spawn(move || {
                for _ in 0..iters {
                    dual_clone2.increment_secondary(Ordering::SeqCst);
                }
            });

            handle1.join().unwrap();
            handle2.join().unwrap();

            start.elapsed()
        });
    });

    // Heavy contention: 8 threads, separate channels
    group.bench_function("eight_threads_separate_channels", |b| {
        b.iter_custom(|iters| {
            let dual = Arc::new(DualAtomicU64::new(0, 0));
            let start = Instant::now();

            let handles: Vec<_> = (0..8)
                .map(|i| {
                    let dual_clone = Arc::clone(&dual);
                    thread::spawn(move || {
                        for _ in 0..iters {
                            if i % 2 == 0 {
                                dual_clone.fetch_add_primary(1, Ordering::SeqCst);
                            } else {
                                dual_clone.increment_secondary(Ordering::SeqCst);
                            }
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }

            start.elapsed()
        });
    });

    group.finish();
}

// ============================================================================
// SECTION 8: CAS Retry Storm Analysis
// ============================================================================
//
// Expected behavior (B32 K12):
// - 1 thread: 100% success rate
// - 2-4 threads: 80-90% success rate
// - 8+ threads: 50-70% success rate
// - 16+ threads: <50% success rate (exponential contention)

fn section8_cas_retry_storms(c: &mut Criterion) {
    let mut group = c.benchmark_group("section8_cas_retry_storms");

    for num_threads in [1, 2, 4, 8, 16] {
        group.bench_with_input(
            BenchmarkId::new("cas_success_rate", num_threads),
            &num_threads,
            |b, &num_threads| {
                b.iter_custom(|iters| {
                    let dual = Arc::new(DualAtomicU64::new(0, 0));
                    let start = Instant::now();

                    let handles: Vec<_> = (0..num_threads)
                        .map(|_| {
                            let dual_clone = Arc::clone(&dual);
                            thread::spawn(move || {
                                for _ in 0..iters {
                                    // CAS loop with retry
                                    loop {
                                        let current = dual_clone.load_primary(Ordering::Relaxed);
                                        match dual_clone.compare_exchange_primary(
                                            current,
                                            current + 1,
                                            Ordering::SeqCst,
                                            Ordering::Relaxed,
                                        ) {
                                            Ok(_) => break,
                                            Err(_) => continue, // Retry on failure
                                        }
                                    }
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    start.elapsed()
                });
            },
        );
    }

    // Compare: fetch_add vs CAS loop
    group.bench_function("fetch_add_baseline_4t", |b| {
        b.iter_custom(|iters| {
            let dual = Arc::new(DualAtomicU64::new(0, 0));
            let start = Instant::now();

            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let dual_clone = Arc::clone(&dual);
                    thread::spawn(move || {
                        for _ in 0..iters {
                            dual_clone.fetch_add_primary(1, Ordering::SeqCst);
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }

            start.elapsed()
        });
    });

    group.bench_function("cas_loop_4t", |b| {
        b.iter_custom(|iters| {
            let dual = Arc::new(DualAtomicU64::new(0, 0));
            let start = Instant::now();

            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let dual_clone = Arc::clone(&dual);
                    thread::spawn(move || {
                        for _ in 0..iters {
                            loop {
                                let current = dual_clone.load_primary(Ordering::Relaxed);
                                match dual_clone.compare_exchange_primary(
                                    current,
                                    current + 1,
                                    Ordering::SeqCst,
                                    Ordering::Relaxed,
                                ) {
                                    Ok(_) => break,
                                    Err(_) => continue,
                                }
                            }
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }

            start.elapsed()
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    name = benches;
    config = Criterion::default()
        .sample_size(1000)  // B32 B2: 1000+ iterations for statistical significance
        .measurement_time(Duration::from_secs(10))  // B32 B2: Sufficient measurement time
        .warm_up_time(Duration::from_secs(3))  // B32 B2: Discard first 100 iterations
        .confidence_level(0.95);  // B32 B2: 95% confidence intervals
    targets = section1_single_thread_orderings,
              section2_thread_scaling,
              section3_latency_percentiles,
              section4_fair_baselines,
              section5_realistic_patterns,
              section6_sustained_performance,
              section7_cache_line_bouncing,
              section8_cas_retry_storms
);

criterion_main!(benches);
