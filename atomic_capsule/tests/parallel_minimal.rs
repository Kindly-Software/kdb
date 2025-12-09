//! Minimal reproduction test for parallel module segfault
//!
//! UCE-D7 Debugging: Isolate the crash to specific operation

use atomic_capsule::parallel::ThreadPool;

#[test]
fn test_minimal_single_task() {
    // Create pool
    let pool = ThreadPool::new(1).unwrap();

    // Push single task
    pool.push(Box::new(|| {
        println!("Task executed");
    }))
    .unwrap();

    // Wait for completion
    pool.wait();

    println!("Test completed successfully");
}

#[test]
fn test_minimal_no_wait() {
    // Create pool
    let pool = ThreadPool::new(1).unwrap();

    // Push single task but DON'T wait
    pool.push(Box::new(|| {
        println!("Task executed");
    }))
    .unwrap();

    // Drop pool without waiting
    drop(pool);

    println!("Test completed successfully");
}

#[test]
fn test_minimal_empty_pool() {
    // Create pool
    let pool = ThreadPool::new(1).unwrap();

    // Wait with no tasks
    pool.wait();

    println!("Test completed successfully");
}
