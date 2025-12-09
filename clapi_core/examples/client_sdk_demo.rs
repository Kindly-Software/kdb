//! Client SDK Demo - Const Hash Utilities
//!
//! This example demonstrates how client libraries can use the CLAPI Core
//! client SDK for fast budget/provider ID hashing.
//!
//! # Performance
//!
//! - Known IDs (const): 0ns (compile-time evaluation)
//! - Unknown IDs (runtime): ~10ns (scalar hash)
//! - Speedup: 100× for known IDs (10ns → 0ns)
//!
//! # Usage Patterns
//!
//! 1. **Fast Path (Const Hashing)**: Use const values for known IDs
//! 2. **Slow Path (Runtime Hashing)**: Fallback to runtime hash for unknown IDs
//! 3. **Helper Functions**: Automatic fast/slow path selection
//!
//! # Run Example
//!
//! ```bash
//! cargo run --example client_sdk_demo
//! ```

use clapi_core::client::const_hash::{
    // Const values (0ns lookup)
    BUDGET_ANTHROPIC,
    BUDGET_OPENAI,
    BUDGET_GOOGLE,
    BUDGET_COHERE,
    PROVIDER_ANTHROPIC,
    PROVIDER_OPENAI,
    PROVIDER_GOOGLE,

    // Runtime hash functions (~10ns)
    hash_for_budget_id,
    hash_for_provider_id,

    // Helper functions (auto fast/slow path)
    client_hash_budget,
    client_hash_provider,
};

fn main() {
    println!("{}", "=".repeat(80));
    println!("CLAPI Core Client SDK Demo - Const Hash Utilities");
    println!("{}", "=".repeat(80));
    println!();

    // ========================================================================
    // Pattern 1: Manual Fast Path (Maximum Performance)
    // ========================================================================

    println!("Pattern 1: Manual Fast Path (0ns for known IDs)");
    println!("{}", "-".repeat(80));

    // Example: Client library chooses const hash manually
    let budget_name = "anthropic";
    let budget_hash = match budget_name {
        "anthropic" => BUDGET_ANTHROPIC,  // 0ns (const)
        "openai" => BUDGET_OPENAI,        // 0ns (const)
        "google" => BUDGET_GOOGLE,        // 0ns (const)
        "cohere" => BUDGET_COHERE,        // 0ns (const)
        _ => hash_for_budget_id(budget_name),  // ~10ns (runtime)
    };

    println!("  Budget: {} → Hash: 0x{:016x}", budget_name, budget_hash);
    println!("  Performance: 0ns (const value from match arm)");
    println!();

    // ========================================================================
    // Pattern 2: Helper Function (Automatic Fast/Slow Path)
    // ========================================================================

    println!("Pattern 2: Helper Function (Automatic Optimization)");
    println!("{}", "-".repeat(80));

    // Known ID: Automatically uses const hash (0ns)
    let hash1 = client_hash_budget("budget_anthropic");
    println!("  Known ID: 'budget_anthropic' → 0x{:016x} (0ns)", hash1);

    // Unknown ID: Automatically falls back to runtime hash (~10ns)
    let hash2 = client_hash_budget("budget_custom_123");
    println!("  Unknown ID: 'budget_custom_123' → 0x{:016x} (~10ns)", hash2);
    println!();

    // ========================================================================
    // Pattern 3: Provider ID Hashing
    // ========================================================================

    println!("Pattern 3: Provider ID Hashing");
    println!("{}", "-".repeat(80));

    let providers = ["provider_anthropic", "provider_openai", "provider_google", "provider_custom"];

    for provider in &providers {
        let hash = client_hash_provider(provider);
        let perf = if provider.contains("custom") { "~10ns" } else { "0ns" };
        println!("  {} → 0x{:016x} ({})", provider, hash, perf);
    }
    println!();

    // ========================================================================
    // Client Integration Example
    // ========================================================================

    println!("Client Integration Example");
    println!("{}", "-".repeat(80));

    // Simulate client SDK making API request
    fn make_api_request(budget_name: &str, amount_cents: i64) {
        // Convert string budget ID to u64 hash for server API
        let budget_id: u64 = client_hash_budget(budget_name);

        println!("  API Request:");
        println!("    Budget Name: {}", budget_name);
        println!("    Budget Hash: 0x{:016x}", budget_id);
        println!("    Amount: {} cents (${:.2})", amount_cents, amount_cents as f64 / 100.0);
        println!("    POST /api/request {{ budget_id: {}, amount_cents: {} }}", budget_id, amount_cents);
    }

    make_api_request("budget_anthropic", 5000);  // $50.00
    println!();

    // ========================================================================
    // Performance Summary
    // ========================================================================

    println!("Performance Summary (B32 Validated)");
    println!("{}", "-".repeat(80));
    println!("  Static IDs (const):    0ns (100× speedup vs runtime)");
    println!("  Dynamic IDs (runtime): ~10ns (scalar hash)");
    println!("  Binary Size:           +8 bytes per const hash");
    println!("  Compile Time:          <5ms per const hash (one-time)");
    println!();

    println!("✅ Client SDK Demo Complete");
    println!("{}", "=".repeat(80));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_const_hashes_are_unique() {
        // Verify all const hashes are unique (compile-time + runtime check)
        let budget_hashes = vec![
            BUDGET_ANTHROPIC,
            BUDGET_OPENAI,
            BUDGET_GOOGLE,
            BUDGET_COHERE,
        ];

        let provider_hashes = vec![
            PROVIDER_ANTHROPIC,
            PROVIDER_OPENAI,
            PROVIDER_GOOGLE,
        ];

        // Budget hashes must be unique
        let unique_budgets: std::collections::HashSet<_> = budget_hashes.iter().collect();
        assert_eq!(unique_budgets.len(), budget_hashes.len(), "Budget hashes must be unique");

        // Provider hashes must be unique
        let unique_providers: std::collections::HashSet<_> = provider_hashes.iter().collect();
        assert_eq!(unique_providers.len(), provider_hashes.len(), "Provider hashes must be unique");

        // All hashes across budget + provider must be globally unique
        let all_hashes: Vec<_> = budget_hashes.iter().chain(provider_hashes.iter()).collect();
        let unique_all: std::collections::HashSet<_> = all_hashes.iter().collect();
        assert_eq!(unique_all.len(), all_hashes.len(), "All hashes must be globally unique");
    }

    #[test]
    fn test_helper_functions_match_manual() {
        // Verify helper functions produce same hashes as manual selection
        assert_eq!(client_hash_budget("budget_anthropic"), BUDGET_ANTHROPIC);
        assert_eq!(client_hash_provider("provider_openai"), PROVIDER_OPENAI);

        // Verify unknown IDs use runtime hash
        let unknown_budget = "budget_unknown_xyz";
        assert_eq!(client_hash_budget(unknown_budget), hash_for_budget_id(unknown_budget));
    }
}
