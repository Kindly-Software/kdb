//! Test Support Primitives
//!
//! Shared utilities for testing and benchmarking across all atomic primitives.
//! Implements B32 benchmarking framework with statistical rigor and lockfree verification.
//!
//! # Core Features
//! - B32-compliant benchmarking with 95% confidence intervals
//! - Statistical validation utilities with proper error handling
//! - Lockfree operation verification helpers
//! - Common test data generators with deterministic seeding
//! - Performance measurement with hardware awareness
//!
//! # Example Usage
//! ```rust
//! use test_support::{BenchmarkValidator, StatisticalValidator, LockfreeVerifier};
//!
//! let validator = BenchmarkValidator::new()
//!     .with_confidence_level(0.95)
//!     .with_min_iterations(1000);
//!
//! let result = validator.measure_operation(|| {
//!     // Your operation here
//! });
//!
//! assert!(result.meets_b32_standards());
//! ```

use std::time::Duration;

pub mod benchmark;
pub mod statistical;
pub mod lockfree;
pub mod generators;
pub mod validation;
pub mod breaker;

pub use benchmark::{BenchmarkValidator, BenchmarkResult, B32Configuration};
pub use statistical::{StatisticalValidator, ConfidenceInterval, PerformanceMetrics};
pub use lockfree::{LockfreeVerifier, AtomicVerificationResult, MemoryOrderingTest};
pub use generators::{TestDataGenerator, DeterministicRng, MarketDataGenerator};
pub use validation::{ValidationResult, TestAssertion, PropertyTest};
pub use breaker::{BreakerProtected, BreakerMetrics, ProtectionResult, BreakerState, ProtectionError};

/// Core test support error types
#[derive(Debug, thiserror::Error)]
pub enum TestSupportError {
    #[error("Benchmark validation failed: {reason}")]
    BenchmarkValidation { reason: String },

    #[error("Statistical analysis failed: {reason}")]
    StatisticalAnalysis { reason: String },

    #[error("Lockfree verification failed: {reason}")]
    LockfreeVerification { reason: String },

    #[error("Test data generation failed: {reason}")]
    DataGeneration { reason: String },

    #[error("Hardware constraint violation: {constraint}")]
    HardwareConstraint { constraint: String },
}

/// Result type for test support operations
pub type TestResult<T> = Result<T, TestSupportError>;

/// B32 Hardware Reality Constants (Intel Ultra 7 155H)
pub mod hardware_constants {
    use std::time::Duration;

    /// Atomic operation costs (measured, not theoretical)
    pub const ATOMIC_U64_CAS_NS: u64 = 15;
    pub const ATOMIC_U64_FETCH_ADD_NS: u64 = 20;
    pub const ATOMIC_U128_CAS_NS: u64 = 20;

    /// Memory hierarchy latencies
    pub const L1_CACHE_NS: u64 = 1;
    pub const L2_CACHE_NS: u64 = 3;
    pub const L3_CACHE_NS: u64 = 12;
    pub const RAM_LATENCY_NS: u64 = 100;

    /// Cache line size
    pub const CACHE_LINE_SIZE: usize = 64;

    /// Thread scaling reality
    pub const EFFICIENT_THREAD_COUNT: usize = 12;
    pub const MAX_USEFUL_THREADS: usize = 14;

    /// Performance expectations
    pub const TYPICAL_IMPROVEMENT_MIN: f64 = 1.1;  // 10%
    pub const TYPICAL_IMPROVEMENT_MAX: f64 = 1.5;  // 50%
    pub const EXCEPTIONAL_IMPROVEMENT_MAX: f64 = 2.0; // 100%
    pub const SUSPICIOUS_IMPROVEMENT_THRESHOLD: f64 = 10.0; // 1000%

    /// Benchmark requirements
    pub const MIN_BENCHMARK_ITERATIONS: usize = 1000;
    pub const REQUIRED_CONFIDENCE_LEVEL: f64 = 0.95;
    pub const MIN_SUSTAINED_DURATION: Duration = Duration::from_secs(60);
}

/// Quick validation helper for B32 compliance
pub fn validate_b32_claim(
    baseline_ns: f64,
    optimized_ns: f64,
    confidence_interval: (f64, f64),
    iterations: usize,
) -> TestResult<()> {
    use hardware_constants::*;

    if iterations < MIN_BENCHMARK_ITERATIONS {
        return Err(TestSupportError::BenchmarkValidation {
            reason: format!(
                "Insufficient iterations: {} < {}",
                iterations, MIN_BENCHMARK_ITERATIONS
            ),
        });
    }

    let improvement = baseline_ns / optimized_ns;

    if improvement > SUSPICIOUS_IMPROVEMENT_THRESHOLD {
        return Err(TestSupportError::BenchmarkValidation {
            reason: format!(
                "Suspicious improvement: {:.2}x exceeds realistic threshold of {:.2}x",
                improvement, SUSPICIOUS_IMPROVEMENT_THRESHOLD
            ),
        });
    }

    let ci_width = confidence_interval.1 - confidence_interval.0;
    let relative_error = ci_width / optimized_ns;

    if relative_error > 0.15 {
        return Err(TestSupportError::StatisticalAnalysis {
            reason: format!(
                "High measurement uncertainty: {:.1}% > 15%",
                relative_error * 100.0
            ),
        });
    }

    Ok(())
}

/// Shared test configuration for consistent testing
#[derive(Debug, Clone)]
pub struct TestConfiguration {
    pub confidence_level: f64,
    pub min_iterations: usize,
    pub warmup_iterations: usize,
    pub sustained_duration: Duration,
    pub max_threads: usize,
    pub random_seed: u64,
}

impl Default for TestConfiguration {
    fn default() -> Self {
        use hardware_constants::*;

        Self {
            confidence_level: REQUIRED_CONFIDENCE_LEVEL,
            min_iterations: MIN_BENCHMARK_ITERATIONS,
            warmup_iterations: 100,
            sustained_duration: MIN_SUSTAINED_DURATION,
            max_threads: EFFICIENT_THREAD_COUNT,
            random_seed: 12345, // Deterministic by default
        }
    }
}

impl TestConfiguration {
    /// Create B32-compliant configuration
    pub fn b32_compliant() -> Self {
        Self::default()
    }

    /// Create configuration for fast development testing
    pub fn fast_dev() -> Self {
        Self {
            min_iterations: 100,
            sustained_duration: Duration::from_secs(1),
            ..Self::default()
        }
    }

    /// Create configuration for stress testing
    pub fn stress_test() -> Self {
        Self {
            min_iterations: 10000,
            sustained_duration: Duration::from_secs(300),
            max_threads: hardware_constants::MAX_USEFUL_THREADS,
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_b32_validation_passes_realistic_claim() {
        let result = validate_b32_claim(
            100.0, // baseline: 100ns
            80.0,  // optimized: 80ns (20% improvement)
            (78.0, 82.0), // tight confidence interval
            1000,  // sufficient iterations
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_b32_validation_rejects_suspicious_claim() {
        let result = validate_b32_claim(
            100.0, // baseline: 100ns
            1.0,   // optimized: 1ns (100x improvement - suspicious)
            (0.9, 1.1),
            1000,
        );

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TestSupportError::BenchmarkValidation { .. }
        ));
    }

    #[test]
    fn test_b32_validation_rejects_insufficient_iterations() {
        let result = validate_b32_claim(
            100.0,
            80.0,
            (78.0, 82.0),
            500, // Too few iterations
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_test_configuration_defaults() {
        let config = TestConfiguration::default();
        assert_eq!(config.confidence_level, 0.95);
        assert!(config.min_iterations >= 1000);
        assert!(config.sustained_duration >= Duration::from_secs(60));
    }
}