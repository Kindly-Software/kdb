//! B32 Framework Benchmarks for DashboardStateCapsule (T1 Atomic)
//!
//! # B32 Compliance
//!
//! - **Fair Baseline**: Compare vs raw AtomicI64 load/store (hardware limit)
//! - **Statistical Rigor**: 1000+ iterations, 95% CI via Criterion
//! - **Honest Reporting**: Document overhead vs hardware baseline
//! - **Reality Check**: <5ns target (hardware CAS latency: 3-10ns)
//!
//! # Benchmarks
//!
//! 1. **Baseline**: Raw AtomicI64 operations (hardware limit)
//! 2. **Single-threaded**: Capsule load/store overhead
//! 3. **Multi-field**: Full snapshot latency (7 atomic loads)
//! 4. **Concurrent**: Scalability (1, 2, 4, 8 threads)
//! 5. **Memory Ordering**: Acquire/Release vs Relaxed
//!
//! # Performance Targets
//!
//! - **Single field**: <5ns (1-2ns capsule overhead over AtomicI64)
//! - **Multi-field**: <50ns (7 fields × 5-8ns)
//! - **Concurrent (8 threads)**: 45M ops/s (56% scaling efficiency)
//!
//! # Build Instructions
//!
//! ```bash
//! cargo bench --bench dashboard_state_bench
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

// Import DashboardStateCapsule from WASM module
// Note: This requires the wasm module to be accessible for benchmarking
// If not available, we'll use a local stub for benchmark purposes
#[path = "../src/wasm/src/capsules/dashboard_state.rs"]
mod dashboard_state;
use dashboard_state::DashboardStateCapsule;

// ============================================================================
// BENCHMARK 1: Hardware Baseline (Raw AtomicI64)
// ============================================================================

fn bench_atomic_i64_load(c: &mut Criterion) {
    let value = AtomicI64::new(50000);

    c.bench_function("baseline/atomic_i64_load", |b| {
        b.iter(|| {
            black_box(value.load(Ordering::Acquire));
        });
    });
}

fn bench_atomic_i64_store(c: &mut Criterion) {
    let value = AtomicI64::new(50000);

    c.bench_function("baseline/atomic_i64_store", |b| {
        b.iter(|| {
            value.store(black_box(50000), Ordering::Release);
        });
    });
}

fn bench_atomic_i64_load_relaxed(c: &mut Criterion) {
    let value = AtomicI64::new(50000);

    c.bench_function("baseline/atomic_i64_load_relaxed", |b| {
        b.iter(|| {
            black_box(value.load(Ordering::Relaxed));
        });
    });
}

// ============================================================================
// BENCHMARK 2: Capsule Single-Field Operations
// ============================================================================

fn bench_capsule_load_budget(c: &mut Criterion) {
    let capsule = DashboardStateCapsule::new();
    capsule.set_budget(50000);

    c.bench_function("capsule/load_budget", |b| {
        b.iter(|| {
            black_box(capsule.load_budget());
        });
    });
}

fn bench_capsule_set_budget(c: &mut Criterion) {
    let capsule = DashboardStateCapsule::new();

    c.bench_function("capsule/set_budget", |b| {
        b.iter(|| {
            capsule.set_budget(black_box(50000));
        });
    });
}

fn bench_capsule_load_status(c: &mut Criterion) {
    let capsule = DashboardStateCapsule::new();
    capsule.set_status(0b1010);

    c.bench_function("capsule/load_status", |b| {
        b.iter(|| {
            black_box(capsule.load_status());
        });
    });
}

fn bench_capsule_load_circuit(c: &mut Criterion) {
    let capsule = DashboardStateCapsule::new();
    capsule.set_circuit(1); // Half-open

    c.bench_function("capsule/load_circuit", |b| {
        b.iter(|| {
            black_box(capsule.load_circuit());
        });
    });
}

// ============================================================================
// BENCHMARK 3: Multi-Field Snapshot
// ============================================================================

fn bench_capsule_load_all_fields(c: &mut Criterion) {
    let capsule = DashboardStateCapsule::new();
    capsule.set_budget(50000);
    capsule.set_status(0b1010);
    capsule.set_circuit(1);
    capsule.set_timestamp(1_000_000_000_000_000_000);
    capsule.set_poll_interval(5000);
    capsule.set_provider_count(3);
    capsule.set_failure_rate_bp(500);

    c.bench_function("capsule/load_all_fields", |b| {
        b.iter(|| {
            black_box((
                capsule.load_budget(),
                capsule.load_status(),
                capsule.load_circuit(),
                capsule.load_timestamp(),
                capsule.poll_interval(),
                capsule.provider_count(),
                capsule.failure_rate_bp(),
            ));
        });
    });
}

fn bench_capsule_snapshot_with_branching(c: &mut Criterion) {
    let capsule = DashboardStateCapsule::new();
    capsule.set_budget(50000);
    capsule.set_status(0b1010);
    capsule.set_circuit(2); // Open
    capsule.set_timestamp(1_000_000_000_000_000_000);

    c.bench_function("capsule/snapshot_with_branching", |b| {
        b.iter(|| {
            let budget = capsule.load_budget();
            let status = capsule.load_status();
            let circuit = capsule.load_circuit();
            let is_open = capsule.is_circuit_open();
            let is_half_open = capsule.is_circuit_half_open();
            let timestamp = capsule.load_timestamp();

            black_box((budget, status, circuit, is_open, is_half_open, timestamp));
        });
    });
}

// ============================================================================
// BENCHMARK 4: Concurrent Budget Updates
// ============================================================================

fn bench_concurrent_budget_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent/budget_updates");

    for num_threads in [1, 2, 4, 8] {
        group.throughput(Throughput::Elements(10_000 * num_threads as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_threads", num_threads)),
            &num_threads,
            |b, &threads| {
                let capsule = Arc::new(DashboardStateCapsule::new());
                capsule.set_budget(100_000);

                b.iter(|| {
                    let mut handles = vec![];
                    for thread_id in 0..threads {
                        let c = Arc::clone(&capsule);
                        handles.push(std::thread::spawn(move || {
                            for i in 0..10_000 {
                                let new_budget = (thread_id as i64 * 10000) + i;
                                c.set_budget(new_budget);
                                black_box(c.load_budget());
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
// BENCHMARK 5: Read-Heavy Concurrent Workload
// ============================================================================

fn bench_read_heavy_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent/read_heavy_90_10");

    for num_threads in [1, 2, 4, 8] {
        group.throughput(Throughput::Elements(10_000 * num_threads as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_threads", num_threads)),
            &num_threads,
            |b, &threads| {
                let capsule = Arc::new(DashboardStateCapsule::new());
                capsule.set_budget(100_000);

                b.iter(|| {
                    let mut handles = vec![];
                    for _ in 0..threads {
                        let c = Arc::clone(&capsule);
                        handles.push(std::thread::spawn(move || {
                            for i in 0..10_000 {
                                if i % 10 == 0 {
                                    // 10% writes
                                    c.set_budget(50000 + i);
                                } else {
                                    // 90% reads
                                    black_box(c.load_budget());
                                }
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
// BENCHMARK 6: Write-Heavy Concurrent Workload
// ============================================================================

fn bench_write_heavy_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent/write_heavy_10_90");

    for num_threads in [1, 2, 4, 8] {
        group.throughput(Throughput::Elements(10_000 * num_threads as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_threads", num_threads)),
            &num_threads,
            |b, &threads| {
                let capsule = Arc::new(DashboardStateCapsule::new());
                capsule.set_budget(100_000);

                b.iter(|| {
                    let mut handles = vec![];
                    for _ in 0..threads {
                        let c = Arc::clone(&capsule);
                        handles.push(std::thread::spawn(move || {
                            for i in 0..10_000 {
                                if i % 10 == 0 {
                                    // 10% reads
                                    black_box(c.load_budget());
                                } else {
                                    // 90% writes
                                    c.set_budget(50000 + i);
                                }
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
// BENCHMARK 7: Memory Ordering Comparison
// ============================================================================

fn bench_ordering_acquire(c: &mut Criterion) {
    let value = AtomicI64::new(50000);

    c.bench_function("ordering/acquire", |b| {
        b.iter(|| {
            black_box(value.load(Ordering::Acquire));
        });
    });
}

fn bench_ordering_release(c: &mut Criterion) {
    let value = AtomicI64::new(50000);

    c.bench_function("ordering/release", |b| {
        b.iter(|| {
            value.store(black_box(50000), Ordering::Release);
        });
    });
}

fn bench_ordering_relaxed(c: &mut Criterion) {
    let value = AtomicI64::new(50000);

    c.bench_function("ordering/relaxed", |b| {
        b.iter(|| {
            black_box(value.load(Ordering::Relaxed));
        });
    });
}

fn bench_ordering_seqcst(c: &mut Criterion) {
    let value = AtomicI64::new(50000);

    c.bench_function("ordering/seqcst", |b| {
        b.iter(|| {
            black_box(value.load(Ordering::SeqCst));
        });
    });
}

// ============================================================================
// BENCHMARK 8: TOCTOU Prevention (Generation Counter)
// ============================================================================

fn bench_generation_counter_increment(c: &mut Criterion) {
    use std::sync::atomic::AtomicU64;
    let generation = AtomicU64::new(0);

    c.bench_function("toctou/generation_increment", |b| {
        b.iter(|| {
            black_box(generation.fetch_add(1, Ordering::Relaxed));
        });
    });
}

fn bench_toctou_detection(c: &mut Criterion) {
    let capsule = DashboardStateCapsule::new();
    capsule.set_budget(50000);

    c.bench_function("toctou/detect_concurrent_modification", |b| {
        b.iter(|| {
            // Simulate TOCTOU detection pattern
            let gen_before = capsule.load_budget(); // Proxy for generation load
            black_box(capsule.load_budget());
            let gen_after = capsule.load_budget();

            // Compare generations (simulated)
            black_box(gen_before == gen_after);
        });
    });
}

// ============================================================================
// BENCHMARK 9: Multi-Field Update Atomicity
// ============================================================================

fn bench_multi_field_update(c: &mut Criterion) {
    let capsule = DashboardStateCapsule::new();

    c.bench_function("multi_field/sequential_update", |b| {
        b.iter(|| {
            capsule.set_budget(black_box(50000));
            capsule.set_status(black_box(0b1010));
            capsule.set_circuit(black_box(1));
            capsule.set_timestamp(black_box(1_000_000_000_000_000_000));
            capsule.set_poll_interval(black_box(5000));
            capsule.set_provider_count(black_box(3));
            capsule.set_failure_rate_bp(black_box(500));
        });
    });
}

// ============================================================================
// BENCHMARK 10: Circuit State Transitions
// ============================================================================

fn bench_circuit_state_transitions(c: &mut Criterion) {
    let capsule = DashboardStateCapsule::new();

    c.bench_function("circuit/state_transitions", |b| {
        b.iter(|| {
            capsule.set_circuit(0); // Closed
            black_box(capsule.is_circuit_open());

            capsule.set_circuit(1); // Half-Open
            black_box(capsule.is_circuit_half_open());

            capsule.set_circuit(2); // Open
            black_box(capsule.is_circuit_open());
        });
    });
}

// ============================================================================
// BENCHMARK 11: Overhead Analysis
// ============================================================================

fn bench_overhead_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("overhead/capsule_vs_raw");

    // Raw AtomicI64
    let raw = AtomicI64::new(50000);
    group.bench_function("raw_atomic_i64", |b| {
        b.iter(|| {
            raw.store(black_box(50000), Ordering::Release);
            black_box(raw.load(Ordering::Acquire));
        });
    });

    // Capsule wrapper
    let capsule = DashboardStateCapsule::new();
    group.bench_function("capsule_wrapper", |b| {
        b.iter(|| {
            capsule.set_budget(black_box(50000));
            black_box(capsule.load_budget());
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    dashboard_state_benches,
    // Baseline (hardware limits)
    bench_atomic_i64_load,
    bench_atomic_i64_store,
    bench_atomic_i64_load_relaxed,
    // Single-field operations
    bench_capsule_load_budget,
    bench_capsule_set_budget,
    bench_capsule_load_status,
    bench_capsule_load_circuit,
    // Multi-field operations
    bench_capsule_load_all_fields,
    bench_capsule_snapshot_with_branching,
    bench_multi_field_update,
    // Concurrent scalability
    bench_concurrent_budget_updates,
    bench_read_heavy_concurrent,
    bench_write_heavy_concurrent,
    // Memory ordering
    bench_ordering_acquire,
    bench_ordering_release,
    bench_ordering_relaxed,
    bench_ordering_seqcst,
    // TOCTOU prevention
    bench_generation_counter_increment,
    bench_toctou_detection,
    // Circuit breaker
    bench_circuit_state_transitions,
    // Overhead analysis
    bench_overhead_analysis,
);

criterion_main!(dashboard_state_benches);
