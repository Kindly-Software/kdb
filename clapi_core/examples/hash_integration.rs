//! Hash Integration Example
//!
//! Demonstrates how to use RequestCapsule128Enhanced with built-in hash integrity.
//!
//! # Features
//! - Automatic hash updates on state changes
//! - Integrity verification
//! - Metrics export with hash validation
//! - Corruption detection
//!
//! # Status
//! This example demonstrates Phase 3 features (CapsuleHash64, RequestCapsule128Enhanced).
//! These types will be implemented in Phase 3. This file serves as documentation and
//! specification for the expected API.
//!
//! # Usage
//! ```bash
//! cargo run --example hash_integration  # After Phase 3 implementation
//! ```

// TODO: Uncomment after Phase 3 implementation
// use clapi_core::capsules::{CapsuleHash64, RequestCapsule128Enhanced};

// Placeholder implementation for documentation purposes
mod placeholder {
    pub struct CapsuleHash64;
    impl CapsuleHash64 {
        pub fn compute(_fields: &[u64]) -> u64 { 0 }
    }

    pub struct RequestCapsule128Enhanced;
    impl RequestCapsule128Enhanced {
        pub fn new(_budget_cents: i64) -> Self { Self }
        pub fn try_deduct(&self, _cost_cents: i64) -> Result<i64, String> { Ok(0) }
        pub fn credit(&self, _amount_cents: i64) -> Result<i64, String> { Ok(0) }
        pub fn hash(&self) -> u64 { 0 }
        pub fn verify_integrity(&self) -> bool { true }
        pub fn budget_cents(&self) -> i64 { 0 }
        pub fn metrics(&self) -> Option<Metrics> { None }
    }

    pub struct Metrics {
        pub deduction_count: u32,
        pub failed_deductions: u32,
        pub current_hash: u64,
        pub prev_hash: u64,
    }
}

use placeholder::{CapsuleHash64, RequestCapsule128Enhanced};
use std::sync::Arc;
use std::thread;

fn main() {
    println!("=== Hash Integration Example ===\n");

    // Example 1: Basic hash usage
    basic_hash_demo();

    // Example 2: Automatic hash updates
    automatic_hash_updates();

    // Example 3: Integrity verification
    integrity_verification();

    // Example 4: Concurrent access
    concurrent_hash_updates();

    // Example 5: Metrics export with verification
    metrics_with_verification();

    println!("\n=== Example Complete ===");
}

/// Example 1: Basic hash computation
fn basic_hash_demo() {
    println!("1. Basic Hash Computation:");

    let fields = [1, 2, 3, 4];
    let hash1 = CapsuleHash64::compute(&fields);
    let hash2 = CapsuleHash64::compute(&fields);

    println!("   Fields: {:?}", fields);
    println!("   Hash 1: 0x{:016x}", hash1);
    println!("   Hash 2: 0x{:016x}", hash2);
    println!("   Deterministic: {}", hash1 == hash2);

    // Different inputs produce different hashes
    let fields2 = [1, 2, 3, 5];
    let hash3 = CapsuleHash64::compute(&fields2);
    println!("   Different input: {:?} -> 0x{:016x}", fields2, hash3);
    println!("   Collision-free: {}\n", hash1 != hash3);
}

/// Example 2: Automatic hash updates
fn automatic_hash_updates() {
    println!("2. Automatic Hash Updates:");

    let capsule = RequestCapsule128Enhanced::new(1000_00); // $1000.00
    let initial_hash = capsule.hash();
    println!("   Initial budget: $1000.00");
    println!("   Initial hash: 0x{:016x}", initial_hash);

    // Deduct $50.00 - hash updates automatically
    capsule.try_deduct(50_00).unwrap();
    let hash_after_deduct = capsule.hash();
    println!("\n   After deducting $50.00:");
    println!("   New hash: 0x{:016x}", hash_after_deduct);
    println!("   Hash changed: {}", initial_hash != hash_after_deduct);

    // Credit $25.00 - hash updates again
    capsule.credit(25_00).unwrap();
    let hash_after_credit = capsule.hash();
    println!("\n   After crediting $25.00:");
    println!("   New hash: 0x{:016x}", hash_after_credit);
    println!("   Hash changed: {}\n", hash_after_deduct != hash_after_credit);
}

/// Example 3: Integrity verification
fn integrity_verification() {
    println!("3. Integrity Verification:");

    let capsule = RequestCapsule128Enhanced::new(500_00); // $500.00
    println!("   Initial budget: $500.00");

    // Perform several operations
    for i in 1..=5 {
        capsule.try_deduct(10_00).unwrap(); // Deduct $10.00
        let is_valid = capsule.verify_integrity();
        println!("   Operation {}: Integrity check = {}", i, is_valid);
    }

    // Verify final state
    if capsule.verify_integrity() {
        println!("\n   ✓ All operations maintained integrity");
    } else {
        println!("\n   ✗ Corruption detected!");
    }
    println!();
}

/// Example 4: Concurrent access
fn concurrent_hash_updates() {
    println!("4. Concurrent Hash Updates:");

    let capsule = Arc::new(RequestCapsule128Enhanced::new(10_000_00)); // $10,000.00
    let mut handles = vec![];

    println!("   Starting 10 threads (100 ops each)...");

    for thread_id in 0..10 {
        let c = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let _ = c.try_deduct(1_00); // Deduct $1.00
            }
            thread_id
        }));
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify integrity after concurrent operations
    let is_valid = capsule.verify_integrity();
    println!("   Operations complete: 1000 deductions");
    println!("   Integrity check: {}", is_valid);

    if is_valid {
        println!("   ✓ Hash integrity maintained under concurrency\n");
    } else {
        println!("   ✗ Corruption detected under concurrency\n");
    }
}

/// Example 5: Metrics export with verification
fn metrics_with_verification() {
    println!("5. Metrics Export with Verification:");

    let capsule = RequestCapsule128Enhanced::new(1000_00); // $1000.00

    // Perform operations
    for _ in 0..50 {
        let _ = capsule.try_deduct(10_00); // $10.00 each
    }

    // Export metrics (returns None if corrupted)
    match capsule.metrics() {
        Some(metrics) => {
            println!("   ✓ Metrics verified and exported:");
            println!("     - Deduction count: {}", metrics.deduction_count);
            println!("     - Failed deductions: {}", metrics.failed_deductions);
            println!("     - Current hash: 0x{:016x}", metrics.current_hash);
            println!("     - Previous hash: 0x{:016x}", metrics.prev_hash);
        }
        None => {
            println!("   ✗ Metrics export failed (corrupted state)");
        }
    }
    println!();
}
