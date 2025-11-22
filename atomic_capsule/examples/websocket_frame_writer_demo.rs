//! WebSocket Frame Writer Demo
//!
//! Demonstrates RFC 6455 frame serialization

use atomic_capsule::runtime::websocket::WebSocketFrameWriterCapsule;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let writer = WebSocketFrameWriterCapsule::new();
    let mut buffer = vec![0u8; 4096];

    println!("=== WebSocket Frame Writer Demo ===\n");

    // 1. Text frame
    println!("1. Text Frame:");
    let text = "Hello, WebSocket!";
    let bytes = writer.write_text_frame(text, true, &mut buffer)?;
    println!("   Payload: '{}'", text);
    println!("   Encoded: {} bytes", bytes);
    println!("   Header: [{:02x}, {:02x}]", buffer[0], buffer[1]);
    println!("   FIN={}, Opcode=0x{:x}", buffer[0] >> 7, buffer[0] & 0x0f);
    println!();

    // 2. Binary frame
    println!("2. Binary Frame:");
    let data = b"\x00\x01\x02\x03\x04";
    let bytes = writer.write_binary_frame(data, true, &mut buffer)?;
    println!("   Payload: {} bytes", data.len());
    println!("   Encoded: {} bytes", bytes);
    println!("   Header: [{:02x}, {:02x}]", buffer[0], buffer[1]);
    println!("   FIN={}, Opcode=0x{:x}", buffer[0] >> 7, buffer[0] & 0x0f);
    println!();

    // 3. Ping frame
    println!("3. Ping Frame:");
    let ping_data = b"ping";
    let bytes = writer.write_ping_frame(ping_data, &mut buffer)?;
    println!("   Payload: '{}'", String::from_utf8_lossy(ping_data));
    println!("   Encoded: {} bytes", bytes);
    println!("   Header: [{:02x}, {:02x}]", buffer[0], buffer[1]);
    println!("   FIN={}, Opcode=0x{:x} (Ping)", buffer[0] >> 7, buffer[0] & 0x0f);
    println!();

    // 4. Pong frame
    println!("4. Pong Frame:");
    let pong_data = b"ping";
    let bytes = writer.write_pong_frame(pong_data, &mut buffer)?;
    println!("   Payload: '{}'", String::from_utf8_lossy(pong_data));
    println!("   Encoded: {} bytes", bytes);
    println!("   Header: [{:02x}, {:02x}]", buffer[0], buffer[1]);
    println!("   FIN={}, Opcode=0x{:x} (Pong)", buffer[0] >> 7, buffer[0] & 0x0f);
    println!();

    // 5. Close frame
    println!("5. Close Frame:");
    let bytes = writer.write_close_frame(1000, Some("Normal closure"), &mut buffer)?;
    println!("   Code: 1000 (Normal closure)");
    println!("   Reason: 'Normal closure'");
    println!("   Encoded: {} bytes", bytes);
    println!("   Header: [{:02x}, {:02x}]", buffer[0], buffer[1]);
    let code = u16::from_be_bytes([buffer[2], buffer[3]]);
    println!("   Close code: {}", code);
    println!();

    // 6. Payload length encoding tests
    println!("6. Payload Length Encoding:");

    // 7-bit length
    let text_125 = "x".repeat(125);
    let bytes = writer.write_text_frame(&text_125, true, &mut buffer)?;
    println!("   125 bytes: header size = {} (7-bit encoding)", bytes - 125);

    // 16-bit length
    let text_1000 = "x".repeat(1000);
    let bytes = writer.write_text_frame(&text_1000, true, &mut buffer)?;
    println!("   1000 bytes: header size = {} (16-bit encoding)", bytes - 1000);

    // 64-bit length
    let text_70000 = "x".repeat(70000);
    let bytes = writer.write_text_frame(&text_70000, true, &mut buffer)?;
    println!("   70000 bytes: header size = {} (64-bit encoding)", bytes - 70000);
    println!();

    // 7. Continuation frames
    println!("7. Continuation Frames:");
    let bytes1 = writer.write_text_frame("Part 1", false, &mut buffer)?;
    println!("   Frame 1 (incomplete): {} bytes, FIN={}", bytes1, buffer[0] >> 7);

    let bytes2 = writer.write_continuation_frame(b"Part 2", true, &mut buffer)?;
    println!("   Frame 2 (final): {} bytes, FIN={}", bytes2, buffer[bytes1]);
    println!();

    // 8. Statistics
    println!("8. Statistics:");
    let stats = writer.stats();
    println!("   Frames written: {}", stats.frame_count);
    println!("   Bytes written: {}", stats.bytes_written);
    println!("   Errors: {}", stats.error_count);
    println!();

    // 9. Error handling
    println!("9. Error Handling:");
    let mut small_buffer = vec![0u8; 2];
    match writer.write_text_frame("Too large for small buffer", true, &mut small_buffer) {
        Ok(_) => println!("   Unexpected success"),
        Err(e) => println!("   Error (expected): {}", e),
    }

    // Control frame size validation
    let large_ping = vec![0u8; 126];
    match writer.write_ping_frame(&large_ping, &mut buffer) {
        Ok(_) => println!("   Unexpected success"),
        Err(e) => println!("   Error (expected): {}", e),
    }
    println!();

    // 10. Close code validation
    println!("10. Close Code Validation:");
    match writer.write_close_frame(999, None, &mut buffer) {
        Ok(_) => println!("   Unexpected success for code 999"),
        Err(e) => println!("   Error (expected): {}", e),
    }

    match writer.write_close_frame(1000, None, &mut buffer) {
        Ok(bytes) => println!("   Valid code 1000: {} bytes", bytes),
        Err(e) => println!("   Unexpected error: {}", e),
    }
    println!();

    println!("=== Demo Complete ===");

    Ok(())
}
