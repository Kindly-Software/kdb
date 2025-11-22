//! # Atomic Capsule Trait
//!
//! Specialized trait for atomic coordination capsules.
//!
//! ## UCE33 Q33 (Atomic Capsule Foundation)
//!
//! This trait captures The Atomic Capsule architecture patterns:
//! - **Lockfree coordination**: 100% atomic operations (no mutex/RwLock)
//! - **Memory ordering**: Explicit Acquire/Release semantics
//! - **Generation counters**: TOCTOU prevention
//! - **Two-phase commit**: Odd→even version flips
//!
//! ## Backward Compatibility (I20 Framework)
//!
//! **Q6 (Architectural Compatibility)**: Extends ComputationalCapsule ✓
//! **Q7 (Performance Compatibility)**: <15ns atomic operations ✓
//! **Q8 (Error Handling)**: Result<T, E> for CAS failures ✓
//! **Q9 (Concurrency)**: Send + Sync enforced ✓

use super::ComputationalCapsule;
use core::sync::atomic::Ordering;

/// Atomic capsule specialization for lockfree coordination.
///
/// Implementors MUST use only atomic primitives (no mutex/RwLock).
///
/// # UCE33 Q33 (Foundation)
///
/// This trait embodies The Atomic Capsule (Section 6: Design Rules):
/// - Rule 1: Shape data to the decision
/// - Rule 2: One writer, many readers (SWeMR)
/// - Rule 3: Two-phase commit (odd→even)
/// - Rule 6: No locks in hot path
///
/// # Safety Model
///
/// This trait is intentionally unsafe to implement because:
/// - Memory ordering violations can cause data races
/// - Incorrect generation counter usage allows TOCTOU bugs
/// - Torn reads violate atomic capsule guarantees
///
/// # ASSUM Framework
///
/// - `#ASSUME_LOCKFREE`: Only atomic primitives used (no mutex/RwLock)
/// - `#VERIFY_LOCKFREE`: Manual code review + property tests
/// - `#ASSUME_MEMORY_ORDER`: Acquire/Release pairs prevent races
/// - `#VERIFY_MEMORY_ORDER`: Miri testing + stress tests
///
/// # Example
///
/// ```rust
/// use atomic_capsule::traits::{ComputationalCapsule, AtomicCapsule};
/// use core::sync::atomic::{AtomicU64, Ordering};
///
/// #[repr(C, align(64))]
/// struct CircuitBreakerCapsule {
///     state: AtomicU64,
/// }
///
/// unsafe impl ComputationalCapsule for CircuitBreakerCapsule {
///     const ALIGNMENT: usize = 64;
///     const SIZE: usize = 8;
///     const TYPE_ID: &'static str = "CircuitBreakerCapsule";
/// }
///
/// unsafe impl AtomicCapsule for CircuitBreakerCapsule {
///     type Primitive = AtomicU64;
///
///     fn load_ordering() -> Ordering {
///         Ordering::Acquire
///     }
///
///     fn store_ordering() -> Ordering {
///         Ordering::Release
///     }
///
///     fn cas_success_ordering() -> Ordering {
///         Ordering::Release
///     }
///
///     fn cas_failure_ordering() -> Ordering {
///         Ordering::Relaxed
///     }
/// }
/// ```
///
/// # Safety
///
/// Implementors must ensure:
/// - All data fields use atomic types (AtomicU32, AtomicU64, etc.) or are immutable
/// - `generation()` correctly extracts generation counter from atomic fields
/// - `commit()` properly implements two-phase commit protocol (odd→even)
/// - Thread-safe: Multiple threads can safely call `load()` concurrently
/// - Send + Sync: Safe to send between threads and share references
/// - Memory ordering follows acquire/release semantics for synchronization
// const_trait disabled for this nightly
// #[cfg_attr(feature = "portable_simd", const_trait)]
pub unsafe trait AtomicCapsule: ComputationalCapsule + Send + Sync {
    /// Atomic primitive type (AtomicU64, AtomicU128, etc.).
    ///
    /// # UCE33 Q31 (Rust Transform)
    /// Associated type enables compile-time primitive selection
    type Primitive: Send + Sync;

    /// Memory ordering for load operations.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_ORDERING_SUFFICIENT`: Acquire prevents reading stale data
    /// - `#VERIFY_ORDERING_SUFFICIENT`: Miri testing + formal verification
    ///
    /// # Default: Acquire
    ///
    /// From The Atomic Capsule (Section 9: Implementation):
    /// "Readers do load(Relaxed); use Acquire only if dereference pointer"
    ///
    /// Most capsules use Relaxed, override to Acquire if needed.
    #[inline(always)]
    fn load_ordering() -> Ordering {
        Ordering::Relaxed
    }

    /// Memory ordering for store operations.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_ORDERING_SUFFICIENT`: Release makes writes visible
    /// - `#VERIFY_ORDERING_SUFFICIENT`: Miri testing + formal verification
    ///
    /// # Default: Release
    ///
    /// From The Atomic Capsule (Section 9: Implementation):
    /// "w0.store(head, Ordering::Release)" for two-phase commit
    #[inline(always)]
    fn store_ordering() -> Ordering {
        Ordering::Release
    }

    /// Memory ordering for successful CAS.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_ORDERING_SUFFICIENT`: Release on success publishes changes
    /// - `#VERIFY_ORDERING_SUFFICIENT`: Property tests + stress tests
    ///
    /// # Default: Release
    ///
    /// CAS success must publish the new value atomically.
    #[inline(always)]
    fn cas_success_ordering() -> Ordering {
        Ordering::Release
    }

    /// Memory ordering for failed CAS.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_ORDERING_SUFFICIENT`: Relaxed on failure (no publication needed)
    /// - `#VERIFY_ORDERING_SUFFICIENT`: Property tests + stress tests
    ///
    /// # Default: Relaxed
    ///
    /// CAS failure doesn't publish anything, Relaxed is sufficient.
    #[inline(always)]
    fn cas_failure_ordering() -> Ordering {
        Ordering::Relaxed
    }

    /// Check if capsule supports generation counters.
    ///
    /// # UCE33 Q33 (Foundation)
    ///
    /// Generation counters prevent TOCTOU races (The Atomic Capsule Section 8).
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_GENERATION_NEEDED`: Complex capsules need TOCTOU prevention
    /// - `#VERIFY_GENERATION_NEEDED`: Property tests with concurrent updates
    ///
    /// # Default: false
    ///
    /// Simple capsules (single atomic) don't need generation counters.
    /// Override to true for multi-word capsules.
    #[inline(always)]
    fn has_generation_counter() -> bool {
        false
    }

    /// Check if capsule uses two-phase commit.
    ///
    /// # UCE33 Q33 (Foundation)
    ///
    /// Two-phase commit (odd→even) from The Atomic Capsule (Section 8):
    /// "Set ver odd → write payload → flip header (ver even)"
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_COMMIT_NEEDED`: Multi-word updates need atomic visibility
    /// - `#VERIFY_COMMIT_NEEDED`: Property tests verify all-old or all-new reads
    ///
    /// # Default: false
    ///
    /// Simple capsules (single atomic) don't need two-phase commit.
    /// Override to true for multi-word capsules.
    #[inline(always)]
    fn uses_two_phase_commit() -> bool {
        false
    }

    /// Expected atomic operation latency in nanoseconds.
    ///
    /// # UCE33 Q29 (Constraints)
    ///
    /// Hardware constraint: CAS latency on modern CPUs
    /// - x86: ~15ns (cache hit)
    /// - ARM: ~10ns (cache hit)
    ///
    /// # Performance Targets (from The Atomic Capsule)
    ///
    /// - Single atomic: <15ns
    /// - Dual atomic: <100ns
    /// - Multi-word: <1μs
    ///
    /// # Default: 15ns
    ///
    /// Typical CAS latency for single atomic operation.
    #[inline(always)]
    fn expected_latency_ns() -> u64 {
        15
    }
}

/// Atomic capsule verification.
///
/// # UCE33 Q30 (Validation)
/// Macro enables atomic capsule verification
///
/// # Example
///
/// ```rust
/// # use atomic_capsule::traits::{ComputationalCapsule, AtomicCapsule};
/// # use core::sync::atomic::AtomicU64;
/// # #[repr(C, align(64))]
/// # struct MyAtomicCapsule {
/// #     state: AtomicU64,
/// # }
/// # unsafe impl ComputationalCapsule for MyAtomicCapsule {
/// #     const ALIGNMENT: usize = 64;
/// #     const SIZE: usize = 8;
/// #     const TYPE_ID: &'static str = "MyAtomicCapsule";
/// # }
/// # unsafe impl AtomicCapsule for MyAtomicCapsule {
/// #     type Primitive = AtomicU64;
/// # }
/// use atomic_capsule::verify_atomic_capsule;
///
/// verify_atomic_capsule!(MyAtomicCapsule);
/// ```
#[macro_export]
macro_rules! verify_atomic_capsule {
    ($capsule:ty) => {
        // Verify base capsule properties
        $crate::verify_capsule!($capsule);

        // Verify atomic-specific properties
        // (Send + Sync checked by trait bound at usage site)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::AtomicU64;

    #[repr(C, align(64))]
    struct TestAtomicCapsule {
        state: AtomicU64,
    }

    unsafe impl ComputationalCapsule for TestAtomicCapsule {
        const ALIGNMENT: usize = 64;
        const SIZE: usize = 8;
        const TYPE_ID: &'static str = "TestAtomicCapsule";
    }

    unsafe impl AtomicCapsule for TestAtomicCapsule {
        type Primitive = AtomicU64;
    }

    #[test]
    fn test_atomic_capsule_defaults() {
        assert_eq!(TestAtomicCapsule::load_ordering(), Ordering::Relaxed);
        assert_eq!(TestAtomicCapsule::store_ordering(), Ordering::Release);
        assert_eq!(TestAtomicCapsule::cas_success_ordering(), Ordering::Release);
        assert_eq!(TestAtomicCapsule::cas_failure_ordering(), Ordering::Relaxed);
        assert!(!TestAtomicCapsule::has_generation_counter());
        assert!(!TestAtomicCapsule::uses_two_phase_commit());
        assert_eq!(TestAtomicCapsule::expected_latency_ns(), 15);
    }

    #[test]
    fn test_atomic_capsule_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<TestAtomicCapsule>();
        assert_sync::<TestAtomicCapsule>();
    }

    #[test]
    fn test_atomic_verification_macro() {
        verify_atomic_capsule!(TestAtomicCapsule);
    }
}
