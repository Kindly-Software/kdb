//! Statistical Validation Framework for B32-Compliant Benchmarks
//!
//! UCE32 Analysis Applied:
//! - Q30 (Empirical Validation): Statistical rigor for all performance claims
//! - Q29 (Practical Constraints): Hardware variance, measurement noise constraints
//! - Q28 (Simplicity): Simple API for complex statistical validation
//! - Kontext27 Reality Check: 10-50% improvement validation framework

use std::collections::HashMap;
use std::time::Duration;

/// B32 Statistical Validation Framework
pub struct B32StatisticalValidator {
    /// Sample measurements for statistical analysis
    measurements: Vec<f64>,
    /// Confidence level (default 95%)
    confidence_level: f64,
    /// Expected improvement percentage (for validation)
    expected_improvement: Option<f64>,
    /// Baseline measurements for comparison
    baseline_measurements: Option<Vec<f64>>,
}

impl B32StatisticalValidator {
    /// Create new validator with 95% confidence level
    pub fn new() -> Self {
        Self {
            measurements: Vec::new(),
            confidence_level: 0.95,
            expected_improvement: None,
            baseline_measurements: None,
        }
    }

    /// Set confidence level (e.g., 0.95 for 95%)
    pub fn with_confidence_level(mut self, level: f64) -> Self {
        self.confidence_level = level;
        self
    }

    /// Set expected improvement percentage for validation
    pub fn with_expected_improvement(mut self, improvement_pct: f64) -> Self {
        self.expected_improvement = Some(improvement_pct);
        self
    }

    /// Add measurement (in nanoseconds)
    pub fn add_measurement(&mut self, duration: Duration) {
        self.measurements.push(duration.as_nanos() as f64);
    }

    /// Add baseline measurements for comparison
    pub fn set_baseline(&mut self, baseline: Vec<Duration>) {
        self.baseline_measurements =
            Some(baseline.into_iter().map(|d| d.as_nanos() as f64).collect());
    }

    /// Calculate mean of measurements
    pub fn mean(&self) -> f64 {
        if self.measurements.is_empty() {
            return 0.0;
        }
        self.measurements.iter().sum::<f64>() / self.measurements.len() as f64
    }

    /// Calculate standard deviation
    pub fn std_dev(&self) -> f64 {
        if self.measurements.len() < 2 {
            return 0.0;
        }

        let mean = self.mean();
        let variance = self
            .measurements
            .iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>()
            / (self.measurements.len() - 1) as f64;

        variance.sqrt()
    }

    /// Calculate 95% confidence interval
    pub fn confidence_interval(&self) -> (f64, f64) {
        if self.measurements.len() < 2 {
            let mean = self.mean();
            return (mean, mean);
        }

        let mean = self.mean();
        let std_err = self.std_dev() / (self.measurements.len() as f64).sqrt();

        // t-value for 95% confidence (approximation for large samples)
        let t_value = if self.measurements.len() >= 30 {
            1.96 // Normal distribution
        } else {
            // Rough t-distribution values
            match self.measurements.len() {
                n if n >= 20 => 2.09,
                n if n >= 10 => 2.26,
                n if n >= 5 => 2.78,
                _ => 3.18,
            }
        };

        let margin_of_error = t_value * std_err;
        (mean - margin_of_error, mean + margin_of_error)
    }

    /// Calculate percentiles (p50, p95, p99)
    pub fn percentiles(&self) -> HashMap<u8, f64> {
        if self.measurements.is_empty() {
            return HashMap::new();
        }

        let mut sorted = self.measurements.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mut percentiles = HashMap::new();

        for &p in &[50, 95, 99] {
            let index = (p as f64 / 100.0 * (sorted.len() - 1) as f64) as usize;
            percentiles.insert(p, sorted[index.min(sorted.len() - 1)]);
        }

        percentiles
    }

    /// Compare against baseline and validate improvement claims
    pub fn validate_improvement(&self) -> ValidationResult {
        let baseline_measurements = match &self.baseline_measurements {
            Some(baseline) => baseline,
            None => return ValidationResult::NoBaseline,
        };

        if baseline_measurements.is_empty() || self.measurements.is_empty() {
            return ValidationResult::InsufficientData;
        }

        let baseline_mean =
            baseline_measurements.iter().sum::<f64>() / baseline_measurements.len() as f64;
        let optimized_mean = self.mean();

        let actual_improvement = if baseline_mean > 0.0 {
            ((baseline_mean - optimized_mean) / baseline_mean) * 100.0
        } else {
            0.0
        };

        let (confidence_lower, confidence_upper) = self.confidence_interval();
        let baseline_in_confidence =
            baseline_mean >= confidence_lower && baseline_mean <= confidence_upper;

        // Statistical significance test (simple t-test approximation)
        let is_significant = !baseline_in_confidence && optimized_mean < baseline_mean;

        ValidationResult::Comparison {
            baseline_mean_ns: baseline_mean,
            optimized_mean_ns: optimized_mean,
            actual_improvement_pct: actual_improvement,
            expected_improvement_pct: self.expected_improvement,
            confidence_interval: (confidence_lower, confidence_upper),
            is_statistically_significant: is_significant,
            meets_expectations: self.check_expectations(actual_improvement),
            kontext27_realistic: Self::check_kontext27_realism(actual_improvement),
        }
    }

    /// Check if improvement meets expected targets
    fn check_expectations(&self, actual_improvement: f64) -> bool {
        match self.expected_improvement {
            Some(expected) => {
                // Allow 10% tolerance on improvement claims
                let tolerance = expected * 0.1;
                actual_improvement >= (expected - tolerance)
            }
            None => true, // No expectations set
        }
    }

    /// Check if improvement is realistic per Kontext27 framework
    fn check_kontext27_realism(improvement_pct: f64) -> Kontext27Classification {
        match improvement_pct {
            x if x < 0.0 => Kontext27Classification::Regression,
            x if x <= 50.0 => Kontext27Classification::Typical,
            x if x <= 500.0 => Kontext27Classification::Exceptional,
            x if x <= 10000.0 => Kontext27Classification::Revolutionary,
            _ => Kontext27Classification::Suspicious,
        }
    }

    /// Generate comprehensive performance report
    pub fn generate_report(&self, test_name: &str) -> PerformanceReport {
        let validation = self.validate_improvement();
        let percentiles = self.percentiles();
        let (conf_lower, conf_upper) = self.confidence_interval();

        PerformanceReport {
            test_name: test_name.to_string(),
            sample_count: self.measurements.len(),
            mean_ns: self.mean(),
            std_dev_ns: self.std_dev(),
            confidence_interval_ns: (conf_lower, conf_upper),
            percentiles_ns: percentiles,
            validation_result: validation,
            recommendations: self.generate_recommendations(),
        }
    }

    /// Generate recommendations based on results
    fn generate_recommendations(&self) -> Vec<String> {
        let mut recommendations = Vec::new();

        if self.measurements.len() < 100 {
            recommendations.push(
                "Increase sample size to at least 100 for better statistical confidence"
                    .to_string(),
            );
        }

        let cv = if self.mean() > 0.0 {
            self.std_dev() / self.mean()
        } else {
            0.0
        };

        if cv > 0.1 {
            recommendations.push(format!("High coefficient of variation ({:.1}%) suggests measurement noise - consider warming up longer", cv * 100.0));
        }

        if let ValidationResult::Comparison {
            kontext27_realistic,
            ..
        } = self.validate_improvement()
        {
            match kontext27_realistic {
                Kontext27Classification::Suspicious => {
                    recommendations.push(
                        "Improvement claims appear unrealistic - verify measurement methodology"
                            .to_string(),
                    );
                }
                Kontext27Classification::Revolutionary => {
                    recommendations.push(
                        "Revolutionary improvement claims require extensive independent validation"
                            .to_string(),
                    );
                }
                _ => {}
            }
        }

        recommendations
    }
}

/// Result of performance validation
#[derive(Debug, Clone)]
pub enum ValidationResult {
    NoBaseline,
    InsufficientData,
    Comparison {
        baseline_mean_ns: f64,
        optimized_mean_ns: f64,
        actual_improvement_pct: f64,
        expected_improvement_pct: Option<f64>,
        confidence_interval: (f64, f64),
        is_statistically_significant: bool,
        meets_expectations: bool,
        kontext27_realistic: Kontext27Classification,
    },
}

/// Kontext27 realism classification
#[derive(Debug, Clone, PartialEq)]
pub enum Kontext27Classification {
    Regression,    // Performance got worse
    Typical,       // 0-50% improvement (realistic)
    Exceptional,   // 50-500% improvement (requires validation)
    Revolutionary, // 500-10000% improvement (extensive validation needed)
    Suspicious,    // >10000% improvement (likely measurement error)
}

/// Comprehensive performance report
#[derive(Debug, Clone)]
pub struct PerformanceReport {
    pub test_name: String,
    pub sample_count: usize,
    pub mean_ns: f64,
    pub std_dev_ns: f64,
    pub confidence_interval_ns: (f64, f64),
    pub percentiles_ns: HashMap<u8, f64>,
    pub validation_result: ValidationResult,
    pub recommendations: Vec<String>,
}

impl PerformanceReport {
    /// Print human-readable report
    pub fn print_report(&self) {
        println!(
            "\n=== B32 Statistical Validation Report: {} ===",
            self.test_name
        );
        println!("Sample Count: {}", self.sample_count);
        println!("Mean: {:.2} ns", self.mean_ns);
        println!(
            "Std Dev: {:.2} ns ({:.1}% CV)",
            self.std_dev_ns,
            (self.std_dev_ns / self.mean_ns) * 100.0
        );
        println!(
            "95% CI: [{:.2}, {:.2}] ns",
            self.confidence_interval_ns.0, self.confidence_interval_ns.1
        );

        if let Some(&p50) = self.percentiles_ns.get(&50) {
            println!("P50: {:.2} ns", p50);
        }
        if let Some(&p95) = self.percentiles_ns.get(&95) {
            println!("P95: {:.2} ns", p95);
        }
        if let Some(&p99) = self.percentiles_ns.get(&99) {
            println!("P99: {:.2} ns", p99);
        }

        match &self.validation_result {
            ValidationResult::Comparison {
                baseline_mean_ns,
                optimized_mean_ns,
                actual_improvement_pct,
                expected_improvement_pct,
                is_statistically_significant,
                meets_expectations,
                kontext27_realistic,
                ..
            } => {
                println!("\n--- Performance Comparison ---");
                println!("Baseline: {:.2} ns", baseline_mean_ns);
                println!("Optimized: {:.2} ns", optimized_mean_ns);
                println!("Improvement: {:.1}%", actual_improvement_pct);

                if let Some(expected) = expected_improvement_pct {
                    println!("Expected: {:.1}%", expected);
                    println!(
                        "Meets Expectations: {}",
                        if *meets_expectations { "✓" } else { "✗" }
                    );
                }

                println!(
                    "Statistically Significant: {}",
                    if *is_statistically_significant {
                        "✓"
                    } else {
                        "✗"
                    }
                );
                println!("Kontext27 Classification: {:?}", kontext27_realistic);
            }
            ValidationResult::NoBaseline => println!("No baseline provided for comparison"),
            ValidationResult::InsufficientData => println!("Insufficient data for validation"),
        }

        if !self.recommendations.is_empty() {
            println!("\n--- Recommendations ---");
            for (i, rec) in self.recommendations.iter().enumerate() {
                println!("{}. {}", i + 1, rec);
            }
        }
        println!("==========================================\n");
    }

    /// Check if results pass B32 validation
    pub fn passes_b32_validation(&self) -> bool {
        match &self.validation_result {
            ValidationResult::Comparison {
                is_statistically_significant,
                meets_expectations,
                kontext27_realistic,
                ..
            } => {
                *is_statistically_significant
                    && *meets_expectations
                    && matches!(
                        kontext27_realistic,
                        Kontext27Classification::Typical | Kontext27Classification::Exceptional
                    )
            }
            _ => false,
        }
    }
}

impl Default for B32StatisticalValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper macro for easy benchmark validation
#[macro_export]
macro_rules! b32_validate {
    ($test_name:expr, $measurements:expr, $baseline:expr, $expected_improvement:expr) => {{
        let mut validator =
            B32StatisticalValidator::new().with_expected_improvement($expected_improvement);

        for measurement in $measurements {
            validator.add_measurement(measurement);
        }

        if let Some(baseline) = $baseline {
            validator.set_baseline(baseline);
        }

        let report = validator.generate_report($test_name);
        report.print_report();
        report.passes_b32_validation()
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statistical_validator_basic() {
        let mut validator = B32StatisticalValidator::new();

        // Add some measurements
        for i in 0..100 {
            validator.add_measurement(Duration::from_nanos(50 + i));
        }

        assert!(validator.mean() > 90.0);
        assert!(validator.std_dev() > 0.0);

        let (lower, upper) = validator.confidence_interval();
        assert!(lower < upper);
        assert!(lower < validator.mean());
        assert!(upper > validator.mean());
    }

    #[test]
    fn test_improvement_validation() {
        let mut validator = B32StatisticalValidator::new().with_expected_improvement(30.0);

        // Baseline: 100ns
        let baseline: Vec<Duration> = (0..50).map(|_| Duration::from_nanos(100)).collect();
        validator.set_baseline(baseline);

        // Optimized: 70ns (30% improvement)
        for _ in 0..50 {
            validator.add_measurement(Duration::from_nanos(70));
        }

        let result = validator.validate_improvement();
        match result {
            ValidationResult::Comparison {
                meets_expectations, ..
            } => {
                assert!(meets_expectations);
            }
            _ => panic!("Expected comparison result"),
        }
    }

    #[test]
    fn test_kontext27_classification() {
        assert_eq!(
            B32StatisticalValidator::check_kontext27_realism(25.0),
            Kontext27Classification::Typical
        );
        assert_eq!(
            B32StatisticalValidator::check_kontext27_realism(200.0),
            Kontext27Classification::Exceptional
        );
        assert_eq!(
            B32StatisticalValidator::check_kontext27_realism(2000.0),
            Kontext27Classification::Revolutionary
        );
        assert_eq!(
            B32StatisticalValidator::check_kontext27_realism(20000.0),
            Kontext27Classification::Suspicious
        );
    }

    #[test]
    fn test_percentile_calculation() {
        let mut validator = B32StatisticalValidator::new();

        // Add measurements 1-100
        for i in 1..=100 {
            validator.add_measurement(Duration::from_nanos(i));
        }

        let percentiles = validator.percentiles();
        assert!(percentiles.get(&50).unwrap() > &40.0);
        assert!(percentiles.get(&95).unwrap() > &90.0);
        assert!(percentiles.get(&99).unwrap() > &95.0);
    }
}
