//! # Hardware Prefetching Load Factor Benchmark
//!
//! **Purpose**: Measure the impact of CPU prefetching (`_mm_prefetch`) on ConcurrentMapCapsule
//! performance across different load factors (25%, 50%, 75%, 90%).
//!
//! **Framework**: B32 Honest Benchmarking
//!
//! ## Expected Results (x86_64 with nightly prefetch enabled)
//!
//! | Load Factor | Avg Probe Dist | No Prefetch (ns) | Prefetch (ns) | Speedup | Verdict |
//! |-------------|----------------|------------------|---------------|---------|---------|
//! | 25% | 1-2 hops | 15ns | 15ns | 0% | No benefit (short probes) |
//! | 50% | 3-4 hops | 25ns | 24ns | 4% | Marginal benefit |
//! | 75% | 6-8 hops | 35ns | 31ns | **11%** | Target case! |
//! | 90% | 10-15 hops | 50ns | 47ns | 6% | High collision overhead |
//!
//! **Key Insight**: Prefetching most effective at **75% load factor** (11% speedup expected).
//!
//! ## Usage
//!
//! ```bash
//! # Run with nightly (prefetch enabled)
//! cargo +nightly bench --bench prefetch_load_factor_bench --features nightly
//!
//! # Run with stable (no prefetch, baseline)
//! cargo bench --bench prefetch_load_factor_bench
//!
//! # Compare before/after
//! cargo +nightly bench --bench prefetch_load_factor_bench --features nightly -- --save-baseline prefetch
//! cargo bench --bench prefetch_load_factor_bench -- --baseline prefetch
//! ```

use atomic_capsule::collections::ConcurrentMapCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::sync::Arc;
use std::thread;

// ============================================================================
// BENCHMARK CONFIGURATION
// ============================================================================

/// Default capacity (16K slots)
const CAPACITY: usize = 16384;

/// Load factors to test (percentage of capacity filled)
const LOAD_FACTORS: &[usize] = &[25, 50, 75, 90];

/// Operations per benchmark iteration
const OPERATIONS_PER_ITER: u64 = 1000;

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Fill map to specific load factor
fn fill_map_to_load_factor(
    map: &ConcurrentMapCapsule<u64, u64>,
    load_factor_percent: usize,
) -> usize {
    let target_entries = (CAPACITY * load_factor_percent) / 100;

    for i in 0..target_entries as u64 {
        // Use predictable keys for reproducibility
        let key = i * 7 + 13; // Prime stride to distribute keys
        map.insert(key, i);
    }

    target_entries
}

/// Generate probe keys that will exercise the hash table
///
/// Returns keys not already in map (to exercise linear probing)
fn generate_probe_keys(count: usize, offset: u64) -> Vec<u64> {
    (0..count)
        .map(|i| {
            // Use different seed to avoid collisions with filled keys
            let key = (i as u64 + offset) * 11 + 17;
            key
        })
        .collect()
}

// ============================================================================
// BENCHMARK: GET OPERATION AT VARYING LOAD FACTORS
// ============================================================================

/// Benchmark: Get operation performance vs load factor
///
/// **Hypothesis**: Prefetch improves performance at high load factors (75%+)
/// where long probe sequences occur.
fn bench_get_by_load_factor(c: &mut Criterion) {
    let mut group = c.benchmark_group("prefetch_get_by_load_factor");

    for &load_factor in LOAD_FACTORS {
        // Set throughput for this benchmark
        group.throughput(Throughput::Elements(OPERATIONS_PER_ITER));

        group.bench_with_input(
            BenchmarkId::new("get", format!("{}%", load_factor)),
            &load_factor,
            |b, &load_factor| {
                // Setup: Fill map to target load factor
                let map = ConcurrentMapCapsule::new();
                let entries_filled = fill_map_to_load_factor(&map, load_factor);

                // Generate keys for probing (mix of hits and misses)
                let probe_keys: Vec<u64> = (0..OPERATIONS_PER_ITER)
                    .map(|i| {
                        if i % 2 == 0 {
                            // Hit: existing key
                            (i / 2) * 7 + 13
                        } else {
                            // Miss: non-existent key (exercises linear probing)
                            (entries_filled as u64 + i) * 11 + 17
                        }
                    })
                    .collect();

                b.iter(|| {
                    for &key in &probe_keys {
                        let _ = black_box(map.get(key));
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK: INSERT OPERATION AT VARYING LOAD FACTORS
// ============================================================================

/// Benchmark: Insert operation performance vs load factor
///
/// **Hypothesis**: Prefetch improves insert probe latency at 75% load
fn bench_insert_by_load_factor(c: &mut Criterion) {
    let mut group = c.benchmark_group("prefetch_insert_by_load_factor");

    for &load_factor in LOAD_FACTORS {
        group.throughput(Throughput::Elements(OPERATIONS_PER_ITER));

        group.bench_with_input(
            BenchmarkId::new("insert", format!("{}%", load_factor)),
            &load_factor,
            |b, &load_factor| {
                b.iter_batched(
                    || {
                        // Setup: Create fresh map for each iteration
                        let map = ConcurrentMapCapsule::new();
                        fill_map_to_load_factor(&map, load_factor);

                        // Generate unique keys for insertion
                        let insert_keys = generate_probe_keys(OPERATIONS_PER_ITER as usize, 1000);

                        (map, insert_keys)
                    },
                    |(map, insert_keys)| {
                        // Benchmark: Insert operations
                        for (i, &key) in insert_keys.iter().enumerate() {
                            map.insert(key, black_box(i as u64));
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARK: PROBE DISTANCE MEASUREMENT
// ============================================================================

/// Benchmark: Average probe distance at varying load factors
///
/// **Purpose**: Validate that load factor increases probe distance (justifies prefetch)
fn bench_probe_distance_by_load_factor(c: &mut Criterion) {
    let mut group = c.benchmark_group("prefetch_probe_distance");

    // Print probe distance analysis
    println!("\n========================================");
    println!("  Probe Distance Analysis");
    println!("========================================\n");
    println!("Load | Entries | Avg Probe | Max Probe | p50 | p95 | p99");
    println!("-----|---------|-----------|-----------|-----|-----|-----");

    for &load_factor in LOAD_FACTORS {
        let map = ConcurrentMapCapsule::new();
        let entries_filled = fill_map_to_load_factor(&map, load_factor);

        // Measure probe distances via benchmark timing
        group.bench_with_input(
            BenchmarkId::new("probe", format!("{}%", load_factor)),
            &load_factor,
            |b, &_load_factor| {
                let probe_keys = generate_probe_keys(100, 2000);

                b.iter(|| {
                    for &key in &probe_keys {
                        let _ = black_box(map.get(key));
                    }
                });
            },
        );

        // Note: Actual probe distance calculation requires instrumentation
        // (linear probing loop counts not exposed by API)
        println!(
            "{:3}% | {:7} | ~{:2} hops | ~{:2} hops | N/A | N/A | N/A",
            load_factor,
            entries_filled,
            estimate_avg_probe_distance(load_factor),
            estimate_max_probe_distance(load_factor)
        );
    }

    println!("\nNote: Probe distances are estimates based on load factor theory.");
    println!("      Actual probe counts require instrumented map implementation.");
    println!();

    group.finish();
}

/// Estimate average probe distance based on load factor
///
/// **Formula**: E[probe_dist] ≈ 1 / (1 - load_factor)
///
/// **Source**: Knuth, The Art of Computer Programming, Vol. 3 (Sorting and Searching)
fn estimate_avg_probe_distance(load_factor_percent: usize) -> usize {
    let load_factor = load_factor_percent as f64 / 100.0;
    let avg_probe = 1.0 / (1.0 - load_factor);
    avg_probe.ceil() as usize
}

/// Estimate max probe distance (p99) based on load factor
///
/// **Heuristic**: max ≈ avg × 2.5 for linear probing
fn estimate_max_probe_distance(load_factor_percent: usize) -> usize {
    let avg = estimate_avg_probe_distance(load_factor_percent);
    (avg as f64 * 2.5).ceil() as usize
}

// ============================================================================
// BENCHMARK: CONCURRENT ACCESS WITH PREFETCH
// ============================================================================

/// Benchmark: Multi-threaded get performance with prefetch
///
/// **Hypothesis**: Prefetch reduces contention latency at high load
fn bench_concurrent_get_with_prefetch(c: &mut Criterion) {
    let mut group = c.benchmark_group("prefetch_concurrent_get");

    let thread_counts = vec![1, 2, 4, 8];

    for &threads in &thread_counts {
        for &load_factor in &[50, 75] {
            group.bench_with_input(
                BenchmarkId::new(
                    format!("{}%_load", load_factor),
                    format!("{}_threads", threads),
                ),
                &(threads, load_factor),
                |b, &(threads, load_factor)| {
                    // Setup: Fill map to target load factor
                    let map = Arc::new(ConcurrentMapCapsule::new());
                    fill_map_to_load_factor(&map, load_factor);

                    b.iter(|| {
                        thread::scope(|s| {
                            for t in 0..threads {
                                let map = Arc::clone(&map);
                                s.spawn(move || {
                                    for i in 0..OPERATIONS_PER_ITER / threads as u64 {
                                        let key = (t as u64 * 1000 + i) * 7 + 13;
                                        let _ = black_box(map.get(key));
                                    }
                                });
                            }
                        });
                    });
                },
            );
        }
    }

    group.finish();
}

// ============================================================================
// BENCHMARK: CACHE MISS PENALTY SIMULATION
// ============================================================================

/// Benchmark: Simulate cache miss penalty reduction via prefetch
///
/// **Purpose**: Demonstrate theoretical 80ns → 5ns improvement from prefetching
fn bench_cache_miss_penalty(c: &mut Criterion) {
    let mut group = c.benchmark_group("prefetch_cache_miss_penalty");

    println!("\n========================================");
    println!("  Cache Miss Penalty Analysis");
    println!("========================================\n");

    #[cfg(all(feature = "nightly", target_arch = "x86_64"))]
    {
        println!("Prefetch enabled:  YES (nightly + x86_64)");
        println!("Expected benefit:  5-10% at 75% load factor");
        println!("Cache miss penalty: 80ns → ~5ns (prefetch hides latency)");
    }

    #[cfg(not(all(feature = "nightly", target_arch = "x86_64")))]
    {
        println!("Prefetch enabled:  NO (stable or non-x86_64)");
        println!("Expected benefit:  0% (baseline)");
        println!("Cache miss penalty: 80ns (full penalty)");
    }

    println!();

    // Benchmark at 75% load (target case for prefetch)
    group.bench_function("75%_load_1000_ops", |b| {
        let map = ConcurrentMapCapsule::new();
        fill_map_to_load_factor(&map, 75);

        let probe_keys = generate_probe_keys(1000, 3000);

        b.iter(|| {
            for &key in &probe_keys {
                let _ = black_box(map.get(key));
            }
        });
    });

    group.finish();

    println!("\nHardware prefetch analysis complete.");
    println!("Compare results with/without --features nightly to measure impact.");
    println!();
}

// ============================================================================
// MAIN BENCHMARK GROUP
// ============================================================================

criterion_group!(
    benches,
    bench_get_by_load_factor,
    bench_insert_by_load_factor,
    bench_probe_distance_by_load_factor,
    bench_concurrent_get_with_prefetch,
    bench_cache_miss_penalty,
);

criterion_main!(benches);

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill_map_to_load_factor() {
        let map = ConcurrentMapCapsule::new();
        let target = fill_map_to_load_factor(&map, 50);

        assert_eq!(target, CAPACITY / 2);
        assert_eq!(map.len(), target);
    }

    #[test]
    fn test_generate_probe_keys_unique() {
        let keys = generate_probe_keys(1000, 0);
        assert_eq!(keys.len(), 1000);

        // Verify keys are unique
        let mut seen = std::collections::HashSet::new();
        for key in keys {
            assert!(seen.insert(key), "Duplicate key generated!");
        }
    }

    #[test]
    fn test_probe_distance_estimates() {
        // Verify probe distance estimates are reasonable
        assert_eq!(estimate_avg_probe_distance(25), 2); // 1/(1-0.25) ≈ 1.33
        assert_eq!(estimate_avg_probe_distance(50), 2); // 1/(1-0.50) = 2.0
        assert_eq!(estimate_avg_probe_distance(75), 4); // 1/(1-0.75) = 4.0
        assert_eq!(estimate_avg_probe_distance(90), 10); // 1/(1-0.90) = 10.0
    }

    #[test]
    fn test_max_probe_distance_estimates() {
        // max ≈ avg × 2.5
        assert!(estimate_max_probe_distance(25) >= 3);
        assert!(estimate_max_probe_distance(75) >= 10);
        assert!(estimate_max_probe_distance(90) >= 25);
    }

    #[test]
    fn test_prefetch_feature_detection() {
        #[cfg(all(feature = "nightly", target_arch = "x86_64"))]
        {
            println!("✅ Prefetch enabled");
        }

        #[cfg(not(all(feature = "nightly", target_arch = "x86_64")))]
        {
            println!("❌ Prefetch disabled (stable or non-x86_64)");
        }
    }
}
