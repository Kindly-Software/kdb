//! Statistical analysis for benchmark results
//!
//! Provides mean, median, percentiles (P95, P99, P999), standard deviation,
//! and 95% confidence intervals for B32 compliance.

use std::cmp::Ordering;

/// Statistical measurements from benchmark runs
#[derive(Debug, Clone)]
pub struct Statistics {
    /// Number of samples
    pub samples: usize,
    /// Mean (average) in nanoseconds
    pub mean_ns: f64,
    /// Median (P50) in nanoseconds
    pub median_ns: f64,
    /// 95th percentile in nanoseconds
    pub p95_ns: f64,
    /// 99th percentile in nanoseconds
    pub p99_ns: f64,
    /// 99.9th percentile in nanoseconds
    pub p999_ns: f64,
    /// Standard deviation in nanoseconds
    pub stddev_ns: f64,
    /// Minimum value in nanoseconds
    pub min_ns: f64,
    /// Maximum value in nanoseconds
    pub max_ns: f64,
    /// Number of outliers detected
    pub outliers: usize,
    /// 95% confidence interval
    pub confidence_interval_95: ConfidenceInterval,
}

/// 95% confidence interval for mean
#[derive(Debug, Clone, Copy)]
pub struct ConfidenceInterval {
    /// Lower bound of 95% CI
    pub lower_bound_ns: f64,
    /// Upper bound of 95% CI
    pub upper_bound_ns: f64,
}

impl Statistics {
    /// Calculate statistics from raw samples
    ///
    /// # Arguments
    /// * `samples` - Raw timing samples in nanoseconds (will be sorted)
    ///
    /// # Returns
    /// Statistical measurements including mean, median, percentiles, stddev, and 95% CI
    pub fn from_samples(mut samples: Vec<f64>) -> Self {
        assert!(!samples.is_empty(), "Cannot calculate statistics from empty samples");

        // Sort samples for percentile calculation
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));

        let n = samples.len();

        // Calculate mean
        let mean = samples.iter().sum::<f64>() / n as f64;

        // Calculate median (P50)
        let median = percentile(&samples, 0.50);

        // Calculate percentiles
        let p95 = percentile(&samples, 0.95);
        let p99 = percentile(&samples, 0.99);
        let p999 = percentile(&samples, 0.999);

        // Calculate standard deviation
        let variance = samples.iter()
            .map(|x| {
                let diff = x - mean;
                diff * diff
            })
            .sum::<f64>() / n as f64;
        let stddev = variance.sqrt();

        // Calculate min/max
        let min = samples[0];
        let max = samples[n - 1];

        // Detect outliers (values beyond 3 standard deviations)
        let outlier_threshold = 3.0;
        let lower_outlier = mean - outlier_threshold * stddev;
        let upper_outlier = mean + outlier_threshold * stddev;
        let outliers = samples.iter()
            .filter(|&&x| x < lower_outlier || x > upper_outlier)
            .count();

        // Calculate 95% confidence interval for mean
        // Using Student's t-distribution approximation (z=1.96 for large n)
        let standard_error = stddev / (n as f64).sqrt();
        let margin_of_error = 1.96 * standard_error; // z-score for 95% CI
        let ci = ConfidenceInterval {
            lower_bound_ns: mean - margin_of_error,
            upper_bound_ns: mean + margin_of_error,
        };

        Self {
            samples: n,
            mean_ns: mean,
            median_ns: median,
            p95_ns: p95,
            p99_ns: p99,
            p999_ns: p999,
            stddev_ns: stddev,
            min_ns: min,
            max_ns: max,
            outliers,
            confidence_interval_95: ci,
        }
    }

    /// Calculate speedup compared to baseline
    pub fn speedup(&self, baseline: &Statistics) -> Speedup {
        Speedup {
            mean_speedup: baseline.mean_ns / self.mean_ns,
            median_speedup: baseline.median_ns / self.median_ns,
            p95_speedup: baseline.p95_ns / self.p95_ns,
            confidence_interval_95: SpeedupConfidenceInterval {
                // Conservative estimate: baseline lower bound / optimized upper bound
                lower_bound: baseline.confidence_interval_95.lower_bound_ns / self.confidence_interval_95.upper_bound_ns,
                // Conservative estimate: baseline upper bound / optimized lower bound
                upper_bound: baseline.confidence_interval_95.upper_bound_ns / self.confidence_interval_95.lower_bound_ns,
            },
        }
    }
}

/// Speedup measurements (optimized vs baseline)
#[derive(Debug, Clone, Copy)]
pub struct Speedup {
    /// Mean speedup
    pub mean_speedup: f64,
    /// Median speedup
    pub median_speedup: f64,
    /// P95 speedup
    pub p95_speedup: f64,
    /// 95% confidence interval for speedup
    pub confidence_interval_95: SpeedupConfidenceInterval,
}

/// 95% confidence interval for speedup
#[derive(Debug, Clone, Copy)]
pub struct SpeedupConfidenceInterval {
    /// Lower bound of speedup CI
    pub lower_bound: f64,
    /// Upper bound of speedup CI
    pub upper_bound: f64,
}

/// Calculate percentile from sorted samples
///
/// # Arguments
/// * `sorted_samples` - Samples sorted in ascending order
/// * `percentile` - Percentile to calculate (0.0 to 1.0)
///
/// # Returns
/// Value at the specified percentile
fn percentile(sorted_samples: &[f64], p: f64) -> f64 {
    assert!(!sorted_samples.is_empty());
    assert!(p >= 0.0 && p <= 1.0);

    let n = sorted_samples.len();

    // Calculate index (linear interpolation)
    let rank = p * (n - 1) as f64;
    let lower_index = rank.floor() as usize;
    let upper_index = rank.ceil() as usize;
    let fraction = rank - lower_index as f64;

    // Interpolate between lower and upper values
    let lower_value = sorted_samples[lower_index];
    let upper_value = sorted_samples[upper_index];

    lower_value + fraction * (upper_value - lower_value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statistics_basic() {
        let samples = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let stats = Statistics::from_samples(samples);

        assert_eq!(stats.samples, 5);
        assert_eq!(stats.mean_ns, 30.0);
        assert_eq!(stats.median_ns, 30.0);
        assert_eq!(stats.min_ns, 10.0);
        assert_eq!(stats.max_ns, 50.0);
    }

    #[test]
    fn test_percentile_calculation() {
        let samples = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let p50 = percentile(&samples, 0.50);
        let p95 = percentile(&samples, 0.95);
        let p99 = percentile(&samples, 0.99);

        assert!((p50 - 5.5).abs() < 0.01, "P50: {}", p50);
        assert!((p95 - 9.55).abs() < 0.01, "P95: {}", p95);
        assert!((p99 - 9.91).abs() < 0.01, "P99: {}", p99);
    }

    #[test]
    fn test_confidence_interval() {
        // Sample with known mean and stddev
        let samples = vec![100.0; 1000]; // All same value
        let stats = Statistics::from_samples(samples);

        // With zero variance, CI should be exactly at mean
        assert_eq!(stats.stddev_ns, 0.0);
        assert_eq!(stats.confidence_interval_95.lower_bound_ns, 100.0);
        assert_eq!(stats.confidence_interval_95.upper_bound_ns, 100.0);
    }

    #[test]
    fn test_speedup_calculation() {
        let optimized_samples = vec![10.0, 20.0, 30.0];
        let baseline_samples = vec![40.0, 80.0, 120.0];

        let optimized_stats = Statistics::from_samples(optimized_samples);
        let baseline_stats = Statistics::from_samples(baseline_samples);

        let speedup = optimized_stats.speedup(&baseline_stats);

        // Baseline mean (80) / Optimized mean (20) = 4x
        assert_eq!(speedup.mean_speedup, 4.0);
        assert_eq!(speedup.median_speedup, 4.0);
    }

    #[test]
    fn test_outlier_detection() {
        let mut samples: Vec<f64> = (0..100).map(|x| x as f64).collect();
        // Add outliers
        samples.push(1000.0);
        samples.push(-1000.0);

        let stats = Statistics::from_samples(samples);

        // Should detect 2 outliers
        assert!(stats.outliers >= 2, "Outliers detected: {}", stats.outliers);
    }
}
