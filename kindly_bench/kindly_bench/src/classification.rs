//! Performance tier classification based on B32 K27 guidelines
//!
//! Classifies benchmark results as:
//! - TYPICAL: 1.1-1.5× (10-50% improvement)
//! - EXCEPTIONAL: 1.5-2.5× (50-150% improvement)
//! - BREAKTHROUGH: 2.5-10× (150-900% improvement)
//! - SUSPICIOUS: >10× (requires manual validation)

use crate::stats::{Speedup, SpeedupConfidenceInterval};

/// Performance tier classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceTier {
    /// 1.1-1.5× (10-50% improvement) - Good but typical
    Typical,
    /// 1.5-2.5× (50-150% improvement) - Exceptional performance
    Exceptional,
    /// 2.5-10× (150-900% improvement) - Breakthrough optimization
    Breakthrough,
    /// >10× - Suspicious, requires manual validation
    Suspicious,
}

/// Confidence level for classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfidenceLevel {
    /// High confidence - CI doesn't span tier boundaries
    High,
    /// Medium confidence - CI spans 1 tier boundary
    Medium,
    /// Low confidence - CI spans 2+ tier boundaries or high variance
    Low,
}

/// Classification result with confidence
#[derive(Debug, Clone)]
pub struct Classification {
    /// Performance tier
    pub tier: PerformanceTier,
    /// Confidence level
    pub confidence: ConfidenceLevel,
    /// Flags for special conditions
    pub flags: Vec<String>,
}

impl Classification {
    /// Classify speedup based on B32 K27 guidelines
    ///
    /// # Arguments
    /// * `speedup` - Speedup measurements from statistics
    ///
    /// # Returns
    /// Classification with tier, confidence, and flags
    pub fn classify(speedup: &Speedup) -> Self {
        let mean_speedup = speedup.mean_speedup;
        let ci = &speedup.confidence_interval_95;

        // Determine performance tier based on mean speedup
        let tier = Self::classify_tier(mean_speedup);

        // Determine confidence level based on CI spread
        let confidence = Self::classify_confidence(tier, ci);

        // Collect flags for special conditions
        let mut flags = Vec::new();

        // Flag if CI is very wide (high variance)
        let ci_width = ci.upper_bound - ci.lower_bound;
        if ci_width > mean_speedup * 0.5 {
            flags.push("HighVariance".to_string());
        }

        // Flag if CI spans multiple tiers
        let lower_tier = Self::classify_tier(ci.lower_bound);
        let upper_tier = Self::classify_tier(ci.upper_bound);
        if lower_tier != upper_tier {
            flags.push("BorderlineResult".to_string());
        }

        // Flag if suspicious speedup
        if tier == PerformanceTier::Suspicious {
            flags.push("RequiresManualValidation".to_string());
        }

        // Flag if speedup is negative (regression)
        if mean_speedup < 1.0 {
            flags.push("PerformanceRegression".to_string());
        }

        Self {
            tier,
            confidence,
            flags,
        }
    }

    /// Classify performance tier based on speedup
    fn classify_tier(speedup: f64) -> PerformanceTier {
        if speedup >= 10.0 {
            PerformanceTier::Suspicious
        } else if speedup >= 2.5 {
            PerformanceTier::Breakthrough
        } else if speedup >= 1.5 {
            PerformanceTier::Exceptional
        } else {
            PerformanceTier::Typical
        }
    }

    /// Classify confidence level based on CI spread across tiers
    fn classify_confidence(_tier: PerformanceTier, ci: &SpeedupConfidenceInterval) -> ConfidenceLevel {
        let lower_tier = Self::classify_tier(ci.lower_bound);
        let upper_tier = Self::classify_tier(ci.upper_bound);

        // Check how many tier boundaries the CI spans
        let tier_count = match (lower_tier, upper_tier) {
            (a, b) if a == b => 1, // Same tier
            _ => {
                // Count tiers between lower and upper
                let tiers = [
                    PerformanceTier::Typical,
                    PerformanceTier::Exceptional,
                    PerformanceTier::Breakthrough,
                    PerformanceTier::Suspicious,
                ];

                let lower_idx = tiers.iter().position(|&t| t == lower_tier).unwrap_or(0);
                let upper_idx = tiers.iter().position(|&t| t == upper_tier).unwrap_or(3);

                upper_idx.saturating_sub(lower_idx) + 1
            }
        };

        // Classify confidence based on tier span
        match tier_count {
            1 => ConfidenceLevel::High,
            2 => ConfidenceLevel::Medium,
            _ => ConfidenceLevel::Low,
        }
    }

    /// Get recommendation action based on classification
    pub fn recommendation_action(&self) -> RecommendationAction {
        match (self.tier, self.confidence) {
            // Suspicious always requires validation
            (PerformanceTier::Suspicious, _) => RecommendationAction::Validate,

            // Breakthrough with high confidence: ship it!
            (PerformanceTier::Breakthrough, ConfidenceLevel::High) => RecommendationAction::Ship,

            // Breakthrough with medium/low confidence: investigate
            (PerformanceTier::Breakthrough, _) => RecommendationAction::Investigate,

            // Exceptional with high confidence: ship it!
            (PerformanceTier::Exceptional, ConfidenceLevel::High) => RecommendationAction::Ship,

            // Exceptional with medium confidence: ship with caution
            (PerformanceTier::Exceptional, ConfidenceLevel::Medium) => RecommendationAction::Ship,

            // Exceptional with low confidence: iterate
            (PerformanceTier::Exceptional, ConfidenceLevel::Low) => RecommendationAction::Iterate,

            // Typical: optimize more
            (PerformanceTier::Typical, _) => RecommendationAction::Optimize,
        }
    }

    /// Get reasoning for recommendation
    pub fn reasoning(&self) -> String {
        let tier_str = match self.tier {
            PerformanceTier::Typical => "TYPICAL (1.1-1.5×)",
            PerformanceTier::Exceptional => "EXCEPTIONAL (1.5-2.5×)",
            PerformanceTier::Breakthrough => "BREAKTHROUGH (2.5-10×)",
            PerformanceTier::Suspicious => "SUSPICIOUS (>10×)",
        };

        let confidence_str = match self.confidence {
            ConfidenceLevel::High => "HIGH confidence",
            ConfidenceLevel::Medium => "MEDIUM confidence",
            ConfidenceLevel::Low => "LOW confidence",
        };

        let mut reasoning = format!("{} speedup with {} (95% CI)", tier_str, confidence_str);

        if !self.flags.is_empty() {
            reasoning.push_str(&format!(". Flags: {}", self.flags.join(", ")));
        }

        reasoning
    }

    /// Get next steps recommendation
    pub fn next_steps(&self) -> String {
        match self.recommendation_action() {
            RecommendationAction::Ship => {
                "Ready for production deployment. Monitor performance metrics in production to validate results.".to_string()
            }
            RecommendationAction::Optimize => {
                "Performance gains below target. Consider additional optimizations: SIMD, fixed-point arithmetic, batch processing, or tier stacking.".to_string()
            }
            RecommendationAction::Investigate => {
                "Unexpected variance or borderline result. Run additional benchmarks with tighter constraints (pin to core, disable turbo, etc.). Validate with production workloads.".to_string()
            }
            RecommendationAction::Validate => {
                "Suspicious speedup (>10×) requires manual validation. Verify: 1) Baseline is fair (not strawman), 2) Measurement methodology is correct, 3) No bugs in optimized code. Run independent validation.".to_string()
            }
            RecommendationAction::Iterate => {
                "Borderline or high-variance result. Iterate on design to reduce variance or improve performance. Consider algorithmic improvements.".to_string()
            }
        }
    }
}

/// Recommendation action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecommendationAction {
    /// Performance acceptable, deploy to production
    Ship,
    /// Performance below target, optimize more
    Optimize,
    /// Unexpected results, investigate further
    Investigate,
    /// Suspicious speedup, manual validation needed
    Validate,
    /// Borderline result, iterate on design
    Iterate,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::Speedup;

    fn make_speedup(mean: f64, ci_width: f64) -> Speedup {
        Speedup {
            mean_speedup: mean,
            median_speedup: mean,
            p95_speedup: mean,
            confidence_interval_95: SpeedupConfidenceInterval {
                lower_bound: mean - ci_width / 2.0,
                upper_bound: mean + ci_width / 2.0,
            },
        }
    }

    #[test]
    fn test_classify_typical() {
        let speedup = make_speedup(1.3, 0.1);
        let classification = Classification::classify(&speedup);

        assert_eq!(classification.tier, PerformanceTier::Typical);
        assert_eq!(classification.confidence, ConfidenceLevel::High);
    }

    #[test]
    fn test_classify_exceptional() {
        let speedup = make_speedup(2.0, 0.2);
        let classification = Classification::classify(&speedup);

        assert_eq!(classification.tier, PerformanceTier::Exceptional);
        assert_eq!(classification.confidence, ConfidenceLevel::High);
        assert_eq!(classification.recommendation_action(), RecommendationAction::Ship);
    }

    #[test]
    fn test_classify_breakthrough() {
        let speedup = make_speedup(4.0, 0.5);
        let classification = Classification::classify(&speedup);

        assert_eq!(classification.tier, PerformanceTier::Breakthrough);
        assert_eq!(classification.confidence, ConfidenceLevel::High);
        assert_eq!(classification.recommendation_action(), RecommendationAction::Ship);
    }

    #[test]
    fn test_classify_suspicious() {
        let speedup = make_speedup(15.0, 2.0);
        let classification = Classification::classify(&speedup);

        assert_eq!(classification.tier, PerformanceTier::Suspicious);
        assert_eq!(classification.recommendation_action(), RecommendationAction::Validate);
        assert!(classification.flags.contains(&"RequiresManualValidation".to_string()));
    }

    #[test]
    fn test_classify_borderline() {
        // CI spans Typical (1.1-1.5) and Exceptional (1.5-2.5)
        let speedup = make_speedup(1.5, 0.5); // CI: 1.25 - 1.75
        let classification = Classification::classify(&speedup);

        assert_eq!(classification.confidence, ConfidenceLevel::Medium);
        assert!(classification.flags.contains(&"BorderlineResult".to_string()));
    }

    #[test]
    fn test_classify_high_variance() {
        let speedup = make_speedup(2.0, 1.5); // Wide CI
        let classification = Classification::classify(&speedup);

        assert!(classification.flags.contains(&"HighVariance".to_string()));
    }

    #[test]
    fn test_classify_regression() {
        let speedup = make_speedup(0.8, 0.1); // Slower than baseline
        let classification = Classification::classify(&speedup);

        assert!(classification.flags.contains(&"PerformanceRegression".to_string()));
    }
}
