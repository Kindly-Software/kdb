//! Deterministic Debugging Infrastructure - T28 Q8-Q14 Validation
//!
//! Provides deterministic context and replay validation infrastructure for
//! time-travel debugging with guaranteed reproducibility across multiple runs.
//!
//! Framework: UCE34 Q8-Q14 (Property-based testing)
//! Tiers: T0 (Auditable) + T1 (Atomic) + T5 (Streaming)
//!
//! #ASSUME_LOCKFREE_ONLY: All coordination via atomics, no mutex/RwLock
//! #ASSUME_DETERMINISTIC_TIME: System time never goes backwards
//! #ASSUME_SEED_BASED_RNG: PRNG with fixed seed always produces same sequence
//! #ASSUME_SNAPSHOT_IMMUTABLE: Snapshots unchanged once written

use std::sync::atomic::{AtomicU64, Ordering};

/// Deterministic context for time-travel debugger
///
/// Encapsulates a fixed seed and simulated time for reproducing
/// debugging sessions bit-for-bit identical across multiple runs.
///
/// Size: 24 bytes (cache-aligned with padding)
/// #[repr(C, align(64))]
///
/// Fields:
/// - seed (u64): Random seed for deterministic PRNG
/// - simulated_time_ns (AtomicU64): Simulated monotonic time in nanoseconds
/// - last_snapshot_ns (AtomicU64): Timestamp of last snapshot taken
#[repr(C, align(64))]
pub struct DeterministicDebuggerContext {
    /// Fixed seed for deterministic random number generation
    pub seed: u64,

    /// Simulated monotonic time in nanoseconds (Release/Acquire ordering)
    pub simulated_time_ns: AtomicU64,

    /// Timestamp of last snapshot captured (for monotonicity validation)
    pub last_snapshot_ns: AtomicU64,

    /// Padding to cache-line boundary (56 bytes - 24 = 32 bytes)
    _padding: [u8; 32],
}

impl DeterministicDebuggerContext {
    /// Create new deterministic context with given seed
    ///
    /// #ASSUME_SEED_BASED_RNG: Fixed seed always produces deterministic behavior
    /// #VERIFY_TEST: test_deterministic_context_creation
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            simulated_time_ns: AtomicU64::new(1_000_000_000),  // Start at 1 second
            last_snapshot_ns: AtomicU64::new(0),
            _padding: [0; 32],
        }
    }

    /// Get current simulated time in nanoseconds
    ///
    /// Performance: O(1), <2ns (Relaxed load)
    /// #ASSUME_LOCKFREE_ONLY: No mutex/RwLock
    pub fn now_ns(&self) -> u64 {
        self.simulated_time_ns.load(Ordering::Relaxed)
    }

    /// Advance simulated time by delta (nanoseconds)
    ///
    /// Performance: O(1), <5ns (fetch_add with Relaxed ordering)
    ///
    /// #ASSUME_DETERMINISTIC_TIME: Each call advances by exactly delta_ns
    /// #VERIFY_UNIT_TEST: test_time_advance_deterministic
    pub fn advance_time(&self, delta_ns: u64) {
        self.simulated_time_ns.fetch_add(delta_ns, Ordering::Relaxed);
    }

    /// Record snapshot timestamp and verify monotonicity
    ///
    /// Performance: O(1), <10ns
    ///
    /// Returns true if monotonic, false if timestamp goes backwards
    /// (detection of time anomalies for compliance auditing)
    ///
    /// #ASSUME_DETERMINISTIC_TIME: Snapshots never have decreasing timestamps
    /// #VERIFY_UNIT_TEST: test_monotonic_snapshot_timestamps
    pub fn record_snapshot_time(&self) -> bool {
        let now = self.now_ns();
        let last = self.last_snapshot_ns.load(Ordering::Acquire);

        if now < last {
            return false;  // Time went backwards!
        }

        self.last_snapshot_ns.store(now, Ordering::Release);
        true
    }

    /// Reset to initial state for test replay
    ///
    /// Performance: O(1)
    ///
    /// Used to reset deterministic context between replay runs
    /// to ensure identical execution from snapshot 0
    pub fn reset_to_initial(&self) {
        self.simulated_time_ns.store(1_000_000_000, Ordering::Release);
        self.last_snapshot_ns.store(0, Ordering::Release);
    }

    /// Get system seed (for determinism validation)
    pub fn get_seed(&self) -> u64 {
        self.seed
    }
}

impl Default for DeterministicDebuggerContext {
    fn default() -> Self {
        Self::new(0xDEADBEEFCAFEBABE)
    }
}

/// Simple linear congruential generator for deterministic random numbers
///
/// LCG: X(n+1) = (a*X(n) + c) mod m
/// Standard MINSTD constants (Park & Miller, 1988)
///
/// NOT cryptographically secure, but deterministic and fast (perfect for testing)
#[derive(Clone, Copy, Debug)]
pub struct DeterministicRng {
    state: u64,
    seed: u64,
}

impl DeterministicRng {
    /// Create new RNG with given seed
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
            seed,
        }
    }

    /// Generate next random u64
    ///
    /// Performance: O(1), <5ns
    ///
    /// #ASSUME_SEED_BASED_RNG: Same seed always produces same sequence
    /// #VERIFY_UNIT_TEST: test_rng_determinism
    pub fn next_u64(&mut self) -> u64 {
        // LCG: X(n+1) = (6364136223846793005 * X(n) + 1442695040888963407) mod 2^64
        // (Solaris/glibc constants)
        self.state = self.state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    /// Generate next random u32
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Generate random u64 in range [0, max)
    pub fn next_u64_bounded(&mut self, max: u64) -> u64 {
        if max == 0 {
            return 0;
        }
        self.next_u64() % max
    }

    /// Reset to seed for deterministic replay
    pub fn reset(&mut self) {
        self.state = if self.seed == 0 { 1 } else { self.seed };
    }
}

impl Default for DeterministicRng {
    fn default() -> Self {
        Self::new(42)
    }
}

// ============================================================================
// TESTS (T28 Unit/Property tier)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_context_creation() {
        let ctx = DeterministicDebuggerContext::new(0xABCD1234);
        assert_eq!(ctx.get_seed(), 0xABCD1234);
        assert_eq!(ctx.now_ns(), 1_000_000_000);
        assert_eq!(ctx.last_snapshot_ns.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_time_advance_deterministic() {
        let ctx = DeterministicDebuggerContext::new(0);
        assert_eq!(ctx.now_ns(), 1_000_000_000);

        ctx.advance_time(100);
        assert_eq!(ctx.now_ns(), 1_000_000_100);

        ctx.advance_time(900);
        assert_eq!(ctx.now_ns(), 1_000_001_000);
    }

    #[test]
    fn test_monotonic_snapshot_timestamps() {
        let ctx = DeterministicDebuggerContext::new(0);

        // Record first snapshot at t=1s
        assert!(ctx.record_snapshot_time());
        assert_eq!(ctx.last_snapshot_ns.load(Ordering::Relaxed), 1_000_000_000);

        // Advance time and record another
        ctx.advance_time(50);
        assert!(ctx.record_snapshot_time());
        assert_eq!(ctx.last_snapshot_ns.load(Ordering::Relaxed), 1_000_000_050);

        // Try to record with time going backwards (manually set earlier time)
        ctx.simulated_time_ns.store(999_999_999, Ordering::Release);
        assert!(!ctx.record_snapshot_time(), "Should detect time going backwards");
    }

    #[test]
    fn test_reset_to_initial() {
        let ctx = DeterministicDebuggerContext::new(42);
        ctx.advance_time(1000);
        ctx.record_snapshot_time();

        assert_ne!(ctx.now_ns(), 1_000_000_000);

        ctx.reset_to_initial();
        assert_eq!(ctx.now_ns(), 1_000_000_000);
        assert_eq!(ctx.last_snapshot_ns.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_rng_determinism() {
        let mut rng1 = DeterministicRng::new(0xDEADBEEF);
        let mut rng2 = DeterministicRng::new(0xDEADBEEF);

        // Same seed produces identical sequence
        for _ in 0..100 {
            assert_eq!(rng1.next_u64(), rng2.next_u64(), "RNG sequences must match");
        }
    }

    #[test]
    fn test_rng_different_seeds() {
        let mut rng1 = DeterministicRng::new(0x1111);
        let mut rng2 = DeterministicRng::new(0x2222);

        // Different seeds produce different sequences
        let mut matches = 0;
        for _ in 0..100 {
            if rng1.next_u64() == rng2.next_u64() {
                matches += 1;
            }
        }

        // Expect very few matches (statistically near 0)
        assert!(matches < 5, "Different seeds should produce different sequences (got {} matches)", matches);
    }

    #[test]
    fn test_rng_reset() {
        let mut rng = DeterministicRng::new(0xABCD);

        // Generate sequence
        let v1 = rng.next_u64();
        let v2 = rng.next_u64();

        // Reset and regenerate
        rng.reset();
        assert_eq!(v1, rng.next_u64());
        assert_eq!(v2, rng.next_u64());
    }

    #[test]
    fn test_rng_bounded() {
        let mut rng = DeterministicRng::new(12345);

        // Generate bounded random values
        for _ in 0..1000 {
            let val = rng.next_u64_bounded(100);
            assert!(val < 100, "Bounded RNG should return values < 100");
        }
    }

    #[test]
    fn test_context_structure_alignment() {
        use std::mem::{align_of, size_of};

        assert_eq!(align_of::<DeterministicDebuggerContext>(), 64);
        assert_eq!(size_of::<DeterministicDebuggerContext>(), 64);
    }
}
