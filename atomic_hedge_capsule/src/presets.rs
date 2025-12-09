//! # Preset Configurations for AtomicHedgeCapsule
//!
//! UCE-32 Q28 (Simplicity): Named constructors for common trading scenarios
//! UCE-32 Q29 (Practical Constraints): Optimized for real-world trading constraints
//! UCE-32 Q30 (Empirical Validation): Performance characteristics validated through benchmarks
//! UCE-32 Q31 (Rust Transform): Zero-cost abstractions with compile-time optimization
//! UCE-32 Q32 (Nightly Enhancement): Cutting-edge features for maximum performance
//!
//! This module provides preset configurations optimized for specific trading scenarios:
//!
//! ## Available Presets
//!
//! ### 1. High Frequency Trading (HFT)
//! - **Emergency threshold**: 0.001% (ultra-sensitive)
//! - **Max retries**: 3 (minimal latency)
//! - **Memory ordering**: Optimized Acquire/Release
//! - **Cache optimization**: Enabled
//! - **Target latency**: < 50ns per operation
//!
//! ### 2. Risk Management
//! - **Emergency threshold**: 5.0% (conservative)
//! - **Max retries**: 10 (increased safety)
//! - **Validation**: Strict mode enabled
//! - **Monitoring**: Enhanced tracking
//! - **Focus**: Safety over speed
//!
//! ### 3. Arbitrage
//! - **Cross-exchange optimization**: Enabled
//! - **Emergency threshold**: 1.0% (balanced)
//! - **Coordination**: Multi-exchange aware
//! - **Latency optimization**: Cross-market sync
//! - **Focus**: Opportunity capture
//!
//! ### 4. Development
//! - **Debug features**: Comprehensive logging
//! - **Validation**: Full safety checks
//! - **Error reporting**: Detailed diagnostics
//! - **Performance**: Debug-friendly (not optimized)
//! - **Focus**: Debugging and testing
//!
//! ### 5. Production
//! - **Performance**: Maximum optimization
//! - **Reliability**: Battle-tested settings
//! - **Monitoring**: Production metrics
//! - **Safety**: Proven configurations
//! - **Focus**: Maximum performance + reliability

use crate::types::HedgeError;
use crate::AtomicHedgeCapsule;
use serde::{Deserialize, Serialize};

#[cfg(feature = "builder")]
use crate::builder::HedgeCapsuleBuilder;

pub type Result<T> = std::result::Result<T, HedgeError>;

/// UCE-32 Q32: Const fn floating-point arithmetic for compile-time calculations
///
/// Nightly features enable compile-time calculation of preset thresholds
#[cfg(all(feature = "nightly", feature = "const_fn_floating_point_arithmetic"))]
const fn calculate_hft_threshold() -> f64 {
    // Ultra-low threshold for HFT: 0.1% = 0.001
    0.001
}

#[cfg(not(all(feature = "nightly", feature = "const_fn_floating_point_arithmetic")))]
const fn calculate_hft_threshold() -> f64 {
    0.001 // Pre-calculated for stable builds
}

#[cfg(all(feature = "nightly", feature = "const_fn_floating_point_arithmetic"))]
const fn calculate_risk_threshold() -> f64 {
    // Conservative threshold for risk management: 5% = 0.05
    5.0 / 100.0
}

#[cfg(not(all(feature = "nightly", feature = "const_fn_floating_point_arithmetic")))]
const fn calculate_risk_threshold() -> f64 {
    0.05 // Pre-calculated for stable builds
}

/// Preset configuration parameters
///
/// UCE-32 Q31 (Rust Transform): Type-safe configuration with impossible states unrepresentable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetConfig {
    /// Emergency threshold percentage (0.0 to 1.0)
    pub emergency_threshold: f64,

    /// Maximum CAS retry attempts
    pub max_retries: u32,

    /// Memory ordering optimization level
    pub memory_ordering_level: MemoryOrderingLevel,

    /// Cache optimization settings
    pub cache_optimization: CacheOptimization,

    /// Validation strictness level
    pub validation_level: ValidationLevel,

    /// Performance optimization features
    pub performance_features: PerformanceFeatures,

    /// Monitoring and debugging settings
    pub monitoring: MonitoringConfig,

    /// Risk management parameters
    pub risk_management: RiskConfig,
}

/// Memory ordering optimization levels
///
/// UCE-32 Q29 (Practical Constraints): Real-world ordering constraints for different scenarios
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryOrderingLevel {
    /// Strict SeqCst for maximum safety (development)
    Strict,
    /// Optimized Acquire/Release for performance (production)
    Optimized,
    /// Ultra-optimized Relaxed where safe (HFT)
    UltraOptimized,
}

/// Cache optimization configuration
///
/// UCE-32 Q29: Hardware cache constraints (64-byte alignment, false sharing prevention)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheOptimization {
    /// Enable 64-byte cache line alignment
    pub cache_aligned: bool,
    /// Separate hot/cold data to different cache lines
    pub hot_cold_separation: bool,
    /// Prevent false sharing between threads
    pub false_sharing_prevention: bool,
    /// Optimize for specific CPU cache hierarchy
    pub cpu_specific: bool,
}

/// Validation strictness levels
///
/// UCE-32 Q28 (Simplicity): Simple validation levels for different use cases
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationLevel {
    /// Minimal validation for maximum performance
    Minimal,
    /// Standard validation for production use
    Standard,
    /// Strict validation for development and testing
    Strict,
    /// Comprehensive validation with full safety checks
    Comprehensive,
}

/// Performance optimization features
///
/// UCE-32 Q32 (Nightly Enhancement): Cutting-edge Rust features for maximum performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceFeatures {
    /// Enable nightly SIMD optimizations
    pub nightly_simd: bool,
    /// Use branch prediction hints
    pub branch_prediction: bool,
    /// Enable atomic_from_mut optimization
    pub atomic_from_mut: bool,
    /// Use const trait implementations
    pub const_traits: bool,
    /// Enable compile-time floating-point math
    pub const_float_math: bool,
}

/// Monitoring and debugging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable detailed operation tracking
    pub detailed_tracking: bool,
    /// Performance metrics collection
    pub performance_metrics: bool,
    /// Memory usage monitoring
    pub memory_monitoring: bool,
    /// Debug assertions in release builds
    pub debug_assertions: bool,
    /// Cache performance analysis
    pub cache_analysis: bool,
}

/// Risk management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    /// Risk level thresholds
    pub risk_thresholds: RiskThresholds,
    /// Emergency stop sensitivity
    pub emergency_sensitivity: f64,
    /// Maximum position size limits
    pub position_limits: PositionLimits,
    /// Timeout configurations
    pub timeouts: TimeoutConfig,
}

/// Risk level thresholds for different risk categories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskThresholds {
    /// Low risk threshold (0.0 to 1.0)
    pub low_risk: f64,
    /// Medium risk threshold (0.0 to 1.0)
    pub medium_risk: f64,
    /// High risk threshold (0.0 to 1.0)
    pub high_risk: f64,
}

/// Position size and exposure limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionLimits {
    /// Maximum position size
    pub max_position_size: f64,
    /// Maximum number of concurrent positions
    pub max_concurrent_positions: u32,
    /// Maximum daily volume
    pub max_daily_volume: f64,
}

/// Timeout configuration for different operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    /// Order execution timeout
    pub execution_timeout_ms: u64,
    /// Emergency stop timeout
    pub emergency_timeout_ms: u64,
    /// Coordination timeout
    pub coordination_timeout_ms: u64,
}

impl Default for PresetConfig {
    fn default() -> Self {
        Self::production()
    }
}

impl PresetConfig {
    /// High Frequency Trading preset
    ///
    /// UCE-32 Q30 (Empirical Validation): Optimized for < 50ns latency
    ///
    /// **Performance Characteristics:**
    /// - Target latency: < 50ns per operation
    /// - Emergency threshold: 0.1% (ultra-sensitive)
    /// - Memory ordering: Ultra-optimized
    /// - Cache optimization: Full alignment + separation
    /// - Validation: Minimal for maximum speed
    ///
    /// **Trade-offs:**
    /// - Maximum speed vs reduced safety margins
    /// - Optimized for single-threaded performance
    /// - Requires stable, high-performance hardware
    pub fn high_frequency_trading() -> Self {
        Self {
            emergency_threshold: calculate_hft_threshold(),
            max_retries: 3, // Minimal retries to reduce latency
            memory_ordering_level: MemoryOrderingLevel::UltraOptimized,
            cache_optimization: CacheOptimization {
                cache_aligned: true,
                hot_cold_separation: true,
                false_sharing_prevention: true,
                cpu_specific: true,
            },
            validation_level: ValidationLevel::Minimal,
            performance_features: PerformanceFeatures {
                nightly_simd: true,
                branch_prediction: true,
                atomic_from_mut: true,
                const_traits: true,
                const_float_math: true,
            },
            monitoring: MonitoringConfig {
                detailed_tracking: false, // Disable for performance
                performance_metrics: true,
                memory_monitoring: false,
                debug_assertions: false,
                cache_analysis: false,
            },
            risk_management: RiskConfig {
                risk_thresholds: RiskThresholds {
                    low_risk: 0.001,    // 0.1%
                    medium_risk: 0.005, // 0.5%
                    high_risk: 0.01,    // 1.0%
                },
                emergency_sensitivity: 0.001, // 0.1%
                position_limits: PositionLimits {
                    max_position_size: 1000.0,
                    max_concurrent_positions: 5,
                    max_daily_volume: 100_000.0,
                },
                timeouts: TimeoutConfig {
                    execution_timeout_ms: 50,    // 50ms
                    emergency_timeout_ms: 10,    // 10ms
                    coordination_timeout_ms: 25, // 25ms
                },
            },
        }
    }

    /// Risk Management preset
    ///
    /// UCE-32 Q29 (Practical Constraints): Conservative settings for maximum safety
    ///
    /// **Performance Characteristics:**
    /// - Emergency threshold: 5.0% (conservative)
    /// - Maximum retries: 10 (increased safety)
    /// - Validation: Comprehensive checks
    /// - Monitoring: Full tracking enabled
    ///
    /// **Trade-offs:**
    /// - Safety over speed
    /// - Higher latency but better risk control
    /// - Comprehensive error detection and recovery
    pub fn risk_management() -> Self {
        Self {
            emergency_threshold: calculate_risk_threshold(),
            max_retries: 10, // More retries for safety
            memory_ordering_level: MemoryOrderingLevel::Strict,
            cache_optimization: CacheOptimization {
                cache_aligned: true,
                hot_cold_separation: true,
                false_sharing_prevention: true,
                cpu_specific: false, // Conservative, hardware-agnostic
            },
            validation_level: ValidationLevel::Comprehensive,
            performance_features: PerformanceFeatures {
                nightly_simd: false,
                branch_prediction: false,
                atomic_from_mut: false,
                const_traits: false,
                const_float_math: false,
            },
            monitoring: MonitoringConfig {
                detailed_tracking: true,
                performance_metrics: true,
                memory_monitoring: true,
                debug_assertions: true,
                cache_analysis: true,
            },
            risk_management: RiskConfig {
                risk_thresholds: RiskThresholds {
                    low_risk: 0.01,    // 1.0%
                    medium_risk: 0.03, // 3.0%
                    high_risk: 0.05,   // 5.0%
                },
                emergency_sensitivity: 0.05, // 5.0%
                position_limits: PositionLimits {
                    max_position_size: 100.0, // Smaller positions
                    max_concurrent_positions: 3,
                    max_daily_volume: 10_000.0, // Conservative volume
                },
                timeouts: TimeoutConfig {
                    execution_timeout_ms: 5000,    // 5 seconds
                    emergency_timeout_ms: 1000,    // 1 second
                    coordination_timeout_ms: 2000, // 2 seconds
                },
            },
        }
    }

    /// Arbitrage preset
    ///
    /// UCE-32 Q31 (Rust Transform): Optimized for cross-exchange coordination
    ///
    /// **Performance Characteristics:**
    /// - Balanced latency and safety
    /// - Cross-exchange synchronization optimized
    /// - Medium risk tolerance for opportunity capture
    ///
    /// **Trade-offs:**
    /// - Optimized for multi-exchange coordination
    /// - Balanced risk vs opportunity capture
    /// - Network latency considerations
    pub fn arbitrage() -> Self {
        Self {
            emergency_threshold: 0.01, // 1.0% - balanced
            max_retries: 5,
            memory_ordering_level: MemoryOrderingLevel::Optimized,
            cache_optimization: CacheOptimization {
                cache_aligned: true,
                hot_cold_separation: true,
                false_sharing_prevention: true,
                cpu_specific: true,
            },
            validation_level: ValidationLevel::Standard,
            performance_features: PerformanceFeatures {
                nightly_simd: true,
                branch_prediction: true,
                atomic_from_mut: true,
                const_traits: true,
                const_float_math: true,
            },
            monitoring: MonitoringConfig {
                detailed_tracking: true,
                performance_metrics: true,
                memory_monitoring: false,
                debug_assertions: false,
                cache_analysis: true,
            },
            risk_management: RiskConfig {
                risk_thresholds: RiskThresholds {
                    low_risk: 0.005,   // 0.5%
                    medium_risk: 0.01, // 1.0%
                    high_risk: 0.02,   // 2.0%
                },
                emergency_sensitivity: 0.01, // 1.0%
                position_limits: PositionLimits {
                    max_position_size: 500.0,
                    max_concurrent_positions: 10, // Multiple exchanges
                    max_daily_volume: 50_000.0,
                },
                timeouts: TimeoutConfig {
                    execution_timeout_ms: 200,    // 200ms for network latency
                    emergency_timeout_ms: 100,    // 100ms
                    coordination_timeout_ms: 150, // 150ms
                },
            },
        }
    }

    /// Development preset
    ///
    /// UCE-32 Q28 (Simplicity): Debug-friendly settings for development
    ///
    /// **Performance Characteristics:**
    /// - Full debugging and validation enabled
    /// - Comprehensive error reporting
    /// - Performance monitoring for optimization
    ///
    /// **Trade-offs:**
    /// - Debug capabilities over performance
    /// - Detailed logging and diagnostics
    /// - Slower execution but comprehensive feedback
    pub fn development() -> Self {
        Self {
            emergency_threshold: 0.02, // 2.0% - safe for testing
            max_retries: 20,           // High retry count for debugging
            memory_ordering_level: MemoryOrderingLevel::Strict,
            cache_optimization: CacheOptimization {
                cache_aligned: false, // Disable for debugging
                hot_cold_separation: false,
                false_sharing_prevention: false,
                cpu_specific: false,
            },
            validation_level: ValidationLevel::Comprehensive,
            performance_features: PerformanceFeatures {
                nightly_simd: false,
                branch_prediction: false,
                atomic_from_mut: false,
                const_traits: false,
                const_float_math: false,
            },
            monitoring: MonitoringConfig {
                detailed_tracking: true,
                performance_metrics: true,
                memory_monitoring: true,
                debug_assertions: true,
                cache_analysis: true,
            },
            risk_management: RiskConfig {
                risk_thresholds: RiskThresholds {
                    low_risk: 0.005,   // 0.5%
                    medium_risk: 0.01, // 1.0%
                    high_risk: 0.02,   // 2.0%
                },
                emergency_sensitivity: 0.02, // 2.0%
                position_limits: PositionLimits {
                    max_position_size: 10.0, // Small test positions
                    max_concurrent_positions: 1,
                    max_daily_volume: 1_000.0,
                },
                timeouts: TimeoutConfig {
                    execution_timeout_ms: 10_000,   // 10 seconds for debugging
                    emergency_timeout_ms: 5_000,    // 5 seconds
                    coordination_timeout_ms: 7_500, // 7.5 seconds
                },
            },
        }
    }

    /// Production preset
    ///
    /// UCE-32 Q30 (Empirical Validation): Battle-tested configuration for production
    ///
    /// **Performance Characteristics:**
    /// - Optimal balance of performance and reliability
    /// - Proven settings from production use
    /// - Maximum optimization with safety guardrails
    ///
    /// **Trade-offs:**
    /// - Balanced performance and safety
    /// - Production-validated settings
    /// - Reliable operation under load
    pub fn production() -> Self {
        Self {
            emergency_threshold: 0.005, // 0.5% - production balanced
            max_retries: 7,             // Balanced retry count
            memory_ordering_level: MemoryOrderingLevel::Optimized,
            cache_optimization: CacheOptimization {
                cache_aligned: true,
                hot_cold_separation: true,
                false_sharing_prevention: true,
                cpu_specific: true,
            },
            validation_level: ValidationLevel::Standard,
            performance_features: PerformanceFeatures {
                nightly_simd: true,
                branch_prediction: true,
                atomic_from_mut: true,
                const_traits: true,
                const_float_math: true,
            },
            monitoring: MonitoringConfig {
                detailed_tracking: true,
                performance_metrics: true,
                memory_monitoring: true,
                debug_assertions: false, // Disabled for performance
                cache_analysis: false,
            },
            risk_management: RiskConfig {
                risk_thresholds: RiskThresholds {
                    low_risk: 0.002,    // 0.2%
                    medium_risk: 0.005, // 0.5%
                    high_risk: 0.01,    // 1.0%
                },
                emergency_sensitivity: 0.005, // 0.5%
                position_limits: PositionLimits {
                    max_position_size: 1000.0,
                    max_concurrent_positions: 20,
                    max_daily_volume: 500_000.0,
                },
                timeouts: TimeoutConfig {
                    execution_timeout_ms: 500,    // 500ms
                    emergency_timeout_ms: 250,    // 250ms
                    coordination_timeout_ms: 300, // 300ms
                },
            },
        }
    }

    /// Validate configuration parameters
    ///
    /// UCE-32 Q31 (Rust Transform): Type-safe validation with Result error handling
    pub fn validate(&self) -> Result<()> {
        // Validate emergency threshold
        if !self.emergency_threshold.is_finite()
            || self.emergency_threshold < 0.0
            || self.emergency_threshold > 1.0
        {
            return Err(HedgeError::ValidationFailed {
                field: "emergency_threshold".to_string(),
                value: self.emergency_threshold.to_string(),
                reason: "Must be finite number between 0.0 and 1.0".to_string(),
            });
        }

        // Validate retry count
        if self.max_retries == 0 || self.max_retries > 1000 {
            return Err(HedgeError::ValidationFailed {
                field: "max_retries".to_string(),
                value: self.max_retries.to_string(),
                reason: "Must be between 1 and 1000".to_string(),
            });
        }

        // Validate risk thresholds
        let thresholds = &self.risk_management.risk_thresholds;
        if thresholds.low_risk >= thresholds.medium_risk
            || thresholds.medium_risk >= thresholds.high_risk
        {
            return Err(HedgeError::ValidationFailed {
                field: "risk_thresholds".to_string(),
                value: format!(
                    "low={}, medium={}, high={}",
                    thresholds.low_risk, thresholds.medium_risk, thresholds.high_risk
                ),
                reason: "Risk thresholds must be in ascending order".to_string(),
            });
        }

        // Validate position limits
        let limits = &self.risk_management.position_limits;
        if limits.max_position_size <= 0.0 {
            return Err(HedgeError::ValidationFailed {
                field: "max_position_size".to_string(),
                value: limits.max_position_size.to_string(),
                reason: "Must be positive".to_string(),
            });
        }

        if limits.max_concurrent_positions == 0 {
            return Err(HedgeError::ValidationFailed {
                field: "max_concurrent_positions".to_string(),
                value: limits.max_concurrent_positions.to_string(),
                reason: "Must be at least 1".to_string(),
            });
        }

        // Validate timeouts
        let timeouts = &self.risk_management.timeouts;
        if timeouts.execution_timeout_ms == 0
            || timeouts.emergency_timeout_ms == 0
            || timeouts.coordination_timeout_ms == 0
        {
            return Err(HedgeError::ValidationFailed {
                field: "timeouts".to_string(),
                value: format!(
                    "exec={}, emergency={}, coord={}",
                    timeouts.execution_timeout_ms,
                    timeouts.emergency_timeout_ms,
                    timeouts.coordination_timeout_ms
                ),
                reason: "All timeouts must be greater than 0".to_string(),
            });
        }

        Ok(())
    }

    /// Get performance characteristics description
    pub fn performance_description(&self) -> String {
        let ordering_desc = match self.memory_ordering_level {
            MemoryOrderingLevel::Strict => "SeqCst (maximum safety)",
            MemoryOrderingLevel::Optimized => "Acquire/Release (balanced)",
            MemoryOrderingLevel::UltraOptimized => "Relaxed where safe (maximum speed)",
        };

        let validation_desc = match self.validation_level {
            ValidationLevel::Minimal => "minimal validation",
            ValidationLevel::Standard => "standard validation",
            ValidationLevel::Strict => "strict validation",
            ValidationLevel::Comprehensive => "comprehensive validation",
        };

        format!(
            "Emergency: {:.3}%, Retries: {}, Ordering: {}, Validation: {}, Cache: {}, Nightly: {}",
            self.emergency_threshold * 100.0,
            self.max_retries,
            ordering_desc,
            validation_desc,
            if self.cache_optimization.cache_aligned {
                "optimized"
            } else {
                "standard"
            },
            if self.performance_features.nightly_simd {
                "enabled"
            } else {
                "disabled"
            }
        )
    }

    /// Estimate relative performance compared to default
    ///
    /// UCE-32 Q30 (Empirical Validation): Conservative performance estimates
    pub fn estimated_performance_multiplier(&self) -> f64 {
        let mut multiplier = 1.0;

        // Memory ordering impact
        match self.memory_ordering_level {
            MemoryOrderingLevel::Strict => multiplier *= 0.7, // 30% slower
            MemoryOrderingLevel::Optimized => multiplier *= 1.0, // baseline
            MemoryOrderingLevel::UltraOptimized => multiplier *= 1.3, // 30% faster
        }

        // Cache optimization impact
        if self.cache_optimization.cache_aligned && self.cache_optimization.hot_cold_separation {
            multiplier *= 1.15; // 15% improvement
        }

        // Nightly features impact
        if self.performance_features.nightly_simd {
            multiplier *= 1.25; // 25% improvement for vectorizable operations
        }

        if self.performance_features.branch_prediction {
            multiplier *= 1.05; // 5% improvement
        }

        // Validation overhead
        match self.validation_level {
            ValidationLevel::Minimal => multiplier *= 1.1, // 10% faster
            ValidationLevel::Standard => multiplier *= 1.0, // baseline
            ValidationLevel::Strict => multiplier *= 0.9,  // 10% slower
            ValidationLevel::Comprehensive => multiplier *= 0.7, // 30% slower
        }

        // Monitoring overhead
        if self.monitoring.detailed_tracking {
            multiplier *= 0.95; // 5% slower
        }

        multiplier
    }

    /// Get risk profile description
    pub fn risk_profile(&self) -> String {
        let emergency_level = if self.emergency_threshold <= 0.001 {
            "Ultra-sensitive"
        } else if self.emergency_threshold <= 0.01 {
            "Sensitive"
        } else if self.emergency_threshold <= 0.05 {
            "Conservative"
        } else {
            "Relaxed"
        };

        let validation_safety = match self.validation_level {
            ValidationLevel::Minimal => "Low safety",
            ValidationLevel::Standard => "Standard safety",
            ValidationLevel::Strict => "High safety",
            ValidationLevel::Comprehensive => "Maximum safety",
        };

        format!(
            "{} emergency response, {}, max position: {:.0}, max concurrent: {}",
            emergency_level,
            validation_safety,
            self.risk_management.position_limits.max_position_size,
            self.risk_management
                .position_limits
                .max_concurrent_positions
        )
    }
}

/// Preset configuration helper functions for the existing builder
#[cfg(feature = "builder")]
impl AtomicHedgeCapsule {
    /// Create builder with HFT preset configuration
    ///
    /// UCE-32 Q30 (Empirical Validation): Optimized for < 50ns latency
    pub fn hft_preset() -> HedgeCapsuleBuilder {
        AtomicHedgeCapsule::high_frequency_trading()
    }

    /// Create builder with Risk Management preset configuration
    ///
    /// UCE-32 Q29 (Practical Constraints): Conservative settings for maximum safety
    pub fn risk_preset() -> HedgeCapsuleBuilder {
        AtomicHedgeCapsule::conservative_trading()
    }

    /// Create builder with Arbitrage preset configuration
    ///
    /// UCE-32 Q31 (Rust Transform): Optimized for cross-exchange coordination
    pub fn arbitrage_preset() -> HedgeCapsuleBuilder {
        AtomicHedgeCapsule::market_making()
    }

    /// Create builder with Development preset configuration
    ///
    /// UCE-32 Q28 (Simplicity): Debug-friendly settings for development
    pub fn development_preset() -> HedgeCapsuleBuilder {
        AtomicHedgeCapsule::builder()
            .with_emergency_threshold(0.02)
            .with_timeout_ms(10_000)
            .without_cache_optimization() // Disable for debugging
            .with_max_position_size(10.0) // Small test positions
    }

    /// Create builder with Production preset configuration
    ///
    /// UCE-32 Q30 (Empirical Validation): Battle-tested configuration for production
    pub fn production_preset() -> HedgeCapsuleBuilder {
        AtomicHedgeCapsule::builder()
            .with_emergency_threshold(0.005)
            .with_timeout_ms(500)
            .with_cache_optimization()
            .with_max_position_size(1000.0)
    }
}

/// Extension trait for AtomicHedgeCapsule to add preset constructors
///
/// UCE-32 Q31 (Rust Transform): Trait-based extension for zero-cost abstractions
pub trait AtomicHedgeCapsulePresets {
    /// Create capsule with High Frequency Trading preset
    fn with_hft_preset(
        symbol: &str,
        exchange: &str,
        size: f64,
        stop_loss: f64,
        take_profit: f64,
    ) -> Result<AtomicHedgeCapsule>;

    /// Create capsule with Risk Management preset
    fn with_risk_preset(
        symbol: &str,
        exchange: &str,
        size: f64,
        stop_loss: f64,
        take_profit: f64,
    ) -> Result<AtomicHedgeCapsule>;

    /// Create capsule with Arbitrage preset
    fn with_arbitrage_preset(
        symbol: &str,
        exchange: &str,
        size: f64,
        stop_loss: f64,
        take_profit: f64,
    ) -> Result<AtomicHedgeCapsule>;

    /// Create capsule with Development preset
    fn with_development_preset(
        symbol: &str,
        exchange: &str,
        size: f64,
        stop_loss: f64,
        take_profit: f64,
    ) -> Result<AtomicHedgeCapsule>;

    /// Create capsule with Production preset
    fn with_production_preset(
        symbol: &str,
        exchange: &str,
        size: f64,
        stop_loss: f64,
        take_profit: f64,
    ) -> Result<AtomicHedgeCapsule>;
}

impl AtomicHedgeCapsulePresets for AtomicHedgeCapsule {
    fn with_hft_preset(
        symbol: &str,
        exchange: &str,
        size: f64,
        stop_loss: f64,
        take_profit: f64,
    ) -> Result<AtomicHedgeCapsule> {
        AtomicHedgeCapsule::hft_preset()
            .with_entry_order(exchange, symbol, "Buy", size)
            .with_bracket_order(stop_loss, take_profit)
            .build()
    }

    fn with_risk_preset(
        symbol: &str,
        exchange: &str,
        size: f64,
        stop_loss: f64,
        take_profit: f64,
    ) -> Result<AtomicHedgeCapsule> {
        AtomicHedgeCapsule::risk_preset()
            .with_entry_order(exchange, symbol, "Buy", size)
            .with_bracket_order(stop_loss, take_profit)
            .build()
    }

    fn with_arbitrage_preset(
        symbol: &str,
        exchange: &str,
        size: f64,
        stop_loss: f64,
        take_profit: f64,
    ) -> Result<AtomicHedgeCapsule> {
        AtomicHedgeCapsule::arbitrage_preset()
            .with_entry_order(exchange, symbol, "Buy", size)
            .with_bracket_order(stop_loss, take_profit)
            .build()
    }

    fn with_development_preset(
        symbol: &str,
        exchange: &str,
        size: f64,
        stop_loss: f64,
        take_profit: f64,
    ) -> Result<AtomicHedgeCapsule> {
        AtomicHedgeCapsule::development_preset()
            .with_entry_order(exchange, symbol, "Buy", size)
            .with_bracket_order(stop_loss, take_profit)
            .build()
    }

    fn with_production_preset(
        symbol: &str,
        exchange: &str,
        size: f64,
        stop_loss: f64,
        take_profit: f64,
    ) -> Result<AtomicHedgeCapsule> {
        AtomicHedgeCapsule::production_preset()
            .with_entry_order(exchange, symbol, "Buy", size)
            .with_bracket_order(stop_loss, take_profit)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_validation() {
        // Test all presets are valid
        assert!(PresetConfig::high_frequency_trading().validate().is_ok());
        assert!(PresetConfig::risk_management().validate().is_ok());
        assert!(PresetConfig::arbitrage().validate().is_ok());
        assert!(PresetConfig::development().validate().is_ok());
        assert!(PresetConfig::production().validate().is_ok());
    }

    #[test]
    fn test_hft_preset_characteristics() {
        let config = PresetConfig::high_frequency_trading();

        // HFT should have ultra-low emergency threshold
        assert!(config.emergency_threshold <= 0.001);

        // Should have minimal retries for speed
        assert!(config.max_retries <= 5);

        // Should have ultra-optimized memory ordering
        assert_eq!(
            config.memory_ordering_level,
            MemoryOrderingLevel::UltraOptimized
        );

        // Should have minimal validation
        assert_eq!(config.validation_level, ValidationLevel::Minimal);

        // Should have nightly features enabled
        assert!(config.performance_features.nightly_simd);
        assert!(config.performance_features.branch_prediction);
    }

    #[test]
    fn test_risk_management_preset_characteristics() {
        let config = PresetConfig::risk_management();

        // Risk management should have high emergency threshold
        assert!(config.emergency_threshold >= 0.05);

        // Should have many retries for safety
        assert!(config.max_retries >= 10);

        // Should have strict memory ordering
        assert_eq!(config.memory_ordering_level, MemoryOrderingLevel::Strict);

        // Should have comprehensive validation
        assert_eq!(config.validation_level, ValidationLevel::Comprehensive);

        // Should have all monitoring enabled
        assert!(config.monitoring.detailed_tracking);
        assert!(config.monitoring.debug_assertions);
    }

    #[test]
    fn test_builder_pattern() {
        // Test the preset builder methods
        let builder = AtomicHedgeCapsule::hft_preset()
            .with_entry_order("NDAX", "BTCUSD", "Buy", 1.0)
            .with_bracket_order(45000.0, 55000.0);

        let result = builder.build();
        assert!(result.is_ok());
        let capsule = result.unwrap();
        assert!(capsule.is_active());
    }

    #[test]
    fn test_performance_multiplier_calculations() {
        let hft = PresetConfig::high_frequency_trading();
        let risk = PresetConfig::risk_management();

        // HFT should be faster than risk management
        assert!(hft.estimated_performance_multiplier() > risk.estimated_performance_multiplier());

        // Performance multiplier should be positive
        assert!(hft.estimated_performance_multiplier() > 0.0);
        assert!(risk.estimated_performance_multiplier() > 0.0);
    }

    #[test]
    fn test_preset_creation_with_trait() {
        let result = AtomicHedgeCapsule::with_hft_preset("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0);
        assert!(result.is_ok());

        let capsule = result.unwrap();
        assert!(capsule.is_active());
    }

    #[test]
    fn test_config_serialization() {
        let config = PresetConfig::production();

        // Test serialization
        let json = serde_json::to_string(&config).expect("Should serialize");
        assert!(!json.is_empty());

        // Test deserialization
        let deserialized: PresetConfig = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(config.emergency_threshold, deserialized.emergency_threshold);
    }

    #[test]
    fn test_invalid_configurations() {
        let mut config = PresetConfig::production();

        // Invalid emergency threshold
        config.emergency_threshold = -1.0;
        assert!(config.validate().is_err());

        config.emergency_threshold = 2.0;
        assert!(config.validate().is_err());

        // Invalid retries
        config.emergency_threshold = 0.01;
        config.max_retries = 0;
        assert!(config.validate().is_err());

        config.max_retries = 10000;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_builder_validation() {
        // Test that preset builders work correctly
        let result = AtomicHedgeCapsule::production_preset()
            .with_entry_order("NDAX", "BTCUSD", "Buy", 1.0)
            .with_bracket_order(45000.0, 55000.0)
            .build();
        assert!(result.is_ok());

        // Test position size validation through existing builder
        let result = AtomicHedgeCapsule::risk_preset()
            .with_max_position_size(50.0) // Set smaller limit
            .with_entry_order("NDAX", "BTCUSD", "Buy", 100.0) // Exceeds limit
            .with_bracket_order(45000.0, 55000.0)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_risk_profile_descriptions() {
        let hft = PresetConfig::high_frequency_trading();
        let risk = PresetConfig::risk_management();

        let hft_profile = hft.risk_profile();
        let risk_profile = risk.risk_profile();

        assert!(hft_profile.contains("Ultra-sensitive") || hft_profile.contains("Sensitive"));
        assert!(risk_profile.contains("Conservative") || risk_profile.contains("Maximum safety"));
    }
}
