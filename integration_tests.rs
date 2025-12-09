//! Integration Tests for Network Primitives
//!
//! Tests the interaction between atomic_network_gateway, atomic_multicast_receiver,
//! and atomic_message_queue to validate complete trading system workflows.
//!
//! Following T42 Framework for comprehensive integration testing:
//! - End-to-end market data processing pipeline
//! - Order management system integration
//! - Failover and recovery scenarios
//! - Performance validation under realistic loads
//! - Memory safety under concurrent access patterns

use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};

// Import all network primitives
use atomic_network_gateway::{AtomicNetworkGateway, ConnectionState, NetworkError};
use atomic_multicast_receiver::{MulticastReceiver, MarketPacket};
use atomic_message_queue::{SPSCQueue, QueueError};

/// Market data message flowing through the system
#[derive(Debug, Clone, Copy)]
struct MarketDataMessage {
    symbol_id: u32,
    price: f64,
    volume: u64,
    timestamp_ns: u64,
    sequence: u32,
}

/// Order message for execution
#[derive(Debug, Clone, Copy)]
struct OrderMessage {
    order_id: u64,
    symbol_id: u32,
    side: OrderSide,
    price: f64,
    quantity: u32,
    timestamp_ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum OrderSide {
    Buy = 0,
    Sell = 1,
}

/// Order execution result
#[derive(Debug, Clone, Copy)]
struct ExecutionReport {
    order_id: u64,
    executed_price: f64,
    executed_quantity: u32,
    timestamp_ns: u64,
}

/// Integration Test 1: End-to-End Market Data Pipeline
#[test]
fn test_market_data_pipeline_integration() {
    println!("Testing end-to-end market data pipeline...");

    // Setup components
    let gateway = Arc::new(AtomicNetworkGateway::new(
        1,
        "md-feed:5000".to_string(),
        vec!["md-backup:5000".to_string()],
    ).unwrap());

    let message_queue = SPSCQueue::<MarketDataMessage, 4096>::new();
    let (mut producer, mut consumer) = message_queue.split();

    let packets_processed = Arc::new(AtomicU64::new(0));
    let pipeline_errors = Arc::new(AtomicU64::new(0));
    let stop_flag = Arc::new(AtomicBool::new(false));

    // Connect gateway
    gateway.connect().unwrap();
    assert_eq!(gateway.get_connection_state(), ConnectionState::Connected);

    // Simulate market data processing pipeline
    let gateway_clone = Arc::clone(&gateway);
    let packets_clone = Arc::clone(&packets_processed);
    let errors_clone = Arc::clone(&pipeline_errors);
    let stop_clone = Arc::clone(&stop_flag);

    let processing_thread = thread::spawn(move || {
        let mut sequence = 1u32;

        while !stop_clone.load(Ordering::Relaxed) {
            // Simulate receiving market data packet
            let market_msg = MarketDataMessage {
                symbol_id: 12345,
                price: 100.0 + (sequence as f64 * 0.01),
                volume: 1000 + (sequence as u64 * 10),
                timestamp_ns: Instant::now().elapsed().as_nanos() as u64,
                sequence,
            };

            // Send raw packet data through gateway
            let packet_data = unsafe {
                std::slice::from_raw_parts(
                    &market_msg as *const _ as *const u8,
                    std::mem::size_of::<MarketDataMessage>(),
                )
            };

            match gateway_clone.send_message(packet_data) {
                Ok(()) => {
                    // Process through message queue
                    match producer.try_send(market_msg) {
                        Ok(()) => {
                            packets_clone.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(QueueError::Full) => {
                            errors_clone.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(_) => {
                            errors_clone.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                Err(_) => {
                    errors_clone.fetch_add(1, Ordering::Relaxed);
                }
            }

            sequence += 1;
            thread::sleep(Duration::from_micros(10)); // Simulate 100khz feed
        }
    });

    // Consumer thread
    let consumer_packets = Arc::clone(&packets_processed);
    let consumer_stop = Arc::clone(&stop_flag);
    let consumer_thread = thread::spawn(move || {
        let mut consumed = 0u64;
        let mut last_sequence = 0u32;

        while !consumer_stop.load(Ordering::Relaxed) || consumer.available_messages() > 0 {
            match consumer.try_recv() {
                Ok(msg) => {
                    consumed += 1;

                    // Validate sequence ordering
                    if msg.sequence <= last_sequence && last_sequence > 0 {
                        println!("Warning: Out of order message detected");
                    }
                    last_sequence = msg.sequence;

                    // Simulate processing latency
                    thread::sleep(Duration::from_nanos(100));
                }
                Err(QueueError::Empty) => {
                    thread::sleep(Duration::from_micros(1));
                }
                Err(_) => break,
            }
        }

        consumed
    });

    // Run for 100ms
    thread::sleep(Duration::from_millis(100));
    stop_flag.store(true, Ordering::Release);

    processing_thread.join().unwrap();
    let consumed = consumer_thread.join().unwrap();

    let processed = packets_processed.load(Ordering::Relaxed);
    let errors = pipeline_errors.load(Ordering::Relaxed);

    println!("Pipeline results:");
    println!("  Packets processed: {}", processed);
    println!("  Packets consumed: {}", consumed);
    println!("  Pipeline errors: {}", errors);

    // Validate pipeline performance
    assert!(processed > 1000, "Should process >1000 packets in 100ms");
    assert!(consumed > 0, "Should consume some packets");
    assert_eq!(errors, 0, "Should have no pipeline errors");

    // Validate gateway statistics
    let (sent, _, _, _, _) = gateway.get_stats();
    assert_eq!(sent, processed, "Gateway sent count should match processed");
}

/// Integration Test 2: Order Management System
#[test]
fn test_order_management_integration() {
    println!("Testing order management system integration...");

    // Setup order gateway
    let order_gateway = Arc::new(AtomicNetworkGateway::new(
        2,
        "order-gateway:8080".to_string(),
        vec!["backup-gateway:8080".to_string()],
    ).unwrap());

    // Setup order queues
    let order_queue = SPSCQueue::<OrderMessage, 1024>::new();
    let (mut order_producer, mut order_consumer) = order_queue.split();

    let execution_queue = SPSCQueue::<ExecutionReport, 1024>::new();
    let (mut exec_producer, mut exec_consumer) = execution_queue.split();

    // Connect gateway
    order_gateway.connect().unwrap();

    let orders_sent = Arc::new(AtomicU64::new(0));
    let executions_received = Arc::new(AtomicU64::new(0));
    let order_errors = Arc::new(AtomicU64::new(0));

    // Order sending thread
    let gateway_clone = Arc::clone(&order_gateway);
    let orders_clone = Arc::clone(&orders_sent);
    let errors_clone = Arc::clone(&order_errors);

    let order_thread = thread::spawn(move || {
        for order_id in 1..=100 {
            let order = OrderMessage {
                order_id,
                symbol_id: 12345,
                side: if order_id % 2 == 0 { OrderSide::Buy } else { OrderSide::Sell },
                price: 100.0 + (order_id as f64 * 0.01),
                quantity: 100,
                timestamp_ns: Instant::now().elapsed().as_nanos() as u64,
            };

            // Send order through gateway
            let order_data = unsafe {
                std::slice::from_raw_parts(
                    &order as *const _ as *const u8,
                    std::mem::size_of::<OrderMessage>(),
                )
            };

            match gateway_clone.send_message(order_data) {
                Ok(()) => {
                    // Queue order for processing
                    match order_producer.try_send(order) {
                        Ok(()) => {
                            orders_clone.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(_) => {
                            errors_clone.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                Err(_) => {
                    errors_clone.fetch_add(1, Ordering::Relaxed);
                }
            }

            thread::sleep(Duration::from_millis(1)); // 1000 orders/sec
        }
    });

    // Order processing and execution thread
    let executions_clone = Arc::clone(&executions_received);
    let processing_thread = thread::spawn(move || {
        while let Ok(order) = order_consumer.recv() {
            // Simulate order processing delay
            thread::sleep(Duration::from_micros(100));

            // Generate execution report
            let execution = ExecutionReport {
                order_id: order.order_id,
                executed_price: order.price,
                executed_quantity: order.quantity,
                timestamp_ns: Instant::now().elapsed().as_nanos() as u64,
            };

            // Send execution report
            if exec_producer.try_send(execution).is_ok() {
                executions_clone.fetch_add(1, Ordering::Relaxed);
            }

            // Stop after processing 100 orders
            if order.order_id >= 100 {
                break;
            }
        }
    });

    // Execution report consumer
    let exec_reports_received = Arc::new(AtomicU64::new(0));
    let reports_clone = Arc::clone(&exec_reports_received);
    let execution_thread = thread::spawn(move || {
        for _ in 0..100 {
            if let Ok(_report) = exec_consumer.recv() {
                reports_clone.fetch_add(1, Ordering::Relaxed);
            }
        }
    });

    // Wait for completion
    order_thread.join().unwrap();
    processing_thread.join().unwrap();
    execution_thread.join().unwrap();

    let sent = orders_sent.load(Ordering::Relaxed);
    let executions = executions_received.load(Ordering::Relaxed);
    let reports = exec_reports_received.load(Ordering::Relaxed);
    let errors = order_errors.load(Ordering::Relaxed);

    println!("Order management results:");
    println!("  Orders sent: {}", sent);
    println!("  Executions generated: {}", executions);
    println!("  Reports received: {}", reports);
    println!("  Errors: {}", errors);

    // Validate order processing
    assert_eq!(sent, 100, "Should send 100 orders");
    assert_eq!(executions, 100, "Should generate 100 executions");
    assert_eq!(reports, 100, "Should receive 100 reports");
    assert_eq!(errors, 0, "Should have no errors");
}

/// Integration Test 3: Failover and Recovery
#[test]
fn test_failover_recovery_integration() {
    println!("Testing failover and recovery integration...");

    let gateway = Arc::new(AtomicNetworkGateway::new(
        3,
        "primary:9000".to_string(),
        vec!["failover1:9000".to_string(), "failover2:9000".to_string()],
    ).unwrap());

    let message_queue = SPSCQueue::<u64, 512>::new();
    let (mut producer, mut consumer) = message_queue.split();

    let messages_sent = Arc::new(AtomicU64::new(0));
    let failovers_triggered = Arc::new(AtomicU64::new(0));

    // Connect to primary initially
    gateway.connect().unwrap();
    assert_eq!(gateway.get_active_endpoint(), "primary:9000");

    let gateway_clone = Arc::clone(&gateway);
    let sent_clone = Arc::clone(&messages_sent);
    let failover_clone = Arc::clone(&failovers_triggered);

    let failover_thread = thread::spawn(move || {
        for i in 0..1000 {
            let message_data = (i as u64).to_be_bytes();

            // Simulate connection failure every 200 messages
            if i > 0 && i % 200 == 0 {
                gateway_clone.shutdown();

                // Trigger failover
                if gateway_clone.failover().is_ok() {
                    failover_clone.fetch_add(1, Ordering::Relaxed);
                    println!("Failover {} successful, now using: {}",
                             failover_clone.load(Ordering::Relaxed),
                             gateway_clone.get_active_endpoint());
                }
            }

            // Try to send message
            match gateway_clone.send_message(&message_data) {
                Ok(()) => {
                    // Queue message
                    match producer.try_send(i as u64) {
                        Ok(()) => sent_clone.fetch_add(1, Ordering::Relaxed),
                        Err(QueueError::Full) => {
                            // Consumer is slow, skip this message
                        }
                        Err(_) => break,
                    }
                }
                Err(_) => {
                    // Connection failed, will be handled on next iteration
                }
            }

            thread::sleep(Duration::from_millis(1));
        }
    });

    // Consumer thread
    let messages_received = Arc::new(AtomicU64::new(0));
    let received_clone = Arc::clone(&messages_received);
    let consumer_thread = thread::spawn(move || {
        let mut last_message = 0u64;

        for _ in 0..1000 {
            match consumer.try_recv() {
                Ok(msg) => {
                    received_clone.fetch_add(1, Ordering::Relaxed);

                    // Check for major gaps (indicating failover)
                    if msg > last_message + 50 && last_message > 0 {
                        println!("Large gap detected: {} -> {} (failover occurred)", last_message, msg);
                    }
                    last_message = msg;
                }
                Err(QueueError::Empty) => {
                    thread::sleep(Duration::from_millis(1));
                }
                Err(_) => break,
            }
        }
    });

    failover_thread.join().unwrap();
    consumer_thread.join().unwrap();

    let sent = messages_sent.load(Ordering::Relaxed);
    let received = messages_received.load(Ordering::Relaxed);
    let failovers = failovers_triggered.load(Ordering::Relaxed);

    println!("Failover test results:");
    println!("  Messages sent: {}", sent);
    println!("  Messages received: {}", received);
    println!("  Failovers triggered: {}", failovers);
    println!("  Final endpoint: {}", gateway.get_active_endpoint());

    // Validate failover functionality
    assert!(failovers >= 4, "Should trigger multiple failovers");
    assert!(sent > 500, "Should send most messages despite failovers");
    assert!(received > 0, "Should receive some messages");
    assert_ne!(gateway.get_active_endpoint(), "primary:9000", "Should not be on primary after failovers");
}

/// Integration Test 4: High-Load Concurrent Access
#[test]
fn test_high_load_concurrent_integration() {
    println!("Testing high-load concurrent integration...");

    const NUM_GATEWAYS: usize = 4;
    const NUM_QUEUES: usize = 4;
    const MESSAGES_PER_GATEWAY: usize = 1000;

    let mut gateways = Vec::new();
    let mut queues = Vec::new();

    // Setup multiple gateways and queues
    for i in 0..NUM_GATEWAYS {
        let gateway = Arc::new(AtomicNetworkGateway::new(
            (i + 10) as u64,
            format!("gateway{}:8000", i),
            vec![format!("backup{}:8000", i)],
        ).unwrap());
        gateway.connect().unwrap();
        gateways.push(gateway);
    }

    for _ in 0..NUM_QUEUES {
        let queue = SPSCQueue::<u64, 2048>::new();
        queues.push(Arc::new(queue));
    }

    let barrier = Arc::new(Barrier::new(NUM_GATEWAYS + NUM_QUEUES * 2));
    let total_sent = Arc::new(AtomicU64::new(0));
    let total_received = Arc::new(AtomicU64::new(0));
    let start_time = Instant::now();

    let mut handles = Vec::new();

    // Gateway threads
    for (i, gateway) in gateways.iter().enumerate() {
        let gateway = Arc::clone(gateway);
        let barrier = Arc::clone(&barrier);
        let sent = Arc::clone(&total_sent);

        let handle = thread::spawn(move || {
            barrier.wait();

            for msg_id in 0..MESSAGES_PER_GATEWAY {
                let message_data = ((i * MESSAGES_PER_GATEWAY + msg_id) as u64).to_be_bytes();

                match gateway.send_message(&message_data) {
                    Ok(()) => {
                        sent.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(_) => {
                        // Retry once
                        if gateway.send_message(&message_data).is_ok() {
                            sent.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }

                // High-frequency: 1μs between messages
                if msg_id % 100 == 0 {
                    thread::sleep(Duration::from_micros(1));
                }
            }
        });
        handles.push(handle);
    }

    // Queue producer/consumer threads
    for (i, queue) in queues.iter().enumerate() {
        let queue_clone = Arc::clone(queue);
        let barrier_producer = Arc::clone(&barrier);

        // Producer thread
        let producer_handle = thread::spawn(move || {
            let (mut producer, _) = queue_clone.split();
            barrier_producer.wait();

            for msg_id in 0..MESSAGES_PER_GATEWAY {
                let msg = (i * MESSAGES_PER_GATEWAY + msg_id) as u64;

                while producer.try_send(msg).is_err() {
                    thread::sleep(Duration::from_nanos(100));
                }
            }
        });

        let queue_clone2 = Arc::clone(queue);
        let barrier_consumer = Arc::clone(&barrier);
        let received = Arc::clone(&total_received);

        // Consumer thread
        let consumer_handle = thread::spawn(move || {
            let (_, mut consumer) = queue_clone2.split();
            barrier_consumer.wait();

            for _ in 0..MESSAGES_PER_GATEWAY {
                while consumer.try_recv().is_err() {
                    thread::sleep(Duration::from_nanos(100));
                }
                received.fetch_add(1, Ordering::Relaxed);
            }
        });

        handles.push(producer_handle);
        handles.push(consumer_handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start_time.elapsed();
    let sent = total_sent.load(Ordering::Relaxed);
    let received = total_received.load(Ordering::Relaxed);

    println!("High-load test results:");
    println!("  Total time: {:?}", elapsed);
    println!("  Messages sent: {}", sent);
    println!("  Messages received: {}", received);
    println!("  Gateway throughput: {:.2} msgs/sec", sent as f64 / elapsed.as_secs_f64());
    println!("  Queue throughput: {:.2} msgs/sec", received as f64 / elapsed.as_secs_f64());

    // Validate high-load performance
    assert_eq!(sent, (NUM_GATEWAYS * MESSAGES_PER_GATEWAY) as u64, "Should send all gateway messages");
    assert_eq!(received, (NUM_QUEUES * MESSAGES_PER_GATEWAY) as u64, "Should receive all queue messages");
    assert!(elapsed < Duration::from_secs(5), "Should complete within 5 seconds");

    // Validate all gateways are still functional
    for gateway in &gateways {
        assert_eq!(gateway.get_connection_state(), ConnectionState::Connected);
        let (gateway_sent, _, _, _, _) = gateway.get_stats();
        assert_eq!(gateway_sent, MESSAGES_PER_GATEWAY as u64);
    }
}

/// Integration Test 5: Memory Safety Under Stress
#[test]
fn test_memory_safety_integration() {
    println!("Testing memory safety under stress...");

    let gateway = Arc::new(AtomicNetworkGateway::new(
        5,
        "stress-test:7000".to_string(),
        vec!["backup:7000".to_string()],
    ).unwrap());

    let queue = Arc::new(SPSCQueue::<[u8; 64], 1024>::new());

    gateway.connect().unwrap();

    let stress_duration = Duration::from_millis(500);
    let start_time = Instant::now();
    let stop_flag = Arc::new(AtomicBool::new(false));

    let operations_count = Arc::new(AtomicU64::new(0));
    let mut handles = Vec::new();

    // Multiple stress threads
    for thread_id in 0..8 {
        let gateway_clone = Arc::clone(&gateway);
        let queue_clone = Arc::clone(&queue);
        let stop_clone = Arc::clone(&stop_flag);
        let ops_clone = Arc::clone(&operations_count);

        let handle = thread::spawn(move || {
            let (mut producer, mut consumer) = queue_clone.split();
            let mut local_ops = 0u64;

            while !stop_clone.load(Ordering::Relaxed) {
                // Gateway operations
                let test_data = [thread_id as u8; 32];
                let _ = gateway_clone.send_message(&test_data);
                gateway_clone.simulate_receive_message();

                // Queue operations
                let queue_data = [thread_id as u8; 64];
                if producer.try_send(queue_data).is_ok() {
                    if consumer.try_recv().is_ok() {
                        local_ops += 1;
                    }
                }

                // Check various states
                let _ = gateway_clone.get_connection_state();
                let _ = gateway_clone.get_stats();
                let _ = gateway_clone.get_active_endpoint();

                local_ops += 1;

                // Small delay to allow other threads to work
                if local_ops % 1000 == 0 {
                    thread::sleep(Duration::from_nanos(100));
                }
            }

            ops_clone.fetch_add(local_ops, Ordering::Relaxed);
        });

        handles.push(handle);
    }

    // Let stress test run
    thread::sleep(stress_duration);
    stop_flag.store(true, Ordering::Release);

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start_time.elapsed();
    let total_ops = operations_count.load(Ordering::Relaxed);

    println!("Memory safety stress test results:");
    println!("  Duration: {:?}", elapsed);
    println!("  Total operations: {}", total_ops);
    println!("  Operations/sec: {:.2}", total_ops as f64 / elapsed.as_secs_f64());

    // Validate system is still functional
    assert_eq!(gateway.get_connection_state(), ConnectionState::Connected);
    assert!(!gateway.is_shutdown());

    let (sent, received, _, _, _) = gateway.get_stats();
    assert!(sent > 0, "Should have sent messages during stress test");
    assert!(received > 0, "Should have received messages during stress test");

    println!("  Gateway sent: {}, received: {}", sent, received);
    println!("Memory safety stress test completed successfully!");
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn run_all_integration_tests() {
        println!("=== Running Network Primitives Integration Tests ===\n");

        test_market_data_pipeline_integration();
        println!();

        test_order_management_integration();
        println!();

        test_failover_recovery_integration();
        println!();

        test_high_load_concurrent_integration();
        println!();

        test_memory_safety_integration();

        println!("\n=== All Integration Tests Passed ===");
    }
}