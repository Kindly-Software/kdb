//! Latency measurement for end-to-end deduplication pipeline
//!
//! Measures:
//! - add_document latency (P50/P95/P99/P99.9/Max)
//! - find_duplicates latency (total and per-document)
//! - End-to-end latency validation (<1ms target)
//!
//! Usage:
//!   cargo run --release --bin measure_latency
//!
//! Target (from LLM_DEDUP_IMPLEMENTATION_ROADMAP.md):
//!   <1ms per document (end-to-end)

use anyhow::Result;
use kindly_dedup::DedupPipeline;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Document {
    id: usize,
    url: String,
    text: String,
}

fn main() -> Result<()> {
    println!("=== Latency Measurement ===\n");

    // Load corpus
    println!("Loading corpus...");
    let file = File::open("test_data/synthetic_100k.json")?;
    let documents: Vec<Document> = serde_json::from_reader(file)?;
    println!("Loaded {} documents\n", documents.len());

    // Take first 10K for measurement
    let sample_size = 10_000;
    let sample_docs = &documents[..sample_size.min(documents.len())];

    // Measure add_document latency
    println!("Measuring add_document latency ({} samples)...", sample_size);
    let mut add_latencies = Vec::with_capacity(sample_size);
    let mut pipeline = DedupPipeline::new(sample_size);

    for doc in sample_docs {
        let start = Instant::now();
        pipeline.add_document(doc.id, &doc.text);
        add_latencies.push(start.elapsed().as_micros());
    }

    add_latencies.sort_unstable();

    let mean_add = add_latencies.iter().sum::<u128>() as f64 / add_latencies.len() as f64;
    let p50_add = add_latencies[sample_size / 2];
    let p95_add = add_latencies[sample_size * 95 / 100];
    let p99_add = add_latencies[sample_size * 99 / 100];
    let p999_add = add_latencies[sample_size * 999 / 1000];
    let max_add = add_latencies.last().unwrap();

    println!("\nAdd Document Latency ({} samples):", sample_size);
    println!("  Mean:  {:.2}μs", mean_add);
    println!("  P50:   {}μs", p50_add);
    println!("  P95:   {}μs", p95_add);
    println!("  P99:   {}μs", p99_add);
    println!("  P99.9: {}μs", p999_add);
    println!("  Max:   {}μs", max_add);

    // Measure find_duplicates latency
    println!("\nMeasuring find_duplicates latency...");
    let start = Instant::now();
    let clusters = pipeline.find_duplicates(0.85);
    let dedup_time = start.elapsed();

    let dedup_time_us = dedup_time.as_micros();
    let dedup_per_doc_us = dedup_time_us as f64 / sample_size as f64;

    println!("\nFind Duplicates Latency:");
    println!(
        "  Total:     {:.2}ms ({} μs)",
        dedup_time.as_secs_f64() * 1000.0,
        dedup_time_us
    );
    println!("  Per doc:   {:.2}μs", dedup_per_doc_us);
    println!("  Clusters:  {}", clusters.len());

    // Calculate duplicate statistics
    let total_docs = clusters.iter().map(|c| c.len()).sum::<usize>();
    let duplicates = total_docs.saturating_sub(clusters.len());
    let dedup_percentage = (duplicates as f64 / total_docs as f64) * 100.0;

    println!("  Total docs in clusters: {}", total_docs);
    println!("  Duplicates found:       {}", duplicates);
    println!("  Dedup percentage:       {:.1}%", dedup_percentage);

    // End-to-end latency (amortized add + dedup)
    let end_to_end_us = mean_add + dedup_per_doc_us;

    println!("\n=== End-to-End Latency (Add + Dedup) ===");
    println!("  Mean add:         {:.2}μs", mean_add);
    println!("  Dedup per doc:    {:.2}μs", dedup_per_doc_us);
    println!("  End-to-end:       {:.2}μs", end_to_end_us);
    println!("  Target:           <1000μs (1ms)");

    // GO/NO-GO decision
    let target_us = 1000.0;
    let passes = end_to_end_us < target_us;
    let margin = target_us / end_to_end_us;

    if passes {
        println!("\n  Status:    ✓ PASS ({:.1}× better than target)", margin);
        println!("\n=== GO/NO-GO: GO ===");
        println!("Performance target met. Ready for production deployment.");
    } else {
        println!(
            "\n  Status:    ✗ FAIL ({:.1}× slower than target)",
            end_to_end_us / target_us
        );
        println!("\n=== GO/NO-GO: NO-GO ===");
        println!("Performance target NOT met. Optimization required.");
    }

    // P99 breakdown
    println!("\n=== P99 Latency Analysis ===");
    println!("  Add P99:          {}μs", p99_add);
    println!("  Dedup (amortized): {:.2}μs", dedup_per_doc_us);
    println!("  P99 end-to-end:   {:.2}μs", p99_add as f64 + dedup_per_doc_us);
    println!("  P99 target:       <1000μs (1ms)");

    let p99_total = p99_add as f64 + dedup_per_doc_us;
    let p99_passes = p99_total < target_us;

    if p99_passes {
        println!("  P99 status:       ✓ PASS ({:.1}× better)", target_us / p99_total);
    } else {
        println!("  P99 status:       ✗ FAIL ({:.1}× slower)", p99_total / target_us);
    }

    // Summary report
    println!("\n=== Summary Report ===");
    println!("Sample size:         {} documents", sample_size);
    println!("Mean latency:        {:.2}μs/doc", end_to_end_us);
    println!("P99 latency:         {:.2}μs/doc", p99_total);
    println!("Throughput:          {:.0} docs/sec", 1_000_000.0 / end_to_end_us);
    println!("Target throughput:   1,000 docs/sec (1ms/doc)");
    println!("Status:              {}", if passes { "READY" } else { "NEEDS WORK" });

    Ok(())
}
