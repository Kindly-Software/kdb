//! Phase 2 Integration Demo - Facade API with CpuStreaming Mode
//!
//! Demonstrates using the Facade API with CpuStreaming mode for memory efficiency
//!
//! Run with: cargo run --example phase2_demo --release

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use kindly_dedup::{Dedup, DedupMode};

    println!("=== Phase 2 Integration Demo ===");
    println!("Testing Facade API with CpuStreaming mode for memory efficiency\n");

    // Create dedup instance with CpuStreaming mode for memory efficiency
    let capacity = 1000;
    let mut dedup = Dedup::with_mode(DedupMode::CpuStreaming, capacity)?;

    println!("Created dedup instance (mode: {:?}, capacity: {})", dedup.current_mode(), capacity);

    // Add test documents (5 unique, 5 duplicates)
    let docs = vec![
        "The quick brown fox jumps over the lazy dog",
        "The quick brown fox leaps over the lazy dog", // Near-duplicate (85%+ similarity)
        "A completely different document about cats and mice",
        "The quick brown fox jumps over the lazy dog", // Exact duplicate
        "Another unique document about quantum computing",
        "The quick brown fox leaps over the lazy dog", // Duplicate of doc 1
        "A completely different document about cats and mice", // Duplicate of doc 2
        "Unique document number seven about space exploration",
        "Unique document number eight about deep sea diving",
        "Another unique document about quantum computing", // Duplicate of doc 4
    ];

    println!("Adding {} documents...", docs.len());
    for (doc_id, text) in docs.iter().enumerate() {
        dedup.add_document(doc_id as u64, text)?;
    }

    println!("Documents added successfully");

    // Find duplicates (Jaccard threshold 0.85)
    println!("\nFinding duplicates (threshold: 0.85)...");
    let clusters = dedup.find_duplicates(0.85)?;

    println!("\n=== Results ===");
    println!("Found {} duplicate clusters:", clusters.len());
    for (i, cluster) in clusters.iter().enumerate() {
        println!("  Cluster {}: {:?} ({} documents)", i + 1, cluster, cluster.len());
    }

    // Show statistics
    let stats = dedup.stats();
    println!("\n=== Statistics ===");
    println!("Mode:                {:?}", stats.mode);
    println!("Documents processed: {}", stats.documents_processed);
    println!("Total time:          {:?}", stats.total_time);
    println!("Avg time per doc:    {:?}", stats.avg_time_per_doc);

    // Verify results
    if clusters.is_empty() {
        println!("\n⚠️  Warning: No clusters found (expected at least 1)");
    } else {
        println!("\n✅ Success: Phase 2 demo completed successfully!");
    }

    Ok(())
}
