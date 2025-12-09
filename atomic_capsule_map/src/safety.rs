// Safety Validation Utilities for AtomicCapsuleMap
// This file will be integrated into atomic_capsule_map/src/safety.rs once implementation exists
//
// ASSUM Framework: Every #ASSUME needs #VERIFY
// These utilities provide runtime verification for safety assumptions

// Allow future proptest feature until it's added to Cargo.toml
#![allow(unexpected_cfgs)]

use std::sync::atomic::Ordering;

/// Generation counter overflow detection
///
/// #ASSUME_GENERATION_MONOTONIC: Generation counters always increase
/// #VERIFY_GENERATION_MONOTONIC: This helper detects overflow before it happens
pub struct GenerationGuard {
    /// Current generation value
    current: u64,
    /// Maximum safe generation before overflow
    max_safe: u64,
}

impl GenerationGuard {
    /// Create a new generation guard
    ///
    /// # Arguments
    /// * `warn_threshold` - Warn when generation exceeds this value (default: u64::MAX - 1_000_000)
    pub fn new(warn_threshold: Option<u64>) -> Self {
        Self {
            current: 0,
            max_safe: warn_threshold.unwrap_or(u64::MAX - 1_000_000),
        }
    }

    /// Increment generation counter and check for overflow risk
    ///
    /// # Returns
    /// * `Ok(new_generation)` if safe to continue
    /// * `Err(current_generation)` if approaching overflow
    ///
    /// #ASSUME_GENERATION_MONOTONIC: Caller ensures single writer
    /// #VERIFY_GENERATION_MONOTONIC: This function validates monotonicity
    pub fn increment(&mut self) -> Result<u64, u64> {
        self.current = self.current.wrapping_add(1);

        if self.current > self.max_safe {
            Err(self.current)
        } else {
            Ok(self.current)
        }
    }

    /// Get current generation value
    pub fn current(&self) -> u64 {
        self.current
    }

    /// Check if generation is approaching overflow
    pub fn is_overflow_risk(&self) -> bool {
        self.current > self.max_safe
    }
}

/// Torn read detection helper
///
/// Validates that two-phase commit prevents reading inconsistent state
///
/// #ASSUME_TOCTOU_SAFE: Two-phase commit prevents torn reads
/// #VERIFY_TOCTOU_PREVENTED: This helper validates head/tail consistency
#[derive(Debug, Clone, Copy)]
pub struct TwoPhaseValidator {
    /// Head generation counter
    head_gen: u32,
    /// Tail generation counter
    tail_gen: u32,
    /// Commit bit
    commit: bool,
}

impl TwoPhaseValidator {
    /// Create validator from head and tail words
    ///
    /// # Arguments
    /// * `head` - Head word containing commit bit and generation
    /// * `tail` - Tail word containing generation counter
    pub fn new(head: u64, tail: u64) -> Self {
        // Extract fields (bit layout depends on implementation)
        let commit = (head & 0x1) == 1;
        let head_gen = ((head >> 1) & 0xFFFFFFFF) as u32;
        let tail_gen = (tail & 0xFFFFFFFF) as u32;

        Self {
            head_gen,
            tail_gen,
            commit,
        }
    }

    /// Validate that head and tail are consistent
    ///
    /// # Returns
    /// * `true` if state is valid (committed and generations match)
    /// * `false` if state is invalid (torn read detected)
    ///
    /// #ASSUME_TOCTOU_SAFE: Valid state has commit=1 and head_gen == tail_gen
    /// #VERIFY_TOCTOU_PREVENTED: This function detects torn reads
    pub fn is_valid(&self) -> bool {
        self.commit && self.head_gen == self.tail_gen
    }

    /// Check if commit bit is set
    pub fn is_committed(&self) -> bool {
        self.commit
    }

    /// Check if generation counters match
    pub fn generations_match(&self) -> bool {
        self.head_gen == self.tail_gen
    }

    /// Get head generation
    pub fn head_generation(&self) -> u32 {
        self.head_gen
    }

    /// Get tail generation
    pub fn tail_generation(&self) -> u32 {
        self.tail_gen
    }
}

/// Memory ordering validation helper
///
/// Provides utilities for validating correct memory ordering usage
///
/// #ASSUME_MEMORY_ORDERING: Ordering choices are justified and sufficient
/// #VERIFY_ORDERING_SUFFICIENT: This helper documents ordering requirements
pub struct OrderingValidator;

impl OrderingValidator {
    /// Validate read ordering for atomic load
    ///
    /// Rules:
    /// - Acquire: Required when dereferencing loaded pointer or establishing happens-before
    /// - Relaxed: Safe for independent loads synchronized by other operations
    ///
    /// #ASSUME_MEMORY_ORDERING: Caller chooses appropriate ordering
    /// #VERIFY_ORDERING_SUFFICIENT: This documents when each ordering is safe
    pub fn validate_load_ordering(
        ordering: Ordering,
        dereferencing: bool,
        synchronized_by_other: bool,
    ) -> Result<(), &'static str> {
        match ordering {
            Ordering::Relaxed => {
                if dereferencing && !synchronized_by_other {
                    return Err("Relaxed load before dereference requires synchronization");
                }
                Ok(())
            }
            Ordering::Acquire => Ok(()),
            Ordering::Release => Err("Release ordering invalid for loads"),
            Ordering::AcqRel => Err("AcqRel ordering overkill for loads"),
            Ordering::SeqCst => {
                // Always safe but may be overkill
                Ok(())
            }
            _ => Err("Invalid ordering"),
        }
    }

    /// Validate write ordering for atomic store
    ///
    /// Rules:
    /// - Release: Required when publishing data for other threads
    /// - Relaxed: Safe for independent stores not establishing happens-before
    ///
    /// #ASSUME_MEMORY_ORDERING: Caller chooses appropriate ordering
    /// #VERIFY_ORDERING_SUFFICIENT: This documents when each ordering is safe
    pub fn validate_store_ordering(
        ordering: Ordering,
        publishing: bool,
    ) -> Result<(), &'static str> {
        match ordering {
            Ordering::Relaxed => {
                if publishing {
                    return Err("Relaxed store for publication may lose updates");
                }
                Ok(())
            }
            Ordering::Release => Ok(()),
            Ordering::Acquire => Err("Acquire ordering invalid for stores"),
            Ordering::AcqRel => Err("AcqRel ordering overkill for stores"),
            Ordering::SeqCst => {
                // Always safe but may be overkill
                Ok(())
            }
            _ => Err("Invalid ordering"),
        }
    }
}

/// ABA problem detection helper
///
/// Validates that generation counters prevent ABA problem in CAS operations
///
/// #ASSUME_ABA_PREVENTED: Generation counters prevent ABA
/// #VERIFY_ABA_PREVENTED: This helper validates generation-based ABA prevention
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionedValue {
    /// Actual value
    pub value: u32,
    /// Generation counter (prevents ABA)
    pub generation: u32,
}

impl VersionedValue {
    /// Pack value and generation into single u64
    pub fn pack(self) -> u64 {
        ((self.generation as u64) << 32) | (self.value as u64)
    }

    /// Unpack u64 into value and generation
    pub fn unpack(packed: u64) -> Self {
        Self {
            value: (packed & 0xFFFFFFFF) as u32,
            generation: ((packed >> 32) & 0xFFFFFFFF) as u32,
        }
    }

    /// Check if CAS with this value would be safe from ABA
    ///
    /// Returns true if generation has advanced since observed value
    pub fn is_aba_safe(&self, observed: Self) -> bool {
        // ABA-safe if generation has advanced
        // Even if value is same, generation difference prevents ABA
        self.generation > observed.generation
    }

    /// Increment generation (use on every update)
    pub fn next_generation(self, new_value: u32) -> Self {
        Self {
            value: new_value,
            generation: self.generation.wrapping_add(1),
        }
    }
}

/// Runtime safety validation for atomic operations
///
/// Provides debug-mode assertions for safety invariants
///
/// #ASSUME_INVARIANT: Safety invariants are maintained
/// #VERIFY_INVARIANT: These assertions validate invariants in debug builds
pub struct InvariantChecker;

impl InvariantChecker {
    /// Validate cache line alignment
    ///
    /// #ASSUME_INVARIANT: All atomic capsules are 64-byte aligned
    /// #VERIFY_INVARIANT: This checks alignment in debug mode
    pub fn check_alignment<T>(ptr: *const T, alignment: usize) {
        debug_assert_eq!(
            (ptr as usize) % alignment,
            0,
            "Pointer {:p} not aligned to {} bytes",
            ptr,
            alignment
        );
    }

    /// Validate generation counter monotonicity
    ///
    /// #ASSUME_GENERATION_MONOTONIC: Generations always increase
    /// #VERIFY_GENERATION_MONOTONIC: This checks monotonicity in debug mode
    pub fn check_generation_monotonic(old_gen: u32, new_gen: u32) {
        debug_assert!(
            new_gen > old_gen || (old_gen == u32::MAX && new_gen == 0),
            "Generation not monotonic: {} -> {}",
            old_gen,
            new_gen
        );
    }

    /// Validate two-phase commit state
    ///
    /// #ASSUME_TOCTOU_SAFE: Two-phase commit maintains consistency
    /// #VERIFY_TOCTOU_PREVENTED: This checks commit state in debug mode
    pub fn check_commit_state(commit: bool, gen_even: bool, head_tail_match: bool) {
        debug_assert!(
            !commit || (gen_even && head_tail_match),
            "Invalid commit state: commit={}, gen_even={}, head_tail_match={}",
            commit,
            gen_even,
            head_tail_match
        );
    }

    /// Validate no overflow in counter
    ///
    /// #ASSUME_METRIC_ATOMIC: Counters don't overflow
    /// #VERIFY_COUNTER_ACCURACY: This checks for overflow risk
    pub fn check_counter_overflow(counter: u64, increment: u64, max_safe: u64) {
        debug_assert!(
            counter.saturating_add(increment) <= max_safe,
            "Counter overflow risk: {} + {} > {}",
            counter,
            increment,
            max_safe
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generation_guard_increment() {
        let mut guard = GenerationGuard::new(Some(100));

        // Normal increments should succeed
        for i in 1..=100 {
            assert_eq!(guard.increment().unwrap(), i);
        }

        // Next increment should warn
        assert!(guard.increment().is_err());
    }

    #[test]
    fn test_two_phase_validator() {
        // Valid state: commit=1 (bit 0), gen=1 (bits 1-32)
        let head = (1u64 << 1) | 1u64; // gen=1 shifted left, commit=1 in bit 0 = 0b11 = 3
        let tail = 1u64; // gen=1 in low bits
        let validator = TwoPhaseValidator::new(head, tail);
        assert!(validator.is_valid());

        // Invalid state: generations don't match
        let head = (2u64 << 1) | 1u64; // gen=2 shifted left, commit=1 = 0b101 = 5
        let tail = 1u64; // gen=1
        let validator = TwoPhaseValidator::new(head, tail);
        assert!(!validator.is_valid());

        // Invalid state: not committed
        let head = (1u64 << 1); // gen=1 shifted left, commit=0 = 0b10 = 2
        let tail = 1u64; // gen=1
        let validator = TwoPhaseValidator::new(head, tail);
        assert!(!validator.is_valid());
    }

    #[test]
    fn test_versioned_value_aba_prevention() {
        let v1 = VersionedValue {
            value: 42,
            generation: 1,
        };
        let v2 = VersionedValue {
            value: 99,
            generation: 2,
        };
        let v3 = VersionedValue {
            value: 42,
            generation: 3,
        }; // Same value, different gen

        // v3 is ABA-safe relative to v1 (generation advanced)
        assert!(v3.is_aba_safe(v1));

        // v2 is ABA-safe relative to v1 (generation advanced)
        assert!(v2.is_aba_safe(v1));

        // v1 is NOT ABA-safe relative to v2 (generation did not advance)
        assert!(!v1.is_aba_safe(v2));
    }

    #[test]
    fn test_versioned_value_pack_unpack() {
        let original = VersionedValue {
            value: 0x12345678,
            generation: 0xABCDEF01,
        };
        let packed = original.pack();
        let unpacked = VersionedValue::unpack(packed);

        assert_eq!(original, unpacked);
    }

    #[test]
    fn test_ordering_validator() {
        // Relaxed load for independent value is safe
        assert!(OrderingValidator::validate_load_ordering(
            Ordering::Relaxed,
            false, // not dereferencing
            true,  // synchronized by other
        )
        .is_ok());

        // Relaxed load before dereferencing without sync is NOT safe
        assert!(OrderingValidator::validate_load_ordering(
            Ordering::Relaxed,
            true,  // dereferencing
            false, // NOT synchronized
        )
        .is_err());

        // Acquire load is always safe
        assert!(OrderingValidator::validate_load_ordering(Ordering::Acquire, true, false,).is_ok());

        // Release load is invalid
        assert!(
            OrderingValidator::validate_load_ordering(Ordering::Release, false, true,).is_err()
        );
    }

    #[test]
    fn test_invariant_checker_alignment() {
        #[repr(align(64))]
        struct Aligned {
            _data: [u8; 64],
        }

        let aligned = Aligned { _data: [0; 64] };
        InvariantChecker::check_alignment(&aligned as *const Aligned, 64);
    }

    #[test]
    fn test_invariant_checker_generation_monotonic() {
        InvariantChecker::check_generation_monotonic(1, 2);
        InvariantChecker::check_generation_monotonic(100, 101);

        // Wraparound is allowed
        InvariantChecker::check_generation_monotonic(u32::MAX, 0);
    }

    #[test]
    #[should_panic]
    fn test_invariant_checker_generation_not_monotonic() {
        // This should panic in debug mode
        InvariantChecker::check_generation_monotonic(2, 1);
    }
}

/// Property-based test helpers for concurrent scenarios
/// TODO(Phase 4): Add proptest feature to Cargo.toml when property tests are needed
#[allow(unexpected_cfgs)]
#[cfg(all(test, feature = "proptest"))]
mod proptest_helpers {
    use super::*;
    use proptest::prelude::*;

    /// Generate arbitrary VersionedValue
    pub fn versioned_value() -> impl Strategy<Value = VersionedValue> {
        (any::<u32>(), any::<u32>())
            .prop_map(|(value, generation)| VersionedValue { value, generation })
    }

    /// Generate sequence of monotonically increasing generations
    pub fn monotonic_generations(count: usize) -> impl Strategy<Value = Vec<u32>> {
        prop::collection::vec(0u32..1000, count).prop_map(|mut gens| {
            gens.sort_unstable();
            gens
        })
    }

    proptest! {
        /// Property: Versioned values with higher generation are always ABA-safe
        #[test]
        fn prop_higher_generation_is_aba_safe(
            value1 in any::<u32>(),
            value2 in any::<u32>(),
            gen1 in 0u32..1_000_000,
            gen2 in 0u32..1_000_000,
        ) {
            let v1 = VersionedValue { value: value1, generation: gen1 };
            let v2 = VersionedValue { value: value2, generation: gen2 };

            if gen2 > gen1 {
                prop_assert!(v2.is_aba_safe(v1));
            }
        }

        /// Property: Pack/unpack is lossless
        #[test]
        fn prop_pack_unpack_lossless(v in versioned_value()) {
            let packed = v.pack();
            let unpacked = VersionedValue::unpack(packed);
            prop_assert_eq!(v, unpacked);
        }
    }
}
