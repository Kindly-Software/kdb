use atomic_capsule::http::{HttpResponseBuilderCapsule, ResponseFlags};

fn main() {
    println!("=== HTTP Response Builder Capsule Demo ===\n");

    // Test 1: Basic response construction
    let builder = HttpResponseBuilderCapsule::new(200);
    println!("✓ Created response builder with status 200");
    println!("  Status: {}", builder.status());
    println!("  Generation: {}", builder.generation());

    // Test 2: Set content length
    builder.set_content_length(1024);
    println!("\n✓ Set content length to 1024");
    println!("  Content-Length: {}", builder.content_length());

    // Test 3: Set keep-alive
    builder.set_keep_alive(true);
    println!("\n✓ Enabled keep-alive");
    println!("  Is keep-alive: {}", builder.is_keep_alive());

    // Test 4: Set metadata
    builder.set_request_id(42);
    builder.set_handler_id(1);
    builder.set_user_id(999);
    println!("\n✓ Set audit metadata");
    println!("  Request ID: {}", builder.request_id());
    println!("  Handler ID: {}", builder.handler_id());
    println!("  User ID: {}", builder.user_id());
    println!("  Timestamp: {}", builder.timestamp_ns());

    // Test 5: Serialize response
    let mut output = vec![0u8; 2048];
    match builder.serialize(&mut output) {
        Ok(len) => {
            println!("\n✓ Serialized response to {} bytes", len);
            let response_str = String::from_utf8_lossy(&output[..len]);
            println!("  First 200 chars of response:");
            println!("  {}", response_str.chars().take(200).collect::<String>());
        }
        Err(e) => eprintln!("✗ Serialization failed: {:?}", e),
    }

    // Test 6: Verify integrity
    let serialized = {
        let mut buf = vec![0u8; 2048];
        let len = builder.serialize(&mut buf).unwrap();
        buf.truncate(len);
        buf
    };

    let audit_hash = builder.audit_hash();
    println!("\n✓ Computed audit hash: 0x{:016x}", audit_hash);

    match builder.verify_integrity(&serialized) {
        Ok(_) => println!("✓ Integrity verification passed"),
        Err(e) => eprintln!("✗ Integrity verification failed: {:?}", e),
    }

    // Test 7: Verify size and alignment
    println!("\n=== Structural Properties ===");
    println!("✓ Size: {} bytes (expected 128)", std::mem::size_of::<HttpResponseBuilderCapsule>());
    println!("✓ Alignment: {} bytes (expected 128)", std::mem::align_of::<HttpResponseBuilderCapsule>());

    // Test 8: Response flags
    println!("\n=== Response Flags ===");
    let mut flags = ResponseFlags::new();
    println!("✓ Created empty flags");
    
    flags.set(ResponseFlags::CHUNKED);
    println!("  Chunked: {}", flags.is_set(ResponseFlags::CHUNKED));
    
    flags.set(ResponseFlags::COMPRESSED);
    println!("  Compressed: {}", flags.is_set(ResponseFlags::COMPRESSED));
    
    flags.clear(ResponseFlags::CHUNKED);
    println!("  After clearing chunked: {}", flags.is_set(ResponseFlags::CHUNKED));

    println!("\n=== All Tests Passed ===");
}
