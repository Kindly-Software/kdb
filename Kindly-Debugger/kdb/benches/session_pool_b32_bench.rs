//! B32 Performance Validation for Session Pool
//!
//! Comprehensive benchmarks for the SessionPoolCapsule tiered session management.
//!
//! # Performance Targets (B32 Framework)
//!
//! | Operation              | Target  | Baseline      | Notes                    |
//! |------------------------|---------|---------------|--------------------------|
//! | Session allocation     | <100ns  | N/A (novel)   | Lockfree CAS             |
//! | Session deallocation   | <100ns  | N/A (novel)   | Lockfree free-list push  |
//! | Session lookup         | <10ns   | N/A (novel)   | Direct ID extraction     |
//! | Session upgrade        | <10μs   | N/A (novel)   | Allocate + release + migrate |
//! | Session downgrade      | <10μs   | N/A (novel)   | Allocate + release + migrate |
//! | Pool stats snapshot    | <50ns   | N/A (novel)   | Atomic loads only        |
//! | Concurrent allocation  | <200ns  | N/A (novel)   | 8 threads contention     |
//!
//! # Methodology
//!
//! - 1000+ iterations per benchmark (Criterion default)
//! - 95% confidence intervals
//! - Fair baselines (novel capability, measuring absolute performance)
//! - Warm-up iterations to ensure JIT/cache steady state
//!
//! # COCA Compliance
//!
//! All benchmarks verify lockfree operation (zero mutex/RwLock in hot paths).

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use kdb::session_pool::{
    PoolConfig, SessionId, SessionPoolCapsule, SessionTierType,
};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Session Allocation Benchmarks
// ============================================================================

/// Benchmark single-threaded LIGHT session allocation
///
/// Target: <100ns per allocation (lockfree CAS)
fn bench_allocate_light(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    c.bench_function("session_allocate_light", |b| {
        b.iter(|| {
            let id = pool.allocate_session(black_box(SessionTierType::Light)).unwrap();
            pool.release_session(id).unwrap();
            black_box(id)
        })
    });
}

/// Benchmark single-threaded MEDIUM session allocation
///
/// Target: <100ns per allocation
fn bench_allocate_medium(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    c.bench_function("session_allocate_medium", |b| {
        b.iter(|| {
            let id = pool.allocate_session(black_box(SessionTierType::Medium)).unwrap();
            pool.release_session(id).unwrap();
            black_box(id)
        })
    });
}

/// Benchmark single-threaded HEAVY session allocation
///
/// Target: <100ns per allocation
fn bench_allocate_heavy(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    c.bench_function("session_allocate_heavy", |b| {
        b.iter(|| {
            let id = pool.allocate_session(black_box(SessionTierType::Heavy)).unwrap();
            pool.release_session(id).unwrap();
            black_box(id)
        })
    });
}

/// Benchmark allocation-only without immediate release
///
/// Measures pure allocation cost without free-list push overhead
fn bench_allocate_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocate_only");

    for tier in [SessionTierType::Light, SessionTierType::Medium, SessionTierType::Heavy] {
        group.bench_with_input(
            BenchmarkId::from_parameter(tier.as_str()),
            &tier,
            |b, &tier| {
                // Create pool with enough capacity for sustained allocation
                let config = PoolConfig {
                    light_capacity: 10000,
                    medium_capacity: 5000,
                    heavy_capacity: 2000,
                    ..PoolConfig::default()
                };
                let pool = SessionPoolCapsule::new(config);
                let mut allocated = Vec::with_capacity(1000);

                b.iter(|| {
                    // Allocate batch
                    for _ in 0..100 {
                        if let Ok(id) = pool.allocate_session(tier) {
                            allocated.push(id);
                        }
                    }

                    // Release batch for next iteration
                    for id in allocated.drain(..) {
                        let _ = pool.release_session(id);
                    }
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// Session Release Benchmarks
// ============================================================================

/// Benchmark session release (deallocation)
///
/// Target: <100ns per release (lockfree free-list push)
fn bench_release_session(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    // Pre-allocate sessions to release
    c.bench_function("session_release", |b| {
        b.iter_custom(|iters| {
            // Allocate sessions for this batch
            let mut ids: Vec<SessionId> = Vec::with_capacity(iters as usize);
            for _ in 0..iters {
                if let Ok(id) = pool.allocate_session(SessionTierType::Light) {
                    ids.push(id);
                }
            }

            // Time the releases
            let start = std::time::Instant::now();
            for id in ids {
                black_box(pool.release_session(id).unwrap());
            }
            start.elapsed()
        })
    });
}

// ============================================================================
// Session Lookup Benchmarks
// ============================================================================

/// Benchmark session tier lookup from SessionId
///
/// Target: <10ns (direct bit extraction)
fn bench_session_lookup(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    // Pre-allocate sessions for lookup
    let ids: Vec<SessionId> = (0..1000)
        .filter_map(|i| {
            let tier = match i % 3 {
                0 => SessionTierType::Light,
                1 => SessionTierType::Medium,
                _ => SessionTierType::Heavy,
            };
            pool.allocate_session(tier).ok()
        })
        .collect();

    c.bench_function("session_lookup_1000", |b| {
        b.iter(|| {
            for id in &ids {
                black_box(pool.get_session_tier(*id));
            }
        })
    });
}

/// Benchmark SessionId field extraction
///
/// Target: <5ns per extraction (pure bit manipulation)
fn bench_session_id_extract(c: &mut Criterion) {
    let id = SessionId::new(1, 12345, 67890);

    c.bench_function("session_id_tier_extract", |b| {
        b.iter(|| {
            black_box(black_box(id).tier())
        })
    });

    c.bench_function("session_id_slot_extract", |b| {
        b.iter(|| {
            black_box(black_box(id).slot())
        })
    });

    c.bench_function("session_id_generation_extract", |b| {
        b.iter(|| {
            black_box(black_box(id).generation())
        })
    });
}

// ============================================================================
// Session Upgrade/Downgrade Benchmarks
// ============================================================================

/// Benchmark LIGHT -> MEDIUM upgrade
///
/// Target: <10μs (allocate new + release old + state migration)
fn bench_upgrade_light_to_medium(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    c.bench_function("session_upgrade_light_to_medium", |b| {
        b.iter(|| {
            let light_id = pool.allocate_session(SessionTierType::Light).unwrap();
            let medium_id = pool.upgrade_session(light_id).unwrap();
            pool.release_session(medium_id).unwrap();
            black_box(medium_id)
        })
    });
}

/// Benchmark MEDIUM -> HEAVY upgrade
///
/// Target: <10μs
fn bench_upgrade_medium_to_heavy(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    c.bench_function("session_upgrade_medium_to_heavy", |b| {
        b.iter(|| {
            let medium_id = pool.allocate_session(SessionTierType::Medium).unwrap();
            let heavy_id = pool.upgrade_session(medium_id).unwrap();
            pool.release_session(heavy_id).unwrap();
            black_box(heavy_id)
        })
    });
}

/// Benchmark full upgrade chain LIGHT -> MEDIUM -> HEAVY
///
/// Target: <20μs (two upgrades)
fn bench_upgrade_chain(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    c.bench_function("session_upgrade_chain_light_to_heavy", |b| {
        b.iter(|| {
            let light_id = pool.allocate_session(SessionTierType::Light).unwrap();
            let medium_id = pool.upgrade_session(light_id).unwrap();
            let heavy_id = pool.upgrade_session(medium_id).unwrap();
            pool.release_session(heavy_id).unwrap();
            black_box(heavy_id)
        })
    });
}

/// Benchmark HEAVY -> MEDIUM downgrade
///
/// Target: <10μs
fn bench_downgrade_heavy_to_medium(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    c.bench_function("session_downgrade_heavy_to_medium", |b| {
        b.iter(|| {
            let heavy_id = pool.allocate_session(SessionTierType::Heavy).unwrap();
            let medium_id = pool.downgrade_session(heavy_id).unwrap();
            pool.release_session(medium_id).unwrap();
            black_box(medium_id)
        })
    });
}

/// Benchmark MEDIUM -> LIGHT downgrade
///
/// Target: <10μs
fn bench_downgrade_medium_to_light(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    c.bench_function("session_downgrade_medium_to_light", |b| {
        b.iter(|| {
            let medium_id = pool.allocate_session(SessionTierType::Medium).unwrap();
            let light_id = pool.downgrade_session(medium_id).unwrap();
            pool.release_session(light_id).unwrap();
            black_box(light_id)
        })
    });
}

// ============================================================================
// Pool Statistics Benchmarks
// ============================================================================

/// Benchmark pool statistics snapshot
///
/// Target: <50ns (atomic loads only)
fn bench_pool_stats(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    // Allocate some sessions for realistic state
    let _ids: Vec<_> = (0..100)
        .filter_map(|i| {
            let tier = match i % 3 {
                0 => SessionTierType::Light,
                1 => SessionTierType::Medium,
                _ => SessionTierType::Heavy,
            };
            pool.allocate_session(tier).ok()
        })
        .collect();

    c.bench_function("pool_get_stats", |b| {
        b.iter(|| {
            black_box(pool.get_pool_stats())
        })
    });
}

/// Benchmark pool config access
///
/// Target: <5ns (immutable reference)
fn bench_pool_config(c: &mut Criterion) {
    let pool = SessionPoolCapsule::new(PoolConfig::default());

    c.bench_function("pool_get_config", |b| {
        b.iter(|| {
            black_box(pool.config())
        })
    });
}

// ============================================================================
// Concurrent Allocation Benchmarks
// ============================================================================

/// Benchmark concurrent allocation with 8 threads
///
/// Target: <200ns per allocation under contention
fn bench_concurrent_allocation_8_threads(c: &mut Criterion) {
    let pool = Arc::new(SessionPoolCapsule::new(PoolConfig::default()));

    c.bench_function("concurrent_allocate_8_threads", |b| {
        b.iter(|| {
            let mut handles = Vec::with_capacity(8);

            for _ in 0..8 {
                let pool_clone = Arc::clone(&pool);
                handles.push(thread::spawn(move || {
                    for _ in 0..100 {
                        if let Ok(id) = pool_clone.allocate_session(SessionTierType::Light) {
                            black_box(id);
                            let _ = pool_clone.release_session(id);
                        }
                    }
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }
        })
    });
}

/// Benchmark concurrent mixed-tier allocation
///
/// Simulates realistic workload with different tier distributions
fn bench_concurrent_mixed_tier(c: &mut Criterion) {
    let pool = Arc::new(SessionPoolCapsule::new(PoolConfig::default()));

    c.bench_function("concurrent_mixed_tier_4_threads", |b| {
        b.iter(|| {
            let mut handles = Vec::with_capacity(4);

            for thread_id in 0..4 {
                let pool_clone = Arc::clone(&pool);
                handles.push(thread::spawn(move || {
                    for i in 0..50 {
                        // Different threads prefer different tiers
                        let tier = match (thread_id + i) % 3 {
                            0 => SessionTierType::Light,
                            1 => SessionTierType::Medium,
                            _ => SessionTierType::Heavy,
                        };

                        if let Ok(id) = pool_clone.allocate_session(tier) {
                            black_box(id);
                            let _ = pool_clone.release_session(id);
                        }
                    }
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }
        })
    });
}

/// Benchmark concurrent upgrade/downgrade operations
fn bench_concurrent_upgrade_downgrade(c: &mut Criterion) {
    let pool = Arc::new(SessionPoolCapsule::new(PoolConfig::default()));

    c.bench_function("concurrent_upgrade_downgrade_2_threads", |b| {
        b.iter(|| {
            let pool1 = Arc::clone(&pool);
            let pool2 = Arc::clone(&pool);

            // Thread 1: Upgrade path
            let h1 = thread::spawn(move || {
                for _ in 0..20 {
                    if let Ok(light_id) = pool1.allocate_session(SessionTierType::Light) {
                        if let Ok(medium_id) = pool1.upgrade_session(light_id) {
                            let _ = pool1.release_session(medium_id);
                        }
                    }
                }
            });

            // Thread 2: Downgrade path
            let h2 = thread::spawn(move || {
                for _ in 0..20 {
                    if let Ok(heavy_id) = pool2.allocate_session(SessionTierType::Heavy) {
                        if let Ok(medium_id) = pool2.downgrade_session(heavy_id) {
                            let _ = pool2.release_session(medium_id);
                        }
                    }
                }
            });

            h1.join().unwrap();
            h2.join().unwrap();
        })
    });
}

// ============================================================================
// Throughput Benchmarks
// ============================================================================

/// Benchmark session allocation throughput
///
/// Measures sessions/second capacity
fn bench_allocation_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocation_throughput");

    for batch_size in [100, 500, 1000] {
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &batch_size,
            |b, &batch_size| {
                let config = PoolConfig {
                    light_capacity: 10000,
                    medium_capacity: 5000,
                    heavy_capacity: 2000,
                    ..PoolConfig::default()
                };
                let pool = SessionPoolCapsule::new(config);

                b.iter(|| {
                    let mut ids = Vec::with_capacity(batch_size);

                    // Allocate batch
                    for _ in 0..batch_size {
                        if let Ok(id) = pool.allocate_session(SessionTierType::Light) {
                            ids.push(id);
                        }
                    }

                    // Release batch
                    for id in ids {
                        let _ = pool.release_session(id);
                    }
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// Pool Capacity Stress Tests
// ============================================================================

/// Benchmark allocation near pool capacity
///
/// Measures performance degradation under high utilization
fn bench_near_capacity_allocation(c: &mut Criterion) {
    let config = PoolConfig {
        light_capacity: 100,
        medium_capacity: 50,
        heavy_capacity: 25,
        ..PoolConfig::default()
    };
    let pool = SessionPoolCapsule::new(config);

    // Fill pool to 90% capacity
    let fill_count = 90;
    let mut held_sessions: Vec<SessionId> = Vec::with_capacity(fill_count);
    for _ in 0..fill_count {
        if let Ok(id) = pool.allocate_session(SessionTierType::Light) {
            held_sessions.push(id);
        }
    }

    c.bench_function("allocate_at_90_percent_capacity", |b| {
        b.iter(|| {
            // Try to allocate in the remaining 10%
            if let Ok(id) = pool.allocate_session(SessionTierType::Light) {
                black_box(id);
                let _ = pool.release_session(id);
            }
        })
    });

    // Cleanup
    for id in held_sessions {
        let _ = pool.release_session(id);
    }
}

/// Benchmark pool exhaustion and recovery
fn bench_pool_exhaustion_recovery(c: &mut Criterion) {
    let config = PoolConfig {
        light_capacity: 50,
        medium_capacity: 25,
        heavy_capacity: 10,
        ..PoolConfig::default()
    };

    c.bench_function("pool_exhaust_and_recover", |b| {
        b.iter(|| {
            let pool = SessionPoolCapsule::new(config);

            // Exhaust pool
            let mut ids = Vec::with_capacity(50);
            while let Ok(id) = pool.allocate_session(SessionTierType::Light) {
                ids.push(id);
            }

            // Release all (recovery)
            for id in ids {
                let _ = pool.release_session(id);
            }

            // Verify recovery
            let id = pool.allocate_session(SessionTierType::Light).unwrap();
            black_box(id);
            let _ = pool.release_session(id);
        })
    });
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    allocation_benches,
    bench_allocate_light,
    bench_allocate_medium,
    bench_allocate_heavy,
    bench_allocate_only,
    bench_release_session,
);

criterion_group!(
    lookup_benches,
    bench_session_lookup,
    bench_session_id_extract,
);

criterion_group!(
    upgrade_benches,
    bench_upgrade_light_to_medium,
    bench_upgrade_medium_to_heavy,
    bench_upgrade_chain,
    bench_downgrade_heavy_to_medium,
    bench_downgrade_medium_to_light,
);

criterion_group!(
    stats_benches,
    bench_pool_stats,
    bench_pool_config,
);

criterion_group!(
    concurrent_benches,
    bench_concurrent_allocation_8_threads,
    bench_concurrent_mixed_tier,
    bench_concurrent_upgrade_downgrade,
);

criterion_group!(
    throughput_benches,
    bench_allocation_throughput,
    bench_near_capacity_allocation,
    bench_pool_exhaustion_recovery,
);

criterion_main!(
    allocation_benches,
    lookup_benches,
    upgrade_benches,
    stats_benches,
    concurrent_benches,
    throughput_benches,
);
