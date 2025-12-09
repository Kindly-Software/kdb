//! Integration test for JSON boundary handling in MmapCorpusReaderCapsule
//!
//! This test validates that chunking respects JSON record boundaries
//! and doesn't split records mid-object.

#[cfg(test)]
mod json_boundary_tests {
    use kindly_dedup::universal::MmapCorpusReaderCapsule;
    use std::fs::File;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Q21: Integration test - JSON boundary preservation
    #[test]
    fn test_q21_json_boundary_preservation() {
        // Create a test corpus with large JSON records
        let mut temp_file = NamedTempFile::new().unwrap();

        // Write 100 documents, each ~50KB (to force multiple chunks)
        for doc_id in 0..100 {
            let large_text = "A".repeat(50_000); // 50KB text
            let json_line = format!(r#"{{"id": {}, "text": "{}"}}"#, doc_id, large_text);
            writeln!(temp_file, "{}", json_line).unwrap();
        }

        temp_file.flush().unwrap();

        // Mmap the file
        let file = temp_file.reopen().unwrap();
        let mmap = unsafe { memmap2::Mmap::map(&file).unwrap() };

        println!("Test corpus size: {:.2} MB", mmap.len() as f64 / 1e6);

        // Create reader
        let reader = MmapCorpusReaderCapsule::new(mmap.len() as u64).unwrap();

        // Use a small chunk size (100 KB) to force many chunks
        const CHUNK_SIZE: u64 = 100 * 1024; // 100 KB

        let mut total_docs = 0;
        let mut chunk_num = 0;

        while let Some(chunk) = reader.next_chunk(&mmap, CHUNK_SIZE).unwrap() {
            chunk_num += 1;
            let docs_in_chunk = chunk.len();
            total_docs += docs_in_chunk;

            println!(
                "Chunk {}: {} documents (total: {}, progress: {:.1}%)",
                chunk_num,
                docs_in_chunk,
                total_docs,
                reader.progress() * 100.0
            );

            // Verify all documents in chunk are valid
            for doc in chunk {
                assert!(doc.id < 100, "Doc ID out of range: {}", doc.id);
                assert!(doc.text.len() > 0, "Empty text for doc {}", doc.id);
            }
        }

        println!("\n=== RESULTS ===");
        println!("Total chunks: {}", chunk_num);
        println!("Total documents: {}", total_docs);
        println!("Expected: 100");

        assert_eq!(total_docs, 100, "Expected 100 documents but got {}", total_docs);
        assert!(chunk_num > 1, "Should have multiple chunks (got {})", chunk_num);
    }

    /// Q22: Integration test - Small records (boundary case)
    #[test]
    fn test_q22_small_records_chunking() {
        // Create a test corpus with tiny JSON records
        let mut temp_file = NamedTempFile::new().unwrap();

        // Write 10,000 tiny documents (each ~50 bytes)
        for doc_id in 0..10_000 {
            let json_line = format!(r#"{{"id": {}, "text": "Short text {}."}}"#, doc_id, doc_id);
            writeln!(temp_file, "{}", json_line).unwrap();
        }

        temp_file.flush().unwrap();

        // Mmap the file
        let file = temp_file.reopen().unwrap();
        let mmap = unsafe { memmap2::Mmap::map(&file).unwrap() };

        println!("Test corpus size: {:.2} KB", mmap.len() as f64 / 1e3);

        // Create reader
        let reader = MmapCorpusReaderCapsule::new(mmap.len() as u64).unwrap();

        // Use a moderate chunk size (10 KB)
        const CHUNK_SIZE: u64 = 10 * 1024; // 10 KB

        let mut total_docs = 0;

        while let Some(chunk) = reader.next_chunk(&mmap, CHUNK_SIZE).unwrap() {
            total_docs += chunk.len();
        }

        assert_eq!(total_docs, 10_000, "Expected 10,000 documents but got {}", total_docs);
    }

    /// Q23: Integration test - Large records (stress test)
    #[test]
    fn test_q23_large_records_stress() {
        // Create a test corpus with very large JSON records (each > chunk size)
        let mut temp_file = NamedTempFile::new().unwrap();

        // Write 10 documents, each 200 KB (larger than typical chunk size)
        for doc_id in 0..10 {
            let large_text = "B".repeat(200_000); // 200 KB text
            let json_line = format!(r#"{{"id": {}, "text": "{}"}}"#, doc_id, large_text);
            writeln!(temp_file, "{}", json_line).unwrap();
        }

        temp_file.flush().unwrap();

        // Mmap the file
        let file = temp_file.reopen().unwrap();
        let mmap = unsafe { memmap2::Mmap::map(&file).unwrap() };

        println!("Test corpus size: {:.2} MB", mmap.len() as f64 / 1e6);

        // Create reader
        let reader = MmapCorpusReaderCapsule::new(mmap.len() as u64).unwrap();

        // Use a small chunk size (100 KB) - smaller than record size
        const CHUNK_SIZE: u64 = 100 * 1024; // 100 KB

        let mut total_docs = 0;

        while let Some(chunk) = reader.next_chunk(&mmap, CHUNK_SIZE).unwrap() {
            total_docs += chunk.len();
        }

        assert_eq!(total_docs, 10, "Expected 10 documents but got {}", total_docs);
    }

    /// Q24: Real-world test - C4 100K corpus (if available)
    #[test]
    #[ignore] // Only run with --ignored flag (requires 2.2 GB corpus)
    fn test_q24_c4_100k_corpus() {
        let corpus_path = "/home/samuel/Primitives/kindly_dedup/test_data/realistic/c4_100k.jsonl";

        if !std::path::Path::new(corpus_path).exists() {
            println!("Skipping test - corpus not found: {}", corpus_path);
            return;
        }

        println!("Testing with real C4 100K corpus...");

        let file = File::open(corpus_path).unwrap();
        let mmap = unsafe { memmap2::Mmap::map(&file).unwrap() };

        println!("Corpus size: {:.2} GB", mmap.len() as f64 / 1e9);

        let reader = MmapCorpusReaderCapsule::new(mmap.len() as u64).unwrap();

        // Use 5 MB chunks (production size)
        const CHUNK_SIZE: u64 = 5 * 1024 * 1024;

        let mut total_docs = 0;
        let mut chunk_num = 0;

        while let Some(chunk) = reader.next_chunk(&mmap, CHUNK_SIZE).unwrap() {
            chunk_num += 1;
            total_docs += chunk.len();

            if chunk_num % 100 == 0 {
                println!(
                    "Chunk {}: {} total docs ({:.1}%)",
                    chunk_num,
                    total_docs,
                    reader.progress() * 100.0
                );
            }
        }

        println!("\n=== RESULTS ===");
        println!("Total chunks: {}", chunk_num);
        println!("Total documents: {}", total_docs);
        println!("Expected: 100,000");

        assert_eq!(total_docs, 100_000, "Expected 100,000 documents but got {}", total_docs);
    }
}
