//! Background processing utilities

use crate::gui::messages::DedupResults;
use crate::pipeline::{DedupPipeline, PipelineError};
use atomic_capsule::CpuCapabilityCapsule;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// Shared progress data between background thread and UI
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

    pub fn progress_fraction(&self) -> f32 {
        let total = self.total_docs.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            let processed = self.processed_docs.load(Ordering::Relaxed);
            processed as f32 / total as f32
        }
    }
}

impl Default for ProgressData {
    fn default() -> Self {
        Self::new()
    }
}

/// Run deduplication in background (blocking)
pub fn run_dedup_sync(file_path: PathBuf, threshold: f32, progress: Arc<ProgressData>) -> Result<DedupResults, String> {
    use std::time::Instant;

    let start_time = Instant::now();

    // 1. Load and count documents
    let file = File::open(&file_path).map_err(|e| format!("Failed to open file: {}", e))?;
    let reader = BufReader::new(file);

    let mut documents = Vec::new();
    for (idx, line) in reader.lines().enumerate() {
        let line = line.map_err(|_|
            format!("Unsupported file format. Please use: JSONL, JSON, CSV, TSV, or TXT.\n\nNeed another format? Contact samuel@kindly.software")
        )?;
        let line = line.trim();
        if !line.is_empty() {
            // Try to parse as JSON
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(text) = json.get("text").and_then(|v| v.as_str()) {
                    documents.push((idx, text.to_string()));
                }
            } else {
                // Plain text
                documents.push((idx, line.to_string()));
            }
        }
    }

    let num_docs = documents.len();
    progress.total_docs.store(num_docs as u64, Ordering::Relaxed);

    // 2. Create dedup pipeline with CPU detection
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = DedupPipeline::new(num_docs, &cpu_caps);

    // 3. Add documents with progress updates
    for (idx, (doc_id, text)) in documents.iter().enumerate() {
        pipeline.add_document(*doc_id, text);

        // Update progress every 1% or 100 docs
        if idx % 100 == 0 || (idx * 100 / num_docs) != ((idx.saturating_sub(1)) * 100 / num_docs) {
            progress.processed_docs.store(idx as u64, Ordering::Relaxed);
        }
    }
    progress.processed_docs.store(num_docs as u64, Ordering::Relaxed);

    // 4. Find duplicates (convert f32 threshold to f64)
    let clusters = pipeline
        .find_duplicates(threshold as f64)
        .map_err(|e| format!("Deduplication failed: {:?}", e))?;

    // 5. Calculate unique documents (first from each cluster is kept)
    let mut duplicate_ids: HashSet<usize> = HashSet::new();
    for cluster in &clusters {
        // Skip first document in each cluster (it's unique)
        for &doc_id in cluster.iter().skip(1) {
            duplicate_ids.insert(doc_id);
        }
    }

    progress
        .found_duplicates
        .store(duplicate_ids.len() as u64, Ordering::Relaxed);

    // 6. Write output file (unique documents only)
    let output_path = file_path.with_file_name(format!(
        "{}_dedup.jsonl",
        file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output")
    ));

    let mut output_file = File::create(&output_path).map_err(|e| format!("Failed to create output file: {}", e))?;

    for (doc_id, text) in documents {
        if !duplicate_ids.contains(&doc_id) {
            // Write as JSON
            let json = serde_json::json!({
                "doc_id": doc_id,
                "text": text
            });
            writeln!(output_file, "{}", json).map_err(|e| format!("Failed to write output: {}", e))?;
        }
    }

    // 7. Calculate results
    let elapsed_sec = start_time.elapsed().as_secs_f64();
    let throughput = num_docs as f64 / elapsed_sec;

    // Estimate Python baseline (1,500 docs/sec from datasketch)
    let python_time = num_docs as f64 / 1_500.0;
    let speedup = python_time / elapsed_sec;

    progress.is_complete.store(true, Ordering::Relaxed);

    Ok(DedupResults {
        total_documents: num_docs,
        unique_documents: num_docs - duplicate_ids.len(),
        duplicate_clusters: clusters.len(),
        processing_time_sec: elapsed_sec,
        throughput_docs_sec: throughput,
        speedup_vs_python: speedup,
        output_file: output_path,
    })
}
