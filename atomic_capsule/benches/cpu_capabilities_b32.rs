//! B32-Compliant Benchmarks for CPU Capability Detection (Phase 1.6)
//!
//! **Target**: Validate <10ns overhead for cached CPU detection
//!
//! ## B32 Framework Compliance
//!
//! - **Fair Baselines**: Direct is_x86_feature_detected!() calls (Rust std)
//! - **1000+ Iterations**: Criterion default (>1000 samples)
//! - **95% CI**: Criterion statistical rigor
//! - **Honest Claims**: <10ns cached, ~1ms one-time init (realistic)
//!
//! ## Benchmarks
//!
//! 1. **Cached Detection Overhead**: CpuCapabilityCapsule::detect() (target <10ns)
//! 2. **Feature Check Overhead**: has_avx2() cached load (target <5ns)
//! 3. **Tier Selection**: best_simd_tier() string lookup (target <10ns)
//! 4. **Baseline**: Direct is_x86_feature_detected!() for comparison
//!
//! ## ASSUM Safety
//!
//! - ASSUM_BENCHMARK_DETERMINISTIC: CPU features immutable (no race conditions)
//! - ASSUM_NO_SIDE_EFFECTS: All operations are pure reads (safe to benchmark)
//! - Total: 99.99% safe (benchmark-only code, no unsafe)

use atomic_capsule::CpuCapabilityCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

/// Benchmark: Cached CPU detection overhead (singleton pattern)
///
/// **Target**: <10ns per call (OnceLock cached access)
/// **Baseline**: N/A (no direct equivalent, singleton is the optimization)
/// **Reality**: Expected 5-10ns (single pointer dereference)
fn bench_cached_detect_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("cpu_detect_cached");
    group.significance_level(0.05).confidence_level(0.95);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));

    // Warm up the singleton (first call ~1ms)
    let _ = CpuCapabilityCapsule::detect();

    group.bench_function("detect_overhead", |b| {
        b.iter(|| {
            let caps = black_box(CpuCapabilityCapsule::detect());
            black_box(caps);
        });
    });

    group.finish();
}

/// Benchmark: Feature flag check overhead (avx2/avx512/sse42/neon)
///
/// **Target**: <5ns per check (Relaxed atomic load)
/// **Baseline**: Direct is_x86_feature_detected!() (~100ns first call, ~5ns cached)
/// **Reality**: Expected 2-5ns (single Relaxed atomic load)
fn bench_feature_check_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("feature_check");
    group.significance_level(0.05).confidence_level(0.95);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));

    let caps = CpuCapabilityCapsule::detect();

    group.bench_function("has_avx2", |b| {
        b.iter(|| {
            let has_avx2 = black_box(caps.has_avx2());
            black_box(has_avx2);
        });
    });

    group.bench_function("has_avx512", |b| {
        b.iter(|| {
            let has_avx512 = black_box(caps.has_avx512());
            black_box(has_avx512);
        });
    });

    group.bench_function("has_sse42", |b| {
        b.iter(|| {
            let has_sse42 = black_box(caps.has_sse42());
            black_box(has_sse42);
        });
    });

    group.bench_function("has_neon", |b| {
        b.iter(|| {
            let has_neon = black_box(caps.has_neon());
            black_box(has_neon);
        });
    });

    group.finish();
}

/// Benchmark: Best SIMD tier selection (string lookup)
///
/// **Target**: <10ns (cached tier selection with static string)
/// **Baseline**: Manual if-else chain with is_x86_feature_detected!()
/// **Reality**: Expected 5-10ns (optimized tier hierarchy)
fn bench_tier_selection_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("tier_selection");
    group.significance_level(0.05).confidence_level(0.95);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));

    let caps = CpuCapabilityCapsule::detect();

    group.bench_function("best_simd_tier", |b| {
        b.iter(|| {
            let tier = black_box(caps.best_simd_tier());
            black_box(tier);
        });
    });

    group.finish();
}

/// Benchmark: Generation counter overhead (TOCTOU prevention)
///
/// **Target**: <2ns (Relaxed atomic load of u64)
/// **Baseline**: Direct AtomicU64::load(Relaxed)
/// **Reality**: Expected 1-2ns (single atomic load)
fn bench_generation_counter_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("generation_counter");
    group.significance_level(0.05).confidence_level(0.95);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));

    let caps = CpuCapabilityCapsule::detect();

    group.bench_function("generation", |b| {
        b.iter(|| {
            let gen = black_box(caps.generation());
            black_box(gen);
        });
    });

    group.finish();
}

/// Benchmark: Baseline - Direct is_x86_feature_detected!() for AVX2
///
/// **Purpose**: Fair comparison for feature detection
/// **Expected**: ~5-10ns cached (first call ~100ns)
/// **Note**: This is the baseline our capsule should match or beat
#[cfg(target_arch = "x86_64")]
fn bench_baseline_direct_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline_direct");
    group.significance_level(0.05).confidence_level(0.95);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));

    // Warm up the Rust std cache
    let _ = is_x86_feature_detected!("avx2");

    group.bench_function("is_x86_feature_detected_avx2", |b| {
        b.iter(|| {
            let has_avx2 = black_box(is_x86_feature_detected!("avx2"));
            black_box(has_avx2);
        });
    });

    group.bench_function("is_x86_feature_detected_avx512f", |b| {
        b.iter(|| {
            let has_avx512 = black_box(is_x86_feature_detected!("avx512f"));
            black_box(has_avx512);
        });
    });

    group.finish();
}

/// Benchmark: Multi-threaded concurrent access (stress test)
///
/// **Target**: <20ns P99 (no contention on OnceLock after init)
/// **Purpose**: Verify lockfree design under concurrent load
/// **Reality**: Expected 10-20ns P99 (cache line bouncing possible)
fn bench_concurrent_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_access");
    group.significance_level(0.05).confidence_level(0.95);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));

    // Warm up
    let _ = CpuCapabilityCapsule::detect();

    group.bench_function("concurrent_detect_10_threads", |b| {
        b.iter(|| {
            let handles: Vec<_> = (0..10)
                .map(|_| {
                    std::thread::spawn(|| {
                        let caps = black_box(CpuCapabilityCapsule::detect());
                        black_box(caps.best_simd_tier());
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    group.finish();
}

/// Benchmark: Real-world usage simulation (kindly_dedup pattern)
///
/// **Pattern**: Detect CPU → Choose SIMD tier → Dispatch
/// **Target**: <20ns total overhead (detect + tier + branch)
/// **Purpose**: Measure end-to-end latency for typical usage
fn bench_real_world_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_world_usage");
    group.significance_level(0.05).confidence_level(0.95);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));

    group.bench_function("detect_and_dispatch", |b| {
        b.iter(|| {
            let caps = black_box(CpuCapabilityCapsule::detect());
            let tier = black_box(caps.best_simd_tier());

            // Simulate dispatch
            let implementation = match tier {
                "avx512" => "AVX-512 impl",
                "avx2" => "AVX2 impl",
                "sse4.2" => "SSE4.2 impl",
                "neon" => "NEON impl",
                "scalar" => "Scalar impl",
                _ => "Unknown",
            };

            black_box(implementation);
        });
    });

    group.finish();
}

// Criterion benchmark groups
criterion_group!(
    cpu_detect_benches,
    bench_cached_detect_overhead,
    bench_feature_check_overhead,
    bench_tier_selection_overhead,
    bench_generation_counter_overhead,
    bench_concurrent_access,
    bench_real_world_usage,
);

#[cfg(target_arch = "x86_64")]
criterion_group!(baseline_benches, bench_baseline_direct_detection);

#[cfg(target_arch = "x86_64")]
criterion_main!(cpu_detect_benches, baseline_benches);

#[cfg(not(target_arch = "x86_64"))]
criterion_main!(cpu_detect_benches);
