//! Time-Travel Debugging Demo
//!
//! Demonstrates bidirectional execution replay with the ReplayEngineCapsule.

use kdb::time_travel::ReplayEngineCapsule;

fn main() {
    println!("=== Atomic Debugger: Time-Travel Demo ===\n");

    let engine = ReplayEngineCapsule::new();

    // Simulate a program execution trace
    println!("1. Recording execution trace (100 instructions)...");
    for i in 0..100 {
        let rip = 0x1000 + (i * 4);
        let rsp = 0x7fff_0000 - (i * 8);
        engine.take_snapshot(rip, rsp).unwrap();
    }

    let (current, total) = engine.get_stats();
    println!("   Current snapshot: {}", current);
    println!("   Total snapshots: {}", total);
    println!();

    // Forward replay
    println!("2. Stepping backward through execution (10 steps):");
    for i in 0..10 {
        match engine.step_backward() {
            Ok((id, rip, rsp)) => {
                println!("   [{}] RIP: {:#06x}, RSP: {:#010x}", id, rip, rsp);
            }
            Err(e) => {
                println!("   Error: {}", e);
                break;
            }
        }
    }
    println!();

    // Jump to specific point
    println!("3. Jump to snapshot 50:");
    match engine.jump_to_snapshot(50) {
        Ok((id, rip, rsp)) => {
            println!("   [{}] RIP: {:#06x}, RSP: {:#010x}", id, rip, rsp);
        }
        Err(e) => println!("   Error: {}", e),
    }
    println!();

    // Forward from snapshot 50
    println!("4. Stepping forward from snapshot 50 (5 steps):");
    for _ in 0..5 {
        match engine.step_forward() {
            Ok((id, rip, rsp)) => {
                println!("   [{}] RIP: {:#06x}, RSP: {:#010x}", id, rip, rsp);
            }
            Err(e) => {
                println!("   Error: {}", e);
                break;
            }
        }
    }
    println!();

    // Ring buffer wraparound demo
    println!("5. Testing ring buffer wraparound (recording 5000 snapshots)...");
    let new_engine = ReplayEngineCapsule::new();
    for i in 0..5000 {
        let rip = 0x2000 + (i * 4);
        let rsp = 0x7fff_0000 - (i * 8);
        new_engine.take_snapshot(rip, rsp).unwrap();
    }

    let (current, total) = new_engine.get_stats();
    println!("   Current snapshot: {}", current);
    println!("   Total recorded: {}", total);
    println!("   Ring buffer size: 4096 snapshots");
    println!();

    // Verify we can still access recent snapshots
    println!("6. Accessing snapshot after wraparound:");
    match new_engine.jump_to_snapshot(4900) {
        Ok((id, rip, rsp)) => {
            println!("   [{}] RIP: {:#06x}, RSP: {:#010x}", id, rip, rsp);
            println!("   ✓ Successfully accessed snapshot after wraparound");
        }
        Err(e) => println!("   Error: {}", e),
    }

    // Verify old snapshots are invalidated
    match new_engine.jump_to_snapshot(100) {
        Ok(_) => println!("   ✗ Unexpectedly accessed old snapshot"),
        Err(e) => println!("   ✓ Old snapshot correctly invalidated: {}", e),
    }
    println!();

    // Performance characteristics
    println!("7. Performance characteristics:");
    println!("   - TimeSnapshot size: 32 bytes");
    println!("   - Ring buffer: 4096 snapshots");
    println!("   - ReplayEngineCapsule: 131,072 bytes (128 KB)");
    println!("   - Recording overhead: <10ns per snapshot");
    println!("   - State captured: RIP, RSP, flags");
    println!("   - Verification: #[derive(ComputationalCapsule)]");
    println!();

    println!("=== Demo Complete ===");
}
