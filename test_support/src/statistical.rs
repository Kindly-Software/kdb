//! Statistical Validation Utilities
//!
//! Provides statistical analysis tools for performance validation and
//! confidence interval calculation following B32 framework requirements.

use std::collections::HashMap;
use crate::{TestResult, TestSupportError};

/// Statistical validator for performance measurements
#[derive(Debug, Clone)]
pub struct StatisticalValidator {
    confidence_level: f64,
    min_sample_size: usize,
    outlier_threshold: f64,
}

/// Confidence interval with statistical properties
#[derive(Debug, Clone)]
pub struct ConfidenceInterval {
    pub lower_bound: f64,
    pub upper_bound: f64,
    pub confidence_level: f64,
    pub margin_of_error: f64,
    pub standard_error: f64,
}

/// Comprehensive performance metrics
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub sample_size: usize,
    pub mean: f64,
    pub median: f64,
    pub mode: Option<f64>,
    pub std_dev: f64,
    pub variance: f64,
    pub min_value: f64,
    pub max_value: f64,
    pub range: f64,
    pub skewness: f64,
    pub kurtosis: f64,
    pub confidence_interval: ConfidenceInterval,
    pub percentiles: HashMap<String, f64>,
    pub outliers: Vec<f64>,
    pub coefficient_of_variation: f64,
}

/// Statistical test results
#[derive(Debug, Clone)]
pub struct StatisticalTestResult {
    pub test_name: String,
    pub p_value: f64,
    pub is_significant: bool,
    pub test_statistic: f64,
    pub degrees_of_freedom: Option<usize>,
    pub effect_size: Option<f64>,
}

impl Default for StatisticalValidator {
    fn default() -> Self {
        Self {
            confidence_level: 0.95,
            min_sample_size: 30, // Central limit theorem threshold
            outlier_threshold: 3.0, // 3-sigma rule
        }
    }
}

impl StatisticalValidator {
    /// Create new statistical validator
    pub fn new() -> Self {
        Self::default()
    }

    /// Set confidence level (typically 0.95 for 95%)
    pub fn with_confidence_level(mut self, level: f64) -> Self {
        assert!((0.0..1.0).contains(&level), "Confidence level must be between 0 and 1");
        self.confidence_level = level;
        self
    }

    /// Set minimum sample size for valid statistics
    pub fn with_min_sample_size(mut self, size: usize) -> Self {
        self.min_sample_size = size;
        self
    }

    /// Set outlier detection threshold (standard deviations)
    pub fn with_outlier_threshold(mut self, threshold: f64) -> Self {
        self.outlier_threshold = threshold;
        self
    }

    /// Calculate comprehensive performance metrics
    pub fn analyze_measurements(&self, measurements: &[f64]) -> TestResult<PerformanceMetrics> {
        if measurements.is_empty() {
            return Err(TestSupportError::StatisticalAnalysis {
                reason: "Empty measurement set".to_string(),
            });
        }

        if measurements.len() < self.min_sample_size {
            return Err(TestSupportError::StatisticalAnalysis {
                reason: format!(
                    "Insufficient sample size: {} < {}",
                    measurements.len(),
                    self.min_sample_size
                ),
            });
        }

        let sample_size = measurements.len();
        let mean = self.calculate_mean(measurements);
        let variance = self.calculate_variance(measurements, mean);
        let std_dev = variance.sqrt();
        let median = self.calculate_median(measurements);
        let mode = self.calculate_mode(measurements);

        let min_value = measurements.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_value = measurements.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let range = max_value - min_value;

        let skewness = self.calculate_skewness(measurements, mean, std_dev);
        let kurtosis = self.calculate_kurtosis(measurements, mean, std_dev);
        let coefficient_of_variation = std_dev / mean;

        let confidence_interval = self.calculate_confidence_interval(measurements, mean, std_dev)?;
        let percentiles = self.calculate_percentiles(measurements);
        let outliers = self.detect_outliers(measurements, mean, std_dev);

        Ok(PerformanceMetrics {
            sample_size,
            mean,
            median,
            mode,
            std_dev,
            variance,
            min_value,
            max_value,
            range,
            skewness,
            kurtosis,
            confidence_interval,
            percentiles,
            outliers,
            coefficient_of_variation,
        })
    }

    /// Compare two sets of measurements (t-test)
    pub fn compare_measurements(
        &self,
        baseline: &[f64],
        treatment: &[f64],
    ) -> TestResult<StatisticalTestResult> {
        if baseline.len() < self.min_sample_size || treatment.len() < self.min_sample_size {
            return Err(TestSupportError::StatisticalAnalysis {
                reason: "Insufficient sample size for comparison".to_string(),
            });
        }

        let baseline_mean = self.calculate_mean(baseline);
        let treatment_mean = self.calculate_mean(treatment);
        let baseline_var = self.calculate_variance(baseline, baseline_mean);
        let treatment_var = self.calculate_variance(treatment, treatment_mean);

        // Welch's t-test (unequal variances)
        let pooled_se = (baseline_var / baseline.len() as f64 + treatment_var / treatment.len() as f64).sqrt();
        let t_statistic = (treatment_mean - baseline_mean) / pooled_se;

        // Welch-Satterthwaite equation for degrees of freedom
        let numerator = (baseline_var / baseline.len() as f64 + treatment_var / treatment.len() as f64).powi(2);
        let denominator = (baseline_var / baseline.len() as f64).powi(2) / (baseline.len() - 1) as f64
            + (treatment_var / treatment.len() as f64).powi(2) / (treatment.len() - 1) as f64;
        let degrees_of_freedom = (numerator / denominator) as usize;

        // Approximate p-value (simplified)
        let p_value = self.calculate_t_test_p_value(t_statistic.abs(), degrees_of_freedom);
        let alpha = 1.0 - self.confidence_level;
        let is_significant = p_value < alpha;

        // Effect size (Cohen's d)
        let pooled_std = ((baseline_var + treatment_var) / 2.0).sqrt();
        let effect_size = (treatment_mean - baseline_mean) / pooled_std;

        Ok(StatisticalTestResult {
            test_name: "Welch's t-test".to_string(),
            p_value,
            is_significant,
            test_statistic: t_statistic,
            degrees_of_freedom: Some(degrees_of_freedom),
            effect_size: Some(effect_size),
        })
    }

    /// Detect performance regressions with statistical significance
    pub fn detect_regression(
        &self,
        historical: &[f64],
        current: &[f64],
        regression_threshold: f64,
    ) -> TestResult<bool> {
        let comparison = self.compare_measurements(historical, current)?;

        if !comparison.is_significant {
            return Ok(false); // No statistically significant difference
        }

        let historical_mean = self.calculate_mean(historical);
        let current_mean = self.calculate_mean(current);
        let performance_ratio = current_mean / historical_mean;

        // Regression if current performance is significantly worse
        Ok(performance_ratio > (1.0 + regression_threshold) && comparison.is_significant)
    }

    /// Validate measurement quality
    pub fn validate_measurement_quality(&self, measurements: &[f64]) -> TestResult<Vec<String>> {
        let mut warnings = Vec::new();

        if measurements.len() < self.min_sample_size {
            warnings.push(format!(
                "Sample size {} below recommended minimum {}",
                measurements.len(),
                self.min_sample_size
            ));
        }

        let mean = self.calculate_mean(measurements);
        let std_dev = self.calculate_variance(measurements, mean).sqrt();
        let cv = std_dev / mean;

        if cv > 0.15 {
            warnings.push(format!(
                "High coefficient of variation: {:.1}% (>15%)",
                cv * 100.0
            ));
        }

        let outliers = self.detect_outliers(measurements, mean, std_dev);
        let outlier_ratio = outliers.len() as f64 / measurements.len() as f64;

        if outlier_ratio > 0.05 {
            warnings.push(format!(
                "High outlier ratio: {:.1}% (>5%)",
                outlier_ratio * 100.0
            ));
        }

        // Check for bimodal distribution (simplified)
        let skewness = self.calculate_skewness(measurements, mean, std_dev);
        if skewness.abs() > 2.0 {
            warnings.push(format!(
                "High skewness: {:.2} (possible bimodal distribution)",
                skewness
            ));
        }

        Ok(warnings)
    }

    // Helper methods for statistical calculations

    fn calculate_mean(&self, data: &[f64]) -> f64 {
        data.iter().sum::<f64>() / data.len() as f64
    }

    fn calculate_variance(&self, data: &[f64], mean: f64) -> f64 {
        if data.len() <= 1 {
            return 0.0;
        }
        data.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / (data.len() - 1) as f64
    }

    fn calculate_median(&self, data: &[f64]) -> f64 {
        let mut sorted = data.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let len = sorted.len();

        if len.is_multiple_of(2) {
            (sorted[len / 2 - 1] + sorted[len / 2]) / 2.0
        } else {
            sorted[len / 2]
        }
    }

    fn calculate_mode(&self, data: &[f64]) -> Option<f64> {
        let mut frequency = HashMap::new();

        // Round to avoid floating point precision issues
        for &value in data {
            let rounded = (value * 1000.0).round() / 1000.0;
            *frequency.entry(ordered_float::OrderedFloat(rounded)).or_insert(0) += 1;
        }

        frequency.into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(value, _)| value.into_inner())
    }

    fn calculate_skewness(&self, data: &[f64], mean: f64, std_dev: f64) -> f64 {
        if std_dev == 0.0 || data.len() < 3 {
            return 0.0;
        }

        let n = data.len() as f64;
        let skewness = data.iter()
            .map(|x| ((x - mean) / std_dev).powi(3))
            .sum::<f64>() / n;

        skewness
    }

    fn calculate_kurtosis(&self, data: &[f64], mean: f64, std_dev: f64) -> f64 {
        if std_dev == 0.0 || data.len() < 4 {
            return 0.0;
        }

        let n = data.len() as f64;
        let kurtosis = data.iter()
            .map(|x| ((x - mean) / std_dev).powi(4))
            .sum::<f64>() / n - 3.0; // Excess kurtosis

        kurtosis
    }

    fn calculate_confidence_interval(
        &self,
        data: &[f64],
        mean: f64,
        std_dev: f64,
    ) -> TestResult<ConfidenceInterval> {
        let n = data.len();
        let degrees_of_freedom = n - 1;

        // t-value lookup (simplified)
        let t_value = self.get_t_value(degrees_of_freedom, self.confidence_level)?;

        let standard_error = std_dev / (n as f64).sqrt();
        let margin_of_error = t_value * standard_error;

        Ok(ConfidenceInterval {
            lower_bound: mean - margin_of_error,
            upper_bound: mean + margin_of_error,
            confidence_level: self.confidence_level,
            margin_of_error,
            standard_error,
        })
    }

    fn calculate_percentiles(&self, data: &[f64]) -> HashMap<String, f64> {
        let mut sorted = data.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mut percentiles = HashMap::new();

        for &p in &[1.0, 5.0, 10.0, 25.0, 50.0, 75.0, 90.0, 95.0, 99.0, 99.9] {
            let index = (p / 100.0 * (sorted.len() - 1) as f64) as usize;
            let key = format!("P{}", p);
            percentiles.insert(key, sorted[index.min(sorted.len() - 1)]);
        }

        percentiles
    }

    fn detect_outliers(&self, data: &[f64], mean: f64, std_dev: f64) -> Vec<f64> {
        if std_dev == 0.0 {
            return Vec::new();
        }

        data.iter()
            .filter(|&&x| {
                let z_score = (x - mean).abs() / std_dev;
                z_score > self.outlier_threshold
            })
            .copied()
            .collect()
    }

    fn get_t_value(&self, degrees_of_freedom: usize, confidence_level: f64) -> TestResult<f64> {
        // Simplified t-table lookup
        let alpha = 1.0 - confidence_level;
        let t_value = match (degrees_of_freedom, alpha) {
            (df, a) if a <= 0.01 => match df {
                df if df >= 1000 => 2.576,
                df if df >= 100 => 2.626,
                df if df >= 30 => 2.750,
                _ => 3.169,
            },
            (df, a) if a <= 0.05 => match df {
                df if df >= 1000 => 1.960,
                df if df >= 100 => 1.984,
                df if df >= 30 => 2.042,
                _ => 2.262,
            },
            _ => 1.645, // 90% confidence
        };

        Ok(t_value)
    }

    fn calculate_t_test_p_value(&self, t_stat: f64, _df: usize) -> f64 {
        // Simplified p-value calculation (approximation)
        if t_stat < 1.0 {
            0.5
        } else if t_stat < 2.0 {
            0.1
        } else if t_stat < 3.0 {
            0.01
        } else {
            0.001
        }
    }
}

impl PerformanceMetrics {
    /// Check if measurements show acceptable variation
    pub fn has_acceptable_variation(&self, max_cv: f64) -> bool {
        self.coefficient_of_variation <= max_cv
    }

    /// Check if distribution is approximately normal
    pub fn is_approximately_normal(&self) -> bool {
        self.skewness.abs() < 2.0 && self.kurtosis.abs() < 7.0
    }

    /// Generate statistical summary report
    pub fn summary_report(&self) -> String {
        format!(
            "Statistical Summary (n={})\n\
             Mean: {:.2} ± {:.2} ({:.1}% CV)\n\
             Median: {:.2}, Range: [{:.2}, {:.2}]\n\
             95% CI: [{:.2}, {:.2}]\n\
             Outliers: {} ({:.1}%)\n\
             Distribution: skew={:.2}, kurtosis={:.2}",
            self.sample_size,
            self.mean, self.std_dev, self.coefficient_of_variation * 100.0,
            self.median, self.min_value, self.max_value,
            self.confidence_interval.lower_bound, self.confidence_interval.upper_bound,
            self.outliers.len(), (self.outliers.len() as f64 / self.sample_size as f64) * 100.0,
            self.skewness, self.kurtosis
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_statistics() {
        let validator = StatisticalValidator::new();
        let data = vec![10.0, 12.0, 11.0, 13.0, 9.0, 14.0, 10.5, 11.5, 12.5, 13.5,
                       9.5, 14.5, 10.2, 11.8, 12.8, 13.2, 9.8, 14.2, 10.8, 11.2,
                       12.2, 13.8, 9.2, 14.8, 10.1, 11.9, 12.9, 13.1, 9.9, 14.1];

        let metrics = validator.analyze_measurements(&data).unwrap();

        assert!(metrics.mean > 0.0);
        assert!(metrics.std_dev > 0.0);
        assert!(metrics.confidence_interval.lower_bound < metrics.confidence_interval.upper_bound);
        assert_eq!(metrics.sample_size, 30);
    }

    #[test]
    fn test_measurement_comparison() {
        let validator = StatisticalValidator::new();

        // Baseline (slower)
        let baseline = vec![100.0; 30];

        // Treatment (faster)
        let treatment = vec![80.0; 30];

        let result = validator.compare_measurements(&baseline, &treatment).unwrap();

        assert!(result.is_significant);
        assert!(result.effect_size.unwrap() < 0.0); // Negative effect size (improvement)
    }

    #[test]
    fn test_outlier_detection() {
        let validator = StatisticalValidator::new();
        let mut data = vec![10.0; 100];
        data.push(1000.0); // Clear outlier

        let metrics = validator.analyze_measurements(&data).unwrap();

        assert!(!metrics.outliers.is_empty());
        assert!(metrics.outliers.contains(&1000.0));
    }

    #[test]
    fn test_insufficient_sample_size() {
        let validator = StatisticalValidator::new();
        let small_data = vec![1.0, 2.0, 3.0]; // Too small

        let result = validator.analyze_measurements(&small_data);
        assert!(result.is_err());
    }
}