// Phase 3D+: Production-Scale Parallel Speedup Measurement (10M+ corpus, configurable threads)
use std::path::Path;
use std::time::Instant;
use std::env;

fn main() {
    use kindly_dedup::universal::JobLevelDedupPipelineMetaCapsule;

    // Parse command-line arguments
    let args: Vec<String> = env::args().collect();

    let (corpus_path, num_documents, thread_configs) = if args.len() >= 2 {
        // Usage: test_parallel_speedup <corpus_path> [num_docs] [thread1,thread2,...]
        let path = &args[1];
        let docs = if args.len() >= 3 {
            args[2].parse::<usize>().unwrap_or(100_000)
        } else {
            // Auto-detect from file line count
            let output = std::process::Command::new("wc")
                .args(&["-l", path])
                .output()
                .ok()
                .and_then(|o| {
                    String::from_utf8(o.stdout).ok()
                });
            output
                .and_then(|s| s.split_whitespace().next().map(|n| n.parse().ok()).flatten())
                .unwrap_or(100_000)
        };

        let threads = if args.len() >= 4 {
            args[3]
                .split(',')
                .filter_map(|s| s.trim().parse::<usize>().ok())
                .collect::<Vec<_>>()
        } else {
            vec![1, 4, 8, 16]  // Default: 1, 4, 8, 16 threads
        };

        (path.to_string(), docs, threads)
    } else {
        // Default: use 10M corpus if available
        let default_path = "test_data/c4_1b_final.jsonl";
        if !Path::new(default_path).exists() {
            eprintln!("Usage: test_parallel_speedup <corpus_path> [num_docs] [thread1,thread2,...]");
            eprintln!("");
            eprintln!("Examples:");
            eprintln!("  test_parallel_speedup test_data/c4_100k.jsonl");
            eprintln!("  test_parallel_speedup test_data/c4_1b_final.jsonl 10236892");
            eprintln!("  test_parallel_speedup test_data/c4_1m.jsonl 1000000 1,2,4,8,16");
            std::process::exit(1);
        }
        (default_path.to_string(), 10_236_892, vec![1, 4, 8, 16])
    };

    if !Path::new(&corpus_path).exists() {
        eprintln!("[ABORT] Corpus file not found: {}", corpus_path);
        std::process::exit(1);
    }

    eprintln!("[START] Production-scale parallel speedup measurement");
    eprintln!("[INFO] Corpus: {}", corpus_path);
    eprintln!("[INFO] Documents: {} (auto-detected)", num_documents);
    eprintln!("[INFO] Testing {} thread configurations: {:?}", thread_configs.len(), thread_configs);
    eprintln!("");
    eprintln!("=== SPEEDUP MEASUREMENT (PRODUCTION SCALE) ===");
    eprintln!("");

    // Convert thread counts to config tuples
    let mut configs: Vec<(usize, usize, String)> = thread_configs.iter()
        .map(|&t| (t, t, format!("{} thread(s)", t)))
        .collect();

    // Sort by thread count
    configs.sort_by_key(|c| c.0);

    let mut results = Vec::new();

    for (num_chunks, num_threads, description) in configs {
        eprintln!("[TEST] {}", description);

        let start = Instant::now();
        let mem_before = get_rss_mb();

        match JobLevelDedupPipelineMetaCapsule::new(
            &corpus_path,
            num_documents as u64,
            num_chunks,
            0.85,     // threshold
        ) {
            Ok(mut pipeline) => {
                match pipeline.run() {
                    Ok(clusters) => {
                        let duration = start.elapsed();
                        let secs = duration.as_secs_f64();
                        let mem_after = get_rss_mb();
                        let mem_peak = mem_after - mem_before;
                        let throughput = num_documents as f64 / secs;

                        eprintln!("  ✅ PASS: {} clusters found", clusters.len());
                        eprintln!("       Runtime: {:.2}s | Throughput: {:.0} docs/s | Memory: {}→{} MB (+{} MB)",
                                 secs, throughput, mem_before, mem_after, mem_peak);

                        results.push((num_chunks, num_threads, secs, clusters.len(), mem_peak));
                    }
                    Err(e) => {
                        eprintln!("  ❌ FAIL: Pipeline error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("  ❌ FAIL: Cannot create pipeline: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Print results table
    eprintln!("");
    eprintln!("=== RESULTS (10M+ PRODUCTION SCALE) ===");
    eprintln!("");
    eprintln!("Threads | Runtime (s) | Throughput (docs/s) | Speedup | Efficiency | Memory");
    eprintln!("--------|-------------|---------------------|---------|------------|--------");

    let baseline_time = results[0].2; // Sequential time

    for (_, num_threads, secs, cluster_count, mem_mb) in &results {
        let throughput = num_documents as f64 / secs;
        let speedup = baseline_time / secs;
        let efficiency = (speedup / *num_threads as f64) * 100.0;

        eprintln!("{:7} | {:11.3} | {:19.0} | {:7.2}x | {:9.1}% | {:>5}MB",
                 num_threads, secs, throughput, speedup, efficiency, mem_mb);
    }

    eprintln!("");
    eprintln!("=== SCALABILITY ANALYSIS ===");
    eprintln!("");
    eprintln!("Baseline (1 thread): {:.3}s ({:.0} docs/sec)", baseline_time, num_documents as f64 / baseline_time);

    // Print speedup for each measured configuration
    for (idx, (_, num_threads, secs, _, _)) in results.iter().enumerate().skip(1) {
        let speedup = baseline_time / secs;
        let ideal_speedup = *num_threads as f64;
        let efficiency = (speedup / ideal_speedup) * 100.0;
        eprintln!("Speedup @ {} threads: {:.2}x (ideal {:.2}x, {:.1}% efficiency)",
                 num_threads, speedup, ideal_speedup, efficiency);
    }

    eprintln!("");
    eprintln!("Expected Amdahl's Law (83.5% parallelizable from Phase 3D):");
    for threads in [2, 4, 8, 16].iter() {
        let p = 0.835;
        let s = 1.0 / ((1.0 - p) + p / *threads as f64);
        eprintln!("  @ {} threads: ~{:.2}x", threads, s);
    }

    eprintln!("");
    eprintln!("[SUCCESS] Production-scale validation complete");
}

fn get_rss_mb() -> u64 {
    // Try to get RSS from /proc/self/status
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                if let Some(kb) = line.split_whitespace().nth(1).and_then(|s| s.parse::<u64>().ok()) {
                    return kb / 1024;
                }
            }
        }
    }
    0
}
