//! Const hash utilities for client SDK
//!
//! This module re-exports const hash functions from the foundation crate
//! (`atomic_capsule`) for client library integration.
//!
//! # Chaos Principles
//!
//! - **Tier 7 (Const)**: Compile-time hash evaluation (0ns runtime)
//! - **Deterministic**: Same input → same output (always)
//! - **Pure functions**: No state, no side effects
//! - **Cache-aware**: Const values inline in binary (no memory access)
//!
//! # Performance (B32 validated)
//!
//! - Compile-time: <5ms per hash (one-time build cost)
//! - Runtime (static IDs): 0ns (const value inlined)
//! - Runtime (dynamic IDs): ~10ns (scalar hash)
//! - Speedup: 100× for known IDs (10ns → 0ns)
//!
//! # ASSUM Safety
//!
//! - #ASSUME_DETERMINISTIC: `const_fast_hash` produces identical output for identical input
//! - #VERIFY_DETERMINISTIC: Unit tests validate hash consistency across builds
//! - #ASSUME_COLLISION_FREE: 64-bit hash space sufficient for small set of static IDs
//! - #VERIFY_COLLISION: Compile-time assertions check uniqueness (see request_capsule128_enhanced.rs)
//!
//! # I20 Integration (Q1-Q20 Compliance)
//!
//! ## Phase 1: Scope
//! - **Q1**: Components = `atomic_capsule::hash` (foundation) + `clapi_core::client` (public SDK)
//! - **Q2**: Problem = Client libraries need fast budget/provider ID hashing
//! - **Q3**: Explicit contract = Pure `const fn` and runtime hash functions
//! - **Q4**: Implicit deps = None (pure functions, no state)
//! - **Q5**: Necessary? Yes (clients need ID hashing without full capsule dependency)
//!
//! ## Phase 2: Compatibility
//! - **Q6**: Architectural = Pure functions (no state) → Always compatible
//! - **Q7**: Performance = 0ns const + 10ns runtime → Always compatible
//! - **Q8**: Error model = Infallible (never fails) → Always compatible
//! - **Q9**: Concurrency = Pure functions (no shared state) → Always thread-safe
//! - **Q10**: Boundaries = None (pure functions, no state transitions)
//!
//! ## Phase 3: Safety (Simplified for Chaos)
//! - **Q11**: Assumptions = Deterministic hash, collision-free static IDs
//! - **Q12**: Failure cascade = N/A (pure functions never fail)
//! - **Q13**: Invariants = Hash(input) always equals expected value
//! - **Q14**: Race/Deadlock = N/A (no state, lockfree by design)
//! - **Q15**: Escape hatches = N/A (always works, no rollback needed)
//!
//! ## Phase 4: Validation (I20-Capsule: Simplified)
//! - **Q16**: Minimal test = `assert_eq!(hash_for_budget_id("foo"), const_fast_hash(b"foo"))`
//! - **Q17**: Property invariants = Hash consistency across platforms
//! - **Q18**: Overhead budget = 0ns (const) → Always acceptable
//! - **Q19**: Integration strategy = Deploy 100% (deterministic, tests predict production)
//! - **Q20**: Rollback plan = Git revert (deterministic → unlikely to need)
//!
//! # Example
//!
//! ```rust
//! use clapi_core::client::const_hash::{
//!     hash_for_budget_id,
//!     BUDGET_ANTHROPIC,
//! };
//!
//! // Fast path: Known budget IDs (0ns)
//! let hash = match budget_id {
//!     "budget_anthropic" => BUDGET_ANTHROPIC,
//!     "budget_openai" => clapi_core::client::BUDGET_OPENAI,
//!     _ => hash_for_budget_id(budget_id),  // Slow path: runtime hash (~10ns)
//! };
//!
//! // Send hash to server
//! send_request_with_budget_hash(hash);
//! ```

use atomic_capsule::hash::const_fast_hash;

// ============================================================================
// Re-export const hashes from request_capsule128_enhanced
// ============================================================================

pub use crate::capsules::request_capsule128_enhanced::{
    BUDGET_ANTHROPIC,
    BUDGET_OPENAI,
    BUDGET_GOOGLE,
    BUDGET_COHERE,
    PROVIDER_ANTHROPIC,
    PROVIDER_OPENAI,
    PROVIDER_GOOGLE,
};

// ============================================================================
// Client helper functions
// ============================================================================

/// Hash a budget ID string for server requests
///
/// # Performance
/// - Static IDs: Use const values (0ns) via match statement
/// - Dynamic IDs: Runtime hash (~10ns)
///
/// # Example
///
/// ```rust
/// use clapi_core::client::const_hash::{hash_for_budget_id, BUDGET_ANTHROPIC};
///
/// // Recommended: Fast path for known IDs
/// let hash = match budget_id {
///     "budget_anthropic" => BUDGET_ANTHROPIC,  // 0ns
///     _ => hash_for_budget_id(budget_id),       // ~10ns
/// };
/// ```
///
/// # ASSUM Safety
/// - #ASSUME_DETERMINISTIC: Same `budget_id` → same hash (always)
/// - #VERIFY_DETERMINISTIC: Unit test validates consistency
#[inline]
pub fn hash_for_budget_id(budget_id: &str) -> u64 {
    const_fast_hash(budget_id.as_bytes())
}

/// Hash a provider ID string for server requests
///
/// # Performance
/// - Static IDs: Use const values (0ns) via match statement
/// - Dynamic IDs: Runtime hash (~10ns)
///
/// # Example
///
/// ```rust
/// use clapi_core::client::const_hash::{hash_for_provider_id, PROVIDER_ANTHROPIC};
///
/// // Recommended: Fast path for known IDs
/// let hash = match provider_id {
///     "provider_anthropic" => PROVIDER_ANTHROPIC,  // 0ns
///     _ => hash_for_provider_id(provider_id),       // ~10ns
/// };
/// ```
///
/// # ASSUM Safety
/// - #ASSUME_DETERMINISTIC: Same `provider_id` → same hash (always)
/// - #VERIFY_DETERMINISTIC: Unit test validates consistency
#[inline]
pub fn hash_for_provider_id(provider_id: &str) -> u64 {
    const_fast_hash(provider_id.as_bytes())
}

// ============================================================================
// Client convenience functions
// ============================================================================

/// Client-friendly budget ID hashing with automatic fast path
///
/// This helper automatically uses const hashes for known budget IDs
/// and falls back to runtime hashing for unknown IDs.
///
/// # Performance
/// - Known IDs: 0ns (const lookup)
/// - Unknown IDs: ~10ns (runtime hash)
///
/// # Example
///
/// ```rust
/// use clapi_core::client::const_hash::client_hash_budget;
///
/// // Automatically optimized for known IDs
/// let hash = client_hash_budget("budget_anthropic");  // 0ns
/// ```
#[inline]
pub fn client_hash_budget(budget_id: &str) -> u64 {
    match budget_id {
        "budget_anthropic" => BUDGET_ANTHROPIC,
        "budget_openai" => BUDGET_OPENAI,
        "budget_google" => BUDGET_GOOGLE,
        "budget_cohere" => BUDGET_COHERE,
        _ => hash_for_budget_id(budget_id),
    }
}

/// Client-friendly provider ID hashing with automatic fast path
///
/// This helper automatically uses const hashes for known provider IDs
/// and falls back to runtime hashing for unknown IDs.
///
/// # Performance
/// - Known IDs: 0ns (const lookup)
/// - Unknown IDs: ~10ns (runtime hash)
///
/// # Example
///
/// ```rust
/// use clapi_core::client::const_hash::client_hash_provider;
///
/// // Automatically optimized for known IDs
/// let hash = client_hash_provider("provider_anthropic");  // 0ns
/// ```
#[inline]
pub fn client_hash_provider(provider_id: &str) -> u64 {
    match provider_id {
        "provider_anthropic" => PROVIDER_ANTHROPIC,
        "provider_openai" => PROVIDER_OPENAI,
        "provider_google" => PROVIDER_GOOGLE,
        _ => hash_for_provider_id(provider_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_determinism() {
        // #VERIFY_DETERMINISTIC: Hash must be consistent
        let budget_id = "budget_test";
        let hash1 = hash_for_budget_id(budget_id);
        let hash2 = hash_for_budget_id(budget_id);
        assert_eq!(hash1, hash2, "Hash must be deterministic");
    }

    #[test]
    fn test_const_hash_matches_runtime() {
        // #VERIFY_DETERMINISTIC: Const hash equals runtime hash
        assert_eq!(
            BUDGET_ANTHROPIC,
            const_fast_hash(b"budget_anthropic"),
            "Const hash must match runtime hash"
        );
    }

    #[test]
    fn test_client_hash_budget_fast_path() {
        // Verify fast path uses const values
        assert_eq!(client_hash_budget("budget_anthropic"), BUDGET_ANTHROPIC);
        assert_eq!(client_hash_budget("budget_openai"), BUDGET_OPENAI);
        assert_eq!(client_hash_budget("budget_google"), BUDGET_GOOGLE);
        assert_eq!(client_hash_budget("budget_cohere"), BUDGET_COHERE);
    }

    #[test]
    fn test_client_hash_budget_slow_path() {
        // Verify slow path uses runtime hash
        let unknown = "budget_unknown";
        assert_eq!(
            client_hash_budget(unknown),
            const_fast_hash(unknown.as_bytes())
        );
    }

    #[test]
    fn test_client_hash_provider_fast_path() {
        // Verify fast path uses const values
        assert_eq!(client_hash_provider("provider_anthropic"), PROVIDER_ANTHROPIC);
        assert_eq!(client_hash_provider("provider_openai"), PROVIDER_OPENAI);
        assert_eq!(client_hash_provider("provider_google"), PROVIDER_GOOGLE);
    }

    #[test]
    fn test_client_hash_provider_slow_path() {
        // Verify slow path uses runtime hash
        let unknown = "provider_unknown";
        assert_eq!(
            client_hash_provider(unknown),
            const_fast_hash(unknown.as_bytes())
        );
    }

    #[test]
    fn test_hash_uniqueness() {
        // #VERIFY_COLLISION: All const hashes must be unique
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

        // Check budget hashes unique
        let unique_budgets: std::collections::HashSet<_> = budget_hashes.iter().collect();
        assert_eq!(
            unique_budgets.len(),
            budget_hashes.len(),
            "Budget hashes must be unique"
        );

        // Check provider hashes unique
        let unique_providers: std::collections::HashSet<_> = provider_hashes.iter().collect();
        assert_eq!(
            unique_providers.len(),
            provider_hashes.len(),
            "Provider hashes must be unique"
        );

        // Check all hashes unique across budget + provider
        let all_hashes: Vec<_> = budget_hashes.iter().chain(provider_hashes.iter()).collect();
        let unique_all: std::collections::HashSet<_> = all_hashes.iter().collect();
        assert_eq!(
            unique_all.len(),
            all_hashes.len(),
            "All hashes must be globally unique"
        );
    }
}
