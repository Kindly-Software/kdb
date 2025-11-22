//! Error types for T11 QuantumHybrid quantum simulation
//!
//! # ASSUM Safety
//!
//! - #ASSUME_ERROR_DETERMINISTIC: Quantum simulation errors are deterministic and reproducible
//! - #ASSUME_NO_PANIC: All quantum operations return Result (no panics in fast path)
//! - #VERIFY_ERROR_CONTEXT: Rich error context for debugging (qubit count, gate index, etc.)

use std::fmt;

/// Result type for quantum operations
pub type QuantumResult<T> = Result<T, QuantumError>;

/// Errors that can occur during quantum simulation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuantumError {
    /// Insufficient qubits allocated for the requested operation
    InsufficientQubits {
        /// Number of qubits required
        required: usize,
        /// Number of qubits available
        available: usize,
    },

    /// Circuit depth exceeded (too many gates for simulation)
    CircuitDepthExceeded {
        /// Maximum depth allowed
        max_depth: usize,
        /// Attempted depth
        attempted: usize,
    },

    /// Simulation error from qip library
    SimulationError(String),

    /// Invalid input parameter (e.g., n=0 for factorization)
    InvalidInput {
        /// Parameter name
        param: &'static str,
        /// Invalid value
        value: String,
        /// Expected constraint
        expected: &'static str,
    },

    /// Measurement failed (no computational basis state found)
    MeasurementFailed {
        /// Measurement description
        context: String,
    },

    /// Algorithm-specific error (e.g., Shor's failed to find period)
    AlgorithmError {
        /// Algorithm name
        algorithm: &'static str,
        /// Error description
        reason: String,
    },

    /// Qubit limit exceeded (classical simulation constraint)
    QubitLimitExceeded {
        /// Requested qubits
        requested: usize,
        /// Maximum supported
        max_qubits: usize,
    },

    /// Qubit index out of bounds
    QubitIndexOutOfBounds {
        /// Qubit index
        index: usize,
        /// Number of qubits
        num_qubits: usize,
    },

    /// Invalid operation (e.g., CNOT with same qubit)
    InvalidOperation(String),

    /// Invalid qubit count (e.g., 0 qubits)
    InvalidQubitCount(usize),

    /// Invalid qubit index (out of bounds)
    InvalidQubitIndex(usize, usize),

    /// Invalid gate parameters
    InvalidGate(String),
}

impl fmt::Display for QuantumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuantumError::InsufficientQubits { required, available } => {
                write!(
                    f,
                    "Insufficient qubits: required {} but only {} available",
                    required, available
                )
            }
            QuantumError::CircuitDepthExceeded { max_depth, attempted } => {
                write!(
                    f,
                    "Circuit depth exceeded: max {} but attempted {}",
                    max_depth, attempted
                )
            }
            QuantumError::SimulationError(msg) => {
                write!(f, "Quantum simulation error: {}", msg)
            }
            QuantumError::InvalidInput { param, value, expected } => {
                write!(
                    f,
                    "Invalid input parameter '{}': got '{}', expected {}",
                    param, value, expected
                )
            }
            QuantumError::MeasurementFailed { context } => {
                write!(f, "Measurement failed: {}", context)
            }
            QuantumError::AlgorithmError { algorithm, reason } => {
                write!(f, "{} algorithm error: {}", algorithm, reason)
            }
            QuantumError::QubitLimitExceeded { requested, max_qubits } => {
                write!(
                    f,
                    "Qubit limit exceeded: requested {} but max {} qubits supported (classical simulation limit)",
                    requested, max_qubits
                )
            }
            QuantumError::QubitIndexOutOfBounds { index, num_qubits } => {
                write!(
                    f,
                    "Qubit index {} out of bounds (num_qubits = {})",
                    index, num_qubits
                )
            }
            QuantumError::InvalidOperation(msg) => {
                write!(f, "Invalid operation: {}", msg)
            }
            QuantumError::InvalidQubitCount(count) => {
                write!(f, "Invalid qubit count: {}", count)
            }
            QuantumError::InvalidQubitIndex(index, max) => {
                write!(f, "Invalid qubit index: {} (max: {})", index, max)
            }
            QuantumError::InvalidGate(msg) => {
                write!(f, "Invalid gate: {}", msg)
            }
        }
    }
}

impl std::error::Error for QuantumError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = QuantumError::InsufficientQubits {
            required: 10,
            available: 5,
        };
        assert_eq!(
            err.to_string(),
            "Insufficient qubits: required 10 but only 5 available"
        );
    }

    #[test]
    fn test_error_clone() {
        let err1 = QuantumError::CircuitDepthExceeded {
            max_depth: 1000,
            attempted: 1500,
        };
        let err2 = err1.clone();
        assert_eq!(err1, err2);
    }

    #[test]
    fn test_invalid_input_error() {
        let err = QuantumError::InvalidInput {
            param: "n",
            value: "0".to_string(),
            expected: "> 1",
        };
        assert!(err.to_string().contains("Invalid input parameter"));
    }

    #[test]
    fn test_qubit_limit_exceeded() {
        let err = QuantumError::QubitLimitExceeded {
            requested: 30,
            max_qubits: 25,
        };
        assert!(err.to_string().contains("Qubit limit exceeded"));
        assert!(err.to_string().contains("30"));
        assert!(err.to_string().contains("25"));
    }
}
