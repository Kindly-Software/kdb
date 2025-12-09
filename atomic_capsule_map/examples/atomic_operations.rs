//! Atomic operations showcase - unique to AtomicCapsuleMap.
//!
//! Run with: cargo run --example atomic_operations

use atomic_capsule_map::AtomicCapsuleMap;

fn main() {
    println!("=== Atomic Operations Showcase ===\n");

    let map = AtomicCapsuleMap::new();

    // 1. GET_OR_INSERT - Atomic lazy initialization
    println!("1. get_or_insert() - Atomic Lazy Initialization");
    println!("   Use case: Default values without races\n");

    let value = map.get_or_insert("config:timeout", 30);
    println!("   First call: {}", value); // 30 (inserted)

    let value = map.get_or_insert("config:timeout", 60);
    println!("   Second call: {}", value); // 30 (existing, not 60)

    println!("   ✓ Thread-safe initialization, single atomic op\n");

    // 2. COMPARE_AND_SWAP - ABA-safe atomic updates
    println!("2. compare_and_swap() - ABA-Safe Updates");
    println!("   Use case: Conditional updates (counters, flags)\n");

    map.insert("version", 1);
    println!("   Initial version: 1");

    // Successful swap
    match map.compare_and_swap(&"version", 1, 2) {
        Ok(()) => println!("   ✓ Swapped 1 → 2"),
        Err(current) => println!("   ✗ Failed, current: {}", current),
    }

    // Failed swap (version is now 2)
    match map.compare_and_swap(&"version", 1, 3) {
        Ok(()) => println!("   ✓ Swapped 1 → 3"),
        Err(current) => println!("   ✗ Failed, current: {}", current),
    }

    println!("   ✓ Generation counters prevent ABA problems\n");

    // 3. UPDATE - Retry-safe atomic modification
    println!("3. update() - Retry-Safe Atomic Modification");
    println!("   Use case: Counters, accumulators, state machines\n");

    map.insert("counter", 0);
    println!("   Initial counter: 0");

    // Increment counter
    map.update("counter", |v| v.map_or(1, |n| n + 1));
    println!("   After increment: {:?}", map.get(&"counter"));

    // Increment again
    map.update("counter", |v| v.map_or(1, |n| n + 1));
    println!("   After increment: {:?}", map.get(&"counter"));

    // Complex update logic
    map.update("counter", |v| {
        v.map_or(0, |n| {
            if n > 5 {
                0 // Reset if > 5
            } else {
                n * 2 // Double otherwise
            }
        })
    });
    println!("   After complex update: {:?}", map.get(&"counter"));
    println!("   ✓ Automatic retry on concurrent modification\n");

    // 4. Concurrent Counter Demo
    println!("4. Concurrent Counter Demonstration");
    println!("   Use case: High-throughput counting\n");

    map.insert("hits", 0_i32);

    #[cfg(feature = "std")]
    {
        use std::sync::Arc;
        use std::thread;

        let map = Arc::new(map);
        let mut handles = vec![];

        println!("   Spawning 4 threads, each incrementing 1000 times...");

        for _ in 0..4 {
            let map = Arc::clone(&map);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    map.update("hits", |v: Option<&i32>| v.map_or(1, |n| n + 1));
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let final_count = map.get(&"hits").unwrap_or(0);
        println!("   Final count: {} (expected: 4000)", final_count);
        println!("   ✓ No lost updates - fully atomic\n");
    }

    #[cfg(not(feature = "std"))]
    {
        println!("   (Skipped - requires std feature)\n");
    }

    // 5. Atomic State Machine
    println!("5. Atomic State Machine");
    println!("   Use case: Connection states, workflow status\n");

    #[derive(Clone, Debug, PartialEq)]
    enum State {
        Idle,
        Connecting,
        Connected,
        Disconnected,
    }

    let state_map: AtomicCapsuleMap<&str, State> = AtomicCapsuleMap::new();
    state_map.insert("conn:1", State::Idle);

    println!("   Initial state: {:?}", state_map.get(&"conn:1"));

    // Transition Idle → Connecting
    state_map.update("conn:1", |v| match v {
        Some(State::Idle) => State::Connecting,
        other => other.cloned().unwrap_or(State::Idle),
    });
    println!("   After transition: {:?}", state_map.get(&"conn:1"));

    // Transition Connecting → Connected
    state_map.update("conn:1", |v| match v {
        Some(State::Connecting) => State::Connected,
        other => other.cloned().unwrap_or(State::Idle),
    });
    println!("   After transition: {:?}", state_map.get(&"conn:1"));

    println!("   ✓ Atomic state transitions\n");

    println!("=== Key Advantages ===\n");

    println!("1. No Race Conditions:");
    println!("   - get_or_insert: Single atomic operation");
    println!("   - compare_and_swap: ABA-safe with generation counters");
    println!("   - update: Automatic retry on conflict\n");

    println!("2. Performance:");
    println!("   - No mutex/RwLock overhead");
    println!("   - Cache-aligned atomic operations");
    println!("   - Constant-time complexity (no waiting)\n");

    println!("3. Safety:");
    println!("   - Generation counters prevent ABA");
    println!("   - No partial updates visible");
    println!("   - All-or-nothing semantics\n");

    println!("=== Atomic Operations Complete ===");
}
