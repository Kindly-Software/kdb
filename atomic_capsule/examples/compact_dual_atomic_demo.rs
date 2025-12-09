//! Demo of CompactDualAtomicU64 using native 128-bit atomics
//!
//! Run with: cargo run --example compact_dual_atomic_demo --features portable-atomic-u128

#[cfg(feature = "portable-atomic-u128")]
fn main() {
    use atomic_capsule::patterns::CompactDualAtomicU64;
    use std::sync::Arc;
    use std::thread;

    println!("=== CompactDualAtomicU64 Demo ===\n");

    // Create a compact dual atomic with initial values
    let dual = CompactDualAtomicU64::new(42, 0);
    println!("Created CompactDualAtomicU64 with initial values (42, 0)");

    // Size verification
    println!("Size: {} bytes (50% smaller than DualAtomicU64)", std::mem::size_of_val(&dual));
    println!("Alignment: {} bytes\n", std::mem::align_of_val(&dual));

    // Basic operations
    println!("--- Basic Operations ---");
    let (primary, secondary) = dual.load_both_acquire();
    println!("load_both_acquire(): ({}, {})", primary, secondary);

    dual.store_both_release(100, 1);
    println!("store_both_release(100, 1)");

    let (primary, secondary) = dual.load_both_acquire();
    println!("load_both_acquire(): ({}, {})\n", primary, secondary);

    // Write with generation
    println!("--- Write with Generation ---");
    for i in 0..5 {
        dual.write_with_generation(200 + i * 10);
        let (value, gen) = dual.load_both_acquire();
        println!("write_with_generation({}): value={}, generation={}", 200 + i * 10, value, gen);
    }
    println!();

    // Consistent read
    println!("--- Consistent Read ---");
    let read = dual.read_consistent();
    println!("read_consistent(): value={}, generation={}", read.value, read.generation);
    println!("(Always consistent - single 128-bit atomic load!)\n");

    // Compare-exchange
    println!("--- Compare-Exchange ---");
    let (current_val, current_gen) = dual.load_both_acquire();
    println!("Current state: ({}, {})", current_val, current_gen);

    match dual.compare_exchange_both(
        (current_val, current_gen),
        (500, current_gen + 1),
        std::sync::atomic::Ordering::AcqRel,
        std::sync::atomic::Ordering::Acquire,
    ) {
        Ok((old_val, old_gen)) => {
            println!("CAS succeeded! Old: ({}, {}), New: (500, {})", old_val, old_gen, current_gen + 1);
        }
        Err((actual_val, actual_gen)) => {
            println!("CAS failed. Actual: ({}, {})", actual_val, actual_gen);
        }
    }
    println!();

    // Concurrent stress test
    println!("--- Concurrent Stress Test ---");
    let dual_shared = Arc::new(CompactDualAtomicU64::new(0, 0));
    let mut handles = vec![];

    println!("Spawning 8 threads, each performing 10,000 write_with_generation operations...");

    let start = std::time::Instant::now();

    for thread_id in 0..8 {
        let dual = Arc::clone(&dual_shared);
        handles.push(thread::spawn(move || {
            for i in 0..10_000 {
                dual.write_with_generation((thread_id * 10_000 + i) as u64);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    let (final_value, final_gen) = dual_shared.load_both_acquire();

    println!("Completed in {:?}", elapsed);
    println!("Final state: value={}, generation={}", final_value, final_gen);
    println!("Expected generation: {} (8 threads × 10,000 writes)", 8 * 10_000);
    println!("Generation match: {}", if final_gen == 80_000 { "✅ PASS" } else { "❌ FAIL" });

    if final_gen == 80_000 {
        println!("\n✅ All 80,000 concurrent writes completed correctly!");
        println!("   Native 128-bit atomics guarantee consistency without retry loops!");
    }
}

#[cfg(not(feature = "portable-atomic-u128"))]
fn main() {
    println!("CompactDualAtomicU64 requires the 'portable-atomic-u128' feature.");
    println!("Run with: cargo run --example compact_dual_atomic_demo --features portable-atomic-u128");
}
