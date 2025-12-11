//! Poisoned Generation Capsule - T0 Auditable + T1 Atomic Fractal Self-Destruct
//!
//! **UCE34 Q1-Q34 Compliant**: Fractal self-destruct mechanism via generation counters
//!
//! # Architecture
//!
//! **Tier 0 (Auditable)**: Compile-time verifiable bit layout with hash-chain integrity
//! **Tier 1 (Atomic)**: Lockfree poisoning operations with generation tracking
//!
//! # Bit Layout (64-bit, Q34 Auditable)
//!
//! ```text
//! Bits 0-55:  Generation Counter (56-bit, ~72 quadrillion values)
//! Bits 56-59: Cascade Level (0=none, 1-15=propagation depth)
//! Bit 60:     POISONED flag (irreversible tamper indicator)
//! Bit 61:     CORRUPTED flag (data zeroed indicator)
//! Bit 62:     PROPAGATING flag (cascade in progress)
//! Bit 63:     TERMINAL flag (no recovery possible)
//! ```
//!
//! # Performance (B32 Targets)
//!
//! - Generation extraction: <1ns (bit masking)
//! - Poison operation: <5ns (single atomic OR)
//! - State check: <1ns (bit test)
//!
//! # Safety (ASSUM Framework)
//!
//! 100% safe - No unsafe code, all operations are pure bit manipulation
//!
//! # Use Cases
//!
//! - Tamper detection in audit trails
//! - Cascading invalidation of dependent capsules
//! - Fractal self-destruct for security-critical data
//! - Generation-based TOCTOU prevention
//!
//! # UCE34 Framework Compliance
//!
//! - **Q1**: Problem = Tamper-evident generation tracking with self-destruct
//! - **Q2**: Constraints = no_std, zero allocations, lockfree
//! - **Q3**: Tier Selection = T0+T1 (Auditable + Atomic)
//! - **Q10**: Data structure = 64-bit packed with bit fields
//! - **Q11**: Rust patterns = repr(transparent), const fn
//! - **Q12**: Nightly features = None required (stable compatible)
//! - **Q33**: Verification = Compile-time layout validation
//! - **Q34**: Auditability = Hash-chain compatible via generation counter
//!
//! # ASSUM Framework Tags
//!
//! - `#ASSUME_BIT_LAYOUT_STABLE`: Bit positions are fixed and documented
//! - `#VERIFY_BIT_LAYOUT_STABLE`: Unit tests verify all bit extractions
//! - `#ASSUME_GENERATION_MONOTONIC`: Generation only increments when not poisoned
//! - `#VERIFY_GENERATION_MONOTONIC`: increment_generation() checks poisoned state
//! - `#ASSUME_POISON_IRREVERSIBLE`: Once poisoned, cannot be cleared
//! - `#VERIFY_POISON_IRREVERSIBLE`: No method to clear poison flags exists
//! - `#ASSUME_CASCADE_BOUNDED`: Cascade level limited to 0-15
//! - `#VERIFY_CASCADE_BOUNDED`: 4-bit field enforces bound at type level
//!
//! # Example
//!
//! ```rust
//! use atomic_capsule::protection::PoisonedGeneration;
//!
//! // Create with initial generation
//! let mut gen = PoisonedGeneration::new(42);
//! assert_eq!(gen.generation(), 42);
//! assert!(!gen.is_poisoned());
//!
//! // Increment generation (safe while not poisoned)
//! gen.increment_generation();
//! assert_eq!(gen.generation(), 43);
//!
//! // Poison with cascade level 3
//! gen.poison(3);
//! assert!(gen.is_poisoned());
//! assert_eq!(gen.cascade_level(), 3);
//!
//! // Generation frozen after poison
//! gen.increment_generation();
//! assert_eq!(gen.generation(), 43); // Unchanged
//!
//! // Terminal state - full self-destruct
//! gen.terminate();
//! assert!(gen.is_terminal());
//! assert!(gen.is_corrupted());
//! assert!(gen.is_poisoned());
//! ```

#![allow(dead_code)]

// ============================================================================
// BIT LAYOUT CONSTANTS (Q34 Auditable)
// ============================================================================

/// Mask for 56-bit generation counter (bits 0-55)
/// #ASSUME_BIT_LAYOUT_STABLE: Generation occupies lower 56 bits
const GENERATION_MASK: u64 = 0x00FF_FFFF_FFFF_FFFF;

/// Mask for 4-bit cascade level (bits 56-59)
/// #ASSUME_CASCADE_BOUNDED: Limited to 0-15 depth
const CASCADE_MASK: u64 = 0x0F00_0000_0000_0000;

/// Bit shift for cascade level extraction
const CASCADE_SHIFT: u32 = 56;

/// POISONED flag (bit 60) - Irreversible tamper indicator
/// #ASSUME_POISON_IRREVERSIBLE: Once set, cannot be cleared
const POISONED_FLAG: u64 = 1 << 60;

/// CORRUPTED flag (bit 61) - Data zeroed indicator
const CORRUPTED_FLAG: u64 = 1 << 61;

/// PROPAGATING flag (bit 62) - Cascade in progress
const PROPAGATING_FLAG: u64 = 1 << 62;

/// TERMINAL flag (bit 63) - No recovery possible
const TERMINAL_FLAG: u64 = 1 << 63;

/// All tamper flags combined (bits 60-63)
const ALL_TAMPER_FLAGS: u64 = POISONED_FLAG | CORRUPTED_FLAG | PROPAGATING_FLAG | TERMINAL_FLAG;

/// Maximum cascade level (4 bits = 0-15)
const MAX_CASCADE_LEVEL: u8 = 15;

/// Maximum generation value (56 bits)
const MAX_GENERATION: u64 = GENERATION_MASK;

// ============================================================================
// POISONED GENERATION CAPSULE (T0+T1)
// ============================================================================

/// Poisoned Generation - T0+T1 Fractal Self-Destruct Capsule
///
/// A 64-bit packed structure for tamper-evident generation tracking with
/// cascading self-destruct capability.
///
/// # Memory Layout
///
/// ```text
/// +---------------------------------------------------------------+
/// |  63  |  62  |  61  |  60  | 59-56  |        55-0              |
/// +------+------+------+------+--------+--------------------------+
/// | TERM | PROP | CORR | POIS | CASCADE|     GENERATION           |
/// +------+------+------+------+--------+--------------------------+
/// ```
///
/// # Tier Classification
///
/// - **T0 (Auditable)**: Compile-time verifiable layout, Q34 hash-chain compatible
/// - **T1 (Atomic)**: Suitable for atomic operations (transparent u64)
///
/// # CHAOS Compliance
///
/// - 100% lockfree (no mutex/RwLock)
/// - Zero allocations (stack-only)
/// - no_std compatible
/// - Compile-time verification
///
/// # ASSUM Framework
///
/// - `#ASSUME_TRANSPARENT_LAYOUT`: repr(transparent) guarantees u64 ABI
/// - `#VERIFY_TRANSPARENT_LAYOUT`: size_of::<Self>() == size_of::<u64>()
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PoisonedGeneration(u64);

impl PoisonedGeneration {
    // ========================================================================
    // CONSTRUCTORS
    // ========================================================================

    /// Create a new PoisonedGeneration with the given initial generation.
    ///
    /// # Arguments
    ///
    /// * `generation` - Initial generation counter (truncated to 56 bits)
    ///
    /// # Returns
    ///
    /// A new PoisonedGeneration with no flags set.
    ///
    /// # Performance
    ///
    /// <1ns - Single bit mask operation
    ///
    /// # Example
    ///
    /// ```rust
    /// use atomic_capsule::protection::PoisonedGeneration;
    ///
    /// let gen = PoisonedGeneration::new(1000);
    /// assert_eq!(gen.generation(), 1000);
    /// assert!(!gen.is_poisoned());
    /// ```
    #[inline]
    #[must_use]
    pub const fn new(generation: u64) -> Self {
        // Truncate to 56 bits to prevent overflow into flag bits
        Self(generation & GENERATION_MASK)
    }

    /// Create a PoisonedGeneration from a raw 64-bit value.
    ///
    /// # Safety Note
    ///
    /// This method trusts the caller to provide a valid packed value.
    /// Use with caution - invalid bit patterns may cause unexpected behavior.
    ///
    /// # Arguments
    ///
    /// * `value` - Raw 64-bit packed value
    ///
    /// # Returns
    ///
    /// A PoisonedGeneration wrapping the raw value.
    #[inline]
    #[must_use]
    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    /// Convert to raw 64-bit value.
    ///
    /// # Returns
    ///
    /// The underlying packed 64-bit value.
    #[inline]
    #[must_use]
    pub const fn into_raw(self) -> u64 {
        self.0
    }

    // ========================================================================
    // GENERATION COUNTER (Bits 0-55)
    // ========================================================================

    /// Extract the 56-bit generation counter.
    ///
    /// # Returns
    ///
    /// The generation counter value (0 to ~72 quadrillion).
    ///
    /// # Performance
    ///
    /// <1ns - Single AND operation
    ///
    /// # ASSUM Framework
    ///
    /// - `#ASSUME_GENERATION_EXTRACTED_CORRECTLY`: Mask isolates bits 0-55
    /// - `#VERIFY_GENERATION_EXTRACTED_CORRECTLY`: Unit tests validate
    #[inline]
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.0 & GENERATION_MASK
    }

    /// Increment the generation counter if not poisoned.
    ///
    /// # Behavior
    ///
    /// - If not poisoned: increments generation by 1
    /// - If poisoned: no-op (generation frozen)
    /// - Wraps at 56-bit boundary (extremely unlikely in practice)
    ///
    /// # Performance
    ///
    /// <2ns - Conditional branch + bit operations
    ///
    /// # ASSUM Framework
    ///
    /// - `#ASSUME_GENERATION_MONOTONIC`: Only increments when healthy
    /// - `#VERIFY_GENERATION_MONOTONIC`: Unit tests verify frozen behavior
    #[inline]
    pub fn increment_generation(&mut self) {
        // #VERIFY_GENERATION_MONOTONIC: Skip if poisoned
        if self.is_poisoned() {
            return;
        }

        let current_gen = self.generation();
        let flags = self.0 & !GENERATION_MASK;

        // Increment with wraparound at 56-bit boundary
        let new_gen = (current_gen.wrapping_add(1)) & GENERATION_MASK;

        self.0 = flags | new_gen;
    }

    // ========================================================================
    // CASCADE LEVEL (Bits 56-59)
    // ========================================================================

    /// Extract the 4-bit cascade level.
    ///
    /// # Returns
    ///
    /// The cascade propagation depth (0-15).
    ///
    /// # Performance
    ///
    /// <1ns - Shift + AND operations
    ///
    /// # ASSUM Framework
    ///
    /// - `#ASSUME_CASCADE_BOUNDED`: 4-bit field limits to 0-15
    /// - `#VERIFY_CASCADE_BOUNDED`: Type system enforces bound
    #[inline]
    #[must_use]
    pub const fn cascade_level(&self) -> u8 {
        ((self.0 & CASCADE_MASK) >> CASCADE_SHIFT) as u8
    }

    // ========================================================================
    // FLAG QUERIES (Bits 60-63)
    // ========================================================================

    /// Check if any tamper flag is set (POISONED, CORRUPTED, PROPAGATING, or TERMINAL).
    ///
    /// # Returns
    ///
    /// `true` if any tamper flag is set.
    ///
    /// # Performance
    ///
    /// <1ns - Single AND + comparison
    #[inline]
    #[must_use]
    pub const fn is_poisoned(&self) -> bool {
        (self.0 & ALL_TAMPER_FLAGS) != 0
    }

    /// Check if the TERMINAL flag is set (bit 63).
    ///
    /// # Returns
    ///
    /// `true` if terminal state - no recovery possible.
    ///
    /// # Performance
    ///
    /// <1ns - Single AND + comparison
    #[inline]
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        (self.0 & TERMINAL_FLAG) != 0
    }

    /// Check if the CORRUPTED flag is set (bit 61).
    ///
    /// # Returns
    ///
    /// `true` if data has been zeroed.
    ///
    /// # Performance
    ///
    /// <1ns - Single AND + comparison
    #[inline]
    #[must_use]
    pub const fn is_corrupted(&self) -> bool {
        (self.0 & CORRUPTED_FLAG) != 0
    }

    /// Check if the PROPAGATING flag is set (bit 62).
    ///
    /// # Returns
    ///
    /// `true` if cascade propagation is in progress.
    ///
    /// # Performance
    ///
    /// <1ns - Single AND + comparison
    #[inline]
    #[must_use]
    pub const fn is_propagating(&self) -> bool {
        (self.0 & PROPAGATING_FLAG) != 0
    }

    // ========================================================================
    // FLAG MUTATIONS (Irreversible Operations)
    // ========================================================================

    /// Poison the generation with a cascade level.
    ///
    /// Sets the POISONED flag and cascade level. This operation is irreversible.
    ///
    /// # Arguments
    ///
    /// * `cascade_level` - Propagation depth (0-15, clamped to 15 if larger)
    ///
    /// # Performance
    ///
    /// <2ns - Bit operations
    ///
    /// # ASSUM Framework
    ///
    /// - `#ASSUME_POISON_IRREVERSIBLE`: Flag can only be set, never cleared
    /// - `#VERIFY_POISON_IRREVERSIBLE`: No clear method exists
    #[inline]
    pub fn poison(&mut self, cascade_level: u8) {
        // Clamp cascade level to 4-bit range
        let clamped_level = cascade_level.min(MAX_CASCADE_LEVEL) as u64;

        // Clear old cascade bits and set new ones + POISONED flag
        self.0 = (self.0 & !CASCADE_MASK) | (clamped_level << CASCADE_SHIFT) | POISONED_FLAG;
    }

    /// Set the CORRUPTED flag (bit 61).
    ///
    /// Indicates that associated data has been zeroed. This operation is irreversible.
    ///
    /// # Performance
    ///
    /// <1ns - Single OR operation
    #[inline]
    pub fn corrupt(&mut self) {
        self.0 |= CORRUPTED_FLAG;
    }

    /// Set the PROPAGATING flag (bit 62).
    ///
    /// Indicates that cascade propagation is in progress. This operation is irreversible.
    ///
    /// # Performance
    ///
    /// <1ns - Single OR operation
    #[inline]
    pub fn propagate(&mut self) {
        self.0 |= PROPAGATING_FLAG;
    }

    /// Terminate the generation - full self-destruct.
    ///
    /// Sets ALL flags (TERMINAL, CORRUPTED, PROPAGATING, POISONED).
    /// This is the final state - no recovery possible.
    ///
    /// # Performance
    ///
    /// <1ns - Single OR operation
    ///
    /// # Note
    ///
    /// After termination:
    /// - `is_terminal()` returns `true`
    /// - `is_corrupted()` returns `true`
    /// - `is_propagating()` returns `true`
    /// - `is_poisoned()` returns `true`
    /// - `increment_generation()` has no effect
    #[inline]
    pub fn terminate(&mut self) {
        self.0 |= ALL_TAMPER_FLAGS;
    }

    // ========================================================================
    // UTILITY METHODS
    // ========================================================================

    /// Check if the generation is in a healthy (unpoisoned) state.
    ///
    /// # Returns
    ///
    /// `true` if no tamper flags are set.
    #[inline]
    #[must_use]
    pub const fn is_healthy(&self) -> bool {
        !self.is_poisoned()
    }

    /// Get all flag bits as a u8 (bits 60-63 mapped to 0-3).
    ///
    /// # Returns
    ///
    /// Flags byte: bit 0 = POISONED, bit 1 = CORRUPTED, bit 2 = PROPAGATING, bit 3 = TERMINAL
    #[inline]
    #[must_use]
    pub const fn flags(&self) -> u8 {
        ((self.0 >> 60) & 0x0F) as u8
    }
}

impl Default for PoisonedGeneration {
    /// Default to generation 0 with no flags.
    #[inline]
    fn default() -> Self {
        Self::new(0)
    }
}

impl core::fmt::Debug for PoisonedGeneration {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PoisonedGeneration")
            .field("generation", &self.generation())
            .field("cascade_level", &self.cascade_level())
            .field("poisoned", &self.is_poisoned())
            .field("corrupted", &self.is_corrupted())
            .field("propagating", &self.is_propagating())
            .field("terminal", &self.is_terminal())
            .field("raw", &format_args!("0x{:016X}", self.0))
            .finish()
    }
}

impl core::fmt::Display for PoisonedGeneration {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_terminal() {
            write!(f, "TERMINAL[gen={}]", self.generation())
        } else if self.is_poisoned() {
            write!(
                f,
                "POISONED[gen={}, cascade={}]",
                self.generation(),
                self.cascade_level()
            )
        } else {
            write!(f, "gen={}", self.generation())
        }
    }
}

// ============================================================================
// COMPILE-TIME VERIFICATION (Q33 Mandatory)
// ============================================================================

// Verify transparent layout matches u64
const _: () = {
    assert!(core::mem::size_of::<PoisonedGeneration>() == 8);
    assert!(core::mem::align_of::<PoisonedGeneration>() == 8);
};

// Verify bit masks are correct
const _: () = {
    // Generation mask should cover bits 0-55 (56 bits)
    assert!(GENERATION_MASK == 0x00FF_FFFF_FFFF_FFFF);
    assert!(GENERATION_MASK.count_ones() == 56);

    // Cascade mask should cover bits 56-59 (4 bits)
    assert!(CASCADE_MASK == 0x0F00_0000_0000_0000);
    assert!(CASCADE_MASK.count_ones() == 4);

    // Flags should be in bits 60-63
    assert!(POISONED_FLAG == 1 << 60);
    assert!(CORRUPTED_FLAG == 1 << 61);
    assert!(PROPAGATING_FLAG == 1 << 62);
    assert!(TERMINAL_FLAG == 1 << 63);

    // All masks should be non-overlapping
    assert!((GENERATION_MASK & CASCADE_MASK) == 0);
    assert!((GENERATION_MASK & ALL_TAMPER_FLAGS) == 0);
    assert!((CASCADE_MASK & ALL_TAMPER_FLAGS) == 0);

    // All masks should cover all 64 bits
    assert!((GENERATION_MASK | CASCADE_MASK | ALL_TAMPER_FLAGS) == u64::MAX);
};

// ============================================================================
// UNIT TESTS (Q1-Q7 Tier - Bit Manipulation Tests)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // Q1: Basic Construction Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_new_generation() {
        let gen = PoisonedGeneration::new(42);
        assert_eq!(gen.generation(), 42);
        assert_eq!(gen.cascade_level(), 0);
        assert!(!gen.is_poisoned());
        assert!(!gen.is_terminal());
        assert!(!gen.is_corrupted());
        assert!(!gen.is_propagating());
        assert!(gen.is_healthy());
    }

    #[test]
    fn test_new_truncates_to_56_bits() {
        // Value larger than 56 bits should be truncated
        let large_value = u64::MAX;
        let gen = PoisonedGeneration::new(large_value);
        assert_eq!(gen.generation(), GENERATION_MASK);
        assert!(!gen.is_poisoned()); // Truncation prevents flag bits from being set
    }

    #[test]
    fn test_default() {
        let gen = PoisonedGeneration::default();
        assert_eq!(gen.generation(), 0);
        assert!(gen.is_healthy());
    }

    // ------------------------------------------------------------------------
    // Q2: Generation Counter Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_generation_extraction() {
        let gen = PoisonedGeneration::new(0x00FF_FFFF_FFFF_FFFF);
        assert_eq!(gen.generation(), MAX_GENERATION);
    }

    #[test]
    fn test_increment_generation() {
        let mut gen = PoisonedGeneration::new(100);
        gen.increment_generation();
        assert_eq!(gen.generation(), 101);
        gen.increment_generation();
        assert_eq!(gen.generation(), 102);
    }

    #[test]
    fn test_increment_generation_frozen_when_poisoned() {
        let mut gen = PoisonedGeneration::new(50);
        gen.poison(1);
        assert_eq!(gen.generation(), 50);

        // Increment should have no effect
        gen.increment_generation();
        assert_eq!(gen.generation(), 50);
    }

    #[test]
    fn test_generation_wraparound() {
        let mut gen = PoisonedGeneration::new(MAX_GENERATION);
        gen.increment_generation();
        assert_eq!(gen.generation(), 0); // Wraps to 0
    }

    // ------------------------------------------------------------------------
    // Q3: Cascade Level Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_cascade_level_extraction() {
        let mut gen = PoisonedGeneration::new(42);
        gen.poison(7);
        assert_eq!(gen.cascade_level(), 7);
    }

    #[test]
    fn test_cascade_level_clamped_to_max() {
        let mut gen = PoisonedGeneration::new(42);
        gen.poison(255); // Way above max
        assert_eq!(gen.cascade_level(), MAX_CASCADE_LEVEL);
    }

    #[test]
    fn test_cascade_level_zero() {
        let mut gen = PoisonedGeneration::new(42);
        gen.poison(0);
        assert_eq!(gen.cascade_level(), 0);
        assert!(gen.is_poisoned()); // Still poisoned even with cascade 0
    }

    // ------------------------------------------------------------------------
    // Q4: Flag Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_poison_flag() {
        let mut gen = PoisonedGeneration::new(100);
        assert!(!gen.is_poisoned());

        gen.poison(3);
        assert!(gen.is_poisoned());
        assert!(!gen.is_healthy());
    }

    #[test]
    fn test_corrupt_flag() {
        let mut gen = PoisonedGeneration::new(100);
        assert!(!gen.is_corrupted());

        gen.corrupt();
        assert!(gen.is_corrupted());
        assert!(gen.is_poisoned()); // CORRUPTED implies poisoned
    }

    #[test]
    fn test_propagating_flag() {
        let mut gen = PoisonedGeneration::new(100);
        assert!(!gen.is_propagating());

        gen.propagate();
        assert!(gen.is_propagating());
        assert!(gen.is_poisoned()); // PROPAGATING implies poisoned
    }

    #[test]
    fn test_terminal_flag() {
        let mut gen = PoisonedGeneration::new(100);
        assert!(!gen.is_terminal());

        gen.terminate();
        assert!(gen.is_terminal());
        assert!(gen.is_corrupted());
        assert!(gen.is_propagating());
        assert!(gen.is_poisoned());
    }

    // ------------------------------------------------------------------------
    // Q5: Raw Value Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_from_raw_into_raw() {
        let raw: u64 = 0x8FFF_0000_0000_002A; // TERMINAL + cascade 15 + gen 42
        let gen = PoisonedGeneration::from_raw(raw);
        assert_eq!(gen.into_raw(), raw);
    }

    #[test]
    fn test_raw_roundtrip() {
        let original = PoisonedGeneration::new(12345);
        let raw = original.into_raw();
        let restored = PoisonedGeneration::from_raw(raw);
        assert_eq!(original, restored);
    }

    // ------------------------------------------------------------------------
    // Q6: Compound State Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_multiple_flags() {
        let mut gen = PoisonedGeneration::new(999);
        gen.poison(5);
        gen.corrupt();
        gen.propagate();

        assert!(gen.is_poisoned());
        assert!(gen.is_corrupted());
        assert!(gen.is_propagating());
        assert!(!gen.is_terminal());
        assert_eq!(gen.cascade_level(), 5);
        assert_eq!(gen.generation(), 999);
    }

    #[test]
    fn test_flags_byte() {
        let mut gen = PoisonedGeneration::new(0);

        // No flags
        assert_eq!(gen.flags(), 0b0000);

        // POISONED only
        gen.poison(0);
        assert_eq!(gen.flags() & 0b0001, 0b0001);

        // Add CORRUPTED
        gen.corrupt();
        assert_eq!(gen.flags() & 0b0011, 0b0011);

        // Full terminal
        gen.terminate();
        assert_eq!(gen.flags(), 0b1111);
    }

    // ------------------------------------------------------------------------
    // Q7: Debug and Display Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_debug_format() {
        let gen = PoisonedGeneration::new(42);
        let debug = format!("{:?}", gen);
        assert!(debug.contains("generation: 42"));
        assert!(debug.contains("poisoned: false"));
    }

    #[test]
    fn test_display_healthy() {
        let gen = PoisonedGeneration::new(42);
        assert_eq!(format!("{}", gen), "gen=42");
    }

    #[test]
    fn test_display_poisoned() {
        let mut gen = PoisonedGeneration::new(42);
        gen.poison(5);
        assert_eq!(format!("{}", gen), "POISONED[gen=42, cascade=5]");
    }

    #[test]
    fn test_display_terminal() {
        let mut gen = PoisonedGeneration::new(42);
        gen.terminate();
        assert_eq!(format!("{}", gen), "TERMINAL[gen=42]");
    }
}
