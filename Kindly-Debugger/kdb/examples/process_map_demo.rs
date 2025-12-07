//! ProcessMapCapsule Demo - T5 Streaming /proc/pid/maps Parser
//!
//! Demonstrates parsing and querying memory maps for the current process.
//!
//! # Compile & Run
//! ```sh
//! cargo run --example process_map_demo --features std
//! ```

#[cfg(target_os = "linux")]
fn main() {
    use kdb::ProcessMapCapsule;
    use std::process;

    println!("=== ProcessMapCapsule Demo ===\n");

    // Create capsule
    let capsule = ProcessMapCapsule::new();
    let pid = process::id();

    println!("Parsing /proc/{}/maps...", pid);
    match capsule.parse_maps(pid) {
        Ok(_) => {
            let count = capsule.region_count();
            println!("✓ Successfully parsed {} memory regions\n", count);

            // Print first 10 regions
            let regions = capsule.get_all_regions();
            println!("Memory regions (first 10):");
            println!("{:<18} {:<18} {:<6} {:<6}", "Start", "End", "Read", "Write");
            println!("{:-<54}", "");

            for (i, (start, end, perms)) in regions.iter().take(10).enumerate() {
                let size_kb = (end - start) / 1024;
                println!(
                    "{:016x}  {:016x}  {:<6} {:<6}  ({} KB)",
                    start,
                    end,
                    if perms.read { "✓" } else { "-" },
                    if perms.write { "✓" } else { "-" },
                    size_kb
                );

                if i == 9 && regions.len() > 10 {
                    println!("... and {} more regions", regions.len() - 10);
                    break;
                }
            }

            // Demo: Find region containing current code
            println!("\n--- Region Lookup Demo ---");
            let current_addr = main as *const () as u64;
            println!("Current code address: {:016x}", current_addr);

            match capsule.find_region(current_addr) {
                Some((start, end, perms)) => {
                    let size_kb = (end - start) / 1024;
                    println!(
                        "✓ Found in region: {:016x}-{:016x} ({} KB)",
                        start, end, size_kb
                    );
                    println!(
                        "  Permissions: {}{}{}",
                        if perms.read { "r" } else { "-" },
                        if perms.write { "w" } else { "-" },
                        if perms.exec { "x" } else { "-" }
                    );
                }
                None => {
                    println!("✗ No region found (unexpected!)");
                }
            }

            // Demo: Find stack region
            println!("\n--- Stack Region Lookup ---");
            let stack_addr = &capsule as *const _ as u64;
            println!("Stack address (capsule var): {:016x}", stack_addr);

            match capsule.find_region(stack_addr) {
                Some((start, end, perms)) => {
                    println!("✓ Found stack in: {:016x}-{:016x}", start, end);
                    println!(
                        "  Permissions: {}{}{}",
                        if perms.read { "r" } else { "-" },
                        if perms.write { "w" } else { "-" },
                        if perms.exec { "x" } else { "-" }
                    );
                }
                None => {
                    println!("✗ No region found");
                }
            }

            // Performance demo
            println!("\n--- Performance Demo ---");
            let start_time = std::time::Instant::now();
            for _ in 0..100 {
                let _ = capsule.find_region(0x7f0000000000);
            }
            let elapsed = start_time.elapsed();
            let avg_us = elapsed.as_micros() as f64 / 100.0;
            println!(
                "100 region lookups: {:?} ({:.3} μs/lookup)",
                elapsed, avg_us
            );

            println!("\n--- Capsule Stats ---");
            println!("Regions parsed:     {}", count);
            println!("Generation:         {}", capsule.generation());
            println!("Cached PID:         {}", capsule.cached_pid());
        }
        Err(e) => {
            eprintln!("✗ Failed to parse maps: {:?}", e);
            std::process::exit(1);
        }
    }

    println!("\n✓ Demo completed successfully!");
}

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("This example requires Linux (ProcessMapCapsule is Linux-specific)");
    println!("Run on Linux with: cargo run --example process_map_demo --features std");
}
