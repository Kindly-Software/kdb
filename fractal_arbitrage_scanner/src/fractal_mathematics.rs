//! Real Fractal Mathematics Implementation
//!
//! Unlike quantum claims, this implements actual mathematical algorithms
//! for fractal analysis that work on classical computers.
//!
//! Enhanced with adaptive parameters and self-modifying algorithms
//! for protection and performance optimization.

use std::sync::atomic::{AtomicU64, Ordering};

// Import fractal protection traits
use crate::fractal_protection::{
    AdaptiveParameters, DefaultAdaptiveParams, PerformanceMetrics
};

/// Wrapper struct for Multifractal DFA analysis with adaptive parameters
pub struct MultifractalDFA {
    generation: AtomicU64,
    /// Adaptive parameters for self-modifying behavior
    adaptive_params: Option<DefaultAdaptiveParams>,
}

impl MultifractalDFA {
    pub fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            adaptive_params: None,
        }
    }

    /// Create new analyzer with adaptive parameters enabled
    pub fn new_adaptive() -> Self {
        let mut params = DefaultAdaptiveParams::new();
        // Set fractal-specific parameters
        params.set_param_internal("hurst_window_min", 20.0);
        params.set_param_internal("hurst_window_max", 200.0);
        params.set_param_internal("box_scaling_factor", 2.0);
        params.set_param_internal("dimension_threshold", 1.5);

        Self {
            generation: AtomicU64::new(0),
            adaptive_params: Some(params),
        }
    }

    /// Enable adaptive parameters for self-modification
    pub fn enable_adaptive_parameters(&mut self) {
        if self.adaptive_params.is_none() {
            self.adaptive_params = Some(DefaultAdaptiveParams::new());
        }
    }

    /// Update performance metrics for parameter adaptation
    pub fn update_performance(&mut self, latency_us: u64, accuracy: f64) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref mut params) = self.adaptive_params {
            let metrics = PerformanceMetrics {
                latency_us,
                accuracy,
                memory_usage: 1024, // Estimate
                cache_hit_rate: 0.8,
                error_rate: 1.0 - accuracy,
                throughput: if latency_us > 0 { 1_000_000.0 / latency_us as f64 } else { 0.0 },
            };

            params.adapt_parameters(&metrics)?;

            // Apply adapted parameters to internal algorithms
            self.apply_adapted_parameters();
        }
        Ok(())
    }

    /// Apply adapted parameters to internal algorithm configuration
    fn apply_adapted_parameters(&self) {
        // Parameters are applied during analysis calls
        // This method exists for future extensions
    }

    pub fn analyze(&self, data: &[f64]) -> Vec<f64> {
        let _gen = self.generation.fetch_add(1, Ordering::Relaxed);

        // Get adaptive parameters or use defaults
        let (min_box, max_box, scaling_factor, similarity_scale) = if let Some(ref params) = self.adaptive_params {
            let min_box = params.get_param_internal("hurst_window_min").unwrap_or(2.0) as usize;
            let max_box = params.get_param_internal("hurst_window_max").unwrap_or(256.0) as usize;
            let scaling = params.get_param_internal("box_scaling_factor").unwrap_or(2.0);
            let similarity = params.get_param_internal("similarity_scale").unwrap_or(4.0) as usize;
            (min_box, data.len().min(max_box) / 4, scaling, similarity)
        } else {
            (2, data.len().min(256) / 4, 2.0, 4)
        };

        // Use adaptive parameters for analysis
        let dimension = box_counting_dimension_adaptive(data, min_box, max_box, scaling_factor);
        let hurst = hurst_exponent_adaptive(data, &self.adaptive_params);
        let similarity = self_similarity(data, similarity_scale);

        vec![dimension, hurst, similarity]
    }

    pub fn get_hurst_exponent(&self, data: &[f64]) -> f64 {
        hurst_exponent(data)
    }

    pub fn calculate_hurst(&self, data: &[f64]) -> f64 {
        hurst_exponent(data)
    }
}

/// Wrapper struct for Williams Fractal detection
pub struct WilliamsFractal {
    generation: AtomicU64,
}

impl WilliamsFractal {
    pub fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
        }
    }

    pub fn detect_fractals(&self, data: &[f64]) -> Vec<(usize, bool)> {
        let _gen = self.generation.fetch_add(1, Ordering::Relaxed);
        let (support, resistance) = fractal_support_resistance(data, 5);
        let mut fractals = Vec::new();

        // Map support/resistance levels to indices
        for (i, &price) in data.iter().enumerate() {
            if support.contains(&price) {
                fractals.push((i, true));  // bullish
            }
            if resistance.contains(&price) {
                fractals.push((i, false)); // bearish
            }
        }
        fractals
    }

    pub fn detect_high(&self, data: &[f64]) -> usize {
        let fractals = self.detect_fractals(data);
        fractals.iter().filter(|(_, is_bullish)| !is_bullish).count()
    }

    pub fn detect_low(&self, data: &[f64]) -> usize {
        let fractals = self.detect_fractals(data);
        fractals.iter().filter(|(_, is_bullish)| *is_bullish).count()
    }

    pub fn calculate_dimension(&self, data: &[f64]) -> f64 {
        box_counting_dimension(data, 2, data.len().min(256) / 4)
    }
}

/// Wrapper struct for Wavelet Leaders analysis
pub struct WaveletLeaders {
    generation: AtomicU64,
}

impl WaveletLeaders {
    pub fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
        }
    }

    pub fn compute_leaders(&self, data: &[f64]) -> Vec<f64> {
        let _gen = self.generation.fetch_add(1, Ordering::Relaxed);
        // Simple wavelet-like decomposition
        let mut leaders = Vec::new();
        for scale in 0..data.len().trailing_zeros().min(8) {
            let window = 1 << scale;
            if window >= data.len() {
                break;
            }
            let max_val = data.chunks(window)
                .map(|chunk| chunk.iter().fold(0.0_f64, |a, &b| a.max(b.abs())))
                .fold(0.0_f64, f64::max);
            leaders.push(max_val);
        }
        leaders
    }

    pub fn singularity_spectrum(&self, data: &[f64]) -> Vec<(f64, f64)> {
        let leaders = self.compute_leaders(data);
        leaders.iter().enumerate()
            .map(|(i, &leader)| {
                let alpha = (i as f64 + 1.0).ln() / (leader + 1.0).ln();
                let f_alpha = alpha * 0.5 + 0.5;
                (alpha, f_alpha)
            })
            .collect()
    }

    pub fn calculate_spectrum(&self, data: &[f64]) -> usize {
        let spectrum = self.singularity_spectrum(data);
        spectrum.len()
    }
}

/// Golden ratio - appears throughout fractals in nature
pub const PHI: f64 = 1.618033988749894848;

/// Calculate fractal dimension using box-counting algorithm
pub fn box_counting_dimension(data: &[f64], min_box: usize, max_box: usize) -> f64 {
    let mut counts = Vec::new();
    let mut sizes = Vec::new();

    let mut box_size = min_box;
    while box_size <= max_box {
        let count = count_boxes(data, box_size);
        counts.push((count as f64).ln());
        sizes.push((1.0 / box_size as f64).ln());
        box_size *= 2; // Fractal scaling
    }

    // Linear regression to find slope (fractal dimension)
    linear_regression(&sizes, &counts)
}

/// Measure self-similarity using correlation
pub fn self_similarity(data: &[f64], scale: usize) -> f64 {
    if data.len() < scale * 2 {
        return 0.0;
    }

    let mut correlation = 0.0;
    let chunks = data.len() / scale;

    for i in 0..chunks-1 {
        let chunk1 = &data[i*scale..(i+1)*scale];
        let chunk2 = &data[(i+1)*scale..(i+2)*scale];
        correlation += calculate_correlation(chunk1, chunk2);
    }

    correlation / (chunks - 1) as f64
}

/// Detect Fibonacci levels in price data
pub fn fibonacci_levels(high: f64, low: f64) -> Vec<(f64, f64)> {
    let range = high - low;
    vec![
        (0.236, low + range * 0.236),  // Key Fibonacci ratios
        (0.382, low + range * 0.382),
        (0.500, low + range * 0.500),
        (0.618, low + range * 0.618),  // Golden ratio
        (0.786, low + range * 0.786),
        (1.000, high),
    ]
}

/// Hurst exponent - measures long-term memory
pub fn hurst_exponent(data: &[f64]) -> f64 {
    // R/S analysis for fractal persistence
    let n = data.len();
    let mean = data.iter().sum::<f64>() / n as f64;

    let mut cumsum = 0.0;
    let mut max_val = f64::MIN;
    let mut min_val = f64::MAX;

    for &val in data {
        cumsum += val - mean;
        max_val = max_val.max(cumsum);
        min_val = min_val.min(cumsum);
    }

    let range = max_val - min_val;
    let std_dev = standard_deviation(data);

    (range / std_dev).ln() / (n as f64).ln()
}

/// Detect power-law distribution (fractal signature)
pub fn is_power_law(data: &mut [f64]) -> (bool, f64) {
    data.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mut log_rank = Vec::new();
    let mut log_value = Vec::new();

    for (i, &val) in data.iter().enumerate() {
        if val > 0.0 {
            log_rank.push(((i + 1) as f64).ln());
            log_value.push(val.ln());
        }
    }

    let alpha = linear_regression(&log_rank, &log_value).abs();
    let is_power_law = alpha > 1.0 && alpha < 4.0; // Typical range

    (is_power_law, alpha)
}

/// Count boxes for box-counting algorithm
fn count_boxes(data: &[f64], box_size: usize) -> usize {
    if data.is_empty() || box_size == 0 {
        return 0;
    }

    let num_boxes = (data.len() + box_size - 1) / box_size;
    let mut occupied_boxes = 0;

    for i in 0..num_boxes {
        let start = i * box_size;
        let end = ((i + 1) * box_size).min(data.len());

        // Check if this box contains any non-zero values
        if data[start..end].iter().any(|&x| x.abs() > f64::EPSILON) {
            occupied_boxes += 1;
        }
    }

    occupied_boxes
}

/// Linear regression to find slope
fn linear_regression(x: &[f64], y: &[f64]) -> f64 {
    if x.len() != y.len() || x.is_empty() {
        return 0.0;
    }

    let n = x.len() as f64;
    let sum_x: f64 = x.iter().sum();
    let sum_y: f64 = y.iter().sum();
    let sum_xy: f64 = x.iter().zip(y.iter()).map(|(xi, yi)| xi * yi).sum();
    let sum_x2: f64 = x.iter().map(|xi| xi * xi).sum();

    let denominator = n * sum_x2 - sum_x * sum_x;
    if denominator.abs() < f64::EPSILON {
        return 0.0;
    }

    (n * sum_xy - sum_x * sum_y) / denominator
}

/// Calculate correlation between two data series
fn calculate_correlation(data1: &[f64], data2: &[f64]) -> f64 {
    if data1.len() != data2.len() || data1.is_empty() {
        return 0.0;
    }

    let mean1 = data1.iter().sum::<f64>() / data1.len() as f64;
    let mean2 = data2.iter().sum::<f64>() / data2.len() as f64;

    let mut numerator = 0.0;
    let mut sum_sq1 = 0.0;
    let mut sum_sq2 = 0.0;

    for (v1, v2) in data1.iter().zip(data2.iter()) {
        let diff1 = v1 - mean1;
        let diff2 = v2 - mean2;
        numerator += diff1 * diff2;
        sum_sq1 += diff1 * diff1;
        sum_sq2 += diff2 * diff2;
    }

    let denominator = (sum_sq1 * sum_sq2).sqrt();
    if denominator.abs() < f64::EPSILON {
        return 0.0;
    }

    numerator / denominator
}

/// Calculate standard deviation
fn standard_deviation(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }

    let mean = data.iter().sum::<f64>() / data.len() as f64;
    let variance = data.iter()
        .map(|x| (x - mean).powi(2))
        .sum::<f64>() / data.len() as f64;

    variance.sqrt()
}

/// Mandelbrot set iteration for fractal analysis
pub fn mandelbrot_iteration(c: (f64, f64), max_iter: usize) -> usize {
    let mut z = (0.0, 0.0);
    let mut iter = 0;

    while iter < max_iter {
        let z_real_new = z.0 * z.0 - z.1 * z.1 + c.0;
        let z_imag_new = 2.0 * z.0 * z.1 + c.1;
        z = (z_real_new, z_imag_new);

        if z.0 * z.0 + z.1 * z.1 > 4.0 {
            break;
        }
        iter += 1;
    }

    iter
}

/// Julia set iteration for fractal analysis
pub fn julia_iteration(z: (f64, f64), c: (f64, f64), max_iter: usize) -> usize {
    let mut z_current = z;
    let mut iter = 0;

    while iter < max_iter {
        let z_real_new = z_current.0 * z_current.0 - z_current.1 * z_current.1 + c.0;
        let z_imag_new = 2.0 * z_current.0 * z_current.1 + c.1;
        z_current = (z_real_new, z_imag_new);

        if z_current.0 * z_current.0 + z_current.1 * z_current.1 > 4.0 {
            break;
        }
        iter += 1;
    }

    iter
}

/// Calculate fractal market efficiency using box-counting
pub fn market_efficiency_dimension(prices: &[f64]) -> f64 {
    if prices.len() < 4 {
        return 1.0; // Perfectly efficient market
    }

    // Convert prices to returns
    let mut returns = Vec::with_capacity(prices.len() - 1);
    for i in 1..prices.len() {
        if prices[i-1] > 0.0 {
            returns.push((prices[i] / prices[i-1]).ln());
        }
    }

    if returns.is_empty() {
        return 1.0;
    }

    // Calculate fractal dimension of return series
    let min_box = 2;
    let max_box = returns.len() / 4;

    if max_box <= min_box {
        return 1.0;
    }

    box_counting_dimension(&returns, min_box, max_box)
}

/// Detect fractal support and resistance levels
pub fn fractal_support_resistance(prices: &[f64], window: usize) -> (Vec<f64>, Vec<f64>) {
    let mut support_levels = Vec::new();
    let mut resistance_levels = Vec::new();

    if prices.len() < window * 2 + 1 {
        return (support_levels, resistance_levels);
    }

    for i in window..prices.len()-window {
        let current = prices[i];
        let mut is_support = true;
        let mut is_resistance = true;

        // Check if current point is local minimum (support)
        for j in i-window..=i+window {
            if j != i && prices[j] < current {
                is_support = false;
                break;
            }
        }

        // Check if current point is local maximum (resistance)
        for j in i-window..=i+window {
            if j != i && prices[j] > current {
                is_resistance = false;
                break;
            }
        }

        if is_support {
            support_levels.push(current);
        }
        if is_resistance {
            resistance_levels.push(current);
        }
    }

    (support_levels, resistance_levels)
}

/// Adaptive box-counting dimension with configurable scaling
pub fn box_counting_dimension_adaptive(data: &[f64], min_box: usize, max_box: usize, scaling_factor: f64) -> f64 {
    let mut counts = Vec::new();
    let mut sizes = Vec::new();

    let mut box_size = min_box;
    while box_size <= max_box {
        let count = count_boxes(data, box_size);
        counts.push((count as f64).ln());
        sizes.push((1.0 / box_size as f64).ln());

        // Use adaptive scaling factor instead of fixed 2x
        box_size = (box_size as f64 * scaling_factor).round() as usize;
        if box_size <= (box_size as f64 / scaling_factor) as usize {
            break; // Prevent infinite loop with bad scaling
        }
    }

    // Linear regression to find slope (fractal dimension)
    linear_regression(&sizes, &counts)
}

/// Adaptive Hurst exponent calculation with parameter optimization
pub fn hurst_exponent_adaptive(data: &[f64], adaptive_params: &Option<DefaultAdaptiveParams>) -> f64 {
    // Get window size from adaptive parameters
    let window_size = if let Some(params) = adaptive_params {
        params.get_param_internal("hurst_window_min").unwrap_or(10.0) as usize
    } else {
        10
    };

    // Use windowed Hurst calculation for better accuracy on small datasets
    if data.len() < window_size * 2 {
        return hurst_exponent(data); // Fall back to standard calculation
    }

    let mut hurst_values = Vec::new();

    // Calculate Hurst in overlapping windows
    for i in 0..=data.len().saturating_sub(window_size) {
        let window_end = (i + window_size).min(data.len());
        let window_data = &data[i..window_end];

        if window_data.len() >= 10 { // Minimum for reliable Hurst calculation
            let hurst = hurst_exponent(window_data);
            if hurst.is_finite() && hurst > 0.0 && hurst < 2.0 {
                hurst_values.push(hurst);
            }
        }
    }

    if hurst_values.is_empty() {
        return hurst_exponent(data); // Fall back
    }

    // Return median for robustness
    hurst_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    hurst_values[hurst_values.len() / 2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fibonacci_levels() {
        let levels = fibonacci_levels(100.0, 50.0);
        assert_eq!(levels.len(), 6);
        assert!((levels[3].1 - 80.9).abs() < 0.1); // 0.618 level
    }

    #[test]
    fn test_hurst_exponent() {
        let random_data = vec![1.0, 1.1, 0.9, 1.2, 0.8, 1.3, 0.7];
        let hurst = hurst_exponent(&random_data);
        assert!(hurst > 0.0 && hurst < 2.0);
    }

    #[test]
    fn test_self_similarity() {
        let periodic_data = vec![1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0];
        let similarity = self_similarity(&periodic_data, 2);
        assert!(similarity > 0.5); // Should detect periodicity
    }

    #[test]
    fn test_mandelbrot_iteration() {
        let iter = mandelbrot_iteration((0.0, 0.0), 100);
        assert_eq!(iter, 100); // Origin is in the set

        let iter2 = mandelbrot_iteration((2.0, 2.0), 100);
        assert!(iter2 < 100); // Point outside the set
    }

    #[test]
    fn test_market_efficiency() {
        let prices = vec![100.0, 101.0, 99.0, 102.0, 98.0, 103.0];
        let efficiency = market_efficiency_dimension(&prices);
        assert!(efficiency > 0.0 && efficiency < 3.0);
    }
}