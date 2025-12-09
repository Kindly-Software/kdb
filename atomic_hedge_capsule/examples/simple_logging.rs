//! Simple Logging Example
//!
//! Basic demonstration of the logging functionality.

use atomic_hedge_capsule::{
    init_logging, log_debug, log_error, log_info, log_warn, set_log_level, LogLevel,
};

fn main() {
    println!("=== Simple Logging Example ===\n");

    // Initialize logging at Info level
    init_logging(LogLevel::Info);

    log_info!("Logging system initialized");
    log_warn!("This is a warning message");
    log_error!("This is an error message");

    // Test different log levels
    set_log_level(LogLevel::Debug);
    log_debug!("Debug message - should now be visible");

    set_log_level(LogLevel::Error);
    log_info!("Info message - should be filtered out");
    log_error!("Error message - should still be visible");

    // Test with formatting
    log_error!("Error with value: {}", 42);
    log_error!("Multiple values: {} and {}", "hello", 3.14);

    println!("\nLogging demonstration completed!");
}
