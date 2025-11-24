// Phase 3D Step 5: Large-scale validation (1M corpus)
use std::path::Path;
use std::time::Instant;

fn main() {
    use kindly_dedup::universal::JobLevelDedupPipelineMetaCapsule;

    let corpus_path = "test_data/c4_1m.jsonl";

    if !Path::new(corpus_path).exists() {
        eprintln!("[ABORT] Corpus file not found: {}", corpus_path);
        std::process::exit(1);
    }

    eprintln!("[START] Large-scale validation (1M corpus, 4 threads)");
    eprintln!("[INFO] Using corpus: {}", corpus_path);
    eprintln!("");

    let start = Instant::now();

    match JobLevelDedupPipelineMetaCapsule::new(
        corpus_path,
        1_000_000,  // num_documents
        4,          // num_threads
        0.85,       // threshold
    ) {
        Ok(mut pipeline) => {
            match pipeline.run() {
                Ok(clusters) => {
                    let duration = start.elapsed();
                    let secs = duration.as_secs_f64();
                    let throughput = 1_000_000.0 / secs;

                    eprintln!("✅ PASS: 1M corpus processed");
                    eprintln!("  Runtime: {:?} ({:.2}s)", duration, secs);
                    eprintln!("  Clusters: {}", clusters.len());
                    eprintln!("  Throughput: {:.0} docs/sec", throughput);
                    eprintln!("");
                    eprintln!("[SUCCESS] 1M validation complete");
                }
                Err(e) => {
                    eprintln!("❌ FAIL: Pipeline error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("❌ FAIL: Cannot create pipeline: {}", e);
            std::process::exit(1);
        }
    }
}
