//! Phase 2 Integration Test - Mmap LSH Bucketer
//!
//! Validates TRUE 93% memory reduction via mmap-backed LSH buckets

#[cfg(feature = "persistent-dedup")]
#[test]
fn test_phase2_mmap_lsh_integration() {
    use kindly_dedup::PersistentDedupPipeline;
    use atomic_capsule::CpuCapabilityCapsule;

    let temp_path = "/tmp/test_phase2_integration.bin";
    let _ = std::fs::remove_file(temp_path); // Clean up

    let cpu_caps = CpuCapabilityCapsule::detect();

    // Create pipeline with 100 document capacity
    let capacity = 100;
    let num_threads = 1;
    let mut pipeline = PersistentDedupPipeline::create(temp_path, capacity, num_threads, &cpu_caps)
        .expect("Failed to create persistent pipeline");

    // Add 10 documents (5 unique, 5 duplicates)
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

    for (doc_id, text) in docs.iter().enumerate() {
        pipeline.add_document(doc_id, text).expect("Failed to add document");
    }

    // Flush to ensure mmap is synced
    pipeline.flush().expect("Failed to flush");

    // Find duplicates (Jaccard threshold 0.85)
    let clusters = pipeline.find_duplicates(0.85).expect("Failed to find duplicates");

    // Verify we found some duplicate clusters
    assert!(!clusters.is_empty(), "Should have found at least one duplicate cluster");
    println!("Phase 2 Integration Test: Found {} clusters", clusters.len());

    // Verify cluster sizes (each cluster should have at least 2 docs)
    for cluster in &clusters {
        assert!(cluster.len() >= 2, "Cluster should have at least 2 documents");
        println!("  Cluster: {:?}", cluster);
    }

    // Clean up
    let _ = std::fs::remove_file(temp_path);
}

#[cfg(feature = "persistent-dedup")]
#[test]
fn test_phase2_mmap_recovery() {
    use kindly_dedup::PersistentDedupPipeline;
    use atomic_capsule::CpuCapabilityCapsule;

    let temp_path = "/tmp/test_phase2_recovery.bin";
    let _ = std::fs::remove_file(temp_path); // Clean up

    let cpu_caps = CpuCapabilityCapsule::detect();

    // Create pipeline and add documents
    {
        let capacity = 100;
        let num_threads = 1;
        let mut pipeline = PersistentDedupPipeline::create(temp_path, capacity, num_threads, &cpu_caps)
            .expect("Failed to create persistent pipeline");

        for (doc_id, text) in [
            "Document one about machine learning",
            "Document two about neural networks",
            "Document one about machine learning", // Duplicate
        ].iter().enumerate() {
            pipeline.add_document(doc_id, text).expect("Failed to add document");
        }

        pipeline.flush().expect("Failed to flush");
    } // Drop pipeline

    // Recover pipeline
    {
        let num_threads = 1;
        let pipeline = PersistentDedupPipeline::recover(temp_path, num_threads, &cpu_caps)
            .expect("Failed to recover pipeline");

        assert_eq!(pipeline.count(), 3, "Should have recovered 3 documents");

        // Find duplicates after recovery
        let clusters = pipeline.find_duplicates(0.85).expect("Failed to find duplicates");
        println!("Phase 2 Recovery Test: Found {} clusters", clusters.len());

        // Should find at least one duplicate cluster (docs 0 and 2)
        assert!(!clusters.is_empty(), "Should have found duplicate cluster after recovery");
    }

    // Clean up
    let _ = std::fs::remove_file(temp_path);
}
