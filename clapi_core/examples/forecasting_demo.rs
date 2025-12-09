//! Forecasting Demo - Cost Prediction & Confidence Intervals
//!
//! Demonstrates forecasting capabilities using historical metrics:
//! - Load cost history from capsules
//! - Run statistical forecasting (linear regression, moving averages)
//! - Display confidence intervals (p50, p90, p95, p99)
//! - Generate budget recommendations
//!
//! # Statistical Methods
//! - Simple Moving Average (SMA): Short-term trends
//! - Exponential Weighted Moving Average (EWMA): Adaptive trends
//! - Linear Regression: Long-term projections
//! - Percentile Analysis: Tail behavior (p50/p90/p95/p99)
//!
//! # Usage
//! ```bash
//! cargo run --example forecasting_demo
//! ```

use clapi_core::capsules::{RequestCapsule128Enhanced, EnhancedMetrics};
use std::collections::VecDeque;

/// Cost data point for forecasting
#[derive(Debug, Clone, Copy)]
struct CostDataPoint {
    timestamp_ns: u64,
    cost_cents: i64,
    cumulative_cost: i64,
}

/// Forecast result with confidence intervals
#[derive(Debug)]
struct ForecastResult {
    period_days: u32,
    predicted_cost_cents: i64,
    confidence_p50: i64, // Median (50th percentile)
    confidence_p90: i64, // 90th percentile
    confidence_p95: i64, // 95th percentile
    confidence_p99: i64, // 99th percentile
    method: &'static str,
}

/// Budget recommendation based on forecast
#[derive(Debug)]
struct BudgetRecommendation {
    current_budget: i64,
    recommended_budget: i64,
    confidence_level: &'static str, // p50, p90, p95, p99
    forecast_period_days: u32,
    rationale: String,
}

fn main() {
    println!("=== Forecasting Demo ===\n");

    // Section 1: Generate Historical Data
    let history = generate_historical_data();
    println!("1. Generated {} days of historical cost data\n", history.len());

    // Section 2: Statistical Analysis
    statistical_analysis(&history);

    // Section 3: Simple Moving Average (SMA)
    sma_forecasting(&history);

    // Section 4: Exponential Weighted Moving Average (EWMA)
    ewma_forecasting(&history);

    // Section 5: Linear Regression
    linear_regression_forecasting(&history);

    // Section 6: Percentile Analysis
    percentile_analysis(&history);

    // Section 7: Budget Recommendations
    budget_recommendations(&history);

    println!("\n=== Example Complete ===");
}

/// Generate 30 days of simulated cost history
fn generate_historical_data() -> Vec<CostDataPoint> {
    let mut history = Vec::new();
    let mut cumulative = 0i64;

    // Base cost: $50-100 per day with trend and variance
    for day in 0..30 {
        // Linear trend: increases $2 per day
        let trend = 50_00 + (day * 2_00);

        // Weekly cycle: higher on weekdays
        let weekday_factor = if day % 7 < 5 { 1.2 } else { 0.8 };

        // Random variance: ±20%
        let variance = ((day * 17 + 42) % 40) - 20; // Pseudo-random [-20, 20]
        let variance_factor = 1.0 + (variance as f64 / 100.0);

        let daily_cost = ((trend as f64 * weekday_factor * variance_factor) as i64).max(10_00);
        cumulative += daily_cost;

        history.push(CostDataPoint {
            timestamp_ns: day as u64 * 24 * 3600 * 1_000_000_000, // Days to nanoseconds
            cost_cents: daily_cost,
            cumulative_cost: cumulative,
        });
    }

    history
}

/// Section 2: Statistical Analysis
fn statistical_analysis(history: &[CostDataPoint]) {
    println!("=== 2. Statistical Analysis ===\n");

    let costs: Vec<i64> = history.iter().map(|p| p.cost_cents).collect();

    // Mean
    let mean = costs.iter().sum::<i64>() / costs.len() as i64;

    // Median (p50)
    let mut sorted_costs = costs.clone();
    sorted_costs.sort();
    let median = sorted_costs[sorted_costs.len() / 2];

    // Min/Max
    let min = *sorted_costs.first().unwrap();
    let max = *sorted_costs.last().unwrap();

    // Standard deviation
    let variance: f64 = costs.iter()
        .map(|c| {
            let diff = (*c as f64 - mean as f64);
            diff * diff
        })
        .sum::<f64>() / costs.len() as f64;
    let std_dev = variance.sqrt();

    // Total cost
    let total_cost = history.last().unwrap().cumulative_cost;

    println!("2.1 Descriptive Statistics:");
    println!("   Period:           {} days", history.len());
    println!("   Total Cost:       ${:.2}", total_cost as f64 / 100.0);
    println!("   Mean Daily Cost:  ${:.2}", mean as f64 / 100.0);
    println!("   Median (p50):     ${:.2}", median as f64 / 100.0);
    println!("   Std Deviation:    ${:.2}", std_dev / 100.0);
    println!("   Min Daily Cost:   ${:.2}", min as f64 / 100.0);
    println!("   Max Daily Cost:   ${:.2}", max as f64 / 100.0);

    println!("\n2.2 Distribution:");
    let range = max - min;
    let bin_width = range / 5;
    for i in 0..5 {
        let bin_start = min + (i * bin_width);
        let bin_end = bin_start + bin_width;
        let count = costs.iter().filter(|&&c| c >= bin_start && c < bin_end).count();
        let bar = "█".repeat(count);
        println!("   ${:.2}-${:.2}: {}",
            bin_start as f64 / 100.0,
            bin_end as f64 / 100.0,
            bar);
    }

    println!("\n");
}

/// Section 3: Simple Moving Average (SMA)
fn sma_forecasting(history: &[CostDataPoint]) {
    println!("=== 3. Simple Moving Average (SMA) Forecasting ===\n");

    let window_sizes = vec![7, 14, 30];

    for window in window_sizes {
        if history.len() < window {
            continue;
        }

        // Calculate SMA for last N days
        let recent_costs: Vec<i64> = history.iter()
            .rev()
            .take(window)
            .map(|p| p.cost_cents)
            .collect();

        let sma = recent_costs.iter().sum::<i64>() / recent_costs.len() as i64;

        // Project 7 days forward
        let forecast_days = 7;
        let forecast_cost = sma * forecast_days;

        // Calculate confidence intervals (assuming normal distribution)
        let std_dev = calculate_std_dev(&recent_costs, sma);
        let confidence_p90 = (sma as f64 + 1.28 * std_dev) as i64 * forecast_days; // 90th percentile
        let confidence_p95 = (sma as f64 + 1.645 * std_dev) as i64 * forecast_days; // 95th percentile
        let confidence_p99 = (sma as f64 + 2.326 * std_dev) as i64 * forecast_days; // 99th percentile

        let result = ForecastResult {
            period_days: forecast_days as u32,
            predicted_cost_cents: forecast_cost,
            confidence_p50: forecast_cost,
            confidence_p90,
            confidence_p95,
            confidence_p99,
            method: &format!("SMA-{}", window),
        };

        print_forecast_result(&result, window);
    }

    println!("\n");
}

/// Section 4: Exponential Weighted Moving Average (EWMA)
fn ewma_forecasting(history: &[CostDataPoint]) {
    println!("=== 4. Exponential Weighted Moving Average (EWMA) ===\n");

    let alphas = vec![0.1, 0.3, 0.5]; // Smoothing factors

    for alpha in alphas {
        // Calculate EWMA
        let mut ewma = history[0].cost_cents as f64;
        for point in history.iter().skip(1) {
            ewma = alpha * point.cost_cents as f64 + (1.0 - alpha) * ewma;
        }

        // Project 7 days forward
        let forecast_days = 7;
        let forecast_cost = (ewma * forecast_days as f64) as i64;

        // Calculate recent volatility for confidence intervals
        let recent_costs: Vec<i64> = history.iter()
            .rev()
            .take(14)
            .map(|p| p.cost_cents)
            .collect();
        let std_dev = calculate_std_dev(&recent_costs, ewma as i64);

        let confidence_p90 = (ewma + 1.28 * std_dev) as i64 * forecast_days;
        let confidence_p95 = (ewma + 1.645 * std_dev) as i64 * forecast_days;
        let confidence_p99 = (ewma + 2.326 * std_dev) as i64 * forecast_days;

        let result = ForecastResult {
            period_days: forecast_days as u32,
            predicted_cost_cents: forecast_cost,
            confidence_p50: forecast_cost,
            confidence_p90,
            confidence_p95,
            confidence_p99,
            method: &format!("EWMA-α={:.1}", alpha),
        };

        println!("4.1 EWMA Forecast (α={:.1}):", alpha);
        println!("   Recent EWMA:      ${:.2}", ewma / 100.0);
        println!("   7-day Forecast:");
        println!("     p50 (median):   ${:.2}", forecast_cost as f64 / 100.0);
        println!("     p90:            ${:.2}", confidence_p90 as f64 / 100.0);
        println!("     p95:            ${:.2}", confidence_p95 as f64 / 100.0);
        println!("     p99:            ${:.2}", confidence_p99 as f64 / 100.0);
        println!("   Interpretation:   α={:.1} → {}",
            alpha,
            if alpha >= 0.5 { "Responsive to recent changes" }
            else if alpha >= 0.3 { "Balanced trend tracking" }
            else { "Smooth long-term trend" });
        println!();
    }

    println!();
}

/// Section 5: Linear Regression Forecasting
fn linear_regression_forecasting(history: &[CostDataPoint]) {
    println!("=== 5. Linear Regression Forecasting ===\n");

    // Calculate linear regression: y = mx + b
    let n = history.len() as f64;
    let sum_x: f64 = (0..history.len()).map(|i| i as f64).sum();
    let sum_y: f64 = history.iter().map(|p| p.cost_cents as f64).sum();
    let sum_xy: f64 = history.iter()
        .enumerate()
        .map(|(i, p)| i as f64 * p.cost_cents as f64)
        .sum();
    let sum_x2: f64 = (0..history.len()).map(|i| (i * i) as f64).sum();

    let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);
    let intercept = (sum_y - slope * sum_x) / n;

    println!("5.1 Linear Regression Model:");
    println!("   y = mx + b");
    println!("   Slope (m):        ${:.4} per day", slope / 100.0);
    println!("   Intercept (b):    ${:.2}", intercept / 100.0);

    // Calculate R² (coefficient of determination)
    let mean_y = sum_y / n;
    let ss_tot: f64 = history.iter()
        .map(|p| {
            let diff = p.cost_cents as f64 - mean_y;
            diff * diff
        })
        .sum();
    let ss_res: f64 = history.iter()
        .enumerate()
        .map(|(i, p)| {
            let predicted = slope * i as f64 + intercept;
            let diff = p.cost_cents as f64 - predicted;
            diff * diff
        })
        .sum();
    let r_squared = 1.0 - (ss_res / ss_tot);

    println!("   R² (fit quality): {:.4} ({})",
        r_squared,
        if r_squared >= 0.8 { "excellent" }
        else if r_squared >= 0.6 { "good" }
        else if r_squared >= 0.4 { "moderate" }
        else { "poor" });

    // Project 7, 14, 30 days forward
    println!("\n5.2 Forecasts:");
    for forecast_days in &[7, 14, 30] {
        let x = (history.len() + forecast_days - 1) as f64;
        let predicted = (slope * x + intercept) as i64;
        let daily_avg = predicted / *forecast_days as i64;

        // Calculate prediction interval (±1.96 * std error for 95%)
        let residuals: Vec<f64> = history.iter()
            .enumerate()
            .map(|(i, p)| {
                let pred = slope * i as f64 + intercept;
                p.cost_cents as f64 - pred
            })
            .collect();
        let std_error = calculate_std_dev_f64(&residuals, 0.0);
        let margin_p95 = 1.96 * std_error * (*forecast_days as f64).sqrt();

        println!("   {}-day Forecast:", forecast_days);
        println!("     Total Cost:     ${:.2}", predicted as f64 / 100.0);
        println!("     Daily Average:  ${:.2}", daily_avg as f64 / 100.0);
        println!("     95% CI:         ${:.2} ± ${:.2}",
            predicted as f64 / 100.0,
            margin_p95 / 100.0);
    }

    println!("\n");
}

/// Section 6: Percentile Analysis
fn percentile_analysis(history: &[CostDataPoint]) {
    println!("=== 6. Percentile Analysis ===\n");

    let mut costs: Vec<i64> = history.iter().map(|p| p.cost_cents).collect();
    costs.sort();

    let percentiles = vec![
        (50, "p50 (median)"),
        (75, "p75"),
        (90, "p90"),
        (95, "p95"),
        (99, "p99"),
    ];

    println!("6.1 Daily Cost Percentiles:");
    for (p, label) in percentiles {
        let index = (costs.len() * p / 100).min(costs.len() - 1);
        let value = costs[index];
        println!("   {}: ${:.2}", label, value as f64 / 100.0);
    }

    // Tail risk analysis
    println!("\n6.2 Tail Risk Analysis:");
    let p50 = costs[costs.len() / 2];
    let p90 = costs[costs.len() * 90 / 100];
    let p99 = costs[costs.len() * 99 / 100];

    println!("   p90 / p50 ratio:  {:.2}× ({})",
        p90 as f64 / p50 as f64,
        if p90 as f64 / p50 as f64 > 2.0 { "high variance" } else { "low variance" });
    println!("   p99 / p90 ratio:  {:.2}× ({})",
        p99 as f64 / p90 as f64,
        if p99 as f64 / p90 as f64 > 1.5 { "heavy tail" } else { "light tail" });

    println!("\n6.3 Budget Recommendations by Percentile:");
    for (p, label) in &[(50, "p50 (median)"), (90, "p90"), (95, "p95"), (99, "p99")] {
        let index = (costs.len() * p / 100).min(costs.len() - 1);
        let daily_cost = costs[index];
        let monthly_budget = daily_cost * 30;
        println!("   {}: ${:.2}/month (${:.2}/day)",
            label,
            monthly_budget as f64 / 100.0,
            daily_cost as f64 / 100.0);
    }

    println!("\n");
}

/// Section 7: Budget Recommendations
fn budget_recommendations(history: &[CostDataPoint]) {
    println!("=== 7. Budget Recommendations ===\n");

    let current_budget = 2500_00; // $2500 current budget

    // Calculate recent trend (last 7 days)
    let recent_costs: Vec<i64> = history.iter()
        .rev()
        .take(7)
        .map(|p| p.cost_cents)
        .collect();
    let recent_avg = recent_costs.iter().sum::<i64>() / recent_costs.len() as i64;

    // Calculate percentiles for 30-day projection
    let mut costs: Vec<i64> = history.iter().map(|p| p.cost_cents).collect();
    costs.sort();
    let p50_daily = costs[costs.len() / 2];
    let p90_daily = costs[costs.len() * 90 / 100];
    let p95_daily = costs[costs.len() * 95 / 100];
    let p99_daily = costs[costs.len() * 99 / 100];

    let recommendations = vec![
        BudgetRecommendation {
            current_budget,
            recommended_budget: p50_daily * 30,
            confidence_level: "p50 (median)",
            forecast_period_days: 30,
            rationale: "50% chance of staying under budget. Optimistic scenario.".to_string(),
        },
        BudgetRecommendation {
            current_budget,
            recommended_budget: p90_daily * 30,
            confidence_level: "p90",
            forecast_period_days: 30,
            rationale: "90% chance of staying under budget. Recommended for production.".to_string(),
        },
        BudgetRecommendation {
            current_budget,
            recommended_budget: p95_daily * 30,
            confidence_level: "p95",
            forecast_period_days: 30,
            rationale: "95% chance of staying under budget. Conservative approach.".to_string(),
        },
        BudgetRecommendation {
            current_budget,
            recommended_budget: p99_daily * 30,
            confidence_level: "p99",
            forecast_period_days: 30,
            rationale: "99% chance of staying under budget. Maximum safety margin.".to_string(),
        },
    ];

    println!("7.1 Current Budget: ${:.2}\n", current_budget as f64 / 100.0);

    for (i, rec) in recommendations.iter().enumerate() {
        println!("7.{} Recommendation ({}):", i + 2, rec.confidence_level);
        println!("   Recommended Budget: ${:.2}/month", rec.recommended_budget as f64 / 100.0);
        println!("   Daily Equivalent:   ${:.2}/day", (rec.recommended_budget / 30) as f64 / 100.0);

        let delta = rec.recommended_budget - current_budget;
        let delta_pct = (delta as f64 / current_budget as f64) * 100.0;
        if delta > 0 {
            println!("   Budget Increase:    +${:.2} ({:+.1}%)",
                delta as f64 / 100.0,
                delta_pct);
        } else {
            println!("   Budget Decrease:    -${:.2} ({:+.1}%)",
                (-delta) as f64 / 100.0,
                delta_pct);
        }

        println!("   Rationale:          {}", rec.rationale);
        println!();
    }

    println!("7.6 Final Recommendation:");
    println!("   Use p90 budget: ${:.2}/month", (p90_daily * 30) as f64 / 100.0);
    println!("   Rationale: Balances cost control with 90% confidence.");
    println!("   Action: {} current budget",
        if (p90_daily * 30) > current_budget { "Increase" } else { "Maintain" });

    println!("\n");
}

/// Helper: Print forecast result
fn print_forecast_result(result: &ForecastResult, window: usize) {
    println!("3.{} SMA-{} Forecast ({}-day window):", window / 7, window, window);
    println!("   Forecast Period:  {} days", result.period_days);
    println!("   Predicted Cost:");
    println!("     p50 (median):   ${:.2}", result.predicted_cost_cents as f64 / 100.0);
    println!("     p90:            ${:.2}", result.confidence_p90 as f64 / 100.0);
    println!("     p95:            ${:.2}", result.confidence_p95 as f64 / 100.0);
    println!("     p99:            ${:.2}", result.confidence_p99 as f64 / 100.0);
    println!();
}

/// Helper: Calculate standard deviation
fn calculate_std_dev(values: &[i64], mean: i64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let variance: f64 = values.iter()
        .map(|v| {
            let diff = (*v as f64 - mean as f64);
            diff * diff
        })
        .sum::<f64>() / values.len() as f64;

    variance.sqrt()
}

/// Helper: Calculate standard deviation (f64)
fn calculate_std_dev_f64(values: &[f64], mean: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let variance: f64 = values.iter()
        .map(|v| {
            let diff = v - mean;
            diff * diff
        })
        .sum::<f64>() / values.len() as f64;

    variance.sqrt()
}
