// Quick memory profiling test for UniversalDedupPipeline

use std::fs::File;
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("[TEST] Starting memory profile test");

    // Generate test corpus (1000 docs)
    let corpus_path = "/tmp/test_corpus_1k.jsonl";
    eprintln!("[TEST] Generating {} with 1000 docs", corpus_path);

    let mut file = File::create(corpus_path)?;
    for i in 0..1000 {
        let doc = format!("{{\"id\":{},\"text\":\"This is document {} with some unique content here\"}}\n", i, i);
        file.write_all(doc.as_bytes())?;
    }
    drop(file);

    eprintln!("[TEST] Corpus generated, initializing UniversalDedupPipeline");

    // Create pipeline
    use kindly_dedup::universal::UniversalDedupPipeline;

    let mut pipeline = UniversalDedupPipeline::new(
        corpus_path,
        1_000,
        0.85
    )?;

    eprintln!("[TEST] Pipeline created, processing corpus");

    // Process corpus (should show [MEMORY] logs)
    pipeline.process_corpus()?;

    eprintln!("[TEST] Corpus processed successfully");

    // Find duplicates
    let clusters = pipeline.find_duplicates()?;
    eprintln!("[TEST] Found {} clusters", clusters.len());

    Ok(())
}
