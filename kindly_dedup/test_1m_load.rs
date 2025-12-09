#!/usr/bin/env bash

# 1M Document Test with Debug Logging
# This script tests the pipeline on 1M documents to identify hang locations

set -e

echo "===== Q1: Creating 1M Document Subset ====="
echo "Using existing: test_data/c4_1m.jsonl (775MB, 1,000,000 docs)"
echo ""

echo "===== Q2: Building benchmark with debug-logging ====="
cargo build --release --lib --features "debug-logging,benchmarking" 2>&1 | grep -E "(Finished|error:|warning:)" | head -5
echo ""

echo "===== Q3-Q4: Running 1M test with monitoring ====="
echo "Starting time-travel loader test..."
echo ""

# Use Rust to load the file
cat > /tmp/test_1m_benchmark.rs << 'RUST_CODE'
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

fn main() {
    let path = "test_data/c4_1m.jsonl";
    let start = Instant::now();

    println!("[INIT] Starting 1M document test");
    println!("[CHECKPOINT] Loading from: {}", path);

    let file = File::open(path).expect("Failed to open file");
    let reader = BufReader::new(file);

    println!("[CHECKPOINT] File opened at T+{:.3}s", start.elapsed().as_secs_f64());

    let mut doc_count = 0u64;
    let mut last_report = 0;
    let report_interval = 50_000;

    for (line_no, line_result) in reader.lines().enumerate() {
        if line_no == 0 {
            println!("[CHECKPOINT] Processing first document at T+{:.3}s",
                     start.elapsed().as_secs_f64());
        }

        match line_result {
            Ok(text) => {
                doc_count += 1;

                // Report progress every 50K docs
                if doc_count > last_report + report_interval {
                    let elapsed = start.elapsed().as_secs_f64();
                    let rate = (doc_count as f64) / elapsed;
                    println!("[PROGRESS] T+{:.2}s: {} docs @ {:.0} docs/sec",
                             elapsed, doc_count, rate);
                    last_report = doc_count;
                }
            }
            Err(e) => {
                eprintln!("[ERROR] Line {}: {}", line_no, e);
                break;
            }
        }

        if doc_count >= 1_000_000 {
            break;
        }
    }

    let elapsed = start.elapsed();
    let rate = (doc_count as f64) / elapsed.as_secs_f64();

    println!("\n===== RESULTS =====");
    println!("Documents processed: {}", doc_count);
    println!("Total time: {:.3}s", elapsed.as_secs_f64());
    println!("Throughput: {:.0} docs/sec", rate);

    if rate > 60_000.0 {
        println!("Classification: EXCEPTIONAL (>60K docs/sec)");
    } else if rate > 30_000.0 {
        println!("Classification: GOOD (30-60K docs/sec)");
    } else {
        println!("Classification: SLOW (<30K docs/sec)");
    }
}
RUST_CODE

echo ""
echo "===== Q5-Q6: Test Results ====="
cd /home/samuel/Primitives/kindly_dedup
rustc --edition 2021 -O /tmp/test_1m_benchmark.rs -o /tmp/test_1m_benchmark
timeout 300 /tmp/test_1m_benchmark

echo ""
echo "===== Q7: Recommendations ====="
echo "- If test completed quickly (>17s for 1M): Issue is NOT file loading"
echo "- If test hangs: Hang is in pipeline initialization or first doc processing"
echo "- Next: Run with actual UniversalDedupPipeline to isolate"
