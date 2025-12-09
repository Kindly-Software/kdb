# Parameter Sweep Guide

Complete guide for defining custom parameter grids and rating parameter tunings.

## Table of Contents
1. [Overview](#overview)
2. [Parameter Grid Basics](#parameter-grid-basics)
3. [Defining Custom Grids](#defining-custom-grids)
4. [Rating Parameter Tunings](#rating-parameter-tunings)
5. [Example Grids (19 Strategies)](#example-grids-19-strategies)
6. [Best Practices](#best-practices)

---

## Overview

The parameter sweep engine processes 300K base examples through **570 strategy parameter variants** (19 strategies × 30 tunings each) to generate 9M parameter-swept examples.

### Purpose
- **Explore parameter space**: Test 30 different parameter combinations per strategy
- **Rate performance**: Calculate Sharpe ratio, win rate, profit factor for each tuning
- **Maximize diversity**: Ensure training data covers full parameter landscape

### Architecture
```
300K base examples
        ↓ (parallel sweep via rayon)
19 strategies × 30 variants each = 570 configurations
        ↓
9M parameter-swept examples (with performance ratings)
```

---

## Parameter Grid Basics

### ParameterVariant Structure

```rust
pub struct ParameterVariant {
    /// Variant ID (0-29 for each strategy)
    pub variant_id: usize,

    /// Strategy name (e.g., "OBI", "Levy", "Trend")
    pub strategy_name: String,

    /// Parameter set as key-value pairs
    pub parameters: HashMap<String, f64>,
}
```

### Example: OBI Strategy Variants

```rust
// OBI has 2 parameters: threshold and lookback
// Grid: 5 thresholds × 6 lookbacks = 30 variants

let thresholds = [0.05, 0.10, 0.15, 0.20, 0.25];
let lookbacks = [10, 20, 50, 100, 200, 500];

for (i, &threshold) in thresholds.iter().enumerate() {
    for (j, &lookback) in lookbacks.iter().enumerate() {
        let mut params = HashMap::new();
        params.insert("threshold".to_string(), threshold);
        params.insert("lookback".to_string(), lookback as f64);

        let variant = ParameterVariant {
            variant_id: i * lookbacks.len() + j,
            strategy_name: "OBI".to_string(),
            parameters: params,
        };
    }
}
```

---

## Defining Custom Grids

### Step-by-Step Grid Creation

#### 1. Identify Strategy Parameters

**Example: Trend Following Strategy**
- `fast_period`: Moving average fast period (5-15 ticks)
- `slow_period`: Moving average slow period (20-50 ticks)
- `threshold`: Minimum crossover threshold (0.0001-0.005)

#### 2. Define Parameter Ranges

```rust
pub fn trend_variants() -> Vec<ParameterVariant> {
    let mut variants = Vec::with_capacity(30);

    // Define parameter ranges
    let fast_periods = [5, 10, 15];
    let slow_periods = [20, 30, 50];
    let thresholds = [0.0001, 0.0005, 0.001, 0.002, 0.005];

    // Grid search: 3 × 3 × 5 = 45 combinations (truncate to 30)
    for (i, &fast) in fast_periods.iter().enumerate() {
        for (j, &slow) in slow_periods.iter().enumerate() {
            for (k, &threshold) in thresholds.iter().enumerate() {
                if variants.len() >= 30 {
                    break;
                }

                let mut params = HashMap::new();
                params.insert("fast_period".to_string(), fast as f64);
                params.insert("slow_period".to_string(), slow as f64);
                params.insert("threshold".to_string(), threshold);

                variants.push(ParameterVariant {
                    variant_id: variants.len(),
                    strategy_name: "Trend".to_string(),
                    parameters: params,
                });
            }
        }
    }

    variants.truncate(30);
    variants
}
```

#### 3. Validate Grid Coverage

```rust
#[test]
fn test_trend_variants_coverage() {
    let variants = trend_variants();

    // Check count
    assert_eq!(variants.len(), 30);

    // Check parameter ranges
    let fast_min = variants.iter()
        .map(|v| v.parameters["fast_period"])
        .min_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();
    assert_eq!(fast_min, 5.0);

    let threshold_max = variants.iter()
        .map(|v| v.parameters["threshold"])
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();
    assert_eq!(threshold_max, 0.005);
}
```

### Grid Search Strategies

#### Exhaustive Grid (Small Parameter Spaces)
```rust
// 3 × 3 × 3 = 27 combinations
let param_a = [1.0, 2.0, 3.0];
let param_b = [10, 20, 30];
let param_c = [0.1, 0.5, 0.9];

for &a in &param_a {
    for &b in &param_b {
        for &c in &param_c {
            // Create variant
        }
    }
}
```

#### Logarithmic Grid (Wide Ranges)
```rust
// Logarithmically-spaced values for wide parameter ranges
fn log_space(start: f64, end: f64, n: usize) -> Vec<f64> {
    let log_start = start.ln();
    let log_end = end.ln();
    let step = (log_end - log_start) / (n - 1) as f64;

    (0..n).map(|i| (log_start + i as f64 * step).exp()).collect()
}

// Example: lookback periods from 10 to 1000
let lookbacks = log_space(10.0, 1000.0, 6);
// [10, 25, 63, 158, 398, 1000]
```

#### Random Sampling (High-Dimensional Spaces)
```rust
use rand::Rng;

fn random_sampling(n_variants: usize, seed: u64) -> Vec<ParameterVariant> {
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let mut variants = Vec::with_capacity(n_variants);

    for i in 0..n_variants {
        let mut params = HashMap::new();
        params.insert("param_a".to_string(), rng.gen_range(0.0..1.0));
        params.insert("param_b".to_string(), rng.gen_range(10.0..100.0));
        params.insert("param_c".to_string(), rng.gen_range(0.001..0.1));

        variants.push(ParameterVariant {
            variant_id: i,
            strategy_name: "Random".to_string(),
            parameters: params,
        });
    }

    variants
}
```

---

## Rating Parameter Tunings

### PerformanceMetrics Structure

```rust
pub struct PerformanceMetrics {
    /// Sharpe ratio (risk-adjusted return)
    pub sharpe_ratio: f64,

    /// Win rate (winning trades / total trades)
    pub win_rate: f64,

    /// Profit factor (gross profit / gross loss)
    pub profit_factor: f64,

    /// Total trades executed
    pub total_trades: usize,
}
```

### Calculating Metrics from Trades

```rust
impl PerformanceMetrics {
    /// Calculate from trade P&L values
    pub fn from_trades(pnl_values: &[f64]) -> Self {
        if pnl_values.is_empty() {
            return Self::zero();
        }

        let total_trades = pnl_values.len();

        // Win rate
        let wins = pnl_values.iter().filter(|&&x| x > 0.0).count();
        let win_rate = wins as f64 / total_trades as f64;

        // Sharpe ratio
        let mean_pnl = pnl_values.iter().sum::<f64>() / total_trades as f64;
        let variance = pnl_values.iter()
            .map(|&x| (x - mean_pnl).powi(2))
            .sum::<f64>() / total_trades as f64;
        let std_dev = variance.sqrt();
        let sharpe_ratio = if std_dev > 0.0 {
            mean_pnl / std_dev
        } else {
            0.0
        };

        // Profit factor
        let gross_profit: f64 = pnl_values.iter()
            .filter(|&&x| x > 0.0)
            .sum();
        let gross_loss: f64 = pnl_values.iter()
            .filter(|&&x| x < 0.0)
            .map(|x| x.abs())
            .sum();
        let profit_factor = if gross_loss > 0.0 {
            gross_profit / gross_loss
        } else if gross_profit > 0.0 {
            10.0 // Cap at 10x (perfect trades)
        } else {
            0.0
        };

        PerformanceMetrics {
            sharpe_ratio,
            win_rate,
            profit_factor,
            total_trades,
        }
    }

    /// Combined rating score (0.0 to 1.0)
    pub fn rating_score(&self) -> f64 {
        let sharpe_norm = (self.sharpe_ratio / 3.0).clamp(0.0, 1.0);
        let pf_norm = (self.profit_factor / 3.0).clamp(0.0, 1.0);

        // Weighted average: 40% Sharpe, 30% win rate, 30% profit factor
        0.4 * sharpe_norm + 0.3 * self.win_rate + 0.3 * pf_norm
    }
}
```

### Rating Example

```rust
// Example: Rate a parameter variant
let trades_pnl = vec![
    10.0,  // Win
    -5.0,  // Loss
    15.0,  // Win
    -3.0,  // Loss
    20.0,  // Win
];

let metrics = PerformanceMetrics::from_trades(&trades_pnl);

println!("Performance Metrics:");
println!("  Sharpe ratio: {:.2}", metrics.sharpe_ratio);
println!("  Win rate: {:.1}%", metrics.win_rate * 100.0);
println!("  Profit factor: {:.2}", metrics.profit_factor);
println!("  Total trades: {}", metrics.total_trades);
println!("  Rating score: {:.2}", metrics.rating_score());
```

**Output:**
```
Performance Metrics:
  Sharpe ratio: 1.47
  Win rate: 60.0%
  Profit factor: 5.63
  Rating score: 0.82
```

### Interpreting Ratings

| Rating Score | Sharpe Ratio | Win Rate | Profit Factor | Interpretation |
|--------------|--------------|----------|---------------|----------------|
| 0.9 - 1.0    | >3.0         | >70%     | >3.0          | Excellent (rare) |
| 0.7 - 0.9    | 2.0 - 3.0    | 55-70%   | 2.0 - 3.0     | Good (promising) |
| 0.5 - 0.7    | 1.0 - 2.0    | 45-55%   | 1.5 - 2.0     | Acceptable (marginal) |
| 0.3 - 0.5    | 0.5 - 1.0    | 40-45%   | 1.0 - 1.5     | Poor (break-even) |
| 0.0 - 0.3    | <0.5         | <40%     | <1.0          | Very poor (losing) |

---

## Example Grids (19 Strategies)

### 1. OBI (Order Book Imbalance)

```rust
pub fn obi_variants() -> Vec<ParameterVariant> {
    let mut variants = Vec::with_capacity(30);
    let thresholds = [0.05, 0.10, 0.15, 0.20, 0.25]; // Imbalance threshold
    let lookbacks = [10, 20, 50, 100, 200, 500];    // Lookback ticks

    for (i, &threshold) in thresholds.iter().enumerate() {
        for (j, &lookback) in lookbacks.iter().enumerate() {
            let mut params = HashMap::new();
            params.insert("threshold".to_string(), threshold);
            params.insert("lookback".to_string(), lookback as f64);

            variants.push(ParameterVariant {
                variant_id: i * lookbacks.len() + j,
                strategy_name: "OBI".to_string(),
                parameters: params,
            });
        }
    }
    variants
}
```

**Parameter Meanings:**
- `threshold`: Minimum imbalance ratio to trigger signal (0.05 = 5% imbalance)
- `lookback`: Number of ticks to calculate average imbalance

### 2. Trend Following

```rust
pub fn trend_variants() -> Vec<ParameterVariant> {
    let mut variants = Vec::with_capacity(30);
    let fast_periods = [5, 10, 15];
    let slow_periods = [20, 30, 50];
    let thresholds = [0.0001, 0.0005, 0.001, 0.002, 0.005];

    for &fast in &fast_periods {
        for &slow in &slow_periods {
            for &threshold in &thresholds {
                if variants.len() >= 30 { break; }

                let mut params = HashMap::new();
                params.insert("fast_period".to_string(), fast as f64);
                params.insert("slow_period".to_string(), slow as f64);
                params.insert("threshold".to_string(), threshold);

                variants.push(ParameterVariant {
                    variant_id: variants.len(),
                    strategy_name: "Trend".to_string(),
                    parameters: params,
                });
            }
        }
    }
    variants.truncate(30);
    variants
}
```

**Parameter Meanings:**
- `fast_period`: Fast moving average period (short-term trend)
- `slow_period`: Slow moving average period (long-term trend)
- `threshold`: Minimum crossover difference to trigger signal

### 3. RSI Divergence

```rust
pub fn rsi_divergence_variants() -> Vec<ParameterVariant> {
    let mut variants = Vec::with_capacity(30);
    let rsi_periods = [7, 14, 21];
    let overbought_levels = [60, 70, 80];
    let oversold_levels = [20, 30, 40];
    let divergence_lookbacks = [5, 10, 20];

    for &period in &rsi_periods {
        for &overbought in &overbought_levels {
            for &oversold in &oversold_levels {
                for &lookback in &divergence_lookbacks {
                    if variants.len() >= 30 { break; }

                    let mut params = HashMap::new();
                    params.insert("rsi_period".to_string(), period as f64);
                    params.insert("overbought".to_string(), overbought as f64);
                    params.insert("oversold".to_string(), oversold as f64);
                    params.insert("lookback".to_string(), lookback as f64);

                    variants.push(ParameterVariant {
                        variant_id: variants.len(),
                        strategy_name: "RSI_Divergence".to_string(),
                        parameters: params,
                    });
                }
            }
        }
    }
    variants.truncate(30);
    variants
}
```

**Parameter Meanings:**
- `rsi_period`: RSI calculation period (standard: 14)
- `overbought`: RSI level considered overbought (standard: 70)
- `oversold`: RSI level considered oversold (standard: 30)
- `lookback`: Ticks to search for divergence pattern

### 4. Volatility Breakout

```rust
pub fn volatility_breakout_variants() -> Vec<ParameterVariant> {
    let mut variants = Vec::with_capacity(30);
    let lookback_periods = [10, 20, 50];
    let std_dev_multipliers = [1.0, 1.5, 2.0, 2.5];
    let entry_thresholds = [0.5, 0.75, 1.0];

    for &lookback in &lookback_periods {
        for &multiplier in &std_dev_multipliers {
            for &threshold in &entry_thresholds {
                if variants.len() >= 30 { break; }

                let mut params = HashMap::new();
                params.insert("lookback".to_string(), lookback as f64);
                params.insert("std_multiplier".to_string(), multiplier);
                params.insert("entry_threshold".to_string(), threshold);

                variants.push(ParameterVariant {
                    variant_id: variants.len(),
                    strategy_name: "Volatility_Breakout".to_string(),
                    parameters: params,
                });
            }
        }
    }
    variants.truncate(30);
    variants
}
```

**Parameter Meanings:**
- `lookback`: Period for volatility calculation
- `std_multiplier`: Bollinger Band width (2.0 = 2 standard deviations)
- `entry_threshold`: Minimum breakout distance (fraction of band width)

### 5. Levy Flight

```rust
pub fn levy_flight_variants() -> Vec<ParameterVariant> {
    let mut variants = Vec::with_capacity(30);
    let alpha_values = [1.5, 1.6, 1.7, 1.8, 1.9]; // Levy exponent
    let jump_thresholds = [0.001, 0.002, 0.005, 0.01];
    let lookbacks = [10, 20, 50];

    for &alpha in &alpha_values {
        for &threshold in &jump_thresholds {
            for &lookback in &lookbacks {
                if variants.len() >= 30 { break; }

                let mut params = HashMap::new();
                params.insert("alpha".to_string(), alpha);
                params.insert("jump_threshold".to_string(), threshold);
                params.insert("lookback".to_string(), lookback as f64);

                variants.push(ParameterVariant {
                    variant_id: variants.len(),
                    strategy_name: "Levy_Flight".to_string(),
                    parameters: params,
                });
            }
        }
    }
    variants.truncate(30);
    variants
}
```

**Parameter Meanings:**
- `alpha`: Levy exponent (1.5-2.0, lower = heavier tails)
- `jump_threshold`: Minimum jump size to trigger signal
- `lookback`: Period for jump detection

### Template for Remaining Strategies

```rust
// 6. Volume Momentum
// 7. VPIN Toxicity
// 8. Hawkes OFI
// 9. Market Making
// 10. Microprice
// 11. Liquidity Weighted Price
// 12. Multi-Level OBI
// 13. Multi-Timeframe Momentum
// 14. Spread Dynamics
// 15. Spread Decomposition
// 16. VAMP
// 17. VWAP Mean Reversion
// 18. Iceberg Detection
// 19. HMM Regime Detector

pub fn strategy_X_variants() -> Vec<ParameterVariant> {
    let mut variants = Vec::with_capacity(30);

    // Define parameter ranges
    let param_a = [/* values */];
    let param_b = [/* values */];
    let param_c = [/* values */];

    // Grid search
    for &a in &param_a {
        for &b in &param_b {
            for &c in &param_c {
                if variants.len() >= 30 { break; }

                let mut params = HashMap::new();
                params.insert("param_a".to_string(), a);
                params.insert("param_b".to_string(), b);
                params.insert("param_c".to_string(), c);

                variants.push(ParameterVariant {
                    variant_id: variants.len(),
                    strategy_name: "Strategy_X".to_string(),
                    parameters: params,
                });
            }
        }
    }

    variants.truncate(30);
    variants
}
```

---

## Best Practices

### 1. Parameter Range Selection

**Guidelines:**
- **Start wide, narrow down**: Begin with wide ranges, refine based on initial results
- **Log-scale for wide ranges**: Use logarithmic spacing for parameters spanning orders of magnitude
- **Domain knowledge**: Leverage strategy-specific insights (e.g., RSI 70/30 levels)

**Example:**
```rust
// Wide initial range
let lookbacks_v1 = [5, 10, 50, 100, 500, 1000];

// Refined after initial sweep shows 10-100 is optimal
let lookbacks_v2 = [10, 20, 30, 50, 70, 100];
```

### 2. Grid Density

**Trade-offs:**
- **Dense grids (50+ variants)**: Better coverage, slower computation
- **Sparse grids (10-20 variants)**: Faster, may miss optimal regions

**Recommendation:** 30 variants per strategy balances coverage and speed

### 3. Validation

**Always validate grids:**
```rust
#[test]
fn test_grid_coverage() {
    let variants = my_strategy_variants();

    // Test 1: Correct count
    assert_eq!(variants.len(), 30);

    // Test 2: No duplicates
    let mut seen = HashSet::new();
    for v in &variants {
        let key = format!("{:?}", v.parameters);
        assert!(seen.insert(key), "Duplicate variant detected");
    }

    // Test 3: Parameter ranges
    for v in &variants {
        let param_a = v.parameters["param_a"];
        assert!(param_a >= 0.0 && param_a <= 1.0);
    }
}
```

### 4. Rating Interpretation

**Context matters:**
- **High Sharpe (>2.0)**: Requires consistent returns with low volatility (rare)
- **High win rate (>60%)**: May indicate small winners, large losers (check profit factor)
- **High profit factor (>2.0)**: Winners significantly outweigh losers (good sign)

**Combined assessment:**
```rust
fn interpret_rating(metrics: &PerformanceMetrics) -> &str {
    match (metrics.sharpe_ratio, metrics.win_rate, metrics.profit_factor) {
        (s, w, p) if s > 2.0 && w > 0.6 && p > 2.0 => "Excellent",
        (s, w, p) if s > 1.0 && w > 0.5 && p > 1.5 => "Good",
        (s, w, p) if s > 0.5 && w > 0.45 && p > 1.0 => "Acceptable",
        _ => "Poor",
    }
}
```

---

## Reference

### Source Code
- Parameter sweep engine: `src/training/parameter_sweep_engine.rs`
- Example grids: See `ParameterVariant::all_variants()`

### Related Documentation
- [MEGA_DATA_PIPELINE_GUIDE.md](MEGA_DATA_PIPELINE_GUIDE.md)
- [QUANTUM_TUNING_GUIDE.md](QUANTUM_TUNING_GUIDE.md)

---

**Generated:** 2025-10-07
**Version:** 1.0
**Strategies Covered:** 19 (with 5 detailed examples)
