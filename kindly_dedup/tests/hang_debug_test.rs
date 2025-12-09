#[test]
fn test_100k_corpus_no_hang() {
    use std::path::Path;
    use std::time::Instant;

    let corpus_path = "/home/samuel/Primitives/kindly_dedup/test_data/c4_100k.jsonl";

    if !Path::new(corpus_path).exists() {
        eprintln!("Skipping test - corpus not found: {}", corpus_path);
        return;
    }

    eprintln!("\n[TEST] Starting 100k corpus test with instrumentation...");
    let start = Instant::now();

    // Check file size
    let metadata = std::fs::metadata(corpus_path).unwrap();
    eprintln!("[TEST] Corpus size: {:.2} MB", metadata.len() as f64 / 1e6);

    // Create a simple test that reads the corpus
    let file = std::fs::File::open(corpus_path).unwrap();
    let mmap = unsafe { memmap2::Mmap::map(&file).unwrap() };
    let mmap_data: &[u8] = &mmap;

    // Import the reader directly
    use kindly_dedup::universal::MmapCorpusReaderCapsule;

    let reader = MmapCorpusReaderCapsule::new(mmap_data.len() as u64).unwrap();

    const CHUNK_SIZE: u64 = 5_242_880;  // 5 MB chunks
    let mut total_docs = 0u64;
    let mut chunk_count = 0u64;

    loop {
        eprintln!("[TEST] Getting chunk #{}", chunk_count);

        match reader.next_chunk_iter(mmap_data, CHUNK_SIZE) {
            Ok(Some(iter)) => {
                let mut docs_in_chunk = 0u64;
                for doc_result in iter {
                    match doc_result {
                        Ok(doc) => {
                            docs_in_chunk += 1;
                            total_docs += 1;
                            if total_docs <= 5 {
                                eprintln!("[TEST] Doc {}: {} bytes text", doc.id, doc.text.len());
                            }
                        },
                        Err(e) => {
                            eprintln!("[TEST] ERROR parsing document: {}", e);
                            panic!("Document parsing failed");
                        }
                    }
                }
                eprintln!("[TEST] Chunk #{}: {} documents", chunk_count, docs_in_chunk);
                chunk_count += 1;
            },
            Ok(None) => {
                eprintln!("[TEST] EOF reached");
                break;
            },
            Err(e) => {
                eprintln!("[TEST] ERROR: {}", e);
                panic!("Corpus reading failed");
            }
        }

        if chunk_count > 100 {
            eprintln!("[TEST] Stopping after 100 chunks for test");
            break;
        }

        let elapsed = start.elapsed();
        if elapsed.as_secs() > 30 {
            eprintln!("[TEST] TIMEOUT - test took too long (>30s)");
            panic!("Timeout");
        }
    }

    let elapsed = start.elapsed();
    eprintln!("[TEST] ✅ SUCCESS: Processed {} documents in {} chunks in {:.2}s",
        total_docs, chunk_count, elapsed.as_secs_f64());
    
    assert!(total_docs > 0, "Should have processed at least some documents");
}
