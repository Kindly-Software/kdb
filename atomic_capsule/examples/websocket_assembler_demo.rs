//! WebSocketMessageAssemblerCapsule Demo - RFC 6455 Fragment Reassembly
//!
//! This example demonstrates the T5 Streaming WebSocketMessageAssemblerCapsule,
//! showing how to reassemble fragmented WebSocket messages.
//!
//! # RFC 6455 Message Fragmentation
//!
//! WebSocket allows splitting messages into multiple frames:
//! ```text
//! Frame 1: FIN=0, opcode=0x1 (text), payload="Hello"
//! Frame 2: FIN=0, opcode=0x0 (continuation), payload=" World"
//! Frame 3: FIN=1, opcode=0x0 (continuation), payload="!"
//! Result: "Hello World!"
//! ```

// Use our message_assembler module types directly
use atomic_capsule::websocket::message_assembler::{
    WebSocketMessageAssemblerCapsule, Frame, AssemblyError,
};

fn main() -> Result<(), AssemblyError> {
    println!("=== WebSocketMessageAssemblerCapsule Demo ===\n");

    // Example 1: Single-frame text message
    println!("Example 1: Single-frame text message");
    let (capsule, mut buffer) = WebSocketMessageAssemblerCapsule::new(1024)?;
    let frame = Frame::new(0x1, true, b"Hello, WebSocket!".to_vec());

    let result = capsule.add_fragment(&mut buffer, frame)?;
    println!("  Result: {:?}", result);

    if let Ok(msg) = capsule.assemble(&buffer) {
        println!("  Message type: {:?}", msg.msg_type);
        println!("  Payload: {}\n", String::from_utf8_lossy(&msg.payload));
    }

    // Example 2: Multi-frame message assembly
    println!("Example 2: Multi-frame text message");
    let (capsule, mut buffer) = WebSocketMessageAssemblerCapsule::new(1024)?;

    // Frame 1: Text, incomplete
    let frame1 = Frame::new(0x1, false, b"Hello".to_vec());
    let result = capsule.add_fragment(&mut buffer, frame1)?;
    println!("  Frame 1: {:?}", result);

    // Frame 2: Continuation, incomplete
    let frame2 = Frame::new(0x0, false, b" ".to_vec());
    let result = capsule.add_fragment(&mut buffer, frame2)?;
    println!("  Frame 2: {:?}", result);

    // Frame 3: Continuation, final
    let frame3 = Frame::new(0x0, true, b"World!".to_vec());
    let result = capsule.add_fragment(&mut buffer, frame3)?;
    println!("  Frame 3: {:?}", result);

    if let Ok(msg) = capsule.assemble(&buffer) {
        println!("  Message type: {:?}", msg.msg_type);
        println!("  Payload: {}\n", String::from_utf8_lossy(&msg.payload));
    }

    // Example 3: Binary message
    println!("Example 3: Binary message");
    let (capsule, mut buffer) = WebSocketMessageAssemblerCapsule::new(1024)?;
    let binary_data = vec![0u8, 1, 2, 3, 4, 5, 255];
    let frame = Frame::new(0x2, true, binary_data);

    let result = capsule.add_fragment(&mut buffer, frame)?;
    println!("  Result: {:?}", result);

    if let Ok(msg) = capsule.assemble(&buffer) {
        println!("  Message type: {:?}", msg.msg_type);
        println!("  Payload size: {} bytes", msg.payload.len());
        println!("  Payload: {:?}\n", msg.payload);
    }

    // Example 4: Metrics tracking
    println!("Example 4: Metrics tracking");
    let (mut capsule, mut buffer) = WebSocketMessageAssemblerCapsule::new(1024)?;

    let initial_metrics = capsule.metrics();
    println!("  Initial metrics: {:?}", initial_metrics);

    // Add a complete message
    let frame = Frame::new(0x1, true, b"Test".to_vec());
    capsule.add_fragment(&mut buffer, frame)?;
    capsule.reset();

    let metrics = capsule.metrics();
    println!("  After reset: {:?}", metrics);
    println!("  Messages assembled: {}", metrics.messages_assembled);
    println!("  Errors: {}\n", metrics.errors);

    // Example 5: UTF-8 validation
    println!("Example 5: UTF-8 validation");
    let (capsule, mut buffer) = WebSocketMessageAssemblerCapsule::new(1024)?;
    let unicode_text = "Hello, 世界 🌍".as_bytes().to_vec();
    let frame = Frame::new(0x1, true, unicode_text);

    let result = capsule.add_fragment(&mut buffer, frame)?;
    println!("  Result: {:?}", result);

    match capsule.assemble(&buffer) {
        Ok(msg) => {
            println!("  Message type: {:?}", msg.msg_type);
            println!("  UTF-8 validation: OK");
            println!("  Payload: {}\n", String::from_utf8_lossy(&msg.payload));
        }
        Err(e) => {
            println!("  Error: {:?}\n", e);
        }
    }

    // Example 6: Capsule properties
    println!("Example 6: Capsule properties");
    println!("  Capsule size: {} bytes", std::mem::size_of::<WebSocketMessageAssemblerCapsule>());
    println!("  Expected: 256 bytes (cache-aligned)");
    println!("  Status: {}",
        if std::mem::size_of::<WebSocketMessageAssemblerCapsule>() == 256 { "✓ PASS" } else { "✗ FAIL" }
    );

    Ok(())
}
