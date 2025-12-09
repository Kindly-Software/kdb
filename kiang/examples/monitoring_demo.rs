//! Monitoring & Observability Demo
//!
//! Demonstrates KIANG's lockfree metrics collection and Prometheus export.
//!
//! # Architecture
//!
//! 1. MetricsCapsule: Atomic 256-bit capsule for lockfree metrics
//! 2. Hot path: Atomic increments (<15ns each)
//! 3. Prometheus export: HTTP endpoint on port 9090
//! 4. Concurrent safety: Multiple threads updating without locks
//!
//! # Usage
//!
//! ```bash
//! cargo run --example monitoring_demo
//! ```
//!
//! Then visit http://localhost:9090/metrics for Prometheus scraping.

use kiang::monitoring::{MetricsCapsule, MetricsExporter};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    // Initialize tracing for structured logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== KIANG Monitoring & Observability Demo ===\n");

    // Create metrics capsule (256-bit atomic capsule)
    let metrics = Arc::new(MetricsCapsule::new());
    println!("✓ Created MetricsCapsule (256-bit atomic, 64-byte aligned)");

    // Start metrics exporter in background thread
    let exporter_metrics = metrics.clone();
    let exporter_handle = thread::spawn(move || {
        let exporter = MetricsExporter::new(exporter_metrics, 9090);
        println!("✓ Starting metrics server on http://localhost:9090/metrics");
        println!("  Available endpoints:");
        println!("    - GET /metrics  (Prometheus format)");
        println!("    - GET /health   (Health check)");
        println!();

        if let Err(e) = exporter.start() {
            eprintln!("Metrics server error: {}", e);
        }
    });

    // Give server time to start
    thread::sleep(Duration::from_millis(100));

    println!("=== Simulating GPU Workload ===\n");

    // Simulate GPU command submission workload
    let workload_metrics = metrics.clone();
    let workload_handle = thread::spawn(move || {
        for batch in 1..=5 {
            println!("Batch {}: Submitting commands...", batch);

            let start = Instant::now();

            // Simulate command submissions
            for _ in 0..1000 {
                workload_metrics.increment_commands_submitted();

                // Simulate command processing
                thread::sleep(Duration::from_micros(10));

                // 95% success rate
                if rand::random::<u8>() < 242 {
                    workload_metrics.increment_commands_completed();
                } else {
                    workload_metrics.increment_commands_failed();
                }
            }

            let elapsed = start.elapsed();
            let latency_ns = (elapsed.as_nanos() / 1000) as u32;
            workload_metrics.update_avg_latency_ns(latency_ns);

            println!("  Completed batch {} in {:?}", batch, elapsed);
            println!("  Average latency: {} ns", latency_ns);

            // Read and display metrics
            if let Some(snapshot) = workload_metrics.read() {
                println!(
                    "  Commands: {} submitted, {} completed, {} failed",
                    snapshot.commands_submitted,
                    snapshot.commands_completed,
                    snapshot.commands_failed
                );
                println!("  Success rate: {:.2}%", snapshot.success_rate() * 100.0);
                println!("  In flight: {}", snapshot.commands_in_flight());
            }

            println!();
            thread::sleep(Duration::from_secs(1));
        }
    });

    // Simulate memory allocation tracking
    let memory_metrics = metrics.clone();
    let memory_handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(500));

        for cycle in 1..=10 {
            // Simulate memory allocations
            let allocated_mb = 512 + (cycle * 128);
            let freed_mb = cycle * 64;

            memory_metrics.update_memory_allocated_mb(allocated_mb);
            memory_metrics.update_memory_freed_mb(freed_mb);

            if cycle % 3 == 0 {
                if let Some(snapshot) = memory_metrics.read() {
                    println!(
                        "Memory cycle {}: {} MB allocated, {} MB freed (net: {} MB)",
                        cycle,
                        snapshot.memory_allocated_mb,
                        snapshot.memory_freed_mb,
                        snapshot.net_memory_mb()
                    );
                }
            }

            thread::sleep(Duration::from_millis(500));
        }
    });

    // Monitor and display metrics periodically
    let monitor_metrics = metrics.clone();
    let monitor_handle = thread::spawn(move || {
        thread::sleep(Duration::from_secs(2));

        for _ in 0..8 {
            if let Some(snapshot) = monitor_metrics.read() {
                println!("\n--- Metrics Snapshot ---");
                println!("Commands:");
                println!("  Submitted:  {}", snapshot.commands_submitted);
                println!("  Completed:  {}", snapshot.commands_completed);
                println!("  Failed:     {}", snapshot.commands_failed);
                println!("  In flight:  {}", snapshot.commands_in_flight());
                println!("Performance:");
                println!("  Avg latency: {} ns", snapshot.avg_latency_ns);
                println!("  Success rate: {:.2}%", snapshot.success_rate() * 100.0);
                println!("Memory:");
                println!("  Allocated:  {} MB", snapshot.memory_allocated_mb);
                println!("  Freed:      {} MB", snapshot.memory_freed_mb);
                println!("  Net usage:  {} MB", snapshot.net_memory_mb());
                println!("System:");
                println!("  Uptime:     {} seconds", snapshot.uptime_seconds);
                println!("  Resets:     {}", snapshot.reset_count);
                println!("------------------------\n");
            }

            thread::sleep(Duration::from_secs(1));
        }
    });

    // Demonstrate concurrent metric updates
    println!("=== Testing Concurrent Updates ===\n");

    let mut concurrent_handles = vec![];
    for worker_id in 0..5 {
        let worker_metrics = metrics.clone();
        concurrent_handles.push(thread::spawn(move || {
            for _ in 0..200 {
                worker_metrics.increment_commands_submitted();
                thread::sleep(Duration::from_micros(50));
            }
            println!("Worker {} completed", worker_id);
        }));
    }

    for handle in concurrent_handles {
        handle.join().unwrap();
    }

    println!("\n✓ All concurrent workers completed");

    // Wait for simulation to complete
    workload_handle.join().unwrap();
    memory_handle.join().unwrap();
    monitor_handle.join().unwrap();

    // Final metrics report
    println!("\n=== Final Metrics Report ===\n");

    if let Some(snapshot) = metrics.read() {
        println!("Summary:");
        println!(
            "  Total commands submitted: {}",
            snapshot.commands_submitted
        );
        println!(
            "  Total commands completed: {}",
            snapshot.commands_completed
        );
        println!("  Total commands failed:    {}", snapshot.commands_failed);
        println!(
            "  Overall success rate:     {:.2}%",
            snapshot.success_rate() * 100.0
        );
        println!("  Final latency:            {} ns", snapshot.avg_latency_ns);
        println!(
            "  Final memory usage:       {} MB",
            snapshot.net_memory_mb()
        );

        println!("\n--- Prometheus Export ---");
        let prom = snapshot.to_prometheus();
        println!("{}", prom);
    }

    println!("\n=== Prometheus Scraping ===");
    println!("Metrics server still running on http://localhost:9090/metrics");
    println!("Press Ctrl+C to stop the server.");
    println!("\nExample Prometheus scrape config:");
    println!("```yaml");
    println!("scrape_configs:");
    println!("  - job_name: 'kiang'");
    println!("    static_configs:");
    println!("      - targets: ['localhost:9090']");
    println!("```");

    // Keep server running
    exporter_handle.join().unwrap();
}

// Simple random number generator for demo
mod rand {
    use std::cell::RefCell;

    thread_local! {
        static RNG: RefCell<u64> = RefCell::new(0x123456789abcdef0);
    }

    pub fn random<T: From<u8>>() -> T {
        RNG.with(|rng| {
            let mut state = rng.borrow_mut();
            *state ^= *state << 13;
            *state ^= *state >> 7;
            *state ^= *state << 17;
            T::from((*state & 0xFF) as u8)
        })
    }
}
