//! Profile-specific example: Focus on find_duplicates phase only
//!
//! This example:
//! 1. Generates 100K documents
//! 2. Adds them to StreamingDedupPipeline
//! 3. Runs ONLY find_duplicates (the problematic phase)
//! 4. Exits immediately after
//!
//! Usage:
//!   cargo build --release --example profile_find_duplicates
//!   sudo flamegraph --output /tmp/finding_duplicates.svg -- ./target/release/examples/profile_find_duplicates

use kindly_dedup::{generate_synthetic_corpus, StreamingDedupPipeline};
use std::time::Instant;

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║   Profiling find_duplicates() - 100K Documents            ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    // Generate smaller corpus (100K to keep profiling fast)
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

    // Add documents (NOT the profiling target)
    println!("[Phase 3: Adding Documents]");
    let add_start = Instant::now();
    pipeline.add_documents(documents).unwrap();
    println!("✓ Documents added in {:.3}s\n", add_start.elapsed().as_secs_f64());

    // PROFILING TARGET: find_duplicates
    println!("[Phase 4: Finding Duplicates - PROFILING TARGET]");
    println!("Starting find_duplicates (this is what we're profiling)...");

    let find_start = Instant::now();
    let clusters = match pipeline.find_duplicates(0.85) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ERROR during find_duplicates: {:?}", e);
            std::process::exit(1);
        }
    };
    let find_time = find_start.elapsed();

    println!("✓ find_duplicates completed in {:.3}s", find_time.as_secs_f64());
    println!("  Clusters found: {}", clusters.len());

    // Print metrics
    let metrics = pipeline.metrics();
    println!("\n[Metrics]");
    println!("  Pairs verified: {}", metrics.pairs_verified);
    println!(
        "  Panics: tok={}, min={}, lsh={}, ver={}",
        metrics.tokenization_panics, metrics.minhash_panics, metrics.lsh_panics, metrics.verification_panics
    );

    println!("\n✅ Profiling target completed successfully");
}
