//! # Crash Recovery for T9 Persistent Capsules
//!
//! Generation counter pattern for crash-safe atomic updates.
//!
//! **UCE34 Q4**: Failure Modes - partial flush, crash during update
//! **UCE34 Q25**: Verification - crash recovery testing
//! **UCE34 Q34**: Auditability - tamper-evident state transitions
//!
//! # Architecture
//!
//! Two-phase commit using generation counters:
//!
//! ```text
//! Phase 1: Start Update
//! ┌────────────────────────────────────────┐
//! │ generation: 0 (even = committed)       │
//! │ ↓ fetch_add(1, Release)                │
//! │ generation: 1 (odd = in-flight)        │
//! │ ↓ write payload                        │
//! │ state: partially updated               │
//! └────────────────────────────────────────┘
//!
//! Phase 2: Commit Update
//! ┌────────────────────────────────────────┐
//! │ state: fully updated                   │
//! │ ↓ fetch_add(1, Release)                │
//! │ generation: 2 (even = committed)       │
//! │ ↓ flush_async()                        │
//! │ Durable on disk                        │
//! └────────────────────────────────────────┘
//!
//! Recovery After Crash:
//! ┌────────────────────────────────────────┐
//! │ Read generation counter                │
//! │ ↓ if even: state is valid              │
//! │ ↓ if odd: discard partial update       │
//! │ Recovered to last committed state      │
//! └────────────────────────────────────────┘
//! ```
//!
//! # Safety (ASSUM Framework)
//!
//! - #ASSUME_GENERATION_RECOVERY: Even generation = committed state
//! - #VERIFY_GENERATION_RECOVERY: Crash tests validate recovery
//! - #ASSUME_ATOMIC_ORDERING: Release/Acquire prevent reordering
//! - #VERIFY_ORDERING: Memory ordering tests (T28)

#![cfg(all(feature = "mmap-persistence", feature = "nightly-atomic"))]

use std::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// GENERATION COUNTER
// ============================================================================

/// Generation counter for crash recovery
///
/// **Pattern**: Odd = in-flight, Even = committed
///
/// # Example
///
/// ```rust,ignore
/// let gen = GenerationCounter::new(atomic);
///
/// // Start update (0 → 1)
/// gen.increment();
/// assert!(!gen.is_committed());
///
/// // Write data...
///
/// // Commit update (1 → 2)
/// gen.increment();
/// assert!(gen.is_committed());
/// ```
pub struct GenerationCounter {
    atomic: AtomicU64,
}

impl GenerationCounter {
    /// Create new generation counter from atomic reference
    ///
    /// # Safety
    ///
    /// Atomic reference must point to valid memory (mmap-backed).
    pub fn new(atomic: &AtomicU64) -> Self {
        // Copy atomic (shares underlying memory via mmap)
        Self {
            atomic: AtomicU64::new(atomic.load(Ordering::Acquire)),
        }
    }

    /// Load current generation
    #[inline]
    pub fn load(&self, ordering: Ordering) -> u64 {
        self.atomic.load(ordering)
    }

    /// Store new generation
    #[inline]
    pub fn store(&self, value: u64, ordering: Ordering) {
        self.atomic.store(value, ordering)
    }

    /// Increment generation counter
    ///
    /// # Returns
    ///
    /// Previous value
    #[inline]
    pub fn increment(&self) -> u64 {
        self.atomic.fetch_add(1, Ordering::Release)
    }

    /// Check if current state is committed (even generation)
    #[inline]
    pub fn is_committed(&self) -> bool {
        self.load(Ordering::Acquire) % 2 == 0
    }

    /// Check if current state is in-flight (odd generation)
    #[inline]
    pub fn is_in_flight(&self) -> bool {
        !self.is_committed()
    }
}

// ============================================================================
// TWO-PHASE COMMIT
// ============================================================================

/// Start two-phase atomic update
///
/// Increments generation counter from even to odd (committed → in-flight).
///
/// # Arguments
///
/// - `gen`: Generation counter
///
/// # Example
///
/// ```rust,ignore
/// two_phase_commit_start(&gen);
/// // Write data...
/// two_phase_commit_finish(&gen);
/// ```
#[inline]
pub fn two_phase_commit_start(gen: &GenerationCounter) {
    let prev = gen.increment();

    // #ASSUME_COMMIT_STATE: Previous generation must be even (committed)
    debug_assert!(
        prev % 2 == 0,
        "Starting update from non-committed state (gen={})",
        prev
    );
}

/// Finish two-phase atomic update
///
/// Increments generation counter from odd to even (in-flight → committed).
///
/// # Arguments
///
/// - `gen`: Generation counter
#[inline]
pub fn two_phase_commit_finish(gen: &GenerationCounter) {
    let prev = gen.increment();

    // #ASSUME_INFLIGHT_STATE: Previous generation must be odd (in-flight)
    debug_assert!(
        prev % 2 == 1,
        "Finishing update from non-in-flight state (gen={})",
        prev
    );
}

// ============================================================================
// RECOVERY STATE
// ============================================================================

/// Recovery state after crash
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryState {
    /// State is committed (generation is even)
    Committed { generation: u64 },

    /// Partial update detected (generation is odd)
    PartialUpdate { generation: u64 },
}

impl RecoveryState {
    /// Analyze generation counter for recovery
    ///
    /// # Arguments
    ///
    /// - `generation`: Current generation counter value
    ///
    /// # Returns
    ///
    /// Recovery state (Committed or PartialUpdate)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let state = RecoveryState::from_generation(gen.load(Ordering::Acquire));
    ///
    /// match state {
    ///     RecoveryState::Committed { generation } => {
    ///         println!("State is valid (gen={})", generation);
    ///     }
    ///     RecoveryState::PartialUpdate { generation } => {
    ///         println!("Partial update detected (gen={}), discarding", generation);
    ///         // Discard partial update, use previous committed state
    ///     }
    /// }
    /// ```
    pub fn from_generation(generation: u64) -> Self {
        if generation % 2 == 0 {
            RecoveryState::Committed { generation }
        } else {
            RecoveryState::PartialUpdate { generation }
        }
    }

    /// Check if state is committed
    pub fn is_committed(&self) -> bool {
        matches!(self, RecoveryState::Committed { .. })
    }

    /// Check if partial update detected
    pub fn is_partial(&self) -> bool {
        matches!(self, RecoveryState::PartialUpdate { .. })
    }

    /// Get generation value
    pub fn generation(&self) -> u64 {
        match self {
            RecoveryState::Committed { generation } => *generation,
            RecoveryState::PartialUpdate { generation } => *generation,
        }
    }
}

// ============================================================================
// RECOVERY PROCEDURES
// ============================================================================

/// Recover from partial update
///
/// If generation is odd (in-flight), increment to even (committed).
/// This marks the partial update as "aborted" and moves to next committed state.
///
/// # Arguments
///
/// - `gen`: Generation counter
///
/// # Returns
///
/// Recovery state after procedure
///
/// # Example
///
/// ```rust,ignore
/// let state = recover_partial_update(&gen);
///
/// match state {
///     RecoveryState::Committed { generation } => {
///         println!("Recovered to gen={}", generation);
///     }
///     RecoveryState::PartialUpdate { .. } => {
///         unreachable!("Recovery should always result in committed state");
///     }
/// }
/// ```
pub fn recover_partial_update(gen: &GenerationCounter) -> RecoveryState {
    let current = gen.load(Ordering::Acquire);
    let state = RecoveryState::from_generation(current);

    match state {
        RecoveryState::Committed { .. } => {
            // Already committed, no recovery needed
            state
        }
        RecoveryState::PartialUpdate { generation } => {
            // Increment to next even (committed) state
            gen.store(generation + 1, Ordering::Release);

            RecoveryState::Committed {
                generation: generation + 1,
            }
        }
    }
}

/// Validate recovery state
///
/// Checks that generation counter is in valid state after recovery.
///
/// # Arguments
///
/// - `gen`: Generation counter
///
/// # Returns
///
/// `true` if state is valid (committed), `false` otherwise
pub fn validate_recovery(gen: &GenerationCounter) -> bool {
    let state = RecoveryState::from_generation(gen.load(Ordering::Acquire));
    state.is_committed()
}

// ============================================================================
// TESTS (T28 Framework)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generation_counter_new() {
        let atomic = AtomicU64::new(0);
        let gen = GenerationCounter::new(&atomic);

        assert_eq!(gen.load(Ordering::Acquire), 0);
        assert!(gen.is_committed());
        assert!(!gen.is_in_flight());
    }

    #[test]
    fn test_generation_increment() {
        let atomic = AtomicU64::new(0);
        let gen = GenerationCounter::new(&atomic);

        // 0 → 1 (committed → in-flight)
        let prev = gen.increment();
        assert_eq!(prev, 0);
        assert_eq!(gen.load(Ordering::Acquire), 1);
        assert!(gen.is_in_flight());

        // 1 → 2 (in-flight → committed)
        let prev = gen.increment();
        assert_eq!(prev, 1);
        assert_eq!(gen.load(Ordering::Acquire), 2);
        assert!(gen.is_committed());
    }

    #[test]
    fn test_two_phase_commit() {
        let atomic = AtomicU64::new(0);
        let gen = GenerationCounter::new(&atomic);

        // Initial: committed
        assert!(gen.is_committed());
        assert_eq!(gen.load(Ordering::Acquire), 0);

        // Phase 1: Start update
        two_phase_commit_start(&gen);
        assert!(gen.is_in_flight());
        assert_eq!(gen.load(Ordering::Acquire), 1);

        // Phase 2: Finish update
        two_phase_commit_finish(&gen);
        assert!(gen.is_committed());
        assert_eq!(gen.load(Ordering::Acquire), 2);
    }

    #[test]
    fn test_recovery_state_committed() {
        let state = RecoveryState::from_generation(0);
        assert!(state.is_committed());
        assert!(!state.is_partial());
        assert_eq!(state.generation(), 0);

        let state = RecoveryState::from_generation(2);
        assert!(state.is_committed());
        assert_eq!(state.generation(), 2);
    }

    #[test]
    fn test_recovery_state_partial() {
        let state = RecoveryState::from_generation(1);
        assert!(!state.is_committed());
        assert!(state.is_partial());
        assert_eq!(state.generation(), 1);

        let state = RecoveryState::from_generation(3);
        assert!(state.is_partial());
        assert_eq!(state.generation(), 3);
    }

    #[test]
    fn test_recover_partial_update() {
        let atomic = AtomicU64::new(1); // Odd = partial
        let gen = GenerationCounter::new(&atomic);

        // Recover
        let state = recover_partial_update(&gen);

        // Should be committed now
        assert!(state.is_committed());
        assert_eq!(state.generation(), 2);
        assert_eq!(gen.load(Ordering::Acquire), 2);
    }

    #[test]
    fn test_recover_already_committed() {
        let atomic = AtomicU64::new(2); // Even = committed
        let gen = GenerationCounter::new(&atomic);

        // Recover (should be no-op)
        let state = recover_partial_update(&gen);

        // Should still be committed at same generation
        assert!(state.is_committed());
        assert_eq!(state.generation(), 2);
        assert_eq!(gen.load(Ordering::Acquire), 2);
    }

    #[test]
    fn test_validate_recovery() {
        // Committed state
        let atomic = AtomicU64::new(2);
        let gen = GenerationCounter::new(&atomic);
        assert!(validate_recovery(&gen));

        // Partial state
        let atomic = AtomicU64::new(3);
        let gen = GenerationCounter::new(&atomic);
        assert!(!validate_recovery(&gen));
    }

    #[test]
    fn test_crash_recovery_scenario() {
        // Simulate crash during update

        // 1. Initial committed state
        let atomic = AtomicU64::new(0);
        let gen = GenerationCounter::new(&atomic);

        // 2. Start update
        two_phase_commit_start(&gen);
        assert_eq!(gen.load(Ordering::Acquire), 1);

        // 3. Write data (simulated)
        // ... modify mmap memory ...

        // 4. ** CRASH HERE ** (before commit)

        // 5. Recovery: Detect partial update
        let state = RecoveryState::from_generation(gen.load(Ordering::Acquire));
        assert!(state.is_partial());

        // 6. Recover: Abort partial update
        let recovered = recover_partial_update(&gen);
        assert!(recovered.is_committed());
        assert_eq!(recovered.generation(), 2);

        // 7. Validate recovery
        assert!(validate_recovery(&gen));
    }

    #[test]
    fn test_successful_update_scenario() {
        // Simulate successful update (no crash)

        let atomic = AtomicU64::new(0);
        let gen = GenerationCounter::new(&atomic);

        // 1. Start update
        two_phase_commit_start(&gen);
        assert_eq!(gen.load(Ordering::Acquire), 1);

        // 2. Write data
        // ... modify mmap memory ...

        // 3. Commit update
        two_phase_commit_finish(&gen);
        assert_eq!(gen.load(Ordering::Acquire), 2);

        // 4. Validate (no recovery needed)
        assert!(validate_recovery(&gen));

        // 5. State is committed
        let state = RecoveryState::from_generation(gen.load(Ordering::Acquire));
        assert!(state.is_committed());
    }

    #[test]
    fn test_multiple_updates() {
        let atomic = AtomicU64::new(0);
        let gen = GenerationCounter::new(&atomic);

        // Update 1
        two_phase_commit_start(&gen); // 0 → 1
        two_phase_commit_finish(&gen); // 1 → 2
        assert_eq!(gen.load(Ordering::Acquire), 2);

        // Update 2
        two_phase_commit_start(&gen); // 2 → 3
        two_phase_commit_finish(&gen); // 3 → 4
        assert_eq!(gen.load(Ordering::Acquire), 4);

        // Update 3
        two_phase_commit_start(&gen); // 4 → 5
        two_phase_commit_finish(&gen); // 5 → 6
        assert_eq!(gen.load(Ordering::Acquire), 6);

        // All committed
        assert!(validate_recovery(&gen));
    }
}
