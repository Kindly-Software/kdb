//! Hash Chain Example
//!
//! Demonstrates how to use hash chains for audit trails and tamper detection.
//!
//! # Features
//! - Hash chain construction
//! - Tamper detection
//! - Audit trail validation
//! - Forensic analysis
//!
//! # Status
//! This example demonstrates Phase 3 features (CapsuleHash64, RequestCapsule128Enhanced).
//! These types will be implemented in Phase 3. This file serves as documentation and
//! specification for the expected API.
//!
//! # Usage
//! ```bash
//! cargo run --example hash_chain  # After Phase 3 implementation
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
    }
}

use placeholder::{CapsuleHash64, RequestCapsule128Enhanced};
use std::collections::VecDeque;

/// Represents a single entry in the hash chain
#[derive(Debug, Clone)]
struct ChainEntry {
    operation: String,
    budget_before: i64,
    budget_after: i64,
    hash: u64,
    prev_hash: u64,
}

/// Hash chain manager for audit trails
struct HashChainManager {
    capsule: RequestCapsule128Enhanced,
    chain: VecDeque<ChainEntry>,
}

impl HashChainManager {
    /// Create new hash chain manager
    fn new(initial_budget_cents: i64) -> Self {
        let capsule = RequestCapsule128Enhanced::new(initial_budget_cents);
        Self {
            capsule,
            chain: VecDeque::new(),
        }
    }

    /// Record a deduction in the hash chain
    fn deduct(&mut self, amount_cents: i64, description: &str) -> Result<(), String> {
        let budget_before = self.capsule.budget_cents();
        let prev_hash = self.capsule.hash();

        // Perform deduction
        self.capsule
            .try_deduct(amount_cents)
            .map_err(|e| format!("{:?}", e))?;

        let budget_after = self.capsule.budget_cents();
        let current_hash = self.capsule.hash();

        // Record in chain
        self.chain.push_back(ChainEntry {
            operation: description.to_string(),
            budget_before,
            budget_after,
            hash: current_hash,
            prev_hash,
        });

        Ok(())
    }

    /// Record a credit in the hash chain
    fn credit(&mut self, amount_cents: i64, description: &str) -> Result<(), String> {
        let budget_before = self.capsule.budget_cents();
        let prev_hash = self.capsule.hash();

        // Perform credit
        self.capsule
            .credit(amount_cents)
            .map_err(|e| format!("{:?}", e))?;

        let budget_after = self.capsule.budget_cents();
        let current_hash = self.capsule.hash();

        // Record in chain
        self.chain.push_back(ChainEntry {
            operation: description.to_string(),
            budget_before,
            budget_after,
            hash: current_hash,
            prev_hash,
        });

        Ok(())
    }

    /// Verify hash chain integrity
    fn verify_chain(&self) -> bool {
        if self.chain.is_empty() {
            return true;
        }

        // Check each link in the chain
        for i in 1..self.chain.len() {
            let prev = &self.chain[i - 1];
            let current = &self.chain[i];

            // Verify prev_hash matches previous entry's hash
            if current.prev_hash != prev.hash {
                println!("   ✗ Chain break at entry {}: prev_hash mismatch", i);
                println!("      Expected: 0x{:016x}", prev.hash);
                println!("      Found:    0x{:016x}", current.prev_hash);
                return false;
            }
        }

        true
    }

    /// Print hash chain
    fn print_chain(&self) {
        println!("   Hash Chain ({} entries):", self.chain.len());
        for (i, entry) in self.chain.iter().enumerate() {
            println!("   [{}] {}", i, entry.operation);
            println!("       Budget: ${:.2} -> ${:.2}",
                entry.budget_before as f64 / 100.0,
                entry.budget_after as f64 / 100.0
            );
            println!("       Prev Hash: 0x{:016x}", entry.prev_hash);
            println!("       Hash:      0x{:016x}", entry.hash);
        }
    }

    /// Simulate tampering (for demonstration)
    fn tamper_entry(&mut self, index: usize) {
        if let Some(entry) = self.chain.get_mut(index) {
            entry.budget_after += 100_00; // Add $100 illegally
            println!("   ⚠ Tampered with entry {} (added $100)", index);
        }
    }
}

fn main() {
    println!("=== Hash Chain Example ===\n");

    // Example 1: Build hash chain
    build_hash_chain();

    // Example 2: Verify integrity
    verify_chain_integrity();

    // Example 3: Detect tampering
    detect_tampering();

    // Example 4: Forensic analysis
    forensic_analysis();

    println!("\n=== Example Complete ===");
}

/// Example 1: Build hash chain
fn build_hash_chain() {
    println!("1. Building Hash Chain:");

    let mut manager = HashChainManager::new(1000_00); // $1000.00
    println!("   Initial budget: $1000.00\n");

    // Record several operations
    manager.deduct(50_00, "API call to GPT-4").unwrap();
    manager.deduct(25_00, "API call to Claude").unwrap();
    manager.credit(100_00, "Budget refill").unwrap();
    manager.deduct(75_00, "API call to GPT-4").unwrap();

    manager.print_chain();
    println!();
}

/// Example 2: Verify chain integrity
fn verify_chain_integrity() {
    println!("2. Verifying Chain Integrity:");

    let mut manager = HashChainManager::new(500_00); // $500.00

    // Build chain
    for i in 1..=10 {
        manager.deduct(10_00, &format!("Operation {}", i)).unwrap();
    }

    // Verify
    let is_valid = manager.verify_chain();
    if is_valid {
        println!("   ✓ Hash chain is valid (10 entries)\n");
    } else {
        println!("   ✗ Hash chain is corrupted\n");
    }
}

/// Example 3: Detect tampering
fn detect_tampering() {
    println!("3. Detecting Tampering:");

    let mut manager = HashChainManager::new(1000_00); // $1000.00

    // Build chain
    manager.deduct(100_00, "Operation 1").unwrap();
    manager.deduct(100_00, "Operation 2").unwrap();
    manager.deduct(100_00, "Operation 3").unwrap();

    println!("   Original chain:");
    let valid_before = manager.verify_chain();
    println!("   Valid: {}\n", valid_before);

    // Tamper with entry
    manager.tamper_entry(1);

    println!("\n   After tampering:");
    let valid_after = manager.verify_chain();
    println!("   Valid: {}", valid_after);

    if !valid_after {
        println!("   ✓ Tampering detected successfully\n");
    }
}

/// Example 4: Forensic analysis
fn forensic_analysis() {
    println!("4. Forensic Analysis:");

    let mut manager = HashChainManager::new(5000_00); // $5000.00

    // Simulate realistic transaction history
    manager.deduct(250_00, "GPT-4 Turbo call").unwrap();
    manager.deduct(150_00, "Claude 3 Sonnet call").unwrap();
    manager.credit(1000_00, "Monthly budget refill").unwrap();
    manager.deduct(500_00, "GPT-4 Vision call").unwrap();
    manager.deduct(300_00, "Claude 3 Opus call").unwrap();

    println!("   Transaction History:");
    manager.print_chain();

    println!("\n   Final State:");
    println!("   - Current budget: ${:.2}", manager.capsule.budget_cents() as f64 / 100.0);
    println!("   - Total entries: {}", manager.chain.len());
    println!("   - Chain valid: {}", manager.verify_chain());
    println!("   - Capsule integrity: {}", manager.capsule.verify_integrity());

    // Calculate totals
    let total_debits: i64 = manager.chain.iter()
        .filter(|e| e.budget_after < e.budget_before)
        .map(|e| e.budget_before - e.budget_after)
        .sum();

    let total_credits: i64 = manager.chain.iter()
        .filter(|e| e.budget_after > e.budget_before)
        .map(|e| e.budget_after - e.budget_before)
        .sum();

    println!("\n   Audit Summary:");
    println!("   - Total debits:  ${:.2}", total_debits as f64 / 100.0);
    println!("   - Total credits: ${:.2}", total_credits as f64 / 100.0);
    println!("   - Net change:    ${:.2}", (total_credits - total_debits) as f64 / 100.0);
    println!();
}
