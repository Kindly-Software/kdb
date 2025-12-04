//! Demonstration of AccessControlCapsule functionality
//!
//! Shows practical usage patterns for secure MCP debugging access control.
//! **Tier**: T1 Atomic (lockfree bitmap access control)
//! **Latency**: <20ns PID check, <10ns command check

use kdb_mcp::access_control::{AccessControlCapsule, Command};
use std::sync::Arc;
use std::thread;

fn main() {
    println!("=== AccessControlCapsule Demo ===\n");

    // Create access control capsule
    let ac = Arc::new(AccessControlCapsule::new());

    println!("1. Basic PID Whitelisting");
    println!("   ========================");

    // Initially all PIDs are denied
    println!("   PID 1234 allowed? {}", ac.is_pid_allowed(1234));

    // Allow specific PIDs
    ac.allow_pid(1234).unwrap();
    ac.allow_pid(5678).unwrap();
    println!("   After allow_pid(1234): {}", ac.is_pid_allowed(1234));
    println!("   After allow_pid(5678): {}", ac.is_pid_allowed(5678));

    println!("\n2. Command Whitelisting");
    println!("   ======================");

    // Allow specific commands
    ac.allow_command(Command::Read).unwrap();
    ac.allow_command(Command::StackTrace).unwrap();

    println!("   Command::Read allowed? {}", ac.is_command_allowed(Command::Read));
    println!("   Command::Write allowed? {}", ac.is_command_allowed(Command::Write));

    println!("\n3. Gated Access Control");
    println!("   ======================");

    // Check both PID and command together
    match ac.check_access(1234, Command::Read) {
        Ok(_) => println!("   PID 1234 + Read: ALLOWED"),
        Err(e) => println!("   PID 1234 + Read: DENIED ({:?})", e),
    }

    match ac.check_access(1234, Command::Write) {
        Ok(_) => println!("   PID 1234 + Write: ALLOWED"),
        Err(e) => println!("   PID 1234 + Write: DENIED ({:?})", e),
    }

    match ac.check_access(9999, Command::Read) {
        Ok(_) => println!("   PID 9999 + Read: ALLOWED"),
        Err(e) => println!("   PID 9999 + Read: DENIED ({:?})", e),
    }

    println!("\n4. Audit Trail");
    println!("   ============");

    // Generate some denials
    let _ = ac.is_pid_allowed(9999);
    let _ = ac.is_command_allowed(Command::Write);
    let _ = ac.check_access(7777, Command::Step);

    let stats = ac.get_stats();
    println!("   Total denials: {}", stats.access_denied_count);
    println!("   Last denied PID: {}", stats.last_denied_pid);
    println!("   Last denied command: {}", stats.last_denied_cmd);

    println!("\n5. Dynamic Whitelist Management");
    println!("   ============================");

    // Deny a PID
    println!("   PID 5678 allowed? {} (before deny)", ac.is_pid_allowed(5678));
    ac.deny_pid(5678);
    println!("   PID 5678 allowed? {} (after deny)", ac.is_pid_allowed(5678));

    // Clear all
    println!("   Before clear_all:");
    println!("     PID 1234 allowed? {}", ac.is_pid_allowed(1234));
    println!("     Command::Read allowed? {}", ac.is_command_allowed(Command::Read));

    ac.clear_all();
    println!("   After clear_all:");
    println!("     PID 1234 allowed? {}", ac.is_pid_allowed(1234));
    println!("     Command::Read allowed? {}", ac.is_command_allowed(Command::Read));

    println!("\n6. Concurrent Access Test");
    println!("   ======================");

    let ac2 = Arc::new(AccessControlCapsule::new());

    // Whitelist PIDs for testing
    for pid in 0..32 {
        ac2.allow_pid(pid).unwrap();
    }
    ac2.allow_command(Command::Read).unwrap();

    let mut handles = vec![];

    // Spawn 8 threads checking access concurrently
    for thread_id in 0..8 {
        let ac_clone = Arc::clone(&ac2);

        handles.push(thread::spawn(move || {
            let mut count = 0;
            for _ in 0..10_000 {
                for pid in 0..32 {
                    if ac_clone.check_access(pid, Command::Read).is_ok() {
                        count += 1;
                    }
                }
            }
            (thread_id, count)
        }));
    }

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    println!("   Concurrent access results:");
    for (thread_id, count) in results {
        println!("     Thread {}: {} successful checks", thread_id, count);
    }

    println!("\n=== Demo Complete ===");
}
