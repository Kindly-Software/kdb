use thiserror::Error;

/// Errors that can occur during capsule operations
#[derive(Error, Debug, Clone, PartialEq)]
pub enum CapsuleError {
    #[error("Budget exhausted: required {required}, available {available}")]
    BudgetExhausted { required: i64, available: i64 },

    #[error("Invalid state transition from {from} to {to}")]
    InvalidStateTransition { from: String, to: String },

    #[error("Overflow in atomic operation: {operation}")]
    Overflow { operation: String },

    #[error("Invalid value: {message}")]
    InvalidValue { message: String },

    #[error("Concurrent modification detected (generation {expected} != {actual})")]
    ConcurrentModification { expected: u64, actual: u64 },

    #[error("WebSocket operation failed: {message}")]
    WebSocketError { message: String },
}

pub type CapsuleResult<T> = Result<T, CapsuleError>;
