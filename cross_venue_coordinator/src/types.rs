//! Core types for cross-venue coordination
//!
//! Type definitions and data structures used throughout the
//! cross-venue coordination system.

use serde::{Deserialize, Serialize};
use crate::error::{CoordinationError, VenueError};

/// Venue identifier type
pub type VenueId = usize;

/// Result type for coordination operations
pub type CoordinationResult<T> = Result<T, CoordinationError>;

/// Arbitrage opportunity detected by the scanner
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    /// Type of arbitrage opportunity
    pub opportunity_type: crate::arbitrage_integration::ArbitrageOpportunityType,

    /// Expected profit in basis points (1 bp = 0.01%)
    pub profit_bps: u32,

    /// Confidence level (0.0 to 1.0)
    pub confidence: f64,

    /// Expected execution latency in nanoseconds
    pub execution_latency_ns: u64,

    /// Additional market data and context
    pub market_data: String,
}

impl ArbitrageOpportunity {
    /// Create new arbitrage opportunity
    pub fn new(
        opportunity_type: crate::arbitrage_integration::ArbitrageOpportunityType,
        profit_bps: u32,
        confidence: f64,
        execution_latency_ns: u64,
        market_data: String,
    ) -> Self {
        Self {
            opportunity_type,
            profit_bps,
            confidence,
            execution_latency_ns,
            market_data,
        }
    }

    /// Calculate expected profit as percentage
    pub fn profit_percentage(&self) -> f64 {
        self.profit_bps as f64 / 100.0
    }

    /// Check if opportunity meets minimum thresholds
    pub fn is_viable(&self, min_profit_bps: u32, min_confidence: f64) -> bool {
        self.profit_bps >= min_profit_bps && self.confidence >= min_confidence
    }

    /// Calculate risk-adjusted return
    pub fn risk_adjusted_return(&self) -> f64 {
        self.profit_percentage() * self.confidence
    }
}

/// Coordination priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CoordinationPriority {
    /// Emergency operations (highest priority)
    Emergency = 0,
    /// High priority operations
    High = 1,
    /// Normal priority operations
    Normal = 2,
    /// Low priority operations
    Low = 3,
    /// Background operations (lowest priority)
    Background = 4,
}

impl Default for CoordinationPriority {
    fn default() -> Self {
        Self::Normal
    }
}

impl From<u8> for CoordinationPriority {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Emergency,
            1 => Self::High,
            2 => Self::Normal,
            3 => Self::Low,
            _ => Self::Background,
        }
    }
}

impl From<CoordinationPriority> for u8 {
    fn from(priority: CoordinationPriority) -> Self {
        priority as u8
    }
}

/// Venue state information
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VenueStatus {
    /// Venue is active and ready for trading
    Active,
    /// Venue is inactive
    Inactive,
    /// Venue is in maintenance mode
    Maintenance,
    /// Venue has connectivity issues
    Unstable,
    /// Venue is temporarily halted
    Halted,
    /// Emergency stop is active
    EmergencyStop,
}

impl VenueStatus {
    /// Check if venue is available for trading
    pub fn is_available(&self) -> bool {
        matches!(self, VenueStatus::Active)
    }

    /// Check if venue has issues
    pub fn has_issues(&self) -> bool {
        matches!(
            self,
            VenueStatus::Maintenance
                | VenueStatus::Unstable
                | VenueStatus::Halted
                | VenueStatus::EmergencyStop
        )
    }
}

/// Venue health metrics
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VenueHealth {
    /// Venue ID
    pub venue_id: VenueId,
    /// Current venue status
    pub status: VenueStatus,
    /// Success rate percentage (0.0 to 100.0)
    pub success_rate: f64,
    /// Average latency in nanoseconds
    pub avg_latency_ns: u64,
    /// Last update timestamp
    pub last_update_ns: u64,
    /// Error count in last window
    pub error_count: u32,
}

impl VenueHealth {
    /// Check if venue is healthy
    pub fn is_healthy(&self) -> bool {
        self.status.is_available() &&
        self.success_rate >= 95.0 &&
        self.avg_latency_ns < 1_000_000 // < 1ms
    }

    /// Calculate health score (0.0 to 1.0)
    pub fn health_score(&self) -> f64 {
        if !self.status.is_available() {
            return 0.0;
        }

        let success_factor = self.success_rate / 100.0;
        let latency_factor = if self.avg_latency_ns == 0 {
            1.0
        } else {
            (1_000_000.0 / self.avg_latency_ns as f64).min(1.0)
        };
        let error_factor = if self.error_count == 0 {
            1.0
        } else {
            (1.0 / (1.0 + self.error_count as f64 / 10.0)).max(0.1)
        };

        (success_factor + latency_factor + error_factor) / 3.0
    }
}

/// Coordination timing constraints
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimingConstraints {
    /// Maximum coordination latency in nanoseconds
    pub max_latency_ns: u64,
    /// Coordination timeout in nanoseconds
    pub timeout_ns: u64,
    /// Deadline for completion (timestamp)
    pub deadline_ns: Option<u64>,
}

impl TimingConstraints {
    /// Create new timing constraints
    pub fn new(max_latency_ns: u64, timeout_ns: u64) -> Self {
        Self {
            max_latency_ns,
            timeout_ns,
            deadline_ns: None,
        }
    }

    /// Set deadline for completion
    pub fn with_deadline(mut self, deadline_ns: u64) -> Self {
        self.deadline_ns = Some(deadline_ns);
        self
    }

    /// Check if constraints are met
    pub fn check_constraints(&self, latency_ns: u64, current_time_ns: u64) -> Result<(), CoordinationError> {
        if latency_ns > self.max_latency_ns {
            return Err(CoordinationError::Timeout { timeout_ns: latency_ns });
        }

        if let Some(deadline) = self.deadline_ns {
            if current_time_ns > deadline {
                return Err(CoordinationError::Timeout { timeout_ns: current_time_ns - deadline });
            }
        }

        Ok(())
    }
}

/// Coordination statistics
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoordinationStats {
    /// Total coordination operations
    pub total_operations: u64,
    /// Successful operations
    pub successful_operations: u64,
    /// Failed operations
    pub failed_operations: u64,
    /// Average latency in nanoseconds
    pub avg_latency_ns: u64,
    /// P95 latency in nanoseconds
    pub p95_latency_ns: u64,
    /// P99 latency in nanoseconds
    pub p99_latency_ns: u64,
    /// Operations per second
    pub operations_per_second: f64,
}

impl CoordinationStats {
    /// Create empty stats
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            avg_latency_ns: 0,
            p95_latency_ns: 0,
            p99_latency_ns: 0,
            operations_per_second: 0.0,
        }
    }

    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            0.0
        } else {
            (self.successful_operations as f64 / self.total_operations as f64) * 100.0
        }
    }

    /// Calculate failure rate
    pub fn failure_rate(&self) -> f64 {
        100.0 - self.success_rate()
    }

    /// Check if performance meets targets
    pub fn meets_targets(&self, min_success_rate: f64, max_avg_latency_ns: u64) -> bool {
        self.success_rate() >= min_success_rate && self.avg_latency_ns <= max_avg_latency_ns
    }
}

impl Default for CoordinationStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for venue selection algorithms
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VenueSelectionConfig {
    /// Minimum success rate for venue selection
    pub min_success_rate: f64,
    /// Maximum latency for venue selection
    pub max_latency_ns: u64,
    /// Venue health check interval in nanoseconds
    pub health_check_interval_ns: u64,
    /// Number of venues to consider for each operation
    pub max_venues_per_operation: usize,
    /// Enable load balancing across venues
    pub enable_load_balancing: bool,
    /// Venue failover timeout in nanoseconds
    pub failover_timeout_ns: u64,
}

impl Default for VenueSelectionConfig {
    fn default() -> Self {
        Self {
            min_success_rate: 95.0,
            max_latency_ns: 1_000_000, // 1ms
            health_check_interval_ns: 1_000_000_000, // 1 second
            max_venues_per_operation: 4,
            enable_load_balancing: true,
            failover_timeout_ns: 100_000_000, // 100ms
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arbitrage_opportunity() {
        use crate::arbitrage_integration::ArbitrageOpportunityType;

        let opp = ArbitrageOpportunity::new(
            ArbitrageOpportunityType::Simple { venue_a: 0, venue_b: 1 },
            50, // 0.5%
            0.95,
            500_000, // 500μs
            "Test opportunity".to_string(),
        );

        assert_eq!(opp.profit_percentage(), 0.5);
        assert!(opp.is_viable(40, 0.9));
        assert!(!opp.is_viable(60, 0.9));
        assert_eq!(opp.risk_adjusted_return(), 0.475); // 0.5 * 0.95
    }

    #[test]
    fn test_venue_health() {
        let health = VenueHealth {
            venue_id: 0,
            status: VenueStatus::Active,
            success_rate: 98.5,
            avg_latency_ns: 500_000,
            last_update_ns: 0,
            error_count: 1,
        };

        assert!(health.is_healthy());
        let score = health.health_score();
        assert!(score > 0.8 && score <= 1.0);
    }

    #[test]
    fn test_coordination_stats() {
        let mut stats = CoordinationStats::new();
        stats.total_operations = 100;
        stats.successful_operations = 95;
        stats.failed_operations = 5;

        assert_eq!(stats.success_rate(), 95.0);
        assert_eq!(stats.failure_rate(), 5.0);
        assert!(stats.meets_targets(90.0, 1_000_000));
    }

    #[test]
    fn test_timing_constraints() {
        let constraints = TimingConstraints::new(1_000_000, 5_000_000)
            .with_deadline(10_000_000);

        assert!(constraints.check_constraints(500_000, 5_000_000).is_ok());
        assert!(constraints.check_constraints(2_000_000, 5_000_000).is_err());
        assert!(constraints.check_constraints(500_000, 15_000_000).is_err());
    }
}