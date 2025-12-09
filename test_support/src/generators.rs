//! Test Data Generators
//!
//! Deterministic and configurable test data generators for consistent
//! benchmarking and testing across all primitives.

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::TestResult;

/// Deterministic random number generator for reproducible tests
#[derive(Debug)]
pub struct DeterministicRng {
    rng: ChaCha20Rng,
    seed: u64,
}

/// Test data generator with various distribution patterns
#[derive(Debug)]
pub struct TestDataGenerator {
    rng: DeterministicRng,
    config: GeneratorConfig,
}

/// Configuration for data generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratorConfig {
    pub seed: u64,
    pub pattern: DataPattern,
    pub size_range: (usize, usize),
    pub value_range: (f64, f64),
    pub distribution: Distribution,
}

/// Data generation patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataPattern {
    Random,
    Sequential,
    Zipfian { alpha: f64 },
    Gaussian { mean: f64, std_dev: f64 },
    Uniform,
    Bimodal { peaks: (f64, f64), ratio: f64 },
    Temporal { trend: f64, seasonality: f64 },
}

/// Statistical distributions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Distribution {
    Uniform,
    Normal { mean: f64, std_dev: f64 },
    Exponential { lambda: f64 },
    Poisson { lambda: f64 },
    Zipfian { s: f64 },
}

/// Market data generator for financial testing
#[derive(Debug)]
pub struct MarketDataGenerator {
    generator: TestDataGenerator,
    market_config: MarketConfig,
}

/// Market simulation configuration
#[derive(Debug, Clone)]
pub struct MarketConfig {
    pub instruments: Vec<String>,
    pub price_ranges: HashMap<String, (f64, f64)>,
    pub volatility: f64,
    pub correlation_matrix: Option<Vec<Vec<f64>>>,
    pub market_hours: (u32, u32), // Hours in UTC
}

/// Generated market data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataPoint {
    pub instrument: String,
    pub timestamp: u64,
    pub price: f64,
    pub volume: u64,
    pub bid: f64,
    pub ask: f64,
    pub spread: f64,
}

/// Generated order book data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookData {
    pub instrument: String,
    pub timestamp: u64,
    pub bids: Vec<(f64, u64)>, // (price, quantity)
    pub asks: Vec<(f64, u64)>,
    pub last_trade: Option<(f64, u64)>,
}

impl DeterministicRng {
    /// Create new deterministic RNG with seed
    pub fn new(seed: u64) -> Self {
        Self {
            rng: ChaCha20Rng::seed_from_u64(seed),
            seed,
        }
    }

    /// Generate random u64
    pub fn gen_u64(&mut self) -> u64 {
        self.rng.gen()
    }

    /// Generate random f64 in range [0, 1)
    pub fn gen_f64(&mut self) -> f64 {
        self.rng.gen()
    }

    /// Generate random value in range
    pub fn gen_range<T>(&mut self, range: std::ops::Range<T>) -> T
    where
        T: rand::distributions::uniform::SampleUniform + std::cmp::PartialOrd,
    {
        self.rng.gen_range(range)
    }

    /// Generate random bool with probability
    pub fn gen_bool(&mut self, probability: f64) -> bool {
        self.rng.gen_bool(probability)
    }

    /// Reset RNG to initial seed
    pub fn reset(&mut self) {
        self.rng = ChaCha20Rng::seed_from_u64(self.seed);
    }

    /// Get current seed
    pub fn seed(&self) -> u64 {
        self.seed
    }
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            seed: 12345,
            pattern: DataPattern::Random,
            size_range: (100, 10000),
            value_range: (0.0, 1000.0),
            distribution: Distribution::Uniform,
        }
    }
}

impl TestDataGenerator {
    /// Create new test data generator
    pub fn new(config: GeneratorConfig) -> Self {
        Self {
            rng: DeterministicRng::new(config.seed),
            config,
        }
    }

    /// Create generator with default configuration
    pub fn default_config() -> Self {
        Self::new(GeneratorConfig::default())
    }

    /// Generate vector of test values
    pub fn generate_values(&mut self, count: usize) -> TestResult<Vec<f64>> {
        let mut values = Vec::with_capacity(count);

        match self.config.pattern {
            DataPattern::Random => {
                for _ in 0..count {
                    values.push(self.generate_random_value());
                }
            }
            DataPattern::Sequential => {
                let step = (self.config.value_range.1 - self.config.value_range.0) / count as f64;
                for i in 0..count {
                    values.push(self.config.value_range.0 + i as f64 * step);
                }
            }
            DataPattern::Zipfian { alpha } => {
                values = self.generate_zipfian_values(count, alpha)?;
            }
            DataPattern::Gaussian { mean, std_dev } => {
                values = self.generate_gaussian_values(count, mean, std_dev)?;
            }
            DataPattern::Uniform => {
                for _ in 0..count {
                    values.push(self.rng.gen_range(self.config.value_range.0..self.config.value_range.1));
                }
            }
            DataPattern::Bimodal { peaks, ratio } => {
                values = self.generate_bimodal_values(count, peaks, ratio)?;
            }
            DataPattern::Temporal { trend, seasonality } => {
                values = self.generate_temporal_values(count, trend, seasonality)?;
            }
        }

        Ok(values)
    }

    /// Generate key-value pairs for map testing
    pub fn generate_key_value_pairs(&mut self, count: usize) -> TestResult<Vec<(String, f64)>> {
        let mut pairs = Vec::with_capacity(count);

        for i in 0..count {
            let key = match self.config.pattern {
                DataPattern::Sequential => format!("key_{:06}", i),
                DataPattern::Zipfian { .. } => {
                    let zipf_index = self.generate_zipfian_index(i);
                    format!("key_{:06}", zipf_index)
                }
                _ => format!("key_{:06}_{}", i, self.rng.gen_u64() % 1000),
            };

            let value = self.generate_random_value();
            pairs.push((key, value));
        }

        Ok(pairs)
    }

    /// Generate atomic operation sequence
    pub fn generate_atomic_operations(&mut self, count: usize) -> TestResult<Vec<AtomicOperation>> {
        let mut operations = Vec::with_capacity(count);

        for _ in 0..count {
            let op_type = match self.rng.gen_range(0..5) {
                0 => AtomicOpType::Load,
                1 => AtomicOpType::Store,
                2 => AtomicOpType::FetchAdd,
                3 => AtomicOpType::CompareExchange,
                _ => AtomicOpType::Swap,
            };

            let value = self.rng.gen_u64();
            let expected = if matches!(op_type, AtomicOpType::CompareExchange) {
                Some(self.rng.gen_u64())
            } else {
                None
            };

            operations.push(AtomicOperation {
                op_type,
                value,
                expected,
                ordering: self.generate_random_ordering(),
            });
        }

        Ok(operations)
    }

    /// Generate contention pattern for threading tests
    pub fn generate_contention_pattern(&mut self, thread_count: usize, operations_per_thread: usize) -> TestResult<Vec<Vec<usize>>> {
        let mut pattern = vec![Vec::new(); thread_count];

        match self.config.pattern {
            DataPattern::Uniform => {
                // Uniform distribution across all threads
                for thread_id in 0..thread_count {
                    for op_id in 0..operations_per_thread {
                        pattern[thread_id].push(op_id);
                    }
                }
            }
            DataPattern::Zipfian { alpha } => {
                // Zipfian distribution - some threads get more work
                let total_ops = thread_count * operations_per_thread;
                let zipf_weights = self.calculate_zipfian_weights(thread_count, alpha);

                let mut remaining_ops = total_ops;
                for (thread_id, &weight) in zipf_weights.iter().enumerate() {
                    let ops_for_thread = if thread_id == thread_count - 1 {
                        remaining_ops
                    } else {
                        ((total_ops as f64 * weight) as usize).min(remaining_ops)
                    };

                    for op_id in 0..ops_for_thread {
                        pattern[thread_id].push(op_id);
                    }
                    remaining_ops = remaining_ops.saturating_sub(ops_for_thread);
                }
            }
            _ => {
                // Default to uniform
                return self.generate_contention_pattern(thread_count, operations_per_thread);
            }
        }

        Ok(pattern)
    }

    // Helper methods for specific distributions

    fn generate_random_value(&mut self) -> f64 {
        match self.config.distribution {
            Distribution::Uniform => {
                self.rng.gen_range(self.config.value_range.0..self.config.value_range.1)
            }
            Distribution::Normal { mean, std_dev } => {
                self.generate_normal_value(mean, std_dev)
            }
            Distribution::Exponential { lambda } => {
                -lambda.ln() * self.rng.gen_f64()
            }
            _ => self.rng.gen_range(self.config.value_range.0..self.config.value_range.1),
        }
    }

    fn generate_normal_value(&mut self, mean: f64, std_dev: f64) -> f64 {
        // Box-Muller transformation
        static mut SPARE: Option<f64> = None;
        static mut HAS_SPARE: bool = false;

        unsafe {
            if HAS_SPARE {
                HAS_SPARE = false;
                return SPARE.unwrap() * std_dev + mean;
            }

            HAS_SPARE = true;
            let u = self.rng.gen_f64();
            let v = self.rng.gen_f64();
            let mag = std_dev * (-2.0 * u.ln()).sqrt();
            SPARE = Some(mag * (2.0 * std::f64::consts::PI * v).sin());
            mag * (2.0 * std::f64::consts::PI * v).cos() + mean
        }
    }

    fn generate_zipfian_values(&mut self, count: usize, alpha: f64) -> TestResult<Vec<f64>> {
        let mut values = Vec::with_capacity(count);
        let weights = self.calculate_zipfian_weights(count, alpha);

        for &weight in &weights {
            let value = self.config.value_range.0 +
                weight * (self.config.value_range.1 - self.config.value_range.0);
            values.push(value);
        }

        Ok(values)
    }

    fn calculate_zipfian_weights(&self, n: usize, alpha: f64) -> Vec<f64> {
        let mut weights = Vec::with_capacity(n);
        let mut sum = 0.0;

        // Calculate raw weights
        for i in 1..=n {
            let weight = 1.0 / (i as f64).powf(alpha);
            weights.push(weight);
            sum += weight;
        }

        // Normalize
        for weight in &mut weights {
            *weight /= sum;
        }

        weights
    }

    fn generate_zipfian_index(&mut self, max_index: usize) -> usize {
        // Simplified zipfian index generation
        let r = self.rng.gen_f64();
        let alpha = 1.0; // Default zipfian parameter
        let scaled = (max_index as f64 * r.powf(1.0 / alpha)) as usize;
        scaled.min(max_index.saturating_sub(1))
    }

    fn generate_gaussian_values(&mut self, count: usize, mean: f64, std_dev: f64) -> TestResult<Vec<f64>> {
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(self.generate_normal_value(mean, std_dev));
        }
        Ok(values)
    }

    fn generate_bimodal_values(&mut self, count: usize, peaks: (f64, f64), ratio: f64) -> TestResult<Vec<f64>> {
        let mut values = Vec::with_capacity(count);
        let std_dev = (peaks.1 - peaks.0) / 6.0; // Approximately 3 sigma separation

        for _ in 0..count {
            let value = if self.rng.gen_bool(ratio) {
                self.generate_normal_value(peaks.0, std_dev)
            } else {
                self.generate_normal_value(peaks.1, std_dev)
            };
            values.push(value);
        }

        Ok(values)
    }

    fn generate_temporal_values(&mut self, count: usize, trend: f64, seasonality: f64) -> TestResult<Vec<f64>> {
        let mut values = Vec::with_capacity(count);
        let base_value = (self.config.value_range.0 + self.config.value_range.1) / 2.0;

        for i in 0..count {
            let t = i as f64 / count as f64;
            let trend_component = trend * t;
            let seasonal_component = seasonality * (2.0 * std::f64::consts::PI * t * 4.0).sin(); // 4 cycles
            let noise = (self.rng.gen_f64() - 0.5) * 0.1 * base_value;

            let value = base_value + trend_component + seasonal_component + noise;
            values.push(value.max(self.config.value_range.0).min(self.config.value_range.1));
        }

        Ok(values)
    }

    fn generate_random_ordering(&mut self) -> std::sync::atomic::Ordering {
        use std::sync::atomic::Ordering;
        match self.rng.gen_range(0..5) {
            0 => Ordering::Relaxed,
            1 => Ordering::Acquire,
            2 => Ordering::Release,
            3 => Ordering::AcqRel,
            _ => Ordering::SeqCst,
        }
    }
}

/// Atomic operation for testing
#[derive(Debug, Clone)]
pub struct AtomicOperation {
    pub op_type: AtomicOpType,
    pub value: u64,
    pub expected: Option<u64>,
    pub ordering: std::sync::atomic::Ordering,
}

/// Types of atomic operations
#[derive(Debug, Clone)]
pub enum AtomicOpType {
    Load,
    Store,
    FetchAdd,
    CompareExchange,
    Swap,
}

impl MarketDataGenerator {
    /// Create new market data generator
    pub fn new(config: MarketConfig, seed: u64) -> Self {
        let generator_config = GeneratorConfig {
            seed,
            pattern: DataPattern::Temporal { trend: 0.01, seasonality: 0.05 },
            size_range: (100, 10000),
            value_range: (50.0, 150.0),
            distribution: Distribution::Normal { mean: 100.0, std_dev: 10.0 },
        };

        Self {
            generator: TestDataGenerator::new(generator_config),
            market_config: config,
        }
    }

    /// Generate market data sequence
    pub fn generate_market_data(&mut self, count: usize) -> TestResult<Vec<MarketDataPoint>> {
        let mut data = Vec::with_capacity(count);
        let base_timestamp = 1640995200; // 2022-01-01 00:00:00 UTC

        for i in 0..count {
            for instrument in &self.market_config.instruments {
                let price_range = self.market_config.price_ranges
                    .get(instrument)
                    .unwrap_or(&(90.0, 110.0));

                let price = self.generator.rng.gen_range(price_range.0..price_range.1);
                let spread = price * 0.001; // 0.1% spread
                let bid = price - spread / 2.0;
                let ask = price + spread / 2.0;

                let volume = self.generator.rng.gen_range(100..10000);
                let timestamp = base_timestamp + (i as u64 * 1000); // 1 second intervals

                data.push(MarketDataPoint {
                    instrument: instrument.clone(),
                    timestamp,
                    price,
                    volume,
                    bid,
                    ask,
                    spread,
                });
            }
        }

        Ok(data)
    }

    /// Generate order book snapshots
    pub fn generate_order_book(&mut self, instrument: &str, depth: usize) -> TestResult<OrderBookData> {
        let price_range = self.market_config.price_ranges
            .get(instrument)
            .unwrap_or(&(90.0, 110.0));

        let mid_price = (price_range.0 + price_range.1) / 2.0;
        let spread = mid_price * 0.001;

        let mut bids = Vec::with_capacity(depth);
        let mut asks = Vec::with_capacity(depth);

        // Generate bid levels
        for i in 0..depth {
            let price = mid_price - spread / 2.0 - (i as f64 * 0.01);
            let quantity = self.generator.rng.gen_range(100..1000);
            bids.push((price, quantity));
        }

        // Generate ask levels
        for i in 0..depth {
            let price = mid_price + spread / 2.0 + (i as f64 * 0.01);
            let quantity = self.generator.rng.gen_range(100..1000);
            asks.push((price, quantity));
        }

        let last_trade = Some((mid_price, self.generator.rng.gen_range(10..100)));

        Ok(OrderBookData {
            instrument: instrument.to_string(),
            timestamp: 1640995200,
            bids,
            asks,
            last_trade,
        })
    }
}

impl Default for MarketConfig {
    fn default() -> Self {
        let mut price_ranges = HashMap::new();
        price_ranges.insert("BTCUSD".to_string(), (20000.0, 80000.0));
        price_ranges.insert("ETHUSD".to_string(), (1000.0, 5000.0));
        price_ranges.insert("EURUSD".to_string(), (0.95, 1.25));

        Self {
            instruments: vec!["BTCUSD".to_string(), "ETHUSD".to_string(), "EURUSD".to_string()],
            price_ranges,
            volatility: 0.02,
            correlation_matrix: None,
            market_hours: (9, 17), // 9 AM to 5 PM UTC
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_rng() {
        let mut rng1 = DeterministicRng::new(12345);
        let mut rng2 = DeterministicRng::new(12345);

        // Same seed should produce same sequence
        for _ in 0..100 {
            assert_eq!(rng1.gen_u64(), rng2.gen_u64());
        }
    }

    #[test]
    fn test_test_data_generator() {
        let config = GeneratorConfig {
            seed: 12345,
            pattern: DataPattern::Uniform,
            value_range: (0.0, 100.0),
            ..Default::default()
        };

        let mut generator = TestDataGenerator::new(config);
        let values = generator.generate_values(1000).unwrap();

        assert_eq!(values.len(), 1000);
        assert!(values.iter().all(|&v| v >= 0.0 && v <= 100.0));
    }

    #[test]
    fn test_atomic_operations_generation() {
        let mut generator = TestDataGenerator::default_config();
        let operations = generator.generate_atomic_operations(100).unwrap();

        assert_eq!(operations.len(), 100);
        assert!(operations.iter().any(|op| matches!(op.op_type, AtomicOpType::Load)));
        assert!(operations.iter().any(|op| matches!(op.op_type, AtomicOpType::Store)));
    }

    #[test]
    fn test_market_data_generation() {
        let config = MarketConfig::default();
        let mut generator = MarketDataGenerator::new(config, 12345);

        let data = generator.generate_market_data(10).unwrap();
        assert!(!data.is_empty());

        // Check data structure
        for point in &data {
            assert!(point.bid < point.ask);
            assert!(point.price > 0.0);
            assert!(point.volume > 0);
        }
    }

    #[test]
    fn test_order_book_generation() {
        let config = MarketConfig::default();
        let mut generator = MarketDataGenerator::new(config, 12345);

        let order_book = generator.generate_order_book("BTCUSD", 10).unwrap();

        assert_eq!(order_book.bids.len(), 10);
        assert_eq!(order_book.asks.len(), 10);

        // Verify price ordering
        for i in 1..order_book.bids.len() {
            assert!(order_book.bids[i-1].0 > order_book.bids[i].0); // Descending prices
        }

        for i in 1..order_book.asks.len() {
            assert!(order_book.asks[i-1].0 < order_book.asks[i].0); // Ascending prices
        }
    }
}