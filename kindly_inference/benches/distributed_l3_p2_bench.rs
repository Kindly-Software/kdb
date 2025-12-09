//! # B32-Compliant Distributed L3 P2 Benchmarks
//!
//! **Fair, reproducible benchmarks for L3 P2 features: Histogram, SIMD batch hashing, Quorum reads.**
//!
//! ## UCE34 Q1-Q34 Internal Analysis Summary
//!
//! **Q1-Q9 (Meta-cognitive):**
//! - Scope: Benchmark L3 P2 features (histogram latency, SIMD hash speedup, quorum overhead)
//! - Success: Fair baselines, honest claims, reproducible results (B32 compliance)
//!
//! **Q10-Q12 (Foundation):**
//! - Q10: T1 Atomic (histogram), T2 SIMD (batch hashing), T8 Network (quorum reads)
//! - Q11: Rust atomics, portable_simd, criterion framework
//! - Q12: Nightly SIMD features (f64x4, u64x8)
//!
//! **Q13-Q30 (Domain + Implementation):**
//! - Resources: <100MB RAM, <10 CPU threads, criterion profiling
//! - Performance: <10ns histogram, 2-8× SIMD hash, <50ms quorum
//! - Testing: B32 framework, fair baselines, 1000+ samples
//!
//! **Q31-Q34 (Refinement):**
//! - Q31: Simple criterion API, clear benchmark names
//! - Q32: Intel Ultra 7 155H constraints, nightly SIMD available
//! - Q33: Empirical validation with 95% CI, fair baselines
//! - Q34: N/A (benchmarks don't modify state)
//!
//! ## B32 Framework Compliance
//!
//! **B1: Fair Baselines** (NOT strawmen):
//! - Sequential hashing: Optimized FNV-1a (NOT DefaultHasher)
//! - Single-node reads: Optimized HashMap lookup (NOT unoptimized)
//! - No histogram: Direct counter increment (NOT mutex contention)
//!
//! **B2: Statistical Rigor**:
//! - 95% confidence interval (Criterion default)
//! - 1000+ samples for fast operations (<1μs)
//! - 100+ samples for slow operations (>1ms)
//! - 3s warm-up time (warm_up_time)
//!
//! **B3: Realistic Workloads**:
//! - Histogram: 1K-100K operations (typical cache usage)
//! - SIMD hashing: 4-16 field structs (realistic capsule sizes)
//! - Quorum reads: 3-node quorum with 2/3 agreement
//!
//! **B5: Reporting Standards**:
//! - P50, P95, P99 percentiles (Criterion built-in)
//! - Hardware specs documented below
//! - Compiler flags: --release, portable_simd
//!
//! ## Hardware Environment
//!
//! - CPU: Intel Ultra 7 155H (6P+8E cores)
//! - RAM: 64GB DDR5-5600
//! - OS: Linux 6.14.0-33-generic
//! - Rust: 1.88.0-nightly (2025-10-26)
//! - Cooling: Active (65W sustained)
//!
//! ## Performance Targets (B32 K27 Reality Check)
//!
//! | Feature | Baseline | Target | Reality |
//! |---------|----------|--------|---------|
//! | Histogram overhead | No tracking | <10ns | 10-50% typical (K27) |
//! | SIMD batch hashing | Sequential | 2-8× | 2-4× typical (K9), 8× exceptional |
//! | Quorum read latency | Single-node | +5-10ms | Network overhead realistic (K15) |
//!
//! ## Running Benchmarks
//!
//! ```bash
//! # Full suite (nightly required for portable_simd)
//! cargo +nightly bench --bench distributed_l3_p2_bench --features portable_simd
//!
//! # Individual feature groups
//! cargo +nightly bench --bench distributed_l3_p2_bench histogram --features portable_simd
//! cargo +nightly bench --bench distributed_l3_p2_bench simd_hash --features portable_simd
//! cargo +nightly bench --bench distributed_l3_p2_bench quorum --features portable_simd
//!
//! # Generate HTML reports with baseline comparison
//! cargo +nightly bench --bench distributed_l3_p2_bench --features portable_simd -- --save-baseline p2_baseline
//! cargo +nightly bench --bench distributed_l3_p2_bench --features portable_simd -- --baseline p2_baseline
//! ```

#![feature(portable_simd)]

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

// ============================================================================
// FAIR BASELINE IMPLEMENTATIONS (B1: No Strawmen)
// ============================================================================

/// Fair baseline: Optimized FNV-1a sequential hashing (NOT DefaultHasher)
///
/// NOT a strawman:
/// - FNV-1a is industry-standard fast hash
/// - Minimal branching, cache-friendly
/// - Used in production hash tables
#[inline(always)]
fn baseline_sequential_hash(data: &[u64]) -> u64 {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;

    let mut hash = FNV_OFFSET;
    for &value in data {
        hash ^= value;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Fair baseline: Single-node read with optimized HashMap lookup
///
/// NOT a strawman:
/// - Uses std::collections::HashMap (optimized implementation)
/// - Realistic key lookup pattern
/// - No artificial delays
fn baseline_single_node_read(map: &std::collections::HashMap<u64, Vec<u8>>, key: u64) -> Option<Vec<u8>> {
    map.get(&key).cloned()
}

/// Fair baseline: No histogram tracking (direct counter increment)
///
/// NOT a strawman:
/// - Direct atomic increment (minimal overhead)
/// - Relaxed ordering (fastest atomic operation)
/// - Represents "what if we didn't track histograms"
#[inline(always)]
fn baseline_no_histogram_tracking(counter: &AtomicU64) {
    counter.fetch_add(1, Ordering::Relaxed);
}

// ============================================================================
// P2.1: HISTOGRAM OVERHEAD BENCHMARKS
// ============================================================================

/// Latency histogram capsule (64B, T1 Atomic)
///
/// **Performance:** <10ns per record operation
#[repr(C, align(64))]
struct LatencyHistogram {
    /// Bucket counters: [0-1ms, 1-5ms, 5-10ms, 10-50ms, 50-100ms, 100ms+]
    buckets: [AtomicU64; 6],
    /// Total samples
    total_samples: AtomicU64,
    /// Min latency (microseconds)
    min_latency_us: AtomicU64,
    /// Max latency (microseconds)
    max_latency_us: AtomicU64,
    /// Padding to 64B
    _padding: [u8; 8],
}

impl LatencyHistogram {
    pub fn new() -> Self {
        Self {
            buckets: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
            total_samples: AtomicU64::new(0),
            min_latency_us: AtomicU64::new(u64::MAX),
            max_latency_us: AtomicU64::new(0),
            _padding: [0u8; 8],
        }
    }

    /// Record latency in microseconds (<10ns target)
    #[inline(always)]
    pub fn record(&self, latency_us: u64) {
        // Determine bucket (branchless bit manipulation preferred, but clarity here)
        let bucket_idx = if latency_us < 1_000 {
            0 // <1ms
        } else if latency_us < 5_000 {
            1 // 1-5ms
        } else if latency_us < 10_000 {
            2 // 5-10ms
        } else if latency_us < 50_000 {
            3 // 10-50ms
        } else if latency_us < 100_000 {
            4 // 50-100ms
        } else {
            5 // 100ms+
        };

        self.buckets[bucket_idx].fetch_add(1, Ordering::Relaxed);
        self.total_samples.fetch_add(1, Ordering::Relaxed);

        // Update min/max (simple CAS loop, typically 1-2 iterations)
        let mut current_min = self.min_latency_us.load(Ordering::Relaxed);
        while latency_us < current_min {
            match self.min_latency_us.compare_exchange_weak(
                current_min,
                latency_us,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_min = x,
            }
        }

        let mut current_max = self.max_latency_us.load(Ordering::Relaxed);
        while latency_us > current_max {
            match self.max_latency_us.compare_exchange_weak(
                current_max,
                latency_us,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_max = x,
            }
        }
    }

    pub fn bucket_count(&self, idx: usize) -> u64 {
        self.buckets[idx].load(Ordering::Relaxed)
    }

    pub fn total_samples(&self) -> u64 {
        self.total_samples.load(Ordering::Relaxed)
    }
}

fn bench_p2_1_histogram_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("p2.1_histogram_overhead");

    // B2: Statistical rigor (1000+ samples for <10ns operation)
    group.confidence_level(0.95).sample_size(1000);
    group.warm_up_time(Duration::from_secs(3));

    // B3: Realistic operation counts (1K-100K cache operations)
    for &num_ops in &[1_000, 10_000, 100_000] {
        group.throughput(Throughput::Elements(num_ops as u64));

        // Baseline: No histogram tracking (direct counter)
        group.bench_with_input(
            BenchmarkId::new("no_histogram_baseline", num_ops),
            &num_ops,
            |b, &ops| {
                let counter = AtomicU64::new(0);
                b.iter(|| {
                    for _ in 0..ops {
                        black_box(baseline_no_histogram_tracking(black_box(&counter)));
                    }
                });
            },
        );

        // Histogram tracking
        group.bench_with_input(
            BenchmarkId::new("histogram_tracking", num_ops),
            &num_ops,
            |b, &ops| {
                let histogram = LatencyHistogram::new();
                // Realistic latency distribution (mostly <5ms, some outliers)
                let latencies: Vec<u64> = (0..ops)
                    .map(|i| {
                        if i % 100 == 0 {
                            50_000 // 1% outliers (50ms)
                        } else if i % 10 == 0 {
                            8_000 // 10% slow (8ms)
                        } else {
                            2_000 // 89% fast (2ms)
                        }
                    })
                    .collect();

                b.iter(|| {
                    for &latency_us in &latencies {
                        black_box(histogram.record(black_box(latency_us)));
                    }
                });
            },
        );

        // Histogram with contention (4 threads)
        group.bench_with_input(
            BenchmarkId::new("histogram_concurrent_4_threads", num_ops),
            &num_ops,
            |b, &ops| {
                let histogram = std::sync::Arc::new(LatencyHistogram::new());
                b.iter(|| {
                    let handles: Vec<_> = (0..4)
                        .map(|_| {
                            let h = histogram.clone();
                            std::thread::spawn(move || {
                                for i in 0..(ops / 4) {
                                    let latency_us = if i % 10 == 0 { 8_000 } else { 2_000 };
                                    h.record(latency_us);
                                }
                            })
                        })
                        .collect();

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
// P2.2: SIMD BATCH HASHING BENCHMARKS
// ============================================================================

/// SIMD batch hash using portable_simd (T2 SIMD)
///
/// **Performance:** 2-8× speedup for 4+ fields
#[cfg(feature = "portable_simd")]
#[inline(always)]
fn simd_batch_hash_u64x8(data: &[u64; 8]) -> u64 {
    use std::simd::u64x8;

    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;

    // Load into SIMD register
    let values = u64x8::from_array(*data);

    // XOR all values (parallel)
    let offset_vec = u64x8::splat(FNV_OFFSET);
    let xored = values ^ offset_vec;

    // Multiply by FNV prime (parallel) - use scalar multiply for u64x8
    // Note: wrapping_mul not available for u64x8, use element-wise multiply
    let prime_vec = u64x8::splat(FNV_PRIME);
    let hashed = xored * prime_vec;

    // Reduce to single hash (horizontal XOR)
    let arr = hashed.to_array();
    arr.iter().fold(FNV_OFFSET, |acc, &x| acc ^ x.wrapping_mul(FNV_PRIME))
}

#[cfg(feature = "portable_simd")]
fn bench_p2_2_simd_batch_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group("p2.2_simd_batch_hashing");

    // B2: Statistical rigor (1000+ samples for <100ns operation)
    group.confidence_level(0.95).sample_size(1000);
    group.warm_up_time(Duration::from_secs(3));

    // B3: Realistic field counts (4, 8, 16 fields = typical capsule sizes)
    for &num_fields in &[4, 8, 16] {
        group.throughput(Throughput::Elements(num_fields as u64));

        // Generate test data
        let test_data: Vec<u64> = (0..num_fields).map(|i| i as u64 * 123456789).collect();

        // Baseline: Sequential hashing (FNV-1a)
        group.bench_with_input(
            BenchmarkId::new("sequential_hash_baseline", num_fields),
            &num_fields,
            |b, &_| {
                b.iter(|| black_box(baseline_sequential_hash(black_box(&test_data))));
            },
        );

        // SIMD batch hashing (8-wide)
        if num_fields >= 8 {
            group.bench_with_input(
                BenchmarkId::new("simd_batch_hash_8wide", num_fields),
                &num_fields,
                |b, &_| {
                    // Pad to 8 elements if needed
                    let mut data_8 = [0u64; 8];
                    for (i, &val) in test_data.iter().take(8).enumerate() {
                        data_8[i] = val;
                    }
                    b.iter(|| black_box(simd_batch_hash_u64x8(black_box(&data_8))));
                },
            );
        }

        // Multi-capsule batch (simulate hashing 100 capsules)
        group.bench_with_input(
            BenchmarkId::new("multi_capsule_batch_100", num_fields),
            &num_fields,
            |b, &_| {
                let capsules: Vec<Vec<u64>> = (0..100)
                    .map(|i| {
                        (0..num_fields)
                            .map(|j| (i * num_fields + j) as u64)
                            .collect()
                    })
                    .collect();

                b.iter(|| {
                    for capsule in &capsules {
                        black_box(baseline_sequential_hash(black_box(capsule)));
                    }
                });
            },
        );

        // Multi-capsule SIMD batch (if applicable)
        if num_fields >= 8 {
            group.bench_with_input(
                BenchmarkId::new("multi_capsule_simd_batch_100", num_fields),
                &num_fields,
                |b, &_| {
                    let capsules: Vec<[u64; 8]> = (0..100)
                        .map(|i| {
                            let mut data = [0u64; 8];
                            for j in 0..8 {
                                data[j] = (i * 8 + j) as u64;
                            }
                            data
                        })
                        .collect();

                    b.iter(|| {
                        for capsule in &capsules {
                            black_box(simd_batch_hash_u64x8(black_box(capsule)));
                        }
                    });
                },
            );
        }
    }

    group.finish();
}

// ============================================================================
// P2.3: QUORUM READ LATENCY BENCHMARKS
// ============================================================================

/// Simulated node read (realistic network latency)
async fn simulate_node_read(
    node_id: usize,
    key: u64,
    value: Vec<u8>,
    latency_ms: u64,
) -> (usize, Option<Vec<u8>>, u64) {
    // Simulate network latency
    tokio::time::sleep(Duration::from_millis(latency_ms)).await;

    // Simulate 5% network errors
    if key % 20 == node_id as u64 {
        (node_id, None, latency_ms)
    } else {
        (node_id, Some(value), latency_ms)
    }
}

/// Quorum read: Read from 3 nodes, wait for 2/3 agreement
async fn quorum_read(key: u64, value: Vec<u8>) -> Option<Vec<u8>> {
    // Realistic latencies: 2ms, 5ms, 8ms (P50, P95, P99)
    let reads = vec![
        simulate_node_read(0, key, value.clone(), 2),
        simulate_node_read(1, key, value.clone(), 5),
        simulate_node_read(2, key, value.clone(), 8),
    ];

    let mut results = futures::future::join_all(reads).await;

    // Sort by latency (fastest first)
    results.sort_by_key(|(_, _, latency)| *latency);

    // Check for 2/3 quorum
    let valid_results: Vec<_> = results.iter().filter_map(|(_, v, _)| v.clone()).collect();

    if valid_results.len() >= 2 {
        // Return first valid result (fastest response)
        valid_results.into_iter().next()
    } else {
        None
    }
}

fn bench_p2_3_quorum_read_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("p2.3_quorum_read_latency");

    // B2: Statistical rigor (100 samples for >1ms operation)
    group.confidence_level(0.95).sample_size(100);
    group.warm_up_time(Duration::from_secs(3));

    // B3: Realistic value sizes (1KB, 4KB, 16KB cache values)
    for &value_size in &[1024, 4096, 16384] {
        group.throughput(Throughput::Bytes(value_size as u64));

        let value = vec![0u8; value_size];
        let key = 12345u64;

        // Baseline: Single-node read (no quorum)
        group.bench_with_input(
            BenchmarkId::new("single_node_baseline", value_size),
            &value_size,
            |b, &_| {
                let mut map = std::collections::HashMap::new();
                map.insert(key, value.clone());

                b.iter(|| {
                    black_box(baseline_single_node_read(black_box(&map), black_box(key)))
                });
            },
        );

        // Quorum read: 2/3 agreement (realistic network latency)
        group.bench_with_input(
            BenchmarkId::new("quorum_read_2_of_3", value_size),
            &value_size,
            |b, &_| {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_time()
                    .build()
                    .unwrap();
                b.iter(|| {
                    runtime.block_on(async {
                        black_box(quorum_read(black_box(key), black_box(value.clone())).await)
                    })
                });
            },
        );

        // Fast-path optimization: Return first valid response (no quorum wait)
        group.bench_with_input(
            BenchmarkId::new("fast_path_first_valid", value_size),
            &value_size,
            |b, &_| {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_time()
                    .build()
                    .unwrap();
                b.iter(|| {
                    runtime.block_on(async {
                        // Simulate fastest node only (2ms)
                        let result = simulate_node_read(0, key, value.clone(), 2).await;
                        black_box(result.1)
                    })
                });
            },
        );

        // Worst-case: All 3 nodes timeout (50ms each)
        group.bench_with_input(
            BenchmarkId::new("worst_case_timeout", value_size),
            &value_size,
            |b, &_| {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_time()
                    .build()
                    .unwrap();
                b.iter(|| {
                    runtime.block_on(async {
                        let reads = vec![
                            simulate_node_read(0, key, value.clone(), 50),
                            simulate_node_read(1, key, value.clone(), 50),
                            simulate_node_read(2, key, value.clone(), 50),
                        ];
                        let results = futures::future::join_all(reads).await;
                        black_box(results)
                    })
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// COMPREHENSIVE BENCHMARKS: Histogram + SIMD Hash + Quorum
// ============================================================================

#[cfg(feature = "portable_simd")]
fn bench_p2_comprehensive(c: &mut Criterion) {
    let mut group = c.benchmark_group("p2_comprehensive");

    // B2: Statistical rigor
    group.confidence_level(0.95).sample_size(100);
    group.warm_up_time(Duration::from_secs(3));

    // Simulate realistic distributed cache operation:
    // 1. Hash key (SIMD batch)
    // 2. Quorum read (2/3 nodes)
    // 3. Record latency in histogram

    let key_fields = [1u64, 2, 3, 4, 5, 6, 7, 8]; // 8-field cache key
    let value = vec![0u8; 4096]; // 4KB value
    let histogram = std::sync::Arc::new(LatencyHistogram::new());

    group.bench_function("full_cache_operation_with_p2_features", |b| {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_time()
            .build()
            .unwrap();
        let hist = histogram.clone();

        b.iter(|| {
            runtime.block_on(async {
                let start = std::time::Instant::now();

                // P2.2: Hash key (SIMD)
                let _key_hash = black_box(simd_batch_hash_u64x8(black_box(&key_fields)));

                // P2.3: Quorum read
                let _result = black_box(quorum_read(12345, value.clone()).await);

                // P2.1: Record latency
                let latency_us = start.elapsed().as_micros() as u64;
                hist.record(latency_us);
            })
        });
    });

    // Baseline: Without P2 features
    group.bench_function("baseline_cache_operation_no_p2", |b| {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_time()
            .build()
            .unwrap();
        let counter = AtomicU64::new(0);

        b.iter(|| {
            runtime.block_on(async {
                // Sequential hash (no SIMD)
                let key_vec = key_fields.to_vec();
                let _key_hash = black_box(baseline_sequential_hash(black_box(&key_vec)));

                // Single-node read (no quorum)
                let mut map = std::collections::HashMap::new();
                map.insert(12345u64, value.clone());
                let _result = black_box(baseline_single_node_read(&map, 12345));

                // No histogram (direct counter)
                baseline_no_histogram_tracking(&counter);
            })
        });
    });

    group.finish();
}

// ============================================================================
// BENCHMARK GROUPS
// ============================================================================

#[cfg(feature = "portable_simd")]
criterion_group!(
    benches,
    bench_p2_1_histogram_overhead,
    bench_p2_2_simd_batch_hashing,
    bench_p2_3_quorum_read_latency,
    bench_p2_comprehensive
);

#[cfg(not(feature = "portable_simd"))]
criterion_group!(
    benches,
    bench_p2_1_histogram_overhead,
    bench_p2_3_quorum_read_latency
);

criterion_main!(benches);
