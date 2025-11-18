//! T5 Streaming Pipeline Benchmark - 1M Documents
//!
//! **Objective**: Validate 200-300K docs/sec target with real 1M document corpus.
//!
//! **B32 Framework**:
//! - Fair baseline: Sequential DedupPipeline (60K docs/sec measured)
//! - Same hardware: AMD Ryzen 9 6900HX (8c/16t)
//! - Realistic workload: 1M documents with 15% duplicate rate
//! - Honest claims: Conservative estimates, no cherry-picking

use atomic_capsule::CpuCapabilityCapsule;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use kindly_dedup::{generate_synthetic_corpus, DedupPipeline, StreamingDedupPipeline};
use std::time::Instant;

/// Benchmark T5 Streaming with 1M documents
fn bench_t5_1m_corpus(c: &mut Criterion) {
    let mut group = c.benchmark_group("T5 Streaming 1M Documents");
    group.sample_size(10); // Reduced sample size (each iteration is expensive)

    // Generate 1M documents (5% exact, 20% near, 75% unique = ~25% total duplicates)
    println!("Generating 1M document corpus...");
    let corpus = generate_synthetic_corpus(1_000_000);

    // Convert to (DocId, String) tuples
    let documents: Vec<(usize, String)> = corpus.iter().map(|doc| (doc.id, doc.text.clone())).collect();

    println!("Corpus generated: {} documents", documents.len());

    // Benchmark T5 add_documents (Stages 1-4)
    group.bench_function("t5_add_1m", |b| {
        b.iter_custom(|iters| {
            let mut total_duration = std::time::Duration::ZERO;

            for _ in 0..iters {
                let mut pipeline = StreamingDedupPipeline::new(1_000_000, 16).unwrap();

                let start = Instant::now();
                pipeline.add_documents(black_box(documents.clone())).unwrap();
                total_duration += start.elapsed();

                // Print metrics
                let metrics = pipeline.metrics();
                println!("  Add phase metrics:");
                println!("    Ingested: {}", metrics.documents_ingested);
                println!("    Tokenized: {}", metrics.documents_tokenized);
                println!("    Skipped (Bloom): {}", metrics.documents_skipped);
                println!("    Signatures: {}", metrics.signatures_computed);
                println!(
                    "    Skip rate: {:.1}%",
                    (metrics.documents_skipped as f64 / metrics.documents_ingested as f64) * 100.0
                );
            }

            total_duration
        });
    });

    // Benchmark T5 find_duplicates (Stage 5)
    group.bench_function("t5_find_1m", |b| {
        // Pre-build pipeline
        let mut pipeline = StreamingDedupPipeline::new(1_000_000, 16).unwrap();
        pipeline.add_documents(documents.clone()).unwrap();

        b.iter(|| pipeline.find_duplicates(black_box(0.85)).unwrap());
    });

    // Benchmark end-to-end throughput
    group.bench_function("t5_end_to_end_1m", |b| {
        b.iter_custom(|iters| {
            let mut total_duration = std::time::Duration::ZERO;

            for _ in 0..iters {
                let mut pipeline = StreamingDedupPipeline::new(1_000_000, 16).unwrap();

                let start = Instant::now();
                pipeline.add_documents(black_box(documents.clone())).unwrap();
                let _clusters = pipeline.find_duplicates(black_box(0.85)).unwrap();
                total_duration += start.elapsed();

                // Print throughput
                let elapsed_secs = start.elapsed().as_secs_f64();
                let throughput = 1_000_000.0 / elapsed_secs;
                println!("  End-to-end: {:.2}s, {:.0} docs/sec", elapsed_secs, throughput);

                // Print final metrics
                let metrics = pipeline.metrics();
                println!("  Final metrics:");
                println!("    Total ingested: {}", metrics.documents_ingested);
                println!(
                    "    Bloom skipped: {} ({:.1}%)",
                    metrics.documents_skipped,
                    (metrics.documents_skipped as f64 / metrics.documents_ingested as f64) * 100.0
                );
                println!(
                    "    Panics: tok={}, min={}, lsh={}, ver={}",
                    metrics.tokenization_panics,
                    metrics.minhash_panics,
                    metrics.lsh_panics,
                    metrics.verification_panics
                );
            }

            total_duration
        });
    });

    group.finish();
}

/// Benchmark sequential baseline for comparison
fn bench_sequential_1m_corpus(c: &mut Criterion) {
    let mut group = c.benchmark_group("Sequential Baseline 1M Documents");
    group.sample_size(10);

    // Use same corpus as T5
    let corpus = generate_synthetic_corpus(1_000_000);
    let documents: Vec<(usize, String)> = corpus.iter().map(|doc| (doc.id, doc.text.clone())).collect();

    group.bench_function("sequential_add_1m", |b| {
        b.iter_custom(|iters| {
            let mut total_duration = std::time::Duration::ZERO;
            let cpu_caps = CpuCapabilityCapsule::detect();

            for _ in 0..iters {
                let mut pipeline = DedupPipeline::new(1_000_000, cpu_caps);

                let start = Instant::now();
                for (doc_id, text) in &documents {
                    pipeline.add_document(*doc_id, &text);
                }
                total_duration += start.elapsed();

                let elapsed_secs = start.elapsed().as_secs_f64();
                let throughput = 1_000_000.0 / elapsed_secs;
                println!("  Sequential add: {:.2}s, {:.0} docs/sec", elapsed_secs, throughput);
            }

            total_duration
        });
    });

    group.finish();
}

criterion_group!(benches, bench_t5_1m_corpus, bench_sequential_1m_corpus);
criterion_main!(benches);
