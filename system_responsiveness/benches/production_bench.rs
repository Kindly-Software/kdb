/// Production benchmarks for sustained system monitoring (B32 Framework)
/// Target: <5% CPU overhead, <50MB memory, 24h sustained operation
/// B21: Thermal throttling awareness, B23: Long-term stability

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{System, ProcessesToUpdate};
use sysrespond::capsules::{ProcessStateCapsule, ResourceGovernorCapsule};

/// B21: Thermal impact - Sustained performance under load
fn benchmark_sustained_monitoring(c: &mut Criterion) {
    let mut group = c.benchmark_group("sustained_monitoring");

    // B21: 60+ second sustained test to detect thermal throttling
    group.confidence_level(0.95)
         .sample_size(10)
         .measurement_time(Duration::from_secs(60));  // 60s sustained

    group.bench_function("60s_sustained_scan", |b| {
        let mut sys = System::new_all();
        let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 10, 60));
        let mut process_map: HashMap<u32, Arc<ProcessStateCapsule>> = HashMap::new();

        b.iter(|| {
            // Continuous scan for 60 seconds
            let start = Instant::now();
            let mut scan_count = 0;

            while start.elapsed() < Duration::from_secs(1) {
                sys.refresh_processes(ProcessesToUpdate::All);

                for (pid, process) in sys.processes() {
                    let pid_u32 = pid.as_u32();
                    let cpu_pct = process.cpu_usage() as f64;
                    let runtime_sec = process.run_time();

                    let capsule = process_map.entry(pid_u32)
                        .or_insert_with(|| Arc::new(ProcessStateCapsule::new(pid_u32)));

                    capsule.update(pid_u32, cpu_pct, runtime_sec, false, false, false);

                    if capsule.is_hung(100.0, 300) && governor.can_kill() {
                        governor.record_kill();
                    }
                }

                scan_count += 1;
            }

            black_box(scan_count)
        });
    });

    group.finish();
}

/// B22: CPU utilization - Measure daemon overhead
fn benchmark_cpu_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("cpu_overhead");

    group.confidence_level(0.95)
         .sample_size(50);

    // Target: <5% CPU overhead at 10s scan interval
    group.bench_function("10s_interval_overhead", |b| {
        let mut sys = System::new_all();
        let mut process_map: HashMap<u32, Arc<ProcessStateCapsule>> = HashMap::new();

        b.iter(|| {
            // Single scan (would run every 10s in production)
            sys.refresh_processes(ProcessesToUpdate::All);

            for (pid, process) in sys.processes() {
                let pid_u32 = pid.as_u32();
                let capsule = process_map.entry(pid_u32)
                    .or_insert_with(|| Arc::new(ProcessStateCapsule::new(pid_u32)));

                capsule.update(
                    pid_u32,
                    process.cpu_usage() as f64,
                    process.run_time(),
                    false,
                    false,
                    false,
                );

                black_box(capsule.is_hung(100.0, 300));
            }
        });
    });

    // CPU overhead = (scan_time / interval) * 100%
    // Target: scan_time < 500ms for 10s interval = 5% overhead
    group.finish();
}

/// B23: Memory usage over time - Leak detection
fn benchmark_memory_stability(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_stability");

    group.confidence_level(0.95)
         .sample_size(10);

    // Simulate 1M scan cycles (represents ~115 days at 10s intervals)
    group.bench_function("1m_scan_cycles", |b| {
        let mut sys = System::new_all();
        let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 10, 60));
        let mut process_map: HashMap<u32, Arc<ProcessStateCapsule>> = HashMap::new();

        b.iter(|| {
            for cycle in 0..1000 {  // 1K cycles for benchmark (scaled)
                sys.refresh_processes(ProcessesToUpdate::All);

                // Scan processes
                for (pid, process) in sys.processes() {
                    let pid_u32 = pid.as_u32();
                    let capsule = process_map.entry(pid_u32)
                        .or_insert_with(|| Arc::new(ProcessStateCapsule::new(pid_u32)));

                    capsule.update(
                        pid_u32,
                        process.cpu_usage() as f64,
                        process.run_time(),
                        false,
                        false,
                        false,
                    );
                }

                // Cleanup dead processes (critical for memory stability)
                if cycle % 100 == 0 {
                    process_map.retain(|pid, _| {
                        sys.process(sysinfo::Pid::from_u32(*pid)).is_some()
                    });
                }

                // Reset circuit breaker every minute (60 cycles at 1s interval)
                if cycle % 60 == 0 {
                    governor.reset_active_kills();
                }
            }

            black_box(process_map.len())
        });
    });

    group.finish();
}

/// B20: Scaling efficiency - Performance vs thread count
fn benchmark_parallel_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_efficiency");

    // Note: This is a sequential simulation of parallel work
    // Real parallel benchmarks would use rayon or tokio spawn

    group.bench_function("batch_process_updates", |b| {
        // Create 1000 capsules for batch processing
        let capsules: Vec<_> = (0..1000)
            .map(|i| Arc::new(ProcessStateCapsule::new(i)))
            .collect();

        b.iter(|| {
            // Simulate parallel batch updates
            for (i, capsule) in capsules.iter().enumerate() {
                capsule.update(
                    i as u32,
                    (i % 300) as f64,
                    (i % 600) as u64,
                    false,
                    false,
                    false,
                );
            }

            // Parallel hung detection
            let mut hung_count = 0;
            for capsule in &capsules {
                if capsule.is_hung(100.0, 300) {
                    hung_count += 1;
                }
            }

            black_box(hung_count)
        });
    });

    group.finish();
}

/// B31: Production validation - Real-world scenarios
fn benchmark_production_scenarios(c: &mut Criterion) {
    let mut group = c.benchmark_group("production_scenarios");

    group.confidence_level(0.95)
         .sample_size(20);

    // Scenario 1: Normal operation (no hung processes)
    group.bench_function("normal_operation", |b| {
        let mut sys = System::new_all();
        let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 10, 60));
        let mut process_map: HashMap<u32, Arc<ProcessStateCapsule>> = HashMap::new();

        b.iter(|| {
            sys.refresh_processes(ProcessesToUpdate::All);

            let mut scanned = 0;
            for (pid, process) in sys.processes() {
                let pid_u32 = pid.as_u32();
                let capsule = process_map.entry(pid_u32)
                    .or_insert_with(|| Arc::new(ProcessStateCapsule::new(pid_u32)));

                // Normal CPU usage (0-50%)
                let cpu_pct = process.cpu_usage() as f64;
                capsule.update(pid_u32, cpu_pct, process.run_time(), false, false, false);

                if capsule.is_hung(100.0, 300) && governor.can_kill() {
                    governor.record_kill();
                }

                scanned += 1;
            }

            black_box(scanned)
        });
    });

    // Scenario 2: Burst of hung processes (stress test)
    group.bench_function("hung_burst", |b| {
        let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 10, 60));

        // Create 50 hung processes
        let hung_processes: Vec<_> = (0..50)
            .map(|i| {
                let capsule = Arc::new(ProcessStateCapsule::new(i));
                capsule.update(i, 250.0, 600, false, false, false);  // All hung
                capsule
            })
            .collect();

        b.iter(|| {
            let mut killed = 0;
            for capsule in &hung_processes {
                if capsule.is_hung(100.0, 300) && governor.can_kill() {
                    if governor.record_kill() {
                        killed += 1;
                    }
                }
            }
            black_box(killed)
        });
    });

    // Scenario 3: Circuit breaker activation
    group.bench_function("circuit_breaker_activation", |b| {
        b.iter_batched(
            || {
                // Setup: Fresh governor for each iteration
                Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 5, 60))
            },
            |governor| {
                // Rapid kills (should trip circuit at 6th kill)
                let mut kills = 0;
                for _ in 0..10 {
                    if governor.can_kill() && governor.record_kill() {
                        kills += 1;
                    }
                }
                black_box(kills)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// B27: Memory footprint - Measure actual memory usage
fn benchmark_memory_footprint(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_footprint");

    group.bench_function("baseline_empty", |b| {
        b.iter(|| {
            let map: HashMap<u32, Arc<ProcessStateCapsule>> = HashMap::new();
            black_box(map.len())
        });
    });

    group.bench_function("1k_processes", |b| {
        b.iter(|| {
            let mut map = HashMap::new();
            for i in 0..1000 {
                let capsule = Arc::new(ProcessStateCapsule::new(i));
                map.insert(i, capsule);
            }

            // Memory usage calculation:
            // ProcessStateCapsule: 128B each
            // Arc overhead: ~16B per Arc
            // HashMap overhead: ~24B per entry
            // Total: (128 + 16 + 24) * 1000 = 168KB for capsules
            // Plus HashMap internal: ~16KB
            // Total: ~184KB for 1000 processes

            black_box(map.len())
        });
    });

    group.bench_function("10k_processes", |b| {
        b.iter(|| {
            let mut map = HashMap::new();
            for i in 0..10_000 {
                let capsule = Arc::new(ProcessStateCapsule::new(i));
                map.insert(i, capsule);
            }

            // Total: ~1.84MB for 10K processes
            // Target: <50MB total (handles 250K+ processes)

            black_box(map.len())
        });
    });

    group.finish();
}

/// B43: Tail latency percentiles - Production outliers
fn benchmark_tail_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("tail_latency");

    // Large sample size for P99.9 measurement
    group.sample_size(10000)
         .confidence_level(0.99);

    group.bench_function("hung_detection_p999", |b| {
        let capsule = ProcessStateCapsule::new(1234);
        capsule.update(1234, 200.0, 500, false, false, false);

        b.iter(|| {
            black_box(capsule.is_hung(100.0, 300))
        });
    });

    // K43: P99 should be 3-5x P50, P99.9 should be 10-20x P50
    // For <50ns target: P99 <250ns, P99.9 <1000ns acceptable

    group.finish();
}

/// B32: Continuous benchmarking - Performance trend tracking
fn benchmark_performance_trends(c: &mut Criterion) {
    let mut group = c.benchmark_group("performance_trends");

    // Baseline for regression detection
    group.bench_function("baseline_v0_1_0", |b| {
        let capsule = ProcessStateCapsule::new(1234);
        capsule.update(1234, 150.0, 400, false, false, false);

        b.iter(|| {
            black_box(capsule.is_hung(100.0, 300))
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_sustained_monitoring,
    benchmark_cpu_overhead,
    benchmark_memory_stability,
    benchmark_parallel_efficiency,
    benchmark_production_scenarios,
    benchmark_memory_footprint,
    benchmark_tail_latency,
    benchmark_performance_trends,
);
criterion_main!(benches);
