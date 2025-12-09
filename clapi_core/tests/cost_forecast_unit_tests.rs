//! Cost Forecast Unit Tests (T28 Framework)
//!
//! Tier 1 (Unit Tests Q1-Q7):
//! - Capsule properties (size, alignment)
//! - Q16.8 fixed-point conversion
//! - Trend calculation (linear regression)
//! - Anomaly detection (mean + 2σ)
//! - Statistics computation (mean, std_dev)
//! - Concurrent updates
//! - Edge cases (flat trend, single update, all zeros)

use clapi_core::capsules::{CostForecast256, ForecastSnapshot};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Q1: Capsule Properties
// ============================================================================

#[test]
fn test_capsule_size() {
    assert_eq!(std::mem::size_of::<CostForecast256>(), 256);
}

#[test]
fn test_capsule_alignment() {
    assert_eq!(std::mem::align_of::<CostForecast256>(), 64);
}

// ============================================================================
// Q2: Q16.8 Fixed-Point Conversion
// ============================================================================

#[test]
fn test_q16_8_conversion_zero() {
    let forecast = CostForecast256::new(1);
    let snapshot = forecast.snapshot();
    assert_eq!(snapshot.daily_burn_rate_cents, 0.0);
}

#[test]
fn test_q16_8_conversion_positive() {
    let forecast = CostForecast256::new(1);
    forecast.update(10.5);

    let snapshot = forecast.snapshot();
    assert!((snapshot.recent_costs_cents[27] - 10.5).abs() < 0.01);
}

#[test]
fn test_q16_8_conversion_negative() {
    // Fixed-point can handle negative values
    let forecast = CostForecast256::new(1);
    forecast.update(-5.25);

    let snapshot = forecast.snapshot();
    assert!((snapshot.recent_costs_cents[27] + 5.25).abs() < 0.01);
}

#[test]
fn test_q16_8_precision() {
    let forecast = CostForecast256::new(1);
    forecast.update(0.004); // Minimum precision

    let snapshot = forecast.snapshot();
    assert!((snapshot.recent_costs_cents[27] - 0.004).abs() < 0.001);
}

// ============================================================================
// Q3: Trend Calculation (Linear Regression)
// ============================================================================

#[test]
fn test_trend_positive_linear() {
    let forecast = CostForecast256::new(1);

    // Perfect linear trend: 1, 2, 3, ..., 28
    for day in 1..=28 {
        forecast.update(day as f64);
    }

    let snapshot = forecast.snapshot();

    // Slope should be 1.0 (increase of 1 per day)
    assert!((snapshot.daily_burn_rate_cents - 1.0).abs() < 0.1);
}

#[test]
fn test_trend_negative_linear() {
    let forecast = CostForecast256::new(1);

    // Negative trend: 28, 27, 26, ..., 1
    for day in 1..=28 {
        forecast.update((29 - day) as f64);
    }

    let snapshot = forecast.snapshot();

    // Slope should be -1.0 (decrease of 1 per day)
    assert!((snapshot.daily_burn_rate_cents + 1.0).abs() < 0.1);
}

#[test]
fn test_trend_flat() {
    let forecast = CostForecast256::new(1);

    // Flat trend: all 10.0
    for _ in 0..28 {
        forecast.update(10.0);
    }

    let snapshot = forecast.snapshot();

    // Slope should be ~0 (no trend)
    assert!(snapshot.daily_burn_rate_cents.abs() < 0.1);
}

#[test]
fn test_trend_steep_positive() {
    let forecast = CostForecast256::new(1);

    // Steep trend: 0, 5, 10, 15, ..., 135
    for day in 0..28 {
        forecast.update((day * 5) as f64);
    }

    let snapshot = forecast.snapshot();

    // Slope should be ~5.0
    assert!((snapshot.daily_burn_rate_cents - 5.0).abs() < 0.5);
}

// ============================================================================
// Q4: Anomaly Detection (Mean + 2σ)
// ============================================================================

#[test]
fn test_anomaly_detection_spike() {
    let forecast = CostForecast256::new(1);

    // Baseline: 10.0 for 27 days
    for _ in 0..27 {
        forecast.update(10.0);
    }

    let before = forecast.snapshot();
    assert_eq!(before.anomaly_count, 0);

    // Spike: 100.0 (far above mean + 2σ)
    forecast.update(100.0);

    let after = forecast.snapshot();
    assert_eq!(after.anomaly_count, 1);
}

#[test]
fn test_anomaly_no_false_positive() {
    let forecast = CostForecast256::new(1);

    // All same value: 10.0
    for _ in 0..28 {
        forecast.update(10.0);
    }

    let snapshot = forecast.snapshot();
    assert_eq!(snapshot.anomaly_count, 0);
}

#[test]
fn test_anomaly_multiple_spikes() {
    let forecast = CostForecast256::new(1);

    // Baseline: 10.0 for 20 days
    for _ in 0..20 {
        forecast.update(10.0);
    }

    // Multiple spikes
    for _ in 20..28 {
        forecast.update(100.0);
    }

    let snapshot = forecast.snapshot();
    assert!(snapshot.anomaly_count >= 1); // At least 1 anomaly
}

#[test]
fn test_anomaly_gradual_increase_no_spike() {
    let forecast = CostForecast256::new(1);

    // Gradual increase: 10, 11, 12, ..., 37 (no sudden spike)
    for day in 0..28 {
        forecast.update((10 + day) as f64);
    }

    let snapshot = forecast.snapshot();
    // Gradual trend should not trigger many anomalies
    assert!(snapshot.anomaly_count < 5);
}

// ============================================================================
// Q5: Statistics Computation (Mean, Std Dev)
// ============================================================================

#[test]
fn test_mean_calculation() {
    let forecast = CostForecast256::new(1);

    // Simple case: all 10.0
    for _ in 0..28 {
        forecast.update(10.0);
    }

    let snapshot = forecast.snapshot();
    assert!((snapshot.mean_cost_cents - 10.0).abs() < 0.1);
}

#[test]
fn test_mean_calculation_varied() {
    let forecast = CostForecast256::new(1);

    // Varied costs: 1, 2, 3, ..., 28
    for day in 1..=28 {
        forecast.update(day as f64);
    }

    let snapshot = forecast.snapshot();
    // Mean = (1+2+...+28) / 28 = 14.5
    assert!((snapshot.mean_cost_cents - 14.5).abs() < 0.5);
}

#[test]
fn test_std_dev_zero_variation() {
    let forecast = CostForecast256::new(1);

    // Zero variation: all 10.0
    for _ in 0..28 {
        forecast.update(10.0);
    }

    let snapshot = forecast.snapshot();
    assert!(snapshot.std_dev_cents.abs() < 0.1); // ~0
}

#[test]
fn test_std_dev_high_variation() {
    let forecast = CostForecast256::new(1);

    // High variation: alternating 0, 100
    for day in 0..28 {
        let cost = if day % 2 == 0 { 0.0 } else { 100.0 };
        forecast.update(cost);
    }

    let snapshot = forecast.snapshot();
    assert!(snapshot.std_dev_cents > 40.0); // High std dev
}

// ============================================================================
// Q6: Concurrent Updates
// ============================================================================

#[test]
fn test_concurrent_updates_4_threads() {
    let forecast = Arc::new(CostForecast256::new(1));

    let mut handles = vec![];

    // 4 threads, each updates 7 times (total 28 updates)
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
fn test_concurrent_updates_8_threads() {
    let forecast = Arc::new(CostForecast256::new(1));

    let mut handles = vec![];

    // 8 threads, 100 updates each
    for _ in 0..8 {
        let f = Arc::clone(&forecast);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                f.update(10.0);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let snapshot = forecast.snapshot();
    assert_eq!(snapshot.generation, 800);

    // Mean should be ~10.0
    assert!((snapshot.mean_cost_cents - 10.0).abs() < 0.5);
}

// ============================================================================
// Q7: Edge Cases
// ============================================================================

#[test]
fn test_edge_case_single_update() {
    let forecast = CostForecast256::new(1);

    forecast.update(42.0);

    let snapshot = forecast.snapshot();
    assert_eq!(snapshot.generation, 1);
    assert!((snapshot.recent_costs_cents[27] - 42.0).abs() < 0.01);
}

#[test]
fn test_edge_case_zero_costs() {
    let forecast = CostForecast256::new(1);

    for _ in 0..28 {
        forecast.update(0.0);
    }

    let snapshot = forecast.snapshot();
    assert_eq!(snapshot.mean_cost_cents, 0.0);
    assert_eq!(snapshot.std_dev_cents, 0.0);
    assert_eq!(snapshot.daily_burn_rate_cents, 0.0);
}

#[test]
fn test_edge_case_large_costs() {
    let forecast = CostForecast256::new(1);

    // Large costs within Q16.8 range (-128 to +127)
    for _ in 0..28 {
        forecast.update(100.0);
    }

    let snapshot = forecast.snapshot();
    assert!((snapshot.mean_cost_cents - 100.0).abs() < 0.5);
}

#[test]
fn test_edge_case_mixed_positive_negative() {
    let forecast = CostForecast256::new(1);

    // Mixed positive and negative
    for day in 0..28 {
        let cost = if day % 2 == 0 { 10.0 } else { -5.0 };
        forecast.update(cost);
    }

    let snapshot = forecast.snapshot();
    // Mean should be (10 - 5) / 2 = 2.5
    assert!((snapshot.mean_cost_cents - 2.5).abs() < 1.0);
}

#[test]
fn test_window_shift_correctness() {
    let forecast = CostForecast256::new(1);

    // Add 30 updates (more than window size)
    for day in 1..=30 {
        forecast.update(day as f64);
    }

    let snapshot = forecast.snapshot();

    // Window should contain last 28 values: 3, 4, 5, ..., 30
    assert!((snapshot.recent_costs_cents[0] - 3.0).abs() < 0.1);
    assert!((snapshot.recent_costs_cents[27] - 30.0).abs() < 0.1);
}

#[test]
fn test_generation_counter_increments() {
    let forecast = CostForecast256::new(1);

    for i in 1..=10 {
        forecast.update(10.0);
        let snapshot = forecast.snapshot();
        assert_eq!(snapshot.generation, i);
    }
}

#[test]
fn test_budget_id_preserved() {
    let forecast = CostForecast256::new(12345);

    forecast.update(10.0);

    let snapshot = forecast.snapshot();
    assert_eq!(snapshot.budget_id, 12345);
}

// ============================================================================
// Additional Property Tests (T28 Q8-Q14)
// ============================================================================

#[test]
fn test_property_mean_bounds() {
    let forecast = CostForecast256::new(1);

    // All costs between 10 and 20
    for day in 0..28 {
        forecast.update(10.0 + (day % 11) as f64);
    }

    let snapshot = forecast.snapshot();
    // Mean should be between min and max
    assert!(snapshot.mean_cost_cents >= 10.0);
    assert!(snapshot.mean_cost_cents <= 20.0);
}

#[test]
fn test_property_std_dev_non_negative() {
    let forecast = CostForecast256::new(1);

    for _ in 0..28 {
        forecast.update(10.0);
    }

    let snapshot = forecast.snapshot();
    assert!(snapshot.std_dev_cents >= 0.0); // Std dev always non-negative
}

#[test]
fn test_property_anomaly_count_monotonic() {
    let forecast = CostForecast256::new(1);

    // Baseline
    for _ in 0..20 {
        forecast.update(10.0);
    }

    let before = forecast.snapshot().anomaly_count;

    // Add anomaly
    forecast.update(100.0);

    let after = forecast.snapshot().anomaly_count;
    assert!(after >= before); // Anomaly count only increases
}
