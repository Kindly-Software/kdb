// Test to verify MmapCorpusReaderCapsule respects JSON boundaries

use kindly_dedup::universal::MmapCorpusReaderCapsule;
use memmap2::Mmap;
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Path to 100K corpus (2.2 GB)
    let corpus_path = "/home/samuel/Primitives/kindly_dedup/test_data/realistic/c4_100k.jsonl";

    println!("Opening corpus: {}", corpus_path);
    let file = File::open(corpus_path)?;
    let mmap = unsafe { Mmap::map(&file)? };

    println!("Corpus size: {:.2} GB", mmap.len() as f64 / 1e9);

    // Create reader
    let reader = MmapCorpusReaderCapsule::new(mmap.len() as u64)?;

    // Process in 5 MB chunks
    const CHUNK_SIZE: u64 = 5 * 1024 * 1024; // 5 MB

    let mut total_docs = 0;
    let mut chunk_num = 0;

    while let Some(chunk) = reader.next_chunk(&mmap, CHUNK_SIZE)? {
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

        // Show first doc in chunk
        if let Some(first) = chunk.first() {
            println!("  First doc ID: {}, text preview: {:?}", first.id, &first.text[..first.text.len().min(50)]);
        }
    }

    println!("\n=== RESULTS ===");
    println!("Total chunks: {}", chunk_num);
    println!("Total documents: {}", total_docs);
    println!("Expected: 100,000");
    println!("Match: {}", if total_docs == 100_000 { "✓ YES" } else { "✗ NO" });

    if total_docs != 100_000 {
        eprintln!("ERROR: Expected 100,000 documents but got {}", total_docs);
        std::process::exit(1);
    }

    println!("\n✓ SUCCESS: All 100,000 documents parsed correctly!");
    Ok(())
}
