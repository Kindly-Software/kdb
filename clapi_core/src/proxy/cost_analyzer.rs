//! Cost Analyzer - Trend analysis and anomaly detection
//!
//! Provides:
//! - Daily cost trend analysis (linear regression)
//! - Anomaly detection (mean + 2σ threshold)
//! - Budget exhaustion prediction
//! - Alert generation
//!
//! Performance: <100ns forecast lookup, <1ms trend update
//!
//! UCE34 Q10: T4+T3 (Batch+Fixed-Point) via CostForecast256
//! UCE34 Q15: O(1) forecast lookup, O(28) trend update
//! UCE34 Q22: Q16.8 fixed-point for deterministic arithmetic

use crate::capsules::cost_forecast::{CostForecast256, ForecastSnapshot};
use crate::error::{ClapiError, ClapiResult};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Cost analyzer - manages forecasts for all budgets
pub struct CostAnalyzer {
    forecasts: RwLock<HashMap<u64, Arc<CostForecast256>>>,
}

/// Alert level for cost anomalies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertLevel {
    /// Normal operation (cost within 2σ)
    Normal,
    /// Warning: cost spike detected (> mean + 2σ)
    Warning,
    /// Critical: budget exhaustion imminent (< 7 days)
    Critical,
}

/// Cost analysis result
#[derive(Debug, Clone)]
pub struct CostAnalysis {
    pub budget_id: u64,
    pub daily_burn_rate_cents: f64,
    pub mean_cost_cents: f64,
    pub std_dev_cents: f64,
    pub anomaly_count: u32,
    pub alert_level: AlertLevel,
    pub alert_message: String,
    pub days_until_exhaustion: Option<f64>,
}

impl CostAnalyzer {
    /// Create new cost analyzer
    pub fn new() -> Self {
        Self {
            forecasts: RwLock::new(HashMap::new()),
        }
    }

    /// Update daily cost for a budget
    ///
    /// # Performance
    /// - <1ms per update (28-day window processing)
    /// - Lockfree capsule operations
    ///
    /// # Safety
    /// - RwLock only for HashMap, capsule is 100% lockfree
    pub fn update_daily_cost(&self, budget_id: u64, daily_cost_cents: f64) -> ClapiResult<()> {
        // Get or create forecast capsule
        let forecast = {
            let forecasts = self.forecasts.read().map_err(|e| ClapiError::QueryError {
                message: format!("Failed to acquire read lock: {}", e),
            })?;

            if let Some(f) = forecasts.get(&budget_id) {
                Arc::clone(f)
            } else {
                drop(forecasts);

                let mut forecasts = self.forecasts.write().map_err(|e| ClapiError::QueryError {
                    message: format!("Failed to acquire write lock: {}", e),
                })?;

                forecasts
                    .entry(budget_id)
                    .or_insert_with(|| Arc::new(CostForecast256::new(budget_id)))
                    .clone()
            }
        };

        // Update forecast (lockfree)
        forecast.update(daily_cost_cents);

        Ok(())
    }

    /// Analyze cost trends and detect anomalies
    ///
    /// # Performance
    /// - <100ns (lockfree snapshot read)
    ///
    /// # Returns
    /// - Cost analysis with alert level and message
    pub fn analyze_budget(&self, budget_id: u64, current_balance_cents: i64) -> ClapiResult<CostAnalysis> {
        let forecasts = self.forecasts.read().map_err(|e| ClapiError::QueryError {
            message: format!("Failed to acquire read lock: {}", e),
        })?;

        let forecast = forecasts
            .get(&budget_id)
            .ok_or_else(|| ClapiError::QueryError {
                message: format!("No forecast data for budget {}", budget_id),
            })?;

        let snapshot = forecast.snapshot();

        // Determine alert level
        let (alert_level, alert_message) = self.compute_alert(
            &snapshot,
            current_balance_cents,
        );

        // Predict days until exhaustion based on mean daily cost
        // (trend is the slope, mean is the actual average daily burn)
        let days_until_exhaustion = if snapshot.mean_cost_cents > 0.0 {
            Some(current_balance_cents as f64 / snapshot.mean_cost_cents)
        } else {
            None // Budget not being used
        };

        Ok(CostAnalysis {
            budget_id: snapshot.budget_id,
            daily_burn_rate_cents: snapshot.daily_burn_rate_cents,
            mean_cost_cents: snapshot.mean_cost_cents,
            std_dev_cents: snapshot.std_dev_cents,
            anomaly_count: snapshot.anomaly_count,
            alert_level,
            alert_message,
            days_until_exhaustion,
        })
    }

    /// Compute alert level and message
    ///
    /// # Alert Logic
    /// 1. **Critical**: Budget exhaustion < 7 days (based on mean daily cost)
    /// 2. **Warning**: Recent anomalies detected (last 7 days)
    /// 3. **Normal**: All good
    fn compute_alert(
        &self,
        snapshot: &ForecastSnapshot,
        current_balance_cents: i64,
    ) -> (AlertLevel, String) {
        // Check for budget exhaustion based on mean daily cost
        if snapshot.mean_cost_cents > 0.0 {
            let days_remaining = current_balance_cents as f64 / snapshot.mean_cost_cents;

            if days_remaining < 7.0 {
                return (
                    AlertLevel::Critical,
                    format!(
                        "CRITICAL: Budget exhaustion in {:.1} days (${:.2}/day avg cost). Immediate top-up required!",
                        days_remaining,
                        snapshot.mean_cost_cents / 100.0
                    ),
                );
            }

            if days_remaining < 30.0 {
                return (
                    AlertLevel::Warning,
                    format!(
                        "WARNING: Budget exhaustion in {:.1} days (${:.2}/day avg cost). Plan refill soon.",
                        days_remaining,
                        snapshot.mean_cost_cents / 100.0
                    ),
                );
            }
        }

        // Check for recent anomalies
        if snapshot.anomaly_count > 0 {
            // Check if recent costs (last 7 days) have anomalies
            let recent_costs = &snapshot.recent_costs_cents[21..28]; // Last 7 days
            let threshold = snapshot.mean_cost_cents + 2.0 * snapshot.std_dev_cents;

            let recent_anomalies = recent_costs.iter().filter(|&&c| c > threshold).count();

            if recent_anomalies > 0 {
                return (
                    AlertLevel::Warning,
                    format!(
                        "WARNING: {} cost spike(s) detected in last 7 days. Mean: ${:.2}, Current: ${:.2}",
                        recent_anomalies,
                        snapshot.mean_cost_cents / 100.0,
                        recent_costs.last().unwrap() / 100.0
                    ),
                );
            }
        }

        // Normal operation
        (
            AlertLevel::Normal,
            format!(
                "OK: Budget healthy. Daily burn rate: ${:.2}, Balance: ${:.2}",
                snapshot.daily_burn_rate_cents / 100.0,
                current_balance_cents as f64 / 100.0
            ),
        )
    }

    /// Get forecast snapshot for a budget
    pub fn get_forecast(&self, budget_id: u64) -> ClapiResult<ForecastSnapshot> {
        let forecasts = self.forecasts.read().map_err(|e| ClapiError::QueryError {
            message: format!("Failed to acquire read lock: {}", e),
        })?;

        let forecast = forecasts
            .get(&budget_id)
            .ok_or_else(|| ClapiError::QueryError {
                message: format!("No forecast data for budget {}", budget_id),
            })?;

        Ok(forecast.snapshot())
    }

    /// Get all budget IDs with forecasts
    pub fn get_all_budget_ids(&self) -> ClapiResult<Vec<u64>> {
        let forecasts = self.forecasts.read().map_err(|e| ClapiError::QueryError {
            message: format!("Failed to acquire read lock: {}", e),
        })?;

        Ok(forecasts.keys().copied().collect())
    }
}

impl Default for CostAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let analyzer = CostAnalyzer::new();
        let budget_ids = analyzer.get_all_budget_ids().unwrap();
        assert_eq!(budget_ids.len(), 0);
    }

    #[test]
    fn test_update_daily_cost() {
        let analyzer = CostAnalyzer::new();

        analyzer.update_daily_cost(123, 10.0).unwrap();
        analyzer.update_daily_cost(123, 12.0).unwrap();

        let forecast = analyzer.get_forecast(123).unwrap();
        assert_eq!(forecast.budget_id, 123);
    }

    #[test]
    fn test_analyze_normal() {
        let analyzer = CostAnalyzer::new();

        // Establish baseline: 10.0 for 28 days
        for _ in 0..28 {
            analyzer.update_daily_cost(123, 10.0).unwrap();
        }

        let analysis = analyzer.analyze_budget(123, 10000_00).unwrap(); // $10,000 balance

        assert_eq!(analysis.alert_level, AlertLevel::Normal);
        assert!((analysis.mean_cost_cents - 10.0).abs() < 0.5);
    }

    #[test]
    fn test_analyze_warning_anomaly() {
        let analyzer = CostAnalyzer::new();

        // Baseline: 10.0 for 27 days
        for _ in 0..27 {
            analyzer.update_daily_cost(123, 10.0).unwrap();
        }

        // Anomaly: 100.0 (spike)
        analyzer.update_daily_cost(123, 100.0).unwrap();

        let analysis = analyzer.analyze_budget(123, 10000_00).unwrap();

        assert_eq!(analysis.alert_level, AlertLevel::Warning);
        assert_eq!(analysis.anomaly_count, 1);
    }

    #[test]
    fn test_analyze_critical_exhaustion() {
        let analyzer = CostAnalyzer::new();

        // Establish mean cost: $100/day = 10,000 cents/day
        for _ in 0..28 {
            analyzer.update_daily_cost(123, 100_00.0).unwrap();
        }

        // Low balance: $500 (5 days remaining at $100/day)
        let analysis = analyzer.analyze_budget(123, 500_00).unwrap();

        assert_eq!(analysis.alert_level, AlertLevel::Critical);
        assert!(analysis.days_until_exhaustion.unwrap() < 7.0);
    }

    #[test]
    fn test_analyze_warning_low_balance() {
        let analyzer = CostAnalyzer::new();

        // Establish mean cost: $100/day = 10,000 cents/day
        for _ in 0..28 {
            analyzer.update_daily_cost(123, 100_00.0).unwrap();
        }

        // Medium balance: $2000 (20 days remaining at $100/day)
        let analysis = analyzer.analyze_budget(123, 2000_00).unwrap();

        assert_eq!(analysis.alert_level, AlertLevel::Warning);
        assert!(analysis.days_until_exhaustion.unwrap() < 30.0);
    }

    #[test]
    fn test_positive_trend() {
        let analyzer = CostAnalyzer::new();

        // Positive trend: 1, 2, 3, ..., 28
        for day in 1..=28 {
            analyzer.update_daily_cost(123, day as f64).unwrap();
        }

        let analysis = analyzer.analyze_budget(123, 10000_00).unwrap();

        // Trend should be positive
        assert!(analysis.daily_burn_rate_cents > 0.8);
    }

    #[test]
    fn test_multiple_budgets() {
        let analyzer = CostAnalyzer::new();

        analyzer.update_daily_cost(123, 10.0).unwrap();
        analyzer.update_daily_cost(456, 20.0).unwrap();
        analyzer.update_daily_cost(789, 30.0).unwrap();

        let budget_ids = analyzer.get_all_budget_ids().unwrap();
        assert_eq!(budget_ids.len(), 3);

        let analysis1 = analyzer.analyze_budget(123, 10000_00).unwrap();
        let analysis2 = analyzer.analyze_budget(456, 10000_00).unwrap();

        assert_eq!(analysis1.budget_id, 123);
        assert_eq!(analysis2.budget_id, 456);
    }
}
