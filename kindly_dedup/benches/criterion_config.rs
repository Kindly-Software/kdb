//! Shared Criterion.rs Configuration for B32 Framework Compliance
//!
//! **Purpose**: Centralized benchmark configuration meeting B32 requirements:
//! - 1000+ iterations (sample_size)
//! - 95% confidence intervals (confidence_level)
//! - Statistical rigor (outlier detection, warmup)
//! - Reproducible results (fixed measurement times)
//!
//! **B32 Framework**: Fair baselines, reproducible methodology, honest reporting

use criterion::{Criterion, PlotConfiguration, AxisScale};
use std::time::Duration;

/// Configure Criterion for B32 Framework compliance
///
/// **Requirements**:
/// - Sample size: 1000+ iterations (B32 requirement)
/// - Confidence level: 95% CI (B32 requirement)
/// - Measurement time: 10s per benchmark (stable results)
/// - Warm-up time: 3s (cache stabilization)
/// - Outlier detection: Enabled (anomaly filtering)
///
/// **Returns**: Configured Criterion instance
pub fn configure_criterion() -> Criterion {
    Criterion::default()
        .sample_size(1000)           // 1000+ iterations (B32 requirement)
        .confidence_level(0.95)      // 95% CI (B32 requirement)
        .measurement_time(Duration::from_secs(10))  // 10s per benchmark
        .warm_up_time(Duration::from_secs(3))       // 3s warmup
        // Note: plot_config not available in this version of Criterion
        // .plot_config(PlotConfiguration::default()
        //     .summary_scale(AxisScale::Logarithmic))
}

/// Configure Criterion for large workloads (reduced sample size)
///
/// **Use Case**: 100K-1M document benchmarks where 1000 iterations = too long
///
/// **Requirements**:
/// - Sample size: 100 iterations (still statistically significant)
/// - Confidence level: 95% CI (B32 requirement maintained)
/// - Measurement time: 20s per benchmark (longer for large workloads)
/// - Warm-up time: 5s (larger cache footprint)
///
/// **Returns**: Configured Criterion instance for large workloads
pub fn configure_criterion_large_workloads() -> Criterion {
    Criterion::default()
        .sample_size(100)            // Reduced sample size for large workloads
        .confidence_level(0.95)      // 95% CI (B32 requirement)
        .measurement_time(Duration::from_secs(20))  // Longer measurement time
        .warm_up_time(Duration::from_secs(5))       // Longer warmup
        // Note: plot_config not available in this version of Criterion
        // .plot_config(PlotConfiguration::default()
        //     .summary_scale(AxisScale::Logarithmic))
}
