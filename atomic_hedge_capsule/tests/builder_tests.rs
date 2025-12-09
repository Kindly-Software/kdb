//! Builder Pattern Tests for AtomicHedgeCapsule
//!
//! [TRADE SECRET] - Comprehensive validation of builder pattern implementation
//!
//! UCE-32 Q28(Simplicity): Builder pattern provides simple construction of complex coordination
//! UCE-32 Q29(Constraints): Validates real-world parameter constraints and boundaries
//! UCE-32 Q30(Validation): Empirical testing with statistical validation
//! UCE-32 Q31(Rust): Type-safe builder preventing invalid configurations

use atomic_hedge_capsule::{AtomicHedgeCapsule, BracketOrder, EntryOrder, HedgeError};
use std::sync::Arc;
use std::thread;

/// Builder Pattern Tests
///
/// UCE-32 Q28: Testing the builder pattern that simplifies complex capsule creation
/// with validation and type safety built into the construction process.

/// Test builder for AtomicHedgeCapsule construction
///
/// UCE-32 Q31: Rust transformation - type-safe builder preventing invalid states
#[derive(Debug, Clone)]
pub struct HedgeCapsuleBuilder {
    symbol: Option<String>,
    exchange: Option<String>,
    entry_size: Option<f64>,
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    order_type: Option<String>,
    price: Option<f64>,
    emergency_stop: Option<f64>,
    timeout_ms: Option<u64>,
    max_retries: Option<u32>,
}

impl HedgeCapsuleBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self {
            symbol: None,
            exchange: None,
            entry_size: None,
            stop_loss: None,
            take_profit: None,
            order_type: None,
            price: None,
            emergency_stop: None,
            timeout_ms: None,
            max_retries: None,
        }
    }

    /// Set trading symbol
    pub fn symbol(mut self, symbol: &str) -> Self {
        self.symbol = Some(symbol.to_string());
        self
    }

    /// Set exchange
    pub fn exchange(mut self, exchange: &str) -> Self {
        self.exchange = Some(exchange.to_string());
        self
    }

    /// Set entry size
    pub fn size(mut self, size: f64) -> Self {
        self.entry_size = Some(size);
        self
    }

    /// Set stop loss price
    pub fn stop_loss(mut self, stop_loss: f64) -> Self {
        self.stop_loss = Some(stop_loss);
        self
    }

    /// Set take profit price
    pub fn take_profit(mut self, take_profit: f64) -> Self {
        self.take_profit = Some(take_profit);
        self
    }

    /// Set order type
    pub fn order_type(mut self, order_type: &str) -> Self {
        self.order_type = Some(order_type.to_string());
        self
    }

    /// Set specific price (for limit orders)
    pub fn price(mut self, price: f64) -> Self {
        self.price = Some(price);
        self
    }

    /// Set emergency stop price
    pub fn emergency_stop(mut self, emergency_stop: f64) -> Self {
        self.emergency_stop = Some(emergency_stop);
        self
    }

    /// Set timeout in milliseconds
    pub fn timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Set maximum retries
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = Some(max_retries);
        self
    }

    /// Build the hedge capsule
    ///
    /// UCE-32 Q29: Validates all practical constraints before construction
    /// UCE-32 Q31: Type-safe validation preventing impossible states
    pub fn build(self) -> Result<AtomicHedgeCapsule, HedgeError> {
        // Validate required fields
        let symbol = self.symbol.ok_or_else(|| HedgeError::ValidationFailed {
            field: "symbol".to_string(),
            value: "None".to_string(),
            reason: "Symbol is required".to_string(),
        })?;

        let exchange = self.exchange.ok_or_else(|| HedgeError::ValidationFailed {
            field: "exchange".to_string(),
            value: "None".to_string(),
            reason: "Exchange is required".to_string(),
        })?;

        let size = self
            .entry_size
            .ok_or_else(|| HedgeError::ValidationFailed {
                field: "size".to_string(),
                value: "None".to_string(),
                reason: "Entry size is required".to_string(),
            })?;

        let stop_loss = self.stop_loss.ok_or_else(|| HedgeError::ValidationFailed {
            field: "stop_loss".to_string(),
            value: "None".to_string(),
            reason: "Stop loss is required".to_string(),
        })?;

        let take_profit = self
            .take_profit
            .ok_or_else(|| HedgeError::ValidationFailed {
                field: "take_profit".to_string(),
                value: "None".to_string(),
                reason: "Take profit is required".to_string(),
            })?;

        // UCE-32 Q29: Validate practical constraints
        Self::validate_constraints_static(size, stop_loss, take_profit, &self.emergency_stop)?;

        // Create entry order
        let mut entry = EntryOrder::new(exchange, symbol, "Buy".to_string(), size);

        if let Some(order_type) = self.order_type {
            entry.order_type = order_type;
        }

        if let Some(price) = self.price {
            entry = entry.with_price(price);
        }

        // Create bracket order
        let mut bracket = BracketOrder::new(stop_loss, take_profit, size);

        if let Some(emergency_stop) = self.emergency_stop {
            bracket = bracket.with_emergency_stop(emergency_stop);
        }

        // Build and initialize the capsule
        let capsule = AtomicHedgeCapsule::new();
        capsule.initialize(entry, bracket)?;

        Ok(capsule)
    }

    /// Validate all constraints
    ///
    /// UCE-32 Q29: Real-world constraint validation
    fn validate_constraints_static(
        size: f64,
        stop_loss: f64,
        take_profit: f64,
        emergency_stop: &Option<f64>,
    ) -> Result<(), HedgeError> {
        // Size validation
        if size <= 0.0 {
            return Err(HedgeError::ValueOutOfBounds {
                value: size.to_string(),
                min: "0.0".to_string(),
                max: "∞".to_string(),
            });
        }

        if size > 1_000_000.0 {
            return Err(HedgeError::ValueOutOfBounds {
                value: size.to_string(),
                min: "0.0".to_string(),
                max: "1000000.0".to_string(),
            });
        }

        // Price validation
        if !stop_loss.is_finite() || !take_profit.is_finite() {
            return Err(HedgeError::ValidationFailed {
                field: "prices".to_string(),
                value: format!("stop_loss={}, take_profit={}", stop_loss, take_profit),
                reason: "Prices must be finite".to_string(),
            });
        }

        if stop_loss <= 0.0 || take_profit <= 0.0 {
            return Err(HedgeError::ValueOutOfBounds {
                value: format!("stop_loss={}, take_profit={}", stop_loss, take_profit),
                min: "0.0".to_string(),
                max: "∞".to_string(),
            });
        }

        // Risk-reward validation
        if (take_profit - stop_loss).abs() < 0.01 {
            return Err(HedgeError::ValidationFailed {
                field: "risk_reward".to_string(),
                value: format!("spread={:.4}", (take_profit - stop_loss).abs()),
                reason: "Stop loss and take profit too close".to_string(),
            });
        }

        // Emergency stop validation if present
        if let Some(emergency_stop) = emergency_stop {
            if !emergency_stop.is_finite() || *emergency_stop <= 0.0 {
                return Err(HedgeError::ValidationFailed {
                    field: "emergency_stop".to_string(),
                    value: emergency_stop.to_string(),
                    reason: "Emergency stop must be positive and finite".to_string(),
                });
            }
        }

        Ok(())
    }
}

impl Default for HedgeCapsuleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic_construction() {
        let result = HedgeCapsuleBuilder::new()
            .symbol("BTCUSD")
            .exchange("NDAX")
            .size(1.0)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .build();

        assert!(result.is_ok(), "Builder should create valid capsule");

        let capsule = result.unwrap();
        assert!(capsule.is_active(), "Built capsule should be active");
    }

    #[test]
    fn test_builder_with_all_options() {
        let result = HedgeCapsuleBuilder::new()
            .symbol("ETHUSD")
            .exchange("COINBASE")
            .size(10.0)
            .stop_loss(3000.0)
            .take_profit(4000.0)
            .order_type("LIMIT")
            .price(3500.0)
            .emergency_stop(2500.0)
            .timeout_ms(30000)
            .max_retries(5)
            .build();

        assert!(result.is_ok(), "Builder with all options should succeed");

        let capsule = result.unwrap();
        assert!(
            capsule.is_active(),
            "Fully configured capsule should be active"
        );
    }

    #[test]
    fn test_builder_missing_required_fields() {
        // Missing symbol
        let result = HedgeCapsuleBuilder::new()
            .exchange("NDAX")
            .size(1.0)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .build();

        assert!(result.is_err(), "Should fail without symbol");

        // Missing exchange
        let result = HedgeCapsuleBuilder::new()
            .symbol("BTCUSD")
            .size(1.0)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .build();

        assert!(result.is_err(), "Should fail without exchange");

        // Missing size
        let result = HedgeCapsuleBuilder::new()
            .symbol("BTCUSD")
            .exchange("NDAX")
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .build();

        assert!(result.is_err(), "Should fail without size");
    }

    #[test]
    fn test_builder_constraint_validation() {
        // Invalid size (negative)
        let result = HedgeCapsuleBuilder::new()
            .symbol("BTCUSD")
            .exchange("NDAX")
            .size(-1.0)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .build();

        assert!(result.is_err(), "Should fail with negative size");

        // Invalid size (too large)
        let result = HedgeCapsuleBuilder::new()
            .symbol("BTCUSD")
            .exchange("NDAX")
            .size(2_000_000.0)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .build();

        assert!(result.is_err(), "Should fail with size too large");

        // Invalid prices (not finite)
        let result = HedgeCapsuleBuilder::new()
            .symbol("BTCUSD")
            .exchange("NDAX")
            .size(1.0)
            .stop_loss(f64::INFINITY)
            .take_profit(55000.0)
            .build();

        assert!(result.is_err(), "Should fail with infinite stop loss");

        // Prices too close together
        let result = HedgeCapsuleBuilder::new()
            .symbol("BTCUSD")
            .exchange("NDAX")
            .size(1.0)
            .stop_loss(50000.0)
            .take_profit(50000.005)
            .build();

        assert!(
            result.is_err(),
            "Should fail when stop loss and take profit too close"
        );
    }

    #[test]
    fn test_builder_fluent_interface() {
        // Test that builder methods can be chained fluently
        let builder = HedgeCapsuleBuilder::new()
            .symbol("BTCUSD")
            .exchange("NDAX")
            .size(1.0)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .order_type("MARKET")
            .timeout_ms(10000)
            .max_retries(3);

        let result = builder.build();
        assert!(result.is_ok(), "Fluent interface should work correctly");
    }

    #[test]
    fn test_builder_default_values() {
        let builder = HedgeCapsuleBuilder::default();

        // Should fail because required fields are None
        let result = builder.build();
        assert!(
            result.is_err(),
            "Default builder should fail without required fields"
        );
    }

    #[test]
    fn test_builder_error_messages() {
        let result = HedgeCapsuleBuilder::new().build();

        assert!(result.is_err(), "Should fail without any fields");

        let error = result.err().unwrap();
        match error {
            HedgeError::ValidationFailed { field, .. } => {
                assert_eq!(field, "symbol", "Should fail on missing symbol first");
            }
            _ => panic!("Expected ValidationFailed error"),
        }
    }

    #[test]
    fn test_builder_emergency_stop_validation() {
        // Valid emergency stop
        let result = HedgeCapsuleBuilder::new()
            .symbol("BTCUSD")
            .exchange("NDAX")
            .size(1.0)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .emergency_stop(40000.0)
            .build();

        assert!(result.is_ok(), "Valid emergency stop should work");

        // Invalid emergency stop (negative)
        let result = HedgeCapsuleBuilder::new()
            .symbol("BTCUSD")
            .exchange("NDAX")
            .size(1.0)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .emergency_stop(-1000.0)
            .build();

        assert!(result.is_err(), "Negative emergency stop should fail");

        // Invalid emergency stop (infinity)
        let result = HedgeCapsuleBuilder::new()
            .symbol("BTCUSD")
            .exchange("NDAX")
            .size(1.0)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .emergency_stop(f64::INFINITY)
            .build();

        assert!(result.is_err(), "Infinite emergency stop should fail");
    }

    #[test]
    fn test_builder_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let builder = Arc::new(
            HedgeCapsuleBuilder::new()
                .symbol("BTCUSD")
                .exchange("NDAX")
                .size(1.0)
                .stop_loss(45000.0)
                .take_profit(55000.0),
        );

        let mut handles = Vec::new();

        // Test concurrent building
        for i in 0..10 {
            let builder_clone = Arc::clone(&builder);
            let handle = thread::spawn(move || {
                let result = (*builder_clone).clone().size(1.0 + i as f64 * 0.1).build();
                assert!(result.is_ok(), "Concurrent building should succeed");
                result.unwrap()
            });
            handles.push(handle);
        }

        // Collect results
        let mut capsules = Vec::new();
        for handle in handles {
            capsules.push(handle.join().expect("Thread should not panic"));
        }

        // Verify all capsules are valid
        assert_eq!(capsules.len(), 10, "Should have 10 capsules");
        for capsule in capsules {
            assert!(capsule.is_active(), "Each capsule should be active");
        }
    }

    /// UCE-32 Q30: Performance benchmark for builder pattern
    #[test]
    fn test_builder_performance() {
        use std::time::Instant;

        const ITERATIONS: usize = 1000;
        let start = Instant::now();

        for i in 0..ITERATIONS {
            let _capsule = HedgeCapsuleBuilder::new()
                .symbol("BTCUSD")
                .exchange("NDAX")
                .size(1.0 + i as f64 * 0.001)
                .stop_loss(45000.0)
                .take_profit(55000.0)
                .build()
                .unwrap();
        }

        let duration = start.elapsed();
        let ns_per_build = duration.as_nanos() / ITERATIONS as u128;

        println!("Builder performance: {} ns per build", ns_per_build);

        // Should be fast: < 50μs per build (reasonable for production use)
        assert!(
            ns_per_build < 50_000,
            "Builder should be fast: {} ns",
            ns_per_build
        );
    }

    /// UCE-32 Q30: Statistical validation of builder pattern
    #[test]
    fn test_builder_statistical_validation() {
        const SAMPLE_SIZE: usize = 100;
        let mut build_times = Vec::with_capacity(SAMPLE_SIZE);

        for i in 0..SAMPLE_SIZE {
            let start = std::time::Instant::now();

            let _capsule = HedgeCapsuleBuilder::new()
                .symbol("BTCUSD")
                .exchange("NDAX")
                .size(1.0 + i as f64 * 0.01)
                .stop_loss(45000.0 + i as f64)
                .take_profit(55000.0 + i as f64)
                .build()
                .unwrap();

            build_times.push(start.elapsed().as_nanos());
        }

        // Calculate statistics
        let mean = build_times.iter().sum::<u128>() / build_times.len() as u128;
        let variance = build_times
            .iter()
            .map(|&x| (x as i128 - mean as i128).pow(2) as u128)
            .sum::<u128>()
            / build_times.len() as u128;
        let std_dev = (variance as f64).sqrt();

        println!(
            "Builder timing statistics (n={}): mean={}ns, std_dev={:.2}ns",
            SAMPLE_SIZE, mean, std_dev
        );

        // UCE-32 Q30: Statistical validation requirements
        assert!(
            mean < 100_000,
            "Mean build time should be < 100μs: {}ns",
            mean
        );
        assert!(
            std_dev < mean as f64 * 0.5,
            "Standard deviation should be < 50% of mean"
        );

        // Test consistency (coefficient of variation)
        let cv = std_dev / mean as f64;
        assert!(
            cv < 0.5,
            "Coefficient of variation should be < 0.5: {:.3}",
            cv
        );
    }

    #[test]
    fn test_builder_integration_with_simplified_api() {
        // Test that builder-created capsules work with simplified API
        let capsule = HedgeCapsuleBuilder::new()
            .symbol("BTCUSD")
            .exchange("NDAX")
            .size(1.0)
            .stop_loss(45000.0)
            .take_profit(55000.0)
            .build()
            .unwrap();

        // Test simplified API methods
        assert!(capsule.is_ready_to_hedge(), "Should be ready to hedge");

        let status = capsule.status();
        assert!(status.is_active, "Status should show active");
        assert!(!status.is_emergency, "Should not be in emergency");

        // Test order submission
        let result = capsule.submit_order();
        assert!(result.is_ok(), "Order submission should succeed");

        // Test progress update
        let result = capsule.update_progress(0.5);
        assert!(result.is_ok(), "Progress update should succeed");

        // Test status after updates
        let status = capsule.status();
        assert!(
            status.completion > 0.0,
            "Completion should be > 0 after progress update"
        );
    }
}
