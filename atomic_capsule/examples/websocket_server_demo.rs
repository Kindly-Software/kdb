//! WebSocket Server Demo - RFC 6455 Compliant with Atomic Capsule Foundation
//!
//! Demonstrates the full WebSocketServerCapsule orchestrating:
//! - T1 (Atomic): Connection coordination
//! - T4 (Batch): Broadcasting to multiple subscribers
//! - T5 (Streaming): Message assembly from frames
//! - T8 (Network): Socket management
//!
//! Framework: UCE34 (Q10 T8+T1+T4+T5, Q33 verification, Q34 compliance)

use atomic_capsule::websocket::{
    WebSocketServerCapsule, ServerState, ServerError,
};
use std::sync::Arc;
use std::time::Instant;

/// Example: Basic server creation and state management
fn example_basic_server() -> Result<(), ServerError> {
    println!("\n=== Example 1: Basic Server Creation ===");

    // Create server: 127.0.0.1:8080, max 10K connections
    let server = WebSocketServerCapsule::new("127.0.0.1:8080", 10000)?;

    println!("✓ Server created");
    println!("  Bind address: {}", server.bind_addr());
    println!("  State: {}", server.state());
    println!("  Listener FD: {}", server.listener_fd());

    // Start listening
    server.start()?;
    println!("✓ Server started");
    println!("  State: {}", server.state());

    // Verify state
    assert_eq!(server.state(), ServerState::Listening);

    // Graceful shutdown
    server.stop()?;
    println!("✓ Server stopped");
    println!("  State: {}", server.state());

    Ok(())
}

/// Example: Connection acceptance and management
fn example_connections() -> Result<(), ServerError> {
    println!("\n=== Example 2: Connection Management ===");

    let server = WebSocketServerCapsule::new("192.168.1.1:9090", 100)?;
    server.start()?;

    println!("✓ Server started on 192.168.1.1:9090");

    // Accept multiple connections
    for i in 0..5 {
        let conn_id = server.accept()?;
        println!("✓ Accepted connection {}", conn_id);
        assert_eq!(conn_id, i);
    }

    // Check metrics
    let metrics = server.metrics();
    println!("✓ Active connections: {}", metrics.active_connections);
    assert_eq!(metrics.active_connections, 5);

    // Close one connection
    server.close_connection(2, 1000)?;
    println!("✓ Closed connection 2");
    assert_eq!(server.metrics().active_connections, 4);

    server.stop()?;
    Ok(())
}

/// Example: Connection limits
fn example_connection_limits() -> Result<(), ServerError> {
    println!("\n=== Example 3: Connection Limits ===");

    let server = WebSocketServerCapsule::new("127.0.0.1:8081", 3)?;
    server.start()?;

    println!("✓ Server created with max 3 connections");

    // Fill to capacity
    for i in 0..3 {
        let _ = server.accept()?;
        println!("✓ Accepted connection {}/3", i + 1);
    }

    // Fourth should fail
    match server.accept() {
        Err(ServerError::MaxConnectionsReached) => {
            println!("✓ Connection 4 rejected (max reached)");
        }
        _ => panic!("Expected MaxConnectionsReached error"),
    }

    server.stop()?;
    Ok(())
}

/// Example: WebSocket upgrade handshake
fn example_upgrade_handshake() -> Result<(), ServerError> {
    println!("\n=== Example 4: WebSocket Upgrade Handshake ===");

    let server = WebSocketServerCapsule::new("127.0.0.1:8082", 1000)?;
    server.start()?;

    // Accept connection
    let conn_id = server.accept()?;
    println!("✓ Accepted connection {}", conn_id);

    // Simulate HTTP upgrade request
    let http_request = "GET /chat HTTP/1.1\r\n\
                       Host: 127.0.0.1:8082\r\n\
                       Upgrade: websocket\r\n\
                       Connection: Upgrade\r\n\
                       Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                       Sec-WebSocket-Version: 13\r\n\r\n";

    let response = server.on_upgrade(conn_id, http_request)?;
    println!("✓ Upgrade successful");
    println!("  Response status: {}",
             if response.contains("101") { "101 Switching Protocols" } else { "Unknown" });

    assert!(response.contains("101"));
    assert!(response.contains("Upgrade: websocket"));

    server.stop()?;
    Ok(())
}

/// Example: Frame parsing and message assembly
fn example_frame_processing() -> Result<(), ServerError> {
    println!("\n=== Example 5: Frame Processing ===");

    let server = WebSocketServerCapsule::new("127.0.0.1:8083", 1000)?;
    server.start()?;

    let conn_id = server.accept()?;
    println!("✓ Accepted connection");

    // Simulate WebSocket frame (simplified, RFC 6455 compliant)
    // Frame format: [FIN|RSV|Opcode] [MASK|Len] [Extended Length] [Masking Key] [Payload]
    let frame_data = b"\x81\x05Hello";  // Text frame with "Hello" payload

    server.on_frame(conn_id, frame_data)?;
    println!("✓ Frame processed");

    let metrics = server.metrics();
    println!("  Bytes received: {}", metrics.bytes_received);
    // Frame is 7 bytes total: 2-byte header (\x81\x05) + 5-byte payload (Hello)
    assert_eq!(metrics.bytes_received, 7);

    server.stop()?;
    Ok(())
}

/// Example: Message processing
fn example_message_handling() -> Result<(), ServerError> {
    println!("\n=== Example 6: Message Handling ===");

    let server = WebSocketServerCapsule::new("127.0.0.1:8084", 1000)?;
    server.start()?;

    let conn_id = server.accept()?;
    println!("✓ Accepted connection");

    // Process multiple messages
    for i in 1..=3 {
        let message = format!("Message {}", i);
        server.on_message(conn_id, &message)?;
        println!("✓ Processed message: {}", message);
    }

    let metrics = server.metrics();
    println!("  Total messages received: {}", metrics.messages_received);
    assert_eq!(metrics.messages_received, 3);

    server.stop()?;
    Ok(())
}

/// Example: Broadcasting to multiple connections
fn example_broadcasting() -> Result<(), ServerError> {
    println!("\n=== Example 7: Broadcasting ===");

    let server = WebSocketServerCapsule::new("127.0.0.1:8085", 1000)?;
    server.start()?;

    // Add 10 connections
    println!("✓ Adding 10 connections...");
    for _i in 0..10 {
        server.accept()?;
    }

    println!("✓ Active connections: {}", server.metrics().active_connections);

    // Broadcast message
    let broadcast_msg = "Hello everyone!";
    let count = server.broadcast(broadcast_msg)?;
    println!("✓ Broadcast sent to {} connections", count);
    assert_eq!(count, 10);

    let metrics = server.metrics();
    println!("  Total messages sent: {}", metrics.messages_sent);
    assert_eq!(metrics.messages_sent, 10);

    server.stop()?;
    Ok(())
}

/// Example: Metrics collection
fn example_metrics() -> Result<(), ServerError> {
    println!("\n=== Example 8: Metrics ===");

    let server = WebSocketServerCapsule::new("127.0.0.1:8086", 1000)?;
    server.start()?;

    // Simulate activity
    server.accept()?;
    server.accept()?;
    server.on_message(0, "Test message 1")?;
    server.on_message(1, "Test message 2")?;
    server.on_frame(0, b"\x81\x04TEST")?;
    server.broadcast("Broadcast message")?;

    // Get metrics
    let metrics = server.metrics();
    println!("✓ Server Metrics:");
    println!("  Active connections: {}", metrics.active_connections);
    println!("  Max connections: {}", metrics.max_connections);
    println!("  Messages sent: {}", metrics.messages_sent);
    println!("  Messages received: {}", metrics.messages_received);
    println!("  Bytes sent: {}", metrics.bytes_sent);
    println!("  Bytes received: {}", metrics.bytes_received);

    server.stop()?;
    Ok(())
}

/// Example: Multiple servers (different ports)
fn example_multiple_servers() -> Result<(), ServerError> {
    println!("\n=== Example 9: Multiple Servers ===");

    let server1 = WebSocketServerCapsule::new("127.0.0.1:9001", 100)?;
    let server2 = WebSocketServerCapsule::new("127.0.0.1:9002", 100)?;
    let server3 = WebSocketServerCapsule::new("127.0.0.1:9003", 100)?;

    server1.start()?;
    server2.start()?;
    server3.start()?;

    println!("✓ Started 3 servers on ports 9001, 9002, 9003");

    // Accept connections on each
    server1.accept()?;
    server2.accept()?;
    server3.accept()?;

    println!("✓ Each server has 1 connection");
    println!("  Server 1: {} connections", server1.metrics().active_connections);
    println!("  Server 2: {} connections", server2.metrics().active_connections);
    println!("  Server 3: {} connections", server3.metrics().active_connections);

    server1.stop()?;
    server2.stop()?;
    server3.stop()?;

    Ok(())
}

/// Example: Performance measurement
fn example_performance() -> Result<(), ServerError> {
    println!("\n=== Example 10: Performance Measurement ===");

    let server = Arc::new(WebSocketServerCapsule::new("127.0.0.1:8087", 10000)?);
    server.start()?;

    // Measure accept latency
    let start = Instant::now();
    for _i in 0..1000 {
        let _ = server.accept();
    }
    let accept_time = start.elapsed().as_micros();
    println!("✓ 1000 accept() calls: {} μs (avg {:.2} μs/op)",
             accept_time, accept_time as f64 / 1000.0);

    // Measure message latency
    let start = Instant::now();
    for i in 0..1000 {
        let _ = server.on_message(i % 100, &format!("Message {}", i));
    }
    let message_time = start.elapsed().as_micros();
    println!("✓ 1000 on_message() calls: {} μs (avg {:.2} μs/op)",
             message_time, message_time as f64 / 1000.0);

    // Verify broadcast target: <5ms per 1K connections
    let start = Instant::now();
    let _ = server.broadcast("Performance test");
    let broadcast_time = start.elapsed().as_micros();
    println!("✓ Broadcast to {} connections: {:.2} μs",
             server.metrics().active_connections, broadcast_time as f64);

    server.stop()?;
    Ok(())
}

/// Example: Concurrent operations (T8 + T1 lockfree coordination)
fn example_concurrent_operations() -> Result<(), ServerError> {
    println!("\n=== Example 11: Concurrent Operations ===");

    let server = Arc::new(WebSocketServerCapsule::new("127.0.0.1:8088", 10000)?);
    server.start()?;

    // Spawn concurrent threads
    let mut handles = vec![];

    // Thread 1: Accept connections
    let s1 = Arc::clone(&server);
    let h1 = std::thread::spawn(move || {
        for i in 0..5 {
            let _ = s1.accept();
            if i % 100 == 0 {
                println!("  [Accept] Accepted connections");
            }
        }
    });
    handles.push(h1);

    // Thread 2: Send messages
    let s2 = Arc::clone(&server);
    let h2 = std::thread::spawn(move || {
        for i in 0..10 {
            let _ = s2.on_message(i % 5, "Test message");
            if i % 100 == 0 {
                println!("  [Message] Processed messages");
            }
        }
    });
    handles.push(h2);

    // Thread 3: Broadcast
    let s3 = Arc::clone(&server);
    let h3 = std::thread::spawn(move || {
        for i in 0..3 {
            let _ = s3.broadcast(&format!("Broadcast {}", i));
            if i % 100 == 0 {
                println!("  [Broadcast] Sent broadcasts");
            }
        }
    });
    handles.push(h3);

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    println!("✓ All concurrent operations completed");
    println!("  Final metrics: {} connections, {} messages sent",
             server.metrics().active_connections,
             server.metrics().messages_sent);

    server.stop()?;
    Ok(())
}

/// Example: Error handling
fn example_error_handling() -> Result<(), ServerError> {
    println!("\n=== Example 12: Error Handling ===");

    // Empty address error
    match WebSocketServerCapsule::new("", 1000) {
        Err(ServerError::InvalidAddress) => println!("✓ Empty address rejected"),
        _ => panic!("Expected InvalidAddress error"),
    }

    // Server not running error
    let server = WebSocketServerCapsule::new("127.0.0.1:8089", 1000)?;
    match server.accept() {
        Err(ServerError::ServerNotRunning) => println!("✓ Accept before start rejected"),
        _ => panic!("Expected ServerNotRunning error"),
    }

    // Max connections error
    let server = WebSocketServerCapsule::new("127.0.0.1:8090", 2)?;
    server.start()?;
    server.accept()?;
    server.accept()?;
    match server.accept() {
        Err(ServerError::MaxConnectionsReached) => println!("✓ Max connections enforced"),
        _ => panic!("Expected MaxConnectionsReached error"),
    }

    server.stop()?;
    Ok(())
}

fn main() {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║  WebSocket Server Capsule Demo (RFC 6455 Compliant)           ║");
    println!("║  Tier: T8 (Network) + T1 (Atomic) + T4 (Batch) + T5 (Stream)  ║");
    println!("║  Framework: UCE34, Chaos, ASSUM, B32, T28, I20                 ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");

    let examples: Vec<(&str, fn() -> Result<(), ServerError>)> = vec![
        ("Basic Server", example_basic_server),
        ("Connections", example_connections),
        ("Connection Limits", example_connection_limits),
        ("Upgrade Handshake", example_upgrade_handshake),
        ("Frame Processing", example_frame_processing),
        ("Message Handling", example_message_handling),
        ("Broadcasting", example_broadcasting),
        ("Metrics", example_metrics),
        ("Multiple Servers", example_multiple_servers),
        ("Performance", example_performance),
        ("Concurrent Ops", example_concurrent_operations),
        ("Error Handling", example_error_handling),
    ];

    let mut passed = 0;
    let mut failed = 0;

    for (name, example) in examples {
        match example() {
            Ok(_) => {
                passed += 1;
            }
            Err(e) => {
                println!("✗ {} failed: {}", name, e);
                failed += 1;
            }
        }
    }

    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║  Examples Complete: {} passed, {} failed                       ║", passed, failed);
    println!("╚═══════════════════════════════════════════════════════════════╝");
}
