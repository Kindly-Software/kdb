// Stress test for multi-producer ThreadPool fix
// Tests 30 threads × 100 tasks = 3000 total with 8 workers

use atomic_capsule::parallel::ThreadPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

fn main() {
    println!("=== Multi-Producer Stress Test: 30 threads × 100 tasks ===");

    let pool = Arc::new(ThreadPool::new(8).unwrap());
    let counter = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    for thread_id in 0..30 {
        let p = Arc::clone(&pool);
        let c = Arc::clone(&counter);

        handles.push(thread::spawn(move || {
            println!("  Thread {} spawning tasks...", thread_id);

            for _ in 0..100 {
                // Retry on QueueFull (queue capacity = 2048, 30×100=3000 may overflow)
                loop {
                    let c_task = Arc::clone(&c);
                    match p.push(Box::new(move || {
                        c_task.fetch_add(1, Ordering::Relaxed);
                    })) {
                        Ok(_) => break,
                        Err(_) => {
                            // Queue full, wait briefly and retry
                            thread::sleep(std::time::Duration::from_micros(10));
                        }
                    }
                }
            }
            println!("  Thread {} done", thread_id);
        }));
    }

    println!("=== Waiting for submitters ===");
    for h in handles {
        h.join().unwrap();
    }

    println!("=== All tasks submitted, waiting for execution ===");
    pool.wait();

    let executed = counter.load(Ordering::Acquire);
    let expected = 30 * 100;

    println!("=== Results ===");
    println!("  Expected: {} tasks", expected);
    println!("  Executed: {} tasks", executed);
    println!("  Lost:     {} tasks", expected - executed);

    if executed == expected {
        println!("✅ PASS: No task loss!");
        std::process::exit(0);
    } else {
        println!("❌ FAIL: {} tasks lost", expected - executed);
        std::process::exit(1);
    }
}
