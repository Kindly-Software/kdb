//! Simple example demonstrating the Dedup facade API
//!
//! Run with: cargo run --example facade_demo --features benchmarking

use kindly_dedup::{Dedup, DedupMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Kindly Dedup Facade Demo ===\n");

    // Create deduplication instance (auto-selects best mode)
    let mut dedup = Dedup::new(1000)?;
    println!("Created dedup instance (mode: {:?})", dedup.current_mode());

    // Add some documents
    println!("\nAdding documents...");
    dedup.add_document(0, "The quick brown fox jumps over the lazy dog")?;
    dedup.add_document(1, "A completely different document about cats")?;
    dedup.add_document(2, "The quick brown fox jumps over the lazy dog")?; // Duplicate of 0
    dedup.add_document(3, "Another unique document about programming")?;
    dedup.add_document(4, "The quick brown fox jumps over lazy dog")?; // Similar to 0
    dedup.add_document(5, "A completely different document about cats")?; // Duplicate of 1

    // Get statistics before finding duplicates
    let stats = dedup.stats();
    println!("Processed {} documents in {:?}",
        stats.documents_processed,
        stats.total_time
    );

    // Find duplicate clusters
    println!("\nFinding duplicates (threshold=0.85)...");
    let clusters = dedup.find_duplicates(0.85)?;

    println!("Found {} duplicate clusters:\n", clusters.len());
    for (i, cluster) in clusters.iter().enumerate() {
        if cluster.len() > 1 {
            println!("Cluster {}: {:?}", i + 1, cluster);
        }
    }

    // Get final statistics
    let final_stats = dedup.stats();
    println!("\n=== Final Statistics ===");
    println!("Mode: {:?}", final_stats.mode);
    println!("Documents processed: {}", final_stats.documents_processed);
    println!("Total time: {:?}", final_stats.total_time);
    println!("Avg time per doc: {:?}", final_stats.avg_time_per_doc);
    println!("Throughput: {:.0} docs/sec",
        final_stats.documents_processed as f64 / final_stats.total_time.as_secs_f64()
    );

    // Try different modes explicitly
    println!("\n=== Testing Explicit Modes ===");

    // Force CPU streaming mode
    let mut dedup2 = Dedup::with_mode(DedupMode::CpuStreaming, 500)?;
    println!("Created with explicit CpuStreaming mode: {:?}", dedup2.current_mode());

    // Auto mode (recommended)
    let dedup3 = Dedup::new(10_000)?;
    println!("Auto mode for 10K docs: {:?}", dedup3.current_mode());

    Ok(())
}
