//! Safety validation tests for ASSUM framework compliance
//!
//! These tests verify the safety assumptions documented in the main implementation:
//! - TOCTOU prevention
//! - Memory ordering correctness
//! - Thread safety validation
//! - Invariant maintenance

use atomic_message_queue::{SPSCQueue, MessageBatch, QueueError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Test ASSUM_TOCTOU_SAFE: Ring buffer prevents ABA through power-of-2 masking
#[test]
fn test_toctou_prevention() {
    let queue = Arc::new(SPSCQueue::<u64, 16>::new());
    let producer_queue = Arc::clone(&queue);
    let consumer_queue = Arc::clone(&queue);
    let verification_counter = Arc::new(AtomicU64::new(0));
    let producer_counter = Arc::clone(&verification_counter);
    let consumer_counter = Arc::clone(&verification_counter);

    const NUM_CYCLES: u64 = 1000;
    const ITEMS_PER_CYCLE: u64 = 32; // More than queue capacity to force wraparound

    // Producer: rapidly fill and signal
    let producer = thread::spawn(move || {
        for cycle in 0..NUM_CYCLES {
            let base = cycle * ITEMS_PER_CYCLE;

            // Fill queue completely
            for i in 0..ITEMS_PER_CYCLE {
                loop {
                    match producer_queue.push(base + i) {
                        Ok(()) => {
                            producer_counter.fetch_add(1, Ordering::Relaxed);
                            break;
                        }
                        Err(QueueError::Full) => {
                            thread::yield_now();
                            continue;
                        }
                        Err(e) => panic!("Unexpected producer error: {:?}", e),
                    }
                }
            }
        }
    });

    // Consumer: rapidly drain
    let consumer = thread::spawn(move || {
        let mut received_count = 0;
        let mut last_value: Option<u64> = None;

        while received_count < NUM_CYCLES * ITEMS_PER_CYCLE {
            match consumer_queue.pop() {
                Ok(value) => {
                    // Verify no ABA issues - values should be monotonic within cycles
                    if let Some(last) = last_value {
                        if value < last && (value % ITEMS_PER_CYCLE) != 0 {
                            panic!("ABA problem detected: got {} after {}", value, last);
                        }
                    }
                    last_value = Some(value);
                    consumer_counter.fetch_add(1, Ordering::Relaxed);
                    received_count += 1;
                }
                Err(QueueError::Empty) => {
                    thread::yield_now();
                    continue;
                }
                Err(e) => panic!("Unexpected consumer error: {:?}", e),
            }
        }

        received_count
    });

    producer.join().unwrap();
    let received = consumer.join().unwrap();

    // Verify all items processed
    assert_eq!(received, NUM_CYCLES * ITEMS_PER_CYCLE);

    // Verify counters match (no lost operations)
    let producer_total = verification_counter.load(Ordering::Relaxed);
    assert_eq!(producer_total, NUM_CYCLES * ITEMS_PER_CYCLE * 2); // Both producer and consumer increment
}

/// Test ASSUM_MEMORY_ORDERING: Acquire/Release synchronization works correctly
#[test]
fn test_memory_ordering_synchronization() {
    #[derive(Debug, Clone, PartialEq)]
    struct TestMessage {
        id: u64,
        data: [u64; 8], // Larger payload to test memory synchronization
    }

    let queue = Arc::new(SPSCQueue::<TestMessage, 64>::new());
    let producer_queue = Arc::clone(&queue);
    let consumer_queue = Arc::clone(&queue);

    const NUM_MESSAGES: u64 = 10000;

    // Producer with complex data
    let producer = thread::spawn(move || {
        for i in 0..NUM_MESSAGES {
            let message = TestMessage {
                id: i,
                data: [i; 8], // All elements should be the same
            };

            loop {
                match producer_queue.push(message.clone()) {
                    Ok(()) => break,
                    Err(QueueError::Full) => {
                        thread::yield_now();
                        continue;
                    }
                    Err(e) => panic!("Producer error: {:?}", e),
                }
            }
        }
    });

    // Consumer verifying data integrity
    let consumer = thread::spawn(move || {
        let mut received = 0;
        let mut corruption_detected = false;

        while received < NUM_MESSAGES {
            match consumer_queue.pop() {
                Ok(message) => {
                    // Verify memory ordering: all data elements should match ID
                    for &data_elem in &message.data {
                        if data_elem != message.id {
                            corruption_detected = true;
                            eprintln!(
                                "Memory ordering violation: message ID {} has data element {}",
                                message.id, data_elem
                            );
                        }
                    }
                    received += 1;
                }
                Err(QueueError::Empty) => {
                    thread::yield_now();
                    continue;
                }
                Err(e) => panic!("Consumer error: {:?}", e),
            }
        }

        (received, corruption_detected)
    });

    producer.join().unwrap();
    let (received, corruption) = consumer.join().unwrap();

    assert_eq!(received, NUM_MESSAGES);
    assert!(!corruption, "Memory ordering violation detected");
}

/// Test ASSUM_SEND_SYNC: Thread safety under extreme contention
#[test]
fn test_thread_safety_stress() {
    let queue = Arc::new(SPSCQueue::<u64, 1024>::new());
    let producer_queue = Arc::clone(&queue);
    let consumer_queue = Arc::clone(&queue);

    const STRESS_DURATION: Duration = Duration::from_millis(1000);
    const EXPECTED_MIN_OPS: u64 = 100000; // Minimum operations expected

    let start_time = Instant::now();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let producer_stop = Arc::clone(&stop_flag);
    let consumer_stop = Arc::clone(&stop_flag);

    let operations_counter = Arc::new(AtomicU64::new(0));
    let producer_ops = Arc::clone(&operations_counter);
    let consumer_ops = Arc::clone(&operations_counter);

    // Aggressive producer
    let producer = thread::spawn(move || {
        let mut value = 0u64;
        let mut successful_pushes = 0u64;

        while !producer_stop.load(Ordering::Relaxed) {
            match producer_queue.push(value) {
                Ok(()) => {
                    value = value.wrapping_add(1);
                    successful_pushes += 1;
                    producer_ops.fetch_add(1, Ordering::Relaxed);
                }
                Err(QueueError::Full) => {
                    // Brief yield to avoid live-lock
                    if successful_pushes.is_multiple_of(1000) {
                        thread::yield_now();
                    }
                }
                Err(e) => panic!("Producer error: {:?}", e),
            }
        }

        successful_pushes
    });

    // Aggressive consumer
    let consumer = thread::spawn(move || {
        let mut last_value: Option<u64> = None;
        let mut successful_pops = 0u64;
        let mut out_of_order = 0u64;

        while !consumer_stop.load(Ordering::Relaxed) {
            match consumer_queue.pop() {
                Ok(value) => {
                    // Check for ordering violations
                    if let Some(last) = last_value {
                        if value != last.wrapping_add(1) {
                            out_of_order += 1;
                        }
                    }
                    last_value = Some(value);
                    successful_pops += 1;
                    consumer_ops.fetch_add(1, Ordering::Relaxed);
                }
                Err(QueueError::Empty) => {
                    // Brief yield to avoid live-lock
                    if successful_pops.is_multiple_of(1000) {
                        thread::yield_now();
                    }
                }
                Err(e) => panic!("Consumer error: {:?}", e),
            }
        }

        (successful_pops, out_of_order)
    });

    // Run stress test
    thread::sleep(STRESS_DURATION);
    stop_flag.store(true, Ordering::Relaxed);

    let producer_pushes = producer.join().unwrap();
    let (consumer_pops, ordering_violations) = consumer.join().unwrap();

    let total_ops = operations_counter.load(Ordering::Relaxed);
    let actual_duration = start_time.elapsed();

    println!(
        "Stress test results: {} total ops in {:?} ({:.0} ops/sec)",
        total_ops,
        actual_duration,
        total_ops as f64 / actual_duration.as_secs_f64()
    );
    println!("Producer pushes: {}, Consumer pops: {}", producer_pushes, consumer_pops);

    // Verify performance and correctness
    assert!(total_ops >= EXPECTED_MIN_OPS, "Performance below threshold");
    assert_eq!(ordering_violations, 0, "Ordering violations detected");
}

/// Test ASSUM_INVARIANT: Queue invariants maintained under all conditions
#[test]
fn test_invariant_maintenance() {
    let queue = SPSCQueue::<u64, 32>::new();

    // Test 1: Capacity invariant
    assert_eq!(queue.capacity(), 32);
    assert!(queue.capacity().is_power_of_two());

    // Test 2: Empty queue invariants
    assert!(queue.is_empty());
    assert!(!queue.is_full());
    assert_eq!(queue.len(), 0);

    // Test 3: Fill queue and verify invariants
    for i in 0..32 {
        assert_eq!(queue.push(i), Ok(()));
        assert_eq!(queue.len(), (i + 1) as usize);
        assert!(!queue.is_empty());
    }

    assert!(queue.is_full());
    assert_eq!(queue.len(), 32);
    assert_eq!(queue.push(999), Err(QueueError::Full));

    // Test 4: Empty queue and verify invariants
    for i in 0..32 {
        assert_eq!(queue.pop(), Ok(i));
        assert_eq!(queue.len(), (31 - i) as usize);
        if i < 31 {
            assert!(!queue.is_empty());
        }
    }

    assert!(queue.is_empty());
    assert!(!queue.is_full());
    assert_eq!(queue.len(), 0);
    assert_eq!(queue.pop(), Err(QueueError::Empty));
}

/// Test ASSUM_METRIC_ATOMIC: Counter accuracy under concurrent access
#[test]
fn test_metric_atomicity() {
    let queue = Arc::new(SPSCQueue::<u64, 256>::new());
    let producer_queue = Arc::clone(&queue);
    let consumer_queue = Arc::clone(&queue);

    const NUM_OPERATIONS: u64 = 50000;
    let producer_counter = Arc::new(AtomicU64::new(0));
    let consumer_counter = Arc::new(AtomicU64::new(0));
    let prod_count = Arc::clone(&producer_counter);
    let cons_count = Arc::clone(&consumer_counter);

    // Producer counting successful operations
    let producer = thread::spawn(move || {
        for i in 0..NUM_OPERATIONS {
            loop {
                match producer_queue.push(i) {
                    Ok(()) => {
                        prod_count.fetch_add(1, Ordering::Relaxed);
                        break;
                    }
                    Err(QueueError::Full) => {
                        thread::yield_now();
                        continue;
                    }
                    Err(e) => panic!("Producer error: {:?}", e),
                }
            }
        }
    });

    // Consumer counting successful operations
    let consumer = thread::spawn(move || {
        let mut received = 0;

        while received < NUM_OPERATIONS {
            match consumer_queue.pop() {
                Ok(_) => {
                    cons_count.fetch_add(1, Ordering::Relaxed);
                    received += 1;
                }
                Err(QueueError::Empty) => {
                    thread::yield_now();
                    continue;
                }
                Err(e) => panic!("Consumer error: {:?}", e),
            }
        }

        received
    });

    producer.join().unwrap();
    let received = consumer.join().unwrap();

    // Verify atomic counters match actual operations
    let producer_count = producer_counter.load(Ordering::Relaxed);
    let consumer_count = consumer_counter.load(Ordering::Relaxed);

    assert_eq!(producer_count, NUM_OPERATIONS);
    assert_eq!(consumer_count, NUM_OPERATIONS);
    assert_eq!(received, NUM_OPERATIONS);

    // Verify queue metrics are consistent
    assert!(queue.is_empty());
    assert_eq!(queue.len(), 0);
}

/// Test message batching efficiency and correctness
#[test]
fn test_batch_operations() {
    let queue = SPSCQueue::<u64, 128>::new();
    let mut batch = MessageBatch::new(16);

    // Test batch filling
    for i in 0..16 {
        assert!(batch.add(i));
    }
    assert!(!batch.add(16)); // Batch should be full

    // Test batch push
    let pushed = batch.push_to_queue(&queue);
    assert_eq!(pushed, 16);
    assert!(batch.is_empty());
    assert_eq!(queue.len(), 16);

    // Test batch pop
    let popped = batch.pop_from_queue(&queue);
    assert_eq!(popped, 16);
    assert_eq!(batch.len(), 16);
    assert!(queue.is_empty());

    // Verify order preservation
    for (i, &item) in batch.items().iter().enumerate() {
        assert_eq!(item, i as u64);
    }
}

/// Test queue behavior with different data types
#[test]
fn test_generic_types() {
    // Test with complex types
    #[derive(Debug, Clone, PartialEq)]
    struct ComplexMessage {
        id: u64,
        timestamp: u64,
        payload: Vec<u8>,
    }

    let queue = SPSCQueue::<ComplexMessage, 16>::new();

    let original_msg = ComplexMessage {
        id: 12345,
        timestamp: 67890,
        payload: vec![1, 2, 3, 4, 5],
    };

    // Test push and pop with complex type
    assert_eq!(queue.push(original_msg.clone()), Ok(()));
    assert_eq!(queue.pop(), Ok(original_msg));

    // Test with Option type
    let opt_queue = SPSCQueue::<Option<u64>, 8>::new();
    assert_eq!(opt_queue.push(Some(42)), Ok(()));
    assert_eq!(opt_queue.push(None), Ok(()));
    assert_eq!(opt_queue.pop(), Ok(Some(42)));
    assert_eq!(opt_queue.pop(), Ok(None));
}

/// Performance regression test
#[test]
fn test_performance_baseline() {
    let queue = Arc::new(SPSCQueue::<u64, 2048>::new());
    let producer_queue = Arc::clone(&queue);
    let consumer_queue = Arc::clone(&queue);

    const NUM_ITEMS: u64 = 1000000;
    let start = Instant::now();

    // High-throughput test
    let producer = thread::spawn(move || {
        for i in 0..NUM_ITEMS {
            loop {
                match producer_queue.push(i) {
                    Ok(()) => break,
                    Err(QueueError::Full) => continue,
                    Err(e) => panic!("Producer error: {:?}", e),
                }
            }
        }
    });

    let consumer = thread::spawn(move || {
        let mut received = 0;

        while received < NUM_ITEMS {
            match consumer_queue.pop() {
                Ok(_) => received += 1,
                Err(QueueError::Empty) => continue,
                Err(e) => panic!("Consumer error: {:?}", e),
            }
        }

        received
    });

    producer.join().unwrap();
    let received = consumer.join().unwrap();
    let duration = start.elapsed();

    assert_eq!(received, NUM_ITEMS);

    let throughput = NUM_ITEMS as f64 / duration.as_secs_f64();
    println!(
        "Performance baseline: {:.0} operations/second ({} items in {:?})",
        throughput, NUM_ITEMS, duration
    );

    // Baseline expectation: at least 10M ops/sec on modern hardware
    // This is conservative - actual performance should be much higher
    assert!(
        throughput >= 10_000_000.0,
        "Performance regression: {:.0} ops/sec < 10M ops/sec baseline",
        throughput
    );
}