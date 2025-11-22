//! Real-time monitoring dashboard demo
//!
//! This example demonstrates the monitoring dashboard with live metrics.
//!
//! # Usage
//! ```bash
//! cargo run --example monitoring_demo --features histogram
//! ```
//!
//! # Features
//! - Real-time metrics collection (<10ns overhead)
//! - Dashboard updates every 1 second
//! - Lockfree histogram (P50/P95/P99/P999)
//! - Alerting (P99 > 10ms, error rate > 1%, hit ratio < 80%)

#![cfg(feature = "histogram")]

use atomic_capsule::network::monitoring::{MetricsDashboard, GLOBAL_METRICS};
use std::thread;
use std::time::Duration;

fn main() {
    println!("╔════════════════════════════════════════════════════════════════════════════╗");
    println!("║                 Real-Time Monitoring Dashboard Demo                       ║");
    println!("╚════════════════════════════════════════════════════════════════════════════╝");
    println!();
    println!("Starting dashboard (updates every 1 second)...");
    println!("Press Ctrl+C to stop");
    println!();

    // Start dashboard (spawns background thread)
    let dashboard = MetricsDashboard::start(&GLOBAL_METRICS);

    // Simulate workload: 3 shards with different characteristics
    let workload = thread::spawn(|| {
        let mut iteration = 0;

        loop {
            iteration += 1;

            // Shard 1: High throughput, low latency, good hit ratio
            for _ in 0..100 {
                GLOBAL_METRICS[0].record_operation(500_000 + (iteration % 100) * 1000); // 0.5-1.5ms
                if iteration % 10 < 9 {
                    GLOBAL_METRICS[0].record_hit();
                } else {
                    GLOBAL_METRICS[0].record_miss();
                }
            }

            // Shard 2: Medium throughput, higher latency, medium hit ratio
            for _ in 0..50 {
                GLOBAL_METRICS[1].record_operation(2_000_000 + (iteration % 50) * 10000); // 2-2.5ms
                if iteration % 10 < 7 {
                    GLOBAL_METRICS[1].record_hit();
                } else {
                    GLOBAL_METRICS[1].record_miss();
                }
            }

            // Shard 3: Low throughput, varied latency, some errors
            for _ in 0..25 {
                GLOBAL_METRICS[2].record_operation(1_000_000 + (iteration % 200) * 5000); // 1-2ms
                if iteration % 10 < 8 {
                    GLOBAL_METRICS[2].record_hit();
                } else {
                    GLOBAL_METRICS[2].record_miss();
                }
            }

            // Simulate occasional errors
            if iteration % 100 == 0 {
                GLOBAL_METRICS[2].record_error();
            }

            // Simulate replication lag
            GLOBAL_METRICS[0].set_replication_lag(500_000); // 0.5ms
            GLOBAL_METRICS[1].set_replication_lag(1_200_000); // 1.2ms
            GLOBAL_METRICS[2].set_replication_lag(800_000); // 0.8ms

            // Sleep to simulate realistic workload (100ms between batches)
            thread::sleep(Duration::from_millis(100));

            // Run for 60 seconds
            if iteration >= 600 {
                break;
            }
        }

        println!("\nWorkload complete. Dashboard will continue for 5 more seconds...");
        thread::sleep(Duration::from_secs(5));
    });

    // Wait for workload to complete
    workload.join().unwrap();

    // Stop dashboard
    println!("\nStopping dashboard...");
    dashboard.stop();

    println!("\n╔════════════════════════════════════════════════════════════════════════════╗");
    println!("║                        Dashboard Demo Complete                             ║");
    println!("╚════════════════════════════════════════════════════════════════════════════╝");
}
