//! Build script for KDB Signup
//!
//! Generates BLAKE3 hashes for disposable email domains at compile time.
//! This allows O(1) lookup instead of string comparison.
//!
//! # Planned Features
//!
//! - Load disposable email domain list
//! - Generate BLAKE3 hashes for each domain
//! - Write to OUT_DIR as Rust const array
//!
//! # Framework Compliance
//!
//! - Compile-time computation (0ns runtime overhead)
//! - Deterministic output (reproducible builds)

fn main() {
    // Stub: Will generate disposable email hash set
    //
    // Future implementation:
    // 1. Read disposable domain list from data/disposable_domains.txt
    // 2. Hash each domain with BLAKE3
    // 3. Write const array to OUT_DIR/disposable_hashes.rs
    //
    // Example output:
    // pub const DISPOSABLE_HASHES: &[[u8; 32]; N] = &[
    //     [0x12, 0x34, ...], // mailinator.com
    //     [0x56, 0x78, ...], // tempmail.com
    //     ...
    // ];

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=data/disposable_domains.txt");
}
