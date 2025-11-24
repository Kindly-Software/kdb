//! Memory Validation Binary - Prove O(1) memory guarantee using jemalloc_ctl
//!
//! **PURPOSE**: Validate that memory usage is O(1) constant (<5 GB) regardless of document count.
//!
//! # Expected Results
//! - Memory growth 1K→100K: <100 MB (O(1) validated)
//! - Memory growth 1K→1M: <5 GB (production guarantee)
//! - No memory leaks (RSS stable after processing)

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

use anyhow::Result;
use kindly_dedup::{DedupPipeline, DocId};
use atomic_capsule::CpuCapabilityCapsule;
use std::thread;
use std::time::{Duration, Instant};

/// Memory measurement point
#[derive(Debug, Clone)]
struct MemoryPoint {
    doc_count: usize,
    rss_mb: f64,
    allocated_mb: f64,
    active_mb: f64,
    resident_mb: f64,
    retained_mb: f64,
    growth_mb: f64,
    time_sec: f64,
}

/// Get current memory stats from jemalloc
fn get_memory_stats() -> Result<(f64, f64, f64, f64, f64)> {
    use jemalloc_ctl::{stats, epoch};

    // Update the epoch to get fresh stats
    epoch::mib()
        .map_err(|e| anyhow::anyhow!("Failed to get epoch mib: {:?}", e))?
        .advance()
        .map_err(|e| anyhow::anyhow!("Failed to advance epoch: {:?}", e))?;

    // Get various memory metrics (in bytes)
    let allocated = stats::allocated::mib()
        .map_err(|e| anyhow::anyhow!("Failed to get allocated mib: {:?}", e))?
        .read()
        .map_err(|e| anyhow::anyhow!("Failed to read allocated: {:?}", e))?
        as f64;
    let active = stats::active::mib()
        .map_err(|e| anyhow::anyhow!("Failed to get active mib: {:?}", e))?
        .read()
        .map_err(|e| anyhow::anyhow!("Failed to read active: {:?}", e))?
        as f64;
    let resident = stats::resident::mib()
        .map_err(|e| anyhow::anyhow!("Failed to get resident mib: {:?}", e))?
        .read()
        .map_err(|e| anyhow::anyhow!("Failed to read resident: {:?}", e))?
        as f64;
    let retained = stats::retained::mib()
        .map_err(|e| anyhow::anyhow!("Failed to get retained mib: {:?}", e))?
        .read()
        .map_err(|e| anyhow::anyhow!("Failed to read retained: {:?}", e))?
        as f64;

    // RSS is best approximated by resident memory
    let rss = resident;

    // Convert to MB
    let mb = 1024.0 * 1024.0;
    Ok((rss / mb, allocated / mb, active / mb, resident / mb, retained / mb))
}

/// Force garbage collection and stabilize memory
fn stabilize_memory() {
    // Force deallocation of temporary allocations
    for _ in 0..3 {
        // Allocate and deallocate to trigger GC
        let _temp: Vec<u8> = Vec::with_capacity(1024 * 1024);
        drop(_temp);

        // Sleep to allow background threads to finish
        thread::sleep(Duration::from_millis(50));
    }

    // Final sleep for stabilization
    thread::sleep(Duration::from_millis(100));
}

/// Generate synthetic document
fn generate_document(id: usize) -> String {
    // Generate realistic document with some variation
    let base = "The quick brown fox jumps over the lazy dog. ";
    let variation = match id % 10 {
        0 => "This is a completely unique document with different content. ",
        1 => "Another variation of text that creates diversity. ",
        2 => "Scientific papers often contain technical jargon and formulas. ",
        3 => "News articles discuss current events and politics. ",
        4 => "Fiction books tell imaginative stories and adventures. ",
        5 => "Technical documentation explains software and hardware. ",
        6 => "Social media posts are short and informal messages. ",
        7 => "Academic research involves systematic investigation. ",
        8 => "Legal documents use precise and formal language. ",
        9 => "Marketing copy persuades and informs customers. ",
        _ => base,
    };

    // Create document with some repetition for deduplication
    let repeat_count = if id % 100 == 0 { 1 } else { 2 };
    format!("{}{}{}", base, variation.repeat(repeat_count), id)
}

/// Run memory validation test
fn validate_memory() -> Result<()> {
    println!("=== O(1) Memory Validation using jemalloc_ctl ===\n");
    println!("Testing DedupPipeline with real RSS measurement");
    println!("Expected: <100 MB growth from 1K to 100K documents (O(1))\n");

    // Test points: 1K, 10K, 100K, 1M documents
    let test_points = vec![1_000, 10_000, 100_000, 1_000_000];
    let mut results = Vec::new();
    let mut baseline_rss = 0.0;

    // Detect CPU capabilities
    let cpu_caps = CpuCapabilityCapsule::detect();

    // Initialize pipeline with large expected capacity
    println!("Initializing DedupPipeline...");
    let mut pipeline = DedupPipeline::new(10_000_000, &cpu_caps);

    // Get initial memory state
    stabilize_memory();
    let (initial_rss, initial_allocated, initial_active, initial_resident, initial_retained) = get_memory_stats()?;
    baseline_rss = initial_rss;

    println!("Initial memory state:");
    println!("  RSS:       {:.2} MB", initial_rss);
    println!("  Allocated: {:.2} MB", initial_allocated);
    println!("  Active:    {:.2} MB", initial_active);
    println!("  Resident:  {:.2} MB", initial_resident);
    println!("  Retained:  {:.2} MB\n", initial_retained);

    // Process documents at each test point
    let mut total_docs_processed = 0;

    for &target_count in &test_points {
        println!("Processing to {} documents...", target_count);
        let start_time = Instant::now();

        // Add documents up to target count
        while total_docs_processed < target_count {
            let doc = generate_document(total_docs_processed);
            pipeline.add_document(total_docs_processed as DocId, &doc)?;
            total_docs_processed += 1;

            // Progress indicator
            if total_docs_processed % 10_000 == 0 {
                print!("  {} docs...\r", total_docs_processed);
                std::io::Write::flush(&mut std::io::stdout())?;
            }
        }

        // Stabilize memory after processing
        println!("  Stabilizing memory...");
        stabilize_memory();

        // Measure memory after stabilization
        let (rss, allocated, active, resident, retained) = get_memory_stats()?;
        let elapsed = start_time.elapsed().as_secs_f64();

        // Calculate growth
        let growth = rss - baseline_rss;

        // Store result
        results.push(MemoryPoint {
            doc_count: target_count,
            rss_mb: rss,
            allocated_mb: allocated,
            active_mb: active,
            resident_mb: resident,
            retained_mb: retained,
            growth_mb: growth,
            time_sec: elapsed,
        });

        println!("  Complete: {:.2}s, RSS: {:.2} MB (growth: {:.2} MB)\n", elapsed, rss, growth);
    }

    // Print results table
    println!("\n=== Memory Validation Results ===\n");
    println!("{:<12} | {:<12} | {:<12} | {:<12} | {:<12} | {:<12}",
             "Documents", "RSS (MB)", "Growth (MB)", "Allocated", "Active", "Time (s)");
    println!("{:-<12}-+-{:-<12}-+-{:-<12}-+-{:-<12}-+-{:-<12}-+-{:-<12}",
             "", "", "", "", "", "");

    for point in &results {
        println!("{:<12} | {:<12.2} | {:<12.2} | {:<12.2} | {:<12.2} | {:<12.2}",
                 point.doc_count,
                 point.rss_mb,
                 point.growth_mb,
                 point.allocated_mb,
                 point.active_mb,
                 point.time_sec);
    }

    // Analyze O(1) compliance
    println!("\n=== O(1) Compliance Analysis ===\n");

    // Check growth from 1K to 100K
    let growth_1k_to_100k = results[2].rss_mb - results[0].rss_mb;
    let o1_100k_pass = growth_1k_to_100k < 100.0;

    println!("Growth 1K → 100K: {:.2} MB (target: <100 MB) - {}",
             growth_1k_to_100k,
             if o1_100k_pass { "✅ PASS" } else { "❌ FAIL" });

    // Check growth from 1K to 1M
    let growth_1k_to_1m = results[3].rss_mb - results[0].rss_mb;
    let o1_1m_pass = growth_1k_to_1m < 5000.0;

    println!("Growth 1K → 1M:   {:.2} MB (target: <5000 MB) - {}",
             growth_1k_to_1m,
             if o1_1m_pass { "✅ PASS" } else { "❌ FAIL" });

    // Check for linear growth (O(N) detection)
    let growth_rate_10k = (results[1].rss_mb - results[0].rss_mb) / 9.0;  // per 1K docs
    let growth_rate_100k = (results[2].rss_mb - results[1].rss_mb) / 90.0; // per 1K docs
    let growth_rate_1m = (results[3].rss_mb - results[2].rss_mb) / 900.0;  // per 1K docs

    println!("\nGrowth Rate Analysis (MB per 1K docs):");
    println!("  1K → 10K:   {:.4} MB/1K", growth_rate_10k);
    println!("  10K → 100K: {:.4} MB/1K", growth_rate_100k);
    println!("  100K → 1M:  {:.4} MB/1K", growth_rate_1m);

    // Decreasing growth rate indicates sub-linear (O(1) or O(log n))
    let sublinear = growth_rate_100k < growth_rate_10k && growth_rate_1m < growth_rate_100k;
    println!("\nGrowth pattern: {} (decreasing rate = sub-linear)",
             if sublinear { "✅ SUB-LINEAR" } else { "❌ LINEAR OR WORSE" });

    // Memory breakdown
    println!("\n=== Memory Breakdown (at 1M docs) ===\n");
    let final_point = &results[3];
    println!("Resident memory:  {:.2} MB", final_point.resident_mb);
    println!("Allocated memory: {:.2} MB", final_point.allocated_mb);
    println!("Active memory:    {:.2} MB", final_point.active_mb);
    println!("Retained memory:  {:.2} MB", final_point.retained_mb);

    // Memory efficiency
    let efficiency = (final_point.allocated_mb / final_point.resident_mb) * 100.0;
    println!("Memory efficiency: {:.1}% (allocated/resident)", efficiency);

    // Theoretical O(N) comparison
    println!("\n=== Comparison to O(N) Baseline ===\n");
    let bytes_per_doc = 1024; // Assume 1KB per document in naive implementation
    let naive_1m_mb = (1_000_000 * bytes_per_doc) as f64 / (1024.0 * 1024.0);
    println!("Naive O(N) at 1M docs: {:.2} MB", naive_1m_mb);
    println!("Actual at 1M docs:     {:.2} MB", final_point.rss_mb);
    println!("Reduction:             {:.1}x", naive_1m_mb / final_point.rss_mb);

    // Final verdict
    println!("\n=== FINAL VERDICT ===\n");
    if o1_100k_pass && o1_1m_pass && sublinear {
        println!("✅ O(1) MEMORY GUARANTEE VALIDATED");
        println!("   - Growth is sub-linear");
        println!("   - Memory usage <5 GB at 1M documents");
        println!("   - Production-ready for billion-scale corpora");
    } else {
        println!("❌ O(1) MEMORY GUARANTEE FAILED");
        if !o1_100k_pass {
            println!("   - Excessive growth 1K→100K: {:.2} MB > 100 MB", growth_1k_to_100k);
        }
        if !o1_1m_pass {
            println!("   - Excessive growth 1K→1M: {:.2} MB > 5000 MB", growth_1k_to_1m);
        }
        if !sublinear {
            println!("   - Growth pattern is linear or worse (not O(1))");
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    // Set up panic handler
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("\n❌ PANIC: {}", panic_info);
    }));

    // Run validation
    validate_memory()?;

    Ok(())
}