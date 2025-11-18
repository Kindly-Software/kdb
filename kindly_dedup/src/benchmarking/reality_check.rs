//! # Reality Check Module (B32 Guidelines K1-K70)
//!
//! **Hardware Reality Validation for Performance Claims**
//!
//! Classifies speedup claims against B32 hardware reality checks (K1-K70):
//! - **Marginal**: <10% (noise level)
//! - **Typical**: 10-50% (most optimizations)
//! - **Good**: 50-100% (solid improvement)
//! - **Exceptional**: 2-10× (requires validation, B32 K27)
//! - **Breakthrough**: 10×+ (extensive validation required, often algorithm change)
//!
//! ## B32 Reality Checks
//!
//! - **K1-K9**: Physical hardware constraints (CPU, memory, cache)
//! - **K10-K18**: Algorithm and implementation truths
//! - **K19-K27**: System-level end-to-end reality
//! - **K28-K42**: Foundation tier capsule constraints (T4-T6)
//! - **K43-K50**: Production concerns (tail latency, energy efficiency)
//! - **K51-K70**: Extended tier reality (T7-T10: GPU, Network, Persistent, Probabilistic)
//!
//! ## Example
//!
//! ```rust,ignore
//! use kindly_dedup::benchmarking::RealityCheck;
//!
//! let check = RealityCheck::new(1000.0, 38000.0); // 38× speedup
//! assert_eq!(check.classify(), SpeedupClassification::Breakthrough);
//! println!("{}", check.format_assessment());
//! ```
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_SPEEDUP_CALCULATION`: Assumes baseline and optimized measurements are accurate
//! - `#VERIFY_CLASSIFICATION`: Tests validate threshold boundaries
//! - `#ASSUME_HARDWARE_LIMITS`: B32 K1-K70 reality checks are evidence-based
//! - `#VERIFY_RECOMMENDATIONS`: Tests validate recommendation logic
//!
//! **Safety Rating**: 100% (pure Rust arithmetic, no unsafe code, no external dependencies)

use std::fmt;

/// Reality check for speedup claims
#[derive(Debug, Clone)]
pub struct RealityCheck {
    /// Baseline performance (throughput or 1/latency)
    baseline: f64,

    /// Optimized performance (throughput or 1/latency)
    optimized: f64,
}

impl RealityCheck {
    /// Create new reality check
    ///
    /// # Arguments
    ///
    /// * `baseline` - Baseline performance (throughput in ops/sec OR 1/latency)
    /// * `optimized` - Optimized performance (same units as baseline)
    ///
    /// # Example
    ///
    /// ```
    /// use kindly_dedup::benchmarking::RealityCheck;
    ///
    /// // Throughput: 1,000 → 2,000 ops/sec = 2× speedup
    /// let check = RealityCheck::new(1000.0, 2000.0);
    /// assert!((check.speedup() - 2.0).abs() < 0.01);
    /// ```
    pub fn new(baseline: f64, optimized: f64) -> Self {
        Self { baseline, optimized }
    }

    /// Calculate speedup factor
    ///
    /// Speedup = optimized / baseline
    pub fn speedup(&self) -> f64 {
        if self.baseline == 0.0 {
            return 0.0;
        }
        self.optimized / self.baseline
    }

    /// Calculate improvement percentage
    ///
    /// Improvement = (optimized - baseline) / baseline × 100%
    pub fn improvement_percent(&self) -> f64 {
        if self.baseline == 0.0 {
            return 0.0;
        }
        ((self.optimized - self.baseline) / self.baseline) * 100.0
    }

    /// Classify speedup according to B32 reality checks
    ///
    /// - **Marginal**: <10% (within noise, not meaningful)
    /// - **Typical**: 10-50% (most optimizations, B32 K27)
    /// - **Good**: 50-100% (solid improvement)
    /// - **Exceptional**: 2-10× (requires validation, B32 K27)
    /// - **Breakthrough**: 10×+ (extensive validation required, often new algorithm)
    pub fn classify(&self) -> SpeedupClassification {
        let speedup = self.speedup();

        if speedup < 1.1 {
            SpeedupClassification::Marginal
        } else if speedup < 1.5 {
            SpeedupClassification::Typical
        } else if speedup < 2.0 {
            SpeedupClassification::Good
        } else if speedup < 10.0 {
            SpeedupClassification::Exceptional
        } else {
            SpeedupClassification::Breakthrough
        }
    }

    /// Get recommendation based on classification
    pub fn recommendation(&self) -> String {
        match self.classify() {
            SpeedupClassification::Marginal => {
                "Performance gain is marginal (<10%). Consider whether optimization is worth the complexity."
                    .to_string()
            }
            SpeedupClassification::Typical => {
                "Typical optimization result (10-50%). Good improvement for production.".to_string()
            }
            SpeedupClassification::Good => {
                "Good optimization (50-100%). Solid improvement, validate with B32 benchmarks.".to_string()
            }
            SpeedupClassification::Exceptional => {
                format!(
                    "Exceptional speedup ({:.1}×). REQUIRES VALIDATION:\n\
                     - Verify baseline is optimized (not strawman)\n\
                     - Measure on actual hardware (B32 K1-K9)\n\
                     - Test sustained performance (>60s, B32 K21)\n\
                     - Report percentiles (P50/P95/P99, B32 K19)\n\
                     - Document thermal throttling (B32 K5, K21)\n\
                     - Multiple independent runs (B32 B2)",
                    self.speedup()
                )
            }
            SpeedupClassification::Breakthrough => {
                format!(
                    "Breakthrough speedup ({:.1}×). EXTENSIVE VALIDATION REQUIRED:\n\
                     - Verify algorithm change or paradigm shift\n\
                     - Compare against best-in-class baselines (B32 B1)\n\
                     - Measure on production workloads (B32 B3)\n\
                     - Test multiple hardware configurations (B32 B24)\n\
                     - Verify scaling with realistic data sizes\n\
                     - Document all assumptions (ASSUM framework)\n\
                     - Independent reproduction by third party\n\
                     - Consider tier reality checks:\n\
                       * T2 SIMD: 3-4× typical, 19× exceptional (K30)\n\
                       * T4 Batch: 10-100× with parallelism (K31)\n\
                       * T7 GPU: 100-500× for large data (K55)\n\
                       * T10 Probabilistic: 100-1000× vs exact (K70)",
                    self.speedup()
                )
            }
        }
    }

    /// Format complete assessment
    pub fn format_assessment(&self) -> String {
        format!(
            "Reality Check Assessment:\n\
             ========================\n\
             Baseline:    {:.2} ops/sec\n\
             Optimized:   {:.2} ops/sec\n\
             Speedup:     {:.2}×\n\
             Improvement: {:.1}%\n\
             \n\
             Classification: {:?}\n\
             \n\
             Recommendation:\n\
             {}\n",
            self.baseline,
            self.optimized,
            self.speedup(),
            self.improvement_percent(),
            self.classify(),
            self.recommendation()
        )
    }

    /// Check against specific B32 reality constraint
    ///
    /// Returns true if speedup claim respects the given constraint.
    pub fn check_constraint(&self, constraint: B32Constraint) -> bool {
        let speedup = self.speedup();

        match constraint {
            B32Constraint::AtomicCAS => {
                // K2: AtomicU64 CAS is 10-15ns
                // Maximum theoretical speedup from CAS optimization is ~2-3×
                speedup <= 3.0
            }
            B32Constraint::SimdAvx2 => {
                // K9, K30: AVX2 typical 3-4×, exceptional 19× (Hebbian)
                speedup <= 20.0
            }
            B32Constraint::ParallelScaling6Cores => {
                // K20, K31: 6 P-cores = 6.5× actual (not 6×)
                speedup <= 7.0
            }
            B32Constraint::MemoryBandwidth => {
                // K3, K29: 15.2 GB/s measured (17% of theoretical)
                // Memory-bound algorithms saturate at ~8-12 threads
                speedup <= 12.0
            }
            B32Constraint::BatchProcessing => {
                // K28-K34: Batch 512-4096 optimal, 10-100× throughput
                speedup <= 100.0
            }
            B32Constraint::CompoundTiers => {
                // K39: Compound 60-80% efficiency of theoretical
                // T1 (3×) × T2 (4×) × T4 (10×) = 120× theoretical → 72-96× actual
                speedup <= 100.0
            }
        }
    }
}

/// Speedup classification (B32 K27)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedupClassification {
    /// <10% improvement (noise level)
    Marginal,

    /// 10-50% improvement (typical optimization)
    Typical,

    /// 50-100% improvement (good optimization)
    Good,

    /// 2-10× speedup (exceptional, requires validation)
    Exceptional,

    /// 10×+ speedup (breakthrough, extensive validation required)
    Breakthrough,
}

impl fmt::Display for SpeedupClassification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpeedupClassification::Marginal => write!(f, "Marginal (<10%)"),
            SpeedupClassification::Typical => write!(f, "Typical (10-50%)"),
            SpeedupClassification::Good => write!(f, "Good (50-100%)"),
            SpeedupClassification::Exceptional => write!(f, "Exceptional (2-10×)"),
            SpeedupClassification::Breakthrough => write!(f, "Breakthrough (10×+)"),
        }
    }
}

/// B32 hardware reality constraints
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum B32Constraint {
    /// K2: Atomic CAS operations (10-15ns, max ~3× speedup)
    AtomicCAS,

    /// K9, K30: SIMD AVX2 (3-4× typical, 19× exceptional)
    SimdAvx2,

    /// K20, K31: Parallel scaling on 6 P-cores (6.5× actual)
    ParallelScaling6Cores,

    /// K3, K29: Memory bandwidth saturation (15.2 GB/s, 8-12 threads)
    MemoryBandwidth,

    /// K28-K34: Batch processing (10-100× throughput)
    BatchProcessing,

    /// K39: Compound tier efficiency (60-80% of theoretical)
    CompoundTiers,
}

// ============================================================================
// TESTS (T28 Framework)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reality_check_new() {
        let check = RealityCheck::new(1000.0, 2000.0);
        assert_eq!(check.baseline, 1000.0);
        assert_eq!(check.optimized, 2000.0);
    }

    #[test]
    fn test_speedup_calculation() {
        let check = RealityCheck::new(1000.0, 2000.0);
        assert!((check.speedup() - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_improvement_percent() {
        let check = RealityCheck::new(1000.0, 2000.0);
        assert!((check.improvement_percent() - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_classify_marginal() {
        let check = RealityCheck::new(1000.0, 1050.0); // 5% improvement
        assert_eq!(check.classify(), SpeedupClassification::Marginal);
    }

    #[test]
    fn test_classify_typical() {
        let check = RealityCheck::new(1000.0, 1300.0); // 30% improvement
        assert_eq!(check.classify(), SpeedupClassification::Typical);
    }

    #[test]
    fn test_classify_good() {
        let check = RealityCheck::new(1000.0, 1800.0); // 80% improvement
        assert_eq!(check.classify(), SpeedupClassification::Good);
    }

    #[test]
    fn test_classify_exceptional() {
        let check = RealityCheck::new(1000.0, 5000.0); // 5× speedup
        assert_eq!(check.classify(), SpeedupClassification::Exceptional);
    }

    #[test]
    fn test_classify_breakthrough() {
        let check = RealityCheck::new(1000.0, 38000.0); // 38× speedup (kindly_dedup actual)
        assert_eq!(check.classify(), SpeedupClassification::Breakthrough);
    }

    #[test]
    fn test_classification_boundaries() {
        // Test boundary conditions
        assert_eq!(
            RealityCheck::new(1000.0, 1099.0).classify(),
            SpeedupClassification::Marginal
        );
        assert_eq!(
            RealityCheck::new(1000.0, 1100.0).classify(),
            SpeedupClassification::Typical
        );
        assert_eq!(
            RealityCheck::new(1000.0, 1499.0).classify(),
            SpeedupClassification::Typical
        );
        assert_eq!(
            RealityCheck::new(1000.0, 1500.0).classify(),
            SpeedupClassification::Good
        );
        assert_eq!(
            RealityCheck::new(1000.0, 1999.0).classify(),
            SpeedupClassification::Good
        );
        assert_eq!(
            RealityCheck::new(1000.0, 2000.0).classify(),
            SpeedupClassification::Exceptional
        );
        assert_eq!(
            RealityCheck::new(1000.0, 9999.0).classify(),
            SpeedupClassification::Exceptional
        );
        assert_eq!(
            RealityCheck::new(1000.0, 10000.0).classify(),
            SpeedupClassification::Breakthrough
        );
    }

    #[test]
    fn test_recommendation_marginal() {
        let check = RealityCheck::new(1000.0, 1050.0);
        let rec = check.recommendation();
        assert!(rec.contains("marginal"));
        assert!(rec.contains("complexity"));
    }

    #[test]
    fn test_recommendation_exceptional() {
        let check = RealityCheck::new(1000.0, 5000.0);
        let rec = check.recommendation();
        assert!(rec.contains("REQUIRES VALIDATION"));
        assert!(rec.contains("baseline"));
        assert!(rec.contains("hardware"));
    }

    #[test]
    fn test_recommendation_breakthrough() {
        let check = RealityCheck::new(1000.0, 38000.0);
        let rec = check.recommendation();
        assert!(rec.contains("EXTENSIVE VALIDATION"));
        assert!(rec.contains("algorithm change"));
        assert!(rec.contains("tier reality"));
    }

    #[test]
    fn test_format_assessment() {
        let check = RealityCheck::new(1000.0, 2000.0);
        let assessment = check.format_assessment();

        assert!(assessment.contains("Reality Check Assessment"));
        assert!(assessment.contains("Baseline"));
        assert!(assessment.contains("Optimized"));
        assert!(assessment.contains("Speedup"));
        assert!(assessment.contains("Classification"));
        assert!(assessment.contains("Recommendation"));
    }

    #[test]
    fn test_check_constraint_atomic_cas() {
        // Valid: 2× speedup from CAS optimization
        let check1 = RealityCheck::new(1000.0, 2000.0);
        assert!(check1.check_constraint(B32Constraint::AtomicCAS));

        // Invalid: 5× speedup impossible from CAS alone
        let check2 = RealityCheck::new(1000.0, 5000.0);
        assert!(!check2.check_constraint(B32Constraint::AtomicCAS));
    }

    #[test]
    fn test_check_constraint_simd_avx2() {
        // Valid: 4× SIMD speedup (typical)
        let check1 = RealityCheck::new(1000.0, 4000.0);
        assert!(check1.check_constraint(B32Constraint::SimdAvx2));

        // Valid: 19× SIMD speedup (exceptional, Hebbian)
        let check2 = RealityCheck::new(1000.0, 19000.0);
        assert!(check2.check_constraint(B32Constraint::SimdAvx2));

        // Invalid: 30× speedup impossible from AVX2 alone
        let check3 = RealityCheck::new(1000.0, 30000.0);
        assert!(!check3.check_constraint(B32Constraint::SimdAvx2));
    }

    #[test]
    fn test_check_constraint_parallel_scaling() {
        // Valid: 6.5× speedup on 6 P-cores
        let check1 = RealityCheck::new(1000.0, 6500.0);
        assert!(check1.check_constraint(B32Constraint::ParallelScaling6Cores));

        // Invalid: 12× speedup impossible on 6 cores
        let check2 = RealityCheck::new(1000.0, 12000.0);
        assert!(!check2.check_constraint(B32Constraint::ParallelScaling6Cores));
    }

    #[test]
    fn test_check_constraint_batch_processing() {
        // Valid: 50× batch throughput improvement
        let check1 = RealityCheck::new(1000.0, 50000.0);
        assert!(check1.check_constraint(B32Constraint::BatchProcessing));

        // Invalid: 200× impossible for batch alone
        let check2 = RealityCheck::new(1000.0, 200000.0);
        assert!(!check2.check_constraint(B32Constraint::BatchProcessing));
    }

    #[test]
    fn test_check_constraint_compound_tiers() {
        // Valid: 50× compound speedup (T1+T2+T4)
        let check1 = RealityCheck::new(1000.0, 50000.0);
        assert!(check1.check_constraint(B32Constraint::CompoundTiers));

        // Invalid: 200× requires more tiers or algorithm change
        let check2 = RealityCheck::new(1000.0, 200000.0);
        assert!(!check2.check_constraint(B32Constraint::CompoundTiers));
    }

    #[test]
    fn test_speedup_classification_display() {
        assert_eq!(format!("{}", SpeedupClassification::Marginal), "Marginal (<10%)");
        assert_eq!(format!("{}", SpeedupClassification::Typical), "Typical (10-50%)");
        assert_eq!(format!("{}", SpeedupClassification::Good), "Good (50-100%)");
        assert_eq!(format!("{}", SpeedupClassification::Exceptional), "Exceptional (2-10×)");
        assert_eq!(
            format!("{}", SpeedupClassification::Breakthrough),
            "Breakthrough (10×+)"
        );
    }

    #[test]
    fn test_zero_baseline() {
        let check = RealityCheck::new(0.0, 1000.0);
        assert_eq!(check.speedup(), 0.0);
        assert_eq!(check.improvement_percent(), 0.0);
    }
}
