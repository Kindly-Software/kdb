//! WorkStealingQueue<T> Demo
//!
//! Demonstrates the generic work-stealing queue with:
//! - Producer pushing items
//! - Consumer popping locally (LIFO)
//! - Thief stealing remotely (FIFO)
//! - Custom struct usage

use atomic_capsule::parallel::WorkStealingQueue;
use std::sync::Arc;
use std::thread;

fn main() {
    println!("WorkStealingQueue<T> Demo");
    println!("==========================\n");

    // Create a generic work-stealing queue for u64
    let queue: Arc<WorkStealingQueue<u64>> = Arc::new(WorkStealingQueue::new(1024));
    println!("Created queue with capacity: {}", queue.capacity());

    // Producer: push items
    for i in 0..100 {
        queue.push(i).unwrap();
    }
    println!("Pushed 100 items, queue length: {}", queue.len());

    // Consumer (local LIFO pop)
    let queue_pop = Arc::clone(&queue);
    let pop_thread = thread::spawn(move || {
        let mut items = Vec::new();
        while let Some(item) = queue_pop.pop() {
            items.push(item);
        }
        items
    });

    // Thief (remote FIFO steal)
    let queue_steal = Arc::clone(&queue);
    let steal_thread = thread::spawn(move || {
        let mut items = Vec::new();
        while let Some(item) = queue_steal.steal() {
            items.push(item);
        }
        items
    });

    let popped = pop_thread.join().unwrap();
    let stolen = steal_thread.join().unwrap();

    println!("\nResults:");
    println!("  Popped (LIFO): {} items", popped.len());
    println!("  Stolen (FIFO): {} items", stolen.len());
    println!("  Total: {} items", popped.len() + stolen.len());
    println!("  Queue empty: {}", queue.is_empty());

    // Demonstrate with custom struct
    #[derive(Debug)]
    struct Task {
        id: usize,
        data: String,
    }

    let task_queue = WorkStealingQueue::new(16);

    for i in 0..5 {
        task_queue
            .push(Task {
                id: i,
                data: format!("Task {}", i),
            })
            .unwrap();
    }

    println!("\nCustom struct demo:");
    while let Some(task) = task_queue.pop() {
        println!("  Processed: {:?}", task);
    }

    println!("\n✓ WorkStealingQueue<T> demonstration complete!");
}
