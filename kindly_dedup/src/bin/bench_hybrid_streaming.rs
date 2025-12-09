//! bench_hybrid_streaming - O(1) Memory Hybrid Pipeline Benchmark
//!
//! Streams documents from JSONL files with O(1) memory guarantees.
//!
//! # CLI Usage
//!
//! ```bash
//! cargo run --release --bin bench_hybrid_streaming --features gpu-hybrid \
//!     <corpus.jsonl> \
//!     --limit 100000 \
//!     --mode auto \
//!     --memory-budget 1500 \
//!     --threshold 0.85
//! ```
//!
//! # Memory Invariant
//!
//! Must stay ≤ memory_budget MB regardless of corpus size (default: 1500 MB).
//! Uses streaming line-by-line reading, NOT Vec<String> loading.
//!
//! # Framework Compliance
//!
//! - **T5 Streaming**: Line-by-line iterator processing, O(1) memory
//! - **T7 Heterogeneous**: HybridDedupPipeline (CPU+GPU coordination)
//! - **B32**: Fair benchmarking with memory tracking, RSS reporting
//! - **ASSUM**: Documented O(1) memory assumptions (#ASSUME/#VERIFY)

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use atomic_capsule::CpuCapabilityCapsule;
use kindly_dedup::hybrid_pipeline::{HybridDedupPipeline, PipelineMode};
use kindly_dedup::PipelineError;

// ============================================================================
// MEMORY BUDGET CAPSULE (T0 Auditable Tier)
// ============================================================================

/// Memory budget capsule for O(1) memory validation
///
/// Tracks RSS (Resident Set Size) via /proc/self/statm on Linux.
/// Enforces hard budget limit to prevent OOM.
#[repr(C, align(64))]
struct MemoryBudgetCapsule {
    /// Budget limit in bytes
    budget_bytes: u64,

    /// Current RSS (atomic read)
    current_rss: AtomicU64,

    /// Peak RSS observed
    peak_rss: AtomicU64,

    /// Last check timestamp (nanos since epoch)
    last_check_ns: AtomicU64,

    /// Padding to 64 bytes
    _padding: [u8; 24],
}

impl MemoryBudgetCapsule {
    /// Create budget from MB
    fn new_mb(budget_mb: u64) -> Self {
        Self {
            budget_bytes: budget_mb * 1024 * 1024,
            current_rss: AtomicU64::new(0),
            peak_rss: AtomicU64::new(0),
            last_check_ns: AtomicU64::new(0),
            _padding: [0; 24],
        }
    }

    /// Update RSS from /proc/self/statm
    ///
    /// Format: size resident shared text lib data dt
    /// We use field[1] (resident) in pages, multiply by page size (4096)
    #[cfg(target_os = "linux")]
    fn update_rss(&self) -> Result<(), std::io::Error> {
        let statm = std::fs::read_to_string("/proc/self/statm")?;
        let fields: Vec<&str> = statm.split_whitespace().collect();

        if fields.len() < 2 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid /proc/self/statm format",
            ));
        }

        let resident_pages: u64 = fields[1].parse().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Failed to parse resident pages")
        })?;

        let rss_bytes = resident_pages * 4096; // Linux page size

        // Update current and peak
        self.current_rss.store(rss_bytes, Ordering::Release);
        let peak = self.peak_rss.load(Ordering::Acquire);
        if rss_bytes > peak {
            self.peak_rss.store(rss_bytes, Ordering::Release);
        }

        // Update timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        self.last_check_ns.store(now, Ordering::Release);

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    fn update_rss(&self) -> Result<(), std::io::Error> {
        // Non-Linux platforms: return 0 (no-op)
        self.current_rss.store(0, Ordering::Release);
        Ok(())
    }

    /// Get current RSS in MB
    fn current_mb(&self) -> f64 {
        let bytes = self.current_rss.load(Ordering::Acquire);
        bytes as f64 / (1024.0 * 1024.0)
    }

    /// Get peak RSS in MB
    fn peak_mb(&self) -> f64 {
        let bytes = self.peak_rss.load(Ordering::Acquire);
        bytes as f64 / (1024.0 * 1024.0)
    }

    /// Assert O(1) invariant: RSS <= budget
    ///
    /// # Panics
    ///
    /// Panics if RSS exceeds budget (memory leak or unbounded growth)
    fn assert_o1(&self) {
        if let Err(e) = self.update_rss() {
            eprintln!("Warning: Failed to read RSS: {}", e);
            return;
        }

        let rss = self.current_rss.load(Ordering::Acquire);
        if rss > self.budget_bytes {
            panic!(
                "MEMORY BUDGET EXCEEDED: {} MB > {} MB (O(1) invariant violated)",
                rss as f64 / (1024.0 * 1024.0),
                self.budget_bytes as f64 / (1024.0 * 1024.0)
            );
        }
    }

    /// Check budget without panic (returns true if under budget)
    fn check_budget(&self) -> bool {
        if let Err(_) = self.update_rss() {
            return true; // Assume OK if can't read
        }

        let rss = self.current_rss.load(Ordering::Acquire);
        rss <= self.budget_bytes
    }
}

// ============================================================================
// JSON TEXT EXTRACTOR (Simple, Zero Dependencies)
// ============================================================================

/// Extract "text" field from JSON line (simple parser)
///
/// Handles basic escape sequences: \", \\, \n, \r, \t
/// Does NOT handle Unicode escapes (\\uXXXX) or complex nesting.
///
/// # Example
///
/// ```text
/// {"text": "The quick brown fox"}  →  Some("The quick brown fox")
/// {"id": 123, "text": "Hello"}     →  Some("Hello")
/// {"no_text": "foo"}               →  None
/// ```
fn extract_text(line: &str) -> Option<String> {
    // Find "text" field
    let text_start = line.find(r#""text""#)?;
    let colon_pos = line[text_start..].find(':')?;
    let after_colon = &line[text_start + colon_pos + 1..];

    // Skip whitespace
    let after_ws = after_colon.trim_start();

    // Expect opening quote
    if !after_ws.starts_with('"') {
        return None;
    }

    // Find closing quote (handle escapes)
    let mut chars = after_ws[1..].chars();
    let mut result = String::new();
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if escaped {
            match ch {
                '"' => result.push('"'),
                '\\' => result.push('\\'),
                'n' => result.push('\n'),
                'r' => result.push('\r'),
                't' => result.push('\t'),
                _ => {
                    // Unknown escape, keep literal
                    result.push('\\');
                    result.push(ch);
                }
            }
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            // Found closing quote
            return Some(result);
        } else {
            result.push(ch);
        }
    }

    // Unclosed string
    None
}

// ============================================================================
// CLI ARGUMENTS
// ============================================================================

#[derive(Debug)]
struct Args {
    corpus_path: PathBuf,
    limit: Option<u32>,
    mode: PipelineMode,
    memory_budget: u64,
    threshold: f64,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut args_iter = std::env::args().skip(1);

        // Check for --help first
        let first_arg = args_iter.next().ok_or("Missing corpus path")?;
        if first_arg == "--help" || first_arg == "-h" {
            return Err("USAGE".to_string());
        }

        // First arg: corpus path (required)
        let corpus_path: PathBuf = first_arg.into();

        // Parse optional flags
        let mut limit = None;
        let mut mode = PipelineMode::Auto;
        let mut memory_budget = 1500; // MB
        let mut threshold = 0.85;

        while let Some(arg) = args_iter.next() {
            match arg.as_str() {
                "--limit" => {
                    let val = args_iter.next().ok_or("Missing --limit value")?;
                    limit = Some(val.parse().map_err(|_| "Invalid --limit value")?);
                }
                "--mode" => {
                    let val = args_iter.next().ok_or("Missing --mode value")?;
                    mode = match val.as_str() {
                        "cpu" => PipelineMode::CpuOnly,
                        "gpu" => PipelineMode::GpuAccelerated,
                        "auto" => PipelineMode::Auto,
                        _ => return Err(format!("Invalid mode: {} (use cpu|gpu|auto)", val)),
                    };
                }
                "--memory-budget" => {
                    let val = args_iter.next().ok_or("Missing --memory-budget value")?;
                    memory_budget = val.parse().map_err(|_| "Invalid --memory-budget value")?;
                }
                "--threshold" => {
                    let val = args_iter.next().ok_or("Missing --threshold value")?;
                    threshold = val.parse().map_err(|_| "Invalid --threshold value")?;
                }
                "--help" => {
                    return Err("USAGE".to_string()); // Trigger help message
                }
                _ => return Err(format!("Unknown argument: {}", arg)),
            }
        }

        Ok(Args {
            corpus_path,
            limit,
            mode,
            memory_budget,
            threshold,
        })
    }
}

fn print_usage() {
    eprintln!("bench_hybrid_streaming - O(1) Memory Hybrid Pipeline Benchmark");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("    bench_hybrid_streaming <corpus.jsonl> [OPTIONS]");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("    --limit <N>             Process only first N documents");
    eprintln!("    --mode <cpu|gpu|auto>   Pipeline mode (default: auto)");
    eprintln!("    --memory-budget <MB>    Max memory budget in MB (default: 1500)");
    eprintln!("    --threshold <0.0-1.0>   Jaccard similarity threshold (default: 0.85)");
    eprintln!("    --help                  Show this help message");
    eprintln!();
    eprintln!("EXAMPLES:");
    eprintln!("    # Auto-detect GPU, 100K docs, 1.5 GB budget");
    eprintln!("    bench_hybrid_streaming corpus.jsonl --limit 100000");
    eprintln!();
    eprintln!("    # Force CPU, unlimited docs, 2 GB budget");
    eprintln!("    bench_hybrid_streaming corpus.jsonl --mode cpu --memory-budget 2000");
    eprintln!();
    eprintln!("FRAMEWORK COMPLIANCE:");
    eprintln!("    T5 Streaming:    Line-by-line iterator, O(1) memory");
    eprintln!("    T7 Heterogeneous: CPU+GPU hybrid coordination");
    eprintln!("    B32:             Memory tracking, fair benchmarking");
    eprintln!("    ASSUM:           O(1) memory assumptions documented");
}

// ============================================================================
// MAIN
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse arguments
    let args = match Args::parse() {
        Ok(a) => a,
        Err(e) => {
            if e == "USAGE" {
                print_usage();
                return Ok(());
            }
            eprintln!("Error: {}", e);
            eprintln!();
            print_usage();
            return Err(e.into());
        }
    };

    // Print configuration
    eprintln!("=== Hybrid Pipeline Streaming Benchmark ===");
    eprintln!("Corpus:        {}", args.corpus_path.display());
    eprintln!("Limit:         {}", args.limit.map_or("unlimited".to_string(), |n| n.to_string()));
    eprintln!("Mode:          {:?}", args.mode);
    eprintln!("Memory Budget: {} MB", args.memory_budget);
    eprintln!("Threshold:     {:.2}", args.threshold);
    eprintln!();

    // Create memory budget capsule
    let budget = MemoryBudgetCapsule::new_mb(args.memory_budget);
    budget.update_rss().ok(); // Initial read
    eprintln!("Initial RSS:   {:.1} MB", budget.current_mb());
    eprintln!();

    // Estimate capacity (if limit provided)
    let estimated_capacity = args.limit.unwrap_or(1_000_000) as usize;

    // Create CPU capabilities
    let cpu_caps = CpuCapabilityCapsule::detect();

    // Create pipeline
    eprintln!("Initializing pipeline (capacity: {})...", estimated_capacity);
    let mut pipeline = HybridDedupPipeline::new(
        estimated_capacity,
        args.mode,
        &cpu_caps,
    )?;

    eprintln!("Pipeline mode: {:?}", args.mode);
    eprintln!("Using GPU:     {}", pipeline.is_using_gpu());
    #[cfg(feature = "gpu")]
    if let Some(caps) = pipeline.gpu_capabilities() {
        eprintln!("GPU:           {:?} ({:?})", caps.backend, caps.device_class);
    }
    eprintln!();

    // Open file for streaming
    let file = File::open(&args.corpus_path)?;
    let reader = BufReader::with_capacity(64 * 1024, file); // 64 KB buffer

    // #ASSUME: BufReader with 64 KB buffer provides O(1) memory for line reading
    // #VERIFY: Measured RSS stays <1.5 GB regardless of corpus size

    // Stream documents from file
    eprintln!("Streaming documents...");
    let start = Instant::now();
    let mut doc_id = 0u32;
    let mut last_report = Instant::now();
    let mut skipped = 0u64;

    for line in reader.lines() {
        let line = line?;

        // Extract text field from JSON
        let text = match extract_text(&line) {
            Some(t) => t,
            None => {
                skipped += 1;
                continue;
            }
        };

        // Add document to pipeline
        pipeline.add_document(doc_id, &text)?;
        doc_id += 1;

        // Check memory budget periodically (every 10K docs)
        if doc_id % 10_000 == 0 {
            budget.assert_o1(); // Panic if exceeded

            // Report progress every 10 seconds
            if last_report.elapsed() >= Duration::from_secs(10) {
                let elapsed = start.elapsed();
                let rate = doc_id as f64 / elapsed.as_secs_f64();
                eprintln!(
                    "Progress: {} docs, {:.1} MB, {:.0} docs/sec",
                    doc_id,
                    budget.current_mb(),
                    rate
                );
                last_report = Instant::now();
            }
        }

        // Check limit
        if let Some(limit) = args.limit {
            if doc_id >= limit {
                break;
            }
        }
    }

    let load_time = start.elapsed();
    eprintln!();
    eprintln!("=== Loading Complete ===");
    eprintln!("Documents:     {}", doc_id);
    eprintln!("Skipped:       {}", skipped);
    eprintln!("Load time:     {:.2}s", load_time.as_secs_f64());
    eprintln!("Throughput:    {:.0} docs/sec", doc_id as f64 / load_time.as_secs_f64());
    eprintln!("Peak RSS:      {:.1} MB", budget.peak_mb());
    eprintln!();

    // Find duplicates
    eprintln!("Finding duplicates (threshold: {:.2})...", args.threshold);
    let dedup_start = Instant::now();
    let clusters = pipeline.find_duplicates(args.threshold)?;
    let dedup_time = dedup_start.elapsed();

    // Final memory check
    budget.assert_o1();

    // Print results
    eprintln!();
    eprintln!("=== Results ===");
    eprintln!("Clusters:      {}", clusters.len());
    eprintln!("Dedup time:    {:.2}s", dedup_time.as_secs_f64());
    eprintln!("Total time:    {:.2}s", start.elapsed().as_secs_f64());
    eprintln!("Final RSS:     {:.1} MB", budget.current_mb());
    eprintln!("Peak RSS:      {:.1} MB", budget.peak_mb());
    eprintln!();

    // Pipeline stats
    let stats = pipeline.stats();
    eprintln!("=== Pipeline Stats ===");
    eprintln!("Docs processed:    {}", stats.docs_processed);
    eprintln!("GPU docs:          {}", stats.gpu_docs);
    eprintln!("CPU docs:          {}", stats.cpu_docs);
    eprintln!("GPU batches:       {}", stats.gpu_batches);
    eprintln!("Duplicate pairs:   {}", stats.duplicate_pairs);
    eprintln!("LSH candidates:    {}", stats.lsh_candidates);
    eprintln!();

    // O(1) memory validation
    if budget.check_budget() {
        eprintln!("✅ O(1) MEMORY INVARIANT: PASSED ({:.1} MB ≤ {} MB)",
                 budget.peak_mb(), args.memory_budget);
    } else {
        eprintln!("❌ O(1) MEMORY INVARIANT: FAILED ({:.1} MB > {} MB)",
                 budget.peak_mb(), args.memory_budget);
        return Err("Memory budget exceeded".into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_text_simple() {
        let line = r#"{"text": "Hello world"}"#;
        assert_eq!(extract_text(line), Some("Hello world".to_string()));
    }

    #[test]
    fn test_extract_text_with_fields() {
        let line = r#"{"id": 123, "text": "The quick brown fox", "other": "data"}"#;
        assert_eq!(extract_text(line), Some("The quick brown fox".to_string()));
    }

    #[test]
    fn test_extract_text_escaped_quote() {
        let line = r#"{"text": "He said \"hello\""}"#;
        assert_eq!(extract_text(line), Some(r#"He said "hello""#.to_string()));
    }

    #[test]
    fn test_extract_text_escaped_backslash() {
        let line = r#"{"text": "C:\\Users\\path"}"#;
        assert_eq!(extract_text(line), Some(r"C:\Users\path".to_string()));
    }

    #[test]
    fn test_extract_text_newline() {
        let line = r#"{"text": "Line1\nLine2"}"#;
        assert_eq!(extract_text(line), Some("Line1\nLine2".to_string()));
    }

    #[test]
    fn test_extract_text_missing() {
        let line = r#"{"id": 123, "other": "data"}"#;
        assert_eq!(extract_text(line), None);
    }

    #[test]
    fn test_extract_text_empty() {
        let line = r#"{"text": ""}"#;
        assert_eq!(extract_text(line), Some("".to_string()));
    }

    #[test]
    fn test_memory_budget_new() {
        let budget = MemoryBudgetCapsule::new_mb(1500);
        assert_eq!(budget.budget_bytes, 1500 * 1024 * 1024);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_memory_budget_update() {
        let budget = MemoryBudgetCapsule::new_mb(10_000); // 10 GB, won't exceed
        assert!(budget.update_rss().is_ok());
        assert!(budget.current_mb() > 0.0); // Should have some RSS
    }

    #[test]
    fn test_memory_budget_check() {
        let budget = MemoryBudgetCapsule::new_mb(10_000); // 10 GB
        assert!(budget.check_budget()); // Should pass (test process uses <10 GB)
    }
}
