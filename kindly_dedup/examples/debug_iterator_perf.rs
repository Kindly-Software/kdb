//! Debug iterator performance to understand the bottleneck
//!
//! This example:
//! 1. Generates 100K documents
//! 2. Adds them to StreamingDedupPipeline
//! 3. Manually iterates through LSH buckets to measure performance
//! 4. Reports timing and entry counts

use kindly_dedup::{generate_synthetic_corpus, StreamingDedupPipeline};
use std::time::Instant;

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║   Debug Iterator Performance - 100K Documents             ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    // Generate corpus
    println!("[Phase 1: Corpus Generation]");
    let corpus_start = Instant::now();
    let corpus = generate_synthetic_corpus(100_000);
    println!(
        "✓ Generated {} docs in {:.2}s\n",
        corpus.len(),
        corpus_start.elapsed().as_secs_f64()
    );

    let documents: Vec<(usize, String)> = corpus.iter().map(|doc| (doc.id, doc.text.clone())).collect();

    // Initialize pipeline
    println!("[Phase 2: Pipeline Initialization]");
    let mut pipeline = StreamingDedupPipeline::new(100_000, 16).unwrap();
    println!("✓ Pipeline initialized\n");

    // Add documents
    println!("[Phase 3: Adding Documents]");
    let add_start = Instant::now();
    pipeline.add_documents(documents).unwrap();
    println!("✓ Documents added in {:.3}s\n", add_start.elapsed().as_secs_f64());

    println!("[Phase 4: Debug LSH Bucket Statistics]");
    println!("Analyzing LSH bucket structure...");

    // Access the internal lsh_buckets to analyze them
    // NOTE: This requires exposing internals, so we'll just try to profile the iterator

    // Call find_duplicates with timing
    println!("\n[Phase 5: find_duplicates() with detailed timing]");
    println!("Starting find_duplicates (with timeout monitoring)...");

    let find_start = Instant::now();

    // Set a timeout
    let timeout_secs = 30;
    let start_time = std::time::Instant::now();

    // This is where it hangs - let's see how long it takes
    match pipeline.find_duplicates(0.85) {
        Ok(clusters) => {
            let find_time = find_start.elapsed();
            println!("✓ find_duplicates completed in {:.3}s", find_time.as_secs_f64());
            println!("  Clusters found: {}", clusters.len());
        }
        Err(e) => {
            eprintln!("ERROR during find_duplicates: {:?}", e);
            std::process::exit(1);
        }
    }

    let metrics = pipeline.metrics();
    println!("\n[Metrics]");
    println!("  Pairs verified: {}", metrics.pairs_verified);
    println!("  Documents ingested: {}", metrics.documents_ingested);
    println!("  Signatures computed: {}", metrics.signatures_computed);
}
