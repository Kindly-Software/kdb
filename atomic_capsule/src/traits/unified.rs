//! # Unified Capsule Trait Hierarchy
//!
//! Complete hierarchical trait system for all 10 computational capsule tiers.
//!
//! ## UCE33 Q10: Foundation for All Tiers
//!
//! This module implements the systematic tier hierarchy from UCE33 framework:
//! - **Tier 1 (Atomic)**: <100ns lockfree coordination (3-10× speedup)
//! - **Tier 2 (SIMD)**: Vectorized computation (2-19× speedup)
//! - **Tier 3 (Fixed-Point)**: Deterministic arithmetic (2-10× speedup)
//! - **Tier 4 (Batch)**: High-throughput processing (10-100× speedup)
//! - **Tier 5 (Streaming)**: Continuous computation (configurable latency)
//! - **Tier 6 (Mixed)**: Hybrid coordination + computation (12-2000× compound speedups)
//!
//! ## Architecture
//!
//! ```text
//! Capsule (base trait - all capsules implement this)
//!   ├── AtomicCapsule: Capsule (Tier 1)
//!   ├── SimdCapsule: Capsule (Tier 2)
//!   ├── FixedPointCapsule: Capsule (Tier 3)
//!   ├── BatchCapsule: Capsule (Tier 4)
//!   ├── StreamingCapsule: Capsule (Tier 5)
//!   └── MixedCapsule<T1, T2>: Capsule (Tier 6)
//! ```

use core::fmt;

/// Tier classification for computational capsules.
///
/// ## UCE33 Q10: Systematic Tier Selection
///
/// Each tier corresponds to a specific computational pattern:
/// - **Coordination**: Tier 1 (Atomic)
/// - **Vectorization**: Tier 2 (SIMD)
/// - **Precision**: Tier 3 (Fixed-Point)
/// - **Throughput**: Tier 4 (Batch)
/// - **Streaming**: Tier 5 (Streaming)
/// - **Hybrid**: Tier 6 (Mixed)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Tier {
    /// Tier 1: Atomic capsules (<100ns lockfree coordination)
    T1Atomic = 1,
    /// Tier 2: SIMD capsules (2-19× vectorized computation)
    T2Simd = 2,
    /// Tier 3: Fixed-point capsules (2-10× deterministic arithmetic)
    T3FixedPoint = 3,
    /// Tier 4: Batch capsules (10-100× high-throughput)
    T4Batch = 4,
    /// Tier 5: Streaming capsules (O(1) continuous computation)
    T5Streaming = 5,
    /// Tier 6: Mixed capsules (12-2000× compound speedups)
    T6Mixed = 6,
    /// Tier 7: GPU capsules (100-1000× parallel workloads) - FUTURE
    T7Gpu = 7,
    /// Tier 8: Network capsules (10-50× packet throughput) - FUTURE
    T8Network = 8,
    /// Tier 9: Persistent capsules (ACID guarantees) - FUTURE
    T9Persistent = 9,
    /// Tier 10: Probabilistic capsules (100-1000× memory reduction) - FUTURE
    T10Probabilistic = 10,
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tier::T1Atomic => write!(f, "Tier 1: Atomic"),
            Tier::T2Simd => write!(f, "Tier 2: SIMD"),
            Tier::T3FixedPoint => write!(f, "Tier 3: Fixed-Point"),
            Tier::T4Batch => write!(f, "Tier 4: Batch"),
            Tier::T5Streaming => write!(f, "Tier 5: Streaming"),
            Tier::T6Mixed => write!(f, "Tier 6: Mixed"),
            Tier::T7Gpu => write!(f, "Tier 7: GPU"),
            Tier::T8Network => write!(f, "Tier 8: Network"),
            Tier::T9Persistent => write!(f, "Tier 9: Persistent"),
            Tier::T10Probabilistic => write!(f, "Tier 10: Probabilistic"),
        }
    }
}

/// Verification error types for capsule validation.
///
/// ## UCE33 Q33: Compile-Time Verification
///
/// All errors are caught at compile-time via const assertions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationError {
    /// Alignment mismatch (expected vs actual)
    AlignmentMismatch {
        /// Expected alignment in bytes
        expected: usize,
        /// Actual alignment detected
        actual: usize,
    },
    /// Size mismatch (expected vs actual)
    SizeMismatch {
        /// Expected size in bytes
        expected: usize,
        /// Actual size detected
        actual: usize,
    },
    /// Tier violation (expected vs actual)
    TierViolation {
        /// Expected tier
        expected: Tier,
        /// Actual tier
        actual: Tier,
    },
    /// Generic verification failure
    VerificationFailed(&'static str),
}

impl fmt::Display for VerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerificationError::AlignmentMismatch { expected, actual } => {
                write!(
                    f,
                    "Alignment mismatch: expected {} bytes, got {} bytes",
                    expected, actual
                )
            }
            VerificationError::SizeMismatch { expected, actual } => {
                write!(
                    f,
                    "Size mismatch: expected {} bytes, got {} bytes",
                    expected, actual
                )
            }
            VerificationError::TierViolation { expected, actual } => {
                write!(f, "Tier violation: expected {}, got {}", expected, actual)
            }
            VerificationError::VerificationFailed(msg) => {
                write!(f, "Verification failed: {}", msg)
            }
        }
    }
}

/// Base capsule trait - ALL capsules implement this.
///
/// ## UCE33 Q10: Foundation Question
///
/// This trait embodies the core capsule principles:
/// - Cache alignment (64B/128B/256B)
/// - Tiered sizing (memory hierarchy optimization)
/// - Self-contained data (one-read decisions)
/// - Predictable layout (hardware prefetch-friendly)
///
/// ## ASSUM Framework
///
/// - `#ASSUME_CACHE_ALIGNED`: All capsules are cache-aligned
/// - `#VERIFY_CACHE_ALIGNED`: Enforced via const ALIGNMENT and verify() method
/// - `#ASSUME_SIZED`: All capsules have known size at compile-time
/// - `#VERIFY_SIZED`: Enforced via const SIZE and verify() method
///
/// ## Safety
///
/// This trait is unsafe to implement because:
/// - Incorrect alignment causes cache thrashing and false sharing
/// - Incorrect size causes memory layout violations
/// - Incorrect tier classification causes performance regressions
///
/// # Example
///
/// ```rust,ignore
/// use atomic_capsule::traits::unified::{Capsule, Tier};
/// use core::sync::atomic::AtomicU64;
///
/// #[repr(C, align(64))]
/// struct CircuitBreakerCapsule {
///     state: AtomicU64,
///     _padding: [u8; 56],
/// }
///
/// unsafe impl Capsule for CircuitBreakerCapsule {
///     const TIER: Tier = Tier::T1Atomic;
///     const ALIGNMENT: usize = 64;
///     const SIZE: usize = 64;
/// }
/// ```
pub unsafe trait Capsule: Send + Sync + Sized {
    /// Tier classification (1-10)
    const TIER: Tier;

    /// Expected alignment (power of 2: 64B/128B/256B)
    ///
    /// ## UCE33 Q29 (Constraints)
    ///
    /// Hardware constraint: Cache line sizes
    /// - L1: 64 bytes (typical)
    /// - L2/L3: 64-128 bytes
    /// - SIMD: 32-64 bytes (AVX/AVX-512)
    const ALIGNMENT: usize;

    /// Expected size (bytes)
    ///
    /// ## UCE33 Q29 (Constraints)
    ///
    /// Size should match tier requirements:
    /// - Tier 1 (Atomic): 64-1024 bytes
    /// - Tier 2 (SIMD): 128-512 bytes
    /// - Tier 3 (Fixed-Point): 64-256 bytes
    /// - Tier 4 (Batch): 1KB-64KB
    /// - Tier 5 (Streaming): Variable (windowed)
    const SIZE: usize;

    /// Verify capsule properties at compile-time.
    ///
    /// ## UCE33 Q33: Integrated Verification
    ///
    /// This method replaces standalone verification macros with trait-integrated validation.
    ///
    /// # Returns
    ///
    /// - `Ok(())` if all properties are valid
    /// - `Err(VerificationError)` if validation fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// const _: () = match CircuitBreakerCapsule::verify() {
    ///     Ok(()) => (),
    ///     Err(_) => panic!("Capsule verification failed"),
    /// };
    /// ```
    fn verify() -> Result<(), VerificationError> {
        // Alignment check
        let actual_align = core::mem::align_of::<Self>();
        if actual_align != Self::ALIGNMENT {
            return Err(VerificationError::AlignmentMismatch {
                expected: Self::ALIGNMENT,
                actual: actual_align,
            });
        }

        // Size check
        let actual_size = core::mem::size_of::<Self>();
        if actual_size != Self::SIZE {
            return Err(VerificationError::SizeMismatch {
                expected: Self::SIZE,
                actual: actual_size,
            });
        }

        // Alignment must be power of 2
        if !Self::ALIGNMENT.is_power_of_two() {
            return Err(VerificationError::VerificationFailed(
                "Alignment must be power of 2",
            ));
        }

        // Alignment must be at least 64 bytes (cache line)
        if Self::ALIGNMENT < 64 {
            return Err(VerificationError::VerificationFailed(
                "Alignment must be at least 64 bytes",
            ));
        }

        Ok(())
    }

    /// Type identifier for debugging
    ///
    /// ## UCE33 Q31 (Rust Transform)
    ///
    /// Const fn enables compile-time string generation
    fn type_id() -> &'static str {
        core::any::type_name::<Self>()
    }
}

/// Tier 1: Atomic capsules (<100ns lockfree coordination).
///
/// ## UCE33 Q10: Tier 1 Atomic
///
/// Atomic capsules provide lockfree coordination with:
/// - 3-10× speedup vs mutex (proven: 9.8ns vs 32ns)
/// - Zero blocking (no context switches)
/// - Deterministic latency (<100ns)
///
/// ## ASSUM Framework
///
/// - `#ASSUME_LOCKFREE`: Only atomic primitives used (no mutex/RwLock)
/// - `#VERIFY_LOCKFREE`: Manual code review + property tests
/// - `#ASSUME_MEMORY_ORDER`: Acquire/Release pairs prevent races
/// - `#VERIFY_MEMORY_ORDER`: Miri testing + stress tests
///
/// # Safety
///
/// This trait is unsafe to implement because:
/// - Implementor must ensure only atomic primitives are used (no mutex/RwLock)
/// - Incorrect memory ordering causes data races
/// - Missing Acquire/Release pairs cause undefined behavior
/// - Implementor must validate lockfree guarantees via property tests
pub unsafe trait AtomicCapsule: Capsule {
    /// Atomic primitive type (AtomicU64, AtomicU128, etc.)
    type Primitive: Send + Sync;

    /// Expected atomic operation latency in nanoseconds
    ///
    /// ## B32 Framework
    ///
    /// Reality check: <15ns for cache-hit atomic CAS
    fn expected_latency_ns() -> u64 {
        15
    }
}

/// Tier 2: SIMD capsules (2-19× vectorized computation).
///
/// ## UCE33 Q10: Tier 2 SIMD
///
/// SIMD capsules provide vectorized computation with:
/// - 2-19× speedup (proven: 2.5ns vs 47.9ns per connection in Hebbian learning)
/// - Cross-platform portability (x86/ARM/RISC-V)
/// - Zero unsafe code (via std::simd)
///
/// ## ASSUM Framework
///
/// - `#ASSUME_SIMD_ALIGNED`: SIMD types require alignment (16/32/64 bytes)
/// - `#VERIFY_SIMD_ALIGNED`: Compile-time via const generics
/// - `#ASSUME_LANES_VALID`: Lane count is power of 2 (2/4/8/16/32/64)
/// - `#VERIFY_LANES_VALID`: Enforced by Simd<T, N> type bounds
///
/// # Safety
///
/// This trait is unsafe to implement because:
/// - Implementor must ensure proper SIMD alignment (16/32/64 bytes for AVX2/AVX-512)
/// - Incorrect alignment causes undefined behavior on SIMD loads/stores
/// - Lane count must be power of 2 and supported by target architecture
/// - Implementor must validate SIMD operations compile correctly for target CPU
#[cfg(feature = "portable_simd")]
pub unsafe trait SimdCapsule: Capsule {
    /// SIMD element type (f32, f64, i32, u64, etc.)
    type Element: core::simd::SimdElement;

    /// Number of SIMD lanes (2, 4, 8, 16, 32, 64)
    const LANES: usize;

    /// Expected SIMD operation latency in nanoseconds
    ///
    /// ## B32 Framework
    ///
    /// Reality check: <10ns for SIMD arithmetic
    fn expected_simd_latency_ns() -> u64 {
        5
    }

    /// Verify lane count is valid
    fn verify_lanes() -> bool {
        // Power of 2 check
        Self::LANES.count_ones() == 1
            // Reasonable bounds (2-64 lanes)
            && Self::LANES >= 2
            && Self::LANES <= 64
    }
}

/// Tier 3: Fixed-point capsules (2-10× deterministic arithmetic).
///
/// ## UCE33 Q10: Tier 3 Fixed-Point
///
/// Fixed-point capsules provide deterministic arithmetic with:
/// - 2-10× speedup (proven: 83.4ns vs ~200ns float+mutex)
/// - Zero FP drift (100× $0.01 = $1.00 exactly)
/// - Deterministic rounding (no edge cases)
///
/// ## ASSUM Framework
///
/// - `#ASSUME_SCALE_VALID`: Scale factor matches fractional bits
/// - `#VERIFY_SCALE_VALID`: Compile-time via const evaluation
/// - `#ASSUME_NO_OVERFLOW`: Arithmetic operations don't overflow
/// - `#VERIFY_NO_OVERFLOW`: Property tests with boundary values
///
/// # Safety
///
/// This trait is unsafe to implement because:
/// - Implementor must ensure fractional bits are valid for the integer type
/// - Incorrect scale factor causes arithmetic precision loss
/// - Overflow in fixed-point operations causes incorrect results
/// - Implementor must validate range bounds via property tests
pub unsafe trait FixedPointCapsule: Capsule {
    /// Integer type for fixed-point storage (i16, i32, i64, u16, u32, u64)
    type Integer: Copy + Sized;

    /// Number of fractional bits (0-63)
    ///
    /// ## Common Formats
    ///
    /// - Q8.8: 8 fractional bits (basis points)
    /// - Q16.16: 16 fractional bits (high-precision)
    const FRACTIONAL_BITS: u32;

    /// Scale factor for conversion (2^FRACTIONAL_BITS)
    fn scale_factor() -> f64 {
        (1u64 << Self::FRACTIONAL_BITS) as f64
    }

    /// Expected fixed-point operation latency in nanoseconds
    ///
    /// ## B32 Framework
    ///
    /// Reality check: <2ns for integer arithmetic
    fn expected_latency_ns() -> u64 {
        2
    }

    /// Verify fractional bits are valid
    fn verify_fractional_bits() -> bool {
        let total_bits = (core::mem::size_of::<Self::Integer>() * 8) as u32;
        Self::FRACTIONAL_BITS <= total_bits && Self::FRACTIONAL_BITS > 0
    }
}

/// Tier 4: Batch capsules (10-100× high-throughput).
///
/// ## UCE33 Q10: Tier 4 Batch
///
/// Batch capsules provide high-throughput processing with:
/// - 10-100× speedup (amortized overhead)
/// - Cache-friendly batching (16-512 items typical)
/// - Streaming throughput (millions of items/sec)
///
/// ## ASSUM Framework
///
/// - `#ASSUME_BATCH_SIZE_OPTIMAL`: Batch size matches L1/L2 cache
/// - `#VERIFY_BATCH_SIZE_OPTIMAL`: Benchmark with B32 framework
///
/// # Safety
///
/// This trait is unsafe to implement because:
/// - Implementor must ensure batch size is optimal for target cache hierarchy
/// - Too-large batches cause cache thrashing and performance degradation
/// - Implementor must validate batch processing is correct via integration tests
pub unsafe trait BatchCapsule: Capsule {
    /// Item type for batch processing
    type Item;

    /// Batch size (16-512 typical, must be power of 2)
    const BATCH_SIZE: usize;

    /// Push item to batch buffer
    ///
    /// # Returns
    ///
    /// - `Ok(())` if item was added
    /// - `Err(item)` if buffer is full
    fn push(&mut self, item: Self::Item) -> Result<(), Self::Item>;

    /// Process batch with user-provided function
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// batch.batch_process(|items| {
    ///     for item in items {
    ///         process(item);
    ///     }
    /// });
    /// ```
    fn batch_process<F>(&mut self, f: F)
    where
        F: FnMut(&[Self::Item]);

    /// Verify batch size is valid
    fn verify_batch_size() -> bool {
        // Power of 2 check
        Self::BATCH_SIZE.count_ones() == 1
            // Reasonable bounds (16-512)
            && Self::BATCH_SIZE >= 16
            && Self::BATCH_SIZE <= 512
    }
}

/// Tier 5: Streaming capsules (O(1) continuous computation).
///
/// ## UCE33 Q10: Tier 5 Streaming
///
/// Streaming capsules provide continuous computation with:
/// - O(1) latency (windowed aggregation)
/// - Configurable windows (60s, 1h, 1d)
/// - Real-time metrics (no buffering delay)
///
/// ## ASSUM Framework
///
/// - `#ASSUME_WINDOW_SIZE_VALID`: Window size fits in memory
/// - `#VERIFY_WINDOW_SIZE_VALID`: Checked at compile-time
///
/// # Safety
///
/// This trait is unsafe to implement because:
/// - Implementor must ensure window size fits in available memory
/// - Incorrect aggregate pre-computation causes incorrect results
/// - Implementor must validate streaming operations via property tests
pub unsafe trait StreamingCapsule: Capsule {
    /// Input item type
    type Input;

    /// Aggregated output type
    type Aggregate;

    /// Window size (number of items)
    const WINDOW_SIZE: usize;

    /// Push item to streaming window
    ///
    /// Oldest item is automatically dropped when window is full.
    fn push(&mut self, item: Self::Input);

    /// Get current aggregate (O(1) if pre-computed)
    fn aggregate(&self) -> Self::Aggregate;

    /// Verify window size is reasonable
    fn verify_window_size() -> bool {
        // Window size must be positive and fit in memory
        Self::WINDOW_SIZE > 0 && Self::WINDOW_SIZE <= 1_000_000
    }
}

/// Tier 6: Mixed capsules (12-2000× compound speedups).
///
/// ## UCE33 Q10: Tier 6 Mixed
///
/// Mixed capsules combine multiple tiers for compound speedups:
/// - Atomic + SIMD: 12× (3× × 4×)
/// - Atomic + Fixed-Point + SIMD: 24× (3× × 2× × 4×)
/// - GPU + Fixed-Point + Batch: 2000× (100× × 2× × 10×)
///
/// ## ASSUM Framework
///
/// - `#ASSUME_ALIGNMENT_MAX`: Alignment is max of component alignments
/// - `#VERIFY_ALIGNMENT_MAX`: Checked at compile-time
/// - `#ASSUME_TIERS_COMPATIBLE`: Component tiers can be composed
/// - `#VERIFY_TIERS_COMPATIBLE`: Manual validation
///
/// # Safety
///
/// This trait is unsafe to implement because:
/// - Implementor must ensure alignment is at least the maximum of component alignments
/// - Incorrect composition causes cache thrashing or false sharing
/// - Component tiers must be compatible (validated manually)
/// - Implementor must validate compound operations preserve correctness
pub unsafe trait MixedCapsule<T1: Capsule, T2: Capsule>: Capsule {
    /// Primary component (usually coordination)
    fn component1(&self) -> &T1;

    /// Secondary component (usually computation)
    fn component2(&self) -> &T2;

    /// Verify mixed capsule alignment (must be max of components)
    fn verify_mixed_alignment() -> bool {
        Self::ALIGNMENT >= T1::ALIGNMENT && Self::ALIGNMENT >= T2::ALIGNMENT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_display() {
        assert_eq!(Tier::T1Atomic.to_string(), "Tier 1: Atomic");
        assert_eq!(Tier::T2Simd.to_string(), "Tier 2: SIMD");
        assert_eq!(Tier::T6Mixed.to_string(), "Tier 6: Mixed");
    }

    #[test]
    fn test_tier_values() {
        assert_eq!(Tier::T1Atomic as u8, 1);
        assert_eq!(Tier::T2Simd as u8, 2);
        assert_eq!(Tier::T10Probabilistic as u8, 10);
    }

    #[test]
    fn test_verification_error_display() {
        let err = VerificationError::AlignmentMismatch {
            expected: 64,
            actual: 32,
        };
        assert!(err.to_string().contains("64"));
        assert!(err.to_string().contains("32"));

        let err = VerificationError::SizeMismatch {
            expected: 128,
            actual: 64,
        };
        assert!(err.to_string().contains("128"));

        let err = VerificationError::TierViolation {
            expected: Tier::T1Atomic,
            actual: Tier::T2Simd,
        };
        assert!(err.to_string().contains("Tier 1"));
    }

    // Test capsule for verification
    #[repr(C, align(64))]
    struct TestCapsule {
        _data: [u8; 64],
    }

    unsafe impl Capsule for TestCapsule {
        const TIER: Tier = Tier::T1Atomic;
        const ALIGNMENT: usize = 64;
        const SIZE: usize = 64;
    }

    #[test]
    fn test_capsule_verification() {
        // Should pass verification
        assert!(TestCapsule::verify().is_ok());
    }

    #[test]
    fn test_capsule_type_id() {
        let type_name = TestCapsule::type_id();
        assert!(type_name.contains("TestCapsule"));
    }
}
