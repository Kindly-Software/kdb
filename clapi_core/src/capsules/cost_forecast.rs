//! CostForecast256 - Trend analysis and anomaly detection
//!
//! Tier 4+3 (Batch+Fixed-Point) - 256B capsule for:
//! - 28-day rolling window (batch processing)
//! - Linear regression trend (Q16.8 fixed-point)
//! - Anomaly detection (mean + 2σ threshold)
//! - Cost prediction (daily burn rate)
//!
//! Performance: <100ns forecast lookup, <1ms trend update
//!
//! UCE34 Q10: T4+T3 (Batch+Fixed-Point) for deterministic forecasting
//! UCE34 Q11: AtomicI32 for Q16.8 fixed-point trend storage
//! UCE34 Q33: Compile-time verification with #[derive(ComputationalCapsule)]

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicI32, AtomicU32, AtomicU64, Ordering};

/// Cost forecast capsule (256B, T4+T3 Batch+Fixed-Point)
///
/// # Memory Layout
/// ```text
/// [0-7]     budget_id: AtomicU64
/// [8-11]    trend_q16_8: AtomicI32        // Daily burn rate (Q16.8 cents/day)
/// [12-15]   mean_cost_q16_8: AtomicI32    // Mean daily cost (Q16.8 cents)
/// [16-19]   std_dev_q16_8: AtomicI32      // Standard deviation (Q16.8 cents)
/// [20-23]   anomaly_count: AtomicU32      // Anomaly counter
/// [24-27]   generation: AtomicU32         // Update counter
/// [28-139]  recent_costs[28]: [AtomicI32; 28]  // 28-day window (Q16.8)
/// [140-255] _padding: [u8; 116]
/// ```
///
/// # Q16.8 Fixed-Point Format
/// - 16 bits integer, 8 bits fractional
/// - Range: -128.00 to +127.996 cents ($-1.28 to $1.28)
/// - Precision: 1/256 cent (~0.004 cents)
/// - Sufficient for daily cost deltas (-$1 to +$1 typical)
///
/// # Trend Calculation (Linear Regression)
/// Slope = (Σ(xy) - n*mean_x*mean_y) / (Σ(x²) - n*mean_x²)
/// Where: x = day index (0-27), y = cost (Q16.8)
///
/// # Anomaly Detection
/// Anomaly if: current_cost > mean + 2*std_dev
/// (2σ threshold = 95% confidence interval)
///
/// # Safety
/// - #ASSUME: Q16.8 fixed-point prevents FP drift
/// - #VERIFY: Unit tests validate arithmetic correctness
/// - #ASSUME: 28-day window sufficient for trend detection
/// - #VERIFY: Property tests validate trend accuracy
/// - #ASSUME: Relaxed ordering safe for statistical aggregation
/// - #VERIFY: Concurrent update tests validate correctness
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 256)]
#[repr(C, align(64))]
pub struct CostForecast256 {
    budget_id: AtomicU64,
    trend_q16_8: AtomicI32,       // Daily burn rate (cents/day, Q16.8)
    mean_cost_q16_8: AtomicI32,   // Mean daily cost (Q16.8)
    std_dev_q16_8: AtomicI32,     // Standard deviation (Q16.8)
    anomaly_count: AtomicU32,     // Total anomalies detected
    generation: AtomicU32,        // Update counter
    recent_costs: [AtomicI32; 28], // 28-day window (Q16.8)
    _padding: [u8; 116],
}

/// Forecast snapshot (for reading)
#[derive(Debug, Clone, Copy)]
pub struct ForecastSnapshot {
    pub budget_id: u64,
    pub daily_burn_rate_cents: f64,  // Trend (cents/day)
    pub mean_cost_cents: f64,         // Mean daily cost
    pub std_dev_cents: f64,           // Standard deviation
    pub anomaly_count: u32,           // Total anomalies
    pub generation: u32,              // Update counter
    pub recent_costs_cents: [f64; 28], // 28-day window
}

impl CostForecast256 {
    /// Create new cost forecast capsule
    pub fn new(budget_id: u64) -> Self {
        Self {
            budget_id: AtomicU64::new(budget_id),
            trend_q16_8: AtomicI32::new(0),
            mean_cost_q16_8: AtomicI32::new(0),
            std_dev_q16_8: AtomicI32::new(0),
            anomaly_count: AtomicU32::new(0),
            generation: AtomicU32::new(0),
            recent_costs: Self::init_costs(),
            _padding: [0u8; 116],
        }
    }

    /// Initialize cost array (const fn for atomic init)
    const fn init_costs() -> [AtomicI32; 28] {
        // Manual array initialization (const fn limitation)
        [
            AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0),
            AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0),
            AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0),
            AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0),
            AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0),
            AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0),
            AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0),
        ]
    }

    /// Update with new daily cost (<1ms, batch processing)
    ///
    /// # Algorithm
    /// 1. Shift window left (discard oldest)
    /// 2. Append new cost (newest)
    /// 3. Recompute trend (linear regression slope)
    /// 4. Recompute mean and std_dev
    /// 5. Check for anomaly (current > mean + 2σ)
    ///
    /// # Safety
    /// - #ASSUME: Relaxed ordering safe for statistical updates
    /// - #VERIFY: Unit test validates trend calculation
    pub fn update(&self, daily_cost_cents: f64) {
        let cost_q16_8 = Self::to_q16_8(daily_cost_cents);

        // Shift window left (0 ← 1 ← 2 ← ... ← 27)
        for i in 0..27 {
            let next_val = self.recent_costs[i + 1].load(Ordering::Relaxed);
            self.recent_costs[i].store(next_val, Ordering::Relaxed);
        }

        // Append new cost
        self.recent_costs[27].store(cost_q16_8, Ordering::Relaxed);

        // Recompute statistics
        let (mean_q16_8, std_dev_q16_8, trend_q16_8) = self.compute_statistics();

        // Update capsule
        self.mean_cost_q16_8.store(mean_q16_8, Ordering::Relaxed);
        self.std_dev_q16_8.store(std_dev_q16_8, Ordering::Relaxed);
        self.trend_q16_8.store(trend_q16_8, Ordering::Relaxed);

        // Increment generation
        let gen = self.generation.fetch_add(1, Ordering::Release);

        // Check for anomaly ONLY after we have a full window (28 samples)
        // This prevents false positives during the initial fill phase
        if gen >= 27 {
            let threshold_q16_8 = mean_q16_8 + 2 * std_dev_q16_8;
            if cost_q16_8 > threshold_q16_8 {
                self.anomaly_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Compute statistics (mean, std_dev, trend) from window
    ///
    /// # Returns
    /// (mean_q16_8, std_dev_q16_8, trend_q16_8)
    ///
    /// # Algorithm
    /// 1. Mean: Σy / n
    /// 2. Std Dev: √(Σ(y - mean)² / n)
    /// 3. Trend (slope): (Σ(xy) - n*mean_x*mean_y) / (Σ(x²) - n*mean_x²)
    ///
    /// # Performance
    /// Single-pass over 28 elements: O(28) = O(1)
    fn compute_statistics(&self) -> (i32, i32, i32) {
        let n = 28;

        // Load all costs (single pass)
        let costs: [i32; 28] = [
            self.recent_costs[0].load(Ordering::Relaxed),
            self.recent_costs[1].load(Ordering::Relaxed),
            self.recent_costs[2].load(Ordering::Relaxed),
            self.recent_costs[3].load(Ordering::Relaxed),
            self.recent_costs[4].load(Ordering::Relaxed),
            self.recent_costs[5].load(Ordering::Relaxed),
            self.recent_costs[6].load(Ordering::Relaxed),
            self.recent_costs[7].load(Ordering::Relaxed),
            self.recent_costs[8].load(Ordering::Relaxed),
            self.recent_costs[9].load(Ordering::Relaxed),
            self.recent_costs[10].load(Ordering::Relaxed),
            self.recent_costs[11].load(Ordering::Relaxed),
            self.recent_costs[12].load(Ordering::Relaxed),
            self.recent_costs[13].load(Ordering::Relaxed),
            self.recent_costs[14].load(Ordering::Relaxed),
            self.recent_costs[15].load(Ordering::Relaxed),
            self.recent_costs[16].load(Ordering::Relaxed),
            self.recent_costs[17].load(Ordering::Relaxed),
            self.recent_costs[18].load(Ordering::Relaxed),
            self.recent_costs[19].load(Ordering::Relaxed),
            self.recent_costs[20].load(Ordering::Relaxed),
            self.recent_costs[21].load(Ordering::Relaxed),
            self.recent_costs[22].load(Ordering::Relaxed),
            self.recent_costs[23].load(Ordering::Relaxed),
            self.recent_costs[24].load(Ordering::Relaxed),
            self.recent_costs[25].load(Ordering::Relaxed),
            self.recent_costs[26].load(Ordering::Relaxed),
            self.recent_costs[27].load(Ordering::Relaxed),
        ];

        // Compute mean (Q16.8)
        let sum_y: i64 = costs.iter().map(|&c| c as i64).sum();
        let mean_q16_8 = (sum_y / n) as i32;

        // Compute variance (Q16.8 squared, need to rescale)
        let sum_sq_dev: i64 = costs
            .iter()
            .map(|&c| {
                let dev = c - mean_q16_8;
                (dev as i64) * (dev as i64)
            })
            .sum();

        // Variance in Q16.8 space (divide by n, then sqrt)
        // Note: sqrt(Q16.8²) = Q16.8 * 256, so divide by 256
        let variance_q32_16 = sum_sq_dev / n;
        let std_dev_q16_8 = ((variance_q32_16 as f64).sqrt() / 256.0) as i32;

        // Compute linear regression slope (trend)
        // x = day index (0-27), y = cost (Q16.8)
        // slope = (Σ(xy) - n*mean_x*mean_y) / (Σ(x²) - n*mean_x²)

        let sum_x: i64 = (0..n).sum(); // 0+1+...+27 = 378
        let sum_x2: i64 = (0..n).map(|x| x * x).sum(); // 0²+1²+...+27² = 7,182
        let sum_xy: i64 = costs
            .iter()
            .enumerate()
            .map(|(x, &y)| (x as i64) * (y as i64))
            .sum();

        let mean_x = sum_x as f64 / n as f64; // 13.5
        let mean_y = mean_q16_8 as f64;

        let numerator = sum_xy as f64 - (n as f64 * mean_x * mean_y);
        let denominator = sum_x2 as f64 - (n as f64 * mean_x * mean_x);

        let slope = if denominator.abs() > 1e-6 {
            numerator / denominator
        } else {
            0.0 // Flat trend (no variation)
        };

        let trend_q16_8 = slope as i32;

        (mean_q16_8, std_dev_q16_8, trend_q16_8)
    }

    /// Get forecast snapshot (<100ns, lockfree read)
    pub fn snapshot(&self) -> ForecastSnapshot {
        let recent_costs_q16_8: [i32; 28] = [
            self.recent_costs[0].load(Ordering::Relaxed),
            self.recent_costs[1].load(Ordering::Relaxed),
            self.recent_costs[2].load(Ordering::Relaxed),
            self.recent_costs[3].load(Ordering::Relaxed),
            self.recent_costs[4].load(Ordering::Relaxed),
            self.recent_costs[5].load(Ordering::Relaxed),
            self.recent_costs[6].load(Ordering::Relaxed),
            self.recent_costs[7].load(Ordering::Relaxed),
            self.recent_costs[8].load(Ordering::Relaxed),
            self.recent_costs[9].load(Ordering::Relaxed),
            self.recent_costs[10].load(Ordering::Relaxed),
            self.recent_costs[11].load(Ordering::Relaxed),
            self.recent_costs[12].load(Ordering::Relaxed),
            self.recent_costs[13].load(Ordering::Relaxed),
            self.recent_costs[14].load(Ordering::Relaxed),
            self.recent_costs[15].load(Ordering::Relaxed),
            self.recent_costs[16].load(Ordering::Relaxed),
            self.recent_costs[17].load(Ordering::Relaxed),
            self.recent_costs[18].load(Ordering::Relaxed),
            self.recent_costs[19].load(Ordering::Relaxed),
            self.recent_costs[20].load(Ordering::Relaxed),
            self.recent_costs[21].load(Ordering::Relaxed),
            self.recent_costs[22].load(Ordering::Relaxed),
            self.recent_costs[23].load(Ordering::Relaxed),
            self.recent_costs[24].load(Ordering::Relaxed),
            self.recent_costs[25].load(Ordering::Relaxed),
            self.recent_costs[26].load(Ordering::Relaxed),
            self.recent_costs[27].load(Ordering::Relaxed),
        ];

        let recent_costs_cents: [f64; 28] = [
            Self::from_q16_8(recent_costs_q16_8[0]),
            Self::from_q16_8(recent_costs_q16_8[1]),
            Self::from_q16_8(recent_costs_q16_8[2]),
            Self::from_q16_8(recent_costs_q16_8[3]),
            Self::from_q16_8(recent_costs_q16_8[4]),
            Self::from_q16_8(recent_costs_q16_8[5]),
            Self::from_q16_8(recent_costs_q16_8[6]),
            Self::from_q16_8(recent_costs_q16_8[7]),
            Self::from_q16_8(recent_costs_q16_8[8]),
            Self::from_q16_8(recent_costs_q16_8[9]),
            Self::from_q16_8(recent_costs_q16_8[10]),
            Self::from_q16_8(recent_costs_q16_8[11]),
            Self::from_q16_8(recent_costs_q16_8[12]),
            Self::from_q16_8(recent_costs_q16_8[13]),
            Self::from_q16_8(recent_costs_q16_8[14]),
            Self::from_q16_8(recent_costs_q16_8[15]),
            Self::from_q16_8(recent_costs_q16_8[16]),
            Self::from_q16_8(recent_costs_q16_8[17]),
            Self::from_q16_8(recent_costs_q16_8[18]),
            Self::from_q16_8(recent_costs_q16_8[19]),
            Self::from_q16_8(recent_costs_q16_8[20]),
            Self::from_q16_8(recent_costs_q16_8[21]),
            Self::from_q16_8(recent_costs_q16_8[22]),
            Self::from_q16_8(recent_costs_q16_8[23]),
            Self::from_q16_8(recent_costs_q16_8[24]),
            Self::from_q16_8(recent_costs_q16_8[25]),
            Self::from_q16_8(recent_costs_q16_8[26]),
            Self::from_q16_8(recent_costs_q16_8[27]),
        ];

        ForecastSnapshot {
            budget_id: self.budget_id.load(Ordering::Relaxed),
            daily_burn_rate_cents: Self::from_q16_8(self.trend_q16_8.load(Ordering::Relaxed)),
            mean_cost_cents: Self::from_q16_8(self.mean_cost_q16_8.load(Ordering::Relaxed)),
            std_dev_cents: Self::from_q16_8(self.std_dev_q16_8.load(Ordering::Relaxed)),
            anomaly_count: self.anomaly_count.load(Ordering::Relaxed),
            generation: self.generation.load(Ordering::Acquire),
            recent_costs_cents,
        }
    }

    /// Convert float cents to Q16.8 fixed-point
    ///
    /// # Format
    /// Q16.8 = value * 256
    /// Range: -128.00 to +127.996 cents
    /// Precision: 1/256 cent (~0.004 cents)
    fn to_q16_8(cents: f64) -> i32 {
        (cents * 256.0).round() as i32
    }

    /// Convert Q16.8 fixed-point to float cents
    fn from_q16_8(q16_8: i32) -> f64 {
        q16_8 as f64 / 256.0
    }
}

impl Default for CostForecast256 {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size() {
        assert_eq!(std::mem::size_of::<CostForecast256>(), 256);
    }

    #[test]
    fn test_capsule_alignment() {
        assert_eq!(std::mem::align_of::<CostForecast256>(), 64);
    }

    #[test]
    fn test_new() {
        let forecast = CostForecast256::new(123);

        let snapshot = forecast.snapshot();
        assert_eq!(snapshot.budget_id, 123);
        assert_eq!(snapshot.daily_burn_rate_cents, 0.0);
        assert_eq!(snapshot.mean_cost_cents, 0.0);
        assert_eq!(snapshot.anomaly_count, 0);
    }

    #[test]
    fn test_q16_8_conversion() {
        assert_eq!(CostForecast256::to_q16_8(0.0), 0);
        assert_eq!(CostForecast256::to_q16_8(1.0), 256);
        assert_eq!(CostForecast256::to_q16_8(10.5), 2688);
        assert_eq!(CostForecast256::to_q16_8(-5.25), -1344);

        assert!((CostForecast256::from_q16_8(0) - 0.0).abs() < 0.01);
        assert!((CostForecast256::from_q16_8(256) - 1.0).abs() < 0.01);
        assert!((CostForecast256::from_q16_8(2688) - 10.5).abs() < 0.01);
    }

    #[test]
    fn test_update_single() {
        let forecast = CostForecast256::new(123);

        forecast.update(10.0); // $0.10

        let snapshot = forecast.snapshot();
        assert_eq!(snapshot.generation, 1);
        assert!((snapshot.recent_costs_cents[27] - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_update_trend_positive() {
        let forecast = CostForecast256::new(123);

        // Positive trend: 1, 2, 3, ..., 28
        for day in 1..=28 {
            forecast.update(day as f64);
        }

        let snapshot = forecast.snapshot();

        // Mean = (1+2+...+28) / 28 = 14.5
        assert!((snapshot.mean_cost_cents - 14.5).abs() < 0.5);

        // Trend should be positive (slope ≈ 1.0)
        assert!(snapshot.daily_burn_rate_cents > 0.8);
        assert!(snapshot.daily_burn_rate_cents < 1.2);
    }

    #[test]
    fn test_update_trend_negative() {
        let forecast = CostForecast256::new(123);

        // Negative trend: 28, 27, 26, ..., 1
        for day in 1..=28 {
            forecast.update((29 - day) as f64);
        }

        let snapshot = forecast.snapshot();

        // Mean = (28+27+...+1) / 28 = 14.5
        assert!((snapshot.mean_cost_cents - 14.5).abs() < 0.5);

        // Trend should be negative (slope ≈ -1.0)
        assert!(snapshot.daily_burn_rate_cents < -0.8);
        assert!(snapshot.daily_burn_rate_cents > -1.2);
    }

    #[test]
    fn test_anomaly_detection() {
        let forecast = CostForecast256::new(123);

        // Establish baseline: 10.0 for 27 days
        for _ in 0..27 {
            forecast.update(10.0);
        }

        // Snapshot before anomaly
        let before = forecast.snapshot();
        assert_eq!(before.anomaly_count, 0);

        // Add anomaly: 100.0 (far above mean + 2σ)
        forecast.update(100.0);

        let after = forecast.snapshot();
        assert_eq!(after.anomaly_count, 1);
    }

    #[test]
    fn test_anomaly_no_false_positive() {
        let forecast = CostForecast256::new(123);

        // Establish baseline: 10.0 for 28 days
        for _ in 0..28 {
            forecast.update(10.0);
        }

        let snapshot = forecast.snapshot();
        assert_eq!(snapshot.anomaly_count, 0); // No false positives
    }

    #[test]
    fn test_concurrent_updates() {
        use std::sync::Arc;
        use std::thread;

        let forecast = Arc::new(CostForecast256::new(123));

        let mut handles = vec![];

        // 4 threads, each updates 7 times (total 28 days)
        for i in 0..4 {
            let f = Arc::clone(&forecast);
            handles.push(thread::spawn(move || {
                for day in 0..7 {
                    let cost = (i * 7 + day + 1) as f64;
                    f.update(cost);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let snapshot = forecast.snapshot();
        assert_eq!(snapshot.generation, 28);

        // Mean should be ~14.5
        assert!((snapshot.mean_cost_cents - 14.5).abs() < 1.0);
    }

    #[test]
    fn test_statistics_flat_trend() {
        let forecast = CostForecast256::new(123);

        // Flat trend: all 10.0
        for _ in 0..28 {
            forecast.update(10.0);
        }

        let snapshot = forecast.snapshot();

        // Mean = 10.0
        assert!((snapshot.mean_cost_cents - 10.0).abs() < 0.1);

        // Trend = 0 (flat)
        assert!(snapshot.daily_burn_rate_cents.abs() < 0.1);

        // Std dev = 0 (no variation)
        assert!(snapshot.std_dev_cents.abs() < 0.1);
    }
}
