// C4 Corpus Persistent Deduplication Validation
// Tests PersistentDedupPipeline (T9 mmap) for enterprise RAM efficiency

use kindly_dedup::persistent_pipeline::PersistentDedupPipeline;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("═══════════════════════════════════════════════════════════");
    println!("  C4 PERSISTENT DEDUPLICATION VALIDATION (T9 Mmap)");
    println!("═══════════════════════════════════════════════════════════\n");

    // Test datasets
    let datasets = vec![
        ("test_data/c4_10k.jsonl", 10_000, "10K docs"),
        ("test_data/c4_100k.jsonl", 100_000, "100K docs"),
        ("test_data/c4_1m.jsonl", 354_326, "354K docs (C4 shard limit)"),
    ];

    for (path, expected_count, label) in datasets {
        println!("Testing: {}", label);
        println!("├─ File: {}", path);

        // Load dataset
        let start_load = Instant::now();
        let file = match File::open(path) {
            Ok(f) => f,
            Err(e) => {
                println!("└─ ⚠ Skipped: {} (not found)\n", e);
                continue;
            }
        };

        let reader = BufReader::new(file);
        let mut documents = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if let Ok(doc) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(text) = doc["text"].as_str() {
                    documents.push(text.to_string());
                }
            }
        }

        let load_time = start_load.elapsed();
        println!("├─ Loaded: {} documents in {:.2}s", documents.len(), load_time.as_secs_f64());

        // Create persistent pipeline (T9 mmap architecture)
        let mmap_path = format!("/tmp/dedup_{}.mmap", label.replace(" ", "_"));
        let start_dedup = Instant::now();

        let mut pipeline = match PersistentDedupPipeline::create(&mmap_path, documents.len()) {
            Ok(p) => p,
            Err(e) => {
                println!("└─ ⚠ Failed to create pipeline: {}\n", e);
                continue;
            }
        };

        // Add documents
        for (id, text) in documents.iter().enumerate() {
            if let Err(e) = pipeline.add_document(id, text) {
                println!("└─ ⚠ Failed at doc {}: {}\n", id, e);
                continue;
            }
        }

        // Find duplicates
        let clusters = match pipeline.find_duplicates(0.85) {
            Ok(c) => c,
            Err(e) => {
                println!("└─ ⚠ Find duplicates failed: {}\n", e);
                continue;
            }
        };

        let dedup_time = start_dedup.elapsed();

        // Get memory stats from /proc/self/status
        let status = std::fs::read_to_string("/proc/self/status")?;
        let rss_kb: u64 = status
            .lines()
            .find(|l| l.starts_with("VmRSS:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let rss_mb = rss_kb / 1024;

        // Get mmap file size
        let mmap_size = std::fs::metadata(&mmap_path)?.len();
        let mmap_mb = mmap_size / (1024 * 1024);

        // Calculate metrics
        let throughput = documents.len() as f64 / dedup_time.as_secs_f64();
        let latency_us = dedup_time.as_micros() as f64 / documents.len() as f64;
        let bytes_per_doc = rss_kb as f64 * 1024.0 / documents.len() as f64;

        println!("├─ Duplicates: {} clusters found ({:.1}% duplicate rate)",
            clusters.len(),
            (1.0 - clusters.len() as f64 / documents.len() as f64) * 100.0
        );
        println!("├─ Time: {:.2}s", dedup_time.as_secs_f64());
        println!("├─ Throughput: {:.0} docs/sec", throughput);
        println!("├─ Latency: {:.2} µs/doc", latency_us);
        println!("├─ RAM (RSS): {} MB ({:.2} bytes/doc)", rss_mb, bytes_per_doc);
        println!("└─ Disk (mmap): {} MB\n", mmap_mb);

        // Cleanup
        let _ = std::fs::remove_file(&mmap_path);
    }

    println!("═══════════════════════════════════════════════════════════");
    println!("  PERSISTENT VALIDATION COMPLETE");
    println!("  Architecture: T9 Persistent (mmap) + T10 Probabilistic");
    println!("═══════════════════════════════════════════════════════════");

    Ok(())
}
