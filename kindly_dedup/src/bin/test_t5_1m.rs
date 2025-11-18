//! Simple T5 1M benchmark - Direct measurement without Criterion
//!
//! Validates 200-300K docs/sec target with Bloom filter fix

use kindly_dedup::{generate_synthetic_corpus, StreamingDedupPipeline};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== T5 Streaming Pipeline - 1M Document Benchmark ===\n");

    // Generate corpus
    println!("Generating 1M document corpus...");
    let corpus_start = Instant::now();
    let corpus = generate_synthetic_corpus(1_000_000);
    let corpus_time = corpus_start.elapsed();
    println!(
        "  Corpus generated in {:.2}s ({:.0} docs/sec)\n",
        corpus_time.as_secs_f64(),
        1_000_000.0 / corpus_time.as_secs_f64()
    );

    // Convert to (DocId, String) tuples
    let documents: Vec<(usize, String)> = corpus.iter().map(|doc| (doc.id, doc.text.clone())).collect();

    // Create T5 pipeline
    println!("Creating T5 Streaming Pipeline...");
    let mut pipeline = StreamingDedupPipeline::new(1_000_000, 16)?;

    // Benchmark add_documents (Stages 1-4)
    println!("\n--- Stage 1-4: Add Documents ---");
    let add_start = Instant::now();
    pipeline.add_documents(documents.clone())?;
    let add_time = add_start.elapsed();

    let add_throughput = 1_000_000.0 / add_time.as_secs_f64();
    println!("  Time: {:.2}s", add_time.as_secs_f64());
    println!("  Throughput: {:.0} docs/sec", add_throughput);

    // Print metrics
    let metrics = pipeline.metrics();
    println!("\n  Metrics:");
    println!("    Ingested: {}", metrics.documents_ingested);
    println!("    Tokenized: {}", metrics.documents_tokenized);
    println!(
        "    Skipped (Bloom): {} ({:.1}%)",
        metrics.documents_skipped,
        (metrics.documents_skipped as f64 / metrics.documents_ingested as f64) * 100.0
    );
    println!("    Signatures: {}", metrics.signatures_computed);
    println!(
        "    Panics: tok={}, min={}, lsh={}, ver={}",
        metrics.tokenization_panics, metrics.minhash_panics, metrics.lsh_panics, metrics.verification_panics
    );

    // Benchmark find_duplicates (Stage 5)
    println!("\n--- Stage 5: Find Duplicates ---");
    let find_start = Instant::now();
    let clusters = pipeline.find_duplicates(0.85)?;
    let find_time = find_start.elapsed();

    println!("  Time: {:.2}s", find_time.as_secs_f64());
    println!("  Clusters found: {}", clusters.len());

    // End-to-end
    let total_time = add_time + find_time;
    let total_throughput = 1_000_000.0 / total_time.as_secs_f64();

    println!("\n=== End-to-End Results ===");
    println!("  Total time: {:.2}s", total_time.as_secs_f64());
    println!("  Throughput: {:.0} docs/sec", total_throughput);

    // Validation
    println!("\n=== Validation ===");
    let baseline = 39_788.0; // Measured sequential baseline
    let speedup = total_throughput / baseline;
    println!("  Sequential baseline: {:.0} docs/sec", baseline);
    println!("  Speedup: {:.2}× vs baseline", speedup);

    // Target validation
    let target_min = 200_000.0;
    let target_max = 300_000.0;
    let target_met = total_throughput >= target_min;

    println!(
        "\n  Target: {:.0}-{:.0} docs/sec (3.3-5× speedup)",
        target_min, target_max
    );
    if target_met {
        println!("  ✅ TARGET MET: {:.0} docs/sec", total_throughput);
    } else {
        println!(
            "  ⚠️  Below target: {:.0} docs/sec (need {:.0}+)",
            total_throughput, target_min
        );
    }

    // Bloom filter validation
    let expected_skip_rate = 0.25; // 25% duplicates in corpus
    let actual_skip_rate = metrics.documents_skipped as f64 / metrics.documents_ingested as f64;
    let bloom_working = actual_skip_rate >= 0.15 && actual_skip_rate <= 0.35; // ±10% tolerance

    println!("\n  Bloom filter:");
    println!(
        "    Expected skip: ~25% ({} docs)",
        (1_000_000.0 * expected_skip_rate) as usize
    );
    println!(
        "    Actual skip: {:.1}% ({} docs)",
        actual_skip_rate * 100.0,
        metrics.documents_skipped
    );
    if bloom_working {
        println!("    ✅ Bloom filter working correctly");
    } else {
        println!("    ⚠️  Bloom skip rate outside expected range");
    }

    // Safety validation
    let panics_total =
        metrics.tokenization_panics + metrics.minhash_panics + metrics.lsh_panics + metrics.verification_panics;
    if panics_total == 0 {
        println!("\n  ✅ Zero panics (100% reliability)");
    } else {
        println!("\n  ⚠️  {} panics detected", panics_total);
    }

    Ok(())
}
