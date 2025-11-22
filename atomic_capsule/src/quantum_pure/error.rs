//! Error types for pure-capsule quantum simulator

use std::fmt;

/// Errors for quantum pure-capsule operations
#[derive(Debug, Clone, PartialEq)]
pub enum QuantumPureError {
    /// Invalid number of qubits (must be 4-20)
    InvalidQubitCount {
        requested: usize,
        min: usize,
        max: usize,
    },

    /// Invalid qubit index (must be < num_qubits)
    InvalidQubitIndex {
        index: usize,
        num_qubits: usize,
    },

    /// Matrix is not unitary (U†U ≠ I)
    NonUnitaryMatrix {
        row: usize,
        col: usize,
        value: f64,
        expected: f64,
    },

    /// Normalization error (Σ|amplitude|² ≠ 1.0)
    NormalizationError {
        sum_squared: f64,
        tolerance: f64,
    },

    /// Measurement probability error (sum ≠ 1.0)
    InvalidProbabilities {
        sum: f64,
    },

    /// Circuit too deep (exceeds max gates)
    CircuitTooDeep {
        depth: usize,
        max_depth: usize,
    },

    /// Empty circuit (no gates to execute)
    EmptyCircuit,

    /// Invalid gate parameters
    InvalidGateParameters {
        gate_type: String,
        reason: String,
    },

    /// Unsupported gate type (Phase 2)
    UnsupportedGateType {
        gate_type: String,
    },
}

impl fmt::Display for QuantumPureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuantumPureError::InvalidQubitCount { requested, min, max } => {
                write!(
                    f,
                    "Invalid qubit count: requested {}, must be {}-{}",
                    requested, min, max
                )
            }
            QuantumPureError::InvalidQubitIndex { index, num_qubits } => {
                write!(
                    f,
                    "Invalid qubit index: {} (must be < {})",
                    index, num_qubits
                )
            }
            QuantumPureError::NonUnitaryMatrix { row, col, value, expected } => {
                write!(
                    f,
                    "Matrix not unitary: element [{}][{}] = {} (expected {})",
                    row, col, value, expected
                )
            }
            QuantumPureError::NormalizationError { sum_squared, tolerance } => {
                write!(
                    f,
                    "Normalization error: Σ|amplitude|² = {} (expected 1.0 ± {})",
                    sum_squared, tolerance
                )
            }
            QuantumPureError::InvalidProbabilities { sum } => {
                write!(
                    f,
                    "Invalid probabilities: sum = {} (expected 1.0)",
                    sum
                )
            }
            QuantumPureError::CircuitTooDeep { depth, max_depth } => {
                write!(
                    f,
                    "Circuit too deep: {} gates (max {})",
                    depth, max_depth
                )
            }
            QuantumPureError::EmptyCircuit => {
                write!(f, "Empty circuit: no gates to execute")
            }
            QuantumPureError::InvalidGateParameters { gate_type, reason } => {
                write!(
                    f,
                    "Invalid gate parameters for {}: {}",
                    gate_type, reason
                )
            }
            QuantumPureError::UnsupportedGateType { gate_type } => {
                write!(
                    f,
                    "Unsupported gate type: {}",
                    gate_type
                )
            }
        }
    }
}

impl std::error::Error for QuantumPureError {}

/// Result type for quantum pure-capsule operations
pub type QuantumPureResult<T> = Result<T, QuantumPureError>;
