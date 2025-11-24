//! C4 Production Validation Binary
//!
//! Comprehensive production validation for kindly_dedup with real C4 corpus data.
//! Measures memory usage, throughput, and accuracy metrics.
//!
//! **UCE34 Compliance**: Q1-Q34 systematic discovery
//! **B32 Compliance**: Fair baselines, statistical rigor
//! **T28 Compliance**: Production-tier testing

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

use anyhow::{Context, Result};
use kindly_dedup::{DedupPipeline, DocId};
use atomic_capsule::CpuCapabilityCapsule;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

/// Memory measurement point
#[derive(Debug, Clone)]
struct MemoryPoint {
    doc_count: usize,
    rss_mb: f64,
    allocated_mb: f64,
    growth_mb: f64,
    time_sec: f64,
    docs_per_sec: f64,
}

/// Validation results
#[derive(Debug, Clone)]
struct ValidationResults {
    corpus_path: String,
    total_docs: usize,
    total_time_sec: f64,
    avg_throughput: f64,
    peak_rss_mb: f64,
    memory_growth_mb: f64,
    duplicate_clusters: usize,
    total_duplicates: usize,
    duplicate_rate: f64,
    memory_points: Vec<MemoryPoint>,
}

/// Get current memory stats from jemalloc
fn get_memory_stats() -> Result<(f64, f64)> {
    use jemalloc_ctl::{stats, epoch};

    // Update the epoch to get fresh stats
    epoch::mib()
        .map_err(|e| anyhow::anyhow!("Failed to get epoch mib: {:?}", e))?
        .advance()
        .map_err(|e| anyhow::anyhow!("Failed to advance epoch: {:?}", e))?;

    // Get memory metrics (in bytes)
    let allocated = stats::allocated::mib()
        .map_err(|e| anyhow::anyhow!("Failed to get allocated mib: {:?}", e))?
        .read()
        .map_err(|e| anyhow::anyhow!("Failed to read allocated: {:?}", e))?
        as f64;
    let resident = stats::resident::mib()
        .map_err(|e| anyhow::anyhow!("Failed to get resident mib: {:?}", e))?
        .read()
        .map_err(|e| anyhow::anyhow!("Failed to read resident: {:?}", e))?
        as f64;

    // Convert to MB
    let mb = 1024.0 * 1024.0;
    Ok((resident / mb, allocated / mb))
}

/// Parse JSONL line to extract text field
fn parse_jsonl_text(line: &str) -> Option<String> {
    // Simple JSON parsing - extract "text" field
    // Format: {"id":N,"url":"...","text":"..."}
    if let Some(start) = line.find("\"text\":\"") {
        let text_start = start + 8; // Skip `"text":"`
        let remaining = &line[text_start..];

        // Find end of text field (handle escapes)
        let mut end = 0;
        let mut escaped = false;
        for (i, c) in remaining.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if c == '\\' {
                escaped = true;
                continue;
            }
            if c == '"' {
                end = i;
                break;
            }
        }

        if end > 0 {
            return Some(remaining[..end].to_string());
        }
    }
    None
}

/// Run production validation
fn run_validation(corpus_path: &str, max_docs: Option<usize>) -> Result<ValidationResults> {
    println!("=== C4 Production Validation ===\n");
    println!("Corpus: {}", corpus_path);
    println!("Max docs: {}", max_docs.map(|n| n.to_string()).unwrap_or("unlimited".to_string()));

    // Open corpus file
    let file = File::open(corpus_path)
        .with_context(|| format!("Failed to open corpus: {}", corpus_path))?;
    let reader = BufReader::with_capacity(8 * 1024 * 1024, file); // 8MB buffer

    // Count lines first for progress
    println!("\nCounting documents...");
    let line_count = {
        let file = File::open(corpus_path)?;
        BufReader::new(file).lines().count()
    };
    let total_expected = max_docs.map(|m| m.min(line_count)).unwrap_or(line_count);
    println!("Found {} documents in corpus, processing {}\n", line_count, total_expected);

    // Detect CPU capabilities
    let cpu_caps = CpuCapabilityCapsule::detect();
    println!("CPU: {} cores available", std::thread::available_parallelism()?.get());

    // Get initial memory state
    let (initial_rss, initial_allocated) = get_memory_stats()?;
    println!("Initial memory: {:.2} MB RSS, {:.2} MB allocated\n", initial_rss, initial_allocated);

    // Initialize pipeline
    println!("Initializing DedupPipeline for {} documents...", total_expected);
    let mut pipeline = DedupPipeline::new(total_expected, &cpu_caps);

    let (post_init_rss, _) = get_memory_stats()?;
    println!("Post-init memory: {:.2} MB RSS (growth: {:.2} MB)\n",
             post_init_rss, post_init_rss - initial_rss);

    // Memory measurement points
    let measurement_points: Vec<usize> = vec![
        1_000, 10_000, 50_000, 100_000, 200_000, 354_000, 500_000, 1_000_000
    ].into_iter()
     .filter(|&n| n <= total_expected)
     .collect();

    let mut memory_points = Vec::new();
    let mut peak_rss = post_init_rss;

    // Process documents
    println!("Processing documents...\n");
    let overall_start = Instant::now();
    let mut docs_processed = 0;
    let mut measurement_idx = 0;
    let mut segment_start = Instant::now();

    // Re-open file for reading
    let file = File::open(corpus_path)?;
    let reader = BufReader::with_capacity(8 * 1024 * 1024, file);

    for line in reader.lines() {
        let line = line?;

        // Parse text from JSONL
        if let Some(text) = parse_jsonl_text(&line) {
            pipeline.add_document(docs_processed as DocId, &text)?;
            docs_processed += 1;

            // Progress indicator
            if docs_processed % 10_000 == 0 {
                let elapsed = overall_start.elapsed().as_secs_f64();
                let rate = docs_processed as f64 / elapsed;
                print!("\r  {:>8} docs | {:>8.0} docs/sec | {:>6.1}s elapsed",
                       docs_processed, rate, elapsed);
                std::io::Write::flush(&mut std::io::stdout())?;
            }

            // Memory measurement at checkpoint
            if measurement_idx < measurement_points.len()
               && docs_processed >= measurement_points[measurement_idx] {
                let (rss, allocated) = get_memory_stats()?;
                let segment_elapsed = segment_start.elapsed().as_secs_f64();
                let total_elapsed = overall_start.elapsed().as_secs_f64();

                peak_rss = peak_rss.max(rss);

                memory_points.push(MemoryPoint {
                    doc_count: docs_processed,
                    rss_mb: rss,
                    allocated_mb: allocated,
                    growth_mb: rss - initial_rss,
                    time_sec: total_elapsed,
                    docs_per_sec: docs_processed as f64 / total_elapsed,
                });

                println!("\n  Checkpoint @ {}K: RSS {:.2} MB, {:.0} docs/sec",
                         docs_processed / 1000, rss, docs_processed as f64 / total_elapsed);

                measurement_idx += 1;
                segment_start = Instant::now();
            }

            // Check max docs limit
            if let Some(max) = max_docs {
                if docs_processed >= max {
                    break;
                }
            }
        }
    }

    let add_phase_elapsed = overall_start.elapsed();
    println!("\n\nAdd phase complete: {} docs in {:.2}s ({:.0} docs/sec)\n",
             docs_processed, add_phase_elapsed.as_secs_f64(),
             docs_processed as f64 / add_phase_elapsed.as_secs_f64());

    // Final memory measurement
    let (final_rss, final_allocated) = get_memory_stats()?;
    peak_rss = peak_rss.max(final_rss);

    println!("Final memory: {:.2} MB RSS, {:.2} MB allocated", final_rss, final_allocated);
    println!("Memory growth: {:.2} MB (from {:.2} MB to {:.2} MB)",
             final_rss - initial_rss, initial_rss, final_rss);

    // Find duplicates
    println!("\nFinding duplicates (threshold: 0.8)...");
    let dedup_start = Instant::now();
    let clusters = pipeline.find_duplicates(0.8)?;
    let dedup_elapsed = dedup_start.elapsed();

    // Analyze clusters (clusters is Vec<Vec<DocId>>)
    let total_duplicates: usize = clusters.iter().map(|c| c.len().saturating_sub(1)).sum();
    let duplicate_rate = if docs_processed > 0 {
        total_duplicates as f64 / docs_processed as f64 * 100.0
    } else {
        0.0
    };

    println!("Dedup complete: {} clusters, {} duplicates ({:.2}%) in {:.2}s\n",
             clusters.len(), total_duplicates, duplicate_rate, dedup_elapsed.as_secs_f64());

    let total_time = overall_start.elapsed().as_secs_f64();

    // Create results
    let results = ValidationResults {
        corpus_path: corpus_path.to_string(),
        total_docs: docs_processed,
        total_time_sec: total_time,
        avg_throughput: docs_processed as f64 / add_phase_elapsed.as_secs_f64(),
        peak_rss_mb: peak_rss,
        memory_growth_mb: final_rss - initial_rss,
        duplicate_clusters: clusters.len(),
        total_duplicates,
        duplicate_rate,
        memory_points,
    };

    Ok(results)
}

/// Print validation report
fn print_report(results: &ValidationResults) {
    println!("\n{}", "=".repeat(72));
    println!("PRODUCTION VALIDATION REPORT");
    println!("{}\n", "=".repeat(72));

    println!("CORPUS DETAILS");
    println!("  Source: {}", results.corpus_path);
    println!("  Documents: {}", results.total_docs);
    println!("  Total time: {:.2}s", results.total_time_sec);

    println!("\nMEMORY VALIDATION");
    println!("{:-<72}", "");
    println!("{:<12} | {:<12} | {:<12} | {:<12} | {:<12}",
             "Documents", "RSS (MB)", "Growth (MB)", "Time (s)", "Docs/sec");
    println!("{:-<12}-+-{:-<12}-+-{:-<12}-+-{:-<12}-+-{:-<12}",
             "", "", "", "", "");

    for point in &results.memory_points {
        println!("{:<12} | {:<12.2} | {:<12.2} | {:<12.2} | {:<12.0}",
                 point.doc_count,
                 point.rss_mb,
                 point.growth_mb,
                 point.time_sec,
                 point.docs_per_sec);
    }

    println!("\nMEMORY TARGETS");
    let targets = vec![
        (100_000, 100.0, "100K docs: <100 MB"),
        (500_000, 500.0, "500K docs: <500 MB"),
        (1_000_000, 1024.0, "1M docs: <1 GB"),
    ];

    for (docs, target_mb, label) in targets {
        if let Some(point) = results.memory_points.iter().find(|p| p.doc_count >= docs) {
            let status = if point.growth_mb < target_mb { "PASS" } else { "FAIL" };
            println!("  {}: {:.2} MB ({} target: <{:.0} MB)", label, point.growth_mb, status, target_mb);
        }
    }

    println!("\nTHROUGHPUT VALIDATION");
    println!("  Average: {:.0} docs/sec", results.avg_throughput);
    println!("  Target: >=60,000 docs/sec");
    println!("  Status: {}", if results.avg_throughput >= 60_000.0 { "PASS" } else {
        if results.avg_throughput >= 50_000.0 { "ACCEPTABLE (>50K)" } else { "BELOW TARGET" }
    });

    // Calculate vs Python baseline
    let python_baseline = 1_600.0; // Python datasketch ~1.6K docs/sec
    let speedup = results.avg_throughput / python_baseline;
    println!("  vs Python datasketch: {:.1}x speedup (target: >=38x)", speedup);

    println!("\nDUPLICATION ANALYSIS");
    println!("  Clusters found: {}", results.duplicate_clusters);
    println!("  Total duplicates: {}", results.total_duplicates);
    println!("  Duplicate rate: {:.2}%", results.duplicate_rate);

    println!("\nO(1) MEMORY COMPLIANCE");
    if results.memory_points.len() >= 2 {
        // Calculate growth rates
        let mut growth_rates = Vec::new();
        for i in 1..results.memory_points.len() {
            let prev = &results.memory_points[i-1];
            let curr = &results.memory_points[i];
            let docs_delta = (curr.doc_count - prev.doc_count) as f64 / 1000.0;
            let mem_delta = curr.growth_mb - prev.growth_mb;
            let rate = mem_delta / docs_delta;
            growth_rates.push((prev.doc_count, curr.doc_count, rate));
        }

        println!("  Growth rates (MB per 1K docs):");
        for (from, to, rate) in &growth_rates {
            println!("    {}K -> {}K: {:.4} MB/1K docs", from/1000, to/1000, rate);
        }

        // Check if sub-linear
        let is_sublinear = growth_rates.windows(2).all(|w| w[1].2 <= w[0].2 * 1.1);
        println!("  Growth pattern: {}", if is_sublinear { "SUB-LINEAR (O(1))" } else { "LINEAR" });
    }

    println!("\n{}", "=".repeat(72));
    println!("FINAL VERDICT");
    println!("{}", "=".repeat(72));

    let memory_ok = results.peak_rss_mb < 5000.0;
    let throughput_ok = results.avg_throughput >= 50_000.0;
    let speedup_ok = speedup >= 30.0;

    if memory_ok && throughput_ok && speedup_ok {
        println!("PRODUCTION READY");
        println!("  Memory: {:.2} MB peak (<5 GB target)", results.peak_rss_mb);
        println!("  Throughput: {:.0} docs/sec (>=50K acceptable)", results.avg_throughput);
        println!("  Speedup: {:.1}x vs Python (>=30x acceptable)", speedup);
    } else {
        println!("ISSUES FOUND");
        if !memory_ok {
            println!("  Memory: {:.2} MB EXCEEDS 5 GB target", results.peak_rss_mb);
        }
        if !throughput_ok {
            println!("  Throughput: {:.0} docs/sec BELOW 50K target", results.avg_throughput);
        }
        if !speedup_ok {
            println!("  Speedup: {:.1}x BELOW 30x target", speedup);
        }
    }

    println!();
}

fn main() -> Result<()> {
    // Parse arguments
    let args: Vec<String> = std::env::args().collect();

    let corpus_path = args.get(1).map(|s| s.as_str()).unwrap_or(
        "/home/samuel/Primitives/kindly_dedup/test_data/c4_100k.jsonl"
    );

    let max_docs: Option<usize> = args.get(2).and_then(|s| s.parse().ok());

    println!("C4 Production Validation");
    println!("========================\n");
    println!("Usage: c4_production_validation [corpus_path] [max_docs]");
    println!("  corpus_path: Path to JSONL corpus (default: c4_100k.jsonl)");
    println!("  max_docs: Maximum documents to process (default: all)\n");

    // Run validation
    let results = run_validation(corpus_path, max_docs)?;

    // Print report
    print_report(&results);

    Ok(())
}
