//! Basic Deduplication Example
//!
//! Demonstrates the simplest usage of the Dedup facade API.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example basic_dedup
//! ```

use kindly_dedup::Dedup;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Basic Deduplication Example ===\n");

    // Create deduplicator with auto mode selection
    // The facade will automatically choose the best mode based on system resources
    let mut dedup = Dedup::new(1000)?;
    println!("Created dedup instance");
    println!("Mode: {:?}\n", dedup.current_mode());

    // Add some documents
    println!("Adding documents...");
    dedup.add_document(0, "The quick brown fox jumps over the lazy dog")?;
    dedup.add_document(1, "The quick brown fox jumps over the lazy dog")?; // exact duplicate
    dedup.add_document(2, "A completely different document about cats and dogs")?;
    dedup.add_document(3, "The quick brown fox leaps over the lazy dog")?; // similar (85%+)
    dedup.add_document(4, "Python is a high-level programming language")?;
    dedup.add_document(5, "Rust provides memory safety without garbage collection")?;
    dedup.add_document(6, "Python is a high-level programming language")?; // duplicate of 4
    println!("Added 7 documents\n");

    // Find duplicates with 85% similarity threshold
    println!("Finding duplicates (threshold=0.85)...");
    let clusters = dedup.find_duplicates(0.85)?;

    println!("Found {} duplicate clusters:\n", clusters.len());
    for (i, cluster) in clusters.iter().enumerate() {
        if cluster.len() > 1 {
            println!("  Cluster {}: {:?} ({} documents)", i + 1, cluster, cluster.len());
        }
    }

    // Show statistics
    let stats = dedup.stats();
    println!("\n=== Statistics ===");
    println!("Mode:                {:?}", stats.mode);
    println!("Documents processed: {}", stats.documents_processed);
    println!("Total time:          {:?}", stats.total_time);
    println!("Avg time per doc:    {:?}", stats.avg_time_per_doc);

    Ok(())
}
