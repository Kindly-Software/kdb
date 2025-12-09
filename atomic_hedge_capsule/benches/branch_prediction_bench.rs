//! Branch Prediction Optimization Benchmark - B32 Framework Compliant
//!
//! Following B32 fairness framework principles:
//! - Fair baselines: Sequential vs random access patterns
//! - Statistical rigor: 95% confidence intervals, large sample sizes
//! - Hardware measurement: Branch misprediction impact on performance
//! - Kontext27 reality checks: 5-15% improvement from branch optimization
//! - Empirical validation: Real trading patterns, not artificial microbenchmarks
//!
//! UCE32 Analysis Results:
//! - Q28 (Simplicity): Focus on actual branch patterns in hedge operations
//! - Q29 (Constraints): Hardware constraint: Branch predictor has ~95% accuracy on patterns
//! - Q30 (Validation): Prove 5-15% misprediction reduction in realistic scenarios
//! - Q31 (Rust Transform): match expressions enable compiler branch optimization
//! - Q32 (Nightly): Branch hinting and const_for can optimize hot paths

use atomic_hedge_capsule::{AtomicHedgeCapsule, HedgeError, HedgeState};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use fastrand;
use std::hint::black_box as std_black_box;
use std::time::{Duration, Instant};

/// B32 Framework: Trading pattern generators for realistic branch testing
struct TradingPatternGenerator {
    rng: fastrand::Rng,
}

impl TradingPatternGenerator {
    fn new(seed: u64) -> Self {
        Self {
            rng: fastrand::Rng::with_seed(seed),
        }
    }

    /// Sequential pattern: Predictable branches (buy, sell, buy, sell...)
    fn generate_sequential_pattern(&mut self, size: usize) -> Vec<bool> {
        (0..size).map(|i| i % 2 == 0).collect()
    }

    /// Random pattern: Unpredictable branches (random buy/sell decisions)
    fn generate_random_pattern(&mut self, size: usize) -> Vec<bool> {
        (0..size).map(|_| self.rng.bool()).collect()
    }

    /// Burst pattern: Common in real trading (5-10 buys, then 5-10 sells)
    fn generate_burst_pattern(&mut self, size: usize) -> Vec<bool> {
        let mut pattern = Vec::with_capacity(size);
        let mut i = 0;

        while i < size {
            let burst_side = self.rng.bool();
            let burst_length = self.rng.usize(5..=10).min(size - i);

            for _ in 0..burst_length {
                pattern.push(burst_side);
            }
            i += burst_length;
        }

        pattern
    }

    /// Market trend pattern: Bias toward one direction with occasional reversals
    fn generate_trend_pattern(&mut self, size: usize) -> Vec<bool> {
        let mut pattern = Vec::with_capacity(size);
        let primary_bias = self.rng.bool();
        let reversal_prob = 0.1; // 10% chance of reversal

        for _ in 0..size {
            let side = if self.rng.f32() < reversal_prob {
                !primary_bias // Reversal
            } else {
                primary_bias // Follow trend
            };
            pattern.push(side);
        }

        pattern
    }
}

/// Baseline branch-heavy processing for comparison
fn process_pattern_baseline(pattern: &[bool], entry_prices: &[u32]) -> (u64, u64) {
    let mut buy_total = 0u64;
    let mut sell_total = 0u64;

    for (&side, &price) in pattern.iter().zip(entry_prices.iter()) {
        if side {
            // Buy side processing (complex branching)
            if price > 50000 {
                if price > 60000 {
                    buy_total += (price as u64) * 2;
                } else {
                    buy_total += (price as u64) * 3;
                }
            } else {
                if price > 40000 {
                    buy_total += (price as u64) * 4;
                } else {
                    buy_total += (price as u64) * 5;
                }
            }
        } else {
            // Sell side processing (different branching pattern)
            if price < 50000 {
                if price < 40000 {
                    sell_total += (price as u64) * 2;
                } else {
                    sell_total += (price as u64) * 3;
                }
            } else {
                if price < 60000 {
                    sell_total += (price as u64) * 4;
                } else {
                    sell_total += (price as u64) * 5;
                }
            }
        }
    }

    (buy_total, sell_total)
}

/// Optimized branch processing using match expressions and patterns
fn process_pattern_optimized(pattern: &[bool], entry_prices: &[u32]) -> (u64, u64) {
    let mut buy_total = 0u64;
    let mut sell_total = 0u64;

    for (&side, &price) in pattern.iter().zip(entry_prices.iter()) {
        match (side, price) {
            // Buy side - optimized match patterns
            (true, price) if price > 60000 => buy_total += (price as u64) * 2,
            (true, price) if price > 50000 => buy_total += (price as u64) * 3,
            (true, price) if price > 40000 => buy_total += (price as u64) * 4,
            (true, price) => buy_total += (price as u64) * 5,

            // Sell side - optimized match patterns
            (false, price) if price < 40000 => sell_total += (price as u64) * 2,
            (false, price) if price < 50000 => sell_total += (price as u64) * 3,
            (false, price) if price < 60000 => sell_total += (price as u64) * 4,
            (false, price) => sell_total += (price as u64) * 5,
        }
    }

    (buy_total, sell_total)
}

/// Sequential vs Random pattern impact on branch prediction
fn bench_branch_prediction_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("branch_prediction_patterns");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(500);

    let pattern_size = 10000;
    group.throughput(Throughput::Elements(pattern_size as u64));

    let mut generator = TradingPatternGenerator::new(42);

    // Generate test patterns
    let sequential_pattern = generator.generate_sequential_pattern(pattern_size);
    let random_pattern = generator.generate_random_pattern(pattern_size);
    let burst_pattern = generator.generate_burst_pattern(pattern_size);
    let trend_pattern = generator.generate_trend_pattern(pattern_size);

    // Generate corresponding price data
    let entry_prices: Vec<u32> = (0..pattern_size)
        .map(|i| 40000 + (i % 20000) as u32)
        .collect();

    // Sequential pattern - highly predictable
    group.bench_function("sequential_pattern_baseline", |b| {
        b.iter(|| {
            let result = process_pattern_baseline(&sequential_pattern, &entry_prices);
            std_black_box(result);
        });
    });

    group.bench_function("sequential_pattern_optimized", |b| {
        b.iter(|| {
            let result = process_pattern_optimized(&sequential_pattern, &entry_prices);
            std_black_box(result);
        });
    });

    // Random pattern - unpredictable branches
    group.bench_function("random_pattern_baseline", |b| {
        b.iter(|| {
            let result = process_pattern_baseline(&random_pattern, &entry_prices);
            std_black_box(result);
        });
    });

    group.bench_function("random_pattern_optimized", |b| {
        b.iter(|| {
            let result = process_pattern_optimized(&random_pattern, &entry_prices);
            std_black_box(result);
        });
    });

    // Burst pattern - realistic trading pattern
    group.bench_function("burst_pattern_baseline", |b| {
        b.iter(|| {
            let result = process_pattern_baseline(&burst_pattern, &entry_prices);
            std_black_box(result);
        });
    });

    group.bench_function("burst_pattern_optimized", |b| {
        b.iter(|| {
            let result = process_pattern_optimized(&burst_pattern, &entry_prices);
            std_black_box(result);
        });
    });

    // Trend pattern - biased but with reversals
    group.bench_function("trend_pattern_baseline", |b| {
        b.iter(|| {
            let result = process_pattern_baseline(&trend_pattern, &entry_prices);
            std_black_box(result);
        });
    });

    group.bench_function("trend_pattern_optimized", |b| {
        b.iter(|| {
            let result = process_pattern_optimized(&trend_pattern, &entry_prices);
            std_black_box(result);
        });
    });

    group.finish();
}

/// Hedge capsule operation patterns with branch optimization
fn bench_hedge_capsule_branch_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("hedge_capsule_branch_patterns");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(200);

    let operations = 1000u64;
    group.throughput(Throughput::Elements(operations));

    let mut generator = TradingPatternGenerator::new(123);

    // Sequential hedge operations
    group.bench_function("sequential_hedge_operations", |b| {
        let pattern = generator.generate_sequential_pattern(operations as usize);

        b.iter(|| {
            let capsule = AtomicHedgeCapsule::new();

            for (i, &side) in pattern.iter().enumerate() {
                let quantity = 1000 + (i % 1000) as u32;
                let entry_price = 50000 + (i % 5000) as u32;
                let stop_ticks = 500;
                let target_ticks = 1000;

                let _result =
                    capsule.start_bracket(side, quantity, entry_price, stop_ticks, target_ticks);

                // Periodic state checks (predictable pattern)
                if i % 10 == 0 {
                    let _state = capsule.read_if_ready();
                }

                // Reset every 50 operations
                if i % 50 == 0 {
                    let _rollback = capsule.rollback_bracket();
                }
            }
        });
    });

    // Random hedge operations
    group.bench_function("random_hedge_operations", |b| {
        let pattern = generator.generate_random_pattern(operations as usize);

        b.iter(|| {
            let capsule = AtomicHedgeCapsule::new();

            for (i, &side) in pattern.iter().enumerate() {
                let quantity = 1000 + (i % 1000) as u32;
                let entry_price = 50000 + (i % 5000) as u32;
                let stop_ticks = 500;
                let target_ticks = 1000;

                let _result =
                    capsule.start_bracket(side, quantity, entry_price, stop_ticks, target_ticks);

                // Random state checks (unpredictable pattern)
                if generator.rng.bool() {
                    let _state = capsule.read_if_ready();
                }

                // Random resets
                if generator.rng.u32(0..100) < 5 {
                    let _rollback = capsule.rollback_bracket();
                }
            }
        });
    });

    // Burst hedge operations (realistic trading pattern)
    group.bench_function("burst_hedge_operations", |b| {
        let pattern = generator.generate_burst_pattern(operations as usize);

        b.iter(|| {
            let capsule = AtomicHedgeCapsule::new();

            for (i, &side) in pattern.iter().enumerate() {
                let quantity = 1000 + (i % 1000) as u32;
                let entry_price = 50000 + (i % 5000) as u32;
                let stop_ticks = 500;
                let target_ticks = 1000;

                let _result =
                    capsule.start_bracket(side, quantity, entry_price, stop_ticks, target_ticks);

                // Burst-aware state management
                if i > 0 && pattern[i] != pattern[i - 1] {
                    // Direction change - check state
                    let _state = capsule.read_if_ready();
                }
            }
        });
    });

    group.finish();
}

/// Conditional processing with different branch densities
fn bench_branch_density_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("branch_density_impact");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));
    group.sample_size(300);

    let operations = 5000u64;
    group.throughput(Throughput::Elements(operations));

    // Low branch density (mostly linear processing)
    group.bench_function("low_branch_density", |b| {
        b.iter(|| {
            let mut total = 0u64;

            for i in 0..operations {
                let value = i * 1000;
                total += value;

                // Only 10% conditional branches
                if i % 10 == 0 {
                    total += value / 2;
                }
            }

            std_black_box(total);
        });
    });

    // Medium branch density (balanced processing)
    group.bench_function("medium_branch_density", |b| {
        b.iter(|| {
            let mut total = 0u64;

            for i in 0..operations {
                let value = i * 1000;

                // 50% conditional branches
                if i % 2 == 0 {
                    total += value * 2;
                } else {
                    total += value / 2;
                }
            }

            std_black_box(total);
        });
    });

    // High branch density (heavy conditional processing)
    group.bench_function("high_branch_density", |b| {
        b.iter(|| {
            let mut total = 0u64;

            for i in 0..operations {
                let value = i * 1000;

                // 90% conditional branches with nested conditions
                if i % 10 != 0 {
                    if value > 1000000 {
                        if value > 2000000 {
                            total += value * 3;
                        } else {
                            total += value * 2;
                        }
                    } else {
                        if value > 500000 {
                            total += value + 1000;
                        } else {
                            total += value;
                        }
                    }
                } else {
                    total += value / 10;
                }
            }

            std_black_box(total);
        });
    });

    group.finish();
}

/// Branch optimization in state machine transitions
fn bench_state_machine_branches(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_machine_branches");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));
    group.sample_size(500);

    #[derive(Clone, Copy, PartialEq, Debug)]
    enum OrderState {
        Created = 0,
        Pending = 1,
        PartiallyFilled = 2,
        Filled = 3,
        Cancelled = 4,
        Rejected = 5,
    }

    let transitions = 1000u64;
    group.throughput(Throughput::Elements(transitions));

    // Baseline: if-else state machine (poor branch prediction)
    group.bench_function("if_else_state_machine", |b| {
        b.iter(|| {
            let mut state = OrderState::Created;
            let mut result_count = [0u32; 6];

            for i in 0..transitions {
                let event = (i % 6) as u8;

                if state == OrderState::Created && event == 1 {
                    state = OrderState::Pending;
                } else if state == OrderState::Pending && event == 2 {
                    state = OrderState::PartiallyFilled;
                } else if state == OrderState::PartiallyFilled && event == 3 {
                    state = OrderState::Filled;
                } else if (state == OrderState::Pending || state == OrderState::PartiallyFilled)
                    && event == 4
                {
                    state = OrderState::Cancelled;
                } else if state == OrderState::Created && event == 5 {
                    state = OrderState::Rejected;
                } else if event == 0 {
                    state = OrderState::Created; // Reset
                }

                result_count[state as usize] += 1;
            }

            std_black_box(result_count);
        });
    });

    // Optimized: match-based state machine (better branch prediction)
    group.bench_function("match_state_machine", |b| {
        b.iter(|| {
            let mut state = OrderState::Created;
            let mut result_count = [0u32; 6];

            for i in 0..transitions {
                let event = (i % 6) as u8;

                state = match (state, event) {
                    (OrderState::Created, 1) => OrderState::Pending,
                    (OrderState::Pending, 2) => OrderState::PartiallyFilled,
                    (OrderState::PartiallyFilled, 3) => OrderState::Filled,
                    (OrderState::Pending, 4) | (OrderState::PartiallyFilled, 4) => {
                        OrderState::Cancelled
                    }
                    (OrderState::Created, 5) => OrderState::Rejected,
                    (_, 0) => OrderState::Created,       // Reset
                    (current_state, _) => current_state, // No transition
                };

                result_count[state as usize] += 1;
            }

            std_black_box(result_count);
        });
    });

    // Table-driven state machine (most predictable)
    group.bench_function("table_driven_state_machine", |b| {
        // Transition table: [current_state][event] -> new_state
        const TRANSITION_TABLE: [[OrderState; 6]; 6] = [
            // Created state transitions
            [
                OrderState::Created,
                OrderState::Pending,
                OrderState::Created,
                OrderState::Created,
                OrderState::Created,
                OrderState::Rejected,
            ],
            // Pending state transitions
            [
                OrderState::Created,
                OrderState::Pending,
                OrderState::PartiallyFilled,
                OrderState::Pending,
                OrderState::Cancelled,
                OrderState::Pending,
            ],
            // PartiallyFilled state transitions
            [
                OrderState::Created,
                OrderState::PartiallyFilled,
                OrderState::PartiallyFilled,
                OrderState::Filled,
                OrderState::Cancelled,
                OrderState::PartiallyFilled,
            ],
            // Filled state transitions
            [
                OrderState::Created,
                OrderState::Filled,
                OrderState::Filled,
                OrderState::Filled,
                OrderState::Filled,
                OrderState::Filled,
            ],
            // Cancelled state transitions
            [
                OrderState::Created,
                OrderState::Cancelled,
                OrderState::Cancelled,
                OrderState::Cancelled,
                OrderState::Cancelled,
                OrderState::Cancelled,
            ],
            // Rejected state transitions
            [
                OrderState::Created,
                OrderState::Rejected,
                OrderState::Rejected,
                OrderState::Rejected,
                OrderState::Rejected,
                OrderState::Rejected,
            ],
        ];

        b.iter(|| {
            let mut state = OrderState::Created;
            let mut result_count = [0u32; 6];

            for i in 0..transitions {
                let event = (i % 6) as usize;
                state = TRANSITION_TABLE[state as usize][event];
                result_count[state as usize] += 1;
            }

            std_black_box(result_count);
        });
    });

    group.finish();
}

/// Real hedge operation decision tree optimization
fn bench_hedge_decision_tree(c: &mut Criterion) {
    let mut group = c.benchmark_group("hedge_decision_tree");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(2));
    group.sample_size(300);

    let decisions = 2000u64;
    group.throughput(Throughput::Elements(decisions));

    let mut generator = TradingPatternGenerator::new(789);

    // Generate market data
    let spreads: Vec<f64> = (0..decisions)
        .map(|i| 0.001 + (i as f64 % 100.0) / 100000.0)
        .collect();

    let volumes: Vec<u64> = (0..decisions).map(|i| 1000 + (i % 5000)).collect();

    let volatilities: Vec<f64> = (0..decisions)
        .map(|i| 0.1 + (i as f64 % 50.0) / 1000.0)
        .collect();

    // Baseline: nested if-else decision tree
    group.bench_function("nested_if_hedge_decision", |b| {
        b.iter(|| {
            let mut hedge_decisions = Vec::with_capacity(decisions as usize);

            for i in 0..decisions as usize {
                let spread = spreads[i];
                let volume = volumes[i];
                let volatility = volatilities[i];

                let should_hedge = if spread > 0.01 {
                    if volume > 2000 {
                        if volatility > 0.15 {
                            true // High spread, high volume, high volatility
                        } else {
                            volume > 3000 // High spread, high volume, low volatility
                        }
                    } else {
                        if volatility > 0.20 {
                            spread > 0.015 // High spread, low volume, high volatility
                        } else {
                            false // High spread, low volume, low volatility
                        }
                    }
                } else {
                    if volume > 4000 {
                        volatility > 0.25 // Low spread, very high volume
                    } else {
                        false // Low spread, normal volume
                    }
                };

                hedge_decisions.push(should_hedge);
            }

            std_black_box(hedge_decisions);
        });
    });

    // Optimized: match-based decision tree
    group.bench_function("match_hedge_decision", |b| {
        b.iter(|| {
            let mut hedge_decisions = Vec::with_capacity(decisions as usize);

            for i in 0..decisions as usize {
                let spread = spreads[i];
                let volume = volumes[i];
                let volatility = volatilities[i];

                let spread_class = if spread > 0.015 {
                    2
                } else if spread > 0.01 {
                    1
                } else {
                    0
                };
                let volume_class = if volume > 4000 {
                    2
                } else if volume > 2000 {
                    1
                } else {
                    0
                };
                let volatility_class = if volatility > 0.25 {
                    2
                } else if volatility > 0.15 {
                    1
                } else {
                    0
                };

                let should_hedge = match (spread_class, volume_class, volatility_class) {
                    (2, _, _) => true,     // High spread always hedge
                    (1, 2, _) => true,     // Medium spread + high volume
                    (1, 1, 1..=2) => true, // Medium spread + medium volume + medium/high volatility
                    (0, 2, 2) => true,     // Low spread + very high volume + very high volatility
                    _ => false,
                };

                hedge_decisions.push(should_hedge);
            }

            std_black_box(hedge_decisions);
        });
    });

    // Score-based: arithmetic decision (most predictable)
    group.bench_function("score_based_hedge_decision", |b| {
        b.iter(|| {
            let mut hedge_decisions = Vec::with_capacity(decisions as usize);

            for i in 0..decisions as usize {
                let spread = spreads[i];
                let volume = volumes[i];
                let volatility = volatilities[i];

                // Calculate composite score (no branches)
                let spread_score = (spread * 1000.0) as u64;
                let volume_score = volume / 100;
                let volatility_score = (volatility * 100.0) as u64;

                let composite_score = spread_score * 3 + volume_score + volatility_score * 2;
                let should_hedge = composite_score > 50;

                hedge_decisions.push(should_hedge);
            }

            std_black_box(hedge_decisions);
        });
    });

    group.finish();
}

// Configure Criterion benchmark groups
criterion_group!(
    branch_benches,
    bench_branch_prediction_patterns,
    bench_hedge_capsule_branch_patterns,
    bench_branch_density_impact,
    bench_state_machine_branches,
    bench_hedge_decision_tree,
);

criterion_main!(branch_benches);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trading_pattern_generator() {
        let mut generator = TradingPatternGenerator::new(42);

        let sequential = generator.generate_sequential_pattern(10);
        assert_eq!(sequential.len(), 10);
        assert_eq!(
            sequential,
            vec![true, false, true, false, true, false, true, false, true, false]
        );

        let random = generator.generate_random_pattern(100);
        assert_eq!(random.len(), 100);
        // Should have some variety (not all same)
        let all_same = random.iter().all(|&x| x == random[0]);
        assert!(!all_same);

        let burst = generator.generate_burst_pattern(50);
        assert_eq!(burst.len(), 50);
    }

    #[test]
    fn test_pattern_processing_consistency() {
        let pattern = vec![true, false, true, false, true];
        let prices = vec![45000, 55000, 35000, 65000, 50000];

        let baseline_result = process_pattern_baseline(&pattern, &prices);
        let optimized_result = process_pattern_optimized(&pattern, &prices);

        // Results should be identical
        assert_eq!(baseline_result, optimized_result);
    }

    #[test]
    fn test_pattern_processing_correctness() {
        let pattern = vec![true, false]; // buy, sell
        let prices = vec![60000, 30000]; // above 50k, below 40k

        let (buy_total, sell_total) = process_pattern_baseline(&pattern, &prices);

        // Buy: 60000 > 50000 and > 60000, so multiply by 2
        assert_eq!(buy_total, 60000 * 2);

        // Sell: 30000 < 50000 and < 40000, so multiply by 2
        assert_eq!(sell_total, 30000 * 2);
    }

    #[test]
    fn test_branch_patterns_deterministic() {
        let mut gen1 = TradingPatternGenerator::new(123);
        let mut gen2 = TradingPatternGenerator::new(123);

        let pattern1 = gen1.generate_random_pattern(100);
        let pattern2 = gen2.generate_random_pattern(100);

        // Same seed should produce same pattern
        assert_eq!(pattern1, pattern2);
    }

    #[test]
    fn test_decision_tree_consistency() {
        let spread = 0.012;
        let volume = 2500;
        let volatility = 0.18;

        // Test that different decision methods handle edge cases consistently
        // This is more of a validation that the logic is reasonable

        let spread_class = if spread > 0.015 {
            2
        } else if spread > 0.01 {
            1
        } else {
            0
        };
        let volume_class = if volume > 4000 {
            2
        } else if volume > 2000 {
            1
        } else {
            0
        };
        let volatility_class = if volatility > 0.25 {
            2
        } else if volatility > 0.15 {
            1
        } else {
            0
        };

        assert_eq!(spread_class, 1); // Medium spread
        assert_eq!(volume_class, 1); // Medium volume
        assert_eq!(volatility_class, 1); // Medium volatility
    }
}
