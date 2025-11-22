#![cfg_attr(feature = "portable_simd", feature(portable_simd))]
//! # MinHash SIMD vs Scalar Benchmark (B32 Compliant)
//!
//! **Purpose**: Validate 2-8× SIMD speedup claim for MinHash signature computation
//!
//! ## B32 Compliance
//!
//! - **Fair Baselines**: Scalar vs SIMD on same hardware
//! - **Statistical Rigor**: 1000+ iterations via Criterion.rs, 95% CI
//! - **Realistic Workloads**: Multiple token counts (10, 100, 1000)
//! - **Honest Interpretation**: 2-8× is EXCEPTIONAL tier, requires validation
//!
//! ## Expected Results
//!
//! Based on B32 framework K9 (SIMD Reality):
//! - **AVX2 Theoretical**: 8× speedup (8-lane SIMD)
//! - **AVX2 Measured**: 3-4× typical for general workloads
//! - **MinHash Specific**: 2-8× target (vectorized hash + min operations)
//!
//! ## Methodology
//!
//! 1. **Baseline**: compute_signature() - scalar MurmurHash3 implementation
//! 2. **SIMD**: simd_compute_signature() - 8-lane vectorized MurmurHash3
//! 3. **Token Counts**: 10 (small), 100 (typical), 1000 (large)
//! 4. **Measurement**: Per-document latency (microseconds)
//!
//! ## Hardware Requirements
//!
//! - x86-64 with AVX2 (Intel Ultra 7 155H or AMD Ryzen 9 6900HX)
//! - Nightly Rust with portable_simd feature
//!
//! ## Usage
//!
//! ```bash
//! # Run benchmark
//! cargo +nightly bench --bench minhash_simd_bench --features portable_simd
//!
//! # View results
//! open target/criterion/minhash_compute/report/index.html
//! ```

use atomic_capsule::probabilistic::MinHashSignatureCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

#[cfg(feature = "portable_simd")]
use atomic_capsule::hash::murmur3_simd::murmur3_hash_simd_x8;

/// Generate token set of specified size
fn generate_tokens(count: usize) -> Vec<String> {
    (0..count).map(|i| format!("token_{}", i)).collect()
}

/// Benchmark MinHash signature computation: Scalar vs SIMD
fn bench_minhash_scalar_vs_simd(c: &mut Criterion) {
    let mut group = c.benchmark_group("minhash_compute");

    // Configure for B32 compliance
    group.confidence_level(0.95); // 95% confidence interval
    group.sample_size(1000); // 1000+ iterations for statistical significance

    for token_count in [10, 100, 1000] {
        // Set throughput for per-document measurement
        group.throughput(Throughput::Elements(1));

        let tokens: Vec<String> = generate_tokens(token_count);
        let token_refs: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();

        // Baseline: Scalar implementation
        group.bench_with_input(
            BenchmarkId::new("scalar", token_count),
            &token_refs,
            |b, tokens| b.iter(|| MinHashSignatureCapsule::compute_signature(black_box(tokens))),
        );

        // SIMD: Vectorized implementation (nightly only)
        #[cfg(feature = "portable_simd")]
        group.bench_with_input(
            BenchmarkId::new("simd", token_count),
            &token_refs,
            |b, tokens| b.iter(|| simd_compute_signature_bench(black_box(tokens))),
        );
    }

    group.finish();
}

/// SIMD MinHash signature computation (benchmark version)
///
/// This is a local copy to avoid cross-crate dependencies.
/// Production version is in kindly_dedup::simd_minhash module.
#[cfg(feature = "portable_simd")]
fn simd_compute_signature_bench(tokens: &[&str]) -> MinHashSignatureCapsule {
    use core::simd::{cmp::SimdOrd, u16x8};

    const NUM_HASHES: usize = 128;
    const SIMD_LANES: usize = 8;
    const ITERATIONS: usize = NUM_HASHES / SIMD_LANES;

    let mut signature = [u16::MAX; NUM_HASHES];

    for token in tokens {
        let token_u64 = token_to_u64(token);

        for iter in 0..ITERATIONS {
            let element = token_u64 ^ (iter as u64);
            let simd_hashes = murmur3_hash_simd_x8(element);

            let hashes: [u16; 8] = [
                (simd_hashes[0] & 0xFFFF) as u16,
                (simd_hashes[1] & 0xFFFF) as u16,
                (simd_hashes[2] & 0xFFFF) as u16,
                (simd_hashes[3] & 0xFFFF) as u16,
                (simd_hashes[4] & 0xFFFF) as u16,
                (simd_hashes[5] & 0xFFFF) as u16,
                (simd_hashes[6] & 0xFFFF) as u16,
                (simd_hashes[7] & 0xFFFF) as u16,
            ];

            let hash_vec = u16x8::from_array(hashes);
            let start = iter * SIMD_LANES;
            let sig_vec = u16x8::from_slice(&signature[start..start + SIMD_LANES]);
            let min_vec = sig_vec.simd_min(hash_vec);
            min_vec.copy_to_slice(&mut signature[start..start + SIMD_LANES]);
        }
    }

    MinHashSignatureCapsule::from_signature(signature)
}

/// Convert token to u64 using FNV-1a hash
#[cfg(feature = "portable_simd")]
#[inline(always)]
fn token_to_u64(token: &str) -> u64 {
    let bytes = token.as_bytes();
    let mut h = 0xcbf29ce484222325_u64;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3_u64);
    }
    h
}

/// Benchmark Jaccard similarity computation: Scalar vs SIMD
fn bench_jaccard_similarity(c: &mut Criterion) {
    let mut group = c.benchmark_group("jaccard_similarity");

    // Configure for B32 compliance
    group.confidence_level(0.95);
    group.sample_size(1000);

    // Create two signatures with ~50% overlap
    let tokens1: Vec<String> = (0..100).map(|i| format!("token_{}", i)).collect();
    let tokens2: Vec<String> = (50..150).map(|i| format!("token_{}", i)).collect();

    let refs1: Vec<&str> = tokens1.iter().map(|s| s.as_str()).collect();
    let refs2: Vec<&str> = tokens2.iter().map(|s| s.as_str()).collect();

    let sig1 = MinHashSignatureCapsule::compute_signature(&refs1);
    let sig2 = MinHashSignatureCapsule::compute_signature(&refs2);

    // Set throughput for per-comparison measurement
    group.throughput(Throughput::Elements(1));

    group.bench_function("scalar", |b| {
        b.iter(|| black_box(sig1.jaccard_similarity(black_box(&sig2))))
    });

    group.finish();
}

/// Benchmark end-to-end: Token set to Jaccard similarity
fn bench_end_to_end(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end");

    // Configure for B32 compliance
    group.confidence_level(0.95);
    group.sample_size(500); // Slightly lower for longer operations

    let tokens1: Vec<String> = (0..100).map(|i| format!("token_{}", i)).collect();
    let tokens2: Vec<String> = (50..150).map(|i| format!("token_{}", i)).collect();

    let refs1: Vec<&str> = tokens1.iter().map(|s| s.as_str()).collect();
    let refs2: Vec<&str> = tokens2.iter().map(|s| s.as_str()).collect();

    // Set throughput for per-pair measurement
    group.throughput(Throughput::Elements(1));

    // Scalar end-to-end
    group.bench_function("scalar", |b| {
        b.iter(|| {
            let sig1 = MinHashSignatureCapsule::compute_signature(black_box(&refs1));
            let sig2 = MinHashSignatureCapsule::compute_signature(black_box(&refs2));
            black_box(sig1.jaccard_similarity(&sig2))
        })
    });

    // SIMD end-to-end (nightly only)
    #[cfg(feature = "portable_simd")]
    group.bench_function("simd", |b| {
        b.iter(|| {
            let sig1 = simd_compute_signature_bench(black_box(&refs1));
            let sig2 = simd_compute_signature_bench(black_box(&refs2));
            black_box(sig1.jaccard_similarity(&sig2))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_minhash_scalar_vs_simd,
    bench_jaccard_similarity,
    bench_end_to_end
);
criterion_main!(benches);
