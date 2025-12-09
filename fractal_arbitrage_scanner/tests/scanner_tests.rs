//! Comprehensive tests for QuantumArbitrageScanner
//!
//! Implements T42 framework testing scanner functionality, error handling,
//! and integration with temporal/tunneling components.

use fractal_arbitrage_scanner::{
    aid_class, ArbitrageError, ArbitrageOpportunity, OpportunityParams, QuantumArbitrageScanner,
    TemporalArbitrageOpportunity, TunnelingOpportunity,
};
use fractal_arbitrage_scanner::tunneling_integration::BarrierType;
use std::time::Duration;

/// T42.1: Test basic scanner creation and initialization
#[test]
fn test_scanner_creation() {
    let scanner = QuantumArbitrageScanner::new(42);
    // Scanner should be created successfully
    // We can't inspect internal state directly, but we can test its methods work

    let default_scanner = QuantumArbitrageScanner::default();
    // Default scanner should also work

    // Test that both scanners can perform operations
    let result = scanner.scan_arbitrage(
        "BTC/USD",
        "binance",
        "coinbase",
        50_000.0,
        50_100.0,
        1.0,
    );
    assert!(result.is_ok());

    let default_result = default_scanner.scan_arbitrage(
        "ETH/USD",
        "kraken",
        "bitstamp",
        3_000.0,
        3_010.0,
        2.0,
    );
    assert!(default_result.is_ok());
}

/// T42.2: Test successful arbitrage opportunity scanning
#[test]
fn test_scan_arbitrage_success() {
    let scanner = QuantumArbitrageScanner::new(1337);

    let result = scanner.scan_arbitrage(
        "BTC/USD",
        "binance",
        "coinbase",
        50_000.0,
        50_100.0,
        1.0,
    );

    assert!(result.is_ok());
    let opportunity = result.unwrap();

    // Verify all fields are set correctly
    assert_eq!(opportunity.symbol, "BTC/USD");
    assert_eq!(opportunity.buy_exchange, "binance");
    assert_eq!(opportunity.sell_exchange, "coinbase");
    assert_eq!(opportunity.buy_price, 50_000.0);
    assert_eq!(opportunity.sell_price, 50_100.0);
    assert_eq!(opportunity.volume, 1.0);
    assert_eq!(opportunity.profit_basis_points, 20); // (100/50000) * 10000
    assert_eq!(opportunity.estimated_profit(), 100.0);

    // Verify the AID-96 class is correct
    assert_eq!(opportunity.id.class(), aid_class::PEX);

    // Verify timestamps are reasonable (within last second and expiry is 250ms later)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap().as_nanos() as u64;
    assert!(opportunity.timestamp_nanos <= now);
    assert!(opportunity.timestamp_nanos > now - 1_000_000_000); // Within 1 second

    let expected_expiry = opportunity.timestamp_nanos + Duration::from_millis(250).as_nanos() as u64;
    assert_eq!(opportunity.expiry_nanos, expected_expiry);
}

/// T42.3: Test arbitrage scanning with invalid prices
#[test]
fn test_scan_arbitrage_invalid_prices() {
    let scanner = QuantumArbitrageScanner::new(42);

    // Test negative buy price
    let result = scanner.scan_arbitrage(
        "BTC/USD",
        "binance",
        "coinbase",
        -50_000.0, // Invalid
        50_100.0,
        1.0,
    );
    match result {
        Err(ArbitrageError::InvalidPrice { price }) => {
            assert_eq!(price, -50_000.0);
        }
        _ => panic!("Expected InvalidPrice error for negative buy price"),
    }

    // Test zero sell price
    let result = scanner.scan_arbitrage(
        "BTC/USD",
        "binance",
        "coinbase",
        50_000.0,
        0.0, // Invalid
        1.0,
    );
    match result {
        Err(ArbitrageError::InvalidPrice { price }) => {
            assert_eq!(price, 0.0);
        }
        _ => panic!("Expected InvalidPrice error for zero sell price"),
    }

    // Test NaN prices
    let result = scanner.scan_arbitrage(
        "BTC/USD",
        "binance",
        "coinbase",
        f64::NAN, // Invalid
        50_100.0,
        1.0,
    );
    match result {
        Err(ArbitrageError::InvalidPrice { price }) => {
            assert!(price.is_nan());
        }
        _ => panic!("Expected InvalidPrice error for NaN buy price"),
    }

    // Test infinity prices
    let result = scanner.scan_arbitrage(
        "BTC/USD",
        "binance",
        "coinbase",
        50_000.0,
        f64::INFINITY, // Invalid
        1.0,
    );
    match result {
        Err(ArbitrageError::InvalidPrice { price }) => {
            assert_eq!(price, f64::INFINITY);
        }
        _ => panic!("Expected InvalidPrice error for infinity sell price"),
    }
}

/// T42.4: Test arbitrage scanning with invalid volume
#[test]
fn test_scan_arbitrage_invalid_volume() {
    let scanner = QuantumArbitrageScanner::new(42);

    // Test negative volume
    let result = scanner.scan_arbitrage(
        "BTC/USD",
        "binance",
        "coinbase",
        50_000.0,
        50_100.0,
        -1.0, // Invalid
    );
    match result {
        Err(ArbitrageError::InvalidVolume) => {}
        _ => panic!("Expected InvalidVolume error for negative volume"),
    }

    // Test zero volume
    let result = scanner.scan_arbitrage(
        "BTC/USD",
        "binance",
        "coinbase",
        50_000.0,
        50_100.0,
        0.0, // Invalid
    );
    match result {
        Err(ArbitrageError::InvalidVolume) => {}
        _ => panic!("Expected InvalidVolume error for zero volume"),
    }

    // Test NaN volume
    let result = scanner.scan_arbitrage(
        "BTC/USD",
        "binance",
        "coinbase",
        50_000.0,
        50_100.0,
        f64::NAN, // Invalid
    );
    match result {
        Err(ArbitrageError::InvalidVolume) => {}
        _ => panic!("Expected InvalidVolume error for NaN volume"),
    }
}

/// T42.5: Test negative spread handling (no actual arbitrage)
#[test]
fn test_scan_arbitrage_negative_spread() {
    let scanner = QuantumArbitrageScanner::new(42);

    // Sell price lower than buy price (no arbitrage)
    let result = scanner.scan_arbitrage(
        "BTC/USD",
        "binance",
        "coinbase",
        50_100.0, // Higher buy price
        50_000.0, // Lower sell price
        1.0,
    );

    assert!(result.is_ok());
    let opportunity = result.unwrap();

    // Should have zero profit basis points for negative spread
    assert_eq!(opportunity.profit_basis_points, 0);
    assert_eq!(opportunity.estimated_profit(), -100.0); // Loss

    // Other fields should still be set correctly
    assert_eq!(opportunity.buy_price, 50_100.0);
    assert_eq!(opportunity.sell_price, 50_000.0);
    assert_eq!(opportunity.volume, 1.0);
}

/// T42.6: Test temporal hint functionality
#[test]
fn test_temporal_hint() {
    let scanner = QuantumArbitrageScanner::new(42);

    let temporal_op = scanner.temporal_hint(
        "ETH/USD",
        3_000.0,   // current_price
        3_100.0,   // future_price
        0.75,      // confidence
        Duration::from_millis(100), // latency
    );

    // Verify temporal opportunity fields
    assert_eq!(temporal_op.symbol, "ETH/USD");
    assert_eq!(temporal_op.current_price, 3_000.0);
    assert_eq!(temporal_op.future_price, 3_100.0);
    assert_eq!(temporal_op.confidence, 0.75);
    assert_eq!(temporal_op.execution_delay, Duration::from_millis(100));

    // Verify AID-96 class is correct for temporal opportunities
    assert_eq!(temporal_op.id.class(), aid_class::ALT);
}

/// T42.7: Test temporal hint with confidence clamping
#[test]
fn test_temporal_hint_confidence_clamping() {
    let scanner = QuantumArbitrageScanner::new(42);

    // Test confidence > 1.0 gets clamped
    let temporal_op = scanner.temporal_hint(
        "BTC/USD",
        50_000.0,
        51_000.0,
        1.5, // Should be clamped to 1.0
        Duration::from_millis(50),
    );
    assert_eq!(temporal_op.confidence, 1.0);

    // Test negative confidence gets clamped to 0.0
    let temporal_op = scanner.temporal_hint(
        "BTC/USD",
        50_000.0,
        51_000.0,
        -0.5, // Should be clamped to 0.0
        Duration::from_millis(50),
    );
    assert_eq!(temporal_op.confidence, 0.0);
}

/// T42.8: Test tunneling hint functionality
#[test]
fn test_tunneling_hint() {
    let scanner = QuantumArbitrageScanner::new(1337);

    // Test resistance barrier (barrier above current price)
    let tunneling_op = scanner.tunneling_hint(
        "BTC/USD",
        50_000.0, // current_price
        51_000.0, // barrier_price (resistance)
    );

    // Verify tunneling opportunity fields
    assert_eq!(tunneling_op.symbol, "BTC/USD");
    assert_eq!(tunneling_op.current_price, 50_000.0);
    assert_eq!(tunneling_op.barrier_price, 51_000.0);
    assert_eq!(tunneling_op.barrier_type, BarrierType::Resistance);
    assert_eq!(tunneling_op.transmission_probability, 0.5);
    assert_eq!(tunneling_op.node_hint, 1337);

    // Verify profit calculation
    let expected_profit_bp = (((51_000.0 - 50_000.0) / 50_000.0) * 10_000.0_f64).abs() as u32;
    assert_eq!(tunneling_op.expected_profit_bp, expected_profit_bp);

    // Verify AID-96 class is correct for tunneling opportunities
    assert_eq!(tunneling_op.id.class(), aid_class::DOS);
}

/// T42.9: Test tunneling hint with support barrier
#[test]
fn test_tunneling_hint_support() {
    let scanner = QuantumArbitrageScanner::new(42);

    // Test support barrier (barrier below current price)
    let tunneling_op = scanner.tunneling_hint(
        "ETH/USD",
        3_000.0, // current_price
        2_900.0, // barrier_price (support)
    );

    assert_eq!(tunneling_op.barrier_type, BarrierType::Support);
    assert_eq!(tunneling_op.current_price, 3_000.0);
    assert_eq!(tunneling_op.barrier_price, 2_900.0);

    // Profit calculation should use absolute value
    let expected_profit_bp = (((2_900.0 - 3_000.0) / 3_000.0) * 10_000.0_f64).abs() as u32;
    assert_eq!(tunneling_op.expected_profit_bp, expected_profit_bp);
}

/// T42.10: Test multiple consecutive scans produce unique IDs
#[test]
fn test_unique_aid96_generation() {
    let scanner = QuantumArbitrageScanner::new(42);

    let mut ids = std::collections::HashSet::new();

    // Generate multiple opportunities
    for i in 0..100 {
        let result = scanner.scan_arbitrage(
            "BTC/USD",
            "binance",
            "coinbase",
            50_000.0 + i as f64,
            50_100.0 + i as f64,
            1.0,
        );

        assert!(result.is_ok());
        let opportunity = result.unwrap();

        // Each ID should be unique
        assert!(ids.insert(opportunity.id), "Duplicate ID generated: {:?}", opportunity.id);
    }

    // Should have 100 unique IDs
    assert_eq!(ids.len(), 100);
}

/// T42.11: Test temporal hints produce unique IDs
#[test]
fn test_temporal_unique_ids() {
    let scanner = QuantumArbitrageScanner::new(42);

    let mut ids = std::collections::HashSet::new();

    for i in 0..50 {
        let temporal_op = scanner.temporal_hint(
            "BTC/USD",
            50_000.0 + i as f64,
            51_000.0 + i as f64,
            0.5,
            Duration::from_millis(100),
        );

        assert!(ids.insert(temporal_op.id), "Duplicate temporal ID: {:?}", temporal_op.id);
    }

    assert_eq!(ids.len(), 50);
}

/// T42.12: Test tunneling hints produce unique IDs
#[test]
fn test_tunneling_unique_ids() {
    let scanner = QuantumArbitrageScanner::new(42);

    let mut ids = std::collections::HashSet::new();

    for i in 0..50 {
        let tunneling_op = scanner.tunneling_hint(
            "BTC/USD",
            50_000.0 + i as f64,
            51_000.0 + i as f64,
        );

        assert!(ids.insert(tunneling_op.id), "Duplicate tunneling ID: {:?}", tunneling_op.id);
    }

    assert_eq!(ids.len(), 50);
}

/// T42.13: Test scanner behavior with edge case prices
#[test]
fn test_scanner_edge_case_prices() {
    let scanner = QuantumArbitrageScanner::new(42);

    // Very small prices
    let result = scanner.scan_arbitrage(
        "TINY/USD",
        "exchange_a",
        "exchange_b",
        0.000001,
        0.000002,
        1_000_000.0,
    );
    assert!(result.is_ok());
    let opportunity = result.unwrap();
    assert_eq!(opportunity.estimated_profit(), 1_000_000.0 * 0.000001); // Should handle small numbers

    // Very large prices
    let result = scanner.scan_arbitrage(
        "LARGE/USD",
        "exchange_a",
        "exchange_b",
        1_000_000.0,
        1_001_000.0,
        0.1,
    );
    assert!(result.is_ok());
    let opportunity = result.unwrap();
    assert_eq!(opportunity.estimated_profit(), 100.0); // 1000 * 0.1
}

/// T42.14: Test concurrent scanner usage (basic thread safety test)
#[test]
fn test_scanner_concurrent_usage() {
    use std::sync::Arc;
    use std::thread;

    let scanner = Arc::new(QuantumArbitrageScanner::new(42));
    let mut handles = vec![];

    // Spawn multiple threads using the scanner
    for i in 0..10 {
        let scanner_clone = Arc::clone(&scanner);
        let handle = thread::spawn(move || {
            for j in 0..10 {
                let result = scanner_clone.scan_arbitrage(
                    &format!("COIN{}/USD", i),
                    "exchange_a",
                    "exchange_b",
                    1000.0 + (i * 10 + j) as f64,
                    1010.0 + (i * 10 + j) as f64,
                    1.0,
                );
                assert!(result.is_ok());
            }
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }
}

/// T42.15: Test timing and expiry validation
#[test]
fn test_timing_and_expiry() {
    let scanner = QuantumArbitrageScanner::new(42);

    let start_time = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap().as_nanos() as u64;

    let result = scanner.scan_arbitrage(
        "BTC/USD",
        "binance",
        "coinbase",
        50_000.0,
        50_100.0,
        1.0,
    );

    assert!(result.is_ok());
    let opportunity = result.unwrap();

    let end_time = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap().as_nanos() as u64;

    // Timestamp should be between start and end of test
    assert!(opportunity.timestamp_nanos >= start_time);
    assert!(opportunity.timestamp_nanos <= end_time);

    // Expiry should be 250ms after timestamp
    let expected_expiry = opportunity.timestamp_nanos + Duration::from_millis(250).as_nanos() as u64;
    assert_eq!(opportunity.expiry_nanos, expected_expiry);

    // Expiry should be in the future relative to start time
    assert!(opportunity.expiry_nanos > start_time);
}
