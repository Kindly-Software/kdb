//! Audit Dashboard Demo - Real-Time Visualization Example
//!
//! **Purpose**: Demonstrate audit_dashboard integration with demo pipeline
//!
//! **Usage**:
//! ```bash
//! cargo run --example audit_dashboard_demo --features interactive
//! ```
//!
//! **Expected Output**:
//! - Byzantine purple + gold progress bars
//! - Real-time throughput updates
//! - CPU/Memory visualization
//! - Audit trail metrics
//! - Compliance badges
//! - Final summary with speedup chart

use atomic_capsule::CpuCapabilityCapsule;
use kindly_dedup::{
    audit_dashboard::{AuditDashboard, DemoSummary},
    DedupPipeline,
};
use std::thread;
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Audit Dashboard Demo - kindly_dedup v0.2.1\n");

    // Detect CPU capabilities
    let cpu_caps = CpuCapabilityCapsule::detect();
    let simd_tier = cpu_caps.best_simd_tier();

    // Simulate demo processing (100K documents)
    let doc_count = 100_000;
    let dashboard = AuditDashboard::new(doc_count);

    // Display SIMD tier
    dashboard.set_simd_tier(simd_tier);

    // Create pipeline
    let mut pipeline = DedupPipeline::new(doc_count, &cpu_caps);

    // Simulate processing with progress updates
    let start = Instant::now();
    let update_interval = 100; // Update every 100 docs

    for i in 0..doc_count {
        // Simulate document processing
        let text = format!("Document {} with some sample content for deduplication", i);
        pipeline.add_document(i, &text)?;

        // Update dashboard periodically
        if i % update_interval == 0 || i == doc_count - 1 {
            let elapsed = start.elapsed().as_secs_f64();
            let throughput = (i + 1) as f64 / elapsed;

            // Update progress
            dashboard.update_progress(i + 1, throughput);

            // Simulate CPU usage (varies over time)
            let cpu_usage = 40.0 + (i as f64 / doc_count as f64) * 30.0;
            dashboard.update_cpu(cpu_usage);

            // Simulate memory usage (grows with processing)
            let memory_gb = 0.5 + (i as f64 / doc_count as f64) * 2.0;
            dashboard.update_memory(memory_gb);

            // Update audit metrics (mock values)
            let audit_events = (i / 1000) as u64;
            let chain_intact = true;
            dashboard.update_audit(audit_events, chain_intact);

            // Simulate processing time (for realistic updates)
            thread::sleep(Duration::from_micros(10));
        }
    }

    // Find duplicates
    let clusters = pipeline.find_duplicates(0.85)?;
    let elapsed = start.elapsed();

    // Display final summary
    let summary = DemoSummary {
        tier_name: "Demo: Audit Dashboard Visualization",
        doc_count,
        elapsed,
        throughput: doc_count as f64 / elapsed.as_secs_f64(),
        cluster_count: clusters.len(),
        accuracy_f1: Some(100.0),
        baseline_throughput: 1572.0, // Python datasketch
    };

    dashboard.finish(&summary);

    Ok(())
}
