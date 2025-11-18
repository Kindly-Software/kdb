//! 10M Document T5 Streaming Benchmark ONLY (no sequential baseline)
//!
//! This is a debug version to isolate the T5 streaming performance issue
//! and avoid the sequential baseline which adds 10+ minutes

use kindly_dedup::{StreamingDedupPipeline, generate_synthetic_corpus};
use std::time::Instant;

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║   10M Document T5 Streaming Benchmark (T5 ONLY, no seq)    ║");
    println!("║            Debug version for troubleshooting               ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    // Generate corpus
    println!("[Phase 1: Corpus Generation]");
    let corpus_start = Instant::now();
    let corpus = generate_synthetic_corpus(10_000_000);
    let corpus_time = corpus_start.elapsed();
    println!("✓ Corpus: {} docs in {:.2}s ({:.0} docs/sec)\n",
        corpus.len(),
        corpus_time.as_secs_f64(),
        10_000_000.0 / corpus_time.as_secs_f64()
    );

    let documents: Vec<(usize, String)> = corpus.iter()
        .enumerate()
        .map(|(i, doc)| (i, doc.text.clone()))
        .collect();

    // T5 Pipeline
    println!("[Phase 2: T5 Streaming Pipeline]");
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(16);
    println!("Threads: {}", num_threads);

    println!("Initializing StreamingDedupPipeline...");
    let mut pipeline = match StreamingDedupPipeline::new(10_000_000, num_threads) {
        Ok(p) => {
            println!("✓ Initialized");
            p
        }
        Err(e) => {
            eprintln!("ERROR: Failed to initialize pipeline: {:?}", e);
            return;
        }
    };

    println!("Adding 10M documents...");
    let add_start = Instant::now();
    match pipeline.add_documents(documents.clone()) {
        Ok(_) => {},
        Err(e) => {
            eprintln!("ERROR during add_documents: {:?}", e);
            return;
        }
    }
    let add_time = add_start.elapsed();
    println!("✓ Add phase complete: {:.3}s ({:.0} docs/sec)\n",
        add_time.as_secs_f64(),
        10_000_000.0 / add_time.as_secs_f64()
    );

    println!("[Phase 3: Find Duplicates (T5 Streaming)]");
    println!("This is where the hang occurs - monitoring...");

    let find_start = Instant::now();
    let clusters = match pipeline.find_duplicates(0.85) {
        Ok(c) => {
            let find_time = find_start.elapsed();
            println!("✓ Find phase complete: {:.3}s", find_time.as_secs_f64());
            c
        }
        Err(e) => {
            eprintln!("ERROR during find_duplicates: {:?}", e);
            vec![]
        }
    };

    let find_time = find_start.elapsed();
    let total_time = add_time + find_time;
    let total_throughput = 10_000_000.0 / total_time.as_secs_f64();

    let metrics = pipeline.metrics();
    let bloom_skip_rate = if metrics.documents_ingested > 0 {
        (metrics.documents_skipped as f64 / metrics.documents_ingested as f64) * 100.0
    } else {
        0.0
    };

    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║                   FINAL RESULTS (T5)                       ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    println!("Timing:");
    println!("  Add phase:        {:.3}s ({:.0} docs/sec)",
        add_time.as_secs_f64(),
        10_000_000.0 / add_time.as_secs_f64()
    );
    println!("  Find phase:       {:.3}s ({:.0} docs/sec)",
        find_time.as_secs_f64(),
        10_000_000.0 / find_time.as_secs_f64()
    );
    println!("  TOTAL TIME:       {:.3}s", total_time.as_secs_f64());
    println!("  TOTAL THROUGHPUT: {:.0} docs/sec\n", total_throughput);

    println!("Metrics:");
    println!("  Ingested:         {}", metrics.documents_ingested);
    println!("  Tokenized:        {} ({:.1}%)",
        metrics.documents_tokenized,
        (metrics.documents_tokenized as f64 / metrics.documents_ingested.max(1) as f64) * 100.0
    );
    println!("  Skipped (Bloom):  {} ({:.1}%)",
        metrics.documents_skipped,
        bloom_skip_rate
    );
    println!("  Signatures:       {}", metrics.signatures_computed);
    println!("  Pairs Verified:   {}", metrics.pairs_verified);
    println!("  Clusters Found:   {}\n", clusters.len());

    println!("ASSUM Safety Metrics:");
    let total_panics = metrics.tokenization_panics + metrics.minhash_panics +
                       metrics.lsh_panics + metrics.verification_panics;
    println!("  Tokenization panics: {}", metrics.tokenization_panics);
    println!("  MinHash panics:      {}", metrics.minhash_panics);
    println!("  LSH panics:          {}", metrics.lsh_panics);
    println!("  Verification panics: {}", metrics.verification_panics);
    println!("  TOTAL PANICS:        {}", total_panics);
    if total_panics == 0 {
        println!("  ✓ ASSUM_PANIC_SAFETY: PASSED (100% safe)\n");
    } else {
        println!("  ✗ ASSUM_PANIC_SAFETY: {} panics detected\n", total_panics);
    }

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║          T5 STREAMING BENCHMARK COMPLETE                   ║");
    println!("╚════════════════════════════════════════════════════════════╝");
}
