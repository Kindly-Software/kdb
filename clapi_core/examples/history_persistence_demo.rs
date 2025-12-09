//! History Persistence Demo - Atomic Command History with Disk Persistence
//!
//! This example demonstrates the HistoryPersistenceCapsule integration with InputHandler.
//!
//! # Features
//! - 128B cache-aligned persistence capsule
//! - Atomic counters (load_count, save_count, error_flag)
//! - Max 1000 entries with FIFO eviction
//! - <10ms load/save latency
//! - Graceful degradation on I/O errors
//!
//! # Usage
//! ```bash
//! cargo run --example history_persistence_demo
//! ```

use clapi_core::tui::persistence::{HistoryPersistenceCapsule, HistoryPersistenceManager};
use std::io::{self, Write};

fn main() {
    println!("=== History Persistence Demo ===\n");

    // 1. Create persistence manager (default path: ~/.clapi/history)
    println!("1. Creating persistence manager...");
    let manager = HistoryPersistenceManager::new();
    println!("   ✓ Manager created");
    println!("   Path: {}", manager.capsule().file_path());
    println!();

    // 2. Load existing history
    println!("2. Loading history from disk...");
    match manager.load_history() {
        Ok(history) => {
            println!("   ✓ Loaded {} entries", history.len());
            if !history.is_empty() {
                println!("   Most recent: {}", history[0]);
            }
        }
        Err(e) => {
            println!("   ⚠ Load error: {}", e);
        }
    }
    println!();

    // 3. Add new entries
    println!("3. Adding new commands to history...");
    let commands = vec![
        "clapi start --profile production",
        "clapi metrics --watch 5",
        "clapi budget",
        "clapi providers",
        "clapi audit --last 100",
    ];

    for cmd in &commands {
        match manager.append_entry(cmd) {
            Ok(_) => println!("   ✓ Added: {}", cmd),
            Err(e) => println!("   ✗ Failed to add '{}': {}", cmd, e),
        }
    }
    println!();

    // 4. Load updated history
    println!("4. Loading updated history...");
    match manager.load_history() {
        Ok(history) => {
            println!("   ✓ Loaded {} entries", history.len());
            println!("   Recent commands:");
            for (i, entry) in history.iter().take(10).enumerate() {
                println!("     {}. {}", i + 1, entry);
            }
        }
        Err(e) => {
            println!("   ⚠ Load error: {}", e);
        }
    }
    println!();

    // 5. Display capsule statistics
    println!("5. Capsule statistics:");
    let capsule = manager.capsule();
    println!("   Load count: {}", capsule.load_count());
    println!("   Save count: {}", capsule.save_count());
    println!("   Last save: {} ns", capsule.last_save_ns());
    println!("   Error flag: {}", capsule.has_error());
    println!();

    // 6. Interactive mode (optional)
    println!("6. Interactive mode (type 'quit' to exit):");
    loop {
        print!("   > ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        if input == "quit" || input == "exit" {
            println!("   ✓ Exiting...");
            break;
        }

        // Add to history
        match manager.append_entry(input) {
            Ok(_) => println!("   ✓ Saved to history"),
            Err(e) => println!("   ✗ Failed to save: {}", e),
        }
    }

    // 7. Final statistics
    println!("\n7. Final statistics:");
    let capsule = manager.capsule();
    println!("   Load count: {}", capsule.load_count());
    println!("   Save count: {}", capsule.save_count());
    println!("   Error flag: {}", capsule.has_error());
    println!("\n=== Demo Complete ===");
}
