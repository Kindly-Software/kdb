// C4 Corpus Performance Validation
// Tests real-world datasets: 10K, 100K, 1M documents

use kindly_dedup::pipeline::DedupPipeline;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("═══════════════════════════════════════════════════════════");
    println!("  C4 CORPUS PERFORMANCE VALIDATION");
    println!("═══════════════════════════════════════════════════════════\n");

    // Test datasets
    let datasets = vec![
        ("test_data/c4_test_100.jsonl", 100, "100 docs (warmup)"),
        ("test_data/c4_10k.jsonl", 10_000, "10K docs"),
        ("test_data/c4_100k.jsonl", 100_000, "100K docs"),
        ("test_data/c4_1m.jsonl", 1_000_000, "1M docs"),
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

        if documents.len() != expected_count {
            println!("└─ ⚠ Warning: Expected {} docs, got {}\n", expected_count, documents.len());
        }

        // Run deduplication
        let cpu_caps = atomic_capsule::CpuCapabilityCapsule::detect();
        let start_dedup = Instant::now();
        let mut pipeline = DedupPipeline::new(documents.len(), &cpu_caps);

        for (id, text) in documents.iter().enumerate() {
            pipeline.add_document(id, text)?;
        }

        let clusters = pipeline.find_duplicates(0.85)?;
        let dedup_time = start_dedup.elapsed();

        // Calculate metrics
        let throughput = documents.len() as f64 / dedup_time.as_secs_f64();
        let latency_us = dedup_time.as_micros() as f64 / documents.len() as f64;

        println!("├─ Duplicates: {} clusters found", clusters.len());
        println!("├─ Time: {:.2}s", dedup_time.as_secs_f64());
        println!("├─ Throughput: {:.0} docs/sec", throughput);
        println!("└─ Latency: {:.2} µs/doc\n", latency_us);
    }

    println!("═══════════════════════════════════════════════════════════");
    println!("  VALIDATION COMPLETE");
    println!("═══════════════════════════════════════════════════════════");

    Ok(())
}
