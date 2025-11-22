//! Hierarchical Work-Stealing Demo
//!
//! Demonstrates multi-level stealing hierarchy with platform-aware backoff

use atomic_capsule::parallel::hierarchical_steal::{StealHierarchy, StealLevel};
use atomic_capsule::parallel::topology::CpuTopology;

fn main() {
    println!("=== Hierarchical Work-Stealing Demo ===\n");

    // Detect CPU topology
    let topology = CpuTopology::detect().expect("Failed to detect topology");

    println!("CPU Topology:");
    println!("  Cores: {}", topology.num_cores());
    println!("  NUMA domains: {}", topology.num_numa_domains());
    println!("  Cache line size: {} bytes", topology.cache_line_size());
    println!("  Platform: {:?}\n", topology.platform());

    // Build stealing hierarchy for worker 0
    let worker_id = 0;
    let hierarchy = StealHierarchy::from_topology(&topology, worker_id);

    println!("Stealing Hierarchy for Worker {}:", worker_id);
    println!("  Total levels: {}", hierarchy.levels.len());

    for (idx, (level, workers)) in hierarchy.levels.iter().enumerate() {
        println!("\n  Level {}: {:?}", idx + 1, level);
        println!("    Workers: {} cores", workers.len());
        println!("    Latency: {} ns", level.latency_ns());
        println!("    Backoff spins: {}", level.backoff_spins());

        if workers.len() <= 8 {
            println!("    Worker IDs: {:?}", workers);
        } else {
            println!("    Worker IDs: {:?}...", &workers[..8]);
        }
    }

    // Test fairness metrics
    println!("\n\nFairness Metrics (before any steals):");
    let metrics = hierarchy.fairness_metrics();
    for (level, attempts, successes, success_rate) in metrics {
        println!(
            "  {:?}: {} attempts, {} successes, {:.1}% success rate",
            level,
            attempts,
            successes,
            success_rate * 100.0
        );
    }

    println!("\n=== Demo Complete ===");
}
