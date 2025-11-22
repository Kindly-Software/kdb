//! Worker Affinity Demo
//!
//! Demonstrates NUMA-aware worker affinity on cross-platform systems.
//!
//! **Usage**:
//! ```bash
//! cargo run --example worker_affinity_demo --features nightly-adaptive
//! ```

#[cfg(feature = "nightly-adaptive")]
use atomic_capsule::parallel::{compute_worker_assignment, CpuTopology};

#[cfg(feature = "nightly-adaptive")]
fn main() {
    println!("=== Worker Affinity Demo ===\n");

    // Detect CPU topology
    let topology = match CpuTopology::detect() {
        Ok(topo) => topo,
        Err(e) => {
            eprintln!("Failed to detect topology: {}", e);
            return;
        }
    };

    println!("CPU Topology:");
    println!("  Physical cores: {}", topology.num_cores());
    println!("  NUMA domains: {}", topology.num_numa_domains());
    println!("  Cache line size: {} bytes", topology.cache_line_size());
    println!("  Platform: {:?}\n", topology.platform());

    // Compute worker assignments (8 workers)
    let num_workers = 8.min(topology.num_cores());
    let assignments = compute_worker_assignment(num_workers, &topology);

    println!("Worker Assignments ({} workers):", num_workers);
    println!("{:<10} {:<12} {:<10}", "Worker ID", "NUMA Domain", "CPU ID");
    println!("{}", "-".repeat(35));

    for affinity in &assignments {
        println!(
            "{:<10} {:<12} {:<10}",
            affinity.worker_id, affinity.numa_domain, affinity.cpu_id
        );
    }

    println!("\n=== Affinity Pinning Test ===\n");

    // Test pinning (may fail without CAP_SYS_NICE)
    if let Some(affinity) = assignments.first() {
        match affinity.pin() {
            Ok(()) => {
                println!(
                    "✓ Successfully pinned current thread to CPU {}",
                    affinity.cpu_id
                );

                // Verify pinning on Linux
                #[cfg(target_os = "linux")]
                {
                    unsafe {
                        let current_cpu = libc::sched_getcpu();
                        println!("  Current CPU (after pinning): {}", current_cpu);
                        if current_cpu as usize == affinity.cpu_id {
                            println!("  ✓ Pinning verified!");
                        } else {
                            println!(
                                "  ⚠ Warning: Pinned to CPU {}, running on CPU {}",
                                affinity.cpu_id, current_cpu
                            );
                        }
                    }
                }
            }
            Err(e) => {
                println!("✗ Pinning failed: {:?}", e);
                println!("  Note: CPU pinning requires CAP_SYS_NICE capability on Linux");
                println!("  Run with: sudo setcap cap_sys_nice=eip ./target/debug/examples/worker_affinity_demo");
            }
        }
    }

    println!("\n=== NUMA Distribution Analysis ===\n");

    // Analyze NUMA distribution
    if topology.num_numa_domains() > 1 {
        let mut numa_counts = vec![0; topology.num_numa_domains()];
        for affinity in &assignments {
            numa_counts[affinity.numa_domain] += 1;
        }

        println!("Workers per NUMA domain:");
        for (numa_id, count) in numa_counts.iter().enumerate() {
            let percentage = (*count as f64 / num_workers as f64) * 100.0;
            println!("  NUMA {}: {} workers ({:.1}%)", numa_id, count, percentage);
        }
    } else {
        println!("Single NUMA domain system (UMA)");
    }

    println!("\nDemo complete!");
}

#[cfg(not(feature = "nightly-adaptive"))]
fn main() {
    eprintln!("This example requires the 'nightly-adaptive' feature.");
    eprintln!("Run with: cargo run --example worker_affinity_demo --features nightly-adaptive");
}
