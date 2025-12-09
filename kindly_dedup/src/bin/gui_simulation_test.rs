//! GUI Simulation Test - Tests run_dedup_sync with progress tracking
//!
//! This simulates EXACTLY what the GUI does to isolate heap corruption.
//! If this crashes, the issue is in run_dedup_sync or progress atomics.
//! If this works, the issue is in Iced GUI framework interaction.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

/// Shared progress data - EXACT copy from gui/utils.rs
pub struct ProgressData {
    pub total_docs: AtomicU64,
    pub processed_docs: AtomicU64,
    pub found_duplicates: AtomicU64,
    pub is_complete: AtomicBool,
}

impl ProgressData {
    pub fn new() -> Self {
        Self {
            total_docs: AtomicU64::new(0),
            processed_docs: AtomicU64::new(0),
            found_duplicates: AtomicU64::new(0),
            is_complete: AtomicBool::new(false),
        }
    }

    pub fn reset(&self) {
        self.total_docs.store(0, Ordering::Relaxed);
        self.processed_docs.store(0, Ordering::Relaxed);
        self.found_duplicates.store(0, Ordering::Relaxed);
        self.is_complete.store(false, Ordering::Relaxed);
    }
}

fn main() {
    eprintln!("=== GUI SIMULATION TEST ===");
    eprintln!("Simulating EXACT GUI behavior with ProgressData atomics\n");

    let corpus_path = PathBuf::from("/home/samuel/Downloads/corpus_1m.jsonl");

    if !corpus_path.exists() {
        eprintln!("ERROR: Corpus file not found: {:?}", corpus_path);
        std::process::exit(1);
    }

    // Full stress test: 30K-100K docs (validates MPMC queue fix)
    let test_sizes = [30_000, 50_000, 75_000, 100_000];

    for &size in &test_sizes {
        eprintln!("\n=== Testing with {} documents ===", size);

        match run_test(size, &corpus_path) {
            Ok(elapsed) => {
                let throughput = size as f64 / elapsed;
                eprintln!(
                    "✅ SUCCESS: {} docs in {:.2}s ({:.0} docs/sec)",
                    size, elapsed, throughput
                );
            }
            Err(e) => {
                eprintln!("❌ FAILED at {} docs: {}", size, e);
                eprintln!("\n=== CRASH THRESHOLD FOUND: {} docs ===", size);
                std::process::exit(1);
            }
        }
    }

    eprintln!("\n=== ALL TESTS PASSED ===");
    eprintln!("No heap corruption detected up to 100K documents");
    eprintln!("Issue is likely in Iced GUI framework, not run_dedup_sync");
}

fn run_test(max_docs: usize, corpus_path: &PathBuf) -> Result<f64, String> {
    use kindly_dedup::facade::{Dedup, DedupMode};
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let start = Instant::now();

    // Create progress data EXACTLY like the GUI does
    let progress = Arc::new(ProgressData::new());
    progress.reset();

    // Simulate progress reads (like GUI timer would do)
    let progress_reader = Arc::clone(&progress);
    let _reader_thread = std::thread::spawn(move || {
        while !progress_reader.is_complete.load(Ordering::Relaxed) {
            let _total = progress_reader.total_docs.load(Ordering::Relaxed);
            let _processed = progress_reader.processed_docs.load(Ordering::Relaxed);
            let _dups = progress_reader.found_duplicates.load(Ordering::Relaxed);
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });

    // Open file
    let file = File::open(corpus_path)
        .map_err(|e| format!("Failed to open file: {}", e))?;

    // Parse documents - simplified JSON extraction (no serde dependency)
    let reader = BufReader::new(file);
    let mut documents: Vec<(u64, String)> = Vec::new();

    for (idx, line) in reader.lines().take(max_docs).enumerate() {
        let line = line.map_err(|e| format!("Read error at line {}: {}", idx, e))?;
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            // Simple JSON text extraction: look for "text": "..." pattern
            if let Some(start) = trimmed.find("\"text\":") {
                let after_key = &trimmed[start + 7..];
                // Skip whitespace and opening quote
                let text_start = after_key.find('"').map(|i| i + 1);
                if let Some(ts) = text_start {
                    let rest = &after_key[ts..];
                    // Find closing quote (handle escaped quotes)
                    let mut end = 0;
                    let mut in_escape = false;
                    for (i, c) in rest.chars().enumerate() {
                        if in_escape {
                            in_escape = false;
                            continue;
                        }
                        if c == '\\' {
                            in_escape = true;
                            continue;
                        }
                        if c == '"' {
                            end = i;
                            break;
                        }
                    }
                    let text = &rest[..end];
                    documents.push((idx as u64, text.to_string()));
                }
            } else {
                // Plain text fallback
                documents.push((idx as u64, trimmed.to_string()));
            }
        }
    }
    eprintln!("  Parsed {} documents", documents.len());

    // Create Dedup - EXACTLY matching GUI code
    let num_docs = documents.len();
    let mut dedup = Dedup::with_mode(DedupMode::Auto, num_docs)
        .map_err(|e| format!("Failed to create dedup: {}", e))?;

    // Add documents with progress updates - EXACTLY matching GUI code
    for (idx, (doc_id, text)) in documents.iter().enumerate() {
        dedup.add_document(*doc_id, text)
            .map_err(|e| format!("Failed to add doc {}: {}", doc_id, e))?;

        // Update progress atomics like GUI does
        if idx % 100 == 0 {
            progress.processed_docs.store(idx as u64, Ordering::Relaxed);
        }
    }
    progress.processed_docs.store(num_docs as u64, Ordering::Relaxed);
    eprintln!("  Added {} documents", num_docs);

    // Find duplicates
    let clusters = dedup.find_duplicates(0.85)
        .map_err(|e| format!("find_duplicates failed: {}", e))?;
    eprintln!("  Found {} duplicate clusters", clusters.len());

    // Mark complete - like GUI does
    progress.is_complete.store(true, Ordering::Relaxed);
    progress.found_duplicates.store(clusters.len() as u64, Ordering::Relaxed);

    Ok(start.elapsed().as_secs_f64())
}
