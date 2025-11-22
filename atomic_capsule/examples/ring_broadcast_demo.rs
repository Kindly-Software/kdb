//! Ring Buffer Broadcast Demo
//!
//! Demonstrates lossless multi-consumer broadcast channel.

use atomic_capsule::collections::channel;

fn main() {
    // Create broadcast channel
    let (tx, mut rx1) = channel();
    let mut rx2 = tx.subscribe();
    let mut rx3 = tx.subscribe();

    println!("=== Ring Buffer Broadcast Demo ===\n");

    // Send 10 messages
    println!("Sending 10 messages...");
    for i in 0..10 {
        tx.send(i).unwrap();
        println!("  Sent: {}", i);
    }

    println!("\nReceiving on rx1:");
    for i in 0..10 {
        let msg = rx1.recv().unwrap();
        println!("  rx1 received: {}", msg);
        assert_eq!(msg, i);
    }

    println!("\nReceiving on rx2:");
    for i in 0..10 {
        let msg = rx2.recv().unwrap();
        println!("  rx2 received: {}", msg);
        assert_eq!(msg, i);
    }

    println!("\nReceiving on rx3:");
    for i in 0..10 {
        let msg = rx3.recv().unwrap();
        println!("  rx3 received: {}", msg);
        assert_eq!(msg, i);
    }

    println!("\n✅ All receivers got all messages (lossless broadcast)");
    println!("Active receivers: {}", tx.receiver_count());
}
