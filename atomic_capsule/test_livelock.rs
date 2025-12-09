// Minimal livelock reproduction test
use atomic_capsule::parallel::ThreadPool;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn main() {
    println!("Creating 4-worker pool...");
    let pool = ThreadPool::new(4).unwrap();

    let counter = Arc::new(AtomicUsize::new(0));

    println!("Spawning 4 tasks...");
    for i in 0..4 {
        let c = Arc::clone(&counter);
        let result = pool.push(Box::new(move || {
            println!("Task {} executing", i);
            c.fetch_add(1, Ordering::Relaxed);
        }));
        println!("Task {} spawn result: {:?}", i, result);
    }

    println!("Waiting for tasks to complete...");
    pool.wait();

    println!("Done! Counter: {}", counter.load(Ordering::Relaxed));
}
