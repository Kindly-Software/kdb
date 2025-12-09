//! Error types for Clapi Core with comprehensive classification
//!
//! ## Error Classification Framework (E18)
//!
//! All errors are categorized into 5 classes:
//! 1. **TRANSIENT**: Safe to retry with backoff (temporary failures)
//! 2. **PERMANENT**: Require immediate alerting (persistent failures)
//! 3. **CONFIGURATION**: Code/config bugs requiring fixes
//! 4. **USER_ERROR**: Invalid input requiring user guidance
//! 5. **SECURITY**: Security-related errors requiring audit logging
//!
//! ## ASSUM Safety Assumptions
//!
//! #ASSUME_ERROR_CLASSIFICATION: All errors have clear retry semantics
//! #VERIFY_CLASSIFICATION: Unit tests validate is_retryable() for each error
//!
//! #ASSUME_NO_PANIC: All error paths return Result<T,E>, no unwrap() in production
//! #VERIFY_NO_PANIC: Clippy enforces Result handling, code review validates
//!
//! #ASSUME_STRUCTURED_LOGGING: Error context logged via tracing framework
//! #VERIFY_LOGGING: Integration tests validate log output format

use thiserror::Error;

/// Alert severity levels for operational monitoring
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertSeverity {
    /// Critical: System degraded, immediate action required
    Critical,
    /// High: Service impact, alert ops team
    High,
    /// Medium: Non-critical issue, investigate during business hours
    Medium,
    /// Low: Informational, no immediate action needed
    Low,
}

/// Error category for retry and alerting decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Transient errors: Safe to retry with exponential backoff
    Transient,
    /// Permanent errors: Alert required, no retry without intervention
    Permanent,
    /// Configuration errors: Code or config bug, requires fix
    Configuration,
    /// User errors: Invalid input, provide helpful guidance
    UserError,
    /// Security errors: Potential attack, log for audit
    Security,
}

/// Errors that can occur in Clapi Core operations
#[derive(Debug, Error, Clone, PartialEq)]
pub enum ClapiError {
    /// Budget exhausted - request rejected
    #[error("Budget exhausted: requested {requested}, available {available}")]
    BudgetExhausted {
        /// Amount requested
        requested: i64,
        /// Amount available
        available: i64,
    },

    /// Invalid cost amount (negative or overflow)
    #[error("Invalid cost: {0}")]
    InvalidCost(i64),

    /// All providers unavailable
    #[error("All providers unavailable")]
    NoProvidersAvailable,

    /// All providers unavailable (alternate name)
    #[error("All providers unavailable")]
    AllProvidersUnavailable,

    /// Provider health check failed
    #[error("Provider {provider_id} failed health check")]
    ProviderUnhealthy {
        /// Provider ID
        provider_id: u8,
    },

    /// Hash chain validation failed (tampering detected)
    #[error("Hash chain validation failed at entry {entry_index}")]
    HashChainCorrupted {
        /// Entry index where corruption detected
        entry_index: u64,
    },

    /// Invalid request format
    #[error("Invalid request: {reason}")]
    InvalidRequest {
        /// Reason for rejection
        reason: String,
    },

    /// Retry limit exceeded
    #[error("Retry limit exceeded after {attempts} attempts")]
    RetryLimitExceeded {
        /// Number of attempts made
        attempts: u32,
    },

    /// Epoch overflow
    #[error("Epoch tile full: cannot append more entries")]
    EpochFull,

    /// Provider error (upstream API failure)
    #[error("Provider error: {0}")]
    ProviderError(String),

    /// Unauthorized (invalid API key)
    #[error("Unauthorized: invalid API key")]
    Unauthorized,

    /// Request timeout
    #[error("Request timeout after {timeout_ms}ms")]
    Timeout {
        /// Timeout in milliseconds
        timeout_ms: u64,
    },

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// IO error
    #[error("IO error: {0}")]
    IoError(String),

    /// JSON parse error
    #[error("JSON parse error: {0}")]
    JsonError(String),

    /// Invalid provider ID
    #[error("Invalid provider ID: {0}")]
    InvalidProviderId(u16),

    /// Budget slots exhausted (metacapsule capacity reached)
    #[error("Budget slots exhausted: {current}/{max} slots used")]
    SlotsExhausted {
        /// Maximum number of slots
        max: usize,
        /// Current number of slots
        current: usize,
    },

    /// Invalid slot ID (out of bounds)
    #[error("Invalid slot ID: {slot_id} (max: {max})")]
    InvalidSlotId {
        /// Slot ID requested
        slot_id: usize,
        /// Maximum valid slot ID
        max: usize,
    },

    /// Slot not allocated (empty slot access)
    #[error("Slot {slot_id} not allocated")]
    SlotNotAllocated {
        /// Slot ID requested
        slot_id: usize,
    },

    /// No slots allocated (deallocate on empty)
    #[error("No slots allocated (cannot deallocate)")]
    NoSlotsAllocated,

    /// Query execution error
    #[error("Query error: {message}")]
    QueryError {
        /// Error message
        message: String,
    },

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {quota} requests per {window_duration_secs}s")]
    RateLimitExceeded {
        /// Request quota per window
        quota: u64,
        /// Window duration in seconds
        window_duration_secs: u64,
    },

    /// Rate limit exceeded with backpressure information
    #[error("Rate limit exceeded for user {user_id}: retry after {retry_after_ms}ms (quota: {quota}, throttle rate: {throttle_rate_percent:.1}%)")]
    RateLimitExceededWithBackpressure {
        /// User ID
        user_id: String,
        /// Retry after (milliseconds)
        retry_after_ms: u64,
        /// Request quota
        quota: u64,
        /// Current throttle rate (percentage, 0-100)
        throttle_rate_percent: f64,
    },

    /// Database error (KindlyDB)
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// Monthly quota exceeded
    #[error("Monthly quota exceeded: used {used}, limit {limit}")]
    QuotaExceeded {
        /// Requests used this month
        used: u64,
        /// Monthly quota limit
        limit: u64,
    },

    /// Burst detected (short-term spike protection)
    #[error("Burst detected: {count} requests in {window_secs}s (threshold: {threshold})")]
    BurstDetected {
        /// Number of requests in burst
        count: usize,
        /// Window duration in seconds
        window_secs: u64,
        /// Burst threshold
        threshold: usize,
    },

    /// Cost velocity exceeded (spend rate limit)
    #[error("Cost velocity exceeded: {velocity_cents_per_min} cents/min (threshold: {threshold_cents_per_min})")]
    CostVelocityExceeded {
        /// Current velocity (cents/minute)
        velocity_cents_per_min: u64,
        /// Threshold velocity (cents/minute)
        threshold_cents_per_min: u64,
    },

    /// Pattern detected (repeated sequences)
    #[error("Pattern detected: {matches}/{window} matching hashes (threshold: {threshold})")]
    PatternDetected {
        /// Number of matching hashes
        matches: u32,
        /// Window size
        window: usize,
        /// Match threshold
        threshold: u32,
    },

    /// Circuit breaker is open (client temporarily isolated)
    #[error("Circuit breaker open for client (cooldown: {cooldown_remaining}s)")]
    CircuitBreakerOpen {
        /// Cooldown remaining in seconds
        cooldown_remaining: u64,
    },
}

/// Result type for Clapi Core operations
pub type ClapiResult<T> = Result<T, ClapiError>;

// Implement From for reqwest::Error
impl From<reqwest::Error> for ClapiError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            ClapiError::Timeout { timeout_ms: 30000 }
        } else {
            ClapiError::ProviderError(err.to_string())
        }
    }
}

// Implement From for std::io::Error
impl From<std::io::Error> for ClapiError {
    fn from(err: std::io::Error) -> Self {
        ClapiError::IoError(err.to_string())
    }
}

// Implement From for serde_json::Error
impl From<serde_json::Error> for ClapiError {
    fn from(err: serde_json::Error) -> Self {
        ClapiError::JsonError(err.to_string())
    }
}

// ============================================================================
// E18: Error Classification Implementation
// ============================================================================

impl ClapiError {
    /// Returns true if error is safe to retry with exponential backoff
    ///
    /// # ASSUM Safety
    /// #ASSUME_RETRY_SAFE: Classified errors have deterministic retry behavior
    /// #VERIFY_RETRY_CORRECTNESS: Unit tests validate retry logic for each error
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            // TRANSIENT errors (safe to retry)
            Self::Timeout { .. }
            | Self::ProviderError(_)
            | Self::ProviderUnhealthy { .. }
            | Self::RateLimitExceeded { .. }
            | Self::RateLimitExceededWithBackpressure { .. }
            | Self::RetryLimitExceeded { .. } // Retry after backoff period
            | Self::NoProvidersAvailable
            | Self::AllProvidersUnavailable
            | Self::BurstDetected { .. } // Retry after burst subsides
            | Self::CircuitBreakerOpen { .. } // Retry after cooldown
        )
    }

    /// Returns true if error indicates permanent failure (requires intervention)
    pub fn is_permanent(&self) -> bool {
        matches!(
            self,
            // PERMANENT errors (alert required)
            Self::BudgetExhausted { .. }
            | Self::SlotsExhausted { .. }
            | Self::EpochFull
            | Self::HashChainCorrupted { .. }
            | Self::DatabaseError(_)
            | Self::QuotaExceeded { .. }
            | Self::CostVelocityExceeded { .. } // Spending control
            | Self::PatternDetected { .. } // Security pattern
        )
    }

    /// Returns true if error indicates a configuration or code bug
    pub fn is_bug(&self) -> bool {
        matches!(
            self,
            // CONFIGURATION errors (fix code/config)
            Self::InvalidCost(_)
            | Self::InvalidProviderId(_)
            | Self::InvalidSlotId { .. }
            | Self::SlotNotAllocated { .. }
            | Self::NoSlotsAllocated
            | Self::ConfigError(_)
            | Self::QueryError { .. }
        )
    }

    /// Returns true if error indicates user input error
    pub fn is_user_error(&self) -> bool {
        matches!(
            self,
            // USER_ERROR (provide guidance)
            Self::InvalidRequest { .. }
            | Self::Unauthorized
            | Self::JsonError(_)
        )
    }

    /// Returns true if error has security implications
    pub fn is_security_related(&self) -> bool {
        matches!(
            self,
            // SECURITY errors (audit logging required)
            Self::Unauthorized | Self::HashChainCorrupted { .. }
        )
    }

    /// Get error category for retry and alerting decisions
    pub fn category(&self) -> ErrorCategory {
        if self.is_security_related() {
            ErrorCategory::Security
        } else if self.is_bug() {
            ErrorCategory::Configuration
        } else if self.is_user_error() {
            ErrorCategory::UserError
        } else if self.is_permanent() {
            ErrorCategory::Permanent
        } else if self.is_retryable() {
            ErrorCategory::Transient
        } else {
            // Default to permanent for unknown errors (conservative)
            ErrorCategory::Permanent
        }
    }

    /// Get suggested action for operational response
    ///
    /// Returns helpful guidance for ops team or users
    pub fn suggested_action(&self) -> &'static str {
        match self {
            // TRANSIENT errors
            Self::Timeout { .. } => {
                "Retry request with exponential backoff (100ms → 1600ms)"
            }
            Self::ProviderError(_) => {
                "Check provider status page, retry after 5s backoff"
            }
            Self::ProviderUnhealthy { .. } => {
                "Provider circuit breaker open, retry after cooldown (60s)"
            }
            Self::RateLimitExceeded { .. }
            | Self::RateLimitExceededWithBackpressure { .. } => {
                "Reduce request rate or upgrade quota tier"
            }
            Self::RetryLimitExceeded { .. } => {
                "All retries exhausted, check provider availability"
            }
            Self::NoProvidersAvailable | Self::AllProvidersUnavailable => {
                "All providers circuit breakers open, wait 60s for recovery"
            }

            // PERMANENT errors
            Self::BudgetExhausted { .. } => "Increase budget allocation or upgrade tier",
            Self::SlotsExhausted { .. } => {
                "Increase slot capacity (currently at max 1M slots)"
            }
            Self::EpochFull => "Flush current epoch to disk and start new epoch",
            Self::HashChainCorrupted { .. } => {
                "CRITICAL: Audit trail tampering detected, investigate immediately"
            }
            Self::DatabaseError(_) => "Check database connection and disk space",
            Self::QuotaExceeded { .. } => "Monthly quota exceeded, upgrade plan or wait for reset",

            // CONFIGURATION errors
            Self::InvalidCost(_) => "Fix cost calculation logic (negative or overflow detected)",
            Self::InvalidProviderId(_) => "Fix provider ID mapping (invalid provider referenced)",
            Self::InvalidSlotId { .. } => {
                "Fix slot allocation logic (out-of-bounds slot access)"
            }
            Self::SlotNotAllocated { .. } => {
                "Fix slot lifecycle management (accessing unallocated slot)"
            }
            Self::NoSlotsAllocated => {
                "Fix slot deallocation logic (deallocate on empty registry)"
            }
            Self::ConfigError(_) => "Fix configuration file (invalid TOML or missing fields)",
            Self::QueryError { .. } => "Fix query parameters (invalid time range or filter)",

            // USER_ERROR
            Self::InvalidRequest { .. } => "Check API documentation for correct request format",
            Self::Unauthorized => "Provide valid API key in Authorization header",
            Self::JsonError(_) => "Fix JSON syntax (malformed request body)",

            // IO errors (context-dependent)
            Self::IoError(_) => "Check file permissions and disk space availability",

            // PHASE 2 LOOP ARMOR errors
            Self::BurstDetected { .. } => "Reduce request rate temporarily, retry after 10s cooldown",
            Self::CostVelocityExceeded { .. } => "Spending exceeds budget velocity, review cost controls",
            Self::PatternDetected { .. } => "Repeated pattern detected, possible attack or misconfiguration",

            // PHASE 3 LOOP ARMOR errors
            Self::CircuitBreakerOpen { .. } => "Client circuit breaker open due to high error rate, retry after cooldown",
        }
    }

    /// Get alert severity for operational monitoring
    ///
    /// Used by alerting systems (PagerDuty, Slack) to prioritize incidents
    pub fn alert_severity(&self) -> AlertSeverity {
        match self {
            // CRITICAL (immediate action required)
            Self::HashChainCorrupted { .. } => AlertSeverity::Critical,
            Self::DatabaseError(_) => AlertSeverity::Critical,
            Self::SlotsExhausted { .. } => AlertSeverity::Critical,

            // HIGH (service impact)
            Self::AllProvidersUnavailable | Self::NoProvidersAvailable => AlertSeverity::High,
            Self::EpochFull => AlertSeverity::High,
            Self::BudgetExhausted { .. } => AlertSeverity::High,

            // MEDIUM (non-critical issues)
            Self::ProviderUnhealthy { .. } => AlertSeverity::Medium,
            Self::RetryLimitExceeded { .. } => AlertSeverity::Medium,
            Self::QuotaExceeded { .. } => AlertSeverity::Medium,
            Self::RateLimitExceeded { .. } => AlertSeverity::Medium,
            Self::RateLimitExceededWithBackpressure { .. } => AlertSeverity::Medium,

            // LOW (informational, no immediate action)
            Self::Timeout { .. } => AlertSeverity::Low,
            Self::ProviderError(_) => AlertSeverity::Low,
            Self::InvalidRequest { .. } => AlertSeverity::Low,
            Self::Unauthorized => AlertSeverity::Low,
            Self::JsonError(_) => AlertSeverity::Low,
            Self::InvalidCost(_) => AlertSeverity::Low,
            Self::InvalidProviderId(_) => AlertSeverity::Low,
            Self::InvalidSlotId { .. } => AlertSeverity::Low,
            Self::SlotNotAllocated { .. } => AlertSeverity::Low,
            Self::NoSlotsAllocated => AlertSeverity::Low,
            Self::ConfigError(_) => AlertSeverity::Low,
            Self::QueryError { .. } => AlertSeverity::Low,
            Self::IoError(_) => AlertSeverity::Low,

            // PHASE 2 LOOP ARMOR (varying severity)
            Self::BurstDetected { .. } => AlertSeverity::Medium, // Rate spike
            Self::CostVelocityExceeded { .. } => AlertSeverity::High, // Budget control
            Self::PatternDetected { .. } => AlertSeverity::High, // Security concern

            // PHASE 3 LOOP ARMOR (varying severity)
            Self::CircuitBreakerOpen { .. } => AlertSeverity::Medium, // Client isolation
        }
    }

    /// Get recommended retry backoff duration in milliseconds
    ///
    /// Returns None if error should not be retried
    pub fn retry_backoff_ms(&self) -> Option<u64> {
        if !self.is_retryable() {
            return None;
        }

        match self {
            Self::Timeout { .. } => Some(100), // Start with 100ms
            Self::ProviderError(_) => Some(5000), // 5s for provider errors
            Self::ProviderUnhealthy { .. } => Some(60000), // 60s circuit breaker cooldown
            Self::RateLimitExceeded { .. } => Some(1000), // 1s for rate limits
            Self::RateLimitExceededWithBackpressure { retry_after_ms, .. } => Some(*retry_after_ms),
            Self::RetryLimitExceeded { .. } => Some(10000), // 10s after exhaustion
            Self::NoProvidersAvailable | Self::AllProvidersUnavailable => Some(60000), // 60s
            Self::BurstDetected { .. } => Some(10000), // 10s burst cooldown
            Self::CircuitBreakerOpen { cooldown_remaining } => Some(cooldown_remaining * 1000), // cooldown in ms
            _ => Some(1000), // Default 1s backoff
        }
    }
}
