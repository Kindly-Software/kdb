//! Comprehensive Tests for Atomic Network Gateway
//!
//! Following T42 Framework for comprehensive testing:
//! - Connection establishment under various conditions
//! - Message routing with packet loss simulation
//! - Failover scenarios and recovery testing
//! - Multi-threaded stress testing
//! - Performance validation with realistic loads

use atomic_network_gateway::{AtomicNetworkGateway, ConnectionState};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};

/// T42 Test 1: Connection Establishment
#[test]
fn test_connection_establishment() {
    let gateway = AtomicNetworkGateway::new(
        1,
        "primary:8080".to_string(),
        vec!["failover1:8080".to_string(), "failover2:8080".to_string()],
    ).unwrap();

    // Initial state should be disconnected
    assert_eq!(gateway.get_connection_state(), ConnectionState::Disconnected);

    // Connect to primary
    gateway.connect().unwrap();
    assert_eq!(gateway.get_connection_state(), ConnectionState::Connected);
    assert_eq!(gateway.get_active_endpoint(), "primary:8080");

    // Connecting again should be idempotent
    gateway.connect().unwrap();
    assert_eq!(gateway.get_connection_state(), ConnectionState::Connected);
}

/// T42 Test 2: Connection Establishment with Invalid Configuration
#[test]
fn test_connection_invalid_config() {
    // Test empty primary endpoint
    let result = AtomicNetworkGateway::new(
        1,
        "".to_string(),
        vec![],
    );
    assert!(result.is_err());
}

/// T42 Test 3: Message Routing Success Cases
#[test]
fn test_message_routing_success() {
    let gateway = AtomicNetworkGateway::new(
        1,
        "primary:8080".to_string(),
        vec![],
    ).unwrap();

    gateway.connect().unwrap();

    // Send valid messages
    gateway.send_message(b"test message 1").unwrap();
    gateway.send_message(b"test message 2").unwrap();

    let (sent, _, _, _, _) = gateway.get_stats();
    assert_eq!(sent, 2);
}

/// T42 Test 4: Message Routing Failure Cases
#[test]
fn test_message_routing_failures() {
    let gateway = AtomicNetworkGateway::new(
        1,
        "primary:8080".to_string(),
        vec![],
    ).unwrap();

    // Try to send without connection
    let result = gateway.send_message(b"test");
    assert!(result.is_err());

    // Connect and try to send empty message
    gateway.connect().unwrap();
    let result = gateway.send_message(b"");
    assert!(result.is_err());

    // Valid message should work
    gateway.send_message(b"valid").unwrap();
}

/// T42 Test 5: Failover Mechanism - Success
#[test]
fn test_failover_success() {
    let gateway = AtomicNetworkGateway::new(
        1,
        "primary:8080".to_string(),
        vec!["failover1:8080".to_string(), "failover2:8080".to_string()],
    ).unwrap();

    // Test failover (should succeed and switch to failover1)
    gateway.failover().unwrap();
    assert_eq!(gateway.get_connection_state(), ConnectionState::Connected);
    assert_eq!(gateway.get_active_endpoint(), "failover1:8080");

    let (_, _, _, _, failures) = gateway.get_stats();
    assert_eq!(failures, 1);
}

/// T42 Test 6: Failover Mechanism - No Failovers Available
#[test]
fn test_failover_no_failovers() {
    let gateway = AtomicNetworkGateway::new(
        1,
        "primary:8080".to_string(),
        vec![], // No failover endpoints
    ).unwrap();

    // Failover should fail when no endpoints available
    let result = gateway.failover();
    assert!(result.is_err());
}

/// T42 Test 7: Connection State Tracking
#[test]
fn test_connection_state_tracking() {
    let gateway = AtomicNetworkGateway::new(
        1,
        "primary:8080".to_string(),
        vec!["failover:8080".to_string()],
    ).unwrap();

    // Test state transitions
    assert_eq!(gateway.get_connection_state(), ConnectionState::Disconnected);

    gateway.connect().unwrap();
    assert_eq!(gateway.get_connection_state(), ConnectionState::Connected);

    gateway.shutdown();
    assert_eq!(gateway.get_connection_state(), ConnectionState::Disconnected);
    assert!(gateway.is_shutdown());
}

/// T42 Test 8: Statistics Tracking
#[test]
fn test_statistics_tracking() {
    let gateway = AtomicNetworkGateway::new(
        1,
        "primary:8080".to_string(),
        vec![],
    ).unwrap();

    gateway.connect().unwrap();

    // Send messages and track statistics
    for i in 0..10 {
        gateway.send_message(format!("message {}", i).as_bytes()).unwrap();
    }

    // Simulate receiving messages
    for _ in 0..5 {
        gateway.simulate_receive_message();
    }

    let (sent, received, connection_time, heartbeat, failures) = gateway.get_stats();
    assert_eq!(sent, 10);
    assert_eq!(received, 5);
    assert!(connection_time > 0); // Should have connection time recorded
    assert!(heartbeat > 0); // Should have heartbeat timestamp
    assert_eq!(failures, 0); // No failures yet
}

/// T42 Test 9: Multi-threaded Connection Stress Test
#[test]
fn test_multithreaded_connection_stress() {
    let gateway = Arc::new(AtomicNetworkGateway::new(
        1,
        "primary:8080".to_string(),
        vec!["failover:8080".to_string()],
    ).unwrap());

    let num_threads = 10;
    let operations_per_thread = 100;
    let barrier = Arc::new(Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads).map(|thread_id| {
        let gateway = Arc::clone(&gateway);
        let barrier = Arc::clone(&barrier);

        thread::spawn(move || {
            barrier.wait(); // Synchronize start

            for i in 0..operations_per_thread {
                // Each thread attempts various operations
                match thread_id % 4 {
                    0 => {
                        // Connect/disconnect cycle
                        let _ = gateway.connect();
                        if i % 10 == 0 {
                            gateway.shutdown();
                        }
                    }
                    1 => {
                        // Send messages
                        if gateway.get_connection_state() == ConnectionState::Connected {
                            let _ = gateway.send_message(
                                format!("thread {} msg {}", thread_id, i).as_bytes()
                            );
                        }
                    }
                    2 => {
                        // Simulate receive
                        gateway.simulate_receive_message();
                    }
                    3 => {
                        // Check statistics
                        let _ = gateway.get_stats();
                        let _ = gateway.get_active_endpoint();
                    }
                    _ => unreachable!(),
                }

                // Small delay to increase contention
                thread::sleep(Duration::from_nanos(1000));
            }
        })
    }).collect();

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify gateway is still functional
    gateway.connect().unwrap();
    gateway.send_message(b"final test").unwrap();
}

/// T42 Test 10: Message Throughput Under Load
#[test]
fn test_message_throughput() {
    let gateway = Arc::new(AtomicNetworkGateway::new(
        1,
        "primary:8080".to_string(),
        vec![],
    ).unwrap());

    gateway.connect().unwrap();

    let num_threads = 4;
    let messages_per_thread = 10000;
    let start_time = Instant::now();
    let total_messages = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..num_threads).map(|thread_id| {
        let gateway = Arc::clone(&gateway);
        let total_messages = Arc::clone(&total_messages);

        thread::spawn(move || {
            let mut local_count = 0;

            for i in 0..messages_per_thread {
                let message = format!("thread {} message {}", thread_id, i);

                loop {
                    match gateway.send_message(message.as_bytes()) {
                        Ok(()) => {
                            local_count += 1;
                            break;
                        }
                        Err(_) => {
                            // Retry on failure (simulates network recovery)
                            thread::sleep(Duration::from_nanos(100));
                        }
                    }
                }
            }

            total_messages.fetch_add(local_count, Ordering::Relaxed);
        })
    }).collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start_time.elapsed();
    let total_sent = total_messages.load(Ordering::Relaxed);
    let throughput = total_sent as f64 / elapsed.as_secs_f64();

    println!("Throughput test results:");
    println!("  Total messages: {}", total_sent);
    println!("  Time elapsed: {:?}", elapsed);
    println!("  Throughput: {:.2} messages/second", throughput);

    assert_eq!(total_sent, (num_threads * messages_per_thread) as u64);

    // Verify gateway statistics match
    let (sent, _, _, _, _) = gateway.get_stats();
    assert_eq!(sent, total_sent);
}

/// T42 Test 11: Packet Loss Simulation
#[test]
fn test_packet_loss_simulation() {
    let gateway = AtomicNetworkGateway::new(
        1,
        "primary:8080".to_string(),
        vec!["failover:8080".to_string()],
    ).unwrap();

    gateway.connect().unwrap();

    let mut successful_sends = 0;
    let mut failed_sends = 0;

    // Simulate packet loss by disconnecting randomly
    for i in 0..100 {
        if i % 20 == 0 {
            // Simulate connection failure every 20 messages
            gateway.shutdown();
            failed_sends += 1;
        } else if gateway.get_connection_state() != ConnectionState::Connected {
            // Try to reconnect via failover
            if gateway.failover().is_ok() {
                match gateway.send_message(format!("message {}", i).as_bytes()) {
                    Ok(()) => successful_sends += 1,
                    Err(_) => failed_sends += 1,
                }
            } else {
                failed_sends += 1;
            }
        } else {
            // Normal operation
            match gateway.send_message(format!("message {}", i).as_bytes()) {
                Ok(()) => successful_sends += 1,
                Err(_) => failed_sends += 1,
            }
        }
    }

    println!("Packet loss simulation results:");
    println!("  Successful sends: {}", successful_sends);
    println!("  Failed sends: {}", failed_sends);
    println!("  Success rate: {:.2}%", successful_sends as f64 / 100.0 * 100.0);

    // Should have some successful sends despite simulated failures
    assert!(successful_sends > 0);
}

/// T42 Test 12: Reconnection Logic Stress Test
#[test]
fn test_reconnection_logic() {
    let gateway = AtomicNetworkGateway::new(
        1,
        "primary:8080".to_string(),
        vec!["failover1:8080".to_string(), "failover2:8080".to_string()],
    ).unwrap();

    let reconnection_cycles = 50;

    for cycle in 0..reconnection_cycles {
        // Connect
        gateway.connect().unwrap();
        assert_eq!(gateway.get_connection_state(), ConnectionState::Connected);

        // Send a message to verify connection works
        gateway.send_message(format!("cycle {} message", cycle).as_bytes()).unwrap();

        // Disconnect
        gateway.shutdown();
        assert!(gateway.is_shutdown());

        // Test failover
        if cycle % 3 == 0 {
            // Reset shutdown flag for failover test
            let new_gateway = AtomicNetworkGateway::new(
                cycle,
                "primary:8080".to_string(),
                vec!["failover1:8080".to_string()],
            ).unwrap();

            new_gateway.failover().unwrap();
            assert_eq!(new_gateway.get_connection_state(), ConnectionState::Connected);
            assert_eq!(new_gateway.get_active_endpoint(), "failover1:8080");
        }
    }

    println!("Completed {} reconnection cycles", reconnection_cycles);
}

/// T42 Test 13: Concurrent Failover Operations
#[test]
fn test_concurrent_failover() {
    let gateway = Arc::new(AtomicNetworkGateway::new(
        1,
        "primary:8080".to_string(),
        vec!["failover1:8080".to_string(), "failover2:8080".to_string()],
    ).unwrap());

    let num_threads = 5;
    let operations_per_thread = 20;
    let barrier = Arc::new(Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads).map(|_| {
        let gateway = Arc::clone(&gateway);
        let barrier = Arc::clone(&barrier);

        thread::spawn(move || {
            barrier.wait();

            for _ in 0..operations_per_thread {
                // Attempt failover operations concurrently
                let _ = gateway.failover();
                thread::sleep(Duration::from_millis(1));
            }
        })
    }).collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Gateway should still be functional
    assert_eq!(gateway.get_connection_state(), ConnectionState::Connected);

    // Should be using a failover endpoint
    let active = gateway.get_active_endpoint();
    assert!(active == "failover1:8080" || active == "failover2:8080");
}

/// T42 Test 14: Memory Safety Under Concurrent Access
#[test]
fn test_memory_safety_concurrent() {
    let gateway = Arc::new(AtomicNetworkGateway::new(
        1,
        "primary:8080".to_string(),
        vec!["failover:8080".to_string()],
    ).unwrap());

    let num_readers = 10;
    let num_writers = 5;
    let duration = Duration::from_millis(500);
    let start_time = Instant::now();
    let stop_flag = Arc::new(AtomicBool::new(false));

    // Reader threads (only read operations)
    let reader_handles: Vec<_> = (0..num_readers).map(|_| {
        let gateway = Arc::clone(&gateway);
        let stop_flag = Arc::clone(&stop_flag);

        thread::spawn(move || {
            while !stop_flag.load(Ordering::Relaxed) {
                let _ = gateway.get_connection_state();
                let _ = gateway.get_stats();
                let _ = gateway.get_active_endpoint();
                let _ = gateway.is_shutdown();
                thread::sleep(Duration::from_nanos(100));
            }
        })
    }).collect();

    // Writer threads (modify state)
    let writer_handles: Vec<_> = (0..num_writers).map(|_| {
        let gateway = Arc::clone(&gateway);
        let stop_flag = Arc::clone(&stop_flag);

        thread::spawn(move || {
            while !stop_flag.load(Ordering::Relaxed) {
                let _ = gateway.connect();
                let _ = gateway.send_message(b"test");
                gateway.simulate_receive_message();
                if start_time.elapsed() > duration / 2 {
                    let _ = gateway.failover();
                }
                thread::sleep(Duration::from_micros(10));
            }
        })
    }).collect();

    // Let threads run for specified duration
    thread::sleep(duration);
    stop_flag.store(true, Ordering::Relaxed);

    // Wait for all threads to complete
    for handle in reader_handles {
        handle.join().unwrap();
    }
    for handle in writer_handles {
        handle.join().unwrap();
    }

    // Verify gateway is still in a valid state
    let _ = gateway.get_stats();
    assert!(!gateway.is_shutdown() || gateway.get_connection_state() == ConnectionState::Disconnected);
}

/// Performance benchmark following B32 guidelines
#[test]
fn test_performance_baseline() {
    let gateway = AtomicNetworkGateway::new(
        1,
        "primary:8080".to_string(),
        vec![],
    ).unwrap();

    gateway.connect().unwrap();

    // Warm up
    for _ in 0..1000 {
        gateway.send_message(b"warmup").unwrap();
    }

    // Measure latency of individual operations
    let iterations = 10000;
    let start = Instant::now();

    for i in 0..iterations {
        gateway.send_message(format!("benchmark {}", i).as_bytes()).unwrap();
    }

    let elapsed = start.elapsed();
    let avg_latency = elapsed.as_nanos() / iterations;

    println!("Performance baseline results:");
    println!("  Operations: {}", iterations);
    println!("  Total time: {:?}", elapsed);
    println!("  Average latency: {} ns per operation", avg_latency);
    println!("  Throughput: {:.2} ops/sec", iterations as f64 / elapsed.as_secs_f64());

    // B32 Framework: Verify realistic performance expectations
    // Expected: <1μs per operation for in-memory operations
    assert!(avg_latency < 1000, "Average latency {} ns exceeds 1μs threshold", avg_latency);
}