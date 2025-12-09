//! B32 benchmark suite for ConstantTimeOpsCapsule
//!
//! **Framework**: B32 (Fair Baseline Performance Measurement)
//! - Baseline: memcmp for comparison, volatile writes for zero
//! - Methodology: 1000+ iterations, 95% CI, fair comparison
//! - Reality Check: 10-50% typical, 2-10× exceptional
//!
//! **Performance Targets**:
//! - Comparison: <20ns (same as memcmp baseline)
//! - Select: <10ns (branchless ternary)
//! - SIMD speedup: 2-8× vs scalar
//! - Timing variance: Zero (<1ns σ)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use kindly_verified_web::capsules::security::ConstantTimeOpsCapsule;
use std::time::Instant;

// ============================================================================
// Baseline Comparisons (for B32 fair baseline measurement)
// ============================================================================

/// Baseline memcmp-style comparison
#[inline(never)]
fn baseline_memcmp(a: &[u8], b: &[u8]) -> bool {
    a == b
}

/// Baseline volatile zero (for fair comparison)
#[inline(never)]
fn baseline_volatile_zero(buf: &mut [u8]) {
    unsafe {
        std::ptr::write_bytes(buf.as_mut_ptr(), 0, buf.len());
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

// ============================================================================
// B1: Comparison Latency Benchmark (Target: <20ns)
// ============================================================================

fn bench_comparison_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("comparison_latency");
    group.measurement_time(std::time::Duration::from_secs(5));
    group.sample_size(1000);

    for size in &[8, 16, 32, 64, 128, 256] {
        let capsule = ConstantTimeOpsCapsule::new();
        let a = vec![0x42u8; *size];
        let b = vec![0x42u8; *size];

        // Baseline: memcmp
        group.bench_with_input(
            BenchmarkId::new("baseline_memcmp", size),
            size,
            |b, _| b.iter(|| baseline_memcmp(black_box(&a), black_box(&b))),
        );

        // ConstantTimeOps: Scalar comparison
        group.bench_with_input(
            BenchmarkId::new("constant_time_eq_scalar", size),
            size,
            |b, _| {
                b.iter(|| {
                    capsule.constant_time_eq(black_box(&a), black_box(&b))
                })
            },
        );

        // ConstantTimeOps: SIMD comparison
        group.bench_with_input(
            BenchmarkId::new("constant_time_eq_simd", size),
            size,
            |b, _| {
                b.iter(|| {
                    capsule.simd_constant_time_eq(black_box(&a), black_box(&b))
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// B2: Select Latency Benchmark (Target: <10ns)
// ============================================================================

fn bench_select_latency(c: &mut Criterion) {
    let capsule = ConstantTimeOpsCapsule::new();
    let mut group = c.benchmark_group("select_latency");
    group.measurement_time(std::time::Duration::from_secs(5));
    group.sample_size(10000);

    group.bench_function("select_true", |b| {
        b.iter(|| capsule.constant_time_select(true, black_box(42u64), black_box(13u64)))
    });

    group.bench_function("select_false", |b| {
        b.iter(|| capsule.constant_time_select(false, black_box(42u64), black_box(13u64)))
    });

    group.finish();
}

// ============================================================================
// B3: Zero Latency Benchmark (Target: <100ns per 64 bytes)
// ============================================================================

fn bench_zero_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("zero_latency");
    group.measurement_time(std::time::Duration::from_secs(5));
    group.sample_size(1000);

    for size in &[32, 64, 128, 256, 512] {
        let capsule = ConstantTimeOpsCapsule::new();
        let mut buf = vec![0xFFu8; *size];

        // Baseline: volatile zero
        let mut buf_baseline = buf.clone();
        group.bench_with_input(
            BenchmarkId::new("baseline_volatile_zero", size),
            size,
            |b, _| {
                b.iter(|| {
                    baseline_volatile_zero(black_box(&mut buf_baseline));
                })
            },
        );

        // ConstantTimeOps: constant_time_zero
        group.bench_with_input(
            BenchmarkId::new("constant_time_zero", size),
            size,
            |b, _| {
                b.iter(|| {
                    capsule.constant_time_zero(black_box(&mut buf.clone()));
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// B4: SIMD Speedup Measurement (Target: 2-8×)
// ============================================================================

fn bench_simd_speedup(c: &mut Criterion) {
    let capsule = ConstantTimeOpsCapsule::new();
    let mut group = c.benchmark_group("simd_speedup");
    group.measurement_time(std::time::Duration::from_secs(5));
    group.sample_size(1000);

    for size in &[64, 128, 256, 512, 1024] {
        let a = vec![0x42u8; *size];
        let b = vec![0x42u8; *size];

        // Scalar baseline
        group.bench_with_input(
            BenchmarkId::new("scalar", size),
            size,
            |b, _| {
                b.iter(|| {
                    capsule.constant_time_eq(black_box(&a), black_box(&b))
                })
            },
        );

        // SIMD implementation
        group.bench_with_input(
            BenchmarkId::new("simd", size),
            size,
            |b, _| {
                b.iter(|| {
                    capsule.simd_constant_time_eq(black_box(&a), black_box(&b))
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// B5: Timing Variance Measurement (Target: σ ≈ 0ns)
// ============================================================================

fn bench_timing_variance(c: &mut Criterion) {
    let capsule = ConstantTimeOpsCapsule::new();
    let mut group = c.benchmark_group("timing_variance");
    group.sample_size(100);

    let token = b"test_constant_time_token_0123456789";
    let matching = b"test_constant_time_token_0123456789";
    let different = b"different_token_string_0123456789x";

    group.bench_function("eq_matching", |b| {
        b.iter(|| capsule.constant_time_eq(black_box(token), black_box(matching)))
    });

    group.bench_function("eq_different", |b| {
        b.iter(|| capsule.constant_time_eq(black_box(token), black_box(different)))
    });

    group.finish();
}

// ============================================================================
// B6: Copy Latency Benchmark (Target: <100ns per 64 bytes)
// ============================================================================

fn bench_copy_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("copy_latency");
    group.measurement_time(std::time::Duration::from_secs(5));
    group.sample_size(1000);

    for size in &[32, 64, 128, 256] {
        let capsule = ConstantTimeOpsCapsule::new();
        let src = vec![0x42u8; *size];

        group.bench_with_input(
            BenchmarkId::new("constant_time_copy", size),
            size,
            |b, _| {
                b.iter(|| {
                    let mut dst = vec![0u8; *size];
                    capsule.constant_time_copy(&mut dst, black_box(&src)).unwrap();
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// B7: Concurrent Operations Benchmark
// ============================================================================

fn bench_concurrent_operations(c: &mut Criterion) {
    let capsule = std::sync::Arc::new(ConstantTimeOpsCapsule::new());
    let mut group = c.benchmark_group("concurrent_ops");
    group.measurement_time(std::time::Duration::from_secs(5));
    group.sample_size(100);

    for num_threads in &[1, 2, 4, 8, 16] {
        group.bench_with_input(
            BenchmarkId::new("select_concurrent", num_threads),
            num_threads,
            |b, &n| {
                b.iter(|| {
                    let mut handles = vec![];
                    for _ in 0..n {
                        let capsule_clone = std::sync::Arc::clone(&capsule);
                        let handle = std::thread::spawn(move || {
                            for _ in 0..100 {
                                capsule_clone.constant_time_select(true, 42, 13);
                            }
                        });
                        handles.push(handle);
                    }
                    for handle in handles {
                        handle.join().unwrap();
                    }
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// B8: Real-World Use Case Benchmarks
// ============================================================================

fn bench_real_world_scenarios(c: &mut Criterion) {
    let capsule = ConstantTimeOpsCapsule::new();
    let mut group = c.benchmark_group("real_world_scenarios");
    group.measurement_time(std::time::Duration::from_secs(5));
    group.sample_size(1000);

    // Scenario 1: Password verification (hash comparison)
    let password_hash = b"$2b$12$R9h7cIPz0gi.URNNX3kh2OPST9/PgBkqquzi.Ss7KIUgO2t0jWMUm";
    let candidate_hash = b"$2b$12$R9h7cIPz0gi.URNNX3kh2OPST9/PgBkqquzi.Ss7KIUgO2t0jWMUm";
    group.bench_function("password_verification", |b| {
        b.iter(|| capsule.constant_time_eq(black_box(password_hash), black_box(candidate_hash)))
    });

    // Scenario 2: Token comparison (JWT-like)
    let token1 = b"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
    let token2 = b"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
    group.bench_function("token_comparison", |b| {
        b.iter(|| capsule.constant_time_eq(black_box(token1), black_box(token2)))
    });

    // Scenario 3: HMAC verification
    let expected_mac = b"c910e8400c09c05c7b6c7f4d3a3c6e4a";
    let computed_mac = b"c910e8400c09c05c7b6c7f4d3a3c6e4a";
    group.bench_function("hmac_verification", |b| {
        b.iter(|| capsule.constant_time_eq(black_box(expected_mac), black_box(computed_mac)))
    });

    // Scenario 4: Key comparison (AES-256)
    let key1 = [0x2B; 32];
    let key2 = [0x2B; 32];
    group.bench_function("key_comparison", |b| {
        b.iter(|| capsule.constant_time_eq(black_box(&key1), black_box(&key2)))
    });

    group.finish();
}

// ============================================================================
// Custom Manual Timing (for detailed analysis)
// ============================================================================

#[test]
fn manual_timing_analysis() {
    let capsule = ConstantTimeOpsCapsule::new();

    // Time comparison with matching data
    let matching = vec![0x42u8; 64];
    let iterations = 10_000;

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = capsule.constant_time_eq(&matching, &matching);
    }
    let elapsed_ns = start.elapsed().as_nanos() as u64;
    let per_op_ns = elapsed_ns / iterations;

    println!("\n=== Manual Timing Analysis ===");
    println!("Iterations: {}", iterations);
    println!("Total time: {} ns", elapsed_ns);
    println!("Per-operation: {} ns", per_op_ns);
    println!("Status: {} ns (target: <20ns)", per_op_ns);

    assert!(per_op_ns < 50, "Comparison too slow: {} ns", per_op_ns);
}

criterion_group!(
    benches,
    bench_comparison_latency,
    bench_select_latency,
    bench_zero_latency,
    bench_simd_speedup,
    bench_timing_variance,
    bench_copy_latency,
    bench_concurrent_operations,
    bench_real_world_scenarios
);

criterion_main!(benches);
