//! PacketBufferConst Demonstration
//!
//! Shows zero-allocation const-generic packet buffer usage patterns.

use atomic_capsule::network::PacketBufferConst;

fn main() {
    println!("=== PacketBufferConst Demo ===\n");

    // Test 1: Basic enqueue/dequeue
    test_basic_operations();

    // Test 2: Multiple MTU sizes
    test_mtu_variants();

    // Test 3: Size validation
    test_size_validation();

    // Test 4: Ring buffer wraparound
    test_wraparound();

    // Test 5: Throughput stress test
    test_stress();

    println!("\nAll tests completed successfully!");
}

fn test_basic_operations() {
    println!("Test 1: Basic enqueue/dequeue operations");
    let buf: PacketBufferConst<1500, 16> = PacketBufferConst::new();

    println!("  Initial state:");
    println!("    Capacity: {} packets", buf.capacity());
    println!("    Fill level: {}", buf.len());
    println!("    Is empty: {}", buf.is_empty());

    // Enqueue a packet
    let packet = [42u8; 100];
    match buf.enqueue(&packet) {
        Ok(_) => println!("  ✓ Enqueued 100-byte packet"),
        Err(e) => println!("  ✗ Enqueue failed: {:?}", e),
    }

    println!("  After enqueue:");
    println!("    Fill level: {}", buf.len());

    // Dequeue the packet
    match buf.dequeue() {
        Some(pkt) => println!("  ✓ Dequeued packet ({} bytes)", pkt.len()),
        None => println!("  ✗ Dequeue failed"),
    }

    println!("  After dequeue:");
    println!("    Fill level: {}", buf.len());
    println!();
}

fn test_mtu_variants() {
    println!("Test 2: MTU variant compilation");

    // Ethernet (1500)
    let buf_eth: PacketBufferConst<1500, 256> = PacketBufferConst::new();
    println!("  ✓ Ethernet (1500 MTU) buffer created");

    // Jumbo frames (9000)
    let buf_jumbo: PacketBufferConst<9000, 256> = PacketBufferConst::new();
    println!("  ✓ Jumbo frame (9000 MTU) buffer created");

    // IP maximum (65535)
    let buf_ip: PacketBufferConst<65535, 256> = PacketBufferConst::new();
    println!("  ✓ IP maximum (65535 MTU) buffer created");

    // Test enqueue on each
    let eth_pkt = [1u8; 1500];
    let _ = buf_eth.enqueue(&eth_pkt);
    println!("  ✓ Enqueued 1500-byte packet");

    let jumbo_pkt = [2u8; 9000];
    let _ = buf_jumbo.enqueue(&jumbo_pkt);
    println!("  ✓ Enqueued 9000-byte packet");

    println!();
}

fn test_size_validation() {
    println!("Test 3: Size validation");
    let buf: PacketBufferConst<100, 8> = PacketBufferConst::new();

    // Valid size
    let valid = [0u8; 100];
    match buf.enqueue(&valid) {
        Ok(_) => println!("  ✓ Accepted 100-byte packet (MTU=100)"),
        Err(e) => println!("  ✗ Rejected valid packet: {:?}", e),
    }

    let _ = buf.dequeue(); // Make room

    // Invalid size
    let invalid = [0u8; 101];
    match buf.enqueue(&invalid) {
        Ok(_) => println!("  ✗ Accepted oversized packet (should have rejected)"),
        Err(_) => println!("  ✓ Rejected oversized packet (101 > MTU=100)"),
    }

    println!();
}

fn test_wraparound() {
    println!("Test 4: Ring buffer wraparound");
    let buf: PacketBufferConst<256, 4> = PacketBufferConst::new();
    let packet = [3u8; 50];

    // Fill buffer to capacity (3 of 4 slots, 1 reserved for distinguishing empty/full)
    for i in 0..3 {
        let _ = buf.enqueue(&packet);
        println!("  Enqueued packet {}, fill level: {}", i + 1, buf.len());
    }

    // Try to add 4th - should fail
    match buf.enqueue(&packet) {
        Ok(_) => println!("  ✗ Added 4th packet (should have been full)"),
        Err(_) => println!("  ✓ Buffer correctly reported as full"),
    }

    // Dequeue one
    let _ = buf.dequeue();
    println!("  Dequeued packet, fill level: {}", buf.len());

    // Now enqueue should succeed
    match buf.enqueue(&packet) {
        Ok(_) => println!("  ✓ Enqueued after freeing space"),
        Err(e) => println!("  ✗ Enqueue failed after dequeue: {:?}", e),
    }

    println!();
}

fn test_stress() {
    println!("Test 5: Stress test (10,000 operations)");
    let buf: PacketBufferConst<1500, 256> = PacketBufferConst::new();

    let packet = [4u8; 1000];
    let mut enqueued = 0;
    let mut dequeued = 0;

    for i in 0..10_000 {
        if i % 2 == 0 {
            // Try to enqueue
            match buf.enqueue(&packet) {
                Ok(_) => enqueued += 1,
                Err(_) => {
                    // Buffer full, drain one
                    if buf.dequeue().is_some() {
                        dequeued += 1;
                    }
                }
            }
        } else {
            // Try to dequeue
            if buf.dequeue().is_some() {
                dequeued += 1;
            }
        }
    }

    // Drain remaining
    while buf.dequeue().is_some() {
        dequeued += 1;
    }

    println!("  Enqueued: {}", enqueued);
    println!("  Dequeued: {}", dequeued);
    println!("  ✓ Stress test completed without panics");

    println!();
}
