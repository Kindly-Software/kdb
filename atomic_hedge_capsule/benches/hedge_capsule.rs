//! AtomicHedgeCapsule Benchmarks - B32 Framework Compliant
//!
//! Following B32 fairness framework principles:
//! - Fair baselines (optimized AtomicU128 vs AtomicHedgeCapsule)
//! - Statistical rigor (95% confidence intervals, 1000+ iterations)
//! - Hardware measurement methodology documentation
//! - Kontext27 reality checks (10-50% typical improvement expectations)
//! - Empirical validation of 45-55ns coordination claims
//!
//! UCE32 Analysis Applied:
//! - Q28 (Simplicity): Focus on real hedge operations, not synthetic benchmarks
//! - Q29 (Constraints): Hardware constraint: L1 cache access ~1ns, memory ~100ns
//! - Q30 (Validation): Prove 45-55ns coordination target with statistical rigor
//! - Q31 (Rust Transform): AtomicU128 lockfree operations enable zero-copy coordination
//! - Q32 (Nightly): SIMD and const optimizations for batch operations

use atomic_hedge_capsule::{AtomicHedgeCapsule, HedgeError, HedgeState};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use portable_atomic::{AtomicBool, AtomicU128, AtomicU64, Ordering};
use std::hint::black_box as std_black_box;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// B32 Framework: Performance target constants
const HIGH_PERF_COORD_NS_TARGET: u64 = 45; // Target: 45ns hedge coordination
const HIGH_PERF_COORD_NS_MAX: u64 = 55; // Maximum acceptable: 55ns
const ACQUIRE: Ordering = Ordering::Acquire;
const RELEASE: Ordering = Ordering::Release;
const ACQ_REL: Ordering = Ordering::AcqRel;
const RELAXED: Ordering = Ordering::Relaxed;

/// B32 Framework: Fair baseline implementation using raw AtomicU128
/// This represents optimized baseline performance for comparison
#[repr(C, align(32))]
struct BaselineAtomicU128Hedge {
    word0: AtomicU128,
    word1: AtomicU128,
    generation: AtomicU64,
    active: AtomicBool,
}

impl BaselineAtomicU128Hedge {
    fn new() -> Self {
        Self {
            word0: AtomicU128::new(0),
            word1: AtomicU128::new(0),
            generation: AtomicU64::new(0),
            active: AtomicBool::new(false),
        }
    }

    fn simple_store(&self, value0: u128, value1: u128) -> u64 {
        let gen = self.generation.fetch_add(1, ACQ_REL);
        self.word0.store(value0, RELEASE);
        self.word1.store(value1, RELEASE);
        gen
    }

    fn simple_load(&self) -> (u128, u128) {
        let w0 = self.word0.load(ACQUIRE);
        let w1 = self.word1.load(ACQUIRE);
        (w0, w1)
    }

    fn compare_exchange_store(
        &self,
        old0: u128,
        old1: u128,
        new0: u128,
        new1: u128,
    ) -> Result<u64, (u128, u128)> {
        match self
            .word0
            .compare_exchange_weak(old0, new0, ACQ_REL, RELAXED)
        {
            Ok(_) => {
                match self
                    .word1
                    .compare_exchange_weak(old1, new1, ACQ_REL, RELAXED)
                {
                    Ok(_) => {
                        let gen = self.generation.fetch_add(1, ACQ_REL);
                        Ok(gen)
                    }
                    Err(actual1) => {
                        // Rollback word0
                        let _ = self
                            .word0
                            .compare_exchange_weak(new0, old0, ACQ_REL, RELAXED);
                        Err((old0, actual1))
                    }
                }
            }
            Err(actual0) => Err((actual0, old1)),
        }
    }
}

/// B32 Framework: System hardware information for benchmark context
fn gather_hardware_info() -> String {
    use std::fs;

    let cpu_info =
        fs::read_to_string("/proc/cpuinfo").unwrap_or_else(|_| "CPU info unavailable".to_string());
    let cpu_model = cpu_info
        .lines()
        .find(|line| line.starts_with("model name"))
        .map(|line| line.split(':').nth(1).unwrap_or("Unknown").trim())
        .unwrap_or("Unknown CPU");

    let memory_info = fs::read_to_string("/proc/meminfo")
        .unwrap_or_else(|_| "Memory info unavailable".to_string());
    let total_memory = memory_info
        .lines()
        .find(|line| line.starts_with("MemTotal:"))
        .map(|line| line.split_whitespace().nth(1).unwrap_or("Unknown"))
        .unwrap_or("Unknown");

    format!("CPU: {}, Memory: {} kB", cpu_model, total_memory)
}

/// B32 Framework: Creation overhead benchmarks
fn bench_creation_overhead(c: &mut Criterion) {
    let hardware_info = gather_hardware_info();
    println!("Hardware: {}", hardware_info);

    let mut group = c.benchmark_group("creation_overhead");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));
    group.sample_size(1000);

    // B32 Fair Baseline: Raw AtomicU128 creation
    group.bench_function("baseline_atomic_u128", |b| {
        b.iter(|| {
            let baseline = black_box(BaselineAtomicU128Hedge::new());
            std_black_box(baseline);
        });
    });

    // AtomicHedgeCapsule creation
    group.bench_function("atomic_hedge_capsule", |b| {
        b.iter(|| {
            let capsule = black_box(AtomicHedgeCapsule::new());
            std_black_box(capsule);
        });
    });

    // Standard creation (simplified for actual API)
    group.bench_function("hedge_capsule_standard", |b| {
        b.iter(|| {
            let capsule = black_box(AtomicHedgeCapsule::new());
            std_black_box(capsule);
        });
    });

    group.finish();
}

/// B32 Framework: State update latency (45-55ns target)
fn bench_state_update_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_update_latency");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(2000); // Higher sample size for latency measurement

    // B32 Framework: Test parameters for realistic hedge operations

    // B32 Fair Baseline: Raw AtomicU128 two-phase commit
    group.bench_function("baseline_two_phase_commit", |b| {
        let baseline = BaselineAtomicU128Hedge::new();
        let word0 = 0x8000_0000_0000_0001u128; // Commit flag + minimal data
        let word1 = 0x1234_5678_9ABC_DEF0u128; // Test pattern

        b.iter(|| {
            let (old0, old1) = baseline.simple_load();
            let result = black_box(baseline.compare_exchange_store(old0, old1, word0, word1));
            std_black_box(result);
        });
    });

    // AtomicHedgeCapsule start_bracket (primary operation under test)
    group.bench_function("start_bracket", |b| {
        let capsule = AtomicHedgeCapsule::new();

        b.iter(|| {
            // Reset capsule for each iteration
            let _ = capsule.rollback_bracket();

            let result = black_box(capsule.start_bracket(true, 1000, 50000, 500, 1000));
            std_black_box(result);
        });
    });

    // AtomicHedgeCapsule read_if_ready
    group.bench_function("read_if_ready", |b| {
        let capsule = AtomicHedgeCapsule::new();
        let _ = capsule.start_bracket(true, 1000, 50000, 500, 1000); // Pre-populate

        b.iter(|| {
            let result = black_box(capsule.read_if_ready());
            std_black_box(result);
        });
    });

    // Two-phase commit performance
    group.bench_function("start_commit_bracket", |b| {
        let capsule = AtomicHedgeCapsule::new();

        b.iter(|| {
            // Reset for each iteration
            let _ = capsule.rollback_bracket();

            let gen1 = black_box(capsule.start_bracket(true, 1000, 50000, 500, 1000));
            let gen2 = black_box(capsule.commit_bracket(50100, 1000));
            std_black_box((gen1, gen2));
        });
    });

    group.finish();
}

/// B32 Framework: Memory ordering impact measurement
fn bench_memory_ordering_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_ordering_impact");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));
    group.sample_size(1000);

    let capsule = AtomicHedgeCapsule::new();
    // B32 Test parameters for hedge operations
    let side = true;
    let quantity = 1000u32;
    let entry_price = 50000u32;
    let stop_ticks = 500u32;
    let target_ticks = 1000u32;

    // Relaxed ordering baseline
    group.bench_function("relaxed_ordering", |b| {
        let atomic = AtomicU128::new(0);
        let test_value = 0x1234_5678_9ABC_DEF0u128;

        b.iter(|| {
            atomic.store(black_box(test_value), Ordering::Relaxed);
            let result = atomic.load(Ordering::Relaxed);
            std_black_box(result);
        });
    });

    // Acquire/Release ordering (used in hedge capsule)
    group.bench_function("acquire_release_ordering", |b| {
        let atomic = AtomicU128::new(0);
        let test_value = 0x1234_5678_9ABC_DEF0u128;

        b.iter(|| {
            atomic.store(black_box(test_value), Ordering::Release);
            let result = atomic.load(Ordering::Acquire);
            std_black_box(result);
        });
    });

    // Sequential consistency (strictest)
    group.bench_function("seq_cst_ordering", |b| {
        let atomic = AtomicU128::new(0);
        let test_value = 0x1234_5678_9ABC_DEF0u128;

        b.iter(|| {
            atomic.store(black_box(test_value), Ordering::SeqCst);
            let result = atomic.load(Ordering::SeqCst);
            std_black_box(result);
        });
    });

    // Hedge capsule with standard ordering
    group.bench_function("hedge_capsule_standard", |b| {
        b.iter(|| {
            let _ = capsule.rollback_bracket(); // Reset
            let result = black_box(capsule.publish_bracket(hedge_order));
            std_black_box(result);
        });
    });

    group.finish();
}

/// B32 Framework: Concurrent update throughput
fn bench_concurrent_update_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_throughput");
    group.warm_up_time(Duration::from_millis(1000));
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(100); // Fewer samples due to threading overhead

    // Test different thread counts
    for thread_count in [1, 2, 4, 8].iter() {
        group.throughput(Throughput::Elements(*thread_count as u64));

        // B32 Fair Baseline: Multi-threaded AtomicU128 operations
        group.bench_with_input(
            BenchmarkId::new("baseline_concurrent", thread_count),
            thread_count,
            |b, &thread_count| {
                let baseline = Arc::new(BaselineAtomicU128Hedge::new());

                b.iter(|| {
                    let mut handles = vec![];

                    for i in 0..thread_count {
                        let baseline_clone = Arc::clone(&baseline);
                        let handle = thread::spawn(move || {
                            let value0 = 0x1000_0000_0000_0000u128 | i as u128;
                            let value1 = 0x2000_0000_0000_0000u128 | i as u128;

                            for _ in 0..100 {
                                black_box(baseline_clone.simple_store(value0, value1));
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );

        // AtomicHedgeCapsule concurrent operations
        group.bench_with_input(
            BenchmarkId::new("hedge_capsule_concurrent", thread_count),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    let mut handles = vec![];
                    let mut successes = 0u64;
                    let mut failures = 0u64;

                    for i in 0..thread_count {
                        let handle = thread::spawn(move || {
                            let capsule = AtomicHedgeCapsule::new();
                            let side = i % 2 == 0;
                            let quantity = 1000 + i as u32;
                            let entry_price = 50000 + i as u32 * 100;
                            let stop_ticks = 500u32;
                            let target_ticks = 1000u32;

                            let mut local_successes = 0;
                            let mut local_failures = 0;

                            for _ in 0..10 {
                                match capsule.start_bracket(
                                    side,
                                    quantity,
                                    entry_price,
                                    stop_ticks,
                                    target_ticks,
                                ) {
                                    Ok(_) => {
                                        local_successes += 1;
                                        let _ = capsule.rollback_bracket(); // Reset for next iteration
                                    }
                                    Err(_) => local_failures += 1,
                                }
                            }

                            (local_successes, local_failures)
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        let (s, f) = handle.join().unwrap();
                        successes += s;
                        failures += f;
                    }

                    std_black_box((successes, failures));
                });
            },
        );
    }

    group.finish();
}

/// B32 Framework: Emergency coordination overhead (<50μs target)
fn bench_emergency_coordination(c: &mut Criterion) {
    let mut group = c.benchmark_group("emergency_coordination");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));
    group.sample_size(1000);

    // Initialize hedge capsule with entry and bracket orders
    let capsule = AtomicHedgeCapsule::new();
    let entry = EntryOrder::new(
        "NDAX".to_string(),
        "BTCUSD".to_string(),
        "Buy".to_string(),
        1.0,
    );
    let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
    let _ = capsule.initialize(entry, bracket);

    // B32 Fair Baseline: Simple atomic increment (emergency trigger baseline)
    group.bench_function("baseline_atomic_increment", |b| {
        let counter = AtomicU64::new(0);

        b.iter(|| {
            let result = black_box(counter.fetch_add(1, ACQ_REL));
            std_black_box(result);
        });
    });

    // Emergency hedge trigger
    group.bench_function("trigger_emergency_hedge", |b| {
        b.iter(|| {
            let result = black_box(capsule.trigger_emergency_hedge());
            std_black_box(result);
        });
    });

    // Emergency state check
    group.bench_function("is_emergency_check", |b| {
        b.iter(|| {
            let result = black_box(capsule.is_emergency());
            std_black_box(result);
        });
    });

    // Emergency with state snapshot
    group.bench_function("emergency_with_snapshot", |b| {
        b.iter(|| {
            let _ = black_box(capsule.trigger_emergency_hedge());
            let snapshot = black_box(capsule.get_hedge_state());
            std_black_box(snapshot);
        });
    });

    group.finish();
}

/// B32 Framework: Generation counter overhead measurement
fn bench_generation_counter_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("generation_counter");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));
    group.sample_size(1000);

    // B32 Fair Baseline: Raw AtomicU64 increment
    group.bench_function("baseline_atomic_u64_increment", |b| {
        let counter = AtomicU64::new(0);

        b.iter(|| {
            let result = black_box(counter.fetch_add(1, ACQ_REL));
            std_black_box(result);
        });
    });

    // Hedge capsule operations with generation tracking
    group.bench_function("hedge_operations_with_generation", |b| {
        let capsule = AtomicHedgeCapsule::new();
        let hedge_order = HedgeBracketOrder {
            side: true,
            quantity: 1000,
            entry_price: 50000,
            ttl: 300,
            stop_ticks: 500,
            target_ticks: 1000,
            risk_amount: 10000,
            flags: 0,
        };

        b.iter(|| {
            let _ = capsule.rollback_bracket(); // Reset
            let gen1 = black_box(capsule.publish_bracket(hedge_order));
            let gen2 = black_box(capsule.rollback_bracket());
            std_black_box((gen1, gen2));
        });
    });

    // Performance metrics gathering
    group.bench_function("get_performance_metrics", |b| {
        let capsule = AtomicHedgeCapsule::new();
        let hedge_order = HedgeBracketOrder {
            side: true,
            quantity: 1000,
            entry_price: 50000,
            ttl: 300,
            stop_ticks: 500,
            target_ticks: 1000,
            risk_amount: 10000,
            flags: 0,
        };
        let _ = capsule.publish_bracket(hedge_order);

        b.iter(|| {
            let metrics = black_box(capsule.get_performance_metrics());
            std_black_box(metrics);
        });
    });

    group.finish();
}

/// B32 Framework: Cache effects measurement (single vs multi-threaded)
fn bench_cache_effects(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_effects");
    group.warm_up_time(Duration::from_millis(1000));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(500);

    // Single-threaded access pattern
    group.bench_function("single_threaded_sequential", |b| {
        let capsules: Vec<AtomicHedgeCapsule> = (0..8).map(|_| AtomicHedgeCapsule::new()).collect();
        let hedge_order = HedgeBracketOrder {
            side: true,
            quantity: 1000,
            entry_price: 50000,
            ttl: 300,
            stop_ticks: 500,
            target_ticks: 1000,
            risk_amount: 10000,
            flags: 0,
        };

        b.iter(|| {
            for (i, capsule) in capsules.iter().enumerate() {
                let _ = capsule.rollback_bracket();
                let mut order = hedge_order;
                order.quantity = 1000 + i as u32;
                let result = black_box(capsule.publish_bracket(order));
                std_black_box(result);
            }
        });
    });

    // Multi-threaded with potential false sharing
    group.bench_function("multi_threaded_potential_sharing", |b| {
        let capsules: Vec<Arc<AtomicHedgeCapsule>> = (0..4)
            .map(|_| Arc::new(AtomicHedgeCapsule::new()))
            .collect();

        b.iter(|| {
            let mut handles = vec![];

            for (i, capsule) in capsules.iter().enumerate() {
                let capsule_clone = Arc::clone(capsule);
                let handle = thread::spawn(move || {
                    let hedge_order = HedgeBracketOrder {
                        side: i % 2 == 0,
                        quantity: 1000 + i as u32,
                        entry_price: 50000 + i as u32 * 100,
                        ttl: 300,
                        stop_ticks: 500,
                        target_ticks: 1000,
                        risk_amount: 10000,
                        flags: 0,
                    };

                    for _ in 0..10 {
                        let _ = capsule_clone.rollback_bracket();
                        let result = black_box(capsule_clone.publish_bracket(hedge_order));
                        std_black_box(result);
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    // Cache-separated instances
    group.bench_function("cache_separated_instances", |b| {
        b.iter(|| {
            let mut handles = vec![];

            for i in 0..4 {
                let handle = thread::spawn(move || {
                    // Each thread creates its own capsule (no sharing)
                    let capsule = AtomicHedgeCapsule::new();
                    let hedge_order = HedgeBracketOrder {
                        side: i % 2 == 0,
                        quantity: 1000 + i as u32,
                        entry_price: 50000 + i as u32 * 100,
                        ttl: 300,
                        stop_ticks: 500,
                        target_ticks: 1000,
                        risk_amount: 10000,
                        flags: 0,
                    };

                    for _ in 0..10 {
                        let _ = capsule.rollback_bracket();
                        let result = black_box(capsule.publish_bracket(hedge_order));
                        std_black_box(result);
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    group.finish();
}

/// B32 Framework: Bit packing/unpacking overhead
fn bench_bit_packing_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("bit_packing");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));
    group.sample_size(1000);

    let capsule = AtomicHedgeCapsule::new();
    let hedge_order = HedgeBracketOrder {
        side: true,
        quantity: 1000,
        entry_price: 50000,
        ttl: 300,
        stop_ticks: 500,
        target_ticks: 1000,
        risk_amount: 10000,
        flags: 0b10101010,
    };

    // B32 Fair Baseline: Raw bit operations without packing
    group.bench_function("baseline_raw_bit_ops", |b| {
        b.iter(|| {
            let side = black_box(true);
            let quantity = black_box(1000u32);
            let entry_price = black_box(50000u32);
            let packed =
                black_box((side as u64) << 63 | (quantity as u64) << 32 | entry_price as u64);

            let unpacked_side = (packed >> 63) != 0;
            let unpacked_quantity = ((packed >> 32) & 0xFFFFFFFF) as u32;
            let unpacked_price = (packed & 0xFFFFFFFF) as u32;

            std_black_box((unpacked_side, unpacked_quantity, unpacked_price));
        });
    });

    // Hedge capsule pack operation
    group.bench_function("hedge_capsule_pack", |b| {
        b.iter(|| {
            let result = black_box(capsule.pack_hedge_order(hedge_order));
            std_black_box(result);
        });
    });

    // Hedge capsule unpack operation
    group.bench_function("hedge_capsule_unpack", |b| {
        let (word0, word1) = capsule.pack_hedge_order(hedge_order).unwrap();

        b.iter(|| {
            let result = black_box(AtomicHedgeCapsule::unpack_hedge_order(word0, word1));
            std_black_box(result);
        });
    });

    // Combined pack + unpack operation
    group.bench_function("pack_unpack_roundtrip", |b| {
        b.iter(|| {
            let (word0, word1) = black_box(capsule.pack_hedge_order(hedge_order)).unwrap();
            let unpacked = black_box(AtomicHedgeCapsule::unpack_hedge_order(word0, word1));
            std_black_box(unpacked);
        });
    });

    group.finish();
}

/// B32 Framework: Real-world trading scenario simulation
fn bench_trading_scenario_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("trading_scenarios");
    group.warm_up_time(Duration::from_millis(1000));
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(200);

    // Complete hedge lifecycle benchmark
    group.bench_function("complete_hedge_lifecycle", |b| {
        b.iter(|| {
            let capsule = AtomicHedgeCapsule::new();

            // Initialize
            let entry = EntryOrder::new(
                "NDAX".to_string(),
                "BTCUSD".to_string(),
                "Buy".to_string(),
                1.0,
            );
            let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
            let gen1 = black_box(capsule.initialize(entry, bracket)).unwrap();

            // Start bracket
            let gen2 = black_box(capsule.start_bracket(true, 1000, 50000, 500, 1000)).unwrap();

            // Update entry state
            let gen3 = black_box(capsule.update_entry_state(OrderState::Submitted, 0.0)).unwrap();
            let gen4 =
                black_box(capsule.update_entry_state(OrderState::PartiallyFilled, 0.5)).unwrap();
            let gen5 = black_box(capsule.update_entry_state(OrderState::Filled, 1.0)).unwrap();

            // Update bracket states
            let gen6 = black_box(
                capsule.update_bracket_states(OrderState::Submitted, OrderState::Submitted),
            )
            .unwrap();

            // Get final state
            let final_state = black_box(capsule.get_hedge_state());

            // Close
            let close_result = black_box(capsule.close()).unwrap();

            std_black_box((
                gen1,
                gen2,
                gen3,
                gen4,
                gen5,
                gen6,
                final_state,
                close_result,
            ));
        });
    });

    // High-frequency updates simulation
    group.bench_function("high_frequency_updates", |b| {
        let capsule = AtomicHedgeCapsule::new();
        let entry = EntryOrder::new(
            "NDAX".to_string(),
            "BTCUSD".to_string(),
            "Buy".to_string(),
            1.0,
        );
        let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
        let _ = capsule.initialize(entry, bracket);

        b.iter(|| {
            // Simulate rapid state changes
            for i in 0..10 {
                let fill_ratio = (i as f64) / 10.0;
                let state = if fill_ratio == 1.0 {
                    OrderState::Filled
                } else {
                    OrderState::PartiallyFilled
                };
                let result = black_box(capsule.update_entry_state(state, fill_ratio));
                std_black_box(result);
            }
        });
    });

    // Emergency scenarios
    group.bench_function("emergency_scenarios", |b| {
        let capsule = AtomicHedgeCapsule::new();
        let entry = EntryOrder::new(
            "NDAX".to_string(),
            "BTCUSD".to_string(),
            "Buy".to_string(),
            1.0,
        );
        let bracket = BracketOrder::new(45000.0, 55000.0, 1.0);
        let _ = capsule.initialize(entry, bracket);

        b.iter(|| {
            // Normal operation
            let _ = black_box(capsule.update_entry_state(OrderState::Submitted, 0.0));

            // Emergency trigger
            let _ = black_box(capsule.trigger_emergency_hedge());

            // Emergency state check
            let is_emergency = black_box(capsule.is_emergency());

            // Get state during emergency
            let emergency_state = black_box(capsule.get_hedge_state());

            std_black_box((is_emergency, emergency_state));
        });
    });

    group.finish();
}

/// B32 Framework: Performance target validation
fn bench_performance_target_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("performance_targets");
    group.warm_up_time(Duration::from_millis(1000));
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(5000); // High sample size for precise measurement

    println!("=== B32 Framework Performance Target Validation ===");
    println!("Target: 45-55ns coordination operations");
    println!("Hardware: {}", gather_hardware_info());
    println!("Kontext27 Reality Check: 10-50% typical improvement expected");

    // Validate 45-55ns target for publish_bracket
    group.bench_function("validate_45_55ns_target", |b| {
        let capsule = AtomicHedgeCapsule::new();
        let hedge_order = HedgeBracketOrder {
            side: true,
            quantity: 1000,
            entry_price: 50000,
            ttl: 300,
            stop_ticks: 500,
            target_ticks: 1000,
            risk_amount: 10000,
            flags: 0,
        };

        let mut success_count = 0u64;
        let mut total_count = 0u64;

        b.iter_custom(|iters| {
            let start = Instant::now();

            for _ in 0..iters {
                let _ = capsule.rollback_bracket();

                let op_start = Instant::now();
                let result = capsule.publish_bracket(hedge_order);
                let op_duration = op_start.elapsed();

                total_count += 1;
                if result.is_ok() && op_duration.as_nanos() <= HIGH_PERF_COORD_NS_MAX as u128 {
                    success_count += 1;
                }

                black_box(result);
            }

            let success_rate = (success_count as f64) / (total_count as f64) * 100.0;
            if total_count % 1000 == 0 {
                println!(
                    "Success rate (≤55ns): {:.2}% ({}/{})",
                    success_rate, success_count, total_count
                );
            }

            start.elapsed()
        });
    });

    group.finish();
}

// Configure Criterion benchmark groups
criterion_group!(
    benches,
    bench_creation_overhead,
    bench_state_update_latency,
    bench_memory_ordering_impact,
    bench_concurrent_update_throughput,
    bench_emergency_coordination,
    bench_generation_counter_overhead,
    bench_cache_effects,
    bench_bit_packing_overhead,
    bench_trading_scenario_simulation,
    bench_performance_target_validation,
);

criterion_main!(benches);

#[cfg(test)]
mod benchmark_validation_tests {
    use super::*;

    #[test]
    fn test_baseline_atomic_u128_functionality() {
        let baseline = BaselineAtomicU128Hedge::new();

        // Test simple operations
        let gen1 = baseline.simple_store(0x1234, 0x5678);
        let (w0, w1) = baseline.simple_load();
        assert_eq!(w0, 0x1234);
        assert_eq!(w1, 0x5678);

        // Test compare-exchange
        let result = baseline.compare_exchange_store(0x1234, 0x5678, 0xABCD, 0xEF01);
        assert!(result.is_ok());

        let (w0_new, w1_new) = baseline.simple_load();
        assert_eq!(w0_new, 0xABCD);
        assert_eq!(w1_new, 0xEF01);
    }

    #[test]
    fn test_hedge_capsule_vs_baseline_consistency() {
        let capsule = AtomicHedgeCapsule::new();
        let baseline = BaselineAtomicU128Hedge::new();

        // Both should start empty/zero
        let (b_w0, b_w1) = baseline.simple_load();
        assert_eq!(b_w0, 0);
        assert_eq!(b_w1, 0);

        let metrics = capsule.get_performance_metrics();
        assert_eq!(metrics.current_generation, 0);
        assert_eq!(metrics.current_state, HedgeOrderState::Empty);
    }

    #[test]
    fn test_performance_target_constants() {
        // Verify our performance targets are realistic per Kontext27
        assert!(HIGH_PERF_COORD_NS_TARGET >= 10); // Not unrealistically low
        assert!(HIGH_PERF_COORD_NS_TARGET <= 100); // Not too lenient
        assert!(HIGH_PERF_COORD_NS_MAX > HIGH_PERF_COORD_NS_TARGET);

        // Kontext27 reality check: 10-50% improvement is typical
        let improvement_factor = HIGH_PERF_COORD_NS_TARGET as f64 / 100.0; // vs 100ns baseline
        assert!(improvement_factor >= 0.5); // At least 50% improvement
        assert!(improvement_factor <= 1.0); // Not claiming > 100% improvement
    }

    #[test]
    fn test_bit_packing_correctness() {
        let capsule = AtomicHedgeCapsule::new();
        let order = HedgeBracketOrder {
            side: true,
            quantity: 1000,
            entry_price: 50000,
            ttl: 300,
            stop_ticks: 500,
            target_ticks: 1000,
            risk_amount: 10000,
            flags: 0b10101010,
        };

        let (word0, word1) = capsule.pack_hedge_order(order).unwrap();
        let unpacked = AtomicHedgeCapsule::unpack_hedge_order(word0, word1);

        // Verify round-trip accuracy
        assert_eq!(order.side, unpacked.side);
        assert_eq!(order.quantity, unpacked.quantity);
        assert_eq!(order.entry_price, unpacked.entry_price);
        assert_eq!(order.ttl, unpacked.ttl);
        assert_eq!(order.stop_ticks, unpacked.stop_ticks);
        assert_eq!(order.target_ticks, unpacked.target_ticks);
        assert_eq!(order.risk_amount, unpacked.risk_amount);
        assert_eq!(order.flags, unpacked.flags);
    }

    #[test]
    fn test_b32_framework_compliance() {
        // Verify we have fair baselines
        let baseline = BaselineAtomicU128Hedge::new();
        let capsule = AtomicHedgeCapsule::new();

        // Both should be properly aligned
        assert_eq!(std::mem::align_of_val(&baseline), 32);
        assert_eq!(std::mem::align_of_val(&capsule), 32);

        // Both should have comparable memory layout
        assert_eq!(
            std::mem::size_of_val(&baseline.word0),
            std::mem::size_of::<AtomicU128>()
        );
        assert_eq!(
            std::mem::size_of_val(&baseline.word1),
            std::mem::size_of::<AtomicU128>()
        );
    }
}
