// Minimal test to diagnose ThreadPool hang

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// Path-based import
use atomic_capsule::parallel::{ThreadPool, ParallelError};

fn main() {
    println!("=== ThreadPool Hang Diagnosis ===");

    // Test with increasing contention
    for num_threads in [4, 8, 12, 16] {
        println!("\n--- Testing {} threads × 100 tasks ({} total) ---",
            num_threads, num_threads * 100);

        let pool = Arc::new(ThreadPool::new(8).unwrap());
        let executed = Arc::new(AtomicUsize::new(0));
        let spawned = Arc::new(AtomicUsize::new(0));

        let start = std::time::Instant::now();

        // Spawn threads to submit tasks
        let mut handles = vec![];
        for thread_id in 0..num_threads {
            let p = Arc::clone(&pool);
            let e = Arc::clone(&executed);
            let s = Arc::clone(&spawned);

            handles.push(thread::spawn(move || {
                println!("  Thread {} starting...", thread_id);
                for task_id in 0..100 {
                    let e_task = Arc::clone(&e);

                    // Try to push task
                    let task = Box::new(move || {
                        e_task.fetch_add(1, Ordering::Relaxed);
                    });

                    match pool.push(task) {
                        Ok(_) => {
                            s.fetch_add(1, Ordering::Relaxed);
                        },
                        Err(ParallelError::QueueFull) => {
                            // Queue full - expected under high load
                        },
                        Err(e) => {
                            eprintln!("  Thread {}: Unexpected error on task {}: {:?}",
                                thread_id, task_id, e);
                        }
                    }
                }
                println!("  Thread {} finished spawning", thread_id);
            }));
        }

        // Wait for all submitter threads
        for h in handles {
            h.join().unwrap();
        }

        let spawn_count = spawned.load(Ordering::Acquire);
        println!("  Spawned {} tasks total", spawn_count);
        println!("  Pending tasks: {}", pool.pending_tasks());
        println!("  Global counter: {}", pool.global_tasks.load(Ordering::Acquire));

        // Wait for execution with timeout
        println!("  Waiting for completion...");
        let wait_start = std::time::Instant::now();
        let timeout = Duration::from_secs(10);

        while pool.global_tasks.load(Ordering::Acquire) > 0 {
            if wait_start.elapsed() > timeout {
                println!("  ❌ HANG DETECTED!");
                println!("     Pending: {}", pool.global_tasks.load(Ordering::Acquire));
                println!("     Queue length: {}", pool.pending_tasks());
                println!("     Executed: {}", executed.load(Ordering::Acquire));
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }

        let elapsed = start.elapsed();
        let exec_count = executed.load(Ordering::Acquire);

        if pool.global_tasks.load(Ordering::Acquire) == 0 {
            println!("  ✅ Completed in {:?}", elapsed);
            println!("     Executed: {}/{} tasks", exec_count, spawn_count);
        }
    }
}
