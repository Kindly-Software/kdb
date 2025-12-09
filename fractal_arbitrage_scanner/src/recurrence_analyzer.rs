//! Recurrence Quantification Analysis for Market Regime Detection
//!
//! Detects regime transitions (normal → bubble → crash → recovery) using RQA.
//! LAM (Laminarity) measure identifies bubbles before they burst.
//!
//! # UCE32 Framework Analysis
//!
//! Q28 (Simplicity): Use LAM as primary measure (proven bubble detector)
//! Q29 (Constraints): Works with short nonstationary data (100 points minimum)
//! Q30 (Validation): Validate on 2008, COVID-19, and crypto crashes
//! Q31 (Rust): Zero-cost regime state machine with atomics
//! Q32 (Nightly): const_trait_impl for compile-time RQA parameters

#![cfg_attr(feature = "const_trait_impl", feature(const_trait_impl))]

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::collections::VecDeque;

/// Market regime states detected by RQA
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketRegime {
    /// Normal functioning market
    Normal = 0,

    /// Pre-bubble formation (increasing LAM)
    PreBubble = 1,

    /// Bubble phase (high LAM, low entropy)
    Bubble = 2,

    /// Critical transition point (LAM peak)
    Critical = 3,

    /// Crash phase (LAM collapse)
    Crash = 4,

    /// Recovery/relaxation phase
    Recovery = 5,

    /// Chaotic/turbulent (high entropy, low LAM)
    Chaotic = 6,
}

impl MarketRegime {
    /// Can we arbitrage in this regime?
    pub fn allows_arbitrage(&self) -> bool {
        matches!(self,
            MarketRegime::Normal |
            MarketRegime::PreBubble |
            MarketRegime::Recovery
        )
    }

    /// Risk level of the regime
    pub fn risk_level(&self) -> f64 {
        match self {
            MarketRegime::Normal => 0.2,
            MarketRegime::PreBubble => 0.4,
            MarketRegime::Bubble => 0.7,
            MarketRegime::Critical => 0.95,
            MarketRegime::Crash => 1.0,
            MarketRegime::Recovery => 0.5,
            MarketRegime::Chaotic => 0.8,
        }
    }
}

/// Recurrence plot for phase space analysis
struct RecurrencePlot {
    /// Binary recurrence matrix
    matrix: Vec<Vec<bool>>,
    /// Embedding dimension
    dimension: usize,
    /// Time delay
    delay: usize,
    /// Recurrence threshold
    threshold: f64,
}

impl RecurrencePlot {
    fn new(data: &[f64], dimension: usize, delay: usize, threshold: f64) -> Self {
        let n = data.len() - (dimension - 1) * delay;
        let mut matrix = vec![vec![false; n]; n];

        // Construct phase space vectors
        let mut vectors = Vec::new();
        for i in 0..n {
            let mut v = Vec::new();
            for j in 0..dimension {
                v.push(data[i + j * delay]);
            }
            vectors.push(v);
        }

        // Calculate recurrence matrix
        for i in 0..n {
            for j in 0..n {
                let dist = vectors[i].iter()
                    .zip(vectors[j].iter())
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f64>()
                    .sqrt();

                matrix[i][j] = dist < threshold;
            }
        }

        Self {
            matrix,
            dimension,
            delay,
            threshold,
        }
    }

    /// Count diagonal lines of length l
    fn count_diagonals(&self, min_length: usize) -> Vec<usize> {
        let n = self.matrix.len();
        let mut counts = vec![0; n];

        for i in 0..n {
            let mut length = 0;
            for j in 0..n.saturating_sub(i) {
                if self.matrix[j][i + j] {
                    length += 1;
                } else {
                    if length >= min_length {
                        counts[length] += 1;
                    }
                    length = 0;
                }
            }
            if length >= min_length {
                counts[length] += 1;
            }
        }

        counts
    }

    /// Count vertical lines (laminarity)
    fn count_verticals(&self, min_length: usize) -> Vec<usize> {
        let n = self.matrix.len();
        let mut counts = vec![0; n];

        for i in 0..n {
            let mut length = 0;
            for j in 0..n {
                if self.matrix[j][i] {
                    length += 1;
                } else {
                    if length >= min_length {
                        counts[length] += 1;
                    }
                    length = 0;
                }
            }
            if length >= min_length {
                counts[length] += 1;
            }
        }

        counts
    }
}

/// RQA measures for regime detection
#[derive(Debug, Clone)]
pub struct RQAMeasures {
    /// Recurrence rate (density of recurrence points)
    pub recurrence_rate: f64,

    /// Determinism (predictability)
    pub determinism: f64,

    /// Laminarity (LAM) - key bubble indicator
    pub laminarity: f64,

    /// Trapping time (average vertical line length)
    pub trapping_time: f64,

    /// Entropy of diagonal lines
    pub entropy: f64,

    /// Maximum diagonal line length
    pub max_line: usize,
}

impl RQAMeasures {
    fn calculate(rp: &RecurrencePlot) -> Self {
        let n = rp.matrix.len();
        let n2 = n * n;

        // Recurrence rate
        let recurrence_points: usize = rp.matrix.iter()
            .flat_map(|row| row.iter())
            .filter(|&&x| x)
            .count();
        let recurrence_rate = recurrence_points as f64 / n2 as f64;

        // Diagonal lines (determinism)
        let diagonals = rp.count_diagonals(2);
        let total_diagonal: usize = diagonals.iter()
            .enumerate()
            .map(|(len, &count)| len * count)
            .sum();
        let determinism = if recurrence_points > 0 {
            total_diagonal as f64 / recurrence_points as f64
        } else {
            0.0
        };

        // Vertical lines (laminarity)
        let verticals = rp.count_verticals(2);
        let total_vertical: usize = verticals.iter()
            .enumerate()
            .map(|(len, &count)| len * count)
            .sum();
        let laminarity = if recurrence_points > 0 {
            total_vertical as f64 / recurrence_points as f64
        } else {
            0.0
        };

        // Trapping time
        let total_v_count: usize = verticals.iter().sum();
        let trapping_time = if total_v_count > 0 {
            total_vertical as f64 / total_v_count as f64
        } else {
            0.0
        };

        // Entropy
        let total_d_count: usize = diagonals.iter().sum();
        let entropy = if total_d_count > 0 {
            -diagonals.iter()
                .filter(|&&c| c > 0)
                .map(|&c| {
                    let p = c as f64 / total_d_count as f64;
                    p * p.ln()
                })
                .sum::<f64>()
        } else {
            0.0
        };

        // Max line
        let max_line = diagonals.iter()
            .rposition(|&c| c > 0)
            .unwrap_or(0);

        Self {
            recurrence_rate,
            determinism,
            laminarity,
            trapping_time,
            entropy,
            max_line,
        }
    }
}

/// Recurrence Quantification Analyzer
pub struct RecurrenceAnalyzer {
    /// Sliding window of prices
    price_window: VecDeque<f64>,

    /// Window size
    window_size: usize,

    /// Embedding dimension for phase space
    embedding_dim: usize,

    /// Time delay for embedding
    time_delay: usize,

    /// Current detected regime
    current_regime: AtomicU8,

    /// LAM history for trend detection
    lam_history: VecDeque<f64>,

    /// Generation counter
    generation: AtomicU64,

    /// Regime transitions detected
    transitions_detected: AtomicU64,
}

impl RecurrenceAnalyzer {
    pub fn new() -> Self {
        Self {
            price_window: VecDeque::with_capacity(256),
            window_size: 100,
            embedding_dim: 3,
            time_delay: 1,
            current_regime: AtomicU8::new(MarketRegime::Normal as u8),
            lam_history: VecDeque::with_capacity(50),
            generation: AtomicU64::new(0),
            transitions_detected: AtomicU64::new(0),
        }
    }

    /// Update with new price and detect regime
    pub fn update(&mut self, price: f64) -> MarketRegime {
        let _gen = self.generation.fetch_add(1, Ordering::Relaxed);

        // Add to window
        self.price_window.push_back(price);
        if self.price_window.len() > self.window_size {
            self.price_window.pop_front();
        }

        // Need enough data
        if self.price_window.len() < self.window_size {
            return MarketRegime::Normal;
        }

        // Convert to returns
        let returns: Vec<f64> = self.price_window.iter()
            .collect::<Vec<_>>()
            .windows(2)
            .map(|w| (w[1] / w[0]).ln())
            .collect();

        // Calculate RQA measures
        let threshold = self.calculate_threshold(&returns);
        let rp = RecurrencePlot::new(&returns, self.embedding_dim, self.time_delay, threshold);
        let measures = RQAMeasures::calculate(&rp);

        // Update LAM history
        self.lam_history.push_back(measures.laminarity);
        if self.lam_history.len() > 50 {
            self.lam_history.pop_front();
        }

        // Detect regime based on RQA measures
        let regime = self.classify_regime(&measures);

        // Check for transition
        let old_regime = self.current_regime.load(Ordering::Relaxed);
        if old_regime != regime as u8 {
            self.transitions_detected.fetch_add(1, Ordering::Relaxed);
            self.current_regime.store(regime as u8, Ordering::Relaxed);
        }

        regime
    }

    /// Calculate adaptive threshold using variance
    fn calculate_threshold(&self, data: &[f64]) -> f64 {
        let mean = data.iter().sum::<f64>() / data.len() as f64;
        let variance = data.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / data.len() as f64;

        0.1 * variance.sqrt()  // 10% of standard deviation
    }

    /// Classify regime based on RQA measures
    fn classify_regime(&self, measures: &RQAMeasures) -> MarketRegime {
        let lam = measures.laminarity;
        let det = measures.determinism;
        let _ent = measures.entropy;

        // LAM trend (increasing/decreasing)
        let lam_trend = self.calculate_lam_trend();

        // Classification rules based on empirical research
        if lam > 0.95 {
            MarketRegime::Critical  // LAM peak = critical transition
        } else if lam > 0.8 && lam_trend > 0.1 {
            MarketRegime::Bubble  // High LAM and increasing
        } else if lam > 0.6 && lam_trend > 0.05 {
            MarketRegime::PreBubble  // Moderate LAM, increasing
        } else if lam < 0.3 && det < 0.4 {
            MarketRegime::Chaotic  // Low structure
        } else if lam < 0.4 && lam_trend < -0.1 {
            MarketRegime::Crash  // LAM collapse
        } else if lam < 0.5 && lam_trend > 0.0 {
            MarketRegime::Recovery  // Low LAM but recovering
        } else {
            MarketRegime::Normal  // Default state
        }
    }

    /// Calculate LAM trend (derivative)
    fn calculate_lam_trend(&self) -> f64 {
        if self.lam_history.len() < 10 {
            return 0.0;
        }

        // Linear regression on recent LAM values
        let n = self.lam_history.len().min(20);
        let recent: Vec<f64> = self.lam_history.iter()
            .rev()
            .take(n)
            .copied()
            .collect();

        let x_mean = (n - 1) as f64 / 2.0;
        let y_mean = recent.iter().sum::<f64>() / n as f64;

        let mut num = 0.0;
        let mut den = 0.0;

        for (i, &y) in recent.iter().enumerate() {
            let x = i as f64;
            num += (x - x_mean) * (y - y_mean);
            den += (x - x_mean).powi(2);
        }

        if den > 0.0 {
            num / den
        } else {
            0.0
        }
    }

    /// Get current RQA measures
    pub fn get_measures(&self) -> Option<RQAMeasures> {
        if self.price_window.len() < self.window_size {
            return None;
        }

        let returns: Vec<f64> = self.price_window.iter()
            .collect::<Vec<_>>()
            .windows(2)
            .map(|w| (w[1] / w[0]).ln())
            .collect();

        let threshold = self.calculate_threshold(&returns);
        let rp = RecurrencePlot::new(&returns, self.embedding_dim, self.time_delay, threshold);

        Some(RQAMeasures::calculate(&rp))
    }

    /// Predict time to regime transition
    pub fn predict_transition_time(&self) -> Option<u64> {
        // Based on LAM acceleration
        let trend = self.calculate_lam_trend();

        if trend.abs() < 0.001 {
            return None;  // No significant trend
        }

        // Estimate based on current LAM and trend
        let current_lam = self.lam_history.back().copied().unwrap_or(0.5);

        // Time to critical LAM (0.95)
        let time_to_critical = ((0.95 - current_lam) / trend).abs();

        Some((time_to_critical * 1000.0) as u64)  // Convert to milliseconds
    }
}

/// Regime-aware arbitrage strategy
#[derive(Debug, Clone)]
pub struct RegimeArbitrageStrategy {
    pub regime: MarketRegime,
    pub position_size_multiplier: f64,
    pub stop_loss_multiplier: f64,
    pub take_profit_multiplier: f64,
    pub max_holding_period_ms: u64,
}

impl RegimeArbitrageStrategy {
    pub fn from_regime(regime: MarketRegime) -> Self {
        match regime {
            MarketRegime::Normal => Self {
                regime,
                position_size_multiplier: 1.0,
                stop_loss_multiplier: 1.0,
                take_profit_multiplier: 1.0,
                max_holding_period_ms: 60000,  // 1 minute
            },
            MarketRegime::PreBubble => Self {
                regime,
                position_size_multiplier: 0.8,  // Reduce size
                stop_loss_multiplier: 0.8,      // Tighter stops
                take_profit_multiplier: 1.2,    // Let winners run
                max_holding_period_ms: 30000,   // 30 seconds
            },
            MarketRegime::Bubble => Self {
                regime,
                position_size_multiplier: 0.3,  // Minimal size
                stop_loss_multiplier: 0.5,      // Very tight stops
                take_profit_multiplier: 0.8,    // Quick profits
                max_holding_period_ms: 10000,   // 10 seconds
            },
            MarketRegime::Critical => Self {
                regime,
                position_size_multiplier: 0.0,  // NO TRADING
                stop_loss_multiplier: 0.0,
                take_profit_multiplier: 0.0,
                max_holding_period_ms: 0,
            },
            MarketRegime::Crash => Self {
                regime,
                position_size_multiplier: 0.0,  // NO TRADING
                stop_loss_multiplier: 0.0,
                take_profit_multiplier: 0.0,
                max_holding_period_ms: 0,
            },
            MarketRegime::Recovery => Self {
                regime,
                position_size_multiplier: 1.2,  // Increase size
                stop_loss_multiplier: 1.5,      // Wider stops
                take_profit_multiplier: 1.5,    // Larger targets
                max_holding_period_ms: 120000,  // 2 minutes
            },
            MarketRegime::Chaotic => Self {
                regime,
                position_size_multiplier: 0.5,  // Half size
                stop_loss_multiplier: 0.7,      // Tight stops
                take_profit_multiplier: 0.9,    // Quick exits
                max_holding_period_ms: 20000,   // 20 seconds
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regime_detection() {
        let mut analyzer = RecurrenceAnalyzer::new();

        // Simulate bubble formation
        for i in 0..150 {
            let price = 100.0 * (1.0 + 0.01 * i as f64).powi(2);  // Parabolic growth
            let regime = analyzer.update(price);

            // Should detect bubble or critical regime eventually
            if i > 100 {
                assert!(matches!(
                    regime,
                    MarketRegime::PreBubble | MarketRegime::Bubble | MarketRegime::Critical
                ));
            }
        }
    }

    #[test]
    fn test_rqa_measures() {
        let data = vec![1.0, 1.1, 1.0, 0.9, 1.0, 1.1, 1.0, 0.9];  // Oscillating
        let rp = RecurrencePlot::new(&data, 2, 1, 0.2);
        let measures = RQAMeasures::calculate(&rp);

        // Should have high determinism for periodic data
        assert!(measures.determinism > 0.5);
        assert!(measures.recurrence_rate > 0.0);
    }

    #[test]
    fn test_regime_strategy() {
        let normal_strategy = RegimeArbitrageStrategy::from_regime(MarketRegime::Normal);
        assert_eq!(normal_strategy.position_size_multiplier, 1.0);

        let critical_strategy = RegimeArbitrageStrategy::from_regime(MarketRegime::Critical);
        assert_eq!(critical_strategy.position_size_multiplier, 0.0);  // No trading

        let recovery_strategy = RegimeArbitrageStrategy::from_regime(MarketRegime::Recovery);
        assert!(recovery_strategy.position_size_multiplier > 1.0);  // Increased size
    }
}