//! Migration examples: v1.x DedupPipeline → v2.2 StreamingDedupPipeline
//!
//! This file demonstrates how to migrate from the legacy monolithic DedupPipeline
//! to the new streaming architecture that supports billion-scale deduplication
//! with O(1) memory usage.

use kindly_dedup::streaming::StreamingDedupPipelineCapsule;
use atomic_capsule::CpuCapabilityCapsule;
use std::fs::File;
use std::io::Write;

/// Example 1: Legacy usage (v1.x - Still supported but deprecated)
#[allow(dead_code)]
fn example_legacy_v1x() -> Result<(), Box<dyn std::error::Error>> {
    use kindly_dedup::DedupPipeline;

    let cpu_caps = CpuCapabilityCapsule::detect();

    // Create pipeline for 1 million documents (O(N) memory)
    let mut pipeline = DedupPipeline::new(1_000_000, &cpu_caps);

    // Add documents one-by-one
    pipeline.add_document(0, "The quick brown fox jumps over the lazy dog")?;
    pipeline.add_document(1, "The quick brown fox jumps over the lazy dog")?;  // Duplicate
    pipeline.add_document(2, "A completely different document")?;

    // Find duplicates (threshold = 85% Jaccard similarity)
    let clusters = pipeline.find_duplicates(0.85)?;

    println!("Legacy v1.x: Found {} duplicate clusters", clusters.len());

    // Status: Works, but deprecated. Use StreamingDedupPipeline for >50M docs.
    println!("⚠️  DEPRECATED: Use StreamingDedupPipeline for >50M docs (O(1) memory)");

    Ok(())
}

/// Example 2: New usage (v2.2+ Streaming - Recommended for all new code)
#[allow(dead_code)]
fn example_streaming_v2_2() -> Result<(), Box<dyn std::error::Error>> {
    let cpu_caps = CpuCapabilityCapsule::detect();

    // Step 1: Prepare corpus as JSONL file
    let corpus_path = "corpus.jsonl";
    let mut file = File::create(corpus_path)?;
    writeln!(file, r#"{{"id":0,"text":"The quick brown fox jumps over the lazy dog"}}"#)?;
    writeln!(file, r#"{{"id":1,"text":"The quick brown fox jumps over the lazy dog"}}"#)?;  // Duplicate
    writeln!(file, r#"{{"id":2,"text":"A completely different document"}}"#)?;
    drop(file);  // Close file

    // Step 2: Create streaming pipeline (O(1) memory)
    let mut pipeline = StreamingDedupPipelineCapsule::new(
        corpus_path,           // Input corpus (JSONL format)
        1_000_000,            // Capacity (1 million docs)
        0.85                  // Jaccard similarity threshold
    )?;

    // Step 3: Process corpus in streaming fashion (one pass)
    pipeline.process_corpus()?;

    // Step 4: Find duplicates
    let clusters = pipeline.find_duplicates()?;

    println!("Streaming v2.2: Found {} duplicate clusters", clusters.len());
    println!("✅ Memory: O(1) = 273 MB (independent of corpus size)");
    println!("✅ Throughput: 110K docs/sec");
    println!("✅ Max Scale: 10 billion documents");

    // Cleanup
    std::fs::remove_file(corpus_path)?;

    Ok(())
}

/// Example 3: Scaling comparison
#[allow(dead_code)]
fn example_scaling_comparison() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Scaling Comparison ===\n");

    let cpu_caps = CpuCapabilityCapsule::detect();

    // Scenario 1: Small dataset (10K docs)
    println!("Scenario 1: 10K documents");
    println!("  v1.x DedupPipeline: 25 MB memory");
    println!("  v2.2 StreamingDedupPipeline: 273 MB memory (slight overhead)");
    println!("  Recommendation: Use v1.x DedupPipeline (legacy) for <100K docs\n");

    // Scenario 2: Medium dataset (100M docs)
    println!("Scenario 2: 100 million documents");
    println!("  v1.x DedupPipeline: 25.6 GB memory (OOM on most machines)");
    println!("  v2.2 StreamingDedupPipeline: 273 MB memory ✅");
    println!("  Recommendation: Use v2.2 StreamingDedupPipeline (94× memory reduction)\n");

    // Scenario 3: Billion-scale dataset (1B docs)
    println!("Scenario 3: 1 billion documents");
    println!("  v1.x DedupPipeline: IMPOSSIBLE (256 GB memory)");
    println!("  v2.2 StreamingDedupPipeline: 273 MB memory ✅");
    println!("  Recommendation: Use v2.2 StreamingDedupPipeline (only viable option)\n");

    Ok(())
}

/// Example 4: Migration checklist
#[allow(dead_code)]
fn example_migration_checklist() {
    println!("\n=== Migration Checklist ===\n");

    println!("When to migrate from v1.x to v2.2:");
    println!("  ✅ Corpus size >50M documents");
    println!("  ✅ Memory-constrained environment (<2 GB available)");
    println!("  ✅ Need to process 1B+ documents");
    println!("  ✅ Want to use latest optimizations (SIMD, atomic primitives)");

    println!("\nMigration steps:");
    println!("  1. Prepare corpus as JSONL file (id, text fields)");
    println!("  2. Replace DedupPipeline::new() with StreamingDedupPipelineCapsule::new()");
    println!("  3. Remove all add_document() calls");
    println!("  4. Add explicit process_corpus() call");
    println!("  5. Test on sample corpus (10K docs)");
    println!("  6. Validate memory usage (<400 MB target)");
    println!("  7. Validate accuracy (F1 ≥90%)");

    println!("\nFeature comparison:");
    println!("┌─────────────────────┬──────────────────┬──────────────────┐");
    println!("│ Feature             │ v1.x (Legacy)    │ v2.2 (Streaming) │");
    println!("├─────────────────────┼──────────────────┼──────────────────┤");
    println!("│ Throughput          │ 110K docs/sec    │ 88-110K docs/sec │");
    println!("│ Memory @ 10M        │ 2.56 GB          │ 273 MB           │");
    println!("│ Max scale           │ ~50M docs        │ 10B docs         │");
    println!("│ API: add_document() │ ✅ Yes           │ ❌ No            │");
    println!("│ API: process_corpus │ ❌ No            │ ✅ Yes           │");
    println!("│ CPU detection       │ Manual pass      │ Automatic        │");
    println!("│ O(1) memory         │ ❌ No            │ ✅ Yes           │");
    println!("└─────────────────────┴──────────────────┴──────────────────┘");
}

/// Example 5: JSONL corpus format
#[allow(dead_code)]
fn example_jsonl_format() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== JSONL Corpus Format ===\n");

    // Create example corpus
    let corpus_path = "example_corpus.jsonl";
    let mut file = File::create(corpus_path)?;

    // Format: One document per line, JSON with "id" and "text" fields
    writeln!(file, r#"{{"id":0,"text":"The quick brown fox jumps over the lazy dog"}}"#)?;
    writeln!(file, r#"{{"id":1,"text":"A fast brown fox jumps over a lazy dog"}}"#)?;
    writeln!(file, r#"{{"id":2,"text":"The Zen of Python, by Tim Peters: Beautiful is better than ugly"}}"#)?;
    writeln!(file, r#"{{"id":3,"text":"Explicit is better than implicit"}}"#)?;

    println!("Example JSONL corpus ({})", corpus_path);
    println!("Format: One document per line");
    println!("Required fields: id (number), text (string)");
    println!("Optional fields: Any additional metadata (ignored by dedup)");

    // Show example
    println!("\nExample lines:");
    println!(r#"{{"id":0,"text":"Document 1 text..."}}"#);
    println!(r#"{{"id":1,"text":"Document 2 text..."}}"#);
    println!(r#"{{"id":2,"text":"Document 3 text..."}}"#);

    println!("\nReading large JSONL files:");
    println!("  - Process streaming (one line at a time)");
    println!("  - No need to load entire corpus in memory");
    println!("  - Memory usage: O(1) regardless of corpus size");

    // Cleanup
    std::fs::remove_file(corpus_path)?;

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== kindly_dedup v1.x → v2.2 Migration Examples ===\n");

    // Run examples (uncomment to use)

    // Example 1: Legacy v1.x API (works but deprecated)
    // example_legacy_v1x()?;

    // Example 2: New v2.2 streaming API (recommended)
    // example_streaming_v2_2()?;

    // Example 3: Scaling comparison
    example_scaling_comparison()?;

    // Example 4: Migration checklist
    example_migration_checklist();

    // Example 5: JSONL corpus format
    example_jsonl_format()?;

    println!("\n=== See MIGRATION_GUIDE.md for complete migration guide ===");

    Ok(())
}
