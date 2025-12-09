//! Logging demonstration example
//!
//! This example demonstrates the Chaos-compliant logging system with:
//! - RUST_LOG environment variable parsing
//! - Module-level filtering
//! - Multiple log levels
//!
//! # Run with different log levels:
//! ```bash
//! # Default (info level)
//! cargo run --example logging_demo --features logging
//!
//! # Debug level
//! RUST_LOG=debug cargo run --example logging_demo --features logging
//!
//! # Module-specific
//! RUST_LOG=atomic_capsule::logging=trace cargo run --example logging_demo --features logging
//!
//! # Multiple modules
//! RUST_LOG=trace,other_module=info cargo run --example logging_demo --features logging
//! ```

#[cfg(feature = "logging")]
fn main() {
    use atomic_capsule::logging::EnvLoggerCapsule;
    use atomic_capsule::{debug, error, info, trace, warn};

    // Initialize logger from RUST_LOG environment variable
    if let Err(e) = EnvLoggerCapsule::init() {
        eprintln!("Failed to initialize logger: {}", e);
        return;
    }

    // Demonstrate all log levels
    error!("This is an error message (red)");
    warn!("This is a warning message (yellow)");
    info!("This is an info message (default)");
    debug!("This is a debug message (only if RUST_LOG=debug)");
    trace!("This is a trace message (only if RUST_LOG=trace)");

    println!("\n--- Messages with formatting ---");
    let version = "1.0.0";
    let count = 42;
    info!("Application v{} started with {} items", version, count);
    debug!("Debug info: count={}, version={}", count, version);

    println!("\n--- Target-specific logging ---");
    info!(target: "network", "Network subsystem initialized");
    debug!(target: "network", "Network: Creating socket on port 8080");
    trace!(target: "network", "Network: Socket configuration complete");

    println!("\n--- Complex formatting ---");
    let items = vec!["apple", "banana", "cherry"];
    debug!("Processing {} items: {:?}", items.len(), items);
    info!("Ready to process {} documents", 1000);

    println!("\n--- Module path (automatic) ---");
    info!("Message with module path (logged with module_path!())");

    println!("\nTip: Set RUST_LOG environment variable to control logging:");
    println!("  RUST_LOG=debug           - Enable debug messages");
    println!("  RUST_LOG=trace           - Enable trace messages");
    println!("  RUST_LOG=network=debug   - Debug level for 'network' target");
    println!("  RUST_LOG=trace,other=info - Trace by default, info for 'other'");
}

#[cfg(not(feature = "logging"))]
fn main() {
    eprintln!("This example requires the 'logging' feature to be enabled.");
    eprintln!("Run with: cargo run --example logging_demo --features logging");
}
