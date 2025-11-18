//! Simple 1M document T5 benchmark (non-Criterion)

use atomic_capsule::CpuCapabilityCapsule;
use kindly_dedup::{generate_synthetic_corpus, DedupPipeline, StreamingDedupPipeline};
use std::time::Instant;

fn main() {
    // Generate corpus
    println!("Generating 1M document corpus...");
    let corpus_start = Instant::now();
    let corpus = generate_synthetic_corpus(1_000_000);
    println!("  Corpus generation: {:.2}s", corpus_start.elapsed().as_secs_f64());

    let documents: Vec<(usize, String)> = corpus.iter().map(|doc| (doc.id, doc.text.clone())).collect();

    println!("\n=== T5 Streaming Pipeline (16 threads) ===");

    // T5 Benchmark
    let mut pipeline = StreamingDedupPipeline::new(1_000_000, 16).unwrap();

    let start = Instant::now();
    pipeline.add_documents(documents.clone()).unwrap();
    let add_time = start.elapsed();

    let start_find = Instant::now();
    let clusters = pipeline.find_duplicates(0.85).unwrap();
    let find_time = start_find.elapsed();

    let metrics = pipeline.metrics();
    let total_time = add_time + find_time;
    let throughput = 1_000_000.0 / total_time.as_secs_f64();

    println!(
        "Add phase: {:.3}s ({:.0} docs/sec)",
        add_time.as_secs_f64(),
        1_000_000.0 / add_time.as_secs_f64()
    );
    println!(
        "Find phase: {:.3}s ({:.0} docs/sec)",
        find_time.as_secs_f64(),
        1_000_000.0 / find_time.as_secs_f64()
    );
    println!("Total: {:.3}s", total_time.as_secs_f64());
    println!("Throughput: {:.0} docs/sec", throughput);
    println!("Clusters: {}", clusters.len());
    println!("\nMetrics:");
    println!("  Ingested: {}", metrics.documents_ingested);
    println!("  Tokenized: {}", metrics.documents_tokenized);
    println!(
        "  Skipped (Bloom): {} ({:.1}%)",
        metrics.documents_skipped,
        (metrics.documents_skipped as f64 / metrics.documents_ingested as f64) * 100.0
    );
    println!("  Signatures: {}", metrics.signatures_computed);
    println!("  Pairs verified: {}", metrics.pairs_verified);
    println!(
        "  Panics: tok={}, min={}, lsh={}, ver={}",
        metrics.tokenization_panics, metrics.minhash_panics, metrics.lsh_panics, metrics.verification_panics
    );

    println!("\n=== Sequential Baseline (1 thread) ===");

    // Sequential benchmark
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut seq_pipeline = DedupPipeline::new(1_000_000, cpu_caps);

    let seq_start = Instant::now();
    for (doc_id, text) in &documents {
        let _ = seq_pipeline.add_document(*doc_id, &text);
    }
    let seq_add_time = seq_start.elapsed();

    let seq_find_start = Instant::now();
    let seq_clusters = seq_pipeline.find_duplicates(0.85).unwrap();
    let seq_find_time = seq_find_start.elapsed();

    let seq_total = seq_add_time + seq_find_time;
    let seq_throughput = 1_000_000.0 / seq_total.as_secs_f64();

    println!(
        "Add phase: {:.3}s ({:.0} docs/sec)",
        seq_add_time.as_secs_f64(),
        1_000_000.0 / seq_add_time.as_secs_f64()
    );
    println!(
        "Find phase: {:.3}s ({:.0} docs/sec)",
        seq_find_time.as_secs_f64(),
        1_000_000.0 / seq_find_time.as_secs_f64()
    );
    println!("Total: {:.3}s", seq_total.as_secs_f64());
    println!("Throughput: {:.0} docs/sec", seq_throughput);
    println!("Clusters: {}", seq_clusters.len());

    println!("\n=== Speedup Analysis ===");
    let speedup = throughput / seq_throughput;
    println!("T5 vs Sequential: {:.2}× speedup", speedup);
    println!("Parallel efficiency: {:.1}%", (speedup / 16.0) * 100.0);

    // B32 Classification
    let classification = if speedup >= 3.3 && speedup <= 5.0 {
        "✅ TARGET MET (3.3-5.0× expected)"
    } else if speedup > 5.0 {
        "🎉 EXCEPTIONAL (exceeded 5.0× target)"
    } else {
        "⚠️  BELOW TARGET (< 3.3× speedup)"
    };
    println!("\nB32 Classification: {}", classification);
}
