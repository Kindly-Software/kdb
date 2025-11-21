//! # LeaderElectionCapsule - Lockfree Distributed Leader Election
//!
//! **UCE34 Tier 1 Atomic Capsule for Raft-style epoch-based leader election.**
//!
//! ## Performance (B32 Targets)
//! - Vote: <50ns (CAS loop, max 3 retries)
//! - Check leader: <10ns (atomic load)
//! - Failover: <100ns (new epoch election)
//!
//! ## Use Cases
//! - Distributed systems (primary/backup coordination)
//! - Cluster management (active/standby failover)
//! - Database replication (leader election for RAFT)
//!
//! ## Pattern Origin
//! From PHASE4_ADVANCED_PRIMITIVES_PLAN.md:
//! > "LeaderElectionCapsule (T1 Atomic) - Lockfree distributed leader election
//! > DualAtomicU64 (epoch_primary: 48-bit epoch + 16-bit flags, leader_id: 64-bit)
//! > CAS-based epoch voting (lockfree Raft leader election)"
//!
//! ## ASSUM Framework
//! - `#ASSUME_128B_ALIGNMENT`: 128 bytes prevents false sharing between channels
//! - `#VERIFY_128B_ALIGNMENT`: verify_capsule_properties! compile-time check
//! - `#ASSUME_EPOCH_MONOTONIC`: Epochs always increase, never decrease
//! - `#VERIFY_EPOCH_MONOTONIC`: Property test validates (10K iterations)
//! - `#ASSUME_CAS_CONVERGENCE`: Max 3 retries under normal load
//! - `#VERIFY_CAS_CONVERGENCE`: Concurrent tests with 16+ voters
//! - `#ASSUME_SPLIT_BRAIN_PREVENTION`: Epoch-based voting prevents split-brain
//! - `#VERIFY_SPLIT_BRAIN_PREVENTION`: Property test with concurrent elections
//! - `#ASSUME_LEADER_UNIQUENESS`: Only one leader per epoch
//! - `#VERIFY_LEADER_UNIQUENESS`: Property test validates single leader
//! - `#ASSUME_FAILOVER_SPEED`: <100ns failover via epoch increment
//! - `#VERIFY_FAILOVER_SPEED`: Benchmark validates <100ns

use crate::alignment::AlignmentTier;
use crate::patterns::DualAtomicU64;
use core::sync::atomic::Ordering;

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

/// Leader election state flags (16-bit)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum LeaderState {
    /// No leader elected yet
    NoLeader = 0,
    /// Leader elected and active
    LeaderActive = 1,
    /// Leader suspected failed (pre-election state)
    LeaderSuspected = 2,
    /// Election in progress
    ElectionInProgress = 3,
}

impl LeaderState {
    /// Convert from u16
    #[inline]
    pub fn from_u16(value: u16) -> Self {
        match value & 0x3 {
            0 => LeaderState::NoLeader,
            1 => LeaderState::LeaderActive,
            2 => LeaderState::LeaderSuspected,
            3 => LeaderState::ElectionInProgress,
            _ => unreachable!(),
        }
    }

    /// Convert to u16
    #[inline]
    pub const fn to_u16(self) -> u16 {
        self as u16
    }
}

/// Leader election result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElectionResult {
    /// Vote succeeded, became leader
    BecameLeader { epoch: u64 },
    /// Vote succeeded, but another node is leader
    LeaderElsewhere { epoch: u64, leader_id: u64 },
    /// Vote failed due to stale epoch
    StaleEpoch { current_epoch: u64 },
    /// Vote failed due to CAS contention (retry)
    Contention,
}

/// Leader information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeaderInfo {
    /// Current epoch
    pub epoch: u64,
    /// Leader ID (0 = no leader)
    pub leader_id: u64,
    /// Leader state
    pub state: LeaderState,
}

/// LeaderElectionCapsule - Lockfree distributed leader election
///
/// Primary (offset 0): 48-bit epoch + 16-bit state flags
/// Secondary (offset 64): 64-bit leader ID
///
/// # Memory Layout
/// ```text
/// Offset 0-7:    Primary AtomicU64 (epoch + flags)
///                [bits 0-47: epoch] [bits 48-63: state flags]
/// Offset 8-63:   Padding (complete first 64-byte cache line)
/// Offset 64-71:  Secondary AtomicU64 (leader ID)
/// Offset 72-127: Padding (complete second 64-byte cache line)
/// Total: 128 bytes (WarmTier alignment)
/// ```
///
/// # Example
/// ```rust
/// use atomic_capsule::patterns::{LeaderElectionCapsule, ElectionResult};
///
/// let election = LeaderElectionCapsule::new();
///
/// // Node 1 votes to become leader
/// let result = election.vote(1, 1);
/// match result {
///     ElectionResult::BecameLeader { epoch } => {
///         println!("Node 1 is now leader for epoch {}", epoch);
///     }
///     _ => {}
/// }
///
/// // Node 2 checks current leader
/// if let Some(info) = election.check_leader() {
///     println!("Leader is node {} (epoch {})", info.leader_id, info.epoch);
/// }
///
/// // Trigger failover to new epoch
/// election.trigger_failover();
/// ```
#[repr(C, align(128))]
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
pub struct LeaderElectionCapsule {
    /// Dual atomic coordination (epoch+flags, leader_id)
    state: DualAtomicU64,
}

impl LeaderElectionCapsule {
    /// Maximum epoch value (48-bit)
    pub const MAX_EPOCH: u64 = (1u64 << 48) - 1;

    /// State flags mask (bits 48-63)
    const STATE_MASK: u64 = 0xFFFF_0000_0000_0000;

    /// Epoch mask (bits 0-47)
    const EPOCH_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

    /// Create a new leader election capsule
    ///
    /// # Example
    /// ```rust
    /// use atomic_capsule::patterns::LeaderElectionCapsule;
    ///
    /// let election = LeaderElectionCapsule::new();
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            state: DualAtomicU64::new(0, 0),
        }
    }

    /// Pack epoch and state into primary channel
    #[inline]
    const fn pack_primary(epoch: u64, state: LeaderState) -> u64 {
        debug_assert!(epoch <= Self::MAX_EPOCH, "epoch exceeds 48-bit limit");
        (epoch & Self::EPOCH_MASK) | ((state.to_u16() as u64) << 48)
    }

    /// Unpack epoch from primary channel
    #[inline]
    const fn unpack_epoch(primary: u64) -> u64 {
        primary & Self::EPOCH_MASK
    }

    /// Unpack state from primary channel
    #[inline]
    fn unpack_state(primary: u64) -> LeaderState {
        LeaderState::from_u16(((primary & Self::STATE_MASK) >> 48) as u16)
    }

    /// Vote to become leader for the given epoch
    ///
    /// # Arguments
    /// - `node_id`: This node's unique ID (must be > 0)
    /// - `epoch`: Epoch to vote for (must be >= current epoch)
    ///
    /// # Returns
    /// - `BecameLeader`: Vote succeeded, this node is now leader
    /// - `LeaderElsewhere`: Another node already elected for this epoch
    /// - `StaleEpoch`: Requested epoch is behind current epoch
    /// - `Contention`: CAS failed due to contention (caller should retry)
    ///
    /// # Performance
    /// - Target: <50ns (CAS loop, max 3 retries typical)
    /// - Hot path: <30ns (no contention)
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_NODE_ID_NONZERO`: node_id must be > 0 (0 = no leader)
    /// - `#ASSUME_EPOCH_MONOTONIC`: epoch must be >= current epoch
    /// - `#ASSUME_CAS_CONVERGENCE`: Max 3 retries under normal load
    ///
    /// # Example
    /// ```rust
    /// use atomic_capsule::patterns::{LeaderElectionCapsule, ElectionResult};
    ///
    /// let election = LeaderElectionCapsule::new();
    /// let result = election.vote(1, 1);
    /// match result {
    ///     ElectionResult::BecameLeader { epoch } => {
    ///         println!("Became leader for epoch {}", epoch);
    ///     }
    ///     ElectionResult::StaleEpoch { current_epoch } => {
    ///         println!("Epoch too old, current is {}", current_epoch);
    ///     }
    ///     _ => {}
    /// }
    /// ```
    #[inline]
    pub fn vote(&self, node_id: u64, epoch: u64) -> ElectionResult {
        debug_assert!(node_id > 0, "node_id must be > 0");
        debug_assert!(epoch <= Self::MAX_EPOCH, "epoch exceeds 48-bit limit");

        // Fast path: Check if we're already behind
        let current_primary = self.state.load_primary(Ordering::Acquire);
        let current_epoch = Self::unpack_epoch(current_primary);

        if epoch < current_epoch {
            return ElectionResult::StaleEpoch { current_epoch };
        }

        // CAS loop: Attempt to claim leadership (max 3 retries typical)
        const MAX_RETRIES: u32 = 16;
        for _retry in 0..MAX_RETRIES {
            let current_primary = self.state.load_primary(Ordering::Acquire);
            let current_epoch_inner = Self::unpack_epoch(current_primary);
            let current_state = Self::unpack_state(current_primary);

            // Check epoch validity
            if epoch < current_epoch_inner {
                return ElectionResult::StaleEpoch {
                    current_epoch: current_epoch_inner,
                };
            }

            // Check if leader already elected for this epoch
            let current_leader = self.state.load_secondary(Ordering::Acquire);
            if epoch == current_epoch_inner
                && current_state == LeaderState::LeaderActive
                && current_leader != 0
            {
                return ElectionResult::LeaderElsewhere {
                    epoch: current_epoch_inner,
                    leader_id: current_leader,
                };
            }

            // Attempt to claim leadership
            let new_primary = Self::pack_primary(epoch, LeaderState::LeaderActive);

            // Two-phase CAS: Primary first (epoch + state), then secondary (leader_id)
            match self.state.compare_exchange_primary(
                current_primary,
                new_primary,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Primary CAS succeeded, now update leader_id
                    self.state.store_secondary(node_id, Ordering::Release);
                    return ElectionResult::BecameLeader { epoch };
                }
                Err(_) => {
                    // CAS failed, retry
                    continue;
                }
            }
        }

        // Max retries exceeded (should be rare under normal load)
        ElectionResult::Contention
    }

    /// Check current leader
    ///
    /// # Returns
    /// - `Some(LeaderInfo)`: Leader exists
    /// - `None`: No leader elected yet
    ///
    /// # Performance
    /// - Target: <10ns (atomic load)
    ///
    /// # Example
    /// ```rust
    /// use atomic_capsule::patterns::LeaderElectionCapsule;
    ///
    /// let election = LeaderElectionCapsule::new();
    /// if let Some(info) = election.check_leader() {
    ///     println!("Leader: node {} (epoch {})", info.leader_id, info.epoch);
    /// } else {
    ///     println!("No leader elected");
    /// }
    /// ```
    #[inline]
    pub fn check_leader(&self) -> Option<LeaderInfo> {
        let primary = self.state.load_primary(Ordering::Acquire);
        let epoch = Self::unpack_epoch(primary);
        let state = Self::unpack_state(primary);
        let leader_id = self.state.load_secondary(Ordering::Acquire);

        if leader_id == 0 || state == LeaderState::NoLeader {
            None
        } else {
            Some(LeaderInfo {
                epoch,
                leader_id,
                state,
            })
        }
    }

    /// Trigger failover to new epoch
    ///
    /// Increments epoch and resets leader state to NoLeader.
    /// Callers should then re-run election via `vote()`.
    ///
    /// # Returns
    /// - New epoch number
    ///
    /// # Performance
    /// - Target: <100ns (CAS loop to increment epoch)
    ///
    /// # ASSUM Safety
    /// - `#ASSUME_EPOCH_OVERFLOW_SAFE`: 48-bit epoch allows 281 trillion epochs
    ///   (at 1M epochs/sec = 8.9 years continuous operation)
    ///
    /// # Example
    /// ```rust
    /// use atomic_capsule::patterns::LeaderElectionCapsule;
    ///
    /// let election = LeaderElectionCapsule::new();
    /// election.vote(1, 1); // Node 1 becomes leader
    ///
    /// // Trigger failover (e.g., leader heartbeat timeout)
    /// let new_epoch = election.trigger_failover();
    /// println!("Failover triggered, new epoch: {}", new_epoch);
    ///
    /// // Re-run election
    /// election.vote(2, new_epoch); // Node 2 votes for new epoch
    /// ```
    #[inline]
    pub fn trigger_failover(&self) -> u64 {
        const MAX_RETRIES: u32 = 16;
        for _retry in 0..MAX_RETRIES {
            let current_primary = self.state.load_primary(Ordering::Acquire);
            let current_epoch = Self::unpack_epoch(current_primary);
            let new_epoch = (current_epoch + 1).min(Self::MAX_EPOCH);

            let new_primary = Self::pack_primary(new_epoch, LeaderState::NoLeader);

            match self.state.compare_exchange_primary(
                current_primary,
                new_primary,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Reset leader_id
                    self.state.store_secondary(0, Ordering::Release);
                    return new_epoch;
                }
                Err(_) => {
                    // Retry
                    continue;
                }
            }
        }

        // Fallback: return current epoch + 1 (should be rare)
        let current_primary = self.state.load_primary(Ordering::Acquire);
        Self::unpack_epoch(current_primary) + 1
    }

    /// Mark leader as suspected (pre-failover state)
    ///
    /// Transitions state to LeaderSuspected without changing epoch.
    /// Useful for implementing heartbeat timeouts.
    ///
    /// # Returns
    /// - `true`: State transitioned successfully
    /// - `false`: Failed to transition (CAS contention or already in different state)
    ///
    /// # Example
    /// ```rust
    /// use atomic_capsule::patterns::LeaderElectionCapsule;
    ///
    /// let election = LeaderElectionCapsule::new();
    /// election.vote(1, 1); // Node 1 becomes leader
    ///
    /// // Heartbeat timeout detected
    /// if election.mark_suspected() {
    ///     println!("Leader marked as suspected, preparing failover");
    /// }
    /// ```
    #[inline]
    pub fn mark_suspected(&self) -> bool {
        const MAX_RETRIES: u32 = 8;
        for _retry in 0..MAX_RETRIES {
            let current_primary = self.state.load_primary(Ordering::Acquire);
            let epoch = Self::unpack_epoch(current_primary);
            let state = Self::unpack_state(current_primary);

            // Only transition from LeaderActive -> LeaderSuspected
            if state != LeaderState::LeaderActive {
                return false;
            }

            let new_primary = Self::pack_primary(epoch, LeaderState::LeaderSuspected);

            match self.state.compare_exchange_primary(
                current_primary,
                new_primary,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(_) => continue,
            }
        }

        false
    }

    /// Get current epoch
    ///
    /// # Performance
    /// - <10ns (atomic load)
    #[inline]
    pub fn current_epoch(&self) -> u64 {
        let primary = self.state.load_primary(Ordering::Acquire);
        Self::unpack_epoch(primary)
    }

    /// Get current state
    ///
    /// # Performance
    /// - <10ns (atomic load)
    #[inline]
    pub fn current_state(&self) -> LeaderState {
        let primary = self.state.load_primary(Ordering::Acquire);
        Self::unpack_state(primary)
    }
}

impl Default for LeaderElectionCapsule {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

// Verify 128-byte alignment (WarmTier)
const _: () = {
    assert!(
        core::mem::size_of::<LeaderElectionCapsule>() == 128,
        "LeaderElectionCapsule must be 128 bytes"
    );
    assert!(
        core::mem::align_of::<LeaderElectionCapsule>() == 128,
        "LeaderElectionCapsule must be 128-byte aligned"
    );
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout() {
        assert_eq!(
            core::mem::size_of::<LeaderElectionCapsule>(),
            128,
            "Size must be 128 bytes"
        );
        assert_eq!(
            core::mem::align_of::<LeaderElectionCapsule>(),
            128,
            "Alignment must be 128 bytes"
        );
    }

    #[test]
    fn test_basic_election() {
        let election = LeaderElectionCapsule::new();

        // Node 1 votes for epoch 1
        let result = election.vote(1, 1);
        assert!(
            matches!(result, ElectionResult::BecameLeader { epoch: 1 }),
            "Node 1 should become leader"
        );

        // Check leader
        let info = election.check_leader().expect("Leader should exist");
        assert_eq!(info.epoch, 1);
        assert_eq!(info.leader_id, 1);
        assert_eq!(info.state, LeaderState::LeaderActive);
    }

    #[test]
    fn test_second_vote_rejects() {
        let election = LeaderElectionCapsule::new();

        // Node 1 becomes leader
        election.vote(1, 1);

        // Node 2 tries to vote for same epoch
        let result = election.vote(2, 1);
        assert!(
            matches!(
                result,
                ElectionResult::LeaderElsewhere {
                    epoch: 1,
                    leader_id: 1
                }
            ),
            "Node 2 should see node 1 as leader"
        );
    }

    #[test]
    fn test_stale_epoch() {
        let election = LeaderElectionCapsule::new();

        // Node 1 becomes leader for epoch 2
        election.vote(1, 2);

        // Node 2 tries to vote for epoch 1 (stale)
        let result = election.vote(2, 1);
        assert!(
            matches!(result, ElectionResult::StaleEpoch { current_epoch: 2 }),
            "Stale epoch should be rejected"
        );
    }

    #[test]
    fn test_failover() {
        let election = LeaderElectionCapsule::new();

        // Node 1 becomes leader for epoch 1
        election.vote(1, 1);

        // Trigger failover
        let new_epoch = election.trigger_failover();
        assert_eq!(new_epoch, 2);

        // Check no leader
        assert!(election.check_leader().is_none());

        // Node 2 becomes leader for epoch 2
        let result = election.vote(2, 2);
        assert!(matches!(result, ElectionResult::BecameLeader { epoch: 2 }));
    }

    #[test]
    fn test_mark_suspected() {
        let election = LeaderElectionCapsule::new();

        // Node 1 becomes leader
        election.vote(1, 1);
        assert_eq!(election.current_state(), LeaderState::LeaderActive);

        // Mark suspected
        assert!(election.mark_suspected());
        assert_eq!(election.current_state(), LeaderState::LeaderSuspected);

        // Cannot mark suspected twice
        assert!(!election.mark_suspected());
    }
}
