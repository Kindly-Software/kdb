//! Unified Type System for AtomicHedgeCapsule
//!
//! Consolidated type definitions providing single source of truth for all atomic hedge capsule types.
//! UCE32 Q28(Simplicity): Simple, consistent types that encode correctness in the type system.
//! UCE32 Q31(Rust): Strong typing, zero-cost abstractions, impossible states unrepresentable.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use thiserror::Error;

/// Unified hedge error type for comprehensive error handling
/// UCE32 Q31(Rust): Rich error context with operation details and recovery hints
#[derive(Error, Debug, Clone)]
pub enum HedgeError {
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),

    #[error("State update failed: {0}")]
    StateUpdateFailed(String),

    #[error("Emergency stop triggered: {0}")]
    EmergencyStopped(String),

    #[error("Operation timeout")]
    Timeout,

    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition { from: OrderState, to: OrderState },

    #[error("Numeric overflow: {operation} would overflow {max_value}")]
    NumericOverflow {
        operation: String,
        max_value: String,
    },

    #[error("Value out of bounds: {value} not in range [{min}, {max}]")]
    ValueOutOfBounds {
        value: String,
        min: String,
        max: String,
    },

    #[error("Validation failed: {field} with value {value} failed validation: {reason}")]
    ValidationFailed {
        field: String,
        value: String,
        reason: String,
    },

    #[error("Coordination failure: {operation} failed due to {reason}")]
    CoordinationFailure { operation: String, reason: String },

    #[error("Memory ordering violation: {operation} violated ordering constraints")]
    MemoryOrderingViolation { operation: String },

    #[error("Cache alignment error: {structure} not properly aligned")]
    CacheAlignmentError { structure: String },
}

/// Order state enumeration for individual order tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderState {
    PendingValidation = 0,
    Validated = 1,
    Submitted = 2,
    Acknowledged = 3,
    PartiallyFilled = 4,
    Filled = 5,
    Cancelled = 6,
    Rejected = 7,
    Expired = 8,
    Suspended = 9,
    PendingCancel = 10,
    PendingReplace = 11,
    Replaced = 12,
    Stopped = 13,
    Triggered = 14,
    Activated = 15,
    Unknown = 16,
}

/// Hedge state enumeration for overall hedge capsule state
/// UCE32 Q31(Rust): Type-safe state machine preventing impossible transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum HedgeState {
    Idle = 0,
    Building = 1,
    Active = 2,
    Unwinding = 3,
    Emergency = 4,
}

impl HedgeState {
    /// Check if state transition is valid
    /// UCE32 Q28(Simplicity): Simple state validation preventing invalid transitions
    pub fn can_transition_to(self, target: HedgeState) -> bool {
        use HedgeState::*;
        match (self, target) {
            // From Idle
            (Idle, Building) => true,
            // From Building
            (Building, Active) => true,
            (Building, Idle) => true,
            // From Active
            (Active, Unwinding) => true,
            (Active, Emergency) => true,
            // From Unwinding
            (Unwinding, Idle) => true,
            (Unwinding, Emergency) => true,
            // From Emergency
            (Emergency, Idle) => true,
            // Emergency can be reached from any state
            (_, Emergency) => true,
            // All other transitions invalid
            _ => false,
        }
    }

    /// Check if state is terminal (no further transitions expected)
    pub fn is_terminal(self) -> bool {
        matches!(self, HedgeState::Idle)
    }

    /// Check if state represents active operation
    pub fn is_active(self) -> bool {
        matches!(
            self,
            HedgeState::Active | HedgeState::Building | HedgeState::Unwinding
        )
    }

    /// Check if state is emergency
    pub fn is_emergency(self) -> bool {
        matches!(self, HedgeState::Emergency)
    }
}

impl From<u32> for OrderState {
    fn from(value: u32) -> Self {
        match value {
            0 => OrderState::PendingValidation,
            1 => OrderState::Validated,
            2 => OrderState::Submitted,
            3 => OrderState::Acknowledged,
            4 => OrderState::PartiallyFilled,
            5 => OrderState::Filled,
            6 => OrderState::Cancelled,
            7 => OrderState::Rejected,
            8 => OrderState::Expired,
            9 => OrderState::Suspended,
            10 => OrderState::PendingCancel,
            11 => OrderState::PendingReplace,
            12 => OrderState::Replaced,
            13 => OrderState::Stopped,
            14 => OrderState::Triggered,
            15 => OrderState::Activated,
            _ => OrderState::Unknown,
        }
    }
}

/// Entry order for hedge capsule
/// UCE32 Q31(Rust): Zero-cost abstractions with comprehensive validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryOrder {
    pub exchange: String,
    pub symbol: String,
    pub side: String,
    pub size: f64,
    pub price: Option<f64>,
    pub order_type: String,
}

impl EntryOrder {
    pub fn new(exchange: String, symbol: String, side: String, size: f64) -> Self {
        Self {
            exchange,
            symbol,
            side,
            size,
            price: None,
            order_type: "MARKET".to_string(),
        }
    }

    pub fn with_price(mut self, price: f64) -> Self {
        self.price = Some(price);
        self.order_type = "LIMIT".to_string();
        self
    }

    /// Check if entry order is valid
    /// UCE32 Q28(Simplicity): Simple validation that prevents invalid states
    pub fn is_valid(&self) -> bool {
        self.size > 0.0 && !self.symbol.is_empty() && !self.exchange.is_empty()
    }

    /// Get order priority (higher number = higher priority)
    pub fn priority(&self) -> u32 {
        match self.order_type.as_str() {
            "MARKET" => 100,
            "LIMIT" => 50,
            _ => 10,
        }
    }

    /// Calculate estimated execution time
    pub fn estimated_execution_time(&self) -> Duration {
        match self.order_type.as_str() {
            "MARKET" => Duration::from_millis(50),
            "LIMIT" => Duration::from_millis(500),
            _ => Duration::from_secs(1),
        }
    }
}

/// Bracket order for hedge capsule
/// UCE32 Q31(Rust): Enhanced bracket order with comprehensive validation and risk management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BracketOrder {
    pub symbol: String,
    pub exchange: String,
    pub entry_price: f64,
    pub stop_price: f64,
    pub target_price: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub size: f64,
    pub emergency_stop: Option<f64>,
}

impl BracketOrder {
    pub fn new(stop_loss: f64, take_profit: f64, size: f64) -> Self {
        Self {
            symbol: "BTCUSD".to_string(),
            exchange: "NDAX".to_string(),
            entry_price: (stop_loss + take_profit) / 2.0,
            stop_price: stop_loss,
            target_price: take_profit,
            stop_loss,
            take_profit,
            size,
            emergency_stop: None,
        }
    }

    pub fn with_emergency_stop(mut self, emergency_stop: f64) -> Self {
        self.emergency_stop = Some(emergency_stop);
        self
    }

    /// Calculate risk-reward ratio
    /// UCE32 Q28(Simplicity): Simple risk calculation that prevents invalid trades
    pub fn risk_reward_ratio(&self, entry_price: f64) -> Option<f64> {
        let risk = (entry_price - self.stop_price).abs();
        let reward = (self.target_price - entry_price).abs();
        if risk > 0.0 {
            Some(reward / risk)
        } else {
            None
        }
    }

    /// Validate bracket order parameters
    /// UCE32 Q31(Rust): Type-safe validation that prevents impossible states
    pub fn is_valid(&self) -> Result<(), HedgeError> {
        if self.size <= 0.0 {
            return Err(HedgeError::ValidationFailed {
                field: "size".to_string(),
                value: self.size.to_string(),
                reason: "Size must be positive".to_string(),
            });
        }

        if self.symbol.is_empty() {
            return Err(HedgeError::ValidationFailed {
                field: "symbol".to_string(),
                value: self.symbol.clone(),
                reason: "Symbol cannot be empty".to_string(),
            });
        }

        if self.exchange.is_empty() {
            return Err(HedgeError::ValidationFailed {
                field: "exchange".to_string(),
                value: self.exchange.clone(),
                reason: "Exchange cannot be empty".to_string(),
            });
        }

        if !self.stop_loss.is_finite() || !self.take_profit.is_finite() {
            return Err(HedgeError::ValidationFailed {
                field: "prices".to_string(),
                value: format!(
                    "stop_loss={}, take_profit={}",
                    self.stop_loss, self.take_profit
                ),
                reason: "Prices must be finite numbers".to_string(),
            });
        }

        // Validate emergency stop if present
        if let Some(emergency) = self.emergency_stop {
            if !emergency.is_finite() {
                return Err(HedgeError::ValidationFailed {
                    field: "emergency_stop".to_string(),
                    value: emergency.to_string(),
                    reason: "Emergency stop must be finite".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Calculate total position risk
    pub fn position_risk(&self) -> f64 {
        let risk_per_unit = (self.entry_price - self.stop_loss).abs();
        risk_per_unit * self.size
    }

    /// Calculate potential profit
    pub fn potential_profit(&self) -> f64 {
        let profit_per_unit = (self.take_profit - self.entry_price).abs();
        profit_per_unit * self.size
    }
}

/// Hedge execution result
#[derive(Debug, Clone)]
pub struct HedgeExecutionResult {
    pub success: bool,
    pub entry_filled: f64,
    pub stop_placed: bool,
    pub target_placed: bool,
    pub total_cost: f64,
    pub message: String,
}

impl HedgeExecutionResult {
    pub fn success(entry_filled: f64, total_cost: f64) -> Self {
        Self {
            success: true,
            entry_filled,
            stop_placed: true,
            target_placed: true,
            total_cost,
            message: "Hedge executed successfully".to_string(),
        }
    }

    pub fn failure(message: String) -> Self {
        Self {
            success: false,
            entry_filled: 0.0,
            stop_placed: false,
            target_placed: false,
            total_cost: 0.0,
            message,
        }
    }
}

/// Unified hedge state snapshot with comprehensive state tracking
/// UCE32 Q31(Rust): Zero-cost abstraction providing complete hedge state visibility
#[derive(Debug, Clone)]
pub struct HedgeStateSnapshot {
    pub entry_state: OrderState,
    pub stop_state: OrderState,
    pub target_state: OrderState,
    pub filled_size: f64,
    pub operation_count: u64,
    pub emergency_stopped: bool,
    // Enhanced fields from capsule.rs
    pub is_active: bool,
    pub is_emergency: bool,
    pub emergency_count: u32,
    pub generation: u64,
    pub entry_generation: u64,
    pub bracket_generation: u64,
    pub emergency_generation: u64,
    pub age_ms: u64,
}

impl HedgeStateSnapshot {
    /// Create a basic snapshot with minimal fields
    pub fn basic(
        entry_state: OrderState,
        stop_state: OrderState,
        target_state: OrderState,
        filled_size: f64,
        operation_count: u64,
        emergency_stopped: bool,
    ) -> Self {
        Self {
            entry_state,
            stop_state,
            target_state,
            filled_size,
            operation_count,
            emergency_stopped,
            is_active: entry_state != OrderState::PendingValidation,
            is_emergency: emergency_stopped,
            emergency_count: if emergency_stopped { 1 } else { 0 },
            generation: operation_count,
            entry_generation: operation_count,
            bracket_generation: operation_count,
            emergency_generation: if emergency_stopped {
                operation_count
            } else {
                0
            },
            age_ms: 0,
        }
    }

    /// Create an enhanced snapshot with all tracking fields
    pub fn enhanced(
        entry_state: OrderState,
        stop_state: OrderState,
        target_state: OrderState,
        filled_size: f64,
        operation_count: u64,
        emergency_stopped: bool,
        is_active: bool,
        emergency_count: u32,
        generation: u64,
        entry_generation: u64,
        bracket_generation: u64,
        emergency_generation: u64,
        age_ms: u64,
    ) -> Self {
        Self {
            entry_state,
            stop_state,
            target_state,
            filled_size,
            operation_count,
            emergency_stopped,
            is_active,
            is_emergency: emergency_stopped,
            emergency_count,
            generation,
            entry_generation,
            bracket_generation,
            emergency_generation,
            age_ms,
        }
    }

    /// Check if hedge is in a terminal state
    /// UCE32 Q28(Simplicity): Simple state classification for decision making
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.entry_state,
            OrderState::Filled
                | OrderState::Cancelled
                | OrderState::Rejected
                | OrderState::Expired
                | OrderState::Stopped
        )
    }

    /// Check if hedge is still processing
    pub fn is_processing(&self) -> bool {
        matches!(
            self.entry_state,
            OrderState::PendingValidation
                | OrderState::Validated
                | OrderState::Submitted
                | OrderState::Acknowledged
                | OrderState::PartiallyFilled
                | OrderState::PendingCancel
                | OrderState::PendingReplace
        )
    }

    /// Get completion percentage (0.0 to 1.0)
    pub fn completion_percentage(&self) -> f64 {
        if self.is_terminal() {
            1.0
        } else if matches!(self.entry_state, OrderState::PartiallyFilled) {
            // Estimate based on filled size (would need more context for exact calculation)
            0.5
        } else if self.is_processing() {
            0.1
        } else {
            0.0
        }
    }

    /// Get risk status assessment
    pub fn risk_status(&self) -> HedgeRiskStatus {
        if self.emergency_stopped {
            HedgeRiskStatus::Emergency
        } else if self.emergency_count > 3 {
            HedgeRiskStatus::HighRisk
        } else if self.emergency_count > 0 {
            HedgeRiskStatus::MediumRisk
        } else {
            HedgeRiskStatus::LowRisk
        }
    }
}

/// Risk status classification for hedge operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HedgeRiskStatus {
    LowRisk,
    MediumRisk,
    HighRisk,
    Emergency,
}

/// Simplified hedge status for UCE-32 Q28 API simplification
///
/// Single struct providing all essential hedge information
#[derive(Debug, Clone)]
pub struct HedgeStatus {
    /// Whether hedge is currently active
    pub is_active: bool,
    /// Whether emergency stop is engaged
    pub is_emergency: bool,
    /// Completion percentage (0.0 to 1.0)
    pub completion: f64,
    /// Amount filled so far
    pub filled_size: f64,
    /// Current risk assessment
    pub risk_level: HedgeRiskStatus,
}

/// Fluent builder for hedge capsules
///
/// UCE-32 Q28: Simplified fluent interface for common operations
#[derive(Debug, Clone)]
pub struct HedgeBuilder {
    symbol: String,
    exchange: Option<String>,
    size: Option<f64>,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    side: String,
    order_type: String,
    price: Option<f64>,
}

impl HedgeBuilder {
    /// Create new builder for a symbol
    pub fn new(symbol: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            exchange: None,
            size: None,
            stop_loss: None,
            take_profit: None,
            side: "Buy".to_string(),
            order_type: "MARKET".to_string(),
            price: None,
        }
    }

    /// Set the exchange
    pub fn on_exchange(mut self, exchange: &str) -> Self {
        self.exchange = Some(exchange.to_string());
        self
    }

    /// Set position size
    pub fn size(mut self, size: f64) -> Self {
        self.size = Some(size);
        self
    }

    /// Set stop loss level
    pub fn stop_loss(mut self, stop_loss: f64) -> Self {
        self.stop_loss = Some(stop_loss);
        self
    }

    /// Set take profit level
    pub fn take_profit(mut self, take_profit: f64) -> Self {
        self.take_profit = Some(take_profit);
        self
    }

    /// Set order side
    pub fn side(mut self, side: &str) -> Self {
        self.side = side.to_string();
        self
    }

    /// Set as limit order with price
    pub fn limit(mut self, price: f64) -> Self {
        self.price = Some(price);
        self.order_type = "LIMIT".to_string();
        self
    }

    /// Set as market order
    pub fn market(mut self) -> Self {
        self.order_type = "MARKET".to_string();
        self.price = None;
        self
    }

    /// Build the hedge capsule
    pub fn build(self) -> Result<crate::AtomicHedgeCapsule, HedgeError> {
        let exchange = self.exchange.unwrap_or_else(|| "NDAX".to_string());
        let size = self.size.ok_or_else(|| HedgeError::ValidationFailed {
            field: "size".to_string(),
            value: "None".to_string(),
            reason: "Size is required".to_string(),
        })?;
        let stop_loss = self.stop_loss.ok_or_else(|| HedgeError::ValidationFailed {
            field: "stop_loss".to_string(),
            value: "None".to_string(),
            reason: "Stop loss is required".to_string(),
        })?;
        let take_profit = self
            .take_profit
            .ok_or_else(|| HedgeError::ValidationFailed {
                field: "take_profit".to_string(),
                value: "None".to_string(),
                reason: "Take profit is required".to_string(),
            })?;

        crate::AtomicHedgeCapsule::create_hedge(
            &self.symbol,
            &exchange,
            size,
            stop_loss,
            take_profit,
        )
    }
}

impl HedgeStatus {
    /// Check if hedge is in a safe operating state
    pub fn is_safe(&self) -> bool {
        !self.is_emergency
            && matches!(
                self.risk_level,
                HedgeRiskStatus::LowRisk | HedgeRiskStatus::MediumRisk
            )
    }

    /// Check if hedge needs attention
    pub fn needs_attention(&self) -> bool {
        self.is_emergency
            || matches!(
                self.risk_level,
                HedgeRiskStatus::HighRisk | HedgeRiskStatus::Emergency
            )
    }

    /// Get human-readable status description
    pub fn description(&self) -> &'static str {
        if self.is_emergency {
            "Emergency stop engaged"
        } else if !self.is_active {
            "Inactive"
        } else if self.completion >= 1.0 {
            "Completed"
        } else if self.completion > 0.0 {
            "In progress"
        } else {
            "Ready"
        }
    }
}

impl fmt::Display for HedgeStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "HedgeStatus {{ {}, {:.1}% complete, filled: {:.4}, risk: {:?} }}",
            self.description(),
            self.completion * 100.0,
            self.filled_size,
            self.risk_level
        )
    }
}

impl fmt::Display for HedgeStateSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "HedgeState {{ entry: {:?}, stop: {:?}, target: {:?}, filled: {:.4}, ops: {}, active: {}, emergency: {} }}",
            self.entry_state,
            self.stop_state,
            self.target_state,
            self.filled_size,
            self.operation_count,
            self.is_active,
            self.emergency_stopped
        )
    }
}

// ============================================================================
// ERROR HANDLING HELPERS - UCE-32 Q28 Simplification
// ============================================================================

impl HedgeError {
    /// Check if error is recoverable
    ///
    /// UCE-32 Q28: Simple boolean for error classification
    pub fn is_recoverable(&self) -> bool {
        match self {
            HedgeError::Timeout => true,
            HedgeError::StateUpdateFailed(_) => true,
            HedgeError::CoordinationFailure { .. } => true,
            HedgeError::EmergencyStopped(_) => false,
            HedgeError::NumericOverflow { .. } => false,
            HedgeError::ValidationFailed { .. } => false,
            HedgeError::InitializationFailed(_) => false,
            HedgeError::MemoryOrderingViolation { .. } => false,
            HedgeError::CacheAlignmentError { .. } => false,
            HedgeError::InvalidStateTransition { .. } => false,
            HedgeError::ValueOutOfBounds { .. } => false,
        }
    }

    /// Check if error requires immediate attention
    ///
    /// UCE-32 Q28: Clear priority classification
    pub fn is_critical(&self) -> bool {
        match self {
            HedgeError::EmergencyStopped(_) => true,
            HedgeError::NumericOverflow { .. } => true,
            HedgeError::MemoryOrderingViolation { .. } => true,
            HedgeError::CacheAlignmentError { .. } => true,
            _ => false,
        }
    }

    /// Get suggested action for error
    ///
    /// UCE-32 Q28: Simple guidance for error handling
    pub fn suggested_action(&self) -> &'static str {
        match self {
            HedgeError::Timeout => "Retry operation with longer timeout",
            HedgeError::StateUpdateFailed(_) => "Check system state and retry",
            HedgeError::CoordinationFailure { .. } => "Verify coordination parameters and retry",
            HedgeError::EmergencyStopped(_) => "Clear emergency condition before proceeding",
            HedgeError::NumericOverflow { .. } => "Reduce values to prevent overflow",
            HedgeError::ValidationFailed { .. } => {
                "Check input parameters and fix validation errors"
            }
            HedgeError::InitializationFailed(_) => "Review initialization parameters",
            HedgeError::MemoryOrderingViolation { .. } => "Contact support - system error",
            HedgeError::CacheAlignmentError { .. } => "Contact support - system error",
            HedgeError::InvalidStateTransition { .. } => "Check current state before transition",
            HedgeError::ValueOutOfBounds { .. } => "Provide values within valid range",
        }
    }

    /// Get error category for logging
    ///
    /// UCE-32 Q28: Simple categorization for monitoring
    pub fn category(&self) -> ErrorCategory {
        match self {
            HedgeError::Timeout => ErrorCategory::Transient,
            HedgeError::StateUpdateFailed(_) => ErrorCategory::Transient,
            HedgeError::CoordinationFailure { .. } => ErrorCategory::Transient,
            HedgeError::EmergencyStopped(_) => ErrorCategory::Operational,
            HedgeError::NumericOverflow { .. } => ErrorCategory::Configuration,
            HedgeError::ValidationFailed { .. } => ErrorCategory::Configuration,
            HedgeError::InitializationFailed(_) => ErrorCategory::Configuration,
            HedgeError::MemoryOrderingViolation { .. } => ErrorCategory::System,
            HedgeError::CacheAlignmentError { .. } => ErrorCategory::System,
            HedgeError::InvalidStateTransition { .. } => ErrorCategory::Operational,
            HedgeError::ValueOutOfBounds { .. } => ErrorCategory::Configuration,
        }
    }

    /// Create common timeout error
    ///
    /// UCE-32 Q28: Helper for frequent error case
    pub fn timeout() -> Self {
        HedgeError::Timeout
    }

    /// Create emergency stop error with reason
    ///
    /// UCE-32 Q28: Helper for emergency scenarios
    pub fn emergency(reason: &str) -> Self {
        HedgeError::EmergencyStopped(reason.to_string())
    }

    /// Create validation error helper
    ///
    /// UCE-32 Q28: Simplified validation error creation
    pub fn invalid_value(field: &str, value: &str, reason: &str) -> Self {
        HedgeError::ValidationFailed {
            field: field.to_string(),
            value: value.to_string(),
            reason: reason.to_string(),
        }
    }

    /// Create out of bounds error helper
    ///
    /// UCE-32 Q28: Common bounds checking error
    pub fn out_of_bounds(value: f64, min: f64, max: f64) -> Self {
        HedgeError::ValueOutOfBounds {
            value: value.to_string(),
            min: min.to_string(),
            max: max.to_string(),
        }
    }
}

/// Error category for simplified error handling
///
/// UCE-32 Q28: Simple classification for error handling strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Temporary errors that can be retried
    Transient,
    /// Configuration or input errors
    Configuration,
    /// Operational state errors
    Operational,
    /// System-level errors requiring support
    System,
}

impl ErrorCategory {
    /// Check if errors in this category should be retried
    pub fn should_retry(&self) -> bool {
        matches!(self, ErrorCategory::Transient)
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            ErrorCategory::Transient => "Temporary issue - retry recommended",
            ErrorCategory::Configuration => "Configuration error - fix inputs",
            ErrorCategory::Operational => "Operational issue - check state",
            ErrorCategory::System => "System error - contact support",
        }
    }
}

/// Result type helper for simplified error handling
///
/// UCE-32 Q28: Common result patterns
pub type HedgeResult<T> = std::result::Result<T, HedgeError>;

/// Helper trait for result handling
///
/// UCE-32 Q28: Simplified error handling patterns
pub trait HedgeResultExt<T> {
    /// Convert to simple success/failure
    fn is_success(&self) -> bool;

    /// Get error category if failed
    fn error_category(&self) -> Option<ErrorCategory>;

    /// Check if error is recoverable
    fn is_recoverable(&self) -> bool;

    /// Get suggested action for error
    fn suggested_action(&self) -> Option<&'static str>;
}

impl<T> HedgeResultExt<T> for HedgeResult<T> {
    fn is_success(&self) -> bool {
        self.is_ok()
    }

    fn error_category(&self) -> Option<ErrorCategory> {
        self.as_ref().err().map(|e| e.category())
    }

    fn is_recoverable(&self) -> bool {
        self.as_ref().err().is_none_or(|e| e.is_recoverable())
    }

    fn suggested_action(&self) -> Option<&'static str> {
        self.as_ref().err().map(|e| e.suggested_action())
    }
}
