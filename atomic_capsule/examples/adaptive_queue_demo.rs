//! Adaptive Work Queue Demo
//!
//! Demonstrates runtime-configurable capacity scaling for universal CPU support (8-256 cores).

use atomic_capsule::parallel::AdaptiveWorkQueue;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

fn main() {
    println!("=== Adaptive Work Queue Demo ===\n");

    // Demo 1: Capacity scaling
    demo_capacity_scaling();

    // Demo 2: Performance comparison (8 vs 192 cores)
    demo_performance_scaling();

    // Demo 3: Concurrent push/steal
    demo_concurrent_operations();
}

fn demo_capacity_scaling() {
    println!("1. Capacity Scaling Demo");
    println!("-------------------------");

    let test_cases = vec![
        (8, "Laptop/Desktop (8 cores)"),
        (64, "Server (64 cores)"),
        (192, "EPYC Workstation (192 cores)"),
        (256, "Dual EPYC (256 cores)"),
    ];

    for (cores, description) in test_cases {
        let q = AdaptiveWorkQueue::new(cores);
        println!(
            "{}: {} slots ({} KB deterministic)",
            description,
            q.capacity(),
            q.capacity() * 64 / 1024
        );
    }

    println!();
}

fn demo_performance_scaling() {
    println!("2. Performance Scaling Demo");
    println!("----------------------------");

    // 8-core configuration (1K capacity)
    let q8 = Arc::new(AdaptiveWorkQueue::new(8));
    let counter8 = Arc::new(AtomicUsize::new(0));

    let start = Instant::now();
    for i in 0..500 {
        let c = Arc::clone(&counter8);
        q8.push(Box::new(move || {
            c.fetch_add(i + 1, Ordering::Relaxed);
        }))
        .unwrap();
    }

    // Single-threaded drain
    while let Some(task) = q8.pop() {
        task();
    }
    let elapsed_8 = start.elapsed();

    println!(
        "8-core queue (1K capacity): {} tasks in {:?}",
        counter8.load(Ordering::Acquire),
        elapsed_8
    );

    // 192-core configuration (4K capacity)
    let q192 = Arc::new(AdaptiveWorkQueue::new(192));
    let counter192 = Arc::new(AtomicUsize::new(0));

    let start = Instant::now();
    for i in 0..500 {
        let c = Arc::clone(&counter192);
        q192.push(Box::new(move || {
            c.fetch_add(i + 1, Ordering::Relaxed);
        }))
        .unwrap();
    }

    // Single-threaded drain
    while let Some(task) = q192.pop() {
        task();
    }
    let elapsed_192 = start.elapsed();

    println!(
        "192-core queue (4K capacity): {} tasks in {:?}",
        counter192.load(Ordering::Acquire),
        elapsed_192
    );

    println!(
        "Speedup ratio: {:.2}×\n",
        elapsed_8.as_nanos() as f64 / elapsed_192.as_nanos() as f64
    );
}

fn demo_concurrent_operations() {
    println!("3. Concurrent Push/Steal Demo");
    println!("------------------------------");

    let q = Arc::new(AdaptiveWorkQueue::new(64)); // Server configuration
    let counter = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    // Start timing
    let start = Instant::now();

    // 4 pushers × 100 tasks = 400 total
    for _ in 0..4 {
        let q_clone = Arc::clone(&q);
        let c = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                let mut backoff = 0;
                loop {
                    let c_task = Arc::clone(&c);
                    match q_clone.push(Box::new(move || {
                        c_task.fetch_add(i + 1, Ordering::Relaxed);
                    })) {
                        Ok(_) => break,
                        Err(_) => {
                            backoff = (backoff + 1).min(100);
                            for _ in 0..backoff {
                                std::hint::spin_loop();
                            }
                        }
                    }
                }
            }
        }));
    }

    // 4 stealers, each stealing 100 tasks
    for _ in 0..4 {
        let q_clone = Arc::clone(&q);
        handles.push(thread::spawn(move || {
            let mut stolen = 0;
            while stolen < 100 {
                if let Some(task) = q_clone.steal() {
                    task();
                    stolen += 1;
                } else {
                    thread::yield_now();
                }
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();

    println!(
        "Completed {} tasks with 4 pushers + 4 stealers in {:?}",
        counter.load(Ordering::Acquire),
        elapsed
    );
    println!(
        "Throughput: {:.2} tasks/ms",
        counter.load(Ordering::Acquire) as f64 / elapsed.as_micros() as f64 * 1000.0
    );

    println!("\n=== Demo Complete ===");
}
