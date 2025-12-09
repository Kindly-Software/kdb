//! Error types for UBI distribution system

use core::fmt;

/// UBI distribution errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UbiError {
    /// Pool has insufficient funds for distribution
    InsufficientFunds {
        /// Amount requested
        requested: u64,
        /// Amount available
        available: u64,
    },

    /// Invalid Merkle proof provided
    InvalidMerkleProof {
        /// Citizen ID that failed verification
        citizen_id: u32,
    },

    /// Citizen has already claimed for this period
    AlreadyClaimed {
        /// Citizen ID
        citizen_id: u32,
        /// Block height of previous claim
        claimed_at: u64,
    },

    /// Fraud detection triggered (Sybil attack suspected)
    FraudDetected {
        /// Citizen ID flagged
        citizen_id: u32,
        /// Reason for flag
        reason: &'static str,
    },

    /// Circuit breaker is active (system protection)
    CircuitBreakerActive {
        /// Protection level (0-3)
        level: u8,
    },

    /// Invalid citizen ID
    InvalidCitizenId {
        /// Invalid ID value
        id: u32,
    },

    /// Arithmetic overflow detected
    ArithmeticOverflow {
        /// Operation that caused overflow
        operation: &'static str,
    },

    /// Invalid distribution period
    InvalidPeriod {
        /// Current block height
        current_height: u64,
        /// Expected distribution height
        expected_height: u64,
    },

    /// Treasury is locked (governance intervention)
    TreasuryLocked {
        /// Unlock block height
        unlock_height: u64,
    },

    /// Concurrent update conflict (CAS failure)
    ConcurrentUpdate,

    /// Invalid state transition
    InvalidStateTransition {
        /// Current state
        from: &'static str,
        /// Attempted state
        to: &'static str,
    },
}

impl fmt::Display for UbiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UbiError::InsufficientFunds { requested, available } => {
                write!(
                    f,
                    "Insufficient funds: requested {} but only {} available",
                    requested, available
                )
            }
            UbiError::InvalidMerkleProof { citizen_id } => {
                write!(f, "Invalid Merkle proof for citizen {}", citizen_id)
            }
            UbiError::AlreadyClaimed { citizen_id, claimed_at } => {
                write!(
                    f,
                    "Citizen {} already claimed at block {}",
                    citizen_id, claimed_at
                )
            }
            UbiError::FraudDetected { citizen_id, reason } => {
                write!(f, "Fraud detected for citizen {}: {}", citizen_id, reason)
            }
            UbiError::CircuitBreakerActive { level } => {
                write!(f, "Circuit breaker active at level {}", level)
            }
            UbiError::InvalidCitizenId { id } => {
                write!(f, "Invalid citizen ID: {}", id)
            }
            UbiError::ArithmeticOverflow { operation } => {
                write!(f, "Arithmetic overflow in operation: {}", operation)
            }
            UbiError::InvalidPeriod { current_height, expected_height } => {
                write!(
                    f,
                    "Invalid distribution period: current {} expected {}",
                    current_height, expected_height
                )
            }
            UbiError::TreasuryLocked { unlock_height } => {
                write!(f, "Treasury locked until block {}", unlock_height)
            }
            UbiError::ConcurrentUpdate => {
                write!(f, "Concurrent update conflict - retry operation")
            }
            UbiError::InvalidStateTransition { from, to } => {
                write!(f, "Invalid state transition from {} to {}", from, to)
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for UbiError {}

/// Result type for UBI operations
pub type Result<T> = core::result::Result<T, UbiError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = UbiError::InsufficientFunds {
            requested: 1000,
            available: 500,
        };
        assert_eq!(
            format!("{}", err),
            "Insufficient funds: requested 1000 but only 500 available"
        );
    }

    #[test]
    fn test_fraud_error() {
        let err = UbiError::FraudDetected {
            citizen_id: 12345,
            reason: "Multiple claims detected",
        };
        assert_eq!(
            format!("{}", err),
            "Fraud detected for citizen 12345: Multiple claims detected"
        );
    }
}
