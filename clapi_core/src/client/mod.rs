//! Client SDK for CLAPI Core
//!
//! This module provides utilities for client applications to integrate with CLAPI.
//!
//! # Architecture
//!
//! Pure computational capsule utilities (deterministic, no state):
//! - **Const hashing**: Compile-time hash computation (0ns runtime)
//! - **Budget ID hashing**: Fast hash for budget identification
//! - **Provider ID hashing**: Fast hash for provider selection
//!
//! # Performance (Chaos Tier 7: Const)
//! - Static IDs: 0ns (const evaluation at compile-time)
//! - Dynamic IDs: ~10ns (scalar runtime hash)
//! - Speedup: 100× for known IDs (10ns → 0ns)
//!
//! # Use Cases
//!
//! ```rust
//! use clapi_core::client::const_hash::{hash_for_budget_id, BUDGET_ANTHROPIC};
//!
//! // Client SDK example: Fast budget ID lookup
//! let budget_hash = match budget_id {
//!     "budget_anthropic" => BUDGET_ANTHROPIC,  // 0ns (const)
//!     _ => hash_for_budget_id(budget_id),       // ~10ns (runtime)
//! };
//! ```

pub mod const_hash;

// Re-export commonly used items for convenience
pub use const_hash::{
    // Compile-time budget ID hashes (0ns)
    BUDGET_ANTHROPIC,
    BUDGET_OPENAI,
    BUDGET_GOOGLE,
    BUDGET_COHERE,

    // Compile-time provider ID hashes (0ns)
    PROVIDER_ANTHROPIC,
    PROVIDER_OPENAI,
    PROVIDER_GOOGLE,

    // Hash computation functions (10ns runtime)
    hash_for_budget_id,
    hash_for_provider_id,
};
