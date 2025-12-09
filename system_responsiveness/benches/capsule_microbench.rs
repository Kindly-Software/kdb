/// Microbenchmarks for atomic capsule operations (B32 Framework)
/// Target: <50ns hung detection, <100ns state update, <20ns circuit breaker
/// Statistical rigor: 1000+ iterations, 95% CI, P50/P95/P99 percentiles

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;
use std::time::Duration;
use sysrespond::capsules::{ProcessStateCapsule, ResourceGovernorCapsule};

/// B1-B5: Measurement methodology with statistical rigor
fn benchmark_process_state_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("process_state_capsule");

    // B2: Statistical rigor (1000+ iterations, 95% CI)
    group.confidence_level(0.95)
         .sample_size(1000)
         .warm_up_time(Duration::from_secs(3))
         .measurement_time(Duration::from_secs(10));

    // Benchmark: is_hung() - Target <50ns
    // K2: AtomicU64 load should be 5-10ns, single-read decision <50ns
    group.bench_function("is_hung_not_hung", |b| {
        let capsule = ProcessStateCapsule::new(1234);
        capsule.update(1234, 50.0, 100, false, false, false);

        b.iter(|| {
            black_box(capsule.is_hung(black_box(100.0), black_box(300)))
        });
    });

    group.bench_function("is_hung_detected", |b| {
        let capsule = ProcessStateCapsule::new(5678);
        capsule.update(5678, 200.0, 500, false, false, false);

        b.iter(|| {
            black_box(capsule.is_hung(black_box(100.0), black_box(300)))
        });
    });

    // Benchmark: update() - Target <100ns
    // K2: AtomicU64 store should be 5-10ns, packing + store <100ns
    group.bench_function("update_state", |b| {
        let capsule = ProcessStateCapsule::new(9999);

        b.iter(|| {
            capsule.update(
                black_box(9999),
                black_box(125.5),
                black_box(200),
                black_box(false),
                black_box(false),
                black_box(false),
            )
        });
    });

    // Benchmark: pid() extraction - Target <10ns
    group.bench_function("pid_extraction", |b| {
        let capsule = ProcessStateCapsule::new(1234);

        b.iter(|| {
            black_box(capsule.pid())
        });
    });

    // Benchmark: generation() counter - Target <10ns
    group.bench_function("generation_counter", |b| {
        let capsule = ProcessStateCapsule::new(5678);

        b.iter(|| {
            black_box(capsule.generation())
        });
    });

    // Benchmark: set_whitelisted() - Target <50ns (CAS loop)
    // K2: AtomicU64 CAS is 10-15ns, loop convergence <50ns
    group.bench_function("set_whitelisted", |b| {
        let capsule = ProcessStateCapsule::new(9999);

        b.iter(|| {
            capsule.set_whitelisted(black_box(true))
        });
    });

    group.finish();
}

/// B4: Contention scenarios (uncontended baseline)
fn benchmark_resource_governor_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("resource_governor_capsule");

    group.confidence_level(0.95)
         .sample_size(1000)
         .warm_up_time(Duration::from_secs(3));

    // Benchmark: can_kill() - Target <20ns
    // K2: Single AtomicU64 load + bit extraction <20ns
    group.bench_function("can_kill_closed", |b| {
        let governor = ResourceGovernorCapsule::new(100.0, 4096, 10, 60);

        b.iter(|| {
            black_box(governor.can_kill())
        });
    });

    group.bench_function("can_kill_half_open", |b| {
        let governor = ResourceGovernorCapsule::new(100.0, 4096, 2, 60);

        // Trip circuit (3 kills with threshold=2)
        governor.record_kill();
        governor.record_kill();
        governor.record_kill();

        // Reset to half-open
        governor.reset_active_kills();

        b.iter(|| {
            black_box(governor.can_kill())
        });
    });

    // Benchmark: record_kill() - Target <50ns
    // K2: CAS loop + conditional circuit trip <50ns
    group.bench_function("record_kill_no_trip", |b| {
        let governor = ResourceGovernorCapsule::new(100.0, 4096, 100, 60);

        b.iter(|| {
            governor.record_kill()
        });
    });

    group.bench_function("record_kill_with_trip", |b| {
        // Reset before each iteration
        b.iter_batched(
            || {
                let gov = ResourceGovernorCapsule::new(100.0, 4096, 5, 60);
                // Record 5 kills (threshold=5)
                for _ in 0..5 {
                    gov.record_kill();
                }
                gov
            },
            |gov| {
                // 6th kill trips circuit
                black_box(gov.record_kill())
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Benchmark: reset_active_kills() - Target <100ns
    group.bench_function("reset_active_kills", |b| {
        let governor = ResourceGovernorCapsule::new(100.0, 4096, 10, 60);

        // Add some kills to reset
        governor.record_kill();
        governor.record_kill();
        governor.record_kill();

        b.iter(|| {
            governor.reset_active_kills()
        });
    });

    // Benchmark: circuit state reads
    group.bench_function("circuit_state", |b| {
        let governor = ResourceGovernorCapsule::new(100.0, 4096, 10, 60);

        b.iter(|| {
            black_box(governor.circuit_state())
        });
    });

    group.finish();
}

/// B17: Throughput vs latency tradeoffs
fn benchmark_throughput_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_scaling");

    group.throughput(Throughput::Elements(1));

    // Measure operations per second for hung detection
    group.bench_function("hung_detection_throughput", |b| {
        let capsule = ProcessStateCapsule::new(1234);
        capsule.update(1234, 200.0, 500, false, false, false);

        b.iter(|| {
            for _ in 0..1000 {
                black_box(capsule.is_hung(black_box(100.0), black_box(300)));
            }
        });
    });

    // Measure kill decision throughput
    group.bench_function("kill_decision_throughput", |b| {
        let governor = ResourceGovernorCapsule::new(100.0, 4096, 100, 60);

        b.iter(|| {
            for _ in 0..1000 {
                black_box(governor.can_kill());
            }
        });
    });

    group.finish();
}

/// B3: Realistic workloads - Pattern: scan → detect → decide
fn benchmark_realistic_detection_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_detection_cycle");

    group.confidence_level(0.95)
         .sample_size(100);

    // Simulate realistic hung detection cycle (as daemon would do)
    group.bench_function("full_detection_cycle", |b| {
        let capsule = ProcessStateCapsule::new(1234);
        let governor = ResourceGovernorCapsule::new(100.0, 4096, 10, 60);

        b.iter(|| {
            // 1. Update process state (from sysinfo)
            capsule.update(
                black_box(1234),
                black_box(150.5),
                black_box(400),
                black_box(false),
                black_box(false),
                black_box(false),
            );

            // 2. Check if hung
            let is_hung = capsule.is_hung(black_box(100.0), black_box(300));

            // 3. If hung, check circuit breaker
            if is_hung {
                let can_kill = governor.can_kill();

                // 4. Record kill decision
                if can_kill {
                    governor.record_kill();
                }
            }
        });
    });

    group.finish();
}

/// B24: Platform diversity - Report CPU-specific metrics
fn benchmark_cache_effects(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_effects");

    // K6: L1 cache hit should be ~1ns, L2 ~3ns, L3 ~12ns
    // Test cache-aligned capsule access patterns

    group.bench_function("sequential_capsule_access", |b| {
        // Create 1000 capsules (sequential memory layout)
        let capsules: Vec<_> = (0..1000)
            .map(|i| ProcessStateCapsule::new(i as u32))
            .collect();

        b.iter(|| {
            for capsule in &capsules {
                black_box(capsule.is_hung(black_box(100.0), black_box(300)));
            }
        });
    });

    group.bench_function("random_capsule_access", |b| {
        use std::collections::HashMap;

        // Create hash map (random memory access pattern)
        let mut map = HashMap::new();
        for i in 0..1000 {
            map.insert(i, Arc::new(ProcessStateCapsule::new(i as u32)));
        }

        b.iter(|| {
            for i in 0..1000 {
                if let Some(capsule) = map.get(&i) {
                    black_box(capsule.is_hung(black_box(100.0), black_box(300)));
                }
            }
        });
    });

    group.finish();
}

/// B16: Latency distribution analysis - Percentiles matter
fn benchmark_latency_percentiles(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_percentiles");

    // Large sample size for percentile analysis
    group.sample_size(10000)
         .confidence_level(0.95);

    // Critical path: hung detection under various loads
    group.bench_function("hung_detection_p50_p99", |b| {
        let capsule = ProcessStateCapsule::new(1234);
        capsule.update(1234, 200.0, 500, false, false, false);

        b.iter(|| {
            black_box(capsule.is_hung(black_box(100.0), black_box(300)))
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_process_state_operations,
    benchmark_resource_governor_operations,
    benchmark_throughput_scaling,
    benchmark_realistic_detection_cycle,
    benchmark_cache_effects,
    benchmark_latency_percentiles,
);
criterion_main!(benches);
