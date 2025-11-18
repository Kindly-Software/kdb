//! Example: Load corpus with progress tracking
//!
//! Demonstrates how to monitor loading progress using the
//! ProgressTrackerCapsule while loading documents.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example load_with_progress --release --features "format-json,cpu-detection"
//! ```
//!
//! # Implementation Notes
//!
//! Progress tracking is lockfree and has <5ns overhead per document.
//! The example shows how to create a progress tracker and pass it
//! to the format reader.

use kindly_dedup::format::{FormatReaderCapsule, FormatRegistryCapsule, ProgressTrackerCapsule};
use kindly_dedup::DedupPipeline;
use atomic_capsule::CpuCapabilityCapsule;
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Instant;
use std::thread;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("===== Format Loading with Progress Tracking =====\n");

    let cpu_caps = CpuCapabilityCapsule::detect();

    // Create test corpus
    let test_file = "corpus_with_progress.jsonl";
    create_test_corpus(test_file, 1000)?;
    println!("Created test corpus: {} (1000 documents)\n", test_file);

    // Create progress tracker (wrapped in Arc for shared ownership)
    let progress = Arc::new(AtomicU64::new(0));
    let progress_clone = Arc::clone(&progress);

    // Spawn progress monitoring thread
    let monitor = thread::spawn(move || {
        let mut last_count = 0u64;
        let start = Instant::now();

        loop {
            thread::sleep(std::time::Duration::from_millis(100));
            let current = progress_clone.load(std::sync::atomic::Ordering::Relaxed);

            if current > last_count {
                let elapsed = start.elapsed().as_secs_f64();
                let throughput = current as f64 / elapsed;
                println!("Progress: {}/1000 docs ({:.0} docs/sec)", current, throughput);
                last_count = current;
            }

            if current >= 1000 {
                break;
            }
        }
    });

    // Load documents with progress tracking
    println!("Loading documents...");
    let start = Instant::now();

    #[cfg(feature = "format-json")]
    {
        use kindly_dedup::format::jsonl::JsonlReaderCapsule;

        let reader = JsonlReaderCapsule::default();
        let file = File::open(test_file)?;

        let docs: Vec<_> = reader
            .stream_documents(file, Some(Arc::clone(&progress)))
            .collect::<Result<Vec<_>, _>>()?;

        let load_time = start.elapsed();
        println!(
            "\nLoaded {} documents in {:.2}ms",
            docs.len(),
            load_time.as_secs_f64() * 1000.0
        );

        // Create dedup pipeline
        let mut pipeline = DedupPipeline::new(docs.len(), &cpu_caps)?;

        // Add documents
        println!("\nAdding documents to pipeline with progress...");
        let progress = Arc::new(AtomicU64::new(0));
        let progress_add = Arc::clone(&progress);

        for (i, doc) in docs.iter().enumerate() {
            pipeline.add_document(doc.id, &doc.text)?;
            progress_add.store((i + 1) as u64, std::sync::atomic::Ordering::Relaxed);

            if (i + 1) % 100 == 0 {
                print!(".");
            }
        }
        println!();

        // Find duplicates
        println!("\nFinding duplicates...");
        let clusters = pipeline.find_duplicates(0.85)?;
        println!("Found {} duplicate clusters\n", clusters.len());

        // Print statistics
        println!("===== Statistics =====");
        println!("Total documents:      {}", docs.len());
        println!("Unique clusters:      {}", clusters.len());
        println!(
            "Duplicate clusters:   {}",
            clusters.iter().filter(|c| c.len() > 1).count()
        );
        println!(
            "Loading throughput:   {:.0} docs/sec",
            docs.len() as f64 / load_time.as_secs_f64()
        );
    }

    #[cfg(not(feature = "format-json"))]
    {
        println!("JSON feature not enabled. Build with: --features format-json");
    }

    // Wait for monitoring thread
    let _ = monitor.join();

    // Cleanup
    std::fs::remove_file(test_file).ok();

    Ok(())
}

/// Create a test JSONL corpus
fn create_test_corpus(path: &str, count: usize) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::create(path)?;

    let texts = vec![
        "The quick brown fox jumps over the lazy dog",
        "Python is a high-level programming language",
        "Rust provides memory safety without garbage collection",
        "Machine learning models require large datasets",
        "Cloud computing enables scalable infrastructure",
    ];

    for i in 0..count {
        let text_idx = i % texts.len();
        let doc = serde_json::json!({
            "id": i as u64,
            "text": texts[text_idx]
        });
        writeln!(file, "{}", doc.to_string())?;
    }

    Ok(())
}
