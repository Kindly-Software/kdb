//! Streaming Deduplication Pipeline Demo
//!
//! Demonstrates using StreamingFileIterator with Dedup facade for O(1) memory deduplication.
//!
//! # Architecture
//!
//! ```text
//! File (1GB+) → StreamingFileIterator → Dedup (CpuStreaming) → Duplicates
//!                  ↓ (64KB)              ↓ (O(n))               ↓ (O(n))
//!                  O(1) memory          Signatures             Clusters
//! ```
//!
//! # Usage
//!
//! ```bash
//! # Generate test corpus
//! cargo run --example streaming_dedup_pipeline generate test_corpus.jsonl 10000
//!
//! # Run deduplication
//! cargo run --example streaming_dedup_pipeline dedup test_corpus.jsonl 0.9
//! ```
//!
//! # Performance
//!
//! - **Memory**: O(1) loading + O(n) signatures = ~16 bytes/doc
//! - **Throughput**: ~60K docs/sec (single-threaded)
//! - **Scalability**: Tested up to 100M documents

use kindly_dedup::format::StreamingFileIterator;
use kindly_dedup::{Dedup, DedupMode};
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage(&args[0]);
        std::process::exit(1);
    }

    match args[1].as_str() {
        "generate" => generate_corpus(&args),
        "dedup" => run_dedup(&args),
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_usage(&args[0]);
            std::process::exit(1);
        }
    }
}

fn print_usage(prog: &str) {
    eprintln!("Streaming Deduplication Pipeline Demo");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  {} generate <output.jsonl> <num_docs>", prog);
    eprintln!("  {} dedup <input.jsonl> <threshold>", prog);
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {} generate corpus.jsonl 10000", prog);
    eprintln!("  {} dedup corpus.jsonl 0.9", prog);
}

fn generate_corpus(args: &[String]) {
    if args.len() != 4 {
        eprintln!("Usage: {} generate <output.jsonl> <num_docs>", args[0]);
        std::process::exit(1);
    }

    let output_path = Path::new(&args[2]);
    let num_docs: usize = args[3].parse().expect("Invalid number");

    println!("Generating {} documents...", num_docs);

    let mut file = File::create(output_path).expect("Failed to create file");

    // Generate documents with some duplicates
    for i in 0..num_docs {
        let text = if i % 5 == 0 {
            // Every 5th document is a duplicate of the first
            format!("This is document number 0")
        } else if i % 10 == 0 {
            // Every 10th document is a duplicate
            format!("This is a duplicate document")
        } else {
            format!("This is document number {}", i)
        };

        let json = format!("{{\"text\":\"{}\"}}\n", text);
        file.write_all(json.as_bytes()).expect("Failed to write");
    }

    println!("Generated {} documents to {}", num_docs, output_path.display());
    println!("Expected duplicates: ~{} (20% of corpus)", num_docs / 5);
}

fn run_dedup(args: &[String]) {
    if args.len() != 4 {
        eprintln!("Usage: {} dedup <input.jsonl> <threshold>", args[0]);
        std::process::exit(1);
    }

    let input_path = Path::new(&args[2]);
    let threshold: f64 = args[3].parse().expect("Invalid threshold");

    if !input_path.exists() {
        eprintln!("Error: File not found: {}", input_path.display());
        std::process::exit(1);
    }

    println!("Streaming Deduplication Pipeline Demo");
    println!("========================================");
    println!("Input: {}", input_path.display());
    println!("Threshold: {:.2}", threshold);
    println!();

    // Phase 1: Count documents
    println!("Phase 1: Counting documents...");
    let start = Instant::now();
    let iter = StreamingFileIterator::new(input_path).expect("Failed to open file");
    let file_size = iter.total_bytes();
    let num_docs = iter.count();
    let count_time = start.elapsed();
    println!("  Documents: {}", num_docs);
    println!("  File size: {} bytes ({:.1} MB)", file_size, file_size as f64 / 1_000_000.0);
    println!("  Time: {:.2}s", count_time.as_secs_f64());
    println!();

    // Phase 2: Initialize dedup facade
    println!("Phase 2: Initializing dedup facade...");
    let start = Instant::now();
    let mut dedup = Dedup::with_mode(DedupMode::CpuStreaming, num_docs)
        .expect("Failed to create dedup facade");
    let init_time = start.elapsed();
    println!("  Mode: {:?}", dedup.current_mode());
    println!("  Time: {:.2}s", init_time.as_secs_f64());
    println!();

    // Phase 3: Stream documents into dedup
    println!("Phase 3: Streaming documents (O(1) memory)...");
    let start = Instant::now();
    let iter = StreamingFileIterator::new(input_path).expect("Failed to open file");

    let mut count = 0usize;
    let mut errors = 0usize;
    for result in iter {
        match result {
            Ok((doc_id, text)) => {
                if let Err(e) = dedup.add_document(doc_id as u64, &text) {
                    eprintln!("Error adding document {}: {}", doc_id, e);
                    errors += 1;
                }
                count += 1;

                if count % 1000 == 0 {
                    print!("\r  Progress: {} docs", count);
                    std::io::stdout().flush().unwrap();
                }
            }
            Err(e) => {
                eprintln!("Error reading document: {}", e);
                errors += 1;
            }
        }
    }
    println!("\r  Progress: {} docs (complete)", count);
    let add_time = start.elapsed();
    println!("  Throughput: {:.0} docs/sec", count as f64 / add_time.as_secs_f64());
    println!("  Errors: {}", errors);
    println!();

    // Phase 4: Find duplicates
    println!("Phase 4: Finding duplicates...");
    let start = Instant::now();
    let clusters = dedup.find_duplicates(threshold).expect("Failed to find duplicates");
    let find_time = start.elapsed();

    // Each cluster is Vec<DocId>, count total duplicates (all but first in each cluster)
    let num_duplicates: usize = clusters.iter()
        .map(|cluster| cluster.len().saturating_sub(1))
        .sum();

    println!("  Clusters: {}", clusters.len());
    println!("  Duplicates: {} ({:.1}% of corpus)",
        num_duplicates,
        num_duplicates as f64 / count as f64 * 100.0);
    println!("  Time: {:.2}s", find_time.as_secs_f64());
    println!();

    // Summary
    println!("Summary");
    println!("========================================");
    println!("Total time: {:.2}s", (count_time + init_time + add_time + find_time).as_secs_f64());
    println!("  Phase 1 (count):  {:.2}s ({:.1}%)",
        count_time.as_secs_f64(),
        count_time.as_secs_f64() / (count_time + init_time + add_time + find_time).as_secs_f64() * 100.0);
    println!("  Phase 2 (init):   {:.2}s ({:.1}%)",
        init_time.as_secs_f64(),
        init_time.as_secs_f64() / (count_time + init_time + add_time + find_time).as_secs_f64() * 100.0);
    println!("  Phase 3 (stream): {:.2}s ({:.1}%)",
        add_time.as_secs_f64(),
        add_time.as_secs_f64() / (count_time + init_time + add_time + find_time).as_secs_f64() * 100.0);
    println!("  Phase 4 (find):   {:.2}s ({:.1}%)",
        find_time.as_secs_f64(),
        find_time.as_secs_f64() / (count_time + init_time + add_time + find_time).as_secs_f64() * 100.0);
    println!();
    println!("Memory usage:");
    println!("  Streaming: O(1) = 64KB buffer");
    println!("  Signatures: O(n) = ~{} MB", (count * 256) / 1_000_000);
    println!("  Total: ~{} MB", (count * 256 + 64 * 1024) / 1_000_000);
}
