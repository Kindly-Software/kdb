//! Example: Auto-detect format from file extension
//!
//! Demonstrates the format registry's auto-detection capability.
//! Shows how to load documents from different formats without
//! explicitly specifying the format.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example load_auto_detect --release --features "format-all,cpu-detection"
//! ```
//!
//! # Supported Extensions
//!
//! - `.jsonl`: JSON Lines format (T2 SIMD, 436K docs/sec)
//! - `.json`: JSON array format (T2 SIMD, 436K docs/sec)
//! - `.csv`, `.tsv`: Comma/Tab-separated values (T1 Atomic, 10K docs/sec)
//! - `.txt`: Plain text, one document per line (T1 Atomic, 50K docs/sec)
//!
//! # Performance Tips
//!
//! - Use `.jsonl` for best performance (streaming, no memory overhead)
//! - Use `.txt` for simple text corpora (minimal parsing)
//! - Use `.csv` for structured data with metadata
//! - Auto-detection has <1ms overhead

use atomic_capsule::CpuCapabilityCapsule;
use kindly_dedup::format::{list_available_formats, load_documents_auto};
use kindly_dedup::DedupPipeline;
use std::fs::File;
use std::io::Write;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("===== Format Auto-Detection Example =====\n");

    // List available formats
    println!("Available formats:");
    for (name, exts) in list_available_formats() {
        println!("  - {}: {}", name, exts);
    }
    println!();

    let cpu_caps = CpuCapabilityCapsule::detect();

    // Test different formats
    let formats = vec![
        ("corpus.txt", "Plain Text"),
        #[cfg(feature = "format-json")]
        ("corpus.jsonl", "JSONL"),
        #[cfg(feature = "format-json")]
        ("corpus.json", "JSON"),
        #[cfg(feature = "format-csv")]
        ("corpus.csv", "CSV"),
    ];

    for (filename, format_name) in formats {
        println!("\n===== Testing {} =====", format_name);

        // Create test file
        create_test_file(filename, format_name, 500)?;

        // Load with auto-detection
        println!("Loading {} with auto-detection...", filename);
        let start = Instant::now();
        let docs = load_documents_auto(filename)?;
        let load_time = start.elapsed();

        println!(
            "Loaded {} documents in {:.2}ms",
            docs.len(),
            load_time.as_secs_f64() * 1000.0
        );
        println!(
            "Throughput: {:.0} docs/sec",
            docs.len() as f64 / load_time.as_secs_f64()
        );

        // Quick dedup to show integration
        let mut pipeline = DedupPipeline::new(docs.len(), &cpu_caps)?;
        for doc in &docs {
            pipeline.add_document(doc.id, &doc.text)?;
        }

        let start = Instant::now();
        let clusters = pipeline.find_duplicates(0.85)?;
        let dedup_time = start.elapsed();

        println!(
            "Deduplication: {} clusters in {:.2}ms",
            clusters.len(),
            dedup_time.as_secs_f64() * 1000.0
        );
        println!(
            "Total throughput: {:.0} docs/sec (format + dedup)",
            docs.len() as f64 / (load_time + dedup_time).as_secs_f64()
        );

        // Cleanup
        std::fs::remove_file(filename).ok();
    }

    println!("\n===== Auto-Detection Complete =====");

    Ok(())
}

/// Create a test file in the specified format
fn create_test_file(filename: &str, format: &str, count: usize) -> Result<(), Box<dyn std::error::Error>> {
    let texts = vec![
        "The quick brown fox jumps over the lazy dog",
        "Python is a high-level programming language",
        "Rust provides memory safety without garbage collection",
        "Machine learning models require large datasets",
        "Cloud computing enables scalable infrastructure",
    ];

    match format {
        "JSONL" => {
            let mut file = File::create(filename)?;
            for i in 0..count {
                let text_idx = i % texts.len();
                let doc = serde_json::json!({
                    "id": i as u64,
                    "text": texts[text_idx]
                });
                writeln!(file, "{}", doc.to_string())?;
            }
        }
        "JSON" => {
            let mut file = File::create(filename)?;
            let docs: Vec<_> = (0..count)
                .map(|i| {
                    let text_idx = i % texts.len();
                    serde_json::json!({
                        "id": i as u64,
                        "text": texts[text_idx]
                    })
                })
                .collect();
            writeln!(file, "{}", serde_json::to_string(&docs)?)?;
        }
        "CSV" => {
            let mut file = File::create(filename)?;
            writeln!(file, "id,text")?;
            for i in 0..count {
                let text_idx = i % texts.len();
                writeln!(file, "{},{}", i, texts[text_idx])?;
            }
        }
        "Plain Text" => {
            let mut file = File::create(filename)?;
            for i in 0..count {
                let text_idx = i % texts.len();
                writeln!(file, "{}", texts[text_idx])?;
            }
        }
        _ => return Err("Unknown format".into()),
    }

    Ok(())
}
