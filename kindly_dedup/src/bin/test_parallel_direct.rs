// Direct test of JobLevelDedupPipelineMetaCapsule
fn main() {
    use std::path::Path;
    
    let corpus_path = "test_data/c4_100k.jsonl";
    
    if !Path::new(corpus_path).exists() {
        eprintln!("[ABORT] Corpus file not found: {}", corpus_path);
        std::process::exit(1);
    }
    
    eprintln!("[START] Direct parallel pipeline test");
    eprintln!("[INFO] Using corpus: {}", corpus_path);
    eprintln!("[INFO] Expecting 4 chunks");
    
    // Try to create pipeline
    match kindly_dedup::universal::JobLevelDedupPipelineMetaCapsule::new(
        corpus_path,
        100_000,  // num_documents
        4,        // num_chunks
        0.85,     // threshold
    ) {
        Ok(mut pipeline) => {
            eprintln!("[INFO] Pipeline created successfully");
            eprintln!("[INFO] Starting pipeline.run()...");
            
            // This will trigger all the trace points
            match pipeline.run() {
                Ok(clusters) => {
                    eprintln!("[SUCCESS] Pipeline completed with {} clusters", clusters.len());
                }
                Err(e) => {
                    eprintln!("[FAILED] Pipeline error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("[FAILED] Cannot create pipeline: {}", e);
            std::process::exit(1);
        }
    }
}
