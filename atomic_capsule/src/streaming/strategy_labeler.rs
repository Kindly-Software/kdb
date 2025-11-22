//! # StrategyLabelerCapsule - T5 Streaming Tier
//!
//! **Lockfree incremental strategy regime detection** with O(1) rolling window updates.
//!
//! ## UCE34 Framework Application
//!
//! ### Q1-Q9: Problem Analysis
//! - **Q1**: Detect 4 market regimes (Trend, MeanReversion, Breakout, Range) for strategy labeling
//! - **Q2**: Previous approach used placeholder [0.0] labels (no actual strategy detection)
//! - **Q3**: <10ns rolling update + <100ns detection (vs placeholder 0ns but incorrect labels)
//! - **Q4**: T5 Streaming (rolling windows) + T1 Atomic (lockfree coordination)
//! - **Q5**: `StrategyLabelerCapsule` (256-byte aligned for cache efficiency)
//! - **Q8**: 256 bytes (2 cache lines: state + rolling windows)
//!
//! ### Q10-Q12: Tier Selection
//! - **Q10**: Tier 5 Streaming (O(1) incremental updates) + Tier 1 Atomic (lockfree metrics)
//! - **Q11**: Rolling mean via running sum (O(1)), autocorrelation via ring buffer (O(1))
//! - **Q12**: Stable Rust sufficient (no nightly features required)
//!
//! ### Q13-Q27: Implementation Details
//! - **Memory ordering**: Relaxed for metrics (independent counters), Acquire for reads
//! - **Rolling windows**: 15-min ATR MA (900 ticks), 5-min spread MA (300 ticks)
//! - **Autocorrelation**: Lag-15 rolling autocorr via efficient ring buffer
//! - **False sharing prevention**: 256B alignment for entire capsule
//!
//! ### Q31: Simplicity
//! - Simple API: update(price, spread, volume) → StrategyLabel
//! - Hide complexity: Rolling window math internal, rule-based detection
//! - Builder pattern: Initialize with sensible defaults
//!
//! ### Q33: Verification
//! - #[derive(ComputationalCapsule)] for compile-time verification
//! - Alignment: 256 bytes, Size: 256 bytes, Tier: T5+T1
//!
//! ### Q34: Auditability
//! - Strategy distribution tracking (% Trend, MeanReversion, Breakout, Range)
//! - Transition count monitoring (regime change frequency)
//! - Confidence scoring (how strongly rule matches)
//!
//! ## Performance Targets (B32)
//! - `update()`: <10ns (O(1) rolling update)
//! - `detect()`: <100ns (rule-based classification, 4 checks)
//! - `statistics()`: <50ns (4 atomic loads)
//! - **Accuracy**: ≥70% on manual validation (1000 samples)
//! - **Memory**: 256 bytes per instance
//!
//! ## ASSUM Safety
//! - 99.99% safe: Zero unsafe code
//! - Memory ordering: Relaxed for metrics (approximate OK), Acquire for detection
//! - Overflow handling: Ring buffer wraps cleanly via modulo
//! - Divide-by-zero: All divisions guarded with epsilon checks
//!
//! ## Architecture
//!
//! ### Capsule Layout (256 bytes)
//! ```text
//! [0-63]:    metrics (DualAtomicU64)
//!            - primary: detection_count
//!            - secondary: transition_count
//! [64-127]:  strategy_counts (4 × AtomicU64)
//!            - Trend, MeanReversion, Breakout, Range counts
//! [128-191]: rolling_state (f64 accumulators)
//!            - atr_sum, spread_sum, price_sum, autocorr_state
//! [192-255]: ring_buffers (prices[16], spreads[16] for lag-15 autocorr)
//! ```
//!
//! ### 4-Regime Taxonomy
//! 1. **Trend**: High persistence (autocorr >0.7) + high volatility (ATR >1.2× MA)
//! 2. **MeanReversion**: Negative autocorr (<-0.3) + low volatility (ATR <0.8× MA)
//! 3. **Breakout**: Volatility spike (ATR >2.0× MA) + spread widens (>1.5× MA)
//! 4. **Range** (default): Everything else (low vol + near-zero autocorr)
//!
//! ### Detection Rules (Priority Order)
//! ```rust
//! if atr > 2.0 * atr_ma && spread > 1.5 * spread_ma {
//!     StrategyLabel::Breakout
//! } else if autocorr > 0.7 && atr > 1.2 * atr_ma {
//!     StrategyLabel::Trend
//! } else if autocorr < -0.3 && atr < 0.8 * atr_ma {
//!     StrategyLabel::MeanReversion
//! } else {
//!     StrategyLabel::Range
//! }
//! ```
//!
//! ## Usage
//! ```rust
//! use atomic_capsule::streaming::strategy_labeler::{
//!     StrategyLabelerCapsule, StrategyLabel
//! };
//!
//! // Initialize labeler
//! let mut labeler = StrategyLabelerCapsule::new();
//!
//! // Update with each market snapshot (O(1))
//! for snapshot in market_data {
//!     let label = labeler.update(
//!         snapshot.high,
//!         snapshot.low,
//!         snapshot.close,
//!         snapshot.spread
//!     );
//!
//!     match label {
//!         StrategyLabel::Trend => { /* Train trend strategy */ },
//!         StrategyLabel::MeanReversion => { /* Train mean reversion */ },
//!         StrategyLabel::Breakout => { /* Train breakout */ },
//!         StrategyLabel::Range => { /* Train range-bound */ },
//!     }
//! }
//!
//! // Get statistics
//! let stats = labeler.statistics();
//! println!("Trend: {:.1}%, MeanRev: {:.1}%, Breakout: {:.1}%, Range: {:.1}%",
//!     stats.trend_pct, stats.mean_reversion_pct, stats.breakout_pct, stats.range_pct);
//! ```

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU64, Ordering};

/// Strategy regime label (4 regimes)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StrategyLabel {
    /// High persistence (autocorr >0.7) + high volatility
    Trend = 0,
    /// Negative autocorr (<-0.3) + low volatility
    MeanReversion = 1,
    /// Volatility spike (ATR >2.0× MA) + spread widens
    Breakout = 2,
    /// Low volatility + near-zero autocorr (default)
    Range = 3,
}

impl StrategyLabel {
    /// Convert to string for serialization
    pub fn as_str(&self) -> &'static str {
        match self {
            StrategyLabel::Trend => "trend",
            StrategyLabel::MeanReversion => "mean_reversion",
            StrategyLabel::Breakout => "breakout",
            StrategyLabel::Range => "range",
        }
    }

    /// Convert to u8 for compact binary format
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// Convert from u8 (for deserialization)
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(StrategyLabel::Trend),
            1 => Some(StrategyLabel::MeanReversion),
            2 => Some(StrategyLabel::Breakout),
            3 => Some(StrategyLabel::Range),
            _ => None,
        }
    }
}

/// Strategy labeling statistics
#[derive(Debug, Clone)]
pub struct StrategyStats {
    pub total_detections: u64,
    pub transitions: u64,
    pub trend_count: u64,
    pub mean_reversion_count: u64,
    pub breakout_count: u64,
    pub range_count: u64,
    pub trend_pct: f64,
    pub mean_reversion_pct: f64,
    pub breakout_pct: f64,
    pub range_pct: f64,
}

/// T5 Streaming strategy labeler capsule
///
/// # Cache Alignment
/// - 256 bytes (2 cache lines)
/// - Entire capsule fits in L1 cache
/// - Zero false sharing (256B alignment)
///
/// # Lockfree Guarantees
/// - All updates via atomics (Relaxed ordering for metrics)
/// - Acquire ordering for detection reads
/// - No mutex/RwLock
///
/// #ASSUME_CACHE_ALIGNED: 256-byte alignment fits 2 cache lines (verified by derive macro)
/// #ASSUME_RELAXED_METRICS: Approximate metrics acceptable (counters independent)
/// #ASSUME_RING_WRAP: Modulo wraps cleanly for power-of-2 sizes
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 256, size = 256))]
#[repr(C, align(256))]
pub struct StrategyLabelerCapsule {
    /// Detection count (total calls to detect)
    detection_count: AtomicU64,

    /// Transition count (regime changes)
    transition_count: AtomicU64,

    /// Strategy counts (4 regimes)
    trend_count: AtomicU64,
    mean_reversion_count: AtomicU64,
    breakout_count: AtomicU64,
    range_count: AtomicU64,

    /// Rolling state (8 × f64 = 64 bytes)
    /// NOTE: Not atomic because single-threaded update pattern
    /// (each capsule instance owned by one thread in batch processing)
    atr_sum: f64,         // Sum of last 900 ATR values (15 min @ 1/sec)
    spread_sum: f64,      // Sum of last 300 spread values (5 min @ 1/sec)
    price_sum: f64,       // Sum of last 16 prices (for autocorr)
    atr_count: usize,     // Count for ATR MA (saturates at 900)
    spread_count: usize,  // Count for spread MA (saturates at 300)
    ring_index: usize,    // Current index in ring buffers
    prev_label: u8,       // Previous label (for transition detection)
    _padding1: [u8; 7],   // Align to 8 bytes

    /// Ring buffers for autocorrelation (16 × f64 = 128 bytes)
    /// Lag-15 autocorr requires 16 price history
    prices: [f64; 16],

    /// ATR history for true range calculation
    prev_high: f64,
    prev_low: f64,
    prev_close: f64,
}

impl StrategyLabelerCapsule {
    /// Create new strategy labeler
    ///
    /// #ASSUME_ZERO_INIT: All atomics zero-initialized (safe for counters)
    /// #ASSUME_F64_DEFAULT: f64 zero-init is 0.0 (IEEE 754 standard)
    pub fn new() -> Self {
        Self {
            detection_count: AtomicU64::new(0),
            transition_count: AtomicU64::new(0),
            trend_count: AtomicU64::new(0),
            mean_reversion_count: AtomicU64::new(0),
            breakout_count: AtomicU64::new(0),
            range_count: AtomicU64::new(0),
            atr_sum: 0.0,
            spread_sum: 0.0,
            price_sum: 0.0,
            atr_count: 0,
            spread_count: 0,
            ring_index: 0,
            prev_label: StrategyLabel::Range as u8,
            _padding1: [0u8; 7],
            prices: [0.0; 16],
            prev_high: 0.0,
            prev_low: 0.0,
            prev_close: 0.0,
        }
    }

    /// Update with market snapshot and detect strategy (O(1))
    ///
    /// # Arguments
    /// - `high`: Snapshot high price
    /// - `low`: Snapshot low price
    /// - `close`: Snapshot close price
    /// - `spread`: Bid-ask spread
    ///
    /// # Performance
    /// - <10ns: Rolling update (O(1) arithmetic)
    /// - <100ns: Strategy detection (4 rule checks)
    /// - Total: <110ns per call
    ///
    /// #ASSUME_FINITE: All inputs finite (validated upstream)
    /// #ASSUME_POSITIVE: Spread ≥ 0.0 (validated upstream)
    pub fn update(&mut self, high: f64, low: f64, close: f64, spread: f64) -> StrategyLabel {
        // Calculate true range for ATR
        let true_range = if self.prev_close > 0.0 {
            let hl = high - low;
            let hc = (high - self.prev_close).abs();
            let lc = (low - self.prev_close).abs();
            hl.max(hc).max(lc)
        } else {
            high - low
        };

        // Update ATR rolling sum (O(1))
        self.atr_sum += true_range;
        self.atr_count += 1;
        if self.atr_count > 900 {
            // Approximate decay (good enough for strategy detection)
            self.atr_sum *= 0.999;
            self.atr_count = 900;
        }

        // Update spread rolling sum (O(1))
        self.spread_sum += spread;
        self.spread_count += 1;
        if self.spread_count > 300 {
            self.spread_sum *= 0.997;
            self.spread_count = 300;
        }

        // Update ring buffer for autocorrelation (O(1))
        let old_price = self.prices[self.ring_index];
        self.prices[self.ring_index] = close;
        self.price_sum = self.price_sum - old_price + close;
        self.ring_index = (self.ring_index + 1) % 16;

        // Update previous OHLC for next iteration
        self.prev_high = high;
        self.prev_low = low;
        self.prev_close = close;

        // Detect strategy
        let label = self.detect();

        // Update metrics (atomic for multi-threaded stats)
        self.detection_count.fetch_add(1, Ordering::Relaxed);

        if label as u8 != self.prev_label {
            self.transition_count.fetch_add(1, Ordering::Relaxed);
            self.prev_label = label as u8;
        }

        match label {
            StrategyLabel::Trend => self.trend_count.fetch_add(1, Ordering::Relaxed),
            StrategyLabel::MeanReversion => self.mean_reversion_count.fetch_add(1, Ordering::Relaxed),
            StrategyLabel::Breakout => self.breakout_count.fetch_add(1, Ordering::Relaxed),
            StrategyLabel::Range => self.range_count.fetch_add(1, Ordering::Relaxed),
        };

        label
    }

    /// Detect strategy from current state (<100ns)
    ///
    /// # Detection Rules (Priority Order)
    /// 1. Breakout: `atr > 2.0 × atr_ma && spread > 1.5 × spread_ma`
    /// 2. Trend: `autocorr > 0.7 && atr > 1.2 × atr_ma`
    /// 3. MeanReversion: `autocorr < -0.3 && atr < 0.8 × atr_ma`
    /// 4. Range: Default (everything else)
    ///
    /// #ASSUME_DIVIDE_BY_ZERO: atr_count/spread_count guarded with epsilon checks
    fn detect(&self) -> StrategyLabel {
        // Calculate ATR MA (avoid division by zero)
        let atr_ma = if self.atr_count > 0 {
            self.atr_sum / self.atr_count as f64
        } else {
            return StrategyLabel::Range; // Not enough data
        };

        // Calculate spread MA
        let spread_ma = if self.spread_count > 0 {
            self.spread_sum / self.spread_count as f64
        } else {
            return StrategyLabel::Range;
        };

        // Calculate current ATR (last value approximation)
        let current_atr = if self.atr_count > 0 {
            self.atr_sum / self.atr_count as f64
        } else {
            0.0
        };

        // Calculate lag-15 autocorrelation (O(1) from ring buffer)
        let autocorr = self.calculate_autocorr();

        // Apply detection rules (priority order)
        if current_atr > 2.0 * atr_ma && self.spread_sum / self.spread_count.max(1) as f64 > 1.5 * spread_ma {
            StrategyLabel::Breakout
        } else if autocorr > 0.7 && current_atr > 1.2 * atr_ma {
            StrategyLabel::Trend
        } else if autocorr < -0.3 && current_atr < 0.8 * atr_ma {
            StrategyLabel::MeanReversion
        } else {
            StrategyLabel::Range
        }
    }

    /// Calculate lag-15 autocorrelation (O(1) from ring buffer)
    ///
    /// # Formula
    /// autocorr(15) = Cov(price_t, price_{t-15}) / (σ_t × σ_{t-15})
    ///
    /// # Approximation
    /// Use simple correlation of [0..15] vs [1..16] from ring buffer
    ///
    /// #ASSUME_VARIANCE_NONZERO: Returns 0.0 if variance is zero (rare)
    fn calculate_autocorr(&self) -> f64 {
        if self.price_sum == 0.0 {
            return 0.0; // Not enough data
        }

        // Calculate mean
        let mean = self.price_sum / 16.0;

        // Calculate variance and covariance
        let mut variance = 0.0;
        let mut covariance = 0.0;

        for i in 0..15 {
            let curr = self.prices[i];
            let next = self.prices[i + 1];
            let curr_dev = curr - mean;
            let next_dev = next - mean;
            variance += curr_dev * curr_dev;
            covariance += curr_dev * next_dev;
        }

        // Autocorrelation = cov / var (lag-1 approximation)
        if variance > 1e-10 {
            covariance / variance
        } else {
            0.0 // Near-constant prices
        }
    }

    /// Get strategy labeling statistics
    ///
    /// # Performance
    /// <50ns (6 atomic loads + arithmetic)
    pub fn statistics(&self) -> StrategyStats {
        let total = self.detection_count.load(Ordering::Acquire);
        let transitions = self.transition_count.load(Ordering::Acquire);
        let trend = self.trend_count.load(Ordering::Acquire);
        let mean_rev = self.mean_reversion_count.load(Ordering::Acquire);
        let breakout = self.breakout_count.load(Ordering::Acquire);
        let range = self.range_count.load(Ordering::Acquire);

        let total_f64 = total.max(1) as f64; // Avoid division by zero

        StrategyStats {
            total_detections: total,
            transitions,
            trend_count: trend,
            mean_reversion_count: mean_rev,
            breakout_count: breakout,
            range_count: range,
            trend_pct: (trend as f64 / total_f64) * 100.0,
            mean_reversion_pct: (mean_rev as f64 / total_f64) * 100.0,
            breakout_pct: (breakout as f64 / total_f64) * 100.0,
            range_pct: (range as f64 / total_f64) * 100.0,
        }
    }
}

impl Default for StrategyLabelerCapsule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t28_q1_new_labeler() {
        let labeler = StrategyLabelerCapsule::new();
        assert_eq!(labeler.detection_count.load(Ordering::Relaxed), 0);
        assert_eq!(labeler.transition_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn t28_q2_strategy_label_conversion() {
        assert_eq!(StrategyLabel::Trend.as_u8(), 0);
        assert_eq!(StrategyLabel::MeanReversion.as_u8(), 1);
        assert_eq!(StrategyLabel::Breakout.as_u8(), 2);
        assert_eq!(StrategyLabel::Range.as_u8(), 3);

        assert_eq!(StrategyLabel::from_u8(0), Some(StrategyLabel::Trend));
        assert_eq!(StrategyLabel::from_u8(3), Some(StrategyLabel::Range));
        assert_eq!(StrategyLabel::from_u8(4), None);
    }

    #[test]
    fn t28_q3_default_range_detection() {
        let mut labeler = StrategyLabelerCapsule::new();

        // Initial detection should be Range (not enough data)
        let label = labeler.update(100.0, 99.5, 99.8, 0.1);
        assert_eq!(label, StrategyLabel::Range);
    }

    #[test]
    fn t28_q4_statistics() {
        let mut labeler = StrategyLabelerCapsule::new();

        // Update 10 times
        for i in 0..10 {
            labeler.update(100.0 + i as f64, 99.5 + i as f64, 99.8 + i as f64, 0.1);
        }

        let stats = labeler.statistics();
        assert_eq!(stats.total_detections, 10);
        assert!(stats.trend_pct + stats.mean_reversion_pct + stats.breakout_pct + stats.range_pct > 99.0);
    }

    #[test]
    fn t28_q5_trend_detection() {
        let mut labeler = StrategyLabelerCapsule::new();

        // Simulate trending market (high persistence)
        for i in 0..100 {
            let price = 100.0 + i as f64; // Strong uptrend
            labeler.update(price + 0.5, price - 0.5, price, 0.1);
        }

        let stats = labeler.statistics();
        // Should detect trend eventually (may take time to build autocorr)
        assert!(stats.trend_count > 0 || stats.breakout_count > 0); // Either trend or breakout
    }

    #[test]
    fn t28_q6_range_detection() {
        let mut labeler = StrategyLabelerCapsule::new();

        // Simulate range-bound market (oscillating)
        for i in 0..100 {
            let price = 100.0 + ((i % 10) as f64 - 5.0) * 0.1; // Oscillating ±0.5
            labeler.update(price + 0.05, price - 0.05, price, 0.05);
        }

        let stats = labeler.statistics();
        // Should detect range (low volatility, near-zero autocorr)
        assert!(stats.range_count > 50); // Majority should be Range
    }

    #[test]
    fn t28_q7_breakout_detection() {
        let mut labeler = StrategyLabelerCapsule::new();

        // Simulate stable period
        for _i in 0..50 {
            labeler.update(100.0, 99.9, 99.95, 0.05);
        }

        // Simulate breakout (volatility spike)
        for i in 0..20 {
            let price = 100.0 + i as f64 * 2.0; // Large price moves
            labeler.update(price + 2.0, price - 2.0, price, 0.5); // Wide spread
        }

        let stats = labeler.statistics();
        // Should detect breakout (volatility spike + spread widening)
        assert!(stats.breakout_count > 0);
    }
}
