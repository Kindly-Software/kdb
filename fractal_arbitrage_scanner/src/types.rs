use aid_96::Aid96;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Top-level error type for the simplified arbitrage scanner.
///
/// # Examples
///
/// ```
/// use fractal_arbitrage_scanner::ArbitrageError;
///
/// let error = ArbitrageError::InvalidPrice { price: -1.0 };
/// assert_eq!(format!("{}", error), "invalid price: -1");
///
/// let error = ArbitrageError::InvalidVolume;
/// assert_eq!(format!("{}", error), "volume must be positive");
///
/// let error = ArbitrageError::CalculationOverflow;
/// assert_eq!(format!("{}", error), "calculation overflow");
/// ```
#[derive(Debug, Error, Clone, PartialEq)]
pub enum ArbitrageError {
    #[error("invalid price: {price}")]
    InvalidPrice { price: f64 },
    #[error("volume must be positive")]
    InvalidVolume,
    #[error("calculation overflow")]
    CalculationOverflow,
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("cache eviction failed")]
    CacheEvictionFailed,
}

/// Parameters for creating an arbitrage opportunity.
///
/// # Examples
///
/// ```
/// use fractal_arbitrage_scanner::OpportunityParams;
/// use std::time::Duration;
///
/// let params = OpportunityParams {
///     buy_exchange: "binance".to_string(),
///     sell_exchange: "coinbase".to_string(),
///     symbol: "BTC/USD".to_string(),
///     buy_price: 50_000.0,
///     sell_price: 50_100.0,
///     volume: 1.0,
///     timestamp_nanos: 1_000_000_000_000,
///     ttl_nanos: Duration::from_millis(250).as_nanos() as u64,
/// };
///
/// assert_eq!(params.buy_exchange, "binance");
/// assert_eq!(params.symbol, "BTC/USD");
/// ```
#[derive(Debug, Clone)]
pub struct OpportunityParams {
    pub buy_exchange: String,
    pub sell_exchange: String,
    pub symbol: String,
    pub buy_price: f64,
    pub sell_price: f64,
    pub volume: f64,
    pub timestamp_nanos: u64,
    pub ttl_nanos: u64,
}

/// A minimal arbitrage opportunity decorated with an AID-96 handle.
///
/// An arbitrage opportunity represents a potential profit from price differences
/// between exchanges. It includes timing information and profit calculations.
///
/// # Examples
///
/// ```
/// use fractal_arbitrage_scanner::{Aid96, ArbitrageOpportunity, OpportunityParams, aid_class};
/// use std::time::Duration;
///
/// let id = Aid96::new(aid_class::PEX);
/// let params = OpportunityParams {
///     buy_exchange: "binance".to_string(),
///     sell_exchange: "coinbase".to_string(),
///     symbol: "BTC/USD".to_string(),
///     buy_price: 50_000.0,
///     sell_price: 50_100.0,
///     volume: 1.0,
///     timestamp_nanos: 1_000_000_000_000,
///     ttl_nanos: Duration::from_millis(250).as_nanos() as u64,
/// };
///
/// let opportunity = ArbitrageOpportunity::new(id, params).unwrap();
/// assert_eq!(opportunity.estimated_profit(), 100.0); // (50100 - 50000) * 1.0
/// assert_eq!(opportunity.profit_basis_points, 20); // (100/50000) * 10000
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArbitrageOpportunity {
    pub id: Aid96,
    pub buy_exchange: String,
    pub sell_exchange: String,
    pub symbol: String,
    pub buy_price: f64,
    pub sell_price: f64,
    pub volume: f64,
    pub profit_basis_points: u32,
    pub timestamp_nanos: u64,
    pub expiry_nanos: u64,
}

impl ArbitrageOpportunity {
    /// Create a new opportunity after validating basic invariants.
    ///
    /// # Examples
    ///
    /// ```
    /// use fractal_arbitrage_scanner::{Aid96, ArbitrageOpportunity, OpportunityParams, aid_class};
    /// use std::time::Duration;
    ///
    /// let id = Aid96::new(aid_class::PEX);
    /// let params = OpportunityParams {
    ///     buy_exchange: "binance".to_string(),
    ///     sell_exchange: "coinbase".to_string(),
    ///     symbol: "BTC/USD".to_string(),
    ///     buy_price: 50_000.0,
    ///     sell_price: 50_100.0,
    ///     volume: 1.0,
    ///     timestamp_nanos: 1_000_000_000_000,
    ///     ttl_nanos: Duration::from_millis(250).as_nanos() as u64,
    /// };
    ///
    /// let opportunity = ArbitrageOpportunity::new(id, params)?;
    /// assert_eq!(opportunity.buy_price, 50_000.0);
    /// # Ok::<(), fractal_arbitrage_scanner::ArbitrageError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `ArbitrageError::InvalidPrice` if prices are negative, zero, or non-finite:
    ///
    /// ```
    /// use fractal_arbitrage_scanner::{Aid96, ArbitrageOpportunity, OpportunityParams, ArbitrageError, aid_class};
    /// use std::time::Duration;
    ///
    /// let id = Aid96::new(aid_class::PEX);
    /// let params = OpportunityParams {
    ///     buy_exchange: "binance".to_string(),
    ///     sell_exchange: "coinbase".to_string(),
    ///     symbol: "BTC/USD".to_string(),
    ///     buy_price: -50_000.0, // Invalid negative price
    ///     sell_price: 50_100.0,
    ///     volume: 1.0,
    ///     timestamp_nanos: 1_000_000_000_000,
    ///     ttl_nanos: Duration::from_millis(250).as_nanos() as u64,
    /// };
    ///
    /// match ArbitrageOpportunity::new(id, params) {
    ///     Err(ArbitrageError::InvalidPrice { price }) => assert_eq!(price, -50_000.0),
    ///     _ => panic!("Expected InvalidPrice error"),
    /// }
    /// ```
    pub fn new(
        id: Aid96,
        buy_exchange: impl Into<String>,
        sell_exchange: impl Into<String>,
        symbol: impl Into<String>,
        buy_price: f64,
        sell_price: f64,
        volume: f64,
        timestamp_nanos: u64,
        ttl_nanos: u64,
    ) -> Result<Self, ArbitrageError> {
        let params = OpportunityParams {
            buy_exchange: buy_exchange.into(),
            sell_exchange: sell_exchange.into(),
            symbol: symbol.into(),
            buy_price,
            sell_price,
            volume,
            timestamp_nanos,
            ttl_nanos,
        };
        Self::from_params(id, params)
    }

    pub fn from_params(id: Aid96, params: OpportunityParams) -> Result<Self, ArbitrageError> {
        let OpportunityParams {
            buy_exchange,
            sell_exchange,
            symbol,
            buy_price,
            sell_price,
            volume,
            timestamp_nanos,
            ttl_nanos,
        } = params;
        if !buy_price.is_finite() || buy_price <= 0.0 {
            return Err(ArbitrageError::InvalidPrice { price: buy_price });
        }
        if !sell_price.is_finite() || sell_price <= 0.0 {
            return Err(ArbitrageError::InvalidPrice { price: sell_price });
        }
        if !volume.is_finite() || volume <= 0.0 {
            return Err(ArbitrageError::InvalidVolume);
        }

        let spread = sell_price - buy_price;
        if spread.is_sign_negative() {
            return Ok(Self {
                id,
                buy_exchange,
                sell_exchange,
                symbol,
                buy_price,
                sell_price,
                volume,
                profit_basis_points: 0,
                timestamp_nanos,
                expiry_nanos: timestamp_nanos + ttl_nanos,
            });
        }

        let profit_bp = ((spread / buy_price) * 10_000.0).round();
        if !profit_bp.is_finite() {
            return Err(ArbitrageError::CalculationOverflow);
        }

        Ok(Self {
            id,
            buy_exchange,
            sell_exchange,
            symbol,
            buy_price,
            sell_price,
            volume,
            profit_basis_points: profit_bp.clamp(0.0, u32::MAX as f64) as u32,
            timestamp_nanos,
            expiry_nanos: timestamp_nanos.saturating_add(ttl_nanos),
        })
    }

    /// Approximate profit in the traded currency.
    ///
    /// Calculates the profit as `(sell_price - buy_price) * volume`.
    /// Note that this can be negative if sell_price < buy_price.
    ///
    /// # Examples
    ///
    /// ```
    /// use fractal_arbitrage_scanner::{Aid96, ArbitrageOpportunity, OpportunityParams, aid_class};
    /// use std::time::Duration;
    ///
    /// let id = Aid96::new(aid_class::PEX);
    /// let params = OpportunityParams {
    ///     buy_exchange: "binance".to_string(),
    ///     sell_exchange: "coinbase".to_string(),
    ///     symbol: "BTC/USD".to_string(),
    ///     buy_price: 50_000.0,
    ///     sell_price: 50_100.0,
    ///     volume: 1.5,
    ///     timestamp_nanos: 1_000_000_000_000,
    ///     ttl_nanos: Duration::from_millis(250).as_nanos() as u64,
    /// };
    ///
    /// let opportunity = ArbitrageOpportunity::new(id, params).unwrap();
    /// assert_eq!(opportunity.estimated_profit(), 150.0); // (50100 - 50000) * 1.5
    /// ```
    pub fn estimated_profit(&self) -> f64 {
        (self.sell_price - self.buy_price) * self.volume
    }
}
