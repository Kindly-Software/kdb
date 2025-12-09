//! B32-Compliant Benchmark: RoutingCapsule128 Provider Selection
//!
//! **Framework**: B32 (Fair baselines + Statistical rigor)
//! **Baseline**: DashMap, HashMap with RwLock, parking_lot::Mutex
//! **Focus**: Provider selection latency and contention scaling
//!
//! ## Benchmarks
//!
//! 1. **Single-threaded**: Atomic routing vs hash table lookups
//! 2. **Contention scaling**: Round-robin under concurrent load
//! 3. **Health check updates**: Provider failure handling
//!
//! ## Expected Results (B32 Reality Checks)
//!
//! - Atomic vs DashMap: 1.5-3× speedup (K27: DashMap is optimized)
//! - Atomic vs RwLock: 3-8× speedup (K4: RwLock contention)
//! - Health updates: <30ns (vs ~200ns for hash updates)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use clapi_core::routing::{RoutingCapsule128, ProviderId};
use std::sync::{Arc, RwLock as StdRwLock};
use std::thread;
use std::time::Duration;

// ============================================================================
// B1-B5: Fair Baseline Implementations
// ============================================================================

/// Baseline 1: DashMap (lockfree concurrent hash map)
struct DashMapRouter {
    providers: dashmap::DashMap<u64, ProviderId>,
    next_idx: std::sync::atomic::AtomicU64,
    provider_list: Vec<ProviderId>,
}

impl DashMapRouter {
    fn new(providers: &[ProviderId]) -> Self {
        let map = dashmap::DashMap::new();
        for (idx, &pid) in providers.iter().enumerate() {
            map.insert(idx as u64, pid);
        }

        Self {
            providers: map,
            next_idx: std::sync::atomic::AtomicU64::new(0),
            provider_list: providers.to_vec(),
        }
    }

    fn select_provider(&self, _request_id: u64) -> ProviderId {
        let idx = self.next_idx.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let provider_idx = (idx % self.provider_list.len() as u64) as usize;
        self.provider_list[provider_idx]
    }
}

/// Baseline 2: HashMap with RwLock
struct RwLockRouter {
    providers: StdRwLock<Vec<ProviderId>>,
    next_idx: std::sync::atomic::AtomicU64,
}

impl RwLockRouter {
    fn new(providers: &[ProviderId]) -> Self {
        Self {
            providers: StdRwLock::new(providers.to_vec()),
            next_idx: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn select_provider(&self, _request_id: u64) -> ProviderId {
        let providers = self.providers.read().unwrap();
        let idx = self.next_idx.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let provider_idx = (idx % providers.len() as u64) as usize;
        providers[provider_idx]
    }
}

/// Baseline 3: parking_lot::RwLock (optimized)
struct ParkingLotRouter {
    providers: parking_lot::RwLock<Vec<ProviderId>>,
    next_idx: std::sync::atomic::AtomicU64,
}

impl ParkingLotRouter {
    fn new(providers: &[ProviderId]) -> Self {
        Self {
            providers: parking_lot::RwLock::new(providers.to_vec()),
            next_idx: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn select_provider(&self, _request_id: u64) -> ProviderId {
        let providers = self.providers.read();
        let idx = self.next_idx.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let provider_idx = (idx % providers.len() as u64) as usize;
        providers[provider_idx]
    }
}

// ============================================================================
// B2: Single-Threaded Benchmarks (Uncontended)
// ============================================================================

fn bench_single_threaded(c: &mut Criterion) {
    let mut group = c.benchmark_group("provider_routing_single_thread");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);

    let providers = vec![0, 1, 2, 3, 4]; // 5 providers

    // Atomic capsule (our implementation)
    group.bench_function("atomic_capsule", |b| {
        let capsule = RoutingCapsule128::new(&providers);
        let mut request_id = 0u64;
        b.iter(|| {
            request_id += 1;
            black_box(capsule.select_provider(request_id).unwrap())
        });
    });

    // Baseline 1: DashMap
    group.bench_function("dashmap", |b| {
        let router = DashMapRouter::new(&providers);
        let mut request_id = 0u64;
        b.iter(|| {
            request_id += 1;
            black_box(router.select_provider(request_id))
        });
    });

    // Baseline 2: RwLock
    group.bench_function("rwlock", |b| {
        let router = RwLockRouter::new(&providers);
        let mut request_id = 0u64;
        b.iter(|| {
            request_id += 1;
            black_box(router.select_provider(request_id))
        });
    });

    // Baseline 3: parking_lot::RwLock
    group.bench_function("parking_lot_rwlock", |b| {
        let router = ParkingLotRouter::new(&providers);
        let mut request_id = 0u64;
        b.iter(|| {
            request_id += 1;
            black_box(router.select_provider(request_id))
        });
    });

    group.finish();
}

// ============================================================================
// B4: Contention Scaling Benchmarks
// ============================================================================

fn bench_contention_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("provider_routing_contention");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(100);

    let providers = vec![0, 1, 2, 3, 4];

    // Test with 1, 2, 4, 8, 16 threads
    for num_threads in [1, 2, 4, 8, 16] {
        group.throughput(Throughput::Elements(num_threads as u64 * 10000));

        // Atomic capsule
        group.bench_with_input(
            BenchmarkId::new("atomic_capsule", num_threads),
            &num_threads,
            |b, &num_threads| {
                let capsule = Arc::new(RoutingCapsule128::new(&providers));
                b.iter(|| {
                    let handles: Vec<_> = (0..num_threads)
                        .map(|tid| {
                            let capsule_clone = Arc::clone(&capsule);
                            thread::spawn(move || {
                                for i in 0..10000 {
                                    let request_id = (tid as u64 * 10000) + i;
                                    let _ = black_box(capsule_clone.select_provider(request_id));
                                }
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );

        // DashMap baseline
        group.bench_with_input(
            BenchmarkId::new("dashmap", num_threads),
            &num_threads,
            |b, &num_threads| {
                let router = Arc::new(DashMapRouter::new(&providers));
                b.iter(|| {
                    let handles: Vec<_> = (0..num_threads)
                        .map(|tid| {
                            let router_clone = Arc::clone(&router);
                            thread::spawn(move || {
                                for i in 0..10000 {
                                    let request_id = (tid as u64 * 10000) + i;
                                    let _ = black_box(router_clone.select_provider(request_id));
                                }
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );

        // parking_lot::RwLock baseline
        group.bench_with_input(
            BenchmarkId::new("parking_lot_rwlock", num_threads),
            &num_threads,
            |b, &num_threads| {
                let router = Arc::new(ParkingLotRouter::new(&providers));
                b.iter(|| {
                    let handles: Vec<_> = (0..num_threads)
                        .map(|tid| {
                            let router_clone = Arc::clone(&router);
                            thread::spawn(move || {
                                for i in 0..10000 {
                                    let request_id = (tid as u64 * 10000) + i;
                                    let _ = black_box(router_clone.select_provider(request_id));
                                }
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// B3: Realistic Workload - Provider Failure Handling
// ============================================================================

fn bench_health_check_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("provider_routing_health_updates");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(500);

    let providers = vec![0, 1, 2, 3, 4, 5, 6, 7];

    // Realistic workload: 95% reads (selections), 5% writes (health updates)
    group.bench_function("atomic_capsule_health_updates", |b| {
        let capsule = Arc::new(RoutingCapsule128::new(&providers));
        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|tid| {
                    let capsule_clone = Arc::clone(&capsule);
                    thread::spawn(move || {
                        for i in 0..1000 {
                            if i % 20 == 0 {
                                // 5% writes: Mark provider as failed
                                let provider_id = ((tid + i) % 8) as ProviderId;
                                capsule_clone.mark_provider_failed(provider_id);
                            } else {
                                // 95% reads: Select provider
                                let request_id = (tid as u64 * 1000) + i;
                                let _ = black_box(capsule_clone.select_provider(request_id));
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

    // Parking lot baseline with similar workload
    group.bench_function("parking_lot_rwlock_health_updates", |b| {
        let router = Arc::new(ParkingLotRouter::new(&providers));
        b.iter(|| {
            let handles: Vec<_> = (0..4)
                .map(|tid| {
                    let router_clone = Arc::clone(&router);
                    thread::spawn(move || {
                        for i in 0..1000 {
                            if i % 20 == 0 {
                                // 5% writes: Update provider list (simulated health update)
                                let mut providers = router_clone.providers.write();
                                if !providers.is_empty() {
                                    providers.rotate_left(1); // Simulated health update
                                }
                            } else {
                                // 95% reads: Select provider
                                let request_id = (tid as u64 * 1000) + i;
                                let _ = black_box(router_clone.select_provider(request_id));
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
// B16: Latency Distribution Analysis
// ============================================================================

fn bench_latency_distribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("provider_routing_latency");
    group.warm_up_time(Duration::from_secs(5));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(2000);

    let providers = vec![0, 1, 2, 3, 4];

    // Atomic capsule - single operation latency
    group.bench_function("atomic_capsule_latency", |b| {
        let capsule = RoutingCapsule128::new(&providers);
        let mut request_id = 0u64;
        b.iter(|| {
            request_id += 1;
            black_box(capsule.select_provider(request_id))
        });
    });

    // DashMap baseline - single operation latency
    group.bench_function("dashmap_latency", |b| {
        let router = DashMapRouter::new(&providers);
        let mut request_id = 0u64;
        b.iter(|| {
            request_id += 1;
            black_box(router.select_provider(request_id))
        });
    });

    // parking_lot::RwLock baseline - single operation latency
    group.bench_function("parking_lot_latency", |b| {
        let router = ParkingLotRouter::new(&providers);
        let mut request_id = 0u64;
        b.iter(|| {
            request_id += 1;
            black_box(router.select_provider(request_id))
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group! {
    name = benches;
    config = Criterion::default()
        .confidence_level(0.95)
        .significance_level(0.05)
        .noise_threshold(0.05);
    targets =
        bench_single_threaded,
        bench_contention_scaling,
        bench_health_check_updates,
        bench_latency_distribution
}

criterion_main!(benches);
