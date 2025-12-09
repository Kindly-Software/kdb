//! Quick Demo - Git Coordinator Usage
//!
//! Demonstrates basic usage of the git coordinator for Claude Code workflows.

use git_coordinator_bench::{GitCoordinator, LockMetrics, QueueMetrics};
use std::time::Instant;

fn main() {
    println!("=== Git Lock Coordinator Demo ===\n");

    // Create coordinator for this instance
    let coord = GitCoordinator::new(1);

    println!("1. Single-instance commit workflow");
    let start = Instant::now();
    coord.execute(|| {
        println!("   - Reading file...");
        std::thread::sleep(std::time::Duration::from_micros(100));
        println!("   - Modifying content...");
        std::thread::sleep(std::time::Duration::from_micros(200));
        println!("   - git add...");
        std::thread::sleep(std::time::Duration::from_micros(500));
        println!("   - git commit...");
        std::thread::sleep(std::time::Duration::from_millis(1));
    }).unwrap();
    let elapsed = start.elapsed();
    println!("   Total time: {:?}\n", elapsed);

    println!("2. Lock metrics");
    let metrics: LockMetrics = coord.lock.metrics();
    println!("   Acquires: {}", metrics.acquires);
    println!("   Releases: {}", metrics.releases);
    println!("   Waiters: {}", metrics.waiters);
    println!("   Timeouts: {}\n", metrics.timeouts);

    println!("3. Queue metrics");
    let queue_metrics: QueueMetrics = coord.queue.metrics();
    println!("   Depth: {}", queue_metrics.depth);
    println!("   Enqueues: {}", queue_metrics.enqueues);
    println!("   Dequeues: {}", queue_metrics.dequeues);
    println!("   Drops: {}\n", queue_metrics.drops);

    println!("4. Multi-instance simulation (4 concurrent)");
    let handles: Vec<_> = (0..4)
        .map(|tid| {
            let coord_clone = coord.clone_shared(tid as u32);
            std::thread::spawn(move || {
                coord_clone.execute(|| {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                    tid
                }).unwrap()
            })
        })
        .collect();

    let start = Instant::now();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let elapsed = start.elapsed();

    println!("   Completed instances: {:?}", results);
    println!("   Total time: {:?}", elapsed);
    println!("   Average per instance: {:?}\n", elapsed / 4);

    println!("5. Final lock metrics");
    let final_metrics: LockMetrics = coord.lock.metrics();
    println!("   Total acquires: {}", final_metrics.acquires);
    println!("   Total releases: {}", final_metrics.releases);
    println!("   Peak waiters: {}", final_metrics.waiters);
    println!("   Total timeouts: {}", final_metrics.timeouts);

    println!("\n=== Demo Complete ===");
}
