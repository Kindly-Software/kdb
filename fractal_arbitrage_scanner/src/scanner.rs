use crate::temporal::TemporalArbitrageOpportunity;
use crate::tunneling_integration::{TunnelingOpportunity, TunnelingScanner};
use crate::types::{ArbitrageError, ArbitrageOpportunity, OpportunityParams};
use aid_96::{class, Aid96};
use std::time::{Duration, SystemTime};

/// Lightweight arbitrage scanner that stitches together the temporal and
/// tunneling hints while minting AID-96 identifiers.
///
/// The scanner provides methods to identify arbitrage opportunities, temporal hints,
/// and tunneling opportunities across different exchanges and price barriers.
///
/// # Examples
///
/// ```
/// use fractal_arbitrage_scanner::FractalArbitrageScanner;
/// use std::time::Duration;
///
/// let scanner = FractalArbitrageScanner::new(42);
///
/// // Scan for arbitrage opportunity
/// let arbitrage = scanner.scan_arbitrage(
///     "BTC/USD",
///     "binance",
///     "coinbase",
///     50_000.0,
///     50_100.0,
///     1.0,
/// ).unwrap();
///
/// assert_eq!(arbitrage.symbol, "BTC/USD");
/// assert_eq!(arbitrage.estimated_profit(), 100.0);
///
/// // Generate temporal hint
/// let temporal = scanner.temporal_hint(
///     "BTC/USD",
///     50_000.0,
///     51_000.0,
///     0.8,
///     Duration::from_millis(100),
/// );
///
/// assert_eq!(temporal.current_price, 50_000.0);
/// assert_eq!(temporal.confidence, 0.8);
///
/// // Generate tunneling hint
/// let tunneling = scanner.tunneling_hint("BTC/USD", 50_000.0, 51_000.0);
/// assert_eq!(tunneling.current_price, 50_000.0);
/// ```
#[derive(Default)]
pub struct FractalArbitrageScanner {
    tunneling: TunnelingScanner,
}

impl FractalArbitrageScanner {
    pub fn new(node_id_hint: u16) -> Self {
        let tunneling = TunnelingScanner::new(node_id_hint);
        Self { tunneling }
    }

    pub fn scan_arbitrage(
        &self,
        symbol: &str,
        buy_exchange: &str,
        sell_exchange: &str,
        buy_price: f64,
        sell_price: f64,
        volume: f64,
    ) -> Result<ArbitrageOpportunity, ArbitrageError> {
        let id = Aid96::new(class::PEX);
        let timestamp = now_nanos();
        let params = OpportunityParams {
            buy_exchange: buy_exchange.to_string(),
            sell_exchange: sell_exchange.to_string(),
            symbol: symbol.to_string(),
            buy_price,
            sell_price,
            volume,
            timestamp_nanos: timestamp,
            ttl_nanos: Duration::from_millis(250).as_nanos() as u64,
        };
        ArbitrageOpportunity::from_params(id, params)
    }

    pub fn temporal_hint(
        &self,
        symbol: &str,
        current_price: f64,
        future_price: f64,
        confidence: f64,
        latency: Duration,
    ) -> TemporalArbitrageOpportunity {
        TemporalArbitrageOpportunity::new(symbol, current_price, future_price, confidence, latency)
    }

    pub fn tunneling_hint(
        &self,
        symbol: &str,
        current_price: f64,
        barrier_price: f64,
    ) -> TunnelingOpportunity {
        self.tunneling
            .derive_opportunity(symbol, current_price, barrier_price)
    }
}

fn now_nanos() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}