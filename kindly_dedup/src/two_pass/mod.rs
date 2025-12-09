//! Two-Pass Exact→Fuzzy Deduplication (SOTA Phase 3.2)
//!
//! **Problem**: MinHash is expensive (~17µs), but ~40% of duplicates are EXACT
//! **Solution**: Two-pass deduplication:
//! 1. **Pass 1**: XXH3-128 exact hash (fast, <5ns per hash, catches 40% of duplicates)
//! 2. **Pass 2**: MinHash fuzzy dedup (only for remaining 60%)
//!
//! **Performance Target**: 1.67× overall speedup (40% skip MinHash at 17µs each)
//!
//! **Framework Compliance**:
//! - **UCE34**: T1 Atomic tier (lockfree hash table)
//! - **Chaos**: 100% lockfree (RobinHoodHashCapsule)
//! - **ASSUM**: XXH3-128 collision assumptions documented
//! - **B32**: Fair baseline (vs current MinHash-only pipeline)
//! - **T28**: Unit tests, property tests

mod exact_hash_capsule;

pub use exact_hash_capsule::{ExactHashCapsule, ExactHashStats};
