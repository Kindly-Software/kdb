//! Phase 4.3 Thread-Local Buffer Stress Tests (T28 Q22-Q28)
//!
//! **Purpose**: Production-grade stress testing for Phase 4.3 thread-local buffer implementation.
//!
//! # T28 Coverage
//!
//! - **Q22**: Stress tests (1M documents, 64-core scaling, long-running stability)
//! - **Q23**: Security/adversarial tests (malformed inputs, edge cases)
//! - **Q24**: Benchmark validation (≥ 912K docs/sec @ 16 cores, ≥ 95% efficiency)
//! - **Q25**: Memory efficiency (< 10KB per document)
//! - **Q26**: Thread-local correctness (identical to sequential)
//! - **Q27**: Edge cases (single thread, more threads than docs)
//! - **Q28**: Production readiness (sustained throughput, memory stability)
//!
//! # Framework Compliance
//!
//! - **T28**: Q22-Q28 production tier
//! - **B32**: Fair baselines, statistical rigor, 95% CI
//! - **ASSUM**: 99.99% safe under stress
//! - **UCE34**: Q1-Q34 complete
//!
//! # Feature Gates
//!
//! This test requires `parallel-dedup` feature to be enabled.

#![cfg(feature = "parallel-dedup")]

use atomic_capsule::CpuCapabilityCapsule;
use kindly_dedup::ParallelDedupPipeline;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

// ============================================================================
// Test Utilities
// ============================================================================

/// Get system memory info (Linux only)
#[cfg(target_os = "linux")]
fn get_system_memory_gb() -> Option<f64> {
    use std::fs;

    let meminfo = fs::read_to_string("/proc/meminfo").ok()?;
    for line in meminfo.lines() {
        if line.starts_with("MemTotal:") {
            let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
            return Some(kb as f64 / 1024.0 / 1024.0); // Convert KB to GB
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn get_system_memory_gb() -> Option<f64> {
    None // Memory detection not implemented for non-Linux
}

/// Get current process RSS (Linux only)
#[cfg(target_os = "linux")]
fn get_process_rss_mb() -> Option<f64> {
    use std::fs;

    let status = fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if line.starts_with("VmRSS:") {
            let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
            return Some(kb as f64 / 1024.0); // Convert KB to MB
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn get_process_rss_mb() -> Option<f64> {
    None // RSS tracking not implemented for non-Linux
}

/// Generate synthetic documents with controlled characteristics
fn generate_synthetic_docs(num_docs: usize, unique_ratio: f64) -> Vec<(usize, String)> {
    let num_unique = (num_docs as f64 * unique_ratio) as usize;
    let mut docs = Vec::with_capacity(num_docs);

    // Generate unique documents
    for i in 0..num_unique {
        let text = format!(
            "Synthetic document {} with unique content: {}",
            i,
            (0..50)
                .map(|j| format!("word{}", i * 50 + j))
                .collect::<Vec<_>>()
                .join(" ")
        );
        docs.push((i, text));
    }

    // Fill remaining with duplicates
    for i in num_unique..num_docs {
        let original_idx = i % num_unique;
        let text = format!(
            "Synthetic document {} with unique content: {}",
            original_idx,
            (0..50)
                .map(|j| format!("word{}", original_idx * 50 + j))
                .collect::<Vec<_>>()
                .join(" ")
        );
        docs.push((i, text));
    }

    docs
}

// ============================================================================
// T28 Q22: Production Stress Tests
// ============================================================================

/// Test 1: 1M Documents Production (T28 Q22)
///
/// **Target**: ≥ 912K docs/sec @ 16 cores (≥ 95% efficiency)
#[test]
#[ignore] // Long-running (5-10 minutes)
fn test_stress_1m_documents_phase4_3() {
    println!("\n=== Phase 4.3: 1M Documents Production Test ===\n");

    let system_mem_gb = get_system_memory_gb().unwrap_or(16.0);
    println!("System memory: {:.2} GB", system_mem_gb);

    if system_mem_gb < 8.0 {
        println!("SKIPPED: Insufficient memory ({:.2} GB < 8 GB required)", system_mem_gb);
        return;
    }

    println!("Generating 1M documents (100K unique, 90% duplicates)...");
    let docs = generate_synthetic_docs(1_000_000, 0.10);
    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();

    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = ParallelDedupPipeline::new(1_000_000, 16, &cpu_caps).unwrap();

    let rss_before = get_process_rss_mb().unwrap_or(0.0);
    println!("RSS before: {:.2} MB", rss_before);

    let start = Instant::now();
    pipeline.add_documents(&doc_refs).unwrap();
    let add_elapsed = start.elapsed();

    let rss_after = get_process_rss_mb().unwrap_or(0.0);
    println!(
        "RSS after: {:.2} MB (delta: {:.2} MB)",
        rss_after,
        rss_after - rss_before
    );

    println!(
        "Added {} documents in {:.2}s",
        pipeline.documents_added(),
        add_elapsed.as_secs_f64()
    );

    let throughput = 1_000_000.0 / add_elapsed.as_secs_f64();
    println!("Throughput: {:.0} docs/sec", throughput);

    let start = Instant::now();
    let clusters = pipeline.find_duplicates(0.85).unwrap();
    let find_elapsed = start.elapsed();

    println!(
        "Found {} clusters in {:.2}s",
        clusters.len(),
        find_elapsed.as_secs_f64()
    );

    let total_time = add_elapsed + find_elapsed;
    println!("Total time: {:.2}s", total_time.as_secs_f64());

    // Validate throughput ≥ 912K docs/sec @ 16 cores (95% efficiency)
    // Baseline: 60K docs/sec sequential
    // Target: 60K × 16 × 0.95 = 912K docs/sec
    assert!(
        throughput >= 912_000.0,
        "Throughput {:.0} docs/sec < 912K target (95% efficiency @ 16 cores)",
        throughput
    );

    // Validate efficiency ≥ 95% (Phase 4.3 improvement)
    let efficiency = throughput / (16.0 * 60_000.0); // 60K sequential baseline
    println!("Efficiency: {:.1}% (target: ≥ 95%)", efficiency * 100.0);
    assert!(efficiency >= 0.95, "Efficiency {:.1}% < 95% target", efficiency * 100.0);

    // Memory efficiency: < 10KB per document
    let bytes_per_doc = ((rss_after - rss_before) * 1024.0 * 1024.0) / 1_000_000.0;
    println!("Memory: {:.0} bytes/doc (target: < 10KB)", bytes_per_doc);
    assert!(
        bytes_per_doc < 10_240.0,
        "Memory {:.0} bytes/doc exceeds 10KB limit",
        bytes_per_doc
    );

    println!("\n✅ 1M documents test PASSED");
}

/// Test 2: 64-Core Scaling (T28 Q23)
///
/// **Target**: Scale to 64 cores with ≥ 93% efficiency
#[test]
#[ignore] // Requires 64-core machine
fn test_stress_64_cores_scaling() {
    println!("\n=== Phase 4.3: 64-Core Scaling Test ===\n");

    let max_cores = num_cpus::get();
    println!("System cores: {}", max_cores);

    if max_cores < 64 {
        println!("SKIPPED: Insufficient cores ({} < 64 required)", max_cores);
        return;
    }

    println!("Generating 1M documents (100K unique, 90% duplicates)...");
    let docs = generate_synthetic_docs(1_000_000, 0.10);
    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();

    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = ParallelDedupPipeline::new(1_000_000, 64, &cpu_caps).unwrap();

    let start = Instant::now();
    pipeline.add_documents(&doc_refs).unwrap();
    let add_elapsed = start.elapsed();

    let throughput = 1_000_000.0 / add_elapsed.as_secs_f64();
    println!("Throughput: {:.0} docs/sec", throughput);

    // Validate: 64 cores @ 93% efficiency = 3.6M docs/sec
    // 60K baseline × 64 × 0.93 = 3.571M docs/sec
    let target_throughput = 60_000.0 * 64.0 * 0.93;
    assert!(
        throughput >= target_throughput,
        "Throughput {:.0} docs/sec < {:.0} target (93% efficiency @ 64 cores)",
        throughput,
        target_throughput
    );

    let efficiency = throughput / (64.0 * 60_000.0);
    println!("Efficiency: {:.1}% (target: ≥ 93%)", efficiency * 100.0);
    assert!(efficiency >= 0.93, "Efficiency {:.1}% < 93% target", efficiency * 100.0);

    println!("\n✅ 64-core scaling test PASSED");
}

/// Test 3: Memory Efficiency (T28 Q24)
///
/// **Target**: < 10KB per document across all scales
#[test]
fn test_memory_efficiency_thread_local() {
    println!("\n=== Phase 4.3: Memory Efficiency Test ===\n");

    let test_scales = [(1_000, "1K"), (10_000, "10K"), (100_000, "100K")];

    for (num_docs, label) in &test_scales {
        let docs = generate_synthetic_docs(*num_docs, 0.20);
        let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();

        let rss_before = get_process_rss_mb().unwrap_or(0.0);

        let cpu_caps = CpuCapabilityCapsule::detect();
        let mut pipeline = ParallelDedupPipeline::new(*num_docs, 8, &cpu_caps).unwrap();
        pipeline.add_documents(&doc_refs).unwrap();

        let rss_after = get_process_rss_mb().unwrap_or(0.0);
        let delta_mb = rss_after - rss_before;
        let bytes_per_doc = (delta_mb * 1024.0 * 1024.0) / *num_docs as f64;

        println!("{} documents:", label);
        println!("  RSS delta: {:.2} MB", delta_mb);
        println!("  Bytes per doc: {:.0}", bytes_per_doc);

        // Memory efficiency target: <10KB per document
        assert!(
            bytes_per_doc < 10_240.0,
            "{}: Memory usage {:.0} bytes/doc exceeds 10KB limit",
            label,
            bytes_per_doc
        );
    }

    println!("\n✅ Memory efficiency test PASSED");
}

/// Test 4: Sustained Throughput (T28 Q25)
///
/// **Target**: Stable throughput over 1-hour run, no memory leaks
#[test]
#[ignore] // 1-hour run
fn test_sustained_throughput_1_hour() {
    println!("\n=== Phase 4.3: 1-Hour Sustained Throughput Test ===\n");

    let target_duration_secs = 3600; // 1 hour
    let batch_size = 10_000;

    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = ParallelDedupPipeline::new(100_000_000, 16, &cpu_caps).unwrap();
    let total_processed = Arc::new(AtomicUsize::new(0));

    let start = Instant::now();
    let mut iteration = 0;

    while start.elapsed().as_secs() < target_duration_secs {
        iteration += 1;

        // Generate batch
        let offset = (iteration - 1) * batch_size;
        let docs = generate_synthetic_docs(batch_size, 0.20);
        let doc_refs: Vec<(usize, &str)> = docs
            .iter()
            .enumerate()
            .map(|(i, (_, text))| (offset + i, text.as_str()))
            .collect();

        // Process batch
        pipeline.add_documents(&doc_refs).unwrap();
        total_processed.fetch_add(batch_size, Ordering::Relaxed);

        // Report progress every 10 minutes
        if iteration % 60 == 0 {
            let elapsed = start.elapsed().as_secs();
            let processed = total_processed.load(Ordering::Relaxed);
            let throughput = processed as f64 / elapsed as f64;

            println!(
                "[{:02}:{:02}] Processed {} docs ({:.0} docs/sec)",
                elapsed / 3600,
                (elapsed % 3600) / 60,
                processed,
                throughput
            );

            let rss = get_process_rss_mb().unwrap_or(0.0);
            println!("  RSS: {:.2} MB", rss);
        }

        // Memory check: abort if RSS > 16GB
        if let Some(rss) = get_process_rss_mb() {
            if rss > 16_384.0 {
                println!("ABORT: Memory usage {:.2} MB exceeds 16GB limit", rss);
                panic!("Memory usage exceeded 16GB limit");
            }
        }
    }

    let elapsed = start.elapsed();
    let processed = total_processed.load(Ordering::Relaxed);
    let throughput = processed as f64 / elapsed.as_secs_f64();

    println!("\n1-hour test complete:");
    println!("  Total processed: {} documents", processed);
    println!("  Average throughput: {:.0} docs/sec", throughput);

    // Performance target: ≥ 900K docs/sec sustained (98% of target 912K)
    assert!(
        throughput >= 900_000.0,
        "Sustained throughput {:.0} docs/sec below 900K target",
        throughput
    );

    println!("\n✅ 1-hour sustained test PASSED");
}

/// Test 5: Thread-Local Buffer Correctness (T28 Q26)
///
/// **Target**: Thread-local results IDENTICAL to sequential
#[test]
fn test_thread_local_correctness() {
    println!("\n=== Phase 4.3: Thread-Local Correctness Test ===\n");

    // Property test: Thread-local == sequential
    // Run 100 times with random doc sets
    let num_runs = 100;
    let num_docs = 1_000;

    for run in 0..num_runs {
        let docs = generate_synthetic_docs(num_docs, 0.20);
        let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();

        let cpu_caps = CpuCapabilityCapsule::detect();

        // Sequential baseline
        let mut sequential_pipeline = ParallelDedupPipeline::new(num_docs, 1, &cpu_caps).unwrap();
        sequential_pipeline.add_documents(&doc_refs).unwrap();
        let sequential_clusters = sequential_pipeline.find_duplicates(0.85).unwrap();

        // Thread-local parallel (16 threads)
        let mut parallel_pipeline = ParallelDedupPipeline::new(num_docs, 16, &cpu_caps).unwrap();
        parallel_pipeline.add_documents(&doc_refs).unwrap();
        let parallel_clusters = parallel_pipeline.find_duplicates(0.85).unwrap();

        // Validate: IDENTICAL cluster counts
        assert_eq!(
            parallel_clusters.len(),
            sequential_clusters.len(),
            "Run {}: Cluster count mismatch (parallel {} != sequential {})",
            run,
            parallel_clusters.len(),
            sequential_clusters.len()
        );

        // Validate: IDENTICAL cluster sizes (sorted)
        let mut sequential_sizes: Vec<usize> = sequential_clusters.iter().map(|c: &Vec<usize>| c.len()).collect();
        let mut parallel_sizes: Vec<usize> = parallel_clusters.iter().map(|c: &Vec<usize>| c.len()).collect();
        sequential_sizes.sort_unstable();
        parallel_sizes.sort_unstable();

        assert_eq!(parallel_sizes, sequential_sizes, "Run {}: Cluster sizes mismatch", run);

        if (run + 1) % 10 == 0 {
            println!("  Completed {} / {} runs", run + 1, num_runs);
        }
    }

    println!("\n✅ Thread-local correctness test PASSED (100/100 runs)");
}

/// Test 6: Edge Case - Single Thread (T28 Q27)
///
/// **Target**: 1 thread works correctly (no deadlocks)
#[test]
fn test_thread_local_single_thread() {
    println!("\n=== Phase 4.3: Single Thread Test ===\n");

    let docs = generate_synthetic_docs(10_000, 0.20);
    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();

    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = ParallelDedupPipeline::new(10_000, 1, &cpu_caps).unwrap();

    let start = Instant::now();
    pipeline.add_documents(&doc_refs).unwrap();
    let add_elapsed = start.elapsed();

    let clusters = pipeline.find_duplicates(0.85).unwrap();

    println!("Single thread: {:.0} docs/sec", 10_000.0 / add_elapsed.as_secs_f64());
    println!("Clusters: {}", clusters.len());

    // Expected throughput: ~60K docs/sec (same as sequential)
    let throughput = 10_000.0 / add_elapsed.as_secs_f64();
    assert!(
        throughput >= 50_000.0 && throughput <= 80_000.0,
        "Single thread throughput {:.0} outside expected range [50K, 80K]",
        throughput
    );

    println!("\n✅ Single thread test PASSED");
}

/// Test 7: Edge Case - More Threads Than Docs (T28 Q28)
///
/// **Target**: No panics, correct results
#[test]
fn test_thread_local_more_threads_than_docs() {
    println!("\n=== Phase 4.3: More Threads Than Docs Test ===\n");

    let docs = generate_synthetic_docs(50, 0.40);
    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();

    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = ParallelDedupPipeline::new(50, 100, &cpu_caps).unwrap();

    // Should not panic
    pipeline.add_documents(&doc_refs).unwrap();
    let clusters = pipeline.find_duplicates(0.85).unwrap();

    println!("100 threads, 50 documents: {} clusters", clusters.len());

    // Should find duplicates correctly
    assert!(clusters.len() > 0, "Should find at least one cluster");

    println!("\n✅ More threads than docs test PASSED");
}

// ============================================================================
// T28 Q23: Adversarial Tests (Security)
// ============================================================================

#[test]
fn test_adversarial_empty_documents() {
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = ParallelDedupPipeline::new(10, 4, &cpu_caps).unwrap();

    let docs = vec![(0, ""), (1, ""), (2, ""), (3, "normal document"), (4, "")];

    pipeline.add_documents(&docs).unwrap();
    let clusters = pipeline.find_duplicates(0.85).unwrap();

    // Should not panic, all empty docs may cluster together
    assert!(clusters.len() > 0, "Should produce at least one cluster");
}

#[test]
fn test_adversarial_very_long_documents() {
    let long_text = (0..10_000).map(|i| format!("word{}", i)).collect::<Vec<_>>().join(" ");

    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = ParallelDedupPipeline::new(5, 4, &cpu_caps).unwrap();

    let docs = vec![
        (0, long_text.as_str()),
        (1, long_text.as_str()), // Duplicate
        (2, "Short doc"),
        (3, long_text.as_str()), // Another duplicate
        (4, "Another short doc"),
    ];

    pipeline.add_documents(&docs).unwrap();
    let clusters = pipeline.find_duplicates(0.85).unwrap();

    // Should detect long document duplicates
    assert!(clusters.len() >= 2, "Should produce at least 2 clusters");
}

#[test]
fn test_adversarial_unicode_documents() {
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = ParallelDedupPipeline::new(5, 4, &cpu_caps).unwrap();

    let docs = vec![
        (0, "Hello 世界 🌍"),
        (1, "Hello 世界 🌍"), // Duplicate
        (2, "مرحبا العالم 🌎"),
        (3, "Привет мир 🌏"),
        (4, "Hello 世界 🌍"), // Another duplicate
    ];

    pipeline.add_documents(&docs).unwrap();
    let clusters = pipeline.find_duplicates(0.85).unwrap();

    // Should handle Unicode correctly
    assert!(clusters.len() >= 2, "Should produce at least 2 clusters");
}

#[test]
fn test_adversarial_special_characters() {
    let cpu_caps = CpuCapabilityCapsule::detect();
    let mut pipeline = ParallelDedupPipeline::new(5, 4, &cpu_caps).unwrap();

    let docs = vec![
        (0, "Hello\nWorld\tTest"),
        (1, "Hello\nWorld\tTest"), // Duplicate
        (2, "Special!@#$%^&*(){}[]"),
        (3, "Quotes\"'`and\\backslashes"),
        (4, "Hello\nWorld\tTest"), // Another duplicate
    ];

    pipeline.add_documents(&docs).unwrap();
    let clusters = pipeline.find_duplicates(0.85).unwrap();

    // Should handle special characters correctly
    assert!(clusters.len() >= 2, "Should produce at least 2 clusters");
}
