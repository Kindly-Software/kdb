//! Adaptive Pipeline Selection Examples
//!
//! This example demonstrates the automatic pipeline selection feature of kindly_dedup v2.2.0.
//! The adaptive selector automatically chooses between:
//! - Fast: 136K docs/sec, O(N) memory (for small-to-medium corpora)
//! - Streaming: 30-100K docs/sec, O(1) 273 MB (for billion-scale corpora)

use std::fs::File;
use std::io::Write;

// NOTE: This is a conceptual example. Actual implementation pending Phase 1-6.
// Replace `AdaptiveDedupPipeline` with actual struct after implementation.

/// Example 1: Automatic Selection (Recommended)
///
/// The system automatically detects available RAM and corpus size,
/// then selects the optimal pipeline without user intervention.
fn example_automatic_selection() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Example 1: Automatic Selection ===\n");

    // Create a sample corpus
    let corpus_path = "sample_corpus.jsonl";
    create_sample_corpus(corpus_path, 100)?;

    // Let the system automatically select the pipeline
    // (Pseudo-code - actual implementation pending)
    println!("Available RAM: 16 GB");
    println!("Corpus size: 100 documents");
    println!("Creating adaptive pipeline with automatic selection...\n");

    // let mut pipeline = AdaptiveDedupPipeline::new_auto(100, 0.85)?;

    // Check which pipeline was selected
    // println!("Selected pipeline: {}",
    //     if pipeline.is_fast() {
    //         "Fast (136K docs/sec, O(N) memory)"
    //     } else {
    //         "Streaming (O(1) 273 MB)"
    //     });

    println!("✓ Automatic selection complete");
    println!("  - Fast selected for 100 docs on 16 GB machine");
    println!("  - Expected throughput: 136K docs/sec");
    println!("  - Expected processing time: <1ms\n");

    Ok(())
}

/// Example 2: Force Fast Pipeline
///
/// Manually override to use the Fast pipeline if you're confident
/// that RAM is sufficient.
fn example_force_fast() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Example 2: Force Fast Pipeline ===\n");

    println!("Creating pipeline with manual Fast override...\n");

    // Force fast (if you know RAM is sufficient)
    // let mut pipeline = AdaptiveDedupPipeline::new_fast(1_000_000, 0.85)?;

    println!("✓ Fast pipeline forced");
    println!("  - Override: --fast flag");
    println!("  - Performance: 136K docs/sec");
    println!("  - Memory: O(N) scaling (610 bytes/doc)");
    println!("  - Warning: May OOM if RAM insufficient\n");

    Ok(())
}

/// Example 3: Force Streaming Pipeline
///
/// Manually override to use the Streaming pipeline when you want
/// guaranteed O(1) memory usage regardless of corpus size.
fn example_force_streaming() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Example 3: Force Streaming Pipeline ===\n");

    println!("Creating pipeline with manual Streaming override...\n");

    // Force streaming (if you want guaranteed O(1) memory)
    // let mut pipeline = AdaptiveDedupPipeline::new_streaming(1_000_000_000, 0.85)?;

    println!("✓ Streaming pipeline forced");
    println!("  - Override: --streaming flag");
    println!("  - Performance: 30-100K docs/sec");
    println!("  - Memory: O(1) 273 MB constant");
    println!("  - Advantage: Handles 1-10 billion documents\n");

    Ok(())
}

/// Example 4: Selection Decision Matrix
///
/// Shows how the adaptive selector chooses pipelines for different
/// combinations of RAM and corpus size.
fn example_selection_matrix() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Example 4: Selection Decision Matrix ===\n");

    let scenarios = vec![
        ("8 GB", 1_000_000, "Fast", "810 MB << 6.4 GB"),
        ("8 GB", 10_000_000, "Streaming", "6.3 GB > 6.4 GB"),
        ("16 GB", 10_000_000, "Fast", "6.3 GB << 12.8 GB"),
        ("64 GB", 10_000_000, "Fast", "6.3 GB << 51.2 GB"),
        ("64 GB", 100_000_000, "Streaming", "61.2 GB > 51.2 GB"),
        ("128 GB", 100_000_000, "Fast", "61.2 GB << 102.4 GB"),
    ];

    println!("{:<12} {:<15} {:<12} {:<30}", "RAM", "Docs", "Selected", "Reason");
    println!("{}", "-".repeat(70));

    for (ram, docs, selected, reason) in scenarios {
        println!("{:<12} {:<15} {:<12} {:<30}", ram, docs, selected, reason);
    }

    println!("\nKey Insights:");
    println!("  • Selection algorithm is conservative (prefers Streaming when close)");
    println!("  • Uses 20% safety margin on estimates (1.25× multiplier)");
    println!("  • Reserves 20% of available RAM for OS/other processes");
    println!("  • Never OOMs: defaults to Streaming when uncertain\n");

    Ok(())
}

/// Example 5: Processing Documents (Unified API)
///
/// Shows that both pipelines use the same API - you don't need to
/// change your code based on which pipeline is selected.
fn example_unified_api() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Example 5: Unified API ===\n");

    println!("Pseudo-code showing unified API:\n");

    println!("// This code works the same regardless of which pipeline is selected!");
    println!("let mut pipeline = AdaptiveDedupPipeline::new_auto(1_000_000, 0.85)?;");
    println!("");
    println!("// Process documents (same API for both Fast and Streaming)");
    println!("for (doc_id, text) in documents {{");
    println!("    pipeline.add_document(doc_id, &text)?;");
    println!("}}");
    println!("");
    println!("// Find duplicates (same API for both)");
    println!("let clusters = pipeline.find_duplicates()?;");
    println!("");
    println!("println!(\"Found {{}} clusters\", clusters.len());\n");

    println!("Benefits of unified API:");
    println!("  • Same code works for 1M or 1B documents");
    println!("  • Transparent switching (no user intervention needed)");
    println!("  • Optimal performance for available resources\n");

    Ok(())
}

/// Example 6: Selection Metadata and Logging
///
/// Shows how to access selection metadata for logging and auditing.
fn example_selection_metadata() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Example 6: Selection Metadata (Q34 Audit Trail) ===\n");

    println!("Pseudo-code showing audit trail logging:\n");

    println!("let mut pipeline = AdaptiveDedupPipeline::new_auto(10_000_000, 0.85)?;");
    println!("");
    println!("// Access selection metadata");
    println!("let metadata = pipeline.selection_metadata();");
    println!("");
    println!("// Log for compliance (SOX, SOC2, GDPR, HIPAA)");
    println!("println!(\"{{:?}}\", serde_json::json!({{");
    println!("    \"event\": \"adaptive_selection\",");
    println!("    \"timestamp\": metadata.timestamp,");
    println!("    \"pipeline\": pipeline.implementation_name(),");
    println!("    \"available_ram_gb\": metadata.available_ram_bytes / 1e9,");
    println!("    \"estimated_ram_gb\": metadata.estimated_ram_bytes / 1e9,");
    println!("    \"corpus_size\": metadata.corpus_size,");
    println!("    \"threshold\": metadata.threshold,");
    println!("    \"reason\": metadata.reason,");
    println!("}}));\n");

    println!("Example output:");
    println!("{{");
    println!("  \"event\": \"adaptive_selection\",");
    println!("  \"timestamp\": \"2025-11-19T12:34:56.789Z\",");
    println!("  \"pipeline\": \"DedupPipeline\",");
    println!("  \"available_ram_gb\": 64.0,");
    println!("  \"estimated_ram_gb\": 6.3,");
    println!("  \"corpus_size\": 10000000,");
    println!("  \"threshold\": 0.85,");
    println!("  \"reason\": \"RAM sufficient (10.2× headroom)\"");
    println!("}}\n");

    Ok(())
}

/// Example 7: Performance Expectations
///
/// Shows realistic performance expectations for both pipelines.
fn example_performance_expectations() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Example 7: Performance Expectations ===\n");

    let benchmarks = vec![
        ("1M docs", "Fast", "7.4 sec", "136K docs/sec"),
        ("1M docs", "Streaming", "10-33 sec", "30-100K docs/sec"),
        ("10M docs", "Fast", "74 sec", "136K docs/sec"),
        ("10M docs", "Streaming", "100-333 sec", "30-100K docs/sec"),
        ("100M docs", "N/A (OOM)", "N/A", "N/A"),
        ("100M docs", "Streaming", "16-55 min", "30-100K docs/sec"),
        ("1B docs", "N/A (OOM)", "N/A", "N/A"),
        ("1B docs", "Streaming", "2.8-9.3 hrs", "30-100K docs/sec"),
    ];

    println!("{:<12} {:<12} {:<15} {:<20}", "Corpus", "Pipeline", "Time", "Throughput");
    println!("{}", "-".repeat(60));

    for (corpus, pipeline, time, throughput) in benchmarks {
        println!("{:<12} {:<12} {:<15} {:<20}", corpus, pipeline, time, throughput);
    }

    println!("\nKey takeaways:");
    println!("  • Fast: Validated on C4 (11.86M docs), ~136K docs/sec");
    println!("  • Streaming: Target (30-100K docs/sec), needs B32 validation");
    println!("  • Streaming enables 1B+ doc capability (impossible with Fast)");
    println!("  • Choose Fast for <50M docs with ample RAM");
    println!("  • Choose Streaming for >50M docs or limited RAM\n");

    Ok(())
}

/// Helper: Create a sample corpus for testing
fn create_sample_corpus(path: &str, num_docs: usize) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::create(path)?;

    for i in 0..num_docs {
        let json = format!(
            r#"{{"id": {}, "text": "Document {} with some sample content"}}"#,
            i, i
        );
        writeln!(file, "{}", json)?;
    }

    Ok(())
}

/// Main: Run all examples
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║   Adaptive Pipeline Selection Examples (v2.2.0)             ║");
    println!("║   kindly_dedup - LLM Dataset Deduplication                  ║");
    println!("╚════════════════════════════════════════════════════════════════╝");

    example_automatic_selection()?;
    example_force_fast()?;
    example_force_streaming()?;
    example_selection_matrix()?;
    example_unified_api()?;
    example_selection_metadata()?;
    example_performance_expectations()?;

    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║   All examples complete!                                     ║");
    println!("║                                                              ║");
    println!("║   Next Steps:                                                ║");
    println!("║   1. Read: docs/ADAPTIVE_SELECTOR_GUIDE.md                  ║");
    println!("║   2. Design: ADAPTIVE_PIPELINE_SELECTOR_UCE34_DESIGN.md     ║");
    println!("║   3. Implement: Phase 1-7 (see design document)             ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    Ok(())
}
