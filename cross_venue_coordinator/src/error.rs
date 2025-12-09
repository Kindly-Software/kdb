//! Error types for cross-venue coordination
//!
//! Comprehensive error handling following Rust best practices with
//! structured error types and rich context information.

use crate::types::VenueId;

/// Coordination errors for cross-venue operations
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CoordinationError {
    /// Generation counter mismatch indicates concurrent modification
    #[error("Generation mismatch: expected {expected}, got {actual}")]
    GenerationMismatch { expected: u32, actual: u32 },

    /// Coordination timeout
    #[error("Coordination timeout after {timeout_ns}ns")]
    Timeout { timeout_ns: u64 },

    /// Invalid venue ID
    #[error("Invalid venue ID: {venue_id}, must be < {max_venues}")]
    InvalidVenue { venue_id: VenueId, max_venues: usize },

    /// System in maintenance mode
    #[error("System in maintenance mode")]
    MaintenanceMode,

    /// Emergency stop active
    #[error("Emergency stop active")]
    EmergencyStop,

    /// Circuit breaker active
    #[error("Circuit breaker active for venue {venue_id}: {reason}")]
    CircuitBreakerActive { venue_id: VenueId, reason: String },

    /// Venue unavailable
    #[error("Venue {venue_id} is unavailable")]
    VenueUnavailable { venue_id: VenueId },

    /// Arbitrage error
    #[error("Arbitrage error: {message}")]
    ArbitrageError { message: String },

    /// Invalid request
    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },

    /// Venue coordination failed
    #[error("Venue coordination failed")]
    VenueCoordinationFailed,

    /// Network error
    #[error("Network error: {message}")]
    NetworkError { message: String },

    /// Configuration error
    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },
}

/// Venue-specific errors
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VenueError {
    /// Invalid venue ID
    #[error("Invalid venue ID: {venue_id}, max venues: {max_venues}")]
    InvalidVenueId { venue_id: VenueId, max_venues: usize },

    /// Concurrent update detected
    #[error("Concurrent update detected for venue {venue_id}")]
    ConcurrentUpdate { venue_id: VenueId },

    /// Update failed
    #[error("Update failed for venue {venue_id}: {reason}")]
    UpdateFailed { venue_id: VenueId, reason: String },

    /// Coordination failed
    #[error("Coordination failed")]
    CoordinationFailed,

    /// Batch update failed
    #[error("Batch update failed for venues: {failed_venues:?}")]
    BatchUpdateFailed { failed_venues: Vec<VenueId> },

    /// Circuit breaker open
    #[error("Circuit breaker open for venue {venue_id}")]
    CircuitBreakerOpen { venue_id: VenueId },

    /// Circuit breaker error
    #[error("Circuit breaker error for venue {venue_id}: {error}")]
    CircuitBreakerError { venue_id: VenueId, error: String },

    /// Health check failed
    #[error("Health check failed for venue {venue_id}")]
    HealthCheckFailed { venue_id: VenueId },

    /// Venue maintenance mode
    #[error("Venue {venue_id} is in maintenance mode")]
    MaintenanceMode { venue_id: VenueId },

    /// Venue emergency stop
    #[error("Emergency stop active for venue {venue_id}")]
    EmergencyStop { venue_id: VenueId },
}

/// Result type for coordination operations
pub type CoordinationResult<T> = Result<T, CoordinationError>;

/// Result type for venue operations
pub type VenueResult<T> = Result<T, VenueError>;