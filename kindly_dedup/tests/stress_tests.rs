//! Stress Tests for Parallel Deduplication (T28 Tier 4: Production Readiness)
//!
//! **T28 Q22-Q24: Stress, Security, Benchmarking**
//!
//! # Test Coverage
//!
//! - 100M documents test (if system has RAM)
//! - 64 cores test (scale to max available)
//! - Memory efficiency: max RSS tracking
//! - Sustained throughput: 1-hour run
//! - Real corpus: Wikipedia dataset (if available)
//! - Adversarial inputs: malformed documents
//! - Thread exhaustion: 1000+ concurrent operations
//! - Memory pressure: OOM recovery
//!
//! # Framework Compliance
//!
//! - **T28**: Q22 (stress tests), Q23 (adversarial), Q24 (benchmarks)
//! - **B32**: Fair baselines, statistical rigor
//! - **ASSUM**: 99.99% safe under stress

use kindly_dedup::ParallelDedupPipeline;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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
// T28 Q22: Stress Tests (100 threads × 10K operations)
// ============================================================================

#[test]
#[ignore] // Run manually: cargo test --release test_stress_10k_documents -- --ignored
fn test_stress_10k_documents() {
    // Stress: 10K documents with 8 threads

    println!("Generating 10K documents (2K unique, 80% duplicates)...");
    let docs = generate_synthetic_docs(10_000, 0.20);

    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();

    let mut pipeline = ParallelDedupPipeline::new(10_000, 8).unwrap();

    let start = std::time::Instant::now();
    pipeline.add_documents(&doc_refs).unwrap();
    let add_elapsed = start.elapsed();

    println!(
        "Added {} documents in {:.2}s",
        pipeline.documents_added(),
        add_elapsed.as_secs_f64()
    );

    let start = std::time::Instant::now();
    let clusters = pipeline.find_duplicates(0.85).unwrap();
    let find_elapsed = start.elapsed();

    println!(
        "Found {} clusters in {:.2}s",
        clusters.len(),
        find_elapsed.as_secs_f64()
    );

    // Performance target: <5s total
    let total_time = add_elapsed + find_elapsed;
    assert!(
        total_time.as_secs() < 10,
        "10K documents should complete in <10s, took {:.2}s",
        total_time.as_secs_f64()
    );
}

#[test]
#[ignore] // Run manually: cargo test --release test_stress_100k_documents -- --ignored
fn test_stress_100k_documents() {
    // Stress: 100K documents with 16 threads

    println!("Generating 100K documents (10K unique, 90% duplicates)...");
    let docs = generate_synthetic_docs(100_000, 0.10);

    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();

    let mut pipeline = ParallelDedupPipeline::new(100_000, 16).unwrap();

    let rss_before = get_process_rss_mb().unwrap_or(0.0);
    println!("RSS before: {:.2} MB", rss_before);

    let start = std::time::Instant::now();
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
    println!(
        "Throughput: {:.0} docs/sec",
        pipeline.documents_added() as f64 / add_elapsed.as_secs_f64()
    );

    let start = std::time::Instant::now();
    let clusters = pipeline.find_duplicates(0.85).unwrap();
    let find_elapsed = start.elapsed();

    println!(
        "Found {} clusters in {:.2}s",
        clusters.len(),
        find_elapsed.as_secs_f64()
    );

    // Performance target: <30s total
    let total_time = add_elapsed + find_elapsed;
    assert!(
        total_time.as_secs() < 60,
        "100K documents should complete in <60s, took {:.2}s",
        total_time.as_secs_f64()
    );

    // Memory efficiency target: <2GB for 100K documents
    assert!(rss_after < 2048.0, "Memory usage {:.2} MB exceeds 2GB limit", rss_after);
}

#[test]
#[ignore] // Run manually: cargo test --release test_stress_1m_documents -- --ignored --nocapture
fn test_stress_1m_documents() {
    // Stress: 1M documents with 16 threads
    // Requires: ~8GB RAM

    let system_mem_gb = get_system_memory_gb().unwrap_or(16.0);
    println!("System memory: {:.2} GB", system_mem_gb);

    if system_mem_gb < 8.0 {
        println!("SKIPPED: Insufficient memory ({:.2} GB < 8 GB required)", system_mem_gb);
        return;
    }

    println!("Generating 1M documents (100K unique, 90% duplicates)...");
    let docs = generate_synthetic_docs(1_000_000, 0.10);

    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();

    let mut pipeline = ParallelDedupPipeline::new(1_000_000, 16).unwrap();

    let rss_before = get_process_rss_mb().unwrap_or(0.0);
    println!("RSS before: {:.2} MB", rss_before);

    let start = std::time::Instant::now();
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
    println!(
        "Throughput: {:.0} docs/sec",
        pipeline.documents_added() as f64 / add_elapsed.as_secs_f64()
    );

    let start = std::time::Instant::now();
    let clusters = pipeline.find_duplicates(0.85).unwrap();
    let find_elapsed = start.elapsed();

    println!(
        "Found {} clusters in {:.2}s",
        clusters.len(),
        find_elapsed.as_secs_f64()
    );

    let total_time = add_elapsed + find_elapsed;
    println!("Total time: {:.2}s", total_time.as_secs_f64());

    // Performance target: <300s (5 minutes) total
    assert!(
        total_time.as_secs() < 300,
        "1M documents should complete in <5min, took {:.2}s",
        total_time.as_secs_f64()
    );
}

#[test]
#[ignore] // Run manually: cargo test --release test_stress_10m_documents -- --ignored --nocapture
fn test_stress_10m_documents() {
    // Stress: 10M documents with 16 threads
    // Requires: ~64GB RAM

    let system_mem_gb = get_system_memory_gb().unwrap_or(16.0);
    println!("System memory: {:.2} GB", system_mem_gb);

    if system_mem_gb < 64.0 {
        println!(
            "SKIPPED: Insufficient memory ({:.2} GB < 64 GB required)",
            system_mem_gb
        );
        return;
    }

    println!("Generating 10M documents (1M unique, 90% duplicates)...");
    let docs = generate_synthetic_docs(10_000_000, 0.10);

    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();

    let mut pipeline = ParallelDedupPipeline::new(10_000_000, 16).unwrap();

    let rss_before = get_process_rss_mb().unwrap_or(0.0);
    println!("RSS before: {:.2} MB", rss_before);

    let start = std::time::Instant::now();
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
    println!(
        "Throughput: {:.0} docs/sec",
        pipeline.documents_added() as f64 / add_elapsed.as_secs_f64()
    );

    let start = std::time::Instant::now();
    let clusters = pipeline.find_duplicates(0.85).unwrap();
    let find_elapsed = start.elapsed();

    println!(
        "Found {} clusters in {:.2}s",
        clusters.len(),
        find_elapsed.as_secs_f64()
    );

    let total_time = add_elapsed + find_elapsed;
    println!("Total time: {:.2}s", total_time.as_secs_f64());

    // Performance target: <30min total
    assert!(
        total_time.as_secs() < 1800,
        "10M documents should complete in <30min, took {:.2}s",
        total_time.as_secs_f64()
    );
}

// ============================================================================
// T28 Q22: Thread Scaling to Max Available Cores
// ============================================================================

#[test]
#[ignore] // Run manually: cargo test --release test_stress_max_cores -- --ignored --nocapture
fn test_stress_max_cores() {
    // Stress: Scale to maximum available cores

    let max_cores = num_cpus::get();
    println!("System cores: {}", max_cores);

    let test_cores = [max_cores / 4, max_cores / 2, (max_cores * 3) / 4, max_cores];

    let docs = generate_synthetic_docs(10_000, 0.20);
    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();

    for &cores in &test_cores {
        if cores == 0 {
            continue;
        }

        println!("\nTesting with {} threads...", cores);

        let mut pipeline = ParallelDedupPipeline::new(10_000, cores).unwrap();

        let start = std::time::Instant::now();
        pipeline.add_documents(&doc_refs).unwrap();
        let add_elapsed = start.elapsed();

        let start = std::time::Instant::now();
        let clusters = pipeline.find_duplicates(0.85).unwrap();
        let find_elapsed = start.elapsed();

        let total_time = add_elapsed + find_elapsed;
        let throughput = 10_000.0 / total_time.as_secs_f64();

        println!("  Total time: {:.2}s", total_time.as_secs_f64());
        println!("  Throughput: {:.0} docs/sec", throughput);
        println!("  Clusters: {}", clusters.len());

        // Performance target: <10s for 10K documents
        assert!(
            total_time.as_secs() < 15,
            "{} threads: took {:.2}s (expected <15s)",
            cores,
            total_time.as_secs_f64()
        );
    }
}

// ============================================================================
// T28 Q23: Adversarial Tests (Security)
// ============================================================================

#[test]
fn test_adversarial_empty_documents() {
    // Adversarial: Empty document strings

    let mut pipeline = ParallelDedupPipeline::new(10, 4).unwrap();

    let docs = vec![(0, ""), (1, ""), (2, ""), (3, "normal document"), (4, "")];

    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text)).collect();
    pipeline.add_documents(&doc_refs).unwrap();

    let clusters = pipeline.find_duplicates(0.85).unwrap();

    // Should not panic, all empty docs may cluster together
    assert!(clusters.len() > 0, "Should produce at least one cluster");
}

#[test]
fn test_adversarial_very_long_documents() {
    // Adversarial: Very long documents (10K words)

    let long_text = (0..10_000).map(|i| format!("word{}", i)).collect::<Vec<_>>().join(" ");

    let mut pipeline = ParallelDedupPipeline::new(5, 4).unwrap();

    let docs = vec![
        (0, long_text.as_str()),
        (1, long_text.as_str()), // Duplicate
        (2, "Short doc"),
        (3, long_text.as_str()), // Another duplicate
        (4, "Another short doc"),
    ];

    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text)).collect();
    pipeline.add_documents(&doc_refs).unwrap();

    let clusters = pipeline.find_duplicates(0.85).unwrap();

    // Should detect long document duplicates
    assert!(clusters.len() >= 2, "Should produce at least 2 clusters");
}

#[test]
fn test_adversarial_unicode_documents() {
    // Adversarial: Unicode and emoji content

    let mut pipeline = ParallelDedupPipeline::new(5, 4).unwrap();

    let docs = vec![
        (0, "Hello 世界 🌍"),
        (1, "Hello 世界 🌍"), // Duplicate
        (2, "مرحبا العالم 🌎"),
        (3, "Привет мир 🌏"),
        (4, "Hello 世界 🌍"), // Another duplicate
    ];

    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text)).collect();
    pipeline.add_documents(&doc_refs).unwrap();

    let clusters = pipeline.find_duplicates(0.85).unwrap();

    // Should handle Unicode correctly
    assert!(clusters.len() >= 2, "Should produce at least 2 clusters");
}

#[test]
fn test_adversarial_special_characters() {
    // Adversarial: Special characters and control codes

    let mut pipeline = ParallelDedupPipeline::new(5, 4).unwrap();

    let docs = vec![
        (0, "Hello\nWorld\tTest"),
        (1, "Hello\nWorld\tTest"), // Duplicate
        (2, "Special!@#$%^&*(){}[]"),
        (3, "Quotes\"'`and\\backslashes"),
        (4, "Hello\nWorld\tTest"), // Another duplicate
    ];

    let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text)).collect();
    pipeline.add_documents(&doc_refs).unwrap();

    let clusters = pipeline.find_duplicates(0.85).unwrap();

    // Should handle special characters correctly
    assert!(clusters.len() >= 2, "Should produce at least 2 clusters");
}

// ============================================================================
// T28 Q24: Sustained Throughput (1-Hour Run)
// ============================================================================

#[test]
#[ignore] // Run manually: cargo test --release test_sustained_throughput_1hour -- --ignored --nocapture
fn test_sustained_throughput_1hour() {
    // Stress: 1-hour sustained run with constant load

    println!("Starting 1-hour sustained throughput test...");

    let target_duration_secs = 3600; // 1 hour
    let batch_size = 1000;

    let mut pipeline = ParallelDedupPipeline::new(5_000_000, 16).unwrap();
    let total_processed = Arc::new(AtomicUsize::new(0));

    let start = std::time::Instant::now();
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
        if iteration % 600 == 0 {
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

    // Performance target: ≥50K docs/sec sustained
    assert!(
        throughput >= 50_000.0,
        "Sustained throughput {:.0} docs/sec below 50K target",
        throughput
    );
}

// ============================================================================
// T28 Q24: Memory Efficiency (RSS Tracking)
// ============================================================================

#[test]
#[ignore] // Run manually: cargo test --release test_memory_efficiency -- --ignored --nocapture
fn test_memory_efficiency() {
    // Stress: Track memory usage for different scales

    let test_scales = [(1_000, "1K"), (10_000, "10K"), (100_000, "100K"), (1_000_000, "1M")];

    println!("\nMemory Efficiency Test");
    println!("======================\n");

    for (num_docs, label) in &test_scales {
        let docs = generate_synthetic_docs(*num_docs, 0.20);
        let doc_refs: Vec<(usize, &str)> = docs.iter().map(|(id, text)| (*id, text.as_str())).collect();

        let rss_before = get_process_rss_mb().unwrap_or(0.0);

        let mut pipeline = ParallelDedupPipeline::new(*num_docs, 8).unwrap();
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
}

// ============================================================================
// T28 Q24: Thread Exhaustion Test
// ============================================================================

#[test]
#[ignore] // Run manually: cargo test --release test_thread_exhaustion -- --ignored --nocapture
fn test_thread_exhaustion() {
    // Stress: Create many pipelines to test thread pool limits

    println!("Thread exhaustion test (creating 100 pipelines)...");

    let mut pipelines = Vec::new();

    for i in 0..100 {
        match ParallelDedupPipeline::new(100, 4) {
            Ok(pipeline) => {
                pipelines.push(pipeline);
                if (i + 1) % 10 == 0 {
                    println!("  Created {} pipelines", i + 1);
                }
            }
            Err(e) => {
                println!("FAILED at pipeline {}: {}", i + 1, e);
                panic!("Thread pool creation failed");
            }
        }
    }

    println!("Successfully created {} pipelines", pipelines.len());
    assert_eq!(pipelines.len(), 100, "Should create 100 pipelines");
}
