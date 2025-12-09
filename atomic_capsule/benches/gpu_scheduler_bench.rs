// GPU Scheduler Capsule - B32 Benchmarks
// Validates multi-engine work submission with fair baselines (Mutex, RwLock)
// Target: <200ns submit, <50ns load query, 5-10× vs sequential
//
// UCE34 Framework: B32 (1000+ iterations, 95% CI, fair baselines)
// Test Hardware: AMD Ryzen 9 6900HX (2.4-4.5 GHz), 64GB DDR5-4800

use atomic_capsule::gpu::hal::{GpuEngine, GpuSchedulerCapsule};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};

// ============================================================================
// FAIR BASELINE: Sequential scheduler (Mutex-protected per-engine loads)
// ============================================================================

struct SequentialScheduler {
    rcs_load: Arc<Mutex<u16>>,
    ccs_load: Arc<Mutex<u16>>,
    bcs_load: Arc<Mutex<u16>>,
    vecs_load: Arc<Mutex<u16>>,
}

impl SequentialScheduler {
    fn new() -> Self {
        Self {
            rcs_load: Arc::new(Mutex::new(0)),
            ccs_load: Arc::new(Mutex::new(0)),
            bcs_load: Arc::new(Mutex::new(0)),
            vecs_load: Arc::new(Mutex::new(0)),
        }
    }

    fn submit_workload(&self) -> Result<(u32, u16), &'static str> {
        // Find least-loaded engine (requires acquiring 4 mutex locks sequentially)
        let rcs = *self.rcs_load.lock().unwrap();
        let ccs = *self.ccs_load.lock().unwrap();
        let bcs = *self.bcs_load.lock().unwrap();
        let vecs = *self.vecs_load.lock().unwrap();

        let min_load = rcs.min(ccs).min(bcs).min(vecs);
        if min_load > 10_000 {
            return Err("all_engines_overloaded");
        }

        // Increment selected engine
        let engine_idx = if rcs == min_load {
            0
        } else if ccs == min_load {
            1
        } else if bcs == min_load {
            2
        } else {
            3
        };

        let new_load = match engine_idx {
            0 => {
                let mut load = self.rcs_load.lock().unwrap();
                *load = load.saturating_add(1);
                *load
            }
            1 => {
                let mut load = self.ccs_load.lock().unwrap();
                *load = load.saturating_add(1);
                *load
            }
            2 => {
                let mut load = self.bcs_load.lock().unwrap();
                *load = load.saturating_add(1);
                *load
            }
            3 => {
                let mut load = self.vecs_load.lock().unwrap();
                *load = load.saturating_add(1);
                *load
            }
            _ => unreachable!(),
        };

        Ok((engine_idx, new_load))
    }

    fn get_engine_load(&self, engine: u32) -> u16 {
        match engine {
            0 => *self.rcs_load.lock().unwrap(),
            1 => *self.ccs_load.lock().unwrap(),
            2 => *self.bcs_load.lock().unwrap(),
            3 => *self.vecs_load.lock().unwrap(),
            _ => 0,
        }
    }
}

impl Clone for SequentialScheduler {
    fn clone(&self) -> Self {
        Self {
            rcs_load: Arc::clone(&self.rcs_load),
            ccs_load: Arc::clone(&self.ccs_load),
            bcs_load: Arc::clone(&self.bcs_load),
            vecs_load: Arc::clone(&self.vecs_load),
        }
    }
}

// ============================================================================
// BENCHMARK: Submit Workload (Least-Loaded)
// ============================================================================

fn bench_submit_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("submit_workload");
    group.sample_size(1000);
    group.measurement_time(std::time::Duration::from_secs(10));

    // GpuSchedulerCapsule implementation
    let capsule = GpuSchedulerCapsule::new();

    // Sequential baseline (4 mutex locks)
    let baseline = SequentialScheduler::new();

    // Benchmark: Capsule least-loaded submission
    // Target: <200ns
    group.bench_function("capsule_least_loaded", |b| {
        b.iter(|| black_box(capsule.submit_workload()))
    });

    // Benchmark: Sequential baseline (4 mutex acquisitions)
    group.bench_function("baseline_mutex_4locks", |b| {
        b.iter(|| black_box(baseline.submit_workload()))
    });

    // Benchmark: Capsule specific engine (render)
    group.bench_function("capsule_submit_render", |b| {
        b.iter(|| black_box(capsule.submit_render()))
    });

    // Benchmark: Capsule specific engine (compute)
    group.bench_function("capsule_submit_compute", |b| {
        b.iter(|| black_box(capsule.submit_compute()))
    });

    // Benchmark: Capsule specific engine (copy)
    group.bench_function("capsule_submit_copy", |b| {
        b.iter(|| black_box(capsule.submit_copy()))
    });

    // Benchmark: Capsule specific engine (video)
    group.bench_function("capsule_submit_video", |b| {
        b.iter(|| black_box(capsule.submit_video()))
    });

    group.finish();
}

// ============================================================================
// BENCHMARK: Load Query
// ============================================================================

fn bench_load_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_query");
    group.sample_size(1000);
    group.measurement_time(std::time::Duration::from_secs(10));

    // GpuSchedulerCapsule implementation
    let capsule = GpuSchedulerCapsule::new();
    // Pre-populate with some load
    for _ in 0..100 {
        let _ = capsule.submit_workload();
    }

    // Baseline (mutex lock)
    let baseline = SequentialScheduler::new();
    for _ in 0..100 {
        let _ = baseline.submit_workload();
    }

    // Benchmark: Get single engine load (RCS)
    // Target: <50ns (Acquire ordering atomic read)
    group.bench_function("capsule_get_rcs_load", |b| {
        b.iter(|| black_box(capsule.get_engine_load(GpuEngine::RCS)))
    });

    // Benchmark: Get single engine load (CCS)
    group.bench_function("capsule_get_ccs_load", |b| {
        b.iter(|| black_box(capsule.get_engine_load(GpuEngine::CCS)))
    });

    // Benchmark: Get snapshot of all engines
    // Target: <200ns (4 parallel reads + unpack)
    group.bench_function("capsule_snapshot_all", |b| {
        b.iter(|| black_box(capsule.snapshot()))
    });

    // Baseline: Get single engine load
    group.bench_function("baseline_mutex_get_load", |b| {
        b.iter(|| black_box(baseline.get_engine_load(0)))
    });

    group.finish();
}

// ============================================================================
// BENCHMARK: Complete Workload
// ============================================================================

fn bench_complete_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("complete_workload");
    group.sample_size(1000);
    group.measurement_time(std::time::Duration::from_secs(10));

    // GpuSchedulerCapsule implementation
    let capsule = GpuSchedulerCapsule::new();
    capsule.submit_render().ok();
    capsule.submit_compute().ok();
    capsule.submit_copy().ok();
    capsule.submit_video().ok();

    // Benchmark: Complete single workload
    group.bench_function("capsule_complete_workload", |b| {
        b.iter(|| {
            capsule.submit_render().ok();
            capsule.complete_workload(GpuEngine::RCS)
        })
    });

    group.finish();
}

// ============================================================================
// BENCHMARK: Load Balancing
// ============================================================================

fn bench_load_balancing(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_balancing");
    group.sample_size(100);
    group.measurement_time(std::time::Duration::from_secs(10));

    let capsule = GpuSchedulerCapsule::new();

    // Create imbalance
    for _ in 0..50 {
        capsule.submit_render().ok();
    }
    for _ in 0..20 {
        capsule.submit_compute().ok();
    }
    for _ in 0..10 {
        capsule.submit_copy().ok();
    }
    for _ in 0..5 {
        capsule.submit_video().ok();
    }

    // Benchmark: Identify overloaded engines
    // Target: <10μs for 4 engines
    group.bench_function("capsule_balance_load", |b| {
        b.iter(|| black_box(capsule.balance_load()))
    });

    group.finish();
}

// ============================================================================
// BENCHMARK: Reset Operations
// ============================================================================

fn bench_reset(c: &mut Criterion) {
    let mut group = c.benchmark_group("reset");
    group.sample_size(100);
    group.measurement_time(std::time::Duration::from_secs(10));

    let capsule = GpuSchedulerCapsule::new();

    // Pre-populate
    for _ in 0..100 {
        capsule.submit_workload().ok();
    }

    // Benchmark: Reset single engine
    group.bench_function("capsule_reset_single_engine", |b| {
        b.iter(|| {
            capsule.submit_render().ok();
            capsule.reset_engine(GpuEngine::RCS);
        })
    });

    // Benchmark: Reset all engines
    group.bench_function("capsule_reset_all", |b| {
        b.iter(|| {
            capsule.submit_workload().ok();
            capsule.reset_all();
        })
    });

    group.finish();
}

// ============================================================================
// BENCHMARK: Multi-threaded stress (contention analysis)
// ============================================================================

fn bench_concurrent_submit(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_submit");
    group.sample_size(100);
    group.measurement_time(std::time::Duration::from_secs(10));

    // Benchmark: 4-threaded concurrent submission
    group.bench_function("capsule_4threads_100ops", |b| {
        b.iter(|| {
            let capsule: Arc<GpuSchedulerCapsule> = Arc::new(GpuSchedulerCapsule::new());
            let mut handles = vec![];

            for _ in 0..4 {
                let cap = Arc::clone(&capsule);
                let handle = std::thread::spawn(move || {
                    for _ in 0..25 {
                        cap.submit_workload().ok();
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }

            black_box(capsule.snapshot())
        })
    });

    // Benchmark: 8-threaded concurrent submission
    group.bench_function("capsule_8threads_100ops", |b| {
        b.iter(|| {
            let capsule: Arc<GpuSchedulerCapsule> = Arc::new(GpuSchedulerCapsule::new());
            let mut handles = vec![];

            for _ in 0..8 {
                let cap = Arc::clone(&capsule);
                let handle = std::thread::spawn(move || {
                    for _ in 0..12 {
                        cap.submit_workload().ok();
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }

            black_box(capsule.snapshot())
        })
    });

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    benches,
    bench_submit_workload,
    bench_load_query,
    bench_complete_workload,
    bench_load_balancing,
    bench_reset,
    bench_concurrent_submit
);

criterion_main!(benches);
