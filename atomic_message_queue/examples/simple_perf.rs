use atomic_message_queue::SPSCQueue;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

fn main() {
    println!("Atomic Message Queue Performance Demo");
    println!("=====================================");

    // Single-threaded performance
    single_threaded_test();

    // Concurrent SPSC performance
    concurrent_spsc_test();

    // Batch operations
    batch_operations_test();
}

fn single_threaded_test() {
    println!("\n1. Single-threaded Performance:");

    let queue = SPSCQueue::<u64, 1024>::new();
    const NUM_OPS: usize = 1_000_000;

    // Push performance
    let start = Instant::now();
    for i in 0..NUM_OPS {
        loop {
            match queue.push(i as u64) {
                Ok(()) => break,
                Err(_) => {
                    // Queue full, pop one item and continue
                    let _ = queue.pop();
                }
            }
        }
    }
    let push_duration = start.elapsed();

    // Clear queue first
    while queue.pop().is_ok() {}

    // Fill queue for pop test
    for i in 0..NUM_OPS {
        if queue.push(i as u64).is_err() {
            break;
        }
    }

    // Pop performance
    let start = Instant::now();
    let mut popped = 0;
    while popped < NUM_OPS {
        match queue.pop() {
            Ok(_) => popped += 1,
            Err(_) => {
                // Queue empty, add more items
                for i in 0..1000 {
                    if queue.push(i).is_err() {
                        break;
                    }
                }
            }
        }
    }
    let pop_duration = start.elapsed();

    println!("  Push: {:.1}ns per op ({:.1}M ops/sec)",
        push_duration.as_nanos() as f64 / NUM_OPS as f64,
        NUM_OPS as f64 / push_duration.as_secs_f64() / 1_000_000.0
    );
    println!("  Pop:  {:.1}ns per op ({:.1}M ops/sec)",
        pop_duration.as_nanos() as f64 / NUM_OPS as f64,
        NUM_OPS as f64 / pop_duration.as_secs_f64() / 1_000_000.0
    );
}

fn concurrent_spsc_test() {
    println!("\n2. Concurrent SPSC Performance:");

    let queue = Arc::new(SPSCQueue::<u64, 4096>::new());
    let producer_queue = Arc::clone(&queue);
    let consumer_queue = Arc::clone(&queue);

    const NUM_ITEMS: u64 = 10_000_000;

    let start = Instant::now();

    // Producer thread
    let producer = thread::spawn(move || {
        for i in 0..NUM_ITEMS {
            loop {
                match producer_queue.push(i) {
                    Ok(()) => break,
                    Err(_) => {
                        thread::yield_now();
                        continue;
                    }
                }
            }
        }
    });

    // Consumer thread
    let consumer = thread::spawn(move || {
        let mut received = 0;

        while received < NUM_ITEMS {
            match consumer_queue.pop() {
                Ok(_) => received += 1,
                Err(_) => {
                    thread::yield_now();
                    continue;
                }
            }
        }

        received
    });

    producer.join().unwrap();
    let received = consumer.join().unwrap();
    let duration = start.elapsed();

    assert_eq!(received, NUM_ITEMS);

    let throughput = NUM_ITEMS as f64 / duration.as_secs_f64();
    let latency_per_op = duration.as_nanos() as f64 / NUM_ITEMS as f64;

    println!("  Throughput: {:.1}M ops/sec", throughput / 1_000_000.0);
    println!("  Latency:    {:.1}ns per operation", latency_per_op);
    println!("  Total time: {:?} for {} operations", duration, NUM_ITEMS);
}

fn batch_operations_test() {
    use atomic_message_queue::MessageBatch;

    println!("\n3. Batch Operations:");

    let queue = SPSCQueue::<u64, 2048>::new();
    let batch_sizes = [1, 4, 16, 64];

    for &batch_size in &batch_sizes {
        let mut batch = MessageBatch::new(batch_size);

        const NUM_BATCHES: usize = 100_000;
        let start = Instant::now();

        for batch_num in 0..NUM_BATCHES {
            // Fill batch
            batch.clear();
            for i in 0..batch_size {
                batch.add((batch_num * batch_size + i) as u64);
            }

            // Push batch to queue
            while !batch.is_empty() {
                batch.push_to_queue(&queue);
            }

            // Pop batch from queue
            batch.pop_from_queue(&queue);
        }

        let duration = start.elapsed();
        let total_items = NUM_BATCHES * batch_size;
        let items_per_sec = total_items as f64 / duration.as_secs_f64();

        println!("  Batch size {}: {:.1}M items/sec",
            batch_size,
            items_per_sec / 1_000_000.0
        );
    }
}