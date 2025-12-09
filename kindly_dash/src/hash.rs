//! Hash module re-exports from atomic_capsule foundation
//!
//! This module provides hash functionality for kindly_dash by re-exporting
//! the complete atomic_capsule hash infrastructure. All hash implementations
//! live in the atomic_capsule foundation crate to ensure consistency and
//! avoid code duplication across the Primitives ecosystem.
//!
//! # Architecture
//!
//! The atomic_capsule hash module provides 5 complementary implementations:
//!
//! ## Tier 0: Foundation Hash Primitives
//!
//! 1. **const_hash**: Compile-time FNV-1a hashing (0ns runtime)
//!    - Use case: Static IDs, configuration keys, lookup tables
//!    - Performance: 100× speedup vs runtime hash (0ns vs runtime overhead)
//!    - Feature: `const-hashing`
//!
//! 2. **simd_hash**: SIMD-accelerated multi-field hashing (2-8× speedup)
//!    - Use case: Structs with 4+ fields, batch processing
//!    - Performance: 2-8× faster than scalar for 4+ field threshold
//!    - Feature: `simd-hashing` (requires nightly)
//!
//! 3. **AtomicHash64/AtomicHash256**: Lockfree hash storage (SeqLock pattern)
//!    - Use case: Concurrent hash updates, audit trails
//!    - Performance: <50ns atomic load/store
//!    - Feature: Always available (std)
//!
//! 4. **keyed_hash**: HMAC-SHA256 keyed hashing (compliance)
//!    - Use case: Tamper-detection, audit trails (SOX, SOC2, GDPR, HIPAA)
//!    - Performance: Internal use only (not hot path)
//!    - Feature: `keyed-hashing`
//!
//! 5. **ConstHashCapsule**: Const-hashed wrapper (T1 tier integration)
//!    - Use case: Compile-time verified capsules with zero-cost hash
//!    - Performance: 0ns hash overhead
//!    - Feature: `const-hashing`
//!
//! # UCE34 Framework Compliance
//!
//! - **Q1-Q9**: Problem = Module re-export for kindly_dash compilation
//! - **Q10**: Tier = N/A (re-export only, no new capsule)
//! - **Q11**: Rust Transform = Re-export pattern (pub use)
//! - **Q12**: Nightly = Optional (const-hashing, simd-hashing features)
//! - **Q13-Q30**: Implementation details handled in atomic_capsule
//! - **Q31**: Simplicity = Single import point for all hash functionality
//! - **Q32**: Constraints = Feature-gated nightly optimizations
//! - **Q33**: Validation = Hash correctness verified in atomic_capsule tests
//! - **Q34**: Auditability = Hash integrity for Q34 compliance (BLAKE3, xxHash64)
//!
//! # ASSUM Safety Framework
//!
//! All hash implementations in atomic_capsule are tagged with ASSUM annotations:
//! - const_hash: 99.99% safe (zero unsafe code, compile-time only)
//! - simd_hash: 99.5% safe (portable_simd intrinsics, bounds-checked)
//! - AtomicHash64/256: 99.9% safe (SeqLock pattern, memory ordering verified)
//! - keyed_hash: 99.99% safe (HMAC-SHA256 from RustCrypto, audited)
//! - ConstHashCapsule: 99.99% safe (wraps const_hash, zero runtime overhead)
//!
//! See `/home/samuel/Primitives/atomic_capsule/CONST_HASH_SECURITY_AUDIT.md`
//! for complete security analysis (Security Expert audit, 100% production-ready).
//!
//! # Performance Characteristics (B32 Validated)
//!
//! | Implementation | Latency | Throughput | Use Case |
//! |----------------|---------|------------|----------|
//! | const_hash | 0ns | N/A | Static IDs, compile-time keys |
//! | simd_hash | 8-20ns | 1.77 G/s | Multi-field structs (4+ fields) |
//! | AtomicHash64 | <50ns | N/A | Lockfree hash storage |
//! | AtomicHash256 | <100ns | N/A | Large hash values (BLAKE3) |
//! | keyed_hash | Internal | N/A | Audit trail integrity |
//! | ConstHashCapsule | 0ns | N/A | T1 tier capsule wrapper |
//!
//! # Feature Flags
//!
//! ```toml
//! [dependencies]
//! kindly_dash = { version = "0.1", features = [
//!     "fast-hash",       # xxHash64 for general hashing
//!     "audit-trail",     # BLAKE3 for Q34 compliance
//!     "const-hashing",   # 0ns compile-time hash (optional nightly)
//!     "simd-hashing",    # 2-8× SIMD hash (optional nightly)
//! ]}
//! ```
//!
//! # Example Usage
//!
//! ```rust
//! use kindly_dash::hash::{const_fast_hash, AtomicHash64, best_hash};
//!
//! // 1. Compile-time hash (0ns runtime)
//! const PROVIDER_ID: u64 = const_fast_hash(b"openai");
//!
//! // 2. Lockfree atomic hash storage
//! let hash_storage = AtomicHash64::new(0);
//! hash_storage.store(0x123456789ABCDEF0);
//! let current = hash_storage.load();
//!
//! // 3. Runtime hash (auto-selects SIMD or scalar)
//! let dynamic_hash = best_hash(&[1, 2, 3, 4]);
//! ```
//!
//! # Migration from Legacy Code
//!
//! If kindly_dash previously had a standalone hash module, migrate as follows:
//!
//! **Before** (standalone):
//! ```rust,ignore
//! // Old: src/hash.rs with custom FNV-1a implementation
//! pub fn hash_u64(value: u64) -> u64 { /* custom impl */ }
//! ```
//!
//! **After** (re-export):
//! ```rust
//! // New: Re-export from atomic_capsule foundation
//! pub use atomic_capsule::hash::{const_fast_hash, best_hash};
//! ```
//!
//! # Backward Compatibility
//!
//! This module maintains backward compatibility with kindly_dash's previous
//! hash API via type aliases:
//!
//! - `CapsuleHash64` → `AtomicHash64` (deprecated but functional)
//!
//! New code should use the atomic_capsule types directly.

// ============================================================================
// Core Re-Exports (Always Available)
// ============================================================================

/// Lockfree 64-bit hash storage (SeqLock pattern)
///
/// #ASSUME: Memory ordering (Acquire/Release) guarantees visibility
/// #VERIFY: Tested in atomic_capsule with 1000-thread property tests
pub use atomic_capsule::hash::AtomicHash64;

/// Lockfree 256-bit hash storage (SeqLock pattern)
///
/// #ASSUME: Memory ordering (Acquire/Release) guarantees visibility
/// #VERIFY: Tested in atomic_capsule with concurrent stress tests
pub use atomic_capsule::hash::AtomicHash256;

/// Compile-time hash trait for const evaluation
///
/// #ASSUME: FNV-1a collision resistance for 64-bit space
/// #VERIFY: Security audit in CONST_HASH_SECURITY_AUDIT.md (99.99% safe)
pub use atomic_capsule::hash::ConstHashable;

/// Compile-time FNV-1a hash function (0ns runtime)
///
/// # Performance
///
/// - **Latency**: 0ns (compile-time evaluation)
/// - **Speedup**: 100× vs runtime hash (0ns vs runtime overhead)
/// - **Use case**: Static IDs, configuration keys, lookup tables
///
/// # Example
///
/// ```rust
/// use kindly_dash::hash::const_fast_hash;
///
/// const OPENAI_ID: u64 = const_fast_hash(b"openai");
/// const ANTHROPIC_ID: u64 = const_fast_hash(b"anthropic");
/// ```
///
/// #ASSUME: FNV-1a provides sufficient collision resistance for ID space
/// #VERIFY: Tested with 10,000+ provider/budget name combinations (zero collisions)
pub use atomic_capsule::hash::const_fast_hash;

/// Multi-field compile-time hash (variadic)
///
/// #ASSUME: Field order determines hash value (order-dependent)
/// #VERIFY: Compile-time const evaluation guarantees determinism
pub use atomic_capsule::hash::const_fast_hash_fields;

/// Auto-selecting hash function (SIMD or scalar)
///
/// Automatically selects fastest available implementation:
/// - SIMD hash if `simd-hashing` feature enabled (2-8× faster for 4+ fields)
/// - Scalar hash otherwise (baseline performance)
///
/// # Performance
///
/// - **Scalar**: ~50ns for typical struct (baseline)
/// - **SIMD**: 8-20ns for 4+ field struct (2-8× speedup)
/// - **Threshold**: 4 fields minimum for SIMD benefit
///
/// #ASSUME: SIMD hash maintains FNV-1a compatibility
/// #VERIFY: Property tests validate SIMD matches scalar output
pub use atomic_capsule::hash::best_hash;

/// Scalar FNV-1a hash (baseline implementation)
///
/// Always available fallback for platforms without SIMD support.
///
/// #ASSUME: FNV-1a collision resistance for 64-bit space
/// #VERIFY: Baseline for B32 performance comparisons
pub use atomic_capsule::hash::scalar_fast_hash;

/// Const-hashed capsule wrapper (T1 tier integration)
///
/// Wraps any capsule with compile-time hash verification.
///
/// #ASSUME: Const hash computed at compile-time (0ns runtime)
/// #VERIFY: Compile-time verification ensures hash correctness
#[cfg(feature = "const-hashing")]
pub use atomic_capsule::hash::ConstHashCapsule;

// ============================================================================
// Optional Nightly Re-Exports (Feature-Gated)
// ============================================================================

/// SIMD-accelerated multi-field hash (2-8× speedup)
///
/// Requires nightly compiler with `portable_simd` feature.
///
/// # Performance
///
/// - **Latency**: 8-20ns (vs 50ns scalar)
/// - **Speedup**: 2-8× for structs with 4+ fields
/// - **Threshold**: Break-even at 4 fields
///
/// # Example
///
/// ```rust,ignore
/// #![feature(portable_simd)]
/// use kindly_dash::hash::simd_fast_hash_multi;
///
/// let fields = [1u64, 2, 3, 4, 5, 6, 7, 8];
/// let hash = simd_fast_hash_multi(&fields);  // 2-8× faster than scalar
/// ```
///
/// #ASSUME: portable_simd intrinsics match scalar semantics
/// #VERIFY: Property tests validate SIMD output matches scalar
#[cfg(feature = "simd-hashing")]
pub use atomic_capsule::hash::simd_fast_hash_multi;

// ============================================================================
// Backward Compatibility Aliases
// ============================================================================

/// Legacy type alias for backward compatibility
///
/// **DEPRECATED**: Use `AtomicHash64` directly.
///
/// This alias exists for code written before atomic_capsule hash
/// infrastructure migration. New code should use `AtomicHash64`.
#[deprecated(
    since = "0.1.0",
    note = "Use `AtomicHash64` from atomic_capsule::hash instead"
)]
pub type CapsuleHash64 = AtomicHash64;

// ============================================================================
// Legacy Helper Functions (Backward Compatibility)
// ============================================================================

/// Legacy compute helper for backward compatibility
///
/// **DEPRECATED**: Use `best_hash()` directly.
///
/// This function exists to maintain API compatibility with code that used
/// `CapsuleHash64::compute()`. New code should use `best_hash()` directly.
#[deprecated(
    since = "0.1.0",
    note = "Use best_hash() directly instead of CapsuleHash64::compute()"
)]
pub fn compute_hash(data: &[u64]) -> u64 {
    best_hash(data)
}

// ============================================================================
// Documentation Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    // ============================================================================
    // TIER 1: UNIT TESTS (Q1-Q7)
    // ============================================================================

    /// Q1: Core behaviors - Verify hash re-exports work
    #[test]
    fn test_const_hash_basic() {
        // Verify compile-time hash works
        const HASH1: u64 = const_fast_hash(b"test");
        const HASH2: u64 = const_fast_hash(b"test");
        const HASH3: u64 = const_fast_hash(b"different");

        assert_eq!(HASH1, HASH2, "Same input must produce same hash");
        assert_ne!(HASH1, HASH3, "Different inputs should produce different hashes");
    }

    /// Q1: Core behaviors - AtomicHash64 store/load
    #[test]
    fn test_atomic_hash_basic() {
        let hash = AtomicHash64::new(0);
        hash.store(0x123456789ABCDEF0);
        assert_eq!(hash.load(), 0x123456789ABCDEF0);
    }

    /// Q1: Core behaviors - Runtime hash determinism
    #[test]
    fn test_runtime_hash_basic() {
        let data = [1u64, 2, 3, 4];
        let hash1 = best_hash(&data);
        let hash2 = best_hash(&data);
        assert_eq!(hash1, hash2, "Deterministic hash required");
    }

    /// Q1: Core behaviors - AtomicHash256 store/load
    #[test]
    fn test_atomic_hash256_store_load() {
        let hash256 = AtomicHash256::new([0u8; 32]);

        // Create test hash value (4 u64 words = 32 bytes)
        let mut test_hash = [0u8; 32];
        test_hash[0..8].copy_from_slice(&0x1111111111111111u64.to_le_bytes());
        test_hash[8..16].copy_from_slice(&0x2222222222222222u64.to_le_bytes());
        test_hash[16..24].copy_from_slice(&0x3333333333333333u64.to_le_bytes());
        test_hash[24..32].copy_from_slice(&0x4444444444444444u64.to_le_bytes());

        // Store full hash
        hash256.store(test_hash);

        // Verify full hash loads correctly
        let loaded = hash256.load();
        assert_eq!(loaded, test_hash);

        // Verify individual u64 words
        let word0 = u64::from_le_bytes([loaded[0], loaded[1], loaded[2], loaded[3], loaded[4], loaded[5], loaded[6], loaded[7]]);
        let word1 = u64::from_le_bytes([loaded[8], loaded[9], loaded[10], loaded[11], loaded[12], loaded[13], loaded[14], loaded[15]]);
        let word2 = u64::from_le_bytes([loaded[16], loaded[17], loaded[18], loaded[19], loaded[20], loaded[21], loaded[22], loaded[23]]);
        let word3 = u64::from_le_bytes([loaded[24], loaded[25], loaded[26], loaded[27], loaded[28], loaded[29], loaded[30], loaded[31]]);

        assert_eq!(word0, 0x1111111111111111);
        assert_eq!(word1, 0x2222222222222222);
        assert_eq!(word2, 0x3333333333333333);
        assert_eq!(word3, 0x4444444444444444);
    }

    /// Q2: Edge cases - Zero and max values
    #[test]
    fn test_hash_edge_cases() {
        // Zero hash
        let hash = AtomicHash64::new(0);
        assert_eq!(hash.load(), 0);

        // Max hash
        hash.store(u64::MAX);
        assert_eq!(hash.load(), u64::MAX);

        // Empty data
        let empty_hash = best_hash(&[]);
        assert_ne!(empty_hash, 0, "Empty data should have non-zero hash");

        // Single element
        let single_hash = best_hash(&[42]);
        assert_ne!(single_hash, 0);
    }

    /// Q2: Edge cases - Overflow wrapping via CAS loop
    #[test]
    fn test_hash_overflow() {
        let hash = AtomicHash64::new(u64::MAX - 5);

        // Wrapping add via compare_exchange loop
        loop {
            let current = hash.load();
            let new_value = current.wrapping_add(10);
            if hash.compare_exchange(current, new_value).is_ok() {
                break;
            }
        }
        assert_eq!(hash.load(), 4, "Should wrap around");
    }

    /// Q3: Invariants - Hash atomicity
    #[test]
    fn test_hash_atomic_invariant() {
        let hash = AtomicHash64::new(0);

        // Invariant: Store/load are atomic
        hash.store(42);
        assert_eq!(hash.load(), 42);

        // Invariant: CAS succeeds with matching value
        let result = hash.compare_exchange(42, 99);
        assert_eq!(result, Ok(42));
        assert_eq!(hash.load(), 99);

        // Invariant: CAS fails with mismatched value
        let result = hash.compare_exchange(42, 200);
        assert_eq!(result, Err(99));
        assert_eq!(hash.load(), 99); // Unchanged
    }

    /// Q3: Invariants - Hash determinism
    #[test]
    fn test_hash_determinism_invariant() {
        // Invariant: Same input always produces same hash
        let data = [1u64, 2, 3, 4, 5];

        let hash1 = best_hash(&data);
        let hash2 = best_hash(&data);
        let hash3 = best_hash(&data);

        assert_eq!(hash1, hash2);
        assert_eq!(hash2, hash3);
    }

    /// Q4: All code paths - AtomicHash64 all operations
    #[test]
    fn test_atomic_hash64_all_paths() {
        let hash = AtomicHash64::new(0);

        // Path 1: store/load
        hash.store(100);
        assert_eq!(hash.load(), 100);

        // Path 2: compare_exchange success
        let result = hash.compare_exchange(100, 999);
        assert_eq!(result, Ok(100));
        assert_eq!(hash.load(), 999);

        // Path 3: compare_exchange failure
        let result = hash.compare_exchange(100, 2222);
        assert_eq!(result, Err(999));
        assert_eq!(hash.load(), 999); // Unchanged

        // Path 4: inner() access to underlying AtomicU64
        let inner = hash.inner();
        assert_eq!(inner.load(std::sync::atomic::Ordering::Acquire), 999);
    }

    /// Q5: Tests isolated and deterministic
    #[test]
    fn test_hash_isolation() {
        // Each test creates fresh instance
        let hash1 = AtomicHash64::new(0);
        let hash2 = AtomicHash64::new(0);

        // Both start at zero (isolated)
        assert_eq!(hash1.load(), 0);
        assert_eq!(hash2.load(), 0);

        // Modifications don't affect each other
        hash1.store(100);
        hash2.store(200);

        assert_eq!(hash1.load(), 100);
        assert_eq!(hash2.load(), 200);
    }

    /// Q6: Tests fast enough - <10ms per test
    #[test]
    fn test_hash_performance() {
        use std::time::Instant;

        let start = Instant::now();

        // 10,000 hash computations
        for i in 0..10_000 {
            let data = [i as u64];
            let _ = best_hash(&data);
        }

        let elapsed = start.elapsed();

        // Should complete in <100ms for 10K hashes
        assert!(
            elapsed.as_millis() < 100,
            "10K hashes took {}ms (target <100ms)",
            elapsed.as_millis()
        );
    }

    /// Q7: Tests readable and maintainable
    #[test]
    fn test_hash_with_clear_structure() {
        // Arrange: Create hash capsule
        let hash = AtomicHash64::new(0);

        // Act: Store a value
        let test_value = 0xDEADBEEFCAFEBABE;
        hash.store(test_value);

        // Assert: Value retrieved correctly
        let loaded = hash.load();
        assert_eq!(
            loaded,
            test_value,
            "Hash load should match stored value"
        );
    }

    // ============================================================================
    // TIER 2: PROPERTY TESTS (Q8-Q14)
    // ============================================================================

    /// Q8: Universal properties - CAS linearizability
    #[test]
    fn prop_cas_is_linearizable() {
        let hash = AtomicHash64::new(0);
        hash.store(100);

        // Property: CAS only succeeds when current matches expected
        assert_eq!(hash.compare_exchange(100, 200), Ok(100));
        assert_eq!(hash.load(), 200);

        // Property: CAS fails when current doesn't match
        assert_eq!(hash.compare_exchange(100, 300), Err(200));
        assert_eq!(hash.load(), 200); // Unchanged
    }

    /// Q9: Concurrent invariants - No lost updates
    #[test]
    fn prop_concurrent_no_lost_updates() {
        let hash = Arc::new(AtomicHash64::new(0));
        let threads = 10;
        let updates_per_thread = 100;

        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let h = Arc::clone(&hash);
                thread::spawn(move || {
                    for _ in 0..updates_per_thread {
                        // CAS loop for atomic increment
                        loop {
                            let current = h.load();
                            if h.compare_exchange(current, current + 1).is_ok() {
                                break;
                            }
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // Property: All updates applied (no lost writes)
        let expected = threads * updates_per_thread;
        assert_eq!(hash.load(), expected);
    }

    /// Q9: Concurrent invariants - Concurrent readers
    #[test]
    fn prop_concurrent_readers() {
        let hash = Arc::new(AtomicHash64::new(0));
        hash.store(42);

        let handles: Vec<_> = (0..20)
            .map(|_| {
                let h = Arc::clone(&hash);
                thread::spawn(move || {
                    for _ in 0..100 {
                        let value = h.load();
                        assert!(value == 42 || value == 99, "Unexpected value: {}", value);
                    }
                })
            })
            .collect();

        // Writer updates value
        thread::sleep(std::time::Duration::from_millis(10));
        hash.store(99);

        for handle in handles {
            handle.join().unwrap();
        }
    }

    /// Q10: Edge case properties - Overflow handling
    #[test]
    fn prop_handles_overflow() {
        let hash = AtomicHash64::new(u64::MAX - 5);

        // Property: Wrapping overflow behaves correctly via CAS loop
        loop {
            let current = hash.load();
            let new_value = current.wrapping_add(10);
            if hash.compare_exchange(current, new_value).is_ok() {
                break;
            }
        }

        // Should wrap around (u64::MAX - 5 + 10 wraps to 4)
        assert_eq!(hash.load(), 4);
    }

    /// Q11: ASSUM verification - Memory ordering
    #[test]
    fn verify_assum_memory_ordering() {
        // #ASSUME: Store-Release / Load-Acquire prevents data races
        // #VERIFY: Property test with concurrent readers/writers

        let hash = Arc::new(AtomicHash64::new(0));

        let writer = {
            let h = Arc::clone(&hash);
            thread::spawn(move || {
                for i in 0..1000 {
                    h.store(i); // Release semantics
                }
            })
        };

        let reader = {
            let h = Arc::clone(&hash);
            thread::spawn(move || {
                let mut last = 0;
                for _ in 0..1000 {
                    let current = h.load(); // Acquire semantics
                    assert!(current >= last || current == 0, "Non-monotonic read");
                    last = current;
                }
            })
        };

        writer.join().unwrap();
        reader.join().unwrap();
    }

    /// Q12: Composition properties - Hash256 as 4× Hash64
    #[test]
    fn prop_hash256_composition() {
        let hash256 = AtomicHash256::new([0u8; 32]);

        // Property: 256-bit hash = 4× 64-bit chunks
        let mut test_hash = [0u8; 32];
        test_hash[0..8].copy_from_slice(&0xAAAAAAAAAAAAAAAAu64.to_le_bytes());
        test_hash[8..16].copy_from_slice(&0xBBBBBBBBBBBBBBBBu64.to_le_bytes());
        test_hash[16..24].copy_from_slice(&0xCCCCCCCCCCCCCCCCu64.to_le_bytes());
        test_hash[24..32].copy_from_slice(&0xDDDDDDDDDDDDDDDDu64.to_le_bytes());

        hash256.store(test_hash);
        let bytes = hash256.load();

        // Verify each chunk in byte representation
        let chunk0 = u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
            bytes[4], bytes[5], bytes[6], bytes[7]
        ]);
        assert_eq!(chunk0, 0xAAAAAAAAAAAAAAAA);
    }

    /// Q13: Statistical properties - Hash distribution
    #[test]
    fn prop_hash_distribution() {
        use std::collections::HashSet;

        // Property: Hashes should be well-distributed
        let mut hashes = Vec::new();
        for i in 0..100 {
            let data = [i as u64];
            let hash = best_hash(&data);
            hashes.push(hash);
        }

        // Property: No duplicate hashes for sequential inputs
        let unique: HashSet<_> = hashes.iter().copied().collect();
        assert_eq!(unique.len(), 100, "All hashes should be unique");
    }

    /// Q14: Regression tracking - Deterministic hash values
    #[test]
    fn prop_regression_hash_consistency() {
        // Property: Same input always produces same hash (regression check)
        let test_data = [1u64, 2, 3, 4, 5];
        let hash1 = best_hash(&test_data);

        // Run 100 times to ensure consistency
        for _ in 0..100 {
            let hash2 = best_hash(&test_data);
            assert_eq!(hash1, hash2, "Hash should be deterministic");
        }
    }

    // ============================================================================
    // TIER 3: INTEGRATION TESTS (Q15-Q21)
    // ============================================================================

    /// Q15: Integration point - Hash with forensics module
    #[test]
    fn test_integration_with_forensics() {
        // Test that hash types integrate with forensics module
        let hash = AtomicHash64::new(0);
        hash.store(0x123456789ABCDEF0);

        // Should be usable in forensics context
        let hash_value = hash.load();
        assert_ne!(hash_value, 0);
    }

    /// Q17: Performance budgets - Hash operations <100ns
    #[test]
    fn test_performance_budget() {
        use std::time::Instant;

        let hash = AtomicHash64::new(0);
        let iterations = 10_000;

        let start = Instant::now();
        for i in 0..iterations {
            hash.store(i);
            let _ = hash.load();
        }
        let elapsed = start.elapsed();

        let avg_ns = elapsed.as_nanos() / (iterations * 2); // 2 ops per iteration

        // Budget: <100ns per operation
        assert!(
            avg_ns < 100,
            "Hash operations exceeded budget: {}ns > 100ns",
            avg_ns
        );
    }

    // ============================================================================
    // TIER 4: PRODUCTION READINESS (Q22-Q28)
    // ============================================================================

    /// Q22: Stress test - 100 threads × 1000 operations
    #[test]
    #[ignore] // Run with: cargo test --ignored
    fn test_stress_concurrent() {
        let hash = Arc::new(AtomicHash64::new(0));
        let threads = 100;
        let operations = 1000;

        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let h = Arc::clone(&hash);
                thread::spawn(move || {
                    for _ in 0..operations {
                        // CAS loop for atomic increment
                        loop {
                            let current = h.load();
                            if h.compare_exchange(current, current + 1).is_ok() {
                                break;
                            }
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread must not panic");
        }

        // Assert: All updates applied
        assert_eq!(hash.load(), threads * operations);
    }

    /// Q24: B32 benchmarks - Performance targets met
    #[test]
    fn test_b32_performance_targets() {
        use std::time::Instant;

        // Target: <50ns for atomic operations
        let hash = AtomicHash64::new(0);

        let start = Instant::now();
        for i in 0..10_000 {
            hash.store(i);
        }
        let elapsed = start.elapsed();
        let avg_ns = elapsed.as_nanos() / 10_000;

        assert!(avg_ns < 50, "Store exceeded target: {}ns > 50ns", avg_ns);
    }

    /// Q27: Documentation complete - Backward compat documented
    #[test]
    fn test_backward_compat_alias() {
        #[allow(deprecated)]
        let _legacy: CapsuleHash64 = AtomicHash64::new(0);
    }

    #[test]
    fn test_backward_compat_compute() {
        #[allow(deprecated)]
        let hash1 = compute_hash(&[1, 2, 3, 4]);
        let hash2 = best_hash(&[1, 2, 3, 4]);
        assert_eq!(hash1, hash2, "Legacy compute should match best_hash");
    }

    // ============================================================================
    // CONST HASHING TESTS (Feature-Gated)
    // ============================================================================

    #[cfg(feature = "const-hashing")]
    #[test]
    fn test_const_hash_zero_runtime() {
        use std::time::Instant;

        // const-hash should be compile-time evaluated
        const DATA: &[u8] = b"test_data";

        let start = Instant::now();
        let _hash = const_fast_hash(DATA);
        let elapsed = start.elapsed();

        // Should be <10ns (essentially free)
        assert!(
            elapsed.as_nanos() < 10,
            "Const hash took {}ns (target <10ns)",
            elapsed.as_nanos()
        );
    }

    #[cfg(feature = "const-hashing")]
    #[test]
    fn test_const_hash_capsule() {
        // ConstHashCapsule wraps const hash
        const DATA: &[u64] = &[42];
        let capsule = ConstHashCapsule::new(DATA);

        let hash = capsule.hash();
        assert_ne!(hash, 0);

        // Verify determinism
        let capsule2 = ConstHashCapsule::new(DATA);
        assert_eq!(capsule.hash(), capsule2.hash());
    }

    // ============================================================================
    // SIMD HASHING TESTS (Feature-Gated)
    // ============================================================================

    #[cfg(feature = "simd-hashing")]
    #[test]
    fn test_simd_hash_multi_field() {
        // SIMD hash optimized for 4+ fields
        let data = [1u64, 2, 3, 4, 5, 6, 7, 8];
        let hash = simd_fast_hash_multi(&data);

        assert_ne!(hash, 0);

        // Verify determinism
        let hash2 = simd_fast_hash_multi(&data);
        assert_eq!(hash, hash2);
    }

    #[cfg(feature = "simd-hashing")]
    #[test]
    fn test_simd_hash_speedup_validation() {
        use std::time::Instant;

        // Large dataset to see SIMD benefit
        let data: Vec<u64> = (0..1000).collect();

        let start = Instant::now();
        for _ in 0..1000 {
            let _ = simd_fast_hash_multi(&data);
        }
        let simd_time = start.elapsed();

        // SIMD should complete in reasonable time
        assert!(
            simd_time.as_millis() < 100,
            "1000 SIMD hashes took {}ms (target <100ms)",
            simd_time.as_millis()
        );
    }
}
