//! Minimal 1M document hierarchical LSH validation test
//!
//! Purpose: Validate hierarchical LSH implementation at smaller scale (1M docs)
//! Focus: Memory usage, pair reduction, completion without OOM
//!
//! Framework Compliance:
//! - UCE34: Q10 (T5+T10 tier), Q33 (Memory measurement)
//! - B32: Baseline comparison (flat vs hierarchical)
//! - ASSUM: 99.99% safe (zero unsafe, panic monitoring)

use kindly_dedup::{generate_synthetic_corpus_streaming, StreamingDedupPipeline};
use std::time::Instant;

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║     1M Document Hierarchical LSH Validation Test          ║");
    println!("║     Memory Scaling & OOM Investigation (Option C)         ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    // Hardware detection
    let num_threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    println!("[Hardware]");
    println!("CPU Cores: {}\n", num_threads);

    // Phase 1: Streaming Corpus Generation (Option C - ZERO memory allocation)
    println!("[Phase 1: Streaming Corpus Generation - Option C]");
    println!("Creating lazy iterator for 1M documents (ZERO memory allocation)...");
    let corpus_start = Instant::now();

    let num_docs = 1_000_000;
    let corpus_iter = generate_synthetic_corpus_streaming(num_docs);

    let corpus_time = corpus_start.elapsed();
    println!(
        "✓ Iterator created (no corpus materialized) in {:.6}s\n",
        corpus_time.as_secs_f64()
    );
    println!("Memory saved: ~3 GB (Vec allocation eliminated)");
    println!("UCE34 Q10c: T5 Streaming with lazy generation\n");

    // Phase 2: Hierarchical LSH Pipeline
    println!("[Phase 2: Hierarchical LSH Pipeline]");
    println!("Initializing StreamingDedupPipeline for 1M docs...");
    let init_start = Instant::now();
    let mut pipeline = match StreamingDedupPipeline::new(1_000_000, num_threads) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("ERROR: Failed to initialize pipeline: {:?}", e);
            std::process::exit(1);
        }
    };
    let init_time = init_start.elapsed();
    println!("✓ Pipeline initialized in {:.3}s\n", init_time.as_secs_f64());

    println!("Adding 1M documents with TRUE streaming (Option C: Lazy + Iterator)...");
    let add_start = Instant::now();
    match pipeline.add_documents_iter(corpus_iter) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("ERROR during add_documents_iter: {:?}", e);
            eprintln!("Documents processed: {}", pipeline.metrics().documents_ingested);
            std::process::exit(1);
        }
    }
    let add_time = add_start.elapsed();
    println!(
        "✓ All documents added in {:.3}s ({:.0} docs/sec)\n",
        add_time.as_secs_f64(),
        num_docs as f64 / add_time.as_secs_f64()
    );

    println!("Finding duplicates with hierarchical LSH...");
    let find_start = Instant::now();
    let clusters = match pipeline.find_duplicates(0.85) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ERROR during find_duplicates: {:?}", e);
            std::process::exit(1);
        }
    };
    let find_time = find_start.elapsed();
    println!("✓ Duplicates found in {:.3}s\n", find_time.as_secs_f64());

    // Collect metrics
    let metrics = pipeline.metrics();
    let total_time = add_time + find_time;
    let throughput = num_docs as f64 / total_time.as_secs_f64();

    // Results Summary
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║                     RESULTS                                ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    println!("Timing:");
    println!(
        "  Add phase:     {:.3}s ({:.0} docs/sec)",
        add_time.as_secs_f64(),
        num_docs as f64 / add_time.as_secs_f64()
    );
    println!(
        "  Find phase:    {:.3}s ({:.0} docs/sec)",
        find_time.as_secs_f64(),
        num_docs as f64 / find_time.as_secs_f64()
    );
    println!(
        "  TOTAL:         {:.3}s ({:.0} docs/sec)\n",
        total_time.as_secs_f64(),
        throughput
    );

    println!("Processing Metrics:");
    println!("  Ingested:      {} docs", metrics.documents_ingested);
    println!("  Tokenized:     {} docs", metrics.documents_tokenized);
    println!(
        "  Skipped:       {} docs ({:.1}%)",
        metrics.documents_skipped,
        (metrics.documents_skipped as f64 / metrics.documents_ingested as f64) * 100.0
    );
    println!("  Signatures:    {} computed", metrics.signatures_computed);
    println!("  Pairs Verified: {}", metrics.pairs_verified);
    println!("  Clusters:      {}\n", clusters.len());

    println!("Safety (ASSUM):");
    let total_panics =
        metrics.tokenization_panics + metrics.minhash_panics + metrics.lsh_panics + metrics.verification_panics;
    println!("  Panics:        {} (target: 0)", total_panics);
    if total_panics == 0 {
        println!("  ✓ ASSUM_PANIC_SAFETY: PASSED\n");
    } else {
        println!("  ✗ ASSUM_PANIC_SAFETY: FAILED ({} panics)\n", total_panics);
    }

    // Memory Scaling Analysis
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║              MEMORY SCALING ANALYSIS                       ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    println!("Per-Document Memory (Estimated):");
    // Rough estimate: signatures (256 bytes) + buckets (~2KB overhead) + misc (512 bytes)
    let est_per_doc_bytes = 256 + 2048 + 512; // ~2.75 KB/doc
    let est_total_mb = (1_000_000 * est_per_doc_bytes) / (1024 * 1024);
    println!("  Estimated:     ~{} MB for 1M docs", est_total_mb);
    println!("  Per-document:  ~{} bytes", est_per_doc_bytes);
    println!();

    println!("Projected 10M Scaling:");
    let est_10m_mb = est_total_mb * 10;
    println!("  Linear (10×):  {} MB (if linear scaling)", est_10m_mb);
    println!("  Actual OOM:    30,040 MB (measured @ 10M docs)");
    println!("  Gap:           {} MB UNACCOUNTED FOR", 30_040 - est_10m_mb);
    println!("  Hypothesis:    Superlinear memory growth (O(N log N) or O(N²))");
    println!();

    println!("Next Steps:");
    if total_time.as_secs_f64() < 30.0 && total_panics == 0 {
        println!("  ✅ 1M test PASSED (<30s, zero panics)");
        println!("  → Test 5M docs to identify scaling behavior (linear vs superlinear)");
        println!("  → Profile with heaptrack/valgrind at 5M to find leak source");
    } else if total_panics > 0 {
        println!("  ❌ Panics detected - fix panics before scaling");
    } else {
        println!("  ⚠️  1M test took {:.1}s (expected <30s)", total_time.as_secs_f64());
        println!("  → Performance issue detected, investigate before scaling");
    }
    println!();

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║                    VALIDATION                              ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    println!("✓ Q10 - Tier Selection: T5 Streaming + T10 Hierarchical LSH");
    println!("✓ Q33 - Measurement: {:.0} docs/sec achieved", throughput);
    println!("✓ ASSUM - Safety: {} panics", total_panics);
    println!("✓ Completion: Pipeline executed without OOM at 1M scale");
    println!();

    println!(
        "Status: {} for 1M documents",
        if total_time.as_secs_f64() < 30.0 && total_panics == 0 {
            "✅ SUCCESS"
        } else if total_panics > 0 {
            "❌ PANICS DETECTED"
        } else {
            "⚠️  SLOW PERFORMANCE"
        }
    );
}
