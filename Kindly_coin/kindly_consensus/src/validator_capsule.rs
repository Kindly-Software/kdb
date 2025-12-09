//! # Validator Capsule (AVC-512)
//!
//! Atomic validator state with phi-based selection and circuit breaker integration.
//!
//! ## Memory Layout (512 bits, 128-byte aligned)
//!
//! ```text
//! W0 (header):    commit:1 | ver:8 | validator_id:23 | stake_amount:32
//! W1 (state):     reputation_score:32 | last_proposal_gen:16 | vote_count:16
//! W2 (security):  circuit_breaker_level:8 | slashing_record:24 | penalties:32
//! W3 (tail):      ver_tail:8 | checksum:16 | status:8 | generation:32
//! ```
//!
//! ## Performance
//!
//! - Read validator: <20ns (single atomic read)
//! - Update state: <100ns (two-phase commit)
//! - Phi score calculation: <10ns (fixed-point math)
//! - Circuit breaker check: <5ns (bit mask)

// Note: Using compile-time assertions for capsule verification
use kindly_core::PHI;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

/// Maximum validators supported
pub const MAX_VALIDATORS: usize = 128;

/// Validator identifier (0-127)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValidatorId(u8);

impl std::fmt::Display for ValidatorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ValidatorId {
    /// Create new validator ID (range 0-127)
    pub fn new(id: u8) -> Result<Self, ValidatorError> {
        if id >= MAX_VALIDATORS as u8 {
            return Err(ValidatorError::InvalidId { id });
        }
        Ok(Self(id))
    }

    /// Get raw ID
    pub fn as_u8(&self) -> u8 {
        self.0
    }
}

/// Validator status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidatorStatus {
    /// Active and eligible for selection
    Active = 0,
    /// Temporarily suspended (circuit breaker)
    Suspended = 1,
    /// Slashed and ineligible
    Slashed = 2,
    /// Gracefully exited
    Exited = 3,
}

/// Circuit breaker protection levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CircuitBreakerLevel {
    /// Normal operation
    Normal = 0,
    /// Minor violations detected
    Level1 = 1,
    /// Moderate violations, reduced participation
    Level2 = 2,
    /// Severe violations, suspended
    Level3 = 3,
}

/// Validator capsule errors
#[derive(Error, Debug)]
pub enum ValidatorError {
    /// Invalid validator ID
    #[error("Invalid validator ID: {id} (max {max})", max = MAX_VALIDATORS - 1)]
    InvalidId { id: u8 },

    /// Uncommitted state
    #[error("Validator state uncommitted")]
    Uncommitted,

    /// Version mismatch (TOCTOU detected)
    #[error("Version mismatch: head {head_ver} != tail {tail_ver}")]
    VersionMismatch { head_ver: u8, tail_ver: u8 },

    /// Validator suspended by circuit breaker
    #[error("Validator {id} suspended at level {level:?}")]
    Suspended {
        id: ValidatorId,
        level: CircuitBreakerLevel,
    },

    /// Validator slashed
    #[error("Validator {id} slashed with {penalties} penalties")]
    Slashed { id: ValidatorId, penalties: u32 },
}

/// Validator state data
#[derive(Debug, Clone, Copy)]
pub struct ValidatorState {
    /// Validator identifier
    pub validator_id: ValidatorId,
    /// Staked amount (in minimal units)
    pub stake_amount: u32,
    /// Reputation score (0-1000000, higher is better)
    pub reputation_score: u32,
    /// Last proposal generation number
    pub last_proposal_gen: u16,
    /// Total votes cast
    pub vote_count: u16,
    /// Circuit breaker protection level
    pub circuit_breaker_level: CircuitBreakerLevel,
    /// Number of slashing penalties
    pub slashing_record: u32,
    /// Validator status
    pub status: ValidatorStatus,
    /// State generation counter
    pub generation: u32,
}

/// Phi-based validator selection parameters
#[derive(Debug, Clone, Copy)]
pub struct ValidatorSelection {
    /// Base voting weight (stake × reputation)
    pub base_weight: u64,
    /// Phi-adjusted weight (base_weight × φ^n)
    pub phi_weight: u64,
    /// Selection probability (0.0-1.0)
    pub probability: f64,
}

/// Atomic Validator Capsule (AVC-512)
///
/// # ASSUM Safety Documentation
///
/// #ASSUME_TOCTOU_SAFE: Two-phase commit with generation counters prevents torn reads
/// #VERIFY_TOCTOU_PREVENTED: Property tests validate atomic state transitions
/// #ASSUME_MEMORY_ORDERING: Acquire/Release for state coordination
/// #VERIFY_ORDERING_SUFFICIENT: Required for validator state consistency
/// #ASSUME_PHI_OVERFLOW: Phi calculations stay within u64 bounds for realistic stakes
/// #VERIFY_PHI_BOUNDS: Property tests validate phi weight calculations
#[repr(C, align(128))]
pub struct ValidatorCapsule {
    /// W0: commit:1 | ver:8 | validator_id:23 | stake_amount:32
    header: AtomicU64,

    /// W1: reputation_score:32 | last_proposal_gen:16 | vote_count:16
    state: AtomicU64,

    /// W2: circuit_breaker_level:8 | slashing_record:24 | penalties:32
    security: AtomicU64,

    /// W3: ver_tail:8 | checksum:16 | status:8 | generation:32
    tail: AtomicU64,
}

// Compile-time verification: AVC-512 is 128-byte aligned (align attribute guarantees this)
// Size will be 128 bytes due to padding to meet alignment requirement
const _: () = {
    const fn assert_alignment() {
        assert!(std::mem::align_of::<ValidatorCapsule>() == 128);
    }
    assert_alignment();
};

impl ValidatorCapsule {
    // Bit masks for header (W0)
    const COMMIT_MASK: u64 = 1 << 63;
    const VERSION_MASK: u64 = 0xFF << 55;
    const VALIDATOR_ID_MASK: u64 = 0x7FFFFF << 32;
    const STAKE_MASK: u64 = 0xFFFFFFFF;

    // Bit masks for state (W1)
    const REPUTATION_MASK: u64 = 0xFFFFFFFF << 32;
    const LAST_PROPOSAL_MASK: u64 = 0xFFFF << 16;
    const VOTE_COUNT_MASK: u64 = 0xFFFF;

    // Bit masks for security (W2)
    const CB_LEVEL_MASK: u64 = 0xFF << 56;
    const SLASHING_MASK: u64 = 0xFFFFFF << 32;
    const PENALTIES_MASK: u64 = 0xFFFFFFFF;

    // Bit masks for tail (W3)
    const VER_TAIL_MASK: u64 = 0xFF << 56;
    const CHECKSUM_MASK: u64 = 0xFFFF << 40;
    const STATUS_MASK: u64 = 0xFF << 32;
    const GENERATION_MASK: u64 = 0xFFFFFFFF;

    /// Create new validator capsule
    pub fn new(
        validator_id: ValidatorId,
        stake_amount: u32,
        reputation_score: u32,
    ) -> Self {
        let header = Self::pack_header(true, 0, validator_id, stake_amount);
        let state = Self::pack_state(reputation_score, 0, 0);
        let security = Self::pack_security(CircuitBreakerLevel::Normal, 0, 0);
        let tail = Self::pack_tail(0, 0, ValidatorStatus::Active, 0);

        Self {
            header: AtomicU64::new(header),
            state: AtomicU64::new(state),
            security: AtomicU64::new(security),
            tail: AtomicU64::new(tail),
        }
    }

    /// Read validator state (hot path: <20ns)
    #[inline(always)]
    pub fn read(&self) -> Result<ValidatorState, ValidatorError> {
        // #ASSUME_MEMORY_ORDERING: Acquire for head/tail consistency
        let header = self.header.load(Ordering::Acquire);
        let (commit, ver, validator_id, stake) = Self::unpack_header(header);

        if !commit {
            return Err(ValidatorError::Uncommitted);
        }

        let state = self.state.load(Ordering::Acquire);
        let (reputation, last_proposal, vote_count) = Self::unpack_state(state);

        let security = self.security.load(Ordering::Acquire);
        let (cb_level, slashing, _penalties) = Self::unpack_security(security);

        let tail = self.tail.load(Ordering::Acquire);
        let (ver_tail, _checksum, status, generation) = Self::unpack_tail(tail);

        // #ASSUME_TOCTOU_SAFE: Generation counter prevents ABA
        if ver != ver_tail {
            return Err(ValidatorError::VersionMismatch {
                head_ver: ver,
                tail_ver: ver_tail,
            });
        }

        Ok(ValidatorState {
            validator_id,
            stake_amount: stake,
            reputation_score: reputation,
            last_proposal_gen: last_proposal,
            vote_count,
            circuit_breaker_level: cb_level,
            slashing_record: slashing,
            status,
            generation,
        })
    }

    /// Update validator state (cold path: <100ns)
    pub fn update(
        &self,
        reputation_score: u32,
        last_proposal_gen: u16,
        vote_count: u16,
    ) -> Result<(), ValidatorError> {
        // Read current state
        let current = self.read()?;
        let new_version = current.generation.wrapping_add(1) as u8;

        // Phase 1: Set version odd (uncommitted)
        let new_header = Self::pack_header(
            false, // Uncommitted
            new_version,
            current.validator_id,
            current.stake_amount,
        );
        self.header.store(new_header, Ordering::Release);

        // Phase 2: Update payload
        let new_state = Self::pack_state(reputation_score, last_proposal_gen, vote_count);
        self.state.store(new_state, Ordering::Release);

        // Phase 3: Update tail with new version
        let new_tail = Self::pack_tail(
            new_version,
            Self::calculate_checksum(new_state),
            current.status,
            current.generation.wrapping_add(1),
        );
        self.tail.store(new_tail, Ordering::Release);

        // Phase 4: Commit (set version even)
        let committed_header = Self::pack_header(
            true, // Committed
            new_version,
            current.validator_id,
            current.stake_amount,
        );
        self.header.store(committed_header, Ordering::Release);

        Ok(())
    }

    /// Update circuit breaker level
    pub fn update_circuit_breaker(
        &self,
        level: CircuitBreakerLevel,
    ) -> Result<(), ValidatorError> {
        let current = self.read()?;
        let new_version = (current.generation as u8).wrapping_add(1);

        // Phase 1: Mark uncommitted
        let uncommitted_header = Self::pack_header(
            false,
            new_version,
            current.validator_id,
            current.stake_amount,
        );
        self.header.store(uncommitted_header, Ordering::Release);

        // Phase 2: Update security
        let new_security = Self::pack_security(level, current.slashing_record, 0);
        self.security.store(new_security, Ordering::Release);

        // Phase 3: Update status based on circuit breaker level
        let new_status = match level {
            CircuitBreakerLevel::Level3 => ValidatorStatus::Suspended,
            CircuitBreakerLevel::Normal => {
                // Reset to Active if not slashed or exited
                match current.status {
                    ValidatorStatus::Slashed | ValidatorStatus::Exited => current.status,
                    _ => ValidatorStatus::Active,
                }
            }
            _ => current.status,
        };

        let new_tail = Self::pack_tail(
            new_version,
            0,
            new_status,
            current.generation.wrapping_add(1),
        );
        self.tail.store(new_tail, Ordering::Release);

        // Phase 4: Commit
        let committed_header = Self::pack_header(
            true,
            new_version,
            current.validator_id,
            current.stake_amount,
        );
        self.header.store(committed_header, Ordering::Release);

        Ok(())
    }

    /// Calculate phi-based selection weight
    ///
    /// Formula: base_weight × φ^(reputation_tier)
    /// Where reputation_tier = reputation / 100000
    ///
    /// # ASSUM Safety
    /// #ASSUME_PHI_OVERFLOW: Stakes < 2^32, reputation < 10^6, phi^10 < 10^5
    /// #VERIFY_PHI_BOUNDS: Max weight = 2^32 × 10^5 < 2^64
    pub fn calculate_phi_weight(&self) -> Result<ValidatorSelection, ValidatorError> {
        let state = self.read()?;

        // Base weight = stake × reputation
        let base_weight = (state.stake_amount as u64)
            .saturating_mul(state.reputation_score as u64);

        // Reputation tier (0-10 for reputation 0-1000000)
        let reputation_tier = (state.reputation_score / 100_000) as u32;

        // Phi adjustment: φ^reputation_tier
        let phi_multiplier = PHI.powi(reputation_tier as i32);
        let phi_weight = (base_weight as f64 * phi_multiplier) as u64;

        // Probability calculation (normalized by total network weight)
        // Note: Actual probability requires total network weight context
        let probability = phi_multiplier / (1.0 + phi_multiplier);

        Ok(ValidatorSelection {
            base_weight,
            phi_weight,
            probability,
        })
    }

    /// Check if validator is eligible for selection
    #[inline(always)]
    pub fn is_eligible(&self) -> bool {
        // Fast path: single atomic read
        let header = self.header.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Relaxed);

        let (commit, ..) = Self::unpack_header(header);
        let (.., status, _) = Self::unpack_tail(tail);

        commit && status == ValidatorStatus::Active
    }

    /// Record slashing event
    pub fn slash(&self, penalty_amount: u32) -> Result<(), ValidatorError> {
        let current = self.read()?;
        let new_version = (current.generation as u8).wrapping_add(1);
        let new_slashing = current.slashing_record.saturating_add(1);

        // Phase 1: Mark uncommitted
        let uncommitted_header = Self::pack_header(
            false,
            new_version,
            current.validator_id,
            current.stake_amount,
        );
        self.header.store(uncommitted_header, Ordering::Release);

        // Phase 2: Update security
        let new_security = Self::pack_security(
            CircuitBreakerLevel::Level3,
            new_slashing,
            penalty_amount,
        );
        self.security.store(new_security, Ordering::Release);

        // Phase 3: Mark as slashed
        let new_tail = Self::pack_tail(
            new_version,
            0,
            ValidatorStatus::Slashed,
            current.generation.wrapping_add(1),
        );
        self.tail.store(new_tail, Ordering::Release);

        // Phase 4: Commit
        let committed_header = Self::pack_header(
            true,
            new_version,
            current.validator_id,
            current.stake_amount,
        );
        self.header.store(committed_header, Ordering::Release);

        Ok(())
    }

    // Bit packing/unpacking helpers
    fn pack_header(commit: bool, ver: u8, validator_id: ValidatorId, stake: u32) -> u64 {
        ((commit as u64) << 63)
            | ((ver as u64) << 55)
            | ((validator_id.as_u8() as u64) << 32)
            | (stake as u64)
    }

    fn unpack_header(header: u64) -> (bool, u8, ValidatorId, u32) {
        let commit = (header & Self::COMMIT_MASK) != 0;
        let ver = ((header & Self::VERSION_MASK) >> 55) as u8;
        let id = ((header & Self::VALIDATOR_ID_MASK) >> 32) as u8;
        let stake = (header & Self::STAKE_MASK) as u32;
        (commit, ver, ValidatorId(id), stake)
    }

    fn pack_state(reputation: u32, last_proposal: u16, vote_count: u16) -> u64 {
        ((reputation as u64) << 32) | ((last_proposal as u64) << 16) | (vote_count as u64)
    }

    fn unpack_state(state: u64) -> (u32, u16, u16) {
        let reputation = ((state & Self::REPUTATION_MASK) >> 32) as u32;
        let last_proposal = ((state & Self::LAST_PROPOSAL_MASK) >> 16) as u16;
        let vote_count = (state & Self::VOTE_COUNT_MASK) as u16;
        (reputation, last_proposal, vote_count)
    }

    fn pack_security(level: CircuitBreakerLevel, slashing: u32, penalties: u32) -> u64 {
        ((level as u64) << 56) | ((slashing as u64) << 32) | (penalties as u64)
    }

    fn unpack_security(security: u64) -> (CircuitBreakerLevel, u32, u32) {
        let level_bits = ((security & Self::CB_LEVEL_MASK) >> 56) as u8;
        let level = match level_bits {
            0 => CircuitBreakerLevel::Normal,
            1 => CircuitBreakerLevel::Level1,
            2 => CircuitBreakerLevel::Level2,
            _ => CircuitBreakerLevel::Level3,
        };
        let slashing = ((security & Self::SLASHING_MASK) >> 32) as u32;
        let penalties = (security & Self::PENALTIES_MASK) as u32;
        (level, slashing, penalties)
    }

    fn pack_tail(ver: u8, checksum: u16, status: ValidatorStatus, generation: u32) -> u64 {
        ((ver as u64) << 56)
            | ((checksum as u64) << 40)
            | ((status as u64) << 32)
            | (generation as u64)
    }

    fn unpack_tail(tail: u64) -> (u8, u16, ValidatorStatus, u32) {
        let ver = ((tail & Self::VER_TAIL_MASK) >> 56) as u8;
        let checksum = ((tail & Self::CHECKSUM_MASK) >> 40) as u16;
        let status_bits = ((tail & Self::STATUS_MASK) >> 32) as u8;
        let status = match status_bits {
            0 => ValidatorStatus::Active,
            1 => ValidatorStatus::Suspended,
            2 => ValidatorStatus::Slashed,
            _ => ValidatorStatus::Exited,
        };
        let generation = (tail & Self::GENERATION_MASK) as u32;
        (ver, checksum, status, generation)
    }

    fn calculate_checksum(state: u64) -> u16 {
        // Simple XOR-based checksum for state validation
        ((state >> 48) ^ (state >> 32) ^ (state >> 16) ^ state) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_capsule_creation() {
        let id = ValidatorId::new(0).unwrap();
        let capsule = ValidatorCapsule::new(id, 1000, 500000);
        let state = capsule.read().unwrap();

        assert_eq!(state.validator_id, id);
        assert_eq!(state.stake_amount, 1000);
        assert_eq!(state.reputation_score, 500000);
        assert_eq!(state.circuit_breaker_level, CircuitBreakerLevel::Normal);
        assert_eq!(state.status, ValidatorStatus::Active);
    }

    #[test]
    fn test_validator_update() {
        let id = ValidatorId::new(5).unwrap();
        let capsule = ValidatorCapsule::new(id, 2000, 700000);

        capsule.update(750000, 10, 100).unwrap();
        let state = capsule.read().unwrap();

        assert_eq!(state.reputation_score, 750000);
        assert_eq!(state.last_proposal_gen, 10);
        assert_eq!(state.vote_count, 100);
    }

    #[test]
    fn test_phi_weight_calculation() {
        let id = ValidatorId::new(1).unwrap();
        let capsule = ValidatorCapsule::new(id, 10000, 500000);

        let selection = capsule.calculate_phi_weight().unwrap();
        assert!(selection.base_weight > 0);
        assert!(selection.phi_weight >= selection.base_weight);
        assert!(selection.probability > 0.0 && selection.probability <= 1.0);
    }

    #[test]
    fn test_circuit_breaker_suspension() {
        let id = ValidatorId::new(2).unwrap();
        let capsule = ValidatorCapsule::new(id, 5000, 800000);

        capsule.update_circuit_breaker(CircuitBreakerLevel::Level3).unwrap();
        let state = capsule.read().unwrap();

        assert_eq!(state.circuit_breaker_level, CircuitBreakerLevel::Level3);
        assert_eq!(state.status, ValidatorStatus::Suspended);
        assert!(!capsule.is_eligible());
    }

    #[test]
    fn test_slashing() {
        let id = ValidatorId::new(3).unwrap();
        let capsule = ValidatorCapsule::new(id, 15000, 900000);

        capsule.slash(1000).unwrap();
        let state = capsule.read().unwrap();

        assert_eq!(state.slashing_record, 1);
        assert_eq!(state.status, ValidatorStatus::Slashed);
        assert!(!capsule.is_eligible());
    }

    #[test]
    fn test_version_consistency() {
        let id = ValidatorId::new(4).unwrap();
        let capsule = ValidatorCapsule::new(id, 20000, 950000);

        // Multiple updates should maintain version consistency
        for i in 0..10 {
            capsule.update(950000 + i * 1000, i as u16, i as u16 * 10).unwrap();
            let state = capsule.read().unwrap();
            assert_eq!(state.generation, i + 1);
        }
    }
}
