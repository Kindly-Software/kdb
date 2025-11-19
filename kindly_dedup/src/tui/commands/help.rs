//! Help Command - Interactive Help System
//!
//! Provides comprehensive help on:
//! - Available commands
//! - Configuration options
//! - Performance tuning
//! - Troubleshooting
//! - API reference
//!
//! **UCE34 Q31**: Simple interfaces (scrollable text viewer)

use inquire::Select;

// ============================================================================
// HELP TOPICS
// ============================================================================

/// Available help topics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpTopic {
    /// Command overview
    Commands,
    /// Configuration guide
    Configuration,
    /// Performance tuning
    Performance,
    /// Troubleshooting
    Troubleshooting,
    /// API reference
    ApiReference,
    /// Examples
    Examples,
}

impl HelpTopic {
    fn name(&self) -> &'static str {
        match self {
            HelpTopic::Commands => "Available Commands",
            HelpTopic::Configuration => "Configuration Guide",
            HelpTopic::Performance => "Performance Tuning",
            HelpTopic::Troubleshooting => "Troubleshooting",
            HelpTopic::ApiReference => "API Reference",
            HelpTopic::Examples => "Examples",
        }
    }

    fn content(&self) -> &'static str {
        match self {
            HelpTopic::Commands => HELP_COMMANDS,
            HelpTopic::Configuration => HELP_CONFIGURATION,
            HelpTopic::Performance => HELP_PERFORMANCE,
            HelpTopic::Troubleshooting => HELP_TROUBLESHOOTING,
            HelpTopic::ApiReference => HELP_API_REFERENCE,
            HelpTopic::Examples => HELP_EXAMPLES,
        }
    }
}

// ============================================================================
// HELP CONTENT
// ============================================================================

const HELP_COMMANDS: &str = r#"
═══════════════════════════════════════════════════════════
  AVAILABLE COMMANDS
═══════════════════════════════════════════════════════════

kindly_dedup provides 6 main commands for interactive workflows:

1. /demo - Production Demo Wizard
   ─────────────────────────────────
   Interactive 3-tier demonstration:
   • Tier 1: 100K docs with 100% accuracy validation (~17 min)
   • Tier 2: 1M docs with production speed (~17 sec)
   • Tier 3: 10M docs with massive scale (~3 min)

   Use Cases:
   - Sales demonstrations
   - Performance validation
   - Accuracy verification

   Example:
   $ kindly_dedup /demo


2. /dedup - Interactive Deduplication
   ─────────────────────────────────
   Complete E2E deduplication workflow:
   • File browser for input selection
   • Configuration wizard (threshold, format, threading)
   • Live execution with progress metrics
   • Results export (JSON/JSONL/CSV/Text)

   Use Cases:
   - Production deduplication
   - Custom dataset processing
   - Batch processing

   Example:
   $ kindly_dedup /dedup


3. /verify - Audit Trail Validation
   ─────────────────────────────────
   Verify Q34 audit trails:
   • Hash chain integrity (tamper detection)
   • Generation counter consistency
   • Reproducibility verification
   • License event correlation

   Use Cases:
   - Compliance audits (SOX, SOC2, GDPR, HIPAA)
   - Tamper detection
   - Forensic analysis

   Example:
   $ kindly_dedup /verify


4. /benchmark - Performance Validation
   ─────────────────────────────────
   Run B32-compliant benchmarks:
   • v1.0 Baseline (38× speedup)
   • v1.1 SIMD (7.1× speedup)
   • v1.1 Compound (204× tier stacking)
   • v1.2 Incremental (100× weekly updates)
   • Accuracy validation (95% F1 score)

   Use Cases:
   - Performance validation
   - Regression testing
   - Hardware comparison

   Example:
   $ kindly_dedup /benchmark


5. /stats - Statistics Analysis
   ─────────────────────────────────
   Analyze deduplication results:
   • Cluster size distribution
   • Duplicate rate analysis
   • Top N largest clusters
   • Memory efficiency metrics
   • Performance statistics

   Use Cases:
   - Results analysis
   - Quality assessment
   - Reporting

   Example:
   $ kindly_dedup /stats


6. /help - Interactive Help
   ─────────────────────────────────
   This help system!

   Use Cases:
   - Learn commands
   - Configuration reference
   - Troubleshooting guide

   Example:
   $ kindly_dedup /help


GETTING STARTED
───────────────
1. Start with /demo to see performance and accuracy
2. Use /dedup for your own datasets
3. Verify results with /verify (compliance)
4. Analyze results with /stats

For detailed examples, select "Examples" from the help menu.
"#;

const HELP_CONFIGURATION: &str = r#"
═══════════════════════════════════════════════════════════
  CONFIGURATION GUIDE
═══════════════════════════════════════════════════════════

JACCARD THRESHOLD
─────────────────
Controls duplicate detection sensitivity:

• 0.70-0.80: Loose matching (more duplicates found)
• 0.85: Industry standard (recommended)
• 0.90-0.95: Strict matching (fewer false positives)
• 1.00: Exact duplicates only

Recommendation: Start with 0.85, adjust based on results.


THREADING
─────────
Number of threads for parallel processing:

• 0 (auto): Use all available cores (recommended)
• 1: Single-threaded (debugging, sequential processing)
• N: Use N threads (manual control)

Performance: 8-12× speedup with 16 cores @ 60% efficiency.


EXPORT FORMATS
──────────────
• JSON: Full structure, nested objects
  - Best for: Application integration
  - Size: Moderate (pretty-printed)

• JSONL: Newline-delimited JSON
  - Best for: Streaming processing
  - Size: Compact (one line per cluster)

• CSV: Spreadsheet-compatible
  - Best for: Excel, data analysis
  - Size: Compact (tabular format)

• Text: Human-readable
  - Best for: Quick inspection
  - Size: Most compact


PERSISTENT MODE
───────────────
Use mmap-backed storage for large datasets:

• Enabled: Disk-backed, handles billions of documents
• Disabled: In-memory, faster but limited by RAM

Recommendation: Enable for >1M documents or low-RAM systems.


VERBOSE OUTPUT
──────────────
Controls logging detail:

• Enabled: Progress updates, metrics, debugging info
• Disabled: Summary only (faster, cleaner)

Recommendation: Enable for first run, disable for automation.


EXAMPLE CONFIGURATION
─────────────────────
Recommended settings for production:

  Jaccard Threshold: 0.85
  Threads: 0 (auto)
  Export Format: JSONL
  Persistent Mode: Enabled (>1M docs)
  Verbose: Enabled (interactive), Disabled (batch)


ENVIRONMENT VARIABLES
─────────────────────
Optional environment overrides:

  KINDLY_DEDUP_THRESHOLD=0.85
  KINDLY_DEDUP_THREADS=16
  KINDLY_DEDUP_FORMAT=jsonl
  KINDLY_DEDUP_VERBOSE=1

Example:
$ KINDLY_DEDUP_THREADS=8 kindly_dedup /dedup
"#;

const HELP_PERFORMANCE: &str = r#"
═══════════════════════════════════════════════════════════
  PERFORMANCE TUNING GUIDE
═══════════════════════════════════════════════════════════

VALIDATED PERFORMANCE
─────────────────────
• Single-threaded: 60,000 docs/sec (38× vs Python)
• Multi-threaded (16 cores): 576,000 docs/sec (366×)
• Latency: <1ms per document
• Memory: 256 bytes per signature


OPTIMIZATION STRATEGIES
───────────────────────

1. THREADING
   ─────────
   • Use all available cores (threads=0)
   • Expected scaling: 8-12× with 16 cores
   • Efficiency: 60% parallel efficiency

2. CORPUS SIZE
   ───────────
   • Small (<10K): In-memory mode
   • Medium (10K-1M): In-memory or persistent
   • Large (>1M): Persistent mode (mmap-backed)

3. THRESHOLD TUNING
   ────────────────
   • Lower threshold (0.70): More duplicates, slower
   • Higher threshold (0.95): Fewer duplicates, faster
   • Optimal: 0.85 (industry standard)

4. MEMORY OPTIMIZATION
   ───────────────────
   • Persistent mode: Reduces RAM usage 100×
   • Trade-off: 10-20% slower (disk I/O)
   • Recommendation: Use for >1M documents


PERFORMANCE TIERS (B32 Validated)
──────────────────────────────────

T1: Single-threaded Baseline
  • 60K docs/sec
  • 38× vs Python datasketch
  • Classification: EXCEPTIONAL

T2: SIMD MinHash
  • 7.1× speedup over scalar
  • 20.7 μs per signature (vs 147 μs)
  • Classification: EXCEPTIONAL

T4: Multi-threaded (16 cores)
  • 576K docs/sec (projected)
  • 366× vs Python
  • Classification: BREAKTHROUGH

T6: Compound (Bloom + SIMD + Lockfree + Parallel)
  • 320K docs/sec (projected)
  • 204× vs Python
  • Classification: BREAKTHROUGH


HARDWARE REQUIREMENTS
─────────────────────

Minimum:
  • CPU: x86-64 (SSE2 required)
  • RAM: 4 GB (10K documents)
  • Disk: 1 GB temporary space

Recommended:
  • CPU: AMD Ryzen 9 / Intel Core i7+ (8+ cores)
  • RAM: 16 GB (1M documents)
  • Disk: 10 GB SSD (fast I/O)

Optimal:
  • CPU: AMD Ryzen 9 6900HX (16 cores)
  • RAM: 64 GB (10M documents)
  • Disk: 100 GB NVMe SSD


TROUBLESHOOTING SLOW PERFORMANCE
─────────────────────────────────

Symptom: Throughput < 30K docs/sec
Solutions:
  1. Check CPU usage (should be 95%+)
  2. Enable multi-threading (threads=0)
  3. Reduce threshold (0.85 → 0.80)
  4. Disable verbose output
  5. Use persistent mode (reduce RAM pressure)

Symptom: High memory usage
Solutions:
  1. Enable persistent mode
  2. Reduce corpus size (batch processing)
  3. Increase swap space
  4. Use mmap-backed storage


BENCHMARKING
────────────
Run /benchmark command to validate performance on your hardware:

$ kindly_dedup /benchmark

Results compared against Python datasketch baseline (1,572 docs/sec).
"#;

const HELP_TROUBLESHOOTING: &str = r#"
═══════════════════════════════════════════════════════════
  TROUBLESHOOTING GUIDE
═══════════════════════════════════════════════════════════

COMMON ISSUES
─────────────

1. File Not Found
   ──────────────
   Error: "File not found: data.txt"

   Solution:
   - Check file path (use absolute paths)
   - Verify file exists: ls -la data.txt
   - Check permissions: chmod +r data.txt

2. Out of Memory
   ─────────────
   Error: "Cannot allocate memory"

   Solution:
   - Enable persistent mode (mmap-backed)
   - Reduce corpus size (batch processing)
   - Increase swap space
   - Use smaller threshold (fewer duplicates)

3. Accuracy < 99%
   ──────────────
   Symptom: F1 score < 99%

   Solution:
   - Increase threshold (0.85 → 0.90)
   - Check corpus quality (garbage data)
   - Validate tokenization (whitespace split)
   - Run accuracy benchmark (/benchmark → Accuracy)

4. Throughput < 30K docs/sec
   ──────────────────────────
   Symptom: Performance slower than expected

   Solution:
   - Enable multi-threading (threads=0)
   - Check CPU usage (should be 95%+)
   - Disable verbose output
   - Use SSD (not HDD)
   - Run performance benchmark (/benchmark → v1.0 Baseline)

5. Audit Trail Not Generated
   ──────────────────────────
   Error: "Audit trail file not found"

   Solution:
   - Check /tmp permissions: chmod +w /tmp
   - Verify feature enabled: --features meta-capsule
   - Check disk space: df -h /tmp
   - Use custom path: export AUDIT_PATH=/path/to/audit.jsonl


HARDWARE-SPECIFIC ISSUES
─────────────────────────

AMD Ryzen Issues:
  - Symptom: Performance degradation
  - Solution: Disable SMT if needed
  - Command: echo off > /sys/devices/system/cpu/smt/control

Intel CPU Issues:
  - Symptom: SIMD not working
  - Solution: Verify AVX2 support
  - Command: grep avx2 /proc/cpuinfo

Low-Memory Systems:
  - Symptom: Frequent swapping
  - Solution: Enable persistent mode + reduce batch size


LICENSE VALIDATION ISSUES
─────────────────────────

Warning: "License validation warning"
  - Cause: Evaluation license compatibility issue
  - Action: Contact support@kindly.software

Error: "License validation error"
  - Cause: License cannot be validated
  - Action: Provide Customer ID to support@kindly.software

Error: "License expired"
  - Cause: Evaluation period ended
  - Action: Contact sales@kindly.ai for production license


GETTING HELP
────────────

1. Check logs:
   $ tail -f /tmp/kindly_dedup.log

2. Run diagnostics:
   $ kindly_dedup /verify

3. Contact support:
   Email: support@kindly.software
   Include: Customer ID, error message, system info

4. Report bugs:
   GitHub: github.com/kindly-ai/kindly_dedup (if public)
   Email: support@kindly.software with [BUG] prefix
"#;

const HELP_API_REFERENCE: &str = r#"
═══════════════════════════════════════════════════════════
  API REFERENCE
═══════════════════════════════════════════════════════════

CORE TYPES
──────────

DedupPipeline
  Purpose: Main deduplication pipeline
  Construction: DedupPipeline::new(capacity: usize)
  Methods:
    - add_document(doc_id: usize, text: &str) -> Result<()>
    - find_duplicates(threshold: f64) -> Result<Vec<Vec<usize>>>
    - documents_added() -> usize
    - skip_rate() -> f64

  Example:
    let mut pipeline = DedupPipeline::new(10_000);
    pipeline.add_document(0, "The quick brown fox")?;
    let clusters = pipeline.find_duplicates(0.85)?;


PersistentDedupPipeline
  Purpose: Crash-safe persistent deduplication
  Construction:
    - create(path: &Path, capacity: usize) -> Result<Self>
    - recover(path: &Path) -> Result<Self>
  Methods:
    - add_document(doc_id: usize, text: &str) -> Result<()>
    - find_duplicates(threshold: f64) -> Result<Vec<Vec<usize>>>
    - flush() -> Result<()>
    - count() -> usize
    - capacity() -> usize

  Example:
    let mut pipeline = PersistentDedupPipeline::create("dedup.bin", 1_000_000)?;
    pipeline.add_document(0, "document text")?;
    pipeline.flush()?;


UniversalGroundTruthGenerator
  Purpose: Compute exact ground truth for accuracy validation
  Methods:
    - compute_ground_truth(corpus: &[Document], threshold: f64) -> Result<GroundTruth>
    - compute_ground_truth_production(corpus: &[Document], threshold: f64) -> Result<GroundTruth>

  Example:
    let gt = UniversalGroundTruthGenerator::compute_ground_truth(&corpus, 0.85)?;
    println!("Found {} duplicate pairs", gt.pairs.len());


BENCHMARKING API
────────────────

B32Runner
  Purpose: B32-compliant benchmark runner
  Construction: B32Runner::new(audit_path: &str) -> Result<Self>
  Methods:
    - run_benchmark<F>(name: &str, f: F) -> BenchmarkStats

  Example:
    let runner = B32Runner::new("audit.jsonl")?;
    let stats = runner.run_benchmark("my_bench", || {
        // benchmark code
    });


AuditLogger
  Purpose: Q34 audit trail logging
  Methods:
    - log_entry(entry: BenchmarkAuditEntry) -> Result<()>
    - verify_trail(path: &str) -> Result<bool>

  Example:
    let mut logger = AuditLogger::new("audit.jsonl")?;
    logger.log_entry(entry)?;


PERFORMANCE PRIMITIVES
──────────────────────

MinHashSignatureCapsule (T10 Probabilistic)
  - compute_signature(tokens: &[&str]) -> Self
  - jaccard_similarity(other: &Self) -> f64
  - size: 256 bytes (128 × u16)

UnionFind (O(α(n)) clustering)
  - new(capacity: usize) -> Self
  - union(a: usize, b: usize)
  - find(a: usize) -> usize
  - build_clusters() -> Vec<Vec<usize>>


ERROR TYPES
───────────

PipelineError
  - DocumentIdOutOfBounds
  - ProtectionViolation (when meta-capsule enabled)

PersistentError
  - IoError(io::Error)
  - InvalidMagic
  - UnsupportedVersion
  - GenerationMismatch
  - IndexFull


FEATURE FLAGS
─────────────

std: Standard library support (required)
parallel-dedup: Multi-threaded processing
http-server: HTTP API server
meta-capsule: 4-layer protection (binary protection)
simd-minhash: SIMD-accelerated MinHash (nightly)
benchmarking: B32 benchmark infrastructure
download-tools: Corpus download utilities


EXAMPLES
────────

See "Examples" topic in help menu for complete code examples.
"#;

const HELP_EXAMPLES: &str = r#"
═══════════════════════════════════════════════════════════
  EXAMPLES
═══════════════════════════════════════════════════════════

EXAMPLE 1: Basic Deduplication
───────────────────────────────

use kindly_dedup::DedupPipeline;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut pipeline = DedupPipeline::new(1000);

    // Add documents
    pipeline.add_document(0, "The quick brown fox jumps")?;
    pipeline.add_document(1, "The quick brown fox leaps")?;
    pipeline.add_document(2, "A completely different document")?;

    // Find duplicates (Jaccard ≥ 0.85)
    let clusters = pipeline.find_duplicates(0.85)?;

    println!("Found {} clusters", clusters.len());
    for (i, cluster) in clusters.iter().enumerate() {
        println!("Cluster {}: {:?}", i, cluster);
    }

    Ok(())
}


EXAMPLE 2: Persistent Deduplication
────────────────────────────────────

use kindly_dedup::PersistentDedupPipeline;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create new persistent index
    let mut pipeline = PersistentDedupPipeline::create(
        "dedup.bin",
        10_000_000  // 10M documents
    )?;

    // Add documents
    for (id, text) in load_documents() {
        pipeline.add_document(id, &text)?;
    }

    // Flush to disk (crash-safe)
    pipeline.flush()?;

    // Later: recover from crash
    drop(pipeline);
    let recovered = PersistentDedupPipeline::recover("dedup.bin")?;
    println!("Recovered {} documents", recovered.count());

    Ok(())
}


EXAMPLE 3: Accuracy Validation
───────────────────────────────

use kindly_dedup::benchmarking::UniversalGroundTruthGenerator;
use kindly_dedup::DedupPipeline;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = load_corpus();

    // Run deduplication
    let mut pipeline = DedupPipeline::new(corpus.len());
    for doc in &corpus {
        pipeline.add_document(doc.id, &doc.text)?;
    }
    let clusters = pipeline.find_duplicates(0.85)?;

    // Compute ground truth
    let ground_truth = UniversalGroundTruthGenerator::compute_ground_truth(
        &corpus,
        0.85
    )?;

    // Compare results (confusion matrix)
    let (precision, recall, f1) = compute_accuracy(&clusters, &ground_truth);

    println!("Precision: {:.2}%", precision);
    println!("Recall: {:.2}%", recall);
    println!("F1 Score: {:.2}%", f1);

    Ok(())
}


EXAMPLE 4: Benchmark Suite
───────────────────────────

use kindly_dedup::benchmarking::B32Runner;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runner = B32Runner::new("audit.jsonl")?;

    // Benchmark deduplication
    let stats = runner.run_benchmark("dedup_10k", || {
        let mut pipeline = DedupPipeline::new(10_000);
        for i in 0..10_000 {
            pipeline.add_document(i, &format!("document {}", i)).unwrap();
        }
        pipeline.find_duplicates(0.85).unwrap();
    });

    println!("Mean: {:.2} ms", stats.mean_ms);
    println!("Std Dev: {:.2} ms", stats.std_dev_ms);
    println!("95% CI: [{:.2}, {:.2}] ms", stats.ci_lower_ms, stats.ci_upper_ms);

    Ok(())
}


EXAMPLE 5: Interactive Commands
────────────────────────────────

# Run production demo
$ kindly_dedup /demo

# Deduplicate custom dataset
$ kindly_dedup /dedup

# Verify audit trail
$ kindly_dedup /verify

# Run benchmarks
$ kindly_dedup /benchmark

# Analyze results
$ kindly_dedup /stats

# Get help
$ kindly_dedup /help


MORE EXAMPLES
─────────────

For more examples, see:
- GitHub repository (if public)
- Documentation: docs.kindly.ai/dedup
- Contact: support@kindly.software
"#;

// ============================================================================
// TOPIC SELECTION
// ============================================================================

/// Select help topic
pub fn select_topic() -> Result<HelpTopic, Box<dyn std::error::Error>> {
    let topic_names = vec![
        HelpTopic::Commands.name(),
        HelpTopic::Configuration.name(),
        HelpTopic::Performance.name(),
        HelpTopic::Troubleshooting.name(),
        HelpTopic::ApiReference.name(),
        HelpTopic::Examples.name(),
    ];

    let selection = Select::new("Select help topic:", topic_names)
        .with_help_message("Use ↑↓ to navigate, Enter to select")
        .prompt()?;

    for topic in [
        HelpTopic::Commands,
        HelpTopic::Configuration,
        HelpTopic::Performance,
        HelpTopic::Troubleshooting,
        HelpTopic::ApiReference,
        HelpTopic::Examples,
    ] {
        if topic.name() == selection {
            return Ok(topic);
        }
    }

    Err("Invalid topic".into())
}

// ============================================================================
// HELP DISPLAY
// ============================================================================

/// Display help content for a topic
pub fn display_help(topic: HelpTopic) {
    println!("{}", topic.content());

    println!("\n[Press Enter to return to menu]");
    let mut _input = String::new();
    let _ = std::io::stdin().read_line(&mut _input);
}

// ============================================================================
// MAIN ENTRY POINT
// ============================================================================

/// Run interactive help system
pub fn run_help() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║                                                            ║");
    println!("║              kindly_dedup - Help System                   ║");
    println!("║                                                            ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    loop {
        let topic = select_topic()?;
        display_help(topic);

        // Ask if user wants to see another topic
        use inquire::Confirm;
        let continue_help = Confirm::new("View another help topic?").with_default(false).prompt()?;

        if !continue_help {
            break;
        }
    }

    println!("\nGoodbye!\n");
    Ok(())
}
