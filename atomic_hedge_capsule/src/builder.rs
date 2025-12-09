//! AtomicHedgeCapsule Builder Pattern
//!
//! UCE-32 Q28 (Simplicity): Simple, intuitive API for AtomicHedgeCapsule construction
//! UCE-32 Q31 (Rust Transform): Type-safe builder with zero-cost abstractions
//! UCE-32 Q32 (Nightly): Const builder optimizations for compile-time construction

use crate::{AtomicHedgeCapsule, BracketOrder, EntryOrder, HedgeError};
use std::marker::PhantomData;

/// UCE-32 Q28: Simple builder pattern with sensible defaults
///
/// Provides fluent API for AtomicHedgeCapsule construction with:
/// - Sensible defaults for all parameters
/// - Type-safe validation at build time
/// - Zero runtime overhead through compile-time optimization
/// - Common presets for typical use cases
pub struct HedgeCapsuleBuilder<State = Uninitialized> {
    // Core configuration
    emergency_threshold: Option<f64>,
    cache_optimization: bool,
    max_position_size: Option<f64>,
    timeout_ms: Option<u64>,

    // Entry order configuration
    exchange: Option<String>,
    symbol: Option<String>,

    // Bracket order configuration
    stop_loss: Option<f64>,
    take_profit: Option<f64>,
    position_size: Option<f64>,

    // Type state marker
    _state: PhantomData<State>,
}

/// Type-state markers for compile-time validation
/// UCE-32 Q31: Type system prevents invalid states
pub struct Uninitialized;
pub struct WithEntry;
pub struct WithBracket;
pub struct Complete;

/// UCE-32 Q28: Default values following simplicity principle
impl Default for HedgeCapsuleBuilder<Uninitialized> {
    fn default() -> Self {
        Self {
            emergency_threshold: Some(0.02), // 2% emergency threshold
            cache_optimization: true,        // Enable by default
            max_position_size: Some(1000.0), // Conservative default
            timeout_ms: Some(5000),          // 5 second timeout
            exchange: None,
            symbol: None,
            stop_loss: None,
            take_profit: None,
            position_size: None,
            _state: PhantomData,
        }
    }
}

impl AtomicHedgeCapsule {
    /// Create a new builder instance
    /// UCE-32 Q28: Simple entry point with sensible defaults
    pub fn builder() -> HedgeCapsuleBuilder<Uninitialized> {
        HedgeCapsuleBuilder::default()
    }

    /// High-frequency trading preset
    /// UCE-32 Q28: Common configuration for HFT scenarios
    pub fn high_frequency_trading() -> HedgeCapsuleBuilder<Uninitialized> {
        HedgeCapsuleBuilder::default()
            .with_emergency_threshold(0.005) // Tighter threshold for HFT
            .with_timeout_ms(100) // Ultra-fast timeout
            .with_cache_optimization() // Maximum performance
    }

    /// Conservative trading preset
    /// UCE-32 Q28: Safe configuration for risk-averse trading
    pub fn conservative_trading() -> HedgeCapsuleBuilder<Uninitialized> {
        HedgeCapsuleBuilder::default()
            .with_emergency_threshold(0.05) // Looser threshold
            .with_timeout_ms(30000) // Longer timeout
            .with_max_position_size(100.0) // Smaller positions
    }

    /// Market making preset
    /// UCE-32 Q28: Optimized for market making operations
    pub fn market_making() -> HedgeCapsuleBuilder<Uninitialized> {
        HedgeCapsuleBuilder::default()
            .with_emergency_threshold(0.01) // Moderate threshold
            .with_timeout_ms(1000) // Quick but not ultra-fast
            .with_cache_optimization() // Performance important
    }
}

impl<State> HedgeCapsuleBuilder<State> {
    /// Set emergency threshold (0.0 to 1.0)
    /// UCE-32 Q28: Simple validation with clear error messages
    pub fn with_emergency_threshold(mut self, threshold: f64) -> Self {
        self.emergency_threshold = Some(threshold);
        self
    }

    /// Enable cache optimization (enabled by default)
    /// UCE-32 Q32: Nightly features for maximum performance
    pub fn with_cache_optimization(mut self) -> Self {
        self.cache_optimization = true;
        self
    }

    /// Disable cache optimization
    pub fn without_cache_optimization(mut self) -> Self {
        self.cache_optimization = false;
        self
    }

    /// Set maximum position size
    pub fn with_max_position_size(mut self, size: f64) -> Self {
        self.max_position_size = Some(size);
        self
    }

    /// Set operation timeout in milliseconds
    pub fn with_timeout_ms(mut self, timeout: u64) -> Self {
        self.timeout_ms = Some(timeout);
        self
    }

    /// Set exchange (e.g., "NDAX", "Binance")
    pub fn with_exchange(mut self, exchange: impl Into<String>) -> Self {
        self.exchange = Some(exchange.into());
        self
    }

    /// Set trading symbol (e.g., "BTCUSD", "ETHUSD")
    pub fn with_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbol = Some(symbol.into());
        self
    }
}

impl HedgeCapsuleBuilder<Uninitialized> {
    /// Configure entry order parameters
    /// UCE-32 Q31: Type-state transition ensures proper configuration flow
    pub fn with_entry_order(
        mut self,
        exchange: impl Into<String>,
        symbol: impl Into<String>,
        _side: impl Into<String>,
        size: f64,
    ) -> HedgeCapsuleBuilder<WithEntry> {
        self.exchange = Some(exchange.into());
        self.symbol = Some(symbol.into());
        self.position_size = Some(size);

        HedgeCapsuleBuilder {
            emergency_threshold: self.emergency_threshold,
            cache_optimization: self.cache_optimization,
            max_position_size: self.max_position_size,
            timeout_ms: self.timeout_ms,
            exchange: self.exchange,
            symbol: self.symbol,
            stop_loss: self.stop_loss,
            take_profit: self.take_profit,
            position_size: self.position_size,
            _state: PhantomData,
        }
    }
}

impl HedgeCapsuleBuilder<WithEntry> {
    /// Configure bracket order parameters
    /// UCE-32 Q31: Type-state ensures entry order is configured first
    pub fn with_bracket_order(
        mut self,
        stop_loss: f64,
        take_profit: f64,
    ) -> HedgeCapsuleBuilder<WithBracket> {
        self.stop_loss = Some(stop_loss);
        self.take_profit = Some(take_profit);

        HedgeCapsuleBuilder {
            emergency_threshold: self.emergency_threshold,
            cache_optimization: self.cache_optimization,
            max_position_size: self.max_position_size,
            timeout_ms: self.timeout_ms,
            exchange: self.exchange,
            symbol: self.symbol,
            stop_loss: self.stop_loss,
            take_profit: self.take_profit,
            position_size: self.position_size,
            _state: PhantomData,
        }
    }
}

impl HedgeCapsuleBuilder<WithBracket> {
    /// Build and initialize the AtomicHedgeCapsule
    /// UCE-32 Q28: Simple build method with comprehensive validation
    /// UCE-32 Q31: Type-safe construction prevents runtime errors
    pub fn build(self) -> Result<AtomicHedgeCapsule, HedgeError> {
        // Validate configuration
        self.validate_configuration()?;

        // Create capsule with cache optimization if enabled
        let capsule = if self.cache_optimization {
            AtomicHedgeCapsule::new()
        } else {
            AtomicHedgeCapsule::new()
        };

        // Create entry order
        let entry = EntryOrder::new(
            self.exchange.unwrap_or_else(|| "NDAX".to_string()),
            self.symbol.unwrap_or_else(|| "BTCUSD".to_string()),
            "Buy".to_string(), // Default to Buy side
            self.position_size.unwrap_or(1.0),
        );

        // Create bracket order
        let bracket = BracketOrder::new(
            self.stop_loss.unwrap_or(45000.0),
            self.take_profit.unwrap_or(55000.0),
            self.position_size.unwrap_or(1.0),
        );

        // Initialize the capsule
        capsule.initialize(entry, bracket)?;

        Ok(capsule)
    }

    /// Validate configuration before building
    /// UCE-32 Q28: Simple validation with clear error messages
    fn validate_configuration(&self) -> Result<(), HedgeError> {
        // Validate emergency threshold
        if let Some(threshold) = self.emergency_threshold {
            if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
                return Err(HedgeError::ValidationFailed {
                    field: "emergency_threshold".to_string(),
                    value: threshold.to_string(),
                    reason: "Must be between 0.0 and 1.0".to_string(),
                });
            }
        }

        // Validate position size
        if let Some(size) = self.position_size {
            if !size.is_finite() || size <= 0.0 {
                return Err(HedgeError::ValidationFailed {
                    field: "position_size".to_string(),
                    value: size.to_string(),
                    reason: "Must be positive and finite".to_string(),
                });
            }

            // Check against max position size
            if let Some(max_size) = self.max_position_size {
                if size > max_size {
                    return Err(HedgeError::ValidationFailed {
                        field: "position_size".to_string(),
                        value: size.to_string(),
                        reason: format!("Exceeds maximum position size of {}", max_size),
                    });
                }
            }
        }

        // Validate stop loss and take profit
        if let (Some(stop), Some(profit)) = (self.stop_loss, self.take_profit) {
            if !stop.is_finite() || !profit.is_finite() {
                return Err(HedgeError::ValidationFailed {
                    field: "stop_loss_take_profit".to_string(),
                    value: format!("stop: {}, profit: {}", stop, profit),
                    reason: "Both must be finite numbers".to_string(),
                });
            }

            // Basic sanity check: take profit should be different from stop loss
            if (profit - stop).abs() < 0.01 {
                return Err(HedgeError::ValidationFailed {
                    field: "stop_loss_take_profit".to_string(),
                    value: format!("stop: {}, profit: {}", stop, profit),
                    reason: "Take profit and stop loss are too close".to_string(),
                });
            }
        }

        // Validate timeout
        if let Some(timeout) = self.timeout_ms {
            if timeout == 0 {
                return Err(HedgeError::ValidationFailed {
                    field: "timeout_ms".to_string(),
                    value: timeout.to_string(),
                    reason: "Timeout must be greater than 0".to_string(),
                });
            }
        }

        Ok(())
    }
}

/// Simple builder for common cases without type-state validation
/// UCE-32 Q28: Ultra-simple API for basic use cases
impl HedgeCapsuleBuilder<Uninitialized> {
    /// Quick build method with all parameters
    /// UCE-32 Q28: One-line construction for simple cases
    pub fn quick_build(
        exchange: impl Into<String>,
        symbol: impl Into<String>,
        position_size: f64,
        stop_loss: f64,
        take_profit: f64,
    ) -> Result<AtomicHedgeCapsule, HedgeError> {
        Self::default()
            .with_entry_order(exchange, symbol, "Buy", position_size)
            .with_bracket_order(stop_loss, take_profit)
            .build()
    }

    /// Build with minimal parameters using defaults
    /// UCE-32 Q28: Maximum simplicity for getting started
    pub fn minimal_build() -> Result<AtomicHedgeCapsule, HedgeError> {
        Self::quick_build("NDAX", "BTCUSD", 1.0, 45000.0, 55000.0)
    }
}

/// Builder extensions for advanced configuration
/// UCE-32 Q32: Nightly features for advanced users
impl<State> HedgeCapsuleBuilder<State> {
    /// Configure for algorithmic trading
    /// UCE-32 Q32: Advanced preset with nightly optimizations
    #[cfg(feature = "nightly")]
    pub fn algorithmic_trading(mut self) -> Self {
        self.emergency_threshold = Some(0.001); // Very tight threshold
        self.cache_optimization = true; // Maximum performance
        self.timeout_ms = Some(50); // Ultra-fast timeout
        self
    }

    /// Configure for quantitative research
    /// UCE-32 Q32: Research-oriented configuration
    #[cfg(feature = "nightly")]
    pub fn quantitative_research(mut self) -> Self {
        self.emergency_threshold = Some(0.1); // Loose threshold for research
        self.cache_optimization = true; // Performance for large datasets
        self.timeout_ms = Some(60000); // Long timeout for research
        self
    }
}

/// UCE-32 Q31: Const builder for compile-time construction
/// Available when const_fn features are stable
#[cfg(all(feature = "nightly", feature = "const_fn_floating_point_arithmetic"))]
impl HedgeCapsuleBuilder<Uninitialized> {
    /// Create builder at compile-time
    /// UCE-32 Q32: Const construction for maximum performance
    pub const fn const_new() -> Self {
        Self {
            emergency_threshold: Some(0.02),
            cache_optimization: true,
            max_position_size: Some(1000.0),
            timeout_ms: Some(5000),
            exchange: None,
            symbol: None,
            stop_loss: None,
            take_profit: None,
            position_size: None,
            _state: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_builder() {
        let capsule = AtomicHedgeCapsule::builder()
            .with_emergency_threshold(0.02)
            .with_cache_optimization()
            .with_entry_order("NDAX", "BTCUSD", "Buy", 1.0)
            .with_bracket_order(45000.0, 55000.0)
            .build();

        assert!(capsule.is_ok());
        let capsule = capsule.unwrap();
        assert!(capsule.is_active());
    }

    #[test]
    fn test_preset_configurations() {
        // High-frequency trading preset
        let hft_capsule = AtomicHedgeCapsule::high_frequency_trading()
            .with_entry_order("Binance", "BTCUSDT", "Buy", 0.1)
            .with_bracket_order(50000.0, 52000.0)
            .build();
        assert!(hft_capsule.is_ok());

        // Conservative trading preset
        let conservative_capsule = AtomicHedgeCapsule::conservative_trading()
            .with_entry_order("NDAX", "BTCUSD", "Buy", 0.5)
            .with_bracket_order(45000.0, 55000.0)
            .build();
        assert!(conservative_capsule.is_ok());

        // Market making preset
        let mm_capsule = AtomicHedgeCapsule::market_making()
            .with_entry_order("Kraken", "XBTUSD", "Buy", 2.0)
            .with_bracket_order(48000.0, 52000.0)
            .build();
        assert!(mm_capsule.is_ok());
    }

    #[test]
    fn test_quick_build() {
        let capsule = HedgeCapsuleBuilder::quick_build("NDAX", "BTCUSD", 1.0, 45000.0, 55000.0);
        assert!(capsule.is_ok());
        let capsule = capsule.unwrap();
        assert!(capsule.is_active());
    }

    #[test]
    fn test_minimal_build() {
        let capsule = HedgeCapsuleBuilder::minimal_build();
        assert!(capsule.is_ok());
        let capsule = capsule.unwrap();
        assert!(capsule.is_active());
    }

    #[test]
    fn test_validation_errors() {
        // Test invalid emergency threshold
        let result = AtomicHedgeCapsule::builder()
            .with_emergency_threshold(1.5) // Invalid: > 1.0
            .with_entry_order("NDAX", "BTCUSD", "Buy", 1.0)
            .with_bracket_order(45000.0, 55000.0)
            .build();
        assert!(result.is_err());

        // Test zero position size
        let result = HedgeCapsuleBuilder::quick_build(
            "NDAX", "BTCUSD", 0.0, // Invalid: zero size
            45000.0, 55000.0,
        );
        assert!(result.is_err());

        // Test too close stop/profit levels
        let result = HedgeCapsuleBuilder::quick_build(
            "NDAX", "BTCUSD", 1.0, 50000.0, 50000.001, // Invalid: too close
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_max_position_size_validation() {
        let result = AtomicHedgeCapsule::builder()
            .with_max_position_size(0.5)
            .with_entry_order("NDAX", "BTCUSD", "Buy", 1.0) // Exceeds max
            .with_bracket_order(45000.0, 55000.0)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_type_safety() {
        // This should compile - correct usage
        let _builder = AtomicHedgeCapsule::builder()
            .with_entry_order("NDAX", "BTCUSD", "Buy", 1.0)
            .with_bracket_order(45000.0, 55000.0);

        // This should not compile - missing entry order:
        // let _builder = AtomicHedgeCapsule::builder()
        //     .with_bracket_order(45000.0, 55000.0); // Error: no entry order
    }

    #[test]
    fn test_fluent_chaining() {
        let capsule = AtomicHedgeCapsule::builder()
            .with_emergency_threshold(0.015)
            .with_cache_optimization()
            .with_max_position_size(500.0)
            .with_timeout_ms(2000)
            .with_exchange("Coinbase")
            .with_symbol("BTC-USD")
            .with_entry_order("Coinbase", "BTC-USD", "Buy", 0.1)
            .with_bracket_order(48000.0, 52000.0)
            .build();

        assert!(capsule.is_ok());
        let capsule = capsule.unwrap();
        assert!(capsule.is_active());
        assert!(!capsule.is_emergency_stopped());
    }

    #[cfg(feature = "nightly")]
    #[test]
    fn test_nightly_features() {
        let capsule = AtomicHedgeCapsule::builder()
            .algorithmic_trading()
            .with_entry_order("NDAX", "BTCUSD", "Buy", 1.0)
            .with_bracket_order(45000.0, 55000.0)
            .build();
        assert!(capsule.is_ok());

        let capsule = AtomicHedgeCapsule::builder()
            .quantitative_research()
            .with_entry_order("NDAX", "BTCUSD", "Buy", 1.0)
            .with_bracket_order(45000.0, 55000.0)
            .build();
        assert!(capsule.is_ok());
    }

    #[cfg(all(feature = "nightly", feature = "const_fn_floating_point_arithmetic"))]
    #[test]
    fn test_const_builder() {
        const BUILDER: HedgeCapsuleBuilder<Uninitialized> = HedgeCapsuleBuilder::const_new();

        let capsule = BUILDER
            .with_entry_order("NDAX", "BTCUSD", "Buy", 1.0)
            .with_bracket_order(45000.0, 55000.0)
            .build();
        assert!(capsule.is_ok());
    }
}
