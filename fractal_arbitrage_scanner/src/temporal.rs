use aid_96::Aid96;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Minimal representation of a temporal opportunity.
///
/// A temporal arbitrage opportunity represents a predicted price movement
/// over time with associated confidence and execution delay.
///
/// # Examples
///
/// ```
/// use fractal_arbitrage_scanner::{TemporalArbitrageOpportunity, aid_class};
/// use std::time::Duration;
///
/// let temporal = TemporalArbitrageOpportunity::new(
///     "BTC/USD",
///     50_000.0,  // current price
///     51_000.0,  // predicted future price
///     0.85,      // 85% confidence
///     Duration::from_millis(150),
/// );
///
/// assert_eq!(temporal.symbol, "BTC/USD");
/// assert_eq!(temporal.current_price, 50_000.0);
/// assert_eq!(temporal.future_price, 51_000.0);
/// assert_eq!(temporal.confidence, 0.85);
/// assert_eq!(temporal.execution_delay, Duration::from_millis(150));
/// assert_eq!(temporal.id.class(), aid_class::ALT);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TemporalArbitrageOpportunity {
    pub id: Aid96,
    pub symbol: String,
    pub current_price: f64,
    pub future_price: f64,
    pub confidence: f64,
    pub execution_delay: Duration,
}

impl TemporalArbitrageOpportunity {
    /// Create a new temporal arbitrage opportunity.
    ///
    /// The confidence value is automatically clamped to the range [0.0, 1.0].
    ///
    /// # Examples
    ///
    /// ```
    /// use fractal_arbitrage_scanner::TemporalArbitrageOpportunity;
    /// use std::time::Duration;
    ///
    /// // Normal case
    /// let temporal = TemporalArbitrageOpportunity::new(
    ///     "ETH/USD",
    ///     3_000.0,
    ///     3_100.0,
    ///     0.7,
    ///     Duration::from_millis(200),
    /// );
    /// assert_eq!(temporal.confidence, 0.7);
    ///
    /// // Confidence clamping
    /// let temporal = TemporalArbitrageOpportunity::new(
    ///     "ETH/USD",
    ///     3_000.0,
    ///     3_100.0,
    ///     1.5, // Clamped to 1.0
    ///     Duration::from_millis(200),
    /// );
    /// assert_eq!(temporal.confidence, 1.0);
    ///
    /// let temporal = TemporalArbitrageOpportunity::new(
    ///     "ETH/USD",
    ///     3_000.0,
    ///     3_100.0,
    ///     -0.5, // Clamped to 0.0
    ///     Duration::from_millis(200),
    /// );
    /// assert_eq!(temporal.confidence, 0.0);
    /// ```
    pub fn new(
        symbol: impl Into<String>,
        current_price: f64,
        future_price: f64,
        confidence: f64,
        execution_delay: Duration,
    ) -> Self {
        Self {
            id: Aid96::new(aid_96::class::ALT),
            symbol: symbol.into(),
            current_price,
            future_price,
            confidence: confidence.clamp(0.0, 1.0),
            execution_delay,
        }
    }
}
