//! Atomic Capsule Performance Benchmarks
//!
//! Validates performance targets from B32 framework:
//! - Atomic operations: <15ns (hardware CAS latency)
//! - Coordination operations: <100ns
//! - Circuit breaker checks: <5ns

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use kiang::{GpuCircuitBreaker, GpuState, GpuStateCapsule, QualityLevel};

/// Benchmark: Single atomic read (target: <15ns)
fn bench_atomic_read(c: &mut Criterion) {
    let capsule = GpuStateCapsule::new();
    let state = GpuState {
        gpu_id: 0,
        frequency_mhz: 2100,
        power_mw: 45000,
        temp_celsius: 65,
        utilization: 50,
        valid: true,
    };
    capsule.publish(state);

    c.bench_function("atomic_read_gpu_state", |b| {
        b.iter(|| {
            let state = black_box(&capsule).read();
            black_box(state);
        });
    });
}

/// Benchmark: Two-phase commit publish (target: <100ns)
fn bench_two_phase_commit(c: &mut Criterion) {
    let capsule = GpuStateCapsule::new();

    c.bench_function("two_phase_commit_publish", |b| {
        b.iter(|| {
            let state = GpuState {
                gpu_id: 0,
                frequency_mhz: black_box(2100),
                power_mw: black_box(45000),
                temp_celsius: black_box(65),
                utilization: black_box(50),
                valid: true,
            };
            black_box(&capsule).publish(state);
        });
    });
}

/// Benchmark: Circuit breaker quality check (target: <5ns)
fn bench_circuit_breaker_check(c: &mut Criterion) {
    let breaker = GpuCircuitBreaker::new();

    c.bench_function("circuit_breaker_quality_check", |b| {
        b.iter(|| {
            let level = black_box(&breaker).level();
            black_box(level);
        });
    });
}

/// Benchmark: Circuit breaker should_allow check
fn bench_circuit_breaker_allow(c: &mut Criterion) {
    let breaker = GpuCircuitBreaker::new();

    c.bench_function("circuit_breaker_should_allow", |b| {
        b.iter(|| {
            let allow = black_box(&breaker).should_allow_command();
            black_box(allow);
        });
    });
}

/// Benchmark: Complete read → decision flow (target: <20ns)
fn bench_read_decision_flow(c: &mut Criterion) {
    let capsule = GpuStateCapsule::new();
    let breaker = GpuCircuitBreaker::new();

    let state = GpuState {
        gpu_id: 0,
        frequency_mhz: 2100,
        power_mw: 45000,
        temp_celsius: 65,
        utilization: 50,
        valid: true,
    };
    capsule.publish(state);

    c.bench_function("read_decision_flow", |b| {
        b.iter(|| {
            let state = black_box(&capsule).read();
            let breaker_ok = black_box(&breaker).should_allow_command();
            let ready = state.is_ready() && breaker_ok;
            black_box(ready);
        });
    });
}

/// Benchmark: Concurrent read scaling (1, 2, 4, 8 threads)
fn bench_concurrent_reads(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;

    let mut group = c.benchmark_group("concurrent_reads");

    for num_threads in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            num_threads,
            |b, &num_threads| {
                let capsule = Arc::new(GpuStateCapsule::new());
                let state = GpuState {
                    gpu_id: 0,
                    frequency_mhz: 2100,
                    power_mw: 45000,
                    temp_celsius: 65,
                    utilization: 50,
                    valid: true,
                };
                capsule.publish(state);

                b.iter(|| {
                    let mut handles = vec![];
                    for _ in 0..num_threads {
                        let capsule_clone = Arc::clone(&capsule);
                        handles.push(thread::spawn(move || {
                            for _ in 0..100 {
                                let state = capsule_clone.read();
                                black_box(state);
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

/// Benchmark: Circuit breaker auto-adjustment
fn bench_circuit_breaker_auto_adjust(c: &mut Criterion) {
    let breaker = GpuCircuitBreaker::new();

    c.bench_function("circuit_breaker_auto_adjust", |b| {
        b.iter(|| {
            black_box(&breaker).auto_adjust(
                black_box(70_000), // thermal_mc
                black_box(10),     // errors_per_sec
                black_box(60),     // memory_used_pct
                black_box(75),     // util
            );
        });
    });
}

/// Benchmark: Force quality level change
fn bench_force_quality_level(c: &mut Criterion) {
    let breaker = GpuCircuitBreaker::new();

    c.bench_function("force_quality_level", |b| {
        b.iter(|| {
            black_box(&breaker).force_level(QualityLevel::L1);
            black_box(&breaker).force_level(QualityLevel::L0);
        });
    });
}

/// Benchmark: Cache alignment impact
fn bench_cache_alignment_verification(c: &mut Criterion) {
    use std::mem::{align_of, size_of};

    c.bench_function("cache_alignment_info", |b| {
        b.iter(|| {
            let capsule_align = align_of::<GpuStateCapsule>();
            let capsule_size = size_of::<GpuStateCapsule>();
            let breaker_align = align_of::<GpuCircuitBreaker>();

            // Verify 64-byte alignment
            assert_eq!(capsule_align, 64);
            assert_eq!(breaker_align, 64);

            black_box((capsule_align, capsule_size, breaker_align));
        });
    });
}

criterion_group!(
    benches,
    bench_atomic_read,
    bench_two_phase_commit,
    bench_circuit_breaker_check,
    bench_circuit_breaker_allow,
    bench_read_decision_flow,
    bench_concurrent_reads,
    bench_circuit_breaker_auto_adjust,
    bench_force_quality_level,
    bench_cache_alignment_verification,
);

criterion_main!(benches);
