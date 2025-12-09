use kindly_dedup::{Dedup, DedupMode};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Test with 10K documents
    let num_docs = 10_000;
    let mut dedup = Dedup::with_mode(DedupMode::CpuStreaming, num_docs)?;

    println!("Testing Dedup throughput (Facade API with CpuStreaming mode)");
    println!("Test configuration: {} documents", num_docs);
    println!("Mode: {:?}", dedup.current_mode());

    // Add documents
    let start = Instant::now();
    for i in 0..num_docs {
        let text = format!(
            "Document {} with machine learning and artificial intelligence content. \
            Deep learning networks and transformers are crucial. \
            Large language models represent the frontier of AI research.",
            i
        );
        dedup.add_document(i as u64, &text)?;
    }
    let add_elapsed = start.elapsed();
    let add_throughput = (num_docs as f64) / add_elapsed.as_secs_f64();

    println!("\nAdd Phase Results:");
    println!("  Time: {:.3}s", add_elapsed.as_secs_f64());
    println!("  Throughput: {:.0} docs/sec", add_throughput);
    println!("  Per-doc latency: {:.2} µs", add_elapsed.as_secs_f64() * 1_000_000.0 / num_docs as f64);

    // Find duplicates
    let start = Instant::now();
    let clusters = dedup.find_duplicates(0.85)?;
    let dedup_elapsed = start.elapsed();
    let dedup_throughput = (num_docs as f64) / dedup_elapsed.as_secs_f64();

    println!("\nDedup Phase Results:");
    println!("  Time: {:.3}s", dedup_elapsed.as_secs_f64());
    println!("  Throughput: {:.0} docs/sec", dedup_throughput);
    println!("  Per-doc latency: {:.2} µs", dedup_elapsed.as_secs_f64() * 1_000_000.0 / num_docs as f64);
    println!("  Clusters found: {}", clusters.len());

    // Total
    let total_elapsed = add_elapsed + dedup_elapsed;
    let total_throughput = (num_docs as f64) / total_elapsed.as_secs_f64();

    println!("\nEnd-to-End Results:");
    println!("  Total time: {:.3}s", total_elapsed.as_secs_f64());
    println!("  Total throughput: {:.0} docs/sec", total_throughput);
    println!("  Per-doc latency: {:.2} µs", total_elapsed.as_secs_f64() * 1_000_000.0 / num_docs as f64);

    // Statistics
    let stats = dedup.stats();
    println!("\nStatistics:");
    println!("  Documents processed: {}", stats.documents_processed);
    println!("  Total time: {:?}", stats.total_time);
    println!("  Avg time per doc: {:?}", stats.avg_time_per_doc);

    println!("\nComparison to 458 docs/sec baseline:");
    println!("  Current throughput: {:.0} docs/sec", total_throughput);
    println!("  Baseline: 458 docs/sec");
    println!("  Estimated speedup: {:.1}×", total_throughput / 458.0);

    Ok(())
}
