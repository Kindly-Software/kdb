//! B32 Benchmarking Framework Implementation
//!
//! Provides comprehensive benchmarking validation following B32 guidelines
//! with statistical rigor and hardware-aware performance measurement.

use std::time::{Duration, Instant};
use std::collections::HashMap;
use crate::{TestResult, TestSupportError, hardware_constants};

/// B32-compliant benchmark validator
#[derive(Debug, Clone)]
pub struct BenchmarkValidator {
    config: B32Configuration,
    baselines: HashMap<String, f64>,
    hardware_limits: HardwareLimits,
}

/// B32 benchmark configuration
#[derive(Debug, Clone)]
pub struct B32Configuration {
    pub confidence_level: f64,
    pub min_iterations: usize,
    pub warmup_iterations: usize,
    pub sustained_duration: Duration,
    pub report_percentiles: bool,
    pub require_multiple_baselines: bool,
    pub max_acceptable_variance: f64,
}

/// Hardware performance limits for validation
#[derive(Debug, Clone)]
pub struct HardwareLimits {
    pub min_atomic_cas_ns: f64,
    pub min_memory_access_ns: f64,
    pub max_threads_efficient: usize,
    pub cache_line_size: usize,
}

/// Comprehensive benchmark result
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub mean_ns: f64,
    pub std_dev_ns: f64,
    pub confidence_interval: (f64, f64),
    pub percentiles: Percentiles,
    pub iterations: usize,
    pub sustained_duration: Duration,
    pub baseline_comparison: Option<BaselineComparison>,
    pub b32_compliance: B32Compliance,
}

/// Performance percentiles
#[derive(Debug, Clone)]
pub struct Percentiles {
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub p99_9: f64,
}

/// Baseline comparison result
#[derive(Debug, Clone)]
pub struct BaselineComparison {
    pub baseline_name: String,
    pub baseline_ns: f64,
    pub speedup: f64,
    pub improvement_type: ImprovementType,
}

/// Classification of performance improvement
#[derive(Debug, Clone, PartialEq)]
pub enum ImprovementType {
    Typical,      // 10-50%
    Exceptional,  // 50-200%
    Suspicious,   // >200% (needs verification)
    Regression,   // Slower than baseline
}

/// B32 compliance check result
#[derive(Debug, Clone)]
pub struct B32Compliance {
    pub is_compliant: bool,
    pub violations: Vec<String>,
    pub recommendations: Vec<String>,
}

impl Default for B32Configuration {
    fn default() -> Self {
        Self {
            confidence_level: hardware_constants::REQUIRED_CONFIDENCE_LEVEL,
            min_iterations: hardware_constants::MIN_BENCHMARK_ITERATIONS,
            warmup_iterations: 100,
            sustained_duration: hardware_constants::MIN_SUSTAINED_DURATION,
            report_percentiles: true,
            require_multiple_baselines: false,
            max_acceptable_variance: 0.15, // 15%
        }
    }
}

impl Default for HardwareLimits {
    fn default() -> Self {
        Self {
            min_atomic_cas_ns: hardware_constants::ATOMIC_U64_CAS_NS as f64,
            min_memory_access_ns: hardware_constants::L1_CACHE_NS as f64,
            max_threads_efficient: hardware_constants::EFFICIENT_THREAD_COUNT,
            cache_line_size: hardware_constants::CACHE_LINE_SIZE,
        }
    }
}

impl Default for BenchmarkValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchmarkValidator {
    /// Create new B32-compliant benchmark validator
    pub fn new() -> Self {
        Self {
            config: B32Configuration::default(),
            baselines: HashMap::new(),
            hardware_limits: HardwareLimits::default(),
        }
    }

    /// Configure benchmark parameters
    pub fn with_config(mut self, config: B32Configuration) -> Self {
        self.config = config;
        self
    }

    /// Add baseline for comparison
    pub fn with_baseline(mut self, name: &str, time_ns: f64) -> Self {
        self.baselines.insert(name.to_string(), time_ns);
        self
    }

    /// Add multiple baselines (B32 guideline B1: No strawmen)
    pub fn with_baselines(mut self, baselines: HashMap<String, f64>) -> Self {
        self.baselines.extend(baselines);
        self
    }

    /// Measure operation performance with B32 compliance
    pub fn measure_operation<F>(&self, mut operation: F) -> TestResult<BenchmarkResult>
    where
        F: FnMut(),
    {
        // Warmup phase
        for _ in 0..self.config.warmup_iterations {
            operation();
        }

        // Main measurement phase
        let mut measurements = Vec::with_capacity(self.config.min_iterations);
        let start_time = Instant::now();

        for _ in 0..self.config.min_iterations {
            let measure_start = Instant::now();
            operation();
            let elapsed = measure_start.elapsed();
            measurements.push(elapsed.as_nanos() as f64);
        }

        let total_duration = start_time.elapsed();

        // Statistical analysis
        let mean_ns = measurements.iter().sum::<f64>() / measurements.len() as f64;
        let variance = measurements.iter()
            .map(|x| (x - mean_ns).powi(2))
            .sum::<f64>() / (measurements.len() - 1) as f64;
        let std_dev_ns = variance.sqrt();

        // Calculate confidence interval (95% by default)
        let t_value = self.calculate_t_value(measurements.len() - 1)?;
        let margin_error = t_value * std_dev_ns / (measurements.len() as f64).sqrt();
        let confidence_interval = (mean_ns - margin_error, mean_ns + margin_error);

        // Calculate percentiles
        let mut sorted_measurements = measurements.clone();
        sorted_measurements.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let percentiles = Percentiles {
            p50: self.percentile(&sorted_measurements, 0.50),
            p95: self.percentile(&sorted_measurements, 0.95),
            p99: self.percentile(&sorted_measurements, 0.99),
            p99_9: self.percentile(&sorted_measurements, 0.999),
        };

        // Baseline comparison
        let baseline_comparison = self.find_best_baseline()
            .map(|(name, baseline_ns)| {
                let speedup = baseline_ns / mean_ns;
                let improvement_type = self.classify_improvement(speedup);
                BaselineComparison {
                    baseline_name: name,
                    baseline_ns,
                    speedup,
                    improvement_type,
                }
            });

        // B32 compliance check
        let b32_compliance = self.check_b32_compliance(
            mean_ns,
            std_dev_ns,
            measurements.len(),
            total_duration,
            &baseline_comparison,
        )?;

        Ok(BenchmarkResult {
            mean_ns,
            std_dev_ns,
            confidence_interval,
            percentiles,
            iterations: measurements.len(),
            sustained_duration: total_duration,
            baseline_comparison,
            b32_compliance,
        })
    }

    /// Measure contention scaling (B32 guideline B4)
    pub fn measure_contention_scaling<F>(&self, operation: F, thread_counts: &[usize])
        -> TestResult<HashMap<usize, BenchmarkResult>>
    where
        F: Fn() + Send + Sync + Clone + 'static,
    {
        let mut results = HashMap::new();

        for &num_threads in thread_counts {
            if num_threads > self.hardware_limits.max_threads_efficient {
                // Warn about exceeding efficient thread count
                eprintln!(
                    "Warning: {} threads exceeds efficient count of {}",
                    num_threads, self.hardware_limits.max_threads_efficient
                );
            }

            let result = self.measure_multi_threaded(operation.clone(), num_threads)?;
            results.insert(num_threads, result);
        }

        Ok(results)
    }

    fn measure_multi_threaded<F>(&self, operation: F, num_threads: usize) -> TestResult<BenchmarkResult>
    where
        F: Fn() + Send + Sync + 'static,
    {
        use std::sync::Arc;
        use std::thread;

        let operation = Arc::new(operation);
        let mut handles = Vec::new();
        let mut thread_results = Vec::new();

        let start_time = Instant::now();

        for _ in 0..num_threads {
            let op = Arc::clone(&operation);
            let iterations_per_thread = self.config.min_iterations / num_threads;

            let handle = thread::spawn(move || {
                let mut measurements = Vec::new();

                // Warmup
                for _ in 0..10 {
                    op();
                }

                // Measure
                for _ in 0..iterations_per_thread {
                    let measure_start = Instant::now();
                    op();
                    let elapsed = measure_start.elapsed();
                    measurements.push(elapsed.as_nanos() as f64);
                }

                measurements
            });

            handles.push(handle);
        }

        for handle in handles {
            let measurements = handle.join().map_err(|_| {
                TestSupportError::BenchmarkValidation {
                    reason: "Thread join failed during multi-threaded benchmark".to_string(),
                }
            })?;
            thread_results.extend(measurements);
        }

        let total_duration = start_time.elapsed();

        // Aggregate analysis similar to single-threaded case
        self.analyze_measurements(thread_results, total_duration)
    }

    fn analyze_measurements(&self, measurements: Vec<f64>, duration: Duration) -> TestResult<BenchmarkResult> {
        let mean_ns = measurements.iter().sum::<f64>() / measurements.len() as f64;
        let variance = measurements.iter()
            .map(|x| (x - mean_ns).powi(2))
            .sum::<f64>() / (measurements.len() - 1) as f64;
        let std_dev_ns = variance.sqrt();

        let t_value = self.calculate_t_value(measurements.len() - 1)?;
        let margin_error = t_value * std_dev_ns / (measurements.len() as f64).sqrt();
        let confidence_interval = (mean_ns - margin_error, mean_ns + margin_error);

        let mut sorted_measurements = measurements.clone();
        sorted_measurements.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let percentiles = Percentiles {
            p50: self.percentile(&sorted_measurements, 0.50),
            p95: self.percentile(&sorted_measurements, 0.95),
            p99: self.percentile(&sorted_measurements, 0.99),
            p99_9: self.percentile(&sorted_measurements, 0.999),
        };

        let baseline_comparison = self.find_best_baseline()
            .map(|(name, baseline_ns)| {
                let speedup = baseline_ns / mean_ns;
                let improvement_type = self.classify_improvement(speedup);
                BaselineComparison {
                    baseline_name: name,
                    baseline_ns,
                    speedup,
                    improvement_type,
                }
            });

        let b32_compliance = self.check_b32_compliance(
            mean_ns,
            std_dev_ns,
            measurements.len(),
            duration,
            &baseline_comparison,
        )?;

        Ok(BenchmarkResult {
            mean_ns,
            std_dev_ns,
            confidence_interval,
            percentiles,
            iterations: measurements.len(),
            sustained_duration: duration,
            baseline_comparison,
            b32_compliance,
        })
    }

    fn calculate_t_value(&self, degrees_freedom: usize) -> TestResult<f64> {
        // Simplified t-table lookup for 95% confidence
        let t_95 = match degrees_freedom {
            df if df >= 1000 => 1.96,
            df if df >= 100 => 1.98,
            df if df >= 30 => 2.04,
            df if df >= 10 => 2.23,
            _ => 2.78, // Conservative estimate
        };
        Ok(t_95)
    }

    fn percentile(&self, sorted_data: &[f64], p: f64) -> f64 {
        let index = (p * (sorted_data.len() - 1) as f64) as usize;
        sorted_data[index.min(sorted_data.len() - 1)]
    }

    fn find_best_baseline(&self) -> Option<(String, f64)> {
        self.baselines.iter()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(name, &time)| (name.clone(), time))
    }

    fn classify_improvement(&self, speedup: f64) -> ImprovementType {
        if speedup < 1.0 {
            ImprovementType::Regression
        } else if speedup <= hardware_constants::TYPICAL_IMPROVEMENT_MAX {
            ImprovementType::Typical
        } else if speedup <= hardware_constants::EXCEPTIONAL_IMPROVEMENT_MAX {
            ImprovementType::Exceptional
        } else {
            ImprovementType::Suspicious
        }
    }

    fn check_b32_compliance(
        &self,
        mean_ns: f64,
        std_dev_ns: f64,
        iterations: usize,
        duration: Duration,
        baseline_comparison: &Option<BaselineComparison>,
    ) -> TestResult<B32Compliance> {
        let mut violations = Vec::new();
        let mut recommendations = Vec::new();

        // Check minimum iterations (B2)
        if iterations < self.config.min_iterations {
            violations.push(format!(
                "Insufficient iterations: {} < {}",
                iterations, self.config.min_iterations
            ));
        }

        // Check sustained duration (B3)
        if duration < self.config.sustained_duration {
            violations.push(format!(
                "Duration too short: {:?} < {:?}",
                duration, self.config.sustained_duration
            ));
        }

        // Check variance (B2)
        let relative_variance = std_dev_ns / mean_ns;
        if relative_variance > self.config.max_acceptable_variance {
            violations.push(format!(
                "High variance: {:.1}% > {:.1}%",
                relative_variance * 100.0,
                self.config.max_acceptable_variance * 100.0
            ));
        }

        // Check baseline comparison (B1)
        if let Some(comparison) = baseline_comparison {
            if comparison.improvement_type == ImprovementType::Suspicious {
                violations.push(format!(
                    "Suspicious speedup: {:.2}x may indicate measurement error",
                    comparison.speedup
                ));
                recommendations.push(
                    "Verify measurement methodology and check for strawman baseline".to_string()
                );
            }
        } else if self.config.require_multiple_baselines {
            violations.push("No baseline comparison provided".to_string());
            recommendations.push("Add optimized baselines for fair comparison".to_string());
        }

        // Hardware sanity checks
        if mean_ns < self.hardware_limits.min_atomic_cas_ns {
            violations.push(format!(
                "Operation faster than hardware limits: {:.2}ns < {:.2}ns",
                mean_ns, self.hardware_limits.min_atomic_cas_ns
            ));
        }

        let is_compliant = violations.is_empty();

        Ok(B32Compliance {
            is_compliant,
            violations,
            recommendations,
        })
    }
}

impl BenchmarkResult {
    /// Check if benchmark meets B32 standards
    pub fn meets_b32_standards(&self) -> bool {
        self.b32_compliance.is_compliant
    }

    /// Generate B32-compliant report
    pub fn generate_report(&self) -> String {
        let mut report = String::new();

        report.push_str(&"B32 Benchmark Report\n".to_string());
        report.push_str(&"====================\n\n".to_string());

        report.push_str(&"Performance Metrics:\n".to_string());
        report.push_str(&format!("  Mean: {:.2}ns\n", self.mean_ns));
        report.push_str(&format!("  95% CI: ({:.2}, {:.2})ns\n",
            self.confidence_interval.0, self.confidence_interval.1));
        report.push_str(&format!("  Std Dev: {:.2}ns ({:.1}%)\n",
            self.std_dev_ns, (self.std_dev_ns / self.mean_ns) * 100.0));

        report.push_str(&"\nPercentiles:\n".to_string());
        report.push_str(&format!("  P50: {:.2}ns\n", self.percentiles.p50));
        report.push_str(&format!("  P95: {:.2}ns\n", self.percentiles.p95));
        report.push_str(&format!("  P99: {:.2}ns\n", self.percentiles.p99));
        report.push_str(&format!("  P99.9: {:.2}ns\n", self.percentiles.p99_9));

        if let Some(ref comparison) = self.baseline_comparison {
            report.push_str(&"\nBaseline Comparison:\n".to_string());
            report.push_str(&format!("  Baseline ({}): {:.2}ns\n",
                comparison.baseline_name, comparison.baseline_ns));
            report.push_str(&format!("  Speedup: {:.2}x\n", comparison.speedup));
            report.push_str(&format!("  Improvement: {:?}\n", comparison.improvement_type));
        }

        report.push_str(&format!("\nB32 Compliance: {}\n",
            if self.b32_compliance.is_compliant { "PASS" } else { "FAIL" }));

        if !self.b32_compliance.violations.is_empty() {
            report.push_str(&"\nViolations:\n".to_string());
            for violation in &self.b32_compliance.violations {
                report.push_str(&format!("  - {}\n", violation));
            }
        }

        if !self.b32_compliance.recommendations.is_empty() {
            report.push_str(&"\nRecommendations:\n".to_string());
            for recommendation in &self.b32_compliance.recommendations {
                report.push_str(&format!("  - {}\n", recommendation));
            }
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn test_benchmark_validator_basic() {
        let validator = BenchmarkValidator::new();
        let atomic = AtomicU64::new(0);

        let result = validator.measure_operation(|| {
            atomic.fetch_add(1, Ordering::Relaxed);
        }).unwrap();

        assert!(result.iterations >= 1000);
        assert!(result.mean_ns > 0.0);
        assert!(result.std_dev_ns >= 0.0);
        assert!(result.confidence_interval.0 < result.confidence_interval.1);
    }

    #[test]
    fn test_improvement_classification() {
        let validator = BenchmarkValidator::new();

        assert_eq!(validator.classify_improvement(0.9), ImprovementType::Regression);
        assert_eq!(validator.classify_improvement(1.2), ImprovementType::Typical);
        assert_eq!(validator.classify_improvement(1.8), ImprovementType::Exceptional);
        assert_eq!(validator.classify_improvement(15.0), ImprovementType::Suspicious);
    }

    #[test]
    fn test_b32_compliance_check() {
        let validator = BenchmarkValidator::new()
            .with_baseline("mutex", 100.0);

        let atomic = AtomicU64::new(0);
        let result = validator.measure_operation(|| {
            atomic.fetch_add(1, Ordering::Relaxed);
        }).unwrap();

        // Should pass basic compliance checks
        assert!(!result.b32_compliance.violations.iter()
            .any(|v| v.contains("Insufficient iterations")));
    }
}