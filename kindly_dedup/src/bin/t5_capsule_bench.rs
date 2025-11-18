//! T5 Streaming Pipeline - UCE34 Capsule Benchmark
//!
//! **Tier Stack**: T0 (Auditable) + T1 (Atomic) + T4 (Batch) + T5 (Streaming) + T10 (Probabilistic)
//!
//! **Purpose**: Validate T5 Streaming Pipeline performance with fixed Bloom filter
//! - Measure throughput (target: 200-300K docs/sec)
//! - Benchmark 1M document deduplication
//! - Compare vs 39,788 docs/sec sequential baseline
//! - Validate 3.3-5× speedup expectation
//!
//! **UCE34 Framework Application**:
//! - Q1-Q9: Problem understanding (throughput measurement, validation)
//! - Q10: Tier selection (T0+T1+T5+T10 stack)
//! - Q11: Rust transform (DualAtomicU64, zero heap allocation hot path)
//! - Q12: Nightly features (portable_simd if available)
//! - Q33: Verification (#[derive(ComputationalCapsule)])
//! - Q34: Auditability (hash-chained results, compliance-ready)
//!
//! **ASSUM Safety Tags**:
//! - #ASSUME_ATOMIC_TIMING: DualAtomicU64 stores nanosecond timing atomically
//! - #ASSUME_CACHE_ALIGNED: 64-byte alignment prevents false sharing
//! - #ASSUME_THROUGHPUT_CALC: throughput = (docs * 1e9) / elapsed_ns
//! - #ASSUME_BLOOM_CONSISTENCY: Bloom skip rate stable across runs

use atomic_capsule_derive::ComputationalCapsule;
use kindly_dedup::{generate_synthetic_corpus, StreamingDedupPipeline};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

// ============================================================================
// CONFIGURATION CONSTANTS
// ============================================================================

/// Number of documents to benchmark
const BENCHMARK_CORPUS_SIZE: usize = 1_000_000;

/// Jaccard similarity threshold
const SIMILARITY_THRESHOLD: f64 = 0.85;

/// Baseline throughput (sequential, 39.8K docs/sec from CLAUDE.md)
const BASELINE_THROUGHPUT: f64 = 39_788.0;

/// Target throughput range (200-300K docs/sec from CLAUDE.md)
const TARGET_MIN_THROUGHPUT: f64 = 200_000.0;
const TARGET_MAX_THROUGHPUT: f64 = 300_000.0;

/// Minimum speedup target (3.3× from CLAUDE.md)
const MINIMUM_SPEEDUP: f64 = 3.3;

// ============================================================================
// BENCHMARK RESULTS CAPSULE (T0+T1)
// ============================================================================

/// Benchmark results capsule with atomic metrics
///
/// **Architecture**:
/// - T0 (Auditable): Hash-chained results for compliance
/// - T1 (Atomic): Lockfree timing counters with separate atomics
/// - 128-byte cache alignment to prevent false sharing
///
/// **ASSUM Safety**:
/// - #ASSUME_CACHE_ALIGNED: 128-byte alignment (checked at compile-time)
/// - #ASSUME_ATOMIC_TIMING: All timing is atomic-safe
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
struct BenchmarkResultsCapsule {
    /// Corpus generation time (nanoseconds)
    corpus_ns: AtomicU64,

    /// Corpus generation throughput (docs/sec)
    corpus_throughput: AtomicU64,

    /// Add documents time (nanoseconds)
    add_ns: AtomicU64,

    /// Add documents throughput (docs/sec)
    add_throughput: AtomicU64,

    /// Find duplicates time (nanoseconds)
    find_ns: AtomicU64,

    /// Find duplicates throughput (docs/sec)
    find_throughput: AtomicU64,

    /// Total end-to-end time (nanoseconds)
    total_ns: AtomicU64,

    /// Total end-to-end throughput (docs/sec)
    total_throughput: AtomicU64,

    /// Bloom filter skip count (pre-filter efficiency metric)
    bloom_skipped: AtomicU64,

    /// Total documents processed
    total_docs: AtomicU64,

    /// Duplicate clusters found
    clusters_found: AtomicU64,

    /// Pipeline panic count (reliability metric)
    total_panics: AtomicU64,

    /// Padding to 128 bytes (two cache-line aligned)
    _padding: [u8; 32],
}

impl BenchmarkResultsCapsule {
    /// Create new results capsule
    fn new() -> Self {
        Self {
            corpus_ns: AtomicU64::new(0),
            corpus_throughput: AtomicU64::new(0),
            add_ns: AtomicU64::new(0),
            add_throughput: AtomicU64::new(0),
            find_ns: AtomicU64::new(0),
            find_throughput: AtomicU64::new(0),
            total_ns: AtomicU64::new(0),
            total_throughput: AtomicU64::new(0),
            bloom_skipped: AtomicU64::new(0),
            total_docs: AtomicU64::new(0),
            clusters_found: AtomicU64::new(0),
            total_panics: AtomicU64::new(0),
            _padding: [0u8; 32],
        }
    }

    /// Record corpus generation metrics
    ///
    /// #ASSUME_ATOMIC_TIMING: Timing stored atomically via AtomicU64
    /// #VERIFY_ATOMIC_TIMING: Timing measured with Instant::now() (monotonic)
    fn record_corpus_gen(&self, elapsed_ns: u64, docs: u64) {
        let throughput = if elapsed_ns > 0 {
            (docs * 1_000_000_000) / elapsed_ns
        } else {
            0
        };
        self.corpus_ns.store(elapsed_ns, Ordering::Release);
        self.corpus_throughput.store(throughput, Ordering::Release);
    }

    /// Record add_documents stage metrics
    fn record_add_docs(&self, elapsed_ns: u64, docs: u64) {
        let throughput = if elapsed_ns > 0 {
            (docs * 1_000_000_000) / elapsed_ns
        } else {
            0
        };
        self.add_ns.store(elapsed_ns, Ordering::Release);
        self.add_throughput.store(throughput, Ordering::Release);
    }

    /// Record find_duplicates stage metrics
    fn record_find_dups(&self, elapsed_ns: u64, docs: u64) {
        let throughput = if elapsed_ns > 0 {
            (docs * 1_000_000_000) / elapsed_ns
        } else {
            0
        };
        self.find_ns.store(elapsed_ns, Ordering::Release);
        self.find_throughput.store(throughput, Ordering::Release);
    }

    /// Record total end-to-end metrics
    fn record_total(&self, elapsed_ns: u64, docs: u64) {
        let throughput = if elapsed_ns > 0 {
            (docs * 1_000_000_000) / elapsed_ns
        } else {
            0
        };
        self.total_ns.store(elapsed_ns, Ordering::Release);
        self.total_throughput.store(throughput, Ordering::Release);
    }

    /// Get corpus generation metrics
    fn get_corpus_metrics(&self) -> (u64, u64) {
        let ns = self.corpus_ns.load(Ordering::Acquire);
        let tput = self.corpus_throughput.load(Ordering::Acquire);
        (ns, tput)
    }

    /// Get add_documents metrics
    fn get_add_metrics(&self) -> (u64, u64) {
        let ns = self.add_ns.load(Ordering::Acquire);
        let tput = self.add_throughput.load(Ordering::Acquire);
        (ns, tput)
    }

    /// Get find_duplicates metrics
    fn get_find_metrics(&self) -> (u64, u64) {
        let ns = self.find_ns.load(Ordering::Acquire);
        let tput = self.find_throughput.load(Ordering::Acquire);
        (ns, tput)
    }

    /// Get total metrics
    fn get_total_metrics(&self) -> (u64, u64) {
        let ns = self.total_ns.load(Ordering::Acquire);
        let tput = self.total_throughput.load(Ordering::Acquire);
        (ns, tput)
    }

    /// Print formatted benchmark results with validation
    fn print_results(&self) {
        println!("\n{}", "=".repeat(80));
        println!("     T5 STREAMING PIPELINE - UCE34 CAPSULE BENCHMARK");
        println!("{}\n", "=".repeat(80));

        // Corpus generation metrics
        let (corpus_ns, corpus_throughput) = self.get_corpus_metrics();
        let corpus_secs = corpus_ns as f64 / 1e9;
        println!("CORPUS GENERATION:");
        println!("  Time: {:.2}s", corpus_secs);
        println!("  Throughput: {} docs/sec", format_number(corpus_throughput as usize));
        println!();

        // Add documents stage
        let (add_ns, add_throughput) = self.get_add_metrics();
        let add_secs = add_ns as f64 / 1e9;
        println!("T5 ADD DOCUMENTS (Stages 1-4):");
        println!("  Time: {:.2}s", add_secs);
        println!("  Throughput: {} docs/sec", format_number(add_throughput as usize));

        let bloom_skipped = self.bloom_skipped.load(Ordering::Acquire);
        let total_docs = self.total_docs.load(Ordering::Acquire);
        if total_docs > 0 {
            let skip_pct = (bloom_skipped as f64 / total_docs as f64) * 100.0;
            println!(
                "  Bloom skipped: {} docs ({:.1}%) ✅",
                format_number(bloom_skipped as usize),
                skip_pct
            );
        }

        // Validate throughput target
        let target_met = add_throughput >= TARGET_MIN_THROUGHPUT as u64;
        let status = if target_met { "✅" } else { "❌" };
        println!(
            "  Target validation: {} docs/sec {} (target: {}+)",
            format_number(add_throughput as usize),
            status,
            format_number(TARGET_MIN_THROUGHPUT as usize)
        );
        println!();

        // Find duplicates stage
        let (find_ns, find_throughput) = self.get_find_metrics();
        let find_secs = find_ns as f64 / 1e9;
        println!("T5 FIND DUPLICATES (Stage 5):");
        println!("  Time: {:.2}s", find_secs);
        println!("  Throughput: {} docs/sec", format_number(find_throughput as usize));

        let clusters = self.clusters_found.load(Ordering::Acquire);
        println!("  Clusters: {} found", format_number(clusters as usize));
        println!();

        // End-to-end metrics
        let (total_ns, total_throughput) = self.get_total_metrics();
        let total_secs = total_ns as f64 / 1e9;
        let speedup = total_throughput as f64 / BASELINE_THROUGHPUT;

        println!("END-TO-END SUMMARY:");
        println!("  Total time: {:.2}s", total_secs);
        println!("  Throughput: {} docs/sec", format_number(total_throughput as usize));
        println!("  Speedup: {:.2}× vs {:.0} baseline", speedup, BASELINE_THROUGHPUT);
        println!();

        // Validation section
        println!("VALIDATION:");

        let throughput_ok = total_throughput >= TARGET_MIN_THROUGHPUT as u64;
        let status = if throughput_ok { "✅" } else { "⚠️ " };
        println!(
            "  {} Target met: {} docs/sec (≥{})",
            status,
            format_number(total_throughput as usize),
            format_number(TARGET_MIN_THROUGHPUT as usize)
        );

        let bloom_ok = bloom_skipped > 0 && bloom_skipped < (total_docs / 2);
        let status = if bloom_ok { "✅" } else { "❌" };
        println!(
            "  {} Bloom filter working ({:.1}% skip rate)",
            status,
            (bloom_skipped as f64 / total_docs as f64) * 100.0
        );

        let panics = self.total_panics.load(Ordering::Acquire);
        let panic_ok = panics == 0;
        let status = if panic_ok { "✅" } else { "❌" };
        println!("  {} Zero panics (reliability: 100%)", status);

        let speedup_ok = speedup >= MINIMUM_SPEEDUP;
        let status = if speedup_ok { "✅" } else { "⚠️ " };
        println!(
            "  {} Speedup target: {:.2}× (≥{:.1}×)",
            status, speedup, MINIMUM_SPEEDUP
        );
        println!();

        // Classification
        let classification = if total_throughput > TARGET_MAX_THROUGHPUT as u64 {
            "EXCEPTIONAL (5×+ tier)"
        } else if total_throughput >= TARGET_MIN_THROUGHPUT as u64 {
            "EXCELLENT (3.3-5× tier)"
        } else if speedup >= 2.0 {
            "GOOD (2-3.3× tier)"
        } else if speedup >= 1.0 {
            "ACCEPTABLE (1-2× tier)"
        } else {
            "NEEDS WORK (regression detected)"
        };

        println!("CLASSIFICATION: {}", classification);
        println!("{}\n", "=".repeat(80));
    }

    /// Write audit trail (Q34 compliance)
    fn write_audit_trail(&self, path: &str) -> std::io::Result<()> {
        use std::fs::File;
        use std::io::Write;

        let (corpus_ns, corpus_tput) = self.get_corpus_metrics();
        let (add_ns, add_tput) = self.get_add_metrics();
        let (find_ns, find_tput) = self.get_find_metrics();
        let (total_ns, total_tput) = self.get_total_metrics();

        let mut file = File::create(path)?;
        writeln!(
            file,
            r#"{{"benchmark":"t5_streaming","corpus_ns":{},"corpus_tput":{},"add_ns":{},"add_tput":{},"find_ns":{},"find_tput":{},"total_ns":{},"total_tput":{},"bloom_skipped":{},"clusters":{},"panics":{}}}"#,
            corpus_ns,
            corpus_tput,
            add_ns,
            add_tput,
            find_ns,
            find_tput,
            total_ns,
            total_tput,
            self.bloom_skipped.load(Ordering::Acquire),
            self.clusters_found.load(Ordering::Acquire),
            self.total_panics.load(Ordering::Acquire)
        )?;
        println!("✅ Audit trail written to: {}", path);
        Ok(())
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Format large numbers with thousand separators
fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.insert(0, ',');
        }
        result.insert(0, c);
    }
    result
}

/// Convert nanoseconds to seconds with precision
fn nanos_to_secs(nanos: u64) -> f64 {
    nanos as f64 / 1_000_000_000.0
}

// ============================================================================
// MAIN BENCHMARK
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let results = BenchmarkResultsCapsule::new();

    // ========================================================================
    // STAGE 1: CORPUS GENERATION (T4 Batch tier)
    // ========================================================================

    println!(
        "\n[1/5] Generating {} documents...",
        format_number(BENCHMARK_CORPUS_SIZE)
    );

    let corpus_start = Instant::now();
    let corpus = generate_synthetic_corpus(BENCHMARK_CORPUS_SIZE);
    let corpus_elapsed = corpus_start.elapsed().as_nanos() as u64;

    println!("  ✅ Corpus generated in {:.2}s", nanos_to_secs(corpus_elapsed));
    results.record_corpus_gen(corpus_elapsed, BENCHMARK_CORPUS_SIZE as u64);

    // ========================================================================
    // STAGE 2: PIPELINE CREATION & ADD DOCUMENTS (T5 Streaming)
    // ========================================================================

    println!("\n[2/5] Creating T5 Streaming Pipeline...");

    // Detect CPU capabilities for automatic SIMD selection
    let cpu_caps = atomic_capsule::CpuCapabilityCapsule::detect();
    let num_threads = std::thread::available_parallelism().map(|p| p.get()).unwrap_or(8);

    println!("  CPU: {} cores detected", num_threads);
    println!("  SIMD: {:?}", cpu_caps);

    // Create streaming pipeline
    let mut pipeline = StreamingDedupPipeline::new(BENCHMARK_CORPUS_SIZE, num_threads)?;

    // Convert documents to (id, text) format
    let documents: Vec<(usize, String)> = corpus.iter().map(|doc| (doc.id, doc.text.clone())).collect();

    println!(
        "\n[3/5] Adding {} documents to pipeline...",
        format_number(documents.len())
    );

    let add_start = Instant::now();
    pipeline.add_documents(documents)?;
    let add_elapsed = add_start.elapsed().as_nanos() as u64;

    let add_tput = if add_elapsed > 0 {
        (BENCHMARK_CORPUS_SIZE as u64 * 1_000_000_000) / add_elapsed
    } else {
        0
    };

    println!(
        "  ✅ Documents added in {:.2}s ({} docs/sec)",
        nanos_to_secs(add_elapsed),
        format_number(add_tput as usize)
    );
    results.record_add_docs(add_elapsed, BENCHMARK_CORPUS_SIZE as u64);

    // ========================================================================
    // STAGE 3: FIND DUPLICATES (T5 Streaming + T10 Probabilistic)
    // ========================================================================

    println!(
        "\n[4/5] Finding duplicates with Jaccard threshold {}...",
        SIMILARITY_THRESHOLD
    );

    let find_start = Instant::now();
    let clusters = pipeline.find_duplicates(SIMILARITY_THRESHOLD)?;
    let find_elapsed = find_start.elapsed().as_nanos() as u64;

    let find_tput = if find_elapsed > 0 {
        (BENCHMARK_CORPUS_SIZE as u64 * 1_000_000_000) / find_elapsed
    } else {
        0
    };

    println!(
        "  ✅ Deduplication complete in {:.2}s ({} docs/sec)",
        nanos_to_secs(find_elapsed),
        format_number(find_tput as usize)
    );
    println!("  Found {} duplicate clusters", format_number(clusters.len()));

    results.record_find_dups(find_elapsed, BENCHMARK_CORPUS_SIZE as u64);

    // ========================================================================
    // STAGE 4: COLLECT METRICS (T1 Atomic)
    // ========================================================================

    println!("\n[5/5] Collecting metrics...");

    let metrics = pipeline.metrics();

    results
        .total_docs
        .store(BENCHMARK_CORPUS_SIZE as u64, Ordering::Release);
    results
        .bloom_skipped
        .store(metrics.documents_skipped as u64, Ordering::Release);
    results.clusters_found.store(clusters.len() as u64, Ordering::Release);

    // Panic count
    let total_panics =
        metrics.tokenization_panics + metrics.minhash_panics + metrics.lsh_panics + metrics.verification_panics;
    results.total_panics.store(total_panics as u64, Ordering::Release);

    // ========================================================================
    // STAGE 5: END-TO-END TIMING
    // ========================================================================

    let total_elapsed = corpus_elapsed + add_elapsed + find_elapsed;
    let _total_tput = if total_elapsed > 0 {
        (BENCHMARK_CORPUS_SIZE as u64 * 1_000_000_000) / total_elapsed
    } else {
        0
    };

    results.record_total(total_elapsed, BENCHMARK_CORPUS_SIZE as u64);

    // ========================================================================
    // RESULTS & VALIDATION
    // ========================================================================

    println!("\n[COMPLETE] Benchmark finished successfully!");

    // Print comprehensive results
    results.print_results();

    // Write audit trail (Q34 compliance)
    results.write_audit_trail("t5_benchmark_results.jsonl")?;

    // ========================================================================
    // EXIT CODE BASED ON VALIDATION
    // ========================================================================

    let target_met = add_tput >= TARGET_MIN_THROUGHPUT as u64;
    let zero_panics = total_panics == 0;
    let bloom_working = metrics.documents_skipped > 0;

    if target_met && zero_panics && bloom_working {
        println!("✅ All validation checks passed!\n");
        Ok(())
    } else {
        eprintln!("\n⚠️  Validation issues detected:");
        if !target_met {
            eprintln!(
                "  - Throughput below target: {} vs {}+",
                format_number(add_tput as usize),
                format_number(TARGET_MIN_THROUGHPUT as usize)
            );
        }
        if !zero_panics {
            eprintln!("  - Panics detected: {}", total_panics);
        }
        if !bloom_working {
            eprintln!("  - Bloom filter not working (zero documents skipped)");
        }
        Err("Benchmark validation failed".into())
    }
}
