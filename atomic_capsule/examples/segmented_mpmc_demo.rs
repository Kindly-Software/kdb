
//! SegmentedMPMC Demonstration
//!
//! Simple example showing SegmentedMPMC usage and performance.

use atomic_capsule::parallel::SegmentedMPMC;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Instant;

fn main() {
    println!("=== SegmentedMPMC Demonstration ===\n");

    // Test 1: Basic functionality
    test_basic();

    // Test 2: Multi-threaded contention
    test_contention();

    // Test 3: Performance benchmark
    test_performance();
}

fn test_basic() {
    println!("Test 1: Basic Push/Pop");
    println!("--------------------");

    let mpmc = SegmentedMPMC::new(4);
    let counter = Arc::new(AtomicUsize::new(0));

    // Push 20 tasks
    for i in 0..20 {
        let c = counter.clone();
        mpmc.push(Box::new(move || {
            c.fetch_add(1, Ordering::Relaxed);
            println!("  Executed task {}", i);
        }))
        .expect("push failed");
    }

    println!("Pushed 20 tasks");

    // Pop and execute all
    let mut count = 0;
    while let Some(task) = mpmc.pop() {
        task();
        count += 1;
    }

    println!("Popped and executed {} tasks", count);
    println!("Counter: {}\n", counter.load(Ordering::Relaxed));
}

fn test_contention() {
    println!("Test 2: Multi-Threaded Contention");
    println!("---------------------------------");

    let mpmc = Arc::new(SegmentedMPMC::new(8));

    // Spawn 8 producer threads, each pushing 100 tasks
    let handles: Vec<_> = (0..8)
        .map(|thread_id| {
            let mpmc = mpmc.clone();
            thread::spawn(move || {
                for i in 0..100 {
                    let _ = mpmc.push(Box::new(move || {
                        // Simulate work
                        let _ = i;
                    }));
                }
                println!("  Thread {} pushed 100 tasks", thread_id);
            })
        })
        .collect();

    // Wait for producers
    for h in handles {
        h.join().unwrap();
    }

    // Pop all tasks
    let mut count = 0;
    while mpmc.pop().is_some() {
        count += 1;
    }

    let stats = mpmc.stats();
    println!("Total tasks: {}", count);
    println!("Segments: {}", stats.segment_count);
    println!("Fallback rate: {:.1}%\n", stats.fallback_rate * 100.0);
}

fn test_performance() {
    println!("Test 3: Performance Benchmark (1,600 tasks)");
    println!("-------------------------------------------");

    let mpmc = Arc::new(SegmentedMPMC::new(16));

    // Measure push time
    let start = Instant::now();

    // 16 threads, 100 tasks each = 1,600 tasks
    let handles: Vec<_> = (0..16)
        .map(|_| {
            let mpmc = mpmc.clone();
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = mpmc.push(Box::new(|| {}));
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let elapsed = start.elapsed();

    // Pop all
    let mut count = 0;
    while mpmc.pop().is_some() {
        count += 1;
    }

    println!("Pushed and popped {} tasks", count);
    println!("Time: {:.2}μs", elapsed.as_micros());
    println!("Rate: {:.1}M tasks/sec", count as f64 / elapsed.as_secs_f64() / 1_000_000.0);

    let stats = mpmc.stats();
    println!("Segment balance: {:.2}", stats.segment_balance);
    println!("Per-segment stats:");
    for seg in &stats.per_segment {
        if seg.push_count > 0 {
            println!(
                "  Segment {}: {} pushes",
                seg.segment_id, seg.push_count
            );
        }
    }

    println!("\n✅ All tests completed!");
}
