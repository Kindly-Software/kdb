//! # WorkStealingQueueCapsule Demonstration
//!
//! This example demonstrates the Chase-Lev work-stealing queue in action
//! with a realistic scenario: distributing document batches across worker threads.

use kindly_dedup::parallel::{WorkStealingQueueCapsule, WorkItem, QueueStats};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║   WorkStealingQueueCapsule - Chase-Lev Demo              ║");
    println!("║   Tier: T1 (Atomic) + T4 (Batch)                         ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    // Scenario: Processing document batches for deduplication
    // - 1 owner thread produces batches
    // - 4 worker threads steal and process batches
    // - Queue capacity: 1024 items

    demo_basic_operations()?;
    println!();
    demo_single_owner_single_thief()?;
    println!();
    demo_multi_worker_load_balancing()?;
    println!();
    demo_statistics_and_metrics()?;

    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║   All demonstrations completed successfully!              ║");
    println!("╚══════════════════════════════════════════════════════════╝");

    Ok(())
}

/// Demonstrate basic push/pop/steal operations
fn demo_basic_operations() -> Result<(), Box<dyn std::error::Error>> {
    println!("DEMO 1: Basic Operations (Push/Pop/Steal)");
    println!("─────────────────────────────────────────");

    let mut queue = WorkStealingQueueCapsule::new(16)?;

    // Owner: push items (LIFO from bottom)
    println!("Owner: Pushing 3 items...");
    for i in 1..=3 {
        let mut item = WorkItem::new(i, 10);
        item.batch.push((100 + i, Arc::from(format!("doc_{}", i).as_str())));
        queue.push(item)?;
        println!("  ✓ Pushed batch {}", i);
    }

    println!("Queue state: {} items", queue.len());
    println!();

    // Owner: pop items (LIFO order: 3, 2, 1)
    println!("Owner: Popping items (LIFO order)...");
    while let Some(item) = queue.pop() {
        println!("  ✓ Popped batch {} (doc_count={})", item.batch_id, item.batch.len());
    }

    println!("Queue state: {} items (empty)", queue.len());
    Ok(())
}

/// Demonstrate owner-thief cooperation
fn demo_single_owner_single_thief() -> Result<(), Box<dyn std::error::Error>> {
    println!("DEMO 2: Owner-Thief Cooperation");
    println!("───────────────────────────────");

    let queue = Arc::new(WorkStealingQueueCapsule::new(128)?);

    // Owner thread: produce items
    let queue_owner = Arc::clone(&queue);
    let owner_handle = thread::spawn(move || {
        let queue_mut = unsafe { &mut *(Arc::as_ptr(&queue_owner) as *mut _) };
        for i in 1..=10 {
            let mut item = WorkItem::new(i, 100);
            // Simulate document batch
            for j in 0..10 {
                let doc_id = i * 100 + j;
                let text = format!("document_{}", doc_id);
                item.batch.push((doc_id, Arc::from(text)));
            }
            queue_mut.push(item).ok();
            thread::sleep(std::time::Duration::from_millis(10));
            println!("  Owner: Produced batch {}", i);
        }
    });

    thread::sleep(std::time::Duration::from_millis(5));

    // Thief thread: consume items
    let queue_thief = Arc::clone(&queue);
    let thief_handle = thread::spawn(move || {
        let mut stolen_count = 0;
        for _ in 0..20 {
            if let Some(item) = queue_thief.steal() {
                println!("  Thief: Stole batch {} ({} docs)", item.batch_id, item.batch.len());
                stolen_count += 1;
            }
            thread::sleep(std::time::Duration::from_millis(5));
        }
        stolen_count
    });

    owner_handle.join().unwrap();
    let stolen_count = thief_handle.join().unwrap();

    println!("Result: {} batches stolen by thief thread", stolen_count);
    Ok(())
}

/// Demonstrate multi-worker load balancing
fn demo_multi_worker_load_balancing() -> Result<(), Box<dyn std::error::Error>> {
    println!("DEMO 3: Multi-Worker Load Balancing");
    println!("──────────────────────────────────");

    let queue = Arc::new(WorkStealingQueueCapsule::new(1024)?);
    let work_done = Arc::new(AtomicUsize::new(0));

    // Owner: produce batches
    let queue_owner = Arc::clone(&queue);
    let owner_start = Instant::now();
    let owner_handle = thread::spawn(move || {
        let queue_mut = unsafe { &mut *(Arc::as_ptr(&queue_owner) as *mut _) };
        for batch_id in 1..=100 {
            let mut item = WorkItem::new(batch_id, 1000);
            // Simulate batch with 1000 documents
            for doc_id in 0..1000 {
                let text = format!("batch_{}_doc_{}", batch_id, doc_id);
                item.batch.push((doc_id, Arc::from(text)));
            }
            queue_mut.push(item).ok();
        }
    });

    thread::sleep(std::time::Duration::from_millis(10));

    // Thief threads: steal and process batches
    let mut thief_handles = vec![];
    for worker_id in 1..=4 {
        let queue_thief = Arc::clone(&queue);
        let work_done_clone = Arc::clone(&work_done);

        let handle = thread::spawn(move || {
            let mut count = 0;
            loop {
                if let Some(item) = queue_thief.steal() {
                    // Simulate processing (count documents)
                    count += item.batch.len();
                    work_done_clone.fetch_add(item.batch.len(), Ordering::Relaxed);
                } else {
                    thread::yield_now();
                    // Check if we should exit (approximate, in real code use barrier)
                    if count > 100 {
                        break;
                    }
                }
            }
            count
        });
        thief_handles.push((worker_id, handle));
    }

    owner_handle.join().unwrap();
    println!("Owner: Produced 100 batches with 1000 docs each");

    let mut total_processed = 0;
    for (worker_id, handle) in thief_handles {
        let count = handle.join().unwrap();
        total_processed += count;
        println!("Worker {}: Processed {} documents", worker_id, count);
    }

    let elapsed = owner_start.elapsed();
    println!(
        "\nTotal: {} documents processed in {:?}",
        total_processed, elapsed
    );
    println!(
        "Rate: {:.0} docs/sec",
        total_processed as f64 / elapsed.as_secs_f64()
    );

    Ok(())
}

/// Demonstrate statistics and metrics
fn demo_statistics_and_metrics() -> Result<(), Box<dyn std::error::Error>> {
    println!("DEMO 4: Statistics and Metrics");
    println!("──────────────────────────────");

    let queue = Arc::new(WorkStealingQueueCapsule::new(256)?);

    // Owner: produce items
    let queue_owner = Arc::clone(&queue);
    let owner_handle = thread::spawn(move || {
        let queue_mut = unsafe { &mut *(Arc::as_ptr(&queue_owner) as *mut _) };
        for i in 1..=50 {
            let item = WorkItem::new(i, 10);
            queue_mut.push(item).ok();
        }
    });

    // Thief: steal items
    let queue_thief = Arc::clone(&queue);
    let thief_handle = thread::spawn(move || {
        for _ in 0..100 {
            queue_thief.steal();
            thread::yield_now();
        }
    });

    owner_handle.join().unwrap();
    thief_handle.join().unwrap();

    // Collect statistics
    let stats = queue.stats();
    print_statistics(&stats);

    Ok(())
}

/// Pretty-print queue statistics
fn print_statistics(stats: &QueueStats) {
    println!("Queue Statistics:");
    println!("  Pushes:         {}", stats.pushes);
    println!("  Pops:           {}", stats.pops);
    println!("  Steals:         {}", stats.steals);
    println!("  Steal attempts: {}", stats.steal_attempts);
    println!("  Empty steals:   {}", stats.empty_steals);
    println!();
    println!("Derived Metrics:");
    println!("  Items in queue: {}", stats.net_work());
    println!(
        "  Steal success rate: {:.1}%",
        stats.steal_success_rate()
    );
}
