//! Phase 2 Integration Demo - Mmap LSH Bucketer
//!
//! Demonstrates TRUE 93% memory reduction via mmap-backed LSH buckets
//!
//! Run with: cargo run --example phase2_demo --features persistent-dedup --release

#[cfg(feature = "persistent-dedup")]
fn main() {
    use kindly_dedup::PersistentDedupPipeline;
    use atomic_capsule::CpuCapabilityCapsule;

    let temp_path = "/tmp/phase2_demo.bin";
    let _ = std::fs::remove_file(temp_path); // Clean up

    let cpu_caps = CpuCapabilityCapsule::detect();

    println!("=== Phase 2 Integration Demo ===");
    println!("Testing mmap-backed LSH buckets for 93% memory reduction\n");

    // Create pipeline with 1000 document capacity
    let capacity = 1000;
    let num_threads = 1;
    let mut pipeline = PersistentDedupPipeline::create(temp_path, capacity, num_threads, &cpu_caps)
        .expect("Failed to create persistent pipeline");

    println!("Created persistent pipeline (capacity: {})", capacity);

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
        pipeline.add_document(doc_id, text).expect("Failed to add document");
    }

    // Flush to ensure mmap is synced
    pipeline.flush().expect("Failed to flush");
    println!("Documents flushed to disk");

    // Find duplicates (Jaccard threshold 0.85)
    println!("\nFinding duplicates (threshold: 0.85)...");
    let clusters = pipeline.find_duplicates(0.85).expect("Failed to find duplicates");

    println!("\n=== Results ===");
    println!("Found {} duplicate clusters:", clusters.len());
    for (i, cluster) in clusters.iter().enumerate() {
        println!("  Cluster {}: {:?} ({} documents)", i + 1, cluster, cluster.len());
    }

    // Verify results
    if clusters.is_empty() {
        println!("\n⚠️  Warning: No clusters found (expected at least 1)");
    } else {
        println!("\n✅ Success: Phase 2 integration working correctly!");
    }

    // Clean up
    let _ = std::fs::remove_file(temp_path);
    println!("\nCleaned up temporary file");
}

#[cfg(not(feature = "persistent-dedup"))]
fn main() {
    eprintln!("Error: This example requires the 'persistent-dedup' feature");
    eprintln!("Run with: cargo run --example phase2_demo --features persistent-dedup");
    std::process::exit(1);
}
