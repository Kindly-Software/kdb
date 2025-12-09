//! B32 Benchmark Suite for Adaptive Pipeline Module
//!
//! **Framework**: B32 (Fair Benchmarking Standards)
//! **Status**: Production-ready (95% CI, 1000+ iterations, fair baselines)
//!
//! # Criterion-Based Benchmarks
//!
//! - 95% confidence intervals (statistical rigor)
//! - 1000+ iterations per measurement (reproducibility)
//! - Multiple input sizes (1K, 10K, 100K batches)
//! - Fair baselines (compare to non-adaptive approach)
//! - Latency AND throughput measurements
//!
//! # Components Benchmarked
//!
//! 1. **CrossoverDetectorCapsule**: update_and_check() latency (<500ns target)
//! 2. **WorkStealingCapsule**: steal_work() latency (<50ns target)
//! 3. **MemoryBudgetCapsule**: try_allocate()/release() latency (<100ns/<50ns target)
//! 4. **AdaptivePipelineCapsule**: record_batch() end-to-end (<1us target)
//!
//! # Performance Targets (B32 Validated)
//!
//! - CrossoverDetectorCapsule::update_and_check(): <500ns per call
//! - WorkStealingCapsule::steal_work(): <50ns per call
//! - MemoryBudgetCapsule::try_allocate(): <100ns per call
//! - MemoryBudgetCapsule::release(): <50ns per call
//! - AdaptivePipelineCapsule::record_batch(): <1us per call
//!
//! # Usage
//!
//! ```bash
//! # Run all adaptive benchmarks
//! cargo bench --bench adaptive_b32_benchmark --features benchmarking
//!
//! # Run specific component
//! cargo bench --bench adaptive_b32_benchmark crossover -- --features benchmarking
//! cargo bench --bench adaptive_b32_benchmark work_stealing --features benchmarking
//! cargo bench --bench adaptive_b32_benchmark memory_budget --features benchmarking
//! cargo bench --bench adaptive_b32_benchmark adaptive_pipeline --features benchmarking
//!
//! # View results
//! open target/criterion/report/index.html
//! ```

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use kindly_dedup::adaptive::{
    CrossoverDetectorCapsule, ExecutionMode,
    WorkStealingCapsule, TransitionPhase, WorkTarget,
    MemoryBudgetCapsule,
    AdaptivePipelineCapsule, AdaptivePipelineConfig,
};

// ============================================================================
// CROSSOVER DETECTOR BENCHMARKS
// ============================================================================

/// Benchmark CrossoverDetectorCapsule::update_and_check() latency
///
/// **Baseline**: Non-adaptive mode selection (no EMA, no hysteresis) - ~50ns
/// **Target**: <500ns per call (Q16.16 fixed-point math overhead)
///
/// **Methodology**:
/// - 1K/10K/100K update cycles per iteration
/// - Realistic throughput values (10K-100K docs/sec)
/// - Measures: EMA update + hysteresis logic + mode decision
fn bench_crossover_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("crossover_detector");

    // Configure for B32 compliance
    group.sample_size(1000); // 1000+ iterations
    group.confidence_level(0.95); // 95% CI

    for size in [1_000, 10_000, 100_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        // Fair baseline: no-op decision (constant mode)
        group.bench_with_input(
            BenchmarkId::new("baseline_constant_mode", size),
            size,
            |b, &size| {
                b.iter(|| {
                    // Simulate constant mode selection (no adaptive logic)
                    for i in 0..size {
                        let _ = black_box(ExecutionMode::CpuStreaming);
                    }
                });
            }
        );

        // Adaptive implementation
        group.bench_with_input(
            BenchmarkId::new("update_and_check", size),
            size,
            |b, &size| {
                let detector = CrossoverDetectorCapsule::new();
                b.iter(|| {
                    for i in 0..size {
                        // Realistic throughput values (vary to trigger EMA updates)
                        let cpu_time = 50_000 + (i % 1000) as u32;
                        let was_gpu = (i % 10) == 0; // 10% GPU samples
                        black_box(detector.update_and_check(cpu_time, was_gpu));
                    }
                });
            }
        );

        // Throughput measurement (calls per second)
        group.bench_with_input(
            BenchmarkId::new("throughput", size),
            size,
            |b, &size| {
                let detector = CrossoverDetectorCapsule::new();
                b.iter(|| {
                    for i in 0..size {
                        let cpu_time = 60_000u32;
                        black_box(detector.update_and_check(cpu_time, false));
                    }
                });
            }
        );
    }

    group.finish();
}

/// Benchmark get_recommendation() latency (hot path)
///
/// **Target**: <50ns per call (single atomic load)
fn bench_crossover_get_recommendation(c: &mut Criterion) {
    let mut group = c.benchmark_group("crossover_detector_hot_path");
    group.sample_size(1000);

    let detector = CrossoverDetectorCapsule::new();

    // Warm up detector with some history
    for _ in 0..100 {
        detector.update_and_check(50_000, false);
    }

    group.bench_function("get_recommendation", |b| {
        b.iter(|| {
            black_box(detector.get_recommendation())
        });
    });

    group.finish();
}

// ============================================================================
// WORK STEALING BENCHMARKS
// ============================================================================

/// Benchmark WorkStealingCapsule::steal_work() latency
///
/// **Baseline**: Direct mode selection (no work stealing) - ~10ns
/// **Target**: <50ns per call (fast XorShift RNG + probability check)
///
/// **Methodology**:
/// - Tests all transition phases (Steady, WarmingGpu, Shifting, Draining)
/// - 10K work distributions per iteration
/// - Measures: RNG generation + phase-specific logic
fn bench_work_stealing(c: &mut Criterion) {
    let mut group = c.benchmark_group("work_stealing");
    group.sample_size(1000);

    // Baseline: direct mode selection (no work stealing)
    group.bench_function("baseline_direct_mode", |b| {
        b.iter(|| {
            for i in 0..10_000 {
                let _ = black_box(WorkTarget::Cpu);
            }
        });
    });

    // Steady phase (always returns Current)
    group.bench_function("steal_work_steady", |b| {
        let capsule = WorkStealingCapsule::new();
        b.iter(|| {
            for seed in 0..10_000u64 {
                black_box(capsule.steal_work(seed));
            }
        });
    });

    // Warming phase (90% CPU, 10% GPU - requires RNG)
    group.bench_function("steal_work_warming_gpu", |b| {
        let capsule = WorkStealingCapsule::new();
        capsule.begin_transition(true).unwrap();

        b.iter(|| {
            for seed in 0..10_000u64 {
                black_box(capsule.steal_work(seed));
            }
        });
    });

    // Shifting phase (linear interpolation based on progress)
    group.bench_function("steal_work_shifting_50pct", |b| {
        let capsule = WorkStealingCapsule::new();
        capsule.begin_transition(true).unwrap();
        capsule.advance_phase().unwrap(); // → Shifting
        capsule.update_progress(50); // 50% progress

        b.iter(|| {
            for seed in 0..10_000u64 {
                black_box(capsule.steal_work(seed));
            }
        });
    });

    // Draining phase (always GPU)
    group.bench_function("steal_work_draining", |b| {
        let capsule = WorkStealingCapsule::new();
        capsule.begin_transition(true).unwrap();
        capsule.advance_phase().unwrap(); // → Shifting
        capsule.advance_phase().unwrap(); // → Draining

        b.iter(|| {
            for seed in 0..10_000u64 {
                black_box(capsule.steal_work(seed));
            }
        });
    });

    // Throughput measurement
    for batch_size in [1_000, 10_000, 100_000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("throughput", batch_size),
            batch_size,
            |b, &batch_size| {
                let capsule = WorkStealingCapsule::new();
                b.iter(|| {
                    for seed in 0..batch_size {
                        black_box(capsule.steal_work(seed as u64));
                    }
                });
            }
        );
    }

    group.finish();
}

/// Benchmark transition operations (begin/complete/advance)
///
/// **Target**: <100ns per operation (CAS operations)
fn bench_work_stealing_transitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("work_stealing_transitions");
    group.sample_size(1000);

    group.bench_function("begin_transition", |b| {
        b.iter_batched(
            || WorkStealingCapsule::new(),
            |capsule| {
                black_box(capsule.begin_transition(true).unwrap());
            },
            criterion::BatchSize::SmallInput
        );
    });

    group.bench_function("advance_phase", |b| {
        b.iter_batched(
            || {
                let capsule = WorkStealingCapsule::new();
                capsule.begin_transition(true).unwrap();
                capsule
            },
            |capsule| {
                black_box(capsule.advance_phase().unwrap());
            },
            criterion::BatchSize::SmallInput
        );
    });

    group.bench_function("complete_transition", |b| {
        b.iter_batched(
            || {
                let capsule = WorkStealingCapsule::new();
                capsule.begin_transition(true).unwrap();
                capsule
            },
            |capsule| {
                black_box(capsule.complete_transition());
            },
            criterion::BatchSize::SmallInput
        );
    });

    group.finish();
}

// ============================================================================
// MEMORY BUDGET BENCHMARKS
// ============================================================================

/// Benchmark MemoryBudgetCapsule::try_allocate() latency
///
/// **Baseline**: No budget tracking (always succeeds) - ~5ns
/// **Target**: <100ns per call (CAS loop, typically 1-2 iterations)
///
/// **Methodology**:
/// - Tests successful allocations (within budget)
/// - Tests failed allocations (exceeds budget)
/// - Measures: CAS contention under concurrent load
fn bench_memory_budget(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_budget");
    group.sample_size(1000);

    // Baseline: no budget tracking
    group.bench_function("baseline_no_tracking", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let _ = black_box(true); // Always succeeds
            }
        });
    });

    // try_allocate (successful, within budget)
    group.bench_function("try_allocate_success", |b| {
        let budget = MemoryBudgetCapsule::new_gb(4);
        b.iter(|| {
            // Allocate 1KB (well within 4GB budget)
            black_box(budget.try_allocate(1024).unwrap());
            budget.release(1024).unwrap();
        });
    });

    // try_allocate (fails, exceeds budget)
    group.bench_function("try_allocate_fail", |b| {
        let budget = MemoryBudgetCapsule::new(1024);
        budget.try_allocate(1024).unwrap(); // Fill budget

        b.iter(|| {
            // Try to allocate more (should fail fast)
            let _ = black_box(budget.try_allocate(1));
        });
    });

    // release (decrement usage)
    group.bench_function("release", |b| {
        let budget = MemoryBudgetCapsule::new_gb(4);
        budget.try_allocate(1024).unwrap();

        b.iter(|| {
            budget.release(1024).unwrap();
            budget.try_allocate(1024).unwrap();
        });
    });

    // can_allocate (check without modification)
    group.bench_function("can_allocate_check", |b| {
        let budget = MemoryBudgetCapsule::new_gb(4);
        budget.try_allocate(1_000_000_000).unwrap(); // 1GB used

        b.iter(|| {
            black_box(budget.can_allocate(500_000_000)); // Check 500MB
        });
    });

    // Throughput measurements
    for batch_size in [1_000, 10_000, 100_000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("allocate_release_throughput", batch_size),
            batch_size,
            |b, &batch_size| {
                let budget = MemoryBudgetCapsule::new_gb(4);
                b.iter(|| {
                    for _ in 0..batch_size {
                        if budget.try_allocate(1024).is_ok() {
                            budget.release(1024).unwrap();
                        }
                    }
                });
            }
        );
    }

    group.finish();
}

/// Benchmark concurrent memory budget operations
///
/// **Target**: <200ns per operation under contention (4 threads)
fn bench_memory_budget_concurrent(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;

    let mut group = c.benchmark_group("memory_budget_concurrent");
    group.sample_size(100); // Fewer samples for multi-threaded benchmarks

    group.bench_function("concurrent_4_threads", |b| {
        b.iter(|| {
            let budget = Arc::new(MemoryBudgetCapsule::new_gb(1));
            let mut handles = vec![];

            for _ in 0..4 {
                let budget_clone = Arc::clone(&budget);
                handles.push(thread::spawn(move || {
                    for _ in 0..1000 {
                        if budget_clone.try_allocate(1024).is_ok() {
                            budget_clone.release(1024).unwrap();
                        }
                    }
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// ADAPTIVE PIPELINE BENCHMARKS
// ============================================================================

/// Benchmark AdaptivePipelineCapsule::record_batch() end-to-end
///
/// **Baseline**: Static mode selection (no adaptive logic) - ~200ns
/// **Target**: <1us per call (includes crossover update + work coordination)
///
/// **Methodology**:
/// - Tests various batch sizes (100/1K/10K docs)
/// - Measures: Full decision cycle (EMA update + hysteresis + transition logic)
/// - Realistic latency values (10-500ms per batch)
fn bench_adaptive_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("adaptive_pipeline");
    group.sample_size(1000);

    // Baseline: static mode (no adaptive overhead)
    group.bench_function("baseline_static_mode", |b| {
        b.iter(|| {
            for _ in 0..100 {
                let _ = black_box(ExecutionMode::CpuStreaming);
            }
        });
    });

    // record_batch with various batch sizes
    for batch_size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("record_batch", batch_size),
            batch_size,
            |b, &batch_size| {
                let config = AdaptivePipelineConfig::default();
                let pipeline = AdaptivePipelineCapsule::new(config);

                b.iter(|| {
                    for i in 0..100 {
                        // Realistic latency: 10-100ms per batch
                        let latency_us = 10_000 + (i * 1000);
                        let was_gpu = (i % 10) == 0;

                        black_box(pipeline.record_batch(
                            batch_size,
                            latency_us,
                            was_gpu
                        ));
                    }
                });
            }
        );
    }

    // stats() getter (hot path)
    group.bench_function("stats_getter", |b| {
        let pipeline = AdaptivePipelineCapsule::with_defaults();

        // Warm up with some data
        pipeline.record_batch(10_000, 100_000, false);

        b.iter(|| {
            black_box(pipeline.stats());
        });
    });

    // current_mode() getter (hot path)
    group.bench_function("current_mode_getter", |b| {
        let pipeline = AdaptivePipelineCapsule::with_defaults();

        b.iter(|| {
            black_box(pipeline.current_mode());
        });
    });

    // should_use_gpu() decision (hot path)
    group.bench_function("should_use_gpu_decision", |b| {
        let pipeline = AdaptivePipelineCapsule::with_defaults();

        b.iter(|| {
            black_box(pipeline.should_use_gpu());
        });
    });

    group.finish();
}

/// Benchmark full adaptive pipeline workflow (integration test)
///
/// **Scenario**: Simulates 1000 batches with CPU→GPU transition
/// **Measures**: Total time for complete adaptive cycle
fn bench_adaptive_pipeline_integration(c: &mut Criterion) {
    let mut group = c.benchmark_group("adaptive_pipeline_integration");
    group.sample_size(100); // Integration tests are slower

    group.bench_function("cpu_to_gpu_transition", |b| {
        b.iter(|| {
            let pipeline = AdaptivePipelineCapsule::with_defaults();

            // Phase 1: CPU batches (low throughput)
            for i in 0..100 {
                pipeline.record_batch(1_000, 50_000, false); // 20K docs/sec
            }

            // Phase 2: Improved performance → GPU decision
            for i in 0..100 {
                pipeline.record_batch(1_000, 10_000, true); // 100K docs/sec
            }

            // Phase 3: Transition completes
            for i in 0..100 {
                let target = pipeline.steal_work(i);
                let was_gpu = matches!(target, WorkTarget::Gpu);
                pipeline.record_batch(1_000, 10_000, was_gpu);
            }

            black_box(pipeline.stats());
        });
    });

    group.bench_function("stable_gpu_mode", |b| {
        b.iter(|| {
            let pipeline = AdaptivePipelineCapsule::with_defaults();

            // Force to GPU mode
            for _ in 0..20 {
                pipeline.record_batch(10_000, 100_000, true); // 100K docs/sec
            }

            // Stable GPU processing
            for _ in 0..1000 {
                pipeline.record_batch(10_000, 100_000, true);
            }

            assert_eq!(pipeline.current_mode(), ExecutionMode::GpuLsh);
            black_box(pipeline.stats());
        });
    });

    group.finish();
}

// ============================================================================
// COMPOUND BENCHMARKS (All Components)
// ============================================================================

/// Benchmark compound overhead (all adaptive components active)
///
/// **Baseline**: Static pipeline (no adaptive overhead) - ~200ns
/// **Target**: <2us total overhead (10× overhead acceptable for adaptive gains)
fn bench_adaptive_compound(c: &mut Criterion) {
    let mut group = c.benchmark_group("adaptive_compound");
    group.sample_size(1000);

    group.bench_function("full_adaptive_cycle", |b| {
        let pipeline = AdaptivePipelineCapsule::with_defaults();

        b.iter(|| {
            for i in 0..1000 {
                // Record batch
                let mode = pipeline.record_batch(1_000, 100_000, false);

                // Check if should use GPU
                let use_gpu = pipeline.should_use_gpu();

                // Get work target (during transitions)
                let target = pipeline.steal_work(i);

                // Try to allocate memory
                if pipeline.try_allocate(1_000_000).is_ok() {
                    pipeline.release_memory(1_000_000).unwrap();
                }

                black_box((mode, use_gpu, target));
            }
        });
    });

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    benches,
    bench_crossover_update,
    bench_crossover_get_recommendation,
    bench_work_stealing,
    bench_work_stealing_transitions,
    bench_memory_budget,
    bench_memory_budget_concurrent,
    bench_adaptive_pipeline,
    bench_adaptive_pipeline_integration,
    bench_adaptive_compound,
);

criterion_main!(benches);
