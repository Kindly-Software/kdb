//! SymbolResolverCapsule Demo - DWARF Symbol Resolution
//!
//! Demonstrates T5 Streaming + T9 Persistent symbol resolution using gimli.
//!
//! **Performance Targets**:
//! - DWARF parse: <100ms (one-time)
//! - Symbol lookup: <50μs cold, <500ns cached
//!
//! **Usage**:
//! ```bash
//! # Build with debug symbols
//! cargo build --example symbol_resolver_demo
//!
//! # Run demo
//! cargo run --example symbol_resolver_demo
//! ```

use kdb::SymbolResolverCapsule;
use std::process;

fn main() {
    println!("=== SymbolResolverCapsule Demo ===\n");

    // Create symbol resolver capsule
    let capsule = match SymbolResolverCapsule::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create SymbolResolverCapsule: {}", e);
            process::exit(1);
        }
    };

    println!("✓ Created SymbolResolverCapsule");
    println!(
        "  Size: {} KB",
        std::mem::size_of::<SymbolResolverCapsule>() / 1024
    );
    println!("  Symbol count: {}", capsule.symbol_count());
    println!("  Generation: {}\n", capsule.generation());

    // Cache symbols for current process (self)
    let pid = process::id() as i32;
    println!("Caching symbols for PID {}...", pid);

    let start = std::time::Instant::now();
    match capsule.cache_symbols(pid) {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!(
                "✓ Cached symbols in {:.2}ms",
                elapsed.as_secs_f64() * 1000.0
            );
            println!("  Symbol count: {}", capsule.symbol_count());
            println!("  Generation: {}\n", capsule.generation());
        }
        Err(e) => {
            eprintln!("Failed to cache symbols: {}", e);
            process::exit(1);
        }
    }

    // Resolve main function address
    println!("Resolving symbol for main function...");

    // Get address of main (this is a simplified demo - in real usage, you'd get this from registers/backtrace)
    let main_addr = main as *const () as u64;
    println!("  Address: 0x{:x}", main_addr);

    let start = std::time::Instant::now();
    match capsule.resolve_symbol(pid, main_addr) {
        Ok(symbol) => {
            let elapsed = start.elapsed();
            println!(
                "✓ Resolved symbol in {:.2}μs",
                elapsed.as_secs_f64() * 1_000_000.0
            );
            println!("  Name: {}", symbol.name);
            println!("  File: {}", symbol.file);
            println!("  Line: {}", symbol.line);
            println!("  Column: {}", symbol.column);
        }
        Err(e) => {
            println!("Symbol not found: {}", e);
            println!("  (This is expected if binary has no debug symbols)");
            println!("  Build with: RUSTFLAGS=\"-C debuginfo=2\" cargo build");
        }
    }

    println!("\n=== Demo Complete ===");
}
