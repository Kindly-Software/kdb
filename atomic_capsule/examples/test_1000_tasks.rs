use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// Minimal reproduction: 1000 tasks on 4-worker pool
fn main() {
    // Import ThreadPool from atomic_capsule
    use atomic_capsule::parallel::ThreadPool;

    let pool = ThreadPool::new(4).unwrap();
    let counter = Arc::new(AtomicUsize::new(0));
    let num_tasks = 1000;

    println!("Pushing {} tasks to 4-worker pool...", num_tasks);

    for i in 0..num_tasks {
        let c = Arc::clone(&counter);
        match pool.push(Box::new(move || {
            c.fetch_add(1, Ordering::Relaxed);
        })) {
            Ok(_) => {}
            Err(e) => {
                println!("Task {} failed to push: {:?}", i, e);
                break;
            }
        }
    }

    println!("Waiting for all tasks to complete...");

    // Add debug output during wait
    let start = std::time::Instant::now();
    let mut last_print = start;
    loop {
        let pending = pool.pending_tasks();
        let now = std::time::Instant::now();

        if pending == 0 {
            println!("All tasks complete after {:?}", now - start);
            break;
        }

        if now - last_print > std::time::Duration::from_millis(100) {
            println!("Still waiting... pending: {}", pending);
            last_print = now;
        }

        if now - start > std::time::Duration::from_secs(5) {
            println!("TIMEOUT after 5s, pending: {}", pending);
            break;
        }

        std::thread::yield_now();
    }

    let final_count = counter.load(Ordering::Acquire);
    println!("Final count: {} (expected {})", final_count, num_tasks);

    if final_count == num_tasks {
        println!("SUCCESS: All tasks executed");
    } else {
        println!("FAILURE: Missing {} tasks", num_tasks - final_count);
    }
}
