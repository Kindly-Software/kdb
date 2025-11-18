//! Memory profiling for 10M document T5 benchmark using DHAT
//!
//! This is a modified version of t5_10m_benchmark.rs with DHAT memory profiling
//! enabled to identify memory bottlenecks.
//!
//! Usage:
//!   cargo run --release --example t5_10m_memory_profile
//!
//! Output:
//!   - Console output showing benchmark progress
//!   - dhat-heap.json file for analysis
//!   - Viewer will open in browser automatically

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use kindly_dedup::{StreamingDedupPipeline, generate_synthetic_corpus};
use atomic_capsule::CpuCapabilityCapsule;
use std::time::Instant;

fn main() {
    let _profiler = dhat::Profiler::new_heap();

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║        10M Document Memory Profile (DHAT)                  ║");
    println!("║        Identifying 26 GB Memory Bottleneck                 ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    // Hardware detection
    println!("[Hardware Detection]");
    let cpu_caps = CpuCapabilityCapsule::detect();
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    println!("CPU Cores: {}", num_threads);
    println!("SIMD: {}", if cpu_caps.has_avx2() { "AVX2" } else if cpu_caps.has_sse42() { "SSE4.2" } else { "Scalar" });
    println!();

    // Phase 1: Corpus Generation
    println!("[Phase 1: Corpus Generation]");
    println!("Generating 10M document corpus (may take 2-5 minutes)...");
    let corpus_start = Instant::now();
    let corpus = generate_synthetic_corpus(10_000_000);
    let corpus_time = corpus_start.elapsed();
    println!("✓ Corpus generated: {} docs in {:.2}s", corpus.len(), corpus_time.as_secs_f64());
    println!("  Memory checkpoint: Post-corpus generation\n");

    // Convert to (id, text) tuples
    let documents: Vec<(usize, String)> = corpus.iter()
        .map(|doc| (doc.id, doc.text.clone()))
        .collect();

    println!("  Memory checkpoint: Post-document conversion\n");

    // Phase 2: T5 Streaming Pipeline Benchmark
    println!("[Phase 2: T5 Streaming Pipeline (16 threads)]");
    println!("Initializing StreamingDedupPipeline...");
    let mut pipeline = match StreamingDedupPipeline::new(10_000_000, num_threads) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("ERROR: Failed to initialize pipeline: {:?}", e);
            std::process::exit(1);
        }
    };

    println!("  Memory checkpoint: Post-pipeline initialization\n");

    println!("Adding 10M documents...");
    let add_start = Instant::now();
    match pipeline.add_documents(documents.clone()) {
        Ok(_) => {},
        Err(e) => {
            eprintln!("ERROR during add_documents: {:?}", e);
            std::process::exit(1);
        }
    }
    let add_time = add_start.elapsed();
    println!("✓ Documents added in {:.3}s", add_time.as_secs_f64());
    println!("  Memory checkpoint: Post-add_documents (CRITICAL - should be ~10 GB)\n");

    println!("Finding duplicates (PROFILING CRITICAL PHASE)...");
    let find_start = Instant::now();

    // THIS IS WHERE THE 26 GB ALLOCATION OCCURS
    // DHAT will capture the memory profile here
    let clusters = match pipeline.find_duplicates(0.85) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ERROR during find_duplicates: {:?}", e);
            std::process::exit(1);
        }
    };
    let find_time = find_start.elapsed();
    println!("✓ Duplicates found in {:.3}s", find_time.as_secs_f64());
    println!("  Memory checkpoint: Post-find_duplicates (PEAK - expect 26-30 GB)\n");

    // Report metrics
    let metrics = pipeline.metrics();
    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║                 MEMORY PROFILE RESULTS                     ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!("Documents ingested: {}", metrics.documents_ingested);
    println!("Bloom skipped: {} ({:.2}%)",
        metrics.documents_skipped,
        (metrics.documents_skipped as f64 / metrics.documents_ingested as f64) * 100.0);
    println!("Duplicate clusters: {}", clusters.len());
    println!("\nPhase timings:");
    println!("  Corpus generation: {:.2}s", corpus_time.as_secs_f64());
    println!("  Add documents: {:.3}s", add_time.as_secs_f64());
    println!("  Find duplicates: {:.3}s", find_time.as_secs_f64());
    println!("\nDHAT heap profile written to: dhat-heap.json");
    println!("View with: dhat/dh_view.html (opens automatically in browser)");
}
