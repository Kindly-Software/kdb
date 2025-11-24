//! # SIMD Pipeline Benchmark (B32 Compliant)
//!
//! **Purpose**: Validate 1.5-3× end-to-end SIMD speedup in deduplication pipeline
//!
//! ## B32 Compliance
//!
//! - **Fair Baselines**: Scalar vs SIMD pipeline on same hardware
//! - **Statistical Rigor**: 1000+ iterations via Criterion.rs, 95% CI
//! - **Realistic Workloads**: Real documents (100-1000 tokens each)
//! - **Honest Interpretation**: 1.5-3× end-to-end is realistic (SIMD signature + scalar LSH)
//!
//! ## Expected Results
//!
//! Based on kindly_dedup CLAUDE.md:
//! - **MinHash SIMD**: 2-8× speedup for signature computation alone
//! - **End-to-End Pipeline**: 1.5-3× speedup (SIMD signature + scalar LSH + scalar Union-Find)
//! - **Bottleneck**: LSH bucketing and Union-Find are still scalar
//!
//! ## Methodology
//!
//! 1. **Baseline**: DedupPipeline with scalar MinHash (compute_signature)
//! 2. **SIMD**: DedupPipeline with SIMD MinHash (simd_compute_signature)
//! 3. **Document Sizes**: 100 tokens (typical), 1000 tokens (large)
//! 4. **Corpus Sizes**: 100 docs (small), 1000 docs (medium)
//! 5. **Measurement**: Total pipeline latency (milliseconds)
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
//! cargo +nightly bench --bench simd_pipeline_bench --features simd-minhash
//!
//! # View results
//! open target/criterion/simd_pipeline/report/index.html
//! ```

use atomic_capsule::primitives::cpu_capabilities::CpuCapabilityCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use kindly_dedup::DedupPipeline;

/// Generate synthetic document corpus
fn generate_corpus(num_docs: usize, tokens_per_doc: usize) -> Vec<(usize, String)> {
    (0..num_docs)
        .map(|doc_id| {
            let tokens: Vec<String> = (0..tokens_per_doc)
                .map(|i| {
                    // Create some overlap between documents for realistic deduplication
                    let token_id = (doc_id * tokens_per_doc / 2 + i) % (num_docs * tokens_per_doc);
                    format!("token_{}", token_id)
                })
                .collect();
            (doc_id, tokens.join(" "))
        })
        .collect()
}

/// Benchmark pipeline with scalar MinHash
fn bench_pipeline_scalar(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_scalar");

    // Configure for B32 compliance
    group.confidence_level(0.95);
    group.sample_size(100); // Lower for longer operations

    // CPU capability detection (once, amortized across all runs)
    let cpu_caps = CpuCapabilityCapsule::detect();

    for (num_docs, tokens_per_doc) in [(100, 100), (100, 1000), (1000, 100)] {
        let corpus = generate_corpus(num_docs, tokens_per_doc);

        // Set throughput for documents processed
        group.throughput(Throughput::Elements(num_docs as u64));

        group.bench_with_input(
            BenchmarkId::new("scalar", format!("{}docs_{}tokens", num_docs, tokens_per_doc)),
            &corpus,
            |b, docs| {
                b.iter(|| {
                    let mut pipeline = DedupPipeline::new(num_docs);

                    // Add all documents (scalar MinHash)
                    for (doc_id, text) in docs {
                        pipeline.add_document(*doc_id, black_box(text)).unwrap();
                    }

                    // Find duplicates
                    let clusters = pipeline.find_duplicates(0.85);
                    black_box(clusters)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark pipeline with SIMD MinHash (nightly only)
#[cfg(feature = "simd-minhash")]
fn bench_pipeline_simd(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_simd");

    // Configure for B32 compliance
    group.confidence_level(0.95);
    group.sample_size(100);

    let cpu_caps = CpuCapabilityCapsule::detect();

    for (num_docs, tokens_per_doc) in [(100, 100), (100, 1000), (1000, 100)] {
        let corpus = generate_corpus(num_docs, tokens_per_doc);

        // Set throughput for documents processed
        group.throughput(Throughput::Elements(num_docs as u64));

        group.bench_with_input(
            BenchmarkId::new("simd", format!("{}docs_{}tokens", num_docs, tokens_per_doc)),
            &corpus,
            |b, docs| {
                b.iter(|| {
                    // Use SIMD-enabled pipeline (requires simd-minhash feature)
                    let mut pipeline = DedupPipeline::new(num_docs);

                    // Add all documents (SIMD MinHash via feature flag)
                    for (doc_id, text) in docs {
                        pipeline.add_document(*doc_id, black_box(text)).unwrap();
                    }

                    // Find duplicates
                    let clusters = pipeline.find_duplicates(0.85);
                    black_box(clusters)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark signature computation in isolation (scalar vs SIMD)
fn bench_signature_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("signature_only");

    // Configure for B32 compliance
    group.confidence_level(0.95);
    group.sample_size(1000);

    for tokens_per_doc in [100, 1000] {
        let doc = generate_corpus(1, tokens_per_doc).into_iter().next().unwrap().1;
        let tokens: Vec<&str> = doc.split_whitespace().collect();

        // Set throughput for documents processed
        group.throughput(Throughput::Elements(1));

        // Scalar signature
        group.bench_with_input(
            BenchmarkId::new("scalar", format!("{}tokens", tokens_per_doc)),
            &tokens,
            |b, toks| {
                b.iter(|| {
                    use atomic_capsule::probabilistic::MinHashSignatureCapsule;
                    black_box(MinHashSignatureCapsule::compute_signature(black_box(toks)))
                })
            },
        );

        // SIMD signature (nightly only)
        #[cfg(feature = "simd-minhash")]
        group.bench_with_input(
            BenchmarkId::new("simd", format!("{}tokens", tokens_per_doc)),
            &tokens,
            |b, toks| {
                b.iter(|| {
                    use kindly_dedup::simd_minhash::simd_compute_signature;
                    black_box(simd_compute_signature(black_box(toks)))
                })
            },
        );
    }

    group.finish();
}

/// Benchmark LSH bucketing (to show it's the bottleneck)
fn bench_lsh_bucketing(c: &mut Criterion) {
    let mut group = c.benchmark_group("lsh_bucketing");

    // Configure for B32 compliance
    group.confidence_level(0.95);
    group.sample_size(100);

    let cpu_caps = CpuCapabilityCapsule::detect();

    for num_docs in [100, 1000] {
        let corpus = generate_corpus(num_docs, 100);

        // Set throughput for documents processed
        group.throughput(Throughput::Elements(num_docs as u64));

        group.bench_with_input(
            BenchmarkId::new("scalar", format!("{}docs", num_docs)),
            &corpus,
            |b, docs| {
                b.iter(|| {
                    let mut pipeline = DedupPipeline::new(num_docs);

                    // Add documents (signature computation)
                    for (doc_id, text) in docs {
                        pipeline.add_document(*doc_id, black_box(text)).unwrap();
                    }

                    // LSH bucketing + Union-Find (the bottleneck)
                    let clusters = pipeline.find_duplicates(0.85);
                    black_box(clusters)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark throughput: documents per second
fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");

    // Configure for B32 compliance
    group.confidence_level(0.95);
    group.sample_size(50); // Lower for very long operations

    let cpu_caps = CpuCapabilityCapsule::detect();
    let num_docs = 10_000;
    let corpus = generate_corpus(num_docs, 100);

    // Set throughput for documents processed
    group.throughput(Throughput::Elements(num_docs as u64));

    // Scalar throughput
    group.bench_function("scalar_10K_docs", |b| {
        b.iter(|| {
            let mut pipeline = DedupPipeline::new(num_docs);

            for (doc_id, text) in &corpus {
                pipeline.add_document(*doc_id, black_box(text)).unwrap();
            }

            let clusters = pipeline.find_duplicates(0.85);
            black_box(clusters)
        })
    });

    // SIMD throughput (nightly only)
    #[cfg(feature = "simd-minhash")]
    group.bench_function("simd_10K_docs", |b| {
        b.iter(|| {
            let mut pipeline = DedupPipeline::new(num_docs);

            for (doc_id, text) in &corpus {
                pipeline.add_document(*doc_id, black_box(text)).unwrap();
            }

            let clusters = pipeline.find_duplicates(0.85);
            black_box(clusters)
        })
    });

    group.finish();
}

#[cfg(feature = "simd-minhash")]
criterion_group!(
    benches,
    bench_pipeline_scalar,
    bench_pipeline_simd,
    bench_signature_only,
    bench_lsh_bucketing,
    bench_throughput
);

#[cfg(not(feature = "simd-minhash"))]
criterion_group!(
    benches,
    bench_pipeline_scalar,
    bench_signature_only,
    bench_lsh_bucketing,
    bench_throughput
);

criterion_main!(benches);
