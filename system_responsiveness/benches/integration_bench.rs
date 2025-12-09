/// Integration benchmarks for full monitoring cycles (B32 Framework)
/// Target: <10ms for 100 processes, <100ms for 1000 processes, <1s for 10K processes
/// Real /proc filesystem, real process scanning, real hung detection

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{System, ProcessesToUpdate};
use sysrespond::capsules::{ProcessStateCapsule, ResourceGovernorCapsule};

/// B3: Realistic workloads - Full monitoring cycle with real sysinfo
fn benchmark_full_scan_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_scan_cycle");

    // B2: Statistical rigor with longer measurement for I/O operations
    group.confidence_level(0.95)
         .sample_size(50)  // Smaller for expensive I/O operations
         .warm_up_time(Duration::from_secs(5))
         .measurement_time(Duration::from_secs(30));

    // Real system scan (actual /proc filesystem)
    group.bench_function("real_system_scan", |b| {
        let mut sys = System::new_all();
        let _governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 10, 60));
        let mut process_map: HashMap<u32, Arc<ProcessStateCapsule>> = HashMap::new();

        b.iter(|| {
            // Refresh system info (reads /proc)
            sys.refresh_processes(ProcessesToUpdate::All);

            let mut scanned = 0;
            let mut hung_detected = 0;

            // Scan all processes
            for (pid, process) in sys.processes() {
                scanned += 1;

                let pid_u32 = pid.as_u32();
                let cpu_pct = process.cpu_usage() as f64;
                let runtime_sec = process.run_time();

                // Get or create capsule
                let capsule = process_map.entry(pid_u32)
                    .or_insert_with(|| Arc::new(ProcessStateCapsule::new(pid_u32)));

                // Update state
                capsule.update(pid_u32, cpu_pct, runtime_sec, false, false, false);

                // Check if hung
                if capsule.is_hung(100.0, 300) {
                    hung_detected += 1;
                }
            }

            // Cleanup dead processes
            process_map.retain(|pid, _| {
                sys.process(sysinfo::Pid::from_u32(*pid)).is_some()
            });

            black_box((scanned, hung_detected))
        });
    });

    group.finish();
}

/// B18: Scalability limits - Test with varying process counts
fn benchmark_scan_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("scan_scalability");

    group.confidence_level(0.95)
         .sample_size(20);

    // Create synthetic process lists of varying sizes
    for count in [100, 500, 1000, 2000, 5000].iter() {
        group.throughput(Throughput::Elements(*count as u64));

        group.bench_with_input(
            BenchmarkId::new("synthetic_processes", count),
            count,
            |b, &process_count| {
                // Pre-create capsules for synthetic processes
                let mut process_map: HashMap<u32, Arc<ProcessStateCapsule>> = HashMap::new();
                for i in 0..process_count {
                    let capsule = Arc::new(ProcessStateCapsule::new(i as u32));
                    // Simulate various states
                    let cpu_pct = (i % 300) as f64;
                    let runtime = (i % 600) as u64;
                    capsule.update(i as u32, cpu_pct, runtime, false, false, false);
                    process_map.insert(i as u32, capsule);
                }

                let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 100, 60));

                b.iter(|| {
                    let mut hung_count = 0;

                    // Scan all processes
                    for (pid, capsule) in &process_map {
                        // Simulate update from sysinfo
                        let cpu_pct = (*pid % 300) as f64;
                        let runtime = (*pid % 600) as u64;
                        capsule.update(*pid, cpu_pct, runtime, false, false, false);

                        // Check hung
                        if capsule.is_hung(100.0, 300) {
                            hung_count += 1;

                            if governor.can_kill() {
                                governor.record_kill();
                            }
                        }
                    }

                    black_box(hung_count)
                });
            },
        );
    }

    group.finish();
}

/// B19: Warmup period validation - Measure cache warming effects
fn benchmark_cache_warmup_effects(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_warmup");

    // Test cold vs warm cache performance
    group.bench_function("cold_cache_scan", |b| {
        b.iter_batched(
            || {
                // Setup: Fresh capsules every iteration (cold cache)
                let mut map = HashMap::new();
                for i in 0..1000 {
                    let capsule = Arc::new(ProcessStateCapsule::new(i));
                    capsule.update(i, (i % 300) as f64, (i % 600) as u64, false, false, false);
                    map.insert(i, capsule);
                }
                map
            },
            |map| {
                // Measure: Scan with cold cache
                let mut count = 0;
                for capsule in map.values() {
                    if capsule.is_hung(100.0, 300) {
                        count += 1;
                    }
                }
                black_box(count)
            },
            criterion::BatchSize::LargeInput,
        );
    });

    group.bench_function("warm_cache_scan", |b| {
        // Setup once: Capsules stay hot in cache
        let mut map = HashMap::new();
        for i in 0..1000 {
            let capsule = Arc::new(ProcessStateCapsule::new(i));
            capsule.update(i, (i % 300) as f64, (i % 600) as u64, false, false, false);
            map.insert(i, capsule);
        }

        b.iter(|| {
            // Measure: Scan with warm cache
            let mut count = 0;
            for capsule in map.values() {
                if capsule.is_hung(100.0, 300) {
                    count += 1;
                }
            }
            black_box(count)
        });
    });

    group.finish();
}

/// B15: Lock contention analysis - Circuit breaker under contention
fn benchmark_circuit_breaker_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker_contention");

    group.confidence_level(0.95)
         .sample_size(100);

    // Single-threaded baseline (no contention)
    group.bench_function("no_contention", |b| {
        let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 100, 60));

        b.iter(|| {
            for _ in 0..100 {
                governor.record_kill();
            }
        });
    });

    // Multi-threaded contention (4 threads simulated sequentially)
    // Note: Actual parallel benchmarks require different setup
    group.bench_function("sequential_kills", |b| {
        let governor = Arc::new(ResourceGovernorCapsule::new(100.0, 4096, 100, 60));

        b.iter(|| {
            // Simulate multiple processes recording kills
            for _ in 0..100 {
                governor.record_kill();
            }
        });
    });

    group.finish();
}

/// B14: Memory bandwidth saturation - Large-scale process maps
fn benchmark_memory_bandwidth(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_bandwidth");

    // K3: Memory bandwidth reality check (15.2GB/s sequential, 3-5GB/s random)
    group.bench_function("large_hashmap_iteration", |b| {
        // 10K processes = ~1.28MB of ProcessStateCapsule data
        let mut map = HashMap::new();
        for i in 0..10_000 {
            let capsule = Arc::new(ProcessStateCapsule::new(i));
            capsule.update(i, 100.0, 500, false, false, false);
            map.insert(i, capsule);
        }

        b.iter(|| {
            let mut hung = 0;
            for capsule in map.values() {
                if capsule.is_hung(100.0, 300) {
                    hung += 1;
                }
            }
            black_box(hung)
        });
    });

    group.finish();
}

/// B17: Detection latency - Time from state change to detection
fn benchmark_detection_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("detection_latency");

    group.confidence_level(0.95)
         .sample_size(1000);

    // Measure time from update to hung detection
    group.bench_function("update_to_detection", |b| {
        let capsule = ProcessStateCapsule::new(1234);

        b.iter(|| {
            // Step 1: Update to hung state
            capsule.update(1234, 200.0, 500, false, false, false);

            // Step 2: Immediate detection (should be <100ns total)
            let is_hung = capsule.is_hung(100.0, 300);

            black_box(is_hung)
        });
    });

    // Measure full reaction chain
    group.bench_function("full_reaction_chain", |b| {
        let capsule = ProcessStateCapsule::new(5678);
        let governor = ResourceGovernorCapsule::new(100.0, 4096, 10, 60);

        b.iter(|| {
            // 1. Update state
            capsule.update(5678, 250.0, 600, false, false, false);

            // 2. Detect hung
            let is_hung = capsule.is_hung(100.0, 300);

            // 3. Check circuit breaker
            let can_kill = if is_hung {
                governor.can_kill()
            } else {
                false
            };

            // 4. Record kill decision
            if can_kill {
                governor.record_kill();
            }

            black_box((is_hung, can_kill))
        });
    });

    group.finish();
}

/// B30: Cost-benefit analysis - Performance vs complexity
fn benchmark_capsule_vs_mutex(c: &mut Criterion) {
    use std::sync::Mutex;

    let mut group = c.benchmark_group("capsule_vs_mutex");

    // B1: Fair baseline - parking_lot::Mutex (optimized)
    // Note: Using std::Mutex as baseline (parking_lot would be fairer)
    group.bench_function("mutex_baseline", |b| {
        struct ProcessState {
            _pid: u32,
            cpu_pct: f64,
            runtime_sec: u64,
            _is_hung: bool,
        }

        let state = Mutex::new(ProcessState {
            _pid: 1234,
            cpu_pct: 100.0,
            runtime_sec: 500,
            _is_hung: false,
        });

        b.iter(|| {
            let s = state.lock().unwrap();
            let hung = s.cpu_pct > 100.0 && s.runtime_sec > 300;
            black_box(hung)
        });
    });

    group.bench_function("capsule_optimized", |b| {
        let capsule = ProcessStateCapsule::new(1234);
        capsule.update(1234, 100.0, 500, false, false, false);

        b.iter(|| {
            black_box(capsule.is_hung(100.0, 300))
        });
    });

    // Speedup calculation will be in report
    group.finish();
}

criterion_group!(
    benches,
    benchmark_full_scan_cycle,
    benchmark_scan_scalability,
    benchmark_cache_warmup_effects,
    benchmark_circuit_breaker_contention,
    benchmark_memory_bandwidth,
    benchmark_detection_latency,
    benchmark_capsule_vs_mutex,
);
criterion_main!(benches);
