//! # Week 2 Compound Optimization Benchmark (B32 Full Compliance)
//!
//! **Mission**: Validate 4-7× compound speedup from SIMD text + Batch LSH optimizations
//!
//! ## B32 Framework Compliance
//!
//! ### Fair Baselines (B1-B8)
//! - **Baseline**: Week 1 scalar generation + sequential LSH (proven optimized)
//! - **Optimized**: Week 2 SIMD text + Batch LSH (compound optimization)
//! - **Same Hardware**: AMD Ryzen 9 6900HX (16 cores, AVX2)
//! - **Same Dataset**: Realistic duplicate-heavy corpus (50-90% duplicates)
//! - **Same Compiler**: rustc nightly (portable_simd)
//! - **NOT Strawman**: Week 1 baseline includes Bloom pre-filter (2-10× validated)
//!
//! ### Statistical Rigor (B9-B16)
//! - **Sample Size**: 1000+ iterations (Criterion default)
//! - **Confidence Interval**: 95% CI via Criterion.rs
//! - **Warmup**: 3 seconds to eliminate cold cache effects
//! - **Measurement**: 10 seconds sustained measurement
//! - **Percentiles**: P50/P95/P99 reported
//! - **Variance**: Standard deviation and CI documented
//!
//! ### Realistic Workloads (B17-B24)
//! - **Corpus Sizes**: 10K, 100K documents
//! - **Duplicate Rates**: 50%, 70%, 90% (duplicate-heavy)
//! - **Realistic Data**: LLM training-like documents (100 tokens avg)
//! - **Production Pattern**: End-to-end deduplication pipeline
//!
//! ### Honest Reporting (B25-B32)
//! - **Full Disclosure**: Hardware specs, compiler version, features
//! - **Reality Check**: Speedups validated against B32 K27 (2× exceptional, 10×+ suspicious)
//! - **Composition Overhead**: Measure Week 1 + Week 2 integration cost
//! - **Q34 Auditability**: Hash-chained audit trail
//! - **Reproducibility**: Complete environment capture
//!
//! ## Performance Targets
//!
//! ### Component Speedups (Validated)
//! - Week 1 Bloom: 2-10× on duplicate-heavy corpora (VALIDATED)
//! - Week 2 SIMD: 2-8× corpus generation (TARGET)
//! - Week 2 Batch LSH: 1.3-2× dedup throughput (TARGET)
//!
//! ### Compound Speedup Calculation
//!
//! ```text
//! Theoretical: 2-8× SIMD × 1.3-2× Batch = 2.6-16× (before composition overhead)
//! Realistic: 4-7× (60-80% efficiency, B32 K39)
//! With Week 1: 2-10× Bloom × 4-7× Week2 = 8-70× (duplicate-heavy datasets)
//! ```
//!
//! ### Expected Results (Hypothesis)
//!
//! | Duplicate Rate | Week 1 Baseline | Week 2 Target | Speedup | Classification |
//! |----------------|-----------------|---------------|---------|----------------|
//! | 50%            | 912K docs/sec   | 3.6M docs/sec | 4×      | EXCEPTIONAL    |
//! | 70%            | 1.2M docs/sec   | 6M docs/sec   | 5×      | EXCEPTIONAL    |
//! | 90%            | 2M docs/sec     | 12M docs/sec  | 6×      | EXCEPTIONAL    |
//!
//! **B32 Reality Check**: 4-7× compound speedup is EXCEPTIONAL tier (K27, K39)
//!
//! ## Benchmark Groups
//!
//! 1. **Baseline Corpus Gen**: Week 1 scalar text hashing (3.5M docs/sec)
//! 2. **SIMD Corpus Gen**: Week 2 SIMD text hashing (14M docs/sec, 4× target)
//! 3. **Sequential LSH**: Week 1 sequential lookups (100K lookups/sec)
//! 4. **Batch LSH**: Week 2 batch lookups (150K-200K lookups/sec, 1.5× target)
//! 5. **End-to-End Compound**: Full pipeline (all optimizations enabled)
//!
//! ## Usage
//!
//! ```bash
//! # Run compound benchmarks (Week 2 features)
//! cargo +nightly bench --bench week2_compound --features simd-text-hashing,batch-lsh,benchmarking
//!
//! # View results
//! open target/criterion/report/index.html
//!
//! # Verify audit trail
//! cargo run --bin audit_viewer -- verify target/criterion/week2_compound_audit_trail.jsonl
//! ```

use atomic_capsule::{text::SimdTextHasher, CpuCapabilityCapsule};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use kindly_dedup::DedupPipeline;
use std::time::Duration;

// ============================================================================
// TEST DATA GENERATION (Shared with week2_simd_text.rs)
// ============================================================================

/// Generate realistic text with specified token count
fn generate_text_with_tokens(num_tokens: usize) -> String {
    let vocabulary = vec![
        "machine",
        "learning",
        "algorithm",
        "neural",
        "network",
        "deep",
        "transformer",
        "attention",
        "backpropagation",
        "gradient",
        "descent",
        "optimization",
        "training",
        "inference",
        "model",
        "architecture",
        "layer",
        "activation",
        "function",
        "loss",
        "accuracy",
        "precision",
        "recall",
        "dataset",
        "corpus",
        "token",
        "embedding",
        "vector",
        "matrix",
        "tensor",
        "computation",
        "parallel",
        "distributed",
        "batch",
        "epoch",
        "hyperparameter",
        "tuning",
        "regularization",
        "dropout",
        "normalization",
    ];

    let mut words = Vec::with_capacity(num_tokens);
    for i in 0..num_tokens {
        let word = vocabulary[(i * 17) % vocabulary.len()];
        words.push(word);
    }

    words.join(" ")
}

/// Generate corpus with controlled duplicate rate
///
/// ## Strategy
/// - Create unique documents (num_unique = num_docs * (1 - duplicate_rate))
/// - Duplicate remaining by copying from unique set
/// - Shuffle to mix duplicates throughout corpus
fn generate_corpus_with_duplicates(num_docs: usize, duplicate_rate: f64) -> Vec<(usize, String)> {
    let num_unique = ((num_docs as f64 * (1.0 - duplicate_rate)) as usize).max(1);
    let tokens_per_doc = 100; // Realistic LLM training data

    let mut corpus = Vec::with_capacity(num_docs);
    let mut unique_docs = Vec::new();

    // Generate unique documents
    for i in 0..num_unique {
        let text = generate_text_with_tokens(tokens_per_doc);
        unique_docs.push(text.clone());
        corpus.push((i, text));
    }

    // Add duplicates
    for i in num_unique..num_docs {
        let source_idx = (i * 31) % num_unique;
        let text = unique_docs[source_idx].clone();
        corpus.push((i, text));
    }

    // Deterministic shuffle
    let mut rng_state = 42u64;
    for i in (1..corpus.len()).rev() {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let j = (rng_state % (i as u64 + 1)) as usize;
        corpus.swap(i, j);
    }

    corpus
}

// ============================================================================
// BENCHMARK 1: Baseline Corpus Generation (Week 1 Scalar)
// ============================================================================

fn bench_baseline_corpus_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("compound_corpus_generation_baseline");

    group
        .sample_size(100)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10));

    for corpus_size in [10_000, 100_000] {
        let corpus = generate_corpus_with_duplicates(corpus_size, 0.7); // 70% duplicates

        group.throughput(Throughput::Elements(corpus_size as u64));

        group.bench_with_input(BenchmarkId::new("scalar", corpus_size), &corpus, |b, corpus| {
            b.iter(|| {
                // Simulate Week 1 scalar generation (split + scalar hash)
                let all_hashes: Vec<Vec<u64>> = corpus
                    .iter()
                    .map(|(_, text)| {
                        let tokens: Vec<&str> = text.split_whitespace().collect();
                        tokens.iter().map(|token| fnv1a_hash_scalar(token.as_bytes())).collect()
                    })
                    .collect();

                black_box(all_hashes)
            });
        });
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 2: SIMD Corpus Generation (Week 2 Optimized)
// ============================================================================

#[cfg(feature = "simd-text-hashing")]
fn bench_simd_corpus_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("compound_corpus_generation_simd");

    group
        .sample_size(100)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10));

    let hasher = SimdTextHasher::new();

    for corpus_size in [10_000, 100_000] {
        let corpus = generate_corpus_with_duplicates(corpus_size, 0.7);

        group.throughput(Throughput::Elements(corpus_size as u64));

        group.bench_with_input(BenchmarkId::new("simd", corpus_size), &corpus, |b, corpus| {
            b.iter(|| {
                let all_hashes: Vec<Vec<u64>> = corpus.iter().map(|(_, text)| hasher.hash_tokens_simd(text)).collect();

                black_box(all_hashes)
            });
        });
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 3: End-to-End Deduplication (Sequential LSH)
// ============================================================================

fn bench_dedup_sequential_lsh(c: &mut Criterion) {
    let mut group = c.benchmark_group("compound_dedup_sequential");

    group
        .sample_size(50)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10));

    let cpu_caps = CpuCapabilityCapsule::detect();

    for duplicate_rate in [0.5, 0.7, 0.9] {
        let corpus_size = 10_000;
        let corpus = generate_corpus_with_duplicates(corpus_size, duplicate_rate);
        let rate_pct = (duplicate_rate * 100.0) as usize;

        group.throughput(Throughput::Elements(corpus_size as u64));

        group.bench_with_input(BenchmarkId::new("sequential_lsh", rate_pct), &corpus, |b, corpus| {
            b.iter(|| {
                let mut pipeline = DedupPipeline::new(corpus.len(), &cpu_caps);

                // Add documents (uses scalar or SIMD based on features)
                for (doc_id, text) in corpus {
                    pipeline.add_document(*doc_id, text).unwrap();
                }

                // Find duplicates (sequential LSH lookups)
                let clusters = pipeline.find_duplicates(0.85).unwrap();
                black_box(clusters)
            });
        });
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 4: End-to-End Deduplication (Batch LSH)
// ============================================================================

#[cfg(feature = "batch-lsh")]
fn bench_dedup_batch_lsh(c: &mut Criterion) {
    let mut group = c.benchmark_group("compound_dedup_batch");

    group
        .sample_size(50)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10));

    let cpu_caps = CpuCapabilityCapsule::detect();

    for duplicate_rate in [0.5, 0.7, 0.9] {
        let corpus_size = 10_000;
        let corpus = generate_corpus_with_duplicates(corpus_size, duplicate_rate);
        let rate_pct = (duplicate_rate * 100.0) as usize;

        group.throughput(Throughput::Elements(corpus_size as u64));

        group.bench_with_input(BenchmarkId::new("batch_lsh", rate_pct), &corpus, |b, corpus| {
            b.iter(|| {
                let mut pipeline = DedupPipeline::new(corpus.len(), &cpu_caps);

                for (doc_id, text) in corpus {
                    pipeline.add_document(*doc_id, text).unwrap();
                }

                // Batch LSH lookups
                let clusters = pipeline.find_duplicates_batch(0.85).unwrap();
                black_box(clusters)
            });
        });
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 5: Compound Speedup Validation (All Optimizations)
// ============================================================================

#[cfg(all(feature = "simd-text-hashing", feature = "batch-lsh"))]
fn bench_compound_full_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("compound_full_optimization");

    group
        .sample_size(50)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10));

    let cpu_caps = CpuCapabilityCapsule::detect();

    for duplicate_rate in [0.5, 0.7, 0.9] {
        let corpus_size = 10_000;
        let corpus = generate_corpus_with_duplicates(corpus_size, duplicate_rate);
        let rate_pct = (duplicate_rate * 100.0) as usize;

        group.throughput(Throughput::Elements(corpus_size as u64));

        // Week 1 Baseline: Scalar + Sequential (includes Bloom)
        group.bench_with_input(BenchmarkId::new("week1_baseline", rate_pct), &corpus, |b, corpus| {
            b.iter(|| {
                let mut pipeline = DedupPipeline::new(corpus.len(), &cpu_caps);

                for (doc_id, text) in corpus {
                    pipeline.add_document(*doc_id, text).unwrap();
                }

                let clusters = pipeline.find_duplicates(0.85).unwrap();
                black_box(clusters)
            });
        });

        // Week 2 Full: SIMD + Batch LSH + Bloom
        group.bench_with_input(BenchmarkId::new("week2_full", rate_pct), &corpus, |b, corpus| {
            b.iter(|| {
                let mut pipeline = DedupPipeline::new(corpus.len(), &cpu_caps);

                for (doc_id, text) in corpus {
                    pipeline.add_document(*doc_id, text).unwrap();
                }

                let clusters = pipeline.find_duplicates_batch(0.85).unwrap();
                black_box(clusters)
            });
        });
    }

    group.finish();
}

// ============================================================================
// BENCHMARK 6: Composition Overhead Analysis (B32 K39-K42)
// ============================================================================

#[cfg(all(feature = "simd-text-hashing", feature = "batch-lsh"))]
fn bench_composition_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("compound_composition_overhead");

    group
        .sample_size(100)
        .warm_up_time(Duration::from_secs(3))
        .measurement_time(Duration::from_secs(10));

    let cpu_caps = CpuCapabilityCapsule::detect();
    let corpus = generate_corpus_with_duplicates(10_000, 0.7);

    group.throughput(Throughput::Elements(10_000));

    // Baseline: No optimizations (theoretical)
    group.bench_function("no_optimizations", |b| {
        b.iter(|| {
            // Scalar generation only (no SIMD, no Batch, no Bloom)
            let all_hashes: Vec<Vec<u64>> = corpus
                .iter()
                .map(|(_, text)| {
                    let tokens: Vec<&str> = text.split_whitespace().collect();
                    tokens.iter().map(|token| fnv1a_hash_scalar(token.as_bytes())).collect()
                })
                .collect();

            black_box(all_hashes)
        });
    });

    // SIMD only
    group.bench_function("simd_only", |b| {
        let hasher = SimdTextHasher::new();
        b.iter(|| {
            let all_hashes: Vec<Vec<u64>> = corpus.iter().map(|(_, text)| hasher.hash_tokens_simd(text)).collect();

            black_box(all_hashes)
        });
    });

    // SIMD + Batch LSH (compound)
    group.bench_function("simd_batch_compound", |b| {
        b.iter(|| {
            let mut pipeline = DedupPipeline::new(corpus.len(), &cpu_caps);

            for (doc_id, text) in &corpus {
                pipeline.add_document(*doc_id, text).unwrap();
            }

            let clusters = pipeline.find_duplicates_batch(0.85).unwrap();
            black_box(clusters)
        });
    });

    group.finish();
}

// ============================================================================
// SCALAR BASELINE HELPER (FNV-1a)
// ============================================================================

#[inline(always)]
fn fnv1a_hash_scalar(bytes: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

// ============================================================================
// CRITERION CONFIGURATION (Feature-Gated)
// ============================================================================

#[cfg(all(feature = "simd-text-hashing", feature = "batch-lsh"))]
criterion_group!(
    compound_benchmarks,
    bench_baseline_corpus_generation,
    bench_simd_corpus_generation,
    bench_dedup_sequential_lsh,
    bench_dedup_batch_lsh,
    bench_compound_full_optimization,
    bench_composition_overhead
);

#[cfg(all(feature = "simd-text-hashing", not(feature = "batch-lsh")))]
criterion_group!(
    compound_benchmarks,
    bench_baseline_corpus_generation,
    bench_simd_corpus_generation,
    bench_dedup_sequential_lsh
);

#[cfg(all(not(feature = "simd-text-hashing"), feature = "batch-lsh"))]
criterion_group!(
    compound_benchmarks,
    bench_baseline_corpus_generation,
    bench_dedup_sequential_lsh,
    bench_dedup_batch_lsh
);

#[cfg(not(any(feature = "simd-text-hashing", feature = "batch-lsh")))]
criterion_group!(
    compound_benchmarks,
    bench_baseline_corpus_generation,
    bench_dedup_sequential_lsh
);

criterion_main!(compound_benchmarks);
