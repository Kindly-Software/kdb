//! Example: Verify build-time constants
//!
//! Demonstrates runtime access to customer ID, build signature, and build timestamp.
//!
//! Usage:
//! ```bash
//! cargo run --example verify_build --release
//! ```

use kindly_dedup::protection::BuildVerification;

fn main() {
    let build_info = BuildVerification::get();

    println!("=== Build Verification ===");
    println!("{}", build_info);

    // Verify integrity
    if build_info.verify_integrity() {
        println!("\n✓ Build integrity check PASSED");
        std::process::exit(0);
    } else {
        eprintln!("\n✗ Build integrity check FAILED");
        std::process::exit(1);
    }
}
