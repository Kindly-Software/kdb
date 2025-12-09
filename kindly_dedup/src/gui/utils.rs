//! Background processing utilities

use crate::gui::messages::{DedupResults, ExecutionMode};
use crate::facade::{Dedup, DedupMode, FacadeError};
use crate::protection::audit::{log_security_event, SecurityEventType};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;

/// Processing phases for progress tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ProcessingPhase {
    Idle = 0,
    Loading = 1,         // Loading documents from file
    Computing = 2,       // Computing MinHash signatures
    FindingDuplicates = 3, // Finding duplicate pairs (slow)
    WritingOutput = 4,   // Writing deduplicated output
}

impl ProcessingPhase {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Loading,
            2 => Self::Computing,
            3 => Self::FindingDuplicates,
            4 => Self::WritingOutput,
            _ => Self::Idle,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Loading => "Loading documents...",
            Self::Computing => "Computing signatures...",
            Self::FindingDuplicates => "Finding duplicates...",
            Self::WritingOutput => "Writing output...",
        }
    }
}

/// Shared progress data between background thread and UI
pub struct ProgressData {
    pub total_docs: AtomicU64,
    pub processed_docs: AtomicU64,
    pub found_duplicates: AtomicU64,
    pub is_complete: AtomicBool,
    pub is_paused: AtomicBool,
    pub phase: AtomicU8,
    /// Generation counter to detect stale updates from cancelled runs
    /// Each new dedup run increments this; background threads check their
    /// generation matches before writing updates
    pub generation: AtomicU64,
}

impl ProgressData {
    pub fn new() -> Self {
        Self {
            total_docs: AtomicU64::new(0),
            processed_docs: AtomicU64::new(0),
            found_duplicates: AtomicU64::new(0),
            is_complete: AtomicBool::new(false),
            is_paused: AtomicBool::new(false),
            phase: AtomicU8::new(ProcessingPhase::Idle as u8),
            generation: AtomicU64::new(0),
        }
    }

    pub fn reset(&self) {
        self.total_docs.store(0, Ordering::Relaxed);
        self.processed_docs.store(0, Ordering::Relaxed);
        self.found_duplicates.store(0, Ordering::Relaxed);
        self.is_complete.store(false, Ordering::Relaxed);
        self.is_paused.store(false, Ordering::Relaxed);
        self.phase.store(ProcessingPhase::Idle as u8, Ordering::Relaxed);
        // Note: generation is NOT reset here - it's incremented by start_new_run()
    }

    /// Start a new dedup run - increments generation and returns the new value
    /// Background threads should capture this at start and check it periodically
    pub fn start_new_run(&self) -> u64 {
        self.reset();
        self.generation.fetch_add(1, Ordering::SeqCst)
    }

    /// Check if this generation is still current (not cancelled/replaced)
    pub fn is_current_generation(&self, gen: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == gen
    }

    pub fn set_phase(&self, phase: ProcessingPhase) {
        self.phase.store(phase as u8, Ordering::Relaxed);
    }

    pub fn get_phase(&self) -> ProcessingPhase {
        ProcessingPhase::from_u8(self.phase.load(Ordering::Relaxed))
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

/// Convert GUI ExecutionMode to Facade DedupMode
///
/// Note: Auto mode uses CPU only (safest default) because GPU mode can
/// cause unrecoverable crashes via abort() that bypass catch_unwind.
/// Users who want GPU must explicitly select "GPU Accelerated" mode.
fn to_dedup_mode(mode: ExecutionMode) -> DedupMode {
    match mode {
        // Auto uses CPU-only for safety - GPU can abort() instead of panic
        ExecutionMode::Auto => DedupMode::CpuStreaming,
        ExecutionMode::Cpu => DedupMode::CpuStreaming,
        #[cfg(feature = "gpu-hybrid")]
        ExecutionMode::Gpu => DedupMode::Gpu,
        #[cfg(not(feature = "gpu-hybrid"))]
        ExecutionMode::Gpu => DedupMode::CpuStreaming, // Fallback to CPU
    }
}

/// Run deduplication in background (blocking)
///
/// # Arguments
/// * `file_path` - Path to the input corpus file
/// * `threshold` - Similarity threshold (0.0-1.0)
/// * `mode` - Execution mode (Auto, CPU, GPU, Persistent)
/// * `progress` - Shared progress data for UI updates
/// * `cancel_flag` - Optional atomic flag to signal cancellation
pub fn run_dedup_sync(
    file_path: PathBuf,
    threshold: f32,
    mode: ExecutionMode,
    progress: Arc<ProgressData>,
    cancel_flag: Option<Arc<AtomicBool>>,
) -> Result<DedupResults, String> {
    use std::time::Instant;

    // Helper to check if cancellation was requested
    let is_cancelled = || {
        cancel_flag.as_ref().map_or(false, |f| f.load(Ordering::Relaxed))
    };

    // Helper to wait while paused (returns Err if cancelled during pause)
    let wait_if_paused = || -> Result<(), String> {
        while progress.is_paused.load(Ordering::Relaxed) {
            // Check for cancellation while paused
            if is_cancelled() {
                return Err("Deduplication cancelled by user".to_string());
            }
            // Sleep briefly to avoid busy-waiting (10ms)
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        Ok(())
    };

    // Capture our generation - if it changes, we've been superseded by a new run
    // This prevents race conditions when user cancels and starts a new run quickly
    let our_generation = progress.generation.load(Ordering::SeqCst);

    // Helper to check if our run has been superseded by a new one
    let is_stale = || {
        !progress.is_current_generation(our_generation)
    };

    eprintln!("[DEBUG] [dedup] run_dedup_sync ENTERED - thread: {:?}, generation: {}", std::thread::current().id(), our_generation);
    eprintln!("[DEBUG] [dedup] file_path: {:?}", file_path);
    eprintln!("[DEBUG] [dedup] threshold: {}, mode: {:?}", threshold, mode);

    let start_time = Instant::now();
    eprintln!("[DEBUG] [dedup] Starting full corpus processing...");

    // Q34 Audit: Log dedup start event (<200ns overhead)
    let _ = log_security_event(
        SecurityEventType::DemoTierStarted,
        "gui_user",
        None,
        0,
        &format!("GUI Dedup | File: {} | Threshold: {:.0}%",
                 file_path.display(), threshold * 100.0),
    );

    // Phase 1: Loading documents from file
    progress.set_phase(ProcessingPhase::Loading);

    let file = File::open(&file_path).map_err(|e| format!("Failed to open file: {}", e))?;
    // Pre-allocate based on file size (~200 bytes per doc average) to prevent OOM
    let file_size = file.metadata().map(|m| m.len()).unwrap_or(1_000_000);
    let estimated_capacity = (file_size / 200).max(1000) as usize;
    let reader = BufReader::new(file);

    let mut documents = Vec::with_capacity(estimated_capacity);
    for (idx, line) in reader.lines().enumerate() {
        // Check for cancellation/pause/staleness every 1000 lines during loading
        if idx % 1000 == 0 {
            if is_cancelled() || is_stale() {
                eprintln!("[DEBUG] [dedup] Exiting (gen {}): cancelled={}, stale={}",
                    our_generation, is_cancelled(), is_stale());
                return Err("Deduplication cancelled by user".to_string());
            }
            wait_if_paused()?;
        }

        let line = line.map_err(|_|
            format!("Unsupported file format. Please use: JSONL, JSON, CSV, TSV, or TXT.\n\nNeed another format? Contact samuel@kindly.software")
        )?;
        let line = line.trim();
        if !line.is_empty() {
            // Try to parse as JSON
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(text) = json.get("text").and_then(|v| v.as_str()) {
                    documents.push((idx as u64, text.to_string()));
                }
            } else {
                // Plain text
                documents.push((idx as u64, line.to_string()));
            }
        }
    }

    let num_docs = documents.len();
    eprintln!("[DEBUG] [dedup] Parsed {} documents from corpus", num_docs);

    // Validate we have documents to process
    if num_docs == 0 {
        return Err(format!(
            "No valid documents found in file.\n\n\
            Expected formats:\n\
            • JSONL: One JSON object per line with a \"text\" field\n\
            • Plain text: One document per line\n\n\
            Your file appears to be: {}\n\n\
            Tip: For JSONL, each line should look like:\n\
            {{\"text\": \"Your document content here\"}}",
            file_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
        ));
    }

    progress.total_docs.store(num_docs as u64, Ordering::Relaxed);

    // Phase 2: Computing MinHash signatures
    progress.set_phase(ProcessingPhase::Computing);

    // 2. Create Facade deduplicator (auto-selects best implementation)
    let dedup_mode = to_dedup_mode(mode);

    // Check GPU availability
    #[cfg(feature = "gpu-hybrid")]
    let gpu_available = crate::gpu::is_gpu_available();
    #[cfg(not(feature = "gpu-hybrid"))]
    let gpu_available = false;

    // Create deduplicator with panic protection for GPU mode
    // GPU operations (shader compilation, buffer creation) can panic on some drivers
    let (mut dedup, actual_mode) = {
        // Try GPU mode first if requested, with panic protection
        #[cfg(feature = "gpu-hybrid")]
        if dedup_mode == DedupMode::Gpu || (dedup_mode == DedupMode::Auto && gpu_available) {
            // Wrap GPU creation in catch_unwind to prevent thread crash
            let gpu_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                Dedup::with_mode(DedupMode::Gpu, num_docs)
            }));

            match gpu_result {
                Ok(Ok(dedup)) => {
                    eprintln!("[DEBUG] [dedup] GPU mode initialized successfully");
                    (dedup, ExecutionMode::Gpu)
                },
                Ok(Err(e)) => {
                    // GPU init returned an error, fall back to CPU
                    eprintln!("[DEBUG] [dedup] GPU mode failed ({}), falling back to CPU", e);
                    let dedup = Dedup::with_mode(DedupMode::CpuStreaming, num_docs)
                        .map_err(|e| format!("Failed to create deduplicator: {}", e))?;
                    (dedup, ExecutionMode::Cpu)
                },
                Err(_panic) => {
                    // GPU init panicked, fall back to CPU
                    eprintln!("[DEBUG] [dedup] GPU mode panicked during initialization, falling back to CPU");
                    let dedup = Dedup::with_mode(DedupMode::CpuStreaming, num_docs)
                        .map_err(|e| format!("Failed to create deduplicator: {}", e))?;
                    (dedup, ExecutionMode::Cpu)
                }
            }
        } else {
            // CPU mode requested or no GPU available
            let dedup = Dedup::with_mode(DedupMode::CpuStreaming, num_docs)
                .map_err(|e| format!("Failed to create deduplicator: {}", e))?;
            (dedup, ExecutionMode::Cpu)
        }

        #[cfg(not(feature = "gpu-hybrid"))]
        {
            let dedup = Dedup::with_mode(dedup_mode, num_docs)
                .map_err(|e| format!("Failed to create deduplicator: {}", e))?;
            (dedup, ExecutionMode::Cpu)
        }
    };

    // 3. Add documents with progress updates
    for (idx, (doc_id, text)) in documents.iter().enumerate() {
        // Check for cancellation/pause/staleness every 100 documents
        if idx % 100 == 0 {
            if is_cancelled() || is_stale() {
                eprintln!("[DEBUG] [dedup] Exiting add_document loop (gen {}): cancelled={}, stale={}",
                    our_generation, is_cancelled(), is_stale());
                return Err("Deduplication cancelled by user".to_string());
            }
            wait_if_paused()?;
        }

        dedup.add_document(*doc_id, text)
            .map_err(|e| format!("Failed to add document {}: {}", doc_id, e))?;

        // Update progress every 1% or 100 docs
        if idx % 100 == 0 || (num_docs > 0 && (idx * 100 / num_docs) != ((idx.saturating_sub(1)) * 100 / num_docs)) {
            progress.processed_docs.store(idx as u64, Ordering::Relaxed);
        }
    }
    progress.processed_docs.store(num_docs as u64, Ordering::Relaxed);

    // Phase 3: Finding duplicates (slow phase)
    progress.set_phase(ProcessingPhase::FindingDuplicates);

    // Check for cancellation/pause/staleness before starting slow phase
    if is_cancelled() || is_stale() {
        eprintln!("[DEBUG] [dedup] Exiting before find_duplicates (gen {}): cancelled={}, stale={}",
            our_generation, is_cancelled(), is_stale());
        return Err("Deduplication cancelled by user".to_string());
    }
    wait_if_paused()?;

    // 4. Find duplicates
    let clusters = dedup
        .find_duplicates(threshold as f64)
        .map_err(|e| format!("Deduplication failed: {}", e))?;

    // 5. Calculate unique documents (first from each cluster is kept)
    let mut duplicate_ids: HashSet<u64> = HashSet::new();
    for cluster in &clusters {
        // Skip first document in each cluster (it's unique)
        for &doc_id in cluster.iter().skip(1) {
            duplicate_ids.insert(doc_id);
        }
    }

    progress
        .found_duplicates
        .store(duplicate_ids.len() as u64, Ordering::Relaxed);

    // Phase 4: Writing output file
    progress.set_phase(ProcessingPhase::WritingOutput);

    // 6. Write output file (unique documents only)
    let output_path = file_path.with_file_name(format!(
        "{}_dedup.jsonl",
        file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output")
    ));

    let mut output_file = File::create(&output_path).map_err(|e| format!("Failed to create output file: {}", e))?;

    for (doc_id, text) in &documents {
        if !duplicate_ids.contains(doc_id) {
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

    // Q34 Audit: Log dedup completion event (<200ns overhead)
    let _ = log_security_event(
        SecurityEventType::DemoTierCompleted,
        "gui_user",
        None,
        0,
        &format!("GUI Dedup Complete | Docs: {} | Dups: {} | Time: {:.1}s | Throughput: {:.0}/sec",
                 num_docs, duplicate_ids.len(), elapsed_sec, throughput),
    );

    progress.is_complete.store(true, Ordering::Relaxed);

    Ok(DedupResults {
        total_documents: num_docs,
        unique_documents: num_docs - duplicate_ids.len(),
        duplicate_clusters: clusters.len(),
        processing_time_sec: elapsed_sec,
        throughput_docs_sec: throughput,
        speedup_vs_python: speedup,
        output_file: output_path,
        actual_mode,
        gpu_available,
    })
}
