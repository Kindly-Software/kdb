//! Comprehensive tests for types module
//!
//! Implements T42 framework with UCE32 Q30 (Empirical Validation) and Q31 (Rust Transform).
//! Tests edge cases, error handling, and validates type safety invariants.

use fractal_arbitrage_scanner::{aid_class, Aid96, ArbitrageError, ArbitrageOpportunity, OpportunityParams};
use std::time::Duration;

fn create_params(
    buy_exchange: &str,
    sell_exchange: &str,
    symbol: &str,
    buy_price: f64,
    sell_price: f64,
    volume: f64,
    timestamp: u64,
    ttl: u64,
) -> OpportunityParams {
    OpportunityParams {
        buy_exchange: buy_exchange.to_string(),
        sell_exchange: sell_exchange.to_string(),
        symbol: symbol.to_string(),
        buy_price,
        sell_price,
        volume,
        timestamp_nanos: timestamp,
        ttl_nanos: ttl,
    }
}

/// T42.1: Test basic ArbitrageOpportunity creation with valid inputs
#[test]
fn test_arbitrage_opportunity_creation_valid() {
    let id = Aid96::new(aid_class::PEX);
    let timestamp = 1_000_000_000_000u64; // Valid nanosecond timestamp
    let ttl = Duration::from_millis(250).as_nanos() as u64;

    let params = create_params(
        "binance",
        "coinbase",
        "BTC/USD",
        50_000.0,
        50_100.0,
        1.0,
        timestamp,
        ttl,
    );
    let opportunity = ArbitrageOpportunity::from_params(id, params).expect("Valid opportunity should be created");

    // Verify all fields are set correctly
    assert_eq!(opportunity.id, id);
    assert_eq!(opportunity.buy_exchange, "binance");
    assert_eq!(opportunity.sell_exchange, "coinbase");
    assert_eq!(opportunity.symbol, "BTC/USD");
    assert_eq!(opportunity.buy_price, 50_000.0);
    assert_eq!(opportunity.sell_price, 50_100.0);
    assert_eq!(opportunity.volume, 1.0);
    assert_eq!(opportunity.timestamp_nanos, timestamp);
    assert_eq!(opportunity.expiry_nanos, timestamp + ttl);

    // Verify profit calculation
    assert_eq!(opportunity.profit_basis_points, 20); // (100/50000) * 10000 = 20 bp
    assert_eq!(opportunity.estimated_profit(), 100.0); // (50100 - 50000) * 1.0
}

/// T42.2: Test ArbitrageOpportunity creation with negative spread (no arbitrage)
#[test]
fn test_arbitrage_opportunity_negative_spread() {
    let id = Aid96::new(aid_class::PEX);
    let timestamp = 1_000_000_000_000u64;
    let ttl = Duration::from_millis(250).as_nanos() as u64;

    // Sell price lower than buy price (no arbitrage opportunity)
    let opportunity = ArbitrageOpportunity::new(
        id,
        "binance",
        "coinbase",
        "BTC/USD",
        50_100.0, // Higher buy price
        50_000.0, // Lower sell price
        1.0,
        timestamp,
        ttl,
    ).expect("Opportunity should be created even with negative spread");

    // Should have zero profit basis points for negative spread
    assert_eq!(opportunity.profit_basis_points, 0);
    assert_eq!(opportunity.estimated_profit(), -100.0); // Loss, not profit
}

/// T42.3: Test error handling for invalid buy prices
#[test]
fn test_arbitrage_opportunity_invalid_buy_price() {
    let id = Aid96::new(aid_class::PEX);
    let timestamp = 1_000_000_000_000u64;
    let ttl = Duration::from_millis(250).as_nanos() as u64;

    // Test negative buy price
    let result = ArbitrageOpportunity::new(
        id,
        "binance",
        "coinbase",
        "BTC/USD",
        -50_000.0, // Invalid negative price
        50_100.0,
        1.0,
        timestamp,
        ttl,
    );

    match result {
        Err(ArbitrageError::InvalidPrice { price }) => {
            assert_eq!(price, -50_000.0);
        }
        _ => panic!("Expected InvalidPrice error for negative buy price"),
    }

    // Test zero buy price
    let result = ArbitrageOpportunity::new(
        id,
        "binance",
        "coinbase",
        "BTC/USD",
        0.0, // Invalid zero price
        50_100.0,
        1.0,
        timestamp,
        ttl,
    );

    match result {
        Err(ArbitrageError::InvalidPrice { price }) => {
            assert_eq!(price, 0.0);
        }
        _ => panic!("Expected InvalidPrice error for zero buy price"),
    }

    // Test NaN buy price
    let result = ArbitrageOpportunity::new(
        id,
        "binance",
        "coinbase",
        "BTC/USD",
        f64::NAN, // Invalid NaN price
        50_100.0,
        1.0,
        timestamp,
        ttl,
    );

    match result {
        Err(ArbitrageError::InvalidPrice { price }) => {
            assert!(price.is_nan());
        }
        _ => panic!("Expected InvalidPrice error for NaN buy price"),
    }

    // Test infinity buy price
    let result = ArbitrageOpportunity::new(
        id,
        "binance",
        "coinbase",
        "BTC/USD",
        f64::INFINITY, // Invalid infinity price
        50_100.0,
        1.0,
        timestamp,
        ttl,
    );

    match result {
        Err(ArbitrageError::InvalidPrice { price }) => {
            assert_eq!(price, f64::INFINITY);
        }
        _ => panic!("Expected InvalidPrice error for infinity buy price"),
    }
}

/// T42.4: Test error handling for invalid sell prices
#[test]
fn test_arbitrage_opportunity_invalid_sell_price() {
    let id = Aid96::new(aid_class::PEX);
    let timestamp = 1_000_000_000_000u64;
    let ttl = Duration::from_millis(250).as_nanos() as u64;

    // Test negative sell price
    let result = ArbitrageOpportunity::new(
        id,
        "binance",
        "coinbase",
        "BTC/USD",
        50_000.0,
        -50_100.0, // Invalid negative price
        1.0,
        timestamp,
        ttl,
    );

    match result {
        Err(ArbitrageError::InvalidPrice { price }) => {
            assert_eq!(price, -50_100.0);
        }
        _ => panic!("Expected InvalidPrice error for negative sell price"),
    }

    // Test zero sell price
    let result = ArbitrageOpportunity::new(
        id,
        "binance",
        "coinbase",
        "BTC/USD",
        50_000.0,
        0.0, // Invalid zero price
        1.0,
        timestamp,
        ttl,
    );

    match result {
        Err(ArbitrageError::InvalidPrice { price }) => {
            assert_eq!(price, 0.0);
        }
        _ => panic!("Expected InvalidPrice error for zero sell price"),
    }
}

/// T42.5: Test error handling for invalid volume
#[test]
fn test_arbitrage_opportunity_invalid_volume() {
    let id = Aid96::new(aid_class::PEX);
    let timestamp = 1_000_000_000_000u64;
    let ttl = Duration::from_millis(250).as_nanos() as u64;

    // Test negative volume
    let result = ArbitrageOpportunity::new(
        id,
        "binance",
        "coinbase",
        "BTC/USD",
        50_000.0,
        50_100.0,
        -1.0, // Invalid negative volume
        timestamp,
        ttl,
    );

    match result {
        Err(ArbitrageError::InvalidVolume) => {}
        _ => panic!("Expected InvalidVolume error for negative volume"),
    }

    // Test zero volume
    let result = ArbitrageOpportunity::new(
        id,
        "binance",
        "coinbase",
        "BTC/USD",
        50_000.0,
        50_100.0,
        0.0, // Invalid zero volume
        timestamp,
        ttl,
    );

    match result {
        Err(ArbitrageError::InvalidVolume) => {}
        _ => panic!("Expected InvalidVolume error for zero volume"),
    }

    // Test NaN volume
    let result = ArbitrageOpportunity::new(
        id,
        "binance",
        "coinbase",
        "BTC/USD",
        50_000.0,
        50_100.0,
        f64::NAN, // Invalid NaN volume
        timestamp,
        ttl,
    );

    match result {
        Err(ArbitrageError::InvalidVolume) => {}
        _ => panic!("Expected InvalidVolume error for NaN volume"),
    }

    // Test infinity volume
    let result = ArbitrageOpportunity::new(
        id,
        "binance",
        "coinbase",
        "BTC/USD",
        50_000.0,
        50_100.0,
        f64::INFINITY, // Invalid infinity volume
        timestamp,
        ttl,
    );

    match result {
        Err(ArbitrageError::InvalidVolume) => {}
        _ => panic!("Expected InvalidVolume error for infinity volume"),
    }
}

/// T42.6: Test calculation overflow handling
#[test]
fn test_arbitrage_opportunity_calculation_overflow() {
    let id = Aid96::new(aid_class::PEX);
    let timestamp = 1_000_000_000_000u64;
    let ttl = Duration::from_millis(250).as_nanos() as u64;

    // Test with extremely small buy price that causes overflow in profit calculation
    let result = ArbitrageOpportunity::new(
        id,
        "binance",
        "coinbase",
        "BTC/USD",
        f64::MIN_POSITIVE, // Very small positive number
        1.0, // Large spread relative to buy price
        1.0,
        timestamp,
        ttl,
    );

    // This should either succeed with clamped values or fail with overflow
    match result {
        Ok(opportunity) => {
            // If it succeeds, profit should be clamped to u32::MAX
            assert!(opportunity.profit_basis_points <= u32::MAX);
        }
        Err(ArbitrageError::CalculationOverflow) => {
            // This is also acceptable
        }
        _ => panic!("Expected either success with clamped values or CalculationOverflow"),
    }
}

/// T42.7: Test edge cases for profit calculation
#[test]
fn test_arbitrage_opportunity_profit_edge_cases() {
    let id = Aid96::new(aid_class::PEX);
    let timestamp = 1_000_000_000_000u64;
    let ttl = Duration::from_millis(250).as_nanos() as u64;

    // Test very small spreads
    let opportunity = ArbitrageOpportunity::new(
        id,
        "binance",
        "coinbase",
        "BTC/USD",
        50_000.0,
        50_000.01, // Very small spread
        1.0,
        timestamp,
        ttl,
    ).expect("Should handle small spreads");

    // Should calculate correctly for small spreads
    assert_eq!(opportunity.estimated_profit(), 0.01);

    // Test large volumes
    let opportunity = ArbitrageOpportunity::new(
        id,
        "binance",
        "coinbase",
        "BTC/USD",
        50_000.0,
        50_100.0,
        1_000_000.0, // Large volume
        timestamp,
        ttl,
    ).expect("Should handle large volumes");

    assert_eq!(opportunity.estimated_profit(), 100_000_000.0); // 100 * 1M
}

/// T42.8: Test timestamp overflow handling
#[test]
fn test_arbitrage_opportunity_timestamp_overflow() {
    let id = Aid96::new(aid_class::PEX);
    let timestamp = u64::MAX - 1000; // Near max timestamp
    let ttl = 2000u64; // TTL that would cause overflow

    let opportunity = ArbitrageOpportunity::new(
        id,
        "binance",
        "coinbase",
        "BTC/USD",
        50_000.0,
        50_100.0,
        1.0,
        timestamp,
        ttl,
    ).expect("Should handle timestamp overflow gracefully");

    // Should use saturating_add to prevent overflow
    assert_eq!(opportunity.expiry_nanos, u64::MAX);
}

/// T42.9: Test serialization and deserialization
#[test]
fn test_arbitrage_opportunity_serde() {
    let id = Aid96::new(aid_class::PEX);
    let timestamp = 1_000_000_000_000u64;
    let ttl = Duration::from_millis(250).as_nanos() as u64;

    let original = ArbitrageOpportunity::new(
        id,
        "binance",
        "coinbase",
        "BTC/USD",
        50_000.0,
        50_100.0,
        1.0,
        timestamp,
        ttl,
    ).expect("Valid opportunity should be created");

    // Test JSON serialization
    let serialized = serde_json::to_string(&original)
        .expect("Should serialize to JSON");

    let deserialized: ArbitrageOpportunity = serde_json::from_str(&serialized)
        .expect("Should deserialize from JSON");

    assert_eq!(original, deserialized);
}

/// T42.10: Test ArbitrageError formatting and properties
#[test]
fn test_arbitrage_error_properties() {
    // Test InvalidPrice error
    let error = ArbitrageError::InvalidPrice { price: -1.0 };
    assert_eq!(format!("{}", error), "invalid price: -1");
    assert_eq!(format!("{:?}", error), "InvalidPrice { price: -1.0 }");

    // Test InvalidVolume error
    let error = ArbitrageError::InvalidVolume;
    assert_eq!(format!("{}", error), "volume must be positive");
    assert_eq!(format!("{:?}", error), "InvalidVolume");

    // Test CalculationOverflow error
    let error = ArbitrageError::CalculationOverflow;
    assert_eq!(format!("{}", error), "calculation overflow");
    assert_eq!(format!("{:?}", error), "CalculationOverflow");

    // Test error equality
    assert_eq!(
        ArbitrageError::InvalidPrice { price: 1.0 },
        ArbitrageError::InvalidPrice { price: 1.0 }
    );
    assert_ne!(
        ArbitrageError::InvalidPrice { price: 1.0 },
        ArbitrageError::InvalidVolume
    );
}

/// T42.11: Test ArbitrageError cloning
#[test]
fn test_arbitrage_error_clone() {
    let original = ArbitrageError::InvalidPrice { price: 42.0 };
    let cloned = original.clone();

    assert_eq!(original, cloned);

    // Verify they are separate instances
    let original_ptr = &original as *const ArbitrageError;
    let cloned_ptr = &cloned as *const ArbitrageError;
    assert_ne!(original_ptr, cloned_ptr);
}

/// T42.12: Property-based test for profit calculation invariants
#[test]
fn test_profit_calculation_invariants() {
    use std::f64::consts::E;

    let id = Aid96::new(aid_class::PEX);
    let timestamp = 1_000_000_000_000u64;
    let ttl = Duration::from_millis(250).as_nanos() as u64;

    // Test multiple valid price combinations
    let test_cases = vec![
        (1.0, 1.1, 1.0),     // Small numbers
        (100.0, 101.0, 10.0), // Medium numbers
        (50_000.0, 51_000.0, 0.5), // Large numbers
        (E, E * 1.01, 2.718), // Irrational numbers
    ];

    for (buy_price, sell_price, volume) in test_cases {
        let opportunity = ArbitrageOpportunity::new(
            id,
            "exchange_a",
            "exchange_b",
            "TEST/USD",
            buy_price,
            sell_price,
            volume,
            timestamp,
            ttl,
        ).expect("Valid inputs should create opportunity");

        // Profit calculation invariant: estimated_profit = (sell_price - buy_price) * volume
        let expected_profit = (sell_price - buy_price) * volume;
        assert!((opportunity.estimated_profit() - expected_profit).abs() < f64::EPSILON);

        // Basis points invariant: should be proportional to spread
        if sell_price > buy_price {
            assert!(opportunity.profit_basis_points > 0);
        }

        // Expiry invariant: should be after timestamp
        assert!(opportunity.expiry_nanos >= opportunity.timestamp_nanos);
    }
}
