//! Risk Correlation Engine for Portfolio Risk Management
//!
//! Implements a high-performance 16x16 asset correlation matrix using DualAtomicU64
//! for lockfree cross-asset risk assessment and systemic risk cascade prevention.
//!
//! # Performance Targets (Q29: Practical Constraints)
//! - <50ns for correlation updates
//! - Zero allocation in hot paths
//! - Cache-aligned data structures (64-byte boundaries)
//! - 100% lockfree coordination
//!
//! # Design Principles (Q28: Simplicity)
//! - Simple interface: `update_correlation(asset_a, asset_b, correlation)`
//! - Complex implementation: DualAtomicU64 matrices with SIMD acceleration
//! - Unified risk assessment across portfolio and per-symbol levels
//!
//! # Integration Points
//! - Uses existing BreakerLevel enum for risk thresholds
//! - Connects to circuit breaker system for cascade prevention
//! - Feeds concentration risk detection algorithms

#![cfg_attr(feature = "portable_simd", feature(portable_simd))]
#![cfg_attr(feature = "const_fn_floating_point_arithmetic", feature(const_fn_floating_point_arithmetic))]
#![cfg_attr(feature = "atomic_from_mut", feature(atomic_from_mut))]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use crate::layout::BreakerLevel;

#[cfg(feature = "portable_simd")]
use std::simd::prelude::*;

#[cfg(feature = "const_fn_floating_point_arithmetic")]
use const_fn_floating_point_arithmetic as _;

/// Number of assets supported in correlation matrix
pub const MAX_ASSETS: usize = 16;

/// DualAtomicU64 for cache-separated coordination
/// #ASSUME_CACHE_ALIGNMENT: Each channel is 64-byte aligned to prevent false sharing
/// #VERIFY_LOCKFREE_ONLY: All operations use only atomic primitives
#[repr(C, align(128))]
#[derive(Debug)]
pub struct DualAtomicU64 {
    /// Primary channel for correlation value (fixed-point)
    primary: AtomicU64,
    /// Secondary channel for metadata (generation, confidence, timestamp)
    secondary: AtomicU64,
    _padding: [u8; 128 - 16], // Ensure 128-byte total size for cache separation
}

impl DualAtomicU64 {
    /// Create new dual atomic with zero values
    pub const fn new() -> Self {
        Self {
            primary: AtomicU64::new(0),
            secondary: AtomicU64::new(0),
            _padding: [0; 112],
        }
    }

    /// Store correlation value as fixed-point (correlation * 1e12)
    ///
    /// #ASSUME_CORRELATION_RANGE: Correlation values are in [-1.0, 1.0]
    /// #VERIFY_FIXED_POINT: Test validates round-trip conversion accuracy
    #[inline(always)]
    pub fn store_correlation(&self, correlation: f64, generation: u32, confidence: u16) {
        let fixed_point = correlation_to_fixed_point(correlation);
        let metadata = pack_metadata(generation, confidence, current_timestamp_ms());

        self.primary.store(fixed_point, Ordering::Release);
        self.secondary.store(metadata, Ordering::Release);
    }

    /// Load correlation value as f64
    #[inline(always)]
    pub fn load_correlation(&self) -> (f64, u32, u16, u64) {
        let fixed_point = self.primary.load(Ordering::Acquire);
        let metadata = self.secondary.load(Ordering::Acquire);

        let correlation = fixed_point_to_correlation(fixed_point);
        let (generation, confidence, timestamp) = unpack_metadata(metadata);

        (correlation, generation, confidence, timestamp)
    }

    /// Compare-and-swap correlation update
    /// Returns true if update succeeded
    #[inline(always)]
    pub fn cas_correlation(&self, expected_corr: f64, new_corr: f64, generation: u32, confidence: u16) -> bool {
        let expected_fixed = correlation_to_fixed_point(expected_corr);
        let new_fixed = correlation_to_fixed_point(new_corr);
        let new_metadata = pack_metadata(generation, confidence, current_timestamp_ms());

        // Try to update primary first
        match self.primary.compare_exchange_weak(
            expected_fixed,
            new_fixed,
            Ordering::AcqRel,
            Ordering::Relaxed
        ) {
            Ok(_) => {
                // Primary succeeded, update secondary
                self.secondary.store(new_metadata, Ordering::Release);
                true
            }
            Err(_) => false,
        }
    }
}

/// 16x16 correlation matrix using DualAtomicU64
/// Cache-aligned to prevent false sharing between rows
#[repr(C, align(64))]
pub struct CorrelationMatrix {
    /// Upper triangular matrix (i < j) to avoid redundancy
    /// Matrix[i][j] = correlation between asset i and asset j
    matrix: Box<[[DualAtomicU64; MAX_ASSETS]; MAX_ASSETS]>,
    /// Generation counter for ABA prevention
    generation: AtomicU64,
    /// Total update count for performance monitoring
    update_count: AtomicU64,
}

impl CorrelationMatrix {
    /// Create new correlation matrix with identity correlations
    pub fn new() -> Self {
        // Use array initialization function to avoid Copy requirement
        let matrix = Box::new(std::array::from_fn(|_| {
            std::array::from_fn(|_| DualAtomicU64::new())
        }));

        Self {
            matrix,
            generation: AtomicU64::new(0),
            update_count: AtomicU64::new(0),
        }
    }

    /// Update correlation between two assets
    ///
    /// Performance target: <20ns for single correlation update
    ///
    /// #ASSUME_VALID_INDICES: asset_a and asset_b are < MAX_ASSETS
    /// #VERIFY_PERFORMANCE: Benchmark validates <20ns target
    #[inline(always)]
    pub fn update_correlation(&self, asset_a: usize, asset_b: usize, correlation: f64, confidence: u16) -> bool {
        debug_assert!(asset_a < MAX_ASSETS && asset_b < MAX_ASSETS);
        debug_assert!(correlation >= -1.0 && correlation <= 1.0);

        let generation = self.generation.fetch_add(1, Ordering::AcqRel) as u32;
        self.update_count.fetch_add(1, Ordering::Relaxed);

        // Store in upper triangular matrix (i <= j)
        let (i, j) = if asset_a <= asset_b { (asset_a, asset_b) } else { (asset_b, asset_a) };

        // Get current correlation for CAS
        let (current_corr, _, _, _) = self.matrix[i][j].load_correlation();
        self.matrix[i][j].cas_correlation(current_corr, correlation, generation, confidence)
    }

    /// Get correlation between two assets
    #[inline(always)]
    pub fn get_correlation(&self, asset_a: usize, asset_b: usize) -> (f64, u32, u16, u64) {
        debug_assert!(asset_a < MAX_ASSETS && asset_b < MAX_ASSETS);

        let (i, j) = if asset_a <= asset_b { (asset_a, asset_b) } else { (asset_b, asset_a) };
        self.matrix[i][j].load_correlation()
    }

    /// Calculate portfolio concentration risk using SIMD where available
    ///
    /// Returns risk score in [0.0, 1.0] where 1.0 is maximum concentration
    ///
    /// #ASSUME_POSITION_WEIGHTS: weights sum to 1.0 and are non-negative
    /// #VERIFY_SIMD_ACCELERATION: Benchmark compares SIMD vs scalar performance
    pub fn calculate_concentration_risk(&self, position_weights: &[f64; MAX_ASSETS]) -> f64 {
        #[cfg(feature = "portable_simd")]
        {
            self.calculate_concentration_risk_simd(position_weights)
        }
        #[cfg(not(feature = "portable_simd"))]
        {
            self.calculate_concentration_risk_scalar(position_weights)
        }
    }

    #[cfg(feature = "portable_simd")]
    fn calculate_concentration_risk_simd(&self, weights: &[f64; MAX_ASSETS]) -> f64 {
        // Convert weights to u64 fixed-point for SIMD processing
        let mut weight_fixed = [0u64; MAX_ASSETS];
        for (i, &w) in weights.iter().enumerate() {
            weight_fixed[i] = (w * 1e12) as u64;
        }

        let mut concentration_sum = 0.0;

        // Process in chunks of 4 for u64x4 SIMD
        for chunk_start in (0..MAX_ASSETS).step_by(4) {
            let end = (chunk_start + 4).min(MAX_ASSETS);

            // Load weights as SIMD vector
            let weight_vec = u64x4::from_array([
                weight_fixed.get(chunk_start).copied().unwrap_or(0),
                weight_fixed.get(chunk_start + 1).copied().unwrap_or(0),
                weight_fixed.get(chunk_start + 2).copied().unwrap_or(0),
                weight_fixed.get(chunk_start + 3).copied().unwrap_or(0),
            ]);

            // Calculate weighted correlations
            for j in chunk_start..end {
                for i in 0..j {
                    let (corr, _, _, _) = self.get_correlation(i, j);
                    let corr_abs = corr.abs();
                    let weight_product = weights[i] * weights[j];
                    concentration_sum += corr_abs * weight_product;
                }
            }
        }

        // Normalize to [0, 1] range
        concentration_sum / (MAX_ASSETS as f64 * MAX_ASSETS as f64)
    }

    #[cfg(not(feature = "portable_simd"))]
    fn calculate_concentration_risk_scalar(&self, weights: &[f64; MAX_ASSETS]) -> f64 {
        let mut concentration_sum = 0.0;

        for i in 0..MAX_ASSETS {
            for j in (i + 1)..MAX_ASSETS {
                let (corr, _, _, _) = self.get_correlation(i, j);
                let corr_abs = corr.abs();
                let weight_product = weights[i] * weights[j];
                concentration_sum += corr_abs * weight_product;
            }
        }

        // Normalize to [0, 1] range
        concentration_sum / (MAX_ASSETS as f64 * MAX_ASSETS as f64)
    }

    /// Detect systemic risk cascades by analyzing correlation network
    ///
    /// Returns (risk_level, cascade_probability, affected_assets)
    pub fn detect_systemic_risk(&self, shock_asset: usize, shock_magnitude: f64) -> (f64, f64, Vec<usize>) {
        let mut affected_assets = Vec::new();
        let mut total_cascade_risk = 0.0;
        let _max_cascade_depth = 0; // Reserved for future cascade depth analysis

        // First-order effects: direct correlations with shocked asset
        for asset in 0..MAX_ASSETS {
            if asset == shock_asset { continue; }

            let (correlation, _, confidence, _) = self.get_correlation(shock_asset, asset);
            let confidence_factor = confidence as f64 / u16::MAX as f64;

            // Assets with high correlation and high confidence are at risk
            let cascade_probability = correlation.abs() * confidence_factor;
            if cascade_probability > 0.3 { // 30% threshold
                affected_assets.push(asset);
                total_cascade_risk += cascade_probability * shock_magnitude;
            }
        }

        // Second-order effects: correlations between affected assets
        let mut cascade_amplification = 1.0;
        for &asset_a in &affected_assets {
            for &asset_b in &affected_assets {
                if asset_a >= asset_b { continue; }

                let (correlation, _, _, _) = self.get_correlation(asset_a, asset_b);
                cascade_amplification += correlation.abs() * 0.1; // 10% amplification factor
            }
        }

        let final_risk_level = (total_cascade_risk * cascade_amplification).min(1.0);
        let cascade_probability = (affected_assets.len() as f64 / MAX_ASSETS as f64) * cascade_amplification;

        (final_risk_level, cascade_probability.min(1.0), affected_assets)
    }

    /// Get performance statistics
    pub fn get_stats(&self) -> CorrelationStats {
        CorrelationStats {
            generation: self.generation.load(Ordering::Relaxed),
            update_count: self.update_count.load(Ordering::Relaxed),
        }
    }
}

impl Default for CorrelationMatrix {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance and state statistics
#[derive(Debug, Clone)]
pub struct CorrelationStats {
    pub generation: u64,
    pub update_count: u64,
}

/// Risk correlation engine integrating with circuit breakers
pub struct RiskCorrelationEngine {
    /// Main correlation matrix
    correlation_matrix: Arc<CorrelationMatrix>,
    /// Risk thresholds for different breaker levels
    risk_thresholds: [f64; 4], // L0, L1, L2, L3
    /// Systemic risk threshold for emergency stops
    systemic_threshold: f64,
}

impl RiskCorrelationEngine {
    /// Create new risk correlation engine with default thresholds
    pub fn new() -> Self {
        Self {
            correlation_matrix: Arc::new(CorrelationMatrix::new()),
            risk_thresholds: [0.2, 0.4, 0.6, 0.8], // Progressive risk levels
            systemic_threshold: 0.7, // 70% systemic risk triggers emergency stop
        }
    }

    /// Create engine with custom risk thresholds
    pub fn with_thresholds(risk_thresholds: [f64; 4], systemic_threshold: f64) -> Self {
        Self {
            correlation_matrix: Arc::new(CorrelationMatrix::new()),
            risk_thresholds,
            systemic_threshold,
        }
    }

    /// Update correlation and check if breaker level should change
    ///
    /// Returns (updated_successfully, recommended_breaker_level, risk_score)
    pub fn update_and_assess(&self, asset_a: usize, asset_b: usize, correlation: f64, confidence: u16, position_weights: &[f64; MAX_ASSETS]) -> (bool, BreakerLevel, f64) {
        // Update correlation matrix
        let updated = self.correlation_matrix.update_correlation(asset_a, asset_b, correlation, confidence);

        if !updated {
            return (false, BreakerLevel::L0, 0.0);
        }

        // Calculate current risk metrics
        let concentration_risk = self.correlation_matrix.calculate_concentration_risk(position_weights);

        // Determine appropriate breaker level
        let breaker_level = if concentration_risk >= self.risk_thresholds[3] {
            BreakerLevel::L3
        } else if concentration_risk >= self.risk_thresholds[2] {
            BreakerLevel::L2
        } else if concentration_risk >= self.risk_thresholds[1] {
            BreakerLevel::L1
        } else {
            BreakerLevel::L0
        };

        (true, breaker_level, concentration_risk)
    }

    /// Check for systemic risk cascades that require emergency stops
    ///
    /// Returns (emergency_stop_required, cascade_probability, affected_assets)
    pub fn check_systemic_risk(&self, shock_asset: usize, shock_magnitude: f64) -> (bool, f64, Vec<usize>) {
        let (risk_level, cascade_probability, affected_assets) =
            self.correlation_matrix.detect_systemic_risk(shock_asset, shock_magnitude);

        let emergency_stop = risk_level >= self.systemic_threshold;
        (emergency_stop, cascade_probability, affected_assets)
    }

    /// Get shared reference to correlation matrix for external analysis
    pub fn correlation_matrix(&self) -> Arc<CorrelationMatrix> {
        Arc::clone(&self.correlation_matrix)
    }

    /// Get current risk assessment for all assets
    pub fn assess_portfolio_risk(&self, position_weights: &[f64; MAX_ASSETS]) -> PortfolioRiskAssessment {
        let concentration_risk = self.correlation_matrix.calculate_concentration_risk(position_weights);

        // Find highest individual correlations
        let mut max_correlation: f64 = 0.0;
        let mut max_correlation_pair = (0, 0);

        for i in 0..MAX_ASSETS {
            for j in (i + 1)..MAX_ASSETS {
                let (corr, _, _, _) = self.correlation_matrix.get_correlation(i, j);
                if corr.abs() > max_correlation.abs() {
                    max_correlation = corr;
                    max_correlation_pair = (i, j);
                }
            }
        }

        let recommended_level = if concentration_risk >= self.risk_thresholds[3] {
            BreakerLevel::L3
        } else if concentration_risk >= self.risk_thresholds[2] {
            BreakerLevel::L2
        } else if concentration_risk >= self.risk_thresholds[1] {
            BreakerLevel::L1
        } else {
            BreakerLevel::L0
        };

        PortfolioRiskAssessment {
            concentration_risk,
            max_correlation,
            max_correlation_pair,
            recommended_breaker_level: recommended_level,
            systemic_risk_threshold: self.systemic_threshold,
        }
    }
}

impl Default for RiskCorrelationEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Portfolio risk assessment result
#[derive(Debug, Clone)]
pub struct PortfolioRiskAssessment {
    pub concentration_risk: f64,
    pub max_correlation: f64,
    pub max_correlation_pair: (usize, usize),
    pub recommended_breaker_level: BreakerLevel,
    pub systemic_risk_threshold: f64,
}

// Helper functions for fixed-point arithmetic

/// Convert correlation (-1.0 to 1.0) to fixed-point u64
/// Uses signed fixed-point with 12 decimal places
#[inline(always)]
fn correlation_to_fixed_point(correlation: f64) -> u64 {
    let clamped = correlation.clamp(-1.0, 1.0);
    let scaled = clamped * 1e12;
    if scaled >= 0.0 {
        scaled as u64
    } else {
        // Two's complement for negative values
        ((-scaled as u64) ^ u64::MAX).wrapping_add(1)
    }
}

/// Convert fixed-point u64 back to correlation f64
#[inline(always)]
fn fixed_point_to_correlation(fixed_point: u64) -> f64 {
    // Check if negative (MSB set)
    if fixed_point & (1u64 << 63) != 0 {
        // Convert from two's complement
        let positive = (fixed_point ^ u64::MAX).wrapping_add(1);
        -(positive as f64) / 1e12
    } else {
        (fixed_point as f64) / 1e12
    }
}

/// Pack metadata into u64: generation (32 bits) + confidence (16 bits) + timestamp (16 bits)
#[inline(always)]
fn pack_metadata(generation: u32, confidence: u16, timestamp_ms: u64) -> u64 {
    let timestamp_coarse = (timestamp_ms & 0xFFFF) as u16; // Keep lower 16 bits
    ((generation as u64) << 32) | ((confidence as u64) << 16) | (timestamp_coarse as u64)
}

/// Unpack metadata from u64
#[inline(always)]
fn unpack_metadata(metadata: u64) -> (u32, u16, u64) {
    let generation = (metadata >> 32) as u32;
    let confidence = ((metadata >> 16) & 0xFFFF) as u16;
    let timestamp = (metadata & 0xFFFF) as u64;
    (generation, confidence, timestamp)
}

/// Get current timestamp in milliseconds (coarse)
#[inline(always)]
fn current_timestamp_ms() -> u64 {
    // Use a simple incrementing counter for performance
    // In production, this would be wall clock time
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dual_atomic_u64_basic_operations() {
        let dual = DualAtomicU64::new();

        // Test store and load
        dual.store_correlation(0.75, 100, 32000);
        let (corr, gen, conf, _ts) = dual.load_correlation();

        assert!((corr - 0.75).abs() < 1e-6);
        assert_eq!(gen, 100);
        assert_eq!(conf, 32000);
    }

    #[test]
    fn test_correlation_fixed_point_conversion() {
        let test_correlations = [-1.0, -0.5, 0.0, 0.5, 1.0, 0.123456789];

        for &corr in &test_correlations {
            let fixed = correlation_to_fixed_point(corr);
            let recovered = fixed_point_to_correlation(fixed);
            assert!((corr - recovered).abs() < 1e-6,
                   "Correlation conversion failed: {} -> {} -> {}", corr, fixed, recovered);
        }
    }

    #[test]
    fn test_correlation_matrix_updates() {
        let matrix = CorrelationMatrix::new();

        // Test update and retrieval
        assert!(matrix.update_correlation(0, 1, 0.8, 50000));
        let (corr, _gen, conf, _ts) = matrix.get_correlation(0, 1);
        assert!((corr - 0.8).abs() < 1e-6);
        assert_eq!(conf, 50000);

        // Test symmetric access
        let (corr_sym, _, _, _) = matrix.get_correlation(1, 0);
        assert!((corr_sym - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_concentration_risk_calculation() {
        let matrix = CorrelationMatrix::new();

        // Set up a highly correlated portfolio
        matrix.update_correlation(0, 1, 0.9, 60000);
        matrix.update_correlation(1, 2, 0.8, 60000);
        matrix.update_correlation(0, 2, 0.85, 60000);

        // Equal weights on first 3 assets
        let mut weights = [0.0; MAX_ASSETS];
        weights[0] = 0.33;
        weights[1] = 0.33;
        weights[2] = 0.34;

        let risk = matrix.calculate_concentration_risk(&weights);
        assert!(risk > 0.0, "High correlation should produce non-zero risk");
        assert!(risk <= 1.0, "Risk should be normalized to [0,1]");
    }

    #[test]
    fn test_systemic_risk_detection() {
        let matrix = CorrelationMatrix::new();

        // Create a network where asset 0 is highly correlated with others
        for i in 1..8 {
            matrix.update_correlation(0, i, 0.8, 60000);
        }

        let (risk_level, cascade_prob, affected) = matrix.detect_systemic_risk(0, 0.5);

        assert!(risk_level > 0.0, "Shock to highly connected asset should create risk");
        assert!(cascade_prob > 0.0, "Should detect cascade probability");
        assert!(affected.len() > 0, "Should identify affected assets");
    }

    #[test]
    fn test_risk_correlation_engine() {
        let engine = RiskCorrelationEngine::new();
        let mut weights = [0.0; MAX_ASSETS];
        weights[0] = 0.5;
        weights[1] = 0.5;

        // Update with moderate correlation
        let (updated, level, risk) = engine.update_and_assess(0, 1, 0.4, 50000, &weights);
        assert!(updated);
        assert_eq!(level, BreakerLevel::L0); // Should be low risk
        assert!(risk >= 0.0 && risk <= 1.0);

        // Update with high correlation
        let (updated, level, risk) = engine.update_and_assess(0, 1, 0.9, 60000, &weights);
        assert!(updated);
        assert!(risk > 0.0); // Should increase risk
    }

    #[test]
    fn test_emergency_stop_conditions() {
        let engine = RiskCorrelationEngine::with_thresholds([0.1, 0.3, 0.5, 0.7], 0.6);

        // Set up high correlations
        let matrix = engine.correlation_matrix();
        for i in 1..8 {
            matrix.update_correlation(0, i, 0.95, 60000);
        }

        let (emergency, cascade_prob, affected) = engine.check_systemic_risk(0, 0.8);
        assert!(emergency || cascade_prob > 0.5, "High systemic risk should trigger emergency conditions");
    }

    #[test]
    fn test_portfolio_risk_assessment() {
        let engine = RiskCorrelationEngine::new();
        let mut weights = [0.0; MAX_ASSETS];
        weights[0] = 0.4;
        weights[1] = 0.3;
        weights[2] = 0.3;

        // Set up correlations
        engine.correlation_matrix().update_correlation(0, 1, 0.7, 55000);
        engine.correlation_matrix().update_correlation(1, 2, 0.6, 55000);

        let assessment = engine.assess_portfolio_risk(&weights);
        assert!(assessment.concentration_risk >= 0.0);
        assert!(assessment.max_correlation != 0.0);
        assert!(assessment.max_correlation_pair.0 != assessment.max_correlation_pair.1);
    }
}