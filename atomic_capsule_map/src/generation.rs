//! Generation Counter Utilities for ABA Prevention
//!
//! Provides monotonic generation counters to prevent ABA problems in lockfree
//! operations. Generation counters are packed into the atomic capsule state
//! to enable TOCTOU-safe operations without locks.
//!
//! # The Atomic Capsule Pattern
//!
//! From "The Atomic Capsule" architecture:
//! - Generation counters prevent ABA problems through monotonic versioning
//! - Pack generation with value for atomic CAS operations
//! - TOCTOU prevention: validate generation hasn't changed during operation
//!
//! # Safety Assumptions
//!
//! #ASSUME: Generation counter wraps are rare enough that we can detect ABA
//! #VERIFY: Tests stress generation wrap behavior
//! #ASSUME: 32-bit generation space is sufficient for practical operation
//! #VERIFY: Benchmark validates generation update performance <15ns

use core::sync::atomic::{AtomicU64, Ordering};

/// Monotonic generation counter for ABA prevention
///
/// Generation is incremented on every update to detect concurrent modifications.
/// The generation counter is 32 bits, providing 4 billion unique versions before wrap.
///
/// # Memory Layout
///
/// AtomicU64 contains:
/// - Low 32 bits: Generation counter (monotonic, wraps at u32::MAX)
/// - High 32 bits: Available for packing with value data
///
/// # Performance Target
///
/// Per atomic capsule principles: <15ns for generation update (hardware CAS latency)
#[repr(align(8))]
pub struct MonotonicGen {
    /// Packed generation counter (low 32 bits)
    /// #ASSUME: AtomicU64 provides sequentially consistent ordering for CAS
    /// #VERIFY: Memory ordering validated in concurrent tests
    state: AtomicU64,
}

impl MonotonicGen {
    /// Create a new generation counter starting at generation 0
    ///
    /// # Const Construction
    ///
    /// Uses const fn for compile-time initialization, enabling static allocation
    /// of generation counters with zero runtime cost.
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
        }
    }

    /// Create generation counter with specific starting generation
    ///
    /// Used for testing or recovery scenarios where generation needs to be restored.
    pub const fn with_generation(generation: u32) -> Self {
        Self {
            state: AtomicU64::new(generation as u64),
        }
    }

    /// Load current generation (low 32 bits)
    ///
    /// # Memory Ordering
    ///
    /// Uses Relaxed ordering - generation is always read as part of larger
    /// atomic operation with proper synchronization at capsule level.
    ///
    /// #ASSUME: Relaxed load is sufficient for generation-only read
    /// #VERIFY: Synchronization validated in capsule integration tests
    #[inline(always)]
    pub fn load(&self) -> u32 {
        (self.state.load(Ordering::Relaxed) & 0xFFFF_FFFF) as u32
    }

    /// Load full 64-bit state (generation + packed data)
    ///
    /// Returns complete state for atomic operations that pack additional
    /// data in high 32 bits.
    #[inline(always)]
    pub fn load_full(&self) -> u64 {
        self.state.load(Ordering::Relaxed)
    }

    /// Increment generation and return new value
    ///
    /// Atomically increments generation counter, wrapping at u32::MAX.
    /// Returns the new generation value.
    ///
    /// # Performance
    ///
    /// Target: <15ns (single atomic fetch_add)
    ///
    /// # Memory Ordering
    ///
    /// Uses Release ordering to ensure all prior writes are visible
    /// to threads that observe the new generation.
    #[inline(always)]
    pub fn increment(&self) -> u32 {
        // Fetch-add with generation mask to handle wrap
        let old = self.state.fetch_add(1, Ordering::Release);
        ((old + 1) & 0xFFFF_FFFF) as u32
    }

    /// Atomically update generation and packed data
    ///
    /// Performs CAS operation on full 64-bit state. Used when updating
    /// both generation and associated data in single atomic operation.
    ///
    /// # Arguments
    ///
    /// * `current` - Expected current state (generation + data)
    /// * `new` - New state to write (generation + data)
    ///
    /// # Returns
    ///
    /// Ok(()) if CAS succeeded, Err(actual) with actual state if CAS failed
    ///
    /// # Memory Ordering
    ///
    /// Success: Release (makes all prior writes visible)
    /// Failure: Relaxed (no synchronization needed on failure)
    ///
    /// #ASSUME: Release/Relaxed ordering is sufficient for generation CAS
    /// #VERIFY: Memory ordering validated in concurrent stress tests
    #[inline(always)]
    pub fn compare_exchange(&self, current: u64, new: u64) -> Result<(), u64> {
        self.state
            .compare_exchange(current, new, Ordering::Release, Ordering::Relaxed)
            .map(|_| ())
    }

    /// Weak CAS variant for retry loops
    ///
    /// Faster than strong CAS but may spuriously fail. Use in retry loops
    /// where spurious failure is acceptable.
    #[inline(always)]
    pub fn compare_exchange_weak(&self, current: u64, new: u64) -> Result<(), u64> {
        self.state
            .compare_exchange_weak(current, new, Ordering::Release, Ordering::Relaxed)
            .map(|_| ())
    }
}

impl Default for MonotonicGen {
    fn default() -> Self {
        Self::new()
    }
}

/// Pack generation into high 32 bits of u64
///
/// # Use Case
///
/// Pack generation with 32-bit value for atomic CAS operations.
///
/// # Example
///
/// ```
/// # use atomic_capsule_map::generation::pack_gen_high;
/// let value = 0x1234_5678u32;
/// let generation = 42u32;
/// let packed = pack_gen_high(value, generation);
/// assert_eq!(packed, 0x0000_002A_1234_5678u64);
/// ```
#[inline(always)]
pub const fn pack_gen_high(value: u32, generation: u32) -> u64 {
    (value as u64) | ((generation as u64) << 32)
}

/// Pack generation into low 32 bits of u64
///
/// # Use Case
///
/// Pack generation with 32-bit value where generation goes in low bits.
#[inline(always)]
pub const fn pack_gen_low(generation: u32, value: u32) -> u64 {
    (generation as u64) | ((value as u64) << 32)
}

/// Extract generation from high 32 bits
#[inline(always)]
pub const fn unpack_gen_high(packed: u64) -> u32 {
    (packed >> 32) as u32
}

/// Extract generation from low 32 bits
#[inline(always)]
pub const fn unpack_gen_low(packed: u64) -> u32 {
    (packed & 0xFFFF_FFFF) as u32
}

/// Extract value from low 32 bits (when generation is high)
#[inline(always)]
pub const fn unpack_value_low(packed: u64) -> u32 {
    (packed & 0xFFFF_FFFF) as u32
}

/// Extract value from high 32 bits (when generation is low)
#[inline(always)]
pub const fn unpack_value_high(packed: u64) -> u32 {
    (packed >> 32) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic_gen_increment() {
        let generation = MonotonicGen::new();
        assert_eq!(generation.load(), 0);

        let g1 = generation.increment();
        assert_eq!(g1, 1);
        assert_eq!(generation.load(), 1);

        let g2 = generation.increment();
        assert_eq!(g2, 2);
        assert_eq!(generation.load(), 2);
    }

    #[test]
    fn pack_unpack_gen_high() {
        let value = 0xDEAD_BEEFu32;
        let generation = 123u32;
        let packed = pack_gen_high(value, generation);

        assert_eq!(unpack_value_low(packed), value);
        assert_eq!(unpack_gen_high(packed), generation);
    }

    #[test]
    fn pack_unpack_gen_low() {
        let value = 0xCAFE_BABEu32;
        let generation = 456u32;
        let packed = pack_gen_low(generation, value);

        assert_eq!(unpack_gen_low(packed), generation);
        assert_eq!(unpack_value_high(packed), value);
    }

    #[test]
    fn generation_wrapping() {
        let generation = MonotonicGen::with_generation(u32::MAX - 2);

        assert_eq!(generation.increment(), u32::MAX - 1);
        assert_eq!(generation.increment(), u32::MAX);
        // Next increment wraps to 0
        let wrapped = generation.increment();
        assert_eq!(wrapped, 0);
    }

    #[test]
    fn compare_exchange_success() {
        let generation = MonotonicGen::new();
        let current = generation.load_full();
        let new = pack_gen_low(1, 42); // Fixed: generation in LOW bits, value in HIGH

        assert!(generation.compare_exchange(current, new).is_ok());
        assert_eq!(generation.load(), 1);
        assert_eq!(unpack_value_high(generation.load_full()), 42); // Fixed: value in HIGH bits
    }

    #[test]
    fn compare_exchange_failure() {
        let generation = MonotonicGen::new();
        generation.increment();

        let wrong_current = 0u64;
        let new = pack_gen_low(2, 99); // Fixed: generation in LOW bits

        let result = generation.compare_exchange(wrong_current, new);
        assert!(result.is_err());
        assert_eq!(generation.load(), 1); // Unchanged
    }

    #[cfg(not(miri))]
    #[test]
    fn concurrent_increments() {
        use std::sync::Arc;
        use std::thread;

        let generation = Arc::new(MonotonicGen::new());
        let mut handles = vec![];

        for _ in 0..8 {
            let gen_clone = Arc::clone(&generation);
            let handle = thread::spawn(move || {
                for _ in 0..1000 {
                    gen_clone.increment();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(generation.load(), 8000);
    }
}
