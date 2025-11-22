//! ThreadLocalBatchBuffer Demo
//!
//! Demonstrates zero-contention thread-local batch accumulation.

use atomic_capsule::parallel::ThreadLocalBatchBuffer;
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    println!("=== ThreadLocalBatchBuffer Demo ===\n");

    // Test 1: Basic push and flush
    println!("Test 1: Basic push and flush");
    basic_push_flush();

    // Test 2: Auto-flush when buffer full
    println!("\nTest 2: Auto-flush when buffer full");
    auto_flush();

    // Test 3: Concurrent threads
    println!("\nTest 3: Concurrent threads");
    concurrent_threads();

    println!("\n=== All tests passed! ===");
}

fn basic_push_flush() {
    let results = Arc::new(Mutex::new(Vec::new()));
    let results_clone = results.clone();

    let flush_fn = move |batch: &[usize]| {
        println!("  Flushing batch of {} items", batch.len());
        results_clone.lock().unwrap().extend_from_slice(batch);
    };

    let buffer = ThreadLocalBatchBuffer::new(4, flush_fn);

    // Push 3 items (should not flush yet)
    buffer.push(1).unwrap();
    buffer.push(2).unwrap();
    buffer.push(3).unwrap();

    println!("  Buffer length: {}", buffer.len());
    println!("  Results length: {}", results.lock().unwrap().len());

    // Manual flush
    buffer.flush().unwrap();

    println!("  After flush - Buffer length: {}", buffer.len());
    println!("  After flush - Results: {:?}", *results.lock().unwrap());

    assert_eq!(buffer.len(), 0);
    assert_eq!(results.lock().unwrap().as_slice(), &[1, 2, 3]);
    println!("  ✓ Basic push/flush working!");
}

fn auto_flush() {
    let results = Arc::new(Mutex::new(Vec::new()));
    let results_clone = results.clone();

    let flush_fn = move |batch: &[usize]| {
        println!("  Auto-flush triggered! Batch size: {}", batch.len());
        results_clone.lock().unwrap().extend_from_slice(batch);
    };

    let buffer = ThreadLocalBatchBuffer::new(3, flush_fn);

    // Push 3 items (auto-flush at capacity)
    buffer.push(1).unwrap();
    buffer.push(2).unwrap();
    buffer.push(3).unwrap(); // Triggers flush

    println!("  After auto-flush - Buffer length: {}", buffer.len());
    println!("  Results: {:?}", *results.lock().unwrap());

    // Push 2 more items
    buffer.push(4).unwrap();
    buffer.push(5).unwrap();

    println!("  After 2 more pushes - Buffer length: {}", buffer.len());

    // Final flush
    buffer.flush().unwrap();

    println!("  Final results: {:?}", *results.lock().unwrap());

    assert_eq!(results.lock().unwrap().as_slice(), &[1, 2, 3, 4, 5]);
    println!("  ✓ Auto-flush working!");
}

fn concurrent_threads() {
    let results = Arc::new(Mutex::new(Vec::new()));
    let results_clone = results.clone();

    let flush_fn = move |batch: &[usize]| {
        results_clone.lock().unwrap().extend_from_slice(batch);
    };

    let buffer = Arc::new(ThreadLocalBatchBuffer::new(10, flush_fn));

    // Spawn 4 threads, each pushing 100 items
    let mut handles = Vec::new();
    for thread_id in 0..4 {
        let buffer_clone = buffer.clone();
        let handle = thread::spawn(move || {
            println!("  Thread {} starting...", thread_id);
            for i in 0..100 {
                let value = thread_id * 1000 + i;
                buffer_clone.push(value).unwrap();
            }
            buffer_clone.flush().unwrap(); // Flush remaining
            println!("  Thread {} done!", thread_id);
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    println!("  Total items processed: {}", results.lock().unwrap().len());

    assert_eq!(results.lock().unwrap().len(), 400);
    println!("  ✓ Concurrent threads working!");
}
