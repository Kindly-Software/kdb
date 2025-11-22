//! # CPU Topology Detection Demo
//!
//! Demonstrates universal cross-platform CPU topology detection
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example topology_demo
//! ```

use atomic_capsule::parallel::topology::CpuTopology;

fn main() {
    println!("=== CPU Topology Detection Demo ===\n");

    // Detect topology (cached after first call)
    let topo = CpuTopology::detect().expect("Failed to detect CPU topology");

    // Print basic topology
    println!("Physical Cores: {}", topo.num_cores());
    println!("NUMA Domains: {}", topo.num_numa_domains());
    println!("Cache Line Size: {} bytes", topo.cache_line_size());
    println!("Platform: {}\n", topo.platform().description());

    // Print core → NUMA mapping
    println!("Core → NUMA Mapping:");
    for core_id in 0..topo.num_cores().min(16) {
        // Show first 16 cores
        if let Some(numa) = topo.core_numa(core_id) {
            println!("  Core {}: NUMA {}", core_id, numa);
        }
    }
    if topo.num_cores() > 16 {
        println!("  ... (showing first 16 of {} cores)", topo.num_cores());
    }

    // Print NUMA distance matrix
    if topo.num_numa_domains() > 1 {
        println!("\nNUMA Distance Matrix:");
        print!("     ");
        for j in 0..topo.num_numa_domains() {
            print!("{:3} ", j);
        }
        println!();

        for i in 0..topo.num_numa_domains() {
            print!("{:3}: ", i);
            for j in 0..topo.num_numa_domains() {
                print!("{:3} ", topo.numa_distance(i, j));
            }
            println!();
        }
    }

    // Platform-specific steal distance demo
    println!("\nWork-Stealing Distance (first 4 cores):");
    let platform = topo.platform();
    for from in 0..topo.num_cores().min(4) {
        for to in 0..topo.num_cores().min(4) {
            let dist = platform.steal_distance(from, to);
            print!("{:3} ", dist);
        }
        println!();
    }

    println!("\n✅ Topology detection successful!");
    println!("🔥 Performance: <100ns lookup after caching");
}
