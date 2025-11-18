//! Example: Load CSV corpus with custom schema mapping
//!
//! Demonstrates CSV-specific configuration for custom column ordering.
//! Shows how to map document IDs and text from arbitrary CSV columns.
//!
//! # Usage
//!
//! ```bash
//! cargo build --example load_csv --release --features "format-csv,benchmarking,cpu-detection"
//! cargo run --example load_csv --release --features "format-csv,benchmarking,cpu-detection"
//! ```
//!
//! # Configuration
//!
//! Customize the CSV schema by modifying the `CsvConfig`:
//!
//! ```rust,ignore
//! use kindly_dedup::format::CsvConfig;
//!
//! let config = CsvConfig {
//!     id_column: 0,           // Column containing document ID
//!     text_column: 1,         // Column containing document text
//!     has_headers: true,      // First row is header
//!     delimiter: b',',        // Delimiter character
//! };
//! ```

use atomic_capsule::CpuCapabilityCapsule;
use kindly_dedup::format::{CsvConfig, FormatReaderCapsule};
use kindly_dedup::DedupPipeline;
use std::fs::File;
use std::io::Write;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("===== Format Loading Example: CSV =====\n");

    // Detect CPU capabilities
    let cpu_caps = CpuCapabilityCapsule::detect();
    println!("CPU Capabilities: {} cores\n", cpu_caps.logical_cores);

    // Create test CSV corpus
    let test_file = "test_corpus.csv";
    create_test_corpus(test_file, 1000)?;
    println!("Created test corpus: {} (1000 rows)\n", test_file);

    // Configure CSV schema
    let config = CsvConfig {
        id_column: 0,
        text_column: 1,
        has_headers: true,
        delimiter: b',',
    };

    // Create CSV reader with custom config
    #[cfg(feature = "format-csv")]
    {
        use kindly_dedup::format::csv::CsvReaderCapsule;

        let reader = CsvReaderCapsule::new(config);

        // Load documents
        println!("Loading CSV with custom schema...");
        let start = Instant::now();

        let file = File::open(test_file)?;
        let docs: Vec<_> = reader.stream_documents(file, None).collect::<Result<Vec<_>, _>>()?;

        let load_time = start.elapsed();

        println!(
            "Loaded {} documents in {:.2}ms",
            docs.len(),
            load_time.as_secs_f64() * 1000.0
        );
        println!(
            "Throughput: {:.0} docs/sec\n",
            docs.len() as f64 / load_time.as_secs_f64()
        );

        // Create dedup pipeline
        let mut pipeline = DedupPipeline::new(docs.len(), &cpu_caps)?;

        // Add documents
        println!("Adding {} documents to pipeline...", docs.len());
        let start = Instant::now();
        for (i, doc) in docs.iter().enumerate() {
            pipeline.add_document(doc.id, &doc.text)?;
            if (i + 1) % 100 == 0 {
                print!(".");
            }
        }
        let add_time = start.elapsed();
        println!(
            "\nAdded in {:.2}ms ({:.0} docs/sec)\n",
            add_time.as_secs_f64() * 1000.0,
            docs.len() as f64 / add_time.as_secs_f64()
        );

        // Find duplicates
        println!("Finding duplicates...");
        let start = Instant::now();
        let clusters = pipeline.find_duplicates(0.85)?;
        let find_time = start.elapsed();

        println!(
            "Found {} clusters in {:.2}ms\n",
            clusters.len(),
            find_time.as_secs_f64() * 1000.0
        );

        // Print summary
        println!("===== Summary =====");
        println!("Total documents:   {}", docs.len());
        println!("Unique clusters:   {}", clusters.len());
        println!("Duplicates found:  {}", clusters.iter().filter(|c| c.len() > 1).count());
    }

    #[cfg(not(feature = "format-csv"))]
    {
        println!("CSV feature not enabled. Build with: --features format-csv");
    }

    // Cleanup
    std::fs::remove_file(test_file).ok();

    Ok(())
}

/// Create a test CSV corpus
fn create_test_corpus(path: &str, count: usize) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::create(path)?;

    // Write header
    writeln!(file, "id,text,category")?;

    // Create corpus with some duplicates
    let texts = vec![
        "The quick brown fox jumps over the lazy dog",
        "Python is a high-level programming language",
        "Rust provides memory safety without garbage collection",
        "Machine learning models require large datasets",
        "Cloud computing enables scalable infrastructure",
    ];

    for i in 0..count {
        let text_idx = i % texts.len();
        let category = if i % 2 == 0 { "training" } else { "validation" };
        writeln!(file, "{},{},{}", i, texts[text_idx], category)?;
    }

    Ok(())
}
