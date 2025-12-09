//! Cost Comparison - Provider efficiency analysis
//!
//! Tier 3 (Fixed-Point) - Deterministic provider comparison with:
//! - Cost per success calculation (Q16.16 fixed-point)
//! - Provider efficiency ranking (multi-factor scoring)
//! - Latency-adjusted cost comparison
//! - Success rate normalization (basis points)
//!
//! UCE33 Q10: Fixed-point tier for deterministic cost arithmetic
//! UCE33 Q15: O(n) provider scan (bounded to 16 providers)
//! UCE33 Q22: Q16.16 for cost_per_success (no FP drift)
//! UCE33 Q30: Multi-factor scoring (cost + latency + success rate)

use crate::error::{ClapiError, ClapiResult};
use crate::metrics::query::{EpochStorage, ProviderComparison};
use std::sync::Arc;

/// Compare provider costs and efficiency
///
/// # Algorithm
/// 1. Aggregate provider metrics (cost, requests, errors, latency)
/// 2. Calculate cost per success (deterministic Q16.16)
/// 3. Compute efficiency score (weighted: 50% cost, 30% latency, 20% reliability)
/// 4. Rank providers by efficiency
///
/// # Performance
/// - O(n × p) where n = epochs, p = providers (bounded: p ≤ 16)
/// - Single-pass aggregation
/// - Q16.16 fixed-point for all cost calculations
///
/// # Safety
/// - #ASSUME: Cost per success is meaningful metric for comparison
/// - #VERIFY: Unit tests validate cost calculation correctness
/// - #ASSUME: Weighted scoring appropriate for efficiency
/// - #VERIFY: Property tests validate ranking consistency
pub fn compare_provider_costs(
    budget_id: u64,
    provider_id: Option<u64>,
    period_secs: u64,
    epoch_storage: Arc<dyn EpochStorage>,
) -> ClapiResult<Vec<ProviderComparison>> {
    // Get historical epochs for specified period
    let now_ms = current_timestamp_ms();
    let lookback_ms = period_secs * 1000;
    let from_ts = now_ms.saturating_sub(lookback_ms);

    let epochs = epoch_storage.get_epochs_for_budget(budget_id, from_ts, now_ms);

    if epochs.is_empty() {
        return Err(ClapiError::QueryError {
            message: "No data available for comparison".to_string(),
        });
    }

    // Aggregate provider metrics
    let mut provider_stats: std::collections::HashMap<u64, ProviderAggregates> =
        std::collections::HashMap::new();

    for epoch in &epochs {
        let snapshot = epoch.snapshot();

        for provider in &snapshot.providers {
            let stats = provider_stats.entry(provider.provider_id).or_insert_with(|| {
                ProviderAggregates::new(provider.provider_id)
            });

            stats.add_metrics(
                to_q16_16(provider.cost_cents),
                provider.request_count,
                provider.error_count,
                provider.latency_p99_us,
            );
        }
    }

    // Filter by provider_id if specified
    if let Some(pid) = provider_id {
        provider_stats.retain(|&k, _| k == pid);
    }

    if provider_stats.is_empty() {
        return Err(ClapiError::QueryError {
            message: "No provider data found".to_string(),
        });
    }

    // Calculate cost per success and efficiency scores
    let mut comparisons: Vec<ProviderComparison> = provider_stats
        .into_iter()
        .map(|(_, stats)| stats.compute_comparison())
        .collect();

    // Rank by efficiency score (composite metric)
    rank_providers(&mut comparisons);

    Ok(comparisons)
}

// ---- Provider Aggregates ----

/// Aggregated provider metrics (across multiple epochs)
struct ProviderAggregates {
    provider_id: u64,
    total_cost_q16_16: i64, // Q16.16 fixed-point
    total_requests: u64,
    total_errors: u64,
    max_latency_p99_us: u64,
}

impl ProviderAggregates {
    fn new(provider_id: u64) -> Self {
        Self {
            provider_id,
            total_cost_q16_16: 0,
            total_requests: 0,
            total_errors: 0,
            max_latency_p99_us: 0,
        }
    }

    /// Add metrics from one epoch
    fn add_metrics(
        &mut self,
        cost_q16_16: i64,
        requests: u64,
        errors: u64,
        latency_p99_us: u64,
    ) {
        self.total_cost_q16_16 = self.total_cost_q16_16.saturating_add(cost_q16_16);
        self.total_requests = self.total_requests.saturating_add(requests);
        self.total_errors = self.total_errors.saturating_add(errors);
        self.max_latency_p99_us = self.max_latency_p99_us.max(latency_p99_us);
    }

    /// Compute provider comparison metrics
    fn compute_comparison(self) -> ProviderComparison {
        // Success count
        let success_count = self.total_requests.saturating_sub(self.total_errors);

        // Success rate (basis points: 0-10000)
        let success_rate_bp = if self.total_requests > 0 {
            (success_count * 10000) / self.total_requests
        } else {
            0
        };

        // Cost per success (Q16.16 fixed-point)
        let cost_per_success_q16 = if success_count > 0 {
            self.total_cost_q16_16 / success_count as i64
        } else {
            i64::MAX // Infinite cost (no successes)
        };

        ProviderComparison {
            provider_id: self.provider_id,
            cost_cents: from_q16_16(self.total_cost_q16_16) as i64,
            request_count: self.total_requests,
            success_rate_bp,
            latency_p99_ns: self.max_latency_p99_us * 1000, // Convert μs to ns
            cost_per_success_cents: cost_per_success_q16,
            efficiency_rank: 0, // Will be set by ranking function
        }
    }
}

// ---- Efficiency Ranking ----

/// Rank providers by multi-factor efficiency score
///
/// # Scoring Formula
/// efficiency_score = w1 * cost_score + w2 * latency_score + w3 * reliability_score
///
/// Where:
/// - cost_score = normalized cost per success (lower is better)
/// - latency_score = normalized P99 latency (lower is better)
/// - reliability_score = success rate (higher is better)
/// - Weights: w1 = 0.5, w2 = 0.3, w3 = 0.2
///
/// # Normalization
/// All metrics normalized to [0, 1] using min-max scaling
fn rank_providers(comparisons: &mut [ProviderComparison]) {
    if comparisons.is_empty() {
        return;
    }

    // Find min/max for normalization
    let (min_cost, max_cost) = find_min_max(comparisons.iter().map(|c| c.cost_per_success_cents));
    let (min_latency, max_latency) = find_min_max(comparisons.iter().map(|c| c.latency_p99_ns as i64));

    // Compute efficiency scores
    let mut scores: Vec<(usize, f64)> = comparisons
        .iter()
        .enumerate()
        .map(|(idx, c)| {
            // Normalize cost (lower is better, so invert)
            let cost_score = if max_cost > min_cost {
                1.0 - normalize(c.cost_per_success_cents, min_cost, max_cost)
            } else {
                1.0
            };

            // Normalize latency (lower is better, so invert)
            let latency_score = if max_latency > min_latency {
                1.0 - normalize(c.latency_p99_ns as i64, min_latency, max_latency)
            } else {
                1.0
            };

            // Normalize success rate (higher is better)
            let reliability_score = c.success_rate_bp as f64 / 10000.0;

            // Weighted composite score
            let efficiency = 0.5 * cost_score + 0.3 * latency_score + 0.2 * reliability_score;

            (idx, efficiency)
        })
        .collect();

    // Sort by efficiency (descending: higher is better)
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // Assign ranks (1-based)
    for (rank, &(idx, _)) in scores.iter().enumerate() {
        comparisons[idx].efficiency_rank = rank + 1;
    }
}

/// Find min/max values
fn find_min_max<I>(mut iter: I) -> (i64, i64)
where
    I: Iterator<Item = i64>,
{
    let first = iter.next().unwrap_or(0);
    let (min, max) = iter.fold((first, first), |(min, max), val| {
        (min.min(val), max.max(val))
    });
    (min, max)
}

/// Normalize value to [0, 1] range
fn normalize(value: i64, min: i64, max: i64) -> f64 {
    if max == min {
        0.5 // All values equal
    } else {
        (value - min) as f64 / (max - min) as f64
    }
}

// ---- Advanced Comparison ----

/// Compare providers with latency-adjusted costs
///
/// Adjusts cost by latency penalty: adjusted_cost = cost × (1 + latency_penalty)
/// Where latency_penalty = (latency_p99 - median_latency) / median_latency
///
/// Use case: Prefer faster providers even if slightly more expensive
pub fn compare_with_latency_adjustment(
    budget_id: u64,
    period_secs: u64,
    epoch_storage: Arc<dyn EpochStorage>,
) -> ClapiResult<Vec<LatencyAdjustedComparison>> {
    let comparisons = compare_provider_costs(budget_id, None, period_secs, epoch_storage)?;

    if comparisons.is_empty() {
        return Ok(vec![]);
    }

    // Compute median latency
    let mut latencies: Vec<u64> = comparisons.iter().map(|c| c.latency_p99_ns).collect();
    latencies.sort_unstable();
    let median_latency = latencies[latencies.len() / 2];

    // Compute latency-adjusted costs
    let mut adjusted: Vec<LatencyAdjustedComparison> = comparisons
        .into_iter()
        .map(|c| {
            let latency_penalty = if median_latency > 0 {
                (c.latency_p99_ns as f64 - median_latency as f64) / median_latency as f64
            } else {
                0.0
            };

            let adjusted_cost_q16 = if latency_penalty >= 0.0 {
                // Penalty for slow providers
                c.cost_per_success_cents + (c.cost_per_success_cents as f64 * latency_penalty) as i64
            } else {
                // Bonus for fast providers (negative penalty)
                c.cost_per_success_cents + (c.cost_per_success_cents as f64 * latency_penalty) as i64
            };

            LatencyAdjustedComparison {
                provider_id: c.provider_id,
                raw_cost_cents: c.cost_per_success_cents,
                adjusted_cost_cents: adjusted_cost_q16,
                latency_p99_ns: c.latency_p99_ns,
                latency_penalty_percent: latency_penalty * 100.0,
                success_rate_bp: c.success_rate_bp,
            }
        })
        .collect();

    // Sort by adjusted cost (lower is better)
    adjusted.sort_by_key(|c| c.adjusted_cost_cents);

    Ok(adjusted)
}

/// Latency-adjusted comparison result
#[derive(Debug, Clone)]
pub struct LatencyAdjustedComparison {
    pub provider_id: u64,
    pub raw_cost_cents: i64,
    pub adjusted_cost_cents: i64,
    pub latency_p99_ns: u64,
    pub latency_penalty_percent: f64,
    pub success_rate_bp: u64,
}

// ---- Q16.16 Fixed-Point Helpers ----

const Q16_16_SCALE: i64 = 65536;

fn to_q16_16(cents: f64) -> i64 {
    (cents * Q16_16_SCALE as f64).round() as i64
}

fn from_q16_16(q16: i64) -> f64 {
    q16 as f64 / Q16_16_SCALE as f64
}

fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_aggregates() {
        let mut agg = ProviderAggregates::new(1);

        agg.add_metrics(to_q16_16(1.5), 100, 5, 50_000);
        agg.add_metrics(to_q16_16(2.5), 200, 10, 75_000);

        assert_eq!(agg.total_requests, 300);
        assert_eq!(agg.total_errors, 15);
        assert_eq!(agg.max_latency_p99_us, 75_000);

        let comparison = agg.compute_comparison();
        assert_eq!(comparison.success_rate_bp, 9500); // (285/300) * 10000 = 9500
    }

    #[test]
    fn test_cost_per_success() {
        let mut agg = ProviderAggregates::new(1);

        // $3.00 total cost, 100 requests, 10 errors = 90 successes
        agg.add_metrics(to_q16_16(3.0), 100, 10, 50_000);

        let comparison = agg.compute_comparison();

        // Cost per success = $3.00 / 90 ≈ $0.0333
        let cost_per_success = from_q16_16(comparison.cost_per_success_cents);
        assert!((cost_per_success - 0.0333).abs() < 0.001);
    }

    #[test]
    fn test_efficiency_ranking() {
        let mut comparisons = vec![
            ProviderComparison {
                provider_id: 1,
                cost_cents: 100,
                request_count: 1000,
                success_rate_bp: 9500,
                latency_p99_ns: 50_000_000, // 50ms
                cost_per_success_cents: to_q16_16(0.1),
                efficiency_rank: 0,
            },
            ProviderComparison {
                provider_id: 2,
                cost_cents: 80,
                request_count: 1000,
                success_rate_bp: 9000,
                latency_p99_ns: 100_000_000, // 100ms (slower)
                cost_per_success_cents: to_q16_16(0.08),
                efficiency_rank: 0,
            },
            ProviderComparison {
                provider_id: 3,
                cost_cents: 120,
                request_count: 1000,
                success_rate_bp: 9800,
                latency_p99_ns: 30_000_000, // 30ms (fastest)
                cost_per_success_cents: to_q16_16(0.12),
                efficiency_rank: 0,
            },
        ];

        rank_providers(&mut comparisons);

        // Provider 3 should be ranked highest (fastest + reliable, despite higher cost)
        // Provider 1 should be mid-tier (balanced)
        // Provider 2 should be lowest (slowest + lowest reliability)
        assert!(comparisons.iter().any(|c| c.provider_id == 3 && c.efficiency_rank == 1));
    }

    #[test]
    fn test_normalization() {
        assert_eq!(normalize(5, 0, 10), 0.5);
        assert_eq!(normalize(0, 0, 10), 0.0);
        assert_eq!(normalize(10, 0, 10), 1.0);
        assert_eq!(normalize(5, 5, 5), 0.5); // All equal
    }
}
